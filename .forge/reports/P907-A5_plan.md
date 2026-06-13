# Plan Report: P907-A5

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P907-A5                                           |
| Phase       | 907 — ZeroMQ IPC Transport                        |
| Description | worker/ipc.py: replace named-pipe/Unix-socket transport with ZeroMQ DEALER |
| Depends on  | P907-A3, P907-A4                                  |
| Project     | anvilml                                           |
| Planned at  | 2026-06-13T15:10:00Z                              |
| Attempt     | 1                                                 |

## Objective

Rewrite `worker/ipc.py` to replace the legacy transport layer (Unix domain sockets, Windows named pipes via ctypes, stdin/stdout fallback) with a ZeroMQ DEALER socket over TCP loopback. The Python worker will connect to `tcp://127.0.0.1:{port}` using `zmq.DEALER`, and the new `connect(port: int)` function will be called from `worker/worker_main.py` with the port value from `ANVILML_IPC_PORT`. All custom 4-byte length-prefix framing is removed — ZeroMQ's native message framing handles it.

## Scope

### In Scope
- **`worker/ipc.py`**: Complete rewrite of transport layer:
  - Remove `_WindowsPipeSocket` class (ctypes-based named pipe wrapper)
  - Remove `_StdioTransport` class (stdin/stdout fallback)
  - Remove `_get_transport()` lazy-resolution function
  - Remove `connect(path: str)` — replace with `connect(port: int)`
  - Remove `struct` import (no more length-prefix framing)
  - Remove `socket` import (no more raw socket I/O)
  - Remove `sys` import (no more stdin/stdout transport)
  - New `connect(port: int)`: creates global `zmq.Context.instance().socket(zmq.DEALER)`, connects to `tcp://127.0.0.1:{port}`
  - New `_sock` type: `zmq.Socket | None` instead of `socket.socket | None`
  - New `read_frame()`: `_sock.recv()` → `msgpack.unpackb()`
  - New `write_frame(data)`: `_sock.send(msgpack.packb(data, use_bin_type=True))`
  - Update module docstring to reflect ZeroMQ DEALER transport
- **`worker/worker_main.py`**: Update IPC connection at startup:
  - Replace `socket_path = os.environ.get("ANVILML_IPC_SOCKET")` with `port = int(os.environ["ANVILML_IPC_PORT"])`
  - Replace `ipc.connect(socket_path)` with `ipc.connect(int(os.environ["ANVILML_IPC_PORT"]))`
  - Remove the conditional `if socket_path is not None:` block — `ANVILML_IPC_PORT` is always set by Rust's `build_worker_env` (P907-A3)
  - Update docstring to remove stdin/stdout fallback references
- **`worker/tests/test_ipc.py`**: Rewrite tests to use ZeroMQ PAIR sockets:
  - Replace `socket.socketpair()` with in-process `zmq.PAIR` socket pairs
  - Remove all `struct.pack(">I", ...)` length-prefix framing from test helpers
  - All existing test assertions preserved (roundtrip data, bytes, empty dict, bidirectional, EOF)
- **`worker/requirements/base.txt`**: No change needed — `pyzmq>=26.0` already present

### Out of Scope
- **`worker/tests/test_worker_main.py`**: Subprocess integration tests that use `stdin=PIPE`/`stdout=PIPE` — handled by P907-A6
- **`crates/anvilml-worker/src/managed.rs`**: Rust-side ZeroMQ changes — handled by P907-A4
- **`crates/anvilml-worker/src/env.rs`**: `ANVILML_IPC_PORT` env var injection — handled by P907-A3
- **`anvilml-ipc` crate**: `framing.rs` removal — handled by P907-A8

## Approach

1. **Verify dependency**: Confirm `pyzmq>=26.0` is compatible with Python 3.12. MCP lookup confirms: `pyzmq 27.1.0` supports `>=3.8`, already listed in `base.txt`.

2. **Rewrite `worker/ipc.py`**:
   - Replace imports: remove `io`, `socket`, `struct`, `sys`; keep `os` (for env var access if needed), add `zmq`
   - Change `_sock` type annotation from `socket.socket | None` to `zmq.Socket | None`
   - Rewrite `connect(port: int) -> None`:
     ```python
     def connect(port: int) -> None:
         global _sock
         ctx = zmq.Context.instance()
         _sock = ctx.socket(zmq.DEALER)
         _sock.connect(f"tcp://127.0.0.1:{port}")
     ```
   - Rewrite `read_frame() -> object`:
     ```python
     def read_frame() -> object:
         if _sock is None:
             raise RuntimeError("ipc: not connected — call connect(port) first")
         data = _sock.recv()
         return msgpack.unpackb(data, raw=False)
     ```
   - Rewrite `write_frame(data: object) -> None`:
     ```python
     def write_frame(data: object) -> None:
         if _sock is None:
             raise RuntimeError("ipc: not connected — call connect(port) first")
         _sock.send(msgpack.packb(data, use_bin_type=True))
     ```
   - Remove `_get_transport()`, `_StdioTransport`, `_WindowsPipeSocket` entirely
   - Update module docstring

3. **Update `worker/worker_main.py`**:
   - In `main()`, replace lines 203-207:
     ```python
     # Old:
     socket_path = os.environ.get("ANVILML_IPC_SOCKET")
     if socket_path is not None:
         ipc.connect(socket_path)
     
     # New:
     ipc.connect(int(os.environ["ANVILML_IPC_PORT"]))
     ```
   - Update docstring to remove stdin/stdout fallback references (lines 2-3, 203-204 comment)

4. **Rewrite `worker/tests/test_ipc.py`**:
   - Replace `import socket` + `import struct` with `import zmq`
   - New test helper `_zmq_pair()` that creates an in-process PAIR socket pair:
     ```python
     def _zmq_pair():
         ctx = zmq.Context()
         a = ctx.socket(zmq.PAIR)
         b = ctx.socket(zmq.PAIR)
         a.bind("tcp://127.0.0.1:0")
         addr = a.getsockopt(zmq.LAST_ENDPOINT).decode()
         b.connect(addr)
         return a, b, ctx
     ```
   - Rewrite `_monkeypatch_sock` to accept a zmq socket and patch `ipc._sock`
   - Update `TestReadFrame` tests: replace `sock_b.sendall(length_prefix + payload)` with `sock_b.send(payload)` — PAIR sockets deliver the exact bytes sent
   - Update `TestSocketRoundtrip` tests similarly — no length prefix needed
   - Update `test_read_frame_eof`: close one side of PAIR pair, verify `zmq.Again` or `Error` → wrap in `pytest.raises`

5. **Verify tests pass**: Run `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_ipc.py -v` — must exit 0.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `worker/ipc.py` | Complete transport rewrite: remove Unix socket, named pipe, stdio; add ZeroMQ DEALER |
| Modify | `worker/worker_main.py` | Update `main()` to call `ipc.connect(int(os.environ["ANVILML_IPC_PORT"]))`; update docstring |
| Modify | `worker/tests/test_ipc.py` | Rewrite tests: replace socketpair+length-prefix with zmq.PAIR roundtrips |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `worker/tests/test_ipc.py` | `TestReadFrame.test_write_read_roundtrip` | msgpack dict roundtrip through ZMQ DEALER socket |
| `worker/tests/test_ipc.py` | `TestReadFrame.test_roundtrip_with_bytes` | Raw bytes payload roundtrip |
| `worker/tests/test_ipc.py` | `TestReadFrame.test_roundtrip_empty_dict` | Empty dict roundtrip |
| `worker/tests/test_ipc.py` | `TestSocketRoundtrip.test_socketpair_roundtrip` | Full `write_frame` + `read_frame` roundtrip over PAIR socket |
| `worker/tests/test_ipc.py` | `TestSocketRoundtrip.test_full_bidirectional_roundtrip` | Server→worker and worker→server bidirectional messaging |
| `worker/tests/test_ipc.py` | `TestSocketRoundtrip.test_read_frame_eof` | `read_frame` raises on closed peer |

## CI Impact

No CI workflow file changes required. The Python worker test gate (`ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/ -v`) will continue to run. The `pyzmq>=26.0` dependency is already in `worker/requirements/base.txt`. The subprocess-based integration tests in `test_worker_main.py` are not covered by this task (handled by P907-A6), so they may temporarily not pass — but the gate command includes all tests in `worker/tests/`, so P907-A6 must follow immediately after.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `zmq.Context.instance()` singleton shared across test functions causing stale sockets | Medium | High | Each test creates its own `zmq.Context()` and cleans it up in `finally`; do NOT use the global singleton in tests |
| `msgpack.unpackb` on raw `bytes` from `_sock.recv()` fails if ZMQ sends an empty frame | Low | Medium | ZeroMQ DEALER only delivers complete messages; add a check for `data == b""` → raise `EOFError` to match existing behavior |
| PAIR socket `getsockopt(LAST_ENDPOINT)` returns bytes vs str | Low | Low | Decode with `.decode()` consistently; test with `assert isinstance(addr, str)` |
| `worker_main.py` tests still use stdin/stdout pipes | N/A | N/A | Out of scope — handled by P907-A6; these tests will be updated in the next task |
| `pyzmq` not installed in test environment | Low | Medium | Already in `base.txt` as `pyzmq>=26.0`; verify installation before running tests |

## Acceptance Criteria

- [ ] `worker/ipc.py` has no references to `socket`, `struct`, `CreateFileW`, `_WindowsPipeSocket`, `_StdioTransport`, or `ANVILML_IPC_SOCKET`
- [ ] `worker/ipc.py` `connect()` signature is `connect(port: int) -> None` using `zmq.DEALER`
- [ ] `worker/ipc.py` `read_frame()` uses `_sock.recv()` + `msgpack.unpackb(data, raw=False)`
- [ ] `worker/ipc.py` `write_frame()` uses `_sock.send(msgpack.packb(data, use_bin_type=True))`
- [ ] `worker/worker_main.py` calls `ipc.connect(int(os.environ["ANVILML_IPC_PORT"]))` at startup
- [ ] `worker/worker_main.py` has no references to `ANVILML_IPC_SOCKET`
- [ ] `worker/tests/test_ipc.py` uses `zmq.PAIR` sockets for all tests (no `socket.socketpair`)
- [ ] `worker/tests/test_ipc.py` has no `struct.pack(">I", ...)` length-prefix framing
- [ ] `pytest worker/tests/test_ipc.py -v` exits 0 with all 6 tests passing
