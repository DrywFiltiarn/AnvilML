# Tasks: Phase 9 — Real Worker Startup

**Phase:** 9
**Name:** Real Worker Startup
**Project(s):** anvilml
**Status:** Draft
**Depends on phases:** 1, 2, 3, 7, 8

---

## Overview

This phase creates the Python worker process for the first time and gives it a real
startup sequence — connecting over IPC, importing torch, selecting a device,
running a genuine torch-level capability probe, and sending a `Ready` event back to
the Rust supervisor — with **no mock-only gate of any kind, anywhere, at any point**.
A parallel, equally-maintained mock-mode startup sequence exists from this same
phase, not as a placeholder for a real path written later, but as a permanent second
branch that differs from the real path in exactly one step (capability probing).

This phase exists as its own named phase — something v3 never did — because
`ANVILML_DESIGN.md §14.1` identifies v3's single largest defect as a literal
`if os.environ.get("ANVILML_WORKER_MOCK") != "1"): exit(1)` gate at the top of
`worker_main.py`, which meant the real startup code path **never ran, even once**,
for the entire v3 effort. This phase's tasks are written with that history in mind:
P9-D1/P9-D2 implement the real path first and explicitly call out that no such gate
exists anywhere in the file, and P9-E1 closes the phase with an integration test that
spawns a genuine subprocess — not a mock IPC backend — specifically to prove the real
path actually executes end to end, not merely that the code compiles.

At the start of this phase, `worker/` does not exist at all. At the end: a real
Python subprocess, spawned by the Rust `WorkerPool` built in Phase 8, connects over
ZeroMQ, runs a real (CPU-targeted) capability probe, and sends a `Ready` event with
`capabilities_source: "pytorch"` — with zero nodes registered, since the node system
itself doesn't exist until Phase 10. This is exactly the scope
`ANVILML_DESIGN.md §20`'s roadmap names for this phase: real-mode startup "against a
CPU device with no nodes registered yet."

---

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | Worker scaffolding | P9-A1 … P9-A3 | `requirements/base.txt` (no torch), the real torch CPU wheel pin, the `real_mode` pytest marker registration |
| B | IPC | P9-B1 | `worker/ipc.py`'s DEALER transport, mirroring Phase 7's Rust-side `RouterTransport` |
| C | Capability probing | P9-C1 … P9-C2 | The real torch probe, then the mock equivalent |
| D | Startup sequences | P9-D1 … P9-D3 | Real-mode sequence split across two tasks, then the mock-mode sequence |
| E | Integration proof | P9-E1 | The real-subprocess integration test — this phase's actual proof |
| F | CI | P9-F1 | Wires the Phase 1 placeholder `worker-test` job to real install/test steps |

---

## Prerequisites

`anvilml-worker`'s `spawn_worker()` (Phase 8's P8-B2) and `RouterTransport` (Phase
7's P7-B2) must exist and pass their own tests. `anvilml-core` must export
`InferenceCaps` exactly as defined in Phase 3 (P3-A5), since `probe_capabilities()`'s
return dict's keys mirror that struct's field names exactly.

---

## Interfaces and Contracts

| Contract document | Relevant to tasks | What must match |
|--------------------|--------------------|------------------|
| `ANVILML_DESIGN.md §14.1` | P9-D1, P9-D2, P9-D3 | **No mock-only startup gate, anywhere, ever** — read this section before writing any line of `worker_main.py` |
| `ANVILML_DESIGN.md §14.2`–§14.3 | P9-D1, P9-D2, P9-D3 | The exact real-mode and mock-mode startup sequences, step by step |
| `ANVILML_DESIGN.md §14.4` | P9-B1 | `worker/ipc.py`'s module docstring and function signatures — normative, copy verbatim |
| `ANVILML_DESIGN.md §6.6` | P9-C1 | `probe_capabilities()`'s exact mechanical probing contract per capability |
| `ANVILML_DESIGN.md §18.3` | P9-A3, P9-F1 | The `real_mode` pytest marker convention and the worker CI job's install order |
| `ANVILML_DESIGN.md §17.3` | P9-D1 … P9-D3 | Mock AND real are both mandatory in the same phase — never separate phases |

---

## Task Descriptions

### Group A — Worker scaffolding

#### P9-A1: worker/: requirements/base.txt (no torch, core deps only)

**Goal:** Create the worker's core dependency manifest with the one absolute
constraint that makes the mock-mode CI jobs possible: no `torch` anywhere in this
file.

**Files to create or modify:**
- `worker/requirements/base.txt` — core deps: `diffusers`, `transformers`,
  `safetensors`, `pillow`, `msgpack`, `pyzmq`, `pytest`.
- `worker/requirements/cpu-linux-agent.txt`, `cpu-runner-reqs.txt` — empty
  placeholders for now.

**Key implementation notes:**
- `torch` must never appear in `base.txt` — restated here because it's CI-breaking,
  not a style preference; this property is what lets the mock CI jobs install
  cleanly with no GPU driver and no torch wheel index configured at all.
- Resolve every package's current version live via the PyPI registry MCP tool — do
  not pin a version recalled from training data.
- The two CPU-specific requirement files stay empty in this task; torch CPU wheel
  pins are added once real-mode tests exist to justify them, later in this phase.

**Acceptance criterion:**
```bash
pip install --dry-run -r worker/requirements/base.txt
# -> exit 0, no torch index touched
```

#### P9-A2: worker/requirements/: real torch CPU wheel pin in cpu-* files

**Goal:** Populate the two CPU-specific requirement files P9-A1 left empty,
closing a real gap: every real-mode test from this point in the project onward
needs `torch` actually installable from these exact files, not assumed present
from some other source.

**Files to create or modify:**
- `worker/requirements/cpu-linux-agent.txt`, `cpu-runner-reqs.txt` — both gain the
  real `torch` CPU wheel pin.

**Key implementation notes:**
- Both files get the **identical** pin — `cpu-linux-agent.txt` is consumed by this
  project's own CI `worker-test` job (P9-F1); `cpu-runner-reqs.txt` is the same
  content under the name later real-mode tasks expect.
- The CPU-only build comes from the official PyTorch CPU wheel index
  (`https://download.pytorch.org/whl/cpu` or equivalent) — never the default
  index, which would pull in a CUDA-bundled wheel unnecessarily.
- Without this task, every real-mode test in every later phase (capability
  probing, real startup, every architecture module's `-m real_mode` suite) has no
  `torch` to import wherever an environment installs strictly from these files
  rather than relying on a pre-existing system `torch`.

**Acceptance criterion:**
```bash
pip install --dry-run -r worker/requirements/cpu-linux-agent.txt
# -> exit 0, resolves to a CPU-only torch wheel (no CUDA/ROCm variant)
```

#### P9-A3: worker/: pyproject.toml or pytest.ini with real_mode marker registered

**Goal:** Register the `real_mode` pytest marker convention before any test file in
this phase uses it, establishing the single source of truth for what that marker
means.

**Files to create or modify:**
- `worker/pyproject.toml` — `[tool.pytest.ini_options]` with `real_mode` registered.

**Key implementation notes:**
- A test with no marker is assumed mock-compatible and must not import torch
  unconditionally — only `real_mode`-marked tests may do so, per
  `ANVILML_DESIGN.md §18.3`.

**Acceptance criterion:**
```bash
cd worker && python -m pytest --markers | grep real_mode
# -> exit 0, shows the registered marker
```

---

### Group B — IPC

#### P9-B1: worker/ipc.py: ZeroMQ DEALER transport + msgpack framing

**Goal:** Implement the Python-side counterpart to Phase 7's Rust `RouterTransport`
— the DEALER socket wrapper every worker uses to talk to the supervisor.

**Files to create or modify:**
- `worker/ipc.py` — `connect()`, `send_event()`, `recv_message()`.

**Key implementation notes:**
- `ANVILML_DESIGN.md §14.4`'s module docstring and function signatures are
  **normative** — copy them verbatim rather than paraphrasing, since this is exactly
  the kind of cross-language contract where a subtly different signature would
  silently break interoperability with the Rust side.
- The socket's `IDENTITY` is set to `worker_id.encode()` **before** `connect()` — the
  same ordering requirement the Rust-side topology depends on.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_ipc.py -v
# -> >=5 tests, exits 0
```

---

### Group C — Capability probing

#### P9-C1: worker/capability.py: probe_capabilities() real torch probe

**Goal:** Implement the real, mechanical torch-level capability probe — the
function that makes this phase's `Ready` event trustworthy rather than a hint.

**Files to create or modify:**
- `worker/capability.py` — `probe_capabilities()`.

**Key implementation notes:**
- Every capability is probed by actually attempting the operation, never read from
  a hint table: `fp16`/`bf16` via a tiny `torch.nn.Linear` forward pass at that
  dtype; `fp8` the same pattern at `torch.float8_e4m3fn`; `flash_attention` via the
  lightest available call path.
- **`fp8` on CPU correctly returns `False`** — `torch.float8_e4m3fn` raises
  `NotImplementedError` on CPU today, and that's the expected, correct result, not a
  bug to engineer around.
- A hardcoded `True` for any field without running the actual probe is non-compliant
  — this is exactly the failure mode (a device-table hint claiming a capability the
  installed torch build can't actually use) that produced a real, recorded v3
  defect.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_capability.py -v -m real_mode
# -> >=6 tests, exits 0
```

#### P9-C2: worker_main.py: _mock_probe_capabilities() synthetic values

**Goal:** Implement the mock-mode equivalent of the capability probe — fixed
synthetic values that never import torch — as the first piece of `worker_main.py`
to exist.

**Files to create or modify:**
- `worker/worker_main.py` — new file; only `_mock_probe_capabilities()` in this
  task.

**Key implementation notes:**
- This function lives **inline in `worker_main.py`**, not in `capability.py` — the
  mock equivalent and the real probe are deliberately in different files, per
  `ARCHITECTURE.md`'s explicit module-placement note.
- Must never import torch, not even transitively — the test suite confirms this via
  a subprocess-isolated check (running `worker_main` in a fresh subprocess and
  asserting `"torch" not in sys.modules`), not the forbidden
  `sys.modules.pop("torch")` + `importlib.reload()` pattern that crashed the agent
  VM twice in a prior project history.

**Acceptance criterion:**
```bash
ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_worker_main.py -v
# -> >=3 tests, exits 0
```

---

### Group D — Startup sequences

#### P9-D1: worker_main.py: real-mode connect+device-select+probe (no mock gate)

**Goal:** Implement the first half of the real-mode startup sequence — connecting
over IPC, importing torch, selecting the device, and running the real capability
probe — establishing that no mock-only gate exists before any later step is added.

**Files to create or modify:**
- `worker/worker_main.py` — adds the real-mode sequence's connect/device-select/
  probe steps.

**Key implementation notes:**
- **There is no `if ANVILML_WORKER_MOCK != "1": exit(1)` gate anywhere in this file**
  — not in this task, not added "temporarily," not under any framing. This exact
  pattern is the single largest defect in this project's history, and this task's
  test suite includes an explicit check that no such gate exists.
- Node import and the `Ready` event send are explicitly deferred to the next task —
  this task stops right after the capability probe.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_worker_main.py -v -m real_mode
# -> >=3 tests, exits 0
```

#### P9-D2: worker_main.py: real-mode node-import stub + Ready event + loop

**Goal:** Complete the real-mode startup sequence with the node-import stub (zero
nodes, correctly, since the node system doesn't exist until Phase 10), the `Ready`
event send, and a dispatch-loop placeholder.

**Files to create or modify:**
- `worker/worker_main.py` — completes the real-mode sequence.

**Key implementation notes:**
- `_import_nodes()` returns an empty list at this point in the project — this is
  correct, not a stub awaiting completion within this phase; real node import is
  Phase 10's scope entirely.
- `Ready`'s `capabilities_source` field is `"pytorch"` in this branch — the literal
  string the scheduler and operator diagnostics will key off of later.
- This receives exactly the scope P9-D1 deferred.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_worker_main.py -v -m real_mode
# -> >=7 real_mode tests total in the file, exits 0
```

#### P9-D3: worker_main.py: mock-mode startup sequence

**Goal:** Implement the mock-mode startup sequence as a permanent, equally
maintained second branch — not a stand-in for the real path this phase already
built.

**Files to create or modify:**
- `worker/worker_main.py` — adds the mock-mode branch, selected by
  `ANVILML_WORKER_MOCK=1`.

**Key implementation notes:**
- The mock/real branch check happens **once**, at the top-level entry point — never
  re-checked deep inside helper functions.
- IPC connection, node import, and the dispatch loop are **identical code** between
  the two branches — only the capability-probing step differs. This is what makes
  the eventual `REAL_PATH_VERIFIED`/`MOCK_PATH_VERIFIED` marker convention (Phase
  10+) meaningful: there is genuinely one real branch and one mock branch, not two
  separately maintained worker implementations.

**Acceptance criterion:**
```bash
ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_worker_main.py -v
# -> >=11 tests total in the file across both modes, exits 0
```

---

### Group E — Integration proof

#### P9-E1: anvilml-worker: integration test, real subprocess sends Ready

**Goal:** Prove, with a genuine spawned subprocess rather than a mock IPC backend,
that real worker startup actually works end to end — the explicit gap this phase
exists to close.

**Files to create or modify:**
- `crates/anvilml-worker/tests/real_startup_tests.rs` — new file.

**Key implementation notes:**
- This is the **only** test in this phase that spawns a real `worker_main.py`
  subprocess via `spawn_worker()` (Phase 8's P8-B2) against a real bound
  `RouterTransport` (Phase 7's P7-B2) — every other test in this phase exercises
  Python and Rust code in isolation.
- Uses an explicit timeout on the `recv()` await, per the mandatory subprocess-IPC
  timeout pattern — never an unguarded blocking call.
- Asserts `capabilities_source == "pytorch"` and `node_types` is empty — the exact
  externally observable proof that real (not mock) startup executed.

**Acceptance criterion:**
```bash
cargo test -p anvilml-worker --test real_startup_tests -- --test-threads=1
# -> exits 0
```

---

### Group F — CI

#### P9-F1: CI: wire worker-test job to real base.txt install + both test suites

**Goal:** Replace Phase 1's placeholder echo in the `worker-test` CI job with real
installation and test-execution steps, now that there's a real worker to test.

**Files to create or modify:**
- `.github/workflows/ci.yml` — `worker-test` job's steps.

**Key implementation notes:**
- Install order, identical on both OS matrix legs per `ANVILML_DESIGN.md §18.3`:
  `base.txt` → (mock leg) run the mock suite directly; (real leg) a mock-suite
  collection check (`pytest --collect-only -m "not real_mode"`, confirming nothing
  in the mock suite accidentally imports torch) → `cpu-runner-reqs.txt` → the
  real-mode suite.
- No platform-specific branch in this ordering without an explicitly stated reason.

**Acceptance criterion:**
```bash
grep -c 'worker-test' .github/workflows/ci.yml
# -> job exists with real (non-echo) steps
# A pushed commit shows all 4 worker-test matrix entries green.
```

---

## Phase Acceptance Criteria

```bash
cargo fmt --all -- --check
cargo clippy --workspace --features mock-hardware -- -D warnings
cargo test --workspace --features mock-hardware
python -m py_compile $(git ls-files 'worker/*.py')
ANVILML_WORKER_MOCK=1 python -m pytest worker/tests -v
python -m pytest worker/tests -v -m real_mode

# Runnable Proof (manual):
cargo test -p anvilml-worker --test real_startup_tests -- --test-threads=1
# -> exits 0: a real worker_main.py subprocess connects over IPC, runs the real
#    capability probe, and sends a Ready event with capabilities_source="pytorch"
#    and an empty node_types list.
```

---

## Known Constraints and Gotchas

- **No mock-only startup gate may exist anywhere in `worker_main.py`, at any point,
  for any reason** — this is the single most important constraint in this phase,
  restated from `ANVILML_DESIGN.md §14.1`, and is the literal defect that consumed
  the entire v3 effort's worker-startup work without anyone noticing for four
  phases.
- Real-mode and mock-mode startup sequences must both exist and both be tested
  within this same phase — they may be separate tasks (P9-D1/P9-D2 vs. P9-D3) but
  never separate phases.
- `_import_nodes()` correctly returns an empty list throughout this phase — this is
  not an oversight; the node system itself is Phase 10's scope.
- The real-mode test suite in this phase runs on torch CPU only, using no GPU
  hardware and no production-size checkpoint — per the project-wide rule that no
  automated task may assume real GPU hardware is available.
- `worker/ipc.py`'s function signatures are copied verbatim from
  `ANVILML_DESIGN.md §14.4` — this is one of the few places in the spec where the
  code shown is meant to be transcribed exactly, not adapted.
- `cpu-linux-agent.txt`/`cpu-runner-reqs.txt` must actually contain the real torch
  CPU wheel pin (P9-A2) before any real-mode test in this phase or any later one
  can be expected to pass in an environment that installs strictly from these
  files — leaving them empty (which an earlier pass through this delivery did,
  before being caught on review) would make every subsequent `-m real_mode`
  pytest invocation in this project fail with an import error, despite every
  later task's acceptance criterion assuming `torch` is simply available.

---

## docs/RUNNABLE_PROOF.md entry

```markdown
## Phase 9 — Real Worker Startup

**Capability proved:** A real Python `worker_main.py` subprocess, spawned by the
Rust worker pool, connects over ZeroMQ, runs a real torch-level capability probe on
a CPU device, and sends a `Ready` event with `capabilities_source: "pytorch"` — the
first genuine end-to-end real-mode execution of the worker startup path.

\`\`\`bash
# Runnable Proof (manual):
cargo test -p anvilml-worker --test real_startup_tests -- --test-threads=1
# -> exits 0
\`\`\`
```
