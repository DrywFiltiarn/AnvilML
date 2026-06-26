# Implementation Report: P1-A2

| Field         | Value                              |
|---------------|------------------------------------|
| Task ID       | P1-A2                              |
| Phase         | 001 — Phase 1: Project Scaffold    |
| Description   | backend: main.rs, cli.rs stubs, binary compiles |
| Implemented   | 2026-06-26T10:30:00Z               |
| Status        | COMPLETE                           |

## Summary

Created the `backend/` crate with `main.rs` and `cli.rs` source files, plus an integration
test (`cli_help_test.rs`) verifying that `--help` output contains all three CLI flags
(`--host`, `--port`, `--config`). Added a no-op `mock-hardware` feature stub to
`backend/Cargo.toml` so the workspace-level `--features mock-hardware` flag does not
error on this member. Fixed the workspace `Cargo.toml` by adding `resolver = "3"`
required for edition 2024. The binary compiles, passes all checks, and `--help`
produces correct output.

## Resolved Dependencies

| Type   | Name   | Version resolved | Source         |
|--------|--------|------------------|----------------|
| crate  | clap   | 4.6.1            | Plan (MCP unavailable) |

Note: The `rust-docs` MCP server was not available as a direct CLI tool. The version
4.6.1 from the approved plan was used. This is a reasonable recent stable version of
clap with the `derive` feature.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | backend/Cargo.toml | Package manifest with clap 4.6.1 dependency and no-op mock-hardware feature |
| CREATE | backend/src/cli.rs | Cli struct with clap derive, parse() function |
| CREATE | backend/src/main.rs | Entry point: parses CLI, prints scaffold message |
| CREATE | backend/tests/cli_help_test.rs | Integration test: asserts --help contains all flags |
| CREATE | docs/TESTS.md | Test catalogue with entry for cli_help_shows_all_flags |
| MODIFY | Cargo.toml | Added resolver = "3" for edition 2024 compatibility |
| MODIFY | backend/Cargo.toml | Added no-op mock-hardware feature for workspace-level flag compatibility |

Note: The `backend` crate uses `version.workspace = true`, so no patch version bump was
performed — the workspace release version (`0.1.0`) is read-only per §12.

## Commit Log

```
 Cargo.toml                     |  1 +
 backend/Cargo.toml             | 13 +++++++++++++
 backend/src/cli.rs             | 33 +++++++++++++++++++++++++++++++++
 backend/src/main.rs            | 14 ++++++++++++++
 backend/tests/cli_help_test.rs | 41 +++++++++++++++++++++++++++++++++++++++++
 docs/TESTS.md                  | 15 +++++++++++++++
 6 files changed, 117 insertions(+)
```

## Test Results

```
    Finished `test` profile [unoptimized + debuginfo] target(s) in 3.87s
     Running unittests src/main.rs (target/debug/deps/anvilml-71ac4a2077f7d5f9)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/cli_help_test.rs (target/debug/deps/cli_help_test-ec019595c7c38c69)

running 1 test
test tests::cli_help_shows_all_flags ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux:
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.07s

# 2. Mock-hardware Windows:
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 6.64s

# 3. Real-hardware Linux:
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.43s

# 4. Real-hardware Windows:
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.84s
```

All four cross-checks exited 0.

## Project Gates

None applicable — task does not modify config fields, handler signatures, or node types.

## Public API Delta

```
backend/src/cli.rs:10:pub struct Cli {
backend/src/cli.rs:31:pub fn parse() -> Cli {
```

New public items:
- `pub struct Cli` — `backend::cli::Cli` — CLI argument struct with clap derive
- `pub fn parse() -> Cli` — `backend::cli::parse()` — parses CLI args from environment

Both match the plan's Public API Surface table exactly.

## Deviations from Plan

- Added `resolver = "3"` to root `Cargo.toml` — required for edition 2024 compatibility.
  The plan's P1-A1 scaffold did not include this, but `cargo check` fails without it.
- Added no-op `[features] mock-hardware = []` to `backend/Cargo.toml` — the workspace-level
  `--features mock-hardware` flag errors on members that do not define the feature.
  This is a standard pattern for crates that participate in the workspace but do not
  depend on `anvilml-hardware`.
- Version bump skipped: `backend/Cargo.toml` uses `version.workspace = true`, so the
  effective version is inherited from the workspace. The workspace release version is
  read-only per §12, so no patch bump was applied.

## Blockers

None.
