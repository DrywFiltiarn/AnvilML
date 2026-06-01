# Plan Report: P3-B2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P3-B2                                       |
| Phase       | 003 — Core Domain Types                     |
| Description | anvilml.toml drift guard test (committed toml key-set == ServerConfig) |
| Depends on  | P3-B1                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-06-01T14:00:00Z                        |
| Attempt     | 1                                           |

## Objective

Create a drift guard test that mechanically ensures the committed `anvilml.toml` file stays in sync with the `ServerConfig` Rust struct from `anvilml-core`. The test reads the committed TOML into a `toml::Value`, serializes `ServerConfig::default()` into a `toml::Value`, and asserts their key-sets match recursively — catching any config field added to the struct but missing from the TOML, or any unknown key introduced in the TOML. This prevents silent configuration drift from propagating into CI.

## Scope

### In Scope
- Create `backend/tests/config_reference.rs` integration test
- Add `toml = "0.8"` as a dev-dependency to `backend/Cargo.toml`
- Fix `anvilml.toml` `[frontend]` section: change `mode = { path = "./bloomery" }` (inline table) to the correct nested table format that matches how serde serializes `FrontendMode::Local { path: PathBuf }`
- Recursive key-set comparison logic between two `toml::Value` trees
- Ignoring `[[model_dirs]]` array contents (each entry compared as a single element)
- Ignoring commented `[hardware_override]` section in the TOML

### Out of Scope
- Changing any field values in `anvilml.toml` (only structural format fix)
- Adding new fields to `ServerConfig` (handled by other tasks)
- Modifying `.github/workflows/ci.yml` (no CI changes needed — the test runs as part of the existing `cargo test -p backend` gate)
- Runtime config loading logic (this is a compile-time / CI drift check only)

## Approach

1. **Fix `anvilml.toml` `[frontend]` section format.**
   The current TOML has `mode = { path = "./bloomery" }` (inline table). When `FrontendMode::Local { path: PathBuf::from("./bloomery") }` is serialized by serde-toml, it produces a nested table with the variant name as a key:
   ```toml
   [frontend]
   
   [frontend.mode]
   path = "./bloomery"
   ```
   These produce different `toml::Value` structures. Update the TOML to use the nested table format so both sides serialize identically.

2. **Add `toml` dev-dependency to `backend/Cargo.toml`.**
   The `toml = "0.8"` crate is already in `anvilml-core/Cargo.toml` but not exposed to the `backend` crate. Add it as a dev-dependency so the test file can use `toml::Value`, `toml::from_str`, and `toml::to_string_pretty`.

3. **Create `backend/tests/config_reference.rs`.**
   Write an integration test with two test functions:
   - **`test_toml_key_set_matches_default`**: Reads `anvilml.toml` from the repo root, parses to `toml::Value`; serializes `ServerConfig::default()` to TOML string then parses to `toml::Value`; calls a recursive helper to compare key-sets.
   - **`test_toml_deserializes_into_default`** (optional sanity): Deserializes the TOML into `ServerConfig` and asserts it equals `ServerConfig::default()`, confirming round-trip correctness.

4. **Implement recursive key-set comparison helper.**
   A private function `fn keys_match(a: &toml::Value, b: &toml::Value, path: &str) -> bool` that:
   - For `Table`: collects all keys from both sides; asserts no missing/extra keys; recurses into each matching pair.
   - For `Array`: compares only the first element of each array (ignores array cardinality per task spec);
   - For scalar/primitive types: asserts equality.
   - Tracks the dotted path for error messages.

5. **Verify locally.**
   Run `cargo test -p backend --features mock-hardware -- config_reference` and confirm exit 0 with the test passing.

## Files Affected

| Action   | Path                              | Description                                                  |
|----------|-----------------------------------|--------------------------------------------------------------|
| MODIFY   | anvilml.toml                      | Fix `[frontend]` section: change inline `mode = { path }` to nested table format matching serde-toml enum serialization |
| MODIFY   | backend/Cargo.toml                | Add `toml = "0.8"` as dev-dependency                         |
| CREATE   | backend/tests/config_reference.rs | Integration test: drift guard comparing TOML key-set vs ServerConfig::default() key-set |

## Tests

| Test ID / Name            | File                              | Validates                                  |
|---------------------------|-----------------------------------|--------------------------------------------|
| `test_toml_key_set_matches_default` | `backend/tests/config_reference.rs` | Every ServerConfig field (incl. nested [rocm]/[frontend]/[gpu_selection]/[limits]) appears in anvilml.toml and vice versa; ignores [[model_dirs]] array contents and commented [hardware_override] |
| `test_toml_deserializes_into_default` | `backend/tests/config_reference.rs` | `anvilml.toml` round-trips into `ServerConfig::default()` confirming semantic equivalence |

## CI Impact

No CI changes required. The test runs automatically as part of the existing `cargo test -p backend --features mock-hardware` gate in the CI matrix (see ARCHITECTURE.md §9). No new workflow jobs or steps need to be added.

## Risks and Mitigations

| Risk                      | Likelihood | Impact | Mitigation              |
|---------------------------|-----------|--------|-------------------------|
| `FrontendMode` enum serialization format differs between current TOML and serde-toml output | High | High | Fix TOML to use nested table format `[frontend.mode]` with `Local { path }` — verified against serde-toml behavior |
| `toml::Value` representation of `Option<T>` (None vs absent key) | Medium | Medium | `ServerConfig::default()` produces `hardware_override: None`, which serializes as no key; the commented `[hardware_override]` in TOML is ignored by `toml::Value` parsing. Explicitly skip commented sections by comparing only parsed values, not raw text. |
| PathBuf serialization uses forward slashes on Windows | Low | Medium | `toml::Value` comparison uses string equality; PathBuf serializes to string — both sides use the same serializer so they match identically regardless of platform |
| `toml` crate version mismatch between anvilml-core and backend dev-dep | Low | Low | Use the same `"0.8"` version string; workspace Cargo.toml already pins versions consistently |

## Acceptance Criteria

- [ ] `anvilml.toml` `[frontend]` section uses nested table format: `[frontend.mode]` with `path = "./bloomery"` (not inline table)
- [ ] `backend/Cargo.toml` includes `toml = "0.8"` under `[dev-dependencies]`
- [ ] `backend/tests/config_reference.rs` exists and compiles
- [ ] `cargo test -p backend --features mock-hardware -- config_reference` exits 0 with all tests passing
- [ ] `cargo test -p backend --features mock-hardware` (full workspace) exits 0 — no regressions
- [ ] Recursive key-set comparison correctly detects: (a) a ServerConfig field missing from TOML, (b) an unknown key in TOML not present in ServerConfig
