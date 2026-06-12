# Plan Report: P905-A1

| Field       | Value                                               |
|-------------|-----------------------------------------------------|
| Task ID     | P905-A1                                             |
| Phase       | 905 — FP8 dtype support                             |
| Description | anvilml-core: add F8E4M3 and F8E5M2 variants to DType enum |
| Depends on  | P20-A2                                              |
| Project     | anvilml                                             |
| Planned at  | 2026-06-12T09:33:00Z                                |
| Attempt     | 1                                                   |

## Objective

Add two new 8-bit floating-point variants (`F8E4M3` and `F8E5M2`) to the `DType` enum in `anvilml-core`, ensuring they serialize via serde as `f8_e4m3` and `f8_e5m2`, and update the existing tests and crate version accordingly.

## Scope

### In Scope
- Add `F8E4M3` and `F8E5M2` variants to `DType` enum in `crates/anvilml-core/src/types/model.rs`, placed after `BF16` and before `Q8`.
- Each variant gets a doc comment: `/// 8-bit float E4M3, torch float8_e4m3fn` and `/// 8-bit float E5M2, torch float8_e5m2`.
- The existing `#[serde(rename_all = "snake_case")]` attribute handles serialization to `f8_e4m3` / `f8_e5m2` automatically — no additional serde attributes needed.
- Update `dtype_variants` test: change expected count from 6 to 8, add `DType::F8E4M3` and `DType::F8E5M2` to the variants vec, and include them in the roundtrip array check.
- Update `dtype_roundtrip_json` test: add `DType::F8E4M3` and `DType::F8E5M2` to the dtypes array so they are also tested for JSON roundtrip.
- Add new test `dtype_f8_serde_strings` that asserts exact JSON serialization strings: `"[\"f8_e4m3\"]"` for F8E4M3 and `"[\"f8_e5m2\"]"` for F8E5M2 (or individual `serde_json::to_string(&DType::F8E4M3)` equals `"\"f8_e4m3\""`).
- Bump `anvilml-core` patch version in `crates/anvilml-core/Cargo.toml` from `0.1.2` to `0.1.3`.

### Out of Scope
- Changes to `anvilml-registry` scanner (handled by P905-A2).
- Changes to `anvilml-server` or any other crate.
- Changes to the OpenAPI schema (P905-A6 handles that).
- ModelMetaPatch or PATCH endpoint (handled by P905-A5).
- Stale model removal (handled by P905-A4).
- Cancel fix (handled by P905-A7).

## Approach

1. **Read current state.** Confirm `DType` enum in `crates/anvilml-core/src/types/model.rs` currently has 6 variants (F32, F16, BF16, Q8, Q4, Unknown) with `#[serde(rename_all = "snake_case")]`.

2. **Add F8E4M3 variant.** Insert after the `BF16` variant (line 24):
   ```rust
   /// 8-bit float E4M3, torch float8_e4m3fn
   F8E4M3,
   ```

3. **Add F8E5M2 variant.** Insert immediately after F8E4M3:
   ```rust
   /// 8-bit float E5M2, torch float8_e5m2
   F8E5M2,
   ```

4. **Update `dtype_variants` test.** Modify the variants vec to include all 8 variants in order:
   ```rust
   let variants: Vec<DType> = vec![
       DType::F32,
       DType::F16,
       DType::BF16,
       DType::F8E4M3,
       DType::F8E5M2,
       DType::Q8,
       DType::Q4,
       DType::Unknown,
   ];
   assert_eq!(variants.len(), 8, "must have exactly 8 variants");
   ```

5. **Update `dtype_roundtrip_json` test.** Add both new variants to the dtypes array:
   ```rust
   let dtypes = [
       DType::F32,
       DType::F16,
       DType::BF16,
       DType::F8E4M3,
       DType::F8E5M2,
       DType::Q8,
       DType::Q4,
       DType::Unknown,
   ];
   ```

6. **Add `dtype_f8_serde_strings` test.** New test asserting exact JSON strings:
   ```rust
   #[test]
   fn dtype_f8_serde_strings() {
       assert_eq!(
           serde_json::to_string(&DType::F8E4M3).expect("serialize F8E4M3"),
           "\"f8_e4m3\""
       );
       assert_eq!(
           serde_json::to_string(&DType::F8E5M2).expect("serialize F8E5M2"),
           "\"f8_e5m2\""
       );
   }
   ```

7. **Bump version.** Update `crates/anvilml-core/Cargo.toml`:
   - Change `version = "0.1.2"` to `version = "0.1.3"`.

8. **Verify.** Run `cargo test -p anvilml-core` — expect 75+ tests (74 existing + 1 new), all passing, exit code 0.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-core/src/types/model.rs` | Add F8E4M3 and F8E5M2 enum variants; update tests |
| Modify | `crates/anvilml-core/Cargo.toml` | Bump patch version 0.1.2 → 0.1.3 |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `crates/anvilml-core/src/types/model.rs` | `dtype_variants` | All 8 variants exist, are distinct, count=8 |
| `crates/anvilml-core/src/types/model.rs` | `dtype_roundtrip_json` | Both F8E4M3 and F8E5M2 roundtrip through JSON serialization |
| `crates/anvilml-core/src/types/model.rs` | `dtype_f8_serde_strings` (new) | Exact JSON strings: `"f8_e4m3"` and `"f8_e5m2"` |
| `crates/anvilml-core/src/types/model.rs` | `dtype_default_is_unknown` | Unchanged — still passes |
| `crates/anvilml-core/src/types/model.rs` | `model_meta_roundtrip` | Unchanged — still passes |
| `crates/anvilml-core/src/types/model.rs` | `model_meta_defaults` | Unchanged — still passes |
| `crates/anvilml-core/src/types/model.rs` | `model_meta_default_impl` | Unchanged — still passes |
| `crates/anvilml-core/src/types/model.rs` | `model_meta_scanned_at_default` | Unchanged — still passes |
| `crates/anvilml-core/src/types/model.rs` | `model_meta_serde_json_preserves_all_fields` | Unchanged — still passes |

## CI Impact

No CI changes required. The task only adds enum variants and tests within `anvilml-core`, which is already tested by the standard `cargo test --workspace --features mock-hardware` CI gate. The crate has no new dependencies, no feature flags, and no public API surface changes beyond the enum extension (which is additive and backward-compatible).

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `ToSchema` derive from utoipa may need regeneration for OpenAPI schema | Low | Medium — downstream crates using OpenAPI may reference stale schema | This task does not modify OpenAPI; P905-A6 handles schema regeneration. The `ToSchema` derive is additive and compiles fine. |
| Existing code in other crates pattern-matches exhaustively on `DType` and may get non-exhaustive warnings | Low | Medium — compiler warns on non-exhaustive matches | Rust enums without `#[non_exhaustive]` are exhaustive; adding variants breaks compilation in downstream crates. Since this task only touches `anvilml-core` and the PR will be reviewed before merging, downstream crates will be updated in subsequent tasks (P905-A2, etc.). |
| serde snake_case produces unexpected casing | Very low | Low — `F8E4M3` → `f8_e4m3` and `F8E5M2` → `f8_e5m2` follow standard snake_case rules | The `dtype_f8_serde_strings` test explicitly asserts the exact strings. |

## Acceptance Criteria

- [ ] `DType::F8E4M3` and `DType::F8E5M2` exist in `crates/anvilml-core/src/types/model.rs` after `BF16`
- [ ] Both variants serialize to `"f8_e4m3"` and `"f8_e5m2"` respectively via serde JSON
- [ ] `dtype_variants` test passes with count == 8
- [ ] `dtype_roundtrip_json` test passes for all 8 variants including F8E4M3 and F8E5M2
- [ ] `dtype_f8_serde_strings` test passes with exact string assertions
- [ ] `anvilml-core` version bumped to `0.1.3` in `Cargo.toml`
- [ ] `cargo test -p anvilml-core` exits 0 with >= 9 tests in `model.rs` (total >= 75)
