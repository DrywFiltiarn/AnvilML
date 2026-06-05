# Plan Report: P7-G3

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P7-G3                                             |
| Phase       | 007 — WebSocket Event Stream                      |
| Description | Replace SEED_ENTRIES startup seed with SeedLoader; remove const |
| Depends on  | P7-G2b                                            |
| Project     | anvilml                                           |
| Planned at  | 2026-06-05T18:15:00Z                              |
| Attempt     | 1                                                 |

## Objective

Replace the compile-time `SEED_ENTRIES` const in `anvilml-hardware` with runtime SQL seed loading via the existing `SeedLoader::run()` function, eliminating binary bloat (~1700 lines of device data) and enabling post-deployment seed file updates without recompilation.

## Scope

### In Scope
- Remove `pub const SEED_ENTRIES: &[DeviceCapabilityEntry]` from `crates/anvilml-hardware/src/device_db.rs` (all 126 entries, ~1700 lines)
- Remove the `SEED_ENTRIES` import and the inline conversion-to-row logic from `detect_all_devices()` in `crates/anvilml-hardware/src/lib.rs`
- Replace the seed step with `anvilml_registry::seed_loader::run(pool, &cfg.seeds_path).await?`
- Add `seeds_path: PathBuf` field to `ServerConfig` in `crates/anvilml-core/src/config.rs`, defaulting to `<current_exe_dir>/seeds` (with debug fallback to `backend/seeds/` relative to `CARGO_MANIFEST_DIR`)
- Gate `DeviceCapabilityStore::seed()` method and its test module with `#[cfg(any(test, feature = "seed-util"))]` in `crates/anvilml-registry/src/device_store.rs`
- Update all `mod tests` call sites in `lib.rs` that invoke `detect_all_devices()`: copy `backend/seeds/devices.sql` into a `tempfile::TempDir` and set `cfg.seeds_path` to the temp dir path
- Remove unused imports (`anvilml_registry::DeviceCapabilityRow`, `device_db` usage for SEED_ENTRIES) from `lib.rs`

### Out of Scope
- Modifying `resolve_caps_from_row()` or any other function in `device_db.rs` (handled by P7-G4)
- Adding CLI flags or env var overrides for `seeds_path` beyond the built-in default (config resolution is via serde/toml only)
- Any changes to `backend/src/main.rs` (no change needed — `detect_all_devices` already receives `&cfg`)
- Modifying P7-G2a/P7-G2b seed loader implementation

## Approach

1. **Add `seeds_path` to `ServerConfig`** in `crates/anvilml-core/src/config.rs`:
   - Add field `#[serde(default = "default_seeds_path")] pub seeds_path: PathBuf` after the `limits` field
   - Add `fn default_seeds_path() -> PathBuf` that calls `std::env::current_exe()` to get the executable directory, then `.join("seeds")`. If `debug_assertions` is enabled and that path does not exist, fall back to `PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap().join("backend").join("seeds")`
   - Add `seeds_path: default_seeds_path()` to the `Default` impl

2. **Replace seeding in `detect_all_devices()`** in `crates/anvilml-hardware/src/lib.rs`:
   - Remove lines 225-242 (store creation + SEED_ENTRIES iteration + seed call)
   - Replace with: `anvilml_registry::seed_loader::run(pool, &cfg.seeds_path).await?;`

3. **Remove `SEED_ENTRIES` const** from `crates/anvilml-hardware/src/device_db.rs`:
   - Remove the entire `pub const SEED_ENTRIES` block (lines 45-1701)
   - Keep `DeviceCapabilityEntry` struct definition and `resolve_caps_from_row()` function intact

4. **Gate `DeviceCapabilityStore::seed()`** in `crates/anvilml-registry/src/device_store.rs`:
   - Prepend `#[cfg(any(test, feature = "seed-util"))]` to the `seed()` method (line 138)
   - Prepend `#[cfg(any(test, feature = "seed-util"))]` to the entire `mod tests` block (line 169)

5. **Update test modules** in `crates/anvilml-hardware/src/lib.rs`:
   - Add `tempfile` as a dev-dependency if not already present
   - For each test calling `detect_all_devices()`, after creating the pool, also create a temp dir, copy `backend/seeds/devices.sql` into it, and set up the config with `seeds_path` pointing to that temp dir
   - The simplest approach: add a helper function in the test module that creates a temp seeds directory and returns the path

6. **Clean up imports** in `lib.rs`:
   - Remove any imports now unused after removing SEED_ENTRIES conversion logic (e.g., `anvilml_registry::DeviceCapabilityRow` if it was only used for the conversion)

7. **Verify**: `cargo test --workspace --features mock-hardware` exits 0, and cross-checks pass.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-core/src/config.rs` | Add `seeds_path: PathBuf` field + default fn to `ServerConfig` |
| Modify | `crates/anvilml-hardware/src/lib.rs` | Replace inline seed logic with `SeedLoader::run()`, update tests, clean imports |
| Modify | `crates/anvilml-hardware/src/device_db.rs` | Remove `SEED_ENTRIES` const (keep `DeviceCapabilityEntry` struct + `resolve_caps_from_row`) |
| Modify | `crates/anvilml-registry/src/device_store.rs` | Gate `seed()` method and test module with `#[cfg(any(test, feature = "seed-util"))]` |

## Tests

<table>
<tr><th>Test File</th><th>Test Name</th><th>What It Verifies</th></tr>
<tr><td><code>crates/anvilml-hardware/src/lib.rs</code> (mod tests)</td><td>All existing <code>detect_all_devices_*</code> tests</td><td>Tests continue to pass with SeedLoader-backed seeding via temp dir; mock devices resolve correctly; override path works</td></tr>
<tr><td><code>crates/anvilml-registry/src/device_store.rs</code> (mod tests)</td><td>All existing device_store tests</td><td>Gated test module still runs under <code>#[cfg(test)]</code>; upsert + get roundtrip works</td></tr>
<tr><td><code>crates/anvilml-hardware/src/device_db.rs</code> (mod tests)</td><td>All existing device_db tests</td><td>Tests that previously referenced SEED_ENTRIES still compile and pass (they only iterate the const for validation — const is removed, so these tests are also gated or removed)</td></tr>
</table>

Note: The `device_db.rs` test module references `SEED_ENTRIES` directly. Since the const is removed, those tests must be moved under `#[cfg(any(test, feature = "seed-util"))]` as well — or more practically, since the task says to remove the const entirely and the device_db tests are purely about validating the const's content (which no longer exists), they should be gated with `#[cfg(feature = "seed-util")]` so they compile but don't run in normal builds. However, since the const is gone, these tests cannot compile at all — they must be removed or replaced. The plan is to remove the entire test module from `device_db.rs` since its sole purpose was validating `SEED_ENTRIES`, which no longer exists.

## CI Impact

No CI workflow files are modified. The `cargo test --workspace --features mock-hardware` gate and all existing CI checks continue to apply unchanged. The `seed-util` feature flag is optional (not enabled in CI), so the gated code paths are simply not compiled — no regression risk.

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| `device_db.rs` test module references `SEED_ENTRIES` which is being removed, causing compile errors | Remove the entire test module from `device_db.rs` — it only validated const contents which no longer exist. The same validation is implicitly covered by the SeedLoader tests in G2b and the integration-level tests in lib.rs. |
| `tempfile` dev-dependency not present in `anvilml-hardware/Cargo.toml` | Check current dependencies; if absent, add it. Otherwise use `std::env::temp_dir()` with a unique subdirectory. |
| Debug fallback path for `seeds_path` points to wrong directory during workspace builds | Use `CARGO_MANIFEST_DIR` from the `anvilml-hardware` crate (which is at `crates/anvilml-hardware/`) — walk up two levels (`..`) to reach repo root, then into `backend/seeds/`. Guard with `debug_assertions`. |
| `SeedLoader::run()` returns an error when seeds dir doesn't exist, causing startup panic | The default path resolves to a real directory in production (exe-relative) and debug builds. Tests explicitly create temp dirs. The function will still return Err if the path is genuinely absent — this is correct behavior and matches the existing design. |
| `DeviceCapabilityStore::seed()` gated behind feature flag breaks tests that call it directly | The gate `#[cfg(any(test, feature = "seed-util"))]` keeps the method available during test builds (the `test` cfg). Tests in `device_store.rs` continue to work. |

## Acceptance Criteria

- [ ] `SEED_ENTRIES` const removed from `crates/anvilml-hardware/src/device_db.rs` — `grep -c "SEED_ENTRIES" crates/anvilml-hardware/src/device_db.rs` returns 0
- [ ] `detect_all_devices()` uses `anvilml_registry::seed_loader::run(pool, &cfg.seeds_path).await?` instead of inline seed conversion
- [ ] `ServerConfig` has a `seeds_path: PathBuf` field with a working default
- [ ] `DeviceCapabilityStore::seed()` gated behind `#[cfg(any(test, feature = "seed-util"))]`
- [ ] All tests in `crates/anvilml-hardware/src/lib.rs` pass with temp-dir-backed seeding
- [ ] `cargo test --workspace --features mock-hardware` exits 0
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
- [ ] Windows cross-check: `cargo check --target x86_64-pc-windows-gnu --workspace --features mock-hardware` exits 0
