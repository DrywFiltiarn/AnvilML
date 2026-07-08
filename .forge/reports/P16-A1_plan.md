# Plan Report: P16-A1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P16-A1                                        |
| Phase       | 016 — Live Events                             |
| Description | anvilml-scheduler: event_loop subscribes WorkerEvent, publishes WsEvent |
| Depends on  | P15-C1                                         |
| Project     | anvilml                                        |
| Planned at  | 2026-07-09T00:00:00Z                          |
| Attempt     | 1                                              |

## Objective

Give every `WorkerEvent` variant its first real consumer by wiring the scheduler's event
loop to the `RouterTransport` receive path. `spawn_event_loop()` will loop on
`transport.recv()`, map each `WorkerEvent` to its corresponding `WsEvent` counterpart,
publish it via the shared `EventBroadcaster`, and log a `DEBUG` transition record
(job_id, from, to) per `ANVILML_DESIGN.md §16.3`. The `ImageReady` path is extended to
publish `JobImageReady` **after** the artifact save succeeds (not before). This closes
the gap that left `Progress`/`Completed`/`Failed`/`Cancelled` events unconsumed since
Phase 3 defined them.

## Scope

### In Scope
- Add `pub fn spawn_event_loop(self: Arc<JobScheduler>, transport: Arc<RouterTransport>, broadcaster: Arc<EventBroadcaster>) -> JoinHandle<()>` to `event_loop.rs`.
- Add `pub fn map_worker_event(event: WorkerEvent) -> WsEvent` helper that performs the one-to-one mapping.
- In the loop body: `transport.recv()` → `map_worker_event()` → `broadcaster.publish()` → DEBUG log.
- `ImageReady` path: after `handle_image_ready()` save succeeds, publish `JobImageReady` (not before).
- Log `DEBUG` transition per `ANVILML_DESIGN.md §16.3`: `job_id=%id, from=%old, to=%new`.
- Add ≥6 new tests to `tests/event_loop_tests.rs` covering each of the 4 new variants,
  the `ImageReady` save-before-publish ordering, and the event loop end-to-end path.
- Version bump `anvilml-scheduler` patch version `0.1.20 → 0.1.21`.

### Out of Scope
None. The `defers_to (from JSON): []` value is empty — no scope is deferred. The interim
patch (`interim_job_completion.rs`) removal is explicitly scoped to P16-A2/P16-B1 per
`TASKS_PHASE016.md`'s "Known Constraints" section.

## Existing Codebase Assessment

The existing `event_loop.rs` (Phase 15's P15-C1) contains only `handle_image_ready()`,
which base64-decodes the image payload, constructs an `ArtifactMeta`, and calls
`artifact_store.save()`. It is called from the interim job completion listener
(`interim_job_completion.rs`) but that module is not touched by this task.

`JobScheduler` (Phase 14) already holds an `Arc<ArtifactStore>` field (`artifact_store`)
and exposes no public accessor for it. The scheduler's dispatch loop (`start_dispatch_loop()`)
establishes the pattern: a `pub fn` taking `Arc<Self>` plus external dependencies, returning
`JoinHandle<()>`.

`RouterTransport::recv()` (Phase 7) returns `Result<(String, WorkerEvent), IpcError>` —
the worker identity (frame 0) and the deserialized event (last frame). The 2/3-frame
ROUTER layout is handled transparently by `recv()`.

`EventBroadcaster` (Phase 7) wraps `tokio::sync::broadcast::Sender<WsEvent>` with
capacity 1024. `publish()` silently ignores `SendError` (zero subscribers).

The established patterns to follow:
- `#[tracing::instrument]` on public async functions.
- Structured logging with `tracing::info!`/`tracing::debug!` using `%` format for display fields.
- Integration tests in `tests/` as separate test crates (not inline `#[cfg(test)]`).
- The existing test file uses `create_test_artifact_store()` and `make_valid_png_b64()`
  helpers that I will reuse.

Gap between design doc and source: the design doc (§16.3) specifies the mandatory DEBUG
log point for job state transitions as `job_id=%id, from=%old, to=%new`. The existing
`handle_image_ready()` does not log a transition (there was no prior state to track).
This task introduces the first transition logging by recording the WorkerEvent variant
as the "from" and the WsEvent variant as the "to".

## Resolved Dependencies

No new external crates are introduced. All types and APIs used are from existing
dependencies already declared in `anvilml-scheduler/Cargo.toml`.

| Type   | Name          | Version verified | MCP source     | Feature flags confirmed |
|--------|---------------|-----------------|----------------|------------------------|
| crate  | zeromq        | 0.6.0           | Cargo.lock (existing dep) | tokio-runtime, all-transport |
| crate  | tokio         | 1.52.3          | Cargo.lock (existing dep) | rt, sync, macros, spawn |
| crate  | base64        | 0.22.1          | Cargo.lock (existing dep) | n/a |
| crate  | uuid          | 1.23.4          | Cargo.lock (existing dep) | v4 |

All external API names (`RouterSocket`, `DealerSocket`, `ZmqMessage`, `spawn`,
`broadcast::Sender`, etc.) are confirmed against the versions already in the
workspace lockfile — no MCP lookup needed since no new versions are being introduced.

## Approach

### Step 1 — Add `map_worker_event()` helper function to `event_loop.rs`

Add a new `pub fn map_worker_event(event: WorkerEvent) -> WsEvent` that pattern-matches
on the input `WorkerEvent` and returns the corresponding `WsEvent`:

```rust
pub fn map_worker_event(event: WorkerEvent) -> WsEvent {
    match event {
        WorkerEvent::Progress { job_id, step, total_steps, preview_b64 } => {
            WsEvent::JobProgress { job_id, step, total_steps, preview_b64 }
        }
        WorkerEvent::Completed { job_id, elapsed_ms } => {
            WsEvent::JobCompleted { job_id, elapsed_ms }
        }
        WorkerEvent::Failed { job_id, error, traceback: _ } => {
            // traceback is omitted from WsEvent::JobFailed per the type definition
            WsEvent::JobFailed { job_id, error }
        }
        WorkerEvent::Cancelled { job_id } => {
            WsEvent::JobCancelled { job_id }
        }
        WorkerEvent::ImageReady { job_id, image_b64: _, width, height, format: _, seed, steps } => {
            // ImageReady is handled specially in spawn_event_loop —
            // this branch should never be reached via the mapping path.
            // Return a placeholder WsEvent; the caller must not map ImageReady
            // through this function directly.
            WsEvent::JobImageReady {
                job_id,
                artifact_hash: String::new(),
                width,
                height,
                seed,
                steps,
            }
        }
        // Other events (Ready, Pong, Dying, MemoryReport) are not mapped
        // to WsEvent — they are handled elsewhere or ignored.
        WorkerEvent::Ready { .. } => panic!("Ready events are handled by the node registry, not the event loop"),
        WorkerEvent::Pong { .. } => panic!("Pong events are handled by the keepalive watchdog, not the event loop"),
        WorkerEvent::Dying { .. } => panic!("Dying events are handled by the worker pool, not the event loop"),
        WorkerEvent::MemoryReport { .. } => panic!("MemoryReport events are handled by the worker pool, not the event loop"),
    }
}
```

Rationale: `WorkerEvent::ImageReady` is excluded from the mapping because it requires
the artifact save to complete before publishing `JobImageReady`. The `traceback` field
from `Failed` is dropped since `WsEvent::JobFailed` only carries `error` (not `traceback`).

### Step 2 — Add `spawn_event_loop()` to `event_loop.rs`

Add the main event loop function. It runs an infinite loop that:
1. Calls `transport.recv()` to get `(worker_id, event)`.
2. If the event is `ImageReady`: call `handle_image_ready()` with the scheduler's
   `artifact_store`, then publish `JobImageReady` with the returned hash.
3. For all other mapped events: call `map_worker_event()` and publish the result.
4. Log a `DEBUG` transition record after each publish.

```rust
#[tracing::instrument(skip(self, transport, broadcaster))]
pub fn spawn_event_loop(
    self: Arc<JobScheduler>,
    transport: Arc<RouterTransport>,
    broadcaster: Arc<EventBroadcaster>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            // Receive the next event from the worker. This blocks until
            // a message arrives on the ROUTER socket. The recv() method
            // returns (worker_identity, WorkerEvent).
            let (worker_id, event) = match transport.recv().await {
                Ok(pair) => pair,
                Err(e) => {
                    // Transport error (e.g. closed). Log and retry on the
                    // next iteration — the loop continues indefinitely.
                    tracing::error!(error = %e, "event_loop recv error, retrying");
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    continue;
                }
            };

            // Route ImageReady through the artifact save path, then publish.
            // All other event types go through the generic mapping path.
            match event {
                WorkerEvent::ImageReady { job_id, .. } => {
                    // Save the artifact first — this is the critical ordering
                    // requirement: JobImageReady must only be published AFTER
                    // the save succeeds, never before.
                    match handle_image_ready(self.artifact_store.clone(), event, job_id).await {
                        Ok(hash) => {
                            let ws_event = WsEvent::JobImageReady {
                                job_id,
                                artifact_hash: hash,
                                // width/height/seed/steps extracted below
                                width: 0, height: 0, seed: 0, steps: 0,
                            };
                            broadcaster.publish(ws_event);
                            tracing::debug!(
                                job_id = %job_id,
                                from = "ImageReady",
                                to = "JobImageReady",
                                "event transition"
                            );
                        }
                        Err(e) => {
                            // Save failed — log the error but do not publish
                            // JobImageReady. The event loop continues to the
                            // next message.
                            tracing::error!(
                                job_id = %job_id,
                                error = %e,
                                "event_loop image_ready save failed"
                            );
                        }
                    }
                }
                // All other events go through the generic mapping path.
                _ => {
                    let ws_event = map_worker_event(event);
                    broadcaster.publish(ws_event);

                    // Log the state transition per §16.3. The "from" is the
                    // WorkerEvent variant name, "to" is the WsEvent variant name.
                    let from_variant = match &ws_event {
                        WsEvent::JobProgress { .. } => "Progress",
                        WsEvent::JobCompleted { .. } => "Completed",
                        WsEvent::JobFailed { .. } => "Failed",
                        WsEvent::JobCancelled { .. } => "Cancelled",
                        _ => "Other",
                    };
                    let to_variant = match &ws_event {
                        WsEvent::JobProgress { .. } => "JobProgress",
                        WsEvent::JobCompleted { .. } => "JobCompleted",
                        WsEvent::JobFailed { .. } => "JobFailed",
                        WsEvent::JobCancelled { .. } => "JobCancelled",
                        _ => "Other",
                    };

                    // Extract job_id for the log if present.
                    let job_id = match &ws_event {
                        WsEvent::JobProgress { job_id, .. }
                        | WsEvent::JobCompleted { job_id, .. }
                        | WsEvent::JobFailed { job_id, .. }
                        | WsEvent::JobCancelled { job_id, .. } => Some(*job_id),
                        _ => None,
                    };

                    if let Some(jid) = job_id {
                        tracing::debug!(
                            job_id = %jid,
                            from = from_variant,
                            to = to_variant,
                            "event transition"
                        );
                    }
                }
            }
        }
    })
}
```

Rationale for the `ImageReady` handling: the `handle_image_ready()` function extracts
all needed fields (width, height, seed, steps) from the event internally. After the save
succeeds and returns the hash, we construct `JobImageReady` with those fields plus the
hash. The `width/height/seed/steps` values above are placeholders — the ACT agent will
need to restructure this slightly (see Public API Surface below).

Wait — actually there's a problem. The `handle_image_ready()` function consumes the
`event` and doesn't return the individual fields. We need the `width`, `height`, `seed`,
and `steps` values to construct `JobImageReady`. Let me reconsider the approach.

Better approach: destructure the `ImageReady` event before calling `handle_image_ready()`,
pass the needed fields separately, and pass the event (or a clone) to `handle_image_ready()`.
But `handle_image_ready()` takes ownership of `event` for the pattern match inside.

Actually, looking at `handle_image_ready()` more carefully — it takes `WorkerEvent` by
value and destructures it internally. The function doesn't return the individual fields.
So I need to either:
(a) Clone the event before passing it to `handle_image_ready()`, or
(b) Restructure `handle_image_ready()` to return the fields along with the hash.

Option (a) is simpler and doesn't change the existing function's contract. The `ImageReady`
event is small (a few fields), so cloning is cheap. This is the correct approach for this
task since it minimizes changes to existing code.

Revised `ImageReady` handling:
```rust
WorkerEvent::ImageReady { job_id, .. } => {
    // Clone the event so handle_image_ready() can consume its copy
    // while we retain the fields needed for JobImageReady construction.
    let event_clone = event.clone();
    let (width, height, seed, steps) = match &event {
        WorkerEvent::ImageReady { width, height, seed, steps, .. } => (*width, *height, *seed, *steps),
        _ => unreachable!(),
    };

    match handle_image_ready(self.artifact_store.clone(), event, job_id).await {
        Ok(hash) => {
            let ws_event = WsEvent::JobImageReady {
                job_id,
                artifact_hash: hash,
                width, height, seed, steps,
            };
            broadcaster.publish(ws_event);
            tracing::debug!(
                job_id = %job_id,
                from = "ImageReady",
                to = "JobImageReady",
                "event transition"
            );
        }
        Err(e) => {
            tracing::error!(
                job_id = %job_id,
                error = %e,
                "event_loop image_ready save failed"
            );
        }
    }
}
```

### Step 3 — Add `JoinHandle` import to `event_loop.rs`

Add `use tokio::task::JoinHandle;` at the top of `event_loop.rs`.

### Step 4 — Re-export `spawn_event_loop` from `lib.rs`

Add `pub use event_loop::spawn_event_loop;` to `lib.rs`.

### Step 5 — Write integration tests in `tests/event_loop_tests.rs`

Add 7 new tests:
1. `test_map_progress` — verifies `WorkerEvent::Progress` → `WsEvent::JobProgress` field mapping
2. `test_map_completed` — verifies `WorkerEvent::Completed` → `WsEvent::JobCompleted` field mapping
3. `test_map_failed` — verifies `WorkerEvent::Failed` → `WsEvent::JobFailed` (traceback dropped)
4. `test_map_cancelled` — verifies `WorkerEvent::Cancelled` → `WsEvent::JobCancelled` field mapping
5. `test_image_ready_publishes_after_save` — end-to-end: sends `ImageReady` through the event loop,
   verifies `JobImageReady` is published only after the artifact save succeeds
6. `test_spawn_event_loop_receives_and_publishes` — end-to-end with real transport: creates a
   ROUTER/DEALER pair, spawns the event loop, sends a `Completed` event, verifies the
   broadcaster receives the correct `JobCompleted`
7. `test_spawn_event_loop_handles_recv_error` — verifies the event loop retries after a
   transport recv error (by closing the transport)

See `## Tests` section for the full table.

### Step 6 — Version bump

Bump `anvilml-scheduler` patch version from `0.1.20` to `0.1.21` in `Cargo.toml`.

## Public API Surface

| Item | Crate/Module Path | Signature |
|------|-------------------|-----------|
| `map_worker_event` | `crates/anvilml-scheduler/src/event_loop.rs` | `pub fn map_worker_event(event: WorkerEvent) -> WsEvent` |
| `spawn_event_loop` | `crates/anvilml-scheduler/src/event_loop.rs` | `pub fn spawn_event_loop(self: Arc<JobScheduler>, transport: Arc<RouterTransport>, broadcaster: Arc<EventBroadcaster>) -> JoinHandle<()>` |

Both items are re-exported from `anvilml-scheduler` via `lib.rs`:
- `pub use event_loop::map_worker_event;`
- `pub use event_loop::spawn_event_loop;`

No changes to existing public APIs. No new structs or traits.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-scheduler/src/event_loop.rs` | Add `map_worker_event()` and `spawn_event_loop()` |
| Modify | `crates/anvilml-scheduler/src/lib.rs` | Re-export new pub items |
| Modify | `crates/anvilml-scheduler/tests/event_loop_tests.rs` | Add 7 new integration tests |
| Modify | `crates/anvilml-scheduler/Cargo.toml` | Bump patch version 0.1.20 → 0.1.21 |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|--------------------|
| `event_loop_tests.rs` | `test_map_progress` | `WorkerEvent::Progress` maps to `WsEvent::JobProgress` with correct field values | `cargo test -p anvilml-scheduler --test event_loop_tests test_map_progress` exits 0 |
| `event_loop_tests.rs` | `test_map_completed` | `WorkerEvent::Completed` maps to `WsEvent::JobCompleted` with correct field values | `cargo test -p anvilml-scheduler --test event_loop_tests test_map_completed` exits 0 |
| `event_loop_tests.rs` | `test_map_failed` | `WorkerEvent::Failed` maps to `WsEvent::JobFailed` with correct fields; `traceback` is dropped | `cargo test -p anvilml-scheduler --test event_loop_tests test_map_failed` exits 0 |
| `event_loop_tests.rs` | `test_map_cancelled` | `WorkerEvent::Cancelled` maps to `WsEvent::JobCancelled` with correct field values | `cargo test -p anvilml-scheduler --test event_loop_tests test_map_cancelled` exits 0 |
| `event_loop_tests.rs` | `test_image_ready_publishes_after_save` | `spawn_event_loop` publishes `JobImageReady` only after `handle_image_ready()` save succeeds; verifies ordering by instrumenting the artifact store | `cargo test -p anvilml-scheduler --test event_loop_tests test_image_ready_publishes_after_save` exits 0 |
| `event_loop_tests.rs` | `test_spawn_event_loop_receives_and_publishes` | End-to-end: real ROUTER/DEALER transport, `spawn_event_loop` receives `Completed`, broadcaster emits `JobCompleted` with correct fields | `cargo test -p anvilml-scheduler --test event_loop_tests test_spawn_event_loop_receives_and_publishes` exits 0 |
| `event_loop_tests.rs` | `test_spawn_event_loop_handles_recv_error` | Event loop retries gracefully after transport recv error (transport closed) | `cargo test -p anvilml-scheduler --test event_loop_tests test_spawn_event_loop_handles_recv_error` exits 0 |

Acceptance command: `cargo test -p anvilml-scheduler --test event_loop_tests` exits 0
(≥10 total tests in the file).

## CI Impact

No CI changes required. The new tests run under the existing `cargo test --workspace`
command (Phase 16 Phase Acceptance Criteria). The `mock-hardware` feature flag is
already declared in `anvilml-scheduler/Cargo.toml` and forwarded from workspace-level
test runs.

## Platform Considerations

None identified. The Windows cross-check in `ENVIRONMENT.md §7` is sufficient. The event
loop code uses only `tokio::spawn`, `tokio::time::sleep`, and ZeroMQ — all cross-platform.
No `#[cfg(unix)]` or `#[cfg(windows)]` guards are needed.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `WorkerEvent::ImageReady` cloning before `handle_image_ready()` — the event contains a potentially large base64 string. Cloning it before passing to `handle_image_ready()` doubles memory for this event. | Low | Medium | The base64 string is at most a few MiB (one image). This is acceptable for the event loop's scope; if it becomes a concern, `handle_image_ready()` can be refactored to accept individual fields instead of the full event (out of scope for this task). |
| The `map_worker_event` function includes `panic!` arms for events that should never reach it (`Ready`, `Pong`, `Dying`, `MemoryReport`). If a new `WorkerEvent` variant is added to `messages.rs` without updating this function, the compiler will produce a non-exhaustive match error (Rust 2024 edition with `#[non_exhaustive]` on the enum would suppress this). | Low | Medium | The enum is not marked `#[non_exhaustive]`, so adding a new variant will cause a compile error. This is actually desirable — it forces the mapping function to be updated. Document this in the function's doc comment. |
| The end-to-end test with real transport (`test_spawn_event_loop_receives_and_publishes`) requires a ZeroMQ DEALER peer connected to the ROUTER socket. If the DEALER connection fails, the test hangs. | Low | High | Use `tokio::time::timeout` with a 5-second timeout. On timeout failure, surface the event loop task's captured output (if any) in the assertion message. This follows the bounded-wait pattern from `ENVIRONMENT.md §11.5`. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-scheduler --test event_loop_tests test_map_progress` exits 0
- [ ] `cargo test -p anvilml-scheduler --test event_loop_tests test_map_completed` exits 0
- [ ] `cargo test -p anvilml-scheduler --test event_loop_tests test_map_failed` exits 0
- [ ] `cargo test -p anvilml-scheduler --test event_loop_tests test_map_cancelled` exits 0
- [ ] `cargo test -p anvilml-scheduler --test event_loop_tests test_image_ready_publishes_after_save` exits 0
- [ ] `cargo test -p anvilml-scheduler --test event_loop_tests test_spawn_event_loop_receives_and_publishes` exits 0
- [ ] `cargo test -p anvilml-scheduler --test event_loop_tests test_spawn_event_loop_handles_recv_error` exits 0
- [ ] `cargo test -p anvilml-scheduler --test event_loop_tests` exits 0 with ≥10 tests total
- [ ] `grep "^## " .forge/reports/P16-A1_plan.md` shows 12 section headings
- [ ] `wc -l .forge/reports/P16-A1_plan.md` returns > 40 lines
