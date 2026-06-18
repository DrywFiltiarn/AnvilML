# Implementation Report: P902-A3

| Field         | Value                                                                              |
|---------------|-------------------------------------------------------------------------------------|
| Task ID       | P902-A3                                                                              |
| Phase         | 902 — Keepalive Ready-Gate, Prompt Shutdown, and Demux Per-Message Error Handling Retrofit |
| Description   | anvilml-ipc/anvilml-worker: RecvError type so RouterTransport::recv() distinguishes socket-fatal from per-message failures; fix demux.rs to only stop on the fatal case |
| Implemented   | 2026-06-18T11:50:00Z                                                                |
| Status        | COMPLETE                                                                             |

## Summary

A carried-forward known-issue note (dated 2026-06-17, deferred at the time to keep focus
on the run()/shutdown() mutex deadlock fix) flagged that `RouterTransport::recv()` in
`anvilml-ipc` collapsed four structurally distinct failure modes — a genuine ZeroMQ
socket-level failure, a missing identity frame, a missing payload frame, and a
`decode_event` failure — into one flat `AnvilError::Ipc(String)`, and that
`demux::start()`'s recv loop treated every one of those as fatal, breaking the loop
(and stopping the only demux task in the entire process) on a single malformed message
from any one peer, killing event delivery for every worker simultaneously. Both of the
note's own draft fix options (restructure `AnvilError` itself, or have `demux.rs`
fragile-parse error strings) turned out to be broader than necessary once the actual code
was read closely: `AnvilError` is the server's public HTTP-facing error type and nothing
matches on its `Ipc` variant by name outside `recv()`'s own error construction, and
`recv()` already constructs each of the four failure cases at a distinct call site — it
was simply discarding which one by flattening to a `String` before returning. Fixed by
adding a new `RecvError` enum to `anvilml-ipc::error` (`SocketClosed(#[from] ZmqError)`,
`MissingIdentityFrame`, `MissingPayloadFrame`, `DecodeFailed(#[from] IpcError)`), changing
`RouterTransport::recv()` and `recv_with_raw_identity()` to return it in place of
`AnvilError`, and adding `impl From<RecvError> for AnvilError` so the one caller outside
`anvilml-ipc`/`anvilml-worker` that propagates the error via `?` into a function pinned to
`Result<(), AnvilError>` (`anvilml-ipc/tests/stress_test.rs::run_stress_test`) keeps
compiling unchanged, with no edits required. `demux::start()`'s loop now matches on the
concrete `RecvError` variant: `break` only on `SocketClosed`, log-and-fall-through to the
next iteration on the other three.

## Resolved Dependencies

None. `RecvError` is built entirely from types already present in the workspace
(`thiserror`, `zeromq::ZmqError`, the crate's own pre-existing `IpcError`). No new
external crates, packages, or feature flags introduced.

## Files Changed

| Action | Path | Description |
|--------|------|--------------|
| MODIFY | `crates/anvilml-ipc/src/error.rs` | New `RecvError` enum (4 variants); `impl From<RecvError> for anvilml_core::AnvilError` |
| MODIFY | `crates/anvilml-ipc/src/transport.rs` | `recv()` and `recv_with_raw_identity()` signatures changed from `Result<_, AnvilError>` to `Result<_, RecvError>`; each of the four internal error-construction sites changed from `AnvilError::Ipc(format!(...))` to the matching concrete `RecvError` variant; doc comments updated |
| MODIFY | `crates/anvilml-ipc/src/lib.rs` | `RecvError` added to the `pub use error::{...}` re-export line |
| MODIFY | `crates/anvilml-worker/src/demux.rs` | `Err(e) => { break }` single arm replaced with `Err(RecvError::SocketClosed(e)) => break` plus a second arm (or-pattern over the remaining three variants) that logs and falls through without breaking; `start()`'s doc comment updated to describe the corrected per-variant behaviour |
| MODIFY | `crates/anvilml-worker/tests/demux_tests.rs` | New test `test_demux_survives_undecodable_payload` (sends genuinely invalid msgpack from one DEALER, asserts the demux task's `JoinHandle::is_finished()` is still `false`, then sends a real registered event from a second DEALER and confirms it's still delivered); stale comment in `test_demux_drops_event_for_unregistered_identity` corrected to stop describing a decode-failure workaround that is no longer necessary (the test body itself required no behavioural change) |

## Commit Log

```
Not available in this session — changes were applied directly to repository files
outside the normal Forge git-staging flow. No `git add -A` was run; this report
documents the change set for task-graph reconciliation purposes. The user applied
these files locally and confirmed cargo fmt, cargo clippy, and cargo test --workspace
all green after applying them.
```

## Test Results

```
User-confirmed, full workspace, after this task's files were applied locally:

cargo fmt --check
  (clean, no output)

cargo clippy --workspace --all-targets --features mock-hardware -- -D warnings
  (clean, no output)

cargo test --workspace --features mock-hardware
  (all suites green, including the new test_demux_survives_undecodable_payload and the
  corrected test_demux_drops_event_for_unregistered_identity)
```

A full-workspace grep for every call site of `RouterTransport::recv()` and
`recv_with_raw_identity()` (via a fresh `codeload.github.com` tarball of the repository,
not a cached or partial view) was performed before this change was finalised, confirming
exactly two production call sites (`demux.rs`; nothing else) and five test call sites
across `anvilml-ipc`, `anvilml-worker` — none of which match on the error type by name
except via type-agnostic `.expect()`/`?` propagation, both of which are unaffected by the
signature change.

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
New pub items introduced:
- anvilml_ipc::error::RecvError (enum, 4 variants) — re-exported as anvilml_ipc::RecvError
- impl From<RecvError> for anvilml_core::AnvilError

Breaking signature changes to existing pub items:
- RouterTransport::recv() — return type Result<_, AnvilError> -> Result<_, RecvError>
- RouterTransport::recv_with_raw_identity() — same change

No changes to anvilml_core::AnvilError's own variant set.
```

## Deviations from Plan

There is no prior approved plan for this task — it was performed as direct, manual,
human-directed work against the repository rather than through a PLAN/ACT Forge session.
This report is written retroactively. One deviation worth recording: the carried-forward
note that originally flagged this defect proposed two fix options, both broader than what
was actually implemented (see Summary above for why). Neither of the note's two options
was the one ultimately chosen.

## Blockers

None.
