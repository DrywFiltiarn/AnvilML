//! IPC message types for AnvilML worker communication.
//!
//! This module defines the `WorkerMessage` and `WorkerEvent` enums that form the
//! contract between the Rust supervisor and Python worker processes. Messages are
//! serialised and deserialised using msgpack via `rmp-serde` flat-dict encoding,
//! with the `_type` discriminator field enabling variant selection on the receiver.
//!
//! **Serialization contract:**
//! - `WorkerMessage` values are encoded with `rmp_serde::to_vec_named` which produces
//!   a flat msgpack map (dict) with the `_type` key included as a named field.
//!   This matches the format expected by Python's `msgpack` library.
//! - `WorkerEvent` values are decoded with `rmp_serde::from_slice` which reads the
//!   msgpack map and uses the `_type` discriminator to select the correct enum variant.

use anvilml_core::{JobSettings, NodeTypeDescriptor};
use rmp_serde;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

/// Errors that can occur during IPC message serialization or deserialization.
#[derive(Debug, Error)]
pub enum IpcError {
    /// Serialization failed — the message could not be encoded to msgpack bytes.
    /// This typically occurs when a nested type lacks a valid `Serialize` impl.
    #[error("failed to serialize message: {0}")]
    Serialize(String),

    /// Deserialization failed — the bytes could not be decoded into a `WorkerEvent`.
    /// This typically occurs when the `_type` discriminator is missing or unrecognized,
    /// or when a field has an unexpected type.
    #[error("failed to deserialize event: {0}")]
    Deserialize(String),
}

/// A command sent from the Rust supervisor to a Python worker.
///
/// Each variant carries a `_type` discriminator field (via `#[serde(tag = "_type")]`)
/// that the Python worker uses to select the correct handler.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "_type")]
pub enum WorkerMessage {
    /// Heartbeat ping sent by the supervisor to check worker liveness.
    /// The `seq` field is a monotonically increasing sequence number
    /// that allows the supervisor to match pongs to their pings.
    Ping { seq: u64 },

    /// Graceful shutdown command sent to the worker.
    /// The worker should finish any in-progress work, emit a `Dying` event,
    /// and then terminate cleanly.
    Shutdown,

    /// Execute a job on the worker.
    ///
    /// The `graph` field contains the computation graph as opaque JSON.
    /// The Rust types do not interpret the graph contents — that is handled
    /// by the Python worker's node execution engine.
    Execute {
        /// Unique identifier for the job to execute.
        job_id: Uuid,
        /// Computation graph as JSON.
        graph: Value,
        /// Job settings (device preference, etc.).
        settings: JobSettings,
        /// GPU device index to run on.
        device_index: u32,
    },

    /// Request cancellation of a running or queued job.
    CancelJob {
        /// Unique identifier of the job to cancel.
        job_id: Uuid,
    },

    /// Query the worker for current memory usage.
    /// The worker responds with a `MemoryReport` event.
    MemoryQuery,
}

/// An event emitted by a Python worker to the Rust supervisor.
///
/// Each variant carries a `_type` discriminator field (via `#[serde(tag = "_type")]`)
/// that the Rust supervisor uses to select the correct handler.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "_type")]
pub enum WorkerEvent {
    /// Worker readiness notification emitted after hardware probing and node import.
    ///
    /// This event is the synchronization point between Rust and Python. The Rust
    /// supervisor transitions the worker to `Idle` only on receipt of a valid
    /// `Ready` event. A worker that does not emit `Ready` within 60 seconds
    /// is killed and respawned.
    Ready {
        /// Logical worker identity (e.g. `"worker-0"`).
        worker_id: String,
        /// GPU device index this worker is bound to.
        device_index: u32,
        /// Human-readable device name (e.g. `"NVIDIA RTX 4090"`).
        device_name: String,
        /// Hardware type: `"cuda"`, `"rocm"`, or `"cpu"`.
        device_type: String,
        /// Total VRAM in mebibytes.
        vram_total_mib: u32,
        /// Free VRAM in mebibytes at time of probe.
        vram_free_mib: u32,
        /// Torch version string reported by `torch.__version__`.
        torch_version: String,
        /// Whether the device supports fp16 (half-precision) inference.
        fp16: bool,
        /// Whether the device supports bf16 (bfloat16) inference.
        bf16: bool,
        /// Whether the device supports fp8 inference.
        fp8: bool,
        /// Whether the device supports Flash Attention.
        flash_attention: bool,
        /// All node types registered in the Python worker's NODE_REGISTRY.
        node_types: Vec<NodeTypeDescriptor>,
    },

    /// Response to a `Ping` message, confirming the worker is alive.
    Pong {
        /// Sequence number matching the original `Ping`.
        seq: u64,
    },

    /// Worker is terminating — sent just before process exit.
    ///
    /// The `reason` field explains why the worker is dying (e.g. `"SIGTERM"`,
    /// `"unrecoverable_error"`). The supervisor uses this to distinguish
    /// graceful shutdown from crashes.
    Dying {
        /// Human-readable reason for termination.
        reason: String,
    },

    /// Response to a `MemoryQuery` message reporting current memory usage.
    MemoryReport {
        /// VRAM used in mebibytes.
        vram_used_mib: u32,
        /// System RAM used in mebibytes.
        ram_used_mib: u64,
    },

    /// Progress update emitted during job execution.
    Progress {
        /// Job this progress report belongs to.
        job_id: Uuid,
        /// Current step number (0-based).
        step: u32,
        /// Total number of steps in the job.
        total_steps: u32,
        /// Optional base64-encoded thumbnail preview of the current state.
        /// `None` if no preview is available at this step.
        preview_b64: Option<String>,
    },

    /// Generated image delivered by the worker.
    ImageReady {
        /// Job this image belongs to.
        job_id: Uuid,
        /// Base64-encoded image data.
        image_b64: String,
        /// Image width in pixels.
        width: u32,
        /// Image height in pixels.
        height: u32,
        /// Image format (e.g. `"png"`, `"jpg"`).
        format: String,
        /// Random seed used for generation.
        seed: i64,
        /// Number of steps executed to produce this image.
        steps: u32,
    },

    /// Job completed successfully.
    Completed {
        /// Job that completed.
        job_id: Uuid,
        /// Total wall-clock time in milliseconds.
        elapsed_ms: u64,
    },

    /// Job failed with an error.
    Failed {
        /// Job that failed.
        job_id: Uuid,
        /// Human-readable error message.
        error: String,
        /// Optional Python traceback string for diagnostic purposes.
        /// `None` if no traceback was available.
        traceback: Option<String>,
    },

    /// Job was cancelled by the supervisor.
    Cancelled {
        /// Job that was cancelled.
        job_id: Uuid,
    },
}

/// Encode a `WorkerMessage` into msgpack bytes.
///
/// Uses `rmp_serde::to_vec_named` which produces a flat msgpack map with the
/// `_type` discriminator included as a named field. This matches the format
/// expected by Python's `msgpack` library for tagged-dict deserialization.
///
/// # Errors
///
/// Returns `IpcError::Serialize` if the message cannot be encoded. This is
/// unexpected for well-formed messages since all variants derive `Serialize`.
pub fn encode_message(msg: &WorkerMessage) -> Result<Vec<u8>, IpcError> {
    rmp_serde::to_vec_named(msg).map_err(|e| IpcError::Serialize(e.to_string()))
}

/// Decode msgpack bytes into a `WorkerEvent`.
///
/// Uses `rmp_serde::from_slice` which reads the msgpack map and uses the
/// `_type` discriminator field to select the correct enum variant.
///
/// # Errors
///
/// Returns `IpcError::Deserialize` if the bytes are not valid msgpack,
/// if the `_type` discriminator is missing or unrecognized, or if any
/// field has an unexpected type.
pub fn decode_event(bytes: &[u8]) -> Result<WorkerEvent, IpcError> {
    rmp_serde::from_slice(bytes).map_err(|e| IpcError::Deserialize(e.to_string()))
}
