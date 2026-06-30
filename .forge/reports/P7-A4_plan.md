# Plan Report: P7-A4

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P7-A4                                         |
| Phase       | 007 — IPC Foundations                         |
| Description | anvilml-ipc: WorkerEvent job-lifecycle variants |
| Depends on  | P7-A3                                         |
| Project     | anvilml                                       |
| Planned at  | 2026-06-30T21:15:00Z                          |
| Attempt     | 1                                             |

## Objective

Complete the `WorkerEvent` enum in `crates/anvilml-ipc/src/messages.rs` by adding the five job-lifecycle variants (`Progress`, `ImageReady`, `Completed`, `Failed`, `Cancelled`) that `P7-A3` deferred, and add `pub use messages::{WorkerMessage, WorkerEvent};` to `lib.rs`. This finishes the full Python-to-Rust event vocabulary defined in `ANVILML_DESIGN.md §8.6`, enabling the downstream `RouterTransport::recv()` method (P7-B2) to deserialise any event a Python worker can emit.

## Scope

### In Scope
- Extend `WorkerEvent` enum in `crates/anvilml-ipc/src/messages.rs` with five new variants:
  - `Progress { job_id: Uuid, step: u32, total_steps: u32, preview_b64: Option<String> }`
  - `ImageReady { job_id: Uuid, image_b64: String, width: u32, height: u32, format: String, seed: i64, steps: u32 }`
  - `Completed { job_id: Uuid, elapsed_ms: u64 }`
  - `Failed { job_id: Uuid, error: String, traceback: Option<String> }`
  - `Cancelled { job_id: Uuid }`
- Each variant uses exact field names and types from `ANVILML_DESIGN.md §8.6`.
- Update the module-level doc comment on `WorkerEvent` to remove the "deferred to P7-A4" note (the event is now complete).
- Add `pub use messages::{WorkerMessage, WorkerEvent};` to `crates/anvilml-ipc/src/lib.rs`.
- Add five msgpack roundtrip tests in `crates/anvilml-ipc/tests/roundtrip_tests.rs`, one per new variant.
- Bump `anvilml-ipc` crate version from `0.1.4` to `0.1.5` in `Cargo.toml`.

### Out of Scope
None. This task's `defers_to` field is `[]` (empty). No scope is deferred. All functionality described in the task context and `ANVILML_DESIGN.md §8.6` for these variants is implemented in full.

## Existing Codebase Assessment

The `anvilml-ipc` crate already has a working `WorkerEvent` enum with four variants (`Ready`, `Pong`, `Dying`, `MemoryReport`) defined in `messages.rs`, following the exact pattern of `#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]` + `#[serde(tag = "_type")]`. The existing tests in `roundtrip_tests.rs` use `rmp_serde::to_vec_named` / `rmp_serde::from_slice` for msgpack roundtrip verification, following a consistent per-variant test structure.

The `lib.rs` currently re-exports `IpcError` and `EventBroadcaster` but does **not** yet re-export `WorkerMessage` or `WorkerEvent` from the `messages` module — this task adds that missing re-export.

No new external dependencies are needed: `uuid` (v1.23.4, already in `Cargo.toml` with `serde` + `v4` features) and `rmp-serde` (v1.3.1, already in `[dev-dependencies]`) cover all type and serialization requirements.

## Resolved Dependencies

| Type   | Name     | Version verified | MCP source      | Feature flags confirmed |
|--------|----------|-----------------|-----------------|------------------------|
| crate  | uuid     | 1.23.4          | rust-docs MCP   | serde, v4              |
| crate  | rmp-serde| 1.3.1           | rust-docs MCP   | n/a (dev-dependency)   |

No new dependencies are introduced. Both `uuid` and `rmp-serde` already exist in `Cargo.toml` with the correct features. The `Uuid` type's `Serialize`/`Deserialize` derives are confirmed present in v1.23.4 via the `serde` feature (already enabled).

## Approach

1. **Extend `WorkerEvent` enum in `messages.rs`.** After the existing `MemoryReport` variant, add five new variants using the exact field names, types, and order from `ANVILML_DESIGN.md §8.6`:
   - `Progress { job_id: Uuid, step: u32, total_steps: u32, preview_b64: Option<String> }`
   - `ImageReady { job_id: Uuid, image_b64: String, width: u32, height: u32, format: String, seed: i64, steps: u32 }`
   - `Completed { job_id: Uuid, elapsed_ms: u64 }`
   - `Failed { job_id: Uuid, error: String, traceback: Option<String> }`
   - `Cancelled { job_id: Uuid }`
   
   Each variant gets a `///` doc comment describing its purpose and fields, following the style of existing variants (e.g. `Ready`'s doc comment in lines 78-108 of `messages.rs`). The `#[serde(tag = "_type")]` attribute already covers all variants — no change needed there.

2. **Update the module-level doc comment on `WorkerEvent`.** Remove the sentence "Job-lifecycle variants (`Progress`, `ImageReady`, `Completed`, `Failed`, `Cancelled`) are deferred to task P7-A4" from the existing doc comment (line 72-73 of `messages.rs`), since those variants are now implemented. Update the list of event categories in the doc comment to include "job-lifecycle events" alongside "startup reports, keepalive pongs, and memory reports."

3. **Add `pub use messages::{WorkerMessage, WorkerEvent};` to `lib.rs`.** Add this line after `pub mod messages;` (which already exists implicitly via the module declaration). The `messages` module is already declared as `pub mod messages;` — no module declaration change needed, only the re-export.

4. **Add five roundtrip tests in `roundtrip_tests.rs`.** Following the exact pattern of existing tests (construct a variant, serialize with `rmp_serde::to_vec_named`, deserialize with `rmp_serde::from_slice`, assert equality):
   - `test_progress_roundtrip` — `Progress { job_id: Uuid::new_v4(), step: 3, total_steps: 20, preview_b64: Some("iVBORw0KGgo...".into()) }`
   - `test_image_ready_roundtrip` — `ImageReady { job_id: Uuid::new_v4(), image_b64: "iVBORw0KGgo...".into(), width: 512, height: 512, format: "png".into(), seed: 42, steps: 20 }`
   - `test_completed_roundtrip` — `Completed { job_id: Uuid::new_v4(), elapsed_ms: 5432 }`
   - `test_failed_roundtrip` — `Failed { job_id: Uuid::new_v4(), error: "CUDA out of memory".into(), traceback: Some("Traceback...".into()) }`
   - `test_cancelled_roundtrip` — `Cancelled { job_id: Uuid::new_v4() }`

   Each test is a `#[test] fn` (not async, following existing pattern), uses `Uuid::new_v4()` for unique job IDs, and follows the same assertion style as existing tests.

5. **Bump crate version in `Cargo.toml`.** Change `version = "0.1.4"` to `version = "0.1.5"` in `crates/anvilml-ipc/Cargo.toml`.

6. **Verify.** Run `cargo test -p anvilml-ipc --test roundtrip_tests` — must exit 0 with >=14 tests total (9 existing + 5 new).

## Public API Surface

### New enum variants (messages.rs, non-exhaustive addition)

```rust
pub enum WorkerEvent {
    // ... existing variants unchanged ...
    
    /// Job execution progress report — sent periodically during generation.
    Progress {
        job_id: Uuid,
        step: u32,
        total_steps: u32,
        preview_b64: Option<String>,
    },
    
    /// Generated image is ready — sent after a successful decode step.
    ImageReady {
        job_id: Uuid,
        image_b64: String,
        width: u32,
        height: u32,
        format: String,  // "png"
        seed: i64,
        steps: u32,
    },
    
    /// Job completed successfully.
    Completed {
        job_id: Uuid,
        elapsed_ms: u64,
    },
    
    /// Job failed with an error.
    Failed {
        job_id: Uuid,
        error: String,
        traceback: Option<String>,
    },
    
    /// Job was cancelled by the client.
    Cancelled {
        job_id: Uuid,
    },
}
```

### New re-export (lib.rs)

```rust
pub use messages::{WorkerMessage, WorkerEvent};
```

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | crates/anvilml-ipc/src/messages.rs | Add 5 WorkerEvent job-lifecycle variants, update doc comment |
| Modify | crates/anvilml-ipc/src/lib.rs | Add `pub use messages::{WorkerMessage, WorkerEvent};` |
| Modify | crates/anvilml-ipc/tests/roundtrip_tests.rs | Add 5 msgpack roundtrip tests, one per new variant |
| Modify | crates/anvilml-ipc/Cargo.toml | Bump patch version 0.1.4 → 0.1.5 |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| crates/anvilml-ipc/tests/roundtrip_tests.rs | test_progress_roundtrip | `WorkerEvent::Progress` serialises and roundtrips via rmp-serde, preserving all 4 fields including `preview_b64: Some(...)` | `cargo test -p anvilml-ipc --test roundtrip_tests test_progress_roundtrip` exits 0 |
| crates/anvilml-ipc/tests/roundtrip_tests.rs | test_image_ready_roundtrip | `WorkerEvent::ImageReady` serialises and roundtrips via rmp-serde, preserving all 7 fields (job_id, image_b64, width, height, format, seed, steps) | `cargo test -p anvilml-ipc --test roundtrip_tests test_image_ready_roundtrip` exits 0 |
| crates/anvilml-ipc/tests/roundtrip_tests.rs | test_completed_roundtrip | `WorkerEvent::Completed` serialises and roundtrips via rmp-serde, preserving job_id and elapsed_ms | `cargo test -p anvilml-ipc --test roundtrip_tests test_completed_roundtrip` exits 0 |
| crates/anvilml-ipc/tests/roundtrip_tests.rs | test_failed_roundtrip | `WorkerEvent::Failed` serialises and roundtrips via rmp-serde, preserving job_id, error string, and `traceback: Some(...)` | `cargo test -p anvilml-ipc --test roundtrip_tests test_failed_roundtrip` exits 0 |
| crates/anvilml-ipc/tests/roundtrip_tests.rs | test_cancelled_roundtrip | `WorkerEvent::Cancelled` serialises and roundtrips via rmp-serde, preserving job_id | `cargo test -p anvilml-ipc --test roundtrip_tests test_cancelled_roundtrip` exits 0 |

## CI Impact

No CI changes required. The task only adds enum variants and tests within the existing `anvilml-ipc` crate. The `rust-linux` and `rust-windows` CI jobs already run `cargo test --workspace --features mock-hardware`, which includes `anvilml-ipc`'s test suite. No new file types, gates, or test modules are introduced.

## Platform Considerations

None identified. The `WorkerEvent` enum variants are pure data types with no platform-specific behavior. `Uuid`, `String`, `u32`, `u64`, `i64`, and `Option<T>` all behave identically across all Rust target platforms. The msgpack serialization via `rmp-serde` is platform-neutral.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `serde(tag = "_type")` serialisation of new variants produces a different `_type` discriminator format than expected by the Python msgpack deserialiser (e.g. camelCase vs snake_case) | Low | High | The existing variants already use `#[serde(tag = "_type")]` with `rmp_serde::to_vec_named`, which produces flat dicts with the exact variant name as the `_type` value. The Python side uses `msgpack` which maps Rust's flat dict directly. No custom naming is used, so the format is consistent with existing variants. Verify by checking that existing roundtrip tests (e.g. `test_ready_roundtrip`) already confirm the format. |
| `Uuid` serialisation as a string (not bytes) causes a type mismatch on the Python side | Low | Medium | `uuid` v1.23.4 with the `serde` feature serialises `Uuid` as a string (e.g. `"550e8400-e29b-41d4-a716-446655440000"`), which msgpack maps to a Python `str`. This is the standard and expected format. No change needed. |
| Adding variants to an existing enum could break downstream pattern matching that uses `#[non_exhaustive]` or exhaustive `match` | Low | Low | The enum does not derive `NonExhaustive`, so existing exhaustive matches in downstream crates will get a compile-time error if they don't handle the new variants. This is the correct behavior — it forces all callers to acknowledge the new variants. Downstream crates (anvilml-worker, anvilml-scheduler) will need to be updated in their respective tasks. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-ipc --test roundtrip_tests` exits 0 with >=14 tests total
- [ ] `wc -l crates/anvilml-ipc/src/lib.rs` reports <=80 lines
- [ ] `grep -c "pub use messages::" crates/anvilml-ipc/src/lib.rs` returns >=1 (WorkerMessage and WorkerEvent are re-exported)
- [ ] `grep -c "Progress\|ImageReady\|Completed\|Failed\|Cancelled" crates/anvilml-ipc/src/messages.rs` returns >=5 (all five variants present)
- [ ] `cargo clippy -p anvilml-ipc -- -D warnings` exits 0
