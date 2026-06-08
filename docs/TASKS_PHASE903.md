# Tasks: Phase 903 — IPC Transport Rework

| Field | Value |
|-------|-------|
| Phase | 903 |
| Name | IPC Transport Rework |
| Milestone group | Retrofit |
| Project(s) | anvilml |
| Status | Draft |
| Depends on phases | 0–12 (via P902-D1) |
| Task file | `.forge/tasks/tasks_phase903.json` |
| Tasks | 7 |

---

## Overview

Phase 903 replaces the stdin/stdout IPC transport between the Rust supervisor and Python
workers with Unix domain sockets (Linux/macOS) and Windows named pipes, unified through
the `interprocess` crate on the Rust side and the standard `socket` module on the Python
side.

**Why this is a retrofit phase and not a later primary phase:**

The stdin/stdout transport has two structural defects that cannot be patched:

1. **Stdout pollution.** Any Python library that writes to stdout — PyTorch model loading
   messages, HuggingFace hub download progress, tqdm bars, arbitrary `print()` calls — injects
   raw bytes into the IPC framing stream. A stray line break desynchronises the length-prefix
   framer; every subsequent read is garbage. This is not a hypothetical: PyTorch and HuggingFace
   print to stdout during model load by default. The defect is structural because stdout is
   shared between the IPC transport and the Python runtime's output stream.

2. **No path to real-time latent previews.** When the product reaches per-step diffusion
   previews (planned for a post-MVP phase), preview frames (~768 KB each at 512×512 decoded
   RGB, emitted every 1–3 seconds during a run) must flow from worker to supervisor. Routing
   them through a shared-memory ring buffer — the correct approach — requires a dedicated
   control channel for frame-ready notifications. That dedicated channel cannot be stdin/stdout.
   A socket provides it without additional complexity.

Switching transport at Phase 25 would require touching `managed.rs`, `env.rs`, `ipc.py`,
`worker_main.py`, all test fixtures, and every document that describes the IPC protocol.
The cost increases proportionally with the number of phases built on top of it. Phase 903
makes the change before Phase 13 (dispatch loop) and Phase 14 (artifact pipeline) build on
top of the worker IPC path.

**What does not change:**

- The framing protocol: `[ 4 bytes big-endian u32: payload_len ] [ N bytes: msgpack ]`
- The message schema: all `WorkerMessage` and `WorkerEvent` variants are unchanged
- The `anvilml-ipc` crate public API: `write_frame` and `read_frame` accept `AsyncWrite`
  and `AsyncRead` respectively — transport is already abstracted at this boundary
- The `WorkerPool`, keepalive, respawn, and event listener logic in `pool.rs`
- The scheduler, server, and all phases above the worker layer

**Design decision: supervisor-creates lifecycle**

The Rust supervisor creates the socket before spawning the Python process and passes the
path via the `ANVILML_IPC_SOCKET` environment variable. The worker connects on startup.
This matches the existing ownership model (supervisor owns the pipe lifecycle) and is
simpler than worker-creates because no additional startup signal is needed.

---

## Socket Path Convention

**Linux / macOS:**
```
{std::env::temp_dir()}/anvilml-{supervisor_pid}/worker-{device_index}.sock
```
Example: `/tmp/anvilml-12345/worker-0.sock`

The parent directory is created by the supervisor before spawning the worker. It is removed
on clean shutdown. On unclean restart the directory is recreated (overwriting any stale
socket). Abstract namespace sockets (`\0anvilml-...`) are not used — the file-based path is
visible in `ls /tmp` for debugging.

**Windows:**
```
\\.\pipe\anvilml-worker-{device_index}-{supervisor_pid}
```
Example: `\\.\pipe\anvilml-worker-0-12345`

Named pipes on Windows do not require directory creation. The `interprocess` crate exposes
`LocalSocketListener::bind` with a `NameTypeSupport`-aware name that maps to the correct
path format per platform automatically when using the `to_ns_name::<GenericNamespaced>()`
helper. The agent must resolve the exact `interprocess` API at implementation time using
`mcp-rust-docs`.

---

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | anvilml-worker / worker | P903-A1, A2, A3, A3x, A4, A5 | env var injection; Rust transport replacement; Python transport replacement; Windows name-type fix; ipc-probe gate; ignored test reactivation |
| C | Gate | P903-C1 | Full workspace clean gate |

---

## Prerequisites

All tasks in phases 000 through 912 must be complete (`P902-D1` is the immediate
predecessor). `tasks_phase013.json` must have `P13-A1.prereqs` updated to `["P903-C1"]`
before The Forge starts this phase.

---

## Interfaces and Contracts

| Contract document | Relevant to tasks | What must match |
|---|---|---|
| `docs/ANVILML_DESIGN.md §7.1–7.3` | P903-A2, P903-A3 | Framing format unchanged; message schema unchanged |
| `docs/ENVIRONMENT.md §3.7` | P903-A1 | ANVILML_IPC_SOCKET per-worker variable |
| `crates/anvilml-ipc/src/framing.rs` | P903-A2, P903-A4 | write_frame/read_frame accept AsyncWrite/AsyncRead — no changes required |

---

## Task Descriptions

### Group A — anvilml-worker / worker

#### P903-A1: Add ANVILML_IPC_SOCKET to build_worker_env

**File:** `crates/anvilml-worker/src/env.rs`

Add `ipc_socket_path: &str` as the third parameter to `build_worker_env(device, cfg,
ipc_socket_path)`. Insert `ANVILML_IPC_SOCKET` into the returned `HashMap` with the
provided value. Update the single call site in `managed.rs` to pass an empty string
placeholder (`""`); the real path is wired in P903-A2.

Update all tests in `env.rs` that call `build_worker_env` to pass `""` as the new
third argument. Add one new test asserting that a non-empty `ipc_socket_path` appears
in the returned map under `ANVILML_IPC_SOCKET`.

**Acceptance criterion:** `cargo test -p anvilml-worker --features mock-hardware` exits 0.

---

#### P903-A2: Replace stdin/stdout IPC transport with Unix socket / Windows named pipe (Rust)

**File:** `crates/anvilml-worker/src/managed.rs`, `crates/anvilml-worker/Cargo.toml`

**Cargo.toml:** Add `interprocess` as a workspace-resolved dependency. Resolve the current
version using `mcp-rust-docs` before writing any version string.

**`IpcHandles` struct:** Replace `stdin: tokio::process::ChildStdin` and
`stdout: tokio::process::ChildStdout` with `reader: OwnedReadHalf` and
`writer: OwnedWriteHalf` from `interprocess::local_socket::tokio`.

**`ManagedWorker` struct:** Add `ipc_socket_path: String` field (the path used for the
socket this worker listens on; stored for logging and cleanup).

**`spawn()` method changes:**

1. Build the socket path using the convention from this document's Socket Path Convention
   section. Use `std::process::id()` for the supervisor PID.
2. Create the parent directory with `tokio::fs::create_dir_all`.
3. Bind a `LocalSocketListener` on the path.
4. Pass the socket path to `build_worker_env` (replacing the `""` placeholder from A1).
5. Set `Command` stdin to `Stdio::null()`. Stdout is no longer used for IPC; remove the
   `.stdout(Stdio::piped())` line. Keep stderr piped for the drain task.
6. Spawn the child process.
7. `listener.accept().await` — this blocks until the Python worker connects. Wrap in a
   10-second timeout consistent with the existing `InitializeHardware` handshake timeout.
8. Split the accepted stream: `stream.into_split()` → `(reader, writer)`.
9. Deliver `IpcHandles { reader, writer }` via `ipc_tx`.

**`writer_task` signature:** Change `mut stdin: tokio::process::ChildStdin` to
`mut writer: OwnedWriteHalf`. The body is unchanged — `framing::write_frame` accepts
any `AsyncWrite`.

**`reader_task` signature:** Change `mut stdout: tokio::process::ChildStdout` to
`mut reader: OwnedReadHalf`. The body is unchanged — `framing::read_frame` accepts
any `AsyncRead`.

**Cleanup:** On clean shutdown (Shutdown message processed, child exits), remove the
socket file and parent directory if empty. Use a best-effort `tokio::fs::remove_file`
in the writer task exit path; do not fail if the file is already gone.

**No changes to:** `pool.rs`, `env.rs` (beyond the call site update from A1), framing,
message types, keepalive, respawn, or any other file.

**Acceptance criterion:** `cargo clippy -p anvilml-worker --features mock-hardware -- -D warnings`
exits 0. `cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu`
exits 0.

---

#### P903-A3: Replace stdin/stdout IPC transport with socket in Python worker

**Files:** `worker/ipc.py`, `worker/tests/test_ipc.py`

**`worker/ipc.py`:**

Remove the `msvcrt.setmode` Windows binary-stdio guard entirely. Remove all references
to `sys.stdin.buffer` and `sys.stdout.buffer`.

Add a module-level `_sock: socket.socket | None = None` and a `connect(path: str) -> None`
function that opens the socket:

```python
import socket as _socket
import os
import sys

_sock: _socket.socket | None = None

def connect(path: str) -> None:
    """Connect to the supervisor IPC socket.

    On Linux/macOS opens an AF_UNIX socket.
    On Windows opens a named pipe via CreateFile and wraps it as a socket.
    Must be called once at worker startup before any read_frame/write_frame calls.
    """
    global _sock
    if sys.platform == "win32":
        import ctypes
        GENERIC_READ  = 0x80000000
        GENERIC_WRITE = 0x40000000
        OPEN_EXISTING = 3
        handle = ctypes.windll.kernel32.CreateFileW(
            path, GENERIC_READ | GENERIC_WRITE, 0, None,
            OPEN_EXISTING, 0, None
        )
        if handle == -1:
            raise OSError(f"CreateFile failed for {path}")
        _sock = _socket.socket(fileno=ctypes.windll.kernel32.get_osfhandle(handle))
    else:
        _sock = _socket.socket(_socket.AF_UNIX, _socket.SOCK_STREAM)
        _sock.connect(path)
```

Update `read_frame()` to read from `_sock` using `_sock.recv` in a loop (equivalent to
the current `sys.stdin.buffer.read` loop). Update `write_frame()` to send via
`_sock.sendall`.

**`worker/worker_main.py`:** Call `ipc.connect(os.environ["ANVILML_IPC_SOCKET"])` at
startup, before the message loop. This replaces the implicit stdin/stdout connection.

**`worker/tests/test_ipc.py`:**

Remove `test_windows_binary_mode_guard_present` and `test_guard_code_exists_in_source`.

Add `test_socketpair_roundtrip`: create a `socket.socketpair()`, call `ipc.connect()`
with one end injected via monkeypatch on `ipc._sock`, write a frame via `write_frame`,
read it back via `read_frame` on the other end, assert the payload round-trips correctly.

**Acceptance criterion:** `pytest worker/tests/ -v` exits 0 with all tests passing.

---

#### P903-A3x: Fix GenericFilePath/GenericNamespaced Windows name-type error

**File:** `crates/anvilml-worker/src/managed.rs`

**Prereqs:** P903-A3

**Root cause.** `build_socket_path()` returns a Windows named pipe path
(`\\.\pipe\anvilml-worker-...`) on Windows. The production `spawn()` method and the
`respawn_after_death` test both call `.to_fs_name::<GenericFilePath>()` on this path.
`GenericFilePath` maps to Unix file-system socket paths; it explicitly rejects
`\\.\pipe\...` paths, producing `"not a named pipe path"`. The correct name type for
Windows named pipes is `GenericNamespaced`.

**Fix.** Introduce a private `to_socket_name` helper that is cfg-gated:

```rust
#[cfg(unix)]
fn to_socket_name(
    path: &std::path::Path,
) -> std::io::Result<interprocess::local_socket::Name<'_>> {
    use interprocess::local_socket::traits::ToFsName;
    use interprocess::local_socket::GenericFilePath;
    path.to_fs_name::<GenericFilePath>()
}

#[cfg(windows)]
fn to_socket_name(
    path: &std::path::Path,
) -> std::io::Result<interprocess::local_socket::Name<'_>> {
    use interprocess::local_socket::traits::ToNsName;
    use interprocess::local_socket::GenericNamespaced;
    path.to_ns_name::<GenericNamespaced>()
}
```

Replace every `path.to_fs_name::<GenericFilePath>()` call that operates on a
`build_socket_path()` result with `to_socket_name(&path)?`. Affected sites:

- `spawn()` — the `listener` bind call
- `respawn_after_death` test — `socket_path` bind and connect, `socket_path2` bind and connect

No changes to any other logic, fields, or files.

**Acceptance criterion:** `cargo test -p anvilml-worker --features mock-hardware` exits 0
on Linux. `cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu`
exits 0.

---

#### P903-A4: Verify ipc-probe after transport change

**File:** `crates/anvilml-ipc/src/bin/ipc-probe.rs`

The `ipc-probe` binary uses `tokio::io::duplex` for an in-process round-trip test. The
framing layer is transport-agnostic; no changes are required by the transport switch.

If P902-A1 has not yet been applied (probe still hand-rolls `rmp_serde` instead of calling
`write_frame`), apply that fix now: replace the manual serialization with
`write_frame(&mut tx, &WorkerMessage::Ping { seq: 7 }).await?`.

**Acceptance criterion:** `cargo run -p anvilml-ipc --bin ipc-probe` prints `OK seq=7`
and exits 0.

---

#### P903-A5: Reactivate four ignored integration tests

**File:** `crates/anvilml-worker/src/managed.rs`

Remove the `#[ignore]` attribute from the following four tests:

- `spawn_ping_pong`
- `status_transitions`
- `handshake_completes_once`
- `spawn_reaches_idle`

These tests were ignored during P903-A2 pending the Python worker socket implementation
in P903-A3. With P903-A3 complete the workers can connect to the socket, and all four
tests must pass without modification to their assertions.

If any test has a doc comment referencing the P903-A3 pending state (e.g. `/// Ignored
until P903-A3 updates the Python worker to connect to the socket.`), remove or update
that comment to reflect that the test is now active.

No changes to test logic, assertions, or any other file.

**Acceptance criterion:** `cargo test -p anvilml-worker --features mock-hardware` exits 0
with all four tests listed as `ok` (not `ignored`).

---

## Manual Documentation Updates (human-owned, not agent tasks)

The following documents must be updated by hand after P903-C1 is green and before Phase 13
begins. They are human-authored and must not be modified by agents under any circumstances.

### `docs/ANVILML_DESIGN.md`

**§7 opening paragraph:** Replace *"Communication uses the worker's stdin/stdout pipes."*
with *"Communication uses a Unix domain socket (Linux/macOS) or Windows named pipe. The
supervisor creates the socket before spawning the worker and passes its path via the
`ANVILML_IPC_SOCKET` environment variable. The worker connects on startup."*

**§7.1 Framing:** Remove the Windows binary-stdio requirement block (the `msvcrt.setmode`
code block and surrounding paragraph). Update the opening sentence from
*"Because frames are raw binary msgpack carried over stdout/stdin…"* to
*"Frames are raw binary msgpack carried over the IPC socket…"*
The framing format itself — 4-byte big-endian length prefix + msgpack — is unchanged.

**§1 platform table IPC row:** Change *"IPC over stdio"* to
*"Unix domain socket (Linux/macOS) / Windows named pipe — path via `ANVILML_IPC_SOCKET`"*.

### `docs/ARCHITECTURE.md`

**§4 anvilml-worker row:** Change `managed.rs` description from
*"spawn + stdin/stdout IPC bridge"* to *"spawn + Unix socket/named pipe IPC bridge"*.

**§6 IPC Protocol Summary, transport line:** Change
*"child process stdin/stdout pipes (not TCP/UDS)"* to
*"Unix domain socket (Linux/macOS) / Windows named pipe; supervisor creates before spawn,
worker connects via `ANVILML_IPC_SOCKET`"*.
Remove the Windows binary-stdio requirement note from §6.

### `docs/ENVIRONMENT.md`

**§3.7 per-worker variables table:** Add row:

| `ANVILML_IPC_SOCKET` | Unix socket path (Linux/macOS) or Windows named pipe path. Injected by `build_worker_env`. Worker must connect to this path at startup before processing any IPC frames. |

Remove any row or note referencing `msvcrt.setmode`, stdout binary mode, or
`sys.stdin.buffer`/`sys.stdout.buffer` as the IPC transport.

---

### Group C — Gate

#### P903-C1: Full workspace clean gate after IPC transport rework

**No files modified.**

Run and record verbatim output:

```bash
# 1. Lint
cargo clippy --workspace --features mock-hardware -- -D warnings

# 2. Tests — ambient env cleared
env -i HOME=$HOME PATH=$PATH ANVILML_WORKER_MOCK=1 ANVILML_VENV_PATH=./worker/.venv \
  cargo test --workspace --features mock-hardware

# 3. Windows cross-check
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu

# 4. Python worker
python -m pytest worker/tests/ -v
```

All four must exit 0. Write verbatim outputs as the implementation report body. Task is
COMPLETE only when all four exit 0.

---

## Phase Acceptance Criteria

```bash
cargo clippy --workspace --features mock-hardware -- -D warnings
env -i HOME=$HOME PATH=$PATH ANVILML_WORKER_MOCK=1 ANVILML_VENV_PATH=./worker/.venv \
  cargo test --workspace --features mock-hardware
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
python -m pytest worker/tests/ -v
```

All four must exit 0.

---

## Known Constraints and Gotchas

- **`interprocess` version resolution is mandatory.** The agent must query `mcp-rust-docs`
  for the current `interprocess` version before writing the `Cargo.toml` entry. Do not
  copy a version from memory. The crate has had breaking API changes between 1.x and 2.x;
  the 2.x async API uses `LocalSocketListener::bind` with a typed name — verify the exact
  call signature before writing code.

- **P903-A2: accept timeout.** The `listener.accept().await` call in `spawn()` must be
  wrapped in a `tokio::time::timeout` matching the existing handshake timeout (10 seconds
  default). If the worker process fails to connect within that window the spawn must return
  `Err(AnvilError::Io(...))` and the listener socket must be cleaned up.

- **P903-A2: socket cleanup on Windows.** Named pipes on Windows are cleaned up
  automatically by the OS when all handles are closed. `tokio::fs::remove_file` must be
  guarded with `#[cfg(unix)]` — it must not be called on Windows paths.

- **P903-A2: `Stdio::null()` for stdin.** The Python worker previously used
  `sys.stdin.buffer` for IPC reads. After this change stdin is `Stdio::null()`. Any
  Python code that reads from `sys.stdin` directly (outside `ipc.py`) will block or error.
  `worker_main.py` must not read from stdin after this change.

- **P903-A3: Windows named pipe via CreateFile.** The `ctypes` approach shown in the
  task description is one option. An alternative is `open(path, 'r+b', buffering=0)` on
  Windows named pipes if the Python version supports it. Either is acceptable; document
  the choice in the implementation report. The critical requirement is that reads and
  writes are binary and unbuffered.

- **P903-A3: `_sock` module global.** The `connect()` / module-global pattern is chosen
  for minimal diff from the existing `read_frame`/`write_frame` API. `worker_main.py`
  calls `connect()` once; all subsequent `read_frame`/`write_frame` calls use the
  established connection transparently. Do not refactor `read_frame`/`write_frame` to
  accept a socket argument — that is a larger API change than this phase authorises.

- **P903-A4: P902-A1 dependency.** If P902-A1 (ipc-probe fix) has already been applied,
  P903-A4 is a pure verification task with no code changes. If it has not been applied,
  the ipc-probe fix is in scope for P903-A4. The agent must check the current state of
  `ipc-probe.rs` before writing any code.

- **P13-A1 prereq must be updated manually** from `["P902-D1"]` to `["P903-C1"]` in
  `tasks_phase013.json` before The Forge runs Phase 903. P903-C1 now prereqs both
  P903-A4 and P903-A5, so the gate does not close until all four ignored tests are
  reactivated and passing.

- **No shared memory data channel in this phase.** The socket transport is the control
  channel. A separate shared memory channel for high-bandwidth data (latent preview frames)
  is deferred to the phase that implements real-time previews. Do not attempt to implement
  it here.