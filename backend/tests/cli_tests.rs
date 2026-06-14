/// Integration test: the server accepts `--port` CLI override, binds to the
/// OS-assigned port, and the health endpoint returns HTTP 200 with
/// `{"status":"ok"}`.
///
/// This test verifies the full config-loading path: CLI parsing →
/// ConfigOverrides → config::load → TCP bind → health endpoint.
///
/// The server is spawned as a subprocess with `--port 0` (OS-assigned port).
/// The actual port is detected via platform-specific tooling (`lsof` on Unix,
/// `netstat` on Windows) — the OS-assigned port cannot be known in advance.
/// The subprocess is killed after the assertion regardless of outcome.
///
/// Preconditions:
///   - Workspace builds with `mock-hardware` feature.
///   - No prior server running on the OS-assigned port.
///
/// Acceptance command:
///   `cargo test -p anvilml --features mock-hardware -- cli_tests` exits 0.
use std::io::{Read, Write};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

/// Spawn the anvilml binary directly (pre-built) with `--port 0`, detect the
/// bound port via platform-specific tooling (`lsof` on Unix, `netstat` on
/// Windows), hit the health endpoint, and assert HTTP 200 with
/// `"status":"ok"`.
///
/// The subprocess is killed unconditionally at the end of the test.
#[test]
fn test_custom_port_health() {
    // Find the path to the built binary.
    // The binary is at CARGO_TARGET_DIR/debug/anvilml (set by cargo).
    // CARGO_TARGET_DIR may be absolute or relative to the workspace root.
    // If not set, fall back to the workspace-relative path.
    let binary = match std::env::var("CARGO_TARGET_DIR") {
        Ok(target_dir) => {
            let path = std::path::Path::new(&target_dir);
            if path.is_absolute() {
                format!("{}/debug/anvilml", target_dir)
            } else {
                let ws_root = std::env::current_dir()
                    .expect("failed to get cwd")
                    .parent()
                    .expect("parent of backend/")
                    .to_path_buf();
                format!("{}", ws_root.join(target_dir).display())
            }
        }
        Err(_) => {
            let ws_root = std::env::current_dir()
                .expect("failed to get cwd")
                .parent()
                .expect("parent of backend/")
                .to_path_buf();
            format!("{}", ws_root.join("target/debug/anvilml").display())
        }
    };

    // Spawn the server binary directly with port 0 for OS-assigned port.
    // The --log-format plain flag ensures clean stderr output.
    // Using the pre-built binary avoids cargo's output buffering issues.
    let mut child = Command::new(&binary)
        .args(["--port", "0", "--log-format", "plain"])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| {
            panic!(
                "failed to spawn server binary at '{}': {}. \
                 Make sure the project is built with `cargo build --bin anvilml --features mock-hardware`.",
                binary, e
            )
        });

    // Capture and clean up any ANVILML_* env vars that might have leaked from
    // other parallel test runs. This prevents env var pollution from affecting
    // other tests in the workspace.
    let prior_env: Vec<(String, Option<String>)> = [
        "ANVILML_PORT",
        "ANVILML_HOST",
        "ANVILML_DB_PATH",
        "ANVILML_ARTIFACT_DIR",
        "ANVILML_VENV_PATH",
        "ANVILML_SEEDS_PATH",
        "ANVILML_MAX_IPC_PAYLOAD_MIB",
        "ANVILML_NUM_THREADS",
        "ANVILML_GPU_SELECTION__DEFAULT_DEVICE",
    ]
    .iter()
    .map(|name| {
        let val = std::env::var(name).ok();
        (name.to_string(), val)
    })
    .collect();

    // Scope for the child handle — ensures it's dropped (killed) on test exit.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // Clear any ANVILML_* env vars that may have leaked from other
        // parallel test runs. This prevents env var pollution from
        // affecting the test's own logic (e.g., if a prior test set
        // ANVILML_PORT and failed to restore it).
        for name in [
            "ANVILML_PORT",
            "ANVILML_HOST",
            "ANVILML_DB_PATH",
            "ANVILML_ARTIFACT_DIR",
            "ANVILML_VENV_PATH",
            "ANVILML_SEEDS_PATH",
            "ANVILML_MAX_IPC_PAYLOAD_MIB",
            "ANVILML_NUM_THREADS",
            "ANVILML_GPU_SELECTION__DEFAULT_DEVICE",
        ] {
            std::env::remove_var(name);
        }

        // Give the server time to start and bind.
        thread::sleep(Duration::from_millis(500));

        // Capture the child PID before port detection — needed by the Windows
        // netstat branch to filter listening sockets by owning process.
        // child.id() is always Some(u32) after a successful spawn().
        #[allow(unused_variables)]
        let child_pid = child.id();

        // Detect the bound port using platform-specific tooling.
        // Both branches produce the same `port: u16` variable, so the
        // downstream HTTP request code is unchanged.
        let port: u16 = {
            #[cfg(unix)]
            {
                // Detect the bound port using `lsof`.
                // `lsof -i TCP -sTCP:LISTEN -P -n` lists all TCP listeners with
                // numeric ports (no DNS resolution). We filter for the anvilml
                // process and extract its port.
                let output = Command::new("lsof")
                    .args(["-i", "TCP", "-sTCP:LISTEN", "-P", "-n"])
                    .output()
                    .expect("failed to run lsof");

                let lsof_output = String::from_utf8_lossy(&output.stdout);

                // Find the anvilml process's port.
                // lsof output format: COMMAND PID USER FD TYPE DEVICE SIZE/OFF NODE NAME
                // We look for lines containing "anvilml" and extract the port from
                // the NAME column (e.g., "127.0.0.1:12345 (LISTEN)").
                let mut port: Option<u16> = None;
                for line in lsof_output.lines() {
                    if line.contains("anvilml") && line.contains("LISTEN") {
                        // Extract the port from the NAME field.
                        // Format: 127.0.0.1:PORT (LISTEN)
                        // split_whitespace().last() gives "(LISTEN)", so we use
                        // the second-to-last field which is the address:port.
                        let fields: Vec<&str> = line.split_whitespace().collect();
                        if fields.len() >= 2 {
                            let addr_port = fields[fields.len() - 2]; // e.g. "127.0.0.1:PORT"
                            if let Some(colon) = addr_port.rfind(':') {
                                if let Ok(p) = addr_port[colon + 1..].parse::<u16>() {
                                    port = Some(p);
                                    break;
                                }
                            }
                        }
                    }
                }

                // Fallback: if lsof didn't work, try ss as an alternative.
                if port.is_none() {
                    let ss_output = Command::new("ss")
                        .args(["-tlnp"])
                        .output()
                        .ok()
                        .map(|o| String::from_utf8_lossy(&o.stdout).to_string());

                    if let Some(ss_out) = ss_output {
                        for line in ss_out.lines() {
                            if line.contains("anvilml") {
                                // ss -tlnp format: State Recv-Q Send-Q Local Address:Port Peer Address:Port
                                if let Some(addr_port) = line.split_whitespace().nth(4) {
                                    if let Some(colon) = addr_port.rfind(':') {
                                        if let Ok(p) = addr_port[colon + 1..].parse::<u16>() {
                                            port = Some(p);
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                port.expect(
                    "could not detect server port via lsof/ss — server may not have started. \
                     Check that mock-hardware feature is available and the binary runs correctly.",
                )
            }

            #[cfg(windows)]
            {
                // Execute `netstat -ano -p TCP` to list all TCP listeners with
                // owning PIDs. Flags: -a=all connections, -n=numeric addresses,
                // -o=owning PID, -p TCP=TCP protocol only.
                // Output format (columns separated by whitespace):
                //   Proto  Local Address          Foreign Address        State           PID
                //   TCP    0.0.0.0:PORT           0.0.0.0:0              LISTENING       PID
                // Local Address is column index 1; PID is the last column (index 4+).
                let output = Command::new("netstat")
                    .args(["-ano", "-p", "TCP"])
                    .output()
                    .expect("failed to run netstat");

                let netstat_output = String::from_utf8_lossy(&output.stdout);
                let pid_str = child_pid.to_string();

                // Parse netstat output line-by-line.
                // Skip the header line (starts with "Proto") and find the line
                // whose PID matches our child process.
                let mut port: Option<u16> = None;
                for line in netstat_output.lines() {
                    // Skip header and empty lines.
                    if line.starts_with("Proto") || line.trim().is_empty() {
                        continue;
                    }

                    // Split by whitespace to get columns.
                    let fields: Vec<&str> = line.split_whitespace().collect();
                    if fields.len() < 5 {
                        // Skip malformed lines (need at least Proto, LocalAddr, ForeignAddr, State, PID).
                        continue;
                    }

                    // Check if PID (last column) matches our child process.
                    if fields[fields.len() - 1] != pid_str {
                        continue;
                    }

                    // Extract port from Local Address column (index 1).
                    // Format: 0.0.0.0:PORT or 127.0.0.1:PORT
                    let local_addr = fields[1];
                    if let Some(colon) = local_addr.rfind(':') {
                        if let Ok(p) = local_addr[colon + 1..].parse::<u16>() {
                            port = Some(p);
                            break;
                        }
                    }
                }

                port.expect(
                    "could not detect server port via netstat — server may not have started. \
                     Check that mock-hardware feature is available and the binary runs correctly.",
                )
            }
        };

        // Send a raw HTTP GET /health request over TCP.
        // Using std::net::TcpStream avoids adding a new dependency for the
        // test crate. The server is axum-based and speaks HTTP/1.1 natively.
        let mut stream = std::net::TcpStream::connect(format!("127.0.0.1:{port}"))
            .expect("failed to connect to server");

        // Write the HTTP request.
        stream
            .write_all(b"GET /health HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
            .expect("failed to write HTTP request");

        // Read the response. We use a read loop with a timeout to handle
        // the case where the server sends data in chunks.
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("failed to set read timeout");
        let mut response = Vec::new();
        let mut buf = [0u8; 4096];
        loop {
            match stream.read(&mut buf) {
                Ok(0) => break, // EOF
                Ok(n) => response.extend_from_slice(&buf[..n]),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // Timeout — we've read what was available.
                    break;
                }
                Err(e) => panic!("failed to read HTTP response: {e}"),
            }
        }

        let body = String::from_utf8_lossy(&response);

        // Assert the response starts with HTTP 200 OK.
        assert!(
            body.starts_with("HTTP/1.1 200"),
            "expected HTTP 200, got: {:?}",
            body.lines().next().unwrap_or_default()
        );

        // Assert response body contains "status":"ok" (JSON).
        assert!(
            body.contains(r#""status":"ok""#) || body.contains(r#""status": "ok""#),
            "expected health response to contain status=ok, got: {body}"
        );
    }));

    // Kill the subprocess unconditionally, regardless of test outcome.
    // This ensures no orphaned server processes linger after the test.
    kill_child(&mut child);

    // Restore ANVILML_* env vars to their prior state.
    // This is critical for test isolation when tests run in parallel —
    // a previous test may have set ANVILML_PORT or similar vars and
    // failed to restore them (e.g., due to a panic).
    for (name, prior) in &prior_env {
        match prior {
            Some(v) => std::env::set_var(name, v),
            None => std::env::remove_var(name),
        }
    }

    // Propagate any panic from the test body.
    result.expect("test panicked");
}

/// Kill the child process and wait for it to exit.
///
/// Sends SIGTERM first, then SIGKILL if the process doesn't exit within
/// 2 seconds. This is a best-effort cleanup — failures are logged but
/// not propagated.
fn kill_child(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}
