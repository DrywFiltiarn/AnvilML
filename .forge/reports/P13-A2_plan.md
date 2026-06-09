# Plan Report: P13-A2

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P13-A2                                            |
| Phase       | 013 — Dispatch & Execute                          |
| Description | anvilml-scheduler: select_worker (preference/auto/cpu) |
| Depends on  | P13-A1 (VramLedger)                               |
| Project     | anvilml                                           |
| Planned at  | 2026-06-09T07:45:00Z                              |
| Attempt     | 1                                                 |

## Objective

Add a `select_worker` function to `crates/anvilml-scheduler/src/scheduler.rs` that implements the GPU/device selection algorithm specified in ANVILML_DESIGN.md §9.3. The function takes a `Job`, a slice of `WorkerInfo`, a `VramLedger`, and a `default_device` string, and returns the index of the best idle worker or `None` when no worker is suitable.

## Scope

### In Scope
- Add `pub fn select_worker(job: &Job, workers: &[WorkerInfo], ledger: &VramLedger, default_device: &str) -> Option<usize>` to `scheduler.rs`.
- Implement three selection modes per ANVILML_DESIGN.md §9.3:
  1. **User-specified** (`settings.device_preference = Some(n)`): return the worker at index `n` if `Idle`; return `None` if busy or not found.
  2. **Auto** (`device_preference = None`): collect all `Idle` workers, rank by `free_mib` descending, break ties by `device_index` ascending, pick top.
  3. **Force-CPU** (`default_device == "cpu"`): only consider the worker whose `device_name` is `"CPU"`; return it if `Idle`, else `None`.
- Add unit tests under `mod tests` in `scheduler.rs`:
  - `test_select_preference_idle` — device_preference Some(0) returns worker 0 when idle.
  - `test_select_preference_busy` — device_preference Some(0) returns None when worker 0 is busy.
  - `test_select_preference_not_found` — device_preference Some(99) returns None.
  - `test_select_auto_single_idle` — auto mode picks the only idle worker.
  - `test_select_auto_ranked_by_free_mib` — auto mode picks the worker with highest free_mib.
  - `test_select_auto_tie_break_device_index` — auto mode breaks ties by device_index ascending.
  - `test_select_auto_all_busy` — auto mode returns None when no worker is idle.
  - `test_select_cpu` — force-cpu mode picks the CPU worker.
  - `test_select_cpu_not_available` — force-cpu mode returns None when no CPU worker.
- Bump `anvilml-scheduler` crate patch version from `0.1.10` to `0.1.11`.

### Out of Scope
- The dispatch loop (P13-A3).
- Worker event handling (P13-A5).
- Mock executor (P13-A4).
- Any changes to `ledger.rs`, `queue.rs`, `lib.rs`, or other crates.
- Logging beyond a single DEBUG call inside `select_worker` (no INFO required for a pure selection function).

## Approach

1. **Add the `select_worker` function** to the end of `scheduler.rs`, after the `JobScheduler` impl block and before the existing `mod tests` block. The function is free-standing (not a method on `JobScheduler`) since it is a pure algorithm with no side effects.

2. **Implementation logic:**
   ```
   fn select_worker(job: &Job, workers: &[WorkerInfo], ledger: &VramLedger, default_device: &str) -> Option<usize> {
       // 1. If default_device == "cpu", find and return the CPU worker index if Idle.
       // 2. Otherwise, determine device_preference from job.settings.
       // 3. If Some(n): find worker at index n, return Some(n) if Idle, else None.
       // 4. If None (auto): filter idle workers, sort by free_mib desc then device_index asc, return index of top.
   }
   ```
   - For auto mode, `free_mib` is looked up via `ledger.free_mib(worker.device_index)`.
   - Workers not in the ledger get `free_mib == 0` (per existing ledger semantics).
   - The function returns a `usize` index into the `workers` slice.

3. **Add unit tests** — each test constructs a minimal set of `WorkerInfo` values and a `VramLedger`, calls `select_worker`, and asserts the returned index (or `None`). Tests use `serial_test::serial` to avoid any shared state issues (though `select_worker` is pure, this follows the existing pattern in `scheduler.rs`).

4. **Bump crate version** — change `version = "0.1.10"` to `version = "0.1.11"` in `crates/anvilml-scheduler/Cargo.toml`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-scheduler/src/scheduler.rs` | Add `select_worker` function + test module |
| Modify | `crates/anvilml-scheduler/Cargo.toml` | Bump patch version `0.1.10 → 0.1.11` |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `scheduler.rs` | `test_select_preference_idle` | `device_preference Some(0)` returns index 0 when worker 0 is Idle |
| `scheduler.rs` | `test_select_preference_busy` | `device_preference Some(0)` returns None when worker 0 is Busy |
| `scheduler.rs` | `test_select_preference_not_found` | `device_preference Some(99)` returns None when no such worker |
| `scheduler.rs` | `test_select_auto_single_idle` | Auto mode returns the only idle worker |
| `scheduler.rs` | `test_select_auto_ranked_by_free_mib` | Auto mode picks worker with highest free_mib |
| `scheduler.rs` | `test_select_auto_tie_break_device_index` | Auto mode breaks free_mib ties by device_index ascending |
| `scheduler.rs` | `test_select_auto_all_busy` | Auto mode returns None when all workers are Busy |
| `scheduler.rs` | `test_select_cpu` | Force-CPU mode picks the CPU worker (device_name == "CPU") |
| `scheduler.rs` | `test_select_cpu_not_available` | Force-CPU mode returns None when no CPU worker exists |

## CI Impact

No CI changes required. The task only adds code and tests to an existing crate. The existing CI gates (`cargo test --workspace --features mock-hardware`, `cargo clippy --workspace --features mock-hardware -- -D warnings`, `cargo fmt --all -- --check`) will automatically cover the new code. No new CI jobs, gates, or workflow files are introduced.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `VramLedger::free_mib` returns 0 for unknown devices, causing auto mode to rank unknown workers last | Low | Low | This is the intended ledger behavior; documented in P13-A1. No action needed. |
| Auto mode with all-zero free_mib (empty ledger) could return any idle worker | Medium | Low | Deterministic tie-break by `device_index` ascending ensures stable selection. |
| CPU worker identification relies on `device_name == "CPU"` string comparison | Low | Low | This matches the existing convention in `WorkerInfo::device_name` ("CPU" for CPU workers). If naming changes later, the dispatch loop (P13-A3) can be updated. |
| Test isolation via `serial_test` — if a test panics, subsequent tests are skipped | Low | Low | Each test is self-contained with fresh data; `serial` is only used because the existing test module already uses it. |

## Acceptance Criteria

- [ ] `select_worker` function exists in `crates/anvilml-scheduler/src/scheduler.rs` with signature `fn select_worker(job: &Job, workers: &[WorkerInfo], ledger: &VramLedger, default_device: &str) -> Option<usize>`
- [ ] Device preference mode: `Some(n)` returns the worker at index `n` if Idle, `None` otherwise
- [ ] Auto mode: idle workers ranked by `free_mib` desc, tie `device_index` asc, top selected
- [ ] Force-CPU mode: only the CPU worker (device_name == "CPU") is considered
- [ ] Busy-preferred scenario returns `None`
- [ ] `cargo test -p anvilml-scheduler -- select` exits 0 with all tests passing
- [ ] `cargo clippy --package anvilml-scheduler --features mock-hardware -- -D warnings` exits 0
- [ ] `cargo fmt --all -- --check` exits 0
- [ ] `anvilml-scheduler` crate version bumped to `0.1.11` in `Cargo.toml`
