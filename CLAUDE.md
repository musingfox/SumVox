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

- **Main config**: `~/.config/sumvox/config.yaml`
- **Hook script**: `.claude/hooks/run_sumvox_hook.sh`
- **Binary location**: `target/release/sumvox`
- **Example config**: `config/recommended.yaml`

### Environment Variables

```bash
# LLM API Keys
export GEMINI_API_KEY="AIza..."
export ANTHROPIC_API_KEY="sk-ant-..."
export OPENAI_API_KEY="sk-..."

# Logging
export RUST_LOG=info  # or debug, trace
```

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
- `src/llm/` - Multi-provider LLM support (Gemini, Anthropic, OpenAI, Ollama)
- `src/tts/` - Text-to-Speech engines (Google TTS, macOS say)
- `src/provider_factory.rs` - Provider creation with fallback chain

### Configuration Format

See `~/.config/sumvox/config.yaml`:
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
│   ├── hooks/            # Hook handlers
│   ├── llm/              # LLM providers
│   ├── tts/              # TTS engines
│   └── provider_factory.rs
├── config/
│   └── recommended.yaml  # Example configuration with comments
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

See `config/recommended.yaml` for the Gemini-based setup:
- Google Gemini for LLM (tested and optimized)
- macOS TTS for notifications (fast and free)
- Google TTS for summaries (high quality)
- YAML format with inline comments for easy customization
