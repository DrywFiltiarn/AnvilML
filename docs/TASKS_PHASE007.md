# Tasks: Phase 7 — IPC Foundations

**Phase:** 7
**Name:** IPC Foundations
**Project(s):** anvilml
**Status:** Draft
**Depends on phases:** 1, 2, 3

---

## Overview

This phase builds `anvilml-ipc` up to (but not including) the 1000-round-trip stress
test that gates everything downstream of it: the IPC-specific error type, the
`WorkerMessage`/`WorkerEvent` wire-protocol enums, the `RouterTransport` ROUTER
socket wrapper with its critical split-lock send/recv design, and the
`EventBroadcaster` WebSocket fan-out wrapper. The stress test itself, and the worker
pool that actually spawns and supervises Python subprocesses over this transport,
are Phase 8's scope — this phase proves the transport's basic mechanics (bind, one
send, one recv, broadcast fan-out) work correctly in isolation first.

This phase exists at this exact point in the sequence — and is read with unusual
care — because `ANVILML_DESIGN.md §8.0` opens by stating that this exact subsystem
was rewritten three times across v2 and v3, with every failure traced back to an
ownership question the design left unanswered, which an agent then had to invent an
answer to under task pressure. The design closes every one of those questions in
advance: §8.3 specifies `RouterTransport`'s send/recv split-lock shape byte-for-byte,
specifically to prevent a recorded shutdown deadlock from recurring. Every task in
this phase's Group B implements that shape exactly as written, not as re-derived.

At the start of this phase, `anvilml-ipc` is an empty stub crate (Phase 1's P1-B4).
At the end, it has a working, tested ROUTER transport that can bind, send, and
receive real msgpack-framed messages over a real loopback socket, plus a working
WebSocket event broadcaster — both fully testable without any Python subprocess
existing yet. Phase 8 depends on every type and method this phase produces.

---

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | Error & message types | P7-A1 … P7-A4 | `IpcError`, `WorkerMessage`, `WorkerEvent` (split across two tasks for size) |
| B | RouterTransport | P7-B1 … P7-B2 | Construction/bind, then the critical split-lock `send()`/`recv()` methods |
| C | EventBroadcaster | P7-C1 | WebSocket fan-out wrapper, placed here to avoid a worker↔server crate cycle |
| D | Closeout | P7-D1 | `lib.rs` re-export pass, 80-line check |

---

## Prerequisites

`anvilml-core` must export `AnvilError` (with its existing `Ipc(String)` variant
from Phase 2's P2-A1), `JobSettings` (Phase 3's P3-A1), `NodeTypeDescriptor` (Phase
3's P3-A7), and `WsEvent` (Phase 3's P3-A8/P3-A9). `anvilml-ipc` must exist as a
buildable stub crate per Phase 1's P1-B4.

---

## Interfaces and Contracts

| Contract document | Relevant to tasks | What must match |
|--------------------|--------------------|------------------|
| `ANVILML_DESIGN.md §8.0`–§8.1 | All tasks in this phase | Read in full before touching `anvilml-ipc` — this section exists specifically to prevent re-deriving a previously broken design |
| `ANVILML_DESIGN.md §8.3` | P7-B1, P7-B2 | `RouterTransport`'s exact split-lock ownership shape — byte-for-byte, not a reinterpretation |
| `ANVILML_DESIGN.md §8.4` | P7-D1 | Module layout (`error.rs`, `messages.rs`, `transport.rs`, `ws/`) |
| `ANVILML_DESIGN.md §8.5`–§8.6 | P7-A2, P7-A3, P7-A4 | Exact `WorkerMessage`/`WorkerEvent` variant lists and field shapes |
| `ANVILML_DESIGN.md §13.6` | P7-C1 | `EventBroadcaster`'s 1024-event buffer capacity |
| `ANVILML_DESIGN.md §3.2` | P7-C1 | Crate dependency graph — `EventBroadcaster` lives in `anvilml-ipc` specifically to avoid a worker↔server cycle |

---

## Task Descriptions

### Group A — Error & message types

#### P7-A1: anvilml-ipc: IPC-specific error types

**Goal:** Define the error type every IPC operation in this crate returns, with a
conversion into the existing `AnvilError::Ipc` variant so callers outside this crate
never need to know about `IpcError` directly.

**Files to create or modify:**
- `crates/anvilml-ipc/src/error.rs` — `IpcError`.
- `crates/anvilml-ipc/src/lib.rs` — adds `mod error; pub use error::IpcError;`.

**Key implementation notes:**
- `From<IpcError> for AnvilError` maps to the **existing** `AnvilError::Ipc(String)`
  variant from Phase 2's P2-A1 — do not invent a new `AnvilError` variant for this;
  the existing one already exists for exactly this purpose.

**Acceptance criterion:**
```bash
cargo test -p anvilml-ipc --test error_tests
# -> >=5 tests, exits 0
```

#### P7-A2: anvilml-ipc: WorkerMessage enum (Rust to Python)

**Goal:** Define the Rust-to-Python half of the wire protocol — the messages the
supervisor sends to a worker.

**Files to create or modify:**
- `crates/anvilml-ipc/src/messages.rs` — `WorkerMessage`.

**Key implementation notes:**
- Variant list is fixed per `ANVILML_DESIGN.md §8.5`: `Ping`, `Shutdown`, `Execute`,
  `CancelJob`, `MemoryQuery` — no more, no fewer.
- There is **no** `InitializeHardware` message — hardware initialization happens via
  the `ANVILML_DEVICE_INDEX` environment variable injected at worker spawn, not an
  IPC round trip. This is an explicit design decision, not an oversight to "fix."

**Acceptance criterion:**
```bash
cargo test -p anvilml-ipc --test roundtrip_tests
# -> >=5 tests, exits 0
```

#### P7-A3: anvilml-ipc: WorkerEvent enum, Ready/Pong/Dying/MemoryReport

**Goal:** Define the startup-and-health half of the Python-to-Rust event enum —
the four variants that don't carry a `job_id`, kept separate from the job-lifecycle
variants to keep this task's scope (and its `context` field) a manageable size.

**Files to create or modify:**
- `crates/anvilml-ipc/src/messages.rs` — `WorkerEvent`, with only `Ready`, `Pong`,
  `Dying`, `MemoryReport`.

**Key implementation notes:**
- `Ready`'s field list is large and exact per `ANVILML_DESIGN.md §8.6` — including
  `capabilities_source: String` (`"pytorch"` or `"mock"`), which is the field that
  lets the scheduler and operator diagnostics distinguish a real torch probe from a
  mock value, per the explicit v3→v4 change table entry for this field.
- Job-lifecycle variants (`Progress`, `ImageReady`, `Completed`, `Failed`,
  `Cancelled`) are explicitly deferred to the next task.

**Acceptance criterion:**
```bash
cargo test -p anvilml-ipc --test roundtrip_tests
# -> >=9 tests total in the file, exits 0
```

#### P7-A4: anvilml-ipc: WorkerEvent job-lifecycle variants

**Goal:** Complete `WorkerEvent` with the five job-lifecycle variants, finishing
the full Python-to-Rust event vocabulary.

**Files to create or modify:**
- `crates/anvilml-ipc/src/messages.rs` — adds `Progress`, `ImageReady`,
  `Completed`, `Failed`, `Cancelled`.
- `crates/anvilml-ipc/src/lib.rs` — adds `pub use messages::{WorkerMessage,
  WorkerEvent};`.

**Key implementation notes:**
- This receives exactly the scope P7-A3 deferred — confirm P7-A3's `WorkerEvent`
  has precisely the four startup/health variants before extending it.
- `ImageReady.format` is always `"png"` in this design — there's no other format in
  scope.

**Acceptance criterion:**
```bash
cargo test -p anvilml-ipc --test roundtrip_tests
# -> >=14 tests total in the file, exits 0
```

---

### Group B — RouterTransport

#### P7-B1: anvilml-ipc: RouterTransport struct + bind()

**Goal:** Implement the ROUTER socket wrapper's construction, establishing the
split-lock field shape every later method builds on, before any send/recv logic
exists.

**Files to create or modify:**
- `crates/anvilml-ipc/src/transport.rs` — `RouterTransport` struct, `bind()`.
- `crates/anvilml-ipc/Cargo.toml` — adds `zeromq`.

**Key implementation notes:**
- **Read `ANVILML_DESIGN.md §8.3` in full before writing any code in this task** —
  it documents a previously-fixed shutdown deadlock incident, and the struct shape
  given there (`sender`/`receiver` as two independent `Arc<Mutex<...>>` fields, never
  one shared lock) is not optional styling, it's the fix.
- Resolve the `zeromq` crate's current version live via the registry, and confirm
  its split-socket API's exact shape in that version — do not assume it matches any
  API shape recalled from training data.
- Binds on `tcp://127.0.0.1:0` (OS-assigned port) — `send()`/`recv()` themselves are
  explicitly out of scope here, deferred to the next task.

**Acceptance criterion:**
```bash
cargo test -p anvilml-ipc --test roundtrip_tests
# -> >=3 tests, exits 0
```

#### P7-B2: anvilml-ipc: RouterTransport send()/recv() split-lock methods

**Goal:** Complete `RouterTransport` with the send and receive methods, each
locking only its own half — the concrete mechanism that makes the previously-fixed
shutdown deadlock structurally impossible to reintroduce.

**Files to create or modify:**
- `crates/anvilml-ipc/src/transport.rs` — adds `send()`, `recv()`.

**Key implementation notes:**
- `send()` locks **only** `self.sender`; `recv()` locks **only** `self.receiver`.
  Neither method may, under any circumstance, touch the other's lock.
- This is the exact fix for a real, documented v3 incident: a blocked `recv()` held
  a shared lock that a concurrent `Shutdown` `send()` needed, deadlocking shutdown
  entirely. Reintroducing a single combined lock around both directions — for any
  reason, including a refactor that seems simpler — is a regression of a
  previously-fixed incident, not a simplification.
- The load-bearing test case is explicit: a `recv()` blocked waiting for a message
  must not prevent a concurrent `send()` from completing.

**Acceptance criterion:**
```bash
cargo test -p anvilml-ipc --test roundtrip_tests
# -> exits 0 (includes the load-bearing concurrent send/recv regression test)
```

---

### Group C — EventBroadcaster

#### P7-C1: anvilml-ipc: EventBroadcaster tokio::sync::broadcast wrapper

**Goal:** Implement the WebSocket event fan-out wrapper that the future
`GET /v1/events` handler will subscribe to, placed in `anvilml-ipc` specifically to
avoid creating a crate dependency cycle between the worker and server crates.

**Files to create or modify:**
- `crates/anvilml-ipc/src/ws/mod.rs`, `crates/anvilml-ipc/src/ws/broadcaster.rs` —
  `EventBroadcaster`.
- `crates/anvilml-ipc/src/lib.rs` — adds `mod ws; pub use
  ws::broadcaster::EventBroadcaster;`.

**Key implementation notes:**
- Wraps `tokio::sync::broadcast::Sender<WsEvent>` with a buffer capacity of 1024,
  per `ANVILML_DESIGN.md §13.6` exactly.
- `publish()` with zero current subscribers is expected, normal behavior, not an
  error condition — `tokio::sync::broadcast`'s `SendError` in that case is ignored,
  not propagated.
- This module's location in `anvilml-ipc`, rather than `anvilml-worker` or
  `anvilml-server`, is deliberate per `ANVILML_DESIGN.md §3.2`'s dependency graph —
  do not relocate it during this or any later task.

**Acceptance criterion:**
```bash
cargo test -p anvilml-ipc --test roundtrip_tests
# -> >=4 tests, exits 0
```

---

### Group D — Closeout

#### P7-D1: anvilml-ipc: lib.rs re-export pass, 80-line check

**Goal:** Finalize `anvilml-ipc`'s public surface and confirm `lib.rs` stays within
the 80-line hard cap, closing out this phase's crate work.

**Files to create or modify:**
- `crates/anvilml-ipc/src/lib.rs` — re-exports only.

**Key implementation notes:**
- Same pattern as every prior crate's closing `lib.rs` task — no implementation
  logic, re-export and line-count verification only.

**Acceptance criterion:**
```bash
wc -l crates/anvilml-ipc/src/lib.rs
# -> <=80
cargo test -p anvilml-ipc
# -> exits 0, full crate suite
```

---

## Phase Acceptance Criteria

```bash
cargo fmt --all -- --check
cargo clippy --workspace --features mock-hardware -- -D warnings
cargo test --workspace --features mock-hardware

# Runnable Proof: not applicable — this phase implements the IPC transport and
# message-type layer in isolation, with no worker subprocess yet spawned to
# communicate with and no HTTP/WebSocket surface wired up. The 1000-round-trip
# stress test that constitutes this subsystem's real Runnable Proof is Phase 8's
# explicit gate, not this phase's. The full test suite (error_tests,
# roundtrip_tests covering messages, transport, and the broadcaster) is the
# complete and sufficient proof of this phase's deliverable, per the narrow
# exemption in FORGE_TASK_AUTHORING_SPEC.md §9.
```

---

## Known Constraints and Gotchas

- `RouterTransport`'s send/recv split-lock shape (P7-B1, P7-B2) is specified
  exactly in `ANVILML_DESIGN.md §8.3` and must not be reinterpreted, simplified, or
  "improved" — every prior attempt to do so produced the exact deadlock this shape
  exists to prevent.
- `EventBroadcaster` belongs in `anvilml-ipc`, never in `anvilml-worker` or
  `anvilml-server` — this placement avoids a crate dependency cycle per the
  project's dependency graph, and is not a matter of "where it seems to fit best."
- `WorkerEvent`'s nine variants are deliberately split across two tasks (P7-A3,
  P7-A4) for the same reason `WsEvent` was split in Phase 3 — keeping each task's
  `context` field under the authoring spec's 1000-character cap while keeping each
  task focused on one coherent sub-concern.
- This phase does not implement or run the 1000-round-trip stress test —
  that is Phase 8's explicit, named gate. Do not consider this phase's IPC layer
  "proven" beyond what its own unit/roundtrip tests demonstrate.

---

## docs/RUNNABLE_PROOF.md entry

```markdown
## Phase 7 — IPC Foundations

**Capability proved:** Not applicable — this phase implements the IPC message
types and `RouterTransport`/`EventBroadcaster` wrappers in isolation, with no
worker subprocess yet spawned to communicate with. The 1000-round-trip stress test
that proves this subsystem end-to-end is Phase 8's explicit gate. See
`TASKS_PHASE007.md`'s Phase Acceptance Criteria for this phase's own test-suite
proof.
```
