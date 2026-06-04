# Implementation Report: P6-D1

| Field         | Value                                                           |
|---------------|-----------------------------------------------------------------|
| Task ID       | P6-D1                                                           |
| Phase         | 006 — Model Registry                                            |
| Description   | anvilml-server: fix api_models test isolation (shared temp db causes parallel test failures) |
| Implemented   | 2026-06-04T12:45:00Z                                            |
| Status        | COMPLETE                                                        |

## Summary

Replaced the `setup_test_env()` function in `crates/anvilml-server/tests/api_models.rs` with a `tempfile::TempDir`-based approach. Each test now creates its own OS-managed temporary directory, eliminating the race condition where parallel tests shared a PID-based temp directory name. All three tests (`list_models_returns_scanned_models`, `list_models_kind_filter_diffusion`, `list_models_kind_filter_no_match`) were updated to bind the returned `TempDir` guard via `_tmp`, ensuring cleanup at end of scope.

## Resolved Dependencies

| Type   | Name    | Version resolved | Source        |
|--------|---------|-----------------|---------------|
| crate  | tempfile | 3 (already in Cargo.toml) | Confirmed present in `crates/anvilml-server/Cargo.toml` `[dev-dependencies]` line 22 — no version lookup needed |

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-server/tests/api_models.rs` | Replace `setup_test_env()` with `TempDir`-based isolation; update 3 test functions to bind `_tmp` guard |

## Commit Log

```
diff --git a/crates/anvilml-server/tests/api_models.rs b/crates/anvilml-server/tests/api_models.rs
index d6151d7..65f5246 100644
--- a/crates/anvilml-server/tests/api_models.rs
+++ b/crates/anvilml-server/tests/api_models.rs
@@ -2,6 +2,8 @@ use std::fs;
 use std::path::PathBuf;
 use std::sync::Arc;
 
+use tempfile::TempDir;
+
 use anvilml_core::config::ModelDirConfig;
 use anvilml_core::ModelKind;
 use anvilml_registry::ModelRegistry;
@@ -16,20 +18,20 @@ use anvilml_server::{build_router, AppState};
 
 /// Create a unique temporary directory structure for testing model scanning.
 ///
-/// Creates a unique subdirectory under the system temp dir to avoid races
-/// between concurrent tests. Returns `(diffusion_dir_path, db_file_path)`.
-fn setup_test_env() -> (PathBuf, PathBuf) {
-    let id = std::process::id();
-    let temp_base = std::env::temp_dir().join(format!("anvilml_test_models_{id}"));
-    let _ = fs::remove_dir_all(&temp_base); // clean up from previous runs
-    fs::create_dir_all(temp_base.join("diffusion")).expect("create test dir");
-    fs::File::create(temp_base.join("diffusion/model-fp16.safetensors"))
+/// Each call creates its own `TempDir` (OS-managed, under `/tmp`) so that
+/// parallel tests never share files. Returns `(temp_dir_guard, diffusion_dir_path, db_file_path)`.
+fn setup_test_env() -> (TempDir, PathBuf, PathBuf) {
+    let tmp = tempfile::TempDir::new().expect("create temp dir");
+    let diffusion_dir = tmp.path().join("diffusion");
+    let db_path = tmp.path().join("test.db");
+
+    fs::create_dir_all(&diffusion_dir).expect("create test dir");
+    fs::File::create(diffusion_dir.join("model-fp16.safetensors"))
         .expect("create model file");
 
-    let db_path = temp_base.join("test.db");
     // Pre-create the database file — `anvilml_registry::open` requires it.
     fs::File::create(&db_path).expect("pre-create db file");
-    (temp_base.join("diffusion"), db_path)
+    (tmp, diffusion_dir, db_path)
 }
 
 /// Build an `AppState` with a fresh registry backed by a file-based SQLite
@@ -52,7 +54,7 @@ async fn build_test_app_state(model_dir: PathBuf, db_path: PathBuf) -> AppState
 
 #[tokio::test]
 async fn list_models_returns_scanned_models() {
-    let (model_dir, db_path) = setup_test_env();
+    let (_tmp, model_dir, db_path) = setup_test_env();
     let state = build_test_app_state(model_dir, db_path).await;
     let app = build_router(state);
 
@@ -86,7 +88,7 @@ async fn list_models_returns_scanned_models() {
 
 #[tokio::test]
 async fn list_models_kind_filter_diffusion() {
-    let (model_dir, db_path) = setup_test_env();
+    let (_tmp, model_dir, db_path) = setup_test_env();
     let state = build_test_app_state(model_dir, db_path).await;
     let app = build_router(state);
 
@@ -123,7 +125,7 @@ async fn list_models_kind_filter_diffusion() {
 
 #[tokio::test]
 async fn list_models_kind_filter_no_match() {
-    let (model_dir, db_path) = setup_test_env();
+    let (_tmp, model_dir, db_path) = setup_test_env();
     let state = build_test_app_state(model_dir, db_path).await;
     let app = build_router(state);
```

## Test Results

### Parallel execution (default)
```
running 3 tests
test list_models_returns_scanned_models ... ok
test list_models_kind_filter_no_match ... ok
test list_models_kind_filter_diffusion ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
```

### Sequential execution (`--test-threads=1`)
```
running 3 tests
test list_models_kind_filter_diffusion ... ok
test list_models_kind_filter_no_match ... ok
test list_models_returns_scanned_models ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
```

### Full workspace test suite
```
test result: ok. 74 passed; 0 failed (anvilml-core)
test result: ok. 59 passed; 0 failed (anvilml-hardware)
test result: ok. 0 passed; 0 failed (anvilml-ipc)
test result: ok. 0 passed; 0 failed (anvilml-openapi)
test result: ok. 10 passed; 0 failed (anvilml-registry lib)
test result: ok. 1 passed; 0 failed (anvilml_registry_db test)
test result: ok. 2 passed; 0 failed (rescan test)
test result: ok. 1 passed; 0 failed (scanner test)
test result: ok. 2 passed; 0 failed (store_get test)
test result: ok. 3 passed; 0 failed (store_list test)
test result: ok. 0 passed; 0 failed (anvilml-scheduler)
test result: ok. 5 passed; 0 failed (anvilml-server lib)
test result: ok. 3 passed; 0 failed (api_models test)
test result: ok. 0 passed; 0 failed (anvilml-worker)
test result: ok. 8 passed; 0 failed (anvilml binary tests)
test result: ok. 1 passed; 0 failed (config_reference test)
Doc-tests: all 2 passed, 0 failed
```

Total: 169 tests passed, 0 failed across the entire workspace.

## Platform Cross-Check

### Check 1 — Mock-hardware Windows-gnu cross-check
```
Checking anvilml-server v0.1.0 (/home/dryw/AnvilML/crates/anvilml-server)
Checking backend v0.1.0 (/home/dryw/AnvilML/backend)
Checking anvilml-openapi v0.1.0 (/home/dryw/AnvilML/crates/anvilml-openapi)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.78s
```
Exit: 0

### Check 2 — Real-hardware Linux native
```
Checking anvilml-server v0.1.0 (/home/dryw/AnvilML/crates/anvilml-server)
Checking backend v0.1.0 (/home/dryw/AnvilML/backend)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.55s
```
Exit: 0

### Check 3 — Real-hardware Windows-gnu cross-check
```
warning: variable does not need to be mutable
   --> crates/anvilml-hardware/src/lib.rs:106:9
    |
106 |     let mut devices = vulkan::VulkanDetector.detect().unwrap_or_default();
    |         ----^^^^^^^
    |         |
    |         help: remove this `mut`
    |
    = note: `#[warn(unused_mut)]` (part of `#[warn(unused)]`) on by default

warning: `anvilml-hardware` (lib) generated 1 warning (run `cargo fix --lib -p anvilml-hardware` to apply 1 suggestion)
    Checking anvilml-server v0.1.0 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking backend v0.1.0 (/home/dryw/AnvilML/backend)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.65s
```
Exit: 0 (warning is pre-existing, not introduced by this task)

## Project Gates

### Config Surface Sync (`config_reference`)
```
running 1 test
test test_toml_key_set_matches_default ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```
Exit: 0

## Deviations from Plan

None. Implementation follows the approved plan exactly. The only deviation was a compiler fix during implementation: the initial `setup_test_env()` body tried to return `tmp` while also borrowing `tmp.path()`, causing an E0382 borrow-of-moved-value error. This was fixed by computing `diffusion_dir` and `db_path` as `PathBuf` values before returning `tmp`, which is a straightforward implementation detail with no plan impact.

## Blockers

None.
