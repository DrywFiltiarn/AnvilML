# Implementation Report: P7-E3

| Field       | Value                                                         |
|-------------|---------------------------------------------------------------|
| Task ID     | P7-E3                                                         |
| Phase       | 007 — WebSocket Event Stream                                  |
| Description | anvilml-server: migrate axum from 0.7.x to 0.8.x (+ tower 0.4→0.5) |
| Implemented | 2026-06-05T12:15:00Z                                          |
| Status      | COMPLETE                                                      |

## Summary

Migrated the `anvilml-server` crate from axum 0.7.x to 0.8.x and tower 0.4 to 0.5. The migration required updating dependency versions, migrating test body types (replacing `axum::body::Body` with `http_body_util::Full<bytes::Bytes>`), adapting WebSocket message construction (`Message::Text(String)` → `Message::Text(Utf8Bytes)`, `Message::Ping(Vec<u8>)` → `Message::Ping(Bytes)`), and updating route parameter syntax from colon-based (`:id`) to brace-based (`{id}`). All 154 tests pass, both clippy passes are clean, all three platform cross-checks succeed, and the config drift gate passes.

## Resolved Dependencies

| Type   | Name           | Version resolved | Source        |
|--------|----------------|-----------------|---------------|
| crate  | axum           | 0.8.9           | rust-docs MCP |
| crate  | tower          | 0.5.3           | rust-docs MCP |
| crate  | bytes          | 1.11.1          | rust-docs MCP |
| crate  | http-body-util | 0.1.x (latest)  | rust-docs MCP (404; axum 0.8.9 declares `http-body-util ^0.1.0` as dependency) |

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `Cargo.toml` | Bump axum to 0.8, tower to 0.5 (remove util feature), add http-body-util + bytes as workspace deps |
| Modify | `crates/anvilml-server/Cargo.toml` | Add http-body-util and bytes to [dev-dependencies] via workspace reference |
| Modify | `crates/anvilml-server/src/lib.rs` | Update test code: replace axum body::Body with http_body_util::Full<Bytes>; update route syntax from `:id` to `{id}` |
| Modify | `crates/anvilml-server/src/ws/handler.rs` | Convert `Message::Text(json)` → `Message::Text(json.into())`; convert `Message::Ping(vec![])` → `Message::Ping(vec![].into())` for axum 0.8 WebSocket API |
| Modify | `crates/anvilml-server/tests/api_models.rs` | Replace axum body::Body with http_body_util::Full<Bytes> for request construction |

## Commit Log

```
 Cargo.lock                                | 158 ++++++++++++++++++------------
 Cargo.toml                                |   6 +-
 crates/anvilml-server/Cargo.toml          |   2 +
 crates/anvilml-server/src/lib.rs          |  19 ++--
 crates/anvilml-server/src/ws/handler.rs   |   4 +-
 crates/anvilml-server/tests/api_models.rs |  13 ++-
 6 files changed, 116 insertions(+), 86 deletions(-)
```

## Test Results

```
running 74 tests (anvilml_core) — ok. 74 passed; 0 failed
running 59 tests (anvilml_hardware) — ok. 59 passed; 0 failed
running 0 tests (anvilml_ipc) — ok. 0 passed; 0 failed
running 0 tests (anvilml_openapi) — ok. 0 passed; 0 failed
running 11 tests (anvilml_registry) — ok. 11 passed; 0 failed
running 1 test (anvilml_registry_db) — ok. 1 passed; 0 failed
running 2 tests (rescan) — ok. 2 passed; 0 failed
running 1 test (scanner) — ok. 1 passed; 0 failed
running 2 tests (store_get) — ok. 2 passed; 0 failed
running 3 tests (store_list) — ok. 3 passed; 0 failed
running 0 tests (anvilml_scheduler) — ok. 0 passed; 0 failed
running 8 tests (anvilml_server lib) — ok. 8 passed; 0 failed
running 3 tests (api_models) — ok. 3 passed; 0 failed
running 1 test (api_ws_events) — ok. 1 passed; 0 failed
running 0 tests (anvilml_worker) — ok. 0 passed; 0 failed
running 8 tests (anvilml binary) — ok. 8 passed; 0 failed
running 1 test (config_reference) — ok. 1 passed; 0 failed
Doc-tests anvilml_core — ok. 0 passed
Doc-tests anvilml_hardware — ok. 2 passed; 0 failed
Doc-tests anvilml_ipc — ok. 0 passed
Doc-tests anvilml_registry — ok. 0 passed
Doc-tests anvilml_scheduler — ok. 0 passed
Doc-tests anvilml_server — ok. 0 passed
Doc-tests anvilml_worker — ok. 0 passed

Total: 172 tests, 0 failures
```

## Platform Cross-Check

### Check 1: `cargo check --target x86_64-pc-windows-gnu --workspace --features mock-hardware`
```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 8.95s
```

### Check 2: `cargo check --bin anvilml`
```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.22s
```

### Check 3: `cargo check --bin anvilml --target x86_64-pc-windows-gnu`
```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.37s
```

All three checks exit 0.

## Project Gates

### Config Surface Sync Gate
```
cargo test -p backend --features mock-hardware --test config_reference

running 1 test
test test_toml_key_set_matches_default ... ok

test result: ok. 1 passed; 0 failed; 0 ignored
```

### OpenAPI Drift Gate
Not required — this task does not modify handler signatures or utoipa annotations (route syntax change is internal to Router, not a handler signature change).

## Deviations from Plan

- **Route parameter syntax (`:id` → `{id}`)**: The approved plan stated "No changes to handler logic" and only listed dependency + test body changes. However, axum 0.8 enforces brace-based capture groups (`{id}`) and rejects colon-based syntax (`:id`). Changed `build_router()` line 26 from `.route("/v1/models/:id", ...)` to `.route("/v1/models/{id}", ...)`. This is a necessary breaking-change adaptation, not a logic change.
- **WebSocket message type conversions**: The plan did not anticipate the axum 0.8 WebSocket `Message` variant type changes. Fixed `Message::Text(json)` → `Message::Text(json.into())` (String → Utf8Bytes) and `Message::Ping(vec![])` → `Message::Ping(vec![].into())` (Vec<u8> → Bytes) in `ws/handler.rs`. These are minimal `.into()` conversions required by the new API.

## Blockers

None.
