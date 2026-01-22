#!/bin/bash
# Wrapper script to run voice_notification.py with uv-managed Python environment

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"

cd "$PROJECT_DIR" && uv run python "$SCRIPT_DIR/voice_notification.py"
