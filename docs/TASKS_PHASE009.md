# Tasks: Phase 009 — Worker Spawn & Handshake

| Field | Value |
|-------|-------|
| Phase | 009 |
| Name | Worker Spawn & Handshake |
| Milestone group | Worker lifecycle |
| Depends on phases | 1-8 |
| Task file | `forge/tasks/tasks_phase009.json` |
| Tasks | 6 |

## Overview

Phase 9 brings the Python worker to life: the worker package with the binary-stdio guard and framing, a mock-mode `worker_main.py` that handles Ping/Init/Shutdown, the Rust `ManagedWorker` (spawn + IPC bridge), the `WorkerPool`, and `GET /v1/workers`. After this phase the running binary spawns a real Python child process, completes the Init/Ready handshake, and reports the worker as Idle over the API.

Every task in this phase implements **one module or one endpoint** plus its test. No task touches more than its named file(s). `cargo test` and `cargo clippy` are per-task gates; the phase as a whole is only complete when the **Runnable Proof** below passes.

## Tasks

| Task | Module / File | Summary |
|------|-------------|---------|
| P9-A1 | `worker/requirements/base.txt` | worker: Python package skeleton + ipc.py binary-stdio guard + framing |
| P9-A2 | `worker/worker_main.py` | worker: worker_main.py mock-mode message loop (Ping/Pong/Init/Shutdown) |
| P9-A3 | `crates/anvilml-worker/src/env.rs` | anvilml-worker: env.rs build_worker_env |
| P9-A4 | `crates/anvilml-worker/src/managed.rs` | anvilml-worker: ManagedWorker spawn + IPC bridge (writer/reader tasks) |
| P9-A5 | `crates/anvilml-worker/src/pool.rs` | anvilml-worker: WorkerPool spawn_all + list + acquire/set status |
| P9-A6 | `backend/src/main.rs` | anvilml: spawn WorkerPool at startup + GET /v1/workers |

## Task details

#### P9-A1: worker: Python package skeleton + ipc.py binary-stdio guard + framing

- **Prereqs:** P8-A4
- **Tags:** reasoning

Create worker/ package: __init__.py, nodes/__init__.py, tests/__init__.py. worker/requirements/: base.txt (msgpack>=1.0, pillow>=10.0, numpy, safetensors, diffusers, transformers, pytest) plus torch-selectors per design 2.2/21: cuda.txt, rocm-linux.txt, rocm-windows.txt (AMD PyTorch-on-Windows, ROCm>=7.2), cpu.txt. Create worker/ipc.py: at top if win32 msvcrt.setmode O_BINARY on stdin+stdout. read_frame(): 4-byte big-endian len then N bytes, unpackb(raw=False). write_frame(): packb(use_bin_type=True), 4-byte prefix, write stdout.buffer, flush. pytest worker/tests/test_ipc.py exits 0.

#### P9-A2: worker: worker_main.py mock-mode message loop (Ping/Pong/Init/Shutdown)

- **Prereqs:** P9-A1
- **Tags:** reasoning

Create worker/worker_main.py: argparse --worker-id --device-index. Set OMP/MKL/OPENBLAS/VECLIB thread env before imports. If ANVILML_WORKER_MOCK=1 skip torch import. Loop reading frames: InitializeHardware->Ready{worker_id,device_index,vram_total_mib,vram_free_mib,arch,fp16,bf16,flash_attention} (mock: 8192/8192/gfx1100/true/true/false; real: from torch cuda/hip props + mem_get_info). Ping->Pong{seq}; MemoryQuery->MemoryReport(0,0); Shutdown->Dying{reason:shutdown}, flush, exit 0. Background thread MemoryReport every 10s. Verify: ANVILML_WORKER_MOCK=1 manual run; full proof via P9-A5 REST.

#### P9-A3: anvilml-worker: env.rs build_worker_env

- **Prereqs:** P9-A2
- **Tags:** —

Add anvilml-core + anvilml-hardware. Create src/env.rs: fn build_worker_env(device:&GpuDevice,cfg:&ServerConfig)->HashMap<String,String>. Cuda: CUDA_VISIBLE_DEVICES={idx}. Rocm: HIP_VISIBLE_DEVICES={idx} on BOTH Linux+Windows; ROCBLAS_USE_HIPBLASLT 0/1; HSA_OVERRIDE_GFX_VERSION only #[cfg(unix)] when set (Linux only, never Windows - design 8.3). All: OMP/MKL/OPENBLAS/VECLIB_NUM_THREADS, ANVILML_NUM_THREADS, ANVILML_NUM_INTEROP_THREADS, ANVILML_WORKER_ID, ANVILML_DEVICE_INDEX, ANVILML_WORKER_MOCK if set. cargo test -- env exits 0: cuda, rocm-linux (HSA), rocm-windows (no HSA), cpu.

#### P9-A4: anvilml-worker: ManagedWorker spawn + IPC bridge (writer/reader tasks)

- **Prereqs:** P9-A3
- **Tags:** reasoning

Add tokio(full), anvilml-ipc, tracing. Create src/managed.rs: ManagedWorker{worker_id,device_index,status:Arc<RwLock<WorkerStatus>>,tx:mpsc::Sender<WorkerMessage>,event_tx:broadcast::Sender}. spawn(): resolve venv python (Linux {venv}/bin/python3, Windows {venv}\Scripts\python.exe), Command with build_worker_env, piped stdin/stdout, stderr->log. Writer task mpsc->write_frame; reader task read_frame->broadcast; on EOF set Dead + WorkerStatusChanged. cargo test -p anvilml-worker --features mock-hardware -- managed exits 0 (Ping->Pong). Also: cargo check --target x86_64-pc-windows-gnu.

#### P9-A5: anvilml-worker: WorkerPool spawn_all + list + acquire/set status

- **Prereqs:** P9-A4
- **Tags:** reasoning

Create src/pool.rs: WorkerPool holding Vec<Arc<ManagedWorker>>. spawn_all(hw,cfg): one ManagedWorker per GpuDevice or one CPU worker if none. list()->Vec<WorkerInfo>. acquire_idle(Option<u32>)->Option<WorkerRef>. set_busy/set_idle. subscribe_events(). send(worker_id,msg). On Ready event set status Idle AND merge caps (arch/fp16/bf16/flash_attention/vram) into the matching GpuDevice, capabilities_source=Worker (design 5.4). Re-export from lib.rs. cargo test -p anvilml-worker --features mock-hardware -- pool exits 0: spawn_all 1 CPU worker reaches Idle after Ready.

#### P9-A6: anvilml: spawn WorkerPool at startup + GET /v1/workers

- **Prereqs:** P9-A5
- **Tags:** —

Add anvilml-worker to backend + AppState workers: Arc<WorkerPool>. In main.rs after hardware detect: WorkerPool::spawn_all(&hw,&cfg), send InitializeHardware to each, store in AppState. device_str maps Cuda AND Rocm -> 'cuda:{index}' (HIP exposes via torch.cuda on both Linux+Windows, design 6), Cpu -> 'cpu'. Create handlers/workers.rs list_workers(State)->Json<Vec<WorkerInfo>>. Wire GET /v1/workers. Verify: ANVILML_WORKER_MOCK=1 ANVILML_VENV_PATH=<venv> cargo run --features mock-hardware; curl /v1/workers shows one worker reaching status Idle.


## Runnable Proof

Provision a venv (mock mode needs only Python + msgpack), start the server, and confirm a worker reaches Idle.

```bash
python3 -m venv venv && ./venv/bin/pip install msgpack pillow
ANVILML_WORKER_MOCK=1 ANVILML_VENV_PATH=./venv \
  cargo run --features mock-hardware
# another terminal:
curl -s http://127.0.0.1:8488/v1/workers | python -m json.tool
```

Expected (200): an array with one `WorkerInfo` whose `status` transitions from `initializing` to `idle` within a second or two (re-run the curl). The server log shows the worker process spawned and a `Ready` event received. Phase done when `/v1/workers` shows a live worker reaching `idle`.
