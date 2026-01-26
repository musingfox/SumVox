// CLI argument parsing for claude-voice

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "claude-voice")]
#[command(about = "Voice notification for Claude Code")]
#[command(version)]
pub struct Cli {
    /// LLM provider: google, anthropic, openai, ollama
    #[arg(long)]
    pub provider: Option<String>,

    /// Model name (e.g., gemini-2.5-flash, gpt-4o-mini)
    #[arg(long)]
    pub model: Option<String>,

    /// Request timeout in seconds
    #[arg(long, default_value = "10")]
    pub timeout: u64,

    /// TTS engine: macos, google
    #[arg(long, default_value = "macos")]
    pub tts: String,

    /// TTS voice name (engine-specific)
    /// For --tts macos: Ting-Ting, Meijia, etc.
    /// For --tts google: Aoede, Charon, Fenrir, Kore, Puck, Orus (Gemini TTS)
    #[arg(long)]
    pub tts_voice: Option<String>,

    /// Speech rate for macOS say (90-300), ignored for Google TTS
    #[arg(long, default_value = "200")]
    pub rate: u32,

    /// Maximum summary length in characters
    #[arg(long, default_value = "50")]
    pub max_length: usize,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Manage API credentials
    Credentials {
        #[command(subcommand)]
        action: CredentialAction,
    },
}

#[derive(Subcommand, Debug)]
pub enum CredentialAction {
    /// Set API key for a provider
    Set {
        /// Provider name: google, anthropic, openai
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
    fn test_parse_with_defaults() {
        let cli = Cli::try_parse_from(["claude-voice"]).unwrap();

        assert_eq!(cli.provider, None);
        assert_eq!(cli.model, None);
        assert_eq!(cli.timeout, 10);
        assert_eq!(cli.tts, "macos");
        assert_eq!(cli.tts_voice, None);
        assert_eq!(cli.rate, 200);
        assert_eq!(cli.max_length, 50);
        assert!(cli.command.is_none());
    }

    #[test]
    fn test_parse_provider_model() {
        let cli = Cli::try_parse_from([
            "claude-voice",
            "--provider",
            "google",
            "--model",
            "gemini-2.5-flash",
        ])
        .unwrap();

        assert_eq!(cli.provider, Some("google".to_string()));
        assert_eq!(cli.model, Some("gemini-2.5-flash".to_string()));
    }

    #[test]
    fn test_parse_all_options() {
        let cli = Cli::try_parse_from([
            "claude-voice",
            "--provider",
            "openai",
            "--model",
            "gpt-4o-mini",
            "--timeout",
            "30",
            "--tts",
            "google",
            "--tts-voice",
            "zh-TW-Wavenet-A",
            "--rate",
            "180",
            "--max-length",
            "100",
        ])
        .unwrap();

        assert_eq!(cli.provider, Some("openai".to_string()));
        assert_eq!(cli.model, Some("gpt-4o-mini".to_string()));
        assert_eq!(cli.timeout, 30);
        assert_eq!(cli.tts, "google");
        assert_eq!(cli.tts_voice, Some("zh-TW-Wavenet-A".to_string()));
        assert_eq!(cli.rate, 180);
        assert_eq!(cli.max_length, 100);
    }

    #[test]
    fn test_parse_credentials_set() {
        let cli = Cli::try_parse_from(["claude-voice", "credentials", "set", "google"]).unwrap();

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
        let cli = Cli::try_parse_from(["claude-voice", "credentials", "list"]).unwrap();

        match cli.command {
            Some(Commands::Credentials { action }) => {
                assert!(matches!(action, CredentialAction::List));
            }
            _ => panic!("Expected Credentials command"),
        }
    }

    #[test]
    fn test_parse_credentials_test() {
        let cli =
            Cli::try_parse_from(["claude-voice", "credentials", "test", "anthropic"]).unwrap();

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
        let cli =
            Cli::try_parse_from(["claude-voice", "credentials", "remove", "openai"]).unwrap();

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
