// ElevenLabs Text-to-Speech provider
// Docs: https://elevenlabs.io/docs/api-reference/text-to-speech/convert
// Pricing: $0.05 / 1K chars (Flash v2.5), $0.10 / 1K chars (Multilingual v2/v3)

use async_trait::async_trait;
use reqwest::Client;
use serde::Serialize;

use super::TtsProvider;
use crate::error::{Result, VoiceError};

const ELEVENLABS_API_BASE: &str = "https://api.elevenlabs.io/v1/text-to-speech";

/// Default output format — MP3 44.1kHz 128kbps (free tier)
const DEFAULT_OUTPUT_FORMAT: &str = "mp3_44100_128";

/// Maximum characters per request (ElevenLabs limit)
const MAX_TEXT_LENGTH: usize = 5_000;

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
        voice_id: String,
        model_id: String,
        speed: Option<f32>,
        stability: Option<f32>,
        style: Option<f32>,
        volume: u32,
    ) -> Self {
        Self {
            api_key,
            voice_id,
            model_id,
            output_format: DEFAULT_OUTPUT_FORMAT.to_string(),
            speed: speed.map(|s| s.clamp(0.7, 1.2)),
            stability: stability.map(|s| s.clamp(0.0, 1.0)),
            style: style.map(|s| s.clamp(0.0, 1.0)),
            volume,
        }
    }

    fn create_client() -> Result<Client> {
        crate::http::tts_client()
    }

    fn play_audio(&self, audio_data: &[u8]) -> Result<()> {
        // ElevenLabs output isn't loudness-normalized, so volume swings between
        // (and within) generations. Even it out before playback; fall back to
        // the raw MP3 when ffmpeg isn't installed.
        if let Some(wav) =
            crate::audio::normalize::normalize_to_wav(audio_data, "sumvox_elevenlabs")
        {
            tracing::debug!(
                "Playing loudness-normalized ElevenLabs audio: {} bytes, volume: {}",
                wav.len(),
                self.volume
            );
            return crate::audio::player::play_bytes(&wav, self.volume, "sumvox_elevenlabs");
        }

        tracing::debug!(
            "Playing ElevenLabs audio without normalization: {} bytes, volume: {}",
            audio_data.len(),
            self.volume
        );

        let tmp_path =
            crate::audio::player::write_temp_audio("sumvox_elevenlabs", "mp3", audio_data)?;

        // Capture the result before cleanup so the temp file is removed on
        // every path, including a spawn failure (the pre-refactor `?` skipped it).
        let result = crate::audio::player::play_file(&tmp_path, self.volume);
        let _ = std::fs::remove_file(&tmp_path);
        result
    }
}

#[async_trait]
impl TtsProvider for ElevenLabsProvider {
    fn name(&self) -> &str {
        "elevenlabs"
    }

    fn is_available(&self) -> bool {
        crate::config::is_usable_key(&self.api_key)
    }

    fn supports_audio_tags(&self) -> bool {
        true
    }

    async fn speak(&self, text: &str) -> Result<bool> {
        if text.trim().is_empty() {
            tracing::warn!("Empty message, skipping voice notification");
            return Ok(false);
        }

        let text = if text.len() > MAX_TEXT_LENGTH {
            tracing::warn!(
                "Text exceeds {} bytes, truncating to limit",
                MAX_TEXT_LENGTH
            );
            truncate_to_limit(text, MAX_TEXT_LENGTH)
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

        self.play_audio(&audio_data)?;

        tracing::debug!("Voice playback completed");
        Ok(true)
    }
}

fn truncate_to_limit(text: &str, limit: usize) -> &str {
    &text[..text.floor_char_boundary(limit)]
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_truncate_to_limit_respects_char_boundary() {
        let text = format!("a{}", "你好".repeat(MAX_TEXT_LENGTH));
        let out = truncate_to_limit(&text, MAX_TEXT_LENGTH);
        assert!(out.len() <= MAX_TEXT_LENGTH);
        assert!(!out.is_empty());
    }

    use super::*;

    #[test]
    fn test_speed_clamped_to_valid_range() {
        let too_slow = ElevenLabsProvider::new(
            "k".to_string(),
            "21m00Tcm4TlvDq8ikWAM".to_string(),
            "eleven_flash_v2_5".to_string(),
            Some(0.5),
            None,
            None,
            100,
        );
        assert_eq!(too_slow.speed, Some(0.7));
        let too_fast = ElevenLabsProvider::new(
            "k".to_string(),
            "21m00Tcm4TlvDq8ikWAM".to_string(),
            "eleven_flash_v2_5".to_string(),
            Some(2.0),
            None,
            None,
            100,
        );
        assert_eq!(too_fast.speed, Some(1.2));
    }

    #[test]
    fn test_unavailable_with_empty_or_placeholder_key() {
        let empty = ElevenLabsProvider::new(
            String::new(),
            "21m00Tcm4TlvDq8ikWAM".to_string(),
            "eleven_flash_v2_5".to_string(),
            None,
            None,
            None,
            100,
        );
        assert!(!empty.is_available());

        let placeholder = ElevenLabsProvider::new(
            "${ELEVENLABS_API_KEY}".to_string(),
            "21m00Tcm4TlvDq8ikWAM".to_string(),
            "eleven_flash_v2_5".to_string(),
            None,
            None,
            None,
            100,
        );
        assert!(!placeholder.is_available());
    }

    #[tokio::test]
    async fn test_speak_empty_message() {
        let provider = ElevenLabsProvider::new(
            "test-key".to_string(),
            "21m00Tcm4TlvDq8ikWAM".to_string(),
            "eleven_flash_v2_5".to_string(),
            None,
            None,
            None,
            100,
        );
        let result = provider.speak("").await.unwrap();
        assert!(!result);
    }
}
