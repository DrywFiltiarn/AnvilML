//! `GET /v1/events` WebSocket upgrade handler.
//!
//! Per `ANVILML_DESIGN.md §13.6`: on connect, (1) subscribe to the shared
//! `EventBroadcaster`, (2) send the current `SystemStats` immediately, then
//! (3) forward all subsequent `WsEvent`s as JSON text frames until the
//! consumer falls more than 1024 events behind, at which point the
//! connection is closed rather than caught up.

use anvilml_core::WsEvent;
use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use tokio::sync::broadcast::error::RecvError;

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
/// message. A placeholder value is acceptable here — the real periodic
/// tick (`P16-D1`) is a separate, later task, and this handler must not
/// block waiting for one.
///
/// After the initial frame, enters a forward loop that receives `WsEvent`s
/// from the broadcast channel, serialises each as JSON text, and sends it
/// to the client. On `Lagged` (consumer fell >1024 events behind) the
/// connection is closed with a Close frame; on `Closed` the channel is
/// permanently gone. On send failure the loop breaks (graceful disconnect).
#[tracing::instrument(skip(socket, state))]
async fn handle_socket(mut socket: WebSocket, state: AppState) {
    // Subscribe before sending anything, so no event published in the gap
    // between subscribing and the first send is lost — the forward loop
    // below consumes this same subscription.
    let mut receiver = state.broadcaster.subscribe();

    // A placeholder/zero-valued SystemStats frame is acceptable here —
    // the real periodic tick is P16-D1's later concern.
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

    // Best-effort send of the initial frame: if the client already
    // disconnected before this point, there is nothing further to do.
    if socket.send(Message::text(json)).await.is_err() {
        return;
    }

    // Forward loop: receive events from the broadcast channel and send
    // each as a JSON text frame to the connected client.
    loop {
        match receiver.recv().await {
            Ok(event) => {
                // Serialize the event as JSON text. If serialization
                // fails for this particular event, log and skip it
                // rather than dropping the entire connection — one
                // bad event should not disconnect all clients.
                let Ok(json) = serde_json::to_string(&event) else {
                    tracing::warn!(event = ?event, "failed to serialize WsEvent, skipping");
                    continue;
                };

                // Send the serialized event as a text frame. If the send
                // fails, the client has disconnected or the socket is
                // broken — break the loop for a graceful disconnect.
                if socket.send(Message::text(json)).await.is_err() {
                    tracing::warn!("WebSocket send failed, disconnecting client");
                    break;
                }
            }
            Err(RecvError::Lagged(n)) => {
                // The consumer fell behind the broadcast buffer (>1024
                // events). Per ANVILML_DESIGN.md §13.6, close the
                // connection rather than attempting to catch up — the
                // client must reconnect.
                tracing::warn!(lagged_by = %n, "WebSocket client lagged behind, closing connection");
                let _ = socket.send(Message::Close(None)).await;
                break;
            }
            Err(RecvError::Closed) => {
                // The broadcast channel sender was dropped — this should
                // never happen while the server is running.
                tracing::error!("WebSocket broadcast channel closed");
                let _ = socket.send(Message::Close(None)).await;
                break;
            }
        }
    }
}
