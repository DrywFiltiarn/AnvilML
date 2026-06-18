# Tasks: Phase 902 — Keepalive Ready-Gate, Prompt Shutdown, and Demux Per-Message Error Handling Retrofit

| Field | Value |
|-------|-------|
| Phase | 902 |
| Name | Keepalive Ready-Gate, Prompt Shutdown, and Demux Per-Message Error Handling Retrofit |
| Project | anvilml |
| Status | Approved — all three tasks already implemented and verified; this document is a retroactive record |
| Depends on phases | 10 (partially — after P10-A2, before P10-A3 resumes) |

## Overview

Phase 902 is a three-task retrofit, in the same spirit as Phase 901, except all three
tasks were already implemented, tested, and verified directly against the repository in
a manual working session that ran in parallel with — and after — `P10-A2`'s completion.
Unlike Phase 901, there is no remaining implementation work in this phase: every task
below is recorded as `COMPLETE` from the moment this phase is authored. The purpose of
this document is solely to give the task graph and any future agent session an accurate
record of what changed, why, and in what order, so that `P10-A3` (and anything after it)
is planned against the codebase as it actually exists rather than against the snapshot
`P10-A2`'s own plan and implementation reports describe.

**Why this phase exists.** `P10-A2`'s plan report's "Existing Codebase Assessment"
describes `managed.rs` as having exactly two `select!` arms (ready-timeout,
`event_rx.recv()`) and no `device_index` field. `P10-A2`'s implementation report
confirms it added exactly `device_index` and a third `child.wait()` arm on top of that
two-arm baseline. Three further rounds of manual work then happened directly against the
repository, on top of `P10-A2`'s output, fixing real defects surfaced by running the
actual binary (`cargo run --bin anvilml --features mock-hardware`) and reading its log
output — not by following a written plan. None of that work is reflected in `P10-A2`'s
reports, `state.json`, or any task file, which is the gap this phase closes. Without it,
`P10-A3`'s own plan step would need to re-derive the current shape of `managed.rs` and
`keepalive.rs` from scratch with no record of why they look the way they do, and could
plausibly "fix" something this phase deliberately changed, or call `ManagedWorker::spawn()`
with the wrong arity (see `P902-A1`'s note on `spawn()`'s signature, and the correction
made to `P10-A3`'s own `context` field alongside this phase).

**P902-A1** closes the heartbeat-on-Ready gap: `keepalive::start()`'s ping loop began
sending pings immediately on task spawn, before the worker had ever reported `Ready` —
confirmed directly from a `cargo run` trace showing `sending ping seq=1` at the same
timestamp as `worker spawned`, several hundred milliseconds before the corresponding
`Ready` event was even received. The fix threads a `oneshot::Receiver<()>` into
`keepalive::start()`, awaited once before the ping loop begins, fired from
`ManagedWorker::run()`'s `Initializing → Idle` transition arm via a new
`ready_tx: Option<oneshot::Sender<()>>` field. While designing the fix, a related gap was
found and closed in the same task: the keepalive task was only ever *dropped*, never
*aborted*, on the `Dead`-bound exit paths (ready-timeout, child-exit, and the existing
graceful shutdown arm) — dropping a `JoinHandle` detaches Rust's interest in the task's
result without actually stopping the task, so a keepalive task could keep running (and,
if it had already passed the new Ready gate, keep pinging) against a worker that was
already confirmed dead. All three exit paths now call `keepalive_handle.take().abort()`.

**P902-A2** closes a second defect that the `cargo run` log surfaced only once `P902-A1`'s
gate was in place: every graceful shutdown produced a 2-second `WARN: bridge writer task
did not finish within grace period`. Root cause: `HeartbeatHandle::shutdown()` only set a
boolean flag checked at the *top* of the keepalive loop's outer iteration — it had no way
to interrupt the loop's bottom-of-iteration `tokio::time::sleep(ping_interval)` (up to 30s
in production) if shutdown was requested while the task was already asleep there. The
supervisor's shutdown arm dropped its own `msg_tx` clone immediately, but the keepalive
task's separate clone of the same sender stayed alive for the rest of that sleep, so the
bridge writer's `while let Some(msg) = msg_rx.recv().await` had nothing to make it exit
until the sleep finally elapsed on its own. The fix replaces the `Mutex<bool>` flag with a
`tokio::sync::watch::channel(false)`; both of the keepalive loop's wait points (the
`ping_interval` sleep and the `pong_timeout` wait) now race against a small
`wait_for_shutdown(&mut rx)` helper inside a `select!`. The helper checks `*rx.borrow()`
before falling back to `rx.changed().await`, which is what makes it safe to call at two
separate, sequential wait points without one consuming a wakeup the other needs — the
failure mode a single-permit `tokio::sync::Notify` would have had, and was caught and
discarded mid-implementation in favour of `watch` for exactly that reason.
`ManagedWorker`'s shutdown arm additionally moved `keepalive_handle.take().abort()` to
immediately after the graceful signal — before the `Shutdown` IPC message and writer-wait
sequence — as a second, unconditional guarantee independent of the keepalive task's own
scheduling. Confirmed via a second `cargo run` trace: shutdown-to-fully-torn-down dropped
from roughly 4 seconds (dominated by two sequential 2-second WARN timeouts) to under one
millisecond per worker.

**P902-A3** is unrelated to the keepalive work above — it closes a pre-existing,
previously-deferred defect documented in a carried-forward known-issue note, found while
reviewing outstanding work rather than introduced by `P902-A1`/`P902-A2`.
`RouterTransport::recv()` in `anvilml-ipc` collapsed four structurally distinct failure
modes — a genuine ZeroMQ socket failure, a missing identity frame, a missing payload
frame, and a `decode_event` failure — into one flat `AnvilError::Ipc(String)`.
`demux::start()`'s recv loop, the only demux task for the entire process, treated every
one of those as fatal and `break`-stopped on any of them, meaning a single malformed or
undecodable message from any one peer (including a worker that briefly desyncs, or any
other peer that connects and sends garbage) silently killed event delivery for every
worker in the pool simultaneously. The note's own draft options both turned out to be
broader than necessary once the actual code was read closely: `AnvilError` itself did not
need new variants (nothing matches on it by name outside `recv()`'s own error
construction), and `demux.rs` did not need to fragile-parse error strings. The actual fix
is narrower: a new `RecvError` enum in `anvilml-ipc::error`, returned by `recv()` and
`recv_with_raw_identity()` in place of `AnvilError`, with `impl From<RecvError> for
AnvilError` added so the one existing caller that propagates via `?` into an
`AnvilError`-returning function (`anvilml-ipc/tests/stress_test.rs`) keeps compiling
unchanged. `demux::start()`'s loop now matches on the concrete variant: `break` only on
`RecvError::SocketClosed`, log-and-continue on the other three. A new regression test,
`test_demux_survives_undecodable_payload`, sends genuinely invalid msgpack from one DEALER
and proves the demux task is still alive and correctly dispatching a second, real,
registered worker's event afterward.

All three tasks are confined to `anvilml-worker` and `anvilml-ipc`. None change any
HTTP-visible behaviour or any public type's shape outside the two crates named. `P10-A3`'s
respawn cycle is the next task in the primary phase sequence and must be planned against
the `managed.rs`/`keepalive.rs` shape this phase leaves behind, not against `P10-A2`'s
own "Existing Codebase Assessment" snapshot.

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | anvilml-worker, anvilml-ipc | P902-A1 … P902-A3 | Ready-gated keepalive + abort-on-Dead; prompt watch-based shutdown; per-message demux error handling |

## Prerequisites

P10-A2 complete (per its own implementation report, though see this phase's Overview for
why that report's "Existing Codebase Assessment" no longer matches the live file).
`crates/anvilml-worker/src/managed.rs` exists with `device_index` and the `child.wait()`
arm from `P10-A2`. `crates/anvilml-worker/src/keepalive.rs` exists with the pre-gate
ping loop and `Mutex<bool>`-based `HeartbeatHandle`. `crates/anvilml-ipc/src/transport.rs`
exists with `RouterTransport::recv()` returning flat `AnvilError::Ipc`.

## Task Descriptions

### Group A — anvilml-worker, anvilml-ipc

#### P902-A1: anvilml-worker: managed.rs/keepalive.rs ready-gated keepalive plus abort-on-Dead

**Status:** COMPLETE. See `.forge/reports/P902-A1_implement.md`.

**Goal:** Stop the keepalive task from sending its first ping before the worker has
reported `Ready`, and ensure the keepalive task is genuinely stopped (not merely
detached) on every path that puts the worker into `Dead`.

**Files modified:**
- `crates/anvilml-worker/src/keepalive.rs` — `start()` gained a `ready_rx` parameter
- `crates/anvilml-worker/src/managed.rs` — `ready_tx` field; fired in the `Ready` arm;
  `keepalive_handle.take().abort()` added to the ready-timeout and child-exit arms
- `crates/anvilml-worker/tests/keepalive_tests.rs`, `tests/managed_tests.rs` — updated
  call sites; new tests proving the gate withholds and releases correctly

**Key implementation notes:**
- `ManagedWorker::spawn()`'s signature is unaffected by this task — it already took
  `routes: crate::demux::RouteTable` as of work prior to this phase; `P10-A3`'s respawn
  cycle must call `spawn()` with that same five-argument shape (`cfg, device, transport,
  worker_id, routes`), not the three-argument shape described in `P10-A3`'s original
  `context` field, which has been corrected alongside this phase.
- A worker built via `ManagedWorker::new()` for tests passes `None` for `ready_tx` unless
  the test specifically needs to exercise a real keepalive task, in which case it
  constructs and fires the `oneshot` pair itself before calling `new()`.

---

#### P902-A2: anvilml-worker: keepalive.rs watch-based prompt shutdown

**Status:** COMPLETE. See `.forge/reports/P902-A2_implement.md`.

**Goal:** Make `HeartbeatHandle::shutdown()` actually take effect promptly at either of
the keepalive loop's two wait points, not only at a cycle boundary — closing the gap that
caused the 2-second bridge-writer-grace-period `WARN` observed on every shutdown.

**Files modified:**
- `crates/anvilml-worker/src/keepalive.rs` — `Mutex<bool>` replaced with
  `tokio::sync::watch::channel(false)`; new `wait_for_shutdown()` helper; both wait points
  now `select!` against it
- `crates/anvilml-worker/src/managed.rs` — shutdown arm reordered: `keepalive_handle.take().abort()`
  moved to immediately after `HeartbeatHandle::shutdown()`, before the `Shutdown` message
  and bridge-writer wait

**Key implementation notes:**
- `wait_for_shutdown()` checks `*rx.borrow()` before `rx.changed().await` specifically so
  it can be called at two separate, sequential `select!` sites on the same receiver
  without the first call consuming a wakeup the second site still needs — this is why
  `watch` was used instead of `tokio::sync::Notify`, which was tried first and discarded
  for exactly this reason during implementation.
- This task does not change `HeartbeatHandle::shutdown()`'s public signature
  (`pub async fn shutdown(&self)`); only its internal mechanism changed, so no caller
  outside `keepalive.rs`/`managed.rs` needed updating.

---

#### P902-A3: anvilml-ipc/anvilml-worker: RecvError and per-message demux error handling

**Status:** COMPLETE. See `.forge/reports/P902-A3_implement.md`.

**Goal:** Stop a single malformed or undecodable message from one peer from silently
killing event delivery for every worker in the pool, by giving `RouterTransport::recv()`
a structured error type the demux loop can actually discriminate on.

**Files modified:**
- `crates/anvilml-ipc/src/error.rs` — new `RecvError` enum; `impl From<RecvError> for AnvilError`
- `crates/anvilml-ipc/src/transport.rs` — `recv()`/`recv_with_raw_identity()` return `RecvError`
- `crates/anvilml-ipc/src/lib.rs` — `RecvError` added to the crate's public re-exports
- `crates/anvilml-worker/src/demux.rs` — loop matches on `RecvError`'s concrete variants
- `crates/anvilml-worker/tests/demux_tests.rs` — new `test_demux_survives_undecodable_payload`;
  stale comment in `test_demux_drops_event_for_unregistered_identity` corrected

**Key implementation notes:**
- `AnvilError` itself was not modified — it gained no new variants. `RecvError` is purely
  internal to `anvilml-ipc`'s `recv()` family; the `From` impl exists only so the one
  existing `?`-based caller outside `anvilml-ipc` and `anvilml-worker`
  (`anvilml-ipc/tests/stress_test.rs`, which propagates into a function pinned to
  `Result<(), AnvilError>` for its own unrelated reasons) keeps compiling unchanged.
- A full-workspace grep for every caller of `RouterTransport::recv()` and
  `recv_with_raw_identity()` was performed before this change landed, to confirm no other
  call site matches on the error type by name. None do.

## Runnable Proof

```
cargo fmt --check          # exits 0
cargo clippy --workspace --all-targets --features mock-hardware -- -D warnings   # exits 0
cargo test --workspace --features mock-hardware                                  # exits 0
cargo run --bin anvilml --features mock-hardware
# trace shows: no "sending ping" before the corresponding "worker reached Ready";
# Ctrl-C shutdown shows "writer task ended (channel closed)" within ~1ms of the
# "message sent to worker ... Shutdown" log line, no grace-period WARN
```
