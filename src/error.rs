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
}

#[derive(Error, Debug)]
pub enum LlmError {
    #[error("Provider unavailable: {0}")]
    Unavailable(String),

    #[error("API request failed: {0}")]
    Request(String),

    #[error("Budget exceeded: ${0:.2}")]
    BudgetExceeded(f64),

    #[error("Timeout: {0}s")]
    Timeout(u64),

    #[error("All providers failed")]
    AllProvidersFailed,
}

pub type Result<T> = std::result::Result<T, VoiceError>;
pub type LlmResult<T> = std::result::Result<T, LlmError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = VoiceError::Config("missing field".to_string());
        assert_eq!(err.to_string(), "Configuration error: missing field");
    }

    #[test]
    fn test_llm_error_display() {
        let err = LlmError::BudgetExceeded(0.15);
        assert_eq!(err.to_string(), "Budget exceeded: $0.15");
    }

    #[test]
    fn test_error_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let voice_err: VoiceError = io_err.into();
        assert!(matches!(voice_err, VoiceError::Io(_)));
    }

    #[test]
    fn test_error_from_json() {
        let json_str = "{invalid json}";
        let json_err = serde_json::from_str::<serde_json::Value>(json_str).unwrap_err();
        let voice_err: VoiceError = json_err.into();
        assert!(matches!(voice_err, VoiceError::Json(_)));
    }
}
