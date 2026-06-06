# Implementation Report: P10-B1

| Field | Value |
|-------|-------|
| Task ID | P10-B1 |
| Phase | 010 — Worker Crash Recovery |
| Description | anvilml-worker: fix double InitializeHardware write on Windows/Unix in ManagedWorker::spawn |
| Implemented | 2026-06-06T21:00:00Z |
| Status | COMPLETE |

## Summary

Fixed a race-condition bug in `ManagedWorker::spawn()` where `InitializeHardware` was delivered twice to the Python worker process — once via a direct stdin write (fd-dup on Unix, async write+flush on Windows) and again via the mpsc channel. The second delivery caused the Python worker to exit prematurely. Also removed a redundant `Dead` status write at the end of `reader_task()` that produced a duplicate broadcast. Bumped the `anvilml-worker` crate patch version from `0.1.2` to `0.1.3`.

## Resolved Dependencies

No new dependencies were added or modified in this task. No MCP lookups required.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-worker/src/managed.rs` | Remove duplicate `self.tx.send(init_msg).await` block in `spawn()` (lines 243–246); remove redundant Dead status write block in `reader_task()` (lines 636–640) |
| Modify | `crates/anvilml-worker/Cargo.toml` | Bump patch version `0.1.2 → 0.1.3` |

## Commit Log

```
 .forge/reports/P10-B1_plan.md            | 103 +++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md             |   6 +-
 .forge/state/state.json                  |  13 +++--
 Cargo.lock                               |   2 +-
 crates/anvilml-worker/Cargo.toml         |   2 +-
 crates/anvilml-worker/src/managed.rs     |  11 ----
 6 files changed, 115 insertions(+), 22 deletions(-)
```

## Test Results

```
     Running unittests src/lib.rs (target/debug/deps/anvilml_worker-36e9d79df6fddd03)

running 14 tests
test env::tests::test_build_env_cpu ... ok
test env::tests::test_build_env_cuda ... ok
test env::tests::test_build_env_mock_propagation ... ok
test env::tests::test_build_env_rocm_linux_hsa ... ok
test env::tests::test_build_env_rocm_windows_no_hsa ... ok
test managed::tests::spawn_ping_pong ... ignored, requires Python worker; set ANVILML_TEST_WORKER_PYTHON to enable
test managed::tests::status_transitions ... ignored, requires Python worker; set ANVILML_TEST_WORKER_PYTHON to enable
test managed::tests::eof_sets_dead ... ok
test pool::tests::pid_for_returns_none_for_missing_worker ... ok
test pool::tests::pool_event_listener_merges_ready_capabilities ... ok
test pool::tests::spawn_all_creates_cpu_worker_when_no_gpus ... ok
test managed::tests::respawn_after_death ... ok
test managed::tests::keepalive_pings_and_kills_on_timeout ... ok

test result: ok. 12 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 0.19s

   Doc-tests anvilml_worker

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

## Format Gate

```
(no output — exit 0, no formatting drift)
```

## Platform Cross-Check

```
# Mock-hardware Linux check:
    Checking anvilml-worker v0.1.3 (/home/dryw/AnvilML/crates/anvilml-worker)
    Checking anvilml-scheduler v0.1.0 (/home/dryw/AnvilML/crates/anvilml-scheduler)
    Checking anvilml-server v0.1.0 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking anvilml-openapi v0.1.0 (/home/dryw/AnvilML/crates/anvilml-openapi)
    Checking backend v0.1.1 (/home/dryw/AnvilML/backend)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.50s

# Mock-hardware Windows cross-check:
    Checking anvilml-worker v0.1.3 (/home/dryw/AnvilML/crates/anvilml-worker)
    Checking anvilml-scheduler v0.1.0 (/home/dryw/AnvilML/crates/anvilml-scheduler)
    Checking anvilml-server v0.1.0 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking anvilml-openapi v0.1.0 (/home/dryw/AnvilML/crates/anvilml-openapi)
    Checking backend v0.1.1 (/home/dryw/AnvilML/backend)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.67s
```

## Project Gates

```
# Gate 1 — Config Surface Sync:
     Running unittests src/main.rs (target/debug/deps/anvilml-8e4596c72d22b76d)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 8 filtered out; finished in 0.00s

     Running tests/config_reference.rs (target/debug/deps/config_reference-055b8e4db88c02b4)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.00s
```

## Deviations from Plan

None. All three plan steps were implemented exactly as specified:
- Removed the duplicate `self.tx.send(init_msg).await` block in `spawn()` (lines 243–246)
- Removed the redundant Dead status write block at the end of `reader_task()` (lines 636–640)
- Bumped crate version from `0.1.2` to `0.1.3`

## Blockers

None.
