use anvilml_core::error::AnvilError;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::{WorkerEvent, WorkerMessage};

/// Write a single length-prefixed msgpack frame to the given async sink.
///
/// The frame layout is:
///   - 4 bytes: payload length as big-endian `u32`
///   - N bytes: msgpack-encoded `WorkerMessage` (via `rmp_serde::to_vec_named`)
pub async fn write_frame<W>(w: &mut W, msg: &WorkerMessage) -> Result<(), AnvilError>
where
    W: AsyncWrite + Unpin,
{
    let payload = rmp_serde::to_vec_named(msg).map_err(|e| {
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
    let event = rmp_serde::from_slice::<WorkerEvent>(&payload).map_err(|e| {
        tracing::error!(error = %e, "IPC frame deserialize failed");
        AnvilError::Json(e.to_string())
    })?;

    Ok(event)
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
        super::write_frame(&mut buf, &msg)
            .await
            .expect("write_frame");

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
        super::write_frame(&mut buf, &msg)
            .await
            .expect("write_frame");

        let payload = rmp_serde::to_vec_named(&msg).expect("serialize");
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

        // Write a Pong event frame through the write side.
        let event = WorkerEvent::Pong { seq: 7 };
        let payload = rmp_serde::to_vec_named(&event).expect("serialize");
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
}
