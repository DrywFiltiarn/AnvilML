mod cli;
mod shutdown;

use anvilml_core::{load_config, DeviceType, EnumerationSource, HardwareInfo};
use anvilml_server::{build_router, AppState};
use tracing_subscriber::fmt::layer as fmt_layer;
use tracing_subscriber::Layer;

/// Format a hardware info table to stdout.
fn print_hardware_table(hw: &HardwareInfo) {
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║                     Host Information                     ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║ OS:          {}:", " ".repeat(50 - 8));
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
        let caps_str = if dev.caps.fp16 && dev.caps.bf16 && dev.caps.flash_attention {
            "FP16+BF16+FA"
        } else if dev.caps.fp16 && dev.caps.bf16 {
            "FP16+BF16"
        } else if dev.caps.fp16 {
            "FP16"
        } else if dev.caps.bf16 {
            "BF16"
        } else {
            "-"
        };
        let arch_str = dev.arch.as_deref().unwrap_or("-");

        let name_trunc: String = dev.name.chars().take(20).collect();
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
        "  FP16: {}  BF16: {}  Flash Attention: {}",
        hw.inference_caps.fp16, hw.inference_caps.bf16, hw.inference_caps.flash_attention
    );
}

#[tokio::main]
async fn main() {
    let args = cli::parse();

    // Initialise the tracing subscriber before any server logic.
    let env_filter = std::env::var("ANVILML_LOG")
        .or_else(|_| std::env::var("RUST_LOG"))
        .unwrap_or_else(|_| "info".to_string());
    let filter = tracing_subscriber::EnvFilter::try_new(env_filter).unwrap_or_else(|e| {
        eprintln!("Invalid RUST_LOG/ANVILML_LOG value: {e}, falling back to info");
        tracing_subscriber::EnvFilter::new("info")
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

    // --print-hardware: detect hardware, print table, exit 0.
    if args.print_hardware {
        let hw_info =
            anvilml_hardware::detect_all_devices(&cfg).expect("hardware detection failed");
        print_hardware_table(&hw_info);
        std::process::exit(0);
    }

    // Normal server path: detect hardware, log devices, store in AppState.
    let hw_info = anvilml_hardware::detect_all_devices(&cfg).expect("hardware detection failed");

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

    let state = AppState::new_with_hardware(env!("CARGO_PKG_VERSION"), hw_info);
    let router = build_router(state);

    let bind_addr = format!("{}:{}", cfg.host, cfg.port);
    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .unwrap_or_else(|e| panic!("Failed to bind to {bind_addr}: {e}"));

    tracing::info!("Listening on http://{bind_addr}");
    let _ = axum::serve(listener, router)
        .with_graceful_shutdown(shutdown::shutdown_signal())
        .await;
}
