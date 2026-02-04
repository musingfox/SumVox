#!/bin/bash
# Wrapper script for claude-voice notification hook
# Now using Rust binary for 7ms startup (vs 200-300ms Python)

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
RUST_BINARY="$PROJECT_DIR/target/release/claude-voice"
LOG_FILE="$HOME/.claude/claude-voice-hook.log"

# Load environment variables
if [ -f "$PROJECT_DIR/.env" ]; then
    export $(grep -v '^#' "$PROJECT_DIR/.env" | xargs)
fi

# Enable debug logging
export RUST_LOG=debug

# Use Rust binary with auto TTS fallback (config chain: Google → macOS)
if [ -x "$RUST_BINARY" ]; then
    # Log hook invocation and pipe through the binary
    echo "=== Hook triggered at $(date) ===" >> "$LOG_FILE"
    tee -a "$LOG_FILE" | "$RUST_BINARY" --tts auto --tts-voice Aoede 2>&1 | tee -a "$LOG_FILE"
else
    echo "Error: Rust binary not found at $RUST_BINARY" >&2
    exit 1
fi
