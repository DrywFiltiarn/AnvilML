//! Inter-process communication for AnvilML.
//!
//! Provides the message types (`WorkerMessage`, `WorkerEvent`) that form the
//! complete communication contract between the Rust supervisor and Python
//! worker processes, as well as the length-prefixed msgpack framing layer.
//!
//! # Message types
//!
//! - [`WorkerMessage`]: Commands sent **Rust → Python** (§7.2)
//! - [`WorkerEvent`]: Status events sent **Python → Rust** (§7.3)
//!
//! Both use `rmp-serde` named-map encoding for Python interop.

pub mod messages;

pub use messages::{WorkerEvent, WorkerMessage};
