# Implementation Report: P20-A2

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P20-A2                          |
| Phase         | 020 — OpenAPI & Launcher Polish |
| Description   | anvilml-openapi: generate backend/openapi.json |
| Implemented   | 2026-06-12T09:15:00Z           |
| Status        | COMPLETE                        |

## Summary

Implemented `crates/anvilml-openapi/src/main.rs` to generate a complete OpenAPI 3.1 specification from all `anvilml-server` handler annotations and component schemas. The binary uses utoipa's programmatic API to collect path definitions from handler functions with `#[utoipa::path]` attributes and registers all schema types. The generated `backend/openapi.json` contains 14 paths (including all `/v1/*` endpoints and `/health`) and 33 component schemas.

## Resolved Dependencies

| Type   | Name      | Version resolved | Source         |
|--------|-----------|------------------|----------------|
| crate  | utoipa    | 5.5.0            | workspace dep  |
| crate  | serde_json| 1.0.150          | workspace dep  |

Note: The plan specified `utoipa` with `features = ["json"]`, but this feature does not exist in utoipa 5.5.0. Removed the feature flag — JSON serialization is always available.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-openapi/Cargo.toml` | Add `serde_json` and `utoipa` workspace deps; bump version 0.1.0 → 0.1.1 |
| Modify | `crates/anvilml-openapi/src/main.rs` | Implement OpenAPI spec generation using utoipa programmatic API |
| Modify | `crates/anvilml-server/src/lib.rs` | Make `handlers` module public (`mod handlers` → `pub mod handlers`) |
| Modify | `crates/anvilml-server/src/handlers/jobs.rs` | Remove unfulfilled `#[expect(dead_code)]` on `ErrorInline` (now used by openapi crate) |
| Modify | `crates/anvilml-server/src/ws/handler.rs` | Add `#[utoipa::path]` annotation to `ws_events` handler; remove unused `use utoipa::ToSchema` |
| Create | `backend/openapi.json` | Generated OpenAPI 3.1 spec (1712 lines, 51899 bytes) |

## Commit Log

```
 .forge/reports/P20-A2_plan.md              |   82 ++
 .forge/state/CURRENT_TASK.md               |    6 +-
 .forge/state/state.json                    |   13 +-
 Cargo.lock                                 |    4 +-
 backend/openapi.json                       | 1712 ++++++++++++++++++++++++++++
 crates/anvilml-openapi/Cargo.toml          |    4 +-
 crates/anvilml-openapi/src/main.rs         |  122 +-
 crates/anvilml-server/src/handlers/jobs.rs |    1 -
 crates/anvilml-server/src/lib.rs           |    2 +-
 crates/anvilml-server/src/ws/handler.rs    |    8 +
 10 files changed, 1940 insertions(+), 14 deletions(-)
```

## Test Results

```
cargo test --workspace --features mock-hardware

anvilml_core:  74 passed, 0 failed
anvilml_hardware: 56 passed, 0 failed
anvilml_ipc: 18 passed, 0 failed
anvilml_registry: 19 + 1 + 4 + 2 + 1 + 7 + 2 + 3 = 39 passed, 0 failed
anvilml_scheduler: 43 passed, 0 failed
anvilml_server: 42 + 1 + 3 + 1 = 47 passed, 0 failed
anvilml_worker: 19 passed, 0 failed
backend (anvilml binary): 17 + 2 + 5 + 1 + 4 = 29 passed, 0 failed
Doc-tests: 2 passed, 0 failed

Total: 269+ tests passed, 0 failed
```

## Format Gate

```
cargo fmt --all -- --check
(Exit 0 — no formatting drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux check
cargo check --workspace --features mock-hardware
→ Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.84s

# 2. Mock-hardware Windows cross-check
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
→ Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.34s

# 3. Real-hardware Linux check
cargo check --bin anvilml
→ Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.95s

# 4. Real-hardware Windows cross-check
cargo check --bin anvilml --target x86_64-pc-windows-gnu
→ Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.47s

All 4 checks passed.
```

## Project Gates

```
cargo test -p backend --features mock-hardware --test config_reference
→ running 1 test
→ test test_toml_key_set_matches_default ... ok
→ test result: ok. 1 passed; 0 failed; 0 ignored
```

## Deviations from Plan

- **utoipa feature flag**: The plan specified `utoipa = { workspace = true, features = ["json"] }`, but utoipa 5.5.0 does not have a `json` feature. Removed the feature flag — JSON serialization is always available in utoipa 5.x.
- **Programmatic API instead of derive macro**: The plan's approach of using `#[derive(OpenApi)]` with cross-crate function references does not work because the utoipa derive macro cannot resolve `__path_*` types generated in a sibling crate. Used utoipa's programmatic API (`PathsBuilder::path_from`, `Components::builder().schema()`) instead.
- **Added `pub mod handlers`**: The `handlers` module was private in `anvilml-server/src/lib.rs`. Made it public to allow cross-crate access to handler types (`HealthResponse`, `ErrorInline`, etc.) needed for schema registration.
- **Added `#[utoipa::path]` to `ws_events`**: The WebSocket handler was missing the path annotation. Added it so the endpoint appears in the generated spec.
- **Removed `#[expect(dead_code)]` from `ErrorInline`**: This was a pre-existing lint expectation that became unfulfilled because `ErrorInline` is now used by the openapi crate. Removed the attribute per FORGE_AGENT_RULES §9.3.
- **Removed unused `use utoipa::ToSchema` from ws/handler.rs**: Pre-existing unused import warning. Fixed per FORGE_AGENT_RULES §9.3.

## Blockers

None.
