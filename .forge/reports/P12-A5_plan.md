# Plan Report: P12-A5

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P12-A5                                      |
| Phase       | 12 — Graph Validation                       |
| Description | anvilml-scheduler: validate_graph slot-type-compat check (5) |
| Depends on  | P12-A4                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-07-06T19:40:00Z                        |
| Attempt     | 1                                           |

## Objective

Extend `validate_graph()` in `crates/anvilml-scheduler/src/dag.rs` with check 5 of the six-check validation pipeline: verify that every edge's output slot type is compatible with its destination input slot type. Compatibility is an exact `SlotType` match on both sides, or `SlotType::Any` on either side. The check only runs on edges that passed check 4 (dangling-edge check), preventing double-reporting of the same edge. This completes the type-safety layer of the validator before cycle detection (P12-A6) closes the loop.

## Scope

### In Scope
- Add check 5 to `validate_graph()` in `crates/anvilml-scheduler/src/dag.rs`: for every edge that passed check 4, compare the source output slot type with the destination input slot type from the registry; push `SlotTypeMismatch` for incompatibilities.
- Add at least 5 new integration tests in `crates/anvilml-scheduler/tests/dag_tests.rs` covering: mismatched types reported correctly, exact-match types pass, `SlotType::Any` on either side passes, a dangling edge from check 4 is not double-reported, and multiple mismatches are collected.

### Out of Scope
None. This task's `defers_to` (from JSON): `[]` — no deferrals permitted.

## Existing Codebase Assessment

`validate_graph()` in `dag.rs` implements checks 1–4 in collect-all-errors mode. It builds an `id_to_type` map (line 126) and iterates edges (line 139) with `"from"` field parsing. The `GraphError::SlotTypeMismatch` variant already exists in `types.rs` with `node_id`, `slot_name`, `expected`, and `found` fields. `SlotType` enum with `Any` variant is defined in `anvilml-core/src/types/node.rs`. The `NodeTypeRegistry::get()` method returns `Option<NodeTypeDescriptor>`, and `NodeTypeDescriptor` has `inputs: Vec<SlotDescriptor>` and `outputs: Vec<SlotDescriptor>`, each with a `slot_type: SlotType` field.

The existing code uses `match` guards for field extraction, `continue` on malformed fields, `errors.push()` for collect-all-errors, and `if errors.is_empty() { Ok(...) } else { Err(errors) }` at the end. Tests construct registries with `NodeTypeRegistry::new()` + `registry.register_all()`, build graphs via `serde_json::json!()`, and assert on error counts and variant matching.

The design doc's example graph (Appendix B) uses inline edge references in node `inputs`, but `validate_graph()` uses a separate `edges` array with `"from": "node_id:slot_name"` format. The edge format for check 5 must also include a `"to": "node_id:slot_name"` field to specify the destination input.

## Resolved Dependencies

| Type   | Name       | Version verified | MCP source     | Feature flags confirmed |
|--------|------------|-----------------|----------------|------------------------|
| crate  | anvilml-core | 0.1.x (workspace path dep) | codebase inspection | n/a (path dependency) |

No new external dependencies are introduced by this task. All types (`SlotType`, `NodeTypeRegistry`, `NodeTypeDescriptor`, `SlotDescriptor`) are already available via the existing `anvilml-core` workspace dependency.

## Approach

1. **Add a helper to convert `SlotType` to its `SCREAMING_SNAKE_CASE` string.** `SlotType` does not derive `Display`; it derives `Debug` and uses `#[serde(rename_all = "SCREAMING_SNAKE_CASE")]`. Write a private function `fn slot_type_label(t: SlotType) -> String` in `dag.rs` that maps each variant to its uppercase string (e.g. `Model` → `"Model"`, `Any` → `"Any"`). This is used only for the error message fields `expected` and `found`.

2. **Track dangling-edge source node IDs from check 4.** After the check 4 loop (lines 139–200), collect all source node IDs that were reported as `DanglingEdge` into a `HashSet<String>`. This set is used by check 5 to skip edges already flagged.

3. **Add check 5 loop over edges.** After the check 4 block and before the final `if errors.is_empty()` guard:
   - Iterate all edges that have a `"to"` field.
   - Skip edges whose source node ID is in the dangling-edge set from step 2.
   - Parse `"from"` (source_node_id:source_slot_name) and `"to"` (dest_node_id:dest_slot_name).
   - Look up the source node's descriptor from `id_to_type`, find the matching output slot in `descriptor.outputs`.
   - Look up the destination node's descriptor from `id_to_type`, find the matching input slot in `descriptor.inputs`.
   - If either slot lookup fails, skip (already covered by check 3/4).
   - Compare `output_slot.slot_type` vs `input_slot.slot_type`: if unequal AND neither is `SlotType::Any`, push `SlotTypeMismatch { node_id: dest_node_id, slot_name: dest_slot_name, expected: slot_type_label(input_slot.slot_type), found: slot_type_label(output_slot.slot_type) }`.

4. **Update module doc comment** in `dag.rs` to reflect checks 1–5 are implemented (check 6 deferred to P12-A6).

5. **Write integration tests** in `crates/anvilml-scheduler/tests/dag_tests.rs` (see Tests section below).

## Public API Surface

No new public items are introduced. The only change is to the existing `validate_graph()` function's behavior. A private helper `slot_type_label(SlotType) -> String` is added to `dag.rs` but is not `pub`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-scheduler/src/dag.rs` | Add check 5 slot-type compatibility logic and `slot_type_label` helper; update module doc comment |
| Modify | `crates/anvilml-scheduler/tests/dag_tests.rs` | Add >=5 new integration tests for check 5 |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-scheduler/tests/dag_tests.rs` | `test_validate_graph_slot_type_mismatch_reported` | A MODEL→CLIP mismatch produces SlotTypeMismatch with correct fields | Registry has LoadModel (outputs MODEL, CLIP) and ClipTextEncode (inputs CLIP) | Edge from `a:MODEL` to `b:CLIP` | `SlotTypeMismatch { node_id: "b", slot_name: "CLIP", expected: "Clip", found: "Model" }` | `cargo test -p anvilml-scheduler --test dag_tests test_validate_graph_slot_type_mismatch_reported` exits 0 |
| `crates/anvilml-scheduler/tests/dag_tests.rs` | `test_validate_graph_exact_slot_type_match_passes` | Matching output/input types pass cleanly | Registry has LoadModel (MODEL output) and a node with MODEL input | Edge from `a:MODEL` to `b:MODEL` | `Ok(ValidatedGraph)` | `cargo test -p anvilml-scheduler --test dag_tests test_validate_graph_exact_slot_type_match_passes` exits 0 |
| `crates/anvilml-scheduler/tests/dag_tests.rs` | `test_validate_graph_any_on_source_side_passes` | Source output is Any — passes regardless of destination type | Registry has a node with Any output and a node with MODEL input | Edge from `a:any_slot` to `b:MODEL` | `Ok(ValidatedGraph)` | `cargo test -p anvilml-scheduler --test dag_tests test_validate_graph_any_on_source_side_passes` exits 0 |
| `crates/anvilml-scheduler/tests/dag_tests.rs` | `test_validate_graph_any_on_dest_side_passes` | Destination input is Any — passes regardless of source type | Registry has a node with MODEL output and a node with Any input | Edge from `a:MODEL` to `b:any_slot` | `Ok(ValidatedGraph)` | `cargo test -p anvilml-scheduler --test dag_tests test_validate_graph_any_on_dest_side_passes` exits 0 |
| `crates/anvilml-scheduler/tests/dag_tests.rs` | `test_validate_graph_dangling_edge_not_double_reported` | An edge flagged DanglingEdge in check 4 is not also reported as SlotTypeMismatch | Empty registry (all types unknown → edges are dangling) | Edge from `nonexistent:out` to `also_missing:in` | One `DanglingEdge` error, zero `SlotTypeMismatch` | `cargo test -p anvilml-scheduler --test dag_tests test_validate_graph_dangling_edge_not_double_reported` exits 0 |
| `crates/anvilml-scheduler/tests/dag_tests.rs` | `test_validate_graph_multiple_slot_type_mismatches_collected` | Two edges with different mismatches produce two errors | Registry with nodes having incompatible output/input types | Two edges with MODEL→Clip and Int→Float mismatches | Two `SlotTypeMismatch` errors in one `Err` | `cargo test -p anvilml-scheduler --test dag_tests test_validate_graph_multiple_slot_type_mismatches_collected` exits 0 |

## CI Impact

No CI changes required. The new tests are integration tests in the existing `dag_tests.rs` file under `crates/anvilml-scheduler/tests/`, which is already picked up by `cargo test -p anvilml-scheduler`. The workspace test command `cargo test --workspace --features mock-hardware` already covers this crate.

## Platform Considerations

None identified. The check 5 logic is pure data comparison (enum equality and string formatting) with no platform-specific behavior. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are needed.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `SlotType` does not derive `Display`, so converting to string for the error message requires a manual approach. Using `format!("{:?}", ...)` gives Debug output (e.g. `"Model"`) which differs from the serde `SCREAMING_SNAKE_CASE` form (`"MODEL"`). | Medium | Medium | Implement a small private helper function `slot_type_label(t: SlotType) -> String` in `dag.rs` that maps each variant to its `SCREAMING_SNAKE_CASE` form, matching the serde serialization. Not a pub item. |
| Edge format ambiguity: the existing tests use only `"from"` fields. Check 5 requires a `"to"` field for the destination. If the edge format doesn't include `"to"`, the check cannot determine the destination input type. | Medium | High | The plan assumes the edge format includes `"to"` (standard ComfyUI-style). Tests will be written with `"to"` fields. If the ACT agent discovers the edge format doesn't support `"to"`, it must surface this as a blocker. |
| Skipping dangling edges requires tracking which edges were already flagged. A naive approach might skip valid edges if the same node_id appears in both a dangling edge and a valid edge. | Low | Medium | Track dangling edge source node IDs in a `HashSet<String>` built during check 4, then check membership before running check 5 on each edge. This is O(1) per edge lookup. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-scheduler --test dag_tests test_validate_graph_slot_type_mismatch_reported` exits 0
- [ ] `cargo test -p anvilml-scheduler --test dag_tests test_validate_graph_exact_slot_type_match_passes` exits 0
- [ ] `cargo test -p anvilml-scheduler --test dag_tests test_validate_graph_any_on_source_side_passes` exits 0
- [ ] `cargo test -p anvilml-scheduler --test dag_tests test_validate_graph_any_on_dest_side_passes` exits 0
- [ ] `cargo test -p anvilml-scheduler --test dag_tests test_validate_graph_dangling_edge_not_double_reported` exits 0
- [ ] `cargo test -p anvilml-scheduler --test dag_tests test_validate_graph_multiple_slot_type_mismatches_collected` exits 0
- [ ] `cargo test -p anvilml-scheduler --test dag_tests` exits 0 with >=16 tests total in the file
