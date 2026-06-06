# Tasks: Phase 011 — Graph Validation

| Field | Value |
|-------|-------|
| Phase | 011 |
| Name | Graph Validation |
| Milestone group | End-to-end generation (mock) |
| Depends on phases | 1-10 |
| Task file | `forge/tasks/tasks_phase011.json` |
| Tasks | 6 |

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

## Runnable Proof

Submit a bad graph and a good graph; confirm 422 vs 202.

```bash
cargo run --features mock-hardware
# bad graph: unknown node type
curl -s -X POST http://127.0.0.1:8488/v1/jobs \
  -H 'content-type: application/json' \
  -d '{"graph":{"nodes":[{"id":"n0","type":"NopeNode","inputs":{}}]},"settings":{"seed":-1,"steps":8,"guidance_scale":0.0,"width":1024,"height":1024}}'
```

Expected: 422 with body `{"error":"invalid_graph","message":"unknown_node_type: NopeNode", ...}`. A valid two-node ZiT graph (ZitLoadPipeline -> ... -> SaveImage) returns 202. Phase done when invalid graphs are rejected with 422 listing the specific problems and `cargo test -p anvilml-scheduler -- dag` is green.
