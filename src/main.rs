// sumvox: Voice notification CLI for AI coding tools
// LLM summarization with TTS - supporting multiple AI coding tools

mod cli;
mod config;
mod error;
mod hooks;
mod llm;
mod provider_factory;
mod transcript;
mod tts;

use std::io::{IsTerminal, Read};
use std::time::Duration;

use clap::Parser;
use cli::{Cli, Commands, CredentialAction, InitArgs, JsonArgs, SayArgs, SumArgs};
use config::SumvoxConfig;
use error::{Result, VoiceError};
use hooks::claude_code::{ClaudeCodeInput, LlmOptions, TtsOptions};
use hooks::HookFormat;
use llm::{CostTracker, GenerationRequest};
use provider_factory::ProviderFactory;
use tts::{create_tts_by_name, create_tts_from_config, TtsEngine, TtsProvider};

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

    // Dispatch subcommands
    match cli.command {
        Some(Commands::Say(args)) => handle_say(args).await,
        Some(Commands::Sum(args)) => handle_sum(args).await,
        Some(Commands::Json(args)) => handle_json(args).await,
        Some(Commands::Init(args)) => handle_init(args).await,
        Some(Commands::Credentials { action }) => handle_credentials(action).await,
        None => {
            // No subcommand provided - check if stdin is available (hook mode)
            if !std::io::stdin().is_terminal() {
                tracing::info!("No subcommand provided, auto-detecting json mode from stdin");
                handle_json(JsonArgs {
                    format: "auto".to_string(),
                    timeout: 10,
                }).await
            } else {
                // No stdin available, show help
                eprintln!("Error: No subcommand provided and stdin is not available");
                eprintln!("Run 'sumvox --help' for usage information");
                Err(VoiceError::Config("No subcommand provided".into()))
            }
        }
    }
}

// ============================================================================
// Say Command - Direct TTS
// ============================================================================

async fn handle_say(args: SayArgs) -> Result<()> {
    tracing::info!("sumvox say: {}", args.text);

    let config = SumvoxConfig::load_from_home()?;

    let tts_opts = TtsOptions {
        engine: args.tts,
        voice: args.voice,
        rate: args.rate,
        volume: args.volume,
    };

    speak_text(&config, &tts_opts, &args.text).await?;

    tracing::info!("sumvox say completed");
    Ok(())
}

// ============================================================================
// Sum Command - LLM Summarization + TTS
// ============================================================================

async fn handle_sum(args: SumArgs) -> Result<()> {
    // Read text: from stdin if "-", otherwise use provided text
    let text = if args.text == "-" {
        let mut buffer = String::new();
        std::io::stdin()
            .read_to_string(&mut buffer)
            .map_err(VoiceError::Io)?;
        buffer
    } else {
        args.text.clone()
    };

    if text.trim().is_empty() {
        return Err(VoiceError::Config("Empty text provided".into()));
    }

    tracing::info!("sumvox sum: {} chars", text.len());

    let config = SumvoxConfig::load_from_home()?;

    // Build summarization prompt
    let user_prompt = config
        .summarization
        .prompt_template
        .replace("{max_length}", &args.max_length.to_string())
        .replace("{context}", &text);

    let system_message = Some(config.summarization.system_message.clone());

    // Generate summary
    let llm_opts = LlmOptions {
        provider: args.provider,
        model: args.model,
        timeout: args.timeout,
        max_length: args.max_length,
    };

    let summary = generate_summary(&config, &llm_opts, system_message, &user_prompt).await?;

    if summary.is_empty() {
        eprintln!("Warning: Empty summary generated");
        return Ok(());
    }

    // Output summary
    println!("{}", summary);

    // Speak if not --no-speak
    if !args.no_speak {
        let tts_opts = TtsOptions {
            engine: args.tts,
            voice: args.voice,
            rate: args.rate,
            volume: args.volume,
        };

        speak_text(&config, &tts_opts, &summary).await?;
    }

    tracing::info!("sumvox sum completed");
    Ok(())
}

// ============================================================================
// Json Command - Hook Mode with Format Detection
// ============================================================================

async fn handle_json(args: JsonArgs) -> Result<()> {
    tracing::info!("sumvox json: reading from stdin");

    // Read JSON from stdin
    let mut input_buffer = String::new();
    std::io::stdin()
        .read_to_string(&mut input_buffer)
        .map_err(VoiceError::Io)?;

    if input_buffer.trim().is_empty() {
        return Err(VoiceError::Config("Empty JSON input".into()));
    }

    // Detect or use specified format
    let (_json, detected_format) = hooks::parse_input(&input_buffer)?;

    let format = args.format.parse().unwrap_or(detected_format);

    tracing::info!("Hook format: {:?}", format);

    let config = SumvoxConfig::load_from_home()?;

    match format {
        HookFormat::ClaudeCode => {
            let input = ClaudeCodeInput::parse(&input_buffer)?;
            let tts_opts = TtsOptions::default();
            let llm_opts = LlmOptions {
                timeout: args.timeout,
                ..Default::default()
            };

            hooks::claude_code::process(&input, &config, &tts_opts, &llm_opts).await?;
        }
        HookFormat::GeminiCli => {
            // TODO: Implement Gemini CLI hook handler
            tracing::warn!("Gemini CLI format not yet implemented");
            eprintln!("Warning: Gemini CLI format not yet implemented");
        }
        HookFormat::Generic => {
            // Generic format: extract text and summarize
            let generic = hooks::parse_generic(&input_buffer)?;
            let text = generic.get_text().unwrap(); // Already validated

            // Use sum logic
            let user_prompt = config
                .summarization
                .prompt_template
                .replace("{max_length}", &config.summarization.max_length.to_string())
                .replace("{context}", text);

            let system_message = Some(config.summarization.system_message.clone());

            let llm_opts = LlmOptions {
                timeout: args.timeout,
                ..Default::default()
            };

            let summary = generate_summary(&config, &llm_opts, system_message, &user_prompt).await?;

            if !summary.is_empty() {
                println!("{}", summary);
                let tts_opts = TtsOptions::default();
                speak_text(&config, &tts_opts, &summary).await?;
            }
        }
    }

    tracing::info!("sumvox json completed");
    Ok(())
}

// ============================================================================
// Init Command
// ============================================================================

async fn handle_init(args: InitArgs) -> Result<()> {
    let config_path = SumvoxConfig::config_path()?;

    if config_path.exists() && !args.force {
        eprintln!("Config file already exists at: {:?}", config_path);
        eprintln!();
        eprintln!("To reset to defaults, use --force:");
        eprintln!("  sumvox init --force");
        return Ok(());
    }

    // Create default config
    let config = SumvoxConfig::default();
    config.save_to_home()?;

    eprintln!("Created config at: {:?}", config_path);
    eprintln!();
    eprintln!("Next steps:");
    eprintln!("1. Set your preferred LLM API key:");
    eprintln!("   sumvox credentials set google");
    eprintln!("   # or: sumvox credentials set anthropic");
    eprintln!("   # or: sumvox credentials set openai");
    eprintln!();
    eprintln!("2. (Optional) Set Gemini API key for TTS:");
    eprintln!("   sumvox credentials set google_tts");
    eprintln!();
    eprintln!("3. Test with:");
    eprintln!("   sumvox say \"Hello world\"");
    eprintln!("   sumvox sum \"Long text to summarize...\"");

    Ok(())
}

// ============================================================================
// Credentials Command
// ============================================================================

async fn handle_credentials(action: CredentialAction) -> Result<()> {
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
            let mut config = SumvoxConfig::load_from_home()?;

            if provider.to_lowercase() == "google_tts" || provider.to_lowercase() == "gemini_tts" {
                // For TTS, set Gemini API key
                config.set_tts_api_key(api_key.trim());
            } else {
                config.set_llm_api_key(&provider, api_key.trim());
            }

            config.save_to_home()?;
            eprintln!("API key saved for {}", provider);
        }
        CredentialAction::List => {
            let config = SumvoxConfig::load_from_home()?;

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
            let config = SumvoxConfig::load_from_home()?;

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
            let mut config = SumvoxConfig::load_from_home()?;

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

// ============================================================================
// Shared Utilities
// ============================================================================

/// Generate summary using LLM
async fn generate_summary(
    config: &SumvoxConfig,
    llm_opts: &LlmOptions,
    system_message: Option<String>,
    prompt: &str,
) -> Result<String> {
    let llm_config = &config.llm;

    // Initialize cost tracker
    let cost_tracker = if config.cost_control.usage_tracking {
        Some(CostTracker::new(&config.cost_control.usage_file))
    } else {
        None
    };

    // Check budget
    if let Some(ref tracker) = cost_tracker {
        let daily_limit = config.cost_control.daily_limit_usd;
        let under_budget = tracker.check_budget(daily_limit).await?;

        if !under_budget {
            eprintln!("Warning: Daily budget ${:.2} exceeded", daily_limit);
            tracing::warn!("Daily budget limit ${} exceeded", daily_limit);
            return Ok(String::new());
        }
    }

    // Create provider: CLI override or config fallback chain
    let provider = if llm_opts.provider.is_some() || llm_opts.model.is_some() {
        // CLI specified - create specific provider
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
        disable_thinking: llm_config.parameters.disable_thinking,
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
                .or_else(|| macos_config.and_then(|p| p.voice.clone()))
                .unwrap_or_else(|| "Ting-Ting".to_string());

            let volume = tts_opts
                .volume
                .or_else(|| macos_config.and_then(|p| p.volume))
                .unwrap_or(100);

            create_tts_by_name("macos", Some(voice), tts_opts.rate, volume, true, None)?
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

            // Priority: CLI > Config > Default
            let voice = tts_opts
                .voice
                .clone()
                .or_else(|| google_config.and_then(|p| p.voice.clone()))
                .unwrap_or_else(|| "Aoede".to_string());

            let volume = tts_opts
                .volume
                .or_else(|| google_config.and_then(|p| p.volume))
                .unwrap_or(100);

            create_tts_by_name("google", Some(voice), tts_opts.rate, volume, true, api_key)?
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

    // Speak with error handling (TTS failures should be silent)
    match provider.speak(text).await {
        Ok(_) => tracing::debug!("TTS playback completed"),
        Err(e) => {
            tracing::warn!("TTS playback failed: {}. Notification will be silent.", e);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tts_options_from_say_args() {
        let args = SayArgs {
            text: "Hello".to_string(),
            tts: "macos".to_string(),
            voice: Some("Ting-Ting".to_string()),
            rate: 200,
            volume: Some(80),
        };

        let opts = TtsOptions {
            engine: args.tts,
            voice: args.voice,
            rate: args.rate,
            volume: args.volume,
        };

        assert_eq!(opts.engine, "macos");
        assert_eq!(opts.voice, Some("Ting-Ting".to_string()));
        assert_eq!(opts.rate, 200);
        assert_eq!(opts.volume, Some(80));
    }
}
