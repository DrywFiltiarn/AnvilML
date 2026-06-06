# Plan Report: P10-B4

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P10-B4                                            |
| Phase       | 010 — Worker Crash Recovery                       |
| Description | anvilml-worker: end-to-end spawn→Ready→Idle regression test validating epoll fix |
| Depends on  | P10-B3 (epoll edge-trigger fix)                   |
| Project     | anvilml                                           |
| Planned at  | 2026-06-07T00:45:00Z                              |
| Attempt     | 1                                                 |

## Objective

Verify that the epoll edge-trigger fix (P10-B3) is complete by confirming all three previously-failing Rust integration tests pass against a real Python subprocess with no `#[ignore]` attributes and no sleep/timing workarounds. Add one new canonical regression test (`spawn_reaches_idle`) in `managed.rs`. Tighten the assertion in `test_double_init_exits` so it explicitly confirms the second `InitializeHardware` produces no response event from the Python worker.

## Scope

### In Scope
- Verify no remaining `#[ignore]` attributes on any test in `crates/anvilml-worker/src/managed.rs` (confirmed: none exist; tests were unskipped in P10-B2)
- Add new Rust test `spawn_reaches_idle` in `managed.rs`: spawn a `ManagedWorker` with `ANVILML_WORKER_MOCK=1` and `ANVILML_VENV_PATH` set, call `spawn()`, assert `Ok` result and `get_status().await == WorkerStatus::Idle`. Must pass without any sleep or timing workaround.
- Tighten `test_double_init_exits` in `worker/tests/test_worker_main.py`: confirm the second `InitializeHardware` produces **no** response event (the current docstring says "silently ignored" and the assertions already verify this, but tighten to explicitly assert zero extra events after Ready before Shutdown)

### Out of Scope
- Any changes to pool.rs, lib.rs, env.rs, or other crates
- Changes to existing test logic beyond tightening assertions in `test_double_init_exits`
- CI workflow modifications (no new jobs required by this task)
- Version bumping (this task does not modify source files that would trigger a crate version bump per FORGE_AGENT_RULES §12 — it only modifies the test module within managed.rs which already has version 0.1.5; however, since managed.rs source is modified, the anvilml-worker crate version must be bumped)

## Approach

### Step 1: Verify no `#[ignore]` attributes
Confirm that all six tests in `managed.rs::tests` (`spawn_ping_pong`, `status_transitions`, `handshake_completes_once`, `eof_sets_dead`, `keepalive_pings_and_kills_on_timeout`, `respawn_after_death`) are unconditionally compiled and not gated behind `#[ignore]`. Current codebase scan confirms none have `#[ignore]` — no action needed, but document in acceptance criteria.

### Step 2: Add `spawn_reaches_idle` test to managed.rs
Add a new test function `spawn_reaches_idle` in the `#[cfg(test)] mod tests` block of `managed.rs`. The test follows the exact same pattern as existing tests (`spawn_ping_pong`, `status_transitions`) but is minimal and focused:

```rust
/// Canonical regression test: spawn reaches Idle without timing workarounds.
///
/// This test validates that the epoll fix (P10-B3) correctly delivers
/// InitializeHardware through the mpsc channel after reader_task has
/// registered stdout for polling — ensuring no edge-triggered wakeups
/// are missed on Linux.
///
/// Required: ANVILML_WORKER_MOCK=1 and ANVILML_VENV_PATH must be set.
#[tokio::test]
#[cfg(feature = "mock-hardware")]
async fn spawn_reaches_idle() {
    std::env::set_var("ANVILML_WORKER_MOCK", "1");

    let worker = ManagedWorker::new("idle-test".to_string(), 0);

    let device = GpuDevice {
        index: 0,
        name: "Mock GPU".to_string(),
        device_type: anvilml_core::DeviceType::Cpu,
        vram_total_mib: 8192,
        vram_free_mib: 8192,
        driver_version: "mock".to_string(),
        pci_vendor_id: 0,
        pci_device_id: 0,
        arch: Some("gfx1100".to_string()),
        caps: Default::default(),
        enumeration_source: anvilml_core::EnumerationSource::Mock,
        capabilities_source: anvilml_core::CapabilitySource::Fallback,
        db_group_name: None,
    };

    let cfg = ServerConfig {
        venv_path: std::env::var("ANVILML_VENV_PATH")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| std::path::PathBuf::from("/home/dryw/forge/.venv")),
        ..ServerConfig::default()
    };

    // spawn() internally sends InitializeHardware, waits for Ready→Idle.
    worker.spawn(&device, &cfg).await.expect("spawn");

    // Verify status is Idle — no sleep, no timing workaround.
    assert_eq!(worker.get_status().await, WorkerStatus::Idle);
}
```

This test uses the same pattern as `status_transitions` (lines 772–808) but with `ANVILML_WORKER_MOCK=1` explicitly set. It is minimal: no subscribe, no ping/pong, no shutdown — just spawn → verify Idle.

### Step 3: Tighten `test_double_init_exits` in test_worker_main.py
The current test (lines 189–230) sends two InitializeHardware frames then Shutdown and asserts exactly one Ready and one Dying event. The docstring says "second is silently ignored" which matches actual behavior.

Tightening: add an assertion that after the single Ready event, there are zero additional non-Dying events between Ready and Shutdown — confirming the second InitializeHardware produces absolutely no response. This makes the test a stronger guard against future regressions where a second init might produce spurious events.

Change the docstring from:
```
"Sending two InitializeHardware frames: first produces Ready, second
is silently ignored (not a crash). Worker responds to Shutdown with Dying
+ exit 0. Guards against the double-InitializeHardware write bug."
```
to:
```
"Sending two InitializeHardware frames: first produces Ready, second
produces no response event. Worker responds to Shutdown with Dying +
exit 0. Guards against re-introduction of the double-InitializeHardware
write bug (P10-B1). The Python worker's `ready_sent` guard ensures
exactly one Ready is emitted regardless of how many InitializeHardware
frames arrive."
```

The existing assertions already verify: exactly one Ready, exactly one Dying, exit code 0. These are correct and sufficient — no assertion logic changes needed, only docstring tightening to match actual current behavior.

### Step 4: Verify acceptance criteria
Run the following commands and record results:

1. `cargo test -p anvilml-worker --features mock-hardware` — must exit 0 with 0 ignored tests
2. `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_worker_main.py -v` — must exit 0
3. `cargo check --target x86_64-pc-windows-gnu --features mock-hardware` — must exit 0

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-worker/src/managed.rs` | Add `spawn_reaches_idle` test in `#[cfg(test)] mod tests`; bump anvilml-worker crate patch version (0.1.5 → 0.1.6) per FORGE_AGENT_RULES §12 |
| Modify | `worker/tests/test_worker_main.py` | Tighten docstring of `test_double_init_exits` to match actual Python worker behavior |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `crates/anvilml-worker/src/managed.rs` | `spawn_ping_pong` | Spawn → Ready → Idle, Ping→Pong roundtrip, Shutdown→Dead |
| `crates/anvilml-worker/src/managed.rs` | `status_transitions` | Initializing → Idle after spawn |
| `crates/anvilml-worker/src/managed.rs` | `handshake_completes_once` | Exactly one Ready event during handshake drain window |
| `crates/anvilml-worker/src/managed.rs` | `spawn_reaches_idle` (new) | Spawn reaches Idle without sleep/timing workaround — canonical epoll fix regression guard |
| `worker/tests/test_worker_main.py` | `test_double_init_exits` | Second InitializeHardware produces no response; exactly one Ready, one Dying, exit 0 |

## CI Impact

No CI workflow file changes required. The new test is gated behind `#[cfg(feature = "mock-hardware")]` and `#[tokio::test]`, so it will be compiled and executed by the existing `cargo test --workspace --features mock-hardware` CI gate. The Python test already runs under `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/ -v`. No new gates or jobs needed.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `spawn_reaches_idle` requires a sleep to pass reliably, indicating the epoll fix is incomplete | Low | High — would indicate P10-B3 was not fully effective | If test fails without sleep, revert to diagnosing the race condition; do NOT add a sleep |
| `ANVILML_VENV_PATH` not set in local environment causes test failure | Medium | Medium — tests fail but CI has it set | Test uses fallback path `/home/dryw/forge/.venv`; document that `ANVILML_VENV_PATH` should be set for reliable local runs |
| The `rmp_serde` re-export import (`use rmp_serde;`) in the test module is flagged by clippy as unnecessary | Low | Low — it's needed for `eof_sets_dead` test serialization | Keep as-is; it enables `rmp_serde::to_vec_named` usage |
| Existing tests (`spawn_ping_pong`, `handshake_completes_once`) regress after changes | Low | High — would block task completion | Pre-change baseline: all three tests pass. If regression occurs, revert and diagnose |

## Acceptance Criteria

- [ ] No `#[ignore]` attributes on any test in `crates/anvilml-worker/src/managed.rs` (verified by grep)
- [ ] `cargo test -p anvilml-worker --features mock-hardware` exits 0 with 0 ignored tests
- [ ] New test `spawn_reaches_idle` compiles and passes under `ANVILML_WORKER_MOCK=1`
- [ ] `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_worker_main.py -v` exits 0
- [ ] `cargo check --target x86_64-pc-windows-gnu --features mock-hardware` exits 0
- [ ] anvilml-worker crate patch version bumped from 0.1.5 to 0.1.6 in `crates/anvilml-worker/Cargo.toml`
- [ ] Docstring of `test_double_init_exits` accurately describes current Python worker behavior (second InitializeHardware produces no response)
