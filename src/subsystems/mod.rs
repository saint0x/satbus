pub mod power;
pub mod thermal;
pub mod comms;

pub use power::{PowerSystem, PowerState};
pub use thermal::{ThermalSystem, ThermalState};
pub use comms::{CommsSystem, CommsState};

use heapless::Vec;
use serde::{Deserialize, Serialize};

pub const MAX_SUBSYSTEMS: usize = 8;
pub const MAX_FAULTS: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubsystemId {
    Power,
    Thermal,
    Comms,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FaultType {
    Degraded,
    Failed,
    Offline,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Fault {
    pub subsystem: SubsystemId,
    pub fault_type: FaultType,
    pub timestamp: u64,
}

pub type FaultList = Vec<Fault, MAX_FAULTS>;

pub trait Subsystem {
    type State: Clone + Serialize;
    type Command: Clone;
    
    fn update(&mut self, dt_ms: u16) -> Result<(), FaultType>;
    fn execute_command(&mut self, command: Self::Command) -> Result<(), &'static str>;
    fn get_state(&self) -> Self::State;
    fn inject_fault(&mut self, fault: FaultType);
    fn clear_faults(&mut self);
    fn is_healthy(&self) -> bool;
}