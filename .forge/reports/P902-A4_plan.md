# Plan Report: P902-A4

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P902-A4                                           |
| Phase       | 902 — Stabilisation Retrofit                      |
| Description | anvilml-worker: replace serial_test env-var workaround with scoped env isolation |
| Depends on  | none                                              |
| Project     | anvilml                                           |
| Planned at  | 2026-06-08T15:00:00Z                              |
| Attempt     | 1                                                 |

## Objective

Replace the `std::env::set_var("ANVILML_WORKER_MOCK", "1")` + `#[serial_test::serial]` workaround in four spawning tests of `crates/anvilml-worker/src/managed.rs` with scoped environment-variable isolation using `temp_env::async_with_vars`. This eliminates the need for serial test execution and the `serial_test` dependency entirely.

## Scope

### In Scope
- Add `temp-env = { version = "0.3", features = ["async_closure"] }` to `[dev-dependencies]` in `crates/anvilml-worker/Cargo.toml`.
- Remove `serial_test = "1"` from `[dev-dependencies]` in `crates/anvilml-worker/Cargo.toml`.
- In `crates/anvilml-worker/src/managed.rs`, replace all four spawning tests:
  - `spawn_ping_pong` — wrap body with `temp_env::async_with_vars([("ANVILML_WORKER_MOCK", Some("1"))], async { ... }).await`; remove `std::env::set_var` at top and `std::env::remove_var` at bottom; remove `#[serial_test::serial]`.
  - `status_transitions` — same pattern as above.
  - `handshake_completes_once` — wrap body with `temp_env::async_with_vars([("ANVILML_WORKER_MOCK", Some("1")), ("ANVILML_PING_INTERVAL_MS", Some("50")), ("ANVILML_PONG_TIMEOUT_MS", Some("150"))], async { ... }).await`; remove all three `std::env::set_var` calls at top, the save/restore logic for those vars and the teardown block at bottom; remove `#[serial_test::serial]`.
  - `spawn_reaches_idle` — same pattern as `spawn_ping_pong`.
- Bump `anvilml-worker` patch version from `0.1.12` to `0.1.13` in `crates/anvilml-worker/Cargo.toml`.

### Out of Scope
- No changes to test assertions or logic.
- No changes to non-test code paths.
- No changes to any other crate's Cargo.toml or source files.
- No changes to CI configuration.
- No changes to `eof_sets_dead` or `keepalive_pings_and_kills_on_timeout` tests (they don't set `ANVILML_WORKER_MOCK`).

## Approach

1. **Add `temp-env` dev-dependency.** In `crates/anvilml-worker/Cargo.toml`, add `temp-env = { version = "0.3", features = ["async_closure"] }` under `[dev-dependencies]`. The `async_closure` feature is required to expose the `async_with_vars` function (see dependency verification below).

2. **Remove `serial_test` dev-dependency.** Delete the line `serial_test = "1"` from `[dev-dependencies]` in `crates/anvilml-worker/Cargo.toml`. Note: `serial_test` is also listed in the workspace root `[workspace.dependencies]` at version `3.5.0`, but that does not affect this crate since it uses a direct local override `"1"`.

3. **Bump crate version.** Change `version = "0.1.12"` to `version = "0.1.13"` in the `[package]` section of `crates/anvilml-worker/Cargo.toml`.

4. **Refactor `spawn_ping_pong` test.** Remove the `#[serial_test::serial]` attribute. Remove `std::env::set_var("ANVILML_WORKER_MOCK", "1")` from the top of the body and `std::env::remove_var("ANVILML_WORKER_MOCK")` from the teardown. Wrap the entire async function body with:
   ```rust
   temp_env::async_with_vars(
       [("ANVILML_WORKER_MOCK", Some("1"))],
       async { <existing test body> },
   )
   .await;
   ```

5. **Refactor `status_transitions` test.** Same pattern as step 4.

6. **Refactor `handshake_completes_once` test.** Remove the `#[serial_test::serial]` attribute. Remove the entire save/restore block for the three env vars and the teardown block at the bottom. Wrap the body with:
   ```rust
   temp_env::async_with_vars(
       [
           ("ANVILML_WORKER_MOCK", Some("1")),
           ("ANVILML_PING_INTERVAL_MS", Some("50")),
           ("ANVILML_PONG_TIMEOUT_MS", Some("150")),
       ],
       async { <existing test body> },
   )
   .await;
   ```

7. **Refactor `spawn_reaches_idle` test.** Same pattern as step 4.

8. **Verify.** Run `cargo clippy -p anvilml-worker --features mock-hardware -- -D warnings` and `cargo test -p anvilml-worker --features mock-hardware`. Then run `env -i HOME=$HOME PATH=$PATH cargo test -p anvilml-worker --features mock-hardware` to confirm ambient env isolation.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-worker/Cargo.toml` | Add `temp-env` dev-dep, remove `serial_test`, bump version `0.1.12 → 0.1.13` |
| Modify | `crates/anvilml-worker/src/managed.rs` | Refactor four test functions to use `temp_env::async_with_vars`; remove `#[serial_test::serial]` attributes and manual env-var teardown code |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `crates/anvilml-worker/src/managed.rs` | `spawn_ping_pong` | Worker spawns, responds to Ping→Pong, exits on Shutdown (env-scoped) |
| `crates/anvilml-worker/src/managed.rs` | `status_transitions` | Status flows Initializing → Idle → Dead under env isolation |
| `crates/anvilml-worker/src/managed.rs` | `handshake_completes_once` | Exactly one Ready event during spawn handshake (env-scoped) |
| `crates/anvilml-worker/src/managed.rs` | `spawn_reaches_idle` | spawn() reaches Idle without timing workarounds (env-scoped) |

## CI Impact

No CI changes required. The test suite already uses `--features mock-hardware`. Removing `serial_test` from dev-dependencies means tests can now run in parallel without serialisation. The ambient-env-clear test (`env -i`) remains the definitive isolation gate. No `.github/` workflow files are modified.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `temp_env::async_with_vars` requires `futures` crate (via `async_closure` feature) which may not be in the workspace | Low | Build failure if unresolved | `temp-env 0.3.x` declares `futures` as an optional dependency activated by the `async_closure` feature; Cargo will fetch it from crates.io automatically — no workspace addition needed |
| `handshake_completes_once` has complex save/restore logic that must be fully replaced | Medium | Logic error if teardown is incomplete | The test body inside `async_with_vars` runs with all three vars permanently set — the original save/restore and teardown blocks are entirely redundant and can be removed wholesale |
| Removing `serial_test` could expose hidden env-var contamination in other tests | Low | Test flakiness | Only four tests set `ANVILML_WORKER_MOCK`; the remaining two (`eof_sets_dead`, `keepalive_pings_and_kills_on_timeout`) don't. The ambient-env-clear test (`env -i`) will catch any remaining issues |
| Version bump changes Cargo.lock, potentially affecting other crates' build order | Low | None — workspace path deps don't pin versions | Cargo regenerates Cargo.lock on next build; no cascade needed |

## Acceptance Criteria

- [ ] `cargo clippy -p anvilml-worker --features mock-hardware -- -D warnings` exits 0
- [ ] `cargo test -p anvilml-worker --features mock-hardware` exits 0 with all 16 tests passing (4 spawning + 2 existing + 10 others)
- [ ] `env -i HOME=$HOME PATH=$PATH cargo test -p anvilml-worker --features mock-hardware` exits 0
- [ ] No `#[serial_test::serial]` attributes remain in `managed.rs`
- [ ] `serial_test` removed from `[dev-dependencies]` in `crates/anvilml-worker/Cargo.toml`
- [ ] `temp-env` present in `[dev-dependencies]` with `async_closure` feature
- [ ] Crate version bumped to `0.1.13` in `crates/anvilml-worker/Cargo.toml`
- [ ] No changes to test assertions or non-test code paths

## Dependency Notes

The task description references `temp_env::async_with_var` (singular), but the `temp-env 0.3.6` crate provides only `async_with_vars` (plural, behind the `async_closure` feature flag). There is no singular variant. The plan uses `async_with_vars` with a single-element array for tests that set one variable, and a three-element array for `handshake_completes_once`. This matches the crate's actual API shape verified via docs.rs source inspection.
