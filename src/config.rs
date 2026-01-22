// Configuration loading and validation

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::error::{Result, VoiceError};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VoiceConfig {
    pub version: String,
    pub enabled: bool,
    pub llm: LlmConfig,
    pub voice: VoiceEngineConfig,
    pub triggers: TriggerConfig,
    pub summarization: SummarizationConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub advanced: AdvancedConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LlmConfig {
    pub provider: String,
    pub models: ModelConfig,
    pub api_keys: HashMap<String, String>,
    pub parameters: LlmParameters,
    pub cost_control: CostControl,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ModelConfig {
    pub primary: String,
    pub fallback: Option<String>,
    pub local: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LlmParameters {
    pub max_tokens: u32,
    pub temperature: f32,
    pub timeout: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CostControl {
    pub daily_limit_usd: f64,
    pub usage_tracking: bool,
    pub usage_file: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VoiceEngineConfig {
    pub engine: String,
    pub voice_name: String,
    pub rate: u32,
    pub volume: u32,
    pub max_summary_length: usize,
    #[serde(default = "default_true")]
    pub async_mode: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TriggerConfig {
    pub on_completion: bool,
    pub on_error: bool,
    pub min_duration_seconds: u64,
    #[serde(default)]
    pub error_keywords: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SummarizationConfig {
    pub language: String,
    pub format: String,
    pub include: IncludeConfig,
    pub prompt_template: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IncludeConfig {
    pub operation_type: bool,
    pub result_status: bool,
    pub key_data: bool,
    pub next_steps: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct LoggingConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_log_file")]
    pub log_file: String,
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct AdvancedConfig {
    #[serde(default)]
    pub cache_summaries: bool,
    #[serde(default = "default_cache_ttl")]
    pub cache_ttl_seconds: u64,
    #[serde(default = "default_retry_attempts")]
    pub retry_attempts: u32,
    #[serde(default = "default_fallback_message")]
    pub fallback_message: String,
}

fn default_true() -> bool {
    true
}

fn default_log_file() -> String {
    "~/.claude/logs/voice-notifications.log".to_string()
}

fn default_log_level() -> String {
    "INFO".to_string()
}

fn default_cache_ttl() -> u64 {
    3600
}

fn default_retry_attempts() -> u32 {
    3
}

fn default_fallback_message() -> String {
    "Claude Code task completed".to_string()
}

impl VoiceConfig {
    /// Load configuration from JSON file
    pub fn load(path: PathBuf) -> Result<Self> {
        let content = std::fs::read_to_string(&path).map_err(|e| {
            VoiceError::Config(format!("Failed to read config file {:?}: {}", path, e))
        })?;

        let config: VoiceConfig = serde_json::from_str(&content)?;
        config.validate()?;
        Ok(config)
    }

    /// Validate configuration
    fn validate(&self) -> Result<()> {
        // Validate voice config
        if self.voice.rate < 90 || self.voice.rate > 300 {
            return Err(VoiceError::Config(format!(
                "Voice rate {} out of range [90-300]",
                self.voice.rate
            )));
        }

        if self.voice.volume > 100 {
            return Err(VoiceError::Config(format!(
                "Voice volume {} out of range [0-100]",
                self.voice.volume
            )));
        }

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

        Ok(())
    }

    /// Expand environment variables in API keys
    pub fn expand_env_vars(&mut self) {
        for (_, value) in self.llm.api_keys.iter_mut() {
            if value.starts_with("${") && value.ends_with('}') {
                let env_var = &value[2..value.len() - 1];
                if let Ok(env_value) = std::env::var(env_var) {
                    *value = env_value;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_load_valid_config() {
        let config_json = r#"{
            "version": "1.0.0",
            "enabled": true,
            "llm": {
                "provider": "gemini",
                "models": {
                    "primary": "gemini/gemini-2.0-flash-exp",
                    "fallback": "claude-3-haiku-20240307"
                },
                "api_keys": {
                    "gemini": "${GEMINI_API_KEY}"
                },
                "parameters": {
                    "max_tokens": 100,
                    "temperature": 0.3,
                    "timeout": 10
                },
                "cost_control": {
                    "daily_limit_usd": 0.10,
                    "usage_tracking": true,
                    "usage_file": "~/.claude/voice-usage.json"
                }
            },
            "voice": {
                "engine": "macos_say",
                "voice_name": "Ting-Ting",
                "rate": 200,
                "volume": 75,
                "max_summary_length": 50,
                "async_mode": true
            },
            "triggers": {
                "on_completion": true,
                "on_error": true,
                "min_duration_seconds": 0,
                "error_keywords": ["Error:", "Failed:"]
            },
            "summarization": {
                "language": "zh-TW",
                "format": "concise",
                "include": {
                    "operation_type": true,
                    "result_status": true,
                    "key_data": true,
                    "next_steps": true
                },
                "prompt_template": "Summarize in {language}: {context}"
            }
        }"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(config_json.as_bytes()).unwrap();
        let path = temp_file.path().to_path_buf();

        let config = VoiceConfig::load(path).unwrap();
        assert_eq!(config.version, "1.0.0");
        assert_eq!(config.llm.models.primary, "gemini/gemini-2.0-flash-exp");
        assert_eq!(config.voice.voice_name, "Ting-Ting");
    }

    #[test]
    fn test_validate_invalid_rate() {
        let mut config = create_minimal_config();
        config.voice.rate = 500; // Invalid

        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Voice rate 500 out of range"));
    }

    #[test]
    fn test_validate_invalid_temperature() {
        let mut config = create_minimal_config();
        config.llm.parameters.temperature = 3.0; // Invalid

        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Temperature 3 out of range"));
    }

    #[test]
    fn test_expand_env_vars() {
        std::env::set_var("TEST_API_KEY", "test_key_123");

        let mut config = create_minimal_config();
        config
            .llm
            .api_keys
            .insert("test".to_string(), "${TEST_API_KEY}".to_string());

        config.expand_env_vars();
        assert_eq!(config.llm.api_keys.get("test").unwrap(), "test_key_123");
    }

    fn create_minimal_config() -> VoiceConfig {
        VoiceConfig {
            version: "1.0.0".to_string(),
            enabled: true,
            llm: LlmConfig {
                provider: "gemini".to_string(),
                models: ModelConfig {
                    primary: "gemini/test".to_string(),
                    fallback: None,
                    local: None,
                },
                api_keys: HashMap::new(),
                parameters: LlmParameters {
                    max_tokens: 100,
                    temperature: 0.3,
                    timeout: 10,
                },
                cost_control: CostControl {
                    daily_limit_usd: 0.10,
                    usage_tracking: false,
                    usage_file: "test.json".to_string(),
                },
            },
            voice: VoiceEngineConfig {
                engine: "macos_say".to_string(),
                voice_name: "Ting-Ting".to_string(),
                rate: 200,
                volume: 75,
                max_summary_length: 50,
                async_mode: true,
            },
            triggers: TriggerConfig {
                on_completion: true,
                on_error: true,
                min_duration_seconds: 0,
                error_keywords: vec![],
            },
            summarization: SummarizationConfig {
                language: "zh-TW".to_string(),
                format: "concise".to_string(),
                include: IncludeConfig {
                    operation_type: true,
                    result_status: true,
                    key_data: true,
                    next_steps: true,
                },
                prompt_template: "Summarize: {context}".to_string(),
            },
            logging: LoggingConfig::default(),
            advanced: AdvancedConfig::default(),
        }
    }
}
