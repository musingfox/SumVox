// macOS say command TTS provider

use std::sync::atomic::{AtomicU64, Ordering};

use async_trait::async_trait;
use tokio::process::Command;

use super::TtsProvider;
use crate::error::{Result, VoiceError};

// Per-call counter so the temp path is unique even for concurrent calls that
// share a PID — same collision-safety scheme as audio/normalize.rs.
static CALL_SEQ: AtomicU64 = AtomicU64::new(0);

/// macOS TTS provider using the built-in `say` command
pub struct MacOsTtsProvider {
    voice_name: Option<String>,
    rate: u32,
    // `say` itself has no volume flag, so we render to a file and let the
    // platform player apply the volume on playback. This also routes macOS TTS
    // through the same playback choke point as every other provider (honors the
    // volume knob on output devices with no software system volume, drives the
    // avatar).
    volume: u32,
}

impl MacOsTtsProvider {
    pub fn new(voice_name: Option<String>, rate: u32, volume: u32) -> Self {
        Self {
            voice_name,
            rate,
            volume,
        }
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
            "Speaking with macOS say: voice={:?}, rate={}, volume={}",
            self.voice_name,
            self.rate,
            self.volume
        );

        // Render to a temp AIFF, then play it so the volume knob applies.
        // Qualify by PID + per-call counter so concurrent invocations (rapid
        // `sumvox say` calls that don't hold the hook queue lock, or across
        // processes) never clobber each other's file.
        let aiff_path = std::env::temp_dir().join(format!(
            "sumvox_macos_{}_{}.aiff",
            std::process::id(),
            CALL_SEQ.fetch_add(1, Ordering::Relaxed)
        ));

        let mut cmd = Command::new("say");
        cmd.arg("-o").arg(&aiff_path);

        // Only add -v argument if voice is specified and not empty
        if let Some(ref voice) = self.voice_name {
            if !voice.trim().is_empty() {
                cmd.arg("-v").arg(voice);
            }
        }

        cmd.arg("-r").arg(self.rate.to_string()).arg(text);

        // Blocking: wait for synthesis to finish
        let output = cmd
            .output()
            .await
            .map_err(|e| VoiceError::Voice(format!("Say command failed: {}", e)))?;

        if !output.status.success() {
            // `say` may have left a partial file behind before failing.
            let _ = std::fs::remove_file(&aiff_path);
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(VoiceError::Voice(format!("Say command failed: {}", stderr)));
        }

        // Play at the configured volume; clean up on every path (including
        // playback error).
        let result = crate::audio::player::play_file(&aiff_path, self.volume);
        let _ = std::fs::remove_file(&aiff_path);
        result?;

        tracing::debug!("Voice playback completed (blocking)");

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

    #[tokio::test]
    async fn test_speak_empty_message() {
        let provider = MacOsTtsProvider::new(Some("Tingting".to_string()), 200, 100);
        let result = provider.speak("").await.unwrap();
        assert!(!result);
        assert!(!provider.speak("   ").await.unwrap());
        assert_eq!(provider.estimate_cost(100), 0.0);
    }

    // Exercises the full render-to-file + playback path at a low volume; fails
    // if `say -o` or the playback handoff breaks. Volume 1 stays near-silent.
    #[cfg(target_os = "macos")]
    #[tokio::test]
    #[ignore = "e2e-audio"]
    async fn test_speak_renders_and_plays() {
        let provider = MacOsTtsProvider::new(None, 300, 1);
        let result = provider.speak("test").await.unwrap();
        assert!(result);
    }
}
