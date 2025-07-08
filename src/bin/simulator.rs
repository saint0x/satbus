use satbus::agent::SatelliteAgent;
use satbus::protocol::{Command, CommandResponse};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, Mutex};
use tokio::time;
use tracing::{error, info, warn};

const TCP_PORT: u16 = 8080;
const TELEMETRY_BROADCAST_BUFFER_SIZE: usize = 256;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();
    
    println!("🛰️  Mock Satellite Bus Simulator");
    println!("================================");
    
    // Create and start satellite agent
    let agent = Arc::new(Mutex::new(SatelliteAgent::new()));
    {
        let mut agent_guard = agent.lock().await;
        agent_guard.start();
    }
    
    // Create broadcast channel for telemetry
    let (telemetry_tx, _) = broadcast::channel(TELEMETRY_BROADCAST_BUFFER_SIZE);
    
    // Start TCP server
    let tcp_agent = Arc::clone(&agent);
    let tcp_telemetry_tx = telemetry_tx.clone();
    let tcp_server = tokio::spawn(async move {
        if let Err(e) = start_tcp_server(tcp_agent, tcp_telemetry_tx).await {
            error!("TCP server error: {}", e);
        }
    });
    
    // Main simulation loop - Production rate: 1 Hz (1000ms) per production specs
    let mut interval = time::interval(Duration::from_millis(1000));
    
    loop {
        interval.tick().await;
        
        let telemetry_result = {
            let mut agent_guard = agent.lock().await;
            agent_guard.update()
        };
        
        match telemetry_result {
            Ok(Some(telemetry)) => {
                // Broadcast telemetry to all connected clients
                if let Err(e) = telemetry_tx.send(telemetry.clone()) {
                    warn!("Failed to broadcast telemetry: {}", e);
                }
                info!("📡 TELEMETRY: {}", telemetry);
            }
            Ok(None) => {
                // No telemetry this cycle
            }
            Err(e) => {
                error!("❌ Agent error: {}", e);
                break;
            }
        }
        
        // Check for shutdown signal (Ctrl+C)
        let running = {
            let agent_guard = agent.lock().await;
            agent_guard.get_state().running
        };
        
        if !running {
            break;
        }
    }
    
    {
        let mut agent_guard = agent.lock().await;
        agent_guard.stop();
    }
    
    tcp_server.abort();
    println!("🚀 Satellite Bus Simulator stopped");
    
    Ok(())
}

async fn start_tcp_server(
    agent: Arc<Mutex<SatelliteAgent>>,
    telemetry_tx: broadcast::Sender<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind(format!("127.0.0.1:{}", TCP_PORT)).await?;
    info!("🌐 TCP server listening on port {}", TCP_PORT);
    
    loop {
        match listener.accept().await {
            Ok((stream, addr)) => {
                info!("🔗 New client connected: {}", addr);
                let client_agent = Arc::clone(&agent);
                let client_telemetry_rx = telemetry_tx.subscribe();
                
                tokio::spawn(async move {
                    if let Err(e) = handle_client(stream, client_agent, client_telemetry_rx).await {
                        warn!("Client {} error: {}", addr, e);
                    }
                    info!("🔌 Client {} disconnected", addr);
                });
            }
            Err(e) => {
                error!("Failed to accept connection: {}", e);
            }
        }
    }
}

async fn handle_client(
    stream: TcpStream,
    agent: Arc<Mutex<SatelliteAgent>>,
    mut telemetry_rx: broadcast::Receiver<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let (reader, writer) = stream.into_split();
    let mut buf_reader = BufReader::new(reader);
    
    // Wrap writer in Arc<Mutex<>> for sharing
    let writer = Arc::new(Mutex::new(writer));
    
    // Spawn telemetry streaming task
    let telemetry_writer = Arc::clone(&writer);
    let telemetry_task = tokio::spawn(async move {
        while let Ok(telemetry) = telemetry_rx.recv().await {
            let mut writer_guard = telemetry_writer.lock().await;
            if let Err(e) = writer_guard.write_all(telemetry.as_bytes()).await {
                warn!("Failed to send telemetry: {}", e);
                break;
            }
            if let Err(e) = writer_guard.write_all(b"\n").await {
                warn!("Failed to send telemetry newline: {}", e);
                break;
            }
        }
    });
    
    // Process commands from client
    let mut line = String::new();
    loop {
        line.clear();
        match buf_reader.read_line(&mut line).await {
            Ok(0) => break, // Client disconnected
            Ok(_) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                
                // Parse command
                match serde_json::from_str::<Command>(trimmed) {
                    Ok(command) => {
                        info!("📨 Received command: {:?}", command);
                        
                        // Execute command synchronously
                        let response = {
                            let mut agent_guard = agent.lock().await;
                            match agent_guard.queue_command(command.clone()) {
                                Ok(()) => {
                                    // Process commands immediately to get the response
                                    if let Err(e) = agent_guard.process_commands() {
                                        error!("Command processing error: {}", e);
                                        CommandResponse {
                                            id: command.id,
                                            timestamp: std::time::SystemTime::now()
                                                .duration_since(std::time::UNIX_EPOCH)
                                                .unwrap()
                                                .as_millis() as u64,
                                            status: satbus::protocol::ResponseStatus::Error,
                                            message: Some(format!("Processing error: {}", e)),
                                        }
                                    } else {
                                        // Get the response for this command
                                        let responses = agent_guard.get_responses();
                                        if let Some(response) = responses.iter().find(|r| r.id == command.id) {
                                            response.clone()
                                        } else {
                                            // Create a default success response
                                            CommandResponse {
                                                id: command.id,
                                                timestamp: std::time::SystemTime::now()
                                                    .duration_since(std::time::UNIX_EPOCH)
                                                    .unwrap()
                                                    .as_millis() as u64,
                                                status: satbus::protocol::ResponseStatus::Success,
                                                message: None,
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    error!("Command queue error: {}", e);
                                    CommandResponse {
                                        id: command.id,
                                        timestamp: std::time::SystemTime::now()
                                            .duration_since(std::time::UNIX_EPOCH)
                                            .unwrap()
                                            .as_millis() as u64,
                                        status: satbus::protocol::ResponseStatus::Error,
                                        message: Some(format!("Queue error: {}", e)),
                                    }
                                }
                            }
                        };
                        
                        // Send response
                        let response_json = serde_json::to_string(&response)?;
                        {
                            let mut writer_guard = writer.lock().await;
                            writer_guard.write_all(response_json.as_bytes()).await?;
                            writer_guard.write_all(b"\n").await?;
                        }
                        info!("📤 Sent response: {}", response_json);
                    }
                    Err(e) => {
                        error!("Failed to parse command: {}", e);
                        let error_response = serde_json::json!({
                            "id": 0,
                            "timestamp": std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_millis() as u64,
                            "status": "ParseError",
                            "message": format!("Invalid command format: {}", e)
                        });
                        {
                            let mut writer_guard = writer.lock().await;
                            writer_guard.write_all(error_response.to_string().as_bytes()).await?;
                            writer_guard.write_all(b"\n").await?;
                        }
                    }
                }
            }
            Err(e) => {
                error!("Error reading from client: {}", e);
                break;
            }
        }
    }
    
    telemetry_task.abort();
    Ok(())
}
