# Plan Report: P12-A2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P12-A2                                        |
| Phase       | 012 — Graph Validation                        |
| Description | GraphError enum and types.rs ValidatedGraph     |
| Depends on  | P12-A1                                        |
| Project     | anvilml                                       |
| Planned at  | 2026-06-19T18:30:00Z                          |
| Attempt     | 1                                             |

## Objective

Define `GraphError` as a typed enum covering all five graph validation failure modes
(`UnknownNodeType`, `DuplicateNodeId`, `UnknownEdgeRef`, `SlotTypeMismatch`,
`CycleDetected`) and implement `Display` for each variant so the human-readable error
strings used by the existing `validate_graph` return type (`Result<ValidatedGraph, Vec<String>>`)
can be produced from enum instances. Create `crates/anvilml-scheduler/src/types.rs` as a
new module that re-exports `ValidatedGraph` from `dag.rs` and publishes `GraphError`
publicly, then update `dag.rs` to use `GraphError` internally while keeping the same
public API surface so existing `dag_tests.rs` tests pass without modification.

## Scope

### In Scope
- **CREATE** `crates/anvilml-scheduler/src/types.rs` — `GraphError` enum with five variants,
  `Display` implementation, `Debug` derive, and re-export of `ValidatedGraph` from `dag.rs`.
- **MODIFY** `crates/anvilml-scheduler/src/dag.rs` — import `GraphError`, refactor each
  internal check function to return `Vec<GraphError>` instead of `Vec<String>`, convert
  to strings only at the top-level `validate_graph` return point.
- **MODIFY** `crates/anvilml-scheduler/src/lib.rs` — add `pub mod types;` and
  `pub use types::GraphError;`.
- **MODIFY** `crates/anvilml-scheduler/Cargo.toml` — bump patch version `0.1.3 → 0.1.4`.
- **NO CHANGES** to `dag_tests.rs` — the public API of `validate_graph` is unchanged
  (`Result<ValidatedGraph, Vec<String>>`), so all existing tests must continue to pass.

### Out of Scope
- Adding new test cases (P12-A1 already has ≥ 8 tests).
- Changing the `ValidatedGraph` definition (it stays in `dag.rs`).
- Any changes to `anvilml-core` types (`SlotType`, `NodeTypeDescriptor`, etc.).
- Wiring `validate_graph` into the HTTP handler (that is P12-B1).
- Adding new external dependencies.

## Existing Codebase Assessment

**What already exists:** P12-A1 created `dag.rs` with `ValidatedGraph` (newtype wrapping
`serde_json::Value`) and `validate_graph` which performs six validation checks, collecting
all errors as `Vec<String>`. Each check function (`check_duplicate_ids`, `check_node_types`,
`check_edge_refs`, `check_slot_compatibility`, `check_acyclic`) returns `Vec<String>`. The
public API is `Result<ValidatedGraph, Vec<String>>`. Tests in `dag_tests.rs` exercise all
six checks plus the `Any` slot compatibility rule and multi-error collection.

**Established patterns:**
- Error strings all follow the prefix `"validation failed: "` followed by a descriptive
  message naming the offending entity (node id, slot name, type name).
- `SlotType` uses `#[derive(Debug)]` only — the existing tests rely on `{slot_type:?}`
  (Debug format) for PascalCase names like `"Model"`, `"Image"` in error strings.
- `NodeTypeRegistry::get()` is async and returns `Option<NodeTypeDescriptor>`.
- `lib.rs` uses `pub use` for re-exports from sibling modules and from `anvilml-core`.
- `lib.rs` contains only `pub mod`, `pub use`, and the crate-level `//!` doc comment
  (20 lines, well under the 80-line threshold).

**Gap between design doc and current source:** The task context describes `GraphError`
variants with specific field names. The current codebase has no `GraphError` type — all
errors are plain strings. The `SlotType` type does not implement `Display`, only `Debug`,
so the `SlotTypeMismatch` Display impl must use `{from:?}` and `{to:?}` formatting to
produce PascalCase names (matching what existing tests assert).

## Resolved Dependencies

None. This task introduces no new external crates or packages. All types used
(`GraphError`, `SlotType`, `ValidatedGraph`) are either created in this task or
already exist in `anvilml-core` / `dag.rs`.

| Type   | Name    | Version verified | MCP source | Feature flags confirmed |
|--------|---------|-----------------|------------|------------------------|
| (none) | —       | —               | —          | —                      |

## Approach

1. **Create `crates/anvilml-scheduler/src/types.rs`.** Define the `GraphError` enum with
   five variants matching the task specification exactly:
   ```rust
   #[derive(Debug, Clone)]
   pub enum GraphError {
       UnknownNodeType(String),
       DuplicateNodeId(String),
       UnknownEdgeRef { node_id: String, slot: String },
       SlotTypeMismatch { from: SlotType, to: SlotType },
       CycleDetected(Vec<String>),
   }
   ```
   Derive `Debug` and `Clone` (following the pattern of `ValidatedGraph` and other
   error-adjacent types in the crate). Do **not** derive `PartialEq` or `Eq` — this is
   an error type, not a comparison type, and deriving them would encourage equality
   checks on error variants which is an anti-pattern.

   Implement `Display for GraphError` that produces the same human-readable strings
   the current check functions already produce. Each variant's `Display` impl maps to
   the corresponding error message format:
   - `UnknownNodeType(id)` → `"validation failed: node \"{id}\" has unknown type \"{id}\""`
     (the type_name is the same as the id field since the function receives a single
     string for both — see `check_node_types` which formats `"{node_id}"` and `"{node_type}"`)
   - `DuplicateNodeId(id)` → `"validation failed: duplicate node id \"{id}\""`
   - `UnknownEdgeRef { node_id, slot }` → `"validation failed: node \"{node_id}\" has no output slot \"{slot}\""`
   - `SlotTypeMismatch { from, to }` → `"validation failed: slot type mismatch on edge from ..." `{from:?}` ... `{to:?}``
     (uses Debug format because `SlotType` does not implement Display — this produces
     PascalCase names like `"Model"` matching the existing test assertions)
   - `CycleDetected(nodes)` → `"validation failed: cycle detected involving nodes: {joined}"`

   Re-export `ValidatedGraph` from `dag.rs`:
   ```rust
   pub use crate::dag::ValidatedGraph;
   ```

   Add a `///` doc comment on `GraphError` describing each variant's purpose.

2. **Modify `crates/anvilml-scheduler/src/dag.rs`.** Add `use crate::types::GraphError;`
   to the imports. Refactor each internal check function to return `Vec<GraphError>`
   instead of `Vec<String>`:
   - `check_duplicate_ids` → returns `GraphError::DuplicateNodeId(id)` for each duplicate.
   - `check_node_types` → returns `GraphError::UnknownNodeType(type_name)` for each unknown.
   - `check_edge_refs` → returns `GraphError::UnknownEdgeRef { node_id, slot }` for
     missing nodes and missing slots.
   - `check_slot_compatibility` → returns `GraphError::SlotTypeMismatch { from, to }`
     for incompatible pairs.
   - `check_acyclic` → returns `GraphError::CycleDetected(cycle_nodes)` when a cycle
     is detected.

   In the top-level `validate_graph` function, after collecting all `GraphError` instances,
   convert them to strings with `.into_iter().map(|e| e.to_string()).collect()` before
   returning `Err(errors)`. This is the only place where `String` conversion happens —
   internal functions work with typed errors throughout.

   The public signature of `validate_graph` is unchanged:
   ```rust
   pub async fn validate_graph(
       graph: &Value,
       registry: &NodeTypeRegistry,
   ) -> Result<ValidatedGraph, Vec<String>>
   ```
   This ensures `dag_tests.rs` needs zero modifications.

3. **Modify `crates/anvilml-scheduler/src/lib.rs`.** Add two lines after the existing
   `pub use anvilml_core::NodeTypeRegistry;`:
   ```rust
   pub mod types;
   pub use types::GraphError;
   ```
   This publishes `GraphError` at the crate root so downstream crates (specifically
   `anvilml-server` in P12-B1) can import it as `anvilml_scheduler::GraphError`.

4. **Bump `crates/anvilml-scheduler/Cargo.toml`** patch version from `0.1.3` to `0.1.4`.
   Per ENVIRONMENT.md §12, only the `Z` in `X.Y.Z` changes.

5. **Verify** by running `cargo test -p anvilml-scheduler --features mock-hardware`.
   All existing `dag_tests.rs` tests must pass without modification because the public
   API is unchanged.

## Public API Surface

| Crate/Module | Item | Type | Signature / Definition |
|--------------|------|------|----------------------|
| `anvilml-scheduler::types` | `GraphError` | `pub enum` | `pub enum GraphError { UnknownNodeType(String), DuplicateNodeId(String), UnknownEdgeRef { node_id: String, slot: String }, SlotTypeMismatch { from: SlotType, to: SlotType }, CycleDetected(Vec<String>) }` |
| `anvilml-scheduler::types` | `ValidatedGraph` | `pub use` | Re-exported from `crate::dag::ValidatedGraph` |
| `anvilml-scheduler` | `GraphError` | `pub use` | `pub use types::GraphError;` (crate root re-export) |

No new `pub fn` items. The `Display` impl on `GraphError` is `impl Display for GraphError`
which is not itself `pub` but is public because `Display` is a public trait and the impl
is `pub`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-scheduler/src/types.rs` | `GraphError` enum, `Display` impl, `ValidatedGraph` re-export |
| MODIFY | `crates/anvilml-scheduler/src/dag.rs` | Import `GraphError`, refactor check functions to return `Vec<GraphError>`, convert to strings at return point |
| MODIFY | `crates/anvilml-scheduler/src/lib.rs` | Add `pub mod types; pub use types::GraphError;` |
| MODIFY | `crates/anvilml-scheduler/Cargo.toml` | Bump patch version `0.1.3 → 0.1.4` |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-scheduler/tests/dag_tests.rs` | `test_missing_nodes_array` | Existing test passes unchanged — `validate_graph` returns error string containing "nodes" and "missing" | Registry populated with no node types | Graph with `"edges": []` but no `"nodes"` | `Err(Vec<String>)` with ≥ 1 error | `cargo test -p anvilml-scheduler --features mock-hardware -- test_missing_nodes_array` exits 0 |
| `crates/anvilml-scheduler/tests/dag_tests.rs` | `test_duplicate_node_ids` | Existing test passes — duplicate ID detected via `GraphError::DuplicateNodeId` | Registry has `LoadModel` | Graph with two nodes sharing `"id": "n1"` | Error contains "duplicate" and "n1" | `cargo test -p anvilml-scheduler --features mock-hardware -- test_duplicate_node_ids` exits 0 |
| `crates/anvilml-scheduler/tests/dag_tests.rs` | `test_unknown_node_type` | Existing test passes — unknown type detected via `GraphError::UnknownNodeType` | Registry has `LoadModel` only | Graph with node type `"NonExistent"` | Error contains "NonExistent" and "unknown type" | `cargo test -p anvilml-scheduler --features mock-hardware -- test_unknown_node_type` exits 0 |
| `crates/anvilml-scheduler/tests/dag_tests.rs` | `test_bad_edge_ref_missing_node` | Existing test passes — missing source node via `GraphError::UnknownEdgeRef` | Registry has `LoadModel` | Edge referencing `"ghost"` node | Error contains "ghost" and "missing source node" | `cargo test -p anvilml-scheduler --features mock-hardware -- test_bad_edge_ref_missing_node` exits 0 |
| `crates/anvilml-scheduler/tests/dag_tests.rs` | `test_bad_edge_ref_missing_slot` | Existing test passes — missing slot via `GraphError::UnknownEdgeRef` | Registry has `LoadModel` with `"model"` output only | Edge with `"output_slot": "nonexistent"` | Error contains "nonexistent" and "no output slot" | `cargo test -p anvilml-scheduler --features mock-hardware -- test_bad_edge_ref_missing_slot` exits 0 |
| `crates/anvilml-scheduler/tests/dag_tests.rs` | `test_slot_type_mismatch` | Existing test passes — type mismatch via `GraphError::SlotTypeMismatch` | Registry has `LoadModel` (Model output) and `SaveImage` (Image input) | Edge connecting Model→Image | Error contains "type mismatch", "Model", "Image" | `cargo test -p anvilml-scheduler --features mock-hardware -- test_slot_type_mismatch` exits 0 |
| `crates/anvilml-scheduler/tests/dag_tests.rs` | `test_cycle_detected` | Existing test passes — cycle via `GraphError::CycleDetected` | Registry has NodeA, NodeB, NodeC | A→B→C→A cycle | Error contains "cycle", "A", "B", "C" | `cargo test -p anvilml-scheduler --features mock-hardware -- test_cycle_detected` exits 0 |
| `crates/anvilml-scheduler/tests/dag_tests.rs` | `test_valid_graph_returns_ok` | Existing test passes — valid graph returns `Ok(ValidatedGraph)` | Registry has all four node types for full pipeline | Valid LoadModel→Sampler→VaeDecode→SaveImage graph | `Ok(ValidatedGraph(_))` | `cargo test -p anvilml-scheduler --features mock-hardware -- test_valid_graph_returns_ok` exits 0 |
| `crates/anvilml-scheduler/tests/dag_tests.rs` | `test_multiple_errors_collected` | Existing test passes — ≥ 2 errors collected in single response | Registry has `LoadModel` only | Graph with duplicate ID + unknown type | ≥ 2 error strings containing "duplicate" and "NonExistent" | `cargo test -p anvilml-scheduler --features mock-hardware -- test_multiple_errors_collected` exits 0 |
| `crates/anvilml-scheduler/tests/dag_tests.rs` | `test_any_slot_type_compatible` | Existing test passes — `Any` type accepts any connection | Registry has `NodeAny` (Any output) and `NodeModel` (Model input) | Edge connecting Any→Model | `Ok(ValidatedGraph(_))` | `cargo test -p anvilml-scheduler --features mock-hardware -- test_any_slot_type_compatible` exits 0 |

## CI Impact

No CI changes required. This task modifies only `anvilml-scheduler` crate source files
and its `Cargo.toml`. The existing CI jobs (`rust-linux`, `rust-windows`) already run
`cargo test --workspace --features mock-hardware` which includes this crate. No new
file types, gate definitions, or test modules are introduced.

## Platform Considerations

None identified. The `GraphError` enum and its `Display` implementation are purely
in-memory types with no platform-specific behaviour. No `#[cfg(unix)]` or
`#[cfg(windows)]` guards are needed. The Windows cross-check in ENVIRONMENT.md §7
is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `SlotType` does not implement `Display` — the `SlotTypeMismatch` Display impl must use `{from:?}` (Debug format). If the existing tests assert on a specific format string (e.g. `"Model"` vs `"model"`), the Debug format (PascalCase) must match. The test comment explicitly says "SlotType debug format uses PascalCase" and asserts on `"Model"` and `"Image"`. | Low | High | Verify the exact assertion strings in `test_slot_type_mismatch` (lines 250-252 of dag_tests.rs) which assert `e.contains("Model")` and `e.contains("Image")`. The Debug format of `SlotType` produces PascalCase, matching these assertions. |
| Refactoring check functions to return `Vec<GraphError>` instead of `Vec<String>` requires changing every call site within `dag.rs`. A missed conversion or incorrect field mapping in one variant will cause a compile error. | Low | Medium | The compiler will catch any mismatch. The `GraphError` variants map 1:1 with the current error strings, so the Display impl is a mechanical translation. Write the Display impl first, then refactor check functions one at a time, compiling after each change. |
| Re-exporting `ValidatedGraph` from `types.rs` while it is still defined in `dag.rs` creates a circular module path (`dag.rs` → `types.rs` → `dag.rs`). Rust module system handles this because `pub use crate::dag::ValidatedGraph` in `types.rs` references the parent crate's `dag` module, not a sibling re-export cycle. | Low | Low | This is a standard Rust pattern. The `pub use` in `types.rs` imports from `crate::dag`, which is a sibling module — no circular dependency at the module resolution level. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-scheduler --features mock-hardware` exits 0
- [ ] `cargo clippy --package anvilml-scheduler --features mock-hardware -- -D warnings` exits 0
- [ ] `head -1 .forge/reports/P12-A2_plan.md` prints `# Plan Report: P12-A2`
- [ ] `grep "^## " .forge/reports/P12-A2_plan.md` returns exactly 12 section headings
- [ ] `wc -l .forge/reports/P12-A2_plan.md` returns a value greater than 40
