# Tasks: Phase 009 — Python Worker (ZiT)

| Field            | Value                                                                       |
|------------------|-----------------------------------------------------------------------------|
| Phase            | 009                                                                         |
| Name             | Python Worker — ZiT                                                         |
| ANVIL Milestone  | M5                                                                          |
| Status           | Draft                                                                       |
| Depends on phases| 1, 2, 3, 4, 5, 6, 7, 8                                                      |
| Task file        | `forge/tasks/tasks_phase009.json`                                           |
| Design reference | `ANVILML_DESIGN.md` §14 (Python Worker), §7 (IPC Protocol)                 |

---

## Overview

Phase 009 implements the Python worker in full: the IPC framing layer (`ipc.py`), the startup/message loop (`worker_main.py`), the node infrastructure (`nodes/base.py`, `executor.py`), the pipeline cache, the ZiT node set, the model defaults, and the KNOWN_NODE_TYPES parity test. This phase completes M5.

The M5 exit criterion is: "`Execute→Progress→ImageReady→Completed` in mock; ZiT end-to-end smoke on real hardware." The mock path is covered by the `ANVILML_WORKER_MOCK=1` pytest suite and by the Rust worker integration test that spawns the real Python process. The real-hardware smoke is a manual verification step (see §20.4) performed after the phase is complete — CI only covers the mock path.

The Python worker deliberately mirrors the Rust framing contract. Every IPC invariant established in phase 002 (`read_exact` loops, named msgpack maps, big-endian length prefix) must be respected on the Python side. Divergence between the two sides will produce silent stream corruption or deserialization failures that are difficult to diagnose. This is why `worker/ipc.py` already has its binary-mode guard from phase 001, and why the IPC round-trip is tested independently of the full execution path.

---

## Group Reference

| Group | Subsystem   | Tasks          | Summary                                                         |
|-------|-------------|----------------|-----------------------------------------------------------------|
| A     | worker/     | P9-A1 … P9-A5  | ipc.py, worker_main.py, base.py, executor.py, pipeline_cache.py |
| B     | worker/     | P9-B1, P9-B2   | ZiT nodes, defaults.py, requirements, parity test               |

---

## Prerequisites

- P8-B1 complete: the full Rust integration test suite passes, confirming the Rust side of the IPC contract is correct.
- `worker/ipc.py` has the binary-mode guard at module top (from P1-A4). The `read_frame`/`write_frame` stubs are replaced in P9-A1.
- `worker/worker_main.py` has a non-functional stub (from P1-A4). It is replaced in P9-A2.

---

## Contract Documents Applicable to This Phase

| Document section          | Relevant tasks | What must match                                                     |
|---------------------------|----------------|---------------------------------------------------------------------|
| `ANVILML_DESIGN.md` §7.1  | P9-A1          | 4-byte big-endian u32 prefix; named msgpack maps; read-fully loops; flush after every write |
| `ANVILML_DESIGN.md` §7.2  | P9-A1, P9-A2   | `WorkerMessage` variant names and field names (msgpack keys)        |
| `ANVILML_DESIGN.md` §7.3  | P9-A1, P9-A2   | `WorkerEvent` variant names and field names                         |
| `ANVILML_DESIGN.md` §14.1 | P9-A2          | Startup sequence exact order (thread vars before torch import)      |
| `ANVILML_DESIGN.md` §14.2 | P9-A4          | `run_graph` cancel-flag check placement and error handling          |
| `ANVILML_DESIGN.md` §14.3 | P9-A3          | `BaseNode` ABC contract; `NODE_REGISTRY` dict                       |
| `ANVILML_DESIGN.md` §14.4 | P9-A5          | LRU eviction policy; `empty_cache()` call rules                     |
| `ANVILML_DESIGN.md` §14.6 | P9-B1          | ZiT node input/output slot names exactly as in the table            |

---

## Task Descriptions

### Group A — Core Worker Infrastructure

#### P9-A1: worker/ipc.py — read_frame and write_frame (Python-side framing)

**Goal:** Replace the `NotImplementedError` stubs in `worker/ipc.py` with a correct implementation of the 4-byte length-prefix msgpack framing, mirroring the Rust framing layer exactly.

**Files to create or modify:**
- `worker/ipc.py` — replace stub functions; binary-mode guard already present from P1-A4

**Key implementation notes:**
- `read_frame() -> dict`: Read exactly 4 bytes from `sys.stdin.buffer` using a loop: `while len(buf) < 4: buf += sys.stdin.buffer.read(4 - len(buf))`. Decode as big-endian unsigned int: `n = struct.unpack('>I', header)[0]`. Read exactly N bytes using the same loop pattern. Deserialize: `msgpack.unpackb(payload, raw=False)`.
- `write_frame(msg: dict)`: Serialize: `payload = msgpack.packb(msg, use_bin_type=True)`. Prepend: `header = struct.pack('>I', len(payload))`. Write: `sys.stdout.buffer.write(header + payload)`. Flush: `sys.stdout.buffer.flush()`. The flush must happen after **every** frame, not deferred — partial frames cause the Rust reader to block indefinitely.
- The read loops are mandatory. A single `sys.stdin.buffer.read(N)` call may return fewer bytes than N on Windows pipes; the same is true for `sys.stdout.buffer.write` (though less common in practice). The loop is the correct solution.
- Message format: msgpack map keys are the field names as UTF-8 strings (not integers), because `use_bin_type=True` / `raw=False` ensures Python strings round-trip correctly. This matches Rust's `rmp_serde::to_vec_named`.
- `import struct` at the top of the file.
- Write `worker/tests/test_ipc.py`: use `io.BytesIO` as a fake stdin/stdout to test a full round-trip: write a `{"type": "Ping", "seq": 1}` frame, read it back, assert equality.

**Acceptance criterion:** `pytest worker/tests/test_ipc.py -v` exits 0.

---

#### P9-A2: worker/worker_main.py — startup sequence and message loop

**Goal:** Replace the non-functional stub with the complete worker entry point implementing all startup steps and the blocking IPC message loop.

**Files to create or modify:**
- `worker/worker_main.py` — full replacement

**Key implementation notes:**
- Parse `--worker-id` and `--device-index` with `argparse` at the top of `main()`.
- Set thread count environment variables **before any import that could trigger a C extension**: `os.environ['OMP_NUM_THREADS'] = N`, and likewise `MKL_NUM_THREADS`, `OPENBLAS_NUM_THREADS`, `VECLIB_MAXIMUM_THREADS`. Read `N` from `ANVILML_NUM_THREADS` (default `14`).
- If `os.environ.get('ANVILML_WORKER_MOCK') == '1'`: skip `import torch`. Otherwise: `import torch; torch.set_num_threads(N); torch.set_num_interop_threads(M); torch.backends.cuda.matmul.allow_tf32 = False; torch.backends.cudnn.allow_tf32 = False`.
- Message loop: call `ipc.read_frame()` in a blocking loop. Dispatch on `msg['type']`: `'InitializeHardware'` → resolve device string (e.g. `f"cuda:{device_index}"` for CUDA, `"cpu"` for CPU), send `Ready { worker_id, device_index, vram_total_mib }`; `'Ping'` → send `Pong { seq }`; `'Execute'` → call `run_graph(...)` in a thread or inline; `'CancelJob'` → set `cancel_flag`; `'Shutdown'` → send `Dying { reason: "shutdown" }`, flush, exit 0; `'MemoryQuery'` → read VRAM via `torch.cuda.memory_reserved` (or 0 in mock), send `MemoryReport`.
- Background `MemoryReport` thread: `threading.Thread(daemon=True)` that every 10 s sends `MemoryReport { vram_used_mib, ram_used_mib }`. In mock mode, both values are 0.
- stderr logging: use Python's `logging` module with `logging.basicConfig(stream=sys.stderr, level=logging.INFO)`. Never write to stdout except via `ipc.write_frame`.

**Acceptance criterion:** `ANVILML_WORKER_MOCK=1 python worker/worker_main.py --worker-id worker-0 --device-index 0` reads a `Ping` frame from stdin, writes a `Pong` to stdout, then exits on `Shutdown`. Verified by the Rust Ping→Pong integration test from P5-A3.

---

#### P9-A3: worker/nodes/base.py — BaseNode, NodeContext, NODE_REGISTRY

**Goal:** Define the node infrastructure that all node implementations build on.

**Files to create or modify:**
- `worker/nodes/base.py` — `NodeContext`, `BaseNode`, `NODE_REGISTRY`, `@register`
- `worker/nodes/__init__.py` — auto-import all node modules

**Key implementation notes:**
- `NodeContext`: a `dataclasses.dataclass` with fields: `pipeline_cache` (a `PipelineCache` instance or `None` in mock), `device_str: str`, `emit_fn: Callable[[dict], None]` (calls `ipc.write_frame`), `cancel_flag: threading.Event`.
- `BaseNode(ABC)`: `NODE_TYPE: ClassVar[str]`; `INPUT_SLOTS: ClassVar[list[str]]`; `OUTPUT_SLOTS: ClassVar[list[str]]`; `__init__(self, ctx: NodeContext)`; `@abstractmethod execute(self, **inputs) -> dict[str, Any]`.
- `NODE_REGISTRY: dict[str, type[BaseNode]] = {}`.
- `register` decorator: `NODE_REGISTRY[cls.NODE_TYPE] = cls; return cls`. Used as `@register` on each concrete node class.
- `nodes/__init__.py`: `from worker.nodes import zit, sdxl, common` — explicit imports so the registry is always populated on `import worker.nodes`.
- Write `worker/tests/test_nodes_base.py`: import `worker.nodes`, assert `'ZitLoadPipeline'` in `NODE_REGISTRY`; create a subclass without implementing `execute` and assert `TypeError` on instantiation.

**Acceptance criterion:** `ANVILML_WORKER_MOCK=1 pytest worker/tests/test_nodes_base.py -v` exits 0.

---

#### P9-A4: worker/executor.py — run_graph with topo-sort, cancel_flag, exception handling

**Goal:** Implement the graph execution engine that drives the node execution loop, handles cancellation at every node boundary, and emits the correct IPC events for each outcome.

**Files to create or modify:**
- `worker/executor.py` — `run_graph(graph, settings, device_str, cancel_flag, emit_fn) -> None`

**Key implementation notes:**
- Parse `graph['nodes']` into a list of node dicts. Build an adjacency list (node_id → list of node_ids that consume its outputs, via edge refs). Run Kahn's algorithm to produce a topological order. On cycle: `emit_fn({'type': 'Failed', 'job_id': job_id, 'error': 'cycle_detected', 'traceback': ''})` and return.
- Per-node loop:
  1. Check `cancel_flag.is_set()` → `emit_fn({'type': 'Cancelled', 'job_id': job_id})` and return.
  2. Resolve inputs: literal values pass through; edge refs `{'node_id': ..., 'output_slot': ...}` look up `node_outputs[node_id][output_slot]`. If a ref cannot be resolved → `Failed` and return.
  3. `node_cls = NODE_REGISTRY.get(node['type'])`; if not found → `Failed` and return.
  4. `ctx = NodeContext(pipeline_cache=cache, device_str=device_str, emit_fn=emit_fn, cancel_flag=cancel_flag)`.
  5. Try `result = node_cls(ctx).execute(**resolved_inputs)`. Catch `CancelledError` (raised by sampler step callback) → emit `Cancelled` and return. Catch any other exception → emit `Failed { error: str(e), traceback: traceback.format_exc() }` and return.
  6. Store `node_outputs[node['id']] = result`.
  7. Emit `Progress { job_id, node_index, node_total, node_type: node['type'] }`.
- After all nodes: `emit_fn({'type': 'Completed', 'job_id': job_id, 'elapsed_ms': int((time.monotonic() - start) * 1000)})`.

**Acceptance criterion:** `pytest worker/tests/test_executor.py -v` exits 0 covering all 5 scenarios.

---

#### P9-A5: worker/pipeline_cache.py — LRU cache and OOM trap

**Goal:** Implement the in-worker LRU pipeline cache and wire the OOM trap into the executor's node dispatch.

**Files to create or modify:**
- `worker/pipeline_cache.py` — `PipelineCache`
- `worker/executor.py` — wrap `node.execute()` with OOM trap

**Key implementation notes:**
- `PipelineCache(max_entries: int = 4)`. Uses `collections.OrderedDict`.
- `get_or_load(model_id: str, dtype: str, loader: Callable[[], Any]) -> Any`:
  - If `(model_id, dtype)` in cache: move to MRU end (`cache.move_to_end(key)`), return value.
  - Miss: call `_ensure_space(est_vram_mib)`, then `pipeline = loader()`, store, return.
- `_ensure_space(est_vram_mib)`: while `free_vram_mib < est_vram_mib` and cache non-empty: pop LRU entry (first item in OrderedDict), `del pipeline` reference, `torch.cuda.empty_cache()` once per eviction. In mock mode (torch absent): skip the VRAM check and always load without eviction.
- OOM trap in `executor.py`: wrap step 5 with `try/except torch.cuda.OutOfMemoryError`: delete partial output variables, call `torch.cuda.empty_cache()`, emit `Failed { error: 'cuda_oom', traceback: ... }`, return without re-raising. In mock mode (torch absent): the OOM trap `except` clause is unreachable; do not import `torch` in `executor.py` directly — import it only if needed.
- Write `worker/tests/test_pipeline_cache.py`: test hit returns same object; miss calls loader; eviction calls `del` on LRU.

**Acceptance criterion:** `ANVILML_WORKER_MOCK=1 pytest worker/tests/test_pipeline_cache.py -v` exits 0.

---

### Group B — ZiT Nodes and Completion

#### P9-B1: worker/nodes/zit.py — ZiT node set

**Goal:** Implement all four ZiT inference nodes and the shared `SaveImage` node, with both real and mock execution paths.

**Files to create or modify:**
- `worker/nodes/zit.py` — `ZitLoadPipeline`, `ZitTextEncode`, `ZitSampler`, `ZitDecode`
- `worker/nodes/common.py` — `SaveImage`

**Key implementation notes:**
- All nodes use `@register` and inherit `BaseNode`. Declare `NODE_TYPE`, `INPUT_SLOTS`, `OUTPUT_SLOTS` matching `ANVILML_DESIGN.md §14.6` exactly.
- **Mock mode** (`ANVILML_WORKER_MOCK=1`, no torch): `ZitLoadPipeline.execute(model_id)` → `{'pipeline': 'mock_pipeline'}`. `ZitTextEncode.execute(pipeline, prompt)` → `{'conditioning': 'mock_cond'}`. `ZitSampler.execute(pipeline, conditioning, steps, seed)`: resolve seed: if `seed == -1` → `random.randint(0, 2**32-1)`. Return `{'latents': b'mock_latents', 'seed': resolved_seed}`. `ZitDecode.execute(pipeline, latents)` → `{'image': Image.new('RGB', (64, 64), color=0)}` (black image). `SaveImage.execute(image, prompt, seed, steps)`: encode image to PNG bytes via `io.BytesIO`, `base64.b64encode`, emit `ImageReady { job_id, image_b64, width, height, format='png', seed, steps, prompt }`. Note: `SaveImage` gets `job_id` from `NodeContext`.
- **Real mode**: `ZitLoadPipeline` calls `ctx.pipeline_cache.get_or_load(model_id, dtype, loader)` where loader instantiates the diffusers pipeline class for ZiT. `ZitSampler` passes `cancel_flag` as a `callback_on_step_end` that raises `CancelledError` if the flag is set.
- `NodeContext` must expose `job_id` for `SaveImage` to include it in the `ImageReady` event. Add `job_id: str` to `NodeContext`.
- Write `worker/tests/test_nodes_zit.py` with `ANVILML_WORKER_MOCK=1`: assert each node returns its declared output slots; assert `SaveImage` calls `emit_fn` with a dict whose `type == 'ImageReady'`.

**Acceptance criterion:** `ANVILML_WORKER_MOCK=1 pytest worker/tests/test_nodes_zit.py -v` exits 0.

---

#### P9-B2: worker/defaults.py, requirements, and parity test

**Goal:** Define the per-model generation defaults, populate the `requirements/*.txt` files with real package versions, and add the parity test that ensures the Python `NODE_REGISTRY` matches the Rust `KNOWN_NODE_TYPES`.

**Files to create or modify:**
- `worker/defaults.py` — `ModelDefaults`, `ZIT_DEFAULTS`, `SDXL_DEFAULTS`
- `worker/requirements/base.txt` — real dependency list
- `worker/requirements/cuda.txt`, `rocm.txt`, `cpu.txt` — torch wheel index entries
- `backend/tests/known_node_types.json` — JSON array listing the 9 MVP node type names
- `worker/tests/test_parity.py` — parity assertion

**Key implementation notes:**
- `defaults.py`: `ModelDefaults` is a simple dataclass or namedtuple. `ZIT_DEFAULTS = ModelDefaults(steps=8, guidance_scale=0.0, width=1024, height=1024, dtype='bf16')`. `SDXL_DEFAULTS = ModelDefaults(steps=20, guidance_scale=7.5, width=1024, height=1024, dtype='fp16', supports_negative_prompt=True)`.
- `requirements/base.txt`: `diffusers>=0.27`, `transformers>=4.40`, `accelerate`, `pillow>=10.0`, `msgpack>=1.0`, `numpy>=1.26`, `safetensors>=0.4`, `pytest>=8.0`.
- `requirements/cuda.txt`: `--extra-index-url https://download.pytorch.org/whl/cu121` on the first line, then `torch>=2.2`.
- `requirements/rocm.txt`: `--extra-index-url https://download.pytorch.org/whl/rocm5.7` then `torch>=2.2`.
- `requirements/cpu.txt`: `--extra-index-url https://download.pytorch.org/whl/cpu` then `torch>=2.2`.
- `backend/tests/known_node_types.json`: `["ZitLoadPipeline","ZitTextEncode","ZitSampler","ZitDecode","SdxlLoadPipeline","SdxlTextEncode","SdxlSampler","SdxlDecode","SaveImage"]`. This file is the source of truth for the parity test and is read by both the Python test and (in phase 010) potentially by the Rust parity test.
- `test_parity.py`: load `known_node_types.json` from the path relative to the test file (use `pathlib.Path(__file__).parent.parent.parent / 'backend/tests/known_node_types.json'`). Assert `set(NODE_REGISTRY.keys()) == set(json_list)`. Any divergence between Rust and Python node sets is a build error.

**Acceptance criterion:** `ANVILML_WORKER_MOCK=1 pytest worker/tests/ -v` exits 0 with all tests passing including parity.

---

## Phase Acceptance Criteria

```
ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/ -v
cargo test --workspace --features mock-hardware
```

---

## Known Constraints and Gotchas

- Thread environment variables (`OMP_NUM_THREADS` etc.) must be set via `os.environ` **before** any C extension is imported. PyTorch reads these at import time, not at runtime. The order in `worker_main.py` is: set env vars → (optionally) import torch. Violating this order results in thread counts that silently ignore the configured limits.
- `sys.stdout.buffer.flush()` after every `write_frame` is mandatory. Python's stdout buffer is line-buffered by default when attached to a pipe (not a tty). Without explicit flush, frames can sit in the buffer indefinitely, causing the Rust reader to block.
- `CancelledError` in Python 3.8+ is a subclass of `BaseException`, not `Exception`. The `except Exception` block in the executor will **not** catch it. Use `except BaseException` or `except (Exception, CancelledError)` if you need to differentiate, or use a dedicated `cancel_flag` check before re-raising.
- `SaveImage` must obtain `job_id` from `NodeContext`. Ensure `NodeContext.job_id` is populated when `run_graph` constructs the context for each node. Forgetting this produces `ImageReady` frames with a missing or null `job_id`, which the Rust artifact store will reject.
- The parity test path `../../backend/tests/known_node_types.json` is relative to `worker/tests/test_parity.py`. Verify the path is correct for CI, which runs `pytest` from the repository root. Use `__file__`-relative path construction to avoid CWD assumptions.
