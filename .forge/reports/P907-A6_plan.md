# Plan Report: P907-A6

| Field | Value |
|-------|-------|
| Task ID | P907-A6 |
| Phase | 907 — ZeroMQ IPC Transport |
| Description | worker/tests: update test_ipc.py and test_worker_main.py for ZeroMQ transport |
| Depends on | P907-A5 (ZeroMQ DEALER transport in worker_main.py and ipc.py) |
| Project | anvilml |
| Planned at | 2026-06-13T16:22:00Z |
| Attempt | 1 |

## Objective

Update the Python worker test suite to work with the new ZeroMQ DEALER transport (P907-A5).
`test_ipc.py` is already rewritten for ZMQ PAIR sockets. `test_worker_main.py` must be
rewritten from stdin/stdout length-prefixed framing to ZMQ DEALER socket communication,
using `ANVILML_IPC_PORT` instead of `ANVILML_IPC_SOCKET`, and removing `stdin=PIPE` /
`stdout=PIPE` subprocess arguments. All 14 skipped tests must be unskipped and functional.

## Scope

### In Scope
- `worker/tests/test_worker_main.py`: rewrite all tests for ZMQ DEALER transport
- `worker/tests/test_ipc.py`: confirm no changes needed (already ZMQ PAIR)

### Out of Scope
- `worker/ipc.py` changes (already done in P907-A5)
- `worker/worker_main.py` changes (already done in P907-A5)
- Rust-side managed.rs changes (already done in P907-A5)
- Any crate version bumps (no source files modified)
- CI workflow file changes

## Approach

### Step 1: Confirm test_ipc.py is complete
The file `worker/tests/test_ipc.py` already uses `_zmq_pair()` (PAIR sockets),
`_monkeypatch_sock()` (patching `ipc._sock`), and msgpack roundtrip tests. No changes needed.

### Step 2: Rewrite test_worker_main.py

#### 2.1 Replace module docstring
Update the module-level docstring to reflect ZMQ DEALER transport instead of
"stdin/stdout pipes with length-prefix framing." Remove the skip notice.

#### 2.2 Remove obsolete imports and helpers
- Remove `import struct` (no length prefix)
- Remove `_make_frame()` (no length-prefixed framing)
- Remove `_parse_frames()` (no pipe-based frame parsing — each ZMQ message is a single msgpack frame)

#### 2.3 Add `_ZmqTransport` helper class
```python
class _ZmqTransport:
    """Thin wrapper around a zmq.DEALER socket for test communication."""
    def __init__(self, socket: zmq.Socket):
        self._sock = socket

    def send(self, data: dict) -> None:
        self._sock.send(msgpack.packb(data, use_bin_type=True))

    def recv(self, timeout_ms: int = 10000) -> dict:
        self._sock.setsockopt(zmq.RCVTIMEO, timeout_ms)
        data = self._sock.recv()
        return msgpack.unpackb(data, raw=False)
```

#### 2.4 Rewrite `_spawn_worker()`
```python
def _spawn_worker(self, worker_id: str = "test-0", device_index: int = 0):
    """Spawn the worker in mock mode with ZMQ DEALER transport.

    Binds a DEALER socket on an ephemeral port, passes the port via
    ANVILML_IPC_PORT, and returns (worker_proc, transport).
    """
    ctx = zmq.Context()
    sock = ctx.socket(zmq.DEALER)
    sock.bind("tcp://127.0.0.1:0")
    endpoint = sock.getsockopt(zmq.LAST_ENDPOINT).decode()  # e.g. "tcp://127.0.0.1:54321"
    port = int(endpoint.split(":")[-1])

    env = os.environ.copy()
    env["ANVILML_WORKER_MOCK"] = "1"
    env["ANVILML_IPC_PORT"] = str(port)

    proc = subprocess.Popen(
        [sys.executable, _WORKER_SCRIPT, "--worker-id", worker_id,
         "--device-index", str(device_index)],
        stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE, env=env,
    )
    transport = _ZmqTransport(sock)
    return proc, transport, ctx
```

Key changes from old implementation:
- `ANVILML_IPC_SOCKET` → `ANVILML_IPC_PORT` (env var name and value)
- `stdin=subprocess.PIPE, stdout=subprocess.PIPE` → `stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL`
- Worker communicates via ZMQ socket, not stdio pipes
- Returns `(proc, transport, ctx)` for cleanup

#### 2.5 Rewrite each test method

Every test follows this pattern:
1. `proc, transport, ctx = self._spawn_worker(...)`
2. Send `InitializeHardware` via `transport.send()`
3. Read and assert `Ready` event via `transport.recv()`
4. Send test-specific message(s) via `transport.send()`
5. Read and assert response(s) via `transport.recv()`
6. Send `Shutdown` via `transport.send()`
7. Read `Dying` event via `transport.recv()`
8. Assert `proc.returncode == 0`
9. Cleanup: `proc.kill()` if needed, `sock.close()`, `ctx.term()`

**test_ready_on_init_hardware:** Send InitializeHardware → assert Ready(worker_id=test-0, device_index=0) → send Shutdown → assert Dying(reason=shutdown) → assert exit 0.

**test_mock_values:** Same as above, additionally assert Ready contains vram_total_mib=8192, vram_free_mib=8192, arch=gfx1100, fp16=True, bf16=True, flash_attention=False.

**test_ping_pong:** Send InitializeHardware → Ready → send Ping{seq=42} → assert Pong{seq=42} → send Shutdown → Dying → exit 0.

**test_memory_query_report:** Send InitializeHardware → Ready → send MemoryQuery → assert MemoryReport{vram_used_mib=0, ram_used_mib=0} → send Shutdown → Dying → exit 0.

**test_shutdown_dying_exit:** Send InitializeHardware → Ready → send Shutdown → assert Dying(reason=shutdown) → assert exit 0.

**test_double_init_exits:** Send InitializeHardware twice → assert exactly one Ready event → send Shutdown → assert exactly one Dying → assert exit 0. Guards against double-InitializeHardware bug (P10-B1).

**test_execute_progress_completed:** Build mock graph with 3 nodes → send Execute → assert Ready → assert 3 Progress events with correct job_id/node_index/node_total/node_type → assert Completed(job_id, elapsed_ms>=0) → send Shutdown → Dying → exit 0.

**test_execute_saveimage_imageready:** Build graph with SaveImage node → send Execute → assert Ready → assert Progress events → assert ImageReady with width=64, height=64, format='png', valid image_b64, seed>=0, steps=1, prompt='' → assert Completed → Dying → exit 0.

**test_execute_saveimage_seed_resolution:** SaveImage with seed=-1 → assert ImageReady seed in [0, 2^63-1].

**test_execute_saveimage_inputs_resolved:** SaveImage with explicit prompt/seed/steps → assert ImageReady matches those inputs.

**test_execute_no_saveimage_no_imageready:** Graph without SaveImage → assert NO ImageReady events, only Progress + Completed.

**test_cancel_job_during_execute:** Set `ANVILML_MOCK_NODE_DELAY_MS=100`. Send InitializeHardware + Execute → poll ZMQ with timeout until first Progress(node_index=0) arrives → send CancelJob{job_id} → assert Cancelled(job_id) → assert no Completed → send Shutdown → Dying → exit 0.

**test_cancel_before_execute:** Send InitializeHardware + CancelJob{job_id} + Execute{job_id} → assert Cancelled(job_id) → assert NO Progress → assert NO Completed → send Shutdown → Dying → exit 0.

**test_mock_node_delay_ms:** Set `ANVILML_MOCK_NODE_DELAY_MS=75`. Execute 3-node graph → assert Completed with elapsed_ms >= 120 (2 inter-node delays × 75ms = 150ms nominal, 120ms threshold for Windows timer granularity).

#### 2.6 Remove `pytestmark` skip decorator
The `pytestmark = pytest.mark.skip(...)` at module level must be removed so all tests run.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `worker/tests/test_worker_main.py` | Rewrite all tests for ZMQ DEALER transport |
| Read only | `worker/tests/test_ipc.py` | Already ZMQ PAIR — no changes needed |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|-----------------|
| test_worker_main.py | test_ready_on_init_hardware | InitializeHardware → Ready{worker_id, device_index} |
| test_worker_main.py | test_mock_values | Ready payload matches mock spec values (vram, arch, fp16, bf16) |
| test_worker_main.py | test_ping_pong | Ping{seq} → Pong{seq} roundtrip |
| test_worker_main.py | test_memory_query_report | MemoryQuery → MemoryReport{0, 0} |
| test_worker_main.py | test_shutdown_dying_exit | Shutdown → Dying{shutdown} + exit 0 |
| test_worker_main.py | test_double_init_exits | Double InitializeHardware produces exactly one Ready |
| test_worker_main.py | test_execute_progress_completed | Execute 3-node graph → Progress × 3 + Completed |
| test_worker_main.py | test_execute_saveimage_imageready | SaveImage graph → ImageReady with correct fields |
| test_worker_main.py | test_execute_saveimage_seed_resolution | Seed=-1 → valid random seed in [0, 2^63-1] |
| test_worker_main.py | test_execute_saveimage_inputs_resolved | Explicit prompt/seed/steps → ImageReady matches inputs |
| test_worker_main.py | test_execute_no_saveimage_no_imageready | No SaveImage → no ImageReady event |
| test_worker_main.py | test_cancel_job_during_execute | CancelJob mid-execution → Cancelled, no Completed |
| test_worker_main.py | test_cancel_before_execute | CancelJob before Execute → Cancelled, no Progress |
| test_worker_main.py | test_mock_node_delay_ms | ANVILML_MOCK_NODE_DELAY_MS affects elapsed_ms |

## CI Impact

No CI workflow file changes. The Python worker test command
`ANVILML_WORKER_MOCK=1 pytest worker/tests/ -v` (from ENVIRONMENT.md §6 and ARCHITECTURE.md §9)
already runs all tests in `worker/tests/`. The only change is that previously-skipped tests
in `test_worker_main.py` will now execute.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| ZMQ socket timing: worker may not have connected to the bound port when test starts sending messages | Medium | High | Bind socket before spawning worker; use a small sleep or recv timeout to ensure connection is established |
| ZMQ DEALER socket identity frame complexity with ROUTER vs DEALER | Medium | High | Use DEALER socket on both sides (matching supervisor pattern); avoids identity frame handling entirely |
| Subprocess spawn failure (missing python, torch import) | Low | Medium | Worker runs in mock mode (no torch); ANVILML_WORKER_MOCK=1 ensures clean startup |
| Test flakiness due to ZMQ recv timeout | Medium | Medium | Use reasonable timeout (10s default); increase for execute tests that process nodes |
| test_cancel_job_during_execute timing: CancelJob may arrive after worker already finished | Medium | Medium | Use ANVILML_MOCK_NODE_DELAY_MS to create a cooperative cancellation window |
| ZMQ context not cleaned up on test failure | Low | Low | Use try/finally blocks to ensure sock.close() and ctx.term() always run |

## Acceptance Criteria

- [ ] `worker/tests/test_worker_main.py` contains no `pytest.mark.skip` decorator
- [ ] `worker/tests/test_worker_main.py` `_spawn_worker()` sets `ANVILML_IPC_PORT` env var (not `ANVILML_IPC_SOCKET`)
- [ ] `worker/tests/test_worker_main.py` `_spawn_worker()` uses `stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL` (not `PIPE`)
- [ ] No `struct` import for length-prefix framing in test_worker_main.py
- [ ] No `_make_frame()` or `_parse_frames()` helpers remain
- [ ] `ANVILML_WORKER_MOCK=1 pytest worker/tests/ -v` exits 0 with all 14 tests passing
- [ ] No source code files in `worker/` (ipc.py, worker_main.py) are modified
- [ ] No `Cargo.toml` version bumps required (no Rust source modified)
