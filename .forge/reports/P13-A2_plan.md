# Plan Report: P13-A2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P13-A2                                      |
| Phase       | 013 — Job Queue & Persistence               |
| Description | VramLedger per-device VRAM reservation      |
| Depends on  | none                                        |
| Project     | anvilml                                     |
| Planned at  | 2026-06-19T19:00:00Z                        |
| Attempt     | 1                                           |

## Objective

Create `VramLedger` in `crates/anvilml-scheduler/src/ledger.rs` — a pure-synchronous, in-memory data structure that tracks per-device VRAM reservations. This ledger is used by the dispatch loop (Phase 014) to avoid over-scheduling a GPU device. It provides device registration, fit-checking before dispatch, and reservation/release semantics. The task also adds the `pub mod ledger` declaration to `lib.rs` and creates a test file with ≥5 tests.

## Scope

### In Scope
- Create `crates/anvilml-scheduler/src/ledger.rs` with `VramLedger` struct and four public methods: `register_device`, `would_fit`, `reserve`, `release`.
- Add `pub mod ledger` to `crates/anvilml-scheduler/src/lib.rs`.
- Create `crates/anvilml-scheduler/tests/ledger_tests.rs` with ≥5 tests.
- Bump `anvilml-scheduler` version from `0.1.5` to `0.1.6` in `Cargo.toml`.
- Add `tracing::debug!` log call in `reserve()` per mandatory DEBUG log points (ENVIRONMENT.md §9).

### Out of Scope
- Integration with `JobScheduler` or dispatch loop (Phase 014).
- Any database persistence of ledger state.
- Any async code — `VramLedger` is pure synchronous logic.
- Any hardware detection or real VRAM querying.
- Any HTTP handler changes (Phase 013-B1).
- Version bump of any other crate.

## Existing Codebase Assessment

The `anvilml-scheduler` crate already has three modules: `queue.rs` (JobQueue FIFO), `dag.rs` (graph validation), and `types.rs` (GraphError enum). The `lib.rs` follows the project convention: `//!` crate-level doc comment, `pub use` re-exports, and `pub mod` declarations — all under 80 lines (currently 24 lines).

The `queue.rs` module establishes the coding style for this crate: `///` doc comments on every `pub` item with argument descriptions and return value documentation, inline `//` comments explaining non-obvious decisions (e.g. swap-remove strategy in `cancel`), `#[derive(Debug, Clone)]` on public types, and `Default` implementations. Tests live in `tests/` as separate test crates, not inline `#[cfg(test)]` blocks — following the project's test file convention (ENVIRONMENT.md §11).

No `ledger.rs` or `ledger_tests.rs` exists yet. The design doc (ANVILML_DESIGN.md §11.4) describes the ledger as a per-device VRAM reservation tracker that is advisory, not enforced. The task context specifies the exact struct fields (`reservations: HashMap<u32, u32>`, `totals: HashMap<u32, u32>`) and method signatures, which I have confirmed against the existing codebase patterns.

No new external dependencies are needed — the ledger only uses `std::collections::HashMap`, which is part of the Rust standard library. The `tracing` crate is already declared in `Cargo.toml` and used throughout the crate (e.g., `dag.rs` uses `#[tracing::instrument]`).

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| std    | std::collections::HashMap | N/A (stdlib) | N/A | n/a |
| crate  | tracing | Already in Cargo.toml (workspace dep) | Cargo.toml | n/a |

No new external dependencies are introduced. The ledger uses only `std::collections::HashMap` (stdlib) and `tracing::debug!` (already a workspace dependency).

## Approach

1. **Create `crates/anvilml-scheduler/src/ledger.rs`** with the following:
   - A module-level `//!` doc comment describing the ledger's purpose (per-device VRAM reservation tracking, pure sync, advisory only).
   - `use std::collections::HashMap;` and `use tracing;` imports.
   - `pub struct VramLedger { reservations: HashMap<u32, u32>, totals: HashMap<u32, u32> }` with `///` doc comment on the struct and each field explaining what it tracks.
   - `impl VramLedger { ... }` block with four methods:
     - `pub fn new() -> Self` — creates empty ledger (returns `Self` with two empty HashMaps). Implements `Default`.
     - `pub fn register_device(&mut self, index: u32, vram_total_mib: u32)` — stores the device's total VRAM in `totals` and sets `reservations[index]` to `0` (no prior reservations). If the device is already registered, this is a no-op (idempotent — prevents duplicate registration errors).
     - `pub fn would_fit(&self, index: u32, requested_mib: u32) -> bool` — returns `false` if the device index is unknown in `totals` (device not registered). Otherwise returns `totals[index] - reservations[index] >= requested_mib`. This is a pure computation — no side effects.
     - `pub fn reserve(&mut self, index: u32, mib: u32)` — asserts the device exists, updates `reservations[index] += mib`, logs `tracing::debug!(device_index = index, reserved_mib = mib, free_after_mib = totals[index] - reservations[index], "vram reserved")` using structured field notation per ENVIRONMENT.md §9, and panics if the reservation would exceed total VRAM. The panic is intentional: it represents a programming error in the dispatch loop (scheduling beyond capacity), not a recoverable runtime condition. The dispatch loop must check `would_fit` before calling `reserve`.
     - `pub fn release(&mut self, index: u32, mib: u32)` — asserts the device exists, updates `reservations[index] -= mib`, and panics if the release would underflow (reservation cannot go negative — represents a bug in release logic).
   - `impl Default for VramLedger { fn default() -> Self { Self::new() } }` — follows the pattern established by `JobQueue`.
   - Every `pub` item gets a `///` doc comment describing what it does, its arguments, and return value.
   - Every decision point in method bodies gets an inline `//` comment (e.g., why panic on over-reservation, why no-op on duplicate registration).

2. **Update `crates/anvilml-scheduler/src/lib.rs`** — add `pub mod ledger;` after the existing `pub mod queue;` declaration. Keep the file under 80 lines.

3. **Create `crates/anvilml-scheduler/tests/ledger_tests.rs`** with ≥5 tests following the `queue_tests.rs` style:
   - Separate test crate file (not inline `#[cfg(test)]`).
   - Doc comment on each test describing what it verifies and its preconditions.
   - No external dependencies beyond the crate's public API.
   - Tests: (a) register_device + would_fit true; (b) would_fit false for unknown device; (c) reserve reduces free VRAM; (d) release restores free VRAM; (e) reserve overflow panics.

4. **Bump `anvilml-scheduler` version** in `Cargo.toml` from `0.1.5` to `0.1.6`.

## Public API Surface

| Item | Crate/Module Path | Signature |
|------|-------------------|-----------|
| `VramLedger` | `anvilml_scheduler::ledger::VramLedger` | `pub struct VramLedger { reservations: HashMap<u32, u32>, totals: HashMap<u32, u32> }` |
| `new` | `anvilml_scheduler::ledger::VramLedger::new` | `pub fn new() -> Self` |
| `register_device` | `anvilml_scheduler::ledger::VramLedger::register_device` | `pub fn register_device(&mut self, index: u32, vram_total_mib: u32)` |
| `would_fit` | `anvilml_scheduler::ledger::VramLedger::would_fit` | `pub fn would_fit(&self, index: u32, requested_mib: u32) -> bool` |
| `reserve` | `anvilml_scheduler::ledger::VramLedger::reserve` | `pub fn reserve(&mut self, index: u32, mib: u32)` |
| `release` | `anvilml_scheduler::ledger::VramLedger::release` | `pub fn release(&mut self, index: u32, mib: u32)` |

No trait impls are public beyond `Default`. No re-exports are needed.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-scheduler/src/ledger.rs` | New file; VramLedger struct and methods |
| MODIFY | `crates/anvilml-scheduler/src/lib.rs` | Add `pub mod ledger;` |
| CREATE | `crates/anvilml-scheduler/tests/ledger_tests.rs` | New test file; ≥5 tests |
| MODIFY | `crates/anvilml-scheduler/Cargo.toml` | Bump patch version 0.1.5 → 0.1.6 |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-scheduler/tests/ledger_tests.rs` | `test_register_device_and_would_fit` | Register a device and confirm `would_fit` returns true for any amount up to total | Fresh ledger | `register_device(0, 16384)`, `would_fit(0, 8192)` | `true` | `cargo test -p anvilml-scheduler --features mock-hardware -- ledger` exits 0 |
| `crates/anvilml-scheduler/tests/ledger_tests.rs` | `test_would_fit_unknown_device` | `would_fit` returns false for a device index that was never registered | Fresh ledger | `would_fit(99, 1024)` | `false` | `cargo test -p anvilml-scheduler --features mock-hardware -- ledger` exits 0 |
| `crates/anvilml-scheduler/tests/ledger_tests.rs` | `test_reserve_reduces_free_vram` | Reserving VRAM reduces the free amount; `would_fit` for the reserved amount returns false after reserve | Fresh ledger with registered device | `register_device(0, 16384)`, `reserve(0, 8192)`, `would_fit(0, 8192)` | `false` (no free space left for that amount) | `cargo test -p anvilml-scheduler --features mock-hardware -- ledger` exits 0 |
| `crates/anvilml-scheduler/tests/ledger_tests.rs` | `test_release_restores_free_vram` | Releasing VRAM restores the free amount; `would_fit` returns true after release | Fresh ledger with registered device and active reservation | `register_device(0, 16384)`, `reserve(0, 8192)`, `release(0, 8192)`, `would_fit(0, 8192)` | `true` | `cargo test -p anvilml-scheduler --features mock-hardware -- ledger` exits 0 |
| `crates/anvilml-scheduler/tests/ledger_tests.rs` | `test_reserve_multiple_and_release` | Multiple reserves and releases on the same device track correctly | Fresh ledger with registered device | `register_device(0, 16384)`, `reserve(0, 4096)`, `reserve(0, 4096)`, `release(0, 4096)`, `would_fit(0, 8192)` | `false` (only 8192 free after partial release) | `cargo test -p anvilml-scheduler --features mock-hardware -- ledger` exits 0 |

## CI Impact

No CI changes required. The task adds a new test module (`ledger_tests.rs`) which is automatically picked up by `cargo test --workspace --features mock-hardware`. No new file types, gates, or test frameworks are introduced. The existing CI job matrix (rust-linux, rust-windows) already runs the full workspace test suite with `--features mock-hardware`, which includes this new test file.

## Platform Considerations

None identified. The `VramLedger` uses only `std::collections::HashMap` and primitive `u32` types — no platform-specific code, no `#[cfg(unix)]` or `#[cfg(windows)]` guards required. The Windows cross-check in ENVIRONMENT.md §7 (`cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu`) is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Panic on over-reservation or under-release may conflict with callers that expect `Result` instead of panic | Low | Medium | The dispatch loop (Phase 014) is the sole intended caller, and it must call `would_fit` before `reserve`. Panicking on a programming error (scheduling beyond capacity) is appropriate — a `Result` would add error handling noise for an invariant that the caller already checks. Document this in the `///` doc comment on `reserve` and `release`. |
| Test for panic behavior requires `#[should_panic]` which may not work well with `cargo test --filter ledger` | Low | Low | Use `#[should_panic(expected = "...")]` with a specific error message substring. The test filter `-- ledger` will match the test module name. If `#[should_panic]` causes issues with the test filter, the test can be placed in the same file and named with `ledger_` prefix. |
| `HashMap` iteration order is non-deterministic — not relevant for correctness but could affect debug output | Low | Low | Not applicable. The ledger never exposes internal HashMap state externally; all public methods return computed results (bool, or mutate state deterministically). |

## Acceptance Criteria

- [ ] `head -1 .forge/reports/P13-A2_plan.md` prints `# Plan Report: P13-A2`
- [ ] `grep "^## " .forge/reports/P13-A2_plan.md` returns 12 section headings
- [ ] `wc -l .forge/reports/P13-A2_plan.md` returns > 40 lines
- [ ] `cargo test -p anvilml-scheduler --features mock-hardware -- ledger` exits 0 with ≥ 5 tests
- [ ] `cargo check --workspace --features mock-hardware` exits 0
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
