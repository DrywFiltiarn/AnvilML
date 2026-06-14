# Plan Report: P3-D1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P3-D1                                       |
| Phase       | 003 — Core Domain Types                     |
| Description | backend: config_reference drift guard integration test |
| Depends on  | P3-A1, P3-A2, P3-A3, P3-A4, P3-A5, P3-B1, P3-C1 (all Phase 003 tasks that define ServerConfig and its nested structs) |
| Project     | anvilml                                     |
| Planned at  | 2026-06-14T22:30:00Z                        |
| Attempt     | 1                                           |

## Objective

Create the `config_reference` integration test (`backend/tests/config_reference.rs`) and the checked-in reference config (`anvilml.toml` at repo root) that together form the **config drift guard** (Gate 1). The test serialises `ServerConfig::default()` to a TOML string via `toml::to_string_pretty`, parses both that string and the `anvilml.toml` file content into `toml::Value`, and recursively compares their key sets — failing if any key is present in one but absent in the other. When complete, `cargo test -p anvilml --features mock-hardware -- config_reference` exits 0, and the CI `config-drift` job has a passing gate.

## Scope

### In Scope
- Create `backend/tests/config_reference.rs` with a single `config_reference` test function that:
  - Serialises `anvilml_core::config::ServerConfig::default()` to a TOML string via `toml::to_string_pretty`.
  - Reads the `anvilml.toml` file from the repo root (path: `../anvilml.toml` relative to `backend/tests/`).
  - Parses both TOML strings into `toml::Value` via `toml::from_str`.
  - Recursively collects all keys from each `toml::Value` tree into `BTreeSet<String>`.
  - Asserts the two key sets are equal, with a descriptive panic message listing missing or extra keys.
- Create `anvilml.toml` at the repo root with all config keys at their documented defaults per `ENVIRONMENT.md §4`, matching the key set of `ServerConfig::default()` serialised to TOML.
- Add `toml` as a dev-dependency of `backend` in `backend/Cargo.toml` (using workspace declaration).
- Bump `backend` crate patch version from `0.1.5` to `0.1.6` per FORGE_AGENT_RULES §14.

### Out of Scope
- Adding or modifying any fields on `ServerConfig` or nested config structs (handled by the task that introduces those fields).
- Updating `docs/ENVIRONMENT.md §4` (only needed when config fields change).
- Updating `docs/TESTS.md` — this is handled by the task that adds the test file entry.
- Any runtime config loading logic (already implemented in `config_load.rs`).

## Existing Codebase Assessment

Phase 003 has completed all type definitions in `anvilml-core`. The `ServerConfig` struct lives in `crates/anvilml-core/src/config.rs` (216 lines) and derives `Serialize` + `Deserialize` with `#[serde(default)]` on the struct. It includes nine top-level scalar fields (`host`, `port`, `db_path`, `artifact_dir`, `num_threads`, `venv_path`, `max_ipc_payload_mib`, `seeds_path`, `model_dirs`), two nested structs (`gpu_selection: GpuSelectionConfig`, `limits: LimitsConfig`), and two `Option` fields (`rocm: Option<RocmConfig>`, `hardware_override: Option<HardwareOverrideConfig>`). PathBuf fields use a `path_as_string` serde helper for JSON roundtrips.

The `toml` crate is already a workspace dependency at version `1.1.2` (confirmed in `Cargo.lock`: `1.1.2+spec-1.1.0`) and is used by `anvilml-core` in `config_load.rs` for TOML deserialisation. However, integration tests in `backend/tests/` are separate Rust crates that need their own `toml` dev-dependency to call `toml::to_string_pretty` directly.

The existing test in `backend/tests/cli_tests.rs` (328 lines) demonstrates the project's integration test style: doc comments describing preconditions and acceptance commands, platform-specific `#[cfg(unix)]` / `#[cfg(windows)]` guards, env var isolation with capture-and-restore, and unconditional subprocess cleanup. The new `config_reference` test is simpler — no subprocess, no network, no env mutation — but should follow the same doc comment convention.

No `anvilml.toml` file exists at the repo root yet. This is the first file that establishes the checked-in reference config that the drift guard compares against.

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| crate  | toml    | 1.1.2           | Cargo.lock fallback (MCP rust-docs unavailable) | n/a |

Note: `toml = "1.1.2"` is already declared in `[workspace.dependencies]` of the root `Cargo.toml`. The Cargo.lock confirms version `1.1.2+spec-1.1.0`. The API surface used — `toml::to_string_pretty()` and `toml::from_str()` — is part of the stable public API of `toml` v1.x. If the MCP result at ACT time differs, the ACT agent must use the MCP result per FORGE_AGENT_RULES §6.2.

## Approach

1. **Add `toml` dev-dependency to `backend/Cargo.toml`.** Add a `[dev-dependencies]` section with `toml = { workspace = true }`. This gives the integration test crate access to `toml::to_string_pretty` and `toml::from_str`. Rationale: integration tests are separate crates that cannot access transitive dependencies without explicit declaration.

2. **Bump `backend` crate version from `0.1.5` to `0.1.6`.** Modify `version = "0.1.5"` to `version = "0.1.6"` in `backend/Cargo.toml`. Rationale: FORGE_AGENT_RULES §14 requires patch version bump when any source file (including manifest) inside a crate is modified.

3. **Create `anvilml.toml` at repo root.** Write a TOML file with all config keys at their documented defaults per `ENVIRONMENT.md §4`. The file must contain:
   - Top-level scalars: `host`, `port`, `db_path`, `artifact_dir`, `num_threads`, `venv_path`, `max_ipc_payload_mib`, `seeds_path`
   - `model_dirs = []` (empty array)
   - `[gpu_selection]` table with `default_device = "auto"`
   - `[limits]` table with `max_queued_jobs = 100` and `max_concurrent_jobs = 1`
   - `rocm = null` and `hardware_override = null` (these are `Option<T>` fields in `ServerConfig::default()`; serde serialises `Option<T>` with `None` as `null` in TOML, so the keys must be present for key-set equality)
   
   Rationale: the test compares key sets, not values. All keys present in `ServerConfig::default()`'s serialized output must appear in `anvilml.toml`, and vice versa. The `null` values for `Option` fields are required because `toml::to_string_pretty` serialises `None` as the TOML `null` literal, which produces a key in the resulting `toml::Value::Table`.

4. **Create `backend/tests/config_reference.rs`.** Write a single test function `config_reference` that:
   - Imports `anvilml_core::config::ServerConfig` and `std::collections::BTreeSet`.
   - Serialises `ServerConfig::default()` to a TOML string via `toml::to_string_pretty()`, asserting success.
   - Reads `../anvilml.toml` (relative to the test file's directory) via `std::fs::read_to_string`, asserting success.
   - Parses both TOML strings into `toml::Value` via `toml::from_str`, asserting success for each.
   - Calls a helper function `collect_keys(&toml::Value) -> BTreeSet<String>` on each parsed value.
   - Compares the two `BTreeSet`s: asserts equal, with a panic message listing any missing or extra keys.
   - Includes a doc comment describing preconditions, what it verifies, and the acceptance command.
   
   Rationale: using `toml::Value` (rather than trying to deserialize into a specific struct) is the correct approach because we only care about key equality, not value equality. The `toml::Value::Table` variant holds an `IndexMap<String, Value>` which we iterate to collect keys recursively.

5. **Implement `collect_keys` helper.** A private function that takes `&toml::Value`, matches on `toml::Value::Table`, inserts each key into a `BTreeSet<String>`, and recursively calls itself on each value. For non-table values, returns an empty set. Rationale: this is a pure data transformation with no decision points, so no logging or doc comments are needed beyond the test's own doc comment.

6. **Verify the test compiles and passes.** The ACT agent will run `cargo test -p anvilml --features mock-hardware -- config_reference` to confirm.

## Public API Surface

None. The test file uses only private functions and the public API of `anvilml-core`'s `config` module (which is already public). No new `pub` items are introduced.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `backend/tests/config_reference.rs` | Integration test: config drift guard comparing key sets |
| CREATE | `anvilml.toml` | Reference config with all keys at documented defaults |
| Modify | `backend/Cargo.toml` | Add `[dev-dependencies]` with `toml = { workspace = true }`; bump version 0.1.5 → 0.1.6 |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `backend/tests/config_reference.rs` | `config_reference` | The checked-in `anvilml.toml` has the same key set as `ServerConfig::default()` serialised to TOML. Any missing or extra key causes a test failure with a descriptive message listing the mismatched keys. | Workspace builds with `mock-hardware` feature. `anvilml.toml` exists at repo root. | `ServerConfig::default()` serialised to TOML; file content of `anvilml.toml`. | Test exits 0 — both key sets are equal. | `cargo test -p anvilml --features mock-hardware -- config_reference` exits 0 |

## CI Impact

No CI changes required. The `config-drift` CI job (defined in GitHub Actions) already runs `cargo test -p anvilml --features mock-hardware -- config_reference`. Adding a new test function to an existing integration test file is automatically picked up by the existing test runner. No new CI jobs or gates are needed.

## Platform Considerations

None identified. The test uses only `std::fs::read_to_string` and `toml` crate APIs, both of which are platform-neutral. The `anvilml.toml` file uses Unix-style paths (`./anvilml.db`, etc.) which are valid on both Linux and Windows. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `toml::to_string_pretty` serialises `Option<T>` with `None` as `null` in TOML, producing keys like `rocm = null` and `hardware_override = null`. If the `anvilml.toml` file omits these keys (as is conventional for optional sections), the key-set comparison will fail. | High | High | Write `anvilml.toml` to include `rocm = null` and `hardware_override = null` to match the serialized output. The ACT agent should verify the exact serialization behavior of the `toml` crate at the resolved version before writing the file. |
| The `toml` crate version resolved at ACT time differs from the workspace-declared `1.1.2`. The `to_string_pretty` and `from_str` API shapes may have changed. | Low | High | Per FORGE_AGENT_RULES §6.2, the ACT agent queries MCP at session start and uses the MCP result. If the API shape differs, the ACT agent adapts the approach (e.g., if `to_string_pretty` was renamed or moved). |
| The `anvilml.toml` path resolution (`../anvilml.toml` from `backend/tests/`) may break if the test is run from a different working directory. | Low | Medium | Use `std::env::current_dir()` to get the workspace root, or use `env!("CARGO_MANIFEST_DIR")` to derive the path relative to the `backend` crate directory. The ACT agent should use a robust path derivation. |
| The `toml` crate's `Value::Table` uses `IndexMap` internally (not `BTreeMap`). Iteration order is insertion order, not alphabetical. Key-set comparison via `BTreeSet` is order-independent, but the ACT agent must not assume any ordering in error messages. | Low | Low | The `collect_keys` function inserts keys into a `BTreeSet`, which sorts them alphabetically. Error messages will list keys in sorted order. This is deterministic and acceptable. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml --features mock-hardware -- config_reference` exits 0
- [ ] `head -1 .forge/reports/P3-D1_plan.md` prints `# Plan Report: P3-D1`
- [ ] `grep "^## " .forge/reports/P3-D1_plan.md` shows exactly 11 section headings
- [ ] `wc -l .forge/reports/P3-D1_plan.md` returns a value greater than 40
