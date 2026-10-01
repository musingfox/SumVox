// OpenAI Text-to-Speech provider (gpt-4o-mini-tts)
// Docs: https://platform.openai.com/docs/api-reference/audio/createSpeech
// Returns raw MP3 bytes played through the platform player.

use async_trait::async_trait;
use reqwest::Client;
use serde::Serialize;

use super::TtsProvider;
use crate::error::{Result, VoiceError};

/// OpenAI speech synthesis endpoint
const OPENAI_TTS_API_URL: &str = "https://api.openai.com/v1/audio/speech";

/// Maximum input length per request (OpenAI limit: 4096 chars)
const MAX_TEXT_LENGTH: usize = 4_096;

/// OpenAI TTS provider
pub struct OpenAiTtsProvider {
    api_key: String,
    model: String,
    voice: String,
    instructions: Option<String>,
    speed: Option<f32>,
    volume: u32,
}

#[derive(Debug, Serialize)]
struct OpenAiTtsRequest {
    model: String,
    input: String,
    voice: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    speed: Option<f32>,
    response_format: String,
}

impl OpenAiTtsProvider {
    pub fn new(
        api_key: String,
        model: String,
        voice: String,
        instructions: Option<String>,
        speed: Option<f32>,
        volume: u32,
    ) -> Self {
        Self {
            api_key,
            model,
            voice,
            instructions,
            // OpenAI accepts 0.25-4.0 (1.0 default).
            speed: speed.map(|s| s.clamp(0.25, 4.0)),
            volume,
        }
    }

    fn create_client() -> Result<Client> {
        crate::http::tts_client()
    }

    fn play_audio(&self, audio_data: &[u8]) -> Result<()> {
        use crate::audio::player::play_bytes;

        tracing::debug!(
            "Playing OpenAI TTS audio: {} bytes, volume: {}",
            audio_data.len(),
            self.volume
        );

        play_bytes(audio_data, self.volume, "sumvox_openai")
    }
}

#[async_trait]
impl TtsProvider for OpenAiTtsProvider {
    fn name(&self) -> &str {
        "openai"
    }

    fn is_available(&self) -> bool {
        crate::config::is_usable_key(&self.api_key)
    }

    async fn speak(&self, text: &str) -> Result<bool> {
        if text.trim().is_empty() {
            tracing::warn!("Empty message, skipping voice notification");
            return Ok(false);
        }

        // OpenAI's limit is 4096 characters (not bytes); slice on a char
        // boundary so multibyte text (the primary Chinese use case) can't panic.
        let text = match text.char_indices().nth(MAX_TEXT_LENGTH) {
            Some((byte_idx, _)) => {
                tracing::warn!(
                    "Text exceeds {} chars, truncating to limit",
                    MAX_TEXT_LENGTH
                );
                &text[..byte_idx]
            }
            None => text,
        };

        tracing::info!(
            "Speaking with OpenAI TTS: model={}, voice={}, chars={}",
            self.model,
            self.voice,
            text.len()
        );

        let request = OpenAiTtsRequest {
            model: self.model.clone(),
            input: text.to_string(),
            voice: self.voice.clone(),
            instructions: self.instructions.clone(),
            speed: self.speed,
            response_format: "mp3".to_string(),
        };

        let client = Self::create_client()?;

        let response = client
            .post(OPENAI_TTS_API_URL)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| VoiceError::Voice(format!("OpenAI TTS API request failed: {}", e)))?;

        let status = response.status();

        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(VoiceError::Voice(format!(
                "OpenAI TTS API error ({}): {}",
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

#[cfg(test)]
mod tests {
    use super::*;

    fn provider(speed: Option<f32>) -> OpenAiTtsProvider {
        OpenAiTtsProvider::new(
            "test-api-key".to_string(),
            "gpt-4o-mini-tts".to_string(),
            "nova".to_string(),
            None,
            speed,
            100,
        )
    }

    #[test]
    fn test_unavailable_with_empty_or_placeholder_key() {
        let empty = OpenAiTtsProvider::new(
            String::new(),
            "gpt-4o-mini-tts".to_string(),
            "nova".to_string(),
            None,
            None,
            100,
        );
        assert!(!empty.is_available());

        let placeholder = OpenAiTtsProvider::new(
            "${OPENAI_API_KEY}".to_string(),
            "gpt-4o-mini-tts".to_string(),
            "nova".to_string(),
            None,
            None,
            100,
        );
        assert!(!placeholder.is_available());
    }

    #[test]
    fn test_speed_clamped_to_valid_range() {
        assert_eq!(provider(Some(5.0)).speed, Some(4.0));
        assert_eq!(provider(Some(0.1)).speed, Some(0.25));
        assert_eq!(provider(Some(1.5)).speed, Some(1.5));
        assert!(!provider(None).supports_audio_tags());
    }

    #[tokio::test]
    async fn test_speak_empty_message() {
        let p = provider(None);
        assert!(!p.speak("").await.unwrap());
        assert!(!p.speak("   ").await.unwrap());
    }
}
