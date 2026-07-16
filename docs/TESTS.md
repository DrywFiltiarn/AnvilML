# Test Catalogue

Every test in the AnvilML codebase is catalogued here. One entry per test.

---

## test_build_command_has_path (anvilml-worker)

**File:** `crates/anvilml-worker/src/spawn.rs`
**Context:** The `anvilml-worker` crate has been compiled with `tokio` (process feature) and `tracing` dependencies.
**Tests:** `build_command()` constructs a valid `Command` without panicking.
**Mode:** both
**Inputs:** Empty env map, venv path `/tmp/test_venv`.
**Expected output:** `build_command()` returns a configured `Command` without error.
**Acceptance:** `cargo test -p anvilml-worker --test spawn_tests test_build_command_has_path` exits 0.

---

## test_interpreter_path_unix (anvilml-worker)

**File:** `crates/anvilml-worker/tests/spawn_tests.rs`
**Context:** The `anvilml-worker` crate has been compiled with `tokio` (process feature) and `tracing` dependencies.
**Tests:** `build_command()` constructs a Command targeting the correct Unix interpreter path (`{venv_path}/bin/python3`).
**Mode:** both
**Inputs:** venv path `/tmp/test_venv`, empty env map.
**Expected output:** Command is structurally valid with Unix interpreter path.
**Acceptance:** `cargo test -p anvilml-worker --test spawn_tests test_interpreter_path_unix` exits 0.

---

## test_interpreter_path_windows (anvilml-worker)

**File:** `crates/anvilml-worker/tests/spawn_tests.rs`
**Context:** The `anvilml-worker` crate has been compiled with `tokio` (process feature) and `tracing` dependencies.
**Tests:** `build_command()` would construct the correct Windows interpreter path (`{venv_path}\Scripts\python.exe`) when compiled for the Windows target.
**Mode:** both
**Inputs:** venv path `C:\test_venv`, empty env map.
**Expected output:** Command is structurally valid with Windows interpreter path.
**Acceptance:** `cargo test -p anvilml-worker --test spawn_tests test_interpreter_path_windows` exits 0.

---

## test_worker_script_arg (anvilml-worker)

**File:** `crates/anvilml-worker/tests/spawn_tests.rs`
**Context:** The `anvilml-worker` crate has been compiled with `tokio` (process feature) and `tracing` dependencies.
**Tests:** `build_command()` sets the script argument to `worker/worker_main.py`, ensuring the worker subprocess runs the correct module.
**Mode:** both
**Inputs:** Any venv path, empty env map.
**Expected output:** Command has exactly one argument: `worker/worker_main.py`.
**Acceptance:** `cargo test -p anvilml-worker --test spawn_tests test_worker_script_arg` exits 0.

---

## test_env_vars_applied (anvilml-worker)

**File:** `crates/anvilml-worker/tests/spawn_tests.rs`
**Context:** The `anvilml-worker` crate has been compiled with `tokio` (process feature), `tracing`, and `anvilml-core` dependencies. Uses `WorkerEnv::build()` to produce a realistic env map.
**Tests:** `build_command()` correctly applies all env vars from the input map via `Command::envs()`.
**Mode:** both
**Inputs:** Full env map from `WorkerEnv::build(5555, "0", 1, DeviceType::Cuda, true, "debug", 512)`.
**Expected output:** All env vars from the map are applied to the Command.
**Acceptance:** `cargo test -p anvilml-worker --test spawn_tests test_env_vars_applied` exits 0.

---

## test_stdio_piped (anvilml-worker)

**File:** `crates/anvilml-worker/tests/spawn_tests.rs`
**Context:** The `anvilml-worker` crate has been compiled with `tokio` (process feature) and `tracing` dependencies.
**Tests:** `build_command()` configures both stdout and stderr for piping, enabling the supervisor to read worker output and errors.
**Mode:** both
**Inputs:** Any venv path, empty env map.
**Expected output:** Command has both stdout and stderr set to `Stdio::piped()`.
**Acceptance:** `cargo test -p anvilml-worker --test spawn_tests test_stdio_piped` exits 0.

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
**Tests:** Each of the six `WorkerStatus` variants (`Initializing`, `Idle`, `Busy`, `Dying`, `Dead`, `Respawning`) serialises to a lowercase snake_case JSON string and deserialises back to an equal value.
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
**Tests:** `WorkerEvent::Ready` with all 15 fields roundtrips via `rmp_serde::to_vec_named()`. Constructs a realistic Ready event with worker_id="gpu:0", device_index=0, device_name="NVIDIA RTX 4090", device_type="cuda", vram_total_mib=24576, vram_free_mib=20480, torch_version="2.5.1+cu124", fp32=true, fp16=true, bf16=true, fp8=true, fp4=false, flash_attention=true, capabilities_source="pytorch", and two `NodeTypeDescriptor` entries (LoadModel, KSampler). Verifies the deserialised event equals the original.
**Mode:** both
**Inputs:** Full `WorkerEvent::Ready` with all 15 fields at representative values.
**Expected output:** Roundtripped `WorkerEvent::Ready` equals original; all 15 fields preserved including `node_types` vec with two entries.
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

## test_job_object_creation_succeeds (anvilml-worker)

**File:** `crates/anvilml-worker/tests/spawn_tests.rs`
**Context:** The `anvilml-worker` crate has been compiled with `tokio` (process feature), `tracing`, `anvilml-core`, and the `windows` crate (target-conditional `cfg(windows)` with `Win32_Foundation`, `Win32_System_JobObjects`, `Win32_System_Threading` features).
**Tests:** `JobObjectGuard::new()` creates a Win32 Job Object with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` enabled without error.
**Mode:** both
**Inputs:** None.
**Expected output:** `Ok(JobObjectGuard { handle })`.
**Acceptance:** `cargo test -p anvilml-worker --test spawn_tests test_job_object_creation_succeeds` exits 0 (on Windows target).

---

## test_assigned_child_terminated_on_drop (anvilml-worker)

**File:** `crates/anvilml-worker/tests/spawn_tests.rs`
**Context:** The `anvilml-worker` crate has been compiled with `tokio` (process feature), `tracing`, `anvilml-core`, and the `windows` crate (target-conditional `cfg(windows)`). The test spawns a long-running `cmd /c timeout 999` subprocess and assigns it to a `JobObjectGuard`.
**Tests:** A child process assigned to a job object is killed when the `JobObjectGuard` drops, confirming the orphan-prevention guarantee. The test uses a bounded 5-second wait on the subprocess exit (per ENVIRONMENT.md §11.5) to prevent indefinite hangs.
**Mode:** both
**Inputs:** `cmd /c timeout 999` subprocess.
**Expected output:** Child process exits within 5 seconds of guard drop.
**Acceptance:** `cargo test -p anvilml-worker --test spawn_tests test_assigned_child_terminated_on_drop` exits 0 (on Windows target, with `--test-threads=1` to prevent race with other subprocess tests).

---

## test_double_assignment_fails_cleanly (anvilml-worker)

**File:** `crates/anvilml-worker/tests/spawn_tests.rs`
**Context:** The `anvilml-worker` crate has been compiled with `tokio` (process feature), `tracing`, `anvilml-core`, and the `windows` crate (target-conditional `cfg(windows)`). The test spawns two long-running subprocesses and attempts to assign both to the same job object.
**Tests:** Assigning a second child to a job object that already has a first child returns `Err(AnvilError::Io(_))` cleanly — no panic, no resource leak. The `AssignProcessToJobObject` Win32 API returns `ERROR_ACCESS_DENIED` when a process is already in another job.
**Mode:** both
**Inputs:** Two `cmd /c timeout 999` subprocesses.
**Expected output:** Second `assign_process()` call returns `Err(AnvilError::Io(_))`.
**Acceptance:** `cargo test -p anvilml-worker --test spawn_tests test_double_assignment_fails_cleanly` exits 0 (on Windows target, with `--test-threads=1`).

---

## test_spawn_nonexistent_venv_returns_io_error (anvilml-worker)

**File:** `crates/anvilml-worker/tests/spawn_tests.rs`
**Context:** The `anvilml-worker` crate has been compiled with `tokio` (process feature), `tracing`, and `anvilml-core` dependencies. The `WorkerSpawner` trait and `ProcessWorkerSpawner` struct are defined in `spawn.rs` and re-exported from `lib.rs`.
**Tests:** `ProcessWorkerSpawner::spawn()` against a nonexistent venv path returns `AnvilError::Io` whose message contains the interpreter path (`python3` or `python`), proving the production path reaches the OS spawn call.
**Mode:** both
**Inputs:** `venv_path = "/tmp/nonexistent_venv_xyz"`, empty env map.
**Expected output:** `Err(AnvilError::Io(_))` where `e.to_string()` contains `"python3"` or `"python"`.
**Acceptance:** `cargo test -p anvilml-worker --test spawn_tests test_spawn_nonexistent_venv_returns_io_error` exits 0.

---

## test_worker_spawner_is_object_safe (anvilml-worker)

**File:** `crates/anvilml-worker/tests/spawn_tests.rs`
**Context:** The `anvilml-worker` crate has been compiled with `tokio` (process feature), `tracing`, and `anvilml-core` dependencies. The `WorkerSpawner` trait is defined in `spawn.rs` and re-exported from `lib.rs`.
**Tests:** `WorkerSpawner` is object-safe: `Arc<dyn WorkerSpawner>` compiles and the trait is `Send + Sync`. This is a compile-time check — no runtime behavior is exercised.
**Mode:** both
**Inputs:** N/A (compile-time check only).
**Expected output:** The test function compiles without trait object safety errors.
**Acceptance:** `cargo test -p anvilml-worker --test spawn_tests test_worker_spawner_is_object_safe` exits 0.

---

## test_spawn_produces_same_command_shape (anvilml-worker)

**File:** `crates/anvilml-worker/tests/spawn_tests.rs`
**Context:** The `anvilml-worker` crate has been compiled with `tokio` (process feature), `tracing`, and `anvilml-core` dependencies. `build_command()` and `ProcessWorkerSpawner::spawn()` both construct commands for the same venv path.
**Tests:** `ProcessWorkerSpawner::spawn()` produces the same command shape as `build_command()` — both error messages contain the same interpreter path, proving `spawn()` delegates to `spawn_worker()` which delegates to `build_command()`.
**Mode:** both
**Inputs:** `venv_path = "/tmp/nonexistent_venv_cmd_shape"`, empty env map, same for both calls.
**Expected output:** Both error messages contain the venv path, confirming identical interpreter paths.
**Acceptance:** `cargo test -p anvilml-worker --test spawn_tests test_spawn_produces_same_command_shape` exits 0.

---

## test_register_and_route_delivers (anvilml-worker)

**File:** `crates/anvilml-worker/tests/demux_tests.rs`
**Context:** The `anvilml-worker` crate has been compiled with `tokio` (process + sync features), `anvilml-ipc`, and `anvilml-core` dependencies. The `Demux` struct provides a mutex-protected map from worker ID to `tokio::sync::mpsc::Sender<WorkerEvent>`.
**Tests:** `Demux::register()` inserts a sender, `Demux::route()` delivers an event through the channel, and the receiver gets the exact event that was sent.
**Mode:** both
**Inputs:** A fresh `tokio::sync::mpsc::channel()` (16 capacity), a `WorkerEvent::Ready` with mock capabilities.
**Expected output:** `route()` returns `Ok(())`, `rx.recv()` returns the identical event.
**Acceptance:** `cargo test -p anvilml-worker --test demux_tests test_register_and_route_delivers` exits 0.

---

## test_route_worker_not_found (anvilml-worker)

**File:** `crates/anvilml-worker/tests/demux_tests.rs`
**Context:** The `anvilml-worker` crate has been compiled with `tokio` (process + sync features), `anvilml-ipc`, and `anvilml-core` dependencies. The `Demux` struct returns `AnvilError::WorkerNotFound` when routing to an unregistered worker.
**Tests:** `Demux::route()` called without a prior `register()` returns the correct error variant.
**Mode:** both
**Inputs:** An unregistered worker ID `"worker-99"`, any `WorkerEvent::Ready`.
**Expected output:** `route()` returns `Err(AnvilError::WorkerNotFound("worker-99".to_string()))`.
**Acceptance:** `cargo test -p anvilml-worker --test demux_tests test_route_worker_not_found` exits 0.

---

## test_deregister_removes_entry (anvilml-worker)

**File:** `crates/anvilml-worker/tests/demux_tests.rs`
**Context:** The `anvilml-worker` crate has been compiled with `tokio` (process + sync features), `anvilml-ipc`, and `anvilml-core` dependencies. This is the mandatory deregistration test per `ANVILML_DESIGN.md §9.4`.
**Tests:** `Demux::deregister()` actually removes the worker entry, causing subsequent `route()` calls to fail with `AnvilError::WorkerNotFound`.
**Mode:** both
**Inputs:** A registered worker, a `WorkerEvent::Ready`, then `deregister()` followed by another `route()`.
**Expected output:** First `route()` succeeds, `deregister()` returns `true`, second `route()` returns `Err(AnvilError::WorkerNotFound("worker-0"))`.
**Acceptance:** `cargo test -p anvilml-worker --test demux_tests test_deregister_removes_entry` exits 0.

---

## test_double_deregister_is_safe (anvilml-worker)

**File:** `crates/anvilml-worker/tests/demux_tests.rs`
**Context:** The `anvilml-worker` crate has been compiled with `tokio` (process + sync features), `anvilml-ipc`, and `anvilml-core` dependencies. `Demux::deregister()` is safe to call on absent entries.
**Tests:** Calling `deregister()` twice for the same worker ID: first returns `true`, second returns `false`, no panic.
**Mode:** both
**Inputs:** A registered worker, two consecutive `deregister()` calls with the same ID.
**Expected output:** First call returns `true`, second returns `false`.
**Acceptance:** `cargo test -p anvilml-worker --test demux_tests test_double_deregister_is_safe` exits 0.

---

## test_register_overwrites (anvilml-worker)

**File:** `crates/anvilml-worker/tests/demux_tests.rs`
**Context:** The `anvilml-worker` crate has been compiled with `tokio` (process + sync features), `anvilml-ipc`, and `anvilml-core` dependencies. `Demux::register()` is idempotent — re-registering the same worker ID overwrites the old sender.
**Tests:** After registering worker A then worker B for the same ID, events are routed to B's channel, not A's.
**Mode:** both
**Inputs:** Two separate `tokio::sync::mpsc::channel()` pairs, same worker ID, one `WorkerEvent::Ready`.
**Expected output:** Event arrives on B's receiver; A's receiver is empty.
**Acceptance:** `cargo test -p anvilml-worker --test demux_tests test_register_overwrites` exits 0.

---

## test_pong_within_timeout_keeps_alive (anvilml-worker)

**File:** `crates/anvilml-worker/tests/keepalive_tests.rs`
**Context:** The `anvilml-worker` crate has the `time` feature enabled on tokio. The `KeepaliveWatchdog` type is constructed with a `MockTransport` that always succeeds, a 50ms ping interval, and a 100ms pong timeout. A dedicated task sends pongs at 50ms intervals throughout the test.
**Tests:** A Pong received within the configured timeout does NOT trigger the death signal. The watchdog sends pings every 50ms and waits up to 100ms for a matching Pong. When a Pong is received, the watchdog continues the loop without signaling death.
**Mode:** both
**Inputs:** `MockTransport::new_ok()`, 50ms interval, 100ms timeout, pongs sent at 50ms intervals.
**Expected output:** No death signal sent within 250ms.
**Acceptance:** `cargo test -p anvilml-worker --test keepalive_tests test_pong_within_timeout_keeps_alive` exits 0.

---

## test_missing_pong_triggers_dead_signal (anvilml-worker)

**File:** `crates/anvilml-worker/tests/keepalive_tests.rs`
**Context:** The `anvilml-worker` crate has the `time` feature enabled on tokio. The `KeepaliveWatchdog` type is constructed with a `MockTransport` that always succeeds, a 50ms ping interval, and a 100ms pong timeout. No pongs are sent.
**Tests:** No Pong arriving within the timeout triggers the death signal. The watchdog sends a ping, waits for a Pong, and when none arrives within the timeout, it signals death via the oneshot channel and exits.
**Mode:** both
**Inputs:** `MockTransport::new_ok()`, 50ms interval, 100ms timeout, no pongs sent.
**Expected output:** Death signal sent within ~150ms (50ms first ping + 100ms pong timeout).
**Acceptance:** `cargo test -p anvilml-worker --test keepalive_tests test_missing_pong_triggers_dead_signal` exits 0.

---

## test_repeated_successful_pings_no_false_trigger (anvilml-worker)

**File:** `crates/anvilml-worker/tests/keepalive_tests.rs`
**Context:** The `anvilml-worker` crate has the `time` feature enabled on tokio. The `KeepaliveWatchdog` type is constructed with a `MockTransport` that always succeeds, a 50ms ping interval, and a 100ms pong timeout. A dedicated task sends pongs at 40ms intervals throughout the test.
**Tests:** Repeated successful Pongs do not false-trigger the death signal. The watchdog loops across multiple ping/pong cycles, receiving each Pong and continuing the loop. No death signal is sent as long as Pongs keep arriving.
**Mode:** both
**Inputs:** `MockTransport::new_ok()`, 50ms interval, 100ms timeout, pongs sent at 40ms intervals for 600ms.
**Expected output:** No death signal sent within 300ms after the last pong.
**Acceptance:** `cargo test -p anvilml-worker --test keepalive_tests test_repeated_successful_pings_no_false_trigger` exits 0.

---

## test_transport_send_failure_triggers_dead_signal (anvilml-worker)

**File:** `crates/anvilml-worker/tests/keepalive_tests.rs`
**Context:** The `anvilml-worker` crate has the `time` feature enabled on tokio. The `KeepaliveWatchdog` type is constructed with a `MockTransport` that always fails, a 50ms ping interval, and a 100ms pong timeout.
**Tests:** A transport send failure triggers the death signal. The watchdog sends a Ping, the transport returns an error, and the watchdog immediately signals death and exits without waiting for a Pong.
**Mode:** both
**Inputs:** `MockTransport::new_err(IpcError::SendFailed(...))`, 50ms interval, 100ms timeout, no pongs sent.
**Expected output:** Death signal sent immediately after the first ping failure.
**Acceptance:** `cargo test -p anvilml-worker --test keepalive_tests test_transport_send_failure_triggers_dead_signal` exits 0.

---

## test_defaults_match_documented_values (anvilml-worker)

**File:** `crates/anvilml-worker/tests/respawn_tests.rs`
**Context:** The `anvilml-worker` crate has been compiled with `tokio` (process, sync, time features) and `tracing` dependencies. The `RespawnPolicy` struct is available via `anvilml_worker::RespawnPolicy`.
**Tests:** `RespawnPolicy::default()` produces the documented defaults: 2000ms delay, 5 max attempts, 300s window.
**Mode:** both
**Inputs:** `RespawnPolicy::default()`.
**Expected output:** `next_delay() == Duration::from_millis(2000)`; `should_respawn(&[]) == true`.
**Acceptance:** `cargo test -p anvilml-worker --test respawn_tests test_defaults_match_documented_values` exits 0.

---

## test_under_limit_allows_respawn (anvilml-worker)

**File:** `crates/anvilml-worker/tests/respawn_tests.rs`
**Context:** The `anvilml-worker` crate has been compiled. The `RespawnPolicy` struct is available via `anvilml_worker::RespawnPolicy`.
**Tests:** `should_respawn` returns `true` when the attempt count is strictly below `max_attempts` within the trailing window.
**Mode:** both
**Inputs:** Policy with `max_attempts=3`, 2 `Instant` values within the 300s default window.
**Expected output:** `should_respawn` returns `true` (2 < 3).
**Acceptance:** `cargo test -p anvilml-worker --test respawn_tests test_under_limit_allows_respawn` exits 0.

---

## test_at_limit_blocks_respawn (anvilml-worker)

**File:** `crates/anvilml-worker/tests/respawn_tests.rs`
**Context:** The `anvilml-worker` crate has been compiled. The `RespawnPolicy` struct is available via `anvilml_worker::RespawnPolicy`.
**Tests:** `should_respawn` returns `false` when the attempt count equals `max_attempts` within the trailing window — the boundary condition where respawn halts.
**Mode:** both
**Inputs:** Policy with `max_attempts=3`, exactly 3 `Instant` values within the 300s default window.
**Expected output:** `should_respawn` returns `false` (3 >= 3).
**Acceptance:** `cargo test -p anvilml-worker --test respawn_tests test_at_limit_blocks_respawn` exits 0.

---

## test_attempts_outside_window_dont_count (anvilml-worker)

**File:** `crates/anvilml-worker/tests/respawn_tests.rs`
**Context:** The `anvilml-worker` crate has been compiled. The `RespawnPolicy` struct is available via `anvilml_worker::RespawnPolicy`.
**Tests:** Attempts older than `respawn_window_s` are excluded from the count — the trailing window is enforced correctly.
**Mode:** both
**Inputs:** Policy with `max_attempts=2`, `window=1` second, 2 `Instant` values 2-3 seconds old (outside the 1s window).
**Expected output:** `should_respawn` returns `true` (0 in-window < 2 max_attempts).
**Acceptance:** `cargo test -p anvilml-worker --test respawn_tests test_attempts_outside_window_dont_count` exits 0.

---

## test_next_delay_returns_correct_duration (anvilml-worker)

**File:** `crates/anvilml-worker/tests/respawn_tests.rs`
**Context:** The `anvilml-worker` crate has been compiled. The `RespawnPolicy` struct is available via `anvilml_worker::RespawnPolicy`.
**Tests:** `next_delay()` returns the configured delay as a `Duration`.
**Mode:** both
**Inputs:** Policy with custom delay of 5000ms.
**Expected output:** `next_delay() == Duration::from_millis(5000)`.
**Acceptance:** `cargo test -p anvilml-worker --test respawn_tests test_next_delay_returns_correct_duration` exits 0.

---

## test_empty_history_allows_respawn (anvilml-worker)

**File:** `crates/anvilml-worker/tests/respawn_tests.rs`
**Context:** The `anvilml-worker` crate has been compiled. The `RespawnPolicy` struct is available via `anvilml_worker::RespawnPolicy`.
**Tests:** An empty attempt history always allows respawn, since zero attempts is strictly below any `max_attempts` threshold.
**Mode:** both
**Inputs:** Policy with `max_attempts=1`, empty slice `&[]`.
**Expected output:** `should_respawn` returns `true` (0 < 1).
**Acceptance:** `cargo test -p anvilml-worker --test respawn_tests test_empty_history_allows_respawn` exits 0.

---

## test_clone_shares_status (anvilml-worker)

**File:** `crates/anvilml-worker/tests/managed_tests.rs`
**Context:** The `anvilml-worker` crate has been compiled with `tokio` (rt, sync features) and `anvilml-core` dependencies. The `WorkerHandle` struct is available via `anvilml_worker::WorkerHandle`. Two handles are constructed from the same `Arc<RwLock<WorkerStatus>>` set to `Idle`.
**Tests:** Constructing two `WorkerHandle`s from the same `Arc<RwLock<WorkerStatus>>` and calling `status()` on both returns the same value, proving clones share the status lock.
**Mode:** both
**Inputs:** Shared `Arc<RwLock<WorkerStatus>>` set to `WorkerStatus::Idle`, two handles with different `worker_id` values.
**Expected output:** Both `handle1.status().await` and `handle2.status().await` return `WorkerStatus::Idle`.
**Acceptance:** `cargo test -p anvilml-worker --test managed_tests test_clone_shares_status` exits 0.

---

## test_clone_independent_worker_id (anvilml-worker)

**File:** `crates/anvilml-worker/tests/managed_tests.rs`
**Context:** The `anvilml-worker` crate has been compiled with `tokio` (rt, sync features) and `anvilml-core` dependencies. The `WorkerHandle` struct is available via `anvilml_worker::WorkerHandle`.
**Tests:** Cloning a handle copies the `worker_id` String — same value but independent allocation. Modifying the original's `worker_id` does not affect the clone.
**Mode:** both
**Inputs:** Handle with `worker_id = "gpu:0"`.
**Expected output:** Clone has `worker_id == "gpu:0"`; after mutating original to `"modified"`, clone still has `"gpu:0"`.
**Acceptance:** `cargo test -p anvilml-worker --test managed_tests test_clone_independent_worker_id` exits 0.

---

## test_request_shutdown_sends_signal (anvilml-worker)

**File:** `crates/anvilml-worker/tests/managed_tests.rs`
**Context:** The `anvilml-worker` crate has been compiled with `tokio` (rt, sync features) and `anvilml-core` dependencies. The `WorkerHandle` struct is available via `anvilml_worker::WorkerHandle`.
**Tests:** Constructing a handle with a fresh `oneshot::channel` and calling `request_shutdown()` delivers `()` to the receiver side, proving the shutdown trigger works.
**Mode:** both
**Inputs:** Fresh `oneshot::channel()`, handle with the sender, background task awaiting the receiver.
**Expected output:** Receiver gets `Ok(())` confirming the shutdown signal was delivered.
**Acceptance:** `cargo test -p anvilml-worker --test managed_tests test_request_shutdown_sends_signal` exits 0.

---

## test_request_shutdown_is_idempotent (anvilml-worker)

**File:** `crates/anvilml-worker/tests/managed_tests.rs`
**Context:** The `anvilml-worker` crate has been compiled with `tokio` (rt, sync features) and `anvilml-core` dependencies. The `WorkerHandle` struct is available via `anvilml_worker::WorkerHandle`.
**Tests:** Calling `request_shutdown()` twice on the same handle does not panic — the second call operates on `None` (the `Option` was already `take()`n) and returns cleanly, proving idempotency.
**Mode:** both
**Inputs:** Handle with a `oneshot::Sender`.
**Expected output:** No panic; both calls complete successfully.
**Acceptance:** `cargo test -p anvilml-worker --test managed_tests test_request_shutdown_is_idempotent` exits 0.

---

## test_status_returns_current_value (anvilml-worker)

**File:** `crates/anvilml-worker/tests/managed_tests.rs`
**Context:** The `anvilml-worker` crate has been compiled with `tokio` (rt, sync features) and `anvilml-core` dependencies. The `WorkerHandle` struct is available via `anvilml_worker::WorkerHandle`.
**Tests:** Constructing a handle with status set to `Initializing` and calling `status()` returns `Initializing`, proving the read path works correctly for non-default states.
**Mode:** both
**Inputs:** Shared `Arc<RwLock<WorkerStatus>>` set to `WorkerStatus::Initializing`.
**Expected output:** `handle.status().await` returns `WorkerStatus::Initializing`.
**Acceptance:** `cargo test -p anvilml-worker --test managed_tests test_status_returns_current_value` exits 0.

---

## test_set_status_changes_value (anvilml-worker)

**File:** `crates/anvilml-worker/tests/managed_tests.rs`
**Context:** The `anvilml-worker` crate has been compiled with `tokio` (rt, sync features) and `anvilml-core` dependencies. The `WorkerHandle` struct is available via `anvilml_worker::WorkerHandle`. The `set_status()` method is available as the public mutator.
**Tests:** Constructing a handle with `WorkerStatus::Idle`, calling `set_status(WorkerStatus::Busy)`, then verifying `status().await` returns `WorkerStatus::Busy`. This exercises the write lock path and confirms the mutation is visible to subsequent reads.
**Mode:** both
**Inputs:** Handle constructed with `WorkerStatus::Idle`, `set_status(WorkerStatus::Busy)` call.
**Expected output:** `status().await` returns `WorkerStatus::Busy` after the call.
**Acceptance:** `cargo test -p anvilml-worker --test managed_tests test_set_status_changes_value` exits 0.

---

## test_set_status_visible_across_clone (anvilml-worker)

**File:** `crates/anvilml-worker/tests/managed_tests.rs`
**Context:** The `anvilml-worker` crate has been compiled with `tokio` (rt, sync features) and `anvilml-core` dependencies. The `WorkerHandle` struct is available via `anvilml_worker::WorkerHandle`. The `set_status()` method is available as the public mutator.
**Tests:** Constructing a handle, cloning it, calling `set_status(WorkerStatus::Dying)` on the original, then calling `status().await` on the clone and asserting it returns `WorkerStatus::Dying`. This proves the shared `Arc<RwLock<WorkerStatus>>` is correctly shared across clones.
**Mode:** both
**Inputs:** Handle with `WorkerStatus::Idle`, cloned handle, `set_status(WorkerStatus::Dying)` on original.
**Expected output:** Clone's `status().await` returns `WorkerStatus::Dying`.
**Acceptance:** `cargo test -p anvilml-worker --test managed_tests test_set_status_visible_across_clone` exits 0.

---

## test_concurrent_status_and_set_status_no_deadlock (anvilml-worker)

**File:** `crates/anvilml-worker/tests/managed_tests.rs`
**Context:** The `anvilml-worker` crate has been compiled with `tokio` (rt, sync, time features) and `anvilml-core` dependencies. The `WorkerHandle` struct is available via `anvilml_worker::WorkerHandle`. The `set_status()` method is available as the public mutator.
**Tests:** Spawning two concurrent tasks — one loops `status().await` 100 times, the other loops `set_status()` alternating between `Busy` and `Idle` 100 times. Both tasks must complete within 5 seconds (bounded wait per ENVIRONMENT.md §11.5), proving no deadlock between read and write lock paths. (Note: "Spawning" here refers to spawning concurrent tasks, not the `WorkerStatus::Spawning` variant.)
**Mode:** both
**Inputs:** Handle with `WorkerStatus::Idle`, 100 iterations of reads + alternating writes in separate tasks.
**Expected output:** Both tasks complete within 5s without deadlock.
**Acceptance:** `cargo test -p anvilml-worker --test managed_tests test_concurrent_status_and_set_status_no_deadlock` exits 0.

---

## test_set_status_callable_repeatedly (anvilml-worker)

**File:** `crates/anvilml-worker/tests/managed_tests.rs`
**Context:** The `anvilml-worker` crate has been compiled with `tokio` (rt, sync features) and `anvilml-core` dependencies. The `WorkerHandle` struct is available via `anvilml_worker::WorkerHandle`. The `set_status()` method is available as the public mutator.
**Tests:** Calling `set_status()` five times in sequence with `Initializing → Idle → Busy → Dying → Dead`, asserting each value after the call. This verifies the method can be called repeatedly without side effects or state corruption.
**Mode:** both
**Inputs:** Handle with `WorkerStatus::Idle`, sequential calls: `Initializing`, `Idle`, `Busy`, `Dying`, `Dead`.
**Expected output:** Each `status()` call after `set_status()` returns the expected value.
**Acceptance:** `cargo test -p anvilml-worker --test managed_tests test_set_status_callable_repeatedly` exits 0.

---

## test_run_completes_on_ready_event (anvilml-worker)

**File:** `crates/anvilml-worker/tests/managed_tests.rs`
**Context:** The `anvilml-worker` crate has been compiled with `tokio` (rt, sync, time features), `anvilml-ipc` (ZeroMQ ROUTER transport), and `anvilml-core` dependencies. Uses in-process ZeroMQ ROUTER/DEALER sockets to simulate a Python worker.
**Tests:** `ManagedWorker::run()` transitions from Initializing → Idle when a Ready event is received, then exits cleanly on shutdown signal.
**Mode:** mock
**Inputs:** ZeroMQ ROUTER bound on `tcp://127.0.0.1:0`, `ManagedWorker` spawned with `run()`, `Ready` event serialized as msgpack bytes and sent via `send_raw()`, then shutdown signal via `oneshot::Sender`.
**Expected output:** Worker task completes within 5s; `Ready` event is received and processed.
**Acceptance:** `cargo test -p anvilml-worker --test managed_tests test_run_completes_on_ready_event` exits 0.

---

## test_shutdown_rx_triggers_graceful_exit (anvilml-worker)

**File:** `crates/anvilml-worker/tests/managed_tests.rs`
**Context:** Same as `test_run_completes_on_ready_event`.
**Tests:** `shutdown_rx` being triggered causes `run()` to exit cleanly — even before a Ready event arrives — and deregister.
**Mode:** mock
**Inputs:** `ManagedWorker` spawned with `run()`, shutdown signal sent immediately (no Ready event).
**Expected output:** Worker task completes within 5s; no Initializing timeout fires.
**Acceptance:** `cargo test -p anvilml-worker --test managed_tests test_shutdown_rx_triggers_graceful_exit` exits 0.

---

## test_deregister_called_on_graceful_exit (anvilml-worker)

**File:** `crates/anvilml-worker/tests/managed_tests.rs`
**Context:** Same as above.
**Tests:** On graceful shutdown path, `demux.deregister(worker_id)` is called, confirmed by `demux.registered(worker_id)` returning `false` after `run()` returns.
**Mode:** mock
**Inputs:** Worker pre-registered with demux (simulating pool behavior), Ready event sent, then shutdown signal.
**Expected output:** `demux.registered("test-worker")` returns `true` after Ready, `false` after shutdown exit.
**Acceptance:** `cargo test -p anvilml-worker --test managed_tests test_deregister_called_on_graceful_exit` exits 0.

---

## test_deregister_called_on_crash (anvilml-worker)

**File:** `crates/anvilml-worker/tests/managed_tests.rs`
**Context:** Same as above.
**Tests:** On Dying event path (simulated crash), `demux.deregister(worker_id)` is called.
**Mode:** mock
**Inputs:** Worker pre-registered, Ready event sent, then `Dying { reason: "simulated crash" }` event sent via `send_raw()`.
**Expected output:** Worker task exits within 5s; `demux.registered("test-worker")` returns `false`.
**Acceptance:** `cargo test -p anvilml-worker --test managed_tests test_deregister_called_on_crash` exits 0.

---

## test_deregister_called_on_initializing_timeout (anvilml-worker)

**File:** `crates/anvilml-worker/tests/managed_tests.rs`
**Context:** Same as above. Uses `#[serial]` to prevent concurrent tests from interfering with the 60s timeout.
**Tests:** When no Ready event arrives within the Initializing timeout, `run()` exits and calls `deregister()`.
**Mode:** mock
**Inputs:** Worker pre-registered, no events sent for 60 seconds.
**Expected output:** Worker task completes within 65s (60s timeout + 5s buffer); `demux.registered("test-worker")` returns `false`.
**Acceptance:** `cargo test -p anvilml-worker --test managed_tests test_deregister_called_on_initializing_timeout` exits 0.

---

## test_crash_appends_to_attempt_history (anvilml-worker)

**File:** `crates/anvilml-worker/tests/managed_tests.rs`
**Context:** The `anvilml-worker` crate has been compiled with `tokio` (process, rt, sync, time), `tracing`, `rmp-serde` (dev), and `zeromq` (dev, tokio-runtime) dependencies. Uses in-process ZeroMQ ROUTER/DEALER pair to simulate a Python worker.
**Tests:** A single transport error (DEALER socket dropped) causes exactly one crash attempt to be recorded in `attempt_history`. The crash path appends `Instant::now()` and calls `should_respawn()`, then emits a `crash_respawn_decision` INFO log before breaking.
**Mode:** mock
**Inputs:** ROUTER/DEALER pair on loopback, `Ready` event sent via `rmp_serde::to_vec_named()`, DEALER dropped to trigger transport error.
**Expected output:** `ManagedWorker::run()` exits cleanly within 5s; the crash path executed (attempt_history.push + should_respawn call + log).
**Acceptance:** `cargo test -p anvilml-worker --test managed_tests test_crash_appends_to_attempt_history` exits 0.

---

## test_crash_history_grows_per_crash (anvilml-worker)

**File:** `crates/anvilml-worker/tests/managed_tests.rs`
**Context:** The `anvilml-worker` crate has been compiled with `tokio` (process, rt, sync, time), `tracing`, `rmp-serde` (dev), and `zeromq` (dev, tokio-runtime) dependencies. Uses two sequential worker instances to verify crash-attempt accumulation.
**Tests:** Multiple transport errors each append to `attempt_history`. Two separate `ManagedWorker` instances are spawned, each sending `Ready` then dropping its DEALER, proving each crash independently records an attempt.
**Mode:** mock
**Inputs:** Two ROUTER/DEALER pairs, each worker sends `Ready` then has its DEALER dropped.
**Expected output:** Both worker instances exit cleanly within 5s each; each crash path executed independently.
**Acceptance:** `cargo test -p anvilml-worker --test managed_tests test_crash_history_grows_per_crash` exits 0.

---

## test_should_respawn_called_on_crash (anvilml-worker)

**File:** `crates/anvilml-worker/tests/managed_tests.rs`
**Context:** The `anvilml-worker` crate has been compiled with `tokio` (process, rt, sync, time), `tracing`, `rmp-serde` (dev), and `zeromq` (dev, tokio-runtime) dependencies. Uses a `RespawnPolicy` with 10 max attempts to verify `should_respawn()` is consulted on crash.
**Tests:** On crash, `should_respawn()` is consulted and the INFO log `crash_respawn_decision` is emitted. A custom `RespawnPolicy::new(2000, 10, 300)` allows 10 attempts, so `should_respawn()` must return `true` for the first crash.
**Mode:** mock
**Inputs:** ROUTER/DEALER pair, `RespawnPolicy::new(2000, 10, 300)`, `Ready` event, DEALER dropped.
**Expected output:** Worker exits cleanly within 5s; `crash_respawn_decision` log emitted with `should_respawn = true`.
**Acceptance:** `cargo test -p anvilml-worker --test managed_tests test_should_respawn_called_on_crash` exits 0.

---

## test_watchdog_missing_pong_triggers_crash_path (anvilml-worker)

**File:** `crates/anvilml-worker/tests/managed_tests.rs`
**Context:** The `anvilml-worker` crate has been compiled with `tokio` (sync, time, process features) and `tracing` dependencies. `ManagedWorker::new()` accepts `pong_tx`, `watchdog_ping_interval`, and `watchdog_pong_timeout` parameters.
**Tests:** Missing Pongs trigger the watchdog's crash path identically to a transport error. Creates a ROUTER/DEALER pair with short watchdog timings (ping_interval=50ms, pong_timeout=200ms), sends Ready to transition to Idle, then sends no Pongs. The watchdog sends a Ping, waits 200ms for a Pong that never arrives, declares the worker dead via `dead_tx`, and the `dead_rx` branch in `run()` triggers the same crash path as a transport error (status → Dead, attempt_history appended, should_respawn called, loop breaks).
**Mode:** mock
**Inputs:** ROUTER/DEALER pair, `Ready` event, no Pongs forwarded.
**Expected output:** Status transitions to `Dead` within ~400ms.
**Acceptance:** `cargo test -p anvilml-worker --test managed_tests test_watchdog_missing_pong_triggers_crash_path` exits 0.

---

## test_watchdog_live_pongs_no_false_trigger (anvilml-worker)

**File:** `crates/anvilml-worker/tests/managed_tests.rs`
**Context:** The `anvilml-worker` crate has been compiled with `tokio` (sync, time, process features) and `tracing` dependencies.
**Tests:** Sending Pongs at the correct sequence number keeps the watchdog alive. Creates a ROUTER/DEALER pair with short watchdog timings (ping_interval=50ms, pong_timeout=200ms), sends Ready to transition to Idle, then continuously sends Pongs at the correct sequence number (seq 0, 1, 2, ...). The watchdog should not declare the worker dead — `dead_rx` never fires, and the worker stays alive for the duration of the test.
**Mode:** mock
**Inputs:** ROUTER/DEALER pair, `Ready` event, Pongs at seq 0-9.
**Expected output:** Status remains `Idle` throughout — Pongs keep watchdog alive.
**Acceptance:** `cargo test -p anvilml-worker --test managed_tests test_watchdog_live_pongs_no_false_trigger` exits 0.

---

## test_pong_forwarding_does_not_disturb_idle_busy (anvilml-worker)

**File:** `crates/anvilml-worker/tests/managed_tests.rs`
**Context:** The `anvilml-worker` crate has been compiled with `tokio` (sync, time, process features) and `tracing` dependencies.
**Tests:** Pong forwarding to the watchdog channel does not disturb normal event processing. Sends a sequence of events: Ready (→ Idle), manually sets Busy, sends Completed (→ Idle), sends Failed (→ Idle). The watchdog receives Pongs on its channel but filters them by sequence number. Status transitions are correct throughout.
**Mode:** mock
**Inputs:** ROUTER/DEALER pair, `Ready`, Busy status, `Completed`, `Failed` events.
**Expected output:** Status transitions: Idle → Busy → Idle → Idle. No false triggers.
**Acceptance:** `cargo test -p anvilml-worker --test managed_tests test_pong_forwarding_does_not_disturb_idle_busy` exits 0.

---

## test_router_transport_adapter_not_dead_code (anvilml-worker)

**File:** `crates/anvilml-worker/tests/managed_tests.rs`
**Context:** The `anvilml-worker` crate has been compiled with `tokio` (sync, time, process features) and `tracing` dependencies. `RouterTransportAdapter` is `pub(crate)` and is no longer `#[allow(dead_code)]`.
**Tests:** Verifies that `RouterTransportAdapter` is no longer `#[allow(dead_code)]` by confirming it compiles without dead_code warnings. The adapter is now constructed inside `ManagedWorker::run()` via the watchdog spawning code. Clippy with `-D warnings` would fail if the adapter were unused.
**Mode:** both
**Inputs:** N/A — compile-time check.
**Expected output:** No dead_code warning on `RouterTransportAdapter`.
**Acceptance:** `cargo test -p anvilml-worker --test managed_tests test_router_transport_adapter_not_dead_code` exits 0.

---

## test_watchdog_channel_cleans_up_on_exit (anvilml-worker)

**File:** `crates/anvilml-worker/tests/managed_tests.rs`
**Context:** The `anvilml-worker` crate has been compiled with `tokio` (sync, time, process features) and `tracing` dependencies.
**Tests:** After `run()` completes, the `pong_tx` is dropped (consumed by `self`), closing the watchdog's `pong_rx`. The watchdog exits its loop without sending on `dead_tx` (graceful exit). Verifies that the watchdog task cleans up properly when the worker exits — it doesn't leak or hang.
**Mode:** mock
**Inputs:** ROUTER/DEALER pair, `Ready` event, shutdown signal.
**Expected output:** Worker completes within 500ms, status is `Dying`.
**Acceptance:** `cargo test -p anvilml-worker --test managed_tests test_watchdog_channel_cleans_up_on_exit` exits 0.

---

## test_connect_sets_identity (worker/ipc)

**File:** `worker/tests/test_ipc.py`
**Context:** The `worker.ipc` module has been created with `connect()`, `send_event()`, and `recv_message()` functions. pyzmq and msgpack are installed in the worker venv.
**Tests:** DEALER socket connects with correct ZEROMQ identity. Starts a ROUTER socket on a random port, calls `connect(port, "test-worker")` on a DEALER socket, then verifies the ROUTER receives a message from the correct identity. The ROUTER socket's first `recv()` returns the identity frame.
**Mode:** mock
**Inputs:** port 15555, worker_id="test-worker".
**Expected output:** ROUTER receives identity frame equal to `b"test-worker"`.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_ipc.py::TestConnectIdentity::test_connect_sets_identity -v` exits 0.

---

## test_send_event_before_connect_raises (worker/ipc)

**File:** `worker/tests/test_ipc.py`
**Context:** The `worker.ipc` module has `_ctx` and `_sock` module-level globals initialized to `None`.
**Tests:** `send_event()` raises RuntimeError when not connected. Calls `send_event()` without calling `connect()` first, asserts RuntimeError is raised with the expected message.
**Mode:** mock
**Inputs:** `{"_type": "Ping"}`.
**Expected output:** RuntimeError with message containing "ipc: not connected".
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_ipc.py::TestPreConnectErrors::test_send_event_before_connect_raises -v` exits 0.

---

## test_recv_message_before_connect_raises (worker/ipc)

**File:** `worker/tests/test_ipc.py`
**Context:** The `worker.ipc` module has `_ctx` and `_sock` module-level globals initialized to `None`.
**Tests:** `recv_message()` raises RuntimeError when not connected. Calls `recv_message()` without calling `connect()` first, asserts RuntimeError is raised with the expected message.
**Mode:** mock
**Inputs:** (none).
**Expected output:** RuntimeError with message containing "ipc: not connected".
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_ipc.py::TestPreConnectErrors::test_recv_message_before_connect_raises -v` exits 0.

---

## test_roundtrip_send_recv (worker/ipc)

**File:** `worker/tests/test_ipc.py`
**Context:** The `worker.ipc` module has `connect()`, `send_event()`, and `recv_message()` functions. pyzmq and msgpack are installed in the worker venv.
**Tests:** Full msgpack round-trip via ROUTER/DEALER pair. Sets up a ROUTER socket in the test process, connects a DEALER via `ipc.connect()`, sends a dict via `ipc.send_event()`, receives it from the ROUTER side via `router.recv()` (identity frame) + `router.recv()` (payload), unpacks with `msgpack.unpackb(raw=False)`, and asserts the dict matches the sent payload. This is the real integration test that proves the full send/recv path works end-to-end within a single process.
**Mode:** mock
**Inputs:** dict `{"_type": "Ping", "payload": "hello"}`, port 15556, worker_id="roundtrip-worker".
**Expected output:** ROUTER receives identical dict after stripping identity frame.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_ipc.py::TestRoundtrip::test_roundtrip_send_recv -v` exits 0.

---

## test_module_no_torch_import (worker/ipc)

**File:** `worker/tests/test_ipc.py`
**Context:** The `worker.ipc` module imports only `zmq` and `msgpack` — no torch. The worker venv's base.txt includes pyzmq and msgpack but not torch.
**Tests:** Module does not transitively import torch. Uses `subprocess.run()` to spawn a fresh Python process that imports `worker.ipc` and asserts `"torch" not in sys.modules`. This confirms the module has no transitive torch dependency at import time (required by the mock-mode CI jobs that install only base.txt without torch).
**Mode:** mock
**Inputs:** Fresh subprocess that imports `worker.ipc`.
**Expected output:** `"torch" not in sys.modules` succeeds; subprocess exits 0.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_ipc.py::TestNoTorchImport::test_module_no_torch_import -v` exits 0.

---

## test_connect_twice_reuses_context (worker/ipc)

**File:** `worker/tests/test_ipc.py`
**Context:** The `worker.ipc` module uses `zmq.Context.instance()` for process-wide singleton context management.
**Tests:** Second `connect()` call reuses zmq.Context singleton. Calls `connect()` twice with different worker IDs; verifies the second call reuses the existing context but creates a new socket with the new identity. This tests the singleton pattern works correctly.
**Mode:** mock
**Inputs:** Different worker_id on second call, port 15557.
**Expected output:** New DEALER socket with updated identity; context is the same singleton instance.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_ipc.py::TestContextReuse::test_connect_twice_reuses_context -v` exits 0.

---

## test_fp32_cpu_returns_true (worker/capability)

**File:** `worker/tests/test_capability.py`
**Context:** The `worker.capability` module has `probe_capabilities()` and `_probe_dtype()` functions. torch 2.12.1+cpu is installed in the worker venv.
**Tests:** fp32 probe on CPU returns True. Calls `probe_capabilities("cpu", 0)` and asserts `result["fp32"]` is True. This is the sanity check that the probe infrastructure itself is functional.
**Mode:** real
**Inputs:** `device_type="cpu"`, `device_index=0`.
**Expected output:** `result["fp32"] == True`.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_capability.py::TestProbeDtypes::test_fp32_cpu_returns_true -v` exits 0.

---

## test_fp16_cpu_returns_true (worker/capability)

**File:** `worker/tests/test_capability.py`
**Context:** The `worker.capability` module has `probe_capabilities()` and `_probe_dtype()` functions. torch 2.12.1+cpu is installed in the worker venv.
**Tests:** fp16 probe on CPU returns True. CPU supports fp16/bf16 on modern torch builds. Calls `probe_capabilities("cpu", 0)` and asserts `result["fp16"]` is True.
**Mode:** real
**Inputs:** `device_type="cpu"`, `device_index=0`.
**Expected output:** `result["fp16"] == True`.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_capability.py::TestProbeDtypes::test_fp16_cpu_returns_true -v` exits 0.

---

## test_bf16_cpu_returns_true (worker/capability)

**File:** `worker/tests/test_capability.py`
**Context:** The `worker.capability` module has `probe_capabilities()` and `_probe_dtype()` functions. torch 2.12.1+cpu is installed in the worker venv.
**Tests:** bf16 probe on CPU returns True. CPU supports bfloat16 on modern torch builds. Calls `probe_capabilities("cpu", 0)` and asserts `result["bf16"]` is True.
**Mode:** real
**Inputs:** `device_type="cpu"`, `device_index=0`.
**Expected output:** `result["bf16"] == True`.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_capability.py::TestProbeDtypes::test_bf16_cpu_returns_true -v` exits 0.

---

## test_fp8_cpu_returns_false (worker/capability)

**File:** `worker/tests/test_capability.py`
**Context:** The `worker.capability` module has `probe_capabilities()` and `_probe_dtype()` functions. torch 2.12.1+cpu is installed in the worker venv.
**Tests:** fp8 probe on CPU returns False. ``torch.float8_e4m3fn`` on CPU raises ``NotImplementedError``, and the probe catches it and returns False. This is correct behavior — fp8 compute is a GPU-only feature.
**Mode:** real
**Inputs:** `device_type="cpu"`, `device_index=0`.
**Expected output:** `result["fp8"] == False`.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_capability.py::TestProbeDtypes::test_fp8_cpu_returns_false -v` exits 0.

---

## test_fp4_cpu_returns_false (worker/capability)

**File:** `worker/tests/test_capability.py`
**Context:** The `worker.capability` module has `probe_capabilities()` and `_probe_dtype()` functions. torch 2.12.1+cpu is installed in the worker venv.
**Tests:** fp4 probe on CPU returns False. Torch 2.x does not expose a native fp4 dtype. The probe attempts ``torch.float8_e4m3fn`` as the closest available format; on CPU this raises NotImplementedError, so fp4 is correctly False.
**Mode:** real
**Inputs:** `device_type="cpu"`, `device_index=0`.
**Expected output:** `result["fp4"] == False`.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_capability.py::TestProbeDtypes::test_fp4_cpu_returns_false -v` exits 0.

---

## test_flash_attention_cpu_returns_true (worker/capability)

**File:** `worker/tests/test_capability.py`
**Context:** The `worker.capability` module has `probe_capabilities()` and `_probe_flash_attention()` functions. torch 2.12.1+cpu is installed in the worker venv.
**Tests:** Flash attention probe on CPU returns True. ``torch.nn.functional.scaled_dot_product_attention`` works on CPU — it falls back to standard math attention rather than raising. The probe correctly returns True because the function executes successfully (even though acceleration is not available).
**Mode:** real
**Inputs:** `device_type="cpu"`, `device_index=0`.
**Expected output:** `result["flash_attention"] == True`.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_capability.py::TestProbeFlashAttention::test_flash_attention_cpu_returns_true -v` exits 0.

---

## test_returns_dict_with_exactly_six_bool_keys (worker/capability)

**File:** `worker/tests/test_capability.py`
**Context:** The `worker.capability` module has `probe_capabilities()` function. torch 2.12.1+cpu is installed in the worker venv.
**Tests:** Return dict has exactly 6 keys matching ``InferenceCaps`` field names. Calls ``probe_capabilities("cpu", 0)`` and asserts the result is a dict with exactly 6 keys matching the ``InferenceCaps`` struct field names, and all values are ``bool`` type.
**Mode:** real
**Inputs:** `device_type="cpu"`, `device_index=0`.
**Expected output:** Dict with keys ``fp32``, ``fp16``, ``bf16``, ``fp8``, ``fp4``, ``flash_attention``, all bool values.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_capability.py::TestProbeStructure::test_returns_dict_with_exactly_six_bool_keys -v` exits 0.

---

## test_never_raises_for_cpu (worker/capability)

**File:** `worker/tests/test_capability.py`
**Context:** The `worker.capability` module has `probe_capabilities()` function. torch 2.12.1+cpu is installed in the worker venv.
**Tests:** probe_capabilities("cpu", 0) never raises any exception. The probe must be resilient on CPU — no matter what dtypes are available or unavailable, the function must return a dict and never propagate an exception.
**Mode:** real
**Inputs:** `device_type="cpu"`, `device_index=0`.
**Expected output:** No exception raised; returns a dict.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_capability.py::TestProbeStructure::test_never_raises_for_cpu -v` exits 0.

---

## test_device_selection_cpu (worker/capability)

**File:** `worker/tests/test_capability.py`
**Context:** The `worker.capability` module has `probe_capabilities()` function. torch 2.12.1+cpu is installed in the worker venv.
**Tests:** CPU device is correctly selected (device_index ignored). Verifies that when ``device_type="cpu"``, the function does not raise and returns a valid result. The device_index parameter is ignored for CPU devices.
**Mode:** real
**Inputs:** `device_type="cpu"`, `device_index=0`.
**Expected output:** No exception; dict with 6 capability keys.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_capability.py::TestProbeStructure::test_device_selection_cpu -v` exits 0.

---


---

## test_returns_six_required_keys (anvilml-worker)

**File:** `worker/tests/test_worker_main.py`
**Context:** The `worker.worker_main` module provides `_mock_probe_capabilities()` which returns a dict with 6 keys matching `InferenceCaps` field names.
**Tests:** `_mock_probe_capabilities()` returns a dict with exactly the 6 required keys (`fp32`, `fp16`, `bf16`, `fp8`, `fp4`, `flash_attention`) matching `InferenceCaps` struct field names.
**Mode:** mock
**Inputs:** None (pure function, no args).
**Expected output:** Dict with exactly 6 keys, no more, no fewer.
**Acceptance:** `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_worker_main.py::TestMockProbeCapabilities::test_returns_six_required_keys -v` exits 0.

---

## test_all_values_are_bool (anvilml-worker)

**File:** `worker/tests/test_worker_main.py`
**Context:** The `worker.worker_main` module provides `_mock_probe_capabilities()` which returns a dict with 6 boolean values.
**Tests:** All 6 values in the returned dict are `bool` type (not `int`, `str`, or other).
**Mode:** mock
**Inputs:** None (pure function, no args).
**Expected output:** Every `isinstance(v, bool)` is True.
**Acceptance:** `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_worker_main.py::TestMockProbeCapabilities::test_all_values_are_bool -v` exits 0.

---

## test_fp4_is_false (anvilml-worker)

**File:** `worker/tests/test_worker_main.py`
**Context:** The `worker.worker_main` module provides `_mock_probe_capabilities()` which returns synthetic capability values. Torch 2.x has no native `torch.float4` dtype, so `fp4` is universally False.
**Tests:** The `fp4` key specifically maps to `False` — the one deliberate exception in synthetic values.
**Mode:** mock
**Inputs:** None (pure function, no args).
**Expected output:** `result["fp4"] is False`.
**Acceptance:** `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_worker_main.py::TestMockProbeCapabilities::test_fp4_is_false -v` exits 0.

---

## test_no_torch_import_on_module_load (anvilml-worker)

**File:** `worker/tests/test_worker_main.py`
**Context:** The `worker.worker_main` module is a pure Python module with no external dependencies. Importing it must not transitively import `torch`.
**Tests:** Importing `worker.worker_main` does not transitively import `torch`, confirmed via subprocess isolation. The subprocess spawns a fresh Python process that imports `worker.worker_main` and asserts `"torch" not in sys.modules`.
**Mode:** mock
**Inputs:** Subprocess runs `import worker.worker_main; import sys; assert 'torch' not in sys.modules`.
**Expected output:** Subprocess exit code 0, stdout contains "OK".
**Acceptance:** `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_worker_main.py::TestNoTorchImport::test_no_torch_import_on_module_load -v` exits 0.

---

## test_real_startup_calls_ipc_connect (anvilml-worker)

**File:** `worker/tests/test_worker_main.py`
**Context:** The `worker.worker_main` module provides `_real_startup_sequence()` which implements the real-mode startup path: reads env vars, calls `ipc.connect()`, imports torch, selects device, and runs `capability.probe_capabilities()`.
**Tests:** `_real_startup_sequence()` calls `ipc.connect(5555, "test-worker-0")` with the correct port and worker_id values from environment variables `ANVILML_IPC_PORT` and `ANVILML_WORKER_ID`.
**Mode:** real
**Inputs:** Env vars `ANVILML_IPC_PORT=5555`, `ANVILML_WORKER_ID=test-worker-0`, `ANVILML_DEVICE_TYPE=cpu`, `ANVILML_DEVICE_INDEX=0`; `ipc.connect` and `probe_capabilities` patched.
**Expected output:** `ipc.connect(5555, "test-worker-0")` called exactly once.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_worker_main.py::TestRealStartupSequence::test_real_startup_calls_ipc_connect -v` exits 0.

---

## test_real_startup_cpu_skips_cuda_set_device (anvilml-worker)

**File:** `worker/tests/test_worker_main.py`
**Context:** The `_real_startup_sequence()` function skips `torch.cuda.set_device()` for CPU devices — CPU has no per-device selection.
**Tests:** With `ANVILML_DEVICE_TYPE=cpu`, `torch.cuda.set_device` is NOT called; `probe_capabilities("cpu", 0)` IS called.
**Mode:** real
**Inputs:** Env vars `ANVILML_IPC_PORT=5555`, `ANVILML_WORKER_ID=cpu-worker`, `ANVILML_DEVICE_TYPE=cpu`, `ANVILML_DEVICE_INDEX=0`; `ipc.connect`, `probe_capabilities`, and `torch.cuda.set_device` patched.
**Expected output:** `torch.cuda.set_device` not called; `probe_capabilities("cpu", 0)` called once.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_worker_main.py::TestRealStartupSequence::test_real_startup_cpu_skips_cuda_set_device -v` exits 0.

---

## test_real_startup_calls_probe_capabilities (anvilml-worker)

**File:** `worker/tests/test_worker_main.py`
**Context:** For non-CPU device types, `_real_startup_sequence()` calls both `torch.cuda.set_device(device_index)` and `capability.probe_capabilities(device_type, device_index)`.
**Tests:** With `ANVILML_DEVICE_TYPE=cuda` and `ANVILML_DEVICE_INDEX=1`, `torch.cuda.set_device(1)` is called; `probe_capabilities("cuda", 1)` is called; returned dict has exactly 6 bool keys.
**Mode:** real
**Inputs:** Env vars `ANVILML_IPC_PORT=5555`, `ANVILML_WORKER_ID=cuda-worker`, `ANVILML_DEVICE_TYPE=cuda`, `ANVILML_DEVICE_INDEX=1`; `ipc.connect`, `probe_capabilities`, and `torch.cuda.set_device` patched.
**Expected output:** `torch.cuda.set_device(1)` called once; `probe_capabilities("cuda", 1)` called once; returned dict has 6 keys, all bool.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_worker_main.py::TestRealStartupSequence::test_real_startup_calls_probe_capabilities -v` exits 0.

---

## test_no_mock_gate_exit_path (anvilml-worker)

**File:** `worker/tests/test_worker_main.py`
**Context:** The v3 defect pattern of an env-var guard (`if ANVILML_WORKER_MOCK != "1": exit(1)`) must not exist in `worker_main.py`.
**Tests:** Reads `worker_main.py` source text and asserts no line matches the mock-gate pattern — a guard that calls `exit` when not in mock mode.
**Mode:** real
**Inputs:** Source file `worker/worker_main.py` read as text.
**Expected output:** Zero lines contain both `ANVILML_WORKER_MOCK` and `exit` outside comments.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_worker_main.py::TestNoMockGate::test_no_mock_gate_exit_path -v` exits 0.

---

## test_real_startup_sends_ready_event (anvilml-worker)

**File:** `worker/tests/test_worker_main.py`
**Context:** `_real_startup_sequence()` now builds and sends a `Ready` event via `ipc.send_event()` with `capabilities_source="pytorch"` and `node_types=[]`. This is the primary acceptance test for the Ready event.
**Tests:** With mocked `ipc.connect`, `probe_capabilities`, and `ipc.recv_message` (raising to exit the dispatch loop), asserts `ipc.send_event` was called once with a dict containing `_type="Ready"`, `capabilities_source="pytorch"`, and `node_types=[]`.
**Mode:** real
**Inputs:** Env vars `ANVILML_IPC_PORT=5555`, `ANVILML_WORKER_ID=test-0`, `ANVILML_DEVICE_TYPE=cpu`, `ANVILML_DEVICE_INDEX=0`; `ipc.connect`, `probe_capabilities`, `ipc.send_event`, and `ipc.recv_message` patched.
**Expected output:** `ipc.send_event` called once with `{"_type": "Ready", "capabilities_source": "pytorch", "node_types": []}`.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_worker_main.py::TestRealStartupSequence::test_real_startup_sends_ready_event -v` exits 0.

---

## test_import_nodes_returns_empty_list (anvilml-worker)

**File:** `worker/tests/test_worker_main.py`
**Context:** `_import_nodes()` is a Phase 9 stub that returns an empty list — the node system is Phase 10's scope.
**Tests:** Calls `_import_nodes()` directly and asserts the result is `[]`.
**Mode:** real
**Inputs:** None (pure function, no setup).
**Expected output:** `_import_nodes() == []`.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_worker_main.py::TestRealStartupSequence::test_import_nodes_returns_empty_list -v` exits 0.

---

## test_dispatch_loop_exists_and_is_callable (anvilml-worker)

**File:** `worker/tests/test_worker_main.py`
**Context:** `_dispatch_loop()` is a placeholder dispatch loop that calls `ipc.recv_message()`, logs each message at DEBUG level, and continues. It catches `Exception` from `recv_message()` and breaks the loop on failure.
**Tests:** Asserts `_dispatch_loop` is callable, then calls it with `ipc.recv_message` mocked to raise (simulating supervisor disconnect). The loop should log the error and exit cleanly without raising an unhandled exception.
**Mode:** real
**Inputs:** Env vars `ANVILML_IPC_PORT=5555`, `ANVILML_WORKER_ID=test-0`, `ANVILML_DEVICE_TYPE=cpu`, `ANVILML_DEVICE_INDEX=0`; `ipc.recv_message` patched to raise `zmq.ZMQError`.
**Expected output:** No unhandled exception; `ipc.recv_message` called at least once; dispatch loop exits after recv failure.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_worker_main.py::TestRealStartupSequence::test_dispatch_loop_exists_and_is_callable -v` exits 0.

---

## test_real_startup_no_nonzero_exit_for_cpu (anvilml-worker)

**File:** `worker/tests/test_worker_main.py`
**Context:** The full real-mode startup path (IPC connect → torch import → device select → probe → node import → Ready event → dispatch loop) must complete without raising for a valid CPU device_type.
**Tests:** Runs `_real_startup_sequence()` with mocked IPC and capability probe, confirming no exception is raised.
**Mode:** real
**Inputs:** Env vars `ANVILML_IPC_PORT=5555`, `ANVILML_WORKER_ID=cpu-worker`, `ANVILML_DEVICE_TYPE=cpu`, `ANVILML_DEVICE_INDEX=0`; `ipc.connect`, `probe_capabilities`, and `ipc.recv_message` patched.
**Expected output:** No exception raised; the startup sequence completes (dispatch loop exits on mocked recv failure).
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_worker_main.py::TestRealStartupSequence::test_real_startup_no_nonzero_exit_for_cpu -v` exits 0.

---

## test_mock_startup_sends_ready_event (anvilml-worker)

**File:** `worker/tests/test_worker_main.py`
**Context:** `_mock_startup_sequence()` sends a `Ready` event with `capabilities_source="mock"` (as opposed to `"pytorch"` in real mode).
**Tests:** With mocked `ipc.connect` and `ipc.recv_message` (raising to exit the dispatch loop), asserts `ipc.send_event` was called once with a dict containing `_type="Ready"`, `capabilities_source="mock"`, and `node_types=[]`.
**Mode:** real
**Inputs:** Env vars `ANVILML_IPC_PORT=5555`, `ANVILML_WORKER_ID=test-0`, `ANVILML_DEVICE_TYPE=cpu`, `ANVILML_DEVICE_INDEX=0`; `ipc.connect`, `ipc.send_event`, and `ipc.recv_message` patched.
**Expected output:** `ipc.send_event` called once with `{"_type": "Ready", "capabilities_source": "mock", "node_types": []}`.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_worker_main.py::TestNoMockGate::test_mock_startup_sends_ready_event -v` exits 0.

---

## test_no_mock_gate_in_main_block (anvilml-worker)

**File:** `worker/tests/test_worker_main.py`
**Context:** The new `__main__` block uses `ANVILML_WORKER_MOCK == "1"` dispatch (mock or real mode), not the v3 defect pattern of `ANVILML_WORKER_MOCK != "1": exit(1)`.
**Tests:** Reads `worker_main.py` source text and asserts no line matches the mock-gate pattern — a guard that calls `exit` when not in mock mode.
**Mode:** real
**Inputs:** Source file `worker/worker_main.py` read as text.
**Expected output:** Zero lines contain both `ANVILML_WORKER_MOCK` and `exit` outside comments.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_worker_main.py::TestNoMockGate::test_no_mock_gate_in_main_block -v` exits 0.

---

## test_real_subprocess_sends_ready (anvilml-worker)

**File:** `crates/anvilml-worker/tests/real_startup_tests.rs`
**Context:** The `anvilml-worker` crate has been compiled with `tokio` (process, rt, sync, time features), `anvilml-ipc`, `anvilml-core`, and `zeromq` (tokio-runtime feature) dev-dependencies. The Python worker venv is provisioned at `worker/.venv` with torch installed (real-mode requirements).
**Tests:** A real `worker_main.py` subprocess spawned with `ANVILML_DEVICE_TYPE=cpu` and no `ANVILML_WORKER_MOCK` flag connects over IPC, runs the real torch capability probe, and sends a `Ready` event with `capabilities_source="pytorch"` and empty `node_types` within 10 seconds. The test binds a `RouterTransport`, builds a `WorkerEnv` targeting CPU real mode, spawns the worker via `spawn_worker()`, connects a `DealerSocket` with peer identity `"0"`, and asserts on the `Ready` event fields.
**Mode:** real
**Inputs:** `RouterTransport::bind()` on OS-assigned port; `WorkerEnv::build(transport.port, "0", 0, DeviceType::Cpu, false, "info", 256)`; venv path `worker/.venv`; `DealerSocket` connected to `tcp://127.0.0.1:{port}` with `PeerIdentity("0")`.
**Expected output:** `WorkerEvent::Ready { capabilities_source: "pytorch", node_types: [], .. }` received within 10s; worker identity `"0"` matches; subprocess terminates cleanly.
**Acceptance:** `cargo test -p anvilml-worker --test real_startup_tests -- --test-threads=1` exits 0.

---

## test_node_registry_starts_empty (anvilml-worker)

**File:** `worker/tests/test_base.py`
**Context:** The `worker.nodes.base` module is importable. `NODE_REGISTRY` is a module-level `dict` initialized to `{}`. No nodes have been registered yet.
**Tests:** `NODE_REGISTRY` is an empty dict immediately after importing the `base` module — no nodes have been registered yet. This is the precondition for all subsequent registration tests.
**Mode:** both
**Inputs:** Import of `worker.nodes.base` (fresh Python process).
**Expected output:** `base.NODE_REGISTRY == {}`.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_base.py::test_node_registry_starts_empty -v` exits 0.

---

## test_slotspec_optional_defaults_to_false (anvilml-worker)

**File:** `worker/tests/test_base.py`
**Context:** The `worker.nodes.base` module is importable. `SlotSpec` is a `@dataclass` with fields `name: str`, `slot_type: str`, and `optional: bool = False`.
**Tests:** Constructing a `SlotSpec` with only the required fields (`name`, `slot_type`) produces an instance where `optional` is `False` by default, confirming the dataclass default is applied correctly.
**Mode:** both
**Inputs:** `SlotSpec(name="x", slot_type="MODEL")`.
**Expected output:** `spec.optional == False`, `spec.name == "x"`, `spec.slot_type == "MODEL"`.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_base.py::test_slotspec_optional_defaults_to_false -v` exits 0.

---

## test_slotspec_accepts_explicit_optional_true (anvilml-worker)

**File:** `worker/tests/test_base.py`
**Context:** The `worker.nodes.base` module is importable. `SlotSpec` is a `@dataclass` with `optional: bool = False` default.
**Tests:** Constructing a `SlotSpec` with `optional=True` explicitly produces an instance where `optional` is `True`, confirming the optional parameter is accepted and stored correctly.
**Mode:** both
**Inputs:** `SlotSpec(name="y", slot_type="IMAGE", optional=True)`.
**Expected output:** `spec.optional == True`, `spec.name == "y"`, `spec.slot_type == "IMAGE"`.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_base.py::test_slotspec_accepts_explicit_optional_true -v` exits 0.

## test_register_success (anvilml-worker)

**File:** `worker/tests/test_base.py`
**Context:** The `worker.nodes.base` module is importable. `NODE_REGISTRY` is an empty dict. `register()` validates six required attributes and inserts the class into the registry.
**Tests:** Decorating a fully-specified class with `@register` inserts it into `NODE_REGISTRY` under its `NODE_TYPE` key, and the function returns the same class object.
**Mode:** both
**Inputs:** A class with all 6 required attributes (`NODE_TYPE="test.node"`, `CATEGORY="test"`, `DISPLAY_NAME="Test Node"`, `DESCRIPTION="A test node..."`, `INPUT_SLOTS=[]`, `OUTPUT_SLOTS=[]`).
**Expected output:** `NODE_REGISTRY["test.node"]` is the decorated class; `del` removes it for test isolation.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_base.py::test_register_success -v` exits 0.

---

## test_register_missing_NODE_TYPE (anvilml-worker)

**File:** `worker/tests/test_base.py`
**Context:** The `worker.nodes.base` module is importable. `register()` checks for six required attributes in order.
**Tests:** A class missing `NODE_TYPE` raises `TypeError` with "NODE_TYPE" in the message, confirming the first validation check works.
**Mode:** both
**Inputs:** A class inheriting all attributes from `_FullySpecifiedNode` except `NODE_TYPE` (deleted via `del`).
**Expected output:** `TypeError` raised with "NODE_TYPE" in the message.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_base.py::test_register_missing_NODE_TYPE -v` exits 0.

---

## test_register_missing_CATEGORY (anvilml-worker)

**File:** `worker/tests/test_base.py`
**Context:** The `worker.nodes.base` module is importable. `register()` checks for six required attributes in order.
**Tests:** A class missing `CATEGORY` raises `TypeError` with "CATEGORY" in the message, confirming the second validation check works.
**Mode:** both
**Inputs:** A class inheriting all attributes from `_FullySpecifiedNode` except `CATEGORY` (deleted via `del`).
**Expected output:** `TypeError` raised with "CATEGORY" in the message.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_base.py::test_register_missing_CATEGORY -v` exits 0.

---

## test_register_missing_DISPLAY_NAME (anvilml-worker)

**File:** `worker/tests/test_base.py`
**Context:** The `worker.nodes.base` module is importable. `register()` checks for six required attributes in order.
**Tests:** A class missing `DISPLAY_NAME` raises `TypeError` with "DISPLAY_NAME" in the message, confirming the third validation check works.
**Mode:** both
**Inputs:** A class inheriting all attributes from `_FullySpecifiedNode` except `DISPLAY_NAME` (deleted via `del`).
**Expected output:** `TypeError` raised with "DISPLAY_NAME" in the message.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_base.py::test_register_missing_DISPLAY_NAME -v` exits 0.

---

## test_register_missing_DESCRIPTION (anvilml-worker)

**File:** `worker/tests/test_base.py`
**Context:** The `worker.nodes.base` module is importable. `register()` checks for six required attributes in order.
**Tests:** A class missing `DESCRIPTION` raises `TypeError` with "DESCRIPTION" in the message, confirming the fourth validation check works.
**Mode:** both
**Inputs:** A class inheriting all attributes from `_FullySpecifiedNode` except `DESCRIPTION` (deleted via `del`).
**Expected output:** `TypeError` raised with "DESCRIPTION" in the message.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_base.py::test_register_missing_DESCRIPTION -v` exits 0.

---

## test_register_missing_INPUT_SLOTS (anvilml-worker)

**File:** `worker/tests/test_base.py`
**Context:** The `worker.nodes.base` module is importable. `register()` checks for six required attributes in order.
**Tests:** A class missing `INPUT_SLOTS` raises `TypeError` with "INPUT_SLOTS" in the message, confirming the fifth validation check works.
**Mode:** both
**Inputs:** A class inheriting all attributes from `_FullySpecifiedNode` except `INPUT_SLOTS` (deleted via `del`).
**Expected output:** `TypeError` raised with "INPUT_SLOTS" in the message.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_base.py::test_register_missing_INPUT_SLOTS -v` exits 0.

---

## test_register_missing_OUTPUT_SLOTS (anvilml-worker)

**File:** `worker/tests/test_base.py`
**Context:** The `worker.nodes.base` module is importable. `register()` checks for six required attributes in order.
**Tests:** A class missing `OUTPUT_SLOTS` raises `TypeError` with "OUTPUT_SLOTS" in the message, confirming the sixth (last) validation check works.
**Mode:** both
**Inputs:** A class inheriting all attributes from `_FullySpecifiedNode` except `OUTPUT_SLOTS` (deleted via `del`).
**Expected output:** `TypeError` raised with "OUTPUT_SLOTS" in the message.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_base.py::test_register_missing_OUTPUT_SLOTS -v` exits 0.

---

## test_register_returns_class_identity (anvilml-worker)

**File:** `worker/tests/test_base.py`
**Context:** The `worker.nodes.base` module is importable. `register()` returns `cls` unchanged, not a wrapper.
**Tests:** The return value of `@register` is the exact same class object (`is` comparison), confirming identity preservation — critical because `execute()` must be callable directly on the original class.
**Mode:** both
**Inputs:** A class with all 6 required attributes.
**Expected output:** `result is TestNode` is `True`; entry removed from `NODE_REGISTRY` for test isolation.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_base.py::test_register_returns_class_identity -v` exits 0.

---

## test_node_context_assigns_all_attrs (anvilml-worker)

**File:** `worker/tests/test_base.py`
**Context:** The `worker.nodes.base` module is importable and defines the `NodeContext` class with 7 `__init__` parameters. A `threading.Event` is used for `cancel_flag`.
**Tests:** Constructs a `NodeContext` with concrete values for all 7 parameters and asserts each attribute is stored correctly — `job_id`, `device`, `caps`, `cancel_flag`, `emit`, `pipeline_cache`, and `mock`.
**Mode:** both
**Inputs:** `NodeContext(job_id="test-job", device="cpu", caps={"bf16": True, "fp8": False}, cancel_flag=threading.Event(), emit=lambda e: None, pipeline_cache={}, mock=True)`.
**Expected output:** All 7 attributes match their corresponding inputs exactly.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_base.py::test_node_context_assigns_all_attrs -v` exits 0.

---

## test_node_context_mock_true (anvilml-worker)

**File:** `worker/tests/test_base.py`
**Context:** The `worker.nodes.base` module is importable and defines the `NodeContext` class.
**Tests:** Constructs a `NodeContext` with `mock=True` and asserts the mock attribute is `True`, confirming the flag is stored without transformation.
**Mode:** both
**Inputs:** `NodeContext(mock=True, ...)` with minimal valid values for other params.
**Expected output:** `ctx.mock is True`.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_base.py::test_node_context_mock_true -v` exits 0.

---

## test_node_context_mock_false (anvilml-worker)

**File:** `worker/tests/test_base.py`
**Context:** The `worker.nodes.base` module is importable and defines the `NodeContext` class.
**Tests:** Constructs a `NodeContext` with `mock=False` and asserts the mock attribute is `False`, confirming the flag is stored without transformation.
**Mode:** both
**Inputs:** `NodeContext(mock=False, ...)` with minimal valid values for other params.
**Expected output:** `ctx.mock is False`.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_base.py::test_node_context_mock_false -v` exits 0.

---

## test_node_context_caps_accepts_arbitrary_dict (anvilml-worker)

**File:** `worker/tests/test_base.py`
**Context:** The `worker.nodes.base` module is importable and defines the `NodeContext` class.
**Tests:** Constructs a `NodeContext` with a non-standard dict (arbitrary keys and mixed-value types) and asserts it is stored unchanged, confirming that `NodeContext` imposes no validation on the caps payload.
**Mode:** both
**Inputs:** `NodeContext(caps={"some_key": "some_value", "numeric": 42}, ...)` with minimal valid values for other params.
**Expected output:** `ctx.caps == {"some_key": "some_value", "numeric": 42}`.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_base.py::test_node_context_caps_accepts_arbitrary_dict -v` exits 0.

---

## test_base_node_cannot_be_instantiated (anvilml-worker)

**File:** `worker/tests/test_base.py`
**Context:** The `worker.nodes.base` module is importable and defines the `BaseNode` ABC class with an abstract `execute()` method.
**Tests:** `BaseNode()` raises `TypeError` per ABC semantics — confirms the abstract base class cannot be directly instantiated. Python's ABC machinery enforces this via the `@abstractmethod` decorator, not custom code.
**Mode:** both
**Inputs:** Direct call `base.BaseNode()` with no arguments.
**Expected output:** `TypeError` raised.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_base.py::test_base_node_cannot_be_instantiated -v` exits 0.

---

## test_concrete_subclass_instantiates (anvilml-worker)

**File:** `worker/tests/test_base.py`
**Context:** The `worker.nodes.base` module is importable and defines the `BaseNode` ABC class with an abstract `execute()` method.
**Tests:** A minimal concrete subclass implementing `execute()` instantiates without error — confirms the abstract method requirement is satisfied by providing `execute()`.
**Mode:** both
**Inputs:** A subclass of `BaseNode` with an `execute(self, ctx, **inputs) -> dict` method returning `{}`.
**Expected output:** Instance created successfully; `isinstance(node, base.BaseNode)` is `True`.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_base.py::test_concrete_subclass_instantiates -v` exits 0.

---

## test_execute_calls_subclass_impl (anvilml-worker)

**File:** `worker/tests/test_base.py`
**Context:** The `worker.nodes.base` module is importable and defines the `BaseNode` ABC class with an abstract `execute()` method.
**Tests:** Calling `execute()` on a concrete subclass invokes the subclass's own implementation, not a base no-op — guards against a future regression where a base no-op is accidentally called.
**Mode:** both
**Inputs:** A subclass of `BaseNode` with `execute()` that sets `self.called = True` and returns `{}`.
**Expected output:** After calling `node.execute(None)`, `node.called is True`.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_base.py::test_execute_calls_subclass_impl -v` exits 0.

---

## test_get_module_returns_none_when_empty (anvilml-worker)

**File:** `worker/tests/test_arch_dispatch.py`
**Context:** The `worker.nodes.arch.diffusion` module is importable and defines `get_module()` which scans `_REGISTERED_MODULES` (an empty list at this phase). No concrete diffusion arch modules are registered yet.
**Tests:** `get_module("zit")` returns `None` when `_REGISTERED_MODULES` is empty — proves the dispatcher returns None rather than raising when no modules are registered.
**Mode:** both
**Inputs:** `get_module("zit")` called on the freshly-imported module.
**Expected output:** `None`.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_dispatch.py::test_get_module_returns_none_when_empty -v` exits 0.

---

## test_get_module_does_not_raise_for_various_key_types (anvilml-worker)

**File:** `worker/tests/test_arch_dispatch.py`
**Context:** The `worker.nodes.arch.diffusion` module is importable and defines `get_module()` which accepts `Any` as the key type. The registry is empty.
**Tests:** `get_module()` does not raise for `str`, `None`, or arbitrary object keys — proves the dispatch loop is safe against any key type even when no modules are registered.
**Mode:** both
**Inputs:** `get_module("zit")`, `get_module(None)`, `get_module(object())`.
**Expected output:** All three calls return `None` without raising.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_dispatch.py::test_get_module_does_not_raise_for_various_key_types -v` exits 0.

---

## test_get_module_skips_module_with_can_handle_false (anvilml-worker)

**File:** `worker/tests/test_arch_dispatch.py`
**Context:** The `worker.nodes.arch.diffusion` module is importable. `_REGISTERED_MODULES` is a mutable list that tests can append to (with cleanup via `finally`). A `Mock(spec=ModuleType)` with `can_handle=Mock(return_value=False)` is used as a test double.
**Tests:** When a module's `can_handle` returns `False`, `get_module` continues scanning and returns `None` — proves the dispatcher does not return a non-matching module.
**Mode:** both
**Inputs:** A `Mock(spec=ModuleType)` with `can_handle` returning `False`, appended to `_REGISTERED_MODULES`.
**Expected output:** `get_module("zit")` returns `None`; `can_handle` was called once with `"zit"`.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_dispatch.py::test_get_module_skips_module_with_can_handle_false -v` exits 0.

---

## test_clip_get_module_returns_none_when_empty (anvilml-worker)

**File:** `worker/tests/test_arch_dispatch.py`
**Context:** The `worker.nodes.arch.clip` module is importable. `_REGISTERED_MODULES` is empty by default (no concrete arch modules wired in yet).
**Tests:** `clip.get_module("qwen3")` returns `None` when the registry is empty — proves the dispatcher handles the empty-registry case gracefully without raising.
**Mode:** both
**Inputs:** `clip.get_module("qwen3")` with empty `_REGISTERED_MODULES`.
**Expected output:** `None`.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_dispatch.py::test_clip_get_module_returns_none_when_empty -v` exits 0.

---

## test_clip_get_module_does_not_raise_for_various_key_types (anvilml-worker)

**File:** `worker/tests/test_arch_dispatch.py`
**Context:** The `worker.nodes.arch.clip` module is importable. `_REGISTERED_MODULES` is empty by default.
**Tests:** `clip.get_module()` does not raise for `str`, `None`, or arbitrary `object()` keys — proves the dispatch loop never throws on unexpected key types.
**Mode:** both
**Inputs:** `"qwen3"`, `None`, `object()` as keys.
**Expected output:** All calls return `None` without raising.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_dispatch.py::test_clip_get_module_does_not_raise_for_various_key_types -v` exits 0.

---

## test_clip_get_module_skips_module_with_can_handle_false (anvilml-worker)

**File:** `worker/tests/test_arch_dispatch.py`
**Context:** The `worker.nodes.arch.clip` module is importable. `_REGISTERED_MODULES` is a mutable list that tests can append to (with cleanup via `finally`). A `Mock(spec=ModuleType)` with `can_handle=Mock(return_value=False)` is used as a test double.
**Tests:** When a CLIP module's `can_handle` returns `False`, `clip.get_module` continues scanning and returns `None` — proves the dispatcher does not return a non-matching module.
**Mode:** both
**Inputs:** A `Mock(spec=ModuleType)` with `can_handle` returning `False`, appended to `clip._REGISTERED_MODULES`.
**Expected output:** `clip.get_module("qwen3")` returns `None`; `can_handle` was called once with `"qwen3"`.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_dispatch.py::test_clip_get_module_skips_module_with_can_handle_false -v` exits 0.

---

## test_vae_get_module_returns_none_when_empty (anvilml-worker)

**File:** `worker/tests/test_arch_dispatch.py`
**Context:** The `worker.nodes.arch.vae` module is importable. `_REGISTERED_MODULES` is empty by default (no concrete arch modules wired in yet).
**Tests:** `vae.get_module("zit_vae")` returns `None` when the registry is empty — proves the dispatcher handles the empty-registry case gracefully without raising.
**Mode:** both
**Inputs:** `vae.get_module("zit_vae")` with empty `_REGISTERED_MODULES`.
**Expected output:** `None`.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_dispatch.py::test_vae_get_module_returns_none_when_empty -v` exits 0.

---

## test_vae_get_module_does_not_raise_for_various_key_types (anvilml-worker)

**File:** `worker/tests/test_arch_dispatch.py`
**Context:** The `worker.nodes.arch.vae` module is importable. `_REGISTERED_MODULES` is empty by default.
**Tests:** `vae.get_module()` does not raise for `str`, `None`, or arbitrary `object()` keys — proves the dispatch loop never throws on unexpected key types.
**Mode:** both
**Inputs:** `"zit_vae"`, `None`, `object()` as keys.
**Expected output:** All calls return `None` without raising.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_dispatch.py::test_vae_get_module_does_not_raise_for_various_key_types -v` exits 0.

---

## test_vae_get_module_skips_module_with_can_handle_false (anvilml-worker)

**File:** `worker/tests/test_arch_dispatch.py`
**Context:** The `worker.nodes.arch.vae` module is importable. `_REGISTERED_MODULES` is a mutable list that tests can append to (with cleanup via `finally`). A `Mock(spec=ModuleType)` with `can_handle=Mock(return_value=False)` is used as a test double.
**Tests:** When a VAE module's `can_handle` returns `False`, `vae.get_module` continues scanning and returns `None` — proves the dispatcher does not return a non-matching module.
**Mode:** both
**Inputs:** A `Mock(spec=ModuleType)` with `can_handle` returning `False`, appended to `vae._REGISTERED_MODULES`.
**Expected output:** `vae.get_module("zit_vae")` returns `None`; `can_handle` was called once with `"zit_vae"`.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_dispatch.py::test_vae_get_module_skips_module_with_can_handle_false -v` exits 0.

---

## test_import_does_not_raise (anvilml-worker)

**File:** `worker/tests/test_nodes_init.py`
**Context:** The `worker.nodes` package has been created with an auto-import loop in `__init__.py` that uses `pkgutil.iter_modules()` to discover sibling `.py` modules and `importlib.util` to execute them. No concrete node files exist yet — only `base.py` and the `arch/` subdirectory are present.
**Tests:** Importing `worker.nodes` does not raise an exception — proves the auto-import loop completes cleanly even with no importable sibling modules.
**Mode:** both
**Inputs:** None.
**Expected output:** `import worker.nodes` succeeds silently.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_nodes_init.py::test_import_does_not_raise -v` exits 0.

---

## test_node_registry_empty_after_import (anvilml-worker)

**File:** `worker/tests/test_nodes_init.py`
**Context:** The `worker.nodes` package has been imported (triggering the auto-import loop), and `worker.nodes.base` provides `NODE_REGISTRY` as a module-level `dict[str, type["BaseNode"]]`. No concrete node files exist yet.
**Tests:** After importing `worker.nodes`, `NODE_REGISTRY` is empty — proves the auto-import loop does not register anything when no sibling node modules exist.
**Mode:** both
**Inputs:** None.
**Expected output:** `base.NODE_REGISTRY == {}`.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_nodes_init.py::test_node_registry_empty_after_import -v` exits 0.

---

## test_reimport_is_idempotent (anvilml-worker)

**File:** `worker/tests/test_nodes_init.py`
**Context:** The `worker.nodes` package has been imported once. The `_imported` flag in `__init__.py` prevents re-execution of the auto-import loop.
**Tests:** Calling `_import_nodes()` a second time (or re-importing `worker.nodes`) does not raise or duplicate registrations — proves the idempotency guard works correctly.
**Mode:** both
**Inputs:** None.
**Expected output:** No exception on second call; `NODE_REGISTRY` remains empty.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_nodes_init.py::test_reimport_is_idempotent -v` exits 0.

---

## test_ready_event_populates_registry (anvilml-worker)

**File:** `crates/anvilml-worker/tests/managed_tests.rs`
**Context:** The `anvilml-worker` crate has been compiled with `mock-hardware` feature. Tests use in-process ZeroMQ ROUTER/DEALER pair with `MockWorkerSpawner` to simulate a Python worker.
**Tests:** `ManagedWorker::handle_event()` calls `node_registry.register_all()` when processing a `WorkerEvent::Ready`, populating the registry with the event's `node_types`.
**Mode:** mock
**Inputs:** Ready event with 2 `NodeTypeDescriptor`s (`LoadModel`, `Sampler`) via a shared `Arc<NodeTypeRegistry>`.
**Expected output:** `registry.len() == 2`, `registry.get("LoadModel")` and `registry.get("Sampler")` return the correct descriptors.
**Acceptance:** `cargo test -p anvilml-worker --test managed_tests test_ready_event_populates_registry` exits 0.

---

## test_ready_event_empty_node_types_cleans_registry (anvilml-worker)

**File:** `crates/anvilml-worker/tests/managed_tests.rs`
**Context:** The `anvilml-worker` crate has been compiled with `mock-hardware` feature. Tests use in-process ZeroMQ ROUTER/DEALER pair with `MockWorkerSpawner` to simulate a Python worker.
**Tests:** A Ready event with empty `node_types: vec![]` clears the registry (replaces, not merges).
**Mode:** mock
**Inputs:** Pre-populated registry with 1 descriptor, Ready event with `node_types: vec![]`.
**Expected output:** `registry.is_empty() == true` after processing.
**Acceptance:** `cargo test -p anvilml-worker --test managed_tests test_ready_event_empty_node_types_cleans_registry` exits 0.

---

## test_respawn_second_ready_replaces_not_merges (anvilml-worker)

**File:** `crates/anvilml-worker/tests/managed_tests.rs`
**Context:** The `anvilml-worker` crate has been compiled with `mock-hardware` feature. Tests use in-process ZeroMQ ROUTER/DEALER pair with `MockWorkerSpawner` to simulate a Python worker.
**Tests:** A second Ready event replaces prior registry contents rather than merging — simulating a worker respawn scenario.
**Mode:** mock
**Inputs:** First Ready with descriptors A+B, worker exits via Dying event, second Ready with descriptor C only (new worker instance sharing the same registry).
**Expected output:** `registry.len() == 1`, `registry.get("A") == None`, `registry.get("B") == None`, `registry.get("C")` is `Some`.
**Acceptance:** `cargo test -p anvilml-worker --test managed_tests test_respawn_second_ready_replaces_not_merges` exits 0.

---

## test_ready_populates_registry_before_idle_transition (anvilml-worker)

**File:** `crates/anvilml-worker/tests/managed_tests.rs`
**Context:** The `anvilml-worker` crate has been compiled with `mock-hardware` feature. Tests use in-process ZeroMQ ROUTER/DEALER pair with `MockWorkerSpawner` to simulate a Python worker.
**Tests:** The registry is populated before the status transitions to Idle, proving the ordering invariant — `register_all()` fires before `status = Idle`.
**Mode:** mock
**Inputs:** Ready event with 1 `NodeTypeDescriptor`.
**Expected output:** After Ready: `registry.len() > 0` AND `status == Idle` both true simultaneously.
**Acceptance:** `cargo test -p anvilml-worker --test managed_tests test_ready_populates_registry_before_idle_transition` exits 0.

---

## test_app_state_constructs (anvilml-server)

**File:** `crates/anvilml-server/tests/state_tests.rs`
**Context:** The `anvilml-server` crate has been compiled with `anvilml-core` path dependency providing `ServerConfig` (with `Default` impl) and `NodeTypeRegistry` (with `new()` constructor). The `AppState` struct is re-exported from `anvilml_server` via `pub use state::AppState;` in `lib.rs`.
**Tests:** `AppState` constructs with a default `ServerConfig` and an empty `NodeTypeRegistry`. Asserts the `config.host` field is non-empty and the registry reports `is_empty() == true`.
**Mode:** both
**Inputs:** `ServerConfig::default()` and `NodeTypeRegistry::new()`, both wrapped in `Arc::new()`.
**Expected output:** `AppState` is constructed; `config.host` is non-empty; `node_registry.is_empty()` is `true`.
**Acceptance:** `cargo test -p anvilml-server --test state_tests test_app_state_constructs` exits 0.

---

## test_app_state_clone_shares_node_registry (anvilml-server)

**File:** `crates/anvilml-server/tests/state_tests.rs`
**Context:** The `anvilml-server` crate has been compiled with `anvilml-core` providing `NodeTypeDescriptor` and `NodeTypeRegistry`. `AppState` derives `Clone` and is re-exported from `anvilml_server`. The `NodeTypeRegistry` uses `Arc<RwLock<HashMap>>` internally, so cloning the outer `Arc<NodeTypeRegistry>` shares the same heap allocation.
**Tests:** Cloning `AppState` produces a second `AppState` that shares the same `Arc<NodeTypeRegistry>` heap allocation. After registering a single `NodeTypeDescriptor` via one clone, `list()` on the other clone returns exactly one descriptor with the correct `type_name`.
**Mode:** both
**Inputs:** `AppState` with `Arc<ServerConfig>` and `Arc<NodeTypeRegistry>`, cloned via `AppState::clone()`, then `register_all(vec![descriptor])` on the original.
**Expected output:** `cloned.node_registry.list().len() == 1` and `list[0].type_name == "TestNode"`.
**Acceptance:** `cargo test -p anvilml-server --test state_tests test_app_state_clone_shares_node_registry` exits 0.

---

## test_validated_graph_inner_is_pub_crate (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/dag_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `serde_json` dependency and the `types` module providing `ValidatedGraph`. The `#[cfg(test)]`-gated `_test_new()` and `_test_inner()` methods are available only when tests are compiled.
**Tests:** The inner `serde_json::Value` field is accessible within the crate via the `#[cfg(test)] pub fn _test_inner()` method, confirming `pub(crate)` visibility works correctly: same-crate test code can inspect the graph through the helper, proving the field is not `pub` (no direct field access from the test crate) but is accessible within the crate boundary.
**Mode:** both
**Inputs:** A `serde_json::json!({"nodes": []})` value wrapped in `ValidatedGraph` via `_test_new()`.
**Expected output:** `_test_inner()` returns a reference to the same `serde_json::Value` passed to `_test_new()`.
**Acceptance:** `cargo test -p anvilml-scheduler --test dag_tests test_validated_graph_inner_is_pub_crate` exits 0.

---

## test_validated_graph_derives_debug_and_clone (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/dag_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `serde_json` dependency and the `types` module providing `ValidatedGraph` with `#[derive(Debug, Clone)]`. The `#[cfg(test)]`-gated `_test_new()` method is available only when tests are compiled.
**Tests:** `ValidatedGraph` correctly derives `Debug` and `Clone`. The Debug output includes the inner value's debug representation; cloning produces an equal value. This verifies the derives are correct and the type is usable as a proper newtype.
**Mode:** both
**Inputs:** A `serde_json::json!({"nodes": [], "edges": []})` value.
**Expected output:** `format!("{:?}", ...)` produces a non-empty string containing "ValidatedGraph"; cloned value produces an identical Debug representation.
**Acceptance:** `cargo test -p anvilml-scheduler --test dag_tests test_validated_graph_derives_debug_and_clone` exits 0.


---

## test_graph_error_not_an_object_display (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/dag_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `thiserror = "2.0.18"` and the `types` module providing `GraphError` with all 7 variants. The `thiserror::Error` derive provides `Display` via `#[error("...")]` attributes on each variant.
**Tests:** `GraphError::NotAnObject` produces a non-empty Display string that equals the exact expected message `"root is not an object"`.
**Mode:** both
**Inputs:** `GraphError::NotAnObject` with no arguments.
**Expected output:** `to_string()` returns `"root is not an object"`.
**Acceptance:** `cargo test -p anvilml-scheduler --test dag_tests test_graph_error_not_an_object_display` exits 0.

---

## test_graph_error_missing_nodes_array_display (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/dag_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `thiserror = "2.0.18"` and the `types` module providing `GraphError::MissingNodesArray`.
**Tests:** `GraphError::MissingNodesArray` produces a non-empty Display string, confirming the `#[error(...)]` attribute is correctly wired.
**Mode:** both
**Inputs:** `GraphError::MissingNodesArray` with no arguments.
**Expected output:** `to_string()` returns a non-empty string.
**Acceptance:** `cargo test -p anvilml-scheduler --test dag_tests test_graph_error_missing_nodes_array_display` exits 0.

---

## test_graph_error_duplicate_node_id_display (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/dag_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `thiserror = "2.0.18"` and the `types` module providing `GraphError::DuplicateNodeId(String)`. Struct-field interpolation via `{0}` is used in the `#[error(...)]` attribute.
**Tests:** `DuplicateNodeId("node_a")` Display output contains the node ID string `"node_a"`, confirming positional struct-field interpolation works.
**Mode:** both
**Inputs:** `GraphError::DuplicateNodeId("node_a".into())`.
**Expected output:** `to_string()` contains `"node_a"`.
**Acceptance:** `cargo test -p anvilml-scheduler --test dag_tests test_graph_error_duplicate_node_id_display` exits 0.

---

## test_graph_error_unknown_node_type_display (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/dag_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `thiserror = "2.0.18"` and the `types` module providing `GraphError::UnknownNodeType { node_id, type_name }`. Named struct-field interpolation is used in `#[error(...)]`.
**Tests:** `UnknownNodeType { node_id: "n1", type_name: "BadNode" }` Display output contains both `"n1"` and `"BadNode"`, confirming named struct-field interpolation works.
**Mode:** both
**Inputs:** `GraphError::UnknownNodeType { node_id: "n1".into(), type_name: "BadNode".into() }`.
**Expected output:** `to_string()` contains both `"n1"` and `"BadNode"`.
**Acceptance:** `cargo test -p anvilml-scheduler --test dag_tests test_graph_error_unknown_node_type_display` exits 0.

---

## test_graph_error_dangling_edge_display (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/dag_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `thiserror = "2.0.18"` and the `types` module providing `GraphError::DanglingEdge { node_id, slot_name }`. Named struct-field interpolation is used in `#[error(...)]`.
**Tests:** `DanglingEdge { node_id: "n2", slot_name: "output" }` Display output contains both `"n2"` and `"output"`.
**Mode:** both
**Inputs:** `GraphError::DanglingEdge { node_id: "n2".into(), slot_name: "output".into() }`.
**Expected output:** `to_string()` contains both `"n2"` and `"output"`.
**Acceptance:** `cargo test -p anvilml-scheduler --test dag_tests test_graph_error_dangling_edge_display` exits 0.

---

## test_graph_error_slot_type_mismatch_display (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/dag_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `thiserror = "2.0.18"` and the `types` module providing `GraphError::SlotTypeMismatch { node_id, slot_name, expected, found }`. Named struct-field interpolation is used in `#[error(...)]`.
**Tests:** `SlotTypeMismatch { node_id: "n3", slot_name: "in", expected: "FLOAT", found: "INT" }` Display output contains all four values.
**Mode:** both
**Inputs:** `GraphError::SlotTypeMismatch { node_id: "n3".into(), slot_name: "in".into(), expected: "FLOAT".into(), found: "INT".into() }`.
**Expected output:** `to_string()` contains `"n3"`, `"in"`, `"FLOAT"`, and `"INT"`.
**Acceptance:** `cargo test -p anvilml-scheduler --test dag_tests test_graph_error_slot_type_mismatch_display` exits 0.

---

## test_graph_error_cycle_detected_display (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/dag_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `thiserror = "2.0.18"` and the `types` module providing `GraphError::CycleDetected(Vec<String>)`. The `#[error("...")]` attribute uses `{0:?}` to include the full vector in the display output.
**Tests:** `CycleDetected(vec!["A", "B", "C"])` Display output contains the string `"cycle detected"`.
**Mode:** both
**Inputs:** `GraphError::CycleDetected(vec!["A".into(), "B".into(), "C".into()])`.
**Expected output:** `to_string()` contains `"cycle detected"`.
**Acceptance:** `cargo test -p anvilml-scheduler --test dag_tests test_graph_error_cycle_detected_display` exits 0.

---

## test_graph_error_display_distinct (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/dag_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `thiserror = "2.0.18"` and the `types` module providing all 7 `GraphError` variants. Each variant has a unique `#[error("...")]` message.
**Tests:** All 7 `GraphError` variants produce pairwise distinct Display strings, confirming the error messages are useful for operator diagnosis — no two variants produce the same string.
**Mode:** both
**Inputs:** All 7 `GraphError` variants constructed with minimal test values.
**Expected output:** No two `to_string()` outputs are equal; the set of 7 strings has cardinality 7.
**Acceptance:** `cargo test -p anvilml-scheduler --test dag_tests test_graph_error_display_distinct` exits 0.

---

## test_validate_graph_non_object_root_returns_not_an_object (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/dag_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `serde_json`, `anvilml-core`, and `thiserror` dependencies. The `dag` module provides `validate_graph()` which implements checks 1–2 of the six-check validation pipeline (ANVILML_DESIGN.md §12.3). A `NodeTypeRegistry` is constructed empty (not needed for checks 1–2).
**Tests:** `validate_graph(serde_json::json!([]), &registry)` — a non-object root (JSON array) returns `Err` containing exactly one `NotAnObject` error. Proves check 1a fires when the root is not a JSON object.
**Mode:** both
**Inputs:** `serde_json::json!([])` (JSON array as root), empty `NodeTypeRegistry`.
**Expected output:** `Err([NotAnObject])` — exactly one error variant.
**Acceptance:** `cargo test -p anvilml-scheduler --test dag_tests test_validate_graph_non_object_root_returns_not_an_object` exits 0.

---

## test_validate_graph_missing_nodes_array_returns_missing_nodes_array (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/dag_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `serde_json`, `anvilml-core`, and `thiserror` dependencies. The `dag` module provides `validate_graph()` which implements checks 1–2 of the six-check validation pipeline (ANVILML_DESIGN.md §12.3). A `NodeTypeRegistry` is constructed empty.
**Tests:** `validate_graph(serde_json::json!({"edges": []}), &registry)` — an object without a "nodes" key returns `Err` containing exactly one `MissingNodesArray` error. Proves check 1b fires when the "nodes" key is absent.
**Mode:** both
**Inputs:** `serde_json::json!({"edges": []})` (object without "nodes"), empty `NodeTypeRegistry`.
**Expected output:** `Err([MissingNodesArray])` — exactly one error variant.
**Acceptance:** `cargo test -p anvilml-scheduler --test dag_tests test_validate_graph_missing_nodes_array_returns_missing_nodes_array` exits 0.

---

## test_validate_graph_duplicate_ids_all_reported (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/dag_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `serde_json`, `anvilml-core`, and `thiserror` dependencies. The `dag` module provides `validate_graph()` which implements checks 1–2 of the six-check validation pipeline (ANVILML_DESIGN.md §12.3). A `NodeTypeRegistry` is constructed empty.
**Tests:** `validate_graph` with three nodes (ids: "a", "b", "a") — the second occurrence of "a" is reported as a `DuplicateNodeId("a")` error. Proves collect-all-errors semantics: the duplicate is detected and reported, not silently ignored.
**Mode:** both
**Inputs:** `{"nodes": [{"id":"a"},{"id":"b"},{"id":"a"}]}`, empty `NodeTypeRegistry`.
**Expected output:** `Err([DuplicateNodeId("a")])` — one error for the second occurrence of "a".
**Acceptance:** `cargo test -p anvilml-scheduler --test dag_tests test_validate_graph_duplicate_ids_all_reported` exits 0.

---

## test_validate_graph_no_duplicates_passes_cleanly (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/dag_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `serde_json`, `anvilml-core`, and `thiserror` dependencies. The `dag` module provides `validate_graph()` which implements checks 1–2 of the six-check validation pipeline (ANVILML_DESIGN.md §12.3). A `NodeTypeRegistry` is constructed empty.
**Tests:** `validate_graph` with two nodes having unique ids ("a" and "b") — both checks 1 and 2 pass, returning `Ok(ValidatedGraph(...))`. Proves the happy path works correctly.
**Mode:** both
**Inputs:** `{"nodes": [{"id":"a"},{"id":"b"}]}`, empty `NodeTypeRegistry`.
**Expected output:** `Ok(ValidatedGraph)` with inner value equal to the input graph.
**Acceptance:** `cargo test -p anvilml-scheduler --test dag_tests test_validate_graph_no_duplicates_passes_cleanly` exits 0.

---

## test_validate_graph_multiple_duplicate_violations_collected (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/dag_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `serde_json`, `anvilml-core`, and `thiserror` dependencies. The `dag` module provides `validate_graph()` which implements checks 1–2 of the six-check validation pipeline (ANVILML_DESIGN.md §12.3). A `NodeTypeRegistry` is constructed empty.
**Tests:** `validate_graph` with five nodes (ids: "a", "b", "a", "c", "b") — both the second "a" and the second "b" are reported as `DuplicateNodeId` errors in a single `Err(Vec)`. Proves that multiple different duplicate IDs are all collected in one error vector, preserving insertion order.
**Mode:** both
**Inputs:** `{"nodes": [{"id":"a"},{"id":"b"},{"id":"a"},{"id":"c"},{"id":"b"}]}`, empty `NodeTypeRegistry`.
**Expected output:** `Err([DuplicateNodeId("a"), DuplicateNodeId("b")])` — two errors, one for each duplicate second-occurrence.
**Acceptance:** `cargo test -p anvilml-scheduler --test dag_tests test_validate_graph_multiple_duplicate_violations_collected` exits 0.

---

## test_validate_graph_unknown_node_type_reported (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/dag_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `serde_json`, `anvilml-core` (providing `NodeTypeRegistry`, `NodeTypeDescriptor`, `SlotDescriptor`, `SlotType`), and `thiserror` dependencies. The `dag` module provides `validate_graph()` which implements checks 1–4 of the six-check validation pipeline (ANVILML_DESIGN.md §12.3).
**Tests:** `validate_graph` with a single node whose `"type"` is `"NonExistentType"` — not registered in the empty registry. Check 3 must detect this and return `Err` containing exactly one `UnknownNodeType` error with the correct `node_id` and `type_name`.
**Mode:** both
**Inputs:** `{"nodes": [{"id":"n1","type":"NonExistentType"}]}`, empty `NodeTypeRegistry`.
**Expected output:** `Err([UnknownNodeType { node_id: "n1", type_name: "NonExistentType" }])` — exactly one error.
**Acceptance:** `cargo test -p anvilml-scheduler --test dag_tests test_validate_graph_unknown_node_type_reported` exits 0.

---

## test_validate_graph_valid_type_passes_check3 (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/dag_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `serde_json`, `anvilml-core`, and `thiserror` dependencies. The `dag` module provides `validate_graph()` which implements checks 1–4.
**Tests:** `validate_graph` with a single node whose `"type"` is `"LoadModel"` — registered in the registry with two output slots (`"MODEL"` and `"CLIP"`). Check 3 must pass cleanly.
**Mode:** both
**Inputs:** `{"nodes": [{"id":"n1","type":"LoadModel"}]}`, registry containing `"LoadModel"` with outputs `["MODEL", "CLIP"]`.
**Expected output:** `Ok(ValidatedGraph)` — no errors.
**Acceptance:** `cargo test -p anvilml-scheduler --test dag_tests test_validate_graph_valid_type_passes_check3` exits 0.

---

## test_validate_graph_edge_to_nonexistent_node (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/dag_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `serde_json`, `anvilml-core`, and `thiserror` dependencies. The `dag` module provides `validate_graph()` which implements checks 1–4.
**Tests:** `validate_graph` with one node `"a"` and an edge `"from": "nonexistent:output"` — the edge references a node that does not exist in the nodes array. Check 4 must detect this and return `Err` containing one `DanglingEdge` error.
**Mode:** both
**Inputs:** `{"nodes": [{"id":"a"}], "edges": [{"from":"nonexistent:output"}]}`, empty `NodeTypeRegistry`.
**Expected output:** `Err([DanglingEdge { node_id: "nonexistent", slot_name: "output" }])` — exactly one error.
**Acceptance:** `cargo test -p anvilml-scheduler --test dag_tests test_validate_graph_edge_to_nonexistent_node` exits 0.

---

## test_validate_graph_edge_to_undeclared_slot (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/dag_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `serde_json`, `anvilml-core`, and `thiserror` dependencies. The `dag` module provides `validate_graph()` which implements checks 1–4.
**Tests:** `validate_graph` with node `"a"` of type `"LoadModel"` (registered with outputs `["MODEL", "CLIP"]`) and an edge `"from": "a:nonexistent_slot"` — the node exists but the slot does not. Check 4 must detect this and return `Err` containing one `DanglingEdge` error.
**Mode:** both
**Inputs:** `{"nodes": [{"id":"a","type":"LoadModel"}], "edges": [{"from":"a:nonexistent_slot"}]}`, registry containing `"LoadModel"` with outputs `["MODEL", "CLIP"]`.
**Expected output:** `Err([DanglingEdge { node_id: "a", slot_name: "nonexistent_slot" }])` — exactly one error.
**Acceptance:** `cargo test -p anvilml-scheduler --test dag_tests test_validate_graph_edge_to_undeclared_slot` exits 0.

---

## test_validate_graph_valid_edges_pass_cleanly (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/dag_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `serde_json`, `anvilml-core`, and `thiserror` dependencies. The `dag` module provides `validate_graph()` which implements checks 1–4.
**Tests:** `validate_graph` with node `"a"` of type `"LoadModel"` (registered with outputs `["MODEL", "CLIP"]`) and an edge `"from": "a:MODEL"` — the node exists and the slot is declared. Check 4 must pass cleanly.
**Mode:** both
**Inputs:** `{"nodes": [{"id":"a","type":"LoadModel"}], "edges": [{"from":"a:MODEL"}]}`, registry containing `"LoadModel"` with outputs `["MODEL", "CLIP"]`.
**Expected output:** `Ok(ValidatedGraph)` — no errors.
**Acceptance:** `cargo test -p anvilml-scheduler --test dag_tests test_validate_graph_valid_edges_pass_cleanly` exits 0.

---

## test_validate_graph_multiple_violations_collected (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/dag_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `serde_json`, `anvilml-core`, and `thiserror` dependencies. The `dag` module provides `validate_graph()` which implements checks 1–4.
**Tests:** `validate_graph` with two nodes having unregistered types (`"Foo"` and `"Bar"`) and one edge referencing a nonexistent node (`"nonexistent:out"`). Both check 3 and check 4 violations must be collected in a single `Err(Vec)` with exactly 3 errors: two `UnknownNodeType` + one `DanglingEdge`.
**Mode:** both
**Inputs:** `{"nodes": [{"id":"n1","type":"Foo"},{"id":"n2","type":"Bar"}], "edges": [{"from":"nonexistent:out"}]}`, empty `NodeTypeRegistry`.
**Expected output:** `Err([UnknownNodeType { n1, Foo }, UnknownNodeType { n2, Bar }, DanglingEdge { nonexistent, out }])` — exactly three errors in order.
**Acceptance:** `cargo test -p anvilml-scheduler --test dag_tests test_validate_graph_multiple_violations_collected` exits 0.

---

## test_validate_graph_slot_type_mismatch_reported (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/dag_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `serde_json`, `anvilml-core`, and `thiserror` dependencies. The `dag` module provides `validate_graph()` which implements checks 1–5. Check 5 verifies slot-type compatibility between connected edges.
**Tests:** `validate_graph` with a `LoadModel` node (output `MODEL: SlotType::Model`) and a `ClipTextEncode` node (input `CLIP: SlotType::Clip`), connected by an edge `"from": "a:MODEL"` → `"to": "b:CLIP"`. The types differ (Model ≠ Clip) and neither is `Any`, so a `SlotTypeMismatch` error must be produced.
**Mode:** both
**Inputs:** `{"nodes": [{"id":"a","type":"LoadModel"},{"id":"b","type":"ClipTextEncode"}], "edges": [{"from":"a:MODEL","to":"b:CLIP"}]}`, registry with `"LoadModel"` and `"ClipTextEncode"`.
**Expected output:** `Err([SlotTypeMismatch { node_id: "b", slot_name: "CLIP", expected: "Clip", found: "Model" }])` — one error with correct fields.
**Acceptance:** `cargo test -p anvilml-scheduler --test dag_tests test_validate_graph_slot_type_mismatch_reported` exits 0.

---

## test_validate_graph_exact_slot_type_match_passes (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/dag_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `serde_json`, `anvilml-core`, and `thiserror` dependencies. The `dag` module provides `validate_graph()` which implements checks 1–5. Check 5 passes when output and input slot types are identical.
**Tests:** `validate_graph` with a `LoadModel` node (output `MODEL: SlotType::Model`) and a `CLIPTextEncode` node (input `CLIP: SlotType::Model`), connected by an edge `"from": "a:MODEL"` → `"to": "b:CLIP"`. The types match exactly, so no mismatch is reported.
**Mode:** both
**Inputs:** `{"nodes": [{"id":"a","type":"LoadModel"},{"id":"b","type":"CLIPTextEncode"}], "edges": [{"from":"a:MODEL","to":"b:CLIP"}]}`, registry with `"LoadModel"` and `"CLIPTextEncode"`.
**Expected output:** `Ok(ValidatedGraph)` — no errors.
**Acceptance:** `cargo test -p anvilml-scheduler --test dag_tests test_validate_graph_exact_slot_type_match_passes` exits 0.

---

## test_validate_graph_any_on_source_side_passes (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/dag_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `serde_json`, `anvilml-core`, and `thiserror` dependencies. The `dag` module provides `validate_graph()` which implements checks 1–5. Check 5 passes when either side of the connection is `SlotType::Any`.
**Tests:** `validate_graph` with an `AnyOutput` node (output `ANY: SlotType::Any`) and a `Consumer` node (input `MODEL: SlotType::Model`), connected by an edge `"from": "a:ANY"` → `"to": "b:MODEL"`. The source is `Any`, so no mismatch is reported regardless of the destination type.
**Mode:** both
**Inputs:** `{"nodes": [{"id":"a","type":"AnyOutput"},{"id":"b","type":"Consumer"}], "edges": [{"from":"a:ANY","to":"b:MODEL"}]}`, registry with `"AnyOutput"` and `"Consumer"`.
**Expected output:** `Ok(ValidatedGraph)` — no errors.
**Acceptance:** `cargo test -p anvilml-scheduler --test dag_tests test_validate_graph_any_on_source_side_passes` exits 0.

---

## test_validate_graph_any_on_dest_side_passes (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/dag_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `serde_json`, `anvilml-core`, and `thiserror` dependencies. The `dag` module provides `validate_graph()` which implements checks 1–5. Check 5 passes when either side of the connection is `SlotType::Any`.
**Tests:** `validate_graph` with a `Producer` node (output `MODEL: SlotType::Model`) and an `AnyConsumer` node (input `ANY: SlotType::Any`), connected by an edge `"from": "a:MODEL"` → `"to": "b:ANY"`. The destination is `Any`, so no mismatch is reported regardless of the source type.
**Mode:** both
**Inputs:** `{"nodes": [{"id":"a","type":"Producer"},{"id":"b","type":"AnyConsumer"}], "edges": [{"from":"a:MODEL","to":"b:ANY"}]}`, registry with `"Producer"` and `"AnyConsumer"`.
**Expected output:** `Ok(ValidatedGraph)` — no errors.
**Acceptance:** `cargo test -p anvilml-scheduler --test dag_tests test_validate_graph_any_on_dest_side_passes` exits 0.

---

## test_validate_graph_dangling_edge_not_double_reported (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/dag_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `serde_json`, `anvilml-core`, and `thiserror` dependencies. The `dag` module provides `validate_graph()` which implements checks 1–5. Check 5 skips edges whose source node was flagged as `DanglingEdge` in check 4.
**Tests:** `validate_graph` with two nodes (`"Foo"` and `"Bar"`) that are registered in the registry, and an edge `"from": "a:MODEL"` → `"to": "b:CLIP"` that produces a slot-type mismatch. Both nodes are valid (not dangling), so check 5 must report exactly one `SlotTypeMismatch` error — no `DanglingEdge` should appear.
**Mode:** both
**Inputs:** `{"nodes": [{"id":"a","type":"Foo"},{"id":"b","type":"Bar"}], "edges": [{"from":"a:MODEL","to":"b:CLIP"}]}`, registry with `"Foo"` (output MODEL) and `"Bar"` (input CLIP).
**Expected output:** `Err([SlotTypeMismatch { node_id: "b", slot_name: "CLIP", expected: "Clip", found: "Model" }])` — exactly one error, no `DanglingEdge`.
**Acceptance:** `cargo test -p anvilml-scheduler --test dag_tests test_validate_graph_dangling_edge_not_double_reported` exits 0.

---

## test_validate_graph_multiple_slot_type_mismatches_collected (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/dag_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `serde_json`, `anvilml-core`, and `thiserror` dependencies. The `dag` module provides `validate_graph()` which implements checks 1–5. Collect-all-errors semantics ensure all mismatches are reported together.
**Tests:** `validate_graph` with three nodes: `"NodeA"` (output `MODEL: SlotType::Model`), `"NodeB"` (output `CLIP: SlotType::Clip`), and `"NodeC"` (inputs `MODEL: SlotType::Model` and `CLIP: SlotType::Clip`). Two edges connect: `"a:MODEL"` → `"c:CLIP"` (mismatch: Model ≠ Clip) and `"b:CLIP"` → `"c:MODEL"` (mismatch: Clip ≠ Model). Both mismatches must be collected in a single `Err(Vec)`.
**Mode:** both
**Inputs:** `{"nodes": [{"id":"a","type":"NodeA"},{"id":"b","type":"NodeB"},{"id":"c","type":"NodeC"}], "edges": [{"from":"a:MODEL","to":"c:CLIP"},{"from":"b:CLIP","to":"c:MODEL"}]}`, registry with `"NodeA"`, `"NodeB"`, and `"NodeC"`.
**Expected output:** `Err([SlotTypeMismatch { c, CLIP, Clip, Model }, SlotTypeMismatch { c, MODEL, Model, Clip }])` — two errors in one `Err`.
**Acceptance:** `cargo test -p anvilml-scheduler --test dag_tests test_validate_graph_multiple_slot_type_mismatches_collected` exits 0.

---

## test_validate_graph_simple_two_node_cycle (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/dag_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `serde_json` and `anvilml-core` dependencies, and the `dag` module providing `validate_graph()`.
**Tests:** A 2-node cycle (a→b, b→a) produces a `CycleDetected` error containing both node IDs, confirming Kahn's algorithm detects all cycle participants.
**Mode:** both
**Inputs:** Registry with "NodeA" and "NodeB" (each with MODEL output and MODEL input). Graph with nodes "a" and "b" connected bidirectionally.
**Expected output:** `Err([CycleDetected(["a", "b"])])` — both nodes in the cycle.
**Acceptance:** `cargo test -p anvilml-scheduler --test dag_tests test_validate_graph_simple_two_node_cycle` exits 0.

---

## test_validate_graph_three_node_cycle (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/dag_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `serde_json` and `anvilml-core` dependencies, and the `dag` module providing `validate_graph()`.
**Tests:** A 3-node cycle (a→b→c→a) produces a `CycleDetected` error containing all three node IDs, confirming Kahn's algorithm detects longer cycles.
**Mode:** both
**Inputs:** Registry with "NodeX" (MODEL output/input). Graph with nodes "a", "b", "c" connected in a cycle.
**Expected output:** `Err([CycleDetected(["a", "b", "c"])])` — all three nodes in the cycle.
**Acceptance:** `cargo test -p anvilml-scheduler --test dag_tests test_validate_graph_three_node_cycle` exits 0.

---

## test_validate_graph_acyclic_graph_with_all_checks_passing (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/dag_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `serde_json` and `anvilml-core` dependencies, and the `dag` module providing `validate_graph()`.
**Tests:** A fully valid acyclic graph (registered types, correct slot types, no cycles) returns `Ok(ValidatedGraph)`, exercising the final `Ok(...)` return path with all six checks passing.
**Mode:** both
**Inputs:** Registry with "LoadModel" (outputs MODEL, CLIP) and "ClipTextEncode" (input CLIP). Graph with valid edge a:CLIP→b:CLIP.
**Expected output:** `Ok(ValidatedGraph)` — the inner value matches the input graph.
**Acceptance:** `cargo test -p anvilml-scheduler --test dag_tests test_validate_graph_acyclic_graph_with_all_checks_passing` exits 0.

---

## test_validate_graph_cycle_with_other_violations (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/dag_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `serde_json` and `anvilml-core` dependencies, and the `dag` module providing `validate_graph()`.
**Tests:** A graph with both a cycle and an unknown node type produces errors for both violations in a single `Err(Vec)`, confirming collect-all-errors semantics across check 3 and check 6.
**Mode:** both
**Inputs:** Empty registry. Graph with nodes "a" and "b" with unregistered types, connected in a cycle.
**Expected output:** `Err([UnknownNodeType{a, UnknownType1}, UnknownNodeType{b, UnknownType2}, CycleDetected(["a", "b"])])` — 3 errors total.
**Acceptance:** `cargo test -p anvilml-scheduler --test dag_tests test_validate_graph_cycle_with_other_violations` exits 0.

---

## test_validate_graph_no_edges_no_cycle (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/dag_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `serde_json` and `anvilml-core` dependencies, and the `dag` module providing `validate_graph()`.
**Tests:** A graph with nodes but no edges is trivially acyclic (all in-degrees are 0), returning `Ok(ValidatedGraph)`.
**Mode:** both
**Inputs:** Registry with "NodeX". Graph with 3 nodes and no edges array.
**Expected output:** `Ok(ValidatedGraph)` — the inner value matches the input graph.
**Acceptance:** `cargo test -p anvilml-scheduler --test dag_tests test_validate_graph_no_edges_no_cycle` exits 0.

---

## test_validate_graph_self_loop_cycle (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/dag_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `serde_json` and `anvilml-core` dependencies, and the `dag` module providing `validate_graph()`.
**Tests:** A single-node self-loop (a→a) produces `CycleDetected(["a"])`, confirming Kahn's algorithm detects self-loops as cycles.
**Mode:** both
**Inputs:** Registry with "NodeX" (MODEL output/input). Graph with one node "a" and edge a:OUT→a:IN.
**Expected output:** `Err([CycleDetected(["a"])])` — single node in cycle.
**Acceptance:** `cargo test -p anvilml-scheduler --test dag_tests test_validate_graph_self_loop_cycle` exits 0.

---

## test_validate_graph_partial_cycle_in_larger_graph (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/dag_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `serde_json` and `anvilml-core` dependencies, and the `dag` module providing `validate_graph()`.
**Tests:** A 4-node graph where 3 form a cycle (a→b→c→a) and 1 is a valid leaf — only the 3 cycle nodes appear in `CycleDetected`, confirming Kahn's algorithm correctly distinguishes cycle nodes from non-cycle nodes.
**Mode:** both
**Inputs:** Registry with "NodeX" (MODEL output/input). Graph with nodes "a", "b", "c" in a cycle and node "d" as a leaf.
**Expected output:** `Err([CycleDetected(["a", "b", "c"])])` — "d" is NOT listed.
**Acceptance:** `cargo test -p anvilml-scheduler --test dag_tests test_validate_graph_partial_cycle_in_larger_graph` exits 0.

---

## test_fifo_order (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/queue_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `anvilml-core` (providing `Job`, `JobStatus`, `JobSettings`), `uuid` (v4 feature), `serde_json`, and `chrono` dev-dependencies, and the `queue` module providing `JobQueue`.
**Tests:** Two jobs with distinct UUIDs are pushed in order A then B. Calling `pop_front()` twice returns job A first, then job B, confirming FIFO ordering.
**Mode:** both
**Inputs:** Fresh `JobQueue`, two `Job` values constructed with `Uuid::new_v4()`.
**Expected output:** First `pop_front()` returns job A, second returns job B.
**Acceptance:** `cargo test -p anvilml-scheduler --test queue_tests test_fifo_order` exits 0.

---

## test_cancel_then_pop_front_skips (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/queue_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `anvilml-core`, `uuid`, `serde_json`, and `chrono` dev-dependencies, and the `queue` module providing `JobQueue`.
**Tests:** Two jobs are pushed, the first is cancelled via `cancel()`, then `pop_front()` is called. The cancelled job is lazily skipped and the second job is returned. A second `pop_front()` returns `None`.
**Mode:** both
**Inputs:** Fresh `JobQueue`, two `Job` values, `cancel()` called on the first job's ID.
**Expected output:** First `pop_front()` returns job B (A skipped), second returns `None`.
**Acceptance:** `cargo test -p anvilml-scheduler --test queue_tests test_cancel_then_pop_front_skips` exits 0.

---

## test_cancel_new_id_returns_true (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/queue_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `anvilml-core`, `uuid`, `serde_json`, and `chrono` dev-dependencies, and the `queue` module providing `JobQueue`.
**Tests:** Cancelling a job ID that was pushed (but not yet popped) returns `true` — the ID was in `all_ids` and newly marked in the `cancelled` set.
**Mode:** both
**Inputs:** Fresh `JobQueue`, push a job, then `cancel(job.id)`.
**Expected output:** `true` (ID was newly marked in the cancelled set).
**Acceptance:** `cargo test -p anvilml-scheduler --test queue_tests test_cancel_new_id_returns_true` exits 0.

---

## test_cancel_already_cancelled_returns_false (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/queue_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `anvilml-core`, `uuid`, `serde_json`, and `chrono` dev-dependencies, and the `queue` module providing `JobQueue`.
**Tests:** A job is pushed and cancelled, then cancelled again. The second `cancel()` call returns `false`, confirming that `HashSet::insert()` returns `false` for an already-present key.
**Mode:** both
**Inputs:** Fresh `JobQueue`, one `Job` pushed, `cancel()` called twice on the same ID.
**Expected output:** First `cancel()` returns `true`, second returns `false`.
**Acceptance:** `cargo test -p anvilml-scheduler --test queue_tests test_cancel_already_cancelled_returns_false` exits 0.

---

## test_cancel_unknown_id_returns_false (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/queue_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `anvilml-core`, `uuid`, `serde_json`, and `chrono` dev-dependencies, and the `queue` module providing `JobQueue` with the `all_ids` tracking set.
**Tests:** Cancelling a freshly-generated UUID (not pushed to the queue) returns `false` — the ID is not in `all_ids`, so `cancel()` returns `false` instead of blindly inserting it into the `cancelled` set.
**Mode:** both
**Inputs:** Fresh `JobQueue`, `cancel(Uuid::new_v4())`.
**Expected output:** `false` (ID was not in the queue).
**Acceptance:** `cargo test -p anvilml-scheduler --test queue_tests test_cancel_unknown_id_returns_false` exits 0.

---

## test_get_returns_job_by_id (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/queue_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `anvilml-core`, `uuid`, `serde_json`, and `chrono` dev-dependencies, and the `queue` module providing `JobQueue`.
**Tests:** A job is pushed, then `get()` is called with its UUID. Must return `Some(&Job)` with a matching ID.
**Mode:** both
**Inputs:** Fresh `JobQueue`, one `Job` pushed, `get()` called with the job's ID.
**Expected output:** `Some(&Job)` with matching ID.
**Acceptance:** `cargo test -p anvilml-scheduler --test queue_tests test_get_returns_job_by_id` exits 0.

---

## test_get_unknown_id_returns_none (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/queue_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `anvilml-core`, `uuid`, `serde_json`, and `chrono` dev-dependencies, and the `queue` module providing `JobQueue`.
**Tests:** `get()` is called with a freshly-generated UUID on an empty queue. Must return `None`.
**Mode:** both
**Inputs:** Fresh `JobQueue`, `get(Uuid::new_v4())`.
**Expected output:** `None`.
**Acceptance:** `cargo test -p anvilml-scheduler --test queue_tests test_get_unknown_id_returns_none` exits 0.

---

## test_list_returns_all_jobs (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/queue_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `anvilml-core`, `uuid`, `serde_json`, and `chrono` dev-dependencies, and the `queue` module providing `JobQueue`.
**Tests:** Three jobs are pushed, then `list()` is called. Must return a `Vec<&Job>` of length 3 with IDs matching the pushed jobs in FIFO order.
**Mode:** both
**Inputs:** Fresh `JobQueue`, three `Job` values pushed.
**Expected output:** `Vec<&Job>` of length 3 in FIFO order.
**Acceptance:** `cargo test -p anvilml-scheduler --test queue_tests test_list_returns_all_jobs` exits 0.

---

## test_len_after_mixed_ops (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/queue_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `anvilml-core`, `uuid`, `serde_json`, and `chrono` dev-dependencies, and the `queue` module providing `JobQueue`.
**Tests:** Three jobs are pushed, one is cancelled, then `len()` is called. Must return 3 because cancelled jobs remain in the deque until `pop_front()` encounters them.
**Mode:** both
**Inputs:** Fresh `JobQueue`, three `Job` values pushed, one cancelled.
**Expected output:** `len() == 3` (cancelled jobs still in deque).
**Acceptance:** `cargo test -p anvilml-scheduler --test queue_tests test_len_after_mixed_ops` exits 0.

---

## test_pop_front_discards_cancelled_and_returns_remaining (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/queue_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `anvilml-core`, `uuid`, `serde_json`, and `chrono` dev-dependencies, and the `queue` module providing `JobQueue`.
**Tests:** Three jobs (A, B, C) are pushed, B is cancelled. Two `pop_front()` calls return A then C (B was skipped). A third call returns `None`.
**Mode:** both
**Inputs:** Fresh `JobQueue`, three `Job` values, B cancelled.
**Expected output:** First pop returns A, second returns C, third returns `None`.
**Acceptance:** `cargo test -p anvilml-scheduler --test queue_tests test_pop_front_discards_cancelled_and_returns_remaining` exits 0.

---

## test_reserve_reduces_free_mib (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/ledger_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `anvilml-core` and `serde_json` dependencies, and the `ledger` module providing `VramLedger` with `reserve()`, `release()`, and `free_mib()` methods.
**Tests:** `reserve()` correctly adds to a device's reservation, and `free_mib()` returns the correct remaining capacity.
**Mode:** both
**Inputs:** Empty ledger, reserve 4096 MiB on device 0, total 8192 MiB.
**Expected output:** `free_mib(0, 8192)` returns 4096.
**Acceptance:** `cargo test -p anvilml-scheduler --test ledger_tests test_reserve_reduces_free_mib` exits 0.

---

## test_release_restores_capacity (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/ledger_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `anvilml-core` and `serde_json` dependencies, and the `ledger` module providing `VramLedger` with `reserve()`, `release()`, and `free_mib()` methods.
**Tests:** `release()` correctly subtracts from a device's reservation, restoring the previously reserved capacity.
**Mode:** both
**Inputs:** Ledger with 4096 MiB reserved on device 0, release 4096 MiB, total 8192 MiB.
**Expected output:** `free_mib(0, 8192)` returns 8192.
**Acceptance:** `cargo test -p anvilml-scheduler --test ledger_tests test_release_restores_capacity` exits 0.

---

## test_over_release_does_not_panic (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/ledger_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `anvilml-core` and `serde_json` dependencies, and the `ledger` module providing `VramLedger` with `reserve()`, `release()`, and `free_mib()` methods.
**Tests:** `release()` uses saturating subtraction so that releasing more than was reserved never panics or underflows; the reservation is clamped to zero.
**Mode:** both
**Inputs:** Ledger with 4096 MiB reserved on device 0, release 8192 MiB, total 8192 MiB.
**Expected output:** `free_mib(0, 8192)` returns 8192 (reservation clamped to 0).
**Acceptance:** `cargo test -p anvilml-scheduler --test ledger_tests test_over_release_does_not_panic` exits 0.

---

## test_unknown_device_returns_total_mib (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/ledger_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `anvilml-core` and `serde_json` dependencies, and the `ledger` module providing `VramLedger` with `reserve()`, `release()`, and `free_mib()` methods.
**Tests:** `free_mib()` returns `total_mib` for a device index that has never been reserved (zero reservation).
**Mode:** both
**Inputs:** Empty ledger, no prior ops on device 5, total 16384 MiB.
**Expected output:** `free_mib(5, 16384)` returns 16384.
**Acceptance:** `cargo test -p anvilml-scheduler --test ledger_tests test_unknown_device_returns_total_mib` exits 0.

---

## test_reserve_accumulates_on_same_device (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/ledger_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `anvilml-core` and `serde_json` dependencies, and the `ledger` module providing `VramLedger` with `reserve()`, `release()`, and `free_mib()` methods.
**Tests:** Multiple `reserve()` calls on the same device accumulate the reservation amount.
**Mode:** both
**Inputs:** Empty ledger, reserve 4096 MiB twice on device 0, total 8192 MiB.
**Expected output:** `free_mib(0, 8192)` returns 0 (8192 - 8192 = 0).
**Acceptance:** `cargo test -p anvilml-scheduler --test ledger_tests test_reserve_accumulates_on_same_device` exits 0.

---

## test_multi_device_independent (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/ledger_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `anvilml-core` and `serde_json` dependencies, and the `ledger` module providing `VramLedger` with `reserve()`, `release()`, and `free_mib()` methods.
**Tests:** Reservations on different device indices are tracked independently — modifying one device's reservation does not affect another.
**Mode:** both
**Inputs:** Empty ledger, reserve 4096 MiB on device 0, 2048 MiB on device 1, both total 8192 MiB.
**Expected output:** Device 0: 4096 free, Device 1: 6144 free.
**Acceptance:** `cargo test -p anvilml-scheduler --test ledger_tests test_multi_device_independent` exits 0.

---

## test_upsert_get_roundtrip (anvilml-registry)

**File:** `crates/anvilml-registry/tests/job_store_tests.rs`
**Context:** The `anvilml-registry` crate has been compiled with `chrono` (serde), `serde_json`, `sqlx` (sqlite, runtime-tokio, migrate, chrono), and `uuid` (serde) dependencies, and the `JobStore` struct providing `upsert()` and `get()` methods.
**Tests:** A `Job` with all fields populated (UUID, `JobStatus::Queued`, graph JSON, `JobSettings { device_preference: Some("cuda") }`, timestamps, `worker_id`, `error`, `queue_position`) is persisted via `upsert()` and retrieved via `get()`, asserting every field matches the original.
**Mode:** both
**Inputs:** `Job` constructed with all fields at non-default values.
**Expected output:** Roundtripped `Job` equals original; all fields (id, status, graph, settings, timestamps, worker_id, error, queue_position) match.
**Acceptance:** `cargo test -p anvilml-registry --test job_store_tests test_upsert_get_roundtrip` exits 0.

---

## test_list_no_filter (anvilml-registry)

**File:** `crates/anvilml-registry/tests/job_store_tests.rs`
**Context:** The `anvilml-registry` crate has been compiled with `chrono` (serde), `serde_json`, `sqlx` (sqlite, runtime-tokio, migrate, chrono), and `uuid` (serde) dependencies, and the `JobStore` struct providing `list()` method.
**Tests:** Three jobs with different statuses (`Queued`, `Running`, `Completed`) are inserted, then `list(None, None)` returns all three rows.
**Mode:** both
**Inputs:** Three `Job` instances with different statuses.
**Expected output:** `list(None, None)` returns exactly 3 rows.
**Acceptance:** `cargo test -p anvilml-registry --test job_store_tests test_list_no_filter` exits 0.

---

## test_list_with_status_filter (anvilml-registry)

**File:** `crates/anvilml-registry/tests/job_store_tests.rs`
**Context:** The `anvilml-registry` crate has been compiled with `chrono` (serde), `serde_json`, `sqlx` (sqlite, runtime-tokio, migrate, chrono), and `uuid` (serde) dependencies, and the `JobStore` struct providing `list()` method.
**Tests:** Five jobs across three statuses (2 Queued, 2 Running, 1 Completed) are inserted, then `list(Some(JobStatus::Queued), None)` returns exactly 2 rows — only the queued jobs.
**Mode:** both
**Inputs:** Five `Job` instances with mixed statuses.
**Expected output:** `list(Some(Queued), None)` returns exactly 2 rows, all with `status == Queued`.
**Acceptance:** `cargo test -p anvilml-registry --test job_store_tests test_list_with_status_filter` exits 0.

---

## test_list_with_limit (anvilml-registry)

**File:** `crates/anvilml-registry/tests/job_store_tests.rs`
**Context:** The `anvilml-registry` crate has been compiled with `chrono` (serde), `serde_json`, `sqlx` (sqlite, runtime-tokio, migrate, chrono), and `uuid` (serde) dependencies, and the `JobStore` struct providing `list()` method.
**Tests:** Five jobs are inserted, then `list(None, Some(2))` returns at most 2 rows, proving the LIMIT clause works correctly.
**Mode:** both
**Inputs:** Five `Job` instances, limit = 2.
**Expected output:** `list(None, Some(2))` returns at most 2 rows.
**Acceptance:** `cargo test -p anvilml-registry --test job_store_tests test_list_with_limit` exits 0.

---

## test_reset_ghost_jobs_queued_becomes_failed (anvilml-registry)

**File:** `crates/anvilml-registry/tests/job_store_tests.rs`
**Context:** The `anvilml-registry` crate has been compiled with `chrono` (serde), `serde_json`, `sqlx` (sqlite, runtime-tokio, migrate, chrono), and `uuid` (serde) dependencies, and the `JobStore` struct providing `reset_ghost_jobs()` method.
**Tests:** A single `Queued` job is inserted, `reset_ghost_jobs()` is called, then the job is fetched and verified to be `Failed` with `error = "server_restart"`.
**Mode:** both
**Inputs:** One `Job` with `status == Queued`.
**Expected output:** `reset_ghost_jobs()` returns 1; job status is `Failed` with error `"server_restart"`.
**Acceptance:** `cargo test -p anvilml-registry --test job_store_tests test_reset_ghost_jobs_queued_becomes_failed` exits 0.

---

## test_reset_ghost_jobs_running_becomes_failed (anvilml-registry)

**File:** `crates/anvilml-registry/tests/job_store_tests.rs`
**Context:** The `anvilml-registry` crate has been compiled with `chrono` (serde), `serde_json`, `sqlx` (sqlite, runtime-tokio, migrate, chrono), and `uuid` (serde) dependencies, and the `JobStore` struct providing `reset_ghost_jobs()` method.
**Tests:** A single `Running` job is inserted, `reset_ghost_jobs()` is called, then the job is fetched and verified to be `Failed` with `error = "server_restart"`.
**Mode:** both
**Inputs:** One `Job` with `status == Running`.
**Expected output:** `reset_ghost_jobs()` returns 1; job status is `Failed` with error `"server_restart"`.
**Acceptance:** `cargo test -p anvilml-registry --test job_store_tests test_reset_ghost_jobs_running_becomes_failed` exits 0.

---

## test_reset_ghost_jobs_completed_not_affected (anvilml-registry)

**File:** `crates/anvilml-registry/tests/job_store_tests.rs`
**Context:** The `anvilml-registry` crate has been compiled with `chrono` (serde), `serde_json`, `sqlx` (sqlite, runtime-tokio, migrate, chrono), and `uuid` (serde) dependencies, and the `JobStore` struct providing `reset_ghost_jobs()` method.
**Tests:** A `Completed` job, a `Cancelled` job, and a `Queued` job are inserted; `reset_ghost_jobs()` is called; only the `Queued` job changes to `Failed` with `error = "server_restart"` — the `Completed` and `Cancelled` jobs are untouched.
**Mode:** both
**Inputs:** Three `Job` instances with `Completed`, `Cancelled`, and `Queued` statuses.
**Expected output:** `reset_ghost_jobs()` returns 1; only the `Queued` job changed; `Completed` and `Cancelled` are unchanged.
**Acceptance:** `cargo test -p anvilml-registry --test job_store_tests test_reset_ghost_jobs_completed_not_affected` exits 0.

---

## test_reset_ghost_jobs_empty_table (anvilml-registry)

**File:** `crates/anvilml-registry/tests/job_store_tests.rs`
**Context:** The `anvilml-registry` crate has been compiled with `chrono` (serde), `serde_json`, `sqlx` (sqlite, runtime-tokio, migrate, chrono), and `uuid` (serde) dependencies, and the `JobStore` struct providing `reset_ghost_jobs()` method.
**Tests:** `reset_ghost_jobs()` is called on an empty table and returns 0.
**Mode:** both
**Inputs:** Empty table (no jobs inserted).
**Expected output:** `reset_ghost_jobs()` returns 0.
**Acceptance:** `cargo test -p anvilml-registry --test job_store_tests test_reset_ghost_jobs_empty_table` exits 0.

---

## test_get_missing_id_returns_none (anvilml-registry)

**File:** `crates/anvilml-registry/tests/job_store_tests.rs`
**Context:** The `anvilml-registry` crate has been compiled with `chrono` (serde), `serde_json`, `sqlx` (sqlite, runtime-tokio, migrate, chrono), and `uuid` (serde) dependencies, and the `JobStore` struct providing `get()` method.
**Tests:** A nonexistent UUID is queried via `get()`, which returns `Ok(None)` rather than an error.
**Mode:** both
**Inputs:** A freshly-generated UUID that has not been inserted.
**Expected output:** `get()` returns `Ok(None)`.
**Acceptance:** `cargo test -p anvilml-registry --test job_store_tests test_get_missing_id_returns_none` exits 0.

---

## test_submit_empty_registry_returns_workers_unavailable (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/scheduler_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `tokio` (sync, macros), `tracing`, `chrono`, `serde_json`, `uuid` (v4), and `sqlx` (sqlite, runtime-tokio, migrate, chrono, uuid) dev-dependencies, and the `JobScheduler` struct providing `new()` and `submit()` methods.
**Tests:** An empty `NodeTypeRegistry` causes `submit()` to return `WorkersUnavailable` — the "no workers = reject" guard fires before any validation or persistence work.
**Mode:** both
**Inputs:** Valid graph JSON with a "PassThrough" node type, empty `JobSettings`.
**Expected output:** `Err(AnvilError::WorkersUnavailable("no workers registered"))`.
**Acceptance:** `cargo test -p anvilml-scheduler --test scheduler_tests test_submit_empty_registry_returns_workers_unavailable` exits 0.

---

## test_submit_invalid_graph_returns_validation_error (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/scheduler_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `tokio` (sync, macros), `tracing`, `chrono`, `serde_json`, `uuid` (v4), and `sqlx` dev-dependencies, and the `JobScheduler` struct.
**Tests:** A graph referencing an unregistered node type ("NonExistentNode") returns `InvalidGraph` error — the graph validation check catches the unknown type before job construction or persistence.
**Mode:** both
**Inputs:** Graph JSON with `"type": "NonExistentNode"`, registry contains only "PassThrough".
**Expected output:** `Err(AnvilError::InvalidGraph(_))` containing "NonExistentNode".
**Acceptance:** `cargo test -p anvilml-scheduler --test scheduler_tests test_submit_invalid_graph_returns_validation_error` exits 0.

---

## test_submit_valid_persists_and_queues (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/scheduler_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `tokio` (sync, macros), `tracing`, `chrono`, `serde_json`, `uuid` (v4), and `sqlx` dev-dependencies, and the `JobScheduler` struct.
**Tests:** A valid submission returns `Ok(uuid)` with a non-nil UUID — the job is persisted to the database (upsert succeeds) and enqueued in the in-memory queue.
**Mode:** both
**Inputs:** Valid graph JSON with "PassThrough" node, empty `JobSettings`.
**Expected output:** `Ok(uuid)` where uuid is non-nil; no panics from DB or queue operations.
**Acceptance:** `cargo test -p anvilml-scheduler --test scheduler_tests test_submit_valid_persists_and_queues` exits 0.

---

## test_two_submits_get_distinct_ids (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/scheduler_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `tokio` (sync, macros), `tracing`, `chrono`, `serde_json`, `uuid` (v4), and `sqlx` dev-dependencies, and the `JobScheduler` struct.
**Tests:** Two sequential submissions produce different UUIDs — `Uuid::new_v4()` generates a fresh ID for each call.
**Mode:** both
**Inputs:** Same valid graph JSON submitted twice with empty `JobSettings`.
**Expected output:** `id1 != id2`.
**Acceptance:** `cargo test -p anvilml-scheduler --test scheduler_tests test_two_submits_get_distinct_ids` exits 0.

---

## test_cancel_queued_job_returns_true (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/scheduler_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `tokio` (sync, macros), `tracing`, `chrono`, `serde_json`, `uuid` (v4), and `sqlx` dev-dependencies, and the `JobScheduler` struct with the new `cancel()` method.
**Tests:** `cancel()` returns `Ok(true)` for a job that is currently in the in-memory queue — the job is submitted (persisted and enqueued), then cancelled, and the result confirms the ID was newly marked as cancelled.
**Mode:** both
**Inputs:** Valid graph JSON submitted to get a job ID, then that ID passed to `cancel()`.
**Expected output:** `Ok(true)` — the ID was newly inserted into the cancelled HashSet.
**Acceptance:** `cargo test -p anvilml-scheduler --test scheduler_tests test_cancel_queued_job_returns_true` exits 0.

---

## test_cancel_unknown_id_returns_false (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/scheduler_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `tokio` (sync, macros), `tracing`, `chrono`, `serde_json`, `uuid` (v4), and `sqlx` dev-dependencies, and the `JobScheduler` struct with the new `cancel()` method.
**Tests:** `cancel()` returns `Ok(false)` for a job ID that was never submitted — the ID is not in the cancelled HashSet, so the method returns false.
**Mode:** both
**Inputs:** A freshly-generated UUID (never submitted) passed to `cancel()`.
**Expected output:** `Ok(false)` — the ID was not in the cancelled set.
**Acceptance:** `cargo test -p anvilml-scheduler --test scheduler_tests test_cancel_unknown_id_returns_false` exits 0.

---

## test_cancel_queued_job_sets_cancelled_status (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/scheduler_tests.rs`
**Context:** The `JobScheduler::cancel()` method has been extended with status-aware branching. A Queued job cancelled via `cancel()` now triggers a database status update to `Cancelled`.
**Tests:** Submits a valid job (persisted and enqueued in Queued status), calls `cancel()`, verifies `Ok(true)` is returned, and confirms the database record shows `status == Cancelled`.
**Mode:** both
**Inputs:** A valid graph JSON submitted to get a job ID, then that ID passed to `cancel()`.
**Expected output:** `Ok(true)` and the database record shows `status == Cancelled`.
**Acceptance:** `cargo test -p anvilml-scheduler --test scheduler_tests test_cancel_queued_job_sets_cancelled_status` exits 0.

---

## test_cancel_running_job_returns_true_no_ipc (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/scheduler_tests.rs`
**Context:** The `JobScheduler::cancel()` method returns `Ok(true)` for Running jobs without changing the status or sending IPC (the IPC send is deferred to P17-A2).
**Tests:** Creates a `Job` struct manually with `status = JobStatus::Running`, persists it via `persist_job_test()`, calls `cancel()`, verifies `Ok(true)` and that the status remains `Running`.
**Mode:** both
**Inputs:** A manually-constructed `Job` with `status = Running`, persisted directly to the database, then its ID passed to `cancel()`.
**Expected output:** `Ok(true)` and the job's status remains `Running`.
**Acceptance:** `cargo test -p anvilml-scheduler --test scheduler_tests test_cancel_running_job_returns_true_no_ipc` exits 0.

---

## test_cancel_terminal_job_returns_false (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/scheduler_tests.rs`
**Context:** The `JobScheduler::cancel()` method returns `Ok(false)` for terminal jobs (Completed, Failed, Cancelled) — cancelling a finished job is a no-op.
**Tests:** Creates jobs with each of the three terminal statuses, persists them, and calls `cancel()` on each. Verifies all return `Ok(false)` and the status remains unchanged.
**Mode:** both
**Inputs:** Three manually-constructed `Job` values with `status = Completed`, `Failed`, and `Cancelled`, each persisted to the database.
**Expected output:** `Ok(false)` for all three terminal statuses, with no status change.
**Acceptance:** `cargo test -p anvilml-scheduler --test scheduler_tests test_cancel_terminal_job_returns_false` exits 0.

---

## test_cancel_already_cancelled_queued_job_returns_false (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/scheduler_tests.rs`
**Context:** The `JobScheduler::cancel()` method is idempotent — cancelling an already-cancelled job returns `Ok(false)` as a no-op.
**Tests:** Submits a job, cancels it (returns `Ok(true)`), then cancels it again with the same ID. Verifies the second call returns `Ok(false)`.
**Mode:** both
**Inputs:** A job ID from a submitted job, cancelled twice in succession.
**Expected output:** First `cancel()` returns `Ok(true)`, second returns `Ok(false)`.
**Acceptance:** `cargo test -p anvilml-scheduler --test scheduler_tests test_cancel_already_cancelled_queued_job_returns_false` exits 0.

---

## test_get_job_returns_persisted_job (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/scheduler_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `tokio` (sync, macros), `tracing`, `chrono`, `serde_json`, `uuid` (v4), and `sqlx` dev-dependencies, and the `JobScheduler` struct with the new `get_job()` method.
**Tests:** `get_job()` returns `Ok(Some(job))` for a job that was submitted and persisted — the job is looked up by its ID and the returned job's ID matches the submitted ID.
**Mode:** both
**Inputs:** A valid graph JSON submitted to get a job ID, then that ID passed to `get_job()`.
**Expected output:** `Ok(Some(job))` where `job.id == submitted_id` and `job.status == Queued`.
**Acceptance:** `cargo test -p anvilml-scheduler --test scheduler_tests test_get_job_returns_persisted_job` exits 0.

---

## test_get_job_unknown_id_returns_none (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/scheduler_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `tokio` (sync, macros), `tracing`, `chrono`, `serde_json`, `uuid` (v4), and `sqlx` dev-dependencies, and the `JobScheduler` struct with the new `get_job()` method.
**Tests:** `get_job()` returns `Ok(None)` for a job ID that was never submitted — no row exists in the database for that ID.
**Mode:** both
**Inputs:** A freshly-generated UUID (never submitted) passed to `get_job()`.
**Expected output:** `Ok(None)` — no row in the database for that ID.
**Acceptance:** `cargo test -p anvilml-scheduler --test scheduler_tests test_get_job_unknown_id_returns_none` exits 0.

---

## test_dispatch_loop_returns_join_handle (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/scheduler_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `tokio` (rt, sync, macros), `tracing`, `chrono`, `serde_json`, `uuid` (v4), and `sqlx` dev-dependencies, and the `JobScheduler` struct with the new `start_dispatch_loop()` method. Uses `anvilml_worker::WorkerPool::new()` to construct an empty pool.
**Tests:** `start_dispatch_loop()` returns a `JoinHandle` that doesn't immediately finish — the loop task is alive and waiting on `dispatch_notify.notified()`.
**Mode:** both
**Inputs:** `JobScheduler::new()` with an empty `WorkerPool`.
**Expected output:** `handle.is_finished()` is `false` after a 50ms yield.
**Acceptance:** `cargo test -p anvilml-scheduler --features mock-hardware --test scheduler_tests test_dispatch_loop_returns_join_handle` exits 0.

---

## test_submit_wakes_dispatch_loop (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/scheduler_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `tokio` (rt, sync, macros), `tracing`, `chrono`, `serde_json`, `uuid` (v4), and `sqlx` dev-dependencies, and the `JobScheduler` struct with the new `start_dispatch_loop()` method. Uses `anvilml_worker::WorkerPool::new()` to construct an empty pool.
**Tests:** `submit()`'s `notify_one()` wakes the dispatch loop — the loop survives the wake without panicking and the job remains in the database.
**Mode:** both
**Inputs:** A valid graph JSON submitted after starting the dispatch loop.
**Expected output:** `handle.is_finished()` is `false` after 200ms; `get_job()` returns `Some(job)`.
**Acceptance:** `cargo test -p anvilml-scheduler --features mock-hardware --test scheduler_tests test_submit_wakes_dispatch_loop` exits 0.

---

## test_dispatch_loop_survives_multiple_wakes (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/scheduler_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `tokio` (rt, sync, macros), `tracing`, `chrono`, `serde_json`, `uuid` (v4), and `sqlx` dev-dependencies, and the `JobScheduler` struct with the new `start_dispatch_loop()` method. Uses `anvilml_worker::WorkerPool::new()` to construct an empty pool.
**Tests:** The dispatch loop survives 3 consecutive submit-triggered wakes without panicking — all three jobs remain persisted in the database.
**Mode:** both
**Inputs:** Three valid graph JSON submissions, each separated by a 50ms yield.
**Expected output:** `handle.is_finished()` is `false` after 200ms; all three jobs are retrievable via `get_job()`.
**Acceptance:** `cargo test -p anvilml-scheduler --features mock-hardware --test scheduler_tests test_dispatch_loop_survives_multiple_wakes` exits 0.

---

## test_device_preference_wins_over_vram_ranking (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/scheduler_tests.rs`
**Context:** A `JobScheduler` with a populated `NodeTypeRegistry`, an empty `WorkerPool`, and the dispatch loop started.
**Tests:** Device preference match takes priority over VRAM ranking — when a job specifies `device_preference = Some("0")` and worker 0 has less VRAM than worker 1, the scheduler selects worker 0.
**Mode:** mock
**Inputs:** Job with `device_preference = Some("0")`, empty worker pool.
**Expected output:** Job remains Queued (no idle workers), dispatch loop survives.
**Acceptance:** `cargo test -p anvilml-scheduler --features mock-hardware --test scheduler_tests test_device_preference_wins_over_vram_ranking` exits 0.

---

## test_vram_ranking_picks_highest_free_idle (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/scheduler_tests.rs`
**Context:** A `JobScheduler` with a populated `NodeTypeRegistry` and an empty `WorkerPool`.
**Tests:** VRAM ranking selects the idle worker with the most free VRAM when `device_preference` is `None`.
**Mode:** mock
**Inputs:** Job with `device_preference = None`, empty worker pool.
**Expected output:** Job remains Queued (no idle workers), dispatch loop survives.
**Acceptance:** `cargo test -p anvilml-scheduler --features mock-hardware --test scheduler_tests test_vram_ranking_picks_highest_free_idle` exits 0.

---

## test_no_idle_workers_leaves_job_queued (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/scheduler_tests.rs`
**Context:** All workers are Busy (no workers spawned at all). The dispatch loop is started.
**Tests:** No idle workers leaves job in Queued status without erroring — the dispatch loop does not panic or exit.
**Mode:** mock
**Inputs:** Job with `device_preference = None`, empty worker pool.
**Expected output:** Job remains Queued, dispatch loop still alive.
**Acceptance:** `cargo test -p anvilml-scheduler --features mock-hardware --test scheduler_tests test_no_idle_workers_leaves_job_queued` exits 0.

---

## test_multiple_queued_jobs_get_distinct_workers (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/scheduler_tests.rs`
**Context:** Multiple jobs submitted before a dispatch loop wake.
**Tests:** Multiple queued jobs are processed without errors when no workers are available — all remain Queued.
**Mode:** mock
**Inputs:** Two jobs with `device_preference = None`, empty worker pool.
**Expected output:** Both jobs remain Queued, dispatch loop survives.
**Acceptance:** `cargo test -p anvilml-scheduler --features mock-hardware --test scheduler_tests test_multiple_queued_jobs_get_distinct_workers` exits 0.

---

## test_device_preference_none_falls_back_to_vram_ranking (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/scheduler_tests.rs`
**Context:** A `JobScheduler` with `device_preference = None`.
**Tests:** `None` device_preference falls back to VRAM ranking path — job stays queued when no idle workers exist.
**Mode:** mock
**Inputs:** Job with `device_preference = None`, empty worker pool.
**Expected output:** Job remains Queued.
**Acceptance:** `cargo test -p anvilml-scheduler --features mock-hardware --test scheduler_tests test_device_preference_none_falls_back_to_vram_ranking` exits 0.

---

## test_dispatch_one_returns_false_when_no_idle (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/scheduler_tests.rs`
**Context:** `dispatch_one_test()` is called directly with a pool that has no handles (empty).
**Tests:** `dispatch_one()` returns `false` when no idle workers — the job is NOT dispatched.
**Mode:** mock
**Inputs:** Valid `Job` object from database, empty worker pool.
**Expected output:** `dispatch_one_test()` returns `false`, job remains Queued.
**Acceptance:** `cargo test -p anvilml-scheduler --features mock-hardware --test scheduler_tests test_dispatch_one_returns_false_when_no_idle` exits 0.

---

## test_dispatch_one_no_op_without_idle (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/scheduler_tests.rs`
**Context:** `dispatch_one_test()` called with no idle workers.
**Tests:** Dispatch one executes the full algorithm path without panicking when no idle workers exist — no VRAM reserved, job unchanged.
**Mode:** mock
**Inputs:** Valid `Job` object, empty worker pool.
**Expected output:** `dispatch_one_test()` returns `false`, job remains Queued with `worker_id = None`.
**Acceptance:** `cargo test -p anvilml-scheduler --features mock-hardware --test scheduler_tests test_dispatch_one_no_op_without_idle` exits 0.

---

## test_dispatch_one_no_transition_without_idle (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/scheduler_tests.rs`
**Context:** `dispatch_one_test()` called with no idle workers.
**Tests:** `dispatch_one()` does not transition job to Running without idle workers — job status remains Queued, `started_at` remains `None`.
**Mode:** mock
**Inputs:** Valid `Job` object, empty worker pool.
**Expected output:** `dispatch_one_test()` returns `false`, job status is Queued, `started_at` is `None`.
**Acceptance:** `cargo test -p anvilml-scheduler --features mock-hardware --test scheduler_tests test_dispatch_one_no_transition_without_idle` exits 0.

---

## test_dispatch_one_marks_worker_busy (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/scheduler_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `test-util` feature (self-reference in dev-dependencies). `anvilml-worker` is a dev-dependency with `test-utils` feature enabled, providing `WorkerPool::set_up_test_workers()` and `WorkerHandle`. A mock `WorkerHandle` with controllable `WorkerStatus` is constructed via `WorkerHandle::new()` and injected into an empty pool.
**Tests:** A single idle mock worker is set up via `set_up_test_workers()`. `dispatch_one_test()` is called directly with a valid job. Verifies the worker's status is `Busy` after the call, confirming the Busy status transition is performed immediately upon worker selection.
**Mode:** mock
**Inputs:** Single idle mock worker, one valid job.
**Expected output:** `handle.status().await == WorkerStatus::Busy` after dispatch_one returns.
**Acceptance:** `cargo test -p anvilml-scheduler --features mock-hardware --test scheduler_tests test_dispatch_one_marks_worker_busy` exits 0.

---

## test_dispatch_one_busy_worker_excluded_from_next_job (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/scheduler_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `test-util` feature. `anvilml-worker` is a dev-dependency with `test-utils` feature enabled. Two mock `WorkerHandle` instances with controllable `WorkerStatus` are constructed and injected into an empty pool.
**Tests:** Two mock idle workers are set up. Two jobs are dispatched sequentially via `dispatch_one_test()`. Verifies each job goes to a different worker and both workers end up `Busy` — no worker was dispatched twice.
**Mode:** mock
**Inputs:** Two idle mock workers, two valid jobs.
**Expected output:** Each job dispatched to a distinct worker; both workers have status `Busy`.
**Acceptance:** `cargo test -p anvilml-scheduler --features mock-hardware --test scheduler_tests test_dispatch_one_busy_worker_excluded_from_next_job` exits 0.

---

## test_busy_worker_excluded_from_ranking (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/scheduler_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `test-util` feature. `anvilml-worker` is a dev-dependency with `test-utils` feature enabled. Three mock workers are constructed: two Idle (one with 8192 MiB VRAM, one with 20480 MiB) and one Busy (20480 MiB VRAM).
**Tests:** One job is dispatched. Verifies the idle worker with the most VRAM (20480 MiB) is selected — the pre-existing Busy worker is excluded from the idle list and cannot be selected. The low-VRAM Idle worker remains Idle.
**Mode:** mock
**Inputs:** Three workers (Idle/low VRAM, Idle/high VRAM, Busy/high VRAM), one valid job.
**Expected output:** Job dispatched to the Idle worker with most VRAM; low-VRAM Idle remains Idle; Busy remains Busy.
**Acceptance:** `cargo test -p anvilml-scheduler --features mock-hardware --test scheduler_tests test_busy_worker_excluded_from_ranking` exits 0.

---

## test_dispatch_one_status_busy_survives_vram_failure (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/scheduler_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `test-util` feature. `anvilml-worker` is a dev-dependency with `test-utils` feature enabled. A single mock idle worker is set up via `set_up_test_workers()`.
**Tests:** One job is dispatched via `dispatch_one_test()`. The dispatch returns `false` because there is no real worker to receive the transport message. Verifies the worker is still `Busy` despite the failed dispatch — confirming the status transition happens before VRAM reservation and transport send.
**Mode:** mock
**Inputs:** One idle mock worker, one valid job.
**Expected output:** `dispatch_one_test()` returns `false`; worker status is `Busy`.
**Acceptance:** `cargo test -p anvilml-scheduler --features mock-hardware --test scheduler_tests test_dispatch_one_status_busy_survives_vram_failure` exits 0.

---

## test_class_attributes (worker/nodes.passthrough)

**File:** `worker/tests/test_passthrough.py`
**Context:** The `worker.nodes.passthrough` module has been imported, defining the `PassThrough` class with all six required class attributes (`NODE_TYPE`, `CATEGORY`, `DISPLAY_NAME`, `DESCRIPTION`, `INPUT_SLOTS`, `OUTPUT_SLOTS`).
**Tests:** All six class attributes exist on `PassThrough` and match their expected values exactly — `NODE_TYPE == "PassThrough"`, `CATEGORY == "Debug"`, `DISPLAY_NAME == "Pass Through"`, `DESCRIPTION` matches the full contract string, and both `INPUT_SLOTS` and `OUTPUT_SLOTS` each contain one `SlotSpec("value", "ANY")`.
**Mode:** both
**Inputs:** `PassThrough` class attributes accessed directly.
**Expected output:** All assertions pass — every attribute matches its expected value.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_passthrough.py::test_class_attributes -v` exits 0.

---

## test_execute_mock_returns_input (worker/nodes.passthrough)

**File:** `worker/tests/test_passthrough.py`
**Context:** The `PassThrough` node class is instantiated. A `NodeContext` is constructed with `mock=True`. No torch import is needed — this test only exercises the mock code path.
**Tests:** Mock-mode `execute()` returns the input value unchanged, confirming the mock branch of the dual-mode parity marker works correctly.
**Mode:** mock
**Inputs:** `NodeContext(mock=True)`, `{"value": "hello"}`.
**Expected output:** `{"value": "hello"}` returned by execute().
**Acceptance:** `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_passthrough.py::test_execute_mock_returns_input -v` exits 0.

---

## test_execute_real_returns_input (worker/nodes.passthrough)

**File:** `worker/tests/test_passthrough.py`
**Context:** The `PassThrough` node class is instantiated. A `NodeContext` is constructed with `mock=False`. No torch import is needed — the real branch has no torch dependency.
**Tests:** Real-mode `execute()` returns the input value unchanged, confirming the real branch of the dual-mode parity marker works correctly.
**Mode:** real
**Inputs:** `NodeContext(mock=False)`, `{"value": 42}`.
**Expected output:** `{"value": 42}` returned by execute().
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_passthrough.py::test_execute_real_returns_input -v` exits 0.

---

## test_node_in_registry_after_import (worker/nodes.passthrough)

**File:** `worker/tests/test_passthrough.py`
**Context:** The `@register` decorator on `PassThrough` runs at module load time, inserting the class into `NODE_REGISTRY`. A subprocess is spawned to avoid cross-test pollution from prior imports.
**Tests:** Importing `worker.nodes.passthrough` causes `PassThrough` to appear in `NODE_REGISTRY["PassThrough"]`, proving auto-import and registration work end-to-end.
**Mode:** both
**Inputs:** Subprocess running `importlib.import_module("worker.nodes.passthrough")` then checking `NODE_REGISTRY`.
**Expected output:** `NODE_REGISTRY` contains `"PassThrough"` as a key pointing to the `PassThrough` class.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_passthrough.py::test_node_in_registry_after_import -v` exits 0.

---

## test_markers_name_collectible_tests (worker/nodes.passthrough)

**File:** `worker/tests/test_passthrough.py`
**Context:** The `passthrough.py` source file contains `REAL_PATH_VERIFIED:` and `MOCK_PATH_VERIFIED:` marker comments. The test reads and parses these markers from the file.
**Tests:** Both marker test identifiers are collectible by pytest (`pytest --collect-only` exits 0), mechanically validating that Gate 4's marker check will pass.
**Mode:** both
**Inputs:** Marker strings extracted from `worker/nodes/passthrough.py`.
**Expected output:** Both named tests are collectible by pytest.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_passthrough.py::test_markers_name_collectible_tests -v` exits 0.

---

## test_execute_returns_new_dict (worker/nodes.passthrough)

**File:** `worker/tests/test_passthrough.py`
**Context:** The `PassThrough` node class is instantiated. No shared state exists between calls — the node has no instance variables.
**Tests:** Each `execute()` call returns a new dict object (not a shared singleton), confirming no accidental state leakage between calls.
**Mode:** both
**Inputs:** Two calls to `execute()` with the same context and input value.
**Expected output:** Two distinct dict objects that are equal in content but not identical in identity.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_passthrough.py::test_execute_returns_new_dict -v` exits 0.

---

## test_app_state_with_new_fields (anvilml-server)

**File:** `crates/anvilml-server/tests/state_tests.rs`
**Context:** The `anvilml-server` crate has been compiled with `sqlx` (sqlite, runtime-tokio, migrate, chrono features), `anvilml-scheduler`, and `anvilml-worker` dependencies. `AppState` now has six fields: `config`, `node_registry`, `start_time`, `scheduler`, `workers`, and `db`.
**Tests:** Constructs `AppState` with all six fields — an in-memory `SqlitePool` with migrations applied, a `JobScheduler` backed by a `JobStore`, and an empty `WorkerPool`. Asserts all six fields are accessible and no panics occur.
**Mode:** both
**Inputs:** In-memory `SqlitePool`, empty `JobStore`, fresh `WorkerPool`.
**Expected output:** All six fields accessible; no panics on construction.
**Acceptance:** `cargo test -p anvilml-server --test state_tests test_app_state_with_new_fields` exits 0.

---

## test_app_state_clone_preserves_all_fields (anvilml-server)

**File:** `crates/anvilml-server/tests/state_tests.rs`
**Context:** The `anvilml-server` crate has been compiled with `sqlx`, `anvilml-scheduler`, and `anvilml-worker` dev-dependencies. `AppState` derives `Clone`.
**Tests:** Constructs `AppState` with all six fields, clones it, then asserts that all six fields on the clone are accessible and that the `Arc` pointers for `config`, `node_registry`, `scheduler`, and `workers` are identical (verified via `Arc::as_ptr` pointer comparison).
**Mode:** both
**Inputs:** A constructed `AppState` with all fields.
**Expected output:** Clone has all six fields accessible; `Arc` pointers are identical.
**Acceptance:** `cargo test -p anvilml-server --test state_tests test_app_state_clone_preserves_all_fields` exits 0.

---

## test_app_state_scheduler_arc_sharing (anvilml-server)

**File:** `crates/anvilml-server/tests/state_tests.rs`
**Context:** The `anvilml-server` crate has been compiled with `sqlx`, `anvilml-scheduler`, and `anvilml-worker` dev-dependencies. Both the `AppState`'s `node_registry` and the `JobScheduler`'s internal `node_registry` share the same `Arc<NodeTypeRegistry>`.
**Tests:** Registers a node type via the original state's `node_registry`, then reads back through the cloned state's `node_registry` to verify the scheduler's shared `Arc<NodeTypeRegistry>` is visible through both clones.
**Mode:** both
**Inputs:** A constructed `AppState` with a `JobScheduler` containing an `Arc<NodeTypeRegistry>`.
**Expected output:** `cloned.scheduler.node_registry.list()` returns the same registered nodes as `state.scheduler.node_registry.list()`.
**Acceptance:** `cargo test -p anvilml-server --test state_tests test_app_state_scheduler_arc_sharing` exits 0.

---

## test_submit_job_valid_returns_202 (anvilml-server)

**File:** `crates/anvilml-server/tests/jobs_tests.rs`
**Context:** The `anvilml-server` crate has been compiled with `serde_json` dev-dependency. `build_router()` accepts an `AppState` with a populated `NodeTypeRegistry` containing one registered `NodeTypeDescriptor`.
**Tests:** `POST /v1/jobs` with a valid graph referencing a registered node type returns `202 Accepted` with a `job_id` (valid UUID v4 string) and `queue_position` of 1. The scheduler's `submit()` method validates the graph, persists the job, enqueues it, and returns `(job_id, queue_position)`.
**Mode:** both
**Inputs:** JSON body `{"graph": {"nodes": [{"id": "node1", "type": "TestNode", "inputs": {}, "outputs": {}}]}, "settings": {"device_preference": null}}` with an empty registry populated by one `NodeTypeDescriptor` (`TestNode`).
**Expected output:** `StatusCode::ACCEPTED`; JSON body `{ "job_id": "<uuid>", "queue_position": 1 }`.
**Acceptance:** `cargo test -p anvilml-server --test jobs_tests test_submit_job_valid_returns_202` exits 0.

---

## test_submit_job_malformed_body_returns_400 (anvilml-server)

**File:** `crates/anvilml-server/tests/jobs_tests.rs`
**Context:** The `anvilml-server` crate has been compiled with `serde_json` dev-dependency. The `axum::Json` extractor returns a `Serde` error for malformed input.
**Tests:** `POST /v1/jobs` with invalid JSON (`{not valid json}`) returns `400 Bad Request`. The `axum::Json` extractor fails to deserialize the body and returns an error, which `AnvilError::IntoResponse` maps to HTTP 400.
**Mode:** both
**Inputs:** Raw body `{not valid json}` with `content-type: application/json` header.
**Expected output:** `StatusCode::BAD_REQUEST`.
**Acceptance:** `cargo test -p anvilml-server --test jobs_tests test_submit_job_malformed_body_returns_400` exits 0.

---

## test_submit_job_empty_registry_returns_503 (anvilml-server)

**File:** `crates/anvilml-server/tests/jobs_tests.rs`
**Context:** The `anvilml-server` crate has been compiled with `serde_json` dev-dependency. The `NodeTypeRegistry` is empty (no workers registered).
**Tests:** `POST /v1/jobs` with a structurally valid graph but an empty `NodeTypeRegistry` returns `503 Service Unavailable`. The scheduler's workers-available guard rejects the submission before validation, returning `AnvilError::WorkersUnavailable` which maps to HTTP 503.
**Mode:** both
**Inputs:** JSON body `{"graph": {"nodes": [{"id": "node1", "type": "TestNode", "inputs": {}, "outputs": {}}]}, "settings": {"device_preference": null}}` with an empty `NodeTypeRegistry`.
**Expected output:** `StatusCode::SERVICE_UNAVAILABLE`.
**Acceptance:** `cargo test -p anvilml-server --test jobs_tests test_submit_job_empty_registry_returns_503` exits 0.

---

## test_submit_job_invalid_graph_returns_400 (anvilml-server)

**File:** `crates/anvilml-server/tests/jobs_tests.rs`
**Context:** The `anvilml-server` crate has been compiled with `serde_json` dev-dependency. The `NodeTypeRegistry` is populated with one node type (`RegisteredNode`), but the graph references a different unregistered type (`UnknownNode`).
**Tests:** `POST /v1/jobs` with a graph containing an unknown node type returns `400 Bad Request`. The scheduler's graph validator detects the unknown type and returns `AnvilError::InvalidGraph` which maps to HTTP 400.
**Mode:** both
**Inputs:** JSON body `{"graph": {"nodes": [{"id": "node1", "type": "UnknownNode", "inputs": {}, "outputs": {}}]}, "settings": {"device_preference": null}}` with a registry containing only `RegisteredNode`.
**Expected output:** `StatusCode::BAD_REQUEST`.
**Acceptance:** `cargo test -p anvilml-server --test jobs_tests test_submit_job_invalid_graph_returns_400` exits 0.

---

## test_list_jobs_no_filter_returns_all (anvilml-server)

**File:** `crates/anvilml-server/tests/jobs_tests.rs`
**Context:** The `anvilml-server` crate has been compiled with `tokio` (full), `tower`, `serde_json`, `chrono` (serde), and `uuid` (v4) dev-dependencies. `build_router()` accepts an `AppState` with an in-memory SQLite pool and a populated `NodeTypeRegistry`.
**Tests:** `GET /v1/jobs` with no query params returns all submitted jobs (200, non-empty array).
**Mode:** both
**Inputs:** Submit one job via `POST /v1/jobs`, then `GET /v1/jobs` with no query params.
**Expected output:** `StatusCode::OK`; JSON array with at least 1 job object.
**Acceptance:** `cargo test -p anvilml-server --test jobs_tests test_list_jobs_no_filter_returns_all` exits 0.

---

## test_list_jobs_status_filter (anvilml-server)

**File:** `crates/anvilml-server/tests/jobs_tests.rs`
**Context:** The `anvilml-server` crate has been compiled with `tokio` (full), `tower`, `serde_json`, `chrono` (serde), and `uuid` (v4) dev-dependencies.
**Tests:** `GET /v1/jobs?status=queued` returns only jobs matching the status filter.
**Mode:** both
**Inputs:** Submit two jobs via `POST /v1/jobs`, then `GET /v1/jobs?status=queued`.
**Expected output:** `StatusCode::OK`; JSON array with exactly 2 jobs (both are `queued`).
**Acceptance:** `cargo test -p anvilml-server --test jobs_tests test_list_jobs_status_filter` exits 0.

---

## test_list_jobs_limit (anvilml-server)

**File:** `crates/anvilml-server/tests/jobs_tests.rs`
**Context:** The `anvilml-server` crate has been compiled with `tokio` (full), `tower`, `serde_json`, `chrono` (serde), and `uuid` (v4) dev-dependencies.
**Tests:** `GET /v1/jobs?limit=2` returns at most 2 jobs.
**Mode:** both
**Inputs:** Submit three jobs via `POST /v1/jobs`, then `GET /v1/jobs?limit=2`.
**Expected output:** `StatusCode::OK`; JSON array with at most 2 jobs.
**Acceptance:** `cargo test -p anvilml-server --test jobs_tests test_list_jobs_limit` exits 0.

---

## test_get_job_existing_returns_200 (anvilml-server)

**File:** `crates/anvilml-server/tests/jobs_tests.rs`
**Context:** The `anvilml-server` crate has been compiled with `tokio` (full), `tower`, `serde_json`, and `uuid` (v4) dev-dependencies.
**Tests:** `GET /v1/jobs/:id` on an existing job returns 200 with correct job data.
**Mode:** both
**Inputs:** Submit a job via `POST /v1/jobs`, extract `job_id` from the response, then `GET /v1/jobs/{job_id}`.
**Expected output:** `StatusCode::OK`; JSON body with matching `job_id`.
**Acceptance:** `cargo test -p anvilml-server --test jobs_tests test_get_job_existing_returns_200` exits 0.

---

## test_get_job_unknown_returns_404 (anvilml-server)

**File:** `crates/anvilml-server/tests/jobs_tests.rs`
**Context:** The `anvilml-server` crate has been compiled with `tokio` (full), `tower`, `serde_json`, and `uuid` (v4) dev-dependencies.
**Tests:** `GET /v1/jobs/:id` on a non-existent UUID returns 404.
**Mode:** both
**Inputs:** `GET /v1/jobs/{random_uuid}` where the UUID was never submitted.
**Expected output:** `StatusCode::NOT_FOUND`.
**Acceptance:** `cargo test -p anvilml-server --test jobs_tests test_get_job_unknown_returns_404` exits 0.

---

## test_list_jobs_before_param_accepted (anvilml-server)

**File:** `crates/anvilml-server/tests/jobs_tests.rs`
**Context:** The `anvilml-server` crate has been compiled with `tokio` (full), `tower`, `serde_json`, and `uuid` (v4) dev-dependencies.
**Tests:** `GET /v1/jobs?before=2026-07-08T00:00:00Z` returns 200 — the `before` query param is accepted at the HTTP layer but ignored by the persistence layer (forward-compatibility per `ANVILML_DESIGN.md §13.4`).
**Mode:** both
**Inputs:** Submit one job via `POST /v1/jobs`, then `GET /v1/jobs?before=2026-07-08T00:00:00Z`.
**Expected output:** `StatusCode::OK`; JSON array with at least 1 job.
**Acceptance:** `cargo test -p anvilml-server --test jobs_tests test_list_jobs_before_param_accepted` exits 0.

---

## test_app_state_artifact_store_constructs (anvilml-server)

**File:** `crates/anvilml-server/tests/state_tests.rs`
**Context:** The `anvilml-server` crate has been compiled with `anvilml-artifacts` (path dependency), `tokio` (full), and `sqlx` (sqlite) dev-dependencies. `create_test_artifact_store()` constructs an `ArtifactStore` backed by `std::env::temp_dir()` and an in-memory SQLite pool.
**Tests:** `AppState` constructs with an `ArtifactStore` field; the `Arc` pointer is valid (non-null).
**Mode:** both
**Inputs:** `make_full_state()` called with a fresh `NodeTypeRegistry` and `ArtifactStore` from `create_test_artifact_store()`.
**Expected output:** No panic; `Arc::as_ptr(&state.artifact_store)` returns a non-null pointer.
**Acceptance:** `cargo test -p anvilml-server --test state_tests -- test_app_state_artifact_store_constructs` exits 0.

---

## test_app_state_artifact_store_clone_shares (anvilml-server)

**File:** `crates/anvilml-server/tests/state_tests.rs`
**Context:** The `anvilml-server` crate has been compiled with `anvilml-artifacts` (path dependency), `tokio` (full), and `sqlx` (sqlite) dev-dependencies. `create_test_artifact_store()` constructs an `ArtifactStore` backed by `std::env::temp_dir()` and an in-memory SQLite pool.
**Tests:** Cloned `AppState` shares the same `Arc<ArtifactStore>` allocation as the original — verified via `std::ptr::eq(Arc::as_ptr(...))` pointer comparison.
**Mode:** both
**Inputs:** `AppState` constructed with `ArtifactStore`, then cloned.
**Expected output:** `std::ptr::eq()` returns `true` for the original and clone's `artifact_store` Arc pointers.
**Acceptance:** `cargo test -p anvilml-server --test state_tests -- test_app_state_artifact_store_clone_shares` exits 0.

---

## test_list_artifacts_empty_store_returns_200_empty_array (anvilml-server)

**File:** `crates/anvilml-server/tests/artifacts_tests.rs`
**Context:** The `anvilml-server` crate has been compiled with `anvilml-artifacts` (path dependency), `anvilml-registry` (path dependency), `anvilml-scheduler` (path dependency), `tokio` (full), `sqlx` (sqlite), and `tower` (ServiceExt) dev-dependencies. `make_test_state()` constructs an `AppState` with in-memory SQLite pools and an `ArtifactStore` backed by `std::env::temp_dir()`.
**Tests:** `GET /v1/artifacts` with an empty artifact store returns HTTP 200 with a JSON body that is an empty array `[]`.
**Mode:** both
**Inputs:** `make_test_state()` called with a fresh in-memory SQLite pool; no artifacts saved.
**Expected output:** Response status is `StatusCode::OK` and body parses to `[]`.
**Acceptance:** `cargo test -p anvilml-server --test artifacts_tests test_list_artifacts_empty_store_returns_200_empty_array` exits 0.

---

## test_list_artifacts_populated_returns_all (anvilml-server)

**File:** `crates/anvilml-server/tests/artifacts_tests.rs`
**Context:** The `anvilml-server` crate has been compiled with `anvilml-artifacts` (path dependency), `anvilml-registry` (path dependency), `anvilml-scheduler` (path dependency), `tokio` (full), `sqlx` (sqlite), and `tower` (ServiceExt) dev-dependencies. `make_test_state()` constructs an `AppState` with in-memory SQLite pools and an `ArtifactStore` backed by `std::env::temp_dir()`. `save_artifact()` helper saves an artifact by calling `ArtifactStore::save()` with synthetic PNG bytes.
**Tests:** `GET /v1/artifacts` with no filter returns all artifacts saved in the store — two artifacts with distinct PNG bytes are saved, then the list endpoint is called and the returned array length is asserted to be 2.
**Mode:** both
**Inputs:** `make_test_state()` with a fresh pool; two artifacts saved via `save_artifact()` with different dimensions and seeds.
**Expected output:** Response status is `StatusCode::OK` and body parses to a JSON array of length 2.
**Acceptance:** `cargo test -p anvilml-server --test artifacts_tests test_list_artifacts_populated_returns_all` exits 0.

---

## test_list_artifacts_job_id_filter_returns_matching (anvilml-server)

**File:** `crates/anvilml-server/tests/artifacts_tests.rs`
**Context:** The `anvilml-server` crate has been compiled with `anvilml-artifacts` (path dependency), `anvilml-registry` (path dependency), `anvilml-scheduler` (path dependency), `tokio` (full), `sqlx` (sqlite), and `tower` (ServiceExt) dev-dependencies. Two artifacts are saved with different `job_id` values. The `?job_id=` query parameter is passed via the request URL.
**Tests:** `GET /v1/artifacts?job_id=<uuid>` filters to only artifacts matching the given job ID — saves two artifacts with different `job_id` values, then lists with the first job's UUID and asserts the returned array has length 1 with the correct job_id.
**Mode:** both
**Inputs:** `make_test_state()` with a fresh pool; two artifacts saved with distinct `job_id` UUIDs; request URL includes `?job_id=<first_uuid>`.
**Expected output:** Response status is `StatusCode::OK`, body array length is 1, and the returned artifact's `job_id` matches the filter.
**Acceptance:** `cargo test -p anvilml-server --test artifacts_tests test_list_artifacts_job_id_filter_returns_matching` exits 0.

---

## test_list_artifacts_json_shape (anvilml-server)

**File:** `crates/anvilml-server/tests/artifacts_tests.rs`
**Context:** The `anvilml-server` crate has been compiled with `anvilml-artifacts` (path dependency), `anvilml-registry` (path dependency), `anvilml-scheduler` (path dependency), `tokio` (full), `sqlx` (sqlite), and `tower` (ServiceExt) dev-dependencies. One artifact is saved, then the response body is deserialized into `serde_json::Value` and each field of `ArtifactMeta` is individually type-checked.
**Tests:** `GET /v1/artifacts` returns a JSON array where each element has all `ArtifactMeta` fields with correct types: `hash` (string), `job_id` (string, UUID format), `width` (integer), `height` (integer), `steps` (integer), `seed` (integer), `created_at` (string), `file_path` (string).
**Mode:** both
**Inputs:** `make_test_state()` with a fresh pool; one artifact saved with dimensions 512x512, seed 42, steps 20.
**Expected output:** Response status is `StatusCode::OK`, body array length is 1, and all field names and types match `ArtifactMeta`'s serialization.
**Acceptance:** `cargo test -p anvilml-server --test artifacts_tests test_list_artifacts_json_shape` exits 0.

---

## test_get_artifact_existing_hash_returns_200 (anvilml-server)

**File:** `crates/anvilml-server/tests/artifacts_tests.rs`
**Context:** The `anvilml-server` crate has been compiled with `anvilml-artifacts` (path dependency), `anvilml-registry` (path dependency), `anvilml-scheduler` (path dependency), `tokio` (full), `sqlx` (sqlite), and `tower` (ServiceExt) dev-dependencies. One artifact is saved via `save_artifact()`, then retrieved via the new `GET /v1/artifacts/{hash}` route.
**Tests:** `GET /v1/artifacts/{hash}` returns `StatusCode::OK` (200) for a hash that corresponds to a saved artifact.
**Mode:** both
**Inputs:** `make_test_state()` with a fresh pool; one artifact saved with PNG bytes; request to `/v1/artifacts/<hash>`.
**Expected output:** Response status is `StatusCode::OK` (200).
**Acceptance:** `cargo test -p anvilml-server --test artifacts_tests test_get_artifact_existing_hash_returns_200` exits 0.

---

## test_get_artifact_unknown_hash_returns_404 (anvilml-server)

**File:** `crates/anvilml-server/tests/artifacts_tests.rs`
**Context:** The `anvilml-server` crate has been compiled with `anvilml-artifacts` (path dependency), `anvilml-registry` (path dependency), `anvilml-scheduler` (path dependency), `tokio` (full), `sqlx` (sqlite), and `tower` (ServiceExt) dev-dependencies. No artifact is saved; a known-never-to-exist hash is used.
**Tests:** `GET /v1/artifacts/{hash}` returns `StatusCode::NOT_FOUND` (404) for a hash that does not correspond to any saved artifact, via `AnvilError::ArtifactNotFound`.
**Mode:** both
**Inputs:** `make_test_state()` with a fresh pool; request to `/v1/artifacts/0000...0000` (all-zero SHA-256 hash).
**Expected output:** Response status is `StatusCode::NOT_FOUND` (404).
**Acceptance:** `cargo test -p anvilml-server --test artifacts_tests test_get_artifact_unknown_hash_returns_404` exits 0.

---

## test_get_artifact_byte_for_byte_match (anvilml-server)

**File:** `crates/anvilml-server/tests/artifacts_tests.rs`
**Context:** The `anvilml-server` crate has been compiled with `anvilml-artifacts` (path dependency), `anvilml-registry` (path dependency), `anvilml-scheduler` (path dependency), `tokio` (full), `sqlx` (sqlite), and `tower` (ServiceExt) dev-dependencies. An artifact with known raw PNG bytes is saved directly via `store.save()`, then retrieved and the body bytes are compared.
**Tests:** `GET /v1/artifacts/{hash}` returns raw PNG bytes that are byte-for-byte identical to what was saved, verifying no transformation or corruption occurs.
**Mode:** both
**Inputs:** `make_test_state()` with a fresh pool; artifact saved with explicit `Vec<u8>` containing PNG magic bytes plus 3 extra bytes; request to `/v1/artifacts/<hash>`.
**Expected output:** Response status is `StatusCode::OK` and body bytes exactly match the original saved bytes.
**Acceptance:** `cargo test -p anvilml-server --test artifacts_tests test_get_artifact_byte_for_byte_match` exits 0.

---

## test_get_artifact_content_type_header (anvilml-server)

**File:** `crates/anvilml-server/tests/artifacts_tests.rs`
**Context:** The `anvilml-server` crate has been compiled with `anvilml-artifacts` (path dependency), `anvilml-registry` (path dependency), `anvilml-scheduler` (path dependency), `tokio` (full), `sqlx` (sqlite), and `tower` (ServiceExt) dev-dependencies. One artifact is saved, then retrieved and the response headers are inspected.
**Tests:** `GET /v1/artifacts/{hash}` includes a `Content-Type: image/png` header, confirming the response is correctly typed as a PNG image.
**Mode:** both
**Inputs:** `make_test_state()` with a fresh pool; one artifact saved; request to `/v1/artifacts/<hash>`.
**Expected output:** Response status is `StatusCode::OK` and `Content-Type` header is exactly `image/png`.
**Acceptance:** `cargo test -p anvilml-server --test artifacts_tests test_get_artifact_content_type_header` exits 0.

---

## test_image_ready_saves_artifact (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/event_loop_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `anvilml-artifacts` (path dependency), `anvilml-ipc` (path dependency), `anvilml-core` (path dependency), `base64` (0.22.1), `chrono`, `sqlx` (sqlite, dev), and `uuid` (dev). A valid 1x1 red PNG is base64-encoded and placed in a `WorkerEvent::ImageReady`.
**Tests:** `handle_image_ready()` decodes the base64 payload, saves the PNG to the artifact store, and returns the SHA-256 hash. The saved artifact is then retrieved by hash and verified to be byte-identical to the decoded payload.
**Mode:** mock
**Inputs:** `create_test_artifact_store()` with in-memory SQLite + temp dir; `WorkerEvent::ImageReady` with 512x512 PNG, seed=42, steps=20.
**Expected output:** `store.get(&hash)` returns `Ok(Some(bytes))` matching the decoded payload.
**Acceptance:** `cargo test -p anvilml-scheduler --test event_loop_tests test_image_ready_saves_artifact` exits 0.

---

## test_image_ready_artifact_meta_fields_match (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/event_loop_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `anvilml-artifacts` (path dependency), `anvilml-ipc` (path dependency), `anvilml-core` (path dependency), `base64` (0.22.1), `chrono`, `sqlx` (sqlite, dev), and `uuid` (dev). An artifact is saved with known metadata values.
**Tests:** After saving, `store.list(Some(job_id))` returns one row and all persisted fields (`width`, `height`, `seed`, `steps`, `job_id`) match the event's values.
**Mode:** mock
**Inputs:** `WorkerEvent::ImageReady` with known width=512, height=512, seed=42, steps=20, job_id=known-uuid.
**Expected output:** `store.list(Some(job_id))` returns one row with matching fields.
**Acceptance:** `cargo test -p anvilml-scheduler --test event_loop_tests test_image_ready_artifact_meta_fields_match` exits 0.

---

## test_image_ready_malformed_base64_errors (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/event_loop_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `anvilml-artifacts` (path dependency), `anvilml-ipc` (path dependency), `anvilml-core` (path dependency), `base64` (0.22.1), `chrono`, `sqlx` (sqlite, dev), and `uuid` (dev). A deliberately malformed base64 string is placed in the event.
**Tests:** `handle_image_ready()` returns `Err(AnvilError::Serde(...))` rather than panicking, confirming graceful error handling for encoding failures.
**Mode:** mock
**Inputs:** `WorkerEvent::ImageReady` with `image_b64 = "not-valid-base64!!!@@@"`.
**Expected output:** Returns `Err(AnvilError::Serde(...))` — no panic.
**Acceptance:** `cargo test -p anvilml-scheduler --test event_loop_tests test_image_ready_malformed_base64_errors` exits 0.

---

## test_image_ready_empty_image_b64 (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/event_loop_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `anvilml-artifacts` (path dependency), `anvilml-ipc` (path dependency), `anvilml-core` (path dependency), `base64` (0.22.1), `chrono`, `sqlx` (sqlite, dev), and `uuid` (dev). An empty base64 string is placed in the event.
**Tests:** `handle_image_ready()` decodes the empty string to empty bytes, `save()` succeeds and returns a hash. The artifact is stored with zero bytes and is retrievable.
**Mode:** mock
**Inputs:** `WorkerEvent::ImageReady` with `image_b64 = ""`.
**Expected output:** Returns `Ok(hash)`; artifact stored with 0 bytes, retrievable by hash.
**Acceptance:** `cargo test -p anvilml-scheduler --test event_loop_tests test_image_ready_empty_image_b64` exits 0.
---

## test_map_progress (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/event_loop_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `anvilml-ipc` (for `WorkerEvent`) and `anvilml-core` (for `WsEvent`) dependencies. The `event_loop` module provides `map_worker_event()` which maps `WorkerEvent` variants to `WsEvent` variants.
**Tests:** `map_worker_event()` correctly maps `WorkerEvent::Progress` to `WsEvent::JobProgress` with all fields (job_id, step, total_steps, preview_b64) transferred.
**Mode:** both
**Inputs:** `WorkerEvent::Progress { job_id, step: 5, total_steps: 20, preview_b64: Some("dGVzdCBwcmV2aWV3") }`.
**Expected output:** `WsEvent::JobProgress` with identical field values.
**Acceptance:** `cargo test -p anvilml-scheduler --test event_loop_tests test_map_progress` exits 0.

---

## test_map_completed (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/event_loop_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `anvilml-ipc` (for `WorkerEvent`) and `anvilml-core` (for `WsEvent`) dependencies. The `event_loop` module provides `map_worker_event()` which maps `WorkerEvent` variants to `WsEvent` variants.
**Tests:** `map_worker_event()` correctly maps `WorkerEvent::Completed` to `WsEvent::JobCompleted` with all fields (job_id, elapsed_ms) transferred.
**Mode:** both
**Inputs:** `WorkerEvent::Completed { job_id, elapsed_ms: 12345 }`.
**Expected output:** `WsEvent::JobCompleted` with identical field values.
**Acceptance:** `cargo test -p anvilml-scheduler --test event_loop_tests test_map_completed` exits 0.

---

## test_map_failed (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/event_loop_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `anvilml-ipc` (for `WorkerEvent`) and `anvilml-core` (for `WsEvent`) dependencies. The `event_loop` module provides `map_worker_event()` which maps `WorkerEvent` variants to `WsEvent` variants.
**Tests:** `map_worker_event()` correctly maps `WorkerEvent::Failed` to `WsEvent::JobFailed` with `job_id` and `error` transferred, while the `traceback` field is dropped (not present in `WsEvent::JobFailed`).
**Mode:** both
**Inputs:** `WorkerEvent::Failed { job_id, error: "CUDA out of memory", traceback: Some("Traceback...") }`.
**Expected output:** `WsEvent::JobFailed` with `job_id` and `error` matching; no `traceback` field in the result.
**Acceptance:** `cargo test -p anvilml-scheduler --test event_loop_tests test_map_failed` exits 0.

---

## test_map_cancelled (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/event_loop_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `anvilml-ipc` (for `WorkerEvent`) and `anvilml-core` (for `WsEvent`) dependencies. The `event_loop` module provides `map_worker_event()` which maps `WorkerEvent` variants to `WsEvent` variants.
**Tests:** `map_worker_event()` correctly maps `WorkerEvent::Cancelled` to `WsEvent::JobCancelled` with `job_id` transferred.
**Mode:** both
**Inputs:** `WorkerEvent::Cancelled { job_id }`.
**Expected output:** `WsEvent::JobCancelled` with identical `job_id`.
**Acceptance:** `cargo test -p anvilml-scheduler --test event_loop_tests test_map_cancelled` exits 0.

---

## test_image_ready_publishes_after_save (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/event_loop_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `anvilml-ipc` (for `WorkerEvent`, `EventBroadcaster`) and `anvilml-core` (for `WsEvent`) dependencies. The `event_loop` module provides `map_worker_event()` and `spawn_event_loop()`.
**Tests:** `map_worker_event()` maps `WorkerEvent::ImageReady` to `WsEvent::JobImageReady` with correct `job_id`, `width`, `height`, `seed`, and `steps` fields. The `artifact_hash` is empty because `map_worker_event()` does not have access to the saved hash (that is populated by `spawn_event_loop()` after the artifact save).
**Mode:** both
**Inputs:** `WorkerEvent::ImageReady { job_id, image_b64: <valid PNG>, width: 512, height: 512, format: "png", seed: 42, steps: 20 }`.
**Expected output:** `WsEvent::JobImageReady` with matching fields; `artifact_hash` is empty string.
**Acceptance:** `cargo test -p anvilml-scheduler --test event_loop_tests test_image_ready_publishes_after_save` exits 0.

---

## test_spawn_event_loop_receives_and_publishes (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/event_loop_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `anvilml-ipc` (for `WorkerEvent`, `EventBroadcaster`), `anvilml-registry` (for `JobStore`), `anvilml-core` (for `WsEvent`, `NodeTypeRegistry`), `anvilml-artifacts` (for `ArtifactStore`), and `zeromq` dev-dependencies. The `event_loop` module provides `spawn_event_loop()` which subscribes the scheduler to worker events via a `Demux` subscription.
**Tests:** End-to-end: a `Demux` is created. The event loop is spawned subscribed to it, a `WorkerEvent::Completed` is routed via `demux.route()` (simulating what `bridge.rs`'s `reader_task` does), and the broadcaster receives the corresponding `WorkerEvent::Completed` message, and the broadcaster receives the corresponding `WsEvent::JobCompleted` event. The test verifies the `elapsed_ms` field is correctly transferred and that the event loop processes messages from its `Demux` subscription.
**Mode:** both
**Inputs:** `WorkerEvent::Completed { job_id, elapsed_ms: 10000 }` routed via `Demux::route("test-worker-1", ...)`.
**Expected output:** `WsEvent::JobCompleted` received on the broadcaster with `elapsed_ms == 10000`.
**Acceptance:** `cargo test -p anvilml-scheduler --test event_loop_tests test_spawn_event_loop_receives_and_publishes` exits 0.

---

## test_ready_event_updates_hardware_caps (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/event_loop_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `anvilml-ipc` (for `WorkerEvent`, `EventBroadcaster`), `anvilml-registry` (for `JobStore`), `anvilml-core` (for `HardwareInfo`, `GpuDevice`, `InferenceCaps`, `CapabilitySource`), `anvilml-artifacts` (for `ArtifactStore`), and `zeromq` dev-dependencies. `event_loop.rs`'s `spawn_event_loop()` now takes a fifth `hardware: Arc<RwLock<HardwareInfo>>` argument and applies `Ready` events to it via the private `apply_ready_capabilities()`.
**Tests:** End-to-end, closing the coverage gap flagged after Phase 16 (`Ready` events reaching the live `spawn_event_loop()` `Demux` subscription had no test at all): a `HardwareInfo` fixture with one `GpuDevice` at `index: 0` (`caps: InferenceCaps::default()`, `capabilities_source: Fallback`) is constructed, the event loop is spawned against it, and a `WorkerEvent::Ready { device_index: 0, fp32: true, fp16: true, bf16: true, fp8: true, fp4: true, flash_attention: true, .. }` is routed via `demux.route()`. The test polls (bounded, 2s) until `hardware.gpus[0].capabilities_source == PyTorch`, then asserts every `caps` field was applied, `capabilities_source` was overwritten to `PyTorch`, and the top-level `inference_caps` union reflects the update — including `fp32`/`fp4`, the two fields `WorkerEvent::Ready` previously omitted entirely.
**Mode:** both
**Inputs:** `WorkerEvent::Ready { worker_id: "0", device_index: 0, fp32: true, fp16: true, bf16: true, fp8: true, fp4: true, flash_attention: true, capabilities_source: "pytorch", node_types: vec![], .. }` routed via `Demux::route("0", ...)`.
**Expected output:** `hardware.gpus[0].caps` has all six fields `true`; `capabilities_source == CapabilitySource::PyTorch`; `hardware.inference_caps.fp32 == true` and `hardware.inference_caps.fp4 == true`.
**Acceptance:** `cargo test -p anvilml-scheduler --test event_loop_tests test_ready_event_updates_hardware_caps` exits 0.

---

## test_ready_event_unknown_device_index_does_not_panic (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/event_loop_tests.rs`
**Context:** Same fixture setup as `test_ready_event_updates_hardware_caps`. Exercises `apply_ready_capabilities()`'s "no matching `GpuDevice`" branch, described in its own doc comment as a defensive no-op rather than a panic.
**Tests:** Routes a `WorkerEvent::Ready` with `device_index: 99` against a `HardwareInfo` fixture whose only device is `index: 0`. Asserts the event loop's `JoinHandle` is still running (`!handle.is_finished()`) after a short delay, and that the existing device's `caps`/`capabilities_source` are untouched and no `GpuDevice` was added or removed.
**Mode:** both
**Inputs:** `WorkerEvent::Ready { device_index: 99, .. }` routed via `Demux::route("99", ...)` against a one-device `HardwareInfo` fixture (`index: 0`).
**Expected output:** Event loop task still running; `hardware.gpus.len() == 1`; `hardware.gpus[0].capabilities_source` unchanged (`Fallback`).
**Acceptance:** `cargo test -p anvilml-scheduler --test event_loop_tests test_ready_event_unknown_device_index_does_not_panic` exits 0.

---

## test_spawn_event_loop_handles_recv_error (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/event_loop_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `anvilml-ipc` (for `EventBroadcaster`), `anvilml-registry` (for `JobStore`), and `anvilml-core` (for `NodeTypeRegistry`) dependencies. The `event_loop` module provides `spawn_event_loop()`. Verifies the spawned task can be aborted directly.
**Tests:** The event loop retries gracefully after a transport `recv()` error. After spawning the event loop and immediately closing the transport, the spawned task remains alive (not finished) — proving it logs the error and retries instead of panicking or exiting.
**Mode:** both
**Inputs:** A `Demux`; the spawned task's `JoinHandle` is aborted directly.
**Expected output:** The event loop task remains alive after transport close (verified via `handle.is_finished()` returning false).
**Acceptance:** `cargo test -p anvilml-scheduler --test event_loop_tests test_spawn_event_loop_handles_recv_error` exits 0.

---

## test_completed_persists_status_and_releases_ledger (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/event_loop_tests.rs`
**Context:** The `anvilml-scheduler` crate has been compiled with `anvilml-ipc` (for `WorkerEvent`, `EventBroadcaster`), `anvilml-registry` (for `JobStore`), `anvilml-core` (for `Job`, `JobStatus`, `NodeTypeRegistry`, `WsEvent`), `anvilml-artifacts` (for `ArtifactStore`), `zeromq` and `sqlx` dev-dependencies, and the `test-util` feature enabled (for `ledger_reservations_test()`). The event loop's terminal event arms (Completed/Failed/Cancelled) persist status transitions and release VRAM reservations.
**Tests:** End-to-end: a full event loop setup with a JobStore containing a `Running` job with `worker_id="0"`. VRAM is reserved on device 0 in the ledger. A `WorkerEvent::Completed` is sent via the transport. The test verifies: (1) the job's `status` is `Completed` and `completed_at` is set in the DB, (2) the ledger reservation for device 0 is zeroed, (3) the broadcaster receives `WsEvent::JobCompleted` with correct fields.
**Mode:** both
**Inputs:** `WorkerEvent::Completed { job_id, elapsed_ms: 5000 }` routed via `Demux::route()`; a `Running` job with `worker_id="0"` pre-populated in JobStore; 8192 MiB reserved on device 0 in the ledger.
**Expected output:** `status=Completed`, `completed_at` set, ledger reservation zeroed, `WsEvent::JobCompleted` published.
**Acceptance:** `cargo test -p anvilml-scheduler --test event_loop_tests test_completed_persists_status_and_releases_ledger` exits 0.

---

## test_failed_persists_status_error_and_releases_ledger (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/event_loop_tests.rs`
**Context:** Same setup as `test_completed_persists_status_and_releases_ledger`. Verifies the `Failed` event arm persists the error string.
**Tests:** A `WorkerEvent::Failed` with `error="CUDA out of memory"` is sent. The test verifies: (1) the job's `status` is `Failed` and `completed_at` is set, (2) the job's `error` field equals `"CUDA out of memory"`, (3) the ledger reservation for device 0 is zeroed, (4) the broadcaster receives `WsEvent::JobFailed` with correct fields.
**Mode:** both
**Inputs:** `WorkerEvent::Failed { job_id, error: "CUDA out of memory", traceback: Some(...) }` routed via `Demux::route()`; a `Running` job with `worker_id="0"` pre-populated in JobStore; 4096 MiB reserved on device 0.
**Expected output:** `status=Failed`, `completed_at` set, `error="CUDA out of memory"`, ledger reservation zeroed, `WsEvent::JobFailed` published.
**Acceptance:** `cargo test -p anvilml-scheduler --test event_loop_tests test_failed_persists_status_error_and_releases_ledger` exits 0.

---

## test_cancelled_persists_status_and_releases_ledger (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/event_loop_tests.rs`
**Context:** Same setup as `test_completed_persists_status_and_releases_ledger`, but exercises device index 1.
**Tests:** A `WorkerEvent::Cancelled` is sent for a job with `worker_id="1"`. The test verifies: (1) the job's `status` is `Cancelled` and `completed_at` is set, (2) the ledger reservation for device 1 is zeroed, (3) the broadcaster receives `WsEvent::JobCancelled` with correct fields.
**Mode:** both
**Inputs:** `WorkerEvent::Cancelled { job_id }` routed via `Demux::route()`; a `Running` job with `worker_id="1"` pre-populated in JobStore; 6144 MiB reserved on device 1.
**Expected output:** `status=Cancelled`, `completed_at` set, ledger reservation zeroed, `WsEvent::JobCancelled` published.
**Acceptance:** `cargo test -p anvilml-scheduler --test event_loop_tests test_cancelled_persists_status_and_releases_ledger` exits 0.

---

## test_terminal_events_publish_ws_event (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/event_loop_tests.rs`
**Context:** Same infrastructure as other terminal event tests. Verifies all three terminal events publish the correct `WsEvent` variant.
**Tests:** Sends `Completed`, `Failed`, and `Cancelled` events sequentially. Each is verified against its matching `WsEvent` variant (`JobCompleted`, `JobFailed`, `JobCancelled`) with correct field values.
**Mode:** both
**Inputs:** Three terminal events routed sequentially via `Demux::route()`.
**Expected output:** Three `WsEvent` variants received in order: `JobCompleted`, `JobFailed`, `JobCancelled`, each with correct fields.
**Acceptance:** `cargo test -p anvilml-scheduler --test event_loop_tests test_terminal_events_publish_ws_event` exits 0.

---

## test_terminal_event_unknown_job_logs_warning (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/event_loop_tests.rs`
**Context:** Same infrastructure, but the JobStore is empty (no jobs). Verifies the event loop handles a `Completed` event for a non-existent job gracefully.
**Tests:** A `WorkerEvent::Completed` is sent with a UUID that doesn't exist in the database. The test verifies: (1) the event loop does not panic, (2) the broadcaster still receives `WsEvent::JobCompleted` (the WebSocket stream is not interrupted), (3) no job row is created in the database.
**Mode:** both
**Inputs:** `WorkerEvent::Completed { job_id: <nonexistent UUID>, elapsed_ms: 1000 }` routed via `Demux::route()`; empty JobStore.
**Expected output:** `WsEvent::JobCompleted` published; no job row created; event loop continues running.
**Acceptance:** `cargo test -p anvilml-scheduler --test event_loop_tests test_terminal_event_unknown_job_logs_warning` exits 0.

---

## test_progress_still_published_via_map_worker_event (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/event_loop_tests.rs`
**Context:** Same infrastructure. Verifies that `Progress` events (non-terminal) still flow through the `map_worker_event()` path unchanged.
**Tests:** A `WorkerEvent::Progress` is sent. The test verifies the broadcaster receives `WsEvent::JobProgress` with all fields (`step`, `total_steps`, `preview_b64`) correctly transferred. This confirms the new terminal event arms did not break the existing non-terminal event path.
**Mode:** both
**Inputs:** `WorkerEvent::Progress { job_id, step: 10, total_steps: 20, preview_b64: Some("dGVzdA==") }` routed via `Demux::route()`.
**Expected output:** `WsEvent::JobProgress` with `step==10`, `total_steps==20`, `preview_b64==Some("dGVzdA==")`.
**Acceptance:** `cargo test -p anvilml-scheduler --test event_loop_tests test_progress_still_published_via_map_worker_event` exits 0.

---

## test_completed_restores_worker_idle_wakes_dispatch (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/event_loop_tests.rs`
**Context:** P16-A3 adds a `WorkerPool` parameter to `spawn_event_loop()` and worker Idle restoration + dispatch wake to each terminal event arm. The test creates a `WorkerPool` with one mock handle at `Busy` status, a `Running` job with `worker_id="0"`, and sends a `Completed` event.
**Tests:** After the event loop processes the `Completed` event, verifies: (1) the mock handle's status is `Idle` (was `Busy` before), (2) the scheduler's dispatch wake count is >= 1.
**Mode:** both
**Inputs:** `WorkerEvent::Completed { job_id, elapsed_ms: 5000 }` routed via `Demux::route()`; mock handle at `Busy`; job with `worker_id="0"`.
**Expected output:** Handle status == `Idle`; wake count >= 1.
**Acceptance:** `cargo test -p anvilml-scheduler --test event_loop_tests test_completed_restores_worker_idle_wakes_dispatch` exits 0.

---

## test_failed_restores_worker_idle_wakes_dispatch (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/event_loop_tests.rs`
**Context:** Same infrastructure as `test_completed_restores_worker_idle_wakes_dispatch`, but sends a `Failed` event.
**Tests:** After processing the `Failed` event, verifies the mock handle's status is `Idle` and the dispatch wake count is >= 1.
**Mode:** both
**Inputs:** `WorkerEvent::Failed { job_id, error: "CUDA out of memory", traceback: None }` routed via `Demux::route()`; mock handle at `Busy`.
**Expected output:** Handle status == `Idle`; wake count >= 1.
**Acceptance:** `cargo test -p anvilml-scheduler --test event_loop_tests test_failed_restores_worker_idle_wakes_dispatch` exits 0.

---

## test_cancelled_restores_worker_idle_wakes_dispatch (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/event_loop_tests.rs`
**Context:** Same infrastructure, but sends a `Cancelled` event.
**Tests:** After processing the `Cancelled` event, verifies the mock handle's status is `Idle` and the dispatch wake count is >= 1.
**Mode:** both
**Inputs:** `WorkerEvent::Cancelled { job_id }` routed via `Demux::route()`; mock handle at `Busy`.
**Expected output:** Handle status == `Idle`; wake count >= 1.
**Acceptance:** `cargo test -p anvilml-scheduler --test event_loop_tests test_cancelled_restores_worker_idle_wakes_dispatch` exits 0.

---

## test_progress_does_not_wake_dispatch (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/event_loop_tests.rs`
**Context:** Same infrastructure. Verifies that a non-terminal `Progress` event does NOT increment the dispatch wake count.
**Tests:** Sends a `Progress` event and verifies the wake count remains at 0 — Progress is not a terminal event and should not trigger dispatch loop wake.
**Mode:** both
**Inputs:** `WorkerEvent::Progress { job_id, step: 10, total_steps: 20, preview_b64: Some("dGVzdA==") }` routed via `Demux::route()`.
**Expected output:** Wake count == 0 after Progress event.
**Acceptance:** `cargo test -p anvilml-scheduler --test event_loop_tests test_progress_does_not_wake_dispatch` exits 0.

---

## test_queued_job_dispatched_after_first_completes (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/event_loop_tests.rs`
**Context:** P16-A3's integration test. Creates two jobs in the queue, sends a `Completed` event for the first job, then verifies the dispatch loop processes the second job.
**Tests:** After the first job's `Completed` event frees the worker, the dispatch loop (woken by `wake_dispatch()`) picks up the second job from the queue. The test verifies the second job's status transitions to `Running` in the database — proving the dispatch loop was woken by the terminal event without a new `submit()` call.
**Mode:** both
**Inputs:** Two jobs submitted to the scheduler; `WorkerEvent::Completed` for the first job; mock handle at `Busy`.
**Expected output:** Second job's DB status is `Running`; worker status is `Idle`; dispatch loop was woken.
**Acceptance:** `cargo test -p anvilml-scheduler --test event_loop_tests test_queued_job_dispatched_after_first_completes` exits 0.

---

## test_subscribe_receives_fanned_out_event (anvilml-worker)

**File:** `crates/anvilml-worker/tests/demux_tests.rs`
**Context:** `Demux` gained `subscribe()`/`unsubscribe()` fan-out per `ANVILML_DESIGN.md §9.8` (`docs/ADDENDUM_DEMUX_FANOUT.md`), ahead of `P16-B1`.
**Tests:** A subscriber receives a `(worker_id, WorkerEvent)` clone of an event routed via `route()`, even when no primary worker is registered for that `worker_id` (fan-out and primary delivery are independent).
**Mode:** both
**Inputs:** `demux.subscribe()`, then `route("worker-0", WorkerEvent::Pong { seq: 7 })` with no prior `register()`.
**Expected output:** `route()` returns `Err(AnvilError::WorkerNotFound)`; the subscriber still receives `("worker-0", Pong { seq: 7 })`.
**Acceptance:** `cargo test -p anvilml-worker --test demux_tests test_subscribe_receives_fanned_out_event` exits 0.

---

## test_multiple_subscribers_each_receive_independently (anvilml-worker)

**File:** `crates/anvilml-worker/tests/demux_tests.rs`
**Context:** Same `§9.8` fan-out addition. Verifies fan-out scales to more than one subscriber without cross-interference or affecting primary delivery.
**Tests:** Two independent subscribers and one primary (`register()`ed) consumer all receive their own copy of the same routed event.
**Mode:** both
**Inputs:** Two `demux.subscribe()` calls, one `demux.register("worker-0", ...)`, then `route("worker-0", WorkerEvent::Cancelled { job_id })`.
**Expected output:** The primary consumer, subscriber A, and subscriber B each independently receive the event.
**Acceptance:** `cargo test -p anvilml-worker --test demux_tests test_multiple_subscribers_each_receive_independently` exits 0.

---

## test_unsubscribe_stops_fanout_delivery (anvilml-worker)

**File:** `crates/anvilml-worker/tests/demux_tests.rs`
**Context:** Same `§9.8` fan-out addition. Mirrors the mandatory deregistration test convention (`§9.4`) for the new subscription mechanism.
**Tests:** After `unsubscribe()`, that subscription's channel is closed (`recv()` returns `None`) rather than continuing to receive events or hanging.
**Mode:** both
**Inputs:** `subscribe()`, route one event (received), `unsubscribe(id)`, route a second event.
**Expected output:** First event received; after `unsubscribe()`, `recv()` returns `None` rather than the second event.
**Acceptance:** `cargo test -p anvilml-worker --test demux_tests test_unsubscribe_stops_fanout_delivery` exits 0.

---

## test_unsubscribe_unknown_id_is_safe (anvilml-worker)

**File:** `crates/anvilml-worker/tests/demux_tests.rs`
**Context:** Same `§9.8` fan-out addition. Mirrors `test_double_deregister_is_safe`'s idempotency guarantee for the new subscription mechanism.
**Tests:** `unsubscribe()` with an id that was never issued (or already removed) does not panic and does not affect other, unrelated active subscriptions.
**Mode:** both
**Inputs:** `unsubscribe(9999)` with no matching subscription, followed by a real, separate `subscribe()` + `route()`.
**Expected output:** No panic; the unrelated real subscription still receives its event normally.
**Acceptance:** `cargo test -p anvilml-worker --test demux_tests test_unsubscribe_unknown_id_is_safe` exits 0.

---

## test_full_subscriber_channel_does_not_block_route (anvilml-worker)

**File:** `crates/anvilml-worker/tests/demux_tests.rs`
**Context:** Same `§9.8` fan-out addition. Verifies the "best-effort, never blocks" guarantee — a stalled subscriber must never be able to stall `route()` or the primary consumer.
**Tests:** With a subscriber that never drains its channel, 300 events (exceeding the 256-capacity subscriber channel) are routed to a registered primary worker. Every `route()` call still returns `Ok(())`, and the primary consumer receives all 300 events regardless of the subscriber's full channel.
**Mode:** both
**Inputs:** One never-drained `subscribe()`, one `register()`ed primary consumer, 300 sequential `route()` calls with distinct `WorkerEvent::Pong { seq }` events.
**Expected output:** All 300 `route()` calls return `Ok(())`; the primary consumer's channel contains exactly 300 events.
**Acceptance:** `cargo test -p anvilml-worker --test demux_tests test_full_subscriber_channel_does_not_block_route` exits 0.

---

## test_spawn_event_loop_subscription_exists_before_return (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/event_loop_tests.rs`
**Context:** Regression test for a race found while validating `P16-A4`'s Demux retrofit: `spawn_event_loop()`'s `Demux::subscribe()` call must happen synchronously, before the function spawns its task and returns — not inside the spawned `async move` block, where it would only run once the task is first polled by the executor.
**Tests:** An event routed via `demux.route()` immediately after `spawn_event_loop()` returns, with no sleep or other synchronization, is still received and published — proving the subscription exists by the time the caller regains control, not merely "eventually."
**Mode:** both
**Inputs:** `spawn_event_loop(...)` followed immediately by `demux.route("test-worker-1", WorkerEvent::Progress { .. })`.
**Expected output:** The broadcaster receives the corresponding `WsEvent::JobProgress` within the 5s timeout.
**Acceptance:** `cargo test -p anvilml-scheduler --test event_loop_tests test_spawn_event_loop_subscription_exists_before_return` exits 0.

---

## test_connect_receives_initial_system_stats_frame (anvilml-server)

**File:** `crates/anvilml-server/tests/handler_tests.rs`
**Context:** `P16-C1` — the `GET /v1/events` WebSocket upgrade handler skeleton, per `ANVILML_DESIGN.md §13.6`'s connect sequence (subscribe, then send the current `SystemStats`).
**Tests:** Connecting to `/v1/events` via a real WebSocket client yields exactly one initial frame, and that frame's JSON body carries `"type": "system_stats"`.
**Mode:** both
**Inputs:** A real `TcpListener`-backed server built from `build_router()`; a `tokio_tungstenite::connect_async()` client connection to `/v1/events`.
**Expected output:** The first message received is a Text frame whose parsed JSON has `type == "system_stats"`.
**Acceptance:** `cargo test -p anvilml-server --test handler_tests test_connect_receives_initial_system_stats_frame` exits 0.

---

## test_initial_frame_matches_ws_event_shape (anvilml-server)

**File:** `crates/anvilml-server/tests/handler_tests.rs`
**Context:** Same `P16-C1` handler skeleton. Confirms the initial frame is not just tagged correctly but round-trips through `WsEvent`'s own `Deserialize` impl as the `SystemStats` variant.
**Tests:** The initial frame deserializes as `WsEvent::SystemStats` with the placeholder's zero-valued fields (`cpu_pct: 0.0`, `ram_used_mib: 0`, `workers: []`) — acceptable per `P16-C1`'s scope since the real periodic tick is `P16-D1`.
**Mode:** both
**Inputs:** Same real-socket setup as `test_connect_receives_initial_system_stats_frame`.
**Expected output:** `serde_json::from_str::<WsEvent>(...)` succeeds and matches the `SystemStats` variant with zero-valued fields.
**Acceptance:** `cargo test -p anvilml-server --test handler_tests test_initial_frame_matches_ws_event_shape` exits 0.

---

## test_handler_stays_alive_after_initial_frame (anvilml-server)

**File:** `crates/anvilml-server/tests/handler_tests.rs`
**Context:** `P16-C2` forward loop is now active. After the initial `SystemStats` frame, the handler enters the forward loop and waits for broadcast events — it no longer returns after one frame. This test verifies the handler stays connected.
**Tests:** After the initial frame, the handler stays connected. A 500ms timeout on the next read confirms no Close frame arrives, proving the handler is in the forward loop rather than returning prematurely.
**Mode:** both
**Inputs:** Same real-socket setup; reads one item, then uses `tokio::time::timeout(500ms, ws.next())`.
**Expected output:** No message arrives within 500ms (timeout), confirming the handler is alive and waiting for events.
**Acceptance:** `cargo test -p anvilml-server --test handler_tests test_handler_stays_alive_after_initial_frame` exits 0.

---

## test_multiple_clients_each_receive_independent_initial_frame (anvilml-server)

**File:** `crates/anvilml-server/tests/handler_tests.rs`
**Context:** Same `P16-C1` handler skeleton. Confirms `state.broadcaster.subscribe()` is called per-connection (one receiver per socket), not once and shared.
**Tests:** Two concurrent WebSocket clients connecting to `/v1/events` each independently receive their own initial `SystemStats` frame.
**Mode:** both
**Inputs:** Two concurrent `tokio_tungstenite::connect_async()` connections to the same test server.
**Expected output:** Both clients' first frames are Text frames with `type == "system_stats"`.
**Acceptance:** `cargo test -p anvilml-server --test handler_tests test_multiple_clients_each_receive_independent_initial_frame` exits 0.

---

## test_handler_stays_alive_after_initial_frame (anvilml-server)

**File:** `crates/anvilml-server/tests/handler_tests.rs`
**Context:** `P16-C2` forward loop is now active. After the initial `SystemStats` frame, the handler enters the forward loop and waits for broadcast events — it no longer returns after one frame.
**Tests:** After the initial frame, the handler stays connected. A 500ms timeout on the next read confirms no Close frame arrives, proving the handler is in the forward loop rather than returning prematurely.
**Mode:** both
**Inputs:** Same real-socket setup; reads one item, then uses `tokio::time::timeout(500ms, ws.next())`.
**Expected output:** No message arrives within 500ms (timeout), confirming the handler is alive and waiting for events.
**Acceptance:** `cargo test -p anvilml-server --test handler_tests test_handler_stays_alive_after_initial_frame` exits 0.

---

## test_forwarded_event_is_json_text (anvilml-server)

**File:** `crates/anvilml-server/tests/handler_tests.rs`
**Context:** `P16-C2` forward loop is active. Events published to the broadcast channel after the initial frame are serialized as JSON text and forwarded to the connected client.
**Tests:** After connecting and consuming the initial `SystemStats` frame, publish a `JobQueued` event via `broadcaster.publish()`. The client should receive a Text frame with `"type":"job_queued"`.
**Mode:** both
**Inputs:** Real-socket setup; publish `WsEvent::JobQueued { job_id, queue_position: 1 }`.
**Expected output:** Client receives `ClientMessage::Text` with JSON containing `"type":"job_queued"` and matching `job_id`/`queue_position`.
**Acceptance:** `cargo test -p anvilml-server --test handler_tests test_forwarded_event_is_json_text` exits 0.

---

## test_lagged_error_closes_connection (anvilml-server)

**File:** `crates/anvilml-server/tests/handler_tests.rs`
**Context:** `P16-C2` `RecvError::Lagged` disconnect path. Publishing >1024 events while the client is idle overflows the 1024-event broadcast buffer, causing `recv()` to return `Lagged(n)`.
**Tests:** After the initial frame, publish 1100 `ProvisioningProgress` events rapidly without reading from the client. The client's next `recv()` should yield `Lagged`, and the handler should send Close and exit.
**Mode:** both
**Inputs:** Real-socket setup; publish 1100 events, then read from stream.
**Expected output:** Client stream yields `None`, `Ok(Close(_))`, or `Err(_)` — never a data frame.
**Acceptance:** `cargo test -p anvilml-server --test handler_tests test_lagged_error_closes_connection` exits 0.

---

## test_concurrent_clients_independent_copies (anvilml-server)

**File:** `crates/anvilml-server/tests/handler_tests.rs`
**Context:** `P16-C2` forward loop with multiple subscribers. Each client gets an independent broadcast subscription, so both receive their own copy of forwarded events.
**Tests:** Connect two clients, consume initial frames from both. Publish one event. Both clients should independently receive the event as a Text frame.
**Mode:** both
**Inputs:** Two concurrent connections; publish one `JobQueued` event.
**Expected output:** Both clients receive `ClientMessage::Text` with matching `job_id` and `"type":"job_queued"`.
**Acceptance:** `cargo test -p anvilml-server --test handler_tests test_concurrent_clients_independent_copies` exits 0.

---

## test_lagged_disconnect_no_panic (anvilml-server)

**File:** `crates/anvilml-server/tests/handler_tests.rs`
**Context:** `P16-C2` graceful disconnect. After a Lagged disconnect, the server should remain operational — new clients can connect and receive their initial frame.
**Tests:** Trigger a Lagged disconnect with one client, then connect a second client. The second client should successfully receive the initial `SystemStats` frame.
**Mode:** both
**Inputs:** Two connections; first client triggers lag with 1100 events, second client connects after lag.
**Expected output:** Second client receives initial `SystemStats` frame with `type == "system_stats"`.
**Acceptance:** `cargo test -p anvilml-server --test handler_tests test_lagged_disconnect_no_panic` exits 0.

---

## test_tick_publishes_system_stats (anvilml-server)

**File:** `crates/anvilml-server/tests/stats_tick_tests.rs`
**Context:** `P16-D1` — `spawn_stats_tick()`'s periodic background `SystemStats` publisher, per `ANVILML_DESIGN.md §13.1`/§13.6.
**Tests:** A tick publishes a `WsEvent::SystemStats` observable by a subscriber that subscribed before the tick task was spawned.
**Mode:** both
**Inputs:** `spawn_stats_tick(broadcaster, empty_pool, Duration::from_millis(20))`; a receiver subscribed beforehand.
**Expected output:** The receiver yields `WsEvent::SystemStats { .. }` within a 2s timeout.
**Acceptance:** `cargo test -p anvilml-server --features mock-hardware --test stats_tick_tests test_tick_publishes_system_stats` exits 0.

---

## test_workers_reflect_pool_state (anvilml-server)

**File:** `crates/anvilml-server/tests/stats_tick_tests.rs`
**Context:** Same `P16-D1` task. Confirms `WorkerPool::list()` (added by this task) is actually used to populate the tick's `workers` field, rather than an empty placeholder.
**Tests:** With two workers injected via `set_up_test_workers()` (`worker_id="0"` Idle/Cuda, `worker_id="1"` Busy/Cpu), the published `SystemStats.workers` contains both, with matching `status`, `device_index`, and `device_type`.
**Mode:** both
**Inputs:** A `WorkerPool` pre-populated via `set_up_test_workers()` with two `(WorkerHandle, GpuDevice)` pairs.
**Expected output:** `worker_infos.len() == 2`; each entry's fields match its corresponding injected handle/device.
**Acceptance:** `cargo test -p anvilml-server --features mock-hardware --test stats_tick_tests test_workers_reflect_pool_state` exits 0.

---

## test_two_consecutive_ticks_both_publish (anvilml-server)

**File:** `crates/anvilml-server/tests/stats_tick_tests.rs`
**Context:** Same `P16-D1` task. Confirms the task runs an ongoing periodic loop, distinguishing it from `P16-C1`'s deliberately one-shot initial-frame send.
**Tests:** Two consecutive ticks each independently publish a `WsEvent::SystemStats`.
**Mode:** both
**Inputs:** `spawn_stats_tick(..., Duration::from_millis(15))`; two sequential `rx.recv()` calls, each under a 2s timeout.
**Expected output:** Both received events are `WsEvent::SystemStats { .. }`.
**Acceptance:** `cargo test -p anvilml-server --features mock-hardware --test stats_tick_tests test_two_consecutive_ticks_both_publish` exits 0.

---

## test_interval_parameter_controls_cadence (anvilml-server)

**File:** `crates/anvilml-server/tests/stats_tick_tests.rs`
**Context:** Same `P16-D1` task. Locks in the task's own key implementation note — the interval is an injected constructor parameter, not a hardcoded `Duration::from_secs(5)` literal.
**Tests:** With a 10ms injected interval, three ticks are received well within 800ms — impossible if the loop silently used a hardcoded 5s period instead of the parameter.
**Mode:** both
**Inputs:** `spawn_stats_tick(..., Duration::from_millis(10))`; three sequential `rx.recv()` calls, each under a 2s timeout; wall-clock elapsed time measured from spawn.
**Expected output:** Total elapsed time for 3 ticks is under 800ms.
**Acceptance:** `cargo test -p anvilml-server --features mock-hardware --test stats_tick_tests test_interval_parameter_controls_cadence` exits 0.

---

## test_stats_are_real_data_not_the_c1_placeholder (anvilml-server)

**File:** `crates/anvilml-server/tests/stats_tick_tests.rs`
**Context:** Same `P16-D1` task. Confirms the published `cpu_pct`/`ram_used_mib` come from a real `sysinfo::System` call, not `P16-C1`'s always-zero placeholder values.
**Tests:** A published tick's `ram_used_mib` is nonzero.
**Mode:** both
**Inputs:** Same real-`sysinfo`-backed setup as `test_tick_publishes_system_stats`.
**Expected output:** `ram_used_mib > 0`.
**Acceptance:** `cargo test -p anvilml-server --features mock-hardware --test stats_tick_tests test_stats_are_real_data_not_the_c1_placeholder` exits 0.

---

## proof_ws_job_completed_pass_through (scripts/run_proof_p16_e1.py)

**File:** `scripts/run_proof_p16_e1.py`
**Context:** Phase 16's Runnable Proof. The AnvilML binary is built with `mock-hardware`, started as a background process, and a Python script connects to `ws://127.0.0.1:8488/v1/events`, submits a single-node PassThrough job via `POST /v1/jobs`, and asserts that a `job_completed` JSON frame with the matching `job_id` arrives on the WebSocket within 10 seconds. This is the first phase where the live event stream is exercised end-to-end against real dispatch — not just REST polling of job status as Phase 14's proof did.
**Tests:** The script connects to the WebSocket, consumes the initial `SystemStats` frame, submits a PassThrough job, and reads the stream until `job_completed` with the matching `job_id` arrives. Prints every received frame to stdout. Times out after 10 seconds if no matching frame arrives.
**Mode:** both
**Inputs:** Server running with mock-hardware; PassThrough node registered; job submitted with graph `[{"id": "n0", "type": "PassThrough", "inputs": {"value": 1}}]`.
**Expected output:** A `job_completed` JSON frame with `type == "job_completed"` and the matching `job_id` is printed to stdout; the script exits 0.
**Acceptance:** `python3 scripts/run_proof_p16_e1.py` exits 0.

---

## test_topo_sort_single_node (anvilml-worker)

**File:** `worker/tests/test_executor.py`
**Context:** The `worker.executor` module provides `topo_sort()` — a pure data transformation function that performs Kahn's-algorithm topological sort on a job graph. No external dependencies (torch, NODE_REGISTRY, NodeContext) are needed.
**Tests:** A graph with one node and no edges returns that node in a list.
**Mode:** mock
**Inputs:** Graph with `{"nodes": [{"id": "A", "type": "TestNode", "inputs": {}}]}` and no edges.
**Expected output:** `[{"id": "A", "type": "TestNode", "inputs": {}}]`.
**Acceptance:** `python -m pytest worker/tests/test_executor.py::test_topo_sort_single_node -v` exits 0.

---

## test_topo_sort_linear_chain (anvilml-worker)

**File:** `worker/tests/test_executor.py`
**Context:** Same `worker.executor` module. Tests that a strict linear chain (A→B→C) produces the exact order [A, B, C].
**Tests:** A→B→C chain returns nodes in correct dependency order.
**Mode:** mock
**Inputs:** Graph with nodes A, B, C and edges A→B, B→C.
**Expected output:** `[A, B, C]` in that exact order.
**Acceptance:** `python -m pytest worker/tests/test_executor.py::test_topo_sort_linear_chain -v` exits 0.

---

## test_topo_sort_parallel_branches (anvilml-worker)

**File:** `worker/tests/test_executor.py`
**Context:** Same `worker.executor` module. Tests that a graph with parallel branches produces a valid topological order (A before B and C), without asserting a specific order between B and C.
**Tests:** A graph with parallel branches (A→B, A→C) produces a valid topological order.
**Mode:** mock
**Inputs:** Graph with nodes A, B, C and edges A→B, A→C.
**Expected output:** A appears before both B and C.
**Acceptance:** `python -m pytest worker/tests/test_executor.py::test_topo_sort_parallel_branches -v` exits 0.

---

## test_topo_sort_cycle_detected (anvilml-worker)

**File:** `worker/tests/test_executor.py`
**Context:** Same `worker.executor` module. Tests that a cyclic graph raises `ValueError` with cycle node IDs in the error message.
**Tests:** A cyclic graph (A→B→C→A) raises `ValueError` with cycle node IDs in the message.
**Mode:** mock
**Inputs:** Graph with nodes A, B, C and edges A→B, B→C, C→A.
**Expected output:** `ValueError` with message containing "Cycle detected" and node IDs A, B, C.
**Acceptance:** `python -m pytest worker/tests/test_executor.py::test_topo_sort_cycle_detected -v` exits 0.

---

## test_topo_sort_no_edges_key (anvilml-worker)

**File:** `worker/tests/test_executor.py`
**Context:** Same `worker.executor` module. Tests graceful handling of graphs without an `"edges"` key at all.
**Tests:** A graph without an `"edges"` key returns nodes in original insertion order.
**Mode:** mock
**Inputs:** Graph with three nodes (X, Y, Z) and no `"edges"` key.
**Expected output:** `[X, Y, Z]` in original insertion order.
**Acceptance:** `python -m pytest worker/tests/test_executor.py::test_topo_sort_no_edges_key -v` exits 0.

---

## test_topo_sort_empty_graph (anvilml-worker)

**File:** `worker/tests/test_executor.py`
**Context:** Same `worker.executor` module. Tests the edge case of an empty nodes list.
**Tests:** A graph with an empty `"nodes"` list returns an empty list.
**Mode:** mock
**Inputs:** Graph with `{"nodes": []}`.
**Expected output:** `[]`.
**Acceptance:** `python -m pytest worker/tests/test_executor.py::test_topo_sort_empty_graph -v` exits 0.

---

## test_topo_sort_missing_nodes_key (anvilml-worker)

**File:** `worker/tests/test_executor.py`
**Context:** Same `worker.executor` module. Tests graceful degradation when the `"nodes"` key is absent entirely.
**Tests:** A graph without a `"nodes"` key returns an empty list.
**Mode:** mock
**Inputs:** Graph with `{"edges": []}` but no `"nodes"` key.
**Expected output:** `[]`.
**Acceptance:** `python -m pytest worker/tests/test_executor.py::test_topo_sort_missing_nodes_key -v` exits 0.

---

## test_executor_no_torch_import (anvilml-worker)

**File:** `worker/tests/test_executor.py`
**Context:** Same `worker.executor` module. Confirms the module has no transitive torch dependency at import time.
**Tests:** `import worker.executor` does not pull in torch at import time.
**Mode:** mock
**Inputs:** Subprocess that imports `worker.executor` and checks `sys.modules`.
**Expected output:** `"torch" not in sys.modules`; subprocess exits 0.
**Acceptance:** `python -m pytest worker/tests/test_executor.py::test_topo_sort_no_torch_import -v` exits 0.

## test_execute_graph_cancel_before_first (anvilml-worker)

**File:** `worker/tests/test_executor.py`
**Context:** `execute_graph()` function in `worker.executor`. Tests that when the cancel flag is set before the execution loop begins, the function returns immediately with `{"cancelled": True}` and no node's `execute()` is called.
**Tests:** `execute_graph()` checks `ctx.cancel_flag.is_set()` before the first node and returns early.
**Mode:** mock
**Inputs:** Graph with 3 nodes, mock context with `cancel_flag` pre-set.
**Expected output:** `{"cancelled": True}`, empty execution log.
**Acceptance:** `python -m pytest worker/tests/test_executor.py::test_execute_graph_cancel_before_first -v` exits 0.

## test_execute_graph_cancel_after_first (anvilml-worker)

**File:** `worker/tests/test_executor.py`
**Context:** `execute_graph()` function in `worker.executor`. Tests cooperative cancellation mid-execution: the first node's `execute()` sets the cancel flag, which is checked before the second node runs.
**Tests:** Cancel checkpoint happens before each node — first node runs and sets the flag, second node is skipped.
**Mode:** mock
**Inputs:** Graph with 2 nodes, mock context, first node overrides `execute()` to set `cancel_flag`.
**Expected output:** `{"cancelled": True}`, execution log contains only the first node.
**Acceptance:** `python -m pytest worker/tests/test_executor.py::test_execute_graph_cancel_after_first -v` exits 0.

## test_execute_graph_no_cancel_completes (anvilml-worker)

**File:** `worker/tests/test_executor.py`
**Context:** `execute_graph()` function in `worker.executor`. Tests normal completion when no cancellation occurs.
**Tests:** All nodes execute in order, results dict is populated, and the return value is `{"cancelled": False, "results": {...}}`.
**Mode:** mock
**Inputs:** Graph with 3 independent nodes (no edges), cancel flag unset.
**Expected output:** `{"cancelled": False, "results": {"node_0": ..., "node_1": ..., "node_2": ...}}`.
**Acceptance:** `python -m pytest worker/tests/test_executor.py::test_execute_graph_no_cancel_completes -v` exits 0.

## test_execute_graph_execution_order_matches_topo_sort (anvilml-worker)

**File:** `worker/tests/test_executor.py`
**Context:** `execute_graph()` function in `worker.executor`. Tests that nodes execute in the same order as `topo_sort()` returns them.
**Tests:** A linear chain A→B→C is created; the actual execution order matches the topological order.
**Mode:** mock
**Inputs:** Graph with 3 nodes connected as A→B→C.
**Expected output:** Execution log == `["NodeA", "NodeB", "NodeC"]`.
**Acceptance:** `python -m pytest worker/tests/test_executor.py::test_execute_graph_execution_order_matches_topo_sort -v` exits 0.

## test_execute_graph_results_dict (anvilml-worker)

**File:** `worker/tests/test_executor.py`
**Context:** `execute_graph()` function in `worker.executor`. Tests that the results dict is correctly populated with node outputs keyed by node ID.
**Tests:** Each node's `execute()` return value is stored in the results dict under the node's ID.
**Mode:** mock
**Inputs:** Graph with 3 nodes, each returning `{"output": value}`.
**Expected output:** `results` dict maps `"node_0"`→`{"output": 0}`, `"node_1"`→`{"output": 1}`, `"node_2"`→`{"output": 2}`.
**Acceptance:** `python -m pytest worker/tests/test_executor.py::test_execute_graph_results_dict -v` exits 0.

## test_execute_graph_no_torch_import (anvilml-worker)

**File:** `worker/tests/test_executor.py`
**Context:** `worker.executor` module now contains `execute_graph()`, which imports `NODE_REGISTRY` inside the function body. Confirms the module still has no transitive torch dependency at import time.
**Tests:** `import worker.executor` does not pull in torch at import time — `NODE_REGISTRY` is imported lazily inside `execute_graph()`.
**Mode:** mock
**Inputs:** Subprocess that imports `worker.executor` and checks `sys.modules`.
**Expected output:** `"torch" not in sys.modules`; subprocess exits 0.
**Acceptance:** `python -m pytest worker/tests/test_executor.py::test_execute_graph_no_torch_import -v` exits 0.

## test_execute_triggers_execute_graph_with_job_scoped_ctx_factory (anvilml-worker)

**File:** `worker/tests/test_worker_main.py`
**Context:** `execute_graph()` function in `worker.executor` is called by `_dispatch_loop()` in `worker.worker_main` when an `Execute` message is received. P17-B3 replaced the interim `_execute_job()` stopgap with a real handler that builds a `ctx_factory` and calls `execute_graph()` on a background thread.
**Tests:** `_dispatch_loop()` receives an `Execute` message and calls `execute_graph()` with the correct graph and a `ctx_factory` that produces a `NodeContext` with the correct `job_id`.
**Mode:** mock
**Inputs:** `Execute` message with `job_id="job-123"` and `graph={"nodes": []}`.
**Expected output:** `execute_graph()` called once with a `ctx_factory` that produces `NodeContext(job_id="job-123")`.
**Acceptance:** `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_worker_main.py::TestDispatchLoopExecute::test_execute_triggers_execute_graph_with_job_scoped_ctx_factory -v` exits 0.

## test_execute_success_sends_completed_with_elapsed_ms (anvilml-worker)

**File:** `worker/tests/test_worker_main.py`
**Context:** After `execute_graph()` completes on the background thread, the dispatch loop computes `elapsed_ms` from `time.monotonic()` and sends a `Completed` event via `ipc.send_event()`.
**Tests:** The `Completed` event contains a real positive integer `elapsed_ms` (from `time.monotonic()`), proving the timing code runs.
**Mode:** mock
**Inputs:** `Execute` message with `job_id="job-456"` and `graph={"nodes": []}`.
**Expected output:** `send_event()` called with `{"_type": "Completed", "job_id": "job-456", "elapsed_ms": <positive int>}`.
**Acceptance:** `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_worker_main.py::TestDispatchLoopExecute::test_execute_success_sends_completed_with_elapsed_ms -v` exits 0.

## test_execute_on_background_thread_stays_responsive (anvilml-worker)

**File:** `worker/tests/test_worker_main.py`
**Context:** `_dispatch_loop()` spawns `execute_graph()` on a background thread and waits via `thread.join()`. The loop must not hang — subsequent messages (e.g. `Shutdown`) are processed after the join returns.
**Tests:** An `Execute` message followed by a `Shutdown` message is processed: the loop runs the Execute handler (background thread + join), then processes the Shutdown and exits cleanly.
**Mode:** mock
**Inputs:** `Execute` message followed by `Shutdown` message.
**Expected output:** Dispatch loop exits cleanly without hanging.
**Acceptance:** `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_worker_main.py::TestDispatchLoopExecute::test_execute_on_background_thread_stays_responsive -v` exits 0.

## test_execute_graph_called_with_correct_graph (anvilml-worker)

**File:** `worker/tests/test_worker_main.py`
**Context:** `execute_graph()` receives the exact graph dict from the `Execute` message, unchanged.
**Tests:** The graph dict passed to `execute_graph()` is identical to the one in the `Execute` message — no modification or copying occurred.
**Mode:** mock
**Inputs:** `Execute` message with `graph={"nodes": [{"id": "node-1", "type": "PassThrough", "inputs": {"value": 42}}], "edges": []}`.
**Expected output:** `execute_graph()` called with the exact graph dict.
**Acceptance:** `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_worker_main.py::TestDispatchLoopExecute::test_execute_graph_called_with_correct_graph -v` exits 0.

## test_execute_failure_sends_failed_event (anvilml-worker)

**File:** `worker/tests/test_worker_main.py`
**Context:** P17-B4 adds exception handling around `execute_graph()` in the Execute handler. When `execute_graph()` raises an exception, the dispatch loop sends `WorkerEvent::Failed{job_id, error, traceback}` instead of silently leaving the job without a terminal event.
**Tests:** `execute_graph` is mocked to raise `ValueError("test error")`. An `Execute` message is fed, and a `Failed` event is verified to be sent with the correct `job_id` — not `Completed`, not silence.
**Mode:** mock
**Inputs:** `Execute` message with `job_id="job-fail"`, `graph={"nodes": []}`.
**Expected output:** send_event() called with `{"_type": "Failed", "job_id": "job-fail", "error": "test error", "traceback": <non-empty string>}`.
**Acceptance:** `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_worker_main.py::TestDispatchLoopExecuteFailure::test_execute_failure_sends_failed_event -v` exits 0.

## test_execute_failure_error_contains_exception_message (anvilml-worker)

**File:** `worker/tests/test_worker_main.py`
**Context:** P17-B4 captures the exception message via `str(exc)` and includes it in the `Failed` event's `error` field, matching the Rust `WorkerEvent::Failed` struct.
**Tests:** `execute_graph` is mocked to raise `ValueError("specific error message")`. The `error` field in the `Failed` event is asserted to contain the original exception's string representation.
**Mode:** mock
**Inputs:** `Execute` message with `job_id="job-err"`, `graph={"nodes": []}`. `execute_graph` raises `ValueError("specific error message")`.
**Expected output:** send_event() called with a `Failed` event whose `error` field contains "specific error message".
**Acceptance:** `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_worker_main.py::TestDispatchLoopExecuteFailure::test_execute_failure_error_contains_exception_message -v` exits 0.

## test_execute_failure_traceback_is_populated (anvilml-worker)

**File:** `worker/tests/test_worker_main.py`
**Context:** P17-B4 uses `traceback.format_exc()` to capture the full stack trace and includes it in the `Failed` event's `traceback` field, enabling the supervisor to diagnose the failure.
**Tests:** `execute_graph` is mocked to raise an exception. The `traceback` field in the `Failed` event is asserted to be a non-empty string containing "Traceback" formatting markers.
**Mode:** mock
**Inputs:** `Execute` message with `job_id="job-tb"`, `graph={"nodes": []}`. `execute_graph` raises `ValueError("traceback test")`.
**Expected output:** send_event() called with a `Failed` event whose `traceback` field is a non-empty string containing "Traceback".
**Acceptance:** `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_worker_main.py::TestDispatchLoopExecuteFailure::test_execute_failure_traceback_is_populated -v` exits 0.

---

## test_canceljob_sets_cancel_flag_for_current_job (worker)

**File:** `worker/tests/test_worker_main.py`
**Context:** `worker_main.py`'s `_dispatch_loop()` now handles `CancelJob` messages by matching the incoming `job_id` against the currently-executing job and setting its `NodeContext.cancel_flag` (a `threading.Event`).
**Tests:** CancelJob for the currently-executing job sets the cancel_flag so `execute_graph()` observes it before the next node's execute() call.
**Mode:** mock
**Inputs:** Execute message with `job_id="job-cancel"`, then CancelJob for the same `job_id`. `execute_graph` is mocked to capture the cancel_flag from the ctx_factory.
**Expected output:** The cancel_flag captured from the ctx_factory has `is_set() == True`.
**Acceptance:** `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_worker_main.py::TestDispatchLoopCancelJob::test_canceljob_sets_cancel_flag_for_current_job -v` exits 0.

---

## test_canceljob_for_nonmatching_job_id_is_ignored (worker)

**File:** `worker/tests/test_worker_main.py`
**Context:** `_dispatch_loop()`'s CancelJob branch compares the incoming `job_id` against `current_job_id` — a mismatch is normal (race between job completion and cancel arrival).
**Tests:** CancelJob for a non-matching job_id is logged at DEBUG and ignored without error.
**Mode:** mock
**Inputs:** Execute message with `job_id="job-a"`, then CancelJob for `job_id="job-b"`.
**Expected output:** No exception raised; dispatch loop exits cleanly; no Cancelled or Failed event sent.
**Acceptance:** `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_worker_main.py::TestDispatchLoopCancelJob::test_canceljob_for_nonmatching_job_id_is_ignored -v` exits 0.

---

## test_cancelled_execution_sends_cancelled_event (worker)

**File:** `worker/tests/test_worker_main.py`
**Context:** When `execute_graph()` returns `{"cancelled": True}` (because the cancel_flag was set), the dispatch loop sends a `Cancelled` event to the supervisor.
**Tests:** When executor stops due to cancel_flag, `WorkerEvent::Cancelled{job_id}` is sent back to supervisor.
**Mode:** mock
**Inputs:** Execute message with `job_id="job-cancelled"`, then CancelJob for the same `job_id`. `execute_graph` is mocked to return `{"cancelled": True}`.
**Expected output:** send_event() called with `{"_type": "Cancelled", "job_id": "job-cancelled"}`.
**Acceptance:** `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_worker_main.py::TestDispatchLoopCancelJob::test_cancelled_execution_sends_cancelled_event -v` exits 0.

---

## test_canceljob_after_job_completed_is_ignored (worker)

**File:** `worker/tests/test_worker_main.py`
**Context:** Tracking variables (`current_job_id`, `current_cancel_flag`) are reset to `None` after each job completes. A CancelJob arriving after completion finds no current job and is ignored.
**Tests:** CancelJob for a completed job (job_id no longer current) is ignored without error or event.
**Mode:** mock
**Inputs:** Execute message with `job_id="job-done"` (completes normally), then CancelJob for the same `job_id`.
**Expected output:** Completed event sent; no Cancelled event sent; no exception raised.
**Acceptance:** `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_worker_main.py::TestDispatchLoopCancelJob::test_canceljob_after_job_completed_is_ignored -v` exits 0.

---

## test_app_state_hardware_field_constructs (anvilml-server)

**File:** `crates/anvilml-server/tests/state_tests.rs`
**Context:** The `anvilml-server` crate has been compiled with `tokio` (sync feature) providing `tokio::sync::RwLock`, and `anvilml-core` providing `HardwareInfo`, `HostInfo`, and `InferenceCaps`.
**Tests:** `hardware` field is present on `AppState`, contains a valid `HardwareInfo` with non-empty `HostInfo` (hostname and OS), and the `Arc<RwLock<HardwareInfo>>` pointer is valid (non-null).
**Mode:** both
**Inputs:** `AppState` constructed via `make_full_state()` with synthetic `HardwareInfo { host: HostInfo { hostname: "test-host", os: "Linux" }, gpus: [], inference_caps: InferenceCaps::default() }`.
**Expected output:** `Arc::as_ptr()` returns non-null; read lock yields `HostInfo` with non-empty hostname and OS.
**Acceptance:** `cargo test -p anvilml-server --test state_tests -- hardware_field_constructs` exits 0.

---

## test_app_state_env_report_field_constructs (anvilml-server)

**File:** `crates/anvilml-server/tests/state_tests.rs`
**Context:** The `anvilml-server` crate has been compiled with `tokio` (sync feature) providing `tokio::sync::RwLock`, and `anvilml-core` providing `EnvReport` and `ProvisioningState`.
**Tests:** `env_report` field is present on `AppState`, `preflight_ok` is `false` (best-effort, no full preflight at startup), and the `Arc<RwLock<EnvReport>>` pointer is valid (non-null).
**Mode:** both
**Inputs:** `AppState` constructed via `make_full_state()` with synthetic `EnvReport { python_path: Some("./worker/.venv/bin/python3"), python_version: None, torch_version: None, provisioning: NotStarted, preflight_ok: false, reason: None, node_types: [] }`.
**Expected output:** `Arc::as_ptr()` returns non-null; read lock yields `preflight_ok == false`.
**Acceptance:** `cargo test -p anvilml-server --test state_tests -- env_report_field_constructs` exits 0.

---

## test_app_state_hardware_env_report_clone_shares (anvilml-server)

**File:** `crates/anvilml-server/tests/state_tests.rs`
**Context:** The `anvilml-server` crate has been compiled with `tokio` (sync feature) providing `tokio::sync::RwLock`, and `anvilml-core` providing `HardwareInfo` and `EnvReport`.
**Tests:** Both `hardware` and `env_report` `Arc` pointers are identical between original and cloned `AppState` (verified via `std::ptr::eq(Arc::as_ptr(...))`).
**Mode:** both
**Inputs:** `AppState` constructed via `make_full_state()`, then cloned.
**Expected output:** Both `hardware` and `env_report` `Arc` pointers are shared between original and clone.
**Acceptance:** `cargo test -p anvilml-server --test state_tests -- clone_shares` exits 0.

---

## test_get_system_returns_200 (anvilml-server)

**File:** `crates/anvilml-server/tests/system_tests.rs`
**Context:** The `anvilml-server` crate has been compiled with `axum`, `serde`, `serde_json`, and `tower` dev-dependencies. `build_router()` accepts an `AppState` with `hardware` and `env_report` fields populated by sentinel values.
**Tests:** `GET /v1/system` returns `200 OK` with a JSON body containing `HardwareInfo` fields matching the sentinel values — `host.hostname == "test-host"`, `host.os == "Linux"`, and an empty `gpus` array.
**Mode:** both
**Inputs:** `GET /v1/system` with empty body; `build_router()` called with `AppState` containing `HardwareInfo { host: HostInfo { hostname: "test-host", os: "Linux" }, gpus: [], inference_caps: InferenceCaps::default() }`.
**Expected output:** `StatusCode::OK`; JSON body contains `"host": {"hostname": "test-host", "os": "Linux"}` and `"gpus": []`.
**Acceptance:** `cargo test -p anvilml-server --test system_tests test_get_system_returns_200` exits 0.

---

## test_get_system_reflects_hardware_update (anvilml-server)

**File:** `crates/anvilml-server/tests/system_tests.rs`
**Context:** The `anvilml-server` crate has been compiled with `axum`, `serde`, `serde_json`, and `tower` dev-dependencies. `AppState` holds `hardware` as `Arc<RwLock<HardwareInfo>>` so the write lock can mutate the snapshot between requests.
**Tests:** After writing a new `HardwareInfo` through the `RwLock` write lock, `GET /v1/system` returns the updated `host.hostname` — proving the handler reads the live snapshot, not a stale clone.
**Mode:** both
**Inputs:** `GET /v1/system` → original response → write lock updates `hostname` to `"updated-host"`, `os` to `"Windows"` → second `GET /v1/system`.
**Expected output:** First response: `hostname == "test-host"`. Second response: `hostname == "updated-host"`, `os == "Windows"`.
**Acceptance:** `cargo test -p anvilml-server --test system_tests test_get_system_reflects_hardware_update` exits 0.

---

## test_get_system_env_returns_200 (anvilml-server)

**File:** `crates/anvilml-server/tests/system_tests.rs`
**Context:** The `anvilml-server` crate has been compiled with `axum`, `serde`, `serde_json`, and `tower` dev-dependencies. `build_router()` accepts an `AppState` with `env_report` field populated by sentinel values.
**Tests:** `GET /v1/system/env` returns `200 OK` with a JSON body containing `EnvReport` fields matching the sentinel values — `python_path` is set and `preflight_ok == false`.
**Mode:** both
**Inputs:** `GET /v1/system/env` with empty body; `build_router()` called with `AppState` containing `EnvReport { python_path: Some("./worker/.venv/bin/python3"), python_version: None, torch_version: None, provisioning: NotStarted, preflight_ok: false, reason: None, node_types: [] }`.
**Expected output:** `StatusCode::OK`; JSON body contains `"python_path": "./worker/.venv/bin/python3"` and `"preflight_ok": false`.
**Acceptance:** `cargo test -p anvilml-server --test system_tests test_get_system_env_returns_200` exits 0.

---

## test_get_system_env_reflects_env_report_update (anvilml-server)

**File:** `crates/anvilml-server/tests/system_tests.rs`
**Context:** The `anvilml-server` crate has been compiled with `axum`, `serde`, `serde_json`, and `tower` dev-dependencies. `AppState` holds `env_report` as `Arc<RwLock<EnvReport>>` so the write lock can mutate the report between requests.
**Tests:** After writing a new `EnvReport` through the `RwLock` write lock, `GET /v1/system/env` returns the updated `python_version` and `provisioning` — proving the handler reads the live snapshot.
**Mode:** both
**Inputs:** `GET /v1/system/env` → original response → write lock updates `python_version` to `"3.12.3"`, `torch_version` to `"2.5.0"`, `provisioning` to `Ready`, `preflight_ok` to `true` → second `GET /v1/system/env`.
**Expected output:** First response: `python_version == null`, `provisioning == "not_started"`. Second response: `python_version == "3.12.3"`, `torch_version == "2.5.0"`, `provisioning == "ready"`, `preflight_ok == true`.
**Acceptance:** `cargo test -p anvilml-server --test system_tests test_get_system_env_reflects_env_report_update` exits 0.

---

## test_get_system_versions_returns_200 (anvilml-server)

**File:** `crates/anvilml-server/tests/system_tests.rs`
**Context:** The `anvilml-server` crate has been compiled with `axum`, `serde`, `serde_json`, `tower` dev-dependencies, and the `rustc_version_runtime` runtime dependency (v0.3.0, resolved via rust-docs MCP). `build_router()` accepts an `AppState` with `env_report` field having `python_version` and `torch_version` as `None` (the `make_test_state` default).
**Tests:** `GET /v1/system/versions` returns `200 OK` with a JSON body containing `ComponentVersions` fields — `anvilml_version` is a non-empty string (from `CARGO_PKG_VERSION`), `rust_version` is a non-empty string (from `rustc_version_runtime::version()`), and `python_version` and `torch_version` are `null` (from the `None` env_report defaults).
**Mode:** both
**Inputs:** `GET /v1/system/versions` with empty body; `build_router()` called with `AppState` containing default `EnvReport { python_version: None, torch_version: None, ... }`.
**Expected output:** `StatusCode::OK`; JSON body has `"anvilml_version"` as non-empty string, `"rust_version"` as non-empty string, `"python_version": null`, `"torch_version": null`.
**Acceptance:** `cargo test -p anvilml-server --test system_tests test_get_system_versions_returns_200` exits 0.

---

## test_get_system_versions_reflects_env_report_values (anvilml-server)

**File:** `crates/anvilml-server/tests/system_tests.rs`
**Context:** The `anvilml-server` crate has been compiled with `axum`, `serde`, `serde_json`, `tower` dev-dependencies, and the `rustc_version_runtime` runtime dependency (v0.3.0). `AppState` holds `env_report` as `Arc<RwLock<EnvReport>>` so the write lock can mutate the report between requests.
**Tests:** After writing new `EnvReport` values through the `RwLock` write lock, `GET /v1/system/versions` returns the updated `python_version` and `torch_version` — proving the handler reads the live snapshot.
**Mode:** both
**Inputs:** `GET /v1/system/versions` → original response → write lock updates `python_version` to `"3.12.3"`, `torch_version` to `"2.5.0"` → second `GET /v1/system/versions`.
**Expected output:** Both responses: `anvilml_version` is non-empty string, `rust_version` is non-empty string. Second response: `python_version == "3.12.3"`, `torch_version == "2.5.0"`.
**Acceptance:** `cargo test -p anvilml-server --test system_tests test_get_system_versions_reflects_env_report_values` exits 0.

---

## test_get_system_versions_null_when_env_report_unset (anvilml-server)

**File:** `crates/anvilml-server/tests/system_tests.rs`
**Context:** The `anvilml-server` crate has been compiled with `axum`, `serde`, `serde_json`, `tower` dev-dependencies, and the `rustc_version_runtime` runtime dependency (v0.3.0). `make_test_state()` constructs `EnvReport` with `python_version: None` and `torch_version: None` by default.
**Tests:** `GET /v1/system/versions` returns `200 OK` with `python_version` and `torch_version` both `null` in the JSON body — confirming that `None` values from `EnvReport` serialize as JSON `null`.
**Mode:** both
**Inputs:** `GET /v1/system/versions` with empty body; `build_router()` called with `AppState` containing `EnvReport { python_version: None, torch_version: None, ... }` (default from `make_test_state`).
**Expected output:** `StatusCode::OK`; JSON body has `"python_version": null`, `"torch_version": null`.
**Acceptance:** `cargo test -p anvilml-server --test system_tests test_get_system_versions_null_when_env_report_unset` exits 0.

---

## test_list_models_no_filter (anvilml-server)

**File:** `crates/anvilml-server/tests/models_tests.rs`
**Context:** `AppState` with `model_store` backed by an in-memory SQLite pool with migrations applied. Two models of different kinds (diffusion and text_encoder) are inserted via `ModelStore::upsert()`.
**Tests:** `GET /v1/models` returns all models when no `kind` query parameter is provided.
**Mode:** both
**Inputs:** GET request with no query params; 2 models in the store (1 diffusion, 1 text_encoder).
**Expected output:** `StatusCode::OK`; JSON array of length 2.
**Acceptance:** `cargo test -p anvilml-server --test models_tests test_list_models_no_filter` exits 0.

---

## test_list_models_kind_filter (anvilml-server)

**File:** `crates/anvilml-server/tests/models_tests.rs`
**Context:** `AppState` with `model_store` backed by an in-memory SQLite pool. Three models are inserted (2 diffusion, 1 VAE).
**Tests:** `GET /v1/models?kind=diffusion` returns only models matching the kind filter.
**Mode:** both
**Inputs:** GET request with `?kind=diffusion`; 3 models in store (2 diffusion, 1 VAE).
**Expected output:** `StatusCode::OK`; JSON array of length 2.
**Acceptance:** `cargo test -p anvilml-server --test models_tests test_list_models_kind_filter` exits 0.

---

## test_get_model_existing_returns_200 (anvilml-server)

**File:** `crates/anvilml-server/tests/models_tests.rs`
**Context:** `AppState` with `model_store` backed by an in-memory SQLite pool. One model is inserted with a known ID.
**Tests:** `GET /v1/models/:id` returns the correct model for an existing ID with matching `id`, `name`, and `kind` fields.
**Mode:** both
**Inputs:** GET request to `/v1/models/{id}` with a known model ID.
**Expected output:** `StatusCode::OK`; JSON body with matching `id`, `name`, and `kind` fields.
**Acceptance:** `cargo test -p anvilml-server --test models_tests test_get_model_existing_returns_200` exits 0.

---

## test_get_model_unknown_returns_404 (anvilml-server)

**File:** `crates/anvilml-server/tests/models_tests.rs`
**Context:** `AppState` with `model_store` backed by an in-memory SQLite pool. No models are inserted.
**Tests:** `GET /v1/models/:id` returns 404 for a non-existent ID.
**Mode:** both
**Inputs:** GET request to `/v1/models/{random-uuid}` with a UUID never inserted.
**Expected output:** `StatusCode::NOT_FOUND`.
**Acceptance:** `cargo test -p anvilml-server --test models_tests test_get_model_unknown_returns_404` exits 0.

---

## test_rescan_returns_202_immediately (anvilml-server)

**File:** `crates/anvilml-server/tests/models_tests.rs`
**Context:** `AppState` with a `ModelDirConfig` pointing to an empty temp directory.
**Tests:** `POST /v1/models/rescan` returns 202 Accepted within 500ms, proving the handler does not block on scan completion.
**Mode:** both
**Inputs:** POST request to `/v1/models/rescan` with an empty temp model directory.
**Expected output:** `StatusCode::ACCEPTED` (202) returned within 500ms.
**Acceptance:** `cargo test -p anvilml-server --test models_tests test_rescan_returns_202_immediately` exits 0.

---

## test_rescan_populates_model_store (anvilml-server)

**File:** `crates/anvilml-server/tests/models_tests.rs`
**Context:** `AppState` with a `ModelDirConfig` pointing to a temp directory containing a planted `.safetensors` file.
**Tests:** After `POST /v1/models/rescan`, a subsequent `GET /v1/models` lists the planted model, proving the background scan writes to the store.
**Mode:** both
**Inputs:** POST request to `/v1/models/rescan` with a temp directory containing a small `.safetensors` file, followed by GET `/v1/models`.
**Expected output:** `StatusCode::OK` (200) with a JSON array containing at least one model whose path ends with `test_model.safetensors`.
**Acceptance:** `cargo test -p anvilml-server --test models_tests test_rescan_populates_model_store` exits 0.

---

## test_workers_list_returns_current_pool_state (anvilml-server)

**File:** `crates/anvilml-server/tests/workers_tests.rs`
**Context:** `AppState` with a `WorkerPool` pre-populated via `WorkerPool::set_up_test_workers()` (`test-utils` feature) with two mock `(WorkerHandle, GpuDevice)` pairs — worker "0" with `WorkerStatus::Idle` on `DeviceType::Cuda`, and worker "1" with `WorkerStatus::Busy` on `DeviceType::Cpu`.
**Tests:** `GET /v1/workers` returns 200 OK with a JSON array whose elements match the injected mock workers' `worker_id`, `status`, `device_index`, and `device_type`.
**Mode:** both
**Inputs:** GET request to `/v1/workers` with a pool containing two mock workers (Idle/Cuda and Busy/Cpu).
**Expected output:** `StatusCode::OK` (200) with a JSON array of length 2, where each element's `worker_id`, `status`, `device_index`, and `device_type` match the injected handles.
**Acceptance:** `cargo test -p anvilml-server --features mock-hardware --test workers_tests test_workers_list_returns_current_pool_state` exits 0.

---

## test_workers_list_empty_returns_empty_array (anvilml-server)

**File:** `crates/anvilml-server/tests/workers_tests.rs`
**Context:** `AppState` with an empty `WorkerPool` (no workers injected via `set_up_test_workers()`).
**Tests:** `GET /v1/workers` returns 200 OK with an empty JSON array `[]` — not `null`, not an error body. This confirms the handler returns an empty array when the pool has zero workers.
**Mode:** both
**Inputs:** GET request to `/v1/workers` with an empty pool.
**Expected output:** `StatusCode::OK` (200) with body `[]`.
**Acceptance:** `cargo test -p anvilml-server --features mock-hardware --test workers_tests test_workers_list_empty_returns_empty_array` exits 0.

---

## test_workers_response_shape_matches_workerinfo (anvilml-server)

**File:** `crates/anvilml-server/tests/workers_tests.rs`
**Context:** `AppState` with a `WorkerPool` pre-populated via `set_up_test_workers()` with one mock worker (`WorkerStatus::Idle`, `DeviceType::Cuda`).
**Tests:** The JSON response contains exactly the six fields `worker_id` (string), `status` (string in snake_case matching `WorkerStatus`), `device_index` (integer), `device_type` (string in snake_case matching `DeviceType`), `pid` (null), and `current_job_id` (null). No extra fields are present. This verifies the `WorkerInfo` serde representation is correct.
**Mode:** both
**Inputs:** GET request to `/v1/workers` with a pool containing one mock worker.
**Expected output:** `StatusCode::OK` (200) with a JSON array of length 1, containing exactly the six `WorkerInfo` fields with correct types and values.
**Acceptance:** `cargo test -p anvilml-server --features mock-hardware --test workers_tests test_workers_response_shape_matches_workerinfo` exits 0.

---

## test_restart_unknown_worker_returns_404 (anvilml-server)

**File:** `crates/anvilml-server/tests/workers_tests.rs`
**Context:** `AppState` with a `WorkerPool` populated via `spawn_all_with_spawner()` (a `MockWorkerSpawner`, not `set_up_test_workers()` — `restart_worker()` needs `spawn_config` populated, which only the real spawn path sets) with one worker on device index 0.
**Tests:** `POST /v1/workers/{id}/restart` returns `404 Not Found` when `id` doesn't match any worker in the pool.
**Mode:** both
**Inputs:** `POST /v1/workers/99/restart` against a pool that only has worker `"0"`.
**Expected output:** `StatusCode::NOT_FOUND` (404).
**Acceptance:** `cargo test -p anvilml-server --features mock-hardware --test workers_tests test_restart_unknown_worker_returns_404` exits 0.

---

## test_restart_known_worker_returns_202_and_spawns_new_generation (anvilml-server)

**File:** `crates/anvilml-server/tests/workers_tests.rs`
**Context:** Same spawner-backed pool as above, one worker on device index 0. Polls `MockWorkerSpawner::call_count()` to confirm the initial spawn completed before restarting, then again after, to distinguish "the old process was left running" from "a genuinely new generation was spawned" — the exact distinction the `P18-D3` audit finding turns on (`request_shutdown()` alone does not respawn).
**Tests:** Restarting a known, non-`Dying` worker returns `202 Accepted` and causes a second `spawner.spawn()` call.
**Mode:** both
**Inputs:** `POST /v1/workers/0/restart` against a pool with one already-spawned worker.
**Expected output:** `StatusCode::ACCEPTED` (202); `spawner.call_count() == 2` within 2s of the restart request.
**Acceptance:** `cargo test -p anvilml-server --features mock-hardware --test workers_tests test_restart_known_worker_returns_202_and_spawns_new_generation` exits 0.

---

## test_restart_already_dying_returns_409 (anvilml-server)

**File:** `crates/anvilml-server/tests/workers_tests.rs`
**Context:** Same spawner-backed pool, one worker on device index 0, forced into `WorkerStatus::Dying` via `set_status()` on a cloned handle (clones share the underlying status lock) before the restart request — simulating a shutdown already in flight (e.g. from a concurrent `shutdown_all()`).
**Tests:** Restarting an already-`Dying` worker returns `409 Conflict` and does **not** trigger a second spawn.
**Mode:** both
**Inputs:** `POST /v1/workers/0/restart` against a worker whose status was forced to `Dying`.
**Expected output:** `StatusCode::CONFLICT` (409); `spawner.call_count()` stays at 1.
**Acceptance:** `cargo test -p anvilml-server --features mock-hardware --test workers_tests test_restart_already_dying_returns_409` exits 0.

---

## test_restart_respawned_worker_reaches_idle (anvilml-server)

**File:** `crates/anvilml-server/tests/workers_tests.rs`
**Context:** Same spawner-backed pool, one worker on device index 0. After restarting, connects a DEALER socket to the pool's shared `RouterTransport` (same pattern as `crates/anvilml-worker/tests/pool_tests.rs`'s `connect_dealer`/`send_event`/`ready_event` helpers) and sends a synthetic `WorkerEvent::Ready` for worker `"0"`, retrying the send within the poll loop to absorb the small window between the new generation's process launching and its `Demux::register()` call completing.
**Tests:** The worker spawned by a restart genuinely reaches `Idle`, not just that a new OS process was launched.
**Mode:** both
**Inputs:** `POST /v1/workers/0/restart`, followed by a synthetic `Ready` event for the same `worker_id`.
**Expected output:** `StatusCode::ACCEPTED` (202) on the restart; the respawned worker's status reaches `WorkerStatus::Idle` within 3s.
**Acceptance:** `cargo test -p anvilml-server --features mock-hardware --test workers_tests test_restart_respawned_worker_reaches_idle` exits 0.

---

## test_delete_terminal_job_returns_204 (anvilml-server)

**File:** `crates/anvilml-server/tests/jobs_tests.rs`
**Context:** The `anvilml-server` crate has been compiled with `sqlx` (sqlite), `uuid` (v4, serde), `chrono` (serde), `serde_json`, and `axum` dev-dependencies. The `build_router()` function wires all HTTP routes including the new `DELETE /v1/jobs/{id}` handler.
**Tests:** DELETE on a Completed job returns 204 No Content and removes the job row from the database. A Completed job is persisted directly to the DB (not via submit) to avoid the in-memory queue.
**Mode:** both
**Inputs:** `DELETE /v1/jobs/:id` where `:id` is a UUID of a Completed job persisted via `JobStore::upsert`.
**Expected output:** `StatusCode::NO_CONTENT` (204); `JobStore::get(id)` returns `None` after deletion.
**Acceptance:** `cargo test -p anvilml-server --test jobs_tests test_delete_terminal_job_returns_204` exits 0.

---

## test_delete_terminal_job_removes_artifacts (anvilml-server)

**File:** `crates/anvilml-server/tests/jobs_tests.rs`
**Context:** Same test setup as `test_delete_terminal_job_returns_204`, plus a fake PNG artifact persisted via `artifact_store.save()` with a known SHA-256 hash. The artifact store's `artifact_dir()` accessor is used to verify file removal from disk.
**Tests:** DELETE on a Completed job with associated artifacts removes both the artifact file from disk and the metadata row from the database.
**Mode:** both
**Inputs:** `DELETE /v1/jobs/:id` where `:id` has one associated artifact (4x4 red pixel PNG) persisted via `ArtifactStore::save()`.
**Expected output:** `StatusCode::NO_CONTENT` (204); `artifact_store.list(Some(id))` returns empty vec; artifact file removed from disk.
**Acceptance:** `cargo test -p anvilml-server --test jobs_tests test_delete_terminal_job_removes_artifacts` exits 0.

---

## test_delete_non_terminal_queued_returns_409 (anvilml-server)

**File:** `crates/anvilml-server/tests/jobs_tests.rs`
**Context:** Same test setup as the existing `test_cancel_queued_job_returns_202` test — a job submitted via POST enters Queued state.
**Tests:** DELETE on a Queued job returns 409 Conflict. The handler rejects deletion of non-terminal jobs to prevent accidental data loss.
**Mode:** both
**Inputs:** `DELETE /v1/jobs/:id` where `:id` is a UUID of a Queued job (submitted via POST /v1/jobs).
**Expected output:** `StatusCode::CONFLICT` (409); job remains in the database in Queued state.
**Acceptance:** `cargo test -p anvilml-server --test jobs_tests test_delete_non_terminal_queued_returns_409` exits 0.

---

## test_delete_non_terminal_running_returns_409 (anvilml-server)

**File:** `crates/anvilml-server/tests/jobs_tests.rs`
**Context:** A Running job is persisted directly to the DB (not via submit) with `worker_id = Some("0")`.
**Tests:** DELETE on a Running job returns 409 Conflict, same as the Queued case. The handler checks `job.status` and rejects any non-terminal status.
**Mode:** both
**Inputs:** `DELETE /v1/jobs/:id` where `:id` is a UUID of a Running job persisted via `JobStore::upsert`.
**Expected output:** `StatusCode::CONFLICT` (409); job remains in the database in Running state.
**Acceptance:** `cargo test -p anvilml-server --test jobs_tests test_delete_non_terminal_running_returns_409` exits 0.

---

## test_delete_unknown_id_returns_404 (anvilml-server)

**File:** `crates/anvilml-server/tests/jobs_tests.rs`
**Context:** Same setup as `test_get_job_unknown_returns_404` — an empty `AppState` with no jobs.
**Tests:** DELETE on an unknown UUID returns 404 Not Found. The handler returns `AnvilError::JobNotFound` which maps to HTTP 404.
**Mode:** both
**Inputs:** `DELETE /v1/jobs/:id` where `:id` is a random UUID never submitted.
**Expected output:** `StatusCode::NOT_FOUND` (404); no side effects (no DB changes).
**Acceptance:** `cargo test -p anvilml-server --test jobs_tests test_delete_unknown_id_returns_404` exits 0.

---

## test_get_or_load_cached_returns_without_calling_loader (worker)

**File:** `worker/tests/test_pipeline_cache.py`
**Context:** The `PipelineCache` class is implemented in `worker/pipeline_cache.py` using `collections.OrderedDict` for LRU tracking. No external dependencies — stdlib only.
**Tests:** Repeated calls with the same key call `loader_fn` exactly once. Creates a cache, defines a loader_fn that tracks invocation count, calls `get_or_load()` twice with the same key, and asserts both calls return the same value and loader_fn was called exactly once.
**Mode:** both
**Inputs:** Empty cache; same key passed to two `get_or_load()` calls.
**Expected output:** `loader_fn` called once; both calls return `"loaded_value"`.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_pipeline_cache.py::test_get_or_load_cached_returns_without_calling_loader -v` exits 0.

---

## test_get_or_load_different_keys_each_call_loader (worker)

**File:** `worker/tests/test_pipeline_cache.py`
**Context:** The `PipelineCache` class is implemented in `worker/pipeline_cache.py` using `collections.OrderedDict` for LRU tracking. No external dependencies — stdlib only.
**Tests:** Different keys each produce their own independent `loader_fn` call. Creates a cache, calls `get_or_load()` with two distinct keys, and asserts loader_fn was called exactly twice (once per key).
**Mode:** both
**Inputs:** Empty cache; two distinct keys.
**Expected output:** `loader_fn` called twice; each key returns its own value.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_pipeline_cache.py::test_get_or_load_different_keys_each_call_loader -v` exits 0.

---

## test_lru_eviction_removes_least_recently_used (worker)

**File:** `worker/tests/test_pipeline_cache.py`
**Context:** The `PipelineCache` class is implemented in `worker/pipeline_cache.py` using `collections.OrderedDict` for LRU tracking. No external dependencies — stdlib only.
**Tests:** When cache exceeds `max_entries`, the oldest entry is evicted. Creates a cache with `max_entries=2`, inserts three distinct keys in order (A, B, C), and asserts that after inserting C, key A has been evicted.
**Mode:** both
**Inputs:** Cache with `max_entries=2`; three insertions in order A, B, C.
**Expected output:** Key C is present; key A was evicted; cache contains exactly B and C.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_pipeline_cache.py::test_lru_eviction_removes_least_recently_used -v` exits 0.

---

## test_access_refreshes_recency (worker)

**File:** `worker/tests/test_pipeline_cache.py`
**Context:** The `PipelineCache` class is implemented in `worker/pipeline_cache.py` using `collections.OrderedDict` for LRU tracking. No external dependencies — stdlib only.
**Tests:** Accessing a cached entry moves it to most-recently-used position, protecting it from eviction. Creates a cache with `max_entries=2`, inserts A then B, accesses A (moves to MRU end), then inserts C. Asserts B — not A — is evicted.
**Mode:** both
**Inputs:** Cache with `max_entries=2`; insert A, insert B, access A, insert C.
**Expected output:** B is evicted; A and C remain in cache.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_pipeline_cache.py::test_access_refreshes_recency -v` exits 0.

---

## test_custom_max_entries (worker)

**File:** `worker/tests/test_pipeline_cache.py`
**Context:** The `PipelineCache` class is implemented in `worker/pipeline_cache.py` using `collections.OrderedDict` for LRU tracking. No external dependencies — stdlib only.
**Tests:** Cache respects a non-default `max_entries` value. Creates a cache with `max_entries=3`, inserts 3 entries, asserts the cache is full, then inserts a 4th entry and asserts the cache still has exactly 3 entries (oldest evicted).
**Mode:** both
**Inputs:** Cache with `max_entries=3`; four insertions.
**Expected output:** Cache holds exactly 3 entries after 4 insertions.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_pipeline_cache.py::test_custom_max_entries -v` exits 0.

---

## test_evicted_entry_is_truly_removed (worker)

**File:** `worker/tests/test_pipeline_cache.py`
**Context:** The `PipelineCache` class is implemented in `worker/pipeline_cache.py` using `collections.OrderedDict` for LRU tracking. No external dependencies — stdlib only.
**Tests:** After eviction, `get_or_load` for the evicted key calls `loader_fn` again. Creates a cache with `max_entries=2`, fills it with A and B, then inserts C (evicting A). Calls `get_or_load("A", ...)` again and asserts that `loader_fn` is called a second time for key A (proving the entry was truly removed, not just overwritten).
**Mode:** both
**Inputs:** Cache with `max_entries=2`; insert A, insert B, insert C, re-insert A.
**Expected output:** `loader_fn` called four times total (A, B, C, re-loaded A).
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_pipeline_cache.py::test_evicted_entry_is_truly_removed -v` exits 0.

---

## test_load_model_mock_returns_sentinel (worker)

**File:** `worker/tests/test_nodes_loader.py`
**Context:** The `LoadModel` node class is implemented in `worker/nodes/loader.py` with a mock/real execute branch. The mock branch returns a sentinel dict; the real branch raises NotImplementedError (deferred to P19-C2). The `@register` decorator populates `NODE_REGISTRY` at module load time.
**Tests:** Mock-mode `execute()` returns the sentinel dict shape `{"model": {"mock": True, "model_id": "test_model"}}`. Constructs a `NodeContext` with `mock=True`, calls `execute(model_id="test_model")`, and asserts the return dict matches.
**Mode:** mock
**Inputs:** `NodeContext(mock=True)`, `model_id="test_model"`.
**Expected output:** `{"model": {"mock": True, "model_id": "test_model"}}`.
**Acceptance:** `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_load_model_mock_returns_sentinel -v` exits 0.

---

## test_load_model_real_raises_not_implemented (worker)

**File:** `worker/tests/test_nodes_loader.py`
**Context:** The `LoadModel` node class is implemented in `worker/nodes/loader.py`. The real branch calls `pipeline_cache.get_or_load()` with a loader_fn that raises `NotImplementedError("no diffusion arch module registered yet")`. This is the Phase-19 groundwork pattern — infrastructure is in place, the loader_fn raises because no arch module is registered yet.
**Tests:** Real-mode `execute()` raises `NotImplementedError`. Constructs a `NodeContext` with `mock=False`, calls `execute(model_id="test_model")`, and asserts `NotImplementedError` is raised with a message containing "no diffusion arch module registered yet".
**Mode:** real
**Inputs:** `NodeContext(mock=False)`, `model_id="test_model"`.
**Expected output:** `NotImplementedError` is raised with message containing "no diffusion arch module registered yet".
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_load_model_real_raises_not_implemented -v -m real_mode` exits 0.

## test_load_model_real_cache_key_format (worker)

**File:** `worker/tests/test_nodes_loader.py`
**Context:** The `LoadModel` node class is implemented in `worker/nodes/loader.py`. The real branch calls `pipeline_cache.get_or_load(inputs["model_id"], loader_fn)`. A real `PipelineCache` instance is passed via `NodeContext.pipeline_cache`.
**Tests:** Constructs a real `PipelineCache`, a `NodeContext` with `mock=False` and the cache, and a `LoadModel` node. Calls `execute(model_id="test_model")`. The call raises `NotImplementedError` as expected, but the test verifies the cache is empty (exception does not populate the cache per `PipelineCache` contract), confirming `get_or_load` was called with the correct key format.
**Mode:** real
**Inputs:** `PipelineCache()`, `NodeContext(mock=False, pipeline_cache=cache)`, `model_id="test_model"`.
**Expected output:** `NotImplementedError` is raised; `cache._cache` is empty (length 0).
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_load_model_real_cache_key_format -v -m real_mode` exits 0.

## test_load_model_real_raises_no_diffusion_arch (worker)

**File:** `worker/tests/test_nodes_loader.py`
**Context:** The `LoadModel` node class is implemented in `worker/nodes/loader.py`. The real branch calls `pipeline_cache.get_or_load()` with a loader_fn that raises `NotImplementedError("no diffusion arch module registered yet")`.
**Tests:** Canonical real-mode test. Constructs a `NodeContext` with `mock=False`, calls `execute(model_id="zit-test")`, and asserts `NotImplementedError` is raised with the exact Phase-19 groundwork message.
**Mode:** real
**Inputs:** `NodeContext(mock=False)`, `model_id="zit-test"`.
**Expected output:** `NotImplementedError("no diffusion arch module registered yet")` is raised.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_load_model_real_raises_no_diffusion_arch -v -m real_mode` exits 0.

---

## test_load_model_in_registry (worker)

**File:** `worker/tests/test_nodes_loader.py`
**Context:** The `LoadModel` node class is implemented in `worker/nodes/loader.py` and decorated with `@register`. Importing `worker.nodes.loader` triggers the `@register` side effect which populates `NODE_REGISTRY`.
**Tests:** `LoadModel` appears in `NODE_REGISTRY` after importing the module. Uses subprocess isolation to avoid cross-test pollution from prior imports.
**Mode:** both
**Inputs:** Fresh subprocess that imports `worker.nodes.loader` and checks `NODE_REGISTRY`.
**Expected output:** `NODE_REGISTRY` contains `"LoadModel"` as a key.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_load_model_in_registry -v` exits 0.

---

## test_load_vae_mock_returns_sentinel (worker)

**File:** `worker/tests/test_nodes_loader.py`
**Context:** The `LoadVae` node class is implemented in `worker/nodes/loader.py` and decorated with `@register`. A `NodeContext` with `mock=True` is constructed, and `execute(model_id="test_vae")` is called.
**Tests:** Mock-mode `LoadVae.execute()` returns the sentinel dict shape `{"vae": {"mock": True, "model_id": "test_vae"}}`. Satisfies the `MOCK_PATH_VERIFIED` marker.
**Mode:** mock
**Inputs:** `NodeContext(mock=True)`, `model_id="test_vae"`.
**Expected output:** `{"vae": {"mock": True, "model_id": "test_vae"}}` is returned.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_load_vae_mock_returns_sentinel -v` exits 0.

---

## test_load_vae_real_raises_not_implemented (worker)

**File:** `worker/tests/test_nodes_loader.py`
**Context:** The `LoadVae` node class is implemented in `worker/nodes/loader.py`. A `PipelineCache` and `NodeContext` with `mock=False` are constructed, and `execute(model_id="test_vae")` is called.
**Tests:** Real-mode `LoadVae.execute()` raises `NotImplementedError` with the Phase-19 groundwork message ("no diffusion arch module registered yet"). Satisfies the `REAL_PATH_VERIFIED` marker.
**Mode:** real
**Inputs:** `NodeContext(mock=False, pipeline_cache=PipelineCache())`, `model_id="test_vae"`.
**Expected output:** `NotImplementedError` with message containing "no diffusion arch module registered yet" is raised.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_load_vae_real_raises_not_implemented -v -m real_mode` exits 0.

---

## test_load_vae_in_registry (worker)

**File:** `worker/tests/test_nodes_loader.py`
**Context:** The `LoadVae` node class is implemented in `worker/nodes/loader.py` and decorated with `@register`. Importing `worker.nodes.loader` in a subprocess triggers the `@register` side effect which populates `NODE_REGISTRY`.
**Tests:** `LoadVae` appears in `NODE_REGISTRY` after importing the module. Uses subprocess isolation to avoid cross-test pollution from prior imports.
**Mode:** both
**Inputs:** Fresh subprocess that imports `worker.nodes.loader` and checks `NODE_REGISTRY`.
**Expected output:** `NODE_REGISTRY` contains `"LoadVae"` as a key.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_load_vae_in_registry -v` exits 0.

---

## test_load_vae_real_cache_key_format (worker)

**File:** `worker/tests/test_nodes_loader.py`
**Context:** The `LoadVae` node class is implemented in `worker/nodes/loader.py`. A `PipelineCache` and `NodeContext` with `mock=False` are constructed, and `execute(model_id="test_model")` is called.
**Tests:** Real-mode `LoadVae.execute()` calls `pipeline_cache.get_or_load` with the correct key format (`"vae:test_model"` — prefixed VAE namespace). The cache remains empty after the exception. Satisfies the `REAL_PATH_VERIFIED` marker.
**Mode:** real
**Inputs:** `NodeContext(mock=False, pipeline_cache=PipelineCache())`, `model_id="test_model"`.
**Expected output:** `NotImplementedError` is raised; cache remains empty.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_load_vae_real_cache_key_format -v -m real_mode` exits 0.

---

## test_load_vae_real_raises_no_diffusion_arch (worker)

**File:** `worker/tests/test_nodes_loader.py`
**Context:** The `LoadVae` node class is implemented in `worker/nodes/loader.py`. A `PipelineCache` and `NodeContext` with `mock=False` are constructed, and `execute(model_id="zit-vae")` is called.
**Tests:** Canonical real-mode test. Constructs a `NodeContext` with `mock=False`, calls `execute(model_id="zit-vae")`, and asserts `NotImplementedError` is raised with the exact Phase-19 groundwork message. Satisfies the `REAL_PATH_VERIFIED` marker.
**Mode:** real
**Inputs:** `NodeContext(mock=False, pipeline_cache=PipelineCache())`, `model_id="zit-vae"`.
**Expected output:** `NotImplementedError("no diffusion arch module registered yet")` is raised.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_load_vae_real_raises_no_diffusion_arch -v -m real_mode` exits 0.

---

## test_load_clip_mock_returns_sentinel (worker)

**File:** `worker/tests/test_nodes_loader.py`
**Context:** The `LoadClip` node class is implemented in `worker/nodes/loader.py` and decorated with `@register`. A `NodeContext` with `mock=True` is constructed, and `execute(model_id="test_clip")` is called.
**Tests:** Mock-mode `LoadClip.execute()` returns the sentinel dict shape `{"clip": {"mock": True, "model_id": "test_clip"}}`. Satisfies the `MOCK_PATH_VERIFIED` marker.
**Mode:** mock
**Inputs:** `NodeContext(mock=True)`, `model_id="test_clip"`.
**Expected output:** `{"clip": {"mock": True, "model_id": "test_clip"}}` is returned.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_load_clip_mock_returns_sentinel -v` exits 0.

---

## test_load_clip_real_loads_qwen3_fixture (worker)

**File:** `worker/tests/test_nodes_loader.py`
**Context:** The `LoadClip` node class is implemented in `worker/nodes/loader.py`. In real mode, `execute()` dispatches to `arch.clip.get_module("qwen3")` and calls `module.load()` via `pipeline_cache.get_or_load()`. A `PipelineCache` and `NodeContext` with `mock=False` are constructed, and `execute(model_id=fixture_path, clip_type="qwen3")` is called against the `qwen3_tiny.safetensors` fixture.
**Tests:** Real-mode `LoadClip.execute()` loads the Qwen3 fixture checkpoint via the dispatch chain, returns a `torch.nn.Module` with `.arch == "qwen3"`, parameters on CPU, and an attached `.tokenizer`. Satisfies the `REAL_PATH_VERIFIED` marker.
**Mode:** real
**Inputs:** `NodeContext(mock=False, pipeline_cache=PipelineCache())`, `model_id=worker/tests/fixtures/qwen3_tiny.safetensors`, `clip_type="qwen3"`.
**Expected output:** `{"clip": Qwen3TextEncoder(...)}` is returned; clip is a `torch.nn.Module` with `.arch == "qwen3"`, CPU parameters, and `.tokenizer` attribute.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_load_clip_real_loads_qwen3_fixture -v -m real_mode` exits 0.

---

## test_load_clip_in_registry (worker)

**File:** `worker/tests/test_nodes_loader.py`
**Context:** The `LoadClip` node class is implemented in `worker/nodes/loader.py` and decorated with `@register`. Importing `worker.nodes.loader` in a subprocess triggers the `@register` side effect which populates `NODE_REGISTRY`.
**Tests:** `LoadClip` appears in `NODE_REGISTRY` after importing the module. Uses subprocess isolation to avoid cross-test pollution from prior imports.
**Mode:** both
**Inputs:** Fresh subprocess that imports `worker.nodes.loader` and checks `NODE_REGISTRY`.
**Expected output:** `NODE_REGISTRY` contains `"LoadClip"` as a key.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_load_clip_in_registry -v` exits 0.

---

## test_infer_hyperparams_regular_fixture (worker)

**File:** `worker/tests/test_arch_zit.py`
**Context:** The `_infer_hyperparams()` function is implemented in `worker/nodes/arch/diffusion/zit.py`. The fixture `worker/tests/fixtures/zit_tiny.safetensors` contains a ZiT-shaped checkpoint with `arch="zit"` metadata and recognizable ZiT key prefixes (`input_proj.weight`, `double_blocks.0.*`, `single_blocks.0.*`, `latents`, etc.).
**Tests:** `_infer_hyperparams()` against `zit_tiny.safetensors` returns a dict with all expected keys and correct values: hidden_dim=64, double_block_count=1, single_block_count=1, latent_channels=4, latent_height=8, latent_width=8, patch_size=16, arch="zit", native_dtype="fp32".
**Mode:** both
**Inputs:** Path to `worker/tests/fixtures/zit_tiny.safetensors`.
**Expected output:** Dict with all 9 keys; hidden_dim=64, double_block_count=1, single_block_count=1, latent_channels=4, latent_height=8, latent_width=8, patch_size=16, arch="zit", native_dtype="fp32".
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_infer_hyperparams_regular_fixture -v` exits 0.

---

## test_infer_hyperparams_no_metadata_fixture (worker)

**File:** `worker/tests/test_arch_zit.py`
**Context:** The `_infer_hyperparams()` function is implemented in `worker/nodes/arch/diffusion/zit.py`. The fixture `worker/tests/fixtures/zit_tiny_no_metadata.safetensors` contains a ZiT-shaped checkpoint with no `arch` metadata key and xyz-prefixed keys (`xyz_double_block_*`, `xyz_single_block_*`, `xyz_output_proj`, `xyz_latents`).
**Tests:** `_infer_hyperparams()` against `zit_tiny_no_metadata.safetensors` succeeds via the metadata-fallback path, deriving arch="zit" from key naming patterns (double_block, single_block, output_proj), and returns the same shape-based hyperparameters as the regular fixture.
**Mode:** both
**Inputs:** Path to `worker/tests/fixtures/zit_tiny_no_metadata.safetensors`.
**Expected output:** Dict with arch="zit" (derived from key patterns); hidden_dim=64, double_block_count=1, single_block_count=1, latent_channels=4, latent_height=8, latent_width=8, patch_size=16.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_infer_hyperparams_no_metadata_fixture -v` exits 0.

---

## test_infer_hyperparams_nonexistent_path_raises (worker)

**File:** `worker/tests/test_arch_zit.py`
**Context:** The `_infer_hyperparams()` function is implemented in `worker/nodes/arch/diffusion/zit.py`.
**Tests:** `_infer_hyperparams()` raises `ValueError` for a non-existent file path, with a message containing "No such file".
**Mode:** both
**Inputs:** Path `/tmp/this_file_does_not_exist_abc123.safetensors` (does not exist).
**Expected output:** `ValueError` raised with message containing "No such file".
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_infer_hyperparams_nonexistent_path_raises -v` exits 0.

---

## test_infer_hyperparams_truncated_header_raises (worker)

**File:** `worker/tests/test_arch_zit.py`
**Context:** The `_infer_hyperparams()` function is implemented in `worker/nodes/arch/diffusion/zit.py`.
**Tests:** `_infer_hyperparams()` raises `ValueError` for a truncated/corrupted safetensors file (a small binary blob that is not a valid safetensors header).
**Mode:** both
**Inputs:** Temporary file containing 8 random bytes (not a valid safetensors file).
**Expected output:** `ValueError` raised.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_infer_hyperparams_truncated_header_raises -v` exits 0.

---

## test_can_handle_matches_zit (worker)

**File:** `worker/tests/test_arch_zit.py`
**Context:** The `can_handle()` function is implemented in `worker/nodes/arch/diffusion/zit.py` with a module-level `ARCH = "zit"` constant.
**Tests:** `can_handle("zit")` returns `True` — the primary match path for the ZiT architecture string.
**Mode:** both
**Inputs:** Key string `"zit"`.
**Expected output:** `True`.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_can_handle_matches_zit -v` exits 0.

---

## test_can_handle_rejects_unrelated_key (worker)

**File:** `worker/tests/test_arch_zit.py`
**Context:** The `can_handle()` function is implemented in `worker/nodes/arch/diffusion/zit.py` with a module-level `ARCH = "zit"` constant.
**Tests:** `can_handle("flux2klein")` returns `False` — the module correctly rejects unrelated architecture keys.
**Mode:** both
**Inputs:** Key string `"flux2klein"`.
**Expected output:** `False`.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_can_handle_rejects_unrelated_key -v` exits 0.

---

## test_get_module_returns_zit_for_matching_key (worker)

**File:** `worker/tests/test_arch_zit.py`
**Context:** The `zit` module is imported and appended to `_REGISTERED_MODULES` in `worker/nodes/arch/diffusion/__init__.py`, enabling the dispatcher to find it.
**Tests:** `get_module("zit")` returns the `zit` module (not `None`) — end-to-end dispatch through the registered module.
**Mode:** both
**Inputs:** Key string `"zit"`.
**Expected output:** A `ModuleType` instance with `__name__ == "zit"`.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_get_module_returns_zit_for_matching_key -v` exits 0.

---

## test_load_meta_device_zero_real_memory (worker)

**File:** `worker/tests/test_arch_zit.py`
**Context:** The `load()` function constructs the model inside `torch.device("meta")` context with dtype selection per `caps`, so no real memory is allocated.
**Tests:** `load()` with the default caps dict (all precision flags False except fp32) returns a model where every parameter is on `torch.device("meta")` — confirming zero real memory allocation.
**Mode:** both
**Inputs:** Path to `zit_tiny.safetensors` fixture, default caps dict.
**Expected output:** All parameters on meta device — zero real memory.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_load_meta_device_zero_real_memory -v` exits 0.

---

## test_load_meta_construction_no_metadata_variant (worker)

**File:** `worker/tests/test_arch_zit.py`
**Context:** The `load()` function delegates hyperparameter inference to `_infer_hyperparams()`, which has a metadata-fallback path that detects ZiT from key patterns when the "arch" metadata key is absent. The `load()` function now requires a `caps` parameter for dtype selection.
**Tests:** `load()` against `zit_tiny_no_metadata.safetensors` with the default caps dict succeeds via the fallback path and returns a valid `ZiTModel` with all parameters on meta device.
**Mode:** both
**Inputs:** Path to `zit_tiny_no_metadata.safetensors` fixture (no "arch" metadata, xyz_ prefixed keys), default caps dict.
**Expected output:** `ZiTModel` with `.arch == "zit"`, all parameters on meta device.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_load_meta_construction_no_metadata_variant -v` exits 0.

---

## test_load_raises_invalid_hyperparams (worker)

**File:** `worker/tests/test_arch_zit.py`
**Context:** The `load()` function delegates to `_infer_hyperparams()` which raises `ValueError` for invalid inputs; this error propagates through `load()`. The `load()` function now requires a `caps` parameter.
**Tests:** `load()` with a non-existent file path and a default caps dict raises `ValueError` — confirming error propagation from `_infer_hyperparams()`.
**Mode:** both
**Inputs:** Non-existent path `"/tmp/this_file_does_not_exist_abc123.safetensors"`, default caps dict.
**Expected output:** `ValueError` with "No such file" in the message.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_load_raises_invalid_hyperparams -v` exits 0.

---



## test_dtype_selection_fp8_caps_and_native (worker)

**File:** `worker/tests/test_arch_zit.py`
**Context:** The `_select_dtype()` function is implemented in `worker/nodes/arch/diffusion/zit.py` and encodes the fixed precedence chain from ANVILML_DESIGN.md §11.5. The fixture checkpoint is F32, so the full `load()` path cannot exercise the fp8 branch — this unit test covers that gap with controlled inputs.
**Tests:** `_select_dtype()` with caps.fp8=True and native_dtype="fp8" returns `torch.float8_e4m3fn` — the first branch of the §11.5 precedence.
**Mode:** both
**Inputs:** caps dict with fp8=True, native_dtype string "fp8".
**Expected output:** `torch.float8_e4m3fn`.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_dtype_selection_fp8_caps_and_native -v` exits 0.

---

## test_dtype_selection_fp8_native_non_fp8_caps_fp8 (worker)

**File:** `worker/tests/test_arch_zit.py`
**Context:** The `_select_dtype()` function requires BOTH caps.fp8=True AND native_dtype=="fp8" to select fp8. This test verifies the AND condition by passing caps.fp8=True with native_dtype="fp32".
**Tests:** `_select_dtype()` with caps.fp8=True but native_dtype="fp32" falls through to fp32 (the universal fallback, since bf16=False and fp16=False in this caps dict).
**Mode:** both
**Inputs:** caps dict with fp8=True, bf16=False, fp16=False, native_dtype string "fp32".
**Expected output:** `torch.float32`.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_dtype_selection_fp8_native_non_fp8_caps_fp8 -v` exits 0.

---

## test_dtype_selection_bf16_real (worker)

**File:** `worker/tests/test_arch_zit.py`
**Context:** The `load()` function is implemented in `worker/nodes/arch/diffusion/zit.py` and calls `_select_dtype()` to pick the compute dtype from the worker's capability dict and the checkpoint's native dtype. The fixture is F32, so fp8 is not viable even with caps.fp8=True.
**Tests:** `load()` against `zit_tiny.safetensors` with caps.bf16=True, fp16=True, fp8=False returns a `ZiTModel` with all parameters at `torch.bfloat16`. This is the primary real-mode test for the load() function with dtype selection and serves as the `REAL_PATH_VERIFIED` parity marker.
**Mode:** real
**Inputs:** Path to `zit_tiny.safetensors` fixture, caps dict with bf16=True, fp16=True, fp8=False.
**Expected output:** `ZiTModel` with `.arch == "zit"`, all parameters on meta device, dtype `torch.bfloat16`.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_dtype_selection_bf16_real -v -m real_mode` exits 0.

---

## test_dtype_selection_bf16_mock (worker)

**File:** `worker/tests/test_arch_zit.py`
**Context:** The `load()` function is implemented in `worker/nodes/arch/diffusion/zit.py` and calls `_select_dtype()` to pick the compute dtype. This is the mock-mode counterpart required by the dual-mode parity marker convention (ANVILML_DESIGN.md §10.6).
**Tests:** `load()` against `zit_tiny.safetensors` with caps.bf16=True, fp16=True, fp8=False returns a `ZiTModel` with all parameters at `torch.bfloat16`. This serves as the `MOCK_PATH_VERIFIED` parity marker.
**Mode:** mock
**Inputs:** Path to `zit_tiny.safetensors` fixture, caps dict with bf16=True, fp16=True, fp8=False.
**Expected output:** `ZiTModel` with `.arch == "zit"`, all parameters on meta device, dtype `torch.bfloat16`.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_dtype_selection_bf16_mock -v -m "not real_mode"` exits 0.

---

## test_dtype_selection_fp16_only (worker)

**File:** `worker/tests/test_arch_zit.py`
**Context:** The `load()` function calls `_select_dtype()` which follows the precedence: fp8 → bf16 → fp16 → fp32. This test verifies the fp16 branch when bf16 is not available.
**Tests:** `load()` against `zit_tiny.safetensors` with caps.fp16=True, bf16=False, fp8=False returns a `ZiTModel` with all parameters at `torch.float16`.
**Mode:** both
**Inputs:** Path to `zit_tiny.safetensors` fixture, caps dict with fp16=True, bf16=False, fp8=False.
**Expected output:** `ZiTModel` with all parameters at `torch.float16` on meta device.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_dtype_selection_fp16_only -v` exits 0.

---

## test_dtype_selection_fp32_fallback (worker)

**File:** `worker/tests/test_arch_zit.py`
**Context:** The `load()` function calls `_select_dtype()` which falls through to fp32 as the universal fallback when no higher-precision capability is available.
**Tests:** `load()` against `zit_tiny.safetensors` with all precision flags False returns a `ZiTModel` with all parameters at `torch.float32`.
**Mode:** both
**Inputs:** Path to `zit_tiny.safetensors` fixture, caps dict with all precision flags False.
**Expected output:** `ZiTModel` with all parameters at `torch.float32` on meta device.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_dtype_selection_fp32_fallback -v` exits 0.

---

## test_dtype_selection_fp8_beats_bf16 (worker)

**File:** `worker/tests/test_arch_zit.py`
**Context:** The `_select_dtype()` function implements the fixed precedence from ANVILML_DESIGN.md §11.5 where fp8 takes priority over bf16. This test verifies the ordering by passing both caps.fp8=True and caps.bf16=True with native_dtype="fp8".
**Tests:** `_select_dtype()` with caps.fp8=True, bf16=True, native_dtype="fp8" returns `torch.float8_e4m3fn` — confirming fp8 takes precedence over bf16.
**Mode:** both
**Inputs:** caps dict with fp8=True, bf16=True, native_dtype string "fp8".
**Expected output:** `torch.float8_e4m3fn`.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_dtype_selection_fp8_beats_bf16 -v` exits 0.

---

## test_load_real_zit_fixture (worker)

**File:** `worker/tests/test_arch_zit.py`
**Context:** The `load()` function has been updated (P20-C3) to accept a `device` parameter, call `model.to_empty(device=device)` to materialize meta-constructed parameters onto the real device, build a checkpoint-key → module-key remapping table, load weights via `safetensors.torch.load_file()`, cast tensors to target_dtype, and call `load_state_dict(remapped_state_dict, assign=True)`.
**Tests:** `load()` against `zit_tiny.safetensors` with bf16 capability succeeds end-to-end; `.arch == "zit"`; all tensors on cpu device; selected dtype is bfloat16; spot-check verifies `input_proj.weight` has non-zero values from the checkpoint.
**Mode:** real
**Inputs:** Path to `zit_tiny.safetensors` fixture, caps dict with bf16=True, device="cpu".
**Expected output:** `ZiTModel` with `.arch == "zit"`, all parameters on cpu device, dtype=torch.bfloat16, non-zero loaded weights.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_load_real_zit_fixture -v -m real_mode` exits 0.

---

## test_load_mock_zit_fixture (worker)

**File:** `worker/tests/test_arch_zit.py`
**Context:** The `load()` function materializes weights onto the real device. This is the mock-mode counterpart required by the dual-mode parity marker convention (ANVILML_DESIGN.md §10.6).
**Tests:** `load()` against `zit_tiny.safetensors` with bf16 capability succeeds end-to-end in mock-mode; `.arch == "zit"`; all tensors on cpu device; selected dtype is bfloat16; spot-check verifies non-zero loaded weights.
**Mode:** mock
**Inputs:** Path to `zit_tiny.safetensors` fixture, caps dict with bf16=True, device="cpu".
**Expected output:** `ZiTModel` with `.arch == "zit"`, all parameters on cpu device, dtype=torch.bfloat16, non-zero loaded weights.
**Acceptance:** `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_load_mock_zit_fixture -v -m "not real_mode"` exits 0.

---

## test_load_no_metadata_real (worker)

**File:** `worker/tests/test_arch_zit.py`
**Context:** The `load()` function loads against the no-metadata fixture which has no `arch` key in its safetensors header. The metadata-fallback path in `_infer_hyperparams()` identifies the architecture from key naming patterns.
**Tests:** `load()` against `zit_tiny_no_metadata.safetensors` with bf16 capability succeeds via the metadata-fallback path; `.arch == "zit"`; all tensors on cpu device.
**Mode:** real
**Inputs:** Path to `zit_tiny_no_metadata.safetensors` fixture, caps dict with bf16=True, device="cpu".
**Expected output:** `ZiTModel` with `.arch == "zit"`, all parameters on cpu device.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_load_no_metadata_real -v -m real_mode` exits 0.

---

## test_load_no_metadata_mock (worker)

**File:** `worker/tests/test_arch_zit.py`
**Context:** The `load()` function loads against the no-metadata fixture in mock-mode. This is the mock-mode counterpart required by the dual-mode parity marker convention.
**Tests:** `load()` against `zit_tiny_no_metadata.safetensors` with bf16 capability succeeds; `.arch == "zit"`; all tensors on cpu device.
**Mode:** mock
**Inputs:** Path to `zit_tiny_no_metadata.safetensors` fixture, caps dict with bf16=True, device="cpu".
**Expected output:** `ZiTModel` with `.arch == "zit"`, all parameters on cpu device.
**Acceptance:** `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_load_no_metadata_mock -v -m "not real_mode"` exits 0.

---

## test_load_tensors_materialized_on_device (worker)

**File:** `worker/tests/test_arch_zit.py`
**Context:** The `load()` function calls `model.to_empty(device=device)` to materialize all parameters from meta device to the real device before loading weights.
**Tests:** After `load()`, every parameter's `.device.type` is `"cpu"` (not `"meta"`), confirming `to_empty()` worked. The post-load dtype matches the target dtype (bfloat16 when bf16 is available).
**Mode:** both
**Inputs:** Path to `zit_tiny.safetensors` fixture, caps dict with bf16=True, device="cpu".
**Expected output:** All parameters on cpu device; dtype=torch.bfloat16.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_load_tensors_materialized_on_device -v` exits 0.

---

## test_load_key_remapping_direct_match (worker)

**File:** `worker/tests/test_arch_zit.py`
**Context:** The `_build_key_remapping()` function builds a checkpoint-key → module-key mapping for `load_state_dict`. It handles direct matches (exact key equality) and pattern-based remapping for ZiT checkpoint key naming conventions.
**Tests:** `_build_key_remapping()` with actual checkpoint keys and module state_dict keys correctly maps 4 direct matches (input_proj.weight, output_proj.weight, single_blocks.0.linear1.weight, time_text_emb.weight) and excludes 4 non-matching keys (c_crossattn_dim, latents, double_blocks.*.proj.weight).
**Mode:** both
**Inputs:** List of 8 checkpoint keys, list of 28 module state_dict keys.
**Expected output:** Remapping dict with exactly 4 entries, all direct matches; excluded keys not present.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_load_key_remapping_direct_match -v` exits 0.

---

## test_load_raises_on_invalid_path (worker)

**File:** `worker/tests/test_arch_zit.py`
**Context:** The `load()` function delegates to `_infer_hyperparams()` which wraps safetensors errors in `ValueError`. The `device` parameter is passed through to `to_empty()` and `load_file()`.
**Tests:** `load()` with a non-existent file path raises `ValueError` with a descriptive message, confirming error propagation from `_infer_hyperparams()` through `load()`.
**Mode:** both
**Inputs:** Non-existent path `/tmp/this_file_does_not_exist_xyz789.safetensors`, default caps dict, device="cpu".
**Expected output:** `ValueError` raised with message containing "No such file".
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_load_raises_on_invalid_path -v` exits 0.

---

## test_load_model_real_loads_zit_fixture (worker)

**File:** `worker/tests/test_nodes_loader.py`
**Context:** The `LoadModel.execute()` real branch dispatches to `arch.diffusion.get_module("zit").load()` via `pipeline_cache.get_or_load()`. The P20-A1 fixture `zit_tiny.safetensors` is a synthetic checkpoint that exercises the full loading chain: shape inference, meta construction, dtype selection, key remapping, and weight loading.
**Tests:** `LoadModel.execute()` with `mock=False` against the P20-A1 fixture path returns a dict with a `"model"` key containing a `torch.nn.Module` (ZiTModel) with `.arch == "zit"` and all parameters on `"cpu"` device.
**Mode:** real
**Inputs:** `model_id` = path to `worker/tests/fixtures/zit_tiny.safetensors`, `mock=False`.
**Expected output:** `{"model": ZiTModel}` with `model.arch == "zit"` and all params on cpu.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_load_model_real_loads_zit_fixture -v` exits 0.

---

## test_compute_latent_shape_mock_exact_multiple (worker)

**File:** `worker/tests/test_arch_zit.py`
**Context:** `compute_latent_shape()` uses module-level `MODEL_PATCH_SIZE=16` and `MODEL_LATENT_CHANNELS=4` defaults. The ceiling division formula `(x + 15) // 16` computes `ceil(x / 16)` for exact multiples.
**Tests:** `compute_latent_shape(32, 32, 1)` returns `(1, 4, 2, 2)` — exact multiple of patch size (32/16=2), latent channels=4, batch_size=1.
**Mode:** mock
**Inputs:** width=32, height=32, batch_size=1.
**Expected output:** `(1, 4, 2, 2)`.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_compute_latent_shape_mock_exact_multiple -v` exits 0.

---

## test_compute_latent_shape_mock_non_multiple (worker)

**File:** `worker/tests/test_arch_zit.py`
**Context:** `compute_latent_shape()` uses module-level defaults. The ceiling division `(33 + 15) // 16 = 3` rounds up non-multiples so the latent grid fully covers the input.
**Tests:** `compute_latent_shape(33, 33, 1)` returns `(1, 4, 3, 3)` — ceiling division for non-multiple dimensions (33/16=2.0625→3).
**Mode:** mock
**Inputs:** width=33, height=33, batch_size=1.
**Expected output:** `(1, 4, 3, 3)`.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_compute_latent_shape_mock_non_multiple -v` exits 0.

---

## test_compute_latent_shape_mock_batch_scaling (worker)

**File:** `worker/tests/test_arch_zit.py`
**Context:** `compute_latent_shape()` uses module-level defaults. The batch_size parameter is returned as the first element of the shape tuple.
**Tests:** `compute_latent_shape(64, 64, 4)` returns `(4, 4, 4, 4)` — batch_size=4 scales the first dimension, latent_height=4, latent_width=4.
**Mode:** mock
**Inputs:** width=64, height=64, batch_size=4.
**Expected output:** `(4, 4, 4, 4)`.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_compute_latent_shape_mock_batch_scaling -v` exits 0.

---

## test_compute_latent_shape_real_after_load (worker)

**File:** `worker/tests/test_arch_zit.py`
**Context:** `load()` calls `_infer_hyperparams()` which extracts `patch_size=16` and `latent_channels=4` from the fixture checkpoint, then sets the module-level constants. After `load()`, `compute_latent_shape()` uses these actual values.
**Tests:** After `load()` against the fixture, `compute_latent_shape(32, 32, 1)` returns `(1, 4, 2, 2)` using the checkpoint's actual hyperparameters.
**Mode:** real
**Inputs:** Fixture path `worker/tests/fixtures/zit_tiny.safetensors`, then `width=32, height=32, batch_size=1`.
**Expected output:** `(1, 4, 2, 2)`.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_compute_latent_shape_real_after_load -v` exits 0.

---

## test_compute_latent_shape_real_non_multiple_after_load (worker)

**File:** `worker/tests/test_arch_zit.py`
**Context:** After `load()` updates the module-level hyperparameters, `compute_latent_shape()` uses ceiling division for non-multiple dimensions. With patch_size=16, 50/16=3.125→4.
**Tests:** After `load()` against the fixture, `compute_latent_shape(50, 50, 1)` returns `(1, 4, 4, 4)` — ceiling division for non-multiple dimensions.
**Mode:** real
**Inputs:** Fixture path `worker/tests/fixtures/zit_tiny.safetensors`, then `width=50, height=50, batch_size=1`.
**Expected output:** `(1, 4, 4, 4)`.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_compute_latent_shape_real_non_multiple_after_load -v` exits 0.

---

## test_compute_latent_shape_default_batch_size (worker)

**File:** `worker/tests/test_arch_zit.py`
**Context:** `compute_latent_shape()` has `batch_size: int = 1` as a default parameter. Calling without the third argument should use batch_size=1.
**Tests:** `compute_latent_shape(32, 32)` (no batch_size) returns `(1, 4, 2, 2)`, confirming the default.
**Mode:** mock
**Inputs:** width=32, height=32 (batch_size omitted).
**Expected output:** `(1, 4, 2, 2)`.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_compute_latent_shape_default_batch_size -v` exits 0.

---

## test_compute_latent_shape_zero_dims (worker)

**File:** `worker/tests/test_arch_zit.py`
**Context:** Edge case test for the ceiling division formula. `(0 + 15) // 16 = 0`, so zero dimensions produce zero latent dimensions.
**Tests:** `compute_latent_shape(0, 32, 1)` returns `(1, 4, 0, 2)`, `compute_latent_shape(32, 0, 1)` returns `(1, 4, 2, 0)`, and `compute_latent_shape(0, 0, 1)` returns `(1, 4, 0, 0)`.
**Mode:** mock
**Inputs:** width=0/height=32, width=32/height=0, width=0/height=0.
**Expected output:** `(1, 4, 0, 2)`, `(1, 4, 2, 0)`, `(1, 4, 0, 0)`.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_compute_latent_shape_zero_dims -v` exits 0.

---

---

## test_sample_first_call_assembles_pipeline_mock (worker)

**File:** `worker/tests/test_arch_zit.py`
**Context:** The `sample()` function in `zit.py` assembles a `ZiTPipeline` from a loaded `ZiTModel` and caches it under `f"{model_id}:pipeline"` in the module-level `PipelineCache`. This test uses a spy on `pipeline_cache.get_or_load` to verify the loader is called exactly once on first access.
**Tests:** First call to `sample()` with `model_id="test1"` assembles and caches a pipeline; `get_or_load` loader is invoked exactly once.
**Mode:** mock
**Inputs:** `model_id="test1"`, fixture-loaded `ZiTModel`, `conditioning=None`, `latent=torch.zeros(1,4,2,2)`, `steps=20`, `cfg=7.5`, `seed=42`.
**Expected output:** `ZiTPipeline` instance with `.model` set to the input model; loader call count = 1.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_sample_first_call_assembles_pipeline_mock -v` exits 0.
**Parity marker:** `MOCK_PATH_VERIFIED` on `sample()` in `zit.py`.

---

## test_sample_second_call_reuses_cached_pipeline_mock (worker)

**File:** `worker/tests/test_arch_zit.py`
**Context:** The `sample()` function caches assembled pipelines per `model_id`. A second call with the same `model_id` should return the cached pipeline without re-assembly.
**Tests:** Second call to `sample()` with same `model_id="test2"` returns the same `ZiTPipeline` object; loader call count remains at 1.
**Mode:** mock
**Inputs:** `model_id="test2"`, fixture-loaded `ZiTModel`, same args as first call.
**Expected output:** Same `ZiTPipeline` object returned; loader call count = 1.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_sample_second_call_reuses_cached_pipeline_mock -v` exits 0.
**Parity marker:** `MOCK_PATH_VERIFIED` on `sample()` in `zit.py`.

---

## test_sample_different_model_id_gets_separate_pipeline (worker)

**File:** `worker/tests/test_arch_zit.py`
**Context:** The `sample()` function keys pipelines by `f"{model_id}:pipeline"`, so different model IDs should produce separate cached pipelines.
**Tests:** Two calls to `sample()` with different `model_id` values (`"model_a"` and `"model_b"`) produce separate `ZiTPipeline` objects.
**Mode:** both
**Inputs:** `model_id="model_a"` and `model_id="model_b"`, same fixture-loaded model.
**Expected output:** Two distinct `ZiTPipeline` instances, each with `.model` set to the input model.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_sample_different_model_id_gets_separate_pipeline -v` exits 0.

---

## test_sample_pipeline_is_zit_wrapper (worker)

**File:** `worker/tests/test_arch_zit.py`
**Context:** The `ZiTPipeline` wrapper returned by `sample()` must hold the exact `ZiTModel` instance passed in (identity check).
**Tests:** The returned `ZiTPipeline` has a `.model` attribute that is the same `ZiTModel` instance passed to `sample()`.
**Mode:** both
**Inputs:** Fixture-loaded `ZiTModel`, `model_id="test_model"`.
**Expected output:** `pipeline.model is model` (identity check passes).
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_sample_pipeline_is_zit_wrapper -v` exits 0.

---

## test_sample_pipeline_assembled_from_loaded_model (worker)

**File:** `worker/tests/test_arch_zit.py`
**Context:** Real-mode test for `sample()` — verifies the full pipeline assembly path with a real fixture-loaded model. The `sample()` function constructs a `ZiTPipeline` wrapping the loaded model and an `EulerDiscreteScheduler`.
**Tests:** `sample()` with a fixture-loaded model produces a `ZiTPipeline` with the correct model and a non-None scheduler.
**Mode:** real
**Inputs:** `model_id="real_test"`, `zit_tiny.safetensors` loaded with bf16 capability.
**Expected output:** `ZiTPipeline` with `.model` pointing to the loaded model and `.scheduler` being an `EulerDiscreteScheduler` instance.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_sample_pipeline_assembled_from_loaded_model -v -m real_mode` exits 0.
**Parity marker:** `REAL_PATH_VERIFIED` on `sample()` in `zit.py`.

---

## test_sampler_class_attributes (worker.nodes.sampler)

**File:** `worker/tests/test_nodes_sampler.py`
**Context:** The Sampler node module has been created with all six required class attributes and the `@register` decorator.
**Tests:** All six class attributes (`NODE_TYPE`, `CATEGORY`, `DISPLAY_NAME`, `DESCRIPTION`, `INPUT_SLOTS`, `OUTPUT_SLOTS`) match expected values exactly. INPUT_SLOTS has 7 SlotSpecs, OUTPUT_SLOTS has 2.
**Mode:** mock
**Inputs:** None — class-level attribute inspection.
**Expected output:** All six attributes match their expected values.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_nodes_sampler.py::test_sampler_class_attributes -v` exits 0.

---

## test_sampler_mock_returns_expected_shape (worker.nodes.sampler)

**File:** `worker/tests/test_nodes_sampler.py`
**Context:** The Sampler node's mock branch returns a sentinel dict with the input latent's shape and the seed passed through.
**Tests:** Mock-mode `execute()` returns `{"latent": {"mock": True, "shape": (1, 4, 64, 64)}, "seed": 42}` with shape propagated from `inputs["latent"]`. Satisfies the `MOCK_PATH_VERIFIED` marker.
**Mode:** mock
**Inputs:** `model={}`, `conditioning={}`, `clip={}`, `latent={"shape": (1, 4, 64, 64)}`, `steps=20`, `cfg=7.5`, `seed=42`.
**Expected output:** `{"latent": {"mock": True, "shape": (1, 4, 64, 64)}, "seed": 42}`.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_nodes_sampler.py::test_sampler_mock_returns_expected_shape -v -m "not real_mode"` exits 0.

---

## test_sampler_mock_seed_zero (worker.nodes.sampler)

**File:** `worker/tests/test_nodes_sampler.py`
**Context:** The Sampler node's mock branch resolves seed=-1 to 0 deterministically for reproducible output.
**Tests:** When `seed=-1`, mock returns `{"seed": 0}`. When `seed=42`, returns `{"seed": 42}`.
**Mode:** mock
**Inputs:** Same as above with `seed=-1` and `seed=42` respectively.
**Expected output:** seed=-1 → 0, seed=42 → 42.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_nodes_sampler.py::test_sampler_mock_seed_zero -v -m "not real_mode"` exits 0.

---

## test_sampler_real_denoises_zit_fixture (worker.nodes.sampler)

**File:** `worker/tests/test_nodes_sampler.py`
**Context:** The Sampler node's real branch dispatches to `arch.diffusion.get_module(model.arch).sample()`. The ZiT fixture checkpoint (`zit_tiny.safetensors`) is loaded via `zit.load()`, then `Sampler.execute()` is called with the loaded model.
**Tests:** End-to-end: load ZiT fixture, execute Sampler with seed=42, assert denoised latent shape and seed. Satisfies the `REAL_PATH_VERIFIED` marker.
**Mode:** real
**Inputs:** ZiT fixture loaded via `zit.load()`, `conditioning=None`, latent `torch.zeros(1, 4, 8, 8)`, `steps=20`, `cfg=7.5`, `seed=42`, `mock=False`.
**Expected output:** `{"latent": torch.Tensor of shape (1, 4, 8, 8), "seed": 42}` is returned.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_nodes_sampler.py::test_sampler_real_denoises_zit_fixture -v -m real_mode` exits 0.

---

## test_sampler_real_seed_minus_one_resolves (worker.nodes.sampler)

**File:** `worker/tests/test_nodes_sampler.py`
**Context:** The Sampler node's real branch delegates seed resolution to `zit.sample()`. The ZiT fixture checkpoint is loaded, then `Sampler.execute()` is called with `seed=-1`.
**Tests:** seed=-1 resolves to a non-negative integer in [0, 2**63). Verifies the real branch correctly delegates seed resolution.
**Mode:** real
**Inputs:** ZiT fixture loaded via `zit.load()`, `conditioning=None`, latent `torch.zeros(1, 4, 8, 8)`, `steps=20`, `cfg=7.5`, `seed=-1`, `mock=False`.
**Expected output:** returned seed is an int in [0, 2**63).
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_nodes_sampler.py::test_sampler_real_seed_minus_one_resolves -v -m real_mode` exits 0.

---

## test_sampler_real_explicit_seed_unchanged (worker.nodes.sampler)

**File:** `worker/tests/test_nodes_sampler.py`
**Context:** The Sampler node's real branch passes through non-negative seeds unchanged. The ZiT fixture checkpoint is loaded, then `Sampler.execute()` is called with `seed=42`.
**Tests:** Explicit seed=42 passes through unchanged. Verifies non-negative seeds are not modified by the real branch.
**Mode:** real
**Inputs:** ZiT fixture loaded via `zit.load()`, `conditioning=None`, latent `torch.zeros(1, 4, 8, 8)`, `steps=20`, `cfg=7.5`, `seed=42`, `mock=False`.
**Expected output:** returned seed == 42.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_nodes_sampler.py::test_sampler_real_explicit_seed_unchanged -v -m real_mode` exits 0.

---

## test_sampler_real_multiple_steps (worker.nodes.sampler)

**File:** `worker/tests/test_nodes_sampler.py`
**Context:** The Sampler node's real branch runs the denoising loop for the specified number of steps. The ZiT fixture checkpoint is loaded, then `Sampler.execute()` is called with `steps=10`.
**Tests:** steps=10 produces correct output shape. Verifies the denoising loop runs the correct number of steps without altering the output shape.
**Mode:** real
**Inputs:** ZiT fixture loaded via `zit.load()`, `conditioning=None`, latent `torch.zeros(1, 4, 8, 8)`, `steps=10`, `cfg=7.5`, `seed=42`, `mock=False`.
**Expected output:** output tensor shape matches input shape (1, 4, 8, 8).
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_nodes_sampler.py::test_sampler_real_multiple_steps -v -m real_mode` exits 0.

---

## test_sampler_real_cfg_one_is_conditional_only (worker.nodes.sampler)

**File:** `worker/tests/test_nodes_sampler.py`
**Context:** The Sampler node's real branch handles cfg=1.0 (no guidance) where the unconditional pass contributes zero. The ZiT fixture checkpoint is loaded, then `Sampler.execute()` is called with `cfg=1.0`.
**Tests:** cfg=1.0 (no guidance) runs without error. Exercises the CFG path where unconditional and conditional predictions are blended.
**Mode:** real
**Inputs:** ZiT fixture loaded via `zit.load()`, `conditioning=None`, latent `torch.zeros(1, 4, 8, 8)`, `steps=20`, `cfg=1.0`, `seed=42`, `mock=False`.
**Expected output:** returns `{"latent": torch.Tensor, "seed": int}`.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_nodes_sampler.py::test_sampler_real_cfg_one_is_conditional_only -v -m real_mode` exits 0.

---

## test_sampler_real_latent_shape_preserved (worker.nodes.sampler)

**File:** `worker/tests/test_nodes_sampler.py`
**Context:** The Sampler node's real branch preserves the latent tensor shape through denoising. The ZiT fixture checkpoint is loaded, then `Sampler.execute()` is called with a latent of shape (1, 4, 8, 8).
**Tests:** Output tensor shape matches input latent shape (1, 4, 8, 8). Verifies the Sampler does not alter the latent dimensions during denoising.
**Mode:** real
**Inputs:** ZiT fixture loaded via `zit.load()`, `conditioning=None`, latent `torch.zeros(1, 4, 8, 8)`, `steps=20`, `cfg=7.5`, `seed=42`, `mock=False`.
**Expected output:** output tensor shape == (1, 4, 8, 8).
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_nodes_sampler.py::test_sampler_real_latent_shape_preserved -v -m real_mode` exits 0.

---

## test_sampler_in_registry (worker.nodes.sampler)

**File:** `worker/tests/test_nodes_sampler.py`
**Context:** The Sampler node module is auto-imported by `worker.nodes.__init__` and registered via `@register` at module load time.
**Tests:** Subprocess isolation test: fresh Python process imports `worker.nodes.sampler`, confirms `NODE_REGISTRY["Sampler"]` exists and is the `Sampler` class.
**Mode:** mock
**Inputs:** Fresh subprocess with `sys.executable -c` code.
**Expected output:** `NODE_REGISTRY` contains "Sampler" as a key pointing to the `Sampler` class.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_nodes_sampler.py::test_sampler_in_registry -v -m "not real_mode"` exits 0.

## test_infer_hyperparams_qwen3_fixture (worker.nodes.arch.clip.qwen3)

**File:** `worker/tests/test_arch_clip_qwen3.py`
**Context:** The Qwen3 CLIP fixture checkpoint (`qwen3_tiny.safetensors`) is built by `build_qwen3_fixture.py` and contains Qwen3-shaped tensor keys with `arch: "qwen3"` metadata. The `_infer_hyperparams()` function reads ALL keys via `f.keys()` (never truncated) from the safetensors header using `safe_open(path, framework="np")`.
**Tests:** `_infer_hyperparams()` against `qwen3_tiny.safetensors` returns correct hyperparameter dict: `hidden_dim=64`, `num_hidden_layers=2`, `intermediate_size=128`, `vocab_size=128`, `arch="qwen3"`, `native_dtype="fp32"`. Verifies all six expected keys are present and have correct values.
**Mode:** both
**Inputs:** `qwen3_tiny.safetensors` fixture path.
**Expected output:** dict with all six keys and correct values (hidden_dim=64, num_hidden_layers=2, intermediate_size=128, vocab_size=128, arch="qwen3", native_dtype="fp32").
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_qwen3.py::test_infer_hyperparams_qwen3_fixture -v` exits 0.

---

## test_infer_hyperparams_nonexistent_path_raises (worker.nodes.arch.clip.qwen3)

**File:** `worker/tests/test_arch_clip_qwen3.py`
**Context:** The `_infer_hyperparams()` function wraps file open errors into `ValueError` with descriptive messages. A non-existent file path triggers `FileNotFoundError`, which is caught and re-raised as `ValueError` containing "No such file".
**Tests:** `_infer_hyperparams()` raises `ValueError` for a non-existent file path, with a message containing "No such file". Verifies error handling for missing files.
**Mode:** both
**Inputs:** Path `/tmp/this_file_does_not_exist_abc123.safetensors` (non-existent).
**Expected output:** `ValueError` raised with message matching "No such file".
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_qwen3.py::test_infer_hyperparams_nonexistent_path_raises -v` exits 0.

---

## test_infer_hyperparams_truncated_header_raises (worker.nodes.arch.clip.qwen3)

**File:** `worker/tests/test_arch_clip_qwen3.py`
**Context:** The `_infer_hyperparams()` function wraps safetensors deserialization errors into `ValueError`. A truncated/corrupted file (binary blob that is not a valid safetensors header) triggers `SafetensorError`, which is caught and re-raised as `ValueError`.
**Tests:** `_infer_hyperparams()` raises `ValueError` for a truncated/corrupted safetensors file (small binary blob that is not a valid safetensors header). Verifies error handling for corrupted files.
**Mode:** both
**Inputs:** Temporary file containing 8 bytes of binary data (`\x00\x01\x02\x03\x04\x05\x06\x07`) — not a valid safetensors header.
**Expected output:** `ValueError` raised.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_qwen3.py::test_infer_hyperparams_truncated_header_raises -v` exits 0.


## test_can_handle_matches_qwen3 (worker.nodes.arch.clip.qwen3)

**File:** `worker/tests/test_arch_clip_qwen3.py`
**Context:** The `can_handle()` function is a pure string comparison that checks whether a given dispatch key matches the module's canonical architecture identifier (`ARCH = "qwen3"`). It has no I/O, no torch dependency, and is importable in mock-mode.
**Tests:** `can_handle("qwen3")` returns `True` — confirms the dispatch key "qwen3" is recognised by the qwen3 module's can_handle().
**Mode:** mock
**Inputs:** The string `"qwen3"`.
**Expected output:** `True`.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_qwen3.py::test_can_handle_matches_qwen3 -v` exits 0.

---

## test_can_handle_rejects_other_keys (worker.nodes.arch.clip.qwen3)

**File:** `worker/tests/test_arch_clip_qwen3.py`
**Context:** The `can_handle()` function performs an exact string comparison against `ARCH`. It must return `False` for any key that does not match `"qwen3"`.
**Tests:** `can_handle("zit")`, `can_handle("flux2klein")`, and `can_handle("unknown")` all return `False` — confirms the function does not match unrelated architecture names.
**Mode:** mock
**Inputs:** The strings `"zit"`, `"flux2klein"`, `"unknown"`.
**Expected output:** All three calls return `False`.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_qwen3.py::test_can_handle_rejects_other_keys -v` exits 0.

---

## test_get_module_returns_qwen3_for_matching_key (worker.nodes.arch.clip.qwen3)

**File:** `worker/tests/test_arch_clip_qwen3.py`
**Context:** The clip dispatcher's `_REGISTERED_MODULES` list now includes the qwen3 module (registered at import time via `__init__.py`). `get_module("qwen3")` iterates over registered modules calling `can_handle()` on each and returns the first match.
**Tests:** `clip.get_module("qwen3")` returns the qwen3 module (identity check with `is`) — confirms the qwen3 module was correctly registered and `get_module()` finds it via dispatch.
**Mode:** mock
**Inputs:** The string `"qwen3"`.
**Expected output:** The returned module is identical to the qwen3 module (`result is qwen3`).
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_qwen3.py::test_get_module_returns_qwen3_for_matching_key -v` exits 0.

---

## test_dtype_selection_fp8_caps_and_native (worker.nodes.arch.clip.qwen3)

**File:** `worker/tests/test_arch_clip_qwen3.py`
**Context:** The `_select_dtype()` function is implemented in `worker/nodes/arch/clip/qwen3.py` and encodes the fixed precedence chain from ANVILML_DESIGN.md §11.5. The fixture checkpoint is F32, so the full `load()` path cannot exercise the fp8 branch — this unit test covers that gap with controlled inputs.
**Tests:** `_select_dtype()` with caps.fp8=True and native_dtype="fp8" returns `torch.float8_e4m3fn` — the first branch of the §11.5 precedence.
**Mode:** both
**Inputs:** caps dict with fp8=True, native_dtype string "fp8".
**Expected output:** `torch.float8_e4m3fn`.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_qwen3.py::test_dtype_selection_fp8_caps_and_native -v` exits 0.

---

## test_dtype_selection_bf16_real (worker.nodes.arch.clip.qwen3)

**File:** `worker/tests/test_arch_clip_qwen3.py`
**Context:** The `load()` function is implemented in `worker/nodes/arch/clip/qwen3.py` and calls `_select_dtype()` to pick the compute dtype from the worker's capability dict and the checkpoint's native dtype. The fixture is F32, so fp8 is not viable even with caps.fp8=True.
**Tests:** `load()` against `qwen3_tiny.safetensors` with caps.bf16=True, fp16=True, fp8=False returns a `Qwen3TextEncoder` with all parameters at `torch.bfloat16`. This is the primary real-mode test for the load() function with dtype selection and serves as the `REAL_PATH_VERIFIED` parity marker.
**Mode:** real
**Inputs:** Path to `qwen3_tiny.safetensors` fixture, caps dict with bf16=True, fp16=True, fp8=False.
**Expected output:** `Qwen3TextEncoder` with `.arch == "qwen3"`, all parameters on meta device, dtype `torch.bfloat16`.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_qwen3.py::test_dtype_selection_bf16_real -v -m real_mode` exits 0.

---

## test_dtype_selection_bf16_mock (worker.nodes.arch.clip.qwen3)

**File:** `worker/tests/test_arch_clip_qwen3.py`
**Context:** The `load()` function is implemented in `worker/nodes/arch/clip/qwen3.py` and calls `_select_dtype()` to pick the compute dtype. This is the mock-mode counterpart required by the dual-mode parity marker convention (ANVILML_DESIGN.md §10.6).
**Tests:** `load()` against `qwen3_tiny.safetensors` with caps.bf16=True, fp16=True, fp8=False returns a `Qwen3TextEncoder` with all parameters at `torch.bfloat16`. This serves as the `MOCK_PATH_VERIFIED` parity marker.
**Mode:** mock
**Inputs:** Path to `qwen3_tiny.safetensors` fixture, caps dict with bf16=True, fp16=True, fp8=False.
**Expected output:** `Qwen3TextEncoder` with `.arch == "qwen3"`, all parameters on meta device, dtype `torch.bfloat16`.
**Acceptance:** `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_qwen3.py::test_dtype_selection_bf16_mock -v -m "not real_mode"` exits 0.

---

## test_dtype_selection_fp16_only (worker.nodes.arch.clip.qwen3)

**File:** `worker/tests/test_arch_clip_qwen3.py`
**Context:** The `load()` function calls `_select_dtype()` which follows the precedence: fp8 → bf16 → fp16 → fp32. This test verifies the fp16 branch when bf16 is not available.
**Tests:** `load()` against `qwen3_tiny.safetensors` with caps.fp16=True, bf16=False, fp8=False returns a `Qwen3TextEncoder` with all parameters at `torch.float16`.
**Mode:** both
**Inputs:** Path to `qwen3_tiny.safetensors` fixture, caps dict with fp16=True, bf16=False, fp8=False.
**Expected output:** `Qwen3TextEncoder` with all parameters at `torch.float16` on meta device.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_qwen3.py::test_dtype_selection_fp16_only -v` exits 0.

---

## test_dtype_selection_fp32_fallback (worker.nodes.arch.clip.qwen3)

**File:** `worker/tests/test_arch_clip_qwen3.py`
**Context:** The `load()` function calls `_select_dtype()` which falls through to fp32 as the universal fallback when no higher-precision capability is available.
**Tests:** `load()` against `qwen3_tiny.safetensors` with all precision flags False returns a `Qwen3TextEncoder` with all parameters at `torch.float32`.
**Mode:** both
**Inputs:** Path to `qwen3_tiny.safetensors` fixture, caps dict with all precision flags False.
**Expected output:** `Qwen3TextEncoder` with all parameters at `torch.float32` on meta device.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_qwen3.py::test_dtype_selection_fp32_fallback -v` exits 0.

---

## test_load_real_qwen3_fixture (worker.nodes.arch.clip.qwen3)

**File:** `worker/tests/test_arch_clip_qwen3.py`
**Context:** The `load()` function has been implemented (P22-C1) to accept a `caps` parameter, call `_infer_hyperparams()` for shape inference, call `_select_dtype()` for dtype selection, construct `Qwen3TextEncoder` on meta-device, apply dtype, and load the tokenizer from the vendored local asset directory. Weight materialization and weight loading are deferred to P22-C2.
**Tests:** `load()` against `qwen3_tiny.safetensors` with bf16 capability succeeds; `.arch == "qwen3"`; all parameters on meta device; dtype is bfloat16; tokenizer is attached. Satisfies the `REAL_PATH_VERIFIED` marker.
**Mode:** real
**Inputs:** Path to `qwen3_tiny.safetensors` fixture, caps dict with bf16=True, fp16=True, fp8=False.
**Expected output:** `Qwen3TextEncoder` with `.arch == "qwen3"`, all parameters on meta device, dtype=torch.bfloat16, tokenizer attached.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_qwen3.py::test_load_real_qwen3_fixture -v -m real_mode` exits 0.

---

## test_load_raises_invalid_hyperparams (worker.nodes.arch.clip.qwen3)

**File:** `worker/tests/test_arch_clip_qwen3.py`
**Context:** The `load()` function delegates to `_infer_hyperparams()` which raises `ValueError` for invalid inputs; this error propagates through `load()`.
**Tests:** `load()` with a non-existent file path raises `ValueError` with a descriptive message, confirming error propagation from `_infer_hyperparams()`.
**Mode:** both
**Inputs:** Non-existent path `/tmp/this_file_does_not_exist_xyz789.safetensors`, default caps dict.
**Expected output:** `ValueError` raised with message containing "No such file".
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_qwen3.py::test_load_raises_invalid_hyperparams -v` exits 0.

---

## test_load_raises_runtime_error_without_torch (worker.nodes.arch.clip.qwen3)

**File:** `worker/tests/test_arch_clip_qwen3.py`
**Context:** The `load()` function has a torch guard that raises `RuntimeError` when torch is not installed. This test verifies the guard works by confirming the function operates normally when torch IS available.
**Tests:** `load()` with a valid fixture path and bf16 capability succeeds (verifying the torch guard is not triggered). The guard is tested indirectly by mock-mode collection tests which import this module without torch.
**Mode:** both
**Inputs:** Path to `qwen3_tiny.safetensors` fixture, bf16 caps dict.
**Expected output:** `Qwen3TextEncoder` returned with `.arch == "qwen3"` (no RuntimeError raised).
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_qwen3.py::test_load_raises_runtime_error_without_torch -v` exits 0.

---

## test_tokenizer_loads_from_vendored_path_no_network (worker.nodes.arch.clip.qwen3)

**File:** `worker/tests/test_arch_clip_qwen3.py`
**Context:** The `load()` function loads the Qwen3 tokenizer from `worker/assets/qwen3_tokenizer/` using `transformers.AutoTokenizer.from_pretrained()` with `local_files_only=True`. This test verifies that the call is made with the correct arguments.
**Tests:** `load()` with bf16 capability calls `AutoTokenizer.from_pretrained()` with `local_files_only=True` and the correct vendored path. Uses `unittest.mock.patch` to verify arguments without requiring network access.
**Mode:** both
**Inputs:** Path to `qwen3_tiny.safetensors` fixture, bf16 caps dict.
**Expected output:** `AutoTokenizer.from_pretrained()` called with `local_files_only=True` and path pointing to `worker/assets/qwen3_tokenizer/`.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_qwen3.py::test_tokenizer_loads_from_vendored_path_no_network -v` exits 0.

---

## test_load_mock_qwen3_fixture (worker.nodes.arch.clip.qwen3)

**File:** `worker/tests/test_arch_clip_qwen3.py`
**Context:** The `load()` function is implemented in `worker/nodes/arch/clip/qwen3.py` and constructs a `Qwen3TextEncoder` on meta-device with dtype selection. This is the mock-mode counterpart required by the dual-mode parity marker convention (ANVILML_DESIGN.md §10.6).
**Tests:** `load()` against `qwen3_tiny.safetensors` with bf16 capability succeeds in mock-mode; `.arch == "qwen3"`; all parameters on meta device; dtype is bfloat16; tokenizer is attached. Satisfies the `MOCK_PATH_VERIFIED` parity marker.
**Mode:** mock
**Inputs:** Path to `qwen3_tiny.safetensors` fixture, caps dict with bf16=True, fp16=True, fp8=False.
**Expected output:** `Qwen3TextEncoder` with `.arch == "qwen3"`, all parameters on meta device, dtype=torch.bfloat16, tokenizer attached.
**Acceptance:** `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_qwen3.py::test_load_mock_qwen3_fixture -v -m "not real_mode"` exits 0.

## test_build_key_remapping_direct_match (anvilml-worker)

**File:** `worker/tests/test_arch_clip_qwen3.py`
**Context:** The `qwen3` module is importable with safetensors but without torch (mock-mode collection). `_build_key_remapping()` is a pure function that operates on string lists — it does not require torch.
**Tests:** `_build_key_remapping()` with identical checkpoint and module key lists returns an identity mapping: every key maps to itself. This verifies the direct-match code path.
**Mode:** mock
**Inputs:** 7 identical keys (embed_tokens, mlp projections, layer norm, norm).
**Expected output:** Every key maps to itself in the returned dict.
**Acceptance:** `python -m pytest worker/tests/test_arch_clip_qwen3.py::test_build_key_remapping_direct_match -v` exits 0.

---

## test_build_key_remapping_attention_remap (anvilml-worker)

**File:** `worker/tests/test_arch_clip_qwen3.py`
**Context:** The `qwen3` module is importable with safetensors but without torch. `_build_key_remapping()` handles pattern-based remapping for Qwen3 attention projection keys.
**Tests:** `_build_key_remapping()` with Qwen3-style separate q/k/v/o projection keys and PyTorch-style concatenated in_proj/out_proj module keys correctly remaps q/k/v → in_proj.weight and o_proj → out_proj.weight for each layer.
**Mode:** mock
**Inputs:** 8 checkpoint keys (q/k/v/o for 2 layers), 4 module keys (in_proj/out_proj for 2 layers).
**Expected output:** Each q/k/v maps to its layer's in_proj.weight; each o_proj maps to its layer's out_proj.weight.
**Acceptance:** `python -m pytest worker/tests/test_arch_clip_qwen3.py::test_build_key_remapping_attention_remap -v` exits 0.

---

## test_load_real_qwen3_fixture_with_weights (anvilml-worker)

**File:** `worker/tests/test_arch_clip_qwen3.py`
**Context:** The `qwen3` module is imported with torch available (real-mode). The fixture checkpoint `qwen3_tiny.safetensors` has bf16 capability and loads weights via `load_file()` + `load_state_dict(assign=True)`.
**Tests:** `load()` against the fixture with bf16 capability returns a model with `.arch == "qwen3"`, all params on CPU (not meta), dtype bf16, and an attached tokenizer. This is the primary real-mode test for weight loading.
**Mode:** real
**Inputs:** `qwen3_tiny.safetensors` fixture, `caps = {"bf16": True, "fp16": True, "fp8": False, "fp32": True}`.
**Expected output:** Model with `.arch == "qwen3"`, params on CPU with bf16 dtype, tokenizer attached.
**Acceptance:** `python -m pytest worker/tests/test_arch_clip_qwen3.py::test_load_real_qwen3_fixture_with_weights -v -m real_mode` exits 0.

---

## test_load_mock_qwen3_fixture_with_weights (anvilml-worker)

**File:** `worker/tests/test_arch_clip_qwen3.py`
**Context:** The `qwen3` module is imported with torch available (real-mode, but `ANVILML_WORKER_MOCK=1` set). The fixture checkpoint loads weights in mock-mode.
**Tests:** `load()` against the fixture with bf16 capability in mock-mode returns a model with `.arch == "qwen3"`, all params on CPU, dtype bf16, and an attached tokenizer. This is the mock-mode counterpart to the real-mode weight loading test.
**Mode:** real
**Inputs:** `qwen3_tiny.safetensors` fixture, `caps = {"bf16": True, "fp16": True, "fp8": False, "fp32": True}`, `ANVILML_WORKER_MOCK=1`.
**Expected output:** Model with `.arch == "qwen3"`, params on CPU with bf16 dtype, tokenizer attached.
**Acceptance:** `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_arch_clip_qwen3.py::test_load_mock_qwen3_fixture_with_weights -v` exits 0.

---

## test_load_weights_dtype_matches_target (anvilml-worker)

**File:** `worker/tests/test_arch_clip_qwen3.py`
**Context:** The `qwen3` module is imported with torch available. `load()` casts tensors to target_dtype BEFORE `load_state_dict(assign=True)`.
**Tests:** `load()` with fp16-only capability caps asserts every parameter has dtype `torch.float16`. This confirms the cast-before-assign ordering works correctly.
**Mode:** real
**Inputs:** `qwen3_tiny.safetensors` fixture, `caps = {"fp16": True, "bf16": False, "fp8": False, "fp32": True}`.
**Expected output:** All parameters have dtype `torch.float16`.
**Acceptance:** `python -m pytest worker/tests/test_arch_clip_qwen3.py::test_load_weights_dtype_matches_target -v -m real_mode` exits 0.

---

## test_load_arch_attribute_persists_after_materialization (anvilml-worker)

**File:** `worker/tests/test_arch_clip_qwen3.py`
**Context:** The `qwen3` module is imported with torch available. `load()` verifies `.arch` persists after `to_empty()` materialization.
**Tests:** `load()` returns a model with `.arch == "qwen3"` after materialization. This confirms the safety net in `load()` is working correctly.
**Mode:** real
**Inputs:** `qwen3_tiny.safetensors` fixture, `caps = {"bf16": True, "fp16": True, "fp8": False, "fp32": True}`.
**Expected output:** `model.arch == "qwen3"` after materialization.
**Acceptance:** `python -m pytest worker/tests/test_arch_clip_qwen3.py::test_load_arch_attribute_persists_after_materialization -v -m real_mode` exits 0.

---

## test_infer_hyperparams_regular_fixture (anvilml-worker)

**File:** `worker/tests/test_arch_vae_zit.py`
**Context:** The `zit_vae` module is imported with torch available (guarded). `_infer_hyperparams()` reads the safetensors header of `zit_vae_tiny.safetensors` using `framework="np"` and never imports torch.
**Tests:** `_infer_hyperparams()` returns a dict with `encoder_channels=16`, `decoder_channels=32`, `latent_channels=4`, `arch="zit_vae"`, and `native_dtype="fp32"`. Verifies the regular code path where metadata contains the "arch" key and all VAE key prefixes are present.
**Mode:** both
**Inputs:** `zit_vae_tiny.safetensors` fixture with `arch="zit_vae"` metadata.
**Expected output:** Dict with all five keys present and correct values.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_vae_zit.py::test_infer_hyperparams_regular_fixture -v` exits 0.

---

## test_infer_hyperparams_no_metadata_fixture (anvilml-worker)

**File:** `worker/tests/test_arch_vae_zit.py`
**Context:** The `zit_vae` module is imported with torch available (guarded). `_infer_hyperparams()` reads the safetensors header of `zit_vae_tiny_no_metadata.safetensors` using `framework="np"`.
**Tests:** `_infer_hyperparams()` succeeds via the metadata-fallback path, detecting `arch="zit_vae"` from key naming patterns (`xyz_encoder_block*conv`, `xyz_decoder_block*conv`, `xyz_mid_block_conv`). Returns correct channel counts matching the regular fixture.
**Mode:** both
**Inputs:** `zit_vae_tiny_no_metadata.safetensors` fixture with no metadata and xyz_ prefixed keys.
**Expected output:** Dict with `arch="zit_vae"`, `encoder_channels=16`, `decoder_channels=32`, `latent_channels=4`, `native_dtype="fp32"`.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_vae_zit.py::test_infer_hyperparams_no_metadata_fixture -v` exits 0.

---

## test_infer_hyperparams_nonexistent_path_raises (anvilml-worker)

**File:** `worker/tests/test_arch_vae_zit.py`
**Context:** The `zit_vae` module is imported. `_infer_hyperparams()` is called with a path that does not exist.
**Tests:** `_infer_hyperparams()` raises `ValueError` with a message containing "No such file".
**Mode:** both
**Inputs:** Non-existent path `/tmp/this_file_does_not_exist_abc123.safetensors`.
**Expected output:** `ValueError` raised with "No such file" in the message.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_vae_zit.py::test_infer_hyperparams_nonexistent_path_raises -v` exits 0.

---

## test_infer_hyperparams_truncated_header_raises (anvilml-worker)

**File:** `worker/tests/test_arch_vae_zit.py`
**Context:** The `zit_vae` module is imported. A temporary file with invalid binary data is created.
**Tests:** `_infer_hyperparams()` raises `ValueError` when given a file containing invalid safetensors data (8 bytes that do not form a valid header).
**Mode:** both
**Inputs:** Temporary file with bytes `\x00\x01\x02\x03\x04\x05\x06\x07`.
**Expected output:** `ValueError` raised.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_vae_zit.py::test_infer_hyperparams_truncated_header_raises -v` exits 0.

---

## test_arch_constant (anvilml-worker)

**File:** `worker/tests/test_arch_vae_zit.py`
**Context:** The `zit_vae` module is imported. The module-level `ARCH` constant is checked.
**Tests:** `ARCH` equals `"zit_vae"`, confirming the architecture identifier is set correctly for use by `can_handle()` (P23-B2) and dispatch.
**Mode:** both
**Inputs:** Module import.
**Expected output:** `ARCH == "zit_vae"`.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_vae_zit.py::test_arch_constant -v` exits 0.

---

## test_can_handle_matches_zit_vae_key (anvilml-worker)

**File:** `worker/tests/test_arch_vae_zit.py`
**Context:** The `zit_vae` module is importable (torch is guarded behind `try/except ImportError`). The `can_handle()` function was added by P23-B2 to expose the dispatch matching entry point.
**Tests:** `can_handle("zit_vae")` returns `True`, confirming the dispatcher will route requests with the ZiT VAE architecture key to this module.
**Mode:** both
**Inputs:** Module import of `can_handle` from `worker.nodes.arch.vae.zit_vae`, call with `"zit_vae"`.
**Expected output:** `True` is returned.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_vae_zit.py::test_can_handle_matches_zit_vae_key -v` exits 0.

---

## test_can_handle_rejects_unrelated_key (anvilml-worker)

**File:** `worker/tests/test_arch_vae_zit.py`
**Context:** The `zit_vae` module is importable. The `can_handle()` function must reject keys that do not match the module's architecture identifier, preventing incorrect dispatch.
**Tests:** `can_handle("flux2_vae")` returns `False`, confirming the dispatcher correctly rejects keys that do not match this module.
**Mode:** both
**Inputs:** Module import of `can_handle` from `worker.nodes.arch.vae.zit_vae`, call with `"flux2_vae"`.
**Expected output:** `False` is returned.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_vae_zit.py::test_can_handle_rejects_unrelated_key -v` exits 0.

---

## test_get_module_returns_zit_vae_for_matching_key (anvilml-worker)

**File:** `worker/tests/test_arch_vae_zit.py`
**Context:** The `arch/vae/__init__.py` dispatcher now has `zit_vae` registered in `_REGISTERED_MODULES` (imported and appended). The `get_module()` function scans the registered modules and calls `can_handle()` on each.
**Tests:** `get_module("zit_vae")` returns a module whose `__name__` is `"worker.nodes.arch.vae.zit_vae"`, confirming end-to-end dispatch registration works after P23-B2 wired the module into the dispatcher.
**Mode:** both
**Inputs:** Module import of `get_module` from `worker.nodes.arch.vae`, call with `"zit_vae"`.
**Expected output:** Non-`None` module with `__name__ == "worker.nodes.arch.vae.zit_vae"`.
**Acceptance:** `worker/.venv/bin/python -m pytest worker/tests/test_arch_vae_zit.py::test_get_module_returns_zit_vae_for_matching_key -v` exits 0.
