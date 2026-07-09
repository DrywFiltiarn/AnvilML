//! Periodic `SystemStats` background task.
//!
//! Per `ANVILML_DESIGN.md §13.1`/§13.6: independently of any individual
//! WebSocket connection's own initial frame (`ws/handler.rs`, `P16-C1`),
//! the server publishes a fresh `WsEvent::SystemStats` to every connected
//! client at a fixed cadence, so an otherwise-idle connection still sees
//! periodic host-level telemetry rather than only job-related events.
//! This module does not touch `ws/handler.rs`'s own per-connection initial
//! frame — that remains the placeholder `P16-C1` established; this task's
//! scope is strictly the recurring broadcast to already-subscribed clients.

use std::sync::Arc;
use std::time::Duration;

use anvilml_core::WsEvent;
use anvilml_ipc::EventBroadcaster;
use anvilml_worker::WorkerPool;
use sysinfo::System;
use tokio::task::JoinHandle;
use tokio::time::MissedTickBehavior;

/// Spawn the background task that publishes `WsEvent::SystemStats` to
/// `broadcaster` every `interval`.
///
/// `interval` is a constructor parameter, not a hardcoded
/// `Duration::from_secs(5)` literal — the same testability pattern Phase
/// 8's `keepalive.rs` established for its injected `ping_interval`
/// (`ANVILML_DESIGN.md §9.2`), letting tests use millisecond-scale
/// durations instead of waiting real seconds. Production callers
/// (`backend/src/main.rs`) pass `Duration::from_secs(5)` per
/// `ANVILML_DESIGN.md §13.1`.
///
/// The returned `JoinHandle` holds an `Arc<WorkerPool>` clone (via
/// `workers`) and must be aborted and awaited during graceful shutdown,
/// mirroring `spawn_event_loop()`'s own `JoinHandle` — see `main.rs`'s
/// shutdown sequence comment on why every such clone must be released
/// before `Arc::try_unwrap(workers)` can succeed.
pub fn spawn_stats_tick(
    broadcaster: Arc<EventBroadcaster>,
    workers: Arc<WorkerPool>,
    interval: Duration,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        // `System::new_all()` performs an initial refresh, giving the
        // loop's first `refresh_cpu_usage()` call a prior sample to diff
        // against. sysinfo needs two refreshes at least
        // `sysinfo::MINIMUM_CPU_UPDATE_INTERVAL` (200ms) apart to report a
        // meaningful delta-based CPU percentage — recreating `System`
        // fresh on every tick would report 0.0 every single time instead.
        // `sys` is held across the whole loop's lifetime for this reason.
        // Only the very first tick (if it fires within 200ms of process
        // start, which cannot happen at the production 5s interval) can
        // under-report — an accepted, documented sysinfo limitation, not
        // a bug in this code.
        let mut sys = System::new_all();

        let mut ticker = tokio::time::interval(interval);
        // Delay (not Burst): if a tick is missed because a previous
        // iteration ran long, wait a full `interval` from *now* rather
        // than firing several catch-up ticks back-to-back. A burst of
        // stale SystemStats snapshots would actively mislead a connected
        // client about current host state, not merely arrive late.
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

        loop {
            ticker.tick().await;

            sys.refresh_cpu_usage();
            sys.refresh_memory();

            let cpu_pct = sys.global_cpu_usage();
            let ram_used_mib = sys.used_memory() / (1024 * 1024);
            let workers_info = workers.list().await;

            broadcaster.publish(WsEvent::SystemStats {
                cpu_pct,
                ram_used_mib,
                workers: workers_info,
            });
        }
    })
}
