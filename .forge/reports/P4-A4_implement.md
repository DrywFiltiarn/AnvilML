# Implementation Report: P4-A4

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P4-A4                           |
| Phase         | 4 — Hardware Detection: Detectors |
| Description   | anvilml-hardware: VulkanDetector headless enumeration |
| Implemented   | 2026-06-29T00:30:00Z            |
| Status        | COMPLETE                          |

## Summary

Created `crates/anvilml-hardware/src/vulkan.rs` implementing `VulkanDetector: DeviceDetector` using the `ash` crate (v0.38.0+1.3.281) for headless Vulkan instance creation and physical device enumeration. The detector creates a minimal Vulkan 1.3 instance without surface extensions, enumerates all physical devices, and maps each to a `GpuDevice` by vendor ID (`0x10de` → `Cuda`, `0x1002` → `Rocm`). Unknown vendors are skipped. `detect()` never panics — loader absence or instance creation failure returns `Ok(vec![])`. `refresh_vram()` queries memory heap totals via `get_physical_device_memory_properties()`, using saturating arithmetic to prevent overflow on misreported heap sizes. 8 integration tests were created covering vendor ID mapping, detect error resilience, and refresh_vram behavior.

## Resolved Dependencies

| Type   | Name    | Version resolved | Source        |
|--------|---------|------------------|---------------|
| crate  | ash     | 0.38.0+1.3.281   | rust-docs MCP |
| crate  | tracing | 0.1              | MCP lookup    |

`ash` v0.38.0+1.3.281 is the latest on crates.io. The `loaded` default feature enables dynamic loading of the Vulkan loader (`libvulkan.so` / `vulkan-1.dll`) at runtime. `tracing` was added as a dependency to support mandatory debug log points in the Vulkan detection path.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-hardware/src/vulkan.rs` | `VulkanDetector` struct and `DeviceDetector` impl with `detect()` and `refresh_vram()` |
| MODIFY | `crates/anvilml-hardware/Cargo.toml` | Added `ash = "0.38.0"` and `tracing = "0.1"` dependencies; bumped version 0.1.2 → 0.1.3 |
| MODIFY | `crates/anvilml-hardware/src/lib.rs` | Added `pub mod vulkan;`, `pub use vulkan::VulkanDetector;`, `pub use vulkan::vendor_id_to_device_type;` |
| CREATE | `crates/anvilml-hardware/tests/vulkan_tests.rs` | 8 integration tests for `VulkanDetector` |
| MODIFY | `docs/TESTS.md` | Added 8 test entries for new vulkan_tests |

## Commit Log

```
 .../anvilml-hardware/Cargo.toml            |  4 +++-
 .../anvilml-hardware/src/lib.rs            |  3 +++
 .../anvilml-hardware/src/vulkan.rs         | 293 +++++++++++++++++++++++++++++
 .../anvilml-hardware/tests/vulkan_tests.rs | 127 +++++++++++++
 docs/TESTS.md                              | 112 ++++++++++++
 5 files changed, 538 insertions(+), 1 deletion(-)
```

## Test Results

```
     Running tests/vulkan_tests.rs (target/debug/deps/vulkan_tests-b618aba930c987d8)

running 8 tests
test test_vulkan_amd_vendor_maps_to_rocm ... ok
test test_vulkan_intel_vendor_skipped ... ok
test test_vulkan_nvidia_vendor_maps_to_cuda ... ok
test test_vulkan_unknown_vendor_skipped ... ok
test test_vulkan_detect_returns_empty_when_no_gpu ... ok
test test_vulkan_detect_never_errors ... ok
test test_vulkan_refresh_vram_out_of_range ... ok
test test_vulkan_refresh_vram_never_errors ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
```

Full workspace test suite: 132 tests passed, 0 failed, 0 ignored.

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware → Finished (0.74s)

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu → Finished (25.75s)

# 3. Real-hardware Linux
cargo check --bin anvilml → Finished (21.48s)

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu → Finished (19.28s)
```

All four cross-checks exited 0.

## Project Gates

Gate 1 (config_reference): `cargo test -p anvilml --features mock-hardware -- config_reference` → 1 passed, 0 failed.

No config fields were added/modified by this task, so only Gate 1 was triggered and passed.

## Public API Delta

```
+pub mod vulkan;
+pub use vulkan::VulkanDetector;
+pub use vulkan::vendor_id_to_device_type;
```

New `pub` items:
- `pub mod vulkan` — new module in `anvilml_hardware`
- `pub struct VulkanDetector` — in `anvilml_hardware::vulkan`
- `pub fn vendor_id_to_device_type(vendor_id: u32) -> Option<DeviceType>` — in `anvilml_hardware::vulkan`

## Deviations from Plan

- `vendor_id_to_device_type` was specified as `pub(crate)` in the plan, but integration tests in `tests/` are separate crates and cannot see `pub(crate)` items. Changed to `pub` and re-exported from `lib.rs` so the function is accessible from integration tests. This is the minimal correct fix — the function is still crate-internal in spirit (no public docs beyond the doc comment) and is only exposed to satisfy the test isolation requirement.
- The `ash` crate's builder methods use non-prefixed names (`application_name` / `application_info`) rather than the `p_`-prefixed names found in older ash versions. This was confirmed via MCP docs and used accordingly.
- The deprecated `ash::vk::version_major/minor/patch` functions were replaced with inline bit extraction (`(v >> 22) & 0x3FFF`, etc.) to avoid clippy deprecation warnings.
- A clippy `manual_c_str_literals` warning was fixed by using `c""` literal syntax instead of `CStr::from_bytes_with_nul(b"...\0")`.
- Integer overflow was discovered during testing when summing memory heap sizes. Fixed with `saturating_add` fold to prevent panics on misreported heap values.

## Blockers

None.
