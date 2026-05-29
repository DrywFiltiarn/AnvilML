# Plan Report: P2-A2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P2-A2                                         |
| Phase       | 002 — Core Types & IPC                       |
| Description | anvilml-core: configuration types             |
| Depends on  | P2-A1                                         |
| Project     | anvilml                                       |
| Planned at  | 2026-05-29T17:50:00Z                          |
| Attempt     | 1                                             |

## Objective

Implement the full `ServerConfig` struct and all nested configuration types in `crates/anvilml-core/src/config.rs`, matching the field names, types, and defaults specified in `ANVILML_DESIGN.md §3.1`. This is the foundational data-structure work that enables every downstream crate (hardware detection, scheduler, server) to load system configuration from a TOML file with environment-variable overrides. The config types are pure serializable data — no I/O, no async — consistent with `anvilml-core`'s zero-I/O mandate.

## Scope

### In Scope
- Create `crates/anvilml-core/src/config.rs` with all config structs and enums:
  - `ServerConfig` — top-level config holding all fields from §3.1
  - `ModelDirConfig` — model directory entry with optional `kind`
  - `RocmConfig` — ROCm backend settings
  - `HardwareOverrideConfig` — synthetic hardware override
  - `FrontendConfig` + `FrontendMode` enum (`Local`, `Remote`, `Headless`)
  - `GpuSelectionConfig` — GPU device selection policy
  - `LimitsConfig` — IPC and API limits
- Add `serde` (derive feature) and `toml` dependencies to `crates/anvilml-core/Cargo.toml`
- Update `crates/anvilml-core/src/lib.rs` to export the config module and re-export `ServerConfig`
- Write a round-trip serialization test: construct `ServerConfig`, serialize to TOML, deserialize back, assert equality
- All fields use documented defaults via `#[serde(default = "...")]` or `Default` impls
- All types derive `serde::Serialize`, `serde::Deserialize`, `Clone`, `Debug`

### Out of Scope
- Environment variable override logic (`ANVILML_*` resolution) — that is a consumer concern for the launcher binary (P2-A1 handles error types; env resolution will be in P2-A8 / backend main)
- CLI flag parsing (`--config <path>`) — handled by the launcher (Phase 008)
- Config validation or loading from disk — these are I/O operations; config types here are pure data
- TOML file generation or migration — not needed for MVP
- `ModelKind` enum definition — that belongs to P2-A3 (domain types: Job, Model, Artifact)
- `DeviceType` enum definition — that belongs to P2-A4 (hardware types)
- `DateTime<Utc>`, `Uuid`, `serde_json::Value` dependencies — added in later tasks

## Approach

1. **Add dependencies to `Cargo.toml`.** Add `serde = { version = "1", features = ["derive"] }` and `toml = "0.8"` to `[dependencies]` in `crates/anvilml-core/Cargo.toml`. These are the only new external deps for this task.

2. **Create `config.rs`.** Define all config types with exact field names, types, and defaults from `ANVILML_DESIGN.md §3.1`:
   - Use `IpAddr` (from `std::net`) for `host` — default `"127.0.0.1"` via serde default function
   - Use `u16` for `port` — default `8488`
   - Use `Vec<ModelDirConfig>` for `model_dirs` — default empty vec
   - Use `PathBuf` for directory fields (`artifact_dir`, `db_path`, `venv_path`, `worker_log_dir`) — defaults per §3.1
   - Use `usize` for `num_threads` (default 14) and `num_interop_threads` (default 4)
   - Use `Option<PathBuf>` for `worker_log_dir` so it can be absent from TOML
   - Use `Option<HardwareOverrideConfig>` — optional section
   - For `FrontendMode`: use serde's internally-tagged enum with `#[serde(tag = "mode")]` to serialize as a single key in TOML. The `Local` variant carries an optional `path: PathBuf`, the `Remote` variant carries `url: String`, and `Headless` is unit.
   - For `ModelDirConfig.kind`: since `ModelKind` doesn't exist yet (P2-A3), use `Option<String>` with a placeholder comment noting it will be `Option<ModelKind>` later. Alternatively, define `ModelKind` here as a forward-compatible enum — but the task says "per §3.1" and §3.1 shows `kind: Option<ModelKind>`. Since this is P2-A2 specifically for config types and §3.1 references `ModelKind`, I will define `ModelKind` in `config.rs` as a minimal serde-serializable enum matching the TOML reference (`diffusion`, `vae`, `lora`, `controlnet`). This avoids breaking the config module's self-containment.
   - For `HardwareOverrideConfig`: references `DeviceType`. Similarly, define a minimal `DeviceType` enum in `config.rs` (or use `String` with a comment). Since §3.1 shows `device_type: DeviceType`, and `DeviceType` is defined in P2-A4, I will define it here as a minimal serde-compatible enum matching the MVP set (`Cuda`, `Rocm`, `Cpu`) — consistent with §5's device detection types.
   - For `FrontendConfig.mode`: use `FrontendMode` enum. Default is `Local { path: PathBuf::from("./bloomery") }`.

3. **Implement `Default` for each struct.** Each field with a documented default gets either `#[serde(default = "default_xxx")]` pointing to a helper function, or the field uses a type whose natural zero-value is correct (e.g., empty vec). Provide explicit `Default` impls where needed.

4. **Update `lib.rs`.** Add `pub mod config;` and `pub use config::ServerConfig;` (and re-export other public types as appropriate).

5. **Write the round-trip test.** In `config.rs` under `#[cfg(test)]`, create a test that:
   - Constructs a `ServerConfig` with all fields set to non-default values
   - Serializes to TOML string via `toml::to_string`
   - Deserializes back via `toml::from_str`
   - Asserts every field matches the original value
   - Also tests deserialization of a minimal TOML (only required keys) into default-filled struct

6. **Verify compilation.** Run `cargo check -p anvilml-core` to ensure no type errors or missing imports.

## Files Affected

| Action   | Path                              | Description                                            |
|----------|-----------------------------------|--------------------------------------------------------|
| MODIFY   | `crates/anvilml-core/Cargo.toml`  | Add `serde` (derive) and `toml` dependencies           |
| CREATE   | `crates/anvilml-core/src/config.rs` | All config structs, enums, defaults, and round-trip test |
| MODIFY   | `crates/anvilml-core/src/lib.rs`  | Add `pub mod config;` and re-exports                   |

## Tests

| Test ID / Name            | File                     | Validates                                               |
|---------------------------|--------------------------|---------------------------------------------------------|
| `config_round_trip`       | `crates/anvilml-core/src/config.rs` | Full `ServerConfig` serializes to TOML and deserializes back with all fields preserved |
| `config_default_deserialize` | `crates/anvilml-core/src/config.rs` | Minimal TOML (empty or partial) deserializes into a `ServerConfig` with all documented defaults populated |
| `config_frontend_modes`   | `crates/anvilml-core/src/config.rs` | Each `FrontendMode` variant (`Local`, `Remote`, `Headless`) round-trips correctly through TOML |

## CI Impact

No CI changes required. The existing CI workflow (from P1-A2) already runs `cargo test -p anvilml-core --features mock-hardware` as part of the workspace test suite. Adding tests to `anvilml-core` will be automatically picked up by that command.

## Risks and Mitigations

| Risk                      | Likelihood | Impact | Mitigation              |
|---------------------------|-----------|--------|-------------------------|
| `ModelKind` / `DeviceType` forward-reference conflict with P2-A3/P2-A4 | Medium | Low | Define minimal copies in `config.rs` for self-containment; document that these will be replaced by the canonical types from later tasks. The enum variants and their serde names must match exactly so no breaking change is needed when P2-A3/P2-A4 types are introduced. |
| TOML serialization of `PathBuf` fields | Low | Low | `toml` crate serializes `PathBuf` as string natively; this is well-tested in the ecosystem. No special handling needed. |
| `IpAddr` default function conflict with serde | Low | Low | Use a simple helper function `fn default_host() -> IpAddr { "127.0.0.1".parse().unwrap() }` — standard pattern. |
| FrontendMode enum tagging strategy conflicts with TOML inline-table format | Medium | Medium | Use `#[serde(tag = "mode")]` which produces `{ mode = "local", path = "..." }` for `Local` variant in TOML — matches the reference TOML in §3.2. If this doesn't match, switch to a string-based representation where `mode` is a plain string field and variant-specific fields are optional. |

## Acceptance Criteria

- [ ] `crates/anvilml-core/src/config.rs` exists with all 8 config types: `ServerConfig`, `ModelDirConfig`, `RocmConfig`, `HardwareOverrideConfig`, `FrontendConfig`, `FrontendMode`, `GpuSelectionConfig`, `LimitsConfig`
- [ ] All types derive `serde::Serialize`, `serde::Deserialize`, `Clone`, `Debug`
- [ ] `ServerConfig` fields match §3.1 exactly: `host`, `port`, `model_dirs`, `artifact_dir`, `db_path`, `venv_path`, `rocm`, `hardware_override`, `worker_log_dir`, `num_threads`, `num_interop_threads`, `frontend`, `gpu_selection`, `limits`
- [ ] All documented defaults are applied via serde default functions or Default impls
- [ ] `FrontendMode` enum has variants: `Local { path }`, `Remote { url }`, `Headless`
- [ ] `crates/anvilml-core/Cargo.toml` includes `serde` (with derive feature) and `toml` dependencies
- [ ] `crates/anvilml-core/src/lib.rs` exports `pub mod config;` and re-exports `ServerConfig`
- [ ] `cargo test -p anvilml-core -- config` exits 0 with round-trip test passing
