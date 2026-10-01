# SumVox

Rust CLI and Claude Code hook that summarizes AI coding sessions with an LLM and speaks the result.
Docs are only a cache: the code, `sumvox --help` and `config/recommended.toml` are the source of truth.

## Commands

- `just check`: fmt check, clippy `-D warnings`, `cargo test` (hermetic: no network, no audio).
- `just test-e2e` (`cargo test --test e2e -- --ignored`): real APIs and audio; needs
  `config/e2e_test.toml` copied from `config/e2e_test.toml.example`.

## Gotchas

- `config/e2e_test.toml` is gitignored but holds real keys: never force-add it, and stage files
  by name rather than `git add -A` (a renamed copy is not ignored).
- No hard-coded provider, model or voice identities in runtime code. Config is the single source;
  CLI flags only select a configured entry.
- `llvm-cov --branch` segfaults on the `async_trait` provider files (`src/llm/`, `src/tts/`):
  exclude them from branch coverage.
- All playback goes through `src/audio/player.rs`; the summarize-then-speak flow lives in
  `src/pipeline.rs`, shared by `say`, `sum`, `json` and the hooks.
- `src/notify_log.rs` (muted flag, `history.log`, `now_playing`) is the contract with the menu bar
  app in `menubar/`; change both together.

## Pointers

Contributor workflow: CONTRIBUTING.md. Agent task state: `.agents/README.md`.
