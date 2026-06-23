# Plan Report: P18-D16

| Field | Value |
|-------|-------|
| Task ID | P18-D16 |
| Phase | 018 — ZiT Generic Nodes |
| Description | worker/nodes/encoder.py: ClipTextEncode real text encoding path |
| Depends on | P18-D14 |
| Project | anvilml |
| Planned at | 2026-06-23T09:45:00Z |
| Attempt | 1 |

## Objective

Replace `ClipTextEncode.execute()`'s bare `NotImplementedError` with a working real-mode text encoding path that calls the loaded CLIP object's `encode()` method to produce `prompt_embeds`/`negative_prompt_embeds` tensors, and returns a `Conditioning` object with `.positive` and `.negative` attributes matching the contract `ZImagePipeline.__call__` expects (via `encode_prompt`). In mock mode (`ANVILML_WORKER_MOCK=1`), existing behavior is preserved — the same `MockConditioning(text=...)` sentinel is returned so all existing tests pass unchanged.

## Scope

### In Scope
- Add `encode(self, text: str, negative_text: str = "") -> tuple[list[torch.FloatTensor], list[torch.FloatTensor]]` method to `RealClip` class in `worker/nodes/loader.py`. This method tokenizes text (using `tokenizer.apply_chat_template` for Qwen3-style tokenizers, or direct `tokenizer()` for CLIP-L/T5), runs through `text_encoder`, extracts `hidden_states[-2]`, filters by attention mask, and returns `(positive_embeds, negative_embeds)` as `list[torch.FloatTensor]` per `ZImagePipeline._encode_prompt`'s return type.
- Create `Conditioning` class in `worker/nodes/encoder.py` with `.positive` and `.negative` attributes (each `list[torch.FloatTensor]`). Add to `__all__`.
- Replace `ClipTextEncode.execute()`'s real-mode `NotImplementedError` with a call to `clip.encode(text, negative_text)` producing `Conditioning(positive, negative)`. All `torch`/`transformers`/`diffusers` imports must be lazy (inside the non-mock guard).
- Add inline comment explaining dual-conditioning construction (positive + negative for classifier-free guidance).
- Update module docstring in `encoder.py` to describe the real-mode encoding path.
- Add `Conditioning` to `encoder.py`'s `__all__`.
- Add a test for the `Conditioning` class structure (`.positive` and `.negative` attributes).
- Update `docs/TESTS.md` with the new test entry.

### Out of Scope
None. `defers_to (from JSON): []` — this task must implement its full scope.

## Existing Codebase Assessment

**What already exists:** `ClipTextEncode` is fully defined with correct metadata attributes (`NODE_TYPE`, `INPUT_SLOTS`, `OUTPUT_SLOTS`), registered via `@register`, and has four passing mock-mode tests. `RealClip` (in `loader.py`) wraps a tokenizer and text_encoder with `.tokenizer` and `.text_encoder` properties. The `MockConditioning` sentinel carries a `.text` attribute but has no `.positive`/`.negative` structure. The `conftest.py` autouse fixture sets `ANVILML_WORKER_MOCK=1` for every test.

**Established patterns:** (1) Lazy imports inside the non-mock code path — never at module top level. (2) `RealModel`/`RealClip` wrapper pattern: private `_`-prefixed refs, public properties, unified interface. (3) `os.environ.get("ANVILML_WORKER_MOCK") == "1"` runtime check for mock gating. (4) Google-style docstrings on all public classes and methods. (5) Inline comments explaining non-obvious decisions (chat template usage, hidden state extraction). (6) Tests use `importlib.reload()` to re-register nodes against a cleared `NODE_REGISTRY`.

**Gap between design doc and source:** The design doc (§10.3) states `ClipTextEncode` should call `clip.encode(text)` — an arch-agnostic interface on the CLIP object — but `RealClip` currently has no `encode` method. This task fills that gap. Additionally, the conditioning object contract (`.positive`/`.negative`) is documented in the task context but does not yet exist in the codebase.

## Resolved Dependencies

| Type | Name | Version verified | MCP source | Feature flags confirmed |
|------|------|-----------------|------------|------------------------|
| python | diffusers | 0.38.0 | pypi-query MCP (confirmed via installed package) | n/a |
| python | transformers | (installed, torch not available for import) | pypi-query MCP fallback | n/a |

Notes: diffusers 0.38.0 confirmed via `worker/.venv/bin/python -c "import diffusers; print(diffusers.__version__)"`. The `ZImagePipeline._encode_prompt` method returns `list[torch.FloatTensor]` per the source at `pipeline_z_image.py:198-247`. The `apply_chat_template` method is available on `Qwen2Tokenizer` (confirmed via `hasattr` check). `torch` is not installed in the venv (mock mode), so direct API verification of model classes is not possible — the ACT agent must confirm exact tensor shapes against the installed diffusers source at session start.

## Approach

1. **Add `encode` method to `RealClip` in `worker/nodes/loader.py`.**
   - Implement `def encode(self, text: str, negative_text: str = "") -> tuple[list[Any], list[Any]]` (return type uses `Any` for `torch.FloatTensor` since torch is not importable at module level in mock mode).
   - The method body:
     a. Check mock mode: if `os.environ.get("ANVILML_WORKER_MOCK") == "1"`, return `([], [])` — no real encoding in mock mode.
     b. Lazy-import `torch` and call `tokenizer.apply_chat_template()` with `messages=[{"role": "user", "content": text}]`, `tokenize=False`, `add_generation_prompt=True`, `enable_thinking=True` (matching `ZImagePipeline._encode_prompt`'s exact template parameters for Qwen3).
     c. Tokenize the templated text: `text_inputs = self.tokenizer(templated_text, padding="max_length", max_length=512, truncation=True, return_tensors="pt")`.
     d. Move inputs to device: `text_input_ids = text_inputs.input_ids.to(self._device)`, `prompt_masks = text_inputs.attention_mask.to(self._device).bool()`. (The device comes from `NodeContext.device`; store it on `RealClip` during construction — see step 2.)
     e. Run through text encoder: `hidden = self._text_encoder(input_ids=text_input_ids, attention_mask=prompt_masks, output_hidden_states=True).hidden_states[-2]`.
     f. Filter by attention mask: `[hidden[i][prompt_masks[i]] for i in range(len(hidden))]`.
     g. Repeat for `negative_text` (default `""`). If `negative_text` is empty string and classifier-free guidance is assumed (which `ZImagePipeline.__call__` always enables), encode it too to produce a negative embedding list.
     h. Return `(positive_embeds, negative_embeds)`.
   - Inline comment: "Dual-conditioning: ZImagePipeline uses classifier-free guidance (always enabled), so both positive and negative embeddings are required. The negative embeds are produced by encoding the negative_text string through the same text encoder pipeline."

2. **Add `_device` attribute to `RealClip.__init__`.**
   - Add `device: str = "cpu"` parameter to `RealClip.__init__` (optional, defaults to `"cpu"` for backward compatibility with mock mode which doesn't pass it).
   - Store as `self._device = device`.
   - This is needed by the `encode` method to move tensors to the correct device.

3. **Create `Conditioning` class in `worker/nodes/encoder.py`.**
   - Define `class Conditioning:` with `__init__(self, positive: list[Any], negative: list[Any]) -> None`.
   - Store `self.positive = positive` and `self.negative = negative`.
   - Google-style docstring describing the `.positive` and `.negative` attributes and their role in classifier-free guidance.
   - Add `"Conditioning"` to `__all__`.

4. **Replace `ClipTextEncode.execute()`'s real-mode path.**
   - In the non-mock branch, read `negative_text = inputs.get("negative_text", "")`.
   - Call `positive_embeds, negative_embeds = clip.encode(text, negative_text)`.
   - Return `{"conditioning": Conditioning(positive_embeds, negative_embeds)}`.
   - Inline comment on the dual-conditioning construction (as described in step 1).
   - Update the method's docstring `Raises` section to remove `NotImplementedError` and add no new raises (the encode method may raise from tokenizer/model errors, which propagate naturally).

5. **Update `encoder.py` module docstring.**
   - Add a sentence describing the real-mode encoding path: "In real mode, the CLIP object's `encode()` method is called to produce positive and negative embedding lists (matching `ZImagePipeline.__call__`'s `prompt_embeds`/`negative_prompt_embeds` contract), wrapped in a `Conditioning` object."

6. **Add test for `Conditioning` class structure.**
   - Test name: `test_conditioning_class_has_positive_negative`.
   - Import `Conditioning` from `worker.nodes.encoder`.
   - Create instance with two lists of tensors (mock tensors as `list` of `list` objects since torch is unavailable).
   - Assert `.positive` and `.negative` attributes exist and match the inputs.
   - This test runs in mock mode and does not require torch.

7. **Update `docs/TESTS.md` with the new test entry.**
   - Add one entry for `test_conditioning_class_has_positive_negative` under the encoder test section.

## Public API Surface

| Item | Path | Description |
|------|------|-------------|
| class | `worker.nodes.encoder.Conditioning` | New conditioning object with `.positive` and `.negative` attributes (`list[torch.FloatTensor]`). |
| method | `worker.nodes.loader.RealClip.encode(text: str, negative_text: str = "") -> tuple[list[Any], list[Any]]` | New encode method on RealClip producing positive and negative embedding lists. |
| param | `worker.nodes.loader.RealClip.__init__` gains `device: str = "cpu"` | Optional device parameter for tensor device placement. |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `worker/nodes/loader.py` | Add `encode()` method to `RealClip`, add `device` parameter to `__init__`. |
| Modify | `worker/nodes/encoder.py` | Create `Conditioning` class, replace `NotImplementedError` with real encoding path, update docstrings. |
| Modify | `docs/TESTS.md` | Add test entry for `test_conditioning_class_has_positive_negative`. |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `worker/tests/test_nodes_encoder.py` | `test_conditioning_class_has_positive_negative` | `Conditioning` class exposes `.positive` and `.negative` attributes matching constructor args | `ANVILML_WORKER_MOCK=1` (from conftest.py) | `Conditioning([1,2], [3,4])` | `.positive == [1,2]`, `.negative == [3,4]` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_encoder.py::test_conditioning_class_has_positive_negative -v` exits 0 |

## CI Impact

No CI changes required. The test file `worker/tests/test_nodes_encoder.py` already exists and is picked up by the existing `worker-linux` and `worker-windows` CI jobs. No new file types or test modules are introduced.

## Platform Considerations

None identified. The platform-neutral code path is the mock mode branch (which returns `([], [])`). The real mode branch uses `torch` which handles device placement internally. The `device` parameter defaults to `"cpu"` which is universally available. No `#[cfg(...)]` guards are needed (Python, not Rust). The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `apply_chat_template` signature differs between tokenizer versions — the `enable_thinking` parameter may not exist in older transformers versions. | Medium | High | At ACT time, confirm `enable_thinking` is available on the installed `Qwen2Tokenizer`. If absent, fall back to calling without it. The template format (user role + content + generation prompt) is stable across versions. |
| `hidden_states[-2]` indexing may differ for non-Qwen3 models (CLIP-L uses a single text tower, T5 uses encoder-decoder). | Medium | High | The `encode` method is on `RealClip` which wraps a specific tokenizer+encoder pair. For CLIP-L, the text encoder returns `last_hidden_state` not `hidden_states` — the ACT agent must confirm the exact attribute name for each model class and use conditional logic or a unified interface. Confirm against diffusers source at ACT time. |
| Mock mode tests may break if `Conditioning` is accidentally imported by the reloaded module in a way that conflicts with existing `MockConditioning` usage. | Low | Medium | The mock code path is unchanged — it returns `MockConditioning`, not `Conditioning`. The `importlib.reload()` pattern used by existing tests will re-execute the module body, defining both classes. No conflict expected. |

## Acceptance Criteria

- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_encoder.py -v` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -c "from worker.nodes.encoder import Conditioning; c = Conditioning([1], [2]); assert c.positive == [1] and c.negative == [2]"` exits 0
- [ ] `worker/.venv/bin/python -m py_compile worker/nodes/encoder.py worker/nodes/loader.py` exits 0
