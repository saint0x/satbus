use satbus::protocol::*;
use satbus::subsystems::*;

fn main() {
    // Create a test telemetry packet
    let mut protocol_handler = ProtocolHandler::new();
    
    let system_state = SystemState {
        safe_mode: false,
        uptime_seconds: 12345,
        cpu_usage_percent: 45,
        memory_usage_percent: 60,
        last_command_id: 100,
        telemetry_rate_hz: 1,
        boot_voltage_pack: (1u32 << 16) | 3300u32,  // boot_count=1, voltage=3300mv
        last_reset_reason: ResetReason::PowerOn,
        firmware_hash: 0x5A7B510u32,
        system_temperature_c: 25,
    };
    
    let power_state = PowerState {
        battery_voltage_mv: 3700,
        battery_current_ma: -200,
        solar_voltage_mv: 4100,
        solar_current_ma: 800,
        charging: true,
        battery_level_percent: 85,
        power_draw_mw: 2500,
    };
    
    let thermal_state = ThermalState {
        core_temp_c: 22,
        battery_temp_c: 18,
        solar_panel_temp_c: 35,
        heater_power_w: 0,  // 0=off (merged heaters_on)
        power_dissipation_w: 15,
    };
    
    let comms_state = CommsState {
        link_up: true,
        signal_tx_power_dbm: ((-85i8 as i16) << 8) | (20i8 as i16),  // signal=-85, tx_power=20
        data_rate_bps: 9600,
        rx_packets: 1500,
        tx_packets: 1200,
        packet_loss_percent: 2,
        queue_depth: 5,
        uplink_active: true,
        downlink_active: false,
    };
    
    let faults = vec![];
    
    let packet = protocol_handler.create_telemetry_packet(
        system_state,
        power_state,
        thermal_state,
        comms_state,
        faults,
    );
    
    // Test serialization and measure size
    match serde_json::to_string(&packet) {
        Ok(json_str) => {
            println!("✅ Telemetry packet serialization successful!");
            println!("📏 Packet size: {} bytes", json_str.len());
            println!("🎯 Target size: 2048 bytes");
            println!("📊 Size ratio: {:.1}%", (json_str.len() as f32 / 2048.0) * 100.0);
            
            if json_str.len() >= 1800 && json_str.len() <= 2200 {
                println!("✅ Packet size is within target range (~2kB)");
            } else if json_str.len() < 1800 {
                println!("⚠️  Packet size is below target - need more data");
            } else {
                println!("⚠️  Packet size exceeds target - too much data");
            }
            
            // Show first 200 chars of JSON for inspection
            if json_str.len() > 200 {
                println!("\n📄 JSON preview (first 200 chars):");
                println!("{}", &json_str[..200]);
                println!("...");
            } else {
                println!("\n📄 Full JSON:");
                println!("{}", json_str);
            }
        }
        Err(e) => {
            println!("❌ Serialization failed: {}", e);
        }
    }
}