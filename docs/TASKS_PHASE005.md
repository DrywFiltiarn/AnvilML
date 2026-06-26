# Tasks: Phase 5 — Hardware Detection: Orchestration

**Phase:** 5
**Name:** Hardware Detection: Orchestration
**Project(s):** anvilml
**Status:** Draft
**Depends on phases:** 1, 2, 3, 4

---

## Overview

This phase wires Phase 4's five independent detectors into the single priority-ordered
decision function, `detect_all_devices()`, that the rest of the system actually calls.
It implements `ANVILML_DESIGN.md §6.4`'s exact six-step priority chain — config
override, then mock (if compiled), then Vulkan, then a platform-specific fallback,
then the unconditional CPU device — and assembles the final `HardwareInfo` with a
unioned `inference_caps`. It then exposes this capability through a small CLI
subcommand on the `backend` binary, giving the phase a real, externally-observable
Runnable Proof without prematurely building the full `/v1/system` HTTP handler (which
needs `AppState` fields — `SqlitePool`, `JobScheduler`, `WorkerPool`, `ModelRegistry`
— that don't exist until much later phases).

This phase exists right after the individual detectors because orchestrating five
independent strategies into one decision function is a distinct concern from building
each strategy correctly in isolation — Phase 4 proved each detector works on its own;
this phase proves they compose correctly, with the override always winning, mock and
real never both queried in the same build, and the CPU device always present as the
final guarantee.

At the start of this phase, `anvilml-hardware` has five working detectors but no
function that chooses among them. At the end, `detect_all_devices()` is the one entry
point any later crate (the scheduler, the server) will call to get a `HardwareInfo`
snapshot, and a human can run `anvilml hw-probe` and see real (or mock) detected
hardware printed as JSON. This closes out the Hardware Detection subsystem entirely —
Phase 6 moves on to Model Registry & Artifacts.

---

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | Orchestration & proof | P5-A1 … P5-A6 | `detect_all_devices()`'s priority chain built up in three sequential steps, a `lib.rs` cleanup, the `hw-probe` CLI subcommand, and the phase's Runnable Proof |

A single group is used because every task contributes to the same function
(`detect_all_devices()`) or its immediate consumer (the CLI subcommand) — there is no
meaningful subsystem split within "wire the detectors together and expose the
result."

---

## Prerequisites

All five `DeviceDetector` implementations (`CpuDetector`, `MockDetector`,
`VulkanDetector`, `DxgiDetector`, `SysfsPciDetector`) must exist and pass their own
tests per Phase 4. `ServerConfig.hardware_override` must exist per Phase 2 (P2-A3).

---

## Interfaces and Contracts

| Contract document | Relevant to tasks | What must match |
|--------------------|--------------------|------------------|
| `ANVILML_DESIGN.md §6.4` | P5-A1, P5-A2, P5-A3 | The exact six-step priority order — override, mock, Vulkan, platform fallback, CPU |
| `ANVILML_DESIGN.md §6.3` | P5-A4 | Module layout / re-export shape of `lib.rs` |

---

## Task Descriptions

### Group A — Orchestration & proof

#### P5-A1: anvilml-hardware: hardware_override config short-circuit

**Goal:** Implement the highest-priority step of the detection chain — an explicit
config override always wins, unconditionally, before any detector runs — establishing
`detect_all_devices()`'s signature for every later step to extend.

**Files to create or modify:**
- `crates/anvilml-hardware/src/detect.rs` — `detect_all_devices()`, override-only
  logic.

**Key implementation notes:**
- Signature is `pub async fn detect_all_devices(cfg: &ServerConfig) ->
  Result<HardwareInfo, AnvilError>` — no `SqlitePool` parameter yet, since
  `anvilml-registry`'s `DeviceCapabilityStore` doesn't exist until a later phase;
  that parameter is added when that dependency actually exists, not speculatively
  now.
- If `cfg.hardware_override` is `Some`, synthesize exactly one `GpuDevice` from its
  fields with `enumeration_source: EnumerationSource::Override`,
  `capabilities_source: CapabilitySource::Fallback`, and return immediately —
  skipping every other detector entirely, including the CPU fallback.
- This task's scope is strictly the override short-circuit. The mock/Vulkan/
  fallback/CPU chain is explicitly deferred to P5-A2.

**Acceptance criterion:**
```bash
cargo test -p anvilml-hardware --test detect_tests
# -> >=2 tests, exits 0
```

#### P5-A2: anvilml-hardware: mock-vs-real branch + Vulkan fallback chain

**Goal:** Implement the middle of the priority chain — choosing between the mock
detector and the real-hardware detection path, then trying Vulkan with a
platform-specific fallback if it comes back empty.

**Files to create or modify:**
- `crates/anvilml-hardware/src/detect.rs` — extends `detect_all_devices()` past
  P5-A1's override check.

**Key implementation notes:**
- If the `mock-hardware` feature is compiled in, use `MockDetector` exclusively —
  mock and real detection are mutually exclusive **per build**, never both queried
  in the same compiled binary, per `ARCHITECTURE.md §5`'s feature-forwarding
  contract.
- If `mock-hardware` is not compiled, try `VulkanDetector` first; if it returns an
  empty `Vec`, try the platform fallback (`DxgiDetector` on Windows,
  `SysfsPciDetector` on Linux, selected via `cfg`).
- This task returns the resulting `Vec<GpuDevice>` from this branch only — it does
  **not** append the CPU device or construct the final `HardwareInfo`; that
  assembly is P5-A3's explicitly deferred scope.

**Acceptance criterion:**
```bash
cargo test -p anvilml-hardware --features mock-hardware --test detect_tests
# -> >=4 tests, exits 0
```

#### P5-A3: anvilml-hardware: CPU-append + HardwareInfo assembly

**Goal:** Complete `detect_all_devices()` by appending the unconditional CPU
fallback device and assembling the final `HardwareInfo`, guaranteeing the function
never returns an empty device list.

**Files to create or modify:**
- `crates/anvilml-hardware/src/detect.rs` — finishes `detect_all_devices()`.

**Key implementation notes:**
- `CpuDetector`'s single device is always appended last, regardless of what
  P5-A2's branch produced — this is the concrete mechanism behind
  `ANVILML_DESIGN.md §6.2`'s "result is always `Ok(HardwareInfo)` with at least one
  device" guarantee.
- `inference_caps` on the final `HardwareInfo` is the **union** (any-field-true) of
  every individual `GpuDevice.caps` — not just the first device's, and not an
  intersection.
- `HostInfo` is populated minimally here (`hostname` via `std::env`, `os` via
  `std::env::consts::OS`) — no richer host probing is in scope for this task.

**Acceptance criterion:**
```bash
cargo test -p anvilml-hardware --features mock-hardware --test detect_tests
# -> >=9 tests total in the file, exits 0
```

#### P5-A4: anvilml-hardware: lib.rs re-export detect_all_devices, 80-line check

**Goal:** Finalize the crate's public surface by re-exporting
`detect_all_devices()` and confirming every module from Phase 4 and this phase is
correctly declared with its `cfg`/feature gate.

**Files to create or modify:**
- `crates/anvilml-hardware/src/lib.rs` — adds `pub use detect::detect_all_devices;`.

**Key implementation notes:**
- This is a re-export and gate-verification pass only — no implementation logic
  changes. Confirm `detect`, `cpu`, `mock`, `vulkan`, `dxgi`, `sysfs` are all present
  with the correct gates per `ANVILML_DESIGN.md §6.3`'s module layout table.
- Stays under the 80-line hard cap, same as every other crate's `lib.rs`.

**Acceptance criterion:**
```bash
wc -l crates/anvilml-hardware/src/lib.rs
# -> <=80
cargo build -p anvilml-hardware --features mock-hardware
cargo build -p anvilml-hardware
# -> both exit 0
```

#### P5-A5: backend: hw-probe CLI subcommand prints HardwareInfo JSON

**Goal:** Expose `detect_all_devices()` through a real CLI subcommand on the
`anvilml` binary, giving this phase's capability an externally observable surface
without prematurely building the full HTTP server's `AppState`.

**Files to create or modify:**
- `backend/src/cli.rs` — adds a `Commands` enum with one variant, `HwProbe`;
  restructures `Cli` to hold an optional `#[command(subcommand)] command:
  Option<Commands>` (default `None` keeps today's "run the server" behavior).
- `backend/src/main.rs` — branches on `Commands::HwProbe`: loads `ServerConfig` the
  normal way, calls `detect_all_devices`, prints pretty JSON, exits — no socket
  bind in this branch.

**Key implementation notes:**
- This is tagged `breaking` because `Cli`'s shape changes (a new optional
  subcommand field) — confirm nothing outside this task already constructs `Cli`
  directly with the old field set.
- Deliberately not a new HTTP route: `/v1/system` (the eventual real endpoint, per
  `ANVILML_DESIGN.md §13.4`) needs `AppState` fields that don't exist until the
  scheduler, worker pool, and registry phases land — building a premature handler
  now would mean either a fake `AppState` or non-compliant scope creep into this
  phase.

**Acceptance criterion:**
```bash
cargo build -p anvilml
cargo test --workspace --features mock-hardware
# -> both exit 0
```

#### P5-A6: Runnable Proof: hw-probe CLI prints valid HardwareInfo JSON

**Goal:** Produce this phase's Runnable Proof — confirming the built binary's new
subcommand prints real, valid, well-formed hardware detection output — and record
the transcript.

**Files to create or modify:**
- None. This task runs the already-built binary; see Acceptance Criterion.

**Key implementation notes:**
- Run with `ANVILML_MOCK_DEVICE_TYPE=cuda` and `ANVILML_MOCK_VRAM_MIB=24576` set,
  so the proof demonstrates both the mock detector's output and the CPU fallback's
  unconditional presence in the same run — two devices, not one.
- Record the literal terminal JSON output in the implementation report; this is
  what `docs/RUNNABLE_PROOF.md`'s Phase 5 entry references.

**Acceptance criterion:**
```bash
cargo build --release -p anvilml --features mock-hardware
ANVILML_MOCK_DEVICE_TYPE=cuda ANVILML_MOCK_VRAM_MIB=24576 ./target/release/anvilml hw-probe \
  | python3 -c "import sys,json; d=json.load(sys.stdin); assert len(d['gpus'])>=2; assert any(g['device_type']=='cpu' for g in d['gpus']); assert any(g['device_type']=='cuda' for g in d['gpus'])"
# -> exits 0
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

# Runnable Proof (manual):
cargo build --release -p anvilml --features mock-hardware
ANVILML_MOCK_DEVICE_TYPE=cuda ANVILML_MOCK_VRAM_MIB=24576 ./target/release/anvilml hw-probe \
  | python3 -c "import sys,json; d=json.load(sys.stdin); assert len(d['gpus'])>=2; assert any(g['device_type']=='cpu' for g in d['gpus']); assert any(g['device_type']=='cuda' for g in d['gpus'])"
# -> exits 0
```

---

## Known Constraints and Gotchas

- The override step (P5-A1) must short-circuit **before** the mock/real branch
  check (P5-A2) — an override present in config wins even when `mock-hardware` is
  also compiled in. Getting this order backwards would make the override silently
  unreachable in any mock-feature build, which is exactly the build CI runs most.
- Mock and real detection are mutually exclusive **at compile time** via the
  `mock-hardware` feature flag — `detect_all_devices()` never queries both in the
  same binary. There is no runtime toggle between them.
- `detect_all_devices()` deliberately does not yet take a `SqlitePool` parameter —
  that arrives only when `anvilml-registry`'s `DeviceCapabilityStore` exists to fill
  it. Don't add the parameter speculatively in this phase.
- The `hw-probe` CLI subcommand is intentionally not an HTTP endpoint — `/v1/system`
  is deferred until `AppState`'s other fields exist in a later phase.

---

## docs/RUNNABLE_PROOF.md entry

```markdown
## Phase 5 — Hardware Detection: Orchestration

**Capability proved:** The `anvilml hw-probe` CLI subcommand calls the real
`detect_all_devices()` priority chain and prints valid `HardwareInfo` JSON,
including the guaranteed CPU fallback device and (when mock env vars are set) the
mock GPU device.

\`\`\`bash
# Runnable Proof (manual):
cargo build --release -p anvilml --features mock-hardware
ANVILML_MOCK_DEVICE_TYPE=cuda ANVILML_MOCK_VRAM_MIB=24576 ./target/release/anvilml hw-probe \
  | python3 -c "import sys,json; d=json.load(sys.stdin); assert len(d['gpus'])>=2; assert any(g['device_type']=='cpu' for g in d['gpus']); assert any(g['device_type']=='cuda' for g in d['gpus'])"
# -> exits 0
\`\`\`
```
