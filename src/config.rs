// Configuration loading and validation
// Unified config at ~/.config/sumvox/config.json with array-based provider fallback

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::error::{Result, VoiceError};

/// Default timeout in seconds for LLM requests
fn default_timeout() -> u64 {
    10
}

/// Ollama needs longer timeout for local inference
fn default_ollama_timeout() -> u64 {
    60
}

/// Serialize API key, converting None to placeholder
fn serialize_api_key<S>(key: &Option<String>, serializer: S) -> std::result::Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    use serde::Serialize;
    match key {
        Some(k) if !k.is_empty() && !k.starts_with("${") => k.serialize(serializer),
        _ => "${PROVIDER_API_KEY}".serialize(serializer),
    }
}

fn default_version() -> String {
    "1.0.0".to_string()
}

fn default_turns() -> usize {
    1
}

fn default_fallback_message() -> String {
    "Task completed".to_string()
}

fn default_max_tokens() -> u32 {
    10000
}

fn default_temperature() -> f32 {
    0.3
}

fn default_prompt_template() -> String {
    "Based on the following context, generate a concise summary.\n\nContext:\n{context}\n\nSummary:".to_string()
}

fn default_system_message() -> String {
    "You are a voice notification assistant. Generate concise summaries suitable for voice playback.".to_string()
}

fn default_notification_filter() -> Vec<String> {
    vec![
        "permission_prompt".to_string(),
        "idle_prompt".to_string(),
        "elicitation_dialog".to_string(),
    ]
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
    #[serde(default, serialize_with = "serialize_api_key")]
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
            _ => self.api_key.as_ref().is_some_and(|k| !k.is_empty()),
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

    /// Disable thinking/reasoning to reduce token usage
    #[serde(default)]
    pub disable_thinking: bool,
}

impl Default for LlmParameters {
    fn default() -> Self {
        Self {
            max_tokens: default_max_tokens(),
            temperature: default_temperature(),
            disable_thinking: false,
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
                    timeout: default_ollama_timeout(),
                },
            ],
            parameters: LlmParameters::default(),
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

    /// TTS model name (optional, provider-specific)
    /// - For Google TTS: REQUIRED, e.g., "gemini-2.5-flash-preview-tts"
    /// - For macOS say: not used, should be None
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// Voice name (provider-specific)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voice: Option<String>,

    /// API key (for google provider - Gemini API key)
    #[serde(default, serialize_with = "serialize_api_key")]
    pub api_key: Option<String>,

    /// Speech rate for macOS (90-300)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate: Option<u32>,

    /// Volume level (0-100), applies to both macOS and Google TTS
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volume: Option<u32>,
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
                    model: Some("gemini-2.5-flash-preview-tts".to_string()),
                    voice: Some("Zephyr".to_string()),
                    api_key: None,
                    rate: None,
                    volume: None,
                },
                TtsProviderConfig {
                    name: "macos".to_string(),
                    model: None,
                    voice: None,
                    api_key: None,
                    rate: Some(200),
                    volume: None,
                },
            ],
        }
    }
}

// ============================================================================
// Summarization Configuration (generic, used by sum command and hooks)
// ============================================================================

/// Summarization configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SummarizationConfig {
    /// Number of conversation turns to summarize (default: 1)
    /// A turn is from a user message to the next user message or EOF
    #[serde(default = "default_turns")]
    pub turns: usize,

    /// System message for summarization
    #[serde(default = "default_system_message")]
    pub system_message: String,

    /// Prompt template for summarization
    #[serde(default = "default_prompt_template")]
    pub prompt_template: String,

    /// Fallback message when summarization fails
    #[serde(default = "default_fallback_message")]
    pub fallback_message: String,
}

impl Default for SummarizationConfig {
    fn default() -> Self {
        Self {
            turns: default_turns(),
            system_message: default_system_message(),
            prompt_template: default_prompt_template(),
            fallback_message: default_fallback_message(),
        }
    }
}

// ============================================================================
// Hook Configurations
// ============================================================================

fn default_auto_tts() -> Option<String> {
    Some("auto".to_string())
}

/// Claude Code specific hook configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClaudeCodeHookConfig {
    /// Notification filter: which notification types to speak
    /// Available: "permission_prompt", "idle_prompt", "elicitation_dialog", "auth_success", "*"
    #[serde(default = "default_notification_filter")]
    pub notification_filter: Vec<String>,

    /// TTS provider for Notification hook (e.g., "macos", "google", "auto")
    /// Default: "auto" (uses the default TTS provider fallback chain)
    #[serde(default = "default_auto_tts")]
    pub notification_tts_provider: Option<String>,

    /// TTS provider for Stop hook (e.g., "google", "macos", "auto")
    /// Default: "auto" (uses the default TTS provider fallback chain)
    #[serde(default = "default_auto_tts")]
    pub stop_tts_provider: Option<String>,
}

impl Default for ClaudeCodeHookConfig {
    fn default() -> Self {
        Self {
            notification_filter: default_notification_filter(),
            notification_tts_provider: default_auto_tts(),
            stop_tts_provider: default_auto_tts(),
        }
    }
}

/// All hook configurations
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct HooksConfig {
    /// Claude Code specific settings
    #[serde(default)]
    pub claude_code: ClaudeCodeHookConfig,
}

// ============================================================================
// Main SumvoxConfig
// ============================================================================

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SumvoxConfig {
    #[serde(default = "default_version")]
    pub version: String,

    #[serde(default)]
    pub llm: LlmConfig,

    #[serde(default)]
    pub tts: TtsConfig,

    /// Generic summarization settings (used by sum command)
    #[serde(default)]
    pub summarization: SummarizationConfig,

    /// Hook-specific configurations
    #[serde(default)]
    pub hooks: HooksConfig,
}

impl Default for SumvoxConfig {
    fn default() -> Self {
        Self {
            version: default_version(),
            llm: LlmConfig::default(),
            tts: TtsConfig::default(),
            summarization: SummarizationConfig::default(),
            hooks: HooksConfig::default(),
        }
    }
}

impl SumvoxConfig {
    /// Get the standard config directory: ~/.config/sumvox/
    pub fn config_dir() -> Result<PathBuf> {
        let home = dirs::home_dir()
            .ok_or_else(|| VoiceError::Config("Cannot find home directory".into()))?;
        Ok(home.join(".config").join("sumvox"))
    }

    /// Get the standard config path: ~/.config/sumvox/config.json (deprecated)
    pub fn config_path() -> Result<PathBuf> {
        Ok(Self::config_dir()?.join("config.json"))
    }

    /// Get the YAML config path: ~/.config/sumvox/config.yaml
    pub fn yaml_config_path() -> Result<PathBuf> {
        Ok(Self::config_dir()?.join("config.yaml"))
    }

    /// Load configuration from ~/.config/sumvox/config.yaml (preferred) or config.json (fallback)
    pub fn load_from_home() -> Result<Self> {
        // Try YAML first (preferred format)
        let yaml_path = Self::yaml_config_path()?;
        if yaml_path.exists() {
            tracing::info!("Loading config from {:?}", yaml_path);
            return Self::load_yaml(yaml_path);
        }

        // Fallback to JSON for backward compatibility
        let json_path = Self::config_path()?;
        if json_path.exists() {
            tracing::info!("Loading config from {:?}", json_path);
            return Self::load_json(json_path);
        }

        tracing::info!("No config file found, using defaults");
        Ok(Self::default())
    }

    /// Load configuration from a specific path (auto-detect format)
    pub fn load(path: PathBuf) -> Result<Self> {
        if path.extension().and_then(|s| s.to_str()) == Some("yaml")
            || path.extension().and_then(|s| s.to_str()) == Some("yml")
        {
            Self::load_yaml(path)
        } else {
            Self::load_json(path)
        }
    }

    /// Load configuration from a JSON file
    pub fn load_json(path: PathBuf) -> Result<Self> {
        let content = std::fs::read_to_string(&path).map_err(|e| {
            VoiceError::Config(format!("Failed to read config file {:?}: {}", path, e))
        })?;

        let config: SumvoxConfig = serde_json::from_str(&content)?;
        config.validate()?;
        Ok(config)
    }

    /// Load configuration from a YAML file
    pub fn load_yaml(path: PathBuf) -> Result<Self> {
        let content = std::fs::read_to_string(&path).map_err(|e| {
            VoiceError::Config(format!("Failed to read config file {:?}: {}", path, e))
        })?;

        let config: SumvoxConfig = serde_yaml::from_str(&content)
            .map_err(|e| VoiceError::Config(format!("Failed to parse YAML config: {}", e)))?;
        config.validate()?;
        Ok(config)
    }

    /// Save configuration to ~/.config/sumvox/config.yaml (preferred format)
    pub fn save_to_home(&self) -> Result<()> {
        let config_path = Self::yaml_config_path()?;
        self.save_yaml(config_path)
    }

    /// Save configuration to a specific path (auto-detect format)
    pub fn save(&self, path: PathBuf) -> Result<()> {
        if path.extension().and_then(|s| s.to_str()) == Some("yaml")
            || path.extension().and_then(|s| s.to_str()) == Some("yml")
        {
            self.save_yaml(path)
        } else {
            self.save_json(path)
        }
    }

    /// Save configuration to a JSON file
    pub fn save_json(&self, path: PathBuf) -> Result<()> {
        // Ensure directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, json)?;

        tracing::info!("Config saved to {:?}", path);
        Ok(())
    }

    /// Save configuration to a YAML file
    pub fn save_yaml(&self, path: PathBuf) -> Result<()> {
        // Ensure directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let yaml = serde_yaml::to_string(self)
            .map_err(|e| VoiceError::Config(format!("Failed to serialize YAML: {}", e)))?;
        std::fs::write(&path, yaml)?;

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

        // Validate TTS rate and volume if specified
        for tts in &self.tts.providers {
            if let Some(rate) = tts.rate {
                if !(90..=300).contains(&rate) {
                    return Err(VoiceError::Config(format!(
                        "TTS rate {} out of range [90-300] for provider {}",
                        rate, tts.name
                    )));
                }
            }
            if let Some(volume) = tts.volume {
                if volume > 100 {
                    return Err(VoiceError::Config(format!(
                        "TTS volume {} out of range [0-100] for provider {}",
                        volume, tts.name
                    )));
                }
            }
        }

        // Validate summarization prompt template contains required variable (warning only)
        if !self.summarization.prompt_template.contains("{context}") {
            tracing::warn!(
                "Summarization prompt_template missing required variable: {{context}}"
            );
        }

        Ok(())
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
                model: Some("gemini-2.5-flash-preview-tts".to_string()),
                voice: Some("Zephyr".to_string()),
                api_key: Some(api_key.to_string()),
                rate: None,
                volume: None,
            },
        );
        true
    }

    /// Get default model for a provider
    fn default_model_for_provider(provider: &str) -> String {
        match provider.to_lowercase().as_str() {
            "google" | "gemini" => "gemini-2.5-flash".to_string(),
            "anthropic" | "claude" => "claude-haiku-4-5-20251001".to_string(),
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
        let config = SumvoxConfig::default();
        assert_eq!(config.version, "1.0.0");
        assert!(!config.llm.providers.is_empty());
        assert!(!config.tts.providers.is_empty());
        assert_eq!(config.summarization.turns, 1);
        assert_eq!(
            config.hooks.claude_code.notification_tts_provider,
            Some("auto".to_string())
        );
    }

    #[test]
    fn test_load_new_format() {
        let config_json = r#"{
            "version": "1.0.0",
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
                        "voice": "Tingting",
                        "rate": 200
                    }
                ]
            }
        }"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(config_json.as_bytes()).unwrap();
        let path = temp_file.path().to_path_buf();

        let config = SumvoxConfig::load(path).unwrap();
        assert_eq!(config.version, "1.0.0");
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
            model: None,
            voice: Some("Tingting".to_string()),
            api_key: None,
            rate: Some(200),
            volume: None,
        };
        assert!(macos_provider.is_configured());
    }

    #[test]
    fn test_set_llm_api_key() {
        let mut config = SumvoxConfig::default();

        // Update existing provider
        config.set_llm_api_key("google", "new-key");
        assert_eq!(config.llm.providers[0].api_key, Some("new-key".to_string()));

        // Add new provider
        let initial_count = config.llm.providers.len();
        config.set_llm_api_key("anthropic", "anthropic-key");
        assert_eq!(config.llm.providers.len(), initial_count + 1);
    }

    #[test]
    fn test_validate_invalid_temperature() {
        let mut config = SumvoxConfig::default();
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
        let mut config = SumvoxConfig::default();
        config.tts.providers[1].rate = Some(500);

        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("TTS rate 500 out of range"));
    }

    #[test]
    fn test_validate_invalid_tts_volume() {
        let mut config = SumvoxConfig::default();
        config.tts.providers[0].volume = Some(150);

        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("TTS volume 150 out of range"));
    }

    #[test]
    fn test_validate_valid_tts_volume() {
        let mut config = SumvoxConfig::default();
        config.tts.providers[0].volume = Some(75);
        config.tts.providers[1].volume = Some(100);

        let result = config.validate();
        assert!(result.is_ok());
    }

    #[test]
    fn test_save_and_load() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("test-config.json");

        let mut config = SumvoxConfig::default();
        config.set_llm_api_key("google", "test-key");

        config.save(path.clone()).unwrap();

        let loaded = SumvoxConfig::load(path).unwrap();
        assert_eq!(
            loaded.llm.providers[0].api_key,
            Some("test-key".to_string())
        );
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

    #[test]
    fn test_api_key_placeholder_serialization() {
        let provider = LlmProviderConfig {
            name: "google".to_string(),
            model: "gemini-2.5-flash".to_string(),
            api_key: None,
            base_url: None,
            timeout: 10,
        };

        let json = serde_json::to_string(&provider).unwrap();
        assert!(json.contains("${PROVIDER_API_KEY}"));
    }

    #[test]
    fn test_ollama_timeout_60_seconds() {
        let config = LlmConfig::default();
        let ollama = config
            .providers
            .iter()
            .find(|p| p.name == "ollama")
            .unwrap();
        assert_eq!(ollama.timeout, 60);
    }

    #[test]
    fn test_summarization_config() {
        let config = SumvoxConfig::default();
        assert_eq!(config.summarization.turns, 1);
        assert!(!config.summarization.fallback_message.is_empty());
        assert!(!config.summarization.prompt_template.contains("{max_length}"));
        assert!(config.summarization.prompt_template.contains("{context}"));
    }

    #[test]
    fn test_claude_code_hook_config() {
        let config = SumvoxConfig::default();
        assert!(!config.hooks.claude_code.notification_filter.is_empty());
        assert_eq!(
            config.hooks.claude_code.notification_tts_provider,
            Some("auto".to_string())
        );
        assert_eq!(
            config.hooks.claude_code.stop_tts_provider,
            Some("auto".to_string())
        );
    }

    #[test]
    fn test_max_tokens_default_10000() {
        let params = LlmParameters::default();
        assert_eq!(params.max_tokens, 10000);
    }

    #[test]
    fn test_disable_thinking_default_false() {
        let params = LlmParameters::default();
        assert!(!params.disable_thinking);
    }

    #[test]
    fn test_config_path_is_xdg() {
        let path = SumvoxConfig::config_path().unwrap();
        assert!(path.to_string_lossy().contains(".config/sumvox"));
    }

    #[test]
    fn test_load_yaml_format() {
        let config_yaml = r#"
version: "1.0.0"
llm:
  providers:
    - name: google
      model: gemini-2.5-flash
      api_key: test-key
      timeout: 10
    - name: ollama
      model: llama3.2
  parameters:
    max_tokens: 100
    temperature: 0.3
tts:
  providers:
    - name: macos
      voice: Tingting
      rate: 200
"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(config_yaml.as_bytes()).unwrap();
        let mut path = temp_file.path().to_path_buf();

        // Rename to .yaml extension
        let yaml_path = path.with_extension("yaml");
        std::fs::rename(&path, &yaml_path).unwrap();
        path = yaml_path;

        let config = SumvoxConfig::load_yaml(path).unwrap();
        assert_eq!(config.version, "1.0.0");
        assert_eq!(config.llm.providers.len(), 2);
        assert_eq!(config.llm.providers[0].name, "google");
        assert_eq!(
            config.llm.providers[0].api_key,
            Some("test-key".to_string())
        );
        assert_eq!(config.tts.providers[0].name, "macos");
    }

    #[test]
    fn test_save_and_load_yaml() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("test-config.yaml");

        let mut config = SumvoxConfig::default();
        config.set_llm_api_key("google", "test-yaml-key");

        config.save_yaml(path.clone()).unwrap();

        let loaded = SumvoxConfig::load_yaml(path).unwrap();
        assert_eq!(
            loaded.llm.providers[0].api_key,
            Some("test-yaml-key".to_string())
        );
    }
}
