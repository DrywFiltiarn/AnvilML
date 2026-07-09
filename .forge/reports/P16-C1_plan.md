# Plan Report: P16-C1

| Field       | Value                                                        |
|-------------|---------------------------------------------------------------|
| Task ID     | P16-C1                                                         |
| Phase       | 016 — Live Events                                               |
| Description | anvilml-server: ws_handler skeleton + initial SystemStats frame |
| Depends on  | P16-B1                                                          |
| Project     | anvilml                                                         |
| Planned at  | 2026-07-09T16:10:00Z                                            |
| Attempt     | 2                                                                |

## Objective

Create the `GET /v1/events` WebSocket upgrade handler in `anvilml-server`, establishing
the connection skeleton `ANVILML_DESIGN.md §13.6` specifies: on connect, subscribe to the
shared `EventBroadcaster` (wired into `AppState` by `P16-B1`), then immediately send the
current `SystemStats` as the connection's first frame. When this task completes, a
WebSocket client connecting to `ws://<host>:<port>/v1/events` receives exactly one JSON
text frame tagged `"type": "system_stats"` before the connection closes — the ongoing
forward loop that keeps the connection open and streams subsequent events is explicitly
out of scope, deferred to `P16-C2`.

**Prior attempt:** Attempt 1 (OpenCode / Qwen3.6 35B A3B) correctly identified that axum
0.8 has no built-in WebSocket test client and that `tokio-tungstenite` is the correct tool
for `handler_tests.rs`, but entered a repeating thinking loop re-deriving that same
conclusion without acting on it, and exhausted its session budget before writing any code.
This plan and the corresponding implementation supersede that attempt.

## Scope

### In Scope
- Create `crates/anvilml-server/src/ws/mod.rs` — declares and re-exports the `handler` module.
- Create `crates/anvilml-server/src/ws/handler.rs`:
  - `pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response`
    — delegates to `ws.on_upgrade(move |socket| handle_socket(socket, state))`.
  - `async fn handle_socket(mut socket: WebSocket, state: AppState)` — subscribes to
    `state.broadcaster` via `EventBroadcaster::subscribe()`, sends one placeholder/zero-valued
    `WsEvent::SystemStats { cpu_pct: 0.0, ram_used_mib: 0, workers: vec![] }` as a JSON text
    frame, then returns.
- Update `crates/anvilml-server/src/lib.rs` — add `pub mod ws;`, register
  `GET /v1/events → ws::ws_handler` in `build_router()`.
- Update `crates/anvilml-server/Cargo.toml`:
  - Enable axum's `ws` feature (gated, non-default — required for `axum::extract::ws`).
  - Add `tokio-tungstenite` and `futures-util` as dev-dependencies for a real-socket test client.
- Create `crates/anvilml-server/tests/handler_tests.rs` — >=3 tests using a real
  `TcpListener` + `axum::serve()` background task and a `tokio_tungstenite::connect_async()`
  client (see Approach for why `tower::ServiceExt::oneshot()` cannot be used here).
- Update `docs/TESTS.md` with one entry per new test, per `FORGE_AGENT_RULES.md §5.10`.

### Out of Scope
- The ongoing forward loop that consumes the subscription and forwards subsequent
  `WsEvent`s as JSON text frames — `P16-C2`.
- The `RecvError::Lagged` disconnect rule — `P16-C2`.
- Populating `SystemStats` with real `cpu_pct`/`ram_used_mib`/`workers` data — `P16-D1`
  (`stats_tick.rs`). This task's frame is explicitly placeholder/zero-valued, per the task
  context and `ANVILML_DESIGN.md §13.6`'s note that the periodic tick is a separate task.
- Any change to `backend/main.rs` — `AppState.broadcaster` already exists and is already
  wired to the scheduler's event loop as of `P16-B1`; this task only adds a route that reads
  the field already present on `AppState`.

## Existing Codebase Assessment

`AppState` (`crates/anvilml-server/src/state.rs`) already carries `broadcaster:
Arc<EventBroadcaster>` as of `P16-B1` (confirmed complete — `.forge/state/CURRENT_TASK.md`
shows `Task: P16-B1, Status: COMPLETE`), alongside `config`, `node_registry`, `start_time`,
`scheduler`, `workers`, `db`, and `artifact_store`. `EventBroadcaster` itself
(`crates/anvilml-ipc/src/ws/broadcaster.rs`, Phase 7's `P7-C1`) is a thin wrapper around
`tokio::sync::broadcast::Sender<WsEvent>` with a 1024-event buffer, re-exported directly as
`anvilml_ipc::EventBroadcaster` (`pub use ws::broadcaster::EventBroadcaster;` in
`anvilml-ipc/src/lib.rs`) — handler code imports it from there, not from a redefinition in
`anvilml-server`, matching `ANVILML_DESIGN.md §13.1`'s note that `ws/broadcaster.rs` in this
crate is "re-exported from anvilml-ipc, not redefined here" (this plan therefore does not
create that file).

`build_router()` (`crates/anvilml-server/src/lib.rs`) currently registers `/health`,
`/v1/jobs` (GET/POST), `/v1/jobs/{id}`, `/v1/nodes`, `/v1/artifacts`, and
`/v1/artifacts/{hash}`, wrapped in `CorsLayer::permissive()` and `.with_state(app_state)`.
Axum 0.8+ path-parameter syntax is `{param}`, not `:param` — the new route follows the
existing literal-path style (no parameters) already used by `/health` and `/v1/nodes`.

Every existing handler test file (`health_tests.rs`, `cors_tests.rs`, `jobs_tests.rs`,
`nodes_tests.rs`, `artifacts_tests.rs`) uses `tower::ServiceExt::oneshot()` against the
router returned by `build_router()`, entirely in-process with no real socket. That pattern
cannot be reused for this task: a WebSocket upgrade requires axum's `on_upgrade()` callback
to receive a genuine `hyper::upgrade::Upgraded` connection, which `oneshot()`'s in-memory
`tower::Service::call()` does not provide — the response would report `101 Switching
Protocols` but there is no real bidirectional stream behind it to read frames from. This is
a genuine gap in the established test convention, not a deviation from it; the plan
introduces a second, real-socket pattern (`spawn_test_server()`) specifically for this file.

No prior WebSocket handler code exists anywhere in the crate — `crates/anvilml-server/src/ws/`
does not yet exist on disk.

## Resolved Dependencies

| Type  | Name              | Version verified | MCP source                      | Feature flags confirmed |
|-------|-------------------|-------------------|----------------------------------|--------------------------|
| crate | axum (existing)   | 0.8.9 (already pinned) | crates.io sparse index + downloaded source | `ws` feature required — gated, non-default; pulls in `dep:tokio-tungstenite`, `dep:hyper`, `dep:sha1`, `dep:base64` internally for the server-side upgrade |
| crate | tokio-tungstenite | 0.29.0 (latest stable) | crates.io sparse index (`index.crates.io/to/ki/tokio-tungstenite`) | default features (`connect` + `handshake`) are sufficient — no TLS feature needed for a `ws://127.0.0.1` test client |
| crate | futures-util      | 0.3 (existing transitive version 0.3.32 in `Cargo.lock`, pinned loosely to match project convention for utility crates) | `Cargo.lock` | none — only `StreamExt` is used |

No MCP tool was available in this session (network-restricted sandbox, no `rust-docs` MCP
configured); the crates.io sparse index and the downloaded crate/axum source tarballs were
used instead as the live-version source of truth, per `FORGE_AGENT_RULES.md §6.4`'s fallback
guidance. `axum::extract::ws::Message::Text(Utf8Bytes)`, `Message::text<S: Into<Utf8Bytes>>`,
and `tokio_tungstenite::connect_async<R: IntoClientRequest>` signatures were confirmed by
inspecting the actual downloaded source of `axum-0.8.9` and `tungstenite-0.28.0`, not
recalled from training data.

## Approach

1. **Create `ws/mod.rs`.** A thin module declaration (`pub mod handler; pub use
   handler::ws_handler;`), matching the one-line-doc-comment-plus-declarations style used by
   `handlers/mod.rs`.

2. **Create `ws/handler.rs`.** `ws_handler()` is a pure delegation to
   `WebSocketUpgrade::on_upgrade()`, per `ANVILML_DESIGN.md §3.3`'s "no business logic in
   handler functions" — matching the task context's exact signature. `handle_socket()`
   subscribes to `state.broadcaster` *before* constructing or sending the initial frame, even
   though this task drops the resulting `Receiver` immediately — this ordering means `P16-C2`
   can later replace the `let _receiver = ...;` line with the actual forward loop and inherit
   a subscription that was already active before the first frame was sent, so no event
   published in that narrow window is silently lost once the loop exists. Constructing
   `WsEvent::SystemStats` inline with `cpu_pct: 0.0, ram_used_mib: 0, workers: vec![]` avoids
   a placeholder struct/constant that would need to be deleted again once `P16-D1` adds the
   real value source — the task context explicitly sanctions a literal zero-valued instance.

3. **Register the route in `lib.rs`.** Insert
   `.route("/v1/events", axum::routing::get(ws::ws_handler))` after the existing
   `/v1/artifacts/{hash}` route and before `.layer(CorsLayer::permissive())`, matching the
   existing ordering convention (routes first, then layers, then `.with_state()`).

4. **Enable axum's `ws` feature.** Confirmed via the downloaded `axum-0.8.9/Cargo.toml` that
   `extract::ws` is `#[cfg(feature = "ws")]`-gated and not part of any default feature set —
   without this, `axum::extract::ws` does not resolve at all. Changed
   `axum = "0.8.9"` → `axum = { version = "0.8.9", features = ["ws"] }`.

5. **Add test-only dependencies.** `tokio-tungstenite = "0.29.0"` and `futures-util = "0.3"`
   under `[dev-dependencies]` only — the handler itself needs no client-side WebSocket crate,
   only the test file does.

6. **Write `tests/handler_tests.rs`** using a `spawn_test_server()` helper that binds
   `TcpListener::bind("127.0.0.1:0")` for an OS-assigned ephemeral port, spawns
   `axum::serve(listener, build_router(state))` as a background task, and returns the
   `ws://` URL. This is necessary (not a stylistic choice) because `tower::ServiceExt::oneshot()`
   cannot complete a real WebSocket upgrade — see Existing Codebase Assessment. `make_test_state()`
   duplicates `health_tests.rs`'s in-memory-SQLite-plus-minimal-subsystem construction pattern
   rather than sharing a helper module, matching this crate's established
   self-contained-test-file convention.

7. **Update `docs/TESTS.md`.** One entry per new test, in the existing catalogue's exact
   heading/field format, appended after the file's current last entry.

## Public API Surface

```rust
// crates/anvilml-server/src/ws/mod.rs
pub mod handler;
pub use handler::ws_handler;

// crates/anvilml-server/src/ws/handler.rs
pub async fn ws_handler(
    ws: axum::extract::ws::WebSocketUpgrade,
    State(state): State<AppState>,
) -> axum::response::Response;
// handle_socket() is a private fn — not part of the public surface.
```

`build_router()`'s signature is unchanged (`pub fn build_router(app_state: AppState) ->
axum::Router`) — only its route table grows by one entry.

## Files Affected

| Action | Path | Description |
|--------|------|--------------|
| CREATE | `crates/anvilml-server/src/ws/mod.rs` | Module declaration, re-exports `ws_handler` |
| CREATE | `crates/anvilml-server/src/ws/handler.rs` | `ws_handler()` + `handle_socket()` (partial — initial frame only) |
| MODIFY | `crates/anvilml-server/src/lib.rs` | Adds `pub mod ws;`, registers `GET /v1/events` |
| MODIFY | `crates/anvilml-server/Cargo.toml` | Enables axum's `ws` feature; adds `tokio-tungstenite`/`futures-util` dev-deps |
| CREATE | `crates/anvilml-server/tests/handler_tests.rs` | 4 real-socket integration tests |
| MODIFY | `docs/TESTS.md` | 4 new catalogue entries, per `FORGE_AGENT_RULES.md §5.10` |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-------------------|----------------------|
| `crates/anvilml-server/tests/handler_tests.rs` | `test_connect_receives_initial_system_stats_frame` | Connecting yields exactly one message, tagged `"type": "system_stats"` | `cargo test -p anvilml-server --test handler_tests test_connect_receives_initial_system_stats_frame` exits 0 |
| `crates/anvilml-server/tests/handler_tests.rs` | `test_initial_frame_matches_ws_event_shape` | The frame round-trips through `WsEvent`'s `Deserialize` impl as `SystemStats` with zero-valued fields | `cargo test -p anvilml-server --test handler_tests test_initial_frame_matches_ws_event_shape` exits 0 |
| `crates/anvilml-server/tests/handler_tests.rs` | `test_handler_sends_exactly_one_frame_then_returns` | The second read off the socket is Close/error/EOF, never a second Text frame — pins the P16-C1/P16-C2 boundary | `cargo test -p anvilml-server --test handler_tests test_handler_sends_exactly_one_frame_then_returns` exits 0 |
| `crates/anvilml-server/tests/handler_tests.rs` | `test_multiple_clients_each_receive_independent_initial_frame` | `subscribe()` is called per-connection; two concurrent clients each get their own initial frame | `cargo test -p anvilml-server --test handler_tests test_multiple_clients_each_receive_independent_initial_frame` exits 0 |

Combined acceptance, per the task's own criterion: `cargo test -p anvilml-server --test
handler_tests` exits 0 (>=3 tests required; 4 delivered).