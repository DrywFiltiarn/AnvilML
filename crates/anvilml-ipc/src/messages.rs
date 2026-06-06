use anvilml_core::types::job::JobSettings;
use anvilml_core::types::worker::WorkerStatus;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use uuid::Uuid;

// ── WorkerMessage (Rust → Python) ───────────────────────────────────────────────

/// Messages sent from the Rust server to the Python worker over IPC.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkerMessage {
    /// Keepalive ping.
    Ping { seq: u64 },
    /// Graceful shutdown request.
    Shutdown,
    /// Initialize the worker with a hardware device descriptor.
    InitializeHardware { device_str: String },
    /// Execute a generation job.
    Execute {
        job_id: Uuid,
        graph: JsonValue,
        settings: JobSettings,
        device_index: u32,
    },
    /// Cancel an in-flight job.
    CancelJob { job_id: Uuid },
    /// Query the worker's current memory usage.
    MemoryQuery,
}

impl PartialEq for WorkerMessage {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Ping { seq: a }, Self::Ping { seq: b }) => a == b,
            (Self::Shutdown, Self::Shutdown) => true,
            (
                Self::InitializeHardware { device_str: a },
                Self::InitializeHardware { device_str: b },
            ) => a == b,
            (
                Self::Execute {
                    job_id: a_job,
                    graph: a_graph,
                    settings: a_settings,
                    device_index: a_idx,
                },
                Self::Execute {
                    job_id: b_job,
                    graph: b_graph,
                    settings: b_settings,
                    device_index: b_idx,
                },
            ) => {
                a_job == b_job
                    && a_graph == b_graph
                    && a_settings.seed == b_settings.seed
                    && a_settings.steps == b_settings.steps
                    && a_settings.guidance_scale == b_settings.guidance_scale
                    && a_settings.width == b_settings.width
                    && a_settings.height == b_settings.height
                    && a_settings.device_preference == b_settings.device_preference
                    && a_idx == b_idx
            }
            (Self::CancelJob { job_id: a }, Self::CancelJob { job_id: b }) => a == b,
            (Self::MemoryQuery, Self::MemoryQuery) => true,
            _ => false,
        }
    }
}

// ── WorkerEvent (Python → Rust) ────────────────────────────────────────────────

/// Events sent from the Python worker to the Rust server over IPC.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkerEvent {
    /// Worker is ready and reporting its hardware capabilities.
    Ready {
        worker_id: String,
        device_index: u32,
        vram_total_mib: u32,
        vram_free_mib: u32,
        arch: String,
        fp16: bool,
        bf16: bool,
        flash_attention: bool,
    },
    /// Keepalive pong response.
    Pong { seq: u64 },
    /// Worker is dying; includes a reason.
    Dying { reason: String },
    /// Memory usage report from the worker.
    MemoryReport {
        vram_used_mib: u32,
        ram_used_mib: u64,
    },
    /// Progress update during job execution.
    Progress {
        job_id: Uuid,
        node_index: u32,
        node_total: u32,
        node_type: String,
        step: Option<u32>,
        step_total: Option<u32>,
    },
    /// An image has been generated and is ready.
    ImageReady {
        job_id: Uuid,
        image_b64: String,
        width: u32,
        height: u32,
        format: String,
        seed: i64,
        steps: u32,
        prompt: String,
    },
    /// Job completed successfully.
    Completed { job_id: Uuid, elapsed_ms: u64 },
    /// Job failed with an error.
    Failed {
        job_id: Uuid,
        error: String,
        traceback: String,
    },
    /// Job was cancelled by the user.
    Cancelled { job_id: Uuid },
    /// Internal status transition (not sent over IPC to Python).
    WorkerStatusChanged { status: WorkerStatus },
}

impl PartialEq for WorkerEvent {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::Ready {
                    worker_id: a_wid,
                    device_index: a_didx,
                    vram_total_mib: a_vt,
                    vram_free_mib: a_vf,
                    arch: a_arch,
                    fp16: a_fp16,
                    bf16: a_bf16,
                    flash_attention: a_fa,
                },
                Self::Ready {
                    worker_id: b_wid,
                    device_index: b_didx,
                    vram_total_mib: b_vt,
                    vram_free_mib: b_vf,
                    arch: b_arch,
                    fp16: b_fp16,
                    bf16: b_bf16,
                    flash_attention: b_fa,
                },
            ) => {
                a_wid == b_wid
                    && a_didx == b_didx
                    && a_vt == b_vt
                    && a_vf == b_vf
                    && a_arch == b_arch
                    && a_fp16 == b_fp16
                    && a_bf16 == b_bf16
                    && a_fa == b_fa
            }
            (Self::Pong { seq: a }, Self::Pong { seq: b }) => a == b,
            (Self::Dying { reason: a }, Self::Dying { reason: b }) => a == b,
            (
                Self::MemoryReport {
                    vram_used_mib: a_v,
                    ram_used_mib: a_r,
                },
                Self::MemoryReport {
                    vram_used_mib: b_v,
                    ram_used_mib: b_r,
                },
            ) => a_v == b_v && a_r == b_r,
            (
                Self::Progress {
                    job_id: a_jid,
                    node_index: a_ni,
                    node_total: a_nt,
                    node_type: a_ntype,
                    step: a_step,
                    step_total: a_st,
                },
                Self::Progress {
                    job_id: b_jid,
                    node_index: b_ni,
                    node_total: b_nt,
                    node_type: b_ntype,
                    step: b_step,
                    step_total: b_st,
                },
            ) => {
                a_jid == b_jid
                    && a_ni == b_ni
                    && a_nt == b_nt
                    && a_ntype == b_ntype
                    && a_step == b_step
                    && a_st == b_st
            }
            (
                Self::ImageReady {
                    job_id: a_jid,
                    image_b64: a_img,
                    width: a_w,
                    height: a_h,
                    format: a_fmt,
                    seed: a_seed,
                    steps: a_steps,
                    prompt: a_prompt,
                },
                Self::ImageReady {
                    job_id: b_jid,
                    image_b64: b_img,
                    width: b_w,
                    height: b_h,
                    format: b_fmt,
                    seed: b_seed,
                    steps: b_steps,
                    prompt: b_prompt,
                },
            ) => {
                a_jid == b_jid
                    && a_img == b_img
                    && a_w == b_w
                    && a_h == b_h
                    && a_fmt == b_fmt
                    && a_seed == b_seed
                    && a_steps == b_steps
                    && a_prompt == b_prompt
            }
            (
                Self::Completed {
                    job_id: a_jid,
                    elapsed_ms: a_el,
                },
                Self::Completed {
                    job_id: b_jid,
                    elapsed_ms: b_el,
                },
            ) => a_jid == b_jid && a_el == b_el,
            (
                Self::Failed {
                    job_id: a_jid,
                    error: a_err,
                    traceback: a_tb,
                },
                Self::Failed {
                    job_id: b_jid,
                    error: b_err,
                    traceback: b_tb,
                },
            ) => a_jid == b_jid && a_err == b_err && a_tb == b_tb,
            (Self::Cancelled { job_id: a }, Self::Cancelled { job_id: b }) => a == b,
            (Self::WorkerStatusChanged { status: a }, Self::WorkerStatusChanged { status: b }) => {
                a == b
            }
            _ => false,
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip<T>(value: &T) -> bool
    where
        T: Serialize + for<'de> Deserialize<'de> + PartialEq + std::fmt::Debug,
    {
        let bytes = rmp_serde::to_vec_named(value).expect("serialize");
        let cursor = std::io::Cursor::new(&bytes);
        let restored: T = rmp_serde::from_read(cursor).expect("deserialize");
        assert_eq!(value, &restored, "round-trip mismatch");
        true
    }

    // ── WorkerMessage round-trips ────────────────────────────────────────────────

    #[test]
    fn worker_message_roundtrip_ping() {
        let msg = WorkerMessage::Ping { seq: 1 };
        assert!(roundtrip(&msg));
    }

    #[test]
    fn worker_message_roundtrip_shutdown() {
        let msg = WorkerMessage::Shutdown;
        assert!(roundtrip(&msg));
    }

    #[test]
    fn worker_message_roundtrip_init_hardware() {
        let msg = WorkerMessage::InitializeHardware {
            device_str: "cuda:0".to_string(),
        };
        assert!(roundtrip(&msg));
    }

    #[test]
    fn worker_message_roundtrip_execute() {
        let job_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let graph = serde_json::json!({
            "nodes": [
                { "id": "n0", "type": "ZitLoadPipeline", "inputs": { "model_id": "abc" } }
            ]
        });
        let settings = JobSettings {
            seed: 42,
            steps: 30,
            guidance_scale: 7.5,
            width: 1024,
            height: 1024,
            device_preference: Some(0),
        };
        let msg = WorkerMessage::Execute {
            job_id,
            graph,
            settings,
            device_index: 0,
        };
        assert!(roundtrip(&msg));
    }

    #[test]
    fn worker_message_roundtrip_cancel_job() {
        let job_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440001").unwrap();
        let msg = WorkerMessage::CancelJob { job_id };
        assert!(roundtrip(&msg));
    }

    #[test]
    fn worker_message_roundtrip_memory_query() {
        let msg = WorkerMessage::MemoryQuery;
        assert!(roundtrip(&msg));
    }

    // ── WorkerEvent round-trips ──────────────────────────────────────────────────

    #[test]
    fn worker_event_roundtrip_ready() {
        let evt = WorkerEvent::Ready {
            worker_id: "worker-0".to_string(),
            device_index: 0,
            vram_total_mib: 8192,
            vram_free_mib: 7000,
            arch: "gfx1100".to_string(),
            fp16: true,
            bf16: true,
            flash_attention: false,
        };
        assert!(roundtrip(&evt));
    }

    #[test]
    fn worker_event_roundtrip_cancelled() {
        let job_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440006").unwrap();
        let evt = WorkerEvent::Cancelled { job_id };
        assert!(roundtrip(&evt));
    }

    #[test]
    fn worker_event_roundtrip_status_changed() {
        let evt = WorkerEvent::WorkerStatusChanged {
            status: WorkerStatus::Dead,
        };
        assert!(roundtrip(&evt));
    }

    // ── Discriminant uniqueness ──────────────────────────────────────────────────

    #[test]
    fn all_worker_message_variants() {
        // Verify each variant is distinct by checking discriminants.
        let ping = WorkerMessage::Ping { seq: 0 };
        let shutdown = WorkerMessage::Shutdown;
        let init_hw = WorkerMessage::InitializeHardware {
            device_str: String::new(),
        };
        let execute = WorkerMessage::Execute {
            job_id: Uuid::nil(),
            graph: JsonValue::Null,
            settings: JobSettings::default(),
            device_index: 0,
        };
        let cancel = WorkerMessage::CancelJob {
            job_id: Uuid::nil(),
        };
        let memory = WorkerMessage::MemoryQuery;

        assert_ne!(
            std::mem::discriminant(&ping),
            std::mem::discriminant(&shutdown)
        );
        assert_ne!(
            std::mem::discriminant(&ping),
            std::mem::discriminant(&init_hw)
        );
        assert_ne!(
            std::mem::discriminant(&ping),
            std::mem::discriminant(&execute)
        );
        assert_ne!(
            std::mem::discriminant(&ping),
            std::mem::discriminant(&cancel)
        );
        assert_ne!(
            std::mem::discriminant(&ping),
            std::mem::discriminant(&memory)
        );
        assert_ne!(
            std::mem::discriminant(&shutdown),
            std::mem::discriminant(&init_hw)
        );
        assert_ne!(
            std::mem::discriminant(&shutdown),
            std::mem::discriminant(&execute)
        );
        assert_ne!(
            std::mem::discriminant(&shutdown),
            std::mem::discriminant(&cancel)
        );
        assert_ne!(
            std::mem::discriminant(&shutdown),
            std::mem::discriminant(&memory)
        );
        assert_ne!(
            std::mem::discriminant(&init_hw),
            std::mem::discriminant(&execute)
        );
        assert_ne!(
            std::mem::discriminant(&init_hw),
            std::mem::discriminant(&cancel)
        );
        assert_ne!(
            std::mem::discriminant(&init_hw),
            std::mem::discriminant(&memory)
        );
        assert_ne!(
            std::mem::discriminant(&execute),
            std::mem::discriminant(&cancel)
        );
        assert_ne!(
            std::mem::discriminant(&execute),
            std::mem::discriminant(&memory)
        );
        assert_ne!(
            std::mem::discriminant(&cancel),
            std::mem::discriminant(&memory)
        );
    }

    #[test]
    fn all_worker_event_variants() {
        let ready = WorkerEvent::Ready {
            worker_id: String::new(),
            device_index: 0,
            vram_total_mib: 0,
            vram_free_mib: 0,
            arch: String::new(),
            fp16: false,
            bf16: false,
            flash_attention: false,
        };
        let pong = WorkerEvent::Pong { seq: 0 };
        let dying = WorkerEvent::Dying {
            reason: String::new(),
        };
        let memory = WorkerEvent::MemoryReport {
            vram_used_mib: 0,
            ram_used_mib: 0,
        };
        let progress = WorkerEvent::Progress {
            job_id: Uuid::nil(),
            node_index: 0,
            node_total: 0,
            node_type: String::new(),
            step: None,
            step_total: None,
        };
        let image = WorkerEvent::ImageReady {
            job_id: Uuid::nil(),
            image_b64: String::new(),
            width: 0,
            height: 0,
            format: String::new(),
            seed: 0,
            steps: 0,
            prompt: String::new(),
        };
        let completed = WorkerEvent::Completed {
            job_id: Uuid::nil(),
            elapsed_ms: 0,
        };
        let failed = WorkerEvent::Failed {
            job_id: Uuid::nil(),
            error: String::new(),
            traceback: String::new(),
        };
        let cancelled = WorkerEvent::Cancelled {
            job_id: Uuid::nil(),
        };
        let status_changed = WorkerEvent::WorkerStatusChanged {
            status: WorkerStatus::Initializing,
        };

        let variants = [
            (&ready, "Ready"),
            (&pong, "Pong"),
            (&dying, "Dying"),
            (&memory, "MemoryReport"),
            (&progress, "Progress"),
            (&image, "ImageReady"),
            (&completed, "Completed"),
            (&failed, "Failed"),
            (&cancelled, "Cancelled"),
            (&status_changed, "WorkerStatusChanged"),
        ];

        for i in 0..variants.len() {
            for j in (i + 1)..variants.len() {
                let di = std::mem::discriminant(variants[i].0);
                let dj = std::mem::discriminant(variants[j].0);
                assert_ne!(
                    di, dj,
                    "Event variants {} and {} share a discriminant",
                    variants[i].1, variants[j].1
                );
            }
        }
    }
}
