use crate::protocol::{TelemetryPacket, SystemState, ProtocolHandler};
use crate::subsystems::{PowerSystem, ThermalSystem, CommsSystem, Subsystem, Fault};
use heapless::Vec;
use serde::{Deserialize, Serialize};

const TELEMETRY_BUFFER_SIZE: usize = 128;
const DEFAULT_TELEMETRY_RATE_HZ: u8 = 1;

// Production telemetry batching parameters
const MAX_BATCH_SIZE: usize = 8;           // Maximum packets per batch
const BATCH_TIMEOUT_MS: u64 = 5000;       // Force batch transmission after 5 seconds
const MAX_SEQUENCE_NUMBER: u32 = 65535;   // 16-bit sequence numbers
pub const TELEMETRY_PRIORITY_HIGH: u8 = 1;
pub const TELEMETRY_PRIORITY_NORMAL: u8 = 2;
pub const TELEMETRY_PRIORITY_LOW: u8 = 3;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequencedTelemetryPacket {
    pub packet: TelemetryPacket,
    pub priority: u8,
    pub batch_id: u32,
    pub created_at: u64,
    pub retransmit_count: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryBatch {
    pub batch_id: u32,
    pub sequence_start: u32,
    pub sequence_end: u32,
    pub packet_count: u8,
    pub created_at: u64,
    pub priority: u8,
    pub packets: alloc::vec::Vec<SequencedTelemetryPacket>,
    pub checksum: u32,
}

impl TelemetryBatch {
    pub fn new(batch_id: u32, priority: u8, created_at: u64) -> Self {
        Self {
            batch_id,
            sequence_start: 0,
            sequence_end: 0,
            packet_count: 0,
            created_at,
            priority,
            packets: alloc::vec::Vec::new(),
            checksum: 0,
        }
    }
    
    pub fn add_packet(&mut self, mut packet: SequencedTelemetryPacket) -> Result<(), &'static str> {
        if self.packets.len() >= MAX_BATCH_SIZE {
            return Err("Batch is full");
        }
        
        // Set batch ID
        packet.batch_id = self.batch_id;
        
        // Update sequence range
        if self.packet_count == 0 {
            self.sequence_start = packet.packet.sequence_number;
        }
        self.sequence_end = packet.packet.sequence_number;
        
        self.packets.push(packet);
        self.packet_count = self.packets.len() as u8;
        
        // Update checksum (simple XOR)
        self.checksum ^= self.sequence_end;
        
        Ok(())
    }
    
    pub fn is_full(&self) -> bool {
        self.packets.len() >= MAX_BATCH_SIZE
    }
    
    pub fn is_expired(&self, current_time: u64) -> bool {
        current_time > self.created_at + BATCH_TIMEOUT_MS
    }
    
    pub fn size_bytes(&self) -> usize {
        // Rough estimate: each packet ~2KB + batch overhead
        (self.packet_count as usize * 2048) + 256
    }
}

#[derive(Debug)]
pub struct TelemetryBatcher {
    current_batch: Option<TelemetryBatch>,
    completed_batches: alloc::vec::Vec<TelemetryBatch>,
    next_batch_id: u32,
    sequence_number: u32,
    batch_stats: BatchingStats,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BatchingStats {
    pub total_packets_batched: u32,
    pub total_batches_created: u32,
    pub total_batches_transmitted: u32,
    pub average_batch_size: f32,
    pub packets_retransmitted: u32,
    pub sequence_gaps_detected: u32,
}

impl TelemetryBatcher {
    pub fn new() -> Self {
        Self {
            current_batch: None,
            completed_batches: alloc::vec::Vec::new(),
            next_batch_id: 1,
            sequence_number: 1,
            batch_stats: BatchingStats::default(),
        }
    }
    
    pub fn queue_packet(&mut self, packet: TelemetryPacket, priority: u8, current_time: u64) -> Result<(), &'static str> {
        // Create sequenced packet
        let mut sequenced_packet = SequencedTelemetryPacket {
            packet,
            priority,
            batch_id: 0,
            created_at: current_time,
            retransmit_count: 0,
        };
        
        // Assign sequence number
        sequenced_packet.packet.sequence_number = self.sequence_number;
        self.sequence_number = (self.sequence_number % MAX_SEQUENCE_NUMBER) + 1;
        
        // Create new batch if needed
        if self.current_batch.is_none() || 
           self.current_batch.as_ref().unwrap().is_full() ||
           self.current_batch.as_ref().unwrap().is_expired(current_time) {
            self.finalize_current_batch()?;
            self.start_new_batch(priority, current_time);
        }
        
        // Add packet to current batch
        if let Some(ref mut batch) = self.current_batch {
            batch.add_packet(sequenced_packet)?;
            self.batch_stats.total_packets_batched += 1;
        }
        
        Ok(())
    }
    
    pub fn finalize_current_batch(&mut self) -> Result<(), &'static str> {
        if let Some(batch) = self.current_batch.take() {
            if batch.packet_count > 0 {
                if self.completed_batches.len() >= 16 {
                    // Remove oldest batch if buffer is full
                    self.completed_batches.remove(0);
                }
                self.completed_batches.push(batch);
                self.batch_stats.total_batches_created += 1;
            }
        }
        Ok(())
    }
    
    pub fn get_ready_batches(&mut self, current_time: u64) -> alloc::vec::Vec<TelemetryBatch> {
        let mut ready_batches = alloc::vec::Vec::new();
        
        // Check if current batch should be finalized due to timeout
        if let Some(ref batch) = self.current_batch {
            if batch.is_expired(current_time) && batch.packet_count > 0 {
                let _ = self.finalize_current_batch();
            }
        }
        
        // Return completed batches (limit to 4 for processing efficiency)
        let mut batches_to_remove = alloc::vec::Vec::new();
        for (index, batch) in self.completed_batches.iter().enumerate() {
            if ready_batches.len() < 4 {
                ready_batches.push(batch.clone());
                batches_to_remove.push(index);
            } else {
                break;
            }
        }
        
        // Remove batches that were returned (in reverse order to maintain indices)
        for &index in batches_to_remove.iter().rev() {
            self.completed_batches.swap_remove(index);
            self.batch_stats.total_batches_transmitted += 1;
        }
        
        // Update average batch size
        if self.batch_stats.total_batches_transmitted > 0 {
            self.batch_stats.average_batch_size = 
                self.batch_stats.total_packets_batched as f32 / self.batch_stats.total_batches_transmitted as f32;
        }
        
        ready_batches
    }
    
    fn start_new_batch(&mut self, priority: u8, current_time: u64) {
        self.current_batch = Some(TelemetryBatch::new(self.next_batch_id, priority, current_time));
        self.next_batch_id = self.next_batch_id.wrapping_add(1);
    }
    
    pub fn get_stats(&self) -> &BatchingStats {
        &self.batch_stats
    }
    
    pub fn get_current_sequence_number(&self) -> u32 {
        self.sequence_number
    }
    
    pub fn handle_sequence_gap(&mut self, expected_seq: u32, received_seq: u32) {
        if received_seq != expected_seq {
            self.batch_stats.sequence_gaps_detected += 1;
        }
    }
    
    /// Set sequence number for testing purposes
    pub fn set_sequence_number(&mut self, seq: u32) {
        self.sequence_number = seq;
    }
}

#[derive(Debug)]
pub struct TelemetryCollector {
    protocol_handler: ProtocolHandler,
    telemetry_rate_hz: u8,
    last_collection_time: u64,
    packet_counter: u32,
    
    // Preallocated telemetry storage
    telemetry_buffer: Vec<TelemetryPacket, TELEMETRY_BUFFER_SIZE>,
    system_stats: SystemStats,
    
    // Performance tracking
    collection_time_us: u32,
    serialization_time_us: u32,
    
    // Serialized telemetry buffer
    serialized_buffer: alloc::string::String,
    
    // Telemetry sequencing and batching
    batcher: TelemetryBatcher,
    expected_sequence_number: u32,
    sequence_gap_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemStats {
    pub cpu_usage_percent: u8,
    pub memory_usage_percent: u8,
    pub task_switches: u32,
    pub interrupts: u32,
    pub context_switches: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryMetrics {
    pub packets_generated: u32,
    pub packets_transmitted: u32,
    pub packets_dropped: u32,
    pub average_collection_time_us: u32,
    pub average_serialization_time_us: u32,
    pub buffer_utilization_percent: u8,
}

impl TelemetryCollector {
    pub fn new() -> Self {
        Self {
            protocol_handler: ProtocolHandler::new(),
            telemetry_rate_hz: DEFAULT_TELEMETRY_RATE_HZ,
            last_collection_time: 0,
            packet_counter: 0,
            telemetry_buffer: Vec::new(),
            system_stats: SystemStats::new(),
            collection_time_us: 0,
            serialization_time_us: 0,
            serialized_buffer: alloc::string::String::new(),
            batcher: TelemetryBatcher::new(),
            expected_sequence_number: 1,
            sequence_gap_count: 0,
        }
    }
    
    pub fn set_telemetry_rate(&mut self, rate_hz: u8) {
        self.telemetry_rate_hz = rate_hz.clamp(1, 10);
    }
    
    pub fn should_collect(&self, current_time: u64) -> bool {
        let interval_ms = 1000 / self.telemetry_rate_hz as u64;
        current_time >= self.last_collection_time + interval_ms
    }
    
    pub fn collect_telemetry(
        &mut self,
        current_time: u64,
        uptime_seconds: u64,
        safe_mode: bool,
        last_command_id: u32,
        power_system: &PowerSystem,
        thermal_system: &ThermalSystem,
        comms_system: &CommsSystem,
        faults: &[Fault],
    ) -> Result<Option<&str>, &'static str> {
        if !self.should_collect(current_time) {
            return Ok(None);
        }
        
        let start_time = self.get_microseconds();
        
        // Update system statistics
        self.system_stats.update(current_time);
        
        // Create optimized system state for 2kB telemetry packets
        let boot_count = ((uptime_seconds / 86400) as u32 + 1).min(65535) as u16;
        let system_voltage_mv = (3300.0 + ((current_time as f32 * 0.002).cos() * 100.0)) as u16;
        
        let system_state = SystemState {
            safe_mode,
            uptime_seconds,
            cpu_usage_percent: self.system_stats.cpu_usage_percent,
            memory_usage_percent: self.system_stats.memory_usage_percent,
            last_command_id,
            telemetry_rate_hz: self.telemetry_rate_hz,
            
            // Optimized system state for production telemetry
            boot_voltage_pack: ((boot_count as u32) << 16) | (system_voltage_mv as u32),
            last_reset_reason: crate::protocol::ResetReason::PowerOn,
            firmware_hash: 0x5A7B510u32,  // "SATBUS_v1.0" hash
            system_temperature_c: 25 + ((current_time as f32 * 0.001).sin() * 10.0) as i8,
        };
        
        // Collect subsystem states
        let power_state = power_system.get_state();
        let thermal_state = thermal_system.get_state();
        let comms_state = comms_system.get_state();
        
        // Convert faults to alloc Vec
        let fault_vec: alloc::vec::Vec<_> = faults.iter().cloned().collect();
        
        // Create telemetry packet
        let packet = self.protocol_handler.create_telemetry_packet(
            system_state,
            power_state,
            thermal_state,
            comms_state,
            fault_vec,
        );
        
        self.collection_time_us = self.get_microseconds() - start_time;
        
        // Serialize packet
        let serialization_start = self.get_microseconds();
        self.serialized_buffer = match self.protocol_handler.serialize_telemetry(&packet) {
            Ok(s) => s.to_string(),
            Err(_) => return Err("Serialization failed"),
        };
        self.serialization_time_us = self.get_microseconds() - serialization_start;
        
        // Queue packet for batching (high priority for critical systems, normal for telemetry)
        let priority = if safe_mode || !faults.is_empty() {
            TELEMETRY_PRIORITY_HIGH
        } else if uptime_seconds < 300 {  // Low priority for first 5 minutes
            TELEMETRY_PRIORITY_LOW
        } else {
            TELEMETRY_PRIORITY_NORMAL
        };
        
        // Add packet to batcher
        if let Err(_) = self.batcher.queue_packet(packet.clone(), priority, current_time) {
            return Err("Failed to queue packet for batching");
        }
        
        // Store packet in buffer (circular buffer behavior)
        if self.telemetry_buffer.is_full() {
            // Remove oldest entry to make room
            self.telemetry_buffer.remove(0);
        }
        
        if self.telemetry_buffer.push(packet).is_err() {
            return Err("Telemetry buffer full");
        }
        
        self.last_collection_time = current_time;
        self.packet_counter = self.packet_counter.wrapping_add(1);
        
        Ok(Some(&self.serialized_buffer))
    }
    
    pub fn get_telemetry_buffer(&self) -> &[TelemetryPacket] {
        &self.telemetry_buffer
    }
    
    pub fn get_latest_telemetry(&self) -> Option<&TelemetryPacket> {
        self.telemetry_buffer.last()
    }
    
    pub fn get_metrics(&self) -> TelemetryMetrics {
        TelemetryMetrics {
            packets_generated: self.packet_counter,
            packets_transmitted: self.packet_counter, // Assuming all packets are transmitted
            packets_dropped: 0,
            average_collection_time_us: self.collection_time_us,
            average_serialization_time_us: self.serialization_time_us,
            buffer_utilization_percent: ((self.telemetry_buffer.len() * 100) / TELEMETRY_BUFFER_SIZE) as u8,
        }
    }
    
    pub fn clear_buffer(&mut self) {
        self.telemetry_buffer.clear();
        self.packet_counter = 0;
    }
    
    // Telemetry batching and sequencing methods
    
    /// Get ready batches for transmission
    pub fn get_ready_batches(&mut self, current_time: u64) -> alloc::vec::Vec<TelemetryBatch> {
        self.batcher.get_ready_batches(current_time)
    }
    
    /// Force finalization of current batch
    pub fn finalize_current_batch(&mut self) -> Result<(), &'static str> {
        self.batcher.finalize_current_batch()
    }
    
    /// Get batching statistics
    pub fn get_batching_stats(&self) -> &BatchingStats {
        self.batcher.get_stats()
    }
    
    /// Get current sequence number
    pub fn get_current_sequence_number(&self) -> u32 {
        self.batcher.get_current_sequence_number()
    }
    
    /// Validate sequence number and detect gaps
    pub fn validate_sequence_number(&mut self, received_seq: u32) -> bool {
        let is_valid = received_seq == self.expected_sequence_number;
        
        if !is_valid {
            self.sequence_gap_count += 1;
            self.batcher.handle_sequence_gap(self.expected_sequence_number, received_seq);
        }
        
        // Update expected sequence number
        self.expected_sequence_number = (received_seq % MAX_SEQUENCE_NUMBER) + 1;
        
        is_valid
    }
    
    /// Get sequence gap statistics
    pub fn get_sequence_gap_count(&self) -> u32 {
        self.sequence_gap_count
    }
    
    /// Serialize a telemetry batch for transmission
    pub fn serialize_batch(&mut self, batch: &TelemetryBatch) -> Result<alloc::string::String, &'static str> {
        match serde_json::to_string(batch) {
            Ok(serialized) => Ok(serialized),
            Err(_) => Err("Failed to serialize batch"),
        }
    }
    
    /// Create a batch transmission summary for logging
    pub fn create_batch_summary(&self, batch: &TelemetryBatch) -> alloc::string::String {
        alloc::format!(
            "BATCH[{}]: seq={}-{}, packets={}, priority={}, size={}KB", 
            batch.batch_id,
            batch.sequence_start,
            batch.sequence_end,
            batch.packet_count,
            batch.priority,
            batch.size_bytes() / 1024
        )
    }
    
    /// Get mutable reference to batcher for testing
    pub fn get_batcher_mut(&mut self) -> &mut TelemetryBatcher {
        &mut self.batcher
    }
    
    pub fn export_csv_headers(&self) -> &'static str {
        "timestamp,sequence,safe_mode,uptime_s,cpu_pct,mem_pct,\
         batt_mv,batt_ma,solar_mv,solar_ma,charging,batt_pct,\
         core_temp_c,batt_temp_c,heaters_on,heater_power_w,\
         link_up,signal_dbm,data_rate_bps,tx_power_dbm,rx_packets,tx_packets,\
         fault_count"
    }
    
    pub fn export_packet_csv(&self, packet: &TelemetryPacket) -> Result<heapless::String<512>, &'static str> {
        let mut csv_line = heapless::String::new();
        
        // Format CSV line with all telemetry data
        let fault_count = packet.faults.len();
        let csv_string = alloc::format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
            packet.timestamp,
            packet.sequence_number,
            packet.system_state.safe_mode,
            packet.system_state.uptime_seconds,
            packet.system_state.cpu_usage_percent,
            packet.system_state.memory_usage_percent,
            packet.power.battery_voltage_mv,
            packet.power.battery_current_ma,
            packet.power.solar_voltage_mv,
            packet.power.solar_current_ma,
            packet.power.charging,
            packet.power.battery_level_percent,
            packet.thermal.core_temp_c,
            packet.thermal.battery_temp_c,
            packet.thermal.heater_power_w > 0,  // heaters_on encoded in power
            packet.thermal.heater_power_w,
            packet.comms.link_up,
            ((packet.comms.signal_tx_power_dbm >> 8) & 0xFF) as i8,  // signal_strength_dbm
            packet.comms.data_rate_bps,
            (packet.comms.signal_tx_power_dbm & 0xFF) as i8,  // tx_power_dbm
            packet.comms.rx_packets,
            packet.comms.tx_packets,
            fault_count
        );
        
        csv_line.push_str(&csv_string).map_err(|_| "CSV formatting failed")?;
        
        Ok(csv_line)
    }
    
    fn get_microseconds(&self) -> u32 {
        // In real implementation, this would use high-precision timer
        // For simulation, we'll use a simple counter
        self.packet_counter * 1000
    }
}

impl SystemStats {
    pub fn new() -> Self {
        Self {
            cpu_usage_percent: 25,
            memory_usage_percent: 45,
            task_switches: 0,
            interrupts: 0,
            context_switches: 0,
        }
    }
    
    pub fn update(&mut self, current_time: u64) {
        // Simulate realistic system statistics
        let time_factor = (current_time as f32 * 0.001).sin();
        
        // CPU usage varies between 20-80%
        self.cpu_usage_percent = (50.0 + time_factor * 30.0).max(20.0).min(80.0) as u8;
        
        // Memory usage slowly increases over time
        let memory_drift = (current_time as f32 * 0.0001).sin() * 10.0;
        self.memory_usage_percent = (45.0 + memory_drift).max(30.0).min(70.0) as u8;
        
        // Update counters
        self.task_switches = self.task_switches.wrapping_add(1);
        self.interrupts = self.interrupts.wrapping_add(3);
        self.context_switches = self.context_switches.wrapping_add(2);
    }
}

