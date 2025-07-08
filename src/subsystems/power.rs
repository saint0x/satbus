use super::{Subsystem, FaultType};
use serde::{Deserialize, Serialize};

const NOMINAL_VOLTAGE: u16 = 3700;
const CRITICAL_VOLTAGE: u16 = 3200;
const MAX_VOLTAGE: u16 = 4200;
const VOLTAGE_TOLERANCE: u16 = 50;

const NOMINAL_CURRENT_MA: u16 = 500;
const SOLAR_CURRENT_MA: u16 = 800;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PowerState {
    pub battery_voltage_mv: u16,
    pub battery_current_ma: i16,
    pub solar_voltage_mv: u16,
    pub solar_current_ma: u16,
    pub charging: bool,
    pub battery_level_percent: u8,
    pub power_draw_mw: u16,
    // Removed uptime_seconds - redundant with SystemState
}

#[derive(Debug, Clone)]
pub enum PowerCommand {
    SetSolarPanel(bool),
    SetPowerSave(bool),
    Reboot,
}

#[derive(Debug)]
pub struct PowerSystem {
    state: PowerState,
    solar_enabled: bool,
    power_save_mode: bool,
    fault_state: Option<FaultType>,
    internal_resistance_mohm: u16,
    
    // Preallocated state for calculations
    #[allow(dead_code)]
    last_update_ms: u32,
}

impl PowerSystem {
    pub fn new() -> Self {
        Self {
            state: PowerState {
                battery_voltage_mv: NOMINAL_VOLTAGE,
                battery_current_ma: -(NOMINAL_CURRENT_MA as i16),
                solar_voltage_mv: 0,
                solar_current_ma: 0,
                charging: false,
                battery_level_percent: 85,
                power_draw_mw: (NOMINAL_VOLTAGE as u32 * NOMINAL_CURRENT_MA as u32 / 1000) as u16,
            },
            solar_enabled: true,
            power_save_mode: false,
            fault_state: None,
            internal_resistance_mohm: 100,
            last_update_ms: 0,
        }
    }
    
    fn calculate_battery_level(&self) -> u8 {
        let voltage_range = MAX_VOLTAGE - CRITICAL_VOLTAGE;
        let current_range = self.state.battery_voltage_mv.saturating_sub(CRITICAL_VOLTAGE);
        
        ((current_range as u32 * 100) / voltage_range as u32).min(100) as u8
    }
    
    fn simulate_solar_input(&mut self, _dt_ms: u16) {
        if !self.solar_enabled {
            self.state.solar_voltage_mv = 0;
            self.state.solar_current_ma = 0;
            return;
        }
        
        // Simulate solar panel efficiency based on orbital position
        let time_factor = (self.last_update_ms as f32 * 0.001).sin().abs();
        let solar_efficiency = 0.7 + 0.3 * time_factor;
        
        self.state.solar_voltage_mv = (4200.0 * solar_efficiency) as u16;
        self.state.solar_current_ma = (SOLAR_CURRENT_MA as f32 * solar_efficiency) as u16;
    }
    
    fn update_battery_state(&mut self, dt_ms: u16) -> Result<(), FaultType> {
        let dt_s = dt_ms as f32 / 1000.0;
        
        // Calculate net current
        let load_current = if self.power_save_mode {
            NOMINAL_CURRENT_MA / 2
        } else {
            NOMINAL_CURRENT_MA
        };
        
        let net_current = self.state.solar_current_ma as i16 - load_current as i16;
        self.state.battery_current_ma = net_current;
        
        // Update charging state
        self.state.charging = net_current > 0;
        
        // Simulate battery voltage based on current flow
        let voltage_delta = (net_current as f32 * self.internal_resistance_mohm as f32 / 1000.0) as i16;
        let target_voltage = (NOMINAL_VOLTAGE as i16 + voltage_delta).max(0) as u16;
        
        // Smooth voltage transition
        let voltage_diff = target_voltage as i16 - self.state.battery_voltage_mv as i16;
        let voltage_change = (voltage_diff as f32 * dt_s * 0.1) as i16;
        
        self.state.battery_voltage_mv = 
            (self.state.battery_voltage_mv as i16 + voltage_change)
            .max(0)
            .min(MAX_VOLTAGE as i16) as u16;
        
        // Update battery level
        self.state.battery_level_percent = self.calculate_battery_level();
        
        // NASA Rule 5: Safety assertions for invariants
        debug_assert!(
            self.state.battery_voltage_mv <= MAX_VOLTAGE,
            "Battery voltage {} exceeds maximum {}", 
            self.state.battery_voltage_mv, MAX_VOLTAGE
        );
        debug_assert!(
            self.state.battery_voltage_mv >= CRITICAL_VOLTAGE,
            "Battery voltage {} below critical {}", 
            self.state.battery_voltage_mv, CRITICAL_VOLTAGE
        );
        debug_assert!(
            self.state.battery_level_percent <= 100,
            "Battery level {} exceeds 100%", 
            self.state.battery_level_percent
        );
        debug_assert!(
            self.state.solar_current_ma <= SOLAR_CURRENT_MA,
            "Solar current {} exceeds maximum {}", 
            self.state.solar_current_ma, SOLAR_CURRENT_MA
        );
        
        // Update power draw
        self.state.power_draw_mw = 
            (self.state.battery_voltage_mv as u32 * load_current as u32 / 1000) as u16;
        
        // Check critical voltage
        if self.state.battery_voltage_mv < CRITICAL_VOLTAGE {
            return Err(FaultType::Failed);
        }
        
        // Check for voltage instability
        if self.state.battery_voltage_mv > MAX_VOLTAGE + VOLTAGE_TOLERANCE {
            return Err(FaultType::Degraded);
        }
        
        Ok(())
    }
}

impl Subsystem for PowerSystem {
    type State = PowerState;
    type Command = PowerCommand;
    
    fn update(&mut self, dt_ms: u16) -> Result<(), FaultType> {
        if let Some(fault) = self.fault_state {
            match fault {
                FaultType::Failed => return Err(fault),
                FaultType::Degraded => {
                    // Continue with degraded performance
                    self.internal_resistance_mohm = 200;
                }
                FaultType::Offline => return Err(fault),
            }
        }
        
        // uptime_seconds removed - tracked at system level
        
        self.simulate_solar_input(dt_ms);
        self.update_battery_state(dt_ms)?;
        
        Ok(())
    }
    
    fn execute_command(&mut self, command: Self::Command) -> Result<(), &'static str> {
        match command {
            PowerCommand::SetSolarPanel(enabled) => {
                self.solar_enabled = enabled;
                Ok(())
            }
            PowerCommand::SetPowerSave(enabled) => {
                self.power_save_mode = enabled;
                Ok(())
            }
            PowerCommand::Reboot => {
                // uptime_seconds removed - tracked at system level
                self.fault_state = None;
                Ok(())
            }
        }
    }
    
    fn get_state(&self) -> Self::State {
        self.state.clone()
    }
    
    fn inject_fault(&mut self, fault: FaultType) {
        self.fault_state = Some(fault);
    }
    
    fn clear_faults(&mut self) {
        self.fault_state = None;
        self.internal_resistance_mohm = 100;
    }
    
    fn is_healthy(&self) -> bool {
        self.fault_state.is_none() && 
        self.state.battery_voltage_mv >= CRITICAL_VOLTAGE &&
        self.state.battery_level_percent > 10
    }
}