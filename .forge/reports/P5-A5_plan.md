# Plan Report: P5-A5

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P5-A5                                       |
| Phase       | 005 — Hardware Detection: Orchestration     |
| Description | backend: hw-probe CLI subcommand prints HardwareInfo JSON |
| Depends on  | P5-A4, P2-A6                                |
| Project     | anvilml                                     |
| Planned at  | 2026-06-29T12:50:00Z                        |
| Attempt     | 1                                           |

## Objective

Add a `hw-probe` CLI subcommand to the `anvilml` binary that calls `detect_all_devices()` and prints the resulting `HardwareInfo` as pretty-printed JSON to stdout, then exits 0. This gives Phase 5 a real, externally-observable Runnable Proof without prematurely building the full HTTP server's `AppState`.

## Scope

### In Scope
- Restructure `backend/src/cli.rs`: introduce a `Commands` enum with `HwProbe` variant, restructure `Cli` to hold `#[command(subcommand)] command: Option<Commands>` (default `None` preserves today's "run the server" behavior), keep existing `--host`, `--port`, `--config` CLI flags as top-level options on the `Cli` struct.
- Modify `backend/src/main.rs`: branch on `Commands::HwProbe` — when selected, load `ServerConfig` via the same `config_load::load()` path, call `anvilml_hardware::detect_all_devices(&cfg).await`, serialize to pretty JSON via `serde_json::to_string_pretty`, print to stdout, exit 0. Do not bind any socket or start the server in this branch.
- Add an integration test in `backend/tests/` that spawns the built binary with `hw-probe --help` and asserts the help text contains "hw-probe".
- Bump `backend/Cargo.toml` patch version (0.1.3 → 0.1.4).

### Out of Scope
None. `defers_to (from JSON): []` — this task implements its full scope.

## Existing Codebase Assessment

**(a) What already exists:** `backend/src/cli.rs` defines a `Cli` struct with `#[derive(Parser)]` containing three optional CLI flags (`--host`, `--port`, `--config`). `backend/src/main.rs` calls `cli::parse()`, builds `CliOverrides`, loads config via `config_load::load()`, then immediately builds the HTTP router and binds a TCP listener — no branching on subcommands. `anvilml-hardware` already exports `pub use detect::detect_all_devices;` (from P5-A4), and `HardwareInfo` derives `Serialize`/`Deserialize` via serde, so `serde_json::to_string_pretty()` will work directly. The `serde_json` crate is already available as a transitive dependency through `anvilml-core`.

**(b) Established patterns:** The codebase uses clap derive macros with `#[command(name = "...", about = "...")]` attributes. All `pub` items have `///` doc comments. The `main.rs` entry point uses `#[tokio::main]` and handles errors with `.map_err()` + `eprintln!` + `std::process::exit(1)`. Integration tests live in `backend/tests/` as separate test crate files (e.g., `cli_help_test.rs`, `config_reference.rs`, `shutdown_tests.rs`).

**(c) Gap between design doc and source:** The design doc (TASKS_PHASE005.md §P5-A5) describes restructuring `Cli` to have an `Option<Commands>` subcommand field. The current source has no subcommand support — it's a flat `Cli` struct. This is the baseline state that the task transforms. No other discrepancies exist: `detect_all_devices()` already exists and works, `HardwareInfo` is serializable, and `serde_json` is transitively available.

## Resolved Dependencies

| Type   | Name       | Version verified | MCP source     | Feature flags confirmed |
|--------|------------|-----------------|----------------|------------------------|
| crate  | clap       | 4.6.1           | Cargo.lock     | derive (already present) |
| crate  | serde_json | 1.0.150         | Cargo.lock     | n/a (transitive dep)    |

Both dependencies are already present in the project's `Cargo.lock`. No new crates are introduced. Clap 4.6.1 includes the `Subcommand` derive macro (part of `clap_derive`). `serde_json` is available as a transitive dependency through `anvilml-core`.

## Approach

1. **Restructure `cli.rs` — add `Commands` enum and optional subcommand field.**
   - Define a new `#[derive(Subcommand)]` enum `Commands` with one variant:
     ```rust
     /// Probe the system hardware and print detected devices as JSON.
     HwProbe,
     ```
   - Restructure `Cli` to replace its current flat fields with an optional subcommand:
     ```rust
     #[command(subcommand)]
     pub command: Option<Commands>,
     ```
   - Keep `--host`, `--port`, `--config` as top-level `Cli` fields (they are needed for config loading in both the server and hw-probe paths).
   - Add doc comments to the new enum and variant following the project's `///` convention.

2. **Modify `main.rs` — branch on `Commands::HwProbe`.**
   - After loading `ServerConfig` (same path as today), check `cli.command`:
     - `Some(Commands::HwProbe)` → call `anvilml_hardware::detect_all_devices(&cfg).await`, serialize to pretty JSON, print to stdout, `std::process::exit(0)`.
     - `None` → proceed with the existing server startup path (build router, bind listener, serve).
   - The `hw-probe` branch does NOT call `build_router()` or bind any socket.
   - Error handling: if `detect_all_devices()` returns `Err`, print the error to stderr and exit 1 (same pattern as config load failure today).
   - Add a `use anvilml_hardware;` import at the top of `main.rs`.

3. **Add integration test.**
   - Create `backend/tests/hw_probe_help_test.rs` that spawns the built binary with `hw-probe --help` and asserts the help text contains "hw-probe".
   - Use the same pattern as `cli_help_test.rs`: `Command::new(env!("CARGO_BIN_EXE_anvilml"))`.
   - Include a bounded timeout (10 seconds) on the subprocess to prevent hangs.

4. **Bump `backend/Cargo.toml` patch version.**
   - Change `version = "0.1.3"` → `version = "0.1.4"`.

## Public API Surface

No new `pub` items are introduced. The `Commands` enum and `Cli` struct fields are all `pub` but they are internal to the `anvilml` binary crate (not re-exported from any library crate). The only change is the restructuring of `Cli.command` from non-existent to `Option<Commands>`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `backend/src/cli.rs` | Add `Commands` enum with `HwProbe` variant; restructure `Cli` to include `Option<Commands>` subcommand field |
| Modify | `backend/src/main.rs` | Branch on `Commands::HwProbe` to call `detect_all_devices()` and print JSON; keep server path for `None` |
| CREATE   | `backend/tests/hw_probe_help_test.rs` | Integration test: `hw-probe --help` contains "hw-probe" |
| Modify | `backend/Cargo.toml` | Bump patch version 0.1.3 → 0.1.4 |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `backend/tests/hw_probe_help_test.rs` | `hw_probe_help_shows_subcommand` | The `hw-probe` subcommand appears in the binary's help output | `anvilml` binary compiled | `anvilml hw-probe --help` | stdout contains "hw-probe" | `cargo test -p anvilml --test hw_probe_help_test` exits 0 |

## CI Impact

No CI changes required. The new integration test file in `backend/tests/` is automatically picked up by `cargo test --workspace --features mock-hardware`, which is the existing CI job matrix entry for `rust-linux` and `rust-windows`. The test does not require any new CI configuration.

## Platform Considerations

None identified. The `hw-probe` subcommand path calls `detect_all_devices()` which is already cross-platform (cfg-gated Vulkan/DXGI/sysfs detectors). The JSON serialization of `HardwareInfo` is platform-neutral. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Restructuring `Cli` may break existing tests that construct `Cli` directly with field names | Low | Medium | The only existing test (`cli_help_test.rs`) uses `--help` on the binary, not direct struct construction. Verify by grepping for `Cli {` in test files. If found, update to use the new `Option<Commands>` shape. |
| `serde_json::to_string_pretty` on `HardwareInfo` may produce unexpected field ordering | Low | Low | `HardwareInfo` derives `Serialize` with serde's default serialization order (field declaration order). The output is deterministic and matches the struct field order. The Runnable Proof (P5-A6) validates structure, not field order. |
| `detect_all_devices()` panics in some edge case (e.g., missing env vars for hostname) | Low | Medium | `detect_all_devices()` already handles missing `HOSTNAME`/`COMPUTERNAME` gracefully with `.unwrap_or_else(|_| "unknown".into())`. No additional error handling needed. |

## Acceptance Criteria

- [ ] `cargo build -p anvilml` exits 0
- [ ] `cargo test --workspace --features mock-hardware` exits 0
- [ ] `cargo test -p anvilml --test hw_probe_help_test` exits 0
- [ ] `grep "hw-probe" target/debug/anvilml --help` matches (subcommand appears in help text)
- [ ] `wc -l backend/Cargo.toml | awk '{print $1}'` — version line shows `0.1.4`
