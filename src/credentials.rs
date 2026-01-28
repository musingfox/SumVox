// Credential management for claude-voice
// Note: Credentials are now stored in ~/.claude/claude-voice.json
// This module is kept for backwards compatibility and utility functions

use crate::config::VoiceConfig;

/// Get environment variable name for a provider
///
/// This function is used to check environment variables as fallback
/// when API keys are not set in the config file.
pub fn env_var_name(provider: &str) -> &'static str {
    match provider.to_lowercase().as_str() {
        "google" | "gemini" => "GEMINI_API_KEY",
        "anthropic" | "claude" => "ANTHROPIC_API_KEY",
        "openai" | "gpt" => "OPENAI_API_KEY",
        "google_tts" => "GOOGLE_CLOUD_PROJECT",
        _ => "API_KEY",
    }
}

/// Check if an API key is available for a provider
///
/// Checks both the config file and environment variables.
pub fn has_api_key(provider: &str) -> bool {
    // Check environment variable first
    let env_var = env_var_name(provider);
    if std::env::var(env_var).ok().filter(|k| !k.is_empty()).is_some() {
        return true;
    }

    // Check config file
    if let Ok(config) = VoiceConfig::load_from_home() {
        return config
            .llm
            .providers
            .iter()
            .any(|p| p.name.to_lowercase() == provider.to_lowercase() && p.has_credentials());
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_env_var_name() {
        assert_eq!(env_var_name("google"), "GEMINI_API_KEY");
        assert_eq!(env_var_name("gemini"), "GEMINI_API_KEY");
        assert_eq!(env_var_name("anthropic"), "ANTHROPIC_API_KEY");
        assert_eq!(env_var_name("openai"), "OPENAI_API_KEY");
        assert_eq!(env_var_name("google_tts"), "GOOGLE_CLOUD_PROJECT");
        assert_eq!(env_var_name("unknown"), "API_KEY");
    }

    #[test]
    fn test_has_api_key_from_env() {
        std::env::set_var("GEMINI_API_KEY", "test-key");
        assert!(has_api_key("google"));
        std::env::remove_var("GEMINI_API_KEY");
    }
}
