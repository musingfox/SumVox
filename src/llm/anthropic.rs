// Anthropic API provider implementation

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use super::{GenerationRequest, GenerationResponse, LlmProvider};
use crate::error::{LlmError, LlmResult};

pub(crate) const ANTHROPIC_API_BASE: &str = "https://api.anthropic.com/v1";
const ANTHROPIC_VERSION: &str = "2023-06-01";

#[derive(Debug, Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
}

#[derive(Debug, Serialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    content: Vec<ContentBlock>,
    usage: Usage,
}

#[derive(Debug, Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    content_type: String,

    // For text blocks
    #[serde(default)]
    text: Option<String>,

    // For thinking blocks (extended thinking)
    #[serde(default)]
    thinking: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Usage {
    input_tokens: u32,
    output_tokens: u32,
}

pub struct AnthropicProvider {
    api_key: String,
    model: String,
    base_url: String,
    timeout: Duration,
}

impl AnthropicProvider {
    pub fn with_base_url(
        api_key: String,
        model: String,
        base_url: String,
        timeout: Duration,
    ) -> Self {
        Self {
            api_key,
            model,
            base_url,
            timeout,
        }
    }

    fn client(&self) -> Client {
        Client::builder()
            .no_proxy() // Disable system proxy detection to avoid CoreFoundation crash
            .timeout(self.timeout)
            .build()
            .unwrap_or_else(|_| Client::new())
    }
}

#[async_trait]
impl LlmProvider for AnthropicProvider {
    fn name(&self) -> &str {
        "anthropic"
    }

    fn is_available(&self) -> bool {
        !self.api_key.is_empty() && !self.api_key.starts_with("${")
    }

    async fn generate(&self, request: &GenerationRequest) -> LlmResult<GenerationResponse> {
        if !self.is_available() {
            return Err(LlmError::Unavailable(
                "Anthropic API key not configured".to_string(),
            ));
        }

        let url = format!("{}/messages", self.base_url);

        // Never send thinking — extended thinking is not used.
        let anthropic_request = AnthropicRequest {
            model: self.model.clone(),
            max_tokens: request.max_tokens,
            messages: vec![Message {
                role: "user".to_string(),
                content: request.prompt.clone(),
            }],
            system: request.system_message.clone(),
        };

        tracing::debug!("Sending request to Anthropic API: {}", self.model);

        let response = self
            .client()
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&anthropic_request)
            .send()
            .await
            .map_err(|e| LlmError::Request(format!("Anthropic API request failed: {}", e)))?;

        let status = response.status();
        let response_text = response
            .text()
            .await
            .map_err(|e| LlmError::Request(format!("Failed to read response body: {}", e)))?;

        if !status.is_success() {
            return Err(LlmError::Request(format!(
                "Anthropic API returned {}: {}",
                status, response_text
            )));
        }

        tracing::debug!("Anthropic API response: {}", response_text);

        let anthropic_response: AnthropicResponse =
            serde_json::from_str(&response_text).map_err(|e| {
                LlmError::Request(format!(
                    "Failed to parse Anthropic response: {}. Response body: {}",
                    e, response_text
                ))
            })?;

        if anthropic_response.content.is_empty() {
            return Err(LlmError::Request(
                "No content in Anthropic response".to_string(),
            ));
        }

        // Extract text from content blocks, skipping thinking blocks
        let text = anthropic_response
            .content
            .iter()
            .filter_map(|c| {
                match c.content_type.as_str() {
                    "text" => c.text.as_deref(),
                    "thinking" => {
                        // Log thinking content in debug mode
                        if let Some(thinking) = &c.thinking {
                            tracing::debug!("Extended thinking: {}", thinking);
                        }
                        None
                    }
                    _ => {
                        tracing::warn!("Unknown content type: {}", c.content_type);
                        None
                    }
                }
            })
            .collect::<Vec<_>>()
            .join("");

        Ok(GenerationResponse {
            text,
            input_tokens: anthropic_response.usage.input_tokens,
            output_tokens: anthropic_response.usage.output_tokens,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_anthropic_provider_creation() {
        let provider = AnthropicProvider::with_base_url(
            "test-key".to_string(),
            "claude-haiku-4-5".to_string(),
            ANTHROPIC_API_BASE.to_string(),
            Duration::from_secs(10),
        );

        assert_eq!(provider.name(), "anthropic");
        assert!(provider.is_available());
    }

    #[test]
    fn test_is_available_with_key() {
        let provider = AnthropicProvider::with_base_url(
            "sk-ant-test-key".to_string(),
            "claude-haiku-4-5".to_string(),
            ANTHROPIC_API_BASE.to_string(),
            Duration::from_secs(10),
        );

        assert!(provider.is_available());
    }

    #[test]
    fn test_is_available_without_key() {
        let provider = AnthropicProvider::with_base_url(
            "".to_string(),
            "claude-haiku-4-5".to_string(),
            ANTHROPIC_API_BASE.to_string(),
            Duration::from_secs(10),
        );

        assert!(!provider.is_available());
    }

    #[test]
    fn test_is_available_with_placeholder() {
        let provider = AnthropicProvider::with_base_url(
            "${ANTHROPIC_API_KEY}".to_string(),
            "claude-haiku-4-5".to_string(),
            ANTHROPIC_API_BASE.to_string(),
            Duration::from_secs(10),
        );

        assert!(!provider.is_available());
    }

    #[tokio::test]
    async fn test_generate_with_unavailable_provider() {
        let provider = AnthropicProvider::with_base_url(
            "".to_string(),
            "claude-haiku-4-5".to_string(),
            ANTHROPIC_API_BASE.to_string(),
            Duration::from_secs(10),
        );

        let request = GenerationRequest {
            system_message: None,
            prompt: "Test".to_string(),
            max_tokens: 100,
            temperature: 0.3,
            disable_thinking: false,
        };

        let result = provider.generate(&request).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LlmError::Unavailable(_)));
    }

    // Integration test - requires actual API key
    #[tokio::test]
    #[ignore = "e2e-network"]
    async fn test_generate_with_real_api() {
        let api_key = std::env::var("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY not set");
        let provider = AnthropicProvider::with_base_url(
            api_key,
            "claude-haiku-4-5".to_string(),
            ANTHROPIC_API_BASE.to_string(),
            Duration::from_secs(30),
        );

        let request = GenerationRequest {
            system_message: None,
            prompt: "Say 'Hello' in Traditional Chinese".to_string(),
            max_tokens: 50,
            temperature: 0.3,
            disable_thinking: false,
        };

        let response = provider.generate(&request).await.unwrap();
        assert!(!response.text.is_empty());
        assert!(response.input_tokens > 0);
        assert!(response.output_tokens > 0);
    }
}
