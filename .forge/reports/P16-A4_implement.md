# Implementation Report: P16-A4

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P16-A4                          |
| Phase         | 16 — Live Events                |
| Description   | anvilml-scheduler + anvilml-worker: retrofit spawn_event_loop() onto Demux::subscribe() |
| Implemented   | 2026-07-09T13:40:00Z            |
| Status        | COMPLETE                          |

## Summary

Closed an audit-found gap in `P16-A1`'s own committed implementation:
`spawn_event_loop()` called `RouterTransport::recv()` directly, which races
`bridge.rs`'s `reader_task` — already documented as the sole permitted caller
of `recv()` on the pool's shared ROUTER socket — for every incoming frame.
Extended `Demux` with additive `subscribe()`/`unsubscribe()` fan-out
(existing `register()`/`deregister()`/`route()` contract unchanged), added
`WorkerPool::demux()`, and retargeted `spawn_event_loop()` and all of
`event_loop_tests.rs` onto the new subscription mechanism. See
`docs/ADDENDUM_DEMUX_FANOUT.md` for full background.

## Resolved Dependencies

No new external crates. Uses only existing types already in scope:
`tokio::sync::mpsc` (already a dependency of both `anvilml-worker` and
`anvilml-scheduler`), `std::sync::atomic::AtomicU64`.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | crates/anvilml-worker/src/demux.rs | Added `SubscriptionId`, `subscribe()`, `unsubscribe()`, `fan_out_to_subscribers()`; `route()` now calls fan-out before primary delivery |
| Modify | crates/anvilml-worker/src/pool.rs | Added `WorkerPool::demux()` accessor |
| Modify | crates/anvilml-worker/src/lib.rs | Re-exports `SubscriptionId` alongside `Demux` |
| Modify | crates/anvilml-worker/tests/demux_tests.rs | Added 5 new tests for subscribe/unsubscribe/fan-out/backpressure |
| Modify | crates/anvilml-worker/Cargo.toml | Bumped version 0.1.32 → 0.1.33 |
| Modify | crates/anvilml-scheduler/src/event_loop.rs | `spawn_event_loop()` second parameter changed from `transport: Arc<RouterTransport>` to `demux: Arc<Demux>`; loop body now consumes `demux.subscribe()`'s receiver instead of calling `transport.recv()` |
| Modify | crates/anvilml-scheduler/tests/event_loop_tests.rs | All 22 tests retargeted from `RouterTransport` + real DEALER socket sends to `Demux` + `demux.route()`, matching production's `bridge.rs` reader_task path |
| Modify | crates/anvilml-scheduler/Cargo.toml | Bumped version 0.1.23 → 0.1.24 |
| Modify | Cargo.lock | Version bumps for both crates |
| Modify | docs/ANVILML_DESIGN.md | New §9.8 (Demux Fan-Out Subscription); §12.1 module-layout comment for `event_loop.rs` updated |
| Create | docs/ADDENDUM_DEMUX_FANOUT.md | Full background, resolution, and cross-references, per this project's existing addendum convention |
| Modify | docs/TASKS_PHASE016.md | Inserted `P16-A4` task; updated `P16-B1` to depend on it and pass `workers.demux()`; updated Group Reference table, Interfaces and Contracts table, and the stale "Interim-patch removal checklist" gotcha (confirmed already resolved) |
| Modify | .forge/tasks/tasks_phase016.json | Inserted `P16-A4` task entry; `P16-B1`'s `prereqs` changed to `["P16-A4"]` |
| Modify | docs/TESTS.md | Retargeted `event_loop_tests.rs` entries' DEALER/RouterTransport wording onto Demux; added 5 new entries for the demux fan-out tests |

## Test Results

Sandbox has no Rust toolchain available (no `cargo`/`rustc` on `PATH`, and none
found elsewhere on the filesystem) — every prior report in this delivery was
produced against a real build/test environment that this session does not
have. **The commands below were not executed in this session; they must be
run locally before this patch is merged, per this project's own workflow
(`agents/forge-act.md`: Dryw applies patches locally).**

```bash
cargo test -p anvilml-worker --test demux_tests
# expected: >=10 tests total (5 pre-existing + 5 new), exits 0

cargo test -p anvilml-scheduler --test event_loop_tests
# expected: >=20 tests total (unchanged count, retargeted onto Demux), exits 0

cargo build -p anvilml
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

## Format Gate

Not run — no local Rust toolchain in this session. `cargo fmt --all -- --check`
should be run before merge; the diff was hand-formatted to match the existing
file's style (4-space indent, trailing commas, doc-comment wrapping) but was
not verified against `rustfmt`'s actual output.

## Platform Cross-Check

Not run — no local Rust toolchain in this session.

## Project Gates

Not run — no local Rust toolchain in this session.

## Public API Delta

```
+ pub type SubscriptionId = u64;  (anvilml_worker)
+ impl Demux {
+     pub fn subscribe(&self) -> (SubscriptionId, Receiver<(String, WorkerEvent)>)
+     pub fn unsubscribe(&self, id: SubscriptionId)
+ }
+ impl WorkerPool {
+     pub fn demux(&self) -> &Arc<Demux>
+ }
```

`spawn_event_loop()`'s signature changed (second parameter's type changed from
`Arc<RouterTransport>` to `Arc<Demux>`) — a breaking change to that function's
own signature, but it has exactly one real call site planned (`P16-B1`, not
yet implemented) and its only current callers are this crate's own tests,
which are updated in this same patch.

## Deviations from Plan

- No prior plan existed for this task — it was inserted into the phase ahead
  of `P16-B1` as an audit finding, not planned in advance. See
  `docs/ADDENDUM_DEMUX_FANOUT.md` for the full background in place of a
  separate `P16-A4_plan.md`.
- Fan-out fairness/ordering across subscribers is not guaranteed beyond
  "each active subscriber gets a best-effort attempt per `route()` call, in
  the iteration order of the subscribers map at that moment" — no ordering
  guarantee was requested or needed for the one real subscriber
  (`spawn_event_loop()`) this task exists to support, and `HashMap` iteration
  order is unspecified regardless.
- Did not add a `subscriber_count()`-style introspection method — nothing in
  this task or `P16-B1`'s stated scope needs one, and speculative additions
  are against this project's own stated convention (`P15-A1`'s "no
  speculative fields" precedent, applied here to methods).

## Blockers

None for this task's own scope. Flagging for `P16-B1`'s implementer: pass
`workers.demux()`, not `workers.transport()`, into `spawn_event_loop()` — see
this report's own `docs/TASKS_PHASE016.md` update for the added note.
