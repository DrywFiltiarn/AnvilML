# Plan Report: P2-A4

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P2-A4                                       |
| Phase       | 2 — Core Domain Types: Config & Errors      |
| Description | anvilml-core: config_load layered precedence (defaults+toml) |
| Depends on  | P2-A3                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-06-26T19:00:00Z                        |
| Attempt     | 1                                           |

## Objective

Create `crates/anvilml-core/src/config_load.rs` implementing `pub fn load(toml_path: Option<&Path>) -> Result<ServerConfig, AnvilError>` that starts from `ServerConfig::default()` and, if a TOML file is found (at the provided path or the default `./anvilml.toml`), parses it and merges field-by-field so that TOML values override defaults while missing fields keep their defaults. This establishes the first two layers of the four-layer config precedence chain (defaults → TOML → env vars → CLI flags) defined in `ANVILML_DESIGN.md §15`.

## Scope

### In Scope
- Create `crates/anvilml-core/src/config_load.rs` with `pub fn load(toml_path: Option<&Path>) -> Result<ServerConfig, AnvilError>`.
- Add the `toml` crate as a dependency in `crates/anvilml-core/Cargo.toml`.
- Add `mod config_load;` and `pub use config_load::load;` to `crates/anvilml-core/src/lib.rs`.
- Implement two-layer precedence: `ServerConfig::default()` → optional TOML override.
- Create `crates/anvilml-core/tests/config_load_tests.rs` with ≥4 tests.
- Bump `anvilml-core` patch version from `0.1.3` to `0.1.4`.

### Out of Scope
- Environment variable overrides (layer 3, P2-A5's scope).
- CLI flag overrides (layer 4, P2-A5's scope).
- Updating `anvilml.toml` beyond its current two fields (P2-A7's scope).
- The `config_reference` drift test (P2-A7's scope).

## Existing Codebase Assessment

`ServerConfig` is fully defined in `crates/anvilml-core/src/config.rs` (106 lines) with all eight scalar fields and five nested structs (`ModelDirConfig`, `GpuSelectionConfig`, `LimitsConfig`, `RocmConfig`, `HardwareOverrideConfig`), all deriving `Debug`, `Clone`, `serde::Serialize`, and `serde::Deserialize`. The `Default` impl is complete and matches the field defaults documented in `ENVIRONMENT.md §4`.

The crate's `lib.rs` (8 lines) declares `mod config;` and `mod error;` with pub re-exports. No `config_load` module exists yet — this task creates it from scratch.

The existing test file `crates/anvilml-core/tests/config_tests.rs` (97 lines, 11 tests) follows a consistent style: one test function per scalar/nested field, each with a `///` doc comment describing the assertion, importing `ServerConfig` directly from `anvilml_core`, and using simple `assert_eq!` / `assert!` comparisons. The new `config_load_tests.rs` should follow this same pattern.

The checked-in `anvilml.toml` at the repo root currently only has `host` and `port` fields (Phase 1 stub). The config loading task will use this file for its tests, but since it only contains two of the nine+ fields, partial override tests will need to write temporary TOML files with more fields.

No dual-mode parity markers (`REAL_PATH_VERIFIED`/`MOCK_PATH_VERIFIED`) apply to this task — config loading is pure Rust data processing with no node `execute()`, arch `load()`, or similar functions covered by the marker convention.

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| crate  | toml    | 0.8.23          | rust-docs unavailable; lockfile fallback | n/a |

Note: The `rust-docs` MCP tool is not directly callable from this planning session. Version `0.8.23` is the current stable release of the `toml` crate as of mid-2026. The ACT agent must confirm this version via `rust-docs` MCP at session start and adjust if the MCP result differs. The `toml` crate's `from_str` API accepts `&str` and returns `Result<T, toml::de::Error>` for any type `T: serde::Deserialize`, which matches `ServerConfig`'s derives exactly.

## Approach

### Step 1: Add `toml` dependency to `crates/anvilml-core/Cargo.toml`

Add `toml = "0.8.23"` to the `[dependencies]` section. The `toml` crate is a pure-data crate with zero async/I/O — fully compatible with `anvilml-core`'s hard constraint of zero I/O, zero async, no tokio. No feature flags are needed.

### Step 2: Create `crates/anvilml-core/src/config_load.rs`

Implement the module with the following structure:

**a) The `load` function signature:**
```rust
pub fn load(toml_path: Option<&Path>) -> Result<ServerConfig, AnvilError>
```

**b) Function body logic (in order):**
1. Start with `let mut config = ServerConfig::default();` — this is the base layer (layer 1).
2. Determine the TOML file path: if `toml_path` is `Some(path)`, use it; otherwise, use `PathBuf::from("./anvilml.toml")` as the default path. This is a design decision: the task context says "if toml_path (or default ./anvilml.toml) exists" — we resolve the path early so the existence check and read use the same path.
3. Check if the file exists using `Path::exists()` (available on all Rust editions via `std::path::Path`). If it does not exist, return `Ok(config)` immediately — defaults are the correct result when no TOML file is present.
4. Read the file contents with `std::fs::read_to_string(path)`, propagating any I/O error via `?` into `AnvilError::Io`. This is the correct error type because file I/O failure is an `std::io::Error` which `AnvilError::Io` wraps via `#[from]`.
5. Parse the TOML string with `toml::from_str::<ServerConfig>(&contents)`. If parsing fails, map the error to `AnvilError::Serde` (since TOML deserialization is a serialization concern, and `AnvilError::Serde(String)` is the variant for deserialization failures).
6. Merge the parsed config into the defaults using a field-by-field override approach: iterate over the parsed TOML-derived `ServerConfig` and override only the fields that are present in the TOML file. Since `toml::from_str` deserializes the entire struct, missing fields in the TOML will have their default values from the TOML side. However, the task requires that "missing fields keep default" — meaning if a field is absent from the TOML, its value from `ServerConfig::default()` should be retained, not overwritten by the TOML-side default.

**Critical merge strategy:** The `toml` crate's `from_str` will produce a `ServerConfig` where fields absent from the TOML are populated with their serde default values (or the type's inherent defaults like `0` for integers, empty strings for `String`). This means a naive assignment `config = parsed` would overwrite defaults with TOML-side zeros/empty strings. Instead, we must perform a field-by-field merge where each field is overridden only if it was explicitly present in the TOML.

The correct approach is to use a **two-pass strategy**:
- Parse the TOML into a `toml::Value` (the untyped representation) first.
- For each field, check if it exists in the `toml::Value` table. If present, deserialize that specific field's value; if absent, keep the default.
- This avoids the "TOML-side default overwrites compiled-in default" problem.

Actually, a simpler approach: since all `ServerConfig` fields have non-default defaults (e.g., `host` defaults to `"127.0.0.1"`, not `""`), and the TOML crate deserializes missing fields using their type's default (e.g., `String::default()` = `""`, `Vec::default()` = `[]`), we need to detect which fields were explicitly set in the TOML.

The cleanest implementation uses `toml::Value` for inspection:

```rust
let value: toml::Value = toml::from_str(&contents)?;
let mut config = ServerConfig::default();

// Override scalar fields only if present in TOML
if let Some(host) = value.get("host").and_then(|v| v.as_str()) {
    config.host = host.to_string();
}
if let Some(port) = value.get("port").and_then(|v| v.as_u64()) {
    config.port = port as u16;
}
// ... repeat for each scalar field ...

// Override nested structs only if present
if let Some(gpu) = value.get("gpu_selection").and_then(|v| v.as_table()) {
    if let Some(default_device) = gpu.get("default_device").and_then(|v| v.as_str()) {
        config.gpu_selection.default_device = default_device.to_string();
    }
}
// ... repeat for model_dirs, limits, rocm, hardware_override ...
```

This is verbose but correct and explicit. Each field is overridden only if the TOML contains it, preserving defaults for all absent fields.

**c) Error mapping:**
- `std::fs::read_to_string` errors → `AnvilError::Io(e)` (via `?` since `AnvilError` has `#[from] std::io::Error`).
- `toml::from_str` errors → `AnvilError::Serde(e.to_string())` — TOML parsing is a deserialization failure, which maps to the `Serde` variant.

**d) Documentation:**
- `///` doc comment on `load()` describing the four-layer precedence context (layers 1-2 only), the `toml_path` parameter, and return value.
- Module-level `//!` doc comment describing the module's purpose.

### Step 3: Update `crates/anvilml-core/src/lib.rs`

Add `mod config_load;` and `pub use config_load::load;` after the existing `mod error;` line. Keep `lib.rs` under 80 lines.

### Step 4: Create `crates/anvilml-core/tests/config_load_tests.rs`

Create the test file with ≥4 tests following the established style from `config_tests.rs`:

**Test 1 — `test_load_missing_file_falls_back_to_defaults`:** Call `load(Some(Path::new("/nonexistent/path.toml")))` and assert every field equals `ServerConfig::default()`. This verifies the missing-file path returns defaults without error.

**Test 2 — `test_load_partial_toml_overrides_only_specified_fields`:** Write a temporary TOML file with only `host = "0.0.0.0"` and `port = 9999`, call `load(Some(&temp_path))`, assert `host` and `port` are overridden while all other fields (including nested ones) match defaults. Use `tempfile` or write to a unique path under `/tmp/` and clean up. Since `anvilml-core` has no `tempfile` dependency, use `std::fs::write` to a temp path and `std::fs::remove_file` for cleanup in a `drop` guard or `#[cfg(test)]` block.

**Test 3 — `test_load_malformed_toml_returns_err`:** Write a temporary TOML file with invalid syntax (e.g., `host = ` without a value), call `load(Some(&temp_path))`, and assert the result is `Err(_)`.

**Test 4 — `test_load_full_toml_roundtrips_all_fields`:** Write a temporary TOML file with all `ServerConfig` fields set to non-default values, call `load(Some(&temp_path))`, and assert every loaded field matches the TOML values. This proves the merge covers all fields including nested structs.

**Test 5 — `test_load_default_path_resolves_anvilml_toml`:** Call `load(None)` from the repo root context (or with a path relative to the test binary). This tests the default path resolution.

**Test 6 — `test_load_nested_struct_partial_override`:** Write a TOML with only `[gpu_selection]` and `default_device = "cpu"`, assert only `gpu_selection.default_device` is overridden while all other nested structs retain defaults.

### Step 5: Bump `anvilml-core` version

Change `version = "0.1.3"` to `version = "0.1.4"` in `crates/anvilml-core/Cargo.toml`.

## Public API Surface

| Path | Item | Signature |
|------|------|-----------|
| `anvilml_core::config_load::load` | function | `pub fn load(toml_path: Option<&std::path::Path>) -> Result<ServerConfig, AnvilError>` |
| `anvilml_core::config_load` | module | `pub mod config_load;` (re-exported via `pub use config_load::load;` in `lib.rs`) |

No new structs, enums, or traits are introduced. The only new public item is the `load` function.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-core/src/config_load.rs` | New module: `load()` function implementing defaults→TOML merge |
| MODIFY | `crates/anvilml-core/src/lib.rs` | Add `mod config_load;` and `pub use config_load::load;` |
| MODIFY | `crates/anvilml-core/Cargo.toml` | Add `toml = "0.8.23"` dependency; bump version `0.1.3` → `0.1.4` |
| CREATE | `crates/anvilml-core/tests/config_load_tests.rs` | Test file with ≥6 tests covering missing file, partial override, malformed TOML, full round-trip, and default path |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-core/tests/config_load_tests.rs` | `test_load_missing_file_falls_back_to_defaults` | Missing TOML file returns `Ok` with all defaults | None | `load(Some(Path::new("/nonexistent.toml")))` | `Ok(ServerConfig::default())` — every field matches default | `cargo test -p anvilml-core --test config_load_tests test_load_missing_file_falls_back_to_defaults` exits 0 |
| `crates/anvilml-core/tests/config_load_tests.rs` | `test_load_partial_toml_overrides_only_specified_fields` | TOML with 2 fields overrides only those 2, rest keep defaults | Temp TOML file with `host` and `port` set | `load(Some(&temp_path))` | `host="0.0.0.0"`, `port=9999`, all other fields = defaults | `cargo test -p anvilml-core --test config_load_tests test_load_partial_toml_overrides_only_specified_fields` exits 0 |
| `crates/anvilml-core/tests/config_load_tests.rs` | `test_load_malformed_toml_returns_err` | Malformed TOML returns `Err(AnvilError::Serde)` | Temp TOML file with invalid syntax | `load(Some(&temp_path))` | `Err(_)` — error variant is `Serde` | `cargo test -p anvilml-core --test config_load_tests test_load_malformed_toml_returns_err` exits 0 |
| `crates/anvilml-core/tests/config_load_tests.rs` | `test_load_full_toml_roundtrips_all_fields` | TOML with all fields set produces matching `ServerConfig` | Temp TOML file with all fields at non-default values | `load(Some(&temp_path))` | Every field matches the TOML values exactly | `cargo test -p anvilml-core --test config_load_tests test_load_full_toml_roundtrips_all_fields` exits 0 |
| `crates/anvilml-core/tests/config_load_tests.rs` | `test_load_default_path_resolves_anvilml_toml` | `load(None)` uses default `./anvilml.toml` path | `./anvilml.toml` exists at repo root (current 2-field stub) | `load(None)` | `host="127.0.0.1"`, `port=8488`, all other fields = defaults | `cargo test -p anvilml-core --test config_load_tests test_load_default_path_resolves_anvilml_toml` exits 0 |
| `crates/anvilml-core/tests/config_load_tests.rs` | `test_load_nested_struct_partial_override` | `[gpu_selection]` in TOML overrides only `default_device` | Temp TOML with `[gpu_selection]` section only | `load(Some(&temp_path))` | `gpu_selection.default_device="cpu"`, all other nested fields = defaults | `cargo test -p anvilml-core --test config_load_tests test_load_nested_struct_partial_override` exits 0 |

## CI Impact

No CI changes required. The task adds a new test file under `crates/anvilml-core/tests/`, which is automatically picked up by `cargo test --workspace --features mock-hardware` (the standard test command defined in `ENVIRONMENT.md §6 Step 6`). The `toml` crate is a new dependency but has no platform-specific code — it is pure Rust and compiles identically on Linux and Windows.

## Platform Considerations

None identified. The `toml` crate is platform-neutral (pure Rust, no FFI). `Path` operations (`Path::exists()`, `PathBuf::from`) are cross-platform by design. The Windows cross-check in `ENVIRONMENT.md §7` is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `toml` crate version from training data may be stale — the MCP tool (`rust-docs`) was unavailable for live verification. | High | High | ACT agent must confirm the version via `rust-docs` MCP at session start and adjust if different. The plan notes the fallback version (0.8.23) and the MCP unavailability. |
| Field-by-field merge with `toml::Value` is verbose and error-prone — a missing `if let` arm silently drops a field from override. | Medium | Medium | Write a comprehensive round-trip test (test 4) that sets every field to a non-default value and asserts each one is correctly overridden. This catches any missing merge arm. |
| `PathBuf` fields (`db_path`, `artifact_dir`, `venv_path`) serialize/deserialize as strings in TOML — TOML's `as_str()` returns the correct value, but path separator differences between Windows and Linux could cause a string like `./anvilml.db` to be read correctly as a string but not round-trip through `PathBuf::from`. | Low | Medium | The TOML value is a string; `PathBuf::from(string)` handles both Unix and Windows separators correctly. The round-trip test on the target platform validates this. No special handling needed since we're reading from TOML (not writing). |
| Test temp files leave artifacts if a test panics before cleanup. | Low | Low | Use `std::fs::remove_file` inside a `drop` guard or `std::env::temp_dir()` with unique names to minimize collision risk. Tests that panic will leave temp files but this is acceptable for a test-only concern. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-core --test config_load_tests` exits 0 (≥4 tests pass)
- [ ] `cargo test -p anvilml-core --test config_load_tests test_load_missing_file_falls_back_to_defaults` exits 0
- [ ] `cargo test -p anvilml-core --test config_load_tests test_load_partial_toml_overrides_only_specified_fields` exits 0
- [ ] `cargo test -p anvilml-core --test config_load_tests test_load_malformed_toml_returns_err` exits 0
- [ ] `cargo test -p anvilml-core --test config_load_tests test_load_full_toml_roundtrips_all_fields` exits 0
- [ ] `cargo check -p anvilml-core --features mock-hardware` exits 0 (new dependency compiles)
- [ ] `grep -c "^## " .forge/reports/P2-A4_plan.md` returns 12 (all required sections present)
