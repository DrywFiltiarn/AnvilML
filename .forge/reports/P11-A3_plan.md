# Plan Report: P11-A3

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P11-A3                                      |
| Phase       | 011 — Graph Validation                        |
| Description | anvilml-scheduler: dag.rs edge-reference validation |
| Depends on  | P11-A2, P11-A1                                |
| Project     | anvilml                                       |
| Planned at  | 2026-06-07T09:38:00Z                          |
| Attempt     | 1                                             |

## Objective

Extend `dag.rs::validate_graph()` to validate edge references in node inputs. For each input that is an object `{node_id, output_slot}`, report `'unknown_node_ref: {node_id}'` if the referenced node does not exist in the graph, and `'unknown_output_slot: {node_id}.{slot}'` if the referenced node's type does not declare that output slot in `NODE_SLOTS`.

## Scope

### In Scope
- Extend `validate_graph()` in `crates/anvilml-scheduler/src/dag.rs` to iterate each node's `inputs` object and validate edge references.
- Add two new error strings: `unknown_node_ref: {node_id}` and `unknown_output_slot: {node_id}.{slot}`.
- Add a test module function `dag_edges` (test name prefix) with three cases:
  1. Bad node reference (`node_id` absent from graph) → reports `unknown_node_ref`.
  2. Bad output slot (referenced node exists but type lacks that output slot) → reports `unknown_output_slot`.
  3. Valid edge (all references correct) → validation passes with `Ok`.
- Bump `anvilml-scheduler` crate patch version from `0.1.2` to `0.1.3`.

### Out of Scope
- Cycle detection (P11-A4).
- Input slot type-checking (does the input value match the expected type for that slot name) — not in scope.
- Any changes to `nodes.rs`, `lib.rs`, or other crates beyond the version bump.
- Changes to CI, formatting, or documentation files.

## Approach

1. **Read existing `dag.rs`** (already done). The function collects errors into a `Vec<String>`, iterates nodes, checks duplicate IDs and unknown types. After the type check loop but before the `if !errors.is_empty()` early-return, insert the edge-reference validation pass.

2. **Build a node-type lookup map.** Before iterating nodes for edges (or during the same first-pass iteration), collect a map from node `id` → node `type`. This requires two approaches:
   - Option A (preferred): After the existing duplicate-ID + unknown-type pass, build a `HashMap<&str, &str>` mapping `id → type` from `seen_ids` and the original nodes array. Then iterate all nodes again for edge validation.
   - Option B: Single-pass — accumulate both ID→type map and errors in one loop. This is slightly more complex but avoids a second full iteration.

   **Decision:** Use Option A (two passes) for clarity and to keep the existing error-collection logic untouched. The first pass (already implemented) collects IDs and types; after it completes, build the map from `seen_ids` + the nodes array, then run the edge-validation loop.

3. **Edge-validation algorithm** (new code block inserted before `if !errors.is_empty()`):
   ```
   // Build id→type lookup from the already-validated nodes.
   let mut id_type_map: HashMap<&str, &str> = HashMap::new();
   for node in nodes {
       if let Value::Object(obj) = node {
           if let (Some(Value::String(id)), Some(Value::String(t))) =
               (obj.get("id"), obj.get("type")) {
               id_type_map.insert(id.as_str(), t.as_str());
           }
       }
   }

   // For each node, iterate its inputs object.
   for node in nodes {
       if let Value::Object(obj) = node {
           if let Some(inputs_val) = obj.get("inputs") {
               if let Some(inputs_obj) = inputs_obj.as_object() {
                   for (slot_name, input_value) in inputs_obj {
                       // If input is an object with node_id + output_slot keys, it's an edge ref.
                       if let (Some(Value::String(ref_node_id)), Some(Value::String(ref_slot))) =
                           (input_value.get("node_id"), input_value.get("output_slot")) {

                           // Check 1: does the referenced node exist?
                           if !id_type_map.contains_key(ref_node_id.as_str()) {
                               errors.push(format!("unknown_node_ref: {}", ref_node_id));
                               continue; // skip slot check — node doesn't exist
                           }

                           // Check 2: does that node's type declare this output slot?
                           let ref_type = id_type_map[ref_node_id.as_str()];
                           if let Some(slots) = get_node_slots(ref_type) {
                               if !slots.outputs.contains(&ref_slot.as_str()) {
                                   errors.push(format!(
                                       "unknown_output_slot: {}.{}",
                                       ref_node_id, ref_slot
                                   ));
                               }
                           }
                       }
                   }
               }
           }
       }
   }
   ```

4. **Add `use std::collections::HashMap;`** import at the top of `dag.rs`.

5. **Write tests.** Add three test functions in the existing `mod tests` block in `dag.rs`:
   - `test_unknown_node_ref()` — graph with two nodes where one references a non-existent node ID.
   - `test_unknown_output_slot()` — graph with two nodes where output slot doesn't exist on the referenced type.
   - `test_valid_edge_references()` — a valid ZiT 2-node graph (ZitLoadPipeline → ZitTextEncode) where pipeline edge is correct.

6. **Bump version.** Change `version = "0.1.2"` to `version = "0.1.3"` in `crates/anvilml-scheduler/Cargo.toml`.

7. **Verify.** Run `cargo test -p anvilml-scheduler -- dag_edges` (or equivalent test filter) — all new tests must pass. Run `cargo clippy -p anvilml-scheduler --features mock-hardware -- -D warnings` with zero warnings.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-scheduler/src/dag.rs` | Add edge-reference validation logic + tests |
| Modify | `crates/anvilml-scheduler/Cargo.toml` | Bump patch version `0.1.2 → 0.1.3` |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `dag.rs` (mod tests) | `test_unknown_node_ref` | Graph with edge referencing absent node_id → error `unknown_node_ref: {id}` |
| `dag.rs` (mod tests) | `test_unknown_output_slot` | Graph with edge to existing node but nonexistent output slot → error `unknown_output_slot: {node}.{slot}` |
| `dag.rs` (mod tests) | `test_valid_edge_references` | Valid ZiT 2-node graph with correct edge → `Ok(ValidatedGraph)` |

## CI Impact

No CI changes required. The test is a unit test within the existing crate; it runs under the standard `cargo test --workspace --features mock-hardware` gate. No new dev-dependencies or feature flags needed.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Edge-reference validation fires on inputs that are plain objects without `node_id`/`output_slot` keys (e.g. user data objects) | Low | Medium | Only treat as edge ref when **both** `node_id` and `output_slot` keys are present and both are strings — matches the design spec exactly. Non-matching objects are silently skipped (treated as literals). |
| Test ordering dependency with P11-A2 tests (env-var pollution) | Low | Low | This task does not set any `ANVILML_MOCK_*` env vars; no serial test needed. |
| HashMap import adds dependency overhead | None | None | `std::collections::HashMap` is in the standard library — zero additional deps. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-scheduler -- dag_edges` (or equivalent filter matching new tests) exits 0 with all three tests passing
- [ ] `cargo clippy -p anvilml-scheduler --features mock-hardware -- -D warnings` exits 0 with zero warnings
- [ ] Version in `crates/anvilml-scheduler/Cargo.toml` is `0.1.3`
- [ ] Error string format for bad node ref: `unknown_node_ref: {node_id}` (exact match)
- [ ] Error string format for bad slot: `unknown_output_slot: {node_id}.{slot}` (exact match)
- [ ] Valid edge references produce `Ok(ValidatedGraph)` with no errors

