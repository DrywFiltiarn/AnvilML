# Plan Report: P8-A1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P8-A1                                             |
| Phase       | 008 — ZeroMQ IPC Transport                        |
| Description | WorkerMessage and WorkerEvent enums with msgpack codecs |
| Depends on  | none (Phase 007 prerequisite: core types exist in anvilml-core) |
| Project     | anvilml                                           |
| Planned at  | 2026-06-16T08:00:00Z                              |
| Attempt     | 1                                                 |

## Objective

Create `crates/anvilml-ipc/src/messages.rs` defining the `WorkerMessage` and `WorkerEvent` enums with `#[serde(tag = "_type")]` disciminators, plus `encode_message()` and `decode_event()` functions using `rmp-serde` flat-dict serialisation. This establishes the IPC message types that the `RouterTransport` (P8-A2/A3) will serialise and deserialise over the ZeroMQ ROUTER socket. When complete, `cargo test -p anvilml-ipc -- messages` exits 0 with ≥ 8 roundtrip tests proving every variant serialises and deserialises correctly.

## Scope

### In Scope
- **CREATE** `crates/anvilml-ipc/src/messages.rs`:
  - `WorkerMessage` enum with variants: `Ping { seq: u64 }`, `Shutdown`, `Execute { job_id: Uuid, graph: Value, settings: JobSettings, device_index: u32 }`, `CancelJob { job_id: Uuid }`, `MemoryQuery`
  - `WorkerEvent` enum with variants: `Ready { worker_id, device_index, device_name, device_type, vram_total_mib, vram_free_mib, torch_version, fp16, bf16, fp8, flash_attention, node_types }`, `Pong { seq: u64 }`, `Dying { reason: String }`, `MemoryReport { vram_used_mib: u32, ram_used_mib: u64 }`, `Progress { job_id, step, total_steps, preview_b64 }`, `ImageReady { job_id, image_b64, width, height, format, seed, steps }`, `Completed { job_id, elapsed_ms }`, `Failed { job_id, error, traceback }`, `Cancelled { job_id }`
  - `pub fn encode_message(msg: &WorkerMessage) -> Result<Vec<u8>, IpcError>`
  - `pub fn decode_event(bytes: &[u8]) -> Result<WorkerEvent, IpcError>`
  - `IpcError` enum (using `thiserror`) for serialisation/deserialisation errors
- **MODIFY** `crates/anvilml-ipc/src/lib.rs`: add `pub mod messages;` and `pub use messages::{WorkerMessage, WorkerEvent, encode_message, decode_event};`
- **MODIFY** `crates/anvilml-ipc/Cargo.toml`: add `serde_json` to dependencies (needed for `serde_json::Value` in `Execute` variant)
- **CREATE** `crates/anvilml-ipc/tests/roundtrip_tests.rs`: ≥ 8 roundtrip tests
- **Bump** `crates/anvilml-ipc` patch version: `0.1.0 → 0.1.1`

### Out of Scope
- `RouterTransport` bind/send/recv implementation (P8-A2, P8-A3)
- Python worker IPC transport (P8-B1, P8-B2)
- Stress test (P8-C1)
- Any changes to `anvilml-core` types (they already exist with correct derives)

## Existing Codebase Assessment

The `anvilml-ipc` crate currently has only a stub `lib.rs` with a single `stub()` function and no other source files. The `Cargo.toml` already declares all runtime dependencies: `anvilml-core`, `zeromq`, `rmp-serde`, `tokio`, and `tracing`. However, `serde_json` is missing from the crate's direct dependencies despite being needed for the `Execute` variant's `graph: serde_json::Value` field.

The `anvilml-core` crate provides all referenced types: `JobSettings` (with `Serialize, Deserialize` derives), `NodeTypeDescriptor` (with `Serialize, Deserialize` derives), and `AnvilError` (for the broader error hierarchy). These types are already exported through `anvilml-core`'s public API and are available to `anvilml-ipc` via the path dependency.

The project follows a strict pattern: enums use `#[serde(tag = "_type")]` for the discriminated union format, msgpack serialisation uses `rmp_serde::to_vec_named` / `from_slice`, and all `pub` items have `///` doc comments. The crate's `lib.rs` contains only module declarations and re-exports (≤ 80 lines). Test files live in `crates/{name}/tests/` as separate test crates.

## Resolved Dependencies

| Type   | Name         | Version verified | MCP source     | Feature flags confirmed |
|--------|-------------|------------------|----------------|------------------------|
| crate  | zeromq      | 0.6.0            | cargo search + local cache | tokio-runtime, tcp-transport (workspace) |
| crate  | rmp-serde   | 1.3.1            | cargo search + local cache | n/a |
| crate  | serde_json  | 1.0.150          | workspace Cargo.toml | n/a |
| crate  | thiserror   | 2.0.18           | workspace Cargo.toml | n/a |

Notes:
- `zeromq` 0.6.0 verified: `RouterSocket` and `DealerSocket` exist in the local cache at `~/.cargo/registry/src/index.crates.io-*/zeromq-0.6.0/src/`. The `SocketRecv` trait provides `async fn recv(&mut self) -> ZmqResult<ZmqMessage>`. The `SocketSend` trait provides `async fn send(&mut self, message: ZmqMessage) -> ZmqResult<()>`.
- `rmp-serde` 1.3.1 verified: `to_vec_named()` and `from_slice()` exist as public functions in the crate root.
- `serde_json` is a workspace dependency (`1.0.150`); it is not currently listed in `anvilml-ipc/Cargo.toml` but must be added for the `Execute` variant.

## Approach

1. **Add `serde_json` to `anvilml-ipc/Cargo.toml`.** Add `serde_json = { workspace = true }` under `[dependencies]`. Rationale: the `Execute` variant carries `graph: serde_json::Value`, and `serde_json` is already available as a workspace dependency.

2. **Create `crates/anvilml-ipc/src/messages.rs`.** Write the complete module with:
   - Module-level `//!` doc comment describing the crate's message types and serialisation contract.
   - `use` statements for `serde::{Deserialize, Serialize}`, `rmp_serde`, `anvilml_core::{JobSettings, NodeTypeDescriptor}`, `uuid::Uuid`, `serde_json::Value`, and `thiserror::Error`.
   - `IpcError` enum with variants `Serialize(String)` and `Deserialize(String)`, deriving `Debug, thiserror::Error`. Doc comments on each variant explaining when it occurs.
   - `WorkerMessage` enum with `#[derive(Debug, Clone, Serialize, Deserialize)]` and `#[serde(tag = "_type")]`. Five variants matching the design doc §8.4 exactly: `Ping { seq: u64 }`, `Shutdown`, `Execute { job_id: Uuid, graph: Value, settings: JobSettings, device_index: u32 }`, `CancelJob { job_id: Uuid }`, `MemoryQuery`. Each variant has a `///` doc comment.
   - `WorkerEvent` enum with `#[derive(Debug, Clone, Serialize, Deserialize)]` and `#[serde(tag = "_type")]`. Nine variants matching the design doc §8.5 exactly: `Ready { worker_id: String, device_index: u32, device_name: String, device_type: String, vram_total_mib: u32, vram_free_mib: u32, torch_version: String, fp16: bool, bf16: bool, fp8: bool, flash_attention: bool, node_types: Vec<NodeTypeDescriptor> }`, `Pong { seq: u64 }`, `Dying { reason: String }`, `MemoryReport { vram_used_mib: u32, ram_used_mib: u64 }`, `Progress { job_id: Uuid, step: u32, total_steps: u32, preview_b64: Option<String> }`, `ImageReady { job_id: Uuid, image_b64: String, width: u32, height: u32, format: String, seed: i64, steps: u32 }`, `Completed { job_id: Uuid, elapsed_ms: u64 }`, `Failed { job_id: Uuid, error: String, traceback: Option<String> }`, `Cancelled { job_id: Uuid }`. Each variant has a `///` doc comment.
   - `pub fn encode_message(msg: &WorkerMessage) -> Result<Vec<u8>, IpcError>`: calls `rmp_serde::to_vec_named(msg).map_err(|e| IpcError::Serialize(e.to_string()))`. Rationale: `to_vec_named` produces a flat dict with the `_type` key included as a named field, matching the msgpack flat-dict format expected by the Python worker's `msgpack` library.
   - `pub fn decode_event(bytes: &[u8]) -> Result<WorkerEvent, IpcError>`: calls `rmp_serde::from_slice(bytes).map_err(|e| IpcError::Deserialize(e.to_string()))`. Rationale: `from_slice` deserialises the msgpack bytes back into `WorkerEvent`, using the `_type` discriminator to select the correct enum variant.

3. **Update `crates/anvilml-ipc/src/lib.rs`.** Replace the stub with:
   - Keep the existing `//!` crate-level doc comment (it already correctly describes the crate's scope).
   - Add `pub mod messages;` to declare the new module.
   - Add `pub use messages::{WorkerMessage, WorkerEvent, encode_message, decode_event};` to re-export the public API.
   - Remove the `#[allow(dead_code)] pub fn stub()` line.
   - Result: lib.rs remains ≤ 80 lines, containing only `//!` doc comment, `pub mod`, and `pub use` items.

4. **Create `crates/anvilml-ipc/tests/roundtrip_tests.rs`.** Write integration tests that import `anvilml_ipc::{WorkerMessage, WorkerEvent, encode_message, decode_event}` and verify roundtrip serialisation for each variant. Use the `serial_test` crate's `#[serial]` annotation if any test mutates process-global state (none expected here — all tests are pure data transformations). Each test function has a `///` doc comment describing the invariant it verifies.

5. **Bump `anvilml-ipc` patch version** from `0.1.0` to `0.1.1` in `crates/anvilml-ipc/Cargo.toml` per §12 of ENVIRONMENT.md.

6. **Verify** `cargo test -p anvilml-ipc -- messages` exits 0.

## Public API Surface

| Item | Type | Module Path | Signature |
|------|------|-------------|-----------|
| `WorkerMessage` | enum | `anvilml_ipc::WorkerMessage` | `#[derive(Debug, Clone, Serialize, Deserialize)] #[serde(tag = "_type")] enum WorkerMessage { ... }` |
| `WorkerEvent` | enum | `anvilml_ipc::WorkerEvent` | `#[derive(Debug, Clone, Serialize, Deserialize)] #[serde(tag = "_type")] enum WorkerEvent { ... }` |
| `encode_message` | fn | `anvilml_ipc::encode_message` | `pub fn encode_message(msg: &WorkerMessage) -> Result<Vec<u8>, IpcError>` |
| `decode_event` | fn | `anvilml_ipc::decode_event` | `pub fn decode_event(bytes: &[u8]) -> Result<WorkerEvent, IpcError>` |
| `IpcError` | enum | `anvilml_ipc::IpcError` | `#[derive(Debug, thiserror::Error)] enum IpcError { Serialize(String), Deserialize(String) }` |

All five items are `pub` and exported through `lib.rs`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-ipc/src/messages.rs` | WorkerMessage/WorkerEvent enums, encode/decode functions, IpcError |
| MODIFY | `crates/anvilml-ipc/src/lib.rs` | Add `pub mod messages;`, re-export, remove stub |
| MODIFY | `crates/anvilml-ipc/Cargo.toml` | Add `serde_json` dep, bump version 0.1.0 → 0.1.1 |
| CREATE | `crates/anvilml-ipc/tests/roundtrip_tests.rs` | ≥ 8 roundtrip tests for all message/event variants |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-ipc/tests/roundtrip_tests.rs` | `ping_roundtrip` | `WorkerMessage::Ping { seq: 42 }` serialises and deserialises correctly | None | `WorkerMessage::Ping { seq: 42 }` | `decode_message(encode_message(...))` returns `Ok(Ping { seq: 42 })` | `cargo test -p anvilml-ipc -- messages` exits 0 |
| `crates/anvilml-ipc/tests/roundtrip_tests.rs` | `shutdown_roundtrip` | `WorkerMessage::Shutdown` serialises and deserialises correctly | None | `WorkerMessage::Shutdown` | `decode_message(encode_message(...))` returns `Ok(Shutdown)` | `cargo test -p anvilml-ipc -- messages` exits 0 |
| `crates/anvilml-ipc/tests/roundtrip_tests.rs` | `execute_roundtrip` | `WorkerMessage::Execute` with a full graph serialises and deserialises correctly | None | `WorkerMessage::Execute { job_id: uuid, graph: Value::Object(...), settings: JobSettings::default(), device_index: 0 }` | Roundtrip preserves all fields | `cargo test -p anvilml-ipc -- messages` exits 0 |
| `crates/anvilml-ipc/tests/roundtrip_tests.rs` | `cancel_job_roundtrip` | `WorkerMessage::CancelJob` serialises and deserialises correctly | None | `WorkerMessage::CancelJob { job_id: uuid }` | Roundtrip preserves job_id | `cargo test -p anvilml-ipc -- messages` exits 0 |
| `crates/anvilml-ipc/tests/roundtrip_tests.rs` | `memory_query_roundtrip` | `WorkerMessage::MemoryQuery` serialises and deserialises correctly | None | `WorkerMessage::MemoryQuery` | `decode_message(encode_message(...))` returns `Ok(MemoryQuery)` | `cargo test -p anvilml-ipc -- messages` exits 0 |
| `crates/anvilml-ipc/tests/roundtrip_tests.rs` | `ready_roundtrip` | `WorkerEvent::Ready` with full device info and node types serialises and deserialises correctly | None | `WorkerEvent::Ready { worker_id: "worker-0", device_index: 0, device_name: "NVIDIA RTX 4090", device_type: "cuda", vram_total_mib: 24576, vram_free_mib: 24000, torch_version: "2.5.1", fp16: true, bf16: true, fp8: false, flash_attention: true, node_types: vec![...] }` | Roundtrip preserves all fields | `cargo test -p anvilml-ipc -- messages` exits 0 |
| `crates/anvilml-ipc/tests/roundtrip_tests.rs` | `pong_roundtrip` | `WorkerEvent::Pong` serialises and deserialises correctly | None | `WorkerEvent::Pong { seq: 42 }` | `decode_event(encode_message_as_event(...))` returns `Ok(Pong { seq: 42 })` | `cargo test -p anvilml-ipc -- messages` exits 0 |
| `crates/anvilml-ipc/tests/roundtrip_tests.rs` | `dying_roundtrip` | `WorkerEvent::Dying` with a reason string serialises and deserialises correctly | None | `WorkerEvent::Dying { reason: "SIGTERM" }` | Roundtrip preserves reason | `cargo test -p anvilml-ipc -- messages` exits 0 |
| `crates/anvilml-ipc/tests/roundtrip_tests.rs` | `completed_roundtrip` | `WorkerEvent::Completed` serialises and deserialises correctly | None | `WorkerEvent::Completed { job_id: uuid, elapsed_ms: 1234 }` | Roundtrip preserves all fields | `cargo test -p anvilml-ipc -- messages` exits 0 |
| `crates/anvilml-ipc/tests/roundtrip_tests.rs` | `failed_roundtrip` | `WorkerEvent::Failed` with error and traceback serialises and deserialises correctly | None | `WorkerEvent::Failed { job_id: uuid, error: "OOM", traceback: Some("trace...") }` | Roundtrip preserves all fields | `cargo test -p anvilml-ipc -- messages` exits 0 |
| `crates/anvilml-ipc/tests/roundtrip_tests.rs` | `cancelled_roundtrip` | `WorkerEvent::Cancelled` serialises and deserialises correctly | None | `WorkerEvent::Cancelled { job_id: uuid }` | Roundtrip preserves job_id | `cargo test -p anvilml-ipc -- messages` exits 0 |
| `crates/anvilml-ipc/tests/roundtrip_tests.rs` | `image_ready_roundtrip` | `WorkerEvent::ImageReady` with all fields serialises and deserialises correctly | None | `WorkerEvent::ImageReady { job_id: uuid, image_b64: "abc", width: 512, height: 512, format: "png", seed: 42, steps: 20 }` | Roundtrip preserves all fields | `cargo test -p anvilml-ipc -- messages` exits 0 |
| `crates/anvilml-ipc/tests/roundtrip_tests.rs` | `progress_roundtrip` | `WorkerEvent::Progress` with optional preview serialises and deserialises correctly | None | `WorkerEvent::Progress { job_id: uuid, step: 5, total_steps: 20, preview_b64: None }` | Roundtrip preserves all fields | `cargo test -p anvilml-ipc -- messages` exits 0 |
| `crates/anvilml-ipc/tests/roundtrip_tests.rs` | `memory_report_roundtrip` | `WorkerEvent::MemoryReport` serialises and deserialises correctly | None | `WorkerEvent::MemoryReport { vram_used_mib: 4096, ram_used_mib: 8192 }` | Roundtrip preserves all fields | `cargo test -p anvilml-ipc -- messages` exits 0 |

## CI Impact

No CI changes required. The new test file lives in `crates/anvilml-ipc/tests/` which is already picked up by `cargo test --workspace --features mock-hardware`. No new file types, gates, or test modules are introduced that would require CI workflow changes.

## Platform Considerations

None identified. The message types are pure data structures with no platform-specific behaviour. `serde` and `rmp-serde` handle serialisation uniformly across all platforms. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `serde_json::Value` inside `WorkerMessage::Execute` requires `serde_json` dependency in `anvilml-ipc`; if forgotten, compilation fails with missing `Value` type | Low | High | Add `serde_json = { workspace = true }` to `anvilml-ipc/Cargo.toml` in the same step as creating `messages.rs`. Verify with `cargo check -p anvilml-ipc`. |
| `rmp_serde::to_vec_named` vs `to_vec` — using `to_vec` would produce a compact array encoding that Python's `msgpack` library may not handle correctly for tagged enums | Medium | High | Use `to_vec_named` (verified in MCP) which produces flat dicts with named keys including `_type`, matching the Python-side expectation. This is explicitly stated in ANVILML_DESIGN.md §8.6. |
| `WorkerEvent::Ready` contains `Vec<NodeTypeDescriptor>` which is a large nested type — serialisation may produce deep msgpack structures that could hit implementation limits | Low | Medium | `rmp-serde` handles arbitrary nesting depth; no known limits at the sizes expected (typical node count < 20). No action needed. |
| Missing `thiserror` dependency in `anvilml-ipc` — `IpcError` needs `#[derive(Error)]` but `thiserror` is not listed in `anvilml-ipc/Cargo.toml` | Low | High | Add `thiserror = { workspace = true }` to `anvilml-ipc/Cargo.toml`. The workspace version is `2.0.18`. |

## Acceptance Criteria

- [ ] `cargo check -p anvilml-ipc` exits 0
- [ ] `cargo test -p anvilml-ipc -- messages` exits 0 (all roundtrip tests pass)
- [ ] `grep "^## " .forge/reports/P8-A1_plan.md` returns exactly 11 headings
- [ ] `head -1 .forge/reports/P8-A1_plan.md` prints `# Plan Report: P8-A1`
- [ ] `wc -l .forge/reports/P8-A1_plan.md` returns > 40 lines
