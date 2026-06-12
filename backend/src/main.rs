mod cli;
mod preflight;
mod shutdown;

use std::sync::Arc;

use tokio::sync::broadcast;

use anvilml_core::types::events::{WorkerStatusChangedEvent, WsEvent};
use anvilml_core::{load_config, DeviceType, EnumerationSource, FrontendMode, HardwareInfo};
use anvilml_ipc::WorkerEvent;
use anvilml_scheduler::{JobQueue, JobScheduler};
use anvilml_server::artifact::store::ArtifactStore;
use anvilml_server::ws::stats_tick::spawn_system_stats_tick;
use anvilml_server::{build_router, App, EventBroadcaster};
use chrono::Utc;
use tracing_subscriber::fmt::layer as fmt_layer;
use tracing_subscriber::Layer;

/// Spawns a background task that bridges WorkerPool events to the WS broadcaster.
fn spawn_worker_status_bridge(
    workers: &anvilml_worker::WorkerPool,
    broadcaster: &Arc<EventBroadcaster>,
) {
    let mut rx = workers.subscribe_events();
    let bc = Arc::clone(broadcaster);

    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok((worker_id, event)) => {
                    if let WorkerEvent::WorkerStatusChanged { status } = event {
                        tracing::debug!(
                            worker_id = %worker_id,
                            status = ?status,
                            "bridging worker status change to WS"
                        );
                        bc.send(WsEvent::WorkerStatusChanged(WorkerStatusChangedEvent {
                            event: "worker.status".to_string(),
                            timestamp: Utc::now(),
                            worker_id,
                            status,
                        }));
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    tracing::debug!(lagged = n, "worker status bridge dropped events");
                }
                Err(broadcast::error::RecvError::Closed) => {
                    tracing::warn!("worker status bridge channel closed");
                    break;
                }
            }
        }
    });
}

/// Format a hardware info table to stdout.
fn print_hardware_table(hw: &HardwareInfo) {
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║                     Host Information                     ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║ OS:          {}", hw.host.os);
    println!("║ CPU:         {}", hw.host.cpu_model);
    println!("║ Total RAM:   {} MiB", hw.host.ram_total_mib);
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║                    GPU Devices                           ║");
    println!("╠══════╦════════════════════╦═════════╦═══════════╦══════════╦═════════════════╦════════════════════╣");
    println!(
        "║ #    ║ Name               ║ Type    ║ VRAM (MiB)║ Enum Src ║ Capabilities   ║ Arch                ║"
    );
    println!("╠══════╬════════════════════╬═════════╬═══════════╬══════════╬═════════════════╬════════════════════╣");

    for dev in &hw.gpus {
        let device_type_str = match dev.device_type {
            DeviceType::Cuda => "CUDA",
            DeviceType::Rocm => "ROCm",
            DeviceType::Cpu => "CPU",
        };
        let enum_src_str = match dev.enumeration_source {
            EnumerationSource::Vulkan => "Vulkan",
            EnumerationSource::Dxgi => "DXGI",
            EnumerationSource::Sysfs => "sysfs",
            EnumerationSource::Nvml => "NVML",
            EnumerationSource::Override => "Override",
            EnumerationSource::Mock => "Mock",
            EnumerationSource::DeviceTable => "DB",
            EnumerationSource::Fallback => "Fallback",
        };
        let mut caps_parts: Vec<&str> = Vec::new();
        if dev.caps.fp32 {
            caps_parts.push("FP32");
        }
        if dev.caps.fp16 {
            caps_parts.push("FP16");
        }
        if dev.caps.bf16 {
            caps_parts.push("BF16");
        }
        if dev.caps.fp8 {
            caps_parts.push("FP8");
        }
        if dev.caps.fp4 {
            caps_parts.push("FP4");
        }
        if dev.caps.nvfp4 {
            caps_parts.push("NVFP4");
        }
        if dev.caps.flash_attention {
            caps_parts.push("FA");
        }
        let caps_str = if caps_parts.is_empty() {
            "-"
        } else {
            caps_parts.join("+").leak()
        };
        let arch_str = dev.arch.as_deref().unwrap_or("-");

        let display_name = match &dev.db_group_name {
            Some(group) if group != &dev.name => format!("{} ({})", dev.name, group),
            _ => dev.name.clone(),
        };
        let name_trunc: String = display_name.chars().take(20).collect();
        let arch_trunc: String = arch_str.chars().take(16).collect();

        println!(
            "║ {:<4} ║ {:<20} ║ {:<7} ║ {:<9} ║ {:<8} ║ {:<15} ║ {:<16} ║",
            dev.index,
            name_trunc,
            device_type_str,
            dev.vram_total_mib,
            enum_src_str,
            caps_str,
            arch_trunc
        );
    }

    println!("╚══════╩════════════════════╩═════════╩═══════════╩══════════╩═════════════════╩════════════════════╝");

    println!("\nInference capabilities:");
    println!(
        "  FP32: {}  FP16: {}  BF16: {}  FP8: {}  FP4: {}  NVFP4: {}  Flash Attention: {}",
        hw.inference_caps.fp32,
        hw.inference_caps.fp16,
        hw.inference_caps.bf16,
        hw.inference_caps.fp8,
        hw.inference_caps.fp4,
        hw.inference_caps.nvfp4,
        hw.inference_caps.flash_attention,
    );
}

#[tokio::main]
async fn main() {
    let args = cli::parse();

    // Initialise the tracing subscriber before any server logic.
    let env_filter = std::env::var("ANVILML_LOG")
        .or_else(|_| std::env::var("RUST_LOG"))
        .unwrap_or_else(|_| "info,sqlx::query=off".to_string());
    let filter = tracing_subscriber::EnvFilter::try_new(env_filter).unwrap_or_else(|e| {
        eprintln!("Invalid RUST_LOG/ANVILML_LOG value: {e}, falling back to info");
        tracing_subscriber::EnvFilter::new("info,sqlx::query=off")
    });

    // Build the formatter layer.  Boxing via `dyn Layer` unifies the
    // plain and JSON variants which have incompatible concrete types.
    let fmt_layer: Box<dyn Layer<tracing_subscriber::Registry> + Send + Sync> =
        match args.log_format {
            cli::LogFormat::Plain => Box::new(fmt_layer().with_filter(filter.clone())),
            cli::LogFormat::Json => Box::new(fmt_layer().json().with_filter(filter)),
        };

    use tracing_subscriber::prelude::*;
    let subscriber = tracing_subscriber::registry().with(fmt_layer).try_init();
    let _ = subscriber;

    let overrides = args.to_overrides();

    // Resolve config path from CLI or use the default.
    let toml_path = if args.config.as_os_str().is_empty() {
        None
    } else {
        Some(args.config.as_path())
    };

    let cfg = load_config(toml_path, overrides).expect("Failed to load config");

    // Open the SQLite database and run migrations first — needed for device
    // capability lookups in detect_all_devices.
    let db = anvilml_registry::db::open(&cfg.db_path)
        .await
        .expect("failed to open database");

    // --print-hardware: detect hardware, print table, exit 0.
    if args.print_hardware {
        let hw_info = anvilml_hardware::detect_all_devices(&cfg, &db)
            .await
            .expect("hardware detection failed");
        print_hardware_table(&hw_info);
        std::process::exit(0);
    }

    // Normal server path: detect hardware, log devices, store in AppState.
    let hw_info = anvilml_hardware::detect_all_devices(&cfg, &db)
        .await
        .expect("hardware detection failed");

    for dev in &hw_info.gpus {
        tracing::info!(
            device.name = %dev.name,
            index = dev.index,
            device_type = ?dev.device_type,
            vram_total_mib = dev.vram_total_mib,
            enumeration_source = ?dev.enumeration_source,
            capabilities_source = ?dev.capabilities_source,
        );
    }

    // Reset any ghost jobs left from a previous unclean exit.
    let ghost_count = anvilml_registry::db::reset_ghost_jobs(&db)
        .await
        .expect("failed to reset ghost jobs");
    tracing::info!(ghost_jobs_reset = ghost_count, "ghost jobs reset");

    // Build the model registry and perform an initial background rescan.
    let registry = Arc::new(anvilml_registry::ModelRegistry::new(db.clone()));
    let scan_reg = Arc::clone(&registry);
    let scan_dirs = cfg.model_dirs.clone();
    tokio::spawn(async move {
        match scan_reg.rescan(&scan_dirs).await {
            Ok((upserted, removed)) => {
                tracing::info!(
                    models_scanned = upserted,
                    removed_stale = removed,
                    "initial model scan complete"
                )
            }
            Err(e) => tracing::warn!("initial model scan failed: {e}"),
        }
    });

    // Spawn the worker pool: one worker per GPU device (or CPU fallback).
    let workers = anvilml_worker::WorkerPool::spawn_all(&hw_info, &cfg).await;
    tracing::info!(
        workers_spawned = workers.list().await.len(),
        "worker pool spawned"
    );

    let broadcaster = Arc::new(EventBroadcaster::new(cfg.limits.ws_broadcast_capacity));

    // Bridge worker status events to WebSocket clients.
    spawn_worker_status_bridge(&workers, &broadcaster);

    // Separate broadcast channel for the job scheduler (WsEvent, not Arc<WsEvent>).
    let (scheduler_broadcaster, _scheduler_rx) = broadcast::channel::<WsEvent>(16);

    // Wrap the worker pool in Arc for sharing with the scheduler.
    let workers = Arc::new(workers);

    // Construct the artifact store.
    let artifact_store = ArtifactStore::new(cfg.artifact_dir.clone(), db.clone());

    // Construct the job scheduler and wire it into AppState.
    let ledger = Arc::new(tokio::sync::Mutex::new(anvilml_scheduler::VramLedger::new()));
    let scheduler = Arc::new(JobScheduler::new(
        JobQueue::new(),
        workers.clone(),
        db.clone(),
        scheduler_broadcaster,
        ledger,
        cfg.gpu_selection.default_device.clone(),
        artifact_store.clone(),
    ));

    let bind_addr = format!("{}:{}", cfg.host, cfg.port);
    // Clone db for the shutdown handler (state consumes the original).
    let db_shutdown = db.clone();

    let state = App::new_with_hardware(
        env!("CARGO_PKG_VERSION"),
        hw_info,
        Some(db),
        Some(registry),
        Some(cfg.model_dirs.clone()),
        broadcaster,
        Some(workers.clone()),
        Some(scheduler.clone()),
        artifact_store,
        cfg.clone(),
    );

    // Run Python environment preflight and populate AppState.env_report.
    let env_report = preflight::run_preflight(&cfg).await;
    tracing::info!(
        preflight_ok = env_report.preflight_ok,
        python_path = %env_report.python_path,
        python_version = %env_report.python_version,
        torch_version = %env_report.torch_version,
        reason = %env_report.reason,
        "python preflight complete"
    );
    state.set_env_report(env_report);

    // Clone state for shutdown handler before build_router consumes it.
    let shutdown_state = state.clone();

    spawn_system_stats_tick(shutdown_state.clone());
    let _dispatch_handle = scheduler.start_dispatch_loop();
    let router = build_router(shutdown_state);
    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .unwrap_or_else(|e| panic!("Failed to bind to {bind_addr}: {e}"));

    // Open the default browser to the server URL (unless disabled or headless).
    if args.no_browser {
        tracing::debug!("skipping browser open: --no-browser flag set");
    } else if matches!(cfg.frontend.mode, FrontendMode::Headless) {
        tracing::debug!("skipping browser open: frontend mode is headless");
    } else {
        let url = format!("http://{bind_addr}");
        match open::that(&url) {
            Ok(()) => tracing::debug!(url = %url, "browser opened"),
            Err(e) => tracing::warn!(url = %url, error = %e, "failed to open browser"),
        }
    }

    let _ = axum::serve(listener, router)
        .with_graceful_shutdown(shutdown::shutdown_signal(Arc::new(state), db_shutdown))
        .await;

    tracing::info!("HTTP server drained, exiting");
    std::process::exit(0);
}
