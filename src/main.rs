// claude-voice: Voice notification hook for Claude Code
// Rust rewrite - single binary, zero dependencies deployment

mod cli;
mod config;
mod credentials;
mod error;
mod llm;
mod provider_factory;
mod transcript;
mod tts;
mod voice;

use std::io::Read;
use std::path::PathBuf;
use std::time::Duration;

use clap::Parser;
use cli::{Cli, Commands, CredentialAction};
use config::VoiceConfig;
use error::Result;
use llm::{CostTracker, GenerationRequest};
use provider_factory::ProviderFactory;
use transcript::TranscriptReader;
use tts::{create_tts_by_name, create_tts_from_config, TtsEngine, TtsProvider};

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct HookInput {
    session_id: String,
    transcript_path: String,
    #[allow(dead_code)]
    permission_mode: String,
    hook_event_name: String,
    stop_hook_active: Option<bool>,
    // Notification hook specific fields
    message: Option<String>,
    notification_type: Option<String>,
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

    // Handle subcommands
    match &cli.command {
        Some(Commands::Credentials { action }) => {
            return handle_credentials_command(action.clone()).await;
        }
        Some(Commands::Init) => {
            return handle_init_command().await;
        }
        None => {}
    }

    tracing::info!("claude-voice starting");

    // Read JSON input from stdin
    let mut input_buffer = String::new();
    std::io::stdin()
        .read_to_string(&mut input_buffer)
        .map_err(error::VoiceError::Io)?;

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

    // Load configuration from ~/.claude/claude-voice.json
    let config = VoiceConfig::load_from_home()?;

    if !config.enabled {
        tracing::info!("Voice notifications disabled in config");
        return Ok(());
    }

    // Dispatch based on hook event type
    match hook_input.hook_event_name.as_str() {
        "Notification" => {
            handle_notification_hook(&hook_input, &config, &cli).await?;
        }
        "Stop" => {
            handle_stop_hook(&hook_input, &config, &cli).await?;
        }
        _ => {
            tracing::warn!("Unknown hook event: {}", hook_input.hook_event_name);
        }
    }

    tracing::info!("claude-voice completed successfully");
    Ok(())
}

/// Handle Notification hook - speak notification message directly
async fn handle_notification_hook(
    hook_input: &HookInput,
    config: &VoiceConfig,
    cli: &Cli,
) -> Result<()> {
    tracing::info!("Processing Notification hook");

    // Get notification message
    let message = match &hook_input.message {
        Some(msg) => msg,
        None => {
            tracing::warn!("Notification hook has no message field");
            return Ok(());
        }
    };

    let notification_type = hook_input.notification_type.as_deref().unwrap_or("unknown");
    tracing::info!(
        "Notification type: {}, message: {}",
        notification_type,
        message
    );

    // Filter: only speak important notifications
    // - permission_prompt: User needs to approve an action (HIGH priority)
    // - idle_prompt: Claude waiting 60+ seconds for response (HIGH priority)
    // - elicitation_dialog: MCP tool needs user input (HIGH priority)
    // - auth_success: Authentication completed (LOW priority, skipped)
    let should_speak = matches!(
        notification_type,
        "permission_prompt" | "idle_prompt" | "elicitation_dialog"
    );

    if !should_speak {
        tracing::debug!("Skipping notification type: {}", notification_type);
        return Ok(());
    }

    // Process notification message with LLM
    let user_prompt = config
        .summarization
        .notification_prompt
        .replace("{message}", message);

    let system_message = Some(config.summarization.notification_system_message.clone());
    let processed_message = generate_summary(config, cli, system_message, &user_prompt).await?;

    // Use original message as fallback if LLM processing fails
    let final_message = if processed_message.is_empty() {
        message.clone()
    } else {
        processed_message
    };

    speak_summary(cli, config, &final_message).await?;

    Ok(())
}

/// Handle Stop hook - read transcript and generate summary
async fn handle_stop_hook(hook_input: &HookInput, config: &VoiceConfig, cli: &Cli) -> Result<()> {
    tracing::info!("Processing Stop hook");

    // Read transcript
    let transcript_path = PathBuf::from(&hook_input.transcript_path);
    tracing::debug!("Reading transcript from: {:?}", transcript_path);

    let texts = TranscriptReader::read_last_n_texts(&transcript_path, 10).await?;

    if texts.is_empty() {
        tracing::warn!("No assistant texts found in transcript");
        return Ok(());
    }

    let context = texts.join("\n\n");
    tracing::debug!(
        "Extracted {} text blocks, total length: {}",
        texts.len(),
        context.len()
    );

    // Build summarization prompt
    let max_length = cli.max_length;
    let user_prompt = config
        .summarization
        .prompt_template
        .replace("{max_length}", &max_length.to_string())
        .replace("{context}", &context);

    let system_message = Some(config.summarization.system_message.clone());

    // Generate summary with LLM
    let summary = generate_summary(config, cli, system_message, &user_prompt).await?;

    if summary.is_empty() {
        tracing::warn!("LLM returned empty summary, using fallback");
        let fallback = &config.advanced.fallback_message;
        speak_summary(cli, config, fallback).await?;
    } else {
        tracing::info!("Generated summary: {}", summary);
        speak_summary(cli, config, &summary).await?;
    }

    Ok(())
}

async fn generate_summary(
    config: &VoiceConfig,
    cli: &Cli,
    system_message: Option<String>,
    prompt: &str,
) -> Result<String> {
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

    // Create provider: CLI override or config fallback chain
    let provider = if cli.provider.is_some() || cli.model.is_some() {
        // CLI specified - create specific provider
        let provider_name = cli.provider.as_deref().unwrap_or("google");
        let model_name = cli.model.as_deref().unwrap_or("gemini-2.5-flash");
        let timeout = Duration::from_secs(cli.timeout);

        // Try to get API key from config or env
        let api_key = config
            .llm
            .providers
            .iter()
            .find(|p| p.name.to_lowercase() == provider_name.to_lowercase())
            .and_then(|p| p.get_api_key());

        ProviderFactory::create_by_name(provider_name, model_name, timeout, api_key.as_deref())?
    } else {
        // Use config fallback chain
        ProviderFactory::create_from_config(&llm_config.providers)?
    };

    if !provider.is_available() {
        tracing::warn!("Provider {} not available", provider.name());
        return Ok(String::new());
    }

    let request = GenerationRequest {
        system_message,
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

async fn speak_summary(cli: &Cli, config: &VoiceConfig, summary: &str) -> Result<()> {
    let tts_engine = TtsEngine::from_str(&cli.tts).unwrap_or(TtsEngine::Auto);

    // Create TTS provider: CLI override or config fallback chain
    let provider: Box<dyn TtsProvider> = match tts_engine {
        TtsEngine::Auto => {
            // Use config fallback chain
            create_tts_from_config(&config.tts.providers, true)?
        }
        TtsEngine::MacOS => {
            let voice = cli
                .tts_voice
                .clone()
                .unwrap_or_else(|| "Ting-Ting".to_string());
            create_tts_by_name("macos", Some(voice), cli.rate, true, None)?
        }
        TtsEngine::Google => {
            // Get API key from config or env
            let api_key = config
                .tts
                .providers
                .iter()
                .find(|p| p.name.to_lowercase() == "google")
                .and_then(|p| p.get_api_key());

            let voice = cli
                .tts_voice
                .clone()
                .unwrap_or_else(|| "Aoede".to_string());

            create_tts_by_name("google", Some(voice), cli.rate, true, api_key)?
        }
    };

    if !provider.is_available() {
        tracing::warn!("TTS provider {} not available", provider.name());
        return Ok(());
    }

    // Estimate and log cost for cloud providers
    let cost = provider.estimate_cost(summary.len());
    if cost > 0.0 {
        tracing::info!(
            "TTS cost estimate: ${:.6} for {} chars",
            cost,
            summary.len()
        );
    }

    // Speak with error handling (TTS failures should be silent)
    match provider.speak(summary).await {
        Ok(_) => tracing::debug!("TTS playback completed"),
        Err(e) => {
            tracing::warn!("TTS playback failed: {}. Notification will be silent.", e);
        }
    }

    Ok(())
}

async fn handle_credentials_command(action: CredentialAction) -> Result<()> {
    use error::VoiceError;

    match action {
        CredentialAction::Set { provider } => {
            // Interactive API key input (hidden)
            eprint!("Enter API key for {}: ", provider);
            let api_key = rpassword::read_password()
                .map_err(|e| VoiceError::Config(format!("Failed to read password: {}", e)))?;

            if api_key.trim().is_empty() {
                return Err(VoiceError::Config("API key cannot be empty".into()));
            }

            // Load config, update, and save
            let mut config = VoiceConfig::load_from_home()?;

            if provider.to_lowercase() == "google_tts" || provider.to_lowercase() == "gemini_tts" {
                // For TTS, set Gemini API key
                config.set_tts_api_key(api_key.trim());
            } else {
                config.set_llm_api_key(&provider, api_key.trim());
            }

            config.save_to_home()?;
            eprintln!("API key saved for {} in ~/.claude/claude-voice.json", provider);
        }
        CredentialAction::List => {
            let config = VoiceConfig::load_from_home()?;

            eprintln!("LLM Providers:");
            for (name, available) in config.list_llm_providers() {
                let status = if available { "configured" } else { "no key" };
                eprintln!("  - {} ({})", name, status);
            }

            eprintln!();
            eprintln!("TTS Providers:");
            for (name, available) in config.list_tts_providers() {
                let status = if available { "configured" } else { "not configured" };
                eprintln!("  - {} ({})", name, status);
            }
        }
        CredentialAction::Test { provider } => {
            let config = VoiceConfig::load_from_home()?;

            // Check LLM providers
            let llm_found = config
                .llm
                .providers
                .iter()
                .find(|p| p.name.to_lowercase() == provider.to_lowercase());

            if let Some(p) = llm_found {
                if let Some(key) = p.get_api_key() {
                    eprintln!("LLM provider {} found", provider);
                    eprintln!(
                        "  Key: {}...{}",
                        &key[..4.min(key.len())],
                        if key.len() > 8 {
                            &key[key.len() - 4..]
                        } else {
                            ""
                        }
                    );
                } else {
                    eprintln!("LLM provider {} found but no API key set", provider);
                }
            } else {
                eprintln!("LLM provider {} not found in config", provider);
            }

            // Check TTS providers
            let tts_found = config
                .tts
                .providers
                .iter()
                .find(|p| p.name.to_lowercase() == provider.to_lowercase());

            if let Some(p) = tts_found {
                if p.is_configured() {
                    eprintln!("TTS provider {} configured", provider);
                    if let Some(ref api_key) = p.api_key {
                        eprintln!(
                            "  API Key: {}...{}",
                            &api_key[..4.min(api_key.len())],
                            if api_key.len() > 8 {
                                &api_key[api_key.len() - 4..]
                            } else {
                                ""
                            }
                        );
                    }
                } else {
                    eprintln!("TTS provider {} found but not configured", provider);
                }
            }
        }
        CredentialAction::Remove { provider } => {
            let mut config = VoiceConfig::load_from_home()?;

            // Remove from LLM providers
            config.llm.providers.retain(|p| p.name.to_lowercase() != provider.to_lowercase());

            // Clear TTS API key if it's a TTS provider
            for p in &mut config.tts.providers {
                if p.name.to_lowercase() == provider.to_lowercase() {
                    p.api_key = None;
                }
            }

            config.save_to_home()?;
            eprintln!("Credentials removed for {}", provider);
        }
    }

    Ok(())
}

async fn handle_init_command() -> Result<()> {
    use error::VoiceError;

    let config_path = VoiceConfig::config_path()?;

    if config_path.exists() {
        eprintln!("Config file already exists at: {:?}", config_path);
        eprintln!();
        eprintln!("To reset to defaults, delete the file and run init again:");
        eprintln!("  rm {:?}", config_path);
        eprintln!("  claude-voice init");
        return Ok(());
    }

    // Create default config
    let config = VoiceConfig::default();
    config.save_to_home()?;

    eprintln!("Created default config at: {:?}", config_path);
    eprintln!();
    eprintln!("Next steps:");
    eprintln!("1. Set your preferred LLM API key:");
    eprintln!("   claude-voice credentials set google");
    eprintln!("   # or: claude-voice credentials set anthropic");
    eprintln!("   # or: claude-voice credentials set openai");
    eprintln!();
    eprintln!("2. (Optional) Set Gemini API key for TTS:");
    eprintln!("   claude-voice credentials set google_tts");
    eprintln!("   # Note: Google TTS uses the same Gemini API key");
    eprintln!();
    eprintln!("3. Test the configuration:");
    eprintln!("   claude-voice credentials list");

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
