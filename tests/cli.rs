// Hermetic CLI tests for the sumvox binary: no network, no audio, no real config.
// Each test runs with HOME pointed at a temp dir so ~/.config/sumvox is never touched.

use assert_cmd::cargo::cargo_bin_cmd;
use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

/// The local TTS engine for the platform under test (configs below name `macos`).
const LOCAL_TTS: &str = if cfg!(target_os = "macos") {
    "macos"
} else {
    "espeak"
};

struct TestEnv {
    home_dir: TempDir,
}

impl TestEnv {
    fn new() -> Self {
        let home_dir = TempDir::new().expect("Failed to create temp dir");
        Self { home_dir }
    }

    fn setup_with_config(&self, toml_content: &str) -> &Path {
        let config_dir = self.home_dir.path().join(".config/sumvox");
        fs::create_dir_all(&config_dir).unwrap();
        fs::write(
            config_dir.join("config.toml"),
            toml_content.replace("\"macos\"", &format!("\"{LOCAL_TTS}\"")),
        )
        .unwrap();
        self.home_dir.path()
    }

    /// Create the mute flag so playback short-circuits before any audio device is touched.
    fn mute(&self) {
        let config_dir = self.home_dir.path().join(".config/sumvox");
        fs::create_dir_all(&config_dir).unwrap();
        fs::write(config_dir.join("muted"), "").unwrap();
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

fn config_without_llm() -> String {
    r#"[llm]
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

fn config_with_queue(timeout: u64) -> String {
    format!(
        r#"[llm]
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

fn config_no_tts() -> String {
    r#"[llm]
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

fn config_with_specific_filter(types: &[&str]) -> String {
    let filter = types
        .iter()
        .map(|t| format!("\"{}\"", t))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        r#"[llm]
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

#[test]
fn test_version() {
    let env = TestEnv::new();
    env.cmd()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("sumvox"));
}

#[test]
fn test_help() {
    let env = TestEnv::new();
    env.cmd()
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

#[test]
fn test_init_does_not_overwrite_existing_toml() {
    let env = TestEnv::new();
    let home = env.setup_with_config("[[tts.providers]]\nname = \"macos\"\n");
    let config_path = home.join(".config/sumvox/config.toml");
    let before = fs::read_to_string(&config_path).unwrap();

    // Without --force: the existing config.toml must survive untouched
    env.cmd()
        .arg("init")
        .assert()
        .success()
        .stderr(predicate::str::contains("already exists"));

    assert_eq!(
        fs::read_to_string(&config_path).unwrap(),
        before,
        "init without --force must not overwrite an existing config.toml"
    );

    // With --force: overwriting is the point
    env.cmd().args(["init", "--force"]).assert().success();
    assert_ne!(
        fs::read_to_string(&config_path).unwrap(),
        before,
        "init --force should replace the existing config.toml"
    );
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
fn test_sum_empty_text() {
    let env = TestEnv::new();
    env.setup_with_config(&config_without_llm());

    env.cmd()
        .args(["sum", ""])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Empty text provided"));
}

#[test]
fn test_say_local() {
    let env = TestEnv::new();
    env.setup_with_config(&config_without_llm());
    env.mute();

    env.cmd()
        .args(["say", "hello", "--tts", LOCAL_TTS])
        .assert()
        .success();
}

#[test]
fn test_say_volume() {
    let env = TestEnv::new();
    env.setup_with_config(&config_without_llm());
    env.mute();

    env.cmd()
        .args(["say", "hello", "--tts", LOCAL_TTS, "--volume", "50"])
        .assert()
        .success();
}

#[test]
fn test_say_audio_no_config() {
    let env = TestEnv::new();
    env.setup_with_config(&config_without_llm());

    env.cmd()
        .args(["say", "hello", "--tts", "audio_file"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("audio_file provider not found"));
}

#[test]
fn test_notification_hook() {
    let env = TestEnv::new();
    env.setup_with_config(&config_without_llm());
    env.mute();

    env.cmd_debug()
        .arg("json")
        .write_stdin(notification_json("Test notification", "permission_prompt"))
        .assert()
        .success()
        .stdout(predicate::str::contains("Speaking notification"));
}

#[test]
fn test_stop_hook_active() {
    let env = TestEnv::new();
    env.setup_with_config(&config_without_llm());

    env.cmd_debug()
        .arg("json")
        .write_stdin(notification_json_stop_active())
        .assert()
        .success()
        .stdout(predicate::str::contains("preventing infinite loop"));
}

#[test]
fn test_queue_lock_acquired() {
    let env = TestEnv::new();
    env.setup_with_config(&config_without_llm());
    env.mute();

    env.cmd_debug()
        .arg("json")
        .write_stdin(notification_json("Queue test", "permission_prompt"))
        .assert()
        .success()
        .stdout(predicate::str::contains("Queue lock acquired"));
}

#[test]
fn test_queue_disabled() {
    let env = TestEnv::new();
    env.setup_with_config(&config_with_queue(0));
    env.mute();

    env.cmd_debug()
        .arg("json")
        .write_stdin(notification_json(
            "Queue disabled test",
            "permission_prompt",
        ))
        .assert()
        .success()
        .stdout(predicate::str::contains("queue disabled"));
}

#[test]
fn test_queue_concurrent() {
    use std::io::Write;
    use std::process::Stdio;

    let env = TestEnv::new();
    env.setup_with_config(&config_without_llm());
    env.mute();

    let bin = assert_cmd::cargo::cargo_bin!("sumvox");
    let spawn = |message: &str| {
        let mut child = std::process::Command::new(bin)
            .arg("json")
            .env("HOME", env.home_path())
            .env_remove("SUMVOX_DISABLE")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Failed to spawn child");
        child
            .stdin
            .take()
            .unwrap()
            .write_all(notification_json(message, "permission_prompt").as_bytes())
            .unwrap();
        child
    };

    let (a, b) = (spawn("Concurrent A"), spawn("Concurrent B"));
    for child in [a, b] {
        let output = child.wait_with_output().expect("Failed to wait for child");
        assert!(
            output.status.success(),
            "child failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
