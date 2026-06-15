# Implementation Report: P6-B2

| Field         | Value                                             |
|---------------|---------------------------------------------------|
| Task ID       | P6-B2                                             |
| Phase         | 006 — Model Registry                              |
| Description   | anvilml-server: POST /v1/models/rescan + startup scan |
| Implemented   | 2026-06-15T23:45:00Z                              |
| Status        | COMPLETE                                          |

## Summary

Implemented the POST /v1/models/rescan endpoint and startup model scan. Added `scan_and_upsert` method to `ModelStore` that combines directory scanning with database upsert. Added `model_dirs` field to `AppState` to pass configured directories to the rescan handler. The handler responds with HTTP 202 immediately and spawns a background task. The server now runs an initial scan at startup so models are available before any HTTP request. Three integration tests verify the rescan endpoint behavior.

## Resolved Dependencies

None. This task introduces no new external crates or packages. It reuses existing dependencies: `tokio` (for `tokio::spawn`), `anvilml-core::ModelDirConfig`, `anvilml_registry::ModelScanner`, and `anvilml_registry::ModelStore`. All are already declared in the workspace manifests.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `crates/anvilml-registry/src/store.rs` | Added `scan_and_upsert` method with `#[tracing::instrument]` |
| MODIFY | `crates/anvilml-registry/Cargo.toml` | Bump patch version 0.1.7 → 0.1.8 |
| MODIFY | `crates/anvilml-server/src/state.rs` | Added `model_dirs` field to `AppState`; updated both constructors |
| MODIFY | `crates/anvilml-server/src/handlers/models.rs` | Added `rescan_models` handler (POST /v1/models/rescan) |
| MODIFY | `crates/anvilml-server/src/lib.rs` | Mounted `POST /v1/models/rescan` route; added `post` import |
| MODIFY | `crates/anvilml-server/Cargo.toml` | Bump patch version 0.1.8 → 0.1.9; added `tempfile` dev-dep |
| MODIFY | `backend/src/main.rs` | Added startup scan after AppState construction; passed `model_dirs` to constructor |
| MODIFY | `crates/anvilml-server/tests/models_tests.rs` | Added 3 integration tests for rescan endpoint |
| MODIFY | `crates/anvilml-server/tests/system_tests.rs` | Updated `new_with_hardware` call to pass empty `model_dirs` |
| MODIFY | `docs/TESTS.md` | Added 3 entries for new rescan tests |
| MODIFY | `backend/tests/cli_tests.rs` | Formatting-only change from `cargo fmt --all` |

## Commit Log

```
 .forge/reports/P6-B2_plan.md                 | 160 +++++++++++++++++++
 .forge/state/CURRENT_TASK.md                 |   6 +-
 .forge/state/state.json                      |  13 +-
 Cargo.lock                                   |   7 +-
 backend/src/main.rs                          |  27 ++++
 backend/tests/cli_tests.rs                   | 196 +++++++++++------------
 crates/anvilml-registry/Cargo.toml           |   2 +-
 crates/anvilml-registry/src/store.rs         |  89 ++++++++++-
 crates/anvilml-server/Cargo.toml             |   3 +-
 crates/anvilml-server/src/handlers/models.rs |  68 ++++++++
 crates/anvilml-server/src/lib.rs             |   7 +-
 crates/anvilml-server/src/state.rs           |  15 ++
 crates/anvilml-server/tests/models_tests.rs  | 226 ++++++++++++++++++++++++++-
 crates/anvilml-server/tests/system_tests.rs  |   2 +-
 docs/TESTS.md                                |  27 ++++
 15 files changed, 718 insertions(+), 130 deletions(-)
```

## Test Results

```
     Running tests/models_tests.rs (target/debug/deps/models_tests-b9b3c79d63aa7896)

running 6 tests
test test_rescan_returns_202 ... ok
test test_list_models_empty ... ok
test test_get_model_not_found ... ok
test test_list_models_with_kind_filter ... ok
test test_rescan_populates_registry ... ok
test test_rescan_infer_kind_and_dtype ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

Full workspace test suite: all 108 tests passed across all crates.
```

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.85s

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.79s

# 3. Real-hardware Linux
cargo check --bin anvilml
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.31s

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.30s
```

## Project Gates

```
# Gate 1 — Config Surface Sync
cargo test -p anvilml --features mock-hardware -- config_reference
  Running tests/config_reference.rs
  test config_reference ... ok
  test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

# Gate 2 — OpenAPI Drift
cargo run -p anvilml-openapi && git diff --exit-code api/openapi.json
  (exited 0 — no drift)
```

## Public API Delta

```
+    pub async fn scan_and_upsert(&self, dirs: &[ModelDirConfig]) -> Result<usize, AnvilError> {
+    pub model_dirs: Vec<anvilml_core::ModelDirConfig>,
```

New `pub` items:
- `scan_and_upsert` — `pub async fn` in `anvilml-registry/src/store.rs` — matches plan's Public API Surface
- `model_dirs` — `pub` field in `anvilml-server/src/state.rs` — matches plan's Public API Surface

The `rescan_models` handler is `pub(crate)` (not `pub`), so it does not appear in this grep. It is documented in the plan's Public API Surface table.

## Deviations from Plan

- **Version bump discrepancy**: The plan specified `anvilml-registry` bump from `0.1.7 → 0.1.8`, but the actual file had `0.1.7` — this matched the plan. The plan said `anvilml-server` bump from `0.1.8 → 0.1.9` which was correctly applied.
- **API call pattern**: The plan's approach said `ModelScanner::scan(dirs).await` but the actual API is `ModelScanner.scan(dirs).await` (instance method on the unit struct, not a static method). The fix was documented inline in the code with a comment explaining the zero-size unit struct pattern.
- **Duplicate detection in scan_and_upsert**: Added per-model duplicate detection (checking if the same ID was already upserted in the current batch) to avoid redundant DB writes during rescans. This was not in the plan but is a minor optimization.
- **system_tests.rs and models_tests.rs constructor updates**: The existing tests called `AppState::new_with_hardware` with 4 arguments. Added the 5th `model_dirs` argument (`Vec::new()`) to maintain compilation. These are necessary changes to existing test code due to the constructor signature change.
- **backend/tests/cli_tests.rs formatting**: The `cargo fmt --all` pass reformatted this file (196 lines changed). This is a cosmetic-only change from the formatter, not an implementation change.

## Blockers

None.
