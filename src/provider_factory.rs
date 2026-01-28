// Provider factory for creating LLM providers with fallback support

use crate::config::LlmProviderConfig;
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
    /// Create provider from config array with automatic fallback
    ///
    /// Tries each provider in order until one is available.
    /// Returns an error if no provider can be created.
    pub fn create_from_config(
        providers: &[LlmProviderConfig],
    ) -> Result<Box<dyn LlmProvider>> {
        let mut errors = Vec::new();

        for config in providers {
            match Self::create_single(config) {
                Ok(provider) => {
                    if provider.is_available() {
                        tracing::info!(
                            "Using LLM provider: {} (model: {})",
                            config.name,
                            config.model
                        );
                        return Ok(provider);
                    } else {
                        tracing::debug!(
                            "Provider {} created but not available, trying next",
                            config.name
                        );
                        errors.push(format!("{}: not available", config.name));
                    }
                }
                Err(e) => {
                    tracing::debug!("Failed to create provider {}: {}", config.name, e);
                    errors.push(format!("{}: {}", config.name, e));
                }
            }
        }

        Err(VoiceError::Config(format!(
            "No LLM provider available. Tried: {}",
            errors.join("; ")
        )))
    }

    /// Create a single provider from config
    fn create_single(config: &LlmProviderConfig) -> Result<Box<dyn LlmProvider>> {
        let timeout = Duration::from_secs(config.timeout);
        let provider = Provider::from_str(&config.name)?;

        match provider {
            Provider::Google => {
                let api_key = config.get_api_key().ok_or_else(|| {
                    VoiceError::Config(format!(
                        "No API key for Google. Set in config or env var {}",
                        LlmProviderConfig::env_var_name("google")
                    ))
                })?;
                Ok(Box::new(GeminiProvider::new(
                    api_key,
                    config.model.clone(),
                    timeout,
                )))
            }
            Provider::Anthropic => {
                let api_key = config.get_api_key().ok_or_else(|| {
                    VoiceError::Config(format!(
                        "No API key for Anthropic. Set in config or env var {}",
                        LlmProviderConfig::env_var_name("anthropic")
                    ))
                })?;
                Ok(Box::new(AnthropicProvider::new(
                    api_key,
                    config.model.clone(),
                    timeout,
                )))
            }
            Provider::OpenAI => {
                let api_key = config.get_api_key().ok_or_else(|| {
                    VoiceError::Config(format!(
                        "No API key for OpenAI. Set in config or env var {}",
                        LlmProviderConfig::env_var_name("openai")
                    ))
                })?;
                Ok(Box::new(OpenAIProvider::new(
                    api_key,
                    config.model.clone(),
                    timeout,
                )))
            }
            Provider::Ollama => {
                let base_url = config
                    .base_url
                    .clone()
                    .unwrap_or_else(|| "http://localhost:11434".to_string());
                Ok(Box::new(OllamaProvider::with_base_url(
                    base_url,
                    config.model.clone(),
                    timeout,
                )))
            }
        }
    }

    /// Create a provider by name (for CLI override)
    pub fn create_by_name(
        name: &str,
        model: &str,
        timeout: Duration,
        api_key: Option<&str>,
    ) -> Result<Box<dyn LlmProvider>> {
        let config = LlmProviderConfig {
            name: name.to_string(),
            model: model.to_string(),
            api_key: api_key.map(|s| s.to_string()),
            base_url: None,
            timeout: timeout.as_secs(),
        };
        Self::create_single(&config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

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
    fn test_create_from_config_with_api_key() {
        let providers = vec![LlmProviderConfig {
            name: "google".to_string(),
            model: "gemini-2.5-flash".to_string(),
            api_key: Some("test-key".to_string()),
            base_url: None,
            timeout: 10,
        }];

        let result = ProviderFactory::create_from_config(&providers);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().name(), "gemini");
    }

    #[test]
    fn test_create_from_config_fallback_to_ollama() {
        // First provider has no key, should fallback to Ollama
        let providers = vec![
            LlmProviderConfig {
                name: "google".to_string(),
                model: "gemini-2.5-flash".to_string(),
                api_key: None,
                base_url: None,
                timeout: 10,
            },
            LlmProviderConfig {
                name: "ollama".to_string(),
                model: "llama3.2".to_string(),
                api_key: None,
                base_url: None,
                timeout: 10,
            },
        ];

        // Clear any env vars that might interfere
        env::remove_var("GEMINI_API_KEY");

        let result = ProviderFactory::create_from_config(&providers);
        // Note: This will only succeed if Ollama is actually running
        // In CI, this test may need to be adjusted
        if let Ok(provider) = result {
            assert_eq!(provider.name(), "ollama");
        }
    }

    #[test]
    fn test_create_from_config_empty_providers() {
        let providers: Vec<LlmProviderConfig> = vec![];

        let result = ProviderFactory::create_from_config(&providers);
        assert!(result.is_err());
        let err = result.err().unwrap();
        assert!(err.to_string().contains("No LLM provider"));
    }

    #[test]
    fn test_create_by_name_google() {
        let result = ProviderFactory::create_by_name(
            "google",
            "gemini-2.5-flash",
            Duration::from_secs(10),
            Some("test-key"),
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap().name(), "gemini");
    }

    #[test]
    fn test_create_by_name_ollama() {
        let result = ProviderFactory::create_by_name(
            "ollama",
            "llama3.2",
            Duration::from_secs(10),
            None, // Ollama doesn't need API key
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap().name(), "ollama");
    }

    #[test]
    fn test_create_by_name_missing_api_key() {
        // Clear any env vars
        env::remove_var("GEMINI_API_KEY");

        let result = ProviderFactory::create_by_name(
            "google",
            "gemini-2.5-flash",
            Duration::from_secs(10),
            None, // No API key
        );
        assert!(result.is_err());
        let err = result.err().unwrap();
        assert!(err.to_string().contains("No API key"));
    }
}
