# Tasks: Phase 007 — WebSocket Event Stream

| Field | Value |
|-------|-------|
| Phase | 007 |
| Name | WebSocket Event Stream |
| Milestone group | Observable system state |
| Depends on phases | 1-6 |
| Task file | `.forge/tasks/tasks_phase007.json` |
| Tasks | 7 |

## Overview

Phase 7 adds the `EventBroadcaster`, the `GET /v1/events` WebSocket endpoint with 30s keepalive ping and lag-disconnect, and the 5-second `system.stats` tick. After this phase a client can subscribe to the live event stream and watch system statistics arrive every five seconds — the first real-time surface of the application.

Group B locks in real-hardware lint coverage in CI, mirroring the real-hardware compile check added in P6-B2. Group C eliminates the version-authority gap by introducing `[workspace.dependencies]` and upgrading all external dependencies to their current stable releases via MCP lookup.

Every task in this phase implements one module, one endpoint, or one infrastructure change plus its verification. No task touches more than its named file(s). `cargo test` and `cargo clippy` are per-task gates; the phase as a whole is only complete when the Runnable Proof below passes.

## Tasks

| Task | Module / File | Summary |
|------|-------------|---------|
| P7-A1 | `crates/anvilml-server/src/ws/broadcaster.rs` | anvilml-server: EventBroadcaster |
| P7-A2 | `crates/anvilml-server/src/ws/handler.rs` | anvilml-server: WebSocket /v1/events handler |
| P7-A3 | `crates/anvilml-server/src/ws/handler.rs` | anvilml-server: WS keepalive ping every 30s |
| P7-A4 | `crates/anvilml-server/src/ws/stats_tick.rs` | anvilml-server: system.stats tick task (5s broadcast) |
| P7-A5 | `backend/src/main.rs` | anvilml: start stats tick at startup; verify live WS stream |
| P7-B1 | `.github/workflows/ci.yml` | anvilml: add real-hardware lint steps to rust-linux and rust-windows CI jobs |
| P7-C1 | `Cargo.toml`, all per-crate `Cargo.toml` files | anvilml: introduce [workspace.dependencies] and upgrade all external deps to current stable |

## Task details

### Group A — WebSocket Event Stream

#### P7-A1: anvilml-server: EventBroadcaster

- **Prereqs:** P6-A7
- **Tags:** —

Create src/ws/broadcaster.rs: EventBroadcaster{sender: tokio::sync::broadcast::Sender<Arc<WsEvent>>}. new(capacity:usize). fn send(&self, event:WsEvent) wrapping in Arc and ignoring SendError (no subscribers is fine). fn subscribe(&self)->broadcast::Receiver<Arc<WsEvent>>. cargo test -p anvilml-server -- broadcaster exits 0: subscribe, send, receive equal event; send with no subscribers does not error.

**Acceptance criterion:** `cargo test -p anvilml-server -- broadcaster` exits 0.

---

#### P7-A2: anvilml-server: WebSocket /v1/events handler

- **Prereqs:** P7-A1
- **Tags:** reasoning

Add broadcaster: Arc<EventBroadcaster> to AppState (capacity from cfg.limits.ws_broadcast_capacity). Create src/ws/handler.rs: ws_events(WebSocketUpgrade, State)->on_upgrade. On connect subscribe; forward each Arc<WsEvent> as Message::Text(json). On RecvError::Lagged close with code 1008. No history replay. Wire GET /v1/events. Add tokio-tungstenite dev-dep — use mcp-rust-docs to resolve the current stable version compatible with axum 0.7 (tungstenite 0.23 series) before writing the version to Cargo.toml. cargo test -p anvilml-server --features mock-hardware -- ws exits 0: connect, broadcast a test event, assert received as JSON text.

**Acceptance criterion:** `cargo test -p anvilml-server --features mock-hardware -- ws` exits 0.

---

#### P7-A3: anvilml-server: WS keepalive ping every 30s

- **Prereqs:** P7-A2
- **Tags:** —

In ws/handler.rs add a ping task: tokio::time::interval(30s) sending Message::Ping(vec![]) to the socket; on send error end the connection. Run alongside the broadcast-forward task via tokio::select! so either ending closes the socket. cargo test -p anvilml-server --features mock-hardware -- ws still exits 0 (no regression).

**Acceptance criterion:** `cargo test -p anvilml-server --features mock-hardware -- ws` exits 0 with no regressions.

---

#### P7-A4: anvilml-server: system.stats tick task (5s broadcast)

- **Prereqs:** P7-A3
- **Tags:** —

Create src/ws/stats_tick.rs: spawn_system_stats_tick(state)->JoinHandle. Every 5s build SystemStatsEvent: per-device vram from AppState.hardware (used 0 until worker reports exist), host ram via sysinfo; broadcaster.send(WsEvent::SystemStats). Call it from build_router setup or main startup. Verify via next task.

**Acceptance criterion:** `cargo build --features mock-hardware` exits 0 (runtime verification deferred to P7-A5).

---

#### P7-A5: anvilml: start stats tick at startup; verify live WS stream

- **Prereqs:** P7-A4
- **Tags:** —

In main.rs after AppState built, call spawn_system_stats_tick(state.clone()). Ensure broadcaster + tick are live for the bound server. Verify: cargo run --features mock-hardware, then in another shell `websocat ws://127.0.0.1:8488/v1/events` (or a browser WS console) shows a system.stats JSON frame arriving every ~5 seconds with event='system.stats' and a timestamp.

**Acceptance criterion:** `websocat ws://127.0.0.1:8488/v1/events` receives a `system.stats` JSON frame within 10 seconds of connecting.

---

### Group B — CI Hardening

#### P7-B1: anvilml: add real-hardware lint steps to rust-linux and rust-windows CI jobs

- **Prereqs:** P7-A5
- **Tags:** —

The real-hardware code paths (`#[cfg(unix)]` and `#[cfg(windows)]` branches in `anvilml-hardware`) are never seen by `cargo clippy --workspace --features mock-hardware`. Warnings in those paths only surface at manual run time. P6-B2 added a real-hardware compile check to both CI jobs; this task adds the corresponding clippy pass immediately after it, closing the lint gap.

Both jobs receive a new step placed immediately after their existing `Real-hardware compile check` step:

```yaml
- name: Real-hardware lint
  run: cargo clippy --bin anvilml -- -D warnings
```

No `--features` flag on either. On `rust-linux` this lints `#[cfg(unix)]` paths natively. On `rust-windows` (native MSVC) it lints `#[cfg(windows)]` paths. All existing jobs and steps are preserved unchanged.

**Files to create or modify:**
- `.github/workflows/ci.yml` — add `Real-hardware lint` step to both `rust-linux` and `rust-windows` jobs, each placed immediately after their existing `Real-hardware compile check` step

**Key implementation notes:**
- Placement after `Real-hardware compile check` is mandatory — a compile failure in the preceding step halts the job before lint runs, which is correct ordering.
- Per `FORGE_AGENT_RULES §3.7`, CI workflow files may only be modified when explicitly listed in the task's Files Affected table.
- Do not add the step to any job other than `rust-linux` and `rust-windows`.

**Acceptance criterion:** `grep -c 'Real-hardware lint' .github/workflows/ci.yml` prints `2`.

---

### Group C — Dependency Governance

#### P7-C1: anvilml: introduce [workspace.dependencies] and upgrade all external deps to current stable

- **Prereqs:** P7-B1
- **Tags:** reasoning

The workspace root `Cargo.toml` has no `[workspace.dependencies]` table. Every crate carries its own version strings independently, scattered across nine `Cargo.toml` files with bare major-version constraints. This makes PLAN sessions the effective authority on dependency versions — a PLAN session can write any version it likes and ACT has historically deferred to it rather than the MCP lookup result. The consequence is stale or mismatched pins such as the `tokio-tungstenite`/axum 0.7 mismatch in P7-A2.

This task establishes a single authoritative location for all external dependency versions:

1. Add `[workspace.dependencies]` to root `Cargo.toml`, populated by querying `mcp-rust-docs` for the current stable version of every external dependency currently declared anywhere in the workspace. Use those current stable versions, not the versions currently in any per-crate `Cargo.toml`.
2. Update every per-crate `Cargo.toml` to reference shared external dependencies via `{ workspace = true }`, removing the now-redundant inline version strings. Features that are crate-specific must be declared at the crate level alongside `workspace = true` (e.g. `tokio = { workspace = true, features = ["macros"] }`).
3. Internal path dependencies (`anvilml-core`, `anvilml-hardware`, etc.) are unchanged.
4. Dev-dependencies that appear in more than one crate move to `[workspace.dependencies]`; those in only one crate may stay local.

If an MCP lookup returns a version that is semver-incompatible with existing code (e.g. a breaking major bump), pin the last compatible major version, document it under `## Blockers` with the incompatibility described, and stop. A follow-on retrofit leaf task will be authored to perform that specific migration.

**Files to create or modify:**
- `Cargo.toml` — add `[workspace.dependencies]` with current stable versions of all shared external deps
- `backend/Cargo.toml` — migrate to `{ workspace = true }` references
- `crates/anvilml-core/Cargo.toml` — migrate to `{ workspace = true }` references
- `crates/anvilml-hardware/Cargo.toml` — migrate to `{ workspace = true }` references
- `crates/anvilml-registry/Cargo.toml` — migrate to `{ workspace = true }` references
- `crates/anvilml-ipc/Cargo.toml` — migrate to `{ workspace = true }` references
- `crates/anvilml-worker/Cargo.toml` — migrate to `{ workspace = true }` references
- `crates/anvilml-scheduler/Cargo.toml` — migrate to `{ workspace = true }` references
- `crates/anvilml-server/Cargo.toml` — migrate to `{ workspace = true }` references
- `crates/anvilml-openapi/Cargo.toml` — migrate to `{ workspace = true }` references

**Key implementation notes:**
- Query `mcp-rust-docs` for every external dependency before writing any version number. Record every lookup and its result in `## Resolved Dependencies`.
- `Cargo.lock` will be regenerated by cargo — commit the updated lock file.
- The `mock-hardware` feature flag chain is expressed via crate-level `[features]`, not `[workspace.dependencies]` — leave it unchanged.

**Acceptance criterion:** `cargo build --workspace --features mock-hardware` exits 0 AND `cargo test --workspace --features mock-hardware` exits 0 AND `grep -c 'workspace = true' Cargo.toml` is greater than 0.

---

## Runnable Proof

Subscribe to the WebSocket and watch `system.stats` frames arrive.

```bash
cargo run --features mock-hardware
# another terminal:
websocat ws://127.0.0.1:8488/v1/events
```

Expected: roughly every 5 seconds a JSON text frame arrives with `"event":"system.stats"`, a `timestamp`, a `gpus` array, and `ram_used_mib`/`ram_total_mib`. The connection stays open (30s pings keep it alive). Phase done when a subscriber observes recurring `system.stats` frames, `cargo test -p anvilml-server --features mock-hardware` is green, `grep -c 'Real-hardware lint' .github/workflows/ci.yml` prints `2`, and `grep -c 'workspace = true' Cargo.toml` is greater than 0.

## Known Constraints and Gotchas

- P7-A2 must resolve `tokio-tungstenite` via `mcp-rust-docs` before pinning the version. The compatible version for axum 0.7 is the tungstenite 0.23 series (e.g. tokio-tungstenite 0.24). Do not copy the version from training data.
- P7-B1 modifies `.github/workflows/ci.yml`. Per `FORGE_AGENT_RULES §3.7` this is only permitted because the file is explicitly listed in that task's Files Affected table. The lint step must be placed after `Real-hardware compile check` — a compile failure in the preceding step halts the job before lint runs.
- P7-B1 must run after P7-A5 to avoid disrupting the in-progress WebSocket implementation chain.
- P7-C1 will regenerate `Cargo.lock` — the updated lock file must be staged and committed. Do not suppress lock file changes.
- P7-C1 uses `mcp-rust-docs` as the authoritative version source. Any version written into `[workspace.dependencies]` without a prior MCP lookup is a protocol violation. If the MCP server is unavailable, set `Status=BLOCKED` per `FORGE_AGENT_RULES §6.4`.
- After P7-C1, all future tasks that add a new external dependency MUST add it to `[workspace.dependencies]` first and reference it via `{ workspace = true }` in the per-crate `Cargo.toml`. Adding an inline version string to a per-crate `Cargo.toml` is a drift violation.
- If P7-C1 encounters a semver-incompatible major bump, it pins the last compatible major and stops. A manually authored follow-on retrofit leaf task (e.g. a new Group D task — not pre-authored; created only if needed) handles the migration. The retrofit task's `context` field must open with an explicit origin reference: `"P7-C1 pinned <crate> at <old version> due to semver incompatibility. Migrate to <new version>: ..."`.