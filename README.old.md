# Claude Voice Notification

Voice notifications for Claude Code with intelligent summarization in Traditional Chinese.

## Status: Production Ready (Rust Rewrite) ✅

High-performance Rust implementation with:
- ✅ 90+ automated tests (all passing)
- ✅ Multi-model LLM support (Gemini, Anthropic, OpenAI, Ollama)
- ✅ Multi-TTS support (Google Gemini TTS, macOS say)
- ✅ 7ms startup time (vs 200-300ms Python)
- ✅ Array-based provider fallback chains
- ✅ Cost tracking and budget controls
- ✅ Zero-dependency deployment

## Quick Start

1. **Build Release Binary**
   ```bash
   cargo build --release
   ```

2. **Initialize Configuration**
   ```bash
   ./target/release/claude-voice init
   ```

3. **Set API Keys**
   ```bash
   # Set Gemini API key for both LLM and TTS
   ./target/release/claude-voice credentials set google

   # Or use environment variables
   export GEMINI_API_KEY="AIza..."
   ```

4. **Register Hook**
   Add to `~/.claude/settings.json`:
   ```json
   {
     "hooks": {
       "Notification": [
         {
           "matcher": "",
           "hooks": [
             {
               "type": "command",
               "command": "/absolute/path/to/claude-voice/.claude/hooks/run_voice_hook.sh"
             }
           ]
         }
       ],
       "Stop": [
         {
           "matcher": "",
           "hooks": [
             {
               "type": "command",
               "command": "/absolute/path/to/claude-voice/.claude/hooks/run_voice_hook.sh"
             }
           ]
         }
       ]
     }
   }
   ```

5. **Test It**
   Hook will trigger automatically when Claude Code stops or sends notifications.

## Features

### End-to-End Pipeline
**Claude Code Hook Event → Transcript Reader → LLM Summarizer → TTS Engine**

- **Transcript Parsing**: Reads last 10 text blocks from Claude Code session transcripts
- **Intelligent Summarization**: Multi-model LLM generates concise Traditional Chinese summaries (max 100 chars)
- **Multi-Model LLM**: Array-based fallback chain
  - Gemini 2.5 Flash (primary, handles "thinking" tokens properly)
  - Ollama (local, free, no API key needed)
  - OpenAI GPT-4o-mini
  - Anthropic Claude Haiku
- **Multi-TTS Support**: Array-based fallback chain
  - Google Gemini TTS (cloud, high quality)
  - macOS say (local, always available)
- **Cost Control**: Daily budget limits ($0.10 default) and usage tracking
- **Performance**: 7ms startup time, <5s total pipeline

### Configuration System

- **Unified Config**: Single `~/.claude/claude-voice.json` file
- **Provider Arrays**: Define fallback chains for both LLM and TTS
- **Environment Variables**: API keys from config or env vars
- **CLI Overrides**: All settings can be overridden via CLI flags

### Test Coverage

- **90+ Total Tests**: Comprehensive unit and integration tests
- **Module Coverage**:
  - Config loading/validation
  - LLM providers (Gemini, Anthropic, OpenAI, Ollama)
  - TTS providers (Google, macOS)
  - Transcript parsing (supports Claude Code format)
  - Provider factory with fallback logic
- **Fast Execution**: Full test suite runs in <1 second

### Documentation

- **Phase 7 Implementation**: [PHASE7_IMPLEMENTATION.md](PHASE7_IMPLEMENTATION.md)
- **TDD Summary**: [TDD_SUMMARY.md](TDD_SUMMARY.md)
- **Ollama Setup**: [docs/ollama-setup.md](docs/ollama-setup.md)

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│ Claude Code Hook Event (JSON via stdin)                │
│ {"session_id", "transcript_path", "hook_event_name"}   │
└──────────────────┬──────────────────────────────────────┘
                   │
                   ▼
┌─────────────────────────────────────────────────────────┐
│ main.rs - Hook Orchestrator                            │
│ - Parse JSON input                                     │
│ - Load config from ~/.claude/claude-voice.json         │
│ - Check if enabled                                     │
└──────────────────┬──────────────────────────────────────┘
                   │
                   ▼
┌─────────────────────────────────────────────────────────┐
│ transcript.rs - Transcript Reader                      │
│ - Parse Claude Code JSONL transcript                   │
│ - Extract last 10 assistant text blocks                │
│ - Support both test and production formats             │
└──────────────────┬──────────────────────────────────────┘
                   │
                   ▼
┌─────────────────────────────────────────────────────────┐
│ provider_factory.rs - LLM Provider Factory             │
│ - Try providers in array order                         │
│ - First available provider wins                        │
│ - Fallback chain: Google → Ollama → OpenAI → Claude    │
└──────────────────┬──────────────────────────────────────┘
                   │
                   ▼
┌─────────────────────────────────────────────────────────┐
│ llm/*.rs - LLM Provider Implementation                 │
│ - GeminiProvider (handles thinking tokens)             │
│ - OllamaProvider (local, free)                         │
│ - OpenAIProvider                                       │
│ - AnthropicProvider                                    │
│ - Cost estimation & token counting                     │
└──────────────────┬──────────────────────────────────────┘
                   │
                   ▼
┌─────────────────────────────────────────────────────────┐
│ tts/mod.rs - TTS Provider Factory                      │
│ - Try providers in array order                         │
│ - Fallback chain: Google TTS → macOS say               │
└──────────────────┬──────────────────────────────────────┘
                   │
                   ▼
┌─────────────────────────────────────────────────────────┐
│ tts/*.rs - TTS Implementation                          │
│ - GoogleTtsProvider (Gemini 2.5 Flash TTS)             │
│ - MacOsTtsProvider (system say command)                │
│ - Audio playback via rodio                             │
└─────────────────────────────────────────────────────────┘
```

## Testing

### Run All Tests
```bash
cargo test
```

### Run Specific Module Tests
```bash
# Test LLM providers
cargo test llm::

# Test TTS providers
cargo test tts::

# Test config loading
cargo test config::

# Test transcript parsing
cargo test transcript::
```

### Run with Output
```bash
cargo test -- --nocapture
```

### Manual Testing
```bash
# Test with mock input
echo '{"session_id":"test","transcript_path":"/path/to/transcript.jsonl","permission_mode":"auto","hook_event_name":"Stop","stop_hook_active":false}' | cargo run

# Test with real transcript
echo '{"session_id":"test","transcript_path":"~/.claude/projects/.../session.jsonl","permission_mode":"auto","hook_event_name":"Stop","stop_hook_active":false}' | RUST_LOG=info cargo run
```

## Performance

Rust implementation performance:
- **Startup time**: 7ms (vs 200-300ms Python)
- **Transcript parsing**: <10ms (JSONL parsing)
- **LLM summary generation**: 1-3s (depends on API)
- **TTS playback**: 15-30s (blocking, audio duration)
- **Total pipeline**: 2-5s (optimized HTTP client)

View detailed logs with `RUST_LOG=debug`

## Configuration

Edit `~/.claude/claude-voice.json`:

```json
{
  "version": "2.0.0",
  "enabled": true,
  "llm": {
    "providers": [
      {
        "name": "google",
        "model": "gemini-2.5-flash",
        "api_key": "AIza...",
        "timeout": 10
      },
      {
        "name": "ollama",
        "model": "llama3.2",
        "timeout": 10
      }
    ],
    "parameters": {
      "max_tokens": 10000,
      "temperature": 0.3
    },
    "cost_control": {
      "daily_limit_usd": 0.10,
      "usage_tracking": true,
      "usage_file": "~/.claude/voice-usage.json"
    }
  },
  "tts": {
    "providers": [
      {
        "name": "google",
        "voice": "Aoede",
        "api_key": "AIza..."
      },
      {
        "name": "macos",
        "voice": "Ting-Ting",
        "rate": 200
      }
    ]
  },
  "summarization": {
    "max_length": 100,
    "prompt_template": "你是語音通知助手。根據以下 Claude Code 對話內容，生成簡潔的繁體中文摘要（最多 {max_length} 字）。摘要應涵蓋主要討論的技術問題和解決方案。\n\n對話內容：\n{context}\n\n摘要："
  },
  "advanced": {
    "fallback_message": "任務已完成"
  }
}
```

See [PHASE7_IMPLEMENTATION.md](PHASE7_IMPLEMENTATION.md) for full configuration options.

## Development

### Project Structure
```
claude-voice/
├── src/                    # Rust source code
│   ├── main.rs            # Entry point & orchestration
│   ├── config.rs          # Configuration management
│   ├── transcript.rs      # JSONL transcript parsing
│   ├── credentials.rs     # API key management
│   ├── cli.rs            # Command-line interface
│   ├── provider_factory.rs # LLM provider factory
│   ├── llm/              # LLM providers
│   │   ├── mod.rs
│   │   ├── gemini.rs     # Google Gemini
│   │   ├── anthropic.rs  # Claude
│   │   ├── openai.rs     # GPT models
│   │   └── ollama.rs     # Local Ollama
│   ├── tts/              # TTS engines
│   │   ├── mod.rs
│   │   ├── google.rs     # Gemini TTS
│   │   └── macos.rs      # macOS say
│   └── voice.rs          # Legacy voice module
├── .claude/hooks/         # Hook wrapper
│   └── run_voice_hook.sh
├── docs/                  # Documentation
│   └── ollama-setup.md
├── PHASE7_IMPLEMENTATION.md
├── TDD_SUMMARY.md
└── Cargo.toml
```

### Development History

Completed Phases:
- ✅ Phase 1-6: Python implementation (legacy)
- ✅ Phase 7: Unified config architecture (Rust)
- ✅ Rust rewrite: 7ms startup, multi-provider support
- ✅ Transcript format fix: Support Claude Code JSONL format
- ✅ Gemini thinking tokens: Handle extended token limits

## License

MIT

## Contributing

See development logs in `.agents/tasks/` for implementation details and best practices.
