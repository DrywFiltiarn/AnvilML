# Plan Report: P23-B2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P23-B2                                      |
| Phase       | 23 — ZiT VAE Arch Module                    |
| Description | worker/nodes/arch/vae/zit_vae.py: can_handle() + dispatch registration |
| Depends on  | P23-B1                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-07-16T22:00:00Z                        |
| Attempt     | 1                                           |

## Objective

Wire the ZiT VAE architecture module (`worker/nodes/arch/vae/zit_vae.py`) into the VAE family dispatcher so that `LoadVae` can later route requests to it. This task implements `can_handle(key) -> bool` (the scope P23-B1 deferred) and appends `zit_vae` to `arch/vae/__init__.py`'s `_REGISTERED_MODULES` list — giving the dispatcher its first real entry.

## Scope

### In Scope
- Add `can_handle(key: str) -> bool` to `worker/nodes/arch/vae/zit_vae.py` — returns `True` when `key == ARCH` (i.e. `"zit_vae"`).
- Append `zit_vae` module to `worker/nodes/arch/vae/__init__.py`'s `_REGISTERED_MODULES` list (same pattern as `diffusion/__init__.py` line 20–23).
- Add 3 new tests to `worker/tests/test_arch_vae_zit.py`:
  - `test_can_handle_matches_zit_vae_key` — `can_handle("zit_vae")` returns `True`.
  - `test_can_handle_rejects_unrelated_key` — `can_handle("flux2_vae")` returns `False`.
  - `test_get_module_returns_zit_vae_for_matching_key` — `get_module("zit_vae")` returns the `zit_vae` module.
- Total test count in `test_arch_vae_zit.py` reaches >= 7 (5 existing + 3 new).

### Out of Scope
None. The `defers_to` field is empty (`[]`), and this task implements its full scope. No stubs, no deferred functionality.

## Existing Codebase Assessment

**(a) What already exists:**
- `worker/nodes/arch/vae/zit_vae.py` (created by P23-B1) exports `ARCH = "zit_vae"` and `_infer_hyperparams()` — the shape inference step 1 of the four-step loading contract. It does NOT yet have `can_handle()`.
- `worker/nodes/arch/vae/__init__.py` (created by P10-B2) defines the dispatcher with an empty `_REGISTERED_MODULES` list and `get_module(key)` scan logic. It has zero registered modules — no VAE modules have been wired in yet.
- `worker/nodes/arch/diffusion/__init__.py` already registers `zit` (line 20–23) as its first module, providing the exact pattern to follow.
- `worker/nodes/arch/diffusion/zit.py` has `can_handle(key: str) -> bool` at line 755–769 — a one-liner returning `key == ARCH`. This is the pattern to replicate.

**(b) Established patterns:**
- `can_handle()` is a module-level function (not a class method), takes `key: Any` (typed as `str` in practice), returns `bool`.
- Dispatch registration uses eager top-level import + append: `from worker.nodes.arch.vae import zit_vae` followed by `_REGISTERED_MODULES.append(zit_vae)`.
- The `ARCH` constant is the canonical identifier, shared between `_infer_hyperparams()`'s metadata path and `can_handle()`.

**(c) Gaps between design doc and current source:**
- The VAE `__init__.py` dispatcher has no registered modules yet, while the diffusion `__init__.py` already has one. This task closes that gap for VAE.
- No external dependencies are introduced — only `types.ModuleType` (already imported in `__init__.py`) and existing `zit_vae` module.

## Resolved Dependencies

None. This task only imports existing modules within the project. No new crates or packages are introduced.

| Type   | Name    | Version verified | MCP source | Feature flags confirmed |
|--------|---------|-----------------|------------|------------------------|
| (none) | —       | —               | —          | —                      |

## Approach

1. **Add `can_handle()` to `zit_vae.py`.** Append the following function at module scope (after `_infer_hyperparams()`, following the same placement as `zit.py` line 755):

   ```python
   def can_handle(key: str) -> bool:
       """Confirm this module handles the given architecture key.

       The dispatcher passes the architecture string (from safetensors
       metadata or path fallback) as *key*. This function returns True
       only when the key matches this module's canonical identifier.

       Args:
           key: Architecture string to check, e.g. ``"zit_vae"`` or
               ``"flux2_vae"``.

       Returns:
           ``True`` if *key* equals ``"zit_vae"``, ``False`` otherwise.
       """
       return key == ARCH
   ```

   Rationale: This is the exact same pattern as `zit.py`'s `can_handle()` (line 755–769) — a one-liner comparing the incoming key against the module's `ARCH` constant. The VAE family uses the metadata-or-path-derived dispatch pattern (§10.4), distinct from CLIP's `clip_type` string dispatch.

2. **Register `zit_vae` in `arch/vae/__init__.py`.** Replace the empty `_REGISTERED_MODULES` list with an import and append, mirroring `diffusion/__init__.py` lines 20–23:

   ```python
   from worker.nodes.arch.vae import zit_vae

   _REGISTERED_MODULES: list[ModuleType] = []
   _REGISTERED_MODULES.append(zit_vae)
   ```

   Rationale: The diffusion family uses this exact pattern (import at module level, append to list). The eager import is safe because `zit_vae.py` guards all torch imports behind `try/except ImportError` — it is importable in mock-mode collection (ANVILML_DESIGN.md §11.2 import guard requirement). The existing `_infer_hyperparams()` in `zit_vae.py` already uses `framework="np"` which avoids requiring torch.

3. **Add 3 new tests to `test_arch_vae_zit.py`.** After the existing 5 tests:

   - `test_can_handle_matches_zit_vae_key`: Import `can_handle` from `zit_vae`, call `can_handle("zit_vae")`, assert `True`.
   - `test_can_handle_rejects_unrelated_key`: Call `can_handle("flux2_vae")`, assert `False`.
   - `test_get_module_returns_zit_vae_for_matching_key`: Import `get_module` from `arch.vae`, call `get_module("zit_vae")`, assert the returned module is `zit_vae` (check `module.__name__ == "worker.nodes.arch.vae.zit_vae"`).

   These tests verify the dispatch mechanism works end-to-end: `can_handle()` matches the P23-A1 fixture's arch key, rejects an unrelated key, and `get_module()` returns the correct module.

## Public API Surface

New public items:

| Path | Item | Signature |
|------|------|-----------|
| `worker.nodes.arch.vae.zit_vae` | `can_handle` | `def can_handle(key: str) -> bool` |

No changes to existing public items. The `_REGISTERED_MODULES` list in `arch/vae/__init__.py` is internal (private prefix), not part of the public API.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/arch/vae/zit_vae.py` | Add `can_handle(key: str) -> bool` function |
| MODIFY | `worker/nodes/arch/vae/__init__.py` | Import `zit_vae` module and append to `_REGISTERED_MODULES` |
| MODIFY | `worker/tests/test_arch_vae_zit.py` | Add 3 new tests: `test_can_handle_matches_zit_vae_key`, `test_can_handle_rejects_unrelated_key`, `test_get_module_returns_zit_vae_for_matching_key` |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `worker/tests/test_arch_vae_zit.py` | `test_can_handle_matches_zit_vae_key` | `can_handle("zit_vae")` returns `True` — dispatch matches the P23-A1 fixture's arch key | `python -m pytest worker/tests/test_arch_vae_zit.py -v` exits 0 |
| `worker/tests/test_arch_vae_zit.py` | `test_can_handle_rejects_unrelated_key` | `can_handle("flux2_vae")` returns `False` — dispatch rejects unrelated architecture keys | `python -m pytest worker/tests/test_arch_vae_zit.py -v` exits 0 |
| `worker/tests/test_arch_vae_zit.py` | `test_get_module_returns_zit_vae_for_matching_key` | `get_module("zit_vae")` returns the `zit_vae` module — end-to-end dispatch works after registration | `python -m pytest worker/tests/test_arch_vae_zit.py -v` exits 0 |

Total test count in `test_arch_vae_zit.py`: 5 existing + 3 new = 8 (>= 7 required).

## CI Impact

No CI changes required. The modified files are existing Python source and test files already covered by the `worker-linux-mock`, `worker-linux-real`, `worker-windows-mock`, and `worker-windows-real` CI jobs. The new tests are mock-compatible (they only test `can_handle()` and `get_module()` which don't require torch), so they run in both mock and real CI jobs.

## Platform Considerations

None identified. The Windows cross-check in ENVIRONMENT.md §7 is sufficient. `can_handle()` and `_REGISTERED_MODULES.append()` are platform-neutral Python operations with no `#[cfg]` or path-separator handling.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Importing `zit_vae` in `arch/vae/__init__.py` triggers a top-level torch import at collection time, breaking mock-mode test collection (worker-linux-mock CI job installs `requirements/base.txt` only, no torch) | Low | High | `zit_vae.py` already guards all torch imports behind `try/except ImportError` (per §11.2 import guard requirement). `_infer_hyperparams()` uses `framework="np"` which never requires torch. Verified by reading the source. |
| The fixture's `arch` key is `"zit_vae"` but `can_handle()` compares against `ARCH = "zit_vae"` — a string mismatch from a future fixture change would silently fail dispatch | Low | Medium | The `test_can_handle_matches_zit_vae_key` test uses the literal string `"zit_vae"` which matches both the fixture's metadata and the `ARCH` constant. If a future fixture changes the arch key, the `_infer_hyperparams` tests will fail first, catching the inconsistency before dispatch breaks. |

## Acceptance Criteria

- [ ] `python -m pytest worker/tests/test_arch_vae_zit.py -v` exits 0 (>= 7 total tests)
- [ ] `worker/.venv/bin/python -m py_compile worker/nodes/arch/vae/zit_vae.py worker/nodes/arch/vae/__init__.py worker/tests/test_arch_vae_zit.py` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_vae_zit.py -v -m "not real_mode"` exits 0 (mock-compatible tests pass)
