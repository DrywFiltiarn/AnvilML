# Plan Report: P11-B1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P11-B1                                            |
| Phase       | 011 — Graph Validation                            |
| Description | anvilml-hardware: add clear_mock_env teardown to all serial mock tests to eliminate env-var bleed |
| Depends on  | P11-A1                                            |
| Project     | anvilml                                           |
| Planned at  | 2026-06-07T12:53:00Z                              |
| Attempt     | 1                                                 |

## Objective

Eliminate environment-variable bleed between `#[serial]` tests in `crates/anvilml-hardware/src/mock.rs` and `src/lib.rs` by adding a private `clear_mock_env()` helper that removes `ANVILML_MOCK_DEVICE_TYPE`, `ANVILML_MOCK_VRAM_MIB`, and `ANVILML_MOCK_GFX_ARCH`, and calling it as the last statement of every serial mock test that sets any of these variables.

## Scope

### In Scope
- Add private `fn clear_mock_env()` to `crates/anvilml-hardware/src/mock.rs` inside the existing `#[cfg(test)] mod tests`.
- Add private `fn clear_mock_env()` to `crates/anvilml-hardware/src/lib.rs` inside the existing `#[cfg(test)] mod tests`.
- Append `clear_mock_env();` as the final statement in each of the following test functions:

  **mock.rs (4 tests):**
  - `mock_detect_default_cpu`
  - `mock_detect_cuda`
  - `mock_detect_rocm`
  - `mock_device_new_fields`

  **lib.rs (6 tests):**
  - `detect_all_devices_mock_cuda`
  - `detect_all_devices_mock_rocm`
  - `detect_all_devices_mock_vram`
  - `detect_all_devices_mock_device_type`
  - `detect_all_devices_mock_enum_source`
  - `mock_device_new_fields_in_detect_all`

- No new `set_var` calls — teardown only.
- No changes to test assertions, logic, or ordering.
- No crate version bump needed (task is a test-only fix; no source files modified).

### Out of Scope
- Any changes to non-mock tests.
- Changes to any other crate (`anvilml-scheduler`, `anvilml-worker`, etc.).
- Adding new test files or modules.
- Modifying the mock detector implementation itself.
- Changing `serial_test` dependency or configuration.

## Approach

1. **Add `clear_mock_env()` to `mock.rs`.** Inside `mod tests` (line 65), after the existing imports and before the first test function, insert:
   ```rust
   fn clear_mock_env() {
       std::env::remove_var("ANVILML_MOCK_DEVICE_TYPE");
       std::env::remove_var("ANVILML_MOCK_VRAM_MIB");
       std::env::remove_var("ANVILML_MOCK_GFX_ARCH");
   }
   ```

2. **Append `clear_mock_env();` to each of the 4 tests in `mock.rs`.** Each test ends with a closing `}` on its own line. Replace that final `}` with `clear_mock_env();\n    }`.

3. **Add `clear_mock_env()` to `lib.rs`.** Inside `mod tests` (line 340), after the existing imports and before the first non-mock test (`vendor_map_cuda` at line 378), insert the same function body as in step 1.

4. **Append `clear_mock_env();` to each of the 6 tests in `lib.rs`.** Each test ends with a closing `}` on its own line. Replace that final `}` with `clear_mock_env();\n    }`.

5. **Verify.** Run:
   ```bash
   cargo test -p anvilml-hardware --features mock-hardware
   ```
   Confirm all 48 tests pass. Then run 20 consecutive iterations of:
   ```bash
   cargo test -p anvilml-hardware --features mock-hardware -- --test-threads=1
   ```
   to confirm no ordering-dependent failures remain.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-hardware/src/mock.rs` | Add `clear_mock_env()` helper; append teardown call to 4 tests |
| Modify | `crates/anvilml-hardware/src/lib.rs` | Add `clear_mock_env()` helper; append teardown call to 6 tests |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| (existing) | All 48 existing tests in both files | Existing assertions still pass after teardown addition |
| (manual) | 20 consecutive `--test-threads=1` runs | No ordering-dependent env-var bleed remains |

## CI Impact

No CI changes required. The modification is purely additive test teardown code within the existing `#[cfg(test)]` blocks. No new dependencies, no changed test names, no changed assertions. The existing CI gate `cargo test --workspace --features mock-hardware` already covers this crate.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Test order dependency shifts after adding teardown calls | Low | Medium — a test that previously relied on env var bleed might now fail because the var is cleaned up | Run 20 consecutive `--test-threads=1` iterations; if any failure, investigate which test depends on stale state and adjust |
| `clear_mock_env()` called after assertion failure (panic) doesn't run | Low | Medium — a panicked test leaves vars set for subsequent tests | Acceptable: `serial_test` serialises by mutex so the next test would inherit bleed; however, panic in one test already breaks the run. The teardown is best-effort and sufficient for normal success paths |
| Duplicate function definition if plan agent merges with prior partial work | Very Low | High — compilation error | Only write if the file does not already contain `clear_mock_env`; grep before writing |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-hardware --features mock-hardware` exits 0 with all 48 tests passing
- [ ] 20 consecutive runs of `cargo test -p anvilml-hardware --features mock-hardware -- --test-threads=1` each exit 0 with 48 passing (no ordering-dependent failures)
- [ ] No new `set_var` calls added for any variable a test does not itself set
- [ ] Only `mock.rs` and `lib.rs` in `crates/anvilml-hardware/src/` are modified
