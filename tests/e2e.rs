// E2E tests for sumvox binary
// Tests only external behavior: stdin/stdout/stderr/exit code
// Requires: config/e2e_test.toml with real API keys
//
// The local TTS engine is platform-dependent: `macos` on macOS, `espeak` elsewhere.
// Every config installed by the harness is written with `macos`; on non-macOS the
// harness rewrites that literal to `espeak` so one config serves both platforms.
//
// Every test is #[ignore]d so plain `cargo test` stays offline and silent.
// Run: cargo test --test e2e -- --ignored
// Debug single test: cargo test --test e2e test_name -- --ignored --nocapture

use assert_cmd::cargo::cargo_bin_cmd;
use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

// ============================================================================
// Test Infrastructure
// ============================================================================

/// The local, offline TTS engine for the platform under test.
const LOCAL_TTS: &str = if cfg!(target_os = "macos") {
    "macos"
} else {
    "espeak"
};

/// Swap the `macos` engine for the platform's local engine in a TOML config.
fn localize_config(content: &str) -> String {
    content.replace("\"macos\"", &format!("\"{LOCAL_TTS}\""))
}

struct TestEnv {
    home_dir: TempDir,
}

impl TestEnv {
    fn new() -> Self {
        let home_dir = TempDir::new().expect("Failed to create temp dir");
        Self { home_dir }
    }

    /// Install the real e2e_test.toml config into the isolated HOME
    fn setup_base_config(&self) -> &Path {
        let config_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("config/e2e_test.toml");
        let base_config = fs::read_to_string(&config_path).expect(
            "config/e2e_test.toml not found. \
             Copy from config/e2e_test.toml.example and fill in real API keys.",
        );
        self.install_config(&base_config)
    }

    /// Install a custom TOML config into the isolated HOME
    fn setup_with_config(&self, toml_content: &str) -> &Path {
        self.install_config(toml_content)
    }

    fn install_config(&self, content: &str) -> &Path {
        let config_dir = self.home_dir.path().join(".config/sumvox");
        fs::create_dir_all(&config_dir).unwrap();
        fs::write(config_dir.join("config.toml"), localize_config(content)).unwrap();
        self.home_dir.path()
    }

    fn cmd(&self) -> Command {
        let mut cmd = cargo_bin_cmd!("sumvox");
        cmd.env("HOME", self.home_dir.path());
        cmd.env_remove("SUMVOX_DISABLE");
        cmd
    }

    fn cmd_debug(&self) -> Command {
        let mut cmd = self.cmd();
        cmd.env("RUST_LOG", "debug");
        cmd
    }

    fn home_path(&self) -> &Path {
        self.home_dir.path()
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Create a minimal valid PCM WAV file (44100 Hz, 16-bit, mono, ~0.1s silence)
fn create_minimal_wav(path: &Path) {
    let num_samples: u32 = 4410;
    let data_size: u32 = num_samples * 2; // 16-bit = 2 bytes/sample
    let file_size: u32 = 36 + data_size;

    let mut wav = Vec::with_capacity(44 + data_size as usize);
    // RIFF header
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&file_size.to_le_bytes());
    wav.extend_from_slice(b"WAVE");
    // fmt chunk
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes()); // chunk size
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM format
    wav.extend_from_slice(&1u16.to_le_bytes()); // mono
    wav.extend_from_slice(&44100u32.to_le_bytes()); // sample rate
    wav.extend_from_slice(&88200u32.to_le_bytes()); // byte rate
    wav.extend_from_slice(&2u16.to_le_bytes()); // block align
    wav.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
                                                 // data chunk
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_size.to_le_bytes());
    wav.extend_from_slice(&vec![0u8; data_size as usize]); // silence

    fs::write(path, &wav).unwrap();
}

fn config_with_audio_file(path: &str) -> String {
    format!(
        r#"[llm]
providers = []
[llm.parameters]
max_tokens = 100
temperature = 0.3

[tts]
[[tts.providers]]
name = "audio_file"
path = "{path}"

[[tts.providers]]
name = "macos"
rate = 200

[summarization]
turns = 1
system_message = "Test"
prompt_template = "Summarize: {{context}}"
fallback_message = "Test completed"

[hooks.claude_code]
notification_filter = ["*"]
notification_tts_provider = "macos"
stop_tts_provider = "macos"
"#
    )
}

// ============================================================================
// LLM — sum Command
// ============================================================================

#[test]
#[ignore = "e2e-network"]
fn test_sum_stdin() {
    let env = TestEnv::new();
    env.setup_base_config();

    env.cmd()
        .args(["sum", "-", "--no-speak"])
        .write_stdin("Rust is a systems programming language focused on safety and performance.")
        .timeout(std::time::Duration::from_secs(30))
        .assert()
        .success()
        .stdout(predicate::str::is_empty().not());
}

// ============================================================================
// TTS — say Command
// ============================================================================

#[test]
#[ignore = "e2e-network"]
fn test_say_google_tts() {
    let env = TestEnv::new();
    env.setup_base_config();

    env.cmd()
        .args(["say", "hello", "--tts", "google"])
        .timeout(std::time::Duration::from_secs(30))
        .assert()
        .success();
}

// ============================================================================
// Audio File Provider
// ============================================================================

#[test]
#[ignore = "e2e-audio"]
fn test_say_audio_single_file() {
    let env = TestEnv::new();

    // Create a valid WAV file in temp dir
    let wav_path = env.home_path().join("test.wav");
    create_minimal_wav(&wav_path);

    env.setup_with_config(&config_with_audio_file(wav_path.to_str().unwrap()));

    env.cmd_debug()
        .args(["say", "ignored text", "--tts", "audio_file"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Playing audio file"));
}

// ============================================================================
// LLM + TTS Full Flow
// ============================================================================

#[test]
#[ignore = "e2e-audio"]
fn test_sum_full_flow_local() {
    let env = TestEnv::new();
    env.setup_base_config();

    env.cmd()
        .args([
            "sum",
            "Explain Rust ownership in one sentence",
            "--tts",
            LOCAL_TTS,
        ])
        .timeout(std::time::Duration::from_secs(30))
        .assert()
        .success()
        .stdout(predicate::str::is_empty().not());
}
