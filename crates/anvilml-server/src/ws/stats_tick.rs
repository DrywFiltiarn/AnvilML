//! Background task that periodically broadcasts system statistics via WebSocket.
//!
//! Spawns a tokio task that fires every 5 seconds, reads the latest hardware
//! state from `AppState` (per-device VRAM) and host RAM via the `sysinfo` crate,
//! builds a `SystemStatsEvent`, and broadcasts it as `WsEvent::SystemStats`
//! through the existing `EventBroadcaster`.

use std::sync::Arc;

use anvilml_core::{GpuStatSnapshot, SystemStatsEvent, WsEvent};
use chrono::Utc;
use tokio::task::JoinHandle;
use tokio::time::{interval, Duration};

use crate::App;

/// Spawn a background task that broadcasts system stats every 5 seconds.
///
/// The task reads the latest hardware state from `AppState.hardware` and
/// host RAM via `sysinfo`, building a `SystemStatsEvent` each tick.
///
/// Returns a `JoinHandle` — dropping it cancels the task gracefully
/// (the interval loop yields on cancellation via `interval.tick().await`).
pub fn spawn_system_stats_tick(state: App) -> JoinHandle<()> {
    let broadcaster = Arc::clone(&state.broadcaster);

    tokio::spawn(async move {
        let mut interval = interval(Duration::from_secs(5));

        loop {
            interval.tick().await;

            // Read GPU VRAM from AppState.
            let hw = state.hardware();
            let gpus: Vec<GpuStatSnapshot> = hw
                .gpus
                .iter()
                .map(|d| GpuStatSnapshot {
                    index: d.index,
                    vram_used_mib: d.vram_total_mib.saturating_sub(d.vram_free_mib),
                    vram_total_mib: d.vram_total_mib,
                })
                .collect();

            // Read host RAM via sysinfo.
            let mut system = sysinfo::System::new();
            system.refresh_memory();
            let ram_total_mib = system.total_memory();
            let ram_used_mib = system.used_memory();

            let event = SystemStatsEvent {
                event: "system.stats".to_string(),
                timestamp: Utc::now(),
                gpus,
                ram_used_mib,
                ram_total_mib,
            };

            broadcaster.send(WsEvent::SystemStats(event));
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ws::broadcaster::EventBroadcaster;
    use anvilml_core::{
        DeviceType, EnumerationSource, GpuDevice, HardwareInfo, HostInfo, InferenceCaps,
    };
    use std::time::Duration as StdDuration;

    /// The tick task should broadcast a `SystemStats` event within one interval.
    #[tokio::test]
    async fn stats_tick_broadcasts_event() {
        let broadcaster = Arc::new(EventBroadcaster::new(16));
        let mut rx = broadcaster.subscribe();

        // Build mock hardware with 2 GPUs.
        let hardware = HardwareInfo {
            host: HostInfo {
                os: "Linux".to_string(),
                cpu_model: "Test CPU".to_string(),
                ram_total_mib: 32768,
                ram_free_mib: 16000,
            },
            gpus: vec![
                GpuDevice {
                    index: 0,
                    name: "Mock GPU 0".to_string(),
                    device_type: DeviceType::Cuda,
                    vram_total_mib: 8192,
                    vram_free_mib: 6000,
                    driver_version: "535.0".to_string(),
                    pci_vendor_id: 0x10de,
                    pci_device_id: 0x20b0,
                    arch: Some("8.0".to_string()),
                    caps: InferenceCaps::default(),
                    enumeration_source: EnumerationSource::Mock,
                    capabilities_source: anvilml_core::CapabilitySource::Fallback,
                    db_group_name: None,
                },
                GpuDevice {
                    index: 1,
                    name: "Mock GPU 1".to_string(),
                    device_type: DeviceType::Rocm,
                    vram_total_mib: 16384,
                    vram_free_mib: 14000,
                    driver_version: "535.0".to_string(),
                    pci_vendor_id: 0x1002,
                    pci_device_id: 0x740c,
                    arch: Some("gfx1100".to_string()),
                    caps: InferenceCaps::default(),
                    enumeration_source: EnumerationSource::Mock,
                    capabilities_source: anvilml_core::CapabilitySource::Fallback,
                    db_group_name: None,
                },
            ],
            inference_caps: InferenceCaps::default(),
        };

        let pool = anvilml_registry::open_in_memory().await.unwrap();
        let artifact_store =
            crate::artifact::store::ArtifactStore::new(tempfile::tempdir().unwrap().keep(), pool);
        let state = App::new_with_hardware(
            "test",
            hardware,
            None,
            None,
            None,
            Arc::clone(&broadcaster),
            None,
            None,
            artifact_store,
        );

        // Spawn the tick task.
        let handle = spawn_system_stats_tick(state);

        // Wait slightly more than 5 seconds for the first tick.
        tokio::time::sleep(StdDuration::from_secs(6)).await;

        // Try to receive an event.
        let mut received = None;
        for _ in 0..10 {
            match rx.try_recv() {
                Ok(event) => {
                    received = Some(event);
                    break;
                }
                Err(tokio::sync::broadcast::error::TryRecvError::Empty) => {
                    tokio::time::sleep(StdDuration::from_millis(500)).await;
                }
                Err(e) => panic!("unexpected receiver error: {e}"),
            }
        }

        let event = received.expect("should receive a SystemStats event within 10 seconds");
        let WsEvent::SystemStats(stats) = event.as_ref() else {
            panic!("expected WsEvent::SystemStats, got {:?}", event);
        };

        assert_eq!(stats.event, "system.stats");
        assert_eq!(stats.gpus.len(), 2);
        // GPU 0: used = 8192 - 6000 = 2192 MiB.
        assert_eq!(stats.gpus[0].index, 0);
        assert_eq!(stats.gpus[0].vram_used_mib, 2192);
        assert_eq!(stats.gpus[0].vram_total_mib, 8192);
        // GPU 1: used = 16384 - 14000 = 2384 MiB.
        assert_eq!(stats.gpus[1].index, 1);
        assert_eq!(stats.gpus[1].vram_used_mib, 2384);
        assert_eq!(stats.gpus[1].vram_total_mib, 16384);
        // RAM values from sysinfo should be non-zero on a real system.
        assert!(stats.ram_total_mib > 0, "ram_total_mib should be positive");

        // Cancel the task.
        drop(handle);
    }
}
