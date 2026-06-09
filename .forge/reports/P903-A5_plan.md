# Plan Report: P903-A5

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P903-A5                                     |
| Phase       | 903 — IPC Transport Rework                  |
| Description | anvilml-worker: reactivate four ignored integration tests after Python socket implementation |
| Depends on  | P903-A3                                     |
| Project     | anvilml                                     |
| Planned at  | 2026-06-09T05:35:00Z                        |
| Attempt     | 1                                           |

## Objective

Remove the `#[ignore]` attribute (and its associated doc comment referencing P903-A3) from four integration tests in `crates/anvilml-worker/src/managed.rs` that were previously ignored pending the Python worker socket implementation. These tests must now run and pass without any modification to their logic or assertions.

## Scope

### In Scope
- Remove `#[ignore = "requires P903-A3: Python worker socket connection"]` from four tests:
  - `spawn_ping_pong` (line 840)
  - `status_transitions` (line 932)
  - `handshake_completes_once` (line 983)
  - `spawn_reaches_idle` (line 1373)
- Remove the doc comment `/// Ignored until P903-A3 updates the Python worker to connect to the socket.` from each of these four tests.
- Bump `anvilml-worker` crate patch version in `crates/anvilml-worker/Cargo.toml` from `0.1.17` to `0.1.18`.

### Out of Scope
- Any modification to test logic, assertions, or test fixtures.
- Any changes to other files in the workspace.
- Any changes to Python worker code.
- Any changes to CI configuration.
- Any changes to documentation.

## Approach

1. **Read** `crates/anvilml-worker/src/managed.rs` to confirm the exact text of the `#[ignore]` attributes and their preceding doc comments for all four target tests.
2. **Remove** the `#[ignore = "requires P903-A3: Python worker socket connection"]` attribute line from each of the four tests.
3. **Remove** the doc comment line `/// Ignored until P903-A3 updates the Python worker to connect to the socket.` that immediately precedes each `#[ignore]` attribute in the four tests.
4. **Bump** the `version` field in `crates/anvilml-worker/Cargo.toml` from `0.1.17` to `0.1.18`.
5. **Verify** with `cargo test -p anvilml-worker --features mock-hardware` that all four tests now run (not ignored) and pass, and the overall test exit code is 0.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-worker/src/managed.rs` | Remove `#[ignore]` and P903-A3 doc comment from four tests |
| Modify | `crates/anvilml-worker/Cargo.toml` | Bump patch version `0.1.17 → 0.1.18` |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|-----------------|
| `crates/anvilml-worker/src/managed.rs` | `spawn_ping_pong` | Spawn a mock worker, send Ping, receive Pong, then Shutdown — end-to-end IPC round-trip |
| `crates/anvilml-worker/src/managed.rs` | `status_transitions` | Status flows Initializing → Idle (on Ready) → Dead (on shutdown) |
| `crates/anvilml-worker/src/managed.rs` | `handshake_completes_once` | Exactly one `Ready` event after spawn; no duplicate Ready, Dying, or Dead during handshake drain |
| `crates/anvilml-worker/src/managed.rs` | `spawn_reaches_idle` | Canonical regression: spawn reaches Idle without timing workarounds |

## CI Impact

No CI changes required. The four tests were previously ignored (skipped by the test runner), so removing `#[ignore]` means they will now be executed as part of the standard test suite. The acceptance criterion requires `cargo test -p anvilml-worker --features mock-hardware` to exit 0 with all four tests passing, which is a prerequisite for marking the task complete. No CI workflow files are modified.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Tests fail because P903-A3 Python socket implementation is incomplete | Low | High | The task description states P903-A3 is complete and workers can connect to the socket. If tests fail, the blocker is that P903-A3 is not actually done — report as blocker and STOP. |
| Tests pass on local machine but fail in CI due to environment differences | Low | Medium | The tests use `ANVILML_WORKER_MOCK=1` which is already configured in CI. Verify CI environment matches local (Python path, venv path). |
| Removing doc comments changes test documentation that other tasks may reference | Low | Low | The doc comment is purely a historical note about P903-A3 dependency; it has no semantic meaning to other code or tasks. |
| Cargo.lock drift after version bump | Low | Low | Cargo.lock is regenerated automatically on next build — do not edit manually per FORGE_AGENT_RULES §12.4. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-worker --features mock-hardware` exits 0
- [ ] All four tests (`spawn_ping_pong`, `status_transitions`, `handshake_completes_once`, `spawn_reaches_idle`) appear in test output as `ok` (not `ignored` or `ignored: ...`)
- [ ] No other tests are affected (no new failures, no previously-passing tests now ignored)
- [ ] `anvilml-worker` version bumped to `0.1.18` in `crates/anvilml-worker/Cargo.toml`
