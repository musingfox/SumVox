// macOS say command TTS provider

use async_trait::async_trait;
use std::process::Stdio;
use tokio::process::Command;

use super::TtsProvider;
use crate::error::{Result, VoiceError};

/// macOS TTS provider using the built-in `say` command
pub struct MacOsTtsProvider {
    voice_name: String,
    rate: u32,
    async_mode: bool,
}

impl MacOsTtsProvider {
    pub fn new(voice_name: String, rate: u32, async_mode: bool) -> Self {
        Self {
            voice_name,
            rate,
            async_mode,
        }
    }

    /// Default provider with Ting-Ting voice for Traditional Chinese
    pub fn default_chinese() -> Self {
        Self::new("Ting-Ting".to_string(), 200, true)
    }

    /// Check if a specific voice is available on the system
    pub async fn is_voice_available_by_name(voice_name: &str) -> Result<bool> {
        let output = Command::new("say")
            .arg("-v")
            .arg("?")
            .output()
            .await
            .map_err(|e| VoiceError::Voice(format!("Failed to check voice availability: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.contains(voice_name))
    }
}

#[async_trait]
impl TtsProvider for MacOsTtsProvider {
    fn name(&self) -> &str {
        "macos"
    }

    fn is_available(&self) -> bool {
        // macOS say is always available on macOS
        cfg!(target_os = "macos")
    }

    async fn speak(&self, text: &str) -> Result<bool> {
        if text.trim().is_empty() {
            tracing::warn!("Empty message, skipping voice notification");
            return Ok(false);
        }

        tracing::info!(
            "Speaking with macOS say: voice={}, rate={}, async={}",
            self.voice_name,
            self.rate,
            self.async_mode
        );

        let mut cmd = Command::new("say");
        cmd.arg("-v")
            .arg(&self.voice_name)
            .arg("-r")
            .arg(self.rate.to_string())
            .arg(text);

        if self.async_mode {
            // Non-blocking: spawn and return immediately
            cmd.stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|e| VoiceError::Voice(format!("Failed to spawn say command: {}", e)))?;

            tracing::debug!("Voice playback started (non-blocking)");
        } else {
            // Blocking: wait for completion
            let output = cmd
                .output()
                .await
                .map_err(|e| VoiceError::Voice(format!("Say command failed: {}", e)))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(VoiceError::Voice(format!("Say command failed: {}", stderr)));
            }

            tracing::debug!("Voice playback completed (blocking)");
        }

        Ok(true)
    }

    fn estimate_cost(&self, _char_count: usize) -> f64 {
        // macOS say is free
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_macos_provider_creation() {
        let provider = MacOsTtsProvider::new("Ting-Ting".to_string(), 180, false);
        assert_eq!(provider.name(), "macos");
        assert_eq!(provider.voice_name, "Ting-Ting");
        assert_eq!(provider.rate, 180);
        assert!(!provider.async_mode);
    }

    #[test]
    fn test_default_chinese() {
        let provider = MacOsTtsProvider::default_chinese();
        assert_eq!(provider.voice_name, "Ting-Ting");
        assert_eq!(provider.rate, 200);
        assert!(provider.async_mode);
    }

    #[test]
    fn test_estimate_cost_is_zero() {
        let provider = MacOsTtsProvider::default_chinese();
        assert_eq!(provider.estimate_cost(100), 0.0);
        assert_eq!(provider.estimate_cost(10000), 0.0);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_is_available_on_macos() {
        let provider = MacOsTtsProvider::default_chinese();
        assert!(provider.is_available());
    }

    #[tokio::test]
    async fn test_speak_empty_message() {
        let provider = MacOsTtsProvider::default_chinese();
        let result = provider.speak("").await.unwrap();
        assert!(!result);
    }

    #[tokio::test]
    async fn test_speak_whitespace_only() {
        let provider = MacOsTtsProvider::default_chinese();
        let result = provider.speak("   ").await.unwrap();
        assert!(!result);
    }

    #[tokio::test]
    #[ignore] // Run with: cargo test -- --ignored
    async fn test_voice_availability() {
        let available = MacOsTtsProvider::is_voice_available_by_name("Ting-Ting").await;
        assert!(available.is_ok());
    }
}
