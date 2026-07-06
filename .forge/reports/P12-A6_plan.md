# Plan Report: P12-A6

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P12-A6                                            |
| Phase       | 12 — Graph Validation                             |
| Description | anvilml-scheduler: validate_graph cycle detection (6), Kahn's algorithm |
| Depends on  | P12-A5                                            |
| Project     | anvilml                                           |
| Planned at  | 2026-07-06T19:30:00Z                              |
| Attempt     | 1                                                 |

## Objective

Add check 6 (cycle detection via Kahn's algorithm) to `validate_graph()` in `dag.rs`, making it the final check in the six-check validation pipeline. When all six checks pass with zero collected errors, construct and return `Ok(ValidatedGraph(graph))` — the only place in the crate where `ValidatedGraph` is ever constructed, closing the construction-gated loop established by P12-A1. Add >= 5 new tests covering cycle detection scenarios in `dag_tests.rs`.

## Scope

### In Scope
- Implement Kahn's algorithm for cycle detection as check 6 in `validate_graph()` (`dag.rs`)
- Compute in-degree per node from the edge list, repeatedly remove zero-in-degree nodes; any nodes remaining form a cycle
- Push `CycleDetected(Vec<String>)` naming every remaining node (not just one representative)
- Update the final return condition: `Ok(ValidatedGraph(graph))` is returned only when all six checks pass with zero errors
- Update the module-level doc comment in `dag.rs` to reflect that checks 1–6 are now implemented
- Add >= 5 new tests in `dag_tests.rs` covering cycle detection scenarios
- No new external dependencies (only `std::collections::HashMap` and `HashSet`, already imported)

### Out of Scope
- `lib.rs` re-export pass and 80-line check — this is P12-B1's responsibility
- Any HTTP handler wiring `validate_graph()` into `POST /v1/jobs` (later phase)
- Job queue, VRAM ledger, or dispatch loop (phases 13+)
- Dual-mode parity markers — `validate_graph()` is not a node `execute()` or arch module `load()`/`sample()`/`decode()`/`compute_latent_shape()`, so the §10.6 parity marker convention does not apply

defers_to (from JSON): []

## Existing Codebase Assessment

The `anvilml-scheduler` crate already has checks 1–5 fully implemented in `dag.rs` and a complete test suite in `dag_tests.rs` (27 tests). The `types.rs` file defines `ValidatedGraph` (construction-gated newtype wrapping `serde_json::Value`) and `GraphError` (7 variants including `CycleDetected(Vec<String>)`, already present from P12-A2). The `NodeTypeRegistry` from `anvilml-core` provides `get()`, `register_all()`, `list()`, `len()`, and `is_empty()` — all used by checks 3–5.

The established patterns include: collect-all-errors semantics (never short-circuit on first error), `HashSet`-based deduplication for seen IDs, edge format `"node_id:slot_name"` parsed via `splitn(2, ':')`, and test fixtures using `serde_json::json!` with `NodeTypeRegistry::new()` + `register_all()`. The `dag.rs` module-level doc comment explicitly notes "Check 6 (cycle detection) is added by a subsequent task in Phase 12" — this comment must be updated.

The current `validate_graph()` returns `Ok(ValidatedGraph(graph))` after checks 1–5 pass with zero errors. With check 6 added, this return path must be gated behind all six checks. The existing tests that currently pass (acyclic graphs) will continue to pass since they have no cycles.

## Resolved Dependencies

| Type   | Name       | Version verified | MCP source | Feature flags confirmed |
|--------|-----------|-----------------|------------|------------------------|
| crate  | serde_json | 1.x (workspace) | Cargo.lock | n/a                    |
| crate  | thiserror  | 2.0.18          | Cargo.lock | n/a                    |

No new external dependencies are introduced. Only `std::collections::HashMap` and `HashSet` are used, which are already imported in `dag.rs`.

## Approach

1. **Add check 6 (cycle detection via Kahn's algorithm) to `validate_graph()` in `dag.rs`.**

   After the existing checks 1–5 block and before the final `if errors.is_empty()` return, insert the cycle detection logic:

   a. Build an adjacency list from the edge list. Iterate all edges in the `"edges"` array (same parsing as checks 4–5: extract `"from"` and `"to"` fields in `"node_id:slot_name"` format). For each valid edge where both source and destination node IDs are in `seen_ids`, add a directed edge `source_node_id → dest_node_id` to the adjacency list (`HashMap<String, Vec<String>>`).

   b. Compute in-degree for every node in the graph. Iterate all nodes from the `"nodes"` array (extract `"id"` field) and initialize in-degree to 0 for each. Then iterate the adjacency list and increment in-degree for each destination node.

   c. Initialize a queue (simple `Vec<String>`) with all nodes that have in-degree 0.

   d. Process the queue: while the queue is non-empty, pop a node, add it to a `processed` set, and for each neighbor in the adjacency list, decrement its in-degree; if the neighbor's in-degree becomes 0, push it to the queue.

   e. After the queue is exhausted, any node NOT in the `processed` set is part of a cycle. Collect these remaining node IDs into a `Vec<String>`.

   f. If there are remaining nodes (cycle detected), push `GraphError::CycleDetected(remaining_nodes)` to the errors vector.

   Rationale: Kahn's algorithm is the standard topological-sort approach for cycle detection. It runs in O(V + E) time where V is the number of nodes and E is the number of edges. The algorithm naturally identifies ALL nodes in cycles (not just one representative), which matches the `CycleDetected(Vec<String>)` error shape.

   Implementation detail: use a `HashSet<String>` for the `processed` set and a `Vec<String>` as a queue (push/pop from end for O(1) operations — order doesn't matter for correctness, only completeness).

2. **Update the final return condition.**

   The existing code at line 351–355 returns `Ok(ValidatedGraph(graph))` when `errors.is_empty()`. This is already correct for the all-checks-pass case — no code change needed here. However, the function's doc comment (lines 60–73) must be updated to say "checks 1–6" instead of "checks 1–5".

3. **Update the module-level doc comment in `dag.rs` (lines 1–13).**

   Replace:
   ```
   /// Check 6 (cycle detection) is added by a subsequent task in Phase 12.
   ```
   With:
   ```
   /// Checks 1–6 (structural root, duplicate IDs, unknown types, dangling edges,
   /// slot-type compatibility, and cycle detection via Kahn's algorithm).
   ```

4. **Add >= 5 new tests in `dag_tests.rs`.**

   Test 1 — `test_validate_graph_simple_two_node_cycle`: Create a graph with two nodes ("a" and "b") and two edges forming a 2-cycle: `a → b` and `b → a`. Both node IDs must appear in the `CycleDetected` error's Vec.

   Test 2 — `test_validate_graph_three_node_cycle`: Create a graph with three nodes ("a", "b", "c") and edges forming a 3-cycle: `a → b`, `b → c`, `c → a`. All three node IDs must appear in the `CycleDetected` error's Vec.

   Test 3 — `test_validate_graph_acyclic_graph_with_all_checks_passing`: Create a graph with 3+ nodes and valid edges that form no cycles, all node types registered, and correct slot types. This must return `Ok(ValidatedGraph)` — exercising the final `Ok(...)` return path with all six checks passing.

   Test 4 — `test_validate_graph_cycle_with_other_violations`: Create a graph with a 2-cycle AND an unknown node type. Both `CycleDetected` and `UnknownNodeType` errors must be collected in one `Err(Vec)`.

   Test 5 — `test_validate_graph_no_edges_no_cycle`: Create a graph with nodes but no edges. With no edges, in-degrees are all 0, so no cycle is possible. This must return `Ok(ValidatedGraph)` — verifying the edge case where a graph has nodes but zero edges is trivially acyclic.

   Test 6 — `test_validate_graph_self_loop_cycle`: Create a graph with one node "a" and an edge from "a" to "a" (self-loop). This must produce `CycleDetected(["a"])` — verifying that self-loops are detected as cycles.

   Test 7 — `test_validate_graph_partial_cycle_in_larger_graph`: Create a graph with 4 nodes where 3 form a cycle (a → b → c → a) and one node "d" is a valid leaf with no incoming edges. Only the 3 cycle nodes must appear in `CycleDetected` — "d" must NOT be listed, confirming Kahn's algorithm correctly distinguishes cycle nodes from non-cycle nodes.

## Public API Surface

No new `pub` items are introduced. The only change is internal to `validate_graph()` in `anvilml_scheduler::dag`, which is already `pub`. The function signature remains:

```rust
pub fn validate_graph(
    graph: Value,
    registry: &NodeTypeRegistry,
) -> Result<ValidatedGraph, Vec<GraphError>>
```

The `CycleDetected(Vec<String>)` variant of `GraphError` already exists in `types.rs` from P12-A2.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-scheduler/src/dag.rs` | Add check 6 (Kahn's algorithm cycle detection) and update doc comments |
| Modify | `crates/anvilml-scheduler/tests/dag_tests.rs` | Add >= 5 new tests for cycle detection scenarios |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `dag_tests.rs` | `test_validate_graph_simple_two_node_cycle` | A 2-node cycle (a→b, b→a) produces `CycleDetected` with both node IDs in the Vec | `cargo test -p anvilml-scheduler --test dag_tests test_validate_graph_simple_two_node_cycle` exits 0 |
| `dag_tests.rs` | `test_validate_graph_three_node_cycle` | A 3-node cycle (a→b→c→a) produces `CycleDetected` with all 3 node IDs | `cargo test -p anvilml-scheduler --test dag_tests test_validate_graph_three_node_cycle` exits 0 |
| `dag_tests.rs` | `test_validate_graph_acyclic_graph_with_all_checks_passing` | A fully valid acyclic graph (registered types, correct slots, no cycles) returns `Ok(ValidatedGraph)` | `cargo test -p anvilml-scheduler --test dag_tests test_validate_graph_acyclic_graph_with_all_checks_passing` exits 0 |
| `dag_tests.rs` | `test_validate_graph_cycle_with_other_violations` | A graph with both a cycle and an unknown node type collects both errors in one Err | `cargo test -p anvilml-scheduler --test dag_tests test_validate_graph_cycle_with_other_violations` exits 0 |
| `dag_tests.rs` | `test_validate_graph_no_edges_no_cycle` | A graph with nodes but no edges is trivially acyclic, returns `Ok(ValidatedGraph)` | `cargo test -p anvilml-scheduler --test dag_tests test_validate_graph_no_edges_no_cycle` exits 0 |
| `dag_tests.rs` | `test_validate_graph_self_loop_cycle` | A single-node self-loop (a→a) produces `CycleDetected(["a"])` | `cargo test -p anvilml-scheduler --test dag_tests test_validate_graph_self_loop_cycle` exits 0 |
| `dag_tests.rs` | `test_validate_graph_partial_cycle_in_larger_graph` | A 4-node graph where 3 form a cycle and 1 is a valid leaf: only cycle nodes in `CycleDetected` | `cargo test -p anvilml-scheduler --test dag_tests test_validate_graph_partial_cycle_in_larger_graph` exits 0 |

## CI Impact

No CI changes required. The new tests are added to the existing `dag_tests.rs` file, which is already collected and run by `cargo test -p anvilml-scheduler --test dag_tests`. The CI workflow (`.github/workflows/ci.yml`) runs `cargo test --workspace --features mock-hardware`, which includes this crate's tests.

## Platform Considerations

None identified. Kahn's algorithm operates purely on in-memory data structures (`HashMap`, `HashSet`, `Vec`). No platform-specific code, no `#[cfg(unix)]`/`#[cfg(windows)]` guards required. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Edge parsing in check 6 duplicates the parsing logic from checks 4 and 5 (extracting `"from"` and `"to"` fields, splitting on `:`). If the parsing diverges, check 6 may miss edges or produce incorrect results. | Low | Medium | Reuse the exact same parsing pattern as checks 4–5: `edge.get("from")` → `splitn(2, ':')`. The adjacency list only includes edges where both source and destination node IDs are in `seen_ids`, matching check 4's existence validation. |
| The adjacency list may include edges that were already flagged as `DanglingEdge` in check 4 (where the source node doesn't exist or doesn't declare the slot). These edges should not contribute to cycle detection since they are structurally invalid. | Low | Low | Only add edges to the adjacency list where BOTH source and destination node IDs are in `seen_ids` (the set built in check 2). This naturally excludes dangling edges where the source node doesn't exist. For check 4 edges where the source exists but the slot is undeclared, the edge still has a valid source node ID, so it would be included in the adjacency list — this is correct because the cycle is about node-level graph structure, not slot-level validity. |
| Kahn's algorithm implementation has a subtle bug in the in-degree computation or queue processing that misses some cycles. | Low | High | Write the self-loop test (test 6) and partial-cycle test (test 7) specifically to catch common implementation errors: self-loops are a common edge case where in-degree starts at 1 and never reaches 0; partial cycles test that non-cycle nodes are correctly excluded from the result. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-scheduler --test dag_tests` exits 0 with >= 32 tests (27 existing + 5+ new)
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
- [ ] `cargo fmt --all -- --check` exits 0
- [ ] The `CycleDetected` error names every node in the cycle, not just one representative (verified by `test_validate_graph_simple_two_node_cycle` and `test_validate_graph_three_node_cycle`)
- [ ] A fully valid acyclic graph with all checks passing returns `Ok(ValidatedGraph)` (verified by `test_validate_graph_acyclic_graph_with_all_checks_passing`)
- [ ] Cycle detection runs in collect-all-errors mode: a graph with both a cycle and another violation produces a single `Err(Vec)` containing both error types (verified by `test_validate_graph_cycle_with_other_violations`)
