# Plan Report: P11-A2

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P11-A2                                            |
| Phase       | 011 — Graph Validation                            |
| Description | anvilml-scheduler: dag.rs duplicate-id + unknown-type checks |
| Depends on  | P11-A1                                            |
| Project     | anvilml                                           |
| Planned at  | 2026-06-07T09:20:00Z                              |
| Attempt     | 1                                                 |

## Objective

Create `crates/anvilml-scheduler/src/dag.rs` implementing a `ValidatedGraph(newtype(Value))` and a `validate_graph(&Value) -> Result<ValidatedGraph, Vec<String>>` function that collects all validation errors (non-fail-fast): duplicate node IDs and unknown node types not present in the `KNOWN_NODE_TYPES` set established by P11-A1.

## Scope

### In Scope
- New file: `crates/anvilml-scheduler/src/dag.rs`
- `ValidatedGraph` newtype wrapping `serde_json::Value`
- `validate_graph(v: &Value) -> Result<ValidatedGraph, Vec<String>>` with two checks:
  - Duplicate node id → `'duplicate_node_id: {id}'` error string
  - Unknown node type (not in `KNOWN_NODE_TYPES`) → `'unknown_node_type: {type}'` error string
- All errors collected and returned together (non-fail-fast)
- Update `lib.rs` to export the new module
- Add `serde_json` workspace dependency to `Cargo.toml` (currently missing from this crate)
- Unit tests in `dag.rs`: duplicate-id test, unknown-type test, valid-graph test
- Bump `anvilml-scheduler` patch version `0.1.1 → 0.1.2`

### Out of Scope
- Edge-reference validation (P11-A3)
- Cycle detection via Kahn's algorithm (P11-A4)
- HTTP handler integration (P11-A5)
- Any changes to `anvilml-server` or other crates
- Logging instrumentation (DAG validation is a pure function called at request time; logging will be added in P11-A5 when the handler wires it into the request lifecycle)

## Approach

1. **Add `serde_json` dependency** — Open `Cargo.toml` for `anvilml-scheduler`; add `serde_json = { workspace = true }` to `[dependencies]`. This is required because `ValidatedGraph` wraps `serde_json::Value` and `validate_graph` accepts `&Value`.

2. **Create `src/dag.rs`** with the following structure:
   - Import `serde_json::Value` and re-export `KNOWN_NODE_TYPES` from the `nodes` module via `super::nodes::KNOWN_NODE_TYPES`.
   - Define `pub struct ValidatedGraph(pub Value);` — a zero-cost newtype proving the graph passed validation.
   - Implement `validate_graph(v: &Value) -> Result<ValidatedGraph, Vec<String>>`:
     a. Assert that `v` is an object (`Value::Object`). If not, return early with `["invalid_graph: expected object"]`.
     b. Extract the `"nodes"` field; if absent or not an array, return error.
     c. Iterate nodes collecting errors:
        - Track seen IDs in a temporary `Vec<&str>` (or `HashSet`); if an ID repeats, push `'duplicate_node_id: {id}'`.
        - Check each node's `"type"` field against `KNOWN_NODE_TYPES`; if absent or not found, push `'unknown_node_type: {type}'`.
     d. If any errors were collected, return `Err(errors)`.
     e. Otherwise return `Ok(ValidatedGraph(v.clone()))`.
   - Add `#[cfg(test)]` module with three tests:
     1. `test_duplicate_node_id` — submit a graph with two nodes sharing `"id": "n0"`; expect `Err` containing `'duplicate_node_id: n0'`.
     2. `test_unknown_node_type` — submit a graph with one node having `"type": "NopeNode"`; expect `Err` containing `'unknown_node_type: NopeNode'`.
     3. `test_valid_graph` — submit a minimal valid two-node ZiT graph (`ZitLoadPipeline` → `ZitTextEncode`); expect `Ok(ValidatedGraph)`.

3. **Update `lib.rs`** — add `pub mod dag;` and re-export `dag::ValidatedGraph` and `dag::validate_graph`.

4. **Run verification** — execute `cargo test -p anvilml-scheduler -- dag_basic` (or the actual test names) to confirm all three tests pass, then `cargo clippy --package anvilml-scheduler --features mock-hardware -- -D warnings`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-scheduler/Cargo.toml` | Add `serde_json = { workspace = true }` dependency; bump patch version `0.1.1 → 0.1.2` |
| Create | `crates/anvilml-scheduler/src/dag.rs` | `ValidatedGraph` newtype, `validate_graph()` function, unit tests |
| Modify | `crates/anvilml-scheduler/src/lib.rs` | Export `pub mod dag;` and re-export `ValidatedGraph`, `validate_graph` |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `crates/anvilml-scheduler/src/dag.rs` | `test_duplicate_node_id` | Two nodes with same `"id"` produce `'duplicate_node_id: n0'` error |
| `crates/anvilml-scheduler/src/dag.rs` | `test_unknown_node_type` | Node with unknown type produces `'unknown_node_type: NopeNode'` error |
| `crates/anvilml-scheduler/src/dag.rs` | `test_valid_graph` | A valid minimal ZiT graph (2 nodes) returns `Ok(ValidatedGraph)` |

## CI Impact

No CI workflow files are modified. The new `serde_json` dependency is already declared in the workspace `Cargo.toml`, so no lockfile changes beyond standard cargo resolution. All existing CI gates (format, clippy, test, cross-check) apply to the full workspace as usual. The `anvilml-scheduler` crate's tests are included in `cargo test --workspace --features mock-hardware`.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `serde_json` not previously a dependency of `anvilml-scheduler`, may need to be added with correct workspace reference | Low | Medium | Workspace dependency already declared at `1.0.150`; use `{ workspace = true }` syntax |
| Tests named in task context (`dag_basic`) don't match actual test function names | Low | Low | Task says "cargo test -p anvilml-scheduler -- dag_basic exits 0" — this is a filter pattern; `test_duplicate_node_id` contains no substring `dag_basic`. Plan uses correct test names. If the intent is a module-level or file-level filter, tests will be in `dag.rs` and filtered via `-- dag` |
| Non-fail-fast error collection requires careful iteration to not short-circuit | Medium | Low | Straightforward single-pass over nodes with a `Vec<String>` accumulator; no early returns on individual node errors |

## Acceptance Criteria

- [ ] `crates/anvilml-scheduler/src/dag.rs` exists with `ValidatedGraph` newtype and `validate_graph` function
- [ ] Duplicate node IDs produce `'duplicate_node_id: {id}'` error string
- [ ] Unknown node types produce `'unknown_node_type: {type}'` error string
- [ ] Multiple errors are collected and returned together (non-fail-fast)
- [ ] Valid graph returns `Ok(ValidatedGraph)`
- [ ] `cargo test -p anvilml-scheduler --features mock-hardware` exits 0 with all tests passing
- [ ] `cargo clippy --package anvilml-scheduler --features mock-hardware -- -D warnings` exits 0
- [ ] `anvilml-scheduler` patch version bumped from `0.1.1` to `0.1.2` in `Cargo.toml`
