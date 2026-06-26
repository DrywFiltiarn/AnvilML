# Plan Report: P1-A2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P1-A2                                       |
| Phase       | 1 — Repository Scaffold                     |
| Description | backend: main.rs, cli.rs stubs, binary compiles |
| Depends on  | P1-A1                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-06-26T09:58:00Z                        |
| Attempt     | 1                                           |

## Objective

Create the `backend` crate as a buildable Rust binary package (`anvilml`) containing a minimal CLI argument parser (via `clap` derive) and a `main.rs` entry point that parses CLI args and prints a scaffold confirmation message. This establishes the binary target that all later phases will wire into the full server stack. When complete, `cargo build -p anvilml` compiles cleanly and `./target/debug/anvilml --help` displays the three expected flags (`--host`, `--port`, `--config`).

## Scope

### In Scope
- Create `backend/Cargo.toml` — package `anvilml`, inherits workspace `version`, `edition`, `rust-version`; declares `clap = { version = "4.6.1", features = ["derive"] }` as the sole dependency.
- Create `backend/src/main.rs` — `fn main()` calls `cli::parse()`, prints `"AnvilML scaffold"` to stdout, and returns exit 0.
- Create `backend/src/cli.rs` — `#[derive(Parser)] Cli` struct with three fields (`host: String`, `port: u16`, `config: Option<String>`) and a `pub fn parse() -> Cli` that returns `Cli::parse()`.
- Create `backend/tests/cli_help_test.rs` — integration test that runs the compiled binary with `--help` and verifies the output contains `--host`, `--port`, and `--config`.

### Out of Scope
None. `defers_to (from JSON): []` — this task must implement its full scope. The scaffold message, CLI struct, and compilation are all in scope. No functionality is deferred to another task.

## Existing Codebase Assessment

No prior source exists in the `backend/` directory — it has not yet been created. The workspace root `Cargo.toml` already exists (from P1-A1) with `members = ["backend"]`, `[workspace.package] version = "0.1.0"`, `edition = "2024"`, and `rust-version = "1.96.0"`. No crates directory or `Cargo.lock` exist yet. This task establishes the baseline patterns for the binary crate: workspace-inherited versioning, clap derive for declarative CLI parsing, and a minimal main.rs that will later be extended with server wiring (P1-D1) and shutdown handling (P1-A3).

## Resolved Dependencies

| Type   | Name   | Version verified | MCP source                          | Feature flags confirmed |
|--------|--------|-----------------|-------------------------------------|------------------------|
| crate  | clap   | 4.6.1           | crates.io API (rust-docs MCP unavailable; fetched via webfetch) | derive (= dep:clap_derive) |

The `rust-docs` MCP tool is configured in `opencode.json` but is not available in this session's tool set. Version resolved via `https://crates.io/api/v1/crates/clap` API, which returned `"newest_version":"4.6.1"`. The `derive` feature confirmed to exist in clap 4.6.1, mapping to `dep:clap_derive`.

## Approach

1. **Create `backend/` directory structure.** Make `backend/src/` and `backend/tests/` directories. This is necessary because `Cargo.toml` lists `members = ["backend"]` but the directory does not yet exist on disk.

2. **Write `backend/Cargo.toml`.** Create the file with:
   - `[package]` section: `name = "anvilml"`, `version.workspace = true`, `edition.workspace = true`, `rust-version.workspace = true`.
   - `[dependencies]` section: `clap = { version = "4.6.1", features = ["derive"] }`.
   - Rationale: workspace inheritance keeps version/edition in sync with the workspace root; clap 4.6.1 is the latest stable version confirmed via crates.io API.

3. **Write `backend/src/cli.rs`.** Create the file with:
   - `use clap::Parser;` import.
   - `#[derive(Parser)]` struct `Cli` with three fields:
     - `#[arg(long, default_value = "127.0.0.1")] host: String`
     - `#[arg(long, default_value = "8488")] port: u16`
     - `#[arg(long)] config: Option<String>`
   - `pub fn parse() -> Cli` that returns `Cli::parse()`.
   - Rationale: clap derive keeps the CLI declarative and short. The `default_value` attributes match the config defaults from `ANVILML_DESIGN.md §15` so the binary works correctly with defaults even before config loading is implemented.

4. **Write `backend/src/main.rs`.** Create the file with:
   - `mod cli;` declaration.
   - `fn main()` that calls `let cli = cli::parse();`, prints `"AnvilML scaffold"` to stdout via `println!`, and returns.
   - Rationale: minimal scaffold that proves the binary compiles and the CLI module is wired. Later phases (P1-A3, P1-D1) will convert this to async and wire the server.

5. **Write `backend/tests/cli_help_test.rs`.** Create an integration test that:
   - Spawns `cargo build -p anvilml` to ensure the binary exists (or relies on the acceptance command to have built it).
   - Runs `./target/debug/anvilml --help` as a subprocess with a 10-second timeout.
   - Asserts the stdout contains `--host`, `--port`, and `--config`.
   - Rationale: per FORGE_AGENT_RULES §5.1, every task writing source code must include tests. This test verifies the CLI flags are correctly registered by clap.

6. **Verify compilation.** Run `cargo build -p anvilml` and confirm exit 0.

## Public API Surface

| Item | Path | Signature |
|------|------|-----------|
| struct | `backend/src/cli.rs` | `pub struct Cli { host: String, port: u16, config: Option<String> }` (with `#[derive(Parser)]`) |
| fn | `backend/src/cli.rs` | `pub fn parse() -> Cli` |

No `pub` items in `main.rs` — it is private to the binary.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `backend/Cargo.toml` | Package manifest for `anvilml` binary crate, workspace-inherited version/edition, clap dependency |
| CREATE | `backend/src/main.rs` | Entry point: parse CLI, print scaffold message |
| CREATE | `backend/src/cli.rs` | clap derive `Cli` struct and `parse()` function |
| CREATE | `backend/tests/cli_help_test.rs` | Integration test verifying `--help` output contains all three flags |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `backend/tests/cli_help_test.rs` | `test_cli_help_shows_flags` | The `--help` output of the compiled binary contains `--host`, `--port`, and `--config` flags | Binary has been built (`cargo build -p anvilml`) | `./target/debug/anvilml --help` | Exit 0, stdout contains all three flag strings | `cargo test -p anvilml --test cli_help_test` exits 0 |

## CI Impact

No CI changes required. The `backend/` crate is already listed in the workspace `members` from P1-A1, so `cargo test --workspace` will automatically pick up the new integration test. No new CI jobs, gates, or file type handlers are introduced.

## Platform Considerations

None identified. The clap derive macros and `println!` are platform-neutral. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are required. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| clap 4.6.1 API differs from expected derive/Parser shape — e.g. `Parser` trait name changed or `#[arg(long)]` syntax differs in edition 2024 | Low | Medium | The clap 4.x derive API (Parser trait, `#[derive(Parser)]`, `#[arg(long)]`) is stable across 4.x. Verified by crates.io feature listing confirming `derive` maps to `dep:clap_derive`. The ACT agent should confirm the API compiles on first build attempt. |
| Workspace Cargo.toml `members = ["backend"]` references a directory that does not exist, causing `cargo build` to error before the crate is created | Low | Medium | The approach creates the directory structure in step 1 before writing any files. The workspace root already lists `backend` as a member from P1-A1, so no modification to root `Cargo.toml` is needed. |
| Integration test spawns a subprocess against `./target/debug/anvilml` which may not exist if the test runner builds independently | Low | Low | The test uses `cargo:rerun-if-changed`-style Cargo integration test conventions where the binary is built as a dependency of the test crate. Cargo automatically builds the package under test before running integration tests. |

## Acceptance Criteria

- [ ] `cargo build -p anvilml` exits 0
- [ ] `./target/debug/anvilml --help` output contains `--host`
- [ ] `./target/debug/anvilml --help` output contains `--port`
- [ ] `./target/debug/anvilml --help` output contains `--config`
- [ ] `cargo test -p anvilml --test cli_help_test` exits 0
