//! WebSocket support for the `/v1/events` live event stream.
//!
//! Per `ANVILML_DESIGN.md §13.1`, this module is split into `handler.rs`
//! (the `GET /v1/events` upgrade handler and per-connection logic) and,
//! from `P16-D1` onward, `stats_tick.rs` (the periodic background
//! `SystemStats` publisher). `EventBroadcaster` itself lives in
//! `anvilml-ipc` and is re-exported there, not redefined here.

pub mod handler;

pub use handler::ws_handler;
