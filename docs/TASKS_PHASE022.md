# Tasks: Phase 22 — Qwen3 CLIP Arch Module

**Phase:** 22
**Name:** Qwen3 CLIP Arch Module
**Project(s):** anvilml
**Status:** Draft
**Depends on phases:** 1, 2, 3, 9, 10, 19, 20

---

## Overview

This phase implements the project's first text-encoder arch module —
`worker/nodes/arch/clip/qwen3.py` — following the exact same four-step loading
contract `zit.py` followed in Phase 20, with one addition specific to this family:
vendoring and loading the Qwen3 tokenizer from a local, committed asset directory,
never from a model hub. `LoadClip`'s real branch, deliberately raising since Phase
19, finally calls something real.

This phase exists independently of, and in parallel conceptually with, Phase 21's
sampling work — `qwen3.py` only needs to implement `load()`, since CLIP modules have
no `sample()` or `decode()` per `ANVILML_DESIGN.md §10.4`'s table. This phase follows
the identical task breakdown Phase 20 established (fixture → shape inference →
dispatch registration → meta construction → dtype selection → key remap/load)
specifically so the pattern is recognizable and the same defects (`P904`'s partial
key scan, the dtype-cast-ordering mistake) are guarded against identically, rather
than each arch module inventing its own task structure.

At the start of this phase, `worker/assets/qwen3_tokenizer/` doesn't exist and
`LoadClip`'s real branch unconditionally raises `NotImplementedError`. At the end: a
tiny synthetic Qwen3-shaped fixture checkpoint loads successfully end-to-end through
the real path — shape inference, meta-device construction, vendored tokenizer
loading with zero network calls, dtype selection, key remapping, and weight loading
all genuinely exercised — with `LoadClip`'s stale placeholder test removed and
replaced by one that actually passes.

---

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | Tokenizer vendoring | P22-A1 | The vendored asset directory and its re-seeding scripts |
| B | Fixture & shape inference | P22-B1 … P22-B3 | The Qwen3 fixture, shape inference, then dispatch registration |
| C | Construction & loading | P22-C1 … P22-C2 | Meta construction + dtype + tokenizer load, then key remap + load |
| D | Loader integration | P22-D1 | `LoadClip`'s real branch finally does something real |
| E | Proof | P22-E1 | The phase's Runnable Proof |

---

## Prerequisites

`worker/tests/fixtures/README.md`'s convention must exist per Phase 19 (P19-D1).
`LoadClip`'s mock/real-placeholder structure must exist per Phase 19 (P19-C3). The
`arch/clip/__init__.py` dispatcher must exist (with zero registered modules) per
Phase 10 (P10-B2).

---

## Interfaces and Contracts

| Contract document | Relevant to tasks | What must match |
|--------------------|--------------------|------------------|
| `ANVILML_DESIGN.md §10.5` | P22-A1 | Tokenizer vendoring — committed to git, never downloaded at runtime |
| `ANVILML_DESIGN.md §10.3` LoadClip row | P22-B3 | `clip_type` is a string dispatch key ("qwen3"), distinct from diffusion/vae's metadata-or-path pattern |
| `ANVILML_DESIGN.md §10.4` | P22-B3, P22-C1, P22-C2 | CLIP modules implement `load()` only — no `sample()`/`decode()` |
| `ANVILML_DESIGN.md §11.2` | P22-C1 | Library boundary — `transformers`' tokenizer class from the vendored local path, never a hub lookup |
| `ANVILML_DESIGN.md §11.3` | P22-B2, P22-C1, P22-C2 | The same four-step contract `zit.py` followed in Phase 20 |
| `ANVILML_DESIGN.md §10.6` | P22-D1 | Marker hygiene — a stale marker pointing at a removed test is a real defect |

---

## Task Descriptions

### Group A — Tokenizer vendoring

#### P22-A1: worker/assets/qwen3_tokenizer/: vendored tokenizer + seeding script

**Goal:** Vendor the Qwen3 tokenizer's assets locally, the prerequisite for
keeping the worker fully offline-capable for text encoding.

**Files to create or modify:**
- `worker/assets/qwen3_tokenizer/` — vocab/merges/config files, committed.
- `worker/tools/seed_tokenizers.sh`, `seed_tokenizers.ps1` — new; re-seeding
  scripts.

**Key implementation notes:**
- The re-seeding scripts' provenance reasoning (which upstream repo/release, why
  it's the correct canonical source) is recorded as a comment **in the script
  itself** — not just a bare URL with no justification.
- The vendored directory is committed to git and never downloaded from a model hub
  at worker runtime — this is what keeps the worker fully offline-capable.

**Acceptance criterion:**
```bash
test -d worker/assets/qwen3_tokenizer && ls worker/assets/qwen3_tokenizer | wc -l
# -> >=1 file
```

---

### Group B — Fixture & shape inference

#### P22-B1: worker/tests/fixtures/: Qwen3 CLIP fixture safetensors builder

**Goal:** Create the tiny synthetic checkpoint every subsequent task in this
phase tests against, following Phase 19's documented convention.

**Files to create or modify:**
- `worker/tests/fixtures/build_qwen3_fixture.py` — new; the builder script.
- `worker/tests/fixtures/qwen3_tiny.safetensors` — the generated fixture,
  committed.

**Key implementation notes:**
- Shapes are structurally valid for the shape-inference formula `qwen3.py` will
  implement — not a miniaturized copy of the real model's actual shapes.

**Acceptance criterion:**
```bash
python worker/tests/fixtures/build_qwen3_fixture.py
# -> exits 0, file under 10MB, loads via safetensors.safe_open
```

#### P22-B2: worker/nodes/arch/clip/qwen3.py: shape inference from safetensors header

**Goal:** Implement the contract's first step for the CLIP family — inferring
every architecture hyperparameter from the checkpoint's tensor shapes alone.

**Files to create or modify:**
- `worker/nodes/arch/clip/qwen3.py` — new; `_infer_hyperparams()`.

**Key implementation notes:**
- Same discipline as Phase 20's `zit.py` — reads every key, never a truncated
  sample.
- `can_handle()` and dispatch registration are explicitly deferred to the next
  task.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_arch_clip_qwen3.py -v
# -> >=3 tests, exits 0
```

#### P22-B3: worker/nodes/arch/clip/qwen3.py: can_handle() + dispatch registration

**Goal:** Connect `qwen3.py` to the CLIP dispatch mechanism Phase 10 built.

**Files to create or modify:**
- `worker/nodes/arch/clip/qwen3.py` — adds `can_handle()`.
- `worker/nodes/arch/clip/__init__.py` — registers `qwen3.py`.

**Key implementation notes:**
- The CLIP family's dispatch key is the `clip_type` **string** (`"qwen3"`) — a
  fundamentally different dispatch mechanism from diffusion/VAE's
  metadata-or-path-derived arch string, per `ANVILML_DESIGN.md §10.3`'s `LoadClip`
  row.
- No `sample()`/`decode()` for this family — `load()` is the entire contract for
  CLIP modules.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_arch_clip_qwen3.py -v
# -> >=6 tests total in the file, exits 0
```

---

### Group C — Construction & loading

#### P22-C1: worker/nodes/arch/clip/qwen3.py: meta construction + dtype selection

**Goal:** Implement meta-device construction, dtype selection, and — unique to
this family — vendored tokenizer loading, all in one task since they're tightly
coupled steps of the same contract phase.

**Files to create or modify:**
- `worker/nodes/arch/clip/qwen3.py` — adds meta construction, dtype selection,
  tokenizer loading.

**Key implementation notes:**
- The tokenizer is loaded via `transformers`' tokenizer class pointed at the
  **local vendored path** (Phase 22's P22-A1) — never a hub lookup, even one gated
  behind a flag.
- A test confirms zero network calls occur during tokenizer loading, via a
  network-blocking fixture or monkeypatch — this is the load-bearing test case for
  this task, proving the offline guarantee actually holds, not just that the code
  looks like it should.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_arch_clip_qwen3.py -v
# -> >=10 tests total in the file, exits 0
```

#### P22-C2: worker/nodes/arch/clip/qwen3.py: key remap, load_state_dict, .arch attribute

**Goal:** Complete `load()` with the final contract steps, mirroring the exact
dtype-cast-before-`assign=True` discipline Phase 20 established.

**Files to create or modify:**
- `worker/nodes/arch/clip/qwen3.py` — completes `load()`.

**Key implementation notes:**
- The same mandatory cast-before-`assign=True` ordering from Phase 20's `zit.py`
  applies here too, restated because every arch module independently must get this
  right — it is not a lesson that, once learned in one file, automatically applies
  to the next.
- The returned object bundles the text encoder and its loaded tokenizer together,
  per `§11.3` step 4's single-object-return contract.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_arch_clip_qwen3.py -v
# -> >=16 tests total in the file, exits 0
```

---

### Group D — Loader integration

#### P22-D1: worker/nodes/loader.py: LoadClip real branch calls qwen3.py via dispatch

**Goal:** Close the gap Phase 19 deliberately left open for `LoadClip` — its real
branch finally does something real, end to end.

**Files to create or modify:**
- `worker/nodes/loader.py` — replaces `LoadClip`'s `NotImplementedError`
  placeholder.

**Key implementation notes:**
- This is the **second** loader (after `LoadModel`, Phase 20) to gain a real
  branch — `LoadVae` remains real-raising until a later phase builds its arch
  module.
- The stale marker hygiene from Phase 20's `P20-D1` applies identically here: the
  old `NotImplementedError`-asserting test is removed, not left alongside the new
  passing one.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_nodes_loader.py -v -m real_mode
# -> exits 0
```

---

### Group E — Proof

#### P22-E1: Runnable Proof: LoadClip node loads the Qwen3 fixture checkpoint for real

**Goal:** Produce this phase's Runnable Proof — a real-mode pytest invocation
exercising the entire chain this phase built.

**Files to create or modify:**
- None. This task runs the existing real-mode test suites; see Acceptance
  Criterion.

**Key implementation notes:**
- Like Phases 20–21, this is a pytest invocation, not a live HTTP request — no
  `ClipTextEncode` exists yet to actually use the loaded encoder for conditioning.
  That's a later phase's scope.
- This proof additionally confirms zero network calls anywhere in the tokenizer
  path — the offline guarantee, exercised end to end, not just unit-tested in
  isolation.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_arch_clip_qwen3.py worker/tests/test_nodes_loader.py -v -m real_mode
# -> exits 0, zero skips, zero xfails in this invocation
```

---

## Phase Acceptance Criteria

```bash
cargo fmt --all -- --check
cargo clippy --workspace --features mock-hardware -- -D warnings
cargo test --workspace --features mock-hardware
ANVILML_WORKER_MOCK=1 python -m pytest worker/tests -v
python -m pytest worker/tests -v -m real_mode

# Runnable Proof (manual): see P22-E1 — the full real-mode chain (shape
# inference, meta construction, vendored tokenizer load, dtype selection, key
# remap, load, LoadClip's real branch) succeeds end to end against the Qwen3
# fixture checkpoint, with zero network calls anywhere in the tokenizer path.
python -m pytest worker/tests/test_arch_clip_qwen3.py worker/tests/test_nodes_loader.py -v -m real_mode
```

---

## Known Constraints and Gotchas

- `qwen3.py` implements `load()` only — no `sample()` or `decode()` exist for the
  CLIP family, by design, per `ANVILML_DESIGN.md §10.4`'s table.
- The vendored tokenizer directory must never be read from, written to, or
  bypassed by any code path that could fall back to a hub lookup — even a flag-gated
  one. The network-blocking test in P22-C1 is what makes this guarantee mechanically
  checked rather than merely asserted in prose.
- The CLIP family's dispatch key (`clip_type` string) is fundamentally different
  from diffusion/VAE's arch-metadata-or-path dispatch — don't conflate the two
  patterns when writing `can_handle()`.
- `LoadVae` remains real-raising after this phase — only `LoadModel` (Phase 20) and
  `LoadClip` (this phase) have real branches so far. `LoadVae`'s gap closes in a
  later phase building `zit_vae.py`.

---

## docs/RUNNABLE_PROOF.md entry

```markdown
## Phase 22 — Qwen3 CLIP Arch Module

**Capability proved:** The full real-mode text-encoder loading chain — shape
inference, meta-device construction, vendored tokenizer loading (zero network
calls), dtype selection, key remapping, and weight loading — succeeds end to end
against a tiny synthetic Qwen3-shaped fixture checkpoint, with `LoadClip`'s real
branch calling genuinely real code for the first time in the project.

\`\`\`bash
# Runnable Proof (manual):
python -m pytest worker/tests/test_arch_clip_qwen3.py worker/tests/test_nodes_loader.py -v -m real_mode
# -> exits 0, zero skips, zero xfails
\`\`\`
```
