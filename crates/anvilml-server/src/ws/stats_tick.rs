//! System stats tick — broadcasts periodic CPU and memory metrics.
//!
//! This module spawns a background tokio task that runs an infinite loop,
//! collecting system metrics every 5 seconds and broadcasting them as
//! `WsEvent::SystemStats` events via the shared `EventBroadcaster`.
//!
//! The tick task is fire-and-forget: its `JoinHandle` is dropped after
//! spawning, and the task runs until the broadcaster is shut down with
//! the server. If all receivers lag behind, `EventBroadcaster::send()`
//! internally logs a WARN and drops the event — the tick loop continues
//! unaffected.

use std::sync::Arc;
use std::time::Duration;

use anvilml_core::types::WsEvent;
use sysinfo::System;
use tokio::time::sleep;

use crate::ws::EventBroadcaster;

/// Start the system stats background tick task.
///
/// Spawns a tokio task that loops indefinitely, collecting CPU utilisation
/// and RAM usage every 5 seconds and broadcasting them as `WsEvent::SystemStats`
/// events. The task runs until the broadcaster is shut down with the server.
///
/// The `JoinHandle` is dropped after spawning — this is intentional. The tick
/// task is fire-and-forget; it runs for the lifetime of the server process
/// and does not need to be awaited or cancelled explicitly.
///
/// # Arguments
///
/// * `broadcaster` — An `Arc<EventBroadcaster>` shared across the server.
///   The arc is cloned into the spawned task so the original can continue
///   being used by HTTP handlers.
pub fn start(broadcaster: Arc<EventBroadcaster>) {
    // Spawn the tick loop as a detached tokio task.
    // The JoinHandle is intentionally dropped — the task runs for the
    // lifetime of the server and does not need to be polled or cancelled.
    tokio::spawn(async move {
        // The tick interval is hardcoded to 5 seconds per the task spec.
        // A configurable interval is deferred to a future task.
        let interval = Duration::from_secs(5);

        loop {
            // Sleep before the first tick to avoid reading stale data
            // immediately after server start. The first `global_cpu_usage()`
            // call is known to be inaccurate (cold-start artifact per sysinfo
            // docs), so the 5-second initial delay gives the OS time to
            // establish a baseline measurement.
            sleep(interval).await;

            // Create a fresh System snapshot.
            // System::new_all() is equivalent to System::new() + refresh_all(),
            // which collects CPU, memory, and process info in one call.
            // This matches the pattern already used in anvilml-hardware/src/cpu.rs.
            let mut sys = System::new_all();
            sys.refresh_all();

            // Read CPU utilisation. global_cpu_usage() returns an f32
            // representing total CPU utilisation across all cores as a
            // percentage (0.0–N*100 where N is core count). This matches
            // the WsEvent::SystemStats field type directly.
            // The first reading after boot may be inaccurate (cold-start
            // artifact), but subsequent readings are accurate.
            let cpu_pct = sys.global_cpu_usage();

            // Read used RAM and convert from bytes to mebibytes.
            // sysinfo reports memory in bytes; dividing by 1024*1024 gives
            // mebibytes. This matches the conversion pattern in
            // anvilml-hardware/src/cpu.rs for total_memory().
            // used_memory() returns u64, so the division is u64/u64 = u64,
            // which is always non-negative.
            let ram_used_mib = sys.used_memory() / (1024 * 1024);

            // Build the SystemStats event with an empty workers vec.
            // The workers array is populated by a future task (Phase 009)
            // when the WorkerPool exists.
            let event = WsEvent::SystemStats {
                cpu_pct,
                ram_used_mib,
                workers: Vec::new(),
            };

            // Broadcast the event to all connected WebSocket clients.
            // If all receivers are lagging, EventBroadcaster::send() logs
            // a WARN and drops the event — the tick loop continues unaffected.
            broadcaster.send(event);

            // Log the tick at DEBUG level for diagnostic purposes.
            // This is a routine per-tick log point (not mandatory per
            // ENVIRONMENT.md §9) but useful for verifying the tick task
            // is running correctly during development and debugging.
            tracing::debug!(cpu_pct, ram_used_mib, "system stats tick");
        }
    });
}
