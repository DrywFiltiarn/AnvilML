# Implementation Report: P4-A1

| Field         | Value                              |
|---------------|------------------------------------|
| Task ID       | P4-A1                              |
| Phase         | 004 — Hardware Detection: Detectors |
| Description   | anvilml-hardware: DeviceDetector trait + crate scaffolding |
| Implemented   | 2026-06-28T22:30:00Z               |
| Status        | COMPLETE                           |

## Summary

Created the `DeviceDetector` trait in `crates/anvilml-hardware/src/detect.rs` with two
methods (`detect` and `refresh_vram`) matching `ANVILML_DESIGN.md §6.5` verbatim, and
wired it into the crate's public API via `lib.rs`. The trait uses `Send + Sync`
supertrait bounds and depends on `anvilml_core::{AnvilError, GpuDevice}` from Phase 3.
No concrete implementors exist yet — those are separate tasks (P4-A2 through P4-A6).
All workspace compile, clippy, cross-check, and test gates pass cleanly.

## Resolved Dependencies

None. The only dependency (`anvilml-core`) is already present in `Cargo.toml` from
Phase 1 and was not added or modified by this task.

| Type   | Name    | Version resolved | Source         |
|--------|---------|------------------|----------------|
| crate  | anvilml-core | (workspace path) | N/A           |

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-hardware/src/detect.rs` | `DeviceDetector` trait with `///` doc comments on trait and both methods, per FORGE_AGENT_RULES §12.1 |
| MODIFY | `crates/anvilml-hardware/src/lib.rs` | Added `pub mod detect;` and `pub use detect::DeviceDetector;` after crate-level doc comment |

Note: `anvilml-hardware` uses `version.workspace = true` in its `Cargo.toml`. The
workspace `[package] version` (`0.1.0`) is read-only per ENVIRONMENT.md §12, so no
individual patch bump was possible for this crate.

## Commit Log

```
 .forge/reports/P4-A1_plan.md          | 168 ++++++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md          |   6 +-
 .forge/state/state.json               |  13 +--
 crates/anvilml-hardware/src/detect.rs |  28 ++++++
 crates/anvilml-hardware/src/lib.rs    |   3 +
 5 files changed, 209 insertions(+), 9 deletions(-)
```

## Test Results

```
     Running unittests src/lib.rs (target/debug/deps/anvilml_hardware-a6126077e9976220)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_hardware-...)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

All 76 tests across the workspace passed with 0 failures.
```

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
  → Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.22s

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
  → Finished `dev` profile [unoptimized + debuginfo] target(s) in 24.99s

# 3. Real-hardware Linux
cargo check --bin anvilml
  → Finished `dev` profile [unoptimized + debuginfo] target(s) in 20.54s

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
  → Finished `dev` profile [unoptimized + debuginfo] target(s) in 18.34s
```

All four checks exited 0.

## Project Gates

Gate 1 — Config Surface Sync:
```
cargo test -p anvilml --features mock-hardware -- config_reference
  → test tests::config_reference_matches_defaults ... ok
  → test result: ok. 1 passed; 0 failed; 0 ignored
```

Gate 2 — OpenAPI Drift: Not triggered (no handler signature changes).
Gate 3 — Node Parity: Not triggered (no node type changes).
Gate 4 — Mock/Real Parity Markers: Not triggered (no node execute() or arch load()/
sample()/decode()/compute_latent_shape() changes).

## Public API Delta

```
+pub mod detect;
+pub use detect::DeviceDetector;
```

Two new public items introduced:
- `pub mod detect` — module declaration in `anvilml_hardware`
- `pub use detect::DeviceDetector` — re-export at crate root

The trait itself (`DeviceDetector`) is `pub` within the `detect` module and is
re-exported at the crate root, matching the plan's Public API Surface table.

## Deviations from Plan

None. All implementation matches the approved plan exactly:
- `detect.rs` contains the `DeviceDetector` trait with both methods and doc comments
  matching `ANVILML_DESIGN.md §6.5`
- `lib.rs` declares `pub mod detect;` and `pub use detect::DeviceDetector;`
- No new dependencies introduced
- No test files required (plan states trait has no implementors yet)
- Version bump not possible: `anvilml-hardware` uses `version.workspace = true` and
  the workspace version is read-only per ENVIRONMENT.md §12

## Blockers

None.
