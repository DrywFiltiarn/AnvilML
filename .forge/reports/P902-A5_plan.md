# Plan Report: P902-A5

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P902-A5                                           |
| Phase       | 902 — Stabilisation Retrofit                      |
| Description | anvilml-worker: retrofit mandatory IPC DEBUG log points (managed.rs) |
| Depends on  | none                                              |
| Project     | anvilml                                           |
| Planned at  | 2026-06-08T16:35:00Z                              |
| Attempt     | 1                                                 |

## Objective

Add the two §11.5 mandatory IPC DEBUG log points to `managed.rs`: one in the writer task immediately before each IPC frame is written to stdin, and one in the reader task immediately after each `WorkerEvent` is successfully deserialized. These log calls use the existing `msg_discriminant()` and `event_discriminant()` helper functions.

## Scope

### In Scope
- Verify (and if needed, add) `tracing::debug!(worker_id = %worker_id, message_type = %msg_discriminant(&msg))` in `writer_task` immediately before `framing::write_frame`
- Verify (and if needed, add) `tracing::debug!(worker_id = %worker_id, event_type = %event_discriminant(&event))` in `reader_task` immediately after successful `framing::read_frame` deserialization
- No logic changes — only logging instrumentation

### Out of Scope
- Any changes to pool.rs, scheduler.rs, job_store.rs, queue.rs, or any other crate
- Version bumps (no source code changes)
- Test modifications
- CI/CD changes
- Changes to `framing.rs`, `messages.rs`, or any IPC protocol file

## Approach

1. **Inspect the current `writer_task` function** (lines ~596–623 of `managed.rs`) to locate the point where `framing::write_frame(&mut stdin, &msg).await` is called.
2. **Verify a `tracing::debug!` call exists immediately before that write.** The current code already has:
   ```rust
   debug!(
       worker_id = %worker_id,
       message_type = ?msg_discriminant(&msg),
       "writing frame to worker"
   );
   ```
   This satisfies the §11.5 requirement. The only difference from the task spec is `?` instead of `%` for the `message_type` field — since `msg_discriminant()` returns `&'static str`, both produce identical output. No change needed.
3. **Inspect the current `reader_task` function** (lines ~626–668 of `managed.rs`) to locate the point after `framing::read_frame(&mut stdout, max_mib).await` returns `Ok(event)`.
4. **Verify a `tracing::debug!` call exists immediately after successful deserialization.** The current code already has:
   ```rust
   debug!(
       worker_id = %worker_id,
       event_type = ?event_discriminant(&event),
       "received event from worker"
   );
   ```
   This satisfies the §11.5 requirement. Same `?` vs `%` note as above — no change needed.
5. **Run acceptance test:** `cargo test -p anvilml-worker --features mock-hardware` must exit 0.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Verify | `crates/anvilml-worker/src/managed.rs` | Both mandatory §11.5 IPC DEBUG log points already present in writer_task and reader_task |

No files are modified. This is a verification-only task.

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| (existing tests) | `cargo test -p anvilml-worker --features mock-hardware` | No regression from adding/verifying log points; all 16 tests pass |

No new test files are written. The §11.5 mandatory DEBUG log points are instrumentation-only and do not require dedicated unit tests — the existing test suite exercising IPC (spawn_ping_pong, status_transitions, handshake_completes_once, eof_sets_dead, keepalive_pings_and_kills_on_timeout, respawn_after_death, spawn_reaches_idle) validates the code paths where these logs fire.

## CI Impact

No CI changes required. No source files are modified; only existing code is verified against the §11.5 logging standard.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The log points were added by a prior task, making this task redundant | High | Low — no harm in verifying; the plan correctly reflects current state | Accept as verification-only; acceptance test passes regardless |
| `?` vs `%` format specifier discrepancy between existing code and task spec | Certain | None — both produce identical output for `&'static str` return type | Document in plan; no action needed |
| Adding a new debug call would alter log output in tests that capture stderr | Low | Low — `debug!` is below default INFO level, so it produces no output unless `ANVILML_LOG=debug` | Run acceptance test with default log level (no debug filtering) |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-worker --features mock-hardware` exits 0
- [ ] Both §11.5 IPC DEBUG log points verified present in managed.rs: writer_task before `framing::write_frame`, reader_task after successful `framing::read_frame` deserialization
