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

## hw_probe_help_shows_subcommand (backend)

**File:** `backend/tests/hw_probe_help_test.rs`
**Context:** The `anvilml` binary has been compiled (`cargo build -p anvilml`).
**Tests:** The `hw-probe --help` output contains the "hw-probe" subcommand name, confirming the subcommand was registered with clap.
**Mode:** both
**Inputs:** `hw-probe --help` passed to the compiled binary.
**Expected output:** The help text includes "hw-probe" in the usage line or description.
**Acceptance:** `cargo test -p anvilml --test hw_probe_help_test` exits 0.

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
**Context:** The `anvilml-server` crate has been compiled with `serde` (derive) and `serde_json` dev-dependency. `build_router()` accepts an `Instant` argument for uptime tracking.
**Tests:** `GET /health` returns `200 OK` with a JSON body matching `ANVILML_DESIGN.md §13.4` — constructs a `GET /health` request, sends it through `build_router(start)`, asserts status is `StatusCode::OK`, then parses the body as JSON and asserts `status == "ok"`, `version` is a string, and `uptime_s` is a valid non-negative integer.
**Mode:** both
**Inputs:** `GET /health` with empty body; `build_router()` called with a freshly-captured `Instant`.
**Expected output:** `StatusCode::OK`; JSON body `{ "status": "ok", "version": "<semver>", "uptime_s": <uint> }`.
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

## test_job_serde_roundtrip (anvilml-core)

**File:** `crates/anvilml-core/tests/job_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `chrono` (serde feature), `uuid` (v4, serde), `serde_json`, and `serde` (derive) dependencies, and the `types` submodule providing `Job`, `JobStatus`, and `JobSettings`.
**Tests:** A `Job` with all fields populated (UUID, `JobStatus::Queued`, graph JSON, `JobSettings { device_preference: Some("cuda") }`, timestamps, `worker_id`, `error`, `queue_position`) serialises to JSON and deserialises back to an equal value. The JSON payload is also parsed to verify field names and values.
**Mode:** both
**Inputs:** `Job` constructed with all fields at non-default values.
**Expected output:** Roundtripped `Job` equals original; JSON contains `"status": "queued"`, `"device_preference": "cuda"`, and valid UUID.
**Acceptance:** `cargo test -p anvilml-core --test job_tests test_job_serde_roundtrip` exits 0.

---

## test_job_status_all_variants_roundtrip (anvilml-core)

**File:** `crates/anvilml-core/tests/job_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `chrono` (serde feature), `serde_json`, and `serde` (derive) dependencies, and the `types` submodule providing `JobStatus`.
**Tests:** Each of the five `JobStatus` variants (`Queued`, `Running`, `Completed`, `Failed`, `Cancelled`) serialises to a lowercase snake_case JSON string and deserialises back to an equal value.
**Mode:** both
**Inputs:** All five `JobStatus` variants.
**Expected output:** Each variant roundtrips correctly; JSON strings are `"queued"`, `"running"`, `"completed"`, `"failed"`, `"cancelled"`.
**Acceptance:** `cargo test -p anvilml-core --test job_tests test_job_status_all_variants_roundtrip` exits 0.

---

## test_job_settings_default (anvilml-core)

**File:** `crates/anvilml-core/tests/job_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `chrono` (serde feature), `serde_json`, and `serde` (derive) dependencies, and the `types` submodule providing `JobSettings`.
**Tests:** A `JobSettings` with `device_preference: None` serialises to JSON containing `"device_preference": null` and roundtrips correctly.
**Mode:** both
**Inputs:** `JobSettings { device_preference: None }`.
**Expected output:** JSON contains null for `device_preference`; roundtripped `JobSettings` equals original.
**Acceptance:** `cargo test -p anvilml-core --test job_tests test_job_settings_default` exits 0.

---

## test_job_with_nulls_roundtrip (anvilml-core)

**File:** `crates/anvilml-core/tests/job_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `chrono` (serde feature), `uuid` (v4, serde), `serde_json`, and `serde` (derive) dependencies, and the `types` submodule providing `Job`.
**Tests:** A `Job` with all `Option` fields (`started_at`, `completed_at`, `worker_id`, `error`, `queue_position`) set to `None` serialises to JSON and deserialises back, confirming all `None` fields remain `None` after the roundtrip.
**Mode:** both
**Inputs:** `Job` with `started_at: None`, `completed_at: None`, `worker_id: None`, `error: None`, `queue_position: None`.
**Expected output:** All `None` fields remain `None` after roundtrip; non-null fields unchanged.
**Acceptance:** `cargo test -p anvilml-core --test job_tests test_job_with_nulls_roundtrip` exits 0.

---

## test_model_kind_serde_snake_case (anvilml-core)

**File:** `crates/anvilml-core/tests/model_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `chrono` (serde feature), `serde` (derive), and `serde_json` dependencies, and the `types` submodule providing `ModelKind`.
**Tests:** Each of the seven `ModelKind` variants (`Diffusion`, `TextEncoder`, `Vae`, `Lora`, `ControlNet`, `Upscale`, `Unknown`) serialises to a lowercase snake_case JSON string and deserialises back to an equal value.
**Mode:** both
**Inputs:** All seven `ModelKind` variants.
**Expected output:** Each variant roundtrips correctly; JSON strings are `"diffusion"`, `"text_encoder"`, `"vae"`, `"lora"`, `"control_net"`, `"upscale"`, `"unknown"`.
**Acceptance:** `cargo test -p anvilml-core --test model_tests test_model_kind_serde_snake_case` exits 0.

---

## test_model_dtype_serde_snake_case (anvilml-core)

**File:** `crates/anvilml-core/tests/model_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `chrono` (serde feature), `serde` (derive), and `serde_json` dependencies, and the `types` submodule providing `ModelDtype`.
**Tests:** Each of the six `ModelDtype` variants (`Fp32`, `Fp16`, `Bf16`, `Fp8`, `Fp4`, `Unknown`) serialises to a lowercase snake_case JSON string and deserialises back to an equal value.
**Mode:** both
**Inputs:** All six `ModelDtype` variants.
**Expected output:** Each variant roundtrips correctly; JSON strings are `"fp32"`, `"fp16"`, `"bf16"`, `"fp8"`, `"fp4"`, `"unknown"`.
**Acceptance:** `cargo test -p anvilml-core --test model_tests test_model_dtype_serde_snake_case` exits 0.

---

## test_model_format_serde_snake_case (anvilml-core)

**File:** `crates/anvilml-core/tests/model_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `chrono` (serde feature), `serde` (derive), and `serde_json` dependencies, and the `types` submodule providing `ModelFormat`.
**Tests:** Each of the five `ModelFormat` variants (`Safetensors`, `Ckpt`, `Pt`, `Bin`, `Unknown`) serialises to a lowercase snake_case JSON string and deserialises back to an equal value.
**Mode:** both
**Inputs:** All five `ModelFormat` variants.
**Expected output:** Each variant roundtrips correctly; JSON strings are `"safetensors"`, `"ckpt"`, `"pt"`, `"bin"`, `"unknown"`.
**Acceptance:** `cargo test -p anvilml-core --test model_tests test_model_format_serde_snake_case` exits 0.

---

## test_model_meta_serde_roundtrip (anvilml-core)

**File:** `crates/anvilml-core/tests/model_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `chrono` (serde feature), `uuid` (v4, serde), `serde_json`, and `serde` (derive) dependencies, and the `types` submodule providing `ModelMeta`, `ModelKind`, `ModelDtype`, and `ModelFormat`.
**Tests:** A `ModelMeta` with all fields populated (string ID, name, `PathBuf` path, `ModelKind::Diffusion`, `ModelDtype::Fp16`, `ModelFormat::Safetensors`, size, timestamp) serialises to JSON and deserialises back to an equal value. The JSON payload is also parsed to verify field names, snake_case enum values, and `PathBuf` → `String` conversion.
**Mode:** both
**Inputs:** `ModelMeta` constructed with all fields at non-default values.
**Expected output:** Roundtripped `ModelMeta` equals original; JSON contains `"kind": "diffusion"`, `"dtype": "fp16"`, `"format": "safetensors"`, and `"path": "models/test.safetensors"`.
**Acceptance:** `cargo test -p anvilml-core --test model_tests test_model_meta_serde_roundtrip` exits 0.

---

## config_reference_matches_defaults (backend)

**File:** `backend/tests/config_reference.rs`
**Context:** The `anvilml-core` crate has been compiled with `serde` and `toml` dependencies, and `anvilml.toml` at the repo root contains all `ServerConfig` fields at their documented defaults.
**Tests:** `config_load::load(Some(Path::new("../anvilml.toml")), None)` loads the repo-root config and asserts every field matches `ServerConfig::default()` — scalar fields (`host`, `port`, `db_path`, `artifact_dir`, `venv_path`, `model_scan_depth`, `max_ipc_payload_mib`, `num_threads`) and nested/optional fields (`model_dirs.is_empty()`, `gpu_selection.default_device == "auto"`, `limits.max_queued_jobs == 100`, `rocm.is_none()`, `hardware_override.is_none()`).
**Mode:** both
**Inputs:** `load(Some(Path::new("../anvilml.toml")), None)` — loads the checked-in `anvilml.toml` from the repo root.
**Expected output:** `Ok(config)` where all 13 fields match `ServerConfig::default()` exactly.
**Acceptance:** `cargo test -p anvilml --features mock-hardware -- config_reference` exits 0.

---

## test_artifact_meta_serde_roundtrip (anvilml-core)

**File:** `crates/anvilml-core/tests/artifact_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `chrono` (serde feature), `uuid` (v4, serde), `serde_json`, `serde` (derive), and `utoipa` (uuid, chrono features) dependencies, and the `types` submodule providing `ArtifactMeta`.
**Tests:** A full `ArtifactMeta` with all fields populated (64-char SHA-256 hex hash, UUID, 1024×1024 pixels, seed 42, 30 steps, RFC 3339 timestamp, PNG file path) serialises to JSON and deserialises back to an equal value. The raw JSON is parsed to confirm all eight field names are present.
**Mode:** both
**Inputs:** `ArtifactMeta` constructed with all fields at non-default values.
**Expected output:** Roundtripped `ArtifactMeta` equals original; JSON contains all eight snake_case field names.
**Acceptance:** `cargo test -p anvilml-core --test artifact_tests test_artifact_meta_serde_roundtrip` exits 0.

---

## test_artifact_meta_hash_format (anvilml-core)

**File:** `crates/anvilml-core/tests/artifact_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `chrono` (serde feature), `uuid` (v4, serde), `serde_json`, `serde` (derive), and `utoipa` (uuid, chrono features) dependencies, and the `types` submodule providing `ArtifactMeta`.
**Tests:** A `ArtifactMeta` with a zeroed SHA-256 hex hash (64 `'0'` characters) roundtrips through serde JSON, proving the `hash` field — the primary key for artifact storage — survives serialisation byte-for-byte. The hash format is verified to be exactly 64 lowercase hex characters.
**Mode:** both
**Inputs:** `ArtifactMeta` with `hash = "0000...0000"` (64 zeros).
**Expected output:** Roundtripped hash equals original; hash is 64 ASCII hex characters.
**Acceptance:** `cargo test -p anvilml-core --test artifact_tests test_artifact_meta_hash_format` exits 0.

---

## test_artifact_meta_field_names (anvilml-core)

**File:** `crates/anvilml-core/tests/artifact_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `chrono` (serde feature), `uuid` (v4, serde), `serde_json`, `serde` (derive), and `utoipa` (uuid, chrono features) dependencies, and the `types` submodule providing `ArtifactMeta`.
**Tests:** The JSON output of `ArtifactMeta` contains all eight expected snake_case field names (`hash`, `job_id`, `width`, `height`, `seed`, `steps`, `created_at`, `file_path`) with the correct types (strings, numbers, RFC 3339 timestamp), and no unexpected fields are present.
**Mode:** both
**Inputs:** `ArtifactMeta` with negative seed (`-1`), mixed dimensions (768×1024), 50 steps.
**Expected output:** All eight fields present with correct types; exactly 8 keys in the JSON object.
**Acceptance:** `cargo test -p anvilml-core --test artifact_tests test_artifact_meta_field_names` exits 0.

---

## test_device_type_serde_snake_case (anvilml-core)

**File:** `crates/anvilml-core/tests/hardware_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `serde` (derive), `serde_json`, and `utoipa` dependencies, and the `types` submodule providing `DeviceType`.
**Tests:** Each of the three `DeviceType` variants (`Cuda`, `Rocm`, `Cpu`) serialises to a lowercase snake_case JSON string and deserialises back to an equal value.
**Mode:** both
**Inputs:** All three `DeviceType` variants.
**Expected output:** Each variant roundtrips correctly; JSON strings are `"cuda"`, `"rocm"`, `"cpu"`.
**Acceptance:** `cargo test -p anvilml-core --test hardware_tests test_device_type_serde_snake_case` exits 0.

---

## test_host_info_serde_roundtrip (anvilml-core)

**File:** `crates/anvilml-core/tests/hardware_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `serde` (derive), `serde_json`, and `utoipa` dependencies, and the `types` submodule providing `HostInfo`.
**Tests:** A `HostInfo` with populated fields (`hostname: "testhost"`, `os: "Linux"`) serialises to JSON and deserialises back to an equal value. The JSON payload is also parsed to verify field names.
**Mode:** both
**Inputs:** `HostInfo` constructed with `hostname = "testhost"`, `os = "Linux"`.
**Expected output:** Roundtripped `HostInfo` equals original; JSON contains `"hostname": "testhost"` and `"os": "Linux"`.
**Acceptance:** `cargo test -p anvilml-core --test hardware_tests test_host_info_serde_roundtrip` exits 0.

---

## test_gpu_device_construction_and_serde (anvilml-core)

**File:** `crates/anvilml-core/tests/hardware_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `serde` (derive), `serde_json`, and `utoipa` dependencies, and the `types` submodule providing `GpuDevice` and all its nested types.
**Tests:** A `GpuDevice` with all 12 fields populated (index, name, `DeviceType::Cuda`, VRAM, driver version, PCI IDs, architecture, `InferenceCaps`, `EnumerationSource`, `CapabilitySource`) serialises to JSON and deserialises back to an equal value. The JSON payload is also parsed to verify field names and nested structure.
**Mode:** both
**Inputs:** Full `GpuDevice` with all fields at non-default values.
**Expected output:** Roundtripped `GpuDevice` equals original; JSON contains all 12 snake_case field names with correct types.
**Acceptance:** `cargo test -p anvilml-core --test hardware_tests test_gpu_device_construction_and_serde` exits 0.

---

## test_hardware_info_serde_roundtrip (anvilml-core)

**File:** `crates/anvilml-core/tests/hardware_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `serde` (derive), `serde_json`, and `utoipa` dependencies, and the `types` submodule providing `HardwareInfo` with nested `HostInfo`, `Vec<GpuDevice>`, and `InferenceCaps`.
**Tests:** A `HardwareInfo` with a `HostInfo`, a vector of two `GpuDevice` entries, and an `InferenceCaps` serialises to JSON and deserialises back to an equal value. The JSON payload is parsed to verify nested structure and array length.
**Mode:** both
**Inputs:** `HardwareInfo` with 2 GPUs (RTX 4090 + RTX 3080).
**Expected output:** Roundtripped `HardwareInfo` equals original; nested structures preserved; `gpus` array has 2 elements.
**Acceptance:** `cargo test -p anvilml-core --test hardware_tests test_hardware_info_serde_roundtrip` exits 0.

---

## test_inference_caps_default_roundtrip (anvilml-core)

**File:** `crates/anvilml-core/tests/hardware_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `serde` (derive), `serde_json`, and `utoipa` dependencies, and the `types` submodule providing `InferenceCaps`.
**Tests:** An `InferenceCaps` constructed via `Default` (all fields `false`) serialises to JSON and deserialises back to an equal value. The JSON payload is parsed to verify all fields are `false`.
**Mode:** both
**Inputs:** `InferenceCaps::default()` (all boolean fields `false`).
**Expected output:** Roundtripped `InferenceCaps` equals original; JSON contains `"fp32": false`, `"fp16": false`, `"bf16": false`, `"fp8": false`, `"fp4": false`, `"flash_attention": false`.
**Acceptance:** `cargo test -p anvilml-core --test hardware_tests test_inference_caps_default_roundtrip` exits 0.

---

## test_enumeration_source_serde_snake_case (anvilml-core)

**File:** `crates/anvilml-core/tests/hardware_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `serde` (derive), `serde_json`, and `utoipa` dependencies, and the `types` submodule providing `EnumerationSource`.
**Tests:** Each of the seven `EnumerationSource` variants (`Vulkan`, `Dxgi`, `Sysfs`, `Nvml`, `Cpu`, `Mock`, `Override`) serialises to a lowercase snake_case JSON string and deserialises back to an equal value.
**Mode:** both
**Inputs:** All seven `EnumerationSource` variants.
**Expected output:** Each variant roundtrips correctly; JSON strings are `"vulkan"`, `"dxgi"`, `"sysfs"`, `"nvml"`, `"cpu"`, `"mock"`, `"override"`.
**Acceptance:** `cargo test -p anvilml-core --test hardware_tests test_enumeration_source_serde_snake_case` exits 0.

---

## test_capability_source_serde_snake_case (anvilml-core)

**File:** `crates/anvilml-core/tests/hardware_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `serde` (derive), `serde_json`, and `utoipa` dependencies, and the `types` submodule providing `CapabilitySource`.
**Tests:** Each of the three `CapabilitySource` variants (`PyTorch`, `DeviceTable`, `Fallback`) serialises to a lowercase snake_case JSON string and deserialises back to an equal value. `PyTorch` uses a custom `#[serde(rename = "pytorch")]` to produce `"pytorch"` rather than `"py_torch"`.
**Mode:** both
**Inputs:** All three `CapabilitySource` variants.
**Expected output:** Each variant roundtrips correctly; JSON strings are `"pytorch"`, `"device_table"`, `"fallback"`.
**Acceptance:** `cargo test -p anvilml-core --test hardware_tests test_capability_source_serde_snake_case` exits 0.

---

## test_inference_caps_non_default_roundtrip (anvilml-core)

**File:** `crates/anvilml-core/tests/hardware_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `serde` (derive), `serde_json`, and `utoipa` dependencies, and the `types` submodule providing `InferenceCaps`.
**Tests:** An `InferenceCaps` with mixed true/false fields (`fp32: true, fp16: true, bf16: true, fp8: false, fp4: false, flash_attention: true`) serialises to JSON, roundtrips back to an equal value, and all six JSON field names (`fp32`, `fp16`, `bf16`, `fp8`, `fp4`, `flash_attention`) are verified via `serde_json::Value` parsing.
**Mode:** both
**Inputs:** `InferenceCaps { fp32: true, fp16: true, bf16: true, fp8: false, fp4: false, flash_attention: true }`.
**Expected output:** Roundtripped `InferenceCaps` equals original; JSON contains `"fp32": true`, `"fp16": true`, `"bf16": true`, `"fp8": false`, `"fp4": false`, `"flash_attention": true`.
**Acceptance:** `cargo test -p anvilml-core --test hardware_tests test_inference_caps_non_default_roundtrip` exits 0.

---

## test_enumeration_source_copy_trait (anvilml-core)

**File:** `crates/anvilml-core/tests/hardware_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `serde` (derive), `serde_json`, and `utoipa` dependencies, and the `types` submodule providing `EnumerationSource` and `CapabilitySource`.
**Tests:** Both `EnumerationSource` and `CapabilitySource` implement `Copy` — assigning a variant to a new variable does not move it, so both the original and the copy remain usable. Serialises both to JSON and asserts they produce identical output.
**Mode:** both
**Inputs:** `EnumerationSource::Cpu`, `CapabilitySource::PyTorch`.
**Expected output:** Both original and copy remain usable after assignment; both serialise identically.
**Acceptance:** `cargo test -p anvilml-core --test hardware_tests test_enumeration_source_copy_trait` exits 0.

---

## test_worker_info_construction_and_serde_roundtrip (anvilml-core)

**File:** `crates/anvilml-core/tests/worker_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `serde` (derive), `serde_json`, `utoipa` (uuid feature), `uuid` (v4, serde), and `chrono` (serde) dependencies, and the `types` submodule providing `WorkerInfo`, `WorkerStatus`, `DeviceType`, and `Uuid`.
**Tests:** A `WorkerInfo` with all fields populated (`worker_id="gpu:0"`, `status=Idle`, `device_index=0`, `device_type=Cuda`, `pid=Some(1234)`, `current_job_id=Some(Uuid::new_v4())`) serialises to JSON and deserialises back to an equal value. The JSON payload is also parsed to verify all six field names appear with the correct types.
**Mode:** both
**Inputs:** `WorkerInfo` constructed with all fields at non-default values.
**Expected output:** Roundtripped `WorkerInfo` equals original; JSON contains `"worker_id": "gpu:0"`, `"status": "idle"`, `"device_index": 0`, `"device_type": "cuda"`, `"pid": 1234`, `"current_job_id": "<uuid>"`.
**Acceptance:** `cargo test -p anvilml-core --test worker_tests test_worker_info_construction_and_serde_roundtrip` exits 0.

---

## test_worker_status_serde_snake_case (anvilml-core)

**File:** `crates/anvilml-core/tests/worker_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `serde` (derive), `serde_json`, and `utoipa` dependencies, and the `types` submodule providing `WorkerStatus`.
**Tests:** Each of the five `WorkerStatus` variants (`Spawning`, `Idle`, `Busy`, `Dying`, `Dead`) serialises to a lowercase snake_case JSON string and deserialises back to an equal value.
**Mode:** both
**Inputs:** All five `WorkerStatus` variants.
**Expected output:** Each variant roundtrips correctly; JSON strings are `"spawning"`, `"idle"`, `"busy"`, `"dying"`, `"dead"`.
**Acceptance:** `cargo test -p anvilml-core --test worker_tests test_worker_status_serde_snake_case` exits 0.

---

## test_provisioning_state_serde_snake_case (anvilml-core)

**File:** `crates/anvilml-core/tests/worker_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `serde` (derive), `serde_json`, and `utoipa` dependencies, and the `types` submodule providing `ProvisioningState`.
**Tests:** Each of the four `ProvisioningState` variants (`NotStarted`, `Provisioning`, `Ready`, `Failed`) serialises to a lowercase snake_case JSON string and deserialises back to an equal value.
**Mode:** both
**Inputs:** All four `ProvisioningState` variants.
**Expected output:** Each variant roundtrips correctly; JSON strings are `"not_started"`, `"provisioning"`, `"ready"`, `"failed"`.
**Acceptance:** `cargo test -p anvilml-core --test worker_tests test_provisioning_state_serde_snake_case` exits 0.

---

## test_env_report_serde_roundtrip (anvilml-core)

**File:** `crates/anvilml-core/tests/worker_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `serde` (derive), `serde_json`, `utoipa`, and `uuid` dependencies, and the `types` submodule providing `EnvReport`, `ProvisioningState`, and `NodeTypeDescriptor`.
**Tests:** An `EnvReport` with all 7 fields set (`python_path=Some("/usr/bin/python3")`, `python_version=Some("3.12.3")`, `torch_version=Some("2.5.1")`, `provisioning=NotStarted`, `preflight_ok=true`, `reason=None`, `node_types=[LoadModel]`) serialises to JSON and deserialises back to an equal value. The JSON payload is also parsed to verify all seven field names (`python_path`, `python_version`, `torch_version`, `provisioning`, `preflight_ok`, `reason`, `node_types`) appear with the correct types.
**Mode:** both
**Inputs:** `EnvReport` constructed with all 7 fields at non-default values.
**Expected output:** Roundtripped `EnvReport` equals original; JSON contains `"python_path": "/usr/bin/python3"`, `"python_version": "3.12.3"`, `"torch_version": "2.5.1"`, `"provisioning": "not_started"`, `"preflight_ok": true`, `"reason": null`, `"node_types": [...]`.
**Acceptance:** `cargo test -p anvilml-core --test worker_tests test_env_report_serde_roundtrip` exits 0.

---

## test_slot_type_screaming_snake_case_serde (anvilml-core)

**File:** `crates/anvilml-core/tests/node_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `serde` (derive), `serde_json`, and `utoipa` (uuid, chrono features) dependencies, and the `types` submodule providing `SlotType`.
**Tests:** Each of the eleven `SlotType` variants (`Model`, `Clip`, `Vae`, `Conditioning`, `Latent`, `Image`, `String`, `Int`, `Float`, `Bool`, `Any`) serialises to a `SCREAMING_SNAKE_CASE` JSON string and deserialises back to an equal value.
**Mode:** both
**Inputs:** All eleven `SlotType` variants.
**Expected output:** Each variant roundtrips correctly; JSON strings are `"MODEL"`, `"CLIP"`, `"VAE"`, `"CONDITIONING"`, `"LATENT"`, `"IMAGE"`, `"STRING"`, `"INT"`, `"FLOAT"`, `"BOOL"`, `"ANY"`.
**Acceptance:** `cargo test -p anvilml-core --test node_tests test_slot_type_screaming_snake_case_serde` exits 0.

---

## test_slot_descriptor_serde_roundtrip (anvilml-core)

**File:** `crates/anvilml-core/tests/node_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `serde` (derive), `serde_json`, and `utoipa` dependencies, and the `types` submodule providing `SlotDescriptor` and `SlotType`.
**Tests:** A `SlotDescriptor` with a required slot (`optional: false`) and an optional slot (`optional: true`) both serialise to JSON with the correct field names (`name`, `slot_type`, `optional`) and roundtrip back to equal values.
**Mode:** both
**Inputs:** `SlotDescriptor` with `name="positive"`, `slot_type=Conditioning`, `optional=false`; and `SlotDescriptor` with `name="seed"`, `slot_type=Int`, `optional=true`.
**Expected output:** Both descriptors roundtrip correctly; JSON contains `"name"`, `"slot_type"`, and `"optional"` fields.
**Acceptance:** `cargo test -p anvilml-core --test node_tests test_slot_descriptor_serde_roundtrip` exits 0.

---

## test_node_type_descriptor_construction (anvilml-core)

**File:** `crates/anvilml-core/tests/node_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `serde` (derive), `serde_json`, and `utoipa` dependencies, and the `types` submodule providing `NodeTypeDescriptor`, `SlotDescriptor`, and `SlotType`.
**Tests:** A `NodeTypeDescriptor` modelled after `LoadModel` — one required `model_id` input and one `MODEL` output — serialises to JSON, roundtrips back to an equal value, and contains all expected top-level field names (`type_name`, `display_name`, `category`, `description`, `inputs` array, `outputs` array).
**Mode:** both
**Inputs:** `NodeTypeDescriptor` with `type_name="LoadModel"`, one `String` input slot, one `Model` output slot.
**Expected output:** Roundtripped `NodeTypeDescriptor` equals original; JSON contains all six top-level fields with correct types.
**Acceptance:** `cargo test -p anvilml-core --test node_tests test_node_type_descriptor_construction` exits 0.

---

## test_node_type_descriptor_empty_slots (anvilml-core)

**File:** `crates/anvilml-core/tests/node_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `serde` (derive), `serde_json`, and `utoipa` dependencies, and the `types` submodule providing `NodeTypeDescriptor`, `SlotDescriptor`, and `SlotType`.
**Tests:** A `NodeTypeDescriptor` with empty `inputs` and `outputs` vectors serialises to JSON containing `"inputs": []` and `"outputs": []`, roundtrips back to an equal value, proving the edge case of a node with no slots is handled correctly.
**Mode:** both
**Inputs:** `NodeTypeDescriptor` with `inputs: vec![]` and `outputs: vec![]`.
**Expected output:** JSON contains empty arrays for `inputs` and `outputs`; roundtripped `NodeTypeDescriptor` equals original.
**Acceptance:** `cargo test -p anvilml-core --test node_tests test_node_type_descriptor_empty_slots` exits 0.

---

## test_ws_event_job_queued_serde_roundtrip (anvilml-core)

**File:** `crates/anvilml-core/tests/events_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `serde` (derive), `serde_json`, `utoipa` (uuid, chrono features), and `uuid` (v4, serde) dependencies, and the `types` submodule providing `WsEvent`.
**Tests:** A `WsEvent::JobQueued` with `job_id = "550e8400-e29b-41d4-a716-446655440000"` and `queue_position = 3` serialises to JSON containing `"type": "job_queued"`, all fields roundtrip, and the tag key is `"type"` (not a variant-name key).
**Mode:** both
**Inputs:** `WsEvent::JobQueued { job_id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(), queue_position: 3 }`.
**Expected output:** JSON contains `"type":"job_queued"`, `"job_id":"550e8400-e29b-41d4-a716-446655440000"`, `"queue_position":3`; roundtripped `WsEvent` equals original.
**Acceptance:** `cargo test -p anvilml-core --test events_tests test_ws_event_job_queued_serde_roundtrip` exits 0.

---

## test_ws_event_job_started_serde_roundtrip (anvilml-core)

**File:** `crates/anvilml-core/tests/events_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `serde` (derive), `serde_json`, `utoipa` (uuid, chrono features), and `uuid` (v4, serde) dependencies, and the `types` submodule providing `WsEvent`.
**Tests:** A `WsEvent::JobStarted` with `job_id = "550e8400-e29b-41d4-a716-446655440000"` and `worker_id = "gpu:0"` serialises to JSON containing `"type": "job_started"`, all fields roundtrip, and the tag key is `"type"`.
**Mode:** both
**Inputs:** `WsEvent::JobStarted { job_id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(), worker_id: "gpu:0".to_string() }`.
**Expected output:** JSON contains `"type":"job_started"`, `"worker_id":"gpu:0"`; roundtripped `WsEvent` equals original.
**Acceptance:** `cargo test -p anvilml-core --test events_tests test_ws_event_job_started_serde_roundtrip` exits 0.

---

## test_ws_event_job_progress_serde_roundtrip (anvilml-core)

**File:** `crates/anvilml-core/tests/events_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `serde` (derive), `serde_json`, `utoipa` (uuid, chrono features), and `uuid` (v4, serde) dependencies, and the `types` submodule providing `WsEvent`.
**Tests:** A `WsEvent::JobProgress` with `step = 3`, `total_steps = 20`, and `preview_b64 = None` serialises to JSON containing `"type": "job_progress"`, all fields roundtrip including the null `preview_b64`, and the tag key is `"type"`.
**Mode:** both
**Inputs:** `WsEvent::JobProgress { job_id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(), step: 3, total_steps: 20, preview_b64: None }`.
**Expected output:** JSON contains `"type":"job_progress"`, `"step":3`, `"total_steps":20`, `"preview_b64":null`; roundtripped `WsEvent` equals original.
**Acceptance:** `cargo test -p anvilml-core --test events_tests test_ws_event_job_progress_serde_roundtrip` exits 0.

---

## test_ws_event_job_image_ready_serde_roundtrip (anvilml-core)

**File:** `crates/anvilml-core/tests/events_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `serde` (derive), `serde_json`, `utoipa` (uuid, chrono features), and `uuid` (v4, serde) dependencies, and the `types` submodule providing `WsEvent`.
**Tests:** A `WsEvent::JobImageReady` with `artifact_hash = "abc123def456"`, `width = 512`, `height = 512`, `seed = 42`, `steps = 20` serialises to JSON containing `"type": "job_image_ready"`, all fields roundtrip including `seed: i64`, and the tag key is `"type"`.
**Mode:** both
**Inputs:** `WsEvent::JobImageReady { job_id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(), artifact_hash: "abc123def456".to_string(), width: 512, height: 512, seed: 42, steps: 20 }`.
**Expected output:** JSON contains `"type":"job_image_ready"`, `"seed":42`, `"steps":20`; roundtripped `WsEvent` equals original.
**Acceptance:** `cargo test -p anvilml-core --test events_tests test_ws_event_job_image_ready_serde_roundtrip` exits 0.

---

## test_ws_event_job_completed_serde_roundtrip (anvilml-core)

**File:** `crates/anvilml-core/tests/events_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `serde` (derive), `serde_json`, `utoipa` (uuid, chrono features), and `uuid` (v4, serde) dependencies, and the `types` submodule providing `WsEvent`.
**Tests:** A `WsEvent::JobCompleted` with `elapsed_ms = 15000` serialises to JSON containing `"type": "job_completed"`, `elapsed_ms: u64` roundtrips, and the tag key is `"type"`.
**Mode:** both
**Inputs:** `WsEvent::JobCompleted { job_id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(), elapsed_ms: 15000 }`.
**Expected output:** JSON contains `"type":"job_completed"`, `"elapsed_ms":15000`; roundtripped `WsEvent` equals original.
**Acceptance:** `cargo test -p anvilml-core --test events_tests test_ws_event_job_completed_serde_roundtrip` exits 0.

---

## test_ws_event_job_failed_serde_roundtrip (anvilml-core)

**File:** `crates/anvilml-core/tests/events_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `serde` (derive), `serde_json`, `utoipa` (uuid, chrono features), and `uuid` (v4, serde) dependencies, and the `types` submodule providing `WsEvent`.
**Tests:** A `WsEvent::JobFailed` with `error = "CUDA out of memory"` serialises to JSON containing `"type": "job_failed"`, the error string roundtrips, and the tag key is `"type"`.
**Mode:** both
**Inputs:** `WsEvent::JobFailed { job_id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(), error: "CUDA out of memory".to_string() }`.
**Expected output:** JSON contains `"type":"job_failed"`, `"error":"CUDA out of memory"`; roundtripped `WsEvent` equals original.
**Acceptance:** `cargo test -p anvilml-core --test events_tests test_ws_event_job_failed_serde_roundtrip` exits 0.

---

## test_ws_event_job_cancelled_serde_roundtrip (anvilml-core)

**File:** `crates/anvilml-core/tests/events_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `serde` (derive), `serde_json`, `utoipa` (uuid, chrono features), and `uuid` (v4, serde) dependencies, and the `types` submodule providing `WsEvent`.
**Tests:** A `WsEvent::JobCancelled` with a single `job_id` field serialises to JSON containing `"type": "job_cancelled"`, the `job_id` roundtrips, and the tag key is `"type"`.
**Mode:** both
**Inputs:** `WsEvent::JobCancelled { job_id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap() }`.
**Expected output:** JSON contains `"type":"job_cancelled"`, `"job_id":"550e8400-e29b-41d4-a716-446655440000"`; roundtripped `WsEvent` equals original.
**Acceptance:** `cargo test -p anvilml-core --test events_tests test_ws_event_job_cancelled_serde_roundtrip` exits 0.

---

## test_ws_event_worker_status_changed_serde_roundtrip (anvilml-core)

**File:** `crates/anvilml-core/tests/events_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `serde` (derive), `serde_json`, `utoipa` (uuid, chrono features), and `uuid` (v4, serde) dependencies, and the `types` submodule providing `WsEvent`, `WorkerStatus`, `WorkerInfo`, and `DeviceType`.
**Tests:** A `WsEvent::WorkerStatusChanged` with `worker_id = "gpu:0"`, `status = Busy`, and `device_index = 0` serialises to JSON containing `"type": "worker_status_changed"`, all fields roundtrip, and the tag key is `"type"`.
**Mode:** both
**Inputs:** `WsEvent::WorkerStatusChanged { worker_id: "gpu:0".to_string(), status: WorkerStatus::Busy, device_index: 0 }`.
**Expected output:** JSON contains `"type":"worker_status_changed"`, `"worker_id":"gpu:0"`, `"status":"busy"`, `"device_index":0`; roundtripped `WsEvent` equals original.
**Acceptance:** `cargo test -p anvilml-core --test events_tests test_ws_event_worker_status_changed_serde_roundtrip` exits 0.

---

## test_ws_event_system_stats_serde_roundtrip (anvilml-core)

**File:** `crates/anvilml-core/tests/events_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `serde` (derive), `serde_json`, `utoipa` (uuid, chrono features), and `uuid` (v4, serde) dependencies, and the `types` submodule providing `WsEvent`, `WorkerStatus`, `WorkerInfo`, and `DeviceType`.
**Tests:** A `WsEvent::SystemStats` with `cpu_pct = 45.5`, `ram_used_mib = 512`, and a single `WorkerInfo` in the `workers` vec serialises to JSON containing `"type": "system_stats"`, all fields roundtrip including the nested `WorkerInfo` inside the `workers` array, and the tag key is `"type"`.
**Mode:** both
**Inputs:** `WsEvent::SystemStats { cpu_pct: 45.5, ram_used_mib: 512, workers: vec![WorkerInfo { worker_id: "0".to_string(), status: WorkerStatus::Idle, device_index: 0, device_type: DeviceType::Cpu, pid: None, current_job_id: None }] }`.
**Expected output:** JSON contains `"type":"system_stats"`, `"cpu_pct":45.5`, `"ram_used_mib":512`, `"workers"` array with 1 element; roundtripped `WsEvent` equals original.
**Acceptance:** `cargo test -p anvilml-core --test events_tests test_ws_event_system_stats_serde_roundtrip` exits 0.

---

## test_ws_event_provisioning_progress_serde_roundtrip (anvilml-core)

**File:** `crates/anvilml-core/tests/events_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `serde` (derive), `serde_json`, `utoipa` (uuid, chrono features), and `uuid` (v4, serde) dependencies, and the `types` submodule providing `WsEvent`.
**Tests:** A `WsEvent::ProvisioningProgress` with `message = "Installing torch"` and `pct = 50` serialises to JSON containing `"type": "provisioning_progress"`, all fields roundtrip, and the tag key is `"type"`.
**Mode:** both
**Inputs:** `WsEvent::ProvisioningProgress { message: "Installing torch".to_string(), pct: 50 }`.
**Expected output:** JSON contains `"type":"provisioning_progress"`, `"message":"Installing torch"`, `"pct":50`; roundtripped `WsEvent` equals original.
**Acceptance:** `cargo test -p anvilml-core --test events_tests test_ws_event_provisioning_progress_serde_roundtrip` exits 0.

---

## test_empty_registry_returns_none (anvilml-core)

**File:** `crates/anvilml-core/tests/node_registry_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `std` and the `types` submodule providing `NodeTypeDescriptor`. The `NodeTypeRegistry` struct is available via `anvilml_core::NodeTypeRegistry`.
**Tests:** An empty `NodeTypeRegistry` returns `None` for any `get()` lookup and reports a length of zero via `len()`.
**Mode:** both
**Inputs:** `NodeTypeRegistry::new()` — no descriptors registered.
**Expected output:** `get("NonExistent")` is `None`; `len()` is `0`.
**Acceptance:** `cargo test -p anvilml-core --test node_registry_tests test_empty_registry_returns_none` exits 0.

---

## test_register_all_populates (anvilml-core)

**File:** `crates/anvilml-core/tests/node_registry_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `std` and the `types` submodule providing `NodeTypeDescriptor`. The `NodeTypeRegistry` struct is available via `anvilml_core::NodeTypeRegistry`.
**Tests:** Registering a single descriptor via `register_all` populates the registry: `get` returns the registered value, `len` returns 1, and `list` contains exactly one element.
**Mode:** both
**Inputs:** `NodeTypeDescriptor { type_name: "LoadModel", ... }` passed to `register_all(vec![desc])`.
**Expected output:** `get("LoadModel")` returns `Some(desc)`; `len()` is `1`; `list().len()` is `1`.
**Acceptance:** `cargo test -p anvilml-core --test node_registry_tests test_register_all_populates` exits 0.

---

## test_register_all_replaces_prior_contents (anvilml-core)

**File:** `crates/anvilml-core/tests/node_registry_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `std` and the `types` submodule providing `NodeTypeDescriptor`. The `NodeTypeRegistry` struct is available via `anvilml_core::NodeTypeRegistry`.
**Tests:** Registering a second batch via `register_all` replaces (not merges with) prior contents: the old type name is no longer found after the second registration.
**Mode:** both
**Inputs:** First `register_all(vec![desc_A])`, then `register_all(vec![desc_B])` with a different type name.
**Expected output:** `get("A")` is `None` after second register; `get("B")` is `Some`; `len()` is `1`.
**Acceptance:** `cargo test -p anvilml-core --test node_registry_tests test_register_all_replaces_prior_contents` exits 0.

---

## test_list_returns_all (anvilml-core)

**File:** `crates/anvilml-core/tests/node_registry_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `std` and the `types` submodule providing `NodeTypeDescriptor`. The `NodeTypeRegistry` struct is available via `anvilml_core::NodeTypeRegistry`.
**Tests:** Registering three descriptors with distinct type names results in `list()` returning exactly three elements, each with a matching `type_name`.
**Mode:** both
**Inputs:** `register_all(vec![desc1, desc2, desc3])` with three descriptors.
**Expected output:** `list().len()` is `3`; all three type names are present in the returned vector.
**Acceptance:** `cargo test -p anvilml-core --test node_registry_tests test_list_returns_all` exits 0.

---

## test_concurrent_get_during_register_all_does_not_deadlock (anvilml-core)

**File:** `crates/anvilml-core/tests/node_registry_tests.rs`
**Context:** The `anvilml-core` crate has been compiled with `std` and the `types` submodule providing `NodeTypeDescriptor`. The `NodeTypeRegistry` struct is available via `anvilml_core::NodeTypeRegistry`. Uses `std::sync::Arc` and `std::thread::spawn` for concurrency.
**Tests:** A reader thread calling `get()` in a tight loop (100 iterations) while the main thread calls `register_all()` once completes within 2 seconds without deadlock or panic. This verifies that the `RwLock` correctly allows concurrent reads during a write.
**Mode:** both
**Inputs:** `Arc::new(NodeTypeRegistry::new())` shared between main thread (register) and spawned thread (read loop).
**Expected output:** Both threads complete without deadlock or panic; `join()` returns `Ok`.
**Acceptance:** `cargo test -p anvilml-core --test node_registry_tests test_concurrent_get_during_register_all_does_not_deadlock` exits 0.

---

## test_cpu_detector_returns_one_device (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/cpu_tests.rs`
**Context:** The `anvilml-hardware` crate has been compiled with the `mock-hardware` feature, and `CpuDetector` implements `DeviceDetector`.
**Tests:** `CpuDetector::detect()` returns `Ok(vec![..])` with exactly one element; the device's `name` field equals `"CPU"`.
**Mode:** both
**Inputs:** `CpuDetector` constructed with no arguments.
**Expected output:** `Ok(vec![GpuDevice { name: "CPU", device_type: Cpu, enumeration_source: Cpu, ... }])` — exactly one device.
**Acceptance:** `cargo test -p anvilml-hardware --test cpu_tests test_cpu_detector_returns_one_device` exits 0.

---

## test_cpu_detector_device_type_is_cpu (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/cpu_tests.rs`
**Context:** The `anvilml-hardware` crate has been compiled with the `mock-hardware` feature, and `CpuDetector` implements `DeviceDetector`.
**Tests:** The returned device has `device_type == DeviceType::Cpu` — confirms the device is classified as a CPU backend, not a GPU.
**Mode:** both
**Inputs:** `CpuDetector` constructed with no arguments; `detect()` called.
**Expected output:** `device_type == DeviceType::Cpu`.
**Acceptance:** `cargo test -p anvilml-hardware --test cpu_tests test_cpu_detector_device_type_is_cpu` exits 0.

---

## test_cpu_detector_enumeration_source_is_cpu (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/cpu_tests.rs`
**Context:** The `anvilml-hardware` crate has been compiled with the `mock-hardware` feature, and `CpuDetector` implements `DeviceDetector`.
**Tests:** The returned device has `enumeration_source == EnumerationSource::Cpu` — distinct from `EnumerationSource::Mock` (env-var-driven, P4-A3) and from the four real-enumeration variants (Vulkan, Dxgi, Sysfs, Nvml).
**Mode:** both
**Inputs:** `CpuDetector` constructed with no arguments; `detect()` called.
**Expected output:** `enumeration_source == EnumerationSource::Cpu`.
**Acceptance:** `cargo test -p anvilml-hardware --test cpu_tests test_cpu_detector_enumeration_source_is_cpu` exits 0.

---

## test_cpu_detector_refresh_vram_returns_zero (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/cpu_tests.rs`
**Context:** The `anvilml-hardware` crate has been compiled with the `mock-hardware` feature, and `CpuDetector` implements `DeviceDetector`.
**Tests:** `refresh_vram(0)` returns `Ok((0, 0))` — CPU has no VRAM, so both total and free are zero.
**Mode:** both
**Inputs:** `CpuDetector` constructed with no arguments; `refresh_vram(0)` called.
**Expected output:** `Ok((0, 0))`.
**Acceptance:** `cargo test -p anvilml-hardware --test cpu_tests test_cpu_detector_refresh_vram_returns_zero` exits 0.

---

## test_cpu_detector_all_device_fields (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/cpu_tests.rs`
**Context:** The `anvilml-hardware` crate has been compiled with the `mock-hardware` feature, and `CpuDetector` implements `DeviceDetector`.
**Tests:** Every field on the returned `GpuDevice` matches expected values: `vram_total_mib=0`, `vram_free_mib=0`, `driver_version="n/a"`, `pci_vendor_id=0`, `pci_device_id=0`, `arch=None`, `caps=InferenceCaps::default()` (all-false), `capabilities_source=CapabilitySource::Fallback`.
**Mode:** both
**Inputs:** `CpuDetector` constructed with no arguments; `detect()` called.
**Expected output:** All 12 fields match expected CPU-fallback values.
**Acceptance:** `cargo test -p anvilml-hardware --test cpu_tests test_cpu_detector_all_device_fields` exits 0.

---

## test_cpu_detect_never_errors (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/cpu_tests.rs`
**Context:** The `anvilml-hardware` crate has been compiled with the `mock-hardware` feature, and `CpuDetector` implements `DeviceDetector`.
**Tests:** `detect()` never returns `Err` or panics — `CpuDetector` is pure value construction with no I/O, no fallible operations, no conditional branches.
**Mode:** both
**Inputs:** `CpuDetector` constructed with no arguments; `detect()` called.
**Expected output:** `result.is_ok()` is true.
**Acceptance:** `cargo test -p anvilml-hardware --test cpu_tests test_cpu_detect_never_errors` exits 0.

---

## test_mock_detector_defaults (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/mock_tests.rs`
**Context:** The `anvilml-hardware` crate has been compiled with the `mock-hardware` feature, `MockDetector` implements `DeviceDetector`, and `serial_test` is available as a dev-dependency for env-var isolation. All three `ANVILML_MOCK_*` env vars are unset before the test.
**Tests:** `MockDetector::detect()` returns exactly one device with all default values: `device_type=Cpu`, `vram_total_mib=8192`, `vram_free_mib=8192`, `name="Mock GPU"`, `enumeration_source=Mock`, `capabilities_source=Fallback`.
**Mode:** mock
**Inputs:** `MockDetector` constructed with no arguments; all three `ANVILML_MOCK_*` env vars unset.
**Expected output:** `Ok(vec![GpuDevice { device_type: Cpu, vram_total_mib: 8192, vram_free_mib: 8192, name: "Mock GPU", enumeration_source: Mock, capabilities_source: Fallback, ... }])`.
**Acceptance:** `cargo test -p anvilml-hardware --features mock-hardware --test mock_tests -- test_mock_detector_defaults` exits 0.

---

## test_mock_cuda_device_type (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/mock_tests.rs`
**Context:** The `anvilml-hardware` crate has been compiled with the `mock-hardware` feature. `ANVILML_MOCK_DEVICE_TYPE` is set to `"cuda"`; prior value captured and restored.
**Tests:** The returned device has `device_type == DeviceType::Cuda` — confirms the env var is parsed and mapped to the CUDA backend.
**Mode:** mock
**Inputs:** `ANVILML_MOCK_DEVICE_TYPE=cuda`; `MockDetector::detect()` called.
**Expected output:** `device_type == DeviceType::Cuda`.
**Acceptance:** `cargo test -p anvilml-hardware --features mock-hardware --test mock_tests -- test_mock_cuda_device_type` exits 0.

---

## test_mock_rocm_device_type (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/mock_tests.rs`
**Context:** The `anvilml-hardware` crate has been compiled with the `mock-hardware` feature. `ANVILML_MOCK_DEVICE_TYPE` is set to `"rocm"`; prior value captured and restored.
**Tests:** The returned device has `device_type == DeviceType::Rocm` — confirms the env var is parsed and mapped to the ROCm backend.
**Mode:** mock
**Inputs:** `ANVILML_MOCK_DEVICE_TYPE=rocm`; `MockDetector::detect()` called.
**Expected output:** `device_type == DeviceType::Rocm`.
**Acceptance:** `cargo test -p anvilml-hardware --features mock-hardware --test mock_tests -- test_mock_rocm_device_type` exits 0.

---

## test_mock_vram_override (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/mock_tests.rs`
**Context:** The `anvilml-hardware` crate has been compiled with the `mock-hardware` feature. `ANVILML_MOCK_VRAM_MIB` is set to `"16384"`; prior value captured and restored.
**Tests:** The returned device has `vram_total_mib=16384` and `vram_free_mib=16384` — both fields are set from the env var value.
**Mode:** mock
**Inputs:** `ANVILML_MOCK_VRAM_MIB=16384`; `MockDetector::detect()` called.
**Expected output:** `vram_total_mib == 16384 && vram_free_mib == 16384`.
**Acceptance:** `cargo test -p anvilml-hardware --features mock-hardware --test mock_tests -- test_mock_vram_override` exits 0.

---

## test_mock_device_name_override (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/mock_tests.rs`
**Context:** The `anvilml-hardware` crate has been compiled with the `mock-hardware` feature. `ANVILML_MOCK_DEVICE_NAME` is set to `"Test GPU"`; prior value captured and restored.
**Tests:** The returned device has `name="Test GPU"` — confirms the env var is read and used as the device name.
**Mode:** mock
**Inputs:** `ANVILML_MOCK_DEVICE_NAME=Test GPU`; `MockDetector::detect()` called.
**Expected output:** `name == "Test GPU"`.
**Acceptance:** `cargo test -p anvilml-hardware --features mock-hardware --test mock_tests -- test_mock_device_name_override` exits 0.

---

## test_mock_refresh_vram (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/mock_tests.rs`
**Context:** The `anvilml-hardware` crate has been compiled with the `mock-hardware` feature. No `ANVILML_MOCK_VRAM_MIB` env var is set (uses default 8192).
**Tests:** `refresh_vram(0)` returns `Ok((8192, 8192))` — both total and free VRAM equal the default value; the `_index` parameter is unused.
**Mode:** mock
**Inputs:** `MockDetector::refresh_vram(0)` called with default VRAM.
**Expected output:** `Ok((8192, 8192))`.
**Acceptance:** `cargo test -p anvilml-hardware --features mock-hardware --test mock_tests -- test_mock_refresh_vram` exits 0.

---

## test_vulkan_nvidia_vendor_maps_to_cuda (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/vulkan_tests.rs`
**Context:** The `anvilml-hardware` crate has been compiled with `ash` and `tracing` dependencies.
**Tests:** `vendor_id_to_device_type(0x10de)` returns `Some(DeviceType::Cuda)` — NVIDIA's PCI vendor ID maps to the CUDA backend.
**Mode:** both
**Inputs:** `vendor_id_to_device_type(0x10de)`.
**Expected output:** `Some(DeviceType::Cuda)`.
**Acceptance:** `cargo test -p anvilml-hardware --test vulkan_tests test_vulkan_nvidia_vendor_maps_to_cuda` exits 0.

---

## test_vulkan_amd_vendor_maps_to_rocm (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/vulkan_tests.rs`
**Context:** The `anvilml-hardware` crate has been compiled with `ash` and `tracing` dependencies.
**Tests:** `vendor_id_to_device_type(0x1002)` returns `Some(DeviceType::Rocm)` — AMD's PCI vendor ID maps to the ROCm backend.
**Mode:** both
**Inputs:** `vendor_id_to_device_type(0x1002)`.
**Expected output:** `Some(DeviceType::Rocm)`.
**Acceptance:** `cargo test -p anvilml-hardware --test vulkan_tests test_vulkan_amd_vendor_maps_to_rocm` exits 0.

---

## test_vulkan_unknown_vendor_skipped (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/vulkan_tests.rs`
**Context:** The `anvilml-hardware` crate has been compiled with `ash` and `tracing` dependencies.
**Tests:** `vendor_id_to_device_type(0x1234)` returns `None` — unknown vendor IDs are skipped during enumeration.
**Mode:** both
**Inputs:** `vendor_id_to_device_type(0x1234)`.
**Expected output:** `None`.
**Acceptance:** `cargo test -p anvilml-hardware --test vulkan_tests test_vulkan_unknown_vendor_skipped` exits 0.

---

## test_vulkan_intel_vendor_skipped (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/vulkan_tests.rs`
**Context:** The `anvilml-hardware` crate has been compiled with `ash` and `tracing` dependencies.
**Tests:** `vendor_id_to_device_type(0x8086)` returns `None` — Intel's vendor ID is not a compute backend targeted by Vulkan detection.
**Mode:** both
**Inputs:** `vendor_id_to_device_type(0x8086)`.
**Expected output:** `None`.
**Acceptance:** `cargo test -p anvilml-hardware --test vulkan_tests test_vulkan_intel_vendor_skipped` exits 0.

---

## test_vulkan_detect_never_errors (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/vulkan_tests.rs`
**Context:** The `anvilml-hardware` crate has been compiled with `ash` and `tracing` dependencies. A Vulkan detector is constructed.
**Tests:** `VulkanDetector::detect()` returns `Ok(vec![..])` — never panics and never returns `Err`, even when the Vulkan loader is absent (CI, headless).
**Mode:** both
**Inputs:** `VulkanDetector` constructed, `detect()` called.
**Expected output:** `Ok(vec![])` on headless/CI; `Ok([..GpuDevices..])` on GPU-equipped systems.
**Acceptance:** `cargo test -p anvilml-hardware --test vulkan_tests test_vulkan_detect_never_errors` exits 0.

---

## test_vulkan_detect_returns_empty_when_no_gpu (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/vulkan_tests.rs`
**Context:** The `anvilml-hardware` crate has been compiled with `ash` and `tracing` dependencies.
**Tests:** `VulkanDetector::detect()` returns `Ok(vec![])` when no Vulkan-capable GPU is present (CI, headless). All returned devices have `enumeration_source == EnumerationSource::Vulkan`.
**Mode:** both
**Inputs:** `VulkanDetector` constructed, `detect()` called.
**Expected output:** Empty vector on headless/CI; non-empty vector with Vulkan-sourced devices on GPU systems.
**Acceptance:** `cargo test -p anvilml-hardware --test vulkan_tests test_vulkan_detect_returns_empty_when_no_gpu` exits 0.

---

## test_vulkan_refresh_vram_never_errors (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/vulkan_tests.rs`
**Context:** The `anvilml-hardware` crate has been compiled with `ash` and `tracing` dependencies.
**Tests:** `VulkanDetector::refresh_vram(0)` returns `Ok((total, free))` — never panics or returns `Err`. When Vulkan is unavailable, returns `(0, 0)`. When total equals free, it signals "free unknown" (fallback path).
**Mode:** both
**Inputs:** `VulkanDetector` constructed, `refresh_vram(0)` called.
**Expected output:** `Ok((total, total))` fallback or `Ok((0, 0))` when Vulkan unavailable.
**Acceptance:** `cargo test -p anvilml-hardware --test vulkan_tests test_vulkan_refresh_vram_never_errors` exits 0.

---

## test_vulkan_refresh_vram_out_of_range (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/vulkan_tests.rs`
**Context:** The `anvilml-hardware` crate has been compiled with `ash` and `tracing` dependencies.
**Tests:** `VulkanDetector::refresh_vram(999)` returns `Ok((0, 0))` — out-of-range indices are handled gracefully without panicking.
**Mode:** both
**Inputs:** `VulkanDetector` constructed, `refresh_vram(999)` called.
**Expected output:** `Ok((0, 0))`.
**Acceptance:** `cargo test -p anvilml-hardware --test vulkan_tests test_vulkan_refresh_vram_out_of_range` exits 0.

---

## test_dxgi_nvidia_vendor_maps_to_cuda (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/dxgi_tests.rs`
**Context:** The `anvilml-hardware` crate has been compiled with `anvilml-core` providing `DeviceType`, `vendor_id_to_device_type` from `vulkan.rs`, and `DxgiDetector` from `dxgi.rs`. The test file is gated `#[cfg(target_os = "windows")]`.
**Tests:** `vendor_id_to_device_type(0x10de)` returns `Some(DeviceType::Cuda)` — NVIDIA's PCI vendor ID maps to CUDA backend. This is a pure function test; no Windows API calls or GPU hardware is required.
**Mode:** both
**Inputs:** Vendor ID `0x10de`.
**Expected output:** `Some(DeviceType::Cuda)`.
**Acceptance:** `cargo test -p anvilml-hardware --test dxgi_tests test_dxgi_nvidia_vendor_maps_to_cuda` exits 0.

---

## test_dxgi_amd_vendor_maps_to_rocm (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/dxgi_tests.rs`
**Context:** The `anvilml-hardware` crate has been compiled with `anvilml-core` providing `DeviceType`, `vendor_id_to_device_type` from `vulkan.rs`, and `DxgiDetector` from `dxgi.rs`. The test file is gated `#[cfg(target_os = "windows")]`.
**Tests:** `vendor_id_to_device_type(0x1002)` returns `Some(DeviceType::Rocm)` — AMD's PCI vendor ID maps to ROCm backend. This is a pure function test; no Windows API calls or GPU hardware is required.
**Mode:** both
**Inputs:** Vendor ID `0x1002`.
**Expected output:** `Some(DeviceType::Rocm)`.
**Acceptance:** `cargo test -p anvilml-hardware --test dxgi_tests test_dxgi_amd_vendor_maps_to_rocm` exits 0.

---

## test_dxgi_detect_never_errors (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/dxgi_tests.rs`
**Context:** The `anvilml-hardware` crate has been compiled with `anvilml-core`, `tracing`, and the `windows` crate (with `Win32_Graphics_Dxgi` and `Win32_Graphics_Dxgi_Common` features). The test file is gated `#[cfg(target_os = "windows")]`.
**Tests:** `DxgiDetector::detect()` returns `Ok(vec)` — never panics or returns `Err`. On Windows with GPUs, returns detected devices; on headless/CI Windows, returns `Ok(vec![])`. The invariant is: no panic, no `Err`.
**Mode:** both
**Inputs:** `DxgiDetector` constructed, `detect()` called.
**Expected output:** `result.is_ok()` — `Ok(vec)` with zero or more devices.
**Acceptance:** `cargo test -p anvilml-hardware --test dxgi_tests test_dxgi_detect_never_errors` exits 0.

---

## test_dxgi_refresh_vram_never_errors (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/dxgi_tests.rs`
**Context:** The `anvilml-hardware` crate has been compiled with `anvilml-core`, `tracing`, and the `windows` crate (with `Win32_Graphics_Dxgi` and `Win32_Graphics_Dxgi_Common` features). The test file is gated `#[cfg(target_os = "windows")]`.
**Tests:** `DxgiDetector::refresh_vram(0)` returns `Ok((0, 0))` — DXGI has no VRAM query API. The `(0, 0)` return signals "unknown" to the caller, consistent with Vulkan's fallback when memory budget is unavailable.
**Mode:** both
**Inputs:** `DxgiDetector` constructed, `refresh_vram(0)` called.
**Expected output:** `Ok((0, 0))`.
**Acceptance:** `cargo test -p anvilml-hardware --test dxgi_tests test_dxgi_refresh_vram_never_errors` exits 0.

---

## test_sysfs_detect_missing_path_returns_empty (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/sysfs_tests.rs`
**Context:** The `anvilml-hardware` crate has been compiled with the `mock-hardware` feature, `SysfsPciDetector` implements `DeviceDetector`, and the `detect_from_path` helper is accessible via `pub(crate)` visibility. The test file is gated `#[cfg(target_os = "linux")]`.
**Tests:** `detect_from_path("/nonexistent/sysfs/path")` returns `Ok(vec![])` — proves the detector handles missing sysfs gracefully without panicking or returning `Err`.
**Mode:** both
**Inputs:** `detect_from_path` called with a nonexistent path.
**Expected output:** `Ok(vec![])` — empty vector, no error, no panic.
**Acceptance:** `cargo test -p anvilml-hardware --test sysfs_tests test_sysfs_detect_missing_path_returns_empty` exits 0.

---

## test_sysfs_detect_synthetic_display_device (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/sysfs_tests.rs`
**Context:** The `anvilml-hardware` crate has been compiled with the `mock-hardware` feature. A temp-dir-mocked sysfs tree is created with one synthetic AMD display-class device (vendor=0x1002, device=0x2204, class=0x030000) using `std::env::temp_dir()`.
**Tests:** `detect_from_path(temp_dir)` returns exactly one `GpuDevice` with `enumeration_source == Sysfs`, `device_type == Rocm`, `vram_total_mib == 0`, `driver_version == "n/a"`.
**Mode:** both
**Inputs:** Synthetic sysfs tree in temp dir with AMD display-class device.
**Expected output:** `Ok(vec![GpuDevice { enumeration_source: Sysfs, device_type: Rocm, vram_total_mib: 0, ... }])`.
**Acceptance:** `cargo test -p anvilml-hardware --test sysfs_tests test_sysfs_detect_synthetic_display_device` exits 0.

---

## test_sysfs_filter_non_display_class (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/sysfs_tests.rs`
**Context:** The `anvilml-hardware` crate has been compiled with the `mock-hardware` feature. A temp-dir-mocked sysfs tree is created with one synthetic network controller (class=0x020000, vendor=0x10de) using `std::env::temp_dir()`.
**Tests:** `detect_from_path(temp_dir)` returns an empty vector — the non-display-class device is filtered out by the `0x03` class prefix check.
**Mode:** both
**Inputs:** Synthetic sysfs tree with non-display class device (network controller, class 0x020000).
**Expected output:** `Ok(vec![])` — device excluded by class filter.
**Acceptance:** `cargo test -p anvilml-hardware --test sysfs_tests test_sysfs_filter_non_display_class` exits 0.

---

## test_sysfs_detect_nvidia_vendor (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/sysfs_tests.rs`
**Context:** The `anvilml-hardware` crate has been compiled with the `mock-hardware` feature. A temp-dir-mocked sysfs tree is created with one synthetic NVIDIA display-class device (vendor=0x10de, class=0x030000) using `std::env::temp_dir()`.
**Tests:** `detect_from_path(temp_dir)` returns exactly one `GpuDevice` with `device_type == Cuda` — NVIDIA vendor ID 0x10de maps to the CUDA backend via the shared `vendor_id_to_device_type()` function.
**Mode:** both
**Inputs:** Synthetic sysfs tree with NVIDIA display-class device.
**Expected output:** `Ok(vec![GpuDevice { device_type: Cuda, enumeration_source: Sysfs }])`.
**Acceptance:** `cargo test -p anvilml-hardware --test sysfs_tests test_sysfs_detect_nvidia_vendor` exits 0.

---

## test_sysfs_detect_never_errors (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/sysfs_tests.rs`
**Context:** The `anvilml-hardware` crate has been compiled with the `mock-hardware` feature. `SysfsPciDetector` is constructed.
**Tests:** `SysfsPciDetector::detect()` returns `Ok(vec)` — never panics or returns `Err`. On Linux with `/sys/bus/pci/devices/`, may return real devices; on headless/CI, returns `Ok(vec![])`. The invariant is: no panic, no `Err`.
**Mode:** both
**Inputs:** `SysfsPciDetector` constructed, `detect()` called.
**Expected output:** `result.is_ok()` — `Ok(vec)` with zero or more devices.
**Acceptance:** `cargo test -p anvilml-hardware --test sysfs_tests test_sysfs_detect_never_errors` exits 0.

---

## test_sysfs_refresh_vram_returns_zero (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/sysfs_tests.rs`
**Context:** The `anvilml-hardware` crate has been compiled with the `mock-hardware` feature. `SysfsPciDetector` is constructed.
**Tests:** `SysfsPciDetector::refresh_vram(0)` returns `Ok((0, 0))` — sysfs has no VRAM query API. The `(0, 0)` return signals "unknown" to the caller, consistent with `DxgiDetector`'s approach.
**Mode:** both
**Inputs:** `SysfsPciDetector` constructed, `refresh_vram(0)` called.
**Expected output:** `Ok((0, 0))`.
**Acceptance:** `cargo test -p anvilml-hardware --test sysfs_tests test_sysfs_refresh_vram_returns_zero` exits 0.

---

## test_sysfs_multi_device_filter (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/sysfs_tests.rs`
**Context:** The `anvilml-hardware` crate has been compiled with the `mock-hardware` feature. A temp-dir-mocked sysfs tree is created with three synthetic devices: one AMD display controller (class 0x030000), one NVIDIA network controller (class 0x020000), and one Intel audio controller (class 0x040300) using `std::env::temp_dir()`.
**Tests:** `detect_from_path(temp_dir)` returns exactly one device — only the display-class device is included; the network and audio controllers are filtered out by the class prefix check.
**Mode:** both
**Inputs:** Synthetic sysfs tree with three devices of different PCI classes.
**Expected output:** `Ok(vec![GpuDevice])` with exactly one AMD/Rocm device.
**Acceptance:** `cargo test -p anvilml-hardware --test sysfs_tests test_sysfs_multi_device_filter` exits 0.

---

## test_override_present_returns_device (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/detect_tests.rs`
**Context:** The `anvilml-hardware` crate has been compiled with `tokio` dev-dependency for async test support. `detect_all_devices()` is called with a `ServerConfig` that has `hardware_override` set to `Some(HardwareOverrideConfig { device_type: "cuda", vram_total_mib: 24576 })`.
**Tests:** `detect_all_devices` returns `Ok(HardwareInfo)` with exactly one synthesized `GpuDevice` matching all override config fields: `device_type == Cuda`, `vram_total_mib == 24576`, `enumeration_source == Override`, `capabilities_source == Fallback`, `name == "CUDA"`, `driver_version == "override"`, `vram_free_mib == 24576`. Host fields are non-empty.
**Mode:** both
**Inputs:** `ServerConfig` with `hardware_override = Some(HardwareOverrideConfig { device_type: "cuda", vram_total_mib: 24576 })`.
**Expected output:** `Ok(HardwareInfo)` with exactly one GPU device matching override config.
**Acceptance:** `cargo test -p anvilml-hardware --test detect_tests test_override_present_returns_device` exits 0.

---

## test_override_absent_returns_hardware_info (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/detect_tests.rs`
**Context:** The `anvilml-hardware` crate has been compiled with `tokio` dev-dependency. `detect_all_devices()` is called with a default `ServerConfig` (no override).
**Tests:** `detect_all_devices` returns `Ok(HardwareInfo)` with host info populated and `inference_caps == InferenceCaps::default()` — the function never returns `Err`. In mock-hardware builds, the mock-detected device is returned; in real builds, Vulkan/platform detection results are returned.
**Mode:** both
**Inputs:** Default `ServerConfig` (hardware_override is None).
**Expected output:** `Ok(HardwareInfo)` with non-empty host info and default inference_caps.
**Acceptance:** `cargo test -p anvilml-hardware --test detect_tests test_override_absent_returns_hardware_info` exits 0.

---

## test_partial_hardware_info_has_default_inference_caps (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/detect_tests.rs`
**Context:** The `anvilml-hardware` crate has been compiled with `tokio` dev-dependency. `detect_all_devices()` is called with a default `ServerConfig` (no override).
**Tests:** The returned `HardwareInfo` has `inference_caps == InferenceCaps::default()` — this verifies the partial HardwareInfo contract where P5-A2 returns detected GPUs with default caps, deferring the per-device caps union to P5-A3.
**Mode:** both
**Inputs:** Default `ServerConfig` (hardware_override is None).
**Expected output:** `Ok(HardwareInfo)` with default inference_caps and populated host info.
**Acceptance:** `cargo test -p anvilml-hardware --test detect_tests test_partial_hardware_info_has_default_inference_caps` exits 0.

---

## test_override_unrecognized_device_type_defaults_to_cpu (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/detect_tests.rs`
**Context:** The `anvilml-hardware` crate has been compiled with `tokio` dev-dependency. `detect_all_devices()` is called with an unrecognized `device_type` value (`"metal"`).
**Tests:** The function falls back to `DeviceType::Cpu` with a warning log, returning a synthesized CPU device. This verifies the graceful degradation path for unrecognized override values.
**Mode:** both
**Inputs:** `ServerConfig` with `hardware_override = Some(HardwareOverrideConfig { device_type: "metal", vram_total_mib: 8192 })`.
**Expected output:** `Ok(HardwareInfo)` with one device having `device_type == Cpu` and `name == "CPU"`.
**Acceptance:** `cargo test -p anvilml-hardware --test detect_tests test_override_unrecognized_device_type_defaults_to_cpu` exits 0.

---

## test_override_rocm_device_type (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/detect_tests.rs`
**Context:** The `anvilml-hardware` crate has been compiled with `tokio` dev-dependency. `detect_all_devices()` is called with `device_type == "rocm"`.
**Tests:** The function returns a synthesized ROCm device with `device_type == Rocm`, `name == "ROCm"`, and correct VRAM from the override config.
**Mode:** both
**Inputs:** `ServerConfig` with `hardware_override = Some(HardwareOverrideConfig { device_type: "rocm", vram_total_mib: 16384 })`.
**Expected output:** `Ok(HardwareInfo)` with one device having `device_type == Rocm`, `name == "ROCm"`, `vram_total_mib == 16384`.
**Acceptance:** `cargo test -p anvilml-hardware --test detect_tests test_override_rocm_device_type` exits 0.

---

## test_override_cpu_device_type (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/detect_tests.rs`
**Context:** The `anvilml-hardware` crate has been compiled with `tokio` dev-dependency. `detect_all_devices()` is called with `device_type == "cpu"`.
**Tests:** The function returns a synthesized CPU device with `device_type == Cpu`, `name == "CPU"`, and correct VRAM (0) from the override config.
**Mode:** both
**Inputs:** `ServerConfig` with `hardware_override = Some(HardwareOverrideConfig { device_type: "cpu", vram_total_mib: 0 })`.
**Expected output:** `Ok(HardwareInfo)` with one device having `device_type == Cpu`, `name == "CPU"`, `vram_total_mib == 0`.
**Acceptance:** `cargo test -p anvilml-hardware --test detect_tests test_override_cpu_device_type` exits 0.

---

## test_override_inference_caps_is_default (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/detect_tests.rs`
**Context:** The `anvilml-hardware` crate has been compiled with `tokio` dev-dependency. `detect_all_devices()` is called with a CUDA override.
**Tests:** The returned `HardwareInfo.inference_caps` equals `InferenceCaps::default()` (all fields false) — since override devices have no real inference capabilities, the default is correct.
**Mode:** both
**Inputs:** `ServerConfig` with `hardware_override = Some(HardwareOverrideConfig { device_type: "cuda", vram_total_mib: 24576 })`.
**Expected output:** `inference_caps == InferenceCaps::default()` (all boolean fields false).
**Acceptance:** `cargo test -p anvilml-hardware --test detect_tests test_override_inference_caps_is_default` exits 0.

---

## test_mock_hardware_feature_returns_mock_device (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/detect_tests.rs`
**Context:** The `anvilml-hardware` crate has been compiled with `mock-hardware` feature and `tokio` dev-dependency. `ANVILML_MOCK_DEVICE_TYPE=cuda` and `ANVILML_MOCK_VRAM_MIB=24576` are set. `detect_all_devices()` is called with a default `ServerConfig` (no override).
**Tests:** `detect_all_devices` returns `Ok(HardwareInfo)` with exactly one mock-detected device: `device_type == Cuda`, `vram_total_mib == 24576`, `enumeration_source == Mock`, `name == "Mock GPU"`.
**Mode:** mock
**Inputs:** `ServerConfig::default()`, env vars `ANVILML_MOCK_DEVICE_TYPE=cuda`, `ANVILML_MOCK_VRAM_MIB=24576`.
**Expected output:** `Ok(HardwareInfo)` with one GPU device matching mock env vars.
**Acceptance:** `cargo test -p anvilml-hardware --features mock-hardware --test detect_tests test_mock_hardware_feature_returns_mock_device` exits 0.

---

## test_override_takes_priority_over_mock (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/detect_tests.rs`
**Context:** The `anvilml-hardware` crate has been compiled with `mock-hardware` feature and `tokio` dev-dependency. `ANVILML_MOCK_DEVICE_TYPE=cuda` and `ANVILML_MOCK_VRAM_MIB=8192` are set. `detect_all_devices()` is called with a `ServerConfig` that has `hardware_override` set to `Some(HardwareOverrideConfig { device_type: "rocm", vram_total_mib: 16384 })`.
**Tests:** The override short-circuit fires before `MockDetector` is queried — the returned device has `device_type == Rocm` and `vram_total_mib == 16384` (from override), not the mock values. Proves override priority is preserved when mock-hardware is compiled in.
**Mode:** mock
**Inputs:** `ServerConfig` with `hardware_override = Some(HardwareOverrideConfig { device_type: "rocm", vram_total_mib: 16384 })`, env vars `ANVILML_MOCK_DEVICE_TYPE=cuda`, `ANVILML_MOCK_VRAM_MIB=8192`.
**Expected output:** `Ok(HardwareInfo)` with one GPU device matching override config (Rocm/16384), not mock (Cuda/8192).
**Acceptance:** `cargo test -p anvilml-hardware --features mock-hardware --test detect_tests test_override_takes_priority_over_mock` exits 0.

---

## test_mock_detector_env_vars_propagate_through_detect_all_devices (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/detect_tests.rs`
**Context:** The `anvilml-hardware` crate has been compiled with `mock-hardware` feature and `tokio` dev-dependency. `ANVILML_MOCK_DEVICE_NAME=Custom Mock GPU` and `ANVILML_MOCK_VRAM_MIB=16384` are set. `detect_all_devices()` is called with a default `ServerConfig` (no override).
**Tests:** The returned device has `name == "Custom Mock GPU"` and `vram_total_mib == 16384`, confirming that custom mock env vars propagate through `detect_all_devices` → `MockDetector::detect()` → `GpuDevice` construction.
**Mode:** mock
**Inputs:** `ServerConfig::default()`, env vars `ANVILML_MOCK_DEVICE_NAME=Custom Mock GPU`, `ANVILML_MOCK_VRAM_MIB=16384`.
**Expected output:** `Ok(HardwareInfo)` with one GPU device having name "Custom Mock GPU" and vram_total_mib=16384.
**Acceptance:** `cargo test -p anvilml-hardware --features mock-hardware --test detect_tests test_mock_detector_env_vars_propagate_through_detect_all_devices` exits 0.

---

## test_cpu_device_always_present_and_last (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/detect_tests.rs`
**Context:** The `anvilml-hardware` crate has been compiled with `mock-hardware` feature and `tokio` dev-dependency. `ANVILML_MOCK_DEVICE_TYPE=cuda` and `ANVILML_MOCK_VRAM_MIB=24576` are set. `detect_all_devices()` is called with a default `ServerConfig` (no override).
**Tests:** The last device in `gpus` is the CPU fallback device (`device_type == Cpu`, `enumeration_source == Cpu`, `name == "CPU"`), confirming that `CpuDetector`'s device is always appended last after P5-A3.
**Mode:** mock
**Inputs:** `ANVILML_MOCK_DEVICE_TYPE=cuda`, `ANVILML_MOCK_VRAM_MIB=24576`, default `ServerConfig`.
**Expected output:** `gpus.len() >= 2`, last device has `device_type == Cpu` and `enumeration_source == Cpu`.
**Acceptance:** `cargo test -p anvilml-hardware --features mock-hardware --test detect_tests test_cpu_device_always_present_and_last` exits 0.

---

## test_inference_caps_is_caps_union (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/detect_tests.rs`
**Context:** The `anvilml-hardware` crate has been compiled with `tokio` dev-dependency. `detect_all_devices()` is called with a default `ServerConfig` (no override).
**Tests:** `inference_caps` is the field-wise OR union of all per-device `InferenceCaps`, not a hardcoded default. With default-cap devices (mock + CPU), the union is all-false (default).
**Mode:** both
**Inputs:** Default `ServerConfig`.
**Expected output:** `inference_caps == InferenceCaps::default()` (union of default caps from all devices).
**Acceptance:** `cargo test -p anvilml-hardware --features mock-hardware --test detect_tests test_inference_caps_is_caps_union` exits 0.

---

## test_inference_caps_union_correctness (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/detect_tests.rs`
**Context:** The `anvilml-hardware` crate has been compiled with `mock-hardware` feature and `tokio` dev-dependency. `ANVILML_MOCK_DEVICE_TYPE=cuda` and `ANVILML_MOCK_VRAM_MIB=24576` are set. `detect_all_devices()` is called with a default `ServerConfig` (no override).
**Tests:** `inference_caps` is the field-wise OR union of all per-device `InferenceCaps`. With mock device (default caps) and CPU fallback (default caps), the union is all false (default).
**Mode:** mock
**Inputs:** `ANVILML_MOCK_DEVICE_TYPE=cuda`, `ANVILML_MOCK_VRAM_MIB=24576`, default `ServerConfig`.
**Expected output:** `inference_caps == InferenceCaps::default()` (union of all device caps).
**Acceptance:** `cargo test -p anvilml-hardware --features mock-hardware --test detect_tests test_inference_caps_union_correctness` exits 0.

---

## test_host_fields_non_empty (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/detect_tests.rs`
**Context:** The `anvilml-hardware` crate has been compiled with `tokio` dev-dependency. `detect_all_devices()` is called with a default `ServerConfig` (no override).
**Tests:** `host.hostname` and `host.os` are both non-empty strings after `detect_all_devices()` returns, verifying the minimal `HostInfo` population works correctly.
**Mode:** both
**Inputs:** Default `ServerConfig`.
**Expected output:** `result.host.hostname.len() > 0` and `result.host.os.len() > 0`.
**Acceptance:** `cargo test -p anvilml-hardware --features mock-hardware --test detect_tests test_host_fields_non_empty` exits 0.

---

## test_override_path_still_has_cpu_device (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/detect_tests.rs`
**Context:** The `anvilml-hardware` crate has been compiled with `tokio` dev-dependency. `detect_all_devices()` is called with `hardware_override` set to `Some(HardwareOverrideConfig { device_type: "cuda", vram_total_mib: 24576 })`.
**Tests:** Even the override path (which previously returned a single override device) now appends the CPU fallback device, making the result contain 2 devices. First device is the override GPU, second is the CPU fallback.
**Mode:** both
**Inputs:** `hardware_override` with `device_type=cuda`, `vram_total_mib=24576`.
**Expected output:** `gpus.len() == 2`, first device has `enumeration_source == Override`, second has `enumeration_source == Cpu`.
**Acceptance:** `cargo test -p anvilml-hardware --features mock-hardware --test detect_tests test_override_path_still_has_cpu_device` exits 0.

---

## test_override_present_returns_device (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/detect_tests.rs`
**Context:** The `anvilml-hardware` crate has been compiled with `tokio` dev-dependency. `detect_all_devices()` is called with a `ServerConfig` that has `hardware_override` set to `Some(HardwareOverrideConfig { device_type: "cuda", vram_total_mib: 24576 })`.
**Tests:** `detect_all_devices` returns `Ok(HardwareInfo)` with two devices: the override-synthesized `GpuDevice` (device_type == Cuda, vram_total_mib == 24576, enumeration_source == Override) followed by the CPU fallback device (device_type == Cpu, enumeration_source == Cpu). Host fields are non-empty.
**Mode:** both
**Inputs:** `hardware_override` with `device_type=cuda`, `vram_total_mib=24576`.
**Expected output:** `gpus.len() == 2`, first device matches override config, second device is CPU fallback.
**Acceptance:** `cargo test -p anvilml-hardware --test detect_tests test_override_present_returns_device` exits 0.

---

## test_override_absent_returns_hardware_info (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/detect_tests.rs`
**Context:** The `anvilml-hardware` crate has been compiled with `tokio` dev-dependency. `detect_all_devices()` is called with a default `ServerConfig` (no override).
**Tests:** `detect_all_devices` returns `Ok(HardwareInfo)` with host info populated and `gpus` containing at least one device (CPU fallback). The function never returns `Err`. After P5-A3, `inference_caps` is the union of all device caps.
**Mode:** both
**Inputs:** Default `ServerConfig`.
**Expected output:** `Ok(HardwareInfo)` with non-empty host fields and `gpus.len() >= 1`.
**Acceptance:** `cargo test -p anvilml-hardware --test detect_tests test_override_absent_returns_hardware_info` exits 0.

---

## test_override_unrecognized_device_type_defaults_to_cpu (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/detect_tests.rs`
**Context:** The `anvilml-hardware` crate has been compiled with `tokio` dev-dependency. `detect_all_devices()` is called with an unrecognized `device_type` value (`"metal"`).
**Tests:** The override path defaults to `DeviceType::Cpu` with `name == "CPU"`, and after P5-A3 returns 2 devices (override CPU + CPU fallback).
**Mode:** both
**Inputs:** `hardware_override` with `device_type=metal`, `vram_total_mib=8192`.
**Expected output:** `gpus.len() == 2`, first device has `device_type == Cpu` and `name == "CPU"`.
**Acceptance:** `cargo test -p anvilml-hardware --test detect_tests test_override_unrecognized_device_type_defaults_to_cpu` exits 0.

---

## test_override_rocm_device_type (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/detect_tests.rs`
**Context:** The `anvilml-hardware` crate has been compiled with `tokio` dev-dependency. `detect_all_devices()` is called with `device_type == "rocm"`.
**Tests:** Returns `Ok(HardwareInfo)` with the override ROCm device followed by the CPU fallback device (2 devices total after P5-A3).
**Mode:** both
**Inputs:** `hardware_override` with `device_type=rocm`, `vram_total_mib=16384`.
**Expected output:** `gpus.len() == 2`, first device has `device_type == Rocm`, second is CPU fallback.
**Acceptance:** `cargo test -p anvilml-hardware --test detect_tests test_override_rocm_device_type` exits 0.

---

## test_override_cpu_device_type (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/detect_tests.rs`
**Context:** The `anvilml-hardware` crate has been compiled with `tokio` dev-dependency. `detect_all_devices()` is called with `device_type == "cpu"`.
**Tests:** Returns `Ok(HardwareInfo)` with the override CPU device followed by the CPU fallback device (2 devices total after P5-A3).
**Mode:** both
**Inputs:** `hardware_override` with `device_type=cpu`, `vram_total_mib=0`.
**Expected output:** `gpus.len() == 2`, first device has `device_type == Cpu`, second is CPU fallback.
**Acceptance:** `cargo test -p anvilml-hardware --test detect_tests test_override_cpu_device_type` exits 0.

---

## test_override_inference_caps_is_default (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/detect_tests.rs`
**Context:** The `anvilml-hardware` crate has been compiled with `tokio` dev-dependency. `detect_all_devices()` is called with a CUDA override.
**Tests:** `inference_caps` is the union of all device caps. Since override device and CPU fallback both have default (all-false) caps, the union is also default.
**Mode:** both
**Inputs:** `hardware_override` with `device_type=cuda`, `vram_total_mib=24576`.
**Expected output:** `inference_caps == InferenceCaps::default()`.
**Acceptance:** `cargo test -p anvilml-hardware --test detect_tests test_override_inference_caps_is_default` exits 0.

---

## test_mock_hardware_feature_returns_mock_device (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/detect_tests.rs`
**Context:** The `anvilml-hardware` crate has been compiled with `mock-hardware` feature and `tokio` dev-dependency. `ANVILML_MOCK_DEVICE_TYPE=cuda` and `ANVILML_MOCK_VRAM_MIB=24576` are set. `detect_all_devices()` is called with a default `ServerConfig` (no override).
**Tests:** `detect_all_devices` returns `Ok(HardwareInfo)` with two devices: the mock-detected device (`device_type == Cuda`, `vram_total_mib == 24576`, `enumeration_source == Mock`, `name == "Mock GPU"`) followed by the CPU fallback device (`device_type == Cpu`, `enumeration_source == Cpu`).
**Mode:** mock
**Inputs:** `ANVILML_MOCK_DEVICE_TYPE=cuda`, `ANVILML_MOCK_VRAM_MIB=24576`, default `ServerConfig`.
**Expected output:** `gpus.len() == 2`, first device matches mock config, second is CPU fallback.
**Acceptance:** `cargo test -p anvilml-hardware --features mock-hardware --test detect_tests test_mock_hardware_feature_returns_mock_device` exits 0.

---

## test_override_takes_priority_over_mock (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/detect_tests.rs`
**Context:** The `anvilml-hardware` crate has been compiled with `mock-hardware` feature and `tokio` dev-dependency. `ANVILML_MOCK_DEVICE_TYPE=cuda` and `ANVILML_MOCK_VRAM_MIB=8192` are set. `detect_all_devices()` is called with a `ServerConfig` that has `hardware_override` set to `Some(HardwareOverrideConfig { device_type: "rocm", vram_total_mib: 16384 })`.
**Tests:** The override path returns 2 devices (override ROCm + CPU fallback), not the mock device. First device is from override (`device_type == Rocm`, `vram_total_mib == 16384`, `enumeration_source == Override`), second is CPU fallback.
**Mode:** mock
**Inputs:** `ANVILML_MOCK_DEVICE_TYPE=cuda`, `ANVILML_MOCK_VRAM_MIB=8192`, override with `device_type=rocm`, `vram_total_mib=16384`.
**Expected output:** `gpus.len() == 2`, first device matches override, second is CPU fallback.
**Acceptance:** `cargo test -p anvilml-hardware --features mock-hardware --test detect_tests test_override_takes_priority_over_mock` exits 0.

---

## test_mock_detector_env_vars_propagate_through_detect_all_devices (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/detect_tests.rs`
**Context:** The `anvilml-hardware` crate has been compiled with `mock-hardware` feature and `tokio` dev-dependency. `ANVILML_MOCK_DEVICE_NAME=Custom Mock GPU` and `ANVILML_MOCK_VRAM_MIB=16384` are set. `detect_all_devices()` is called with a default `ServerConfig` (no override).
**Tests:** The returned mock device has `name == "Custom Mock GPU"` and `vram_total_mib == 16384`, confirming that custom mock env vars propagate through `detect_all_devices` → `MockDetector::detect()` → `GpuDevice` construction. After P5-A3, the result contains 2 devices (mock GPU + CPU fallback).
**Mode:** mock
**Inputs:** `ANVILML_MOCK_DEVICE_NAME=Custom Mock GPU`, `ANVILML_MOCK_VRAM_MIB=16384`, default `ServerConfig`.
**Expected output:** `gpus.len() == 2`, first device has name "Custom Mock GPU" and vram_total_mib=16384, second is CPU fallback.
**Acceptance:** `cargo test -p anvilml-hardware --features mock-hardware --test detect_tests test_mock_detector_env_vars_propagate_through_detect_all_devices` exits 0.

---

## test_pool_creation_succeeds (anvilml-registry)

**File:** `crates/anvilml-registry/tests/db_tests.rs`
**Context:** The `anvilml-registry` crate has been compiled with `sqlx` (sqlite, runtime-tokio, migrate, chrono features), `tokio` (macros feature), and `tempfile` dev-dependency. `create_pool()` opens a SQLite database and runs migrations.
**Tests:** `create_pool()` against a temporary file succeeds and the returned pool can execute queries — proves the connection is valid and migrations ran without error.
**Mode:** both
**Inputs:** A temporary file path (created by `tempfile::NamedTempFile`).
**Expected output:** `create_pool()` returns `Ok(SqlitePool)`, `SELECT 1` returns `1`.
**Acceptance:** `cargo test -p anvilml-registry --test db_tests test_pool_creation_succeeds` exits 0.

---

## test_migrations_create_tables (anvilml-registry)

**File:** `crates/anvilml-registry/tests/db_tests.rs`
**Context:** The `anvilml-registry` crate has been compiled with `sqlx` (sqlite, runtime-tokio, migrate, chrono features), `tokio` (macros feature), and `tempfile` dev-dependency. The migration file `database/migrations/001_initial.sql` defines `models` and `device_capabilities` tables.
**Tests:** After `create_pool()` runs migrations, querying `sqlite_master` returns both `models` and `device_capabilities` tables — proves migrations applied successfully.
**Mode:** both
**Inputs:** A temporary file path (created by `tempfile::NamedTempFile`).
**Expected output:** `sqlite_master` query returns rows for both `"models"` and `"device_capabilities"` table names.
**Acceptance:** `cargo test -p anvilml-registry --test db_tests test_migrations_create_tables` exits 0.

---

## test_wal_mode_enabled (anvilml-registry)

**File:** `crates/anvilml-registry/tests/db_tests.rs`
**Context:** The `anvilml-registry` crate has been compiled with `sqlx` (sqlite, runtime-tokio, migrate, chrono features), `tokio` (macros feature), and `tempfile` dev-dependency. `create_pool()` executes `PRAGMA journal_mode=WAL` after connecting.
**Tests:** After `create_pool()`, querying `PRAGMA journal_mode` returns `"wal"` — proves WAL journaling mode is active for better concurrent access.
**Mode:** both
**Inputs:** A temporary file path (created by `tempfile::NamedTempFile`).
**Expected output:** `PRAGMA journal_mode` returns `"wal"`.
**Acceptance:** `cargo test -p anvilml-registry --test db_tests test_wal_mode_enabled` exits 0.

---

## test_migrations_idempotent (anvilml-registry)

**File:** `crates/anvilml-registry/tests/db_tests.rs`
**Context:** The `anvilml-registry` crate has been compiled with `sqlx` (sqlite, runtime-tokio, migrate, chrono features), `tokio` (macros feature), and `tempfile` dev-dependency. `create_pool()` runs migrations via `sqlx::migrate!().run()` which is idempotent.
**Tests:** Creating two pools against the same database file — the first runs migrations, the second runs them again. Both succeed without error, proving migration idempotency.
**Mode:** both
**Inputs:** A single temporary file path used for both pool creations.
**Expected output:** Both `create_pool()` calls return `Ok(SqlitePool)`, both pools can execute queries.
**Acceptance:** `cargo test -p anvilml-registry --test db_tests test_migrations_idempotent` exits 0.

---

## test_upsert_get_roundtrip (anvilml-registry)

**File:** `crates/anvilml-registry/tests/store_tests.rs`
**Context:** The `anvilml-registry` crate has been compiled with `sqlx` (sqlite, runtime-tokio, migrate, chrono features), `tokio` (macros feature), `chrono` (serde feature), and `serde_json` dependencies. `ModelStore::new()` wraps a `SqlitePool`, `upsert()` inserts via `INSERT OR REPLACE`, and `get()` retrieves via `query_as!` with a `ModelMetaRow` helper struct.
**Tests:** Inserts a `ModelMeta` with id="test-1", name="test-model", kind=Diffusion via `upsert()`, then retrieves it by ID via `get()`. Asserts all fields (id, name, path, kind, dtype, format, size_bytes) match the original; `scanned_at` is within 2s tolerance.
**Mode:** both
**Inputs:** `ModelMeta` with id="test-1", name="test-model", path="/tmp/models/test-model.safetensors", kind=Diffusion, dtype=Fp32, format=Safetensors, size_bytes=1024.
**Expected output:** `get("test-1")` returns `Some(meta)` with all fields matching the inserted values.
**Acceptance:** `cargo test -p anvilml-registry --test store_tests test_upsert_get_roundtrip` exits 0.

---

## test_list_no_filter (anvilml-registry)

**File:** `crates/anvilml-registry/tests/store_tests.rs`
**Context:** The `anvilml-registry` crate has been compiled with `sqlx` (sqlite, runtime-tokio, migrate, chrono features), `tokio` (macros feature), `chrono` (serde feature), and `serde_json` dependencies. `list(None)` returns all rows from the `models` table.
**Tests:** Inserts three models with different kinds (Diffusion, TextEncoder, Vae), then calls `list(None)` and asserts the result contains exactly 3 rows.
**Mode:** both
**Inputs:** 3 `ModelMeta` rows with ids "1", "2", "3" and kinds Diffusion, TextEncoder, Vae.
**Expected output:** `list(None)` returns a `Vec<ModelMeta>` with length 3.
**Acceptance:** `cargo test -p anvilml-registry --test store_tests test_list_no_filter` exits 0.

---

## test_list_with_kind_filter (anvilml-registry)

**File:** `crates/anvilml-registry/tests/store_tests.rs`
**Context:** The `anvilml-registry` crate has been compiled with `sqlx` (sqlite, runtime-tokio, migrate, chrono features), `tokio` (macros feature), `chrono` (serde feature), and `serde_json` dependencies. `list(Some(kind))` filters rows by the `kind` column.
**Tests:** Inserts three models with different kinds (Diffusion, TextEncoder, Vae), then calls `list(Some(ModelKind::Diffusion))` and asserts the result contains exactly 1 row (the diffusion model) with the correct kind.
**Mode:** both
**Inputs:** 3 `ModelMeta` rows with kinds Diffusion, TextEncoder, Vae; kind filter = `Some(ModelKind::Diffusion)`.
**Expected output:** `list(Some(Diffusion))` returns a `Vec<ModelMeta>` with length 1, first element has `kind == Diffusion`.
**Acceptance:** `cargo test -p anvilml-registry --test store_tests test_list_with_kind_filter` exits 0.

---

## test_delete_removes_row (anvilml-registry)

**File:** `crates/anvilml-registry/tests/store_tests.rs`
**Context:** The `anvilml-registry` crate has been compiled with `sqlx` (sqlite, runtime-tokio, migrate, chrono features), `tokio` (macros feature), `chrono` (serde feature), and `serde_json` dependencies. `delete(id)` removes a row by primary key; subsequent `get(id)` returns `None`.
**Tests:** Inserts a model, verifies it exists via `get()`, calls `delete()`, then verifies the row is gone via `get()` returning `None`.
**Mode:** both
**Inputs:** `ModelMeta` with id="del-1", name="to-delete", kind=Lora.
**Expected output:** `delete("del-1")` succeeds; `get("del-1")` returns `None`.
**Acceptance:** `cargo test -p anvilml-registry --test store_tests test_delete_removes_row` exits 0.

---

## test_get_missing_id_returns_none (anvilml-registry)

**File:** `crates/anvilml-registry/tests/store_tests.rs`
**Context:** The `anvilml-registry` crate has been compiled with `sqlx` (sqlite, runtime-tokio, migrate, chrono features), `tokio` (macros feature), `chrono` (serde feature), and `serde_json` dependencies. `get(id)` returns `None` for nonexistent IDs rather than an error.
**Tests:** Does not insert any rows; directly queries for a nonexistent ID and asserts that the result is `None`.
**Mode:** both
**Inputs:** id="nonexistent-id"; no rows in the database.
**Expected output:** `get("nonexistent-id")` returns `None`.
**Acceptance:** `cargo test -p anvilml-registry --test store_tests test_get_missing_id_returns_none` exits 0.

---

## test_lookup_known_pciid_returns_caps (anvilml-registry)

**File:** `crates/anvilml-registry/tests/device_store_tests.rs`
**Context:** The `anvilml-registry` crate has been compiled with `sqlx` (sqlite, runtime-tokio, migrate, chrono features), `tokio` (macros feature), `anvilml-core` (types submodule with `InferenceCaps`), and `uuid` (v4 feature) dev-dependencies. `lookup(vendor_id, device_id)` queries the `device_capabilities` table and returns `Some(InferenceCaps)` when a row exists.
**Tests:** Inserts a row with vendor_id=0x10DE, device_id=0x2684, all capability columns=1 (fp32, fp16, bf16, fp8=1, fp4=0, flash_attention=1), then looks it up and asserts that every bool field matches the expected value.
**Mode:** both
**Inputs:** vendor_id=0x10DE, device_id=0x2684; row with fp32=1, fp16=1, bf16=1, fp8=1, fp4=0, flash_attention=1.
**Expected output:** `Ok(Some(InferenceCaps { fp32: true, fp16: true, bf16: true, fp8: true, fp4: false, flash_attention: true }))`.
**Acceptance:** `cargo test -p anvilml-registry --test device_store_tests test_lookup_known_pciid_returns_caps` exits 0.

---

## test_lookup_unknown_pciid_returns_none (anvilml-registry)

**File:** `crates/anvilml-registry/tests/device_store_tests.rs`
**Context:** The `anvilml-registry` crate has been compiled with `sqlx` (sqlite, runtime-tokio, migrate, chrono features), `tokio` (macros feature), `anvilml-core` (types submodule with `InferenceCaps`), and `uuid` (v4 feature) dev-dependencies. `lookup(vendor_id, device_id)` returns `Ok(None)` for unknown PCI-ID pairs — never `Err`.
**Tests:** Does not insert any rows; directly queries for a nonexistent PCI-ID pair and asserts that the result is `None` rather than an error.
**Mode:** both
**Inputs:** vendor_id=0xFFFF, device_id=0xFFFF; no rows in the database.
**Expected output:** `Ok(None)`.
**Acceptance:** `cargo test -p anvilml-registry --test device_store_tests test_lookup_unknown_pciid_returns_none` exits 0.

---

## test_lookup_boundary_0xffff (anvilml-registry)

**File:** `crates/anvilml-registry/tests/device_store_tests.rs`
**Context:** The `anvilml-registry` crate has been compiled with `sqlx` (sqlite, runtime-tokio, migrate, chrono features), `tokio` (macros feature), `anvilml-core` (types submodule with `InferenceCaps`), and `uuid` (v4 feature) dev-dependencies. `lookup` accepts `u16` arguments and casts them to `i64` for SQL binding — the maximum u16 value (0xFFFF) tests this cast path.
**Tests:** Queries for the maximum u16 values (vendor_id=0xFFFF, device_id=0xFFFF) and asserts that the result is `None` since no row exists at that ID.
**Mode:** both
**Inputs:** vendor_id=0xFFFF, device_id=0xFFFF; no rows in the database.
**Expected output:** `Ok(None)`.
**Acceptance:** `cargo test -p anvilml-registry --test device_store_tests test_lookup_boundary_0xffff` exits 0.

---

## test_lookup_integer_to_bool_mapping (anvilml-registry)

**File:** `crates/anvilml-registry/tests/device_store_tests.rs`
**Context:** The `anvilml-registry` crate has been compiled with `sqlx` (sqlite, runtime-tokio, migrate, chrono features), `tokio` (macros feature), `anvilml-core` (types submodule with `InferenceCaps`), and `uuid` (v4 feature) dev-dependencies. The `row_to_caps` helper maps INTEGER 0/1 columns to `bool` fields via `value != 0`.
**Tests:** Inserts a row with mixed 0/1 values (fp32=1, fp16=0, bf16=1, fp8=0, fp4=0, flash_attention=1) and asserts that the `row_to_caps` conversion produces the correct bool values.
**Mode:** both
**Inputs:** vendor_id=0x1234, device_id=0x5678; row with fp32=1, fp16=0, bf16=1, fp8=0, fp4=0, flash_attention=1.
**Expected output:** `Ok(Some(InferenceCaps { fp32: true, fp16: false, bf16: true, fp8: false, fp4: false, flash_attention: true }))`.
**Acceptance:** `cargo test -p anvilml-registry --test device_store_tests test_lookup_integer_to_bool_mapping` exits 0.

---

## test_lookup_multiple_ids_no_interference (anvilml-registry)

**File:** `crates/anvilml-registry/tests/device_store_tests.rs`
**Context:** The `anvilml-registry` crate has been compiled with `sqlx` (sqlite, runtime-tokio, migrate, chrono features), `tokio` (macros feature), `anvilml-core` (types submodule with `InferenceCaps`), and `uuid` (v4 feature) dev-dependencies. Each test gets its own in-memory SQLite pool, so multiple rows inserted in the same pool must not cause cross-contamination.
**Tests:** Inserts three rows with different PCI-IDs and different capability values, then verifies that each lookup returns only its own row's values — no cross-contamination between rows.
**Mode:** both
**Inputs:** Three rows: (0x1001, 0x1111) with fp32=1,fp16=1; (0x1002, 0x2222) with bf16=1,fp8=1; (0x10DE, 0x3333) with fp4=1,flash_attention=1.
**Expected output:** Each lookup returns `Some` with its own correct caps.
**Acceptance:** `cargo test -p anvilml-registry --test device_store_tests test_lookup_multiple_ids_no_interference` exits 0.

---

## test_already_applied_unseen_seed_returns_false (anvilml-registry)

**File:** `crates/anvilml-registry/tests/seed_loader_tests.rs`
**Context:** The `anvilml-registry` crate has been compiled with `sqlx` (sqlite, runtime-tokio, migrate, chrono features), `tokio` (macros feature), `anvilml-core`, and `uuid` (v4 feature) dev-dependencies. Each test creates its own in-memory SQLite pool with a unique uuid-based cache name.
**Tests:** Creates a fresh pool, constructs a `SeedLoader`, and calls `already_applied()` for a seed_name that has no row in `_seed_log`. The `_seed_log` table does not yet exist and should be created lazily by `already_applied()`.
**Mode:** both
**Inputs:** `seed_name="devices.sql"`, `sha256="abc123def456"`.
**Expected output:** `Ok(false)` — the seed has never been applied.
**Acceptance:** `cargo test -p anvilml-registry --test seed_loader_tests test_already_applied_unseen_seed_returns_false` exits 0.

---

## test_already_applied_hash_mismatch_returns_false (anvilml-registry)

**File:** `crates/anvilml-registry/tests/seed_loader_tests.rs`
**Context:** The `anvilml-registry` crate has been compiled with `sqlx` (sqlite, runtime-tokio, migrate, chrono features), `tokio` (macros feature), `anvilml-core`, and `uuid` (v4 feature) dev-dependencies. Each test creates its own in-memory SQLite pool with a unique uuid-based cache name.
**Tests:** Inserts a row into `_seed_log` with `seed_name="devices.sql"` and `sha256="old_hash"`, then calls `already_applied("devices.sql", "new_hash")`. Verifies that the method returns `false` because the hashes do not match.
**Mode:** both
**Inputs:** `seed_name="devices.sql"`, stored `sha256="old_hash"`, queried `sha256="new_hash"`.
**Expected output:** `Ok(false)` — the seed file has changed since last run.
**Acceptance:** `cargo test -p anvilml-registry --test seed_loader_tests test_already_applied_hash_mismatch_returns_false` exits 0.

---

## test_already_applied_hash_match_returns_true (anvilml-registry)

**File:** `crates/anvilml-registry/tests/seed_loader_tests.rs`
**Context:** The `anvilml-registry` crate has been compiled with `sqlx` (sqlite, runtime-tokio, migrate, chrono features), `tokio` (macros feature), `anvilml-core`, and `uuid` (v4 feature) dev-dependencies. Each test creates its own in-memory SQLite pool with a unique uuid-based cache name.
**Tests:** Inserts a row into `_seed_log` with `seed_name="devices.sql"` and `sha256="abc123"`, then calls `already_applied("devices.sql", "abc123")`. Verifies that the method returns `true` because the hashes match.
**Mode:** both
**Inputs:** `seed_name="devices.sql"`, stored `sha256="abc123"`, queried `sha256="abc123"`.
**Expected output:** `Ok(true)` — the seed has already been applied with this exact content.
**Acceptance:** `cargo test -p anvilml-registry --test seed_loader_tests test_already_applied_hash_match_returns_true` exits 0.

---

## test_seed_log_created_on_first_use (anvilml-registry)

**File:** `crates/anvilml-registry/tests/seed_loader_tests.rs`
**Context:** The `anvilml-registry` crate has been compiled with `sqlx` (sqlite, runtime-tokio, migrate, chrono features), `tokio` (macros feature), `anvilml-core`, and `uuid` (v4 feature) dev-dependencies. Each test creates its own in-memory SQLite pool with a unique uuid-based cache name.
**Tests:** Calls `already_applied()` on a fresh pool (no `_seed_log` table) and verifies the method returns `Ok(false)`. Then queries `sqlite_master` directly to confirm the `_seed_log` table was created.
**Mode:** both
**Inputs:** `seed_name="devices.sql"`, `sha256="abc123"`.
 **Expected output:** `Ok(false)` and `_seed_log` table exists in `sqlite_master`.
 **Acceptance:** `cargo test -p anvilml-registry --test seed_loader_tests test_seed_log_created_on_first_use` exits 0.

---

## test_run_first_time_applies_and_records (anvilml-registry)

**File:** `crates/anvilml-registry/tests/seed_loader_tests.rs`
**Context:** The `anvilml-registry` crate has been compiled with `sha2`, `digest`, `futures-util`, `chrono`, and `sqlx` dependencies. A temp file is created with valid INSERT SQL.
**Tests:** Calls `SeedLoader::run()` for the first time on a seed file with valid SQL. Verifies that the `_seed_log` table contains exactly one row with the seed name, the recorded hash matches the SHA256 of the file content, and a subsequent `already_applied()` call returns `true`.
**Mode:** both
**Inputs:** `seed_name="devices.sql"`, temp file with `INSERT INTO device_capabilities (...) VALUES ('test_device', 10de, 0, 16384);`.
**Expected output:** `_seed_log` has one row with the correct SHA256 hash; `already_applied()` returns `true` for the same hash.
**Acceptance:** `cargo test -p anvilml-registry --test seed_loader_tests test_run_first_time_applies_and_records` exits 0.

---

## test_run_skips_when_already_applied (anvilml-registry)

**File:** `crates/anvilml-registry/tests/seed_loader_tests.rs`
**Context:** The `anvilml-registry` crate has been compiled with `sha2`, `digest`, `futures-util`, `chrono`, and `sqlx` dependencies. A temp file is created with valid INSERT SQL.
**Tests:** Calls `SeedLoader::run()` twice with the same seed file. The second call should detect the hash match and skip execution. Verifies that the `_seed_log` row count stays at 1 and the `applied_at` timestamp is unchanged.
**Mode:** both
**Inputs:** Same seed file passed to `run()` twice consecutively.
**Expected output:** First run records hash+timestamp; second run returns `Ok(())` without changing the row or timestamp.
**Acceptance:** `cargo test -p anvilml-registry --test seed_loader_tests test_run_skips_when_already_applied` exits 0.

---

## test_run_reapplies_on_changed_content (anvilml-registry)

**File:** `crates/anvilml-registry/tests/seed_loader_tests.rs`
**Context:** The `anvilml-registry` crate has been compiled with `sha2`, `digest`, `futures-util`, `chrono`, and `sqlx` dependencies. A temp file is created with initial INSERT SQL, then modified with different content.
**Tests:** Calls `run()` with initial content, records the hash and timestamp, then modifies the file content and calls `run()` again. Verifies that the hash and `applied_at` timestamp in `_seed_log` both change.
**Mode:** both
**Inputs:** Seed file with `device_v1` content, then rewritten with `device_v2` content.
**Expected output:** Both `sha256` and `applied_at` in `_seed_log` change after the second run.
**Acceptance:** `cargo test -p anvilml-registry --test seed_loader_tests test_run_reapplies_on_changed_content` exits 0.

---

## test_run_malformed_sql_returns_err_no_partial_state (anvilml-registry)

**File:** `crates/anvilml-registry/tests/seed_loader_tests.rs`
**Context:** The `anvilml-registry` crate has been compiled with `sha2`, `digest`, `futures-util`, `chrono`, and `sqlx` dependencies. A temp file is created with invalid SQL.
**Tests:** Calls `SeedLoader::run()` on a seed file with invalid SQL (`INVALID SQL STATEMENT THAT WILL FAIL`). Verifies that `run()` returns an error, `_seed_log` has no row for the seed name (transaction rolled back), and `already_applied()` returns `false`.
**Mode:** both
**Inputs:** `seed_name="bad.sql"`, temp file with `INVALID SQL STATEMENT THAT WILL FAIL`.
**Expected output:** `run()` returns `Err`; `_seed_log` has zero rows for the seed; `already_applied()` returns `false`.
**Acceptance:** `cargo test -p anvilml-registry --test seed_loader_tests test_run_malformed_sql_returns_err_no_partial_state` exits 0.

---

## test_save_writes_file_once (anvilml-artifacts)

**File:** `crates/anvilml-artifacts/tests/store_tests.rs`
**Context:** The `anvilml-artifacts` crate has been compiled with `sha2` (0.11), `sqlx` (sqlite, runtime-tokio, migrate, chrono features), `chrono` (serde feature), `tokio` (macros feature), `tempfile`, and `uuid` (v4 feature) dependencies. Each test creates its own in-memory SQLite pool with a unique uuid-based cache name and its own temp directory.
**Tests:** Creates a tempdir and an `ArtifactStore` pointing to it (with an in-memory SQLite pool), calls `save()` with a known 64×64 black PNG byte slice, then verifies: the file exists at the expected content-addressed path `{tempdir}/{hash}.png`, the file size matches the input PNG size, the returned hash matches the computed SHA-256 of the input, and exactly one row exists in the `artifacts` table.
**Mode:** both
**Inputs:** 64×64 black PNG bytes (225 bytes), `ArtifactMeta { hash: "placeholder", job_id: <uuid>, width: 64, height: 64, seed: 42, steps: 20, created_at: <now>, file_path: "/tmp/artifacts/placeholder.png" }`.
**Expected output:** File exists at `{hash}.png` with correct size, returned hash matches SHA-256 of input, exactly one DB row in `artifacts` table.
**Acceptance:** `cargo test -p anvilml-artifacts --test store_tests test_save_writes_file_once` exits 0.

---

## test_duplicate_save_does_not_duplicate_or_error (anvilml-artifacts)

**File:** `crates/anvilml-artifacts/tests/store_tests.rs`
**Context:** The `anvilml-artifacts` crate has been compiled with `sha2` (0.11), `sqlx` (sqlite, runtime-tokio, migrate, chrono features), `chrono` (serde feature), `tokio` (macros feature), `tempfile`, and `uuid` (v4 feature) dependencies. Each test creates its own in-memory SQLite pool with a unique uuid-based cache name and its own temp directory.
**Tests:** Same setup as `test_save_writes_file_once`, but calls `save()` twice with the same PNG bytes. Verifies: exactly 1 PNG file exists in the artifact directory (no duplicate), both calls return `Ok(hash)` with the same hash, and the file content matches the original PNG bytes.
**Mode:** both
**Inputs:** Same 64×64 black PNG bytes passed to `save()` twice with the same `ArtifactMeta`.
**Expected output:** Exactly 1 file in artifact dir, both calls return `Ok(hash)`, file content matches original.
**Acceptance:** `cargo test -p anvilml-artifacts --test store_tests test_duplicate_save_does_not_duplicate_or_error` exits 0.

---

## test_different_content_produces_different_hash (anvilml-artifacts)

**File:** `crates/anvilml-artifacts/tests/store_tests.rs`
**Context:** The `anvilml-artifacts` crate has been compiled with `sha2` (0.11), `sqlx` (sqlite, runtime-tokio, migrate, chrono features), `chrono` (serde feature), `tokio` (macros feature), `tempfile`, and `uuid` (v4 feature) dependencies. Each test creates its own in-memory SQLite pool with a unique uuid-based cache name and its own temp directory.
**Tests:** Creates a tempdir and `ArtifactStore`, calls `save()` with two different PNG byte slices (64×64 black PNG vs 64×64 white PNG), then verifies: both files exist, the two hashes are different, and each file's content matches its corresponding input.
**Mode:** both
**Inputs:** 64×64 black PNG (225 bytes) and 64×64 white PNG (203 bytes), with different `ArtifactMeta` values (seed 42 vs seed 137).
**Expected output:** Two files exist at different `{hash}.png` paths, hashes differ, each file's content matches its corresponding input.
**Acceptance:** `cargo test -p anvilml-artifacts --test store_tests test_different_content_produces_different_hash` exits 0.

---

## test_save_then_get_roundtrips (anvilml-artifacts)

**File:** `crates/anvilml-artifacts/tests/store_tests.rs`
**Context:** The `anvilml-artifacts` crate has been compiled with `sha2` (0.11), `sqlx` (sqlite, runtime-tokio, migrate, chrono features), `chrono` (serde feature), `tokio` (macros feature), `tempfile`, and `uuid` (v4 feature) dependencies. Each test creates its own in-memory SQLite pool with a unique uuid-based cache name and its own temp directory.
**Tests:** Creates a tempdir and `ArtifactStore`, calls `save()` with a known PNG, then calls `get()` with the returned hash and verifies the retrieved bytes match the original input exactly.
**Mode:** both
**Inputs:** 64×64 black PNG (225 bytes), `ArtifactMeta` with seed 42.
**Expected output:** `get(hash)` returns `Ok(Some(bytes))` where bytes are byte-for-byte identical to the original PNG.
**Acceptance:** `cargo test -p anvilml-artifacts --test store_tests test_save_then_get_roundtrips` exits 0.

---

## test_get_unknown_hash_returns_none (anvilml-artifacts)

**File:** `crates/anvilml-artifacts/tests/store_tests.rs`
**Context:** The `anvilml-artifacts` crate has been compiled with `sha2` (0.11), `sqlx` (sqlite, runtime-tokio, migrate, chrono features), `chrono` (serde feature), `tokio` (macros feature), `tempfile`, and `uuid` (v4 feature) dependencies. Each test creates its own in-memory SQLite pool with a unique uuid-based cache name and its own temp directory.
**Tests:** Creates an empty tempdir and `ArtifactStore`, then calls `get()` with a random hex hash that does not correspond to any saved file. Verifies the result is `Ok(None)` — not an error, not `Some`.
**Mode:** both
**Inputs:** 64-character zeroed hex hash string (SHA-256 of all-zero bytes).
**Expected output:** `Ok(None)` — the content-addressed store correctly returns None for an unknown hash.
**Acceptance:** `cargo test -p anvilml-artifacts --test store_tests test_get_unknown_hash_returns_none` exits 0.

---

## test_get_after_duplicate_save_returns_original_content (anvilml-artifacts)

**File:** `crates/anvilml-artifacts/tests/store_tests.rs`
**Context:** The `anvilml-artifacts` crate has been compiled with `sha2` (0.11), `sqlx` (sqlite, runtime-tokio, migrate, chrono features), `chrono` (serde feature), `tokio` (macros feature), `tempfile`, and `uuid` (v4 feature) dependencies. Each test creates its own in-memory SQLite pool with a unique uuid-based cache name and its own temp directory.
**Tests:** Creates a tempdir and `ArtifactStore`, saves two different PNGs (black and white) producing two different hashes, then calls `get()` for each hash and verifies each returns its own content — proving content-addressed retrieval is not confused by having multiple files.
**Mode:** both
**Inputs:** 64×64 black PNG (225 bytes, seed 42) and 64×64 white PNG (203 bytes, seed 137).
**Expected output:** `get(hash1)` returns the black PNG bytes, `get(hash2)` returns the white PNG bytes — each hash maps to its own file content.
**Acceptance:** `cargo test -p anvilml-artifacts --test store_tests test_get_after_duplicate_save_returns_original_content` exits 0.

---

## test_list_with_job_id_filter (anvilml-artifacts)

**File:** `crates/anvilml-artifacts/tests/store_tests.rs`
**Context:** The `anvilml-artifacts` crate has been compiled with `sqlx` (sqlite, runtime-tokio, migrate, chrono, uuid features), `chrono` (serde feature), `uuid` (v4 feature), and `tokio` dev-dependency. Two artifacts are saved under different job IDs via `save()`.
**Tests:** `list(Some(job_id_a))` returns only the artifact whose `job_id` matches the given filter — proves the WHERE clause correctly filters by the bound UUID parameter.
**Mode:** both
**Inputs:** Two artifacts saved under distinct `Uuid` values (job_id_a, job_id_b).
**Expected output:** `list(Some(job_id_a))` returns a `Vec` with exactly 1 `ArtifactMeta` whose `job_id` equals `job_id_a`.
**Acceptance:** `cargo test -p anvilml-artifacts --test store_tests test_list_with_job_id_filter` exits 0.

---

## test_list_without_filter_returns_all (anvilml-artifacts)

**File:** `crates/anvilml-artifacts/tests/store_tests.rs`
**Context:** The `anvilml-artifacts` crate has been compiled with `sqlx` (sqlite, runtime-tokio, migrate, chrono, uuid features), `chrono` (serde feature), `uuid` (v4 feature), and `tokio` dev-dependency. Three artifacts are saved under two different job IDs using three distinct PNG byte slices (TEST_PNG, TEST_PNG_WHITE, and a modified copy of TEST_PNG).
**Tests:** `list(None)` returns all three artifact rows regardless of job ID — proves the unfiltered SELECT returns every row in the table.
**Mode:** both
**Inputs:** Three artifacts saved with distinct content under two job IDs (job_id_a: 2 artifacts, job_id_b: 1 artifact).
**Expected output:** `list(None)` returns a `Vec` with exactly 3 `ArtifactMeta` entries.
**Acceptance:** `cargo test -p anvilml-artifacts --test store_tests test_list_without_filter_returns_all` exits 0.

---

## test_list_empty_table_returns_empty_vec (anvilml-artifacts)

**File:** `crates/anvilml-artifacts/tests/store_tests.rs`
**Context:** The `anvilml-artifacts` crate has been compiled with `sqlx` (sqlite, runtime-tokio, migrate, chrono, uuid features), `chrono` (serde feature), `uuid` (v4 feature), and `tokio` dev-dependency. No artifacts are saved — the `artifacts` table is created on first `list()` call via `ensure_artifacts_table()`.
**Tests:** `list(None)` on an empty table returns an empty `Vec` (not `None` or an error) — proves the method handles the zero-row case gracefully.
**Mode:** both
**Inputs:** No artifacts saved; empty `artifacts` table.
**Expected output:** `list(None)` returns an empty `Vec` (`len() == 0`).
**Acceptance:** `cargo test -p anvilml-artifacts --test store_tests test_list_empty_table_returns_empty_vec` exits 0.

---

## test_publish_zero_subscribers (anvilml-ipc)

**File:** `crates/anvilml-ipc/tests/roundtrip_tests.rs`
**Context:** The `anvilml-ipc` crate has been compiled with `anvilml-core` and `tokio` (sync feature) dependencies.
**Tests:** `publish()` with zero subscribers does not panic — the internal `send()` returns `Err(SendError)` which `publish()` silently discards.
**Mode:** both
**Inputs:** `WsEvent::JobQueued { job_id: Uuid::new_v4(), queue_position: 1 }` published to a fresh `EventBroadcaster` with no subscribers.
**Expected output:** `publish()` returns without panic (SendError silently ignored).
**Acceptance:** `cargo test -p anvilml-ipc --test roundtrip_tests test_publish_zero_subscribers` exits 0.

---

## test_publish_one_subscriber_delivers (anvilml-ipc)

**File:** `crates/anvilml-ipc/tests/roundtrip_tests.rs`
**Context:** The `anvilml-ipc` crate has been compiled with `anvilml-core` and `tokio` (sync, macros, rt-multi-thread features) dependencies.
**Tests:** `publish()` with one subscriber delivers the event — the subscriber's `recv().await` returns the exact event that was published.
**Mode:** both
**Inputs:** `WsEvent::JobStarted { job_id, worker_id: "gpu:0" }` published to an `EventBroadcaster` with one active subscriber.
**Expected output:** `receiver.recv().await` returns `Ok(event)` equal to the published event.
**Acceptance:** `cargo test -p anvilml-ipc --test roundtrip_tests test_publish_one_subscriber_delivers` exits 0.

---

## test_publish_multiple_subscribers_independent_copies (anvilml-ipc)

**File:** `crates/anvilml-ipc/tests/roundtrip_tests.rs`
**Context:** The `anvilml-ipc` crate has been compiled with `anvilml-core` and `tokio` (sync, macros, rt-multi-thread features) dependencies.
**Tests:** Multiple subscribers each receive their own independent copy of the event — publishing one event to two subscribers results in both receivers getting the event.
**Mode:** both
**Inputs:** `WsEvent::JobCompleted { job_id, elapsed_ms: 42 }` published to an `EventBroadcaster` with two active subscribers.
**Expected output:** Both `recv().await` calls return `Ok(event)` equal to the published event.
**Acceptance:** `cargo test -p anvilml-ipc --test roundtrip_tests test_publish_multiple_subscribers_independent_copies` exits 0.

---

## test_subscribe_returns_valid_receiver (anvilml-ipc)

**File:** `crates/anvilml-ipc/tests/roundtrip_tests.rs`
**Context:** The `anvilml-ipc` crate has been compiled with `anvilml-core` and `tokio` (sync, macros, rt-multi-thread features) dependencies.
**Tests:** `subscribe()` returns a receiver that is valid — calling `recv().await` does not immediately return `RecvError::Closed` before any publish occurs.
**Mode:** both
**Inputs:** None (structural test — creates `EventBroadcaster::new()` and calls `subscribe()`).
**Expected output:** `recv().await` does not return `RecvError::Closed` immediately; the receiver is open and waiting for events.
**Acceptance:** `cargo test -p anvilml-ipc --test roundtrip_tests test_subscribe_returns_valid_receiver` exits 0.

---

## test_anvilml_log_debug_yields_stderr (backend)

**File:** `backend/tests/logging_tests.rs`
**Context:** The `anvilml` binary has been compiled (`cargo build -p anvilml`). The `tracing-subscriber` crate is present and initialized in `main()` before CLI parsing.
**Tests:** Setting `ANVILML_LOG=debug` causes the spawned `anvilml` binary to emit non-empty stderr when running `hw-probe`, because hardware detection code paths contain `tracing::debug!()` calls that become visible at debug level.
**Mode:** both
**Inputs:** `ANVILML_LOG=debug` env var set; `hw-probe` subcommand passed to the binary.
**Expected output:** `output.stderr` is non-empty (contains at least one tracing-formatted log line from hardware detection).
**Acceptance:** `cargo test -p anvilml --test logging_tests -- test_anvilml_log_debug_yields_stderr` exits 0.

---

## test_rust_log_debug_yields_stderr (backend)

**File:** `backend/tests/logging_tests.rs`
**Context:** The `anvilml` binary has been compiled (`cargo build -p anvilml`). The `tracing-subscriber` crate is present and initialized in `main()` before CLI parsing. `ANVILML_LOG` must not be set so that `RUST_LOG` is the active filter source.
**Tests:** Setting `RUST_LOG=debug` (when `ANVILML_LOG` is unset) causes the spawned `anvilml` binary to emit non-empty stderr, proving the fallback chain (`ANVILML_LOG` → `RUST_LOG` → `"info"`) works correctly per `ENVIRONMENT.md §3.3`.
**Mode:** both
**Inputs:** `RUST_LOG=debug` env var set; `ANVILML_LOG` unset; `hw-probe` subcommand passed to the binary.
**Expected output:** `output.stderr` is non-empty (contains at least one tracing-formatted log line from hardware detection).
**Acceptance:** `cargo test -p anvilml --test logging_tests -- test_rust_log_debug_yields_stderr` exits 0.

---

## test_log_format_json_produces_json_lines (backend)

**File:** `backend/tests/logging_tests.rs`
**Context:** The `anvilml` binary has been compiled (`cargo build -p anvilml`). The `tracing-subscriber` crate has the `json` feature enabled (`backend/Cargo.toml`), and `--log-format` is a valid CLI flag.
**Tests:** Setting `ANVILML_LOG=debug` and passing `--log-format json` causes the spawned `anvilml` binary to emit newline-delimited JSON lines on stderr. Each non-empty stderr line is parsed as JSON and verified to contain at least a `level` or `msg` field (fields that tracing-subscriber always emits in JSON mode).
**Mode:** both
**Inputs:** `ANVILML_LOG=debug` env var set; `--log-format json` and `hw-probe` passed to the binary.
**Expected output:** `output.stderr` is non-empty; every non-empty line parses as valid JSON with a `level` or `msg` field.
**Acceptance:** `cargo test -p anvilml --test logging_tests -- test_log_format_json_produces_json_lines` exits 0.

---

## test_log_format_plain_produces_text_lines (backend)

**File:** `backend/tests/logging_tests.rs`
**Context:** The `anvilml` binary has been compiled (`cargo build -p anvilml`). The `--log-format plain` flag is valid and produces the default plain-text output.
**Tests:** Setting `ANVILML_LOG=debug` and passing `--log-format plain` causes the spawned `anvilml` binary to emit plain-text (non-JSON) lines on stderr. At least one non-empty stderr line is verified to NOT be valid JSON, confirming the plain-text formatter is active.
**Mode:** both
**Inputs:** `ANVILML_LOG=debug` env var set; `--log-format plain` and `hw-probe` passed to the binary.
**Expected output:** `output.stderr` is non-empty; at least one line is NOT valid JSON (plain-text format like `2024-01-01T00:00:00.000Z  INFO ...`).
**Acceptance:** `cargo test -p anvilml --test logging_tests -- test_log_format_plain_produces_text_lines` exits 0.

---

## test_log_format_invalid_exits_nonzero (backend)

**File:** `backend/tests/logging_tests.rs`
**Context:** The `anvilml` binary has been compiled (`cargo build -p anvilml`). The `--log-format` flag accepts only `"plain"` or `"json"`; clap exits with code 2 on validation failure.
**Tests:** Passing `--log-format invalid_value` causes the binary to exit with a non-zero exit code (clap code 2), because the value is not one of the validated alternatives.
**Mode:** both
**Inputs:** `--log-format invalid_value` and `hw-probe` passed to the binary.
**Expected output:** Non-zero exit code (clap validation failure, exit code 2).
**Acceptance:** `cargo test -p anvilml --test logging_tests -- test_log_format_invalid_exits_nonzero` exits 0.

---

## test_db_file_created_on_startup (backend)

**File:** `backend/tests/db_startup_tests.rs`
**Context:** The `anvilml` binary has been compiled (`cargo build -p anvilml`). The binary's default startup path now calls `create_pool()` from `anvilml-registry`, which creates the SQLite database and runs migrations before binding the TCP listener.
**Tests:** Spawning the binary with `ANVILML_DB_PATH` set to a temp directory path and `ANVILML_PORT=0` (ephemeral port) triggers database creation. The test waits up to 5 seconds for the "listening" log line on stderr, then asserts the `.db` file exists on disk.
**Mode:** both
**Inputs:** `ANVILML_DB_PATH` = temp file path (unique per test via `tempfile::tempdir()`), `ANVILML_PORT=0`, no subcommand (default path).
**Expected output:** `.db` file exists after binary starts; "listening" log line appears on stderr.
**Acceptance:** `cargo test -p anvilml --test db_startup_tests -- test_db_file_created_on_startup` exits 0.

---

## test_migrations_create_required_tables (backend)

**File:** `backend/tests/db_startup_tests.rs`
**Context:** The `anvilml` binary has been compiled (`cargo build -p anvilml`). The `create_pool()` function runs all migrations from `database/migrations/`, which creates the `models` and `device_capabilities` tables.
**Tests:** Spawning the binary with `ANVILML_DB_PATH` set to a temp directory path and `ANVILML_PORT=0` triggers database creation and migration. After confirming the "listening" log line, the test connects to the database with `sqlx` and queries `sqlite_master` to verify both `models` and `device_capabilities` tables exist.
**Mode:** both
**Inputs:** `ANVILML_DB_PATH` = temp file path (unique per test via `tempfile::tempdir()`), `ANVILML_PORT=0`, no subcommand (default path).
**Expected output:** `sqlite_master` query returns both `models` and `device_capabilities` table names.
**Acceptance:** `cargo test -p anvilml --test db_startup_tests -- test_migrations_create_required_tables` exits 0.

---

## test_seed_populates_device_capabilities (backend)

**File:** `backend/tests/db_startup_tests.rs`
**Context:** The `anvilml` binary has been compiled (`cargo build -p anvilml`). The binary's default startup path now calls `SeedLoader::run()` from `anvilml-registry` after `create_pool()`, loading device capability seed data from `database/seeds/devices.sql` (353 INSERT statements).
**Tests:** Spawning the binary with `ANVILML_DB_PATH` set to a temp directory path and `ANVILML_PORT=0` triggers database creation, migrations, and seed loading. After confirming the "listening" log line, the test connects to the database with `sqlx` and queries `SELECT COUNT(*) FROM device_capabilities`, asserting the count is greater than 0 (should be 353 matching the INSERT count in devices.sql).
**Mode:** both
**Inputs:** `ANVILML_DB_PATH` = temp file path (unique per test via `tempfile::tempdir()`), `ANVILML_PORT=0`, no subcommand (default path).
**Expected output:** `device_capabilities` table contains 353 rows after startup; "listening" log line appears on stderr.
**Acceptance:** `cargo test -p anvilml --test db_startup_tests -- test_seed_populates_device_capabilities` exits 0.

---

## test_seed_idempotent_second_run (backend)

**File:** `backend/tests/db_startup_tests.rs`
**Context:** The `anvilml` binary has been compiled (`cargo build -p anvilml`). The `SeedLoader::run()` method is hash-gated: it computes a SHA256 hash of the seed file, checks `_seed_log` for a matching hash, and skips re-application if found.
**Tests:** Spawns the binary twice with the same temp `db_path` and `ANVILML_PORT=0`. After each spawn, it connects to the database and records the `device_capabilities` row count. Asserts the counts are equal, proving the seed is idempotent (no duplicate rows on second run).
**Mode:** both
**Inputs:** Same temp `db_path` for both spawns (via `tempfile::tempdir()`), `ANVILML_PORT=0`, no subcommand (default path).
**Expected output:** Row count after second run equals row count after first run (no duplicates).
**Acceptance:** `cargo test -p anvilml --test db_startup_tests -- test_seed_idempotent_second_run` exits 0.

---

## test_missing_seed_file_causes_startup_failure (backend)

**File:** `backend/tests/db_startup_tests.rs`
**Context:** The `anvilml` binary has been compiled (`cargo build -p anvilml`). The `SeedLoader::run()` method calls `std::fs::read()` on the seed path and returns `AnvilError::Io` on file-not-found. The `main()` function handles this error with `eprintln!` + `std::process::exit(1)`.
**Tests:** Spawns the binary with `ANVILML_SEED_PATH` set to `/tmp/nonexistent_seed.sql` (a path that does not exist) and `ANVILML_PORT=0`. Asserts the process exits with a non-zero code within 10 seconds. This test does NOT wait for the "listening" log line — the binary should never reach TCP bind with a missing seed.
**Mode:** both
**Inputs:** `ANVILML_SEED_PATH=/tmp/nonexistent_seed.sql`, `ANVILML_PORT=0`, no subcommand (default path).
**Expected output:** Process exits with non-zero code within 10 seconds; no "listening" log line produced.
**Acceptance:** `cargo test -p anvilml --test db_startup_tests -- test_missing_seed_file_causes_startup_failure` exits 0.

---

## test_anvilml_log_precedence_over_rust_log (backend)

**File:** `backend/tests/logging_tests.rs`
**Context:** The `anvilml` binary has been compiled (`cargo build -p anvilml`). The binary's logging initialization checks `ANVILML_LOG` first, falling back to `RUST_LOG` when `ANVILML_LOG` is unset (per `ENVIRONMENT.md §3.3`).
**Tests:** Sets both `ANVILML_LOG=debug` and `RUST_LOG=error`, spawns the binary with `hw-probe`, and asserts stderr is non-empty. `RUST_LOG=error` suppresses all debug-level tracing output; non-empty stderr proves `ANVILML_LOG` was the active filter, confirming the precedence rule.
**Mode:** both
**Inputs:** `ANVILML_LOG=debug`, `RUST_LOG=error`, `hw-probe` subcommand.
**Expected output:** stderr is non-empty (debug-level tracing from hardware detection).
**Acceptance:** `cargo test -p anvilml --test logging_tests -- test_anvilml_log_precedence_over_rust_log` exits 0.

---

## test_bind_failed_display (anvilml-ipc)

**File:** `crates/anvilml-ipc/tests/error_tests.rs`
**Context:** The `anvilml-ipc` crate has been compiled with `thiserror` (derive feature) providing `#[derive(thiserror::Error)]` on `IpcError`. The `IpcError::BindFailed(String)` variant carries a `#[error("bind failed: {0}")]` attribute.
**Tests:** Constructs `IpcError::BindFailed("address already in use")` and asserts its `Display` output matches `"bind failed: address already in use"`.
**Mode:** both
**Inputs:** `IpcError::BindFailed("address already in use".to_string())`.
**Expected output:** `to_string()` returns `"bind failed: address already in use"`.
**Acceptance:** `cargo test -p anvilml-ipc --test error_tests test_bind_failed_display` exits 0.

---

## test_send_failed_display (anvilml-ipc)

**File:** `crates/anvilml-ipc/tests/error_tests.rs`
**Context:** The `anvilml-ipc` crate has been compiled with `thiserror` (derive feature). The `IpcError::SendFailed(String)` variant carries a `#[error("send failed: {0}")]` attribute.
**Tests:** Constructs `IpcError::SendFailed("connection closed")` and asserts its `Display` output matches `"send failed: connection closed"`.
**Mode:** both
**Inputs:** `IpcError::SendFailed("connection closed".to_string())`.
**Expected output:** `to_string()` returns `"send failed: connection closed"`.
**Acceptance:** `cargo test -p anvilml-ipc --test error_tests test_send_failed_display` exits 0.

---

## test_recv_failed_display (anvilml-ipc)

**File:** `crates/anvilml-ipc/tests/error_tests.rs`
**Context:** The `anvilml-ipc` crate has been compiled with `thiserror` (derive feature). The `IpcError::RecvFailed(String)` variant carries a `#[error("recv failed: {0}")]` attribute.
**Tests:** Constructs `IpcError::RecvFailed("timeout")` and asserts its `Display` output matches `"recv failed: timeout"`.
**Mode:** both
**Inputs:** `IpcError::RecvFailed("timeout".to_string())`.
**Expected output:** `to_string()` returns `"recv failed: timeout"`.
**Acceptance:** `cargo test -p anvilml-ipc --test error_tests test_recv_failed_display` exits 0.

---

## test_serialization_failed_display (anvilml-ipc)

**File:** `crates/anvilml-ipc/tests/error_tests.rs`
**Context:** The `anvilml-ipc` crate has been compiled with `thiserror` (derive feature). The `IpcError::SerializationFailed(String)` variant carries a `#[error("serialization failed: {0}")]` attribute.
**Tests:** Constructs `IpcError::SerializationFailed("unsupported type")` and asserts its `Display` output matches `"serialization failed: unsupported type"`.
**Mode:** both
**Inputs:** `IpcError::SerializationFailed("unsupported type".to_string())`.
**Expected output:** `to_string()` returns `"serialization failed: unsupported type"`.
**Acceptance:** `cargo test -p anvilml-ipc --test error_tests test_serialization_failed_display` exits 0.

---

## test_payload_too_large_display (anvilml-ipc)

**File:** `crates/anvilml-ipc/tests/error_tests.rs`
**Context:** The `anvilml-ipc` crate has been compiled with `thiserror` (derive feature). The `IpcError::PayloadTooLarge` struct variant carries `#[error("payload too large: {actual} > {max}")]` attribute with named struct fields.
**Tests:** Constructs `IpcError::PayloadTooLarge { actual: 1024, max: 512 }` and asserts its `Display` output includes both values in the format `"payload too large: 1024 > 512"`.
**Mode:** both
**Inputs:** `IpcError::PayloadTooLarge { actual: 1024, max: 512 }`.
**Expected output:** `to_string()` returns `"payload too large: 1024 > 512"`.
**Acceptance:** `cargo test -p anvilml-ipc --test error_tests test_payload_too_large_display` exits 0.

---

## test_unknown_worker_display (anvilml-ipc)

**File:** `crates/anvilml-ipc/tests/error_tests.rs`
**Context:** The `anvilml-ipc` crate has been compiled with `thiserror` (derive feature). The `IpcError::UnknownWorker(String)` variant carries a `#[error("unknown worker: {0}")]` attribute.
**Tests:** Constructs `IpcError::UnknownWorker("gpu:3")` and asserts its `Display` output matches `"unknown worker: gpu:3"`.
**Mode:** both
**Inputs:** `IpcError::UnknownWorker("gpu:3".to_string())`.
**Expected output:** `to_string()` returns `"unknown worker: gpu:3"`.
**Acceptance:** `cargo test -p anvilml-ipc --test error_tests test_unknown_worker_display` exits 0.

---

## test_from_ipc_error_to_anvil_error (anvilml-ipc)

**File:** `crates/anvilml-ipc/tests/error_tests.rs`
**Context:** The `anvilml-ipc` crate has been compiled with `anvilml-core` (path dependency) providing `AnvilError::Ipc(String)`. The `IpcError` enum implements `From<IpcError> for AnvilError` via `AnvilError::Ipc(err.to_string())`.
**Tests:** Converts all six `IpcError` variants to `AnvilError` via `From` and asserts each produces `AnvilError::Ipc(_)` with the correct message matching the variant's `Display` output.
**Mode:** both
**Inputs:** All six `IpcError` variants: `BindFailed`, `SendFailed`, `RecvFailed`, `SerializationFailed`, `PayloadTooLarge { actual: 1024, max: 512 }`, `UnknownWorker`.
**Expected output:** Each variant converts to `AnvilError::Ipc(msg)` where `msg` matches the variant's `Display` output.
**Acceptance:** `cargo test -p anvilml-ipc --test error_tests test_from_ipc_error_to_anvil_error` exits 0.

---

## test_ping_roundtrip (anvilml-ipc)

**File:** `crates/anvilml-ipc/tests/roundtrip_tests.rs`
**Context:** The `anvilml-ipc` crate has been compiled with `rmp-serde` and `uuid` (v4, serde) dev-dependencies, and the `messages` module providing `WorkerMessage`.
**Tests:** `WorkerMessage::Ping { seq: 42 }` serialises via `rmp_serde::to_vec_named()` and roundtrips to an equal value. The msgpack dict contains `"_type": "Ping"` and `"seq": 42`.
**Mode:** both
**Inputs:** `WorkerMessage::Ping { seq: 42 }`.
**Expected output:** Roundtripped `WorkerMessage::Ping { seq: 42 }` equals original.
**Acceptance:** `cargo test -p anvilml-ipc --test roundtrip_tests test_ping_roundtrip` exits 0.

---

## test_shutdown_roundtrip (anvilml-ipc)

**File:** `crates/anvilml-ipc/tests/roundtrip_tests.rs`
**Context:** The `anvilml-ipc` crate has been compiled with `rmp-serde` and `uuid` (v4, serde) dev-dependencies, and the `messages` module providing `WorkerMessage`.
**Tests:** `WorkerMessage::Shutdown` (unit variant, no fields) roundtrips via `rmp_serde::to_vec_named()`. The msgpack dict contains only `"_type": "Shutdown"`.
**Mode:** both
**Inputs:** `WorkerMessage::Shutdown`.
**Expected output:** Roundtripped `WorkerMessage::Shutdown` equals original.
**Acceptance:** `cargo test -p anvilml-ipc --test roundtrip_tests test_shutdown_roundtrip` exits 0.

---

## test_execute_roundtrip (anvilml-ipc)

**File:** `crates/anvilml-ipc/tests/roundtrip_tests.rs`
**Context:** The `anvilml-ipc` crate has been compiled with `rmp-serde`, `uuid` (v4, serde), and `serde_json` dev-dependencies, and the `messages` module providing `WorkerMessage`. The `anvilml-core` crate provides `JobSettings`.
**Tests:** `WorkerMessage::Execute { job_id, graph, settings, device_index }` roundtrips via `rmp_serde::to_vec_named()`. All four fields (`job_id`, `graph`, `settings`, `device_index`) are preserved with correct types (Uuid→string, Value→dict, JobSettings→dict, u32→int).
**Mode:** both
**Inputs:** `WorkerMessage::Execute { job_id: Uuid::new_v4(), graph: serde_json::json!({}), settings: JobSettings { device_preference: None }, device_index: 0 }`.
**Expected output:** Roundtripped `WorkerMessage::Execute` equals original; all four fields preserved.
**Acceptance:** `cargo test -p anvilml-ipc --test roundtrip_tests test_execute_roundtrip` exits 0.

---

## test_cancel_job_roundtrip (anvilml-ipc)

**File:** `crates/anvilml-ipc/tests/roundtrip_tests.rs`
**Context:** The `anvilml-ipc` crate has been compiled with `rmp-serde` and `uuid` (v4, serde) dev-dependencies, and the `messages` module providing `WorkerMessage`.
**Tests:** `WorkerMessage::CancelJob { job_id }` roundtrips via `rmp_serde::to_vec_named()`. The `job_id` field is preserved correctly across serialisation.
**Mode:** both
**Inputs:** `WorkerMessage::CancelJob { job_id: Uuid::new_v4() }`.
**Expected output:** Roundtripped `WorkerMessage::CancelJob` equals original; `job_id` preserved.
**Acceptance:** `cargo test -p anvilml-ipc --test roundtrip_tests test_cancel_job_roundtrip` exits 0.

---

## test_memory_query_roundtrip (anvilml-ipc)

**File:** `crates/anvilml-ipc/tests/roundtrip_tests.rs`
**Context:** The `anvilml-ipc` crate has been compiled with `rmp-serde` dev-dependencies, and the `messages` module providing `WorkerMessage`.
**Tests:** `WorkerMessage::MemoryQuery` (unit variant, no fields) roundtrips via `rmp_serde::to_vec_named()`. The msgpack dict contains only `"_type": "MemoryQuery"`.
**Mode:** both
**Inputs:** `WorkerMessage::MemoryQuery`.
**Expected output:** Roundtripped `WorkerMessage::MemoryQuery` equals original.
**Acceptance:** `cargo test -p anvilml-ipc --test roundtrip_tests test_memory_query_roundtrip` exits 0.

---

## test_ready_roundtrip (anvilml-ipc)

**File:** `crates/anvilml-ipc/tests/roundtrip_tests.rs`
**Context:** The `anvilml-ipc` crate has been compiled with `rmp-serde` and `uuid` (v4, serde) dev-dependencies, and the `messages` module providing `WorkerMessage` and `WorkerEvent`. The `anvilml-core` crate provides `NodeTypeDescriptor`.
**Tests:** `WorkerEvent::Ready` with all 13 fields roundtrips via `rmp_serde::to_vec_named()`. Constructs a realistic Ready event with worker_id="gpu:0", device_index=0, device_name="NVIDIA RTX 4090", device_type="cuda", vram_total_mib=24576, vram_free_mib=20480, torch_version="2.5.1+cu124", fp16=true, bf16=true, fp8=true, flash_attention=true, capabilities_source="pytorch", and two `NodeTypeDescriptor` entries (LoadModel, KSampler). Verifies the deserialised event equals the original.
**Mode:** both
**Inputs:** Full `WorkerEvent::Ready` with all 13 fields at representative values.
**Expected output:** Roundtripped `WorkerEvent::Ready` equals original; all 13 fields preserved including `node_types` vec with two entries.
**Acceptance:** `cargo test -p anvilml-ipc --test roundtrip_tests test_ready_roundtrip` exits 0.

---

## test_pong_roundtrip (anvilml-ipc)

**File:** `crates/anvilml-ipc/tests/roundtrip_tests.rs`
**Context:** The `anvilml-ipc` crate has been compiled with `rmp-serde` dev-dependencies, and the `messages` module providing `WorkerEvent`.
**Tests:** `WorkerEvent::Pong { seq: 42 }` roundtrips via `rmp_serde::to_vec_named()`. The msgpack dict contains `"_type": "Pong"` and `"seq": 42`.
**Mode:** both
**Inputs:** `WorkerEvent::Pong { seq: 42 }`.
**Expected output:** Roundtripped `WorkerEvent::Pong { seq: 42 }` equals original.
**Acceptance:** `cargo test -p anvilml-ipc --test roundtrip_tests test_pong_roundtrip` exits 0.

---

## test_dying_roundtrip (anvilml-ipc)

**File:** `crates/anvilml-ipc/tests/roundtrip_tests.rs`
**Context:** The `anvilml-ipc` crate has been compiled with `rmp-serde` dev-dependencies, and the `messages` module providing `WorkerEvent`.
**Tests:** `WorkerEvent::Dying { reason: "OOM" }` roundtrips via `rmp_serde::to_vec_named()`. The msgpack dict contains `"_type": "Dying"` and `"reason": "OOM"`.
**Mode:** both
**Inputs:** `WorkerEvent::Dying { reason: "OOM" }`.
**Expected output:** Roundtripped `WorkerEvent::Dying { reason: "OOM" }` equals original.
**Acceptance:** `cargo test -p anvilml-ipc --test roundtrip_tests test_dying_roundtrip` exits 0.

---

## test_memory_report_roundtrip (anvilml-ipc)

**File:** `crates/anvilml-ipc/tests/roundtrip_tests.rs`
**Context:** The `anvilml-ipc` crate has been compiled with `rmp-serde` dev-dependencies, and the `messages` module providing `WorkerEvent`.
**Tests:** `WorkerEvent::MemoryReport { vram_used_mib: 4096, ram_used_mib: 8589934592 }` roundtrips via `rmp_serde::to_vec_named()`. The msgpack dict contains `"_type": "MemoryReport"`, `"vram_used_mib": 4096`, and `"ram_used_mib": 8589934592`. Verifies the `u32` and `u64` fields are preserved correctly across serialisation.
**Mode:** both
**Inputs:** `WorkerEvent::MemoryReport { vram_used_mib: 4096, ram_used_mib: 8589934592 }`.
**Expected output:** Roundtripped `WorkerEvent::MemoryReport` equals original; both `vram_used_mib` and `ram_used_mib` preserved.
**Acceptance:** `cargo test -p anvilml-ipc --test roundtrip_tests test_memory_report_roundtrip` exits 0.

---

## test_progress_roundtrip (anvilml-ipc)

**File:** `crates/anvilml-ipc/tests/roundtrip_tests.rs`
**Context:** The `anvilml-ipc` crate has been compiled with `rmp-serde` dev-dependencies, and the `messages` module providing `WorkerEvent` with the `Progress` variant (added by P7-A4).
**Tests:** `WorkerEvent::Progress { job_id: Uuid::new_v4(), step: 3, total_steps: 20, preview_b64: Some("iVBORw0KGgo...") }` roundtrips via `rmp_serde::to_vec_named()`. The msgpack dict contains `"_type": "Progress"`, `"job_id"` (UUID string), `"step": 3`, `"total_steps": 20`, and `"preview_b64": "iVBORw0KGgo..."`. Verifies all four fields including the `Option<String>` field are preserved correctly across serialisation.
**Mode:** both
**Inputs:** `WorkerEvent::Progress { job_id: Uuid::new_v4(), step: 3, total_steps: 20, preview_b64: Some("iVBORw0KGgo...".into()) }`.
**Expected output:** Roundtripped `WorkerEvent::Progress` equals original; all four fields preserved.
**Acceptance:** `cargo test -p anvilml-ipc --test roundtrip_tests test_progress_roundtrip` exits 0.

---

## test_image_ready_roundtrip (anvilml-ipc)

**File:** `crates/anvilml-ipc/tests/roundtrip_tests.rs`
**Context:** The `anvilml-ipc` crate has been compiled with `rmp-serde` dev-dependencies, and the `messages` module providing `WorkerEvent` with the `ImageReady` variant (added by P7-A4).
**Tests:** `WorkerEvent::ImageReady { job_id, image_b64: "iVBORw0KGgo...", width: 512, height: 512, format: "png", seed: 42, steps: 20 }` roundtrips via `rmp_serde::to_vec_named()`. The msgpack dict contains `"_type": "ImageReady"` plus all seven field keys. Verifies `i64` (seed), `u32` (width/height/steps), and `String` fields (image_b64, format) are preserved correctly.
**Mode:** both
**Inputs:** `WorkerEvent::ImageReady { job_id: Uuid::new_v4(), image_b64: "iVBORw0KGgo...".into(), width: 512, height: 512, format: "png".into(), seed: 42, steps: 20 }`.
**Expected output:** Roundtripped `WorkerEvent::ImageReady` equals original; all seven fields preserved.
**Acceptance:** `cargo test -p anvilml-ipc --test roundtrip_tests test_image_ready_roundtrip` exits 0.

---

## test_completed_roundtrip (anvilml-ipc)

**File:** `crates/anvilml-ipc/tests/roundtrip_tests.rs`
**Context:** The `anvilml-ipc` crate has been compiled with `rmp-serde` dev-dependencies, and the `messages` module providing `WorkerEvent` with the `Completed` variant (added by P7-A4).
**Tests:** `WorkerEvent::Completed { job_id, elapsed_ms: 5432 }` roundtrips via `rmp_serde::to_vec_named()`. The msgpack dict contains `"_type": "Completed"`, `"job_id"` (UUID string), and `"elapsed_ms": 5432`. Verifies the `u64` elapsed_ms field is preserved correctly across serialisation.
**Mode:** both
**Inputs:** `WorkerEvent::Completed { job_id: Uuid::new_v4(), elapsed_ms: 5432 }`.
**Expected output:** Roundtripped `WorkerEvent::Completed` equals original; job_id and elapsed_ms preserved.
**Acceptance:** `cargo test -p anvilml-ipc --test roundtrip_tests test_completed_roundtrip` exits 0.

---

## test_failed_roundtrip (anvilml-ipc)

**File:** `crates/anvilml-ipc/tests/roundtrip_tests.rs`
**Context:** The `anvilml-ipc` crate has been compiled with `rmp-serde` dev-dependencies, and the `messages` module providing `WorkerEvent` with the `Failed` variant (added by P7-A4).
**Tests:** `WorkerEvent::Failed { job_id, error: "CUDA out of memory", traceback: Some("Traceback...") }` roundtrips via `rmp_serde::to_vec_named()`. The msgpack dict contains `"_type": "Failed"`, `"job_id"`, `"error": "CUDA out of memory"`, and `"traceback": "Traceback..."`. Verifies the `Option<String>` field is preserved correctly.
**Mode:** both
**Inputs:** `WorkerEvent::Failed { job_id: Uuid::new_v4(), error: "CUDA out of memory".into(), traceback: Some("Traceback...".into()) }`.
**Expected output:** Roundtripped `WorkerEvent::Failed` equals original; job_id, error, and traceback preserved.
**Acceptance:** `cargo test -p anvilml-ipc --test roundtrip_tests test_failed_roundtrip` exits 0.

---

## test_cancelled_roundtrip (anvilml-ipc)

**File:** `crates/anvilml-ipc/tests/roundtrip_tests.rs`
**Context:** The `anvilml-ipc` crate has been compiled with `rmp-serde` dev-dependencies, and the `messages` module providing `WorkerEvent` with the `Cancelled` variant (added by P7-A4).
**Tests:** `WorkerEvent::Cancelled { job_id }` roundtrips via `rmp_serde::to_vec_named()`. The msgpack dict contains `"_type": "Cancelled"` and `"job_id"` (UUID string). Verifies the single `job_id` field is preserved correctly across serialisation.
**Mode:** both
**Inputs:** `WorkerEvent::Cancelled { job_id: Uuid::new_v4() }`.
**Expected output:** Roundtripped `WorkerEvent::Cancelled` equals original; job_id preserved.
**Acceptance:** `cargo test -p anvilml-ipc --test roundtrip_tests test_cancelled_roundtrip` exits 0.

---

## test_bind_returns_nonzero_port (anvilml-ipc)

**File:** `crates/anvilml-ipc/tests/roundtrip_tests.rs`
**Context:** The `anvilml-ipc` crate has been compiled with the `zeromq` dependency (v0.6.0, features `tokio-runtime` and `all-transport`), and the `transport` module providing `RouterTransport::bind()`.
**Tests:** `RouterTransport::bind()` binds a ZeroMQ ROUTER socket on `tcp://127.0.0.1:0` (OS-assigned port), splits the socket into independent send/recv halves, and returns a `RouterTransport` with the assigned port. The test asserts `port > 0`.
**Mode:** both
**Inputs:** None — `bind()` uses the `tcp://127.0.0.1:0` address which requests an OS-assigned port.
**Expected output:** `RouterTransport` with `port > 0`.
**Acceptance:** `cargo test -p anvilml-ipc --test roundtrip_tests test_bind_returns_nonzero_port` exits 0.

---

## test_two_binds_get_different_ports (anvilml-ipc)

**File:** `crates/anvilml-ipc/tests/roundtrip_tests.rs`
**Context:** The `anvilml-ipc` crate has been compiled with the `zeromq` dependency (v0.6.0, features `tokio-runtime` and `all-transport`), and the `transport` module providing `RouterTransport::bind()`.
**Tests:** Two `RouterTransport::bind()` calls are spawned concurrently via `tokio::task::spawn`. The test asserts that their `port` fields differ — proving the OS assigns distinct ports for concurrent binds.
**Mode:** both
**Inputs:** None — both binds use `tcp://127.0.0.1:0`.
**Expected output:** Two `RouterTransport` instances with different `port` values.
**Acceptance:** `cargo test -p anvilml-ipc --test roundtrip_tests test_two_binds_get_different_ports` exits 0.

---

## test_bind_port_is_listening (anvilml-ipc)

**File:** `crates/anvilml-ipc/tests/roundtrip_tests.rs`
**Context:** The `anvilml-ipc` crate has been compiled with the `zeromq` dependency (v0.6.0, features `tokio-runtime` and `all-transport`), and the `transport` module providing `RouterTransport::bind()`.
**Tests:** `RouterTransport::bind()` is called, then a `TcpStream::connect` is attempted to `127.0.0.1:{port}`. A successful connection proves the port is actually listening. The bind is wrapped in a 2-second timeout to prevent indefinite hangs.
**Mode:** both
**Inputs:** None — the transport binds on `tcp://127.0.0.1:0` and the test connects to the returned port.
**Expected output:** `TcpStream::connect` succeeds, confirming the port is listening.
**Acceptance:** `cargo test -p anvilml-ipc --test roundtrip_tests test_bind_port_is_listening` exits 0.

---

## test_send_recv_roundtrip (anvilml-ipc)

**File:** `crates/anvilml-ipc/tests/roundtrip_tests.rs`
**Context:** The `anvilml-ipc` crate has been compiled with `zeromq` (v0.6.0, features `tokio-runtime` and `all-transport`), `bytes` (v1.12), `rmp-serde` (v1.3.1), and `tracing` (v0.1) dependencies. The `transport` module provides `RouterTransport::send()` and `RouterTransport::recv()`.
**Tests:** A `WorkerMessage::Ping { seq: 42 }` is sent via `send("gpu:0", &msg)`, and the matching `WorkerEvent::Pong { seq: 42 }` is received via `recv()`. A background DEALER socket connects to the router with identity `"gpu:0"`, sends a Pong event back, and the test verifies the identity and event content match.
**Mode:** both
**Inputs:** `send()` called with `worker_id = "gpu:0"` and `WorkerMessage::Ping { seq: 42 }`.
**Expected output:** `recv()` returns `("gpu:0", WorkerEvent::Pong { seq: 42 })`.
**Acceptance:** `cargo test -p anvilml-ipc --test roundtrip_tests test_send_recv_roundtrip` exits 0.

---

## test_concurrent_send_recv_does_not_block (anvilml-ipc)

**File:** `crates/anvilml-ipc/tests/roundtrip_tests.rs`
**Context:** The `anvilml-ipc` crate has been compiled with `zeromq` (v0.6.0, features `tokio-runtime` and `all-transport`), `bytes`, `rmp-serde`, and `tracing` dependencies. The `transport` module provides `RouterTransport::send()` and `RouterTransport::recv()`.
**Tests:** `recv()` is spawned in a background task (blocks waiting for a message), then `send()` is called from the main task. The send must complete within a 3-second timeout without waiting for recv to unblock — proving the sender and receiver locks are independent (the v3 shutdown deadlock regression test).
**Mode:** both
**Inputs:** `send()` called with `worker_id = "gpu:0"` and `WorkerMessage::Ping { seq: 99 }` while `recv()` is blocked.
**Expected output:** `send()` completes within 3 seconds; `recv()` is aborted cleanly.
**Acceptance:** `cargo test -p anvilml-ipc --test roundtrip_tests test_concurrent_send_recv_does_not_block` exits 0.

---

## test_send_ping_then_recv_pong (anvilml-ipc)

**File:** `crates/anvilml-ipc/tests/roundtrip_tests.rs`
**Context:** The `anvilml-ipc` crate has been compiled with `zeromq` (v0.6.0, features `tokio-runtime` and `all-transport`), `bytes`, `rmp-serde`, and `tracing` dependencies. The `transport` module provides `RouterTransport::send()` and `RouterTransport::recv()`.
**Tests:** A `WorkerMessage::Ping { seq: 1 }` is sent via `send("worker-1", &msg)`, and the corresponding `WorkerEvent::Pong { seq: 1 }` is received via `recv()`. A background DEALER socket with identity `"worker-1"` sends the Pong back. The test verifies the identity is `"worker-1"` and the seq field is preserved.
**Mode:** both
**Inputs:** `send()` called with `worker_id = "worker-1"` and `WorkerMessage::Ping { seq: 1 }`.
**Expected output:** `recv()` returns `("worker-1", WorkerEvent::Pong { seq: 1 })`.
**Acceptance:** `cargo test -p anvilml-ipc --test roundtrip_tests test_send_ping_then_recv_pong` exits 0.

---

## test_send_execute_message_roundtrip (anvilml-ipc)

**File:** `crates/anvilml-ipc/tests/roundtrip_tests.rs`
**Context:** The `anvilml-ipc` crate has been compiled with `zeromq` (v0.6.0, features `tokio-runtime` and `all-transport`), `bytes`, `rmp-serde`, `tracing`, `serde_json`, and `uuid` dependencies. The `transport` module provides `RouterTransport::send()` and `RouterTransport::recv()`.
**Tests:** A complex `WorkerMessage::Execute` with all four fields (`job_id: Uuid`, `graph: serde_json::Value`, `settings: JobSettings`, `device_index: u32`) is sent via `send("gpu:2", &msg)`, and the corresponding `WorkerEvent::Pong { seq: 7 }` is received. The test verifies the identity is `"gpu:2"` and the seq field is preserved, exercising the most complex message variant through the wire protocol.
**Mode:** both
**Inputs:** `send()` called with `worker_id = "gpu:2"` and a full `WorkerMessage::Execute` with UUID job_id, empty graph, `JobSettings { device_preference: None }`, and `device_index: 2`.
**Expected output:** `recv()` returns `("gpu:2", WorkerEvent::Pong { seq: 7 })`.
**Acceptance:** `cargo test -p anvilml-ipc --test roundtrip_tests test_send_execute_message_roundtrip` exits 0.

---

## test_recv_malformed_frames_returns_error (anvilml-ipc)

**File:** `crates/anvilml-ipc/tests/roundtrip_tests.rs`
**Context:** The `anvilml-ipc` crate has been compiled with `zeromq` (v0.6.0, features `tokio-runtime` and `all-transport`), `bytes`, `rmp-serde`, and `tracing` dependencies. The `transport` module provides `RouterTransport::send()` and `RouterTransport::recv()`.
**Tests:** A DEALER socket sends a single-frame message (no delimiter) to the router. The router receives only 2 frames (identity + payload) instead of the expected 3. The test verifies that `recv()` returns `IpcError::RecvFailed` with an error message containing "expected 3 frames".
**Mode:** both
**Inputs:** A 1-frame message sent from DEALER (router sees 2 frames: identity + payload).
**Expected output:** `recv()` returns `Err(IpcError::RecvFailed("expected 3 frames, got 2"))`.
**Acceptance:** `cargo test -p anvilml-ipc --test roundtrip_tests test_recv_malformed_frames_returns_error` exits 0.

---

## test_1000_roundtrips (anvilml-ipc)

**File:** `crates/anvilml-ipc/tests/stress_test.rs`
**Context:** The `anvilml-ipc` crate has been compiled with `zeromq` (v0.6.0, features `tokio-runtime` and `all-transport`), `bytes`, `rmp-serde`, `tokio` (with `macros` and `rt-multi-thread` features), and `tracing` dependencies. `RouterTransport::bind()` creates a ROUTER socket on a loopback TCP port; `WorkerMessage::Ping { seq }` and `WorkerEvent::Pong { seq }` are msgpack-serialisable via `rmp_serde::to_vec_named` / `from_slice`.
**Tests:** Binds a `RouterTransport`, spawns a simulated DEALER worker with peer identity `"stress-worker"`, and performs 1000 sequential Ping→Pong round trips over loopback TCP. Verifies: (1) all 1000 messages are received (zero loss), (2) sequence numbers arrive in ascending order 1..=1000 (zero reordering), (3) worker identity matches `"stress-worker"` on every round trip, (4) every message completes within the 5-second per-message timeout. The simulated DEALER echoes each Ping back as a Pong with the same sequence number, exercising the full msgpack serialisation/deserialisation path 1000 times.
**Mode:** both
**Inputs:** `RouterTransport::bind()` on loopback TCP; simulated DEALER with identity `"stress-worker"` sending `WorkerEvent::Pong { seq: 1..=1000 }`; main task sending `WorkerMessage::Ping { seq: 1..=1000 }`.
**Expected output:** All 1000 round trips complete with matching seq values; zero assertion failures; background DEALER task exits cleanly.
**Acceptance:** `cargo test -p anvilml-ipc --test stress_test test_1000_roundtrips` exits 0.

---

## test_build_all_vars_present (anvilml-worker)

**File:** `crates/anvilml-worker/tests/env_tests.rs`
**Context:** The `anvilml-worker` crate has been compiled with `anvilml-core` providing `DeviceType`. `WorkerEnv::build()` is called with `ipc_port=5555, worker_id="0", device_index=1, device_type=Cuda, mock=false, log_level=debug, max_ipc_payload_mib=512`.
**Tests:** All six builder-set env vars are present with correct string values: `ANVILML_IPC_PORT="5555"`, `ANVILML_WORKER_ID="0"`, `ANVILML_DEVICE_INDEX="1"`, `ANVILML_DEVICE_TYPE="cuda"`, `ANVILML_LOG_LEVEL="debug"`, `ANVILML_MAX_IPC_PAYLOAD_MIB="512"`.
**Mode:** both
**Inputs:** `WorkerEnv::build(5555, "0", 1, DeviceType::Cuda, false, "debug", 512)`.
**Expected output:** `HashMap` contains exactly 6 entries with all correct key-value pairs.
**Acceptance:** `cargo test -p anvilml-worker --test env_tests -- test_build_all_vars_present` exits 0.

---

## test_worker_mock_absent_when_false (anvilml-worker)

**File:** `crates/anvilml-worker/tests/env_tests.rs`
**Context:** The `anvilml-worker` crate has been compiled with `anvilml-core` providing `DeviceType`. `WorkerEnv::build()` is called with `mock=false`.
**Tests:** `ANVILML_WORKER_MOCK` key is absent from the map when `mock=false` — its absence signals real-mode hardware execution to the Python worker.
**Mode:** both
**Inputs:** `WorkerEnv::build(5555, "0", 0, DeviceType::Cpu, false, "info", 256)`.
**Expected output:** `"ANVILML_WORKER_MOCK"` not in map keys.
**Acceptance:** `cargo test -p anvilml-worker --test env_tests -- test_worker_mock_absent_when_false` exits 0.

---

## test_worker_mock_present_when_true (anvilml-worker)

**File:** `crates/anvilml-worker/tests/env_tests.rs`
**Context:** The `anvilml-worker` crate has been compiled with `anvilml-core` providing `DeviceType`. `WorkerEnv::build()` is called with `mock=true`.
**Tests:** `ANVILML_WORKER_MOCK="1"` when `mock=true` — this is the primary mechanism by which the supervisor tells the Python worker to use mock hardware instead of real torch-level probing.
**Mode:** both
**Inputs:** `WorkerEnv::build(5555, "0", 0, DeviceType::Cpu, true, "info", 256)`.
**Expected output:** `"ANVILML_WORKER_MOCK"` maps to `"1"`.
**Acceptance:** `cargo test -p anvilml-worker --test env_tests -- test_worker_mock_present_when_true` exits 0.

---

## test_device_type_cuda (anvilml-worker)

**File:** `crates/anvilml-worker/tests/env_tests.rs`
**Context:** The `anvilml-worker` crate has been compiled with `anvilml-core` providing `DeviceType`. `WorkerEnv::build()` is called with `device_type=Cuda`.
**Tests:** `DeviceType::Cuda` maps to `"cuda"` in `ANVILML_DEVICE_TYPE`.
**Mode:** both
**Inputs:** `WorkerEnv::build(5555, "0", 0, DeviceType::Cuda, false, "info", 256)`.
**Expected output:** `"ANVILML_DEVICE_TYPE"` maps to `"cuda"`.
**Acceptance:** `cargo test -p anvilml-worker --test env_tests -- test_device_type_cuda` exits 0.

---

## test_device_type_rocm (anvilml-worker)

**File:** `crates/anvilml-worker/tests/env_tests.rs`
**Context:** The `anvilml-worker` crate has been compiled with `anvilml-core` providing `DeviceType`. `WorkerEnv::build()` is called with `device_type=Rocm`.
**Tests:** `DeviceType::Rocm` maps to `"rocm"` in `ANVILML_DEVICE_TYPE`.
**Mode:** both
**Inputs:** `WorkerEnv::build(5555, "0", 0, DeviceType::Rocm, false, "info", 256)`.
**Expected output:** `"ANVILML_DEVICE_TYPE"` maps to `"rocm"`.
**Acceptance:** `cargo test -p anvilml-worker --test env_tests -- test_device_type_rocm` exits 0.

---

## test_device_type_cpu (anvilml-worker)

**File:** `crates/anvilml-worker/tests/env_tests.rs`
**Context:** The `anvilml-worker` crate has been compiled with `anvilml-core` providing `DeviceType`. `WorkerEnv::build()` is called with `device_type=Cpu`.
**Tests:** `DeviceType::Cpu` maps to `"cpu"` in `ANVILML_DEVICE_TYPE`.
**Mode:** both
**Inputs:** `WorkerEnv::build(5555, "0", 0, DeviceType::Cpu, false, "info", 256)`.
**Expected output:** `"ANVILML_DEVICE_TYPE"` maps to `"cpu"`.
**Acceptance:** `cargo test -p anvilml-worker --test env_tests -- test_device_type_cpu` exits 0.

---

## test_force_worker_mock_absent (anvilml-worker)

**File:** `crates/anvilml-worker/tests/env_tests.rs`
**Context:** The `anvilml-worker` crate has been compiled with `anvilml-core` providing `DeviceType`. `WorkerEnv::build()` is called with all parameters including `mock=true`.
**Tests:** `ANVILML_FORCE_WORKER_MOCK` is never set by the builder, even when `mock=true`. That variable is handled separately by the caller (the supervisor) as an independent runtime trigger.
**Mode:** both
**Inputs:** `WorkerEnv::build(5555, "1", 2, DeviceType::Rocm, true, "trace", 1024)`.
**Expected output:** `"ANVILML_FORCE_WORKER_MOCK"` not in map keys.
**Acceptance:** `cargo test -p anvilml-worker --test env_tests -- test_force_worker_mock_absent` exits 0.

---
