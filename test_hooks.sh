#!/bin/bash
# Test script for claude-voice hooks

BINARY="./target/release/claude-voice"
TEST_DIR="/tmp/claude"

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo "=== Testing claude-voice hooks ==="
echo

# Setup
mkdir -p "$TEST_DIR"
echo '{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Test completed successfully."}]}}' > "$TEST_DIR/test_transcript.jsonl"
touch "$TEST_DIR/empty.jsonl"

# Test 1: permission_prompt (should speak)
echo -e "${GREEN}Test 1: permission_prompt${NC}"
echo '{"session_id":"test","transcript_path":"'$TEST_DIR'/empty.jsonl","permission_mode":"default","hook_event_name":"Notification","message":"Claude needs permission","notification_type":"permission_prompt"}' \
  | RUST_LOG=info "$BINARY" 2>&1 | grep -E "Notification type|Skipping|Processing"
echo

# Test 2: idle_prompt (should speak)
echo -e "${GREEN}Test 2: idle_prompt${NC}"
echo '{"session_id":"test","transcript_path":"'$TEST_DIR'/empty.jsonl","permission_mode":"default","hook_event_name":"Notification","message":"Claude is waiting","notification_type":"idle_prompt"}' \
  | RUST_LOG=info "$BINARY" 2>&1 | grep -E "Notification type|Skipping|Processing"
echo

# Test 3: elicitation_dialog (should speak)
echo -e "${GREEN}Test 3: elicitation_dialog${NC}"
echo '{"session_id":"test","transcript_path":"'$TEST_DIR'/empty.jsonl","permission_mode":"default","hook_event_name":"Notification","message":"Need more input","notification_type":"elicitation_dialog"}' \
  | RUST_LOG=info "$BINARY" 2>&1 | grep -E "Notification type|Skipping|Processing"
echo

# Test 4: auth_success (should skip)
echo -e "${YELLOW}Test 4: auth_success (should skip)${NC}"
echo '{"session_id":"test","transcript_path":"'$TEST_DIR'/empty.jsonl","permission_mode":"default","hook_event_name":"Notification","message":"Auth OK","notification_type":"auth_success"}' \
  | RUST_LOG=debug "$BINARY" 2>&1 | grep -E "Notification type|Skipping|Processing"
echo

# Test 5: Stop hook (should read transcript)
echo -e "${GREEN}Test 5: Stop hook${NC}"
echo '{"session_id":"test","transcript_path":"'$TEST_DIR'/test_transcript.jsonl","permission_mode":"default","hook_event_name":"Stop"}' \
  | RUST_LOG=info "$BINARY" 2>&1 | grep -E "Processing|Extracted|Generated summary"
echo

echo "=== All tests completed ==="
