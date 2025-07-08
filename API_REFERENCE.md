# Satellite Bus Simulator - API Reference

## Overview

The Satellite Bus Simulator provides a comprehensive embedded-style satellite bus simulation with real-time subsystem management, command processing, telemetry generation, and safety management. This document covers all public APIs and interfaces.

## Core Components

### 1. SatelliteAgent

The main orchestrator for the satellite bus simulator.

#### Creation and Configuration

```rust
use satbus::SatelliteAgent;

// Create new satellite agent
let mut agent = SatelliteAgent::new();

// Configure update rate (default: 100ms)
agent.set_update_rate(50); // 50ms updates
```

#### Command Processing

```rust
// Process a command from JSON string
let command_json = r#"{"id":123,"timestamp":1000,"command_type":"Ping","execution_time":null}"#;
let response = agent.process_command(command_json);

// Get command processing statistics
let stats = agent.get_command_stats();
println!("Commands processed: {}", stats.total_processed);
```

#### System Updates

```rust
// Update all subsystems (call this regularly in your main loop)
let current_time = 1000; // milliseconds since epoch
agent.update(current_time);

// Check if system is healthy
if agent.is_system_healthy() {
    println!("All systems nominal");
}
```

#### Telemetry Generation

```rust
// Generate telemetry packet
let telemetry = agent.generate_telemetry();

// Get telemetry as JSON string
let telemetry_json = agent.get_telemetry_json();
```

### 2. Protocol Handler

Handles command parsing, validation, and response generation.

#### Command Parsing

```rust
use satbus::protocol::ProtocolHandler;

let mut handler = ProtocolHandler::new();

// Parse command from JSON
let command_json = r#"{"id":456,"timestamp":2000,"command_type":"SetTxPower","power_dbm":20}"#;
match handler.parse_command(command_json) {
    Ok(command) => println!("Parsed command: {:?}", command),
    Err(e) => println!("Parse error: {}", e),
}
```

#### Command Validation

```rust
use satbus::protocol::{Command, CommandType};

let command = Command {
    id: 123,
    timestamp: 1000,
    command_type: CommandType::SetTxPower { power_dbm: 25 },
    execution_time: None,
};

// Validate command parameters
match handler.validate_command(&command) {
    Ok(()) => println!("Command is valid"),
    Err(e) => println!("Validation error: {}", e),
}
```

#### Response Generation

```rust
use satbus::protocol::ResponseStatus;

// Create various response types
let ack = handler.create_ack_response(123, Some("Command received"));
let nack = handler.create_nack_response(124, "Invalid parameter");
let exec_start = handler.create_execution_started_response(125);
let exec_failed = handler.create_execution_failed_response(126, "System error");
let timeout = handler.create_timeout_response(127);

// Serialize response to JSON
let json = handler.serialize_response(&ack).unwrap();
```

#### Command Tracking

```rust
// Track command execution with ACK/NACK semantics
handler.track_command(123, current_time, 5000).unwrap(); // 5 second timeout

// Update command status
handler.update_command_status(123, ResponseStatus::ExecutionStarted, current_time + 100);
handler.update_command_status(123, ResponseStatus::Success, current_time + 500);

// Get command status
if let Some(tracker) = handler.get_command_status(123) {
    println!("Command {} status: {:?}", tracker.command_id, tracker.status);
}

// Clean up expired commands
handler.cleanup_expired_commands(current_time + 10000);
```

### 3. Subsystems

#### Power Subsystem

```rust
use satbus::subsystems::{PowerSystem, power::PowerCommand};

let mut power = PowerSystem::new();

// Execute commands
power.execute_command(PowerCommand::SetSolarPanel(true)).unwrap();
power.execute_command(PowerCommand::SetPowerSave(false)).unwrap();

// Get system state
let state = power.get_state();
println!("Battery: {}mV, {}%", state.battery_voltage_mv, state.battery_level_percent);

// Update system (call regularly)
power.update(100).unwrap(); // 100ms delta

// Health management
if !power.is_healthy() {
    power.clear_faults();
}
```

#### Thermal Subsystem

```rust
use satbus::subsystems::{ThermalSystem, thermal::ThermalCommand};

let mut thermal = ThermalSystem::new();

// Execute commands
thermal.execute_command(ThermalCommand::SetHeaterState(true)).unwrap();

// Get system state
let state = thermal.get_state();
println!("Core temp: {}°C, Heater: {}W", state.core_temp_c, state.heater_power_w);

// Update and health management
thermal.update(100).unwrap();
thermal.inject_fault(FaultType::Degraded); // For testing
```

#### Communications Subsystem

```rust
use satbus::subsystems::{CommsSystem, comms::CommsCommand};
use arrayvec::ArrayString;

let mut comms = CommsSystem::new();

// Execute commands
comms.execute_command(CommsCommand::SetLinkState(true)).unwrap();
comms.execute_command(CommsCommand::SetTxPower(30)).unwrap();

// Transmit message
let mut message = ArrayString::<256>::new();
message.push_str("Hello, Ground!");
comms.execute_command(CommsCommand::TransmitMessage(message)).unwrap();

// Get system state
let state = comms.get_state();
println!("Link: {}, RX: {}, TX: {}", state.link_up, state.rx_packets, state.tx_packets);
```

### 4. Safety Manager

Monitors system health and manages safe mode operations.

#### Basic Operations

```rust
use satbus::safety::SafetyManager;

let mut safety = SafetyManager::new();

// Update safety state with subsystem health
let actions = safety.update_safety_state(
    current_time,
    &power_system,
    &thermal_system,
    &comms_system,
);

// Check safety status
let state = safety.get_state();
if state.safe_mode_active {
    println!("System in safe mode - level: {:?}", state.safety_level);
}
```

#### Manual Safe Mode Control

```rust
// Force safe mode entry
let actions = safety.force_safe_mode(current_time);
if actions.enable_emergency_power_save {
    // Apply emergency power saving measures
}

// Exit safe mode
let exit_actions = safety.disable_safe_mode(current_time);
if exit_actions.restore_normal_operations {
    // Restore normal system operations
}
```

#### Safety Event Management

```rust
// Get safety event history
let events = safety.get_event_history();
for event in events {
    println!("Event: {:?} at {} - Level: {:?}", 
        event.event, event.timestamp, event.level);
}

// Clear resolved events
safety.clear_resolved_events();
```

### 5. Command Scheduler

Handles time-tagged command execution.

#### Basic Scheduling

```rust
use satbus::scheduler::CommandScheduler;
use satbus::protocol::{Command, CommandType};

let mut scheduler = CommandScheduler::new();

// Schedule immediate command
let immediate_cmd = Command {
    id: 1,
    timestamp: current_time,
    command_type: CommandType::Ping,
    execution_time: None, // Execute immediately
};
scheduler.schedule_command(immediate_cmd, current_time).unwrap();

// Schedule future command
let future_cmd = Command {
    id: 2,
    timestamp: current_time,
    command_type: CommandType::SystemStatus,
    execution_time: Some(current_time + 5000), // Execute in 5 seconds
};
scheduler.schedule_command(future_cmd, current_time).unwrap();
```

#### Command Execution

```rust
// Get commands ready for execution
let ready_commands = scheduler.get_ready_commands(current_time);
for command in ready_commands {
    // Process each ready command
    println!("Executing command: {:?}", command);
}

// Clean up expired commands
scheduler.cleanup_expired_commands(current_time);
```

#### Scheduler Configuration

```rust
// Set command timeout (default: 3600 seconds)
scheduler.set_timeout_seconds(1800); // 30 minutes

// Get scheduler statistics
let stats = scheduler.get_stats();
println!("Scheduled: {}, Executed: {}, Expired: {}", 
    stats.total_scheduled, stats.total_executed, stats.total_expired);

// Clear all pending commands
scheduler.clear_all_scheduled();
```

## Data Types

### Command Types

All available command types and their parameters:

```rust
use satbus::protocol::CommandType;
use satbus::subsystems::{SubsystemId, FaultType};

// Basic commands
CommandType::Ping                           // Health check
CommandType::SystemStatus                   // Get system status
CommandType::SystemReboot                   // Restart system

// Power management
CommandType::SetSolarPanel { enabled: bool }

// Thermal management  
CommandType::SetHeaterState { on: bool }

// Communications
CommandType::SetCommsLink { enabled: bool }
CommandType::SetTxPower { power_dbm: i8 }   // 0-30 dBm
CommandType::TransmitMessage { message: String }

// Safety and diagnostics
CommandType::SetSafeMode { enabled: bool }
CommandType::SimulateFault { target: SubsystemId, fault_type: FaultType }
CommandType::ClearFaults { target: Option<SubsystemId> }
CommandType::SetFaultInjection { enabled: bool }
CommandType::GetFaultInjectionStatus
```

### Response Status Types

```rust
use satbus::protocol::ResponseStatus;

ResponseStatus::Success              // Command completed successfully
ResponseStatus::Error               // General error
ResponseStatus::InvalidCommand      // Command not recognized
ResponseStatus::SystemBusy         // System busy, try again later
ResponseStatus::SafeModeActive     // System in safe mode
ResponseStatus::Scheduled          // Command scheduled for future execution

// ACK/NACK semantics
ResponseStatus::Acknowledged       // Command received and accepted
ResponseStatus::NegativeAck       // Command rejected
ResponseStatus::ExecutionStarted  // Command execution begun
ResponseStatus::ExecutionFailed   // Command execution failed
ResponseStatus::Timeout           // Command execution timed out
ResponseStatus::InProgress         // Command execution ongoing
```

### Safety Event Types

```rust
use satbus::safety::SafetyEvent;

SafetyEvent::BatteryLow            // Battery voltage below threshold
SafetyEvent::BatteryVoltageUnstable // Battery voltage fluctuating
SafetyEvent::TemperatureHigh       // Temperature above safe limits
SafetyEvent::TemperatureLow        // Temperature below safe limits
SafetyEvent::CommsLinkLost         // Communication link down
SafetyEvent::SystemOverload        // System overloaded
SafetyEvent::WatchdogTimeout       // Watchdog timer expired
SafetyEvent::PowerSystemFailure    // Power subsystem failed
SafetyEvent::ThermalSystemFailure  // Thermal subsystem failed
SafetyEvent::CommsSystemFailure    // Communications subsystem failed
```

### Safety Levels

```rust
use satbus::safety::SafetyLevel;

SafetyLevel::Normal     // All systems nominal
SafetyLevel::Caution    // Minor issues detected
SafetyLevel::Warning    // Significant issues detected
SafetyLevel::Critical   // Critical issues, safe mode may activate
SafetyLevel::Emergency  // Emergency conditions, safe mode active
```

## Error Handling

### Protocol Errors

```rust
use satbus::protocol::ProtocolError;

match protocol_error {
    ProtocolError::InvalidJson => "Malformed JSON in command",
    ProtocolError::MessageTooLarge => "Message exceeds buffer size limits",
    ProtocolError::SerializationError => "Failed to serialize response",
    ProtocolError::InvalidCommand => "Command validation failed",
    ProtocolError::InvalidParameter => "Command parameter out of range",
    ProtocolError::BufferOverflow => "Internal buffer overflow",
}
```

### Subsystem Errors

```rust
// Subsystem operations return Result<(), &'static str>
match power_system.execute_command(command) {
    Ok(()) => println!("Command executed successfully"),
    Err(msg) => println!("Command failed: {}", msg),
}

// Update operations return Result<(), FaultType>
match thermal_system.update(100) {
    Ok(()) => {}, // Normal operation
    Err(fault_type) => println!("Subsystem fault: {:?}", fault_type),
}
```

## Best Practices

### 1. Regular Updates

```rust
// Call update methods regularly in your main loop
let dt_ms = 100; // 100ms update interval

loop {
    let current_time = get_system_time_ms();
    
    // Update satellite agent
    agent.update(current_time);
    
    // Process any pending commands
    let ready_commands = scheduler.get_ready_commands(current_time);
    for command in ready_commands {
        agent.process_command_direct(command);
    }
    
    // Check safety status
    if !agent.is_system_healthy() {
        // Handle safety issues
    }
    
    sleep(dt_ms);
}
```

### 2. Command Validation

```rust
// Always validate commands before execution
fn process_command_safely(handler: &mut ProtocolHandler, cmd_json: &str) -> Result<(), String> {
    let command = handler.parse_command(cmd_json)
        .map_err(|e| format!("Parse error: {}", e))?;
    
    handler.validate_command(&command)
        .map_err(|e| format!("Validation error: {}", e))?;
    
    // Command is safe to execute
    Ok(())
}
```

### 3. Safety Management

```rust
// Check safety actions and respond appropriately
fn handle_safety_actions(actions: &SafetyActions, systems: &mut Systems) {
    if actions.enable_emergency_power_save {
        systems.power.execute_command(PowerCommand::SetPowerSave(true));
    }
    
    if actions.disable_heaters {
        systems.thermal.execute_command(ThermalCommand::SetHeaterState(false));
    }
    
    if actions.enable_survival_mode {
        // Implement survival mode procedures
        systems.enter_survival_mode();
    }
}
```

### 4. Resource Management

```rust
// Clean up resources periodically
fn periodic_cleanup(
    handler: &mut ProtocolHandler,
    scheduler: &mut CommandScheduler,
    safety: &mut SafetyManager,
    current_time: u64
) {
    // Clean up expired command tracking
    handler.cleanup_expired_commands(current_time);
    
    // Clean up expired scheduled commands
    scheduler.cleanup_expired_commands(current_time);
    
    // Clean up resolved safety events
    safety.clear_resolved_events();
}
```

## Configuration Constants

### Buffer Sizes
- `MAX_COMMAND_SIZE`: 512 bytes
- `MAX_RESPONSE_SIZE`: 1024 bytes  
- `MAX_TELEMETRY_SIZE`: 2048 bytes
- `MAX_TRACKED_COMMANDS`: 16 commands
- `MAX_SCHEDULED_COMMANDS`: 32 commands
- `MAX_SAFETY_EVENTS`: 32 events

### Timeouts
- Default command timeout: 3600 seconds (1 hour)
- Watchdog timeout: Configurable
- Update rate: 100ms (configurable)

### Safety Thresholds
- Battery critical: 3200mV
- Battery warning: 3400mV
- Temperature critical high: 75°C
- Temperature critical low: -40°C
- Temperature warning high: 65°C
- Temperature warning low: -30°C

## Examples

See the `/examples` directory for complete usage examples:
- `basic_simulation.rs` - Basic satellite simulation
- `command_processing.rs` - Command processing example
- `safety_management.rs` - Safety system example
- `telemetry_generation.rs` - Telemetry handling example

## Thread Safety

The satellite bus simulator is designed for single-threaded embedded environments. If using in multi-threaded contexts, ensure proper synchronization around mutable operations.

## Memory Usage

The simulator uses statically allocated buffers and heapless collections where possible to ensure predictable memory usage suitable for embedded systems. Total memory usage is bounded and deterministic.