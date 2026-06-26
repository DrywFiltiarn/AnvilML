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
