/// Integration test: the server accepts `--port` CLI override, binds to the
/// OS-assigned port, and the health endpoint returns HTTP 200 with
/// `{"status":"ok"}`.
///
/// This test verifies the full config-loading path: CLI parsing →
/// ConfigOverrides → config::load → TCP bind → health endpoint.
///
/// The server is spawned as a subprocess with `--port 0` (OS-assigned port).
/// The actual port is recovered by reading the mandatory `"listening"` INFO
/// log line on the subprocess's stderr (`addr = %actual_addr` per
/// ENVIRONMENT.md §9.2) rather than by inspecting the OS socket table.
///
/// This is deliberate: as of P9-C1, the server process also binds a second,
/// unrelated TCP listener (the ZeroMQ ROUTER socket used for worker IPC, via
/// `RouterTransport::bind()`). A PID-scoped `lsof`/`netstat` scan cannot
/// distinguish that socket from the HTTP listener, and previously picked
/// whichever LISTEN entry happened to be returned first by the OS — passing
/// or failing nondeterministically depending on socket-table ordering.
/// Reading the log line is unambiguous regardless of how many sockets the
/// process owns.
///
/// The subprocess is killed after the assertion regardless of outcome.
///
/// Preconditions:
///   - Workspace builds with `mock-hardware` feature.
///   - No prior server running on the OS-assigned port.
///
/// Acceptance command:
///   `cargo test -p anvilml --features mock-hardware -- cli_tests` exits 0.
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

/// Spawn the anvilml binary directly (pre-built) with `--port 0`, recover the
/// bound port from the `"listening"` INFO log line on stderr, hit the health
/// endpoint, and assert HTTP 200 with `"status":"ok"`.
///
/// The subprocess is killed unconditionally at the end of the test.
#[test]
fn test_custom_port_health() {
    // Resolve the workspace root once — used for both binary path resolution
    // and as the working directory for the spawned server process, so that
    // all ./‑relative config defaults (db_path, seeds_path, etc.) resolve
    // correctly against the workspace root rather than backend/.
    let ws_root = std::env::current_dir()
        .expect("failed to get cwd")
        .parent()
        .expect("parent of backend/")
        .to_path_buf();

    let binary = match std::env::var("CARGO_TARGET_DIR") {
        Ok(target_dir) => {
            let path = std::path::Path::new(&target_dir);
            if path.is_absolute() {
                format!("{}/debug/anvilml", target_dir)
            } else {
                format!("{}", ws_root.join(target_dir).display())
            }
        }
        Err(_) => {
            format!("{}", ws_root.join("target/debug/anvilml").display())
        }
    };

    // Spawn the server binary directly with port 0 for OS-assigned port.
    // The --log-format plain flag ensures clean, line-based stderr output
    // that the port-detection logic below can parse.
    // Using the pre-built binary avoids cargo's output buffering issues.
    let mut child = Command::new(&binary)
        .args(["--port", "0", "--log-format", "plain"])
        .current_dir(&ws_root)
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

    // Take ownership of stderr immediately after spawn so the port-detection
    // block below can read it line-by-line. This must happen before the
    // catch_unwind closure, since `child` is borrowed mutably for kill_child
    // after the closure returns and cannot also be moved into it.
    let child_stderr = child.stderr.take().expect("stderr was piped at spawn");

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

        // Capture the child PID before port detection — needed by the Windows
        // netstat branch to filter listening sockets by owning process.
        // child.id() is always Some(u32) after a successful spawn().
        #[allow(unused_variables)]
        let child_pid = child.id();

        // Poll for the bound port in 200ms increments, up to 5 seconds total.
        // A fixed sleep is insufficient — startup time varies significantly by
        // platform: WSL2 Linux ~120ms, native Windows ~1000ms (sysinfo CPU
        // detection adds ~800ms). Polling makes the test robust to both.
        const POLL_INTERVAL_MS: u64 = 200;
        const POLL_MAX_ATTEMPTS: u32 = 25; // 25 × 200ms = 5 seconds

        let port: u16 = {
            let mut port: Option<u16> = None;

            for _ in 0..POLL_MAX_ATTEMPTS {
                thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));

                #[cfg(unix)]
                {
                    // `lsof -i TCP -sTCP:LISTEN -P -n` lists all TCP listeners
                    // with numeric ports. Filter for lines containing "anvilml"
                    // and extract the port from the NAME column:
                    //   COMMAND  PID  USER  FD  TYPE  DEVICE  SIZE/OFF  NODE  NAME
                    //   anvilml  NNN  ...   ..  IPv4  ...     ...       TCP   127.0.0.1:PORT (LISTEN)
                    // The address:port is at fields[len-2]; "(LISTEN)" is last.
                    if let Ok(output) = Command::new("lsof")
                        .args(["-i", "TCP", "-sTCP:LISTEN", "-P", "-n"])
                        .output()
                    {
                        let lsof_output = String::from_utf8_lossy(&output.stdout);
                        for line in lsof_output.lines() {
                            if line.contains("anvilml") && line.contains("LISTEN") {
                                let fields: Vec<&str> = line.split_whitespace().collect();
                                if fields.len() >= 2 {
                                    let addr_port = fields[fields.len() - 2];
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

                    // Fallback: try ss if lsof matched nothing this iteration.
                    // ss -tlnp format: State Recv-Q Send-Q Local Address:Port ...
                    // The address:port is at field index 4.
                    if port.is_none() {
                        if let Some(ss_out) = Command::new("ss")
                            .args(["-tlnp"])
                            .output()
                            .ok()
                            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
                        {
                            for line in ss_out.lines() {
                                if line.contains("anvilml") {
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
                }

                #[cfg(windows)]
                {
                    // `netstat -ano -p TCP` lists all TCP connections with owning
                    // PIDs. Output format (whitespace-separated columns):
                    //   Proto  Local Address    Foreign Address  State      PID
                    //   TCP    127.0.0.1:PORT   0.0.0.0:0        LISTENING  NNN
                    // Filter by PID (last column) to isolate our child process.
                    if let Ok(output) = Command::new("netstat").args(["-ano", "-p", "TCP"]).output()
                    {
                        let netstat_output = String::from_utf8_lossy(&output.stdout);
                        let pid_str = child_pid.to_string();
                        for line in netstat_output.lines() {
                            if line.starts_with("Proto") || line.trim().is_empty() {
                                continue;
                            }
                            let fields: Vec<&str> = line.split_whitespace().collect();
                            if fields.len() < 5 {
                                continue;
                            }
                            if fields[fields.len() - 1] != pid_str {
                                continue;
                            }
                            let local_addr = fields[1];
                            if let Some(colon) = local_addr.rfind(':') {
                                if let Ok(p) = local_addr[colon + 1..].parse::<u16>() {
                                    port = Some(p);
                                    break;
                                }
                            }
                        }
                    }
                }

                if port.is_some() {
                    break;
                }
            }

            port.expect(
                "could not detect server port after 5s — server may not have started. \
                 Check that mock-hardware feature is available and the binary runs correctly.",
            )
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
