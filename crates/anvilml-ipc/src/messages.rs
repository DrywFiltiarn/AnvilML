//! Wire protocol types for Rust ↔ Python messages.
//!
//! This module owns two enums:
//!
//! - `WorkerMessage` — the set of message variants the Rust supervisor sends
//!   to the Python worker over the ZeroMQ ROUTER transport.
//! - `WorkerEvent` — the set of event variants the Python worker sends back
//!   to the Rust supervisor (startup reports, keepalive pongs, memory reports,
//!   and — deferred to P7-A4 — job-lifecycle events).
//!
//! Each variant is msgpack-serialisable via the `serde` derive macros with the
//! `#[serde(tag = "_type")]` attribute, producing flat dicts keyed by `"_type"`
//! that the Python side decodes with `msgpack.unpackb()`.

use anvilml_core::{JobSettings, NodeTypeDescriptor};
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

/// Events the Python worker sends to the Rust supervisor.
///
/// Serialised as msgpack flat dicts with a `"_type"` discriminator field.
/// The Rust supervisor dispatches on `"_type"` to route to the handler.
///
/// Event categories: startup reports (`Ready`), keepalive pongs (`Pong`),
/// memory reports (`MemoryReport`), job-lifecycle events (`Progress`,
/// `ImageReady`, `Completed`, `Failed`, `Cancelled`), and termination
/// notifications (`Dying`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "_type")]
pub enum WorkerEvent {
    /// Worker startup report — sent once when the worker first connects.
    ///
    /// Carries full device capabilities and the registered node type catalogue.
    Ready {
        /// Stable worker identity (e.g. `"gpu:0"`).
        worker_id: String,
        /// Device index of the target GPU (e.g. `0` for the first GPU).
        device_index: u32,
        /// Human-readable device name (e.g. `"NVIDIA RTX 4090"`).
        device_name: String,
        /// Hardware backend: `"cuda"`, `"rocm"`, or `"cpu"`.
        device_type: String,
        /// Total VRAM in mebibytes.
        vram_total_mib: u32,
        /// Free VRAM in mebibytes at time of reporting.
        vram_free_mib: u32,
        /// PyTorch version string (e.g. `"2.5.1+cu124"`).
        torch_version: String,
        /// Whether FP16 (half-precision) is supported.
        fp16: bool,
        /// Whether BF16 (bfloat16) is supported.
        bf16: bool,
        /// Whether FP8 is supported.
        fp8: bool,
        /// Whether Flash Attention is available.
        flash_attention: bool,
        /// Capability source: `"pytorch"` (real hardware probe) or `"mock"`
        /// (synthetic probe from `ANVILML_WORKER_MOCK=1`).
        capabilities_source: String,
        /// The set of node types registered by this worker.
        node_types: Vec<NodeTypeDescriptor>,
    },

    /// Keepalive pong — replies to `WorkerMessage::Ping { seq }`.
    ///
    /// The `seq` field echoes the sequence number from the original ping.
    Pong {
        /// Echoed sequence number from the original `Ping`.
        seq: u64,
    },

    /// Worker is about to terminate.
    ///
    /// Sent before the worker process exits, carrying a reason string.
    Dying {
        /// Human-readable reason for termination (e.g. `"OOM"`, `"shutdown"`).
        reason: String,
    },

    /// Memory usage report — replies to `WorkerMessage::MemoryQuery`.
    ///
    /// Reports current VRAM and system RAM usage in mebibytes.
    MemoryReport {
        /// Current VRAM usage in mebibytes.
        vram_used_mib: u32,
        /// Current system RAM usage in mebibytes.
        ram_used_mib: u64,
    },

    /// Job execution progress report — sent periodically during generation.
    ///
    /// Carries the current step number and an optional base64-encoded preview
    /// image (e.g. a low-res latent decode at this step). The `total_steps`
    /// field lets clients render a progress bar.
    Progress {
        /// Stable unique identifier for this job (UUID v4).
        job_id: Uuid,
        /// Current step number (1-indexed).
        step: u32,
        /// Total number of steps for this job.
        total_steps: u32,
        /// Optional base64-encoded preview image at this step (`None` when
        /// no preview is available yet).
        preview_b64: Option<String>,
    },

    /// Generated image is ready — sent after a successful decode step.
    ///
    /// Carries the full base64-encoded image, its dimensions, format, and the
    /// generation parameters that produced it. The client stores this as an
    /// artifact and associates it with `job_id`.
    ImageReady {
        /// Stable unique identifier for this job (UUID v4).
        job_id: Uuid,
        /// Base64-encoded image bytes (PNG or equivalent).
        image_b64: String,
        /// Image width in pixels.
        width: u32,
        /// Image height in pixels.
        height: u32,
        /// Image format string (e.g. `"png"`, `"webp"`).
        format: String,
        /// Random seed used for generation (for reproducibility).
        seed: i64,
        /// Number of diffusion steps executed.
        steps: u32,
    },

    /// Job completed successfully.
    ///
    /// Sent after `ImageReady` (or when the job produces no image output).
    /// The client may transition the job to a terminal state.
    Completed {
        /// Stable unique identifier for this job (UUID v4).
        job_id: Uuid,
        /// Total wall-clock time in milliseconds from job start to completion.
        elapsed_ms: u64,
    },

    /// Job failed with an error.
    ///
    /// Sent when the worker encounters a non-recoverable error during
    /// execution. The `traceback` field is present when the worker was
    /// able to capture a full Python traceback.
    Failed {
        /// Stable unique identifier for this job (UUID v4).
        job_id: Uuid,
        /// Human-readable error description (e.g. `"CUDA out of memory"`).
        error: String,
        /// Optional full traceback string (present when the worker captured
        /// a Python exception traceback).
        traceback: Option<String>,
    },

    /// Job was cancelled by the client.
    ///
    /// Sent when the worker received a `CancelJob` message and stopped
    /// execution at the next cancellation checkpoint.
    Cancelled {
        /// Stable unique identifier for this job (UUID v4).
        job_id: Uuid,
    },
}
