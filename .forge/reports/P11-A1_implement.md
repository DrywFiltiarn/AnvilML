# Implementation Report: P11-A1

| Field         | Value                              |
|---------------|------------------------------------|
| Task ID       | P11-A1                             |
| Phase         | 11 — Dynamic Node System           |
| Description   | anvilml-worker: ManagedWorker calls node_registry.register_all() on Ready |
| Implemented   | 2026-07-06T13:15:00Z               |
| Status        | COMPLETE                           |

## Summary

Connected the worker lifecycle to the dynamic node registry by adding an `Arc<NodeTypeRegistry>` parameter to `ManagedWorkerConfig` and `ManagedWorker`, then calling `node_registry.register_all(node_types)` inside `handle_event()` when processing a `WorkerEvent::Ready`, before the existing status transition to `Idle`. Added 4 integration tests in `managed_tests.rs` verifying: (1) Ready event populates the registry, (2) empty node_types clears the registry, (3) second Ready replaces rather than merges, and (4) registry is populated before status transitions to Idle.

## Resolved Dependencies

| Type   | Name           | Version verified | Source         |
|--------|----------------|------------------|----------------|
| crate  | NodeTypeRegistry (anvilml-core) | 0.1.26 (workspace internal) | Cargo.toml read | n/a (internal crate) |

No new external dependencies introduced. `NodeTypeRegistry` is already re-exported from `anvilml-core` and `WorkerEvent::Ready` already carries `node_types: Vec<NodeTypeDescriptor>`.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-worker/src/managed.rs` | Added `Arc<NodeTypeRegistry>` to config + struct; updated `new()`; updated `handle_event()` Ready arm to call `register_all()` |
| Modify | `crates/anvilml-worker/src/pool.rs` | Added `NodeTypeRegistry` import and `node_registry` field to `ManagedWorkerConfig` construction in `spawn_all()` |
| Modify | `crates/anvilml-worker/tests/managed_tests.rs` | Added `NodeTypeRegistry`/`NodeTypeDescriptor`/`SlotDescriptor`/`SlotType` imports; added `node_registry` field to all existing test configs; added 4 new integration tests |
| Modify | `crates/anvilml-worker/Cargo.toml` | Bump patch version 0.1.26 → 0.1.27 |
| Modify | `docs/TESTS.md` | Added entries for 4 new tests |

## Commit Log

```
 .forge/reports/P11-A1_plan.md                | 180 +++++++++
 .forge/state/CURRENT_TASK.md                 |   6 +-
 .forge/state/state.json                      |  13 +-
 Cargo.lock                                   |   2 +-
 crates/anvilml-worker/Cargo.toml             |   2 +-
 crates/anvilml-worker/src/managed.rs         |  21 +-
 crates/anvilml-worker/src/pool.rs            |   2 +
 crates/anvilml-worker/tests/managed_tests.rs | 535 ++++++++++++++++++++++++++-
 docs/TESTS.md                                |  48 +++
 9 files changed, 796 insertions(+), 13 deletions(-)
```

## Test Results

```
     Running tests/managed_tests.rs (target/debug/deps/managed_tests-13cc41bc94fa5148)

running 43 tests
test test_clone_independent_worker_id ... ok
test test_clone_shares_status ... ok
test test_default_init_timeout_matches_design_spec ... ok
test test_concurrent_status_and_set_status_no_deadlock ... ok
test test_router_transport_adapter_not_dead_code ... ok
test test_set_status_callable_repeatedly ... ok
test test_request_shutdown_is_idempotent ... ok
test test_request_shutdown_sends_signal ... ok
test test_child_tracked_after_spawn ... ok
test test_set_status_visible_across_clone ... ok
test test_set_status_changes_value ... ok
test test_status_returns_current_value ... ok
test test_spawn_failure_is_respawn_eligible ... ok
test test_respawn_at_limit_exits_permanently ... ok
test test_respawn_kills_previous_child ... ok
test test_should_respawn_called_on_crash ... ok
test test_deregister_called_on_crash ... ok
test test_permanent_crash_force_kills_child ... ok
test test_shutdown_rx_triggers_graceful_exit ... ok
test test_crash_appends_to_attempt_history ... ok
test test_deregister_called_on_graceful_exit ... ok
test test_ready_populates_registry_before_idle_transition ... ok
test test_ready_event_populates_registry ... ok
test test_ready_event_empty_node_types_cleans_registry ... ok
test test_run_completes_on_ready_event ... ok
test test_init_timeout_force_kills_child ... ok
test test_crash_history_grows_per_crash ... ok
test test_watchdog_channel_cleans_up_on_exit ... ok
test test_multi_worker_events_never_cross ... ok
test test_completed_event_transitions_to_idle ... ok
test test_deregister_called_on_initializing_timeout ... ok
test test_failed_event_transitions_to_idle ... ok
test test_cancelled_event_transitions_to_idle ... ok
test test_respawn_delay_matches_next_delay ... ok
test test_worker_reported_dying_force_kills_child ... ok
test test_graceful_shutdown_sends_shutdown_message ... ok
test test_graceful_shutdown_force_kills_after_timeout ... ok
test test_respawn_second_ready_replaces_not_merges ... ok
test test_respawn_under_limit_spawns_again_and_reregisters ... ok
test test_pong_forwarding_does_not_disturb_idle_busy ... ok
test test_respawn_status_transitions_respawning_then_initializing ... ok
test test_watchdog_missing_pong_triggers_crash_path ... ok
test test_watchdog_live_pongs_no_false_trigger ... ok

test result: ok. 43 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Full workspace test suite: all tests passed (43 managed_tests + all other crate tests = 0 failures across workspace).

## Format Gate

```
(cargo fmt --all -- --check exited with 0 — no output, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux:
cargo check --workspace --features mock-hardware
  → Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.99s

# 2. Mock-hardware Windows:
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
  → Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.00s

# 3. Real-hardware Linux:
cargo check --bin anvilml
  → Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.87s

# 4. Real-hardware Windows:
cargo check --bin anvilml --target x86_64-pc-windows-gnu
  → Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.02s

All four checks passed.
```

## Project Gates

```
# Gate 1 — Config Surface Sync:
cargo test -p anvilml --features mock-hardware -- config_reference
  → test tests::config_reference_matches_defaults ... ok
  → test result: ok. 1 passed; 0 failed; 0 ignored

Gate 2 (OpenAPI Drift) — not triggered: task does not modify handler signatures, utoipa annotations, or AppState fields.
Gate 3 (Node Parity) — not triggered: task does not add/remove/renode types or modify node_registry.rs.
Gate 4 (Mock/Real Parity Markers) — not triggered: task does not modify node execute() or arch module load/sample/decode/compute_latent_shape().
```

## Public API Delta

```
+    pub node_registry: Arc<NodeTypeRegistry>,
```

One new `pub` item: `ManagedWorkerConfig::node_registry` — a `pub node_registry: Arc<NodeTypeRegistry>` field. No new `pub` functions, structs, traits, or enums. This matches the plan's Public API Surface table exactly.

## Deviations from Plan

- The `NodeTypeDescriptor` struct has fields `inputs: Vec<SlotDescriptor>` and `outputs: Vec<SlotDescriptor>` (not `Vec<String>`), plus `display_name: String` and `category: String` fields that were not explicitly called out in the plan. The plan's approach of using `NodeTypeDescriptor` directly was followed; the test descriptors were constructed with the correct field names and types as found in the actual codebase.
- The `node_types` field in `WorkerEvent::Ready` is `Vec<NodeTypeDescriptor>` (not `&Vec<NodeTypeDescriptor>`), requiring `.clone()` in `handle_event()` since `register_all()` takes ownership of the `Vec`.

## Blockers

None.
