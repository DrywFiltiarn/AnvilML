/// Event loop module for processing `WorkerEvent` variants from workers.
///
/// This module owns:
/// - `handle_image_ready()` — decodes the base64 image payload, constructs an
///   `ArtifactMeta`, and calls `artifact_store.save()` to persist the decoded PNG
///   bytes under their content hash.
/// - `spawn_event_loop()` — subscribes to `WorkerEvent`s from workers via
///   `Demux::subscribe()` (`ANVILML_DESIGN.md §9.8`), maps each event to its
///   `WsEvent` counterpart, and publishes it via the shared `EventBroadcaster`.
///   Deliberately does **not** hold or call methods on `RouterTransport`
///   directly — `bridge.rs`'s `reader_task` is the sole permitted caller of
///   `RouterTransport::recv()` on the pool's shared ROUTER socket; a second
///   concurrent caller would race it for every incoming frame. See
///   `docs/ADDENDUM_DEMUX_FANOUT.md` for the full background.
/// - `map_worker_event()` — performs the one-to-one mapping between `WorkerEvent`
///   and `WsEvent` variants.
/// - `apply_ready_capabilities()` — writes a worker's runtime-probed `Ready`
///   capabilities *and* real VRAM totals into the shared `HardwareInfo`
///   snapshot (`AppState.hardware`, the source `GET /v1/system` serves), per
///   `ANVILML_DESIGN.md §6.6`. This is the only writer of a `GpuDevice`'s
///   `caps`/`capabilities_source`/`vram_total_mib`/`vram_free_mib` fields
///   after server startup.
///
/// The design separates the event variant matching from the handler function so that
/// callers (the interim job completion listener, or the event loop) can pattern-match
/// on the event to extract the variant before calling the handler. This avoids forcing
/// the caller to destructure the event.
use std::path::PathBuf;
use std::sync::Arc;

use anvilml_core::types::worker::WorkerStatus;
use anvilml_worker::{Demux, WorkerPool};

use crate::JobScheduler;
use anvilml_artifacts::ArtifactStore;
use anvilml_core::{
    AnvilError, ArtifactMeta, CapabilitySource, HardwareInfo, InferenceCaps, JobStatus, WsEvent,
};
use anvilml_hardware::compute_caps_union;
use anvilml_ipc::{EventBroadcaster, WorkerEvent};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use chrono::Utc;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use uuid::Uuid;

/// Handle a `WorkerEvent::ImageReady` event by decoding the base64 image payload,
/// constructing an `ArtifactMeta`, and persisting the decoded bytes to the artifact
/// store.
///
/// The caller is responsible for pattern-matching on the event to ensure it is
/// `ImageReady` before calling this function. The function accepts `WorkerEvent`
/// and `job_id` as separate parameters to avoid forcing the caller to destructure.
///
/// # Arguments
///
/// * `artifact_store` — The artifact storage backend where the decoded image will be saved.
/// * `event` — The worker event. The caller must ensure this is `ImageReady` before calling.
/// * `job_id` — The UUID of the job that produced this artifact.
///
/// # Returns
///
/// On success, returns the SHA-256 hex digest of the saved artifact.
///
/// # Errors
///
/// Returns `AnvilError::Serde` if the base64 decoding fails.
/// Returns `AnvilError::Io` if the filesystem write fails.
/// Returns `AnvilError::Db` if the metadata persistence fails.
///
/// `event` is skipped from the span (see `#[instrument]` below) — the
/// `ImageReady` variant carries `image_b64`, the full base64-encoded image,
/// which would otherwise be Debug-dumped into every span on this hot path.
#[tracing::instrument(fields(job_id), skip(artifact_store, event))]
pub async fn handle_image_ready(
    artifact_store: Arc<ArtifactStore>,
    event: WorkerEvent,
    job_id: Uuid,
) -> Result<String, AnvilError> {
    // Log at DEBUG that we are processing this event. The tracing::instrument
    // macro already creates a span with job_id, so this log entry will be
    // nested under that span.
    tracing::debug!(job_id = %job_id, "processing ImageReady event");

    // Extract the ImageReady fields from the event. The caller should have
    // already verified this is ImageReady, but we match here to extract the
    // fields and provide a clear error if the event is unexpected.
    let WorkerEvent::ImageReady {
        image_b64,
        width,
        height,
        format: _, // format is recorded in the event but not stored in ArtifactMeta
        seed,
        steps,
        job_id: _, // job_id is passed separately as a parameter — ignore the event's own copy
    } = event
    else {
        // This should never happen if the caller follows the contract of
        // matching on ImageReady before calling this function. However,
        // we return a clear error rather than panicking.
        return Err(AnvilError::Serde(
            "expected ImageReady event, got different variant".into(),
        ));
    };

    // Decode the base64-encoded image payload. The STANDARD engine uses
    // the standard base64 alphabet (A-Z, a-z, 0-9, +, /) with padding (=).
    // This matches the encoding used by the Python worker.
    let png_bytes = STANDARD
        .decode(&image_b64)
        .map_err(|err| AnvilError::Serde(format!("base64 decode failed: {err}")))?;

    // Construct the ArtifactMeta for this artifact. The `hash` and `file_path`
    // fields are ignored by `save()` — it computes both from the PNG bytes:
    // hash is SHA-256 of the bytes, file_path is {artifact_dir}/{hash}.png.
    // The remaining fields (job_id, width, height, seed, steps) are persisted
    // in the database and are used for querying artifacts by generation params.
    let meta = ArtifactMeta {
        hash: String::new(), // ignored by save() — computed from bytes
        job_id,
        width,
        height,
        seed,
        steps,
        created_at: Utc::now(),
        file_path: PathBuf::from(""), // ignored by save() — computed from dir + hash
    };

    // Save the decoded PNG bytes to the artifact store. This computes the
    // SHA-256 hash, writes the file, and persists the metadata row. Returns
    // the computed hash on success. The save() method is idempotent — calling
    // it twice with the same bytes returns the same hash without duplicating.
    let hash = artifact_store.save(&png_bytes, &meta).await?;

    // Log the successful artifact save with structured fields for log aggregation.
    tracing::info!(
        job_id = %job_id,
        hash = %hash,
        width = width,
        height = height,
        "artifact saved from ImageReady"
    );

    Ok(hash)
}

/// Map a `WorkerEvent` to its corresponding `WsEvent` variant for WebSocket broadcasting.
///
/// Performs a one-to-one mapping between the internal worker event types and the
/// WebSocket event types that clients subscribe to. Not every `WorkerEvent` variant
/// has a corresponding `WsEvent` — `Ready`, `Pong`, `Dying`, and `MemoryReport` are
/// handled by other subsystems (node registry, keepalive watchdog, worker pool).
///
/// # Panics
///
/// This function panics if called with a `WorkerEvent` variant that has no
/// `WsEvent` mapping. This should never happen in normal operation because
/// the caller (`spawn_event_loop`) routes `Ready`, `Pong`, `Dying`, and
/// `MemoryReport` through separate paths before reaching this function.
///
/// If a new `WorkerEvent` variant is added to `messages.rs` without updating
/// this function, the compiler will produce a non-exhaustive match error
/// (since `WorkerEvent` is not marked `#[non_exhaustive]`), which is desirable
/// — it forces the mapping to be updated.
///
/// # Arguments
///
/// * `event` — The worker event to map. Must be one of `Progress`, `Completed`,
///   `Failed`, `Cancelled`, or `ImageReady`.
///
/// # Returns
///
/// The corresponding `WsEvent` variant.
pub fn map_worker_event(event: WorkerEvent) -> WsEvent {
    match event {
        WorkerEvent::Progress {
            job_id,
            step,
            total_steps,
            preview_b64,
        } => WsEvent::JobProgress {
            job_id,
            step,
            total_steps,
            preview_b64,
        },
        WorkerEvent::Completed { job_id, elapsed_ms } => {
            WsEvent::JobCompleted { job_id, elapsed_ms }
        }
        WorkerEvent::Failed {
            job_id,
            error,
            traceback: _,
        } => {
            // The traceback field is omitted from WsEvent::JobFailed per the
            // type definition — only the human-readable error string is
            // broadcast to WebSocket clients. The full traceback is logged
            // separately by the caller.
            WsEvent::JobFailed { job_id, error }
        }
        WorkerEvent::Cancelled { job_id } => WsEvent::JobCancelled { job_id },
        WorkerEvent::ImageReady {
            job_id,
            image_b64: _,
            width,
            height,
            format: _,
            seed,
            steps,
        } => {
            // ImageReady is handled specially in spawn_event_loop —
            // this branch should never be reached via the mapping path
            // because the event loop routes ImageReady through the artifact
            // save path before publishing. This arm exists only because
            // map_worker_event must be exhaustive over all WorkerEvent
            // variants.
            WsEvent::JobImageReady {
                job_id,
                artifact_hash: String::new(),
                width,
                height,
                seed,
                steps,
            }
        }
        // Events below are handled by other subsystems and should never reach
        // this mapping function. The panic arms force a compile-time update
        // if new WorkerEvent variants are added without updating this code.
        WorkerEvent::Ready { .. } => {
            panic!(
                "Ready events are handled by the node registry and the hardware \
                 snapshot update in spawn_event_loop(), not map_worker_event()"
            )
        }
        WorkerEvent::Pong { .. } => {
            panic!("Pong events are handled by the keepalive watchdog, not the event loop")
        }
        WorkerEvent::Dying { .. } => {
            panic!("Dying events are handled by the worker pool, not the event loop")
        }
        WorkerEvent::MemoryReport { .. } => {
            panic!("MemoryReport events are handled by the worker pool, not the event loop")
        }
    }
}

/// Apply a `WorkerEvent::Ready` event's runtime-probed capabilities to the
/// shared `HardwareInfo` snapshot that `GET /v1/system` serves.
///
/// # Why this exists
///
/// `AppState.hardware` is populated exactly once, at server startup, by
/// `detect_all_devices()` (`anvilml-hardware`) — a pre-spawn hint sourced
/// from either the PCI-ID device table or a CPU/fallback default. Per
/// `ANVILML_DESIGN.md §6.6`, that hint is never authoritative: the Python
/// worker's own runtime torch probe, delivered in its `Ready` event, is the
/// only source permitted to make a compute-dtype decision. Before this
/// function existed, nothing ever consumed that authoritative data — the
/// `Ready` event's capability fields were received, then discarded, leaving
/// `/v1/system` reporting stale pre-spawn hints (frequently all-`false` for
/// any device absent from the seed table) for the server's entire uptime.
///
/// P900-series retrofit: the same "received, then discarded" gap applied to
/// `vram_total_mib`/`vram_free_mib` — every hardware detector
/// (`vulkan.rs`/`dxgi.rs`/`sysfs.rs`/`cpu.rs`) sets these to `0` at
/// `detect()` time with a "filled by refresh_vram later" comment, but
/// nothing ever called `refresh_vram()` anywhere in the codebase. Every
/// device — GPU and CPU alike — therefore reported `vram_free_mib: 0`
/// forever, which made `anvilml-scheduler`'s VRAM-descending worker
/// selection a tie between every GPU and the CPU fallback; Rust's
/// `Iterator::max_by_key` breaks ties by returning the *last* equally-
/// maximal element, and `detect_all_devices()` always appends the CPU
/// device after every GPU device, so the tie was resolved in the CPU's
/// favor on every single dispatch. The `Ready` event already carries the
/// worker's own real, torch-probed `vram_total_mib`/`vram_free_mib`
/// (`worker_main.py` calls `torch.cuda.mem_get_info()` — this also covers
/// ROCm, since ROCm builds of torch alias the `torch.cuda.*` namespace) —
/// this function now applies those fields the same way it already applies
/// `caps`, rather than adding a separate (and currently entirely
/// unwired) `refresh_vram()` call path.
///
/// # Behavior
///
/// Finds the `GpuDevice` in `hardware.gpus` whose `index` matches the
/// event's `device_index` (the same convention `WorkerPool::spawn_worker()`
/// uses to derive `worker_id` from `device.index` — see that method's own
/// doc comment), overwrites its `caps` with the probed values and its
/// `capabilities_source` with `CapabilitySource::PyTorch`, overwrites its
/// `vram_total_mib`/`vram_free_mib` with the probed values, then recomputes
/// the top-level `inference_caps` field as the OR-union of every device's
/// caps via `anvilml_hardware::compute_caps_union()` — the same function
/// `detect_all_devices()` itself uses, so the aggregate is computed
/// identically whether it comes from the initial startup snapshot or a
/// later `Ready`-driven update.
///
/// No matching device (`device_index` outside `hardware.gpus`) is logged at
/// WARN and otherwise ignored — this should not happen given the
/// `worker_id`/`device.index` convention above, but a malformed or
/// out-of-band event must not panic the event loop.
async fn apply_ready_capabilities(
    hardware: &RwLock<HardwareInfo>,
    device_index: u32,
    caps: InferenceCaps,
    vram_total_mib: u32,
    vram_free_mib: u32,
) {
    let mut hw = hardware.write().await;
    match hw.gpus.iter_mut().find(|gpu| gpu.index == device_index) {
        Some(gpu) => {
            // Log before moving `caps` into `gpu.caps` below — InferenceCaps
            // isn't Copy, and logging the bool fields here borrows them
            // rather than consuming the value.
            tracing::info!(
                device_index,
                fp32 = caps.fp32,
                fp16 = caps.fp16,
                bf16 = caps.bf16,
                fp8 = caps.fp8,
                fp4 = caps.fp4,
                flash_attention = caps.flash_attention,
                vram_total_mib,
                vram_free_mib,
                "hardware_caps_updated_from_ready"
            );

            gpu.caps = caps;
            gpu.capabilities_source = CapabilitySource::PyTorch;
            gpu.vram_total_mib = vram_total_mib;
            gpu.vram_free_mib = vram_free_mib;

            hw.inference_caps = compute_caps_union(&hw.gpus);
        }
        None => {
            tracing::warn!(
                device_index,
                "event_loop: Ready event's device_index has no matching \
                 GpuDevice in the hardware snapshot — capabilities not applied"
            );
        }
    }
}

/// Spawn the event loop task that consumes `WorkerEvent`s via a `Demux`
/// subscription and broadcasts them as `WsEvent`s to WebSocket subscribers.
///
/// The event loop runs an infinite loop that:
/// 1. Receives `(worker_id, WorkerEvent)` pairs from its own `Demux::subscribe()`
///    subscription (blocking until one arrives). It never calls
///    `RouterTransport::recv()` itself — see this module's own doc comment and
///    `ANVILML_DESIGN.md §9.8` / `docs/ADDENDUM_DEMUX_FANOUT.md` for why a second
///    direct caller on the pool's shared ROUTER socket would race `bridge.rs`'s
///    `reader_task` for incoming frames.
/// 2. Routes `ImageReady` events through the artifact save path, then publishes
///    `JobImageReady` **after** the save succeeds (never before).
/// 3. Routes all other mapped events through `map_worker_event()` and publishes.
/// 4. Logs a `DEBUG` transition record after each publish per `ANVILML_DESIGN.md §16.3`.
///
/// If the subscription channel closes (every `Sender` for it dropped — i.e. the
/// `Demux`, and with it the whole `WorkerPool`, is gone), the loop logs an `ERROR`
/// and exits. Unlike a single transient IPC hiccup, this is an unrecoverable,
/// terminal condition for this task: there is no transport-level error to retry,
/// since fan-out delivery itself has already permanently ended.
///
/// On each terminal event (`Completed`/`Failed`/`Cancelled`), the event loop also
/// restores the responsible worker's status to `Idle` and wakes the dispatch loop
/// (per `ANVILML_DESIGN.md §12.5`) so that queued jobs waiting for a free worker
/// are re-evaluated.
///
/// The `Demux` subscription is established synchronously, before this function
/// spawns the task and returns — not inside the spawned task itself. Any
/// `WorkerEvent` routed after this function returns is guaranteed to reach this
/// subscription; there is no window where an event routed immediately after
/// `spawn_event_loop()` returns could be missed because the task hadn't been
/// scheduled yet.
///
/// # Arguments
///
/// * `scheduler` — The `JobScheduler` (owned via `Arc`, consumed by the spawned task).
///   The scheduler's `artifact_store` field is used for `ImageReady` artifact saves.
/// * `demux` — The pool-wide `Demux` to subscribe to for `WorkerEvent`s. Callers
///   should pass `workers.demux()` — the same instance `bridge.rs`'s
///   `reader_task` already routes every real worker event into.
/// * `broadcaster` — The WebSocket event broadcaster for publishing `WsEvent`s.
/// * `workers` — The worker pool, used to restore the responsible worker to `Idle`
///   on terminal events and wake the dispatch loop.
/// * `hardware` — The shared hardware snapshot `GET /v1/system` serves. Callers
///   should pass the same `Arc<RwLock<HardwareInfo>>` instance stored in
///   `AppState.hardware`. On every `Ready` event, `apply_ready_capabilities()`
///   writes that worker's runtime-probed capabilities into the matching
///   `GpuDevice` entry and recomputes the aggregate `inference_caps` — see that
///   function's own doc comment for why this update exists.
///
/// # Returns
///
/// A `JoinHandle<()>` for the spawned event loop task.
#[tracing::instrument(skip(scheduler, demux, broadcaster, workers, hardware))]
pub fn spawn_event_loop(
    scheduler: Arc<JobScheduler>,
    demux: Arc<Demux>,
    broadcaster: Arc<EventBroadcaster>,
    workers: Arc<WorkerPool>,
    hardware: Arc<RwLock<HardwareInfo>>,
) -> JoinHandle<()> {
    // Subscribe synchronously, before spawning the task — not inside the
    // spawned `async move` block. `tokio::spawn()` only schedules the task;
    // it does not run any of its body before returning the JoinHandle. If
    // `demux.subscribe()` happened inside the spawned block instead, any
    // `WorkerEvent` routed between this function returning and the spawned
    // task actually getting scheduled would be silently missed — the
    // subscription simply wouldn't exist yet when `route()`'s fan-out ran.
    // Subscribing here means the subscription is guaranteed to exist by the
    // time this function returns to its caller, closing that window
    // entirely rather than narrowing it with a sleep.
    let (subscription_id, mut events) = demux.subscribe();

    tokio::spawn(async move {
        loop {
            // Receive the next fanned-out (worker_id, WorkerEvent) pair.
            // Blocks until one arrives, or resolves to None once every
            // Sender for this subscription has been dropped — an
            // unrecoverable condition (see this function's own doc comment),
            // not a transient error to retry past.
            let (_worker_id, event) = match events.recv().await {
                Some(pair) => pair,
                None => {
                    tracing::error!(
                        subscription_id,
                        "event_loop: Demux subscription closed, exiting"
                    );
                    return;
                }
            };

            // Route ImageReady through the artifact save path, then publish.
            // All other event types go through the generic mapping path.
            match event {
                WorkerEvent::ImageReady {
                    job_id,
                    width,
                    height,
                    seed,
                    steps,
                    ..
                } => {
                    // Save the artifact first — this is the critical ordering
                    // requirement: JobImageReady must only be published AFTER
                    // the save succeeds, never before. The width/height/seed/steps
                    // fields are extracted here before calling handle_image_ready()
                    // because that function consumes the event by value and doesn't
                    // return the individual fields.
                    let job_id_for_log = job_id;
                    match handle_image_ready(scheduler.artifact_store.clone(), event, job_id).await
                    {
                        Ok(hash) => {
                            let ws_event = WsEvent::JobImageReady {
                                job_id,
                                artifact_hash: hash,
                                width,
                                height,
                                seed,
                                steps,
                            };
                            broadcaster.publish(ws_event);
                            tracing::debug!(
                                job_id = %job_id_for_log,
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
                                job_id = %job_id_for_log,
                                error = %e,
                                "event_loop image_ready save failed"
                            );
                        }
                    }
                }
                // Terminal events: persist status transition and release VRAM
                // reservation before publishing. Each arm follows the same
                // pattern: look up the job (to get worker_id for VRAM lookup),
                // release the ledger reservation, update the database, then
                // publish the mapped WsEvent.
                //
                // Non-terminal events (Progress) and the events that should
                // never reach this point (Ready, Pong, Dying, MemoryReport)
                // fall through to the catch-all below.
                WorkerEvent::Completed { job_id, elapsed_ms } => {
                    // Look up the job to get its worker_id. The worker_id
                    // was set during dispatch_one() when the job was assigned
                    // to a worker and transitioned to Running.
                    let job = match scheduler.get_job(job_id).await {
                        Ok(Some(job)) => job,
                        Ok(None) => {
                            // Job not found in the database — log a warning
                            // but still publish the event so the WebSocket
                            // client sees the transition.
                            tracing::warn!(
                                job_id = %job_id,
                                "event_loop: Completed event for unknown job"
                            );
                            broadcaster.publish(WsEvent::JobCompleted { job_id, elapsed_ms });
                            tracing::debug!(
                                job_id = %job_id,
                                from = "Completed",
                                to = "JobCompleted",
                                "event transition"
                            );
                            continue;
                        }
                        Err(e) => {
                            tracing::error!(
                                job_id = %job_id,
                                error = %e,
                                "event_loop: failed to fetch job for Completed"
                            );
                            broadcaster.publish(WsEvent::JobCompleted { job_id, elapsed_ms });
                            continue;
                        }
                    };

                    // Release the VRAM reservation. The worker_id encodes the
                    // device index (bare device index as string, e.g. "0").
                    // We parse it to look up the reservation amount from the ledger.
                    let worker_id = job.worker_id.clone().unwrap_or_default();
                    let device_index = worker_id.parse::<u32>().unwrap_or(0);
                    let vram_mib = scheduler.get_reservation(device_index).await;
                    if vram_mib > 0 {
                        scheduler.release_reservation(device_index, vram_mib).await;
                    }

                    // Persist the terminal status.
                    scheduler
                        .update_job_terminal_status(job_id, JobStatus::Completed, None)
                        .await;

                    // Restore the worker to Idle and wake the dispatch loop.
                    // P14-A5 marks the worker Busy on dispatch but nothing reversed
                    // that until this task. Without this, a worker that finishes a
                    // job stays Busy forever, and queued jobs would never be
                    // re-evaluated since dispatch_notify is only woken by submit()
                    // — a starvation bug under multi-job load.
                    //
                    // Find the worker handle by matching job.worker_id against the
                    // pool's handles — the worker_id is the bare device index as a
                    // string (e.g. "0"), matching the convention in
                    // ANVILML_DESIGN.md §12.5.
                    let worker_id = job.worker_id.clone().unwrap_or_default();
                    if let Some(handle) =
                        workers.handles().iter().find(|h| h.worker_id == worker_id)
                    {
                        handle.set_status(WorkerStatus::Idle).await;
                        tracing::debug!(worker_id = %worker_id, "event_loop_worker_restored_idle");
                    } else {
                        tracing::warn!(
                            worker_id = %worker_id,
                            "event_loop: no worker handle found for Completed — status not restored"
                        );
                    }
                    scheduler.wake_dispatch();

                    // Publish the mapped WsEvent.
                    broadcaster.publish(WsEvent::JobCompleted { job_id, elapsed_ms });
                    tracing::debug!(
                        job_id = %job_id,
                        from = "Completed",
                        to = "JobCompleted",
                        "event transition"
                    );
                }
                WorkerEvent::Failed {
                    job_id,
                    error,
                    traceback: _,
                } => {
                    // Look up the job to get its worker_id for VRAM release.
                    let job = match scheduler.get_job(job_id).await {
                        Ok(Some(job)) => job,
                        Ok(None) => {
                            tracing::warn!(
                                job_id = %job_id,
                                "event_loop: Failed event for unknown job"
                            );
                            broadcaster.publish(WsEvent::JobFailed { job_id, error });
                            tracing::debug!(
                                job_id = %job_id,
                                from = "Failed",
                                to = "JobFailed",
                                "event transition"
                            );
                            continue;
                        }
                        Err(e) => {
                            tracing::error!(
                                job_id = %job_id,
                                error = %e,
                                "event_loop: failed to fetch job for Failed"
                            );
                            broadcaster.publish(WsEvent::JobFailed { job_id, error });
                            continue;
                        }
                    };

                    let worker_id = job.worker_id.clone().unwrap_or_default();
                    let device_index = worker_id.parse::<u32>().unwrap_or(0);
                    let vram_mib = scheduler.get_reservation(device_index).await;
                    if vram_mib > 0 {
                        scheduler.release_reservation(device_index, vram_mib).await;
                    }

                    // Persist the terminal status with the error string.
                    // Clone the error for the WsEvent publish since update_job_terminal_status
                    // consumes it — the error string is the same in both the DB and the broadcast.
                    scheduler
                        .update_job_terminal_status(job_id, JobStatus::Failed, Some(error.clone()))
                        .await;

                    // Restore the worker to Idle and wake the dispatch loop.
                    // Same rationale as the Completed arm above — P14-A5 marks the
                    // worker Busy on dispatch; this task reverses that so queued jobs
                    // can be re-evaluated.
                    let worker_id = job.worker_id.clone().unwrap_or_default();
                    if let Some(handle) =
                        workers.handles().iter().find(|h| h.worker_id == worker_id)
                    {
                        handle.set_status(WorkerStatus::Idle).await;
                        tracing::debug!(worker_id = %worker_id, "event_loop_worker_restored_idle");
                    } else {
                        tracing::warn!(
                            worker_id = %worker_id,
                            "event_loop: no worker handle found for Failed — status not restored"
                        );
                    }
                    scheduler.wake_dispatch();

                    broadcaster.publish(WsEvent::JobFailed { job_id, error });
                    tracing::debug!(
                        job_id = %job_id,
                        from = "Failed",
                        to = "JobFailed",
                        "event transition"
                    );
                }
                WorkerEvent::Cancelled { job_id } => {
                    // Look up the job to get its worker_id for VRAM release.
                    let job = match scheduler.get_job(job_id).await {
                        Ok(Some(job)) => job,
                        Ok(None) => {
                            tracing::warn!(
                                job_id = %job_id,
                                "event_loop: Cancelled event for unknown job"
                            );
                            broadcaster.publish(WsEvent::JobCancelled { job_id });
                            tracing::debug!(
                                job_id = %job_id,
                                from = "Cancelled",
                                to = "JobCancelled",
                                "event transition"
                            );
                            continue;
                        }
                        Err(e) => {
                            tracing::error!(
                                job_id = %job_id,
                                error = %e,
                                "event_loop: failed to fetch job for Cancelled"
                            );
                            broadcaster.publish(WsEvent::JobCancelled { job_id });
                            continue;
                        }
                    };

                    let worker_id = job.worker_id.clone().unwrap_or_default();
                    let device_index = worker_id.parse::<u32>().unwrap_or(0);
                    let vram_mib = scheduler.get_reservation(device_index).await;
                    if vram_mib > 0 {
                        scheduler.release_reservation(device_index, vram_mib).await;
                    }

                    // Persist the terminal status.
                    scheduler
                        .update_job_terminal_status(job_id, JobStatus::Cancelled, None)
                        .await;

                    // Restore the worker to Idle and wake the dispatch loop.
                    // Same rationale as the Completed arm above — P14-A5 marks the
                    // worker Busy on dispatch; this task reverses that so queued jobs
                    // can be re-evaluated.
                    let worker_id = job.worker_id.clone().unwrap_or_default();
                    if let Some(handle) =
                        workers.handles().iter().find(|h| h.worker_id == worker_id)
                    {
                        handle.set_status(WorkerStatus::Idle).await;
                        tracing::debug!(worker_id = %worker_id, "event_loop_worker_restored_idle");
                    } else {
                        tracing::warn!(
                            worker_id = %worker_id,
                            "event_loop: no worker handle found for Cancelled — status not restored"
                        );
                    }
                    scheduler.wake_dispatch();

                    broadcaster.publish(WsEvent::JobCancelled { job_id });
                    tracing::debug!(
                        job_id = %job_id,
                        from = "Cancelled",
                        to = "JobCancelled",
                        "event transition"
                    );
                }
                // Progress events go through the generic mapping path.
                // Ready, Pong, Dying, and MemoryReport are handled by
                // separate subsystems (node registry, keepalive watchdog,
                // worker pool) and are skipped here — the Demux fans out
                // ALL events to all subscribers, but only Progress needs
                // to be broadcast to WebSocket clients.
                WorkerEvent::Progress {
                    job_id,
                    step,
                    total_steps,
                    preview_b64,
                } => {
                    let ws_event = WsEvent::JobProgress {
                        job_id,
                        step,
                        total_steps,
                        preview_b64,
                    };

                    // Log the state transition per ANVILML_DESIGN.md §16.3.
                    // The "from" is the WorkerEvent variant name, "to" is the
                    // WsEvent variant name. Extract these before publishing
                    // since publish() takes ownership of ws_event.
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

                    // Publish after extracting variant info — publish() takes
                    // ownership of the event.
                    broadcaster.publish(ws_event);

                    if let Some(jid) = job_id {
                        tracing::debug!(
                            job_id = %jid,
                            from = from_variant,
                            to = to_variant,
                            "event transition"
                        );
                    }
                }
                // Ready carries the worker's own runtime-probed capabilities
                // (per ANVILML_DESIGN.md §6.6, the only authoritative source
                // for them) — apply them to the shared hardware snapshot, but
                // do not broadcast anything to WebSocket clients. Node-type
                // registration for the same event is handled independently,
                // over the per-worker demux channel, by
                // `ManagedWorker::handle_event()` — this fan-out subscription
                // sees an independent copy of the same event and does not
                // race or duplicate that registration.
                WorkerEvent::Ready {
                    device_index,
                    vram_total_mib,
                    vram_free_mib,
                    fp32,
                    fp16,
                    bf16,
                    fp8,
                    fp4,
                    flash_attention,
                    ..
                } => {
                    let caps = InferenceCaps {
                        fp32,
                        fp16,
                        bf16,
                        fp8,
                        fp4,
                        flash_attention,
                    };
                    apply_ready_capabilities(
                        &hardware,
                        device_index,
                        caps,
                        vram_total_mib,
                        vram_free_mib,
                    )
                    .await;
                }
                // Pong, Dying, and MemoryReport are handled by other
                // subsystems (keepalive watchdog, worker pool) and must not
                // be broadcast to WebSocket clients. The Demux fans out ALL
                // events to all subscribers, so we must explicitly skip
                // these here.
                WorkerEvent::Pong { .. }
                | WorkerEvent::Dying { .. }
                | WorkerEvent::MemoryReport { .. } => {
                    tracing::debug!("skipping non-broadcast event (handled by other subsystem)");
                }
            }
        }
    })
}
