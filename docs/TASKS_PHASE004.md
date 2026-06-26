# Tasks: Phase 4 — Hardware Detection: Detectors

**Phase:** 4
**Name:** Hardware Detection: Detectors
**Project(s):** anvilml
**Status:** Draft
**Depends on phases:** 1, 2, 3

---

## Overview

This phase implements every concrete `DeviceDetector` for `anvilml-hardware`: the
trait itself, the always-succeeds CPU fallback, the env-var-driven mock used by every
CI job and local test, the cross-platform Vulkan-based primary detector, and the two
platform-specific fallback detectors (DXGI on Windows, sysfs on Linux). This phase
builds the individual detectors in isolation — the priority-ordered orchestration
function that picks among them (`detect_all_devices`) is Phase 5's scope, not this
one, since wiring five independent detectors together into one decision function is a
distinct concern from building each detector correctly on its own.

This phase exists right after the domain types (Phase 3) because every detector
returns the `GpuDevice`/`HardwareInfo`/`DeviceType`/`InferenceCaps`/
`EnumerationSource`/`CapabilitySource` types defined there — there is nothing for a
detector to construct until those types exist. The crate enters this phase as an
empty stub (Phase 1's P1-B2) and leaves it with five working, independently-tested
detector implementations, none of which panics under any failure condition per
`ANVILML_DESIGN.md §6.2`'s "detection never panics" principle.

At the end of this phase, each detector can be unit-tested and exercised in isolation,
but no single function yet combines them into the priority chain a real server
startup would use — that orchestration, the `mock-hardware` feature's full
integration, and the corresponding Runnable Proof land in Phase 5.

---

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | Detectors | P4-A1 … P4-A6 | `DeviceDetector` trait, then each concrete implementation in priority order: CPU, Mock, Vulkan, DXGI (Windows), sysfs (Linux) |

A single group is used because every task in this phase adds one detector to the same
crate's flat `src/` layout — there is no meaningful subsystem split within "implement
the detectors."

---

## Prerequisites

`anvilml-core` must export `GpuDevice`, `HardwareInfo`, `DeviceType`,
`InferenceCaps`, `EnumerationSource`, and `CapabilitySource` exactly as defined in
Phase 3 (P3-A4, P3-A5). `anvilml-hardware` must exist as a buildable stub crate with
the `mock-hardware` feature already declared (Phase 1's P1-B2).

---

## Interfaces and Contracts

| Contract document | Relevant to tasks | What must match |
|--------------------|--------------------|------------------|
| `ANVILML_DESIGN.md §6.5` | P4-A1 | `DeviceDetector` trait's exact method signatures |
| `ANVILML_DESIGN.md §6.2` | P4-A2, P4-A4, P4-A5, P4-A6 | "Detection never panics" — every failure mode returns `Ok(vec![])`, never `Err` or a panic |
| `ANVILML_DESIGN.md §6.7` | P4-A3 | `MockDetector`'s exact env var names and defaults |
| `ANVILML_DESIGN.md §17.4` rule 2 | P4-A3 | Test env var restoration on exit, even on panic |

---

## Task Descriptions

### Group A — Detectors

#### P4-A1: anvilml-hardware: DeviceDetector trait + crate scaffolding

**Goal:** Define the trait every concrete detector in this phase implements,
establishing the shared contract before any implementation exists.

**Files to create or modify:**
- `crates/anvilml-hardware/src/detect.rs` — `DeviceDetector` trait only.
- `crates/anvilml-hardware/src/lib.rs` — adds `mod detect; pub use
  detect::DeviceDetector;`.

**Key implementation notes:**
- The trait is exactly two methods: `detect(&self) -> Result<Vec<GpuDevice>,
  AnvilError>` and `refresh_vram(&self, index: u32) -> Result<(u32, u32),
  AnvilError>` (returns `(total_mib, free_mib)`), per `ANVILML_DESIGN.md §6.5`
  verbatim — do not add methods speculatively.
- `detect_all_devices()`, the orchestration function that calls implementors of this
  trait in priority order, is explicitly out of scope here — it belongs to Phase 5.

**Acceptance criterion:**
```bash
cargo build -p anvilml-hardware
# -> exit 0
```

#### P4-A2: anvilml-hardware: CpuDetector always returns one CPU device

**Goal:** Implement the unconditional final-fallback detector that guarantees
`detect_all_devices()` (Phase 5) always has at least one device to report, per
`ANVILML_DESIGN.md §6.2`'s "result is always `Ok(HardwareInfo)` with at least one
device" guarantee.

**Files to create or modify:**
- `crates/anvilml-hardware/src/cpu.rs` — `CpuDetector`.
- `crates/anvilml-hardware/src/lib.rs` — adds `mod cpu;`.

**Key implementation notes:**
- `detect()` never errors and never panics — it always returns exactly one
  synthesized `GpuDevice` with `device_type: DeviceType::Cpu`.
- `enumeration_source` is `EnumerationSource::Cpu` — the amended seventh variant
  added specifically for this purpose (see
  `docs/ADDENDUM_ENUMERATION_SOURCE_CPU.md`). This marks the device as
  *synthesized* by the unconditional fallback, distinct from
  `EnumerationSource::Mock` (reserved for `MockDetector`'s env-var-driven device,
  P4-A3) and from the four real-enumeration variants. The prior session's Deviation
  note on this point is now resolved — no placeholder remains.
- `refresh_vram()` always returns `Ok((0, 0))` — CPU has no VRAM concept.

**Acceptance criterion:**
```bash
cargo test -p anvilml-hardware --test cpu_tests
# -> >=4 tests, exits 0
```

#### P4-A3: anvilml-hardware: MockDetector env-var driven stub

**Goal:** Implement the detector every CI job and most local tests actually run
against, gated behind the `mock-hardware` feature declared in Phase 1.

**Files to create or modify:**
- `crates/anvilml-hardware/src/mock.rs` — `MockDetector`, gated
  `#[cfg(feature = "mock-hardware")]`.
- `crates/anvilml-hardware/src/lib.rs` — adds the same-gated `mod mock;`.

**Key implementation notes:**
- Reads exactly three env vars per `ANVILML_DESIGN.md §6.7`:
  `ANVILML_MOCK_DEVICE_TYPE` (default `"cpu"`), `ANVILML_MOCK_VRAM_MIB` (default
  `8192`), `ANVILML_MOCK_DEVICE_NAME` (default `"Mock GPU"`).
- `enumeration_source` is `EnumerationSource::Mock` here — distinct from
  `EnumerationSource::Cpu` (P4-A2's synthesized fallback device); `Mock` is this
  variant's actual intended use case.
- Every test that sets one of these env vars must capture and restore its
  pre-existing value unconditionally on exit, even on panic, per
  `ANVILML_DESIGN.md §17.4` rule 2 — this is a project-wide, non-negotiable test
  isolation rule, not a suggestion.

**Acceptance criterion:**
```bash
cargo test -p anvilml-hardware --features mock-hardware --test mock_tests
# -> >=4 tests, exits 0
```

#### P4-A4: anvilml-hardware: VulkanDetector headless enumeration

**Goal:** Implement the primary, cross-platform, SDK-free real-hardware detector
that `detect_all_devices()` will try first, per `ANVILML_DESIGN.md §6.2`'s explicit
rejection of `nvidia-smi`/`rocm-smi`/`lspci`/toolkit dependencies.

**Files to create or modify:**
- `crates/anvilml-hardware/src/vulkan.rs` — `VulkanDetector`.
- `crates/anvilml-hardware/Cargo.toml` — adds `ash`.
- `crates/anvilml-hardware/src/lib.rs` — adds `mod vulkan;`.

**Key implementation notes:**
- Resolve `ash`'s current version live via the crates.io registry tool before
  pinning it — do not use a version recalled from training data.
- `detect()` never panics: if the Vulkan loader is absent or `vkCreateInstance`
  fails, return `Ok(vec![])`, never `Err`, per `ANVILML_DESIGN.md §6.2` exactly.
- Vendor ID → `DeviceType` mapping: `0x10de` (NVIDIA) → `Cuda`, `0x1002` (AMD) →
  `Rocm`; any other vendor ID is skipped (not a GPU this system targets). This exact
  mapping is reused verbatim by both fallback detectors (P4-A5, P4-A6) for
  consistency — do not invent a different mapping in either of them.

**Acceptance criterion:**
```bash
cargo test -p anvilml-hardware --test vulkan_tests
# -> >=4 tests, exits 0
```

#### P4-A5: anvilml-hardware: DxgiDetector Windows fallback (cfg-gated)

**Goal:** Implement the Windows-only fallback detector that Phase 5's orchestration
will call when Vulkan enumeration returns empty on a Windows host.

**Files to create or modify:**
- `crates/anvilml-hardware/src/dxgi.rs` — `DxgiDetector`.
- `crates/anvilml-hardware/Cargo.toml` — adds `windows` crate.
- `crates/anvilml-hardware/src/lib.rs` — adds `mod dxgi;` gated
  `#[cfg(target_os = "windows")]`.
- `crates/anvilml-hardware/tests/dxgi_tests.rs` — new, also gated
  `#[cfg(target_os = "windows")]` at the file level.

**Key implementation notes:**
- Uses `IDXGIFactory1::EnumAdapters1` — resolve the `windows` crate's current
  version live via the registry before pinning.
- Reuses the same vendor-ID-to-`DeviceType` mapping as `VulkanDetector` (P4-A4) for
  consistency across detectors.
- `EnumAdapters1` failure returns `Ok(vec![])`, never panics, matching the
  project-wide "detection never panics" rule.
- The test file's `#[cfg(target_os = "windows")]` gate means it compiles to an
  empty test binary on Linux CI and runs for real only on the `windows-latest`
  runner — this is expected and correct, not a gap.

**Acceptance criterion:**
```bash
cargo test -p anvilml-hardware --test dxgi_tests
# -> >=3 tests, exits 0 (on windows-latest CI runner)
```

#### P4-A6: anvilml-hardware: SysfsPciDetector Linux fallback (cfg-gated)

**Goal:** Implement the Linux-only fallback detector that Phase 5's orchestration
will call when Vulkan enumeration returns empty on a Linux host.

**Files to create or modify:**
- `crates/anvilml-hardware/src/sysfs.rs` — `SysfsPciDetector`.
- `crates/anvilml-hardware/src/lib.rs` — adds `mod sysfs;` gated
  `#[cfg(target_os = "linux")]`.
- `crates/anvilml-hardware/tests/sysfs_tests.rs` — new, also gated
  `#[cfg(target_os = "linux")]` at the file level.

**Key implementation notes:**
- Reads `/sys/bus/pci/devices/*/{vendor,device,class}`; filters to PCI class prefix
  `0x03` (display controller) per the PCI class code specification — a device whose
  class isn't a display controller is not a GPU and must be excluded.
- Reuses the same vendor-ID-to-`DeviceType` mapping as `VulkanDetector` (P4-A4).
- A missing `/sys` path or a permission error returns `Ok(vec![])`, never `Err`,
  matching the project-wide "detection never panics" rule.

**Acceptance criterion:**
```bash
cargo test -p anvilml-hardware --test sysfs_tests
# -> >=3 tests, exits 0 (on a Linux runner)
```

---

## Phase Acceptance Criteria

```bash
cargo fmt --all -- --check
cargo clippy --workspace --features mock-hardware -- -D warnings
cargo test --workspace --features mock-hardware

# Platform cross-check (local WSL2 gate, per ENVIRONMENT.md §7):
cargo check --workspace --features mock-hardware
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
cargo check --bin anvilml
cargo check --bin anvilml --target x86_64-pc-windows-gnu

# Runnable Proof: not applicable — this phase implements individual DeviceDetector
# trait objects with no orchestration function wiring them together yet, and no
# new HTTP endpoint, WebSocket event, CLI flag, or file-on-disk artifact a human or
# script could observe from outside the test process. Each detector's own test
# file (cpu_tests, mock_tests, vulkan_tests, dxgi_tests, sysfs_tests) is the
# complete and sufficient proof of this phase's deliverable, per the narrow
# exemption in FORGE_TASK_AUTHORING_SPEC.md §9. The live, externally observable
# capability ("the server reports real detected hardware") is Phase 5's Runnable
# Proof, once detect_all_devices() actually exists to wire these detectors
# together and a handler exposes the result.
```

---

## Known Constraints and Gotchas

- `CpuDetector` uses `EnumerationSource::Cpu` (P4-A2), the amended seventh variant
  added specifically to mark a synthesized fallback device — see
  `docs/ADDENDUM_ENUMERATION_SOURCE_CPU.md` for the exact `ANVILML_DESIGN.md §5.5`
  diff this depends on. This is distinct from `EnumerationSource::Mock`
  (`MockDetector`, P4-A3), which marks an env-var-driven synthetic device, not a
  fallback. Both variants now have an unambiguous, non-overlapping meaning.
- `DxgiDetector` and `SysfsPciDetector` are both `cfg`-gated at the module
  declaration in `lib.rs`, not by wrapping the entire file's contents in a `cfg`
  attribute internally — keep the gate at the `mod` statement, matching
  `ANVILML_DESIGN.md §6.3`'s module layout.
- All three real/fallback detectors (Vulkan, DXGI, sysfs) must use the identical
  vendor-ID-to-`DeviceType` mapping (`0x10de`→`Cuda`, `0x1002`→`Rocm`) — a divergence
  between them would make `detect_all_devices()`'s fallback chain (Phase 5) produce
  inconsistent results depending on which detector happened to succeed.
- No detector implementation in this phase may panic under any failure condition —
  loader absence, permission errors, and enumeration failures all return `Ok(vec![])`,
  never `Err` and never an unwrapped panic.

---

## docs/RUNNABLE_PROOF.md entry

```markdown
## Phase 4 — Hardware Detection: Detectors

**Capability proved:** Not applicable — this phase implements individual
`DeviceDetector` trait objects in isolation, with no orchestration function or HTTP
surface yet wiring them into an externally observable capability. See
`TASKS_PHASE004.md`'s Phase Acceptance Criteria for the full test-suite proof. The
live, externally observable hardware-detection capability is proved in Phase 5.
```
