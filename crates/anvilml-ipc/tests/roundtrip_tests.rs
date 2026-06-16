//! Roundtrip serialization tests for WorkerMessage and WorkerEvent variants.
//!
//! Each test verifies that encoding a message/event and then decoding it
//! produces an identical value — proving that rmp-serde msgpack roundtrip
//! preserves all fields for every variant.
//!
//! WorkerMessage variants use `encode_message()` + `rmp_serde::from_slice::<WorkerMessage>`
//! for roundtrips. WorkerEvent variants use `rmp_serde::to_vec_named()` + `decode_event()`
//! for roundtrips, since `encode_message()` only accepts `&WorkerMessage`.

use anvilml_core::{JobSettings, NodeTypeDescriptor};
use anvilml_ipc::{decode_event, encode_message, WorkerEvent, WorkerMessage};
use rmp_serde;
use serde_json::json;
use uuid::Uuid;

// ── WorkerMessage roundtrip tests ──────────────────────────────────────────

/// Verify that `WorkerMessage::Ping { seq: 42 }` roundtrips correctly.
#[test]
fn ping_roundtrip() {
    let msg = WorkerMessage::Ping { seq: 42 };
    let encoded = encode_message(&msg).expect("encode Ping");
    let decoded: WorkerMessage = rmp_serde::from_slice(&encoded).expect("decode Ping");
    assert!(
        matches!(decoded, WorkerMessage::Ping { seq: 42 }),
        "expected Ping {{ seq: 42 }}, got {decoded:?}"
    );
}

/// Verify that `WorkerMessage::Shutdown` roundtrips correctly.
#[test]
fn shutdown_roundtrip() {
    let msg = WorkerMessage::Shutdown;
    let encoded = encode_message(&msg).expect("encode Shutdown");
    let decoded: WorkerMessage = rmp_serde::from_slice(&encoded).expect("decode Shutdown");
    assert!(
        matches!(decoded, WorkerMessage::Shutdown),
        "expected Shutdown, got {decoded:?}"
    );
}

/// Verify that `WorkerMessage::Execute` with a full graph roundtrips correctly.
#[test]
fn execute_roundtrip() {
    let job_id = Uuid::new_v4();
    let graph = json!({
        "nodes": [
            {"id": "1", "type": "KSampler", "inputs": {"model": "2"}}
        ],
        "links": [["2", "0", "1", "0"]]
    });
    let settings = JobSettings {
        device_preference: Some("cuda".to_string()),
    };
    let msg = WorkerMessage::Execute {
        job_id,
        graph,
        settings,
        device_index: 0,
    };
    let encoded = encode_message(&msg).expect("encode Execute");
    let decoded: WorkerMessage = rmp_serde::from_slice(&encoded).expect("decode Execute");
    if let WorkerMessage::Execute {
        job_id: d_job_id,
        graph: d_graph,
        settings: d_settings,
        device_index: d_device_index,
    } = decoded
    {
        assert_eq!(d_job_id, job_id);
        assert_eq!(d_device_index, 0);
        assert_eq!(d_settings.device_preference, Some("cuda".to_string()));
        assert_eq!(
            d_graph.get("nodes").and_then(|n| n.as_array()),
            Some(&vec![
                json!({"id": "1", "type": "KSampler", "inputs": {"model": "2"}})
            ])
        );
    } else {
        panic!("expected Execute variant, got {decoded:?}");
    }
}

/// Verify that `WorkerMessage::CancelJob` roundtrips correctly.
#[test]
fn cancel_job_roundtrip() {
    let job_id = Uuid::new_v4();
    let msg = WorkerMessage::CancelJob { job_id };
    let encoded = encode_message(&msg).expect("encode CancelJob");
    let decoded: WorkerMessage = rmp_serde::from_slice(&encoded).expect("decode CancelJob");
    if let WorkerMessage::CancelJob { job_id: d_job_id } = decoded {
        assert_eq!(d_job_id, job_id);
    } else {
        panic!("expected CancelJob variant, got {decoded:?}");
    }
}

/// Verify that `WorkerMessage::MemoryQuery` roundtrips correctly.
#[test]
fn memory_query_roundtrip() {
    let msg = WorkerMessage::MemoryQuery;
    let encoded = encode_message(&msg).expect("encode MemoryQuery");
    let decoded: WorkerMessage = rmp_serde::from_slice(&encoded).expect("decode MemoryQuery");
    assert!(
        matches!(decoded, WorkerMessage::MemoryQuery),
        "expected MemoryQuery, got {decoded:?}"
    );
}

// ── WorkerEvent roundtrip tests ────────────────────────────────────────────

/// Verify that `WorkerEvent::Ready` with full device info and node types roundtrips correctly.
#[test]
fn ready_roundtrip() {
    let node_types = vec![NodeTypeDescriptor {
        type_name: "KSampler".to_string(),
        display_name: "KSampler".to_string(),
        category: "sampling".to_string(),
        description: "Samples an image from a latent using a KSampler node.".to_string(),
        inputs: vec![],
        outputs: vec![],
    }];
    let event = WorkerEvent::Ready {
        worker_id: "worker-0".to_string(),
        device_index: 0,
        device_name: "NVIDIA RTX 4090".to_string(),
        device_type: "cuda".to_string(),
        vram_total_mib: 24576,
        vram_free_mib: 24000,
        torch_version: "2.5.1".to_string(),
        fp16: true,
        bf16: true,
        fp8: false,
        flash_attention: true,
        node_types,
    };
    let encoded = rmp_serde::to_vec_named(&event).expect("encode Ready");
    let decoded = decode_event(&encoded).expect("decode Ready");
    if let WorkerEvent::Ready {
        worker_id: d_worker_id,
        device_index: d_device_index,
        device_name: d_device_name,
        device_type: d_device_type,
        vram_total_mib: d_vram_total,
        vram_free_mib: d_vram_free,
        torch_version: d_torch_version,
        fp16: d_fp16,
        bf16: d_bf16,
        fp8: d_fp8,
        flash_attention: d_flash_attention,
        node_types: d_node_types,
    } = decoded
    {
        assert_eq!(d_worker_id, "worker-0");
        assert_eq!(d_device_index, 0);
        assert_eq!(d_device_name, "NVIDIA RTX 4090");
        assert_eq!(d_device_type, "cuda");
        assert_eq!(d_vram_total, 24576);
        assert_eq!(d_vram_free, 24000);
        assert_eq!(d_torch_version, "2.5.1");
        assert!(d_fp16);
        assert!(d_bf16);
        assert!(!d_fp8);
        assert!(d_flash_attention);
        assert_eq!(d_node_types.len(), 1);
        assert_eq!(d_node_types[0].type_name, "KSampler");
    } else {
        panic!("expected Ready variant, got {decoded:?}");
    }
}

/// Verify that `WorkerEvent::Pong { seq: 42 }` roundtrips correctly.
#[test]
fn pong_roundtrip() {
    let event = WorkerEvent::Pong { seq: 42 };
    let encoded = rmp_serde::to_vec_named(&event).expect("encode Pong");
    let decoded = decode_event(&encoded).expect("decode Pong");
    if let WorkerEvent::Pong { seq: d_seq } = decoded {
        assert_eq!(d_seq, 42);
    } else {
        panic!("expected Pong variant, got {decoded:?}");
    }
}

/// Verify that `WorkerEvent::Dying` with a reason string roundtrips correctly.
#[test]
fn dying_roundtrip() {
    let event = WorkerEvent::Dying {
        reason: "SIGTERM".to_string(),
    };
    let encoded = rmp_serde::to_vec_named(&event).expect("encode Dying");
    let decoded = decode_event(&encoded).expect("decode Dying");
    if let WorkerEvent::Dying { reason: d_reason } = decoded {
        assert_eq!(d_reason, "SIGTERM");
    } else {
        panic!("expected Dying variant, got {decoded:?}");
    }
}

/// Verify that `WorkerEvent::Completed` roundtrips correctly.
#[test]
fn completed_roundtrip() {
    let job_id = Uuid::new_v4();
    let event = WorkerEvent::Completed {
        job_id,
        elapsed_ms: 1234,
    };
    let encoded = rmp_serde::to_vec_named(&event).expect("encode Completed");
    let decoded = decode_event(&encoded).expect("decode Completed");
    if let WorkerEvent::Completed {
        job_id: d_job_id,
        elapsed_ms: d_elapsed_ms,
    } = decoded
    {
        assert_eq!(d_job_id, job_id);
        assert_eq!(d_elapsed_ms, 1234);
    } else {
        panic!("expected Completed variant, got {decoded:?}");
    }
}

/// Verify that `WorkerEvent::Failed` with error and traceback roundtrips correctly.
#[test]
fn failed_roundtrip() {
    let job_id = Uuid::new_v4();
    let event = WorkerEvent::Failed {
        job_id,
        error: "OOM".to_string(),
        traceback: Some("Traceback (most recent call last):\n  ...".to_string()),
    };
    let encoded = rmp_serde::to_vec_named(&event).expect("encode Failed");
    let decoded = decode_event(&encoded).expect("decode Failed");
    if let WorkerEvent::Failed {
        job_id: d_job_id,
        error: d_error,
        traceback: d_traceback,
    } = decoded
    {
        assert_eq!(d_job_id, job_id);
        assert_eq!(d_error, "OOM");
        assert_eq!(
            d_traceback,
            Some("Traceback (most recent call last):\n  ...".to_string())
        );
    } else {
        panic!("expected Failed variant, got {decoded:?}");
    }
}

/// Verify that `WorkerEvent::Cancelled` roundtrips correctly.
#[test]
fn cancelled_roundtrip() {
    let job_id = Uuid::new_v4();
    let event = WorkerEvent::Cancelled { job_id };
    let encoded = rmp_serde::to_vec_named(&event).expect("encode Cancelled");
    let decoded = decode_event(&encoded).expect("decode Cancelled");
    if let WorkerEvent::Cancelled { job_id: d_job_id } = decoded {
        assert_eq!(d_job_id, job_id);
    } else {
        panic!("expected Cancelled variant, got {decoded:?}");
    }
}

/// Verify that `WorkerEvent::ImageReady` with all fields roundtrips correctly.
#[test]
fn image_ready_roundtrip() {
    let job_id = Uuid::new_v4();
    let event = WorkerEvent::ImageReady {
        job_id,
        image_b64: "dGVzdCBpbWFnZQ==".to_string(),
        width: 512,
        height: 512,
        format: "png".to_string(),
        seed: 42,
        steps: 20,
    };
    let encoded = rmp_serde::to_vec_named(&event).expect("encode ImageReady");
    let decoded = decode_event(&encoded).expect("decode ImageReady");
    if let WorkerEvent::ImageReady {
        job_id: d_job_id,
        image_b64: d_image_b64,
        width: d_width,
        height: d_height,
        format: d_format,
        seed: d_seed,
        steps: d_steps,
    } = decoded
    {
        assert_eq!(d_job_id, job_id);
        assert_eq!(d_image_b64, "dGVzdCBpbWFnZQ==");
        assert_eq!(d_width, 512);
        assert_eq!(d_height, 512);
        assert_eq!(d_format, "png");
        assert_eq!(d_seed, 42);
        assert_eq!(d_steps, 20);
    } else {
        panic!("expected ImageReady variant, got {decoded:?}");
    }
}

/// Verify that `WorkerEvent::Progress` with optional preview roundtrips correctly.
#[test]
fn progress_roundtrip() {
    let job_id = Uuid::new_v4();
    let event = WorkerEvent::Progress {
        job_id,
        step: 5,
        total_steps: 20,
        preview_b64: None,
    };
    let encoded = rmp_serde::to_vec_named(&event).expect("encode Progress");
    let decoded = decode_event(&encoded).expect("decode Progress");
    if let WorkerEvent::Progress {
        job_id: d_job_id,
        step: d_step,
        total_steps: d_total_steps,
        preview_b64: d_preview_b64,
    } = decoded
    {
        assert_eq!(d_job_id, job_id);
        assert_eq!(d_step, 5);
        assert_eq!(d_total_steps, 20);
        assert!(d_preview_b64.is_none());
    } else {
        panic!("expected Progress variant, got {decoded:?}");
    }
}

/// Verify that `WorkerEvent::Progress` with a preview image roundtrips correctly.
#[test]
fn progress_with_preview_roundtrip() {
    let job_id = Uuid::new_v4();
    let event = WorkerEvent::Progress {
        job_id,
        step: 10,
        total_steps: 20,
        preview_b64: Some("aW1hZ2UgZGF0YQ==".to_string()),
    };
    let encoded = rmp_serde::to_vec_named(&event).expect("encode Progress with preview");
    let decoded = decode_event(&encoded).expect("decode Progress with preview");
    if let WorkerEvent::Progress {
        job_id: d_job_id,
        step: d_step,
        total_steps: d_total_steps,
        preview_b64: d_preview_b64,
    } = decoded
    {
        assert_eq!(d_job_id, job_id);
        assert_eq!(d_step, 10);
        assert_eq!(d_total_steps, 20);
        assert_eq!(d_preview_b64, Some("aW1hZ2UgZGF0YQ==".to_string()));
    } else {
        panic!("expected Progress variant, got {decoded:?}");
    }
}

/// Verify that `WorkerEvent::MemoryReport` roundtrips correctly.
#[test]
fn memory_report_roundtrip() {
    let event = WorkerEvent::MemoryReport {
        vram_used_mib: 4096,
        ram_used_mib: 8192,
    };
    let encoded = rmp_serde::to_vec_named(&event).expect("encode MemoryReport");
    let decoded = decode_event(&encoded).expect("decode MemoryReport");
    if let WorkerEvent::MemoryReport {
        vram_used_mib: d_vram,
        ram_used_mib: d_ram,
    } = decoded
    {
        assert_eq!(d_vram, 4096);
        assert_eq!(d_ram, 8192);
    } else {
        panic!("expected MemoryReport variant, got {decoded:?}");
    }
}

/// Verify that `encode_message` returns a non-empty byte vector for each variant.
#[test]
fn encode_produces_non_empty_bytes() {
    let messages = [
        WorkerMessage::Ping { seq: 0 },
        WorkerMessage::Shutdown,
        WorkerMessage::MemoryQuery,
        WorkerMessage::CancelJob {
            job_id: Uuid::new_v4(),
        },
        WorkerMessage::Execute {
            job_id: Uuid::new_v4(),
            graph: json!({}),
            settings: JobSettings::default(),
            device_index: 0,
        },
    ];
    for msg in &messages {
        let encoded = encode_message(msg).expect("encode");
        assert!(!encoded.is_empty(), "encoded message must not be empty");
    }
}

/// Verify that `IpcError` variants display correctly.
#[test]
fn ipc_error_display() {
    let serialize_err = anvilml_ipc::IpcError::Serialize("test error".to_string());
    let deserialize_err = anvilml_ipc::IpcError::Deserialize("test error".to_string());
    assert!(
        format!("{serialize_err}").contains("test error"),
        "Serialize error display should contain the message"
    );
    assert!(
        format!("{deserialize_err}").contains("test error"),
        "Deserialize error display should contain the message"
    );
}
