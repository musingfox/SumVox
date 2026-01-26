// Credential management for claude-voice
// Handles loading API keys from environment variables or credentials.json file

use std::collections::HashMap;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Credentials {
    pub providers: HashMap<String, ProviderCredential>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderCredential {
    pub api_key: String,
}

pub struct CredentialManager {
    credentials_path: PathBuf,
}

impl CredentialManager {
    /// Create manager with default path: ~/.config/claude-voice/credentials.json
    pub fn new() -> Self {
        let config_dir = dirs::config_dir()
            .expect("Failed to get config directory")
            .join("claude-voice");

        let credentials_path = config_dir.join("credentials.json");

        Self { credentials_path }
    }

    /// Create manager with custom path
    pub fn with_path(path: PathBuf) -> Self {
        Self {
            credentials_path: path,
        }
    }

    /// Load API key, priority: environment variable > credentials file
    pub fn load_api_key(&self, provider: &str) -> Option<String> {
        // Try environment variable first
        let env_var = Self::env_var_name(provider);
        if let Ok(key) = std::env::var(env_var) {
            if !key.is_empty() {
                return Some(key);
            }
        }

        // Try credentials file
        self.load_credentials()
            .ok()
            .and_then(|creds| creds.providers.get(provider).map(|p| p.api_key.clone()))
    }

    /// Save API key to credentials file
    pub fn save_api_key(&self, provider: &str, api_key: &str) -> Result<()> {
        use std::fs;
        use std::os::unix::fs::PermissionsExt;

        // Load existing credentials or create new
        let mut credentials = self.load_credentials().unwrap_or_default();

        // Update provider
        credentials.providers.insert(
            provider.to_string(),
            ProviderCredential {
                api_key: api_key.to_string(),
            },
        );

        // Ensure directory exists
        if let Some(parent) = self.credentials_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Write to file
        let json = serde_json::to_string_pretty(&credentials)?;
        fs::write(&self.credentials_path, json)?;

        // Set file permissions to 0600
        let mut perms = fs::metadata(&self.credentials_path)?.permissions();
        perms.set_mode(0o600);
        fs::set_permissions(&self.credentials_path, perms)?;

        Ok(())
    }

    /// List configured providers
    pub fn list_providers(&self) -> Vec<String> {
        self.load_credentials()
            .ok()
            .map(|creds| creds.providers.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// Remove provider's credentials
    pub fn remove_provider(&self, provider: &str) -> Result<()> {
        let mut credentials = self.load_credentials()?;
        credentials.providers.remove(provider);

        let json = serde_json::to_string_pretty(&credentials)?;
        std::fs::write(&self.credentials_path, json)?;

        Ok(())
    }

    /// Load credentials from file
    fn load_credentials(&self) -> Result<Credentials> {
        use crate::error::VoiceError;

        if !self.credentials_path.exists() {
            return Ok(Credentials::default());
        }

        let contents = std::fs::read_to_string(&self.credentials_path)
            .map_err(|e| VoiceError::Config(format!("Failed to read credentials: {}", e)))?;

        let credentials: Credentials = serde_json::from_str(&contents)?;

        Ok(credentials)
    }

    /// Get environment variable name for provider
    pub fn env_var_name(provider: &str) -> &'static str {
        match provider {
            "google" | "gemini" => "GEMINI_API_KEY",
            "anthropic" => "ANTHROPIC_API_KEY",
            "openai" => "OPENAI_API_KEY",
            "google_tts" => "GOOGLE_TTS_API_KEY",
            _ => panic!("Unknown provider: {}", provider),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use tempfile::TempDir;

    #[test]
    fn test_load_api_key_from_env() {
        let temp_dir = TempDir::new().unwrap();
        let creds_path = temp_dir.path().join("credentials.json");
        let manager = CredentialManager::with_path(creds_path);

        // Set environment variable
        env::set_var("GEMINI_API_KEY", "test-env-key");

        let api_key = manager.load_api_key("google");
        assert_eq!(api_key, Some("test-env-key".to_string()));

        // Clean up
        env::remove_var("GEMINI_API_KEY");
    }

    #[test]
    fn test_load_api_key_from_file() {
        let temp_dir = TempDir::new().unwrap();
        let creds_path = temp_dir.path().join("credentials.json");
        let manager = CredentialManager::with_path(creds_path.clone());

        // Save API key to file
        manager.save_api_key("google", "test-file-key").unwrap();

        // Load it back
        let api_key = manager.load_api_key("google");
        assert_eq!(api_key, Some("test-file-key".to_string()));
    }

    #[test]
    fn test_env_priority_over_file() {
        let temp_dir = TempDir::new().unwrap();
        let creds_path = temp_dir.path().join("credentials.json");
        let manager = CredentialManager::with_path(creds_path.clone());

        // Save to file
        manager.save_api_key("google", "test-file-key").unwrap();

        // Set environment variable
        env::set_var("GEMINI_API_KEY", "test-env-key");

        // Environment should take priority
        let api_key = manager.load_api_key("google");
        assert_eq!(api_key, Some("test-env-key".to_string()));

        // Clean up
        env::remove_var("GEMINI_API_KEY");
    }

    #[test]
    fn test_save_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let creds_path = temp_dir.path().join("credentials.json");
        let manager = CredentialManager::with_path(creds_path.clone());

        // Save multiple providers
        manager.save_api_key("google", "google-key").unwrap();
        manager.save_api_key("anthropic", "anthropic-key").unwrap();
        manager.save_api_key("openai", "openai-key").unwrap();

        // Load them back
        assert_eq!(manager.load_api_key("google"), Some("google-key".to_string()));
        assert_eq!(manager.load_api_key("anthropic"), Some("anthropic-key".to_string()));
        assert_eq!(manager.load_api_key("openai"), Some("openai-key".to_string()));
    }

    #[test]
    fn test_file_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = TempDir::new().unwrap();
        let creds_path = temp_dir.path().join("credentials.json");
        let manager = CredentialManager::with_path(creds_path.clone());

        // Save API key
        manager.save_api_key("google", "test-key").unwrap();

        // Check file permissions are 0600
        let metadata = std::fs::metadata(&creds_path).unwrap();
        let permissions = metadata.permissions();
        assert_eq!(permissions.mode() & 0o777, 0o600);
    }

    #[test]
    fn test_list_providers() {
        let temp_dir = TempDir::new().unwrap();
        let creds_path = temp_dir.path().join("credentials.json");
        let manager = CredentialManager::with_path(creds_path);

        // Initially empty
        assert_eq!(manager.list_providers().len(), 0);

        // Add providers
        manager.save_api_key("google", "key1").unwrap();
        manager.save_api_key("anthropic", "key2").unwrap();

        let providers = manager.list_providers();
        assert_eq!(providers.len(), 2);
        assert!(providers.contains(&"google".to_string()));
        assert!(providers.contains(&"anthropic".to_string()));
    }

    #[test]
    fn test_remove_provider() {
        let temp_dir = TempDir::new().unwrap();
        let creds_path = temp_dir.path().join("credentials.json");
        let manager = CredentialManager::with_path(creds_path);

        // Ensure environment variable is not set
        env::remove_var("GEMINI_API_KEY");

        // Save and remove
        manager.save_api_key("google", "test-key").unwrap();
        assert_eq!(manager.load_api_key("google"), Some("test-key".to_string()));

        manager.remove_provider("google").unwrap();
        assert_eq!(manager.load_api_key("google"), None);
    }

    #[test]
    fn test_env_var_name() {
        assert_eq!(CredentialManager::env_var_name("google"), "GEMINI_API_KEY");
        assert_eq!(CredentialManager::env_var_name("gemini"), "GEMINI_API_KEY");
        assert_eq!(CredentialManager::env_var_name("anthropic"), "ANTHROPIC_API_KEY");
        assert_eq!(CredentialManager::env_var_name("openai"), "OPENAI_API_KEY");
        assert_eq!(CredentialManager::env_var_name("google_tts"), "GOOGLE_TTS_API_KEY");
    }

    #[test]
    #[should_panic(expected = "Unknown provider")]
    fn test_env_var_name_unknown_provider() {
        CredentialManager::env_var_name("unknown");
    }
}
