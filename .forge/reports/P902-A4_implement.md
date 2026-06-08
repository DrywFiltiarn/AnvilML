# Implementation Report: P902-A4

| Field       | Value                                                     |
|-------------|-----------------------------------------------------------|
| Task ID     | P902-A4                                                   |
| Phase       | 902 — Stabilisation Retrofit                              |
| Description | anvilml-worker: replace serial_test env-var workaround with scoped env isolation |
| Implemented | 2026-06-08T18:05:00Z                                      |
| Status      | COMPLETE                                                  |

## Summary

Replaced the `std::env::set_var("ANVILML_WORKER_MOCK", "1")` + `#[serial_test::serial]` workaround in four spawning tests of `crates/anvilml-worker/src/managed.rs` with scoped environment-variable isolation using `temp_env::async_with_vars`. Removed `serial_test` from `[dev-dependencies]` and added `temp-env = { version = "0.3", features = ["async_closure"] }`. Bumped crate version from `0.1.12` to `0.1.13`. All 16 anvilml-worker tests pass, including the ambient env-clear gate (`env -i`).

## Resolved Dependencies

| Type   | Name       | Version resolved | Source           |
|--------|-----------|-----------------|------------------|
| crate  | temp-env  | 0.3.6           | crates.io (MCP unavailable, plan-verified) |

Note: The `rust-docs` MCP returned 404 for `temp-env`. The version `0.3` was verified via the plan's docs.rs source inspection which confirmed that `async_with_vars` is the correct function behind the `async_closure` feature flag. Cargo resolved to `0.3.6` at build time.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-worker/Cargo.toml` | Bump version `0.1.12 → 0.1.13`; replace `serial_test = "1"` with `temp-env = { version = "0.3", features = ["async_closure"] }` in `[dev-dependencies]` |
| Modify | `crates/anvilml-worker/src/managed.rs` | Refactor four test functions (`spawn_ping_pong`, `status_transitions`, `handshake_completes_once`, `spawn_reaches_idle`) to use `temp_env::async_with_vars`; remove all `#[serial_test::serial]` attributes and manual env-var set/remove calls |
| Modify | `Cargo.lock` | Updated by Cargo: added `temp-env 0.3.6`, removed `serial_test 1.0.0` and its transitive deps (`dashmap`, `hashbrown 0.14.5`, `syn 1.0.109`, `serial_test_derive 1.0.0`) |

## Commit Log

```
 Cargo.lock                                    | 121 +++------
 crates/anvilml-worker/Cargo.toml              |   4 +-
 crates/anvilml-worker/src/managed.rs          | 477 +++++++++++++-------------
 3 files changed, 308 insertions(+), 294 deletions(-)
```

## Test Results

```
     Running unittests src/lib.rs (target/debug/deps/anvilml_worker-7eaf272153f08257)

running 16 tests
test env::tests::test_build_env_cpu ... ok
test env::tests::test_build_env_cuda ... ok
test env::tests::test_build_env_rocm_linux_hsa ... ok
test env::tests::test_build_env_mock_propagation ... ok
test env::tests::test_build_env_rocm_windows_no_hsa ... ok
test managed::tests::eof_sets_dead ... ok
test pool::tests::pid_for_returns_none_for_missing_worker ... ok
test pool::tests::pid_for_returns_child_pid_when_spawned ... ok
test pool::tests::pool_event_listener_merges_ready_capabilities ... ok
test pool::tests::spawn_all_creates_cpu_worker_when_no_gpus ... ok
test managed::tests::respawn_after_death ... ok
test managed::tests::keepalive_pings_and_kills_on_timeout ... ok
test managed::tests::handshake_completes_once ... ok
test managed::tests::spawn_ping_pong ... ok
test managed::tests::spawn_reaches_idle ... ok
test managed::tests::status_transitions ... ok

test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.02s
```

Ambient env-clear gate (`env -i HOME=$HOME PATH=$PATH`):

```
running 16 tests
test env::tests::test_build_env_cpu ... ok
test env::tests::test_build_env_cuda ... ok
test env::tests::test_build_env_rocm_linux_hsa ... ok
test env::tests::test_build_env_mock_propagation ... ok
test env::tests::test_build_env_rocm_windows_no_hsa ... ok
test managed::tests::eof_sets_dead ... ok
test pool::tests::pid_for_returns_none_for_missing_worker ... ok
test pool::tests::pid_for_returns_child_pid_when_spawned ... ok
test pool::tests::pool_event_listener_merges_ready_capabilities ... ok
test pool::tests::spawn_all_creates_cpu_worker_when_no_gpus ... ok
test managed::tests::respawn_after_death ... ok
test managed::tests::keepalive_pings_and_kills_on_timeout ... ok
test managed::tests::handshake_completes_once ... ok
test managed::tests::spawn_ping_pong ... ok
test managed::tests::spawn_reaches_idle ... ok
test managed::tests::status_transitions ... ok

test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.02s
```

## Format Gate

```
(No output — exit 0)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux check:
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.80s

# 2. Mock-hardware Windows cross-check:
Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.43s

# 3. Real-hardware Linux check:
Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.72s

# 4. Real-hardware Windows cross-check:
Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.86s
```

All four platform cross-checks pass (exit 0).

## Project Gates

```
Gate 1 — Config Surface Sync:
     Running tests/config_reference.rs
running 1 test
test test_toml_key_set_matches_default ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Deviations from Plan

None. All steps implemented exactly as specified in the approved plan.

## Blockers

None.
