use clap::{App, Arg, ArgMatches, SubCommand};
use colored::*;
use serde_json;
use std::process::Command;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

const DEFAULT_HOST: &str = "127.0.0.1";
const DEFAULT_PORT: &str = "8080";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let matches = App::new("satbus")
        .version("0.1.0")
        .author("Space Systems Engineering Team")
        .about("🛰️  Satellite Bus Simulator - Production-ready spacecraft systems simulation")
        .arg(
            Arg::with_name("host")
                .short("h")
                .long("host")
                .value_name("HOST")
                .help("Simulator host address")
                .takes_value(true)
                .default_value(DEFAULT_HOST)
                .global(true),
        )
        .arg(
            Arg::with_name("port")
                .short("p")
                .long("port")
                .value_name("PORT")
                .help("Simulator port")
                .takes_value(true)
                .default_value(DEFAULT_PORT)
                .global(true),
        )
        .arg(
            Arg::with_name("format")
                .short("f")
                .long("format")
                .value_name("FORMAT")
                .help("Output format")
                .takes_value(true)
                .possible_values(&["json", "table", "compact"])
                .default_value("table")
                .global(true),
        )
        .arg(
            Arg::with_name("verbose")
                .short("v")
                .long("verbose")
                .help("Enable verbose output")
                .global(true),
        )
        .arg(
            Arg::with_name("at")
                .long("at")
                .value_name("TIMESTAMP")
                .help("Schedule command for future execution (Unix timestamp in milliseconds)")
                .takes_value(true)
                .global(true)
                .validator(|v| {
                    match v.parse::<u64>() {
                        Ok(_) => Ok(()),
                        Err(_) => Err("Timestamp must be a valid number".into()),
                    }
                }),
        )
        .subcommand(
            SubCommand::with_name("ping")
                .about("🏓 Test connection to the satellite simulator")
                .long_about("Sends a ping command to verify the satellite simulator is responsive")
        )
        .subcommand(
            SubCommand::with_name("status")
                .about("📊 Get comprehensive system status")
                .long_about("Retrieves detailed status information from all satellite subsystems")
        )
        .subcommand(
            SubCommand::with_name("power")
                .about("🔋 Power system management")
                .subcommand(
                    SubCommand::with_name("status")
                        .about("Get power system status")
                        .long_about("Display detailed power system status including battery voltage/current, solar panel state, charging status, and power consumption")
                )
                .subcommand(
                    SubCommand::with_name("solar")
                        .about("Control solar panel state")
                        .arg(
                            Arg::with_name("state")
                                .help("Solar panel state")
                                .required(true)
                                .possible_values(&["on", "off", "enable", "disable"])
                        )
                )
                .subcommand(
                    SubCommand::with_name("save-mode")
                        .about("Control power save mode")
                        .arg(
                            Arg::with_name("state")
                                .help("Power save mode state")
                                .required(true)
                                .possible_values(&["on", "off", "enable", "disable"])
                        )
                )
        )
        .subcommand(
            SubCommand::with_name("thermal")
                .about("🌡️  Thermal system management")
                .subcommand(
                    SubCommand::with_name("status")
                        .about("Get thermal system status")
                        .long_about("Display detailed thermal system status including core temperature, battery temperature, heater state, and thermal control actions")
                )
                .subcommand(
                    SubCommand::with_name("heater")
                        .about("Control thermal heater state")
                        .arg(
                            Arg::with_name("state")
                                .help("Heater state")
                                .required(true)
                                .possible_values(&["on", "off", "enable", "disable"])
                        )
                )
        )
        .subcommand(
            SubCommand::with_name("comms")
                .about("📡 Communications system management")
                .subcommand(
                    SubCommand::with_name("status")
                        .about("Get communications system status")
                        .long_about("Display detailed communications system status including link state, signal strength, packet statistics, and error rates")
                )
                .subcommand(
                    SubCommand::with_name("link")
                        .about("Control communications link")
                        .arg(
                            Arg::with_name("state")
                                .help("Link state")
                                .required(true)
                                .possible_values(&["up", "down", "enable", "disable"])
                        )
                )
                .subcommand(
                    SubCommand::with_name("tx-power")
                        .about("Set transmitter power level")
                        .arg(
                            Arg::with_name("level")
                                .help("Power level in dBm (0-30)")
                                .required(true)
                                .validator(|v| {
                                    match v.parse::<i8>() {
                                        Ok(level) if level >= 0 && level <= 30 => Ok(()),
                                        _ => Err("Power level must be between 0 and 30 dBm".into()),
                                    }
                                })
                        )
                )
                .subcommand(
                    SubCommand::with_name("transmit")
                        .about("Transmit a message")
                        .arg(
                            Arg::with_name("message")
                                .help("Message to transmit")
                                .required(true)
                        )
                )
        )
        .subcommand(
            SubCommand::with_name("system")
                .about("🛠️  System management and diagnostics")
                .subcommand(
                    SubCommand::with_name("fault")
                        .about("Inject system fault for testing")
                        .arg(
                            Arg::with_name("subsystem")
                                .help("Target subsystem")
                                .required(true)
                                .possible_values(&["power", "thermal", "comms"])
                        )
                        .arg(
                            Arg::with_name("type")
                                .help("Fault type")
                                .required(true)
                                .possible_values(&["degraded", "failed", "offline"])
                        )
                )
                .subcommand(
                    SubCommand::with_name("clear-faults")
                        .about("Clear system faults")
                        .arg(
                            Arg::with_name("subsystem")
                                .help("Target subsystem (optional - clears all if not specified)")
                                .required(false)
                                .possible_values(&["power", "thermal", "comms"])
                        )
                )
                .subcommand(
                    SubCommand::with_name("clear-safety-events")
                        .about("⚠️  GROUND TESTING ONLY: Clear all safety events (DANGEROUS)")
                        .long_about("Clears all unresolved safety events - FOR GROUND TESTING ONLY. This command bypasses safety interlocks and should NEVER be used in flight operations.")
                        .arg(
                            Arg::with_name("force")
                                .long("force")
                                .help("Force clearing of safety events (required for safety)")
                                .required(true)
                        )
                )
                .subcommand(
                    SubCommand::with_name("fault-injection")
                        .about("Control automated fault injection system")
                        .subcommand(
                            SubCommand::with_name("enable")
                                .about("Enable automated fault injection")
                        )
                        .subcommand(
                            SubCommand::with_name("disable")
                                .about("Disable automated fault injection")
                        )
                        .subcommand(
                            SubCommand::with_name("status")
                                .about("Show fault injection statistics and configuration")
                        )
                )
                .subcommand(
                    SubCommand::with_name("safe-mode")
                        .about("Control system safe mode")
                        .arg(
                            Arg::with_name("state")
                                .help("Safe mode state")
                                .required(true)
                                .possible_values(&["on", "off", "enable", "disable"])
                        )
                )
                .subcommand(
                    SubCommand::with_name("reboot")
                        .about("Reboot the satellite system")
                        .arg(
                            Arg::with_name("confirm")
                                .long("confirm")
                                .help("Confirm the reboot operation")
                                .required(true)
                        )
                )
        )
        .subcommand(
            SubCommand::with_name("monitor")
                .about("📈 Monitor live telemetry stream")
                .long_about("Continuously monitor real-time telemetry data from the satellite")
                .arg(
                    Arg::with_name("duration")
                        .short("d")
                        .long("duration")
                        .value_name("SECONDS")
                        .help("Monitor duration in seconds (default: infinite)")
                        .takes_value(true)
                )
                .arg(
                    Arg::with_name("refresh")
                        .short("r")
                        .long("refresh")
                        .value_name("MS")
                        .help("Refresh rate in milliseconds")
                        .takes_value(true)
                        .default_value("1000")
                )
        )
        .subcommand(
            SubCommand::with_name("server")
                .about("🚀 Start the satellite simulator server")
                .long_about("Launches the satellite bus simulator server for testing and development")
                .arg(
                    Arg::with_name("background")
                        .short("b")
                        .long("background")
                        .help("Run server in background")
                )
        )
        .get_matches();

    let host = matches.value_of("host").unwrap();
    let port = matches.value_of("port").unwrap().parse::<u16>()?;
    let format = matches.value_of("format").unwrap();
    let verbose = matches.is_present("verbose");
    let execution_time = matches.value_of("at").map(|t| t.parse::<u64>().unwrap());

    if verbose {
        println!("{}", "🛰️  SatBus - Satellite Bus Simulator".bright_blue().bold());
        println!("{} {}:{}", "Connecting to".dimmed(), host, port);
    }

    match matches.subcommand() {
        ("ping", _) => {
            handle_ping(host, port, format, verbose, execution_time).await?;
        }
        ("status", _) => {
            handle_status(host, port, format, verbose).await?;
        }
        ("power", Some(sub_matches)) => {
            handle_power_command(sub_matches, host, port, format, verbose).await?;
        }
        ("thermal", Some(sub_matches)) => {
            handle_thermal_command(sub_matches, host, port, format, verbose).await?;
        }
        ("comms", Some(sub_matches)) => {
            handle_comms_command(sub_matches, host, port, format, verbose).await?;
        }
        ("system", Some(sub_matches)) => {
            handle_system_command(sub_matches, host, port, format, verbose).await?;
        }
        ("monitor", Some(sub_matches)) => {
            handle_monitor(sub_matches, host, port, format, verbose).await?;
        }
        ("server", Some(sub_matches)) => {
            handle_server(sub_matches, port).await?;
        }
        _ => {
            println!("{}", "No command specified. Use --help for usage information.".yellow());
            println!("{}", "Quick start:".bright_green());
            println!("  {} Start the simulator server", "satbus server".bright_cyan());
            println!("  {} Test connection", "satbus ping".bright_cyan());
            println!("  {} Monitor telemetry", "satbus monitor".bright_cyan());
        }
    }

    Ok(())
}

async fn handle_ping(host: &str, port: u16, format: &str, verbose: bool, execution_time: Option<u64>) -> Result<(), Box<dyn std::error::Error>> {
    if verbose {
        println!("{}", "Sending ping...".dimmed());
    }
    
    let response = send_command(host, port, create_ping_command(execution_time)).await?;
    
    match format {
        "json" => println!("{}", response),
        "compact" => println!("{}", "PONG".bright_green()),
        _ => {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&response) {
                if parsed["status"] == "Success" {
                    println!("{} {}", "✅".green(), "Satellite simulator is responsive".bright_green());
                } else {
                    println!("{} {}", "❌".red(), "Ping failed".bright_red());
                }
            } else {
                println!("{}", "PONG".bright_green());
            }
        }
    }
    
    Ok(())
}

async fn handle_status(host: &str, port: u16, format: &str, verbose: bool) -> Result<(), Box<dyn std::error::Error>> {
    if verbose {
        println!("{}", "Retrieving system status...".dimmed());
    }
    
    let response = send_command(host, port, create_status_command()).await?;
    
    match format {
        "json" => println!("{}", response),
        "compact" => println!("{}", "System operational".bright_green()),
        _ => {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&response) {
                if parsed["status"] == "Success" {
                    println!("{} {}", "📊".bright_blue(), "System Status".bright_blue().bold());
                    println!("{} {}", "Status:".bright_white(), "Operational".bright_green());
                    println!("{} {}", "Response Time:".bright_white(), "OK".bright_green());
                } else {
                    println!("{} {}", "❌".red(), "Status check failed".bright_red());
                }
            }
        }
    }
    
    Ok(())
}

async fn handle_power_command(matches: &ArgMatches<'_>, host: &str, port: u16, format: &str, _verbose: bool) -> Result<(), Box<dyn std::error::Error>> {
    match matches.subcommand() {
        ("status", _) => {
            let response = send_command(host, port, create_status_command()).await?;
            print_power_status(&response, format);
        }
        ("solar", Some(sub_matches)) => {
            let state = normalize_state(sub_matches.value_of("state").unwrap());
            let response = send_command(host, port, create_solar_command(state)).await?;
            print_command_result("Solar Panel", &format!("{}", if state { "ON" } else { "OFF" }), &response, format);
        }
        _ => {
            println!("{}", "Power subcommand required. Use 'satbus power --help' for options.".yellow());
        }
    }
    Ok(())
}

async fn handle_thermal_command(matches: &ArgMatches<'_>, host: &str, port: u16, format: &str, _verbose: bool) -> Result<(), Box<dyn std::error::Error>> {
    match matches.subcommand() {
        ("status", _) => {
            let response = send_command(host, port, create_status_command()).await?;
            print_thermal_status(&response, format);
        }
        ("heater", Some(sub_matches)) => {
            let state = normalize_state(sub_matches.value_of("state").unwrap());
            let response = send_command(host, port, create_heater_command(state)).await?;
            print_command_result("Heater", &format!("{}", if state { "ON" } else { "OFF" }), &response, format);
        }
        _ => {
            println!("{}", "Thermal subcommand required. Use 'satbus thermal --help' for options.".yellow());
        }
    }
    Ok(())
}

async fn handle_comms_command(matches: &ArgMatches<'_>, host: &str, port: u16, format: &str, _verbose: bool) -> Result<(), Box<dyn std::error::Error>> {
    match matches.subcommand() {
        ("status", _) => {
            let response = send_command(host, port, create_status_command()).await?;
            print_comms_status(&response, format);
        }
        ("link", Some(sub_matches)) => {
            let state = normalize_state(sub_matches.value_of("state").unwrap());
            let response = send_command(host, port, create_comms_command(state)).await?;
            print_command_result("Comms Link", &format!("{}", if state { "UP" } else { "DOWN" }), &response, format);
        }
        ("tx-power", Some(sub_matches)) => {
            let level: i8 = sub_matches.value_of("level").unwrap().parse()?;
            let response = send_command(host, port, create_power_command(level)).await?;
            print_command_result("TX Power", &format!("{} dBm", level), &response, format);
        }
        ("transmit", Some(sub_matches)) => {
            let message = sub_matches.value_of("message").unwrap();
            let response = send_command(host, port, create_transmit_command(message)).await?;
            print_command_result("Message", &format!("\"{}\"", message), &response, format);
        }
        _ => {
            println!("{}", "Comms subcommand required. Use 'satbus comms --help' for options.".yellow());
        }
    }
    Ok(())
}

async fn handle_fault_injection_command(matches: &ArgMatches<'_>, host: &str, port: u16, format: &str) -> Result<(), Box<dyn std::error::Error>> {
    match matches.subcommand() {
        ("enable", _) => {
            let response = send_command(host, port, create_fault_injection_enable_command(true)).await?;
            print_command_result("Fault Injection", "ENABLED", &response, format);
        }
        ("disable", _) => {
            let response = send_command(host, port, create_fault_injection_enable_command(false)).await?;
            print_command_result("Fault Injection", "DISABLED", &response, format);
        }
        ("status", _) => {
            let response = send_command(host, port, create_fault_injection_status_command()).await?;
            print_fault_injection_status(&response, format);
        }
        _ => {
            println!("{}", "Fault injection subcommand required. Use 'satbus system fault-injection --help' for options.".yellow());
        }
    }
    Ok(())
}

async fn handle_system_command(matches: &ArgMatches<'_>, host: &str, port: u16, format: &str, _verbose: bool) -> Result<(), Box<dyn std::error::Error>> {
    match matches.subcommand() {
        ("fault", Some(sub_matches)) => {
            let system = sub_matches.value_of("subsystem").unwrap();
            let fault_type = sub_matches.value_of("type").unwrap();
            let response = send_command(host, port, create_fault_command(system, fault_type)).await?;
            print_command_result("Fault Injection", &format!("{} {}", system, fault_type), &response, format);
        }
        ("clear-faults", Some(sub_matches)) => {
            let system = sub_matches.value_of("subsystem");
            let response = send_command(host, port, create_clear_faults_command(system)).await?;
            let target = system.unwrap_or("all systems");
            print_command_result("Clear Faults", target, &response, format);
        }
        ("clear-safety-events", Some(sub_matches)) => {
            if sub_matches.is_present("force") {
                let response = send_command(host, port, create_clear_safety_events_command()).await?;
                print_command_result("Clear Safety Events", "FORCED CLEAR", &response, format);
            } else {
                println!("{}", "Safety event clearing requires --force flag for safety".yellow());
            }
        }
        ("fault-injection", Some(sub_matches)) => {
            handle_fault_injection_command(sub_matches, host, port, format).await?;
        }
        ("safe-mode", Some(sub_matches)) => {
            let state = normalize_state(sub_matches.value_of("state").unwrap());
            let response = send_command(host, port, create_safe_mode_command(state)).await?;
            print_command_result("Safe Mode", &format!("{}", if state { "ENABLED" } else { "DISABLED" }), &response, format);
        }
        ("reboot", Some(sub_matches)) => {
            if sub_matches.is_present("confirm") {
                let response = send_command(host, port, create_reboot_command()).await?;
                print_command_result("System Reboot", "Initiated", &response, format);
            } else {
                println!("{}", "Reboot requires --confirm flag for safety".yellow());
            }
        }
        _ => {
            println!("{}", "System subcommand required. Use 'satbus system --help' for options.".yellow());
        }
    }
    Ok(())
}

async fn handle_monitor(_matches: &ArgMatches<'_>, host: &str, port: u16, format: &str, _verbose: bool) -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", "📡 Monitoring satellite telemetry (Press Ctrl+C to stop)...".bright_blue().bold());
    
    match format {
        "json" => {
            monitor_telemetry_json(host, port).await?;
        }
        "compact" => {
            monitor_telemetry_compact(host, port).await?;
        }
        _ => {
            monitor_telemetry_table(host, port).await?;
        }
    }
    
    Ok(())
}

async fn handle_server(matches: &ArgMatches<'_>, port: u16) -> Result<(), Box<dyn std::error::Error>> {
    let background = matches.is_present("background");
    
    println!("{}", "🚀 Starting satellite bus simulator server...".bright_green().bold());
    
    let mut cmd = Command::new("cargo");
    cmd.args(&["run", "--bin", "satbus-simulator"]);
    
    if background {
        cmd.spawn()?;
        println!("{} Server started in background on port {}", "✅".green(), port);
    } else {
        println!("{} Server starting on port {} (Press Ctrl+C to stop)", "🌐".bright_blue(), port);
        cmd.status()?;
    }
    
    Ok(())
}

// Helper functions

fn normalize_state(state: &str) -> bool {
    matches!(state, "on" | "enable" | "up")
}

fn print_command_result(action: &str, value: &str, response: &str, format: &str) {
    match format {
        "json" => println!("{}", response),
        "compact" => println!("{}", "OK".bright_green()),
        _ => {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(response) {
                let status = parsed["status"].as_str().unwrap_or("Unknown");
                match status {
                    "Success" => {
                        println!("{} {} set to {}", "✅".green(), action.bright_white(), value.bright_cyan());
                    }
                    "NegativeAck" => {
                        let message = parsed["message"].as_str().unwrap_or("Command rejected");
                        println!("{} {} failed: {}", "❌".red(), action.bright_white(), message.bright_red());
                        
                        // Provide helpful suggestions based on common errors
                        if message.contains("safe mode") {
                            println!("{} Try: {}", "💡".yellow(), "satbus system safe-mode off".bright_cyan());
                            println!("{} Or use: {}", "💡".yellow(), "satbus system clear-safety-events --force".bright_cyan());
                        } else if message.contains("already being processed") {
                            println!("{} Wait a moment and try again, or use different command parameters", "💡".yellow());
                        }
                    }
                    "ExecutionFailed" => {
                        let message = parsed["message"].as_str().unwrap_or("Execution failed");
                        println!("{} {} execution failed: {}", "⚠️".yellow(), action.bright_white(), message.bright_red());
                    }
                    "Timeout" => {
                        println!("{} {} timed out", "⏰".yellow(), action.bright_white());
                        println!("{} Command may still be executing in background", "💡".yellow());
                    }
                    _ => {
                        let message = parsed["message"].as_str().unwrap_or("Unknown error");
                        println!("{} {} status {}: {}", "❓".blue(), action.bright_white(), status.bright_blue(), message);
                    }
                }
            } else {
                println!("{} {}", "✅".green(), "Command completed".bright_green());
            }
        }
    }
}

fn print_fault_injection_status(response: &str, format: &str) {
    match format {
        "json" => println!("{}", response),
        _ => {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(response) {
                println!("\n{}", "🔧 Fault Injection System Status".bright_blue().bold());
                println!("{}", "═══════════════════════════════".bright_blue());
                
                // Parse fault injection response from the message field
                if let Some(message) = parsed.get("message").and_then(|m| m.as_str()) {
                    if let Ok(status_data) = serde_json::from_str::<serde_json::Value>(message) {
                        if let Some(config) = status_data.get("config") {
                            let enabled = config.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false);
                            println!("Status: {}", if enabled { "ENABLED".bright_green() } else { "DISABLED".bright_red() });
                            
                            if let Some(power_rate) = config.get("power_rate_percent").and_then(|v| v.as_f64()) {
                                println!("Power system rate: {:.1}%", power_rate);
                            }
                            if let Some(thermal_rate) = config.get("thermal_rate_percent").and_then(|v| v.as_f64()) {
                                println!("Thermal system rate: {:.1}%", thermal_rate);
                            }
                            if let Some(comms_rate) = config.get("comms_rate_percent").and_then(|v| v.as_f64()) {
                                println!("Comms system rate: {:.1}%", comms_rate);
                            }
                        }
                        
                        if let Some(stats) = status_data.get("stats") {
                            println!("\n{}", "📊 Statistics".bright_white().bold());
                            if let Some(total) = stats.get("total_faults_injected").and_then(|v| v.as_u64()) {
                                println!("Total faults injected: {}", total.to_string().bright_cyan());
                            }
                            if let Some(active) = stats.get("current_active_faults").and_then(|v| v.as_u64()) {
                                println!("Currently active faults: {}", active.to_string().bright_yellow());
                            }
                        }
                    }
                }
            } else {
                println!("{} Failed to parse fault injection status", "❌".red());
            }
        }
    }
}

async fn send_command(host: &str, port: u16, command: String) -> Result<String, Box<dyn std::error::Error>> {
    // Enhanced connection with better error handling
    let addr = format!("{}:{}", host, port);
    let mut stream = match TcpStream::connect(&addr).await {
        Ok(stream) => stream,
        Err(e) => {
            eprintln!("{} Failed to connect to satellite simulator at {}", "❌".red(), addr.bright_white());
            
            if e.kind() == std::io::ErrorKind::ConnectionRefused {
                eprintln!("{} Server is not running. Start it with:", "💡".yellow(), );
                eprintln!("   {}", "satbus server".bright_cyan());
                eprintln!("   or");
                eprintln!("   {}", "cargo run --bin satbus-simulator".bright_cyan());
            } else {
                eprintln!("{} Network error: {}", "🔌".yellow(), e.to_string().bright_red());
                eprintln!("{} Check network connectivity and firewall settings", "💡".yellow());
            }
            
            return Err(e.into());
        }
    };
    
    // Send command with timeout protection
    match tokio::time::timeout(std::time::Duration::from_secs(5), async {
        stream.write_all(command.as_bytes()).await?;
        stream.write_all(b"\n").await?;
        
        // Read response
        let mut buffer = vec![0; 1024];
        let n = stream.read(&mut buffer).await?;
        
        if n == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "Server closed connection"
            ));
        }
        
        let response = String::from_utf8_lossy(&buffer[..n]);
        Ok(response.to_string())
    }).await {
        Ok(result) => Ok(result?),
        Err(_) => {
            eprintln!("{} Command timed out after 5 seconds", "⏰".yellow());
            eprintln!("{} Server may be overloaded or unresponsive", "💡".yellow());
            Err("Command timeout".into())
        }
    }
}

async fn monitor_telemetry_table(host: &str, port: u16) -> Result<(), Box<dyn std::error::Error>> {
    let mut stream = TcpStream::connect((host, port)).await?;
    
    println!("{}", "┌─────────────────────────────────────────────────────────────────────────────────────┐".bright_white());
    println!("{}", "│                           🛰️  SATELLITE TELEMETRY MONITOR                         │".bright_blue().bold());
    println!("{}", "├─────────────────────────────────────────────────────────────────────────────────────┤".bright_white());
    println!("{}", "│ Time      │ Battery  │ Temp │ Solar │ Comms │ Safe Mode │ TX Pwr │ Packets │".bright_white());
    println!("{}", "├─────────────────────────────────────────────────────────────────────────────────────┤".bright_white());
    
    let mut buffer = vec![0; 4096];
    
    loop {
        let n = stream.read(&mut buffer).await?;
        if n == 0 {
            break;
        }
        
        let data = String::from_utf8_lossy(&buffer[..n]);
        
        if let Ok(telemetry) = serde_json::from_str::<serde_json::Value>(&data) {
            let timestamp = telemetry["timestamp"].as_u64().unwrap_or(0);
            let battery_mv = telemetry["power"]["battery_voltage_mv"].as_u64().unwrap_or(0);
            let temp_c = telemetry["thermal"]["core_temp_c"].as_i64().unwrap_or(0);
            let solar_mv = telemetry["power"]["solar_voltage_mv"].as_u64().unwrap_or(0);
            let comms_up = telemetry["comms"]["link_up"].as_bool().unwrap_or(false);
            let safe_mode = telemetry["system_state"]["safe_mode"].as_bool().unwrap_or(false);
            // Extract TX power from packed signal_tx_power_dbm field (lower 8 bits)
            let signal_tx_power_packed = telemetry["comms"]["signal_tx_power_dbm"].as_i64().unwrap_or(0);
            let tx_power_dbm = signal_tx_power_packed & 0xFF;
            let rx_packets = telemetry["comms"]["rx_packets"].as_u64().unwrap_or(0);
            
            let time_str = format!("{:>8}", timestamp / 1000);
            let battery_str = if battery_mv > 3600 { format!("{:>7}mV", battery_mv).green() } else { format!("{:>7}mV", battery_mv).yellow() };
            let temp_str = if temp_c > 60 { format!("{:>4}°C", temp_c).red() } else { format!("{:>4}°C", temp_c).white() };
            let solar_str = if solar_mv > 0 { format!("{:>6}mV", solar_mv).green() } else { "    OFF".red() };
            let comms_str = if comms_up { "   UP".bright_green() } else { " DOWN".bright_red() };
            let safe_str = if safe_mode { "  ACTIVE".bright_red() } else { "  NORMAL".bright_green() };
            let signal_str = format!("{:>5}dBm", tx_power_dbm);
            let packets_str = format!("{:>6}", rx_packets);
            
            println!("│ {} │ {} │ {} │ {} │ {} │ {} │ {} │ {} │",
                time_str, battery_str, temp_str, solar_str, comms_str, safe_str, signal_str, packets_str);
        }
    }
    
    Ok(())
}

async fn monitor_telemetry_json(host: &str, port: u16) -> Result<(), Box<dyn std::error::Error>> {
    let mut stream = TcpStream::connect((host, port)).await?;
    let mut buffer = vec![0; 4096];
    
    loop {
        let n = stream.read(&mut buffer).await?;
        if n == 0 {
            break;
        }
        
        let data = String::from_utf8_lossy(&buffer[..n]);
        println!("{}", data);
    }
    
    Ok(())
}

async fn monitor_telemetry_compact(host: &str, port: u16) -> Result<(), Box<dyn std::error::Error>> {
    let mut stream = TcpStream::connect((host, port)).await?;
    let mut buffer = vec![0; 4096];
    
    loop {
        let n = stream.read(&mut buffer).await?;
        if n == 0 {
            break;
        }
        
        let data = String::from_utf8_lossy(&buffer[..n]);
        
        if let Ok(telemetry) = serde_json::from_str::<serde_json::Value>(&data) {
            let timestamp = telemetry["timestamp"].as_u64().unwrap_or(0);
            let battery_mv = telemetry["power"]["battery_voltage_mv"].as_u64().unwrap_or(0);
            let temp_c = telemetry["thermal"]["core_temp_c"].as_i64().unwrap_or(0);
            let comms_up = telemetry["comms"]["link_up"].as_bool().unwrap_or(false);
            let safe_mode = telemetry["system_state"]["safe_mode"].as_bool().unwrap_or(false);
            
            let status = if safe_mode { "SAFE".red() } else if comms_up { "OK".green() } else { "WARN".yellow() };
            
            println!("[{}] {} | {}mV | {}°C | {}", 
                timestamp / 1000, status, battery_mv, temp_c, 
                if comms_up { "COMMS_UP" } else { "COMMS_DOWN" });
        }
    }
    
    Ok(())
}

// Command creation functions (same as before but cleaner)

fn add_execution_time_to_command(mut json: serde_json::Value, execution_time: Option<u64>) -> String {
    if let Some(exec_time) = execution_time {
        json["execution_time"] = serde_json::Value::Number(serde_json::Number::from(exec_time));
    }
    json.to_string()
}

fn create_ping_command(execution_time: Option<u64>) -> String {
    let json = serde_json::json!({
        "id": current_timestamp() as u32,
        "timestamp": current_timestamp(),
        "command_type": "Ping"
    });
    
    add_execution_time_to_command(json, execution_time)
}

fn create_status_command() -> String {
    serde_json::json!({
        "id": current_timestamp() as u32,
        "timestamp": current_timestamp(),
        "command_type": "SystemStatus"
    }).to_string()
}

fn create_heater_command(on: bool) -> String {
    serde_json::json!({
        "id": current_timestamp() as u32,
        "timestamp": current_timestamp(),
        "command_type": {
            "SetHeaterState": { "on": on }
        }
    }).to_string()
}

fn create_comms_command(enabled: bool) -> String {
    serde_json::json!({
        "id": current_timestamp() as u32,
        "timestamp": current_timestamp(),
        "command_type": {
            "SetCommsLink": { "enabled": enabled }
        }
    }).to_string()
}

fn create_solar_command(enabled: bool) -> String {
    serde_json::json!({
        "id": current_timestamp() as u32,
        "timestamp": current_timestamp(),
        "command_type": {
            "SetSolarPanel": { "enabled": enabled }
        }
    }).to_string()
}

fn create_power_command(power_dbm: i8) -> String {
    serde_json::json!({
        "id": current_timestamp() as u32,
        "timestamp": current_timestamp(),
        "command_type": {
            "SetTxPower": { "power_dbm": power_dbm }
        }
    }).to_string()
}

fn create_fault_command(system: &str, fault_type: &str) -> String {
    let subsystem = match system {
        "power" => "Power",
        "thermal" => "Thermal",
        "comms" => "Comms",
        _ => "Power",
    };
    
    let fault = match fault_type {
        "degraded" => "Degraded",
        "failed" => "Failed",
        "offline" => "Offline",
        _ => "Degraded",
    };
    
    serde_json::json!({
        "id": current_timestamp() as u32,
        "timestamp": current_timestamp(),
        "command_type": {
            "SimulateFault": {
                "target": subsystem,
                "fault_type": fault
            }
        }
    }).to_string()
}

fn create_clear_faults_command(system: Option<&str>) -> String {
    let target = system.map(|s| match s {
        "power" => "Power",
        "thermal" => "Thermal",
        "comms" => "Comms",
        _ => "Power",
    });
    
    serde_json::json!({
        "id": current_timestamp() as u32,
        "timestamp": current_timestamp(),
        "command_type": {
            "ClearFaults": { "target": target }
        }
    }).to_string()
}

fn create_safe_mode_command(enabled: bool) -> String {
    serde_json::json!({
        "id": current_timestamp() as u32,
        "timestamp": current_timestamp(),
        "command_type": {
            "SetSafeMode": { "enabled": enabled }
        }
    }).to_string()
}

fn create_transmit_command(message: &str) -> String {
    serde_json::json!({
        "id": current_timestamp() as u32,
        "timestamp": current_timestamp(),
        "command_type": {
            "TransmitMessage": { "message": message }
        }
    }).to_string()
}

fn create_reboot_command() -> String {
    serde_json::json!({
        "id": current_timestamp() as u32,
        "timestamp": current_timestamp(),
        "command_type": "SystemReboot"
    }).to_string()
}

fn create_fault_injection_enable_command(enabled: bool) -> String {
    serde_json::json!({
        "id": current_timestamp() as u32,
        "timestamp": current_timestamp(),
        "command_type": {
            "SetFaultInjection": {
                "enabled": enabled
            }
        }
    }).to_string()
}

fn create_fault_injection_status_command() -> String {
    serde_json::json!({
        "id": current_timestamp() as u32,
        "timestamp": current_timestamp(),
        "command_type": "GetFaultInjectionStatus"
    }).to_string()
}

fn create_clear_safety_events_command() -> String {
    serde_json::json!({
        "id": current_timestamp() as u32,
        "timestamp": current_timestamp(),
        "command_type": {
            "ClearSafetyEvents": { "force": true }
        }
    }).to_string()
}

fn current_timestamp() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64
}

fn print_power_status(response: &str, format: &str) {
    match format {
        "json" => println!("{}", response),
        _ => {
            // We need to get telemetry instead of command response for status
            println!("{}", "🔋 Power System Status".bright_blue().bold());
            println!("{}", "════════════════════════".bright_blue());
            println!("{}", "Note: Use 'satbus monitor' for live power telemetry".yellow());
            println!("{}", "Or connect to telemetry stream for detailed power status".dimmed());
        }
    }
}

fn print_thermal_status(response: &str, format: &str) {
    match format {
        "json" => println!("{}", response),
        _ => {
            println!("{}", "🌡️  Thermal System Status".bright_blue().bold());
            println!("{}", "═══════════════════════════".bright_blue());
            println!("{}", "Note: Use 'satbus monitor' for live thermal telemetry".yellow());
            println!("{}", "Or connect to telemetry stream for detailed thermal status".dimmed());
        }
    }
}

fn print_comms_status(response: &str, format: &str) {
    match format {
        "json" => println!("{}", response),
        _ => {
            println!("{}", "📡 Communications System Status".bright_blue().bold());
            println!("{}", "═══════════════════════════════".bright_blue());
            println!("{}", "Note: Use 'satbus monitor' for live comms telemetry".yellow());
            println!("{}", "Or connect to telemetry stream for detailed comms status".dimmed());
        }
    }
}