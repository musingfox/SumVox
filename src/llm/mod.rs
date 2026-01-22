// LLM provider abstraction and implementations

use async_trait::async_trait;

pub use anthropic::AnthropicProvider;
pub use cost_tracker::CostTracker;
pub use gemini::GeminiProvider;
pub use ollama::OllamaProvider;
pub use openai::OpenAIProvider;

pub mod anthropic;
pub mod cost_tracker;
pub mod gemini;
pub mod ollama;
pub mod openai;

use crate::error::LlmResult;

#[derive(Debug, Clone)]
pub struct GenerationRequest {
    pub prompt: String,
    pub max_tokens: u32,
    pub temperature: f32,
}

#[derive(Debug, Clone)]
pub struct GenerationResponse {
    pub text: String,
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub model: String,
}

#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Get provider name
    fn name(&self) -> &str;

    /// Check if provider is available (API key set, etc.)
    fn is_available(&self) -> bool;

    /// Generate text from prompt
    async fn generate(&self, request: &GenerationRequest) -> LlmResult<GenerationResponse>;

    /// Estimate cost for a request
    fn estimate_cost(&self, input_tokens: u32, output_tokens: u32) -> f64;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generation_request_creation() {
        let request = GenerationRequest {
            prompt: "Test prompt".to_string(),
            max_tokens: 100,
            temperature: 0.3,
        };

        assert_eq!(request.prompt, "Test prompt");
        assert_eq!(request.max_tokens, 100);
        assert_eq!(request.temperature, 0.3);
    }

    #[test]
    fn test_generation_response_creation() {
        let response = GenerationResponse {
            text: "Generated text".to_string(),
            input_tokens: 10,
            output_tokens: 20,
            model: "test-model".to_string(),
        };

        assert_eq!(response.text, "Generated text");
        assert_eq!(response.input_tokens, 10);
        assert_eq!(response.output_tokens, 20);
        assert_eq!(response.model, "test-model");
    }
}
