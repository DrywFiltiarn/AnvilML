//! WebSocket event broadcasting support.
//!
//! Provides `EventBroadcaster` — a thin wrapper around a Tokio broadcast channel
//! for broadcasting `WsEvent` messages to connected WebSocket clients.

pub mod broadcaster;

pub use broadcaster::EventBroadcaster;
