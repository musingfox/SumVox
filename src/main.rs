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
use config::{SumvoxConfig, TtsProviderConfig};
use error::{Result, VoiceError};
use hooks::claude_code::{ClaudeCodeInput, LlmOptions, TtsOptions};
use hooks::HookFormat;
use llm::GenerationRequest;
use provider_factory::ProviderFactory;
use tts::{create_single_tts, create_tts_by_name, create_tts_from_config, TtsEngine, TtsProvider};

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
                })
                .await
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
        .replace("{context}", &text);

    let system_message = Some(config.summarization.system_message.clone());

    // Generate summary
    let llm_opts = LlmOptions {
        provider: args.provider,
        model: args.model,
        timeout: args.timeout,
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
                .replace("{context}", text);

            let system_message = Some(config.summarization.system_message.clone());

            let llm_opts = LlmOptions {
                timeout: args.timeout,
                ..Default::default()
            };

            let summary =
                generate_summary(&config, &llm_opts, system_message, &user_prompt).await?;

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
    // Check for existing config (YAML or JSON)
    let yaml_path = SumvoxConfig::yaml_config_path()?;
    let json_path = SumvoxConfig::config_path()?;

    if (yaml_path.exists() || json_path.exists()) && !args.force {
        let existing_path = if yaml_path.exists() {
            &yaml_path
        } else {
            &json_path
        };
        eprintln!("Config file already exists at: {:?}", existing_path);
        eprintln!();
        eprintln!("To reset to defaults, use --force:");
        eprintln!("  sumvox init --force");
        return Ok(());
    }

    // Remove old JSON config if migrating to YAML
    if args.force && json_path.exists() {
        std::fs::remove_file(&json_path).ok();
    }

    // Create default config with recommended settings
    let mut config = SumvoxConfig::default();

    // Apply recommended settings
    config.summarization.system_message =
        "You are a voice notification assistant. Generate concise summaries suitable for voice playback.".to_string();
    config.summarization.fallback_message = "Task completed".to_string();

    // Set notification TTS to macos by default (fast and free)
    config.hooks.claude_code.notification_tts_provider = Some("macos".to_string());

    // Update default TTS to prefer macOS
    config.tts.providers = vec![
        TtsProviderConfig {
            name: "macos".to_string(),
            model: None,
            voice: None,  // Use system default voice
            api_key: None,
            rate: Some(200),
            volume: None,
        },
        TtsProviderConfig {
            name: "google".to_string(),
            model: Some("gemini-2.5-flash-preview-tts".to_string()),
            voice: Some("Aoede".to_string()),
            api_key: None,
            rate: None,
            volume: None,
        },
    ];

    // Save as YAML (preferred format)
    config.save_to_home()?;

    eprintln!("✓ Created config at: {:?}", yaml_path);
    eprintln!();
    eprintln!("Next steps:");
    eprintln!("1. Set your preferred LLM API key:");
    eprintln!("   export GEMINI_API_KEY=\"your-key-here\"");
    eprintln!("   # or: sumvox credentials set google");
    eprintln!();
    eprintln!("2. Test voice notification:");
    eprintln!("   sumvox say \"測試語音通知\"");
    eprintln!();
    eprintln!("3. Edit config for customization:");
    eprintln!("   # macOS: open ~/.config/sumvox/config.yaml");
    eprintln!("   # See config/recommended.yaml for all options");

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
                let status = if available {
                    "configured"
                } else {
                    "not configured"
                };
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
            config
                .llm
                .providers
                .retain(|p| p.name.to_lowercase() != provider.to_lowercase());

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
            // is_async=false for CLI commands (wait for speech to complete)
            create_tts_from_config(&config.tts.providers, false)?
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

            // is_async=false for CLI commands (wait for speech to complete)
            create_tts_by_name("macos", None, voice, tts_opts.rate, volume, false, None)?
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

            // is_async=false for CLI commands (wait for speech to complete)
            create_tts_by_name(
                "google",
                model,
                Some(voice),
                tts_opts.rate,
                volume,
                false,
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
            speak_with_provider_fallback(&config.tts.providers, text, false).await
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
    providers: &[TtsProviderConfig],
    text: &str,
    is_async: bool,
) -> Result<()> {
    let mut last_error = None;

    for provider_config in providers {
        // Try to create provider
        let provider = match create_single_tts(provider_config, is_async) {
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
            tracing::debug!("TTS provider {} not available, trying next", provider.name());
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
        tracing::warn!("All TTS providers failed. Last error: {}. Notification will be silent.", err);
    } else {
        tracing::warn!("No TTS providers available. Notification will be silent.");
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
            voice: Some("Tingting".to_string()),
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
        assert_eq!(opts.voice, Some("Tingting".to_string()));
        assert_eq!(opts.rate, 200);
        assert_eq!(opts.volume, Some(80));
    }
}
