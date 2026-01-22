// Anthropic API provider implementation

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use super::{GenerationRequest, GenerationResponse, LlmProvider};
use crate::error::{LlmError, LlmResult};

const ANTHROPIC_API_BASE: &str = "https://api.anthropic.com/v1";
const ANTHROPIC_VERSION: &str = "2023-06-01";

#[derive(Debug, Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<Message>,
}

#[derive(Debug, Serialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    content: Vec<ContentBlock>,
    #[allow(dead_code)]
    model: String,
    usage: Usage,
}

#[derive(Debug, Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    #[allow(dead_code)]
    content_type: String,
    text: String,
}

#[derive(Debug, Deserialize)]
struct Usage {
    input_tokens: u32,
    output_tokens: u32,
}

pub struct AnthropicProvider {
    api_key: String,
    model: String,
    timeout: Duration,
}

impl AnthropicProvider {
    pub fn new(api_key: String, model: String, timeout: Duration) -> Self {
        Self {
            api_key,
            model,
            timeout,
        }
    }

    fn client(&self) -> Client {
        Client::builder()
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

        let url = format!("{}/messages", ANTHROPIC_API_BASE);

        let anthropic_request = AnthropicRequest {
            model: self.model.clone(),
            max_tokens: request.max_tokens,
            messages: vec![Message {
                role: "user".to_string(),
                content: request.prompt.clone(),
            }],
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

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(LlmError::Request(format!(
                "Anthropic API returned {}: {}",
                status, error_text
            )));
        }

        let anthropic_response: AnthropicResponse = response
            .json()
            .await
            .map_err(|e| LlmError::Request(format!("Failed to parse Anthropic response: {}", e)))?;

        if anthropic_response.content.is_empty() {
            return Err(LlmError::Request("No content in Anthropic response".to_string()));
        }

        let text = anthropic_response
            .content
            .iter()
            .map(|c| c.text.as_str())
            .collect::<Vec<_>>()
            .join("");

        Ok(GenerationResponse {
            text,
            input_tokens: anthropic_response.usage.input_tokens,
            output_tokens: anthropic_response.usage.output_tokens,
            model: self.model.clone(),
        })
    }

    fn estimate_cost(&self, input_tokens: u32, output_tokens: u32) -> f64 {
        // Claude 3 Haiku pricing (per 1K tokens)
        const INPUT_COST_PER_1K: f64 = 0.00025;
        const OUTPUT_COST_PER_1K: f64 = 0.00125;

        let input_cost = (input_tokens as f64 / 1000.0) * INPUT_COST_PER_1K;
        let output_cost = (output_tokens as f64 / 1000.0) * OUTPUT_COST_PER_1K;

        input_cost + output_cost
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_anthropic_provider_creation() {
        let provider = AnthropicProvider::new(
            "test-key".to_string(),
            "claude-3-haiku-20240307".to_string(),
            Duration::from_secs(10),
        );

        assert_eq!(provider.name(), "anthropic");
        assert!(provider.is_available());
    }

    #[test]
    fn test_is_available_with_key() {
        let provider = AnthropicProvider::new(
            "sk-ant-test-key".to_string(),
            "claude-3-haiku-20240307".to_string(),
            Duration::from_secs(10),
        );

        assert!(provider.is_available());
    }

    #[test]
    fn test_is_available_without_key() {
        let provider = AnthropicProvider::new(
            "".to_string(),
            "claude-3-haiku-20240307".to_string(),
            Duration::from_secs(10),
        );

        assert!(!provider.is_available());
    }

    #[test]
    fn test_is_available_with_placeholder() {
        let provider = AnthropicProvider::new(
            "${ANTHROPIC_API_KEY}".to_string(),
            "claude-3-haiku-20240307".to_string(),
            Duration::from_secs(10),
        );

        assert!(!provider.is_available());
    }

    #[test]
    fn test_estimate_cost() {
        let provider = AnthropicProvider::new(
            "test-key".to_string(),
            "claude-3-haiku-20240307".to_string(),
            Duration::from_secs(10),
        );

        let cost = provider.estimate_cost(1000, 1000);
        // 1000 * 0.00025 + 1000 * 0.00125 = 0.0015
        assert!((cost - 0.0015).abs() < 0.000001);
    }

    #[tokio::test]
    async fn test_generate_with_unavailable_provider() {
        let provider = AnthropicProvider::new(
            "".to_string(),
            "claude-3-haiku-20240307".to_string(),
            Duration::from_secs(10),
        );

        let request = GenerationRequest {
            prompt: "Test".to_string(),
            max_tokens: 100,
            temperature: 0.3,
        };

        let result = provider.generate(&request).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LlmError::Unavailable(_)));
    }

    // Integration test - requires actual API key
    #[tokio::test]
    #[ignore]
    async fn test_generate_with_real_api() {
        let api_key = std::env::var("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY not set");
        let provider = AnthropicProvider::new(
            api_key,
            "claude-3-haiku-20240307".to_string(),
            Duration::from_secs(30),
        );

        let request = GenerationRequest {
            prompt: "Say 'Hello' in Traditional Chinese".to_string(),
            max_tokens: 50,
            temperature: 0.3,
        };

        let response = provider.generate(&request).await.unwrap();
        assert!(!response.text.is_empty());
        assert!(response.input_tokens > 0);
        assert!(response.output_tokens > 0);
    }
}
