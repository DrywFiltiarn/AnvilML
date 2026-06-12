# Plan Report: P21-A6

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P21-A6                                      |
| Phase       | 021 — Real Python Worker — ZiT              |
| Description | worker: parity test KNOWN_NODE_TYPES == NODE_REGISTRY |
| Depends on  | P21-A5                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-06-13T00:25:00Z                        |
| Attempt     | 1                                           |

## Objective

Create a shared JSON fixture listing the nine canonical node-type names, then write two parity tests (one Python, one Rust) that independently load the fixture and assert that the set of registered Python node types (`NODE_REGISTRY.keys()`) and the set of Rust-known node types (`KNOWN_NODE_TYPES`) both match the fixture exactly. This is the contract described in ANVILML_DESIGN.md §14.3 and §20: any divergence between the Rust constant and the Python registry is caught at test time.

## Scope

### In Scope
- Create `backend/tests/known_node_types.json` — a JSON array of the 9 node-type names (identical to the `KNOWN_NODE_TYPES` constant in `crates/anvilml-scheduler/src/nodes.rs`).
- Create `worker/tests/test_parity.py` — a pytest test that loads the JSON via `__file__`-relative path and asserts `set(NODE_REGISTRY.keys()) == set(json)`.
- Add a parity test function to `crates/anvilml-scheduler/src/nodes.rs` — reads the same JSON file from a relative path (`../../backend/tests/known_node_types.json`), parses it, and asserts equality with `KNOWN_NODE_TYPES`.
- Bump `anvilml-scheduler` crate patch version from `0.1.18` to `0.1.19` (modifies source file per FORGE_AGENT_RULES §12).

### Out of Scope
- Changes to any node implementation files (`worker/nodes/zit.py`, `worker/nodes/sdxl.py`, `worker/nodes/common.py`).
- Changes to `KNOWN_NODE_TYPES` or `NODE_SLOTS` in `nodes.rs` (already correct with 9 entries).
- Changes to CI workflow files (new pytest file is auto-discovered; new Rust test is auto-discovered by `cargo test`).
- Changes to `worker/nodes/__init__.py` or `worker/nodes/base.py`.

## Approach

1. **Create `backend/tests/known_node_types.json`**
   - Write a JSON array of 9 strings, one per line for readability:
     ```json
     [
       "ZitLoadPipeline",
       "ZitTextEncode",
       "ZitSampler",
       "ZitDecode",
       "SdxlLoadPipeline",
       "SdxlTextEncode",
       "SdxlSampler",
       "SdxlDecode",
       "SaveImage"
     ]
     ```
   - These are the exact 9 names from `KNOWN_NODE_TYPES` in `crates/anvilml-scheduler/src/nodes.rs` (line 2–12).

2. **Create `worker/tests/test_parity.py`**
   - Import `json`, `from pathlib import Path`, and `from worker.nodes.base import NODE_REGISTRY`.
   - Resolve the JSON file path relative to the test file: `Path(__file__).parent.parent / "backend" / "tests" / "known_node_types.json"`.
   - In `test_node_parity`, load the JSON, parse it as a list of strings, and assert:
     ```python
     assert set(NODE_REGISTRY.keys()) == set(json_data), (
         f"NODE_REGISTRY keys {sorted(NODE_REGISTRY.keys())} "
         f"do not match known_node_types.json {sorted(json_data)}"
     )
     ```
   - The `NODE_REGISTRY` is already populated at module import time by `worker/nodes/__init__.py` (auto-imports all node modules). No fixture cleanup needed since this test reads the live registry and the JSON fixture never changes during a test run.

3. **Add Rust parity test to `crates/anvilml-scheduler/src/nodes.rs`**
   - Add a new test function `test_node_parity` inside the existing `#[cfg(test)] mod tests` block.
   - The test reads `../../backend/tests/known_node_types.json` using `std::fs::read_to_string` with a path computed from `env!("CARGO_MANIFEST_DIR")`:
     ```rust
     let json_path = Path::new(env!("CARGO_MANIFEST_DIR"))
         .parent()
         .and_then(|p| p.parent())
         .unwrap()
         .join("backend")
         .join("tests")
         .join("known_node_types.json");
     let content = std::fs::read_to_string(&json_path)
         .expect("known_node_types.json must exist at backend/tests/");
     let json_values: Vec<String> = serde_json::from_str(&content)
         .expect("known_node_types.json must be a valid JSON array of strings");
     let rust_set: std::collections::HashSet<&str> = KNOWN_NODE_TYPES.iter().copied().collect();
     let json_set: std::collections::HashSet<&str> = json_values.iter().map(|s| s.as_str()).collect();
     assert_eq!(rust_set, json_set, ...);
     ```
   - This test uses `serde_json` which is already a dependency of `anvilml-scheduler` (line 13 of Cargo.toml). No new dependencies needed.

4. **Bump `anvilml-scheduler` crate version**
   - Read `crates/anvilml-scheduler/Cargo.toml`: current version is `0.1.18`.
   - Update to `0.1.19` (patch bump per FORGE_AGENT_RULES §12).

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Create | `backend/tests/known_node_types.json` | JSON fixture: 9 node-type names array |
| Create | `worker/tests/test_parity.py` | Python parity test: NODE_REGISTRY vs JSON |
| Modify | `crates/anvilml-scheduler/src/nodes.rs` | Add `test_node_parity` Rust test in existing `mod tests` |
| Modify | `crates/anvilml-scheduler/Cargo.toml` | Bump version `0.1.18 → 0.1.19` |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `worker/tests/test_parity.py` | `test_node_parity` | `set(NODE_REGISTRY.keys()) == set(json)` — Python registry matches fixture |
| `crates/anvilml-scheduler/src/nodes.rs` | `test_node_parity` | `KNOWN_NODE_TYPES` (Rust) matches JSON fixture — Rust constant matches fixture |

## CI Impact

No CI workflow file changes. The new pytest file is auto-discovered by `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/ -v`. The new Rust test is auto-discovered by `cargo test -p anvilml-scheduler -- parity`. Both commands are already part of the CI gates documented in ENVIRONMENT.md §6 and ARCHITECTURE.md §9.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| JSON file path resolution differs between Python and Rust | Low | Medium | Both paths are computed relative to their own file/module location (`__file__` for Python, `CARGO_MANIFEST_DIR` for Rust). Verified by running both tests locally before staging. |
| `NODE_REGISTRY` contains extra entries (e.g. test-only nodes) | Low | Medium | The parity test runs after all node modules are imported. If a test registers a node that leaks into the live registry, it would be caught. The existing `autouse` fixtures in other tests clear the registry, but `test_parity.py` runs independently and imports the clean module. |
| Rust test fails because `backend/tests/known_node_types.json` is not on the Rust compile-time path | Low | Low | The test uses a runtime path resolution (`std::fs::read_to_string`), not `include_str!`. The path is computed relative to `CARGO_MANIFEST_DIR`, which is reliable. |
| Version bump triggers unnecessary rebuild | N/A | N/A | Standard practice per FORGE_AGENT_RULES §12; only patch version changed. |

## Acceptance Criteria

- [ ] `backend/tests/known_node_types.json` exists and contains exactly 9 string elements matching `KNOWN_NODE_TYPES`
- [ ] `worker/tests/test_parity.py` exists and `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_parity.py -v` exits 0
- [ ] Rust test `test_node_parity` exists in `crates/anvilml-scheduler/src/nodes.rs` and `cargo test -p anvilml-scheduler -- parity` exits 0
- [ ] `anvilml-scheduler` crate version bumped from `0.1.18` to `0.1.19`
- [ ] `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/ -v` exits 0 (no regressions from other worker tests)
- [ ] `cargo test -p anvilml-scheduler --features mock-hardware` exits 0 (no regressions from other scheduler tests)
