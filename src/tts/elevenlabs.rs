// ElevenLabs Text-to-Speech provider
// Docs: https://elevenlabs.io/docs/api-reference/text-to-speech/convert
// Pricing: $0.06 / 1K chars (Flash v2.5), $0.12 / 1K chars (Multilingual v2/v3)

use async_trait::async_trait;
use reqwest::Client;
use serde::Serialize;
use std::io::Write;
use std::time::Duration;

use super::TtsProvider;
use crate::error::{Result, VoiceError};

const ELEVENLABS_API_BASE: &str = "https://api.elevenlabs.io/v1/text-to-speech";

/// Default voice ID — "Rachel" (one of ElevenLabs' stock voices)
const DEFAULT_VOICE_ID: &str = "21m00Tcm4TlvDq8ikWAM";

/// Default model — Flash v2.5: ~75ms latency, $0.06/1K chars
const DEFAULT_MODEL_ID: &str = "eleven_flash_v2_5";

/// Default output format — MP3 44.1kHz 128kbps (free tier)
const DEFAULT_OUTPUT_FORMAT: &str = "mp3_44100_128";

/// Maximum characters per request (ElevenLabs limit)
const MAX_TEXT_LENGTH: usize = 5_000;

/// Per-character cost for Flash v2.5 ($0.06 per 1K chars).
/// Multilingual models cost ~2x; this is a coarse estimate.
const COST_PER_CHAR_FLASH: f64 = 0.00006;
const COST_PER_CHAR_MULTILINGUAL: f64 = 0.00012;

pub struct ElevenLabsProvider {
    api_key: String,
    voice_id: String,
    model_id: String,
    output_format: String,
    speed: Option<f32>,
    stability: Option<f32>,
    style: Option<f32>,
    volume: u32,
}

#[derive(Debug, Serialize)]
struct ElevenLabsRequest {
    text: String,
    model_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    voice_settings: Option<VoiceSettings>,
}

#[derive(Debug, Serialize)]
struct VoiceSettings {
    /// Speech speed multiplier (0.7-1.2; 1.0 default)
    #[serde(skip_serializing_if = "Option::is_none")]
    speed: Option<f32>,
    /// Voice stability (0.0-1.0). Higher = calmer, less pitch variation.
    #[serde(skip_serializing_if = "Option::is_none")]
    stability: Option<f32>,
    /// Style exaggeration (0.0-1.0). Lower = less expressive, flatter pitch.
    #[serde(skip_serializing_if = "Option::is_none")]
    style: Option<f32>,
}

impl ElevenLabsProvider {
    pub fn new(
        api_key: String,
        voice_id: Option<String>,
        model_id: Option<String>,
        speed: Option<f32>,
        stability: Option<f32>,
        style: Option<f32>,
        volume: u32,
    ) -> Self {
        Self {
            api_key,
            voice_id: voice_id.unwrap_or_else(|| DEFAULT_VOICE_ID.to_string()),
            model_id: model_id.unwrap_or_else(|| DEFAULT_MODEL_ID.to_string()),
            output_format: DEFAULT_OUTPUT_FORMAT.to_string(),
            speed: speed.map(|s| s.clamp(0.7, 1.2)),
            stability: stability.map(|s| s.clamp(0.0, 1.0)),
            style: style.map(|s| s.clamp(0.0, 1.0)),
            volume,
        }
    }

    fn create_client() -> Result<Client> {
        Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| VoiceError::Voice(format!("Failed to create HTTP client: {}", e)))
    }

    fn play_mp3(&self, audio_data: &[u8]) -> Result<()> {
        tracing::debug!(
            "Playing ElevenLabs audio: {} bytes, volume: {}",
            audio_data.len(),
            self.volume
        );

        let tmp_path = std::env::temp_dir().join("sumvox_elevenlabs.mp3");
        std::fs::File::create(&tmp_path)
            .and_then(|mut f| f.write_all(audio_data))
            .map_err(|e| VoiceError::Voice(format!("Failed to write temp MP3: {}", e)))?;

        let afplay_volume = self.volume as f32 / 100.0;
        let status = std::process::Command::new("afplay")
            .arg("-v")
            .arg(format!("{:.2}", afplay_volume))
            .arg(&tmp_path)
            .status()
            .map_err(|e| VoiceError::Voice(format!("Failed to run afplay: {}", e)))?;

        let _ = std::fs::remove_file(&tmp_path);

        if !status.success() {
            return Err(VoiceError::Voice("afplay exited with error".to_string()));
        }

        Ok(())
    }
}

#[async_trait]
impl TtsProvider for ElevenLabsProvider {
    fn name(&self) -> &str {
        "elevenlabs"
    }

    fn is_available(&self) -> bool {
        !self.api_key.is_empty() && !self.api_key.starts_with("${")
    }

    async fn speak(&self, text: &str) -> Result<bool> {
        if text.trim().is_empty() {
            tracing::warn!("Empty message, skipping voice notification");
            return Ok(false);
        }

        let text = if text.len() > MAX_TEXT_LENGTH {
            tracing::warn!(
                "Text exceeds {} chars, truncating to limit",
                MAX_TEXT_LENGTH
            );
            &text[..MAX_TEXT_LENGTH]
        } else {
            text
        };

        tracing::info!(
            "Speaking with ElevenLabs: voice={}, model={}, chars={}",
            self.voice_id,
            self.model_id,
            text.len()
        );

        let url = format!(
            "{}/{}?output_format={}",
            ELEVENLABS_API_BASE, self.voice_id, self.output_format
        );

        let voice_settings =
            if self.speed.is_some() || self.stability.is_some() || self.style.is_some() {
                Some(VoiceSettings {
                    speed: self.speed,
                    stability: self.stability,
                    style: self.style,
                })
            } else {
                None
            };

        let request = ElevenLabsRequest {
            text: text.to_string(),
            model_id: self.model_id.clone(),
            voice_settings,
        };

        let client = Self::create_client()?;

        let response = client
            .post(&url)
            .header("xi-api-key", &self.api_key)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| VoiceError::Voice(format!("ElevenLabs API request failed: {}", e)))?;

        let status = response.status();

        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(VoiceError::Voice(format!(
                "ElevenLabs API error ({}): {}",
                status, error_text
            )));
        }

        let audio_data = response
            .bytes()
            .await
            .map_err(|e| VoiceError::Voice(format!("Failed to read audio response: {}", e)))?;

        tracing::debug!("Received {} bytes of MP3 audio data", audio_data.len());

        self.play_mp3(&audio_data)?;

        tracing::debug!("Voice playback completed");
        Ok(true)
    }

    fn estimate_cost(&self, char_count: usize) -> f64 {
        let per_char = if self.model_id.contains("multilingual") {
            COST_PER_CHAR_MULTILINGUAL
        } else {
            COST_PER_CHAR_FLASH
        };
        char_count as f64 * per_char
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_creation_defaults() {
        let provider =
            ElevenLabsProvider::new("test-key".to_string(), None, None, None, None, None, 100);
        assert_eq!(provider.name(), "elevenlabs");
        assert_eq!(provider.voice_id, DEFAULT_VOICE_ID);
        assert_eq!(provider.model_id, DEFAULT_MODEL_ID);
        assert_eq!(provider.volume, 100);
        assert!(provider.is_available());
    }

    #[test]
    fn test_provider_custom_voice_and_model() {
        let provider = ElevenLabsProvider::new(
            "test-key".to_string(),
            Some("JBFqnCBsd6RMkjVDRZzb".to_string()),
            Some("eleven_multilingual_v2".to_string()),
            Some(0.85),
            None,
            None,
            75,
        );
        assert_eq!(provider.voice_id, "JBFqnCBsd6RMkjVDRZzb");
        assert_eq!(provider.model_id, "eleven_multilingual_v2");
        assert_eq!(provider.speed, Some(0.85));
        assert_eq!(provider.volume, 75);
    }

    #[test]
    fn test_speed_clamped_to_valid_range() {
        let too_slow =
            ElevenLabsProvider::new("k".to_string(), None, None, Some(0.5), None, None, 100);
        assert_eq!(too_slow.speed, Some(0.7));
        let too_fast =
            ElevenLabsProvider::new("k".to_string(), None, None, Some(2.0), None, None, 100);
        assert_eq!(too_fast.speed, Some(1.2));
    }

    #[test]
    fn test_unavailable_with_empty_or_placeholder_key() {
        let empty = ElevenLabsProvider::new(String::new(), None, None, None, None, None, 100);
        assert!(!empty.is_available());

        let placeholder = ElevenLabsProvider::new(
            "${ELEVENLABS_API_KEY}".to_string(),
            None,
            None,
            None,
            None,
            None,
            100,
        );
        assert!(!placeholder.is_available());
    }

    #[test]
    fn test_cost_estimation_flash() {
        let provider =
            ElevenLabsProvider::new("test-key".to_string(), None, None, None, None, None, 100);
        // 1M chars × $0.00006 = $60
        let cost = provider.estimate_cost(1_000_000);
        assert!((cost - 60.0).abs() < 0.001);
    }

    #[test]
    fn test_cost_estimation_multilingual() {
        let provider = ElevenLabsProvider::new(
            "test-key".to_string(),
            None,
            Some("eleven_multilingual_v2".to_string()),
            None,
            None,
            None,
            100,
        );
        // 1M chars × $0.00012 = $120
        let cost = provider.estimate_cost(1_000_000);
        assert!((cost - 120.0).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_speak_empty_message() {
        let provider =
            ElevenLabsProvider::new("test-key".to_string(), None, None, None, None, None, 100);
        let result = provider.speak("").await.unwrap();
        assert!(!result);
    }
}
