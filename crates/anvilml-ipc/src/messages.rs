//! Wire protocol types for Rust → Python messages.
//!
//! This module owns the `WorkerMessage` enum — the set of message variants the
//! Rust supervisor sends to the Python worker over the ZeroMQ ROUTER transport.
//! Each variant is msgpack-serialisable via the `serde` derive macros with the
//! `#[serde(tag = "_type")]` attribute, producing flat dicts keyed by `"_type"`
//! that the Python side decodes with `msgpack.unpackb()`.

use anvilml_core::JobSettings;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Messages the Rust supervisor sends to the Python worker.
///
/// Serialised as msgpack flat dicts with a `"_type"` discriminator field.
/// The Python worker dispatches on `"_type"` to route to the handler.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "_type")]
pub enum WorkerMessage {
    /// Keepalive ping. The worker must reply with `WorkerEvent::Pong { seq }`.
    Ping {
        /// Monotonically increasing sequence number for matching ping to pong.
        seq: u64,
    },

    /// Graceful shutdown. The worker should finish its current step, then exit 0.
    Shutdown,

    /// Execute a generation job on the target worker.
    ///
    /// Carries the full computation graph, execution settings, and the target
    /// device index. The worker resolves the graph and dispatches node
    /// execution.
    Execute {
        /// Stable unique identifier for this job (UUID v4).
        job_id: Uuid,
        /// The computation graph to execute, in the format expected by workers.
        graph: serde_json::Value,
        /// Optional execution settings (device preference, etc.).
        settings: JobSettings,
        /// Device index of the target worker (e.g. `0` for the first GPU).
        device_index: u32,
    },

    /// Cooperatively cancel an in-flight job.
    ///
    /// The worker should signal its execution loop to stop at the next
    /// cancellation checkpoint. Does not forcibly kill the process.
    CancelJob {
        /// The job to cancel.
        job_id: Uuid,
    },

    /// Query the worker's current memory usage.
    ///
    /// The worker must reply with `WorkerEvent::MemoryReport` containing
    /// `vram_used_mib` and `ram_used_mib`.
    MemoryQuery,
}
