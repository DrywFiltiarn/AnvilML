# Implementation Report: P25-B2

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P25-B2                          |
| Phase         | 25 — Flux 2 Klein 4B Diffusion + Flux 2 VAE |
| Description   | worker/nodes/arch/diffusion/flux2klein.py: can_handle() + dispatch (4B) |
| Implemented   | 2026-07-22T12:00:00Z            |
| Status        | COMPLETE                          |

## Summary

Added `can_handle(key: str) -> bool` to `flux2klein.py` matching the exact pattern from `zit.py`, and registered the `flux2klein` module into `arch/diffusion/__init__.py`'s `_REGISTERED_MODULES` list. This gives the diffusion dispatcher its second real entry alongside `zit.py`, fulfilling ANVILML_DESIGN.md §20's confirmation point that adding a second diffusion architecture requires zero changes to the generic node layer. Four new tests were added to `test_arch_flux2klein.py` and one cross-check test to `test_arch_zit.py`. The existing `_clear_diffusion_registry` fixture in `test_arch_dispatch.py` was updated to restore both registered modules instead of just `zit`.

## Resolved Dependencies

None. This task introduces no new external dependencies.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/arch/diffusion/flux2klein.py` | Added `can_handle(key: str) -> bool` function; updated module docstring to mark can_handle as implemented |
| MODIFY | `worker/nodes/arch/diffusion/__init__.py` | Imported and registered `flux2klein` in `_REGISTERED_MODULES`; updated module docstring |
| MODIFY | `worker/tests/test_arch_flux2klein.py` | Added 4 new tests (can_handle primary match, can_handle rejects zit, get_module returns flux2klein, get_module returns zit) |
| MODIFY | `worker/tests/test_arch_zit.py` | Added 1 cross-check test (can_handle rejects flux2klein) |
| MODIFY | `worker/tests/test_arch_dispatch.py` | Updated `_clear_diffusion_registry` fixture to save/restore both registered modules instead of only zit |
| MODIFY | `docs/TESTS.md` | Added 5 new test entries for the 5 new tests |

## Commit Log

```
 .forge/state/CURRENT_TASK.md              |  6 ++--
 .forge/state/state.json                   | 13 +++----
 docs/TESTS.md                             | 60 +++++++++++++++++++++++++++++++
 worker/nodes/arch/diffusion/__init__.py   |  3 ++
 worker/nodes/arch/diffusion/flux2klein.py | 23 ++++++++++--
 worker/tests/test_arch_dispatch.py        | 13 +++----
 worker/tests/test_arch_flux2klein.py      | 59 ++++++++++++++++++++++++++++++
 worker/tests/test_arch_zit.py             | 12 +++++++
 8 files changed, 171 insertions(+), 18 deletions(-)
```

## Test Results

### Mock-mode (160 passed, 133 deselected)
```
worker/tests/test_arch_flux2klein.py::test_infer_hyperparams_regular_fixture PASSED
worker/tests/test_arch_flux2klein.py::test_infer_hyperparams_no_metadata_fixture PASSED
worker/tests/test_arch_flux2klein.py::test_infer_hyperparams_nonexistent_path_raises PASSED
worker/tests/test_arch_flux2klein.py::test_infer_hyperparams_truncated_header_raises PASSED
worker/tests/test_arch_flux2klein.py::test_can_handle_matches_flux2klein PASSED
worker/tests/test_arch_flux2klein.py::test_can_handle_rejects_zit_key PASSED
worker/tests/test_arch_flux2klein.py::test_get_module_returns_flux2klein_for_flux2klein_key PASSED
worker/tests/test_arch_flux2klein.py::test_get_module_returns_zit_for_zit_key PASSED
worker/tests/test_arch_zit.py::test_can_handle_rejects_flux2klein PASSED
(all 160 tests passed, including all pre-existing tests)
```

### Real-mode (133 passed, 160 deselected)
```
All 133 real-mode tests passed including zit load/sample tests, vae tests, clip tests,
e2e pipeline tests, node tests, and worker_main tests.
```

### Rust full test suite
```
All 350+ Rust tests passed across all crates (anvilml, anvilml-core, anvilml-hardware,
anvilml-ipc, anvilml-registry, anvilml-artifacts, anvilml-worker, anvilml-scheduler,
anvilml-server, anvilml-openapi).
```

## Format Gate

```
cargo fmt --all -- --check
(exit 0 — no drift)
```

## Platform Cross-Check

```
1. cargo check --workspace --features mock-hardware — Finished (0.32s)
2. cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu — Finished (55.11s)
3. cargo check --bin anvilml — Finished (5.82s)
4. cargo check --bin anvilml --target x86_64-pc-windows-gnu — Finished (6.05s)
```

## Project Gates

```
Gate 1 (Config Surface Sync): cargo test -p anvilml --features mock-hardware -- config_reference
  → config_reference_matches_defaults ... ok (1 passed)

Gate 2 (OpenAPI Drift): Not triggered — no handler signatures modified.

Gate 3 (Node Parity): Not triggered — no node types added/removed/renamed.

Gate 4 (Mock/Real Parity Markers): Not triggered — no load()/sample()/decode()/
  compute_latent_shape()/execute() functions added or modified.
```

## Public API Delta

```
git diff HEAD -- worker/nodes/arch/diffusion/flux2klein.py worker/nodes/arch/diffusion/__init__.py worker/tests/test_arch_flux2klein.py worker/tests/test_arch_zit.py | grep '^+def '
+def can_handle(key: str) -> bool:
```

New public item:
- `can_handle(key: str) -> bool` — `worker.nodes.arch.diffusion.flux2klein` — Returns True if key equals "flux2klein"

## Deviations from Plan

1. **`test_arch_dispatch.py` fixture update**: The `_clear_diffusion_registry` fixture was modified to save and restore all registered modules (`list(diffusion._REGISTERED_MODULES)`) instead of only `zit`. This was necessary because the fixture only restored `zit` after clearing, which caused `test_get_module_returns_flux2klein_for_flux2klein_key` (in a different test file) to fail when run after `test_arch_dispatch.py` tests. The fixture is autouse-scoped to `test_arch_dispatch.py`, but since pytest collects all tests before running any, the module-level registration state from `__init__.py` is shared across the entire test session. The fix ensures both `flux2klein` and `zit` are restored after each test in that file, preventing state leakage to other test files. This is a structural fix to the existing test infrastructure, not scope creep.

## Blockers

None.
