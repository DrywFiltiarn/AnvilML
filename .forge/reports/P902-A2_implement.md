# Implementation Report: P902-A2

| Field         | Value                                                                              |
|---------------|-------------------------------------------------------------------------------------|
| Task ID       | P902-A2                                                                              |
| Phase         | 902 — Keepalive Ready-Gate, Prompt Shutdown, and Demux Per-Message Error Handling Retrofit |
| Description   | keepalive.rs: replace Mutex<bool> shutdown flag with tokio::sync::watch so HeartbeatHandle::shutdown() wakes the loop promptly at both wait points |
| Implemented   | 2026-06-18T09:10:00Z                                                                |
| Status        | COMPLETE                                                                             |

## Summary

A `cargo run` trace taken after `P902-A1` landed showed every graceful shutdown producing
a 2-second `WARN: bridge writer task did not finish within grace period during shutdown`
per worker. Root cause: `HeartbeatHandle::shutdown()` only set a `Mutex<bool>` flag checked
at the *top* of the keepalive loop's outer iteration, with no way to interrupt the loop's
bottom-of-iteration `tokio::time::sleep(ping_interval)` (up to 30 seconds in production) if
shutdown was requested while the task was already inside that sleep — which, per the
trace, it reliably was (21 seconds into a 30-second sleep at the moment shutdown fired).
`ManagedWorker::run()`'s shutdown arm dropped its own `msg_tx` clone immediately, but the
keepalive task's separate clone of the same `mpsc::Sender` stayed alive for the remainder
of that sleep, so the bridge writer's `while let Some(msg) = msg_rx.recv().await` had no
way to observe a closed channel and exit until the sleep elapsed on its own. Fixed by
replacing the `Mutex<bool>` flag with a `tokio::sync::watch::channel(false)`; both of the
keepalive loop's wait points (the `ping_interval` sleep and the `pong_timeout` wait) now
race against a `wait_for_shutdown(&mut rx)` helper inside `select!`. A `tokio::sync::Notify`
based approach was attempted first and discarded mid-implementation: `Notify`'s single
stored permit would be consumed by whichever of the two sequential wait sites reached it
first, leaving the other to wait out its full duration regardless — `watch`'s
"check-the-current-value" semantics (`*rx.borrow()` before falling back to
`rx.changed().await`) do not have that problem, since checking a value is idempotent
across any number of separate call sites. `ManagedWorker`'s shutdown arm additionally moved
`keepalive_handle.take().abort()` to immediately after the graceful `HeartbeatHandle::shutdown()`
call, before the `Shutdown` IPC message and bridge-writer wait, as a second, unconditional
guarantee independent of the keepalive task's own scheduling.

## Resolved Dependencies

None. `tokio::sync::watch` is already part of the `full` feature set enabled workspace-wide;
no new crates or feature flags were introduced.

## Files Changed

| Action | Path | Description |
|--------|------|--------------|
| MODIFY | `crates/anvilml-worker/src/keepalive.rs` | `HeartbeatHandle`'s internal field changed from `Arc<Mutex<bool>>` (briefly, `Arc<Notify>` during an abandoned intermediate attempt) to `watch::Sender<bool>`; new private `wait_for_shutdown()` helper; both of the loop's wait points changed from a bare `sleep`/inner-`select!` to a `select!` that also races `wait_for_shutdown`; module doc comment updated |
| MODIFY | `crates/anvilml-worker/src/managed.rs` | Shutdown arm reordered: `keepalive_handle.take().abort()` moved from the bottom of the arm to immediately after `HeartbeatHandle::shutdown()`, before the `Shutdown` message send and bridge-writer wait; `run()`'s doc comment's numbered shutdown-sequence list renumbered to match; end-of-loop cleanup comment corrected to describe which of the four exit paths already pre-empty `keepalive_handle` before reaching the final unconditional drops |

## Commit Log

```
Not available in this session — changes were applied directly to repository files
outside the normal Forge git-staging flow. No `git add -A` was run; this report
documents the change set for task-graph reconciliation purposes. The user applied
these files locally and confirmed `cargo fmt`, `cargo clippy`, and
`cargo test --workspace` all green after applying them, and additionally provided a
fresh `cargo run` trace confirming the WARN no longer appears.
```

## Test Results

```
User-confirmed, full workspace, after this task's files were applied locally:

cargo fmt --check
  (clean, no output)

cargo clippy --workspace --all-targets --features mock-hardware -- -D warnings
  (clean, no output)

cargo test --workspace --features mock-hardware
  (all suites green; no test file required changes for this task, since
  keepalive_tests.rs never exercises HeartbeatHandle::shutdown() directly, and
  test_shutdown_cleans_up_handles / test_run_shutdown_deregisters_route in
  managed_tests.rs are purely observational about run()'s overall completion,
  not about internal step ordering)

Live verification via cargo run --bin anvilml --features mock-hardware, Ctrl-C shutdown:

Before this task (from a trace taken immediately after P902-A1):
  T+0.000  shutdown requested, beginning teardown  worker_id=worker-0
  T+0.001  message sent to worker  worker_id=worker-0  msg_type=Shutdown
  T+2.009  WARN bridge writer task did not finish within grace period during shutdown
  (same ~2s WARN repeated for worker-1; total teardown ~4s for two workers)

After this task:
  T+0.000  shutdown requested, beginning teardown  worker_id=worker-0
  T+0.001  message sent to worker  worker_id=worker-0  msg_type=Shutdown
  T+0.0003 writer task ended (channel closed)  worker_id=worker-0
  (same sub-millisecond pattern for worker-1; no WARN; total teardown <0.1s)
```

## Format Gate

Clean — `cargo fmt --check` exits 0 (user-confirmed, this session).

## Platform Cross-Check

Not performed in this session — work was done and verified on the user's Windows
development machine only. No Linux cross-check was run for this specific change set.

## Project Gates

`cargo clippy --workspace --all-targets --features mock-hardware -- -D warnings` — clean
(user-confirmed). No other project-specific gates were exercised in this session.

## Public API Delta

```
HeartbeatHandle's public method signature (pub async fn shutdown(&self)) is UNCHANGED —
only its internal field and mechanism changed. No caller outside keepalive.rs/managed.rs
needed updating, and no new pub items were introduced.
```

## Deviations from Plan

There is no prior approved plan for this task — it was performed as direct, manual,
human-directed work against the repository rather than through a PLAN/ACT Forge session.
This report is written retroactively. One deviation worth recording explicitly: the first
implementation attempt used `tokio::sync::Notify` rather than `watch`, and was caught and
discarded before being applied to the repository, once the single-permit consumption
problem across two sequential wait sites was identified during design review. Only the
`watch`-based version was ever actually written to `keepalive.rs`.

## Blockers

None.
