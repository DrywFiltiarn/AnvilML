# Tasks: Phase 008 — ZeroMQ IPC Transport

| Field | Value |
|-------|-------|
| Phase | 008 |
| Name | ZeroMQ IPC Transport |
| Project | anvilml |
| Status | Approved |
| Depends on phases | 7 |

## Overview

Phase 008 is the most critical phase in the build. Everything from Phase 009 onwards depends on reliable, race-free IPC between the Rust supervisor and Python workers. Phase 008 must not be rushed — its Runnable Proof includes a 1000-trip stress test that must pass before Phase 009 begins.

The IPC architecture uses ZeroMQ ROUTER/DEALER topology as specified in `ANVILML_DESIGN.md §8`. The Rust supervisor binds one `RouterTransport` (wrapping a `zeromq::RouterSocket`) before spawning any workers. Each Python worker connects a `zmq.DEALER` socket with its `worker_id` as the ZeroMQ identity. ZeroMQ handles routing automatically via identity frames — no manual framing is needed.

All messages are msgpack-serialised flat dicts with a `_type` discriminator key. The `zeromq` crate version is `0.6.x` (current stable). Do not use `0.4.x`.

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | anvilml-ipc Rust | P8-A1 … P8-A3 | WorkerMessage/WorkerEvent enums, msgpack codecs, RouterTransport |
| B | worker Python | P8-B1 … P8-B2 | ipc.py DEALER transport, test_ipc.py |
| C | Integration | P8-C1 | 1000-trip stress test: RouterTransport ↔ mock Python worker |

## Prerequisites

Phase 007 complete. `WsEvent`, `WorkerMessage`, `WorkerEvent`, `NodeTypeDescriptor` types exist in `anvilml-core`.

## Interfaces and Contracts

| Contract document | Relevant tasks | What must match |
|-------------------|---------------|-----------------|
| `ANVILML_DESIGN.md §8.4` | P8-A1 | `WorkerMessage` variant names and field names |
| `ANVILML_DESIGN.md §8.5` | P8-A1 | `WorkerEvent` variant names and field names |
| `ANVILML_DESIGN.md §8.6` | P8-A1, P8-B1 | `_type` discriminator; msgpack flat dict format |
| `ANVILML_DESIGN.md §8.7` | P8-A2, P8-A3 | `RouterTransport` public API |

## Task Descriptions

### Group A — anvilml-ipc Rust

#### P8-A1: anvilml-ipc: WorkerMessage and WorkerEvent enums with msgpack codecs

**Goal:** Create `crates/anvilml-ipc/src/messages.rs` with `WorkerMessage` and `WorkerEvent` enums per `ANVILML_DESIGN.md §8.4–8.5`. Implement `encode_message(msg: &WorkerMessage) -> Result<Vec<u8>>` and `decode_event(bytes: &[u8]) -> Result<WorkerEvent>` using `rmp-serde` with flat-dict serialisation (discriminator key `_type`).

**Files to create:**
- `crates/anvilml-ipc/src/messages.rs`

**Acceptance criterion:** `cargo test -p anvilml-ipc -- messages` exits 0 with ≥ 8 tests (roundtrip for each major WorkerMessage variant and each major WorkerEvent variant).

#### P8-A2: anvilml-ipc: RouterTransport bind and send

**Goal:** Implement `RouterTransport::bind() -> Result<Self>` in `crates/anvilml-ipc/src/transport.rs`. Bind a `zeromq::RouterSocket` to `tcp://127.0.0.1:0`. Read the OS-assigned port. Store socket in `Arc<tokio::sync::Mutex<RouterSocket>>`. Implement `pub async fn send(&self, worker_id: &str, msg: &WorkerMessage) -> Result<()>` prepending the identity frame.

**Acceptance criterion:** `cargo test -p anvilml-ipc -- transport` exits 0; bind succeeds and returns a non-zero port.

#### P8-A3: anvilml-ipc: RouterTransport recv

**Goal:** Implement `pub async fn recv(&self) -> Result<(String, WorkerEvent)>` on `RouterTransport`. Read identity frame, strip it, decode the msgpack payload into `WorkerEvent`. Return `(worker_id, event)`.

**Acceptance criterion:** `cargo test -p anvilml-ipc -- roundtrip` exits 0: in-process DEALER socket sends an event; RouterTransport.recv() returns correct (id, event) pair.

### Group B — worker Python

#### P8-B1: worker/ipc.py: ZeroMQ DEALER transport

**Goal:** Implement `worker/ipc.py` per `ANVILML_DESIGN.md §13.2` using `zmq.DEALER` with identity set to `ANVILML_WORKER_ID`. Functions: `connect(port, worker_id)`, `send_event(data: dict)`, `recv_message() -> dict`. All messages are msgpack flat dicts with `_type` key.

**Files to create:**
- `worker/ipc.py`
- `worker/requirements/base.txt` with `pyzmq>=26.0`, `msgpack>=1.0`, `pillow>=10.0`, `safetensors>=0.4`, `pytest>=8.0`

**Acceptance criterion:** `python3 -c "from worker import ipc; print('ok')"` exits 0; pytest worker/tests/test_ipc.py exits 0.

#### P8-B2: worker/tests/test_ipc.py

**Goal:** Create `worker/tests/test_ipc.py` testing `ipc.py` using in-process ZeroMQ socket pairs. Tests: `connect()` succeeds; `send_event()` sends correct msgpack dict; `recv_message()` deserialises correctly; identity frame attached. Use `ANVILML_WORKER_MOCK=1` env var. Restore env vars unconditionally.

**Acceptance criterion:** `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_ipc.py -v` exits 0 with ≥ 6 tests.

### Group C — Integration stress test

#### P8-C1: anvilml-ipc: 1000-trip RouterTransport stress test

**Goal:** Create `crates/anvilml-ipc/tests/stress_test.rs`: bind `RouterTransport`; spawn a mock Python subprocess (`worker/ipc.py` + minimal echo loop); send 1000 Ping messages; assert 1000 Pong responses arrive with matching seq values; assert 0 timeouts. Test must complete within 30 seconds.

**Acceptance criterion:** `cargo test -p anvilml-ipc --features mock-hardware --test stress_test` exits 0; all 1000 trips complete; log shows "stress test passed: 1000/1000".

## Phase Acceptance Criteria

```bash
cargo test -p anvilml-ipc --features mock-hardware
ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_ipc.py -v
cargo test -p anvilml-ipc --features mock-hardware --test stress_test
```

All three must exit 0 before Phase 009 begins.

## Known Constraints and Gotchas

- Use `zeromq = { version = "0.6", features = ["tokio"] }` in workspace. Do NOT use 0.4.x — it lacks correct ROUTER/DEALER support.
- The DEALER socket identity must be set via `setsockopt(zmq.IDENTITY, worker_id.encode())` BEFORE calling `connect()`. Setting identity after connect has no effect.
- `RouterTransport.recv()` receives a multipart message: `[identity_frame, empty_delimiter, payload_frame]` for ROUTER sockets. Strip the identity and delimiter before decoding the payload.
- The stress test requires the Python venv to be provisioned (`pyzmq` and `msgpack` installed). Ensure `ANVILML_VENV_PATH` is set or use the default `./worker/.venv`.
