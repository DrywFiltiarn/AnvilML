# Plan Report: P2-A7

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P2-A7                                       |
| Phase       | 002 — Core Domain Types: Config & Errors    |
| Description | config_reference test: anvilml.toml matches ServerConfig |
| Depends on  | P2-A6                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-06-28T14:30:00Z                        |
| Attempt     | 1                                           |

## Objective

Create a config-drift gate that proves the checked-in `anvilml.toml` at the repo root
contains every `ServerConfig` field at its documented default, so the two can never
silently diverge in a later phase. This replaces the Phase 1 placeholder echo with a
real automated test (`config_reference`), which becomes the `config-drift` CI job's
actual implementation.

## Scope

### In Scope
- Update `anvilml.toml` (repo root) to include every `ServerConfig` field at its
  documented default per `ENVIRONMENT.md §4`:
  - Scalar fields: `host = "127.0.0.1"`, `port = 8488`, `db_path = "./anvilml.db"`,
    `artifact_dir = "./artifacts"`, `venv_path = "./worker/.venv"`,
    `model_scan_depth = 2`, `max_ipc_payload_mib = 256`
  - `num_threads` is **omitted** (default is `None` = auto, not a literal TOML value)
  - Nested tables: `[gpu_selection]` with `default_device = "auto"`,
    `[limits]` with `max_queued_jobs = 100`
  - One commented-out example `[[model_dirs]]` entry
  - Optional sections `[rocm]` and `[hardware_override]` are omitted (both default
    to `None` — no TOML key needed for absent optional sections)
- Create `backend/tests/config_reference.rs`: a Rust integration test that calls
  `anvilml_core::config_load::load(Some(Path::new("../anvilml.toml")))` from the
  `backend/tests/` directory (the relative path resolves correctly when cargo runs
  tests from the `backend/` crate root), asserts every loaded field equals
  `ServerConfig::default()`, and fails with a clear message on any mismatch.

### Out of Scope

defers_to (from JSON): []

None — this task may not defer any scope. All functionality described in the task
context is implemented in full.

## Existing Codebase Assessment

**What already exists:** `ServerConfig` is fully defined in
`crates/anvilml-core/src/config.rs` with all 13 fields (8 scalars + 5 nested/optional)
and a complete `Default` impl. The `config_load::load()` function in
`crates/anvilml-core/src/config_load.rs` implements the full four-layer precedence
chain (defaults → TOML → env vars → CLI) using untyped `toml::Value` inspection to
achieve field-by-field merging (never silently overwriting absent fields with type
zero values). The `backend/tests/` directory already exists with two integration test
files (`cli_help_test.rs`, `shutdown_tests.rs`) following the established pattern of
a `mod tests { ... }` module inside a `#[cfg(test)]`-gated file.

**Established patterns:** Tests use `///` doc comments describing what is verified,
preconditions, and expected output. Integration tests in `backend/tests/` are standalone
files (not inline `#[cfg(test)]` blocks), each containing a single `mod tests` module.
The `toml` crate (version 1.1.2) is already a dependency of `anvilml-core`. No new
dependencies are needed.

**Gap between design doc and source:** The current `anvilml.toml` at the repo root
contains only `host` and `port` (Phase 1 scaffold). All other fields must be added.
This is the entire purpose of this task — no unexpected gap.

## Resolved Dependencies

| Type   | Name | Version verified | MCP source     | Feature flags confirmed |
|--------|------|-----------------|----------------|------------------------|
| crate  | toml | 1.1.2           | Cargo.lock     | n/a                    |

The `toml` crate (v1.1.2) is already a dependency of `anvilml-core` — no new
dependencies are introduced. Version confirmed from `Cargo.lock`. The API used
(`toml::Value`, `toml::from_str`, `toml::Table`) is the stable public API of
toml 1.1.x and does not require feature flags.

## Approach

1. **Update `anvilml.toml` to include every `ServerConfig` field at its documented
   default.** Replace the current two-line file with the complete config:
   - Scalar fields at their defaults (host, port, db_path, artifact_dir, venv_path,
     model_scan_depth, max_ipc_payload_mib).
   - `num_threads` is omitted — its default is `None` (auto), which means no TOML key
     should appear. This is consistent with how the `config_load::load()` function
     handles it: if the key is absent from TOML, `num_threads` stays `None`.
   - `[gpu_selection]` table with `default_device = "auto"`.
   - `[limits]` table with `max_queued_jobs = 100`.
   - One commented-out `[[model_dirs]]` example entry:
     ```toml
     # [[model_dirs]]
     # path = "./models"
     # recursive = false
     ```
   - `[rocm]` and `[hardware_override]` are omitted — they are optional sections
     (`Option<T>` fields) that default to `None`. No TOML key is needed for an absent
     optional section, and the `apply_rocm()` / `apply_hardware_override()` functions
     correctly leave the field as `None` when the section is absent.
   - Remove the Phase 1 comment block at the top; replace with a concise description
     of the file's role as the config-drift reference.

2. **Create `backend/tests/config_reference.rs`.** The test file will:
   - Import `anvilml_core::config_load::load` and `anvilml_core::ServerConfig`.
   - Import `std::path::Path`.
   - Define a single test function `config_reference_matches_defaults()`:
     - Call `load(Some(Path::new("../anvilml.toml")), None)` from the test's working
       directory (cargo runs integration tests with the crate root as CWD, so
       `../anvilml.toml` resolves to the repo root's `anvilml.toml`).
     - Assert the result is `Ok(config)`.
     - Assert every field of `config` equals `ServerConfig::default()`:
       - Scalar: `host`, `port`, `db_path`, `artifact_dir`, `venv_path`,
         `model_scan_depth`, `max_ipc_payload_mib`, `num_threads`.
       - Nested: `model_dirs.is_empty()`, `gpu_selection.default_device == "auto"`,
         `limits.max_queued_jobs == 100`, `rocm.is_none()`,
         `hardware_override.is_none()`.
     - On assertion failure, the default `assert_eq!` message will name the field
       and both values, providing a clear diagnosis.
   - Include a `///` doc comment describing what the test verifies (the config-drift
     invariant), preconditions (anvilml.toml exists at repo root), and expected output
     (all fields match defaults).

3. **Verify acceptance.** Run `cargo test -p anvilml --features mock-hardware -- config_reference`
   and confirm it exits 0.

## Public API Surface

None. This task creates no new `pub` items. It modifies an existing toml file and
creates a test file that uses existing public APIs (`load()` and `ServerConfig`)
from `anvilml-core`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | anvilml.toml | Expand to include every ServerConfig field at its documented default |
| CREATE | backend/tests/config_reference.rs | Config-drift test: loads anvilml.toml, asserts fields match ServerConfig::default() |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| backend/tests/config_reference.rs | config_reference_matches_defaults | anvilml.toml at repo root serialises to a ServerConfig where every field equals ServerConfig::default() | anvilml.toml exists at repo root with all fields at defaults | load(Some(Path::new("../anvilml.toml")), None) | Ok(config) with all 13 fields matching defaults | `cargo test -p anvilml --features mock-hardware -- config_reference` exits 0 |

## CI Impact

This task adds a new integration test that will be picked up by the existing `config-drift`
CI job (`cargo test -p anvilml --features mock-hardware -- config_reference`), which is
already defined in `.github/workflows/ci.yml`. No CI workflow changes are needed — the
test is simply collected and run by the existing job. No new CI jobs or steps are
required.

## Platform Considerations

None identified. The `config_load::load()` function uses only `std::fs::read_to_string`
and `toml::from_str`, both platform-neutral. The relative path `../anvilml.toml`
resolves from the `backend/` crate root to the repo root on all platforms. The Windows
cross-check in `ENVIRONMENT.md §7` is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The `../anvilml.toml` relative path may not resolve correctly when cargo runs the integration test from the `backend/` crate root, causing the test to fail with a file-not-found error. | Low | High | The test uses `Path::new("../anvilml.toml")` which resolves relative to the CWD. Cargo runs integration tests with the crate root as CWD (`backend/`), so `../anvilml.toml` correctly points to the repo root. Verified by running the test after implementation. If it fails, switch to using `std::env::var("CARGO_MANIFEST_DIR")` to compute the path relative to `backend/`'s own manifest. |
| The `anvilml.toml` file may contain fields or comments that cause `toml::from_str` to fail if the format is incorrect (e.g. missing quotes around string values, wrong TOML syntax). | Low | Medium | Write the TOML file carefully following standard TOML syntax. Use the same format as the existing `host`/`port` lines (already validated by prior tests). The test itself will catch any TOML parse errors via `AnvilError::Serde`. |
| Optional sections `[rocm]` and `[hardware_override]` are absent from `anvilml.toml` — the test must confirm these remain `None` (not fail because the TOML lacks keys). | None | None | The `config_load::load()` function already handles absent optional sections correctly: `apply_rocm()` and `apply_hardware_override()` only set the field to `Some(...)` if the section is present in the TOML. The test asserts `.is_none()` for both, which is the correct default. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml --features mock-hardware -- config_reference` exits 0
