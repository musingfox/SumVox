# Claude Code Voice Notification Hook Setup Guide

This guide explains how to register and use the voice notification hook with Claude Code.

## Overview

The voice notification hook provides audio feedback when Claude Code completes tasks. It:
- Detects when Claude Code stops execution
- Analyzes the execution output
- Generates a concise Traditional Chinese summary using LLM
- Plays the summary using macOS voice synthesis

## Prerequisites

1. **macOS**: The voice engine uses the macOS `say` command
2. **Python 3.12+**: Installed with `uv`
3. **API Keys**: At least one of:
   - `GEMINI_API_KEY` (recommended, most cost-effective)
   - `ANTHROPIC_API_KEY` (fallback)
   - `OPENAI_API_KEY` (fallback)

## Installation Steps

### 1. Set Up Environment Variables

Create a `.env` file in the project root:

```bash
# Primary LLM (recommended)
GEMINI_API_KEY=your_gemini_api_key_here

# Fallback LLMs (optional but recommended)
ANTHROPIC_API_KEY=your_anthropic_api_key_here
OPENAI_API_KEY=your_openai_api_key_here
```

### 2. Install Dependencies

```bash
cd /path/to/claude-voice
uv sync
```

### 3. Configure Voice Settings

Edit `.claude/hooks/voice_config.json` to customize:

```json
{
  "enabled": true,
  "voice": {
    "engine": "macos_say",
    "voice_name": "Ting-Ting",  // Traditional Chinese voice
    "rate": 200,                 // Speech rate (150-250)
    "volume": 75,                // Volume (0-100)
    "max_summary_length": 50,    // Max characters
    "async": true                // Non-blocking playback
  },
  "triggers": {
    "on_completion": true,       // Notify on success
    "on_error": true,            // Notify on errors
    "min_duration_seconds": 1.0  // Only for tasks > 1 second
  }
}
```

### 4. Register Hook with Claude Code

#### Option A: User-wide Hook (Recommended)

Add to `~/.claude/settings.json`:

```json
{
  "hooks": {
    "stop": [
      "/absolute/path/to/claude-voice/.claude/hooks/voice_notification.py"
    ]
  }
}
```

Replace `/absolute/path/to/claude-voice` with the actual path.

#### Option B: Project-specific Hook

Add to project's `.claude/settings.local.json`:

```json
{
  "hooks": {
    "stop": [
      "./.claude/hooks/voice_notification.py"
    ]
  }
}
```

## Verification

### Test the Hook Manually

Use the provided test script:

```bash
# List available test events
./examples/test_hook.sh

# Test with successful code generation event
./examples/test_hook.sh successful_code_generation

# Test with error detection event
./examples/test_hook.sh test_failure

# Test with git operation event
./examples/test_hook.sh git_commit_push
```

### Test with Claude Code

1. Start Claude Code
2. Ask it to perform a task (e.g., "create a hello world Python script")
3. Wait for task completion
4. You should hear a voice notification in Traditional Chinese

## Hook Event Format

Claude Code sends JSON events to the hook via stdin:

```json
{
  "output": "Created new file: src/example.py\n✓ Tests passed",
  "duration": 2.5,
  "exit_code": 0,
  "timestamp": "2025-11-11T14:00:00Z"
}
```

The hook processes this data and:
1. Checks if notification should trigger (based on duration, exit code, etc.)
2. Extracts context (operation type, status, key data)
3. Generates Traditional Chinese summary using LLM
4. Plays audio notification

## Customization

### Adjust Trigger Conditions

Edit `.claude/hooks/voice_config.json`:

```json
{
  "triggers": {
    "on_completion": true,        // Notify on successful completion
    "on_error": true,             // Notify on errors
    "min_duration_seconds": 5.0,  // Only for tasks longer than 5s
    "error_keywords": [           // Custom error detection
      "Error:",
      "Failed:",
      "Exception:",
      "FAILED"
    ]
  }
}
```

### Change Voice

List available macOS voices:

```bash
say -v "?"
```

Update `voice_name` in config:
- `Ting-Ting` - Traditional Chinese (Female)
- `Sin-ji` - Traditional Chinese (Male)
- `Mei-Jia` - Traditional Chinese (Female)

### Adjust Summary Prompt

Edit the `prompt_template` in config:

```json
{
  "summarization": {
    "prompt_template": "Summarize in Traditional Chinese, max {max_length} characters. Include: 1) operation type 2) result status 3) key data. Context: {context}"
  }
}
```

### Cost Control

The hook includes cost tracking to prevent excessive API usage:

```json
{
  "llm": {
    "cost_control": {
      "daily_limit_usd": 0.10,        // $0.10 per day limit
      "usage_tracking": true,          // Track usage
      "usage_file": "~/.claude/voice-usage.json"
    }
  }
}
```

## Troubleshooting

### No Audio Output

1. **Check audio device**: Ensure your Mac's audio is not muted
2. **Test voice engine**: Run `say "測試"` in terminal
3. **Check voice name**: Verify `Ting-Ting` is installed (`say -v "?"`)

### Hook Not Triggering

1. **Check hook registration**: Verify path in `~/.claude/settings.json`
2. **Check minimum duration**: Task might be too short (< `min_duration_seconds`)
3. **Check enabled flag**: Ensure `enabled: true` in config
4. **Check logs**: View `~/.claude/logs/voice-notifications.log`

### API Errors

1. **Verify API keys**: Check `.env` file has valid keys
2. **Test API access**: Run integration tests with `uv run pytest tests/test_integration.py`
3. **Check budget limit**: Review usage in `~/.claude/voice-usage.json`
4. **Use fallback**: Hook will try alternative LLM models automatically

### Permission Errors

1. **Make script executable**: `chmod +x .claude/hooks/voice_notification.py`
2. **Check Python path**: Hook uses `#!/usr/bin/env python3`
3. **Verify dependencies**: Run `uv sync` to ensure all packages installed

## Logging

Logs are written to `~/.claude/logs/voice-notifications.log` with:
- Hook execution events
- Timing metrics for each pipeline stage
- LLM model selection and token usage
- Error messages and stack traces

View logs:
```bash
tail -f ~/.claude/logs/voice-notifications.log
```

Configure logging level in `voice_config.json`:
```json
{
  "logging": {
    "enabled": true,
    "log_level": "INFO",  // DEBUG, INFO, WARNING, ERROR
    "log_file": "~/.claude/logs/voice-notifications.log"
  }
}
```

## Performance

Expected timings:
- **Trigger check**: < 0.01s
- **Summary generation**: 0.5-3s (depends on LLM API)
- **Voice playback**: 0.01-0.05s (async mode)
- **Total pipeline**: 1-5s

For faster notifications:
1. Use `async: true` for non-blocking playback
2. Set shorter `max_summary_length` (e.g., 30 characters)
3. Use Gemini Flash 2.0 (fastest and cheapest)
4. Adjust LLM `timeout` setting (default 10s)

## Advanced Configuration

### Multiple Hooks

You can chain multiple hooks:

```json
{
  "hooks": {
    "stop": [
      "/path/to/voice_notification.py",
      "/path/to/other_notification.py"
    ]
  }
}
```

### Conditional Hooks

Use different configs for different projects by placing project-specific
`voice_config.json` in project's `.claude/hooks/` directory.

### Custom Fallback Messages

If LLM fails, hook uses fallback message:

```json
{
  "advanced": {
    "fallback_message": "Claude Code task completed",
    "retry_attempts": 3
  }
}
```

## Examples

See `examples/` directory for:
- `sample_stop_events.json` - Example hook events
- `test_hook.sh` - Manual testing script
- `HOOK_SETUP.md` - This guide

## Support

- **GitHub Issues**: Report bugs or request features
- **Test Suite**: Run `uv run pytest tests/` to verify setup
- **Development Log**: See `.agents/tasks/VOICE-007/coder.md` for implementation details

## Credits

Built as part of the Claude Voice Notification project (VOICE-007).
