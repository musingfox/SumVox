// OpenAI API provider implementation

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use super::{GenerationRequest, GenerationResponse, LlmProvider};
use crate::error::{LlmError, LlmResult};

pub(crate) const OPENAI_API_BASE: &str = "https://api.openai.com/v1";

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
    base_url: String,
    timeout: Duration,
}

impl OpenAIProvider {
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

    fn extract_model_name(&self) -> &str {
        // Handle "openai/gpt-4o-mini" -> "gpt-4o-mini"
        if let Some(idx) = self.model.find('/') {
            &self.model[idx + 1..]
        } else {
            &self.model
        }
    }
}

/// Returns true for OpenAI reasoning models that require special API treatment:
/// - max_completion_tokens instead of max_tokens
/// - no temperature parameter (only supports default=1)
///
/// Matches: o1*, o3*, o4*, gpt-5*
fn is_reasoning_model(model_name: &str) -> bool {
    model_name.starts_with("o1")
        || model_name.starts_with("o3")
        || model_name.starts_with("o4")
        || model_name.starts_with("gpt-5")
}

/// Builds the wire request. `reasoning_effort` follows `disable_thinking` alone
/// (no model-name heuristic): true sends "low", false omits the field. Reasoning
/// models take `max_completion_tokens` and no temperature; standard models take
/// `max_tokens` and temperature.
fn build_request(model_name: &str, request: &GenerationRequest) -> OpenAIRequest {
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

    let reasoning_effort = if request.disable_thinking {
        Some("low".to_string())
    } else {
        None
    };

    let (max_completion_tokens, max_tokens, temperature) = if is_reasoning_model(model_name) {
        (Some(request.max_tokens), None, None)
    } else {
        (None, Some(request.max_tokens), Some(request.temperature))
    };

    OpenAIRequest {
        model: model_name.to_string(),
        messages,
        max_completion_tokens,
        max_tokens,
        temperature,
        reasoning_effort,
    }
}

#[async_trait]
impl LlmProvider for OpenAIProvider {
    fn name(&self) -> &str {
        "openai"
    }

    fn is_available(&self) -> bool {
        crate::config::is_usable_key(&self.api_key)
    }

    async fn generate(&self, request: &GenerationRequest) -> LlmResult<GenerationResponse> {
        if !self.is_available() {
            return Err(LlmError::Unavailable(
                "OpenAI API key not configured".to_string(),
            ));
        }

        let model_name = self.extract_model_name();
        let url = format!("{}/chat/completions", self.base_url);

        let openai_request = build_request(model_name, request);

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
            return Err(LlmError::Request(
                "No choices in OpenAI response".to_string(),
            ));
        }

        let text = openai_response.choices[0].message.content.clone();

        Ok(GenerationResponse {
            text,
            input_tokens: openai_response.usage.prompt_tokens,
            output_tokens: openai_response.usage.completion_tokens,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_available_with_key() {
        // (api key, expected): empty and `${...}` placeholder keys are unusable
        for (key, expected) in [
            ("sk-test-key", true),
            ("", false),
            ("${OPENAI_API_KEY}", false),
        ] {
            let provider = OpenAIProvider::with_base_url(
                key.to_string(),
                "gpt-4o-mini".to_string(),
                OPENAI_API_BASE.to_string(),
                Duration::from_secs(10),
            );
            assert_eq!(provider.is_available(), expected, "key={key:?}");
            assert_eq!(provider.name(), "openai");
        }
    }

    #[test]
    fn test_extract_model_name() {
        for (model, expected) in [
            ("openai/gpt-4o-mini", "gpt-4o-mini"),
            ("gpt-4o-mini", "gpt-4o-mini"),
        ] {
            let provider = OpenAIProvider::with_base_url(
                "test-key".to_string(),
                model.to_string(),
                OPENAI_API_BASE.to_string(),
                Duration::from_secs(10),
            );
            assert_eq!(provider.extract_model_name(), expected);
        }
    }

    #[tokio::test]
    async fn test_generate_with_unavailable_provider() {
        let provider = OpenAIProvider::with_base_url(
            "".to_string(),
            "gpt-4o-mini".to_string(),
            OPENAI_API_BASE.to_string(),
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

    #[test]
    fn test_request_wire_format() {
        // (model, disable_thinking, reasoning_model): reasoning_effort follows
        // the flag, token fields follow the model family.
        for (model, disable_thinking, reasoning_model) in [
            ("o3-mini", true, true),
            ("o3-mini", false, true),
            ("gpt-4o-mini", true, false),
            ("gpt-4o-mini", false, false),
        ] {
            let request = GenerationRequest {
                system_message: None,
                prompt: "Test".to_string(),
                max_tokens: 100,
                temperature: 0.3,
                disable_thinking,
            };
            let json = serde_json::to_value(build_request(model, &request)).unwrap();
            let ctx = format!("{model} disable_thinking={disable_thinking}");

            assert_eq!(json["model"], model, "{ctx}");
            assert_eq!(
                json.get("reasoning_effort").is_some(),
                disable_thinking,
                "{ctx}"
            );
            if disable_thinking {
                assert_eq!(json["reasoning_effort"], "low", "{ctx}");
            }
            assert_eq!(
                json.get("max_completion_tokens").is_some(),
                reasoning_model,
                "{ctx}"
            );
            assert_eq!(json.get("max_tokens").is_some(), !reasoning_model, "{ctx}");
            assert_eq!(json.get("temperature").is_some(), !reasoning_model, "{ctx}");
        }
    }

    #[test]
    fn test_a2_is_reasoning_model_detection() {
        assert!(is_reasoning_model("o1-mini"));
        assert!(is_reasoning_model("o1-preview"));
        assert!(is_reasoning_model("o3-mini"));
        assert!(is_reasoning_model("o3"));
        assert!(is_reasoning_model("o4-mini"));
        assert!(is_reasoning_model("gpt-5"));
        assert!(is_reasoning_model("gpt-5-pro"));
        assert!(!is_reasoning_model("gpt-4o"));
        assert!(!is_reasoning_model("gpt-4o-mini"));
        assert!(!is_reasoning_model("gpt-3.5-turbo"));
    }

    // Integration test - requires actual API key
    #[tokio::test]
    #[ignore = "e2e-network"]
    async fn test_generate_with_real_api() {
        let api_key = std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not set");
        let provider = OpenAIProvider::with_base_url(
            api_key,
            "gpt-4o-mini".to_string(),
            OPENAI_API_BASE.to_string(),
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
