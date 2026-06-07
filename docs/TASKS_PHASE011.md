# Tasks: Phase 011 — Graph Validation

| Field | Value |
|-------|-------|
| Phase | 011 |
| Name | Graph Validation |
| Milestone group | End-to-end generation (mock) |
| Depends on phases | 1-10 |
| Task file | `forge/tasks/tasks_phase011.json` |
| Tasks | 9 |

## Overview

Phase 11 implements the DAG validator in `anvilml-scheduler`: the `KNOWN_NODE_TYPES` set and node slot table, duplicate-id / unknown-type / bad-edge / cycle checks (collecting all errors), and wires `POST /v1/jobs` to reject invalid graphs with 422. After this phase the API refuses malformed job graphs with a clear error list (valid graphs are accepted but not yet queued — that is phase 12).

Every task in this phase implements **one module or one endpoint** plus its test. No task touches more than its named file(s). `cargo test` and `cargo clippy` are per-task gates; the phase as a whole is only complete when the **Runnable Proof** below passes.

## Tasks

| Task | Module / File | Summary |
|------|-------------|---------|
| P11-A1 | `crates/anvilml-scheduler/src/nodes.rs` | anvilml-scheduler: KNOWN_NODE_TYPES + node slot table |
| P11-A2 | `crates/anvilml-scheduler/src/dag.rs` | anvilml-scheduler: dag.rs duplicate-id + unknown-type checks |
| P11-A3 | `crates/anvilml-scheduler/src/dag.rs` | anvilml-scheduler: dag.rs edge-reference validation |
| P11-A4 | `crates/anvilml-scheduler/src/dag.rs` | anvilml-scheduler: dag.rs cycle detection (Kahn) |
| P11-A5 | `crates/anvilml-server/src/handlers/jobs.rs` | anvilml-server: POST /v1/jobs validating graph (422 on invalid) |
| P11-B1 | `crates/anvilml-hardware/src/mock.rs`, `src/lib.rs` | anvilml-hardware: serial mock test env-var teardown |
| P11-C1 | `crates/anvilml-worker/src/managed.rs` | anvilml-worker: fix relative venv path causing Windows spawn ERROR_PATH_NOT_FOUND |
| P11-C2 | `crates/anvilml-worker/Cargo.toml`, `crates/anvilml-worker/src/managed.rs` | anvilml-worker: serialise spawning integration tests to eliminate env-var race on Windows |
| P11-C3 | `crates/anvilml-worker/src/managed.rs` | anvilml-worker: fix venv path resolution base — use repo root not current_dir |

## Task details

#### P11-A1: anvilml-scheduler: KNOWN_NODE_TYPES + node slot table

- **Prereqs:** P10-A4
- **Tags:** —

Add anvilml-core to anvilml-scheduler. Create src/nodes.rs: const KNOWN_NODE_TYPES (9 names: ZitLoadPipeline, ZitTextEncode, ZitSampler, ZitDecode, SdxlLoadPipeline, SdxlTextEncode, SdxlSampler, SdxlDecode, SaveImage). NODE_SLOTS map type->(input_slots, output_slots) per ANVILML_DESIGN 14.6. cargo test -p anvilml-scheduler -- nodes exits 0: all 9 present; ZitSampler outputs include latents+seed.

#### P11-A2: anvilml-scheduler: dag.rs duplicate-id + unknown-type checks

- **Prereqs:** P11-A1
- **Tags:** —

Create src/dag.rs: ValidatedGraph newtype(Value). fn validate_graph(v:&Value)->Result<ValidatedGraph,Vec<String>>. Implement checks (collect all errors, non-fail-fast): duplicate node id -> 'duplicate_node_id: {id}'; node.type not in KNOWN_NODE_TYPES -> 'unknown_node_type: {type}'. (edge + cycle checks added next task.) cargo test -p anvilml-scheduler -- dag_basic exits 0: dup ids, unknown type each reported.

#### P11-A3: anvilml-scheduler: dag.rs edge-reference validation

- **Prereqs:** P11-A2
- **Tags:** reasoning

Extend dag.rs: for each input that is an object {node_id,output_slot}: error 'unknown_node_ref: {node_id}' if node_id absent; 'unknown_output_slot: {node_id}.{slot}' if referenced node's type does not declare that output slot in NODE_SLOTS. cargo test -p anvilml-scheduler -- dag_edges exits 0: bad ref + bad slot each reported; valid edge passes.

#### P11-A4: anvilml-scheduler: dag.rs cycle detection (Kahn)

- **Prereqs:** P11-A3
- **Tags:** reasoning

Extend dag.rs: build adjacency from edge refs, run Kahn topo-sort; if processed<total, collect unprocessed ids -> 'cycle_detected: {ids}'. validate_graph returns Ok(ValidatedGraph) only when all checks pass. cargo test -p anvilml-scheduler -- dag_cycle exits 0: 2-node cycle reported; valid ZiT 5-node graph passes clean.

#### P11-A5: anvilml-server: POST /v1/jobs validating graph (422 on invalid)

- **Prereqs:** P11-A4
- **Tags:** —

Add anvilml-scheduler to anvilml-server. Create handlers/jobs.rs submit_job(State, Json<SubmitJobRequest>): call validate_graph(&req.graph); on Err return 422 with body {error:'invalid_graph', message: errors joined, request_id}. On Ok do NOT yet enqueue (queue is phase 12) - return 202 with a placeholder SubmitJobResponse{job_id:new uuid, queue_position:0}. Wire POST /v1/jobs. Verify: curl -X POST /v1/jobs -d '{bad graph}' returns 422 listing offending types; a valid ZiT graph returns 202.

### Group B — Test Reliability

#### P11-B1: anvilml-hardware: add clear_mock_env teardown to all serial mock tests

- **Prereqs:** P11-A1
- **Tags:** —

`serial_test` serialises all `#[serial]` tests within a process under a single
mutex. When a test sets `ANVILML_MOCK_VRAM_MIB` (or any other `ANVILML_MOCK_*`
var) and exits without removing it, the next test in scheduler order inherits
the stale value. This causes `mock_detect_default_cpu` to see `vram_total_mib
== 12288` (left by `detect_all_devices_mock_vram`) instead of the expected
default `8192`, producing a randomly-ordered CI failure.

Add `fn clear_mock_env()` (private, `#[cfg(test)]`) to both `mock.rs` and
`lib.rs`:

```rust
fn clear_mock_env() {
    std::env::remove_var("ANVILML_MOCK_DEVICE_TYPE");
    std::env::remove_var("ANVILML_MOCK_VRAM_MIB");
    std::env::remove_var("ANVILML_MOCK_GFX_ARCH");
}
```

Call `clear_mock_env()` as the **last statement** of every `#[serial]` test
that sets any of these vars. Affected tests:

- `mock.rs`: `mock_detect_default_cpu`, `mock_detect_cuda`, `mock_detect_rocm`,
  `mock_device_new_fields`
- `lib.rs`: `detect_all_devices_mock_cuda`, `detect_all_devices_mock_rocm`,
  `detect_all_devices_mock_vram`, `detect_all_devices_mock_device_type`,
  `detect_all_devices_mock_enum_source`, `mock_device_new_fields_in_detect_all`

Do **not** add `set_var` calls for vars a test does not itself set — that would
change test semantics. The teardown alone is sufficient because every test that
reads a default already implicitly relies on the var being absent; removing it
at the end of the preceding test restores that invariant.

Acceptance: `cargo test -p anvilml-hardware --features mock-hardware` exits 0
with all 48 tests passing. The fix must hold across 20 consecutive runs with
`cargo test ... -- --test-threads=1` to verify no ordering-dependent failure
remains.


#### P11-C1: anvilml-worker: fix relative venv path causing Windows spawn ERROR_PATH_NOT_FOUND

- **Prereqs:** P10-B4
- **Tags:** reasoning

`spawn()` sets `.current_dir(_repo_root_for_worker())` on the child `Command`. On Windows, `CreateProcess` resolves a relative executable path against the *child's* working directory, not the parent process CWD. `ANVILML_VENV_PATH=.ci-venv` (CI) and `default_venv_path() = "./venv"` (default) are both relative paths. When `_repo_root_for_worker()` — which is built from the compile-time `CARGO_MANIFEST_DIR` — differs from the runtime CWD, the relative venv path resolves to a non-existent location and `CreateProcess` returns `ERROR_PATH_NOT_FOUND` (os error code 3).

In `spawn()`, immediately after reading `cfg.venv_path`, resolve it to an absolute path before passing it to `resolve_python_path`:

```rust
let abs_venv = if cfg.venv_path.is_absolute() {
    cfg.venv_path.clone()
} else {
    std::env::current_dir().unwrap_or_default().join(&cfg.venv_path)
};
let python_path = resolve_python_path(&abs_venv);
```

No other changes. `resolve_python_path`, `_repo_root_for_worker`, and the test logic are all unchanged.

`cargo test -p anvilml-worker --features mock-hardware` exits 0 on both platforms. `cargo check --target x86_64-pc-windows-gnu --features mock-hardware` exits 0.

#### P11-C2: anvilml-worker: serialise spawning integration tests to eliminate env-var race on Windows

- **Prereqs:** P11-C1
- **Tags:** reasoning

The four spawning integration tests (`spawn_ping_pong`, `status_transitions`, `handshake_completes_once`, `spawn_reaches_idle`) call `std::env::set_var("ANVILML_WORKER_MOCK", "1")` which mutates process-global state. Cargo's test harness runs tests in parallel OS threads by default. On Windows this causes cross-test env-var contamination: a thread executing `ManagedWorker::new()` or `spawn()` in test A reads env vars mutated by test B mid-flight. This manifests as the wrong test name appearing in another test's backtrace and as intermittent `PATH_NOT_FOUND` or handshake timeout failures.

Add `serial_test = "1"` to `[dev-dependencies]` in `crates/anvilml-worker/Cargo.toml`. Annotate all four spawning tests with `#[serial_test::serial]`:

```rust
#[tokio::test]
#[cfg(feature = "mock-hardware")]
#[serial_test::serial]
async fn spawn_ping_pong() { ... }
```

Apply the same annotation to `status_transitions`, `handshake_completes_once`, and `spawn_reaches_idle`. No test logic changes. The non-spawning tests (`eof_sets_dead`, `keepalive_pings_and_kills_on_timeout`, `respawn_after_death`) do not use `set_var` for mock-mode and do not need `#[serial]`.

`cargo test -p anvilml-worker --features mock-hardware` exits 0 with 0 ignored and 0 failed on both platforms. `cargo check --target x86_64-pc-windows-gnu --features mock-hardware` exits 0.

#### P11-C3: anvilml-worker: fix venv path resolution base — use repo root not current_dir

- **Prereqs:** P11-C2
- **Tags:** reasoning

P11-C1 resolved `cfg.venv_path` against `std::env::current_dir()`. This is incorrect: `cargo test` sets the process CWD to the crate directory (`crates/anvilml-worker/`), not the repo root. The CI setup step creates `.ci-venv` at the repo root, so `current_dir().join(".ci-venv")` resolves to `crates/anvilml-worker/.ci-venv` — a path that does not exist — giving `ENOENT` on Linux (code 2) and `ERROR_PATH_NOT_FOUND` on Windows (code 3).

The correct base is `_repo_root_for_worker()`, which already returns the canonical absolute repo root derived from the compile-time `CARGO_MANIFEST_DIR`. This is stable regardless of invocation context (binary, `cargo test`, direct execution).

In `spawn()`, replace the `current_dir()` call with `_repo_root_for_worker()`:

```rust
// Replace:
std::env::current_dir().unwrap_or_default().join(&cfg.venv_path)
// With:
_repo_root_for_worker().join(&cfg.venv_path)
```

Remove the now-unused `std::env::current_dir()` call. The `is_absolute()` guard and the rest of `spawn()` are unchanged.

`cargo test -p anvilml-worker --features mock-hardware` exits 0 on both platforms. `cargo check --target x86_64-pc-windows-gnu --features mock-hardware` exits 0.

## Runnable Proof

Submit a bad graph and a good graph; confirm 422 vs 202.

```bash
cargo run --features mock-hardware
# Bad graph — unknown node type → expect 422
curl -s -X POST http://127.0.0.1:8488/v1/jobs \
  -H 'content-type: application/json' \
  -d '{"graph":{"nodes":[{"id":"n0","type":"NopeNode","inputs":{}}]},"settings":{"seed":-1,"steps":8,"guidance_scale":7.5,"width":1024,"height":1024}}' \
  | python -m json.tool

# Valid ZiT 5-node graph → expect 202 with job_id
curl -s -X POST http://127.0.0.1:8488/v1/jobs \
  -H 'content-type: application/json' \
  -d @valid_zit_job.json \
  | python -m json.tool
```

Expected: 422 with body `{"error":"invalid_graph","message":"unknown_node_type: NopeNode", ...}`. A valid two-node ZiT graph (ZitLoadPipeline -> ... -> SaveImage) returns 202. Phase done when invalid graphs are rejected with 422 listing the specific problems and `cargo test -p anvilml-scheduler -- dag` is green.
