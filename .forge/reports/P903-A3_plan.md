# Plan Report: P903-A3

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P903-A3                                     |
| Phase       | 903 — IPC Transport Rework                  |
| Description | worker: replace stdin/stdout IPC with socket in ipc.py |
| Depends on  | P903-A1, P903-A2                            |
| Project     | anvilml                                     |
| Planned at  | 2026-06-09T00:00:00Z                        |
| Attempt     | 1                                           |

## Objective

Replace the `sys.stdin.buffer`/`sys.stdout.buffer` IPC transport in the Python worker with a socket-based transport. The Rust supervisor already creates the socket and passes the path via `ANVILML_IPC_SOCKET` (P903-A2). This task updates `worker/ipc.py` to connect to that socket, removes the Windows `msvcrt.setmode` guard, updates `worker/worker_main.py` to call `ipc.connect()`, and updates `worker/tests/test_ipc.py` with a socketpair round-trip test.

## Scope

### In Scope
- `worker/ipc.py`: Remove `msvcrt.setmode` Windows binary-stdio guard entirely. Remove all references to `sys.stdin.buffer` and `sys.stdout.buffer`. Add module-level `_sock` global and `connect(path: str)` function. Implement `connect()` using `AF_UNIX` on Linux/macOS and `CreateFile` + `socket(fileno=...)` on Windows. Update `read_frame()` to read from `_sock.recv()` in a loop. Update `write_frame()` to send via `_sock.sendall()`.
- `worker/worker_main.py`: Call `ipc.connect(os.environ["ANVILML_IPC_SOCKET"])` at startup before the message loop. Replace `sys.stdout.buffer.flush()` on shutdown with `_sock.sendall(b"")` or equivalent socket flush.
- `worker/tests/test_ipc.py`: Remove `test_windows_binary_mode_guard_present` and `test_guard_code_exists_in_source`. Add `test_socketpair_roundtrip` using `socket.socketpair()`.

### Out of Scope
- Rust-side changes (`crates/anvilml-worker/src/managed.rs`) — handled by P903-A2.
- `worker/tests/test_worker_main.py` — the subprocess-based integration tests that use `stdin=subprocess.PIPE, stdout=subprocess.PIPE` are not in scope for this task. They will need separate work to use socket communication.
- Documentation updates (human-owned per TASKS_PHASE903.md).
- `interprocess` crate dependency — handled by P903-A2.

## Approach

### Step 1: Rewrite `worker/ipc.py`

1. **Remove** the module docstring referencing stdin/stdout; update to reference socket IPC.
2. **Remove** the `msvcrt.setmode` Windows binary-stdio guard block entirely (lines 13–24).
3. **Remove** `import io` (no longer needed — no `io.UnsupportedOperation` catch).
4. **Replace imports**: keep `struct`, `msgpack`. Add `import socket as _socket`, `import os`, `import sys`.
5. **Add module-level state**: `_sock: _socket.socket | None = None`.
6. **Add `connect(path: str) -> None`** function:
   - On `sys.platform == "win32"`: use `ctypes.windll.kernel32.CreateFileW` with `GENERIC_READ | GENERIC_WRITE`, `OPEN_EXISTING`, then wrap with `socket.socket(fileno=ctypes.windll.kernel32.get_osfhandle(handle))`.
   - On other platforms: `socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)`, then `connect(path)`.
   - The exact pattern is from TASKS_PHASE903.md §A3.
7. **Rewrite `read_frame()`**: Replace `sys.stdin.buffer.read()` calls with `_sock.recv()` in a loop. The logic is identical — read 4 bytes for header, then `length` bytes for payload. Raise `EOFError` when recv returns empty bytes.
8. **Rewrite `write_frame()`**: Replace `sys.stdout.buffer.write()` and `.flush()` with `_sock.sendall(header + payload)`. Socket `sendall` handles the flush implicitly.
9. **Logging note**: No logging changes required in Python IPC code (logging is Rust-side per §11.5).

### Step 2: Update `worker/worker_main.py`

1. **Add socket connection**: After `import worker.ipc as ipc` (line 155), add `ipc.connect(os.environ["ANVILML_IPC_SOCKET"])`.
2. **Fix shutdown flush**: Replace `sys.stdout.buffer.flush()` (line 205) with `_sock.sendall(b"")` or a no-op since `sendall` is synchronous. Actually, the simplest correct fix: remove the flush line entirely — `sendall` already blocks until data is sent, and the process exits immediately after. If a flush is needed, use a try/except around `_sock.sendall(b"")`.
3. **Update docstring**: Change "blocking stdin/stdout message loop" to "blocking socket message loop".

### Step 3: Update `worker/tests/test_ipc.py`

1. **Remove** the entire `TestWindowsGuard` class (lines 86–118) including `test_windows_binary_mode_guard_present` and `test_guard_code_exists_in_source`.
2. **Update** existing tests (`TestReadFrame`): The current tests mock `sys.stdin` and `sys.stdout` with `io.BytesIO` and `MagicMock`. After the change, `read_frame` and `write_frame` operate on `_sock`, not on `sys`. These tests will break.
   - **Fix**: The existing `TestReadFrame` tests need to be rewritten to use socketpair-based approach, similar to the new `test_socketpair_roundtrip`. Or, we can keep them but inject a mock `_sock` via monkeypatch on `ipc._sock`.
   - **Decision**: Rewrite all `TestReadFrame` tests to use `socket.socketpair()` and monkeypatch `ipc._sock`, consistent with the new architecture. The `io.BytesIO` + `sys.stdout`/`sys.stdin` mocking pattern is no longer valid.

### Step 4: Add `test_socketpair_roundtrip`

1. Create a `socket.socketpair()` (Unix domain socket pair, works on Linux/macOS).
2. Monkeypatch `ipc._sock` to one end of the pair.
3. Call `ipc.write_frame(payload)` with a test payload.
4. Read from the other end of the pair using the framing protocol (4-byte header + payload).
5. Assert the deserialized payload matches the original.
6. This test exercises the actual `read_frame`/`write_frame` code paths with real socket I/O.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `worker/ipc.py` | Replace stdin/stdout with socket transport; remove msvcrt guard; add `connect()` |
| Modify | `worker/worker_main.py` | Call `ipc.connect(os.environ["ANVILML_IPC_SOCKET"])` at startup; fix shutdown flush |
| Modify | `worker/tests/test_ipc.py` | Remove Windows guard tests; rewrite existing tests for socketpair; add `test_socketpair_roundtrip` |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `worker/tests/test_ipc.py` | `test_socketpair_roundtrip` (new) | `write_frame` + `read_frame` round-trip over a real socket pair |
| `worker/tests/test_ipc.py` | `test_write_read_roundtrip` (rewritten) | Dict payload round-trips correctly via socket |
| `worker/tests/test_ipc.py` | `test_roundtrip_with_bytes` (rewritten) | Raw bytes payload round-trips correctly via socket |
| `worker/tests/test_ipc.py` | `test_roundtrip_empty_dict` (rewritten) | Empty dict payload round-trips correctly via socket |

## CI Impact

The Python worker test gate (`ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/ -v`) runs for every commit. This task modifies files tested by that gate. No new CI jobs or steps are needed. The Rust-side tests in `crates/anvilml-worker/src/managed.rs` are gated behind `#[ignore = "requires P903-A3"]` and will remain ignored until the ACT session for P903-A3 enables them.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `test_worker_main.py` tests break because they use stdin/stdout pipes | High | Tests fail, acceptance criterion not met | Note that these tests use subprocess.Popen with stdin/stdout pipes. After the socket change, the worker reads from a socket, not stdin. These tests would need socket-based communication. If blocking, mark as risk and note in implementation report. |
| `socket.socketpair()` not available on all platforms | Low | Test fails on Windows | `socket.socketpair()` is available on Unix but not Windows. Use `AF_UNIX` socketpair on Unix; on Windows, use a temporary file-based Unix socket or skip the test with `@pytest.mark.skipif`. The actual `connect()` on Windows uses `CreateFile` which is tested by integration, not unit tests. |
| `ctypes.windll.kernel32` API shape mismatch | Low | Windows socket connect fails | The exact `ctypes` pattern is specified in TASKS_PHASE903.md §A3. Resolve at implementation time. Document any deviation in implementation report. |
| Existing `TestReadFrame` tests break due to `sys.stdin`/`sys.stdout` mocking | Certain | Tests fail if not rewritten | All existing tests must be rewritten to use socketpair + monkeypatch on `ipc._sock`. This is accounted for in Step 3 of the Approach. |

## Acceptance Criteria

- [ ] `worker/ipc.py` has no references to `sys.stdin.buffer`, `sys.stdout.buffer`, or `msvcrt.setmode`
- [ ] `worker/ipc.py` has `connect(path: str) -> None` function that opens AF_UNIX socket (Unix) or named pipe (Windows)
- [ ] `read_frame()` reads from `_sock.recv()` in a loop
- [ ] `write_frame()` sends via `_sock.sendall()`
- [ ] `worker/worker_main.py` calls `ipc.connect(os.environ["ANVILML_IPC_SOCKET"])` before the message loop
- [ ] `worker/tests/test_ipc.py` has no `TestWindowsGuard` class
- [ ] `worker/tests/test_ipc.py` has `test_socketpair_roundtrip` test
- [ ] `pytest worker/tests/ -v` exits 0 with all tests passing
