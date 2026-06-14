# Plan Report: P3-A5

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P3-A5                                       |
| Phase       | 003 — Core Domain Types                     |
| Description | anvilml-core: WsEvent enum with all variant subtypes |
| Depends on  | P3-A1, P3-A2, P3-A3, P3-A4                  |
| Project     | anvilml                                     |
| Planned at  | 2026-06-14T19:45:00Z                        |
| Attempt     | 1                                           |

## Objective

Create `crates/anvilml-core/src/types/events.rs` containing the `WsEvent` tagged enum and all ten sub-event structs per `ANVILML_DESIGN.md §5.8`. Update `types/mod.rs` to declare and re-export the events module. Add integration tests in `crates/anvilml-core/tests/events_tests.rs`. The observable outcome is that `cargo test -p anvilml-core -- types::events` exits 0 with ≥ 4 tests verifying JSON roundtrips and the `"type"` discriminator field in serialised output.

## Scope

### In Scope
- Create `crates/anvilml-core/src/types/events.rs` with `WsEvent` enum (`#[serde(tag = "type", rename_all = "snake_case")]`) and its ten variants: `JobQueued`, `JobStarted`, `JobProgress`, `JobImageReady`, `JobCompleted`, `JobFailed`, `JobCancelled`, `WorkerStatusChanged`, `SystemStats`, `ProvisioningProgress`.
- Update `crates/anvilml-core/src/types/mod.rs` to add `pub mod events;` and re-export `WsEvent`.
- Update `crates/anvilml-core/src/lib.rs` to add `WsEvent` to the top-level `pub use types::` re-exports.
- Create `crates/anvilml-core/tests/events_tests.rs` with ≥ 4 integration tests.
- Bump `anvilml-core` patch version from `0.1.7` to `0.1.8` in `crates/anvilml-core/Cargo.toml`.

### Out of Scope
- WebSocket handler implementation in `anvilml-server` (future task).
- Any changes to the IPC `WorkerEvent` / `WorkerMessage` enums in `anvilml-ipc` (those use msgpack with `_type` discriminator; `WsEvent` uses JSON with `"type"` discriminator — different protocol, different crate).
- OpenAPI schema regeneration (handled by `anvilml-openapi` binary in a future task).

## Existing Codebase Assessment

`anvilml-core` already has seven type modules under `src/types/`: `job.rs`, `model.rs`, `artifact.rs`, `hardware.rs`, `worker.rs`, `node.rs`, and `config.rs` / `config_load.rs`. Each follows a consistent pattern: module-level `//!` doc comment, `use serde::{Deserialize, Serialize};` + `use utoipa::ToSchema;` imports, derives of `Debug, Clone, Serialize, Deserialize, ToSchema` on all pub types, `///` doc comments on every field, and integration tests in `crates/anvilml-core/tests/{module}_tests.rs`.

The `WorkerStatus` enum (referenced by `WorkerStatusChanged`) and `WorkerInfo` struct (referenced by `SystemStats`) already exist in `worker.rs` and are re-exported through `types/mod.rs`. No new types need to be created — only `events.rs`.

The design doc (§5.8) specifies `#[serde(tag = "type", rename_all = "snake_case")]` for `WsEvent`, which differs from the IPC msgpack convention (`#[serde(tag = "_type")]`) documented in §119 of `TASKS_PHASE003.md`. This is intentional: WebSocket JSON events use `"type"` as the discriminator key.

The `chrono` crate (with `serde` feature) is already a dependency of `anvilml-core`, so no new imports are needed beyond `serde`, `utoipa::ToSchema`, and `uuid::Uuid`.

## Resolved Dependencies

| Type   | Name       | Version verified | MCP source     | Feature flags confirmed |
|--------|------------|-----------------|----------------|------------------------|
| crate  | serde      | 1.0.228         | Workspace lock | derive                   |
| crate  | serde_json | 1.0.150         | Workspace lock | n/a                      |
| crate  | uuid       | 1.23.3          | Workspace lock | serde, v4                |
| crate  | utoipa     | 5.5.0           | Workspace lock | macros, chrono, uuid     |

No new external dependencies are introduced. All types referenced in this task (`Uuid`, `ToSchema`, `Serialize`, `Deserialize`) are already available through existing workspace dependencies. The `ToSchema` derive macro is confirmed working — all existing type modules use it successfully with utoipa 5.5.0.

## Approach

1. **Create `crates/anvilml-core/src/types/events.rs`** with the `WsEvent` enum and ten sub-event variants. Each variant is a struct-like enum variant with named fields matching `ANVILML_DESIGN.md §5.8` exactly. The module-level doc comment follows the established pattern: describes what the file owns, references the design doc section, and notes the serde tag format.

2. **Add `use` imports at the top of `events.rs`**: `use serde::{Deserialize, Serialize};`, `use utoipa::ToSchema;`, `use uuid::Uuid;`. No `chrono` import is needed — none of the WsEvent variants contain `DateTime<Utc>` fields (unlike `Job` which does).

3. **Derive `Debug, Clone, Serialize, Deserialize, ToSchema`** on `WsEvent` and all ten sub-event structs. This matches the pattern used by every other type module in `anvilml-core`. The `#[serde(tag = "type", rename_all = "snake_case")]` attribute goes on `WsEvent` — it causes each variant to serialise as `{"type": "job_queued", ...fields...}`.

4. **Add `///` doc comments** on `WsEvent` and every sub-event struct, following the pattern from `worker.rs` and `node.rs`: one-sentence summary of what the type represents, followed by a description of key fields or usage context.

5. **Update `crates/anvilml-core/src/types/mod.rs`**: Add `pub mod events;` after the existing `pub mod` declarations. Add `pub use events::WsEvent;` to the `pub use` block. This makes `WsEvent` available as `anvilml_core::WsEvent` and `anvilml_core::types::events::WsEvent`.

6. **Update `crates/anvilml-core/src/lib.rs`**: Add `WsEvent` to the existing `pub use types::` re-export block (line 22-26). This is a one-line edit — append `, WsEvent` to the existing import list.

7. **Create `crates/anvilml-core/tests/events_tests.rs`** with four integration tests:
   - **`test_ws_event_roundtrip_job_image_ready`**: Roundtrip test for `JobImageReady` (the most complex variant with 6 fields), verifying all fields survive JSON serialisation.
   - **`test_ws_event_tag_field_present`**: Serialize a `JobQueued` event to JSON and verify the `"type"` key is present with value `"job_queued"`. This directly verifies the tagged-enum discriminator.
   - **`test_ws_event_all_variants_roundtrip`**: Iterate over all 10 variants, serialise each to JSON and deserialise back, asserting equality. This ensures no variant has a serde mapping bug.
   - **`test_ws_event_system_stats_roundtrip`**: Roundtrip test for `SystemStats` (contains `Vec<WorkerInfo>`), verifying nested type handling works correctly.

8. **Bump `anvilml-core` version** from `0.1.7` to `0.1.8` in `crates/anvilml-core/Cargo.toml` using the procedure from `ENVIRONMENT.md §12`: target only the `[package] version` line.

## Public API Surface

| Item | Type | Module Path | Description |
|------|------|-------------|-------------|
| `WsEvent` | `pub enum` | `anvilml_core::types::events::WsEvent` | Tagged enum for WebSocket broadcast events |
| `WsEvent::JobQueued` | struct variant | `anvilml_core::types::events::WsEvent` | Job placed in queue |
| `WsEvent::JobStarted` | struct variant | `anvilml_core::types::events::WsEvent` | Job dispatched to worker |
| `WsEvent::JobProgress` | struct variant | `anvilml_core::types::events::WsEvent` | Execution progress update |
| `WsEvent::JobImageReady` | struct variant | `anvilml_core::types::events::WsEvent` | Generated image available |
| `WsEvent::JobCompleted` | struct variant | `anvilml_core::types::events::WsEvent` | Job finished successfully |
| `WsEvent::JobFailed` | struct variant | `anvilml_core::types::events::WsEvent` | Job failed with error |
| `WsEvent::JobCancelled` | struct variant | `anvilml_core::types::events::WsEvent` | Job cancelled by user |
| `WsEvent::WorkerStatusChanged` | struct variant | `anvilml_core::types::events::WsEvent` | Worker lifecycle state change |
| `WsEvent::SystemStats` | struct variant | `anvilml_core::types::events::WsEvent` | Periodic system metrics |
| `WsEvent::ProvisioningProgress` | struct variant | `anvilml_core::types::events::WsEvent` | venv provisioning progress |

All ten variants are public through the `WsEvent` enum. The enum itself is re-exported at `anvilml_core::WsEvent`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-core/src/types/events.rs` | WsEvent enum with all 10 sub-event variants |
| CREATE | `crates/anvilml-core/tests/events_tests.rs` | Integration tests for WsEvent |
| MODIFY | `crates/anvilml-core/src/types/mod.rs` | Add `pub mod events;` and `pub use events::WsEvent;` |
| MODIFY | `crates/anvilml-core/src/lib.rs` | Add `WsEvent` to top-level `pub use types::` re-exports |
| MODIFY | `crates/anvilml-core/Cargo.toml` | Bump patch version 0.1.7 → 0.1.8 |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-core/tests/events_tests.rs` | `test_ws_event_roundtrip_job_image_ready` | Full JSON roundtrip for `JobImageReady` with all 6 fields (job_id, artifact_hash, width, height, seed, steps) | None | Constructed `WsEvent::JobImageReady` with concrete values | Deserialised event equals original; all fields match | `cargo test -p anvilml-core -- events_tests::test_ws_event_roundtrip_job_image_ready` exits 0 |
| `crates/anvilml-core/tests/events_tests.rs` | `test_ws_event_tag_field_present` | The `"type"` discriminator key appears in serialised JSON with correct snake_case variant name | None | `WsEvent::JobQueued { job_id, queue_position: 1 }` | JSON contains `"type":"job_queued"` | `cargo test -p anvilml-core -- events_tests::test_ws_event_tag_field_present` exits 0 |
| `crates/anvilml-core/tests/events_tests.rs` | `test_ws_event_all_variants_roundtrip` | All 10 enum variants survive JSON roundtrip without data loss | None | One instance of each variant with minimal but non-default values | All 10 deserialised events equal originals | `cargo test -p anvilml-core -- events_tests::test_ws_event_all_variants_roundtrip` exits 0 |
| `crates/anvilml-core/tests/events_tests.rs` | `test_ws_event_system_stats_roundtrip` | `SystemStats` roundtrip including nested `Vec<WorkerInfo>` — tests that the enum correctly handles cross-type references | `WorkerInfo` types already defined | `WsEvent::SystemStats` with two `WorkerInfo` entries | All fields including nested workers match after roundtrip | `cargo test -p anvilml-core -- events_tests::test_ws_event_system_stats_roundtrip` exits 0 |

## CI Impact

No CI changes required. The new test file `crates/anvilml-core/tests/events_tests.rs` is picked up automatically by `cargo test --workspace --features mock-hardware` (the standard CI command). No new file types, gates, or test modules are introduced beyond the established pattern.

## Platform Considerations

None identified. The `WsEvent` enum is pure data with no I/O, no platform-specific types, and no `#[cfg(...)]` guards. Serialization uses `serde_json` which produces platform-neutral UTF-8 JSON strings. The Windows cross-check in `ENVIRONMENT.md §7` is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `WorkerStatus` and `WorkerInfo` types used in `WorkerStatusChanged` and `SystemStats` variants are not in scope for this task but must already exist. If a prior task (P3-A4) is incomplete, compilation will fail. | Low | High | P3-A4 is listed as a dependency. The ACT agent should verify `WorkerStatus` and `WorkerInfo` exist in `worker.rs` before writing `events.rs`. If missing, STOP and report blocker. |
| The `#[serde(tag = "type", rename_all = "snake_case")]` attribute on a struct-variant enum serialises to `{"type": "job_queued", ...}` — the ACT agent must verify the exact JSON shape matches what the WebSocket handler (future task) expects. A mismatch would require a breaking change to the enum. | Low | Medium | The design doc §5.8 and `TASKS_PHASE003.md` §119 explicitly specify `"type"` (not `"_type"`). The `test_ws_event_tag_field_present` test verifies this. The ACT agent should not deviate from the design spec. |
| `SystemStats` variant contains `Vec<WorkerInfo>`. If `WorkerInfo` does not implement `Serialize`/`Deserialize` (e.g., if a prior task omitted the derives), compilation fails. | Low | High | `WorkerInfo` already derives `Serialize, Deserialize` in `worker.rs` (verified during codebase inspection). The ACT agent should confirm the derives exist before building. |
| The `chrono` crate is a dependency of `anvilml-core` but none of the WsEvent variants use `DateTime<Utc>`, so no chrono import is needed. If a future task adds timestamp fields to a variant, the import would be needed. | Very Low | None | Not applicable for this task. No chrono usage in events.rs. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-core -- events_tests::test_ws_event_roundtrip_job_image_ready` exits 0
- [ ] `cargo test -p anvilml-core -- events_tests::test_ws_event_tag_field_present` exits 0
- [ ] `cargo test -p anvilml-core -- events_tests::test_ws_event_all_variants_roundtrip` exits 0
- [ ] `cargo test -p anvilml-core -- events_tests::test_ws_event_system_stats_roundtrip` exits 0
- [ ] `cargo test -p anvilml-core -- types::events` exits 0 with ≥ 4 tests
- [ ] `grep "^pub use" crates/anvilml-core/src/types/mod.rs | grep WsEvent` matches (WsEvent is re-exported)
- [ ] `grep "WsEvent" crates/anvilml-core/src/lib.rs | grep "pub use"` matches (WsEvent is top-level re-exported)
- [ ] `grep 'version = "0.1.8"' crates/anvilml-core/Cargo.toml` matches (version bumped)
