# Plan Report: P12-A4

| Field       | Value                                                       |
|-------------|-------------------------------------------------------------|
| Task ID     | P12-A4                                                      |
| Phase       | 12 — Graph Validation                                       |
| Description | anvilml-scheduler: validate_graph node-type + edge checks (3-4) |
| Depends on  | P12-A3                                                      |
| Project     | anvilml                                                     |
| Planned at  | 2026-07-06T18:00:00Z                                        |
| Attempt     | 1                                                           |

## Objective

Extend `validate_graph()` in `crates/anvilml-scheduler/src/dag.rs` with validation checks 3 and 4 of the six-check pipeline defined in ANVILML_DESIGN.md §12.3: (3) every node's `type` must exist in the `NodeTypeRegistry`, pushing `UnknownNodeType` per violation; (4) every edge must reference a node that exists AND declares the referenced output slot, pushing `DanglingEdge` per violation. Both checks follow collect-all-errors semantics. This receives the scope P12-A3 explicitly deferred. The acceptance requirement is >=6 new tests in `dag_tests.rs`, bringing the file to >=11 tests total, with `cargo test -p anvilml-scheduler --test dag_tests` exiting 0.

## Scope

### In Scope
- Implement check 3 in `validate_graph()`: iterate all nodes, query `registry.get(type_name)` for each node that has a `"type"` field; push `GraphError::UnknownNodeType { node_id, type_name }` for each node whose type is not registered.
- Implement check 4 in `validate_graph()`: iterate all edges, resolve the source node by `"from"` field, verify the node exists in the nodes array AND that the referenced output slot name is declared in the node type's `NodeTypeDescriptor.outputs`; push `GraphError::DanglingEdge { node_id, slot_name }` per violation.
- Write >=6 new integration tests in `crates/anvilml-scheduler/tests/dag_tests.rs` covering: unknown node type reported with correct node_id, edge to nonexistent node reported, edge to undeclared output slot reported, valid type+edges pass cleanly, multiple violations across both checks collected together.
- Update the module-level doc comment in `dag.rs` to reflect that checks 1–4 are now implemented (checks 5–6 deferred).

### Out of Scope
None. This task's `defers_to` is `[]` (absent). No scope is deferred. Checks 5 (slot-type compatibility) and 6 (cycle detection) belong to P12-A5 and P12-A6 respectively.

## Existing Codebase Assessment

**What exists:** The `anvilml-scheduler` crate has a partial `validate_graph()` in `dag.rs` (89 lines) implementing checks 1–2: the structural root check (non-object → `NotAnObject`, missing `"nodes"` → `MissingNodesArray`) and duplicate-ID detection (collect-all-errors via `HashSet`). The `GraphError` enum in `types.rs` already defines all 7 variants including `UnknownNodeType { node_id, type_name }` and `DanglingEdge { node_id, slot_name }` with correct `#[error(...)]` Display attributes. The `NodeTypeRegistry` from `anvilml-core` has a synchronous `get(&self, type_name: &str) -> Option<NodeTypeDescriptor>` method. The `NodeTypeDescriptor` struct has `outputs: Vec<SlotDescriptor>` where each `SlotDescriptor` has a `name: String` field. The function signature already accepts `registry: &NodeTypeRegistry` (currently `_registry` — unused).

**Established patterns:** Tests in `dag_tests.rs` use `NodeTypeRegistry::new()` with `.register_all(...)` to populate known types. Tests construct graphs via `serde_json::json!()` macros. Errors are asserted via `matches!()` or by checking error count and variant type. The collect-all-errors pattern means errors are accumulated in a `Vec<GraphError>` and only returned if non-empty at the end of the function.

**Gap between design doc and source:** The design doc §12.3 specifies that check 4 requires verifying both node existence AND slot declaration. The current codebase has no existing check that consults edges (no edge data is parsed at all in checks 1–2). The `"type"` field on nodes is not extracted in checks 1–2 (the `continue` path for nodes without a valid `"id"` field implicitly skips type checking, which is correct for check 2 but will need to be handled for check 3).

## Resolved Dependencies

| Type   | Name        | Version verified | MCP source | Feature flags confirmed |
|--------|-------------|-----------------|------------|------------------------|
| crate  | thiserror   | 2.0.18          | Cargo.toml | n/a                    |
| crate  | serde_json  | 1               | Cargo.toml | n/a                    |
| crate  | anvilml-core| 0.1.22          | Cargo.toml | n/a (workspace path dep) |

No new external dependencies are introduced. All types (`NodeTypeRegistry::get()`, `NodeTypeDescriptor`, `SlotDescriptor`, `GraphError` variants) are already defined in the workspace and verified present in the source files read during inspection.

## Approach

**Step 1 — Implement check 3 (unknown node type validation) in `dag.rs`.**

After the existing check 2 loop (lines 65–82), add a second loop over the `nodes` array that:
- Extracts the `"id"` field as `node_id` (same pattern as check 2: `match node.get("id") { Some(Value::String(id)) => id.clone(), _ => continue }`).
- Extracts the `"type"` field as `type_name` (same pattern: `match node.get("type") { Some(Value::String(t)) => t.clone(), _ => continue }`). If a node has an `"id"` but no `"type"` field, skip it — this is a data quality issue that will be caught by a future check (or is acceptable as a no-op).
- Queries `registry.get(&type_name)`. If `None`, push `GraphError::UnknownNodeType { node_id: node_id.clone(), type_name: type_name.clone() }` into the `errors` vector.
- If the node has no `"id"` field, skip it (cannot produce a meaningful `node_id` for the error).

This continues the collect-all-errors pattern: iterate all nodes, push every unknown type error, continue to the next node.

**Step 2 — Implement check 4 (dangling edge validation) in `dag.rs`.**

After check 3, add a new section that processes edges:
- Get the `"edges"` array from the root object. If the key is absent or the value is not an array, skip check 4 entirely (a graph without edges has no dangling edges — this is not an error).
- Build a lookup: collect all node IDs from the nodes array into a `HashSet<String>` (or reuse the `seen_ids` set from check 2 if it's still in scope; alternatively build a separate `HashSet` from the nodes array — the check 2 loop already populated `seen_ids`, so it can be reused).
- Iterate each edge in the edges array:
  - Parse the `"from"` field. This field is a string in the format `"node_id:slot_name"` (e.g. `"load_model_0:MODEL"`). Split on the first `:` to get `source_node_id` and `slot_name`. If the field is missing, not a string, or cannot be split into exactly two parts, skip this edge (malformed edge — no specific error variant for this; check 4 only covers edges that are structurally valid but reference invalid targets).
  - Check if `source_node_id` exists in the node ID set. If not, push `GraphError::DanglingEdge { node_id: source_node_id.clone(), slot_name: slot_name.clone() }`.
  - If the node exists, extract its `NodeTypeDescriptor` from the registry by looking up its `"type"` field. If the node has no `"type"` field, skip it (cannot determine if the slot is declared without knowing the node type).
  - Once we have the `NodeTypeDescriptor`, check if any of its `outputs` has a `name` matching `slot_name`. If no match, push `GraphError::DanglingEdge { node_id: source_node_id.clone(), slot_name: slot_name.clone() }`.

**Step 3 — Update the module-level doc comment.**

Change the comment at the top of `dag.rs` from "This task covers checks 1–2" to "This module implements checks 1–4" and update the deferred list to "Checks 5–6 (slot-type compatibility, cycle detection) are added by subsequent tasks."

**Step 4 — Write >=6 new integration tests in `dag_tests.rs`.**

Test 1 (`test_validate_graph_unknown_node_type_reported`): Build a graph with a node whose `"type"` is `"NonExistentType"` not registered in the registry. Call `validate_graph()` with an empty registry (or one that does not contain `"NonExistentType"`). Assert `Err` with exactly one `UnknownNodeType` error containing the correct `node_id` and `type_name`.

Test 2 (`test_validate_graph_valid_type_passes_check3`): Build a graph with a node whose `"type"` is `"LoadModel"`, register `"LoadModel"` in the registry. Call `validate_graph()`. Assert `Ok(ValidatedGraph)` — no unknown type errors.

Test 3 (`test_validate_graph_edge_to_nonexistent_node`): Build a graph with one node `"a"` and an edge `"from": "nonexistent:output"`. Call `validate_graph()`. Assert `Err` with one `DanglingEdge` error for `"nonexistent"`.

Test 4 (`test_validate_graph_edge_to_undeclared_slot`): Register `"LoadModel"` in the registry. Build a graph with node `"a"` of type `"LoadModel"` and an edge `"from": "a:nonexistent_slot"`. Assert `Err` with one `DanglingEdge` error.

Test 5 (`test_validate_graph_valid_edges_pass_cleanly`): Register `"LoadModel"` with outputs `["MODEL"]`. Build a graph with node `"a"` of type `"LoadModel"` and an edge `"from": "a:MODEL"`. Assert `Ok(ValidatedGraph)`.

Test 6 (`test_validate_graph_multiple_violations_collected`): Build a graph with two nodes having unknown types and one edge referencing a nonexistent node. Assert `Err` with exactly 3 errors (two `UnknownNodeType` + one `DanglingEdge`).

**Step 5 — Verify the build and tests.**

Run `cargo test -p anvilml-scheduler --test dag_tests` and confirm >=16 tests total (15 existing + 6 new = 21, but the acceptance criterion is >=11).

## Public API Surface

No new public items are introduced. The function signature of `validate_graph()` does not change:

```rust
pub fn validate_graph(
    graph: Value,
    registry: &NodeTypeRegistry,
) -> Result<ValidatedGraph, Vec<GraphError>>
```

The existing `GraphError::UnknownNodeType` and `GraphError::DanglingEdge` variants are already defined in `types.rs` and re-exported in `lib.rs`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-scheduler/src/dag.rs` | Add checks 3–4 to `validate_graph()`; update module-level doc comment |
| Modify | `crates/anvilml-scheduler/tests/dag_tests.rs` | Add >=6 new integration tests for checks 3–4 |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-scheduler/tests/dag_tests.rs` | `test_validate_graph_unknown_node_type_reported` | Check 3: a node with an unregistered type produces `UnknownNodeType` with correct `node_id` and `type_name` | Registry does not contain `"NonExistentType"` | Graph: `{ "nodes": [{ "id": "n1", "type": "NonExistentType" }] }` | `Err([UnknownNodeType { node_id: "n1", type_name: "NonExistentType" }])` | `cargo test -p anvilml-scheduler --test dag_tests test_validate_graph_unknown_node_type_reported` exits 0 |
| `crates/anvilml-scheduler/tests/dag_tests.rs` | `test_validate_graph_valid_type_passes_check3` | Check 3: a node with a registered type passes cleanly | Registry contains `"LoadModel"` | Graph: `{ "nodes": [{ "id": "n1", "type": "LoadModel" }] }` | `Ok(ValidatedGraph)` | `cargo test -p anvilml-scheduler --test dag_tests test_validate_graph_valid_type_passes_check3` exits 0 |
| `crates/anvilml-scheduler/tests/dag_tests.rs` | `test_validate_graph_edge_to_nonexistent_node` | Check 4: an edge referencing a node that does not exist produces `DanglingEdge` | Graph has node `"a"` but edge references `"nonexistent"` | Graph: `{ "nodes": [{ "id": "a" }], "edges": [{ "from": "nonexistent:output" }] }` | `Err([DanglingEdge { node_id: "nonexistent", slot_name: "output" }])` | `cargo test -p anvilml-scheduler --test dag_tests test_validate_graph_edge_to_nonexistent_node` exits 0 |
| `crates/anvilml-scheduler/tests/dag_tests.rs` | `test_validate_graph_edge_to_undeclared_slot` | Check 4: an edge referencing a valid node but an undeclared output slot produces `DanglingEdge` | Registry has `"LoadModel"` with outputs `["MODEL"]` | Graph: `{ "nodes": [{ "id": "a", "type": "LoadModel" }], "edges": [{ "from": "a:nonexistent_slot" }] }` | `Err([DanglingEdge { node_id: "a", slot_name: "nonexistent_slot" }])` | `cargo test -p anvilml-scheduler --test dag_tests test_validate_graph_edge_to_undeclared_slot` exits 0 |
| `crates/anvilml-scheduler/tests/dag_tests.rs` | `test_validate_graph_valid_edges_pass_cleanly` | Check 4: a graph with valid edges and registered types passes cleanly | Registry has `"LoadModel"` with outputs `["MODEL"]` | Graph: `{ "nodes": [{ "id": "a", "type": "LoadModel" }], "edges": [{ "from": "a:MODEL" }] }` | `Ok(ValidatedGraph)` | `cargo test -p anvilml-scheduler --test dag_tests test_validate_graph_valid_edges_pass_cleanly` exits 0 |
| `crates/anvilml-scheduler/tests/dag_tests.rs` | `test_validate_graph_multiple_violations_collected` | Both checks 3–4: multiple violations across unknown types and dangling edges are all collected in one `Err` | Registry does not contain `"Foo"` or `"Bar"` | Graph: `{ "nodes": [{ "id": "n1", "type": "Foo" }, { "id": "n2", "type": "Bar" }], "edges": [{ "from": "nonexistent:out" }] }` | `Err([UnknownNodeType { n1, Foo }, UnknownNodeType { n2, Bar }, DanglingEdge { nonexistent, out }])` | `cargo test -p anvilml-scheduler --test dag_tests test_validate_graph_multiple_violations_collected` exits 0 |

## CI Impact

No CI changes required. The existing CI job `rust-linux` runs `cargo test --workspace --features mock-hardware`, which includes `anvilml-scheduler`. The new tests are integration tests in the crate's `tests/` directory and are automatically picked up by the existing test command. No new CI jobs, gates, or file patterns are introduced.

## Platform Considerations

None identified. The validation logic operates entirely on `serde_json::Value` data structures and the `NodeTypeRegistry` — both are platform-neutral. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are needed. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Edge `"from"` field format is not `"node_id:slot_name"` — the actual JSON schema may use separate `"source"` and `"output"` fields, or a different delimiter. | Medium | High | Check the graph data format used by the Python executor (`executor.py`) before writing check 4. If the format differs, adjust the parsing logic accordingly. The task context says `"from"` references an output slot, so the colon-delimited format is the most natural interpretation. |
| Nodes without a `"type"` field are silently skipped by check 3, potentially allowing graphs with missing type annotations to pass validation. | Low | Medium | The design doc §12.3 check 3 says "every node's type exists in registry" — a node without a type field has no type to check, so skipping it is correct. If this is later deemed a bug, it would be caught by check 5 or a future structural check, not by check 3. |
| Check 4's edge-to-slot verification requires looking up the node type in the registry, but the node may not have a `"type"` field, making the lookup impossible. | Low | Medium | If a node referenced by an edge has no `"type"` field, skip the slot check for that edge — the error would be an `UnknownNodeType` from check 3 (if the node was processed) or simply unresolvable. The edge itself is not flagged as dangling since we cannot determine what slots it declares. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-scheduler --test dag_tests test_validate_graph_unknown_node_type_reported` exits 0
- [ ] `cargo test -p anvilml-scheduler --test dag_tests test_validate_graph_valid_type_passes_check3` exits 0
- [ ] `cargo test -p anvilml-scheduler --test dag_tests test_validate_graph_edge_to_nonexistent_node` exits 0
- [ ] `cargo test -p anvilml-scheduler --test dag_tests test_validate_graph_edge_to_undeclared_slot` exits 0
- [ ] `cargo test -p anvilml-scheduler --test dag_tests test_validate_graph_valid_edges_pass_cleanly` exits 0
- [ ] `cargo test -p anvilml-scheduler --test dag_tests test_validate_graph_multiple_violations_collected` exits 0
- [ ] `cargo test -p anvilml-scheduler --test dag_tests` exits 0 with >=11 tests total
