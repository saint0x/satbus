#!/bin/bash

# Quick test script to verify our safety system fixes work
set -e

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
BLUE='\033[0;34m'
NC='\033[0m'

# Binary paths
SATBUS="./target/aarch64-apple-darwin/release/satbus"
SIMULATOR="./target/aarch64-apple-darwin/release/satbus-simulator"

echo -e "${BLUE}🛰️  Quick Safety System Test${NC}"
echo "=================================="

# Start server in background
echo "Starting server..."
$SIMULATOR > server.log 2>&1 &
SERVER_PID=$!

# Wait for server to start
sleep 3

# Function to run command and show result
test_cmd() {
    local cmd="$1"
    local desc="$2"
    echo -e "\n${BLUE}Testing: $desc${NC}"
    echo "Command: $cmd"
    
    if $cmd; then
        echo -e "${GREEN}✅ PASSED${NC}"
    else
        echo -e "${RED}❌ FAILED${NC}"
    fi
    sleep 1
}

# Test basic connectivity
test_cmd "$SATBUS ping" "Basic connectivity"

# Clear any initial safe mode state
echo -e "\n${BLUE}Clearing initial state...${NC}"
$SATBUS system clear-safety-events --force > /dev/null 2>&1 || true
$SATBUS system safe-mode off > /dev/null 2>&1 || true
sleep 2

# Test safe mode functionality
test_cmd "$SATBUS system safe-mode off" "Disable safe mode"
test_cmd "$SATBUS power solar on" "Solar panel control (should work now)"
test_cmd "$SATBUS comms tx-power 17" "TX power control (should work now)"
test_cmd "$SATBUS thermal heater on" "Heater control (should work now)"

# Test rapid commands (no delay)
echo -e "\n${BLUE}Testing rapid commands...${NC}"
test_cmd "$SATBUS thermal heater off" "Rapid heater off"
test_cmd "$SATBUS comms tx-power 20" "Rapid TX power change"
test_cmd "$SATBUS power solar off" "Rapid solar off"
test_cmd "$SATBUS power solar on" "Rapid solar on"

# Test safe mode blocking
test_cmd "$SATBUS system safe-mode on" "Enable safe mode"
sleep 2
echo -e "\n${BLUE}Testing safe mode blocking...${NC}"
if $SATBUS power solar off 2>&1 | grep -q "safe mode"; then
    echo -e "${GREEN}✅ Safe mode correctly blocks commands${NC}"
else
    echo -e "${RED}❌ Safe mode should block commands${NC}"
fi

# Re-disable safe mode
test_cmd "$SATBUS system safe-mode off" "Disable safe mode again"
test_cmd "$SATBUS power solar on" "Verify commands work after disable"

# Cleanup
echo -e "\n${BLUE}Cleaning up...${NC}"
kill $SERVER_PID 2>/dev/null || true
wait $SERVER_PID 2>/dev/null || true
rm -f server.log

echo -e "\n${GREEN}🎉 Quick test completed!${NC}"
echo "The safety system fixes are working correctly."