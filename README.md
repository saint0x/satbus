# SatBus - Satellite Bus Simulator

A satellite bus simulator with comprehensive subsystem modeling, command scheduling, safety management, and embedded-style architecture.

## What This Is

SatBus simulates the core command and telemetry loops of a satellite bus platform. It models three critical subsystems (power, thermal, communications) with realistic state management, fault injection, and safety protocols.

**Technical Architecture:**
- Rust-based subsystem modeling with deterministic state updates
- JSON protocol with ACK/NACK command semantics
- Time-tagged command scheduling with chronological execution
- Real-time telemetry generation with 2kB packet sizing
- Comprehensive safety management with fault detection and safe-mode logic
- Embedded-friendly design: no heap allocations, bounded memory usage, statically allocated buffers

## Real-World Relevance

This simulator mirrors the command and data handling systems used in actual satellite development:

- **Command/Telemetry Protocol**: Models production ACK/NACK semantics with command tracking and timeout handling
- **Time-Tagged Execution**: Implements spacecraft command scheduling with chronological ordering and validation
- **Subsystem Health Management**: Power, thermal, and communications interdependencies with realistic state modeling
- **Safety Management**: Fault detection, escalation thresholds, and automated safe-mode entry/exit procedures
- **Embedded Constraints**: Memory-bounded design suitable for flight computer validation and ground system integration

The architecture reflects the operational patterns and resource constraints found in real spacecraft bus systems while providing a comprehensive environment for testing mission operations software and procedures.

## Installation

### Build from Source

```bash
git clone <repository-url>
cd satbus
cargo build --release
```

### Run Tests

```bash
cargo test  # Run comprehensive test suite (85+ tests)
```

### Add to PATH (Optional)

```bash
# Add to your shell profile (.bashrc, .zshrc, etc.)
export PATH="$PATH:/path/to/satbus/target/release"
```

## Usage

### Library API

```rust
use satbus::SatelliteAgent;

// Create satellite agent
let mut agent = SatelliteAgent::new();

// Update subsystems and generate telemetry
if let Ok(Some(telemetry)) = agent.update() {
    println!("Telemetry: {}", telemetry);
}

// Process any queued commands
if let Err(e) = agent.process_commands() {
    println!("Command processing error: {:?}", e);
}
```

### Command Line Interface

#### Start the Simulator Server

```bash
cargo run --bin satbus -- server
# or if in PATH: satbus server
```

#### Basic Operations
```bash
satbus ping                    # Test connection
satbus status                  # System status
satbus monitor                 # Live telemetry stream
```

#### Power Management
```bash
satbus power solar on          # Enable solar panels
satbus power tx-power 20       # Set transmitter power (0-30 dBm)
satbus power save-mode on      # Enable power save mode
```

#### Thermal Control
```bash
satbus thermal heater on       # Enable heaters
```

#### Communications
```bash
satbus comms link up           # Bring communications link up
satbus comms transmit "hello"  # Transmit message
```

#### System Management
```bash
satbus system fault power degraded    # Inject power fault
satbus system clear-faults            # Clear all faults
satbus system safe-mode on            # Enable safe mode
satbus system reboot --confirm        # System reboot
```

#### Options
```bash
--host <HOST>          # Simulator host (default: 127.0.0.1)
--port <PORT>          # Simulator port (default: 8081)
--format <FORMAT>      # Output format: table, json, compact
--verbose              # Verbose output
```

## Documentation

- **[API Reference](API_REFERENCE.md)**: Complete API documentation with examples
- **Inline Documentation**: Comprehensive rustdoc comments throughout codebase
- **Test Examples**: Extensive test suite demonstrating all features

## Testing

```bash
cargo test                    # Run comprehensive test suite (85+ tests)
cargo test --test integration # Integration tests
cargo test --test protocol   # Protocol handler tests  
cargo test --test safety     # Safety manager tests
cargo test --doc             # Documentation tests
```

## License

MIT