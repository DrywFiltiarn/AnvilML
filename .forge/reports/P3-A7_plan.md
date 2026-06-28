# Plan Report: P3-A7

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P3-A7                                       |
| Phase       | 003 — Core Domain Types: Data Model         |
| Description | anvilml-core: NodeTypeDescriptor, SlotDescriptor, SlotType |
| Depends on  | P3-A6                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-06-28T19:15:00Z                        |
| Attempt     | 1                                           |

## Objective

Create the node-type descriptor types that the Python worker reports in its `Ready` event, and the `NodeTypeRegistry` (P3-A10) will store. `NodeTypeDescriptor` describes a node's shape (name, display name, category, description, typed input/output slots). `SlotDescriptor` describes a single slot's name, type, and optionality. `SlotType` is a closed enum of exactly eleven semantic slot kinds that the scheduler's graph validator will use at job-submission time to verify connected slots are type-compatible.

## Scope

### In Scope
- Create `crates/anvilml-core/src/types/node.rs` with `NodeTypeDescriptor`, `SlotDescriptor`, and `SlotType` types per ANVILML_DESIGN.md §5.6.
- Add `mod node;` and `pub use node::*;` to `crates/anvilml-core/src/types/mod.rs`.
- Create `crates/anvilml-core/tests/node_tests.rs` with >=4 tests covering construction with mixed required/optional slots and SCREAMING_SNAKE_CASE serde for all 11 `SlotType` variants.

### Out of Scope
None. `defers_to (from JSON): []` — this task implements its full scope with no deferrals.

## Existing Codebase Assessment

The `anvilml-core` crate already has seven type modules (`artifact`, `hardware`, `job`, `model`, `worker`) and their corresponding integration test files in `tests/`. The established pattern is:

- Each type module declares `#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]` on structs, with `Copy + PartialEq + Eq + Hash` on enums as required by the design doc.
- Each module uses `utoipa::ToSchema` (from the `utoipa` crate v5.5.0 already present with `macros` default feature).
- Integration tests in `crates/anvilml-core/tests/` import via `use anvilml_core::types::*;`, construct instances, serialise to JSON, deserialise back, assert equality, and verify JSON field names.
- `types/mod.rs` uses one `mod <name>;` declaration per line, followed by `pub use <name>::*;`.
- No `#[cfg(test)]` inline blocks are used — all tests live in `tests/` as separate test crates.

No source file `node.rs` exists yet. The `types/mod.rs` currently declares five submodules (artifact, hardware, job, model, worker) and their re-exports. The `utoipa` dependency is already present with features `["uuid", "chrono"]` and the `macros` default feature, which provides `ToSchema`.

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| crate  | utoipa  | 5.5.0           | rust-docs MCP  | macros (default)       |

The `utoipa` crate v5.5.0 is already declared in `crates/anvilml-core/Cargo.toml` with features `["uuid", "chrono"]`. The `macros` feature (which provides `#[derive(ToSchema)]`) is a default feature and is enabled automatically. No new dependency or feature flag addition is required.

## Approach

1. **Write `crates/anvilml-core/src/types/node.rs`.** Create the file with three public types per ANVILML_DESIGN.md §5.6:

   a. `pub enum SlotType` — the fixed, closed enum with exactly 11 variants: `Model`, `Clip`, `Vae`, `Conditioning`, `Latent`, `Image`, `String`, `Int`, `Float`, `Bool`, `Any`. Derive `Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema`. Apply `#[serde(rename_all = "SCREAMING_SNAKE_CASE")]` so that e.g. `SlotType::Model` serialises to `"MODEL"` and `SlotType::Any` to `"ANY"`. Add a doc comment on `Any` noting it disables type checking for that slot.

   b. `pub struct SlotDescriptor` — fields: `name: String`, `slot_type: SlotType`, `optional: bool`. Derive `Debug, Clone, Serialize, Deserialize, ToSchema`. Add a doc comment on `optional` explaining it enables omission in favor of a node-internal default.

   c. `pub struct NodeTypeDescriptor` — fields: `type_name: String`, `display_name: String`, `category: String`, `description: String`, `inputs: Vec<SlotDescriptor>`, `outputs: Vec<SlotDescriptor>`. Derive `Debug, Clone, Serialize, Deserialize, ToSchema`. Add a doc comment on `type_name` noting it is the unique identifier (e.g. "LoadModel").

   Rationale: The type definitions match the design doc §5.6 verbatim. No additional methods or impl blocks are needed — this is a pure-data module.

2. **Modify `crates/anvilml-core/src/types/mod.rs`.** Append one line to declare the submodule and one line to re-export:
   ```rust
   pub mod node;
   pub use node::*;
   ```
   Place these after the existing `worker` entries, maintaining alphabetical order (node comes after model, before worker).

3. **Create `crates/anvilml-core/tests/node_tests.rs`.** Write >=4 integration tests following the established pattern from `model_tests.rs` and `hardware_tests.rs`:

   a. `test_slot_type_screaming_snake_case_serde` — iterate over all 11 `SlotType` variants with their expected JSON strings (`"MODEL"`, `"CLIP"`, `"VAE"`, `"CONDITIONING"`, `"LATENT"`, `"IMAGE"`, `"STRING"`, `"INT"`, `"FLOAT"`, `"BOOL"`, `"ANY"`), serialise each, assert the JSON matches, deserialise back, assert equality. This is the primary test covering all 11 variants.

   b. `test_slot_descriptor_serde_roundtrip` — construct a `SlotDescriptor` with a required slot (`optional: false`) and an optional slot (`optional: true`), serialise both to JSON, deserialise back, assert equality, verify JSON field names (`name`, `slot_type`, `optional`).

   c. `test_node_type_descriptor_construction` — construct a `NodeTypeDescriptor` with mixed required/optional inputs and outputs (e.g. `LoadModel`-like: one required `model_id` input, one `MODEL` output), serialise to JSON, deserialise back, assert equality, verify the JSON contains `type_name`, `display_name`, `category`, `description`, `inputs` (array), `outputs` (array).

   d. `test_node_type_descriptor_empty_slots` — construct a `NodeTypeDescriptor` with empty `inputs` and `outputs` vectors, serialise to JSON, verify the JSON contains `"inputs": []` and `"outputs": []`, deserialise back, assert equality. This exercises the edge case of a node with no slots.

## Public API Surface

| Item | Path | Description |
|------|------|-------------|
| `enum SlotType` | `anvilml_core::types::SlotType` | 11-variant closed enum; derives Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema; serde SCREAMING_SNAKE_CASE |
| `struct SlotDescriptor` | `anvilml_core::types::SlotDescriptor` | Describes one slot; derives Debug, Clone, Serialize, Deserialize, ToSchema |
| `struct NodeTypeDescriptor` | `anvilml_core::types::NodeTypeDescriptor` | Describes a node type; derives Debug, Clone, Serialize, Deserialize, ToSchema |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-core/src/types/node.rs` | NodeTypeDescriptor, SlotDescriptor, SlotType types |
| MODIFY | `crates/anvilml-core/src/types/mod.rs` | Add `mod node;` and `pub use node::*;` |
| CREATE | `crates/anvilml-core/tests/node_tests.rs` | Integration tests for node types |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `crates/anvilml-core/tests/node_tests.rs` | `test_slot_type_screaming_snake_case_serde` | All 11 `SlotType` variants serialise to correct SCREAMING_SNAKE_CASE JSON strings and roundtrip | `cargo test -p anvilml-core --test node_tests` exits 0 |
| `crates/anvilml-core/tests/node_tests.rs` | `test_slot_descriptor_serde_roundtrip` | `SlotDescriptor` with required and optional slots serialises/deserialises correctly with correct JSON field names | `cargo test -p anvilml-core --test node_tests` exits 0 |
| `crates/anvilml-core/tests/node_tests.rs` | `test_node_type_descriptor_construction` | `NodeTypeDescriptor` with mixed required/optional inputs and outputs roundtrips correctly | `cargo test -p anvilml-core --test node_tests` exits 0 |
| `crates/anvilml-core/tests/node_tests.rs` | `test_node_type_descriptor_empty_slots` | `NodeTypeDescriptor` with empty input/output vectors serialises to `"inputs": []` / `"outputs": []` and roundtrips | `cargo test -p anvilml-core --test node_tests` exits 0 |

## CI Impact

No CI changes required. The new test file `crates/anvilml-core/tests/node_tests.rs` follows the established convention — it is automatically picked up by `cargo test --workspace --features mock-hardware` which already runs all integration tests in every crate's `tests/` directory.

## Platform Considerations

None identified. The types are pure data with no platform-specific code, no `#[cfg(unix)]`/`#[cfg(windows)]` guards, no filesystem or network I/O. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `utoipa::ToSchema` derive macro not available for the new types | Low | Medium | The `macros` feature is a default feature of utoipa 5.5.0 (confirmed via MCP). If it fails to compile, the ACT agent should verify `utoipa`'s `macros` feature is active by checking that `utoipa-gen` is in the dependency tree. |
| `SlotType` serde rename_all format does not match downstream expectations | Low | High | The design doc §5.6 specifies `SCREAMING_SNAKE_CASE` explicitly. The test `test_slot_type_screaming_snake_case_serde` asserts each variant's exact JSON string, catching any mismatch before staging. |
| `types/mod.rs` alphabetical ordering drift | Low | Low | The existing pattern is one `mod`/`pub use` pair per submodule. Adding `node` between `model` and `worker` maintains alphabetical order. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-core --test node_tests` exits 0
- [ ] `wc -l crates/anvilml-core/src/types/node.rs` reports > 0 (file exists and is non-empty)
- [ ] `grep -c "^pub " crates/anvilml-core/src/types/node.rs` reports 3 (exactly three pub items: NodeTypeDescriptor, SlotDescriptor, SlotType)
- [ ] `grep "^## " .forge/reports/P3-A7_plan.md` shows 12 section headings
