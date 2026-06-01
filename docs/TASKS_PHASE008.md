# Tasks: Phase 008 — IPC Framing

| Field | Value |
|-------|-------|
| Phase | 008 |
| Name | IPC Framing |
| Milestone group | Worker lifecycle |
| Depends on phases | 1-7 |
| Task file | `forge/tasks/tasks_phase008.json` |
| Tasks | 4 |

## Overview

Phase 8 implements `anvilml-ipc`: the `WorkerMessage`/`WorkerEvent` enums and the length-prefixed msgpack framing (`write_frame`/`read_frame`) with the size cap and read-fully loops required for cross-platform pipe correctness. It ships a tiny `ipc-probe` binary so the framing layer has a *runnable* proof independent of any worker process.

Every task in this phase implements **one module or one endpoint** plus its test. No task touches more than its named file(s). `cargo test` and `cargo clippy` are per-task gates; the phase as a whole is only complete when the **Runnable Proof** below passes.

## Tasks

| Task | Module / File | Summary |
|------|-------------|---------|
| P8-A1 | `crates/anvilml-ipc/src/messages.rs` | anvilml-ipc: WorkerMessage and WorkerEvent enums |
| P8-A2 | `crates/anvilml-ipc/src/framing.rs` | anvilml-ipc: write_frame (length-prefixed msgpack) |
| P8-A3 | `crates/anvilml-ipc/src/framing.rs` | anvilml-ipc: read_frame with size cap and read-fully loop |
| P8-A4 | `crates/anvilml-ipc/src/bin/ipc-probe.rs` | ipc-probe: standalone CLI binary proving frame round-trip |

## Task details

#### P8-A1: anvilml-ipc: WorkerMessage and WorkerEvent enums

- **Prereqs:** P7-A5
- **Tags:** —

Add serde, rmp-serde, uuid(serde), anvilml-core to anvilml-ipc. Create src/messages.rs per 7.2/7.3. WorkerMessage: Ping{seq}, Shutdown, InitializeHardware{device_str}, Execute{job_id,graph,settings,device_index}, CancelJob{job_id}, MemoryQuery. WorkerEvent: Ready{worker_id,device_index,vram_total_mib,vram_free_mib,arch,fp16,bf16,flash_attention}, Pong{seq}, Dying{reason}, MemoryReport{vram_used_mib,ram_used_mib}, Progress, ImageReady{job_id,image_b64,width,height,seed,steps,prompt}, Completed{job_id,elapsed_ms}, Failed{job_id,error,traceback}, Cancelled{job_id}. test messages exits 0.

#### P8-A2: anvilml-ipc: write_frame (length-prefixed msgpack)

- **Prereqs:** P8-A1
- **Tags:** —

Add tokio (io-util) to anvilml-ipc. Create src/framing.rs: async fn write_frame<W:AsyncWrite+Unpin>(w:&mut W, msg:&WorkerMessage)->Result<(),AnvilError>. Serialize via rmp_serde::to_vec_named, prepend 4-byte big-endian u32 length, write_all header+payload. cargo test -p anvilml-ipc -- write_frame exits 0: write to Vec, assert first 4 bytes equal payload len big-endian.

#### P8-A3: anvilml-ipc: read_frame with size cap and read-fully loop

- **Prereqs:** P8-A2
- **Tags:** reasoning

Add to framing.rs: async fn read_frame<R:AsyncRead+Unpin>(r:&mut R, max_mib:u32)->Result<WorkerEvent,AnvilError>. read_exact 4 bytes, decode big-endian u32 N, if N>max_mib*1024*1024 return PayloadTooLarge BEFORE allocating, else read_exact N bytes, rmp_serde::from_slice. read_exact loops internally (critical for Windows partial reads). cargo test -p anvilml-ipc -- read_frame exits 0: round-trip via tokio::io::duplex; oversize header rejected before payload read. Also pass: cargo check --target x86_64-pc-windows-gnu --features mock-hardware.

#### P8-A4: ipc-probe: standalone CLI binary proving frame round-trip

- **Prereqs:** P8-A3
- **Tags:** —

Create a small bin target ipc-probe (in anvilml-ipc as [[bin]] or a tiny crate). It writes a Ping{seq:7} frame to an in-process duplex, reads it back, prints 'OK seq=7' and exits 0; on mismatch exits 1. This gives a runnable proof of the framing layer independent of any worker. Verify: cargo run -p anvilml-ipc --bin ipc-probe prints OK seq=7.


## Runnable Proof

Run the standalone framing probe.

```bash
cargo run -p anvilml-ipc --bin ipc-probe
```

Expected: prints `OK seq=7` and exits 0 — it wrote a `Ping{seq:7}` frame to an in-process pipe, read it back, and verified the round-trip including the 4-byte big-endian length prefix. Phase done when `ipc-probe` prints `OK seq=7` and `cargo test -p anvilml-ipc` is green (round-trip + oversize-rejection tests).
