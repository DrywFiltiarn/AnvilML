# Plan Report: P25-B2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P25-B2                                       |
| Phase       | 25 — Flux 2 Klein 4B Diffusion + Flux 2 VAE  |
| Description | worker/nodes/arch/diffusion/flux2klein.py: can_handle() + dispatch (4B) |
| Depends on  | P25-B1                                       |
| Project     | anvilml                                      |
| Planned at  | 2026-07-22T10:30:00Z                         |
| Attempt     | 1                                            |

## Objective

Add `can_handle(key) -> bool` to `flux2klein.py` and register the module into the
diffusion dispatcher's `_REGISTERED_MODULES`, giving the dispatcher its **second**
real entry alongside `zit.py`. This is ANVILML_DESIGN.md §20's explicit confirmation
point: adding a second diffusion architecture requires **zero changes** to the generic
node layer. The acceptance state is `python -m pytest worker/tests/test_arch_flux2klein.py -v`
exiting 0 with >=7 total tests (4 from P25-B1 + >=4 new).

## Scope

### In Scope
- Add `can_handle(key: str) -> bool` to `worker/nodes/arch/diffusion/flux2klein.py`,
  matching the exact pattern from `zit.py`'s `can_handle()` (returns `key == ARCH`).
- Register `flux2klein` module into `worker/nodes/arch/diffusion/__init__.py`'s
  `_REGISTERED_MODULES` list (import + append), mirroring how `zit` is registered.
- Add >=4 new tests to `worker/tests/test_arch_flux2klein.py`:
  - `test_can_handle_matches_flux2klein` — primary match path
  - `test_can_handle_rejects_zit_key` — rejects zit's fixture key (cross-check)
  - `test_get_module_returns_flux2klein_for_flux2klein_key` — end-to-end dispatch
  - `test_get_module_returns_zit_for_zit_key` — end-to-end dispatch, second module
- Add >=1 cross-check test to `worker/tests/test_arch_zit.py`:
  - `test_can_handle_rejects_flux2klein` — zit.py's can_handle rejects "flux2klein"
- Verify zero changes to `loader.py`, `sampler.py`, `encoder.py`, `decode.py`, `image.py`.

### Out of Scope
None. `defers_to (from JSON): []` — this task has no deferred scope.

## Existing Codebase Assessment

**What already exists:**
- `flux2klein.py` (P25-B1) has `_infer_hyperparams()`, `_infer_hyperparams_inner()`,
  `_safetensors_dtype_to_canonical()`, the `ARCH = "flux2klein"` constant, and the
  torch import guard. It does NOT yet have `can_handle()` — P25-B1 explicitly deferred
  it (line 10 of the module docstring: "can_handle(key) — deferred to P25-B2").
- `zit.py` has `can_handle(key: str) -> bool` at line 837-851, which simply returns
  `key == ARCH` (where `ARCH = "zit"`). It also has `_infer_hyperparams()`, `load()`,
  `sample()`, `compute_latent_shape()`, and the dual-mode parity markers on `load()`
  and `sample()`.
- `arch/diffusion/__init__.py` imports `zit` and appends it to `_REGISTERED_MODULES`.
  `get_module(key)` iterates over registered modules, calling each `can_handle(key)`,
  returning the first match or `None`.
- `test_arch_flux2klein.py` has 4 tests from P25-B1 (all `_infer_hyperparams` tests).
  No tests for `can_handle()` or dispatch registration yet.
- `test_arch_zit.py` has `test_can_handle_matches_zit()` and
  `test_can_handle_rejects_unrelated_key()` (which uses "flux2klein" as the unrelated
  key — this test already exists but does NOT assert the specific cross-check we need).
- `test_arch_dispatch.py` tests the generic dispatcher with mock doubles and empty
  registry; it does NOT test two real modules coexisting.

**Established patterns to follow:**
- `can_handle()` is a one-liner: `return key == ARCH`. No logging, no complexity.
- Module registration: `from worker.nodes.arch.diffusion import <module>` then
  `_REGISTERED_MODULES.append(<module>)` in `__init__.py`.
- Test style: Google-style docstrings, assert-based assertions, no fixtures needed
  for `can_handle()` tests (pure function with string input/output).
- torch guard: `can_handle()` must remain importable without torch (it doesn't use
  torch at all), so no changes to the import guard are needed.

**Gap between design doc and current source:**
- `flux2klein.py` lacks `can_handle()` entirely — this is the sole purpose of this task.
- The diffusion dispatcher only has `zit` registered — adding `flux2klein` is the
  structural change.
- No cross-check test exists in `test_arch_zit.py` that specifically asserts
  `can_handle("flux2klein") is False` (the existing
  `test_can_handle_rejects_unrelated_key()` uses "flux2klein" but its docstring says
  "unrelated architecture string" generically rather than naming flux2klein).

## Resolved Dependencies

None. This task introduces no new external dependencies. All imports are from the
Python standard library (`logging`, `typing`) or existing project packages
(`safetensors` which is in `requirements/base.txt`). No MCP lookups required.

## Approach

### Step 1: Add `can_handle()` to `flux2klein.py`

Add the following function at the end of `flux2klein.py` (after `_infer_hyperparams()`,
before any module-level code that might be affected — place it after the
`_SAFETENSORS_DTYPE_MAP` and `_safetensors_dtype_to_canonical()` but before
`_infer_hyperparams_inner()`, matching `zit.py`'s placement where `can_handle()`
appears after the main public functions):

```python
def can_handle(key: str) -> bool:
    """Confirm this module handles the given architecture key.

    The dispatcher passes the architecture string (from safetensors
    metadata or path fallback) as *key*. This function returns True
    only when the key matches this module's canonical identifier.

    Args:
        key: Architecture string to check, e.g. ``"zit"`` or
            ``"flux2klein"``.

    Returns:
        ``True`` if *key* equals ``"flux2klein"``, ``False`` otherwise.
    """
    return key == ARCH
```

Rationale: This is identical in structure to `zit.py`'s `can_handle()` (line 837-851).
No logging is needed — `can_handle()` is a pure predicate called by the dispatcher
during routing, not an operational event. No docstring is needed beyond the minimal
one matching the pattern.

### Step 2: Register flux2klein in `arch/diffusion/__init__.py`

Modify `worker/nodes/arch/diffusion/__init__.py`:

1. Add import: `from worker.nodes.arch.diffusion import flux2klein`
   (next to the existing `from worker.nodes.arch.diffusion import zit`)
2. Add registration: `_REGISTERED_MODULES.append(flux2klein)`
   (next to the existing `.append(zit)`)

The updated `__init__.py` will look like:

```python
from __future__ import annotations

from types import ModuleType
from typing import Any

from worker.nodes.arch.diffusion import zit
from worker.nodes.arch.diffusion import flux2klein  # NEW: P25-B2

_REGISTERED_MODULES: list[ModuleType] = []
_REGISTERED_MODULES.append(zit)
_REGISTERED_MODULES.append(flux2klein)  # NEW: P25-B2
```

Rationale: Registration order matters — `get_module()` returns the **first** match.
Since `can_handle()` is an exact string match (`key == ARCH`), registration order
does not affect correctness (no two modules will ever match the same key). The order
is kept alphabetical by module name for consistency (`flux2klein` before `zit`).

### Step 3: Add tests to `test_arch_flux2klein.py`

Add the following 4 tests after the existing 4 tests in `test_arch_flux2klein.py`:

**Test 1: `test_can_handle_matches_flux2klein`**
```python
def test_can_handle_matches_flux2klein() -> None:
    """can_handle(\"flux2klein\") returns True — the primary match path.

    Calls can_handle() with the canonical Flux 2 Klein architecture string
    and asserts it returns True, proving the dispatcher will route a
    ``\"flux2klein\"`` key to this module.
    """
    from worker.nodes.arch.diffusion.flux2klein import can_handle
    assert can_handle("flux2klein") is True
```

**Test 2: `test_can_handle_rejects_zit_key`**
```python
def test_can_handle_rejects_zit_key() -> None:
    """can_handle(\"zit\") returns False — the module rejects unrelated keys.

    Calls can_handle() with zit's architecture string and asserts it returns
    False, proving the dispatcher will skip this module for non-Flux2Klein keys.
    This is the cross-check against zit.py's fixture key.
    """
    from worker.nodes.arch.diffusion.flux2klein import can_handle
    assert can_handle("zit") is False
```

**Test 3: `test_get_module_returns_flux2klein_for_flux2klein_key`**
```python
def test_get_module_returns_flux2klein_for_flux2klein_key() -> None:
    """get_module(\"flux2klein\") returns the flux2klein module — end-to-end dispatch.

    Calls get_module() with ``"flux2klein"`` and asserts the result is not None
    and is the flux2klein module, proving that importing and registering flux2klein
    in __init__.py makes the dispatcher find it as the second registered module.
    """
    from worker.nodes.arch.diffusion import flux2klein, get_module
    result = get_module("flux2klein")
    assert result is not None
    assert result is flux2klein
```

**Test 4: `test_get_module_returns_zit_for_zit_key`**
```python
def test_get_module_returns_zit_for_zit_key() -> None:
    """get_module(\"zit\") returns the zit module — two-module coexistence.

    Calls get_module() with ``"zit"`` and asserts the result is not None
    and is the zit module, proving that both zit and flux2klein coexist in
    _REGISTERED_MODULES and each is correctly disambiguated by its own
    can_handle(). This is the primary test for the two-module disambiguation
    that ANVILML_DESIGN.md §20 requires.
    """
    from worker.nodes.arch.diffusion import get_module, zit
    result = get_module("zit")
    assert result is not None
    assert result is zit
```

### Step 4: Add cross-check test to `test_arch_zit.py`

Add one test to `test_arch_zit.py` after the existing `test_can_handle_rejects_unrelated_key()`:

**Test: `test_can_handle_rejects_flux2klein`**
```python
def test_can_handle_rejects_flux2klein() -> None:
    """can_handle(\"flux2klein\") returns False — cross-check against flux2klein fixture.

    Calls can_handle() with the Flux 2 Klein architecture string and asserts
    it returns False, proving that zit.py's can_handle correctly rejects
    flux2klein's key. This is the bidirectional cross-check required by
    P25-B2: flux2klein's can_handle must reject zit's fixture AND
    zit's can_handle must reject flux2klein's fixture.
    """
    assert can_handle("flux2klein") is False
```

### Step 5: Verify zero changes to generic node layer

Confirm that no changes are needed to `loader.py`, `sampler.py`, `encoder.py`,
`decode.py`, or `image.py`. The dispatch mechanism in `arch/diffusion/__init__.py`
is architecture-agnostic — it iterates over `_REGISTERED_MODULES` and calls
`can_handle(key)` on each. Adding a second module to the list is transparent to
all callers.

## Public API Surface

| Item | Module Path | Signature | Description |
|------|-------------|-----------|-------------|
| `can_handle` | `worker.nodes.arch.diffusion.flux2klein` | `def can_handle(key: str) -> bool` | Returns True if key equals "flux2klein" |
| `_REGISTERED_MODULES` (modified) | `worker.nodes.arch.diffusion` | `list[ModuleType]` | Now contains [zit, flux2klein] instead of [zit] |
| `get_module` (unchanged) | `worker.nodes.arch.diffusion` | `def get_module(key: Any) -> ModuleType \| None` | Already handles arbitrary module count |

No new public API items beyond `can_handle()`. The `get_module()` signature is unchanged.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/arch/diffusion/flux2klein.py` | Add `can_handle(key) -> bool` function |
| MODIFY | `worker/nodes/arch/diffusion/__init__.py` | Import and register flux2klein module |
| MODIFY | `worker/tests/test_arch_flux2klein.py` | Add 4 new tests (can_handle + dispatch) |
| MODIFY | `worker/tests/test_arch_zit.py` | Add 1 cross-check test |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|--------------------|
| `test_arch_flux2klein.py` | `test_can_handle_matches_flux2klein` | `can_handle("flux2klein")` returns True | `python -m pytest worker/tests/test_arch_flux2klein.py::test_can_handle_matches_flux2klein -v` |
| `test_arch_flux2klein.py` | `test_can_handle_rejects_zit_key` | `can_handle("zit")` returns False (cross-check) | `python -m pytest worker/tests/test_arch_flux2klein.py::test_can_handle_rejects_zit_key -v` |
| `test_arch_flux2klein.py` | `test_get_module_returns_flux2klein_for_flux2klein_key` | `get_module("flux2klein")` returns flux2klein module | `python -m pytest worker/tests/test_arch_flux2klein.py::test_get_module_returns_flux2klein_for_flux2klein_key -v` |
| `test_arch_flux2klein.py` | `test_get_module_returns_zit_for_zit_key` | `get_module("zit")` returns zit module (two-module coexistence) | `python -m pytest worker/tests/test_arch_flux2klein.py::test_get_module_returns_zit_for_zit_key -v` |
| `test_arch_zit.py` | `test_can_handle_rejects_flux2klein` | `can_handle("flux2klein")` returns False (bidirectional cross-check) | `python -m pytest worker/tests/test_arch_zit.py::test_can_handle_rejects_flux2klein -v` |

Acceptance command: `python -m pytest worker/tests/test_arch_flux2klein.py -v` exits 0 (>=7 total tests).

## CI Impact

No CI changes required. The new tests are Python unit tests that run in both the
`worker-linux-mock` and `worker-windows-mock` CI jobs (they don't import torch at
module level — `can_handle()` is a pure string comparison). No new CI job, gate, or
workflow file is needed.

## Platform Considerations

None identified. The `can_handle()` function is a pure string comparison with no
platform-specific behavior. The module registration in `__init__.py` is standard
Python import/addition with no `#[cfg(...)]` equivalent needed. The Windows
cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `get_module("zit")` returns flux2klein instead of zit due to registration order | Low | High | Registration order does not affect correctness since `can_handle()` is an exact string match. Both `can_handle("zit")` and `can_handle("flux2klein")` return False for the other's key, so the first matching module is always the correct one regardless of order. |
| Importing flux2klein in `__init__.py` triggers torch import during mock-mode collection | Low | Medium | flux2klein.py's torch import is already guarded (try/except at module level, same pattern as zit.py). The module is importable without torch — `can_handle()`, `_infer_hyperparams()`, and `ARCH` are all torch-free. Verified by the existing test file's guarded import. |
| Adding flux2klein to `_REGISTERED_MODULES` breaks existing zit-only tests in `test_arch_dispatch.py` | Low | Low | `test_arch_dispatch.py` uses a `_clear_diffusion_registry` fixture that restores zit after each test. Adding flux2klein to the initial state doesn't affect the fixture's behavior (it clears and restores). The fixture's restoration appends zit back, which is correct. |
| `can_handle()` signature mismatch with dispatcher expectations | Very Low | High | The dispatcher calls `module.can_handle(key)` where key is `Any`. `can_handle(key: str) -> bool` matches zit.py's exact signature. No type coercion or duck-typing needed. |

## Acceptance Criteria

- [ ] `python -m pytest worker/tests/test_arch_flux2klein.py -v` exits 0 with >=7 tests
- [ ] `python -m pytest worker/tests/test_arch_zit.py::test_can_handle_rejects_flux2klein -v` exits 0
- [ ] `can_handle("flux2klein")` returns True (verified by test)
- [ ] `can_handle("zit")` returns False (verified by test)
- [ ] `get_module("flux2klein")` returns the flux2klein module (verified by test)
- [ ] `get_module("zit")` returns the zit module (verified by test)
- [ ] `get_module("unknown")` returns None (no regression — existing behavior preserved)
- [ ] Zero changes to `loader.py`, `sampler.py`, `encoder.py`, `decode.py`, `image.py`
