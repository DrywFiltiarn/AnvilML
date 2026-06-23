# KNOWN_ISSUES.md — Real-path harness findings

Found by reading the live `worker/` source (fresh clone, not project knowledge
or memory paraphrase) while building the real-path node verification harness
in this directory. These are not the harness's job to fix — they're listed
here because each one is a hard blocker for at least one harness script, and
each script's docstring cross-references the relevant item number below.

None of these are caught by the existing `worker/tests/` suite, because
`worker/tests/conftest.py` forces `ANVILML_WORKER_MOCK=1` for every test via
an autouse fixture — the mock-mode branches these bugs live downstream of
are never exercised by `pytest`.

---

## 1. `qwen3.py` / `clip_l.py` resolve the wrong tokenizer asset directory

**Files:** `worker/nodes/arch/clip/qwen3.py:102`, `worker/nodes/arch/clip/clip_l.py:102`

Both use:
```python
tokenizer_dir = Path(__file__).parent.parent / "assets" / "qwen25_tokenizer"
```

From `worker/nodes/arch/clip/qwen3.py`, `.parent.parent` resolves to
`worker/nodes/arch/` — so the constructed path is
`worker/nodes/arch/assets/qwen25_tokenizer`, which does not exist. The real
location is `worker/assets/qwen25_tokenizer` (confirmed via `find` on the
live repo), which requires **three** `.parent` calls from this file, not two.

`worker/nodes/arch/clip/t5.py:105` uses `.parent.parent.parent` and an inline
comment explicitly noting the one-level correction:
```python
# Note: the plan originally specified parent.parent, but the
# actual assets directory is one level higher...
tokenizer_dir = Path(__file__).parent.parent.parent / "assets" / "t5_tokenizer"
```

So the fix was made once, correctly, in `t5.py`, but never propagated to its
two siblings. This matches a fix already recorded as applied elsewhere
(`worker/nodes/arch/clip/qwen3.py` tokenizer path, 4× `.parent` from
`__file__`) — that fix does not appear to be present in this clone's
`qwen3.py`, which still has 2×. Worth reconciling which is actually current
before assuming either description is up to date.

**Surfaced by:** `01_loaders.py` (`LoadClip`), and transitively by
`02_clip_encode.py`, `04_sampler.py`.

**Fix:** change both to `.parent.parent.parent`.

---

## 2. `EmptyLatent`'s real path references an unbound name `ctx`

**File:** `worker/nodes/sampler.py:182-184`

```python
return {"latent": torch.randn(
    shape, dtype=torch.float32, device=ctx.device
)}
```

`execute()` is an instance method; the context is `self.ctx`, not `ctx`.
There is no local or enclosing `ctx` in scope — this is a plain `NameError`
the moment the real branch is reached with `model` provided.

**Surfaced by:** `03_empty_latent.py`, and transitively by `04_sampler.py`,
`05_vae_decode.py` (both call `EmptyLatent` to build a real latent).

**Fix:** `device=self.ctx.device`.

---

## 3. `Conditioning` lacks `.tokenizer` / `.text_encoder` that `zit.py` expects

**Files:** `worker/nodes/encoder.py` (`Conditioning` class), `worker/nodes/arch/diffusion/zit.py:309-310`

`zit.py`'s `sample()` → `loader_fn()`:
```python
tokenizer = getattr(conditioning, "tokenizer", None)
text_encoder = getattr(conditioning, "text_encoder", None)
```

`Conditioning.__init__` only sets `.positive` and `.negative` — there is no
`.tokenizer` or `.text_encoder` attribute anywhere on the object `ClipTextEncode`
produces. `getattr(..., None)` means this doesn't raise; it silently resolves
to `None`, and `ZImagePipeline` gets constructed with `tokenizer=None,
text_encoder=None`. Whether `ZImagePipeline.__call__` actually needs those at
generation time (as opposed to only at the embedding-construction step
`ClipTextEncode` already performed) is the open question — the embeds are
already computed and live in `conditioning.positive` / `.negative`, so it's
plausible the pipeline never touches `.tokenizer`/`.text_encoder` again. This
needs a design decision, not a guess:

- If `ZImagePipeline` never reads `tokenizer`/`text_encoder` post-embedding,
  this is harmless and the `getattr` is just defensive. No fix needed beyond
  maybe removing the unused extraction.
- If it does read them (e.g. for tokenizer-dependent length checks inside
  `__call__`), `loader_fn()` needs to source them from `clip` (the `RealClip`
  object), not from `conditioning` — but `clip` isn't currently passed into
  `sample()` at all, only `conditioning`, `vae`, `model`. That would require
  a signature change.

**Surfaced by:** `04_sampler.py`, call site A. The script reports what
`ZImagePipeline` actually does with `tokenizer=None` rather than guessing.

**Fix:** pending design confirmation — see above.

---

## 4. `cancel_flag.is_set()` assumes `threading.Event`; production passes `list[bool]`

**Files:** `worker/nodes/arch/diffusion/zit.py` (`_make_callback`, calls `cancel_flag.is_set()`), `worker/worker_main.py:48` (`_cancel_flag: list[bool] = [False]`)

`worker_main.py` constructs and threads through a plain `list[bool]`:
```python
_cancel_flag: list[bool] = [False]
...
cancel_flag=_cancel_flag,
```

But `zit.py`'s `_make_callback()` closure does:
```python
if cancel_flag.is_set():
```

A `list[bool]` has no `.is_set()` method — this is `AttributeError` on the
first denoising step, every time, in production, once D18c's callback wiring
is live. `_make_callback`'s own docstring says the cancel_flag is "expected
to be a `threading.Event` (as specified in `ANVILML_DESIGN.md §1550`)" — so
either `ANVILML_DESIGN.md §1550` and `worker_main.py`'s actual implementation
disagree, or the design doc was changed after `worker_main.py` was written
and `worker_main.py` wasn't updated to match.

This is the highest-priority item of the four: items 1–3 only block specific
nodes; this one blocks **every** real Sampler invocation, for every
architecture, permanently, until one side changes.

**Surfaced by:** `04_sampler.py`, call site A, the moment `_make_callback`'s
returned closure is actually invoked by a real `ZImagePipeline.__call__`
step (i.e. once D18c's pipeline invocation lands — until then this bug is
latent because the pipeline is assembled but never called).

**Fix:** pick one:
- (a) `worker_main.py` switches `_cancel_flag` to `threading.Event()`, and
  the `_cancel_flag[0] = False` / `= True` assignments become `.clear()` /
  `.set()`; or
- (b) `zit.py`'s `_make_callback` switches to `cancel_flag[0]` list-index
  semantics.

Recommend (a): `threading.Event` is the documented design
(`ANVILML_DESIGN.md §1550`) and is the more idiomatic primitive for
cross-thread cancellation signaling than a single-element list.

---

## 5. `LoadClip.execute()` calls `torch.bfloat16` without importing `torch`

**File:** `worker/nodes/loader.py:618`

```python
return module.load(model_id, torch_dtype=torch.bfloat16)
```

This line sits directly in `LoadClip.execute()`'s real-mode branch. Every
other real-mode loader function in this file (`_load_model_from_hf_directory`,
the `LoadVae.execute()` closure, `_load_clip_from_hf_directory`) does its own
local `import torch` immediately before using it, per the module's own
documented convention ("any real-mode loading code must import these
packages lazily inside the non-mock code path"). `LoadClip.execute()` is the
one place that convention was skipped — there is no `import torch` anywhere
in its scope.

This means `LoadClip` will raise `NameError: name 'torch' is not defined`
on **any** real invocation, regardless of whether the tokenizer-path bug
(item 1) is fixed first — this fires at the call into `module.load()`, but
the `torch_dtype=torch.bfloat16` argument is evaluated *before* `load()` is
even entered, so it's actually the very first thing to break, ahead of
item 1.

**Confirmed via direct testing**, not inferred from a missing-package
environment: traced to `NameError: name 'torch' is not defined` at exactly
this line, independent of whether `safetensors`/`diffusers` are installed.

**Surfaced by:** `01_loaders.py` (`LoadClip`) — will likely mask item 1
until this is fixed first, since it fires earlier in the call chain.

**Fix:** add `import torch` inside `LoadClip.execute()`'s real-mode branch,
before line 618 (matching the lazy-import convention used everywhere else
in this file).
