# Plan Report: P7-A2

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P7-A2                                             |
| Phase       | 007 — IPC Foundations                             |
| Description | anvilml-ipc: WorkerMessage enum (Rust to Python)  |
| Depends on  | P7-A1                                             |
| Project     | anvilml                                           |
| Planned at  | 2026-06-30T19:55:00Z                              |
| Attempt     | 1                                                 |

## Objective

Create `crates/anvilml-ipc/src/messages.rs` defining the `WorkerMessage` enum — the Rust-to-Python half of the IPC wire protocol. The enum has five variants (`Ping`, `Shutdown`, `Execute`, `CancelJob`, `MemoryQuery`) with msgpack-serialisable fields, using `rmp-serde` for roundtrip testing. This establishes the message vocabulary that every downstream task (transport send/recv, worker pool keepalive, job dispatch) will consume.

## Scope

### In Scope
- Create `crates/anvilml-ipc/src/messages.rs` with the `WorkerMessage` enum per ANVILML_DESIGN.md §8.5.
- Add `mod messages;` declaration to `crates/anvilml-ipc/src/lib.rs`.
- Add `rmp-serde` as a dev-dependency in `crates/anvilml-ipc/Cargo.toml` for msgpack roundtrip tests.
- Add `uuid` with the `serde` feature to dev-dependencies in `Cargo.toml` (the existing `uuid` dep has no `serde` feature, which is needed for `JobSettings` roundtrip).
- Create `crates/anvilml-ipc/tests/roundtrip_tests.rs` (extend existing file) with >=5 msgpack roundtrip tests, one per `WorkerMessage` variant.

### Out of Scope
- `WorkerEvent` enum — this is P7-A3's scope. This task does not define any `WorkerEvent` variants.
- `pub use messages::WorkerMessage;` re-export in `lib.rs` — this is P7-D1's scope (the phase-closing re-export pass).
- Transport layer (`RouterTransport::send()` / `recv()`) — P7-B1/P7-B2's scope.
- `InitializeHardware` message — explicitly excluded by §8.5; hardware init uses the `ANVILML_DEVICE_INDEX` env var.

## Existing Codebase Assessment

The `anvilml-ipc` crate exists as a buildable stub (Phase 1's P1-B4). Phase 7's P7-A1 has already created `error.rs` (with `IpcError` and `From<IpcError> for AnvilError`) and added `thiserror` and `tokio` dependencies. The crate's `lib.rs` declares `pub mod error; pub mod ws;` and re-exports `IpcError` and `EventBroadcaster`.

A `roundtrip_tests.rs` file already exists in `tests/` (populated by P7-C1's EventBroadcaster tests). The test style uses `#[tokio::test]` for async tests and `#[test]` for sync tests, with clear doc comments per test function. The project uses `rmp_serde::to_vec_named` / `rmp_serde::from_slice` for msgpack serialization (per ANVILML_DESIGN.md §8.7).

`JobSettings` is defined in `crates/anvilml-core/src/types/job.rs` and re-exported via `anvilml_core::JobSettings` (through `pub use types::*`). It derives `Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema` and has a single field `device_preference: Option<String>`. No custom serde attributes are applied — the default serde naming (snake_case for fields) is used.

The `uuid` crate is already a dev-dependency in `anvilml-ipc` (`uuid = { version = "1.23.4", features = ["v4"] }`), but lacks the `serde` feature needed for `Uuid` serialization/deserialization in roundtrip tests.

## Resolved Dependencies

| Type   | Name       | Version verified | MCP source     | Feature flags confirmed |
|--------|------------|-----------------|----------------|------------------------|
| crate  | rmp-serde  | 1.3.1           | rust-docs MCP  | none (no feature flags) |
| crate  | uuid       | 1.23.4          | rust-docs MCP  | serde (new feature added) |

`serde` and `serde_json` are already available through `anvilml-core`'s dependency tree (no direct dep needed on `anvilml-ipc`'s own `Cargo.toml` — they come transitively via the `anvilml-core` path dependency).

## Approach

**Step 1: Add dependencies to `crates/anvilml-ipc/Cargo.toml`.**

Add `rmp-serde = "1.3.1"` as a `[dev-dependencies]` entry (msgpack roundtrip tests only; the transport layer's serialization is a later task). Add the `serde` feature to the existing `uuid` dev-dependency: change `uuid = { version = "1.23.4", features = ["v4"] }` to `uuid = { version = "1.23.4", features = ["v4", "serde"] }`. This is needed because `WorkerMessage::Execute` carries a `Uuid` field that must serialize/deserialize via msgpack.

**Step 2: Create `crates/anvilml-ipc/src/messages.rs`.**

Write the file with:
- A crate-level `//!` doc comment describing the module's ownership (wire protocol types for Rust→Python messages).
- The `WorkerMessage` enum with `#[derive(Debug, Clone, Serialize, Deserialize)]` and `#[serde(tag = "_type")]`.
- Five variants exactly as specified in ANVILML_DESIGN.md §8.5:
  - `Ping { seq: u64 }` — keepalive ping
  - `Shutdown` — graceful shutdown (no fields)
  - `Execute { job_id: Uuid, graph: serde_json::Value, settings: JobSettings, device_index: u32 }` — job execution request
  - `CancelJob { job_id: Uuid }` — cooperative job cancellation
  - `MemoryQuery` — memory usage query (no fields)
- Each variant gets a `///` doc comment explaining its purpose (as written in §8.5).
- Import `JobSettings` from `anvilml_core`, `Uuid` from `uuid`, and `serde_json::Value`.
- No custom `Serialize`/`Deserialize` implementations — the derive macros handle everything.

**Step 3: Declare `mod messages;` in `crates/anvilml-ipc/src/lib.rs`.**

Add `pub mod messages;` after the existing `pub mod error;` line. Do NOT add a `pub use messages::WorkerMessage;` re-export — that is P7-D1's scope.

**Step 4: Extend `crates/anvilml-ipc/tests/roundtrip_tests.rs` with >=5 msgpack roundtrip tests.**

Each test follows the established pattern: construct a variant, serialize via `rmp_serde::to_vec_named()`, deserialize via `rmp_serde::from_slice()`, assert equality.

Tests to add (one per variant):
- `test_ping_roundtrip` — `WorkerMessage::Ping { seq: 42 }`
- `test_shutdown_roundtrip` — `WorkerMessage::Shutdown`
- `test_execute_roundtrip` — `WorkerMessage::Execute { job_id: Uuid::new_v4(), graph: serde_json::json!({}), settings: JobSettings { device_preference: None }, device_index: 0 }`
- `test_cancel_job_roundtrip` — `WorkerMessage::CancelJob { job_id: Uuid::new_v4() }`
- `test_memory_query_roundtrip` — `WorkerMessage::MemoryQuery`

Each test is a synchronous `#[test]` (no async needed — msgpack serialization/deserialization are pure, blocking operations). Each test has a doc comment describing what it verifies.

**Step 5: Verify with `cargo test -p anvilml-ipc --test roundtrip_tests`.**

The acceptance criterion is `>=5 tests, exits 0`.

## Public API Surface

```rust
// crates/anvilml-ipc/src/messages.rs (new module)
pub enum WorkerMessage {
    Ping { seq: u64 },
    Shutdown,
    Execute { job_id: Uuid, graph: serde_json::Value, settings: JobSettings, device_index: u32 },
    CancelJob { job_id: Uuid },
    MemoryQuery,
}
```

All derives: `Debug`, `Clone`, `Serialize`, `Deserialize`. The enum is `pub` within the crate; the `pub use messages::WorkerMessage;` re-export to the crate root is deferred to P7-D1.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-ipc/src/messages.rs` | `WorkerMessage` enum with 5 variants, doc comments, serde/msgpack derives |
| MODIFY | `crates/anvilml-ipc/src/lib.rs` | Add `pub mod messages;` declaration |
| MODIFY | `crates/anvilml-ipc/Cargo.toml` | Add `rmp-serde` dev-dep; add `serde` feature to existing `uuid` dep |
| MODIFY | `crates/anvilml-ipc/tests/roundtrip_tests.rs` | Add >=5 msgpack roundtrip tests for each `WorkerMessage` variant |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `crates/anvilml-ipc/tests/roundtrip_tests.rs` | `test_ping_roundtrip` | `WorkerMessage::Ping { seq: 42 }` serialises via rmp-serde and roundtrips to an equal value; msgpack dict contains `"_type": "Ping"` and `"seq": 42` | `cargo test -p anvilml-ipc --test roundtrip_tests test_ping_roundtrip` exits 0 |
| `crates/anvilml-ipc/tests/roundtrip_tests.rs` | `test_shutdown_roundtrip` | `WorkerMessage::Shutdown` (unit variant, no fields) roundtrips via rmp-serde; msgpack dict contains only `"_type": "Shutdown"` | `cargo test -p anvilml-ipc --test roundtrip_tests test_shutdown_roundtrip` exits 0 |
| `crates/anvilml-ipc/tests/roundtrip_tests.rs` | `test_execute_roundtrip` | `WorkerMessage::Execute { job_id, graph, settings, device_index }` roundtrips via rmp-serde; all four fields (`job_id`, `graph`, `settings`, `device_index`) are preserved with correct types (Uuid→string, Value→dict, JobSettings→dict, u32→int) | `cargo test -p anvilml-ipc --test roundtrip_tests test_execute_roundtrip` exits 0 |
| `crates/anvilml-ipc/tests/roundtrip_tests.rs` | `test_cancel_job_roundtrip` | `WorkerMessage::CancelJob { job_id }` roundtrips via rmp-serde; `job_id` field preserved correctly | `cargo test -p anvilml-ipc --test roundtrip_tests test_cancel_job_roundtrip` exits 0 |
| `crates/anvilml-ipc/tests/roundtrip_tests.rs` | `test_memory_query_roundtrip` | `WorkerMessage::MemoryQuery` (unit variant, no fields) roundtrips via rmp-serde; msgpack dict contains only `"_type": "MemoryQuery"` | `cargo test -p anvilml-ipc --test roundtrip_tests test_memory_query_roundtrip` exits 0 |

## CI Impact

No CI changes required. The tests run as part of `cargo test --workspace --features mock-hardware`, which already compiles and runs all test crates in the workspace. The new `roundtrip_tests.rs` tests are automatically picked up by `cargo test -p anvilml-ipc`.

## Platform Considerations

None identified. The `WorkerMessage` enum is a pure data type with no platform-specific behaviour. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are needed. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `rmp-serde::to_vec_named` may not support the `#[serde(tag = "_type")]` enum encoding correctly — the `to_vec_named` function expects named fields, and the `_type` discriminator adds a synthetic field. Incorrect encoding will cause deserialization to fail. | Low | High | Verify the encoding pattern works by writing the roundtrip test first; if `to_vec_named` fails, fall back to `to_vec` (positional encoding is not suitable for tagged enums, but `to_vec_named` with `#[serde(tag = "_type")]` produces flat dicts matching the Python msgpack format). |
| `uuid` crate's `serde` feature serialises `Uuid` as a string (e.g. `"550e8400-e29b-41d4-a716-446655440000"`), but the Python side may expect a different format. The ACT agent will confirm this at implementation time. | Low | Medium | The design doc §8.7 specifies msgpack flat dicts — `uuid`'s string format is standard and interoperable. No custom serialization needed. |
| `JobSettings` from `anvilml-core` may have changed since this plan was written (new fields, renamed fields). The ACT agent should confirm the current `JobSettings` definition matches what this plan expects. | Low | Medium | Read `crates/anvilml-core/src/types/job.rs` at ACT time before writing code; if `JobSettings` has changed, update the `Execute` variant's field types accordingly. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-ipc --test roundtrip_tests` exits 0
- [ ] `grep -c "^fn test_" crates/anvilml-ipc/tests/roundtrip_tests.rs` returns >= 5 (counting WorkerMessage tests)
- [ ] `wc -l crates/anvilml-ipc/src/messages.rs` returns a value <= 80 (lib.rs discipline applies to all source files conceptually, though the hard cap is for lib.rs specifically)
