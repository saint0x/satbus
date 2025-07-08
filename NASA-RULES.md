✅ Which Rules Apply (And Why They’re Worth It)

2. Fixed-bounded loops
	•	Why: Prevents runaway loops and ensures you can reason about execution time at compile-time.
	•	In Project: All loops in telemetry creation, queue processing, or state updates must have a compile-time or runtime-known max iterations. Avoid unbounded iterators.

3. No dynamic memory after init
	•	Why: Essential for deterministic performance—mirrors embedded constraints.
	•	In Project: Reinforces your preallocation requirement: use heapless::*, arrayvec, avoid Vec::new() or Box::new() in runtime loops.

4. Short functions
	•	Why: Improves readability and verification—easier to review, maintain.
	•	In Project: Keep each subsystem handler or loop ≤ 60 lines. Extract helpers where logic grows.

5. Assertions per function
	•	Why: Actively check invariants and catch unexpected states early.
	•	In Project: Add assertions in loops and subsystem updates (e.g., battery_voltage ∈ [0,5000], queue length ≤ capacity).

6. Minimize scope
	•	Why: Limits unintended side-effects and aids verification.
	•	In Project: Variables and subsystems only exposed to where needed; don’t globalize everything.

7. Check return values
	•	Why: Ensures you don’t ignore errors—critical in commands, state transitions.
	•	In Project: Propagate Result<T, E> and explicitly .unwrap_or_else or propagate with ?.

10. Enable all warnings
	•	Why: Prevents hidden issues.
	•	In Project: Use #![deny(warnings)], set rustc lints like unused code, unreachable patterns.

⸻

🧩 Rules to Skip or Loosen
	•	Rule 1 (No recursion/goto) isn’t needed—Rust doesn’t have goto; recursion is fine when bounded.
	•	Rule 8 (Preprocessor use): Not relevant in Rust.
	•	Rule 9 (Pointer restrictions): Applies to C pointers, not Rust references.
	•	Rule 10 is fully applicable.
