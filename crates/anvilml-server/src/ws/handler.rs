//! WebSocket upgrade handler for `/v1/events`.

use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    extract::State,
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use tracing::error;

use crate::AppState;

/// WebSocket handler for the `/v1/events` endpoint.
///
/// Upgrades the HTTP connection to a WebSocket, subscribes to the shared
/// [`EventBroadcaster`], and forwards each event as a JSON text frame.
pub async fn ws_events(
    upgrade: WebSocketUpgrade,
    State(state): State<std::sync::Arc<AppState>>,
) -> impl IntoResponse {
    let state = (*state).clone();
    upgrade.on_upgrade(move |stream| handle_connection(stream, state))
}

async fn handle_connection(stream: WebSocket, state: AppState) {
    let mut rx = state.broadcaster.subscribe();

    let (mut ws_tx, mut ws_rx) = stream.split();

    // Forward task: broadcast receiver → WS client.
    let forward = async move {
        while let Ok(event) = rx.recv().await {
            let json = match serde_json::to_string(&event) {
                Ok(j) => j,
                Err(e) => {
                    error!("Failed to serialize WsEvent: {e}");
                    break;
                }
            };

            if ws_tx.send(Message::Text(json)).await.is_err() {
                break;
            }
        }
    };

    // Receive task: WS client → drop (we ignore inbound messages).
    let receive = async move {
        while let Some(msg) = ws_rx.next().await {
            match msg {
                Ok(Message::Close(_)) => break,
                Err(_) => break,
                _ => {} // ignore non-close messages
            }
        }
    };

    tokio::select! {
        _ = forward => {},
        _ = receive => {},
    }
}
