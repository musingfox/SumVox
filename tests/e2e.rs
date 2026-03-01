// E2E tests for sumvox binary
// Tests only external behavior: stdin/stdout/stderr/exit code
// Requires: config/e2e_test.toml with real API keys
//
// Run: cargo test --test e2e
// Debug single test: cargo test --test e2e test_name -- --nocapture

use assert_cmd::cargo::cargo_bin_cmd;
use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

// ============================================================================
// Test Infrastructure
// ============================================================================

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
        fs::write(config_dir.join("config.toml"), content).unwrap();
        self.home_dir.path()
    }

    fn cmd(&self) -> Command {
        let mut cmd = cargo_bin_cmd!("sumvox");
        cmd.env("HOME", self.home_dir.path());
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

fn notification_json(message: &str, notification_type: &str) -> String {
    serde_json::json!({
        "session_id": "e2e-test",
        "transcript_path": "/tmp/fake-transcript.jsonl",
        "hook_event_name": "Notification",
        "message": message,
        "notification_type": notification_type
    })
    .to_string()
}

fn notification_json_stop_active() -> String {
    serde_json::json!({
        "session_id": "e2e-test",
        "transcript_path": "/tmp/fake-transcript.jsonl",
        "hook_event_name": "Notification",
        "stop_hook_active": true,
        "message": "Should be ignored",
        "notification_type": "permission_prompt"
    })
    .to_string()
}

fn config_without_llm() -> String {
    r#"version = "1.2.0"

[llm]
providers = []
[llm.parameters]
max_tokens = 100
temperature = 0.3

[tts]
[[tts.providers]]
name = "macos"
rate = 200

[summarization]
turns = 1
system_message = "Test"
prompt_template = "Summarize: {context}"
fallback_message = "Test completed"

[hooks.claude_code]
notification_filter = ["*"]
notification_tts_provider = "macos"
stop_tts_provider = "macos"
"#
    .to_string()
}

fn config_no_tts() -> String {
    r#"version = "1.2.0"

[llm]
providers = []
[llm.parameters]
max_tokens = 100
temperature = 0.3

[tts]
providers = []

[summarization]
turns = 1
system_message = "Test"
prompt_template = "Summarize: {context}"
fallback_message = "Test completed"

[hooks.claude_code]
notification_filter = ["*"]
"#
    .to_string()
}

fn config_with_audio_file(path: &str) -> String {
    format!(
        r#"version = "1.2.0"

[llm]
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

fn config_with_queue(timeout: u64) -> String {
    format!(
        r#"version = "1.2.0"

[llm]
providers = []
[llm.parameters]
max_tokens = 100
temperature = 0.3

[tts]
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
queue_timeout = {timeout}
notification_tts_provider = "macos"
stop_tts_provider = "macos"
"#
    )
}

fn config_with_specific_filter(types: &[&str]) -> String {
    let filter = types
        .iter()
        .map(|t| format!("\"{}\"", t))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        r#"version = "1.2.0"

[llm]
providers = []
[llm.parameters]
max_tokens = 100
temperature = 0.3

[tts]
[[tts.providers]]
name = "macos"
rate = 200

[summarization]
turns = 1
system_message = "Test"
prompt_template = "Summarize: {{context}}"
fallback_message = "Test completed"

[hooks.claude_code]
notification_filter = [{filter}]
notification_tts_provider = "macos"
stop_tts_provider = "macos"
"#
    )
}

// ============================================================================
// CLI Basic Behavior (3)
// ============================================================================

#[test]
fn test_version() {
    cargo_bin_cmd!("sumvox")
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("sumvox"));
}

#[test]
fn test_help() {
    cargo_bin_cmd!("sumvox")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("say"))
        .stdout(predicate::str::contains("sum"))
        .stdout(predicate::str::contains("json"))
        .stdout(predicate::str::contains("init"));
}

#[test]
fn test_empty_stdin() {
    let env = TestEnv::new();

    env.cmd()
        .arg("json")
        .write_stdin("")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Empty JSON input"));
}

// ============================================================================
// Init Command (2)
// ============================================================================

#[test]
fn test_init_creates_config() {
    let env = TestEnv::new();

    env.cmd().arg("init").assert().success();

    let config_path = env.home_path().join(".config/sumvox/config.toml");
    assert!(
        config_path.exists(),
        "config.toml should be created by init"
    );
}

#[test]
fn test_init_force() {
    let env = TestEnv::new();

    // Create a legacy config.yaml to trigger "already exists" check
    let config_dir = env.home_path().join(".config/sumvox");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(config_dir.join("config.yaml"), "version: '1.0.0'").unwrap();

    // Without --force: should report existing config
    env.cmd()
        .arg("init")
        .assert()
        .success()
        .stderr(predicate::str::contains("already exists"));

    // With --force: should overwrite and create config.toml
    env.cmd().args(["init", "--force"]).assert().success();

    let config_path = config_dir.join("config.toml");
    assert!(
        config_path.exists(),
        "config.toml should be created after init --force"
    );
}

// ============================================================================
// LLM — sum Command (4)
// ============================================================================

#[test]
fn test_sum_no_speak() {
    let env = TestEnv::new();
    env.setup_base_config();

    env.cmd()
        .args([
            "sum",
            "The quick brown fox jumps over the lazy dog",
            "--no-speak",
        ])
        .timeout(std::time::Duration::from_secs(30))
        .assert()
        .success()
        .stdout(predicate::str::is_empty().not());
}

#[test]
fn test_sum_empty_text() {
    let env = TestEnv::new();
    env.setup_base_config();

    env.cmd()
        .args(["sum", ""])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Empty text provided"));
}

#[test]
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

#[test]
fn test_sum_no_llm_config() {
    let env = TestEnv::new();
    env.setup_with_config(&config_without_llm());

    // With no LLM providers, summary is empty → warning printed, exit 0
    env.cmd()
        .args(["sum", "Hello world", "--no-speak"])
        .assert()
        .success()
        .stderr(predicate::str::contains("Empty summary generated"));
}

// ============================================================================
// TTS — say Command (4)
// ============================================================================

#[test]
fn test_say_macos() {
    let env = TestEnv::new();
    env.setup_base_config();

    env.cmd()
        .args(["say", "hello", "--tts", "macos"])
        .assert()
        .success();
}

#[test]
fn test_say_volume() {
    let env = TestEnv::new();
    env.setup_base_config();

    env.cmd()
        .args(["say", "hello", "--tts", "macos", "--volume", "50"])
        .assert()
        .success();
}

#[test]
fn test_say_unknown_tts() {
    let env = TestEnv::new();
    // Use config with no TTS providers — "nonexistent" falls back to Auto,
    // Auto with empty providers → error
    env.setup_with_config(&config_no_tts());

    env.cmd()
        .args(["say", "hello", "--tts", "nonexistent"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("No TTS provider"));
}

#[test]
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
// Audio File Provider (4)
// ============================================================================

#[test]
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

#[test]
fn test_say_audio_directory() {
    let env = TestEnv::new();

    // Create a directory with multiple WAV files
    let audio_dir = env.home_path().join("sounds");
    fs::create_dir_all(&audio_dir).unwrap();
    create_minimal_wav(&audio_dir.join("sound1.wav"));
    create_minimal_wav(&audio_dir.join("sound2.wav"));

    env.setup_with_config(&config_with_audio_file(audio_dir.to_str().unwrap()));

    env.cmd()
        .args(["say", "ignored text", "--tts", "audio_file"])
        .assert()
        .success();
}

#[test]
fn test_say_audio_missing_path() {
    let env = TestEnv::new();
    env.setup_with_config(&config_with_audio_file("/nonexistent/path/sound.wav"));

    env.cmd()
        .args(["say", "ignored text", "--tts", "audio_file"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("does not exist"));
}

#[test]
fn test_say_audio_no_config() {
    let env = TestEnv::new();
    env.setup_base_config(); // base config has no audio_file provider

    env.cmd()
        .args(["say", "hello", "--tts", "audio_file"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("audio_file provider not found"));
}

// ============================================================================
// LLM + TTS Full Flow (2)
// ============================================================================

#[test]
fn test_sum_full_flow_macos() {
    let env = TestEnv::new();
    env.setup_base_config();

    env.cmd()
        .args([
            "sum",
            "Explain Rust ownership in one sentence",
            "--tts",
            "macos",
        ])
        .timeout(std::time::Duration::from_secs(30))
        .assert()
        .success()
        .stdout(predicate::str::is_empty().not());
}

#[test]
fn test_sum_full_flow_google_tts() {
    let env = TestEnv::new();
    env.setup_base_config();

    env.cmd()
        .args(["sum", "Hello world", "--tts", "google"])
        .timeout(std::time::Duration::from_secs(30))
        .assert()
        .success()
        .stdout(predicate::str::is_empty().not());
}

// ============================================================================
// Hook Dispatch (3)
// ============================================================================

#[test]
fn test_notification_hook() {
    let env = TestEnv::new();
    env.setup_base_config();

    let json = notification_json("Test notification", "permission_prompt");

    env.cmd_debug()
        .arg("json")
        .write_stdin(json)
        .timeout(std::time::Duration::from_secs(15))
        .assert()
        .success()
        .stdout(predicate::str::contains("Speaking notification"));
}

#[test]
fn test_notification_filtered() {
    let env = TestEnv::new();
    // Config only allows "permission_prompt" — send "auth_success" which is not in filter
    env.setup_with_config(&config_with_specific_filter(&["permission_prompt"]));

    let json = notification_json("Should be filtered", "auth_success");

    env.cmd_debug()
        .arg("json")
        .write_stdin(json)
        .assert()
        .success()
        .stdout(predicate::str::contains("not in filter"));
}

#[test]
fn test_stop_hook_active() {
    let env = TestEnv::new();
    env.setup_base_config();

    let json = notification_json_stop_active();

    env.cmd_debug()
        .arg("json")
        .write_stdin(json)
        .assert()
        .success()
        .stdout(predicate::str::contains("preventing infinite loop"));
}

// ============================================================================
// Notification Queue (3)
// ============================================================================

#[test]
fn test_queue_lock_acquired() {
    let env = TestEnv::new();
    env.setup_base_config();

    let json = notification_json("Queue test", "permission_prompt");

    env.cmd_debug()
        .arg("json")
        .write_stdin(json)
        .timeout(std::time::Duration::from_secs(15))
        .assert()
        .success()
        .stdout(predicate::str::contains("Queue lock acquired"));
}

#[test]
fn test_queue_disabled() {
    let env = TestEnv::new();
    env.setup_with_config(&config_with_queue(0));

    let json = notification_json("Queue disabled test", "permission_prompt");

    env.cmd_debug()
        .arg("json")
        .write_stdin(json)
        .timeout(std::time::Duration::from_secs(15))
        .assert()
        .success()
        .stdout(predicate::str::contains("queue disabled"));
}

#[test]
fn test_queue_concurrent() {
    use std::process::Stdio;

    let env = TestEnv::new();
    env.setup_base_config();

    let json1 = notification_json("Concurrent A", "permission_prompt");
    let json2 = notification_json("Concurrent B", "permission_prompt");
    let home = env.home_path().to_path_buf();
    let bin = assert_cmd::cargo::cargo_bin!("sumvox");

    // Spawn two child processes sharing the same HOME (same queue lock file)
    let mut child1 = std::process::Command::new(bin)
        .arg("json")
        .env("HOME", &home)
        .env("RUST_LOG", "debug")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn child 1");

    let mut child2 = std::process::Command::new(bin)
        .arg("json")
        .env("HOME", &home)
        .env("RUST_LOG", "debug")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn child 2");

    // Write JSON to both processes' stdin
    use std::io::Write;
    child1
        .stdin
        .take()
        .unwrap()
        .write_all(json1.as_bytes())
        .unwrap();
    child2
        .stdin
        .take()
        .unwrap()
        .write_all(json2.as_bytes())
        .unwrap();

    // Wait for both to complete
    let output1 = child1
        .wait_with_output()
        .expect("Failed to wait for child 1");
    let output2 = child2
        .wait_with_output()
        .expect("Failed to wait for child 2");

    assert!(
        output1.status.success(),
        "Child 1 failed: {}",
        String::from_utf8_lossy(&output1.stderr)
    );
    assert!(
        output2.status.success(),
        "Child 2 failed: {}",
        String::from_utf8_lossy(&output2.stderr)
    );
}
