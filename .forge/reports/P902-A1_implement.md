# Implementation Report: P902-A1

| Field         | Value                                                                              |
|---------------|-------------------------------------------------------------------------------------|
| Task ID       | P902-A1                                                                              |
| Phase         | 902 — Keepalive Ready-Gate, Prompt Shutdown, and Demux Per-Message Error Handling Retrofit |
| Description   | managed.rs/keepalive.rs: gate keepalive pings on Ready; abort (not drop) keepalive_handle on every Dead-bound exit path |
| Implemented   | 2026-06-18T08:31:00Z                                                                |
| Status        | COMPLETE                                                                             |

## Summary

`keepalive::start()` previously began sending Ping messages the instant its task was
spawned inside `ManagedWorker::spawn()`, before the worker had ever reported `Ready` —
confirmed directly from a `cargo run --bin anvilml --features mock-hardware` trace
showing a `sending ping seq=1` log line at the same timestamp as `worker spawned`, several
hundred milliseconds before the matching `Ready` event was processed. Fixed by threading a
`oneshot::Receiver<()>` into `keepalive::start()`, awaited once before the ping loop
begins; `ManagedWorker` gained a `ready_tx: Option<oneshot::Sender<()>>` field, fired via
`.take()` in `run()`'s `Initializing → Idle` transition arm. While designing this, a
related defect was found and closed in the same task: `keepalive_handle` was only ever
*dropped*, never *aborted*, on the ready-timeout and child-exit exit paths — a dropped
`JoinHandle` detaches interest in the task's result without stopping the task, so the
keepalive task could keep running (and, once past the new Ready gate, keep pinging)
against a worker already confirmed `Dead`. All three `Dead`-bound exit paths (ready
timeout, child exit, graceful shutdown) now call `keepalive_handle.take().abort()`.

## Resolved Dependencies

None. No new external crates, packages, or feature flags introduced — `tokio::sync::oneshot`
is already part of the `full` feature set enabled workspace-wide.

## Files Changed

| Action | Path | Description |
|--------|------|--------------|
| MODIFY | `crates/anvilml-worker/src/keepalive.rs` | `start()` gained `ready_rx: tokio::sync::oneshot::Receiver<()>` parameter, awaited once before the ping loop; module doc comment updated |
| MODIFY | `crates/anvilml-worker/src/managed.rs` | New `ready_tx: Option<oneshot::Sender<()>>` field; `new()` gained trailing parameter; `spawn()` constructs the channel and threads `ready_rx` into `keepalive::start()`; `run()`'s `Ready` arm fires `ready_tx`; ready-timeout and child-exit arms changed from dropping to `.abort()`-ing `keepalive_handle`; shutdown arm's existing drop changed to `.abort()` for uniformity across all three exit paths; doc comments updated throughout |
| MODIFY | `crates/anvilml-worker/tests/keepalive_tests.rs` | Three existing tests updated to fire `ready_tx` immediately (preserving original timing semantics); two new tests added: `test_no_ping_before_ready`, `test_dropped_ready_tx_skips_heartbeat_entirely` |
| MODIFY | `crates/anvilml-worker/tests/managed_tests.rs` | Five `ManagedWorker::new()` call sites updated for the new trailing parameter; one new end-to-end test added: `test_run_ready_event_releases_keepalive_gate` |
| MODIFY | `crates/anvilml-worker/tests/pool_tests.rs` | One `ManagedWorker::new()` call site updated for the new trailing parameter (caught by a full-workspace `cargo test` run after the initial fix, not by the original file sweep) |
| MODIFY | `crates/anvilml-server/tests/workers_tests.rs` | One `ManagedWorker::new()` call site updated for the new trailing parameter (same workspace-wide sweep) |

## Commit Log

```
Not available in this session — changes were applied directly to repository files
outside the normal Forge git-staging flow. No `git add -A` was run; this report
documents the change set for task-graph reconciliation purposes. The user applied
these files locally and confirmed `cargo fmt`, `cargo clippy`, and
`cargo test --workspace` all green after applying them.
```

## Test Results

```
User-confirmed, full workspace, after this task's files were applied locally:

cargo fmt --check
  (clean, no output)

cargo clippy --workspace --all-targets --features mock-hardware -- -D warnings
  (clean, no output)

cargo test --workspace --features mock-hardware
  (all suites green, including the new keepalive_tests.rs and managed_tests.rs tests
  listed above)

Live verification via cargo run --bin anvilml --features mock-hardware:
  trace shows "ready signal received, starting heartbeat" followed immediately by
  "sending ping worker_id=worker-0 seq=1" only AFTER "worker reached Ready" for the
  same worker_id — no ping logged before that point for either worker-0 or worker-1.
```

## Format Gate

Clean — `cargo fmt --check` exits 0 (user-confirmed, this session).

## Platform Cross-Check

Not performed in this session — work was done and verified on the user's Windows
development machine only (`E:\AnvilML>` prompt visible in verification logs). No Linux
cross-check was run for this specific change set.

## Project Gates

`cargo clippy --workspace --all-targets --features mock-hardware -- -D warnings` — clean
(user-confirmed). No other project-specific gates were exercised in this session.

## Public API Delta

```
New pub items introduced:
- anvilml_worker::keepalive::start() — signature changed (added ready_rx parameter);
  not a new item, but a breaking signature change to an existing pub fn
- ManagedWorker::new() — signature changed (added ready_tx trailing parameter); same
  category as above
- ManagedWorker.ready_tx field — private, not pub

No new pub types, traits, or modules were introduced.
```

## Deviations from Plan

There is no prior approved plan for this task — it was performed as direct, manual,
human-directed work against the repository rather than through a PLAN/ACT Forge session.
This report is written retroactively to bring the task graph into agreement with the
actual state of the repository. The "Resolved Dependencies," "Files Changed," and "Public
API Delta" sections above reflect what was actually done, not a plan that was followed.

## Blockers

None.
