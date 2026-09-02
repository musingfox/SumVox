// Shared summarize → speak pipeline used by CLI subcommands and hook handlers.
// Single home for these flows; callers must not duplicate them.

use std::time::Duration;

use crate::config::{effective_disable_thinking, SumvoxConfig, TtsProviderConfig};
use crate::error::Result;
use crate::llm::GenerationRequest;
use crate::notify_log;
use crate::provider_factory::ProviderFactory;
use crate::tts::{
    create_single_tts, create_tts_from_config, resolve_tts_provider, strip_leading_audio_tag,
    TtsEngine, TtsProvider,
};

/// TTS options
#[derive(Clone)]
pub struct TtsOptions {
    pub engine: String,
    pub voice: Option<String>,
    pub rate: u32,
    pub volume: Option<u32>,
}

impl Default for TtsOptions {
    fn default() -> Self {
        Self {
            engine: "auto".to_string(),
            voice: None,
            rate: 200,
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
    for provider_config in &llm_config.providers {
        let disable_thinking = effective_disable_thinking(provider_config, &llm_config.parameters);

        let request = GenerationRequest {
            system_message: system_message.clone(),
            prompt: prompt.to_string(),
            max_tokens: llm_config.parameters.max_tokens,
            temperature: llm_config.parameters.temperature,
            disable_thinking,
        };

        match ProviderFactory::create_single(provider_config) {
            Ok(provider) => {
                if !provider.is_available() {
                    tracing::debug!("Provider {} not available, trying next", provider.name());
                    continue;
                }

                tracing::info!(
                    "Trying LLM provider: {} (model: {})",
                    provider_config.name,
                    provider_config.model
                );

                match provider.generate(&request).await {
                    Ok(response) => {
                        tracing::info!("Provider {} succeeded", provider.name());
                        tracing::debug!(
                            "LLM usage: {} input tokens, {} output tokens",
                            response.input_tokens,
                            response.output_tokens
                        );

                        return Ok(response.text.trim().to_string());
                    }
                    Err(e) => {
                        tracing::warn!("Provider {} failed: {}, trying next", provider.name(), e);
                        continue;
                    }
                }
            }
            Err(e) => {
                tracing::debug!("Failed to create provider {}: {}", provider_config.name, e);
                continue;
            }
        }
    }

    // All providers failed
    tracing::error!("All LLM providers failed");
    Ok(String::new())
}

/// Speak text using TTS
pub async fn speak_text(config: &SumvoxConfig, tts_opts: &TtsOptions, text: &str) -> Result<()> {
    // Record every agent voice report (even when muted) for the menu bar app.
    notify_log::record(text);
    if notify_log::is_muted() {
        tracing::info!("Voice muted via menu bar app, skipping TTS");
        return Ok(());
    }

    // The raw engine name disambiguates entries that share one TtsEngine
    // (cloud_tts vs gemini_tts); resolve_tts_provider matches it exactly first.
    let engine_name = tts_opts.engine.to_lowercase();
    let tts_engine = tts_opts.engine.parse().unwrap_or(TtsEngine::Auto);

    // Create TTS provider: CLI override or config fallback chain
    let provider: Box<dyn TtsProvider> = match tts_engine {
        TtsEngine::Auto => {
            // Use config fallback chain
            create_tts_from_config(&config.tts.providers)?
        }
        // An explicitly selected engine overrides which configured provider to use;
        // all attributes come from that config entry, with only explicit CLI/hook
        // voice/volume layered on top. Nothing is hardcoded.
        TtsEngine::MacOS => resolve_tts_provider(
            &config.tts.providers,
            &["macos", "say"],
            tts_opts.voice.as_deref(),
            tts_opts.rate,
            tts_opts.volume,
        )?,
        TtsEngine::Google => resolve_tts_provider(
            &config.tts.providers,
            &["google", "google_tts", "gcloud", "gemini"],
            tts_opts.voice.as_deref(),
            tts_opts.rate,
            tts_opts.volume,
        )?,
        TtsEngine::CloudTts => resolve_tts_provider(
            &config.tts.providers,
            &[
                engine_name.as_str(),
                "cloud_tts",
                "gcp_tts",
                "google_cloud",
                "gemini_tts",
            ],
            tts_opts.voice.as_deref(),
            tts_opts.rate,
            tts_opts.volume,
        )?,
        TtsEngine::AudioFile => resolve_tts_provider(
            &config.tts.providers,
            &["audio_file", "audio", "file"],
            tts_opts.voice.as_deref(),
            tts_opts.rate,
            tts_opts.volume,
        )?,
        TtsEngine::Xai => resolve_tts_provider(
            &config.tts.providers,
            &["xai", "xai_tts", "grok"],
            tts_opts.voice.as_deref(),
            tts_opts.rate,
            tts_opts.volume,
        )?,
        TtsEngine::ElevenLabs => resolve_tts_provider(
            &config.tts.providers,
            &["elevenlabs", "eleven_labs", "11labs"],
            tts_opts.voice.as_deref(),
            tts_opts.rate,
            tts_opts.volume,
        )?,
        TtsEngine::OpenAi => resolve_tts_provider(
            &config.tts.providers,
            &["openai", "openai_tts"],
            tts_opts.voice.as_deref(),
            tts_opts.rate,
            tts_opts.volume,
        )?,
        TtsEngine::Espeak => resolve_tts_provider(
            &config.tts.providers,
            &["espeak", "espeak_ng", "espeak-ng"],
            tts_opts.voice.as_deref(),
            tts_opts.rate,
            tts_opts.volume,
        )?,
        TtsEngine::Piper => resolve_tts_provider(
            &config.tts.providers,
            &["piper", "piper_tts"],
            tts_opts.voice.as_deref(),
            tts_opts.rate,
            tts_opts.volume,
        )?,
    };

    if !provider.is_available() {
        tracing::warn!("TTS provider {} not available", provider.name());
        return Ok(());
    }

    // Estimate and log cost for cloud providers
    let cost = provider.estimate_cost(text.len());
    if cost > 0.0 {
        tracing::info!("TTS cost estimate: ${:.6} for {} chars", cost, text.len());
    }

    // Speak with error handling and fallback for Auto mode
    match tts_engine {
        TtsEngine::Auto => {
            // For Auto mode, try all providers in config order
            // Pass volume override so hook-level volume (stop_volume/notification_volume) is applied
            speak_with_provider_fallback(&config.tts.providers, text, tts_opts.volume).await
        }
        _ => {
            // Single provider mode - just try once
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
    let mut last_error = None;

    for provider_config in providers {
        // Skip audio_file providers - they play sound effects,
        // not speech synthesis, and cannot render arbitrary text.
        if matches!(
            provider_config.name.to_lowercase().as_str(),
            "audio_file" | "audio" | "file"
        ) {
            tracing::debug!(
                "Skipping audio_file provider in fallback chain (not a speech synthesizer)"
            );
            continue;
        }

        // Apply volume override if provided (hook-level volume takes priority)
        let mut config_with_volume = provider_config.clone();
        if let Some(vol) = volume_override {
            config_with_volume.volume = Some(vol);
        }

        // Try to create provider
        let provider = match create_single_tts(&config_with_volume) {
            Ok(p) => p,
            Err(e) => {
                tracing::debug!(
                    "Failed to create TTS provider {}: {}",
                    provider_config.name,
                    e
                );
                last_error = Some(format!("{}: {}", provider_config.name, e));
                continue;
            }
        };

        // Check availability
        if !provider.is_available() {
            tracing::debug!(
                "TTS provider {} not available, trying next",
                provider.name()
            );
            last_error = Some(format!("{}: not available", provider.name()));
            continue;
        }

        // Log selected provider
        tracing::info!(
            "Using TTS provider: {} (voice: {})",
            provider_config.name,
            provider_config.voice.as_deref().unwrap_or("default")
        );

        // Estimate and log cost for cloud providers
        let cost = provider.estimate_cost(text.len());
        if cost > 0.0 {
            tracing::info!("TTS cost estimate: ${:.6} for {} chars", cost, text.len());
        }

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

    // All providers failed
    if let Some(err) = last_error {
        tracing::warn!(
            "All TTS providers failed. Last error: {}. Notification will be silent.",
            err
        );
    } else {
        tracing::warn!("No TTS providers available. Notification will be silent.");
    }

    Ok(())
}
