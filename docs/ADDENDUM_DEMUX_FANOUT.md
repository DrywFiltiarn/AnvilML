# Addendum: `Demux` fan-out subscription (`subscribe()`/`unsubscribe()`)

**Status:** Resolved ahead of `P16-B1`, before that task's own implementation
session. Recorded here following the same convention as
`docs/ADDENDUM_ARTIFACT_NOT_FOUND.md` and
`docs/ADDENDUM_ENUMERATION_SOURCE_CPU.md` — an addendum plus the corresponding
`tasks/tasks_phase016.json` / `docs/TASKS_PHASE016.md` update, rather than a
silent edit, so the gap and its resolution are both part of the permanent
record.

---

## Background

`P16-A1` (already implemented and committed) gave `anvilml-scheduler`'s
`spawn_event_loop()` its own `Arc<RouterTransport>` parameter and had it call
`transport.recv()` directly in a loop, per that task's own text: *"looping
`transport.recv()`, mapping each `WorkerEvent` to its `WsEvent`..."*.

This was never exercised against the real pool topology — every test in
`event_loop_tests.rs` binds its own fresh, isolated `RouterTransport` with no
`ManagedWorker`/bridge on the other end. But `anvilml-worker/src/bridge.rs`
already establishes, in its own extensive doc comment, that its `reader_task`
is *"the SOLE production caller of `transport.recv()`"* on the pool-wide
ROUTER socket — a second concurrent caller races it for every incoming frame,
nondeterministically stealing messages regardless of which task "should" get
them. `WorkerPool::transport()` is public and returns that exact shared `Arc`,
and `P16-B1`'s own task text says to wire `spawn_event_loop()` from
`main.rs` — the only transport that exists there is the pool's. Passing it
straight into `spawn_event_loop()`, as `P16-A1`'s signature requires, would
have raced `ManagedWorker`'s own demux-based consumption for every
`Ready`/`Progress`/`Completed`/`Failed`/`Cancelled`/`Pong`/`Dying` event —
silently breaking keepalive, node-registry updates, worker lifecycle
tracking, WebSocket broadcast, and the job-status/VRAM/dispatch-wake logic
`P16-A2`/`P16-A3` just implemented, all at once, with no compile-time signal.

`Demux` (`anvilml-worker/src/demux.rs`), the only existing fan-out point for
inbound `WorkerEvent`s, could not have absorbed a second consumer either:
`register()`/`route()` is a strict 1:1 map from `worker_id` to a single
`Sender<WorkerEvent>` — registering a second consumer for the same
`worker_id` silently replaces (steals from) the first, it does not add a
second recipient.

## Resolution

`Demux` gains an independent, pool-wide fan-out mechanism, additive to the
existing per-worker `register()`/`deregister()`/`route()` contract (§9.4),
which is completely unchanged:

```diff
+pub type SubscriptionId = u64;
+
 pub struct Demux {
     inner: Mutex<HashMap<String, Sender<WorkerEvent>>>,
+    subscribers: Mutex<HashMap<SubscriptionId, Sender<(String, WorkerEvent)>>>,
+    next_subscription_id: AtomicU64,
 }

 impl Demux {
+    pub fn subscribe(&self) -> (SubscriptionId, Receiver<(String, WorkerEvent)>);
+    pub fn unsubscribe(&self, id: SubscriptionId);
     pub fn register(&self, worker_id: String, tx: Sender<WorkerEvent>);
     pub fn deregister(&self, worker_id: &str) -> bool;
     pub async fn route(&self, worker_id: &str, event: WorkerEvent) -> Result<(), AnvilError>;
 }
```

`route()`'s existing primary-delivery behavior (error semantics, return
value) is untouched. It additionally best-effort fans the event out to every
active subscriber, tagged with the source `worker_id` since subscribers are
not worker-scoped. A full or closed subscriber channel is skipped with a
`WARN` log rather than blocking or erroring — a slow or dead subscriber must
never be able to stall delivery to the worker's own primary (`register()`ed)
consumer.

`WorkerPool` gains a `demux()` accessor (mirroring its existing
`transport()`/`bridge_sender()` accessors) so callers outside
`anvilml-worker` can subscribe without reaching into pool internals.

`anvilml-scheduler`'s `spawn_event_loop()` is retrofitted: its second
parameter changes from `transport: Arc<RouterTransport>` to
`demux: Arc<Demux>`, and its loop body changes from `transport.recv()` to
consuming the `mpsc::Receiver` returned by `demux.subscribe()`. No other
task's committed behavior changes — the `WorkerEvent`→`WsEvent` mapping
(`P16-A1`), terminal-status persistence and VRAM release (`P16-A2`), and
worker-idle-restore/dispatch-wake (`P16-A3`) are all identical; only the
event source changes.

## Where this is reflected in this delivery

- **`docs/ANVILML_DESIGN.md` §9.8** (new): documents the fan-out API and the
  "must consume via `Demux`, never `RouterTransport::recv()` directly" rule.
- **`docs/ANVILML_DESIGN.md` §12.1**: `event_loop.rs`'s module-layout comment
  updated to point at `Demux::subscribe()` instead of the transport broadcast.
- **`tasks/tasks_phase016.json` / `docs/TASKS_PHASE016.md`**: new task
  `P16-A4` retrofits `spawn_event_loop()` and its tests onto `Demux`.
  `P16-B1`'s context is updated to depend on `P16-A4` (not `P16-A3` directly)
  and to pass `workers.demux()` rather than `workers.transport()`.

## Action required by the repository maintainer

None beyond applying this patch — unlike the two prior addenda, this one
ships with the actual code and doc changes already applied to the live
repository files, not just recorded for a future hand-edit.
