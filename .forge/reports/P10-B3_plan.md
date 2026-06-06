# Plan Report: P10-B3

| Field       | Value                                               |
|-------------|-----------------------------------------------------|
| Task ID     | P10-B3                                              |
| Phase       | 010 — Worker Crash Recovery                         |
| Description | anvilml-worker: fix epoll edge-trigger missed wakeup — deliver IpcHandles before writing InitializeHardware |
| Depends on  | P10-B2                                              |
| Project     | anvilml                                             |
| Planned at  | 2026-06-06T21:05:00Z                                |
| Attempt     | 1                                                   |

## Objective

Fix a race condition in `ManagedWorker::spawn()` where `InitializeHardware` is written to the child process stdin pipe before `IpcHandles` (containing stdout) are delivered to `run_loop`. On Linux, `ChildStdout` uses edge-triggered epoll: data arriving before the fd is registered with epoll via the first `poll_read` is never signalled, so `reader_task` blocks forever waiting for a notification that will never come. The fix ensures the reader task registers stdout with epoll *before* any data arrives, by delivering handles via oneshot first and sending `InitializeHardware` through the mpsc channel (writer_task) instead of writing directly to stdin.

## Scope

### In Scope
- Modify `spawn()` in `crates/anvilml-worker/src/managed.rs`:
  - Remove the `tokio::time::sleep(Duration::from_millis(500))` call entirely (line ~202)
  - Move the `ipc_tx.oneshot.send(IpcHandles { stdin, stdout })` call to immediately after `child.stdout.take()`, before any frame write or serialization
  - Remove the `#[cfg(unix)]` fd-dup synchronous write block (lines ~218–231)
  - Remove the `#[cfg(windows)]` async-write + flush block (lines ~233–242)
  - Replace both direct-write blocks with: serialize `InitializeHardware` via `framing::write_frame` into a buffer, then send via `self.tx.send(init_msg).await`
  - Remove now-unused imports: `std::io::Write` (cfg(unix)), `std::os::fd::{AsRawFd, FromRawFd, IntoRawFd}` (cfg(unix))
  - Remove the manual frame header construction (`init_frame_data`, `init_len`, `init_header`) since `framing::write_frame` handles serialization and framing
  - The `rmp_serde` direct usage in `spawn()` for manual serialization is removed; all framing goes through the `anvilml-ipc` crate's `framing::write_frame` (which internally uses `rmp_serde`)
  - Retain `libc` dependency for `PR_SET_PDEATHSIG` in `pre_exec` block
- The `wait_for_ready` polling loop at end of `spawn()` remains unchanged
- No new tests are written (P10-B4 handles the regression test)

### Out of Scope
- Any changes to `pool.rs`, `env.rs`, or `lib.rs`
- Changes to `framing.rs` in `anvilml-ipc`
- Changes to Python worker code (`worker/worker_main.py`)
- Changes to test files (handled by P10-B4)
- Any changes outside `crates/anvilml-worker/src/managed.rs`

## Approach

### Step 1: Move IpcHandles delivery before any write

In `spawn()`, after the lines that take stdin/stdout from the child (~lines 180–188), **immediately** send the handles via oneshot to `run_loop`. This is currently at lines ~245–256, moved down past the sleep and direct writes. The new position is right after the stderr detach block (line ~197).

### Step 2: Remove the 500ms sleep

Delete the entire `tokio::time::sleep(std::time::Duration::from_millis(500)).await;` call at line ~202. It was a workaround for the direct-write race and is no longer needed — the reader task will be polling before any data arrives.

### Step 3: Remove direct stdin writes and use mpsc channel instead

Delete both the `#[cfg(unix)]` fd-dup block (lines ~218–231) and the `#[cfg(windows)]` async-write block (lines ~233–242). Instead, after moving IpcHandles delivery:

1. Serialize `InitializeHardware` into a frame using `framing::write_frame`:
   - Clone stdin into a local variable (it's already taken)
   - Call `framing::write_frame(&mut stdin, &init_msg).await` where `init_msg = WorkerMessage::InitializeHardware { device_str: ... }`
   
Wait — actually, we can't write to stdin before it's sent via oneshot. The correct approach is:

1. Send IpcHandles via oneshot (now first)
2. Serialize `InitializeHardware` using `framing::write_frame` into a Vec<u8> buffer
3. Send the message through the mpsc channel: `self.tx.send(init_msg).await`

The `writer_task` inside `run_loop` will receive the message and write it to stdin after `reader_task` is already polling stdout. This guarantees the epoll registration happens before any data arrives.

### Step 4: Remove unused imports

Delete these lines at the top of `managed.rs`:
- Line 12: `#[cfg(unix)] use std::io::Write;`
- Lines 13–14: `#[cfg(unix)] use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd};`

### Step 5: Remove unused manual serialization in spawn()

The following variables are only used by the deleted direct-write blocks and can be removed:
- `init_frame_data` (rmp_serde serialization)
- `init_len` (length as u32)
- `init_header` (4-byte header bytes)

These were at lines ~211–216. The `framing::write_frame` function in the writer_task handles all serialization and framing.

### Step 6: Retain libc for PR_SET_PDEATHSIG

The `libc` dependency is used in `pre_exec` (lines ~153–159) for `PR_SET_PDEATHSIG`. Keep this block unchanged.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-worker/src/managed.rs` | Fix epoll race: reorder spawn(), remove direct writes, use mpsc channel for InitializeHardware, clean up unused imports |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|-----------------|
| `crates/anvilml-worker/src/managed.rs` (tests module) | `spawn_ping_pong` | Worker spawns, receives ping, responds with pong (already exists, may be #[ignore] — P10-B4 unskips) |
| `crates/anvilml-worker/src/managed.rs` (tests module) | `status_transitions` | Initializing → Idle → Dead lifecycle (already exists, may be #[ignore] — P10-B4 unskips) |
| `crates/anvilml-worker/src/managed.rs` (tests module) | `handshake_completes_once` | Exactly one Ready event after spawn, no duplicate/dying events (already exists, guards against double-write regression from P10-B1) |
| `crates/anvilml-worker/src/managed.rs` (tests module) | `eof_sets_dead` | EOF on pipe sets status to Dead (unit test with duplex pipe) |
| `crates/anvilml-worker/src/managed.rs` (tests module) | `keepalive_pings_and_kills_on_timeout` | Keepalive watchdog kills worker on Pong timeout (mock-based) |
| `crates/anvilml-worker/src/managed.rs` (tests module) | `respawn_after_death` | Dead → Respawning → Idle lifecycle (mock-based) |

Note: The integration tests against a real Python subprocess (`spawn_ping_pong`, `status_transitions`, `handshake_completes_once`) were previously failing on Linux due to the epoll issue. After this fix, they should pass. P10-B4 will unskip any remaining `#[ignore]` attributes and add the canonical `spawn_reaches_idle` regression test.

## CI Impact

No CI workflow files are modified. The existing CI gates must all pass:
- `cargo test -p anvilml-worker --features mock-hardware` — must exit 0 (the epoll fix should make previously-failing tests pass)
- `cargo check --target x86_64-pc-windows-gnu --features mock-hardware` — cross-check compilation
- `cargo clippy --workspace --features mock-hardware -- -D warnings` — zero warnings
- `cargo fmt --all -- --check` — no formatting drift

The change is purely a reorder + removal in managed.rs, so no new CI jobs are needed. The Windows cross-check is important because the `#[cfg(windows)]` async-write block is being removed — this exercises that the cfg-gated code path compiles cleanly without dead code.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Moving IpcHandles before any write causes writer_task to start before stdin is ready for writing | Low | High — could cause writer_task to block on initial write, deadlocking spawn() | The mpsc send (`self.tx.send(init_msg).await`) is non-blocking up to channel capacity (64). The actual write happens in writer_task which runs after run_loop receives handles. If stdin isn't ready yet, the OS pipe buffer absorbs the write — this is safe. |
| Removing direct writes breaks Windows path | Low | Medium — Windows may have different pipe semantics | The `#[cfg(windows)]` block is being removed entirely. Windows pipes are not epoll-based; they use completion ports. The mpsc channel approach works identically on both platforms. The Windows cross-check (`cargo check --target x86_64-pc-windows-gnu`) will catch any cfg issues. |
| rmp_serde becomes unused in managed.rs top-level imports | Low | Low — clippy will warn about unused import | After removing the direct serialization, `rmp_serde` is no longer imported at the crate level in managed.rs (it was only used via the framing module). The dev-dependency for tests remains. |
| Tests that were previously passing with the sleep workaround may behave differently | Low | Medium — test timing changes | The 500ms sleep was masking the underlying race. Removing it reveals correct behavior. All three integration tests (`spawn_ping_pong`, `status_transitions`, `handshake_completes_once`) should now pass reliably without sleeps. |
| Writer task receives InitializeHardware before reader task registers epoll fd | Very Low | High — same bug reappears | Impossible by construction: `run_loop` spawns `writer_task` **after** receiving handles via oneshot, and spawns `reader_task` in the same block concurrently. Both tasks start essentially simultaneously, but `framing::write_frame` in writer_task will block on the first `poll_read` if stdout isn't ready — however, since both are spawned in the same `spawn()` call, tokio's single-threaded runtime ensures they both begin polling before any yield point. The key guarantee: `reader_task`'s first await (`framing::read_frame` → `poll_read`) happens before writer_task's first await (`framing::write_frame` → `poll_write`). This ordering is guaranteed because reader_task is spawned first in the `run_loop` function (line 553). |

## Acceptance Criteria

- [ ] `spawn()` no longer contains any `tokio::time::sleep` call
- [ ] `spawn()` no longer contains `#[cfg(unix)]` fd-dup write block or `libc::dup` usage
- [ ] `spawn()` no longer contains `#[cfg(windows)]` async-write block
- [ ] `spawn()` no longer imports `std::io::Write`, `AsRawFd`, `FromRawFd`, `IntoRawFd`
- [ ] `spawn()` sends `InitializeHardware` via `self.tx.send(...).await` (mpsc channel) instead of direct pipe writes
- [ ] `libc` dependency retained for `PR_SET_PDEATHSIG` in `pre_exec`
- [ ] `cargo test -p anvilml-worker --features mock-hardware` exits 0
- [ ] `cargo check --target x86_64-pc-windows-gnu --features mock-hardware` exits 0
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0 (no new warnings)
- [ ] `cargo fmt --all -- --check` exits 0
