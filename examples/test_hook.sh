#!/bin/bash
# Test script for voice notification hook
#
# Usage:
#   ./examples/test_hook.sh [event_name]
#
# Examples:
#   ./examples/test_hook.sh successful_code_generation
#   ./examples/test_hook.sh test_failure
#   ./examples/test_hook.sh git_commit_push
#
# If no event name is provided, lists available events

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
EVENTS_FILE="$SCRIPT_DIR/sample_stop_events.json"
HOOK_SCRIPT="$PROJECT_ROOT/.claude/hooks/voice_notification.py"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check dependencies
check_dependencies() {
    if ! command -v jq &> /dev/null; then
        echo -e "${RED}Error: jq is required but not installed${NC}"
        echo "Install with: brew install jq"
        exit 1
    fi

    if [ ! -f "$HOOK_SCRIPT" ]; then
        echo -e "${RED}Error: Hook script not found at $HOOK_SCRIPT${NC}"
        exit 1
    fi

    if [ ! -f "$EVENTS_FILE" ]; then
        echo -e "${RED}Error: Events file not found at $EVENTS_FILE${NC}"
        exit 1
    fi
}

# List available events
list_events() {
    echo -e "${GREEN}Available test events:${NC}"
    echo ""
    jq -r '.events[] | "  \(.name)\n    → \(.description)\n"' "$EVENTS_FILE"
    echo -e "${YELLOW}Usage:${NC} $0 <event_name>"
    echo ""
}

# Run test with specific event
run_test() {
    local event_name=$1

    echo -e "${GREEN}Testing voice notification hook${NC}"
    echo -e "Event: ${YELLOW}$event_name${NC}"
    echo ""

    # Extract event data
    local event_json=$(jq -r --arg name "$event_name" '.events[] | select(.name == $name) | .event' "$EVENTS_FILE")

    if [ -z "$event_json" ] || [ "$event_json" = "null" ]; then
        echo -e "${RED}Error: Event '$event_name' not found${NC}"
        echo ""
        list_events
        exit 1
    fi

    echo -e "${YELLOW}Event data:${NC}"
    echo "$event_json" | jq .
    echo ""

    # Run hook with event data
    echo -e "${GREEN}Executing hook...${NC}"
    echo ""

    if echo "$event_json" | python "$HOOK_SCRIPT"; then
        echo ""
        echo -e "${GREEN}✓ Hook executed successfully${NC}"
    else
        echo ""
        echo -e "${RED}✗ Hook execution failed${NC}"
        exit 1
    fi
}

# Main
main() {
    check_dependencies

    if [ $# -eq 0 ]; then
        list_events
        exit 0
    fi

    run_test "$1"
}

main "$@"
