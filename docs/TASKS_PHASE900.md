# Tasks: Phase 900 — CLI Test Windows Port-Detection Fix

| Field | Value |
|-------|-------|
| Phase | 900 |
| Name | CLI Test Windows Port-Detection Fix |
| Project | anvilml |
| Status | Approved |
| Depends on phases | 3 (partially — after P3-A3, before P3-A4) |

## Overview

Phase 900 is a single-task retrofit that corrects a Windows CI failure introduced by
P2-B1. The `test_custom_port_health` integration test in `backend/tests/cli_tests.rs`
detects the OS-assigned port by shelling out to `lsof`, a tool that does not exist on
Windows. The `ss` fallback is unreachable on Windows because it is only attempted when
`lsof` returns output, not when `lsof` fails to launch. The result is an immediate
`NotFound` panic on every Windows CI run.

The fix is purely mechanical: wrap the port-detection logic in `#[cfg(unix)]` /
`#[cfg(windows)]` branches. The Unix branch is unchanged (`lsof` → `ss` fallback). The
Windows branch uses `netstat -ano`, which is always present on Windows, filtered by the
known child PID (available from the already-spawned `Child` handle) to extract the
listening port without any new dependencies.

No other files are touched. No test logic changes. The insertion point in the
prereqs chain keeps P3-A4 (and all subsequent P3 tasks) blocked until this fix is
confirmed green on both runners.

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | backend integration tests | P900-A1 | cfg-gate port-detection in cli_tests.rs |

## Prerequisites

P3-A3 complete. `cli_tests.rs` exists at `backend/tests/cli_tests.rs` with the
failing `lsof`-only port-detection block.

## Interfaces and Contracts

This task modifies no public APIs, no crate interfaces, and no environment variable
contracts. It is a test-only change.

## Task Descriptions

### Group A — backend integration tests

#### P900-A1: backend: fix cli_tests port-detection to compile and pass on Windows

**Goal:** Replace the unconditional `lsof` call in `test_custom_port_health` with a
`#[cfg(unix)]` / `#[cfg(windows)]` block. The Unix branch retains the existing `lsof`
→ `ss` fallback chain verbatim. The Windows branch shells out to
`netstat -ano -p TCP`, filters lines by the child process PID (obtained from
`child.id()`), and parses the `Local Address` column to extract the port — the same
`0.0.0.0:PORT` or `127.0.0.1:PORT` format `netstat` always emits on Windows.

The `kill_child` helper already has no platform-specific code — `child.kill()` and
`child.wait()` are cross-platform — so it requires no changes.

**Acceptance criterion:** `cargo test -p anvilml --features mock-hardware --test cli_tests`
exits 0 on both the `rust-linux` and `rust-windows` CI runners.

## Runnable Proof