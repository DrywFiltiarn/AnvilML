# Implementation Report: P7-B1

| Field         | Value                                       |
|---------------|---------------------------------------------|
| Task ID       | P7-B1                                       |
| Phase         | 007 — IPC Foundations                       |
| Description   | anvilml-ipc: RouterTransport struct + bind() |
| Implemented   | 2026-06-30T22:15:00Z                        |
| Status        | COMPLETE                                    |

## Summary

Created the `RouterTransport` struct and its `bind()` constructor in `crates/anvilml-ipc/src/transport.rs`, implementing the exact split-lock ownership shape specified in `ANVILML_DESIGN.md §8.3`. The struct wraps a ZeroMQ ROUTER socket bound on `tcp://127.0.0.1:0` (OS-assigned port), splits the socket into independent `tokio::sync::Mutex`-protected send/recv halves at construction time, and exposes the assigned port. Added the `zeromq` crate dependency (v0.6.0), updated `lib.rs` with module declaration and re-export, and added three integration tests verifying bind success, port uniqueness, and TCP listenability.

## Resolved Dependencies

| Type   | Name   | Version resolved | Source         |
|--------|--------|-----------------|----------------|
| crate  | zeromq | 0.6.0           | rust-docs MCP  |

The zeromq 0.6.0 crate (released 2026-05-04, MSRV 1.85.0) was verified via rust-docs MCP. The plan specified features `tokio-runtime` and `all-transport` with default features disabled — these were confirmed to exist in the feature table. The `endpoint` and `router` modules are private in the crate, but `Endpoint`, `RouterSocket`, `RouterSendHalf`, and `RouterRecvHalf` are publicly re-exported at the crate root level.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-ipc/src/transport.rs` | `RouterTransport` struct with `sender`, `receiver`, and `port` fields; `bind()` async constructor |
| MODIFY | `crates/anvilml-ipc/Cargo.toml` | Added `zeromq = { version = "0.6.0", default-features = false, features = ["tokio-runtime", "all-transport"] }`; bumped patch version 0.1.5 → 0.1.6 |
| MODIFY | `crates/anvilml-ipc/src/lib.rs` | Added `pub mod transport;` and `pub use transport::RouterTransport;` |
| MODIFY | `crates/anvilml-ipc/tests/roundtrip_tests.rs` | Added 3 new tests: `test_bind_returns_nonzero_port`, `test_two_binds_get_different_ports`, `test_bind_port_is_listening` |
| MODIFY | `docs/TESTS.md` | Added 3 test catalogue entries for the new tests |

## Commit Log

```
 .forge/reports/P7-B1_plan.md                | 195 ++++++++++++++++
 .forge/state/CURRENT_TASK.md                |   6 +-
 .forge/state/state.json                     |  13 +-
 Cargo.lock                                  | 333 ++++++++++++++++++++++++++--
 crates/anvilml-ipc/Cargo.toml               |   3 +-
 crates/anvilml-ipc/src/lib.rs               |   2 +
 crates/anvilml-ipc/src/transport.rs         | 110 +++++++++
 crates/anvilml-ipc/tests/roundtrip_tests.rs |  86 +++++++
 docs/TESTS.md                               |  36 +++
 9 files changed, 757 insertions(+), 27 deletions(-)
```

## Test Results

```
     Running tests/roundtrip_tests.rs (target/debug/deps/roundtrip_tests-0926a3ce655dbf1b)

running 21 tests
test test_cancel_job_roundtrip ... ok
test test_cancelled_roundtrip ... ok
test test_dying_roundtrip ... ok
test test_completed_roundtrip ... ok
test test_execute_roundtrip ... ok
test test_failed_roundtrip ... ok
test test_image_ready_roundtrip ... ok
test test_memory_query_roundtrip ... ok
test test_memory_report_roundtrip ... ok
test test_ping_roundtrip ... ok
test test_pong_roundtrip ... ok
test test_progress_roundtrip ... ok
test test_publish_multiple_subscribers_independent_copies ... ok
test test_publish_one_subscriber_delivers ... ok
test test_publish_zero_subscribers ... ok
test test_ready_roundtrip ... ok
test test_shutdown_roundtrip ... ok
test test_bind_returns_nonzero_port ... ok
test test_two_binds_get_different_ports ... ok
test test_bind_port_is_listening ... ok
test test_subscribe_returns_valid_receiver ... ok

test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s
```

All 21 tests pass (18 pre-existing + 3 new). The full workspace test suite also passes with zero failures.

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux:
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.80s

# 2. Mock-hardware Windows:
Finished `dev` profile [unoptimized + debuginfo] target(s) in 29.05s

# 3. Real-hardware Linux:
Finished `dev` profile [unoptimized + debuginfo] target(s) in 24.53s

# 4. Real-hardware Windows:
Finished `dev` profile [unoptimized + debuginfo] target(s) in 22.09s
```

All four platform cross-checks exit 0.

## Project Gates

```
# Gate 1 — Config Surface Sync:
test tests::config_reference_matches_defaults ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Gate 1 passes. No other gates are triggered by this task (no ServerConfig field changes, no handler signature changes, no node type changes).

## Public API Delta

```
+pub mod transport;
+pub use transport::RouterTransport;
```

Two new public items introduced, matching the plan's Public API Surface table:
- `pub mod transport;` — module declaration in `crates/anvilml-ipc/src/lib.rs`
- `pub use transport::RouterTransport;` — re-export in `crates/anvilml-ipc/src/lib.rs`

The `RouterTransport` struct itself is `pub` but its `sender` and `receiver` fields are private (not `pub`), matching the plan's specification.

## Deviations from Plan

- **Import path correction:** The plan specified `use zeromq::endpoint::Endpoint` and `use zeromq::router::{RouterSendHalf, RouterRecvHalf, RouterSocket}`. The zeromq 0.6.0 crate has private `endpoint` and `router` modules, but publicly re-exports `Endpoint`, `RouterSocket`, `RouterSendHalf`, and `RouterRecvHalf` at the crate root. Changed imports to `use zeromq::{Endpoint, RouterRecvHalf, RouterSendHalf, RouterSocket}`.
- **Test TCP connect API:** The plan mentioned `std::net::TcpStream::connect` for the listening test. `std::net::TcpStream::connect` is synchronous, so used `tokio::net::TcpStream::connect` instead to work correctly with `tokio::time::timeout`.

## Blockers

None.
