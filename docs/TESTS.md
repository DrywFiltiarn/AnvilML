# Test Catalogue

Every test in the AnvilML codebase is catalogued here. One entry per test.

---

## cli_help_shows_all_flags (backend)

**File:** `backend/tests/cli_help_test.rs`
**Context:** The `anvilml` binary has been compiled (`cargo build -p anvilml`).
**Tests:** The `--help` flag output contains all three CLI flags: `--host`, `--port`, and `--config`.
**Mode:** both
**Inputs:** `--help` flag passed to the compiled binary.
**Expected output:** The help text includes `--host`, `--port`, and `--config`.
**Acceptance:** `cargo test -p anvilml` exits 0.

---

## test_shutdown_signal_returns_on_ctrl_c (backend)

**File:** `backend/tests/shutdown_tests.rs`
**Context:** The `anvilml` binary has been compiled (`cargo build -p anvilml`).
**Tests:** `wait_for_shutdown_signal()` returns when a Ctrl+C / SIGINT signal is received. On Unix, a child process sends SIGINT to the test process after a 0.2s delay; on Windows, the timeout path verifies the function is callable.
**Mode:** both
**Inputs:** SIGINT signal (Unix) or no signal (Windows timeout path).
**Expected output:** The shutdown signal handler returns normally within 5s on Unix, or the timeout path completes cleanly on Windows.
**Acceptance:** `cargo test -p anvilml --test shutdown_tests` exits 0.

---

## test_shutdown_signal_timeout_cancels (backend)

**File:** `backend/tests/shutdown_tests.rs`
**Context:** The `anvilml` binary has been compiled (`cargo build -p anvilml`).
**Tests:** `wait_for_shutdown_signal()` is cancellable via `tokio::select!` with a 2-second timeout — no signal is sent, so the timeout branch wins, proving the function does not hang indefinitely and can be aborted cleanly.
**Mode:** both
**Inputs:** No signal (timeout path only).
**Expected output:** Timeout wins, handle aborted cleanly, test passes.
**Acceptance:** `cargo test -p anvilml --test shutdown_tests` exits 0.

---

## test_health_returns_200 (anvilml-server)

**File:** `crates/anvilml-server/tests/health_tests.rs`
**Context:** The `anvilml-server` crate has been compiled (`cargo test -p anvilml-server`).
**Tests:** `GET /health` returns `200 OK` via in-process router call — constructs a `GET /health` request, sends it through `build_router()`, and asserts the response status is `StatusCode::OK`.
**Mode:** both
**Inputs:** `GET /health` with empty body.
**Expected output:** `StatusCode::OK`.
**Acceptance:** `cargo test -p anvilml-server --test health_tests` exits 0.

---

## test_db_returns_500 (anvilml-core)

**File:** `crates/anvilml-core/tests/error_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `thiserror`, `axum`, `uuid`, `serde_json`, and `sqlx` dependencies.
**Tests:** `AnvilError::Db(sqlx::Error::PoolClosed)` maps to HTTP 500 (Internal Server Error).
**Mode:** both
**Inputs:** `AnvilError::Db` variant with `sqlx::Error::PoolClosed`.
**Expected output:** `StatusCode::INTERNAL_SERVER_ERROR`, JSON body `error="database_error"`.
**Acceptance:** `cargo test -p anvilml-core --test error_tests test_db_returns_500` exits 0.

---

## test_io_returns_500 (anvilml-core)

**File:** `crates/anvilml-core/tests/error_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `thiserror`, `axum`, `uuid`, `serde_json`, and `sqlx` dependencies.
**Tests:** `AnvilError::Io(io::Error)` maps to HTTP 500 (Internal Server Error).
**Mode:** both
**Inputs:** `AnvilError::Io` variant with `std::io::ErrorKind::NotFound`.
**Expected output:** `StatusCode::INTERNAL_SERVER_ERROR`, JSON body `error="io_error"`.
**Acceptance:** `cargo test -p anvilml-core --test error_tests test_io_returns_500` exits 0.

---

## test_serde_returns_400 (anvilml-core)

**File:** `crates/anvilml-core/tests/error_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `thiserror`, `axum`, `uuid`, `serde_json`, and `sqlx` dependencies.
**Tests:** `AnvilError::Serde("bad json")` maps to HTTP 400 (Bad Request).
**Mode:** both
**Inputs:** `AnvilError::Serde` variant.
**Expected output:** `StatusCode::BAD_REQUEST`, JSON body `error="serde_error"`.
**Acceptance:** `cargo test -p anvilml-core --test error_tests test_serde_returns_400` exits 0.

---

## test_ipc_returns_400 (anvilml-core)

**File:** `crates/anvilml-core/tests/error_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `thiserror`, `axum`, `uuid`, `serde_json`, and `sqlx` dependencies.
**Tests:** `AnvilError::Ipc("timeout")` maps to HTTP 400 (Bad Request).
**Mode:** both
**Inputs:** `AnvilError::Ipc` variant.
**Expected output:** `StatusCode::BAD_REQUEST`, JSON body `error="ipc_error"`.
**Acceptance:** `cargo test -p anvilml-core --test error_tests test_ipc_returns_400` exits 0.

---

## test_payload_too_large_returns_413 (anvilml-core)

**File:** `crates/anvilml-core/tests/error_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `thiserror`, `axum`, `uuid`, `serde_json`, and `sqlx` dependencies.
**Tests:** `AnvilError::PayloadTooLarge("1GB")` maps to HTTP 413 (Payload Too Large).
**Mode:** both
**Inputs:** `AnvilError::PayloadTooLarge` variant.
**Expected output:** `StatusCode::PAYLOAD_TOO_LARGE`, JSON body `error="payload_too_large"`.
**Acceptance:** `cargo test -p anvilml-core --test error_tests test_payload_too_large_returns_413` exits 0.

---

## test_worker_not_found_returns_404 (anvilml-core)

**File:** `crates/anvilml-core/tests/error_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `thiserror`, `axum`, `uuid`, `serde_json`, and `sqlx` dependencies.
**Tests:** `AnvilError::WorkerNotFound("gpu:0")` maps to HTTP 404 (Not Found).
**Mode:** both
**Inputs:** `AnvilError::WorkerNotFound` variant.
**Expected output:** `StatusCode::NOT_FOUND`, JSON body `error="worker_not_found"`.
**Acceptance:** `cargo test -p anvilml-core --test error_tests test_worker_not_found_returns_404` exits 0.

---

## test_job_not_found_returns_404 (anvilml-core)

**File:** `crates/anvilml-core/tests/error_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `thiserror`, `axum`, `uuid`, `serde_json`, and `sqlx` dependencies.
**Tests:** `AnvilError::JobNotFound("job-xyz")` maps to HTTP 404 (Not Found).
**Mode:** both
**Inputs:** `AnvilError::JobNotFound` variant.
**Expected output:** `StatusCode::NOT_FOUND`, JSON body `error="job_not_found"`.
**Acceptance:** `cargo test -p anvilml-core --test error_tests test_job_not_found_returns_404` exits 0.

---

## test_invalid_graph_returns_400 (anvilml-core)

**File:** `crates/anvilml-core/tests/error_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `thiserror`, `axum`, `uuid`, `serde_json`, and `sqlx` dependencies.
**Tests:** `AnvilError::InvalidGraph(vec!["missing input"])` maps to HTTP 400 (Bad Request).
**Mode:** both
**Inputs:** `AnvilError::InvalidGraph` variant.
**Expected output:** `StatusCode::BAD_REQUEST`, JSON body `error="invalid_graph"`.
**Acceptance:** `cargo test -p anvilml-core --test error_tests test_invalid_graph_returns_400` exits 0.

---

## test_cycle_detected_returns_400 (anvilml-core)

**File:** `crates/anvilml-core/tests/error_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `thiserror`, `axum`, `uuid`, `serde_json`, and `sqlx` dependencies.
**Tests:** `AnvilError::CycleDetected(vec!["A->B->A"])` maps to HTTP 400 (Bad Request).
**Mode:** both
**Inputs:** `AnvilError::CycleDetected` variant.
**Expected output:** `StatusCode::BAD_REQUEST`, JSON body `error="cycle_detected"`.
**Acceptance:** `cargo test -p anvilml-core --test error_tests test_cycle_detected_returns_400` exits 0.

---

## test_model_not_found_returns_404 (anvilml-core)

**File:** `crates/anvilml-core/tests/error_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `thiserror`, `axum`, `uuid`, `serde_json`, and `sqlx` dependencies.
**Tests:** `AnvilError::ModelNotFound("flux2klein4b")` maps to HTTP 404 (Not Found).
**Mode:** both
**Inputs:** `AnvilError::ModelNotFound` variant.
**Expected output:** `StatusCode::NOT_FOUND`, JSON body `error="model_not_found"`.
**Acceptance:** `cargo test -p anvilml-core --test error_tests test_model_not_found_returns_404` exits 0.

---

## test_artifact_not_found_returns_404 (anvilml-core)

**File:** `crates/anvilml-core/tests/error_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `thiserror`, `axum`, `uuid`, `serde_json`, and `sqlx` dependencies.
**Tests:** `AnvilError::ArtifactNotFound("abc123")` maps to HTTP 404 (Not Found).
**Mode:** both
**Inputs:** `AnvilError::ArtifactNotFound` variant.
**Expected output:** `StatusCode::NOT_FOUND`, JSON body `error="artifact_not_found"`.
**Acceptance:** `cargo test -p anvilml-core --test error_tests test_artifact_not_found_returns_404` exits 0.

---

## test_workers_unavailable_returns_503 (anvilml-core)

**File:** `crates/anvilml-core/tests/error_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `thiserror`, `axum`, `uuid`, `serde_json`, and `sqlx` dependencies.
**Tests:** `AnvilError::WorkersUnavailable("no gpu")` maps to HTTP 503 (Service Unavailable).
**Mode:** both
**Inputs:** `AnvilError::WorkersUnavailable` variant.
**Expected output:** `StatusCode::SERVICE_UNAVAILABLE`, JSON body `error="workers_unavailable"`.
**Acceptance:** `cargo test -p anvilml-core --test error_tests test_workers_unavailable_returns_503` exits 0.

---

## test_internal_returns_500 (anvilml-core)

**File:** `crates/anvilml-core/tests/error_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `thiserror`, `axum`, `uuid`, `serde_json`, and `sqlx` dependencies.
**Tests:** `AnvilError::Internal("panic")` maps to HTTP 500 (Internal Server Error).
**Mode:** both
**Inputs:** `AnvilError::Internal` variant.
**Expected output:** `StatusCode::INTERNAL_SERVER_ERROR`, JSON body `error="internal_error"`.
**Acceptance:** `cargo test -p anvilml-core --test error_tests test_internal_returns_500` exits 0.

---

## test_error_body_has_request_id (anvilml-core)

**File:** `crates/anvilml-core/tests/error_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `thiserror`, `axum`, `uuid`, `serde_json`, and `sqlx` dependencies.
**Tests:** Every `AnvilError` response body contains a valid UUID v4 string in the `request_id` field.
**Mode:** both
**Inputs:** `AnvilError::Serde("test")`.
**Expected output:** `request_id` is a valid UUID v4 string.
**Acceptance:** `cargo test -p anvilml-core --test error_tests test_error_body_has_request_id` exits 0.

---

## test_error_body_message_contains_variant_info (anvilml-core)

**File:** `crates/anvilml-core/tests/error_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `thiserror`, `axum`, `uuid`, `serde_json`, and `sqlx` dependencies.
**Tests:** The `message` field contains the variant's error description (e.g., the worker ID).
**Mode:** both
**Inputs:** `AnvilError::WorkerNotFound("gpu:0")`.
**Expected output:** `message` contains `"gpu:0"`.
**Acceptance:** `cargo test -p anvilml-core --test error_tests test_error_body_message_contains_variant_info` exits 0.

---

## test_error_field_is_snake_case (anvilml-core)

**File:** `crates/anvilml-core/tests/error_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `thiserror`, `axum`, `uuid`, `serde_json`, and `sqlx` dependencies.
**Tests:** All 13 variant `error` fields are lowercase snake_case (only lowercase letters and underscores, non-empty).
**Mode:** both
**Inputs:** All 13 `AnvilError` variants.
**Expected output:** Every `error` field passes the snake-case validation.
**Acceptance:** `cargo test -p anvilml-core --test error_tests test_error_field_is_snake_case` exits 0.

---

## test_host_default (anvilml-core)

**File:** `crates/anvilml-core/tests/config_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `serde` (derive feature) providing `Serialize` and `Deserialize` derives for `ServerConfig`.
**Tests:** `ServerConfig::default().host` equals `"127.0.0.1"`.
**Mode:** both
**Inputs:** `ServerConfig::default()` constructed with compiled-in defaults.
**Expected output:** `host == "127.0.0.1"`.
**Acceptance:** `cargo test -p anvilml-core --test config_tests test_host_default` exits 0.

---

## test_port_default (anvilml-core)

**File:** `crates/anvilml-core/tests/config_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `serde` (derive feature) providing `Serialize` and `Deserialize` derives for `ServerConfig`.
**Tests:** `ServerConfig::default().port` equals `8488`.
**Mode:** both
**Inputs:** `ServerConfig::default()` constructed with compiled-in defaults.
**Expected output:** `port == 8488`.
**Acceptance:** `cargo test -p anvilml-core --test config_tests test_port_default` exits 0.

---

## test_db_path_default (anvilml-core)

**File:** `crates/anvilml-core/tests/config_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `serde` (derive feature) providing `Serialize` and `Deserialize` derives for `ServerConfig`.
**Tests:** `ServerConfig::default().db_path` equals `PathBuf::from("./anvilml.db")`.
**Mode:** both
**Inputs:** `ServerConfig::default()` constructed with compiled-in defaults.
**Expected output:** `db_path == PathBuf::from("./anvilml.db")`.
**Acceptance:** `cargo test -p anvilml-core --test config_tests test_db_path_default` exits 0.

---

## test_artifact_dir_default (anvilml-core)

**File:** `crates/anvilml-core/tests/config_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `serde` (derive feature) providing `Serialize` and `Deserialize` derives for `ServerConfig`.
**Tests:** `ServerConfig::default().artifact_dir` equals `PathBuf::from("./artifacts")`.
**Mode:** both
**Inputs:** `ServerConfig::default()` constructed with compiled-in defaults.
**Expected output:** `artifact_dir == PathBuf::from("./artifacts")`.
**Acceptance:** `cargo test -p anvilml-core --test config_tests test_artifact_dir_default` exits 0.

---

## test_venv_path_default (anvilml-core)

**File:** `crates/anvilml-core/tests/config_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `serde` (derive feature) providing `Serialize` and `Deserialize` derives for `ServerConfig`.
**Tests:** `ServerConfig::default().venv_path` equals `PathBuf::from("./worker/.venv")`.
**Mode:** both
**Inputs:** `ServerConfig::default()` constructed with compiled-in defaults.
**Expected output:** `venv_path == PathBuf::from("./worker/.venv")`.
**Acceptance:** `cargo test -p anvilml-core --test config_tests test_venv_path_default` exits 0.

---

## test_model_scan_depth_default (anvilml-core)

**File:** `crates/anvilml-core/tests/config_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `serde` (derive feature) providing `Serialize` and `Deserialize` derives for `ServerConfig`.
**Tests:** `ServerConfig::default().model_scan_depth` equals `2`.
**Mode:** both
**Inputs:** `ServerConfig::default()` constructed with compiled-in defaults.
**Expected output:** `model_scan_depth == 2`.
**Acceptance:** `cargo test -p anvilml-core --test config_tests test_model_scan_depth_default` exits 0.

---

## test_max_ipc_payload_mib_default (anvilml-core)

**File:** `crates/anvilml-core/tests/config_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `serde` (derive feature) providing `Serialize` and `Deserialize` derives for `ServerConfig`.
**Tests:** `ServerConfig::default().max_ipc_payload_mib` equals `256`.
**Mode:** both
**Inputs:** `ServerConfig::default()` constructed with compiled-in defaults.
**Expected output:** `max_ipc_payload_mib == 256`.
**Acceptance:** `cargo test -p anvilml-core --test config_tests test_max_ipc_payload_mib_default` exits 0.

---

## test_num_threads_default (anvilml-core)

**File:** `crates/anvilml-core/tests/config_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `serde` (derive feature) providing `Serialize` and `Deserialize` derives for `ServerConfig`.
**Tests:** `ServerConfig::default().num_threads` is `None` (auto = num_cpus).
**Mode:** both
**Inputs:** `ServerConfig::default()` constructed with compiled-in defaults.
**Expected output:** `num_threads.is_none()` is true.
**Acceptance:** `cargo test -p anvilml-core --test config_tests test_num_threads_default` exits 0.

---

## test_model_dirs_default (anvilml-core)

**File:** `crates/anvilml-core/tests/config_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `serde` (derive feature) providing `Serialize` and `Deserialize` derives for `ServerConfig`.
**Tests:** `ServerConfig::default().model_dirs` is an empty vec.
**Mode:** both
**Inputs:** `ServerConfig::default()` constructed with compiled-in defaults.
**Expected output:** `model_dirs.is_empty()` is true.
**Acceptance:** `cargo test -p anvilml-core --test config_tests test_model_dirs_default` exits 0.

---

## test_gpu_selection_default (anvilml-core)

**File:** `crates/anvilml-core/tests/config_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `serde` (derive feature) providing `Serialize` and `Deserialize` derives for `ServerConfig`.
**Tests:** `ServerConfig::default().gpu_selection.default_device` equals `"auto"`.
**Mode:** both
**Inputs:** `ServerConfig::default()` constructed with compiled-in defaults.
**Expected output:** `gpu_selection.default_device == "auto"`.
**Acceptance:** `cargo test -p anvilml-core --test config_tests test_gpu_selection_default` exits 0.

---

## test_limits_default (anvilml-core)

**File:** `crates/anvilml-core/tests/config_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `serde` (derive feature) providing `Serialize` and `Deserialize` derives for `ServerConfig`.
**Tests:** `ServerConfig::default().limits.max_queued_jobs` equals `100`.
**Mode:** both
**Inputs:** `ServerConfig::default()` constructed with compiled-in defaults.
**Expected output:** `limits.max_queued_jobs == 100`.
**Acceptance:** `cargo test -p anvilml-core --test config_tests test_limits_default` exits 0.

---

## test_rocm_default (anvilml-core)

**File:** `crates/anvilml-core/tests/config_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `serde` (derive feature) providing `Serialize` and `Deserialize` derives for `ServerConfig`.
**Tests:** `ServerConfig::default().rocm` is `None`.
**Mode:** both
**Inputs:** `ServerConfig::default()` constructed with compiled-in defaults.
**Expected output:** `rocm.is_none()` is true.
**Acceptance:** `cargo test -p anvilml-core --test config_tests test_rocm_default` exits 0.

---

## test_hardware_override_default (anvilml-core)

**File:** `crates/anvilml-core/tests/config_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `serde` (derive feature) providing `Serialize` and `Deserialize` derives for `ServerConfig`.
**Tests:** `ServerConfig::default().hardware_override` is `None`.
**Mode:** both
**Inputs:** `ServerConfig::default()` constructed with compiled-in defaults.
**Expected output:** `hardware_override.is_none()` is true.
**Acceptance:** `cargo test -p anvilml-core --test config_tests test_hardware_override_default` exits 0.

---

## test_load_missing_file_falls_back_to_defaults (anvilml-core)

**File:** `crates/anvilml-core/tests/config_load_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `serde` (derive feature) and `toml` dependencies providing `ServerConfig::default()` and `config_load::load()`.
**Tests:** `load(Some(Path::new("/nonexistent.toml")))` returns `Ok(ServerConfig::default())` — every field matches the compiled-in default.
**Mode:** both
**Inputs:** `load(Some(Path::new("/nonexistent/path.toml")))` with a nonexistent file path.
**Expected output:** `Ok(ServerConfig::default())` — all 13 fields match defaults exactly.
**Acceptance:** `cargo test -p anvilml-core --test config_load_tests test_load_missing_file_falls_back_to_defaults` exits 0.

---

## test_load_partial_toml_overrides_only_specified_fields (anvilml-core)

**File:** `crates/anvilml-core/tests/config_load_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `serde` and `toml` dependencies. A temporary TOML file is created with only `host` and `port` fields.
**Tests:** A TOML file with two fields overrides only those two fields; all other fields (including nested structs) retain their default values.
**Mode:** both
**Inputs:** Temporary TOML with `host = "0.0.0.0"` and `port = 9999`.
**Expected output:** `host == "0.0.0.0"`, `port == 9999`, all other fields == defaults.
**Acceptance:** `cargo test -p anvilml-core --test config_load_tests test_load_partial_toml_overrides_only_specified_fields` exits 0.

---

## test_load_malformed_toml_returns_err (anvilml-core)

**File:** `crates/anvilml-core/tests/config_load_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `serde` and `toml` dependencies. A temporary TOML file is created with invalid syntax (trailing comma).
**Tests:** Malformed TOML returns `Err(AnvilError::Serde(_))` — the error variant correctly identifies a deserialization failure.
**Mode:** both
**Inputs:** Temporary TOML with trailing comma (`host = "127.0.0.1",`).
**Expected output:** `Err(AnvilError::Serde(_))`.
**Acceptance:** `cargo test -p anvilml-core --test config_load_tests test_load_malformed_toml_returns_err` exits 0.

---

## test_load_full_toml_roundtrips_all_fields (anvilml-core)

**File:** `crates/anvilml-core/tests/config_load_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `serde` and `toml` dependencies. A temporary TOML file is created with every `ServerConfig` field set to a non-default value.
**Tests:** A TOML file with all fields set produces a `ServerConfig` where every loaded field matches the TOML values exactly — proves the merge covers all fields including nested structs and optional sections.
**Mode:** both
**Inputs:** Temporary TOML with all fields at non-default values (host, port, db_path, artifact_dir, venv_path, model_scan_depth, max_ipc_payload_mib, num_threads, model_dirs array, gpu_selection, limits, rocm, hardware_override).
**Expected output:** Every field matches the TOML values exactly.
**Acceptance:** `cargo test -p anvilml-core --test config_load_tests test_load_full_toml_roundtrips_all_fields` exits 0.

---

## test_load_default_path_resolves_anvilml_toml (anvilml-core)

**File:** `crates/anvilml-core/tests/config_load_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `serde` and `toml` dependencies. The checked-in `./anvilml.toml` at the repo root contains only `host` and `port` fields.
**Tests:** `load(None)` resolves to the default `./anvilml.toml` path and loads the two present fields; all other fields retain defaults.
**Mode:** both
**Inputs:** `load(None)` — uses default `./anvilml.toml` relative to CWD.
**Expected output:** `host == "127.0.0.1"`, `port == 8488`, all other fields == defaults.
**Acceptance:** `cargo test -p anvilml-core --test config_load_tests test_load_default_path_resolves_anvilml_toml` exits 0.

---

## test_load_nested_struct_partial_override (anvilml-core)

**File:** `crates/anvilml-core/tests/config_load_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `serde` and `toml` dependencies. A temporary TOML file is created with only a `[gpu_selection]` section.
**Tests:** A TOML with only `[gpu_selection]` overrides only `gpu_selection.default_device`; all other nested structs retain their default values.
**Mode:** both
**Inputs:** Temporary TOML with `[gpu_selection]` section only (`default_device = "cpu"`).
**Expected output:** `gpu_selection.default_device == "cpu"`, all other nested fields == defaults.
**Acceptance:** `cargo test -p anvilml-core --test config_load_tests test_load_nested_struct_partial_override` exits 0.

---

## test_env_var_overrides_toml_value (anvilml-core)

**File:** `crates/anvilml-core/tests/config_load_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `serde`, `toml`, and `serial_test` dev-dependencies. A temporary TOML file is created with `host = "0.0.0.0"`, and `ANVILML_HOST` is set to `"10.0.0.1"`.
**Tests:** The `ANVILML_HOST` environment variable overrides a TOML-set `host` value, proving env vars (layer 3) beat TOML (layer 2).
**Mode:** both
**Inputs:** Temporary TOML with `host = "0.0.0.0"`, env var `ANVILML_HOST = "10.0.0.1"`.
**Expected output:** `config.host == "10.0.0.1"` (env var overrides TOML).
**Acceptance:** `cargo test -p anvilml-core --test config_load_tests test_env_var_overrides_toml_value` exits 0.

---

## test_env_var_overrides_default_no_toml (anvilml-core)

**File:** `crates/anvilml-core/tests/config_load_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `serial_test` dev-dependency. `ANVILML_PORT` is set to `"9999"`, and a nonexistent TOML path is passed.
**Tests:** The `ANVILML_PORT` environment variable overrides the compiled-in default when no TOML file is present.
**Mode:** both
**Inputs:** Nonexistent TOML path, env var `ANVILML_PORT = "9999"`.
**Expected output:** `config.port == 9999` (env var overrides default).
**Acceptance:** `cargo test -p anvilml-core --test config_load_tests test_env_var_overrides_default_no_toml` exits 0.

---

## test_cli_override_beats_env_var (anvilml-core)

**File:** `crates/anvilml-core/tests/config_load_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `serial_test` dev-dependency. `ANVILML_HOST` is set to `"10.0.0.1"`, and `CliOverrides { host: Some("127.0.0.2") }` is passed.
**Tests:** CLI flag overrides beat environment variable overrides, proving CLI (layer 4) beats env vars (layer 3).
**Mode:** both
**Inputs:** Nonexistent TOML path, env var `ANVILML_HOST = "10.0.0.1"`, `CliOverrides { host: Some("127.0.0.2"), port: None }`.
**Expected output:** `config.host == "127.0.0.2"` (CLI override beats env var).
**Acceptance:** `cargo test -p anvilml-core --test config_load_tests test_cli_override_beats_env_var` exits 0.

---

## test_nested_env_var_gpu_selection (anvilml-core)

**File:** `crates/anvilml-core/tests/config_load_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `serial_test` dev-dependency. `ANVILML_GPU_SELECTION__DEFAULT_DEVICE` is set to `"cuda"`.
**Tests:** The `__` nested-field convention correctly parses `ANVILML_GPU_SELECTION__DEFAULT_DEVICE` into `gpu_selection.default_device`.
**Mode:** both
**Inputs:** Nonexistent TOML path, env var `ANVILML_GPU_SELECTION__DEFAULT_DEVICE = "cuda"`.
**Expected output:** `config.gpu_selection.default_device == "cuda"`.
**Acceptance:** `cargo test -p anvilml-core --test config_load_tests test_nested_env_var_gpu_selection` exits 0.

---

## test_unset_env_vars_leave_prior_layer_value (anvilml-core)

**File:** `crates/anvilml-core/tests/config_load_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `serial_test` dev-dependency. A temporary TOML file has `host = "0.0.0.0"`, and `ANVILML_HOST` is explicitly unset.
**Tests:** Unset `ANVILML_HOST` preserves the TOML-set value, proving unset env vars leave the prior layer intact.
**Mode:** both
**Inputs:** Temporary TOML with `host = "0.0.0.0"`, `ANVILML_HOST` unset.
**Expected output:** `config.host == "0.0.0.0"` (TOML value preserved).
**Acceptance:** `cargo test -p anvilml-core --test config_load_tests test_unset_env_vars_leave_prior_layer_value` exits 0.

---

## test_env_var_port_override (anvilml-core)

**File:** `crates/anvilml-core/tests/config_load_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `serial_test` dev-dependency. `ANVILML_PORT` is set to `"7777"`.
**Tests:** `ANVILML_PORT` env var parses as `u16` correctly and overrides the default port.
**Mode:** both
**Inputs:** Nonexistent TOML path, env var `ANVILML_PORT = "7777"`.
**Expected output:** `config.port == 7777`.
**Acceptance:** `cargo test -p anvilml-core --test config_load_tests test_env_var_port_override` exits 0.

---

## test_num_threads_env_var (anvilml-core)

**File:** `crates/anvilml-core/tests/config_load_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `serial_test` dev-dependency. `ANVILML_NUM_THREADS` is set to `"4"`.
**Tests:** `ANVILML_NUM_THREADS` env var parses as `Option<u32>` correctly and overrides the default.
**Mode:** both
**Inputs:** Nonexistent TOML path, env var `ANVILML_NUM_THREADS = "4"`.
**Expected output:** `config.num_threads == Some(4)`.
**Acceptance:** `cargo test -p anvilml-core --test config_load_tests test_num_threads_env_var` exits 0.

---

## config_reference_matches_defaults (backend)

**File:** `backend/tests/config_reference.rs`
**Context:** The `anvilml-core` crate has been compiled with `serde` and `toml` dependencies, and `anvilml.toml` at the repo root contains all `ServerConfig` fields at their documented defaults.
**Tests:** `config_load::load(Some(Path::new("../anvilml.toml")), None)` loads the repo-root config and asserts every field matches `ServerConfig::default()` — scalar fields (`host`, `port`, `db_path`, `artifact_dir`, `venv_path`, `model_scan_depth`, `max_ipc_payload_mib`, `num_threads`) and nested/optional fields (`model_dirs.is_empty()`, `gpu_selection.default_device == "auto"`, `limits.max_queued_jobs == 100`, `rocm.is_none()`, `hardware_override.is_none()`).
**Mode:** both
**Inputs:** `load(Some(Path::new("../anvilml.toml")), None)` — loads the checked-in `anvilml.toml` from the repo root.
**Expected output:** `Ok(config)` where all 13 fields match `ServerConfig::default()` exactly.
**Acceptance:** `cargo test -p anvilml --features mock-hardware -- config_reference` exits 0.
