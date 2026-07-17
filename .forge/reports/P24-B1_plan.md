# Plan Report: P24-B1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P24-B1                                            |
| Phase       | 24 — Generic Conditioning/Sampling/Decode Nodes, Real Mode |
| Description | worker/nodes/decode.py: VaeDecode node, mock branch only |
| Depends on  | P23-E1                                            |
| Project     | anvilml                                           |
| Planned at  | 2026-07-17T20:35:00Z                              |
| Attempt     | 1                                                 |

## Objective

Create the `VaeDecode` generic node in `worker/nodes/decode.py` with its `@register`
decorator, all six required class attributes (NODE_TYPE, CATEGORY, DISPLAY_NAME,
DESCRIPTION, INPUT_SLOTS, OUTPUT_SLOTS) matching ANVILML_DESIGN.md §10.3 exactly,
and an `execute()` method that branches on `ctx.mock`: the mock path returns a sentinel
dict `{"image": {"mock": True, "shape": inputs["latent"].get("shape")}}`, while the
real path is a bare `raise NotImplementedError` (deferred to P24-B2). This also creates
`worker/tests/test_nodes_decode.py` with ≥3 tests verifying class attributes, mock
behaviour, and NODE_REGISTRY registration.

## Scope

### In Scope
- `worker/nodes/decode.py` — new file; `VaeDecode` class with `@register`, six class
  attributes, `execute()` with mock + placeholder real branch, dual-mode parity markers.
- `worker/tests/test_nodes_decode.py` — new file; ≥3 tests: class attributes, mock
  sentinel return, NODE_REGISTRY registration.

### Out of Scope
- Real-mode `execute()` body — deferred to P24-B2, which dispatches to
  `arch.vae.get_module(vae.arch).decode()`. P24-B2's context states: "Complete
  VaeDecode's real branch...calls arch.vae.get_module(inputs[\"vae\"].arch).decode(...)".
- Real-mode tests — deferred to P24-B2, which states: ">=5 new tests in
  test_nodes_decode.py: real-mode execute against the Phase 23 fixture's loaded
  VAE+latent produces a real PIL Image".

## Existing Codebase Assessment

The node system infrastructure is fully established: `worker/nodes/base.py` contains
`BaseNode` (ABC), `@register` decorator, `SlotSpec` dataclass, `NodeContext` class with
`mock` field, and `NODE_REGISTRY` dict. The auto-import mechanism in `__init__.py`
scans `nodes/` for `.py` files (excluding `__init__`, `base`, and packages), so any new
`.py` file under `nodes/` is automatically registered at import time.

Existing node files (`sampler.py`, `encoder.py`, `loader.py`) follow a consistent pattern:
module-level docstring, imports from `worker.nodes.base`, `logging.getLogger(__name__)`,
`@register`-decorated class with six class attributes, dual-mode parity markers as
module-level comments above `execute()`, and the `execute()` method branching on
`ctx.mock` at the top. The mock sentinel pattern is `{"<output_slot>": {"mock": True,
...propagated input values...}}`.

Existing test files (`test_nodes_sampler.py`, `test_nodes_encoder.py`) follow a consistent
pattern: `_make_ctx()` helper fixture, class attributes test, mock execute test,
in-registry subprocess test, and real-mode tests marked `@pytest.mark.real_mode`. The
`test_nodes_encoder.py` test file has no real-mode tests yet (the real branch was
deferred in its predecessor task), demonstrating that a node can have a complete test
file with only mock-mode coverage when the real branch is deferred.

There is no existing `worker/nodes/decode.py` or `worker/tests/test_nodes_decode.py` —
this task creates both from scratch.

## Resolved Dependencies

None. This task introduces no new Python dependencies. It uses only the existing
`worker.nodes.base` imports (`BaseNode`, `NodeContext`, `SlotSpec`, `register`) which
are already present in the codebase.

## Approach

1. **Create `worker/nodes/decode.py`.** Follow the exact structure of `sampler.py` and
   `encoder.py`:
   - Module docstring: "VaeDecode node — decodes a denoised latent to a PIL image
     using the explicitly provided VAE. In mock mode it returns a sentinel dict; the
     real branch dispatches to the registered VAE architecture module (currently
     \"zit_vae\") via ``arch.vae.get_module()`` and ``module.decode()``."
   - Import `BaseNode`, `NodeContext`, `SlotSpec`, `register` from `worker.nodes.base`.
   - Import `logging`, create `logger = logging.getLogger(__name__)`.
   - Define `@register class VaeDecode(BaseNode)` with:
     - `NODE_TYPE = "VaeDecode"`
     - `CATEGORY = "Decoding"`
     - `DISPLAY_NAME = "VAE Decode"`
     - `DESCRIPTION = "Decodes a denoised latent to a PIL image using the explicitly provided VAE."`
     - `INPUT_SLOTS = [SlotSpec("vae", "VAE"), SlotSpec("latent", "LATENT")]`
     - `OUTPUT_SLOTS = [SlotSpec("image", "IMAGE")]`
   - Add dual-mode parity markers as module-level comments above `execute()`:
     ```python
     # REAL_PATH_VERIFIED: worker/tests/test_nodes_decode.py::test_vae_decode_real_decodes_zit_vae_fixture
     # MOCK_PATH_VERIFIED: worker/tests/test_nodes_decode.py::test_vae_decode_mock_returns_sentinel
     ```
     The `REAL_PATH_VERIFIED` marker names the test that P24-B2 will create; the
     `MOCK_PATH_VERIFIED` marker names this task's own mock test. Both markers are
     required by §10.6 on every `execute()` method, even when the real branch is a stub.
   - Implement `execute(self, ctx: NodeContext, **inputs) -> dict`:
     - `if ctx.mock:` — mock branch: return
       `{"image": {"mock": True, "shape": inputs["latent"].get("shape")}}`. Add a
       `logger.debug()` call with the shape value for observability.
     - `else:` — real branch placeholder: `raise NotImplementedError(
       "VaeDecode real branch deferred to P24-B2; dispatches to arch.vae.get_module(vae.arch).decode()")`.
       This satisfies §9.7a because the task's `defers_to` field names P24-B2, and
       the stub will carry a `# defers_to: P24-B2` comment at the raise site.

2. **Create `worker/tests/test_nodes_decode.py`.** Follow the structure of
   `test_nodes_encoder.py` (which also has mock-only tests for a deferred real branch):
   - `_make_ctx()` helper fixture (pattern from `test_nodes_encoder.py`, using
     `job_id="test-job"` since no raw UUID bytes are needed for these tests).
   - `test_vae_decode_class_attributes()` — verify all six class attributes match
     the §10.3 spec exactly.
   - `test_vae_decode_mock_returns_sentinel()` — construct `NodeContext(mock=True)`,
     call `execute()` with `vae={}` and `latent={"shape": (1, 4, 64, 64)}`, assert
     the return dict equals `{"image": {"mock": True, "shape": (1, 4, 64, 64)}}`.
     This test satisfies the `MOCK_PATH_VERIFIED` marker.
   - `test_vae_decode_in_registry()` — subprocess-isolated import of
     `worker.nodes.decode`, assert `NODE_REGISTRY["VaeDecode"]` exists and references
     the class. Pattern from `test_nodes_encoder.py::test_clip_text_encode_in_registry`.

3. **Verify syntax.** Run `python -m py_compile worker/nodes/decode.py worker/tests/test_nodes_decode.py`
   to confirm no syntax errors before running pytest.

4. **Run tests.** Execute `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest
   worker/tests/test_nodes_decode.py -v` and confirm all ≥3 tests pass with exit 0.

## Public API Surface

| Item | Location | Signature |
|------|----------|-----------|
| `VaeDecode` class | `worker/nodes/decode.py` | `class VaeDecode(BaseNode)` |
| `VaeDecode.NODE_TYPE` | `worker/nodes/decode.py` | `str = "VaeDecode"` |
| `VaeDecode.CATEGORY` | `worker/nodes/decode.py` | `str = "Decoding"` |
| `VaeDecode.DISPLAY_NAME` | `worker/nodes/decode.py` | `str = "VAE Decode"` |
| `VaeDecode.DESCRIPTION` | `worker/nodes/decode.py` | `str` |
| `VaeDecode.INPUT_SLOTS` | `worker/nodes/decode.py` | `list[SlotSpec]` |
| `VaeDecode.OUTPUT_SLOTS` | `worker/nodes/decode.py` | `list[SlotSpec]` |
| `VaeDecode.execute()` | `worker/nodes/decode.py` | `(self, ctx: NodeContext, **inputs) -> dict` |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/nodes/decode.py` | VaeDecode node class with @register, mock + placeholder real branch |
| CREATE | `worker/tests/test_nodes_decode.py` | ≥3 tests: class attributes, mock sentinel, NODE_REGISTRY registration |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|--------------------|
| `test_nodes_decode.py` | `test_vae_decode_class_attributes` | All six class attributes (NODE_TYPE, CATEGORY, DISPLAY_NAME, DESCRIPTION, INPUT_SLOTS, OUTPUT_SLOTS) match §10.3 spec exactly | `worker/.venv/bin/python -m pytest worker/tests/test_nodes_decode.py -v -k test_vae_decode_class_attributes` exits 0 |
| `test_nodes_decode.py` | `test_vae_decode_mock_returns_sentinel` | Mock-mode execute() returns `{"image": {"mock": True, "shape": <input_shape>}}`. Satisfies `MOCK_PATH_VERIFIED` marker. | `worker/.venv/bin/python -m pytest worker/tests/test_nodes_decode.py -v -k test_vae_decode_mock_returns_sentinel` exits 0 |
| `test_nodes_decode.py` | `test_vae_decode_in_registry` | NODE_REGISTRY contains "VaeDecode" after importing the module (subprocess-isolated) | `worker/.venv/bin/python -m pytest worker/tests/test_nodes_decode.py -v -k test_vae_decode_in_registry` exits 0 |

## CI Impact

The new test file `worker/tests/test_nodes_decode.py` is automatically picked up by
the existing CI `worker-linux-mock` and `worker-windows-mock` jobs, which run
`pytest worker/tests -v -m "not real_mode"`. No CI configuration changes are required.
The real-mode tests for VaeDecode (P24-B2) will be picked up by the
`worker-linux-real` and `worker-windows-real` CI jobs via `-m real_mode`.

## Platform Considerations

None identified. The code is pure Python with no platform-specific branches, no
path separators, no line-ending handling. The Windows cross-check in ENVIRONMENT.md §7
is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The `REAL_PATH_VERIFIED` marker names a test that does not yet exist (P24-B2 has not run). Gate 4 (§8 of ENVIRONMENT.md) will find the marker pointing at a non-collectible test. | High | Medium — Gate 4 will flag the stale marker, but it is expected for this phase-closing pattern. | The marker is correct per §10.6's additive rule — it declares intent. P24-B2 will create the named test, and the marker will become valid. This is the same pattern used by `LoadModel`/`LoadVae`/`LoadClip` where the REAL_PATH_VERIFIED marker was added in Phase 19 before the real branch was implemented. |
| The `defers_to` stub (raise NotImplementedError) violates §9.7a's prohibition on stubs when `defers_to` is empty. | Low | High — would be a session failure. | Mitigated by the fact that `defers_to: ["P24-B2"]` is non-empty. The stub will carry `# defers_to: P24-B2` at the raise site per §9.7. |
| Auto-import in `__init__.py` fails because `decode.py` has an import error or syntax error. | Low | High — would break all node imports and worker startup. | Step 3 of the approach runs `py_compile` on both files before running pytest, catching syntax errors before subprocess spawning. |

## Acceptance Criteria

- [ ] `worker/.venv/bin/python -m py_compile worker/nodes/decode.py worker/tests/test_nodes_decode.py` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_decode.py -v` exits 0 with ≥3 tests
- [ ] `grep -c "VaeDecode" worker/nodes/decode.py` returns ≥1 (file contains the class)
- [ ] `grep -c "REAL_PATH_VERIFIED:" worker/nodes/decode.py` returns 1 (marker present)
- [ ] `grep -c "MOCK_PATH_VERIFIED:" worker/nodes/decode.py` returns 1 (marker present)
