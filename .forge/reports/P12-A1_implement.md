# Implementation Report: P12-A1

| Field         | Value                                           |
|---------------|-------------------------------------------------|
| Task ID       | P12-A1                                          |
| Phase         | 12 — Scheduler job persistence                  |
| Description   | anvilml-scheduler: job DB row helpers (insert, get, list, update status) |
| Implemented   | 2026-06-07T15:45:00Z                            |
| Status        | BLOCKED                                          |

## Summary

Implemented four async job persistence functions (`insert_job`, `get_job`, `list_jobs`, `update_status`) in a new `job_store` module for the `anvilml-scheduler` crate. Uses SQLite-backed sqlx with integer-second timestamps and TEXT-based UUID/JSON storage. All six unit tests pass. The full workspace test suite has a pre-existing flaky failure in `anvilml-worker` (`spawn_ping_pong`) unrelated to this task.

## Resolved Dependencies

| Type   | Name        | Version resolved | Source         |
|--------|-------------|-----------------|----------------|
| crate  | sqlx        | 0.9             | Workspace dep  |
| crate  | uuid        | 1.23.2          | Workspace dep  |
| crate  | chrono      | 0.4.45          | Workspace dep  |
| crate  | serial_test | 3.5.0           | Workspace dev  |
| crate  | tempfile    | 3.27.0          | Workspace dev  |
| crate  | tokio       | 1.52.3          | Workspace dev (added for tests) |

All dependencies already declared in workspace root `Cargo.toml`; no new external crates introduced.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-scheduler/Cargo.toml` | Add sqlx, uuid, chrono deps; add serial_test, tempfile, tokio dev-deps; bump patch version 0.1.4 → 0.1.5 |
| Create | `crates/anvilml-scheduler/src/job_store.rs` | New module with insert_job, get_job, list_jobs, update_status functions + 6 unit tests |
| Modify | `crates/anvilml-scheduler/src/lib.rs` | Register `pub mod job_store;` and re-export via `pub use job_store::*;` |

## Commit Log

```
 .forge/state/CURRENT_TASK.md              |  6 +++---
 .forge/state/state.json                   | 13 +++++++------
 Cargo.lock                                |  8 +++++++-
 crates/anvilml-scheduler/Cargo.toml       | 10 +++++++++-
 crates/anvilml-scheduler/src/lib.rs       |  2 ++
```

New file (not shown in diff --stat): `crates/anvilml-scheduler/src/job_store.rs` (496 lines)

## Test Results

```
Running unittests src/lib.rs (target/debug/deps/anvilml_scheduler-e77704b6b3c6dc98)

running 16 tests
test dag::tests::test_cycle_detected_2node ... ok
test dag::tests::test_duplicate_node_id ... ok
test dag::tests::test_unknown_output_slot ... ok
test dag::tests::test_unknown_node_type ... ok
test dag::tests::test_unknown_node_ref ... ok
test dag::tests::test_valid_edge_references ... ok
test dag::tests::test_valid_graph ... ok
test dag::tests::test_valid_zit_5node_passes ... ok
test nodes::tests::test_all_nine_types_present ... ok
test nodes::tests::test_zitsampler_outputs_include_latents_seed ... ok
test job_store::tests::test_list_jobs_all ... ok
test job_store::tests::test_list_jobs_before_cursor ... ok
test job_store::tests::test_insert_and_get ... ok
test job_store::tests::test_list_jobs_limit ... ok
test job_store::tests::test_list_jobs_status_filter ... ok
test job_store::tests::test_update_status ... ok

test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

All 6 new `job_store` tests pass, plus all 10 existing scheduler tests (dag + nodes).

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no formatting drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux check
cargo check --workspace --features mock-hardware
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.69s

# 2. Mock-hardware Windows cross-check
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.93s

# 3. Real-hardware Linux check
cargo check --bin anvilml
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.59s

# 4. Real-hardware Windows cross-check
cargo check --bin anvilml --target x86_64-pc-windows-gnu
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.89s
```

All four platform cross-checks exit 0.

## Project Gates

```
# Gate 1 — Config Surface Sync
cargo test -p backend --features mock-hardware -- config_reference
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 8 filtered out
```

Config gate passes (test filtered out — `config_reference` test not yet implemented per P3-B2).

## Deviations from Plan

- **Dependency**: Added `tokio = { workspace = true, features = ["macros", "rt-multi-thread"] }` as a dev-dependency. The plan listed `serial_test` and `tempfile` but tokio is required for `#[tokio::test]` attributes in tests and was not previously a dev-dependency of this crate.
- **SQL approach**: Used inline `sqlx::query!`/`sqlx::query_as!` macros with raw `sqlx::query()` calls instead of `query_file!`. The plan mentioned `query_file!` as an option but not a requirement. Inline queries avoid needing external `.sql` seed files and work without existing sqlx infrastructure.
- **Type mapping**: Stored UUIDs as TEXT and timestamps as INTEGER (unix epoch seconds) in SQLite to avoid sqlx 0.9 type trait requirements for `Uuid` and `DateTime<Utc>`. Parsed/serialized manually via `.to_string()` / `Uuid::parse_str()` and `.timestamp()` / `DateTime::from_timestamp()`.

## Blockers

- **Pre-existing flaky test**: `anvilml-worker::managed::tests::spawn_ping_pong` fails consistently (error: "worker did not reach Ready state in time"). Verified by stashing all changes and running the same test on the original codebase — it also fails. This is a pre-existing timing issue in `crates/anvilml-worker/src/managed.rs`, a file this task does not modify. Per FORGE_AGENT_RULES §9.4, pre-existing errors in files this task does not otherwise touch are blockers.
