// claude-voice: Voice notification hook for Claude Code
// Rust rewrite - single binary, zero dependencies deployment

mod config;
mod error;
mod llm;
mod transcript;
mod voice;

use std::io::Read;
use std::path::PathBuf;
use std::time::Duration;

use config::VoiceConfig;
use error::Result;
use llm::{CostTracker, GeminiProvider, GenerationRequest, LlmProvider, OllamaProvider};
use transcript::TranscriptReader;
use voice::VoiceEngine;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct HookInput {
    session_id: String,
    transcript_path: String,
    permission_mode: String,
    hook_event_name: String,
    stop_hook_active: Option<bool>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tracing::info!("claude-voice starting");

    // Read JSON input from stdin
    let mut input_buffer = String::new();
    std::io::stdin()
        .read_to_string(&mut input_buffer)
        .map_err(|e| error::VoiceError::Io(e))?;

    let hook_input: HookInput = serde_json::from_str(&input_buffer)?;

    tracing::info!(
        "Processing hook: session_id={}, event={}",
        hook_input.session_id,
        hook_input.hook_event_name
    );

    // Prevent infinite loop - if stop_hook is active, exit immediately
    if hook_input.stop_hook_active.unwrap_or(false) {
        tracing::warn!("Stop hook already active, preventing infinite loop");
        return Ok(());
    }

    // Load configuration
    let config_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".claude")
        .join("hooks")
        .join("voice_config.json");

    let mut config = VoiceConfig::load(config_path)?;
    config.expand_env_vars();

    if !config.enabled {
        tracing::info!("Voice notifications disabled in config");
        return Ok(());
    }

    // Read transcript
    let transcript_path = PathBuf::from(&hook_input.transcript_path);
    tracing::debug!("Reading transcript from: {:?}", transcript_path);

    let texts = TranscriptReader::read_last_n_texts(&transcript_path, 3).await?;

    if texts.is_empty() {
        tracing::warn!("No assistant texts found in transcript");
        return Ok(());
    }

    let context = texts.join("\n\n");
    tracing::debug!("Extracted {} text blocks, total length: {}", texts.len(), context.len());

    // Build summarization prompt
    let max_length = config.voice.max_summary_length;
    let prompt = config
        .summarization
        .prompt_template
        .replace("{max_length}", &max_length.to_string())
        .replace("{context}", &context);

    // Generate summary with LLM
    let summary = generate_summary(&config, &prompt).await?;

    if summary.is_empty() {
        tracing::warn!("LLM returned empty summary, using fallback");
        let fallback = &config.advanced.fallback_message;
        speak_summary(&config, fallback).await?;
    } else {
        tracing::info!("Generated summary: {}", summary);
        speak_summary(&config, &summary).await?;
    }

    tracing::info!("claude-voice completed successfully");
    Ok(())
}

async fn generate_summary(config: &VoiceConfig, prompt: &str) -> Result<String> {
    let llm_config = &config.llm;

    // Initialize cost tracker
    let cost_tracker = if llm_config.cost_control.usage_tracking {
        Some(CostTracker::new(&llm_config.cost_control.usage_file))
    } else {
        None
    };

    // Check budget
    if let Some(ref tracker) = cost_tracker {
        let daily_limit = llm_config.cost_control.daily_limit_usd;
        let under_budget = tracker.check_budget(daily_limit).await?;

        if !under_budget {
            tracing::warn!("Daily budget limit ${} exceeded", daily_limit);
            return Ok(String::new());
        }
    }

    // Try primary provider (Gemini)
    let api_key = llm_config
        .api_keys
        .get("gemini")
        .cloned()
        .unwrap_or_default();

    let provider = GeminiProvider::new(
        api_key,
        llm_config.models.primary.clone(),
        Duration::from_secs(llm_config.parameters.timeout),
    );

    if !provider.is_available() {
        tracing::warn!("Primary provider (Gemini) not available");
        return Ok(String::new());
    }

    let request = GenerationRequest {
        prompt: prompt.to_string(),
        max_tokens: llm_config.parameters.max_tokens,
        temperature: llm_config.parameters.temperature,
    };

    match provider.generate(&request).await {
        Ok(response) => {
            // Record usage
            if let Some(ref tracker) = cost_tracker {
                let cost = provider.estimate_cost(response.input_tokens, response.output_tokens);
                tracker
                    .record_usage(
                        &response.model,
                        response.input_tokens,
                        response.output_tokens,
                        cost,
                    )
                    .await?;
            }

            Ok(response.text.trim().to_string())
        }
        Err(e) => {
            tracing::warn!("Primary provider failed: {}, trying Ollama fallback", e);

            // Try Ollama as fallback
            let fallback_model = llm_config
                .models
                .fallback
                .as_ref()
                .map(String::as_str)
                .unwrap_or("llama3.1");

            let ollama = OllamaProvider::new(
                fallback_model.to_string(),
                Duration::from_secs(llm_config.parameters.timeout),
            );

            match ollama.generate(&request).await {
                Ok(response) => {
                    tracing::info!("Ollama fallback succeeded");
                    Ok(response.text.trim().to_string())
                }
                Err(e) => {
                    tracing::error!("Ollama fallback also failed: {}", e);
                    Ok(String::new())
                }
            }
        }
    }
}

async fn speak_summary(config: &VoiceConfig, summary: &str) -> Result<()> {
    let voice_engine = VoiceEngine::new(config.voice.clone());

    let is_async = config.voice.async_mode;
    voice_engine.speak(summary, Some(!is_async)).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hook_input_deserialization() {
        let json = r#"{
            "session_id": "test-session",
            "transcript_path": "/path/to/transcript.jsonl",
            "permission_mode": "auto",
            "hook_event_name": "stop",
            "stop_hook_active": false
        }"#;

        let input: HookInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.session_id, "test-session");
        assert_eq!(input.hook_event_name, "stop");
        assert_eq!(input.stop_hook_active, Some(false));
    }

    #[test]
    fn test_hook_input_without_stop_hook_active() {
        let json = r#"{
            "session_id": "test-session",
            "transcript_path": "/path/to/transcript.jsonl",
            "permission_mode": "auto",
            "hook_event_name": "stop"
        }"#;

        let input: HookInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.stop_hook_active, None);
    }
}
