use anvilml_core::error::AnvilError;
use serde_json::Value as JsonValue;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::{WorkerEvent, WorkerMessage};

/// Serialize a `WorkerMessage` into a flat dict compatible with Python's
/// msgpack serialization (uses `_type` as the variant discriminator).
fn serialize_message(msg: &WorkerMessage) -> serde_json::Map<String, JsonValue> {
    let mut map = serde_json::Map::new();
    match msg {
        WorkerMessage::Ping { seq } => {
            map.insert("_type".into(), "Ping".into());
            map.insert("seq".into(), JsonValue::Number((*seq).into()));
        }
        WorkerMessage::Shutdown => {
            map.insert("_type".into(), "Shutdown".into());
        }
        WorkerMessage::InitializeHardware { device_str } => {
            map.insert("_type".into(), "InitializeHardware".into());
            map.insert("device_str".into(), JsonValue::String(device_str.clone()));
        }
        WorkerMessage::Execute {
            job_id,
            graph,
            settings,
            device_index,
        } => {
            map.insert("_type".into(), "Execute".into());
            map.insert("job_id".into(), JsonValue::String(job_id.to_string()));
            map.insert("graph".into(), graph.clone());
            // Serialize JobSettings as a flat dict
            let mut settings_map = serde_json::Map::new();
            settings_map.insert("seed".into(), JsonValue::Number(settings.seed.into()));
            settings_map.insert("steps".into(), JsonValue::Number(settings.steps.into()));
            let gs = settings.guidance_scale as f64;
            map.insert(
                "guidance_scale".into(),
                JsonValue::Number(
                    serde_json::Number::from_f64(gs).unwrap_or_else(|| serde_json::Number::from(1)),
                ),
            );
            settings_map.insert("width".into(), JsonValue::Number(settings.width.into()));
            settings_map.insert("height".into(), JsonValue::Number(settings.height.into()));
            if let Some(ref dp) = settings.device_preference {
                settings_map.insert("device_preference".into(), JsonValue::Number((*dp).into()));
            }
            map.insert("settings".into(), JsonValue::Object(settings_map));
            map.insert(
                "device_index".into(),
                JsonValue::Number((*device_index).into()),
            );
        }
        WorkerMessage::CancelJob { job_id } => {
            map.insert("_type".into(), "CancelJob".into());
            map.insert("job_id".into(), JsonValue::String(job_id.to_string()));
        }
        WorkerMessage::MemoryQuery => {
            map.insert("_type".into(), "MemoryQuery".into());
        }
    }
    map
}

/// Write a single length-prefixed msgpack frame to the given async sink.
///
/// The frame layout is:
///   - 4 bytes: payload length as big-endian `u32`
///   - N bytes: msgpack-encoded flat dict with `_type` discriminator
pub async fn write_frame<W>(w: &mut W, msg: &WorkerMessage) -> Result<(), AnvilError>
where
    W: AsyncWrite + Unpin,
{
    let map = serialize_message(msg);
    // Serialize the flat dict using msgpack-compatible serialization
    let payload = rmp_serde::to_vec_named(&map).map_err(|e| {
        tracing::error!(error = %e, "IPC frame write failed");
        AnvilError::Json(e.to_string())
    })?;
    let len = payload.len() as u32;
    let header = len.to_be_bytes();
    w.write_all(&header).await.map_err(|e| {
        tracing::error!(error = %e, "IPC frame write failed");
        AnvilError::Io(e)
    })?;
    w.write_all(&payload).await.map_err(|e| {
        tracing::error!(error = %e, "IPC frame write failed");
        AnvilError::Io(e)
    })?;
    Ok(())
}

/// Read a single length-prefixed msgpack frame from the given async source.
///
/// The frame layout is:
///   - 4 bytes: payload length as big-endian `u32`
///   - N bytes: msgpack-encoded `WorkerEvent` (via `rmp_serde::from_slice`)
///
/// The `max_mib` parameter enforces a size cap (in MiB) on the payload
/// *before* allocating the buffer, preventing a malicious header from
/// triggering gigabyte-scale allocation.
pub async fn read_frame<R>(r: &mut R, max_mib: u32) -> Result<WorkerEvent, AnvilError>
where
    R: AsyncRead + Unpin,
{
    // 1. Read exactly 4 bytes for the length header.
    let mut header = [0u8; 4];
    r.read_exact(&mut header).await?;

    // 2. Decode big-endian u32 payload length.
    let len = u32::from_be_bytes(header);

    // 3. Enforce size cap BEFORE allocating the payload buffer.
    let max_bytes = (max_mib as u64) * 1024 * 1024;
    let payload_len = len as u64;
    if payload_len > max_bytes {
        tracing::warn!(
            payload_mib = payload_len / 1024 / 1024,
            limit_mib = max_mib,
            "IPC frame rejected: payload too large"
        );
        return Err(AnvilError::PayloadTooLarge(format!(
            "frame length {} exceeds limit {} MiB",
            len, max_mib
        )));
    }

    // 4. Allocate and read exactly N payload bytes.
    let mut payload = vec![0u8; len as usize];
    r.read_exact(&mut payload).await?;

    // 5. Deserialize msgpack → WorkerEvent.
    //
    // Python sends a flat dict with `_type` as the variant discriminator
    // (e.g. `{"_type": "Ready", "worker_id": "...", ...}`).
    // rmp_serde's default enum deserialization expects a nested format
    // (`{"Ready": [...]}`), so we deserialize into a generic map first,
    // then reconstruct the WorkerEvent from the flat dict.
    let map =
        rmp_serde::from_slice::<serde_json::Map<String, JsonValue>>(&payload).map_err(|e| {
            tracing::error!(error = %e, "IPC frame deserialize failed");
            AnvilError::Json(e.to_string())
        })?;

    let event = worker_event_from_map(&map).map_err(|e| {
        tracing::error!(error = %e, "IPC frame deserialize failed");
        AnvilError::Json(e.to_string())
    })?;

    Ok(event)
}

/// Deserialize a flat dict (from Python's msgpack) into a WorkerEvent.
///
/// The dict uses `_type` as the variant discriminator and has fields at
/// the top level (e.g. `{"_type": "Ready", "worker_id": "...", ...}`).
fn worker_event_from_map(map: &serde_json::Map<String, JsonValue>) -> Result<WorkerEvent, String> {
    let _type = map
        .get("_type")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "_type field missing or not a string".to_string())?;

    match _type {
        "Ready" => Ok(WorkerEvent::Ready {
            worker_id: map
                .get("worker_id")
                .and_then(|v| v.as_str())
                .ok_or("worker_id missing")?
                .to_string(),
            device_index: map
                .get("device_index")
                .and_then(|v| v.as_u64())
                .ok_or("device_index missing")? as u32,
            vram_total_mib: map
                .get("vram_total_mib")
                .and_then(|v| v.as_u64())
                .ok_or("vram_total_mib missing")? as u32,
            vram_free_mib: map
                .get("vram_free_mib")
                .and_then(|v| v.as_u64())
                .ok_or("vram_free_mib missing")? as u32,
            arch: map
                .get("arch")
                .and_then(|v| v.as_str())
                .ok_or("arch missing")?
                .to_string(),
            fp16: map
                .get("fp16")
                .and_then(|v| v.as_bool())
                .ok_or("fp16 missing")?,
            bf16: map
                .get("bf16")
                .and_then(|v| v.as_bool())
                .ok_or("bf16 missing")?,
            flash_attention: map
                .get("flash_attention")
                .and_then(|v| v.as_bool())
                .ok_or("flash_attention missing")?,
        }),
        "Pong" => Ok(WorkerEvent::Pong {
            seq: map
                .get("seq")
                .and_then(|v| v.as_u64())
                .ok_or("seq missing")?,
        }),
        "Dying" => Ok(WorkerEvent::Dying {
            reason: map
                .get("reason")
                .and_then(|v| v.as_str())
                .ok_or("reason missing")?
                .to_string(),
        }),
        "MemoryReport" => Ok(WorkerEvent::MemoryReport {
            vram_used_mib: map
                .get("vram_used_mib")
                .and_then(|v| v.as_u64())
                .ok_or("vram_used_mib missing")? as u32,
            ram_used_mib: map
                .get("ram_used_mib")
                .and_then(|v| v.as_u64())
                .ok_or("ram_used_mib missing")?,
        }),
        "Progress" => Ok(WorkerEvent::Progress {
            job_id: uuid::Uuid::parse_str(
                map.get("job_id")
                    .and_then(|v| v.as_str())
                    .ok_or("job_id missing")?,
            )
            .map_err(|e| format!("invalid job_id: {}", e))?,
            node_index: map
                .get("node_index")
                .and_then(|v| v.as_u64())
                .ok_or("node_index missing")? as u32,
            node_total: map
                .get("node_total")
                .and_then(|v| v.as_u64())
                .ok_or("node_total missing")? as u32,
            node_type: map
                .get("node_type")
                .and_then(|v| v.as_str())
                .ok_or("node_type missing")?
                .to_string(),
            step: map.get("step").and_then(|v| v.as_u64()).map(|v| v as u32),
            step_total: map
                .get("step_total")
                .and_then(|v| v.as_u64())
                .map(|v| v as u32),
        }),
        "ImageReady" => Ok(WorkerEvent::ImageReady {
            job_id: uuid::Uuid::parse_str(
                map.get("job_id")
                    .and_then(|v| v.as_str())
                    .ok_or("job_id missing")?,
            )
            .map_err(|e| format!("invalid job_id: {}", e))?,
            image_b64: map
                .get("image_b64")
                .and_then(|v| v.as_str())
                .ok_or("image_b64 missing")?
                .to_string(),
            width: map
                .get("width")
                .and_then(|v| v.as_u64())
                .ok_or("width missing")? as u32,
            height: map
                .get("height")
                .and_then(|v| v.as_u64())
                .ok_or("height missing")? as u32,
            format: map
                .get("format")
                .and_then(|v| v.as_str())
                .ok_or("format missing")?
                .to_string(),
            seed: map
                .get("seed")
                .and_then(|v| v.as_i64())
                .ok_or("seed missing")?,
            steps: map
                .get("steps")
                .and_then(|v| v.as_u64())
                .ok_or("steps missing")? as u32,
            prompt: map
                .get("prompt")
                .and_then(|v| v.as_str())
                .ok_or("prompt missing")?
                .to_string(),
        }),
        "Completed" => Ok(WorkerEvent::Completed {
            job_id: uuid::Uuid::parse_str(
                map.get("job_id")
                    .and_then(|v| v.as_str())
                    .ok_or("job_id missing")?,
            )
            .map_err(|e| format!("invalid job_id: {}", e))?,
            elapsed_ms: map
                .get("elapsed_ms")
                .and_then(|v| v.as_u64())
                .ok_or("elapsed_ms missing")?,
        }),
        "Failed" => Ok(WorkerEvent::Failed {
            job_id: uuid::Uuid::parse_str(
                map.get("job_id")
                    .and_then(|v| v.as_str())
                    .ok_or("job_id missing")?,
            )
            .map_err(|e| format!("invalid job_id: {}", e))?,
            error: map
                .get("error")
                .and_then(|v| v.as_str())
                .ok_or("error missing")?
                .to_string(),
            traceback: map
                .get("traceback")
                .and_then(|v| v.as_str())
                .ok_or("traceback missing")?
                .to_string(),
        }),
        "Cancelled" => Ok(WorkerEvent::Cancelled {
            job_id: uuid::Uuid::parse_str(
                map.get("job_id")
                    .and_then(|v| v.as_str())
                    .ok_or("job_id missing")?,
            )
            .map_err(|e| format!("invalid job_id: {}", e))?,
        }),
        _ => Err(format!("unknown event type: {}", _type)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn write_frame() {
        let msg = WorkerMessage::Ping { seq: 7 };

        // Serialize using the same format as write_frame (flat dict via serialize_message).
        let expected_map = super::serialize_message(&msg);
        let expected_payload = rmp_serde::to_vec_named(&expected_map).expect("serialize");
        let payload_len = expected_payload.len();

        // Write frame to Vec<u8>
        let mut buf = Vec::new();
        super::write_frame(&mut buf, &msg)
            .await
            .expect("write_frame");

        // Total buffer should be 4-byte header + payload
        assert_eq!(buf.len(), 4 + payload_len);

        // First 4 bytes must equal payload length as big-endian u32
        let mut header = [0u8; 4];
        header.copy_from_slice(&buf[0..4]);
        let decoded_len = u32::from_be_bytes(header);
        assert_eq!(decoded_len, payload_len as u32);

        // Payload bytes must match the serialized message
        assert_eq!(&buf[4..], &expected_payload[..]);
    }

    #[test]
    fn write_frame_sync_serialization() {
        // Verify that serialization itself works outside async context.
        let msg = WorkerMessage::Shutdown;
        let map = super::serialize_message(&msg);
        let payload = rmp_serde::to_vec_named(&map).expect("serialize");
        assert!(!payload.is_empty());
    }

    #[tokio::test]
    async fn write_frame_shutdown() {
        let msg = WorkerMessage::Shutdown;
        let mut buf = Vec::new();
        super::write_frame(&mut buf, &msg)
            .await
            .expect("write_frame");

        let expected_map = super::serialize_message(&msg);
        let payload = rmp_serde::to_vec_named(&expected_map).expect("serialize");
        assert_eq!(buf.len(), 4 + payload.len());

        let mut header = [0u8; 4];
        header.copy_from_slice(&buf[0..4]);
        let decoded_len = u32::from_be_bytes(header);
        assert_eq!(decoded_len, payload.len() as u32);
    }

    #[tokio::test]
    async fn write_frame_execute() {
        use uuid::Uuid;

        let job_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let graph = serde_json::json!({ "nodes": [] });
        let settings = anvilml_core::types::job::JobSettings {
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

        let mut buf = Vec::new();
        super::write_frame(&mut buf, &msg)
            .await
            .expect("write_frame");

        let expected_map = super::serialize_message(&msg);
        let payload = rmp_serde::to_vec_named(&expected_map).expect("serialize");
        assert_eq!(buf.len(), 4 + payload.len());

        let mut header = [0u8; 4];
        header.copy_from_slice(&buf[0..4]);
        let decoded_len = u32::from_be_bytes(header);
        assert_eq!(decoded_len, payload.len() as u32);
    }

    #[tokio::test]
    async fn read_frame_roundtrip() {
        // Create a duplex (bidirectional) in-memory pipe.
        let (mut tx, mut rx) = tokio::io::duplex(4096);

        // Write a Pong event frame through the write side using Python-compatible
        // flat dict format (same as what the Python worker sends).
        let pong_json = serde_json::json!({ "_type": "Pong", "seq": 7u64 });
        let payload = rmp_serde::to_vec_named(&pong_json).expect("serialize");
        let len = payload.len() as u32;
        let header = len.to_be_bytes();
        tx.write_all(&header).await.expect("write header");
        tx.write_all(&payload).await.expect("write payload");

        // Read it back through the read side.
        let result = super::read_frame(&mut rx, 64).await.expect("read_frame");

        // Verify the round-tripped event.
        assert_eq!(result, WorkerEvent::Pong { seq: 7 });
    }

    #[tokio::test]
    async fn read_frame_oversize_rejected() {
        // Construct a 4-byte header claiming ~4 GiB with zero payload bytes.
        let oversized_header: [u8; 4] = [0xFF, 0xFF, 0xFF, 0xFF];
        let buf: Vec<u8> = oversized_header.into();

        let mut cursor = std::io::Cursor::new(buf);

        let result = super::read_frame(&mut cursor, 64).await;

        // Should reject before allocating or reading payload.
        match result {
            Err(AnvilError::PayloadTooLarge(msg)) => {
                assert!(msg.contains("4294967295"));
                assert!(msg.contains("64"));
            }
            other => panic!("Expected PayloadTooLarge, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn read_frame_python_format() {
        // Simulate what Python sends: a flat dict with `_type` key.
        let event_json = serde_json::json!({
            "_type": "Ready",
            "worker_id": "test-worker",
            "device_index": 0u64,
            "vram_total_mib": 8192u64,
            "vram_free_mib": 8192u64,
            "arch": "gfx1100",
            "fp16": true,
            "bf16": true,
            "flash_attention": false,
        });
        // Serialize as msgpack (what Python does)
        let payload = rmp_serde::to_vec_named(&event_json).expect("serialize json");
        let len = payload.len() as u32;

        // Write through a duplex pipe.
        let (mut tx, mut rx) = tokio::io::duplex(4096);
        let header = len.to_be_bytes();
        tx.write_all(&header).await.expect("write header");
        tx.write_all(&payload).await.expect("write payload");

        // Read it back.
        let result = super::read_frame(&mut rx, 64).await.expect("read_frame");

        assert!(matches!(result, WorkerEvent::Ready { .. }));
    }
}
