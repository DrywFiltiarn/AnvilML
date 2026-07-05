# Plan Report: P9-E1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P9-E1                                       |
| Phase       | 009 — Real Worker Startup                   |
| Description | anvilml-worker: integration test, real subprocess sends Ready |
| Depends on  | P9-D3                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-07-05T18:45:00Z                        |
| Attempt     | 1                                           |

## Objective

Create `crates/anvilml-worker/tests/real_startup_tests.rs` — a new integration test file that proves, end-to-end against a genuine spawned `worker_main.py` subprocess, that real-mode worker startup works. The test binds a `RouterTransport`, spawns the worker with `ANVILML_DEVICE_TYPE=cpu` and `ANVILML_WORKER_MOCK` unset, receives the `Ready` event via `transport.recv()`, and asserts `capabilities_source == "pytorch"` and `node_types` is empty. This is the phase's Runnable Proof.

## Scope

### In Scope
- Create `crates/anvilml-worker/tests/real_startup_tests.rs` with a single integration test (`test_real_subprocess_sends_ready`) that:
  - Binds a `RouterTransport` on an OS-assigned port.
  - Builds a worker environment via `WorkerEnv::build()` targeting CPU, real mode.
  - Spawns `worker/worker_main.py` via `spawn_worker()`.
  - Connects a `zeromq::DealerSocket` to the bound endpoint.
  - Receives the `Ready` event from the transport with an explicit timeout.
  - Asserts `capabilities_source == "pytorch"` and `node_types.is_empty()`.
  - Terminates the subprocess and waits for it to exit.
- No other files are created or modified.

### Out of Scope
None. `defers_to (from JSON): []` — this task has no deferrals. All described functionality is implemented in full.

## Existing Codebase Assessment

**What exists:** The `anvilml-worker` crate (v0.1.25) already provides `spawn_worker()` in `spawn.rs`, `WorkerEnv::build()` in `env.rs`, and the full `RouterTransport` with `bind()`, `send()`, and `recv()` in `anvilml-ipc` (v0.1.11). The `WorkerEvent::Ready` variant is fully defined in `messages.rs` with all fields including `capabilities_source` and `node_types`. The `zeromq 0.6.0` crate is already in `anvilml-worker`'s dev-dependencies with `tokio-runtime` feature. The `worker/worker_main.py` file exists with a real-mode startup sequence (`_real_startup_sequence()`) that imports torch, runs the real capability probe, and sends `Ready` with `capabilities_source: "pytorch"`.

**Established patterns:** Integration tests in `crates/anvilml-worker/tests/` (e.g., `bridge_tests.rs`, `spawn_tests.rs`) use `#[tokio::test]` attribute macros, construct `RouterTransport` via `RouterTransport::bind().await`, create DEALER sockets with `zeromq::DealerSocket`, and use `tokio::time::timeout()` for bounded waits. The `connect_dealer()` helper pattern in `bridge_tests.rs` (set socket options with `PeerIdentity`, connect to `tcp://127.0.0.1:{port}`, sleep 50ms for ROUTER registration) is the established pattern. Error handling uses `expect()` for setup failures and explicit assertions for outcomes.

**Gap between design and source:** None. The design doc (§14.2) real-mode startup sequence matches the actual `worker_main.py` implementation exactly — the Ready event is sent with `capabilities_source: "pytorch"` and `node_types: []` (empty, since Phase 10's node system doesn't exist yet). The `RouterTransport::recv()` method correctly deserializes the msgpack payload into `WorkerEvent` via `rmp_serde::from_slice`.

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| crate  | zeromq  | 0.6.0           | rust-docs MCP  | tokio-runtime          |

The `zeromq 0.6.0` crate is already listed in `anvilml-worker/Cargo.toml` dev-dependencies as `version = "0.6"` with `default-features = false, features = ["tokio-runtime"]`. The `DealerSocket`, `SocketOptions`, `PeerIdentity`, and `ZmqMessage` types used in the test are confirmed present in this version via MCP. No new dependencies are introduced.

## Approach

1. **Create the test file** `crates/anvilml-worker/tests/real_startup_tests.rs`.

2. **Module doc comment.** Add a `//!` crate-level doc comment describing the file's purpose: proving real-mode worker startup end-to-end against a live subprocess, with the acceptance criterion.

3. **Imports.** Import the following:
   - `anvilml_ipc::RouterTransport` — for binding the ROUTER socket
   - `anvilml_ipc::WorkerEvent` — for type-safe event deserialization
   - `anvilml_worker::{spawn_worker, WorkerEnv}` — for subprocess spawning and env building
   - `anvilml_core::DeviceType` — for specifying CPU device type
   - `std::path::Path` — for the venv path argument
   - `std::time::Duration` — for timeout configuration
   - `zeromq::prelude::*` — for socket traits (`SocketConnect`, `SocketSend`, etc.)
   - `zeromq::{DealerSocket, SocketOptions, PeerIdentity}` — for DEALER socket construction
   - `bytes::Bytes` — for peer identity bytes
   - `rmp_serde` — for serialization helpers if needed

4. **Test function `test_real_subprocess_sends_ready`.** Annotated `#[tokio::test]`. This is the only test in the file.

   a. **Bind the transport.** Call `RouterTransport::bind().await.expect("bind should succeed")`. This binds a ROUTER socket on `tcp://127.0.0.1:0` (OS-assigned port). The port number is available via `transport.port`.

   b. **Build the worker environment.** Call `WorkerEnv::build()` with:
      - `ipc_port = transport.port`
      - `worker_id = "0"`
      - `device_index = 0`
      - `device_type = DeviceType::Cpu`
      - `mock = false` (this ensures `ANVILML_WORKER_MOCK` is NOT injected into the env map — its absence signals real mode)
      - `log_level = "info"`
      - `max_ipc_payload_mib = 256`

   c. **Spawn the worker subprocess.** Call `spawn_worker(Path::new("worker/.venv"), env).await`. The venv path `"worker/.venv"` is the project-standard venv location per `ENVIRONMENT.md §3`. This spawns `python3 worker/worker_main.py` with the built environment. The subprocess will:
      - Read `ANVILML_IPC_PORT`, `ANVILML_WORKER_ID`, `ANVILML_DEVICE_TYPE=cpu`, `ANVILML_DEVICE_INDEX=0` from env.
      - Import torch, select CPU device (no-op for CPU).
      - Run the real `probe_capabilities()` which returns CPU probe results.
      - Call `_import_nodes()` which returns `[]`.
      - Send a `Ready` event with `capabilities_source: "pytorch"` and `node_types: []`.
      - Enter the dispatch loop (blocking).

   d. **Connect a DEALER socket.** Follow the established pattern from `bridge_tests.rs`:
      - Create `SocketOptions`, set `PeerIdentity` to `Bytes::from("0")`.
      - Create `DealerSocket::with_options(opts)`.
      - Connect to `format!("tcp://127.0.0.1:{}", transport.port)`.
      - Sleep 50ms for the ROUTER to register the DEALER identity.

   e. **Receive the Ready event.** Call `transport.recv()` wrapped in `tokio::time::timeout(Duration::from_secs(10), ...)`. The 10-second timeout accounts for torch import on CPU (the heaviest part of the startup sequence). If the timeout fires, the test panics with a message including the subprocess's captured stderr:
      ```rust
      let (identity, event) = tokio::time::timeout(
          Duration::from_secs(10),
          transport.recv(),
      )
      .await
      .expect("worker should send Ready event within 10s")
      .expect("recv should succeed");
      ```
      If the timeout fires (outer `expect`), terminate the subprocess and capture its stderr before failing.

   f. **Assert on the Ready event.** Match on `event`:
      - Assert `matches!(event, WorkerEvent::Ready { .. })` — the event must be a Ready variant.
      - Extract fields from the Ready variant and assert:
        - `capabilities_source == "pytorch"` — proves real-mode torch probe ran.
        - `node_types.is_empty()` — correct for Phase 9 (no nodes registered).

   g. **Clean up.** Terminate the subprocess:
      - `child.kill().await.expect("kill should succeed")`
      - `child.wait().await.expect("wait should succeed")`
      This ensures the subprocess doesn't linger after the test completes.

5. **Environment variable isolation.** The test does not call `std::env::set_var` — it only spawns a subprocess with an isolated environment map. No `#[serial]` annotation is needed for env isolation. However, since the test binds a real socket and spawns a real subprocess, and the acceptance criterion mandates `--test-threads=1`, no additional serialisation mechanism is required beyond what the caller provides.

6. **Logging.** No tracing calls are needed in the test file — tests don't emit operational logs. The worker subprocess itself will log via its own `logging.basicConfig()` call in `worker_main.py`.

## Public API Surface

None. This task creates only a test file under `tests/` — no new `pub` items are introduced. The test uses existing public APIs:
- `RouterTransport::bind()` (from `anvilml-ipc`)
- `RouterTransport::recv()` (from `anvilml-ipc`)
- `spawn_worker()` (from `anvilml-worker`)
- `WorkerEnv::build()` (from `anvilml-worker`)

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-worker/tests/real_startup_tests.rs` | New integration test file with `test_real_subprocess_sends_ready` |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `crates/anvilml-worker/tests/real_startup_tests.rs` | `test_real_subprocess_sends_ready` | A real `worker_main.py` subprocess spawned with CPU device and no mock flag connects over IPC, runs the real torch capability probe, and sends a `Ready` event with `capabilities_source="pytorch"` and empty `node_types` within 10 seconds | `cargo test -p anvilml-worker --test real_startup_tests -- --test-threads=1` exits 0 |

## CI Impact

No CI changes required. The test is a Rust integration test under `crates/anvilml-worker/tests/`, which is automatically picked up by `cargo test --workspace --features mock-hardware` in the CI `rust-linux` and `rust-windows` jobs. The test uses `--features mock-hardware` for compilation but runs against a real subprocess (not mock-IPC), so it exercises the real code path. The test requires the Python venv to be provisioned (`worker/.venv`) — this is already done in CI before any test step (ENVIRONMENT.md §2, Step 3).

## Platform Considerations

None identified. The test uses `Path::new("worker/.venv")` which works on both Linux and Windows (the `spawn_worker()` function handles platform-specific interpreter path resolution internally). The `RouterTransport::bind()` uses TCP loopback which is identical on both platforms. The `--test-threads=1` flag avoids port/subprocess contention across test cases.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `torch` import in the real-mode subprocess is slow on CPU (1–5 seconds), causing the 10-second timeout to be tight | Medium | High | The 10-second timeout is generous for torch CPU import (typically < 3s on a provisioned environment). If CI runners are slower, increase to 15s. The timeout is explicit and bounded per §11.5. |
| The Python venv is not provisioned at test runtime, causing `spawn_worker()` to fail with `AnvilError::Io(NotFound)` | Low | High | The acceptance command runs `cargo test` which is invoked after venv provisioning in CI. For local runs, the developer must provision first (`bash scripts/install_worker_deps.sh`). This is a precondition, not a defect. |
| The worker subprocess hangs after sending Ready (enters dispatch loop that blocks on `ipc.recv_message()`), preventing clean test teardown | Low | Medium | The test calls `child.kill().await` after receiving the Ready event, which sends SIGKILL to the subprocess. This is guaranteed to terminate it regardless of what the dispatch loop is doing. |
| ZeroMQ ROUTER socket does not have the DEALER's identity registered yet when `transport.recv()` is called, causing a timeout | Low | Medium | The test follows the established 50ms sleep pattern from `bridge_tests.rs` after connecting the DEALER, which gives the ROUTER sufficient time to register the identity. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-worker --test real_startup_tests -- --test-threads=1` exits 0 (after `bash scripts/install_worker_deps.sh` provisions the CPU venv)
- [ ] `head -1 .forge/reports/P9-E1_plan.md` prints `# Plan Report: P9-E1`
- [ ] `grep "^## " .forge/reports/P9-E1_plan.md` shows exactly 12 section headings
- [ ] `wc -l .forge/reports/P9-E1_plan.md` returns a value > 40
