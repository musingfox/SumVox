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

    /// Check if this is a human-authored user message (not a tool_result).
    ///
    /// In Claude Code transcripts, both real user input and tool_result entries
    /// share `type: "user"` and `role: "user"`. This method distinguishes them:
    /// - Real user message: content is a string, or content contains "text" blocks
    /// - Tool result: content contains only "tool_result" blocks (no "text" blocks)
    pub fn is_human_text(&self) -> bool {
        match &self.content {
            MessageContent::Text(_) => true,
            MessageContent::Blocks(blocks) => {
                blocks.iter().any(|b| matches!(b, ContentBlock::Text { .. }))
            }
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
                        || (entry.entry_type == "message"
                            && entry
                                .message
                                .as_ref()
                                .is_some_and(|m| m.role == "assistant"));

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

    /// Read assistant texts from the last N conversation turns
    ///
    /// A turn is defined from a user message to the next user message (or EOF).
    /// This ensures we only summarize the most recent interaction, not historical steps.
    ///
    /// # Arguments
    /// * `path` - Path to the transcript JSONL file
    /// * `n` - Number of turns to read (must be >= 1)
    ///
    /// # Fallback
    /// If no user messages are found, returns the last 1 text block to avoid empty results.
    ///
    /// # Example
    /// ```text
    /// Transcript:
    /// user: "Write a function"      <- Turn 1 starts
    /// assistant: "Here's the code..."
    /// assistant: "Function done"
    /// user: "Run tests"              <- Turn 2 starts
    /// assistant: "Running tests..."
    /// assistant: "Tests passed"      <- Turn 2 ends (EOF)
    ///
    /// read_last_n_turns(path, 1) -> ["Running tests...", "Tests passed"]
    /// read_last_n_turns(path, 2) -> ["Here's the code...", "Function done", "Running tests...", "Tests passed"]
    /// ```
    pub async fn read_last_n_turns(path: impl AsRef<Path>, n: usize) -> Result<Vec<String>> {
        let n = n.max(1); // Ensure at least 1 turn

        // Read all lines into memory (transcript files are typically small)
        let file = File::open(path.as_ref()).await.map_err(|e| {
            VoiceError::Transcript(format!("Failed to open transcript file: {}", e))
        })?;

        let reader = BufReader::new(file);
        let mut lines_vec = Vec::new();
        let mut lines = reader.lines();

        while let Some(line) = lines.next_line().await? {
            if !line.trim().is_empty() {
                lines_vec.push(line);
            }
        }

        // Find human user message indices (turn boundaries).
        // In Claude Code transcripts, tool_result entries also have type="user"
        // and role="user", but they should NOT be treated as turn boundaries.
        // Only real human input (text content) marks a new turn.
        let mut user_indices = Vec::new();
        for (idx, line) in lines_vec.iter().enumerate() {
            if let Ok(entry) = serde_json::from_str::<TranscriptEntry>(line) {
                let is_user = entry.entry_type == "user"
                    || (entry.entry_type == "message"
                        && entry.message.as_ref().is_some_and(|m| m.role == "user"));

                if is_user {
                    if let Some(ref message) = entry.message {
                        if message.is_human_text() {
                            user_indices.push(idx);
                        }
                    }
                }
            }
        }

        // Fallback: No user messages found, read last 1 text block
        if user_indices.is_empty() {
            tracing::debug!("No user messages found in transcript, fallback to last 1 text block");
            return Self::read_last_n_texts(path, 1).await;
        }

        // Calculate start index: position of the Nth-last user message
        let start_idx = if user_indices.len() >= n {
            user_indices[user_indices.len() - n]
        } else {
            // Not enough turns, start from the first user message
            user_indices[0]
        };

        // Extract assistant texts from start_idx to EOF
        let mut texts = Vec::new();
        for line in &lines_vec[start_idx..] {
            if let Ok(entry) = serde_json::from_str::<TranscriptEntry>(line) {
                let is_assistant = entry.entry_type == "assistant"
                    || (entry.entry_type == "message"
                        && entry
                            .message
                            .as_ref()
                            .is_some_and(|m| m.role == "assistant"));

                if is_assistant {
                    if let Some(message) = entry.message {
                        texts.extend(message.extract_texts());
                    }
                }
            }
        }

        Ok(texts)
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

        let texts = TranscriptReader::read_assistant_texts(path, 2)
            .await
            .unwrap();

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

    #[tokio::test]
    async fn test_read_last_one_turn() {
        let jsonl_content = r#"{"type":"conversation_start","timestamp":"2025-01-22T10:00:00Z"}
{"type":"message","message":{"role":"user","content":[{"type":"text","text":"Write a function"}]},"timestamp":"2025-01-22T10:00:01Z"}
{"type":"message","message":{"role":"assistant","content":[{"type":"text","text":"Here's the code"}]},"timestamp":"2025-01-22T10:00:02Z"}
{"type":"message","message":{"role":"assistant","content":[{"type":"text","text":"Function done"}]},"timestamp":"2025-01-22T10:00:03Z"}
{"type":"message","message":{"role":"user","content":[{"type":"text","text":"Run tests"}]},"timestamp":"2025-01-22T10:00:04Z"}
{"type":"message","message":{"role":"assistant","content":[{"type":"text","text":"Running tests"}]},"timestamp":"2025-01-22T10:00:05Z"}
{"type":"message","message":{"role":"assistant","content":[{"type":"text","text":"Tests passed"}]},"timestamp":"2025-01-22T10:00:06Z"}
"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(jsonl_content.as_bytes()).unwrap();
        let path = temp_file.path();

        let texts = TranscriptReader::read_last_n_turns(path, 1).await.unwrap();

        // Should only get the last turn (after "Run tests")
        assert_eq!(texts.len(), 2);
        assert_eq!(texts[0], "Running tests");
        assert_eq!(texts[1], "Tests passed");
    }

    #[tokio::test]
    async fn test_read_last_n_turns_multiple() {
        let jsonl_content = r#"{"type":"message","message":{"role":"user","content":[{"type":"text","text":"Turn 1"}]}}
{"type":"message","message":{"role":"assistant","content":[{"type":"text","text":"Response 1"}]}}
{"type":"message","message":{"role":"user","content":[{"type":"text","text":"Turn 2"}]}}
{"type":"message","message":{"role":"assistant","content":[{"type":"text","text":"Response 2"}]}}
{"type":"message","message":{"role":"user","content":[{"type":"text","text":"Turn 3"}]}}
{"type":"message","message":{"role":"assistant","content":[{"type":"text","text":"Response 3"}]}}
"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(jsonl_content.as_bytes()).unwrap();
        let path = temp_file.path();

        // Read last 2 turns
        let texts = TranscriptReader::read_last_n_turns(path, 2).await.unwrap();
        assert_eq!(texts.len(), 2);
        assert_eq!(texts[0], "Response 2");
        assert_eq!(texts[1], "Response 3");

        // Read all 3 turns
        let texts = TranscriptReader::read_last_n_turns(path, 3).await.unwrap();
        assert_eq!(texts.len(), 3);
        assert_eq!(texts[0], "Response 1");
        assert_eq!(texts[1], "Response 2");
        assert_eq!(texts[2], "Response 3");
    }

    #[tokio::test]
    async fn test_read_last_n_turns_with_tools() {
        let jsonl_content = r#"{"type":"message","message":{"role":"user","content":[{"type":"text","text":"Do something"}]}}
{"type":"message","message":{"role":"assistant","content":[{"type":"text","text":"Let me help"}]}}
{"type":"message","message":{"role":"assistant","content":[{"type":"tool_use","name":"bash","input":{"command":"ls"}}]}}
{"type":"message","message":{"role":"assistant","content":[{"type":"text","text":"Done"}]}}
"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(jsonl_content.as_bytes()).unwrap();
        let path = temp_file.path();

        let texts = TranscriptReader::read_last_n_turns(path, 1).await.unwrap();

        // Should only extract text blocks, not tool_use
        assert_eq!(texts.len(), 2);
        assert_eq!(texts[0], "Let me help");
        assert_eq!(texts[1], "Done");
    }

    #[tokio::test]
    async fn test_read_last_n_turns_no_user() {
        let jsonl_content = r#"{"type":"message","message":{"role":"assistant","content":[{"type":"text","text":"Text 1"}]}}
{"type":"message","message":{"role":"assistant","content":[{"type":"text","text":"Text 2"}]}}
{"type":"message","message":{"role":"assistant","content":[{"type":"text","text":"Text 3"}]}}
"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(jsonl_content.as_bytes()).unwrap();
        let path = temp_file.path();

        // Fallback: should return last 1 text block
        let texts = TranscriptReader::read_last_n_turns(path, 1).await.unwrap();
        assert_eq!(texts.len(), 1);
        assert_eq!(texts[0], "Text 3");
    }

    #[tokio::test]
    async fn test_read_last_n_turns_insufficient_turns() {
        let jsonl_content = r#"{"type":"message","message":{"role":"user","content":[{"type":"text","text":"Only one turn"}]}}
{"type":"message","message":{"role":"assistant","content":[{"type":"text","text":"Response"}]}}
"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(jsonl_content.as_bytes()).unwrap();
        let path = temp_file.path();

        // Request 5 turns but only 1 exists - should return all texts from turn 1
        let texts = TranscriptReader::read_last_n_turns(path, 5).await.unwrap();
        assert_eq!(texts.len(), 1);
        assert_eq!(texts[0], "Response");
    }

    #[tokio::test]
    async fn test_read_last_n_turns_ignores_tool_result_boundaries() {
        // Simulates real Claude Code transcript where tool_result entries
        // have type="user" but should NOT be treated as turn boundaries.
        let jsonl_content = r#"{"type":"user","message":{"role":"user","content":"Fix the bug"}}
{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Let me look at the code"}]}}
{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","name":"Read","input":{"path":"/tmp/test.rs"}}]}}
{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_123","content":"fn main() {}"}]}}
{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","name":"Edit","input":{"path":"/tmp/test.rs"}}]}}
{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_456","content":"File edited"}]}}
{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Bug fixed successfully"}]}}
"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(jsonl_content.as_bytes()).unwrap();
        let path = temp_file.path();

        let texts = TranscriptReader::read_last_n_turns(path, 1).await.unwrap();

        // Should get ALL assistant texts from the turn, not just the ones after
        // the last tool_result. tool_result entries should not split the turn.
        assert_eq!(texts.len(), 2);
        assert_eq!(texts[0], "Let me look at the code");
        assert_eq!(texts[1], "Bug fixed successfully");
    }

    #[tokio::test]
    async fn test_read_last_n_turns_claude_code_format() {
        // Uses Claude Code's native format: type="user"/"assistant" (not "message")
        let jsonl_content = r#"{"type":"user","message":{"role":"user","content":"Summarize the changes"}}
{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Here is the summary"}]}}
{"type":"user","message":{"role":"user","content":"Deploy it"}}
{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Deploying now"}]}}
{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","name":"Bash","input":{"command":"deploy.sh"}}]}}
{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_789","content":"Deployed!"}]}}
{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Deployment complete"}]}}
"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(jsonl_content.as_bytes()).unwrap();
        let path = temp_file.path();

        // Last 1 turn should be "Deploy it" and all its assistant responses
        let texts = TranscriptReader::read_last_n_turns(path, 1).await.unwrap();
        assert_eq!(texts.len(), 2);
        assert_eq!(texts[0], "Deploying now");
        assert_eq!(texts[1], "Deployment complete");

        // Last 2 turns should include both
        let texts = TranscriptReader::read_last_n_turns(path, 2).await.unwrap();
        assert_eq!(texts.len(), 3);
        assert_eq!(texts[0], "Here is the summary");
        assert_eq!(texts[1], "Deploying now");
        assert_eq!(texts[2], "Deployment complete");
    }
}
