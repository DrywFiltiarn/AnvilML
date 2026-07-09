/// Event loop module for processing `WorkerEvent` variants from workers.
///
/// This module owns:
/// - `handle_image_ready()` — decodes the base64 image payload, constructs an
///   `ArtifactMeta`, and calls `artifact_store.save()` to persist the decoded PNG
///   bytes under their content hash.
/// - `spawn_event_loop()` — subscribes the scheduler to `WorkerEvent`s from workers
///   via the `RouterTransport`, maps each event to its `WsEvent` counterpart, and
///   publishes it via the shared `EventBroadcaster`.
/// - `map_worker_event()` — performs the one-to-one mapping between `WorkerEvent`
///   and `WsEvent` variants.
///
/// The design separates the event variant matching from the handler function so that
/// callers (the interim job completion listener, or the event loop) can pattern-match
/// on the event to extract the variant before calling the handler. This avoids forcing
/// the caller to destructure the event.
use std::path::PathBuf;
use std::sync::Arc;

use crate::JobScheduler;
use anvilml_artifacts::ArtifactStore;
use anvilml_core::{AnvilError, ArtifactMeta, JobStatus, WsEvent};
use anvilml_ipc::{EventBroadcaster, RouterTransport, WorkerEvent};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use chrono::Utc;
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
#[tracing::instrument(fields(job_id), skip(artifact_store))]
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
            panic!("Ready events are handled by the node registry, not the event loop")
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

/// Spawn the event loop task that consumes `WorkerEvent`s from the transport and
/// broadcasts them as `WsEvent`s to WebSocket subscribers.
///
/// The event loop runs an infinite loop that:
/// 1. Receives events from the `RouterTransport` (blocking until a message arrives).
/// 2. Routes `ImageReady` events through the artifact save path, then publishes
///    `JobImageReady` **after** the save succeeds (never before).
/// 3. Routes all other mapped events through `map_worker_event()` and publishes.
/// 4. Logs a `DEBUG` transition record after each publish per `ANVILML_DESIGN.md §16.3`.
///
/// On transport `recv()` errors (e.g. closed socket), the loop logs the error and
/// retries on the next iteration — it does not abort.
///
/// # Arguments
///
/// * `self` — The `JobScheduler` (owned via `Arc`, consumed by the spawned task).
///   The scheduler's `artifact_store` field is used for `ImageReady` artifact saves.
/// * `transport` — The ZeroMQ ROUTER transport for receiving worker events.
/// * `broadcaster` — The WebSocket event broadcaster for publishing `WsEvent`s.
///
/// # Returns
///
/// A `JoinHandle<()>` for the spawned event loop task.
#[tracing::instrument(skip(scheduler, transport, broadcaster))]
pub fn spawn_event_loop(
    scheduler: Arc<JobScheduler>,
    transport: Arc<RouterTransport>,
    broadcaster: Arc<EventBroadcaster>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            // Receive the next event from the worker. This blocks until a
            // message arrives on the ROUTER socket. The recv() method returns
            // (worker_identity, WorkerEvent).
            let (_worker_id, event) = match transport.recv().await {
                Ok(pair) => pair,
                Err(e) => {
                    // Transport error (e.g. closed socket). Log and retry on
                    // the next iteration — the loop continues indefinitely
                    // rather than aborting the event processing pipeline.
                    tracing::error!(error = %e, "event_loop recv error, retrying");
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    continue;
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

                    broadcaster.publish(WsEvent::JobCancelled { job_id });
                    tracing::debug!(
                        job_id = %job_id,
                        from = "Cancelled",
                        to = "JobCancelled",
                        "event transition"
                    );
                }
                // All other events (Progress, and the panic-gated Ready/Pong/
                // Dying/MemoryReport) go through the generic mapping path.
                _ => {
                    let ws_event = map_worker_event(event);

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
            }
        }
    })
}
