# Plan Report: P3-A8

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P3-A8                                       |
| Phase       | 003 — Core Domain Types: Data Model         |
| Description | anvilml-core: WsEvent job-lifecycle variants |
| Depends on  | P3-A7                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-06-28T17:50:00Z                        |
| Attempt     | 1                                           |

## Objective

Create `crates/anvilml-core/src/types/events.rs` defining the `WsEvent` enum with its seven job-lifecycle variants, and wire it into the crate's public API via `types/mod.rs`. This gives the rest of the system a compile-time-checked, serde-tagged event vocabulary for the job submission pipeline (queued → started → progress → image-ready → completed/failed/cancelled), which downstream phases (scheduler event mapping in P16-A1, WebSocket handler in P7-A4, OpenAPI docs in P29-D1) will consume directly.

## Scope

### In Scope
- Create `crates/anvilml-core/src/types/events.rs` with the `WsEvent` enum containing exactly seven job-lifecycle variants (JobQueued, JobStarted, JobProgress, JobImageReady, JobCompleted, JobFailed, JobCancelled) per ANVILML_DESIGN.md §5.8.
- Derive `Debug`, `Clone`, `Serialize`, `Deserialize`, `ToSchema` on `WsEvent`.
- Apply `#[serde(tag = "type", rename_all = "snake_case")]` to the enum.
- Add `mod events;` and `pub use events::*;` to `crates/anvilml-core/src/types/mod.rs`.
- Create `crates/anvilml-core/tests/events_tests.rs` with >=7 tests (one per variant), asserting snake_case serde tag roundtrip.

### Out of Scope
- Worker, system, and provisioning event variants (WorkerStatusChanged, SystemStats, ProvisioningProgress) — deferred to P3-A9, which extends the same enum.
- Any code that consumes or emits WsEvent (scheduler mapping, WebSocket broadcast, HTTP handler) — these are later-phase concerns.

## Existing Codebase Assessment

`anvilml-core/src/types/` already contains six modules (artifact, hardware, job, model, node, worker), each following a consistent pattern: a `.rs` file with a single public type (or tightly-related types), `#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]`, and a companion integration test in `crates/anvilml-core/tests/`. The `types/mod.rs` accumulates one `mod <name>;` + `pub use <name>::*;` per task.

The existing test style (exemplified by `node_tests.rs` and `job_tests.rs`) uses `use anvilml_core::types::*;`, constructs each type with explicit field values, serialises to JSON via `serde_json::to_string`, deserialises back, and asserts equality. Tests also parse the JSON into `serde_json::Value` to verify field names and tag values.

`utoipa 5.5.0` is already declared in `Cargo.toml` with the `uuid` and `chrono` features, providing the `ToSchema` derive macro needed for WsEvent. No new dependency is required.

ANVILML_DESIGN.md §5.8 defines the complete WsEvent enum (ten variants); P3-A8 implements the first seven. The field names and types match the design doc exactly.

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| crate  | utoipa  | 5.5.0           | rust-docs MCP  | macros (default), uuid, chrono |

The `ToSchema` derive is provided by the `macros` feature, which is a default feature of utoipa 5.5.0 (confirmed via MCP). The `uuid` and `chrono` features are already enabled in the existing Cargo.toml dependency declaration, so no changes are needed.

## Approach

1. **Create `crates/anvilml-core/src/types/events.rs`** with the `WsEvent` enum:
   - Add a `//!` crate-level doc comment describing this module's purpose: "WebSocket event types for the job lifecycle — the variants broadcast to `/v1/events` subscribers as a job moves through the pipeline."
   - Define `#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]` on the enum.
   - Apply `#[serde(tag = "type", rename_all = "snake_case")]`.
   - Declare exactly seven variants with their fields per ANVILML_DESIGN.md §5.8:
     - `JobQueued { job_id: Uuid, queue_position: u32 }`
     - `JobStarted { job_id: Uuid, worker_id: String }`
     - `JobProgress { job_id: Uuid, step: u32, total_steps: u32, preview_b64: Option<String> }`
     - `JobImageReady { job_id: Uuid, artifact_hash: String, width: u32, height: u32, seed: i64, steps: u32 }`
     - `JobCompleted { job_id: Uuid, elapsed_ms: u64 }`
     - `JobFailed { job_id: Uuid, error: String }`
     - `JobCancelled { job_id: Uuid }`
   - Each variant is a struct-like variant (not unit variant), matching the design doc exactly.

2. **Update `crates/anvilml-core/src/types/mod.rs`**:
   - Add `pub mod events;` after the existing `pub mod worker;` line (consistent with the pattern of one `mod` per task, appended in the order tasks are completed).
   - Add `pub use events::*;` after the existing `pub use worker::*;` line.

3. **Create `crates/anvilml-core/tests/events_tests.rs`** with >=7 tests:
   - One test per variant, following the established pattern from `job_tests.rs` and `node_tests.rs`:
     - Construct the variant with concrete field values.
     - Serialise to JSON.
     - Assert the `"type"` key in the JSON equals the snake_case variant name (e.g. `"job_queued"`, `"job_started"`, `"job_progress"`, `"job_image_ready"`, `"job_completed"`, `"job_failed"`, `"job_cancelled"`).
     - Deserialise back and assert equality with the original.
     - Parse into `serde_json::Value` to verify the tag field name is `"type"` (not a variant-name key).
   - Test names follow the convention: `test_<type>_<variant>_serde_roundtrip`, e.g. `test_ws_event_job_queued_serde_roundtrip`.

4. **Verify compilation**: The task's acceptance criterion is `cargo test -p anvilml-core --test events_tests` exits 0. This confirms the types compile, derive correctly, and roundtrip.

## Public API Surface

| Item | Path | Description |
|------|------|-------------|
| enum | `anvilml_core::types::WsEvent` | Tagged enum with seven job-lifecycle variants, derives Debug+Clone+Serialize+Deserialize+ToSchema, serde tag `"type"` with snake_case rename. |

Variant fields:
- `JobQueued { job_id: Uuid, queue_position: u32 }`
- `JobStarted { job_id: Uuid, worker_id: String }`
- `JobProgress { job_id: Uuid, step: u32, total_steps: u32, preview_b64: Option<String> }`
- `JobImageReady { job_id: Uuid, artifact_hash: String, width: u32, height: u32, seed: i64, steps: u32 }`
- `JobCompleted { job_id: Uuid, elapsed_ms: u64 }`
- `JobFailed { job_id: Uuid, error: String }`
- `JobCancelled { job_id: Uuid }`

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-core/src/types/events.rs` | WsEvent enum with seven job-lifecycle variants |
| MODIFY | `crates/anvilml-core/src/types/mod.rs` | Add `pub mod events;` and `pub use events::*;` |
| CREATE | `crates/anvilml-core/tests/events_tests.rs` | >=7 integration tests, one per variant |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-core/tests/events_tests.rs` | `test_ws_event_job_queued_serde_roundtrip` | JobQueued variant serialises with `"type": "job_queued"`, all fields roundtrip, tag key is `"type"` | None | `job_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap()`, `queue_position = 3` | JSON `{"type":"job_queued","job_id":"550e8400-e29b-41d4-a716-446655440000","queue_position":3}`; deserialises back to equal | `cargo test -p anvilml-core --test events_tests test_ws_event_job_queued_serde_roundtrip` exits 0 |
| `crates/anvilml-core/tests/events_tests.rs` | `test_ws_event_job_started_serde_roundtrip` | JobStarted variant serialises with `"type": "job_started"`, all fields roundtrip | None | `job_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap()`, `worker_id = "gpu:0"` | JSON `{"type":"job_started","job_id":"550e8400-e29b-41d4-a716-446655440000","worker_id":"gpu:0"}`; deserialises back to equal | `cargo test -p anvilml-core --test events_tests test_ws_event_job_started_serde_roundtrip` exits 0 |
| `crates/anvilml-core/tests/events_tests.rs` | `test_ws_event_job_progress_serde_roundtrip` | JobProgress variant serialises with `"type": "job_progress"`, all fields including `preview_b64: None` roundtrip | None | `job_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap()`, `step = 3`, `total_steps = 20`, `preview_b64 = None` | JSON contains `"type":"job_progress"`, `"step":3`, `"total_steps":20`, `"preview_b64":null`; deserialises back to equal | `cargo test -p anvilml-core --test events_tests test_ws_event_job_progress_serde_roundtrip` exits 0 |
| `crates/anvilml-core/tests/events_tests.rs` | `test_ws_event_job_image_ready_serde_roundtrip` | JobImageReady variant serialises with `"type": "job_image_ready"`, all fields roundtrip including `seed: i64` | None | `job_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap()`, `artifact_hash = "abc123"`, `width = 512`, `height = 512`, `seed = 42`, `steps = 20` | JSON contains `"type":"job_image_ready"` and all numeric fields; deserialises back to equal | `cargo test -p anvilml-core --test events_tests test_ws_event_job_image_ready_serde_roundtrip` exits 0 |
| `crates/anvilml-core/tests/events_tests.rs` | `test_ws_event_job_completed_serde_roundtrip` | JobCompleted variant serialises with `"type": "job_completed"`, `elapsed_ms: u64` roundtrips | None | `job_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap()`, `elapsed_ms = 15000` | JSON contains `"type":"job_completed"`, `"elapsed_ms":15000`; deserialises back to equal | `cargo test -p anvilml-core --test events_tests test_ws_event_job_completed_serde_roundtrip` exits 0 |
| `crates/anvilml-core/tests/events_tests.rs` | `test_ws_event_job_failed_serde_roundtrip` | JobFailed variant serialises with `"type": "job_failed"`, error string roundtrips | None | `job_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap()`, `error = "CUDA out of memory"` | JSON contains `"type":"job_failed"`, `"error":"CUDA out of memory"`; deserialises back to equal | `cargo test -p anvilml-core --test events_tests test_ws_event_job_failed_serde_roundtrip` exits 0 |
| `crates/anvilml-core/tests/events_tests.rs` | `test_ws_event_job_cancelled_serde_roundtrip` | JobCancelled variant serialises with `"type": "job_cancelled"`, single field roundtrips | None | `job_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap()` | JSON `{"type":"job_cancelled","job_id":"550e8400-e29b-41d4-a716-446655440000"}`; deserialises back to equal | `cargo test -p anvilml-core --test events_tests test_ws_event_job_cancelled_serde_roundtrip` exits 0 |

## CI Impact

No CI changes required. The new test file `crates/anvilml-core/tests/events_tests.rs` is a standard integration test in the crate's `tests/` directory. The existing CI jobs (`rust-linux`, `rust-windows`) run `cargo test --workspace --features mock-hardware`, which picks up all `tests/*.rs` files under every crate. No new file types, gates, or test modules are introduced.

## Platform Considerations

None identified. The `WsEvent` enum is a pure data type with no platform-specific logic, no `#[cfg(...)]` guards, no path handling, and no I/O. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `ToSchema` derive macro may not be available without an explicit `use utoipa::ToSchema;` import in `events.rs` | Low | Medium | The existing `types/mod.rs` re-exports `utoipa::ToSchema` via `pub use types::*;` — but derives in a submodule need the type in scope. Add `use utoipa::ToSchema;` at the top of `events.rs` (same pattern used by other type modules like `artifact.rs`). |
| Serde tag key conflict: `#[serde(tag = "type")]` on a tagged enum may produce `"type":"job_queued"` but downstream consumers might expect a different key name | Low | High | ANVILML_DESIGN.md §5.8 explicitly specifies `tag = "type"` with `rename_all = "snake_case"`. The design doc is the single source of truth. Confirm the tag key matches exactly by asserting `parsed["type"]` in each test. |
| P3-A9's deferred variants reference `WorkerStatus` and `Vec<WorkerInfo>` — if P3-A8's WsEvent enum is written with different field names than what P3-A9 expects, P3-A9 will fail to compile | Low | Medium | This plan copies the seven job-variant signatures verbatim from ANVILML_DESIGN.md §5.8. The three deferred variants are not written here, so there is no risk of mismatch in this task's scope. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-core --test events_tests` exits 0 (all 7+ tests pass)
- [ ] `grep -c "^fn test_" crates/anvilml-core/tests/events_tests.rs` returns >= 7
- [ ] `grep "pub mod events" crates/anvilml-core/src/types/mod.rs` confirms the module is declared
- [ ] `grep "pub use events" crates/anvilml-core/src/types/mod.rs` confirms the re-export
- [ ] `cargo check -p anvilml-core --features mock-hardware` exits 0 (no compile errors from the new types)
