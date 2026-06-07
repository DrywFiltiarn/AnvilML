# Tasks: Phase 901 — Worker Test Teardown Fix

| Field | Value |
|-------|-------|
| Phase | 901 |
| Name | Worker Test Teardown Fix |
| Milestone group | Correction (retrofit) |
| Depends on phases | 11 |
| Task file | `forge/tasks/tasks_phase901.json` |
| Tasks | 1 |

## Overview

Phase 901 is a single-task correction phase in the 900-series retrofit namespace. It fixes a test env-var leak in `anvilml-worker` that causes `spawn_ping_pong` to fail consistently under `serial_test`. The failure blocks all subsequent phases at the `cargo test --workspace` gate via `FORGE_AGENT_RULES §9.4`.

The root cause is missing `remove_var` teardown in two tests introduced during the P10–P11 C-group. The pattern is structurally identical to P11-B1 (which fixed the same class of env-var bleed in `anvilml-hardware`). No production code changes. No new tests. No new dependencies.

## Tasks

| Task | Module / File | Summary |
|------|-------------|---------|
| P901-A1 | `crates/anvilml-worker/src/managed.rs` | anvilml-worker: env-var teardown in spawning tests to fix spawn_ping_pong failure |

## Task details

#### P901-A1: anvilml-worker: env-var teardown in spawning tests to fix spawn_ping_pong failure

- **Prereqs:** P11-C3
- **Tags:** reasoning

`spawn_ping_pong` fails with `"worker did not reach Ready state in time"` (10s timeout) when run after `handshake_completes_once` under `serial_test`. The failure is confirmed pre-existing on the unmodified main branch (verified by the P12-A1 agent via `git stash`).

**Root cause:** `handshake_completes_once` calls `set_var("ANVILML_PING_INTERVAL_MS", "50")` and `set_var("ANVILML_PONG_TIMEOUT_MS", "150")` but never calls `remove_var`. The leaked `ANVILML_PING_INTERVAL_MS=50` is read by `ManagedWorker::new()` in the next serialised test, constructing a worker with `ping_interval=50ms`. The keepalive fires 50ms into Python startup and the `pong_timeout=150ms` watchdog kills the child at ~200ms — before the Python process finishes importing and sends `Ready` (300–500ms on a cold CI runner). `spawn_reaches_idle` also leaks `ANVILML_WORKER_MOCK` without teardown.

**Fix:** Add `remove_var` calls as the unconditional final statements of the two affected tests:

`handshake_completes_once` — append before the closing `}`:
```rust
std::env::remove_var("ANVILML_WORKER_MOCK");
std::env::remove_var("ANVILML_PING_INTERVAL_MS");
std::env::remove_var("ANVILML_PONG_TIMEOUT_MS");
```

`spawn_reaches_idle` — append before the closing `}`:
```rust
std::env::remove_var("ANVILML_WORKER_MOCK");
```

`status_transitions` and `spawn_ping_pong` call no `set_var` and require no changes. No production code changes. No new tests. No new dependencies. This is the identical pattern applied by P11-B1 to `anvilml-hardware`.

`cargo test -p anvilml-worker --features mock-hardware` exits 0 with 0 failed on both platforms. `cargo check --target x86_64-pc-windows-gnu --features mock-hardware` exits 0.

## Runnable Proof

```bash
ANVILML_WORKER_MOCK=1 ANVILML_VENV_PATH=.venv \
  cargo test -p anvilml-worker --features mock-hardware
# Expected: test result: ok. 16 passed; 0 failed; 0 ignored
```