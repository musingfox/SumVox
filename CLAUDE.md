# Project Configuration

## Overview

**Project**: SumVox
**Language**: Rust
**Type**: CLI Tool / Claude Code Hook
**Purpose**: Intelligent voice notifications for AI coding tools with multi-model LLM support

## Task Management System

**System**: Local
**Location**: `.agents/tasks/`

## Development Environment

### Build & Run

```bash
# Development build
cargo build

# Release build (optimized)
cargo build --release

# Run with debug logging
RUST_LOG=debug cargo run

# Run tests
cargo test

# Run with specific input
echo '{"session_id":"test",...}' | cargo run
```

### Configuration

- **Main config**: `~/.config/sumvox/config.toml`
- **Hook script**: `.claude/hooks/run_sumvox_hook.sh`
- **Binary location**: `target/release/sumvox`
- **Example config**: `config/recommended.toml`

### Environment Variables

```bash
# Logging (optional)
export RUST_LOG=info  # or debug, trace
```

**Note:** API keys are configured in `~/.config/sumvox/config.toml`, not as environment variables.

## Testing

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run specific test
cargo test test_name

# Run tests in specific module
cargo test llm::
cargo test tts::
```

## CLI Commands

```bash
# Initialize config
sumvox init

# Direct TTS
sumvox say "Hello world"

# Summarize text
sumvox sum "Long text to summarize..."

# Test with specific providers
sumvox say "Test" --tts macos --voice Daniel
```

## Architecture

### Key Modules

- `src/main.rs` - Entry point, hook orchestration
- `src/config.rs` - Configuration loading/saving
- `src/transcript.rs` - Claude Code transcript parsing
- `src/llm/` - Multi-provider LLM support (Gemini, Anthropic, OpenAI, xAI Grok, Ollama)
- `src/tts/` - Text-to-Speech engines (macOS say, espeak-ng, piper, Google TTS, Google Cloud TTS, xAI TTS, OpenAI TTS, ElevenLabs)
- `src/audio/` - Playback: `player.rs` is the single choke point (afplay on macOS, probed CLI player elsewhere)
- `src/pipeline.rs` - Shared summarize → speak flow used by `say`/`sum`/`json` and the hook handlers
- `src/provider_factory.rs` - Provider creation with fallback chain
- `src/notify_log.rs` - Mute flag, history log and now_playing file shared with the menu bar app

### Configuration Format

See `~/.config/sumvox/config.toml`:

- LLM providers array with fallback chain
- TTS providers array with fallback chain
- Summarization settings (turns, prompt_template)
- Hook-specific configurations (notification_filter, tts_provider overrides)

## Project Structure

```
sumvox/
├── src/
│   ├── main.rs           # Entry point
│   ├── cli.rs            # CLI parsing
│   ├── config.rs         # Configuration loading/saving
│   ├── transcript.rs     # Transcript parsing
│   ├── error.rs          # Error types
│   ├── queue.rs          # Serializes concurrent hook invocations
│   ├── notify_log.rs     # History log + mute flag for the menu bar app
│   ├── hooks/            # Hook handlers
│   ├── pipeline.rs       # Shared summarize → speak flow
│   ├── llm/              # LLM providers
│   ├── tts/              # TTS engines
│   ├── audio/            # Playback (player.rs, file.rs, normalize.rs, wav_header.rs)
│   └── provider_factory.rs
├── menubar/
│   └── SumVoxMenu.swift  # Optional macOS status bar app (just menubar)
├── config/
│   └── recommended.toml  # Example configuration with comments
├── .github/
│   ├── workflows/        # CI/CD
│   └── ISSUE_TEMPLATE/   # Issue templates
├── homebrew/
│   └── sumvox.rb         # Homebrew formula
├── Cargo.toml
├── README.md
├── QUICKSTART.md
├── CHANGELOG.md
├── CONTRIBUTING.md
└── LICENSE
```

## Release Process

See the "Release Process" section in [CONTRIBUTING.md](CONTRIBUTING.md) for detailed release steps.

Quick version:

```bash
# Using justfile
just release 1.0.0

# Or manually
git tag -a v1.0.0 -m "Release v1.0.0"
git push origin v1.0.0
```

## Recommended Configuration

See `config/recommended.toml` for the Gemini-based setup:

- Google Gemini for LLM (tested and optimized)
- macOS TTS for notifications (fast and free)
- Google TTS for summaries (high quality)
- TOML format with inline comments for easy customization
