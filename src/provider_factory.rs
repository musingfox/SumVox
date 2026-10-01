// Provider factory for creating LLM providers with fallback support

use crate::config::LlmProviderConfig;
use crate::error::{Result, VoiceError};
use crate::llm::{
    AnthropicProvider, GeminiProvider, LlmProvider, OllamaProvider, OpenAIProvider,
    ANTHROPIC_API_BASE, GEMINI_API_BASE, OPENAI_API_BASE,
};
use std::str::FromStr;
use std::time::Duration;

pub enum Provider {
    Google,
    Anthropic,
    OpenAI,
    Ollama,
    Xai,
}

impl FromStr for Provider {
    type Err = VoiceError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "google" | "gemini" => Ok(Provider::Google),
            "anthropic" | "claude" => Ok(Provider::Anthropic),
            "openai" | "gpt" => Ok(Provider::OpenAI),
            "ollama" | "local" => Ok(Provider::Ollama),
            "xai" | "grok" => Ok(Provider::Xai),
            _ => Err(VoiceError::Config(format!("Unknown provider: {}", s))),
        }
    }
}

pub struct ProviderFactory;

impl ProviderFactory {
    /// Create a single provider from config
    pub fn create_single(config: &LlmProviderConfig) -> Result<Box<dyn LlmProvider>> {
        let timeout = Duration::from_secs(config.timeout);
        let provider: Provider = config.name.parse()?;

        match provider {
            Provider::Google => {
                let api_key = config.get_api_key().ok_or_else(|| {
                    VoiceError::Config(format!(
                        "No API key for Google. Set in config or env var {}",
                        LlmProviderConfig::env_var_name("google")
                    ))
                })?;
                let base_url = config
                    .base_url
                    .clone()
                    .unwrap_or_else(|| GEMINI_API_BASE.to_string());
                Ok(Box::new(GeminiProvider::with_base_url(
                    api_key,
                    config.model.clone(),
                    base_url,
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
                let base_url = config
                    .base_url
                    .clone()
                    .unwrap_or_else(|| ANTHROPIC_API_BASE.to_string());
                Ok(Box::new(AnthropicProvider::with_base_url(
                    api_key,
                    config.model.clone(),
                    base_url,
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
                let base_url = config
                    .base_url
                    .clone()
                    .unwrap_or_else(|| OPENAI_API_BASE.to_string());
                Ok(Box::new(OpenAIProvider::with_base_url(
                    api_key,
                    config.model.clone(),
                    base_url,
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
            Provider::Xai => {
                let api_key = config.get_api_key().ok_or_else(|| {
                    VoiceError::Config(format!(
                        "No API key for xAI. Set in config or env var {}",
                        LlmProviderConfig::env_var_name("xai")
                    ))
                })?;
                let base_url = config
                    .base_url
                    .clone()
                    .unwrap_or_else(|| "https://api.x.ai/v1".to_string());
                Ok(Box::new(OpenAIProvider::with_base_url(
                    api_key,
                    config.model.clone(),
                    base_url,
                    timeout,
                )))
            }
        }
    }

    /// Create a provider by name (for CLI override). When `matching` is the
    /// configured entry for `name`, its base_url, timeout and other settings are
    /// kept and only the model is replaced.
    pub fn create_by_name(
        name: &str,
        model: &str,
        timeout: Duration,
        api_key: Option<&str>,
        matching: Option<&LlmProviderConfig>,
    ) -> Result<Box<dyn LlmProvider>> {
        Self::create_single(&override_config(name, model, timeout, api_key, matching))
    }
}

fn override_config(
    name: &str,
    model: &str,
    timeout: Duration,
    api_key: Option<&str>,
    matching: Option<&LlmProviderConfig>,
) -> LlmProviderConfig {
    if let Some(entry) = matching {
        let mut config = entry.clone();
        config.model = model.to_string();
        if let Some(key) = api_key {
            config.api_key = Some(key.to_string());
        }
        return config;
    }
    LlmProviderConfig {
        name: name.to_string(),
        model: model.to_string(),
        api_key: api_key.map(|s| s.to_string()),
        base_url: None,
        timeout: timeout.as_secs(),
        disable_thinking: None,
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
            "google".parse::<Provider>().unwrap(),
            Provider::Google
        ));
        assert!(matches!(
            "gemini".parse::<Provider>().unwrap(),
            Provider::Google
        ));

        // Anthropic variants
        assert!(matches!(
            "anthropic".parse::<Provider>().unwrap(),
            Provider::Anthropic
        ));
        assert!(matches!(
            "claude".parse::<Provider>().unwrap(),
            Provider::Anthropic
        ));

        // OpenAI variants
        assert!(matches!(
            "openai".parse::<Provider>().unwrap(),
            Provider::OpenAI
        ));
        assert!(matches!(
            "gpt".parse::<Provider>().unwrap(),
            Provider::OpenAI
        ));

        // Ollama variants
        assert!(matches!(
            "ollama".parse::<Provider>().unwrap(),
            Provider::Ollama
        ));
        assert!(matches!(
            "local".parse::<Provider>().unwrap(),
            Provider::Ollama
        ));

        // Case insensitive
        assert!(matches!(
            "GOOGLE".parse::<Provider>().unwrap(),
            Provider::Google
        ));

        // Unknown provider
        assert!("unknown".parse::<Provider>().is_err());
    }

    #[test]
    fn test_create_by_name_google() {
        let _env = crate::test_support::env_guard();
        let result = ProviderFactory::create_by_name(
            "google",
            "gemini-2.5-flash",
            Duration::from_secs(10),
            Some("test-key"),
            None,
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap().name(), "gemini");
    }

    #[test]
    fn test_create_by_name_ollama() {
        let _env = crate::test_support::env_guard();
        let result = ProviderFactory::create_by_name(
            "ollama",
            "llama3.2",
            Duration::from_secs(10),
            None, // Ollama doesn't need API key
            None,
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap().name(), "ollama");
    }

    #[test]
    fn test_create_by_name_missing_api_key() {
        let _env = crate::test_support::env_guard();
        // Clear any env vars
        env::remove_var("GEMINI_API_KEY");

        let result = ProviderFactory::create_by_name(
            "google",
            "gemini-2.5-flash",
            Duration::from_secs(10),
            None, // No API key
            None,
        );
        assert!(result.is_err());
        let err = result.err().unwrap();
        assert!(err.to_string().contains("No API key"));
    }

    #[test]
    fn test_override_config_keeps_configured_entry() {
        let entry = LlmProviderConfig {
            name: "ollama".to_string(),
            model: "llama3.2".to_string(),
            api_key: None,
            base_url: Some("http://gpu-box:11434".to_string()),
            timeout: 45,
            disable_thinking: Some(true),
        };
        let kept = override_config(
            "ollama",
            "llama3.2",
            Duration::from_secs(10),
            None,
            Some(&entry),
        );
        assert_eq!(kept.base_url.as_deref(), Some("http://gpu-box:11434"));
        assert_eq!(kept.timeout, 45);
        assert_eq!(kept.model, "llama3.2");

        let remodeled = override_config(
            "ollama",
            "qwen3",
            Duration::from_secs(10),
            None,
            Some(&entry),
        );
        assert_eq!(remodeled.model, "qwen3");
        assert_eq!(remodeled.base_url.as_deref(), Some("http://gpu-box:11434"));

        let unconfigured = override_config("ollama", "m", Duration::from_secs(10), None, None);
        assert_eq!(unconfigured.base_url, None);
        assert_eq!(unconfigured.timeout, 10);
    }
}
