# Plan Report: P12-A1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P12-A1                                      |
| Phase       | 12 — Graph Validation                       |
| Description | anvilml-scheduler: ValidatedGraph newtype (construction-gated) |
| Depends on  | P11-E1                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-07-06T16:20:00Z                        |
| Attempt     | 1                                           |

## Objective

Create the `ValidatedGraph` newtype in `crates/anvilml-scheduler/src/types.rs`, a wrapper around `serde_json::Value` that represents "a graph that has passed every validation check." The inner field is `pub(crate)` so code within the crate can inspect the graph, but there is no public bypass constructor — the only way to obtain a `ValidatedGraph` from outside this crate is through `validate_graph()` (implemented in P12-A3). Declare `mod types;` and re-export `ValidatedGraph` in `lib.rs`. Ship ≥2 integration tests in `dag_tests.rs` proving the construction gating works.

## Scope

### In Scope
- Create `crates/anvilml-scheduler/src/types.rs` with `pub struct ValidatedGraph(pub(crate) serde_json::Value)`.
- Add `#[derive(Debug, Clone)]` to `ValidatedGraph` (consistent with other domain types in this crate).
- Add `mod types;` and `pub use types::ValidatedGraph;` to `crates/anvilml-scheduler/src/lib.rs`.
- Create `crates/anvilml-scheduler/tests/dag_tests.rs` with ≥2 integration tests.
- Bump `anvilml-scheduler` patch version in `Cargo.toml` (0.1.0 → 0.1.1).

### Out of Scope
- `GraphError` enum — defined in P12-A2.
- `validate_graph()` function — implemented across P12-A3 through P12-A6.
- Any `From<serde_json::Value>` or `Into<serde_json::Value>` impl — these would be bypass constructors and are explicitly forbidden.
- `lib.rs` re-export pass for additional types — handled in P12-B1.
- `dag.rs` module — created in P12-A3.

## Existing Codebase Assessment

`anvilml-scheduler` exists as a stub crate (created in Phase 1's P1-B5). Its `lib.rs` contains only a crate-level doc comment (`//! Job queue, VRAM ledger, DAG validation, and dispatch loop.`) with no module declarations or re-exports. No `types.rs`, `dag.rs`, or test files exist yet. The crate's `Cargo.toml` declares dependencies on `anvilml-worker`, `anvilml-registry`, `anvilml-artifacts`, `anvilml-core`, and `anvilml-hardware` — all path dependencies within this workspace, with no direct `serde_json` dependency. However, `anvilml-core` depends on `serde_json = "1.0"` (per its own `Cargo.toml`), so `serde_json::Value` is available transitively to `anvilml-scheduler`.

The established crate pattern (confirmed by reading `anvilml-core/src/lib.rs`, `anvilml-ipc/src/lib.rs`, and `anvilml-registry/src/lib.rs`) is: a single-line crate-level `//!` doc comment, `pub mod` declarations for each submodule, and `pub use` re-exports of public types. `lib.rs` never exceeds 80 lines and contains zero implementation code.

`anvilml-core`'s `types/node.rs` defines `NodeTypeDescriptor`, `SlotDescriptor`, and `SlotType` — types that `ValidatedGraph` will eventually be validated against, but which this task does not reference directly.

## Resolved Dependencies

| Type   | Name        | Version verified | MCP source     | Feature flags confirmed |
|--------|-------------|-----------------|----------------|------------------------|
| crate  | serde_json  | 1.0 (transitive via anvilml-core) | rust-docs MCP (confirmed: anvilml-core declares `serde_json = "1.0"`) | n/a |

Note: `serde_json` is not a direct dependency of `anvilml-scheduler` — it comes transitively through `anvilml-core`. The plan adds `serde_json = "1.0"` as a direct dependency in `anvilml-scheduler/Cargo.toml` to make the dependency explicit (good practice for a crate whose primary type is `serde_json::Value`), though the transitive path would also work.

## Approach

1. **Add `serde_json` as a direct dependency** in `crates/anvilml-scheduler/Cargo.toml`. Add `serde_json = "1.0"` to the `[dependencies]` section. This makes the dependency explicit — the crate's primary type wraps `serde_json::Value`, so the dependency should be visible at the crate level.

2. **Create `crates/anvilml-scheduler/src/types.rs`** with the `ValidatedGraph` newtype:
   ```rust
   /// A graph that has passed every validation check.
   ///
   /// This is a construction-gated newtype: the only way to obtain a
   /// `ValidatedGraph` from outside this crate is a successful call to
   /// `validate_graph()` (implemented in dag.rs, P12-A3). The inner
   /// `serde_json::Value` field is `pub(crate)` so code within the
   /// crate can inspect the validated graph, but there is no public
   /// bypass constructor.
   #[derive(Debug, Clone)]
   pub struct ValidatedGraph(pub(crate) serde_json::Value);
   ```
   - The struct is `pub` (visible outside the crate) but has no `pub` methods or `impl` blocks — this task only declares the type.
   - The inner field is `pub(crate)` — accessible within the crate, not from external crates.
   - No `From<serde_json::Value>`, no `Into<serde_json::Value>`, no `Deref` impl, no `AsRef` impl — these would all be bypass constructors.
   - Derives `Debug` and `Clone` for consistency with other types in this crate's domain (e.g., `NodeTypeDescriptor` derives `Debug, Clone`).

3. **Update `crates/anvilml-scheduler/src/lib.rs`** to declare the module and re-export the type:
   ```rust
   //! Job queue, VRAM ledger, DAG validation, and dispatch loop.

   pub mod types;
   pub use types::ValidatedGraph;
   ```
   - `mod types;` is not `pub` — the module itself is internal, only the type it exports is re-exported publicly.
   - `pub use types::ValidatedGraph;` makes `ValidatedGraph` available as `anvilml_scheduler::ValidatedGraph`.

4. **Create `crates/anvilml-scheduler/tests/dag_tests.rs`** with ≥2 integration tests. Since `ValidatedGraph` has no public constructor at this point (only `pub(crate)` inner field, no impl), the tests verify construction gating by:
   - **Test 1 — `test_validated_graph_inner_is_pub_crate`:** Uses a same-crate helper function (defined in `types.rs` as `impl ValidatedGraph { pub(crate) fn inner(&self) -> &serde_json::Value { &self.0 } }`) to demonstrate that code within the crate can access the inner value. The helper is `pub(crate)` — visible to this test file (which is a test crate compiled with `--extern anvilml_scheduler = ...` and linked against the library). This confirms the `pub(crate)` visibility is correct: same-crate code accesses the field through the helper, proving the field is not `pub` (no direct field access from tests) but is accessible within the crate boundary.
   - **Test 2 — `test_validated_graph_has_no_public_constructor`:** Attempts to construct `ValidatedGraph` using only its public API surface. Since there is no `pub fn new()` or `From<serde_json::Value>` impl, the test compiles by confirming the type exists and derives `Debug` (calls `.to_string()` on a debug-formatted value, which requires `Debug` derive). This verifies the type is constructible within the crate (via the `pub(crate)` field) but not from external code.

   Wait — the test file is an integration test crate. It has access to the public API of `anvilml-scheduler` but NOT to `pub(crate)` items. So:
   - The test CANNOT access `ValidatedGraph.0` directly (it's `pub(crate)`, not `pub`).
   - The test CANNOT construct `ValidatedGraph(...)` because there is no `pub fn new()` or `From` impl.
   - The test CAN call `pub(crate)` methods on `ValidatedGraph` IF the test crate is considered "within the crate" — but integration test crates are NOT within the crate. They are separate crates that link against the library.

   This is a critical distinction. In Rust, `pub(crate)` visibility means "visible within this crate." An integration test crate (`tests/dag_tests.rs`) is a SEPARATE crate that depends on the library crate — it does NOT have `pub(crate)` access.

   So the correct approach for the tests is:
   - **Test 1 — `test_validated_graph_is_debug_clone`:** Construct `ValidatedGraph` via a `pub(crate)` helper exposed through a `pub` method, or simply verify the type exists and derives the expected traits. Since we can't construct it publicly, we need to expose a way to construct it for testing. The task says "no public bypass constructor," so we should NOT add a `pub fn new()`. Instead, we add a `pub(crate)` constructor that integration tests cannot use, and rely on the compile-time check.

   Actually, let me re-read the acceptance criteria:
   > >=2 tests in crates/anvilml-scheduler/tests/dag_tests.rs: ValidatedGraph's inner field is not constructible from outside the crate (a same-crate test confirms the pub(crate) visibility compiles as expected within the crate, demonstrating no external bypass exists)

   The key phrase is "a same-crate test confirms." This means the test must be a **unit test** (inline `#[cfg(test)]` block) or a test that runs WITHIN the crate's compilation unit. Integration tests (`tests/*.rs`) are separate crates and don't have `pub(crate)` access.

   But the task says the tests go in `dag_tests.rs` which is in `tests/`. So either:
   (a) The task expects unit tests inline in `types.rs` (but that contradicts the file path), or
   (b) The task considers the test crate as "same-crate" for visibility purposes (it's not — Rust doesn't work that way).

   The correct interpretation: the task wants `>=2 tests` in `dag_tests.rs`. Since integration tests don't have `pub(crate)` access, we need to provide a way for them to construct `ValidatedGraph`. The cleanest approach is to add a `pub(crate)` constructor and a `pub(crate)` inner accessor — both visible to code within the crate. For the integration test to use these, we expose them through a `#[cfg(test)]` conditional in `types.rs`:

   ```rust
   impl ValidatedGraph {
       /// Construct a ValidatedGraph from a serde_json::Value.
       ///
       /// This is pub(crate) — the only way to construct a ValidatedGraph
       /// from outside types.rs is through this method, which is visible
       /// only within the crate. The validate_graph() function (P12-A3)
       /// will be the sole consumer of this constructor in production.
       #[cfg(test)]
       pub fn _test_new(value: serde_json::Value) -> Self {
           Self(value)
       }

       /// Access the inner value. pub(crate) — for same-crate inspection.
       #[cfg(test)]
       pub fn _test_inner(&self) -> &serde_json::Value {
           &self.0
       }
   }
   ```

   This is the standard pattern: `#[cfg(test)]` conditional methods that are `pub` (visible to test crates) only when tests are compiled. The methods are internal implementation details (prefixed with `_test_`) and are not part of the public API surface.

   So the final approach for tests:
   - **Test 1 — `test_validated_graph_inner_is_pub_crate`:** Uses `ValidatedGraph::_test_new(json_value)._test_inner()` to confirm the inner value is accessible within the crate (via the test conditional). This proves `pub(crate)` works — same-crate code (including `#[cfg(test)]` code) can access the field.
   - **Test 2 — `test_validated_graph_derives_debug_and_clone`:** Uses `_test_new` to construct a value, then calls `.to_string()` on `format!("{:?}", ...)` to confirm `Debug` derive works. Also clones the value and confirms the clone is equal. This verifies the derives are correct.

   Both tests use only the `#[cfg(test)]`-gated constructors, confirming the `pub(crate)` gating works correctly in production (no `#[cfg(test)]` means no test helpers available).

5. **Bump `anvilml-scheduler` patch version** in `Cargo.toml` from `0.1.0` to `0.1.1` (per §12 of ENVIRONMENT.md and §14 of FORGE_AGENT_RULES.md — every task modifying source files bumps the patch version).

## Public API Surface

| Item | Crate/Module Path | Description |
|------|-------------------|-------------|
| `pub struct ValidatedGraph(pub(crate) serde_json::Value)` | `anvilml-scheduler/src/types.rs` | Newtype wrapper; no fields beyond wrapped Value. Inner field is `pub(crate)`, not `pub`. No `pub` methods or impl blocks in this task. |
| `pub use types::ValidatedGraph` | `anvilml-scheduler/src/lib.rs` | Re-export so consumers use `anvilml_scheduler::ValidatedGraph`. |
| `pub(crate) mod types` | `anvilml-scheduler/src/types.rs` | Module is not `pub` — only the re-exported type is public. |

Test-only (gated by `#[cfg(test)]`, not part of the production API):
- `pub fn _test_new(serde_json::Value) -> Self` — same-crate construction for tests.
- `pub fn _test_inner(&self) -> &serde_json::Value` — same-crate inner access for tests.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-scheduler/src/types.rs` | `ValidatedGraph` newtype definition with `pub(crate)` inner field. |
| MODIFY | `crates/anvilml-scheduler/src/lib.rs` | Add `pub mod types;` and `pub use types::ValidatedGraph;`. |
| MODIFY | `crates/anvilml-scheduler/Cargo.toml` | Add `serde_json = "1.0"` dependency; bump patch version 0.1.0 → 0.1.1. |
| CREATE | `crates/anvilml-scheduler/tests/dag_tests.rs` | ≥2 integration tests verifying construction gating. |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-scheduler/tests/dag_tests.rs` | `test_validated_graph_inner_is_pub_crate` | The inner `serde_json::Value` field is accessible within the crate via the `#[cfg(test)] pub fn _test_inner()` method, confirming `pub(crate)` visibility works correctly. Same-crate code can inspect the graph; external code cannot. | `ValidatedGraph` exists with `#[cfg(test)]` test helpers. | A `serde_json::json!({"nodes": []})` value wrapped in `ValidatedGraph`. | `_test_inner()` returns a reference to the same `serde_json::Value`. | `cargo test -p anvilml-scheduler --test dag_tests test_validated_graph_inner_is_pub_crate` exits 0 |
| `crates/anvilml-scheduler/tests/dag_tests.rs` | `test_validated_graph_derives_debug_and_clone` | `ValidatedGraph` correctly derives `Debug` and `Clone`. The Debug output includes the inner value's debug representation; cloning produces an equal value. | `ValidatedGraph` exists with `#[cfg(test)]` test helpers. | A `serde_json::json!({"nodes": []})` value. | `format!("{:?}", ...)` produces a non-empty string containing "ValidatedGraph"; clone equals original. | `cargo test -p anvilml-scheduler --test dag_tests test_validated_graph_derives_debug_and_clone` exits 0 |

## CI Impact

No CI changes required. The new test file `crates/anvilml-scheduler/tests/dag_tests.rs` is a standard Rust integration test crate — it will be automatically discovered and run by `cargo test --workspace --features mock-hardware` (Step 6 of ENVIRONMENT.md §6), which is already part of the CI matrix (`rust-linux` and `rust-windows` jobs). No new file type, gate, or test module convention is introduced.

## Platform Considerations

None identified. The `ValidatedGraph` newtype wraps a `serde_json::Value` which is platform-neutral. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are required. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `serde_json` is not a direct dependency of `anvilml-scheduler` — it comes transitively through `anvilml-core`. Without adding it explicitly, `types.rs` may fail to compile if `serde_json`'s re-export is not exposed. | Low | Medium | Add `serde_json = "1.0"` as a direct dependency in `anvilml-scheduler/Cargo.toml`. Verify at ACT time by running `cargo check -p anvilml-scheduler`. |
| Integration test crate (`tests/dag_tests.rs`) does not have `pub(crate)` access — it is a separate crate. The `#[cfg(test)]` helper methods must be `pub` (not `pub(crate)`) to be callable from the test crate. | Medium | High | Use `pub fn _test_new(...)` and `pub fn _test_inner(...)` gated by `#[cfg(test)]` — these are `pub` only when tests are compiled, satisfying both the test's access needs and the "no public bypass in production" requirement. |
| The `#[cfg(test)]` test helpers (`_test_new`, `_test_inner`) may be flagged as dead code by clippy in non-test builds. | Low | Low | Add `#[allow(dead_code)]` with an inline comment explaining the `#[cfg(test)]` gating, or use `#[cfg_attr(test, allow(dead_code))]` on the impl block. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-scheduler --test dag_tests` exits 0 (≥2 tests run and pass)
- [ ] `wc -l crates/anvilml-scheduler/src/lib.rs` reports ≤ 80 lines
- [ ] `grep -c "^## " .forge/reports/P12-A1_plan.md` reports 12 (all required sections present)
- [ ] `head -1 .forge/reports/P12-A1_plan.md` prints `# Plan Report: P12-A1`
