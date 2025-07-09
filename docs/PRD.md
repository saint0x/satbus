Here’s your full PRD for the Mock Satellite Bus Simulator, designed to be:
	•	✦ Realistic enough to impress aerospace engineers (e.g., Apex Space)
	•	✦ Implemented in Rust with preallocation in all relevant areas
	•	✦ Not overengineered, but cleanly modular and testable

⸻

🚀 PRD: Mock Satellite Bus Simulator

📌 Summary

A Rust-based, preallocated simulation of a spacecraft “bus” that models key subsystems (e.g., power, thermal, comms), exposes a command-and-telemetry protocol over TCP, and runs with realistic state transitions and failure modes.

No GUI required. Simple CLI + logs + clean JSON protocol is enough to prove correctness and polish.

⸻

🎯 Goals
	•	✅ Simulate a spacecraft control system with basic subsystems.
	•	✅ Use Rust idioms + preallocation (heapless, arrayvec, statics).
	•	✅ Enable external command/control via a small TCP protocol.
	•	✅ Emit structured telemetry at a defined rate.
	•	✅ Handle fault injection, failure states, and safe-mode behavior.
	•	✅ Keep the architecture modular and testable.

⸻

📐 System Architecture

+---------------------+
|  Ground CLI Client  |
|  - Send Commands    |
|  - View Telemetry   |
+---------+-----------+
          |
     TCP Socket
          |
+---------v-----------+
|   Satellite Agent   |
| (Rust App Runtime)  |
|                     |
|  +----------------+ |
|  | Command Queue  |<----------------------+
|  +----------------+                      |
|                     |                   |
|  +----------------+ |                   |
|  | Telemetry Loop | +-----> Periodic -->|-> [Telemetry Stream (JSON)]
|  +----------------+                     |
|                     |                   |
|  +----------------+ |                   |
|  | Subsystems:     | |                  |
|  |  - Power        | |<----+            |
|  |  - Thermal      | |     | Command    |
|  |  - Comms        | |     | Execution  |
|  +----------------+ |     |            |
+---------------------+     +------------+


⸻

🧩 Core Modules

1. Subsystems

All use preallocated struct-based state.

Name	Description	Example State
PowerSystem	Models battery level, solar input, loads	battery_mv: u16, charging: bool
ThermalSystem	Simulates temperatures, heater state	temp_c: i8, heaters_on: bool
CommsSystem	Manages radio health and downlink queues	link_up: bool, rx_count: u32


⸻

2. Command Handler
	•	Receives TCP JSON messages
	•	Buffers them in heapless::spsc::Queue
	•	Commands are parsed, validated, and dispatched
	•	Supports:
	•	set_heater_state(on|off)
	•	set_comms_link(up|down)
	•	simulate_fault(subsystem)
	•	ping

⸻

3. Telemetry Generator
	•	Periodic task (e.g., every 1s)
	•	Collects telemetry snapshot from each subsystem
	•	Constructs a TelemetryPacket (static struct or JSON) and sends it
	•	Uses preallocated buffer via heapless::Vec or arrayvec::ArrayString

⸻

4. State Machine & Safe Mode
	•	Watchdog loop monitors critical parameters:
	•	If battery < 3200mV or temperature > 75C → enter SafeMode
	•	In SafeMode:
	•	Disable Comms
	•	Enable heaters if cold
	•	Ignore non-critical commands

⸻

💾 Memory Strategy (Prealloc Design)

Area	Strategy
Command Queue	heapless::spsc::Queue<Command, N>
Telemetry Buffers	heapless::Vec<TelemetryPacket, M>
String Serialization	heapless::String<N> or arrayvec::ArrayString<N>
Subsystem State	Stack-allocated structs or static mutable singletons
Logs	Optional fixed-capacity ring buffer for events


⸻

🌐 Protocol

All over plain TCP, one connection.

📤 Commands (JSON)

{ "command": "set_heater_state", "params": { "on": true } }
{ "command": "simulate_fault", "params": { "target": "PowerSystem" } }
{ "command": "ping" }

📥 Telemetry (JSON)

{
  "time": 12345678,
  "power": { "battery_mv": 3750, "charging": true },
  "thermal": { "temp_c": 32, "heaters_on": false },
  "comms": { "link_up": true }
}


⸻

📦 Deliverables

Artifact	Description
sat_sim/	Main Rust crate
src/agent.rs	Runtime loop + system integration
src/subsystems/*.rs	Each subsystem as a module
src/protocol.rs	Telemetry/command schema + framing
src/net.rs	TCP server loop
src/fault.rs	Fault simulation logic
tests/	Unit + integration tests
cli_client.rs	CLI tool to connect and issue commands
README.md	With architecture diagram, examples, usage
PREALLOC.md	Explanation of all memory strategies


⸻

✋ Out of Scope (for now)
	•	No 3D or fancy GUI
	•	No ECS or async event buses
	•	No actual RTOS or embedded target
	•	No real-time clock sync or orbital dynamics

⸻

🧠 Stretch Goals (if you fly fast)
	•	CSV-based telemetry log export
	•	Integration with tokio tracing or metrics
	•	Simple dashboard with Tauri/WASM and graphs

⸻

🧪 MVP Launch Criteria
	•	✅ Launch sim agent, see logs
	•	✅ Connect CLI, issue at least 3 types of commands
	•	✅ Receive telemetry every second
	•	✅ Observe correct state transitions + fault behavior
	•	✅ All buffers fixed-capacity; no heap allocations at runtime
	•	✅ CI passes, doc coverage

⸻

You ship this cleanly, and you’ll look like a Rust-savvy space systems engineer even without aerospace experience.

Want a repo skeleton scaffold to start coding?