//! # Satellite Bus Simulator
//! 
//! A comprehensive embedded-style satellite bus simulation library providing real-time
//! subsystem management, command processing, telemetry generation, and safety management.
//! 
//! ## Features
//! 
//! - **Real-time subsystem simulation**: Power, thermal, and communications systems
//! - **Command processing**: JSON-based command parsing with ACK/NACK semantics
//! - **Telemetry generation**: Production-grade 2kB telemetry packets
//! - **Safety management**: Fault detection, safe mode, and emergency procedures
//! - **Command scheduling**: Time-tagged command execution
//! - **Embedded-friendly**: No heap allocations, bounded memory usage
//! 
//! ## Quick Start
//! 
//! ```rust
//! use satbus::SatelliteAgent;
//! 
//! // Create satellite agent
//! let mut agent = SatelliteAgent::new();
//! 
//! // Update subsystems and generate telemetry
//! if let Ok(Some(telemetry)) = agent.update() {
//!     println!("Telemetry: {}", telemetry);
//! }
//! 
//! // Process any queued commands
//! if let Err(e) = agent.process_commands() {
//!     println!("Command processing error: {:?}", e);
//! }
//! ```
//! 
//! ## Architecture
//! 
//! The simulator is organized into several key modules:
//! 
//! - [`agent`] - Main orchestrator and public API
//! - [`subsystems`] - Individual subsystem implementations  
//! - [`protocol`] - Command/response protocol handling
//! - [`safety`] - Safety monitoring and safe mode management
//! - [`scheduler`] - Time-tagged command scheduling
//! - [`telemetry`] - Telemetry packet generation
//! 
//! See the [API Reference](API_REFERENCE.md) for detailed usage information.

#![deny(warnings)]
#![deny(clippy::all)]
#![deny(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]

extern crate alloc;

pub mod agent;
pub mod subsystems;
pub mod protocol;
pub mod telemetry;
pub mod fault;
pub mod safety;
pub mod fault_injection;
pub mod scheduler;

// Re-export main public types for convenience
pub use agent::SatelliteAgent;
pub use protocol::{Command, TelemetryPacket};
pub use subsystems::{PowerSystem, ThermalSystem, CommsSystem};