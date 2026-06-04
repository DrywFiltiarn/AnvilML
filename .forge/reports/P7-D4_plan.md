# Plan Report: P7-D4

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P7-D4                                       |
| Phase       | 007 — WebSocket Event Stream                |
| Description | fix OS field blank and stray colon in --print-hardware output |
| Depends on  | P7-D3                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-06-04T22:15:00Z                        |
| Attempt     | 1                                           |

## Objective

Fix two bugs in `--print-hardware` output: (1) the OS line prints a blank field with a stray trailing colon because the format string passes `" ".repeat(50 - 8)` instead of `hw.host.os`, and (2) `populate_host_info()` stores an empty string for `HostInfo.os` on Windows because `sysinfo::System::name()` returns `Some("")` rather than `None`.

## Scope

### In Scope
- `backend/src/main.rs`: fix the OS `println!` in `print_hardware_table()` to interpolate `hw.host.os` and remove the stray trailing colon.
- `crates/anvilml-hardware/src/lib.rs`: fix `populate_host_info()` to use `System::long_os_version()` as primary source with `.filter(|s| !s.is_empty())` guard, falling back to `System::name()`.

### Out of Scope
- No new tests (per TASKS_PHASE007.md: "no automated test addition is required for this task" — the acceptance criterion is manual verification via `cargo run -- --print-hardware`).
- No dependency upgrades.
- No changes to CI, config files, or other crates.

## Approach

1. **Fix `backend/src/main.rs` line 17.** Replace:
   ```rust
   println!("║ OS:          {}:", " ".repeat(50 - 8));
   ```
   with:
   ```rust
   println!("║ OS:          {}", hw.host.os);
   ```

2. **Fix `crates/anvilml-hardware/src/lib.rs` line 84.** Replace:
   ```rust
   let os = sysinfo::System::name().unwrap_or_else(|| "Unknown".to_string());
   ```
   with:
   ```rust
   let os = sysinfo::System::long_os_version()
       .or_else(|| sysinfo::System::name())
       .filter(|s| !s.is_empty())
       .unwrap_or_else(|| "Unknown".to_string());
   ```

3. **Verify compilation** with `cargo build --workspace --features mock-hardware` and `cargo clippy --workspace --features mock-hardware -- -D warnings`.

4. **Run tests** with `cargo test --workspace --features mock-hardware` — the existing `host_info_populated()` test asserts `!info.host.os.is_empty()`, which will catch any regression.

5. **Manual acceptance criterion**: `cargo run -- --print-hardware` prints a non-empty OS string on the `OS:` line with no trailing stray colon.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `backend/src/main.rs` | Fix OS `println!` in `print_hardware_table()` (line 17) |
| Modify | `crates/anvilml-hardware/src/lib.rs` | Fix `populate_host_info()` OS resolution (line 84) |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `crates/anvilml-hardware/src/lib.rs` (inline tests) | `host_info_populated` | Asserts `info.host.os.is_empty()` is false — will catch the empty-string bug if regression occurs |
| `crates/anvilml-hardware/src/lib.rs` (inline tests) | `detect_all_devices_never_errs` | Ensures `detect_all_devices` still returns `Ok` |
| `crates/anvilml-hardware/src/lib.rs` (inline tests) | `or_all_caps_*` | Regression guard for unrelated code |

No new test files are created. Per TASKS_PHASE007.md: "This function has no unit test covering the OS string value (it calls live sysinfo APIs). The acceptance criterion is verified manually via `--print-hardware`; no automated test addition is required for this task."

## CI Impact

No CI changes. This task modifies only source files — no workflow, config, or dependency updates. The existing Rust CI jobs (`rust`, `rust-windows`) will run `cargo clippy` and `cargo test --workspace --features mock-hardware`, both of which must pass.

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| `System::long_os_version()` may not be available in the current sysinfo version | The task description confirms sysinfo 0.32 has this API; the existing code already imports `sysinfo` and calls `System::name()`, so the crate is a dependency. Verified via MCP lookup that `long_os_version()` exists in sysinfo 0.32+. |
| `System::long_os_version()` returns `Some("")` on some Windows builds | The `.filter(|s| !s.is_empty())` guard ensures both `long_os_version()` and `name()` empty-string results are treated as absent, falling through to `"Unknown"`. |
| The `host_info_populated` test may fail if sysinfo returns no OS string on the CI runner | The existing test already asserts `!info.host.os.is_empty()`, which would currently pass or fail depending on the CI environment. The fix improves the likelihood of a non-empty value without changing the assertion. |
| No automated regression test for the stray-colon fix | Manual verification via `cargo run -- --print-hardware` is sufficient for this one-line format string change; the risk of introducing another bug in that line is minimal. |

## Acceptance Criteria

- [ ] `cargo build --workspace --features mock-hardware` exits 0
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
- [ ] `cargo test --workspace --features mock-hardware` exits 0 (including `host_info_populated`)
- [ ] `cargo run -- --print-hardware` prints a non-empty OS string on the `OS:` line with no trailing stray colon
