// Gemini API provider implementation

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use super::{GenerationRequest, GenerationResponse, LlmProvider};
use crate::error::{LlmError, LlmResult};

pub(crate) const GEMINI_API_BASE: &str = "https://generativelanguage.googleapis.com/v1beta";

#[derive(Debug, Serialize)]
struct GeminiRequest {
    contents: Vec<Content>,
    #[serde(rename = "generationConfig")]
    generation_config: GenerationConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    system_instruction: Option<SystemInstruction>,
}

#[derive(Debug, Serialize)]
struct SystemInstruction {
    parts: Vec<Part>,
}

#[derive(Debug, Serialize)]
struct Content {
    parts: Vec<Part>,
}

#[derive(Debug, Serialize)]
struct Part {
    text: String,
}

#[derive(Debug, Serialize)]
struct ThinkingConfig {
    /// Values: 0 = disable, -1 = dynamic, >0 = token budget
    /// API docs: https://ai.google.dev/gemini-api/docs/thinking
    #[serde(rename = "thinkingBudget")]
    thinking_budget: i32,
}

#[derive(Debug, Serialize)]
struct GenerationConfig {
    temperature: f32,
    #[serde(rename = "maxOutputTokens")]
    max_output_tokens: u32,

    /// Thinking configuration — send only when disable_thinking=true to set budget=0.
    /// When None (disable_thinking=false), the field is omitted entirely.
    #[serde(skip_serializing_if = "Option::is_none", rename = "thinkingConfig")]
    thinking_config: Option<ThinkingConfig>,
}

#[derive(Debug, Deserialize)]
struct GeminiResponse {
    candidates: Vec<Candidate>,
    #[serde(rename = "usageMetadata")]
    usage_metadata: Option<UsageMetadata>,
}

#[derive(Debug, Deserialize)]
struct Candidate {
    content: ResponseContent,
}

#[derive(Debug, Deserialize)]
struct ResponseContent {
    parts: Vec<ResponsePart>,
}

#[derive(Debug, Deserialize)]
struct ResponsePart {
    text: String,
}

#[derive(Debug, Deserialize)]
struct UsageMetadata {
    #[serde(rename = "promptTokenCount")]
    prompt_token_count: u32,
    #[serde(rename = "candidatesTokenCount")]
    candidates_token_count: u32,
}

pub struct GeminiProvider {
    api_key: String,
    model: String,
    base_url: String,
    timeout: Duration,
}

impl GeminiProvider {
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
        crate::http::client(self.timeout).unwrap_or_else(|_| Client::new())
    }

    fn extract_model_name(&self) -> &str {
        // Handle "gemini/gemini-2.0-flash-exp" -> "gemini-2.0-flash-exp"
        if let Some(idx) = self.model.find('/') {
            &self.model[idx + 1..]
        } else {
            &self.model
        }
    }
}

#[async_trait]
impl LlmProvider for GeminiProvider {
    fn name(&self) -> &str {
        "gemini"
    }

    fn is_available(&self) -> bool {
        crate::config::is_usable_key(&self.api_key)
    }

    async fn generate(&self, request: &GenerationRequest) -> LlmResult<GenerationResponse> {
        if !self.is_available() {
            return Err(LlmError::Unavailable(
                "Gemini API key not configured".to_string(),
            ));
        }

        let model_name = self.extract_model_name();
        let url = format!(
            "{}/models/{}:generateContent?key={}",
            self.base_url, model_name, self.api_key
        );

        let system_instruction = request
            .system_message
            .as_ref()
            .map(|msg| SystemInstruction {
                parts: vec![Part { text: msg.clone() }],
            });

        // Set thinkingConfig based solely on disable_thinking flag.
        // disable_thinking=true  → send thinkingConfig.thinkingBudget=0 (disable thinking)
        // disable_thinking=false → omit thinkingConfig entirely (model default)
        let thinking_config = if request.disable_thinking {
            Some(ThinkingConfig { thinking_budget: 0 })
        } else {
            None
        };

        let gemini_request = GeminiRequest {
            contents: vec![Content {
                parts: vec![Part {
                    text: request.prompt.clone(),
                }],
            }],
            generation_config: GenerationConfig {
                temperature: request.temperature,
                max_output_tokens: request.max_tokens,
                thinking_config,
            },
            system_instruction,
        };

        tracing::debug!("Sending request to Gemini API: {}", model_name);

        let response = self
            .client()
            .post(&url)
            .json(&gemini_request)
            .send()
            .await
            .map_err(|e| LlmError::Request(format!("Gemini API request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(LlmError::Request(format!(
                "Gemini API returned {}: {}",
                status, error_text
            )));
        }

        let response_text = response
            .text()
            .await
            .map_err(|e| LlmError::Request(format!("Failed to read Gemini response: {}", e)))?;

        let gemini_response: GeminiResponse = serde_json::from_str(&response_text)
            .map_err(|e| LlmError::Request(format!("Failed to parse Gemini response: {}", e)))?;

        if gemini_response.candidates.is_empty() {
            return Err(LlmError::Request(
                "No candidates in Gemini response".to_string(),
            ));
        }

        let text = gemini_response.candidates[0]
            .content
            .parts
            .iter()
            .map(|p| p.text.as_str())
            .collect::<Vec<_>>()
            .join("");

        let (input_tokens, output_tokens) = if let Some(usage) = gemini_response.usage_metadata {
            (usage.prompt_token_count, usage.candidates_token_count)
        } else {
            // Estimate if not provided
            ((request.prompt.len() / 4) as u32, (text.len() / 4) as u32)
        };

        Ok(GenerationResponse {
            text,
            input_tokens,
            output_tokens,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_available_with_empty_key() {
        // (api key, expected): empty and `${...}` placeholder keys are unusable
        for (key, expected) in [
            ("test-key", true),
            ("", false),
            ("${GEMINI_API_KEY}", false),
        ] {
            let provider = GeminiProvider::with_base_url(
                key.to_string(),
                "gemini/gemini-2.0-flash-exp".to_string(),
                GEMINI_API_BASE.to_string(),
                Duration::from_secs(10),
            );
            assert_eq!(provider.is_available(), expected, "key={key:?}");
            assert_eq!(provider.name(), "gemini");
        }
    }

    #[test]
    fn test_extract_model_name() {
        for model in ["gemini/gemini-2.0-flash-exp", "gemini-2.0-flash-exp"] {
            let provider = GeminiProvider::with_base_url(
                "test-key".to_string(),
                model.to_string(),
                GEMINI_API_BASE.to_string(),
                Duration::from_secs(10),
            );
            assert_eq!(provider.extract_model_name(), "gemini-2.0-flash-exp");
        }
    }

    #[tokio::test]
    async fn test_generate_with_unavailable_provider() {
        let provider = GeminiProvider::with_base_url(
            "".to_string(),
            "gemini/gemini-2.0-flash-exp".to_string(),
            GEMINI_API_BASE.to_string(),
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

    // ── C3: GeminiRequestSerialization ──────────────────────────────────

    #[test]
    fn test_c3_thinking_config_wire_format() {
        let config = |thinking_config| GenerationConfig {
            temperature: 0.3,
            max_output_tokens: 100,
            thinking_config,
        };

        let disabled =
            serde_json::to_value(config(Some(ThinkingConfig { thinking_budget: 0 }))).unwrap();
        assert_eq!(disabled["thinkingConfig"]["thinkingBudget"], 0);

        let omitted = serde_json::to_value(config(None)).unwrap();
        assert!(omitted.get("thinkingConfig").is_none());
    }

    // Integration test - requires actual API key
    #[tokio::test]
    #[ignore = "e2e-network"]
    async fn test_generate_with_real_api() {
        let api_key = std::env::var("GEMINI_API_KEY").expect("GEMINI_API_KEY not set");
        let provider = GeminiProvider::with_base_url(
            api_key,
            "gemini/gemini-2.0-flash-exp".to_string(),
            GEMINI_API_BASE.to_string(),
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
