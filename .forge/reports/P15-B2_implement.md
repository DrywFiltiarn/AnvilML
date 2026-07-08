# Implementation Report: P15-B2

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P15-B2                          |
| Phase         | 15 — Artifact Storage Wiring    |
| Description   | anvilml-server: GET /v1/artifacts/:hash serve PNG bytes |
| Implemented   | 2026-07-08T22:00:00Z            |
| Status        | COMPLETE                          |

## Summary

Implemented the `GET /v1/artifacts/{hash}` HTTP handler that serves raw PNG bytes for a
content-addressed artifact by its SHA-256 hash. The handler delegates to
`ArtifactStore::get()`, returning `200 OK` with `Content-Type: image/png` on success,
`404 Not Found` via `AnvilError::ArtifactNotFound` for missing artifacts, and `500` for
I/O errors. The route was registered in `build_router()`, 4 integration tests were added,
and the `anvilml-server` crate version was bumped from `0.1.9` to `0.1.10`.

## Resolved Dependencies

| Type   | Name    | Version resolved | Source         |
|--------|---------|------------------|----------------|
| crate  | axum    | 0.8.9            | Cargo.lock     |
| crate  | anvilml-artifacts | 0.1.x (workspace) | workspace path dep |

No new external dependencies were introduced. The existing `anvilml-artifacts` path
dependency already exposes `ArtifactStore::get()`.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-server/src/handlers/artifacts.rs` | Added `get_artifact()` handler function with doc comment, `#[tracing::instrument]`, and inline comments |
| Modify | `crates/anvilml-server/src/lib.rs` | Registered `GET /v1/artifacts/{hash}` route in `build_router()` |
| Modify | `crates/anvilml-server/tests/artifacts_tests.rs` | Added 4 new integration tests (existing_hash_returns_200, unknown_hash_returns_404, byte_for_byte_match, content_type_header) |
| Modify | `crates/anvilml-server/Cargo.toml` | Bumped patch version 0.1.9 → 0.1.10 |
| Modify | `docs/TESTS.md` | Added 4 test catalogue entries for the new tests |

## Commit Log

 .forge/state/CURRENT_TASK.md                    |   6 +-
 .forge/state/state.json                         |  13 +--
 Cargo.lock                                      |   2 +-
 crates/anvilml-server/Cargo.toml                |   2 +-
 crates/anvilml-server/src/handlers/artifacts.rs |  54 +++++++++++
 crates/anvilml-server/src/lib.rs                |   5 +
 crates/anvilml-server/tests/artifacts_tests.rs  | 121 ++++++++++++++++++++++++
 docs/TESTS.md                                   |  48 ++++++++++
 8 files changed, 240 insertions(+), 11 deletions(-)

## Test Results

     Running tests/artifacts_tests.rs (target/debug/deps/artifacts_tests-bb220c634e21a84d)

running 8 tests
test test_get_artifact_unknown_hash_returns_404 ... ok
test test_list_artifacts_empty_store_returns_200_empty_array ... ok
test test_get_artifact_content_type_header ... ok
test test_get_artifact_existing_hash_returns_200 ... ok
test test_get_artifact_byte_for_byte_match ... ok
test test_list_artifacts_json_shape ... ok
test test_list_artifacts_job_id_filter_returns_matching ... ok
test test_list_artifacts_populated_returns_all ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s

All 281 workspace tests passed (0 failed).

## Format Gate

`cargo fmt --all -- --check` exited 0 — no formatting drift detected.

## Platform Cross-Check

All four platform cross-checks passed:

1. `cargo check --workspace --features mock-hardware` — Finished (Linux, mock-hardware)
2. `cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu` — Finished (Windows cross-compile)
3. `cargo check --bin anvilml` — Finished (Linux, real-hardware)
4. `cargo check --bin anvilml --target x86_64-pc-windows-gnu` — Finished (Windows cross-compile, real-hardware)

## Project Gates

Gate 1 — Config Surface Sync: `cargo test -p anvilml --features mock-hardware -- config_reference` exited 0 (test `config_reference_matches_defaults` passed).

Gate 2 — OpenAPI Drift: Not triggered — this task does not modify handler signatures, `#[utoipa::path]` annotations, or `AppState` fields used in response types. No `utoipa` imports are present in the handler files.

Gate 3 — Node Parity: Not triggered — this task does not add, remove, or rename node types.

Gate 4 — Mock/Real Parity Markers: Not triggered — this task adds a Rust HTTP handler, not a Python node `execute()` or arch module `load()`/`sample()`/`decode()` function covered by the marker convention.

## Public API Delta

No new `pub` items introduced. The `get_artifact` function is `pub(crate)` only, matching
the established pattern for handler functions. The grep command returned no output.

## Deviations from Plan

None. Implementation matches the approved plan exactly:
- Handler signature matches the plan's specification.
- Route registered after `/v1/artifacts` in `build_router()`.
- 4 integration tests added (matching the plan's Tests table).
- Version bumped from 0.1.9 to 0.1.10.

## Blockers

None.
