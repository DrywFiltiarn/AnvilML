# Tasks: Phase 020 — End-to-End Validation

| Field | Value |
|-------|-------|
| Phase | 020 |
| Name | End-to-End Validation |
| Project | anvilml |
| Status | Approved |
| Depends on phases | 19 |

## Overview

Phase 020 validates that the complete system works end-to-end: all 6 CI jobs pass, a crash-recovery integration test covers the worker death cycle, and real PNG artifacts are produced by both ZiT and Flux on target hardware.

This phase also closes any gaps opened by earlier phases: the `openapi.json` is regenerated and committed, the config drift gate passes, any clippy warnings accumulated across phases are cleaned up, and the `TESTS.md` catalogue is brought fully up to date.

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | backend | P20-A1 | Crash-recovery integration test |
| B | quality | P20-B1 … P20-B2 | OpenAPI regeneration, TESTS.md catalogue, clippy clean pass |

## Task Descriptions

### Group A — Integration test

#### P20-A1: backend: crash-recovery integration test

**Goal:** Prove the worker crash-recovery cycle works end-to-end: a killed worker is detected, its in-flight job fails cleanly, the worker respawns, and subsequent jobs complete normally.

**Files to create or modify:**
- `backend/tests/api_crash.rs` — new integration test

**Key implementation notes:**
- Start the server with `ANVILML_WORKER_MOCK=1` and `ANVILML_MOCK_NODE_DELAY_MS=2000` set, so the test has time to kill the worker while a job is `Running`.
- Submit a mock job, then get the worker PID via `GET /v1/workers`.
- Kill the PID using a platform-gated call: `libc::kill` on Unix, `TerminateProcess` on Windows — both behind `cfg(...)`.
- Assert via WebSocket, in order: worker reaches `Dead`, the job reaches `Failed` with `error: "worker_crashed"`, the worker reaches `Respawning`, then `Idle`.
- Submit a second job after recovery and assert it reaches `Completed`.

**Acceptance criterion:** `cargo test --workspace --features mock-hardware --test api_crash` exits 0.

### Group B — Quality and documentation

#### P20-B1: OpenAPI regeneration and config drift clean pass

**Goal:** Bring the OpenAPI spec, config drift check, and clippy lint state fully current before the phase closes, since later phases assume a clean baseline.

**Files to create or modify:**
- `api/openapi.json` — regenerated and committed if it differs from the current file
- any source file with an accumulated clippy warning — fixed in place

**Key implementation notes:**
- Run `cargo run -p anvilml-openapi`; if it produces a diff on `api/openapi.json`, commit the result.
- Run `cargo clippy --workspace --features mock-hardware -- -D warnings`; fix all warnings in any file, including pre-existing ones accumulated across earlier phases — this is not scoped to warnings introduced in this phase only.
- Run `cargo test -p anvilml --features mock-hardware -- config_reference` to confirm no config drift.

**Acceptance criterion:** All three commands exit 0 with no output diff on `openapi.json` and no remaining clippy warnings.

#### P20-B2: docs/TESTS.md: complete test catalogue

**Goal:** Bring the test catalogue fully up to date as a one-time catch-up — from Phase 021 onward, every task that adds tests updates this file in the same commit instead.

**Files to create or modify:**
- `docs/TESTS.md` — created or completed

**Key implementation notes:**
- Format per `ANVILML_DESIGN.md §16.1`: one entry per test file, each listing test name, file path, context (preconditions), what it verifies, inputs, and expected output.
- Cover every test file in `crates/*/tests/`, `backend/tests/`, and `worker/tests/` — Rust unit tests, Rust integration tests, and Python pytest tests, with no category omitted.

**Acceptance criterion:** `docs/TESTS.md` exists; contains an entry for every test file in `crates/*/tests/`, `backend/tests/`, and `worker/tests/`; `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/ -v` and `cargo test --workspace --features mock-hardware` both exit 0.

## Phase Acceptance Criteria

```bash
cargo test --workspace --features mock-hardware
ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/ -v
cargo clippy --workspace --features mock-hardware -- -D warnings
cargo run -p anvilml-openapi && git diff --exit-code api/openapi.json
cargo test -p anvilml --features mock-hardware -- config_reference
# Runnable Proof (manual): a killed worker is detected, fails its in-flight job, and respawns
# NOTE: P20-A1's task context says to obtain the worker's OS PID via
# GET /v1/workers, but ANVILML_DESIGN.md's WorkerInfo struct (id, device_index,
# device_name, status, current_job_id, vram_used_mib) has no pid field, and no
# documented HTTP endpoint exposes one. The PID IS available, but only via the
# structured INFO log line on worker spawn (`worker_id=%id, device_index=%idx,
# pid=%pid`, per ANVILML_DESIGN.md's logging table) — not via the API the task
# context names. The proof below uses the log line; confirm with the task
# author/P20-A1's implementer whether this is the intended mechanism, or
# whether WorkerInfo should instead gain a pid field before this phase closes.
cargo run --features mock-hardware 2>worker.log &
sleep 3
PID=$(grep -m1 'Worker spawned' worker.log | grep -oP 'pid=\K[0-9]+')
curl -s -X POST http://127.0.0.1:8488/v1/jobs -H 'Content-Type: application/json' -d @docs/example_workflows/zit_fp8.json
kill -9 "$PID"
sleep 2
curl -s http://127.0.0.1:8488/v1/workers | python3 -c "import sys,json; assert json.load(sys.stdin)[0]['status'] == 'Idle'"
# -> log shows Worker Dead then Worker Respawning then Worker Ready; GET /v1/workers reports status: Idle
kill %1
```

All five gate commands must exit 0. After this phase all 6 GitHub CI jobs pass.

## Known Constraints and Gotchas

- The crash test must use `ANVILML_MOCK_NODE_DELAY_MS` to slow down mock node execution, giving the test time to kill the worker while the job is Running.
- `TESTS.md` must be kept up to date going forward — this is the last phase where catching up is acceptable. From Phase 021 onwards, every task that adds tests must update `TESTS.md` in the same commit.