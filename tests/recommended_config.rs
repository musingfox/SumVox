// Hermetic checks on the shipped example config, config/recommended.toml.
// No audio device, no engine binary, no network: the file is embedded at
// compile time and only parsed / string-matched.
//
// Run: cargo test --test recommended_config

const RECOMMENDED: &str = include_str!("../config/recommended.toml");

/// The shipped example stays valid TOML after documentation edits.
#[test]
fn recommended_config_parses_as_toml() {
    let parsed = toml::from_str::<toml::Value>(RECOMMENDED);
    assert!(
        parsed.is_ok(),
        "config/recommended.toml must stay parseable: {:?}",
        parsed.err()
    );
}

/// A Linux user can find out how to enable espeak-ng and piper, including
/// where a piper voice model comes from and how playback happens.
#[test]
fn recommended_config_documents_linux_local_tts() {
    for needle in [
        "name = \"espeak\"",
        "name = \"piper\"",
        ".onnx",
        "uv tool install piper-tts",
        "paplay",
    ] {
        assert!(
            RECOMMENDED.contains(needle),
            "config/recommended.toml should document {needle:?}"
        );
    }
}

/// The new blocks were added, not written over the existing macOS block.
#[test]
fn recommended_config_keeps_macos_block_intact() {
    assert!(
        RECOMMENDED.contains("rate = 200"),
        "the macOS block's 'rate = 200' line must survive"
    );

    let parsed: toml::Value =
        toml::from_str(RECOMMENDED).expect("config/recommended.toml must stay parseable");
    let providers = parsed["tts"]["providers"]
        .as_array()
        .expect("[[tts.providers]] must be an array");
    let macos = providers
        .iter()
        .find(|p| p.get("name").and_then(toml::Value::as_str) == Some("macos"))
        .expect("the active macOS TTS provider block must still be present");

    assert_eq!(
        macos.get("rate").and_then(toml::Value::as_integer),
        Some(200),
        "the macOS provider must still declare rate = 200"
    );
}
