/// Integration tests for `types::events` — the `WsEvent` enum and all ten
/// sub-event variants.
///
/// Verifies:
/// - Full JSON roundtrip for `JobImageReady` (the most complex variant, 6 fields).
/// - The `"type"` discriminator key appears in serialised JSON with the correct
///   snake_case variant name.
/// - All 10 enum variants survive JSON roundtrip without data loss.
/// - `SystemStats` roundtrip including nested `Vec<WorkerInfo>`.
use anvilml_core::{types::events::WsEvent, WorkerInfo, WorkerStatus};
use uuid::Uuid;

/// Verifies that a fully-populated `JobImageReady` event serialises to JSON
/// and deserialises back to an identical value, including all six fields
/// (`job_id`, `artifact_hash`, `width`, `height`, `seed`, `steps`).
///
/// This is the primary acceptance test for the most data-rich WsEvent variant.
#[test]
fn test_ws_event_roundtrip_job_image_ready() {
    let job_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
    let original = WsEvent::JobImageReady {
        job_id,
        artifact_hash: "a1b2c3d4e5f6".to_string(),
        width: 1024,
        height: 768,
        seed: 42,
        steps: 30,
    };

    // Serialize to JSON
    let json = serde_json::to_string(&original).expect("serialize JobImageReady to JSON");

    // Deserialize back — must not fail
    let restored: WsEvent = serde_json::from_str(&json).expect("deserialize JSON back to WsEvent");

    // All fields must be equal
    if let WsEvent::JobImageReady {
        job_id: r_job_id,
        artifact_hash: r_hash,
        width: r_width,
        height: r_height,
        seed: r_seed,
        steps: r_steps,
    } = restored
    {
        assert_eq!(r_job_id, job_id);
        assert_eq!(r_hash, "a1b2c3d4e5f6");
        assert_eq!(r_width, 1024);
        assert_eq!(r_height, 768);
        assert_eq!(r_seed, 42);
        assert_eq!(r_steps, 30);
    } else {
        panic!("expected JobImageReady variant, got {:?}", restored);
    }
}

/// Verifies that the `"type"` discriminator key appears in serialised JSON
/// with the correct snake_case variant name for `JobQueued`.
///
/// This directly tests the `#[serde(tag = "type", rename_all = "snake_case")]`
/// attribute on `WsEvent`, confirming that the variant name is lowercased
/// and the discriminator key is `"type"` (not `"_type"`).
#[test]
fn test_ws_event_tag_field_present() {
    let job_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
    let event = WsEvent::JobQueued {
        job_id,
        queue_position: 1,
    };

    // Serialize to JSON
    let json = serde_json::to_string(&event).expect("serialize JobQueued to JSON");

    // Parse as a generic JSON object to inspect the "type" key
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("parse JSON as Value");

    // The "type" key must exist and equal "job_queued"
    let type_value = parsed
        .get("type")
        .expect("\"type\" key must be present in serialised WsEvent");

    assert_eq!(
        type_value, "job_queued",
        "WsEvent tag must serialise as \"job_queued\" (snake_case), got: {}",
        type_value
    );
}

/// Iterates over all 10 `WsEvent` variants, serialises each to JSON,
/// deserialises back, and asserts equality.
///
/// This ensures no variant has a serde mapping bug. Each variant is
/// constructed with minimal but non-default values to keep the test concise.
#[test]
fn test_ws_event_all_variants_roundtrip() {
    let job_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();

    let variants: Vec<WsEvent> = vec![
        WsEvent::JobQueued {
            job_id,
            queue_position: 1,
        },
        WsEvent::JobStarted {
            job_id,
            worker_id: "worker-0".to_string(),
        },
        WsEvent::JobProgress {
            job_id,
            step: 5,
            total_steps: 30,
            preview_b64: Some("dGVzdA==".to_string()), // "test" in base64
        },
        WsEvent::JobImageReady {
            job_id,
            artifact_hash: "abc123".to_string(),
            width: 512,
            height: 512,
            seed: 12345,
            steps: 20,
        },
        WsEvent::JobCompleted {
            job_id,
            elapsed_ms: 12345,
        },
        WsEvent::JobFailed {
            job_id,
            error: "out of memory".to_string(),
        },
        WsEvent::JobCancelled { job_id },
        WsEvent::WorkerStatusChanged {
            worker_id: "worker-1".to_string(),
            status: WorkerStatus::Busy,
            device_index: 2,
        },
        WsEvent::SystemStats {
            cpu_pct: 45.5,
            ram_used_mib: 8192,
            workers: vec![WorkerInfo {
                id: "worker-0".to_string(),
                device_index: 0,
                device_name: "Mock GPU".to_string(),
                status: WorkerStatus::Idle,
                current_job_id: None,
                vram_used_mib: None,
            }],
        },
        WsEvent::ProvisioningProgress {
            message: "Installing torch".to_string(),
            pct: 50,
        },
    ];

    for (i, original) in variants.iter().enumerate() {
        let json = serde_json::to_string(&original).expect("serialize variant");
        let restored: WsEvent =
            serde_json::from_str(&json).expect("deserialize JSON back to WsEvent");
        assert_eq!(
            restored,
            original.clone(),
            "Variant {:?} (index {}) did not survive JSON roundtrip (JSON was: {})",
            std::mem::discriminant(original),
            i,
            json
        );
    }
}

/// Verifies that a `SystemStats` event with a `Vec<WorkerInfo>` containing
/// two workers roundtrips through JSON correctly, including all nested
/// fields of each worker.
///
/// This tests that the enum correctly handles cross-type references —
/// `WorkerInfo` must implement `Serialize`/`Deserialize` for this to work.
#[test]
fn test_ws_event_system_stats_roundtrip() {
    let workers = vec![
        WorkerInfo {
            id: "worker-0".to_string(),
            device_index: 0,
            device_name: "NVIDIA A100-SXM4-40GB".to_string(),
            status: WorkerStatus::Idle,
            current_job_id: None,
            vram_used_mib: Some(1024),
        },
        WorkerInfo {
            id: "worker-1".to_string(),
            device_index: 1,
            device_name: "NVIDIA A100-SXM4-40GB".to_string(),
            status: WorkerStatus::Busy,
            current_job_id: Some(Uuid::parse_str("550e8400-e29b-41d4-a716-446655440001").unwrap()),
            vram_used_mib: Some(12288),
        },
    ];

    let original = WsEvent::SystemStats {
        cpu_pct: 67.3,
        ram_used_mib: 16384,
        workers,
    };

    // Serialize to JSON
    let json = serde_json::to_string(&original).expect("serialize SystemStats to JSON");

    // Deserialize back — must not fail
    let restored: WsEvent = serde_json::from_str(&json).expect("deserialize JSON back to WsEvent");

    // Verify it is a SystemStats variant
    if let WsEvent::SystemStats {
        cpu_pct: r_cpu,
        ram_used_mib: r_ram,
        workers: r_workers,
    } = restored
    {
        assert!((r_cpu - 67.3).abs() < f32::EPSILON);
        assert_eq!(r_ram, 16384);
        assert_eq!(r_workers.len(), 2);

        // First worker
        assert_eq!(r_workers[0].id, "worker-0");
        assert_eq!(r_workers[0].device_index, 0);
        assert_eq!(r_workers[0].device_name, "NVIDIA A100-SXM4-40GB");
        assert_eq!(r_workers[0].status, WorkerStatus::Idle);
        assert_eq!(r_workers[0].current_job_id, None);
        assert_eq!(r_workers[0].vram_used_mib, Some(1024));

        // Second worker
        assert_eq!(r_workers[1].id, "worker-1");
        assert_eq!(r_workers[1].device_index, 1);
        assert_eq!(r_workers[1].device_name, "NVIDIA A100-SXM4-40GB");
        assert_eq!(r_workers[1].status, WorkerStatus::Busy);
        assert!(r_workers[1].current_job_id.is_some());
        assert_eq!(r_workers[1].vram_used_mib, Some(12288));
    } else {
        panic!("expected SystemStats variant, got {:?}", restored);
    }
}
