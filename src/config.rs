// Configuration loading and validation
// Unified config at ~/.claude/claude-voice.json with array-based provider fallback

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::error::{Result, VoiceError};

/// Default timeout in seconds for LLM requests
fn default_timeout() -> u64 {
    10
}

fn default_true() -> bool {
    true
}

fn default_version() -> String {
    "2.0.0".to_string()
}

fn default_max_length() -> usize {
    50
}

fn default_fallback_message() -> String {
    "任務已完成".to_string()
}

fn default_daily_limit() -> f64 {
    0.10
}

fn default_usage_file() -> String {
    "~/.claude/voice-usage.json".to_string()
}

fn default_max_tokens() -> u32 {
    100
}

fn default_temperature() -> f32 {
    0.3
}

fn default_prompt_template() -> String {
    "You are a voice notification assistant. Based on the following Claude Code conversation, generate a concise summary (max {max_length} words).\n\nConversation:\n{context}\n\nSummary:".to_string()
}

fn default_system_message() -> String {
    "You are a voice notification assistant for Claude Code. Generate concise summaries suitable for voice playback.".to_string()
}

fn default_notification_system_message() -> String {
    "You are a voice notification assistant for Claude Code. Convert important notifications into concise phrases suitable for voice playback.".to_string()
}

fn default_notification_prompt() -> String {
    "Convert the following notification message into a concise phrase suitable for voice playback (max 30 words):\n\n{message}\n\nVoice output:".to_string()
}

// ============================================================================
// LLM Provider Configuration
// ============================================================================

/// Individual LLM provider configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LlmProviderConfig {
    /// Provider name: google, anthropic, openai, ollama
    pub name: String,

    /// Model name (e.g., gemini-2.5-flash, gpt-4o-mini)
    pub model: String,

    /// API key (optional for ollama)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,

    /// Base URL (optional, for custom endpoints like ollama)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,

    /// Request timeout in seconds
    #[serde(default = "default_timeout")]
    pub timeout: u64,
}

impl LlmProviderConfig {
    /// Check if this provider has the required credentials
    pub fn has_credentials(&self) -> bool {
        match self.name.to_lowercase().as_str() {
            "ollama" | "local" => true, // No API key needed
            _ => self.api_key.as_ref().map_or(false, |k| !k.is_empty()),
        }
    }

    /// Get API key from config or environment variable
    pub fn get_api_key(&self) -> Option<String> {
        // Config value takes priority
        if let Some(ref key) = self.api_key {
            if !key.is_empty() && !key.starts_with("${") {
                return Some(key.clone());
            }
        }

        // Try environment variable
        let env_var = Self::env_var_name(&self.name);
        std::env::var(env_var).ok().filter(|k| !k.is_empty())
    }

    /// Get environment variable name for provider
    pub fn env_var_name(provider: &str) -> &'static str {
        match provider.to_lowercase().as_str() {
            "google" | "gemini" => "GEMINI_API_KEY",
            "anthropic" | "claude" => "ANTHROPIC_API_KEY",
            "openai" | "gpt" => "OPENAI_API_KEY",
            _ => "API_KEY",
        }
    }
}

/// LLM parameters shared across providers
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LlmParameters {
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,

    #[serde(default = "default_temperature")]
    pub temperature: f32,
}

impl Default for LlmParameters {
    fn default() -> Self {
        Self {
            max_tokens: default_max_tokens(),
            temperature: default_temperature(),
        }
    }
}

/// Cost control settings for LLM usage
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CostControlConfig {
    #[serde(default = "default_daily_limit")]
    pub daily_limit_usd: f64,

    #[serde(default = "default_true")]
    pub usage_tracking: bool,

    #[serde(default = "default_usage_file")]
    pub usage_file: String,
}

impl Default for CostControlConfig {
    fn default() -> Self {
        Self {
            daily_limit_usd: default_daily_limit(),
            usage_tracking: true,
            usage_file: default_usage_file(),
        }
    }
}

/// Complete LLM configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LlmConfig {
    /// Ordered list of LLM providers (fallback chain)
    pub providers: Vec<LlmProviderConfig>,

    /// Shared parameters for all providers
    #[serde(default)]
    pub parameters: LlmParameters,

    /// Cost control settings
    #[serde(default)]
    pub cost_control: CostControlConfig,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            providers: vec![
                LlmProviderConfig {
                    name: "google".to_string(),
                    model: "gemini-2.5-flash".to_string(),
                    api_key: None,
                    base_url: None,
                    timeout: default_timeout(),
                },
                LlmProviderConfig {
                    name: "ollama".to_string(),
                    model: "llama3.2".to_string(),
                    api_key: None,
                    base_url: None,
                    timeout: default_timeout(),
                },
            ],
            parameters: LlmParameters::default(),
            cost_control: CostControlConfig::default(),
        }
    }
}

// ============================================================================
// TTS Provider Configuration
// ============================================================================

/// Individual TTS provider configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TtsProviderConfig {
    /// Provider name: google, macos
    pub name: String,

    /// Voice name (provider-specific)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voice: Option<String>,

    /// API key (for google provider - Gemini API key)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,

    /// Speech rate for macOS (90-300)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate: Option<u32>,
}

impl TtsProviderConfig {
    /// Check if this TTS provider has the required configuration
    pub fn is_configured(&self) -> bool {
        match self.name.to_lowercase().as_str() {
            "macos" | "say" => true, // Always available on macOS
            "google" | "google_tts" | "gcloud" | "gemini" => {
                // Need API key from config or env
                self.get_api_key().is_some()
            }
            _ => false,
        }
    }

    /// Get Gemini API key from config or environment
    pub fn get_api_key(&self) -> Option<String> {
        // Config value takes priority
        if let Some(ref key) = self.api_key {
            if !key.is_empty() && !key.starts_with("${") {
                return Some(key.clone());
            }
        }

        // Try environment variables
        std::env::var("GEMINI_API_KEY")
            .ok()
            .or_else(|| std::env::var("GOOGLE_API_KEY").ok())
            .filter(|k| !k.is_empty())
    }
}

/// Complete TTS configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TtsConfig {
    /// Ordered list of TTS providers (fallback chain)
    pub providers: Vec<TtsProviderConfig>,
}

impl Default for TtsConfig {
    fn default() -> Self {
        Self {
            providers: vec![
                TtsProviderConfig {
                    name: "google".to_string(),
                    voice: Some("Aoede".to_string()),
                    api_key: None,
                    rate: None,
                },
                TtsProviderConfig {
                    name: "macos".to_string(),
                    voice: Some("Ting-Ting".to_string()),
                    api_key: None,
                    rate: Some(200),
                },
            ],
        }
    }
}

// ============================================================================
// Summarization Configuration
// ============================================================================

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SummarizationConfig {
    #[serde(default = "default_max_length")]
    pub max_length: usize,

    #[serde(default = "default_prompt_template")]
    pub prompt_template: String,

    #[serde(default = "default_system_message")]
    pub system_message: String,

    #[serde(default = "default_notification_system_message")]
    pub notification_system_message: String,

    #[serde(default = "default_notification_prompt")]
    pub notification_prompt: String,
}

impl Default for SummarizationConfig {
    fn default() -> Self {
        Self {
            max_length: default_max_length(),
            prompt_template: default_prompt_template(),
            system_message: default_system_message(),
            notification_system_message: default_notification_system_message(),
            notification_prompt: default_notification_prompt(),
        }
    }
}

// ============================================================================
// Advanced Configuration
// ============================================================================

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AdvancedConfig {
    #[serde(default = "default_fallback_message")]
    pub fallback_message: String,
}

impl Default for AdvancedConfig {
    fn default() -> Self {
        Self {
            fallback_message: default_fallback_message(),
        }
    }
}

// ============================================================================
// Main VoiceConfig
// ============================================================================

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VoiceConfig {
    #[serde(default = "default_version")]
    pub version: String,

    #[serde(default = "default_true")]
    pub enabled: bool,

    #[serde(default)]
    pub llm: LlmConfig,

    #[serde(default)]
    pub tts: TtsConfig,

    #[serde(default)]
    pub summarization: SummarizationConfig,

    #[serde(default)]
    pub advanced: AdvancedConfig,
}

impl Default for VoiceConfig {
    fn default() -> Self {
        Self {
            version: default_version(),
            enabled: true,
            llm: LlmConfig::default(),
            tts: TtsConfig::default(),
            summarization: SummarizationConfig::default(),
            advanced: AdvancedConfig::default(),
        }
    }
}

impl VoiceConfig {
    /// Get the standard config path: ~/.claude/claude-voice.json
    pub fn config_path() -> Result<PathBuf> {
        let home = dirs::home_dir()
            .ok_or_else(|| VoiceError::Config("Cannot find home directory".into()))?;
        Ok(home.join(".claude").join("claude-voice.json"))
    }

    /// Load configuration from ~/.claude/claude-voice.json
    pub fn load_from_home() -> Result<Self> {
        let config_path = Self::config_path()?;

        if !config_path.exists() {
            tracing::info!(
                "Config file not found at {:?}, using defaults",
                config_path
            );
            return Ok(Self::default());
        }

        Self::load(config_path)
    }

    /// Load configuration from a specific path
    pub fn load(path: PathBuf) -> Result<Self> {
        let content = std::fs::read_to_string(&path).map_err(|e| {
            VoiceError::Config(format!("Failed to read config file {:?}: {}", path, e))
        })?;

        let config: VoiceConfig = serde_json::from_str(&content)?;
        config.validate()?;
        Ok(config)
    }

    /// Save configuration to ~/.claude/claude-voice.json
    pub fn save_to_home(&self) -> Result<()> {
        let config_path = Self::config_path()?;
        self.save(config_path)
    }

    /// Save configuration to a specific path
    pub fn save(&self, path: PathBuf) -> Result<()> {
        // Ensure directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, json)?;

        tracing::info!("Config saved to {:?}", path);
        Ok(())
    }

    /// Validate configuration
    fn validate(&self) -> Result<()> {
        // Validate LLM parameters
        if self.llm.parameters.temperature < 0.0 || self.llm.parameters.temperature > 2.0 {
            return Err(VoiceError::Config(format!(
                "Temperature {} out of range [0.0-2.0]",
                self.llm.parameters.temperature
            )));
        }

        if self.llm.parameters.max_tokens == 0 {
            return Err(VoiceError::Config(
                "max_tokens must be greater than 0".to_string(),
            ));
        }

        // Validate cost control
        if self.llm.cost_control.daily_limit_usd < 0.0 {
            return Err(VoiceError::Config(
                "daily_limit_usd cannot be negative".to_string(),
            ));
        }

        // Validate TTS rate if specified
        for tts in &self.tts.providers {
            if let Some(rate) = tts.rate {
                if rate < 90 || rate > 300 {
                    return Err(VoiceError::Config(format!(
                        "TTS rate {} out of range [90-300] for provider {}",
                        rate, tts.name
                    )));
                }
            }
        }

        // Validate prompt templates contain required variables (warnings only)
        if !self.summarization.prompt_template.contains("{max_length}")
            || !self.summarization.prompt_template.contains("{context}")
        {
            tracing::warn!(
                "Stop hook prompt_template missing required variables: {{max_length}} or {{context}}"
            );
        }

        if !self.summarization.notification_prompt.contains("{message}") {
            tracing::warn!(
                "Notification hook notification_prompt missing required variable: {{message}}"
            );
        }

        Ok(())
    }

    /// Find the first available LLM provider with credentials
    pub fn find_available_llm_provider(&self) -> Option<&LlmProviderConfig> {
        self.llm.providers.iter().find(|p| p.has_credentials())
    }

    /// Find the first available TTS provider
    pub fn find_available_tts_provider(&self) -> Option<&TtsProviderConfig> {
        self.tts.providers.iter().find(|p| p.is_configured())
    }

    /// Update API key for a specific LLM provider
    pub fn set_llm_api_key(&mut self, provider_name: &str, api_key: &str) -> bool {
        for provider in &mut self.llm.providers {
            if provider.name.to_lowercase() == provider_name.to_lowercase() {
                provider.api_key = Some(api_key.to_string());
                return true;
            }
        }

        // Provider not found, add new one
        self.llm.providers.push(LlmProviderConfig {
            name: provider_name.to_string(),
            model: Self::default_model_for_provider(provider_name),
            api_key: Some(api_key.to_string()),
            base_url: None,
            timeout: default_timeout(),
        });
        true
    }

    /// Update API key for Google TTS (Gemini API key)
    pub fn set_tts_api_key(&mut self, api_key: &str) -> bool {
        for provider in &mut self.tts.providers {
            if provider.name.to_lowercase() == "google" {
                provider.api_key = Some(api_key.to_string());
                return true;
            }
        }

        // Add Google TTS provider if not exists
        self.tts.providers.insert(
            0,
            TtsProviderConfig {
                name: "google".to_string(),
                voice: Some("Aoede".to_string()),
                api_key: Some(api_key.to_string()),
                rate: None,
            },
        );
        true
    }

    /// Get default model for a provider
    fn default_model_for_provider(provider: &str) -> String {
        match provider.to_lowercase().as_str() {
            "google" | "gemini" => "gemini-2.5-flash".to_string(),
            "anthropic" | "claude" => "claude-3-haiku-20240307".to_string(),
            "openai" | "gpt" => "gpt-4o-mini".to_string(),
            "ollama" | "local" => "llama3.2".to_string(),
            _ => "unknown".to_string(),
        }
    }

    /// List configured LLM providers
    pub fn list_llm_providers(&self) -> Vec<(&str, bool)> {
        self.llm
            .providers
            .iter()
            .map(|p| (p.name.as_str(), p.has_credentials()))
            .collect()
    }

    /// List configured TTS providers
    pub fn list_tts_providers(&self) -> Vec<(&str, bool)> {
        self.tts
            .providers
            .iter()
            .map(|p| (p.name.as_str(), p.is_configured()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_default_config() {
        let config = VoiceConfig::default();
        assert_eq!(config.version, "2.0.0");
        assert!(config.enabled);
        assert!(!config.llm.providers.is_empty());
        assert!(!config.tts.providers.is_empty());
    }

    #[test]
    fn test_load_new_format() {
        let config_json = r#"{
            "version": "2.0.0",
            "enabled": true,
            "llm": {
                "providers": [
                    {
                        "name": "google",
                        "model": "gemini-2.5-flash",
                        "api_key": "test-key",
                        "timeout": 10
                    },
                    {
                        "name": "ollama",
                        "model": "llama3.2"
                    }
                ],
                "parameters": {
                    "max_tokens": 100,
                    "temperature": 0.3
                }
            },
            "tts": {
                "providers": [
                    {
                        "name": "macos",
                        "voice": "Ting-Ting",
                        "rate": 200
                    }
                ]
            }
        }"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(config_json.as_bytes()).unwrap();
        let path = temp_file.path().to_path_buf();

        let config = VoiceConfig::load(path).unwrap();
        assert_eq!(config.version, "2.0.0");
        assert_eq!(config.llm.providers.len(), 2);
        assert_eq!(config.llm.providers[0].name, "google");
        assert_eq!(
            config.llm.providers[0].api_key,
            Some("test-key".to_string())
        );
        assert_eq!(config.tts.providers[0].name, "macos");
    }

    #[test]
    fn test_provider_has_credentials() {
        let provider_with_key = LlmProviderConfig {
            name: "google".to_string(),
            model: "gemini-2.5-flash".to_string(),
            api_key: Some("test-key".to_string()),
            base_url: None,
            timeout: 10,
        };
        assert!(provider_with_key.has_credentials());

        let provider_without_key = LlmProviderConfig {
            name: "google".to_string(),
            model: "gemini-2.5-flash".to_string(),
            api_key: None,
            base_url: None,
            timeout: 10,
        };
        assert!(!provider_without_key.has_credentials());

        let ollama_provider = LlmProviderConfig {
            name: "ollama".to_string(),
            model: "llama3.2".to_string(),
            api_key: None,
            base_url: None,
            timeout: 10,
        };
        assert!(ollama_provider.has_credentials()); // Ollama doesn't need API key
    }

    #[test]
    fn test_tts_is_configured() {
        let macos_provider = TtsProviderConfig {
            name: "macos".to_string(),
            voice: Some("Ting-Ting".to_string()),
            api_key: None,
            rate: Some(200),
        };
        assert!(macos_provider.is_configured());

        let _google_without_api_key = TtsProviderConfig {
            name: "google".to_string(),
            voice: Some("Aoede".to_string()),
            api_key: None,
            rate: None,
        };
        // This will be false unless GEMINI_API_KEY env var is set
        // In test, we can't guarantee env var state
    }

    #[test]
    fn test_set_llm_api_key() {
        let mut config = VoiceConfig::default();

        // Update existing provider
        config.set_llm_api_key("google", "new-key");
        assert_eq!(
            config.llm.providers[0].api_key,
            Some("new-key".to_string())
        );

        // Add new provider
        let initial_count = config.llm.providers.len();
        config.set_llm_api_key("anthropic", "anthropic-key");
        assert_eq!(config.llm.providers.len(), initial_count + 1);
    }

    #[test]
    fn test_validate_invalid_temperature() {
        let mut config = VoiceConfig::default();
        config.llm.parameters.temperature = 3.0;

        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Temperature 3 out of range"));
    }

    #[test]
    fn test_validate_invalid_tts_rate() {
        let mut config = VoiceConfig::default();
        config.tts.providers[1].rate = Some(500);

        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("TTS rate 500 out of range"));
    }

    #[test]
    fn test_save_and_load() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("test-config.json");

        let mut config = VoiceConfig::default();
        config.set_llm_api_key("google", "test-key");

        config.save(path.clone()).unwrap();

        let loaded = VoiceConfig::load(path).unwrap();
        assert_eq!(
            loaded.llm.providers[0].api_key,
            Some("test-key".to_string())
        );
    }

    #[test]
    fn test_find_available_provider() {
        let mut config = VoiceConfig::default();

        // Initially, only ollama is available (no API key required)
        let available = config.find_available_llm_provider();
        assert!(available.is_some());
        assert_eq!(available.unwrap().name, "ollama");

        // Set Google API key
        config.set_llm_api_key("google", "test-key");
        let available = config.find_available_llm_provider();
        assert!(available.is_some());
        assert_eq!(available.unwrap().name, "google"); // First provider with credentials
    }

    #[test]
    fn test_env_var_name() {
        assert_eq!(LlmProviderConfig::env_var_name("google"), "GEMINI_API_KEY");
        assert_eq!(LlmProviderConfig::env_var_name("gemini"), "GEMINI_API_KEY");
        assert_eq!(
            LlmProviderConfig::env_var_name("anthropic"),
            "ANTHROPIC_API_KEY"
        );
        assert_eq!(LlmProviderConfig::env_var_name("openai"), "OPENAI_API_KEY");
    }
}
