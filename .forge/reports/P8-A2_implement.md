# Implementation Report: P8-A2

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P8-A2                                              |
| Phase       | 008 — IPC Framing                                  |
| Description | anvilml-ipc: write_frame (length-prefixed msgpack) |
| Implemented | 2026-06-05T20:45:00Z                               |
| Status      | COMPLETE                                           |

## Summary

Implemented `write_frame` in the `anvilml-ipc` crate — a length-prefixed msgpack frame writer. Added `tokio` as a dependency (with `io-util` feature), created `src/framing.rs` with the `write_frame` async function and four unit tests, and updated `lib.rs` to export the new module. All gates passed: format, clippy (both passes), platform cross-checks (all 3), tests (21/21 in anvilml-ipc), and config drift gate.

## Resolved Dependencies

| Type   | Name    | Version resolved | Source        |
|--------|---------|-----------------|---------------|
| crate  | tokio   | 1.52.3          | Cargo.toml workspace |

The workspace already declares `tokio = { version = "1.52.3", features = ["full"] }` in `[workspace.dependencies]`. Since `"full"` includes `"io-util"`, the crate references `{ workspace = true, features = ["io-util"] }` which is additive and compatible. No new MCP lookup was needed as the dependency already existed in the workspace manifest.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-ipc/Cargo.toml` | Added `tokio = { workspace = true, features = ["io-util"] }` dependency |
| Create | `crates/anvilml-ipc/src/framing.rs` | New module: `write_frame` function + 4 unit tests |
| Modify | `crates/anvilml-ipc/src/lib.rs` | Declared `pub mod framing;` |

## Commit Log

```
diff --git a/Cargo.lock b/Cargo.lock
index ea800f9..6db7d2d 100644
--- a/Cargo.lock
+++ b/Cargo.lock
@@ -114,6 +114,7 @@ dependencies = [
  "rmp-serde",
  "serde",
  "serde_json",
+ "tokio",
  "uuid",
 ]

diff --git a/crates/anvilml-ipc/Cargo.toml b/crates/anvilml-ipc/Cargo.toml
index 0ac7db7..4db6bdc 100644
--- a/crates/anvilml-ipc/Cargo.toml
+++ b/crates/anvilml-ipc/Cargo.toml
@@ -8,6 +8,7 @@ anvilml-core = { path = "../anvilml-core" }
 rmp-serde = { workspace = true }
 serde = { workspace = true, features = ["derive"] }
 serde_json = { workspace = true }
+tokio = { workspace = true, features = ["io-util"] }
 uuid = { workspace = true, features = ["serde"] }

diff --git a/crates/anvilml-ipc/src/framing.rs b/crates/anvilml-ipc/src/framing.rs
new file mode 100644
index 0000000..da7ad5e
--- /dev/null
+++ b/crates/anvilml-ipc/src/framing.rs
@@ -0,0 +1,107 @@
+use anvilml_core::error::AnvilError;
+use tokio::io::{AsyncWrite, AsyncWriteExt};
+
+use crate::WorkerMessage;
+
+/// Write a single length-prefixed msgpack frame to the given async sink.
+///
+/// The frame layout is:
+///   - 4 bytes: payload length as big-endian `u32`
+///   - N bytes: msgpack-encoded `WorkerMessage` (via `rmp_serde::to_vec_named`)
+pub async fn write_frame<W>(w: &mut W, msg: &WorkerMessage) -> Result<(), AnvilError>
+where
+    W: AsyncWrite + Unpin,
+{
+    let payload = rmp_serde::to_vec_named(msg).map_err(|e| AnvilError::Json(e.to_string()))?;
+    let len = payload.len() as u32;
+    let header = len.to_be_bytes();
+    w.write_all(&header).await.map_err(AnvilError::Io)?;
+    w.write_all(&payload).await.map_err(AnvilError::Io)?;
+    Ok(())
+}
+
+#[cfg(test)]
+mod tests {
+    use super::*;
+
+    #[tokio::test]
+    async fn write_frame() {
+        let msg = WorkerMessage::Ping { seq: 7 };
+
+        // Serialize to get expected payload length
+        let payload = rmp_serde::to_vec_named(&msg).expect("serialize");
+        let payload_len = payload.len();
+
+        // Write frame to Vec<u8>
+        let mut buf = Vec::new();
+        super::write_frame(&mut buf, &msg).await.expect("write_frame");
+
+        // Total buffer should be 4-byte header + payload
+        assert_eq!(buf.len(), 4 + payload_len);
+
+        // First 4 bytes must equal payload length as big-endian u32
+        let mut header = [0u8; 4];
+        header.copy_from_slice(&buf[0..4]);
+        let decoded_len = u32::from_be_bytes(header);
+        assert_eq!(decoded_len, payload_len as u32);
+
+        // Payload bytes must match the serialized message
+        assert_eq!(&buf[4..], &payload[..]);
+    }
+
+    #[test]
+    fn write_frame_sync_serialization() {
+        let msg = WorkerMessage::Shutdown;
+        let payload = rmp_serde::to_vec_named(&msg).expect("serialize");
+        assert!(!payload.is_empty());
+    }
+
+    #[tokio::test]
+    async fn write_frame_shutdown() {
+        let msg = WorkerMessage::Shutdown;
+        let mut buf = Vec::new();
+        super::write_frame(&mut buf, &msg).await.expect("write_frame");
+
+        let payload = rmp_serde::to_vec_named(&msg).expect("serialize");
+        assert_eq!(buf.len(), 4 + payload.len());
+
+        let mut header = [0u8; 4];
+        header.copy_from_slice(&buf[0..4]);
+        let decoded_len = u32::from_be_bytes(header);
+        assert_eq!(decoded_len, payload.len() as u32);
+    }
+
+    #[tokio::test]
+    async fn write_frame_execute() {
+        use uuid::Uuid;
+
+        let job_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
+        let graph = serde_json::json!({ "nodes": [] });
+        let settings = anvilml_core::types::job::JobSettings {
+            seed: 42,
+            steps: 30,
+            guidance_scale: 7.5,
+            width: 1024,
+            height: 1024,
+            device_preference: Some(0),
+        };
+        let msg = WorkerMessage::Execute {
+            job_id,
+            graph,
+            settings,
+            device_index: 0,
+        };
+
+        let mut buf = Vec::new();
+        super::write_frame(&mut buf, &msg).await.expect("write_frame");
+
+        let payload = rmp_serde::to_vec_named(&msg).expect("serialize");
+        assert_eq!(buf.len(), 4 + payload.len());
+
+        let mut header = [0u8; 4];
+        header.copy_from_slice(&buf[0..4]);
+        let decoded_len = u32::from_be_bytes(header);
+        assert_eq!(decoded_len, payload.len() as u32);
+    }
+}

diff --git a/crates/anvilml-ipc/src/lib.rs b/crates/anvilml-ipc/src/lib.rs
index 258c99f..80fbcb9 100644
--- a/crates/anvilml-ipc/src/lib.rs
+++ b/crates/anvilml-ipc/src/lib.rs
@@ -1,3 +1,4 @@
+pub mod framing;
 pub mod messages;

 pub use messages::{WorkerEvent, WorkerMessage};
```

## Test Results

```
$ cargo test -p anvilml-ipc -- write_frame
   Compiling tokio-macros v2.7.0
   Compiling anvilml-core v0.1.0 (/home/dryw/AnvilML/crates/anvilml-core)
   Compiling tokio v1.52.3
   Compiling anvilml-ipc v0.1.0 (/home/dryw/AnvilML/crates/anvilml-ipc)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1.96s
     Running unittests src/lib.rs (target/debug/deps/anvilml_ipc-f8cbc5dabfd16444)

running 4 tests
test framing::tests::write_frame ... ok
test framing::tests::write_frame_shutdown ... ok
test framing::tests::write_frame_sync_serialization ... ok
test framing::tests::write_frame_execute ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 17 filtered out; finished in 0.00s

   Doc-tests anvilml_ipc

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Full crate test suite:
```
$ cargo test -p anvilml-ipc
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.14s
     Running unittests src/lib.rs (target/debug/deps/anvilml_ipc-f8cbc5dabfd16444)

running 21 tests
test framing::tests::write_frame ... ok
test framing::tests::write_frame_shutdown ... ok
test framing::tests::write_frame_execute ... ok
test framing::tests::write_frame_sync_serialization ... ok
test messages::tests::all_worker_message_variants ... ok
test messages::tests::all_worker_event_variants ... ok
test messages::tests::worker_event_roundtrip_cancelled ... ok
test messages::tests::worker_event_roundtrip_completed ... ok
test messages::tests::worker_event_roundtrip_dying ... ok
test messages::tests::worker_event_roundtrip_failed ... ok
test messages::tests::worker_event_roundtrip_image_ready ... ok
test messages::tests::worker_event_roundtrip_memory_report ... ok
test messages::tests::worker_event_roundtrip_pong ... ok
test messages::tests::worker_event_roundtrip_progress ... ok
test messages::tests::worker_event_roundtrip_ready ... ok
test messages::tests::worker_message_roundtrip_cancel_job ... ok
test messages::tests::worker_message_roundtrip_execute ... ok
test messages::tests::worker_message_roundtrip_init_hardware ... ok
test messages::tests::worker_message_roundtrip_memory_query ... ok
test messages::tests::worker_message_roundtrip_ping ... ok
test messages::tests::worker_message_roundtrip_shutdown ... ok

test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests anvilml_ipc

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

## Platform Cross-Check

```
# Check 1: Mock-hardware Windows cross-check
$ cargo check --target x86_64-pc-windows-gnu --workspace --features mock-hardware
    Checking anvilml-ipc v0.1.0 (/home/dryw/AnvilML/crates/anvilml-ipc)
    Checking anvilml-worker v0.1.0 (/home/dryw/AnvilML/crates/anvilml-worker)
    Checking anvilml-scheduler v0.1.0 (/home/dryw/AnvilML/crates/anvilml-scheduler)
    Checking anvilml-server v0.1.0 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking anvilml-openapi v0.1.0 (/home/dryw/AnvilML/crates/anvilml-openapi)
    Checking backend v0.1.0 (/home/dryw/AnvilML/backend)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.16s

# Check 2: Real-hardware Linux native
$ cargo check --bin anvilml
    Checking anvilml-ipc v0.1.0 (/home/dryw/AnvilML/crates/anvilml-ipc)
    Checking anvilml-worker v0.1.0 (/home/dryw/AnvilML/crates/anvilml-worker)
    Checking anvilml-scheduler v0.1.0 (/home/dryw/AnvilML/crates/anvilml-scheduler)
    Checking anvilml-server v0.1.0 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking backend v0.1.0 (/home/dryw/AnvilML/backend)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.70s

# Check 3: Real-hardware Windows cross-check
$ cargo check --bin anvilml --target x86_64-pc-windows-gnu
    Checking anvilml-worker v0.1.0 (/home/dryw/AnvilML/crates/anvilml-worker)
    Checking anvilml-scheduler v0.1.0 (/home/dryw/AnvilML/crates/anvilml-scheduler)
    Checking anvilml-server v0.1.0 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking backend v0.1.0 (/home/dryw/AnvilML/backend)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.57s
```

All three checks exited 0.

## Project Gates

```
# Config Surface Sync Gate
$ cargo test -p backend --features mock-hardware -- config_reference
    Finished `test` profile [unoptimized + debuginfo] target(s) in 6.34s
     Running unittests src/main.rs (target/debug/deps/anvilml-db36bc9a0ecf3709)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 8 filtered out; finished in 0.00s

     Running tests/config_reference.rs (target/debug/deps/config_reference-24159f5595765223)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.00s
```

Note: The filter `config_reference` matches the test file name. The actual test function is `test_toml_key_set_matches_default`. Running with explicit test name confirms it passes:
```
$ cargo test -p backend --features mock-hardware -- test_toml_key_set_matches_default
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.21s
     Running tests/config_reference.rs (target/debug/deps/config_reference-24159f5595765223)

running 1 test
test test_toml_key_set_matches_default ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

## Deviations from Plan

- Removed unused imports (`bytes::Bytes` and `std::io::Cursor`) from `framing.rs` to prevent clippy warnings. The plan mentioned these as imports but noted `bytes::Bytes` was "not needed" and `Cursor` was "for test" — neither was actually used in the implementation.
- Used `super::write_frame` in test functions to avoid shadowing by the local test function named `write_frame`. This is a Rust scoping necessity, not a functional deviation.
- Added 3 additional tests (`write_frame_sync_serialization`, `write_frame_shutdown`, `write_frame_execute`) beyond the single required test to exercise multiple WorkerMessage variants (Shutdown, Execute) and verify serialization outside async context.

## Blockers

None.
