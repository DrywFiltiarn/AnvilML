# Tasks: Phase 021 — Real Python Worker — ZiT

| Field | Value |
|-------|-------|
| Phase | 021 |
| Name | Real Python Worker — ZiT |
| Milestone group | Real inference |
| Depends on phases | 1-20 |
| Task file | `forge/tasks/tasks_phase021.json` |
| Tasks | 7 |

## Overview

Phase 21 replaces the inline mock with the real worker engine: `nodes/base.py` (BaseNode/registry), `executor.py` (real topo-sort/cancel/exception handling), `pipeline_cache.py` (LRU + OOM trap), `defaults.py`, real ZiT diffusers nodes, and the Rust/Python `KNOWN_NODE_TYPES` parity test. Mock branches are preserved so CI stays hermetic. After this phase, on real hardware with a real model, a ZiT job produces a genuine generated image.

Every task in this phase implements **one module or one endpoint** plus its test. No task touches more than its named file(s). `cargo test` and `cargo clippy` are per-task gates; the phase as a whole is only complete when the **Runnable Proof** below passes.

## Tasks

| Task | Module / File | Summary |
|------|---------------|---------|
| P21-A1 | `worker/nodes/base.py` | worker: nodes/base.py BaseNode + NodeContext + NODE_REGISTRY + @register |
| P21-A2 | `worker/executor.py` | worker: executor.py run_graph (topo-sort, cancel, exceptions) |
| P21-A3 | `worker/pipeline_cache.py` | worker: pipeline_cache.py LRU + OOM trap |
| P21-A4 | `worker/defaults.py` | worker: defaults.py + requirements (cuda/rocm/cpu) populated |
| P21-A5 | `worker/nodes/zit.py` | worker: nodes/zit.py real ZiT nodes + nodes/common.py SaveImage |
| P21-A6 | `worker/tests/test_parity.py` | worker: parity test KNOWN_NODE_TYPES == NODE_REGISTRY |
| P21-A7 | `docs/PROOF_phase021.md` | anvilml: real ZiT end-to-end smoke proof (manual, real hardware) |

## Task details

#### P21-A1: worker: nodes/base.py BaseNode + NodeContext + NODE_REGISTRY + @register

- **Prereqs:** P20-A4
- **Tags:** —

Create worker/nodes/base.py: NodeContext dataclass (pipeline_cache, device_str, emit_fn, cancel_flag: threading.Event, job_id:str). BaseNode(ABC) with ClassVars NODE_TYPE, INPUT_SLOTS, OUTPUT_SLOTS; __init__(ctx); abstract execute(**inputs)->dict. NODE_REGISTRY dict + @register decorator. nodes/__init__.py imports node modules to populate registry. pytest worker/tests/test_nodes_base.py exits 0: @register populates; missing execute raises TypeError.

#### P21-A2: worker: executor.py run_graph (topo-sort, cancel, exceptions)

- **Prereqs:** P21-A1
- **Tags:** reasoning

Create worker/executor.py: run_graph(graph,settings,device_str,cancel_flag,emit_fn,pipeline_cache,job_id). Kahn topo-sort. Per node: cancel_flag set->emit Cancelled+return; resolve inputs (literals + edge refs from node_outputs); NODE_REGISTRY[type](ctx).execute(**inputs); store outputs; emit Progress. Catch CancelledError->Cancelled; other Exception->Failed{error,traceback}. End->Completed{elapsed_ms}. Replace the inline mock loop in worker_main with run_graph. pytest worker/tests/test_executor.py exits 0 (mock nodes): valid, cycle, exception, cancel.

#### P21-A3: worker: pipeline_cache.py LRU + OOM trap

- **Prereqs:** P21-A2
- **Tags:** reasoning

Create worker/pipeline_cache.py: PipelineCache(max_entries=4) OrderedDict keyed (model_id,dtype). get_or_load(model_id,dtype,loader): hit->move_to_end+return; miss->evict LRU while free_vram<est (del + torch.cuda.empty_cache once per eviction) then load. In executor wrap execute() in try/except torch.cuda.OutOfMemoryError -> drop partial, empty_cache, emit Failed{error:'cuda_oom'}, stay Idle. Skip OOM trap path when ANVILML_WORKER_MOCK=1 (torch absent). pytest worker/tests/test_pipeline_cache.py exits 0.

#### P21-A4: worker: defaults.py + requirements (cuda/rocm/cpu) populated

- **Prereqs:** P21-A3
- **Tags:** —

Create worker/defaults.py: ModelDefaults; ZIT_DEFAULTS(steps=8,guidance=0.0,1024,1024,bf16); SDXL_DEFAULTS(steps=20,guidance=7.5,1024,1024,fp16,neg_prompt True). Populate base.txt (diffusers>=0.27,transformers>=4.40,accelerate,pillow,msgpack,numpy,safetensors,pytest); cuda.txt/rocm.txt/cpu.txt with torch wheel index URLs + torch>=2.2. No test beyond import. pytest worker/tests passes (import defaults).

#### P21-A5: worker: nodes/zit.py real ZiT nodes + nodes/common.py SaveImage

- **Prereqs:** P21-A4
- **Tags:** reasoning

Create worker/nodes/zit.py (ZitLoadPipeline via pipeline_cache+diffusers; ZitTextEncode; ZitSampler resolves seed=-1, cancel via callback_on_step_end raising CancelledError; ZitDecode) and worker/nodes/common.py SaveImage (encode PNG, emit ImageReady with ctx.job_id). Keep mock branches (ANVILML_WORKER_MOCK=1) returning sentinels/black image as before so CI stays hermetic. Slots exactly per 14.6. pytest worker/tests/test_nodes_zit.py (mock) exits 0: output slots correct; SaveImage emits ImageReady.

#### P21-A6: worker: parity test KNOWN_NODE_TYPES == NODE_REGISTRY

- **Prereqs:** P21-A5
- **Tags:** —

Create backend/tests/known_node_types.json: the 9 node-type names array. Create worker/tests/test_parity.py loading it via __file__-relative path and asserting set(NODE_REGISTRY.keys())==set(json). Also add a Rust test in anvilml-scheduler reading the same JSON and asserting equality with KNOWN_NODE_TYPES. ANVILML_WORKER_MOCK=1 pytest worker/tests exits 0; cargo test -p anvilml-scheduler -- parity exits 0.

#### P21-A7: anvilml: real ZiT end-to-end smoke proof (manual, real hardware)

- **Prereqs:** P21-A6
- **Tags:** —

No new code. Document docs/PROOF_phase021.md: provision venv (install_worker_deps), place a real ZiT model in models/diffusion/, run the binary WITHOUT mock (ANVILML_WORKER_MOCK unset), submit a ZiT graph via curl, observe progress over /v1/events, fetch the produced PNG via /v1/artifacts/:hash and confirm it is a real generated image (not black). Complete when a human verifies a real image is generated end-to-end.


## Runnable Proof

On a machine with a GPU and a real ZiT model, generate a real image end-to-end (no mock).

```bash
bash backend/scripts/install_worker_deps.sh          # builds ./venv with torch
cp <your-zit-model>.safetensors models/diffusion/
ANVILML_VENV_PATH=./venv cargo run --release          # NOTE: no ANVILML_WORKER_MOCK
# submit a ZiT job, watch /v1/events for progress, then:
curl -s -o real.png http://127.0.0.1:8488/v1/artifacts/<hash>
```

Expected: the job runs through real ZiT inference, `/v1/events` shows progress per node, and `real.png` is an actual generated image (not the 64x64 black mock). CI proof (hermetic): `ANVILML_WORKER_MOCK=1 pytest worker/tests` and `cargo test -p anvilml-scheduler -- parity` are green. Phase done when a real ZiT image is produced end-to-end.
