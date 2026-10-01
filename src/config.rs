// Configuration loading and validation
// Unified config at ~/.config/sumvox/config.toml with array-based provider fallback

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

/// Serialize an f32 with its own shortest decimal form. TOML has only f64, so the
/// serializer casts the f32 up and 0.3 lands in the file as 0.30000001192092896.
fn serialize_f32<S>(value: &f32, serializer: S) -> std::result::Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let widened = value.to_string().parse::<f64>().unwrap_or(*value as f64);
    serializer.serialize_f64(widened)
}

/// Same as [`serialize_f32`] for the optional per-provider knobs.
fn serialize_opt_f32<S>(value: &Option<f32>, serializer: S) -> std::result::Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    match value {
        Some(v) => serialize_f32(v, serializer),
        None => serializer.serialize_none(),
    }
}

fn default_turns() -> usize {
    1
}

fn default_fallback_message() -> String {
    "Task completed".to_string()
}

fn default_content_source() -> ContentSource {
    ContentSource::Transcript
}

fn default_max_tokens() -> u32 {
    10000
}

fn default_temperature() -> f32 {
    0.3
}

fn default_prompt_template() -> String {
    "Based on the following context, generate a concise summary.\n\nContext:\n{context}\n\nSummary:"
        .to_string()
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

    /// Per-provider override for disable_thinking.
    /// When Some, overrides the global llm.parameters.disable_thinking.
    /// When None, falls back to the global value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disable_thinking: Option<bool>,
}

/// Resolve effective disable_thinking: provider override takes priority over global default.
pub fn effective_disable_thinking(provider: &LlmProviderConfig, params: &LlmParameters) -> bool {
    provider.disable_thinking.unwrap_or(params.disable_thinking)
}

impl LlmProviderConfig {
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
            "xai" | "grok" => "XAI_API_KEY",
            _ => "API_KEY",
        }
    }
}

/// LLM parameters shared across providers
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LlmParameters {
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,

    #[serde(default = "default_temperature", serialize_with = "serialize_f32")]
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
                    model: "gemini-3.1-flash-lite".to_string(),
                    api_key: None,
                    base_url: None,
                    timeout: default_timeout(),
                    disable_thinking: None,
                },
                LlmProviderConfig {
                    name: "anthropic".to_string(),
                    model: "claude-haiku-4-5-20251001".to_string(),
                    api_key: None,
                    base_url: None,
                    timeout: default_timeout(),
                    disable_thinking: None,
                },
                LlmProviderConfig {
                    name: "openai".to_string(),
                    model: "gpt-5-nano".to_string(),
                    api_key: None,
                    base_url: None,
                    timeout: default_timeout(),
                    disable_thinking: None,
                },
                LlmProviderConfig {
                    name: "ollama".to_string(),
                    model: "llama3.2".to_string(),
                    api_key: None,
                    base_url: None,
                    timeout: default_ollama_timeout(),
                    disable_thinking: None,
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

    /// Audio file path (for audio_file provider only)
    /// Can be a single file or a directory (picks random file each time)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,

    /// Service account key path (for cloud_tts provider only)
    /// Path to JSON service account key file
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_account_key: Option<String>,

    /// Language code (for cloud_tts provider)
    /// Examples: "en-US", "zh-TW", "ja-JP"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language_code: Option<String>,

    /// Speech speed multiplier (for ElevenLabs).
    /// Range 0.7-1.2; 1.0 = default, <1.0 slower, >1.0 faster.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_opt_f32"
    )]
    pub speed: Option<f32>,

    /// Voice stability (for ElevenLabs). Range 0.0-1.0.
    /// Higher = calmer/less pitch variation, lower = more expressive.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_opt_f32"
    )]
    pub stability: Option<f32>,

    /// Style exaggeration (for ElevenLabs). Range 0.0-1.0.
    /// Lower = flatter pitch/less expressive, 0.0 disables style.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_opt_f32"
    )]
    pub style: Option<f32>,

    /// Style instruction prompt (for Gemini-TTS via cloud_tts).
    /// Free-form direction, e.g. "Say the following in a cheerful tone."
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style_prompt: Option<String>,
}

impl TtsProviderConfig {
    /// Get ElevenLabs API key from config or environment
    pub fn get_elevenlabs_api_key(&self) -> Option<String> {
        if let Some(ref key) = self.api_key {
            if !key.is_empty() && !key.starts_with("${") {
                return Some(key.clone());
            }
        }

        std::env::var("ELEVENLABS_API_KEY")
            .ok()
            .filter(|k| !k.is_empty())
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

    /// Get xAI API key from config or environment
    pub fn get_xai_api_key(&self) -> Option<String> {
        if let Some(ref key) = self.api_key {
            if !key.is_empty() && !key.starts_with("${") {
                return Some(key.clone());
            }
        }

        std::env::var("XAI_API_KEY").ok().filter(|k| !k.is_empty())
    }

    /// Get OpenAI API key from config or environment
    pub fn get_openai_api_key(&self) -> Option<String> {
        if let Some(ref key) = self.api_key {
            if !key.is_empty() && !key.starts_with("${") {
                return Some(key.clone());
            }
        }

        std::env::var("OPENAI_API_KEY")
            .ok()
            .filter(|k| !k.is_empty())
    }

    /// Get service account key file content
    /// Expands ~ and reads file content
    pub fn get_service_account_key(&self) -> Option<String> {
        let path_str = self.service_account_key.as_ref()?;

        // Expand ~ to home directory
        let expanded = shellexpand::tilde(path_str).to_string();

        // Read file content
        std::fs::read_to_string(&expanded).ok()
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
                    path: None,
                    service_account_key: None,
                    language_code: None,
                    speed: None,
                    stability: None,
                    style: None,
                    style_prompt: None,
                },
                TtsProviderConfig {
                    name: "macos".to_string(),
                    model: None,
                    voice: None,
                    api_key: None,
                    rate: Some(200),
                    volume: None,
                    path: None,
                    service_account_key: None,
                    language_code: None,
                    speed: None,
                    stability: None,
                    style: None,
                    style_prompt: None,
                },
            ],
        }
    }
}

// ============================================================================
// Summarization Configuration (generic, used by sum command and hooks)
// ============================================================================

/// Content source for Stop hook context
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContentSource {
    /// Read from transcript JSONL file
    Transcript,
    /// Use last_assistant_message from hook input
    LastMessage,
}

/// Summarization configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SummarizationConfig {
    /// Content source for Stop hook context (default: Transcript)
    #[serde(default = "default_content_source")]
    pub content_source: ContentSource,

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
            content_source: default_content_source(),
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

    /// Volume for Notification hook (0-100), default: 80 if not specified
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notification_volume: Option<u32>,

    /// Volume for Stop hook (0-100), default: 100 if not specified
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop_volume: Option<u32>,

    /// Queue timeout in seconds for cross-process notification ordering
    /// Default: 30 seconds. Set to 0 to disable queuing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub queue_timeout: Option<u64>,
}

impl Default for ClaudeCodeHookConfig {
    fn default() -> Self {
        Self {
            notification_filter: default_notification_filter(),
            notification_tts_provider: default_auto_tts(),
            stop_tts_provider: default_auto_tts(),
            notification_volume: None, // Will use 80 in runtime if None
            stop_volume: None,         // Will use 100 in runtime if None
            queue_timeout: None,       // Will use 30s in runtime if None
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

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SumvoxConfig {
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

impl SumvoxConfig {
    /// Get the standard config directory: ~/.config/sumvox/
    pub fn config_dir() -> Result<PathBuf> {
        let home = dirs::home_dir()
            .ok_or_else(|| VoiceError::Config("Cannot find home directory".into()))?;
        Ok(home.join(".config").join("sumvox"))
    }

    /// Get the TOML config path: ~/.config/sumvox/config.toml
    pub fn toml_config_path() -> Result<PathBuf> {
        Ok(Self::config_dir()?.join("config.toml"))
    }

    /// Load configuration from ~/.config/sumvox/config.toml, or defaults when none exists
    pub fn load_from_home() -> Result<Self> {
        let toml_path = Self::toml_config_path()?;
        if toml_path.exists() {
            tracing::info!("Loading config from {:?}", toml_path);
            return Self::load_toml(toml_path);
        }

        // A YAML/JSON config is no longer read; silently using defaults would hide it
        for legacy in ["config.yaml", "config.yml", "config.json"] {
            let legacy_path = Self::config_dir()?.join(legacy);
            if legacy_path.exists() {
                return Err(VoiceError::Config(format!(
                    "Found legacy config {:?} but no config.toml. YAML/JSON configs are no longer supported: convert it to {:?} or run `sumvox init`.",
                    legacy_path, toml_path
                )));
            }
        }

        // No config file found, use defaults
        tracing::info!("No config file found, using defaults");
        Ok(Self::default())
    }

    /// Load configuration from a TOML file
    pub fn load_toml(path: PathBuf) -> Result<Self> {
        let content = std::fs::read_to_string(&path).map_err(|e| {
            VoiceError::Config(format!("Failed to read config file {:?}: {}", path, e))
        })?;
        let config: SumvoxConfig = toml::from_str(&content)
            .map_err(|e| VoiceError::Config(format!("Failed to parse TOML config: {}", e)))?;
        config.validate()?;
        Ok(config)
    }

    /// Save configuration to a TOML file
    pub fn save_toml(&self, path: PathBuf) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let toml_str = toml::to_string_pretty(self)
            .map_err(|e| VoiceError::Config(format!("Failed to serialize TOML: {}", e)))?;
        std::fs::write(&path, toml_str)?;
        tracing::info!("Config saved to {:?}", path);
        Ok(())
    }

    /// Save configuration to ~/.config/sumvox/config.toml (preferred format)
    pub fn save_to_home(&self) -> Result<()> {
        let config_path = Self::toml_config_path()?;
        self.save_toml(config_path)
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
            tracing::warn!("Summarization prompt_template missing required variable: {{context}}");
        }

        // Validate hook-specific volumes
        if let Some(volume) = self.hooks.claude_code.notification_volume {
            if volume > 100 {
                return Err(VoiceError::Config(format!(
                    "Notification volume {} out of range [0-100]",
                    volume
                )));
            }
        }
        if let Some(volume) = self.hooks.claude_code.stop_volume {
            if volume > 100 {
                return Err(VoiceError::Config(format!(
                    "Stop hook volume {} out of range [0-100]",
                    volume
                )));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    fn openai_tts_provider(api_key: Option<String>) -> TtsProviderConfig {
        TtsProviderConfig {
            name: "openai".to_string(),
            model: None,
            voice: None,
            api_key,
            rate: None,
            volume: None,
            path: None,
            service_account_key: None,
            language_code: None,
            speed: None,
            stability: None,
            style: None,
            style_prompt: None,
        }
    }

    #[test]
    fn test_get_openai_api_key_from_config() {
        let _env = crate::test_support::env_guard();
        let provider = openai_tts_provider(Some("sk-test".to_string()));
        assert_eq!(provider.get_openai_api_key(), Some("sk-test".to_string()));

        // A `${...}` placeholder is never returned as the key itself
        let placeholder = openai_tts_provider(Some("${OPENAI_API_KEY}".to_string()));
        assert_ne!(
            placeholder.get_openai_api_key(),
            Some("${OPENAI_API_KEY}".to_string())
        );
    }

    #[test]
    fn test_serialized_temperature_keeps_its_decimal_form() {
        // The f32 -> f64 cast in the TOML serializer used to write 0.30000001192092896.
        let config = SumvoxConfig::default();
        let toml_str = toml::to_string_pretty(&config).unwrap();
        assert!(
            toml_str.contains("temperature = 0.3\n"),
            "temperature should serialize as 0.3, got:\n{}",
            toml_str
        );

        // The same cast applies to the optional per-provider knobs.
        let provider = TtsProviderConfig {
            name: "elevenlabs".to_string(),
            model: None,
            voice: None,
            api_key: None,
            rate: None,
            volume: None,
            path: None,
            service_account_key: None,
            language_code: None,
            speed: Some(0.9),
            stability: Some(0.4),
            style: Some(0.2),
            style_prompt: None,
        };
        let toml_str = toml::to_string_pretty(&provider).unwrap();
        for expected in ["speed = 0.9\n", "stability = 0.4\n", "style = 0.2\n"] {
            assert!(
                toml_str.contains(expected),
                "expected {expected:?} in:\n{toml_str}"
            );
        }
    }

    #[test]
    fn test_validate_invalid_temperature() {
        type Mutate = fn(&mut SumvoxConfig);
        let cases: [(&str, Mutate); 5] = [
            ("Temperature 3 out of range", |c| {
                c.llm.parameters.temperature = 3.0
            }),
            ("TTS rate 500 out of range", |c| {
                c.tts.providers[1].rate = Some(500)
            }),
            ("TTS volume 150 out of range", |c| {
                c.tts.providers[0].volume = Some(150)
            }),
            ("Notification volume", |c| {
                c.hooks.claude_code.notification_volume = Some(150)
            }),
            ("Stop hook volume", |c| {
                c.hooks.claude_code.stop_volume = Some(200)
            }),
        ];
        for (expected, mutate) in cases {
            let mut config = SumvoxConfig::default();
            mutate(&mut config);
            let err = config.validate().expect_err(expected);
            assert!(err.to_string().contains(expected), "{expected}: {err}");
        }

        // Boundary values are accepted
        let mut config = SumvoxConfig::default();
        config.tts.providers[0].volume = Some(75);
        config.tts.providers[1].volume = Some(100);
        config.hooks.claude_code.notification_volume = Some(80);
        config.hooks.claude_code.stop_volume = Some(100);
        assert!(config.validate().is_ok());
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
            disable_thinking: None,
        };

        let json = serde_json::to_string(&provider).unwrap();
        assert!(json.contains("${PROVIDER_API_KEY}"));
    }

    #[test]
    fn test_load_save_toml() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("test.toml");

        let mut config = SumvoxConfig::default();
        config.llm.providers[0].api_key = Some("test-toml-key".to_string());

        config.save_toml(path.clone()).unwrap();
        let loaded = SumvoxConfig::load_toml(path).unwrap();

        assert_eq!(
            loaded.llm.providers[0].api_key,
            Some("test-toml-key".to_string())
        );
    }

    #[test]
    fn test_service_account_key_reads_file() {
        use std::io::Write;

        // Create temp file with JSON content
        let mut temp_file = NamedTempFile::new().unwrap();
        let json_content = r#"{"test": "content"}"#;
        temp_file.write_all(json_content.as_bytes()).unwrap();
        temp_file.flush().unwrap();

        let config = TtsProviderConfig {
            name: "cloud_tts".to_string(),
            model: None,
            voice: None,
            api_key: None,
            rate: None,
            volume: None,
            path: None,
            service_account_key: Some(temp_file.path().to_string_lossy().to_string()),
            language_code: None,
            speed: None,
            stability: None,
            style: None,
            style_prompt: None,
        };

        let content = config.get_service_account_key();
        assert!(content.is_some());
        assert_eq!(content.unwrap(), json_content);

        let no_key = TtsProviderConfig {
            service_account_key: None,
            ..config
        };
        assert_eq!(no_key.get_service_account_key(), None);
    }

    // ── ContentSource tests ──────────────────────────────────────────

    #[test]
    fn test_content_source_serde_round_trip() {
        let toml_absent = r#"
[summarization]
turns = 1
"#;
        let config: SumvoxConfig = toml::from_str(toml_absent).unwrap();
        assert_eq!(
            config.summarization.content_source,
            ContentSource::Transcript
        );

        let toml_transcript = r#"
[summarization]
content_source = "transcript"
turns = 1
"#;
        let config: SumvoxConfig = toml::from_str(toml_transcript).unwrap();
        assert_eq!(
            config.summarization.content_source,
            ContentSource::Transcript
        );

        let toml_last_message = r#"
[summarization]
content_source = "last_message"
turns = 1
"#;
        let config: SumvoxConfig = toml::from_str(toml_last_message).unwrap();
        assert_eq!(
            config.summarization.content_source,
            ContentSource::LastMessage
        );

        let toml_invalid = r#"
[summarization]
content_source = "invalid"
turns = 1
"#;
        let result = toml::from_str::<SumvoxConfig>(toml_invalid);
        assert!(result.is_err());
    }

    // ── C1: effective_disable_thinking resolver ──────────────────────────

    fn make_provider(override_val: Option<bool>) -> LlmProviderConfig {
        LlmProviderConfig {
            name: "google".to_string(),
            model: "gemini-2.5-flash".to_string(),
            api_key: None,
            base_url: None,
            timeout: 10,
            disable_thinking: override_val,
        }
    }

    fn make_params(global: bool) -> LlmParameters {
        LlmParameters {
            max_tokens: 100,
            temperature: 0.3,
            disable_thinking: global,
        }
    }

    #[test]
    fn test_c1_provider_none_uses_global_false() {
        let provider = make_provider(None);
        let params = make_params(false);
        assert!(!effective_disable_thinking(&provider, &params));

        // (provider override, global, expected): the provider wins when set
        for (over, global, expected) in [
            (None, true, true),
            (Some(true), false, true),
            (Some(false), true, false),
        ] {
            assert_eq!(
                effective_disable_thinking(&make_provider(over), &make_params(global)),
                expected
            );
        }
    }
}
