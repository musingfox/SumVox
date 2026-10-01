// LLM provider abstraction and implementations

use async_trait::async_trait;

pub use anthropic::AnthropicProvider;
pub use gemini::GeminiProvider;
pub use ollama::OllamaProvider;
pub use openai::OpenAIProvider;

pub(crate) use anthropic::ANTHROPIC_API_BASE;
pub(crate) use gemini::GEMINI_API_BASE;
pub(crate) use openai::OPENAI_API_BASE;

pub mod anthropic;
pub mod gemini;
pub mod ollama;
pub mod openai;

use crate::error::LlmResult;

#[derive(Debug, Clone)]
pub struct GenerationRequest {
    pub system_message: Option<String>,
    pub prompt: String,
    pub max_tokens: u32,
    pub temperature: f32,
    pub disable_thinking: bool,
}

#[derive(Debug, Clone)]
pub struct GenerationResponse {
    pub text: String,
    pub input_tokens: u32,
    pub output_tokens: u32,
}

#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Get provider name
    fn name(&self) -> &str;

    /// Check if provider is available (API key set, etc.)
    fn is_available(&self) -> bool;

    /// Generate text from prompt
    async fn generate(&self, request: &GenerationRequest) -> LlmResult<GenerationResponse>;
}
