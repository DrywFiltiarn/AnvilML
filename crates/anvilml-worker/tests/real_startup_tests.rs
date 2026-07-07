//! Integration test proving real-mode worker startup end-to-end against a live
//! subprocess.
//!
//! This test spawns a genuine `worker_main.py` process with `ANVILML_DEVICE_TYPE=cpu`
//! and no `ANVILML_WORKER_MOCK` flag, connects a ZeroMQ DEALER socket to the worker's
//! ROUTER transport, and verifies that the worker sends a `Ready` event with
//! `capabilities_source = "pytorch"` (proving the real torch probe ran) and a
//! `node_types` list containing the registered PassThrough node.
//!
//! Acceptance: `cargo test -p anvilml-worker --test real_startup_tests -- --test-threads=1`
//! exits 0 after the Python venv is provisioned (`bash scripts/install_worker_deps.sh`).

use std::path::PathBuf;
use std::time::Duration;

use anvilml_core::DeviceType;
use anvilml_ipc::{RouterTransport, WorkerEvent};
use anvilml_worker::{WorkerEnv, build_command};
use bytes::Bytes;
use zeromq::prelude::*;
use zeromq::util::PeerIdentity;
use zeromq::{DealerSocket, SocketOptions};

/// Resolve the repo root from the crate's manifest directory.
///
/// `CARGO_MANIFEST_DIR` points to `crates/anvilml-worker/`; we go up two
/// levels (`crates/` → project root) to reach the repo root where
/// `worker/.venv` and `worker/worker_main.py` live.
fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

/// A real `worker_main.py` subprocess spawned with CPU device and no mock flag
/// connects over IPC, runs the real torch capability probe, and sends a `Ready`
/// event with `capabilities_source = "pytorch"` and empty `node_types` within
/// 10 seconds.
///
/// This is the phase's Runnable Proof: end-to-end verification that the real-mode
/// startup sequence in `worker_main.py` works when invoked by the Rust supervisor's
/// `spawn_worker()` function.
#[tokio::test]
async fn test_real_subprocess_sends_ready() {
    // Bind a ROUTER socket on an OS-assigned port.
    // The transport owns the socket and provides send/recv for worker events.
    let transport = RouterTransport::bind().await.expect("bind should succeed");

    // Build a worker environment targeting CPU, real mode (mock = false).
    // mock = false ensures ANVILML_WORKER_MOCK is NOT injected — its absence
    // signals real-mode hardware execution to the Python worker.
    let env = WorkerEnv::build(transport.port, "0", 0, DeviceType::Cpu, false, "info", 256);

    // Construct the worker command using build_command(), then set the
    // subprocess CWD to the repo root. This is necessary because cargo test
    // runs from target/debug/deps, so relative paths like "worker/.venv"
    // and "worker/worker_main.py" would not resolve correctly.
    // The supervisor sets CWD to the repo root before spawning, so we match
    // that behavior here.
    //
    // We also set PYTHONPATH to include the repo root, so that
    // `import worker.ipc` (and other `worker.*` imports) resolve correctly.
    // Without this, Python only adds the script's directory (`worker/`) to
    // sys.path, not the repo root where the `worker` package lives.
    let repo = repo_root();
    let venv_path = repo.join("worker/.venv");
    let mut cmd = build_command(&venv_path, env);
    cmd.current_dir(&repo);
    cmd.env("PYTHONPATH", &repo);

    // Spawn the worker subprocess. The subprocess will:
    // 1. Read ANVILML_IPC_PORT, ANVILML_WORKER_ID, ANVILML_DEVICE_TYPE=cpu,
    //    ANVILML_DEVICE_INDEX=0 from the env map.
    // 2. Import torch and select CPU device (no-op for CPU).
    // 3. Run the real probe_capabilities() which returns CPU probe results.
    // 4. Call _import_nodes() which returns descriptors for registered nodes.
    // 5. Send a Ready event with capabilities_source="pytorch" and
    //    node_types containing the PassThrough node descriptor.
    // 6. Enter the dispatch loop (blocking).
    let mut child = cmd.spawn().expect("spawn_worker should succeed");

    // Connect a DEALER socket to the ROUTER endpoint, setting the worker identity.
    // The identity "0" must match the worker_id used in WorkerEnv::build above,
    // because the ROUTER routes messages based on peer identity.
    let mut opts = SocketOptions::default();
    opts.peer_identity(
        PeerIdentity::try_from(Bytes::from("0".to_string())).expect("valid identity"),
    );
    let mut dealer = DealerSocket::with_options(opts);
    let endpoint = format!("tcp://127.0.0.1:{}", transport.port);
    dealer
        .connect(&endpoint)
        .await
        .expect("DEALER connect to ROUTER should succeed");
    // Give the ROUTER time to register the DEALER's identity — the ROUTER
    // does not know about a DEALER until after the connection handshake,
    // and recv() would fail if called before registration.
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Receive the Ready event from the transport with an explicit timeout.
    // The 10-second timeout accounts for torch import on CPU (the heaviest
    // part of the startup sequence). If the timeout fires, terminate the
    // subprocess and capture its stderr before failing — this turns a
    // multi-minute hang into an immediately diagnosable failure.
    let (identity, event) =
        match tokio::time::timeout(Duration::from_secs(10), transport.recv()).await {
            Ok(Ok((id, evt))) => (id, evt),
            Ok(Err(e)) => {
                child.kill().await.expect("kill should succeed");
                let _ = child.wait().await;
                panic!("recv failed: {e}");
            }
            Err(_) => {
                // Timeout — terminate the subprocess and capture its stderr.
                child.kill().await.expect("kill should succeed");
                let output = child
                    .wait_with_output()
                    .await
                    .expect("wait_with_output should succeed");
                let stderr_str = String::from_utf8_lossy(&output.stderr);
                let stdout_str = String::from_utf8_lossy(&output.stdout);
                panic!(
                    "worker did not send Ready event within 10s.\n\
                 stderr:\n{}\n\
                 stdout:\n{}",
                    stderr_str.trim(),
                    stdout_str.trim()
                );
            }
        };

    // Assert that the event is a Ready variant (not Pong, Dying, etc.).
    // This proves the worker reached the end of its startup sequence.
    let WorkerEvent::Ready {
        capabilities_source,
        node_types,
        ..
    } = &event
    else {
        child.kill().await.expect("kill should succeed");
        let _ = child.wait().await;
        panic!("expected Ready event, got: {:?}", event);
    };

    // Assert capabilities_source == "pytorch" — this proves the real torch
    // probe ran (mock mode would produce "mock").
    assert_eq!(
        capabilities_source, "pytorch",
        "capabilities_source should be 'pytorch' in real mode, got '{}'",
        capabilities_source
    );

    // Assert node_types contains the PassThrough node — registered via
    // auto-import at package load time. The _import_nodes() function
    // returns a list of node descriptors from NODE_REGISTRY.
    assert!(
        !node_types.is_empty(),
        "node_types should contain registered nodes (e.g. PassThrough), got empty list"
    );
    assert_eq!(
        node_types[0].type_name, "PassThrough",
        "first node_type should be PassThrough, got '{}'",
        node_types[0].type_name
    );

    // Verify the identity matches the worker_id we set — proves the ROUTER
    // routed the message back to the correct DEALER socket.
    assert_eq!(
        identity, "0",
        "ROUTER should return worker identity '0', got '{}'",
        identity
    );

    // Clean up: terminate the subprocess and wait for it to exit.
    // The worker is in the dispatch loop (blocking recv), so we use kill()
    // (SIGKILL) rather than a graceful shutdown message — the dispatch loop
    // is not yet wired to handle shutdown messages.
    child.kill().await.expect("kill should succeed");
    child.wait().await.expect("wait should succeed");
}
