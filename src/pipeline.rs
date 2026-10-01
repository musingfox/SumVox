// Shared summarize → speak pipeline used by CLI subcommands and hook handlers.
// Single home for these flows; callers must not duplicate them.

use std::time::Duration;

use crate::config::{effective_disable_thinking, SumvoxConfig, TtsProviderConfig};
use crate::error::{Result, VoiceError};
use crate::llm::{GenerationRequest, LlmProvider};
use crate::notify_log;
use crate::provider_factory::ProviderFactory;
use crate::tts::{
    create_single_tts, resolve_tts_provider, strip_leading_audio_tag, TtsEngine, TtsProvider,
};

/// TTS options
#[derive(Clone)]
pub struct TtsOptions {
    pub engine: String,
    pub voice: Option<String>,
    pub rate: Option<u32>,
    pub volume: Option<u32>,
}

impl Default for TtsOptions {
    fn default() -> Self {
        Self {
            engine: "auto".to_string(),
            voice: None,
            rate: None,
            volume: None,
        }
    }
}

/// LLM options
pub struct LlmOptions {
    pub provider: Option<String>,
    pub model: Option<String>,
    pub timeout: u64,
}

impl Default for LlmOptions {
    fn default() -> Self {
        Self {
            provider: None,
            model: None,
            timeout: 10,
        }
    }
}

/// Generate summary using LLM
pub async fn generate_summary(
    config: &SumvoxConfig,
    llm_opts: &LlmOptions,
    system_message: Option<String>,
    prompt: &str,
) -> Result<String> {
    let llm_config = &config.llm;

    // Try providers with fallback
    if llm_opts.provider.is_some() || llm_opts.model.is_some() {
        // CLI specified at least one of provider/model - try only that provider.
        // Defaults are resolved from config, never hardcoded:
        //   provider -> first configured provider; model -> that provider's configured model.
        let provider_name = match llm_opts
            .provider
            .as_deref()
            .or_else(|| llm_config.providers.first().map(|p| p.name.as_str()))
        {
            Some(name) => name,
            None => {
                tracing::error!("No LLM provider specified and none configured");
                return Ok(String::new());
            }
        };
        let timeout = Duration::from_secs(llm_opts.timeout);

        // Find the matching provider config for model + per-provider override resolution
        let matching_provider = config
            .llm
            .providers
            .iter()
            .find(|p| p.name.to_lowercase() == provider_name.to_lowercase());

        let model_name = match llm_opts
            .model
            .as_deref()
            .or_else(|| matching_provider.map(|p| p.model.as_str()))
        {
            Some(model) => model,
            None => {
                tracing::error!(
                    "CLI provider '{}' not found in config and no --model provided",
                    provider_name
                );
                return Ok(String::new());
            }
        };

        let api_key = matching_provider.and_then(|p| p.get_api_key());

        // Resolve effective disable_thinking: provider override > global
        let disable_thinking = matching_provider
            .map(|p| effective_disable_thinking(p, &llm_config.parameters))
            .unwrap_or(llm_config.parameters.disable_thinking);

        let request = GenerationRequest {
            system_message: system_message.clone(),
            prompt: prompt.to_string(),
            max_tokens: llm_config.parameters.max_tokens,
            temperature: llm_config.parameters.temperature,
            disable_thinking,
        };

        match ProviderFactory::create_by_name(
            provider_name,
            model_name,
            timeout,
            api_key.as_deref(),
            matching_provider,
        ) {
            Ok(provider) => {
                if !provider.is_available() {
                    tracing::warn!("CLI provider {} not available", provider.name());
                    return Ok(String::new());
                }

                match provider.generate(&request).await {
                    Ok(response) => {
                        tracing::debug!(
                            "LLM usage: {} input tokens, {} output tokens",
                            response.input_tokens,
                            response.output_tokens
                        );
                        return Ok(response.text.trim().to_string());
                    }
                    Err(e) => {
                        tracing::error!("CLI provider {} failed: {}", provider.name(), e);
                        return Ok(String::new());
                    }
                }
            }
            Err(e) => {
                tracing::error!("Failed to create CLI provider {}: {}", provider_name, e);
                return Ok(String::new());
            }
        }
    }

    // Try each provider in config order until one succeeds.
    // Build a per-provider GenerationRequest so each gets its own effective disable_thinking.
    let mut attempts = llm_config.providers.iter().map(|provider_config| {
        let request = GenerationRequest {
            system_message: system_message.clone(),
            prompt: prompt.to_string(),
            max_tokens: llm_config.parameters.max_tokens,
            temperature: llm_config.parameters.temperature,
            disable_thinking: effective_disable_thinking(provider_config, &llm_config.parameters),
        };
        let provider = ProviderFactory::create_single(provider_config)
            .map_err(|e| format!("{}: {}", provider_config.name, e));
        (provider, request)
    });

    Ok(generate_with_fallback(&mut attempts).await)
}

type LlmAttempt = (
    std::result::Result<Box<dyn LlmProvider>, String>,
    GenerationRequest,
);

type TtsCandidate = std::result::Result<Box<dyn TtsProvider>, String>;

/// Run each (provider, request) in order until one generates; empty string if none do.
/// An `Err` entry is a provider that could not be created and is skipped.
async fn generate_with_fallback(attempts: &mut dyn Iterator<Item = LlmAttempt>) -> String {
    for (provider, request) in attempts {
        let provider = match provider {
            Ok(p) => p,
            Err(e) => {
                tracing::debug!("Failed to create provider {}", e);
                continue;
            }
        };

        if !provider.is_available() {
            tracing::debug!("Provider {} not available, trying next", provider.name());
            continue;
        }

        tracing::info!("Trying LLM provider: {}", provider.name());

        match provider.generate(&request).await {
            Ok(response) => {
                tracing::info!("Provider {} succeeded", provider.name());
                tracing::debug!(
                    "LLM usage: {} input tokens, {} output tokens",
                    response.input_tokens,
                    response.output_tokens
                );

                return response.text.trim().to_string();
            }
            Err(e) => {
                tracing::warn!("Provider {} failed: {}, trying next", provider.name(), e);
                continue;
            }
        }
    }

    // All providers failed
    tracing::error!("All LLM providers failed");
    String::new()
}

enum EngineChoice<'a> {
    Engine(TtsEngine),
    Named(&'a TtsProviderConfig),
}

/// Interpret `--tts`: "auto"/empty, a known engine alias, or the exact name of a
/// configured provider. Anything else is an error rather than a silent fallback.
fn select_engine<'a>(name: &str, providers: &'a [TtsProviderConfig]) -> Result<EngineChoice<'a>> {
    if name.is_empty() {
        return Ok(EngineChoice::Engine(TtsEngine::Auto));
    }
    if let Ok(engine) = name.parse::<TtsEngine>() {
        return Ok(EngineChoice::Engine(engine));
    }
    providers
        .iter()
        .find(|p| p.name.to_lowercase() == name)
        .map(EngineChoice::Named)
        .ok_or_else(|| {
            VoiceError::Config(format!(
                "Unknown TTS engine '{}': not a known engine or a configured provider name",
                name
            ))
        })
}

/// Speak text using TTS
pub async fn speak_text(config: &SumvoxConfig, tts_opts: &TtsOptions, text: &str) -> Result<()> {
    // The raw engine name disambiguates entries that share one TtsEngine
    // (cloud_tts vs gemini_tts); resolve_tts_provider matches it exactly first.
    let engine_name = tts_opts.engine.to_lowercase();
    let choice = select_engine(&engine_name, &config.tts.providers)?;

    // Record every agent voice report (even when muted) for the menu bar app.
    notify_log::record(text);
    if notify_log::is_muted() {
        tracing::info!("Voice muted via menu bar app, skipping TTS");
        return Ok(());
    }

    // Create TTS provider: CLI override or config fallback chain
    let provider: Box<dyn TtsProvider> = match choice {
        EngineChoice::Named(entry) => resolve_tts_provider(
            &config.tts.providers,
            &[entry.name.to_lowercase().as_str()],
            tts_opts.voice.as_deref(),
            tts_opts.rate,
            tts_opts.volume,
        )?,
        EngineChoice::Engine(TtsEngine::Auto) => {
            // Config fallback chain; the hook-level volume overrides each provider's own
            return speak_with_provider_fallback(&config.tts.providers, text, tts_opts.volume)
                .await;
        }
        // An explicitly selected engine overrides which configured provider to use;
        // all attributes come from that config entry, with only explicit CLI/hook
        // voice/volume layered on top. Nothing is hardcoded.
        EngineChoice::Engine(engine) => {
            // The spelling the user typed goes first so it wins over other aliases
            // (cloud_tts vs gemini_tts) when the config holds both.
            let aliases: Vec<&str> = std::iter::once(engine_name.as_str())
                .chain(
                    engine
                        .aliases()
                        .iter()
                        .copied()
                        .filter(|alias| *alias != engine_name),
                )
                .collect();
            resolve_tts_provider(
                &config.tts.providers,
                &aliases,
                tts_opts.voice.as_deref(),
                tts_opts.rate,
                tts_opts.volume,
            )?
        }
    };

    if !provider.is_available() {
        tracing::warn!("TTS provider {} not available", provider.name());
        return Ok(());
    }

    let text = if provider.supports_audio_tags() {
        text
    } else {
        strip_leading_audio_tag(text)
    };
    match provider.speak(text).await {
        Ok(_) => {
            tracing::debug!("TTS playback completed");
            Ok(())
        }
        Err(e) => {
            tracing::warn!("TTS playback failed: {}. Notification will be silent.", e);
            Ok(())
        }
    }
}

/// Try TTS providers in order with automatic runtime fallback
///
/// `volume_override` applies hook-level volume (e.g., stop_volume, notification_volume)
/// over provider-level volume settings. Priority: volume_override > provider config > default.
async fn speak_with_provider_fallback(
    providers: &[TtsProviderConfig],
    text: &str,
    volume_override: Option<u32>,
) -> Result<()> {
    let mut candidates = providers.iter().filter_map(|provider_config| {
        // Skip audio_file providers - they play sound effects,
        // not speech synthesis, and cannot render arbitrary text.
        if provider_config.name.parse::<TtsEngine>().ok() == Some(TtsEngine::AudioFile) {
            tracing::debug!(
                "Skipping audio_file provider in fallback chain (not a speech synthesizer)"
            );
            return None;
        }

        // Apply volume override if provided (hook-level volume takes priority)
        let mut config_with_volume = provider_config.clone();
        if let Some(vol) = volume_override {
            config_with_volume.volume = Some(vol);
        }

        Some(
            create_single_tts(&config_with_volume)
                .map_err(|e| format!("{}: {}", provider_config.name, e)),
        )
    });

    let result = speak_with_fallback(&mut candidates, text).await;

    // A usable audio_file entry is never spoken through, but it still counts as an
    // available TTS: such a chain stays silent instead of erroring.
    if result.is_err()
        && providers.iter().any(|c| {
            c.name.parse::<TtsEngine>().ok() == Some(TtsEngine::AudioFile)
                && create_single_tts(c).is_ok_and(|p| p.is_available())
        })
    {
        tracing::warn!("No speech TTS providers available. Notification will be silent.");
        return Ok(());
    }
    result
}

/// Speak with the first provider that is available and succeeds. An `Err` entry is a
/// provider that could not be created. Total failure of tried providers is only
/// logged; `Err` means no provider could be tried at all.
async fn speak_with_fallback(
    providers: &mut dyn Iterator<Item = TtsCandidate>,
    text: &str,
) -> Result<()> {
    let mut last_error = None;
    let mut skipped = Vec::new();

    for provider in providers {
        let provider = match provider {
            Ok(p) => p,
            Err(e) => {
                tracing::debug!("Failed to create TTS provider {}", e);
                skipped.push(e);
                continue;
            }
        };

        // Check availability
        if !provider.is_available() {
            tracing::debug!(
                "TTS provider {} not available, trying next",
                provider.name()
            );
            skipped.push(format!("{}: not available", provider.name()));
            continue;
        }

        tracing::info!("Using TTS provider: {}", provider.name());

        // Try to speak (strip audio tags for providers that would read them aloud)
        let provider_text = if provider.supports_audio_tags() {
            text
        } else {
            strip_leading_audio_tag(text)
        };
        match provider.speak(provider_text).await {
            Ok(_) => {
                tracing::debug!("TTS playback completed with {}", provider.name());
                return Ok(());
            }
            Err(e) => {
                tracing::warn!(
                    "TTS provider {} failed: {}, trying next provider",
                    provider.name(),
                    e
                );
                last_error = Some(format!("{}: {}", provider.name(), e));
                continue;
            }
        }
    }

    // A provider that was tried and failed degrades to silence; a chain where
    // nothing could even be tried is a configuration error worth surfacing.
    match last_error {
        Some(err) => {
            tracing::warn!(
                "All TTS providers failed. Last error: {}. Notification will be silent.",
                err
            );
            Ok(())
        }
        None => Err(VoiceError::Config(format!(
            "No TTS provider available. Tried: {}",
            skipped.join("; ")
        ))),
    }
}

#[cfg(test)]
mod tests;
