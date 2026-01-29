# Project Configuration

## Overview

**Language**: Rust
**Type**: CLI Tool / Claude Code Hook
**Purpose**: Voice notifications for Claude Code with multi-model LLM support

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

- **Main config**: `~/.claude/claude-voice.json`
- **Hook script**: `.claude/hooks/run_voice_hook.sh`
- **Binary location**: `target/release/claude-voice`

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
claude-voice init

# Set credentials
claude-voice credentials set google
claude-voice credentials list

# Run with CLI overrides
claude-voice --provider google --model gemini-2.5-flash
claude-voice --tts auto --tts-voice Aoede
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

See `~/.claude/claude-voice.json`:
- LLM providers array with fallback chain
- TTS providers array with fallback chain
- Summarization settings (max_length, prompt_template)
- Cost control (daily_limit_usd, usage_tracking)
