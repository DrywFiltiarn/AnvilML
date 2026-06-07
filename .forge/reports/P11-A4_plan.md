# Plan Report: P11-A4

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P11-A4                                      |
| Phase       | 011 — Graph Validation                      |
| Description | anvilml-scheduler: dag.rs cycle detection (Kahn) |
| Depends on  | P11-A3                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-06-07T09:45:00Z                        |
| Attempt     | 1                                           |

## Objective

Add cycle detection to `anvilml-scheduler::dag::validate_graph` using Kahn's topological sort algorithm. Build a directed adjacency graph from node edge references, perform the topo-sort, and if not all nodes are processed (indicating a cycle), collect the unprocessed node IDs into an error string `'cycle_detected: {ids}'`. This is the final validation check so that `validate_graph` returns `Ok(ValidatedGraph)` only when duplicate-id, unknown-type, bad-edge, **and** cycle checks all pass.

## Scope

### In Scope
- Extend `crates/anvilml-scheduler/src/dag.rs`:
  - Build adjacency list from edge references (already extracted during P11-A3's edge validation pass).
  - Implement Kahn's algorithm for topological sort on the node graph.
  - If `processed_count < total_nodes`, collect unprocessed node IDs and emit `'cycle_detected: {ids}'` error.
  - Add unit test(s) in `dag.rs` confirming:
    - A minimal 2-node cycle (A→B, B→A) is detected.
    - A valid ZiT 5-node graph (ZitLoadPipeline → ZitTextEncode → ZitSampler → ZitDecode → SaveImage) passes cleanly through the full pipeline including cycle check.

### Out of Scope
- No changes to `nodes.rs`, `lib.rs`, `Cargo.toml`, or any other crate.
- No new error types — reuses `Vec<String>` from existing signature.
- No API handler changes (that is P11-A5).
- No logging additions — this is a pure data validation function with no side effects; logging is not required for the validator itself (logging would be at the server/handler level in P11-A5).

## Approach

1. **Read existing `dag.rs`** to confirm current structure: `validate_graph()` currently performs checks a–d (root object, nodes array, duplicate IDs, unknown types, edge references) and returns early on errors. The cycle check will be the final validation step before returning `Ok`.

2. **Add adjacency list construction inside `validate_graph`** — after building `id_type_map` for edge validation, also build a `HashMap<&str, Vec<&str>>` mapping each node's ID to the IDs of nodes it depends on (i.e., for each input that is an edge ref `{node_id: X}`, add an edge from current node → X). This captures the dependency direction needed for topo-sort.

3. **Implement Kahn's algorithm** as a helper function or inline block:
   - Compute in-degree for every node (count how many other nodes reference it as an edge target).
   - Seed a queue with all nodes having in-degree 0.
   - Process the queue: for each dequeued node, decrement in-degree of its dependents; add any that reach 0 to the queue.
   - Track `processed_count`.

4. **Cycle detection**: if `processed_count < total_nodes`, collect all node IDs not yet processed (those still with in-degree > 0) into a sorted list, format as `'cycle_detected: {id1},{id2},...'`, and push to `errors`.

5. **Add tests** in the existing `#[cfg(test)] mod tests` block within `dag.rs`:
   - `test_cycle_detected_2node`: two nodes referencing each other → error contains `'cycle_detected'`.
   - `test_valid_zit_5node_passes`: a full ZiT pipeline with 5 nodes and valid edges → `Ok(ValidatedGraph)`.

6. **Run `cargo test -p anvilml-scheduler -- dag`** to verify all tests (including pre-existing ones from P11-A2/A3) pass.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-scheduler/src/dag.rs` | Add Kahn cycle detection logic + 2 unit tests |

No other files are modified. No `Cargo.toml` changes (no new dependencies). No version bump needed for this task since no source files outside `dag.rs` are touched and the crate's existing patch-level code change only touches `dag.rs`.

Wait — per FORGE_AGENT_RULES §12, every task that modifies source files inside a crate must increment that crate's patch version. The `anvilml-scheduler` crate is being modified (source file `dag.rs`), so:

| Modify | `crates/anvilml-scheduler/Cargo.toml` | Bump patch version `0.1.3 → 0.1.4` |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `crates/anvilml-scheduler/src/dag.rs` | `test_cycle_detected_2node` | A graph with two nodes forming a cycle (A→B, B→A) returns Err containing `'cycle_detected'` |
| `crates/anvilml-scheduler/src/dag.rs` | `test_valid_zit_5node_passes` | A valid ZiT 5-node linear pipeline passes all validations including cycle check |

## CI Impact

No CI changes required. The change is purely a new validation step within an existing test module. All existing tests continue to pass; two new tests are added. The full test suite (`cargo test --workspace --features mock-hardware`) remains the gate command, unchanged.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Adjacency direction confusion (dependents vs dependencies) | Medium | High — wrong direction produces false negatives or positives on cycle detection | Build adjacency carefully: if node A's input references node B's output, the edge is A→B (A depends on B). Kahn's algorithm processes nodes with no incoming edges first. In-degree counts how many nodes depend on a given node. |
| Edge cases: self-loop (node references itself) | Low | Medium — must be detected as a cycle | Kahn's handles self-loops naturally: the node has in-degree ≥ 1 and can never reach 0, so it remains unprocessed. |
| Edge cases: diamond dependency (A→B, A→C, B→D, C→D) | Low | Low — should pass; D has in-degree 2, both B and C have in-degree 1 | Kahn's algorithm correctly processes diamonds; no cycle present. |
| Existing tests break from reordering of checks | Low | Medium — existing P11-A2/A3 tests may expect specific error lists | Cycle check runs last (after all other validations), so errors from earlier checks are unaffected. If earlier checks fail, the function returns early before reaching cycle detection. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-scheduler -- dag_cycle` exits 0: 2-node cycle reported as `'cycle_detected'`
- [ ] `cargo test -p anvilml-scheduler -- dag` exits 0: valid ZiT 5-node graph passes clean, all pre-existing tests still pass
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0 (zero warnings)
- [ ] `crates/anvilml-scheduler/Cargo.toml` version bumped from `0.1.3` to `0.1.4`
