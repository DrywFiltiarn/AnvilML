# Plan Report: P7-F4

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P7-F4                                              |
| Phase       | 007 — WebSocket Event Stream (Group F)             |
| Description | anvilml-hardware: detect_all_devices seeds and queries device_capabilities |
| Depends on  | P7-F3                                              |
| Project     | anvilml                                            |
| Planned at  | 2026-06-05T14:00:00Z                               |
| Attempt     | 1                                                  |

## Objective

Make `detect_all_devices` in `crates/anvilml-hardware/src/lib.rs` async with a new `pool: &SqlitePool` parameter. At entry, create a `DeviceCapabilityStore`, seed it with `SEED_ENTRIES`, and for each detected device query the store by PCI vendor/device ID instead of doing an in-memory const lookup. Update both call sites in `backend/src/main.rs` and `mod tests` to pass the pool and await. Add `open_in_memory()` to `anvilml-registry` so tests can create a fresh migrated pool.

## Scope

### In Scope
- Add `pub async fn open_in_memory() -> Result<SqlitePool, AnvilError>` to `crates/anvilml-registry/src/db.rs`
- Change `detect_all_devices` signature from `fn detect_all_devices(cfg: &ServerConfig)` to `pub async fn detect_all_devices(cfg: &ServerConfig, pool: &SqlitePool)` in `crates/anvilml-hardware/src/lib.rs`
- At function entry: `DeviceCapabilityStore::new(pool.clone()).seed(&SEED_ENTRIES).await?`
- Replace the inline `SEED_ENTRIES.iter().find(...)` lookup in both Branch 2 (mock) and Branch 3 (real enumeration) with `store.get(dev.pci_vendor_id, dev.pci_device_id).await?` passed to `device_db::resolve_caps_from_row()`
- The override branch (Branch 1) does not need a DB query — it uses synthetic PCI IDs (0/0), so the lookup is skipped there
- Update the `backend/src/main.rs` call site: pass `&db` and add `.await` at both locations (print-hardware path and normal server path)
- Convert all `mod tests` in `lib.rs` that call `detect_all_devices` to `#[tokio::test]` with an in-memory pool from `open_in_memory()`
- Tests that do not call `detect_all_devices` (`vendor_map_*`, `or_all_caps_*`) remain unchanged as sync `#[test]`

### Out of Scope
- Changes to `device_db.rs` (SEED_ENTRIES, resolve_caps_from_row) — these were handled in P7-F3
- Changes to `device_store.rs` (upsert, get, seed methods) — already implemented in P7-F2
- Any changes to the migration file `004_device_capabilities.sql` — already created in P7-F1
- Changes to any other crate's call sites beyond `backend/src/main.rs`
- Adding new features, config fields, or CLI flags

## Approach

1. **Add `open_in_memory()` to `db.rs`.** Call `open(Path::new(":memory:"))` which runs all four migrations including `004_device_capabilities.sql`, returning a fully migrated in-memory pool. This is the pattern used by existing tests in `device_store.rs` (they use temp files, but `:memory:` is cleaner for unit tests).

2. **Make `detect_all_devices` async.** Change the signature to accept `pool: &SqlitePool`. At function entry, create the store and seed it:
   ```rust
   let store = DeviceCapabilityStore::new(pool.clone());
   store.seed(&SEED_ENTRIES).await?;
   ```

3. **Replace inline lookup in Branch 2 (mock-hardware).** The current code iterates `SEED_ENTRIES` to find a matching entry, then maps it to a `DeviceCapabilityRow`. Replace this with:
   ```rust
   let row = store.get(dev.pci_vendor_id, dev.pci_device_id).await?;
   device_db::resolve_caps_from_row(&mut dev, row.as_ref());
   ```

4. **Replace inline lookup in Branch 3 (real enumeration).** Same pattern as above — call `store.get()` for each device with non-zero PCI IDs and pass the result to `resolve_caps_from_row`. For zero PCI ID devices, keep the existing fallback logic.

5. **Update `backend/src/main.rs`.** Two call sites:
   - Line ~142 (print-hardware path): `anvilml_hardware::detect_all_devices(&cfg).await` — but at this point `db` is not yet opened. The plan is to move the hardware detection *after* database open, or pass a temporary pool. However, re-reading main.rs more carefully: the print-hardware path exits immediately after printing and doesn't need the database. We must restructure so that the database is opened before `detect_all_devices` is called in both paths. Since `detect_all_devices` now needs a pool, we open `db` first, then call detection.
   - Line ~148 (normal server path): already opens `db` at line 162-164, but hardware detection happens before that at line 148. We need to reorder: open db first, then detect hardware.

   Actually — looking more carefully, the print-hardware flag (`--print-hardware`) exits immediately without needing a database. We have two options:
   (a) Open a temporary in-memory pool just for detection in the print-hardware path.
   (b) Reorder so `db` is opened before hardware detection in both paths.

   Option (b) is cleaner and more consistent. The database open is fast (in-memory or file-based), and there's no reason to detect hardware before opening the DB. We'll reorder: open db, then detect_all_devices, then everything else.

6. **Update all tests.** Convert each `#[test]` that calls `detect_all_devices` to `#[tokio::test]`, create a pool via `open_in_memory()`, and call `detect_all_devices(&cfg, &pool).await`. Tests that only test helper functions (`vendor_map_*`, `or_all_caps_*`) stay as sync `#[test]`.

7. **Remove `#[serial]` from mock tests.** Since each test now gets its own in-memory pool, there's no shared mutable state between tests — the `serial_test` attribute is no longer needed for mock tests.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-registry/src/db.rs` | Add `pub async fn open_in_memory() -> Result<SqlitePool, AnvilError>` |
| Modify | `crates/anvilml-hardware/src/lib.rs` | Make `detect_all_devices` async; add pool param; seed store; replace inline lookups with `store.get()`; convert tests to `#[tokio::test]` |
| Modify | `backend/src/main.rs` | Reorder: open db before hardware detection; pass `&db` to `detect_all_devices` at both call sites; add `.await` |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `crates/anvilml-hardware/src/lib.rs` (mod tests) | All existing `detect_all_devices` tests (override, mock_cuda, mock_rocm, override_source, override_cpu, never_errs, host_info_populated, sequential_indices, override_device_new_fields, mock_device_new_fields_in_detect_all, mock_enum_source, mock_device_type, mock_vram, vulkan_empty) | Each test creates an in-memory pool, calls async `detect_all_devices`, and asserts the same invariants as before — now backed by the seeded SQLite store |
| `crates/anvilml-hardware/src/lib.rs` (mod tests) | `vendor_map_*` and `or_all_caps_*` tests | Remain sync `#[test]`; verify helper functions unchanged |

## CI Impact

No CI workflow files are modified. The task only touches source code in three crates. The existing CI gate `cargo test --workspace --features mock-hardware` must pass, which exercises all the updated async code paths under the mock-hardware feature flag. No new dependencies or features are introduced.

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Reordering db open before hardware detection in `main.rs` may change startup behavior if hardware detection is expected to happen before DB init (e.g., for logging). | The current code already awaits the DB open synchronously after detecting hardware; moving it earlier has no functional impact since both operations are independent. The print-hardware path will now open a minimal in-memory pool just for detection, which is fast and harmless. |
| `open_in_memory()` using `:memory:` may not run migrations if sqlx's migrator skips in-memory databases. | `sqlx::migrate!` runs against any valid SQLite connection; `:memory:` is fully supported. The existing test pattern in `device_store.rs` already uses temp files which implicitly exercise migration — `:memory:` will do the same. If it fails, fall back to creating a temp file pool as the test helper does. |
| The override branch (Branch 1) creates a synthetic device with PCI IDs (0, 0). Calling `store.get(0, 0)` on an empty table returns `None`, triggering a DeviceDB warning log. | This is acceptable: the override path intentionally bypasses the device DB. We can either skip the store query in the override branch (since it already sets all fields explicitly) or leave it — the warning would be noisy but correct. The plan skips the query for the override branch since `resolve_caps_from_row` is not called there anyway. |
| Test isolation: multiple `#[tokio::test]` functions sharing global state could cause flakiness. | Each test creates its own in-memory pool via `open_in_memory()`. In-memory SQLite databases are fully isolated per-connection, so no shared mutable state exists between tests. Remove `#[serial]` attributes. |

## Acceptance Criteria

- [ ] `pub async fn open_in_memory() -> Result<SqlitePool, AnvilError>` exists in `crates/anvilml-registry/src/db.rs` and returns a pool where all four migrations have been applied
- [ ] `detect_all_devices` has signature `pub async fn detect_all_devices(cfg: &ServerConfig, pool: &SqlitePool) -> Result<HardwareInfo, AnvilError>`
- [ ] At entry of `detect_all_devices`, `DeviceCapabilityStore::new(pool.clone()).seed(&SEED_ENTRIES).await?` is called before any device processing
- [ ] In the mock-hardware branch (Branch 2), per-device lookup uses `store.get(dev.pci_vendor_id, dev.pci_device_id).await?` instead of inline `SEED_ENTRIES.iter().find()`
- [ ] In the real enumeration branch (Branch 3), per-device lookup uses `store.get(dev.pci_vendor_id, dev.pci_device_id).await?` instead of inline `SEED_ENTRIES.iter().find()`
- [ ] The override branch (Branch 1) does not call `store.get` (skips DB query since PCI IDs are synthetic)
- [ ] `backend/src/main.rs` passes `&db` to `detect_all_devices` and uses `.await` at both the print-hardware path and normal server path
- [ ] All tests in `crates/anvilml-hardware/src/lib.rs` that call `detect_all_devices` use `#[tokio::test]` with a pool from `open_in_memory()`
- [ ] `cargo test --workspace --features mock-hardware` exits 0
