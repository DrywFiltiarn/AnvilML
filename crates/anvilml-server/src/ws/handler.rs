//! `GET /v1/events` WebSocket upgrade handler.
//!
//! Per `ANVILML_DESIGN.md §13.6`: on connect, (1) subscribe to the shared
//! `EventBroadcaster`, (2) send the current `SystemStats` immediately, then
//! (3) forward all subsequent `WsEvent`s as JSON text frames until the
//! consumer falls more than 1024 events behind, at which point the
//! connection is closed rather than caught up.
//!
//! This task (`P16-C1`) implements steps (1) and (2) only, establishing the
//! connection's structure and its exact first frame. Step (3) — the ongoing
//! forward loop and the `Lagged`-disconnect rule — is `P16-C2`'s scope;
//! `handle_socket()` intentionally returns immediately after the initial
//! send, dropping the freshly created subscription.

use anvilml_core::WsEvent;
use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::Response;

use crate::AppState;

/// Upgrade a `GET /v1/events` request to a WebSocket connection.
///
/// This is a thin delegation to `WebSocketUpgrade::on_upgrade()`, per
/// `ANVILML_DESIGN.md §3.3` — all connection behavior lives in
/// `handle_socket()`.
pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

/// Handle a single WebSocket connection.
///
/// Subscribes to `state.broadcaster` first, then immediately sends a
/// placeholder/zero-valued `SystemStats` frame as the connection's first
/// message. A placeholder value is acceptable here — per `P16-C1`'s scope,
/// the real periodic tick (`P16-D1`) is a separate, later task, and this
/// handler must not block waiting for one.
///
/// Subscribing *before* sending the initial frame means no event published
/// in between is lost once the forward loop (`P16-C2`) is added — that loop
/// will consume the same subscription created here. For this task, the
/// subscription is simply dropped when the function returns after the one
/// send; forwarding subsequent events is explicitly out of scope.
async fn handle_socket(mut socket: WebSocket, state: AppState) {
    // Subscribe before sending anything, so the eventual forward loop
    // (P16-C2) — which will pick up this same subscription — cannot miss
    // an event published in the gap between subscribing and the first send.
    let _receiver = state.broadcaster.subscribe();

    // A placeholder/zero-valued SystemStats frame is explicitly acceptable
    // for this task per ANVILML_DESIGN.md §13.6 and the P16-C1 task
    // context — the real periodic tick is P16-D1's scope.
    let initial = WsEvent::SystemStats {
        cpu_pct: 0.0,
        ram_used_mib: 0,
        workers: Vec::new(),
    };

    // Serializing a fixed, known-good enum literal cannot fail in practice.
    // If it somehow did, there is nothing meaningful left to send, so the
    // connection is simply closed by returning without sending.
    let Ok(json) = serde_json::to_string(&initial) else {
        return;
    };

    // Best-effort send: if the client has already disconnected before this
    // point, there is nothing further this task's scope requires — the
    // forward loop (P16-C2) is responsible for handling subsequent send
    // failures on an ongoing connection.
    let _ = socket.send(Message::text(json)).await;
}
