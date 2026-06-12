# Plan Report: P905-A3

| Field       | Value                                         |
|-------------|-----------------------------------------------|
| Task ID     | P905-A3                                       |
| Phase       | 905 — Safetensors dtype detection             |
| Description | anvilml-registry: safetensor header inspection for dtype detection |
| Depends on  | P905-A2                                       |
| Project     | anvilml                                       |
| Planned at  | 2026-06-12T13:30:00Z                          |
| Attempt     | 1                                             |

## Objective

Add safetensors header-based dtype detection to the model registry scanner. Safetensors files store a JSON header at the start containing per-tensor dtype strings (e.g. "F32", "F16", "BF16", "F8_E4M3", "F8_E5M2", "I8", "I4"). By reading this header we can determine the model's dtype from the actual weight types rather than relying solely on filename suffixes, which may be missing or misleading.

## Scope

### In Scope
- Add `fn read_safetensors_dtype(path: &Path) -> Option<DType>` to `crates/anvilml-registry/src/scanner.rs`:
  - Read 8-byte little-endian u64 as header length
  - Guard: if header_len > 100 MiB (107_374_182 bytes), return None
  - Read header_len bytes from file
  - Parse bytes as UTF-8 JSON object
  - For each key in the JSON object (skip `__metadata__`), extract the string value
  - Count occurrences of each dtype string across all keys
  - Return the most-frequent dtype mapped to `DType` via: F32→F32, F16→F16, BF16→BF16, F8_E4M3→F8E4M3, F8_E5M2→F8E5M2, I8→Q8, I4→Q4
  - On any error (read, parse, or decode failure), return None
- Modify `scan_dirs` to call `read_safetensors_dtype` for `.safetensors` files before falling back to `infer_dtype`
  - Use header result if Some and != Unknown; else use `infer_dtype` from filename
- Add unit tests in `scanner.rs` and integration test
- Bump `anvilml-registry` patch version (0.1.1 → 0.1.2)

### Out of Scope
- Changes to any other crate
- Changes to the store layer, handlers, or API surface
- Changes to model hash computation or ID generation
- Handling of non-safetensors file formats (ckpt, pt, bin)
- Changes to CI workflow files
- Changes to `anvilml-core` types

## Approach

1. **Add `read_safetensors_dtype` function** in `crates/anvilml-registry/src/scanner.rs`:
   - Use `std::fs::File` and `std::io::Read` (already available via std)
   - Read first 8 bytes as little-endian u64 (`u64::from_le_bytes`)
   - Guard: `if header_len > 100 * 1024 * 1024 { return None; }`
   - Allocate buffer of `header_len` bytes, read into it
   - Convert to UTF-8 string with `std::str::from_utf8`
   - Parse with `serde_json::from_str` to get `serde_json::Value::Object`
   - Iterate keys: skip `__metadata__`; for each value, if it's a string, increment its count in a `HashMap<String, u32>`
   - Find the key with maximum count; map via a helper function
   - Return `Some(mapped_dtype)` or `None` on any error

2. **Modify `scan_dirs`** in the same file:
   - After computing `stem` (file stem), before calling `infer_dtype(stem)`:
   - If extension is `safetensors`, call `read_safetensors_dtype(entry.path())`
   - If result is `Some(DType::Unknown)` or `None`, fall through to `infer_dtype(stem)`
   - If result is `Some(dtype)` where dtype != Unknown, use it directly

3. **Add unit tests** in `scanner.rs` `mod tests`:
   - `test_read_safetensors_dtype_header_wins`: create temp dir, write a valid safetensors header with mostly F16 keys, write file as `model-f32.safetensors`, verify dtype is F16 (header wins over filename)
   - `test_read_safetensors_dtype_fallback_malformed`: write invalid binary data as `.safetensors`, verify scanner falls back to `infer_dtype` and returns Q8 (from filename)
   - `test_read_safetensors_dtype_fp8_header`: write valid safetensors header with F8_E4M3 keys, verify dtype is F8E4M3

4. **Add integration test** in `crates/anvilml-registry/tests/safetensors_header.rs`:
   - `test_safetensors_header_dtype_detection`: create temp dir with a safetensors file whose header declares F16 but filename says F32, scan it, assert dtype_hint is F16

5. **Bump version** in `crates/anvilml-registry/Cargo.toml`: `0.1.1` → `0.1.2`

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-registry/src/scanner.rs` | Add `read_safetensors_dtype`, modify `scan_dirs`, add unit tests |
| Create   | `crates/anvilml-registry/tests/safetensors_header.rs` | Integration test for header-based dtype detection |
| Modify | `crates/anvilml-registry/Cargo.toml` | Bump patch version 0.1.1 → 0.1.2 |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `scanner.rs` (unit) | `test_read_safetensors_dtype_header_wins` | Header F16 overrides filename suffix f32 → dtype is F16 |
| `scanner.rs` (unit) | `test_read_safetensors_dtype_fallback_malformed` | Invalid header → None → falls back to `infer_dtype(Q8)` |
| `scanner.rs` (unit) | `test_read_safetensors_dtype_fp8_header` | Header with F8_E4M3 → returns DType::F8E4M3 |
| `tests/safetensors_header.rs` (integration) | `test_safetensors_header_dtype_detection` | Full `scan_dirs` pipeline with safetensors file, header dtype wins |

## CI Impact

No CI workflow changes. The existing `cargo test -p anvilml-registry` command will run the new tests. All four platform cross-check commands from ENVIRONMENT.md §7 will be run as normal. No OpenAPI drift (no handler or ToSchema changes). No config surface sync needed.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Safetensors header format changes across libraries | Low | Medium | The spec is stable; we parse generic JSON, not a fixed schema. Any future format change would require a version field in the header, which we don't use. |
| Large safetensors files (>100 MiB header) cause OOM | Low | High | Guard clause rejects headers >100 MiB immediately, returning None |
| Malformed JSON or non-string values in header | Medium | Low | `serde_json` handles parse errors; we check `Value::String` before counting. Non-string values are silently skipped. |
| Test temp directory cleanup conflicts | Low | Low | Each test uses `tempfile::tempdir()` which auto-cleans on drop. No shared state between tests. |
| `std::fs::File` and `std::io::Read` already available | None | None | These are from std, no new dependency needed. `serde_json` is already a dependency. |

## Acceptance Criteria

- [ ] `read_safetensors_dtype` reads 8-byte LE u64 header length, guards >100 MiB, parses JSON, counts dtypes, returns most-frequent mapped DType
- [ ] `scan_dirs` calls `read_safetensors_dtype` for `.safetensors` files and uses header result when available and != Unknown
- [ ] Unit tests `test_read_safetensors_dtype_header_wins`, `test_read_safetensors_dtype_fallback_malformed`, `test_read_safetensors_dtype_fp8_header` pass
- [ ] Integration test `test_safetensors_header_dtype_detection` passes
- [ ] `cargo test -p anvilml-registry` exits 0
- [ ] `anvilml-registry` patch version bumped to 0.1.2
