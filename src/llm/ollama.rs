// Ollama local LLM provider implementation

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use super::{GenerationRequest, GenerationResponse, LlmProvider};
use crate::error::{LlmError, LlmResult};

#[derive(Debug, Serialize)]
struct OllamaRequest {
    model: String,
    prompt: String,
    stream: bool,
    options: OllamaOptions,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    /// Top-level think flag for Ollama thinking models.
    /// Must be at the request body top-level, NOT inside `options`.
    /// false = disable thinking; omitted (None) = model default.
    #[serde(skip_serializing_if = "Option::is_none")]
    think: Option<bool>,
}

#[derive(Debug, Serialize)]
struct OllamaOptions {
    temperature: f32,
    num_predict: u32,
}

#[derive(Debug, Deserialize)]
struct OllamaResponse {
    response: String,
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
            think: if request.disable_thinking {
                Some(false)
            } else {
                None
            },
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
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ollama_provider_creation() {
        let provider = OllamaProvider::with_base_url(
            "http://localhost:11434".to_string(),
            "llama3.1".to_string(),
            Duration::from_secs(30),
        );

        assert_eq!(provider.name(), "ollama");
        assert_eq!(provider.base_url, "http://localhost:11434");
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
        let provider = OllamaProvider::with_base_url(
            "http://localhost:11434".to_string(),
            "llama3.1".to_string(),
            Duration::from_secs(30),
        );

        // Ollama local service is always considered available
        assert!(provider.is_available());
    }

    #[test]
    fn test_extract_model_name() {
        let provider = OllamaProvider::with_base_url(
            "http://localhost:11434".to_string(),
            "ollama/llama3.1".to_string(),
            Duration::from_secs(30),
        );

        assert_eq!(provider.extract_model_name(), "llama3.1");
    }

    #[test]
    fn test_extract_model_name_without_prefix() {
        let provider = OllamaProvider::with_base_url(
            "http://localhost:11434".to_string(),
            "llama3.1".to_string(),
            Duration::from_secs(30),
        );

        assert_eq!(provider.extract_model_name(), "llama3.1");
    }

    // ── C2: OllamaRequestSerialization ──────────────────────────────────

    #[test]
    fn test_c2_think_wire_format() {
        let request = |think| OllamaRequest {
            model: "llama3.2".to_string(),
            prompt: "Hello".to_string(),
            stream: false,
            options: OllamaOptions {
                temperature: 0.3,
                num_predict: 100,
            },
            system: None,
            think,
        };

        let disabled = serde_json::to_value(request(Some(false))).unwrap();
        assert_eq!(disabled["think"], serde_json::Value::Bool(false));
        // think sits at the top level, never inside options
        assert!(disabled["options"].get("think").is_none());

        let omitted = serde_json::to_value(request(None)).unwrap();
        assert!(omitted.get("think").is_none());
        assert!(omitted["options"].get("think").is_none());
    }

    // Integration test - requires actual Ollama service running
    #[tokio::test]
    #[ignore = "e2e-network"]
    async fn test_generate_with_real_ollama() {
        let provider = OllamaProvider::with_base_url(
            "http://localhost:11434".to_string(),
            "llama3.1".to_string(),
            Duration::from_secs(60),
        );

        let request = GenerationRequest {
            system_message: None,
            prompt: "Say 'Hello' in one word".to_string(),
            max_tokens: 10,
            temperature: 0.3,
            disable_thinking: false,
        };

        let response = provider.generate(&request).await.unwrap();
        assert!(!response.text.is_empty());
        println!("Response: {}", response.text);
    }
}
