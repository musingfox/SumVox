// OpenAI API provider implementation

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use super::{GenerationRequest, GenerationResponse, LlmProvider};
use crate::error::{LlmError, LlmResult};

const OPENAI_API_BASE: &str = "https://api.openai.com/v1";

#[derive(Debug, Serialize)]
struct OpenAIRequest {
    model: String,
    messages: Vec<Message>,

    /// Max completion tokens for newer models (o1, o3, GPT-5)
    /// Older models still use max_tokens
    #[serde(skip_serializing_if = "Option::is_none")]
    max_completion_tokens: Option<u32>,

    /// Max tokens for legacy models (GPT-4, GPT-3.5)
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,

    /// Temperature for non-reasoning models
    /// Reasoning models (o1, o3, GPT-5) only support default temperature=1
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,

    /// Reasoning effort for o1/o3/GPT-5 models
    /// Values: "low", "medium", "high", "xhigh" (gpt-5.1-codex-max)
    /// API docs: https://platform.openai.com/docs/guides/reasoning
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_effort: Option<String>,
}

#[derive(Debug, Serialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct OpenAIResponse {
    choices: Vec<Choice>,
    usage: Usage,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: ResponseMessage,
}

#[derive(Debug, Deserialize)]
struct ResponseMessage {
    content: String,
}

#[derive(Debug, Deserialize)]
struct Usage {
    prompt_tokens: u32,
    completion_tokens: u32,
}

pub struct OpenAIProvider {
    api_key: String,
    model: String,
    timeout: Duration,
}

impl OpenAIProvider {
    pub fn new(api_key: String, model: String, timeout: Duration) -> Self {
        Self {
            api_key,
            model,
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

    fn extract_model_name(&self) -> &str {
        // Handle "openai/gpt-4o-mini" -> "gpt-4o-mini"
        if let Some(idx) = self.model.find('/') {
            &self.model[idx + 1..]
        } else {
            &self.model
        }
    }
}

#[async_trait]
impl LlmProvider for OpenAIProvider {
    fn name(&self) -> &str {
        "openai"
    }

    fn is_available(&self) -> bool {
        !self.api_key.is_empty() && !self.api_key.starts_with("${")
    }

    async fn generate(&self, request: &GenerationRequest) -> LlmResult<GenerationResponse> {
        if !self.is_available() {
            return Err(LlmError::Unavailable(
                "OpenAI API key not configured".to_string(),
            ));
        }

        let model_name = self.extract_model_name();
        let url = format!("{}/chat/completions", OPENAI_API_BASE);

        let mut messages = Vec::new();

        if let Some(ref system_msg) = request.system_message {
            messages.push(Message {
                role: "system".to_string(),
                content: system_msg.clone(),
            });
        }

        messages.push(Message {
            role: "user".to_string(),
            content: request.prompt.clone(),
        });

        // Check if this is a reasoning model
        // Supported: o1, o3, o3-mini, GPT-5, gpt-5.1, etc.
        let is_reasoning_model = model_name.starts_with("o1")
            || model_name.starts_with("o3")
            || model_name.starts_with("gpt-5");

        // Set reasoning_effort based on disable_thinking
        // disable_thinking = true: use "low" to minimize reasoning
        // disable_thinking = false: use "high" to maximize reasoning
        let reasoning_effort = if is_reasoning_model {
            Some(if request.disable_thinking {
                "low".to_string() // Minimize reasoning
            } else {
                "high".to_string() // Maximize reasoning
            })
        } else {
            None // Non-reasoning models, don't send parameter
        };

        // Newer models (o1, o3, GPT-5) use max_completion_tokens
        // Older models (GPT-4, GPT-3.5) use max_tokens
        let (max_completion_tokens, max_tokens) = if is_reasoning_model {
            (Some(request.max_tokens), None)
        } else {
            (None, Some(request.max_tokens))
        };

        // Reasoning models only support temperature=1 (default)
        // Don't send temperature parameter for these models
        let temperature = if is_reasoning_model {
            None
        } else {
            Some(request.temperature)
        };

        let openai_request = OpenAIRequest {
            model: model_name.to_string(),
            messages,
            max_completion_tokens,
            max_tokens,
            temperature,
            reasoning_effort,
        };

        tracing::debug!("Sending request to OpenAI API: {}", model_name);

        let response = self
            .client()
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&openai_request)
            .send()
            .await
            .map_err(|e| LlmError::Request(format!("OpenAI API request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(LlmError::Request(format!(
                "OpenAI API returned {}: {}",
                status, error_text
            )));
        }

        let openai_response: OpenAIResponse = response
            .json()
            .await
            .map_err(|e| LlmError::Request(format!("Failed to parse OpenAI response: {}", e)))?;

        if openai_response.choices.is_empty() {
            return Err(LlmError::Request("No choices in OpenAI response".to_string()));
        }

        let text = openai_response.choices[0].message.content.clone();

        Ok(GenerationResponse {
            text,
            input_tokens: openai_response.usage.prompt_tokens,
            output_tokens: openai_response.usage.completion_tokens,
            model: self.model.clone(),
        })
    }

    fn estimate_cost(&self, input_tokens: u32, output_tokens: u32) -> f64 {
        // GPT-4o-mini pricing (per 1K tokens)
        const INPUT_COST_PER_1K: f64 = 0.00015;
        const OUTPUT_COST_PER_1K: f64 = 0.0006;

        let input_cost = (input_tokens as f64 / 1000.0) * INPUT_COST_PER_1K;
        let output_cost = (output_tokens as f64 / 1000.0) * OUTPUT_COST_PER_1K;

        input_cost + output_cost
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openai_provider_creation() {
        let provider = OpenAIProvider::new(
            "test-key".to_string(),
            "gpt-4o-mini".to_string(),
            Duration::from_secs(10),
        );

        assert_eq!(provider.name(), "openai");
        assert!(provider.is_available());
    }

    #[test]
    fn test_is_available_with_key() {
        let provider = OpenAIProvider::new(
            "sk-test-key".to_string(),
            "gpt-4o-mini".to_string(),
            Duration::from_secs(10),
        );

        assert!(provider.is_available());
    }

    #[test]
    fn test_is_available_without_key() {
        let provider = OpenAIProvider::new(
            "".to_string(),
            "gpt-4o-mini".to_string(),
            Duration::from_secs(10),
        );

        assert!(!provider.is_available());
    }

    #[test]
    fn test_is_available_with_placeholder() {
        let provider = OpenAIProvider::new(
            "${OPENAI_API_KEY}".to_string(),
            "gpt-4o-mini".to_string(),
            Duration::from_secs(10),
        );

        assert!(!provider.is_available());
    }

    #[test]
    fn test_extract_model_name() {
        let provider = OpenAIProvider::new(
            "test-key".to_string(),
            "openai/gpt-4o-mini".to_string(),
            Duration::from_secs(10),
        );

        assert_eq!(provider.extract_model_name(), "gpt-4o-mini");
    }

    #[test]
    fn test_extract_model_name_without_prefix() {
        let provider = OpenAIProvider::new(
            "test-key".to_string(),
            "gpt-4o-mini".to_string(),
            Duration::from_secs(10),
        );

        assert_eq!(provider.extract_model_name(), "gpt-4o-mini");
    }

    #[test]
    fn test_estimate_cost() {
        let provider = OpenAIProvider::new(
            "test-key".to_string(),
            "gpt-4o-mini".to_string(),
            Duration::from_secs(10),
        );

        let cost = provider.estimate_cost(1000, 1000);
        // 1000 * 0.00015 + 1000 * 0.0006 = 0.00075
        assert!((cost - 0.00075).abs() < 0.000001);
    }

    #[tokio::test]
    async fn test_generate_with_unavailable_provider() {
        let provider = OpenAIProvider::new(
            "".to_string(),
            "gpt-4o-mini".to_string(),
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
    #[ignore]
    async fn test_generate_with_real_api() {
        let api_key = std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not set");
        let provider = OpenAIProvider::new(
            api_key,
            "gpt-4o-mini".to_string(),
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
