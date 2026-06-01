# Tasks: Phase 004 — Hardware Detection

| Field | Value |
|-------|-------|
| Phase | 004 |
| Name | Hardware Detection |
| Milestone group | Observable system state |
| Depends on phases | 1-3 |
| Task file | `forge/tasks/tasks_phase004.json` |
| Tasks | 7 |

## Overview

Phase 4 implements `anvilml-hardware` (CPU/CUDA/ROCm detectors + an env-driven mock) and surfaces it through `GET /v1/system`. After this phase the running binary reports the real (or mock) hardware it sees. The mock detector, gated behind the `mock-hardware` feature, is what CI and local testing use so no GPU is required.

Every task in this phase implements **one module or one endpoint** plus its test. No task touches more than its named file(s). `cargo test` and `cargo clippy` are per-task gates; the phase as a whole is only complete when the **Runnable Proof** below passes.
 Group B (P4-B1) is a retrofit/leaf: it reconciles the `frontend.mode` default to `Headless` after the topology correction (AnvilML never serves BloomeryUI; SindriStudio runs it separately). The config types and `anvilml.toml` were already authored and committed in earlier phases, so this edits those existing files rather than reopening a finished phase. It depends only on the completed P3-A6, runs at the earliest opportunity, and blocks nothing in the P4-A hardware chain. The struct default and the committed toml must change together so the P3-B2 drift guard stays green.

## Tasks

| Task | Module / File | Summary |
|------|-------------|---------|
| P4-A1 | `crates/anvilml-hardware/src/lib.rs` | anvilml-hardware: DeviceDetector trait and CPU detector |
| P4-A2 | `crates/anvilml-hardware/src/mock.rs` | anvilml-hardware: mock detector (feature mock-hardware, env-driven) |
| P4-A3 | `crates/anvilml-hardware/src/cuda.rs` | anvilml-hardware: CUDA detector via nvidia-smi (fixture-tested) |
| P4-A4 | `crates/anvilml-hardware/src/rocm.rs` | anvilml-hardware: ROCm detector (Linux rocm-smi + Windows amd-smi/HIP probe) |
| P4-A5 | `crates/anvilml-hardware/src/lib.rs` | anvilml-hardware: detect_all_devices with override + host info |
| P4-A6 | `backend/src/main.rs` | anvilml: detect hardware at startup and serve GET /v1/system |
| P4-B1 | `crates/anvilml-core/src/config.rs + anvilml.toml` | anvilml: reconcile frontend.mode default to Headless (retrofit; corrects earlier phases) |

## Task details

#### P4-A1: anvilml-hardware: DeviceDetector trait and CPU detector

- **Prereqs:** P3-A6
- **Tags:** —

Add anvilml-core + sysinfo to anvilml-hardware. Create src/lib.rs DeviceDetector trait: fn detect(&self)->Result<Vec<GpuDevice>,AnvilError>; fn refresh_vram(&self,idx:u32)->Result<(u32,u32),AnvilError>. Create src/cpu.rs CpuDetector returning one GpuDevice {index:0,name:'CPU',device_type:Cpu,vram 0,driver 'n/a'}. cargo test -p anvilml-hardware -- cpu exits 0.

#### P4-A2: anvilml-hardware: mock detector (feature mock-hardware, env-driven)

- **Prereqs:** P4-A1
- **Tags:** —

Create src/mock.rs behind feature mock-hardware: MockDetector reads ANVILML_MOCK_DEVICE_TYPE (cpu/cuda/rocm default cpu), ANVILML_MOCK_VRAM_MIB (default 8192), ANVILML_MOCK_GFX_ARCH (default gfx1100), returns one deterministic GpuDevice. Use serial_test for env-var tests. cargo test -p anvilml-hardware --features mock-hardware -- mock exits 0 with 3 fixture tests.

#### P4-A3: anvilml-hardware: CUDA detector via nvidia-smi (fixture-tested)

- **Prereqs:** P4-A2
- **Tags:** —

Create src/cuda.rs: CudaDetector runs nvidia-smi --query-gpu=index,name,memory.total,memory.free,driver_version --format=csv,noheader,nounits, parses CSV to Vec<GpuDevice> type Cuda. Absent/non-zero exit -> Ok(vec![]). Extract parse_nvidia_smi(raw:&str)->Vec<GpuDevice> helper for testing. InferenceCaps: fp16 true, bf16 true if driver major>=525. cargo test -p anvilml-hardware -- cuda exits 0 with single+dual GPU fixtures.

#### P4-A4: anvilml-hardware: ROCm detector (Linux rocm-smi + Windows amd-smi/HIP probe)

- **Prereqs:** P4-A3
- **Tags:** —

Create src/rocm.rs: RocmDetector — ROCm on BOTH Linux and Windows (mandatory MVP backend, design 5). Linux: parse rocm-smi --showmeminfo vram --json + gfx from rocminfo. Windows: try amd-smi; else HIP probe via worker venv python (torch.cuda.get_device_properties -> index/name/total_vram), no Linux-only CLIs. bytes->MiB; absent/error/no-torch -> Ok(vec![]); missing keys 0+warn. Helpers parse_rocm_smi(raw) + parse_hip_probe(raw). InferenceCaps: fp16+bf16 true, flash_attention gated on gfx arch (gfx>=gfx1100). cargo test -p anvilml-hardware -- rocm exits 0 with rocm-smi AND HIP-probe fixtures.

#### P4-A5: anvilml-hardware: detect_all_devices with override + host info

- **Prereqs:** P4-A4
- **Tags:** reasoning

Implement detect_all_devices(cfg:&ServerConfig)->HardwareInfo in lib.rs. feature mock-hardware: MockDetector only. Else: if cfg.hardware_override set, return one synthetic device of that type/vram; else run Cuda then Rocm (Rocm runs on BOTH Linux and Windows per design 5), first non-empty wins; fall back to exactly one Cpu device ONLY when no supported GPU is detected. Populate HostInfo via sysinfo (os, cpu_model, ram_total_mib, ram_free_mib). cargo test -p anvilml-hardware --features mock-hardware exits 0 with >=8 tests incl override, cuda-wins, rocm-wins, and cpu-only-when-no-gpu.

#### P4-A6: anvilml: detect hardware at startup and serve GET /v1/system

- **Prereqs:** P4-A5
- **Tags:** —

Add anvilml-hardware dep to backend + anvilml-server (forward mock-hardware feature). Add hardware: Arc<RwLock<HardwareInfo>> to AppState. In main.rs call detect_all_devices(&cfg) at startup, log detected devices, store in AppState. Add handlers/system.rs get_system(State)->Json<HardwareInfo> reading AppState.hardware. Wire GET /v1/system. Verify: ANVILML_MOCK_DEVICE_TYPE=cuda cargo run --features mock-hardware then curl /v1/system shows the mock CUDA device.

#### P4-B1: anvilml: reconcile frontend.mode default to Headless (retrofit; corrects earlier phases)

- **Prereqs:** P3-A6
- **Tags:** correction

RETROFIT existing committed files (config.rs + anvilml.toml exist with the OLD frontend default). In ONE atomic change: default FrontendMode/FrontendConfig to Headless in config.rs AND edit ./anvilml.toml [frontend] mode='headless' (any './bloomery' -> commented './frontend') together so P3-B2's drift guard never sees a mismatch. AnvilML is headless; BloomeryUI runs separately under SindriStudio, never served by AnvilML. Update ENVIRONMENT.md frontend section + env table. Verify: ServerConfig::default().frontend.mode==Headless; cargo test config + P3-B2 config_reference both pass.


## Runnable Proof

Run with the mock detector forced to CUDA and confirm the hardware endpoint reflects it.

```bash
ANVILML_MOCK_DEVICE_TYPE=cuda ANVILML_MOCK_VRAM_MIB=24576 \
  cargo run --features mock-hardware
curl -s http://127.0.0.1:8488/v1/system | python -m json.tool
```

Expected (200): `HardwareInfo` JSON with `gpus[0].device_type == "cuda"`, `vram_total_mib == 24576`, a populated `host` block (os, cpu_model, ram_total_mib), and `inference_caps`. Re-run without the env var to see the CPU fallback device. Phase done when `/v1/system` reflects the configured mock device and `cargo test -p anvilml-hardware --features mock-hardware` is green.
