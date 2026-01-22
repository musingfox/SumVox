// Gemini API provider implementation

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use super::{GenerationRequest, GenerationResponse, LlmProvider};
use crate::error::{LlmError, LlmResult};

const GEMINI_API_BASE: &str = "https://generativelanguage.googleapis.com/v1beta";

#[derive(Debug, Serialize)]
struct GeminiRequest {
    contents: Vec<Content>,
    #[serde(rename = "generationConfig")]
    generation_config: GenerationConfig,
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
struct GenerationConfig {
    temperature: f32,
    #[serde(rename = "maxOutputTokens")]
    max_output_tokens: u32,
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
    timeout: Duration,
}

impl GeminiProvider {
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
        !self.api_key.is_empty() && !self.api_key.starts_with("${")
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
            GEMINI_API_BASE, model_name, self.api_key
        );

        let gemini_request = GeminiRequest {
            contents: vec![Content {
                parts: vec![Part {
                    text: request.prompt.clone(),
                }],
            }],
            generation_config: GenerationConfig {
                temperature: request.temperature,
                max_output_tokens: request.max_tokens,
            },
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

        let gemini_response: GeminiResponse = response
            .json()
            .await
            .map_err(|e| LlmError::Request(format!("Failed to parse Gemini response: {}", e)))?;

        if gemini_response.candidates.is_empty() {
            return Err(LlmError::Request("No candidates in Gemini response".to_string()));
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
            (
                (request.prompt.len() / 4) as u32,
                (text.len() / 4) as u32,
            )
        };

        Ok(GenerationResponse {
            text,
            input_tokens,
            output_tokens,
            model: self.model.clone(),
        })
    }

    fn estimate_cost(&self, input_tokens: u32, output_tokens: u32) -> f64 {
        // Gemini Flash 2.0 pricing (per 1K tokens)
        const INPUT_COST_PER_1K: f64 = 0.000075;
        const OUTPUT_COST_PER_1K: f64 = 0.00030;

        let input_cost = (input_tokens as f64 / 1000.0) * INPUT_COST_PER_1K;
        let output_cost = (output_tokens as f64 / 1000.0) * OUTPUT_COST_PER_1K;

        input_cost + output_cost
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gemini_provider_creation() {
        let provider = GeminiProvider::new(
            "test-key".to_string(),
            "gemini/gemini-2.0-flash-exp".to_string(),
            Duration::from_secs(10),
        );

        assert_eq!(provider.name(), "gemini");
        assert!(provider.is_available());
    }

    #[test]
    fn test_is_available_with_empty_key() {
        let provider = GeminiProvider::new(
            "".to_string(),
            "gemini/gemini-2.0-flash-exp".to_string(),
            Duration::from_secs(10),
        );

        assert!(!provider.is_available());
    }

    #[test]
    fn test_is_available_with_env_var_placeholder() {
        let provider = GeminiProvider::new(
            "${GEMINI_API_KEY}".to_string(),
            "gemini/gemini-2.0-flash-exp".to_string(),
            Duration::from_secs(10),
        );

        assert!(!provider.is_available());
    }

    #[test]
    fn test_extract_model_name() {
        let provider = GeminiProvider::new(
            "test-key".to_string(),
            "gemini/gemini-2.0-flash-exp".to_string(),
            Duration::from_secs(10),
        );

        assert_eq!(provider.extract_model_name(), "gemini-2.0-flash-exp");
    }

    #[test]
    fn test_extract_model_name_without_prefix() {
        let provider = GeminiProvider::new(
            "test-key".to_string(),
            "gemini-2.0-flash-exp".to_string(),
            Duration::from_secs(10),
        );

        assert_eq!(provider.extract_model_name(), "gemini-2.0-flash-exp");
    }

    #[test]
    fn test_estimate_cost() {
        let provider = GeminiProvider::new(
            "test-key".to_string(),
            "gemini/gemini-2.0-flash-exp".to_string(),
            Duration::from_secs(10),
        );

        let cost = provider.estimate_cost(1000, 1000);
        // 1000 * 0.000075 + 1000 * 0.00030 = 0.000375
        assert!((cost - 0.000375).abs() < 0.000001);
    }

    #[tokio::test]
    async fn test_generate_with_unavailable_provider() {
        let provider = GeminiProvider::new(
            "".to_string(),
            "gemini/gemini-2.0-flash-exp".to_string(),
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
        let api_key = std::env::var("GEMINI_API_KEY").expect("GEMINI_API_KEY not set");
        let provider = GeminiProvider::new(
            api_key,
            "gemini/gemini-2.0-flash-exp".to_string(),
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
