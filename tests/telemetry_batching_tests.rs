use satbus::*;
use satbus::telemetry::*;
use satbus::protocol::*;
use satbus::subsystems::*;

#[test]
fn test_telemetry_batcher_basic_functionality() {
    let mut batcher = TelemetryBatcher::new();
    let current_time = 1000;
    
    // Create a test telemetry packet
    let packet = create_test_telemetry_packet(1);
    
    // Queue packet
    let result = batcher.queue_packet(packet, TELEMETRY_PRIORITY_NORMAL, current_time);
    assert!(result.is_ok());
    
    // Check sequence number assignment
    assert_eq!(batcher.get_current_sequence_number(), 2); // Should be incremented
    
    // Finalize batch
    let result = batcher.finalize_current_batch();
    assert!(result.is_ok());
    
    // Get ready batches
    let batches = batcher.get_ready_batches(current_time);
    assert_eq!(batches.len(), 1);
    
    let batch = &batches[0];
    assert_eq!(batch.packet_count, 1);
    assert_eq!(batch.sequence_start, 1);
    assert_eq!(batch.sequence_end, 1);
    assert_eq!(batch.priority, TELEMETRY_PRIORITY_NORMAL);
}

#[test]
fn test_telemetry_batch_sequencing() {
    let mut batcher = TelemetryBatcher::new();
    let current_time = 1000;
    
    // Queue multiple packets
    for i in 0..5 {
        let packet = create_test_telemetry_packet(i + 1);
        let result = batcher.queue_packet(packet, TELEMETRY_PRIORITY_NORMAL, current_time);
        assert!(result.is_ok());
    }
    
    // Finalize batch
    let result = batcher.finalize_current_batch();
    assert!(result.is_ok());
    
    // Get ready batches
    let batches = batcher.get_ready_batches(current_time);
    assert_eq!(batches.len(), 1);
    
    let batch = &batches[0];
    assert_eq!(batch.packet_count, 5);
    assert_eq!(batch.sequence_start, 1);
    assert_eq!(batch.sequence_end, 5);
    
    // Verify sequence numbers in batch
    for (i, sequenced_packet) in batch.packets.iter().enumerate() {
        assert_eq!(sequenced_packet.packet.sequence_number, (i + 1) as u32);
    }
}

#[test]
fn test_telemetry_batch_size_limit() {
    let mut batcher = TelemetryBatcher::new();
    let current_time = 1000;
    
    // Queue maximum number of packets (8 = MAX_BATCH_SIZE)
    for i in 0..8 {
        let packet = create_test_telemetry_packet(i + 1);
        let result = batcher.queue_packet(packet, TELEMETRY_PRIORITY_NORMAL, current_time);
        assert!(result.is_ok());
    }
    
    // Queue one more packet - should create a new batch automatically
    let packet = create_test_telemetry_packet(9);
    let result = batcher.queue_packet(packet, TELEMETRY_PRIORITY_NORMAL, current_time);
    assert!(result.is_ok());
    
    // Should have 10 total packets batched but in 2 batches
    let stats = batcher.get_stats();
    assert_eq!(stats.total_packets_batched, 9);
}

#[test]
fn test_telemetry_batch_timeout() {
    let mut batcher = TelemetryBatcher::new();
    let current_time = 1000;
    
    // Queue a packet
    let packet = create_test_telemetry_packet(1);
    let result = batcher.queue_packet(packet, TELEMETRY_PRIORITY_NORMAL, current_time);
    assert!(result.is_ok());
    
    // Check that no batches are ready yet
    let batches = batcher.get_ready_batches(current_time);
    assert_eq!(batches.len(), 0);
    
    // Advance time past timeout
    let future_time = current_time + 6000; // BATCH_TIMEOUT_MS is 5000
    let batches = batcher.get_ready_batches(future_time);
    assert_eq!(batches.len(), 1);
    
    let batch = &batches[0];
    assert_eq!(batch.packet_count, 1);
}

#[test]
fn test_telemetry_priority_handling() {
    let mut batcher = TelemetryBatcher::new();
    let current_time = 1000;
    
    // Queue high priority packet
    let packet1 = create_test_telemetry_packet(1);
    let result = batcher.queue_packet(packet1, TELEMETRY_PRIORITY_HIGH, current_time);
    assert!(result.is_ok());
    
    // Queue normal priority packet (should create new batch due to priority change)
    let packet2 = create_test_telemetry_packet(2);
    let result = batcher.queue_packet(packet2, TELEMETRY_PRIORITY_NORMAL, current_time);
    assert!(result.is_ok());
    
    // Finalize to get batches
    let result = batcher.finalize_current_batch();
    assert!(result.is_ok());
    
    let batches = batcher.get_ready_batches(current_time);
    assert!(batches.len() >= 1);
}

#[test]
fn test_telemetry_collector_integration() {
    let mut collector = TelemetryCollector::new();
    let current_time = 1000;
    
    // Create test subsystems
    let power_system = PowerSystem::new();
    let thermal_system = ThermalSystem::new();
    let comms_system = CommsSystem::new();
    let faults = vec![];
    
    // Collect telemetry (should queue packet for batching)
    let result = collector.collect_telemetry(
        current_time,
        10, // uptime_seconds
        false, // safe_mode
        123, // last_command_id
        &power_system,
        &thermal_system,
        &comms_system,
        &faults,
    );
    assert!(result.is_ok());
    
    // Check that packet was queued for batching
    let batches = collector.get_ready_batches(current_time + 6000); // Past timeout
    assert!(!batches.is_empty());
    
    let batch = &batches[0];
    assert_eq!(batch.packet_count, 1);
    // During first 5 minutes (uptime < 300), priority should be LOW
    assert_eq!(batch.priority, TELEMETRY_PRIORITY_LOW);
}

#[test]
fn test_telemetry_sequence_number_validation() {
    let mut collector = TelemetryCollector::new();
    
    // Test valid sequence
    assert!(collector.validate_sequence_number(1));
    assert!(collector.validate_sequence_number(2));
    assert!(collector.validate_sequence_number(3));
    
    // Test sequence gap
    assert!(!collector.validate_sequence_number(5)); // Gap: expected 4, got 5
    assert_eq!(collector.get_sequence_gap_count(), 1);
    
    // Continue sequence
    assert!(collector.validate_sequence_number(6));
}

#[test]
fn test_telemetry_batching_stats() {
    let mut batcher = TelemetryBatcher::new();
    let current_time = 1000;
    
    // Queue several packets
    for i in 0..3 {
        let packet = create_test_telemetry_packet(i + 1);
        let result = batcher.queue_packet(packet, TELEMETRY_PRIORITY_NORMAL, current_time);
        assert!(result.is_ok());
    }
    
    // Finalize and get batches
    let result = batcher.finalize_current_batch();
    assert!(result.is_ok());
    
    let _batches = batcher.get_ready_batches(current_time);
    
    // Check stats
    let stats = batcher.get_stats();
    assert_eq!(stats.total_packets_batched, 3);
    assert_eq!(stats.total_batches_created, 1);
    assert_eq!(stats.total_batches_transmitted, 1);
    assert_eq!(stats.average_batch_size, 3.0);
}

#[test]
fn test_telemetry_sequence_number_wraparound() {
    let mut batcher = TelemetryBatcher::new();
    let current_time = 1000;
    
    // Set sequence number near max
    batcher.set_sequence_number(65535); // MAX_SEQUENCE_NUMBER
    
    // Queue a packet - should wrap to 1
    let packet = create_test_telemetry_packet(1);
    let result = batcher.queue_packet(packet, TELEMETRY_PRIORITY_NORMAL, current_time);
    assert!(result.is_ok());
    
    // Next sequence number should be 1 (wrapped)
    assert_eq!(batcher.get_current_sequence_number(), 1);
}

#[test]
fn test_telemetry_batch_checksum() {
    let mut batch = TelemetryBatch::new(1, TELEMETRY_PRIORITY_NORMAL, 1000);
    
    let initial_checksum = batch.checksum;
    
    // Add a packet
    let sequenced_packet = SequencedTelemetryPacket {
        packet: create_test_telemetry_packet(1),
        priority: TELEMETRY_PRIORITY_NORMAL,
        batch_id: 0,
        created_at: 1000,
        retransmit_count: 0,
    };
    
    let result = batch.add_packet(sequenced_packet);
    assert!(result.is_ok());
    
    // Checksum should have changed
    assert_ne!(batch.checksum, initial_checksum);
}

// Helper function to create test telemetry packets
fn create_test_telemetry_packet(id: u32) -> TelemetryPacket {
    let system_state = SystemState {
        safe_mode: false,
        uptime_seconds: 10,
        cpu_usage_percent: 50,
        memory_usage_percent: 40,
        last_command_id: id,
        telemetry_rate_hz: 1,
        boot_voltage_pack: 0x12345678,
        last_reset_reason: ResetReason::PowerOn,
        firmware_hash: 0x5A7B510,
        system_temperature_c: 25,
    };
    
    let power_state = PowerState {
        battery_voltage_mv: 3700,
        battery_current_ma: 100,
        solar_voltage_mv: 2940,
        solar_current_ma: 560,
        charging: true,
        battery_level_percent: 75,
        power_draw_mw: 1850,
    };
    
    let thermal_state = ThermalState {
        core_temp_c: 20,
        battery_temp_c: 22,
        solar_panel_temp_c: 127,
        heater_power_w: 0,
        power_dissipation_w: 25,
    };
    
    let comms_state = CommsState {
        link_up: true,
        signal_tx_power_dbm: 30720, // Packed value
        data_rate_bps: 9600,
        rx_packets: 10,
        tx_packets: 5,
        packet_loss_percent: 0,
        queue_depth: 0,
        uplink_active: true,
        downlink_active: true,
    };
    
    TelemetryPacket {
        timestamp: 1000,
        sequence_number: id,
        system_state,
        power: power_state,
        thermal: thermal_state,
        comms: comms_state,
        faults: vec![],
        performance_history: [
            PerformanceSnapshot {
                timestamp: 0,
                loop_time_us: 800,
                memory_free_kb: 1024,
                cpu_load_percent: 25,
                task_count: 8,
            },
            PerformanceSnapshot {
                timestamp: 1,
                loop_time_us: 850,
                memory_free_kb: 974,
                cpu_load_percent: 30,
                task_count: 9,
            },
            PerformanceSnapshot {
                timestamp: 2,
                loop_time_us: 900,
                memory_free_kb: 924,
                cpu_load_percent: 35,
                task_count: 10,
            },
            PerformanceSnapshot {
                timestamp: 3,
                loop_time_us: 950,
                memory_free_kb: 874,
                cpu_load_percent: 40,
                task_count: 11,
            },
        ],
        safety_events: vec![],
        subsystem_diagnostics: SubsystemDiagnostics {
            health_scores: 0x5F5A5C00, // Bit-packed health scores
            cycle_counts: [10, 20, 30],
            last_error_codes: [1, 2, 64, 128],
            diagnostic_data: vec![0x55; 16],
        },
        mission_data: MissionData {
            mission_elapsed_time_s: 100,
            orbit_number: 1,
            ground_contact_count: 5,
            data_downlinked_kb: 1024,
            commands_received: 10,
            mission_phase: MissionPhase::Nominal,
            next_scheduled_event: 2000,
            payload_status: PayloadStatus::Active,
        },
        orbital_data: OrbitalData {
            altitude_km: 408,
            velocity_ms: 7800,
            inclination_deg: 98,
            latitude_deg: 45,
            longitude_deg: 32768,
            sun_angle_deg: 180,
            eclipse_duration_s: 0,
            magnetic_field_nt: [2500, 1500, 4500],
            angular_velocity: [100, -50, 20],
            attitude_quat_xyz: [0, 0, 23166],
        },
        padding: vec![0x42; 64],
    }
}