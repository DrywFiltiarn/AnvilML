# Plan Report: P11-C2

| Field       | Value                                           |
|-------------|-------------------------------------------------|
| Task ID     | P11-C2                                          |
| Phase       | 011 — Graph Validation                          |
| Description | anvilml-worker: serialise spawning integration tests to eliminate env-var race on Windows |
| Depends on  | P11-C1                                          |
| Project     | anvilml                                         |
| Planned at  | 2026-06-07T11:53:00Z                            |
| Attempt     | 1                                               |

## Objective

Eliminate cross-test env-var contamination in the four spawning integration tests of `anvilml-worker` by serialising their execution with the `serial_test` crate. The tests `spawn_ping_pong`, `status_transitions`, `handshake_completes_once`, and `spawn_reaches_idle` all call `std::env::set_var("ANVILML_WORKER_MOCK", "1")`, which mutates process-global state. Cargo's test harness runs tests in parallel OS threads, causing Windows-specific races where one thread reads env vars mutated by another test mid-flight.

## Scope

### In Scope
- Add `serial_test = "1"` to `[dev-dependencies]` in `crates/anvilml-worker/Cargo.toml` (workspace = false, plain version string)
- Annotate all four spawning integration tests with `#[serial_test::serial]` inside `crates/anvilml-worker/src/managed.rs`
- No changes to test logic, no new tests, no changes to other crates

### Out of Scope
- Any changes to test assertions or test bodies
- Changes to non-spawning tests (`eof_sets_dead`, `keepalive_pings_and_kills_on_timeout`, `respawn_after_death`) — these do not call `set_var` for mock-mode and are unaffected
- Version bumps (no source files modified, only Cargo.toml dependency addition and test attribute annotations)

## Approach

1. **Add dev-dependency** in `crates/anvilml-worker/Cargo.toml`: add a new line `serial_test = "1"` under the `[dev-dependencies]` section. This pins to any 1.x release (verified: serial_test 1.x exists on crates.io with the same `#[serial]` attribute macro API as current versions).

2. **Annotate four tests** in `crates/anvilml-worker/src/managed.rs` — add `#[serial_test::serial]` between the existing `#[cfg(feature = "mock-hardware")]` and `async fn ...` lines for:
   - `spawn_ping_pong` (line ~698)
   - `status_transitions` (line ~784)
   - `handshake_completes_once` (line ~829)
   - `spawn_reaches_idle` (line ~1183)

   The annotation pattern:
   ```rust
   #[tokio::test]
   #[cfg(feature = "mock-hardware")]
   #[serial_test::serial]
   async fn spawn_ping_pong() { ... }
   ```

3. **Verify** by running:
   - `cargo test -p anvilml-worker --features mock-hardware` — expects exit 0, 0 ignored, 0 failed
   - `cargo check --target x86_64-pc-windows-gnu --features mock-hardware` — expects exit 0

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-worker/Cargo.toml` | Add `serial_test = "1"` to `[dev-dependencies]` |
| Modify | `crates/anvilml-worker/src/managed.rs` | Add `#[serial_test::serial]` attribute to 4 tests |

## Tests

<table>
<tr><th>Test File</th><th>Test Name</th><th>What It Verifies</th></tr>
<tr><td>crates/anvilml-worker/src/managed.rs (existing)</td><td>spawn_ping_pong</td><td>Serial execution — no env-var bleed from concurrent tests during spawn+ping+pong cycle</td></tr>
<tr><td>crates/anvilml-worker/src/managed.rs (existing)</td><td>status_transitions</td><td>Serial execution — status transitions correct without cross-test contamination</td></tr>
<tr><td>crates/anvilml-worker/src/managed.rs (existing)</td><td>handshake_completes_once</td><td>Serial execution — exactly one Ready event with no phantom events from other tests</td></tr>
<tr><td>crates/anvilml-worker/src/managed.rs (existing)</td><td>spawn_reaches_idle</td><td>Serial execution — spawn reaches Idle without timing workarounds</td></tr>
</table>

## CI Impact

No CI workflow files are modified. The change is purely a dev-dependency addition and test attribute annotations within `anvilml-worker`. The existing CI test command `cargo test --workspace --features mock-hardware` will automatically include the serialised tests. No new CI gates or jobs are needed.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `serial_test = "1"` version not found on crates.io | Low | Blocker — plan cannot proceed | Fallback: use latest available 1.x version from crates.io lookup; if unavailable, document gap and propose alternative |
| Test execution time increases due to serialisation | Medium | Low — tests run sequentially instead of in parallel | The four affected tests are fast (spawn+shutdown <5s each); total added wall-clock time is ~20s, acceptable for CI |
| `#[serial_test::serial]` conflicts with existing `#[tokio::test]` or `#[cfg(feature = "mock-hardware")]` attributes | Low | Compilation failure | Verified: `serial_test` supports async tests via its `async` feature (enabled by default); attribute stacking is standard Rust |
| Non-spawning tests (`eof_sets_dead`, etc.) unexpectedly affected | Low | Medium — test failures | These tests do not use `ANVILML_WORKER_MOCK`; `#[serial]` only affects annotated tests, so they remain unaffected. Verified by reading test bodies. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-worker --features mock-hardware` exits 0 with 0 ignored and 0 failed
- [ ] `cargo check --target x86_64-pc-windows-gnu --features mock-hardware` exits 0
- [ ] Exactly four tests have `#[serial_test::serial]` annotations added (no more, no fewer)
- [ ] No test logic, assertions, or imports are changed beyond the attribute addition and dependency declaration
- [ ] Non-spawning tests (`eof_sets_dead`, `keepalive_pings_and_kills_on_timeout`, `respawn_after_death`) remain unannotated
