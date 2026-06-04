# Plan Report: P7-E1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P7-E1                                              |
| Phase       | 007 — WebSocket Event Stream                      |
| Description | anvilml: upgrade thiserror to 2.x and sha2 to 0.11.x |
| Depends on  | P7-C1                                              |
| Project     | anvilml                                            |
| Planned at  | 2026-06-05T14:32:00Z                              |
| Attempt     | 1                                                  |

## Objective

Bump two workspace dependency versions in `[workspace.dependencies]` of the root `Cargo.toml`: `thiserror` from `"1.0.69"` to `"2"`, and `sha2` from `"0.10.8"` to `"0.11"`. Both bumps are zero or near-zero migration cost — thiserror's derive macro is never invoked in the codebase, and sha2 usage is entirely local with no public trait bounds.

## Scope

### In Scope
- Update `thiserror` version string from `"1.0.69"` to `"2"` in `[workspace.dependencies]` of root `Cargo.toml`.
- Update `sha2` version string from `"0.10.8"` to `"0.11"` in `[workspace.dependencies]` of root `Cargo.toml`.
- Verify that `hex 0.4.3` (current workspace pin) is compatible with sha2 0.11's output type (`GenericArray<u8, N>` implements `AsRef<[u8]>`, which `hex::encode` accepts). If incompatible, bump `hex` to its current stable in the same change.
- Run `cargo build --workspace --features mock-hardware` and `cargo test --workspace --features mock-hardware` to verify no regressions.

### Out of Scope
- No code changes in `error.rs` — `AnvilError` implements `Display`, `Error`, and `From` manually; the derive macro is never used.
- No code changes in `scanner.rs` — sha2 API (`Sha256::new()`, `.update()`, `.finalize()`) is stable across 0.10→0.11 for this usage pattern.
- No new tests, no CI changes, no other dependency bumps.
- No modifications to any per-crate `Cargo.toml` files — they already reference `{ workspace = true }`.

## Approach

1. **Verify MCP lookups** (already performed in PLAN session):
   - `thiserror`: latest stable is **2.0.18** (2.x series). No public API surface — deliberate non-breaking change per the crate's own documentation: "switching from handwritten impls to thiserror or vice versa is not a breaking change."
   - `sha2`: latest in 0.11.x series is **0.11.0**. Depends on `digest ^0.11`. The `Digest` trait's `finalize()` returns `GenericArray<u8, N>` which implements `AsRef<[u8]>`.
   - `hex 0.4.3`: accepts `impl AsRef<[u8]>` in `hex::encode()`, so sha2 0.11 output is compatible. No bump needed.

2. **Edit root `Cargo.toml`** `[workspace.dependencies]`:
   - Change line: `thiserror = "1.0.69"` → `thiserror = "2"`
   - Change line: `sha2 = "0.10"` → `sha2 = "0.11"`

3. **Build verification**: Run `cargo build --workspace --features mock-hardware` to confirm zero compilation errors.

4. **Test verification**: Run `cargo test --workspace --features mock-hardware` to confirm all existing tests pass (including `test_sha256_hex` in `scanner.rs`).

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Edit | `Cargo.toml` | Bump `thiserror` from `"1.0.69"` to `"2"` and `sha2` from `"0.10"` to `"0.11"` in `[workspace.dependencies]` |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `crates/anvilml-registry/src/scanner.rs` (unit) | `test_sha256_hex` | SHA-256 of "hello world" produces the correct known digest — confirms sha2 0.11 output matches expected hex string |

No new test files are needed. The existing test suite is sufficient to detect any regression from the sha2 version bump.

## CI Impact

No CI changes required. The task only modifies dependency version strings in `[workspace.dependencies]`. No CI workflow files, no new jobs, and no new test files are introduced.

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| `hex 0.4.3` may not compile against sha2 0.11's `GenericArray` output type if `AsRef<[u8]>` is missing | Verified via MCP lookup: `digest::array::GenericArray<u8, N>` implements `AsRef<[u8]>`. If unexpected incompatibility surfaces during build, bump `hex` to its current stable in the same commit. |
| Some transitive dependency may have a hard MSRV conflict with thiserror 2.x | thiserror 2.x has minimal MSRV requirements (Rust 1.56+). The workspace already targets a much newer toolchain via `rust-toolchain.toml`. No action needed. |
| `sha2` 0.11 depends on `digest 0.11` which may introduce different default features than 0.10 | Both versions use the same soft backend by default. No feature flag changes needed in the workspace. |

## Acceptance Criteria

- [ ] `cargo build --workspace --features mock-hardware` exits 0
- [ ] `cargo test --workspace --features mock-hardware` exits 0
