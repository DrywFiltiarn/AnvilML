# Plan Report: P7-A3

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P7-A3                                         |
| Phase       | 007 — IPC Foundations                         |
| Description | anvilml-ipc: WorkerEvent enum, Ready/Pong/Dying/MemoryReport |
| Depends on  | P7-A2                                         |
| Project     | anvilml                                       |
| Planned at  | 2026-06-30T18:30:00Z                          |
| Attempt     | 1                                             |

## Objective

Define the `WorkerEvent` enum in `crates/anvilml-ipc/src/messages.rs` with four startup-and-health variants (`Ready`, `Pong`, `Dying`, `MemoryReport`) per `ANVILML_DESIGN.md §8.6`. These variants carry no `job_id`, keeping this task's scope bounded. The enum is msgpack-serialisable via `rmp-serde` and will be tested with four roundtrip tests in `roundtrip_tests.rs`. Job-lifecycle variants are deferred to P7-A4.

## Scope

### In Scope
- Add `WorkerEvent` enum to `crates/anvilml-ipc/src/messages.rs` with four variants:
  - `Ready { worker_id, device_index, device_name, device_type, vram_total_mib, vram_free_mib, torch_version, fp16, bf16, fp8, flash_attention, capabilities_source, node_types }`
  - `Pong { seq: u64 }`
  - `Dying { reason: String }`
  - `MemoryReport { vram_used_mib: u32, ram_used_mib: u64 }`
- Derive `Debug`, `Clone`, `Serialize`, `Deserialize` on the enum
- Apply `#[serde(tag = "_type")]` attribute
- Import `NodeTypeDescriptor` from `anvilml_core::types::node` (already re-exported as `anvilml_core::NodeTypeDescriptor`)
- Add four msgpack roundtrip tests in `crates/anvilml-ipc/tests/roundtrip_tests.rs`, one per variant
- Ensure `cargo test -p anvilml-ipc --test roundtrip_tests` exits 0 with >=9 total tests

### Out of Scope
- Job-lifecycle variants (`Progress`, `ImageReady`, `Completed`, `Failed`, `Cancelled`) — deferred to P7-A4. P7-A4's description and context explicitly state it adds these five variants to complete `WorkerEvent`.
- Exporting `WorkerEvent` from `lib.rs` — handled by P7-A4.
- Any transport-layer send/recv logic — handled by P7-B1/P7-B2.

## Existing Codebase Assessment

The `anvilml-ipc` crate already has a working `WorkerMessage` enum in `messages.rs` (added by P7-A2), with five variants (`Ping`, `Shutdown`, `Execute`, `CancelJob`, `MemoryQuery`) using the same `#[serde(tag = "_type")]` pattern. The `rmp-serde` dependency is already declared in `Cargo.toml` under `[dev-dependencies]` at version `1.3.1`. The `roundtrip_tests.rs` file contains five `WorkerMessage` roundtrip tests and three `EventBroadcaster` tests (eight total).

`NodeTypeDescriptor` already exists in `anvilml-core` at `crates/anvilml-core/src/types/node.rs` with fields `type_name`, `display_name`, `category`, `description`, `inputs`, `outputs`, and is re-exported via `pub use types::*` in `anvilml-core/src/lib.rs`. It derives `Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema`.

The `lib.rs` currently re-exports `IpcError` and `EventBroadcaster` but not `WorkerEvent` — that export is P7-A4's scope. The established pattern in `messages.rs` is: module-level doc comment explaining the wire protocol, then the enum with `#[serde(tag = "_type")]`, each variant field documented with a `///` comment, and `use` imports at the top for external types.

## Resolved Dependencies

| Type   | Name     | Version verified | MCP source       | Feature flags confirmed |
|--------|----------|-----------------|------------------|------------------------|
| crate  | rmp-serde| 1.3.1           | rust-docs MCP    | n/a                    |

`rmp-serde` is already present in `Cargo.toml` `[dev-dependencies]` at version `1.3.1`. The MCP lookup confirmed this is the latest stable version (released 2025-12-23, 21.1M downloads). No version change needed. The API used (`rmp_serde::to_vec_named` / `rmp_serde::from_slice`) matches the existing `WorkerMessage` roundtrip tests, so no API shape verification was needed — these functions are already exercised in the codebase.

## Approach

1. **Add `WorkerEvent` enum to `messages.rs`.** After the existing `WorkerMessage` enum (ending at line 59 of `messages.rs`), append the `WorkerEvent` enum with:
   - `#[derive(Debug, Clone, Serialize, Deserialize)]` — matches the derive set on `WorkerMessage` and the design doc (§8.6).
   - `#[serde(tag = "_type")]` — identical to `WorkerMessage`, producing flat msgpack dicts with a `"_type"` discriminator key.
   - `Ready` variant with all 13 fields exactly as specified in §8.6:
     - `worker_id: String`
     - `device_index: u32`
     - `device_name: String`
     - `device_type: String` (comment: `"cuda" | "rocm" | "cpu"`)
     - `vram_total_mib: u32`
     - `vram_free_mib: u32`
     - `torch_version: String`
     - `fp16: bool`
     - `bf16: bool`
     - `fp8: bool`
     - `flash_attention: bool`
     - `capabilities_source: String` (comment: `"pytorch" (real mode) | "mock" (mock mode)`)
     - `node_types: Vec<NodeTypeDescriptor>` — uses the type from `anvilml_core`, imported at the top of the file.
   - `Pong { seq: u64 }` — mirrors `WorkerMessage::Ping { seq }`.
   - `Dying { reason: String }` — simple reason string.
   - `MemoryReport { vram_used_mib: u32, ram_used_mib: u64 }` — mirrors the fields sent in response to `MemoryQuery`.
   - Each variant field gets a `///` doc comment following the existing pattern in `messages.rs`.
   - `node_types` field on `Ready` uses `anvilml_core::NodeTypeDescriptor` which is already `Serialize + Deserialize`, so no additional derives are needed.

2. **Add four roundtrip tests to `roundtrip_tests.rs`.** Append a new section after the existing `WorkerMessage` tests (line 177), following the same pattern:
   - Import `anvilml_ipc::messages::WorkerEvent` and `anvilml_core::NodeTypeDescriptor`.
   - `test_ready_roundtrip`: Construct a `Ready` event with representative values for all 13 fields including a non-empty `node_types` vec. Serialize with `rmp_serde::to_vec_named`, deserialize with `rmp_serde::from_slice`, assert equality.
   - `test_pong_roundtrip`: Construct `Pong { seq: 42 }`, roundtrip, assert equality.
   - `test_dying_roundtrip`: Construct `Dying { reason: "OOM" }`, roundtrip, assert equality.
   - `test_memory_report_roundtrip`: Construct `MemoryReport { vram_used_mib: 4096, ram_used_mib: 8589934592 }`, roundtrip, assert equality.
   - Each test follows the existing naming convention (`test_<variant>_roundtrip`) and uses the `#[test]` attribute (no async needed).

3. **Verify the file compiles and tests pass.** Run `cargo test -p anvilml-ipc --test roundtrip_tests` to confirm >=9 tests total exit 0. The file will have 8 existing tests + 4 new = 12 total, exceeding the >=9 requirement.

No changes to `lib.rs` or `Cargo.toml` are needed — `rmp-serde` is already a dev-dependency, and `WorkerEvent` export is P7-A4's scope.

## Public API Surface

| Item | Crate/Module Path | Signature |
|------|-------------------|-----------|
| `WorkerEvent` enum | `anvilml_ipc::messages::WorkerEvent` | `pub enum WorkerEvent { Ready { ... }, Pong { seq: u64 }, Dying { reason: String }, MemoryReport { vram_used_mib: u32, ram_used_mib: u64 } }` |

No new `pub use` items or re-exports — `WorkerEvent` is already `pub` within `messages.rs` and will be re-exported from `lib.rs` by P7-A4.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-ipc/src/messages.rs` | Add `WorkerEvent` enum after `WorkerMessage` |
| Modify | `crates/anvilml-ipc/tests/roundtrip_tests.rs` | Add 4 new `WorkerEvent` roundtrip tests |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-ipc/tests/roundtrip_tests.rs` | `test_ready_roundtrip` | `WorkerEvent::Ready` with all 13 fields roundtrips via msgpack | None | Ready event with worker_id="gpu:0", device_index=0, device_name="NVIDIA RTX 4090", device_type="cuda", vram_total_mib=24576, vram_free_mib=20480, torch_version="2.5.1+cu124", fp16=true, bf16=true, fp8=true, flash_attention=true, capabilities_source="pytorch", node_types=[two NodeTypeDescriptor entries] | Deserialized Ready equals original | `cargo test -p anvilml-ipc --test roundtrip_tests -- test_ready_roundtrip` exits 0 |
| `crates/anvilml-ipc/tests/roundtrip_tests.rs` | `test_pong_roundtrip` | `WorkerEvent::Pong { seq }` roundtrips via msgpack | None | Pong { seq: 42 } | Deserialized Pong equals original | `cargo test -p anvilml-ipc --test roundtrip_tests -- test_pong_roundtrip` exits 0 |
| `crates/anvilml-ipc/tests/roundtrip_tests.rs` | `test_dying_roundtrip` | `WorkerEvent::Dying { reason }` roundtrips via msgpack | None | Dying { reason: "OOM" } | Deserialized Dying equals original | `cargo test -p anvilml-ipc --test roundtrip_tests -- test_dying_roundtrip` exits 0 |
| `crates/anvilml-ipc/tests/roundtrip_tests.rs` | `test_memory_report_roundtrip` | `WorkerEvent::MemoryReport { vram_used_mib, ram_used_mib }` roundtrips via msgpack | None | MemoryReport { vram_used_mib: 4096, ram_used_mib: 8589934592 } | Deserialized MemoryReport equals original | `cargo test -p anvilml-ipc --test roundtrip_tests -- test_memory_report_roundtrip` exits 0 |

## CI Impact

No CI changes required. The tests are integration tests in the existing `roundtrip_tests.rs` file, which is already picked up by `cargo test --workspace --features mock-hardware` (the CI Rust test job). No new file types, gates, or test modules are introduced.

## Platform Considerations

None identified. The `WorkerEvent` enum is a pure data type with no platform-specific fields, `#[cfg(...)]` guards, or I/O. The msgpack serialisation via `rmp-serde` is platform-neutral. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `NodeTypeDescriptor` field ordering in the `Ready` variant affects msgpack serialisation order, which could cause deserialisation mismatch if the Python worker sends fields in a different order. | Low | Medium | `rmp_serde::to_vec_named` uses serde's named-map serialisation which is order-independent for deserialisation. This is the same mechanism already proven working by the `WorkerMessage::Execute` roundtrip test (which also contains a `Vec<NodeTypeDescriptor>` indirectly via the graph). No additional mitigation needed. |
| `Vec<NodeTypeDescriptor>` in `Ready` has many fields; a typo in a field name or type will cause a compile error (caught immediately) but a wrong field count could produce a misleading msgpack structure. | Low | Low | The MCP-verified `NodeTypeDescriptor` type from `anvilml-core` is used directly — no manual field mapping. The roundtrip test constructs real `NodeTypeDescriptor` values, so any type mismatch is caught at compile time. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-ipc --test roundtrip_tests` exits 0 (>=9 tests total in file)
- [ ] `cargo test -p anvilml-ipc --test roundtrip_tests -- test_ready_roundtrip` exits 0
- [ ] `cargo test -p anvilml-ipc --test roundtrip_tests -- test_pong_roundtrip` exits 0
- [ ] `cargo test -p anvilml-ipc --test roundtrip_tests -- test_dying_roundtrip` exits 0
- [ ] `cargo test -p anvilml-ipc --test roundtrip_tests -- test_memory_report_roundtrip` exits 0
