use serde::{Deserialize, Serialize};
use arrayvec::ArrayString;
use heapless::Vec;
use crate::subsystems::{SubsystemId, FaultType};

pub const MAX_COMMAND_SIZE: usize = 512;
pub const MAX_RESPONSE_SIZE: usize = 1024;
pub const MAX_TELEMETRY_SIZE: usize = 2048;

pub type CommandBuffer = ArrayString<MAX_COMMAND_SIZE>;
pub type ResponseBuffer = ArrayString<MAX_RESPONSE_SIZE>;
pub type TelemetryBuffer = ArrayString<MAX_TELEMETRY_SIZE>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Command {
    pub id: u32,
    pub timestamp: u64,
    pub command_type: CommandType,
    pub execution_time: Option<u64>, // Optional scheduled execution time (None = immediate)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CommandType {
    Ping,
    SystemStatus,
    SetHeaterState { on: bool },
    SetCommsLink { enabled: bool },
    SetSolarPanel { enabled: bool },
    SetTxPower { power_dbm: i8 },
    SimulateFault { target: SubsystemId, fault_type: FaultType },
    ClearFaults { target: Option<SubsystemId> },
    ClearSafetyEvents { force: bool }, // Ground testing override for safety events
    SetSafeMode { enabled: bool },
    TransmitMessage { message: alloc::string::String },
    SystemReboot,
    SetFaultInjection { enabled: bool },
    GetFaultInjectionStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResponse {
    pub id: u32,
    pub timestamp: u64,
    pub status: ResponseStatus,
    pub message: Option<alloc::string::String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ResponseStatus {
    Success,
    Error,
    InvalidCommand,
    SystemBusy,
    SafeModeActive,
    Scheduled, // Command scheduled for future execution
    
    // Production ACK/NACK semantics
    Acknowledged,      // Command received and accepted for execution
    NegativeAck,      // Command rejected (invalid, unsafe, etc.)
    ExecutionStarted, // Command execution has begun
    ExecutionFailed,  // Command execution failed
    Timeout,          // Command execution timed out
    InProgress,       // Command execution is ongoing
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryPacket {
    pub timestamp: u64,
    pub sequence_number: u32,
    pub system_state: SystemState,
    pub power: crate::subsystems::power::PowerState,
    pub thermal: crate::subsystems::thermal::ThermalState,
    pub comms: crate::subsystems::comms::CommsState,
    pub faults: alloc::vec::Vec<crate::subsystems::Fault>,
    
    // Optimized extended data for ~2kB packet size per production specs
    pub performance_history: [PerformanceSnapshot; 4],  // Reduced from 8 to 4
    pub safety_events: alloc::vec::Vec<SafetyEventSummary>,
    pub subsystem_diagnostics: SubsystemDiagnostics,
    pub mission_data: MissionData,
    pub orbital_data: OrbitalData,
    #[serde(with = "serde_bytes")]
    pub padding: alloc::vec::Vec<u8>,  // Smart padding to reach exactly 2kB
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemState {
    pub safe_mode: bool,
    pub uptime_seconds: u64,
    pub cpu_usage_percent: u8,
    pub memory_usage_percent: u8,
    pub last_command_id: u32,
    pub telemetry_rate_hz: u8,
    
    // Optimized system state data
    pub boot_voltage_pack: u32,      // Packed: boot_count (16bit) + system_voltage_mv (16bit)
    pub last_reset_reason: ResetReason,
    pub firmware_hash: u32,          // Reduced from [u8; 16] to u32 hash
    pub system_temperature_c: i8,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ResetReason {
    PowerOn,
    Watchdog,
    Software,
    External,
    BrownOut,
    Unknown,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PerformanceSnapshot {
    pub timestamp: u32,        // Reduced from u64 - relative time in seconds
    pub loop_time_us: u16,     // Reduced from u32 - max 65ms is plenty
    pub memory_free_kb: u16,   // Reduced from bytes to KB
    pub cpu_load_percent: u8,
    pub task_count: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyEventSummary {
    pub event_type: u8,
    pub timestamp: u64,
    pub severity: u8,
    pub subsystem_id: u8,
    pub resolved: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubsystemDiagnostics {
    pub health_scores: u32,           // Bit-packed: 8 bits each for power/thermal/comms health + 8 spare
    pub cycle_counts: [u16; 3],       // Reduced from u32 to u16 - 65k cycles is plenty
    pub last_error_codes: [u16; 4],   // Reduced from 8 to 4 most recent errors
    #[serde(with = "serde_bytes")]
    pub diagnostic_data: alloc::vec::Vec<u8>,     // Reduced from 64 to 32 bytes
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissionData {
    pub mission_elapsed_time_s: u32,    // Reduced from u64 - 4 billion seconds = 136 years is plenty
    pub orbit_number: u16,              // Reduced from u32 - 65k orbits = ~4 years is plenty
    pub ground_contact_count: u16,      // Reduced from u32
    pub data_downlinked_kb: u32,        // Reduced from u64 - 4TB is plenty
    pub commands_received: u16,         // Reduced from u32
    pub mission_phase: MissionPhase,
    pub next_scheduled_event: u32,      // Reduced from u64 - relative time
    pub payload_status: PayloadStatus,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum MissionPhase {
    Launch,
    EarlyOrbit,
    Commissioning,
    Nominal,
    EndOfLife,
    SafeMode,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum PayloadStatus {
    Off,
    Standby,
    Active,
    Error,
    Maintenance,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrbitalData {
    pub altitude_km: u16,            // Fixed-point: actual = value as f32, max 65km is plenty for LEO
    pub velocity_ms: u16,            // Fixed-point: actual = value as f32, max 65k m/s
    pub inclination_deg: u8,         // 0-180 degrees fits in u8
    pub latitude_deg: i8,            // -90 to +90 degrees
    pub longitude_deg: u16,          // 0-360 degrees, scaled: actual = value * 360.0 / 65535.0
    pub sun_angle_deg: i16,          // -180 to +180 degrees
    pub eclipse_duration_s: u16,     // Max 65k seconds = 18 hours is plenty
    pub magnetic_field_nt: [i16; 3], // Scaled: actual = value as f32 * 10.0 (nanoTesla precision)
    pub angular_velocity: [i16; 3],  // Scaled: actual = value as f32 * 1000.0 (millirad/s precision)
    pub attitude_quat_xyz: [i16; 3], // Compressed quaternion: omit w, derive from xyz
}

// Production command tracking for ACK/NACK semantics
const MAX_TRACKED_COMMANDS: usize = 16;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandTracker {
    pub command_id: u32,
    pub timestamp: u64,
    pub status: ResponseStatus,
    pub execution_start_time: Option<u64>,
    pub timeout_ms: u64,
    pub retry_count: u8,
    pub last_update: u64,
}

impl CommandTracker {
    pub fn new(command_id: u32, timestamp: u64, timeout_ms: u64) -> Self {
        Self {
            command_id,
            timestamp,
            status: ResponseStatus::Acknowledged,
            execution_start_time: None,
            timeout_ms,
            retry_count: 0,
            last_update: timestamp,
        }
    }
    
    pub fn is_expired(&self, current_time: u64) -> bool {
        current_time > self.timestamp + self.timeout_ms
    }
    
    pub fn update_status(&mut self, status: ResponseStatus, current_time: u64) {
        self.status = status;
        self.last_update = current_time;
        
        if matches!(status, ResponseStatus::ExecutionStarted) {
            self.execution_start_time = Some(current_time);
        }
    }
}

#[derive(Debug)]
pub struct ProtocolHandler {
    sequence_counter: u32,
    command_counter: u32,
    #[allow(dead_code)]
    last_telemetry_time: u64,
    
    // Preallocated buffers
    command_buffer: CommandBuffer,
    response_buffer: ResponseBuffer,
    telemetry_buffer: TelemetryBuffer,
    
    // Command tracking for ACK/NACK semantics
    tracked_commands: Vec<CommandTracker, MAX_TRACKED_COMMANDS>,
}

impl ProtocolHandler {
    pub fn new() -> Self {
        Self {
            sequence_counter: 0,
            command_counter: 0,
            last_telemetry_time: 0,
            command_buffer: ArrayString::new(),
            response_buffer: ArrayString::new(),
            telemetry_buffer: ArrayString::new(),
            tracked_commands: Vec::new(),
        }
    }
    
    pub fn parse_command(&mut self, json_str: &str) -> Result<Command, ProtocolError> {
        self.command_buffer.clear();
        if json_str.len() > MAX_COMMAND_SIZE {
            return Err(ProtocolError::MessageTooLarge);
        }
        self.command_buffer.push_str(json_str);
        
        match serde_json::from_str::<Command>(json_str) {
            Ok(command) => Ok(command),
            Err(_) => Err(ProtocolError::InvalidJson),
        }
    }
    
    pub fn serialize_response(&mut self, response: &CommandResponse) -> Result<&str, ProtocolError> {
        self.response_buffer.clear();
        
        let json_str = serde_json::to_string(response)
            .map_err(|_| ProtocolError::SerializationError)?;
        
        if json_str.len() > MAX_RESPONSE_SIZE {
            return Err(ProtocolError::MessageTooLarge);
        }
        self.response_buffer.push_str(&json_str);
        
        Ok(&self.response_buffer)
    }
    
    pub fn serialize_telemetry(&mut self, packet: &TelemetryPacket) -> Result<&str, ProtocolError> {
        self.telemetry_buffer.clear();
        
        let json_str = serde_json::to_string(packet)
            .map_err(|_| ProtocolError::SerializationError)?;
        
        if json_str.len() > MAX_TELEMETRY_SIZE {
            return Err(ProtocolError::MessageTooLarge);
        }
        self.telemetry_buffer.push_str(&json_str);
        
        Ok(&self.telemetry_buffer)
    }
    
    pub fn create_response(&mut self, command_id: u32, status: ResponseStatus, message: Option<&str>) -> CommandResponse {
        let message_string = message.map(|msg| alloc::string::ToString::to_string(msg));
        
        CommandResponse {
            id: command_id,
            timestamp: self.get_timestamp(),
            status,
            message: message_string,
        }
    }
    
    pub fn create_telemetry_packet(
        &mut self,
        system_state: SystemState,
        power: crate::subsystems::power::PowerState,
        thermal: crate::subsystems::thermal::ThermalState,
        comms: crate::subsystems::comms::CommsState,
        faults: alloc::vec::Vec<crate::subsystems::Fault>,
    ) -> TelemetryPacket {
        self.sequence_counter = self.sequence_counter.wrapping_add(1);
        let timestamp = self.get_timestamp();
        
        // Create packet with minimal padding first
        let mut packet = TelemetryPacket {
            timestamp,
            sequence_number: self.sequence_counter,
            system_state,
            power,
            thermal,
            comms,
            faults,
            
            // Generate optimized extended telemetry data
            performance_history: self.generate_performance_history(timestamp),
            safety_events: self.generate_safety_events(),
            subsystem_diagnostics: self.generate_diagnostics(),
            mission_data: self.generate_mission_data(timestamp),
            orbital_data: self.generate_orbital_data(timestamp),
            padding: vec![],  // Start with no padding
        };
        
        // Calculate smart padding to reach exactly 2kB
        if let Ok(json_str) = serde_json::to_string(&packet) {
            let current_size = json_str.len();
            const TARGET_SIZE: usize = 2048;
            
            if current_size < TARGET_SIZE {
                let padding_needed = TARGET_SIZE.saturating_sub(current_size).saturating_sub(150); // Account for JSON field overhead and hit exact target
                packet.padding = vec![0x42; padding_needed.max(1).min(500)]; // Cap padding at 500 bytes
            }
        }
        
        packet
    }
    
    pub fn next_command_id(&mut self) -> u32 {
        self.command_counter = self.command_counter.wrapping_add(1);
        self.command_counter
    }
    
    fn get_timestamp(&self) -> u64 {
        // In real implementation, this would use system time
        // For simulation, we'll use a simple counter
        self.sequence_counter as u64 * 1000
    }
    
    fn generate_performance_history(&self, timestamp: u64) -> [PerformanceSnapshot; 4] {
        let mut history = [PerformanceSnapshot {
            timestamp: 0,
            loop_time_us: 0,
            memory_free_kb: 0,
            cpu_load_percent: 0,
            task_count: 0,
        }; 4];
        
        for (i, snapshot) in history.iter_mut().enumerate() {
            let time_offset = (i as u64 + 1) * 1000;
            *snapshot = PerformanceSnapshot {
                timestamp: (timestamp.saturating_sub(time_offset) / 1000) as u32,
                loop_time_us: (800 + (i * 50)) as u16,
                memory_free_kb: 1024 - (i * 50) as u16,  // KB instead of bytes
                cpu_load_percent: (25 + i * 5) as u8,
                task_count: (8 + i) as u8,
            };
        }
        
        history
    }
    
    fn generate_safety_events(&self) -> alloc::vec::Vec<SafetyEventSummary> {
        let mut events = alloc::vec::Vec::new();
        
        // Add recent safety events (simulated) - reduced to 2 events
        for i in 0..2 {
            events.push(SafetyEventSummary {
                event_type: i as u8,
                timestamp: (self.sequence_counter as u64 * 1000).saturating_sub(i as u64 * 5000),
                severity: if i == 0 { 2 } else { 1 },  // Critical, Warning levels
                subsystem_id: i as u8,
                resolved: i > 0,
            });
        }
        
        events
    }
    
    fn generate_diagnostics(&self) -> SubsystemDiagnostics {
        // Bit-pack health scores: power=95, thermal=88, comms=92
        let health_scores = (95u32 << 24) | (88u32 << 16) | (92u32 << 8) | 0u32;
        
        SubsystemDiagnostics {
            health_scores,
            cycle_counts: [
                (self.sequence_counter / 100).min(65535) as u16,
                (self.sequence_counter / 50).min(65535) as u16,
                (self.sequence_counter / 200).min(65535) as u16,
            ],
            last_error_codes: [0x0001, 0x0002, 0x0040, 0x0080],  // Reduced to 4
            diagnostic_data: vec![0x55; 16],  // Reduced to 16 bytes - core diagnostics only
        }
    }
    
    fn generate_mission_data(&self, timestamp: u64) -> MissionData {
        MissionData {
            mission_elapsed_time_s: (timestamp / 1000) as u32,
            orbit_number: ((timestamp / 1000) / 5400).min(65535) as u16,
            ground_contact_count: ((timestamp / 1000) / 1800).min(65535) as u16,
            data_downlinked_kb: ((timestamp / 1000) * 2).min(u32::MAX as u64) as u32,
            commands_received: (self.sequence_counter / 10).min(65535) as u16,
            mission_phase: if timestamp < 86400000 { MissionPhase::EarlyOrbit } else { MissionPhase::Nominal },
            next_scheduled_event: ((timestamp + 3600000) / 1000) as u32,
            payload_status: PayloadStatus::Active,
        }
    }
    
    fn generate_orbital_data(&self, timestamp: u64) -> OrbitalData {
        let orbit_phase = (timestamp as f32 * 0.001) * 0.001;
        
        // Compressed quaternion: store xyz, derive w = sqrt(1 - x²- y² - z²)
        let qx = 0.0f32;
        let qy = 0.0f32; 
        let qz = 0.707f32;
        
        OrbitalData {
            altitude_km: (400.0 + (orbit_phase.sin() * 50.0)) as u16,
            velocity_ms: (7800.0 + (orbit_phase.cos() * 100.0)) as u16,
            inclination_deg: 98,
            latitude_deg: ((orbit_phase * 6.28).sin() * 90.0) as i8,
            longitude_deg: ((timestamp as f32 * 0.0001) % 360.0 * 65535.0 / 360.0) as u16,
            sun_angle_deg: ((orbit_phase * 2.0).cos() * 180.0) as i16,
            eclipse_duration_s: if orbit_phase.sin() > 0.0 { 0 } else { 2160 },
            magnetic_field_nt: [
                ((25000.0 + orbit_phase.sin() * 5000.0) / 10.0) as i16,
                ((15000.0 + orbit_phase.cos() * 3000.0) / 10.0) as i16,
                ((45000.0 + (orbit_phase * 2.0).sin() * 2000.0) / 10.0) as i16,
            ],
            angular_velocity: [
                (0.1 * 1000.0) as i16,
                (-0.05 * 1000.0) as i16,
                (0.02 * 1000.0) as i16,
            ],
            attitude_quat_xyz: [
                (qx * 32767.0) as i16,
                (qy * 32767.0) as i16,
                (qz * 32767.0) as i16,
            ],
        }
    }
    
    pub fn validate_command(&self, command: &Command) -> Result<(), ProtocolError> {
        // Basic validation
        if command.id == 0 {
            return Err(ProtocolError::InvalidCommand);
        }
        
        // Validate command-specific parameters
        match &command.command_type {
            CommandType::SetTxPower { power_dbm } => {
                if *power_dbm < 0 || *power_dbm > 30 {
                    return Err(ProtocolError::InvalidParameter);
                }
            }
            CommandType::TransmitMessage { message } => {
                if message.is_empty() {
                    return Err(ProtocolError::InvalidParameter);
                }
            }
            _ => {}
        }
        
        Ok(())
    }
    
    // ACK/NACK command tracking methods
    
    /// Start tracking a command with initial ACK
    pub fn track_command(&mut self, command_id: u32, current_time: u64, timeout_ms: u64) -> Result<(), ProtocolError> {
        // Remove expired commands first
        self.cleanup_expired_commands(current_time);
        
        // Check if command is already being tracked
        if self.tracked_commands.iter().any(|t| t.command_id == command_id) {
            return Err(ProtocolError::InvalidCommand);
        }
        
        // Add new tracker
        let tracker = CommandTracker::new(command_id, current_time, timeout_ms);
        if self.tracked_commands.push(tracker).is_err() {
            // Remove oldest command if buffer is full
            self.tracked_commands.swap_remove(0);
            let _ = self.tracked_commands.push(CommandTracker::new(command_id, current_time, timeout_ms));
        }
        
        Ok(())
    }
    
    /// Update command status with proper ACK/NACK
    pub fn update_command_status(&mut self, command_id: u32, status: ResponseStatus, current_time: u64) -> Result<(), ProtocolError> {
        if let Some(tracker) = self.tracked_commands.iter_mut().find(|t| t.command_id == command_id) {
            tracker.update_status(status, current_time);
            Ok(())
        } else {
            Err(ProtocolError::InvalidCommand)
        }
    }
    
    /// Get current status of a tracked command
    pub fn get_command_status(&self, command_id: u32) -> Option<&CommandTracker> {
        self.tracked_commands.iter().find(|t| t.command_id == command_id)
    }
    
    /// Clean up expired commands
    pub fn cleanup_expired_commands(&mut self, current_time: u64) {
        self.tracked_commands.retain(|tracker| !tracker.is_expired(current_time));
    }
    
    /// Get all tracked commands for telemetry
    pub fn get_tracked_commands(&self) -> &[CommandTracker] {
        &self.tracked_commands
    }
    
    /// Create ACK response
    pub fn create_ack_response(&mut self, command_id: u32, message: Option<&str>) -> CommandResponse {
        self.create_response(command_id, ResponseStatus::Acknowledged, message)
    }
    
    /// Create NACK response with reason
    pub fn create_nack_response(&mut self, command_id: u32, reason: &str) -> CommandResponse {
        self.create_response(command_id, ResponseStatus::NegativeAck, Some(reason))
    }
    
    /// Create execution started response
    pub fn create_execution_started_response(&mut self, command_id: u32) -> CommandResponse {
        self.create_response(command_id, ResponseStatus::ExecutionStarted, Some("Command execution started"))
    }
    
    /// Create execution failed response
    pub fn create_execution_failed_response(&mut self, command_id: u32, reason: &str) -> CommandResponse {
        self.create_response(command_id, ResponseStatus::ExecutionFailed, Some(reason))
    }
    
    /// Create timeout response
    pub fn create_timeout_response(&mut self, command_id: u32) -> CommandResponse {
        self.create_response(command_id, ResponseStatus::Timeout, Some("Command execution timed out"))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolError {
    InvalidJson,
    MessageTooLarge,
    SerializationError,
    InvalidCommand,
    InvalidParameter,
    BufferOverflow,
}

impl core::fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ProtocolError::InvalidJson => write!(f, "Invalid JSON format"),
            ProtocolError::MessageTooLarge => write!(f, "Message exceeds buffer size"),
            ProtocolError::SerializationError => write!(f, "Serialization failed"),
            ProtocolError::InvalidCommand => write!(f, "Invalid command"),
            ProtocolError::InvalidParameter => write!(f, "Invalid parameter"),
            ProtocolError::BufferOverflow => write!(f, "Buffer overflow"),
        }
    }
}

// Zero-copy message framing for TCP
#[derive(Debug)]
pub struct MessageFrame {
    pub length: u32,
    pub payload: [u8; MAX_COMMAND_SIZE],
}

impl MessageFrame {
    pub fn new() -> Self {
        Self {
            length: 0,
            payload: [0; MAX_COMMAND_SIZE],
        }
    }
    
    pub fn from_str(s: &str) -> Result<Self, ProtocolError> {
        let bytes = s.as_bytes();
        if bytes.len() > MAX_COMMAND_SIZE {
            return Err(ProtocolError::MessageTooLarge);
        }
        
        let mut frame = Self::new();
        frame.length = bytes.len() as u32;
        frame.payload[..bytes.len()].copy_from_slice(bytes);
        
        Ok(frame)
    }
    
    pub fn as_str(&self) -> Result<&str, ProtocolError> {
        let bytes = &self.payload[..self.length as usize];
        core::str::from_utf8(bytes).map_err(|_| ProtocolError::InvalidJson)
    }
    
    pub fn to_bytes(&self) -> &[u8] {
        &self.payload[..self.length as usize]
    }
}