# Tasks: Phase 004 — Hardware Detection

| Field | Value |
|-------|-------|
| Phase | 004 |
| Name | Hardware Detection |
| Milestone group | Observable system state |
| Depends on phases | 1-3 |
| Task file | `forge/tasks/tasks_phase004.json` |
| Tasks | 9 |

## Overview

Phase 4 implements `anvilml-hardware` (CPU/CUDA/ROCm detectors + an env-driven mock) and surfaces it through `GET /v1/system`. After this phase the running binary reports the real (or mock) hardware it sees. The mock detector, gated behind the `mock-hardware` feature, is what CI and local testing use so no GPU is required.

Every task in this phase implements **one module or one endpoint** plus its test. No task touches more than its named file(s). `cargo test` and `cargo clippy` are per-task gates; the phase as a whole is only complete when the **Runnable Proof** below passes.
 Hardware detection follows the SDK-free model (ANVILML_DESIGN Rev 5): Vulkan is the primary enumerator (P4-A3), with DXGI/sysfs+NVML fallbacks (P4-A4) and a hardcoded PCI-ID capability table (P4-A4B); `detect_all_devices` (P4-A5) orchestrates them and ML capabilities are refined by the worker's PyTorch at `Ready` (see Phase 9). Group B are retrofit leaves on the completed P3-A6: P4-B1 reconciles the frontend.mode default to Headless, and P4-B2 extends the already-committed GpuDevice/InferenceCaps with the new SDK-free fields. P4-A1/A2 (trait + mock) were authored before this revision and remain compatible.

## Tasks

| Task | Module / File | Summary |
|------|-------------|---------|
| P4-A1 | `crates/anvilml-hardware/src/lib.rs` | anvilml-hardware: DeviceDetector trait and CPU detector |
| P4-A2 | `crates/anvilml-hardware/src/mock.rs` | anvilml-hardware: mock detector (feature mock-hardware, env-driven) |
| P4-A3 | `crates/anvilml-hardware/src/vulkan.rs` | anvilml-hardware: Vulkan GPU enumerator (primary, SDK-free, fixture-tested) |
| P4-A4 | `crates/anvilml-hardware/src/{dxgi,sysfs,nvml}.rs` | anvilml-hardware: DXGI (Windows) + sysfs/NVML (Linux) fallback enumerators |
| P4-A4B | `crates/anvilml-hardware/src/device_db.rs` | anvilml-hardware: device_db PCI-ID capability table + resolution |
| P4-A5 | `crates/anvilml-hardware/src/lib.rs` | anvilml-hardware: detect_all_devices with override + host info |
| P4-A6 | `crates/anvilml-server/src/handlers/system.rs + backend/src/main.rs` | anvilml: detect hardware at startup and serve GET /v1/system |
| P4-B1 | `crates/anvilml-core/src/config.rs + anvilml.toml` | anvilml: reconcile frontend.mode default to Headless (retrofit; corrects earlier phases) |
| P4-B2 | `crates/anvilml-core/src/types/hardware.rs` | anvilml-core: extend GpuDevice + InferenceCaps for SDK-free detection (retrofit) |

## Task details

#### P4-A1: anvilml-hardware: DeviceDetector trait and CPU detector

- **Prereqs:** P3-A6
- **Tags:** —

Add anvilml-core + sysinfo to anvilml-hardware. Create src/lib.rs DeviceDetector trait: fn detect(&self)->Result<Vec<GpuDevice>,AnvilError>; fn refresh_vram(&self,idx:u32)->Result<(u32,u32),AnvilError>. Create src/cpu.rs CpuDetector returning one GpuDevice {index:0,name:'CPU',device_type:Cpu,vram 0,driver 'n/a'}. cargo test -p anvilml-hardware -- cpu exits 0.

#### P4-A2: anvilml-hardware: mock detector (feature mock-hardware, env-driven)

- **Prereqs:** P4-A1
- **Tags:** —

Create src/mock.rs behind feature mock-hardware: MockDetector reads ANVILML_MOCK_DEVICE_TYPE (cpu/cuda/rocm default cpu), ANVILML_MOCK_VRAM_MIB (default 8192), ANVILML_MOCK_GFX_ARCH (default gfx1100), returns one deterministic GpuDevice. Use serial_test for env-var tests. cargo test -p anvilml-hardware --features mock-hardware -- mock exits 0 with 3 fixture tests.

#### P4-A3: anvilml-hardware: Vulkan GPU enumerator (primary, SDK-free, fixture-tested)

- **Prereqs:** P4-A2
- **Tags:** —

Replace old SDK approach. Create src/vulkan.rs: VulkanDetector (primary, Linux+Windows) via ash. Headless VkInstance -> enumerate devices -> properties2 (+KHR_driver_properties name/driver) -> memory_properties2 (+EXT_memory_budget). total_vram=largest DEVICE_LOCAL heapSize; available=heapBudget-heapUsage if budget ext else heapSize. Fill pci ids, device_type via vendor map, driver_version, source=Vulkan. Loader absent->Ok(vec![]). Implement P4-A1 DeviceDetector trait. cargo test -p anvilml-hardware -- vulkan exits 0. Also: cargo check --target x86_64-pc-windows-gnu --features mock-hardware.

#### P4-A4: anvilml-hardware: DXGI (Windows) + sysfs/NVML (Linux) fallback enumerators

- **Prereqs:** P4-A3
- **Tags:** —

Fallback enumerators implementing DeviceDetector. src/dxgi.rs (#[cfg(windows)]): DxgiDetector via IDXGIFactory EnumAdapters1 -> name, vendor/device id, DedicatedVideoMemory, enumeration_source=Dxgi. src/sysfs.rs + src/nvml.rs (#[cfg(unix)]): read /sys/bus/pci/devices/* for ids (source=Sysfs); VRAM via amdgpu sysfs or NVML libnvidia-ml (source=Nvml). All: absent/error->Ok(vec![]); per-device failure warn+skip. Parse helpers for tests. cargo test -p anvilml-hardware -- dxgi/sysfs/nvml exit 0 with fixtures. Also pass: cargo check --target x86_64-pc-windows-gnu --features mock-hardware.

#### P4-A4B: anvilml-hardware: device_db PCI-ID capability table + resolution

- **Prereqs:** P4-A4
- **Tags:** —

Create src/device_db.rs: hardcoded compile-time table (const slice or embedded RON validated by a unit test) mapping (vendor_id,device_id)->DeviceCapabilityEntry{model_name,arch,fp16,bf16,flash_attention} per 5.5. NO VRAM in table. lookup(vendor,device)->Option<&Entry>. resolve_caps(dev:&mut GpuDevice): hit->fill name/arch/caps, capabilities_source=DeviceTable; miss->conservative defaults (fp16 from shaderFloat16 if known else false, bf16/flash false), capabilities_source=Fallback, warn! the unknown PCI id. Seed a few NVIDIA+AMD ids. cargo test -p anvilml-hardware -- device_db exits 0.

#### P4-A5: anvilml-hardware: detect_all_devices with override + host info

- **Prereqs:** P4-A4B
- **Tags:** reasoning

Implement detect_all_devices(cfg)->HardwareInfo per 5.1/5.6. mock-hardware: MockDetector only. Else: hardware_override->one synthetic device (source=Override). Else enumerate VulkanDetector; if empty, fall back DXGI (windows)/sysfs+NVML (unix); run device_db::resolve_caps per device. Vendor->DeviceType (0x10DE Cuda, 0x1002 Rocm both OSes, 0x8086 Cpu). No GPU->one Cpu device. HostInfo via sysinfo. Caps/VRAM refined later by worker at Ready. cargo test -p anvilml-hardware --features mock-hardware exits 0 (>=8: override, vulkan, fallback, vendor map, cpu-only-when-no-gpu).

#### P4-A6: anvilml: detect hardware at startup and serve GET /v1/system

- **Prereqs:** P4-A5
- **Tags:** —

Add anvilml-hardware to backend + anvilml-server (forward mock-hardware). AppState gets hardware: Arc<RwLock<HardwareInfo>>. main.rs calls detect_all_devices(&cfg) at startup, logs each device (name, ids, vram, enumeration_source, capabilities_source), stores it. handlers/system.rs get_system->Json<HardwareInfo>; wire GET /v1/system. Add --print-hardware CLI subcommand (Rev5) that detects, prints the table, exits 0 without binding. Verify: ANVILML_MOCK_DEVICE_TYPE=cuda cargo run --features mock-hardware; curl /v1/system shows mock CUDA; anvilml --print-hardware prints+exits.

#### P4-B1: anvilml: reconcile frontend.mode default to Headless (retrofit; corrects earlier phases)

- **Prereqs:** P3-A6
- **Tags:** correction

RETROFIT existing committed files (config.rs + anvilml.toml exist with the OLD frontend default). In ONE atomic change: default FrontendMode/FrontendConfig to Headless in config.rs AND edit ./anvilml.toml [frontend] mode='headless' (any './bloomery' -> commented './frontend') together so P3-B2's drift guard never sees a mismatch. AnvilML is headless; BloomeryUI runs separately under SindriStudio, never served by AnvilML. Update ENVIRONMENT.md frontend section + env table. Verify: ServerConfig::default().frontend.mode==Headless; cargo test config + P3-B2 config_reference both pass.

#### P4-B2: anvilml-core: extend GpuDevice + InferenceCaps for SDK-free detection (retrofit)

- **Prereqs:** P3-A6
- **Tags:** correction

RETROFIT committed src/types/hardware.rs (OLD 4.3). Extend GpuDevice per UPDATED 4.3: add pci_vendor_id:u16, pci_device_id:u16, vram_free_mib:u32, arch:Option<String>, enumeration_source, capabilities_source (keep name/device_type/vram_total_mib/driver_version). Add enums EnumerationSource{Vulkan,Dxgi,Sysfs,Nvml,Override,Mock} + CapabilitySource{Worker,DeviceTable,Fallback}. Add flash_attention:bool to InferenceCaps. Derive standard set+ToSchema. Update P4-A1/A2 GpuDevice constructions for new fields (Cpu: zeros/None/Mock). cargo test -p anvilml-core -- hardware exits 0.


## Runnable Proof

Run with the mock detector forced to CUDA and confirm the hardware endpoint reflects it.

```bash
ANVILML_MOCK_DEVICE_TYPE=cuda ANVILML_MOCK_VRAM_MIB=24576 \
  cargo run --features mock-hardware
curl -s http://127.0.0.1:8488/v1/system | python -m json.tool
```

Expected (200): `HardwareInfo` JSON with `gpus[0].device_type == "cuda"`, `vram_total_mib == 24576`, a populated `host` block (os, cpu_model, ram_total_mib), and `inference_caps`. Re-run without the env var to see the CPU fallback device. Phase done when `/v1/system` reflects the configured mock device and `cargo test -p anvilml-hardware --features mock-hardware` is green.