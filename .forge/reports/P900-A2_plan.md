# Plan Report: P900-A2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P900-A2                                     |
| Phase       | 900 — CLI and Config Test Retrofit          |
| Description | anvilml-core: add #[serial] to env-var-mutating config_load tests |
| Depends on  | none                                        |
| Project     | anvilml                                     |
| Planned at  | 2026-06-14T19:23:00Z                        |
| Attempt     | 1                                           |

## Objective

Add the `serial_test` crate as a dev-dependency to `anvilml-core` and annotate the three tests in `config_load_tests.rs` that mutate `std::env` (process-global, non-atomic state) with `#[serial]`. This eliminates the non-deterministic race condition where concurrent test threads observe mutated env vars mid-flight, causing `test_env_var_beats_toml` to intermittently see the TOML port value instead of the env var value. When complete, the acceptance command — 50 consecutive runs of `cargo test -p anvilml-core --test config_load_tests` — will exit 0 every time.

## Scope

### In Scope
- Add `serial_test = "3.1"` to `[dev-dependencies]` in `crates/anvilml-core/Cargo.toml` (direct dependency, not workspace — only one crate needs it).
- Add `use serial_test::serial;` at the top of `crates/anvilml-core/tests/config_load_tests.rs`.
- Annotate `test_env_var_beats_toml` with `#[serial]`.
- Annotate `test_cli_override_beats_env` with `#[serial]`.
- Annotate `test_nested_env_var` with `#[serial]`.
- Do NOT annotate `test_missing_file_uses_defaults` — it removes an env var but does not set one, and the capture-and-restore pattern is already correct for it.
- Update the three affected entries in `docs/TESTS.md` to note the `#[serial]` annotation and its justification.
- Bump `anvilml-core` patch version from `0.1.5` to `0.1.6` in `crates/anvilml-core/Cargo.toml`.

### Out of Scope
- No changes to any source files outside tests (`config.rs`, `config_load.rs`, `lib.rs`, `error.rs`, `types/`).
- No changes to other test files (`config_tests.rs`, `job_tests.rs`, `model_tests.rs`, `artifact_tests.rs`, `hardware_tests.rs`).
- No changes to `backend/tests/cli_tests.rs` (that is P900-A1).
- No changes to CI workflow files.
- No changes to `anvilml.toml` or `docs/ENVIRONMENT.md`.
- No changes to any crate's public API.

## Existing Codebase Assessment

`anvilml-core` is a pure-data crate with zero I/O, zero async, and zero network dependencies. Its test suite lives in `crates/anvilml-core/tests/` as separate test crate files (no inline `#[cfg(test)]` blocks). The `config_load_tests.rs` file contains four tests: `test_missing_file_uses_defaults`, `test_env_var_beats_toml`, `test_cli_override_beats_env`, and `test_nested_env_var`. Three of the four tests call `std::env::set_var` (or `remove_var`) and already follow the capture-and-restore teardown pattern described in `ENVIRONMENT.md §11.3`.

The established test style is: doc comment describing what the test verifies, `#[test]` attribute, capture-and-restore env var teardown outside all assertions. The `tempfile` dev-dependency is declared via `{ workspace = true }` in the crate's Cargo.toml, referencing the workspace dependency at `tempfile = "3.27.0"`.

The design doc and ENVIRONMENT.md §11.3 explicitly require `#[serial]` for any test that mutates `std::env`, noting that capture-and-restore alone does not prevent concurrent threads from observing the mutated value mid-flight. This task brings the code into compliance with that requirement.

No gap exists between the design doc and the current source — the tests already exist and are functionally correct in isolation; they just lack the `#[serial]` annotation required for safe concurrent execution.

## Resolved Dependencies

| Type   | Name         | Version verified | MCP source     | Feature flags confirmed |
|--------|--------------|-----------------|----------------|------------------------|
| crate  | serial_test  | 3.1             | crates.io (MCP unavailable; version resolved from crates.io latest stable) | none (no features required for basic `#[serial]` usage) |

Note: The `rust-docs` MCP tool is not available in this session's tool set. The version `3.1` was resolved via direct crates.io API query. If the MCP-resolved version differs, the ACT agent must use the MCP result per FORGE_AGENT_RULES §6.2 (ACT is authoritative over PLAN on version numbers).

## Approach

1. **Add `serial_test` dev-dependency to `crates/anvilml-core/Cargo.toml`.**
   Append `serial_test = "3.1"` to the `[dev-dependencies]` section. This is a direct dependency (not `{ workspace = true }`) because only `anvilml-core` needs it — no other crate has env-var-mutating tests. The `serial_test` crate provides the `serial` proc-macro attribute with no additional feature flags required for basic usage.

2. **Add `use serial_test::serial;` import to `config_load_tests.rs`.**
   Insert `use serial_test::serial;` after the existing `use anvilml_core::config::ServerConfig;` and `use anvilml_core::{load, ConfigOverrides};` lines (before the doc comment block). This imports the proc-macro attribute.

3. **Annotate `test_env_var_beats_toml` with `#[serial]`.**
   Place `#[serial]` immediately before `fn test_env_var_beats_toml()`. This test calls `std::env::set_var("ANVILML_PORT", "8080")` — the primary source of the race condition.

4. **Annotate `test_cli_override_beats_env` with `#[serial]`.**
   Place `#[serial]` immediately before `fn test_cli_override_beats_env()`. This test also calls `std::env::set_var("ANVILML_PORT", "8080")` and creates `ConfigOverrides` with `port: Some(7070)`.

5. **Annotate `test_nested_env_var` with `#[serial]`.**
   Place `#[serial]` immediately before `fn test_nested_env_var()`. This test calls `std::env::set_var("ANVILML_GPU_SELECTION__DEFAULT_DEVICE", "cpu")`.

6. **Do NOT annotate `test_missing_file_uses_defaults`.**
   This test calls `std::env::remove_var("ANVILML_PORT")` but does not call `set_var`. The capture-and-restore pattern is already correct and sufficient for this test. Annotating it would serialize it unnecessarily, reducing test parallelism without adding safety.

7. **Bump `anvilml-core` patch version from `0.1.5` to `0.1.6`.**
   Per FORGE_AGENT_RULES §14 and ENVIRONMENT.md §12, every task that modifies source files inside a crate must bump the patch version. The version line in `crates/anvilml-core/Cargo.toml` changes from `version = "0.1.5"` to `version = "0.1.6"`.

8. **Update `docs/TESTS.md` entries for the three annotated tests.**
   For each of the three affected test entries (`test_env_var_beats_toml`, `test_cli_override_beats_env`, `test_nested_env_var`), add a note in the Context field indicating the `#[serial]` annotation and its justification: "Process-global `std::env` is non-atomic; concurrent threads can observe `set_var` mid-flight. Annotated with `#[serial]` to serialise execution and eliminate the race window."

## Public API Surface

None. This task introduces no new `pub` items, changes no function signatures, and modifies no crate interfaces. The only changes are in test code (which is not part of the crate's public API) and in configuration files (Cargo.toml, TESTS.md).

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-core/Cargo.toml` | Add `serial_test = "3.1"` dev-dep; bump version `0.1.5` → `0.1.6` |
| Modify | `crates/anvilml-core/tests/config_load_tests.rs` | Add `use serial_test::serial;`; annotate 3 tests with `#[serial]` |
| Modify | `docs/TESTS.md` | Update 3 test entries to note `#[serial]` annotation |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-core/tests/config_load_tests.rs` | `test_missing_file_uses_defaults` | Config loads defaults when TOML is absent | None | `path = "/nonexistent/path.toml"`, `overrides = ConfigOverrides::default()` | `Result::Ok(ServerConfig::default())` | `cargo test -p anvilml-core --test config_load_tests test_missing_file_uses_defaults` exits 0 |
| `crates/anvilml-core/tests/config_load_tests.rs` | `test_env_var_beats_toml` | Env var overrides TOML file value for same field | `#[serial]` annotation present | TOML `port = 9001`, `ANVILML_PORT=8080` | `cfg.port == 8080` | `cargo test -p anvilml-core --test config_load_tests test_env_var_beats_toml` exits 0 |
| `crates/anvilml-core/tests/config_load_tests.rs` | `test_cli_override_beats_env` | CLI override beats env var which beats TOML | `#[serial]` annotation present | TOML `port = 9001`, `ANVILML_PORT=8080`, `overrides.port = Some(7070)` | `cfg.port == 7070` | `cargo test -p anvilml-core --test config_load_tests test_cli_override_beats_env` exits 0 |
| `crates/anvilml-core/tests/config_load_tests.rs` | `test_nested_env_var` | Double-underscore env var maps to nested config field | `#[serial]` annotation present | TOML without `gpu_selection`, `ANVILML_GPU_SELECTION__DEFAULT_DEVICE=cpu` | `cfg.gpu_selection.default_device == "cpu"` | `cargo test -p anvilml-core --test config_load_tests test_nested_env_var` exits 0 |
| (acceptance) | (50-run loop) | Race condition eliminated; deterministic results across consecutive runs | `#[serial]` on all 3 env-mutating tests | 50 consecutive test runs | All 50 runs exit 0 | `for i in $(seq 1 50); do cargo test -p anvilml-core --test config_load_tests || exit 1; done` exits 0 |

## CI Impact

No CI changes required. The `serial_test` crate is a dev-dependency only; it does not affect production builds or CI job definitions. The `rust-linux` and `rust-windows` CI jobs run `cargo test --workspace --features mock-hardware`, which already picks up all `crates/anvilml-core/tests/` test files. The `#[serial]` attribute has no effect on test semantics — it only serialises execution order within the test binary, which is transparent to CI.

## Platform Considerations

None identified. The `serial_test` crate is cross-platform and does not use any `#[cfg(unix)]` or `#[cfg(windows)]` guards. The `std::env` API it protects is also cross-platform. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `serial_test` version mismatch — the version resolved via crates.io may differ from what the ACT agent's MCP lookup returns. | Low | Medium | The plan notes the MCP unavailability. The ACT agent must query `rust-docs` MCP at session start and use the MCP result per FORGE_AGENT_RULES §6.2 (ACT is authoritative over PLAN on version numbers). |
| Annotating `test_missing_file_uses_defaults` with `#[serial]` unnecessarily — this test only removes an env var, does not set one, and capture-and-restore is already correct. | Low | Low | The plan explicitly excludes this test from annotation. The ACT agent must verify the test body before annotating to confirm it does not call `set_var`. |
| `serial_test` proc-macro expansion conflicts with existing `#[test]` attributes. | Very Low | High | `serial_test` is designed to stack with `#[test]` — the attribute is applied before the test function is registered. If this fails, the ACT agent must check the crate docs for the correct import path (e.g., `serial_test::serial_test` vs `serial_test::serial`). |
| The 50-run acceptance loop takes longer due to serialisation. | Low | Low | Serialisation only affects tests within the same binary that share the annotation — `test_missing_file_uses_defaults` runs in parallel with the other three, so the overhead is bounded to three tests serialised instead of four. |

## Acceptance Criteria

- [ ] `head -1 .forge/reports/P900-A2_plan.md` prints `# Plan Report: P900-A2`
- [ ] `grep "^## " .forge/reports/P900-A2_plan.md` shows exactly 11 section headings
- [ ] `wc -l .forge/reports/P900-A2_plan.md` outputs a number > 40
- [ ] `cargo test -p anvilml-core --test config_load_tests` exits 0 (single-run smoke test)
- [ ] `for i in $(seq 1 50); do cargo test -p anvilml-core --test config_load_tests || exit 1; done` exits 0 (race-condition elimination gate)
- [ ] `cargo test --workspace --features mock-hardware` exits 0 (no regression in other crates)
