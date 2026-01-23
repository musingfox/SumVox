// Provider factory for creating LLM providers

use crate::credentials::CredentialManager;
use crate::error::{Result, VoiceError};
use crate::llm::{AnthropicProvider, GeminiProvider, LlmProvider, OllamaProvider, OpenAIProvider};
use std::time::Duration;

pub enum Provider {
    Google,
    Anthropic,
    OpenAI,
    Ollama,
}

impl Provider {
    pub fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "google" | "gemini" => Ok(Provider::Google),
            "anthropic" | "claude" => Ok(Provider::Anthropic),
            "openai" | "gpt" => Ok(Provider::OpenAI),
            "ollama" | "local" => Ok(Provider::Ollama),
            _ => Err(VoiceError::Config(format!("Unknown provider: {}", s))),
        }
    }

    pub fn requires_api_key(&self) -> bool {
        !matches!(self, Provider::Ollama)
    }
}

pub struct ProviderFactory;

impl ProviderFactory {
    pub fn create(
        provider: &str,
        model: &str,
        timeout: Duration,
        credential_manager: &CredentialManager,
    ) -> Result<Box<dyn LlmProvider>> {
        let provider_enum = Provider::from_str(provider)?;

        match provider_enum {
            Provider::Google => {
                let api_key = credential_manager.load_api_key("google").ok_or_else(|| {
                    VoiceError::Config(
                        "No API key for Google. Run: claude-voice credentials set google".into(),
                    )
                })?;
                Ok(Box::new(GeminiProvider::new(
                    api_key,
                    model.to_string(),
                    timeout,
                )))
            }
            Provider::Anthropic => {
                let api_key =
                    credential_manager
                        .load_api_key("anthropic")
                        .ok_or_else(|| {
                            VoiceError::Config(
                                "No API key for Anthropic. Run: claude-voice credentials set anthropic"
                                    .into(),
                            )
                        })?;
                Ok(Box::new(AnthropicProvider::new(
                    api_key,
                    model.to_string(),
                    timeout,
                )))
            }
            Provider::OpenAI => {
                let api_key = credential_manager.load_api_key("openai").ok_or_else(|| {
                    VoiceError::Config(
                        "No API key for OpenAI. Run: claude-voice credentials set openai".into(),
                    )
                })?;
                Ok(Box::new(OpenAIProvider::new(
                    api_key,
                    model.to_string(),
                    timeout,
                )))
            }
            Provider::Ollama => Ok(Box::new(OllamaProvider::new(model.to_string(), timeout))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use tempfile::TempDir;

    #[test]
    fn test_provider_from_str() {
        // Google variants
        assert!(matches!(
            Provider::from_str("google").unwrap(),
            Provider::Google
        ));
        assert!(matches!(
            Provider::from_str("gemini").unwrap(),
            Provider::Google
        ));

        // Anthropic variants
        assert!(matches!(
            Provider::from_str("anthropic").unwrap(),
            Provider::Anthropic
        ));
        assert!(matches!(
            Provider::from_str("claude").unwrap(),
            Provider::Anthropic
        ));

        // OpenAI variants
        assert!(matches!(
            Provider::from_str("openai").unwrap(),
            Provider::OpenAI
        ));
        assert!(matches!(Provider::from_str("gpt").unwrap(), Provider::OpenAI));

        // Ollama variants
        assert!(matches!(
            Provider::from_str("ollama").unwrap(),
            Provider::Ollama
        ));
        assert!(matches!(
            Provider::from_str("local").unwrap(),
            Provider::Ollama
        ));

        // Case insensitive
        assert!(matches!(
            Provider::from_str("GOOGLE").unwrap(),
            Provider::Google
        ));

        // Unknown provider
        assert!(Provider::from_str("unknown").is_err());
    }

    #[test]
    fn test_provider_requires_api_key() {
        assert!(Provider::Google.requires_api_key());
        assert!(Provider::Anthropic.requires_api_key());
        assert!(Provider::OpenAI.requires_api_key());
        assert!(!Provider::Ollama.requires_api_key());
    }

    #[test]
    fn test_create_google_provider() {
        let temp_dir = TempDir::new().unwrap();
        let creds_path = temp_dir.path().join("credentials.json");
        let manager = CredentialManager::with_path(creds_path);

        // Set API key
        manager.save_api_key("google", "test-key").unwrap();

        // Create provider
        let provider = ProviderFactory::create(
            "google",
            "gemini-2.5-flash",
            Duration::from_secs(10),
            &manager,
        )
        .unwrap();

        assert_eq!(provider.name(), "gemini");
    }

    #[test]
    fn test_create_anthropic_provider() {
        let temp_dir = TempDir::new().unwrap();
        let creds_path = temp_dir.path().join("credentials.json");
        let manager = CredentialManager::with_path(creds_path);

        // Set API key
        manager.save_api_key("anthropic", "test-key").unwrap();

        // Create provider
        let provider = ProviderFactory::create(
            "anthropic",
            "claude-sonnet-4-5",
            Duration::from_secs(10),
            &manager,
        )
        .unwrap();

        assert_eq!(provider.name(), "anthropic");
    }

    #[test]
    fn test_create_openai_provider() {
        let temp_dir = TempDir::new().unwrap();
        let creds_path = temp_dir.path().join("credentials.json");
        let manager = CredentialManager::with_path(creds_path);

        // Set API key
        manager.save_api_key("openai", "test-key").unwrap();

        // Create provider
        let provider = ProviderFactory::create(
            "openai",
            "gpt-4o-mini",
            Duration::from_secs(10),
            &manager,
        )
        .unwrap();

        assert_eq!(provider.name(), "openai");
    }

    #[test]
    fn test_create_ollama_no_key() {
        let temp_dir = TempDir::new().unwrap();
        let creds_path = temp_dir.path().join("credentials.json");
        let manager = CredentialManager::with_path(creds_path);

        // Ollama doesn't need API key
        let provider = ProviderFactory::create(
            "ollama",
            "llama3.1",
            Duration::from_secs(10),
            &manager,
        )
        .unwrap();

        assert_eq!(provider.name(), "ollama");
    }

    #[test]
    fn test_missing_api_key_error() {
        let temp_dir = TempDir::new().unwrap();
        let creds_path = temp_dir.path().join("credentials.json");
        let manager = CredentialManager::with_path(creds_path);

        // Try to create Google provider without API key
        let result =
            ProviderFactory::create("google", "gemini-2.5-flash", Duration::from_secs(10), &manager);

        assert!(result.is_err());
        let err_msg = format!("{}", result.err().unwrap());
        assert!(err_msg.contains("No API key for Google"));
    }

    #[test]
    fn test_env_var_takes_priority() {
        let temp_dir = TempDir::new().unwrap();
        let creds_path = temp_dir.path().join("credentials.json");
        let manager = CredentialManager::with_path(creds_path);

        // Set file key
        manager.save_api_key("google", "file-key").unwrap();

        // Set env var
        env::set_var("GEMINI_API_KEY", "env-key");

        // Create provider - should use env var
        let provider = ProviderFactory::create(
            "google",
            "gemini-2.5-flash",
            Duration::from_secs(10),
            &manager,
        )
        .unwrap();

        assert!(provider.is_available());

        // Clean up
        env::remove_var("GEMINI_API_KEY");
    }
}
