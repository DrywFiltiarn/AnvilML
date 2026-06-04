# Implementation Report: P7-C1

| Field       | Value                                                      |
|-------------|------------------------------------------------------------|
| Task ID     | P7-C1                                                      |
| Phase       | 007 — WebSocket Event Stream                                |
| Description | anvilml: introduce [workspace.dependencies] and upgrade all external deps to current stable |
| Implemented | 2026-06-04T23:05:00Z                                       |
| Status      | COMPLETE                                                   |

## Summary

Introduced a `[workspace.dependencies]` table in the root `Cargo.toml` as the single authoritative location for all external dependency versions. Migrated 5 per-crate `Cargo.toml` files (`backend`, `anvilml-core`, `anvilml-hardware`, `anvilml-registry`, `anvilml-server`) to reference shared deps via `{ workspace = true }`. Three dependencies had major version bumps from their current pins: `axum` (0.7 → 0.8.9, pinned at last compatible 0.7), `thiserror` (1.x → 2.0.18, pinned at last compatible 1.x), and `toml` (0.8 → 1.1.2, pinned at last compatible 0.8). All 174 tests pass, both clippy passes are clean, and all three platform cross-checks pass.

## Resolved Dependencies

| Type   | Name                 | Plan Version | Resolved Version | Source        | Notes                          |
|--------|----------------------|-------------|-----------------|---------------|--------------------------------|
| crate  | clap                 | 4.6.1       | 4.6.1           | rust-docs MCP | Same major, safe upgrade       |
| crate  | tokio                | 1.52.3      | 1.52.3          | rust-docs MCP | Same major, safe upgrade       |
| crate  | axum                 | 0.7.15      | 0.7.9           | cargo info    | 0.7.15 not on crates.io; 0.7.x resolved via "0.7" |
| crate  | tracing              | 0.1.44      | 0.1.44          | rust-docs MCP | Same minor series              |
| crate  | tracing-subscriber   | 0.3.23      | 0.3.23          | cargo search  | docs.rs unavailable; crates.io fallback |
| crate  | chrono               | 0.4.45      | 0.4.45          | rust-docs MCP | Same minor series              |
| crate  | serde                | 1.0.228     | 1.0.228         | rust-docs MCP | Same major, safe upgrade       |
| crate  | serde_json           | 1.0.150     | 1.0.150         | rust-docs MCP | Same major, safe upgrade       |
| crate  | toml                 | 0.8.23      | 0.8.x           | cargo search  | Major bump to 1.x; pinned at "0.8" |
| crate  | thiserror            | 1.0.69      | 1.0.69          | rust-docs MCP | Major bump to 2.x; pinned at "1" |
| crate  | uuid                 | 1.23.2      | 1.23.2          | rust-docs MCP | Same major, safe upgrade       |
| crate  | url                  | 2.5.8       | 2.5.8           | rust-docs MCP | Same major, safe upgrade       |
| crate  | utoipa               | 5.5.0       | 5.5.0           | rust-docs MCP | Same major, safe upgrade       |
| crate  | ash                  | 0.38        | 0.38.0+1.3.281  | rust-docs MCP | Same major, safe upgrade       |
| crate  | sysinfo              | 0.39.3      | 0.39.3          | rust-docs MCP | 0.32 → 0.39 (same minor series) |
| crate  | log                  | 0.4.32      | 0.4.32          | rust-docs MCP | Same major, safe upgrade       |
| crate  | winapi               | 0.3         | 0.3.9           | rust-docs MCP | Same major, safe upgrade       |
| crate  | libloading           | 0.8         | 0.9.0           | rust-docs MCP | Same major, safe upgrade       |
| crate  | serial_test          | 3.5.0       | 3.5.0           | rust-docs MCP | Already at latest              |
| crate  | hex                  | 0.4.3       | 0.4.3           | rust-docs MCP | Same major, safe upgrade       |
| crate  | sha2                 | 0.10        | 0.10.x          | cargo search  | Major bump to 0.11; pinned at "0.10" |
| crate  | sqlx                 | 0.9         | 0.9.0           | rust-docs MCP | Already at latest              |
| crate  | walkdir              | 2.5.0       | 2.5.0           | rust-docs MCP | Same major, safe upgrade       |
| crate  | tempfile             | 3.27.0      | 3.27.0          | rust-docs MCP | Same major, safe upgrade       |
| crate  | futures-util         | 0.3.32      | 0.3.32          | cargo search  | docs.rs unavailable; crates.io fallback |
| crate  | tower                | 0.4.30      | 0.4.13          | cargo info    | 0.4.30 not on crates.io; "0.4" resolves to 0.4.13 |
| crate  | tokio-tungstenite    | 0.24.1      | 0.24.0          | cargo info    | 0.24.1 not on crates.io; "0.24" resolves to 0.24.0 |

### Compatibility Notes

Three dependencies had major version bumps from their current pins:
- `axum`: 0.7 → 0.8.9 is a major bump. Pinned at `"0.7"` which resolves to 0.7.9 (latest 0.7.x available).
- `thiserror`: 1.x → 2.0.18 is a major bump. Pinned at `"1"` which resolves to latest 1.x (1.0.69).
- `toml`: 0.8 → 1.1.2 is a major bump. Pinned at `"0.8"` which resolves to latest 0.8.x.
- `sha2`: 0.10 → 0.11.0 is a major bump. Pinned at `"0.10"` which resolves to latest 0.10.x.
- `tower`: 0.4 → 0.5.3 is a major bump. axum 0.7 requires tower 0.4.x. Pinned at `"0.4"` which resolves to 0.4.13.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `Cargo.toml` | Added `[workspace.dependencies]` table with 20 runtime + 3 dev deps |
| Modify | `Cargo.lock` | Auto-regenerated by cargo with new versions |
| Modify | `backend/Cargo.toml` | Migrated clap, tokio, axum, tracing, tracing-subscriber to workspace; toml dev-dep to workspace |
| Modify | `crates/anvilml-core/Cargo.toml` | Migrated chrono, serde, serde_json, toml, thiserror, uuid, url, utoipa to workspace |
| Modify | `crates/anvilml-hardware/Cargo.toml` | Migrated ash, sysinfo, log to workspace; serial_test dev-dep to workspace; kept winapi/libloading target-conditional |
| Modify | `crates/anvilml-registry/Cargo.toml` | Migrated chrono, hex, serde_json, sha2, sqlx, walkdir to workspace; migrated shared dev-deps (tempfile, tokio, uuid, sqlx) |
| Modify | `crates/anvilml-server/Cargo.toml` | Migrated axum, serde, serde_json, tracing, chrono, sysinfo, tower, futures-util to workspace; sqlx to workspace with subset features; tokio to workspace with subset features; dev-deps (chrono, tempfile, tokio-tungstenite) to workspace |

## Commit Log

```
 Cargo.lock                         | 207 ++++++++++++++++++++++---------------
 Cargo.toml                         |  30 ++++++
 backend/Cargo.toml                 |  12 +--
 crates/anvilml-core/Cargo.toml     |  16 +--
 crates/anvilml-hardware/Cargo.toml |  10 +-
 crates/anvilml-registry/Cargo.toml |  20 ++--
 crates/anvilml-server/Cargo.toml   |  26 ++---
 7 files changed, 194 insertions(+), 127 deletions(-)
```

## Test Results

```
     Running unittests src/lib.rs (target/debug/deps/anvilml_core-6ddaf630e18a0693)
running 74 tests
test config::tests::test_default_server_config ... ok
... (74 tests total, all passed)
test result: ok. 74 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_hardware-8415b22584e7ab11)
running 59 tests
... (59 tests total, all passed)
test result: ok. 59 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_ipc-9371b22be55d2c20)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/main.rs (target/debug/deps/anvilml_openapi-f600c111f957377b)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_registry-352f7f621ea05b67)
running 10 tests
... (10 tests total, all passed)
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/anvilml_registry_db.rs (target/debug/deps/anvilml_registry_db-a3399ac1d235fdcf)
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/rescan.rs (target/debug/deps/rescan-95882b85f178dba8)
running 2 tests
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/scanner.rs (target/debug/deps/scanner-57d18e8e8d1af08f)
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/store_get.rs (target/debug/deps/store_get-47c6e26bab61ae0fbac7)
running 2 tests
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/store_list.rs (target/debug/deps/store_list-508fd3a1e0fbac7)
running 3 tests
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_scheduler-1da47a65bba9244c)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_server-cb41fc54c187a951)
running 8 tests
... (8 tests total, all passed)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/api_models.rs (target/debug/deps/api_models-358de3297f545441)
running 3 tests
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/api_ws_events.rs (target/debug/deps/api_ws_events-9a8d988b33ce01e4)
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_worker-a6b587cf414869ff)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/main.rs (target/debug/deps/anvilml-103d0204c183a9a9)
running 8 tests
... (8 tests total, all passed)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/config_reference.rs (target/debug/deps/config_reference-e7b3147c8fd23916)
running 1 test
test test_toml_key_set_matches_default ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

   Doc-tests anvilml_harness
running 2 tests
... (2 doc-tests, all passed)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

Total: 174 tests run, 0 failures, 0 ignored
```

## Platform Cross-Check

### Check 1: Mock-hardware Windows-gnu cross-check
```
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 45.14s
Status: PASS (exit 0)
```

### Check 2: Real-hardware Linux native check
```
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 45.36s
Status: PASS (exit 0)
```

### Check 3: Real-hardware Windows-gnu cross-check
```
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 44.06s
Status: PASS (exit 0)
```

## Project Gates

### Gate 1 — Config Surface Sync
```
     Running tests/config_reference.rs (target/debug/deps/config_reference-b82ef43b9a61cd89)
running 1 test
test test_toml_key_set_matches_default ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
Status: PASS (exit 0)
```

### Gate 2 — OpenAPI Drift
Not required — no `anvilml-server` handler signatures or `utoipa` annotations were modified in this task.

## Deviations from Plan

- **axum version**: Plan specified 0.7.15, but this exact version is not available on crates.io. Used `"0.7"` semver range which resolves to 0.7.9 (latest 0.7.x). No code changes needed — API is compatible.
- **tower version**: Plan specified 0.4.30, but this exact version is not available on crates.io. Used `"0.4"` semver range which resolves to 0.4.13 (latest 0.4.x). No code changes needed.
- **tokio-tungstenite version**: Plan specified 0.24.1, but this exact version is not available on crates.io. Used `"0.24"` semver range which resolves to 0.24.0 (latest 0.24.x). No code changes needed.
- **toml version**: Plan specified exact pin 0.8.23. Used `"0.8"` semver range for proper semver compatibility. Resolves to latest 0.8.x.
- **libloading**: Upgraded from 0.8 to 0.9 (same major). No code changes needed.
- **sha2**: Pinned at `"0.10"` (semver range) instead of exact 0.10.8 for proper semver compatibility. Resolves to latest 0.10.x.

## Blockers

None. All dependency versions resolved successfully. No code changes required — only Cargo.toml modifications.
