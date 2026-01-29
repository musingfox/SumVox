// Transcript JSONL reader for Claude Code

use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::error::{Result, VoiceError};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TranscriptEntry {
    #[serde(rename = "type")]
    pub entry_type: String,
    pub message: Option<Message>,
    pub timestamp: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Message {
    pub role: String,
    pub content: MessageContent,
}

/// Message content can be either a string or an array of ContentBlocks
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

impl Message {
    /// Extract text content from message, handling both string and array formats
    pub fn extract_texts(&self) -> Vec<String> {
        match &self.content {
            MessageContent::Text(text) => vec![text.clone()],
            MessageContent::Blocks(blocks) => blocks
                .iter()
                .filter_map(|block| {
                    if let ContentBlock::Text { text } = block {
                        Some(text.clone())
                    } else {
                        None
                    }
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        name: String,
        input: serde_json::Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: serde_json::Value,
    },
    #[serde(other)]
    Other,
}

pub struct TranscriptReader;

impl TranscriptReader {
    /// Read transcript JSONL file and extract assistant text blocks
    pub async fn read_assistant_texts(path: impl AsRef<Path>, limit: usize) -> Result<Vec<String>> {
        let file = File::open(path.as_ref()).await.map_err(|e| {
            VoiceError::Transcript(format!("Failed to open transcript file: {}", e))
        })?;

        let reader = BufReader::new(file);
        let mut lines = reader.lines();
        let mut texts = Vec::new();

        while let Some(line) = lines.next_line().await? {
            if line.trim().is_empty() {
                continue;
            }

            match serde_json::from_str::<TranscriptEntry>(&line) {
                Ok(entry) => {
                    // Support both formats:
                    // 1. Test format: {"type":"message","message":{"role":"assistant",...}}
                    // 2. Claude Code format: {"type":"assistant","message":{"role":"assistant",...}}
                    let is_assistant = entry.entry_type == "assistant"
                        || (entry.entry_type == "message" && entry.message.as_ref().map_or(false, |m| m.role == "assistant"));

                    if is_assistant {
                        if let Some(message) = entry.message {
                            for text in message.extract_texts() {
                                texts.push(text);
                                if texts.len() >= limit {
                                    return Ok(texts);
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    // Skip malformed lines (different message formats, progress entries, etc.)
                    tracing::debug!("Skipping non-message JSONL line: {}", e);
                    continue;
                }
            }
        }

        Ok(texts)
    }

    /// Read last N assistant text blocks from transcript
    pub async fn read_last_n_texts(path: impl AsRef<Path>, n: usize) -> Result<Vec<String>> {
        let all_texts = Self::read_assistant_texts(path, usize::MAX).await?;
        let start = all_texts.len().saturating_sub(n);
        Ok(all_texts[start..].to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_read_assistant_texts() {
        let jsonl_content = r#"{"type":"conversation_start","timestamp":"2025-01-22T10:00:00Z"}
{"type":"message","message":{"role":"user","content":[{"type":"text","text":"Hello"}]},"timestamp":"2025-01-22T10:00:01Z"}
{"type":"message","message":{"role":"assistant","content":[{"type":"text","text":"First response"}]},"timestamp":"2025-01-22T10:00:02Z"}
{"type":"message","message":{"role":"assistant","content":[{"type":"text","text":"Second response"}]},"timestamp":"2025-01-22T10:00:03Z"}
{"type":"message","message":{"role":"assistant","content":[{"type":"tool_use","name":"bash","input":{"command":"ls"}}]},"timestamp":"2025-01-22T10:00:04Z"}
{"type":"message","message":{"role":"assistant","content":[{"type":"text","text":"Third response"}]},"timestamp":"2025-01-22T10:00:05Z"}
"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(jsonl_content.as_bytes()).unwrap();
        let path = temp_file.path();

        let texts = TranscriptReader::read_assistant_texts(path, 10)
            .await
            .unwrap();

        assert_eq!(texts.len(), 3);
        assert_eq!(texts[0], "First response");
        assert_eq!(texts[1], "Second response");
        assert_eq!(texts[2], "Third response");
    }

    #[tokio::test]
    async fn test_read_last_n_texts() {
        let jsonl_content = r#"{"type":"message","message":{"role":"assistant","content":[{"type":"text","text":"Text 1"}]},"timestamp":"2025-01-22T10:00:01Z"}
{"type":"message","message":{"role":"assistant","content":[{"type":"text","text":"Text 2"}]},"timestamp":"2025-01-22T10:00:02Z"}
{"type":"message","message":{"role":"assistant","content":[{"type":"text","text":"Text 3"}]},"timestamp":"2025-01-22T10:00:03Z"}
{"type":"message","message":{"role":"assistant","content":[{"type":"text","text":"Text 4"}]},"timestamp":"2025-01-22T10:00:04Z"}
{"type":"message","message":{"role":"assistant","content":[{"type":"text","text":"Text 5"}]},"timestamp":"2025-01-22T10:00:05Z"}
"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(jsonl_content.as_bytes()).unwrap();
        let path = temp_file.path();

        let texts = TranscriptReader::read_last_n_texts(path, 3).await.unwrap();

        assert_eq!(texts.len(), 3);
        assert_eq!(texts[0], "Text 3");
        assert_eq!(texts[1], "Text 4");
        assert_eq!(texts[2], "Text 5");
    }

    #[tokio::test]
    async fn test_read_with_limit() {
        let jsonl_content = r#"{"type":"message","message":{"role":"assistant","content":[{"type":"text","text":"Text 1"}]}}
{"type":"message","message":{"role":"assistant","content":[{"type":"text","text":"Text 2"}]}}
{"type":"message","message":{"role":"assistant","content":[{"type":"text","text":"Text 3"}]}}
"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(jsonl_content.as_bytes()).unwrap();
        let path = temp_file.path();

        let texts = TranscriptReader::read_assistant_texts(path, 2).await.unwrap();

        assert_eq!(texts.len(), 2);
        assert_eq!(texts[0], "Text 1");
        assert_eq!(texts[1], "Text 2");
    }

    #[tokio::test]
    async fn test_empty_file() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let texts = TranscriptReader::read_assistant_texts(path, 10)
            .await
            .unwrap();

        assert_eq!(texts.len(), 0);
    }

    #[tokio::test]
    async fn test_malformed_jsonl() {
        let jsonl_content = r#"{"type":"message","message":{"role":"assistant","content":[{"type":"text","text":"Valid"}]}}
invalid json line
{"type":"message","message":{"role":"assistant","content":[{"type":"text","text":"Also valid"}]}}
"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(jsonl_content.as_bytes()).unwrap();
        let path = temp_file.path();

        let texts = TranscriptReader::read_assistant_texts(path, 10)
            .await
            .unwrap();

        // Should skip the malformed line
        assert_eq!(texts.len(), 2);
        assert_eq!(texts[0], "Valid");
        assert_eq!(texts[1], "Also valid");
    }
}
