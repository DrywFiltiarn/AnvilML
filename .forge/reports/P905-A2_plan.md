# Plan Report: P905-A2

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P905-A2                                           |
| Phase       | 905 — anvilml-registry FP8 and rescan             |
| Description | anvilml-registry: extend infer_dtype with FP8 suffix matching and VRAM factor |
| Depends on  | P905-A1                                           |
| Project     | anvilml                                           |
| Planned at  | 2026-06-12T10:30:00Z                              |
| Attempt     | 1                                                 |

## Objective

Extend `infer_dtype()` in `anvilml-registry/src/scanner.rs` to recognize FP8 filename suffixes (`fp8`, `f8`, `fp8e4m3`, `fp8e5m2`, `f8e4m3`, `f8e5m2`, case-insensitive) and map them to the `F8E4M3` or `F8E5M2` DType variants added in task P905-A1. Verify that `vram_estimate_mib` already applies factor 0.5 for these types, and add corresponding unit tests.

## Scope

### In Scope
- Extend `infer_dtype()` with FP8 suffix matching (before the bf16 check)
- Confirm `vram_estimate_mib` VRAM factor for F8E4M3/F8E5M2 is 0.5 (already correct from P905-A1)
- Add `test_infer_dtype_fp8_suffixes` unit test covering all FP8 suffixes case-insensitively
- Add FP8 assertion to existing `test_vram_estimate_mib`
- Bump `anvilml-registry` patch version (0.1.0 → 0.1.1)

### Out of Scope
- Safetensor header inspection (task P905-A3)
- Stale model removal on rescan (task P905-A4)
- ModelMetaPatch type (task P905-A5)
- PATCH endpoint (task P905-A6)
- Cancel terminal job fix (task P905-A7)
- Any changes to `anvilml-core` (already done in P905-A1)
- Any changes to `anvilml-server`, `backend`, or other crates

## Approach

1. **Read current `infer_dtype()`** — it currently handles f32, bf16, fp16/f16, q8, q4, falling through to Unknown. The bf16 check must remain before the f16 check (bf16 ends with f16).

2. **Add FP8 suffix matching** before the bf16 check in `infer_dtype()`:
   - `f8e4m3` or `fp8e4m3` → `DType::F8E4M3`
   - `f8e5m2` or `fp8e5m2` → `DType::F8E5M2`
   - `fp8` or `f8` → `DType::F8E4M3` (default FP8 type)
   - All checks are case-insensitive (already using `.to_lowercase()`).
   - Order matters: `fp8e4m3`/`fp8e5m2` must be checked before `fp8`/`f8` since "fp8e4m3" ends with "fp8".

3. **Verify `vram_estimate_mib`** — confirm the existing match arm `DType::F8E4M3 | DType::F8E5M2 | DType::Q8 => 0.5` is correct. No code change needed; document in plan.

4. **Add `test_infer_dtype_fp8_suffixes`** — new `#[test]` function covering:
   - `model-fp8` → F8E4M3
   - `model-f8` → F8E4M3
   - `MODEL-FP8` → F8E4M3 (case-insensitive)
   - `model-fp8e4m3` → F8E4M3
   - `model-fp8e5m2` → F8E5M2
   - `model-f8e4m3` → F8E4M3
   - `model-f8e5m2` → F8E5M2

5. **Update `test_vram_estimate_mib`** — add one FP8 assertion:
   - `vram_estimate_mib(1048576, DType::F8E4M3) == 0` (0.5 MiB, clamped to 1)

6. **Bump `anvilml-registry` patch version** — edit `crates/anvilml-registry/Cargo.toml`: `version = "0.1.0"` → `version = "0.1.1"`.

7. **Run tests** — `cargo test -p anvilml-registry` must exit 0.

8. **Run format check** — `cargo fmt --all -- --check` must exit 0.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-registry/src/scanner.rs` | Extend `infer_dtype()` with FP8 suffixes; add `test_infer_dtype_fp8_suffixes`; add FP8 case to `test_vram_estimate_mib` |
| Modify | `crates/anvilml-registry/Cargo.toml` | Bump patch version 0.1.0 → 0.1.1 |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|-----------------|
| `scanner.rs` (existing) | `test_infer_dtype_matches` | Existing dtype suffixes still pass (no regression) |
| `scanner.rs` (existing) | `test_infer_dtype_case_insensitive` | Existing case-insensitive behavior preserved |
| `scanner.rs` (existing) | `test_infer_dtype_unknown` | Non-matching suffixes still return Unknown |
| `scanner.rs` (existing) | `test_vram_estimate_mib` | Updated with FP8 assertion; all existing assertions pass |
| `scanner.rs` (new) | `test_infer_dtype_fp8_suffixes` | All 7 FP8 suffixes map correctly, case-insensitive |

## CI Impact

No CI workflow files are modified. The task only touches `anvilml-registry` source and manifest. The existing CI gates (`cargo test --workspace --features mock-hardware`, clippy, format checks) will exercise the new code. No new CI jobs or steps are required.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| FP8 suffix patterns conflict with existing suffixes (e.g., `f8` matching something unintended) | Low | Medium | Order FP8 checks before bf16/f16; `f8` and `fp8` are distinct from all existing suffixes (f32, bf16, fp16, f16, q8, q4). Verify with test. |
| `fp8e4m3` ends with `fp8` — wrong match order | Low | Medium | Check `fp8e4m3`/`fp8e5m2` before `fp8`/`f8` in the if-else chain. |
| P905-A1 not yet implemented (F8E4M3/F8E5M2 types missing) | Low | High | Prerequisite checked; P905-A1 provides these variants. If blocked, report blocker. |
| Version bump conflicts with concurrent work | Low | Low | Patch bump is isolated to `anvilml-registry/Cargo.toml` `[package]` section only. |

## Acceptance Criteria

- [ ] `infer_dtype("model-fp8e4m3")` returns `DType::F8E4M3`
- [ ] `infer_dtype("model-fp8e5m2")` returns `DType::F8E5M2`
- [ ] `infer_dtype("model-f8e4m3")` returns `DType::F8E4M3`
- [ ] `infer_dtype("model-f8e5m2")` returns `DType::F8E5M2`
- [ ] `infer_dtype("model-fp8")` returns `DType::F8E4M3`
- [ ] `infer_dtype("model-f8")` returns `DType::F8E4M3`
- [ ] All FP8 matches are case-insensitive (`MODEL-FP8E4M3` → `F8E4M3`)
- [ ] Existing dtype suffixes (f32, bf16, fp16, f16, q8, q4) still return correct types
- [ ] `test_infer_dtype_fp8_suffixes` exists and passes
- [ ] `test_vram_estimate_mib` includes an FP8 assertion and passes
- [ ] `cargo test -p anvilml-registry` exits 0
- [ ] `anvilml-registry` version is 0.1.1
