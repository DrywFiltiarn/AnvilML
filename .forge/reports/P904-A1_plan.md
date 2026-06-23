# Plan Report: P904-A1

| Field       | Value                                                       |
|-------------|-------------------------------------------------------------|
| Task ID     | P904-A1                                                     |
| Phase       | 904 — P18 D16–D20 Retrofit (Real-Path Wiring Defects)       |
| Description | worker/tests/test_nodes_decode.py: unconditional torch import breaks CI (no torch in base.txt) |
| Depends on  | P18-D20                                                     |
| Project     | anvilml                                                     |
| Planned at  | 2026-06-23T18:00:00Z                                        |
| Attempt     | 1                                                           |

## Objective

Fix the CI-breaking unconditional `import torch` in `worker/tests/test_nodes_decode.py` so that the file can be collected and run by `pytest worker/tests/ -v` in an environment where `torch` is not installed (the default CI venv built from `base.txt`). The fix adds a guarded torch import at the module level using `pytest.importorskip`, wraps the test function body in a try/except ImportError to skip gracefully, and guards the `_MockVaeWithDecode.decode()` method's torch usage. The chosen approach is to guard in-place (option (a) from the task context) rather than move the test to Group B, because P904-A1 has no dependency on P904-B4 and needs to land first to unblock CI for subsequent Group A tasks.

## Scope

### In Scope
- `worker/tests/test_nodes_decode.py`:
  - Add a guarded `import torch` at the top of the file (try/except ImportError → pytest.skip) so the module is importable even without torch.
  - Add `pytest.importorskip("torch")` immediately after the guarded import so pytest skips the test during collection when torch is absent.
  - Wrap the body of `test_vaedeode_real_path_returns_pil_image()` in a try/except ImportError block that calls `pytest.skip` on failure, covering both `import torch` inside the function and `torch.randn()` usage.
  - Guard `_MockVaeWithDecode.decode()`'s `import torch` with a try/except ImportError that raises ImportError (which bubbles up to the outer handler) so the method fails gracefully when torch is absent.

### Out of Scope
None. `defers_to (from JSON): []` — this task must implement its full scope. No functionality is deferred to another task.

## Existing Codebase Assessment

The test file `worker/tests/test_nodes_decode.py` contains five tests for the `VaeDecode` node: three mock-mode tests (registry registration, mock-mode execution, metadata attributes, and missing-input handling) and one real-path test (`test_vaedeode_real_path_returns_pil_image`). The real-path test clears `ANVILML_WORKER_MOCK` to exercise the real decode code path, creates a `_MockVaeWithDecode` instance (a helper class that provides a real `decode()` method returning a `torch.rand()` tensor), and passes a `torch.randn()` latent tensor to `VaeDecode.execute()`.

The `conftest.py` sets `ANVILML_WORKER_MOCK=1` via an autouse fixture for every test, but the real-path test explicitly pops this variable to run in real mode. The `worker/requirements/base.txt` deliberately excludes `torch` — it installs `pyzmq`, `msgpack`, `pillow`, `safetensors`, `diffusers`, `transformers`, and `pytest`, none of which pull in `torch` transitively. CI's worker job runs `ANVILML_WORKER_MOCK=1 <python> -m pytest worker/tests -v`, which collects this test file and fails at the `import torch` line inside both the test function and the helper class.

The established test style uses Google-style docstrings, explicit env-var capture-and-restore (already present in the test's `try/finally` block), and `importlib.reload()` to ensure fresh module state. The `_MockVaeWithDecode` class is defined at module scope, which means it gets imported during pytest collection — this is the root cause of the CI break, because the class body imports torch unconditionally.

## Resolved Dependencies

None. This task uses only `pytest.importorskip`, which is part of pytest's standard public API (available since pytest 2.0). No new dependencies are introduced. The `pytest>=9.1` dependency already declared in `base.txt` fully covers this usage.

## Approach

**Decision:** Guard in-place (option (a)) rather than move to Group B. Rationale: P904-A1 is the first task in the phase's sequential ordering and every subsequent Group A task needs working CI to validate its implementation report. Moving the test to P904-B4 would create a dependency on a later task that doesn't yet exist, violating the task's own sequencing rationale. Guarding in-place is the minimal, self-contained fix.

### Step 1: Add guarded torch import at module level

Add the following after the existing imports (after `from typing import Any`, before the fixtures section):

```python
# Guarded torch import — the test file must be importable even when
# torch is not installed (CI's base.txt venv excludes torch by design).
# pytest.importorskip() below handles the skip; this try/except
# prevents an ImportError from breaking module-level collection.
try:
    import torch
except ImportError:
    torch = None  # type: ignore[assignment]
```

This ensures `_MockVaeWithDecode` can be defined at module scope without failing when torch is absent. The `torch` variable will be `None` when unavailable, and the `importorskip` call in Step 2 will prevent the test from running.

### Step 2: Add pytest.importorskip guard

Add immediately after the guarded torch import from Step 1:

```python
# Skip this entire test file when torch is not installed.
# This is the primary CI guard — pytest skips the file during
# collection, so the guarded import above is only a fallback
# for edge cases (e.g. manual `python -c "import ..."` without pytest).
pytest.importorskip("torch")
```

This is the key fix: when `torch` is not installed, `importorskip` raises `pytest.skip` during test collection, and pytest reports the test as "skipped" rather than "error". The file is still collected (no import failure), but the test is not executed.

### Step 3: Wrap test function body in try/except ImportError

Replace the current test function body so that the `import torch` inside the function (line 314) and `torch.randn()` usage are covered:

```python
def test_vaedeode_real_path_returns_pil_image() -> None:
    """Verify ``execute()`` returns a ``PIL.Image.Image`` in real mode.

    Preconditions:
        ``ANVILML_WORKER_MOCK`` is unset (cleared by this test) so that
        the real decode code path is exercised.
        NODE_REGISTRY is cleared by the ``registry_clean`` fixture.

    Tests:
        Clear ``ANVILML_WORKER_MOCK``, instantiate ``VaeDecode`` with
        a ``mock_context``, call ``execute()`` with a ``MockVaeWithDecode``
        and a real torch tensor as ``latent``, and assert the returned
        image is a ``PIL.Image.Image``.

    Expected output:
        ``result["image"]`` is a ``PIL.Image.Image`` instance.
    """
    # Guard against torch not being installed — this can happen when
    # running the test manually outside of pytest (importorskip above
    # handles the pytest collection path).
    try:
        # Capture the pre-existing env value and restore unconditionally
        # after the test, per the env isolation convention (§11.3).
        # The conftest sets ANVILML_WORKER_MOCK=1, so we pop it to
        # exercise the real-mode branch.
        original = os.environ.pop("ANVILML_WORKER_MOCK", None)
        try:
            import worker.nodes.decode

            importlib.reload(worker.nodes.decode)
            from worker.nodes.decode import VaeDecode

            import torch  # noqa: F401  # Already imported at module level; noqa for clarity

            vae = _MockVaeWithDecode()
            latent = torch.randn(1, 4, 64, 64, dtype=torch.float32)

            node = VaeDecode(mock_context)
            result = node.execute(vae=vae, latent=latent)

            assert "image" in result
            # Verify the image is a real PIL Image, not a MockImage sentinel.
            from PIL import Image

            assert isinstance(result["image"], Image.Image)
            # Also verify it is NOT a MockImage — the sentinel must be
            # absent from the real-mode output.
            assert not isinstance(result["image"], worker.nodes.decode.MockImage)
        finally:
            # Restore the env var unconditionally so no other test sees
            # a modified environment.
            if original is not None:
                os.environ["ANVILML_WORKER_MOCK"] = original
    except ImportError as exc:
        pytest.skip(f"torch not available: {exc}")
```

The outer try/except ImportError catches both the `import torch` inside the function and any `torch.randn()` usage. When torch is absent, the test skips with a descriptive message. The inner try/finally (env var capture-and-restore) remains unchanged for correctness.

### Step 4: Guard `_MockVaeWithDecode.decode()` method

Replace the current `decode()` method body:

```python
    def decode(self, latents: Any, return_dict: bool = True) -> tuple:
        """Decode latent tensor to a raw image tensor.

        Returns a plain tuple (since ``return_dict=False`` is always
        passed by ``VaeDecode.execute()``) containing a real torch
        tensor in the ``[-1, 1]`` range.

        Args:
            latents: The latent tensor to decode.
            return_dict: Unused — the real method always returns a
                plain tuple to match the ``return_dict=False`` call
                in ``VaeDecode.execute()``.

        Returns:
            A tuple with one element: a ``torch.Tensor`` in the
            ``[-1, 1]`` range (typical VAE decoder output).

        Raises:
            ImportError: If torch is not installed (this method requires
                torch to produce a tensor output).
        """
        # Guard: this method requires torch to produce tensor output.
        # When torch is absent, raise ImportError so the outer try/except
        # in the test function can catch it and skip the test.
        try:
            import torch
        except ImportError:
            raise ImportError("torch required for _MockVaeWithDecode.decode()") from None

        # Produce a small random tensor in [-1, 1] — the exact values
        # don't matter for the test; only the shape and type matter.
        # The postprocess step will handle any valid tensor.
        return (torch.rand(1, 3, 64, 64, dtype=torch.float32),)
```

The guard inside `decode()` is a defense-in-depth measure: even if the outer try/except in the test function is the primary skip mechanism, the decode method itself should not silently succeed with wrong behavior when torch is absent.

### Step 5: Verify no other torch imports exist in the file

Confirm there are no additional bare `import torch` statements in the file. Based on inspection, there are exactly two: the one inside `test_vaedeode_real_path_returns_pil_image()` (Step 3) and the one inside `_MockVaeWithDecode.decode()` (Step 4). Both are covered by the guards above.

## Public API Surface

None. This task modifies only test code — no public API items are introduced or changed.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/tests/test_nodes_decode.py` | Add guarded torch import, `pytest.importorskip` guard, wrap test body in try/except ImportError, guard `_MockVaeWithDecode.decode()` method |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `worker/tests/test_nodes_decode.py` | `test_vaedeode_real_path_returns_pil_image` | The guarded test skips gracefully when torch is absent and passes when torch is available | `ANVILML_WORKER_MOCK=1` (cleared by test), no torch installed | pytest collection/run | Test is SKIPPED (not ERROR) | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_decode.py::test_vaedeode_real_path_returns_pil_image -v` exits 0, output contains "SKIPPED" |
| `worker/tests/test_nodes_decode.py` | all five tests | No regressions in mock-mode tests | `ANVILML_WORKER_MOCK=1`, no torch installed | Full test suite run | All five tests pass (four mock-mode pass, one real-path skipped) | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_decode.py -v` exits 0 |
| Full suite | all worker tests | No regressions across the full mock-mode suite | `ANVILML_WORKER_MOCK=1`, no torch installed | Full CI invocation | All tests pass or skip, zero errors | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/ -v` exits 0 |

## CI Impact

No CI configuration changes are required. The fix is entirely within the test source file. The existing CI invocation (`ANVILML_WORKER_MOCK=1 <python> -m pytest worker/tests -v`) will now collect the test file successfully (no import error) and skip the real-path test (pytest reports it as "skipped" rather than "error"). The `worker` CI job will pass with zero errors. No new CI jobs, markers, or matrix entries are introduced.

## Platform Considerations

None identified. The fix uses only standard Python/pytest constructs (`importorskip`, `try/except ImportError`, `pytest.skip()`) that behave identically on Linux and Windows. No `# cfg` guards or platform-specific code paths are needed. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The guarded module-level import (`torch = None`) could cause a downstream test to silently proceed with `torch` being `None` and produce a confusing `AttributeError` (e.g. `torch.randn` → `TypeError: 'NoneType' object is not callable`) instead of a clean skip | Low | Medium | The `pytest.importorskip("torch")` call at module level prevents collection of the test when torch is absent — the guarded import is a defense-in-depth fallback for non-pytest invocation paths (e.g. manual `python -c`). The outer try/except ImportError in the test function body catches any remaining ImportError from `torch.randn()` or `_MockVaeWithDecode.decode()`. |
| The `_MockVaeWithDecode` class definition at module scope is still evaluated even when torch is absent (because the import is guarded), and a future developer adding a torch-dependent expression at class scope outside `decode()` would silently succeed with `torch=None`, producing a confusing error | Low | Low | This is a code-review concern, not a runtime risk. The docstring on `_MockVaeWithDecode` explicitly notes it requires torch for its `decode()` method. A future developer adding torch-dependent code at class scope would see the guarded import and understand the pattern. |
| Wrapping the test body in try/except ImportError could mask an actual import error from a non-torch module (e.g. `from PIL import Image` fails) that is unrelated to the torch guard | Low | Low | The outer try/except is placed around the entire test body (after env var capture), so only ImportError is caught. If `from PIL import Image` fails, it would be an ImportError — but this is highly unlikely since `pillow` is a base.txt dependency. If it does happen, the error message would include the full traceback from the pytest skip call, making the root cause visible. |

## Acceptance Criteria

- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_decode.py -v` exits 0, with `test_vaedeode_real_path_returns_pil_image` showing "SKIPPED" (not "ERROR" or "FAILED")
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/ -v` exits 0 (full CI gate passes — no torch in base.txt venv)
- [ ] `grep -c 'import torch' worker/tests/test_nodes_decode.py` returns 3 (one guarded module-level import, one inside test function covered by outer try/except, one inside decode() method covered by its own guard)
- [ ] `grep 'importorskip' worker/tests/test_nodes_decode.py` returns at least one match (the primary skip guard)
- [ ] `grep -n 'torch = None' worker/tests/test_nodes_decode.py` returns at least one match (the guarded fallback import)
