# Implementation Report: P9-A1

| Field         | Value                              |
|---------------|------------------------------------|
| Task ID       | P9-A1                              |
| Phase         | 009 — Worker Spawn & Handshake     |
| Description   | anvilml-worker: env.rs WorkerEnv env builder |
| Implemented   | 2026-06-16T17:30:00Z               |
| Status        | COMPLETE                           |

## Summary

Implemented `build_worker_env` in `crates/anvilml-worker/src/env.rs` — a pure data transformation function that constructs a `HashMap<String, String>` of `ANVILML_*` environment variables for Python worker subprocesses. Added `log_level: String` (default `"info"`) to `ServerConfig` in `anvilml-core`. Updated `lib.rs` to declare the `env` module and re-export `build_worker_env`. Created 10 integration tests in `tests/env_tests.rs` verifying each env var. Updated `anvilml.toml` and `docs/ENVIRONMENT.md §4` for Gate 1 config surface sync. All 10 new tests pass, full workspace test suite exits 0, format and lint clean.

## Resolved Dependencies

None. This task uses only `std::collections::HashMap` (standard library) and types already available through `anvilml-core`. No new external crates are introduced.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | crates/anvilml-core/src/config.rs | Added `log_level: String` field with `default_log_level()` helper and `Default` impl update |
| CREATE | crates/anvilml-worker/src/env.rs | New module with `build_worker_env` function and `device_type_label` helper |
| MODIFY | crates/anvilml-worker/src/lib.rs | Replaced stub with `pub mod env;` and `pub use env::build_worker_env;` |
| CREATE | crates/anvilml-worker/tests/env_tests.rs | 10 integration tests for `build_worker_env` |
| MODIFY | crates/anvilml-worker/Cargo.toml | Bumped patch version `0.1.0` → `0.1.1` |
| MODIFY | crates/anvilml-core/tests/config_tests.rs | Added `log_level` field to struct construction (pre-existing test fix) |
| MODIFY | anvilml.toml | Added `log_level = "info"` for Gate 1 config surface sync |
| MODIFY | docs/ENVIRONMENT.md | Added `log_level` field documentation in §4 config reference |
| MODIFY | docs/TESTS.md | Added 10 test entries for `env_tests.rs` |

## Commit Log

```
 .forge/reports/P9-A1_plan.md              | 142 +++++++++++++++++++++
 .forge/state/CURRENT_TASK.md              |   6 +-
 .forge/state/state.json                   |  13 +-
 Cargo.lock                                |   2 +-
 anvilml.toml                              |   1 +
 crates/anvilml-core/src/config.rs         |  11 ++
 crates/anvilml-core/tests/config_tests.rs |   1 +
 crates/anvilml-worker/Cargo.toml          |   2 +-
 crates/anvilml-worker/src/env.rs          |  85 +++++++++++++
 crates/anvilml-worker/src/lib.rs          |   4 +-
 crates/anvilml-worker/tests/env_tests.rs  | 199 ++++++++++++++++++++++++++++++
 docs/ENVIRONMENT.md                       |   1 +
 docs/TESTS.md                             |  90 ++++++++++++++
 13 files changed, 544 insertions(+), 13 deletions(-)
```

## Test Results

```
     Running tests/env_tests.rs (target/debug/deps/env_tests-0fb53f5656d5f06c)

running 10 tests
test test_device_index ... ok
test test_device_type_cpu ... ok
test test_device_type_cuda ... ok
test test_device_type_rocm ... ok
test test_ipc_port ... ok
test test_log_level ... ok
test test_max_ipc_payload_mib ... ok
test test_mock_hardware_flag ... ok
test test_total_count ... ok
test test_worker_id ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Full workspace: all crates passed. 0 failures across all 100+ tests.

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
Checking anvilml-worker v0.1.1 (/home/dryw/AnvilML/crates/anvilml-worker)
...
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.73s
--- CHECK 1 OK ---

# 2. Mock-hardware Windows
Checking anvilml-core v0.1.13 (/home/dryw/AnvilML/crates/anvilml-core)
...
Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.42s
--- CHECK 2 OK ---

# 3. Real-hardware Linux
Checking anvilml-hardware v0.1.9 (/home/dryw/AnvilML/crates/anvilml-hardware)
...
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.66s
--- CHECK 3 OK ---

# 4. Real-hardware Windows
Checking anvilml-hardware v0.1.9 (/home/dryw/AnvilML/crates/anvilml-hardware)
...
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.03s
--- CHECK 4 OK ---
```

## Project Gates

```
Gate 1 — Config Surface Sync:
cargo test -p anvilml --features mock-hardware -- config_reference
running 1 test
test config_reference ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Public API Delta

```
+    pub log_level: String,
+pub fn build_worker_env(
+pub mod env;
+pub use env::build_worker_env;
```

Items:
- `pub log_level: String` — new field on `anvilml_core::config::ServerConfig` (fn/struct field)
- `pub fn build_worker_env` — new public function in `anvilml_worker::env` (fn)
- `pub mod env` — new module declaration in `anvilml_worker` (mod)
- `pub use env::build_worker_env` — re-export in `anvilml_worker` (use)

All match the plan's Public API Surface table.

## Deviations from Plan

- **API fix**: `device_type_label` returns `&'static str` but the first `HashMap::insert` call (`port.to_string()`) forces the value type to `String`. Applied `.to_string()` to the `device_type_label` result to satisfy the type inference. This was discovered during compile check and fixed before proceeding.
- **Gate 1 config sync**: The plan's risk table anticipated that `config_reference` would fail because `log_level` is a new field not in `anvilml.toml`. Added `log_level = "info"` to `anvilml.toml` and updated `docs/ENVIRONMENT.md §4` with the new field documentation — this is a Gate 1 (Config Surface Sync) requirement that was executed as part of this task.
- **Pre-existing test fix**: `crates/anvilml-core/tests/config_tests.rs` had a struct construction of `ServerConfig` that was missing the new `log_level` field. Added `log_level: "debug".to_string()` to keep the test's non-default-value pattern.

## Blockers

None.
