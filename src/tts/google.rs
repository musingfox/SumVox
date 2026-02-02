// Gemini 2.5 Flash TTS provider using Google AI Studio API

use async_trait::async_trait;
use base64::Engine;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use super::TtsProvider;
use crate::error::{Result, VoiceError};

/// Gemini TTS API endpoint
const GEMINI_TTS_API: &str =
    "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash-preview-tts:generateContent";

/// Cost per character for Gemini TTS (estimated)
const COST_PER_CHAR: f64 = 0.000016;

/// Available Gemini TTS voices
pub const GEMINI_TTS_VOICES: &[&str] = &[
    "Aoede",   // Default
    "Charon",
    "Fenrir",
    "Kore",
    "Puck",
    "Orus",
];

/// Gemini TTS provider using Google AI Studio API
pub struct GoogleTtsProvider {
    api_key: String,
    voice_name: String,
    volume: u32,
}

#[derive(Debug, Serialize)]
struct GeminiTtsRequest {
    contents: Vec<Content>,
    #[serde(rename = "generationConfig")]
    generation_config: GenerationConfig,
}

#[derive(Debug, Serialize)]
struct Content {
    parts: Vec<Part>,
}

#[derive(Debug, Serialize)]
struct Part {
    text: String,
}

#[derive(Debug, Serialize)]
struct GenerationConfig {
    #[serde(rename = "responseModalities")]
    response_modalities: Vec<String>,
    #[serde(rename = "speechConfig")]
    speech_config: SpeechConfig,
}

#[derive(Debug, Serialize)]
struct SpeechConfig {
    #[serde(rename = "voiceConfig")]
    voice_config: VoiceConfig,
}

#[derive(Debug, Serialize)]
struct VoiceConfig {
    #[serde(rename = "prebuiltVoiceConfig")]
    prebuilt_voice_config: PrebuiltVoiceConfig,
}

#[derive(Debug, Serialize)]
struct PrebuiltVoiceConfig {
    #[serde(rename = "voiceName")]
    voice_name: String,
}

#[derive(Debug, Deserialize)]
struct GeminiTtsResponse {
    candidates: Vec<Candidate>,
}

#[derive(Debug, Deserialize)]
struct Candidate {
    content: ResponseContent,
}

#[derive(Debug, Deserialize)]
struct ResponseContent {
    parts: Vec<ResponsePart>,
}

#[derive(Debug, Deserialize)]
struct ResponsePart {
    #[serde(rename = "inlineData", default)]
    inline_data: Option<InlineData>,
}

#[derive(Debug, Deserialize)]
struct InlineData {
    #[serde(rename = "mimeType")]
    mime_type: String,
    data: String, // Base64 encoded audio
}

#[derive(Debug, Deserialize)]
struct TtsError {
    error: TtsErrorDetail,
}

#[derive(Debug, Deserialize)]
struct TtsErrorDetail {
    message: String,
}

impl GoogleTtsProvider {
    pub fn new(api_key: String, voice_name: Option<String>, volume: u32) -> Self {
        let voice = voice_name.unwrap_or_else(|| "Aoede".to_string());

        Self {
            api_key,
            voice_name: voice,
            volume,
        }
    }

    /// Create HTTP client lazily (avoids issues in parallel tests)
    fn create_client() -> Result<Client> {
        Client::builder()
            .no_proxy() // Disable system proxy detection to avoid CoreFoundation crash
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| crate::error::VoiceError::Voice(format!("Failed to create HTTP client: {}", e)))
    }

    /// Create provider from environment variable
    pub fn from_env(voice_name: Option<String>, volume: u32) -> Option<Self> {
        std::env::var("GEMINI_API_KEY")
            .ok()
            .or_else(|| std::env::var("GOOGLE_API_KEY").ok())
            .filter(|k| !k.is_empty())
            .map(|api_key| Self::new(api_key, voice_name, volume))
    }

    /// Play audio data using rodio
    fn play_audio(&self, audio_data: &[u8], mime_type: &str) -> Result<()> {
        use rodio::{buffer::SamplesBuffer, OutputStream, Sink};

        tracing::debug!(
            "Playing audio: {} bytes, mime_type: {}, volume: {}",
            audio_data.len(),
            mime_type,
            self.volume
        );

        // Create output stream
        let (_stream, stream_handle) = OutputStream::try_default()
            .map_err(|e| VoiceError::Voice(format!("Failed to open audio output: {}", e)))?;

        // Create sink for playback
        let sink = Sink::try_new(&stream_handle)
            .map_err(|e| VoiceError::Voice(format!("Failed to create audio sink: {}", e)))?;

        // Set volume (0-100 to 0.0-1.0)
        sink.set_volume(self.volume as f32 / 100.0);

        // Gemini TTS returns LINEAR16 PCM format (16-bit signed little-endian at 24kHz)
        // Convert bytes to i16 samples
        let samples: Vec<i16> = audio_data
            .chunks_exact(2)
            .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
            .collect();

        tracing::debug!("Converted {} samples for playback", samples.len());

        // Create samples buffer (24kHz mono, as per Gemini TTS)
        let source = SamplesBuffer::new(1, 24000, samples);

        // Play and wait for completion
        sink.append(source);
        sink.sleep_until_end();

        Ok(())
    }
}

#[async_trait]
impl TtsProvider for GoogleTtsProvider {
    fn name(&self) -> &str {
        "google"
    }

    fn is_available(&self) -> bool {
        !self.api_key.is_empty()
    }

    async fn speak(&self, text: &str) -> Result<bool> {
        if text.trim().is_empty() {
            tracing::warn!("Empty message, skipping voice notification");
            return Ok(false);
        }

        tracing::info!(
            "Speaking with Gemini TTS: voice={}, chars={}",
            self.voice_name,
            text.len()
        );

        // Build request using Gemini 2.5 Flash TTS API format
        // IMPORTANT: Must include TTS instruction prefix for the model to generate audio
        let tts_text = format!("Read this aloud: {}", text);

        let request = GeminiTtsRequest {
            contents: vec![Content {
                parts: vec![Part {
                    text: tts_text,
                }],
            }],
            generation_config: GenerationConfig {
                response_modalities: vec!["AUDIO".to_string()],
                speech_config: SpeechConfig {
                    voice_config: VoiceConfig {
                        prebuilt_voice_config: PrebuiltVoiceConfig {
                            voice_name: self.voice_name.clone(),
                        },
                    },
                },
            },
        };

        // Create client and make API call
        let client = Self::create_client()?;

        let response = client
            .post(GEMINI_TTS_API)
            .header("x-goog-api-key", &self.api_key)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| VoiceError::Voice(format!("Gemini TTS API request failed: {}", e)))?;

        let status = response.status();

        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();

            // Try to parse error response
            if let Ok(error) = serde_json::from_str::<TtsError>(&error_text) {
                return Err(VoiceError::Voice(format!(
                    "Gemini TTS API error ({}): {}",
                    status, error.error.message
                )));
            }

            return Err(VoiceError::Voice(format!(
                "Gemini TTS API error ({}): {}",
                status, error_text
            )));
        }

        // Parse response
        let tts_response: GeminiTtsResponse = response.json().await.map_err(|e| {
            VoiceError::Voice(format!("Failed to parse Gemini TTS response: {}", e))
        })?;

        // Extract audio data from response
        let inline_data = tts_response
            .candidates
            .get(0)
            .and_then(|c| c.content.parts.get(0))
            .and_then(|p| p.inline_data.as_ref())
            .ok_or_else(|| VoiceError::Voice("No audio data in response".into()))?;

        // Decode base64 audio
        let audio_data = base64::engine::general_purpose::STANDARD
            .decode(&inline_data.data)
            .map_err(|e| VoiceError::Voice(format!("Failed to decode audio: {}", e)))?;

        tracing::debug!(
            "Received {} bytes of audio data ({})",
            audio_data.len(),
            inline_data.mime_type
        );

        // Play audio (blocking)
        self.play_audio(&audio_data, &inline_data.mime_type)?;

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
    fn test_google_provider_creation() {
        let provider = GoogleTtsProvider::new("test-api-key".to_string(), None, 100);
        assert_eq!(provider.name(), "google");
        assert_eq!(provider.voice_name, "Aoede");
        assert_eq!(provider.volume, 100);
        assert!(provider.is_available());
    }

    #[test]
    fn test_custom_voice() {
        let provider =
            GoogleTtsProvider::new("test-api-key".to_string(), Some("Charon".to_string()), 75);
        assert_eq!(provider.voice_name, "Charon");
        assert_eq!(provider.volume, 75);
    }

    #[test]
    fn test_empty_api_key() {
        let provider = GoogleTtsProvider::new(String::new(), None, 100);
        assert!(!provider.is_available());
    }

    #[test]
    fn test_cost_estimation() {
        let provider = GoogleTtsProvider::new("test-api-key".to_string(), None, 100);

        // 50 chars (typical summary length)
        let cost_50 = provider.estimate_cost(50);
        assert!((cost_50 - 0.0008).abs() < 0.0001);

        // 100 chars
        let cost_100 = provider.estimate_cost(100);
        assert!((cost_100 - 0.0016).abs() < 0.0001);
    }

    #[test]
    fn test_gemini_tts_voices() {
        assert!(GEMINI_TTS_VOICES.len() >= 6);
        assert!(GEMINI_TTS_VOICES.contains(&"Aoede"));
        assert!(GEMINI_TTS_VOICES.contains(&"Charon"));
        assert!(GEMINI_TTS_VOICES.contains(&"Kore"));
    }

    #[tokio::test]
    async fn test_speak_empty_message() {
        let provider = GoogleTtsProvider::new("test-api-key".to_string(), None, 100);
        let result = provider.speak("").await.unwrap();
        assert!(!result);
    }

    // Integration test - requires valid GEMINI_API_KEY
    #[tokio::test]
    #[ignore]
    async fn test_speak_integration() {
        let provider = GoogleTtsProvider::from_env(None, 100);
        if let Some(p) = provider {
            let result = p.speak("測試語音").await;
            assert!(result.is_ok());
        }
    }
}
