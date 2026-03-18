// xAI Text-to-Speech provider using the xAI TTS API
// Supports 5 voices (eve, ara, rex, sal, leo) with MP3 output

use async_trait::async_trait;
use reqwest::Client;
use serde::Serialize;
use std::io::Cursor;
use std::time::Duration;

use super::TtsProvider;
use crate::error::{Result, VoiceError};

/// xAI TTS API endpoint
const XAI_TTS_API_URL: &str = "https://api.x.ai/v1/tts";

/// Cost per character for xAI TTS ($4.20 / 1M characters)
const COST_PER_CHAR: f64 = 0.0000042;

/// Maximum text length per request
const MAX_TEXT_LENGTH: usize = 15_000;

/// xAI TTS provider
pub struct XaiTtsProvider {
    api_key: String,
    voice_id: String,
    language: String,
    volume: u32,
}

#[derive(Debug, Serialize)]
struct XaiTtsRequest {
    text: String,
    voice_id: String,
    language: String,
}

impl XaiTtsProvider {
    pub fn new(
        api_key: String,
        voice_id: Option<String>,
        language: Option<String>,
        volume: u32,
    ) -> Self {
        Self {
            api_key,
            voice_id: voice_id.unwrap_or_else(|| "eve".to_string()),
            language: language.unwrap_or_else(|| "auto".to_string()),
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

    fn play_audio(&self, audio_data: &[u8]) -> Result<()> {
        use rodio::{Decoder, OutputStream, Sink};

        tracing::debug!(
            "Playing xAI TTS audio: {} bytes, volume: {}",
            audio_data.len(),
            self.volume
        );

        let (_stream, stream_handle) = OutputStream::try_default()
            .map_err(|e| VoiceError::Voice(format!("Failed to open audio output: {}", e)))?;

        let sink = Sink::try_new(&stream_handle)
            .map_err(|e| VoiceError::Voice(format!("Failed to create audio sink: {}", e)))?;

        sink.set_volume(self.volume as f32 / 100.0);

        let cursor = Cursor::new(audio_data.to_vec());
        let source = Decoder::new(cursor)
            .map_err(|e| VoiceError::Voice(format!("Failed to decode MP3 audio: {}", e)))?;

        sink.append(source);
        sink.sleep_until_end();

        Ok(())
    }
}

#[async_trait]
impl TtsProvider for XaiTtsProvider {
    fn name(&self) -> &str {
        "xai"
    }

    fn is_available(&self) -> bool {
        !self.api_key.is_empty()
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
            "Speaking with xAI TTS: voice={}, language={}, chars={}",
            self.voice_id,
            self.language,
            text.len()
        );

        let request = XaiTtsRequest {
            text: text.to_string(),
            voice_id: self.voice_id.clone(),
            language: self.language.clone(),
        };

        let client = Self::create_client()?;

        let response = client
            .post(XAI_TTS_API_URL)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| VoiceError::Voice(format!("xAI TTS API request failed: {}", e)))?;

        let status = response.status();

        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(VoiceError::Voice(format!(
                "xAI TTS API error ({}): {}",
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

    fn estimate_cost(&self, char_count: usize) -> f64 {
        char_count as f64 * COST_PER_CHAR
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xai_provider_creation() {
        let provider = XaiTtsProvider::new("test-api-key".to_string(), None, None, 100);
        assert_eq!(provider.name(), "xai");
        assert_eq!(provider.voice_id, "eve");
        assert_eq!(provider.language, "auto");
        assert_eq!(provider.volume, 100);
        assert!(provider.is_available());
    }

    #[test]
    fn test_custom_voice_and_language() {
        let provider = XaiTtsProvider::new(
            "test-api-key".to_string(),
            Some("rex".to_string()),
            Some("zh".to_string()),
            75,
        );
        assert_eq!(provider.voice_id, "rex");
        assert_eq!(provider.language, "zh");
        assert_eq!(provider.volume, 75);
    }

    #[test]
    fn test_empty_api_key() {
        let provider = XaiTtsProvider::new(String::new(), None, None, 100);
        assert!(!provider.is_available());
    }

    #[test]
    fn test_cost_estimation() {
        let provider = XaiTtsProvider::new("test-api-key".to_string(), None, None, 100);

        // 1M characters = $4.20
        let cost_1m = provider.estimate_cost(1_000_000);
        assert!((cost_1m - 4.2).abs() < 0.001);

        // 100 characters
        let cost_100 = provider.estimate_cost(100);
        assert!((cost_100 - 0.00042).abs() < 0.00001);
    }

    #[tokio::test]
    async fn test_speak_empty_message() {
        let provider = XaiTtsProvider::new("test-api-key".to_string(), None, None, 100);
        let result = provider.speak("").await.unwrap();
        assert!(!result);
    }
}
