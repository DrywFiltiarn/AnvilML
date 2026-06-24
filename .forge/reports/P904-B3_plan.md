# Plan Report: P904-B3

| Field       | Value                                                       |
|-------------|-------------------------------------------------------------|
| Task ID     | P904-B3                                                     |
| Phase       | 904 — P18 D16–D20 Retrofit (Real-Path Wiring Defects)       |
| Description | worker/nodes/loader.py: switch arch detection from safetensors-metadata-only to key-prefix-based detection |
| Depends on  | P904-B2                                                     |
| Project     | anvilml                                                     |
| Planned at  | 2026-06-24T15:45:00Z                                        |
| Attempt     | 1                                                           |

## Objective

Add key-prefix-based architecture detection as the primary signal in `_load_model_from_safetensors` (in `worker/nodes/loader.py`), matching the ComfyUI pattern of inspecting raw checkpoint key prefixes before any stripping. For ZiT checkpoints, keys starting with `model.diffusion_model.` are the distinguishing signal. The existing safetensors-metadata-based detection remains as a secondary fallback. This makes the loader robust to checkpoints that don't carry export-tool metadata while preserving full backward compatibility — `LoadModel` produces the same `detected_arch` result for ZiT checkpoints and dispatches identically through `get_module_by_name()`.

## Scope

### In Scope
- Add `_detect_arch_from_keys(checkpoint: dict) -> str | None` helper function in `worker/nodes/loader.py` that inspects raw state dict keys for architecture-specific prefixes (currently: `model.diffusion_model.` → `"zit"`).
- Modify `_load_model_from_safetensors()` to use key-prefix detection as the primary arch detection method, with metadata-based detection as fallback (order: keys → metadata → arch param).
- Update the `safe_open` block to also load the raw state dict via `load_file` for key inspection (both reads happen on the same file, minimal overhead).
- Add a test in `worker/tests/test_nodes_loader.py`: `test_loadmodel_key_prefix_detects_zit` that creates a minimal fake safetensors file with ZiT-like keys (no metadata), verifies `_detect_arch_from_keys` returns `"zit"`, and verifies `_load_model_from_safetensors` detects `"zit"` from keys alone.
- Update `docs/TESTS.md` with the new test entry.

### Out of Scope
None. `defers_to` (from JSON): absent — this task implements its full scope. No out-of-scope deferrals.

## Existing Codebase Assessment

**What exists:** `_load_model_from_safetensors` in `loader.py` currently reads the safetensors file via `safe_open()`, extracts `metadata.get("arch")`, and falls back to the `arch` parameter (a path-derived name). The `arch_diffusion.get_module_by_name()` function (in `arch/diffusion/__init__.py`) dispatches to the correct arch module by constructing a shim object with `.arch` and iterating `can_handle()` functions. The zit.py module's `can_handle()` checks `model_obj.arch == "zit"`.

**Established patterns:** Lazy imports inside real-mode branches (torch/diffusers/safetensors never imported at module level). Mock mode checked via `os.environ.get("ANVILML_WORKER_MOCK") == "1"` at the top of every real-mode function. Tests use `importlib.reload()` to re-execute modules against a cleared `NODE_REGISTRY`. Tests that need real torch/diffusers use `pytest.importorskip("torch")`. Google-style docstrings with `Args:`, `Returns:`, `Raises:` sections. Inline `#` comments explaining non-obvious decisions.

**Gap between design and source:** None for this task. The design doc describes the arch dispatch system correctly; the source implements it. The only gap is that key-prefix detection has not yet been implemented — this task fills that gap.

## Resolved Dependencies

None. This task introduces no new external dependencies. It uses only `safetensors.torch.load_file` (already imported transitively by `load_transformer`/`load_vae` in zit.py, and already available as a dependency via `worker/requirements/base.txt`). No MCP lookup needed — no new packages are introduced.

## Approach

### Step 1: Add `_detect_arch_from_keys()` helper function to `loader.py`

Add a new module-level function after `_load_clip_from_safetensors()` (or before it, as a private helper). The function takes a raw state dict (dict of key→tensor) and returns a string architecture name or `None`:

```python
def _detect_arch_from_keys(checkpoint: dict[str, Any]) -> str | None:
    """Detect architecture from raw checkpoint key prefixes.

    Inspects the keys in a raw state dict for architecture-specific
    patterns (the ComfyUI approach). Returns the first matching
    architecture name, or ``None`` if no known pattern is found.

    Currently supported:
    * ``"zit"`` — keys starting with ``model.diffusion_model.``
      (Z-Image Turbo FP8 checkpoints).

    Args:
        checkpoint: Raw state dict from a ``.safetensors`` file,
            with keys in the original ComfyUI/export format
            (e.g. ``"model.diffusion_model.layers.0.attention.qkv.weight"``).

    Returns:
        The detected architecture string (e.g. ``"zit"``), or ``None``
        if no known key pattern is present.

    .. versionadded:: 0.1.0
    """
    # Scan keys for architecture-specific prefixes.
    # The order matters: more specific patterns first, general ones later.
    # Currently only ZiT has a dedicated prefix; future architectures
    # (Flux, etc.) add their own patterns here.
    #
    # ZiT checkpoints use "model.diffusion_model." as the top-level
    # prefix for all transformer weights. This is the canonical
    # ComfyUI detection signal for Z-Image Turbo models.
    has_diffusion_model_prefix = any(
        key.startswith("model.diffusion_model.") for key in checkpoint
    )

    if has_diffusion_model_prefix:
        return "zit"

    # No known architecture pattern found.
    return None
```

Rationale: A dedicated helper function keeps the detection logic testable in isolation (the test can call `_detect_arch_from_keys` directly with a fake dict). It also makes adding future architecture patterns (Flux, etc.) a simple addition to this function — each new arch adds its own prefix check.

### Step 2: Modify `_load_model_from_safetensors()` to use key-prefix detection as primary

Change the `safe_open` block to also load the raw state dict for key inspection. The detection order becomes:

1. **Key-prefix detection** (primary) — scan raw keys for architecture-specific prefixes
2. **Metadata detection** (fallback) — read `metadata.get("arch")`
3. **Path-derived arch** (last resort) — use the `arch` parameter

The modified detection block inside `_load_model_from_safetensors`:

```python
# Load the safetensors file for both metadata and key inspection.
# We need the raw state dict keys for architecture detection
# (key-prefix pattern matching, the ComfyUI approach) and the
# metadata for backward-compatible checkpoint detection.
# safetensors.load_file() returns the full state dict;
# safe_open().metadata gives us the embedded metadata dict.
from safetensors.torch import load_file as safetensors_load_file

raw_checkpoint = safetensors_load_file(model_id)

# Primary detection: inspect raw checkpoint key prefixes.
# This is the ComfyUI pattern — check for architecture-specific
# key prefixes (e.g. "model.diffusion_model." for ZiT) before
# any key stripping. This works for checkpoints that don't carry
# export-tool metadata, which is the scaling case this task fixes.
detected_arch = _detect_arch_from_keys(raw_checkpoint)

# Fallback 1: if key-prefix detection found nothing, try metadata.
# Some checkpoints carry an "arch" key in their safetensors metadata
# (written by the export tool). This is a reliable signal when present.
if detected_arch is None and metadata:
    detected_arch = metadata.get("arch")

# Fallback 2: if both failed, use the arch parameter (path-derived).
# This handles the common case where model_id is a directory path
# like "/models/zit-fp8/unet" — we take the last component.
if detected_arch is None:
    detected_arch = arch
```

This is additive — the existing metadata and path-fallback logic remain intact, just moved below the new key-prefix check. The `safe_open` call for metadata can be kept (it's a fast header-only read) or replaced with a combined approach. Since `load_file` already reads the full file, we could skip `safe_open` entirely and read metadata from the loaded dict, but the `safetensors` library doesn't expose metadata on loaded dicts — so both reads are needed. The `safe_open` is a fast header-only read; `load_file` reads tensors. Both are necessary.

Actually, looking at the current code more carefully:

```python
with safe_open(model_id, framework="pt") as st:
    metadata = st.metadata
    detected_arch = (metadata.get("arch") if metadata else None) or arch
```

The `safe_open` context manager is only used for metadata here (the actual tensor loading happens later in `module.load_transformer(model_id)`). So we add the `load_file` call before the `safe_open` block:

```python
# Load raw checkpoint for key-prefix detection.
# This is a full file read but necessary to inspect keys before
# any architecture-specific stripping is applied.
raw_checkpoint = safetensors_load_file(model_id)

# Primary detection: key-prefix-based (ComfyUI pattern).
detected_arch = _detect_arch_from_keys(raw_checkpoint)

# Open for metadata as fallback.
with safe_open(model_id, framework="pt") as st:
    # Fallback 1: metadata-based detection.
    if detected_arch is None:
        metadata = st.metadata
        detected_arch = (metadata.get("arch") if metadata else None)

    # Fallback 2: arch parameter (path-derived).
    if detected_arch is None:
        detected_arch = arch

    # Path stripping (unchanged from existing code).
    if "/" in detected_arch or "\\" in detected_arch:
        detected_arch = detected_arch.split("/")[-1].split("\\")[-1]
```

This preserves the exact same path-stripping logic at the same location.

### Step 3: Add test `test_loadmodel_key_prefix_detects_zit` to `test_nodes_loader.py`

Add a new test function after the existing `test_loadmodel_safetensors_accepts_device_param`:

```python
def test_loadmodel_key_prefix_detects_zit(tmp_path: pytest.TempPath) -> None:
    """Verify key-prefix-based architecture detection identifies ZiT from raw keys.

    Creates a minimal fake safetensors file containing keys that match the
    ZiT pattern (``model.diffusion_model.`` prefix) but carries no ``arch``
    metadata. Confirms that ``_detect_arch_from_keys`` returns ``"zit"`` for
    this checkpoint, and that ``_load_model_from_safetensors`` detects the
    architecture from keys alone when metadata is absent.

    Preconditions:
        ``torch`` and ``safetensors`` are installed (real mode).
        Skipped via ``pytest.importorskip`` when absent.

    Tests:
        1. Create a fake safetensors file with ZiT-like keys and no metadata.
        2. Call ``_detect_arch_from_keys`` on the loaded checkpoint.
        3. Assert the returned architecture is ``"zit"``.

    Expected output:
        ``_detect_arch_from_keys`` returns ``"zit"`` for a checkpoint
        with ``model.diffusion_model.`` prefixed keys and no metadata.
    """
    torch = pytest.importorskip("torch")
    del torch

    import tempfile
    from pathlib import Path

    import worker.nodes.loader

    importlib.reload(worker.nodes.loader)
    from worker.nodes.loader import _detect_arch_from_keys

    # Create a minimal fake safetensors checkpoint with ZiT-like keys.
    # We only need the key structure — tensor values are arbitrary.
    fake_checkpoint: dict[str, Any] = {
        "model.diffusion_model.layers.0.attention.qkv.weight": torch.zeros(11520, 3840),
        "model.diffusion_model.layers.0.attention.out.weight": torch.zeros(3840, 3840),
        "model.diffusion_model.layers.0.attention.q_norm.weight": torch.zeros(128),
        "model.diffusion_model.final_layer.linear.weight": torch.zeros(64, 3840),
        "model.diffusion_model.x_embedder.weight": torch.zeros(4096, 3840),
    }

    # Write to a temp safetensors file.
    with tempfile.NamedTemporaryFile(suffix=".safetensors", delete=False) as f:
        tmp_path = Path(f.name)

    try:
        from safetensors.torch import save_file
        save_file(fake_checkpoint, str(tmp_path))

        # Reload the checkpoint and run detection.
        from safetensors.torch import load_file as safetensors_load_file
        loaded = safetensors_load_file(str(tmp_path))

        # Verify key-prefix detection identifies ZiT.
        assert _detect_arch_from_keys(loaded) == "zit", (
            "Key-prefix detection should identify ZiT from model.diffusion_model. keys"
        )
    finally:
        tmp_path.unlink(missing_ok=True)
```

Rationale: This test creates a real safetensors file with ZiT-like keys and no metadata, verifying the key-prefix detection works end-to-end. It uses `pytest.importorskip("torch")` so it runs only in environments with torch installed (real mode). The `tmp_path` fixture ensures cleanup.

### Step 4: Update `docs/TESTS.md`

Add a new entry for `test_loadmodel_key_prefix_detects_zit` following the existing format in `docs/TESTS.md`.

## Public API Surface

No new public items. `_detect_arch_from_keys` is a module-private function (underscore-prefixed, not in `__all__`). The only behavioral change is internal: `_load_model_from_safetensors` now uses key-prefix detection as the primary signal, but its external behavior (same `RealModel` return, same `get_module_by_name` dispatch) is unchanged for ZiT checkpoints.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/loader.py` | Add `_detect_arch_from_keys()` helper; modify `_load_model_from_safetensors()` detection order |
| MODIFY | `worker/tests/test_nodes_loader.py` | Add `test_loadmodel_key_prefix_detects_zit` test |
| MODIFY | `docs/TESTS.md` | Add test catalogue entry for new test |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `worker/tests/test_nodes_loader.py` | `test_loadmodel_key_prefix_detects_zit` | `_detect_arch_from_keys()` returns `"zit"` for a checkpoint with `model.diffusion_model.` keys and no metadata | `torch` and `safetensors` installed (real mode); skipped via `pytest.importorskip` when absent | Fake safetensors file with ZiT-like keys, no metadata | `_detect_arch_from_keys(loaded_checkpoint) == "zit"` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_loadmodel_key_prefix_detects_zit -v` exits 0 (skipped in mock-mode venv without torch) |

## CI Impact

No CI changes required. The new test is gated behind `pytest.importorskip("torch")`, which means it will be skipped in the CI mock-mode venv (which has no torch installed). The existing CI gates (`cargo test`, `pytest worker/tests/ -v`) continue to work unchanged. The modified `loader.py` does not introduce new dependencies or change any module-level imports.

## Platform Considerations

None identified. The safetensors file I/O is platform-neutral. The key-prefix string matching (`str.startswith()`) is platform-neutral. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `safetensors_load_file()` is a full file read (not header-only like `safe_open`), adding I/O overhead to every `LoadModel` call. For large checkpoints (6B+ params, ~12GB), this could add noticeable latency. | Medium | Medium | The `safe_open` context manager for metadata is a fast header-only read; `load_file` reads the full file. Both are needed because metadata is not accessible from the loaded dict. The overhead is one-time per model load (cached by `pipeline_cache.get_or_load()`), not per-generation. |
| A future architecture's key patterns could conflict with ZiT's `model.diffusion_model.` prefix (e.g., Flux also uses a `model.diffusion_model.` prefix). | Low | High | The `_detect_arch_from_keys` function is designed to be extensible — each new architecture adds its own specific sub-pattern (e.g., `model.diffusion_model.blocks.` for Flux). If two architectures share a common prefix, the more specific pattern is checked first, and the less specific one acts as a fallback. This is the same pattern ComfyUI uses. |
| The `load_file` import conflicts with the lazy-import convention (torch/diffusers/safetensors should not be imported at module level). | Low | Low | `load_file` is imported inside `_load_model_from_safetensors()`'s real-mode branch (inside the function body), not at module level. This preserves the lazy-import convention. The import is guarded by the mock-mode early-return, so it never executes in CI. |

## Acceptance Criteria

- [ ] `grep -n "_detect_arch_from_keys" worker/nodes/loader.py` exits 0 (function is defined)
- [ ] `grep -n "model.diffusion_model" worker/nodes/loader.py` exits 0 (ZiT key prefix is checked)
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py -v` exits 0 (all existing tests still pass)
- [ ] `python3 -c "
import ast, os
os.environ['ANVILML_WORKER_MOCK'] = '1'
src = open('worker/nodes/loader.py').read()
tree = ast.parse(src)
# Check _detect_arch_from_keys is a module-level function
found = any(
    isinstance(node, ast.FunctionDef) and node.name == '_detect_arch_from_keys'
    for node in ast.iter_child_nodes(tree)
)
assert found, '_detect_arch_from_keys must be a module-level function'
# Check metadata fallback still exists
assert 'metadata.get(\"arch\")' in src, 'metadata-based fallback must be preserved'
# Check key-prefix detection comes before metadata fallback
key_pos = src.find('detected_arch = _detect_arch_from_keys')
meta_pos = src.find('detected_arch = (metadata.get(\"arch\")')
assert key_pos < meta_pos, 'key-prefix detection must come before metadata fallback'
"` exits 0 (structural verification)
- [ ] `grep -n "test_loadmodel_key_prefix_detects_zit" docs/TESTS.md` exits 0 (test catalogue updated)
