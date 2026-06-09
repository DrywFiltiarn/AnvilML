# Plan Report: P13-A1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P13-A1                                            |
| Phase       | 013 — Dispatch & Execute                          |
| Description | anvilml-scheduler: VramLedger                     |
| Depends on  | P12-A5                                            |
| Project     | anvilml                                           |
| Planned at  | 2026-06-09T07:20:00Z                              |
| Attempt     | 1                                                 |

## Objective

Create the `VramLedger` struct in `crates/anvilml-scheduler/src/ledger.rs` — a per-device VRAM tracker used by the scheduler's dispatch admission logic. The ledger stores total and used VRAM (in MiB) per device index, supports initialization from `HardwareInfo`, and provides `free_mib` and `would_fit` queries for dispatch ranking.

## Scope

### In Scope
- Create `crates/anvilml-scheduler/src/ledger.rs` with:
  - `VramLedger` struct holding `HashMap<u32, (u32 total, u32 used)>`
  - `new() -> Self` — constructor
  - `update(&mut self, device_index: u32, used_mib: u32, total_mib: u32)` — record/update VRAM state for a device
  - `free_mib(&self, device_index: u32) -> u32` — return `total - used` for the device, or `0` if the device is unknown
  - `would_fit(&self, device_index: u32, required_mib: u32) -> bool` — return `true` if `free_mib(device_index) >= required_mib`
  - `init_from(&mut self, hw: &HardwareInfo)` — populate ledger from `HardwareInfo.gpus`, using each `GpuDevice::vram_total_mib` and deriving initial `used` as `total - vram_free_mib`
- Register `ledger` module in `crates/anvilml-scheduler/src/lib.rs` (pub mod + pub use)
- Unit tests in `ledger.rs` covering: init from HardwareInfo, update, free_mib known/unknown, would_fit true/false
- Bump `anvilml-scheduler` patch version (0.1.9 → 0.1.10) per FORGE_AGENT_RULES §12

### Out of Scope
- Integration with `MemoryReport` events (handled in later tasks, e.g. P13-A5)
- Dispatch loop logic (P13-A3)
- `select_worker` algorithm (P13-A2)
- Any changes to `anvilml-core` or other crates

## Approach

1. **Read** `crates/anvilml-core/src/types/hardware.rs` to confirm `GpuDevice` field names (`index`, `vram_total_mib`, `vram_free_mib`) and `HardwareInfo` shape (`gpus: Vec<GpuDevice>`). Already confirmed.

2. **Create** `crates/anvilml-scheduler/src/ledger.rs`:
   - Import `std::collections::HashMap`, `anvilml_core::types::hardware::HardwareInfo`, `tracing`.
   - Define `pub struct VramLedger { devices: HashMap<u32, (u32, u32)> }` where tuple elements are `(total_mib, used_mib)`.
   - Implement `VramLedger::new()` returning empty ledger.
   - Implement `VramLedger::update(&mut self, device_index, used_mib, total_mib)` — inserts or replaces the entry, with a DEBUG log call recording the update.
   - Implement `VramLedger::free_mib(&self, device_index) -> u32` — lookup, return `total - used` if present, `0` if absent.
   - Implement `VramLedger::would_fit(&self, device_index, required_mib) -> bool` — delegate to `free_mib` and compare.
   - Implement `VramLedger::init_from(&mut self, hw: &HardwareInfo)` — iterate `hw.gpus`, call `update` for each with `total = vram_total_mib`, `used = total - vram_free_mib`.
   - Add `#[cfg(test)]` module with tests:
     - `test_init_from`: create `HardwareInfo` with 2 GPUs, build ledger via `init_from`, assert entries exist with correct total/used.
     - `test_update`: start empty, update device 0, assert `free_mib(0)` returns expected.
     - `test_would_fit_true`: set up ledger with 8192 total, 4096 used; `would_fit(0, 3000)` is `true`.
     - `test_would_fit_false`: same setup; `would_fit(0, 5000)` is `false`.
     - `test_free_mib_unknown_device`: ledger empty; `free_mib(99)` returns `0`.

3. **Edit** `crates/anvilml-scheduler/src/lib.rs`:
   - Add `pub mod ledger;` alongside existing modules.
   - Add `pub use ledger::VramLedger;` to re-export the struct.

4. **Bump** `crates/anvilml-scheduler/Cargo.toml` patch version: `0.1.9` → `0.1.10`.

5. **Verify**: run `cargo test -p anvilml-scheduler -- ledger` — must exit 0.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Create | `crates/anvilml-scheduler/src/ledger.rs` | VramLedger struct + unit tests |
| Modify | `crates/anvilml-scheduler/src/lib.rs` | Add `pub mod ledger;` and `pub use ledger::VramLedger;` |
| Modify | `crates/anvilml-scheduler/Cargo.toml` | Bump patch version `0.1.9 → 0.1.10` |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `crates/anvilml-scheduler/src/ledger.rs` | `test_init_from` | Ledger correctly populates from `HardwareInfo.gpus` with accurate total/used values |
| `crates/anvilml-scheduler/src/ledger.rs` | `test_update` | `update()` inserts and replaces device entries correctly |
| `crates/anvilml-scheduler/src/ledger.rs` | `test_would_fit_true` | `would_fit` returns `true` when free VRAM >= required |
| `crates/anvilml-scheduler/src/ledger.rs` | `test_would_fit_false` | `would_fit` returns `false` when free VRAM < required |
| `crates/anvilml-scheduler/src/ledger.rs` | `test_free_mib_unknown_device` | `free_mib` returns `0` for unknown device index |

## CI Impact

No CI changes required. The task only adds a new module and unit tests within the existing `anvilml-scheduler` crate. The existing CI gates (`cargo test --workspace --features mock-hardware`, `cargo clippy`, format checks) will naturally cover the new code. No new dependencies are introduced.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `init_from` computes negative `used` if `vram_free_mib > vram_total_mib` | Low | Medium | Clamp: `used = total.saturating_sub(vram_free_mib)` to prevent underflow. |
| Ledger not re-exported from lib.rs, breaking downstream crates (P13-A2) | Medium | High | Ensure `pub use ledger::VramLedger;` is in `lib.rs` as part of this task. |
| Test compilation fails due to missing imports | Low | Low | Follow existing module patterns (queue.rs) for imports and test structure. |

## Acceptance Criteria

- [ ] `crates/anvilml-scheduler/src/ledger.rs` exists with `VramLedger` struct and all four methods (`new`, `update`, `free_mib`, `would_fit`, `init_from`)
- [ ] `free_mib` returns `0` for unknown device indices
- [ ] `cargo test -p anvilml-scheduler -- ledger` exits 0 with all tests passing
- [ ] `crates/anvilml-scheduler/src/lib.rs` re-exports `VramLedger`
- [ ] `anvilml-scheduler` patch version bumped to `0.1.10`
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` passes with zero warnings
- [ ] `cargo fmt --all -- --check` exits 0
