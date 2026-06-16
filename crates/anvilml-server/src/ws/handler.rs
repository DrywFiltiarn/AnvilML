//! WebSocket handler — implements the `/v1/events` endpoint (P7-A2).
//!
//! Provides `ws_events`, an async handler that accepts WebSocket upgrade requests,
//! subscribes to the shared `EventBroadcaster`, and forwards each `WsEvent` as a
//! JSON text frame to the connected client.

use axum::extract::ws::{Message, WebSocketUpgrade};
use axum::extract::{ConnectInfo, State};
use axum::response::IntoResponse;
use tokio::sync::broadcast;

use crate::state::AppState;

/// Handle a WebSocket upgrade request at `GET /v1/events`.
///
/// Accepts the upgrade, subscribes to the shared `EventBroadcaster` from
/// `AppState`, and enters a loop that serialises each `WsEvent` to JSON
/// and sends it as a text frame to the connected client.
///
/// The handler logs the remote address on connect and a disconnection
/// message when the client drops. If the broadcast receiver falls behind
/// (buffer overflow), lagged messages are skipped with a warning log
/// so the handler continues delivering newer events rather than
/// terminating.
///
/// # Arguments
///
/// * `ws` — The axum `WebSocketUpgrade` extractor, carrying the
///   client's upgrade request headers.
/// * `state` — Shared application state containing the `EventBroadcaster`
///   that all handlers and tasks use to push events.
/// * `remote_addr` — The client's socket address, extracted from the
///   underlying TCP connection via `ConnectInfo`.
pub async fn ws_events(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    ConnectInfo(remote_addr): ConnectInfo<std::net::SocketAddr>,
) -> impl IntoResponse {
    ws.on_upgrade(move |mut socket| async move {
        // The `WebSocket` passed to `on_upgrade` is already accepted —
        // axum performs the HTTP upgrade handshake before invoking the
        // closure. We can use the socket directly for send/recv.

        // Subscribe to the broadcaster — each subscriber gets an independent
        // receiver that will receive all events sent after this subscription
        // point. If the sender has buffered events at subscription time,
        // the receiver drains those first before receiving new ones.
        let mut rx = state.broadcaster.subscribe();

        tracing::info!(remote_addr = ?remote_addr, "ws client connected");

        // Main event delivery loop — receive events from the broadcaster
        // and forward them as JSON text frames to the WebSocket client.
        // The loop terminates when the client disconnects (send error)
        // or the broadcaster is shut down (recv error).
        loop {
            // Wait for the next event from the broadcast channel.
            // `recv()` returns `Err(Lagged(n))` when the client falls behind
            // and events were dropped from the buffer — we log and continue
            // rather than breaking, so the handler stays live for newer events.
            let event = match rx.recv().await {
                Ok(event) => event,
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    // The client is slow — some events were dropped from
                    // the ring buffer. Log the lag count and continue
                    // with the next (most recent) event.
                    tracing::warn!(lagged = n, "client fell behind, skipping lagged events");
                    continue;
                }
                Err(broadcast::error::RecvError::Closed) => {
                    // The broadcaster was shut down (sender dropped).
                    // This happens when all AppState clones are dropped.
                    tracing::info!("broadcaster shut down, disconnecting client");
                    break;
                }
            };

            // Serialize the event to JSON for transport over WebSocket.
            // WsEvent derives Serialize, so this should never fail on a
            // correctly-defined event — an error here indicates a bug
            // in the event type definition (missing derive or field).
            let json = match serde_json::to_string(&event) {
                Ok(json) => json,
                Err(e) => {
                    // Serialization failure on a WsEvent — this is a
                    // programming bug (missing Serialize derive or
                    // incompatible field type). Break to avoid
                    // infinite loop on a permanent error.
                    tracing::error!(error = %e, event_type = ?event, "failed to serialize WsEvent");
                    break;
                }
            };

            // Send the JSON string as a text frame to the client.
            // If the client has disconnected, the send will fail and we
            // break out of the loop — the connection is cleaned up.
            if socket.send(Message::Text(json.into())).await.is_err() {
                // Client disconnected — the send failed because the
                // underlying TCP connection was closed.
                break;
            }
        }

        tracing::info!("ws client disconnected");
    })
}
