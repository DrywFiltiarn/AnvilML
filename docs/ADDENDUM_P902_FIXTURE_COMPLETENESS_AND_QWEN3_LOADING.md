# Addendum: P902 — Fixture Completeness, `to_empty()` Zero-Init, and the
# Qwen3 Loading Pipeline Was Never Actually Loading Anything

**Status:** Resolved this session, as a manual patch. Follows on from P901;
recorded here per the same convention (`docs/ADDENDUM_P901_MANUAL_RETROFIT.md`).

**Origin:** investigating a `RuntimeWarning: invalid value encountered in
cast` reported from `test_e2e_zit_pipeline.py`'s VAE-decode step.

---

## 1. Root cause: `to_empty()` doesn't zero anything, and no fixture actually populated its model

Tracing the warning backward: the sampled latent feeding `VaeDecode` was
**100% NaN**, before ever reaching the VAE. Instrumenting `zit.py`'s forward
pass found the first NaN inside `double_blocks.0.norm2`, whose own
parameters (`weight`/`bias`) were literally `nan`/`~1.7e38` on the loaded
model.

`load()`'s materialization step is:
```python
model = model.to_empty(device=device)
```
`to_empty()` allocates **uninitialized** memory — it does not zero
anything — despite a comment claiming otherwise ("...doesn't fully
populate the MultiheadAttention parameters (which are zero-initialized by
design)"). Cross-checking every arch family's fixture against its model's
*real* `state_dict()` (via each module's own `_build_key_remapping()`, not
naive string diffs) found this was **not confined to zit**:

| Family | Real parameters | Matched by fixture (before) | Matched (after) |
|---|---|---|---|
| `zit.py` | 28 | 6 (21%) | 28 (100%) |
| `zit_vae.py` | 20 | 10 (50%) | 20 (100%) |
| `qwen3.py` | 31 | **0 (0%)** | 31 (100%) |

VAE's 50%-matched gap didn't visibly manifest (its parameters looked
"sane" in ad-hoc checks) because `load()` already had a defensive
zero-init step for it — narrowly scoped to `.bias`-suffixed parameters,
which happened to be sufficient for VAE's specific gap, but wasn't a real
guarantee (a future gap in a *weight* tensor would not have been caught).
Qwen3's 0%-matched gap didn't surface because no existing real-mode test
ever ran an actual forward pass through it — every Qwen3 test checked only
`.arch`, device, dtype, and tokenizer presence.

### Fix: defensive zero-init in all three `load()` functions

`zit.py` and `qwen3.py` (which had no zero-init step at all) now zero
every parameter and buffer immediately after `to_empty()`:
```python
for param in model.parameters():
    param.data.zero_()
for buf in model.buffers():
    buf.data.zero_()
```
`zit_vae.py`'s existing bias-only step was widened to match (zero
everything, not just `.bias`-suffixed parameters) for the same
defense-in-depth reasoning. Loaded values are unaffected — every matched
key is overwritten by `load_state_dict()` regardless of what it was zeroed
to first.

---

## 2. Fixtures rebuilt to be genuinely complete, not just defended against

Zero-init makes an incomplete fixture *safe* (zero instead of NaN/garbage),
but a fixture that doesn't cover the real model isn't actually testing
real-weight loading. All five affected fixture-generation scripts —
`build_zit_fixture.py`, `build_zit_vae_fixture.py`,
`build_zit_fp8_fixture.py`, `build_zit_vae_fp8_fixture.py`, and
`build_qwen3_fixture.py` — were rewritten to construct the *real* model
class directly (`ZiTModel(hyperparams)`, `ZiTVaeModel(hyperparams)`,
`Qwen3TextEncoder(hyperparams)` — real device, not meta, letting PyTorch's
normal default init populate every parameter) and dump its full
`state_dict()`, rather than hand-picking a handful of representative keys.
This guarantees the fixture always exactly matches the real architecture
going forward — no hand-maintained key list to fall out of sync as a model
evolves (see finding 4 below for a case where exactly that already
happened).

`_HYPERPARAMS` in every rewritten script reproduces exactly what
`_infer_hyperparams()` derived from the *pre*-P902 fixture, so no existing
test's shape/dtype/count assertions needed to change — only the fixtures'
*completeness*, not their *shape*, changed. `torch.manual_seed(42)` was
added to each for reproducibility (previously unseeded — re-running a
build script produced different random weights every time, which is how
an earlier session accidentally clobbered `qwen3_tiny.safetensors` with
fresh random data, caught and reverted during the P901 session).

`*_no_metadata_tensors()` in `build_zit_fixture.py` and
`build_zit_vae_fixture.py` were deliberately **not** rebuilt from the full
state_dict — they test a narrow, different thing (the metadata-fallback
header-parsing path), never run a real forward pass, and (for VAE)
reusing the full state_dict would have introduced bias keys the
no-metadata rename logic doesn't handle. Each now has its own small,
independent minimal tensor builder instead, preserving pre-P902 behavior
exactly. `zit_tiny_fp8.safetensors` is currently unreferenced by any test
(confirmed via exhaustive grep) — rebuilt anyway for consistency.
`zit_vae_tiny_fp8.safetensors`'s pre-P902 channel sizes (8→16→32→16→8)
didn't correspond to any hyperparameters `ZiTVaeModel`'s real
channel-interpolation formula would produce; rather than debug and
preserve that inconsistency, it now reuses the regular VAE fixture's
verified-consistent hyperparams (no test asserts a specific channel size
for the fp8 fixture, only dtype, so this substitution is safe).

---

## 3. A second, deeper bug: Qwen3's loading pipeline never worked at all

Rebuilding the Qwen3 fixture in the *real* Qwen3/HF checkpoint convention
(`model.`-prefixed, separate `self_attn.{q,k,v,o}_proj.weight`) — which is
what `_infer_hyperparams()` has always required, and what any actual Qwen3
checkpoint looks like — revealed `_build_key_remapping()` couldn't load
*anything* from it. Three separate, independent bugs, each sufficient on
its own to break the entire pipeline:

1. **No "model." prefix stripping.** Real checkpoints prefix every key
   with `model.` (`model.embed_tokens.weight`, `model.norm.weight`,
   `model.layers.N.mlp.*`, etc.); this module's own `state_dict()` keys
   are bare (`embed_tokens.weight`, `layers.N.mlp.*`, ...) — there's no
   `.model` submodule wrapping this architecture's construction. Nothing
   stripped the prefix before the direct-match comparison, so **every**
   non-attention key — embeddings, MLP, layer norms, the final norm —
   silently failed to match, not just attention.
2. **A typo in the in_proj target key.** The code built
   `f"{prefix}in_proj.weight"` (with a dot) as the target for Q/K/V
   remapping, but `nn.MultiheadAttention`'s real parameter is
   `in_proj_weight` (no dot — it's a single flat attribute, not a
   submodule with its own `.weight`). This made the check
   `in_proj_key in module_key_set` always `False`, regardless of any other
   fix — dead code.
3. **No actual concatenation.** Even with (1) and (2) fixed, the prior
   code mapped three *different* checkpoint keys (`q_proj.weight`,
   `k_proj.weight`, `v_proj.weight`) to the *same* module-key string in a
   plain `dict` — incoherent as a mapping (only the last-processed of the
   three could ever survive the assignment), and even the survivor's shape
   `(hidden_dim, hidden_dim)` doesn't match `in_proj_weight`'s real shape
   `(3*hidden_dim, hidden_dim)`, so `load()`'s shape check would have
   skipped it regardless of (1) and (2).

Net effect, empirically confirmed before the fix: **0 of Qwen3TextEncoder's
31 real parameters could ever be loaded from any realistically-shaped
checkpoint**, independent of anything in this fixture. The existing unit
tests for `_build_key_remapping()` didn't catch any of this because both
of them tested against *invented* key conventions rather than the real
model's actual `state_dict()` keys: one used `"model."`-prefixed strings
on *both* sides (checkpoint and "module"), so the real-world prefix
mismatch was never exercised; the other asserted against a target key
literally spelled `"in_proj.weight"` (with the same dot-typo the
implementation had), so the test and the bug agreed with each other
rather than with reality.

### Fix: `_normalize_attention_keys()`

The Q/K/V→`in_proj_weight` relationship is a genuine structural difference
(concatenation), not just a naming difference (unlike the `model.`
prefix), so it can't be expressed as a 1:1 string mapping —
`_build_key_remapping()`'s contract. A new function,
`_normalize_attention_keys()`, runs as a preprocessing step in `load()`
before `_build_key_remapping()`:

- Strips a leading `model.` from every key.
- For each layer, concatenates a complete `q_proj`/`k_proj`/`v_proj`
  weight triple (all three present, identically shaped) into a single
  `in_proj_weight` via `torch.cat([q, k, v], dim=0)` — matching
  `nn.MultiheadAttention`'s real internal layout exactly. The bias triple
  is concatenated the same way, independently. An incomplete or
  shape-inconsistent triple is dropped entirely, not passed through under
  any key (a lone `q_proj.weight` isn't a valid `in_proj_weight` and
  loading it as one would silently corrupt the model rather than leaving
  it at its safe zero-initialized default).
- Renames `o_proj.{weight,bias}` to `out_proj.{weight,bias}` — a straight
  rename, shapes already match.

`_build_key_remapping()` itself is now much simpler: by the time it's
called, every key already uses this module's own bare convention, so it's
pure direct-match (`{key: key for key in checkpoint_keys if key in
module_key_set}`).

**Verified:** round-tripping a freshly-constructed `Qwen3TextEncoder`
through `_to_checkpoint_convention()` (the fixture builder's exact inverse
of `_normalize_attention_keys()`) and back through `load()` reproduces the
original model's weights **bit-for-bit** (`torch.equal()` on every
parameter). 31/31 parameters now match via `_build_key_remapping()`
against the rebuilt fixture, versus 0/31 before.

### Test changes

The two pre-existing `_build_key_remapping()` unit tests were rewritten:
`test_build_key_remapping_direct_match` now uses a real
`Qwen3TextEncoder`'s actual keys instead of an invented "model."-prefixed
list on both sides; `test_build_key_remapping_attention_remap` (which
asserted the incorrect `"in_proj.weight"` spelling) was removed and
replaced with five new tests directly against `_normalize_attention_keys()`
— concatenation correctness (weight and bias), incomplete-triple dropping,
the o_proj rename, and prefix stripping.

---

## 4. A third, unrelated bug the fixture rebuild exposed: a stale committed binary

Regenerating `zit_tiny_no_metadata.safetensors` from its (unchanged)
generator function broke `test_infer_hyperparams_no_metadata_fixture`
(`latent_height` came out `8`, not the asserted `4`). This is **not**
related to the P902 completeness work — `_no_metadata_tensors()`'s source
code was never touched by this retrofit. Comparing the checked-in binary
against a fresh run of its own unmodified generator found the *committed
file* had `xyz_latents` shaped `(1, 4, 4, 4)`, while the *current source*
has always said `torch.randn(1, 4, 8, 8)` (apparently copy-pasted from
`_zit_tensors()`'s "latents" marker, which — unlike this one — is
harmless there, since the regular fixture's `input_proj.weight` key makes
`_infer_hyperparams()` take a different code path that never reads
"latents"'s own spatial dimensions). The committed binary predates
whatever edit introduced that `8` and was never regenerated to match —
invisible because CI only ever loads the committed binary, never re-runs
the generator script. This retrofit's rebuild was, apparently, the first
time anyone re-ran it since that edit landed.

**Fix:** `xyz_latents` corrected back to `torch.randn(1, 4, 4, 4)` — the
test's stated intent ("Return the same shape-based hyperparameters as the
regular fixture") was always correct; the source had silently drifted from
it.

---

## 5. New regression tests

One test per family (`test_load_real_{zit,zit_vae,qwen3}_fixture_no_unmatched_parameters`)
independently re-derives the checkpoint-to-module key remapping (via each
module's own `_build_key_remapping()`, and for qwen3,
`_normalize_attention_keys()` first) and asserts every real parameter has
a match — i.e. that the fixture is *genuinely* complete, not merely that
`load()` doesn't crash — plus confirms no parameter is NaN, infinite, or
suspiciously large (`>1e6`) after `load()`, independent of the remapping
check. This is the pair of properties (fixture completeness +
loaded-value sanity) that, had they existed from the start, would have
caught every defect in this addendum immediately.

---

## Process note: a self-inflicted marker bug, caught before delivery

Inserting `test_load_real_qwen3_fixture_no_unmatched_parameters` directly
above the pre-existing `test_load_real_qwen3_fixture_with_weights` (which
was `@pytest.mark.real_mode`-decorated) via a text edit landed the new
test's content *between* that decorator and the original function
definition — silently re-targeting the decorator onto the new test and
leaving the original one unmarked, which would have crashed it under
`ANVILML_WORKER_MOCK=1`. Caught by re-running the full mock-mode suite
before delivery (the same verification step that should always precede
any patch touching test files) and by a systematic before/after marker
diff across every touched test file, confirming no other pre-existing
test's marker status changed unintentionally. Fixed by restoring the
displaced decorator and removing the resulting duplicate.

---

## Where this is reflected in this delivery

- `worker/nodes/arch/diffusion/zit.py`, `worker/nodes/arch/vae/zit_vae.py`,
  `worker/nodes/arch/clip/qwen3.py` — zero-init (finding 1);
  `_normalize_attention_keys()`/`_build_key_remapping()` rewrite in
  `qwen3.py` (finding 3).
- `worker/tests/fixtures/build_{zit,zit_vae,zit_fp8,zit_vae_fp8,qwen3}_fixture.py`
  — full state_dict rebuilds (finding 2); `xyz_latents` shape fix
  (finding 4).
- `worker/tests/test_arch_{zit,vae_zit,clip_qwen3}.py` — new completeness
  regression tests (finding 5); rewritten `_build_key_remapping()` unit
  tests and new `_normalize_attention_keys()` tests (finding 3).
- `docs/TESTS.md` — catalogue entries for every new/changed test.
