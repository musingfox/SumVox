// CLI argument parsing for sumvox
// Subcommand-based architecture for versatile voice notification

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "sumvox")]
#[command(about = "Voice notification CLI for AI coding tools")]
#[command(version)]
pub struct Cli {
    /// Subcommand to execute (optional: auto-detect json mode from stdin if not specified)
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Direct TTS playback - speak text immediately
    Say(SayArgs),

    /// LLM summarization with TTS - summarize text then speak
    Sum(SumArgs),

    /// Read JSON from stdin (hook mode) - auto-detect format
    Json(JsonArgs),

    /// Initialize config file at ~/.config/sumvox/config.toml
    Init(InitArgs),
}

/// Arguments for 'say' subcommand
#[derive(Parser, Debug, Clone)]
pub struct SayArgs {
    /// Text to speak
    pub text: String,

    /// TTS engine: auto, macos, espeak, piper, google, cloud_tts, xai, elevenlabs, openai, audio_file
    #[arg(long, default_value = "auto")]
    pub tts: String,

    /// Voice name (engine-specific)
    ///
    /// macos: Tingting, Meijia, etc.
    /// google: Aoede, Charon, Fenrir, Kore, Puck, Orus.
    /// espeak: an espeak-ng voice, optionally with a variant, e.g. cmn or cmn+f3.
    /// piper: the path to a downloaded .onnx voice model,
    /// e.g. ~/voices/zh_CN-huayan-medium.onnx
    #[arg(long)]
    pub voice: Option<String>,

    /// Speech rate in words per minute
    ///
    /// macos: passed to `say -r`.
    /// espeak: passed to `espeak-ng -s` (clamped to 80-450).
    /// piper: mapped onto --length-scale, where 200 is piper's default speed and
    /// a higher value speaks faster.
    /// Ignored by the cloud engines (google, cloud_tts, xai, elevenlabs, openai).
    #[arg(long, default_value = "200")]
    pub rate: u32,

    /// Volume level (0-100)
    #[arg(long)]
    pub volume: Option<u32>,
}

/// Arguments for 'sum' subcommand
#[derive(Parser, Debug, Clone)]
pub struct SumArgs {
    /// Text to summarize (use "-" to read from stdin)
    pub text: String,

    /// LLM provider: google, anthropic, openai, ollama
    #[arg(long)]
    pub provider: Option<String>,

    /// Model name (e.g., gemini-2.5-flash, gpt-4o-mini)
    #[arg(long)]
    pub model: Option<String>,

    /// Only output summary, don't speak
    #[arg(long)]
    pub no_speak: bool,

    /// Request timeout in seconds
    #[arg(long, default_value = "10")]
    pub timeout: u64,

    /// TTS engine: auto, macos, espeak, piper, google, cloud_tts, xai, elevenlabs, openai, audio_file
    #[arg(long, default_value = "auto")]
    pub tts: String,

    /// Voice name (engine-specific)
    ///
    /// macos: Tingting, Meijia, etc.
    /// google: Aoede, Charon, Fenrir, Kore, Puck, Orus.
    /// espeak: an espeak-ng voice, optionally with a variant, e.g. cmn or cmn+f3.
    /// piper: the path to a downloaded .onnx voice model,
    /// e.g. ~/voices/zh_CN-huayan-medium.onnx
    #[arg(long)]
    pub voice: Option<String>,

    /// Speech rate in words per minute
    ///
    /// macos: passed to `say -r`.
    /// espeak: passed to `espeak-ng -s` (clamped to 80-450).
    /// piper: mapped onto --length-scale, where 200 is piper's default speed and
    /// a higher value speaks faster.
    /// Ignored by the cloud engines (google, cloud_tts, xai, elevenlabs, openai).
    #[arg(long, default_value = "200")]
    pub rate: u32,

    /// Volume level (0-100)
    #[arg(long)]
    pub volume: Option<u32>,
}

/// Arguments for 'json' subcommand (hook mode)
#[derive(Parser, Debug, Clone)]
pub struct JsonArgs {
    /// JSON format: auto, claude-code, generic
    #[arg(long, default_value = "auto")]
    pub format: String,
}

/// Arguments for 'init' subcommand
#[derive(Parser, Debug, Clone)]
pub struct InitArgs {
    /// Force overwrite existing config
    #[arg(long)]
    pub force: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn test_cli_verify() {
        Cli::command().debug_assert();
    }
}
