// Claude Code hook handler
// Processes JSON input from Claude Code Stop and Notification hooks

use std::path::PathBuf;
use std::time::Duration;

use serde::Deserialize;

use crate::config::SumvoxConfig;
use crate::error::Result;
use crate::llm::GenerationRequest;
use crate::provider_factory::ProviderFactory;
use crate::transcript::TranscriptReader;
use crate::tts::{create_tts_by_name, create_tts_from_config, TtsEngine, TtsProvider};

/// Claude Code hook input structure
#[derive(Debug, Deserialize)]
pub struct ClaudeCodeInput {
    pub session_id: String,
    pub transcript_path: String,
    #[allow(dead_code)]
    pub permission_mode: Option<String>,
    pub hook_event_name: String,
    pub stop_hook_active: Option<bool>,
    // Notification hook specific fields
    pub message: Option<String>,
    pub notification_type: Option<String>,
}

impl ClaudeCodeInput {
    /// Parse from JSON string
    pub fn parse(input: &str) -> Result<Self> {
        let parsed: Self = serde_json::from_str(input)?;
        Ok(parsed)
    }
}

/// TTS options for hook handlers
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

/// LLM options for hook handlers
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

/// Process Claude Code hook input
pub async fn process(
    input: &ClaudeCodeInput,
    config: &SumvoxConfig,
    tts_opts: &TtsOptions,
    llm_opts: &LlmOptions,
) -> Result<()> {
    tracing::info!(
        "Processing Claude Code hook: session_id={}, event={}",
        input.session_id,
        input.hook_event_name
    );

    // Prevent infinite loop - if stop_hook is active, exit immediately
    if input.stop_hook_active.unwrap_or(false) {
        tracing::warn!("Stop hook already active, preventing infinite loop");
        return Ok(());
    }

    // Dispatch based on hook event type
    match input.hook_event_name.as_str() {
        "Notification" => {
            handle_notification(input, config, tts_opts).await?;
        }
        "Stop" => {
            handle_stop(input, config, tts_opts, llm_opts).await?;
        }
        _ => {
            tracing::warn!("Unknown hook event: {}", input.hook_event_name);
        }
    }

    Ok(())
}

/// Handle Notification hook - speak notification message directly
async fn handle_notification(
    input: &ClaudeCodeInput,
    config: &SumvoxConfig,
    tts_opts: &TtsOptions,
) -> Result<()> {
    tracing::info!("Processing Notification hook");

    // Get notification message
    let message = match &input.message {
        Some(msg) => msg,
        None => {
            tracing::warn!("Notification hook has no message field");
            return Ok(());
        }
    };

    let notification_type = input.notification_type.as_deref().unwrap_or("unknown");
    tracing::info!(
        "Notification type: {}, message: {}",
        notification_type,
        message
    );

    // Check filter: should we speak this notification type?
    let filter = &config.hooks.claude_code.notification_filter;
    let should_speak = if filter.is_empty() {
        // Empty filter = disabled
        false
    } else if filter.contains(&"*".to_string()) {
        // Wildcard = all notifications
        true
    } else {
        // Check if notification type is in filter
        filter.contains(&notification_type.to_string())
    };

    if !should_speak {
        tracing::debug!(
            "Notification type '{}' not in filter, skipping",
            notification_type
        );
        return Ok(());
    }

    // Speak the notification message directly (no LLM processing)
    tracing::info!("Speaking notification: {}", message);

    // Use configured notification TTS provider if specified
    let mut notification_tts_opts = tts_opts.clone();
    if let Some(ref provider) = config.hooks.claude_code.notification_tts_provider {
        tracing::info!("Using configured notification TTS provider: {}", provider);
        notification_tts_opts.engine = provider.clone();
    }

    speak_text(config, &notification_tts_opts, message).await?;

    Ok(())
}

/// Handle Stop hook - read transcript and generate summary
async fn handle_stop(
    input: &ClaudeCodeInput,
    config: &SumvoxConfig,
    tts_opts: &TtsOptions,
    llm_opts: &LlmOptions,
) -> Result<()> {
    tracing::info!("Processing Stop hook");

    // Read transcript
    let transcript_path = PathBuf::from(&input.transcript_path);
    tracing::debug!("Reading transcript from: {:?}", transcript_path);

    // Initial delay to let filesystem sync (hardcoded 50ms)
    const INITIAL_DELAY_MS: u64 = 50;
    let initial_delay = Duration::from_millis(INITIAL_DELAY_MS);
    tracing::debug!("Waiting {}ms for filesystem sync", INITIAL_DELAY_MS);
    tokio::time::sleep(initial_delay).await;

    let turns = config.summarization.turns.max(1); // At least 1 turn
    let mut texts = TranscriptReader::read_last_n_turns(&transcript_path, turns).await?;

    // Retry once if empty (race condition workaround, hardcoded 100ms)
    if texts.is_empty() {
        const RETRY_DELAY_MS: u64 = 100;
        tracing::debug!("No texts found, retrying after {}ms", RETRY_DELAY_MS);
        let retry_delay = Duration::from_millis(RETRY_DELAY_MS);
        tokio::time::sleep(retry_delay).await;
        texts = TranscriptReader::read_last_n_turns(&transcript_path, turns).await?;
    }

    if texts.is_empty() {
        tracing::warn!("No assistant texts found in transcript after retry");
        return Ok(());
    }

    let context = texts.join("\n\n");
    tracing::debug!(
        "Extracted {} text blocks from last {} turn(s), total length: {}",
        texts.len(),
        turns,
        context.len()
    );

    // Build summarization prompt
    let user_prompt = config
        .summarization
        .prompt_template
        .replace("{context}", &context);

    let system_message = Some(config.summarization.system_message.clone());

    // Generate summary with LLM
    let summary = generate_summary(config, llm_opts, system_message, &user_prompt).await?;

    // Use configured stop TTS provider if specified
    let mut stop_tts_opts = tts_opts.clone();
    if let Some(ref provider) = config.hooks.claude_code.stop_tts_provider {
        tracing::info!("Using configured stop TTS provider: {}", provider);
        stop_tts_opts.engine = provider.clone();
    }

    if summary.is_empty() {
        tracing::warn!("LLM returned empty summary, using fallback");
        let fallback = &config.summarization.fallback_message;
        speak_text(config, &stop_tts_opts, fallback).await?;
    } else {
        tracing::info!("Generated summary: {}", summary);
        speak_text(config, &stop_tts_opts, &summary).await?;
    }

    Ok(())
}

/// Generate summary using LLM
async fn generate_summary(
    config: &SumvoxConfig,
    llm_opts: &LlmOptions,
    system_message: Option<String>,
    prompt: &str,
) -> Result<String> {
    let llm_config = &config.llm;

    let request = GenerationRequest {
        system_message,
        prompt: prompt.to_string(),
        max_tokens: llm_config.parameters.max_tokens,
        temperature: llm_config.parameters.temperature,
        disable_thinking: llm_config.parameters.disable_thinking,
    };

    // Try providers with fallback
    if llm_opts.provider.is_some() || llm_opts.model.is_some() {
        // CLI specified - try only that provider
        let provider_name = llm_opts.provider.as_deref().unwrap_or("google");
        let model_name = llm_opts.model.as_deref().unwrap_or("gemini-2.5-flash");
        let timeout = Duration::from_secs(llm_opts.timeout);

        // Try to get API key from config or env
        let api_key = config
            .llm
            .providers
            .iter()
            .find(|p| p.name.to_lowercase() == provider_name.to_lowercase())
            .and_then(|p| p.get_api_key());

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

    // Try each provider in config order until one succeeds
    for provider_config in &llm_config.providers {
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
async fn speak_text(config: &SumvoxConfig, tts_opts: &TtsOptions, text: &str) -> Result<()> {
    let tts_engine = tts_opts.engine.parse().unwrap_or(TtsEngine::Auto);

    // Create TTS provider: CLI override or config fallback chain
    let provider: Box<dyn TtsProvider> = match tts_engine {
        TtsEngine::Auto => {
            // Use config fallback chain
            create_tts_from_config(&config.tts.providers, true)?
        }
        TtsEngine::MacOS => {
            // Get macOS config for fallback values
            let macos_config = config
                .tts
                .providers
                .iter()
                .find(|p| p.name.to_lowercase() == "macos");

            // Priority: CLI > Config > Default
            let voice = tts_opts
                .voice
                .clone()
                .or_else(|| macos_config.and_then(|p| p.voice.clone()));

            let volume = tts_opts
                .volume
                .or_else(|| macos_config.and_then(|p| p.volume))
                .unwrap_or(100);

            create_tts_by_name("macos", None, voice, tts_opts.rate, volume, true, None)?
        }
        TtsEngine::Google => {
            // Get Google config for fallback values
            let google_config = config
                .tts
                .providers
                .iter()
                .find(|p| p.name.to_lowercase() == "google");

            // Get API key from config or env
            let api_key = google_config.and_then(|p| p.get_api_key());

            // Model is required for Google TTS
            let model = google_config
                .and_then(|p| p.model.clone())
                .or_else(|| Some("gemini-2.5-flash-preview-tts".to_string()));

            // Priority: CLI > Config > Default
            let voice = tts_opts
                .voice
                .clone()
                .or_else(|| google_config.and_then(|p| p.voice.clone()))
                .unwrap_or_else(|| "Zephyr".to_string());

            let volume = tts_opts
                .volume
                .or_else(|| google_config.and_then(|p| p.volume))
                .unwrap_or(100);

            create_tts_by_name(
                "google",
                model,
                Some(voice),
                tts_opts.rate,
                volume,
                true,
                api_key,
            )?
        }
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
            speak_with_provider_fallback(&config.tts.providers, text).await
        }
        _ => {
            // Single provider mode - just try once
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
async fn speak_with_provider_fallback(
    providers: &[crate::config::TtsProviderConfig],
    text: &str,
) -> Result<()> {
    let mut last_error = None;

    for provider_config in providers {
        // Try to create provider
        let provider = match crate::tts::create_single_tts(provider_config, true) {
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

        // Try to speak
        match provider.speak(text).await {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_claude_code_input_deserialization() {
        let json = r#"{
            "session_id": "test-session",
            "transcript_path": "/path/to/transcript.jsonl",
            "permission_mode": "auto",
            "hook_event_name": "Stop",
            "stop_hook_active": false
        }"#;

        let input = ClaudeCodeInput::parse(json).unwrap();
        assert_eq!(input.session_id, "test-session");
        assert_eq!(input.hook_event_name, "Stop");
        assert_eq!(input.stop_hook_active, Some(false));
    }

    #[test]
    fn test_claude_code_input_notification() {
        let json = r#"{
            "session_id": "test-session",
            "transcript_path": "/path/to/transcript.jsonl",
            "hook_event_name": "Notification",
            "message": "Hello notification",
            "notification_type": "permission_prompt"
        }"#;

        let input = ClaudeCodeInput::parse(json).unwrap();
        assert_eq!(input.hook_event_name, "Notification");
        assert_eq!(input.message, Some("Hello notification".to_string()));
        assert_eq!(
            input.notification_type,
            Some("permission_prompt".to_string())
        );
    }

    #[test]
    fn test_tts_options_default() {
        let opts = TtsOptions::default();
        assert_eq!(opts.engine, "auto");
        assert_eq!(opts.rate, 200);
        assert!(opts.voice.is_none());
        assert!(opts.volume.is_none());
    }

    #[test]
    fn test_llm_options_default() {
        let opts = LlmOptions::default();
        assert!(opts.provider.is_none());
        assert!(opts.model.is_none());
        assert_eq!(opts.timeout, 10);
    }
}
