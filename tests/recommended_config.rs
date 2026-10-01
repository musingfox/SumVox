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
