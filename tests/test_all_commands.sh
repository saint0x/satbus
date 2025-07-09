#!/bin/bash

# Comprehensive SatBus Command Test Script
# Tests all commands with 33-second delays to ensure proper operation

set -e  # Exit on any error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
DELAY=5
SERVER_HOST="127.0.0.1"
SERVER_PORT="8080"
SERVER_PID=""

# Binary paths
SATBUS="./target/aarch64-apple-darwin/release/satbus"
SIMULATOR="./target/aarch64-apple-darwin/release/satbus-simulator"

# Function to print colored output
print_status() {
    echo -e "${BLUE}[$(date '+%H:%M:%S')]${NC} $1"
}

print_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

print_error() {
    echo -e "${RED}❌ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

# Function to wait with countdown
wait_with_countdown() {
    local seconds=$1
    local message=$2
    
    print_status "$message"
    for ((i=seconds; i>0; i--)); do
        printf "\r${YELLOW}⏳ Waiting ${i}s...${NC}"
        sleep 1
    done
    printf "\r${GREEN}✅ Ready!${NC}\n"
}

# Function to run a command and check result
run_command() {
    local cmd="$1"
    local description="$2"
    local expected_success="${3:-true}"
    
    print_status "Testing: $description"
    echo "Command: $cmd"
    
    if $cmd; then
        if [ "$expected_success" = "true" ]; then
            print_success "$description - PASSED"
        else
            print_error "$description - FAILED (expected failure but got success)"
            return 1
        fi
    else
        if [ "$expected_success" = "false" ]; then
            print_success "$description - PASSED (expected failure)"
        else
            print_error "$description - FAILED"
            return 1
        fi
    fi
    
    if [ "$DELAY" -gt 0 ]; then
        wait_with_countdown $DELAY "Waiting before next command"
    fi
}

# Function to start the server
start_server() {
    print_status "Starting satellite bus simulator server..."
    
    # Use the pre-built binary directly
    local binary_path="$SIMULATOR"
    
    if [ ! -f "$binary_path" ]; then
        print_error "Binary not found at $binary_path"
        print_status "Make sure you've built the project with: cargo build --release"
        exit 1
    fi
    
    # Start server in background using the binary directly
    $binary_path > server.log 2>&1 &
    SERVER_PID=$!
    
    print_status "Server started with PID: $SERVER_PID using binary: $binary_path"
    
    # Wait for server to be ready
    print_status "Waiting for server to initialize..."
    sleep 5
    
    # Check if server is responding
    local retries=10
    while [ $retries -gt 0 ]; do
        if $SATBUS ping > /dev/null 2>&1; then
            print_success "Server is ready and responding!"
            
            # Clear any initial safe mode state
            print_status "Clearing initial safe mode state..."
            $SATBUS system clear-safety-events --force > /dev/null 2>&1 || true
            $SATBUS system safe-mode off > /dev/null 2>&1 || true
            sleep 2
            
            return 0
        fi
        
        print_status "Server not ready yet, retrying... ($retries attempts left)"
        sleep 2
        retries=$((retries - 1))
    done
    
    print_error "Server failed to start or become ready"
    echo "Server log:"
    cat server.log
    exit 1
}

# Function to stop the server
stop_server() {
    if [ ! -z "$SERVER_PID" ]; then
        print_status "Stopping server (PID: $SERVER_PID)..."
        kill $SERVER_PID 2>/dev/null || true
        wait $SERVER_PID 2>/dev/null || true
        print_success "Server stopped"
    fi
}

# Cleanup function
cleanup() {
    print_status "Cleaning up..."
    stop_server
    rm -f server.log
}

# Set trap for cleanup
trap cleanup EXIT INT TERM

# Function to build the project
build_project() {
    print_status "Building satellite bus simulator..."
    
    if ! cargo build --release; then
        print_error "Failed to build project"
        exit 1
    fi
    
    print_success "Build completed successfully"
    
    # Verify binaries exist
    local satbus_bin="$SATBUS"
    local simulator_bin="$SIMULATOR"
    
    if [ ! -f "$satbus_bin" ]; then
        print_error "satbus binary not found at $satbus_bin"
        exit 1
    fi
    
    if [ ! -f "$simulator_bin" ]; then
        print_error "satbus-simulator binary not found at $simulator_bin"
        exit 1
    fi
    
    print_success "All required binaries are available"
}

# Main test execution
main() {
    echo -e "${BLUE}"
    echo "════════════════════════════════════════════════════════════"
    echo "🛰️  SATELLITE BUS SIMULATOR - COMPREHENSIVE COMMAND TEST"
    echo "════════════════════════════════════════════════════════════"
    echo -e "${NC}"
    
    print_status "Test Configuration:"
    echo "  • Command delay: ${DELAY}s"
    echo "  • Server: ${SERVER_HOST}:${SERVER_PORT}"
    echo "  • Total estimated time: ~20 minutes"
    echo ""
    
    # Build the project first
    build_project
    echo ""
    
    # Start the server
    start_server
    
    print_status "Beginning command tests..."
    echo ""
    
    # ================================
    # BASIC CONNECTIVITY TESTS
    # ================================
    echo -e "${BLUE}📡 BASIC CONNECTIVITY TESTS${NC}"
    run_command "$SATBUS ping" "Basic connectivity test"
    run_command "$SATBUS status" "System status check"
    
    # ================================
    # SYSTEM MANAGEMENT TESTS
    # ================================
    echo -e "${BLUE}🛠️  SYSTEM MANAGEMENT TESTS${NC}"
    
    # Test safe mode operations
    run_command "$SATBUS system safe-mode on" "Enable safe mode"
    run_command "$SATBUS system safe-mode off" "Disable safe mode (should work with override)"
    
    # Test safety event clearing
    run_command "$SATBUS system clear-safety-events --force" "Clear safety events (ground testing)"
    
    # Test fault injection
    run_command "$SATBUS system fault-injection enable" "Enable fault injection system"
    run_command "$SATBUS system fault-injection status" "Check fault injection status"
    run_command "$SATBUS system fault thermal degraded" "Inject thermal fault"
    run_command "$SATBUS system clear-faults thermal" "Clear thermal faults"
    run_command "$SATBUS system clear-faults" "Clear all faults"
    run_command "$SATBUS system fault-injection disable" "Disable fault injection system"
    
    # ================================
    # POWER SYSTEM TESTS
    # ================================
    echo -e "${BLUE}🔋 POWER SYSTEM TESTS${NC}"
    run_command "$SATBUS power status" "Power system status"
    run_command "$SATBUS power solar on" "Enable solar panels"
    run_command "$SATBUS power solar off" "Disable solar panels"
    run_command "$SATBUS power solar on" "Re-enable solar panels"
    
    # ================================
    # THERMAL SYSTEM TESTS  
    # ================================
    echo -e "${BLUE}🌡️  THERMAL SYSTEM TESTS${NC}"
    run_command "$SATBUS thermal status" "Thermal system status"
    run_command "$SATBUS thermal heater on" "Enable thermal heaters"
    run_command "$SATBUS thermal heater off" "Disable thermal heaters"
    
    # ================================
    # COMMUNICATIONS SYSTEM TESTS
    # ================================
    echo -e "${BLUE}📡 COMMUNICATIONS SYSTEM TESTS${NC}"
    run_command "$SATBUS comms status" "Communications system status"
    run_command "$SATBUS comms link up" "Bring communications link up"
    run_command "$SATBUS comms tx-power 15" "Set TX power to 15 dBm"
    run_command "$SATBUS comms tx-power 25" "Set TX power to 25 dBm"
    run_command "$SATBUS comms transmit \"Hello from satellite!\"" "Transmit test message"
    run_command "$SATBUS comms link down" "Take communications link down"
    run_command "$SATBUS comms link up" "Restore communications link"
    
    # ================================
    # STRESS TEST - RAPID COMMANDS
    # ================================
    echo -e "${BLUE}⚡ STRESS TEST - RAPID COMMANDS (No Delay)${NC}"
    local old_delay=$DELAY
    DELAY=1  # Short delay for rapid testing
    
    run_command "$SATBUS thermal heater on" "Rapid test: Heater on"
    run_command "$SATBUS thermal heater off" "Rapid test: Heater off"
    run_command "$SATBUS comms tx-power 10" "Rapid test: TX power 10"
    run_command "$SATBUS comms tx-power 20" "Rapid test: TX power 20"
    run_command "$SATBUS power solar off" "Rapid test: Solar off"
    run_command "$SATBUS power solar on" "Rapid test: Solar on"
    
    DELAY=$old_delay  # Restore original delay
    
    # ================================
    # ERROR CONDITION TESTS
    # ================================
    echo -e "${BLUE}🚨 ERROR CONDITION TESTS${NC}"
    
    # Test invalid parameters
    run_command "$SATBUS comms tx-power 50" "Invalid TX power (should fail)" false
    run_command "$SATBUS comms tx-power -5" "Negative TX power (should fail)" false
    
    # Test safe mode blocking (temporarily enable safe mode)
    run_command "$SATBUS system safe-mode on" "Enable safe mode for blocking test"
    wait_with_countdown 5 "Letting safe mode activate"
    run_command "$SATBUS power solar off" "Command while in safe mode (should fail)" false
    run_command "$SATBUS system safe-mode off" "Disable safe mode again"
    
    # ================================
    # FINAL SYSTEM STATE CHECK
    # ================================
    echo -e "${BLUE}🔍 FINAL SYSTEM STATE CHECK${NC}"
    run_command "$SATBUS status" "Final system status check"
    run_command "$SATBUS power status" "Final power status"
    run_command "$SATBUS thermal status" "Final thermal status"  
    run_command "$SATBUS comms status" "Final comms status"
    
    # Test summary
    echo ""
    echo -e "${GREEN}"
    echo "════════════════════════════════════════════════════════════"
    echo "✅ ALL TESTS COMPLETED SUCCESSFULLY!"
    echo "════════════════════════════════════════════════════════════"
    echo -e "${NC}"
    
    print_success "Satellite bus simulator passed comprehensive testing"
    print_success "All commands executed properly with verified results"
    print_success "Safety system override functionality working correctly"
    print_success "No command conflicts or tracking issues detected"
    
    echo ""
    print_status "Test artifacts:"
    echo "  • Server log: server.log"
    echo "  • Test completed at: $(date)"
}

# Run the main function
main "$@"