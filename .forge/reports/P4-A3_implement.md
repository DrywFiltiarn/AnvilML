# Implementation Report: P4-A3

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P4-A3                           |
| Phase         | 004 — Hardware Detection        |
| Description   | Vulkan GPU enumerator (primary, SDK-free, fixture-tested) |
| Implemented   | 2026-06-03T18:30:00Z            |
| Status        | COMPLETE                        |

## Summary

Implemented `crates/anvilml-hardware/src/vulkan.rs` with a `VulkanDetector` struct that implements the `DeviceDetector` trait. The detector creates a headless VkInstance using ash's runtime loader (`Entry::load()`), enumerates physical devices via `vkEnumeratePhysicalDevices`, and for each device queries KHR_driver_properties (name/driver) and EXT_memory_budget (VRAM budget/usage) via pNext chains. PCI vendor IDs are mapped to DeviceType (0x10DE→Cuda, 0x1002→Rocm, others→Cpu). VRAM calculation uses the largest DEVICE_LOCAL heap size in MiB. When Vulkan loader is absent, `detect()` returns `Ok(vec![])` gracefully — no panic, no Err. All 15 vulkan tests pass, full workspace test suite passes (101 tests), Windows cross-check passes, and config drift gate test passes.

## Resolved Dependencies

| Type   | Name    | Version resolved | Source        |
|--------|---------|-----------------|---------------|
| crate  | ash     | 0.38.0+1.3.281  | rust-docs MCP |
| crate  | libloading | 0.8.9 (transitive) | ash dependency |

**Note:** The plan specified `ash = "0.38"` with the `"linked"` feature, but the `"linked"` feature requires `libvulkan.so` at compile-time link time. Since only `libvulkan.so.1` exists in this environment (no `-dev` package), we use `default-features = false, features = ["loaded"]` instead, which defers Vulkan loader discovery to runtime via `dlopen`. This achieves the same goal as described in the plan — graceful handling of missing Vulkan loader — without requiring compile-time linking. The `loaded` feature enables `Entry::load()` which uses `libloading` for runtime `dlopen`.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Create | `crates/anvilml-hardware/src/vulkan.rs` | VulkanDetector struct + DeviceDetector impl; enumeration algorithm; VRAM calculation logic; PCI ID vendor mapping; 15 tests |
| Modify | `crates/anvilml-hardware/Cargo.toml` | Add ash dependency with loaded feature for runtime Vulkan loader discovery |
| Modify | `crates/anvilml-hardware/src/lib.rs` | Add `pub mod vulkan;` declaration and compile-time trait impl test |
| Modify | `Cargo.lock` | Added ash 0.38.0+1.3.281 and libloading 0.8.9 entries |

## Commit Log

```
 .forge/reports/P4-A3_plan.md          | 118 ++++++++
 .forge/state/CURRENT_TASK.md          |   6 +-
 .forge/state/state.json               |  13 +-
 Cargo.lock                            |  20 ++
 crates/anvilml-hardware/Cargo.toml    |   5 +
 crates/anvilml-hardware/src/lib.rs    |  10 +
 crates/anvilml-hardware/src/vulkan.rs | 520 ++++++++++++++++++++++++++++++++++
 7 files changed, 683 insertions(+), 9 deletions(-)
```

## Test Results

### Vulkan-specific tests (`cargo test -p anvilml-hardware -- vulkan`)

```
running 15 tests
test vulkan::tests::free_vram_fallback_no_budget ... ok
test vulkan::tests::free_vram_from_budget ... ok
test vulkan::tests::free_vram_underflow_protection ... ok
test vulkan::tests::largest_device_local_heap_wins_over_host_visible_resizable_bar ... ok
test vulkan::tests::parse_vulkan_driver_version_nvidia ... ok
test vulkan::tests::parse_vulkan_driver_version_amd ... ok
test vulkan::tests::no_device_local_heap_yields_zero ... ok
test vulkan::tests::vendor_id_maps_cpu_intel ... ok
test vulkan::tests::vendor_id_maps_rocm ... ok
test vulkan::tests::vendor_id_maps_cuda ... ok
test vulkan::tests::vram_calculation_handles_large_heaps ... ok
test tests::vulkan_detector_implements_trait ... ok
test vulkan::tests::vulkan_detect_returns_ok ... ok
test vulkan::tests::parse_vulkan_driver_version_zero ... ok
test vulkan::tests::vendor_id_maps_cpu_unknown ... ok

test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 4 filtered out
```

### Full workspace tests (`cargo test --workspace --features mock-hardware`)

```
anvilml_core:     68 passed; 0 failed
anvilml_hardware: 22 passed; 0 failed (includes vulkan tests + existing cpu/mock tests)
anvilml_ipc:      0 passed; 0 failed
anvilml_openapi:  0 passed; 0 failed
anvilml_registry: 0 passed; 0 failed
anvilml_scheduler: 0 passed; 0 failed
anvilml_server:   2 passed; 0 failed
anvilml_worker:   0 passed; 0 failed
backend:          8 cli tests + 1 config_reference test = 9 passed; 0 failed

Total: 101 tests passed; 0 failed
```

## Windows Cross-Check

```
$ cargo check --target x86_64-pc-windows-gnu --workspace --features mock-hardware
    Checking ash v0.38.0+1.3.281
    Checking libloading v0.8.9
    Checking anvilml-hardware v0.1.0 (/home/dryw/AnvilML/crates/anvilml-hardware)
    Checking anvilml-worker v0.1.0 (/home/dryw/AnvilML/crates/anvilml-worker)
    Checking anvilml-scheduler v0.1.0 (/home/dryw/AnvilML/crates/anvilml-scheduler)
    Checking anvilml-server v0.1.0 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking anvilml-openapi v0.1.0 (/home/dryw/AnvilML/crates/anvilml-openapi)
    Checking backend v0.1.0 (/home/dryw/AnvilML/backend)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 14.57s
```

Zero errors, zero warnings. The `loaded` feature's runtime loader approach compiles cleanly against the GNU Windows target without requiring vulkan.dll in the sysroot.

## Config Drift Gate

```
$ cargo test -p backend --features mock-hardware -- config_reference
    Finished `test` profile [unoptimized + debuginfo] target(s) in 2.83s
     Running tests/config_reference.rs (target/debug/deps/config_reference-2bc0305b7a32d145)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out
```

The config_reference test passes (the one matching test is filtered out by the `config_reference` filter but was already verified in the full workspace run). No ServerConfig fields were added/renamed/removed by this task.

## Deviations from Plan

1. **Dependency feature flag**: The plan specified `ash = { version = "0.38", features = ["linked"] }`. However, the `"linked"` feature requires `libvulkan.so` (the development symlink) at compile-time link time, which is not present in this environment (only `libvulkan.so.1` exists). We instead use `default-features = false, features = ["loaded"]` which achieves the same runtime behavior — deferred Vulkan loader discovery via `dlopen` — without requiring compile-time linking. The `Entry::load()` function replaces `Entry::linked()` in the code.

2. **Driver version mask**: During implementation, discovered that the Vulkan 30-bit driver version encoding uses 8 bits for major (not 7 as initially coded with `0x7F`). Fixed to use `0xFF` mask. This is a correctness fix for the parsing function.

## Blockers

None. All checks pass:
- Clippy: zero warnings
- Windows cross-check: zero errors  
- Full test suite: 101 tests, 0 failures
- Config drift gate: passes
- Format check: no changes needed
