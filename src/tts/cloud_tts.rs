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
// Gemini-TTS caps input `text` at 4000 bytes (prompt is billed separately).
const MAX_TEXT_BYTES_GEMINI: usize = 4000;
// Gemini-TTS is billed per audio token, not per character. Rough estimate:
//   $10 / 1M audio tokens × 25 tokens/sec ÷ ~15 chars/sec ≈ 0.0000167 / char.
// Coarse approximation — real cost depends on synthesized audio duration.
const COST_PER_CHAR_GEMINI: f64 = 0.0000167;

/// Google Cloud TTS provider
pub struct CloudTtsProvider {
    auth: CloudTtsAuth,
    service_account_json: String,
    voice: String,
    language_code: String,
    /// When set (e.g. "gemini-2.5-flash-tts"), switches to the Gemini-TTS
    /// request shape (bare voice name + model_name) and token-based cost/chunking.
    model: Option<String>,
    /// Optional Gemini-TTS style instruction, sent as `input.prompt`.
    style_prompt: Option<String>,
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
    /// Gemini-TTS style instruction. Omitted from the wire for traditional voices.
    #[serde(skip_serializing_if = "Option::is_none")]
    prompt: Option<String>,
}

#[derive(Debug, Serialize)]
struct VoiceSelection {
    #[serde(rename = "languageCode")]
    language_code: String,
    name: String,
    /// Gemini-TTS model. Omitted from the wire for traditional voices.
    #[serde(rename = "modelName", skip_serializing_if = "Option::is_none")]
    model_name: Option<String>,
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
        voice: String,
        language_code: Option<String>,
        model: Option<String>,
        style_prompt: Option<String>,
        volume: u32,
    ) -> Self {
        // language_code is a neutral tuning value: unset = en-US.
        let lang_code = language_code.unwrap_or_else(|| "en-US".to_string());

        Self {
            auth: CloudTtsAuth::new(service_account_json.clone()),
            service_account_json,
            voice,
            language_code: lang_code,
            model,
            style_prompt,
            volume,
        }
    }

    /// Whether this provider is configured for a Gemini-TTS model.
    fn is_gemini(&self) -> bool {
        self.model
            .as_deref()
            .is_some_and(|m| m.to_lowercase().contains("gemini"))
    }

    /// Byte cap for a single synthesis chunk (Gemini-TTS is stricter).
    fn max_chunk_bytes(&self) -> usize {
        if self.model.is_some() {
            MAX_TEXT_BYTES_GEMINI
        } else {
            MAX_TEXT_BYTES
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

    /// Split text into chunks at sentence boundaries, capped at `max_bytes`.
    fn split_text(text: &str, max_bytes: usize) -> Vec<String> {
        if text.len() <= max_bytes {
            return vec![text.to_string()];
        }

        let mut chunks = Vec::new();
        let mut current = String::new();

        for sentence in text.split_inclusive(&['.', '!', '?', '。', '!', '?']) {
            if current.len() + sentence.len() > max_bytes && !current.is_empty() {
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
                prompt: self.style_prompt.clone(),
            },
            voice: VoiceSelection {
                language_code: self.language_code.clone(),
                name: self.voice.clone(),
                model_name: self.model.clone(),
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
        let chunks = Self::split_text(text, self.max_chunk_bytes());
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
        // Gemini-TTS bills per audio token; use a coarse per-char proxy.
        // Traditional voices keep the exact $4/1M-char rate.
        let rate = if self.is_gemini() {
            COST_PER_CHAR_GEMINI
        } else {
            COST_PER_CHAR
        };
        char_count as f64 * rate
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_provider() -> CloudTtsProvider {
        CloudTtsProvider::new(
            r#"{"client_email":"test@test.com","private_key":"key"}"#.to_string(),
            "en-US-Standard-A".to_string(),
            None,
            None,
            None,
            100,
        )
    }

    fn create_gemini_provider() -> CloudTtsProvider {
        CloudTtsProvider::new(
            r#"{"client_email":"test@test.com","private_key":"key"}"#.to_string(),
            "Kore".to_string(),
            Some("en-US".to_string()),
            Some("gemini-2.5-flash-tts".to_string()),
            Some("Say the following in a curious way.".to_string()),
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
        let p = CloudTtsProvider::new(
            String::new(),
            "en-US-Standard-A".to_string(),
            None,
            None,
            None,
            100,
        );
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
    fn test_language_defaults_to_en_us_when_absent() {
        // voice is required and used verbatim; language_code is a neutral tuning
        // value, defaulting to en-US only when the config leaves it unset.
        let p = CloudTtsProvider::new(
            "sa".into(),
            "en-US-Standard-A".into(),
            None,
            None,
            None,
            100,
        );
        assert_eq!(p.voice, "en-US-Standard-A");
        assert_eq!(p.language_code, "en-US");
    }

    #[test]
    fn test_custom_voice() {
        let p = CloudTtsProvider::new(
            "sa".into(),
            "zh-TW-Wavenet-B".into(),
            Some("zh-TW".into()),
            None,
            None,
            100,
        );
        assert_eq!(p.voice, "zh-TW-Wavenet-B");
        assert_eq!(p.language_code, "zh-TW");
    }

    #[test]
    fn test_split_text_short() {
        let text = "Short text.";
        let chunks = CloudTtsProvider::split_text(text, MAX_TEXT_BYTES);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], text);
    }

    #[test]
    fn test_split_text_long() {
        let sentence = "A".repeat(3000);
        let text = format!("{}. {}. {}", sentence, sentence, sentence);
        let chunks = CloudTtsProvider::split_text(&text, MAX_TEXT_BYTES);
        assert!(chunks.len() >= 2);
    }

    #[test]
    fn test_default_request_serialization_omits_gemini_fields() {
        // With no model/style_prompt set, the wire format must be byte-identical
        // to the pre-Gemini request: no "modelName", no "prompt" keys.
        let request = TtsRequest {
            input: TextInput {
                text: "hello".to_string(),
                prompt: None,
            },
            voice: VoiceSelection {
                language_code: "en-US".to_string(),
                name: "en-US-Standard-A".to_string(),
                model_name: None,
            },
            audio_config: AudioConfig {
                audio_encoding: "LINEAR16".to_string(),
            },
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(!json.contains("modelName"), "unexpected modelName: {json}");
        assert!(!json.contains("prompt"), "unexpected prompt: {json}");
        assert_eq!(
            json,
            r#"{"input":{"text":"hello"},"voice":{"languageCode":"en-US","name":"en-US-Standard-A"},"audioConfig":{"audioEncoding":"LINEAR16"}}"#
        );
    }

    #[test]
    fn test_gemini_request_serialization_includes_fields() {
        let request = TtsRequest {
            input: TextInput {
                text: "hello".to_string(),
                prompt: Some("Say it curiously.".to_string()),
            },
            voice: VoiceSelection {
                language_code: "en-US".to_string(),
                name: "Kore".to_string(),
                model_name: Some("gemini-2.5-flash-tts".to_string()),
            },
            audio_config: AudioConfig {
                audio_encoding: "LINEAR16".to_string(),
            },
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains(r#""modelName":"gemini-2.5-flash-tts""#));
        assert!(json.contains(r#""prompt":"Say it curiously.""#));
    }

    #[test]
    fn test_gemini_chunk_cap_is_4000() {
        let p = create_gemini_provider();
        assert_eq!(p.max_chunk_bytes(), 4000);
    }

    #[test]
    fn test_traditional_chunk_cap_is_5000() {
        let p = create_test_provider();
        assert_eq!(p.max_chunk_bytes(), 5000);
    }

    #[test]
    fn test_gemini_cost_uses_token_estimate() {
        let p = create_gemini_provider();
        let cost = p.estimate_cost(1_000_000);
        // Coarse per-char proxy for token billing (~$16.7 / 1M chars).
        assert!((cost - 16.7).abs() < 0.1, "unexpected gemini cost: {cost}");
    }

    #[test]
    fn test_traditional_cost_unchanged() {
        let p = create_test_provider();
        let cost = p.estimate_cost(1_000_000);
        assert!((cost - 4.0).abs() < 0.01);
    }
}
