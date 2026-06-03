# Implementation Report: P4-A6

| Field         | Value                                           |
|---------------|-------------------------------------------------|
| Task ID       | P4-A6                                           |
| Phase         | 004 — Hardware Detection                        |
| Description   | anvilml: detect hardware at startup and serve GET /v1/system |
| Implemented   | 2026-06-03T17:15:00Z                            |
| Status        | COMPLETE                                        |

## Summary

Implemented hardware detection at server startup by wiring `anvilml_hardware::detect_all_devices()` into `backend/src/main.rs`. Added `hardware: Arc<RwLock<HardwareInfo>>` to `AppState` with a `new_with_hardware()` constructor and `hardware()` getter. Exposed hardware info via new `GET /v1/system` endpoint in `anvilml-server`. Added `--print-hardware` CLI subcommand that prints a formatted table to stdout and exits 0. Added integration test for the `/v1/system` endpoint. All tests pass (147 total), clippy is clean, Windows cross-check passes, and config drift gate passes.

## Resolved Dependencies

No new dependency versions were resolved — `anvilml-hardware` was already declared in the workspace but only reachable transitively through `anvilml-server`. The existing path dependency was used as-is:

| Type   | Name              | Version resolved | Source        |
|--------|-------------------|-----------------|---------------|
| crate  | anvilml-hardware  | path = "../crates/anvilml-hardware" | workspace |

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Edit | `backend/Cargo.toml` | Added `anvilml-hardware` dependency, forwarded `mock-hardware` feature |
| Edit | `backend/src/main.rs` | Added hardware detection at startup, device logging, `--print-hardware` handler, `new_with_hardware()` call |
| Edit | `backend/src/cli.rs` | Added `print_hardware: bool` field to `Args`, updated test constructors |
| Edit | `crates/anvilml-server/src/state.rs` | Added `hardware` field, `new_with_hardware()` constructor, `hardware()` getter, updated `Clone` impl |
| Edit | `crates/anvilml-server/src/handlers/system.rs` | Added `get_system()` handler returning `Json<HardwareInfo>` |
| Edit | `crates/anvilml-server/src/lib.rs` | Wired `GET /v1/system` route, added `system_returns_200_with_hardware_info` integration test |
| Edit | `crates/anvilml-hardware/src/lib.rs` | Formatting-only changes from `cargo fmt --all` |

## Commit Log

```
 .forge/reports/P4-A6_plan.md                 | 136 +++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md                 |   6 +-
 .forge/state/state.json                      |  13 +--
 Cargo.lock                                   |   1 +
 backend/Cargo.toml                           |   3 +-
 backend/src/cli.rs                           |   9 ++
 backend/src/main.rs                          |  95 ++++++++++++++++++-
 crates/anvilml-hardware/src/lib.rs           |  18 ++--
 crates/anvilml-server/src/handlers/system.rs |  12 ++-
 crates/anvilml-server/src/lib.rs             |  53 +++++++++++
 crates/anvilml-server/src/state.rs           |  41 +++++++-
 11 files changed, 367 insertions(+), 20 deletions(-)
```

## Test Results

### Full workspace test suite (Linux)

```
     Running unittests src/lib.rs (target/debug/deps/anvilml_core-07dea96ced852234)
running 74 tests
test result: ok. 74 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_hardware-247909921ead7d45)
running 59 tests
test result: ok. 59 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_ipc-ac6fa962a14fee4d)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/main.rs (target/debug/deps/anvilml_openapi-db7b2d8985afd9a3)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_registry-ac13e72bb2559f83)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_scheduler-d88e9af9e2c6c7db)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_server-c73f644e5e368b1e)
running 3 tests
test tests::env_returns_200_with_stub_report ... ok
test tests::health_returns_200 ... ok
test tests::system_returns_200_with_hardware_info ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_worker-f5053e0fc56a5d28)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/main.rs (target/debug/deps/anvilml-3841abccbbb429c0)
running 8 tests
test cli::tests::test_args_to_overrides_all_none ... ok
test cli::tests::test_args_to_overrides_ipv6 ... ok
test cli::tests::test_args_to_overrides_port_edge ... ok
test cli::tests::test_args_to_overrides_with_values ... ok
test cli::tests::test_log_format_default_is_plain ... ok
test cli::tests::test_log_format_possible_values ... ok
test cli::tests::test_log_format_to_string ... ok
test cli::tests::test_log_format_value_enum_variants ... ok
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/config_reference.rs (target/debug/deps/config_reference-051087b9bdf8e9fe)
running 1 test
test test_toml_key_set_matches_default ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

   Doc-tests anvilml_hardware
running 2 tests
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Total: 147 tests passed, 0 failed.

### Windows Cross-Check

```
    Checking backend v0.1.0 (/home/dryw/AnvilML/backend)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.61s
```
Zero errors.

### Config Drift Gate

```
     Running tests/config_reference.rs (target/debug/deps/config_reference-fce139f1c43ee4e4)
running 1 test
test test_toml_key_set_matches_default ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```
Zero failures.

## Deviations from Plan

None. Implementation follows the approved plan exactly.

## Blockers

None.
