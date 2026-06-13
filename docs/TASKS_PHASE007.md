# Tasks: Phase 007 — WebSocket Event Stream

| Field | Value |
|-------|-------|
| Phase | 007 |
| Name | WebSocket Event Stream |
| Project | anvilml |
| Status | Approved |
| Depends on phases | 6 |

## Overview

Phase 007 implements the WebSocket event stream that clients use to observe server state in real time. The `GET /v1/events` endpoint upgrades HTTP connections to WebSocket, subscribes to the `EventBroadcaster`, and forwards each `WsEvent` as a JSON text frame.

A background task emits `WsEvent::SystemStats` every 5 seconds with current CPU usage, RAM usage, and worker summaries. At this phase there are no workers, so the `workers` array is empty — but the tick itself proves the infrastructure is working. Subsequent phases will populate the workers array as the worker pool is implemented.

Clients that fall behind (their receive buffer overflows the broadcast channel) are disconnected silently. The channel capacity is 1024 events.

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | anvilml-server | P7-A1 … P7-A3 | EventBroadcaster, WS handler, SystemStats tick |

## Prerequisites

Phase 006 complete. `WsEvent` type exists in `anvilml-core`.

## Task Descriptions

### Group A — anvilml-server

#### P7-A1: anvilml-server: EventBroadcaster

**Goal:** Implement `EventBroadcaster` in `ws/broadcaster.rs` wrapping a `tokio::sync::broadcast::Sender<WsEvent>` with capacity 1024. Provide `pub fn send(&self, event: WsEvent)` (logs WARN on lagged receiver) and `pub fn subscribe(&self) -> Receiver<WsEvent>`.

**Acceptance criterion:** `cargo test -p anvilml-server -- broadcaster` exits 0 with ≥ 3 tests.

#### P7-A2: anvilml-server: GET /v1/events WebSocket handler

**Goal:** Implement `ws_events` handler in `ws/handler.rs` using `axum::extract::ws::WebSocketUpgrade`. On upgrade: subscribe to `EventBroadcaster`, loop receiving events, serialize to JSON text frame, send. Close on send error. Log connection open/close at INFO with `remote_addr=`.

**Acceptance criterion:** `websocat ws://127.0.0.1:8488/v1/events` connects without error.

#### P7-A3: anvilml-server: SystemStats background tick task

**Goal:** Implement `ws/stats_tick.rs` with `pub fn start(broadcaster: Arc<EventBroadcaster>, workers: Arc<WorkerPool>)` — spawns a tokio task that sends `WsEvent::SystemStats` every 5 seconds with `sysinfo` CPU/RAM readings. `workers` is `Arc<()>` placeholder for now; update when `WorkerPool` exists.

**Acceptance criterion:** `websocat ws://127.0.0.1:8488/v1/events` receives `{"type":"system_stats",...}` frames every 5 seconds.

## Phase Acceptance Criteria

```bash
cargo run --features mock-hardware &
sleep 2
timeout 12 websocat ws://127.0.0.1:8488/v1/events | head -3 | python3 -c "import sys,json; [json.loads(l) for l in sys.stdin]"
kill %1
```

## Known Constraints and Gotchas

- `axum` WebSocket requires the `ws` feature. Add to workspace dep.
- `sysinfo` crate provides CPU and memory readings cross-platform. Add as workspace dep.
- The `stats_tick` start function signature will change in Phase 009 when `WorkerPool` is introduced. Use `Arc<()>` as a placeholder parameter now; the Forge will update the signature in a Phase 009 task with a retrofit task if the signature change touches multiple files.
