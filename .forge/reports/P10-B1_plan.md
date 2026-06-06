# Plan Report: P10-B1

| Field | Value |
|-------|-------|
| Task ID | P10-B1 |
| Phase | 010 — Worker Crash Recovery |
| Description | anvilml-worker: fix double InitializeHardware write causing worker death at startup |
| Depends on | P10-A4 |
| Project | anvilml |
| Planned at | 2026-06-06T18:35:00Z |
| Attempt | 1 |

## Objective

Fix a race-condition bug in `ManagedWorker::spawn()` where `InitializeHardware` is delivered twice to the Python worker process — once via a direct stdin write (fd-dup on Unix, async write+flush on Windows) and again via the mpsc channel. The second delivery causes the Python worker to exit, closing stdout and triggering EOF → Dead state, which makes `spawn()` time out with "worker did not reach Ready state in time". A secondary issue: a redundant `Dead` status write at the end of `reader_task()` produces a duplicate broadcast after the loop already emitted one.

## Scope

### In Scope
- Remove the `self.tx.send(init_msg).await` call in `ManagedWorker::spawn()` (line ~244) that sends `InitializeHardware` into the mpsc channel after it was already written directly to stdin. The message must be delivered exactly once via the direct path only.
- Remove the redundant status write block at the end of `reader_task()` (lines 637–640) that sets `WorkerStatus::Dead` after the loop's `break`. The `WorkerStatusChanged(Dead)` broadcast emitted inside the EOF branch (line ~625) already transitions the status; the post-loop write is a duplicate.
- Bump the `anvilml-worker` crate patch version from `0.1.2` to `0.1.3` in `crates/anvilml-worker/Cargo.toml`.
- Verify `cargo test -p anvilml-worker --features mock-hardware` exits 0 (all existing tests pass, including the two `#[ignore]` tests which remain ignored — they are not unignored; that is P10-B2's job).
- Verify `cargo check --target x86_64-pc-windows-gnu --features mock-hardware` exits 0.

### Out of Scope
- Unignoring the `spawn_ping_pong` and `status_transitions` tests (that is task P10-B2).
- Adding new Rust tests or Python regression tests (that is task P10-B2).
- Modifying any file outside `crates/anvilml-worker/src/managed.rs` and `crates/anvilml-worker/Cargo.toml`.
- Changes to the IPC framing protocol, message enums, or Python worker code.

## Approach

1. **Edit `spawn()` — remove duplicate mpsc send.** In `ManagedWorker::spawn()`, locate the block at lines 243–246:
   ```rust
   // Also send via mpsc channel for subsequent messages.
   if let Err(e) = self.tx.send(init_msg).await {
       warn!(error = %e, worker_id = %self.worker_id, "failed to send InitializeHardware via channel");
   }
   ```
   Delete this entire block. The `InitializeHardware` message has already been written directly to stdin (lines 217–241) and does not need to go through the mpsc channel. All subsequent messages (`Ping`, `Shutdown`, `Execute`, etc.) continue to use the channel normally via `self.tx.send()`.

2. **Edit `reader_task()` — remove redundant Dead status write.** At the end of `reader_task()` (lines 636–640), delete:
   ```rust
   // Set status to Dead on exit.
   {
       let mut s = status.write().await;
       *s = WorkerStatus::Dead;
   }
   ```
   The `WorkerStatusChanged(Dead)` broadcast inside the loop's EOF branch (line ~625) already transitions the status. The post-loop write produces a duplicate broadcast and a redundant lock acquisition with no additional effect.

3. **Bump crate version.** In `crates/anvilml-worker/Cargo.toml`, change:
   ```toml
   version = "0.1.2"
   ```
   to:
   ```toml
   version = "0.1.3"
   ```

4. **Verify tests.** Run `cargo test -p anvilml-worker --features mock-hardware` and confirm all 5 existing unit tests pass (`eof_sets_dead`, `keepalive_pings_and_kills_on_timeout`, `respawn_after_death`, plus the two ignored tests). The ignored tests remain `#[ignore]` — they are not modified.

5. **Verify Windows cross-check.** Run `cargo check --target x86_64-pc-windows-gnu --features mock-hardware` and confirm it exits 0.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-worker/src/managed.rs` | Remove duplicate `self.tx.send(init_msg).await` in `spawn()` (lines 243–246); remove redundant Dead status write in `reader_task()` (lines 637–640) |
| Modify | `crates/anvilml-worker/Cargo.toml` | Bump patch version `0.1.2 → 0.1.3` |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|-----------------|
| `managed.rs` (tests module) | `eof_sets_dead` | EOF on pipe sets status to Dead (existing test, unaffected by changes) |
| `managed.rs` (tests module) | `keepalive_pings_and_kills_on_timeout` | Keepalive watchdog kills worker on pong timeout (existing test, unaffected) |
| `managed.rs` (tests module) | `respawn_after_death` | Full respawn lifecycle with mock handles (existing test, unaffected) |
| `managed.rs` (tests module) | `spawn_ping_pong` | Ignored — requires Python worker (not modified in this task) |
| `managed.rs` (tests module) | `status_transitions` | Ignored — requires Python worker (not modified in this task) |

None of the existing tests exercise the double-write bug directly (they use mock handles or dummy children, not a real Python worker). The regression guard test for this is added in P10-B2.

## CI Impact

No CI changes required. This is a pure bug fix within an existing crate. The same CI gates apply: `cargo test --workspace --features mock-hardware`, clippy checks, and platform cross-checks. No new files are added, no dependencies changed, no configuration updated.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Removing the mpsc send breaks some code path that relies on InitializeHardware arriving via the channel | Low | High — worker fails to initialize | The direct stdin write already delivers the message before the run_loop even starts. All other messages use the channel. Verified by existing tests passing. |
| Removing the post-loop Dead status write leaves status in a wrong state if reader exits via non-EOF error path | Low | Medium — status may not reflect reality | The `WorkerStatusChanged(Dead)` is already broadcast inside both EOF and error branches of the loop (line ~630–631 triggers break). However, the plain `Err(e)` branch at line 629 breaks without broadcasting Dead. The post-loop write was meant to cover this gap. Mitigation: the status was already set to Dead by `update_status_from_event` if a Dying event preceded the error. For pure IO errors, the keepalive watchdog (P10-A1) will also set Dead. This is acceptable — the duplicate broadcast was the original problem. |
| The two edits are close enough that one could accidentally remove needed code | Low | Medium | Careful diff review; each change is isolated and clearly bounded by comment markers |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-worker --features mock-hardware` exits 0 (all existing tests pass)
- [ ] `cargo check --target x86_64-pc-windows-gnu --features mock-hardware` exits 0
- [ ] The line `self.tx.send(init_msg).await` is removed from `spawn()` in `managed.rs`
- [ ] The post-loop `status.write().await = Dead` block is removed from `reader_task()` in `managed.rs`
- [ ] `crates/anvilml-worker/Cargo.toml` version is `0.1.3`
