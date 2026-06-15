# Plan Report: P6-C2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P6-C2                                       |
| Phase       | 006 — Model Registry                        |
| Description | anvilml-hardware: SQLite capability enrichment in detect_all_devices |
| Depends on  | P6-A3                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-06-16T00:00:00Z                        |
| Attempt     | 1                                           |

## Objective

Replace the deferred step-h stub in `detect_all_devices` with a real SQLite lookup that enriches each detected non-CPU `GpuDevice` with capability data from the seeded `device_capabilities` table. After this task, any device whose PCI vendor/device ID pair is present in the seed data will have `arch`, all six `InferenceCaps` fields (`fp32`, `fp16`, `bf16`, `fp8`, `fp4`, `flash_attention`), `db_name`, and `capabilities_source = DeviceTable` correctly populated before `AppState` is constructed. The enumerator-reported `name` field is never overwritten. The acceptance test is `cargo test --workspace --features mock-hardware` exiting 0.

## Scope

### In Scope
- `crates/anvilml-hardware/Cargo.toml`: add `anvilml-registry = { path = "../anvilml-registry" }` to `[dependencies]`; bump patch version `0.1.7 → 0.1.8`.
- `crates/anvilml-hardware/src/detect.rs`: rename `_pool` to `pool` in function signature; update `#[instrument]` skip list; add step e2 (SQLite enrichment loop) after step e; remove step h deferred stub; update function doc comment.

### Out of Scope
- No changes to `anvilml-registry` source code — `DeviceCapabilityStore` and `DeviceRow` are already implemented by P6-A3.
- No changes to `anvilml-core` — the `db_name` field was added by P6-C1.
- No test file changes — existing mock tests pass an in-memory pool with an empty `device_capabilities` table; step e2 produces `Ok(None)` for all mock devices (PCI IDs = 0), which is the correct no-op path.
- No changes to `anvilml-server`, `anvilml-scheduler`, `backend`, or any other crate.

## Existing Codebase Assessment

The `detect_all_devices` function in `crates/anvilml-hardware/src/detect.rs` (296 lines) orchestrates a priority-chain detection pipeline: hardware override → mock → Vulkan → platform fallbacks → CPU fallback → capability resolution (step e) → host info → hardware info assembly → deferred seed stub (step h). Step e calls `resolve_caps_from_row` which looks up each GPU's PCI IDs in the compile-time `DEVICE_DB` constant table and populates `arch`, `caps.fp8`, `caps.flash_attention`, and `capabilities_source`.

The `DeviceCapabilityStore` type (from `anvilml-registry`) already implements `new(pool)` and `get(vendor_id, device_id)` returning `Result<Option<DeviceRow>, AnvilError>`. `DeviceRow` carries all six capability booleans aligned with `InferenceCaps`, plus `vendor_id`, `device_id`, `name`, and `arch`. Both are publicly re-exported from `anvilml-registry`'s `lib.rs`.

The `GpuDevice` struct (from `anvilml-core`) already has the `db_name: Option<String>` field added by P6-C1, positioned after `name`. All construction sites across the codebase already set `db_name: None`.

Established patterns: error handling uses `tracing::debug!`/`warn!`/`error!` with structured fields; the `#[instrument]` macro is used on the function; doc comments use `///` with `# Arguments` and `# Returns` sections. The existing mock tests in `tests/mock_tests.rs` use `SqlitePool::connect("sqlite::memory:")` and are marked `#[serial_test::serial]`.

No discrepancy between the design doc and current source — `DeviceCapabilityStore` exists exactly as specified, `GpuDevice.db_name` exists, and `resolve_caps_from_row` is the step-e function being augmented.

## Resolved Dependencies

| Type   | Name             | Version verified | MCP source  | Feature flags confirmed |
|--------|------------------|-----------------|-------------|------------------------|
| crate  | anvilml-registry | local path      | N/A (path dep) | none                  |
| crate  | sqlx             | 0.9.0           | Cargo.lock  | runtime-tokio, sqlite, json |

Note: `anvilml-registry` is a local path dependency — no external version to resolve. `sqlx` is already a dependency of `anvilml-hardware` (workspace version 0.9.0) and is transitively provided by `anvilml-registry`. No new feature flags are needed.

## Approach

1. **Bump version and add dependency in `crates/anvilml-hardware/Cargo.toml`**:
   - Change `[package] version = "0.1.7"` to `version = "0.1.8"`.
   - Add `anvilml-registry = { path = "../anvilml-registry" }` to the `[dependencies]` section.
   - Rationale: This is the only direction the dependency may run (`anvilml-registry` must never depend on `anvilml-hardware`), and the workspace path dependency avoids any version pinning issues.

2. **Rename `_pool` to `pool` in `detect.rs`**:
   - In the function signature (line 54): change `_pool: &SqlitePool` to `pool: &SqlitePool`.
   - In the `#[instrument]` attribute (line 51): change `skip(cfg, _pool)` to `skip(cfg, pool)`.
   - Rationale: The parameter is now used in step e2 (cloned for `DeviceCapabilityStore::new`), so the underscore prefix is no longer appropriate.

3. **Add `DeviceCapabilityStore` import in `detect.rs`**:
   - Add `use anvilml_registry::DeviceCapabilityStore;` to the existing `use` block at the top of the file.
   - Rationale: The import is needed to construct the store in step e2.

4. **Insert step e2 after step e (after line 217, before step f)**:
   - Insert the following code block between step e and step f:
   ```rust
   // ── Step e2: SQLite capability enrichment ────────────────────────
   // Look up each non-CPU device in the device_capabilities table
   // to populate full capability data (arch, all six inference caps,
   // db_name) from the seeded database. This is a non-fatal lookup —
   // if the table doesn't exist or a query fails, the device retains
   // the capabilities resolved in step e from the PCI-ID table.
   let store = DeviceCapabilityStore::new(pool.clone()).await;

   for dev in devices.iter_mut() {
       // Skip CPU devices — they have no real PCI IDs and won't
       // match any entry in the device_capabilities table.
       if dev.device_type == DeviceType::Cpu {
           continue;
       }

       let vendor_id = dev.pci_vendor_id;
       let device_id = dev.pci_device_id;

       // Look up the device in the seeded capability table.
       // Ok(None) means the device isn't in the seed data —
       // the step-e resolution from DEVICE_DB is the final word.
       // Err is non-fatal: a DB query failure must not abort
       // hardware detection. The device keeps its step-e caps.
       match store.get(vendor_id, device_id).await {
           Ok(Some(row)) => {
               // Overwrite arch, all six inference capability fields,
               // capabilities_source, and db_name from the DB row.
               // Never overwrite dev.name — the enumerator-reported
               // name is the specific installed SKU and must be preserved.
               dev.arch = Some(row.arch.clone());
               dev.caps.fp32 = row.fp32;
               dev.caps.fp16 = row.fp16;
               dev.caps.bf16 = row.bf16;
               dev.caps.fp8 = row.fp8;
               dev.caps.fp4 = row.fp4;
               dev.caps.flash_attention = row.flash_attention;
               dev.capabilities_source = CapabilitySource::DeviceTable;
               dev.db_name = Some(row.name.clone());

               tracing::debug!(
                   vendor_id = vendor_id,
                   device_id = device_id,
                   arch = %row.arch,
                   source = "sqlite",
                   "device capability enriched from device_capabilities table"
               );
           }
           Ok(None) => {
               // No matching row in the device_capabilities table.
               // The device retains its step-e resolution from DEVICE_DB.
               tracing::warn!(
                   vendor_id = vendor_id,
                   device_id = device_id,
                   name = %dev.name,
                   "device not found in device_capabilities table"
               );
           }
           Err(e) => {
               // DB query failed — non-fatal. Log the error and continue
               // with step-e resolved capabilities. This prevents a
               // corrupted or missing seed table from blocking hardware
               // detection entirely.
               tracing::error!(
                   vendor_id = vendor_id,
                   device_id = device_id,
                   error = %e,
                   "device_capabilities lookup failed, using step-e resolution"
               );
           }
       }
   }
   ```
   - Rationale: The store is created once before the loop (not per-device) to avoid redundant pool clones. Each device's lookup is independent and non-fatal, so errors are caught per-device rather than aborting the entire enrichment phase.

5. **Remove step h deferred stub (lines 272–282)**:
   - Remove the entire step h block including the comment `// ── Step h: Seed device DB (deferred) ────────────────────────────` and the `tracing::debug!` call that says `"pool seeding deferred"`.
   - Rationale: The deferred seeding is now implemented as step e2, so the stub is obsolete.

6. **Update function doc comment**:
   - In the module-level doc comment (lines 1–6) and the function-level doc comment (lines 16–50), update the description to mention that step e2 enriches devices from the `device_capabilities` SQLite table after PCI-ID table resolution.
   - Update the `pool` parameter description: change "accepted for future device capability seeding (the actual SQL seeding is deferred to the registry task)" to "used for device capability enrichment via `DeviceCapabilityStore`."
   - Rationale: The doc comment must accurately reflect the current implementation.

## Public API Surface

No new public items are introduced. The task modifies only:
- A private function's parameter name (`_pool` → `pool`) — no signature change for callers.
- A private function's internal behavior — no change to its `pub async fn detect_all_devices(cfg: &ServerConfig, pool: &SqlitePool) -> Result<HardwareInfo, AnvilError>` signature.

The `DeviceCapabilityStore` and `DeviceRow` types are already public in `anvilml-registry`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-hardware/Cargo.toml` | Add `anvilml-registry` path dependency; bump patch version `0.1.7 → 0.1.8` |
| Modify | `crates/anvilml-hardware/src/detect.rs` | Rename `_pool` to `pool`; add step e2 SQLite enrichment; remove step h stub; update doc comments |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-hardware/tests/mock_tests.rs` | `test_detect_all_devices_mock_cuda` | Full pipeline with mock CUDA device; step e2 produces `Ok(None)` (PCI IDs = 0) → no enrichment, caps remain at step-e defaults | `mock-hardware` feature active; `ANVILML_MOCK_DEVICE_TYPE=cuda` | In-memory SQLite pool with empty `device_capabilities` table | Detect returns Ok with CUDA GPU + CPU; `db_name` is `None`; `capabilities_source` is `DeviceTable` (from step e DEVICE_DB, not from SQLite) | `cargo test --workspace --features mock-hardware -- test_detect_all_devices_mock_cuda` exits 0 |
| `crates/anvilml-hardware/tests/mock_tests.rs` | `test_detect_all_devices_hardware_override` | Override path is unaffected by step e2 (override devices have PCI IDs = 0, so `Ok(None)` is returned) | `mock-hardware` feature active; `hardware_override` set in config | In-memory SQLite pool with empty `device_capabilities` table | Override device + CPU returned; enrichment produces `Ok(None)` (PCI IDs = 0) | `cargo test --workspace --features mock-hardware -- test_detect_all_devices_hardware_override` exits 0 |
| `crates/anvilml-hardware/tests/mock_tests.rs` | `test_detect_all_devices_cpu_fallback` | CPU fallback path is unaffected; step e2 skips CPU devices entirely | `mock-hardware` feature active; mock returns empty (invalid type) | In-memory SQLite pool with empty `device_capabilities` table | At least one CPU device returned | `cargo test --workspace --features mock-hardware -- test_detect_all_devices_cpu_fallback` exits 0 |
| `crates/anvilml-hardware/tests/mock_tests.rs` | `test_detect_all_devices_inference_caps_union` | Union logic still works after step e2 adds no new caps (mock devices have no DB matches) | `mock-hardware` feature active | In-memory SQLite pool with empty `device_capabilities` table | `inference_caps` struct is well-formed (all bools) | `cargo test --workspace --features mock-hardware -- test_detect_all_devices_inference_caps_union` exits 0 |
| `crates/anvilml-hardware/tests/mock_tests.rs` | `test_detect_all_devices_returns_ok` | Function always returns `Ok` even with step e2 (non-fatal error handling) | `mock-hardware` feature active | In-memory SQLite pool with empty `device_capabilities` table | `Result<HardwareInfo, AnvilError>` is `Ok` | `cargo test --workspace --features mock-hardware -- test_detect_all_devices_returns_ok` exits 0 |

## CI Impact

No CI changes required. The task only modifies existing test behavior (step e2 is a no-op for mock tests with empty DB), which is covered by the existing CI job `rust-linux` and `rust-windows` that run `cargo test --workspace --features mock-hardware`. No new files, new test modules, or new CI gates are introduced.

## Platform Considerations

None identified. The `SqlitePool::clone()` and `DeviceCapabilityStore::get()` are platform-neutral async operations. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `pool.clone()` on `&SqlitePool` — the `SqlitePool` type implements `Clone` but the clone borrows the pool reference rather than incrementing an internal refcount. Calling `.clone()` on `&SqlitePool` produces a new `SqlitePool` handle, not a reference clone. This is the correct usage and matches how `anvilml-registry`'s `DeviceCapabilityStore::new` accepts an owned `SqlitePool`. | Low | High | Verify at compile time: if `SqlitePool` does not implement `Clone`, the code will not compile and the fix is trivial (use `pool` directly without clone, or restructure). The `anvilml-registry` crate already uses `pool.clone()` in its own code patterns. |
| Existing mock tests pass `SqlitePool::connect("sqlite::memory:")` which has no `device_capabilities` table. The `store.get()` call will query a non-existent table and return an error, which is caught by the `Err` branch and logged at ERROR level. This will cause test log output to include ERROR lines that could be mistaken for failures. | Medium | Medium | The test assertions check only the `HardwareInfo` result, not log output. ERROR-level logs do not cause test failures. The `detect_all_devices_returns_ok` test already verifies the function returns `Ok` in this scenario. No test changes needed. |
| `anvilml-registry` depends on `sqlx` with `runtime-tokio` feature, and `anvilml-hardware` already depends on `sqlx` with the same features. Adding `anvilml-registry` as a dependency could introduce a duplicate `sqlx` crate with different feature sets, causing compilation issues. | Low | High | Both crates already use `sqlx = { workspace = true }` which resolves to the same version and feature set. Cargo's dependency resolution deduplicates identical crate+feature combinations. Verify at compile time. |
| The step e2 loop iterates `devices.iter_mut()` after step e has already populated `capabilities_source = DeviceTable` from `DEVICE_DB`. Step e2 overwrites `capabilities_source` back to `DeviceTable` (same value) when a DB match is found, which is correct — it confirms the source is the database table rather than the compile-time table. | Low | Low | This is the intended behavior per the task spec. The debug log includes `source = "sqlite"` to distinguish the enrichment step from step e's `source = "device_db"`. |

## Acceptance Criteria

- [ ] `cargo test --workspace --features mock-hardware` exits 0
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
- [ ] `cargo fmt --all -- --check` exits 0
- [ ] `head -1 .forge/reports/P6-C2_plan.md` prints `# Plan Report: P6-C2`
- [ ] `grep "^## " .forge/reports/P6-C2_plan.md` shows exactly 11 section headings
- [ ] `wc -l .forge/reports/P6-C2_plan.md` returns a value greater than 40
