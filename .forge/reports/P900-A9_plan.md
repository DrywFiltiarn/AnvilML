# Plan Report: P900-A9

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P900-A9                                     |
| Phase       | 900 — Spec-Drift & Logging Retrofit         |
| Description | anvilml-core: fix EnvReport's field shape to match ANVILML_DESIGN.md |
| Depends on  | P900-A1, P3-A6                              |
| Project     | anvilml                                     |
| Planned at  | 2026-06-30T18:20:00Z                        |
| Attempt     | 1                                           |

## Objective

Rewrite `EnvReport` in `crates/anvilml-core/src/types/worker.rs` from its current 3-field shape (`python_version: String`, `torch_version: Option<String>`, `torch_importable: bool`) to the 7-field shape specified in `ANVILML_DESIGN.md` (§5.7): `python_path: Option<String>`, `python_version: Option<String>`, `torch_version: Option<String>`, `provisioning: ProvisioningState`, `preflight_ok: bool`, `reason: Option<String>`, `node_types: Vec<NodeTypeDescriptor>`. Update the roundtrip serde test in `worker_tests.rs` to exercise all 7 fields. This closes a spec-drift defect found by tracing Phase 3's `EnvReport` forward — two later tasks (`P18-A1`, `P28-B1`) already assume the doc-correct shape, and `P28-B1` will not compile against the current struct.

## Scope

### In Scope
- Rewrite `EnvReport` struct in `crates/anvilml-core/src/types/worker.rs` to match the 7-field design doc shape exactly.
- `python_version` changes from `String` to `Option<String>`.
- `torch_version` stays `Option<String>` (unchanged).
- `torch_importable: bool` is removed (replaced by `preflight_ok: bool` and `reason: Option<String>` in the new shape).
- Add `python_path: Option<String>`, `provisioning: ProvisioningState`, `preflight_ok: bool`, `reason: Option<String>`, `node_types: Vec<NodeTypeDescriptor>`.
- `ProvisioningState` is used as-is (its variant names are P900-A10's scope).
- Update `crates/anvilml-core/tests/worker_tests.rs`'s `test_env_report_serde_roundtrip` to construct all 7 fields and verify all 7 field names in the JSON output.
- Update the test's doc comment to reflect the 7-field shape.
- Increment `anvilml-core` patch version (0.1.19 → 0.1.20).

### Out of Scope
- `ProvisioningState` variant renaming (`InProgress` → `Provisioning`, `Complete` → `Ready`) — this is P900-A10's scope.
- Any consumer code that constructs or reads `EnvReport` fields — none exists in the codebase yet (EnvReport is defined but not consumed by any other crate).
- Adding `Default` impl for `EnvReport` — not specified in the design doc.
- Changes to `docs/TESTS.md` — the test catalogue entry will need updating (FORGE_AGENT_RULES §5.10), but that's covered by the test update.

defers_to (from JSON): []

## Existing Codebase Assessment

`EnvReport` is defined in `crates/anvilml-core/src/types/worker.rs` with 3 fields: `python_version: String`, `torch_version: Option<String>`, `torch_importable: bool`. It is a `pub` struct deriving `Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema`. It is re-exported via `types/mod.rs` → `pub use worker::*` → `lib.rs` → `pub use types::*`.

The `NodeTypeDescriptor` type (referenced by the new `node_types` field) already exists in `crates/anvilml-core/src/types/node.rs` with the correct shape: `type_name: String`, `display_name: String`, `category: String`, `description: String`, `inputs: Vec<SlotDescriptor>`, `outputs: Vec<SlotDescriptor>`. It derives all required traits.

The `ProvisioningState` enum exists in the same `worker.rs` file with 4 variants: `NotStarted`, `InProgress`, `Complete`, `Failed`. It derives `Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema` with `#[serde(rename_all = "snake_case")]`. This task uses it as-is; its variant names don't match the design doc, but fixing that is P900-A10's scope.

A roundtrip serde test exists in `crates/anvilml-core/tests/worker_tests.rs` (`test_env_report_serde_roundtrip`) that constructs `EnvReport` with all 3 current fields, serialises to JSON, roundtrips via `serde_json::from_str`, and asserts equality. The test also parses the JSON to verify the 3 field names appear. This test must be updated to exercise all 7 fields.

No other crate in the workspace currently constructs or reads `EnvReport` fields — it is defined but not yet consumed. This means the field shape change has no breaking impact on existing consumers, only on the test file.

## Resolved Dependencies

None. This task only rewrites an existing struct's fields using types already present in the same crate (`ProvisioningState`, `NodeTypeDescriptor`). No new external crate or feature flag is introduced.

## Approach

1. **Read and confirm current state.** Read `crates/anvilml-core/src/types/worker.rs` to confirm the current `EnvReport` definition (3 fields) and `ProvisioningState` definition (4 variants: `NotStarted`, `InProgress`, `Complete`, `Failed`). Confirm `NodeTypeDescriptor` exists in `node.rs` with the correct shape.

2. **Rewrite `EnvReport` struct.** In `crates/anvilml-core/src/types/worker.rs`, replace the current `EnvReport` struct (lines 63-72) with the design-doc-correct 7-field shape:

   ```rust
   /// Python runtime environment report collected at worker startup preflight.
   ///
   /// The scheduler uses this to verify the worker's Python and PyTorch
   /// environment, provisioning status, and the node types it supports.
   #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
   pub struct EnvReport {
       /// Path to the Python interpreter used by the worker (e.g. `./worker/.venv/bin/python3`).
       pub python_path: Option<String>,
       /// The Python interpreter version string (e.g. `"3.12.3"`), or `None` if unavailable.
       pub python_version: Option<String>,
       /// The PyTorch version string if `import torch` succeeded, or `None` if
       /// the import failed or torch is not installed.
       pub torch_version: Option<String>,
       /// The provisioning status of the worker's environment.
       pub provisioning: ProvisioningState,
       /// Whether all preflight checks passed (interpreter exists, Python version OK, torch importable).
       pub preflight_ok: bool,
       /// An optional human-readable reason when `preflight_ok` is `false`.
       pub reason: Option<String>,
       /// The node types registered by this worker, reported at startup.
       pub node_types: Vec<NodeTypeDescriptor>,
   }
   ```

   Key changes from the current shape:
   - `python_version: String` → `python_version: Option<String>` (the current `String` type is wrong per the design doc).
   - `torch_importable: bool` is removed. The boolean check is replaced by `preflight_ok: bool`, and the version string is already captured by `torch_version: Option<String>` (which is `None` when torch is not importable).
   - Three new fields added: `python_path`, `provisioning`, `preflight_ok`, `reason`, `node_types`.

   The doc comment is updated to describe all 7 fields. Each field gets a doc comment matching the style of other structs in the file (like `WorkerInfo`).

3. **Update the roundtrip test.** In `crates/anvilml-core/tests/worker_tests.rs`, replace the `test_env_report_serde_roundtrip` test function with a version that constructs `EnvReport` with all 7 fields set to non-default values, serialises to JSON, roundtrips, and verifies all 7 field names in the parsed JSON.

   The test should construct:
   ```rust
   let report = EnvReport {
       python_path: Some("/usr/bin/python3".to_string()),
       python_version: Some("3.12.3".to_string()),
       torch_version: Some("2.5.1".to_string()),
       provisioning: ProvisioningState::NotStarted,
       preflight_ok: true,
       reason: None,
       node_types: vec![NodeTypeDescriptor {
           type_name: "LoadModel".to_string(),
           display_name: "Load Model".to_string(),
           category: "loaders".to_string(),
           description: "Loads a model checkpoint.".to_string(),
           inputs: vec![],
           outputs: vec![],
       }],
   };
   ```

   The JSON field assertions should verify all 7 field names: `python_path`, `python_version`, `torch_version`, `provisioning`, `preflight_ok`, `reason`, `node_types`.

4. **Update the test doc comment.** The module-level doc comment at the top of `worker_tests.rs` already lists `EnvReport` — verify it's accurate (it references `ProvisioningState` roundtrip which is a separate test, so no change needed there).

5. **Bump the crate version.** In `crates/anvilml-core/Cargo.toml`, increment the patch version from `0.1.19` to `0.1.20`.

6. **Verify.** Run `cargo test -p anvilml-core --test worker_tests` and `cargo doc -p anvilml-core --no-deps` to confirm the changes compile and tests pass.

## Public API Surface

| Item | Path | Change |
|------|------|--------|
| `struct EnvReport` | `anvilml_core::types::EnvReport` | **Modified** — field list changed from 3 to 7 fields (see above). Derive list unchanged. Doc comment updated. |
| `pub use EnvReport` | `anvilml_core::EnvReport` (via `types::*` re-export) | No change to the re-export; the struct it re-exports has changed shape. |

No new `pub` items are introduced. No items are removed from the public API surface (the old fields are replaced by new ones on the same struct).

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-core/src/types/worker.rs` | Rewrite `EnvReport` struct to 7-field design-doc shape; update doc comments |
| Modify | `crates/anvilml-core/tests/worker_tests.rs` | Update `test_env_report_serde_roundtrip` for all 7 fields and new JSON assertions |
| Modify | `crates/anvilml-core/Cargo.toml` | Bump patch version 0.1.19 → 0.1.20 |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-core/tests/worker_tests.rs` | `test_env_report_serde_roundtrip` | `EnvReport` with all 7 fields set serialises to JSON, roundtrips to an equal value, and the JSON contains all 7 expected field names with correct types | None | `EnvReport` with all 7 fields populated (non-default values) | Roundtripped `EnvReport` equals original; JSON contains `python_path`, `python_version`, `torch_version`, `provisioning`, `preflight_ok`, `reason`, `node_types` keys with correct types | `cargo test -p anvilml-core --test worker_tests test_env_report_serde_roundtrip` exits 0 |

## CI Impact

No CI changes required. The test is a unit test in `anvilml-core`'s own test crate, which is already run by `cargo test --workspace --features mock-hardware` on every CI job. No new test file, binary, or gate is introduced.

## Platform Considerations

None identified. The `EnvReport` struct is a pure data type with no platform-specific behaviour. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are needed. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `serde` serialization format changes for `EnvReport` — the old 3-field JSON shape is no longer valid. If any external client or test (not yet in the codebase but assumed by later tasks P18-A1/P28-B1) sends the old shape, deserialisation will fail with a field-missing error. | Low | Medium | The design doc specifies the 7-field shape; P18-A1 and P28-B1 already assume it. The current codebase has no consumers of `EnvReport` outside this crate, so no live data breaks. The test exercises the new shape explicitly. |
| `ProvisioningState` variant names don't match the design doc (`InProgress`/`Complete` vs `Provisioning`/`Ready`). This task uses the current variant names, which will mismatch the doc. P900-A10 fixes this in the next task. | Certain | Low | Documented as out of scope. P900-A10 is a prerequisite for full correctness but is not required for this task's own acceptance. The `provisioning` field compiles correctly with the current variant names. |
| `ToSchema` derive on the new `node_types: Vec<NodeTypeDescriptor>` field — `NodeTypeDescriptor` already derives `ToSchema`, so this should work, but if there were a transitive trait bound issue it would surface at `cargo doc` time. | Low | Low | `NodeTypeDescriptor` already derives `ToSchema` in the same crate. Verified by reading `node.rs`. The `cargo doc` acceptance check will catch any derive issue. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-core --test worker_tests` exits 0
- [ ] `cargo doc -p anvilml-core --no-deps` exits 0
