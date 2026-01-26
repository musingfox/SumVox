// TTS (Text-to-Speech) module
// Provides abstraction over different TTS engines: macOS say, Google Cloud TTS

pub mod google;
pub mod macos;

use async_trait::async_trait;

use crate::error::Result;

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
}

impl TtsEngine {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "macos" | "say" => Some(TtsEngine::MacOS),
            "google" | "google_tts" | "gcloud" => Some(TtsEngine::Google),
            _ => None,
        }
    }
}

impl std::fmt::Display for TtsEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TtsEngine::MacOS => write!(f, "macos"),
            TtsEngine::Google => write!(f, "google"),
        }
    }
}

// Re-export providers
pub use google::GoogleTtsProvider;
pub use macos::MacOsTtsProvider;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tts_engine_from_str() {
        assert_eq!(TtsEngine::from_str("macos"), Some(TtsEngine::MacOS));
        assert_eq!(TtsEngine::from_str("say"), Some(TtsEngine::MacOS));
        assert_eq!(TtsEngine::from_str("google"), Some(TtsEngine::Google));
        assert_eq!(TtsEngine::from_str("google_tts"), Some(TtsEngine::Google));
        assert_eq!(TtsEngine::from_str("gcloud"), Some(TtsEngine::Google));
        assert_eq!(TtsEngine::from_str("unknown"), None);
    }

    #[test]
    fn test_tts_engine_display() {
        assert_eq!(TtsEngine::MacOS.to_string(), "macos");
        assert_eq!(TtsEngine::Google.to_string(), "google");
    }
}
