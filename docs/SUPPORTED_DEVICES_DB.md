# Supported Devices Database

**Purpose:** Authoritative reference for the `device_capabilities` SQLite table
seed data. Consumed by Forge task P7-F1 (migration DDL) and P7-F2 (store
schema). The two Markdown tables below are the source of truth for all
`DeviceCapabilityRow` values seeded at startup.

**Sources:**
- NVIDIA: Ada GPU Architecture whitepaper v2.02, Blackwell architecture docs,
  CUDA Toolkit documentation, PyTorch CUDA semantics docs, TensorRT-LLM
  release notes, torchao releases, pciutils/pciids database
- AMD: ROCm 7.2.2 Data Types and Precision Support reference
  (rocm.docs.amd.com/en/latest/reference/precision-support.html),
  MI300 microarchitecture docs, Linux kernel `amdgpu_drv.c`, OpenBSD `pcidevs`,
  pciutils/pciids database
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
- GTX 16xx series (TU116/TU117) and T-series workstation GPUs (Turing SM 7.5)
  have no fp16 tensor cores; `fp16 = false` for those entries.
- `model_name` entries covering multiple marketed SKUs sharing one PCI device_id
  are formatted as `Base/Variant1/Variant2`.
- Mobile/laptop GPU variants sharing the same device_id as their desktop
  counterpart are listed in the same collection name where applicable.

---

## NVIDIA Devices

**Architecture capability summary by generation:**

| Generation | SM | fp32 | fp16 | bf16 | fp8 | fp4 | nvfp4 | flash_attn |
|---|---|---|---|---|---|---|---|---|
| Pascal | 6.1 | N | N | N | N | N | N | N |
| Turing GTX 16xx / T-series | 7.5 | N | N | N | N | N | N | N |
| Turing RTX 20xx | 7.5 | N | Y | N | N | N | N | N |
| Ampere datacenter GA100 | 8.0 | Y | Y | Y | N | N | N | Y |
| Ampere consumer RTX 30xx | 8.6 | Y | Y | N | N | N | N | Y |
| Hopper GH100 | 9.0 | Y | Y | Y | Y | N | N | Y |
| Ada consumer RTX 40xx | 8.9 | Y | Y | Y | Y | N | N | Y |
| Ada datacenter L40/L40S | 8.9 | Y | Y | Y | Y | N | N | Y |
| Blackwell consumer RTX 50xx | 10.0 | Y | Y | Y | Y | N | Y | Y |
| Blackwell datacenter / RTX PRO | 10.0 | Y | Y | Y | Y | N | Y | Y |

---

| vendor_id | device_id | model_name | arch | fp32 | fp16 | bf16 | fp8 | fp4 | nvfp4 | flash_attn |
|---|---|---|---|---|---|---|---|---|---|---|
| 0x10DE | 0x1B00 | NVIDIA TITAN X (Pascal) | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1B02 | NVIDIA TITAN Xp | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1B06 | NVIDIA GeForce GTX 1080 Ti | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1B30 | NVIDIA Quadro P6000 | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1B38 | NVIDIA Tesla P40 | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1B80 | NVIDIA GeForce GTX 1080 | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1B81 | NVIDIA GeForce GTX 1070 | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1B82 | NVIDIA GeForce GTX 1070 Ti | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1B83 | NVIDIA GeForce GTX 1060 6GB | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1B84 | NVIDIA GeForce GTX 1060 3GB | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1BA0 | NVIDIA GeForce GTX 1080 Mobile | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1BA1 | NVIDIA GeForce GTX 1070 Mobile | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1BB0 | NVIDIA Quadro P5000 | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1BB1 | NVIDIA Quadro P4000 | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1BB3 | NVIDIA Tesla P4 | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1BB5 | NVIDIA Quadro P5200 Mobile | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1BB6 | NVIDIA Quadro P5000 Mobile | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1BB7 | NVIDIA Quadro P4000 Mobile | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1BB8 | NVIDIA Quadro P3000 Mobile | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1BB9 | NVIDIA Quadro P4200 Mobile | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1BBB | NVIDIA Quadro P3200 Mobile | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1BE0 | NVIDIA GeForce GTX 1080 Mobile | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1BE1 | NVIDIA GeForce GTX 1070 Mobile | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1C02 | NVIDIA GeForce GTX 1060 3GB | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1C04 | NVIDIA GeForce GTX 1060 5GB | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1C06 | NVIDIA GeForce GTX 1060 6GB Rev. 2 | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1C30 | NVIDIA Quadro P2000 | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1C31 | NVIDIA Quadro P2200 | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1C60 | NVIDIA GeForce GTX 1060 Mobile 6GB | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1C61 | NVIDIA GeForce GTX 1050 Ti Mobile | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1C62 | NVIDIA GeForce GTX 1050 Mobile | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1C81 | NVIDIA GeForce GTX 1050 | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1C82 | NVIDIA GeForce GTX 1050 Ti | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1C83 | NVIDIA GeForce GTX 1050 3GB | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1C8C | NVIDIA GeForce GTX 1050 Ti Mobile | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1C8D | NVIDIA GeForce GTX 1050 Mobile | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1C8F | NVIDIA GeForce GTX 1050 Ti Max-Q | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1C90 | NVIDIA GeForce MX150 | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1C92 | NVIDIA GeForce GTX 1050 Mobile | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1C94 | NVIDIA GeForce MX350 | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1CB1 | NVIDIA Quadro P1000 | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1CB2 | NVIDIA Quadro P600 | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1CB3 | NVIDIA Quadro P400 | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1CB6 | NVIDIA Quadro P620 | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1CBA | NVIDIA Quadro P2000 Mobile | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1CBB | NVIDIA Quadro P1000 Mobile | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1CBC | NVIDIA Quadro P600 Mobile | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1CBD | NVIDIA Quadro P620 Mobile | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1CCC | NVIDIA GeForce GTX 1050 Ti Mobile | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1CCD | NVIDIA GeForce GTX 1050 Mobile | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1CFA | NVIDIA Quadro P2000 | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1CFB | NVIDIA Quadro P1000 | 6.1 | N | N | N | N | N | N | N |
| 0x10DE | 0x1F0A | NVIDIA GeForce GTX 1650 | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1F82 | NVIDIA GeForce GTX 1650 | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1F83 | NVIDIA GeForce GTX 1630 | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1F91 | NVIDIA GeForce GTX 1650 Mobile | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1F92 | NVIDIA GeForce GTX 1650 Mobile | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1F95 | NVIDIA GeForce GTX 1650 Ti Mobile | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1F96 | NVIDIA GeForce GTX 1650 Mobile | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1F97 | NVIDIA GeForce MX450 | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1F99 | NVIDIA GeForce GTX 1650 Mobile | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1F9F | NVIDIA GeForce MX550 | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1FAE | NVIDIA T1000 | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1FB0 | NVIDIA Quadro T1000 Mobile | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1FB1 | NVIDIA T600 | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1FB6 | NVIDIA T600 Laptop GPU | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1FB7 | NVIDIA T550 Laptop GPU | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1FB8 | NVIDIA Quadro T2000 Mobile | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1FB9 | NVIDIA Quadro T1000 Mobile | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1FBA | NVIDIA T600 Mobile | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1FBB | NVIDIA Quadro T500 Mobile | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1FBC | NVIDIA T1200 Laptop GPU | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1FBF | NVIDIA T1000 | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1FD9 | NVIDIA GeForce GTX 1650 Mobile Refresh | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1FF0 | NVIDIA T1000 8GB | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1FF2 | NVIDIA T400 | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1FF9 | NVIDIA Quadro T1000 Mobile | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x2182 | NVIDIA GeForce GTX 1660 Ti | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x2184 | NVIDIA GeForce GTX 1660 | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x2187 | NVIDIA GeForce GTX 1650 SUPER | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x21C4 | NVIDIA GeForce GTX 1660 SUPER | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1E02 | NVIDIA TITAN RTX | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1E04 | NVIDIA GeForce RTX 2080 Ti | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1E30 | NVIDIA Quadro RTX 6000/8000 | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1E36 | NVIDIA Quadro RTX 6000 | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1E38 | NVIDIA Tesla T40 24GB | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1E78 | NVIDIA Quadro RTX 8000 | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1E81 | NVIDIA GeForce RTX 2080 SUPER | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1E82 | NVIDIA GeForce RTX 2080 | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1E84 | NVIDIA GeForce RTX 2070 SUPER | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1E89 | NVIDIA GeForce RTX 2060 | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1E90 | NVIDIA GeForce RTX 2080 Mobile | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1E91 | NVIDIA GeForce RTX 2070 SUPER Mobile | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1E93 | NVIDIA GeForce RTX 2080 SUPER Mobile | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1EB0 | NVIDIA Quadro RTX 5000 | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1EB1 | NVIDIA Quadro RTX 4000 | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1EB5 | NVIDIA Quadro RTX 5000 Mobile | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1EB6 | NVIDIA Quadro RTX 4000 Mobile | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1EB8 | NVIDIA Tesla T4 | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1EC2 | NVIDIA GeForce RTX 2070 SUPER | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1ED0 | NVIDIA GeForce RTX 2080 Mobile | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1ED1 | NVIDIA GeForce RTX 2070 SUPER Mobile | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1ED3 | NVIDIA GeForce RTX 2080 SUPER Mobile | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1EF5 | NVIDIA Quadro RTX 5000 Mobile Refresh | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1F02 | NVIDIA GeForce RTX 2070 | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1F03 | NVIDIA GeForce RTX 2060 12GB | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1F06 | NVIDIA GeForce RTX 2060 SUPER | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1F08 | NVIDIA GeForce RTX 2060 | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1F09 | NVIDIA GeForce GTX 1660 SUPER | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1F10 | NVIDIA GeForce RTX 2070 Mobile | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1F11 | NVIDIA GeForce RTX 2060 Mobile | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1F12 | NVIDIA GeForce RTX 2060 Max-Q | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1F14 | NVIDIA GeForce RTX 2070 Mobile Refresh | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1F15 | NVIDIA GeForce RTX 2060 Mobile | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1F36 | NVIDIA Quadro RTX 3000 Mobile | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1F42 | NVIDIA GeForce RTX 2060 SUPER | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1F47 | NVIDIA GeForce RTX 2060 SUPER | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1F50 | NVIDIA GeForce RTX 2070 Mobile | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1F51 | NVIDIA GeForce RTX 2060 Mobile | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1F54 | NVIDIA GeForce RTX 2070 Mobile | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1F55 | NVIDIA GeForce RTX 2060 Mobile | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x1F76 | NVIDIA Quadro RTX 3000 Mobile Refresh | 7.5 | N | N | N | N | N | N | N |
| 0x10DE | 0x20B0 | NVIDIA A100-SXM4-40GB | 8.0 | Y | Y | Y | N | N | N | Y |
| 0x10DE | 0x20B2 | NVIDIA A100-SXM4-80GB | 8.0 | Y | Y | Y | N | N | N | Y |
| 0x10DE | 0x20B5 | NVIDIA A100-PCIe-80GB | 8.0 | Y | Y | Y | N | N | N | Y |
| 0x10DE | 0x20B7 | NVIDIA A30 | 8.0 | Y | Y | Y | N | N | N | Y |
| 0x10DE | 0x20B8 | NVIDIA A100X | 8.0 | Y | Y | Y | N | N | N | Y |
| 0x10DE | 0x20F1 | NVIDIA A100-PCIe-40GB | 8.0 | Y | Y | Y | N | N | N | Y |
| 0x10DE | 0x20F5 | NVIDIA A800-80GB | 8.0 | Y | Y | Y | N | N | N | Y |
| 0x10DE | 0x20F6 | NVIDIA A800-40GB | 8.0 | Y | Y | Y | N | N | N | Y |
| 0x10DE | 0x2203 | NVIDIA GeForce RTX 3090 Ti | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x2204 | NVIDIA GeForce RTX 3090 | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x2206 | NVIDIA GeForce RTX 3080 | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x2207 | NVIDIA GeForce RTX 3070 Ti | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x2208 | NVIDIA GeForce RTX 3090 Ti | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x220A | NVIDIA GeForce RTX 3080 12GB | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x2216 | NVIDIA GeForce RTX 3080 Ti | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x2230 | NVIDIA RTX A6000 | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x2231 | NVIDIA RTX A5000 | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x2232 | NVIDIA RTX A4500 | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x2233 | NVIDIA RTX A5500 | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x2235 | NVIDIA A40 | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x2236 | NVIDIA A10 | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x2237 | NVIDIA A10G | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x2414 | NVIDIA GeForce RTX 3060 Ti | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x2420 | NVIDIA GeForce RTX 3080 Ti Mobile | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x2438 | NVIDIA RTX A5500 Laptop GPU | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x2460 | NVIDIA GeForce RTX 3080 Ti Laptop GPU | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x2482 | NVIDIA GeForce RTX 3070 Ti | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x2484 | NVIDIA GeForce RTX 3070 | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x2489 | NVIDIA GeForce RTX 3060 Ti | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x248C | NVIDIA GeForce RTX 3070 Ti | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x248D | NVIDIA GeForce RTX 3070 | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x248E | NVIDIA GeForce RTX 3060 Ti | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x249C | NVIDIA GeForce RTX 3080 Mobile | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x249D | NVIDIA GeForce RTX 3070 Mobile | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x24A0 | NVIDIA GeForce RTX 3070 Ti Laptop GPU | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x24B0 | NVIDIA RTX A4000 | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x24B1 | NVIDIA RTX A4000H | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x24B6 | NVIDIA RTX A5000 Laptop GPU | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x24B7 | NVIDIA RTX A4000 Laptop GPU | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x24B8 | NVIDIA RTX A3000 Mobile | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x24B9 | NVIDIA RTX A3000 12GB Laptop GPU | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x24BA | NVIDIA RTX A4500 Laptop GPU | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x24BB | NVIDIA RTX A3000 Laptop GPU | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x24C7 | NVIDIA GeForce RTX 3060 8GB | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x24C8 | NVIDIA GeForce RTX 3070 GDDR6X | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x24C9 | NVIDIA GeForce RTX 3060 Ti GDDR6X | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x24DC | NVIDIA GeForce RTX 3080 Mobile | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x24DD | NVIDIA GeForce RTX 3070 Mobile | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x24E0 | NVIDIA GeForce RTX 3070 Ti Laptop GPU | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x2503 | NVIDIA GeForce RTX 3060 | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x2507 | NVIDIA GeForce RTX 3050 | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x2509 | NVIDIA GeForce RTX 3060 12GB Rev. 2 | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x2520 | NVIDIA GeForce RTX 3060 Mobile | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x2521 | NVIDIA GeForce RTX 3060 Laptop GPU | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x2523 | NVIDIA GeForce RTX 3050 Ti Mobile | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x2531 | NVIDIA RTX A2000 | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x2544 | NVIDIA GeForce RTX 3060 | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x2561 | NVIDIA GeForce RTX 3060 Laptop GPU | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x2563 | NVIDIA GeForce RTX 3050 Ti Mobile | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x2571 | NVIDIA RTX A2000 12GB | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x2582 | NVIDIA GeForce RTX 3050 8GB | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x2583 | NVIDIA GeForce RTX 3050 4GB | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x2584 | NVIDIA GeForce RTX 3050 6GB | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x25A0 | NVIDIA GeForce RTX 3050 Ti Mobile | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x25A2 | NVIDIA GeForce RTX 3050 Mobile | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x25A5 | NVIDIA GeForce RTX 3050 Mobile | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x25A6 | NVIDIA GeForce MX570 | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x25A9 | NVIDIA GeForce RTX 2050 | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x25AA | NVIDIA GeForce MX570 A | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x25AB | NVIDIA GeForce RTX 3050 4GB Laptop GPU | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x25AC | NVIDIA GeForce RTX 3050 6GB Laptop GPU | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x25AD | NVIDIA GeForce RTX 2050 | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x25B0 | NVIDIA RTX A1000 | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x25B2 | NVIDIA RTX A400 | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x25B5 | NVIDIA RTX A4 Mobile | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x25B6 | NVIDIA A2 / A16 | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x25B8 | NVIDIA RTX A2000 Laptop GPU | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x25B9 | NVIDIA RTX A1000 Laptop GPU | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x25BA | NVIDIA RTX A2000 8GB Laptop GPU | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x25BB | NVIDIA RTX A500 Laptop GPU | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x25BC | NVIDIA RTX A1000 6GB Laptop GPU | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x25BD | NVIDIA RTX A500 Laptop GPU | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x25E0 | NVIDIA GeForce RTX 3050 Ti Mobile | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x25E2 | NVIDIA GeForce RTX 3050 Mobile | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x25EC | NVIDIA GeForce RTX 3050 6GB Laptop GPU | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x25ED | NVIDIA GeForce RTX 2050 | 8.6 | Y | Y | N | N | N | N | Y |
| 0x10DE | 0x2322 | NVIDIA H100-SXM5-80GB | 9.0 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x2324 | NVIDIA H800-SXM5-80GB | 9.0 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x2328 | NVIDIA H20B | 9.0 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x2329 | NVIDIA H20 | 9.0 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x2330 | NVIDIA H100-PCIe-80GB | 9.0 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x2331 | NVIDIA H100 PCIe | 9.0 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x2333 | NVIDIA H100-SXM5-80GB | 9.0 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x2336 | NVIDIA H20 | 9.0 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x233A | NVIDIA H800L-94GB | 9.0 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x233B | NVIDIA H200 NVL | 9.0 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x233D | NVIDIA H100-96GB | 9.0 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x2342 | NVIDIA GH200 120GB / 480GB | 9.0 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x2348 | NVIDIA GH200 144G HBM3e | 9.0 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x2681 | NVIDIA RTX TITAN Ada | 8.9 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x2684 | NVIDIA GeForce RTX 4090 | 8.9 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x2685 | NVIDIA GeForce RTX 4090 D | 8.9 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x2689 | NVIDIA GeForce RTX 4070 Ti SUPER | 8.9 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x26B1 | NVIDIA RTX 6000 Ada | 8.9 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x26B2 | NVIDIA RTX 5000 Ada | 8.9 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x26B3 | NVIDIA RTX 5880 Ada | 8.9 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x26B5 | NVIDIA L40 | 8.9 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x26B9 | NVIDIA L40S | 8.9 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x26F5 | NVIDIA L4 | 8.9 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x2703 | NVIDIA GeForce RTX 4080 SUPER | 8.9 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x2704 | NVIDIA GeForce RTX 4080 | 8.9 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x2705 | NVIDIA GeForce RTX 4070 Ti SUPER | 8.9 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x2709 | NVIDIA GeForce RTX 4070 | 8.9 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x2717 | NVIDIA GeForce RTX 4090 Laptop GPU | 8.9 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x2757 | NVIDIA GeForce RTX 4090 Laptop GPU | 8.9 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x2782 | NVIDIA GeForce RTX 4070 Ti | 8.9 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x2783 | NVIDIA GeForce RTX 4070 SUPER | 8.9 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x2786 | NVIDIA GeForce RTX 4070 | 8.9 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x2788 | NVIDIA GeForce RTX 4060 Ti | 8.9 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x27A0 | NVIDIA GeForce RTX 4080 Mobile | 8.9 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x27B0 | NVIDIA RTX 4000 SFF Ada | 8.9 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x27B1 | NVIDIA RTX 4500 Ada | 8.9 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x27B2 | NVIDIA RTX 4000 Ada | 8.9 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x27B6 | NVIDIA L2 | 8.9 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x27B8 | NVIDIA L4 | 8.9 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x27BA | NVIDIA RTX 4000 Ada Laptop GPU | 8.9 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x27BB | NVIDIA RTX 3500 Ada Laptop GPU | 8.9 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x27E0 | NVIDIA GeForce RTX 4080 Mobile | 8.9 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x2803 | NVIDIA GeForce RTX 4060 Ti | 8.9 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x2805 | NVIDIA GeForce RTX 4060 Ti 16GB | 8.9 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x2820 | NVIDIA GeForce RTX 4070 Mobile | 8.9 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x2860 | NVIDIA GeForce RTX 4070 Mobile | 8.9 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x2882 | NVIDIA GeForce RTX 4060 | 8.9 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x28A0 | NVIDIA GeForce RTX 4060 Mobile | 8.9 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x28A1 | NVIDIA GeForce RTX 4050 Mobile | 8.9 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x28B0 | NVIDIA RTX 2000 Ada | 8.9 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x28E0 | NVIDIA GeForce RTX 4060 Mobile | 8.9 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x28E1 | NVIDIA GeForce RTX 4050 Mobile | 8.9 | Y | Y | Y | Y | N | N | Y |
| 0x10DE | 0x2B02 | NVIDIA B200 | 10.0 | Y | Y | Y | Y | N | Y | Y |
| 0x10DE | 0x2B03 | NVIDIA B100 | 10.0 | Y | Y | Y | Y | N | Y | Y |
| 0x10DE | 0x2B06 | NVIDIA B40 | 10.0 | Y | Y | Y | Y | N | Y | Y |
| 0x10DE | 0x2B85 | NVIDIA GeForce RTX 5090 | 10.0 | Y | Y | Y | Y | N | Y | Y |
| 0x10DE | 0x2B87 | NVIDIA GeForce RTX 5090 D | 10.0 | Y | Y | Y | Y | N | Y | Y |
| 0x10DE | 0x2BB1 | NVIDIA RTX PRO 6000 Blackwell | 10.0 | Y | Y | Y | Y | N | Y | Y |
| 0x10DE | 0x2BB3 | NVIDIA RTX PRO 5000 Blackwell | 10.0 | Y | Y | Y | Y | N | Y | Y |
| 0x10DE | 0x2BB5 | NVIDIA RTX PRO 6000 Blackwell Server | 10.0 | Y | Y | Y | Y | N | Y | Y |
| 0x10DE | 0x2BB9 | NVIDIA RTX 6000D | 10.0 | Y | Y | Y | Y | N | Y | Y |
| 0x10DE | 0x2C02 | NVIDIA GeForce RTX 5080 | 10.0 | Y | Y | Y | Y | N | Y | Y |
| 0x10DE | 0x2C05 | NVIDIA GeForce RTX 5070 Ti | 10.0 | Y | Y | Y | Y | N | Y | Y |
| 0x10DE | 0x2C18 | NVIDIA GeForce RTX 5090 Mobile | 10.0 | Y | Y | Y | Y | N | Y | Y |
| 0x10DE | 0x2C19 | NVIDIA GeForce RTX 5080 Mobile | 10.0 | Y | Y | Y | Y | N | Y | Y |
| 0x10DE | 0x2C31 | NVIDIA RTX PRO 4500 Blackwell | 10.0 | Y | Y | Y | Y | N | Y | Y |
| 0x10DE | 0x2C34 | NVIDIA RTX PRO 4000 Blackwell | 10.0 | Y | Y | Y | Y | N | Y | Y |
| 0x10DE | 0x2C58 | NVIDIA GeForce RTX 5090 Mobile | 10.0 | Y | Y | Y | Y | N | Y | Y |
| 0x10DE | 0x2C59 | NVIDIA GeForce RTX 5080 Mobile | 10.0 | Y | Y | Y | Y | N | Y | Y |
| 0x10DE | 0x2C82 | NVIDIA GeForce RTX 5060 Ti | 10.0 | Y | Y | Y | Y | N | Y | Y |
| 0x10DE | 0x2C87 | NVIDIA GeForce RTX 5060 | 10.0 | Y | Y | Y | Y | N | Y | Y |
| 0x10DE | 0x2D04 | NVIDIA GeForce RTX 5060 Ti | 10.0 | Y | Y | Y | Y | N | Y | Y |
| 0x10DE | 0x2D05 | NVIDIA GeForce RTX 5060 | 10.0 | Y | Y | Y | Y | N | Y | Y |
| 0x10DE | 0x2D18 | NVIDIA GeForce RTX 5070 Mobile | 10.0 | Y | Y | Y | Y | N | Y | Y |
| 0x10DE | 0x2D19 | NVIDIA GeForce RTX 5060 Mobile | 10.0 | Y | Y | Y | Y | N | Y | Y |
| 0x10DE | 0x2D30 | NVIDIA RTX PRO 2000 Blackwell | 10.0 | Y | Y | Y | Y | N | Y | Y |
| 0x10DE | 0x2D58 | NVIDIA GeForce RTX 5070 Mobile | 10.0 | Y | Y | Y | Y | N | Y | Y |
| 0x10DE | 0x2D59 | NVIDIA GeForce RTX 5060 Mobile | 10.0 | Y | Y | Y | Y | N | Y | Y |
| 0x10DE | 0x2D83 | NVIDIA GeForce RTX 5050 | 10.0 | Y | Y | Y | Y | N | Y | Y |
| 0x10DE | 0x2D98 | NVIDIA GeForce RTX 5050 Mobile | 10.0 | Y | Y | Y | Y | N | Y | Y |
| 0x10DE | 0x2DD8 | NVIDIA GeForce RTX 5050 Mobile | 10.0 | Y | Y | Y | Y | N | Y | Y |
| 0x10DE | 0x2F04 | NVIDIA GeForce RTX 5070 | 10.0 | Y | Y | Y | Y | N | Y | Y |
| 0x10DE | 0x2F18 | NVIDIA GeForce RTX 5070 Ti Mobile | 10.0 | Y | Y | Y | Y | N | Y | Y |
| 0x10DE | 0x2F58 | NVIDIA GeForce RTX 5070 Ti Mobile | 10.0 | Y | Y | Y | Y | N | Y | Y |

---

## AMD Devices

**Architecture capability summary by generation:**

| Generation | arch | fp32 | fp16 | bf16 | fp8 | fp4 | nvfp4 | flash_attn |
|---|---|---|---|---|---|---|---|---|
| RDNA 1 | gfx1010/1011/1012 | N | N | N | N | N | N | N |
| RDNA 2 | gfx1030/1031/1032/1034 | N | Y | Y | N | N | N | Y |
| RDNA 2 (Navi24 cut-down) | gfx1034 | N | Y | N | N | N | N | N |
| RDNA 3 | gfx1100/1101/1102 | N | Y | Y | N | N | N | Y |
| RDNA 3 (cut-down) | gfx1102 | N | Y | N | N | N | N | N |
| RDNA 4 | gfx1200/1201 | N | Y | Y | Y | N | N | Y |
| CDNA 1 | gfx908 | N | Y | N | N | N | N | Y |
| CDNA 2 | gfx90a | N | Y | Y | N | N | N | Y |
| CDNA 3 | gfx942 | Y | Y | Y | Y | N | N | Y |
| CDNA 4 | gfx950 | Y | Y | Y | Y | Y | N | Y |

**Notes on specific entries:**
- Navi24 (gfx1034): W6500M, W6400, W6300/W6300M, RX 6300, and RX 6400/6500 XT/6500M
  are cut-down dies. `bf16 = false, flash_attn = false` — sub-inference-threshold.
- Navi33 RX 7400/7300/Pro W7400 (0x7499, gfx1102): same cut-down rationale.
  `bf16 = false`.
- CDNA1 MI100 (gfx908): bf16 not in matrix core ISA. fp8 absent.
- CDNA2 MI200 series (gfx90a): fp8 absent; fp32 TF32-equivalent matrix path absent.
- CDNA3 MI300 series (gfx942): includes MI300A, MI300X, MI308X, MI325X.
- CDNA4 MI350 series (gfx950): MI350X, MI355X. fp4 native via AMD MXFP4 matrix cores.
  nvfp4 = false (AMD has no NVIDIA fp4 format).
- `model_name` entries list all marketed SKUs sharing one PCI device_id.
  Mobile variants are included where they share a device_id with their desktop
  counterpart (e.g., 0x744C covers the 7900M alongside desktop 7900 variants).

---

| vendor_id | device_id | model_name | arch | fp32 | fp16 | bf16 | fp8 | fp4 | nvfp4 | flash_attn |
|---|---|---|---|---|---|---|---|---|---|---|
| 0x1002 | 0x7310 | AMD Radeon Pro W5700X | gfx1010 | N | N | N | N | N | N | N |
| 0x1002 | 0x7312 | AMD Radeon RX 5700/Radeon Pro W5700 | gfx1010 | N | N | N | N | N | N | N |
| 0x1002 | 0x7318 | AMD Radeon RX 5600 XT | gfx1010 | N | N | N | N | N | N | N |
| 0x1002 | 0x7319 | AMD Radeon Pro 5700 XT | gfx1010 | N | N | N | N | N | N | N |
| 0x1002 | 0x731B | AMD Radeon Pro 5700 | gfx1010 | N | N | N | N | N | N | N |
| 0x1002 | 0x731F | AMD Radeon RX 5700 XT | gfx1010 | N | N | N | N | N | N | N |
| 0x1002 | 0x7340 | AMD Radeon RX 5500/5500M/Pro 5300 | gfx1012 | N | N | N | N | N | N | N |
| 0x1002 | 0x7341 | AMD Radeon RX 5500/Radeon Pro W5500 | gfx1012 | N | N | N | N | N | N | N |
| 0x1002 | 0x7347 | AMD Radeon RX 5300/Radeon Pro W5500M | gfx1012 | N | N | N | N | N | N | N |
| 0x1002 | 0x734F | AMD Radeon Pro W5300M | gfx1012 | N | N | N | N | N | N | N |
| 0x1002 | 0x7360 | AMD Radeon Pro 5600M/V520/BC-160 | gfx1011 | N | N | N | N | N | N | N |
| 0x1002 | 0x7362 | AMD Radeon Pro V520/V540 | gfx1011 | N | N | N | N | N | N | N |
| 0x1002 | 0x73A1 | AMD Radeon Pro V620 | gfx1030 | N | Y | Y | N | N | N | Y |
| 0x1002 | 0x73A3 | AMD Radeon Pro W6800 | gfx1030 | N | Y | Y | N | N | N | Y |
| 0x1002 | 0x73A5 | AMD Radeon RX 6800 XT | gfx1030 | N | Y | Y | N | N | N | Y |
| 0x1002 | 0x73AB | AMD Radeon RX 6800 | gfx1030 | N | Y | Y | N | N | N | Y |
| 0x1002 | 0x73AE | AMD Radeon Pro V620 MxGPU | gfx1030 | N | Y | Y | N | N | N | Y |
| 0x1002 | 0x73AF | AMD Radeon RX 6950 XT | gfx1030 | N | Y | Y | N | N | N | Y |
| 0x1002 | 0x73BF | AMD Radeon RX 6900 XT | gfx1030 | N | Y | Y | N | N | N | Y |
| 0x1002 | 0x73CE | AMD Radeon RX 6700 SRIOV MxGPU | gfx1031 | N | Y | Y | N | N | N | Y |
| 0x1002 | 0x73DA | AMD Radeon RX 6700 XT | gfx1031 | N | Y | Y | N | N | N | Y |
| 0x1002 | 0x73DC | AMD Radeon RX 6700 | gfx1031 | N | Y | Y | N | N | N | Y |
| 0x1002 | 0x73DF | AMD Radeon RX 6700/6700 XT/6750 XT/6800M/6850M XT | gfx1031 | N | Y | Y | N | N | N | Y |
| 0x1002 | 0x73E3 | AMD Radeon Pro W6600 | gfx1032 | N | Y | Y | N | N | N | Y |
| 0x1002 | 0x73EF | AMD Radeon RX 6650 XT/6700S/6800S | gfx1032 | N | Y | Y | N | N | N | Y |
| 0x1002 | 0x73F0 | AMD Radeon RX 6600 | gfx1032 | N | Y | Y | N | N | N | Y |
| 0x1002 | 0x73FF | AMD Radeon RX 6600/6600 XT/6600M | gfx1032 | N | Y | Y | N | N | N | Y |
| 0x1002 | 0x7421 | AMD Radeon Pro W6500M | gfx1034 | N | Y | N | N | N | N | N |
| 0x1002 | 0x7422 | AMD Radeon Pro W6400 | gfx1034 | N | Y | N | N | N | N | N |
| 0x1002 | 0x7423 | AMD Radeon Pro W6300/W6300M | gfx1034 | N | Y | N | N | N | N | N |
| 0x1002 | 0x7424 | AMD Radeon RX 6300 | gfx1034 | N | Y | N | N | N | N | N |
| 0x1002 | 0x743F | AMD Radeon RX 6400/6500 XT/6500M | gfx1034 | N | Y | N | N | N | N | N |
| 0x1002 | 0x744A | AMD Radeon Pro W7900 Dual Slot | gfx1100 | N | Y | Y | N | N | N | Y |
| 0x1002 | 0x744B | AMD Radeon Pro W7900D | gfx1100 | N | Y | Y | N | N | N | Y |
| 0x1002 | 0x744C | AMD Radeon RX 7900 XT/7900 XTX/7900 GRE/7900M | gfx1100 | N | Y | Y | N | N | N | Y |
| 0x1002 | 0x7452 | AMD Radeon Pro W7800 | gfx1100 | N | Y | Y | N | N | N | Y |
| 0x1002 | 0x745E | AMD Radeon RX 7900 GRE | gfx1100 | N | Y | Y | N | N | N | Y |
| 0x1002 | 0x7461 | AMD Radeon Pro W7900 | gfx1100 | N | Y | Y | N | N | N | Y |
| 0x1002 | 0x7470 | AMD Radeon Pro W7700 | gfx1101 | N | Y | Y | N | N | N | Y |
| 0x1002 | 0x747E | AMD Radeon RX 7700 XT/7800 XT | gfx1101 | N | Y | Y | N | N | N | Y |
| 0x1002 | 0x7480 | AMD Radeon RX 7600/7600 XT/7600M XT/7600S/7700S/Pro W7600 | gfx1102 | N | Y | Y | N | N | N | Y |
| 0x1002 | 0x7481 | AMD Radeon RX 7600 (alt) | gfx1102 | N | Y | Y | N | N | N | Y |
| 0x1002 | 0x7483 | AMD Radeon RX 7600M/7600M XT | gfx1102 | N | Y | Y | N | N | N | Y |
| 0x1002 | 0x7489 | AMD Radeon Pro W7500 | gfx1102 | N | Y | Y | N | N | N | Y |
| 0x1002 | 0x7499 | AMD Radeon RX 7400/7300/Pro W7400 | gfx1102 | N | Y | N | N | N | N | N |
| 0x1002 | 0x749F | AMD Radeon RX 7500 | gfx1102 | N | Y | Y | N | N | N | Y |
| 0x1002 | 0x7550 | AMD Radeon RX 9070/RX 9070 XT/RX 9070 GRE | gfx1201 | N | Y | Y | Y | N | N | Y |
| 0x1002 | 0x7551 | AMD Radeon AI PRO R9700 | gfx1201 | N | Y | Y | Y | N | N | Y |
| 0x1002 | 0x7590 | AMD Radeon RX 9060 XT | gfx1200 | N | Y | Y | Y | N | N | Y |
| 0x1002 | 0x7388 | AMD Instinct MI100 (alt) | gfx908 | N | Y | N | N | N | N | Y |
| 0x1002 | 0x738C | AMD Instinct MI100 | gfx908 | N | Y | N | N | N | N | Y |
| 0x1002 | 0x738E | AMD Instinct MI100 (alt 2) | gfx908 | N | Y | N | N | N | N | Y |
| 0x1002 | 0x7408 | AMD Instinct MI250X | gfx90a | N | Y | Y | N | N | N | Y |
| 0x1002 | 0x740C | AMD Instinct MI250 | gfx90a | N | Y | Y | N | N | N | Y |
| 0x1002 | 0x7410 | AMD Instinct MI210 | gfx90a | N | Y | Y | N | N | N | Y |
| 0x1002 | 0x74A0 | AMD Instinct MI300A | gfx942 | Y | Y | Y | Y | N | N | Y |
| 0x1002 | 0x74A1 | AMD Instinct MI300X | gfx942 | Y | Y | Y | Y | N | N | Y |
| 0x1002 | 0x74A2 | AMD Instinct MI308X | gfx942 | Y | Y | Y | Y | N | N | Y |
| 0x1002 | 0x74A5 | AMD Instinct MI325X | gfx942 | Y | Y | Y | Y | N | N | Y |
| 0x1002 | 0x75A0 | AMD Instinct MI350X | gfx950 | Y | Y | Y | Y | Y | N | Y |
| 0x1002 | 0x75A3 | AMD Instinct MI355X | gfx950 | Y | Y | Y | Y | Y | N | Y |

---

## Known omissions

The following device classes are intentionally excluded. They resolve to the
Fallback path at runtime; the `WARN` log entry with the PCI ID is the signal
to add them here.

- NVIDIA Quadro P-series mobile and Tesla parts below SM 6.0
- RTX A-series mobile laptop SKUs not listed above — same SM 8.6 caps as
  desktop counterparts; add device_id on WARN
- Navi24 OEM-only mobile variants (0x7421, 0x7423 partially covered above)
- Navi21/22/23 pure OEM/internal die variants with no retail name
- CDNA2 0x740F (Aldebaran internal die variant, no public SKU name confirmed)
- RDNA4 RX 9060 non-XT — device ID not yet in public driver sources at document date

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