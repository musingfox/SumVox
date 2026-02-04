# Provider Fallback Mechanism Fix

**Date**: 2026-02-05
**Issue**: Provider fallback not working - only uses first provider, doesn't try alternatives on failure
**Status**: ✅ FIXED

---

## Problem Description

When multiple LLM providers are configured in the fallback chain, the system would only try the first `is_available()` provider. If that provider's API call failed (timeout, network error, service down), it would return an empty summary and use the fallback message instead of trying the next configured provider.

### Example Scenario (Broken)

Config: `[ollama, openai, anthropic, google]`

1. Ollama is first in config
2. Ollama service is not running
3. System tries Ollama API → fails
4. **Expected**: Try OpenAI next
5. **Actual**: Return empty string, use fallback message "任務已完成"

---

## Root Cause

### Issue 1: ProviderFactory limitation

`ProviderFactory::create_from_config()` only returns the **first** provider where `is_available()` returns true.

```rust
// OLD CODE
pub fn create_from_config(providers: &[LlmProviderConfig]) -> Result<Box<dyn LlmProvider>> {
    for config in providers {
        match Self::create_single(config) {
            Ok(provider) => {
                if provider.is_available() {
                    return Ok(provider);  // <-- Returns first available, doesn't retry on failure
                }
            }
            // ...
        }
    }
}
```

### Issue 2: is_available() only checks credentials

`is_available()` only verifies that API keys are present, it doesn't actually test if the API is reachable:

```rust
fn is_available(&self) -> bool {
    !self.api_key.is_empty()  // <-- Only checks if key exists
}
```

For Ollama: Always returns `true` (no API key needed)

### Issue 3: generate() failure doesn't trigger fallback

In `generate_summary()` (both `claude_code.rs` and `main.rs`):

```rust
// OLD CODE
match provider.generate(&request).await {
    Ok(response) => {
        // Success path
        Ok(response.text.trim().to_string())
    }
    Err(e) => {
        tracing::error!("Provider {} failed: {}", provider.name(), e);
        Ok(String::new())  // <-- Returns empty, doesn't try next provider
    }
}
```

---

## Solution

### Changes Made

#### 1. Made create_single() public

**File**: `src/provider_factory.rs`

Changed `create_single()` from private to public to allow manual provider creation:

```rust
-    fn create_single(config: &LlmProviderConfig) -> Result<Box<dyn LlmProvider>> {
+    pub fn create_single(config: &LlmProviderConfig) -> Result<Box<dyn LlmProvider>> {
```

#### 2. Implemented real fallback in generate_summary()

**Files**: `src/hooks/claude_code.rs`, `src/main.rs`

Replaced the old logic with a loop that tries each provider in sequence:

```rust
// NEW CODE - Pseudocode
if CLI override provided {
    // Try only the specified provider
    create and try that provider
    if fails, return empty (no fallback for explicit choice)
} else {
    // Try each provider in config order
    for each provider_config in config.llm.providers {
        create provider
        if not available, continue to next

        try provider.generate()
        if success:
            record usage
            return summary
        if failure:
            log warning
            continue to next provider
    }

    // All providers failed
    return empty string (triggers fallback message)
}
```

**Key improvements:**
- Loops through all configured providers
- Continues to next provider on failure
- Logs each attempt with clear status
- Only gives up after all providers fail

---

## Testing

### Test Case 1: Ollama running (first provider available)

```bash
$ echo "Test" | sumvox sum - --max-length 10 --no-speak
```

**Result**: ✅ Uses Ollama directly (first provider)

**Logs**:
```
INFO Trying LLM provider: ollama (model: llama3.2)
INFO Provider ollama succeeded
```

### Test Case 2: Ollama stopped (first provider unavailable)

```bash
$ cat test_input.json | sumvox json --format claude-code
```

**Config order**: `[ollama, openai, anthropic, google]`

**Result**: ✅ Tries each provider until one succeeds

**Logs**:
```
INFO Trying LLM provider: ollama (model: llama3.2)
WARN Provider ollama failed: ... error sending request, trying next
INFO Trying LLM provider: openAI (model: gpt-5-nano)
WARN Provider openai failed: ... timeout, trying next
INFO Trying LLM provider: anthropic (model: claude-haiku-4-5)
INFO Provider anthropic succeeded
```

**Summary generated**: "測試完成：第一個提供者失敗，系統成功自動切換至備援提供者。"

### Test Case 3: CLI override

```bash
$ echo "Test" | sumvox sum - --provider google --model gemini-2.5-flash --no-speak
```

**Result**: ✅ Uses only the specified provider, doesn't fallback to others

---

## Behavior Changes

### Before Fix

| Scenario | Behavior |
|----------|----------|
| First provider fails | Uses fallback message immediately |
| Second provider works | Never tried |
| User configuration | Ignored after first failure |

### After Fix

| Scenario | Behavior |
|----------|----------|
| First provider fails | Tries next provider automatically |
| Second provider works | Uses successfully |
| All providers fail | Uses fallback message |
| CLI override specified | Only tries that provider (no fallback) |

---

## Code Changes Summary

**Files Modified**:
- `src/provider_factory.rs` - Made `create_single()` public
- `src/hooks/claude_code.rs` - Implemented fallback loop in `generate_summary()`
- `src/main.rs` - Implemented fallback loop in `generate_summary()` (duplicate function)
- `README.md` - Updated discontinued model info
- `src/tts/macos.rs` - Added `#[allow(dead_code)]` for volume field

**Tests**: ✅ All 233 tests still passing

**Performance**:
- Ollama running: Same (uses first provider)
- Ollama stopped: Slower (tries multiple providers), but more reliable
- Typical fallback delay: 2-3s per failed provider (timeout dependent)

---

## Recommendations

### For Users

1. **Order providers by preference** in config - fastest/cheapest first
2. **Use CLI override** (`--provider`, `--model`) if you want to force a specific provider without fallback
3. **Check logs** to see which provider was used

### For Future Development

1. **Consider provider health checks** - Ping providers before heavy requests
2. **Add retry logic** - Retry same provider once before moving to next
3. **Parallel provider attempts** - Try multiple providers simultaneously for lowest latency
4. **Provider priority weights** - Not just first-match, but preferred providers

---

## Related Issues

- macOS TTS `volume` field unused warning - ✅ Fixed with `#[allow(dead_code)]`
- README listed discontinued `gemini-2.0-flash-exp` - ✅ Removed
- All code formatting issues - ✅ Fixed with `cargo fmt`

---

**Verified by**: Comprehensive testing with Ollama on/off scenarios
**Impact**: High - Core functionality fix for production reliability
**Breaking changes**: None - Only improves existing behavior
