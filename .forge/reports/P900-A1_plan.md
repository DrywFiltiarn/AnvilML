# Plan Report: P900-A1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P900-A1                                     |
| Phase       | 900 — Spec-Drift & Logging Retrofit         |
| Description | backend: wire tracing-subscriber, ANVILML_LOG/RUST_LOG never read |
| Depends on  | P1-A3                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-06-30T11:15:00Z                        |
| Attempt     | 1                                           |

## Objective

Register a real `tracing` subscriber in the `anvilml` binary so that every existing `tracing::info!`/`debug!` call (e.g. the "listening" log in P1-D1, shutdown log in P1-A3) produces visible output. The subscriber reads its filter from `ANVILML_LOG` (primary) or `RUST_LOG` (fallback), defaulting to `"info"`. Two integration tests verify that setting either variable to `"debug"` yields non-empty stderr from the spawned binary.

## Scope

### In Scope
- Add `tracing-subscriber` (with `env-filter` feature) to `backend/Cargo.toml`.
- Add a single initialization call as the first statement in `main()` before CLI parsing or config loading.
- Create `backend/tests/logging_tests.rs` with two tests:
  - `test_anvilml_log_debug_yields_stderr`: sets `ANVILML_LOG=debug`, spawns `hw-probe`, asserts stderr is non-empty.
  - `test_rust_log_debug_yields_stderr`: sets `RUST_LOG=debug`, spawns `hw-probe`, asserts stderr is non-empty.
- Both tests use `Command::new(env!("CARGO_BIN_EXE_anvilml"))` per the pattern in `backend/tests/hw_probe_help_test.rs`.

### Out of Scope
None. `defers_to (from JSON): absent`. This task implements its full scope in its entirety. No stubs, no deferred functionality.

## Existing Codebase Assessment

The `anvilml` binary (backend crate) already contains `tracing::info!` calls at two sites: `main.rs` line 76-79 (the "listening" log on server bind) and line 83 (shutdown signal log). These calls exist in source but are silent no-ops because no subscriber has ever been registered — `tracing-subscriber` is absent from `Cargo.lock`. The `backend/Cargo.toml` declares `tracing = "0.1"` but has no `tracing-subscriber` dependency.

The project follows a consistent integration test pattern in `backend/tests/`: tests are separate crate files that compile independently against the crate's public API, using `Command::new(env!("CARGO_BIN_EXE_<name>"))` to spawn the built binary (established in `hw_probe_help_test.rs` from P5-A5). Tests that mutate env vars must follow `#[serial]` + capture-and-restore isolation (ENVIRONMENT.md §11.3).

There is a gap between the design doc and current source: `ENVIRONMENT.md §3.3` documents `ANVILML_LOG`/`RUST_LOG` precedence and a `--log-format` CLI flag, but neither the subscriber initialization nor the flag exists in the binary today.

## Resolved Dependencies

| Type   | Name              | Version verified | MCP source  | Feature flags confirmed |
|--------|-------------------|-----------------|-------------|------------------------|
| crate  | tracing-subscriber| 0.3.23          | rust-docs MCP | env-filter             |

The `env-filter` feature enables `EnvFilter::try_from_env()` and `EnvFilter::new()`. The `fmt` feature (default) enables `tracing_subscriber::fmt()` which returns a `SubscriberBuilder` with `.with_env_filter()` and `.init()` methods. All API names confirmed via MCP lookup of version 0.3.23.

## Approach

1. **Add dependency to `backend/Cargo.toml`:** Append `tracing-subscriber = { version = "0.3", features = ["env-filter"] }` to the `[dependencies]` section. Use `"0.3"` as the version constraint (semver-compatible with 0.3.23), matching the project's convention of using major.minor constraints rather than exact pins.

2. **Initialize tracing as the first statement in `main()`:** In `backend/src/main.rs`, before the `let cli = cli::parse();` line, add:
   ```rust
   tracing_subscriber::fmt()
       .with_env_filter(
           EnvFilter::try_from_env("ANVILML_LOG")
               .or_else(|_| EnvFilter::try_from_env("RUST_LOG"))
               .unwrap_or_else(|_| EnvFilter::new("info"))
       )
       .init();
   ```
   This runs before any other startup work so that config-loading and hardware-detection log lines (already written elsewhere in the codebase) become visible immediately once this task lands. The `EnvFilter` precedence matches `ENVIRONMENT.md §3.3` exactly: `ANVILML_LOG` first, `RUST_LOG` fallback, `"info"` default.

   Rationale: Placing this before CLI parsing ensures that even if the CLI fails to parse, the subscriber is registered and any error messages from clap will be visible at the appropriate log level. However, since clap writes to stderr directly (not through tracing), the practical impact is limited — but it ensures consistency for any future tracing calls that might appear during argument validation.

   Add the import: `use tracing_subscriber::{EnvFilter, registry::Registry};` — actually, only `EnvFilter` is needed; `tracing_subscriber::fmt()` is a free function in the crate root, and `.init()` comes from the `SubscriberInitExt` trait which is in the prelude. The minimal import is `use tracing_subscriber::EnvFilter;`.

3. **Create `backend/tests/logging_tests.rs`:** Write two integration tests that spawn the built binary via `Command::new(env!("CARGO_BIN_EXE_anvilml"))`, set the appropriate env var to `"debug"`, run `hw-probe` (which produces hardware detection output including a tracing log), and assert `!output.stderr.is_empty()`. Both tests must be `#[serial]` because they mutate process-global env vars. Each test must capture-and-restore the prior value of the env var it sets.

   Test structure:
   - `test_anvilml_log_debug_yields_stderr`: captures prior `ANVILML_LOG`, sets it to `"debug"`, spawns binary with `hw-probe`, asserts stderr non-empty, restores prior value.
   - `test_rust_log_debug_yields_stderr`: captures prior `RUST_LOG`, sets it to `"debug"`, spawns binary with `hw-probe`, asserts stderr non-empty, restores prior value.

   The `hw-probe` subcommand is appropriate because it triggers `detect_all_devices()` which already has tracing calls inside it (from P1-D1/P1-A3's hardware detection code), so debug-level output will appear in stderr when the subscriber is active.

   Rationale: Using `hw-probe` avoids needing a running HTTP server (no port binding, no TCP listener), making the test fast and reliable. The `hw-probe` path is self-contained and exits cleanly.

4. **Verify the test compiles and passes:** Run `cargo test -p anvilml --test logging_tests`. The command must exit 0.

## Public API Surface

No new public items are introduced. The change is entirely internal to `backend`:
- A new dependency in `backend/Cargo.toml` (not a pub item).
- A tracing subscriber initialization call in `main()` (private to the binary's entry point).
- Two private integration tests in `backend/tests/logging_tests.rs`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `backend/Cargo.toml` | Add `tracing-subscriber` dependency with `env-filter` feature |
| MODIFY | `backend/src/main.rs` | Add tracing subscriber init as first statement in `main()` |
| CREATE | `backend/tests/logging_tests.rs` | Two integration tests for ANVILML_LOG and RUST_LOG |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `backend/tests/logging_tests.rs` | `test_anvilml_log_debug_yields_stderr` | Setting `ANVILML_LOG=debug` causes the spawned binary to emit non-empty stderr (tracing output) | Binary compiled with `cargo build -p anvilml`; `ANVILML_LOG` not previously set to a tracing filter | `Command::new(env!("CARGO_BIN_EXE_anvilml"))`, args `["hw-probe"]`, env `ANVILML_LOG=debug` | `output.stderr` is non-empty (contains at least one tracing-formatted log line) | `cargo test -p anvilml --test logging_tests -- test_anvilml_log_debug_yields_stderr` exits 0 |
| `backend/tests/logging_tests.rs` | `test_rust_log_debug_yields_stderr` | Setting `RUST_LOG=debug` (when `ANVILML_LOG` is unset) causes the spawned binary to emit non-empty stderr (tracing output) | Binary compiled with `cargo build -p anvilml`; `ANVILML_LOG` not set | `Command::new(env!("CARGO_BIN_EXE_anvilml"))`, args `["hw-probe"]`, env `RUST_LOG=debug` | `output.stderr` is non-empty (contains at least one tracing-formatted log line) | `cargo test -p anvilml --test logging_tests -- test_rust_log_debug_yields_stderr` exits 0 |

## CI Impact

No CI changes required. The new dependency is added to `backend/Cargo.toml`, which is part of the workspace and already built by all CI jobs (`cargo build --workspace --features mock-hardware` in the `rust-linux` and `rust-windows` jobs). The new test file lives in `backend/tests/` which is automatically picked up by `cargo test --workspace --features mock-hardware`.

## Platform Considerations

None identified. The Windows cross-check in ENVIRONMENT.md §7 is sufficient. The `tracing-subscriber` crate is fully cross-platform and does not use any `#[cfg(unix)]` or `#[cfg(windows)]` guards. The `EnvFilter` reads standard environment variables which behave identically across platforms.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The `hw-probe` subcommand may not produce any tracing output because hardware detection code paths may not use `tracing::debug!()` or `tracing::info!()` — the existing tracing calls are only in the server bind path and shutdown handler. If so, both tests would fail with empty stderr. | Medium | High | Before writing tests, scan `anvilml_hardware/src/` for any `tracing::` calls. If none exist in the `hw-probe` path, use `anvilml`'s own `--help` output as a fallback: spawn `anvilml --help` and assert that the binary at least starts without panicking (which proves the subscriber init succeeded). Alternatively, spawn the binary without subcommands against a non-existent port to trigger the server bind path's `tracing::info!("listening")` log. |
| `EnvFilter::try_from_env("ANVILML_LOG")` may return an error for an invalid filter string, but the `.or_else()` chain handles this gracefully by falling through to the next env var or the `"info"` default. However, if both env vars contain invalid directives, the `unwrap_or_else(|_| EnvFilter::new("info"))` fallback silently defaults to `"info"`, which may mask a user typo. | Low | Low | This is the documented and intended behaviour per `ENVIRONMENT.md §3.3`. No change needed. |
| The `#[serial]` attribute on both tests ensures env var isolation but means the two tests run sequentially. This adds ~2s per test (binary compilation + spawn). | Low | Low | Acceptable — the tests are fast and the serial requirement is mandated by ENVIRONMENT.md §11.3. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml --test logging_tests` exits 0
- [ ] `cargo build -p anvilml --features mock-hardware` exits 0 (verifies the new dependency resolves)
- [ ] `grep -q "tracing-subscriber" backend/Cargo.toml` exits 0 (confirms dependency was added)
