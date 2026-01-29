// Ollama local LLM provider implementation

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use super::{GenerationRequest, GenerationResponse, LlmProvider};
use crate::error::{LlmError, LlmResult};

const DEFAULT_OLLAMA_BASE_URL: &str = "http://localhost:11434";

#[derive(Debug, Serialize)]
struct OllamaRequest {
    model: String,
    prompt: String,
    stream: bool,
    options: OllamaOptions,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
}

#[derive(Debug, Serialize)]
struct OllamaOptions {
    temperature: f32,
    num_predict: u32,
}

#[derive(Debug, Deserialize)]
struct OllamaResponse {
    #[allow(dead_code)]
    model: String,
    response: String,
    #[allow(dead_code)]
    done: bool,
    #[serde(default)]
    prompt_eval_count: u32,
    #[serde(default)]
    eval_count: u32,
}

pub struct OllamaProvider {
    base_url: String,
    model: String,
    timeout: Duration,
}

impl OllamaProvider {
    pub fn new(model: String, timeout: Duration) -> Self {
        Self {
            base_url: DEFAULT_OLLAMA_BASE_URL.to_string(),
            model,
            timeout,
        }
    }

    pub fn with_base_url(base_url: String, model: String, timeout: Duration) -> Self {
        Self {
            base_url,
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
        // Handle "ollama/llama3.2" -> "llama3.2"
        if let Some(idx) = self.model.find('/') {
            &self.model[idx + 1..]
        } else {
            &self.model
        }
    }
}

#[async_trait]
impl LlmProvider for OllamaProvider {
    fn name(&self) -> &str {
        "ollama"
    }

    fn is_available(&self) -> bool {
        // Ollama is a local service, assume it's available
        // Could optionally ping the service here
        true
    }

    async fn generate(&self, request: &GenerationRequest) -> LlmResult<GenerationResponse> {
        let model_name = self.extract_model_name();
        let url = format!("{}/api/generate", self.base_url);

        let ollama_request = OllamaRequest {
            model: model_name.to_string(),
            prompt: request.prompt.clone(),
            stream: false,
            options: OllamaOptions {
                temperature: request.temperature,
                num_predict: request.max_tokens,
            },
            system: request.system_message.clone(),
        };

        tracing::debug!("Sending request to Ollama API: {}", model_name);

        let response = self
            .client()
            .post(&url)
            .json(&ollama_request)
            .send()
            .await
            .map_err(|e| LlmError::Request(format!("Ollama API request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(LlmError::Request(format!(
                "Ollama API returned {}: {}",
                status, error_text
            )));
        }

        let ollama_response: OllamaResponse = response
            .json()
            .await
            .map_err(|e| LlmError::Request(format!("Failed to parse Ollama response: {}", e)))?;

        Ok(GenerationResponse {
            text: ollama_response.response,
            input_tokens: ollama_response.prompt_eval_count,
            output_tokens: ollama_response.eval_count,
            model: self.model.clone(),
        })
    }

    fn estimate_cost(&self, _input_tokens: u32, _output_tokens: u32) -> f64 {
        // Ollama is free (local)
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ollama_provider_creation() {
        let provider = OllamaProvider::new(
            "llama3.1".to_string(),
            Duration::from_secs(30),
        );

        assert_eq!(provider.name(), "ollama");
        assert_eq!(provider.base_url, DEFAULT_OLLAMA_BASE_URL);
        assert!(provider.is_available());
    }

    #[test]
    fn test_ollama_provider_with_custom_base_url() {
        let provider = OllamaProvider::with_base_url(
            "http://custom:11434".to_string(),
            "llama3.1".to_string(),
            Duration::from_secs(30),
        );

        assert_eq!(provider.base_url, "http://custom:11434");
    }

    #[test]
    fn test_is_available() {
        let provider = OllamaProvider::new(
            "llama3.1".to_string(),
            Duration::from_secs(30),
        );

        // Ollama local service is always considered available
        assert!(provider.is_available());
    }

    #[test]
    fn test_extract_model_name() {
        let provider = OllamaProvider::new(
            "ollama/llama3.1".to_string(),
            Duration::from_secs(30),
        );

        assert_eq!(provider.extract_model_name(), "llama3.1");
    }

    #[test]
    fn test_extract_model_name_without_prefix() {
        let provider = OllamaProvider::new(
            "llama3.1".to_string(),
            Duration::from_secs(30),
        );

        assert_eq!(provider.extract_model_name(), "llama3.1");
    }

    #[test]
    fn test_estimate_cost_is_zero() {
        let provider = OllamaProvider::new(
            "llama3.1".to_string(),
            Duration::from_secs(30),
        );

        let cost = provider.estimate_cost(1000, 1000);
        assert_eq!(cost, 0.0);
    }

    // Integration test - requires actual Ollama service running
    #[tokio::test]
    #[ignore]
    async fn test_generate_with_real_ollama() {
        let provider = OllamaProvider::new(
            "llama3.1".to_string(),
            Duration::from_secs(60),
        );

        let request = GenerationRequest {
            system_message: None,
            prompt: "Say 'Hello' in one word".to_string(),
            max_tokens: 10,
            temperature: 0.3,
        };

        let response = provider.generate(&request).await.unwrap();
        assert!(!response.text.is_empty());
        println!("Response: {}", response.text);
    }
}
