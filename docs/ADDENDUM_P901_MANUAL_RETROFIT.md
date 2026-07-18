# Addendum: P901 Manual Retrofit — Device Propagation, Sampler's Unused `clip`
# Input, Negative Conditioning, Cache-Key Convention, CLIP Metadata Fallback,
# and the Arch-Module Mock-Marker Exception

**Status:** Resolved this session, as a manual patch (not a Forge-executed task) —
per this project's own convention that completed phases (Phases 21–24 here) are
historical record and fixes land as documented manual patches or P900-series
retrofit tasks, never direct edits pretending the original phase always looked
this way. Recorded here in the same spirit as `ADDENDUM_ENUMERATION_SOURCE_CPU.md`
and `ADDENDUM_DEMUX_FANOUT.md`.

---

## Background

A design-vs-implementation audit of Phases 21–24 (`worker/nodes/loader.py`,
`worker/nodes/sampler.py`, `worker/nodes/encoder.py`, `worker/nodes/decode.py`,
and the three arch modules) against `ANVILML_DESIGN.md` §10–11 and §17–18 found
six issues, none caught by existing tests because each is invisible from a
CPU-only, single-device CI environment (§2.2) or is a documentation/marker
inconsistency rather than a runtime crash. This addendum resolves all six.

---

## 1. `device` was never propagated into `load()` — real loads always ran on CPU

**Finding:** `LoadModel`, `LoadVae`, and `LoadClip` all called
`module.load(inputs["model_id"], ctx.caps)`, omitting the third `device`
argument. Every arch module's `load()` defaults `device: str = "cpu"`
(§10.4), so the omission was silent — every model load would land on CPU
regardless of the worker's actual assigned device on real GPU/ROCm hardware.
No existing test could catch this: every fixture test constructs
`ctx` with `device="cpu"` in the first place, so `param.device.type == "cpu"`
holds whether the loader passes the argument explicitly or not.

**Resolution:** All three loaders now call `module.load(inputs["model_id"],
ctx.caps, ctx.device)`.

```diff
-                    lambda: module.load(inputs["model_id"], ctx.caps),
+                    lambda: module.load(inputs["model_id"], ctx.caps, ctx.device),
```
(applied to `LoadModel`, `LoadVae`, `LoadClip` in `worker/nodes/loader.py`)

**Regression tests added** (`worker/tests/test_nodes_loader.py`):
`test_load_model_passes_ctx_device_to_arch_load`,
`test_load_vae_passes_ctx_device_to_arch_load`,
`test_load_clip_passes_ctx_device_to_arch_load`. Each patches the relevant
`get_module()` dispatcher to return a stub whose `load()` records its call
arguments, sets `ctx.device="cuda:0"`, and asserts that value — not the
default — was received. This is the specific test shape that a
`param.device.type == "cpu"` assertion cannot provide, since it can't
distinguish "explicitly passed" from "silently defaulted" when both agree.
Confirmed to fail against the pre-fix `loader.py` (`TypeError: missing 1
required positional argument: 'device'`, since the stub's `load()` requires
all three arguments) and pass against the fix.

**Documentation added:** `ANVILML_DESIGN.md` §11.6 gained an explicit
"Device propagation" paragraph stating the requirement and explaining why a
CPU-only CI environment cannot catch a regression here, so future sessions
don't need to rediscover this failure mode from scratch.

---

## 2. `Sampler`'s `clip` input was declared but never wired to `sample()`

**Finding:** `ANVILML_DESIGN.md` §10.3's table listed `clip: Clip` as a
`Sampler` input, and `sampler.py` declared a required `SlotSpec("clip",
"CLIP")` — but the real branch's call to `module.sample(...)` never
included it, and `zit.py`'s `sample()` has no `clip` parameter at all. Every
test passed a dummy `clip={}` and nothing checked it was used.

**Decision (per project owner):** Sampler does not need `clip`, for now —
it consumes fully-resolved `conditioning` (positive and negative) instead.

**Resolution:** Removed the `clip` slot from `Sampler.INPUT_SLOTS` (7 → 6
slots) and its docstrings, in `worker/nodes/sampler.py`. Removed the
corresponding `clip={}` argument from every call site in
`worker/tests/test_nodes_sampler.py` and updated the
`INPUT_SLOTS`-length/index assertions in `test_sampler_class_attributes`.
Updated `ANVILML_DESIGN.md` §10.3's `Sampler` table row to drop `clip:
Clip` from the inputs column, with a note pointing back here.

---

## 3. `ClipTextEncode`'s `negative_text_embeds` was produced but never consumed

**Finding:** `ClipTextEncode`'s real branch dutifully encodes `negative_text`
into `conditioning["negative_text_embeds"]` when provided, but `zit.py`'s
`sample()` CFG loop always used a fixed empty/no-conditioning pass for the
unconditional side of guidance — it never read `negative_text_embeds` at
all. A user supplying a negative prompt got it silently ignored.
(Investigating this also surfaced a second, more severe latent bug in the
same code: the conditional pass forwarded the *entire* conditioning value
— a dict, once `ClipTextEncode` is in the loop — straight into
`pipeline.model(..., conditioning=conditioning)`, whose `forward()` expects
a raw tensor for cross-attention K/V. Reproduced directly: passing a
`ClipTextEncode`-shaped dict through the pre-fix `sample()` raises
`AttributeError: 'dict' object has no attribute 'is_nested'` inside
`torch.nn.MultiheadAttention`. This means the `ClipTextEncode` → `Sampler`
wiring would not merely have ignored negative prompts — it would have
crashed outright the first time a real graph exercised it, which had not
yet happened since Phase 24's integration/proof tasks (`P24-E1`/`P24-F1`)
had not yet run.)

**Resolution:** Added `_resolve_conditioning(conditioning) -> (cond_embeds,
uncond_embeds)` to `worker/nodes/arch/diffusion/zit.py`: dict input splits
into its `text_embeds`/`negative_text_embeds` keys; a bare tensor or `None`
resolves to `(value, None)`, preserving prior behavior for callers that
bypass `ClipTextEncode`. `sample()`'s CFG loop now calls this once before
the denoising loop and uses `cond_embeds`/`uncond_embeds` for the
conditional/unconditional passes respectively, instead of the previous
inline dict-vs-tensor check that fed the raw dict to both passes.

```diff
+    cond_embeds, uncond_embeds = _resolve_conditioning(conditioning)
     for t in scheduler.timesteps:
         with torch.no_grad():
-            noise_pred_uncond = pipeline.model(latent, t / 1000.0)
+            noise_pred_uncond = pipeline.model(latent, t / 1000.0, conditioning=uncond_embeds)
         with torch.no_grad():
-            noise_pred_cond = pipeline.model(latent, t / 1000.0, conditioning=conditioning)
+            noise_pred_cond = pipeline.model(latent, t / 1000.0, conditioning=cond_embeds)
```

**Regression tests added:**
- `worker/tests/test_arch_zit.py`: three pure unit tests against
  `_resolve_conditioning()` directly (dict-with-negative,
  dict-without-negative, bare-tensor/`None` backward compatibility — none
  require torch or a loaded model), plus two real-mode integration tests
  (`test_sample_uses_negative_text_embeds_for_uncond_pass`,
  `test_sample_no_negative_conditioning_falls_back_to_none`) that patch the
  loaded model's `forward()` to record the `conditioning` argument per call
  and assert identity — sidestepping this fixture model's inherent
  run-to-run floating-point non-determinism (it is never placed in
  `eval()` mode, and the scheduler's ancestral sampling step consumes the
  global RNG; both are pre-existing properties of the fixture model, out of
  scope for this retrofit, and would have made an output-value comparison
  flaky).
- `worker/tests/test_nodes_sampler.py`:
  `test_sampler_real_forwards_dict_conditioning_untouched` — the node-level
  counterpart, proving `Sampler.execute()` forwards the conditioning dict
  to `sample()` unchanged rather than losing or repacking it in transit.

All five confirmed to fail against the pre-fix `zit.py` (either
`ImportError` for the not-yet-existing `_resolve_conditioning`, or the
`AttributeError` above) and pass against the fix.

---

## 4. Pipeline-cache key convention was inconsistent across the three loaders

**Finding:** `LoadVae`/`LoadClip` namespaced their component-cache keys as
`f"vae:{model_id}"` / `f"clip:{model_id}"`; `LoadModel` used the bare
`model_id` with no prefix — inconsistent with its siblings and with the
literal (if under-specified) text of `ANVILML_DESIGN.md` §11.6.

**Resolution (per project owner: prefer the `LoadVae`/`LoadClip` format):**
`LoadModel` now caches under `f"model:{inputs['model_id']}"`, uniform with
its siblings.

```diff
-                    inputs["model_id"],
+                    f"model:{inputs['model_id']}",
```

**Documentation:** `ANVILML_DESIGN.md` §11.6 rewritten to state the
`f"{kind}:{model_id}"` convention explicitly as mandatory and uniform
across all three loaders, and to distinguish it from `sample()`'s own
un-prefixed `f"{model_id}:pipeline"` pipeline-level cache key (a different,
intentionally un-namespaced cache, not a fourth sibling of the three
component caches).

No test asserted the old bare-key format, so no test changes were needed
beyond what's already covered by the existing `PipelineCache`-based loader
tests, which exercise the cache opaquely through `ctx.pipeline_cache` and
don't hardcode key strings.

---

## 5. CLIP family was missing the mandatory no-metadata regression fixture

**Finding:** `ANVILML_DESIGN.md` §17.5 mandates at least one fixture per
diffusion/CLIP/VAE family with no `arch` metadata key, to guard against the
historical `st.metadata` vs `st.metadata()` call-as-property bug. The
diffusion (`zit_tiny_no_metadata.safetensors`) and VAE
(`zit_vae_tiny_no_metadata.safetensors`) families both had this; the CLIP
family (`qwen3.py`) did not.

**Resolution:** Added `qwen3_tiny_no_metadata.safetensors`
(`worker/tests/fixtures/build_qwen3_fixture.py`) and
`test_load_no_metadata_real` (`worker/tests/test_arch_clip_qwen3.py`).
Deliberately **not** a byte-for-byte copy of `build_zit_fixture.py`'s
approach: `zit.py`'s shape inference tolerates non-recognizable key
prefixes (its no-metadata fixture uses an `xyz_`-prefixed key set), but
`qwen3.py`'s `_infer_hyperparams()` genuinely requires the real Qwen3 key
schema (`self_attn.{q,k,v,o}_proj.weight`, etc.) to compute `hidden_dim` —
confirmed directly: `load()` against an `xyz_`-prefixed qwen3 fixture
raises `ValueError: cannot infer hidden_dim ... no recognized attention
projection keys found`, before ever reaching the metadata-fallback code
this fixture exists to test. The qwen3 no-metadata fixture therefore reuses
the real key schema and omits only the `metadata={"arch": "qwen3"}`
argument to `save_file()` — this isolates the actual regression case
(a header lacking an `arch` metadata entry) without conflating it with a
separate, not-yet-generically-supported "unrecognized keys" case for this
family. This difference is documented inline in
`_no_metadata_tensors()`'s docstring so a future session doesn't
"correct" it back to the zit-style prefix-mangling approach and
reintroduce the `ValueError` above.

(Building the fixture initially clobbered the existing
`qwen3_tiny.safetensors` — `build()`'s original single-file form regenerates
both files from a fresh `torch.randn(...)` call with no seed, silently
producing different random weights each run. `build()` was corrected to
generate `qwen3_tiny.safetensors` and `qwen3_tiny_no_metadata.safetensors`
as two clearly separated steps, matching `build_zit_fixture.py`'s existing
pattern for the same two-fixture shape, and the original fixture file was
restored via `git checkout` before the corrected fixture was written.)

---

## 6. `MOCK_PATH_VERIFIED` markers on arch-module `load()`/`sample()` point at
   `real_mode`-marked tests — documented as an accepted exception, not fixed

**Finding:** In `zit.py` and `zit_vae.py`, `MOCK_PATH_VERIFIED` comments
point at tests (e.g. `test_load_mock_zit_fixture`,
`test_sample_seed_minus_one_resolves_random`) that are decorated
`@pytest.mark.real_mode` and import real torch — meaning they do not run
under `ANVILML_WORKER_MOCK=1`, which §17.2 requires of a genuine
"mock-mode test." Mechanically, §10.6 rule 2 is false for these five
markers, even though nothing crashes.

**Decision (per project owner): document as a deviation, not a defect —
this is a case where the otherwise-good standard doesn't apply cleanly.**
§11.2 already states `load()`/`sample()`/`decode()` are "real-mode-only by
nature" with no genuine mock branch to test — unlike a node's `execute()`,
which has two real, distinct code paths gated by `ctx.mock`. Forcing a
synthetic mock-path test for these three functions would test nothing real.

**Resolution:** `ANVILML_DESIGN.md` §10.6 gained a new subsection,
"Documented exception: arch-module `load()`/`sample()`/`decode()` have no
genuine mock-path test," formally carving out this case: for these three
functions only, `MOCK_PATH_VERIFIED` may name a collection-safety test (one
that imports the arch module's package under `ANVILML_WORKER_MOCK=1` with
no torch installed and asserts collection succeeds — genuinely
mock-mode-verifiable, and what the §11.2 import-guard requirement exists to
guarantee) rather than a test that exercises a nonexistent mock branch.
Every node's `execute()` is explicitly carved *out* of this exception and
must continue to satisfy the full marker rule, since those functions do
have a real `ctx.mock` branch.

No code or marker changes were made to `zit.py`/`zit_vae.py` for this
finding — the existing markers are now formally sanctioned by the updated
design text rather than rewired to a not-yet-written collection-safety
test. A future session adding a genuine collection-safety test for these
three functions may retarget the markers to it; that is optional
housekeeping, not a currently-required fix.

---

## Where this is reflected in this delivery

- `worker/nodes/loader.py` — findings 1, 4.
- `worker/nodes/sampler.py`, `worker/tests/test_nodes_sampler.py` — findings
  2, 3.
- `worker/nodes/arch/diffusion/zit.py`, `worker/tests/test_arch_zit.py` —
  finding 3.
- `worker/tests/fixtures/build_qwen3_fixture.py`,
  `worker/tests/fixtures/qwen3_tiny_no_metadata.safetensors`,
  `worker/tests/test_arch_clip_qwen3.py` — finding 5.
- `docs/ANVILML_DESIGN.md` §10.3, §10.6, §11.6 — findings 1 (documentation),
  2, 4, 6.
- `docs/TESTS.md` — catalogue entries for every test listed above, plus
  corrections to the two `test_sampler_*` entries whose `INPUT_SLOTS`
  count/inputs referenced the now-removed `clip` slot.

## Action required by the repository maintainer

None — all six findings are resolved directly in this delivery's diff
(code, tests, and design-doc text together), unlike
`ADDENDUM_ENUMERATION_SOURCE_CPU.md`'s design-doc-only deviation, which
required a separate hand-apply step. Apply the accompanying patch normally.
