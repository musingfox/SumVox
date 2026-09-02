// Claude Code hook handler
// Processes JSON input from Claude Code Stop and Notification hooks

use std::path::PathBuf;
use std::time::Duration;

use serde::Deserialize;

use crate::config::SumvoxConfig;
use crate::error::Result;
use crate::pipeline::{generate_summary, speak_text, LlmOptions, TtsOptions};
use crate::queue::{NotificationQueue, QueueLock};
use crate::transcript::TranscriptReader;

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
    // Stop hook content source alternative
    pub last_assistant_message: Option<String>,
}

impl ClaudeCodeInput {
    /// Parse from JSON string
    pub fn parse(input: &str) -> Result<Self> {
        let parsed: Self = serde_json::from_str(input)?;
        Ok(parsed)
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

/// Acquire notification queue lock if queuing is enabled
async fn acquire_queue_lock(config: &SumvoxConfig) -> Result<Option<QueueLock>> {
    let timeout_secs = config.hooks.claude_code.queue_timeout.unwrap_or(30);
    if timeout_secs == 0 {
        tracing::debug!("Notification queue disabled (timeout=0)");
        return Ok(None);
    }

    let timeout = Duration::from_secs(timeout_secs);
    let queue = NotificationQueue::new(Some(timeout))?;
    match QueueLock::acquire(&queue).await {
        Ok(lock) => Ok(Some(lock)),
        Err(e) => {
            tracing::warn!(
                "Failed to acquire queue lock, proceeding without lock: {}",
                e
            );
            Ok(None)
        }
    }
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

    // Acquire queue lock for cross-process coordination
    let _lock = acquire_queue_lock(config).await?;

    // Speak the notification message directly (no LLM processing)
    tracing::info!("Speaking notification: {}", message);

    // Use configured notification TTS provider if specified
    let mut notification_tts_opts = tts_opts.clone();
    if let Some(ref provider) = config.hooks.claude_code.notification_tts_provider {
        tracing::info!("Using configured notification TTS provider: {}", provider);
        notification_tts_opts.engine = provider.clone();
    }

    // Set notification-specific volume (priority: CLI > hook config > default)
    if notification_tts_opts.volume.is_none() {
        notification_tts_opts.volume = Some(
            config.hooks.claude_code.notification_volume.unwrap_or(80), // Default notification volume
        );
    }

    speak_text(config, &notification_tts_opts, message).await?;

    // Lock released on drop
    Ok(())
}

/// Content source selection result for Stop hook
enum StopContextSource {
    UseLastMessage(String),
    ReadTranscript,
}

/// Select content source based on config and available input
fn select_stop_context_source(
    content_source: crate::config::ContentSource,
    last_message: Option<&str>,
) -> StopContextSource {
    use crate::config::ContentSource;

    match content_source {
        ContentSource::Transcript => StopContextSource::ReadTranscript,
        ContentSource::LastMessage => {
            if let Some(text) = last_message {
                if !text.trim().is_empty() {
                    return StopContextSource::UseLastMessage(text.to_string());
                }
            }
            StopContextSource::ReadTranscript
        }
    }
}

/// Handle Stop hook - read transcript and generate summary
async fn handle_stop(
    input: &ClaudeCodeInput,
    config: &SumvoxConfig,
    tts_opts: &TtsOptions,
    llm_opts: &LlmOptions,
) -> Result<()> {
    tracing::info!("Processing Stop hook");

    // Determine content source
    let source = select_stop_context_source(
        config.summarization.content_source,
        input.last_assistant_message.as_deref(),
    );

    let context = match source {
        StopContextSource::UseLastMessage(text) => {
            tracing::info!("Using last_assistant_message as content source");
            text
        }
        StopContextSource::ReadTranscript => {
            // Emit warning if user configured LastMessage but it wasn't available
            if matches!(
                config.summarization.content_source,
                crate::config::ContentSource::LastMessage
            ) {
                tracing::warn!(
                    "content_source=last_message but last_assistant_message is empty/missing; falling back to transcript"
                );
            } else {
                tracing::info!("Using transcript as content source");
            }

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

            let joined = texts.join("\n\n");
            tracing::debug!(
                "Extracted {} text blocks from last {} turn(s), total length: {}",
                texts.len(),
                turns,
                joined.len()
            );
            joined
        }
    };

    // Build summarization prompt
    let user_prompt = config
        .summarization
        .prompt_template
        .replace("{context}", &context);

    let system_message = Some(config.summarization.system_message.clone());

    // Generate summary with LLM
    let summary = generate_summary(config, llm_opts, system_message, &user_prompt).await?;

    // Acquire queue lock before speaking
    let _lock = acquire_queue_lock(config).await?;

    // Use configured stop TTS provider if specified
    let mut stop_tts_opts = tts_opts.clone();
    if let Some(ref provider) = config.hooks.claude_code.stop_tts_provider {
        tracing::info!("Using configured stop TTS provider: {}", provider);
        stop_tts_opts.engine = provider.clone();
    }

    // Set stop hook specific volume (priority: CLI > hook config > default)
    if stop_tts_opts.volume.is_none() {
        stop_tts_opts.volume = Some(config.hooks.claude_code.stop_volume.unwrap_or(100));
        // Default stop/summary volume
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tts::TtsEngine;

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

    // ── Volume override tests ──────────────────────────────────────────

    #[test]
    fn test_stop_volume_default_when_config_unset() {
        let config = SumvoxConfig::default();
        let tts_opts = TtsOptions::default();

        let mut stop_tts_opts = tts_opts.clone();
        if stop_tts_opts.volume.is_none() {
            stop_tts_opts.volume = Some(config.hooks.claude_code.stop_volume.unwrap_or(100));
        }

        assert_eq!(stop_tts_opts.volume, Some(100));
    }

    #[test]
    fn test_stop_volume_from_hook_config() {
        let mut config = SumvoxConfig::default();
        config.hooks.claude_code.stop_volume = Some(80);
        let tts_opts = TtsOptions::default();

        let mut stop_tts_opts = tts_opts.clone();
        if stop_tts_opts.volume.is_none() {
            stop_tts_opts.volume = Some(config.hooks.claude_code.stop_volume.unwrap_or(100));
        }

        assert_eq!(stop_tts_opts.volume, Some(80));
    }

    #[test]
    fn test_notification_volume_from_hook_config() {
        let mut config = SumvoxConfig::default();
        config.hooks.claude_code.notification_volume = Some(60);
        let tts_opts = TtsOptions::default();

        let mut notification_tts_opts = tts_opts.clone();
        if notification_tts_opts.volume.is_none() {
            notification_tts_opts.volume =
                Some(config.hooks.claude_code.notification_volume.unwrap_or(80));
        }

        assert_eq!(notification_tts_opts.volume, Some(60));
    }

    #[test]
    fn test_cli_volume_overrides_hook_config() {
        let mut config = SumvoxConfig::default();
        config.hooks.claude_code.stop_volume = Some(80);

        let tts_opts = TtsOptions {
            volume: Some(50), // CLI override
            ..Default::default()
        };

        let mut stop_tts_opts = tts_opts.clone();
        if stop_tts_opts.volume.is_none() {
            stop_tts_opts.volume = Some(config.hooks.claude_code.stop_volume.unwrap_or(100));
        }

        // CLI volume (50) takes priority over hook config (80)
        assert_eq!(stop_tts_opts.volume, Some(50));
    }

    #[test]
    fn test_volume_override_applies_to_provider_config() {
        use crate::config::TtsProviderConfig;

        let provider = TtsProviderConfig {
            name: "google".to_string(),
            model: Some("gemini-2.5-flash-preview-tts".to_string()),
            voice: None,
            api_key: None,
            rate: None,
            volume: Some(100), // Provider default
            path: None,
            service_account_key: None,
            language_code: None,
            speed: None,
            stability: None,
            style: None,
            style_prompt: None,
        };

        let volume_override = Some(60u32);
        let mut config_with_volume = provider.clone();
        if let Some(vol) = volume_override {
            config_with_volume.volume = Some(vol);
        }

        // Hook-level volume (60) overrides provider-level (100)
        assert_eq!(config_with_volume.volume, Some(60));
    }

    #[test]
    fn test_volume_override_none_preserves_provider_volume() {
        use crate::config::TtsProviderConfig;

        let provider = TtsProviderConfig {
            name: "google".to_string(),
            model: Some("gemini-2.5-flash-preview-tts".to_string()),
            voice: None,
            api_key: None,
            rate: None,
            volume: Some(100),
            path: None,
            service_account_key: None,
            language_code: None,
            speed: None,
            stability: None,
            style: None,
            style_prompt: None,
        };

        let volume_override: Option<u32> = None;
        let mut config_with_volume = provider.clone();
        if let Some(vol) = volume_override {
            config_with_volume.volume = Some(vol);
        }

        // No override → provider volume preserved
        assert_eq!(config_with_volume.volume, Some(100));
    }

    #[test]
    fn test_volume_override_applies_to_provider_without_volume() {
        use crate::config::TtsProviderConfig;

        let provider = TtsProviderConfig {
            name: "google".to_string(),
            model: Some("gemini-2.5-flash-preview-tts".to_string()),
            voice: None,
            api_key: None,
            rate: None,
            volume: None, // No provider volume set
            path: None,
            service_account_key: None,
            language_code: None,
            speed: None,
            stability: None,
            style: None,
            style_prompt: None,
        };

        let volume_override = Some(80u32);
        let mut config_with_volume = provider.clone();
        if let Some(vol) = volume_override {
            config_with_volume.volume = Some(vol);
        }

        // Hook-level volume applies even when provider has no volume
        assert_eq!(config_with_volume.volume, Some(80));
    }

    #[test]
    fn test_auto_engine_propagates_volume_to_tts_opts() {
        // Simulate the full flow: config → TtsOptions → speak_text
        let mut config = SumvoxConfig::default();
        config.hooks.claude_code.stop_volume = Some(70);
        config.hooks.claude_code.stop_tts_provider = Some("auto".to_string());

        let tts_opts = TtsOptions::default();

        // Replicate handle_stop logic
        let mut stop_tts_opts = tts_opts.clone();
        if let Some(ref provider) = config.hooks.claude_code.stop_tts_provider {
            stop_tts_opts.engine = provider.clone();
        }
        if stop_tts_opts.volume.is_none() {
            stop_tts_opts.volume = Some(config.hooks.claude_code.stop_volume.unwrap_or(100));
        }

        assert_eq!(stop_tts_opts.engine, "auto");
        assert_eq!(stop_tts_opts.volume, Some(70));

        // In speak_text, Auto mode passes tts_opts.volume to speak_with_provider_fallback
        // which applies it as volume_override to each provider config
        let tts_engine: TtsEngine = stop_tts_opts.engine.parse().unwrap();
        assert_eq!(tts_engine, TtsEngine::Auto);
        assert_eq!(stop_tts_opts.volume, Some(70));
    }

    // ── Contract 1: last_assistant_message deserialization ──────────────

    #[test]
    fn test_last_assistant_message_absent() {
        let json = r#"{
            "session_id": "s1",
            "transcript_path": "/tmp/t.jsonl",
            "hook_event_name": "Stop"
        }"#;
        let input = ClaudeCodeInput::parse(json).unwrap();
        assert_eq!(input.last_assistant_message, None);
    }

    #[test]
    fn test_last_assistant_message_present() {
        let json = r#"{
            "session_id": "s1",
            "transcript_path": "/tmp/t.jsonl",
            "hook_event_name": "Stop",
            "last_assistant_message": "Done"
        }"#;
        let input = ClaudeCodeInput::parse(json).unwrap();
        assert_eq!(input.last_assistant_message, Some("Done".to_string()));
    }

    #[test]
    fn test_last_assistant_message_empty() {
        let json = r#"{
            "session_id": "s1",
            "transcript_path": "/tmp/t.jsonl",
            "hook_event_name": "Stop",
            "last_assistant_message": ""
        }"#;
        let input = ClaudeCodeInput::parse(json).unwrap();
        assert_eq!(input.last_assistant_message, Some("".to_string()));
    }

    // ── Contract 3: select_stop_context_source logic ────────────────────

    #[test]
    fn test_select_source_transcript_none() {
        use crate::config::ContentSource;
        let source = select_stop_context_source(ContentSource::Transcript, None);
        assert!(matches!(source, StopContextSource::ReadTranscript));
    }

    #[test]
    fn test_select_source_transcript_some() {
        use crate::config::ContentSource;
        let source = select_stop_context_source(ContentSource::Transcript, Some("anything"));
        assert!(matches!(source, StopContextSource::ReadTranscript));
    }

    #[test]
    fn test_select_source_last_message_present() {
        use crate::config::ContentSource;
        let source = select_stop_context_source(ContentSource::LastMessage, Some("hello"));
        match source {
            StopContextSource::UseLastMessage(text) => assert_eq!(text, "hello"),
            _ => panic!("Expected UseLastMessage"),
        }
    }

    #[test]
    fn test_select_source_last_message_whitespace() {
        use crate::config::ContentSource;
        let source = select_stop_context_source(ContentSource::LastMessage, Some("  "));
        assert!(matches!(source, StopContextSource::ReadTranscript));
    }

    #[test]
    fn test_select_source_last_message_empty() {
        use crate::config::ContentSource;
        let source = select_stop_context_source(ContentSource::LastMessage, Some(""));
        assert!(matches!(source, StopContextSource::ReadTranscript));
    }

    #[test]
    fn test_select_source_last_message_none() {
        use crate::config::ContentSource;
        let source = select_stop_context_source(ContentSource::LastMessage, None);
        assert!(matches!(source, StopContextSource::ReadTranscript));
    }
}
