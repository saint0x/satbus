use crate::subsystems::{PowerSystem, ThermalSystem, CommsSystem, Subsystem, SubsystemId};
use heapless::Vec;
use serde::{Deserialize, Serialize};

const MAX_SAFETY_EVENTS: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum SafetyLevel {
    Normal,
    Caution,
    Warning,
    Critical,
    Emergency,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SafetyEvent {
    BatteryLow,
    BatteryVoltageUnstable,
    TemperatureHigh,
    TemperatureLow,
    CommsLinkLost,
    SystemOverload,
    WatchdogTimeout,
    PowerSystemFailure,
    ThermalSystemFailure,
    CommsSystemFailure,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyEventRecord {
    pub event: SafetyEvent,
    pub timestamp: u64,
    pub level: SafetyLevel,
    pub subsystem: SubsystemId,
    pub resolved: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyState {
    pub safe_mode_active: bool,
    pub safety_level: SafetyLevel,
    pub active_events: u8,
    pub watchdog_enabled: bool,
    pub last_watchdog_reset: u64,
    pub safe_mode_entry_count: u32,
    pub system_uptime_safe_s: u64,
    pub manual_override_active: bool,
    pub manual_override_expires: u64,
}

#[derive(Debug)]
pub struct SafetyManager {
    state: SafetyState,
    event_history: Vec<SafetyEventRecord, MAX_SAFETY_EVENTS>,
    watchdog_last_reset: u64,
    safe_mode_entry_time: u64,
    
    // Safety thresholds (compile-time constants for performance)
    battery_critical_mv: u16,
    battery_warning_mv: u16,
    temp_critical_high_c: i8,
    temp_critical_low_c: i8,
    temp_warning_high_c: i8,
    temp_warning_low_c: i8,
    
    // Emergency actions enabled
    #[allow(dead_code)]
    emergency_heater_override: bool,
    #[allow(dead_code)]
    emergency_power_save: bool,
    #[allow(dead_code)]
    emergency_comms_disable: bool,
}

impl SafetyManager {
    pub fn new() -> Self {
        Self {
            state: SafetyState {
                safe_mode_active: false,
                safety_level: SafetyLevel::Normal,
                active_events: 0,
                watchdog_enabled: true,
                last_watchdog_reset: 0,
                safe_mode_entry_count: 0,
                system_uptime_safe_s: 0,
                manual_override_active: false,
                manual_override_expires: 0,
            },
            event_history: Vec::new(),
            watchdog_last_reset: 0,
            safe_mode_entry_time: 0,
            
            // Conservative safety thresholds
            battery_critical_mv: 3200,
            battery_warning_mv: 3400,
            temp_critical_high_c: 75,
            temp_critical_low_c: -40,
            temp_warning_high_c: 65,
            temp_warning_low_c: -30,
            
            emergency_heater_override: false,
            emergency_power_save: false,
            emergency_comms_disable: false,
        }
    }
    
    pub fn update_safety_state(
        &mut self,
        current_time: u64,
        power_system: &PowerSystem,
        thermal_system: &ThermalSystem,
        comms_system: &CommsSystem,
    ) -> SafetyActions {
        let mut actions = SafetyActions::new();
        
        // Reset watchdog
        if self.state.watchdog_enabled {
            self.reset_watchdog(current_time);
        }
        
        // Check subsystem health
        self.check_power_safety(power_system, current_time, &mut actions);
        self.check_thermal_safety(thermal_system, current_time, &mut actions);
        self.check_comms_safety(comms_system, current_time, &mut actions);
        
        // Update overall safety level
        self.update_safety_level();
        
        // Check if manual override has expired
        if self.state.manual_override_active && current_time > self.state.manual_override_expires {
            self.state.manual_override_active = false;
        }
        
        // Determine if safe mode should be active (but respect manual override)
        let should_enter_safe_mode = self.should_enter_safe_mode() && !self.state.manual_override_active;
        
        if should_enter_safe_mode && !self.state.safe_mode_active {
            self.enter_safe_mode(current_time, &mut actions);
        } else if !should_enter_safe_mode && self.state.safe_mode_active {
            self.exit_safe_mode(current_time, &mut actions);
        }
        
        // Update uptime in safe mode
        if self.state.safe_mode_active {
            self.state.system_uptime_safe_s = current_time / 1000;
        }
        
        actions
    }
    
    fn check_power_safety(
        &mut self,
        power_system: &PowerSystem,
        current_time: u64,
        actions: &mut SafetyActions,
    ) {
        let power_state = power_system.get_state();
        
        // Critical battery voltage
        if power_state.battery_voltage_mv < self.battery_critical_mv {
            self.record_event(
                SafetyEvent::BatteryLow,
                current_time,
                SafetyLevel::Critical,
                SubsystemId::Power,
            );
            actions.enable_emergency_power_save = true;
        }
        
        // Warning battery voltage
        else if power_state.battery_voltage_mv < self.battery_warning_mv {
            self.record_event(
                SafetyEvent::BatteryLow,
                current_time,
                SafetyLevel::Warning,
                SubsystemId::Power,
            );
            actions.enable_power_save = true;
        }
        
        // Battery voltage instability
        if power_state.battery_current_ma.abs() > 1000 {
            self.record_event(
                SafetyEvent::BatteryVoltageUnstable,
                current_time,
                SafetyLevel::Caution,
                SubsystemId::Power,
            );
        }
        
        // Power system health
        if !power_system.is_healthy() {
            self.record_event(
                SafetyEvent::PowerSystemFailure,
                current_time,
                SafetyLevel::Critical,
                SubsystemId::Power,
            );
        }
    }
    
    fn check_thermal_safety(
        &mut self,
        thermal_system: &ThermalSystem,
        current_time: u64,
        actions: &mut SafetyActions,
    ) {
        let thermal_state = thermal_system.get_state();
        
        // Critical high temperature
        if thermal_state.core_temp_c > self.temp_critical_high_c {
            self.record_event(
                SafetyEvent::TemperatureHigh,
                current_time,
                SafetyLevel::Critical,
                SubsystemId::Thermal,
            );
            actions.disable_heaters = true;
            actions.enable_emergency_power_save = true;
        }
        
        // Warning high temperature
        else if thermal_state.core_temp_c > self.temp_warning_high_c {
            self.record_event(
                SafetyEvent::TemperatureHigh,
                current_time,
                SafetyLevel::Warning,
                SubsystemId::Thermal,
            );
            actions.disable_heaters = true;
        }
        
        // Critical low temperature
        if thermal_state.core_temp_c < self.temp_critical_low_c {
            self.record_event(
                SafetyEvent::TemperatureLow,
                current_time,
                SafetyLevel::Critical,
                SubsystemId::Thermal,
            );
            actions.enable_emergency_heaters = true;
        }
        
        // Warning low temperature
        else if thermal_state.core_temp_c < self.temp_warning_low_c {
            self.record_event(
                SafetyEvent::TemperatureLow,
                current_time,
                SafetyLevel::Warning,
                SubsystemId::Thermal,
            );
            actions.enable_heaters = true;
        }
        
        // Thermal system health
        if !thermal_system.is_healthy() {
            self.record_event(
                SafetyEvent::ThermalSystemFailure,
                current_time,
                SafetyLevel::Critical,
                SubsystemId::Thermal,
            );
        }
    }
    
    fn check_comms_safety(
        &mut self,
        comms_system: &CommsSystem,
        current_time: u64,
        _actions: &mut SafetyActions,
    ) {
        let comms_state = comms_system.get_state();
        
        // Communications link lost
        if !comms_state.link_up {
            self.record_event(
                SafetyEvent::CommsLinkLost,
                current_time,
                SafetyLevel::Warning,
                SubsystemId::Comms,
            );
        }
        
        // High packet loss
        if comms_state.packet_loss_percent > 50 {
            self.record_event(
                SafetyEvent::CommsLinkLost,
                current_time,
                SafetyLevel::Caution,
                SubsystemId::Comms,
            );
        }
        
        // Comms system health
        if !comms_system.is_healthy() {
            self.record_event(
                SafetyEvent::CommsSystemFailure,
                current_time,
                SafetyLevel::Critical,
                SubsystemId::Comms,
            );
        }
    }
    
    fn should_enter_safe_mode(&self) -> bool {
        let critical_events = self.event_history.iter()
            .filter(|event| !event.resolved && event.level == SafetyLevel::Critical)
            .count();
        
        let emergency_events = self.event_history.iter()
            .filter(|event| !event.resolved && event.level == SafetyLevel::Emergency)
            .count();
        
        critical_events > 0 || emergency_events > 0
    }
    
    fn enter_safe_mode(&mut self, current_time: u64, actions: &mut SafetyActions) {
        self.state.safe_mode_active = true;
        self.state.safe_mode_entry_count = self.state.safe_mode_entry_count.saturating_add(1);
        self.safe_mode_entry_time = current_time;
        
        // Set emergency actions
        actions.enable_emergency_power_save = true;
        actions.disable_non_essential_systems = true;
        actions.enable_survival_mode = true;
        
        self.record_event(
            SafetyEvent::SystemOverload,
            current_time,
            SafetyLevel::Emergency,
            SubsystemId::Power, // Primary subsystem for safe mode
        );
    }
    
    fn exit_safe_mode(&mut self, _current_time: u64, actions: &mut SafetyActions) {
        self.state.safe_mode_active = false;
        
        // Gradual system restoration
        actions.restore_normal_operations = true;
        
        // Resolve ALL unresolved Emergency and Critical events when manually exiting safe mode
        for event in &mut self.event_history {
            if !event.resolved && (event.level == SafetyLevel::Emergency || event.level == SafetyLevel::Critical) {
                event.resolved = true;
            }
        }
        
        // Clear active event count and reset safety level
        self.state.active_events = 0;
        self.state.safety_level = SafetyLevel::Normal;
    }
    
    fn update_safety_level(&mut self) {
        let active_events: alloc::vec::Vec<_> = self.event_history.iter()
            .filter(|event| !event.resolved)
            .collect();
        
        self.state.active_events = active_events.len() as u8;
        
        // Determine highest safety level
        self.state.safety_level = active_events.iter()
            .map(|event| event.level)
            .max()
            .unwrap_or(SafetyLevel::Normal);
    }
    
    fn record_event(
        &mut self,
        event: SafetyEvent,
        timestamp: u64,
        level: SafetyLevel,
        subsystem: SubsystemId,
    ) {
        // Check if this event is already active
        let existing_event = self.event_history.iter_mut()
            .find(|e| e.event == event && e.subsystem == subsystem && !e.resolved);
        
        if existing_event.is_some() {
            // Update existing event timestamp
            if let Some(existing) = existing_event {
                existing.timestamp = timestamp;
                existing.level = level;
            }
            return;
        }
        
        // Create new event record
        let event_record = SafetyEventRecord {
            event,
            timestamp,
            level,
            subsystem,
            resolved: false,
        };
        
        // Add to history (circular buffer)
        if self.event_history.is_full() {
            self.event_history.remove(0);
        }
        
        let _ = self.event_history.push(event_record);
    }
    
    fn reset_watchdog(&mut self, current_time: u64) {
        self.watchdog_last_reset = current_time;
        self.state.last_watchdog_reset = current_time;
    }
    
    pub fn get_state(&self) -> &SafetyState {
        &self.state
    }
    
    pub fn get_event_history(&self) -> &[SafetyEventRecord] {
        &self.event_history
    }
    
    pub fn clear_resolved_events(&mut self) {
        self.event_history.retain(|event| !event.resolved);
    }
    
    pub fn force_safe_mode(&mut self, current_time: u64) -> SafetyActions {
        let mut actions = SafetyActions::new();
        if !self.state.safe_mode_active {
            self.enter_safe_mode(current_time, &mut actions);
        }
        actions
    }
    
    pub fn disable_safe_mode(&mut self, current_time: u64) -> SafetyActions {
        let mut actions = SafetyActions::new();
        if self.state.safe_mode_active {
            self.exit_safe_mode(current_time, &mut actions);
        }
        
        // Set manual override for 10 minutes (600 seconds) to prevent immediate re-entry
        self.state.manual_override_active = true;
        self.state.manual_override_expires = current_time + 600_000; // 10 minutes in milliseconds
        
        actions
    }
    
    /// Clear safety events for ground testing - USE WITH EXTREME CAUTION
    /// This is a ground testing override that should NEVER be used in flight
    pub fn clear_safety_events(&mut self, force: bool) -> Result<(), alloc::string::String> {
        if !force {
            return Err("Safety event clearing requires force=true for safety".into());
        }
        
        // Clear all unresolved events
        for event in &mut self.event_history {
            if !event.resolved {
                event.resolved = true;
            }
        }
        
        // Reset safety state to normal
        self.state.safety_level = SafetyLevel::Normal;
        self.state.active_events = 0;
        
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct SafetyActions {
    pub enable_power_save: bool,
    pub enable_emergency_power_save: bool,
    pub enable_heaters: bool,
    pub enable_emergency_heaters: bool,
    pub disable_heaters: bool,
    pub disable_non_essential_systems: bool,
    pub enable_survival_mode: bool,
    pub restore_normal_operations: bool,
}

impl SafetyActions {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn has_actions(&self) -> bool {
        self.enable_power_save ||
        self.enable_emergency_power_save ||
        self.enable_heaters ||
        self.enable_emergency_heaters ||
        self.disable_heaters ||
        self.disable_non_essential_systems ||
        self.enable_survival_mode ||
        self.restore_normal_operations
    }
}