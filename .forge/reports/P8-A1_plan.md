# Plan Report: P8-A1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P8-A1                                             |
| Phase       | 008 — IPC Framing                                 |
| Description | anvilml-ipc: WorkerMessage and WorkerEvent enums  |
| Depends on  | P7-A5                                               |
| Project     | anvilml                                           |
| Planned at  | 2026-06-05T18:00:00Z                              |
| Attempt     | 1                                                 |

## Objective

Add the serde, rmp-serde, uuid (with serde feature), and anvilml-core dependencies to the `anvilml-ipc` crate, then create `src/messages.rs` containing the `WorkerMessage` and `WorkerEvent` enums with all variants defined in ANVILML_DESIGN.md §7.2 and §7.3, each deriving Serialize, Deserialize, Debug, Clone, PartialEq, and serde's default attributes for msgpack compatibility.

## Scope

### In Scope
- Add `rmp-serde` dependency (new, not yet in workspace.dependencies)
- Add `serde` (with derive feature) to `anvilml-ipc/Cargo.toml`
- Add `uuid` (with serde feature) to `anvilml-ipc/Cargo.toml`
- Create `crates/anvilml-ipc/src/messages.rs` with:
  - `WorkerMessage` enum (6 variants): Ping{seq: u64}, Shutdown, InitializeHardware{device_str: String}, Execute{job_id: Uuid, graph: serde_json::Value, settings: JobSettings, device_index: u32}, CancelJob{job_id: Uuid}, MemoryQuery
  - `WorkerEvent` enum (10 variants): Ready{worker_id: String, device_index: u32, vram_total_mib: u32, vram_free_mib: u32, arch: String, fp16: bool, bf16: bool, flash_attention: bool}, Pong{seq: u64}, Dying{reason: String}, MemoryReport{vram_used_mib: u32, ram_used_mib: u64}, Progress{job_id: Uuid, node_index: u32, node_total: u32, node_type: String, step: Option<u32>, step_total: Option<u32>}, ImageReady{job_id: Uuid, image_b64: String, width: u32, height: u32, format: String, seed: i64, steps: u32, prompt: String}, Completed{job_id: Uuid, elapsed_ms: u64}, Failed{job_id: Uuid, error: String, traceback: String}, Cancelled{job_id: Uuid}
- Update `crates/anvilml-ipc/src/lib.rs` to declare and re-export the `messages` module
- Write unit tests verifying all enum variants serialize/deserialize via rmp_serde (msgpack round-trip)
- Update `[workspace.dependencies]` in root `Cargo.toml` with `rmp-serde` entry (P7-C1 is complete, so workspace dependency rule 6.5 applies)

### Out of Scope
- Framing layer (`write_frame`, `read_frame`) — handled by P8-A2 and P8-A3
- The `ipc-probe` binary — handled by P8-A4
- Any changes to `anvilml-core` crate
- Any changes to Python worker code
- Any integration tests involving actual subprocess pipes

## Approach

1. **Add `rmp-serde` to workspace dependencies.** Since P7-C1 is complete (confirmed via existing report), rule 6.5 of FORGE_AGENT_RULES requires all external dependencies be declared in `[workspace.dependencies]` and referenced via `{ workspace = true }`. Add `rmp-serde = "0.21"` to the root `Cargo.toml` under `[workspace.dependencies]`.

2. **Update `anvilml-ipc/Cargo.toml`.** Add three new dependencies referencing the workspace table:
   - `serde = { workspace = true, features = ["derive"] }`
   - `rmp-serde = { workspace = true }`
   - `uuid = { workspace = true, features = ["serde"] }`
   Keep existing `anvilml-core = { path = "../anvilml-core" }`.

3. **Create `crates/anvilml-ipc/src/messages.rs`.** Define two enums:
   - `WorkerMessage`: Rust → Python messages. Each variant carries the fields specified in ANVILML_DESIGN.md §7.2. Uses `Uuid` for job identifiers, `serde_json::Value` for the graph, and `anvilml_core::JobSettings` for generation settings. Derives: `Serialize`, `Deserialize`, `Debug`, `Clone`, `PartialEq`.
   - `WorkerEvent`: Python → Rust events. Each variant carries the fields specified in ANVILML_DESIGN.md §7.3. Note: `Progress` and `ImageReady` include additional fields listed in the design doc (section 7.3) beyond what the task description summarizes — the plan follows the authoritative design spec. Derives: `Serialize`, `Deserialize`, `Debug`, `Clone`, `PartialEq`.
   - Module-level unit tests: for each enum, serialize to msgpack bytes via `rmp_serde::to_vec_named`, deserialize back via `rmp_serde::from_read`/`from_slice`, assert equality. Test all 6 WorkerMessage and all 10 WorkerEvent variants with representative values.

4. **Update `crates/anvilml-ipc/src/lib.rs`.** Replace the current stub (`pub fn stub() {}`) with:
   - `pub mod messages;`
   - `pub use messages::{WorkerMessage, WorkerEvent};`

5. **Verify.** Run `cargo test -p anvilml-ipc` — all tests must pass. Run `cargo clippy --workspace --features mock-hardware -- -D warnings` — no warnings.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `/home/dryw/AnvilML/Cargo.toml` | Add `rmp-serde` to `[workspace.dependencies]` |
| Modify | `/home/dryw/AnvilML/crates/anvilml-ipc/Cargo.toml` | Add serde, rmp-serde, uuid dependencies |
| Create   | `/home/dryw/AnvilML/crates/anvilml-ipc/src/messages.rs` | WorkerMessage and WorkerEvent enums with tests |
| Modify | `/home/dryw/AnvilML/crates/anvilml-ipc/src/lib.rs` | Declare `messages` module, re-export enums |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `crates/anvilml-ipc/src/messages.rs` (inline tests) | `worker_message_roundtrip_ping` | Ping{seq:1} serializes and deserializes via msgpack correctly |
| `crates/anvilml-ipc/src/messages.rs` (inline tests) | `worker_message_roundtrip_shutdown` | Shutdown (empty variant) round-trips |
| `crates/anvilml-ipc/src/messages.rs` (inline tests) | `worker_message_roundtrip_init_hardware` | InitializeHardware{device_str} round-trips with a device string |
| `crates/anvilml-ipc/src/messages.rs` (inline tests) | `worker_message_roundtrip_execute` | Execute{job_id, graph, settings, device_index} round-trips with realistic values |
| `crates/anvilml-ipc/src/messages.rs` (inline tests) | `worker_message_roundtrip_cancel_job` | CancelJob{job_id} round-trips |
| `crates/anvilml-ipc/src/messages.rs` (inline tests) | `worker_message_roundtrip_memory_query` | MemoryQuery (empty variant) round-trips |
| `crates/anvilml-ipc/src/messages.rs` (inline tests) | `worker_event_roundtrip_ready` | Ready{…} with all fields round-trips |
| `crates/anvilml-ipc/src/messages.rs` (inline tests) | `worker_event_roundtrip_pong` | Pong{seq} round-trips |
| `crates/anvilml-ipc/src/messages.rs` (inline tests) | `worker_event_roundtrip_dying` | Dying{reason} round-trips |
| `crates/anvilml-ipc/src/messages.rs` (inline tests) | `worker_event_roundtrip_memory_report` | MemoryReport{vram_used_mib, ram_used_mib} round-trips |
| `crates/anvilml-ipc/src/messages.rs` (inline tests) | `worker_event_roundtrip_progress` | Progress{job_id, node_index, node_total, node_type, step, step_total} round-trips |
| `crates/anvilml-ipc/src/messages.rs` (inline tests) | `worker_event_roundtrip_image_ready` | ImageReady{job_id, image_b64, width, height, format, seed, steps, prompt} round-trips |
| `crates/anvilml-ipc/src/messages.rs` (inline tests) | `worker_event_roundtrip_completed` | Completed{job_id, elapsed_ms} round-trips |
| `crates/anvilml-ipc/src/messages.rs` (inline tests) | `worker_event_roundtrip_failed` | Failed{job_id, error, traceback} round-trips |
| `crates/anvilml-ipc/src/messages.rs` (inline tests) | `worker_event_roundtrip_cancelled` | Cancelled{job_id} round-trips |
| `crates/anvilml-ipc/src/messages.rs` (inline tests) | `all_worker_message_variants` | Each enum discriminant is unique; all 6 variants distinct |
| `crates/anvilml-ipc/src/messages.rs` (inline tests) | `all_worker_event_variants` | Each enum discriminant is unique; all 10 variants distinct |

## CI Impact

No CI workflow files are modified. The new dependency (`rmp-serde`) will be fetched during `cargo test` and `cargo clippy` runs. All existing CI gates (format, clippy, tests, cross-checks) must continue to pass. The crate remains a workspace member with no new binaries or features.

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| `rmp-serde` version incompatibility with existing dependencies | Use a recent stable version (0.21.x) that is compatible with serde 1.0.228; verify with `cargo check` before writing tests |
| `serde_json::Value` inside msgpack — rmp_serde may not support it directly | `rmp-serde` delegates to serde's trait system; `serde_json::Value` implements Serialize/Deserialize via serde_json, so it works. If issues arise, use `HashMap<String, Value>` as an intermediate type or pin the exact rmp-serde version that supports this |
| `Progress` variant has optional fields (step, step_total) — msgpack map key ordering matters | Use `rmp_serde::to_vec_named` and `from_slice` (named-map mode) which handles arbitrary key order; this is the default behavior of rmp-serde's named serialization |
| Pre-existing clippy warnings in anvilml-ipc crate | None expected — current code is a single stub function with no logic. Verify during clippy run |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-ipc` exits 0 (all msgpack round-trip tests pass)
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0 with no warnings from the ipc crate
- [ ] `cargo check --target x86_64-pc-windows-gnu --workspace --features mock-hardware` exits 0 (cross-platform compile)
- [ ] All 16 variants across both enums are tested for msgpack serialization/deserialization round-trip
- [ ] `anvilml-core` dependency is used (JobSettings type imported from it, not duplicated)
