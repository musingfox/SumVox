// Google Cloud Text-to-Speech API provider (Gemini 2.5 Flash TTS)

use async_trait::async_trait;
use base64::Engine;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use super::TtsProvider;
use crate::error::{Result, VoiceError};

/// Google Cloud TTS API endpoint
const GOOGLE_TTS_API: &str = "https://texttospeech.googleapis.com/v1/text:synthesize";

/// Cost per character for Gemini TTS
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

/// Google Cloud TTS provider using Gemini 2.5 Flash TTS
pub struct GoogleTtsProvider {
    project_id: String,
    voice_name: String,
}

#[derive(Debug, Serialize)]
struct TtsRequest {
    input: TtsInput,
    voice: TtsVoice,
    #[serde(rename = "audioConfig")]
    audio_config: AudioConfig,
}

#[derive(Debug, Serialize)]
struct TtsInput {
    text: String,
    prompt: String,
}

#[derive(Debug, Serialize)]
struct TtsVoice {
    #[serde(rename = "languageCode")]
    language_code: String,
    name: String,
    model_name: String,
}

#[derive(Debug, Serialize)]
struct AudioConfig {
    #[serde(rename = "audioEncoding")]
    audio_encoding: String,
    #[serde(rename = "speakingRate")]
    speaking_rate: f32,
    pitch: f32,
}

#[derive(Debug, Deserialize)]
struct TtsResponse {
    #[serde(rename = "audioContent")]
    audio_content: String, // Base64 encoded MP3
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
    pub fn new(project_id: String, voice_name: Option<String>) -> Self {
        let voice = voice_name.unwrap_or_else(|| "Aoede".to_string());

        Self {
            project_id,
            voice_name: voice,
        }
    }

    /// Create HTTP client lazily (avoids issues in parallel tests)
    fn create_client() -> Result<Client> {
        Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| crate::error::VoiceError::Voice(format!("Failed to create HTTP client: {}", e)))
    }

    /// Create provider from environment variable
    pub fn from_env(voice_name: Option<String>) -> Option<Self> {
        std::env::var("GOOGLE_CLOUD_PROJECT")
            .ok()
            .filter(|k| !k.is_empty())
            .map(|project_id| Self::new(project_id, voice_name))
    }

    /// Get access token using gcloud CLI
    fn get_access_token() -> Result<String> {
        use std::process::Command;

        let output = Command::new("gcloud")
            .args(["auth", "application-default", "print-access-token"])
            .output()
            .map_err(|e| VoiceError::Voice(format!("Failed to run gcloud: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(VoiceError::Voice(format!(
                "gcloud auth failed: {}. Run 'gcloud auth application-default login' first.",
                stderr.trim()
            )));
        }

        let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(token)
    }

    /// Play LINEAR16 audio data using rodio
    fn play_audio(&self, audio_data: &[u8]) -> Result<()> {
        use rodio::{buffer::SamplesBuffer, OutputStream, Sink};

        // Convert bytes to i16 samples (LINEAR16 = signed 16-bit little-endian)
        let samples: Vec<i16> = audio_data
            .chunks_exact(2)
            .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
            .collect();

        // Create output stream
        let (_stream, stream_handle) = OutputStream::try_default()
            .map_err(|e| VoiceError::Voice(format!("Failed to open audio output: {}", e)))?;

        // Create sink for playback
        let sink = Sink::try_new(&stream_handle)
            .map_err(|e| VoiceError::Voice(format!("Failed to create audio sink: {}", e)))?;

        // Create samples buffer (24kHz mono, as per Google TTS)
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
        !self.project_id.is_empty()
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

        // Build request using Gemini 2.5 Flash TTS
        let request = TtsRequest {
            input: TtsInput {
                text: text.to_string(),
                prompt: "Read aloud in a professional and kind tone.".to_string(),
            },
            voice: TtsVoice {
                language_code: "cmn-tw".to_string(),
                name: self.voice_name.clone(),
                model_name: "gemini-2.5-flash-tts".to_string(),
            },
            audio_config: AudioConfig {
                audio_encoding: "LINEAR16".to_string(),
                speaking_rate: 1.0,
                pitch: 0.0,
            },
        };

        // Get access token from gcloud
        let access_token = Self::get_access_token()?;

        // Create client and make API call
        let client = Self::create_client()?;

        let response = client
            .post(GOOGLE_TTS_API)
            .header("Authorization", format!("Bearer {}", access_token))
            .header("x-goog-user-project", &self.project_id)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| VoiceError::Voice(format!("Google TTS API request failed: {}", e)))?;

        let status = response.status();

        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();

            // Try to parse error response
            if let Ok(error) = serde_json::from_str::<TtsError>(&error_text) {
                return Err(VoiceError::Voice(format!(
                    "Google TTS API error ({}): {}",
                    status, error.error.message
                )));
            }

            return Err(VoiceError::Voice(format!(
                "Google TTS API error ({}): {}",
                status, error_text
            )));
        }

        // Parse response
        let tts_response: TtsResponse = response.json().await.map_err(|e| {
            VoiceError::Voice(format!("Failed to parse Google TTS response: {}", e))
        })?;

        // Decode base64 audio
        let audio_data = base64::engine::general_purpose::STANDARD
            .decode(&tts_response.audio_content)
            .map_err(|e| VoiceError::Voice(format!("Failed to decode audio: {}", e)))?;

        tracing::debug!("Received {} bytes of audio data", audio_data.len());

        // Play audio (blocking)
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
    fn test_google_provider_creation() {
        let provider = GoogleTtsProvider::new("test-project".to_string(), None);
        assert_eq!(provider.name(), "google");
        assert_eq!(provider.voice_name, "Aoede");
        assert!(provider.is_available());
    }

    #[test]
    fn test_custom_voice() {
        let provider =
            GoogleTtsProvider::new("test-project".to_string(), Some("Charon".to_string()));
        assert_eq!(provider.voice_name, "Charon");
    }

    #[test]
    fn test_empty_project_id() {
        let provider = GoogleTtsProvider::new(String::new(), None);
        assert!(!provider.is_available());
    }

    #[test]
    fn test_cost_estimation() {
        let provider = GoogleTtsProvider::new("project".to_string(), None);

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
        let provider = GoogleTtsProvider::new("test-project".to_string(), None);
        let result = provider.speak("").await.unwrap();
        assert!(!result);
    }

    // Integration test - requires gcloud CLI and GOOGLE_CLOUD_PROJECT
    #[tokio::test]
    #[ignore]
    async fn test_speak_integration() {
        let provider = GoogleTtsProvider::from_env(None);
        if let Some(p) = provider {
            let result = p.speak("測試語音").await;
            assert!(result.is_ok());
        }
    }
}
