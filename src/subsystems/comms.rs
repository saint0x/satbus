use super::{Subsystem, FaultType};
use serde::{Deserialize, Serialize};
use heapless::spsc::Queue;
use arrayvec::ArrayString;

const MAX_DOWNLINK_QUEUE: usize = 32;
const MAX_MESSAGE_SIZE: usize = 256;
const NOMINAL_SIGNAL_STRENGTH: i8 = -80;
const CRITICAL_SIGNAL_STRENGTH: i8 = -120;

type MessageBuffer = ArrayString<MAX_MESSAGE_SIZE>;
type DownlinkQueue = Queue<MessageBuffer, MAX_DOWNLINK_QUEUE>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommsState {
    pub link_up: bool,
    pub signal_tx_power_dbm: i16,    // Packed: signal_strength_dbm (8bit) + tx_power_dbm (8bit)
    pub data_rate_bps: u32,
    pub rx_packets: u32,
    pub tx_packets: u32,
    pub packet_loss_percent: u8,
    pub queue_depth: usize,
    pub uplink_active: bool,
    pub downlink_active: bool,
}

#[derive(Debug, Clone)]
pub enum CommsCommand {
    SetLinkState(bool),
    SetTxPower(i8),
    SetDataRate(u32),
    TransmitMessage(ArrayString<MAX_MESSAGE_SIZE>),
    FlushQueue,
}

#[derive(Debug)]
pub struct CommsSystem {
    state: CommsState,
    fault_state: Option<FaultType>,
    
    // Preallocated communication buffers
    downlink_queue: DownlinkQueue,
    #[allow(dead_code)]
    uplink_buffer: MessageBuffer,
    
    // RF simulation parameters
    antenna_gain_db: i8,
    path_loss_db: u8,
    noise_floor_dbm: i8,
    
    // Performance tracking
    bit_error_rate: f32,
    last_packet_time: u32,
}

impl CommsSystem {
    // Helper methods for packed field access
    fn get_signal_strength_dbm(&self) -> i8 {
        ((self.state.signal_tx_power_dbm >> 8) & 0xFF) as i8
    }
    
    fn get_tx_power_dbm(&self) -> i8 {
        (self.state.signal_tx_power_dbm & 0xFF) as i8
    }
    
    fn set_signal_strength_dbm(&mut self, value: i8) {
        self.state.signal_tx_power_dbm = ((value as i16) << 8) | (self.state.signal_tx_power_dbm & 0xFF);
    }
    
    fn set_tx_power_dbm(&mut self, value: i8) {
        self.state.signal_tx_power_dbm = (self.state.signal_tx_power_dbm & 0xFF00u16 as i16) | (value as i16);
    }
    
    pub fn new() -> Self {
        Self {
            state: CommsState {
                link_up: true,
                signal_tx_power_dbm: ((NOMINAL_SIGNAL_STRENGTH as i16) << 8) | (20i16),  // signal + tx_power packed
                data_rate_bps: 9600,
                rx_packets: 0,
                tx_packets: 0,
                packet_loss_percent: 0,
                queue_depth: 0,
                uplink_active: false,
                downlink_active: false,
            },
            fault_state: None,
            downlink_queue: Queue::new(),
            uplink_buffer: ArrayString::new(),
            antenna_gain_db: 3,
            path_loss_db: 140,
            noise_floor_dbm: -110,
            bit_error_rate: 0.0001,
            last_packet_time: 0,
        }
    }
    
    fn calculate_link_budget(&self) -> i8 {
        // Simplified link budget calculation
        let eirp_dbm = self.get_tx_power_dbm().saturating_add(self.antenna_gain_db);
        let received_power = eirp_dbm.saturating_sub(self.path_loss_db as i8).saturating_add(self.antenna_gain_db);
        received_power
    }
    
    fn simulate_rf_environment(&mut self, _dt_ms: u16) {
        // Simulate atmospheric and ionospheric effects
        let time_factor = (self.last_packet_time as f32 * 0.001).sin();
        let atmospheric_loss = 2.0 + time_factor.abs() * 5.0;
        
        // Calculate signal strength
        let base_signal = self.calculate_link_budget();
        self.set_signal_strength_dbm(base_signal.saturating_sub(atmospheric_loss as i8));
        
        // Update link state based on signal strength
        if self.get_signal_strength_dbm() < CRITICAL_SIGNAL_STRENGTH {
            self.state.link_up = false;
        } else {
            self.state.link_up = true;
        }
        
        // Calculate bit error rate based on SNR
        let snr = self.get_signal_strength_dbm().saturating_sub(self.noise_floor_dbm);
        self.bit_error_rate = if snr > 10 {
            0.0001
        } else if snr > 5 {
            0.001
        } else {
            0.01
        };
        
        // Update packet loss percentage
        self.state.packet_loss_percent = (self.bit_error_rate * 100.0).min(99.0) as u8;
        
        // NASA Rule 5: Safety assertions for communications invariants
        debug_assert!(
            self.get_signal_strength_dbm() >= -128,
            "Signal strength {} below i8 minimum", 
            self.get_signal_strength_dbm()
        );
        debug_assert!(
            self.get_tx_power_dbm() >= 0 && self.get_tx_power_dbm() <= 30,
            "TX power {} out of valid range 0-30 dBm", 
            self.get_tx_power_dbm()
        );
        debug_assert!(
            self.state.packet_loss_percent <= 100,
            "Packet loss {} exceeds 100%", 
            self.state.packet_loss_percent
        );
        debug_assert!(
            self.bit_error_rate >= 0.0 && self.bit_error_rate <= 1.0,
            "Bit error rate {} out of valid range 0.0-1.0", 
            self.bit_error_rate
        );
        debug_assert!(
            self.state.data_rate_bps > 0,
            "Data rate {} must be positive", 
            self.state.data_rate_bps
        );
        
        // Adaptive data rate based on link quality
        if self.get_signal_strength_dbm() > -90 {
            self.state.data_rate_bps = 19200;
        } else if self.get_signal_strength_dbm() > -100 {
            self.state.data_rate_bps = 9600;
        } else {
            self.state.data_rate_bps = 4800;
        }
    }
    
    fn process_downlink_queue(&mut self, dt_ms: u16) -> Result<(), FaultType> {
        if !self.state.link_up {
            return Ok(());
        }
        
        // Process one message per update cycle if queue not empty
        if let Some(_message) = self.downlink_queue.dequeue() {
            self.state.tx_packets = self.state.tx_packets.saturating_add(1);
            self.state.downlink_active = true;
            
            // Simulate transmission time
            self.last_packet_time = self.last_packet_time.saturating_add(dt_ms as u32);
        } else {
            self.state.downlink_active = false;
        }
        
        // Update queue depth
        self.state.queue_depth = self.downlink_queue.len();
        
        // Check for queue overflow
        if self.state.queue_depth >= MAX_DOWNLINK_QUEUE - 2 {
            return Err(FaultType::Degraded);
        }
        
        Ok(())
    }
    
    fn simulate_uplink_activity(&mut self, _dt_ms: u16) {
        // Simulate periodic uplink activity
        let uplink_probability = if self.state.link_up { 0.1 } else { 0.0 };
        
        if (self.last_packet_time % 100) < (uplink_probability * 100.0) as u32 {
            self.state.uplink_active = true;
            self.state.rx_packets = self.state.rx_packets.saturating_add(1);
        } else {
            self.state.uplink_active = false;
        }
    }
    
    fn queue_telemetry_message(&mut self, message: &str) -> Result<(), &'static str> {
        let mut buffer = ArrayString::new();
        if buffer.try_push_str(message).is_err() {
            return Err("Message too long");
        }
        
        if self.downlink_queue.enqueue(buffer).is_err() {
            return Err("Queue full");
        }
        
        Ok(())
    }
}

impl Subsystem for CommsSystem {
    type State = CommsState;
    type Command = CommsCommand;
    
    fn update(&mut self, dt_ms: u16) -> Result<(), FaultType> {
        if let Some(fault) = self.fault_state {
            match fault {
                FaultType::Failed => {
                    self.state.link_up = false;
                    return Err(fault);
                }
                FaultType::Degraded => {
                    // Reduced performance in degraded mode
                    let current_tx_power = self.get_tx_power_dbm();
                    self.set_tx_power_dbm(current_tx_power.saturating_sub(6));
                    self.antenna_gain_db = self.antenna_gain_db.saturating_sub(2);
                }
                FaultType::Offline => {
                    self.state.link_up = false;
                    return Err(fault);
                }
            }
        }
        
        // Simulate RF environment
        self.simulate_rf_environment(dt_ms);
        
        // Process communication queues
        self.process_downlink_queue(dt_ms)?;
        self.simulate_uplink_activity(dt_ms);
        
        // Auto-generate telemetry messages
        if self.state.link_up && (self.last_packet_time % 5000) < dt_ms as u32 {
            let _ = self.queue_telemetry_message("HEARTBEAT");
        }
        
        Ok(())
    }
    
    fn execute_command(&mut self, command: Self::Command) -> Result<(), &'static str> {
        match command {
            CommsCommand::SetLinkState(enabled) => {
                if enabled && self.fault_state.is_none() {
                    self.state.link_up = true;
                } else {
                    self.state.link_up = false;
                }
                Ok(())
            }
            CommsCommand::SetTxPower(power_dbm) => {
                if power_dbm >= 0 && power_dbm <= 30 {
                    self.set_tx_power_dbm(power_dbm);
                    Ok(())
                } else {
                    Err("Invalid power level")
                }
            }
            CommsCommand::SetDataRate(rate) => {
                if rate >= 1200 && rate <= 38400 {
                    self.state.data_rate_bps = rate;
                    Ok(())
                } else {
                    Err("Invalid data rate")
                }
            }
            CommsCommand::TransmitMessage(message) => {
                if self.downlink_queue.enqueue(message).is_err() {
                    Err("Queue full")
                } else {
                    Ok(())
                }
            }
            CommsCommand::FlushQueue => {
                while self.downlink_queue.dequeue().is_some() {}
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
        self.set_tx_power_dbm(20);
        self.antenna_gain_db = 3;
    }
    
    fn is_healthy(&self) -> bool {
        self.fault_state.is_none() && 
        self.state.link_up &&
        self.get_signal_strength_dbm() > CRITICAL_SIGNAL_STRENGTH &&
        self.state.packet_loss_percent < 50
    }
}