# Ollama Provider Setup

The Ollama provider allows claude-voice to use local LLM models as a fallback when the primary Gemini API is unavailable or exceeds budget limits.

## Prerequisites

1. Install Ollama: https://ollama.ai/download
2. Start Ollama service:
   ```bash
   ollama serve
   ```

3. Pull the llama3.1 model:
   ```bash
   ollama pull llama3.1
   ```

## Configuration

Add Ollama as a fallback model in `.claude/hooks/voice_config.json`:

```json
{
  "llm": {
    "models": {
      "primary": "gemini/gemini-2.0-flash-exp",
      "fallback": "ollama/llama3.1"
    }
  }
}
```

Or use it without the `ollama/` prefix:

```json
{
  "llm": {
    "models": {
      "primary": "gemini/gemini-2.0-flash-exp",
      "fallback": "llama3.1"
    }
  }
}
```

## Testing

Run the integration test (requires Ollama running):

```bash
cargo test test_generate_with_real_ollama -- --ignored --nocapture
```

## Behavior

- **Cost**: Free (local model)
- **Availability**: Always returns `true` (assumes local service is running)
- **Fallback**: Automatically used when Gemini API fails
- **Timeout**: Configurable (default: from config)

## API Endpoint

Default: `http://localhost:11434/api/generate`

Custom endpoint can be set programmatically:

```rust
let provider = OllamaProvider::with_base_url(
    "http://custom-host:11434".to_string(),
    "llama3.1".to_string(),
    Duration::from_secs(60),
);
```

## Model Support

Any Ollama-compatible model can be used:
- llama3.1 (recommended)
- llama3.2
- mistral
- codellama
- etc.

Check available models:
```bash
ollama list
```

## Token Counting

Ollama returns actual token counts in the response:
- `prompt_eval_count` → input_tokens
- `eval_count` → output_tokens

If not provided by Ollama, tokens are estimated as 0.
