// sumvox: Voice notification CLI for AI coding tools
// LLM summarization with TTS - supporting multiple AI coding tools

mod audio;
mod cli;
mod config;
mod error;
mod hooks;
mod llm;
mod notify_log;
mod pipeline;
mod provider_factory;
mod queue;
#[cfg(test)]
mod test_support;
mod transcript;
mod tts;

use std::io::{IsTerminal, Read};

use clap::Parser;
use cli::{Cli, Commands, InitArgs, JsonArgs, SayArgs, SumArgs};
use config::{SumvoxConfig, TtsProviderConfig};
use error::{Result, VoiceError};
use hooks::claude_code::ClaudeCodeInput;
use hooks::HookFormat;
use pipeline::{generate_summary, speak_text, LlmOptions, TtsOptions};

#[tokio::main]
async fn main() -> Result<()> {
    // Allow disabling via environment variable (e.g., SUMVOX_DISABLE=1 claude)
    if std::env::var("SUMVOX_DISABLE").is_ok() {
        return Ok(());
    }

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
        None => {
            // No subcommand provided - check if stdin is available (hook mode)
            if !std::io::stdin().is_terminal() {
                tracing::info!("No subcommand provided, auto-detecting json mode from stdin");
                handle_json(JsonArgs {
                    format: "auto".to_string(),
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
            let llm_opts = LlmOptions::default();

            hooks::claude_code::process(&input, &config, &tts_opts, &llm_opts).await?;
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

            let llm_opts = LlmOptions::default();

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
    let toml_path = SumvoxConfig::toml_config_path()?;

    if toml_path.exists() && !args.force {
        eprintln!("Config file already exists at: {:?}", toml_path);
        eprintln!();
        eprintln!("To reset to defaults, use --force:");
        eprintln!("  sumvox init --force");
        return Ok(());
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
            rate: Some(200),
            ..Default::default()
        },
        TtsProviderConfig {
            name: "google".to_string(),
            model: Some("gemini-2.5-flash-preview-tts".to_string()),
            voice: Some("Aoede".to_string()),
            ..Default::default()
        },
    ];

    // Save as TOML (preferred format)
    config.save_to_home()?;

    eprintln!("✓ Created config at: {:?}", toml_path);
    eprintln!();
    eprintln!("Next steps:");
    eprintln!("1. Edit config file and set your API keys:");
    eprintln!("   open ~/.config/sumvox/config.toml");
    eprintln!(r#"   # Replace ${{PROVIDER_API_KEY}} with your actual API keys"#);
    eprintln!("   # Google: https://ai.google.dev");
    eprintln!("   # Anthropic: https://console.anthropic.com");
    eprintln!("   # OpenAI: https://platform.openai.com");
    eprintln!();
    eprintln!("2. Test voice notification:");
    eprintln!("   sumvox say \"Hello, SumVox!\"");
    eprintln!();
    eprintln!("3. See config/recommended.toml for more examples");

    Ok(())
}
