# Plan Report: P3-B1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P3-B1                                         |
| Phase       | 003 — Core Domain Types                     |
| Description | anvilml: generate anvilml.toml reference config with every configurable field |
| Depends on  | P2-A1                                         |
| Project     | anvilml                                       |
| Planned at  | 2026-06-01T15:30:00Z                          |
| Attempt     | 1                                             |

## Objective

Create a committed `anvilml.toml` reference configuration file at the repository root that enumerates every field of `ServerConfig` (from `crates/anvilml-core/src/config.rs`) along with all nested sections (`RocmConfig`, `HardwareOverrideConfig`, `FrontendConfig`, `GpuSelectionConfig`, `LimitsConfig`, `ModelDirConfig`). Each field must appear at its documented default value with a preceding comment explaining its purpose and valid range. The TOML key names must match the serde names used in `config.rs` exactly, ensuring round-trip compatibility: `toml::from_str` into `ServerConfig` must succeed with zero warnings.

## Scope

### In Scope
- Create `anvilml.toml` at repo root (`/home/dryw/AnvilML/anvilml.toml`)
- Enumerate every `ServerConfig` top-level field: `host`, `port`, `model_dirs`, `artifact_dir`, `db_path`, `venv_path`, `rocm`, `hardware_override`, `worker_log_dir`, `num_threads`, `num_interop_threads`, `frontend`, `gpu_selection`, `limits`
- Include all nested sections as TOML tables: `[rocm]`, `[hardware_override]` (commented out), `[frontend]`, `[gpu_selection]`, `[limits]`
- Include `[[model_dirs]]` array entries with `path` and optional `kind` fields
- Each key must match the serde name from `config.rs` exactly (no snake_case transformation needed — Rust struct field names already use snake_case)
- Values must match the documented defaults from `config.rs` default functions
- Preceding comments on every field describing purpose and valid values
- `[hardware_override]` section must be present but fully commented out (it is `Option<HardwareOverrideConfig>` with default `None`)

### Out of Scope
- Modifying any source code (config.rs, main.rs, cli.rs, etc.)
- Adding or modifying tests (that is P3-B2's responsibility)
- Running `cargo run`, `cargo test`, or any build tool
- Changing CI configuration
- Adding environment variable documentation
- Creating sample model directory entries beyond the two examples in ANVILML_DESIGN §3.2

## Approach

1. **Inventory every field from `ServerConfig`** using the source at `crates/anvilml-core/src/config.rs`. Map each field to its serde name, default value, and comment text. The mapping is:

   | TOML Key | Serde Name | Default Value | Comment |
   |----------|-----------|---------------|---------|
   | `host` | host | `"127.0.0.1"` | Bind address for the HTTP server |
   | `port` | port | `8488` | HTTP server port |
   | `artifact_dir` | artifact_dir | `"./artifacts"` | Directory where generated images are stored |
   | `db_path` | db_path | `"./anvilml.db"` | SQLite database file path |
   | `venv_path` | venv_path | `"./venv"` | Python virtual environment root (user-managed) |
   | `worker_log_dir` | worker_log_dir | `"./logs"` | Worker stderr capture directory |
   | `num_threads` | num_threads | `14` | PyTorch intra-op thread count |
   | `num_interop_threads` | num_interop_threads | `4` | PyTorch inter-op thread count |

2. **Write the top-level section** of `anvilml.toml` with the plain fields listed above, each preceded by a `#` comment.

3. **Add the `[[model_dirs]]` array section** with two example entries matching ANVILML_DESIGN §3.2:
   - `path = "./models/diffusion"`, `kind = "diffusion"`
   - `path = "./models/vae"`, `kind = "vae"`
   - Comment: `kind` is optional; valid values are `diffusion`, `vae`, `lora`, `controlnet`, `clip`, `unet`, `upscale`.

4. **Add the `[rocm]` section** with:
   - `use_hipblaslt = true` — maps to `ROCBLAS_USE_HIPBLASLT=1` for ROCm performance
   - Commented out: `# hsa_override_gfx_version = "10.3.0"` — override for unsupported GPU architectures

5. **Add the `[frontend]` section** with:
   - `mode = "local"` — valid modes: `local`, `remote`, `headless`
   - Commented out: `# path = "./bloomery"` — static files directory (for mode=local)
   - Commented out: `# url = "http://localhost:5173"` — remote frontend URL (for mode=remote)

6. **Add the `[gpu_selection]` section** with:
   - `default_device = "auto"` — valid values: `auto`, `cpu`, or an integer device index

7. **Add the `[limits]` section** with all four fields:
   - `max_ipc_payload_mib = 64` — max IPC payload size in MiB
   - `list_default_limit = 100` — default list pagination limit
   - `list_max_limit = 1000` — maximum list pagination limit
   - `ws_broadcast_capacity = 256` — WebSocket broadcast buffer capacity

8. **Add the commented-out `[hardware_override]` section** (since it defaults to `None`):
   - `# device_type = "cpu"` — valid values: `cuda`, `rocm`, `cpu`
   - `# vram_total_mib = 8192` — total VRAM in MiB for bypassing auto-detection

## Files Affected

| Action   | Path                              | Description                                            |
|----------|-----------------------------------|--------------------------------------------------------|
| CREATE   | anvilml.toml                      | Reference config with every ServerConfig field + defaults |
| MODIFY   | .forge/reports/P3-B1_plan.md      | This plan report                                       |
| MODIFY   | .forge/state/CURRENT_TASK.md      | Update Step=PLAN, Status=COMPLETE                      |

## Tests

None. (Test file creation is the responsibility of P3-B2: `backend/tests/config_reference.rs`)

## CI Impact

No CI changes required. The `anvilml.toml` file is a static configuration reference committed to the repo. P3-B2's drift-guard test will later validate that the TOML stays in sync with `ServerConfig`, but that test is outside this task's scope.

## Risks and Mitigations

| Risk                      | Likelihood | Impact | Mitigation              |
|---------------------------|-----------|--------|-------------------------|
| Serde name mismatch between TOML keys and config.rs field names | Low | High | All struct fields in config.rs already use snake_case which matches TOML conventions; verify against actual serde derives |
| Missing a newly-added ServerConfig field | Low | Medium | Cross-reference every public field of `ServerConfig` against the TOML output; P3-B2's drift test will catch any future gaps |
| TOML serialization format differs from expected (e.g. PathBuf quoting) | Low | Low | Use string values with quotes for all PathBuf fields; the existing round-trip test in config.rs validates this |
| Comment formatting inconsistency | Low | Low | Follow the exact comment style from ANVILML_DESIGN §3.2 and ENVIRONMENT.md §2 as reference templates |

## Acceptance Criteria

- [ ] File `anvilml.toml` exists at repository root (`/home/dryw/AnvilML/anvilml.toml`)
- [ ] All 13 top-level `ServerConfig` fields are present: `host`, `port`, `model_dirs`, `artifact_dir`, `db_path`, `venv_path`, `rocm`, `hardware_override`, `worker_log_dir`, `num_threads`, `num_interop_threads`, `frontend`, `gpu_selection`, `limits`
- [ ] Every field value matches its default from `config.rs` (verified by comparing against `ServerConfig::default()`)
- [ ] `[rocm]` section contains `use_hipblaslt = true` and commented `hsa_override_gfx_version`
- [ ] `[frontend]` section contains `mode = "local"` with commented `path` and `url`
- [ ] `[gpu_selection]` section contains `default_device = "auto"`
- [ ] `[limits]` section contains all four fields: `max_ipc_payload_mib`, `list_default_limit`, `list_max_limit`, `ws_broadcast_capacity`
- [ ] `[[model_dirs]]` array has two entries with `path` and `kind` fields
- [ ] `[hardware_override]` section is fully commented out (all lines prefixed with `#`)
- [ ] Every field has a preceding `#` comment explaining its purpose
- [ ] TOML keys match serde names from `config.rs` exactly (no transformation needed — all are snake_case)
