# Plan Report: P3-A10

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P3-A10                                      |
| Phase       | 003 — Core Domain Types: Data Model         |
| Description | anvilml-core: NodeTypeRegistry dynamic registry |
| Depends on  | P3-A9                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-06-28T19:15:00Z                        |
| Attempt     | 1                                           |

## Objective

Create `crates/anvilml-core/src/node_registry.rs` implementing `NodeTypeRegistry` — a thread-safe, in-memory registry keyed by `type_name` that stores `NodeTypeDescriptor` values. The registry is populated exclusively at runtime from worker `Ready` events (never a hardcoded compile-time list), and provides `register_all()`, `get()`, `list()`, `len()`, and `new()` methods backed by `RwLock<HashMap<String, NodeTypeDescriptor>>`. This type is consumed by the scheduler's graph validator (P3-A12+), the server's `GET /v1/nodes` handler, and the demux in the worker pool.

## Scope

### In Scope
- Create `crates/anvilml-core/src/node_registry.rs` with `NodeTypeRegistry` struct and all five methods (`new`, `register_all`, `get`, `list`, `len`).
- Declare `mod node_registry;` and `pub use node_registry::NodeTypeRegistry;` in `crates/anvilml-core/src/lib.rs`.
- Create `crates/anvilml-core/tests/node_registry_tests.rs` with ≥5 tests covering: empty registry returns `None`/0, `register_all` populates, `register_all` replaces prior contents, `list()` returns all descriptors, concurrent `get()` during `register_all()` does not deadlock.
- Bump `anvilml-core` patch version from `0.1.15` to `0.1.16` in `crates/anvilml-core/Cargo.toml`.

### Out of Scope
None — `defers_to (from JSON): []`. This task implements its full scope with no deferrals.

## Existing Codebase Assessment

`anvilml-core` is a pure-data crate with zero I/O, zero async, and no external dependencies beyond `serde`, `uuid`, `chrono`, `utoipa`, `thiserror`, `axum`, `sqlx`, and `toml`. The `types/` submodule already defines `NodeTypeDescriptor`, `SlotDescriptor`, and `SlotType` (completed in P3-A7), each with `#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]` and full `///` doc comments. The `types/node.rs` file uses `utoipa::ToSchema` for OpenAPI annotations.

The established test pattern in `crates/anvilml-core/tests/` uses plain `#[test]` functions with `///` doc comments describing the assertion. Integration tests import via `use anvilml_core::...`. The `serial_test` crate is already a dev-dependency. No existing test file uses `std::sync::RwLock` or `std::thread::spawn` — this task will be the first to exercise concurrency primitives in the crate.

The design doc (§12.2) specifies `NodeTypeRegistry` with `Arc<RwLock<...>>` and `async` methods, but the task context specifies a synchronous `RwLock<...>` (no `Arc`, no `async`). The synchronous shape is correct for `anvilml-core`'s hard constraint of zero async — the `Arc` wrapping and async methods are added later when the scheduler consumes this type via `Arc<NodeTypeRegistry>`. The plan follows the task's synchronous specification.

## Resolved Dependencies

None. `RwLock` and `HashMap` are both from the Rust standard library (`std::sync::RwLock`, `std::collections::HashMap`). No new external crate is introduced.

| Type | Name | Version verified | MCP source | Feature flags confirmed |
|------|------|-----------------|------------|------------------------|
| std  | RwLock | 1.96.0 (rust-toolchain.toml) | rustc toolchain pin | n/a |
| std  | HashMap | 1.96.0 (rust-toolchain.toml) | rustc toolchain pin | n/a |

## Approach

1. **Bump `anvilml-core` version** in `crates/anvilml-core/Cargo.toml`: change `version = "0.1.15"` to `version = "0.1.16"`. This follows the §12 bump convention — only the patch digit increments.

2. **Create `crates/anvilml-core/src/node_registry.rs`** with the following content:
   - Import `std::collections::HashMap` and `std::sync::RwLock`.
   - Import `NodeTypeDescriptor` from `crate::types::node`.
   - Define `pub struct NodeTypeRegistry { types: RwLock<HashMap<String, NodeTypeDescriptor>> }`.
   - Implement `NodeTypeRegistry::new() -> Self` — creates an empty `RwLock<HashMap::new>()`.
   - Implement `pub fn register_all(&self, descs: Vec<NodeTypeDescriptor>)` — takes `&self` (shared reference, not `&mut`), acquires a write lock via `self.types.write().unwrap()`, and assigns the new map. The `&self` receiver (not `&mut self`) is the key design choice: it allows the registry to be shared behind an `Arc` without requiring interior mutability beyond the `RwLock`, matching the eventual consumer pattern in `anvilml-scheduler` where the scheduler holds `Arc<NodeTypeRegistry>`. The `unwrap()` on the lock is acceptable here — a poisoned lock indicates a logic bug (a thread panicked while holding the lock), and panicking is the correct failure mode for such a bug in a pure-data crate.
   - Implement `pub fn get(&self, type_name: &str) -> Option<NodeTypeDescriptor>` — acquires a read lock, looks up `type_name` in the map, returns the value or `None`.
   - Implement `pub fn list(&self) -> Vec<NodeTypeDescriptor>` — acquires a read lock, collects all `NodeTypeDescriptor` values from the map into a `Vec`, returns it. The values are cloned (each `NodeTypeDescriptor` derives `Clone`).
   - Implement `pub fn len(&self) -> usize` — acquires a read lock, returns the map's `len()`.
   - Add `///` doc comments on the struct and every public method, following the project's documentation convention (describe what it does, preconditions, return values).

3. **Update `crates/anvilml-core/src/lib.rs`** — add two lines after the existing `pub mod types;` block:
   ```rust
   mod node_registry;
   pub use node_registry::NodeTypeRegistry;
   ```
   These are placed after the `types` module declarations to maintain alphabetical ordering of `mod` declarations (node_registry comes after mod error, before the end of the file).

4. **Create `crates/anvilml-core/tests/node_registry_tests.rs`** with the following tests:
   - `test_empty_registry_returns_none`: constructs `NodeTypeRegistry::new()`, asserts `get("NonExistent")` is `None` and `len()` is `0`.
   - `test_register_all_populates`: creates a descriptor, registers it via `register_all()`, asserts `get("LoadModel")` returns `Some(desc)`, `len()` is `1`, and `list()` contains exactly one element.
   - `test_register_all_replaces_prior_contents`: registers descriptor A, asserts `len()` is 1, registers descriptor B (replacing), asserts `get("A")` is `None` (old entry removed), `get("B")` is `Some`, and `len()` is 1. This verifies `register_all` replaces, not merges.
   - `test_list_returns_all`: registers three descriptors with distinct type names, asserts `list().len()` is `3`, and that each returned descriptor's `type_name` matches one of the registered names.
   - `test_concurrent_get_during_register_all_does_not_deadlock`: spawns a reader thread that calls `get()` in a tight loop (100 iterations) while the main thread calls `register_all()` once. Both complete within 2 seconds (a hard timeout on `join()`). No deadlock, no panic. Uses `std::thread::spawn` and `std::sync::Arc` to share the registry between threads.

5. **Run `cargo test -p anvilml-core --test node_registry_tests`** to verify all tests pass before writing the report.

## Public API Surface

```rust
// crates/anvilml-core/src/node_registry.rs

pub struct NodeTypeRegistry {
    types: RwLock<HashMap<String, NodeTypeDescriptor>>,
}

impl NodeTypeRegistry {
    /// Create a new, empty `NodeTypeRegistry`.
    pub fn new() -> Self;

    /// Replace the entire registry contents with `descs`.
    ///
    /// Called once per worker `Ready` event. Replaces, does not merge.
    pub fn register_all(&self, descs: Vec<NodeTypeDescriptor>);

    /// Look up a node type by its unique name.
    ///
    /// Returns `None` if no descriptor with that `type_name` is registered.
    pub fn get(&self, type_name: &str) -> Option<NodeTypeDescriptor>;

    /// Return all registered node type descriptors.
    ///
    /// The returned `Vec` contains one `NodeTypeDescriptor` per registered type.
    /// Order is not guaranteed.
    pub fn list(&self) -> Vec<NodeTypeDescriptor>;

    /// Return the number of registered node types.
    pub fn len(&self) -> usize;
}
```

Re-export in `lib.rs`:
```rust
pub use node_registry::NodeTypeRegistry;
```

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | crates/anvilml-core/src/node_registry.rs | NodeTypeRegistry struct and impl |
| MODIFY | crates/anvilml-core/src/lib.rs | Add `mod node_registry;` and `pub use` |
| CREATE | crates/anvilml-core/tests/node_registry_tests.rs | ≥5 integration tests |
| MODIFY | crates/anvilml-core/Cargo.toml | Bump version 0.1.15 → 0.1.16 |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| crates/anvilml-core/tests/node_registry_tests.rs | test_empty_registry_returns_none | `NodeTypeRegistry::new()` returns empty: `get("foo")` is `None`, `len()` is `0` | `cargo test -p anvilml-core --test node_registry_tests test_empty_registry_returns_none` exits 0 |
| crates/anvilml-core/tests/node_registry_tests.rs | test_register_all_populates | After `register_all([desc])`, `get("LoadModel")` returns `Some`, `len()` is `1`, `list()` has one element | `cargo test -p anvilml-core --test node_registry_tests test_register_all_populates` exits 0 |
| crates/anvilml-core/tests/node_registry_tests.rs | test_register_all_replaces_prior_contents | Register A then B: A is gone, B is present, `len()` is 1 — verifies replace-not-merge | `cargo test -p anvilml-core --test node_registry_tests test_register_all_replaces_prior_contents` exits 0 |
| crates/anvilml-core/tests/node_registry_tests.rs | test_list_returns_all | Register three descriptors: `list().len()` is 3, all three type_names present in results | `cargo test -p anvilml-core --test node_registry_tests test_list_returns_all` exits 0 |
| crates/anvilml-core/tests/node_registry_tests.rs | test_concurrent_get_during_register_all_does_not_deadlock | Reader thread calls `get()` in a loop while main thread calls `register_all()` — both complete within 2s, no deadlock | `cargo test -p anvilml-core --test node_registry_tests test_concurrent_get_during_register_all_does_not_deadlock` exits 0 |

## CI Impact

No CI changes required. The new test file is in `crates/anvilml-core/tests/`, which is automatically picked up by `cargo test --workspace --features mock-hardware` (Step 6 of ENVIRONMENT.md §6). The test crate has no new file-type requirements — it's a standard Rust integration test.

## Platform Considerations

None identified. The `RwLock`, `HashMap`, and `std::thread::spawn` APIs used are fully cross-platform. The concurrent test uses `std::thread::spawn` (not platform-specific threading APIs) and `std::thread::JoinHandle::join()` with a timeout via `thread::sleep` + `Arc<AtomicBool>` to detect completion. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `RwLock::write().unwrap()` panics if a thread holding the write lock panics, poisoning the lock. A subsequent `write()` or `read()` call on a poisoned `RwLock` will also panic. | Low | Medium | The only code path that holds the write lock is `register_all()`, which performs a single map assignment with no fallible operations. No user input or network I/O occurs inside the lock. The poison risk is effectively zero for this scope. If a future task adds fallible operations inside `register_all()`, the lock should use `write().expect("...)` with a descriptive message. |
| `list()` returns values in arbitrary `HashMap` iteration order, which may differ between calls. A test asserting a specific ordering would be flaky. | Low | Low | The `test_list_returns_all` test only checks that the count is correct and all three registered type_names appear in the result — it does not assert order. No ordering-dependent assertions exist. |
| `std::thread::spawn` in the concurrent test may fail on constrained environments (e.g. WSL2 with low thread limits) if the OS cannot create a new thread. | Very Low | Medium | The test spawns exactly one additional thread. WSL2 and Linux both support thousands of threads. The 2-second timeout on `join()` prevents indefinite hangs if the spawned thread fails to start. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-core --test node_registry_tests` exits 0 with ≥5 tests
- [ ] `wc -l crates/anvilml-core/src/node_registry.rs` reports a file with the struct, impl block, and doc comments (expected ~60–80 lines)
- [ ] `grep "^pub use node_registry::NodeTypeRegistry;" crates/anvilml-core/src/lib.rs` returns a match
- [ ] `grep "^mod node_registry;" crates/anvilml-core/src/lib.rs` returns a match
- [ ] `grep 'version = "0.1.16"' crates/anvilml-core/Cargo.toml` returns a match
