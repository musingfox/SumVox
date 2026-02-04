// CLI argument parsing for sumvox
// Subcommand-based architecture for versatile voice notification

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "sumvox")]
#[command(about = "Voice notification CLI for AI coding tools")]
#[command(version)]
pub struct Cli {
    /// Subcommand to execute (optional: auto-detect json mode from stdin if not specified)
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Direct TTS playback - speak text immediately
    Say(SayArgs),

    /// LLM summarization with TTS - summarize text then speak
    Sum(SumArgs),

    /// Read JSON from stdin (hook mode) - auto-detect format
    Json(JsonArgs),

    /// Initialize config file at ~/.config/sumvox/config.json
    Init(InitArgs),

    /// Manage API credentials
    Credentials {
        #[command(subcommand)]
        action: CredentialAction,
    },
}

/// Arguments for 'say' subcommand
#[derive(Parser, Debug, Clone)]
pub struct SayArgs {
    /// Text to speak
    pub text: String,

    /// TTS engine: auto, macos, google
    #[arg(long, default_value = "auto")]
    pub tts: String,

    /// Voice name (engine-specific)
    /// For macos: Tingting, Meijia, etc.
    /// For google: Aoede, Charon, Fenrir, Kore, Puck, Orus
    #[arg(long)]
    pub voice: Option<String>,

    /// Speech rate for macOS say (90-300), ignored for Google TTS
    #[arg(long, default_value = "200")]
    pub rate: u32,

    /// Volume level (0-100)
    #[arg(long)]
    pub volume: Option<u32>,
}

/// Arguments for 'sum' subcommand
#[derive(Parser, Debug, Clone)]
pub struct SumArgs {
    /// Text to summarize (use "-" to read from stdin)
    pub text: String,

    /// LLM provider: google, anthropic, openai, ollama
    #[arg(long)]
    pub provider: Option<String>,

    /// Model name (e.g., gemini-2.5-flash, gpt-4o-mini)
    #[arg(long)]
    pub model: Option<String>,

    /// Maximum summary length in words
    #[arg(long, default_value = "50")]
    pub max_length: usize,

    /// Only output summary, don't speak
    #[arg(long)]
    pub no_speak: bool,

    /// Request timeout in seconds
    #[arg(long, default_value = "10")]
    pub timeout: u64,

    /// TTS engine: auto, macos, google
    #[arg(long, default_value = "auto")]
    pub tts: String,

    /// Voice name (engine-specific)
    #[arg(long)]
    pub voice: Option<String>,

    /// Speech rate for macOS say (90-300)
    #[arg(long, default_value = "200")]
    pub rate: u32,

    /// Volume level (0-100)
    #[arg(long)]
    pub volume: Option<u32>,
}

/// Arguments for 'json' subcommand (hook mode)
#[derive(Parser, Debug, Clone)]
pub struct JsonArgs {
    /// JSON format: auto, claude-code, gemini-cli, generic
    #[arg(long, default_value = "auto")]
    pub format: String,

    /// Request timeout in seconds
    #[arg(long, default_value = "10")]
    pub timeout: u64,
}

/// Arguments for 'init' subcommand
#[derive(Parser, Debug, Clone)]
pub struct InitArgs {
    /// Force overwrite existing config
    #[arg(long)]
    pub force: bool,
}

#[derive(Subcommand, Debug, Clone)]
pub enum CredentialAction {
    /// Set API key for a provider
    Set {
        /// Provider name: google, anthropic, openai, google_tts
        provider: String,
    },
    /// List configured providers
    List,
    /// Test API key for a provider
    Test {
        /// Provider name
        provider: String,
    },
    /// Remove credentials for a provider
    Remove {
        /// Provider name
        provider: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn test_parse_say_command() {
        let cli = Cli::try_parse_from(["sumvox", "say", "Hello world"]).unwrap();

        match cli.command {
            Some(Commands::Say(args)) => {
                assert_eq!(args.text, "Hello world");
                assert_eq!(args.tts, "auto");
                assert_eq!(args.rate, 200);
                assert_eq!(args.voice, None);
                assert_eq!(args.volume, None);
            }
            _ => panic!("Expected Say command"),
        }
    }

    #[test]
    fn test_parse_say_with_options() {
        let cli = Cli::try_parse_from([
            "sumvox",
            "say",
            "Hello",
            "--tts",
            "macos",
            "--voice",
            "Tingting",
            "--rate",
            "180",
            "--volume",
            "75",
        ])
        .unwrap();

        match cli.command {
            Some(Commands::Say(args)) => {
                assert_eq!(args.text, "Hello");
                assert_eq!(args.tts, "macos");
                assert_eq!(args.voice, Some("Tingting".to_string()));
                assert_eq!(args.rate, 180);
                assert_eq!(args.volume, Some(75));
            }
            _ => panic!("Expected Say command"),
        }
    }

    #[test]
    fn test_parse_sum_command() {
        let cli = Cli::try_parse_from(["sumvox", "sum", "Long text to summarize"]).unwrap();

        match cli.command {
            Some(Commands::Sum(args)) => {
                assert_eq!(args.text, "Long text to summarize");
                assert_eq!(args.provider, None);
                assert_eq!(args.model, None);
                assert_eq!(args.max_length, 50);
                assert!(!args.no_speak);
            }
            _ => panic!("Expected Sum command"),
        }
    }

    #[test]
    fn test_parse_sum_with_options() {
        let cli = Cli::try_parse_from([
            "sumvox",
            "sum",
            "-",
            "--provider",
            "google",
            "--model",
            "gemini-2.5-flash",
            "--max-length",
            "100",
            "--no-speak",
        ])
        .unwrap();

        match cli.command {
            Some(Commands::Sum(args)) => {
                assert_eq!(args.text, "-");
                assert_eq!(args.provider, Some("google".to_string()));
                assert_eq!(args.model, Some("gemini-2.5-flash".to_string()));
                assert_eq!(args.max_length, 100);
                assert!(args.no_speak);
            }
            _ => panic!("Expected Sum command"),
        }
    }

    #[test]
    fn test_parse_json_command() {
        let cli = Cli::try_parse_from(["sumvox", "json"]).unwrap();

        match cli.command {
            Some(Commands::Json(args)) => {
                assert_eq!(args.format, "auto");
                assert_eq!(args.timeout, 10);
            }
            _ => panic!("Expected Json command"),
        }
    }

    #[test]
    fn test_parse_json_with_format() {
        let cli = Cli::try_parse_from(["sumvox", "json", "--format", "claude-code"]).unwrap();

        match cli.command {
            Some(Commands::Json(args)) => {
                assert_eq!(args.format, "claude-code");
            }
            _ => panic!("Expected Json command"),
        }
    }

    #[test]
    fn test_parse_init_command() {
        let cli = Cli::try_parse_from(["sumvox", "init"]).unwrap();

        match cli.command {
            Some(Commands::Init(args)) => {
                assert!(!args.force);
            }
            _ => panic!("Expected Init command"),
        }
    }

    #[test]
    fn test_parse_init_with_force() {
        let cli = Cli::try_parse_from(["sumvox", "init", "--force"]).unwrap();

        match cli.command {
            Some(Commands::Init(args)) => {
                assert!(args.force);
            }
            _ => panic!("Expected Init command"),
        }
    }

    #[test]
    fn test_parse_credentials_set() {
        let cli = Cli::try_parse_from(["sumvox", "credentials", "set", "google"]).unwrap();

        match cli.command {
            Some(Commands::Credentials { action }) => match action {
                CredentialAction::Set { provider } => {
                    assert_eq!(provider, "google");
                }
                _ => panic!("Expected Set action"),
            },
            _ => panic!("Expected Credentials command"),
        }
    }

    #[test]
    fn test_parse_credentials_list() {
        let cli = Cli::try_parse_from(["sumvox", "credentials", "list"]).unwrap();

        match cli.command {
            Some(Commands::Credentials { action }) => {
                assert!(matches!(action, CredentialAction::List));
            }
            _ => panic!("Expected Credentials command"),
        }
    }

    #[test]
    fn test_parse_credentials_test() {
        let cli = Cli::try_parse_from(["sumvox", "credentials", "test", "anthropic"]).unwrap();

        match cli.command {
            Some(Commands::Credentials { action }) => match action {
                CredentialAction::Test { provider } => {
                    assert_eq!(provider, "anthropic");
                }
                _ => panic!("Expected Test action"),
            },
            _ => panic!("Expected Credentials command"),
        }
    }

    #[test]
    fn test_parse_credentials_remove() {
        let cli = Cli::try_parse_from(["sumvox", "credentials", "remove", "openai"]).unwrap();

        match cli.command {
            Some(Commands::Credentials { action }) => match action {
                CredentialAction::Remove { provider } => {
                    assert_eq!(provider, "openai");
                }
                _ => panic!("Expected Remove action"),
            },
            _ => panic!("Expected Credentials command"),
        }
    }

    #[test]
    fn test_cli_verify() {
        Cli::command().debug_assert();
    }
}
