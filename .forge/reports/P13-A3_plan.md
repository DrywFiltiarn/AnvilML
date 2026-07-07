# Plan Report: P13-A3

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P13-A3                                       |
| Phase       | 13 — Job Queue                               |
| Description | anvilml-scheduler: VramLedger per-device reservation tracking |
| Depends on  | P13-A1                                       |
| Project     | anvilml                                      |
| Planned at  | 2026-07-07T01:30:00Z                         |
| Attempt     | 1                                            |

## Objective

Create `VramLedger` — a pure in-memory, per-device VRAM reservation tracker in the
`anvilml-scheduler` crate — and the integration test suite that exercises every public
method. The ledger is advisory: it prevents over-scheduling by tracking how much VRAM
has been reserved per device index, but never claims to prevent an actual OOM. It
completes Group A of Phase 13 alongside `JobQueue` (P13-A2) and the jobs migration
(P13-A1).

## Scope

### In Scope
- Create `crates/anvilml-scheduler/src/ledger.rs` with `VramLedger` struct and three
  methods: `reserve()`, `release()`, `free_mib()`.
- Add `pub mod ledger;` and `pub use ledger::VramLedger;` to
  `crates/anvilml-scheduler/src/lib.rs`.
- Create `crates/anvilml-scheduler/tests/ledger_tests.rs` with ≥6 tests covering
  reserve, release, over-release, unknown device, default capacity, and multi-device
  tracking.
- Bump `anvilml-scheduler` patch version from `0.1.8` to `0.1.9` in `Cargo.toml`.

### Out of Scope
- The dispatch loop that consults the ledger before assigning a job (future phase).
- Integration with `JobStore` persistence (P13-B1).
- Any logging instrumentation — `VramLedger` is a pure data structure with no I/O or
  async, so logging is not applicable here.
- Dual-mode parity markers — these apply to Python node functions (`execute()`,
  `load()`, `sample()`, `decode()`), not to Rust data structures.

## Existing Codebase Assessment

The `anvilml-scheduler` crate already contains `JobQueue` (P13-A2) in `queue.rs` and
`ValidatedGraph`/`GraphError` (P12-A6) in `types.rs` and `dag.rs`. The crate's
conventions are well-established:

- **Naming:** struct names are `PascalCase`, methods are `snake_case`, fields are
  `snake_case` with doc comments explaining each field's role.
- **Error handling:** No `Result` types on `VramLedger` — it uses saturating arithmetic
  so it never fails. `JobQueue` similarly has no error variants; it uses `bool` returns
  and `Option` returns for status signals.
- **Test style:** Integration tests live in `crates/anvilml-scheduler/tests/` as
  separate test crates (e.g., `queue_tests.rs`). Each test has a `///` doc comment
  explaining the precondition, setup, and expected outcome. Tests use direct struct
  literals with a `make_job()` helper when constructing `Job` values.
- **lib.rs discipline:** The file contains only `//!` crate-level doc, `pub mod`
  declarations, and `pub use` re-exports. Currently 9 lines.

No gap exists between the design doc and current source for this task — `ledger.rs`
does not yet exist and needs to be created from scratch.

## Resolved Dependencies

None. `VramLedger` uses only `std::collections::HashMap` — no new external crates are
required.

## Approach

### Step 1 — Create `crates/anvilml-scheduler/src/ledger.rs`

Write the `VramLedger` struct and its `impl` block:

```rust
/// Per-device VRAM reservation ledger.
///
/// `VramLedger` tracks how much VRAM (in MiB) has been reserved on each device
/// index. It is advisory only — it prevents over-scheduling but does not claim to
/// guarantee VRAM sufficiency. A real OOM during execution is still possible; the
/// worker emits `Failed`, and the scheduler calls `release()` to return the
/// reservation.
///
/// Reservations are keyed by device index (`u32`). A device index that has never
/// been reserved is treated as having zero reservation, which means
/// `free_mib(device, total)` returns `total` for an unknown device.
///
/// `release()` uses saturating subtraction so that releasing more than was
/// reserved (which can legitimately happen with imprecise estimates) never
/// panics or underflows.
#[derive(Debug, Default)]
pub struct VramLedger {
    /// Per-device VRAM reservations: device_index → reserved_mib.
    reservations: HashMap<u32, u32>,
}
```

Then implement the three public methods:

1. **`pub fn reserve(&mut self, device_index: u32, vram_mib: u32)`** — adds
   `vram_mib` to the existing reservation for `device_index`, or inserts it if the
   device is unknown. Uses `HashMap::entry()` with the `or_insert(0)` default to
   handle both cases in one expression:
   ```rust
   *self.reservations.entry(device_index).or_insert(0) += vram_mib;
   ```

2. **`pub fn release(&mut self, device_index: u32, vram_mib: u32)`** — subtracts
   `vram_mib` from the existing reservation using saturating subtraction. If the
   device is unknown (no reservation exists), `saturating_sub(0, vram_mib)` yields
   `0`, so the device entry is not created. Uses `HashMap::entry()` with
   `or_insert(0).saturating_sub(vram_mib)` to handle both cases:
   ```rust
   // Saturating subtract so over-release never panics or underflows.
   // If the device has no reservation, saturating_sub(0, vram_mib) = 0,
   // so the entry is not inserted for zero balances.
   *self.reservations.entry(device_index).or_insert(0)
       .saturating_sub(vram_mib);
   ```
   After the subtraction, if the result is `0`, remove the entry to keep the map
   lean. This is a minor optimisation — a device with zero reservation is functionally
   the same as an unknown device for `free_mib()`.

3. **`pub fn free_mib(&self, device_index: u32, total_mib: u32) -> u32`** — returns
   `total_mib - reservation` for the given device, using saturating subtraction so
   that if somehow the reservation exceeds total (shouldn't happen with correct usage,
   but the ledger is advisory so we defend against it), the result is `0`:
   ```rust
   let reserved = self.reservations.get(&device_index).copied().unwrap_or(0);
   total_mib.saturating_sub(reserved)
   ```

### Step 2 — Update `crates/anvilml-scheduler/src/lib.rs`

Add the module declaration and re-export:
```rust
pub mod ledger;
pub use ledger::VramLedger;
```

This keeps `lib.rs` at 11 lines, well under the 80-line hard cap.

### Step 3 — Create `crates/anvilml-scheduler/tests/ledger_tests.rs`

Write ≥6 integration tests following the established `queue_tests.rs` style:

1. **`test_reserve_reduces_free_mib`** — creates ledger with 0 reservation, reserves
   4096 MiB on device 0, asserts `free_mib(0, 8192)` returns 4096.
2. **`test_release_restores_capacity`** — reserves 4096 MiB on device 0, releases
   4096 MiB, asserts `free_mib(0, 8192)` returns 8192.
3. **`test_over_release_does_not_panic`** — releases 8192 MiB from a device that
   was only reserved 4096 MiB. Asserts the call does not panic and `free_mib(0, 8192)`
   still returns 8192 (reservation is clamped to 0).
4. **`test_unknown_device_returns_total_mib`** — never reserves on any device, calls
   `free_mib(5, 16384)`, asserts it returns 16384.
5. **`test_reserve_accumulates_on_same_device`** — reserves 4096 MiB on device 0
   twice, asserts `free_mib(0, 8192)` returns 0 (8192 - 8192 = 0).
6. **`test_multi_device_independent`** — reserves 4096 MiB on device 0 and 2048 MiB
   on device 1, asserts device 0 shows 4096 free (of 8192) and device 1 shows 6144
   free (of 8192).

### Step 4 — Bump `anvilml-scheduler` version

Change `version = "0.1.8"` to `version = "0.1.9"` in
`crates/anvilml-scheduler/Cargo.toml`.

## Public API Surface

| Path | Item | Signature |
|------|------|-----------|
| `anvilml_scheduler::VramLedger` | struct | `pub struct VramLedger { reservations: HashMap<u32, u32> }` |
| `VramLedger::new` | fn | `pub fn new() -> Self` (via `Default`) |
| `VramLedger::reserve` | fn | `pub fn reserve(&mut self, device_index: u32, vram_mib: u32)` |
| `VramLedger::release` | fn | `pub fn release(&mut self, device_index: u32, vram_mib: u32)` |
| `VramLedger::free_mib` | fn | `pub fn free_mib(&self, device_index: u32, total_mib: u32) -> u32` |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-scheduler/src/ledger.rs` | `VramLedger` struct with `reserve`, `release`, `free_mib` |
| MODIFY | `crates/anvilml-scheduler/src/lib.rs` | Add `pub mod ledger;` and `pub use ledger::VramLedger;` |
| CREATE | `crates/anvilml-scheduler/tests/ledger_tests.rs` | ≥6 integration tests |
| MODIFY | `crates/anvilml-scheduler/Cargo.toml` | Bump patch version 0.1.8 → 0.1.9 |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-scheduler/tests/ledger_tests.rs` | `test_reserve_reduces_free_mib` | Reserve reduces available VRAM for the device | Empty ledger | Reserve 4096 on device 0, total 8192 | `free_mib(0, 8192)` returns 4096 | `cargo test -p anvilml-scheduler --test ledger_tests -- test_reserve_reduces_free_mib` exits 0 |
| `crates/anvilml-scheduler/tests/ledger_tests.rs` | `test_release_restores_capacity` | Release restores previously reserved capacity | Ledger with reservation | Reserve 4096, release 4096 on device 0, total 8192 | `free_mib(0, 8192)` returns 8192 | `cargo test -p anvilml-scheduler --test ledger_tests -- test_release_restores_capacity` exits 0 |
| `crates/anvilml-scheduler/tests/ledger_tests.rs` | `test_over_release_does_not_panic` | Releasing more than reserved uses saturating sub, never panics | Ledger with reservation | Reserve 4096, release 8192 on device 0, total 8192 | `free_mib(0, 8192)` returns 8192 (reservation clamped to 0) | `cargo test -p anvilml-scheduler --test ledger_tests -- test_over_release_does_not_panic` exits 0 |
| `crates/anvilml-scheduler/tests/ledger_tests.rs` | `test_unknown_device_returns_total_mib` | Unknown device returns total with no reservation | Empty ledger, no prior ops on device 5 | `free_mib(5, 16384)` | Returns 16384 | `cargo test -p anvilml-scheduler --test ledger_tests -- test_unknown_device_returns_total_mib` exits 0 |
| `crates/anvilml-scheduler/tests/ledger_tests.rs` | `test_reserve_accumulates_on_same_device` | Multiple reserves on same device accumulate | Empty ledger | Reserve 4096 twice on device 0, total 8192 | `free_mib(0, 8192)` returns 0 | `cargo test -p anvilml-scheduler --test ledger_tests -- test_reserve_accumulates_on_same_device` exits 0 |
| `crates/anvilml-scheduler/tests/ledger_tests.rs` | `test_multi_device_independent` | Reservations on different devices are independent | Empty ledger | Reserve 4096 on device 0, 2048 on device 1, both total 8192 | Device 0: 4096 free, Device 1: 6144 free | `cargo test -p anvilml-scheduler --test ledger_tests -- test_multi_device_independent` exits 0 |

## CI Impact

No CI changes required. The new `ledger_tests.rs` file is automatically picked up by
`cargo test --workspace --features mock-hardware` because it lives under the crate's
`tests/` directory, which Cargo treats as an integration test crate. The CI job
`rust-linux` (and `rust-windows`) will run these tests as part of the full workspace
test suite.

## Platform Considerations

None identified. `VramLedger` uses only `std::collections::HashMap` and `u32`
arithmetic — no platform-specific code, no `#[cfg(unix)]` or `#[cfg(windows)]` guards
needed. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `release()` saturating subtraction removes the zero-reservation entry, but a subsequent `reserve()` on the same device starts fresh at the new value rather than resuming from zero. If the dispatch loop expects the reservation to persist across release+reserve cycles, this could undercount. | Low | Medium | The ledger is advisory and per-dispatch — each `reserve()` is called when a job is dispatched, each `release()` when it completes. The entry removal on zero is purely a memory optimisation; the functional behaviour (free_mib returns total for zero-reservation devices) is identical with or without the entry. Document this in the struct's doc comment. |
| The task context specifies `HashMap<u32, u32>` but does not specify whether `free_mib` should use saturating subtraction when reservation > total_mib. Without it, a bug in the dispatch loop could cause a panic on `free_mib`. | Low | Medium | Use `saturating_sub` in `free_mib` as a defensive measure — the ledger is advisory, so defending against incorrect caller behaviour is appropriate. The design doc says "advisory only" which implies this defensibility. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-scheduler --test ledger_tests` exits 0
- [ ] `wc -l crates/anvilml-scheduler/src/lib.rs` returns ≤ 80
- [ ] `grep 'pub use ledger::VramLedger' crates/anvilml-scheduler/src/lib.rs` matches (re-export present)
- [ ] `grep 'pub mod ledger' crates/anvilml-scheduler/src/lib.rs` matches (module declared)
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0 (no new warnings introduced)
