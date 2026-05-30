//! Length-prefixed msgpack framing for stdin/stdout pipe communication.
//!
//! Implements the protocol from `ANVILML_DESIGN.md` §7.1:
//! each frame consists of a 4-byte big-endian u32 length prefix
//! followed by N bytes of msgpack-encoded payload.
//!
//! The framing layer enforces a configurable maximum payload size (in MiB)
//! before any heap allocation, and uses `read_exact` / `write_all` to
//! guarantee full reads/writes on all platforms including Windows where
//! pipe reads are frequently partial.

use std::io;

use anvilml_core::AnvilError;
#[cfg(test)]
use serde::Serialize;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::{WorkerEvent, WorkerMessage};

/// Maximum payload size in MiB. Payloads exceeding this limit cause
/// `AnvilError::PayloadTooLarge` before any heap allocation beyond the
/// length-prefix buffer.
#[expect(dead_code)]
const DEFAULT_MAX_PAYLOAD_MIB: u32 = 10;

// ---------------------------------------------------------------------------
// write_frame — Rust → Python (commands)
// ---------------------------------------------------------------------------

/// Encode a [`WorkerMessage`] as msgpack and write it as a single frame.
///
/// The frame layout is:
///   - 4 bytes: big-endian u32 length of the payload
///   - N bytes: msgpack-encoded `msg`
///
/// Uses `AsyncWriteExt::write_all` to guarantee the entire frame is flushed
/// in one logical write operation.
pub async fn write_frame<W: AsyncWrite + Unpin>(
    writer: &mut W,
    msg: &WorkerMessage,
) -> Result<(), AnvilError> {
    // Serialize the message as msgpack (named-map format, via rmp-serde).
    let payload = rmp_serde::to_vec_named(msg)
        .map_err(|e| AnvilError::Io(io::Error::new(io::ErrorKind::InvalidData, e)))?;

    let len = payload.len() as u32;

    // Write the 4-byte big-endian length prefix.
    writer.write_all(&len.to_be_bytes()).await?;

    // Write the payload bytes.
    writer.write_all(&payload).await?;

    Ok(())
}

// ---------------------------------------------------------------------------
// read_frame — Python → Rust (events)
// ---------------------------------------------------------------------------

/// Read a single frame from an async reader and decode it as a [`WorkerEvent`].
///
/// The frame layout is:
///   - 4 bytes: big-endian u32 length of the payload
///   - N bytes: msgpack-encoded `WorkerEvent`
///
/// # Arguments
///
/// * `reader` — async reader (e.g. stdin, pipe)
/// * `max_mib` — maximum allowed payload size in MiB. If the length prefix
///   encodes a value exceeding this limit, returns
///   `AnvilError::PayloadTooLarge` **without** attempting to read the payload.
///
/// Uses `AsyncReadExt::read_exact` to guarantee exactly 4 bytes are consumed
/// for the length prefix, then reads exactly N payload bytes.
pub async fn read_frame<R: AsyncRead + Unpin>(
    reader: &mut R,
    max_mib: u32,
) -> Result<WorkerEvent, AnvilError> {
    // Read the 4-byte big-endian length prefix.
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf).await?;

    let len = u32::from_be_bytes(len_buf);

    // Enforce the maximum payload size *before* allocating the buffer.
    let max_bytes = (max_mib as u64) * 1024 * 1024;
    if len as u64 > max_bytes {
        let size_mib = len / 1_048_576;
        return Err(AnvilError::PayloadTooLarge {
            size_mib,
            limit_mib: max_mib,
        });
    }

    // Allocate a buffer and read exactly `len` bytes.
    let mut payload = vec![0u8; len as usize];
    reader.read_exact(&mut payload).await?;

    // Deserialize from msgpack (named-map format, via rmp-serde).
    let event: WorkerEvent = rmp_serde::from_slice(&payload)
        .map_err(|e| AnvilError::Io(io::Error::new(io::ErrorKind::InvalidData, e)))?;

    Ok(event)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: write any serde-serializable value as a length-prefixed frame.
    async fn write_raw_frame<W, T>(writer: &mut W, value: &T) -> std::io::Result<()>
    where
        W: AsyncWrite + Unpin,
        T: Serialize,
    {
        let payload = rmp_serde::to_vec_named(value).expect("serialize");
        writer
            .write_all(&(payload.len() as u32).to_be_bytes())
            .await?;
        writer.write_all(&payload).await?;
        Ok(())
    }

    /// Round-trip test: write a `WorkerMessage::Ping { seq: 1 }`, read back as
    /// `WorkerEvent::Pong { seq: 1 }` through a tokio duplex pipe.
    #[tokio::test]
    async fn roundtrip_ping_pong() {
        let (mut tx, mut rx) = tokio::io::duplex(4096);

        // Write a Ping message.
        let ping_msg = WorkerMessage::Ping { seq: 1 };
        write_frame(&mut tx, &ping_msg)
            .await
            .expect("write_frame failed");

        // The worker side simulates replying with Pong.
        let pong_event = WorkerEvent::Pong { seq: 1 };
        write_raw_frame(&mut tx, &pong_event)
            .await
            .expect("write raw frame failed");

        // Read back the Ping frame (echoed).
        let read_back: WorkerMessage = {
            let mut len_buf = [0u8; 4];
            rx.read_exact(&mut len_buf)
                .await
                .expect("read length failed");
            let len = u32::from_be_bytes(len_buf) as usize;
            let mut buf = vec![0u8; len];
            rx.read_exact(&mut buf).await.expect("read payload failed");
            rmp_serde::from_slice::<WorkerMessage>(&buf).expect("decode failed")
        };
        assert_eq!(read_back, ping_msg);

        // Read back the Pong event using read_frame.
        let pong_read = read_frame(&mut rx, DEFAULT_MAX_PAYLOAD_MIB)
            .await
            .expect("read_frame failed");
        assert_eq!(pong_read, pong_event);
    }

    /// Oversize-rejection test: frame with length header encoding 65 MiB + 1,
    /// assert `AnvilError::PayloadTooLarge` without reading payload.
    #[tokio::test]
    async fn reject_oversize_payload() {
        let (mut tx, mut rx) = tokio::io::duplex(4096);

        // Write a length prefix for 65 MiB + 1 bytes (well above default 10 MiB limit).
        let oversized_len: u32 = 65 * 1024 * 1024 + 1;
        tx.write_all(&oversized_len.to_be_bytes())
            .await
            .expect("write length failed");

        // The read_frame should reject immediately without reading the payload.
        let result = read_frame(&mut rx, DEFAULT_MAX_PAYLOAD_MIB).await;
        match result {
            Err(AnvilError::PayloadTooLarge {
                size_mib,
                limit_mib,
            }) => {
                assert_eq!(size_mib, 65);
                assert_eq!(limit_mib, 10);
            }
            other => panic!("expected PayloadTooLarge, got {:?}", other),
        }
    }
}
