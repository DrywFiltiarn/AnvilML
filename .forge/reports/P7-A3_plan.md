# Plan Report: P7-A3

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P7-A3                                       |
| Phase       | 007 — WebSocket Event Stream                |
| Description | anvilml-server: SystemStats background tick task |
| Depends on  | P7-A1, P7-A2                                |
| Project     | anvilml                                     |
| Planned at  | 2026-06-16T09:30:00Z                        |
| Attempt     | 1                                           |

## Objective

Implement the `stats_tick` module in `crates/anvilml-server/src/ws/stats_tick.rs` and wire it into `backend/src/main.rs` so that the running server periodically emits `WsEvent::SystemStats` events (every 5 seconds) containing CPU utilisation percentage and RAM usage in mebibytes, broadcast to all WebSocket subscribers at `GET /v1/events`. An observer connecting via `websocat ws://127.0.0.1:8488/v1/events` will receive a `{"type":"system_stats",...}` JSON text frame within 6 seconds of connecting.

## Scope

### In Scope
- **`crates/anvilml-server/src/ws/stats_tick.rs`**: Implement `pub fn start(broadcaster: Arc<EventBroadcaster>)` — spawns a tokio task that loops every 5 seconds, reads CPU % and RAM MiB via the `sysinfo` crate, and broadcasts a `WsEvent::SystemStats{cpu_pct, ram_used_mib, workers: vec![]}`.
- **`crates/anvilml-server/Cargo.toml`**: Add `sysinfo = "0.33"` dependency (same version as `anvilml-hardware`).
- **`backend/src/main.rs`**: Call `anvilml_server::ws::stats_tick::start(state.broadcaster.clone())` after the server bind log line and before `axum::serve()`.
- **`crates/anvilml-server/tests/stats_tick_tests.rs`**: Unit tests verifying the broadcast channel receives `SystemStats` events with correct field types.
- **`crates/anvilml-server/Cargo.toml`**: Bump patch version `0.1.11 → 0.1.12`.

### Out of Scope
- Populating the `workers` array in `SystemStats` (deferred to Phase 009 when `WorkerPool` exists).
- Making the tick interval configurable (hardcoded to 5s per task spec).
- Modifying the existing `broadcaster.rs` or `handler.rs`.
- Integration tests that require a running server (covered by phase-level Runnable Proof).

## Existing Codebase Assessment

The `anvilml-server` crate already has the WebSocket infrastructure in place: `EventBroadcaster` (P7-A1) wraps a `tokio::sync::broadcast::Sender<WsEvent>` with capacity 1024, and the WebSocket handler (P7-A2) subscribes and forwards events as JSON text frames. The `stats_tick.rs` file exists but contains only a `pub fn _stub() {}` placeholder.

The `sysinfo` crate is already a dependency of `anvilml-hardware` at version `0.33` (locked at `0.33.1` in `Cargo.lock`). The existing `CpuDetector` in `anvilml-hardware/src/cpu.rs` demonstrates the API: `System::new_all()`, `sys.refresh_all()`, `sys.total_memory()`, and `sys.cpus()`. The docs.rs page for sysinfo 0.33.1 confirms the methods needed for this task: `sys.global_cpu_usage()` (returns `f64`) and `sys.used_memory()` (returns `u64` in bytes).

The established patterns in this crate include: `tracing` for logging at INFO/DEBUG levels, `serde_json` for JSON serialization (handled by `WsEvent`'s `Serialize` derive), and tests in `crates/{name}/tests/` as separate test crates using the public API.

## Resolved Dependencies

| Type   | Name       | Version verified | MCP source     | Feature flags confirmed |
|--------|------------|-----------------|----------------|------------------------|
| crate  | sysinfo    | 0.33            | docs.rs MCP    | n/a                    |

Note: `sysinfo` version `0.33` is already in use by `anvilml-hardware` (locked at `0.33.1`). The API shape was verified via docs.rs for sysinfo 0.33.1: `System::new_all()`, `sys.refresh_all()`, `sys.global_cpu_usage()`, `sys.used_memory()` are all confirmed present.

## Approach

1. **Add `sysinfo` dependency to `crates/anvilml-server/Cargo.toml`**: Add `sysinfo = "0.33"` under `[dependencies]`. This is the same version already used by `anvilml-hardware`, ensuring consistent crate resolution across the workspace.

2. **Implement `stats_tick.rs`**: Replace the stub with a complete implementation:
   - Import `sysinfo::System` and `anvilml_core::types::WsEvent`.
   - Implement `pub fn start(broadcaster: Arc<EventBroadcaster>)` that:
     a. Enters an infinite `async loop`.
     b. Sleeps for 5 seconds using `tokio::time::sleep(Duration::from_secs(5)).await`.
     c. Creates a fresh `System` via `System::new_all()` (equivalent to `System::new()` + `refresh_all()`, which is what `cpu.rs` already does).
     d. Reads `cpu_pct` via `sys.global_cpu_usage()` — this returns an `f64` representing total CPU utilisation across all cores as a percentage (0.0–N*100 where N is core count; the task spec says `cpu_pct: f32` in `WsEvent::SystemStats`, so we cast to `f32`).
     e. Reads `ram_used_mib` via `sys.used_memory() / (1024 * 1024)` — converts bytes to mebibytes, matching the pattern already used in `cpu.rs` for total memory.
     f. Constructs `WsEvent::SystemStats { cpu_pct, ram_used_mib, workers: vec![] }` and calls `broadcaster.send(event)`.
     g. Logs the tick at DEBUG level with `cpu_pct=` and `ram_used_mib=` fields per the logging conventions.
     h. If `broadcaster.send()` fails (all receivers lagged), logs WARN (this is already handled by `EventBroadcaster::send()` internally, so no additional error handling is needed in the tick loop).
   - Add `///` doc comment on `start()` describing its purpose and behavior.

3. **Wire `start()` into `backend/src/main.rs`**: After the `tracing::info!(addr = %actual_addr, "listening");` line and before `axum::serve(listener, router)`, add:
   ```rust
   anvilml_server::ws::stats_tick::start(state.broadcaster.clone());
   ```
   This spawns the background tick task after the server is bound and listening but before accepting connections, ensuring events start flowing immediately for any client that connects.

4. **Write tests**: Create `crates/anvilml-server/tests/stats_tick_tests.rs` with:
   - `test_stats_tick_broadcasts_system_stats`: Create a broadcaster, call `start()`, subscribe, wait 6 seconds, verify a `SystemStats` event was received with correct field types.
   - `test_stats_tick_cpu_pct_is_finite`: Verify the CPU percentage value is a finite f32 (not NaN or infinity).
   - `test_stats_tick_ram_used_mib_is_non_negative`: Verify RAM usage is always non-negative.

5. **Bump crate version**: Update `crates/anvilml-server/Cargo.toml` patch version from `0.1.11` to `0.1.12`.

## Public API Surface

| Item | Path | Signature |
|------|------|-----------|
| Function | `anvilml_server::ws::stats_tick` | `pub fn start(broadcaster: Arc<EventBroadcaster>)` |

No new public types or structs are introduced. The function returns `()` (the tokio task is fire-and-spawned, its JoinHandle is dropped — this is acceptable for a background task that runs until the broadcaster is shut down with the server).

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-server/src/ws/stats_tick.rs` | Replace stub with full tick implementation |
| Modify | `crates/anvilml-server/Cargo.toml` | Add `sysinfo = "0.33"` dependency; bump version to `0.1.12` |
| Modify | `backend/src/main.rs` | Call `stats_tick::start()` after server bind |
| CREATE   | `crates/anvilml-server/tests/stats_tick_tests.rs` | Unit tests for the tick task |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-server/tests/stats_tick_tests.rs` | `test_stats_tick_broadcasts_system_stats` | The tick task broadcasts a `SystemStats` event within 6 seconds | Broadcaster exists, `start()` called | None | A `WsEvent::SystemStats` event received on subscriber | `cargo test -p anvilml-server --features mock-hardware -- stats_tick` exits 0 |
| `crates/anvilml-server/tests/stats_tick_tests.rs` | `test_stats_tick_cpu_pct_is_finite` | CPU percentage is a finite f32 (not NaN/infinity) | Broadcaster exists, `start()` called | None | Event received with `cpu_pct.is_finite() == true` | `cargo test -p anvilml-server --features mock-hardware -- stats_tick` exits 0 |
| `crates/anvilml-server/tests/stats_tick_tests.rs` | `test_stats_tick_ram_used_mib_is_non_negative` | RAM usage in MiB is non-negative | Broadcaster exists, `start()` called | None | Event received with `ram_used_mib >= 0` | `cargo test -p anvilml-server --features mock-hardware -- stats_tick` exits 0 |

## CI Impact

No CI changes required. The new test file lives under `crates/anvilml-server/tests/` which is already picked up by `cargo test --workspace --features mock-hardware` (the rust-linux and rust-windows CI jobs). Adding `sysinfo` as a dependency does not change any CI job's behaviour.

## Platform Considerations

None identified. The `sysinfo` crate is cross-platform and the APIs used (`System::new_all()`, `global_cpu_usage()`, `used_memory()`) are available on all supported platforms (Linux, Windows). No `#[cfg(unix)]` or `#[cfg(windows)]` guards are required. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `sysinfo::System::new_all()` includes process data which may be slow on first call, blocking the tokio task for a noticeable duration on first tick. | Low | Medium | Use `System::new()` + `sys.refresh_memory()` + `sys.refresh_cpu_usage()` instead of `refresh_all()` to only refresh the data we need. This avoids the process enumeration overhead. |
| `global_cpu_usage()` returns inaccurate values on the first call (per sysinfo docs: "the result will very likely be inaccurate at the first call"). | Low | Low | The first tick will produce a potentially inaccurate CPU % value. This is acceptable — the task spec says "every 5s" and the first reading after startup is a known cold-start artifact. Subsequent readings will be accurate. Document this in an inline comment. |
| `used_memory()` returns bytes, not MiB. Incorrect division would produce wrong RAM values. | Low | Medium | Use the same conversion pattern already established in `anvilml-hardware/src/cpu.rs`: `sys.used_memory() / (1024 * 1024)`. This is verified working in the existing codebase. |

## Acceptance Criteria

- [ ] `cargo check --workspace --features mock-hardware` exits 0
- [ ] `cargo test -p anvilml-server --features mock-hardware -- stats_tick` exits 0 with ≥ 3 tests
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
- [ ] `cargo fmt --all -- --check` exits 0
- [ ] `head -1 crates/anvilml-server/src/ws/stats_tick.rs | grep -q "^//!"` confirms crate-level doc comment exists
- [ ] `grep "pub fn start" crates/anvilml-server/src/ws/stats_tick.rs` confirms the public function signature exists
- [ ] `grep "stats_tick::start" backend/src/main.rs` confirms the call site exists after the bind log line
