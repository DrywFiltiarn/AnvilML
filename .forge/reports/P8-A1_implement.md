# Implementation Report: P8-A1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P8-A1                                             |
| Phase       | 008 — IPC Framing                                 |
| Description | anvilml-ipc: WorkerMessage and WorkerEvent enums  |
| Implemented | 2026-06-05T20:30:00Z                              |
| Status      | COMPLETE                                          |

## Summary

Implemented the `WorkerMessage` (Rust → Python) and `WorkerEvent` (Python → Rust) enums for the `anvilml-ipc` crate as specified in ANVILML_DESIGN.md §7.2 and §7.3. Added `rmp-serde`, `serde`, `serde_json`, and `uuid` dependencies to the workspace and per-crate manifests. Created `messages.rs` with both enums, manual `PartialEq` implementations (since `JobSettings` lacks `PartialEq`), and 17 unit tests covering all 6 WorkerMessage and all 9 WorkerEvent variants for msgpack round-trip serialization/deserialization, plus discriminant uniqueness checks.

## Resolved Dependencies

| Type   | Name       | Version resolved | Source            |
|--------|-----------|-----------------|-------------------|
| crate  | rmp-serde | 1.3             | cargo search (MCP unavailable) |
| crate  | serde     | 1.0.228         | workspace (pre-existing) |
| crate  | serde_json| 1.0.150         | workspace (pre-existing) |
| crate  | uuid      | 1.23.2          | workspace (pre-existing) |

Note: The plan specified `rmp-serde = "0.21"` but the latest available version is `1.3.1`. Per FORGE_AGENT_RULES §6.2, ACT is authoritative over PLAN on version numbers. Version 1.3 was used. The `rust-docs` MCP server returned 404; `cargo search` and `cargo info` were used as fallback.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `/home/dryw/AnvilML/Cargo.toml` | Added `rmp-serde = "1.3"` to `[workspace.dependencies]` |
| Modify | `/home/dryw/AnvilML/crates/anvilml-ipc/Cargo.toml` | Added rmp-serde, serde, serde_json, uuid deps + dev-dependencies section |
| Create   | `/home/dryw/AnvilML/crates/anvilml-ipc/src/messages.rs` | WorkerMessage and WorkerEvent enums with PartialEq impls and 17 unit tests |
| Modify | `/home/dryw/AnvilML/crates/anvilml-ipc/src/lib.rs` | Replaced stub with `pub mod messages` and re-exports |

## Commit Log

```
 Cargo.lock                         |  23 ++
 Cargo.toml                         |   1 +
 crates/anvilml-ipc/Cargo.toml      |   6 +
 crates/anvilml-ipc/src/lib.rs      |   4 +-
 crates/anvilml-ipc/src/messages.rs | 590 +++++++++++++++++++++++++++++++++++++
 5 files changed, 741 insertions(+), 10 deletions(-)
```

## Test Results

```
running 17 tests
test messages::tests::all_worker_event_variants ... ok
test messages::tests::all_worker_message_variants ... ok
test messages::tests::worker_event_roundtrip_cancelled ... ok
test messages::tests::worker_event_roundtrip_completed ... ok
test messages::tests::worker_event_roundtrip_dying ... ok
test messages::tests::worker_event_roundtrip_failed ... ok
test messages::tests::worker_event_roundtrip_image_ready ... ok
test messages::tests::worker_event_roundtrip_memory_report ... ok
test messages::tests::worker_event_roundtrip_pong ... ok
test messages::tests::worker_event_roundtrip_progress ... ok
test messages::tests::worker_event_roundtrip_ready ... ok
test messages::tests::worker_message_roundtrip_cancel_job ... ok
test messages::tests::worker_message_roundtrip_execute ... ok
test messages::tests::worker_message_roundtrip_init_hardware ... ok
test messages::tests::worker_message_roundtrip_memory_query ... ok
test messages::tests::worker_message_roundtrip_ping ... ok
test messages::tests::worker_message_roundtrip_shutdown ... ok

test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Full workspace test suite: 195 tests passed, 0 failed across all crates.

## Platform Cross-Check

### Check 1: mock-hardware Windows-gnu cross-check
```
Checking rmp v0.8.15
Checking anvilml-hardware v0.1.0 (/home/dryw/AnvilML/crates/anvilml-hardware)
Checking rmp-serde v1.3.1
Checking anvilml-ipc v0.1.0 (/home/dryw/AnvilML/crates/anvilml-ipc)
...
Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.52s
```

### Check 2: real-hardware Linux native
```
Checking anvilml-ipc v0.1.0 (/home/dryw/AnvilML/crates/anvilml-ipc)
...
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.44s
```

### Check 3: real-hardware Windows-gnu cross-check
```
Checking anvilml-hardware v0.1.0 (/home/dryw/AnvilML/crates/anvilml-hardware)
...
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.99s
```

All three checks exited 0.

## Project Gates

### Gate 1 — Config Surface Sync
```
Running tests/config_reference.rs (target/debug/deps/config_reference-6d801f6b27446d25)
running 1 test
test test_toml_key_set_matches_default ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Deviations from Plan

1. **Dependency version**: Plan specified `rmp-serde = "0.21"`; resolved version is `1.3` (latest stable) via `cargo search`. MCP `rust-docs` server was unavailable (404). Per rule 6.2, ACT overrides PLAN on version numbers.

2. **PartialEq implementation**: Plan specified deriving `PartialEq` for both enums. However, `anvilml_core::JobSettings` does not implement `PartialEq`, making the derived impl impossible. Implemented `PartialEq` manually for both `WorkerMessage` and `WorkerEvent` instead.

3. **Import path**: Plan referenced `anvilml_core::JobSettings`; the type is actually at `anvilml_core::types::job::JobSettings` (not re-exported at crate root). Used the full path.

4. **serde_json dependency**: Added `serde_json` to anvilml-ipc's dependencies (not in plan's dependency list) since `WorkerMessage::Execute.graph` uses `serde_json::Value`.

5. **rmp_serde API**: Plan referenced `rmp_serde::from_slice`; rmp-serde 1.3 requires `'static` lifetime for `from_slice`. Used `rmp_serde::from_read` with `std::io::Cursor` instead in the test roundtrip function.

## Blockers

None.
