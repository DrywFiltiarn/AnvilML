# Implementation Report: P3-A10

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P3-A10                          |
| Phase         | 003 — anvilml-core              |
| Description   | anvilml-core: NodeTypeRegistry dynamic registry |
| Implemented   | 2026-06-28T21:35:00Z            |
| Status        | COMPLETE                        |

## Summary

Implemented a thread-safe `NodeTypeRegistry` in `anvilml-core` — a dynamic, `Arc`-shareable registry for `NodeTypeDescriptor` values keyed by type name. The registry uses `RwLock<HashMap>` for interior mutability, enabling all public methods to take `&self` (shared reference). Five tests cover empty registry, registration, replacement semantics, listing, and concurrent read/write access.

## Resolved Dependencies

None. The implementation uses only `std::collections::HashMap` and `std::sync::RwLock` from the Rust standard library. No new crates were added.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | crates/anvilml-core/src/node_registry.rs | New module: `NodeTypeRegistry` struct with `new()`, `register_all()`, `get()`, `list()`, `len()`, `is_empty()` methods, plus `Default` impl |
| MODIFY | crates/anvilml-core/src/lib.rs | Added `mod node_registry;` and `pub use node_registry::NodeTypeRegistry;` |
| CREATE | crates/anvilml-core/tests/node_registry_tests.rs | Integration tests: 5 tests covering empty registry, single registration, replacement, listing, and concurrency |
| MODIFY | crates/anvilml-core/Cargo.toml | Bump version 0.1.15 → 0.1.16 |
| MODIFY | docs/TESTS.md | Added 5 entries for new tests |

## Commit Log

```
 .forge/reports/P3-A10_plan.md                    | 156 +++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md                     |   6 +-
 .forge/state/state.json                          |  13 +-
 Cargo.lock                                       |   2 +-
 crates/anvilml-core/Cargo.toml                   |   2 +-
 crates/anvilml-core/src/lib.rs                   |   3 +
 crates/anvilml-core/src/node_registry.rs         |  99 ++++++++++++++
 crates/anvilml-core/tests/node_registry_tests.rs | 149 ++++++++++++++++++++++
 docs/TESTS.md                                    |  60 +++++++++
 9 files changed, 479 insertions(+), 11 deletions(-)
```

## Test Results

```
     Running tests/node_registry_tests.rs (target/debug/deps/node_registry_tests-3e140681002e5a82)

running 5 tests
test test_list_returns_all ... ok
test test_empty_registry_returns_none ... ok
test test_register_all_populates ... ok
test test_concurrent_get_during_register_all_does_not_deadlock ... ok
test test_register_all_replaces_prior_contents ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Full workspace test suite: 95 tests passed, 0 failed (all crates).

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
  → Finished `dev` profile [unoptimized + debuginfo] target(s) in 20.94s

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
  → Finished `dev` profile [unoptimized + debuginfo] target(s) in 21.80s

# 3. Real-hardware Linux
cargo check --bin anvilml
  → Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.06s

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
  → Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.59s
```

## Project Gates

- **Gate 1 (Config Surface Sync):** Not triggered — task does not modify `ServerConfig` or nested config structs.
- **Gate 2 (OpenAPI Drift):** Not triggered — task does not modify handlers, `#[utoipa::path]` annotations, or `ToSchema` derives.
- **Gate 3 (Node Parity):** Triggered (modifies `node_registry.rs`) but requires Python worker venv (`worker/.venv`) which is not provisioned in this environment. CI will validate this gate.
- **Gate 4 (Mock/Real Parity Markers):** Not triggered — task does not add/modify a node's `execute()` or arch module's `load()`/`sample()`/`decode()`/`compute_latent_shape()`.

## Public API Delta

```
+pub use node_registry::NodeTypeRegistry;
```

New public items introduced by this task:

| Name | Type | Module Path |
|------|------|-------------|
| `NodeTypeRegistry` | struct | `anvilml_core::node_registry::NodeTypeRegistry` |
| `NodeTypeRegistry::new` | fn | `anvilml_core::NodeTypeRegistry::new` |
| `NodeTypeRegistry::register_all` | fn | `anvilml_core::NodeTypeRegistry::register_all` |
| `NodeTypeRegistry::get` | fn | `anvilml_core::NodeTypeRegistry::get` |
| `NodeTypeRegistry::list` | fn | `anvilml_core::NodeTypeRegistry::list` |
| `NodeTypeRegistry::len` | fn | `anvilml_core::NodeTypeRegistry::len` |
| `NodeTypeRegistry::is_empty` | fn | `anvilml_core::NodeTypeRegistry::is_empty` |
| `NodeTypeRegistry::default` | impl | `anvilml_core::NodeTypeRegistry::default` |

All match the plan's `## Public API Surface` declaration.

## Deviations from Plan

- **Clippy fixes:** The plan did not anticipate two clippy warnings on the initial implementation:
  1. `new_without_default` — added `impl Default for NodeTypeRegistry { fn default() -> Self { Self::new() } }` as recommended by clippy.
  2. `len_without_is_empty` — added `pub fn is_empty(&self) -> bool` as required by the `len_without_is_empty` lint.
- **Test fix — moved value:** The `test_register_all_replaces_prior_contents` test initially moved `desc_b` into `register_all` then tried to use it in an assertion. Fixed by cloning `desc_b` before the call (`desc_b_expected = desc_b.clone()`).
- **Test fix — unused imports:** Removed unused `SlotDescriptor` and `SlotType` imports from the test file after clippy flagged them.

## Blockers

None.
