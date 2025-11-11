# Claude Voice Notification

Voice notifications for Claude Code task completions in Traditional Chinese.

## Status: Production Ready ✅

Complete end-to-end voice notification pipeline for Claude Code with:
- ✅ 78 automated tests (77 passing, 1 skipped)
- ✅ 78% code coverage
- ✅ Multi-model LLM support (Gemini, Claude, OpenAI, Ollama)
- ✅ Comprehensive error handling and fallbacks
- ✅ Performance optimized (<5s total pipeline)
- ✅ Cost tracking and budget controls
- ✅ Full documentation and examples

## Quick Start

1. **Install Dependencies**
   ```bash
   uv sync
   ```

2. **Configure API Keys**
   ```bash
   cp .env.example .env
   # Edit .env and add your API keys
   ```

3. **Register Hook**
   Add to `~/.claude/settings.json`:
   ```json
   {
     "hooks": {
       "stop": ["/absolute/path/to/claude-voice/.claude/hooks/voice_notification.py"]
     }
   }
   ```

4. **Test It**
   ```bash
   ./examples/test_hook.sh successful_code_generation
   ```

For detailed setup instructions, see [examples/HOOK_SETUP.md](examples/HOOK_SETUP.md).

## Features

### End-to-End Pipeline
**Claude Code Stop Event → Config → Summarizer → LLM → Voice Engine**

- **Automatic Trigger Detection**: Monitors task duration, exit codes, error keywords
- **Intelligent Summarization**: Extracts operation type, status, key data from output
- **Multi-Model LLM**: Automatic fallback between Gemini, Claude, OpenAI, Ollama
- **Voice Synthesis**: macOS `say` command with Traditional Chinese voices
- **Cost Control**: Daily budget limits and usage tracking
- **Performance Monitoring**: Detailed timing metrics for each pipeline stage

### Test Coverage

- **78 Total Tests**: Comprehensive coverage of all components
- **13 Integration Tests**: E2E scenarios including:
  - Successful operations (code generation, git, builds)
  - Error handling (test failures, exceptions, malformed input)
  - Edge cases (quick tasks, disabled notifications, sync/async modes)
  - Performance validation
- **Fast Execution**: Full test suite runs in ~7 seconds

### Documentation

- **Setup Guide**: [examples/HOOK_SETUP.md](examples/HOOK_SETUP.md)
- **Sample Events**: [examples/sample_stop_events.json](examples/sample_stop_events.json)
- **Test Script**: [examples/test_hook.sh](examples/test_hook.sh)
- **Development Log**: [.agents/tasks/VOICE-007/coder.md](.agents/tasks/VOICE-007/coder.md)

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│ Claude Code Stop Hook Event (JSON via stdin)          │
└──────────────────┬──────────────────────────────────────┘
                   │
                   ▼
┌─────────────────────────────────────────────────────────┐
│ voice_notification.py (Hook Orchestrator)              │
│ - Parse input                                          │
│ - Check trigger conditions                             │
│ - Coordinate pipeline stages                           │
└──────────────────┬──────────────────────────────────────┘
                   │
                   ▼
┌─────────────────────────────────────────────────────────┐
│ voice_config.py (Configuration Loader)                 │
│ - Load voice_config.json                               │
│ - Validate structure                                   │
│ - Provide settings to components                       │
└──────────────────┬──────────────────────────────────────┘
                   │
                   ▼
┌─────────────────────────────────────────────────────────┐
│ summarizer.py (Context Extraction & Summarization)     │
│ - Extract operation type, status, key data             │
│ - Build context string                                 │
│ - Generate fallback summary if needed                  │
└──────────────────┬──────────────────────────────────────┘
                   │
                   ▼
┌─────────────────────────────────────────────────────────┐
│ llm_adapter.py (Multi-Model LLM Support)               │
│ - Try primary model (Gemini Flash 2.0)                 │
│ - Automatic fallback to Claude/OpenAI                  │
│ - Cost tracking and budget control                     │
│ - Token usage logging                                  │
└──────────────────┬──────────────────────────────────────┘
                   │
                   ▼
┌─────────────────────────────────────────────────────────┐
│ voice_engine.py (macOS Voice Synthesis)                │
│ - Validate voice availability                          │
│ - Execute `say` command                                │
│ - Support sync/async playback                          │
└─────────────────────────────────────────────────────────┘
```

## Testing

### Run All Tests
```bash
uv run pytest tests/ -v
```

### Run Integration Tests
```bash
uv run pytest tests/test_integration.py -v
```

### Run with Coverage
```bash
uv run pytest tests/ --cov=.claude/hooks --cov-report=term-missing
```

### Manual Testing
```bash
# List available test events
./examples/test_hook.sh

# Test specific scenario
./examples/test_hook.sh successful_code_generation
./examples/test_hook.sh test_failure
./examples/test_hook.sh git_commit_push
```

## Performance

Typical pipeline timing (with mocked LLM):
- **Trigger check**: <0.01s
- **Summary generation**: 0.5-3s (depends on LLM API)
- **Voice playback**: 0.01-0.05s (async mode)
- **Total pipeline**: 1-5s

View detailed timing in logs: `~/.claude/logs/voice-notifications.log`

## Configuration

Edit `.claude/hooks/voice_config.json`:

```json
{
  "enabled": true,
  "voice": {
    "voice_name": "Ting-Ting",
    "rate": 200,
    "volume": 75,
    "async": true
  },
  "triggers": {
    "on_completion": true,
    "on_error": true,
    "min_duration_seconds": 1.0
  },
  "llm": {
    "provider": "gemini",
    "cost_control": {
      "daily_limit_usd": 0.10
    }
  }
}
```

See [examples/HOOK_SETUP.md](examples/HOOK_SETUP.md) for full configuration options.

## Development

### Project Structure
```
claude-voice/
├── .claude/hooks/          # Hook implementation
│   ├── voice_notification.py
│   ├── voice_config.py
│   ├── llm_adapter.py
│   ├── summarizer.py
│   ├── voice_engine.py
│   └── voice_config.json
├── tests/                  # Test suite
│   ├── test_integration.py (13 E2E tests)
│   ├── test_voice_notification.py
│   ├── test_llm_adapter.py
│   ├── test_summarizer.py
│   └── test_voice_engine.py
├── examples/              # Documentation & samples
│   ├── HOOK_SETUP.md
│   ├── sample_stop_events.json
│   ├── test_hook.sh
│   └── claude_settings_example.json
└── .agents/tasks/        # Development logs
    └── VOICE-007/
        └── coder.md
```

### Development Tasks

Completed:
- ✅ VOICE-001: Python environment setup
- ✅ VOICE-002: Configuration file architecture
- ✅ VOICE-003: Hook integration layer
- ✅ VOICE-004: LiteLLM multi-model adapter
- ✅ VOICE-005: Summarization engine
- ✅ VOICE-006: macOS voice engine
- ✅ VOICE-007: Complete pipeline integration

## License

MIT

## Contributing

See development logs in `.agents/tasks/` for implementation details and best practices.
