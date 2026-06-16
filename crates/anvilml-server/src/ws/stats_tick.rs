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
/// * `pool` — An `Arc<WorkerPool>` providing access to the shared
///   `EventBroadcaster` and the current worker info snapshot.
pub fn start(pool: Arc<anvilml_worker::WorkerPool>) {
    // Spawn the tick loop as a detached tokio task.
    // The JoinHandle is intentionally dropped — the task runs for the
    // lifetime of the server and does not need to be polled or cancelled.
    tokio::spawn(async move {
        // The tick interval is hardcoded to 5 seconds per the task spec.
        // A configurable interval is deferred to a future task.
        let interval = Duration::from_secs(5);

        // Allocate the System instance once and reuse it across ticks.
        // sysinfo computes CPU utilisation as the delta between the last
        // two refresh_cpu_usage() calls. Keeping the instance alive across
        // the 5-second sleep means the delta is measured over 5 seconds,
        // matching the window used by Task Manager. Re-creating System each
        // tick collapses the measurement window to microseconds, causing
        // the OS scheduler noise to dominate and producing inflated readings.
        let mut sys = System::new_all();

        // Perform an initial refresh and discard the reading. The first
        // global_cpu_usage() after construction computes a delta against
        // an undefined prior timestamp (the cold-start sample taken inside
        // new_all()), so it is inaccurate and must not be broadcast.
        // Sleeping the full interval here establishes a valid baseline
        // before the loop begins.
        sys.refresh_cpu_usage();
        sleep(interval).await;

        loop {
            // Refresh only CPU usage and memory — not processes, disks, or
            // networks. This keeps the per-tick refresh cost minimal while
            // providing the two values needed for SystemStats.
            sys.refresh_cpu_usage();
            sys.refresh_memory();

            // Read CPU utilisation. global_cpu_usage() returns an f32
            // representing total CPU utilisation across all cores as a
            // percentage (0.0–100.0). The delta is computed against the
            // previous refresh_cpu_usage() call, which is 5 seconds ago
            // due to the sleep below — producing an accurate reading.
            // This matches the WsEvent::SystemStats field type directly.
            let cpu_pct = sys.global_cpu_usage();

            // Read used RAM and convert from bytes to mebibytes.
            // sysinfo reports memory in bytes; dividing by 1024*1024 gives
            // mebibytes. This matches the conversion pattern in
            // anvilml-hardware/src/cpu.rs for total_memory().
            // used_memory() returns u64, so the division is always exact
            // and non-negative.
            let ram_used_mib = sys.used_memory() / (1024 * 1024);

            // Get the worker info snapshot from the pool.
            // This populates the workers field in SystemStats with the
            // current state of all managed workers.
            let workers = pool.get_worker_infos().await;

            // Build the SystemStats event with the worker snapshot.
            let event = WsEvent::SystemStats {
                cpu_pct,
                ram_used_mib,
                workers,
            };

            // Broadcast the event to all connected WebSocket clients.
            // If all receivers are lagging, EventBroadcaster::send() logs
            // a WARN and drops the event — the tick loop continues unaffected.
            pool.broadcaster().send(event);

            // Log the tick at TRACE level for diagnostic purposes.
            // This is a routine per-tick log point (not mandatory per
            // ENVIRONMENT.md §9) but useful for verifying the tick task
            // is running correctly during development and debugging.
            tracing::trace!(cpu_pct, ram_used_mib, "system stats tick");

            // Sleep for the tick interval before the next measurement.
            // This sleep is also the measurement window for the next
            // refresh_cpu_usage() call — its delta spans exactly this duration.
            sleep(interval).await;
        }
    });
}
