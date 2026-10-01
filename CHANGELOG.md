# Changelog

All notable changes to SumVox will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed
- **BREAKING: the built-in defaults now match `sumvox init`.** The default TTS chain is the platform's local engine (`macos` on macOS, `espeak` elsewhere) then `google` (voice `Aoede`, was `Zephyr`), and the Notification hook speaks through that local engine by default. `sumvox init` writes exactly these defaults. A config with no `[tts]` table, or no `[hooks.claude_code]` table, picks them up.
- `--tts gemini` is now accepted as an alias for the `google` engine, matching the provider-name alias.
- A set-but-empty `GEMINI_API_KEY` no longer hides `GOOGLE_API_KEY`.

### Removed
- **BREAKING: legacy `config.yaml` / `config.yml` / `config.json` are no longer read or migrated.** If one of them exists without a `config.toml`, sumvox now fails with an error telling you to convert it to `config.toml` or run `sumvox init`. `sumvox init` no longer treats them as an existing config.
- The `TTS cost estimate` info log line is gone.

### Fixed
- Over-long text for ElevenLabs and xAI TTS no longer crashes sumvox when it contains Chinese or other multi-byte characters; it is cut at a character boundary.
- `--rate` is now optional: when omitted, the `rate` from your config (or the engine default) is used instead of being overwritten with 200.
- An unknown `--tts` value now fails with an error naming it, instead of silently running the whole fallback chain.
- `--provider` now keeps the matching config entry's `base_url` and `timeout`; `--model` only replaces the model.
- Concurrent playbacks no longer overwrite each other's temporary audio file.
- The Google Cloud TTS token request no longer uses system proxy detection, avoiding the macOS CoreFoundation crash.

## [1.9.0] - 2026-09-04

### Added
- **espeak TTS provider** (`espeak`, aliases `espeak_ng` / `espeak-ng`): local, offline, no API key — the Linux counterpart to `macos say`. Optional `voice` names an espeak-ng voice (e.g. `cmn+f3` for Mandarin with a female variant); `rate` is words per minute (default 175, clamped 80–450). Traditional and Simplified Chinese phonemize identically, so no conversion layer is needed. Requires the `espeak-ng` binary on `PATH`.
- **piper TTS provider** (`piper`, alias `piper_tts`): local, offline neural TTS with markedly better Chinese prosody than espeak. The voice model is a `.onnx` file resolved from `voice`, `model` or `path` (first non-blank wins, `~` expanded) and must sit beside its `.onnx.json` sidecar; `rate` maps onto piper's `--length-scale` (default 200, higher = faster). Requires the `piper` binary (`uv tool install piper-tts`) and a hand-downloaded voice model.

### Changed
- **Audio playback is no longer macOS-only.** The hardcoded `afplay` call became `src/audio/player.rs`, a single playback choke point for the whole crate. On macOS it still runs `afplay`; elsewhere it probes `paplay` → `pw-play` → `ffplay` → `mpv` → `aplay` and uses the first one installed. Every TTS provider — including the six cloud engines that were silent on Linux — now routes through it, so the 0–100 volume knob and the menu-bar avatar hook work on Linux too.
  - Volume is mapped through a cube root for `paplay` and `mpv`, whose scales are cubic in amplitude: `volume = 50` now measures −6 dB rather than −18 dB, matching macOS. `aplay` has no volume control at all and is skipped entirely when `volume` is 0, rather than playing at full blast.
  - `XDG_RUNTIME_DIR` defaults to `/run/user/{uid}` for the player process, so playback works from an SSH session (where the variable is often unset) instead of failing with `Connection refused`.
  - A wedged player is killed after 120 s and reported as a timeout, instead of blocking the hook forever.
- **menu bar app avatar**: replaced the PNG mouth-flap / text-face avatar with a native **Vector Orb** — a smooth, deformable blob drawn as a Catmull-Rom path filled with a radial gradient (original visual language, zero webview/dependencies). The avatar is now driven continuously by a 0..1 level — idle breathes with slow drifting lobes, speaking swells and wobbles the blob driven by the `now_playing` audio's RMS envelope, and the typewriter path synthesizes a smooth level when there is no real audio. Custom `~/.config/sumvox/avatar/{closed,open}.png` art is no longer read; the orb's emerald→cyan palette is built in.
  - The 1.8.0 toast — a HUD that appeared top-right and dismissed itself after 4 s — is now a persistent, draggable orb that sits wherever you leave it and opens the same menu on click. What appears per notification is the speech bubble beside it, which types the text out and hides after 4–12 s scaled to its length. The orb renders at 20 fps idle and 60 fps while speaking.

- **`sumvox say`, `sumvox sum`, and generic-JSON hook input now go through the same speech pipeline as the Claude Code hooks** (`src/pipeline.rs`). Consequences: they honour the menu bar mute flag (`~/.config/sumvox/muted`), append every spoken text to `~/.config/sumvox/history.log`, skip `audio_file` entries in the auto fallback chain, and strip a leading `[emotion]` tag for engines that would read it aloud, and apply `--volume` in the default `auto` engine mode (it was silently ignored there before). In the hook path, `stop_tts_provider` / `notification_tts_provider = "gemini_tts"` now binds to the `gemini_tts` config entry instead of the first `cloud_tts` one.

- **`sumvox init` and `--help` now name the format they actually use.** `init` has written `config.toml` since the TOML migration, but its output still told you it had created `config.yaml` and pointed at `config/recommended.yaml`. Both the `init` summary and the `--help` text for the command now say `config.toml`, as do the Homebrew caveats.
- **`--help` documents every TTS engine**: `--tts`, `--voice` and `--rate` list the engines they accept and what each knob means per engine (macOS words-per-minute, espeak's 80–450 range, piper's length-scale, ignored by the cloud engines).
- **espeak and piper report a configured status.** Both fell through to "not configured", because the check only knew how to look for an API key. `espeak` now always counts as configured (it needs no credentials and no assets); `piper` counts as configured once `voice`, `model` or `path` names a voice model.
- **Documentation**: README and QUICKSTART are TOML throughout (they still showed YAML examples for a format the tool no longer writes), and both were trimmed of duplicated and unverifiable material. `config/recommended.toml` documents the espeak, piper and Linux-playback blocks, and no longer claims macOS TTS cannot honour `volume` — it has since `say` began rendering to a file.

### Fixed
- **`sumvox init` no longer overwrites an existing `config.toml`.** The "already exists" guard only looked for the legacy `config.yaml` / `config.json`, so once a config had been migrated to TOML a plain `sumvox init` silently replaced it — API keys and all. The guard now covers all three paths; `--force` still overwrites.
- **Config values keep their decimal form.** TOML has only 64-bit floats, so the serializer cast every `f32` up and a saved config read `temperature = 0.30000001192092896`. `temperature` and the ElevenLabs `speed` / `stability` / `style` knobs now serialize as written (`0.3`), which also keeps a YAML → TOML migration from mangling them.

### Removed
- **`src/audio/afplay.rs`**: the macOS-only playback module and its `run_afplay` helper are gone, replaced by `src/audio/player.rs`. No configuration change is needed — macOS behaviour is unchanged.
- Dead configuration surface that was parsed but never read: the `version` config key, `sum --max-length`, `json --timeout`, and the `--format gemini-cli` value that was listed in `--help` but never implemented. Existing config files containing `version` still load; the key is ignored.
- **LLM cost tracking**: `src/llm/cost_tracker.rs` and `LlmProvider::estimate_cost` are gone. The tracker wrote a daily usage ledger and enforced a budget, but nothing ever called it, and the per-provider price constants feeding it had drifted to models the project no longer configures (Gemini Flash 2.0, GPT-4o-mini). TTS cost estimation is untouched — that one is live in `src/pipeline.rs`.
- **Unused dependencies**: `anyhow` and `is-terminal` (std has provided `IsTerminal` since Rust 1.70), plus the `libasound2-dev` installs in CI — nothing in the tree links ALSA.
- **`config/recommended.yaml`**: the legacy YAML example, superseded by `config/recommended.toml`. Existing YAML configs in `~/.config/sumvox/` are still migrated automatically.

### Internal
- **The e2e suite runs on Linux.** Every test hard-coded the `macos` engine, so it could only run on macOS and the Linux work had no end-to-end coverage. The harness now rewrites that literal to `espeak` on non-macOS hosts, so one `config/e2e_test.toml` serves both platforms, and CI runs the job on `ubuntu-latest` alongside `macos-latest`.
- **Every `#[allow(dead_code)]` in the tree is gone**, along with what each was masking: `GenerationResponse::model`, the response fields `AnthropicResponse::model` and `OllamaResponse::{model, done}` (parsed, then discarded), and `ClaudeCodeInput::permission_mode` (never consulted — the hook still accepts the field, it is just no longer stored). The `new()` constructor on the Gemini, OpenAI and Anthropic providers is gone too: it only forwarded to `with_base_url` with the default endpoint and only tests called it. The `*_API_BASE` constants it used are now `provider_factory`'s defaults, replacing the three URL literals that file repeated.
- **`Cargo.lock` is now tracked.** It had been ignored since the repository's `.gitignore` was adapted from a JavaScript template; for a binary that ships prebuilt through Homebrew and GitHub Releases, a committed lockfile is what makes a build from source reproducible and keeps CI failing only on new commits.

## [1.8.0] - 2026-07-04

### Added
- **OpenAI TTS provider**: New `openai` provider (alias `openai_tts`) using `gpt-4o-mini-tts` via `POST /v1/audio/speech`. Configure `voice` (e.g. `nova`), `speed` (0.25–4.0), and a `style_prompt` passed as `instructions` for tone/accent control. API key via `api_key` field or `OPENAI_API_KEY` env. Plays MP3 output through `afplay`; input is truncated on a character boundary at the 4096-character API limit (correct for multi-byte languages like Chinese).
- **macOS menu bar app** (`menubar/SumVoxMenu.swift`, built with `just menubar`): a lightweight `.accessory` status-bar companion to control voice and review notifications, with zero polling — disk is read only when the menu opens or a notification lands.
  - **Mute toggle**: flips a `~/.config/sumvox/muted` flag; the Stop hook skips TTS while muted but still records history.
  - **Notification history**: every agent voice report is logged to `~/.config/sumvox/history.log` (last 50 kept); the menu lists recent entries with click-to-copy.
  - **Floating toast**: new notifications pop a HUD panel in the top-right (event-driven `DispatchSource` watch), auto-dismissing after 4s and stacking when several arrive — shown even when muted, so nothing is missed.
  - **Enter/exit animation**: toasts slide in from the right and fade in (0.22s ease-out), slide out and fade on dismiss (0.18s ease-in); surviving toasts reflow smoothly.
- **Gemini-TTS voices via `cloud_tts`**: The `cloud_tts` provider (new `gemini_tts` alias) now supports Google's expressive Gemini-TTS models. Set `model` (e.g. `gemini-2.5-flash-tts`) to switch to bare voice names (`Kore`, `Charon`, ...) with an optional `style_prompt` for tone control. Uses the same synthesis endpoint; input is chunked at 4000 bytes and cost is estimated per audio token (token billing, no free tier, ~4x Standard).

### Fixed
- **`--tts gemini_tts` resolution**: the `gemini_tts` selector now resolves to its own config entry instead of colliding with `cloud_tts`.
- **Audio-tag stripping**: `[tag]`-style audio tags are stripped before non-`eleven_v3` providers, so engines that don't understand them no longer read the markup aloud.
- **TTS cost estimates**: updated xAI and ElevenLabs cost estimates to current pricing.

## [1.7.1] - 2026-06-18

### Fixed
- **ElevenLabs volume swings**: Output is now loudness-normalized through ffmpeg `loudnorm`, so consecutive utterances no longer jump in perceived volume. Hardened `loudnorm` stats parsing and temp-file handling along the way.
- **ffmpeg robustness**: Added a timeout on the `loudnorm` pass, null the ffmpeg stdin, replaced a busy-poll with a channel timeout, closed a temp-file race, and clean up partial temp files on failure — preventing hangs and leaked files during ElevenLabs playback.
- **afplay terminal interference**: `afplay` is now spawned with stdin/stdout/stderr redirected to null, so playback under Claude Code hooks no longer disturbs the terminal/PTY.
- **afplay temp-file leak**: A spawn failure no longer early-returns before cleanup; the temp file is removed on every path. Playback volume is also clamped to ≤100 to avoid amplifying past `-v 1.0`.

### Changed
- **Config is the single source of truth for provider defaults**: Removed hardcoded provider/model literals; CLI flags only select a configured provider, and all values come from config. Backward compatible with existing configs.

### Internal
- Extracted the triplicated `afplay` invocation into a shared `run_afplay(&Path, u32)` helper across the audio and ElevenLabs playback paths.
- Added Claude-based automated PR code review in CI.

## [1.7.0] - 2026-05-05

### Added
- **ElevenLabs TTS provider**: New `elevenlabs` provider with full voice tuning controls. Configure `voice` (Voice ID), `model` (`eleven_v3`, `eleven_multilingual_v2`, `eleven_turbo_v2_5`, `eleven_flash_v2_5`), `speed` (0.7–1.2), `stability` (0.0–1.0), and `style` (0.0–1.0). API key via `api_key` field or `ELEVENLABS_API_KEY` env. Plays MP3 output through `afplay` with full volume control. Supports library voices (paid) and Voice Design (all tiers).
- **xAI Grok LLM provider**: First-class `xai` / `grok` entry in the LLM provider factory, routed through the OpenAI-compatible endpoint at `https://api.x.ai/v1`. API key via `api_key` field or `XAI_API_KEY` env. Works with `grok-4-1-fast-non-reasoning`, `grok-4-1-fast-reasoning`, etc.
- **`recommended.toml` examples**: Commented-out blocks for both new providers with pricing notes and API key sourcing instructions.

## [1.6.0] - 2026-04-28

### Added
- **Per-provider `disable_thinking` override**: Each entry in `[[llm.providers]]` now accepts an optional `disable_thinking` field. When set, it overrides the global `[llm.parameters].disable_thinking` for that provider only. Lets you keep thinking enabled for capable models (e.g. Gemini Pro) while disabling it on faster/cheaper fallbacks without juggling multiple config files.
- **Ollama `think` parameter**: Ollama provider now sends `"think": false` at the request top-level when thinking is disabled, suppressing reasoning output on models that support it (DeepSeek-R1, Qwen3, etc.). Models without `think` support silently ignore the field — no model-name heuristics required.

### Changed
- **Gemini thinking control simplified**: Replaced model-name heuristics (`gemini-3*` vs `gemini-2.5*pro-exp`) with a uniform `thinkingConfig.thinkingBudget = 0` when disabling thinking. Works across the entire 2.5 / 3.x family that supports the field; models that don't support it ignore it.
- **OpenAI reasoning effort heuristic-free**: `reasoning_effort` is now sent as `"low"` whenever `disable_thinking = true`, regardless of model name. The `is_reasoning_model` heuristic is retained only for `max_completion_tokens` / `temperature` dispatch (which still need it for o1/o3/o4 quirks).

### Removed
- **Anthropic request-side `thinking` field**: Removed the always-`enabled` `thinking: {type, budget_tokens}` block that broke Claude Opus 4.7 with HTTP 400. Anthropic providers now omit the field entirely; thinking is governed by Anthropic's per-model defaults. Response-side parsing of thinking content is unchanged.

### Fixed
- **e2e test suite under Claude Code**: Tests no longer inherit `SUMVOX_DISABLE=1` from the developer shell. Previously 20 of 25 e2e tests silently passed `assert().success()` because the binary returned `Ok(())` immediately at startup, then failed any stdout/stderr assertion.

## [1.5.1] - 2026-04-23

### Fixed
- **Concurrent Stop hook voice overlap**: Multiple Claude Code instances triggering Stop hooks simultaneously no longer cause overlapping TTS playback. Root cause: local TTS providers (`macos say`, `audio_file`) returned from `speak()` before the underlying subprocess finished, causing `QueueLock` (flock) to release prematurely.

### Changed
- **TTS flow unified to always-blocking**: All TTS providers now block until playback completes. Removed `is_async` / `async_mode` parameters from `create_tts_from_config`, `create_single_tts`, `create_tts_by_name`, `MacOsTtsProvider::new`, `AudioFileProvider::new`, and the CLI `speak_with_provider_fallback`.

### Removed
- **Fire-and-forget TTS branches**: `MacOsTtsProvider::speak` no longer spawns a detached `tokio::process` task; `AudioFileProvider::speak` no longer spawns a detached `std::thread`. Both paths now call the blocking implementation directly.
- **Stale rodio comment**: Removed the obsolete `OutputStream !Send / tokio runtime hang` comment in `audio/file.rs` left over from the rodio → afplay migration (v1.4.1).

## [1.5.0] - 2026-04-20

### Added
- **Stop hook content source option**: New `summarization.content_source` config field with two variants:
  - `"transcript"` (default) — parse last N turns from JSONL transcript file (existing behavior).
  - `"last_message"` — use Claude Code's `last_assistant_message` hook field directly, skipping transcript I/O and the 50ms/100ms filesystem-sync retries.
- **`ClaudeCodeInput.last_assistant_message`**: Deserializes the new Claude Code Stop hook field. Backward-compatible (missing → `None`).
- **Graceful fallback**: When `content_source = "last_message"` is set but the field is absent or empty/whitespace, falls back to the transcript path with a warning log.

### Changed
- **`handle_stop` branching**: Refactored to select content source via a pure `select_stop_context_source` helper (unit-testable). LLM summarization always runs regardless of source.
- **Documentation**: README, QUICKSTART, and `recommended.toml` document the new option and clarify that `turns` only applies to the transcript source.

## [1.4.1] - 2026-03-24

### Changed
- **Audio playback**: Replace `rodio` with macOS native `afplay` for all TTS providers
  - Peak memory footprint reduced by 56% (6.6MB → 2.9MB)
  - CPU instructions reduced by 73% (247M → 66M)
  - Eliminates symphonia codec initialization and CPAL audio device binding
- **xAI TTS**: Switch output format from MP3 to WAV for zero-decode-overhead playback
- **Shared audio utilities**: Extract common `afplay` and WAV header logic into `audio::afplay` and `audio::wav_header` modules

### Removed
- **rodio dependency**: Fully removed — all audio playback now uses system `afplay`

## [1.4.0] - 2026-03-19

### Added
- **xAI TTS Provider**: New `xai` TTS provider using the xAI Text-to-Speech API
  - 5 natural voices: `eve` (default), `ara`, `rex`, `sal`, `leo`
  - Automatic language detection or explicit language setting via `language_code`
  - MP3 audio output decoded via rodio
  - Volume control (0-100)
  - 15,000 character per-request limit with automatic truncation
  - Cost estimation at $4.20/1M characters (Beta pricing)
  - API key from config or `XAI_API_KEY` environment variable
- **Config**: `get_xai_api_key()` for xAI API key resolution
- **Recommended Config**: Added xAI TTS provider example with voice options and pricing

### Changed
- **Documentation**: Updated README and recommended config to reflect all 4 TTS providers (macOS, xAI, Google TTS, Google Cloud TTS)

## [1.3.1] - 2026-03-16

### Added
- **Disable via Environment Variable**: Set `SUMVOX_DISABLE=1` to temporarily skip all SumVox processing, useful for quiet Claude Code sessions

## [1.3.0] - 2026-03-13

### Added
- **Google Cloud TTS Provider**: New `cloud_tts` TTS provider using Google Cloud Text-to-Speech API
  - OAuth2 authentication via service account JSON key with automatic token caching
  - Support for Standard, WaveNet, and Chirp3-HD voices
  - Multi-language support with `language_code` config (e.g., `cmn-TW`, `cmn-CN`, `en-US`)
  - Volume control via rodio (0-100)
  - Automatic text chunking for messages exceeding 5,000 byte API limit
  - Cost estimation at $4/1M characters (Standard voices)
  - Integrates into existing TTS fallback chain
- **WAV Audio Codec Support**: Added WAV/PCM decoding via `rodio` wav feature
- **Config Fields**: `service_account_key` and `language_code` options for TTS provider configuration

### Changed
- **Recommended Config**: Added commented Cloud TTS provider example with setup instructions and pricing info

## [1.2.2] - 2026-03-04

### Fixed
- **Auto Mode Volume Override**: Fixed `stop_volume` and `notification_volume` hook settings being ignored when TTS provider is set to `"auto"`. The volume override is now correctly propagated through `speak_with_provider_fallback`, preventing unexpectedly loud Gemini TTS playback.

### Changed
- **Recommended Config**: Updated Google TTS volume guidance to suggest 40-60 range (Gemini TTS output is loud by default).

## [1.2.1] - 2026-03-03

### Fixed
- **Audio File Volume Control**: Fixed `notification_volume` setting being ignored for `audio_file` TTS provider, causing playback at maximum volume (100) regardless of config. The hook volume override is now correctly applied, consistent with macOS and Google TTS providers.

## [1.2.0] - 2026-03-01

### Added
- **Audio File Playback Provider**: Play `.wav`, `.mp3`, `.flac`, `.ogg` sound effects via `audio_file` TTS provider
  - Single file or directory mode (random selection from directory)
  - Configurable volume control (0-100)
  - Non-blocking async playback for hooks
- **Cross-Process Notification Queue**: File-lock based queue (`flock`) prevents overlapping TTS output across concurrent hook invocations
- **Vorbis Codec Support**: Added OGG/Vorbis decoding via `rodio` with vorbis feature
- **E2E Test Infrastructure**: 25 end-to-end tests covering CLI commands, hook dispatch, audio playback, and concurrent queue behavior
- **Separate E2E CI Job**: E2E tests run independently with secret-based config, not blocking unit test pipeline

### Fixed
- **Silent Hook Audio Playback**: Fixed bugs where hook audio playback produced no sound
- **Async Audio Process Hang**: Fixed process hang in async audio playback by properly managing tokio runtime and thread lifecycle

## [1.1.1] - 2026-02-15

### Fixed
- **Transcript Turn Detection**: Fixed turn boundary logic that treated `tool_result` entries as new turns. In Claude Code transcripts, both human input and tool results share `type: "user"`, causing the last "turn" to often contain only `thinking`/`tool_use` blocks with no text — resulting in empty summaries on every Stop hook. Now only human-authored messages (with text content) are used as turn boundaries.

## [1.1.0] - 2026-02-10

### Added
- **TOML Configuration Format**: New TOML format support with automatic migration from YAML/JSON
  - Auto-migration creates timestamped backup of legacy config files
  - Priority: `config.toml` > `config.yaml` > `config.json`
  - Recommended config updated to TOML format (`config/recommended.toml`)
- **Separate Volume Control**: Independent volume settings for notifications and summaries
  - `notification_volume` (default: 80) - quieter for non-intrusive alerts
  - `stop_volume` (default: 100) - full volume for task completion summaries
  - Volume priority: CLI override > Hook config > Provider config > Defaults
  - **⚠️ Important**: Volume control only works with Google TTS; macOS TTS does not support volume control (uses system volume)

### Changed
- **Configuration Format**: TOML is now the preferred format (YAML/JSON still supported for backward compatibility)
- **Default Volumes**: Notification volume reduced from 100 to 80 for better user experience
- **Documentation**: Updated all config references to TOML format

### Fixed
- Documentation references to non-existent `credentials.rs` file in CONTRIBUTING.md
- Justfile `show-config` command using outdated config path
- Justfile invalid `credentials` command removed

### Removed
- GeminiCli hook format (unimplemented feature removed from codebase)

### Migration Guide
When upgrading to v1.1.0:
1. Your existing `config.yaml` or `config.json` will be automatically migrated to `config.toml`
2. A timestamped backup will be created (e.g., `config.yaml.backup-20260210-120000`)
3. To customize volumes, add to your `config.toml`:
   ```toml
   [hooks.claude_code]
   notification_volume = 80  # 0-100, default: 80
   stop_volume = 100         # 0-100, default: 100
   ```
4. **Volume Control Notes**:
   - Volume settings only work with **Google TTS**
   - **macOS TTS** does not support volume control - use system volume settings instead
   - To use volume control, set `notification_tts_provider = "google"` or `stop_tts_provider = "google"`

## [1.0.0] - 2026-02-05

### 🎉 Initial Release

**SumVox** - Intelligent voice notifications for AI coding tools

### Added

#### Core Features
- ⚡ **Blazing Fast**: 7ms startup time (Rust implementation)
- 🧠 **Multi-LLM Support**:
  - Google Gemini (gemini-2.5-flash)
  - Anthropic Claude (claude-haiku-4-5-20251001)
  - OpenAI GPT (gpt-5-nano)
  - Ollama (llama3.2, local)
  - All providers support custom API endpoints (base_url)
- 🔊 **Multi-TTS Engines**:
  - Google TTS (high quality, cloud-based)
  - macOS say (local, always available)
- ✅ **Production Ready**: 113 automated tests
- 🔄 **Array-Based Fallback**: Automatic provider switching on failure
- 🪝 **Claude Code Integration**: Seamless hook support with separate TTS configuration

#### Configuration
- **Format**: YAML (preferred) or JSON (backward compatible)
- **Location**: `~/.config/sumvox/config.yaml`
- **Default Config**: Includes all 4 LLM providers ready to use
- **Simple Setup**: Edit config file directly, no environment variables needed
- **Custom API Endpoints**: All providers support base_url for proxies/compatible APIs
- **Hook-Specific TTS**: Separate TTS provider for Notification and Stop hooks
- **Notification Filters**: Choose which notification types to speak
- **Thinking Control**: Support for Gemini 3, Claude extended thinking, OpenAI reasoning

#### Pipeline
1. Reads Claude Code session transcripts (JSONL format)
2. Generates concise summaries using LLM
3. Converts summaries to speech with TTS
4. Automatic provider fallback on errors

#### CLI Commands
- `sumvox init` - Initialize configuration with 4-provider template
- `sumvox say <text>` - Direct text-to-speech
- `sumvox sum <text>` - Summarize and speak
- CLI overrides: `--provider`, `--model`, `--tts`, `--tts-voice`

### Documentation
- Complete README with setup guide and fallback explanation
- Quick Start Guide (QUICKSTART.md) - 5-minute setup
- MIT License
- Contributing guidelines (CONTRIBUTING.md)
- GitHub Issue/PR templates
- Recommended configuration (config/recommended.yaml) with detailed comments
- Homebrew formula
- crates.io support

### Technical Details
- **Language**: Rust 2021 edition
- **Dependencies**: tokio, reqwest, serde, clap, rodio
- **Platforms**: macOS (x86_64, aarch64), Linux (x86_64, aarch64)
- **Minimum macOS**: 10.15
- **Build Optimizations**: LTO, size optimization, panic=abort

### Why Gemini?
- 🚀 **Performance**: 1-2s response time
- 💰 **Cost-effective**: Low pricing for high-frequency use
- 🎯 **Quality**: Accurate and fluent summaries
- 🔊 **Integrated**: One API key for both LLM and TTS
- ✅ **Tested**: Complete test coverage and optimization

### Migration from claude-voice
- New name: SumVox (Summarization + Voice)
- New config location: `~/.config/sumvox/config.yaml` (YAML format)
- Binary renamed: `claude-voice` → `sumvox`
- Homebrew tap: `musingfox/sumvox`
- Configuration: Edit YAML file directly instead of using environment variables

[Unreleased]: https://github.com/musingfox/sumvox/compare/v1.9.0...HEAD
[1.9.0]: https://github.com/musingfox/sumvox/releases/tag/v1.9.0
[1.8.0]: https://github.com/musingfox/sumvox/releases/tag/v1.8.0
[1.7.1]: https://github.com/musingfox/sumvox/releases/tag/v1.7.1
[1.7.0]: https://github.com/musingfox/sumvox/releases/tag/v1.7.0
[1.6.0]: https://github.com/musingfox/sumvox/releases/tag/v1.6.0
[1.5.1]: https://github.com/musingfox/sumvox/releases/tag/v1.5.1
[1.5.0]: https://github.com/musingfox/sumvox/releases/tag/v1.5.0
[1.4.1]: https://github.com/musingfox/sumvox/releases/tag/v1.4.1
[1.4.0]: https://github.com/musingfox/sumvox/releases/tag/v1.4.0
[1.3.1]: https://github.com/musingfox/sumvox/releases/tag/v1.3.1
[1.3.0]: https://github.com/musingfox/sumvox/releases/tag/v1.3.0
[1.2.2]: https://github.com/musingfox/sumvox/releases/tag/v1.2.2
[1.2.1]: https://github.com/musingfox/sumvox/releases/tag/v1.2.1
[1.2.0]: https://github.com/musingfox/sumvox/releases/tag/v1.2.0
[1.1.1]: https://github.com/musingfox/sumvox/releases/tag/v1.1.1
[1.1.0]: https://github.com/musingfox/sumvox/releases/tag/v1.1.0
[1.0.0]: https://github.com/musingfox/sumvox/releases/tag/v1.0.0
