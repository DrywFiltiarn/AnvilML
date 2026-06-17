# Plan Report: P901-A3

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P901-A3                                     |
| Phase       | 901 — ManagedWorker Run-Loop and RespawnPolicy Retrofit |
| Description | anvilml-worker: respawn.rs fix should_respawn to honor last_crash/window_s instead of ignoring them |
| Depends on  | none (confined to anvilml-worker; no production callers exist) |
| Project     | anvilml                                     |
| Planned at  | 2026-06-17T13:35:00Z                        |
| Attempt     | 1                                           |

## Objective

Fix the `RespawnPolicy::should_respawn` method in `crates/anvilml-worker/src/respawn.rs` so that it actually honours the `last_crash` parameter and the `window_s` time window — resetting the crash counter when the window expires, rather than silently ignoring the parameter and always returning `true` below `max_attempts`. This corrects a defect introduced in Phase 10 (P10-A1) where `_last_crash` was renamed to underscore-prefixed form and the window-reset contract was left as caller-inferred behaviour, allowing the bug to ship unnoticed because the existing test suite was shaped around the broken logic.

## Scope

### In Scope
- Modify `crates/anvilml-worker/src/respawn.rs`: change `should_respawn` signature from `(&self, crash_count: u32, _last_crash: Instant) -> bool` to `(&self, crash_count: &mut u32, last_crash: Instant) -> bool`, and implement the window-reset logic inside the method.
- Modify `crates/anvilml-worker/tests/respawn_tests.rs`: update `test_should_respawn_window_reset` to assert that `crash_count` is mutated (reset to a low value post-increment), not just the boolean return value. Update existing test call sites (`test_should_respawn_max_attempts_exceeded`, `test_should_respawn_within_window`) to use the new mutable-reference signature.
- Bump `anvilml-worker` patch version from `0.1.9` to `0.1.10` in `crates/anvilml-worker/Cargo.toml`.
- Update `docs/TESTS.md` entries for the modified test.

### Out of Scope
- No production caller updates — no caller of `should_respawn` exists outside the test suite (the production caller is added in the renumbered P10-A3).
- No changes to `managed.rs`, `pool.rs`, or any other crate.
- No changes to `next_delay_ms` (exponential backoff logic is correct).
- No changes to `anvilml.toml` or `docs/ENVIRONMENT.md` (no config fields modified).
- No OpenAPI or config drift.

## Existing Codebase Assessment

The `RespawnPolicy` struct lives in `crates/anvilml-worker/src/respawn.rs` and is a pure Rust stdlib type — zero I/O, zero async, zero external crate dependencies. It is exported as `pub use respawn::RespawnPolicy` from `lib.rs` (line 22).

The current `should_respawn` method (line 91) takes `crash_count: u32` by value and `_last_crash: Instant` (underscore-prefixed, confirming it is unused). The implementation checks `crash_count >= self.max_attempts` and returns `false`; otherwise returns `true` unconditionally. The module-level doc comment (lines 8–11) says the caller is responsible for tracking `crash_count` and `last_crash` externally — this is the design ambiguity that let the defect ship.

The test file `crates/anvilml-worker/tests/respawn_tests.rs` has four tests:
1. `test_should_respawn_max_attempts_exceeded` — asserts `false` when count equals max.
2. `test_should_respawn_within_window` — asserts `true` when count below max within window.
3. `test_should_respawn_window_reset` — asserts `true` when window expired; but only checks the boolean return, which is identical for both the buggy and correct implementations.
4. `test_next_delay_ms_exponential_backoff_and_cap` — tests the backoff logic (unchanged).

The established patterns in this crate:
- Tests live in `crates/{name}/tests/` as separate test crate files (per ENVIRONMENT.md §11.1).
- Doc comments use `///` on all `pub` items.
- Inline comments explain non-obvious decision points.
- The `serial_test` crate (dev-dependency, version 3.5) is available for serialised tests if needed.

No gap between the design doc and current source beyond the defect itself: ANVILML_DESIGN.md §18.4 correctly specifies the time-windowed crash budget, but the implementation ignores it.

## Resolved Dependencies

No new external crates are introduced or referenced by this task. The only types used are from the Rust standard library (`std::time::Instant`, `std::time::Duration`). The existing dev-dependency `serial_test = "3.5"` is not used by any test in this task.

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| crate  | (none)  | n/a             | n/a            | n/a                    |

No MCP lookup needed — only stdlib types are used.

## Approach

1. **Bump `anvilml-worker` patch version** in `crates/anvilml-worker/Cargo.toml`: change `version = "0.1.9"` to `version = "0.1.10"`. This follows the ENVIRONMENT.md §12 procedure.

2. **Modify `should_respawn` signature and implementation** in `crates/anvilml-worker/src/respawn.rs`:
   - Change the method signature from `pub fn should_respawn(&self, crash_count: u32, _last_crash: Instant) -> bool` to `pub fn should_respawn(&self, crash_count: &mut u32, last_crash: Instant) -> bool`.
   - Update the method body:
     a. Compute `Duration` since last crash: `let elapsed = last_crash.elapsed();`
     b. **Window-reset decision**: if `elapsed >= Duration::from_secs(self.window_s as u64)`, set `*crash_count = 0`. (Rationale: the function now owns the reset contract, eliminating the caller-discretion ambiguity that let the original defect ship. The doc comment and module-level docs are updated to reflect this.)
     c. **Max-attempt guard**: if `*crash_count >= self.max_attempts`, return `false`.
     d. **Increment and allow**: increment `*crash_count += 1`, then return `true`.
   - Update the method's doc comment: remove the "pure decision function" language and the statement that the caller is responsible for tracking/resetting; replace with documentation that the function performs the window-reset internally and takes `crash_count` by mutable reference.
   - Update the module-level doc comment (lines 8–11): remove the statement that the caller tracks `crash_count` and `last_crash` externally.

3. **Update test call sites** in `crates/anvilml-worker/tests/respawn_tests.rs`:
   - `test_should_respawn_max_attempts_exceeded`: change `policy.should_respawn(3, Instant::now())` to use a mutable variable: `let mut count = 3; assert!(!policy.should_respawn(&mut count, Instant::now()));`
   - `test_should_respawn_within_window`: change `policy.should_respawn(2, last_crash)` to: `let mut count = 2; assert!(policy.should_respawn(&mut count, last_crash));`
   - `test_should_respawn_window_reset`: rewrite the assertion to capture both the boolean return AND the mutated crash count. Use: `let mut count = 4; let result = policy.should_respawn(&mut count, last_crash); assert!(result); assert_eq!(count, 1);` (count was reset to 0 by window expiry, then incremented to 1 by the "allow" step). This assertion on `count` is what the old buggy implementation would fail — it never mutated the count, so the old signature didn't even accept a mutable reference.
   - Update the doc comment for `test_should_respawn_window_reset` to explain that the test now asserts both the return value and the counter mutation.

4. **Update `docs/TESTS.md`** entries for the three modified tests (lines ~1803–1828), reflecting the new signature with mutable reference and the updated `test_should_respawn_window_reset` assertion.

5. **Verify** with `cargo test -p anvilml-worker --features mock-hardware -- respawn` — expects ≥ 4 tests, all passing.

## Public API Surface

| Item | Type | Path | Before | After |
|------|------|------|--------|-------|
| `should_respawn` | `pub fn` | `anvilml_worker::RespawnPolicy::should_respawn` | `pub fn should_respawn(&self, crash_count: u32, _last_crash: Instant) -> bool` | `pub fn should_respawn(&self, crash_count: &mut u32, last_crash: Instant) -> bool` |

No new `pub` items are introduced. The only public API change is the signature of the existing `should_respawn` method. Since no production caller exists yet, this is a contained change.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `crates/anvilml-worker/src/respawn.rs` | Change `should_respawn` signature to take `crash_count: &mut u32`; implement window-reset logic; update doc comments |
| MODIFY | `crates/anvilml-worker/tests/respawn_tests.rs` | Update all three `should_respawn` test call sites to use mutable reference; rewrite `test_should_respawn_window_reset` to assert counter mutation |
| MODIFY | `crates/anvilml-worker/Cargo.toml` | Bump patch version `0.1.9` → `0.1.10` |
| MODIFY | `docs/TESTS.md` | Update test catalogue entries for the three modified tests |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-worker/tests/respawn_tests.rs` | `test_should_respawn_max_attempts_exceeded` | `should_respawn` returns `false` when `crash_count >= max_attempts` | `max_attempts = 3` | `crash_count = 3` (mutable ref), `last_crash = Instant::now()` | `false`; `crash_count` unchanged at 3 | `cargo test -p anvilml-worker --features mock-hardware --test respawn_tests test_should_respawn_max_attempts_exceeded` exits 0 |
| `crates/anvilml-worker/tests/respawn_tests.rs` | `test_should_respawn_within_window` | `should_respawn` returns `true` when `crash_count < max_attempts` and window has not expired | `max_attempts = 5`, `window_s = 60` | `crash_count = 2` (mutable ref), `last_crash = now - 30s` | `true`; `crash_count` incremented to 3 | `cargo test -p anvilml-worker --features mock-hardware --test respawn_tests test_should_respawn_within_window` exits 0 |
| `crates/anvilml-worker/tests/respawn_tests.rs` | `test_should_respawn_window_reset` | `should_respawn` resets `crash_count` to 0 when window expired, then increments to 1 and returns `true` | `max_attempts = 5`, `window_s = 10` | `crash_count = 4` (mutable ref), `last_crash = now - 15s` | `true`; `crash_count` == 1 (reset to 0, then incremented) | `cargo test -p anvilml-worker --features mock-hardware --test respawn_tests test_should_respawn_window_reset` exits 0 |
| `crates/anvilml-worker/tests/respawn_tests.rs` | `test_next_delay_ms_exponential_backoff_and_cap` | `next_delay_ms` computes exponential backoff and caps at 30,000 ms | `delay_ms = 1000` | attempts 0–5, 10 | values as documented in test body | `cargo test -p anvilml-worker --features mock-hardware --test respawn_tests test_next_delay_ms_exponential_backoff_and_cap` exits 0 |

## CI Impact

No CI changes required. The task modifies only existing test files within `anvilml-worker` — the CI job `rust-linux` and `rust-windows` already run `cargo test --workspace --features mock-hardware`, which includes `anvilml-worker` tests. No new test file, gate, or file type is introduced.

## Platform Considerations

None identified. The `std::time::Instant` and `std::time::Duration` types are platform-neutral in their semantics. The `elapsed()` method and `Duration::from_secs()` are stable across all platforms. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are needed. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `test_should_respawn_within_window` semantics change: the old test only asserted `true` for a count of 2 within a 60s window. With the new logic, the call increments count to 3 and returns `true`. If the test later calls `should_respawn` again with count 3 (e.g., in a loop), it would still return true since 3 < 5. The single-call test remains valid but now also mutates state. A future caller relying on the old "pass-by-value, no mutation" assumption would break. | Low | Medium | The task scope explicitly states no production caller exists. The mutable reference signature makes the mutation observable at the call site — any code that needs the old value can capture it before the call. The doc comment documents the mutation. |
| `test_should_respawn_window_reset` assertion on `count == 1` could fail if the window-reset logic is implemented incorrectly (e.g., if `elapsed()` comparison uses wrong units). | Low | Medium | The implementation uses `Duration::from_secs(self.window_s as u64)` which matches the `window_s: u32` field type. The test uses `Instant::now() - Duration::from_secs(15)` with `window_s = 10`, so the gap is 5 seconds — well beyond any floating-point or unit conversion edge case. |
| `docs/TESTS.md` entries become stale if not updated in the same task. FORGE_AGENT_RULES §5.10 requires test catalogue sync. | Low | Medium | The plan includes updating `docs/TESTS.md` as step 4 of the Approach. The ACT agent will do this in the same task. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-worker --features mock-hardware -- respawn` exits 0 with ≥ 4 tests
- [ ] `cargo clippy -p anvilml-worker --features mock-hardware -- -D warnings` exits 0
- [ ] `cargo fmt --all -- --check` exits 0
- [ ] `grep '^pub fn should_respawn' crates/anvilml-worker/src/respawn.rs` matches `&mut u32` (signature verified)
- [ ] `grep '_last_crash' crates/anvilml-worker/src/respawn.rs` returns no results (underscore-prefixed parameter eliminated)
- [ ] `cargo test -p anvilml-worker --features mock-hardware --test respawn_tests test_should_respawn_window_reset` exits 0 (window reset test specifically)
- [ ] `cargo test -p anvilml-worker --features mock-hardware` exits 0 (full crate test suite)
