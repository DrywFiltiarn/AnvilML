# Plan Report: P16-D1

| Field       | Value                                                        |
|-------------|---------------------------------------------------------------|
| Task ID     | P16-D1                                                          |
| Phase       | 016 — Live Events                                                |
| Description | anvilml-server: stats_tick.rs background SystemStats every 5s   |
| Depends on  | P16-C2                                                           |
| Project     | anvilml                                                          |
| Planned at  | 2026-07-09T20:05:00Z                                             |
| Attempt     | 2                                                                 |

## Objective

Create `crates/anvilml-server/src/ws/stats_tick.rs`, implementing
`spawn_stats_tick(broadcaster, workers, interval) -> JoinHandle<()>` — a background task
that publishes a real `WsEvent::SystemStats` (actual host CPU/RAM via `sysinfo`, actual
worker roster via a new `WorkerPool::list()`) to every connected WebSocket client at a
fixed cadence, and wire it into `backend/src/main.rs` alongside the dispatch and event
loops. When this task completes, a running server publishes a fresh `SystemStats` event to
`/v1/events` subscribers every 5 seconds in production, distinct from `ws/handler.rs`'s
(`P16-C1`) own always-placeholder per-connection initial frame, which this task does not
touch. This is the last piece of Phase 16 functionality before `P16-E1`'s Runnable Proof.

**Prior attempt:** Attempt 1 (OpenCode / Qwen3.6 35B A3B) tripped on this task — Dryw
handed it off before a plan or implementation was produced. This plan and its corresponding
implementation supersede that attempt.

## Scope

### In Scope
- Create `crates/anvilml-server/src/ws/stats_tick.rs`:
  - `pub fn spawn_stats_tick(broadcaster: Arc<EventBroadcaster>, workers: Arc<WorkerPool>, interval: Duration) -> JoinHandle<()>`.
  - Holds a `sysinfo::System` across the task's full lifetime (not recreated per tick — see
    Approach for why), calling `refresh_cpu_usage()`/`refresh_memory()` each tick and
    reading `global_cpu_usage()`/`used_memory()`.
  - Calls the new `WorkerPool::list()` for the `workers` field.
  - Publishes `WsEvent::SystemStats { cpu_pct, ram_used_mib, workers }` via
    `broadcaster.publish()` every tick, using `tokio::time::interval` with
    `MissedTickBehavior::Delay`.
- Add `pub async fn list(&self) -> Vec<WorkerInfo>` to `WorkerPool`
  (`crates/anvilml-worker/src/pool.rs`) — the task context's own "added here if absent"
  clause, confirmed absent by codebase inspection.
- Update `crates/anvilml-server/src/ws/mod.rs` — declare `pub mod stats_tick;`, re-export
  `spawn_stats_tick`.
- Update `crates/anvilml-server/Cargo.toml`:
  - Add `sysinfo` as a normal (non-dev) dependency — version resolved live.
  - Add explicit `rt` + `time` tokio features to `[dependencies]` (needed by `tokio::spawn`/
    `tokio::time::interval`, not previously required by anything else in this crate's own
    `[dependencies]` edge).
  - Add a `[dev-dependencies]` override enabling `anvilml-worker`'s `test-utils` feature
    (for `set_up_test_workers()` in the new test file), mirroring
    `crates/anvilml-scheduler/Cargo.toml`'s existing identical override.
- Update `backend/src/main.rs` — call `spawn_stats_tick(Arc::clone(&broadcaster),
  Arc::clone(&workers), Duration::from_secs(5))` alongside the existing dispatch/event-loop
  spawns, and abort+await its `JoinHandle` during graceful shutdown (same pattern as
  `event_loop_handle`, and for the same reason — it holds an `Arc<WorkerPool>` clone that
  must be released before `Arc::try_unwrap(workers)`).
- Create `crates/anvilml-server/tests/stats_tick_tests.rs` — >=4 tests using a short
  injected interval.
- Update `docs/TESTS.md` — one entry per new test.
- Bump the patch version of every manifest whose source this task modifies:
  `anvilml-server` (`0.1.14` → `0.1.15`), `anvilml-worker` (`0.1.33` → `0.1.34`), and
  `backend` (`0.1.14` → `0.1.15`, since `main.rs` is modified) — per
  `FORGE_AGENT_RULES.md`'s version-bump rule.

### Out of Scope
- `ws/handler.rs`'s own per-connection initial `SystemStats` frame (`P16-C1`) — this task's
  own Files list is `stats_tick.rs` + `main.rs` only; the initial-connect frame remains the
  placeholder `P16-C1` established. The two are genuinely separate concerns: one fires once
  per new connection, the other fires on a fixed cadence to every already-subscribed client.
- `WorkerInfo.pid` and `WorkerInfo.current_job_id` — neither is tracked at the
  `WorkerHandle`/`WorkerPool` layer today (see Existing Codebase Assessment). `list()`
  always returns `None` for both; populating them accurately is a separate, unscoped task.
- Any `/v1/workers` REST endpoint — `WorkerPool::list()` is written generally enough for a
  future such endpoint to reuse, but adding that route is not this task's job.
- `P16-E1`'s Runnable Proof itself — this task only makes it possible.

## Existing Codebase Assessment

`AppState.broadcaster: Arc<EventBroadcaster>` (`P16-B1`) and `ws/handler.rs`'s forward loop
(`P16-C2`, confirmed complete — `.forge/state/CURRENT_TASK.md` shows `Task: P16-C2, Status:
COMPLETE`) already deliver whatever gets published on `broadcaster` to every connected
client. This task only needs to call `broadcaster.publish()` on a timer; no handler-side
change is needed for a published event to reach clients.

`WorkerPool` (`crates/anvilml-worker/src/pool.rs`) exposes `handles() -> &[WorkerHandle]`
and `devices() -> &[GpuDevice]`, with the struct's own field doc comment confirming `devices[i]`
corresponds to `handles[i]` by construction — safe to `zip()`. No method already returns
`Vec<WorkerInfo>` anywhere in the codebase (confirmed by grep — `WorkerInfo` is defined in
`anvilml-core` and referenced only in that crate's own tests; no production code constructs
one). `WorkerHandle::status()` is `async fn(&self) -> WorkerStatus` (a lock read); `WorkerHandle`
carries no `pid` or `current_job_id` field, and `WorkerStatus::Busy` carries no embedded job
ID either — accurate values for either `WorkerInfo` field would require reaching into
`ManagedWorker`'s own process handle (for `pid`) or `JobScheduler`'s dispatch state (for
`current_job_id`), neither of which `WorkerPool` currently exposes. Rather than invent an ad
hoc cross-layer lookup not designed for this, `list()` returns `None` for both — documented
explicitly as a known gap, not silently guessed at.

No `sysinfo` dependency exists anywhere in the workspace yet (confirmed via `Cargo.lock`
grep). Phase 8's `keepalive.rs` (`crates/anvilml-worker/src/keepalive.rs`) is this
codebase's only precedent for an injected-duration, `tokio::time::interval`-based
periodic task; `stats_tick.rs` follows its `MissedTickBehavior::Delay` choice and its
doc-comment convention of stating the testability rationale for the injected parameter,
though as a plain `spawn_fn() -> JoinHandle<()>` (matching `spawn_event_loop()`'s own shape
in `anvilml-scheduler`) rather than `keepalive.rs`'s struct-with-`run(self)` shape — the
task context's own signature already specifies the simpler free-function form.

`crates/anvilml-scheduler/Cargo.toml` establishes the exact dev-dependency-override pattern
needed to reach `WorkerPool::set_up_test_workers()` (`#[cfg(feature = "test-utils")]`) from
an integration test file in a different crate — `anvilml-server`'s own `Cargo.toml` had no
such override before this task, since no prior `anvilml-server` test file needed to inject
fake worker handles.

## Resolved Dependencies

| Type  | Name     | Version verified | MCP source                                     | Feature flags confirmed |
|-------|----------|-------------------|--------------------------------------------------|---------------------------|
| crate | sysinfo  | 0.39.6 (latest stable) | crates.io sparse index (`index.crates.io/sy/si/sysinfo`) + downloaded source | default features (includes `component`, sufficient for `System::new_all()`/`refresh_cpu_usage()`/`refresh_memory()`/`global_cpu_usage()`/`used_memory()` on both Linux and Windows) |

No MCP tool was available in this session (see prior phase reports' identical note); the
crates.io sparse index and the downloaded `sysinfo-0.39.6` source were used as the live
source of truth. `System::new_all()`, `refresh_cpu_usage()`, `refresh_memory()`,
`global_cpu_usage() -> f32`, and `used_memory() -> u64` (bytes) were confirmed by inspecting
the actual source in `sysinfo-0.39.6/src/common/system.rs`, not recalled from training data.
`sysinfo`'s own documentation confirms `refresh_cpu_usage()` needs two calls at least
`MINIMUM_CPU_UPDATE_INTERVAL` (200ms) apart for a meaningful (non-zero) delta-based reading
— addressed in Approach.

## Approach

1. **Add `WorkerPool::list()`.** An `async fn(&self) -> Vec<WorkerInfo>` that zips
   `self.handles` and `self.devices` by position (safe per the struct's own documented
   invariant), awaiting `status()` per handle. `pid`/`current_job_id` are `None` — see
   Existing Codebase Assessment for why fabricating either would be worse than an explicit
   gap.

2. **Hold one `sysinfo::System` for the task's entire lifetime, not one per tick.**
   `refresh_cpu_usage()` computes a CPU-percentage delta against the *previous* refresh on
   the same `System` instance; a fresh `System::new_all()` every tick would have no prior
   sample and would report `0.0` on every single tick, not just the first. `System::new_all()`
   itself performs one initial refresh, so the loop's first `refresh_cpu_usage()` call
   already has a valid baseline as long as more than 200ms elapsed since construction —
   true unconditionally at the production 5s interval, and irrelevant for tests, which never
   assert an exact `cpu_pct` value (see Tests).

3. **`MissedTickBehavior::Delay`, matching `keepalive.rs`'s own choice and rationale.** If a
   tick is missed because a previous iteration ran long, the next tick fires one full
   `interval` after *now*, not several times back-to-back to "catch up" — a burst of stale
   `SystemStats` snapshots would actively mislead a connected client about current host
   state, which `Burst` mode would produce.

4. **`spawn_stats_tick()` as a plain free function returning `JoinHandle<()>`**, matching
   `spawn_event_loop()`'s shape (`crates/anvilml-scheduler/src/event_loop.rs`) rather than
   `keepalive.rs`'s `KeepaliveWatchdog::new(...).run()` struct shape — the task context's own
   signature already specifies this simpler form, and there is no second constructor
   parameter set here (unlike the watchdog's ping/pong split) that would benefit from a
   struct.

5. **Wire `main.rs` alongside `event_loop_handle`, not before or after unrelated code.**
   Placed immediately after `event_loop_handle`'s own construction (same section, same
   `Arc::clone(&broadcaster)`/`Arc::clone(&workers)` pattern) and its abort+await placed
   immediately after `event_loop_handle`'s own abort+await in the shutdown sequence — for
   the identical reason `main.rs`'s own existing comment already states about
   `event_loop_handle`: every task holding an `Arc<WorkerPool>` clone must release it before
   `Arc::try_unwrap(workers)` can succeed a few lines later.

6. **Bump patch versions** for every manifest whose source is modified, per
   `FORGE_AGENT_RULES.md`.

## Public API Surface

```rust
// crates/anvilml-server/src/ws/mod.rs
pub mod stats_tick;
pub use stats_tick::spawn_stats_tick;

// crates/anvilml-server/src/ws/stats_tick.rs
pub fn spawn_stats_tick(
    broadcaster: Arc<EventBroadcaster>,
    workers: Arc<WorkerPool>,
    interval: Duration,
) -> tokio::task::JoinHandle<()>;

// crates/anvilml-worker/src/pool.rs (impl WorkerPool)
pub async fn list(&self) -> Vec<anvilml_core::WorkerInfo>;
```

## Files Affected

| Action | Path | Description |
|--------|------|--------------|
| CREATE | `crates/anvilml-server/src/ws/stats_tick.rs` | `spawn_stats_tick()` |
| CREATE | `crates/anvilml-server/tests/stats_tick_tests.rs` | >=4 real-broadcaster integration tests |
| MODIFY | `crates/anvilml-server/src/ws/mod.rs` | Declares `stats_tick`, re-exports `spawn_stats_tick` |
| MODIFY | `crates/anvilml-server/Cargo.toml` | Adds `sysinfo`; expands `tokio` features; adds `anvilml-worker/test-utils` dev-dep; version bump |
| MODIFY | `crates/anvilml-worker/src/pool.rs` | Adds `WorkerPool::list()` |
| MODIFY | `crates/anvilml-worker/Cargo.toml` | Version bump only |
| MODIFY | `backend/src/main.rs` | Spawns and gracefully shuts down `spawn_stats_tick()` |
| MODIFY | `backend/Cargo.toml` | Version bump only |
| MODIFY | `docs/TESTS.md` | New catalogue entries |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|--------------------|------------------------|
| `crates/anvilml-server/tests/stats_tick_tests.rs` | `test_tick_publishes_system_stats` | A tick publishes `WsEvent::SystemStats`, observable by a pre-subscribed receiver | `cargo test -p anvilml-server --features mock-hardware --test stats_tick_tests test_tick_publishes_system_stats` exits 0 |
| `crates/anvilml-server/tests/stats_tick_tests.rs` | `test_workers_reflect_pool_state` | `SystemStats.workers` reflects two injected test workers' actual `status`/`device_index`/`device_type` | `cargo test -p anvilml-server --features mock-hardware --test stats_tick_tests test_workers_reflect_pool_state` exits 0 |
| `crates/anvilml-server/tests/stats_tick_tests.rs` | `test_two_consecutive_ticks_both_publish` | The task loops — two consecutive ticks each independently publish | `cargo test -p anvilml-server --features mock-hardware --test stats_tick_tests test_two_consecutive_ticks_both_publish` exits 0 |
| `crates/anvilml-server/tests/stats_tick_tests.rs` | `test_interval_parameter_controls_cadence` | The injected interval genuinely controls cadence, not a hardcoded 5s literal | `cargo test -p anvilml-server --features mock-hardware --test stats_tick_tests test_interval_parameter_controls_cadence` exits 0 |
| `crates/anvilml-server/tests/stats_tick_tests.rs` | `test_stats_are_real_data_not_the_c1_placeholder` | `ram_used_mib` is real nonzero `sysinfo` data, not `P16-C1`'s always-zero placeholder | `cargo test -p anvilml-server --features mock-hardware --test stats_tick_tests test_stats_are_real_data_not_the_c1_placeholder` exits 0 |

Combined acceptance, per the task's own criterion: `cargo test -p anvilml-server --features
mock-hardware --test stats_tick_tests` exits 0 (>=4 tests required; 5 delivered).