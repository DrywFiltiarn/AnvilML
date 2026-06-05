# Plan Report: P7-E3

| Field       | Value                                         |
|-------------|-----------------------------------------------|
| Task ID     | P7-E3                                         |
| Phase       | 007 — WebSocket Event Stream                  |
| Description | anvilml-server: migrate axum from 0.7.x to 0.8.x (+ tower 0.4→0.5) |
| Depends on  | P7-E2                                         |
| Project     | anvilml                                       |
| Planned at  | 2026-06-05T11:35:00Z                          |
| Attempt     | 1                                             |

## Objective

Migrate the `anvilml-server` crate from axum 0.7.x to 0.8.x and tower 0.4 to 0.5 simultaneously — they are incompatible across major versions. The primary breaking changes are: (1) the `axum::body::Body` type changed (from hyper 0.14's body to http-body-util's body), requiring test code to use `http_body_util::Full<bytes::Bytes>` instead of `axum::body::Body`; (2) tower 0.5 removed the `util` feature flag; (3) all handler extractor ordering must place `State<T>` first (already correct in this codebase). No source-level handler logic changes are needed — only dependency version bumps and test-code body type adjustments.

## Scope

### In Scope
- Bump `axum` from `"0.7"` to `"0.8"` in `[workspace.dependencies]` (keep features `["json", "ws"]`)
- Bump `tower` from `"0.4"` to `"0.5"` in `[workspace.dependencies]`; remove the now-invalid `features = ["util"]` flag
- Add `http-body-util` and `bytes` as workspace dev-dependencies (required by axum 0.8 test APIs)
- Update `crates/anvilml-server/Cargo.toml` to reference `http-body-util` via workspace in `[dev-dependencies]`
- Update test code in `crates/anvilml-server/src/lib.rs` — replace `axum::body::Body` with `http_body_util::Full<bytes::Bytes>` for request body construction (`Body::empty()` → `Full::<Bytes>::default()`)
- Update test code in `crates/anvilml-server/tests/api_models.rs` — same body type migration
- Verify `tower::ServiceExt` import path is unchanged (tower 0.5 re-exports it at crate root)

### Out of Scope
- No changes to handler logic in any `.rs` source file under `handlers/` or `ws/` — all extractors already have `State` first, and `Router::with_state` API is stable
- No changes to `tokio-tungstenite` version (deferred to a separate follow-on task per TASKS_PHASE007.md)
- No changes to `anvilml-openapi` crate (it depends on `anvilml-server` but not directly on axum/tower)
- No CI workflow modifications
- No changes to runtime source files — only dependency versions and test code

## Approach

1. **Query MCP for version confirmation.** Use `rust-docs-lookup-crate-docs` to confirm the latest stable versions of `axum`, `tower`, `http-body-util`, and `bytes`. Record versions in plan.
2. **Update root `Cargo.toml`.** Modify `[workspace.dependencies]`:
   - Change `axum = { version = "0.7", features = ["json", "ws"] }` → `axum = { version = "0.8", features = ["json", "ws"] }`
   - Change `tower = { version = "0.4", features = ["util"] }` → `tower = { version = "0.5" }` (remove `features`)
   - Add `http-body-util = "0.1"` to workspace dependencies
   - Add `bytes = "1"` to workspace dependencies
3. **Update `crates/anvilml-server/Cargo.toml`.** Add `[dev-dependencies]` entry: `http-body-util = { workspace = true }` and `bytes = { workspace = true }`.
4. **Audit all handler extractors.** Confirm every handler in `handlers/health.rs`, `handlers/system.rs`, `handlers/models.rs`, and `ws/handler.rs` already has `State<T>` as the first extractor parameter. (Verified: all handlers satisfy this — no reordering needed.)
5. **Verify `Router::with_state`.** The call in `lib.rs` line 30 (`Router::new()...with_state(state_arc)`) is compatible with axum 0.8's API. No change needed.
6. **Migrate test body types.** In `lib.rs` (tests module) and `tests/api_models.rs`:
   - Replace `use axum::body::Body;` with `use http_body_util::Full;` + `use bytes::Bytes;`
   - Replace `.body(Body::empty())` with `.body(Full::<Bytes>::default()).unwrap()`
7. **Verify tower::ServiceExt.** The import `use tower::ServiceExt;` in `lib.rs` test code remains valid — tower 0.5 re-exports `ServiceExt` at the crate root from `tower-service`. No change needed.
8. **Run verification gates.** Execute `cargo clippy --workspace --features mock-hardware -- -D warnings`, then `cargo test --workspace --features mock-hardware`, and cross-check all exit 0.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `Cargo.toml` | Bump axum to 0.8, tower to 0.5 (remove util feature), add http-body-util + bytes as workspace deps |
| Modify | `crates/anvilml-server/Cargo.toml` | Add http-body-util and bytes to [dev-dependencies] via workspace reference |
| Modify | `crates/anvilml-server/src/lib.rs` | Update test code: replace axum body::Body with http_body_util::Full<Bytes> for request construction |
| Modify | `crates/anvilml-server/tests/api_models.rs` | Same body type migration in integration tests |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|-----------------|
| `crates/anvilml-server/src/lib.rs` (tests) | `health_returns_200` | Router builds, handler extracts State correctly, returns 200 JSON |
| `crates/anvilml-server/src/lib.rs` (tests) | `env_returns_200_with_stub_report` | `/v1/system/env` handler works with new axum types |
| `crates/anvilml-server/src/lib.rs` (tests) | `system_returns_200_with_hardware_info` | System handler works with mock-hardware feature |
| `crates/anvilml-server/src/lib.rs` (tests) | `get_model_returns_404_when_missing` | Model handler error path works |
| `crates/anvilml-server/src/lib.rs` (tests) | `rescan_returns_202` | POST handler works with new body type |
| `crates/anvilml-server/tests/api_models.rs` | `list_models_returns_scanned_models` | Integration test: models list endpoint |
| `crates/anvilml-server/tests/api_models.rs` | `list_models_kind_filter_diffusion` | Integration test: kind filter on models |
| `crates/anvilml-server/tests/api_models.rs` | `list_models_kind_filter_no_match` | Integration test: empty result filter |
| `crates/anvilml-server/tests/api_ws_events.rs` | `ws_connect_broadcast_receive` | WebSocket upgrade + broadcast + receive round-trip |
| `crates/anvilml-server/src/ws/broadcaster.rs` (tests) | `subscribe_send_receive` | Broadcaster sends and receives events |
| `crates/anvilml-server/src/ws/broadcaster.rs` (tests) | `send_no_subscribers_no_error` | Broadcast with no subscribers is safe |
| `crates/anvilml-server/src/ws/stats_tick.rs` (tests) | `stats_tick_broadcasts_event` | Stats tick task broadcasts SystemStats events |

## CI Impact

No CI workflow file modifications are required. The existing CI matrix already runs `cargo clippy --workspace --features mock-hardware -- -D warnings` and `cargo test --workspace --features mock-hardware` on both Linux and Windows. This task only changes dependency versions and test code body types — the CI commands remain identical. Cargo.lock will be regenerated with new dependency resolutions; this is expected and the updated lock file should be committed.

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| `http_body_util::Full` API signature differs from expectations (e.g., no `default()`) | Verify via `rust-docs-lookup-crate-docs http-body-util` before writing; if `default()` is unavailable, use `Full::new(Bytes::new()).unwrap()` or `Full::empty()` |
| `tower::ServiceExt` import path changes in tower 0.5 | Verified: tower 0.5 re-exports `ServiceExt` at crate root from `tower-service`. If this changes, update to `use tower_service::Service;` and call `.poll_ready()` / `.call()` directly |
| `axum::body::to_bytes` API changed in axum 0.8 | The function remains available as `axum::body::to_bytes` — it works on any type implementing `http_body::Body`. If the import path changes, adjust to `http_body_util::BodyExt::to_bytes` |
| `WebSocketUpgrade` response type changed | Verified against axum 0.8 docs.rs: `WebSocketUpgrade` still implements `IntoResponse` via `.on_upgrade()`. No change needed |
| `Router::with_state` generic parameter changed | axum 0.8 `with_state` works the same way; the router is typed as `Router<T>` after routing methods, and `with_state` accepts it. Verified against docs.rs |
| `tower` `util` feature removal causes compile error in other crates | The `util` feature is only declared in root `Cargo.toml` workspace deps and referenced by `anvilml-server/Cargo.toml`. Removing it from both locations prevents the error. No other crate references tower features directly |

## Acceptance Criteria

- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
- [ ] `cargo test --workspace --features mock-hardware` exits 0
- [ ] `cargo check --target x86_64-pc-windows-gnu --workspace --features mock-hardware` exits 0 (platform cross-check per ENVIRONMENT.md §5)
- [ ] All handler functions in `handlers/*.rs` and `ws/handler.rs` compile with `State<T>` as first extractor (verified by clean clippy)
- [ ] `Router::with_state()` call in `lib.rs` compiles without error
- [ ] Test code in `lib.rs` and `tests/api_models.rs` uses `http_body_util::Full<Bytes>` instead of `axum::body::Body`
- [ ] No source files under `handlers/` or `ws/*.rs` (outside tests) require modification
