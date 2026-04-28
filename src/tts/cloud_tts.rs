// Google Cloud Text-to-Speech provider using service account authentication
// Supports LINEAR16 audio with volume control via afplay

use async_trait::async_trait;
use base64::Engine;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use super::TtsProvider;
use crate::error::{Result, VoiceError};
use crate::tts::cloud_tts_auth::CloudTtsAuth;

const API_ENDPOINT: &str = "https://texttospeech.googleapis.com/v1/text:synthesize";
const COST_PER_CHAR: f64 = 0.000004; // $4 per 1M chars (Standard voices)
const MAX_TEXT_BYTES: usize = 5000;

/// Google Cloud TTS provider
pub struct CloudTtsProvider {
    auth: CloudTtsAuth,
    service_account_json: String,
    voice: String,
    language_code: String,
    volume: u32,
}

#[derive(Debug, Serialize)]
struct TtsRequest {
    input: TextInput,
    voice: VoiceSelection,
    #[serde(rename = "audioConfig")]
    audio_config: AudioConfig,
}

#[derive(Debug, Serialize)]
struct TextInput {
    text: String,
}

#[derive(Debug, Serialize)]
struct VoiceSelection {
    #[serde(rename = "languageCode")]
    language_code: String,
    name: String,
}

#[derive(Debug, Serialize)]
struct AudioConfig {
    #[serde(rename = "audioEncoding")]
    audio_encoding: String,
}

#[derive(Debug, Deserialize)]
struct TtsResponse {
    #[serde(rename = "audioContent")]
    audio_content: String,
}

impl CloudTtsProvider {
    pub fn new(
        service_account_json: String,
        voice: Option<String>,
        language_code: Option<String>,
        volume: u32,
    ) -> Self {
        let lang_code = language_code.clone().unwrap_or_else(|| "en-US".to_string());

        // Determine default voice based on language code
        // Google Cloud TTS uses "cmn-TW" for Mandarin (Taiwan), not "zh-TW"
        let default_voice = if lang_code.starts_with("cmn") || lang_code.starts_with("zh") {
            format!("{}-Standard-A", lang_code)
        } else {
            "en-US-Standard-A".to_string()
        };

        let voice_name = voice.unwrap_or(default_voice);

        Self {
            auth: CloudTtsAuth::new(service_account_json.clone()),
            service_account_json,
            voice: voice_name,
            language_code: lang_code,
            volume,
        }
    }

    /// Create HTTP client
    fn create_client() -> Result<Client> {
        Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| VoiceError::Voice(format!("Failed to create HTTP client: {}", e)))
    }

    /// Split text into chunks at sentence boundaries
    fn split_text(text: &str) -> Vec<String> {
        if text.len() <= MAX_TEXT_BYTES {
            return vec![text.to_string()];
        }

        let mut chunks = Vec::new();
        let mut current = String::new();

        for sentence in text.split_inclusive(&['.', '!', '?', '。', '!', '?']) {
            if current.len() + sentence.len() > MAX_TEXT_BYTES && !current.is_empty() {
                chunks.push(current.clone());
                current.clear();
            }
            current.push_str(sentence);
        }

        if !current.is_empty() {
            chunks.push(current);
        }

        chunks
    }

    /// Synthesize single text chunk
    async fn synthesize_chunk(&self, text: &str) -> Result<Vec<u8>> {
        let token = self.auth.get_token().await?;

        let request = TtsRequest {
            input: TextInput {
                text: text.to_string(),
            },
            voice: VoiceSelection {
                language_code: self.language_code.clone(),
                name: self.voice.clone(),
            },
            audio_config: AudioConfig {
                audio_encoding: "LINEAR16".to_string(),
            },
        };

        let client = Self::create_client()?;
        let response = client
            .post(API_ENDPOINT)
            .bearer_auth(&token)
            .json(&request)
            .send()
            .await
            .map_err(|e| VoiceError::Voice(format!("Cloud TTS API request failed: {}", e)))?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(VoiceError::Voice(format!(
                "Cloud TTS API error ({}): {}",
                status, error_text
            )));
        }

        let tts_response: TtsResponse = response
            .json()
            .await
            .map_err(|e| VoiceError::Voice(format!("Failed to parse Cloud TTS response: {}", e)))?;

        // Decode base64 audio
        let audio_data = base64::engine::general_purpose::STANDARD
            .decode(&tts_response.audio_content)
            .map_err(|e| VoiceError::Voice(format!("Failed to decode audio: {}", e)))?;

        Ok(audio_data)
    }

    /// Play audio data using afplay
    fn play_audio(&self, audio_data: &[u8]) -> Result<()> {
        use crate::audio::afplay::play_with_afplay;

        tracing::debug!(
            "Playing audio: {} bytes, volume: {}",
            audio_data.len(),
            self.volume
        );

        // Cloud TTS LINEAR16 response already includes WAV header
        play_with_afplay(audio_data, self.volume, "sumvox_cloud_tts")
    }
}

#[async_trait]
impl TtsProvider for CloudTtsProvider {
    fn name(&self) -> &str {
        "cloud_tts"
    }

    fn is_available(&self) -> bool {
        !self.service_account_json.is_empty()
    }

    async fn speak(&self, text: &str) -> Result<bool> {
        if text.trim().is_empty() {
            tracing::warn!("Empty message, skipping voice notification");
            return Ok(false);
        }

        tracing::info!(
            "Speaking with Cloud TTS: voice={}, language={}, chars={}",
            self.voice,
            self.language_code,
            text.len()
        );

        // Split text if needed
        let chunks = Self::split_text(text);
        let mut audio_chunks = Vec::new();

        // Synthesize each chunk
        for chunk in chunks {
            let audio = self.synthesize_chunk(&chunk).await?;
            audio_chunks.push(audio);
        }

        // Concatenate and play audio
        let mut combined_audio = Vec::new();
        for audio in audio_chunks {
            combined_audio.extend(audio);
        }

        self.play_audio(&combined_audio)?;

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

    fn create_test_provider() -> CloudTtsProvider {
        CloudTtsProvider::new(
            r#"{"client_email":"test@test.com","private_key":"key"}"#.to_string(),
            None,
            None,
            100,
        )
    }

    #[test]
    fn test_provider_name() {
        let p = create_test_provider();
        assert_eq!(p.name(), "cloud_tts");
    }

    #[test]
    fn test_is_available_with_json() {
        let p = create_test_provider();
        assert!(p.is_available());
    }

    #[test]
    fn test_is_available_empty() {
        let p = CloudTtsProvider::new(String::new(), None, None, 100);
        assert!(!p.is_available());
    }

    #[tokio::test]
    async fn test_speak_empty_text() {
        let p = create_test_provider();
        assert!(!p.speak("").await.unwrap());
    }

    #[test]
    fn test_cost_estimation() {
        let p = create_test_provider();
        let cost = p.estimate_cost(1_000_000);
        assert!((cost - 4.0).abs() < 0.01);
    }

    #[test]
    fn test_default_voice_zh() {
        let p = CloudTtsProvider::new("sa".into(), None, Some("cmn-TW".into()), 100);
        assert_eq!(p.voice, "cmn-TW-Standard-A");
        assert_eq!(p.language_code, "cmn-TW");
    }

    #[test]
    fn test_default_voice_en() {
        let p = CloudTtsProvider::new("sa".into(), None, None, 100);
        assert_eq!(p.voice, "en-US-Standard-A");
        assert_eq!(p.language_code, "en-US");
    }

    #[test]
    fn test_custom_voice() {
        let p = CloudTtsProvider::new(
            "sa".into(),
            Some("zh-TW-Wavenet-B".into()),
            Some("zh-TW".into()),
            100,
        );
        assert_eq!(p.voice, "zh-TW-Wavenet-B");
        assert_eq!(p.language_code, "zh-TW");
    }

    #[test]
    fn test_split_text_short() {
        let text = "Short text.";
        let chunks = CloudTtsProvider::split_text(text);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], text);
    }

    #[test]
    fn test_split_text_long() {
        let sentence = "A".repeat(3000);
        let text = format!("{}. {}. {}", sentence, sentence, sentence);
        let chunks = CloudTtsProvider::split_text(&text);
        assert!(chunks.len() >= 2);
    }
}
