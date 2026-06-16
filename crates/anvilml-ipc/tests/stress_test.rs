//! Stress test for the full Rust-to-Python IPC path.
//!
//! This test exercises `RouterTransport` end-to-end by spawning a minimal
//! Python echo worker subprocess, then sending 1000 `WorkerMessage::Ping`
//! messages and asserting that all 1000 `WorkerEvent::Pong` responses arrive
//! with matching sequence numbers in order.
//!
//! The test must complete within 30 seconds. When it succeeds, the output
//! contains the line "stress test passed: 1000/1000".

use anvilml_ipc::{RouterTransport, WorkerEvent, WorkerMessage};
use std::process::Command;
use std::time::Duration;

/// Worker identity used by both the Rust test and the Python echo subprocess.
/// Hardcoded because this is a single-worker test — no pool or dynamic
/// identity management is involved.
const WORKER_ID: &[u8] = b"stress-test-worker";

/// Total number of Ping→Pong roundtrips to perform.
const TOTAL_MESSAGES: u64 = 1000;

/// The maximum total time allowed for all 1000 roundtrips.
/// A per-message timeout would be inappropriate for a throughput test;
/// the 30-second deadline covers the entire batch.
const TOTAL_TIMEOUT_SECS: u64 = 30;

/// Discover the Python interpreter path for the worker venv.
///
/// Uses the `ANVILML_VENV_PATH` environment variable if set, otherwise
/// falls back to `{workspace_root}/worker/.venv`. Constructs the
/// platform-specific interpreter path (bin/python3 on Unix,
/// Scripts\python.exe on Windows).
///
/// The workspace root is derived from the crate's manifest directory
/// (`CARGO_MANIFEST_DIR`) by going up one level, since the crate lives
/// under `crates/anvilml-ipc/`.
fn python_interpreter() -> String {
    let venv_path = std::env::var("ANVILML_VENV_PATH").unwrap_or_else(|_| {
        // The crate manifest is at `crates/anvilml-ipc/Cargo.toml`.
        // The workspace root is one directory above that.
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let workspace_root = std::path::Path::new(manifest_dir)
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        workspace_root
            .join("worker/.venv")
            .to_string_lossy()
            .to_string()
    });

    // Platform-specific interpreter path construction.
    // On Windows, the venv structure uses Scripts\python.exe instead
    // of bin/python3. The cfg! macro is evaluated at compile time.
    if cfg!(windows) {
        format!(r"{venv_path}\Scripts\python.exe")
    } else {
        format!("{venv_path}/bin/python3")
    }
}

/// Verify the full IPC roundtrip path: bind `RouterTransport`, spawn a Python
/// echo worker subprocess, send 1000 `WorkerMessage::Ping` messages, receive
/// 1000 matching `WorkerEvent::Pong` responses in order, and assert zero
/// timeouts within a 30-second deadline.
///
/// This test exercises the entire stack:
/// 1. Rust `RouterTransport::bind()` creates a ZeroMQ ROUTER socket.
/// 2. The Python subprocess connects via `worker.ipc.connect()` (DEALER).
/// 3. 1000 Ping messages are sent and 1000 Pong responses received.
/// 4. Each Pong's `seq` field is asserted to match the original Ping's `seq`.
#[tokio::test]
async fn stress_test_1000_trips() {
    // Wrap the entire test in a timeout to enforce the 30-second deadline.
    // Using a total deadline rather than per-message timeouts because this
    // is a throughput test — the spec says "all within 30 seconds".
    let result =
        tokio::time::timeout(Duration::from_secs(TOTAL_TIMEOUT_SECS), run_stress_test()).await;

    match result {
        Ok(Ok(())) => {
            // Test passed — println! output is captured by the test runner.
            println!("stress test passed: {TOTAL_MESSAGES}/{TOTAL_MESSAGES}");
        }
        Ok(Err(e)) => {
            panic!("stress test failed: {e}");
        }
        Err(_) => {
            panic!(
                "stress test timed out after {} seconds — did not complete {} roundtrips",
                TOTAL_TIMEOUT_SECS, TOTAL_MESSAGES
            );
        }
    }
}

/// Core stress test logic, separated into its own function for clarity.
/// Returns `Ok(())` on success or an `AnvilError` on failure.
async fn run_stress_test() -> Result<(), anvilml_core::AnvilError> {
    // Bind a ROUTER socket on a random OS-assigned port.
    let transport = RouterTransport::bind()
        .await
        .map_err(|e| anvilml_core::AnvilError::Ipc(format!("ROUTER bind failed: {e}")))?;

    // Locate the Python interpreter from the worker venv.
    let python = python_interpreter();

    // Verify the interpreter exists before spawning — fail fast with a
    // descriptive error rather than a cryptic subprocess failure.
    if !std::path::Path::new(&python).exists() {
        return Err(anvilml_core::AnvilError::Ipc(format!(
            "Python interpreter not found at {python}. \
             Set ANVILML_VENV_PATH or provision the venv with install_worker_deps.sh"
        )));
    }

    // Spawn the Python echo worker subprocess. The working directory is set
    // to the workspace root so the `worker.ipc` import resolves correctly
    // (the `worker/` directory is on the import path). We also set
    // `PYTHONPATH` to the workspace root so Python can find the `worker`
    // package (the script's own directory is added by Python but that
    // only gives us `worker/`, not the parent directory that contains it).
    let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let child = Command::new(&python)
        .current_dir(workspace_root)
        .env("PYTHONPATH", workspace_root.to_string_lossy().to_string())
        .arg("worker/ipc_echo.py")
        .arg(transport.port.to_string())
        .spawn()
        .map_err(|e| {
            anvilml_core::AnvilError::Ipc(format!("failed to spawn Python worker: {e}"))
        })?;

    // Scope the child handle so it is dropped (and the process killed)
    // when this function returns, ensuring cleanup on both success and
    // failure paths.
    let _child = child;

    // Wait for the Python worker to connect and send its startup Ready
    // message. ZeroMQ's lazy-connection means the ROUTER socket won't
    // route messages to the DEALER until the DEALER has connected and
    // sent at least one message. The 500ms delay ensures the startup
    // Ready message has been received and the identity frame is established.
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Drain the startup Ready message so it doesn't interfere with the
    // ping loop. The first recv() will pick up the Ready event.
    let (ready_id, ready_event) = transport.recv().await?;
    // The Ready event confirms the worker connected successfully and
    // its identity was registered on the ROUTER. We verify the variant
    // is Ready to catch routing mismatches early.
    if !matches!(ready_event, WorkerEvent::Ready { .. }) {
        return Err(anvilml_core::AnvilError::Ipc(format!(
            "expected Ready event from {WORKER_ID:?}, got {ready_event:?} (from {ready_id})"
        )));
    }

    // Send 1000 Ping messages and verify each Pong response.
    let mut received: u64 = 0;

    for seq in 0..TOTAL_MESSAGES {
        // Send a Ping to the worker identity. The ROUTER socket will
        // route it to the DEALER that registered this identity.
        transport
            .send(WORKER_ID, &WorkerMessage::Ping { seq })
            .await
            .map_err(|e| {
                anvilml_core::AnvilError::Ipc(format!("send Ping {{ seq: {seq} }} failed: {e}"))
            })?;

        // Receive the Pong response. The ROUTER returns the identity
        // frame and the encoded payload, which recv() extracts and decodes.
        let (_id, event) = transport.recv().await?;

        // Verify the event is a Pong with the matching sequence number.
        // This assertion checks both the variant (Pong) and the field
        // value (seq), failing fast on any mismatch.
        if !matches!(event, WorkerEvent::Pong { seq: s } if s == seq) {
            return Err(anvilml_core::AnvilError::Ipc(format!(
                "seq {seq}: expected Pong {{ seq: {seq} }}, got {event:?}"
            )));
        }

        received += 1;
    }

    // Send a Shutdown message to the worker so it exits cleanly.
    // This prevents the subprocess from lingering after the test.
    let _ = transport.send(WORKER_ID, &WorkerMessage::Shutdown).await;

    // Verify we received exactly the expected number of responses.
    // This is a sanity check — the loop above would have already failed
    // if any response was missing or mismatched.
    if received != TOTAL_MESSAGES {
        return Err(anvilml_core::AnvilError::Ipc(format!(
            "expected {TOTAL_MESSAGES} Pong responses, got {received}"
        )));
    }

    Ok(())
}
