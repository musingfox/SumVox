// Shared HTTP client construction for every provider

use std::time::Duration;

use reqwest::Client;

use crate::error::{Result, VoiceError};

const TTS_TIMEOUT: Duration = Duration::from_secs(30);

/// A client with system proxy detection off: on macOS it can crash inside
/// CoreFoundation, so every provider must build its client through here.
pub fn client(timeout: Duration) -> reqwest::Result<Client> {
    Client::builder().no_proxy().timeout(timeout).build()
}

/// Client for TTS providers, whose synthesis calls are slower than LLM requests.
pub fn tts_client() -> Result<Client> {
    client(TTS_TIMEOUT)
        .map_err(|e| VoiceError::Voice(format!("Failed to create HTTP client: {}", e)))
}
