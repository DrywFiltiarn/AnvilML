# Plan Report: P7-F0

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P7-F0                                             |
| Phase       | 007 — WebSocket Event Stream                      |
| Description | anvilml-core: extend InferenceCaps with fp32, fp8, fp4, nvfp4 fields |
| Depends on  | P7-E3                                             |
| Project     | anvilml                                           |
| Planned at  | 2026-06-05T10:00:00Z                              |
| Attempt     | 1                                                 |

## Objective

Extend the `InferenceCaps` struct in `anvilml-core` with four new boolean capability fields (`fp32`, `fp8`, `fp4`, `nvfp4`) while preserving the existing three fields (`fp16`, `bf16`, `flash_attention`). Update all struct literal constructions, the `or_all_caps` OR-reduction function, and the CLI hardware table printer across six files in a single atomic change.

## Scope

### In Scope
- Add four `bool` fields to `InferenceCaps` in canonical order: `fp32`, `fp16`, `bf16`, `fp8`, `fp4`, `nvfp4`, `flash_attention`
- Add `#[serde(default)]` to each new field for backward-compatible JSON deserialization
- Update `or_all_caps()` in `anvilml-hardware/src/lib.rs` to OR all 7 fields
- Update every `InferenceCaps` struct literal across the workspace (6 files) to include the 4 new fields defaulting to `false`
- Update `print_hardware_table()` in `backend/src/main.rs` to display all 7 flags in canonical order
- Update tests in `hardware.rs`, `lib.rs`, and `cpu.rs` that assert on individual capability fields

### Out of Scope
- Changes to `DeviceCapabilityEntry` (that is P7-F3)
- SQLite migration DDL (that is P7-F1)
- Device capability store (that is P7-F2)
- Any change to `device_db.rs` struct literals or `resolve_caps()` — those are deferred to P7-F3
- Changes to `anvilml-ipc`, `anvilml-worker`, `anvilml-scheduler`, `anvilml-server` crates

## Approach

### Step 1 — Extend `InferenceCaps` in `hardware.rs`

Add four new `bool` fields between the existing `bf16` and `flash_attention` fields, maintaining canonical order:

```
fp32 (new), fp16 (existing), bf16 (existing), fp8 (new), fp4 (new), nvfp4 (new), flash_attention (existing)
```

Each new field gets `#[serde(default)]` and `pub`. The struct already derives `Default` — since all fields are `bool`, the default is unchanged (all `false`). No change to the derive list.

### Step 2 — Update tests in `hardware.rs`

Update every `InferenceCaps` struct literal in this file to include the four new fields with value `false`:
- `inference_caps_roundtrip` test (line ~163): add `fp32: false, fp8: false, fp4: false, nvfp4: false`
- `gpu_device_roundtrip` test (lines ~210-214, ~297-301, ~315-318): add the four new fields to each `caps:` literal
- `hardware_info_roundtrip` test (line ~324): add the four new fields to the `inference_caps:` literal

Update the assertion tests that check individual fields:
- `inference_caps_defaults` (line ~153): add assertions for `!caps.fp32`, `!caps.fp8`, `!caps.fp4`, `!caps.nvfp4`
- `gpu_device_backward_compat` (line ~265): add assertions for the new fields defaulting to `false`

### Step 3 — Extend `or_all_caps()` in `lib.rs`

Add three OR-lines for the new fields:
```rust
result.fp32 |= caps.fp32;
result.fp8 |= caps.fp8;
result.fp4 |= caps.fp4;
result.nvfp4 |= caps.nvfp4;
```

### Step 4 — Update `or_all_caps` tests in `lib.rs`

Update the two test functions that construct `InferenceCaps` literals:
- `or_all_caps_merges` (line ~356): add the four new fields to both `caps_a` and `caps_b`
- `or_all_caps_empty` (line ~375): add assertions for the new fields

### Step 5 — Update `cpu.rs`

The CPU detector uses `anvilml_core::InferenceCaps::default()` which is already correct — no struct literal changes needed. Only update the test assertion at line ~84-86 to check the new fields:
```rust
assert!(!dev.caps.fp32);
assert!(!dev.caps.fp8);
assert!(!dev.caps.fp4);
assert!(!dev.caps.nvfp4);
```

### Step 6 — Update `mock.rs`

Same as cpu.rs: uses `InferenceCaps::default()` which is correct. No struct literal changes needed. The existing tests do not assert on individual capability fields, so no test updates required.

### Step 7 — Update `device_db.rs`

The `resolve_caps()` function (line ~127) constructs an `InferenceCaps` literal with three fields. Update it to include the four new fields defaulting to `false`:
```rust
dev.caps = anvilml_core::InferenceCaps {
    fp32: false,
    fp16: entry.fp16,
    bf16: entry.bf16,
    fp8: false,
    fp4: false,
    nvfp4: false,
    flash_attention: entry.flash_attention,
};
```

Also update the `InferenceCaps::default()` call at line ~142 — this is already correct (no literal change needed).

### Step 8 — Update `print_hardware_table()` in `main.rs`

Replace the current caps string builder logic (lines ~44-53) with one that checks all 7 fields in canonical order. The new display should show capabilities like:
```
FP32+FP16+BF16+FP8+FP4+NVFP4+FA  (all true)
FP16+BF16                          (only these two)
-                                  (none true)
```

A simple approach: iterate through the 7 fields in order, collect non-zero ones with their label, join with `+`. If empty, show `-`.

Update the summary section at lines ~74-78 to list all 7 flags:
```
FP32: false  FP16: true  BF16: false  FP8: false  FP4: false  NVFP4: false  Flash Attention: true
```

### Step 9 — Verify

Run the acceptance criterion commands:
- `cargo test --workspace --features mock-hardware` — must exit 0
- `cargo clippy --workspace -- -D warnings` — must exit 0

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-core/src/types/hardware.rs` | Add 4 fields to InferenceCaps; update all struct literals and test assertions in this file |
| Modify | `crates/anvilml-hardware/src/lib.rs` | Extend or_all_caps() with 4 new OR-lines; update test literals |
| Modify | `crates/anvilml-hardware/src/cpu.rs` | Update test assertion for new capability fields |
| Modify | `crates/anvilml-hardware/src/device_db.rs` | Update InferenceCaps literal in resolve_caps() |
| Modify | `backend/src/main.rs` | Update print_hardware_table caps display logic and summary line |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `crates/anvilml-core/src/types/hardware.rs` | `inference_caps_defaults` | All 7 fields default to false |
| `crates/anvilml-core/src/types/hardware.rs` | `inference_caps_roundtrip` | Serialization/deserialization of InferenceCaps with all 7 fields |
| `crates/anvilml-core/src/types/hardware.rs` | `gpu_device_backward_compat` | Old JSON (without new fields) deserializes to false for new fields |
| `crates/anvilml-core/src/types/hardware.rs` | `gpu_device_roundtrip` | GpuDevice with InferenceCaps round-trips through JSON |
| `crates/anvilml-core/src/types/hardware.rs` | `hardware_info_roundtrip` | HardwareInfo with multiple GPUs and InferenceCaps round-trips |
| `crates/anvilml-hardware/src/lib.rs` | `or_all_caps_merges` | OR-reduction works across all 7 fields for multiple devices |
| `crates/anvilml-hardware/src/lib.rs` | `or_all_caps_empty` | Empty list returns all-false InferenceCaps |
| `crates/anvilml-hardware/src/cpu.rs` | `cpu_device_new_fields` | CPU device capability flags are all false |

## CI Impact

No CI workflow files are modified. The existing CI matrix (`rust`, `python-worker`, `openapi-diff`, `rust-windows`) already runs the relevant commands: `cargo clippy --workspace --features mock-hardware -- -D warnings` and `cargo test --workspace --features mock-hardware`. This task only adds fields to a struct — it does not change any external API surface, handler signatures, or CI configuration.

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Missing a struct literal site causes compilation failure | The task description explicitly lists all 6 files that contain InferenceCaps literals; each is audited individually in the Approach section |
| `or_all_caps` test asserts only on old fields, missing new field coverage | Tests are explicitly updated to assert on all 7 fields (Approach Steps 4) |
| Backward compatibility: old JSON without new fields fails to deserialize | All new fields use `#[serde(default)]` which maps missing keys to `false` for bools; the existing `gpu_device_backward_compat` test validates this |
| `print_hardware_table` caps display becomes overly complex or breaks formatting | Use a simple iterative approach: collect non-false field names and join with `+`; fall back to `-` if none are true. Width of the table is unchanged since the summary line is on a separate row |

## Acceptance Criteria

- [ ] `cargo test --workspace --features mock-hardware` exits 0
- [ ] `cargo clippy --workspace -- -D warnings` exits 0
