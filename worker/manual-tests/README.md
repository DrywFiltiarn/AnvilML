# AnvilML real-path node verification harness

Not a pytest suite. `worker/tests/` is mock-only by design — `conftest.py`
forces `ANVILML_WORKER_MOCK=1` for every test via an autouse fixture, so it
structurally cannot exercise real `safetensors`/`torch`/`diffusers` code.
These scripts hook directly into the same node classes and arch dispatch
modules that `worker_main.py` calls in production, run them against real
model files with mock mode unset, and print the resulting object's type,
shape, dtype, and key attributes so you can eyeball whether the real path
produced something sane — or get a full traceback if it didn't.

Built against a fresh clone of `DrywFiltiarn/AnvilML` at the point where
P18-D18c is in progress (D16/D17/D18a/D18b landed; D18c in flight; D19/D20
not yet landed). Some scripts therefore document an *expected* failure
(`NotImplementedError`) until those land — see each script's docstring.

## Coverage

| Script | Node(s) / function exercised | Phase task |
|---|---|---|
| `01_loaders.py` | `LoadModel`, `LoadVae`, `LoadClip` | P18-D4/D5/D6, D12/D13/D14 |
| `02_clip_encode.py` | `ClipTextEncode` (+ `RealClip.encode()`) | P18-D16 |
| `03_empty_latent.py` | `EmptyLatent` real-mode branch | P18-D17 |
| `04_sampler.py` | `arch.diffusion.zit.sample()` directly, **and** `Sampler.execute()` | P18-D18a/b/c, D19 |
| `05_vae_decode.py` | `VaeDecode` | P18-D20 |

Run in order — each later script reuses the loader pattern proven by
`01_loaders.py` as its own prerequisite-building step, so if `01` fails for
a given node, the corresponding part of later scripts will also fail (and
will say so).

## Setup

```powershell
$env:ANVILML_MODELS_DIR="/path/to/models"
$env:ANVILML_ZIT_MODEL="zit_fp8.safetensors"
$env:ANVILML_ZIT_VAE="zit_vae.safetensors"
$env:ANVILML_ZIT_CLIP="qwen3_4b.safetensors"
$env:ANVILML_DEVICE="cuda:0"
```

```bash
export ANVILML_MODELS_DIR=/path/to/models
export ANVILML_ZIT_MODEL=zit_fp8.safetensors
export ANVILML_ZIT_VAE=zit_vae.safetensors
export ANVILML_ZIT_CLIP=qwen3_4b.safetensors
export ANVILML_DEVICE=cuda:0
```

`ANVILML_WORKER_MOCK` must be unset (or `"0"`) — every script hard-aborts
with a `FATAL` message if it detects `ANVILML_WORKER_MOCK=1`, since silently
falling back to mock sentinels would produce a meaningless pass.

## Run

```bash
cd worker   # or wherever your repo root is — scripts auto-detect it
python3 /path/to/this/dir/01_loaders.py
# ...or chain all five:
/path/to/this/dir/run_all.sh
```

Each script can also be copied directly into the repo (e.g. into a scratch
`worker/realpath_harness/` directory) and run from there — `_harness_common.py`
walks up from `cwd` and from its own location looking for a `worker/nodes/`
directory to add to `sys.path`.

## Known issues this harness will surface

Four real bugs were found by reading the live repo while building this
harness — none caught by the existing mock-only `pytest` suite. Full
detail, file/line references, and recommended fixes are in
[`KNOWN_ISSUES.md`](./KNOWN_ISSUES.md). Summary:

1. **`qwen3.py` / `clip_l.py`** resolve the tokenizer asset directory with
   `.parent.parent` (2 levels) — should be `.parent.parent.parent` (3
   levels), matching the already-correct `t5.py`. Breaks `LoadClip` for
   both Qwen3 and CLIP-L. *(Surfaces in `01_loaders.py`.)*
2. **`EmptyLatent`**'s real-mode branch in `sampler.py` references a bare
   `ctx` instead of `self.ctx` — plain `NameError`. *(Surfaces in
   `03_empty_latent.py`.)*
3. **`Conditioning`** has no `.tokenizer`/`.text_encoder`, but
   `arch/diffusion/zit.py`'s `sample()` reads both via `getattr(...,
   None)` — silently resolves to `None`; whether that's harmless depends
   on whether `ZImagePipeline.__call__` needs them post-embedding. Needs a
   design decision, not a blind fix. *(Surfaces in `04_sampler.py`.)*
4. **`cancel_flag.is_set()`** in `_make_callback` assumes
   `threading.Event`, but `worker_main.py` constructs `cancel_flag` as
   `list[bool]` (`[False]`). Will be `AttributeError` on the first real
   denoising step once D18c's callback is actually invoked by the
   pipeline. Highest priority of the four — blocks every real `Sampler`
   call for every architecture until one side changes. *(Latent until the
   pipeline invocation lands; will surface in `04_sampler.py` at that
   point.)*
5. **`LoadClip.execute()`** (`loader.py:618`) calls `torch.bfloat16`
   without ever importing `torch` in that scope — plain `NameError`,
   confirmed by direct execution, not inferred. Fires *before* item 1's
   bug gets a chance to, since the argument is evaluated ahead of
   `module.load()`. *(Surfaces in `01_loaders.py`, masks item 1 until
   fixed.)*

These are listed for your triage, not fixed here — you asked for the
harness to fail loudly on them rather than have me patch around them first.
