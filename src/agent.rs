use crate::subsystems::{PowerSystem, ThermalSystem, CommsSystem, Subsystem, FaultType, SubsystemId};
use crate::protocol::{Command, CommandResponse, ResponseStatus, ProtocolHandler, ProtocolError};
use crate::telemetry::TelemetryCollector;
use crate::safety::{SafetyManager, SafetyActions};
use crate::fault_injection::FaultInjector;
use crate::scheduler::CommandScheduler;
use heapless::{spsc::Queue, Vec};
use serde::{Deserialize, Serialize};
use std::time::Instant;

const MAX_COMMAND_QUEUE_SIZE: usize = 32;
// Production satellite telemetry rate: 1 Hz (1000ms) per subsystem
const MAIN_LOOP_PERIOD_MS: u64 = 1000;

// Production command rate limits per satellite specifications
const MAX_COMMAND_RATE_PER_SEC: u32 = 5;   // Burst capacity
const AVG_COMMAND_RATE_PER_SEC: u32 = 2;   // Average sustained rate
const RATE_LIMIT_WINDOW_MS: u64 = 1000;    // 1 second window

type CommandQueue = Queue<Command, MAX_COMMAND_QUEUE_SIZE>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentState {
    pub running: bool,
    pub uptime_seconds: u64,
    pub command_count: u32,
    pub telemetry_count: u32,
    pub last_error: Option<alloc::string::String>,
    pub performance_stats: PerformanceStats,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct PerformanceStats {
    pub loop_time_us: u32,
    pub command_processing_time_us: u32,
    pub telemetry_generation_time_us: u32,
    pub safety_check_time_us: u32,
    pub memory_usage_bytes: u32,
}

pub struct SatelliteAgent {
    // Core subsystems
    power_system: PowerSystem,
    thermal_system: ThermalSystem,
    comms_system: CommsSystem,
    
    // Protocol and telemetry
    protocol_handler: ProtocolHandler,
    telemetry_collector: TelemetryCollector,
    safety_manager: SafetyManager,
    fault_injector: FaultInjector,
    command_scheduler: CommandScheduler,
    
    // Agent state
    state: AgentState,
    start_time: Instant,
    last_telemetry_time: Instant,
    
    // Command processing
    command_queue: CommandQueue,
    
    // Rate limiting for production compliance
    command_timestamps: Vec<Instant, 16>,  // Track recent command times
    
    // Preallocated buffers
    response_buffer: Vec<CommandResponse, 16>,
    
    // Performance monitoring
    loop_start_time: Instant,
    performance_history: [PerformanceStats; 16],
    performance_index: usize,
}

impl SatelliteAgent {
    pub fn new() -> Self {
        let start_time = Instant::now();
        
        Self {
            power_system: PowerSystem::new(),
            thermal_system: ThermalSystem::new(),
            comms_system: CommsSystem::new(),
            protocol_handler: ProtocolHandler::new(),
            telemetry_collector: TelemetryCollector::new(),
            safety_manager: SafetyManager::new(),
            fault_injector: FaultInjector::new(),
            command_scheduler: CommandScheduler::new(),
            state: AgentState {
                running: false,
                uptime_seconds: 0,
                command_count: 0,
                telemetry_count: 0,
                last_error: None,
                performance_stats: PerformanceStats::default(),
            },
            start_time,
            last_telemetry_time: start_time,
            command_queue: Queue::new(),
            command_timestamps: Vec::new(),
            response_buffer: Vec::new(),
            loop_start_time: start_time,
            performance_history: [PerformanceStats::default(); 16],
            performance_index: 0,
        }
    }
    
    pub fn start(&mut self) {
        self.state.running = true;
        self.start_time = Instant::now();
        self.last_telemetry_time = self.start_time;
        
        println!("🚀 Satellite Bus Simulator starting...");
        println!("   Power System: ✓");
        println!("   Thermal System: ✓");
        println!("   Communications System: ✓");
        println!("   Safety Manager: ✓");
        println!("   Telemetry Collector: ✓");
        println!("📡 Ready for commands on TCP port 8080");
    }
    
    pub fn stop(&mut self) {
        self.state.running = false;
        println!("🛑 Satellite Bus Simulator stopping...");
    }
    
    pub fn update(&mut self) -> Result<Option<alloc::string::String>, AgentError> {
        if !self.state.running {
            return Ok(None);
        }
        
        self.loop_start_time = Instant::now();
        
        // Update uptime
        self.state.uptime_seconds = self.start_time.elapsed().as_secs();
        
        // Clean up expired command tracking
        let current_time = self.start_time.elapsed().as_millis() as u64;
        self.protocol_handler.cleanup_expired_commands(current_time);
        
        // Process scheduled commands
        self.process_scheduled_commands()?;
        
        // Process commands
        self.process_commands()?;
        
        // Update subsystems
        self.update_subsystems()?;
        
        // Fault injection (before safety checks to allow safety response)
        self.process_fault_injection()?;
        
        // Safety checks
        self.perform_safety_checks()?;
        
        // Generate telemetry
        let telemetry = self.generate_telemetry()?;
        
        // Update performance stats
        self.update_performance_stats();
        
        Ok(telemetry)
    }
    
    
    fn execute_command(&mut self, command: Command) -> Result<CommandResponse, AgentError> {
        let current_time = self.start_time.elapsed().as_millis() as u64;
        
        // Start tracking command for ACK/NACK semantics (30 second timeout)
        if let Err(_) = self.protocol_handler.track_command(command.id, current_time, 30000) {
            return Ok(self.protocol_handler.create_nack_response(
                command.id,
                "Command already being processed or tracking failed"
            ));
        }
        
        // Handle scheduled commands
        if let Some(execution_time) = command.execution_time {
            if execution_time > current_time {
                // Schedule the command
                self.command_scheduler.schedule_command(command.clone(), current_time)
                    .map_err(|e| AgentError::SchedulingError(alloc::string::ToString::to_string(e)))?;
                
                return Ok(self.protocol_handler.create_response(
                    command.id,
                    ResponseStatus::Scheduled,
                    Some(&alloc::format!("Command scheduled for execution at {}", execution_time)),
                ));
            }
        }
        // Validate command
        if let Err(e) = self.protocol_handler.validate_command(&command) {
            let _ = self.protocol_handler.update_command_status(command.id, ResponseStatus::NegativeAck, current_time);
            return Ok(self.protocol_handler.create_nack_response(
                command.id,
                &alloc::format!("Command validation failed: {}", e)
            ));
        }
        
        // Send initial ACK
        let _ = self.protocol_handler.update_command_status(command.id, ResponseStatus::Acknowledged, current_time);
        
        // Check if safe mode blocks this command
        if self.safety_manager.get_state().safe_mode_active {
            match command.command_type {
                crate::protocol::CommandType::Ping |
                crate::protocol::CommandType::SystemStatus |
                crate::protocol::CommandType::ClearFaults { .. } |
                crate::protocol::CommandType::ClearSafetyEvents { .. } |
                crate::protocol::CommandType::SetSafeMode { .. } => {
                    // Allow these commands in safe mode
                }
                _ => {
                    let _ = self.protocol_handler.update_command_status(command.id, ResponseStatus::NegativeAck, current_time);
                    return Ok(self.protocol_handler.create_nack_response(
                        command.id,
                        "Command blocked - system in safe mode"
                    ));
                }
            }
        }
        
        // Mark execution as started
        let _ = self.protocol_handler.update_command_status(command.id, ResponseStatus::ExecutionStarted, current_time);
        
        // Execute command
        let response_status = match command.command_type {
            crate::protocol::CommandType::Ping => {
                ResponseStatus::Success
            }
            
            crate::protocol::CommandType::SystemStatus => {
                ResponseStatus::Success
            }
            
            crate::protocol::CommandType::SetHeaterState { on } => {
                match self.thermal_system.execute_command(
                    crate::subsystems::thermal::ThermalCommand::SetHeaterState(on)
                ) {
                    Ok(_) => ResponseStatus::Success,
                    Err(_) => ResponseStatus::Error,
                }
            }
            
            crate::protocol::CommandType::SetCommsLink { enabled } => {
                match self.comms_system.execute_command(
                    crate::subsystems::comms::CommsCommand::SetLinkState(enabled)
                ) {
                    Ok(_) => ResponseStatus::Success,
                    Err(_) => ResponseStatus::Error,
                }
            }
            
            crate::protocol::CommandType::SetSolarPanel { enabled } => {
                match self.power_system.execute_command(
                    crate::subsystems::power::PowerCommand::SetSolarPanel(enabled)
                ) {
                    Ok(_) => ResponseStatus::Success,
                    Err(_) => ResponseStatus::Error,
                }
            }
            
            crate::protocol::CommandType::SetTxPower { power_dbm } => {
                match self.comms_system.execute_command(
                    crate::subsystems::comms::CommsCommand::SetTxPower(power_dbm)
                ) {
                    Ok(_) => ResponseStatus::Success,
                    Err(_) => ResponseStatus::Error,
                }
            }
            
            crate::protocol::CommandType::SimulateFault { target, fault_type } => {
                match target {
                    SubsystemId::Power => self.power_system.inject_fault(fault_type),
                    SubsystemId::Thermal => self.thermal_system.inject_fault(fault_type),
                    SubsystemId::Comms => self.comms_system.inject_fault(fault_type),
                }
                ResponseStatus::Success
            }
            
            crate::protocol::CommandType::ClearFaults { target } => {
                match target {
                    Some(SubsystemId::Power) => {
                        self.power_system.clear_faults();
                        self.fault_injector.clear_faults(Some(SubsystemId::Power));
                    }
                    Some(SubsystemId::Thermal) => {
                        self.thermal_system.clear_faults();
                        self.fault_injector.clear_faults(Some(SubsystemId::Thermal));
                    }
                    Some(SubsystemId::Comms) => {
                        self.comms_system.clear_faults();
                        self.fault_injector.clear_faults(Some(SubsystemId::Comms));
                    }
                    None => {
                        self.power_system.clear_faults();
                        self.thermal_system.clear_faults();
                        self.comms_system.clear_faults();
                        self.fault_injector.clear_faults(None);
                    }
                }
                ResponseStatus::Success
            }
            
            crate::protocol::CommandType::ClearSafetyEvents { force } => {
                match self.safety_manager.clear_safety_events(force) {
                    Ok(_) => ResponseStatus::Success,
                    Err(_) => ResponseStatus::Error,
                }
            }
            
            crate::protocol::CommandType::SetSafeMode { enabled } => {
                let current_time = self.start_time.elapsed().as_millis() as u64;
                if enabled {
                    self.safety_manager.force_safe_mode(current_time);
                    // Verify safe mode is actually active
                    if self.safety_manager.get_state().safe_mode_active {
                        ResponseStatus::Success
                    } else {
                        ResponseStatus::Error
                    }
                } else {
                    self.safety_manager.disable_safe_mode(current_time);
                    // For disable, success means either safe mode is off OR manual override is active
                    let state = self.safety_manager.get_state();
                    if !state.safe_mode_active || state.manual_override_active {
                        ResponseStatus::Success
                    } else {
                        ResponseStatus::Error
                    }
                }
            }
            
            crate::protocol::CommandType::TransmitMessage { ref message } => {
                let mut msg_buf = arrayvec::ArrayString::<256>::new();
                if message.len() <= 256 {
                    msg_buf.push_str(&message);
                    if msg_buf.len() <= 256 {
                        match self.comms_system.execute_command(
                            crate::subsystems::comms::CommsCommand::TransmitMessage(msg_buf)
                        ) {
                            Ok(_) => ResponseStatus::Success,
                            Err(_) => ResponseStatus::Error,
                        }
                    } else {
                        ResponseStatus::Error
                    }
                } else {
                    ResponseStatus::Error
                }
            }
            
            crate::protocol::CommandType::SystemReboot => {
                self.power_system.execute_command(
                    crate::subsystems::power::PowerCommand::Reboot
                ).ok();
                ResponseStatus::Success
            }
            
            crate::protocol::CommandType::SetFaultInjection { enabled } => {
                self.fault_injector.set_enabled(enabled);
                ResponseStatus::Success
            }
            
            crate::protocol::CommandType::GetFaultInjectionStatus => {
                // Return detailed fault injection stats
                ResponseStatus::Success
            }
        };
        
        // Handle special response for fault injection status
        let response_message = match &command.command_type {
            crate::protocol::CommandType::GetFaultInjectionStatus => {
                let stats = self.fault_injector.get_stats();
                let config = self.fault_injector.get_config();
                Some(alloc::format!(
                    r#"{{"config":{{"enabled":{},"power_rate_percent":{},"thermal_rate_percent":{},"comms_rate_percent":{}}},"stats":{{"total_faults_injected":{},"current_active_faults":{}}}}}"#,
                    config.enabled,
                    config.power_rate_percent,
                    config.thermal_rate_percent,
                    config.comms_rate_percent,
                    stats.total_faults_injected,
                    stats.current_active_faults
                ))
            }
            _ => None,
        };
        
        // Update final command status
        let final_status = match response_status {
            ResponseStatus::Success => ResponseStatus::Success,
            ResponseStatus::Error => ResponseStatus::ExecutionFailed,
            _ => response_status,
        };
        
        let _ = self.protocol_handler.update_command_status(command.id, final_status, current_time);
        
        Ok(self.protocol_handler.create_response(
            command.id,
            response_status,
            response_message.as_deref(),
        ))
    }
    
    fn process_scheduled_commands(&mut self) -> Result<(), AgentError> {
        let current_time = self.start_time.elapsed().as_millis() as u64;
        
        // Clean up expired commands first
        self.command_scheduler.cleanup_expired_commands(current_time);
        
        // Get commands ready for execution
        let ready_commands = self.command_scheduler.get_ready_commands(current_time);
        
        // Queue ready commands for immediate execution
        for command in ready_commands {
            // Create a copy with execution_time set to None for immediate execution
            let mut immediate_command = command;
            immediate_command.execution_time = None;
            
            if let Err(e) = self.queue_command_immediate(immediate_command) {
                // Log error but continue processing other commands
                self.state.last_error = Some(alloc::format!("Scheduled command error: {}", e));
            }
        }
        
        Ok(())
    }
    
    fn process_fault_injection(&mut self) -> Result<(), AgentError> {
        let current_time = self.start_time.elapsed().as_millis() as u64;
        let fault_actions = self.fault_injector.update(current_time);
        
        // Apply fault injection actions to subsystems
        for (subsystem, fault_option) in fault_actions {
            match subsystem {
                SubsystemId::Power => {
                    if let Some(fault_type) = fault_option {
                        self.power_system.inject_fault(fault_type);
                    } else {
                        self.power_system.clear_faults();
                    }
                }
                SubsystemId::Thermal => {
                    if let Some(fault_type) = fault_option {
                        self.thermal_system.inject_fault(fault_type);
                    } else {
                        self.thermal_system.clear_faults();
                    }
                }
                SubsystemId::Comms => {
                    if let Some(fault_type) = fault_option {
                        self.comms_system.inject_fault(fault_type);
                    } else {
                        self.comms_system.clear_faults();
                    }
                }
            }
        }
        
        Ok(())
    }
    
    fn update_subsystems(&mut self) -> Result<(), AgentError> {
        let dt_ms = MAIN_LOOP_PERIOD_MS as u16;
        
        // Update power system
        if let Err(fault) = self.power_system.update(dt_ms) {
            match fault {
                FaultType::Failed => {
                    self.state.last_error = Some(alloc::string::ToString::to_string("Power system failed"));
                }
                FaultType::Degraded => {
                    // Continue operation with degraded performance
                }
                FaultType::Offline => {
                    return Err(AgentError::SubsystemError(alloc::string::ToString::to_string("Power system offline")));
                }
            }
        }
        
        // Update thermal system
        if let Err(fault) = self.thermal_system.update(dt_ms) {
            match fault {
                FaultType::Failed => {
                    self.state.last_error = Some(alloc::string::ToString::to_string("Thermal system failed"));
                }
                FaultType::Degraded => {
                    // Continue operation with degraded performance
                }
                FaultType::Offline => {
                    return Err(AgentError::SubsystemError(alloc::string::ToString::to_string("Thermal system offline")));
                }
            }
        }
        
        // Update communications system
        if let Err(fault) = self.comms_system.update(dt_ms) {
            match fault {
                FaultType::Failed => {
                    self.state.last_error = Some(alloc::string::ToString::to_string("Communications system failed"));
                }
                FaultType::Degraded => {
                    // Continue operation with degraded performance
                }
                FaultType::Offline => {
                    // Communications offline is not critical for satellite operation
                }
            }
        }
        
        Ok(())
    }
    
    fn perform_safety_checks(&mut self) -> Result<(), AgentError> {
        let start_time = Instant::now();
        let current_time = self.start_time.elapsed().as_millis() as u64;
        
        let safety_actions = self.safety_manager.update_safety_state(
            current_time,
            &self.power_system,
            &self.thermal_system,
            &self.comms_system,
        );
        
        // Execute safety actions
        self.execute_safety_actions(safety_actions)?;
        
        self.state.performance_stats.safety_check_time_us = 
            start_time.elapsed().as_micros() as u32;
        
        Ok(())
    }
    
    fn execute_safety_actions(&mut self, actions: SafetyActions) -> Result<(), AgentError> {
        if !actions.has_actions() {
            return Ok(());
        }
        
        // Power-related actions
        if actions.enable_power_save || actions.enable_emergency_power_save {
            self.power_system.execute_command(
                crate::subsystems::power::PowerCommand::SetPowerSave(true)
            ).ok();
        }
        
        // Thermal-related actions
        if actions.enable_heaters || actions.enable_emergency_heaters {
            self.thermal_system.execute_command(
                crate::subsystems::thermal::ThermalCommand::SetHeaterState(true)
            ).ok();
        }
        
        if actions.disable_heaters {
            self.thermal_system.execute_command(
                crate::subsystems::thermal::ThermalCommand::SetHeaterState(false)
            ).ok();
        }
        
        // Communications-related actions
        if actions.disable_non_essential_systems {
            self.comms_system.execute_command(
                crate::subsystems::comms::CommsCommand::SetLinkState(false)
            ).ok();
        }
        
        if actions.restore_normal_operations {
            self.comms_system.execute_command(
                crate::subsystems::comms::CommsCommand::SetLinkState(true)
            ).ok();
        }
        
        Ok(())
    }
    
    fn generate_telemetry(&mut self) -> Result<Option<alloc::string::String>, AgentError> {
        let start_time = Instant::now();
        let current_time = self.start_time.elapsed().as_millis() as u64;
        
        let empty_faults: &[crate::subsystems::Fault] = &[];
        let telemetry = self.telemetry_collector.collect_telemetry(
            current_time,
            self.state.uptime_seconds,
            self.safety_manager.get_state().safe_mode_active,
            self.state.command_count,
            &self.power_system,
            &self.thermal_system,
            &self.comms_system,
            empty_faults,
        ).map_err(|e| AgentError::TelemetryError(alloc::string::ToString::to_string(e)))?;
        
        if telemetry.is_some() {
            self.state.telemetry_count = self.state.telemetry_count.saturating_add(1);
        }
        
        self.state.performance_stats.telemetry_generation_time_us = 
            start_time.elapsed().as_micros() as u32;
        
        Ok(telemetry.map(|s| alloc::string::ToString::to_string(s)))
    }
    
    fn update_performance_stats(&mut self) {
        self.state.performance_stats.loop_time_us = 
            self.loop_start_time.elapsed().as_micros() as u32;
        
        // Estimate memory usage (simplified)
        self.state.performance_stats.memory_usage_bytes = 
            core::mem::size_of::<Self>() as u32 + 
            self.command_queue.len() as u32 * 64 + 
            self.response_buffer.len() as u32 * 128;
        
        // Store in history
        self.performance_history[self.performance_index] = self.state.performance_stats.clone();
        self.performance_index = (self.performance_index + 1) % self.performance_history.len();
    }
    
    fn cleanup_old_timestamps(&mut self, now: Instant) {
        let cutoff = now - std::time::Duration::from_millis(RATE_LIMIT_WINDOW_MS);
        self.command_timestamps.retain(|&ts| ts >= cutoff);
    }
    
    pub fn queue_command(&mut self, command: Command) -> Result<(), AgentError> {
        // All commands (including scheduled ones) go through the normal queue
        // The execute_command method will handle scheduling logic and responses
        self.queue_command_immediate(command)
    }
    
    fn queue_command_immediate(&mut self, command: Command) -> Result<(), AgentError> {
        // NASA Rule 5: Safety assertion for queue capacity
        debug_assert!(
            self.command_queue.len() < MAX_COMMAND_QUEUE_SIZE,
            "Command queue length {} at capacity {}", 
            self.command_queue.len(), MAX_COMMAND_QUEUE_SIZE
        );
        
        // Production rate limiting per satellite specifications
        let now = Instant::now();
        self.cleanup_old_timestamps(now);
        
        // Check burst rate limit (5 cmd/s)
        if self.command_timestamps.len() >= MAX_COMMAND_RATE_PER_SEC as usize {
            return Err(AgentError::RateLimitExceeded);
        }
        
        // Check average rate limit (2 cmd/s over longer period)
        if self.command_timestamps.len() >= AVG_COMMAND_RATE_PER_SEC as usize {
            let window_start = now - std::time::Duration::from_millis(RATE_LIMIT_WINDOW_MS);
            let recent_commands = self.command_timestamps.iter()
                .filter(|&&ts| ts >= window_start)
                .count();
            
            if recent_commands >= AVG_COMMAND_RATE_PER_SEC as usize {
                return Err(AgentError::RateLimitExceeded);
            }
        }
        
        // Record command timestamp
        if self.command_timestamps.push(now).is_err() {
            // Buffer full, remove oldest
            self.command_timestamps.swap_remove(0);
            let _ = self.command_timestamps.push(now);
        }
        
        self.command_queue.enqueue(command)
            .map_err(|_| AgentError::CommandQueueFull)
    }
    
    pub fn process_commands(&mut self) -> Result<(), AgentError> {
        let start_time = Instant::now();
        
        // Process all queued commands
        while let Some(command) = self.command_queue.dequeue() {
            match self.execute_command(command) {
                Ok(response) => {
                    if self.response_buffer.push(response.clone()).is_err() {
                        // NASA Rule 5: Safety assertion for response buffer capacity
                        debug_assert!(
                            self.response_buffer.len() >= self.response_buffer.capacity(),
                            "Response buffer should be at capacity before overflow"
                        );
                        
                        // Response buffer full, remove oldest
                        self.response_buffer.pop();
                        let _ = self.response_buffer.push(response);
                    }
                }
                Err(e) => {
                    self.state.last_error = Some(alloc::format!("Command error: {}", e));
                }
            }
            
            self.state.command_count = self.state.command_count.saturating_add(1);
        }
        
        self.state.performance_stats.command_processing_time_us = 
            start_time.elapsed().as_micros() as u32;
        
        Ok(())
    }
    
    pub fn get_responses(&mut self) -> Vec<CommandResponse, 16> {
        core::mem::take(&mut self.response_buffer)
    }
    
    pub fn get_state(&self) -> &AgentState {
        &self.state
    }
    
    pub fn get_safety_state(&self) -> &crate::safety::SafetyState {
        self.safety_manager.get_state()
    }
    
    pub fn get_subsystem_states(&self) -> (
        crate::subsystems::PowerState,
        crate::subsystems::ThermalState,
        crate::subsystems::CommsState,
    ) {
        (
            self.power_system.get_state(),
            self.thermal_system.get_state(),
            self.comms_system.get_state(),
        )
    }
    
    pub fn get_performance_history(&self) -> &[PerformanceStats] {
        &self.performance_history
    }
    
    pub fn get_fault_injection_stats(&self) -> &crate::fault_injection::FaultInjectionStats {
        self.fault_injector.get_stats()
    }
    
    pub fn set_fault_injection_enabled(&mut self, enabled: bool) {
        self.fault_injector.set_enabled(enabled);
    }
    
    pub fn get_fault_injection_config(&self) -> &crate::fault_injection::FaultInjectionConfig {
        self.fault_injector.get_config()
    }
    
    pub fn get_scheduler_stats(&self) -> &crate::scheduler::SchedulerStats {
        self.command_scheduler.get_stats()
    }
    
    pub fn get_scheduled_commands(&self) -> &[crate::scheduler::ScheduledCommand] {
        self.command_scheduler.get_scheduled_commands()
    }
    
    pub fn clear_scheduled_commands(&mut self) {
        self.command_scheduler.clear_all_scheduled();
    }
    
    pub fn get_tracked_commands(&self) -> &[crate::protocol::CommandTracker] {
        self.protocol_handler.get_tracked_commands()
    }
}


#[derive(Debug)]
pub enum AgentError {
    ProtocolError(ProtocolError),
    SubsystemError(alloc::string::String),
    TelemetryError(alloc::string::String),
    CommandQueueFull,
    RateLimitExceeded,
    SafetyError(alloc::string::String),
    SchedulingError(alloc::string::String),
}

impl core::fmt::Display for AgentError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            AgentError::ProtocolError(e) => write!(f, "Protocol error: {}", e),
            AgentError::SubsystemError(e) => write!(f, "Subsystem error: {}", e),
            AgentError::TelemetryError(e) => write!(f, "Telemetry error: {}", e),
            AgentError::CommandQueueFull => write!(f, "Command queue full"),
            AgentError::RateLimitExceeded => write!(f, "Command rate limit exceeded"),
            AgentError::SafetyError(e) => write!(f, "Safety error: {}", e),
            AgentError::SchedulingError(e) => write!(f, "Scheduling error: {}", e),
        }
    }
}

impl std::error::Error for AgentError {}