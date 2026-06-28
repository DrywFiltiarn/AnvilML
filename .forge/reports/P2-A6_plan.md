# Plan Report: P2-A6

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P2-A6                                       |
| Phase       | 002 — Core Domain Types: Config & Errors    |
| Description | backend: wire config_load::load() into main.rs|
| Depends on  | P2-A5                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-06-28T14:00:00Z                        |
| Attempt     | 1                                           |

## Objective

Replace `backend/src/main.rs`'s Phase 1 CLI-only host/port binding with the complete
four-layer config loading pipeline from `anvilml_core::config_load::load()`, so the
running binary uses `ServerConfig.host` and `ServerConfig.port` (loaded through defaults
→ TOML → env vars → CLI overrides) instead of raw `cli.host`/`cli.port` values. If
config loading fails, print the error and exit 1 before binding any socket. This makes
`ServerConfig` the actual source of truth for the server's bind address and port.

## Scope

### In Scope
- `backend/src/cli.rs`: Change `Cli.host` from `String` (default `"127.0.0.1"`) to
  `Option<String>` (no default). Change `Cli.port` from `u16` (default `8488`) to
  `Option<u16>` (no default). Update clap doc comments accordingly.
- `backend/src/main.rs`: Add `use anvilml_core::config_load;` and `use anvilml_core::CliOverrides;`.
  Replace `cli.host`/`cli.port` usage with a call to `config_load::load()` passing
  `cli.config.as_deref().map(Path::new)` and `Some(CliOverrides { host: cli.host, port: cli.port })`
  (only `Some` because the fields are now `Option`). On `Err`, print the error via
  `eprintln!` and call `std::process::exit(1)` before any socket bind. Use
  `config.host`/`config.port` for the `TcpListener::bind()` call.
- `backend/Cargo.toml`: Add `anvilml-core = { path = "../crates/anvilml-core" }` to
  dependencies. Bump patch version `0.1.1 → 0.1.2`.

### Out of Scope
defers_to (from JSON): []
- No changes to `anvilml-core` itself (config types, `load()`, `CliOverrides` are
  complete from P2-A1 through P2-A5).
- No changes to `anvilml-server`'s `build_router()` (still returns a router without
  config; config wiring into `AppState` is a later phase).
- No changes to `anvilml-scheduler` or any other crate.
- No new tests beyond verifying existing tests still pass.

## Existing Codebase Assessment

The codebase at this point has `anvilml-core` fully implemented with `ServerConfig`
(11 fields including nested tables), `AnvilError`, and `config_load::load()` implementing
the complete four-layer precedence chain (defaults → TOML → env vars → CLI overrides).
The `CliOverrides` struct already exists in `anvilml-core` with `host: Option<String>`
and `port: Option<u16>`.

`backend/src/main.rs` currently uses `cli.host` and `cli.port` directly from the clap
`Cli` struct, which has hardcoded defaults (`"127.0.0.1"` and `8488`). The server
crate's `build_router()` returns a simple `axum::Router` with only `/health` — it does
not yet accept `ServerConfig` or any `AppState`.

The established pattern is: `lib.rs` contains only re-exports (≤ 80 lines), doc comments
on all `pub` items, and `///` style. Error handling uses `?` for propagation and
`unwrap()` only in tests. The existing `shutdown_tests.rs` and `health_tests.rs` tests
exercise the shutdown signal handler and the health endpoint respectively — neither
touches main.rs's config loading path, so they are unaffected by this wiring change.

No gap exists between the design doc and current source that affects this task:
`config_load::load()`'s signature matches exactly what the task context describes, and
`CliOverrides` is already exported from `anvilml-core`.

## Resolved Dependencies

None. This task does not introduce any new external crates. It uses `anvilml-core`
which is already a workspace member (though not yet a direct dependency of `backend`).

| Type   | Name        | Version verified | MCP source | Feature flags confirmed |
|--------|-------------|-----------------|------------|------------------------|
| crate  | anvilml-core| 0.1.5 (local)   | N/A        | n/a                    |

## Approach

### Step 1: Add `anvilml-core` as a dependency of `backend`

Edit `backend/Cargo.toml`: add `anvilml-core = { path = "../crates/anvilml-core" }`
to the `[dependencies]` section. This is safe because `backend` already depends on
`anvilml-server` and `anvilml-scheduler`, both of which transitively depend on
`anvilml-core` — adding it as a direct dependency does not create any cycles.

Also bump the `[package] version` from `0.1.1` to `0.1.2` (patch increment per
`ENVIRONMENT.md §12`).

### Step 2: Change `cli.rs` host and port fields to `Option`

Edit `backend/src/cli.rs`:
- Change `pub host: String` (with `#[arg(long, default_value = "127.0.0.1")]`) to
  `pub host: Option<String>` (remove the `default_value` attribute).
- Change `pub port: u16` (with `#[arg(long, default_value = "8488")]`) to
  `pub port: Option<u16>` (remove the `default_value` attribute).
- Update the doc comment on the struct to note that host/port defaults come from
  `ServerConfig::default()` via `config_load::load()`, not from clap defaults.

This is the breaking change: any code reading `cli.host` or `cli.port` now gets
`Option` values. Since no code outside this task references these fields yet (Phase 1
was the only phase that touched `main.rs`), this is a clean break.

### Step 3: Wire `config_load::load()` into `main.rs`

Edit `backend/src/main.rs`:

1. Add imports:
   ```rust
   use anvilml_core::config_load;
   use anvilml_core::CliOverrides;
   use std::path::Path;
   ```

2. After `let cli = cli::parse();`, call `load()`:
   ```rust
   let cli_overrides = CliOverrides {
       host: cli.host,
       port: cli.port,
   };
   let config = config_load::load(cli.config.as_deref().map(Path::new), Some(cli_overrides))
       .map_err(|e| {
           eprintln!("Failed to load config: {e}");
           std::process::exit(1);
       })?;
   ```

   This passes `Some(cli_overrides)` because the task context specifies that
   `cli.host`/`cli.port` are now `Option` — `CliOverrides` takes `Option<String>`
   and `Option<u16>`, so even if both are `None`, wrapping in `Some(CliOverrides { .. })`
   is correct: `apply_cli_overrides` in `config_load.rs` checks `if let Some(host) = overrides.host`
   and `if let Some(port) = overrides.port`, so `None` fields are silently skipped
   (the env var / TOML / default value wins).

3. Replace the `TcpListener::bind` call:
   ```rust
   let listener = TcpListener::bind(format!("{}:{}", config.host, config.port))
       .await
       .unwrap();
   ```

4. Update the `tracing::info!` line to use `config.host`/`config.port`:
   ```rust
   tracing::info!(addr = %format!("{}:{}", config.host, config.port), "listening");
   ```

No `?` operator is used on the `load()` call — instead, `.map_err()` converts the
error to an exit(1) with a printed message. This satisfies the requirement to exit
before binding any socket. The `?` on the `load()` call's result is actually not
valid here because `main()` returns `()` not `Result`. The correct pattern is the
`.map_err()` + `std::process::exit(1)` approach shown above, which avoids needing
`main()` to return `Result`.

### Step 4: Verify build and tests

Run `cargo build -p anvilml` to confirm compilation. Run `cargo test --workspace
--features mock-hardware` to confirm existing tests still pass.

## Public API Surface

No new public items are introduced. This task only modifies private fields of the
`Cli` struct (which is not re-exported outside `backend`) and changes the call site
in `main.rs`. The only public API consumed is:
- `anvilml_core::config_load::load(toml_path: Option<&Path>, cli_overrides: Option<CliOverrides>) -> Result<ServerConfig, AnvilError>`
- `anvilml_core::CliOverrides { host: Option<String>, port: Option<u16> }`

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `backend/Cargo.toml` | Add `anvilml-core` dependency; bump version 0.1.1 → 0.1.2 |
| Modify | `backend/src/cli.rs` | Change `host`/`port` fields from `String`/`u16` with defaults to `Option<String>`/`Option<u16>` without defaults |
| Modify | `backend/src/main.rs` | Wire `config_load::load()`, use `ServerConfig` for bind |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|---------------------|
| `backend/tests/shutdown_tests.rs` | `test_shutdown_signal_returns_on_ctrl_c` | Shutdown signal handler still works (no config loading code path involved) | `cargo test -p anvilml --test shutdown_tests` exits 0 |
| `backend/tests/shutdown_tests.rs` | `test_shutdown_signal_timeout_cancels` | Timeout cancellation still works | `cargo test -p anvilml --test shutdown_tests` exits 0 |
| `backend/tests/cli_help_test.rs` | `cli_help_shows_all_flags` | `--help` still lists `--host`, `--port`, `--config` (clap still generates these flags even with `Option` type) | `cargo test -p anvilml --test cli_help_test` exits 0 |
| `crates/anvilml-server/tests/health_tests.rs` | `test_health_returns_200` | Health endpoint still returns 200 (router unchanged) | `cargo test -p anvilml-server --test health_tests` exits 0 |
| Workspace | All workspace tests | No regressions from the wiring change | `cargo test --workspace --features mock-hardware` exits 0 |

## CI Impact

No CI changes required. The task modifies only `backend/` source files and adds a
transitive dependency that already exists in the workspace. The existing CI jobs
(`rust-linux`, `rust-windows`) run `cargo test --workspace --features mock-hardware`
which will pick up the new dependency automatically. The `config-drift` job runs
`cargo test -p anvilml --features mock-hardware -- config_reference` which is
introduced in P2-A7 (not this task).

## Platform Considerations

None identified. The Windows cross-check in `ENVIRONMENT.md §7` is sufficient. The
change is purely about which fields are read from `ServerConfig` vs `Cli` — no
platform-specific code paths are affected. The `TcpListener::bind` call works
identically on Linux and Windows.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `cli.host`/`cli.port` are `Option` — `CliOverrides` takes `Option<String>`/`Option<u16>`, so passing `cli.host`/`cli.port` directly works, but if `apply_cli_overrides` is called with `Some(CliOverrides { host: None, port: None })`, it skips overrides correctly. The risk is that a future refactor accidentally passes `None` for `cli_overrides` entirely, which would mean CLI flags are never applied. | Low | Medium | The code is straightforward: `Some(CliOverrides { host: cli.host, port: cli.port })` is always passed. No conditional `Some`/`None` wrapping. The `apply_cli_overrides` function already handles `None` fields correctly (it checks `if let Some(host) = overrides.host`). |
| The `anvilml-core` dependency is not yet listed in `backend/Cargo.toml`. Adding it could introduce a transitive dependency conflict if `backend`'s existing deps pin incompatible versions of shared crates. | Low | Medium | `backend` already depends on `anvilml-server` and `anvilml-scheduler`, both of which depend on `anvilml-core`. Adding it as a direct dependency cannot introduce new transitive deps — they are already present. Cargo's lockfile will resolve version conflicts. |
| `main.rs` uses `.map_err()` + `exit(1)` instead of `?` because `main()` returns `()`. If the error message from `AnvilError` is not user-friendly (e.g. includes internal debug info), it could confuse operators. | Low | Low | `AnvilError`'s `Display` impl uses `thiserror` which produces clean error messages like "I/O error: ..." or "serialization error: ...". These are operator-friendly. No change needed. |
| The `cli_help_test.rs` test expects `--host` and `--port` to appear in help output. Clap still generates these flags even when the field type is `Option<String>`/`Option<u16>` — the flags are just now optional (no default shown). | Very Low | Low | Verified: clap generates `--host [HOST]` and `--port [PORT]` for `Option` fields. The help output still contains the flag names. |

## Acceptance Criteria

- [ ] `cargo build -p anvilml` exits 0
- [ ] `cargo test -p anvilml --test shutdown_tests` exits 0
- [ ] `cargo test -p anvilml --test cli_help_test` exits 0
- [ ] `cargo test -p anvilml-server --test health_tests` exits 0
- [ ] `cargo test --workspace --features mock-hardware` exits 0
