use satbus::*;
use satbus::protocol::*;

#[test]
fn test_command_tracking_lifecycle() {
    let mut handler = ProtocolHandler::new();
    let current_time = 1000;
    
    // Track a command
    let result = handler.track_command(123, current_time, 5000);
    assert!(result.is_ok());
    
    // Verify initial status
    let tracker = handler.get_command_status(123);
    assert!(tracker.is_some());
    assert!(matches!(tracker.unwrap().status, ResponseStatus::Acknowledged));
    
    // Update to execution started
    let result = handler.update_command_status(123, ResponseStatus::ExecutionStarted, current_time + 100);
    assert!(result.is_ok());
    
    let tracker = handler.get_command_status(123);
    assert!(tracker.is_some());
    assert!(matches!(tracker.unwrap().status, ResponseStatus::ExecutionStarted));
    assert!(tracker.unwrap().execution_start_time.is_some());
    
    // Complete successfully
    let result = handler.update_command_status(123, ResponseStatus::Success, current_time + 500);
    assert!(result.is_ok());
    
    let tracker = handler.get_command_status(123);
    assert!(tracker.is_some());
    assert!(matches!(tracker.unwrap().status, ResponseStatus::Success));
}

#[test]
fn test_command_timeout_handling() {
    let mut handler = ProtocolHandler::new();
    let current_time = 1000;
    
    // Track a command with short timeout
    let result = handler.track_command(456, current_time, 1000); // 1 second timeout
    assert!(result.is_ok());
    
    // Command should exist initially
    let tracker = handler.get_command_status(456);
    assert!(tracker.is_some());
    
    // After timeout period, cleanup should remove it
    handler.cleanup_expired_commands(current_time + 2000);
    
    let tracker = handler.get_command_status(456);
    assert!(tracker.is_none());
}

#[test]
fn test_duplicate_command_rejection() {
    let mut handler = ProtocolHandler::new();
    let current_time = 1000;
    
    // Track first command
    let result = handler.track_command(789, current_time, 5000);
    assert!(result.is_ok());
    
    // Attempt to track same command ID should fail
    let result = handler.track_command(789, current_time, 5000);
    assert!(result.is_err());
}

#[test]
fn test_ack_nack_response_creation() {
    let mut handler = ProtocolHandler::new();
    
    // Test ACK response
    let ack_response = handler.create_ack_response(100, Some("Command received"));
    assert_eq!(ack_response.id, 100);
    assert!(matches!(ack_response.status, ResponseStatus::Acknowledged));
    assert!(ack_response.message.is_some());
    
    // Test NACK response
    let nack_response = handler.create_nack_response(101, "Invalid parameter");
    assert_eq!(nack_response.id, 101);
    assert!(matches!(nack_response.status, ResponseStatus::NegativeAck));
    assert!(nack_response.message.is_some());
    assert!(nack_response.message.unwrap().contains("Invalid parameter"));
    
    // Test execution started response
    let exec_response = handler.create_execution_started_response(102);
    assert_eq!(exec_response.id, 102);
    assert!(matches!(exec_response.status, ResponseStatus::ExecutionStarted));
    
    // Test execution failed response
    let fail_response = handler.create_execution_failed_response(103, "Subsystem error");
    assert_eq!(fail_response.id, 103);
    assert!(matches!(fail_response.status, ResponseStatus::ExecutionFailed));
    
    // Test timeout response
    let timeout_response = handler.create_timeout_response(104);
    assert_eq!(timeout_response.id, 104);
    assert!(matches!(timeout_response.status, ResponseStatus::Timeout));
}

#[test]
fn test_satellite_agent_ack_nack_integration() {
    let mut agent = SatelliteAgent::new();
    agent.start();
    
    // Create a valid ping command
    let ping_command = Command {
        id: 200,
        timestamp: 1000,
        command_type: CommandType::Ping,
        execution_time: None,
    };
    
    // Queue and process the command
    let result = agent.queue_command(ping_command);
    assert!(result.is_ok());
    
    // Process commands
    let result = agent.process_commands();
    assert!(result.is_ok());
    
    // Check that command is being tracked
    let tracked_commands = agent.get_tracked_commands();
    assert!(!tracked_commands.is_empty());
    
    let ping_tracker = tracked_commands.iter().find(|t| t.command_id == 200);
    assert!(ping_tracker.is_some());
    
    // Should have gone through the lifecycle: Acknowledged -> ExecutionStarted -> Success
    assert!(matches!(ping_tracker.unwrap().status, ResponseStatus::Success));
}

#[test]
fn test_invalid_command_nack() {
    let mut agent = SatelliteAgent::new();
    agent.start();
    
    // Create an invalid command (zero ID)
    let invalid_command = Command {
        id: 0, // Invalid ID
        timestamp: 1000,
        command_type: CommandType::Ping,
        execution_time: None,
    };
    
    // Queue and process the command
    let result = agent.queue_command(invalid_command);
    assert!(result.is_ok());
    
    // Process commands
    let result = agent.process_commands();
    assert!(result.is_ok());
    
    // Check responses for NACK
    let responses = agent.get_responses();
    assert!(!responses.is_empty());
    
    let nack_response = responses.iter().find(|r| r.id == 0);
    assert!(nack_response.is_some());
    assert!(matches!(nack_response.unwrap().status, ResponseStatus::NegativeAck));
}

#[test]
fn test_safe_mode_command_nack() {
    let mut agent = SatelliteAgent::new();
    agent.start();
    
    // Force safe mode
    let safe_mode_command = Command {
        id: 300,
        timestamp: 1000,
        command_type: CommandType::SetSafeMode { enabled: true },
        execution_time: None,
    };
    
    let result = agent.queue_command(safe_mode_command);
    assert!(result.is_ok());
    
    let result = agent.process_commands();
    assert!(result.is_ok());
    
    // Now try a command that should be blocked in safe mode
    let blocked_command = Command {
        id: 301,
        timestamp: 1100,
        command_type: CommandType::SetHeaterState { on: true },
        execution_time: None,
    };
    
    let result = agent.queue_command(blocked_command);
    assert!(result.is_ok());
    
    let result = agent.process_commands();
    assert!(result.is_ok());
    
    // Check for NACK response
    let responses = agent.get_responses();
    let blocked_response = responses.iter().find(|r| r.id == 301);
    assert!(blocked_response.is_some());
    assert!(matches!(blocked_response.unwrap().status, ResponseStatus::NegativeAck));
    assert!(blocked_response.unwrap().message.as_ref().unwrap().contains("safe mode"));
}