# Plan Report: P2-B1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P2-B1                                       |
| Phase       | 002 — Core Types & IPC                      |
| Description | anvilml-ipc: message types (Rust→Python and Python→Rust) |
| Depends on  | P2-A1, P2-A2, P2-A3, P2-A4                  |
| Project     | anvilml                                     |
| Planned at  | 2026-05-30T07:45:00Z                        |
| Attempt     | 1                                           |

## Objective

Define the two IPC message enums (`WorkerMessage` and `WorkerEvent`) that form the complete communication contract between the Rust supervisor and Python worker processes. These enums carry all variants and fields specified in `ANVILML_DESIGN.md §7.2–7.3`, derive `Serialize`/`Deserialize` via `serde`, use `rmp-serde` for msgpack encoding (named-map format for Python interop), and include a dependency on `anvilml-core` for the shared `JobSettings` type. The crate must pass `cargo test -p anvilml-ipc -- messages` with serialization round-trip tests.

## Scope

### In Scope
- Define `WorkerMessage` enum (6 variants) in `crates/anvilml-ipc/src/messages.rs`
- Define `WorkerEvent` enum (9 variants) in `crates/anvilml-ipc/src/messages.rs`
- Add required dependencies to `crates/anvilml-ipc/Cargo.toml`: `serde`, `rmp-serde`, `uuid`, `serde_json`, `anvilml-core`
- Update `crates/anvilml-ipc/src/lib.rs` to re-export the enums
- Inline unit tests in `messages.rs` for msgpack serialization round-trips of at least one `WorkerMessage` and one `WorkerEvent` variant
- Verify `cargo test -p anvilml-ipc -- messages` exits 0

### Out of Scope
- IPC framing layer (4-byte length-prefix + msgpack I/O) — this is P2-B2
- Worker spawn/supervise/respawn logic — later phase
- Python worker implementation — not part of this Rust crate
- HTTP server, WebSocket broadcasting, job scheduling — later phases
- `anvilml-core` domain types — already complete from P2-A1–P2-A4

## Approach

1. **Add dependencies to `Cargo.toml`**: Add `serde = { version = "1", features = ["derive"] }`, `rmp-serde = "1"`, `uuid = { version = "1", features = ["serde"] }`, `serde_json = "1"`, and `anvilml-core = { path = "../anvilml-core" }` as a regular dependency. Add `tokio = { version = "1", features = ["rt", "macros", "io-util"] }` under `[dev-dependencies]` for the test harness.

2. **Create `messages.rs`**: In `crates/anvilml-ipc/src/messages.rs`, define:
   - `WorkerMessage` enum with `#[serde(rename_all = "snake_case")]` and 6 variants matching §7.2 exactly: `Ping { seq: u64 }`, `Shutdown {}`, `InitializeHardware { device_str: String }`, `Execute { job_id: Uuid, graph: serde_json::Value, settings: JobSettings, device_index: u32 }`, `CancelJob { job_id: Uuid }`, `MemoryQuery {}`. Derive `Serialize`, `Deserialize`, `Clone`, `Debug`.
   - `WorkerEvent` enum with `#[serde(rename_all = "snake_case")]` and 9 variants matching the task spec exactly: `Pong { seq: u64 }`, `Ready {}`, `Progress { job_id: Uuid, node_index: u32, node_total: u32, node_type: String, step: Option<u32>, step_total: Option<u32> }`, `ImageReady { job_id: Uuid, image_bytes: Vec<u8>, width: u32, height: u32, seed: i64 }`, `MemoryReport { device_index: u32, vram_used_mib: u32, vram_total_mib: u32 }`, `Completed { job_id: Uuid }`, `Failed { job_id: Uuid, error: String, traceback: Option<String> }`, `Cancelled { job_id: Uuid }`, `Dying {}`. Derive `Serialize`, `Deserialize`, `Clone`, `Debug`.

3. **Update `lib.rs`**: Replace the existing stub `mod tests` with `pub mod messages; pub use messages::{WorkerMessage, WorkerEvent};` and keep a minimal test module.

4. **Add inline tests**: At the bottom of `messages.rs`, add a `#[cfg(test)] mod tests` block containing:
   - A msgpack round-trip test for `WorkerMessage::Ping { seq: 1 }` — serialize via `rmp_serde::to_vec_named`, deserialize back, assert equality.
   - A msgpack round-trip test for `WorkerEvent::Progress` — serialize via `rmp_serde::to_vec_named`, deserialize back, assert all fields match (including `Option<u32>` fields being `None`).
   - A test verifying that `rmp_serde::to_vec_named` produces named-map output (not array format) by checking the first byte is `0x80`+ (map fixmap).

5. **Verify**: Run `cargo test -p anvilml-ipc -- messages` and confirm exit code 0.

## Files Affected

| Action   | Path                              | Description                                          |
|----------|-----------------------------------|------------------------------------------------------|
| MODIFY   | crates/anvilml-ipc/Cargo.toml     | Add serde, rmp-serde, uuid, serde_json, anvilml-core deps; add tokio dev-dep |
| CREATE   | crates/anvilml-ipc/src/messages.rs| WorkerMessage and WorkerEvent enums with inline tests  |
| MODIFY   | crates/anvilml-ipc/src/lib.rs     | Re-export messages module and enums                    |

## Tests

| Test ID / Name            | File                          | Validates                                          |
|---------------------------|-------------------------------|----------------------------------------------------|
| msgpack_ping_round_trip   | crates/anvilml-ipc/src/messages.rs (inline) | WorkerMessage::Ping serializes/deserializes correctly via rmp_serde named-map format |
| msgpack_progress_round_trip | crates/anvilml-ipc/src/messages.rs (inline) | WorkerEvent::Progress with all fields (including Option<u32> = None) round-trips correctly |
| named_map_format_verify   | crates/anvilml-ipc/src/messages.rs (inline) | rmp_serde::to_vec_named produces map format (0x8x byte), not array format, for Python interop |

## CI Impact

No CI changes required. The `cargo test` command in the existing CI matrix already runs `cargo test -p anvilml-ipc --features mock-hardware`, and this task adds no new features or crates that would require CI modification.

## Risks and Mitigations

| Risk                      | Likelihood | Impact | Mitigation              |
|---------------------------|-----------|--------|-------------------------|
| `rmp-serde` version mismatch with Python msgpack library | Medium | High | Use `rmp_serde::to_vec_named` (named-map format) which is compatible with Python `msgpack` library's default decode-by-name behavior; documented in TASKS_PHASE002.md as a known constraint |
| Missing `JobSettings` import from `anvilml-core` | Low | Medium | Verify anvilml-core exports `JobSettings` via `types/mod.rs` re-exports before writing the enum (confirmed: it does) |
| `serde_json::Value` serialization in msgpack | Low | Low | `serde_json::Value` implements `Serialize`/`Deserialize`; `rmp-serde` handles it transparently — no special handling needed |
| Option field encoding differences between serde and Python | Medium | Medium | The task spec uses `Option<u32>` for Progress.step/step_total and Failed.traceback; these serialize as `null` in msgpack named-map, which Python's msgpack decodes as `None` — standard behavior, no special handling needed |

## Acceptance Criteria

- [ ] `WorkerMessage` enum defined with all 6 variants (Ping, Shutdown, InitializeHardware, Execute, CancelJob, MemoryQuery) matching field types from §7.2
- [ ] `WorkerEvent` enum defined with all 9 variants (Pong, Ready, Progress, ImageReady, MemoryReport, Completed, Failed, Cancelled, Dying) matching field types from task spec
- [ ] Both enums derive `Serialize`, `Deserialize`, `Clone`, `Debug`
- [ ] Both enums use `#[serde(rename_all = "snake_case")]` for consistent Python interop
- [ ] `WorkerMessage::Execute.graph` uses `serde_json::Value` type
- [ ] `anvilml-core` is a dependency of `anvilml-ipc` (for `JobSettings`)
- [ ] `rmp-serde` is a dependency for msgpack serialization
- [ ] `uuid` with `serde` feature is a dependency for UUID fields
- [ ] `lib.rs` re-exports `WorkerMessage` and `WorkerEvent`
- [ ] `cargo test -p anvilml-ipc -- messages` exits 0 with passing round-trip tests
