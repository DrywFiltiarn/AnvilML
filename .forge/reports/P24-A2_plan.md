# Plan Report: P24-A2

| Field       | Value                                                                 |
|-------------|-----------------------------------------------------------------------|
| Task ID     | P24-A2                                                                |
| Phase       | 24 — Generic Conditioning/Sampling/Decode Nodes, Real Mode            |
| Description | worker/nodes/encoder.py: ClipTextEncode real branch tokenizes + encodes |
| Depends on  | P24-A1                                                                |
| Project     | anvilml                                                               |
| Planned at  | 2026-07-17T19:45:00Z                                                  |
| Attempt     | 1                                                                     |

## Objective

Complete the `ClipTextEncode` node's real branch in `worker/nodes/encoder.py` by replacing the `NotImplementedError` stub with actual tokenization and encoding logic. The real branch receives a `clip` input (a fully-loaded `Qwen3TextEncoder` with an attached `.tokenizer`, produced by `LoadClip`'s real branch in Phase 22), tokenizes `positive_text` (and optionally `negative_text`), runs the encoder's forward pass, and returns `{"conditioning": {"text_embeds": <hidden_states_tensor>, "negative_text_embeds": <optional_hidden_states_tensor>}}`. Both `REAL_PATH_VERIFIED` and `MOCK_PATH_VERIFIED` markers must be updated to name genuinely passing tests. Acceptance: `python -m pytest worker/tests/test_nodes_encoder.py -v -m real_mode` exits 0 with >=8 total tests in the file.

## Scope

### In Scope
- Replace the `NotImplementedError` real branch in `ClipTextEncode.execute()` with actual tokenization + encoding logic.
- Update the `REAL_PATH_VERIFIED` and `MOCK_PATH_VERIFIED` markers on `execute()` to name the correct test functions.
- Update the docstring to remove the "P24-A2 deferred" language.
- Add real-mode tests in `worker/tests/test_nodes_encoder.py` covering: (a) encode with positive_text only, (b) encode with both positive and negative text, (c) conditioning tensor shape verification, (d) negative_text omitted still succeeds.
- Remove the `defers_to` comment from the real branch stub entirely.

### Out of Scope
None. This task implements its full scope. `defers_to (from JSON): []` -- no functionality is deferred.

## Existing Codebase Assessment

Phase 22's P22-D1 built `LoadClip`'s real branch, which dispatches to `arch.clip.get_module("qwen3")`, calls `qwen3.load(path, caps)`, and returns a `Qwen3TextEncoder` instance with an attached `.tokenizer` attribute (an `AutoTokenizer` loaded from `worker/assets/qwen3_tokenizer/`). The encoder has a `.forward(input_ids)` method that takes `(batch_size, sequence_length)` token IDs and returns `(batch_size, sequence_length, hidden_dim)` hidden states. The `qwen3_tiny.safetensors` fixture exists at `worker/tests/fixtures/qwen3_tiny.safetensors` for real-mode testing. The fixture has `hidden_dim=64`, `num_hidden_layers=2`, `vocab_size=128`, `intermediate_size=128`, and `arch="qwen3"` metadata.

Phase 24-A1 created the `ClipTextEncode` node class with a working mock branch that returns `{"conditioning": {"mock": True, "positive_text": ...}}`. The `defers_to` comment in the stub says `defers_to: P24-A1` but this is the opposite direction -- P24-A1 created the node, P24-A2 completes it. The `defers_to` field in the JSON is `[]`.

Established patterns: Nodes branch on `ctx.mock` at the top of `execute()`. The `clip` input is already a loaded `torch.nn.Module` (not a model_id string) -- this is the key design difference from `LoadModel`/`LoadVae`/`LoadClip`, which receive a `model_id` and dispatch through `get_module()`. `ClipTextEncode` does NOT re-dispatch; it operates directly on the loaded encoder. Tests use `_make_ctx()` helper, `PipelineCache()` for real-mode cache, and fixture paths via `Path(__file__).parent / "fixtures" / ...`. Real-mode tests are decorated with `@pytest.mark.real_mode`.

The conditioning format must carry both `text_embeds` (always present) and optionally `negative_text_embeds` -- this is the standard ComfyUI conditioning format that downstream `Sampler` nodes expect. The encoder's `.forward()` returns hidden states at the last layer.

## Resolved Dependencies

| Type   | Name       | Version verified | MCP source | Feature flags confirmed |
|--------|------------|-----------------|------------|------------------------|
| python | transformers | 4.x (vendored tokenizer) | pypi-query MCP | n/a |
| python | torch      | (installed via requirements/cpu-*.txt) | pypi-query MCP | n/a |

No new external dependencies are introduced. The task uses only `torch`, `transformers` (tokenizer already loaded by `LoadClip`), and existing modules (`PipelineCache`, `NodeContext`). The tokenizer is already loaded and attached to the encoder by `LoadClip`'s real branch -- no new tokenizer loading is needed.

## Approach

**Step 1: Implement the real branch in `ClipTextEncode.execute()`.**

Replace the `else` branch (lines 95-99 of `encoder.py`) with actual tokenization and encoding logic:

1. Extract `clip_encoder = inputs["clip"]` -- a `Qwen3TextEncoder` with attached `.tokenizer`.
2. Extract `tokenizer = clip_encoder.tokenizer` (set by `qwen3.py`'s `load()` function).
3. Tokenize `positive_text` with `tokenizer(..., padding="max_length", max_length=77, truncation=True, return_tensors="pt")`.
4. Run `clip_encoder.forward(positive_tokens["input_ids"])` to get `positive_embeds` of shape `(1, 77, hidden_dim)`.
5. Build a `conditioning` dict with `"text_embeds": positive_embeds`.
6. If `negative_text` is provided (not None), tokenize and encode it the same way, adding `"negative_text_embeds"` to the conditioning dict.
7. Return `{"conditioning": conditioning}`.

Key design decisions:
- `max_length=77` is the standard CLIP text encoding length used by ComfyUI and most diffusion workflows. This matches the typical context window of transformer-based text encoders.
- The conditioning dict carries `text_embeds` (always present) and optionally `negative_text_embeds` -- this is the standard ComfyUI conditioning format.
- The tokenizer is accessed via `clip_encoder.tokenizer` -- this attribute is set by `qwen3.py`'s `load()` function.
- `return_tensors="pt"` ensures the tokenizer returns PyTorch tensors directly, compatible with `.forward()`.

**Step 2: Update the markers.**

Replace the existing `REAL_PATH_VERIFIED` marker:
```
# REAL_PATH_VERIFIED: worker/tests/test_nodes_encoder.py::test_clip_text_encode_real_raises_placeholder
```
with:
```
# REAL_PATH_VERIFIED: worker/tests/test_nodes_encoder.py::test_clip_text_encode_real_positive_only
```

The `MOCK_PATH_VERIFIED` marker stays the same (the existing test is already correct).

**Step 3: Update docstrings.**

Remove the "P24-A2 deferred" language from the class docstring and the `execute()` method docstring. Replace with accurate descriptions of the actual behavior.

**Step 4: Add real-mode tests.**

Add the following tests to `worker/tests/test_nodes_encoder.py`:

1. `test_clip_text_encode_real_positive_only` (real_mode): Execute with only positive_text, verify the conditioning dict contains `text_embeds` with correct shape `(1, 77, 64)`, verify the tensor is on CPU. This test also serves as the REAL_PATH_VERIFIED marker target.

2. `test_clip_text_encode_real_with_negative` (real_mode): Execute with both positive_text and negative_text, verify both `text_embeds` and `negative_text_embeds` are present in the conditioning dict.

3. `test_clip_text_encode_real_negative_omitted` (real_mode): Execute without negative_text input, verify only `text_embeds` is in conditioning (no `negative_text_embeds` key). This covers the optional input slot behavior in real mode.

4. `test_clip_text_encode_real_conditioning_shape` (real_mode): Verify the conditioning tensor has the expected shape `(1, 77, 64)` where hidden_dim matches the fixture's inferred value (64).

**Step 5: Update the defers_to comment.**

Remove the `defers_to: P24-A1` comment from the real branch stub entirely, since P24-A2 now implements the full scope.

## Public API Surface

No new public API items are introduced. The only change is to the existing `ClipTextEncode.execute()` method's real branch, which now returns a conditioning dict with actual tensors instead of raising `NotImplementedError`. The return shape changes from:
- Before (mock only): `{"conditioning": {"mock": True, "positive_text": str}}`
- After (real): `{"conditioning": {"text_embeds": torch.Tensor, "negative_text_embeds": torch.Tensor?}}`

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | worker/nodes/encoder.py | Replace NotImplementedError real branch with tokenization + encoding logic; update markers; update docstrings; remove defers_to comment |
| MODIFY | worker/tests/test_nodes_encoder.py | Add real-mode tests (4 new tests); update REAL_PATH_VERIFIED marker reference |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| test_nodes_encoder.py | test_clip_text_encode_class_attributes | Class attributes match spec | None (mock-compatible) | N/A | All assertions pass | `python -m pytest worker/tests/test_nodes_encoder.py::test_clip_text_encode_class_attributes -v` exits 0 |
| test_nodes_encoder.py | test_clip_text_encode_mock_returns_sentinel | Mock execute returns sentinel dict | None | clip={}, positive_text="a red fox" | {"conditioning": {"mock": True, "positive_text": "a red fox"}} | `python -m pytest worker/tests/test_nodes_encoder.py::test_clip_text_encode_mock_returns_sentinel -v` exits 0 |
| test_nodes_encoder.py | test_clip_text_encode_mock_without_negative_text | Optional negative_text omitted works | None | clip={}, positive_text="hello" | {"conditioning": {"mock": True, "positive_text": "hello"}} | `python -m pytest worker/tests/test_nodes_encoder.py::test_clip_text_encode_mock_without_negative_text -v` exits 0 |
| test_nodes_encoder.py | test_clip_text_encode_in_registry | Node registered in NODE_REGISTRY | None | N/A | NODE_REGISTRY["ClipTextEncode"] exists | `python -m pytest worker/tests/test_nodes_encoder.py::test_clip_text_encode_in_registry -v` exits 0 |
| test_nodes_encoder.py | test_clip_text_encode_real_positive_only (real_mode) | Real encode with positive_text only produces valid conditioning | qwen3 fixture loaded, torch available | Loaded Qwen3TextEncoder, positive_text="a red fox" | {"conditioning": {"text_embeds": torch.Tensor}} with shape (1, 77, 64) | `python -m pytest worker/tests/test_nodes_encoder.py::test_clip_text_encode_real_positive_only -v -m real_mode` exits 0 |
| test_nodes_encoder.py | test_clip_text_encode_real_with_negative (real_mode) | Real encode with both texts produces both embeds | qwen3 fixture loaded, torch available | Loaded Qwen3TextEncoder, positive_text="fox", negative_text="dog" | {"conditioning": {"text_embeds": Tensor, "negative_text_embeds": Tensor}} | `python -m pytest worker/tests/test_nodes_encoder.py::test_clip_text_encode_real_with_negative -v -m real_mode` exits 0 |
| test_nodes_encoder.py | test_clip_text_encode_real_negative_omitted (real_mode) | Omitting negative_text omits negative_text_embeds key | qwen3 fixture loaded, torch available | Loaded Qwen3TextEncoder, positive_text="fox" | {"conditioning": {"text_embeds": Tensor}} without "negative_text_embeds" key | `python -m pytest worker/tests/test_nodes_encoder.py::test_clip_text_encode_real_negative_omitted -v -m real_mode` exits 0 |
| test_nodes_encoder.py | test_clip_text_encode_real_conditioning_shape (real_mode) | Conditioning tensor has expected shape | qwen3 fixture loaded, torch available | Loaded Qwen3TextEncoder, positive_text="test" | text_embeds.shape == (1, 77, 64) | `python -m pytest worker/tests/test_nodes_encoder.py::test_clip_text_encode_real_conditioning_shape -v -m real_mode` exits 0 |

## CI Impact

No CI changes required. The new tests are real-mode-only (marked `@pytest.mark.real_mode`), so they are collected and run by the existing `worker-linux-real` and `worker-windows-real` CI jobs. They are excluded from mock-mode jobs by the `real_mode` marker. No new file types, gates, or CI configurations are introduced.

## Platform Considerations

None identified. The Windows cross-check in ENVIRONMENT.md section 7 is sufficient. The tokenization and encoding logic uses `torch` and `transformers` which are cross-platform. The `max_length=77` and `padding="max_length"` arguments are standard and platform-independent. No `# cfg` guards needed -- torch and transformers work identically on Linux and Windows.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The Qwen3TextEncoder's forward() may expect a specific device (cuda vs cpu) and the test fixture loads on cpu -- if the encoder's parameters are on a different device than the input tensor, forward() will raise a device mismatch error. | Medium | High | Ensure the test sets `device="cpu"` in the NodeContext (already the default in `_make_ctx()`). The fixture's encoder parameters are loaded on cpu by `qwen3.load()` when called with `device="cpu"`. Verify this in the first real-mode test. |
| The tokenizer's `max_length` parameter may not exist or may behave differently across `transformers` versions. | Low | Medium | Use `max_length=77` which is a standard CLIP convention supported by all recent transformers versions. If the tokenizer doesn't support `max_length`, fall back to the tokenizer's own `model_max_length` or `max_len` attribute. |
| The fixture checkpoint's tensor shapes may not fully match the constructed Qwen3TextEncoder's expected shapes, causing `load_state_dict()` to skip keys (as happens in `qwen3.py`'s load function). | Low | Low | The fixture is already used by `LoadClip`'s real-mode test and works correctly. The same fixture produces a valid encoder that can be passed to `ClipTextEncode`. |
| `clip_encoder.tokenizer` attribute access may raise `AttributeError` if the tokenizer wasn't attached during load. | Low | High | This is guaranteed by `qwen3.py`'s `load()` function (line ~877). The test should verify this by checking the encoder has the attribute. If it fails, investigate the fixture's load path. |

## Acceptance Criteria

- [ ] `python -m pytest worker/tests/test_nodes_encoder.py -v -m real_mode` exits 0 with >=8 total tests
- [ ] `python -m pytest worker/tests/test_nodes_encoder.py -v` exits 0 (all tests, mock + real)
- [ ] `grep "REAL_PATH_VERIFIED:" worker/nodes/encoder.py` returns a test name that exists and passes under `real_mode`
- [ ] `grep "MOCK_PATH_VERIFIED:" worker/nodes/encoder.py` returns `test_clip_text_encode_mock_returns_sentinel` which exists and passes under mock mode
- [ ] `python -m py_compile worker/nodes/encoder.py` exits 0
- [ ] `python -m py_compile worker/tests/test_nodes_encoder.py` exits 0
