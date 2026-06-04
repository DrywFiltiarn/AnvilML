# Tasks: Phase 007 — WebSocket Event Stream

| Field | Value |
|-------|-------|
| Phase | 007 |
| Name | WebSocket Event Stream |
| Milestone group | Observable system state |
| Depends on phases | 1-6 |
| Task file | `forge/tasks/tasks_phase007.json` |
| Tasks | 6 |

## Overview

Phase 7 adds the `EventBroadcaster`, the `GET /v1/events` WebSocket endpoint with 30s keepalive ping and lag-disconnect, and the 5-second `system.stats` tick. After this phase a client can subscribe to the live event stream and watch system statistics arrive every five seconds — the first real-time surface of the application.

Every task in this phase implements **one module or one endpoint** plus its test. No task touches more than its named file(s). `cargo test` and `cargo clippy` are per-task gates; the phase as a whole is only complete when the **Runnable Proof** below passes.

## Tasks

| Task | Module / File | Summary |
|------|-------------|---------|
| P7-A1 | `crates/anvilml-server/src/ws/broadcaster.rs` | anvilml-server: EventBroadcaster |
| P7-A2 | `crates/anvilml-server/src/ws/handler.rs` | anvilml-server: WebSocket /v1/events handler |
| P7-A3 | `crates/anvilml-server/src/ws/handler.rs` | anvilml-server: WS keepalive ping every 30s |
| P7-A4 | `crates/anvilml-server/src/ws/stats_tick.rs` | anvilml-server: system.stats tick task (5s broadcast) |
| P7-A5 | `backend/src/main.rs` | anvilml: start stats tick at startup; verify live WS stream |
| P7-B1 | `.github/workflows/ci.yml` | anvilml: add real-hardware lint steps to rust-linux and rust-windows CI jobs |

## Task details

### Group A — WebSocket Event Stream

#### P7-A1: anvilml-server: EventBroadcaster

- **Prereqs:** P6-A7
- **Tags:** —

Create src/ws/broadcaster.rs: EventBroadcaster{sender: tokio::sync::broadcast::Sender<Arc<WsEvent>>}. new(capacity:usize). fn send(&self, event:WsEvent) wrapping in Arc and ignoring SendError (no subscribers is fine). fn subscribe(&self)->broadcast::Receiver<Arc<WsEvent>>. cargo test -p anvilml-server -- broadcaster exits 0: subscribe, send, receive equal event; send with no subscribers does not error.

#### P7-A2: anvilml-server: WebSocket /v1/events handler

- **Prereqs:** P7-A1
- **Tags:** reasoning

Add broadcaster: Arc<EventBroadcaster> to AppState (capacity from cfg.limits.ws_broadcast_capacity). Create src/ws/handler.rs: ws_events(WebSocketUpgrade, State)->on_upgrade. On connect subscribe; forward each Arc<WsEvent> as Message::Text(json). On RecvError::Lagged close with code 1008. No history replay. Wire GET /v1/events. Add tokio-tungstenite dev-dep. cargo test -p anvilml-server --features mock-hardware -- ws exits 0: connect, broadcast a test event, assert received as JSON text.

#### P7-A3: anvilml-server: WS keepalive ping every 30s

- **Prereqs:** P7-A2
- **Tags:** —

In ws/handler.rs add a ping task: tokio::time::interval(30s) sending Message::Ping(vec![]) to the socket; on send error end the connection. Run alongside the broadcast-forward task via tokio::select! so either ending closes the socket. cargo test -p anvilml-server --features mock-hardware -- ws still exits 0 (no regression).

#### P7-A4: anvilml-server: system.stats tick task (5s broadcast)

- **Prereqs:** P7-A3
- **Tags:** —

Create src/ws/stats_tick.rs: spawn_system_stats_tick(state)->JoinHandle. Every 5s build SystemStatsEvent: per-device vram from AppState.hardware (used 0 until worker reports exist), host ram via sysinfo; broadcaster.send(WsEvent::SystemStats). Call it from build_router setup or main startup. Verify via next task.

#### P7-A5: anvilml: start stats tick at startup; verify live WS stream

- **Prereqs:** P7-A4
- **Tags:** —

In main.rs after AppState built, call spawn_system_stats_tick(state.clone()). Ensure broadcaster + tick are live for the bound server. Verify: cargo run --features mock-hardware, then in another shell `websocat ws://127.0.0.1:8488/v1/events` (or a browser WS console) shows a system.stats JSON frame arriving every ~5 seconds with event='system.stats' and a timestamp.

---

### Group B — CI Hardening

#### P7-B1: anvilml: add real-hardware lint steps to rust-linux and rust-windows CI jobs

- **Prereqs:** P7-A5
- **Tags:** —

The real-hardware code paths (`#[cfg(unix)]` and `#[cfg(windows)]` branches in `anvilml-hardware`) are never seen by `cargo clippy --workspace --features mock-hardware`. Warnings in those paths — such as the `unused_mut` on `vulkan::VulkanDetector` — only surface at manual run time, not during any automated lint step. P6-B2 added a real-hardware compile check to both CI jobs; this task adds the corresponding clippy pass immediately after it, completing the CI hardening gap.

Both jobs in `.github/workflows/ci.yml` receive a new step placed immediately after their existing `Real-hardware compile check` step:

```yaml
- name: Real-hardware lint
  run: cargo clippy --bin anvilml -- -D warnings
```

No `--features` flag on either. On `rust-linux` (`ubuntu-latest`) this lints the `#[cfg(unix)]` paths natively. On `rust-windows` (`windows-latest`, native MSVC toolchain) this lints the `#[cfg(windows)]` paths. All existing jobs and steps are preserved unchanged; this task inserts only, it does not reorder or alter any existing step.

**Files to create or modify:**
- `.github/workflows/ci.yml` — add `Real-hardware lint` step to both `rust-linux` and `rust-windows` jobs, each placed immediately after their existing `Real-hardware compile check` step

**Key implementation notes:**
- Placement after `Real-hardware compile check` is mandatory. If the real-hardware paths do not compile (caught by the preceding step), clippy must not run — a failed compile step will already halt the job before the lint step is reached.
- Per `FORGE_AGENT_RULES §3.7`, CI workflow files may only be modified when explicitly listed in the task's Files Affected table — which this task does.
- Do not alter any existing step name, command, or position.
- Do not add the step to any job other than `rust-linux` and `rust-windows`.

**Acceptance criterion:** `grep -c 'Real-hardware lint' .github/workflows/ci.yml` prints `2`.

## Runnable Proof

Subscribe to the WebSocket and watch `system.stats` frames arrive.

```bash
cargo run --features mock-hardware
# another terminal (install websocat, or use a browser WS console):
websocat ws://127.0.0.1:8488/v1/events
```

Expected: roughly every 5 seconds a JSON text frame arrives with `"event":"system.stats"`, a `timestamp`, a `gpus` array, and `ram_used_mib`/`ram_total_mib`. The connection stays open (30s pings keep it alive). Phase done when a subscriber observes recurring `system.stats` frames and `cargo test -p anvilml-server --features mock-hardware` is green.

## Known Constraints and Gotchas

- P7-B1 modifies `.github/workflows/ci.yml`. Per `FORGE_AGENT_RULES §3.7` this is only permitted because the file is explicitly listed in that task's Files Affected table.
- The real-hardware lint step must be placed after `Real-hardware compile check`, not before. If the real-hardware paths fail to compile, the compile check halts the job before clippy runs — this is intentional ordering.
- P7-B1 must run after P7-A5 to avoid disrupting the in-progress WebSocket implementation chain.