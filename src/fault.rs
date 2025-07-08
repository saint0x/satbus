use crate::subsystems::{SubsystemId, FaultType};
use heapless::Vec;
use serde::{Deserialize, Serialize};

const MAX_FAULT_HISTORY: usize = 64;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaultRecord {
    pub id: u32,
    pub subsystem: SubsystemId,
    pub fault_type: FaultType,
    pub timestamp: u64,
    pub duration_ms: u32,
    pub resolved: bool,
    pub recovery_attempts: u8,
}

#[derive(Debug)]
pub struct FaultManager {
    fault_history: Vec<FaultRecord, MAX_FAULT_HISTORY>,
    next_fault_id: u32,
}

impl FaultManager {
    pub fn new() -> Self {
        Self {
            fault_history: Vec::new(),
            next_fault_id: 1,
        }
    }
    
    pub fn record_fault(
        &mut self,
        subsystem: SubsystemId,
        fault_type: FaultType,
        timestamp: u64,
    ) -> u32 {
        let fault_id = self.next_fault_id;
        self.next_fault_id = self.next_fault_id.wrapping_add(1);
        
        let fault_record = FaultRecord {
            id: fault_id,
            subsystem,
            fault_type,
            timestamp,
            duration_ms: 0,
            resolved: false,
            recovery_attempts: 0,
        };
        
        if self.fault_history.is_full() {
            self.fault_history.remove(0);
        }
        
        let _ = self.fault_history.push(fault_record);
        fault_id
    }
    
    pub fn resolve_fault(&mut self, fault_id: u32, timestamp: u64) -> bool {
        if let Some(fault) = self.fault_history.iter_mut().find(|f| f.id == fault_id) {
            fault.resolved = true;
            fault.duration_ms = (timestamp - fault.timestamp) as u32;
            true
        } else {
            false
        }
    }
    
    pub fn get_active_faults(&self) -> impl Iterator<Item = &FaultRecord> {
        self.fault_history.iter().filter(|f| !f.resolved)
    }
    
    pub fn get_fault_history(&self) -> &[FaultRecord] {
        &self.fault_history
    }
    
    pub fn clear_resolved_faults(&mut self) {
        self.fault_history.retain(|f| !f.resolved);
    }
}