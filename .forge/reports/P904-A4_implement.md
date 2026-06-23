# Implementation Report: P904-A4

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P904-A4                         |
| Phase         | 904 — Bug fix                   |
| Description   | worker/nodes/sampler.py: EmptyLatent real path references unbound name ctx instead of self.ctx |
| Implemented   | 2026-06-23T21:38:00Z            |
| Status        | COMPLETE                        |

## Summary

Fixed a NameError bug in `worker/nodes/sampler.py` where `EmptyLatent.execute()`'s real-mode branch referenced an unbound variable `ctx` instead of the correct bound instance attribute `self.ctx`. The fix is a one-line substitution on line 183: `device=ctx.device` → `device=self.ctx.device`. The `self.ctx` attribute was already correctly used elsewhere in the same file (e.g., `Sampler.execute()` at lines 309, 331, 332), confirming the correct pattern.

## Resolved Dependencies

No dependencies were added or modified. This is a pure bug fix with no manifest changes.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modified | `worker/nodes/sampler.py` | Fixed unbound `ctx` → `self.ctx` on line 183 |

## Commit Log

```
 .forge/reports/P904-A4_plan.md | 97 ++++++++++++++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md   |  6 +--
 .forge/state/state.json        | 13 +++---
 worker/nodes/sampler.py        |  2 +-
 4 files changed, 108 insertions(+), 10 deletions(-)
```

The only production code change is `worker/nodes/sampler.py` (1 line changed).

## Test Results

### Python tests (91 passed, 0 failed)

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.0, pluggy-1.6.0
collected 91 items

worker/tests/test_nodes_sampler.py::test_emptylatent_registered_in_registry PASSED
worker/tests/test_nodes_sampler.py::test_emptylatent_execute_returns_mock_latent PASSED
worker/tests/test_nodes_sampler.py::test_emptylatent_default_batch_size PASSED
worker/tests/test_nodes_sampler.py::test_sampler_registered_in_registry PASSED
worker/tests/test_nodes_sampler.py::test_sampler_execute_returns_mock_latent_and_seed PASSED
worker/tests/test_nodes_sampler.py::test_sampler_seed_negative_one_resolves_to_random PASSED
worker/tests/test_nodes_sampler.py::test_sampler_emits_progress_flag PASSED
worker/tests/test_nodes_sampler.py::test_sampler_metadata_attributes PASSED
worker/tests/test_nodes_sampler.py::test_emptylatent_metadata_attributes PASSED
... (84 more tests all PASSED) ...

============================= 91 passed in 19.24s ==============================
```

### Rust tests (180+ passed, 0 failed)

```
cargo test --workspace --features mock-hardware
  anvilml: 1 test passed (cli_custom_port_health)
  anvilml-artifacts: 5 tests passed
  anvilml-core: 13 tests passed (artifact, config, error, events, hardware, job, model, node, worker)
  anvilml-hardware: 29 tests passed (cpu, device_db, dxgi_sysfs, mock, vulkan)
  anvilml-ipc: 22 tests passed (roundtrip, stress, transport)
  anvilml-registry: 23 tests passed (db, device_store, scanner, seed_loader, store)
  anvilml-scheduler: 48 tests passed (dag, dispatch, event_loop, image_ready, ledger, model_resolve, node_registry, progress, queue, scheduler_cancel, scheduler)
  anvilml-server: 30 tests passed (artifact_store, artifacts, broadcaster, handler, health, jobs, models, nodes, state, stats_tick, system, workers)
  anvilml-worker: 23 tests passed (bridge, demux, env, keepalive, managed, pool, respawn, spawn)
  Doc-tests: 1 passed

  Total: 0 failures
```

## Format Gate

```
cargo fmt --all -- --check
# Exit 0 — no formatting drift.
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.29s

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.56s

# 3. Real-hardware Linux
cargo check --bin anvilml
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.24s

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.26s
```

All four checks exited 0.

## Project Gates

### Gate 1 — Config Surface Sync
```
cargo test -p anvilml --features mock-hardware -- config_reference
  test config_reference ... ok
  test result: ok. 1 passed; 0 failed
```

### Gate 2 — OpenAPI Drift
```
cargo run -p anvilml-openapi && git diff --exit-code api/openapi.json
  # Exit 0 — no drift.
```

### Gate 3 — Node Parity
```
worker/tests/test_parity.py does not yet exist in this repo phase.
Not applicable — no parity test file present.
```

## Public API Delta

```
git diff HEAD -- worker/nodes/sampler.py | grep '^+.*pub ' | head -40
# No output — no new pub items introduced.
```

This is a Python bug fix; no public API surface changed.

## Deviations from Plan

None. The implementation exactly matches the approved plan:
- Line 183 changed from `device=ctx.device` to `device=self.ctx.device`
- `grep -n "device=ctx\." worker/nodes/sampler.py` returns zero matches
- `grep -n "device=self\.ctx\.device" worker/nodes/sampler.py` returns one match (line 183)

Note: The plan mentioned that line 331 from `Sampler` would also match the second grep, but line 331 uses `self.ctx.device` as a positional argument in `mod.sample()` (not as a keyword argument `device=self.ctx.device`), so the grep returns exactly one match at line 183. This is a minor discrepancy in the plan's description, not in the code.

## Blockers

None.
