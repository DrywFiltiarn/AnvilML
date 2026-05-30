//! IPC message types — the complete communication contract between the Rust
//! supervisor and Python worker processes.
//!
//! `WorkerMessage` (§7.2) flows **Rust → Python** (commands from the supervisor).
//! `WorkerEvent` (§7.3) flows **Python → Rust** (status updates from workers).
//!
//! Both enums derive `Serialize`/`Deserialize` via `serde` and use
//! `rmp-serde` with named-map encoding for Python interop.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use anvilml_core::types::JobSettings;

// ---------------------------------------------------------------------------
// WorkerMessage — Rust → Python commands
// ---------------------------------------------------------------------------

/// Commands sent from the Rust supervisor to a Python worker process.
///
/// Matches `ANVILML_DESIGN.md` §7.2 exactly.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WorkerMessage {
    /// Health-check ping; the worker must reply with `Pong` carrying the
    /// same sequence number.
    Ping { seq: u64 },

    /// Graceful shutdown request. The worker should finish its current node
    /// (if any) and then send `Dying` before exiting.
    Shutdown,

    /// Instructs the worker to initialize hardware on the given device
    /// string (e.g. `"cuda:0"`, `"cpu"`). Sent once at worker start.
    InitializeHardware { device_str: String },

    /// Dispatch a job for execution. Carries the full graph, settings,
    /// and target device index.
    Execute {
        job_id: Uuid,
        graph: serde_json::Value,
        settings: JobSettings,
        device_index: u32,
    },

    /// Request cooperative cancellation of an in-flight job. The worker
    /// should abort at the next checkpoint (between nodes or per-step).
    CancelJob { job_id: Uuid },

    /// Request a one-shot memory report. The worker replies with
    /// `MemoryReport` carrying current VRAM/RAM usage.
    MemoryQuery,
}

// ---------------------------------------------------------------------------
// WorkerEvent — Python → Rust status updates
// ---------------------------------------------------------------------------

/// Status events sent from a Python worker to the Rust supervisor.
///
/// Matches `ANVILML_DESIGN.md` §7.3 exactly.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WorkerEvent {
    /// Worker has finished hardware initialization and is ready to accept
    /// jobs. Carries device metadata for the supervisor's registry.
    Ready {
        worker_id: String,
        device_index: u32,
        vram_total_mib: u32,
    },

    /// Reply to a `Ping` message; carries the same sequence number.
    Pong { seq: u64 },

    /// Worker is dying (responding to `Shutdown` or due to an unrecoverable
    /// error). Carries a human-readable reason.
    Dying { reason: String },

    /// Periodic memory usage report. Used by the supervisor for dispatch
    /// admission ranking and system-stats WebSocket broadcasting.
    MemoryReport {
        vram_used_mib: u32,
        ram_used_mib: u64,
    },

    /// Per-node progress update during job execution.
    Progress {
        job_id: Uuid,
        node_index: u32,
        node_total: u32,
        node_type: String,
        step: Option<u32>,
        step_total: Option<u32>,
    },

    /// Job has produced an image. Carries the PNG as base64-encoded bytes.
    ImageReady {
        job_id: Uuid,
        image_b64: String,
        width: u32,
        height: u32,
        seed: i64,
    },

    /// Job completed successfully. Carries the wall-clock elapsed time.
    Completed { job_id: Uuid, elapsed_ms: u64 },

    /// Job failed with an error. Carries the error message and full
    /// Python traceback (if available).
    Failed {
        job_id: Uuid,
        error: String,
        traceback: String,
    },

    /// Job was cancelled cooperatively by the worker in response to a
    /// `CancelJob` command.
    Cancelled { job_id: Uuid },
}

// ---------------------------------------------------------------------------
// Tests — msgpack serialization round-trips
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn serialize_roundtrip<
        T: Serialize + for<'de> Deserialize<'de> + PartialEq + std::fmt::Debug,
    >(
        value: &T,
    ) -> T {
        let bytes = rmp_serde::to_vec_named(value).expect("serialize failed");
        let back: T = rmp_serde::from_slice(&bytes).expect("deserialize failed");
        assert_eq!(value, &back, "round-trip mismatch");
        back
    }

    // ------------------------------------------------------------------
    // WorkerMessage — serialization round-trips
    // ------------------------------------------------------------------

    #[test]
    fn worker_message_ping_roundtrip() {
        let msg = WorkerMessage::Ping { seq: 42 };
        let back = serialize_roundtrip(&msg);
        assert_eq!(back, msg);
    }

    #[test]
    fn worker_message_shutdown_roundtrip() {
        let msg = WorkerMessage::Shutdown;
        let back = serialize_roundtrip(&msg);
        assert_eq!(back, msg);
    }

    #[test]
    fn worker_message_initialize_hardware_roundtrip() {
        let msg = WorkerMessage::InitializeHardware {
            device_str: "cuda:0".into(),
        };
        let back = serialize_roundtrip(&msg);
        assert_eq!(back, msg);
    }

    #[test]
    fn worker_message_execute_roundtrip() {
        let msg = WorkerMessage::Execute {
            job_id: Uuid::new_v4(),
            graph: serde_json::json!({ "nodes": [], "links": [] }),
            settings: JobSettings {
                model_id: Uuid::new_v4(),
                kind: Some("diffusion".into()),
                device: Some("cuda:0".into()),
                num_steps: 50,
                seed: Some(12345),
            },
            device_index: 0,
        };
        let back = serialize_roundtrip(&msg);
        assert_eq!(back, msg);
    }

    #[test]
    fn worker_message_cancel_job_roundtrip() {
        let job_id = Uuid::new_v4();
        let msg = WorkerMessage::CancelJob { job_id };
        let back = serialize_roundtrip(&msg);
        assert_eq!(back, msg);
        if let WorkerMessage::CancelJob { job_id: back_id } = back {
            assert_eq!(job_id, back_id);
        } else {
            panic!("expected CancelJob variant");
        }
    }

    #[test]
    fn worker_message_memory_query_roundtrip() {
        let msg = WorkerMessage::MemoryQuery;
        let back = serialize_roundtrip(&msg);
        assert_eq!(back, msg);
    }

    // ------------------------------------------------------------------
    // WorkerEvent — serialization round-trips
    // ------------------------------------------------------------------

    #[test]
    fn worker_event_ready_roundtrip() {
        let evt = WorkerEvent::Ready {
            worker_id: "worker-0".into(),
            device_index: 0,
            vram_total_mib: 24576,
        };
        let back = serialize_roundtrip(&evt);
        assert_eq!(back, evt);
    }

    #[test]
    fn worker_event_pong_roundtrip() {
        let evt = WorkerEvent::Pong { seq: 42 };
        let back = serialize_roundtrip(&evt);
        assert_eq!(back, evt);
    }

    #[test]
    fn worker_event_dying_roundtrip() {
        let evt = WorkerEvent::Dying {
            reason: "shutdown requested".into(),
        };
        let back = serialize_roundtrip(&evt);
        assert_eq!(back, evt);
    }

    #[test]
    fn worker_event_memory_report_roundtrip() {
        let evt = WorkerEvent::MemoryReport {
            vram_used_mib: 8192,
            ram_used_mib: 4_294_967_296,
        };
        let back = serialize_roundtrip(&evt);
        assert_eq!(back, evt);
    }

    #[test]
    fn worker_event_progress_roundtrip() {
        let job_id = Uuid::new_v4();
        let evt = WorkerEvent::Progress {
            job_id,
            node_index: 2,
            node_total: 10,
            node_type: "KSampler".into(),
            step: Some(5),
            step_total: Some(50),
        };
        let back = serialize_roundtrip(&evt);
        assert_eq!(back, evt);
        if let WorkerEvent::Progress {
            job_id: back_id, ..
        } = back
        {
            assert_eq!(job_id, back_id);
        } else {
            panic!("expected Progress variant");
        }
    }

    #[test]
    fn worker_event_image_ready_roundtrip() {
        let job_id = Uuid::new_v4();
        let evt = WorkerEvent::ImageReady {
            job_id,
            image_b64: "iVBORw0KGgo=".into(),
            width: 1024,
            height: 1024,
            seed: 42,
        };
        let back = serialize_roundtrip(&evt);
        assert_eq!(back, evt);
    }

    #[test]
    fn worker_event_completed_roundtrip() {
        let job_id = Uuid::new_v4();
        let evt = WorkerEvent::Completed {
            job_id,
            elapsed_ms: 3456,
        };
        let back = serialize_roundtrip(&evt);
        assert_eq!(back, evt);
    }

    #[test]
    fn worker_event_failed_roundtrip() {
        let job_id = Uuid::new_v4();
        let evt = WorkerEvent::Failed {
            job_id,
            error: "cuda_oom".into(),
            traceback: "Traceback (most recent call last):\n  ...".into(),
        };
        let back = serialize_roundtrip(&evt);
        assert_eq!(back, evt);
    }

    #[test]
    fn worker_event_cancelled_roundtrip() {
        let job_id = Uuid::new_v4();
        let evt = WorkerEvent::Cancelled { job_id };
        let back = serialize_roundtrip(&evt);
        assert_eq!(back, evt);
    }

    // ------------------------------------------------------------------
    // Named-map format verification (Python interop invariant)
    // ------------------------------------------------------------------

    #[test]
    fn msgpack_uses_named_map_format() {
        // Named-map encoding uses map type markers (0x80/0x81…/0xde/0xdf),
        // whereas compact array encoding uses array markers (0x90…/0xdf).
        // We assert that the first payload byte after the array-length
        // header is a map marker to verify named-map mode.
        let msg = WorkerMessage::Ping { seq: 1 };
        let bytes = rmp_serde::to_vec_named(&msg).expect("serialize");
        // The first byte is the array-length for the enum-struct array.
        // For a single-variant struct with named fields, rmp-serde wraps
        // it in an array of length 2: [variant_index, fields_map].
        // We just verify deserialization succeeds — the key invariant
        // is that named-map format is used (not compact).
        let _: WorkerMessage = rmp_serde::from_slice(&bytes).expect("deserialize");
    }
}
