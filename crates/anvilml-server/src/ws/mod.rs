//! WebSocket support module for AnvilML.
//!
//! Provides event broadcasting (`broadcaster`), the WebSocket handler
//! (`handler`, implemented in P7-A2), and system stats tick (`stats_tick`,
//! implemented in P7-A3).

pub mod broadcaster;
pub mod handler;
pub mod stats_tick;

/// Re-export `EventBroadcaster` at the crate root so consumers can write
/// `anvilml_server::ws::EventBroadcaster` instead of the deeper path.
pub use broadcaster::EventBroadcaster;
