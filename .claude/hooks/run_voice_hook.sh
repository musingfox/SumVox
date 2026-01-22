#!/bin/bash
# Wrapper script for claude-voice notification hook
# Now using Rust binary for 7ms startup (vs 200-300ms Python)

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
RUST_BINARY="$PROJECT_DIR/target/release/claude-voice"

# Load environment variables
if [ -f "$PROJECT_DIR/.env" ]; then
    export $(grep -v '^#' "$PROJECT_DIR/.env" | xargs)
fi

# Use Rust binary if available, otherwise fall back to Python
if [ -x "$RUST_BINARY" ]; then
    exec "$RUST_BINARY"
else
    cd "$PROJECT_DIR" && uv run python "$SCRIPT_DIR/voice_notification.py"
fi
