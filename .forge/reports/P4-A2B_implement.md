# Implementation Report: P4-A2B

| Field          | Value                                      |
|----------------|---------------------------------------------|
| Task ID        | P4-A2B                                      |
| Phase          | 004 — Persistence & Model Registry          |
| Description    | anvilml — naming correction (binary `anvilml`, database `anvilml.db`) |
| Project        | anvilml                                     |
| Implemented at | 2026-05-30T16:30:00Z                        |
| Attempt        | 1                                           |

## Summary

Applied the naming corrections from the `ANVILML_DESIGN.md` Rev 3 amendment across four files in the anvilml codebase. Renamed the launcher binary from `sindristudio` to `anvilml` in `backend/Cargo.toml`, updated the default database path from `./sindristudio.db` to `./anvilml.db` in `crates/anvilml-core/src/config.rs` (both the `default_db_path()` function and its test assertion), updated the doc comment in `backend/src/main.rs`, and added `db_path = "./anvilml.db"` to the default `anvilml.toml` configuration file. All changes are strictly within the plan's "In Scope" section.

## Files Changed

| Action   | Path                              | Description                                          |
|----------|-----------------------------------|------------------------------------------------------|
| MODIFY   | backend/Cargo.toml                | Binary name `sindristudio` → `anvilml`, description updated |
| MODIFY   | crates/anvilml-core/src/config.rs | Default DB path `./sindristudio.db` → `./anvilml.db` in both `default_db_path()` and test assertion |
| MODIFY   | backend/src/main.rs               | Doc comment `(sindristudio)` → `(anvilml)`           |
| MODIFY   | anvilml.toml                      | Added `db_path = "./anvilml.db"` config field        |

## Test Results

Full workspace test suite run with `--features mock-hardware`. All 144 tests passed, 0 failures.

```
   Compiling backend v0.1.0 (/home/dryw/AnvilML/backend)
   Compiling anvilml-core v0.1.0 (/home/dryw/AnvilML/crates/anvilml-core)
   Compiling anvilml-registry v0.1.0 (/home/dryw/AnvilML/crates/anvilml-registry)
   Compiling anvilml-hardware v0.1.0 (/home/dryw/AnvilML/crates/anvilml-hardware)
   Compiling anvilml-ipc v0.1.0 (/home/dryw/AnvilML/crates/anvilml-ipc)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1.56s
     Running unittests src/lib.rs (target/debug/deps/anvilml_core-ce00214cfdaf1e0a)
running 52 tests
test config::tests::config_default_deserialize ... ok
test config::tests::config_round_trip ... ok
test config::tests::config_frontend_modes ... ok
... (49 more tests) ...
test result: ok. 52 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_hardware-f66c1af789cc861b)
running 43 tests
test result: ok. 43 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_ipc-e6f720e5087a79c2)
running 18 tests
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/main.rs (target/debug/deps/anvilml_openapi-7d4156d460ab7961)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_registry-55337e11a22654ed)
running 28 tests
test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_scheduler-c5ab94446f3e4109)
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_server-2e16d22d2c454f2d)
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_worker-eaa1d41160c21ba5)
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/main.rs (target/debug/deps/anvilml-dd85450561847d88)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

   Doc-tests anvilml_core, anvilml_hardware, anvilml_ipc, anvilml_registry,
anvilml_scheduler, anvilml_server, anvilml_worker
   test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## CI Changes

No CI changes made.

## Commit Log

```
M  anvilml.toml
M  backend/Cargo.toml
M  backend/src/main.rs
M  crates/anvilml-core/src/config.rs
A  .forge/reports/P4-A2B_implement.md
M  .forge/state/CURRENT_TASK.md
M  .forge/state/state.json
```

## Acceptance Criteria — Verification

| Criterion                              | Status | Evidence                                       |
|----------------------------------------|--------|------------------------------------------------|
| `backend/Cargo.toml` binary name is `anvilml` | PASS   | `grep 'name = "anvilml"' backend/Cargo.toml`  |
| `backend/Cargo.toml` description updated to `(anvilml)` | PASS   | `grep 'description' backend/Cargo.toml`        |
| `default_db_path()` returns `./anvilml.db` | PASS   | `grep 'anvilml.db' crates/anvilml-core/src/config.rs` |
| Test assertion in `config_default_deserialize` checks `./anvilml.db` | PASS   | `grep 'anvilml.db' crates/anvilml-core/src/config.rs` (line 409) |
| `backend/src/main.rs` doc comment updated to `(anvilml)` | PASS   | `head -1 backend/src/main.rs`                  |
| `anvilml.toml` contains `db_path = "./anvilml.db"` | PASS   | `grep 'db_path' anvilml.toml`                  |
| `cargo fmt --all` passes              | PASS   | Exit code 0                                    |
| `cargo clippy --workspace --features mock-hardware -- -D warnings` passes | PASS   | Exit code 0, zero warnings                     |
| All workspace tests pass (144 tests)  | PASS   | `cargo test --workspace --features mock-hardware` exit code 0 |
