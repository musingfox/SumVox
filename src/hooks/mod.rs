// Hook processing module
// Handles JSON input from various AI coding tools with format detection

pub mod claude_code;

use serde::Deserialize;
use serde_json::Value;
use std::str::FromStr;

use crate::error::{Result, VoiceError};

/// Detected hook format
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookFormat {
    ClaudeCode,
    GeminiCli,
    Generic,
}

impl FromStr for HookFormat {
    type Err = VoiceError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "claude-code" | "claude_code" | "claudecode" => Ok(HookFormat::ClaudeCode),
            "gemini-cli" | "gemini_cli" | "geminicli" => Ok(HookFormat::GeminiCli),
            "generic" => Ok(HookFormat::Generic),
            _ => Err(VoiceError::Config(format!("Unknown hook format: {}", s))),
        }
    }
}

/// Generic hook input for format detection
#[derive(Debug, Deserialize)]
pub struct GenericHookInput {
    /// Text content to process (for generic format)
    #[serde(default)]
    pub text: Option<String>,

    /// Message content (alternative field name)
    #[serde(default)]
    pub message: Option<String>,

    /// Content field (another alternative)
    #[serde(default)]
    pub content: Option<String>,
}

impl GenericHookInput {
    /// Get the text content from any available field
    pub fn get_text(&self) -> Option<&str> {
        self.text
            .as_deref()
            .or(self.message.as_deref())
            .or(self.content.as_deref())
    }
}

/// Detect the format of JSON input
pub fn detect_format(json: &Value) -> HookFormat {
    // Claude Code: has session_id and hook_event_name
    if json.get("session_id").is_some() && json.get("hook_event_name").is_some() {
        return HookFormat::ClaudeCode;
    }

    // Gemini CLI: has specific fields (to be defined)
    // For now, check for potential Gemini-specific fields
    if json.get("gemini_session").is_some()
        || json
            .get("tool_name")
            .map(|v| v.as_str() == Some("gemini"))
            .unwrap_or(false)
    {
        return HookFormat::GeminiCli;
    }

    // Default to generic
    HookFormat::Generic
}

/// Parse JSON input and detect its format
pub fn parse_input(input: &str) -> Result<(Value, HookFormat)> {
    let json: Value = serde_json::from_str(input)?;
    let format = detect_format(&json);
    Ok((json, format))
}

/// Parse as generic hook input
pub fn parse_generic(input: &str) -> Result<GenericHookInput> {
    let generic: GenericHookInput = serde_json::from_str(input)?;

    if generic.get_text().is_none() {
        return Err(VoiceError::Config(
            "Generic hook input requires 'text', 'message', or 'content' field".into(),
        ));
    }

    Ok(generic)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_claude_code_format() {
        let json = serde_json::json!({
            "session_id": "test-123",
            "hook_event_name": "Stop",
            "transcript_path": "/path/to/transcript.jsonl"
        });

        assert_eq!(detect_format(&json), HookFormat::ClaudeCode);
    }

    #[test]
    fn test_detect_generic_format() {
        let json = serde_json::json!({
            "text": "Hello world"
        });

        assert_eq!(detect_format(&json), HookFormat::Generic);
    }

    #[test]
    fn test_hook_format_from_str() {
        assert_eq!(
            "claude-code".parse::<HookFormat>().ok(),
            Some(HookFormat::ClaudeCode)
        );
        assert_eq!(
            "claude_code".parse::<HookFormat>().ok(),
            Some(HookFormat::ClaudeCode)
        );
        assert_eq!(
            "gemini-cli".parse::<HookFormat>().ok(),
            Some(HookFormat::GeminiCli)
        );
        assert_eq!(
            "generic".parse::<HookFormat>().ok(),
            Some(HookFormat::Generic)
        );
        assert!("auto".parse::<HookFormat>().is_err());
        assert!("unknown".parse::<HookFormat>().is_err());
    }

    #[test]
    fn test_parse_generic_with_text() {
        let input = r#"{"text": "Hello world"}"#;
        let generic = parse_generic(input).unwrap();
        assert_eq!(generic.get_text(), Some("Hello world"));
    }

    #[test]
    fn test_parse_generic_with_message() {
        let input = r#"{"message": "Hello from message"}"#;
        let generic = parse_generic(input).unwrap();
        assert_eq!(generic.get_text(), Some("Hello from message"));
    }

    #[test]
    fn test_parse_generic_with_content() {
        let input = r#"{"content": "Hello from content"}"#;
        let generic = parse_generic(input).unwrap();
        assert_eq!(generic.get_text(), Some("Hello from content"));
    }

    #[test]
    fn test_parse_generic_empty_fails() {
        let input = r#"{}"#;
        let result = parse_generic(input);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_input() {
        let input = r#"{"session_id": "test", "hook_event_name": "Stop"}"#;
        let (json, format) = parse_input(input).unwrap();

        assert_eq!(format, HookFormat::ClaudeCode);
        assert_eq!(json["session_id"], "test");
    }
}
