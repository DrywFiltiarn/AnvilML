# Plan Report: P7-A4

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P7-A4                                         |
| Phase       | 007 — WebSocket Event Stream                  |
| Description | anvilml-server: system.stats tick task (5s broadcast) |
| Depends on  | P7-A3                                        |
| Project     | anvilml                                       |
| Planned at  | 2026-06-04T18:00:00Z                         |
| Attempt     | 1                                             |

## Objective

Create a background tokio task in `anvilml-server` that fires every 5 seconds, reads the latest hardware state from `AppState` (per-device VRAM) and host RAM via the `sysinfo` crate, builds a `SystemStatsEvent`, and broadcasts it as `WsEvent::SystemStats` through the existing `EventBroadcaster`. The task is exposed as `spawn_system_stats_tick(state: AppState) -> JoinHandle<()>` in a new module `src/ws/stats_tick.rs`.

## Scope

### In Scope
- Create `crates/anvilml-server/src/ws/stats_tick.rs` with:
  - `spawn_system_stats_tick(state: AppState) -> JoinHandle<()>` function
  - `tokio::time::interval(Duration::from_secs(5))` loop
  - Per-tick: read `AppState.hardware` (Arc<RwLock<HardwareInfo>>), iterate GPUs for `GpuStatSnapshot`, read host RAM from `sysinfo::System`, build `SystemStatsEvent { timestamp, gpus, ram_used_mib, ram_total_mib }`, call `broadcaster.send(WsEvent::SystemStats(event))`
  - Proper graceful shutdown via `interval.tick()` yielding on cancellation (no explicit cancel token needed — tokio handles JoinHandle drop)
- Update `crates/anvilml-server/src/ws/mod.rs` to declare the new `stats_tick` submodule
- No caller wiring yet (deferred to P7-A5 which calls this from main startup)

### Out of Scope
- Wiring the tick task into `main.rs` or `build_router` — deferred to P7-A5
- Testing the live WS stream — deferred to P7-A5
- Any changes to CI, OpenAPI, config files, or other crates
- Worker MemoryReport integration (VRAM used stays at 0 until workers report; task reads from AppState.hardware which has vram_used_mib = 0 for all devices initially)

## Approach

1. **Read existing types.** Confirm `GpuStatSnapshot` fields (`index: u32`, `vram_used_mib: u32`, `vram_total_mib: u32`), `SystemStatsEvent` fields, and `WsEvent::SystemStats(SystemStatsEvent)` variant from `anvilml-core/src/types/events.rs`. Confirm `HardwareInfo` has `gpus: Vec<GpuDevice>` with each device exposing `index`, `vram_used_mib`, `vram_total_mib`.

2. **Create `stats_tick.rs`.** In `crates/anvilml-server/src/ws/`:
   - Import `tokio::time::{interval, Duration}`, `anvilml_core::types::{HardwareInfo, SystemStatsEvent, WsEvent, GpuStatSnapshot}`, `crate::state::AppState`, `crate::ws::broadcaster::EventBroadcaster` (or re-exported path), `sysinfo::System`.
   - Define `pub fn spawn_system_stats_tick(state: AppState) -> JoinHandle<()>`:
     - Clone broadcaster reference from state (`state.broadcaster.clone()`).
     - Clone hardware reference (`Arc<RwLock<HardwareInfo>>`).
     - Spawn a tokio task with the interval loop.
     - Inside the loop: `interval.tick().await`, then:
       - Read lock `hardware.read()`.
       - Build `gpus: hardware.gpus.iter().map(|d| GpuStatSnapshot { index: d.index, vram_used_mib: d.vram_used_mib, vram_total_mib: d.vram_total_mib }).collect()`
       - Release lock.
       - Create a new `sysinfo::System`, call `system.refresh_memory()`, extract `total_mib` and `used_mib`.
       - Build `SystemStatsEvent { timestamp: Utc::now(), gpus, ram_used_mib, ram_total_mib }`.
       - Call `broadcaster.send(WsEvent::SystemStats(event))` — ignore return value.
   - Add a unit test: construct an AppState with mock hardware, spawn the tick, await one interval, assert broadcaster can receive an event (similar pattern to existing broadcaster tests).

3. **Update `ws/mod.rs`.** Add `pub mod stats_tick;` alongside the existing `broadcaster` and `handler` declarations.

4. **Verify build.** Run `cargo build --features mock-hardware -p anvilml-server` — acceptance criterion is exit 0. No runtime verification (deferred to P7-A5).

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Create | `crates/anvilml-server/src/ws/stats_tick.rs` | New module: `spawn_system_stats_tick` function with 5s interval loop |
| Modify | `crates/anvilml-server/src/ws/mod.rs` | Add `pub mod stats_tick;` declaration |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `crates/anvilml-server/src/ws/stats_tick.rs` (inline test module) | `test_stats_tick_broadcasts_event` | Constructs AppState with mock hardware, spawns tick task, awaits one 5s interval via a shorter test interval or by using tokio's test runtime, asserts broadcaster receiver gets the SystemStats event with correct GPU count and non-zero timestamps. |

Note: The acceptance criterion for this task is `cargo build --features mock-hardware` exits 0. Runtime verification is deferred to P7-A5. A unit test is included but the task does not require a full integration test suite pass — only that the module compiles cleanly.

## CI Impact

No CI changes required. The task only adds source code within the existing `anvilml-server` crate, which is already covered by `cargo test --workspace --features mock-hardware`. No new dependencies are introduced (the `sysinfo` crate is already declared as a dependency). No workflow files or OpenAPI annotations are touched.

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| `sysinfo` memory refresh may be slow on first call | Use `sysinfo::System` fresh each tick (cheap to construct); the first `refresh_memory()` does the OS query, subsequent calls in the same tick are fast. Acceptable for a 5s interval. |
| AppState clone semantics — `AppState` contains `Arc`s so cloning is cheap, but ensure `spawn_system_stats_tick` takes ownership (by value) of a cloned `AppState` to avoid borrow conflicts with the HTTP server task | Take `state: AppState` by value; the caller clones before passing. This matches the pattern used for other spawned tasks in the codebase. |
| `vram_used_mib` stays 0 until workers report — the event will always show 0 VRAM usage initially | Documented in task description as expected behavior ("used 0 until worker reports exist"). No action needed. |
| tokio runtime not available when this is called | The tick task is spawned from `main.rs` after the tokio runtime is established (P7-A5 wiring). The function signature returns a `JoinHandle` that requires an active runtime to be awaited — this is implicit in how it will be called. |

## Acceptance Criteria

- [ ] `cargo build --features mock-hardware -p anvilml-server` exits 0
- [ ] `crates/anvilml-server/src/ws/stats_tick.rs` exists and exports `spawn_system_stats_tick(state: AppState) -> JoinHandle<()>`
- [ ] `crates/anvilml-server/src/ws/mod.rs` declares the `stats_tick` submodule
- [ ] The tick loop uses a 5-second interval (`tokio::time::interval(Duration::from_secs(5))`)
- [ ] Each tick reads hardware from `AppState.hardware`, builds `SystemStatsEvent`, and calls `broadcaster.send(WsEvent::SystemStats(...))`
- [ ] No warnings from `cargo clippy -- -D warnings` on the new file
