Here’s a researched overview of real-world usage distributions you’ll want your mock satellite bus simulator to cover—structured around typical telemetry and command patterns seen in small satellites:

⸻

🎯 Telemetry & Command Use Distributions

1. Downlink Data Rates (Telemetry)
	•	Dominant range: 20–100 kbps common for telemetry in LEO small satellites
	•	Example: MightySat‑2.1 uses ~20 kbps downlink for telemetry, with 1 Mbps dedicated to payload data and 2 kbps for uplink commands  ￼ ￼.
	•	Medium missions: 256–1,024 kbps typical for larger small sats (e.g., Chandra uses ~32 kbps housekeeping but scales during downlink) ().
	•	Low-end systems: 4–9.6 kbps if using UHF or relay networks  ￼.

Recommendation: Design telemetry loops targeting ~20 kbps (~2.5 kB/s, ~1 packet/sec with ~2 kB payload frames).

⸻

2. Uplink Data Rates (Commands)
	•	Command rates are low: ~2 kbps
	•	MightySat‑2.1 uplink operates at ~2 kbps  ￼.
	•	Command cadence:
	•	Routine ops: tens of commands per day (configuration changes, housekeeping)
	•	Priority commands: immediate or scheduled with time tags  ￼.

Recommendation: Support short (<100 Byte) JSON commands, handling perhaps 1–2 commands per second peak, with queue depth ~16–32 commands.

⸻

3. Subsystem Update Frequencies
	•	Housekeeping telemetry: ~0.5–2 Hz per subsystem
	•	Typical state bus architecture pushes out health/status at ~1 Hz  ￼.
	•	Payload data fills extra bandwidth, but for your sim: emulate subsystems sending ~1 update per second.

⸻

4. Fault Events
	•	Fault rates are rare, usually <1 per month in real missions.
	•	In simulation, injecting faults at ~0.1–1% of timesteps (i.e., 1 per 100–1,000 sec) simulates realistic, infrequent anomaly cases.

⸻

✅ Scope Checklist for Your Simulator

To cover the majority of actual satellite needs:
	•	Telemetry: Emit every second; ~1–2 kB per packet (JSON).
	•	Commands: Handle bursts up to 5/sec, average 1/sec; queue size ~16.
	•	Subsystems: 3 subsystems (power, thermal, comms), each sending 1 Hz status.
	•	Fault injection: Random ~0.5% chance per cycle plus specific commands.
	•	Modes: Support scheduled vs. immediate commands; ack/nack semantics.

⸻

🛠️ Prealloc & Design Implications
	•	Telemetry buffer: Preallocate capacity for ~10 packets in a heapless::Vec<Packet, 10>.
	•	Command queue: heapless::spsc::Queue<Command, 32> handles bursts without alloc.
	•	Event log: Fixed ArrayVec<Event, 256> for fault/event tracking.
	•	String encoding: Use heapless::String<2048> or ArrayString<2048> for JSON packet formatting.

⸻

📊 Summary Table

Metric / Event	Real-world Range	Simulator Target
Downlink (telemetry) rate	20–100 kbps	~20 kbps (2 kB/s)
Uplink (command) rate	~2 kbps; ~1–2 cmd/s, bursts possible	~1 cmd/s avg, bursts up to 5
Telemetry frequency per subsys	~0.5–2 Hz	1 Hz
Fault injection rate	<0.01% over mission	~0.5% per cycle
Command queue size	Typically queued during downlink	16–32 depth

By aligning with these numbers, your product will reflect real mission telemetry profiles, offering a credible and grounded simulation that aerospace engineers (like those at Apex) will immediately recognize.

⸻

Next step: we can help you sketch buffer sizing and code patterns for these volumes. Want to do that?