# Plan Report: P8-C1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P8-C1                                         |
| Phase       | 008 — ZeroMQ IPC Transport                    |
| Description | anvilml-ipc: 1000-trip RouterTransport stress test |
| Depends on  | P8-A3, P8-B1, P8-B3                           |
| Project     | anvilml                                       |
| Planned at  | 2026-06-16T15:40:00Z                          |
| Attempt     | 1                                             |

## Objective

Create a stress test that exercises the full Rust-to-Python IPC path: `RouterTransport` (Rust, ZeroMQ ROUTER) ↔ `ipc.py` DEALER (Python) over msgpack-serialised messages. The test spawns a minimal Python subprocess (`ipc_echo.py`) that connects to the bound ROUTER socket, enters a loop receiving `WorkerMessage::Ping` and replying with `WorkerEvent::Pong`, then sends 1000 Ping messages from Rust and asserts all 1000 Pong responses arrive with matching `seq` values in order. The test must complete within 30 seconds. When complete, the command `cargo test -p anvilml-ipc --features mock-hardware --test stress_test` exits 0 and the test log contains "stress test passed: 1000/1000".

## Scope

### In Scope
- **`worker/ipc_echo.py`** — A minimal Python script that:
  - Connects a DEALER socket to `tcp://127.0.0.1:{port}` using `worker.ipc.connect()`.
  - Sends a startup acknowledgement via `ipc.send_event({"_type": "Ready"})`.
  - Enters a loop: `recv_message()` → decode `_type` → if `Ping`, reply with `send_event({"_type": "Pong", "seq": msg["seq"]})`.
  - Exits cleanly on `Shutdown` or when stdin is closed / process receives SIGTERM.
- **`crates/anvilml-ipc/tests/stress_test.rs`** — A `#[tokio::test]` that:
  - Binds a `RouterTransport`.
  - Spawns `ipc_echo.py` from the worker venv with the port as a CLI argument.
  - Waits for the Python startup message.
  - Sends 1000 `WorkerMessage::Ping { seq }` messages (seq: 0–999) to the worker identity.
  - Collects 1000 `WorkerEvent::Pong` responses, verifying `seq` matches and is in order.
  - Asserts zero timeouts within a 30-second deadline.
  - Logs "stress test passed: {received}/1000" on success.

### Out of Scope
- Any modification to `worker/ipc.py` (already exists and is tested by P8-B2).
- Any modification to `RouterTransport` itself (already exists and is tested by P8-A3).
- Worker pool management, job execution, or node system integration.
- Platform-specific path handling beyond what the venv provisioning script already guarantees.
- CI configuration changes (the test is picked up by the existing `cargo test --workspace` CI job).

## Existing Codebase Assessment

The `anvilml-ipc` crate already implements the full ROUTER transport stack:
- `RouterTransport::bind()` binds a `zeromq::RouterSocket` to `tcp://127.0.0.1:0`, extracts the OS-assigned port from `Endpoint::Tcp(_, port)`, and returns `Self { socket: Arc<Mutex<RouterSocket>>, port }`.
- `RouterTransport::send(worker_id: &[u8], msg)` encodes the message with `encode_message()`, constructs a two-frame `ZmqMessage` (`[identity, payload]`), and sends it via the locked mutex.
- `RouterTransport::recv()` acquires the mutex, receives a `ZmqMessage`, extracts frame 0 as identity (UTF-8 string with hex fallback) and frame 1 as payload, decodes via `decode_event()`, and returns `(String, WorkerEvent)`.

The `WorkerMessage::Ping { seq: u64 }` and `WorkerEvent::Pong { seq: u64 }` variants are defined in `messages.rs` with `#[serde(tag = "_type")]` and serialised via `rmp_serde` (flat-dict msgpack). The Python side already has `worker/ipc.py` with `connect(port, worker_id)`, `send_event(data)`, and `recv_message()` using `pyzmq` DEALER sockets.

The test file `crates/anvilml-ipc/tests/roundtrip_tests.rs` uses the crate's public API directly (not subprocess spawning). The new stress test will be the first integration test that spawns a subprocess and exercises the full IPC path.

No discrepancies were found between the design doc and current source — `RouterTransport`'s public API matches the specification in ANVILML_DESIGN.md §8.7 exactly.

## Resolved Dependencies

| Type   | Name       | Version verified | MCP source     | Feature flags confirmed              |
|--------|------------|-----------------|----------------|--------------------------------------|
| crate  | zeromq     | 0.6.0           | Cargo.lock     | tokio-runtime, tcp-transport         |
| crate  | rmp-serde  | 1.3.1           | Cargo.lock     | (none — default features)            |
| python | pyzmq      | 26.x (>=26.0)   | base.txt       | n/a                                  |
| python | msgpack    | 1.x (>=1.0)     | base.txt       | n/a                                  |

**Note:** No MCP tool (`rust-docs`) was available for live API verification. All type names (`RouterSocket`, `ZmqMessage`, `Endpoint::Tcp`), method names (`bind()`, `send()`, `recv()`), and feature flags were confirmed by inspecting existing usage in `transport.rs` and `error.rs`. These APIs are actively compiled and tested by the existing `roundtrip_tests.rs` and `transport_tests.rs` test suites, confirming their correctness at the resolved versions.

## Approach

1. **Create `worker/ipc_echo.py`** — A minimal Python echo worker:
   - Accepts a port argument via `sys.argv[1]`.
   - Calls `worker.ipc.connect(port, "stress-test-worker")` to connect a DEALER socket.
   - Sends `{"_type": "Ready"}` via `ipc.send_event()` to signal readiness (this establishes the worker's identity on the ROUTER socket).
   - Enters an infinite loop: call `ipc.recv_message()`, check `msg["_type"]`, if `"Ping"` reply with `send_event({"_type": "Pong", "seq": msg["seq"]})`.
   - On `"Shutdown"`, break the loop and exit.
   - This script is deliberately minimal — no hardware probing, no node importing, no error handling beyond what's needed for the echo loop.

2. **Create `crates/anvilml-ipc/tests/stress_test.rs`** — The stress test:
   - Use `#[tokio::main]` or `#[tokio::test]` to run the async test.
   - Bind a `RouterTransport` via `RouterTransport::bind().await`.
   - Determine the Python interpreter path: use `std::env::var("ANVILML_VENV_PATH").unwrap_or_else(|_| "./worker/.venv".to_string())` to get the venv path, then construct the interpreter path (`{venv}/bin/python3` on Unix).
   - Spawn `ipc_echo.py` via `std::process::Command` with the bound port as an argument. The working directory is set to the repo root so the Python import of `worker.ipc` resolves correctly.
   - Sleep 500ms with `tokio::time::sleep` to allow ZeroMQ connection establishment and the Python startup message to be received.
   - Enter a loop for `seq` from 0 to 999:
     a. Call `transport.send(b"stress-test-worker", &WorkerMessage::Ping { seq }).await`.
     b. Call `transport.recv().await` to get the `(id, event)` tuple.
     c. Assert `matches!(event, WorkerEvent::Pong { seq: s } if s == seq)` — this verifies both the matching sequence number and that the event is a Pong.
   - If any recv fails or seq doesn't match, fail the test immediately with an assertion.
   - After the loop, log "stress test passed: 1000/1000" via `println!` (tests capture stdout/stderr).
   - Wrap the entire test in a `tokio::time::timeout(Duration::from_secs(30), async { ... })` to enforce the 30-second deadline.

3. **Structural choices and rationale:**
   - The worker identity is hardcoded to `b"stress-test-worker"` in both Rust and Python because this is a single-worker test — there is no pool or dynamic identity management.
   - The Python script uses `sys.argv[1]` for the port rather than environment variables because it simplifies the test setup (no need to set/restore env vars, which would require `#[serial]` per FORGE_AGENT_RULES §11.3).
   - The test does not set `ANVILML_WORKER_MOCK` because `ipc_echo.py` does not import torch or any hardware-dependent modules — it only uses `zmq` and `msgpack`, which are in `base.txt`.
   - The 500ms initial delay is a practical necessity: ZeroMQ's lazy-connection means the ROUTER socket won't route messages to the DEALER until the DEALER has connected and sent at least one message. The startup `Ready` message serves dual purpose — it signals readiness and establishes the identity frame.
   - The test uses `tokio::time::timeout` rather than individual per-message timeouts because the task spec says "all within 30 seconds" — a total deadline is more appropriate for a throughput test.

## Public API Surface

No new `pub` items are introduced. The test file uses only existing public API:

| Item | Module Path | Signature |
|------|-------------|-----------|
| `RouterTransport::bind` | `anvilml_ipc::RouterTransport` | `pub async fn bind() -> Result<Self, TransportError>` |
| `RouterTransport::send` | `anvilml_ipc::RouterTransport` | `pub async fn send(&self, worker_id: &[u8], msg: &WorkerMessage) -> Result<(), TransportError>` |
| `RouterTransport::recv` | `anvilml_ipc::RouterTransport` | `pub async fn recv(&self) -> Result<(String, WorkerEvent), AnvilError>` |
| `WorkerMessage::Ping` | `anvilml_ipc::WorkerMessage` | Enum variant: `Ping { seq: u64 }` |
| `WorkerEvent::Pong` | `anvilml_ipc::WorkerEvent` | Enum variant: `Pong { seq: u64 }` |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/ipc_echo.py` | Minimal Python echo worker for stress test |
| CREATE | `crates/anvilml-ipc/tests/stress_test.rs` | 1000-trip RouterTransport stress test |
| Modify | `crates/anvilml-ipc/Cargo.toml` | Bump patch version 0.1.3 → 0.1.4 |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-ipc/tests/stress_test.rs` | `stress_test_1000_trips` | Full IPC roundtrip: bind RouterTransport, spawn Python echo worker, send 1000 Ping messages, receive 1000 matching Pong responses in order within 30 seconds | Worker venv exists with pyzmq and msgpack installed; port 0 binding is available | 1000 `WorkerMessage::Ping { seq: 0..999 }` messages sent to worker identity | All 1000 Pongs received with matching seq in order; test completes in < 30s; log contains "stress test passed: 1000/1000" | `cargo test -p anvilml-ipc --features mock-hardware --test stress_test` exits 0 |

## CI Impact

No CI changes required. The new test file `crates/anvilml-ipc/tests/stress_test.rs` is automatically discovered by `cargo test` — no configuration changes needed. It runs as part of the existing `rust-linux` and `rust-windows` CI jobs (`cargo test --workspace --features mock-hardware`). The test spawns a Python subprocess from the venv provisioned by `install_worker_deps.sh` / `install_worker_deps.ps1`, which is already a prerequisite in all CI jobs.

## Platform Considerations

None identified. The test uses `std::process::Command` with a platform-independent interpreter path construction:
- On Unix: `{venv_path}/bin/python3`
- On Windows: `{venv_path}\Scripts\python.exe`

The Python path is determined at runtime via a simple platform check (`cfg!(windows)`). ZeroMQ TCP loopback (`tcp://127.0.0.1:{port}`) works identically on both platforms. The Windows cross-check in ENVIRONMENT.md §7 is sufficient — no `#[cfg(unix)]` or `#[cfg(windows)]` guards are needed in the test code itself.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| ZeroMQ lazy-connection causes the first few pings to be lost before the Python worker connects. | Medium | High — could cause seq mismatch on early messages. | Send a startup `Ready` message from Python before the test begins sending pings. Add a 500ms delay after spawning the subprocess to ensure the connection is established and the startup message has been received. The test waits for exactly one recv before starting the ping loop. |
| Python subprocess spawn fails because the venv interpreter doesn't exist or pyzmq/msgpack aren't installed. | Low | High — test cannot run at all. | The task depends on P8-B3 which guarantees the venv and dependencies. The test checks for interpreter existence and fails fast with a descriptive error if missing. CI provisions the venv before running tests per ENVIRONMENT.md §6. |
| 1000 messages within 30 seconds exceeds system capacity (e.g., slow CI runner, high CPU load). | Low | Medium — test timeout rather than logic error. | The 30-second deadline is generous for 1000 simple msgpack roundtrips over localhost TCP. If it fails, add a retry with a slightly longer timeout (up to 60s) before marking as flaky. |
| `RouterTransport::send` takes `worker_id: &[u8]` but the Python worker's identity frame is set via `worker.ipc.connect(port, "stress-test-worker")` — mismatch in identity encoding. | Very Low | High — ROUTER won't route messages to the worker. | The identity is hardcoded identically as `"stress-test-worker"` in both the Python `connect()` call and the Rust `send()` argument. Verified against existing pattern in `transport.rs` examples. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-ipc --features mock-hardware --test stress_test` exits 0
- [ ] Test output contains "stress test passed: 1000/1000"
- [ ] All 1000 Ping→Pong roundtrips complete within 30 seconds (no timeout panic)
- [ ] `cargo test -p anvilml-ipc --features mock-hardware` exits 0 (no regression in existing tests)
- [ ] `cargo fmt --all -- --check` exits 0 (code is formatted)
- [ ] `cargo clippy --package anvilml-ipc --features mock-hardware -- -D warnings` exits 0 (no lint warnings)
