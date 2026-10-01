# Contributing to SumVox

## Setup

You need a stable Rust toolchain (edition 2021) and [`just`](https://github.com/casey/just)
(`cargo install just`). `just` lists the available recipes.

```bash
just check   # cargo fmt --check, cargo clippy -- -D warnings, cargo test
```

CI runs the same three checks. Commit subjects follow Conventional Commits
(`feat`, `fix`, `docs`, `refactor`, `test`, `chore`, ...), and user-visible changes get a line
under `[Unreleased]` in [CHANGELOG.md](CHANGELOG.md).

## Tests

- `cargo test` is hermetic: no network, no audio, no API keys. Tests that need real audio are
  `#[ignore]`d.
- `just test-e2e` (`cargo test --test e2e -- --ignored`) drives the built binary against real
  APIs and plays audio. It reads `config/e2e_test.toml`:

  ```bash
  cp config/e2e_test.toml.example config/e2e_test.toml   # then fill in API keys
  ```

  `config/e2e_test.toml` is gitignored; still stage files by name rather than `git add -A`,
  since a renamed copy of it is not ignored. On Linux the harness rewrites the `macos` engine to
  `espeak`, so `espeak-ng` must be installed. `SUMVOX_DISABLE` must be unset, or the binary exits
  before doing anything.

## Adding a TTS provider

1. Implement `TtsProvider` in `src/tts/<engine>.rs`. Report `is_available()` honestly: the
   fallback chain skips an unavailable provider.
2. In `src/tts/mod.rs`: add `pub mod` and `pub use` lines, a `TtsEngine` variant, its spellings in
   `TtsEngine::NAMES`, and an arm in `create_single_tts`.
3. If it takes an API key, add a `get_*_api_key` method to `TtsProviderConfig` in `src/config.rs`
   (it sits on the shared `resolve_key`) and any new config field.
4. Add the engine to the `--tts` help text in `src/cli.rs`.
5. Document an example block in `config/recommended.toml`. Required fields such as `voice` or
   `model` come from the config and are never hard-coded.

## Adding an LLM provider

1. Implement `LlmProvider` in `src/llm/<provider>.rs`.
2. In `src/llm/mod.rs`: add `pub mod` and `pub use` lines.
3. In `src/provider_factory.rs`: add a `Provider` variant, its names in `FromStr`, and an arm in
   `create_single`.
4. For key-based providers, add the name to `LlmProviderConfig::env_var_name` in `src/config.rs`.
5. Add the name to the `--provider` help text in `src/cli.rs`.
6. Document an example block in `config/recommended.toml`.

## Releasing

```bash
just release X.Y.Z
```

Update `CHANGELOG.md` and commit it first: the recipe refuses to run on a dirty tree. It then
sets the version in `Cargo.toml` and `homebrew/sumvox.rb`, runs `cargo test`, commits
`chore: bump version to X.Y.Z` and creates the tag `vX.Y.Z`. It pushes nothing; push the two refs
yourself:

```bash
git push origin main
git push origin vX.Y.Z
```

The tag triggers `.github/workflows/release.yml`, which builds the four binaries, publishes the
GitHub release, commits the new SHA-256 hashes to `homebrew/sumvox.rb` on `main` and updates the
`musingfox/homebrew-sumvox` tap.
