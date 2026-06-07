# Plan Report: P901-A1

| Field       | Value                                         |
|-------------|-----------------------------------------------|
| Task ID     | P901-A1                                       |
| Phase       | 901 — Worker Test Teardown Fix                |
| Description | anvilml-worker: env-var teardown in spawning tests to fix spawn_ping_pong failure |
| Depends on  | P11-C3                                        |
| Project     | anvilml                                       |
| Planned at  | 2026-06-07T14:15:00Z                          |
| Attempt     | 1                                             |

## Objective

Fix a test env-var leak in `crates/anvilml-worker/src/managed.rs` that causes `spawn_ping_pong` to fail consistently with "worker did not reach Ready state in time" when run after `handshake_completes_once` under `#[serial_test::serial]`. The root cause is missing unconditional teardown of mock environment variables at the end of two tests.

## Scope

### In Scope
- Add unconditional `std::env::remove_var` teardown to `handshake_completes_once` test (three vars: `ANVILML_WORKER_MOCK`, `ANVILML_PING_INTERVAL_MS`, `ANVILML_PONG_TIMEOUT_MS`)
- Add unconditional `std::env::remove_var` teardown to `spawn_reaches_idle` test (one var: `ANVILML_WORKER_MOCK`)
- No production code changes
- No new tests
- No new dependencies

### Out of Scope
- Changes to `status_transitions` or `spawn_ping_pong` (these call no `set_var` and need no changes)
- Any crate version bumps (no source file modifications outside the test module)
- CI workflow changes
- Cross-platform build changes

## Approach

1. **Open `crates/anvilml-worker/src/managed.rs`** — locate the `handshake_completes_once` test function (line ~830).
2. **Add teardown to `handshake_completes_once`** — after the existing restore block (lines 917–929), append three unconditional `remove_var` calls as the final statements of the test body:
   ```rust
   std::env::remove_var("ANVILML_WORKER_MOCK");
   std::env::remove_var("ANVILML_PING_INTERVAL_MS");
   std::env::remove_var("ANVILML_PONG_TIMEOUT_MS");
   ```
3. **Open `spawn_reaches_idle` test function** — locate at line ~1185.
4. **Add teardown to `spawn_reaches_idle`** — before the closing `}` of the function body, append:
   ```rust
   std::env::remove_var("ANVILML_WORKER_MOCK");
   ```

This is the identical pattern applied by P11-B1 to `anvilml-hardware`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-worker/src/managed.rs` | Add `remove_var` teardown in two test functions: `handshake_completes_once` and `spawn_reaches_idle` |

## Tests

<table>
<tr><th>Test File</th><th>Test Name</th><th>What It Verifies</th></tr>
<tr><td>crates/anvilml-worker/src/managed.rs</td><td>handshake_completes_once</td><td>After teardown, subsequent tests run without leaked env vars; exactly one Ready event received</td></tr>
<tr><td>crates/anvilml-worker/src/managed.rs</td><td>spawn_ping_pong</td><td>No longer fails with "worker did not reach Ready state in time" when run after handshake_completes_once under serial_test</td></tr>
<tr><td>crates/anvilml-worker/src/managed.rs</td><td>spawn_reaches_idle</td><td>After its own teardown, no ANVILML_WORKER_MOCK leak to following tests</td></tr>
</table>

No new test files are created or modified. The existing test suite serves as verification: all three affected tests must pass when run together under `--features mock-hardware`.

## CI Impact

No CI changes required. The fix is purely in test code (`#[cfg(test)]` module). The existing CI gate `cargo test --workspace --features mock-hardware` will exercise the fixed teardown on every commit. No new gates, no format/lint changes beyond what's already required.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `handshake_completes_once` already has restore logic (lines 917–929); adding unconditional `remove_var` after it is redundant but harmless | Low | None | The restore block sets vars back to originals or removes them; the subsequent `remove_var` ensures they are definitely gone. This matches P11-B1's pattern of unconditional teardown. |
| `spawn_reaches_idle` teardown could interfere if a test before it needs `ANVILML_WORKER_MOCK` set | Very Low | None | The `serial_test` ordering is: `handshake_completes_once` → `spawn_reaches_idle`. Only `spawn_reaches_idle` sets this var. Removing it afterward protects downstream tests. |
| Cargo check or test failure on Windows cross-target due to `std::env::remove_var` API differences | Very Low | Medium | `std::env::remove_var` is stable cross-platform Rust stdlib — no platform-specific behavior. Verified via existing P11-B1 usage in `anvilml-hardware`. |
| The fix doesn't address the root cause (the restore block in handshake_completes_once may not have been present when the bug was reported) | None | None | We verify by reading current file contents; the fix is applied to current state. If the restore logic was already added, the unconditional teardown adds defense-in-depth. |

## Acceptance Criteria

- [ ] `cat crates/anvilml-worker/src/managed.rs | grep -c 'remove_var'` returns ≥ 6 (3 in `handshake_completes_once` + 1 in `spawn_reaches_idle` + existing ones in other tests)
- [ ] `cargo test -p anvilml-worker --features mock-hardware` exits 0 with 0 failed
- [ ] `cargo check --target x86_64-pc-windows-gnu --features mock-hardware` exits 0
- [ ] No production code files modified (only `crates/anvilml-worker/src/managed.rs` test module)
- [ ] No new dependencies added to any `Cargo.toml`
