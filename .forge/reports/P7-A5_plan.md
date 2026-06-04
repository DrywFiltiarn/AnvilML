# Plan Report: P7-A5

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P7-A5                                             |
| Phase       | 007 — WebSocket Event Stream                      |
| Description | anvilml: start stats tick at startup; verify live WS stream |
| Depends on  | P7-A4                                              |
| Project     | anvilml                                            |
| Planned at  | 2026-06-04T18:30:00Z                               |
| Attempt     | 1                                                  |

## Objective

Wire the `system.stats` tick task (already implemented in P7-A4 as `spawn_system_stats_tick`) into `main.rs` so that it starts immediately after `AppState` is built and the server router is ready. This enables live WebSocket subscribers to receive `system.stats` JSON frames every ~5 seconds on `ws://127.0.0.1:8488/v1/events`.

## Scope

### In Scope
- Add `use anvilml_server::ws::stats_tick::spawn_system_stats_tick;` import in `backend/src/main.rs`.
- Call `spawn_system_stats_tick(state.clone())` after `AppState` is constructed and before the server bind, so the tick loop runs concurrently with the HTTP/WS listener.
- Manual verification: run `cargo run --features mock-hardware`, connect a WS client (`websocat` or browser console), confirm `system.stats` frames arrive every ~5 seconds with `event="system.stats"` and a timestamp.

### Out of Scope
- Modifying the tick interval, event payload shape, or broadcaster logic (already done in P7-A4).
- Adding tests for this wiring — the unit test in `stats_tick.rs` already exercises the broadcast path; integration-level WS stream verification is manual per the task description.
- CI changes, dependency upgrades, or config file modifications.
- Any code outside `backend/src/main.rs`.

## Approach

1. **Read** `backend/src/main.rs` to confirm the current structure: AppState is built at line ~161–168, router built at line ~169, then server binds at line ~172.
2. **Add import**: Insert `use anvilml_server::ws::stats_tick::spawn_system_stats_tick;` alongside the existing `anvilml_server` imports near the top of the file.
3. **Spawn the tick task**: After the `state` variable is constructed (line ~168) and before `build_router(state)` (line ~169), insert:
   ```rust
   spawn_system_stats_tick(state.clone());
   ```
   This clones the owned `AppState` for the spawned task while retaining the original for `build_router`. The tick task runs independently on the same Tokio runtime.
4. **Build check**: Run `cargo build -p backend --features mock-hardware` to confirm compilation succeeds.
5. **Manual verification**: Run `cargo run --features mock-hardware`, open a second shell, connect via `websocat ws://127.0.0.1:8488/v1/events`, and observe recurring `system.stats` JSON frames within 10 seconds.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Edit | `backend/src/main.rs` | Add import for `spawn_system_stats_tick` and call it after AppState construction |

No files created or deleted. No test files written (unit tests already exist in `stats_tick.rs`; integration verification is manual per task spec).

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `crates/anvilml-server/src/ws/stats_tick.rs` | `stats_tick_broadcasts_event` | The tick task broadcasts a `SystemStats` event with correct GPU/ram fields within one interval (6s timeout) |

The existing unit test in `stats_tick.rs` already covers the broadcast path. No new tests are added for this wiring task — the acceptance criterion is manual WS stream verification.

## CI Impact

No CI changes required. This task only modifies `main.rs` to call an existing function. No new dependencies, no config files, no CI workflow changes. The existing CI matrix (fmt, clippy, test with mock-hardware) will pass unchanged.

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Cloning `AppState` for the tick task may duplicate memory-intensive fields (e.g., hardware info). | `AppState::hardware()` returns a reference to an `Arc<HardwareInfo>` inside AppState — cloning the outer struct only clones Arc pointers, not data. Verified by reading `state.rs`. |
| Tick task starts before the WS server is bound, so the first few broadcasts may have zero subscribers and silently drop (expected). | This is correct behavior per the broadcaster design (`send` ignores `SendError`). No mitigation needed. |
| The spawned handle is not stored, so it cannot be explicitly cancelled on shutdown. | Acceptable for MVP: Tokio drops all tasks when the runtime shuts down (graceful shutdown via `shutdown::shutdown_signal()`). The task's interval loop will terminate naturally. |

## Acceptance Criteria

- [ ] `cargo build -p backend --features mock-hardware` exits 0
- [ ] `cargo run --features mock-hardware` starts and binds to port 8488 without panics or errors
- [ ] `websocat ws://127.0.0.1:8488/v1/events` receives a JSON frame with `"event":"system.stats"` within 10 seconds of connecting
- [ ] Subsequent frames arrive approximately every 5 seconds
- [ ] Each frame contains a valid ISO 8601 `timestamp` field and a `gpus` array
