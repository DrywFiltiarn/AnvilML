# Implementation Report: P20-C1

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P20-C1                          |
| Phase         | 20 — ZiT Diffusion Arch Module: Shape Inference & Construction |
| Description   | worker/nodes/arch/diffusion/zit.py: meta-device construction |
| Implemented   | 2026-07-13T18:00:00Z           |
| Status        | COMPLETE                        |

## Summary

Implemented the `ZiTModel(nn.Module)` class and `load()` function in `worker/nodes/arch/diffusion/zit.py`. The `ZiTModel` class assembles the ZiT diffusion transformer architecture using `torch.nn` primitives (Linear, LayerNorm, MultiheadAttention, Sequential). The `load()` function opens a checkpoint header, calls `_infer_hyperparams()` (from P20-B1) to get hyperparameters, constructs `ZiTModel` on `torch.device("meta")`, sets `.arch = "zit"`, and returns the module. No real GPU/CPU memory is allocated during construction. Dual-mode parity markers (`REAL_PATH_VERIFIED` / `MOCK_PATH_VERIFIED`) are placed on the `load()` function. Five new tests were added to `worker/tests/test_arch_zit.py` (total 12 tests, up from 7).

## Resolved Dependencies

| Type   | Name      | Version resolved | Source         |
|--------|-----------|------------------|----------------|
| python | torch     | (project venv)   | project lockfile |
| python | safetensors| (project venv)  | project lockfile |

No new dependencies introduced. This task uses only `torch` (already in the project venv), `torch.nn.Module` (stdlib), and `torch.nn` primitives (Linear, LayerNorm, MultiheadAttention, Sequential, GELU).

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/arch/diffusion/zit.py` | Added `ZiTModel(nn.Module)` class, `load()` function, dual-mode parity markers, updated module docstring, added `import torch` and `import torch.nn as nn` |
| MODIFY | `worker/tests/test_arch_zit.py` | Added 5 new tests: `test_load_meta_construction_real`, `test_load_meta_construction_mock`, `test_load_meta_device_zero_real_memory`, `test_load_meta_construction_no_metadata_variant`, `test_load_raises_invalid_hyperparams` |
| MODIFY | `docs/TESTS.md` | Added 5 entries for new tests |

## Commit Log

```
 .forge/reports/P20-C1_plan.md      | 247 +++++++++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md       |   6 +-
 .forge/state/state.json            |  13 +-
 docs/TESTS.md                      |  60 +++++++++
 worker/nodes/arch/diffusion/zit.py | 158 ++++++++++++++++++++++--
 worker/tests/test_arch_zit.py      | 118 +++++++++++++++++-
 6 files changed, 584 insertions(+), 18 deletions(-)
```

## Test Results

### Rust tests (full workspace, --features mock-hardware)

All Rust tests passed. Key results:
- backend: 18 tests passed
- anvilml-artifacts: 9 tests passed
- anvilml-core: 53 tests passed
- anvilml-hardware: 44 tests passed
- anvilml-ipc: 33 tests passed
- anvilml-registry: 14 tests passed
- anvilml-scheduler: 64 tests passed
- anvilml-server: 48 tests passed
- anvilml-worker: 67 tests passed
- Doc-tests: 3 tests passed

### Python mock-mode tests (104 passed, 31 deselected)

All 104 mock-mode tests passed, including all 5 new tests:
- `test_load_meta_construction_real` — PASSED
- `test_load_meta_construction_mock` — PASSED
- `test_load_meta_device_zero_real_memory` — PASSED
- `test_load_meta_construction_no_metadata_variant` — PASSED
- `test_load_raises_invalid_hyperparams` — PASSED

### Python real-mode tests (31 passed)

All 31 real-mode tests passed.

## Format Gate

```
cargo fmt --all -- --check
```
Exits 0 — no formatting drift detected.

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
# → Finished `dev` profile [unoptimized + debuginfo] target(s) in 7.75s

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
# → Finished `dev` profile [unoptimized + debuginfo] target(s) in 60s

# 3. Real-hardware Linux
cargo check --bin anvilml
# → Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 03s

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
# → Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 06s
```

All four platform cross-checks pass.

## Project Gates

### Gate 1 — Config Surface Sync
```
cargo test -p anvilml --features mock-hardware -- config_reference
# → test tests::config_reference_matches_defaults ... ok
# → test result: ok. 1 passed; 0 failed
```

### Gate 4 — Mock/Real Parity Markers
```
grep -rn "REAL_PATH_VERIFIED:\|MOCK_PATH_VERIFIED:" worker/nodes/arch/diffusion/zit.py
# → 142:# REAL_PATH_VERIFIED: worker/tests/test_arch_zit.py::test_load_meta_construction_real
# → 143:# MOCK_PATH_VERIFIED: worker/tests/test_arch_zit.py::test_load_meta_construction_mock
```

Both markers present and name collectible tests:
- `test_load_meta_construction_real` — collected ✓
- `test_load_meta_construction_mock` — collected ✓

## Public API Delta

```
git diff HEAD -- worker/nodes/arch/diffusion/zit.py | grep '^+class \|^+def '
# → +class ZiTModel(nn.Module):
# → +def load(path: str) -> ZiTModel:
```

New public items:
- `class ZiTModel(nn.Module)` — module path: `worker.nodes.arch.diffusion.zit`
- `def load(path: str) -> ZiTModel` — module path: `worker.nodes.arch.diffusion.zit`
- `ZiTModel.arch: str` — attribute set to `"zit"` after construction

All match the plan's `## Public API Surface` table.

## Deviations from Plan

- **`test_load_meta_device_zero_real_memory` implementation detail:** The plan specified verifying `sum(p.numel() for p in model.parameters()) == 0` for zero real memory. However, PyTorch meta tensors report their logical `numel()` based on shape (not zero). The correct verification is that `param.device.type == "meta"` for all parameters, which IS the zero-memory guarantee. The test was adjusted to check device type instead.
- **Added `test_load_meta_construction_mock`:** The plan's `## Approach` Step 3 placed `MOCK_PATH_VERIFIED:` marker pointing to `worker/tests/test_arch_zit.py::test_load_meta_construction_mock`, but this test was not listed in the `## Tests` table. Per the dual-mode parity marker convention (ANVILML_DESIGN.md §10.6), both markers must name collectible tests. Added this test to satisfy Gate 4.
- **No version bump:** This task modifies only Python files, not any Rust crate source. No crate versions were bumped.

## Blockers

None.
