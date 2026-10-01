// TTS (Text-to-Speech) module
// Provides abstraction over different TTS engines with fallback support

pub mod cloud_tts;
pub mod cloud_tts_auth;
pub mod elevenlabs;
pub mod espeak;
pub mod google;
pub mod macos;
pub mod openai;
pub mod piper;
pub mod xai;

use async_trait::async_trait;
use std::str::FromStr;

use crate::config::TtsProviderConfig;
use crate::error::{Result, VoiceError};

/// TTS Provider trait - defines interface for text-to-speech engines
#[async_trait]
pub trait TtsProvider: Send + Sync {
    /// Provider name for logging
    fn name(&self) -> &str;

    /// Check if the provider is available (has credentials, etc.)
    fn is_available(&self) -> bool;

    /// Speak the given text
    /// Returns true if speech was initiated successfully
    async fn speak(&self, text: &str) -> Result<bool>;

    /// Estimate cost per character (for cloud providers)
    /// Returns 0.0 for local engines
    fn estimate_cost(&self, char_count: usize) -> f64;

    /// Whether this provider interprets `[tag]`-style audio/emotion tags
    /// (e.g. ElevenLabs eleven_v3). Providers that don't must have such
    /// tags stripped before speaking, or they get read aloud literally.
    fn supports_audio_tags(&self) -> bool {
        false
    }
}

/// Strip a single leading `[tag]` (e.g. "[satisfied] ") from text meant for
/// providers that don't interpret audio tags, so they don't read it aloud.
pub fn strip_leading_audio_tag(text: &str) -> &str {
    let trimmed = text.trim_start();
    if let Some(rest) = trimmed.strip_prefix('[') {
        if let Some(end) = rest.find(']') {
            let tag = &rest[..end];
            if !tag.is_empty() && tag.chars().all(|c| c.is_ascii_alphabetic()) {
                return rest[end + 1..].trim_start();
            }
        }
    }
    text
}

/// TTS Engine type for CLI selection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TtsEngine {
    MacOS,
    Google,
    CloudTts,
    Xai,
    ElevenLabs,
    OpenAi,
    AudioFile,
    Espeak,
    Piper,
    Auto,
}

impl FromStr for TtsEngine {
    type Err = VoiceError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "macos" | "say" => Ok(TtsEngine::MacOS),
            "google" | "google_tts" | "gcloud" => Ok(TtsEngine::Google),
            "cloud_tts" | "gcp_tts" | "google_cloud" | "gemini_tts" => Ok(TtsEngine::CloudTts),
            "xai" | "xai_tts" | "grok" => Ok(TtsEngine::Xai),
            "elevenlabs" | "eleven_labs" | "11labs" => Ok(TtsEngine::ElevenLabs),
            "openai" | "openai_tts" => Ok(TtsEngine::OpenAi),
            "audio_file" | "audio" | "file" => Ok(TtsEngine::AudioFile),
            "espeak" | "espeak_ng" | "espeak-ng" => Ok(TtsEngine::Espeak),
            "piper" | "piper_tts" => Ok(TtsEngine::Piper),
            "auto" => Ok(TtsEngine::Auto),
            _ => Err(VoiceError::Config(format!("Unknown TTS engine: {}", s))),
        }
    }
}

impl std::fmt::Display for TtsEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TtsEngine::MacOS => write!(f, "macos"),
            TtsEngine::Google => write!(f, "google"),
            TtsEngine::CloudTts => write!(f, "cloud_tts"),
            TtsEngine::Xai => write!(f, "xai"),
            TtsEngine::ElevenLabs => write!(f, "elevenlabs"),
            TtsEngine::OpenAi => write!(f, "openai"),
            TtsEngine::AudioFile => write!(f, "audio_file"),
            TtsEngine::Espeak => write!(f, "espeak"),
            TtsEngine::Piper => write!(f, "piper"),
            TtsEngine::Auto => write!(f, "auto"),
        }
    }
}

// Re-export providers
pub use cloud_tts::CloudTtsProvider;
pub use elevenlabs::ElevenLabsProvider;
pub use espeak::EspeakProvider;
pub use google::GoogleTtsProvider;
pub use macos::MacOsTtsProvider;
pub use openai::OpenAiTtsProvider;
pub use piper::PiperProvider;
pub use xai::XaiTtsProvider;

/// Create TTS provider from config array with automatic fallback
///
/// Tries each provider in order until one is available.
/// Returns an error if no provider can be created.
pub fn create_tts_from_config(providers: &[TtsProviderConfig]) -> Result<Box<dyn TtsProvider>> {
    let mut errors = Vec::new();

    for config in providers {
        match create_single_tts(config) {
            Ok(provider) => {
                if provider.is_available() {
                    tracing::info!(
                        "Using TTS provider: {} (voice: {})",
                        config.name,
                        config.voice.as_deref().unwrap_or("default")
                    );
                    return Ok(provider);
                } else {
                    tracing::debug!("TTS {} created but not available, trying next", config.name);
                    errors.push(format!("{}: not available", config.name));
                }
            }
            Err(e) => {
                tracing::debug!("Failed to create TTS {}: {}", config.name, e);
                errors.push(format!("{}: {}", config.name, e));
            }
        }
    }

    Err(VoiceError::Config(format!(
        "No TTS provider available. Tried: {}",
        errors.join("; ")
    )))
}

/// Create a single TTS provider from config
pub fn create_single_tts(config: &TtsProviderConfig) -> Result<Box<dyn TtsProvider>> {
    let volume = config.volume.unwrap_or(100);

    match config.name.to_lowercase().as_str() {
        "macos" | "say" => {
            let voice = config.voice.clone();
            let rate = config.rate.unwrap_or(200);
            Ok(Box::new(MacOsTtsProvider::new(voice, rate, volume)))
        }
        "google" | "google_tts" | "gcloud" | "gemini" => {
            let api_key = config.get_api_key().ok_or_else(|| {
                VoiceError::Config(
                    "Gemini API key not found. Set in config or env var GEMINI_API_KEY".into(),
                )
            })?;

            // Model is required for Google TTS
            let model = config.model.clone().ok_or_else(|| {
                VoiceError::Config(
                    "Google TTS model is required. Specify in config, e.g., 'gemini-2.5-flash-preview-tts'".into()
                )
            })?;

            // Voice is required for Google TTS — no hardcoded default.
            let voice = config.voice.clone().ok_or_else(|| {
                VoiceError::Config(
                    "Google TTS voice is required. Specify in config, e.g., 'Aoede'".into(),
                )
            })?;
            Ok(Box::new(GoogleTtsProvider::new(
                api_key, model, voice, volume,
            )))
        }
        "cloud_tts" | "gcp_tts" | "google_cloud" | "gemini_tts" => {
            let sa_json = config.get_service_account_key().ok_or_else(|| {
                VoiceError::Config("Cloud TTS requires service_account_key".into())
            })?;

            // Voice is required — no hardcoded default.
            let voice = config.voice.clone().ok_or_else(|| {
                VoiceError::Config(
                    "Cloud TTS voice is required. Specify in config, e.g., 'en-US-Standard-A'"
                        .into(),
                )
            })?;
            let language_code = config.language_code.clone();
            // model set => Gemini-TTS (bare voice name + model_name); style_prompt optional.
            let model = config.model.clone();
            let style_prompt = config.style_prompt.clone();
            Ok(Box::new(CloudTtsProvider::new(
                sa_json,
                voice,
                language_code,
                model,
                style_prompt,
                volume,
            )))
        }
        "xai" | "xai_tts" | "grok" => {
            let api_key = config.get_xai_api_key().ok_or_else(|| {
                VoiceError::Config(
                    "xAI API key not found. Set in config or env var XAI_API_KEY".into(),
                )
            })?;

            // Voice is required — no hardcoded default.
            let voice = config.voice.clone().ok_or_else(|| {
                VoiceError::Config(
                    "xAI TTS voice is required. Specify in config, e.g., 'eve'".into(),
                )
            })?;
            let language = config.language_code.clone();
            Ok(Box::new(XaiTtsProvider::new(
                api_key, voice, language, volume,
            )))
        }
        "elevenlabs" | "eleven_labs" | "11labs" => {
            let api_key = config.get_elevenlabs_api_key().ok_or_else(|| {
                VoiceError::Config(
                    "ElevenLabs API key not found. Set in config or env var ELEVENLABS_API_KEY"
                        .into(),
                )
            })?;

            // Voice and model are required — no hardcoded defaults.
            let voice = config.voice.clone().ok_or_else(|| {
                VoiceError::Config(
                    "ElevenLabs voice is required. Specify a Voice ID in config.".into(),
                )
            })?;
            let model = config.model.clone().ok_or_else(|| {
                VoiceError::Config(
                    "ElevenLabs model is required. Specify in config, e.g., 'eleven_flash_v2_5'"
                        .into(),
                )
            })?;
            let speed = config.speed;
            let stability = config.stability;
            let style = config.style;
            Ok(Box::new(ElevenLabsProvider::new(
                api_key, voice, model, speed, stability, style, volume,
            )))
        }
        "openai" | "openai_tts" => {
            let api_key = config.get_openai_api_key().ok_or_else(|| {
                VoiceError::Config(
                    "OpenAI API key not found. Set in config or env var OPENAI_API_KEY".into(),
                )
            })?;

            // Model and voice are required — no hardcoded defaults.
            let model = config.model.clone().ok_or_else(|| {
                VoiceError::Config(
                    "OpenAI TTS model is required. Specify in config, e.g., 'gpt-4o-mini-tts'"
                        .into(),
                )
            })?;
            let voice = config.voice.clone().ok_or_else(|| {
                VoiceError::Config(
                    "OpenAI TTS voice is required. Specify in config, e.g., 'nova'".into(),
                )
            })?;
            let instructions = config.style_prompt.clone();
            let speed = config.speed;
            Ok(Box::new(OpenAiTtsProvider::new(
                api_key,
                model,
                voice,
                instructions,
                speed,
                volume,
            )))
        }
        "espeak" | "espeak_ng" | "espeak-ng" => {
            // No required field: espeak-ng ships its own default voice.
            let voice = config.voice.clone();
            let rate = config.rate.unwrap_or(espeak::DEFAULT_RATE);
            Ok(Box::new(EspeakProvider::new(voice, rate, volume)))
        }
        "piper" | "piper_tts" => {
            // For piper, the voice IS a downloaded .onnx model file. `voice`
            // takes precedence because it is the field the CLI overlays, so
            // `--voice ~/voices/zh.onnx` can select one.
            let model_path = piper::resolve_model_path(
                config.voice.as_deref(),
                config.model.as_deref(),
                config.path.as_deref(),
            )
            .ok_or_else(|| {
                VoiceError::Config(
                    "piper model is required. Set `model` (or `voice`) to a downloaded .onnx voice path."
                        .into(),
                )
            })?;
            let rate = config.rate.unwrap_or(piper::DEFAULT_RATE);
            Ok(Box::new(PiperProvider::new(model_path, rate, volume)))
        }
        "audio_file" | "audio" | "file" => {
            let path_str = config.path.as_ref().ok_or_else(|| {
                VoiceError::Config(
                    "Audio file provider requires 'path' field. Set to a file or directory path."
                        .into(),
                )
            })?;
            let expanded = shellexpand::tilde(path_str).to_string();
            let path = std::path::PathBuf::from(expanded);
            Ok(Box::new(crate::audio::AudioFileProvider::new(
                path, volume,
            )?))
        }
        _ => Err(VoiceError::Config(format!(
            "Unknown TTS provider: {}",
            config.name
        ))),
    }
}

/// Resolve a CLI/hook-selected TTS engine to a provider, sourcing all attributes
/// from the matching config entry. Only the voice/volume the caller explicitly set
/// override config; `rate` is taken from the caller (used by macos, espeak and piper,
/// ignored by the cloud engines). The engine must
/// exist in config — config is the single source of truth, so an absent engine is
/// an error and no provider/model/voice value is ever hardcoded here.
pub fn resolve_tts_provider(
    providers: &[TtsProviderConfig],
    aliases: &[&str],
    voice: Option<&str>,
    rate: u32,
    volume: Option<u32>,
) -> Result<Box<dyn TtsProvider>> {
    // Prefer the entry whose name exactly matches what the user asked for
    // (aliases[0]); aliases can map several names to one engine (e.g. cloud_tts
    // and gemini_tts), and config order must not override an explicit choice.
    let base = providers
        .iter()
        .find(|p| p.name.to_lowercase() == aliases[0])
        .or_else(|| {
            providers
                .iter()
                .find(|p| aliases.contains(&p.name.to_lowercase().as_str()))
        })
        .ok_or_else(|| {
            VoiceError::Config(format!("{} provider not found in config", aliases[0]))
        })?;

    let mut resolved = base.clone();
    if let Some(v) = voice {
        resolved.voice = Some(v.to_string());
    }
    if let Some(vol) = volume {
        resolved.volume = Some(vol);
    }
    resolved.rate = Some(rate);
    create_single_tts(&resolved)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_leading_audio_tag() {
        assert_eq!(
            strip_leading_audio_tag("[satisfied] 已經完成任務了"),
            "已經完成任務了"
        );
        assert_eq!(strip_leading_audio_tag("[excited] done"), "done");
        assert_eq!(strip_leading_audio_tag("沒有標籤的句子"), "沒有標籤的句子");
        assert_eq!(
            strip_leading_audio_tag("[123] not a tag"),
            "[123] not a tag"
        );
        assert_eq!(strip_leading_audio_tag("[unclosed tag"), "[unclosed tag");
    }

    #[test]
    fn test_tts_engine_from_str() {
        assert_eq!("macos".parse::<TtsEngine>().ok(), Some(TtsEngine::MacOS));
        assert_eq!("say".parse::<TtsEngine>().ok(), Some(TtsEngine::MacOS));
        assert_eq!("google".parse::<TtsEngine>().ok(), Some(TtsEngine::Google));
        assert_eq!(
            "google_tts".parse::<TtsEngine>().ok(),
            Some(TtsEngine::Google)
        );
        assert_eq!("gcloud".parse::<TtsEngine>().ok(), Some(TtsEngine::Google));
        assert_eq!("auto".parse::<TtsEngine>().ok(), Some(TtsEngine::Auto));
        assert!("unknown".parse::<TtsEngine>().is_err());

        // Display and FromStr agree on the canonical name of every engine
        for engine in [
            TtsEngine::MacOS,
            TtsEngine::Google,
            TtsEngine::CloudTts,
            TtsEngine::Xai,
            TtsEngine::ElevenLabs,
            TtsEngine::OpenAi,
            TtsEngine::AudioFile,
            TtsEngine::Espeak,
            TtsEngine::Piper,
            TtsEngine::Auto,
        ] {
            assert_eq!(engine.to_string().parse::<TtsEngine>().ok(), Some(engine));
        }
    }

    #[test]
    fn test_cloud_tts_engine_from_str() {
        assert_eq!(
            "cloud_tts".parse::<TtsEngine>().ok(),
            Some(TtsEngine::CloudTts)
        );
        assert_eq!(
            "gcp_tts".parse::<TtsEngine>().ok(),
            Some(TtsEngine::CloudTts)
        );
        assert_eq!(
            "google_cloud".parse::<TtsEngine>().ok(),
            Some(TtsEngine::CloudTts)
        );
        assert_eq!(
            "gemini_tts".parse::<TtsEngine>().ok(),
            Some(TtsEngine::CloudTts)
        );
    }

    #[test]
    fn test_factory_recognizes_gemini_tts_alias() {
        // gemini_tts routes through the cloud_tts branch; missing service
        // account surfaces the cloud_tts error, proving the alias is wired in
        // (not the "Unknown TTS provider" fallthrough).
        let config = TtsProviderConfig {
            name: "gemini_tts".to_string(),
            model: Some("gemini-2.5-flash-tts".to_string()),
            voice: Some("Kore".to_string()),
            api_key: None,
            rate: None,
            volume: None,
            path: None,
            service_account_key: None,
            language_code: None,
            speed: None,
            stability: None,
            style: None,
            style_prompt: Some("Say it warmly.".to_string()),
        };
        let err = match create_single_tts(&config) {
            Ok(_) => panic!("expected error without service account key"),
            Err(e) => e.to_string(),
        };
        assert!(
            err.contains("service_account_key"),
            "unexpected error: {err}"
        );
        assert!(!err.contains("Unknown TTS provider"));
    }

    #[test]
    fn test_resolve_prefers_exact_name_over_alias_order() {
        // cloud_tts and gemini_tts share TtsEngine::CloudTts. With a cloud_tts
        // entry listed FIRST in config, resolving with aliases[0] = "gemini_tts"
        // must still pick the gemini_tts entry, not the first alias match.
        let base = TtsProviderConfig {
            name: "cloud_tts".to_string(),
            model: None,
            // No voice: selecting this entry fails with "voice is required",
            // which discriminates it from the gemini_tts entry below.
            voice: None,
            api_key: None,
            rate: None,
            volume: None,
            path: None,
            // /dev/null reads as empty content, passing the sa-key lookup.
            service_account_key: Some("/dev/null".to_string()),
            language_code: None,
            speed: None,
            stability: None,
            style: None,
            style_prompt: None,
        };
        let gemini = TtsProviderConfig {
            name: "gemini_tts".to_string(),
            model: Some("gemini-2.5-flash-tts".to_string()),
            voice: Some("Kore".to_string()),
            ..base.clone()
        };
        let providers = vec![base, gemini];

        let resolved = resolve_tts_provider(
            &providers,
            &["gemini_tts", "cloud_tts", "gcp_tts", "google_cloud"],
            None,
            200,
            None,
        );
        assert!(
            resolved.is_ok(),
            "expected gemini_tts entry, got: {:?}",
            resolved.err()
        );
    }

    // The alias arrays below are exactly the ones the dispatch arms in
    // main.rs and hooks/claude_code.rs pass; the arms' existence is enforced by
    // the compiler (both `match tts_engine` blocks are exhaustive).
    const ESPEAK_ALIASES: &[&str] = &["espeak", "espeak_ng", "espeak-ng"];
    const PIPER_ALIASES: &[&str] = &["piper", "piper_tts"];

    #[test]
    fn test_dispatch_resolves_espeak_from_config() {
        let providers = vec![espeak_config("espeak", Some("cmn+f3"), Some(175))];
        let provider = resolve_tts_provider(&providers, ESPEAK_ALIASES, None, 200, None)
            .expect("configured espeak entry should resolve");
        assert_eq!(provider.name(), "espeak");
    }

    #[test]
    fn test_dispatch_espeak_errors_when_absent_from_config() {
        let providers: Vec<TtsProviderConfig> = vec![];
        let err = resolve_tts_provider(&providers, ESPEAK_ALIASES, None, 200, None)
            .err()
            .expect("an unconfigured engine must error")
            .to_string();
        assert!(
            err.contains("espeak provider not found in config"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_dispatch_resolves_piper_with_cli_voice_override() {
        // The CLI `--voice` overlay must reach piper's model knob.
        let providers = vec![piper_config(None, Some("/m/a.onnx"), None)];
        let provider =
            resolve_tts_provider(&providers, PIPER_ALIASES, Some("/m/b.onnx"), 200, None)
                .expect("configured piper entry should resolve");
        assert_eq!(provider.name(), "piper");
    }

    #[test]
    fn test_dispatch_piper_errors_when_absent_from_config() {
        let providers: Vec<TtsProviderConfig> = vec![];
        let err = resolve_tts_provider(&providers, PIPER_ALIASES, None, 200, None)
            .err()
            .expect("an unconfigured engine must error")
            .to_string();
        assert!(
            err.contains("piper provider not found in config"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_espeak_engine_from_str_accepts_all_aliases() {
        assert_eq!("espeak".parse::<TtsEngine>().ok(), Some(TtsEngine::Espeak));
        assert_eq!(
            "espeak-ng".parse::<TtsEngine>().ok(),
            Some(TtsEngine::Espeak)
        );
        assert_eq!(
            "espeak_ng".parse::<TtsEngine>().ok(),
            Some(TtsEngine::Espeak)
        );
        // Parsing is case-insensitive, like every other engine tag.
        assert_eq!("ESPEAK".parse::<TtsEngine>().ok(), Some(TtsEngine::Espeak));
    }

    #[test]
    fn test_piper_engine_from_str_accepts_all_aliases() {
        assert_eq!("piper".parse::<TtsEngine>().ok(), Some(TtsEngine::Piper));
        assert_eq!(
            "piper_tts".parse::<TtsEngine>().ok(),
            Some(TtsEngine::Piper)
        );
    }

    #[test]
    fn test_espeakng_is_not_an_alias() {
        // Only the three documented spellings map to Espeak.
        assert!("espeakng".parse::<TtsEngine>().is_err());
    }

    fn espeak_config(name: &str, voice: Option<&str>, rate: Option<u32>) -> TtsProviderConfig {
        TtsProviderConfig {
            name: name.to_string(),
            model: None,
            voice: voice.map(str::to_string),
            path: None,
            api_key: None,
            rate,
            volume: None,
            service_account_key: None,
            language_code: None,
            speed: None,
            stability: None,
            style: None,
            style_prompt: None,
        }
    }

    #[test]
    fn test_factory_builds_espeak_from_alias_with_voice_and_rate() {
        let provider = create_single_tts(&espeak_config("espeak_ng", Some("cmn+f3"), Some(175)))
            .expect("espeak_ng entry should build");
        assert_eq!(provider.name(), "espeak");
    }

    #[test]
    fn test_factory_builds_espeak_with_no_voice_or_rate() {
        let provider = create_single_tts(&espeak_config("espeak", None, None))
            .expect("a bare espeak entry needs no configuration");
        assert_eq!(provider.name(), "espeak");
    }

    #[test]
    fn test_factory_builds_piper_from_model() {
        let provider = create_single_tts(&piper_config(None, Some("/m/zh.onnx"), None))
            .expect("piper entry with a model should build");
        assert_eq!(provider.name(), "piper");
    }

    fn piper_config(
        voice: Option<&str>,
        model: Option<&str>,
        path: Option<&str>,
    ) -> TtsProviderConfig {
        TtsProviderConfig {
            name: "piper".to_string(),
            model: model.map(str::to_string),
            voice: voice.map(str::to_string),
            path: path.map(str::to_string),
            api_key: None,
            rate: None,
            volume: None,
            service_account_key: None,
            language_code: None,
            speed: None,
            stability: None,
            style: None,
            style_prompt: None,
        }
    }

    /// The `.onnx` path a built piper provider resolved to.
    fn built_model_path(config: &TtsProviderConfig) -> String {
        let provider = create_single_tts(config).expect("piper entry should build");
        assert_eq!(provider.name(), "piper");
        // Rebuild concretely to inspect the resolved model path.
        piper::resolve_model_path(
            config.voice.as_deref(),
            config.model.as_deref(),
            config.path.as_deref(),
        )
        .expect("a model path must have resolved")
        .to_string_lossy()
        .to_string()
    }

    #[test]
    fn test_piper_without_a_model_reports_a_piper_error() {
        let err = create_single_tts(&piper_config(None, None, None))
            .err()
            .expect("piper with no model must not build")
            .to_string();
        assert!(err.contains("piper model"), "unexpected error: {err}");
        // Proves the arm is wired in, not falling through to the catch-all.
        assert!(!err.contains("Unknown TTS provider"), "unexpected: {err}");
    }

    #[test]
    fn test_piper_builds_from_model_field() {
        let config = piper_config(None, Some("/m/zh.onnx"), None);
        assert_eq!(built_model_path(&config), "/m/zh.onnx");
    }

    #[test]
    fn test_piper_voice_field_wins_over_model() {
        // The CLI overlays `voice`, so it must be able to select the model.
        let config = piper_config(Some("/m/b.onnx"), Some("/m/a.onnx"), None);
        assert_eq!(built_model_path(&config), "/m/b.onnx");
    }

    #[test]
    fn test_piper_blank_voice_falls_back_to_model() {
        let config = piper_config(Some("   "), Some("/m/a.onnx"), None);
        assert_eq!(built_model_path(&config), "/m/a.onnx");
    }

    #[test]
    fn test_piper_builds_from_path_field() {
        let config = piper_config(None, None, Some("/m/c.onnx"));
        assert_eq!(built_model_path(&config), "/m/c.onnx");
    }

    #[test]
    fn test_piper_expands_tilde_in_model_path() {
        let config = piper_config(None, Some("~/v/zh.onnx"), None);
        let resolved = built_model_path(&config);
        assert!(
            !resolved.starts_with('~'),
            "tilde must be expanded: {resolved}"
        );
        assert!(resolved.ends_with("/v/zh.onnx"), "unexpected: {resolved}");
    }

    #[test]
    fn test_openai_engine_from_str_and_display() {
        assert_eq!("openai".parse::<TtsEngine>().ok(), Some(TtsEngine::OpenAi));
        assert_eq!(
            "openai_tts".parse::<TtsEngine>().ok(),
            Some(TtsEngine::OpenAi)
        );
        assert_eq!(TtsEngine::OpenAi.to_string(), "openai");
    }

    fn openai_config(model: Option<&str>, voice: Option<&str>) -> TtsProviderConfig {
        TtsProviderConfig {
            name: "openai".to_string(),
            model: model.map(str::to_string),
            voice: voice.map(str::to_string),
            api_key: Some("test-api-key".to_string()),
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
    fn test_openai_requires_voice() {
        let err = create_single_tts(&openai_config(Some("gpt-4o-mini-tts"), None))
            .err()
            .expect("expected error without voice")
            .to_string();
        assert!(err.contains("voice is required"), "unexpected error: {err}");
    }

    #[test]
    fn test_openai_requires_model() {
        let err = create_single_tts(&openai_config(None, Some("nova")))
            .err()
            .expect("expected error without model")
            .to_string();
        assert!(err.contains("model is required"), "unexpected error: {err}");
    }

    #[test]
    fn test_openai_fully_specified_config() {
        let provider = create_single_tts(&openai_config(Some("gpt-4o-mini-tts"), Some("nova")))
            .expect("fully specified openai entry should build");
        assert_eq!(provider.name(), "openai");
        assert!(provider.is_available());
    }

    #[test]
    fn test_resolve_openai_errors_when_absent() {
        let providers: Vec<TtsProviderConfig> = vec![];
        let err = resolve_tts_provider(&providers, &["openai", "openai_tts"], None, 200, None)
            .err()
            .expect("expected error with empty config")
            .to_string();
        assert!(
            err.contains("openai provider not found in config"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_google_still_maps_to_google() {
        assert!(matches!(
            "google".parse::<TtsEngine>(),
            Ok(TtsEngine::Google)
        ));
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_create_macos_tts() {
        let providers = vec![TtsProviderConfig {
            name: "macos".to_string(),
            model: None,
            voice: Some("Tingting".to_string()),
            api_key: None,
            rate: Some(200),
            volume: Some(80),
            path: None,
            service_account_key: None,
            language_code: None,
            speed: None,
            stability: None,
            style: None,
            style_prompt: None,
        }];

        let result = create_tts_from_config(&providers);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().name(), "macos");
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_create_tts_fallback() {
        // Google TTS without API key should fallback to macOS
        let providers = vec![
            TtsProviderConfig {
                name: "google".to_string(),
                model: Some("gemini-2.5-flash-preview-tts".to_string()),
                voice: Some("Zephyr".to_string()),
                api_key: None, // No API key
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
                voice: Some("Tingting".to_string()),
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
        ];

        // Every env var the google provider's key lookup reads. Only removed, never
        // set, anywhere in the suite, so concurrent tests cannot observe a flip.
        for var in [
            "GOOGLE_CLOUD_PROJECT",
            "GCP_PROJECT",
            "GEMINI_API_KEY",
            "GOOGLE_API_KEY",
        ] {
            std::env::remove_var(var);
        }

        let result = create_tts_from_config(&providers);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().name(), "macos");
    }

    #[test]
    fn test_resolve_tts_provider_uses_config_and_cli_override() {
        let providers = vec![TtsProviderConfig {
            name: "macos".to_string(),
            model: None,
            voice: Some("Meijia".to_string()),
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
        }];

        // CLI voice override wins over config voice; engine sourced from config.
        let result = resolve_tts_provider(
            &providers,
            &["macos", "say"],
            Some("Tingting"),
            250,
            Some(80),
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap().name(), "macos");
    }

    #[test]
    fn test_resolve_tts_provider_errors_when_engine_absent() {
        let providers: Vec<TtsProviderConfig> = vec![];
        let result = resolve_tts_provider(&providers, &["google", "gemini"], None, 200, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_tts_provider_macos_errors_when_absent() {
        // config is the single source of truth: an unconfigured engine errors,
        // even the credential-free macOS one.
        let providers: Vec<TtsProviderConfig> = vec![];
        let result = resolve_tts_provider(&providers, &["macos", "say"], None, 200, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_create_tts_empty_providers() {
        let providers: Vec<TtsProviderConfig> = vec![];

        let result = create_tts_from_config(&providers);
        assert!(result.is_err());
        let err = result.err().unwrap();
        assert!(err.to_string().contains("No TTS provider"));
    }
}
