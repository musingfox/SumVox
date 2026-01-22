// macOS say command wrapper

use std::process::Stdio;
use tokio::process::Command;

use crate::config::VoiceEngineConfig;
use crate::error::{Result, VoiceError};

pub struct VoiceEngine {
    config: VoiceEngineConfig,
}

impl VoiceEngine {
    pub fn new(config: VoiceEngineConfig) -> Self {
        Self { config }
    }

    /// Check if a voice is available on the system
    pub async fn is_voice_available(&self, voice_name: Option<&str>) -> Result<bool> {
        let voice = voice_name.unwrap_or(&self.config.voice_name);

        let output: std::process::Output = Command::new("say")
            .arg("-v")
            .arg("?")
            .output()
            .await
            .map_err(|e| VoiceError::Voice(format!("Failed to check voice availability: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.contains(voice))
    }

    /// Speak a message using macOS say command
    pub async fn speak(&self, message: &str, blocking: Option<bool>) -> Result<bool> {
        if message.trim().is_empty() {
            tracing::warn!("Empty message, skipping voice notification");
            return Ok(false);
        }

        let should_block = blocking.unwrap_or(!self.config.async_mode);

        tracing::info!(
            "Speaking with voice={}, rate={}, async={}",
            self.config.voice_name,
            self.config.rate,
            !should_block
        );

        let mut cmd = Command::new("say");
        cmd.arg("-v")
            .arg(&self.config.voice_name)
            .arg("-r")
            .arg(self.config.rate.to_string())
            .arg(message);

        if should_block {
            // Synchronous execution
            let output: std::process::Output = cmd
                .output()
                .await
                .map_err(|e| VoiceError::Voice(format!("Say command failed: {}", e)))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(VoiceError::Voice(format!("Say command failed: {}", stderr)));
            }

            tracing::debug!("Voice playback completed (blocking)");
            Ok(true)
        } else {
            // Asynchronous execution
            cmd.stdout(Stdio::null()).stderr(Stdio::null()).spawn().map_err(
                |e| VoiceError::Voice(format!("Failed to spawn say command: {}", e)),
            )?;

            tracing::debug!("Voice playback started (non-blocking)");
            Ok(true)
        }
    }

    /// Test voice playback with a simple message
    pub async fn test_voice(&self) -> Result<bool> {
        let test_message = "語音測試成功";
        tracing::info!("Testing voice: {}", self.config.voice_name);
        self.speak(test_message, Some(true)).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> VoiceEngineConfig {
        VoiceEngineConfig {
            engine: "macos_say".to_string(),
            voice_name: "Ting-Ting".to_string(),
            rate: 200,
            volume: 75,
            max_summary_length: 50,
            async_mode: true,
        }
    }

    #[tokio::test]
    async fn test_voice_engine_creation() {
        let config = create_test_config();
        let engine = VoiceEngine::new(config.clone());
        assert_eq!(engine.config.voice_name, "Ting-Ting");
    }

    #[tokio::test]
    async fn test_speak_empty_message() {
        let config = create_test_config();
        let engine = VoiceEngine::new(config);

        let result = engine.speak("", None).await.unwrap();
        assert_eq!(result, false);
    }

    #[tokio::test]
    async fn test_speak_whitespace_only() {
        let config = create_test_config();
        let engine = VoiceEngine::new(config);

        let result = engine.speak("   ", None).await.unwrap();
        assert_eq!(result, false);
    }

    // Note: The following tests require macOS and will actually try to use the say command
    // They are integration tests and may not pass in CI environments

    #[tokio::test]
    #[ignore] // Run with: cargo test -- --ignored
    async fn test_is_voice_available() {
        let config = create_test_config();
        let engine = VoiceEngine::new(config);

        // Check if Ting-Ting is available (should be on macOS with Traditional Chinese)
        let available = engine.is_voice_available(None).await.unwrap();
        assert!(available, "Ting-Ting voice should be available on macOS");
    }

    #[tokio::test]
    #[ignore] // Run with: cargo test -- --ignored
    async fn test_speak_blocking() {
        let config = create_test_config();
        let engine = VoiceEngine::new(config);

        let result = engine.speak("測試", Some(true)).await.unwrap();
        assert!(result);
    }

    #[tokio::test]
    #[ignore] // Run with: cargo test -- --ignored
    async fn test_speak_async() {
        let config = create_test_config();
        let engine = VoiceEngine::new(config);

        let result = engine.speak("測試", Some(false)).await.unwrap();
        assert!(result);
    }
}
