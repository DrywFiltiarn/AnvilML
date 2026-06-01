# Plan Report: P2-A1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P2-A1                                       |
| Phase       | 002 — Config & Graceful Shutdown             |
| Description | anvilml-core: ServerConfig types with defaults |
| Depends on  | P1-A5                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-06-01T06:13:17Z                        |
| Attempt     | 1                                           |

## Objective

Create the `ServerConfig` type hierarchy in `anvilml-core/src/config.rs`, defining all configuration structs and enums specified in ANVILML_DESIGN.md §3.1 with proper `Default` implementations, serde derives (`Deserialize`, `Serialize`, `Clone`, `Debug`), and `#[serde(default)]` annotations for every field. This is the foundational data layer that Phase 2's config loader (P2-A2) will populate via layered resolution. A TOML round-trip test validates correctness.

## Scope

### In Scope
- Add `serde` (with `derive` feature) and `toml` dependencies to `anvilml-core/Cargo.toml`
- Create `crates/anvilml-core/src/config.rs` with all types from ANVILML_DESIGN.md §3.1:
  - `ServerConfig` struct with all 12 fields
  - `ModelDirConfig` struct (path + optional kind)
  - `RocmConfig` struct (hipblaslt bool + optional gfx override string)
  - `HardwareOverrideConfig` struct (device_type + vram_total_mib)
  - `FrontendConfig` struct wrapping `FrontendMode`
  - `FrontendMode` enum: `Local { path: PathBuf }`, `Remote { url: Url }`, `Headless`
  - `GpuSelectionConfig` struct (default_device string)
  - `LimitsConfig` struct (4 limit fields)
- Define supporting enums needed by config types:
  - `ModelKind`: `Clip, Diffusion, Vae, Lora, ControlNet, Unet, Upscale`
  - `DeviceType`: `Cuda, Rocm, Cpu`
- Implement `Default` for every struct and enum (via `#[derive(Default)]` where possible, manual impls where needed)
- Apply `#[serde(default)]` to every field in every struct
- Derive `Deserialize, Serialize, Clone, Debug` on all types
- Update `crates/anvilml-core/src/lib.rs` to re-export the config module and types
- Add an inline `#[cfg(test)]` module with a TOML round-trip test

### Out of Scope
- Config file loading from disk (P2-A2)
- Environment variable resolution (P2-A2)
- CLI parsing with clap (P2-A3)
- Tracing/logging initialization (P2-A4)
- Graceful shutdown signal handler (P2-A5)
- Any I/O, async code, or runtime dependencies
- Other domain types (job, model, artifact, hardware info — Phase 003)

## Approach

1. **Update `anvilml-core/Cargo.toml`** — Add `serde = { version = "1", features = ["derive"] }` and `toml = "0.8"` to `[dependencies]`. These are the only new dependencies; no I/O or async crates.

2. **Create `crates/anvilml-core/src/config.rs`** — Write all types in declaration order matching ANVILML_DESIGN.md §3.1:
   - Define `ModelKind` and `DeviceType` enums first (needed by structs below)
   - Define `FrontendMode` enum with `Local { path: PathBuf }`, `Remote { url: Url }`, `Headless` variants
   - Define nested structs: `ModelDirConfig`, `RocmConfig`, `HardwareOverrideConfig`, `FrontendConfig`, `GpuSelectionConfig`, `LimitsConfig`
   - Define top-level `ServerConfig` with all 12 fields
   - Every struct gets `#[derive(Deserialize, Serialize, Clone, Debug, Default)]` where all fields are themselves `Default`. For `FrontendMode`, implement `Default` manually returning `FrontendMode::Local { path: PathBuf::from("./bloomery") }`
   - Apply `#[serde(default)]` to every field so TOML deserialization fills missing keys with defaults
   - Add doc comments for each struct and field documenting the default value (matching ENVIRONMENT.md §2)

3. **Update `crates/anvilml-core/src/lib.rs`** — Replace the current stub with a `pub mod config;` declaration and re-export all public types from `config` at the crate root level (e.g., `pub use config::{ServerConfig, ModelDirConfig, ...};`). This matches the task requirement "Re-export from lib.rs".

4. **Add TOML round-trip test** — In `config.rs`, add a `#[cfg(test)] mod tests { ... }` module containing:
   - A test that serializes a default-constructed `ServerConfig` to TOML, then deserializes it back, and asserts the round-tripped value equals the original
   - This validates that all derives work correctly and all defaults are preserved through the serde cycle
   - The test runs via `cargo test -p anvilml-core -- config` and must exit 0

## Files Affected

| Action   | Path                              | Description |
|----------|-----------------------------------|-------------|
| MODIFY   | crates/anvilml-core/Cargo.toml    | Add serde (derive) and toml dependencies |
| CREATE   | crates/anvilml-core/src/config.rs | All config types, enums, Default impls, derives, doc comments, and round-trip test |
| MODIFY   | crates/anvilml-core/src/lib.rs    | Replace stub with `pub mod config;` and public re-exports of all config types |

## Tests

| Test ID / Name            | File                              | Validates               |
|---------------------------|-----------------------------------|-------------------------|
| `config::tests::toml_roundtrip` | crates/anvilml-core/src/config.rs | Default `ServerConfig` serializes to valid TOML and deserializes back to an equal value; all fields retain their defaults |

## CI Impact

No CI changes required. The new dependencies (`serde`, `toml`) are lightweight, compile quickly, and are pure Rust with no build scripts. The existing CI matrix (fmt, clippy, test with `--features mock-hardware`) will automatically pick up the new crate contents. No workflow file modifications needed.

## Risks and Mitigations

| Risk                      | Likelihood | Impact | Mitigation              |
|---------------------------|-----------|--------|-------------------------|
| `Url` type from `url` crate not yet a dependency | Low | Medium | Add `url = "2"` to anvilml-core Cargo.toml; it's a small, stable dependency needed for `FrontendMode::Remote { url: Url }` |
| `PathBuf` serde serialization edge cases on Windows vs Linux | Low | Low | Use `#[serde(default)]` and test with default `PathBuf::from("./...")` values; round-trip test catches mismatches |
| `ModelKind` / `DeviceType` overlap with Phase 003 domain types | Medium | Low | Define these enums here in config.rs since they are referenced by config structs; P2-A2 and P3 will consolidate if needed, but keeping them local avoids circular deps |
| TOML table syntax for `FrontendMode` enum with associated data (`Local { path }`, `Remote { url }`) causes deserialization issues | Low | Medium | Use `#[serde(tag = "mode")]` on `FrontendMode` to serialize as `[frontend] mode = "local" path = "..."` which matches the anvilml.toml reference in ENVIRONMENT.md §2 |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-core -- config` exits 0 (TOML round-trip test passes)
- [ ] `cargo clippy -p anvilml-core` exits 0 with no warnings
- [ ] `crates/anvilml-core/src/config.rs` defines all types: `ServerConfig`, `ModelDirConfig`, `RocmConfig`, `HardwareOverrideConfig`, `FrontendConfig`, `FrontendMode`, `GpuSelectionConfig`, `LimitsConfig`, `ModelKind`, `DeviceType`
- [ ] Every struct derives `Deserialize, Serialize, Clone, Debug` and has a `Default` implementation
- [ ] Every field in every struct has `#[serde(default)]`
- [ ] `ServerConfig.db_path` defaults to `./anvilml.db`
- [ ] `FrontendMode` defaults to `Local { path: PathBuf::from("./bloomery") }`
- [ ] `lib.rs` re-exports all config types at the crate root level
- [ ] No async code, no I/O, no runtime dependencies in anvilml-core (pure data types only)
