use super::{Subsystem, FaultType};
use serde::{Deserialize, Serialize};

const NOMINAL_TEMP_C: i8 = 20;
const CRITICAL_TEMP_HIGH_C: i8 = 75;
const CRITICAL_TEMP_LOW_C: i8 = -40;
const HEATER_POWER_W: u16 = 50;
const THERMAL_MASS_J_PER_K: f32 = 2000.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThermalState {
    pub core_temp_c: i8,
    pub battery_temp_c: i8,
    pub solar_panel_temp_c: i8,
    pub heater_power_w: u16,         // 0=off, >0=power (merged heaters_on)
    pub power_dissipation_w: u16,
    // Removed thermal_gradient_c_per_min - can calculate from temp deltas
    // Removed heaters_on - encoded in heater_power_w (0=off)
}

#[derive(Debug, Clone)]
pub enum ThermalCommand {
    SetHeaterState(bool),
    SetThermalMode(ThermalMode),
    CalibrateTemp(i8),
}

#[derive(Debug, Clone, Copy)]
pub enum ThermalMode {
    Nominal,
    Survival,
    PowerSave,
}

#[derive(Debug)]
pub struct ThermalSystem {
    state: ThermalState,
    thermal_mode: ThermalMode,
    fault_state: Option<FaultType>,
    ambient_temp_c: i8,
    thermal_conductivity: f32,
    
    // Preallocated calculation buffers
    temp_history: [i8; 16],
    history_index: usize,
}

impl ThermalSystem {
    pub fn new() -> Self {
        Self {
            state: ThermalState {
                core_temp_c: NOMINAL_TEMP_C,
                battery_temp_c: NOMINAL_TEMP_C + 5,
                solar_panel_temp_c: NOMINAL_TEMP_C - 10,
                heater_power_w: 0,  // 0=off (merged heaters_on)
                power_dissipation_w: 25,
            },
            thermal_mode: ThermalMode::Nominal,
            fault_state: None,
            ambient_temp_c: -20,
            thermal_conductivity: 0.95,
            temp_history: [NOMINAL_TEMP_C; 16],
            history_index: 0,
        }
    }
    
    fn calculate_thermal_gradient(&self) -> f32 {
        let temp_diff = self.state.core_temp_c - self.ambient_temp_c;
        temp_diff as f32 * self.thermal_conductivity
    }
    
    fn update_ambient_temperature(&mut self, uptime_s: u32) {
        // Simulate orbital thermal cycling (90-minute orbit)
        let orbital_phase = (uptime_s as f32 / 5400.0) * 2.0 * core::f32::consts::PI;
        let solar_exposure = orbital_phase.cos();
        
        // Space environment: -150°C to +120°C
        let space_temp = -150.0 + (solar_exposure + 1.0) * 135.0;
        self.ambient_temp_c = space_temp as i8;
    }
    
    fn simulate_thermal_dynamics(&mut self, dt_ms: u16) -> Result<(), FaultType> {
        let dt_s = dt_ms as f32 / 1000.0;
        
        // Calculate heat sources
        let internal_heat_w = self.state.power_dissipation_w as f32;
        let heater_heat_w = if self.state.heater_power_w > 0 {
            match self.thermal_mode {
                ThermalMode::Nominal => self.state.heater_power_w as f32,
                ThermalMode::Survival => self.state.heater_power_w as f32 * 0.5,
                ThermalMode::PowerSave => self.state.heater_power_w as f32 * 0.25,
            }
        } else {
            0.0
        };
        
        // Calculate heat loss to space
        let thermal_gradient = self.calculate_thermal_gradient();
        let heat_loss_w = thermal_gradient * 10.0; // Simplified Stefan-Boltzmann approximation
        
        // Net heat flow
        let net_heat_w = internal_heat_w + heater_heat_w - heat_loss_w;
        
        // Temperature change (dT = Q * dt / (m * c))
        let temp_change_c = net_heat_w * dt_s / THERMAL_MASS_J_PER_K;
        
        // Update core temperature
        let new_core_temp = self.state.core_temp_c as f32 + temp_change_c;
        self.state.core_temp_c = new_core_temp.round() as i8;
        
        // Update thermal gradient
        // Thermal gradient removed for size optimization - can calculate from temp deltas
        
        // Update component temperatures with thermal lag
        self.state.battery_temp_c = self.state.core_temp_c.saturating_add(
            (self.state.power_dissipation_w as f32 * 0.1) as i8);
        self.state.solar_panel_temp_c = self.ambient_temp_c.saturating_add(
            (self.ambient_temp_c - self.state.core_temp_c) / 3);
        
        // heater_power_w already encodes on/off state (0=off, >0=on)
        
        // Update temperature history
        self.temp_history[self.history_index] = self.state.core_temp_c;
        self.history_index = (self.history_index + 1) % self.temp_history.len();
        
        // NASA Rule 5: Safety assertions for thermal invariants
        debug_assert!(
            self.state.core_temp_c > -60,
            "Core temperature {} below absolute minimum", 
            self.state.core_temp_c
        );
        debug_assert!(
            self.state.core_temp_c < 100,
            "Core temperature {} above absolute maximum", 
            self.state.core_temp_c
        );
        debug_assert!(
            self.state.battery_temp_c > -60,
            "Battery temperature {} below absolute minimum", 
            self.state.battery_temp_c
        );
        debug_assert!(
            self.state.battery_temp_c < 100,
            "Battery temperature {} above absolute maximum", 
            self.state.battery_temp_c
        );
        debug_assert!(
            self.state.heater_power_w <= 50,
            "Heater power {} exceeds maximum 50W", 
            self.state.heater_power_w
        );
        debug_assert!(
            self.history_index < self.temp_history.len(),
            "Temperature history index {} out of bounds", 
            self.history_index
        );
        
        // Check thermal limits
        if self.state.core_temp_c > CRITICAL_TEMP_HIGH_C {
            return Err(FaultType::Failed);
        }
        
        if self.state.core_temp_c < CRITICAL_TEMP_LOW_C {
            return Err(FaultType::Failed);
        }
        
        // Check for thermal instability
        let temp_variance = self.calculate_temperature_variance();
        if temp_variance > 15.0 {
            return Err(FaultType::Degraded);
        }
        
        Ok(())
    }
    
    fn calculate_temperature_variance(&self) -> f32 {
        let mut sum = 0i32;
        let mut count = 0;
        
        for &temp in &self.temp_history {
            sum += temp as i32;
            count += 1;
        }
        
        if count == 0 {
            return 0.0;
        }
        
        let mean = sum as f32 / count as f32;
        let mut variance_sum = 0.0;
        
        for &temp in &self.temp_history {
            let diff = temp as f32 - mean;
            variance_sum += diff * diff;
        }
        
        (variance_sum / count as f32).sqrt()
    }
    
    fn auto_thermal_control(&mut self) {
        match self.thermal_mode {
            ThermalMode::Nominal => {
                // Turn on heaters if temperature drops below 10°C
                if self.state.core_temp_c < 10 {
                    self.state.heater_power_w = HEATER_POWER_W;
                } else if self.state.core_temp_c > 30 {
                    self.state.heater_power_w = 0;
                }
            }
            ThermalMode::Survival => {
                // More aggressive heating in survival mode
                if self.state.core_temp_c < 5 {
                    self.state.heater_power_w = HEATER_POWER_W;
                } else if self.state.core_temp_c > 25 {
                    self.state.heater_power_w = 0;
                }
            }
            ThermalMode::PowerSave => {
                // Minimal heating in power save mode
                if self.state.core_temp_c < -10 {
                    self.state.heater_power_w = HEATER_POWER_W / 4; // 25% power
                } else if self.state.core_temp_c > 15 {
                    self.state.heater_power_w = 0;
                }
            }
        }
    }
}

impl Subsystem for ThermalSystem {
    type State = ThermalState;
    type Command = ThermalCommand;
    
    fn update(&mut self, dt_ms: u16) -> Result<(), FaultType> {
        if let Some(fault) = self.fault_state {
            match fault {
                FaultType::Failed => return Err(fault),
                FaultType::Degraded => {
                    // Reduced thermal conductivity in degraded mode
                    self.thermal_conductivity = 0.5;
                }
                FaultType::Offline => return Err(fault),
            }
        }
        
        // Simulate orbital thermal environment
        let uptime_s = dt_ms as u32 / 1000;
        self.update_ambient_temperature(uptime_s);
        
        // Auto thermal control
        self.auto_thermal_control();
        
        // Update thermal dynamics
        self.simulate_thermal_dynamics(dt_ms)?;
        
        Ok(())
    }
    
    fn execute_command(&mut self, command: Self::Command) -> Result<(), &'static str> {
        match command {
            ThermalCommand::SetHeaterState(on) => {
                self.state.heater_power_w = if on { HEATER_POWER_W } else { 0 };
                Ok(())
            }
            ThermalCommand::SetThermalMode(mode) => {
                self.thermal_mode = mode;
                Ok(())
            }
            ThermalCommand::CalibrateTemp(offset) => {
                self.state.core_temp_c = self.state.core_temp_c.saturating_add(offset);
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
        self.thermal_conductivity = 0.95;
    }
    
    fn is_healthy(&self) -> bool {
        self.fault_state.is_none() && 
        self.state.core_temp_c > CRITICAL_TEMP_LOW_C &&
        self.state.core_temp_c < CRITICAL_TEMP_HIGH_C
    }
}