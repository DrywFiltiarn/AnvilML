# Tasks: Phase 907 — ZeroMQ IPC Transport

| Field | Value |
|-------|-------|
| Phase | 907 |
| Name | ZeroMQ IPC Transport |
| Milestone group | IPC stability |
| Depends on phases | 1–21 |
| Task file | `.forge/tasks/tasks_phase907.json` |
| Tasks | 9 |

---

## Overview

Phase 907 replaces the `interprocess` named-pipe / Unix-socket IPC transport with
ZeroMQ (`zeromq` Rust crate + `pyzmq` Python package). The existing transport is
structurally unreliable on Windows: named pipes require IOCP read registration before
the writer sends data, the sequencing of `reader_task` start relative to `write_frame`
is not guaranteed with a fixed yield count, and the `_WindowsPipeSocket` wrapper in
`ipc.py` bypasses Python's normal socket abstraction. These races manifest as the
60-second Ready timeout panic observed during development.

ZeroMQ eliminates the race entirely. The supervisor binds a DEALER socket on an
OS-assigned TCP port before spawning the worker. The worker connects via
`zmq.DEALER` socket after startup. ZeroMQ handles connection, framing, and
backpressure internally on both platforms without any IOCP registration sequencing
or pipe-buffer management by application code.

The custom 4-byte length-prefix framing in `anvilml-ipc/src/framing.rs` is made
redundant by ZeroMQ's built-in message framing. Task A8 removes it from the data
path while retaining `anvilml-ipc` for its message type definitions
(`WorkerMessage`, `WorkerEvent`).

The msgpack serialisation format for `WorkerMessage` and `WorkerEvent` is unchanged.
The Python worker's message dispatch logic is unchanged. Only the transport layer
is replaced.

---

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | anvilml-worker + worker/ | P907-A1 – P907-A8 | Full transport replacement, Rust and Python sides |
| B | backend integration test | P907-B1 | Smoke test proving reliable handshake |

---

## Prerequisites

- `P21-A7` complete (Phase 21 terminal task — real ZiT smoke proof)
- `pyzmq>=26.0` installable in the project venv before P907-A5 runs

---

## Prereq update required after this phase

Update `tasks_phase022.json` (and any subsequent phase that currently prereqs
`P21-A7` or any Phase 903 IPC task) to prereq `P907-B1` instead, so no
subsequent phase begins before the ZeroMQ transport is verified stable.

Update `docs/ENVIRONMENT.md` §3.7 manually: replace `ANVILML_IPC_SOCKET`
with `ANVILML_IPC_PORT` (u16 decimal string, TCP port assigned by supervisor).
This is a human-owned documentation change, not a Forge task.

---

## Architecture: ZeroMQ socket topology

```
Rust supervisor (ManagedWorker)          Python worker
┌─────────────────────────────┐         ┌──────────────────────────┐
│ zeromq::Socket (DEALER)     │         │ zmq.DEALER               │
│ bind tcp://127.0.0.1:0      │◄───────►│ connect tcp://127.0.0.1: │
│ port = local_addr().port()  │         │   {ANVILML_IPC_PORT}     │
└─────────────────────────────┘         └──────────────────────────┘
         │                                         │
   writer_task: send msgpack bytes          ipc.write_frame()
   reader_task: recv msgpack bytes          ipc.read_frame()
```

Key properties:
- Supervisor binds **before** spawning the worker — no race on bind vs connect.
- Worker connects **before** calling `_probe_hardware()` — ZeroMQ queues outbound
  messages until the peer is ready; no blocking write.
- Port is passed via `ANVILML_IPC_PORT` env var injected by `build_worker_env`.
- No named pipes, no Unix sockets, no IOCP registration, no `_WindowsPipeSocket`.

---

## Interfaces and Contracts

| Contract | Relevant tasks | What must be preserved |
|---|---|---|
| `WorkerMessage` / `WorkerEvent` msgpack format | A4, A5, A8 | All field names and types unchanged; only framing wrapper removed |
| `ANVILML_IPC_PORT` env var | A3, A5 | u16 decimal string; replaces `ANVILML_IPC_SOCKET` |
| `GET /v1/workers` response shape | B1 | `status: "Idle"` within 30s of server start |
| `build_worker_env` public API | A3 | Signature unchanged; only env var key changes |

---

## Known Constraints and Gotchas

- **zeromq crate async runtime**: The `zeromq` crate with `tokio` feature is
  fully async and compatible with the existing tokio runtime in `anvilml-worker`.
  Do not use the blocking `zmq` crate (C binding) — use `zeromq` (pure Rust).
- **DEALER socket identity frames**: ZeroMQ DEALER sockets prepend an empty
  identity frame in some configurations. Use `send_multipart` / `recv_multipart`
  if raw `send`/`recv` produces framing artifacts. Verify in P907-A6 tests.
- **Port 0 binding**: `bind("tcp://127.0.0.1:0")` assigns an ephemeral port.
  Read the actual port via `socket.get_last_endpoint()` or equivalent after bind.
  Confirm the zeromq crate API for this in the PLAN session.
- **anvilml-ipc framing.rs retained**: The file is not deleted in A8 — only its
  call sites in managed.rs are removed. The file may be deleted in a later cleanup
  phase if no other consumer remains.
- **mock-hardware tests**: All managed.rs tests that use `inject_handles_for_test`
  must be updated in A7 to use in-process ZeroMQ PAIR sockets instead of
  interprocess local socket halves. PAIR sockets support bidirectional messaging
  without the DEALER/ROUTER identity frame complexity.
- **Windows cross-check required on every task**: `cargo check --workspace
  --features mock-hardware --target x86_64-pc-windows-gnu` must exit 0 after
  each Rust task. ZeroMQ is pure Rust (`zeromq` crate) with no platform-specific
  FFI, so cross-compilation should be clean.

---

## Runnable Proof (phase complete when this passes)

```bash
# 1. Workspace builds and lints clean
cargo build --workspace --features mock-hardware
cargo clippy --workspace --features mock-hardware -- -D warnings

# 2. All Rust tests pass
cargo test --workspace --features mock-hardware

# 3. ZeroMQ handshake integration test passes
cargo test --workspace --features mock-hardware --test api_worker_zmq

# 4. Python worker tests pass (mock mode)
ANVILML_WORKER_MOCK=1 pytest worker/tests/ -v

# 5. Windows cross-check
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu

# 6. Live smoke: server starts, worker reaches Idle, no panic
ANVILML_WORKER_MOCK=1 ANVILML_VENV_PATH=./venv cargo run --bin anvilml
# Expected: "worker reached Ready state" log within 10s, no panic
```