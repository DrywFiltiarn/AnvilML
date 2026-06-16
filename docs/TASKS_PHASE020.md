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

**Goal:** Create `backend/tests/api_crash.rs`: spawn real server with mock worker; submit mock job with `ANVILML_MOCK_NODE_DELAY_MS` set; kill worker PID mid-job; assert via WebSocket that `worker.status == Dead` then `job.status == Failed` with `error: "worker_crashed"` then worker reaches `Idle` again; submit a second job and assert it completes.

**Acceptance criterion:** `cargo test --workspace --features mock-hardware --test api_crash` exits 0.

### Group B — Quality and documentation

#### P20-B1: OpenAPI regeneration and config drift clean pass

**Goal:** Run `cargo run -p anvilml-openapi` and commit the result to `api/openapi.json`. Run `cargo test -p anvilml --features mock-hardware -- config_reference` and fix any drift. Run `cargo clippy --workspace --features mock-hardware -- -D warnings` and fix any accumulated warnings.

**Acceptance criterion:** All three commands exit 0 with no output diff on `openapi.json`.

#### P20-B2: docs/TESTS.md: complete test catalogue

**Goal:** Create or complete `docs/TESTS.md` cataloguing every test in the project per `ANVILML_DESIGN.md §16.1` format: test name, file path, context, what it verifies, inputs, expected output. Include all Rust unit tests, Rust integration tests, and Python pytest tests.

**Acceptance criterion:** `docs/TESTS.md` exists; contains an entry for every test file in `crates/*/tests/`, `backend/tests/`, and `worker/tests/`; `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/ -v` and `cargo test --workspace --features mock-hardware` both exit 0.

## Phase Acceptance Criteria

```bash
cargo test --workspace --features mock-hardware
ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/ -v
cargo clippy --workspace --features mock-hardware -- -D warnings
cargo run -p anvilml-openapi && git diff --exit-code api/openapi.json
cargo test -p anvilml --features mock-hardware -- config_reference
```

All five must exit 0. After this phase all 6 GitHub CI jobs pass.

## Known Constraints and Gotchas

- The crash test must use `ANVILML_MOCK_NODE_DELAY_MS` to slow down mock node execution, giving the test time to kill the worker while the job is Running.
- `TESTS.md` must be kept up to date going forward — this is the last phase where catching up is acceptable. From Phase 021 onwards, every task that adds tests must update `TESTS.md` in the same commit.
