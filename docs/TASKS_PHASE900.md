# Tasks: Phase 900 — CLI and Config Test Retrofit

| Field | Value |
|-------|-------|
| Phase | 900 |
| Name | CLI and Config Test Retrofit |
| Project | anvilml |
| Status | Approved |
| Depends on phases | 3 (partially — after P3-A3, before P3-A4) |

## Overview

Phase 900 is a two-task retrofit that corrects two independent CI failures both
introduced during Phase 2 and exposed before Phase 3 work can safely proceed.

**P900-A1** fixes a hard compilation failure on the `rust-windows` CI runner.
The `test_custom_port_health` integration test in `backend/tests/cli_tests.rs`
detects the OS-assigned port by shelling out to `lsof`, a tool that does not exist
on Windows. The `ss` fallback is unreachable on Windows because it is only attempted
when `lsof` returns output, not when `lsof` fails to launch. The result is an
immediate `NotFound` panic on every Windows CI run. The fix is purely mechanical:
wrap the port-detection logic in `#[cfg(unix)]` / `#[cfg(windows)]` branches. The
Unix branch is unchanged (`lsof` → `ss` fallback). The Windows branch uses
`netstat -ano`, which is always present on Windows, filtered by the known child PID
(available from the already-spawned `Child` handle) to extract the listening port
without any new dependencies.

**P900-A2** fixes a non-deterministic race condition on both CI runners.
`config_load_tests.rs` has three tests that call `std::env::set_var`. Cargo's default
test harness runs all tests in a binary concurrently on multiple OS threads. `std::env`
is a process-global, non-atomic resource. The capture-and-restore pattern already
present in the tests prevents state leaking between *sequential* tests but provides no
protection against a concurrent thread observing the mutated value between `set_var`
and restore. The result is non-deterministic: whichever thread reads the env var first
determines the observed port value, causing `test_env_var_beats_toml` to intermittently
see `port == 9001` (the TOML value) instead of `8080`. The fix is `#[serial]` from the
`serial_test` crate, which serialises all annotated tests within the same binary,
eliminating the race window.

Both fixes are test-only changes. No public APIs, no crate interfaces, and no runtime
behaviour are modified. The prereqs chain holds P3-A4 and all subsequent Phase 3 tasks
until both fixes are confirmed green on both CI runners.

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | backend integration tests | P900-A1 | `#[cfg]`-gate port-detection in `cli_tests.rs` |
| A | anvilml-core unit tests | P900-A2 | Add `serial_test` dev-dep; annotate three env-var tests with `#[serial]` |

## Prerequisites

P3-A3 complete. `backend/tests/cli_tests.rs` exists with the failing `lsof`-only
port-detection block. `crates/anvilml-core/tests/config_load_tests.rs` exists with
the three env-var-mutating tests missing `#[serial]`.

## Interfaces and Contracts

This phase modifies no public APIs, no crate interfaces, and no environment variable
contracts. Both tasks are test-only changes.

## Task Descriptions

### Group A — backend integration tests

#### P900-A1: backend: fix cli_tests port-detection to compile and pass on Windows

**Goal:** Replace the unconditional `lsof` call in `test_custom_port_health` with a
`#[cfg(unix)]` / `#[cfg(windows)]` block. The Unix branch retains the existing `lsof`
→ `ss` fallback chain verbatim. The Windows branch shells out to `netstat -ano`,
filters output lines by the child process PID (obtained from `child.id()` captured
before the `catch_unwind` closure), and parses the `Local Address` column to extract
the port — the same `0.0.0.0:PORT` or `127.0.0.1:PORT` format `netstat` always emits
on Windows.

The child PID must be captured as `let child_pid = child.id();` before the
`catch_unwind` closure, because the closure moves `child` and PID retrieval inside the
closure would require a borrow the closure cannot hold alongside the kill-on-exit
pattern. The `kill_child` helper is cross-platform already and requires no changes.

No new crate dependencies are introduced. `netstat` is an inbox binary on all supported
Windows versions.

**Acceptance criterion:** `cargo test -p anvilml --features mock-hardware --test cli_tests`
exits 0 on both the `rust-linux` and `rust-windows` CI runners.

### Group A — anvilml-core unit tests

#### P900-A2: anvilml-core: add `#[serial]` to env-var-mutating config_load tests

**Goal:** Add `serial_test` as a dev-dependency to `crates/anvilml-core/Cargo.toml`.
Add `use serial_test::serial;` at the top of `config_load_tests.rs`. Annotate
`test_env_var_beats_toml`, `test_cli_override_beats_env`, and `test_nested_env_var`
with `#[serial]`. Do not annotate `test_missing_file_uses_defaults` — it sets no env
vars and must remain freely parallel. The existing capture-and-restore teardown in each
test is correct and must be preserved unchanged.

Update the three affected entries in `docs/TESTS.md` to note the `#[serial]`
annotation and its justification (process-global `std::env` is non-atomic; concurrent
threads can observe `set_var` mid-flight).

**Acceptance criterion:** `cargo test -p anvilml-core --test config_load_tests` exits 0
under 50 consecutive runs:

    for i in $(seq 1 50); do cargo test -p anvilml-core --test config_load_tests || exit 1; done

## Runnable Proof

P900-A1 — Windows port-detection, both runners must be green:

    cargo test -p anvilml --features mock-hardware --test cli_tests

P900-A2 — config load race eliminated, 50 consecutive runs zero failures:

    for i in $(seq 1 50); do
      cargo test -p anvilml-core --test config_load_tests || exit 1
    done

Full workspace clean after both fixes:

    cargo test --workspace --features mock-hardware

All three commands must exit 0. The 50-run loop is the gate for marking P900-A2
complete. The `rust-windows` result for the first command is the gate for P900-A1.