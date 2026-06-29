# Plan Report: P5-A6

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P5-A6                                       |
| Phase       | 005 — Hardware Detection: Orchestration     |
| Description | Runnable Proof: hw-probe CLI prints valid HardwareInfo JSON |
| Depends on  | P5-A5                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-06-29T13:30:00Z                        |
| Attempt     | 1                                           |

## Objective

Produce Phase 5's Runnable Proof by building the `anvilml` binary with the `mock-hardware` feature flag and running its `hw-probe` CLI subcommand (implemented in P5-A5) under mock hardware environment variables. Confirm the piped stdout parses as valid JSON containing at least two GPU devices — one with `device_type: "cuda"` (from `MockDetector`, driven by `ANVILML_MOCK_DEVICE_TYPE=cuda`) and one with `device_type: "cpu"` (the unconditional `CpuDetector` fallback, always appended last per `ANVILML_DESIGN.md §6.4 step 6`). Record the literal terminal output for the implementation report.

## Scope

### In Scope
- Build the release binary: `cargo build --release -p anvilml --features mock-hardware`
- Run the binary's `hw-probe` subcommand under mock env vars: `ANVILML_MOCK_DEVICE_TYPE=cuda ANVILML_MOCK_VRAM_MIB=24576 ./target/release/anvilml hw-probe`
- Pipe the JSON output through a Python one-liner that asserts: (a) at least 2 GPUs in the array, (b) at least one has `device_type == "cpu"`, (c) at least one has `device_type == "cuda"`
- Record the literal terminal output (the full JSON and the assertion result) in the implementation report

### Out of Scope
None. `defers_to (from JSON): []` — this task has no deferred scope. No source files are created or modified.

## Existing Codebase Assessment

Phase 5's five preceding tasks have completed all source work:

- **P5-A1** implemented `detect_all_devices()`'s override short-circuit in `crates/anvilml-hardware/src/detect.rs`, returning immediately when `cfg.hardware_override` is `Some`.
- **P5-A2** extended `detect_all_devices()` with the mock-vs-real branch (`#[cfg(feature = "mock-hardware")]` selects `MockDetector`; otherwise `VulkanDetector` with platform fallback).
- **P5-A3** completed `detect_all_devices()` by appending the unconditional `CpuDetector` fallback and assembling the final `HardwareInfo` with host info and unioned `InferenceCaps`.
- **P5-A4** finalized the crate's `lib.rs` re-exports (under 80 lines).
- **P5-A5** added the `Commands::HwProbe` subcommand to `backend/src/cli.rs` and wired `main.rs` to call `detect_all_devices()`, print pretty JSON, and exit — no socket binding in this branch.

The established patterns to follow (for reference by later phases): `detect_all_devices()` always returns `Ok(HardwareInfo)` with at least one device; the CPU fallback is always the last element in `gpus`; `InferenceCaps` is the field-wise OR union of all per-device caps; env vars are process-global and tests must use `#[serial]` with capture-and-restore.

No gap between the design doc and current source affects this task — the CLI subcommand and detection chain are fully implemented and tested.

## Resolved Dependencies

None. This task introduces no new dependencies. It runs the already-built binary produced by P5-A5's changes.

## Approach

1. **Build the release binary.** Run `cargo build --release -p anvilml --features mock-hardware` from the repo root. This compiles the `anvilml` binary with the `mock-hardware` feature forwarded through `anvilml-scheduler → anvilml-hardware`. The `hw-probe` subcommand is already wired in `main.rs` (P5-A5).

2. **Run `hw-probe` under mock env vars.** Execute:
   ```bash
   ANVILML_MOCK_DEVICE_TYPE=cuda ANVILML_MOCK_VRAM_MIB=24576 ./target/release/anvilml hw-probe
   ```
   This triggers the following code path:
   - `main.rs` parses the `hw-probe` subcommand.
   - `config_load::load()` loads `ServerConfig` (no `hardware_override` set in this env).
   - `detect_all_devices(&config)` enters the `#[cfg(feature = "mock-hardware")]` branch.
   - `MockDetector.detect()` reads `ANVILML_MOCK_DEVICE_TYPE=cuda` → `DeviceType::Cuda`, `ANVILML_MOCK_VRAM_MIB=24576` → 24576 MiB, and returns one `GpuDevice` with `device_type: "cuda"`, `enumeration_source: EnumerationSource::Mock`.
   - `CpuDetector.detect()` appends one `GpuDevice` with `device_type: "cpu"`, `enumeration_source: EnumerationSource::Cpu`.
   - The result is a `HardwareInfo` with 2 GPUs.
   - `main.rs` serialises to pretty JSON via `serde_json::to_string_pretty` and prints to stdout.

3. **Pipe through Python assertion.** Pipe the output to:
   ```bash
   python3 -c "import sys,json; d=json.load(sys.stdin); assert len(d['gpus'])>=2; assert any(g['device_type']=='cpu' for g in d['gpus']); assert any(g['device_type']=='cuda' for g in d['gpus'])"
   ```
   This validates: (a) the JSON is parseable, (b) at least 2 GPU entries exist, (c) one has `device_type == "cpu"`, (d) one has `device_type == "cuda"`.

4. **Record the output.** Capture the full terminal output (the pretty-printed JSON and the assertion result — no stderr output expected if assertions pass) in the implementation report's transcript section.

5. **Confirm exit code 0.** The acceptance criterion is that the combined `cargo build && ... | python3 -c ...` pipeline exits 0. If the assertions fail, the python process exits non-zero and the pipeline fails.

### Phase Deliverable Audit

**§9a procedure (defers_to check):**
- P5-A1 (`defers_to: ["P5-A2"]`): P5-A2 is completed. P5-A1's source (detect.rs) implements only the override short-circuit — no stub code remains. The defers_to was scope separation (P5-A1 did not implement the mock/Vulkan/fallback/CPU chain), not a stub requiring a `// defers_to:` marker. No marker is needed because there is no incomplete code at the P5-A1 boundary.
- P5-A2 (`defers_to: ["P5-A3"]`): P5-A3 is completed. P5-A2's source implements the mock-vs-real branch and Vulkan fallback — no stub code remains. Same reasoning as above.

**§9a.1 Unmarked-stub sweep:**
```bash
grep -rn "NotImplementedError\|unimplemented!\|todo!\|# TODO\|// TODO" \
  crates/anvilml-hardware/src/detect.rs \
  crates/anvilml-hardware/src/lib.rs \
  backend/src/cli.rs \
  backend/src/main.rs
```
Result: 0 findings (no matches).

**§9a.2 Dual-mode parity-marker sweep:**
The parity marker convention (`REAL_PATH_VERIFIED`/`MOCK_PATH_VERIFIED`) applies to node `execute()` and arch module `load()`/`sample()`/`decode()`/`compute_latent_shape()` in `worker/nodes/` per `ANVILML_DESIGN.md §10.6`. Phase 5 touches no node or arch-module code — it only modifies hardware detection files (`detect.rs`, `lib.rs`) and CLI files (`cli.rs`, `main.rs`). These files do not define any functions in scope of the parity convention. The `grep -L` findings (files lacking markers) are therefore not findings.

## Public API Surface

None. This task does not introduce or modify any public API items. The `detect_all_devices()` function, `Commands::HwProbe` subcommand, and `HardwareInfo` type were all introduced in prior tasks (P5-A1 through P5-A5).

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| (none) | — | No source files created or modified. This task runs the already-built binary. |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| (manual) | hw_probe_mock_output_valid_json | The `hw-probe` CLI subcommand prints parseable JSON with ≥2 GPUs including both `"cuda"` and `"cpu"` device types | P5-A1 through P5-A5 completed; `mock-hardware` feature compiled | `ANVILML_MOCK_DEVICE_TYPE=cuda`, `ANVILML_MOCK_VRAM_MIB=24576` | Exit 0; no assertion errors | `cargo build --release -p anvilml --features mock-hardware && ANVILML_MOCK_DEVICE_TYPE=cuda ANVILML_MOCK_VRAM_MIB=24576 ./target/release/anvilml hw-probe \| python3 -c "import sys,json; d=json.load(sys.stdin); assert len(d['gpus'])>=2; assert any(g['device_type']=='cpu' for g in d['gpus']); assert any(g['device_type']=='cuda' for g in d['gpus'])"` exits 0 |

## CI Impact

No CI changes required. This task runs no new tests, modifies no source files, and adds no CI configuration. The existing CI jobs (rust-linux, rust-windows) already build with `--features mock-hardware` and run the full test suite, which exercises `detect_all_devices()` through the `detect_tests` integration tests.

## Platform Considerations

None identified. The Windows cross-check in ENVIRONMENT.md §7 is sufficient. The `hw-probe` subcommand's code path is platform-neutral: `MockDetector` reads env vars (identical on all platforms), `CpuDetector` returns a synthesized device (no platform-specific I/O), and `serde_json` serialisation is cross-platform. The `main.rs` branch that handles `HwProbe` has no `#[cfg(...)]` guards.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The `hw-probe` subcommand was not wired correctly in P5-A5's `main.rs` — e.g., the match arm for `Commands::HwProbe` is missing or the `detect_all_devices` call panics. | Low | High | The acceptance command will fail with a non-zero exit code and panic backtrace. The ACT agent captures this output in the implementation report. If it fails, verify P5-A5's wiring in `main.rs` lines 50-66. |
| `serde_json::to_string_pretty` panics on `HardwareInfo` because a type lacks `Serialize` derive. | Low | Medium | This would be a compilation error (not a runtime panic) since `HardwareInfo` derives `Serialize` via `anvilml_core`. If it fails, check that `anvilml_core`'s `HardwareInfo` has the `Serialize` derive — confirmed present in `hardware.rs`. |
| `MockDetector` reads env vars but `ANVILML_MOCK_DEVICE_TYPE` is not forwarded to the subprocess. | Low | Medium | The mock env vars are read directly by `MockDetector` from the process environment (not forwarded through subprocess injection). They are set in the shell before running the binary, so they are available to the Rust process. No forwarding needed. |

## Acceptance Criteria

- [ ] `cargo build --release -p anvilml --features mock-hardware && ANVILML_MOCK_DEVICE_TYPE=cuda ANVILML_MOCK_VRAM_MIB=24576 ./target/release/anvilml hw-probe | python3 -c "import sys,json; d=json.load(sys.stdin); assert len(d['gpus'])>=2; assert any(g['device_type']=='cpu' for g in d['gpus']); assert any(g['device_type']=='cuda' for g in d['gpus'])"` exits 0
