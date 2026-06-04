# Implementation Report: P7-A4

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P7-A4                                         |
| Phase       | 007 — WebSocket Event Stream                  |
| Description | anvilml-server: system.stats tick task (5s broadcast) |
| Implemented | 2026-06-04T21:00:00Z                          |
| Status      | COMPLETE                                      |

## Summary

Created a background tokio task in `anvilml-server` that fires every 5 seconds, reads the latest hardware state from `AppState` (per-device VRAM computed as total minus free), and host RAM via the `sysinfo` crate. Each tick builds a `SystemStatsEvent` and broadcasts it as `WsEvent::SystemStats` through the existing `EventBroadcaster`. The task is exposed as `spawn_system_stats_tick(state: AppState) -> JoinHandle<()>` in a new module `src/ws/stats_tick.rs`. Includes a unit test that constructs mock hardware, spawns the tick, awaits one interval, and asserts the broadcaster receives an event with correct GPU count and VRAM values.

## Resolved Dependencies

| Type   | Name    | Version resolved | Source              |
|--------|---------|-----------------|---------------------|
| crate  | sysinfo | 0.32            | rust-docs MCP (matched existing anvilml-hardware version) |
| crate  | chrono  | 0.4             | rust-docs MCP (promoted from dev-deps to regular deps for `Utc::now()` usage in main code) |

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Create | `crates/anvilml-server/src/ws/stats_tick.rs` | New module: `spawn_system_stats_tick` function with 5s interval loop and unit test |
| Modify | `crates/anvilml-server/Cargo.toml` | Added `chrono = "0.4"` (regular dep) and `sysinfo = "0.32"` dependencies |
| Modify | `crates/anvilml-server/src/ws/mod.rs` | Added `pub mod stats_tick;` submodule declaration |
| (auto) | `Cargo.lock` | Updated with new dependency entries |

Note: `GpuDevice` exposes `vram_free_mib` (not `vram_used_mib` as the plan described). The implementation computes `vram_used_mib = vram_total_mib - vram_free_mib` using `saturating_sub` to prevent underflow.

## Commit Log

```
 .forge/reports/P7-A4_plan.md               |  92 ++++++++++++++++
 .forge/state/CURRENT_TASK.md               |   6 +-
 .forge/state/state.json                    |  13 ++-
 Cargo.lock                                 |   1 +
 crates/anvilml-server/Cargo.toml           |   2 +
 crates/anvilml-server/src/ws/mod.rs        |   1 +
 crates/anvilml-server/src/ws/stats_tick.rs | 171 +++++++++++++++++++++++++++++
 7 files changed, 277 insertions(+), 9 deletions(-)
```

## Test Results

```
     Running unittests src/lib.rs (target/debug/deps_anvilml_server-d9f7c9ca32a9308c)

running 8 tests
test tests::rescan_returns_202 ... ok
test tests::env_returns_200_with_stub_report ... ok
test ws::broadcaster::tests::send_no_subscribers_no_error ... ok
test tests::health_returns_200 ... ok
test ws::broadcaster::tests::subscribe_send_receive ... ok
test tests::get_model_returns_404_when_missing ... ok
test tests::system_returns_200_with_hardware_info ... ok
test ws::stats_tick::tests::stats_tick_broadcasts_event ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.00s
```

Full workspace test suite: 170 tests passed, 0 failed across all crates (anvilml-core, anvilml-hardware, anvilml-ipc, anvilml-openapi, anvilml-registry, anvilml-scheduler, anvilml-server, anvilml-worker, backend).

## Platform Cross-Check

```
# Check 1: Mock-hardware Windows-gnu cross-check
$ cargo check --target x86_64-pc-windows-gnu --workspace --features mock-hardware
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.53s

# Check 2: Real-hardware Linux native
$ cargo check --bin anvilml
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.15s

# Check 3: Real-hardware Windows-gnu cross-check
$ cargo check --bin anvilml --target x86_64-pc-windows-gnu
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.50s
```

All three checks exit 0.

## Project Gates

```
# Config drift gate
$ cargo test -p backend --features mock-hardware
running 8 tests
test cli::tests::test_args_to_overrides_all_none ... ok
test cli::tests::test_args_to_overrides_ipv6 ... ok
test cli::tests::test_args_to_overrides_port_edge ... ok
test cli::tests::test_args_to_overrides_with_values ... ok
test cli::tests::test_log_format_default_is_plain ... ok
test cli::tests::test_log_format_possible_values ... ok
test cli::tests::test_log_format_to_string ... ok
test cli::tests::test_log_format_value_enum_variants ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/config_reference.rs (target/debug/deps/config_reference-b5e7d85be9b94dc4)
running 1 test
test test_toml_key_set_matches_default ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Deviations from Plan

- **`GpuDevice.vram_used_mib` does not exist.** The plan stated that `GpuDevice` exposes `vram_used_mib`, but the actual type has `vram_free_mib` instead. Implementation computes `vram_used_mib = vram_total_mib - vram_free_mib` using `saturating_sub`.
- **`AppState.hardware` is private.** The plan suggested cloning `Arc<RwLock<HardwareInfo>>` from state. Since the field is private, the implementation uses `state.hardware()` which returns a cloned `HardwareInfo` snapshot each tick. This is actually better for correctness (fresh read each interval).
- **`chrono` promoted to regular dependency.** The plan listed `chrono::Utc` usage but chrono was only in dev-dependencies of anvilml-server. Promoted to regular dependency since the main code uses `Utc::now()`.
- **Removed unused import.** The initial draft imported `EventBroadcaster` at module scope (unused in lib code, only needed in tests). Moved the import into the `#[cfg(test)]` module to avoid clippy warning.

## Blockers

None. All checks pass, all tests pass, no MCP servers unavailable.
