#!/bin/bash
# Wrapper script for SumVox notification hook
# Intelligent voice notifications with LLM summarization

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
RUST_BINARY="$PROJECT_DIR/target/release/sumvox"
LOG_FILE="$HOME/.claude/sumvox-hook.log"

# Load environment variables
if [ -f "$PROJECT_DIR/.env" ]; then
    export $(grep -v '^#' "$PROJECT_DIR/.env" | xargs)
fi

# Enable debug logging (optional: set to 'info' or remove for less verbose output)
export RUST_LOG=debug

# Use Rust binary with JSON stdin processing (auto-detects Claude Code format)
if [ -x "$RUST_BINARY" ]; then
    # Log hook invocation and pipe through the binary
    echo "=== Hook triggered at $(date) ===" >> "$LOG_FILE"
    tee -a "$LOG_FILE" | "$RUST_BINARY" json 2>&1 | tee -a "$LOG_FILE"
else
    echo "Error: SumVox binary not found at $RUST_BINARY" >&2
    echo "Please build the project: cd $PROJECT_DIR && cargo build --release" >&2
    exit 1
fi
