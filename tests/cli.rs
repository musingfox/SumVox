// Hermetic CLI tests for the sumvox binary: no network, no audio, no real config.
// Each test runs with HOME pointed at a temp dir so ~/.config/sumvox is never touched.

use assert_cmd::cargo::cargo_bin_cmd;
use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
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
fn test_init_replaces_legacy_config() {
    let env = TestEnv::new();

    // A legacy config.yaml no longer counts as an existing config: init writes the TOML
    let config_dir = env.home_path().join(".config/sumvox");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(config_dir.join("config.yaml"), "version: '1.0.0'").unwrap();

    env.cmd().arg("init").assert().success();

    let config_path = config_dir.join("config.toml");
    assert!(
        config_path.exists(),
        "config.toml should be created by init next to a legacy config.yaml"
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
fn test_legacy_config_without_toml_is_rejected() {
    for legacy in ["config.yaml", "config.yml", "config.json"] {
        let env = TestEnv::new();
        let config_dir = env.home_path().join(".config/sumvox");
        fs::create_dir_all(&config_dir).unwrap();
        fs::write(config_dir.join(legacy), "{}").unwrap();

        env.cmd()
            .args(["say", "hi"])
            .assert()
            .failure()
            .stderr(predicate::str::contains(legacy))
            .stderr(predicate::str::contains("sumvox init"));
    }
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
    env.setup_with_config(&config_without_llm());
    env.mute();

    env.cmd()
        .args(["say", "hello", "--tts", "nonsense"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Unknown TTS engine 'nonsense'"));
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
fn test_notification_hook() {
    let env = TestEnv::new();
    env.setup_with_config(&config_without_llm());
    env.mute();

    env.cmd_debug()
        .arg("json")
        .write_stdin(notification_json("Test notification", "permission_prompt"))
        .assert()
        .success()
        .stdout(predicate::str::contains("Queue lock acquired"))
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

/// Std-only fake Ollama: answers every POST with `reply` as the generated text and
/// records each request body. Returns (base_url, recorded bodies).
fn fake_ollama(reply: &'static str) -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    let bodies = Arc::new(Mutex::new(Vec::new()));
    let recorded = bodies.clone();
    thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let mut buf = Vec::new();
            let mut chunk = [0u8; 4096];
            let body_start = loop {
                let n = stream.read(&mut chunk).unwrap_or(0);
                if n == 0 {
                    break None;
                }
                buf.extend_from_slice(&chunk[..n]);
                if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                    break Some(pos + 4);
                }
            };
            let Some(body_start) = body_start else {
                continue;
            };
            let headers = String::from_utf8_lossy(&buf[..body_start]).to_lowercase();
            let content_length: usize = headers
                .lines()
                .find_map(|l| l.strip_prefix("content-length:"))
                .and_then(|v| v.trim().parse().ok())
                .unwrap_or(0);
            while buf.len() < body_start + content_length {
                let n = stream.read(&mut chunk).unwrap_or(0);
                if n == 0 {
                    break;
                }
                buf.extend_from_slice(&chunk[..n]);
            }
            recorded
                .lock()
                .unwrap()
                .push(String::from_utf8_lossy(&buf[body_start..]).into_owned());

            let payload = serde_json::json!({
                "response": reply,
                "prompt_eval_count": 1,
                "eval_count": 1
            })
            .to_string();
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                payload.len(),
                payload
            );
        }
    });
    (base_url, bodies)
}

/// A base_url nothing listens on, so the LLM call fails fast with connection refused.
fn dead_ollama_url() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    format!("http://{}", listener.local_addr().unwrap())
}

fn config_with_ollama(base_url: &str, content_source: &str) -> String {
    format!(
        r#"[llm]
[[llm.providers]]
name = "ollama"
model = "fake-model"
base_url = "{base_url}"
timeout = 5

[llm.parameters]
max_tokens = 100
temperature = 0.3

[tts]
[[tts.providers]]
name = "macos"
rate = 200

[summarization]
turns = 1
content_source = "{content_source}"
system_message = "Test"
prompt_template = "Summarize: {{context}}"
fallback_message = "Test completed"

[hooks.claude_code]
notification_filter = ["*"]
queue_timeout = 0
"#
    )
}

fn stop_json(transcript_path: &Path, last_assistant_message: Option<&str>) -> String {
    let mut json = serde_json::json!({
        "session_id": "e2e-test",
        "transcript_path": transcript_path,
        "hook_event_name": "Stop",
    });
    if let Some(msg) = last_assistant_message {
        json["last_assistant_message"] = msg.into();
    }
    json.to_string()
}

fn write_transcript(dir: &Path, assistant_text: &str) -> std::path::PathBuf {
    let path = dir.join("transcript.jsonl");
    let lines = [
        serde_json::json!({"type":"user","message":{"role":"user","content":"please do it"}}),
        serde_json::json!({"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":assistant_text}]}}),
    ];
    let body: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
    fs::write(&path, body.join("\n")).unwrap();
    path
}

fn history(env: &TestEnv) -> String {
    fs::read_to_string(env.home_path().join(".config/sumvox/history.log")).unwrap_or_default()
}

#[test]
fn test_stop_hook_summarizes_transcript_and_speaks_summary() {
    let env = TestEnv::new();
    let (url, bodies) = fake_ollama("CANNED SUMMARY");
    env.setup_with_config(&config_with_ollama(&url, "transcript"));
    env.mute();
    let transcript = write_transcript(env.home_path(), "I refactored the parser");

    env.cmd()
        .arg("json")
        .write_stdin(stop_json(&transcript, None))
        .assert()
        .success();

    let bodies = bodies.lock().unwrap();
    assert_eq!(bodies.len(), 1, "LLM should be called once");
    assert!(
        bodies[0].contains("I refactored the parser"),
        "transcript text must reach the LLM prompt: {}",
        bodies[0]
    );
    assert!(
        history(&env).contains("CANNED SUMMARY"),
        "summary must reach the TTS step"
    );
}

#[test]
fn test_stop_hook_last_message_source_skips_transcript() {
    let env = TestEnv::new();
    let (url, bodies) = fake_ollama("CANNED SUMMARY");
    env.setup_with_config(&config_with_ollama(&url, "last_message"));
    env.mute();
    let missing = env.home_path().join("no-such-transcript.jsonl");

    env.cmd()
        .arg("json")
        .write_stdin(stop_json(&missing, Some("message from hook input")))
        .assert()
        .success();

    let bodies = bodies.lock().unwrap();
    assert_eq!(bodies.len(), 1, "LLM should be called once");
    assert!(bodies[0].contains("message from hook input"));
    assert!(history(&env).contains("CANNED SUMMARY"));
}

#[test]
fn test_stop_hook_llm_failure_speaks_fallback_message() {
    let env = TestEnv::new();
    env.setup_with_config(&config_with_ollama(&dead_ollama_url(), "last_message"));
    env.mute();
    let missing = env.home_path().join("no-such-transcript.jsonl");

    env.cmd()
        .arg("json")
        .write_stdin(stop_json(&missing, Some("anything")))
        .assert()
        .success();

    assert!(history(&env).contains("Test completed"));
}

#[test]
fn test_stop_hook_unreadable_transcript_fails() {
    let env = TestEnv::new();
    env.setup_with_config(&config_with_ollama(&dead_ollama_url(), "transcript"));
    env.mute();
    let missing = env.home_path().join("no-such-transcript.jsonl");

    env.cmd()
        .arg("json")
        .write_stdin(stop_json(&missing, None))
        .assert()
        .failure()
        .stderr(predicate::str::contains("Failed to open transcript"));
    assert_eq!(history(&env), "", "nothing should be spoken");
}

#[test]
fn test_sumvox_disable_short_circuits() {
    let env = TestEnv::new();
    env.setup_with_config(&config_without_llm());
    env.mute();

    env.cmd()
        .env("SUMVOX_DISABLE", "1")
        .args(["say", "should not be recorded"])
        .assert()
        .success();

    assert_eq!(
        history(&env),
        "",
        "disabled run must not reach the pipeline"
    );
}

#[test]
fn test_muted_say_records_history_without_playing() {
    let env = TestEnv::new();
    let sound = env.home_path().join("sound.wav");
    fs::write(&sound, b"not real audio").unwrap();
    env.setup_with_config(&config_with_audio_file(sound.to_str().unwrap()));
    env.mute();

    env.cmd()
        .args(["say", "quiet please", "--tts", "audio_file"])
        .assert()
        .success();

    assert!(history(&env).contains("quiet please"));
    assert!(
        !env.home_path().join(".config/sumvox/now_playing").exists(),
        "muted run must not start playback"
    );
}

#[test]
fn test_bare_invocation_autodetects_generic_json_on_stdin() {
    let env = TestEnv::new();
    let (url, bodies) = fake_ollama("GENERIC SUMMARY");
    env.setup_with_config(&config_with_ollama(&url, "transcript"));
    env.mute();

    env.cmd()
        .write_stdin(r#"{"text":"some tool output"}"#)
        .assert()
        .success()
        .stdout(predicate::str::contains("GENERIC SUMMARY"));

    let bodies = bodies.lock().unwrap();
    assert_eq!(bodies.len(), 1, "LLM should be called once");
    assert!(bodies[0].contains("some tool output"));
    assert!(history(&env).contains("GENERIC SUMMARY"));
}
