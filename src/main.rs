// claude-voice: Voice notification hook for Claude Code
// Rust rewrite - single binary, zero dependencies deployment

mod cli;
mod config;
mod credentials;
mod error;
mod llm;
mod provider_factory;
mod transcript;
mod voice;

use std::io::Read;
use std::path::PathBuf;
use std::time::Duration;

use clap::Parser;
use cli::{Cli, Commands, CredentialAction};
use config::VoiceConfig;
use credentials::CredentialManager;
use error::Result;
use llm::{CostTracker, GenerationRequest, LlmProvider};
use provider_factory::ProviderFactory;
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
    // Parse CLI arguments
    let cli = Cli::parse();

    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // Handle credentials subcommand
    if let Some(Commands::Credentials { action }) = cli.command {
        return handle_credentials_command(action).await;
    }

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
    let max_length = cli.max_length;
    let prompt = config
        .summarization
        .prompt_template
        .replace("{max_length}", &max_length.to_string())
        .replace("{context}", &context);

    // Generate summary with LLM
    let summary = generate_summary(&config, &cli, &prompt).await?;

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

async fn generate_summary(config: &VoiceConfig, cli: &Cli, prompt: &str) -> Result<String> {
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

    // Determine provider and model
    let provider_name = cli
        .provider
        .as_deref()
        .or_else(|| llm_config.models.primary.split(':').next())
        .unwrap_or("google");

    let model_name = cli
        .model
        .as_deref()
        .unwrap_or(&llm_config.models.primary);

    let timeout = Duration::from_secs(cli.timeout);

    // Create provider using factory
    let credential_manager = CredentialManager::new();
    let provider = match ProviderFactory::create(
        provider_name,
        model_name,
        timeout,
        &credential_manager,
    ) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("Failed to create provider {}: {}", provider_name, e);

            // Try fallback to Ollama
            if let Some(fallback_model) = &llm_config.models.fallback {
                tracing::info!("Trying Ollama fallback with model: {}", fallback_model);
                match ProviderFactory::create("ollama", fallback_model, timeout, &credential_manager) {
                    Ok(p) => p,
                    Err(e) => {
                        tracing::error!("Ollama fallback also failed: {}", e);
                        return Ok(String::new());
                    }
                }
            } else {
                return Ok(String::new());
            }
        }
    };

    if !provider.is_available() {
        tracing::warn!("Provider {} not available", provider.name());
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
            tracing::error!("Provider {} failed: {}", provider.name(), e);
            Ok(String::new())
        }
    }
}

async fn speak_summary(config: &VoiceConfig, summary: &str) -> Result<()> {
    let voice_engine = VoiceEngine::new(config.voice.clone());

    let is_async = config.voice.async_mode;
    voice_engine.speak(summary, Some(!is_async)).await?;

    Ok(())
}

async fn handle_credentials_command(action: CredentialAction) -> Result<()> {
    use error::VoiceError;

    let manager = CredentialManager::new();

    match action {
        CredentialAction::Set { provider } => {
            // Interactive API key input (hidden)
            eprint!("Enter API key for {}: ", provider);
            let api_key = rpassword::read_password()
                .map_err(|e| VoiceError::Config(format!("Failed to read password: {}", e)))?;

            if api_key.trim().is_empty() {
                return Err(VoiceError::Config("API key cannot be empty".into()));
            }

            manager.save_api_key(&provider, api_key.trim())?;
            eprintln!("✓ API key saved for {}", provider);
        }
        CredentialAction::List => {
            let providers = manager.list_providers();
            if providers.is_empty() {
                eprintln!("No credentials configured.");
                eprintln!();
                eprintln!("To set an API key, run:");
                eprintln!("  claude-voice credentials set <provider>");
                eprintln!();
                eprintln!("Available providers: google, anthropic, openai");
            } else {
                eprintln!("Configured providers:");
                for p in providers {
                    eprintln!("  - {}", p);
                }
            }
        }
        CredentialAction::Test { provider } => {
            // Test if API key is valid by checking if it can be loaded
            match manager.load_api_key(&provider) {
                Some(key) => {
                    if key.is_empty() {
                        eprintln!("✗ API key for {} is empty", provider);
                    } else {
                        eprintln!("✓ API key found for {}", provider);
                        eprintln!("  Key: {}...{}", &key[..4.min(key.len())],
                                  if key.len() > 8 { &key[key.len()-4..] } else { "" });
                    }
                }
                None => {
                    eprintln!("✗ No API key found for {}", provider);
                    eprintln!();
                    eprintln!("To set an API key, run:");
                    eprintln!("  claude-voice credentials set {}", provider);
                }
            }
        }
        CredentialAction::Remove { provider } => {
            manager.remove_provider(&provider)?;
            eprintln!("✓ Credentials removed for {}", provider);
        }
    }

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
