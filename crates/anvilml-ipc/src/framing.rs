use anvilml_core::error::AnvilError;
use tokio::io::{AsyncWrite, AsyncWriteExt};

use crate::WorkerMessage;

/// Write a single length-prefixed msgpack frame to the given async sink.
///
/// The frame layout is:
///   - 4 bytes: payload length as big-endian `u32`
///   - N bytes: msgpack-encoded `WorkerMessage` (via `rmp_serde::to_vec_named`)
pub async fn write_frame<W>(w: &mut W, msg: &WorkerMessage) -> Result<(), AnvilError>
where
    W: AsyncWrite + Unpin,
{
    let payload = rmp_serde::to_vec_named(msg).map_err(|e| AnvilError::Json(e.to_string()))?;
    let len = payload.len() as u32;
    let header = len.to_be_bytes();
    w.write_all(&header).await.map_err(AnvilError::Io)?;
    w.write_all(&payload).await.map_err(AnvilError::Io)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn write_frame() {
        let msg = WorkerMessage::Ping { seq: 7 };

        // Serialize to get expected payload length
        let payload = rmp_serde::to_vec_named(&msg).expect("serialize");
        let payload_len = payload.len();

        // Write frame to Vec<u8>
        let mut buf = Vec::new();
        super::write_frame(&mut buf, &msg).await.expect("write_frame");

        // Total buffer should be 4-byte header + payload
        assert_eq!(buf.len(), 4 + payload_len);

        // First 4 bytes must equal payload length as big-endian u32
        let mut header = [0u8; 4];
        header.copy_from_slice(&buf[0..4]);
        let decoded_len = u32::from_be_bytes(header);
        assert_eq!(decoded_len, payload_len as u32);

        // Payload bytes must match the serialized message
        assert_eq!(&buf[4..], &payload[..]);
    }

    #[test]
    fn write_frame_sync_serialization() {
        // Verify that serialization itself works outside async context.
        let msg = WorkerMessage::Shutdown;
        let payload = rmp_serde::to_vec_named(&msg).expect("serialize");
        assert!(!payload.is_empty());
    }

    #[tokio::test]
    async fn write_frame_shutdown() {
        let msg = WorkerMessage::Shutdown;
        let mut buf = Vec::new();
        super::write_frame(&mut buf, &msg).await.expect("write_frame");

        let payload = rmp_serde::to_vec_named(&msg).expect("serialize");
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
        super::write_frame(&mut buf, &msg).await.expect("write_frame");

        let payload = rmp_serde::to_vec_named(&msg).expect("serialize");
        assert_eq!(buf.len(), 4 + payload.len());

        let mut header = [0u8; 4];
        header.copy_from_slice(&buf[0..4]);
        let decoded_len = u32::from_be_bytes(header);
        assert_eq!(decoded_len, payload.len() as u32);
    }
}
