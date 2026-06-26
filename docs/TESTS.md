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
