// TTS (Text-to-Speech) module
// Provides abstraction over different TTS engines with fallback support

pub mod google;
pub mod macos;

use async_trait::async_trait;
use std::str::FromStr;

use crate::config::TtsProviderConfig;
use crate::error::{Result, VoiceError};

/// TTS Provider trait - defines interface for text-to-speech engines
#[async_trait]
pub trait TtsProvider: Send + Sync {
    /// Provider name for logging
    fn name(&self) -> &str;

    /// Check if the provider is available (has credentials, etc.)
    fn is_available(&self) -> bool;

    /// Speak the given text
    /// Returns true if speech was initiated successfully
    async fn speak(&self, text: &str) -> Result<bool>;

    /// Estimate cost per character (for cloud providers)
    /// Returns 0.0 for local engines
    fn estimate_cost(&self, char_count: usize) -> f64;
}

/// TTS Engine type for CLI selection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TtsEngine {
    MacOS,
    Google,
    Auto,
}

impl FromStr for TtsEngine {
    type Err = VoiceError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "macos" | "say" => Ok(TtsEngine::MacOS),
            "google" | "google_tts" | "gcloud" => Ok(TtsEngine::Google),
            "auto" => Ok(TtsEngine::Auto),
            _ => Err(VoiceError::Config(format!("Unknown TTS engine: {}", s))),
        }
    }
}

impl std::fmt::Display for TtsEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TtsEngine::MacOS => write!(f, "macos"),
            TtsEngine::Google => write!(f, "google"),
            TtsEngine::Auto => write!(f, "auto"),
        }
    }
}

// Re-export providers
pub use google::GoogleTtsProvider;
pub use macos::MacOsTtsProvider;

/// Create TTS provider from config array with automatic fallback
///
/// Tries each provider in order until one is available.
/// Returns an error if no provider can be created.
pub fn create_tts_from_config(
    providers: &[TtsProviderConfig],
    is_async: bool,
) -> Result<Box<dyn TtsProvider>> {
    let mut errors = Vec::new();

    for config in providers {
        match create_single_tts(config, is_async) {
            Ok(provider) => {
                if provider.is_available() {
                    tracing::info!(
                        "Using TTS provider: {} (voice: {})",
                        config.name,
                        config.voice.as_deref().unwrap_or("default")
                    );
                    return Ok(provider);
                } else {
                    tracing::debug!("TTS {} created but not available, trying next", config.name);
                    errors.push(format!("{}: not available", config.name));
                }
            }
            Err(e) => {
                tracing::debug!("Failed to create TTS {}: {}", config.name, e);
                errors.push(format!("{}: {}", config.name, e));
            }
        }
    }

    Err(VoiceError::Config(format!(
        "No TTS provider available. Tried: {}",
        errors.join("; ")
    )))
}

/// Create a single TTS provider from config
fn create_single_tts(config: &TtsProviderConfig, is_async: bool) -> Result<Box<dyn TtsProvider>> {
    let volume = config.volume.unwrap_or(100);

    match config.name.to_lowercase().as_str() {
        "macos" | "say" => {
            let voice = config.voice.clone();
            let rate = config.rate.unwrap_or(200);
            Ok(Box::new(MacOsTtsProvider::new(
                voice, rate, volume, is_async,
            )))
        }
        "google" | "google_tts" | "gcloud" | "gemini" => {
            let api_key = config.get_api_key().ok_or_else(|| {
                VoiceError::Config(
                    "Gemini API key not found. Set in config or env var GEMINI_API_KEY".into(),
                )
            })?;

            // Model is required for Google TTS
            let model = config.model.clone().ok_or_else(|| {
                VoiceError::Config(
                    "Google TTS model is required. Specify in config, e.g., 'gemini-2.5-flash-preview-tts'".into()
                )
            })?;

            let voice = config.voice.clone();
            Ok(Box::new(GoogleTtsProvider::new(
                api_key, model, voice, volume,
            )))
        }
        _ => Err(VoiceError::Config(format!(
            "Unknown TTS provider: {}",
            config.name
        ))),
    }
}

/// Create TTS provider by name (for CLI override)
pub fn create_tts_by_name(
    name: &str,
    model: Option<String>,
    voice: Option<String>,
    rate: u32,
    volume: u32,
    is_async: bool,
    api_key: Option<String>,
) -> Result<Box<dyn TtsProvider>> {
    let config = TtsProviderConfig {
        name: name.to_string(),
        model,
        voice,
        api_key,
        rate: Some(rate),
        volume: Some(volume),
    };
    create_single_tts(&config, is_async)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tts_engine_from_str() {
        assert_eq!("macos".parse::<TtsEngine>().ok(), Some(TtsEngine::MacOS));
        assert_eq!("say".parse::<TtsEngine>().ok(), Some(TtsEngine::MacOS));
        assert_eq!("google".parse::<TtsEngine>().ok(), Some(TtsEngine::Google));
        assert_eq!(
            "google_tts".parse::<TtsEngine>().ok(),
            Some(TtsEngine::Google)
        );
        assert_eq!("gcloud".parse::<TtsEngine>().ok(), Some(TtsEngine::Google));
        assert_eq!("auto".parse::<TtsEngine>().ok(), Some(TtsEngine::Auto));
        assert!("unknown".parse::<TtsEngine>().is_err());
    }

    #[test]
    fn test_tts_engine_display() {
        assert_eq!(TtsEngine::MacOS.to_string(), "macos");
        assert_eq!(TtsEngine::Google.to_string(), "google");
        assert_eq!(TtsEngine::Auto.to_string(), "auto");
    }

    #[test]
    fn test_create_macos_tts() {
        let providers = vec![TtsProviderConfig {
            name: "macos".to_string(),
            model: None,
            voice: Some("Tingting".to_string()),
            api_key: None,
            rate: Some(200),
            volume: Some(80),
        }];

        let result = create_tts_from_config(&providers, true);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().name(), "macos");
    }

    #[test]
    fn test_create_tts_fallback() {
        // Google TTS without API key should fallback to macOS
        let providers = vec![
            TtsProviderConfig {
                name: "google".to_string(),
                model: Some("gemini-2.5-flash-preview-tts".to_string()),
                voice: Some("Zephyr".to_string()),
                api_key: None, // No API key
                rate: None,
                volume: None,
            },
            TtsProviderConfig {
                name: "macos".to_string(),
                model: None,
                voice: Some("Tingting".to_string()),
                api_key: None,
                rate: Some(200),
                volume: None,
            },
        ];

        // Clear env vars to ensure fallback
        std::env::remove_var("GOOGLE_CLOUD_PROJECT");
        std::env::remove_var("GCP_PROJECT");

        let result = create_tts_from_config(&providers, true);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().name(), "macos");
    }

    #[test]
    fn test_create_tts_by_name_macos() {
        let result = create_tts_by_name(
            "macos",
            None,
            Some("Tingting".to_string()),
            200,
            100,
            true,
            None,
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap().name(), "macos");
    }

    #[test]
    fn test_create_tts_empty_providers() {
        let providers: Vec<TtsProviderConfig> = vec![];

        let result = create_tts_from_config(&providers, true);
        assert!(result.is_err());
        let err = result.err().unwrap();
        assert!(err.to_string().contains("No TTS provider"));
    }
}
