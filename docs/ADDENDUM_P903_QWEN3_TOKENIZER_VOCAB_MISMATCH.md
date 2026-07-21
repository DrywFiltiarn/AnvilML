# Addendum: P903 — P24-F1 Runnable Proof: Six Defects Found Getting the First
# Real End-to-End Generation Job to Actually Complete

**Status:** Resolved this session, as a series of manual patches (not
Forge-executed tasks) — per this project's own convention that completed
phases (Phase 24 here) are historical record and fixes land as documented
manual patches or P900-series retrofit tasks, never direct edits pretending
the original phase always looked this way. Recorded here in the same spirit
as `ADDENDUM_P901_MANUAL_RETROFIT.md` and
`ADDENDUM_P902_FIXTURE_COMPLETENESS_AND_QWEN3_LOADING.md`. Unlike those two,
no corresponding P903 Forge tasks were authored — the project owner judged a
synthetic task-graph entry for after-the-fact bugfixing to have no
significant relevance here, so this addendum is the only record.

**Origin:** actually attempting `P24-F1`'s Runnable Proof for the first
time — submitting `ANVILML_DESIGN.md` Appendix B.2's example graph via
`POST /v1/jobs` against the real fixture checkpoints, in real (non-mock)
mode. This was the first time *any* task's Runnable Proof exercised the
complete `LoadModel → LoadVae → LoadClip → EmptyLatent → ClipTextEncode →
Sampler → VaeDecode → SaveImage` chain through the actual dispatch pipeline
end to end. Six independent defects surfaced, one at a time, each only
reachable once the previous one was fixed — every one of them was already
"passing" its own phase's test suite, because no single existing test
exercised more than one or two adjacent nodes in the real chain at once.

---

## 1. `anvilml.toml`'s `[[model_dirs]]` never configured

The checked-in `anvilml.toml` shipped with `path = "./models"` commented out
entirely — no model directory was ever configured, so the scanner found
nothing to register. Separately, the scanner's `infer_kind()`
(`anvilml-registry/src/scanner.rs`) matches only the exact leaf directory
names `diffusion`, `text_encoders` (plural), and `vae` — a naming
convention not obvious from `anvilml.toml`'s own comments.

**Fix:** three explicit `[[model_dirs]]` entries added, pointing at
`./models/diffusion`, `./models/text_encoders`, and `./models/vae`.

---

## 2. `worker/executor.py` never resolved node-output references

`execute_graph()` passed each node's raw `inputs` dict straight through to
`execute()`, including entries shaped `{"node_id": "...", "output_slot":
"..."}` (the format every non-trivial edge in Appendix B.2's example graph
uses — there is no top-level `"edges"` array in that graph shape, so
`dag.rs`'s validation and this project's whole node-input format expect
inline references resolved at dispatch time). Every downstream node beyond
the first would have received a literal reference dict instead of the
actual prior node's output value.

**Fix:** `execute_graph()` now resolves each input value shaped
`{"node_id", "output_slot"}` against `results` (the accumulated per-node
output dict) before calling `execute()`; scalar values pass through
unchanged.

---

## 3. `ctx.pipeline_cache` was `None`, then per-job instead of per-process

`worker_main.py`'s `_dispatch_loop()` originally passed `pipeline_cache=None`
into every job's `NodeContext` — `LoadModel`/`LoadVae`/`LoadClip` all call
`ctx.pipeline_cache.get_or_load(...)` directly (`loader.py`), so this was an
unconditional `AttributeError` on the very first loader node of any real
job. Fixed in an intermediate patch to construct a `PipelineCache()` — but
inside `run_execute()`'s `ctx_factory` closure, i.e. freshly on **every**
`Execute` message, not once per worker process. `pipeline_cache.py`'s own
module docstring has always described this cache as scoped to "a single
worker process lifetime"; constructing it per-job silently defeated that —
every job would reload every model from disk from scratch regardless of
whether an identical `model_id` had already been loaded by a prior job on
the same worker.

**Fix:** `PipelineCache()` is now constructed once in `_dispatch_loop()`,
before the `while True:` message loop (which runs for the worker process's
entire lifetime), and every job's `ctx_factory` closure references that
one shared instance. This is now consistent with `zit.py`'s own
pipeline-*assembly* cache (`pipeline_cache = PipelineCache()` at module
scope in `zit.py`), which was already correctly process-scoped from Phase
21 onward.

---

## 4. `qwen3.py::load()` always attached the production tokenizer

`load()` hardcoded the tokenizer path to `worker/assets/qwen3_tokenizer/`
(the real, ~151k-token production Qwen3 tokenizer) regardless of which
checkpoint was actually loaded. Against the tiny synthetic fixture
(`qwen3_tiny.safetensors`, `vocab_size=128`), any token ID the production
tokenizer emitted above 128 indexed outside `embed_tokens`'s table —
`IndexError: index out of range in self`, raised deep inside
`Qwen3TextEncoder.forward()`.

This exact mismatch was already hit once, during `P24-A2`'s own
implementation, and "fixed" only inside the test harness:
`test_nodes_encoder.py` swaps `clip_encoder.tokenizer` for a purpose-built
`worker/assets/qwen3_tiny_tokenizer/` asset *after* `load()` returns, never
touching `load()` itself. Every `P24` test passed because every test went
through that monkeypatch; nothing exercised `load()`'s real, unpatched
tokenizer-selection logic until `P24-F1` ran with no test harness in front
of it.

**Fix:** `qwen3.py` gained `_load_tokenizer_matching_vocab(vocab_size)`,
called from `load()` with `hyperparams["vocab_size"]`. It tries each
vendored tokenizer (`qwen3_tokenizer`, then `qwen3_tiny_tokenizer`) in
turn and returns the first whose `len(tokenizer)` matches the checkpoint's
inferred `vocab_size`, raising a clear `RuntimeError` — naming every
candidate tried and its actual vocab size — if none match, rather than
deferring to a cryptic mid-forward `IndexError`. This generalizes correctly
rather than special-casing "is this the test fixture": any real Qwen3
checkpoint's `embed_tokens` row count and its tokenizer's vocabulary must
agree, by construction, for any properly paired model+tokenizer.

---

## 5. `worker/assets/qwen3_tiny_tokenizer/` was itself malformed

Wiring in `qwen3_tiny_tokenizer` (finding 4) surfaced that the asset never
actually worked: `tokenizer_config.json` declares
`"tokenizer_class": "BertTokenizer"`, which requires a plain-text
`vocab.txt` (one token per line). `P24-A2` instead committed `vocab.json` (a
JSON dict — the format BPE tokenizers like GPT2/RoBERTa use, not
`BertTokenizer`) and an empty `merges.txt` (a BPE artifact, irrelevant to
`BertTokenizer`). With no `vocab.txt` present, `BertTokenizer.from_pretrained()`
silently fell back to only the five special tokens explicitly declared in
`special_tokens_map.json` (`<unk>`, `<pad>`, `<s>`, `</s>`, `<mask>`) —
`len(tokenizer) == 5`, not 128. This is also why `P24-A2`'s own real-mode
tests never caught it: with a 5-token vocabulary, every real word in the
test prompt (`"a red fox"`) degenerated to `<unk>`, which happens to be a
valid embedding-table index regardless of vocabulary size, so the test
passed while testing nothing about actual tokenization.

**Fix:** the same 128-token vocabulary (26 lowercase letters, a space, the
5 special tokens, and 96 `<pad_N>` filler tokens — content was already
correct) converted from `vocab.json` into a properly-formatted `vocab.txt`;
`vocab.json` and the unused `merges.txt` removed. Verified directly:
`len(tokenizer) == 128`, and tokenizing `"a red fox"` now produces valid
in-range IDs (`<s> a <unk> <unk> </s>` → `[29, 0, 27, 27, 30]`) instead of
silently degenerating.

---

## 6. `EmptyLatent`'s real branch allocated the latent at the wrong dtype

`loader.py`'s `EmptyLatent.execute()` real branch called
`torch.zeros(latent_shape, device=ctx.device)` with no `dtype=` argument,
defaulting to `float32`. The loaded `ZiTModel`, however, is materialized at
whatever dtype `_select_dtype()` chose per `ANVILML_DESIGN.md §11.5`'s fixed
precedence — `bfloat16` on CPU, always (fp8 fails CPU's capability probe,
bf16 succeeds). The first layer the latent touches, `input_proj` (an
`nn.Linear` inside the bf16 model), rejected the fp32 input:
`RuntimeError: mat1 and mat2 must have the same dtype, but got Float and
BFloat16`.

**Fix:** `EmptyLatent`'s real branch now reads
`next(model.parameters()).dtype` from the already-loaded model (already a
required real-mode input, used moments earlier to call
`compute_latent_shape()`) and allocates the latent tensor at that dtype.
Traced the rest of `zit.py::forward()` to confirm this is the *only* place
needing a fix — the sinusoidal timestep embedding already does
`.to(x.dtype)` before use, so once the latent starts out at the correct
dtype, every downstream layer follows it naturally.

---

## 7. `zit.py::sample()` never called `scheduler.scale_model_input()`

The denoising loop called `pipeline.model(latent, t / 1000.0, ...)` with the
raw, unscaled latent on every step. `EulerDiscreteScheduler` (like most
sigma-based diffusers schedulers) requires the sample to be scaled by the
current timestep's sigma via `scheduler.scale_model_input()` before it's
fed to the model — omitting this doesn't raise an exception, it just
silently produces incorrect denoising. diffusers only ever surfaces this as
a runtime `UserWarning` ("The `scale_model_input` function should be
called before `step`..."), logged once per timestep, easy to miss in a
busy log stream. The pipeline ran to completion and produced an image on
every prior attempt that got this far — just a silently wrong one, on
every single step.

**Fix:** `scaled_latent = scheduler.scale_model_input(latent, t)` is now
computed once per timestep and passed to both the unconditional and
conditional model forward passes. The unscaled `latent` is left untouched
for `scheduler.step()` at the end of the loop, which correctly expects the
previous *unscaled* sample per diffusers' standard usage pattern.

---

## 8. `SaveImage`'s real branch assumed a bare `Image`, but `VaeDecode` emits a list

`decode.py`'s `VaeDecode` real branch is deliberately batch-capable and
always returns `list[PIL.Image.Image]`, even for `batch_size=1` — this is
tested and intentional (`test_vae_decode_real_batched_latent` explicitly
asserts `len(result["image"]) == 2` for a batch-2 latent, `P24-B2`).
`image.py`'s `SaveImage` real branch, separately, calls `.save()` directly
on `inputs["image"]`, assuming a bare `Image` — also tested and intentional
in isolation (`test_nodes_image.py`'s real-mode tests pass a bare
`pil_image`, never a list). Neither node was wrong on its own; the mismatch
only appeared once a real graph actually wired `VaeDecode`'s output into
`SaveImage`'s input end to end for the first time: `AttributeError: 'list'
object has no attribute 'save'`.

An initial attempt fixed this the other way — unwrapping the list inside
`VaeDecode` — but that directly breaks `test_vae_decode_real_batched_latent`
and two sibling tests that deliberately assert the batch-list contract.
Reverted before finalizing.

**Fix:** `SaveImage`'s real branch now accepts either shape: a bare `Image`
passes through unchanged (preserving every existing test), and a
`list[Image]` is unwrapped to its first element, with a warning logged if
more than one image was provided and silently dropped — the current MVP
graph contract (`ANVILML_DESIGN.md §10.3`'s node table) has no
batched-`IMAGE` concept anywhere downstream of `SaveImage`, so this is the
correct behavior for now rather than a design gap this node can fix on its
own.

---

## Verification

Findings 1–3 and 6–7 were verified by directly re-running `P24-F1`'s
Runnable Proof end to end against a live server in real mode after each
fix, observing the failure move one node further down the graph each time.
Findings 4–5 were additionally verified in isolation (outside a full torch
environment, since the tokenizer-loading path itself needs only
`transformers`): `_load_tokenizer_matching_vocab(128)` resolves to
`qwen3_tiny_tokenizer` and reports `len(tokenizer) == 128`; tokenizing
`"a red fox"` produces valid in-range IDs.

`P24-F1`'s Runnable Proof now completes end to end: `POST /v1/jobs` →
`Completed` → `GET /v1/artifacts/:hash` returns a real, valid PNG at the
requested `64×64` dimensions. The image itself is a small, incoherent,
colorful block pattern rather than anything resembling a photograph — this
is the correct and expected result, not a new defect: every model in the
chain (ZiT diffusion, Qwen3 CLIP, ZiT VAE) is the tiny synthetic fixture
checkpoint with randomly-initialized weights (`torch.manual_seed(42)`, per
`P902`'s fixture rebuild), never trained on anything. `P24-F1`'s proof is
about the pipeline being genuinely real end to end — real dispatch, real
tensors, real dtypes, real PNG bytes — not about generating a recognizable
image.

Existing test suites were re-run after each fix to confirm no regression:
`test_arch_clip_qwen3.py`, `test_nodes_encoder.py`, `test_worker_main.py`,
`test_nodes_loader.py`, `test_nodes_image.py`, `test_nodes_decode.py`,
`test_arch_zit.py`, and `test_nodes_sampler.py` all pass unchanged under
`ANVILML_WORKER_MOCK=1 -m "not real_mode"`.

---

## Where this is reflected in this delivery

- `anvilml.toml` — `[[model_dirs]]` entries (finding 1).
- `worker/executor.py` — node-output reference resolution in
  `execute_graph()` (finding 2).
- `worker/worker_main.py` — `PipelineCache()` moved to once-per-process
  construction (finding 3).
- `worker/nodes/arch/clip/qwen3.py` — `_load_tokenizer_matching_vocab()`
  (finding 4).
- `worker/assets/qwen3_tiny_tokenizer/` — `vocab.txt` replacing the
  malformed `vocab.json`/`merges.txt` (finding 5).
- `worker/nodes/loader.py` — `EmptyLatent`'s real branch allocates the
  latent at the model's actual dtype (finding 6).
- `worker/nodes/arch/diffusion/zit.py` — `scheduler.scale_model_input()`
  call added to the denoising loop (finding 7).
- `worker/nodes/image.py` — `SaveImage`'s real branch accepts either a
  bare `Image` or `VaeDecode`'s `list[Image]` (finding 8).
- `docs/RUNNABLE_PROOF.md` — Phase 24 entry annotated as verified passing.
- `docs/PHASES.md` — amendments log entry pointing to this addendum.
- `docs/PHASES_GRAPH.md` — `Known Wiring Gaps Closed` table gained items
  29–36 for these findings.
