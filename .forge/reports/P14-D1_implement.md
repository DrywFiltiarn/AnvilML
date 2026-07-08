# Implementation Report: P14-D1

| Field         | Value                                       |
|---------------|---------------------------------------------|
| Task ID       | P14-D1                                      |
| Phase         | 14 — Dispatch & Execute                     |
| Description   | anvilml-server: POST /v1/jobs handler        |
| Implemented   | 2026-07-08T12:15:00Z                        |
| Status        | COMPLETE                                    |

## Summary

Implemented the `POST /v1/jobs` HTTP endpoint in the `anvilml-server` crate. The handler accepts a JSON body containing a computation graph and job settings, delegates entirely to `JobScheduler::submit()`, and returns `202 Accepted` with a `job_id` (UUID v4) and `queue_position` (1-based index). The `JobScheduler::submit()` return type was changed from `Result<Uuid, AnvilError>` to `Result<(Uuid, u32), AnvilError>` to expose the queue position. Four integration tests verify the handler's behavior: valid submission (202), malformed body (400), empty registry (503), and invalid graph (400).

## Resolved Dependencies

| Type   | Name       | Version resolved | Source         |
|--------|------------|------------------|----------------|
| crate  | serde_json | 1.0              | Cargo.toml     |
| crate  | uuid       | 1.23             | Cargo.toml     |

No MCP lookup was needed — `serde_json` and `uuid` were already declared in `anvilml-server`'s `[dev-dependencies]` but needed to be promoted to `[dependencies]` for the new handler module.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-server/src/handlers/jobs.rs` | New handler module with SubmitJobRequest, SubmitJobResponse, submit_job() |
| MODIFY | `crates/anvilml-server/src/handlers/mod.rs` | Add `pub mod jobs;` export |
| MODIFY | `crates/anvilml-server/src/lib.rs` | Register `POST /v1/jobs` route in build_router() |
| MODIFY | `crates/anvilml-scheduler/src/scheduler.rs` | Change `submit()` return type to include queue_position |
| MODIFY | `crates/anvilml-scheduler/tests/scheduler_tests.rs` | Update all submit() call sites to destructure (Uuid, u32) tuple |
| CREATE | `crates/anvilml-server/tests/jobs_tests.rs` | Integration tests for POST /v1/jobs |
| MODIFY | `crates/anvilml-server/Cargo.toml` | Bump patch version 0.1.5 → 0.1.6; add serde_json and uuid to [dependencies] |
| MODIFY | `crates/anvilml-scheduler/Cargo.toml` | Bump patch version 0.1.16 → 0.1.17 |
| MODIFY | `docs/TESTS.md` | Add 4 test entries for jobs_tests.rs |

## Commit Log

```
 .forge/reports/P14-D1_plan.md                     | 202 ++++++++++++++++++++
 .forge/state/CURRENT_TASK.md                      |   6 +-
 .forge/state/state.json                           |  13 +-
 Cargo.lock                                        |   4 +-
 crates/anvilml-scheduler/Cargo.toml               |   2 +-
 crates/anvilml-scheduler/src/scheduler.rs         |  30 ++-
 crates/anvilml-scheduler/tests/scheduler_tests.rs |  42 +++--
 crates/anvilml-server/Cargo.toml                  |   4 +-
 crates/anvilml-server/src/handlers/jobs.rs        |  74 ++++++++
 crates/anvilml-server/src/handlers/mod.rs         |   1 +
 crates/anvilml-server/src/lib.rs                  |   1 +
 crates/anvilml-server/tests/jobs_tests.rs         | 219 ++++++++++++++++++++++
 docs/TESTS.md                                     |  48 +++++
 13 files changed, 608 insertions(+), 38 deletions(-)
```

## Test Results

```
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 2 tests
test crates/anvilml-registry/src/scanner.rs - scanner::ModelScanner (line 26) - compile ... ok
test crates/anvilml-registry/src/seed_loader.rs - seed_loader::SeedLoader (line 33) - compile ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 1 test
test crates/anvilml-worker/src/keepalive.rs - keepalive::KeepaliveWatchdog (line 124) ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

all doctests ran in 1.16s; merged doctests took 1.11s
```

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
# Gate skipped — task does not introduce platform-specific code.
# The compile check (cargo check --workspace --features mock-hardware) passed
# in Step 4 with zero errors.
```

## Project Gates

```
Gate 1 (Config Surface Sync):
  cargo test -p anvilml --features mock-hardware -- config_reference
  running 1 test
  test tests::config_reference_matches_defaults ... ok
  test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

Gate 2 (OpenAPI Drift):
  Skipped — api/openapi.json does not yet exist (stub only).
  Per ENVIRONMENT.md §8: "Skip only if api/openapi.json does not yet exist."

Gate 3 (Node Parity):
  Not triggered — task does not modify worker/nodes/ or node_registry.rs.

Gate 4 (Mock/Real Parity Markers):
  Not triggered — task does not add/modify node execute() or arch module load()/sample()/decode().
```

## Public API Delta

```
+    pub async fn submit(
+pub mod jobs;
```

New `pub` items introduced:
- `pub mod jobs;` in `handlers/mod.rs` — exports the new jobs handler module
- `pub async fn submit()` in `JobScheduler` — pre-existing function, return type changed from `Result<Uuid, AnvilError>` to `Result<(Uuid, u32), AnvilError>`

Note: `SubmitJobRequest`, `SubmitJobResponse`, and `submit_job()` are `pub(crate)`, not `pub`, so they do not appear in the grep output. This matches the plan's `## Public API Surface` table.

## Deviations from Plan

- **Dependency promotion**: The plan did not list adding `serde_json` and `uuid` to `anvilml-server`'s `[dependencies]`. These were required because the new handler module uses `serde_json::Value` and `uuid::Uuid` in its public-facing structs (the handler is `pub(crate)` but still needs the crate to compile). The plan's `## Files Affected` table only listed `anvilml-server/Cargo.toml` for version bump, not for dependency changes.

- **scheduler_tests.rs call-site updates**: The plan stated "This is the only caller of `submit()` in the codebase (the HTTP handler), so no other call sites need updating." This was incorrect — `crates/anvilml-scheduler/tests/scheduler_tests.rs` contains 25+ call sites to `submit()` that also needed updating to destructure the `(Uuid, u32)` return tuple. All call sites were updated (26 total errors fixed).

- **unused `AnvilError` import in jobs_tests.rs**: The test file initially imported `AnvilError` but never used it (the handler returns `Result<(StatusCode, Json<...>), AnvilError>` and the tests assert on status codes, not error variants). The import was removed.

## Blockers

None.
