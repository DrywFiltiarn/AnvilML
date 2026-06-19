# Plan Report: P11-A3

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P11-A3                                      |
| Phase       | 011 — Dynamic Node Registry                 |
| Description | anvilml-server: GET /v1/nodes listing registered node types |
| Depends on  | P11-A1, P11-A2                              |
| Project     | anvilml                                     |
| Planned at  | 2026-06-19T16:30:00Z                        |
| Attempt     | 1                                           |

## Objective

Expose `GET /v1/nodes` returning the current contents of `NodeTypeRegistry` as a JSON array of `NodeTypeDescriptor` objects. When no worker has ever reached `Ready` (`has_been_updated() == false`), return `503 workers_unavailable`. After any worker reaches `Ready` (even with an empty `node_types` list, as in mock mode), return `200 OK` with a JSON array (empty `[]` for mock, populated for real workers). This completes the dynamic node type registry phase by making the registry queryable from HTTP clients.

## Scope

### In Scope
- Create `crates/anvilml-server/src/handlers/nodes.rs` with `list_nodes` handler function
- Add `node_registry: Arc<NodeTypeRegistry>` field to `AppState` in `state.rs`
- Extend all three `AppState` constructors (`new`, `new_with_hardware`, `new_with_hardware_no_workers`) to accept and store the new field
- Update **all 6 existing test files** in `crates/anvilml-server/tests/` that call `AppState::new(...)` — see the exact file list and call-site counts in the Risks table below. Do not skip any of these 6 files.
- Mount `GET /v1/nodes` route in `build_router()` in `lib.rs`
- Import and re-export `list_nodes` from `handlers/mod.rs`
- Reorder `node_registry` construction in `backend/src/main.rs` so it runs before `temp_state` construction, then pass `Arc::clone(&node_registry)` into both `new_with_hardware_no_workers` and `new_with_hardware`
- Create `crates/anvilml-server/tests/nodes_tests.rs` with integration tests
- **Regenerate and stage `api/openapi.json`** — this task adds a new `#[utoipa::path]` annotation, which means Gate 2 (OpenAPI Drift) WILL fire. This is a required step of this task, not an optional follow-up. See the "CI Impact" section below — do not skip it and do not treat it as "not applicable."
- Update `docs/TESTS.md` with test catalogue entries

### Out of Scope
- Python worker node type registration (handled by P11-B1)
- Graph validation against node types (handled by P11-A2's successor, P11-A4 or later)
- WebSocket event for node type changes

## Existing Codebase Assessment

**What exists:** `NodeTypeRegistry` is already implemented in `crates/anvilml-core/src/node_registry.rs` with methods `new()`, `update_from_worker(worker_id, types)`, `get(type_name)`, `all_types()`, `is_empty()`, and `has_been_updated()`. It is re-exported from `anvilml_core::NodeTypeRegistry` and re-re-exported from `anvilml_scheduler::NodeTypeRegistry`. The registry uses `Arc<RwLock<HashMap<String, NodeTypeDescriptor>>>` internally with an `AtomicBool` for the updated flag.

**Established patterns:** Handler functions follow two styles in this crate. `health.rs`, `system.rs`, and `workers.rs` use `pub async fn` with `pub use` re-exports from `handlers/mod.rs` (and further re-exports from `lib.rs`). `models.rs` uses `pub(crate) async fn` without re-export. The `build_router()` function in `lib.rs` uses `.route("/path", get(handler))` pattern with imports at the top of the file. `AppState` derives `Clone`, uses `Arc` for shared state, and has three constructors: `new()` (async, test stub with in-memory DB), `new_with_hardware()` (production with worker pool), and `new_with_hardware_no_workers()` (temporary state for broadcaster extraction before worker spawn).

**Gap between design doc and current source:** The task context originally referred to `anvilml_core::node_registry`'s module doc for `has_been_updated`'s rationale — this documentation is present and correct in the actual source. The task context also mentioned `is_empty()` being the wrong check; this is confirmed by reading the actual `has_been_updated()` method's doc comment, which explicitly documents why `is_empty()` cannot distinguish "no worker reached Ready" from "worker reached Ready with zero types".

**`main.rs` ordering (already verified — do not re-derive this, just implement it):** `backend/src/main.rs` currently constructs `temp_state` via `AppState::new_with_hardware_no_workers(...)` at line 163, and only constructs `node_registry` at line 176 — 13 lines later. Since `new_with_hardware_no_workers` is gaining a mandatory `node_registry` parameter in this task, the `node_registry` construction line must move to run *before* line 163. See Approach step 8 for the exact instruction.

## Resolved Dependencies

| Type   | Name              | Version verified | MCP source | Feature flags confirmed |
|--------|-------------------|-----------------|------------|------------------------|
| crate  | anvilml-core      | 0.1.14 (workspace) | Cargo.toml | none (path dep)        |
| crate  | anvilml-scheduler | 0.1.19 (workspace) | Cargo.toml | mock-hardware (forwarded) |

No new external dependencies are introduced. All types used (`NodeTypeRegistry`, `NodeTypeDescriptor`) are already in `anvilml-core`. The `anvilml-server` crate already depends on `anvilml-scheduler` (which re-exports `NodeTypeRegistry`), and the handler will import `NodeTypeRegistry` from `anvilml_core::NodeTypeRegistry` directly.

## Approach

1. **Add `node_registry` field to `AppState`** (`state.rs`):
   - Add `pub node_registry: Arc<anvilml_core::NodeTypeRegistry>` field to the struct, after `workers` and before the closing brace.
   - Add a doc comment explaining the field: "Thread-safe node type registry populated from worker Ready events. Shared via Arc so all handlers can query registered node types."

2. **Extend `AppState::new()` constructor** (`state.rs`):
   - Add `node_registry: Arc<anvilml_core::NodeTypeRegistry>` parameter.
   - Store it as `node_registry`.
   - This is an async constructor used only for tests/stubs — every test call site will construct `Arc::new(anvilml_core::NodeTypeRegistry::new().await)` and pass it in. See step 9 for the full, exact list of call sites this affects.

3. **Extend `AppState::new_with_hardware()` constructor** (`state.rs`):
   - Add `node_registry: Arc<anvilml_core::NodeTypeRegistry>` parameter.
   - Store it as `node_registry`.
   - This constructor is used at production server boot.

4. **Extend `AppState::new_with_hardware_no_workers()` constructor** (`state.rs`):
   - Add `node_registry: Arc<anvilml_core::NodeTypeRegistry>` parameter.
   - Store it as `node_registry`.
   - This constructor is used for the temporary AppState at server startup to obtain the EventBroadcaster.

5. **Create `handlers/nodes.rs`** with the `list_nodes` handler:
   - Signature: `pub async fn list_nodes(State(state): State<AppState>) -> Result<Json<Vec<anvilml_core::NodeTypeDescriptor>>, AnvilError>`
   - Logic: await `state.node_registry.has_been_updated()`. If `false`, return `Err(AnvilError::WorkersUnavailable("no worker has reached Ready".to_string()))`. If `true`, await `state.node_registry.all_types()` and return `Json(types)`.
   - Use `pub async fn` (matching the pattern in `health.rs`, `system.rs`, `workers.rs`) so it can be imported and re-exported from `mod.rs`.
   - Add `#[utoipa::path]` annotation for OpenAPI documentation, matching the style in `workers.rs`. **Adding this annotation is exactly what makes Gate 2 fire later — that is expected and required, not a mistake. See the "CI Impact" section below.**

6. **Mount the route in `lib.rs`**:
   - Add `use handlers::nodes::list_nodes;` at the top of the file (after existing imports).
   - Add `.route("/v1/nodes", get(list_nodes))` in the `build_router()` chain, after the `/v1/workers/{id}/restart` route and before the WebSocket route.

7. **Re-export from `handlers/mod.rs`**:
   - Add `pub mod nodes;` to the module declarations.
   - Add `pub use nodes::list_nodes;` to the re-exports.

8. **Reorder `node_registry` construction in `backend/src/main.rs`**:
   - Move the existing `let node_registry = Arc::new(NodeTypeRegistry::new().await);` line (currently at line 176) so it runs *before* `temp_state` construction (currently at line 163).
   - Pass `Arc::clone(&node_registry)` into `AppState::new_with_hardware_no_workers(...)` at the temp_state construction.
   - Pass `Arc::clone(&node_registry)` into `AppState::new_with_hardware(...)` at the real state construction.
   - This is a simple reorder — one `Arc`, cloned twice, no new construction needed.

9. **Update every existing test file that calls `AppState::new(...)`.** There are exactly **6 files, 16 total call sites** — verified directly against the current repo, not estimated. Do not search for other files; this list is complete and exact:

   | File | Call sites |
   |------|-------------|
   | `crates/anvilml-server/tests/handler_tests.rs` | 3 |
   | `crates/anvilml-server/tests/health_tests.rs` | 1 |
   | `crates/anvilml-server/tests/models_tests.rs` | 5 |
   | `crates/anvilml-server/tests/state_tests.rs` | 5 |
   | `crates/anvilml-server/tests/system_tests.rs` | 1 |
   | `crates/anvilml-server/tests/workers_tests.rs` | 1 |

   For every one of these 16 call sites, change `AppState::new("...").await` to `AppState::new("...", Arc::new(anvilml_core::NodeTypeRegistry::new().await)).await` (add the `use std::sync::Arc;` and `use anvilml_core::NodeTypeRegistry;` imports to each file if not already present). The compiler will report a missing-argument error (`E0061`) at each remaining call site you miss — keep fixing and re-running `cargo check -p anvilml-server --features mock-hardware --tests` until it reports zero errors, then confirm the count of fixed sites equals 16.

   **Do not touch** `crates/anvilml-server/tests/stats_tick_tests.rs` or `crates/anvilml-server/tests/broadcaster_tests.rs` for this reason — both files exist, but neither one calls `AppState::new(...)` anywhere (`stats_tick_tests.rs` only calls `WorkerPool::new(...)`, which is unrelated and already takes no registry argument; `broadcaster_tests.rs` only constructs `EventBroadcaster` directly). If you find yourself editing either of these two files to add a `node_registry` argument, stop — that file does not need this change, and adding it means you are editing the wrong call.

10. **Create integration test file** (`crates/anvilml-server/tests/nodes_tests.rs`):
    - Test 1: `test_nodes_returns_503_when_registry_not_updated` — builds `AppState::new("test", Arc::new(NodeTypeRegistry::new().await)).await` (a fresh, never-updated registry), sends GET `/v1/nodes`, asserts 503.
    - Test 2: `test_nodes_returns_200_after_worker_ready` — builds a registry, calls `registry.update_from_worker("worker-0", vec![]).await` on it before wrapping it in `Arc::new(...)` and passing it to `AppState::new(...)`, sends GET `/v1/nodes`, asserts 200 with empty array `[]`.

11. **Bump `anvilml-server` patch version** in `Cargo.toml`: `0.1.19` → `0.1.20`.

12. **Update `docs/TESTS.md`** with entries for the two new tests.

## Public API Surface

| Item | Crate/Module Path | Description |
|------|-------------------|-------------|
| `GET /v1/nodes` | `anvilml-server` route | Returns JSON array of `NodeTypeDescriptor` or 503 |
| `pub async fn list_nodes` | `anvilml_server::handlers::nodes` | Handler function — extracts `State<AppState>`, checks registry, returns `Result<Json<Vec<NodeTypeDescriptor>>, AnvilError>` |
| `node_registry: Arc<NodeTypeRegistry>` | `anvilml_server::state::AppState` | New field in shared application state |

No new `pub` items on structs or traits — only a new handler function and one new field on an existing struct.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-server/src/handlers/nodes.rs` | New handler module with `list_nodes` |
| MODIFY | `crates/anvilml-server/src/lib.rs` | Mount `GET /v1/nodes` route; import `list_nodes` |
| MODIFY | `crates/anvilml-server/src/handlers/mod.rs` | Add `pub mod nodes;` and `pub use nodes::list_nodes;` |
| MODIFY | `crates/anvilml-server/src/state.rs` | Add `node_registry` field + extend all 3 constructors |
| MODIFY | `crates/anvilml-server/Cargo.toml` | Bump patch version 0.1.19 → 0.1.20 |
| CREATE | `crates/anvilml-server/tests/nodes_tests.rs` | Integration tests for GET /v1/nodes |
| MODIFY | `backend/src/main.rs` | Reorder `node_registry` construction before `temp_state`, pass into both AppState constructors |
| MODIFY | `crates/anvilml-server/tests/handler_tests.rs` | Add `node_registry` argument to all 3 `AppState::new(...)` call sites |
| MODIFY | `crates/anvilml-server/tests/health_tests.rs` | Add `node_registry` argument to the 1 `AppState::new(...)` call site |
| MODIFY | `crates/anvilml-server/tests/models_tests.rs` | Add `node_registry` argument to all 5 `AppState::new(...)` call sites |
| MODIFY | `crates/anvilml-server/tests/state_tests.rs` | Add `node_registry` argument to all 5 `AppState::new(...)` call sites |
| MODIFY | `crates/anvilml-server/tests/system_tests.rs` | Add `node_registry` argument to the 1 `AppState::new(...)` call site |
| MODIFY | `crates/anvilml-server/tests/workers_tests.rs` | Add `node_registry` argument to the 1 `AppState::new(...)` call site |
| MODIFY | `api/openapi.json` | Regenerated via `cargo run -p anvilml-openapi` — see Gate 2 section. This file changes as a direct, required consequence of step 5's new `#[utoipa::path]` annotation, not as a side effect to discover later. |
| MODIFY | `docs/TESTS.md` | Add test catalogue entries for new tests |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-server/tests/nodes_tests.rs` | `test_nodes_returns_503_when_registry_not_updated` | `GET /v1/nodes` returns 503 when no worker has reached Ready (fresh registry) | `AppState` built with `new()`, passed a freshly constructed `NodeTypeRegistry::new().await` that has never had `update_from_worker` called on it | GET `/v1/nodes` | HTTP 503, body contains `"error": "workers_unavailable"` | `cargo test -p anvilml-server --features mock-hardware --test nodes_tests test_nodes_returns_503_when_registry_not_updated` exits 0 |
| `crates/anvilml-server/tests/nodes_tests.rs` | `test_nodes_returns_200_after_worker_ready` | `GET /v1/nodes` returns 200 with `[]` after a mock worker reaches Ready (empty node_types) | `AppState` built with a registry that had `update_from_worker("worker-0", vec![]).await` called on it before being wrapped in `Arc` and passed to `AppState::new` | GET `/v1/nodes` | HTTP 200, body is `[]` (empty JSON array) | `cargo test -p anvilml-server --features mock-hardware --test nodes_tests test_nodes_returns_200_after_worker_ready` exits 0 |

## CI Impact

The new test file follows the established pattern of `crates/{name}/tests/{name}_tests.rs`, which `cargo test --workspace` picks up automatically. No new CI job is added.

**Gate 2 (`openapi-drift`) WILL fire for this task — read this carefully, it corrects an error from an earlier draft of this plan.** `ENVIRONMENT.md`'s Gate 2 trigger condition is: *"any task that modifies handler function signatures, `#[utoipa::path]` annotations, `ToSchema` derives, or `AppState` fields used in response types."* Approach step 5 adds a brand new `#[utoipa::path]` annotation to `list_nodes`. **This means Gate 2 is triggered by this task — there is no scenario in which it is skipped.**

This exact situation already happened once before, for a different new route: `P9-C1` (which added `GET /v1/workers`) hit the same trigger, and its implement report shows exactly what to do:

```bash
cargo run -p anvilml-openapi && git diff --exit-code api/openapi.json
```

This command will report a non-empty diff the first time you run it after adding the `/v1/nodes` route, because `api/openapi.json` does not yet contain that route. **This is expected, not an error.** When that happens:

```bash
cargo run -p anvilml-openapi
git add api/openapi.json
```

Then re-run the gate command above to confirm it now exits 0 (no diff — the file is in sync). Stage the regenerated `api/openapi.json` as part of this task's changes, the same way `P9-C1` did.

**Do not write "Gate 2: Not triggered" or "Gate 2: Not applicable" anywhere in the implementation report for this task — for this task, that statement is false. Write "Gate 2: PASSED" with the command output, the same way `P9-C1`'s implement report did.**

## Platform Considerations

None identified. The `GET /v1/nodes` handler is purely in-memory: it reads from an `Arc<RwLock<HashMap>>` under a read lock and serialises to JSON. No platform-specific code, no file I/O, no socket operations. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `AppState::new()` constructor change breaks existing test call sites that don't pass `node_registry` | High | High | Exactly 6 files, 16 call sites need updating — the precise list is in Approach step 9 and the Files Affected table above. Use that list directly; do not search for additional files, and do not edit `stats_tick_tests.rs` or `broadcaster_tests.rs` (neither calls `AppState::new`). After editing all 16 sites, run `cargo check -p anvilml-server --features mock-hardware --tests` — it must report zero errors. If it reports any remaining `E0061` (missing argument) error, find that exact call site and add the missing argument; the compiler output tells you the file and line. |
| `build_router` compilation fails because `node_registry` field is missing from one of the three constructors | Medium | High | The compiler will fail at the `with_state(state)` line if any constructor doesn't populate the new field. This is a compile-time error, not a runtime issue. The fix is straightforward: ensure all three constructors accept and store the parameter. |
| Forgetting that Gate 2 fires for this task, and either skipping the OpenAPI regeneration step or incorrectly writing "Gate 2: Not triggered" in the implementation report | Medium | Medium | This was an error in an earlier draft of this plan and has been corrected — see the dedicated Gate 2 section above. Follow it exactly: run `cargo run -p anvilml-openapi`, stage `api/openapi.json`, re-run the gate command to confirm it now exits 0, and write "Gate 2: PASSED" with the command output in the implementation report. |
| `handler_tests.rs` builds `AppState::new("test-version").await` and passes it to `build_router()` — since the constructor now requires a `node_registry` parameter, every one of this file's 3 call sites must be updated, not just the first one found | Medium | Medium | `handler_tests.rs` is listed in Approach step 9's table with exactly 3 call sites. Update all 3, not just the first match found via a quick search-and-replace — verify by re-running `grep -c "AppState::new(" crates/anvilml-server/tests/handler_tests.rs` after editing and confirming it still finds exactly 3 lines, all now with the new argument. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-server --features mock-hardware --test nodes_tests` exits 0
- [ ] `cargo test -p anvilml-server --features mock-hardware` exits 0 (all server tests pass, including all 16 updated call sites across the 6 files in Approach step 9)
- [ ] `cargo test --workspace --features mock-hardware` exits 0 (full workspace test suite passes)
- [ ] `cargo run -p anvilml-openapi && git diff --exit-code api/openapi.json` exits 0 (Gate 2 — must be run, must pass, must NOT be skipped or marked "not applicable")
- [ ] `curl -s http://127.0.0.1:8488/v1/nodes` returns `{"error":"workers_unavailable","message":"no worker has reached Ready","request_id":"..."}` with HTTP 503 before any worker Ready (manual integration check)
- [ ] `head -1 .forge/reports/P11-A3_plan.md` prints `# Plan Report: P11-A3`
- [ ] `grep "^## " .forge/reports/P11-A3_plan.md` shows all 12 required section headings
- [ ] `wc -l .forge/reports/P11-A3_plan.md` reports > 40 lines
