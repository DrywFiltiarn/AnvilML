# Plan Report: P4-A1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P4-A1                                             |
| Phase       | 004 — Hardware Detection: Detectors               |
| Description | anvilml-hardware: DeviceDetector trait + crate scaffolding |
| Depends on  | P3-A5                                             |
| Project     | anvilml                                           |
| Planned at  | 2026-06-28T22:15:00Z                              |
| Attempt     | 1                                                 |

## Objective

Define the `DeviceDetector` trait in `crates/anvilml-hardware/src/detect.rs` and wire
it into the crate's public API via `lib.rs`. This establishes the shared contract that
all five concrete detectors (CpuDetector, MockDetector, VulkanDetector, DxgiDetector,
SysfsPciDetector) in Phase 4 will implement. The trait uses types from `anvilml-core`
(`GpuDevice`, `AnvilError`) already provided by Phase 3. No concrete implementations
are written here — those are separate tasks (P4-A2 through P4-A6).

## Scope

### In Scope
- Create `crates/anvilml-hardware/src/detect.rs` containing the `DeviceDetector` trait
  with exactly two methods, per `ANVILML_DESIGN.md §6.5` verbatim.
- Modify `crates/anvilml-hardware/src/lib.rs` to declare `mod detect;` and
  `pub use detect::DeviceDetector;`.
- Add `///` doc comments on the trait and both methods, per `FORGE_AGENT_RULES.md §12.1`.
- Verify `cargo build -p anvilml-hardware` exits 0.

### Out of Scope
None. This task's `defers_to` field is `[]` (empty). No scope is deferred.

`defers_to (from JSON): []`

## Existing Codebase Assessment

The `anvilml-hardware` crate exists as a buildable stub created in Phase 1 (P1-B2).
Its `lib.rs` contains only the crate-level `//!` doc comment (one line). Its
`Cargo.toml` already declares `anvilml-core = { path = "../anvilml-core" }` as a
dependency and has the `mock-hardware` feature declared. No `src/` modules exist yet
besides `lib.rs`.

The required types are fully defined and re-exported from `anvilml-core`:
- `GpuDevice` — `pub struct` in `crates/anvilml-core/src/types/hardware.rs`, re-exported
  via `pub use types::*;` in `anvilml-core/src/lib.rs`.
- `AnvilError` — `pub enum` in `crates/anvilml-core/src/error.rs`, re-exported via
  `pub use error::AnvilError;` in `anvilml-core/src/lib.rs`.

The crate follows the project's `lib.rs` discipline: only a `//!` crate-level doc comment
and `pub mod`/`pub use` declarations, no implementation code. The established pattern
is to declare each detector as a separate module (`detect.rs`, `cpu.rs`, `vulkan.rs`,
etc.) in the crate's flat `src/` layout.

No gap exists between the design doc and current source — the stub is exactly what Phase 1
left, ready for Phase 4 to populate.

## Resolved Dependencies

No new external crates are introduced by this task. The only dependency (`anvilml-core`)
is already present in `Cargo.toml` from Phase 1 and was not added by this task.

| Type   | Name        | Version verified | MCP source | Feature flags confirmed |
|--------|-------------|-----------------|------------|------------------------|
| crate  | anvilml-core | (workspace path) | N/A       | N/A                    |

## Approach

1. **Create `crates/anvilml-hardware/src/detect.rs`.** Write the `DeviceDetector` trait
   with exactly two methods, matching `ANVILML_DESIGN.md §6.5` verbatim:

   ```rust
   use anvilml_core::{AnvilError, GpuDevice};

   /// Trait for detecting and refreshing GPU device information.
   ///
   /// Every concrete detector (CPU, mock, Vulkan, DXGI, sysfs) implements this trait.
   /// Implementations must never panic on missing drivers or hardware — they return
   /// `Ok(vec![])` on detection failure per §6.2 of the design.
   pub trait DeviceDetector: Send + Sync {
       /// Enumerate all compute devices on the host.
       ///
       /// Returns a vector of detected `GpuDevice` structs. If no devices are found,
       /// returns `Ok(vec![])` — never an error or a panic. The caller (Phase 5's
       /// `detect_all_devices`) appends a CPU fallback device if the result is empty.
       fn detect(&self) -> Result<Vec<GpuDevice>, AnvilError>;

       /// Refresh VRAM totals for a device by its index.
       ///
       /// Returns `(total_mib, free_mib)` — the total and free VRAM in mebibytes for
       /// the device at the given `index`. This is called at dispatch time to get a
       /// current snapshot rather than relying on the stale value from `detect()`.
       fn refresh_vram(&self, index: u32) -> Result<(u32, u32), AnvilError>;
   }
   ```

   Rationale: The `Send + Sync` supertrait bounds are part of the design spec (§6.5)
   and ensure the trait object can be held behind an `Arc` and shared across async
   task boundaries without additional synchronization.

2. **Modify `crates/anvilml-hardware/src/lib.rs`.** Add two lines after the existing
   crate-level doc comment:

   ```rust
   pub mod detect;
   pub use detect::DeviceDetector;
   ```

   Rationale: This follows the established pattern — each detector module is declared
   as `pub mod` and re-exported at the crate root so downstream crates (worker, scheduler)
   can reference `anvilml_hardware::DeviceDetector` without knowing which concrete
   detector they're using.

3. **Verify compilation.** Run `cargo build -p anvilml-hardware` and confirm it exits 0.
   The trait has no implementors in this task, so there will be no dead-code warnings
   (Rust only warns on unused *items*, not on unused *traits* — a trait definition alone
   compiles cleanly).

## Public API Surface

| Item | Crate/Module Path | Signature |
|------|-------------------|-----------|
| `DeviceDetector` trait | `anvilml_hardware::detect::DeviceDetector` | `pub trait DeviceDetector: Send + Sync { fn detect(&self) -> Result<Vec<GpuDevice>, AnvilError>; fn refresh_vram(&self, index: u32) -> Result<(u32, u32), AnvilError>; }` |
| Re-export | `anvilml_hardware::DeviceDetector` | `pub use detect::DeviceDetector;` |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-hardware/src/detect.rs` | `DeviceDetector` trait definition with doc comments |
| MODIFY | `crates/anvilml-hardware/src/lib.rs` | Add `pub mod detect;` and `pub use detect::DeviceDetector;` |

## Tests

The acceptance criterion is `cargo build -p anvilml-hardware` exits 0. No test file is
required for this task because: (a) the trait has no implementors yet (no concrete
functionality to exercise), and (b) the design doc's acceptance criterion for P4-A1
specifies only the build gate. Tests for the trait's contract will be written when the
first concrete detector (P4-A2, CpuDetector) is implemented, as part of that detector's
own test file.

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| (build gate) | `cargo build -p anvilml-hardware` | Trait compiles, is `pub`, and is re-exported at crate root | None | None | Exit 0, no warnings about the trait definition | `cargo build -p anvilml-hardware` exits 0 |

## CI Impact

No CI changes required. The trait is dead code until implementors exist (Phase 4 tasks
P4-A2–P4-A6), and `cargo build -p anvilml-hardware` already runs as part of the full
workspace build in every CI job. No new test files, file types, or CI gates are introduced.

## Platform Considerations

None identified. The trait is a pure Rust abstraction with no platform-specific code,
no `#[cfg(...)]` attributes, and no I/O. The Windows cross-check in ENVIRONMENT.md §7
is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The trait's supertrait bounds (`Send + Sync`) may conflict with a future implementor that holds non-`Send`/`Sync` state. | Low | Medium | The bounds are specified verbatim in the design doc (§6.5). If a future detector needs interior mutability, it should use `Arc<Mutex<>>` or `RwLock` internally — this is a standard Rust pattern and does not require removing the supertraits. |
| `anvilml-core` types used in the trait (`GpuDevice`, `AnvilError`) could be renamed or restructured in a subsequent task, breaking this trait's signature. | Low | High | The types are stable domain types defined in Phase 3 (P3-A4/P3-A5) and consumed by every downstream crate. Any rename would be a cross-cutting change caught by the full workspace build (`cargo build --workspace --features mock-hardware`) in the next phase's CI run. |

## Acceptance Criteria

- [ ] `cargo build -p anvilml-hardware` exits 0
