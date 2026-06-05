# Supported Devices Database

**Purpose:** Authoritative reference for the `device_capabilities` SQLite table
seed data. Consumed by Forge task P7-F1 (migration DDL) and P7-F2 (store
schema). The two Markdown tables below are the source of truth for all
`DeviceCapabilityRow` values seeded at startup.

**Sources:**
- NVIDIA: Ada GPU Architecture whitepaper v2.02, Blackwell architecture docs,
  CUDA Toolkit documentation, PyTorch CUDA semantics docs, TensorRT-LLM
  release notes, torchao releases
- AMD: ROCm 7.2.2 Data Types and Precision Support reference
  (rocm.docs.amd.com/en/latest/reference/precision-support.html),
  MI300 microarchitecture docs, Linux kernel `amdgpu_drv.c`, OpenBSD `pcidevs`
- Capability flags represent **PyTorch-usable hardware capabilities** as of
  ROCm 7.2 / CUDA 12.x. The worker's torch probe at `Ready` time is the
  authoritative runtime override; these flags are the pre-spawn scheduling hint.

---

## Capability Field Definitions

| Field | Type | Meaning |
|---|---|---|
| `fp32` | bool | Native fp32 via TF32 tensor core path usable by torch (not generic shader fp32) |
| `fp16` | bool | Native fp16 matrix/tensor core usable by torch |
| `bf16` | bool | Native bf16 matrix/tensor core usable by torch |
| `fp8` | bool | Native fp8 matrix/tensor core usable by torch (storage + compute) |
| `fp4` | bool | Native fp4 matrix core usable by torch — AMD CDNA4+ only |
| `nvfp4` | bool | Native NVIDIA fp4 matrix core usable by torch — Blackwell+ only |
| `flash_attn` | bool | Hardware-supported Flash Attention via torch SDPA or ROCm rocWMMA |

**Notes:**
- `fp32 = true` means `torch.set_float32_matmul_precision('high')` routes
  through TF32 tensor cores (Ampere SM 8.0+ and CDNA3+). All GPUs support
  generic fp32 shader compute; this flag is not about that.
- `fp8 = true` on Ada (SM 8.9) reflects torch/torchao/TensorRT-LLM usability
  via `torch._scale_mm` and W4A8 GEMM plugins. TransformerEngine fp8_autocast
  requires SM 9.x and will not activate on Ada regardless of this flag.
- `fp8 = true` on RDNA4 (gfx1201) reflects hardware capability. ROCm
  TransformerEngine whitelists gfx94x/gfx95x only as of ROCm 7.2; the worker
  torch probe will set the effective capability at Ready time.
- `fp4 = false` for all NVIDIA entries. `nvfp4 = false` for all AMD entries.
  These fields exist for completeness and future-proofing.

---

## NVIDIA Devices

**Architecture capability summary by generation:**

| Generation | SM | fp32 | fp16 | bf16 | fp8 | fp4 | nvfp4 | flash_attn |
|---|---|---|---|---|---|---|---|---|
| Pascal | 6.1 | N | N | N | N | N | N | N |
| Turing GTX 16xx | 7.5 | N | N | N | N | N | N | N |
| Turing RTX 20xx | 7.5 | N | Y | N | N | N | N | N |
| Ampere datacenter GA100 | 8.0 | Y | Y | Y | N | N | N | Y |
| Ampere consumer RTX 30xx | 8.6 | Y | Y | N | N | N | N | Y |
| Hopper GH100 | 9.0 | Y | Y | Y | Y | N | N | Y |
| Ada consumer RTX 40xx | 8.9 | Y | Y | Y | Y | N | N | Y |
| Ada datacenter L40/L40S | 8.9 | Y | Y | Y | Y | N | N | Y |
| Blackwell consumer RTX 50xx | 10.0 | Y | Y | Y | Y | N | Y | Y |
| Blackwell datacenter B100/B200 | 10.0 | Y | Y | Y | Y | N | Y | Y |

---

| vendor_id | device_id | model_name | arch | fp32 | fp16 | bf16 | fp8 | fp4 | nvfp4 | flash_attn |
|---|---|---|---|---|---|---|---|---|---|---|
| 0x10DE | 0x1B00 | NVIDIA TITAN X (Pascal) | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1B02 | NVIDIA TITAN Xp | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1B80 | NVIDIA GeForce GTX 1080 Ti | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1B81 | NVIDIA GeForce GTX 1080 | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1B82 | NVIDIA GeForce GTX 1070 Ti | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1B84 | NVIDIA GeForce GTX 1070 | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1B83 | NVIDIA GeForce GTX 1060 6GB | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1C02 | NVIDIA GeForce GTX 1060 3GB | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1C82 | NVIDIA GeForce GTX 1050 Ti | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1C81 | NVIDIA GeForce GTX 1050 | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1CB3 | NVIDIA Quadro P4000 | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1CB1 | NVIDIA Quadro P2000 | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1CA8 | NVIDIA Quadro P5000 | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1CB2 | NVIDIA Quadro P3000 | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x2182 | NVIDIA GeForce GTX 1660 Ti | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x21C4 | NVIDIA GeForce GTX 1660 Super | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x2184 | NVIDIA GeForce GTX 1660 | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x2187 | NVIDIA GeForce GTX 1650 Super | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1F82 | NVIDIA GeForce GTX 1650 | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1E02 | NVIDIA GeForce RTX 2080 Ti | 7.5 | N | Y | N | N | N | N | N |
| 0x10DE | 0x1E84 | NVIDIA GeForce RTX 2080 Super | 7.5 | N | Y | N | N | N | N | N |
| 0x10DE | 0x1E04 | NVIDIA GeForce RTX 2080 | 7.5 | N | Y | N | N | N | N | N |
| 0x10DE | 0x1F06 | NVIDIA GeForce RTX 2070 Super | 7.5 | N | Y | N | N | N | N | N |
| 0x10DE | 0x1F02 | NVIDIA GeForce RTX 2070 | 7.5 | N | Y | N | N | N | N | N |
| 0x10DE | 0x1F47 | NVIDIA GeForce RTX 2060 Super | 7.5 | N | Y | N | N | N | N | N |
| 0x10DE | 0x1F08 | NVIDIA GeForce RTX 2060 | 7.5 | N | Y | N | N | N | N | N |
| 0x10DE | 0x1E30 | NVIDIA Quadro RTX 6000 | 7.5 | N | Y | N | N | N | N | N |
| 0x10DE | 0x1E78 | NVIDIA Quadro RTX 8000 | 7.5 | N | Y | N | N | N | N | N |
| 0x10DE | 0x20B2 | NVIDIA A100-SXM4-80GB | 8.0 | Y | Y | Y | N | N | N | Y |
| 0x10DE | 0x20B0 | NVIDIA A100-SXM4-40GB | 8.0 | Y | Y | Y | N | N | N | Y |
| 0x10DE | 0x20B5 | NVIDIA A100-PCIe-80GB | 8.0 | Y | Y | Y | N | N | N | Y |
| 0x10DE | 0x20F1 | NVIDIA A100-PCIe-40GB | 8.0 | Y | Y | Y | N | N | N | Y |
| 0x10DE | 0x20B8 | NVIDIA A30 | 8.0 | Y | Y | Y | N | N | N | Y |
| 0x10DE | 0x2208 | NVIDIA GeForce RTX 3090 Ti | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x2204 | NVIDIA GeForce RTX 3090 | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x2216 | NVIDIA GeForce RTX 3080 Ti | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x2206 | NVIDIA GeForce RTX 3080 | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x2482 | NVIDIA GeForce RTX 3070 Ti | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x2484 | NVIDIA GeForce RTX 3070 | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x2489 | NVIDIA GeForce RTX 3060 Ti | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x2503 | NVIDIA GeForce RTX 3060 | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x2571 | NVIDIA GeForce RTX 3050 | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x2230 | NVIDIA RTX A6000 | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x2233 | NVIDIA RTX A5000 | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x2235 | NVIDIA RTX A4000 | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x2236 | NVIDIA RTX A4500 | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x2322 | NVIDIA H100-SXM5-80GB | 9.0 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x2330 | NVIDIA H100-PCIe-80GB | 9.0 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x2324 | NVIDIA H800-SXM5-80GB | 9.0 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x2336 | NVIDIA H20 | 9.0 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x2684 | NVIDIA GeForce RTX 4090 | 8.9 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x2702 | NVIDIA GeForce RTX 4080 | 8.9 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x2704 | NVIDIA GeForce RTX 4080 Super | 8.9 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x2782 | NVIDIA GeForce RTX 4070 Ti | 8.9 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x2783 | NVIDIA GeForce RTX 4070 Ti Super | 8.9 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x2788 | NVIDIA GeForce RTX 4070 Super | 8.9 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x2786 | NVIDIA GeForce RTX 4070 | 8.9 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x2803 | NVIDIA GeForce RTX 4060 Ti 16GB | 8.9 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x2805 | NVIDIA GeForce RTX 4060 Ti 8GB | 8.9 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x2882 | NVIDIA GeForce RTX 4060 | 8.9 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x28A0 | NVIDIA GeForce RTX 4050 | 8.9 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x26B5 | NVIDIA L40S | 8.9 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x26B9 | NVIDIA L40 | 8.9 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x26F5 | NVIDIA L4 | 8.9 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x27B0 | NVIDIA RTX 6000 Ada | 8.9 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x27B8 | NVIDIA RTX 5000 Ada | 8.9 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x27B6 | NVIDIA RTX 4500 Ada | 8.9 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x27B2 | NVIDIA RTX 4000 Ada | 8.9 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x2B85 | NVIDIA GeForce RTX 5090 | 10.0 | Y | Y | Y | Y | N | Y | Y |
| 0x10DE | 0x2B87 | NVIDIA GeForce RTX 5080 | 10.0 | Y | Y | Y | Y | N | Y | Y |
| 0x10DE | 0x2C02 | NVIDIA GeForce RTX 5070 Ti | 10.0 | Y | Y | Y | Y | N | Y | Y |
| 0x10DE | 0x2C05 | NVIDIA GeForce RTX 5070 | 10.0 | Y | Y | Y | Y | N | Y | Y |
| 0x10DE | 0x2C82 | NVIDIA GeForce RTX 5060 Ti | 10.0 | Y | Y | Y | Y | N | Y | Y |
| 0x10DE | 0x2C87 | NVIDIA GeForce RTX 5060 | 10.0 | Y | Y | Y | Y | N | Y | Y |
| 0x10DE | 0x2B02 | NVIDIA B200 | 10.0 | Y | Y | Y | Y | N | Y | Y |
| 0x10DE | 0x2B03 | NVIDIA B100 | 10.0 | Y | Y | Y | Y | N | Y | Y |
| 0x10DE | 0x2B06 | NVIDIA B40 | 10.0 | Y | Y | Y | Y | N | Y | Y |

---

## AMD Devices

**Architecture capability summary by generation:**

| Generation | arch | fp32 | fp16 | bf16 | fp8 | fp4 | nvfp4 | flash_attn |
|---|---|---|---|---|---|---|---|---|
| RDNA 1 | gfx1010/1011/1012 | N | N | N | N | N | N | N |
| RDNA 2 | gfx1030/1031/1032/1034 | N | Y | Y | N | N | N | Y |
| RDNA 3 | gfx1100/1101/1102 | N | Y | Y | N | N | N | Y |
| RDNA 4 | gfx1200/1201 | N | Y | Y | Y | N | N | Y |
| CDNA 1 | gfx908 | N | Y | N | N | N | N | Y |
| CDNA 2 | gfx90a | N | Y | Y | N | N | N | Y |
| CDNA 3 | gfx942 | Y | Y | Y | Y | N | N | Y |
| CDNA 4 | gfx950 | Y | Y | Y | Y | Y | N | Y |

**Notes on specific entries:**
- Navi24 (gfx1034): RX 6500 XT and RX 6400 are cut-down dies with only 4
  shader engines. ROCm matrix core support is present but throughput is
  marginal. `bf16 = false, flash_attn = false` — these cards are below the
  practical inference threshold.
- RX 7400 (Navi33 cut-down): same rationale as Navi24. `bf16 = false`.
- CDNA1 MI100 (gfx908): bf16 not in matrix core ISA for this generation.
  fp8 absent. Flash attention via ROCm compute paths confirmed.
- CDNA2 MI200 (gfx90a): fp8 absent per ROCm MI300 architecture docs which
  explicitly state CDNA2 matrix cores support FP16 and BF16 only.
  fp32 TF32-equivalent matrix path absent on CDNA2.
- CDNA4 (gfx950): MI350X and MI355X. fp4 native per AMD MXFP4 matrix core
  support announced in CDNA4 architecture. nvfp4 = false (AMD has no
  NVIDIA fp4 format).

---

| vendor_id | device_id | model_name | arch | fp32 | fp16 | bf16 | fp8 | fp4 | nvfp4 | flash_attn |
|---|---|---|---|---|---|---|---|---|---|---|
| 0x1002 | 0x731F | AMD Radeon RX 5700 XT | gfx1010 | N | N | N | N | N | N | N |
| 0x1002 | 0x7312 | AMD Radeon RX 5700 | gfx1010 | N | N | N | N | N | N | N |
| 0x1002 | 0x7310 | AMD Radeon RX 5700 XT 50th | gfx1010 | N | N | N | N | N | N | N |
| 0x1002 | 0x7318 | AMD Radeon RX 5600 XT | gfx1010 | N | N | N | N | N | N | N |
| 0x1002 | 0x7360 | AMD Radeon Pro V520 | gfx1011 | N | N | N | N | N | N | N |
| 0x1002 | 0x7340 | AMD Radeon RX 5500 XT | gfx1012 | N | N | N | N | N | N | N |
| 0x1002 | 0x7341 | AMD Radeon RX 5500 | gfx1012 | N | N | N | N | N | N | N |
| 0x1002 | 0x7347 | AMD Radeon RX 5300 | gfx1012 | N | N | N | N | N | N | N |
| 0x1002 | 0x7362 | AMD Radeon Pro W5700 | gfx1012 | N | N | N | N | N | N | N |
| 0x1002 | 0x73BF | AMD Radeon RX 6900 XT | gfx1030 | N | Y | Y | N | N | N | Y |
| 0x1002 | 0x73AF | AMD Radeon RX 6950 XT | gfx1030 | N | Y | Y | N | N | N | Y |
| 0x1002 | 0x73A5 | AMD Radeon RX 6800 XT | gfx1030 | N | Y | Y | N | N | N | Y |
| 0x1002 | 0x73AB | AMD Radeon RX 6800 | gfx1030 | N | Y | Y | N | N | N | Y |
| 0x1002 | 0x73A1 | AMD Radeon Pro V620 | gfx1030 | N | Y | Y | N | N | N | Y |
| 0x1002 | 0x73A3 | AMD Radeon Pro W6800 | gfx1030 | N | Y | Y | N | N | N | Y |
| 0x1002 | 0x73DF | AMD Radeon RX 6750 XT | gfx1031 | N | Y | Y | N | N | N | Y |
| 0x1002 | 0x73DA | AMD Radeon RX 6700 XT | gfx1031 | N | Y | Y | N | N | N | Y |
| 0x1002 | 0x73DC | AMD Radeon RX 6700 | gfx1031 | N | Y | Y | N | N | N | Y |
| 0x1002 | 0x73E1 | AMD Radeon Pro W6600 | gfx1032 | N | Y | Y | N | N | N | Y |
| 0x1002 | 0x73FF | AMD Radeon RX 6650 XT | gfx1032 | N | Y | Y | N | N | N | Y |
| 0x1002 | 0x73EF | AMD Radeon RX 6600 XT | gfx1032 | N | Y | Y | N | N | N | Y |
| 0x1002 | 0x73F0 | AMD Radeon RX 6600 | gfx1032 | N | Y | Y | N | N | N | Y |
| 0x1002 | 0x7422 | AMD Radeon RX 6500 XT | gfx1034 | N | Y | N | N | N | N | N |
| 0x1002 | 0x7424 | AMD Radeon RX 6400 | gfx1034 | N | Y | N | N | N | N | N |
| 0x1002 | 0x744C | AMD Radeon RX 7900 XTX | gfx1100 | N | Y | Y | N | N | N | Y |
| 0x1002 | 0x7448 | AMD Radeon RX 7900 XT | gfx1100 | N | Y | Y | N | N | N | Y |
| 0x1002 | 0x745E | AMD Radeon RX 7900 GRE | gfx1100 | N | Y | Y | N | N | N | Y |
| 0x1002 | 0x7461 | AMD Radeon Pro W7900 | gfx1100 | N | Y | Y | N | N | N | Y |
| 0x1002 | 0x7470 | AMD Radeon RX 7800 XT | gfx1101 | N | Y | Y | N | N | N | Y |
| 0x1002 | 0x747E | AMD Radeon RX 7700 XT | gfx1101 | N | Y | Y | N | N | N | Y |
| 0x1002 | 0x7490 | AMD Radeon Pro W7700 | gfx1101 | N | Y | Y | N | N | N | Y |
| 0x1002 | 0x7483 | AMD Radeon RX 7600 XT | gfx1102 | N | Y | Y | N | N | N | Y |
| 0x1002 | 0x7480 | AMD Radeon RX 7600 | gfx1102 | N | Y | Y | N | N | N | Y |
| 0x1002 | 0x7489 | AMD Radeon RX 7700 | gfx1102 | N | Y | Y | N | N | N | Y |
| 0x1002 | 0x7499 | AMD Radeon RX 7400 | gfx1102 | N | Y | N | N | N | N | N |
| 0x1002 | 0x7452 | AMD Radeon Pro W7800 | gfx1100 | N | Y | Y | N | N | N | Y |
| 0x1002 | 0x7550 | AMD Radeon RX 9070 XT | gfx1201 | N | Y | Y | Y | N | N | Y |
| 0x1002 | 0x7551 | AMD Radeon AI PRO R9700 | gfx1201 | N | Y | Y | Y | N | N | Y |
| 0x1002 | 0x7590 | AMD Radeon RX 9060 XT | gfx1200 | N | Y | Y | Y | N | N | Y |
| 0x1002 | 0x738C | AMD Instinct MI100 | gfx908 | N | Y | N | N | N | N | Y |
| 0x1002 | 0x7388 | AMD Instinct MI100 (alt SKU) | gfx908 | N | Y | N | N | N | N | Y |
| 0x1002 | 0x7408 | AMD Instinct MI250X | gfx90a | N | Y | Y | N | N | N | Y |
| 0x1002 | 0x740C | AMD Instinct MI250 | gfx90a | N | Y | Y | N | N | N | Y |
| 0x1002 | 0x7410 | AMD Instinct MI210 | gfx90a | N | Y | Y | N | N | N | Y |
| 0x1002 | 0x74A0 | AMD Instinct MI300A | gfx942 | Y | Y | Y | Y | N | N | Y |
| 0x1002 | 0x74A1 | AMD Instinct MI300X | gfx942 | Y | Y | Y | Y | N | N | Y |
| 0x1002 | 0x74B5 | AMD Instinct MI325X | gfx942 | Y | Y | Y | Y | N | N | Y |
| 0x1002 | 0x74C0 | AMD Instinct MI350X | gfx950 | Y | Y | Y | Y | Y | N | Y |
| 0x1002 | 0x74C1 | AMD Instinct MI355X | gfx950 | Y | Y | Y | Y | Y | N | Y |

---

## Known omissions

The following device classes are intentionally excluded. They resolve to the
Fallback path at runtime; the `WARN` log entry with the PCI ID is the signal
to add them here.

- All mobile/laptop GPU variants (device IDs with M suffix behaviour in
  subsystem IDs) — same arch and capability as desktop counterpart; add the
  specific device_id if a mobile variant produces a WARN log
- Navi24 mobile (0x7421, 0x7423, 0x7425) — sub-entry OEM only
- Navi21/22/23 mobile variants — same gfx1030/1031/1032 caps as desktop
- RDNA4 RX 9070 non-XT and RX 9060 non-XT — device IDs not yet in public
  driver sources at document date
- NVIDIA Quadro P-series mobile, Tesla, and legacy server parts below SM 6.0
- RTX A-series mobile (A3000/A4000/A5000 laptop) — same SM 8.6 caps as
  desktop counterparts; add device_id on WARN

---

## Migration DDL reference

The `device_capabilities` table is created by
`backend/migrations/004_device_capabilities.sql`:

```sql
CREATE TABLE IF NOT EXISTS device_capabilities (
    vendor_id       INTEGER NOT NULL,
    device_id       INTEGER NOT NULL,
    model_name      TEXT    NOT NULL,
    arch            TEXT    NOT NULL,
    fp32            INTEGER NOT NULL DEFAULT 0,
    fp16            INTEGER NOT NULL DEFAULT 0,
    bf16            INTEGER NOT NULL DEFAULT 0,
    fp8             INTEGER NOT NULL DEFAULT 0,
    fp4             INTEGER NOT NULL DEFAULT 0,
    nvfp4           INTEGER NOT NULL DEFAULT 0,
    flash_attn      INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (vendor_id, device_id)
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_device_capabilities_pci
    ON device_capabilities(vendor_id, device_id);
```

Booleans are stored as `INTEGER 0/1`. The `DeviceCapabilityRow` Rust struct
maps `bool` ↔ `i64` at the store boundary (`field as i64` / `value != 0`).