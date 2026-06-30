# Plan Report: P900-A8

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P900-A8                                     |
| Phase       | 900 — Spec-Drift & Logging Retrofit         |
| Description | backend: verify ANVILML_LOG precedence over RUST_LOG (P900-A1 companion) |
| Depends on  | P900-A1                                     |
| Project     | anvilml                                     |
| Planned at  | 2026-06-30T17:40:00Z                        |
| Attempt     | 1                                           |

## Objective

Add one integration test to `backend/tests/logging_tests.rs` that verifies `ANVILML_LOG` takes precedence over `RUST_LOG` per `ENVIRONMENT.md §3.3`. The test spawns the built `anvilml` binary with both `ANVILML_LOG=debug` and `RUST_LOG=error` set simultaneously; non-empty stderr proves the debug filter was applied (since `RUST_LOG=error` alone would suppress all debug-level output). No production code changes are required — P900-A1's filter chain already implements this precedence.

## Scope

### In Scope
- Add one test function `test_anvilml_log_precedence_over_rust_log` to `backend/tests/logging_tests.rs`
- The test sets both `ANVILML_LOG=debug` and `RUST_LOG=error`, spawns the binary with `hw-probe`, and asserts `stderr` is non-empty

### Out of Scope
None. `defers_to (from JSON): []` — this task has an empty `defers_to` field and implements its full scope.

## Existing Codebase Assessment

`backend/tests/logging_tests.rs` already exists with 5 tests created by P900-A1 and P900-A5:
1. `test_anvilml_log_debug_yields_stderr` — verifies `ANVILML_LOG=debug` produces stderr
2. `test_rust_log_debug_yields_stderr` — verifies `RUST_LOG=debug` produces stderr
3. `test_log_format_json_produces_json_lines` — verifies `--log-format json` produces JSON lines
4. `test_log_format_plain_produces_text_lines` — verifies `--log-format plain` produces text
5. `test_log_format_invalid_exits_nonzero` — verifies invalid `--log-format` exits non-zero

The established patterns are:
- `Command::new(env!("CARGO_BIN_EXE_anvilml"))` + `.args(["hw-probe"])` for spawning the binary
- `#[serial]` annotation for tests that mutate env vars (process-global `std::env`)
- Capture-and-restore of prior env values with unconditional restoration in a `match` block
- `unsafe { std::env::set_var(...) }` for setting vars (required for non-static strings)
- `String::from_utf8_lossy(&output.stderr)` for debug output in assertion messages

The existing test file already uses `serial_test::serial` and `serde_json` from `[dev-dependencies]`. No new dependencies are needed.

## Resolved Dependencies

None. All required crates (`serial_test`, `serde_json`) are already declared in `backend/Cargo.toml` `[dev-dependencies]`.

| Type | Name | Version verified | MCP source | Feature flags confirmed |
|------|------|-----------------|------------|------------------------|
| (none) | | | | |

## Approach

1. **Add the precedence test to `backend/tests/logging_tests.rs`.** Append a new test function `test_anvilml_log_precedence_over_rust_log` to the existing `mod tests` block, following the exact pattern of `test_rust_log_debug_yields_stderr` (the closest sibling — it also mutates two env vars):

   a. Add `#[serial]` attribute (required because the test mutates `std::env`).
   b. Capture the prior values of both `ANVILML_LOG` and `RUST_LOG` via `std::env::var(...).ok()`.
   c. Set `ANVILML_LOG=debug` and `RUST_LOG=error` using `unsafe { std::env::set_var(...) }`.
   d. Spawn the binary: `Command::new(env!("CARGO_BIN_EXE_anvilml")).args(["hw-probe"]).output()`.
   e. Restore both prior values unconditionally (match on each captured value).
   f. Assert `!output.stderr.is_empty()` with a descriptive message that includes stdout/stderr debug info.

   The test name: `test_anvilml_log_precedence_over_rust_log`.

   The rationale for the assertion strategy: `RUST_LOG=error` alone suppresses all `debug`-level tracing output. By setting `ANVILML_LOG=debug` alongside `RUST_LOG=error`, if `RUST_LOG` were applied instead, stderr would be empty. Non-empty stderr therefore proves `ANVILML_LOG` was the active filter. This is the exact logic described in the task context and `ENVIRONMENT.md §3.3`.

   No doc comment is needed beyond the module-level doc comment already at the top of the file (the file-level doc covers all tests in the module).

2. **Verify the test compiles and runs.** The acceptance criterion `cargo test -p anvilml --test logging_tests` must exit 0 with 6 tests total (5 existing + 1 new).

## Public API Surface

None. This task only adds a test function in an integration test crate; no public API is changed or introduced.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `backend/tests/logging_tests.rs` | Add `test_anvilml_log_precedence_over_rust_log` test function |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `backend/tests/logging_tests.rs` | `test_anvilml_log_precedence_over_rust_log` | `ANVILML_LOG=debug` takes precedence over `RUST_LOG=error` — non-empty stderr proves the debug filter was applied despite RUST_LOG=error suppressing debug output | `anvilml` binary compiled; `ANVILML_LOG` and `RUST_LOG` prior values captured and restored | `.env("ANVILML_LOG","debug")`, `.env("RUST_LOG","error")`, `hw-probe` subcommand | `output.stderr` is non-empty | `cargo test -p anvilml --test logging_tests -- test_anvilml_log_precedence_over_rust_log` exits 0 |

## CI Impact

No CI changes required. The new test is a Rust integration test in `backend/tests/`, which is picked up automatically by the existing `rust-linux` and `rust-windows` CI jobs that run `cargo test --workspace --features mock-hardware`.

## Platform Considerations

None identified. The test uses only `std::process::Command` and `std::env`, which are platform-neutral. The `CARGO_BIN_EXE_anvilml` compile-time variable resolves correctly on all platforms. The Windows cross-check in `ENVIRONMENT.md §7` is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The `hw-probe` subcommand may produce no stderr even with `ANVILML_LOG=debug` if hardware detection completes silently (e.g., on a system without GPUs). This would cause the test to falsely fail, making it look like the precedence is broken when it is not. | Medium | High | The test asserts `!output.stderr.is_empty()` — if `hw-probe` produces no debug-level output at all on a particular platform, the test would fail. However, P900-A1's own `test_anvilml_log_debug_yields_stderr` already relies on `hw-probe` producing stderr, so if that test passes, this test will too. The two tests are logically equivalent in their hardware-detection dependency. |
| Env var leakage between test runs — if the `#[serial]` annotation is missing or the restore block is incomplete, subsequent tests could observe the mutated env state. | Low | Medium | The test follows the exact capture-and-restore pattern from `test_rust_log_debug_yields_stderr`, which already correctly handles two env vars. The `#[serial]` annotation serialises execution. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml --test logging_tests` exits 0 with >=3 tests (expecting 6)
- [ ] `cargo test -p anvilml --test logging_tests -- test_anvilml_log_precedence_over_rust_log` exits 0
