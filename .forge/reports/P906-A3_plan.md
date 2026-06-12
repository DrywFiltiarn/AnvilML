# Plan Report: P906-A3

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P906-A3                                           |
| Phase       | 906 — OpenAPI Spec Correctness Retrofit           |
| Description | anvilml-core: fix BF16 serde rename (b_f16 -> bf16) |
| Depends on  | P905-A6, P905-A7                                  |
| Project     | anvilml                                           |
| Planned at  | 2026-06-12T15:10:00Z                              |
| Attempt     | 1                                                 |

## Objective

Fix the `DType::BF16` variant in `anvilml-core` so that its serde JSON string is `"bf16"` instead of the incorrect `"b_f16"` produced by `rename_all = "snake_case"` splitting `BF16` as three words (`B`, `F`, `16`). Add a unit test asserting the correct serialization. Bump the `anvilml-core` patch version.

## Scope

### In Scope
- Add `#[serde(rename = "bf16")]` to the `BF16` variant in `DType` enum (`crates/anvilml-core/src/types/model.rs`)
- Add test `dtype_bf16_serde_string` asserting `serde_json::to_string(&DType::BF16) == "\"bf16\""`
- Bump `anvilml-core` patch version from `0.1.3` to `0.1.4` in `crates/anvilml-core/Cargo.toml`
- Run `cargo test -p anvilml-core` to verify all tests pass (existing tests must still pass; the new test must pass)

### Out of Scope
- Regenerating `backend/openapi.json` (owned by P906-A4)
- Any changes to `anvilml-openapi`, `anvilml-server`, or other crates
- Changes to handler signatures, API routes, or utoipa annotations
- Changes to any other `DType` variant (F32, F16, F8E4M3, F8E5M2, Q8, Q4, Unknown are unaffected)

## Approach

1. **Add serde rename attribute.** In `crates/anvilml-core/src/types/model.rs`, on line 24 (the `BF16` variant), add `#[serde(rename = "bf16")]` immediately above it, matching the pattern already used for `F8E4M3` and `F8E5M2`.

2. **Add unit test.** In the existing `#[cfg(test)] mod tests` block in the same file, add:
   ```rust
   #[test]
   fn dtype_bf16_serde_string() {
       assert_eq!(
           serde_json::to_string(&DType::BF16).expect("serialize BF16"),
           "\"bf16\""
       );
   }
   ```

3. **Bump patch version.** In `crates/anvilml-core/Cargo.toml`, change `version = "0.1.3"` to `version = "0.1.4"`.

4. **Verify.** Run `cargo test -p anvilml-core` — all existing tests must pass, plus the new `dtype_bf16_serde_string` test.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-core/src/types/model.rs` | Add `#[serde(rename = "bf16")]` on `BF16` variant; add `dtype_bf16_serde_string` test |
| Modify | `crates/anvilml-core/Cargo.toml` | Bump patch version `0.1.3 → 0.1.4` |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `crates/anvilml-core/src/types/model.rs` | `dtype_bf16_serde_string` | `serde_json::to_string(&DType::BF16)` produces `"\"bf16\""` |
| `crates/anvilml-core/src/types/model.rs` | `dtype_roundtrip_json` | BF16 serialises to `"bf16"` and deserialises back to `DType::BF16` (roundtrip) |
| `crates/anvilml-core/src/types/model.rs` | `dtype_variants` | All 8 variants remain distinct (no regression) |
| `crates/anvilml-core/src/types/model.rs` | `model_meta_serde_json_preserves_all_fields` | ModelMeta with BF16 dtype serialises correctly |

## CI Impact

The OpenAPI Drift Gate (ENVIRONMENT.md §8, Gate 2) is triggered because `DType` is a `ToSchema`-derived type and its serde rename attribute is being changed. However, the actual regeneration of `backend/openapi.json` is owned by P906-A4, not this task. The gate will need to be run and passed in P906-A4 after all fixes from P906-A1 through P906-A3 are in place. No CI workflow file changes are required by this task.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Existing `dtype_roundtrip_json` test fails because it expects `"b_f16"` | Low | Medium | The roundtrip test serialises then deserialises — adding the explicit rename means both sides use `"bf16"`, so roundtrip still holds. Verify after implementation. |
| OpenAPI spec in `backend/openapi.json` becomes temporarily stale (contains `b_f16`) | Certain | Low | This is by design — A4 owns regeneration. Documented constraint in TASKS_PHASE906.md. |
| Version bump conflicts with concurrent work on anvilml-core | Low | Low | Patch bumps are additive; no other crate pins anvilml-core version. |

## Acceptance Criteria

- [ ] `#[serde(rename = "bf16")]` is present on the `BF16` variant in `crates/anvilml-core/src/types/model.rs`
- [ ] Test `dtype_bf16_serde_string` exists and passes
- [ ] `serde_json::to_string(&DType::BF16)` returns `"\"bf16\""`
- [ ] `cargo test -p anvilml-core` exits 0
- [ ] `anvilml-core` patch version bumped to `0.1.4` in `Cargo.toml`
