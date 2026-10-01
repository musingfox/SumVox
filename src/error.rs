// Error types for claude-voice

use thiserror::Error;

#[derive(Error, Debug)]
pub enum VoiceError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON parsing error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Transcript parsing error: {0}")]
    Transcript(String),

    #[error("Voice engine error: {0}")]
    Voice(String),

    #[error("LLM error: {0}")]
    Llm(#[from] LlmError),

    #[error("Queue error: {0}")]
    Queue(String),
}

#[derive(Error, Debug)]
pub enum LlmError {
    #[error("Provider unavailable: {0}")]
    Unavailable(String),

    #[error("API request failed: {0}")]
    Request(String),
}

pub type Result<T> = std::result::Result<T, VoiceError>;
pub type LlmResult<T> = std::result::Result<T, LlmError>;
