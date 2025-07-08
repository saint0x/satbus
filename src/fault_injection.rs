use crate::subsystems::{SubsystemId, FaultType, Fault};
use heapless::Vec;
use serde::{Deserialize, Serialize};

const MAX_ACTIVE_FAULTS: usize = 8;

// Per-subsystem fault rates based on real satellite data
const POWER_FAULT_RATE_PERCENT: f32 = 0.3;   // Power systems are generally reliable
const THERMAL_FAULT_RATE_PERCENT: f32 = 0.5; // Thermal systems have moderate complexity
const COMMS_FAULT_RATE_PERCENT: f32 = 0.7;   // Communications systems are most complex

// Fault type probability weights (must sum to 100)
const DEGRADED_WEIGHT: u8 = 70;  // 70% - Most common, temporary performance issues
const FAILED_WEIGHT: u8 = 25;    // 25% - Serious issues requiring intervention
const OFFLINE_WEIGHT: u8 = 5;    // 5% - Complete subsystem failure

// Fault duration constants (in seconds)
const MIN_FAULT_DURATION_S: u32 = 10;
const MAX_FAULT_DURATION_S: u32 = 60;
const PERMANENT_FAULT_PROBABILITY: f32 = 0.2; // 20% of faults require manual clearing

/// Active fault tracking for duration and recovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveFault {
    pub fault: Fault,
    pub duration_remaining_s: u32,
    pub auto_recoverable: bool,
    pub injected_at_cycle: u64,
}

/// Fault injection statistics for telemetry
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FaultInjectionStats {
    pub total_faults_injected: u32,
    pub power_faults_injected: u32,
    pub thermal_faults_injected: u32,
    pub comms_faults_injected: u32,
    pub degraded_faults: u32,
    pub failed_faults: u32,
    pub offline_faults: u32,
    pub auto_recovered_faults: u32,
    pub manual_cleared_faults: u32,
    pub current_active_faults: u8,
}

/// Configuration for fault injection behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaultInjectionConfig {
    pub enabled: bool,
    pub power_rate_percent: f32,
    pub thermal_rate_percent: f32,
    pub comms_rate_percent: f32,
    pub degraded_weight: u8,
    pub failed_weight: u8,
    pub offline_weight: u8,
    pub min_duration_s: u32,
    pub max_duration_s: u32,
    pub permanent_probability: f32,
}

impl Default for FaultInjectionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            power_rate_percent: POWER_FAULT_RATE_PERCENT,
            thermal_rate_percent: THERMAL_FAULT_RATE_PERCENT,
            comms_rate_percent: COMMS_FAULT_RATE_PERCENT,
            degraded_weight: DEGRADED_WEIGHT,
            failed_weight: FAILED_WEIGHT,
            offline_weight: OFFLINE_WEIGHT,
            min_duration_s: MIN_FAULT_DURATION_S,
            max_duration_s: MAX_FAULT_DURATION_S,
            permanent_probability: PERMANENT_FAULT_PROBABILITY,
        }
    }
}

/// Probabilistic fault injection engine
#[derive(Debug)]
pub struct FaultInjector {
    config: FaultInjectionConfig,
    active_faults: Vec<ActiveFault, MAX_ACTIVE_FAULTS>,
    stats: FaultInjectionStats,
    cycle_count: u64,
    
    // Simple Linear Congruential Generator for deterministic testing
    rng_state: u64,
}

impl FaultInjector {
    pub fn new() -> Self {
        Self {
            config: FaultInjectionConfig::default(),
            active_faults: Vec::new(),
            stats: FaultInjectionStats::default(),
            cycle_count: 0,
            rng_state: 0x1234_5678_9ABC_DEF0, // Fixed seed for deterministic behavior
        }
    }
    
    pub fn new_with_config(config: FaultInjectionConfig) -> Self {
        Self {
            config,
            active_faults: Vec::new(),
            stats: FaultInjectionStats::default(),
            cycle_count: 0,
            rng_state: 0x1234_5678_9ABC_DEF0,
        }
    }
    
    /// Update fault injection engine - call once per simulation cycle
    pub fn update(&mut self, current_time: u64) -> Vec<(SubsystemId, Option<FaultType>), 8> {
        if !self.config.enabled {
            return Vec::new();
        }
        
        self.cycle_count += 1;
        let mut actions = Vec::new();
        
        // Update active faults and handle recovery
        self.update_active_faults(current_time, &mut actions);
        
        // Attempt to inject new faults
        self.attempt_fault_injection(current_time, &mut actions);
        
        // Update statistics
        self.stats.current_active_faults = self.active_faults.len() as u8;
        
        actions
    }
    
    /// Update active faults and handle automatic recovery
    fn update_active_faults(&mut self, _current_time: u64, actions: &mut Vec<(SubsystemId, Option<FaultType>), 8>) {
        let mut recovered_faults: Vec<usize, 8> = Vec::new();
        
        for (index, active_fault) in self.active_faults.iter_mut().enumerate() {
            if active_fault.auto_recoverable {
                if active_fault.duration_remaining_s > 0 {
                    active_fault.duration_remaining_s = active_fault.duration_remaining_s.saturating_sub(1);
                } else {
                    // Fault has expired, schedule for recovery
                    let _ = recovered_faults.push(index);
                    if actions.push((active_fault.fault.subsystem, None)).is_err() {
                        // Actions buffer full, will retry next cycle
                        break;
                    }
                }
            }
        }
        
        // Remove recovered faults in reverse order to maintain indices
        for &index in recovered_faults.iter().rev() {
            self.active_faults.swap_remove(index);
            self.stats.auto_recovered_faults += 1;
        }
    }
    
    /// Attempt to inject new faults based on probability
    fn attempt_fault_injection(&mut self, current_time: u64, actions: &mut Vec<(SubsystemId, Option<FaultType>), 8>) {
        let subsystems = [
            (SubsystemId::Power, self.config.power_rate_percent),
            (SubsystemId::Thermal, self.config.thermal_rate_percent),
            (SubsystemId::Comms, self.config.comms_rate_percent),
        ];
        
        for (subsystem_id, rate_percent) in subsystems {
            // Skip if this subsystem already has an active fault
            if self.active_faults.iter().any(|f| f.fault.subsystem == subsystem_id) {
                continue;
            }
            
            // Check if we should inject a fault
            if self.should_inject_fault(rate_percent) {
                if let Some(fault_type) = self.select_fault_type() {
                    let fault = Fault {
                        subsystem: subsystem_id,
                        fault_type,
                        timestamp: current_time,
                    };
                    
                    let duration = if self.random_float() < self.config.permanent_probability {
                        // Permanent fault - requires manual clearing
                        u32::MAX
                    } else {
                        // Temporary fault with random duration
                        self.random_duration()
                    };
                    
                    let active_fault = ActiveFault {
                        fault,
                        duration_remaining_s: duration,
                        auto_recoverable: duration != u32::MAX,
                        injected_at_cycle: self.cycle_count,
                    };
                    
                    // Add to active faults list
                    if self.active_faults.push(active_fault).is_ok() {
                        // Schedule fault injection
                        if actions.push((subsystem_id, Some(fault_type))).is_ok() {
                            self.update_injection_stats(subsystem_id, fault_type);
                        } else {
                            // Actions buffer full, remove the fault we just added
                            self.active_faults.pop();
                        }
                    }
                }
            }
        }
    }
    
    /// Determine if a fault should be injected based on probability
    fn should_inject_fault(&mut self, rate_percent: f32) -> bool {
        let random_value = self.random_float();
        random_value < (rate_percent / 100.0)
    }
    
    /// Select fault type based on weighted probabilities
    fn select_fault_type(&mut self) -> Option<FaultType> {
        let random_value = self.random_u8();
        let total_weight = self.config.degraded_weight + self.config.failed_weight + self.config.offline_weight;
        
        if total_weight == 0 {
            return None;
        }
        
        let normalized_value = (random_value as u16 * total_weight as u16 / 255) as u8;
        
        if normalized_value < self.config.degraded_weight {
            Some(FaultType::Degraded)
        } else if normalized_value < self.config.degraded_weight + self.config.failed_weight {
            Some(FaultType::Failed)
        } else {
            Some(FaultType::Offline)
        }
    }
    
    /// Generate random fault duration
    fn random_duration(&mut self) -> u32 {
        let range = self.config.max_duration_s - self.config.min_duration_s;
        if range == 0 {
            return self.config.min_duration_s;
        }
        
        let random_offset = self.random_u32() % range;
        self.config.min_duration_s + random_offset
    }
    
    /// Update statistics when a fault is injected
    fn update_injection_stats(&mut self, subsystem: SubsystemId, fault_type: FaultType) {
        self.stats.total_faults_injected += 1;
        
        match subsystem {
            SubsystemId::Power => self.stats.power_faults_injected += 1,
            SubsystemId::Thermal => self.stats.thermal_faults_injected += 1,
            SubsystemId::Comms => self.stats.comms_faults_injected += 1,
        }
        
        match fault_type {
            FaultType::Degraded => self.stats.degraded_faults += 1,
            FaultType::Failed => self.stats.failed_faults += 1,
            FaultType::Offline => self.stats.offline_faults += 1,
        }
    }
    
    /// Manual fault clearing (called when ClearFaults command is received)
    pub fn clear_faults(&mut self, subsystem: Option<SubsystemId>) {
        let initial_count = self.active_faults.len();
        
        match subsystem {
            Some(target_subsystem) => {
                self.active_faults.retain(|fault| fault.fault.subsystem != target_subsystem);
            }
            None => {
                self.active_faults.clear();
            }
        }
        
        let cleared_count = initial_count - self.active_faults.len();
        self.stats.manual_cleared_faults += cleared_count as u32;
    }
    
    /// Get current fault injection statistics
    pub fn get_stats(&self) -> &FaultInjectionStats {
        &self.stats
    }
    
    /// Get current configuration
    pub fn get_config(&self) -> &FaultInjectionConfig {
        &self.config
    }
    
    /// Update configuration
    pub fn update_config(&mut self, config: FaultInjectionConfig) {
        self.config = config;
    }
    
    /// Enable/disable fault injection
    pub fn set_enabled(&mut self, enabled: bool) {
        self.config.enabled = enabled;
    }
    
    /// Get active faults for telemetry
    pub fn get_active_faults(&self) -> &[ActiveFault] {
        &self.active_faults
    }
    
    // Simple PRNG methods for deterministic testing
    fn next_random(&mut self) -> u64 {
        // Linear Congruential Generator: X(n+1) = (aX(n) + c) mod m
        // Using parameters from Numerical Recipes
        self.rng_state = self.rng_state.wrapping_mul(1664525).wrapping_add(1013904223);
        self.rng_state
    }
    
    fn random_u8(&mut self) -> u8 {
        (self.next_random() >> 24) as u8
    }
    
    fn random_u32(&mut self) -> u32 {
        self.next_random() as u32
    }
    
    fn random_float(&mut self) -> f32 {
        (self.next_random() as f32) / (u64::MAX as f32)
    }
}

impl Default for FaultInjector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_fault_injector_creation() {
        let injector = FaultInjector::new();
        assert!(injector.config.enabled);
        assert_eq!(injector.active_faults.len(), 0);
        assert_eq!(injector.stats.total_faults_injected, 0);
    }
    
    #[test]
    fn test_fault_injection_disabled() {
        let mut config = FaultInjectionConfig::default();
        config.enabled = false;
        let mut injector = FaultInjector::new_with_config(config);
        
        let actions = injector.update(1000);
        assert_eq!(actions.len(), 0);
    }
    
    #[test]
    fn test_fault_type_selection() {
        let mut injector = FaultInjector::new();
        
        // Test multiple selections to ensure all types can be selected
        let mut degraded_count = 0;
        let mut failed_count = 0;
        let mut offline_count = 0;
        
        for _ in 0..1000 {
            if let Some(fault_type) = injector.select_fault_type() {
                match fault_type {
                    FaultType::Degraded => degraded_count += 1,
                    FaultType::Failed => failed_count += 1,
                    FaultType::Offline => offline_count += 1,
                }
            }
        }
        
        // Should have selected some of each type (probabilistic test)
        assert!(degraded_count > 0);
        assert!(failed_count > 0);
        assert!(offline_count > 0);
        
        // Degraded should be most common
        assert!(degraded_count > failed_count);
        assert!(degraded_count > offline_count);
    }
    
    #[test]
    fn test_manual_fault_clearing() {
        let mut injector = FaultInjector::new();
        
        // Manually add some active faults
        let fault1 = ActiveFault {
            fault: Fault {
                subsystem: SubsystemId::Power,
                fault_type: FaultType::Degraded,
                timestamp: 1000,
            },
            duration_remaining_s: 30,
            auto_recoverable: true,
            injected_at_cycle: 1,
        };
        
        let fault2 = ActiveFault {
            fault: Fault {
                subsystem: SubsystemId::Thermal,
                fault_type: FaultType::Failed,
                timestamp: 2000,
            },
            duration_remaining_s: u32::MAX,
            auto_recoverable: false,
            injected_at_cycle: 2,
        };
        
        injector.active_faults.push(fault1).unwrap();
        injector.active_faults.push(fault2).unwrap();
        
        // Clear power faults only
        injector.clear_faults(Some(SubsystemId::Power));
        assert_eq!(injector.active_faults.len(), 1);
        assert_eq!(injector.active_faults[0].fault.subsystem, SubsystemId::Thermal);
        
        // Clear all faults
        injector.clear_faults(None);
        assert_eq!(injector.active_faults.len(), 0);
        assert_eq!(injector.stats.manual_cleared_faults, 2);
    }
    
    #[test]
    fn test_random_number_generation() {
        let mut injector = FaultInjector::new();
        
        // Test that RNG produces different values
        let val1 = injector.random_u8();
        let val2 = injector.random_u8();
        let val3 = injector.random_u8();
        
        // Very unlikely to get three identical values
        assert!(val1 != val2 || val2 != val3);
        
        // Test float range
        let float_val = injector.random_float();
        assert!(float_val >= 0.0 && float_val <= 1.0);
    }
}