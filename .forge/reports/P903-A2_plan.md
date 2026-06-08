# Plan Report: P903-A2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P903-A2                                     |
| Phase       | 903 — IPC Transport Rework                  |
| Description | Replace stdin/stdout IPC transport with Unix socket / Windows named pipe (Rust) |
| Depends on  | P903-A1                                     |
| Project     | anvilml                                     |
| Planned at  | 2026-06-08T22:15:00Z                        |
| Attempt     | 1                                           |

## Objective

Replace the `tokio::process::ChildStdin` / `tokio::process::ChildStdout` IPC transport in `crates/anvilml-worker/src/managed.rs` with `interprocess` local socket streams (`OwnedReadHalf` / `OwnedWriteHalf`), enabling the Rust supervisor to create a Unix domain socket (Linux/macOS) or Windows named pipe before spawning the Python worker, accept the worker's connection, and deliver split read/write halves to the writer/reader tasks.

## Scope

### In Scope
- **`Cargo.toml`**: Add `interprocess` dependency with `tokio` feature; bump `anvilml-worker` patch version `0.1.15 → 0.1.16`.
- **`managed.rs` — `IpcHandles` struct**: Replace `stdin: tokio::process::ChildStdin` / `stdout: tokio::process::ChildStdout` with `reader: OwnedReadHalf` / `writer: OwnedWriteHalf` from `interprocess::local_socket::tokio`.
- **`managed.rs` — `ManagedWorker` struct**: Add `ipc_socket_path: String` field.
- **`managed.rs` — `spawn()` method**:
  1. Build socket path using convention from TASKS_PHASE903.md §Socket Path Convention.
  2. Create parent directory (`tokio::fs::create_dir_all`).
  3. Bind `LocalSocketListener` on the path.
  4. Pass socket path to `build_worker_env` (replacing `""` placeholder from P903-A1).
  5. Set `Command` stdin to `Stdio::null()`; remove `.stdout(Stdio::piped())`; keep stderr piped.
  6. Spawn child process.
  7. `listener.accept().await` wrapped in `tokio::time::timeout(10s)`.
  8. Split accepted stream via `into_split()` → `(reader, writer)`.
  9. Deliver `IpcHandles { reader, writer }` via `ipc_tx`.
- **`managed.rs` — `writer_task`**: Change parameter from `mut stdin: tokio::process::ChildStdin` to `mut writer: OwnedWriteHalf`. Body unchanged (framing accepts any `AsyncWrite`).
- **`managed.rs` — `reader_task`**: Change parameter from `mut stdout: tokio::process::ChildStdout` to `mut reader: OwnedReadHalf`. Body unchanged (framing accepts any `AsyncRead`).
- **`managed.rs` — `run_loop`**: Destructure `IpcHandles { reader, writer }` instead of `{ stdin, stdout }`.
- **`managed.rs` — cleanup**: On writer task exit, best-effort `tokio::fs::remove_file` for socket path (guarded with `#[cfg(unix)]`; Windows named pipes are auto-cleaned by OS).
- **`managed.rs` — `inject_handles_for_test()`**: Update signature to accept mock-compatible halves.
- **Logging**: Add INFO log for socket bind, DEBUG logs for accept timeout and connection acceptance, INFO log for socket cleanup.

### Out of Scope
- `pool.rs` — no changes (WorkerPool logic unchanged).
- `env.rs` — already has `ipc_socket_path` parameter from P903-A1.
- `framing.rs` — transport-agnostic; no changes.
- `anvilml-ipc` crate — no changes.
- Python worker (`worker/ipc.py`, `worker/worker_main.py`) — covered by P903-A3.
- Python tests (`worker/tests/test_ipc.py`) — covered by P903-A3.
- `ipc-probe` — covered by P903-A4.
- Documentation updates — human-owned per TASKS_PHASE903.md.

## Approach

1. **Resolve `interprocess` version** via `rust-docs-lookup-crate-docs`. Confirmed: version **2.4.2** with `tokio` feature. The `tokio` feature enables async `LocalSocketListener`, `LocalSocketStream`, `OwnedReadHalf`, and `OwnedWriteHalf` in `interprocess::local_socket::tokio`.

2. **Add dependency to `Cargo.toml`**:
   ```toml
   interprocess = { version = "2.4", features = ["tokio"] }
   ```
   Using `"2.4"` as the semver-compatible range (allows 2.4.x patch updates).

3. **Bump `anvilml-worker` version** from `0.1.15` to `0.1.16` in `[package]` section.

4. **Refactor `IpcHandles` struct** (line 25-28 of managed.rs):
   ```rust
   struct IpcHandles {
       reader: interprocess::local_socket::tokio::OwnedReadHalf,
       writer: interprocess::local_socket::tokio::OwnedWriteHalf,
   }
   ```

5. **Add `ipc_socket_path` to `ManagedWorker`** (line 34-67). Store the socket path for logging and cleanup.

6. **Rewrite `spawn()` method** (line 137-275):
   - Build socket path: `std::env::temp_dir().join(format!("anvilml-{}" / "\\.\pipe\\anvilml-worker-{}-{}", pid, device_index))` — use `cfg!(windows)` for path convention.
   - Create parent directory for Unix: `tokio::fs::create_dir_all(&dir).await`.
   - Bind listener: `let listener = LocalSocketListener::bind(&socket_path)?`.
   - Log bind: `info!(socket_path = %socket_path, "bound IPC socket")`.
   - Set `Command` stdin to `Stdio::null()`, remove `.stdout(Stdio::piped())`, keep stderr piped.
   - Pass `&socket_path` to `build_worker_env` (replacing `""`).
   - Spawn child, log spawn.
   - Accept with 10s timeout: `tokio::time::timeout(Duration::from_secs(10), listener.accept()).await`.
   - On timeout → `Err(AnvilError::Io(...))` with cleanup.
   - On success: `let (reader, writer) = stream.into_split()`.
   - Deliver via `ipc_tx`: `tx.send(IpcHandles { reader, writer })`.
   - Remove old stdin/stdout handle extraction code (lines 191-201).

7. **Update `run_loop`** (line 564-592): Destructure `IpcHandles { reader, writer }` and pass to `writer_task(writer, ...)` / `reader_task(reader, ...)`.

8. **Update `writer_task`** (line 596-623): Change parameter from `mut stdin: tokio::process::ChildStdin` to `mut writer: OwnedWriteHalf`. Update all `stdin` references to `writer`. Add cleanup log on exit.

9. **Update `reader_task`** (line 626-668): Change parameter from `mut stdout: tokio::process::ChildStdout` to `mut reader: OwnedReadHalf`. Update all `stdout` references to `reader`.

10. **Add socket cleanup in `writer_task`**: Before the final `debug!` log, add best-effort removal:
    ```rust
    #[cfg(unix)]
    {
        let _ = tokio::fs::remove_file(&socket_path).await;
    }
    ```
    (Pass `socket_path` from `ManagedWorker` into the task.)

11. **Update `inject_handles_for_test()`** (line 477-493): Change signature to accept `OwnedReadHalf` / `OwnedWriteHalf` and construct `IpcHandles` with them.

12. **Update `reset_ipc_tx()`** (line 495-525): No structural change needed — the oneshot channel type changes automatically with `IpcHandles`.

13. **Update tests** in `managed.rs`: The `spawn_ping_pong`, `status_transitions`, `handshake_completes_once`, and `spawn_reaches_idle` tests call `spawn()` which now creates a real socket. These tests will probably fail until the implementation of P903-A3 where the python worker is adjusted to connect to the socket, which for this task is the acceptable result for the tests. The `eof_sets_dead` test uses `tokio::io::duplex` and the framing layer — no change needed since `framing::read_frame` accepts any `AsyncRead`. The `keepalive_pings_and_kills_on_timeout` test uses a dummy child — no IPC handles are injected. The `respawn_after_death` test uses `inject_handles_for_test()` with `cat` child process — needs updating to construct mock halves.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-worker/Cargo.toml` | Add `interprocess` dependency; bump version `0.1.15 → 0.1.16` |
| Modify | `crates/anvilml-worker/src/managed.rs` | Replace stdin/stdout IPC with local socket streams; add socket path field; refactor spawn/run_loop/writer_task/reader_task; add cleanup; update tests |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `crates/anvilml-worker/src/managed.rs` (inline tests) | `spawn_ping_pong` | Socket-based spawn → Ping → Pong → Shutdown lifecycle |
| `crates/anvilml-worker/src/managed.rs` (inline tests) | `status_transitions` | Initializing → Idle → Dead via socket IPC |
| `crates/anvilml-worker/src/managed.rs` (inline tests) | `handshake_completes_once` | Exactly one Ready event during spawn handshake |
| `crates/anvilml-worker/src/managed.rs` (inline tests) | `eof_sets_dead` | Framing read_frame with AsyncRead (duplex pipe) — no change needed |
| `crates/anvilml-worker/src/managed.rs` (inline tests) | `keepalive_pings_and_kills_on_timeout` | Keepalive watchdog with dummy child — no IPC handles needed |
| `crates/anvilml-worker/src/managed.rs` (inline tests) | `respawn_after_death` | Death detection + respawn via mock handles — needs `inject_handles_for_test` update |
| `crates/anvilml-worker/src/managed.rs` (inline tests) | `spawn_reaches_idle` | Spawn reaches Idle without timing workarounds |

## CI Impact

No CI workflow files are modified. The existing CI gates (clippy, tests, cross-check) apply to the modified crate. The `mock-hardware` feature is forwarded from `anvilml-worker` to `anvilml-hardware` (already configured in Cargo.toml), so CI builds with `--features mock-hardware` will exercise the new socket code path with mock devices. The Windows cross-check (`--target x86_64-pc-windows-gnu`) exercises the `cfg!(windows)` socket path construction and the `#[cfg(unix)]` cleanup guard.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `interprocess::local_socket::tokio` API shape differs from expected | Low | Medium | MCP lookup confirmed 2.4.2 has `LocalSocketListener::bind`, `.accept().await`, `.into_split()`. Agent verifies exact types at implementation time. |
| `OwnedReadHalf`/`OwnedWriteHalf` trait impls don't match `AsyncRead`/`AsyncWrite` expectations | Low | High | `interprocess` docs confirm these types implement the tokio async traits. Framing layer uses generic bounds (`AsyncWrite + Unpin`, `AsyncRead + Unpin`) which will be satisfied. |
| Tests using `inject_handles_for_test()` with `cat` child process break | Medium | Medium | Update `inject_handles_for_test` to accept mock-compatible halves. For tests that don't need real IPC (keepalive), no change needed. |
| Socket path conflict on restart (stale socket file) | Low | Medium | `LocalSocketListener::bind` on Unix will fail if socket exists. Task spec says "recreate directory overwriting stale socket" — agent should call `tokio::fs::remove_file` before bind (best-effort). |
| Windows cross-compilation fails due to interprocess cfg gates | Low | High | interprocess 2.4.2 explicitly supports Windows. Cross-check (`--target x86_64-pc-windows-gnu`) will catch issues early. |
| `tokio::time::timeout` on `listener.accept()` leaves listener in unusable state on timeout | Medium | Medium | On timeout, drop the listener (it goes out of scope) and return error. The socket file is cleaned up in the drop. Next spawn creates a fresh listener. |

## Acceptance Criteria

- [ ] `cargo clippy -p anvilml-worker --features mock-hardware -- -D warnings` exits 0
- [ ] `cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu` exits 0
- [ ] `cargo test -p anvilml-worker --features mock-hardware` exits 0 (all existing tests pass)
- [ ] `Cargo.toml` version bumped to `0.1.16`
- [ ] `interprocess = { version = "2.4", features = ["tokio"] }` added as dependency
- [ ] `IpcHandles` uses `OwnedReadHalf` / `OwnedWriteHalf` (not `ChildStdin`/`ChildStdout`)
- [ ] `ManagedWorker` has `ipc_socket_path: String` field
- [ ] `spawn()` builds socket path, creates dir, binds listener, accepts with timeout, splits stream
- [ ] `Command` stdin set to `Stdio::null()`; stdout no longer piped; stderr still piped
- [ ] `writer_task` and `reader_task` accept `OwnedWriteHalf` / `OwnedReadHalf`
- [ ] Socket cleanup on writer task exit (Unix: `remove_file`; Windows: no-op)
- [ ] `build_worker_env` called with real socket path (not `""` placeholder)
- [ ] Logging: INFO for socket bind, DEBUG for accept timeout/connection, INFO for cleanup
