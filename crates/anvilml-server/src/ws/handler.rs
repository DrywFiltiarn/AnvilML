//! WebSocket upgrade handler for `/v1/events`.

use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    extract::State,
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use std::time::Duration;
use tracing::error;

use crate::App;

/// WebSocket handler for the `/v1/events` endpoint.
///
/// Upgrades the HTTP connection to a WebSocket, subscribes to the shared
/// [`EventBroadcaster`], and forwards each event as a JSON text frame.
pub async fn ws_events(
    upgrade: WebSocketUpgrade,
    State(state): State<std::sync::Arc<App>>,
) -> impl IntoResponse {
    let state = (*state).clone();
    upgrade.on_upgrade(move |stream| handle_connection(stream, state))
}

async fn handle_connection(stream: WebSocket, state: App) {
    let mut rx = state.broadcaster.subscribe();

    let (mut ws_tx, mut ws_rx) = stream.split();

    // Forward + ping task: broadcast receiver → WS client, with keepalive pings.
    let mut interval = tokio::time::interval(Duration::from_secs(30));
    // Skip the first immediate tick so we don't ping before any events arrive.
    let _ = interval.tick().await;

    let forward = async move {
        loop {
            tokio::select! {
                biased;
                event = rx.recv() => {
                    match event {
                        Ok(event) => {
                            let json = match serde_json::to_string(&event) {
                                Ok(j) => j,
                                Err(e) => {
                                    error!("Failed to serialize WsEvent: {e}");
                                    break;
                                }
                            };

                            if ws_tx.send(Message::Text(json.into())).await.is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
                _ = interval.tick() => {
                    if ws_tx.send(Message::Ping(vec![].into())).await.is_err() {
                        break;
                    }
                }
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
