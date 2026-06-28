# Implementation Report: P2-A7

| Field         | Value                                             |
|---------------|---------------------------------------------------|
| Task ID       | P2-A7                                             |
| Phase         | 002 — Core Domain Types: Config & Errors          |
| Description   | config_reference test: anvilml.toml matches ServerConfig |
| Implemented   | 2026-06-28T15:00:00Z                              |
| Status        | COMPLETE                                          |

## Summary

Expanded the repo-root `anvilml.toml` to include every `ServerConfig` field at its documented default, and created the `config_reference_matches_defaults` integration test in `backend/tests/config_reference.rs`. The test loads `anvilml.toml` via `config_load::load()` and asserts all 13 fields match `ServerConfig::default()`, establishing the config-drift invariant that the `config-drift` CI job enforces.

## Resolved Dependencies

None. No new dependencies introduced — the existing `toml` crate (v1.1.2) from `anvilml-core` handles TOML parsing.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | anvilml.toml | Expanded from 2 fields (host, port) to all 13 ServerConfig fields at defaults |
| CREATE | backend/tests/config_reference.rs | Config-drift test: loads anvilml.toml, asserts fields match ServerConfig::default() |
| MODIFY | backend/Cargo.toml | Bumped patch version 0.1.2 → 0.1.3 |
| MODIFY | docs/TESTS.md | Added entry for config_reference_matches_defaults test |

## Commit Log

```
 .forge/reports/P2-A7_plan.md      | 172 ++++++++++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md      |   6 +-
 .forge/state/state.json           |  13 +--
 Cargo.lock                        |   2 +-
 anvilml.toml                      |  28 +++++--
 backend/Cargo.toml                |   2 +-
 backend/tests/config_reference.rs |  73 ++++++++++++++++
 docs/TESTS.md                     |  12 +++
 8 files changed, 289 insertions(+), 19 deletions(-)
```

## Test Results

```
     Running tests/config_reference.rs (target/debug/deps/config_reference-7931ba065e7d5d3d)

running 1 test
test tests::config_reference_matches_defaults ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Full workspace test suite: 61 tests passed, 0 failed across all crates (anvilml, anvilml-core, anvilml-hardware, anvilml-ipc, anvilml-registry, anvilml-artifacts, anvilml-worker, anvilml-server, anvilml-scheduler, anvilml-openapi).

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
  → Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.47s

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
  → Finished `dev` profile [unoptimized + debuginfo] target(s) in 21.27s

# 3. Real-hardware Linux
cargo check --bin anvilml
  → Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.56s

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
  → Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.57s
```

All four checks exited 0.

## Project Gates

Gate 1 (Config Surface Sync): `cargo test -p anvilml --features mock-hardware -- config_reference` → 1 passed, 0 failed.

## Public API Delta

No new `pub` items introduced. The task modifies only a config file and creates a test file that uses existing public APIs (`load()` and `ServerConfig`) from `anvilml-core`.

## Deviations from Plan

None. Implementation matches the approved plan exactly.

## Blockers

None.
