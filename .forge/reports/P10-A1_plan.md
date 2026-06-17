# Plan Report: P10-A1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P10-A1                                      |
| Phase       | 010 — Worker Crash Recovery                 |
| Description | anvilml-worker: respawn.rs RespawnPolicy with backoff and max-attempt guard |
| Depends on  | none                                        |
| Project     | anvilml                                     |
| Planned at  | 2026-06-17T00:00:00Z                        |
| Attempt     | 1                                           |

## Objective

Implement the full `RespawnPolicy` logic in `crates/anvilml-worker/src/respawn.rs`: replace the stub `should_respawn` and `next_delay_ms` methods with production-grade logic that enforces a max-attempt cap, resets crash counts when the time window expires, and applies exponential backoff capped at 30 seconds. This type is pure Rust stdlib — no I/O, no async, no external crates. It is consumed by `ManagedWorker` (P10-A2) to decide when and how long to wait before respawning a dead worker.

## Scope

### In Scope
- Implement `should_respawn(&self, crash_count: u32, last_crash: Instant) -> bool` with max-attempt guard and window reset logic
- Implement `next_delay_ms(&self, attempt: u32) -> u64` with exponential backoff capped at 30,000 ms
- Make all three struct fields (`delay_ms`, `max_attempts`, `window_s`) public
- Update the `///` doc comment on `RespawnPolicy` to remove the "stub" note
- Create `tests/respawn_tests.rs` with ≥ 4 unit tests
- Bump `anvilml-worker` patch version from 0.1.6 to 0.1.7

### Out of Scope
- P10-A2 crash detection and automatic respawn in `managed.rs`
- The HTTP restart endpoint (P10-B1)
- Any changes to `lib.rs` (the `pub mod respawn` and `pub use` are already present)
- Logging additions (those belong in P10-A2 where `RespawnPolicy` is called)

## Existing Codebase Assessment

The `respawn.rs` file already exists at `crates/anvilml-worker/src/respawn.rs` with a stub implementation. The struct `RespawnPolicy` is defined with three private fields (`delay_ms`, `max_attempts`, `window_s`) and both public methods return constant/stub values. The `lib.rs` already declares `pub mod respawn` and re-exports `pub use respawn::RespawnPolicy`, so no module-level changes are needed.

The existing test files in `crates/anvilml-worker/tests/` follow the project convention: separate test crate files (not inline `#[cfg(test)]`), using `use anvilml_worker::...` imports. The `managed_tests.rs` file demonstrates the pattern for importing from the crate root and using `#[tokio::test]` for async tests. However, `RespawnPolicy` methods are synchronous pure functions, so no `#[tokio::test]` is needed — plain `fn` tests suffice.

The `managed.rs` file already imports `use crate::respawn::RespawnPolicy` and stores `respawn_policy: RespawnPolicy` on the `ManagedWorker` struct, with `#[allow(dead_code)]` annotations noting that `child` and `respawn_policy` are for future respawn logic (P10-A1).

No external crates are introduced by this task. The implementation uses only `std::time::Instant`, which is already imported.

## Resolved Dependencies

None. This task introduces no new external crates. The implementation uses only `std::time::Instant` from the Rust standard library.

| Type   | Name | Version verified | MCP source | Feature flags confirmed |
|--------|------|-----------------|------------|------------------------|
| (none) | —    | —               | —          | —                      |

## Approach

1. **Make struct fields public** in `RespawnPolicy`. The current fields are private (`delay_ms: u64`, `max_attempts: u32`, `window_s: u32`). Change to `pub` so consumers (like `ManagedWorker` in P10-A2) can inspect policy values. Add a `#[non_exhaustive]` attribute is not needed since all fields are fixed.

2. **Implement `should_respawn`**. The method takes `crash_count: u32` and `last_crash: Instant`. The logic:
   - If `crash_count >= max_attempts`, return `false` (max attempts exceeded).
   - Otherwise, compute the elapsed time since `last_crash` using `Instant::now()`. If `elapsed >= Duration::from_secs(window_s)`, the window has expired — reset the count conceptually (return `true`, the caller resets crash_count to 0 before the next call). If the window has not expired, return `true`.
   - Rationale: The method is a pure decision function. It does not mutate state; the caller tracks `crash_count` and `last_crash` externally and passes the current values.

3. **Implement `next_delay_ms`**. The method takes `attempt: u32` and returns `u64`. The logic:
   - Compute `delay = delay_ms * 2^attempt` (exponential backoff). Use `checked_mul` or `saturating_mul` to avoid overflow for large attempt values.
   - Cap the result at 30,000 ms (30 seconds). Use `min(30_000)` after computing the exponential value.
   - Rationale: Exponential backoff with a fixed cap is the standard pattern for crash recovery — it prevents rapid re-spawn loops while still responding quickly to transient failures.

4. **Update the module-level and struct-level doc comments**. Remove the "stub" references from `respawn.rs`. The module doc comment currently says "Stub implementation" and "Full exponential backoff logic is deferred to P10-A1." These must be updated to reflect the completed implementation.

5. **Create `tests/respawn_tests.rs`** with four tests:
   - `test_should_respawn_max_attempts_exceeded`: create a policy with `max_attempts = 3`, call `should_respawn(3, Instant::now())`, expect `false`.
   - `test_should_respawn_within_window`: create a policy with `max_attempts = 5, window_s = 60`, call `should_respawn(2, Instant::now() - Duration::from_secs(30))`, expect `true`.
   - `test_should_respawn_window_reset`: create a policy with `max_attempts = 5, window_s = 10`, call `should_respawn(4, Instant::now() - Duration::from_secs(15))`, expect `true` (window expired, count resets).
   - `test_next_delay_ms_exponential_backoff_and_cap`: create a policy with `delay_ms = 1000`, verify the sequence: attempt 0 → 1000, attempt 1 → 2000, attempt 2 → 4000, attempt 3 → 8000, attempt 4 → 16000, attempt 5 → 30000 (capped), attempt 10 → 30000 (capped).

6. **Bump `anvilml-worker` version** from `0.1.6` to `0.1.7` in `crates/anvilml-worker/Cargo.toml`.

## Public API Surface

```rust
// Module: anvilml_worker::respawn

/// Respawn policy for a managed worker subprocess.
///
/// Controls how and when a dead worker subprocess should be respawned. The policy
/// tracks the maximum number of respawn attempts, the delay between attempts,
/// and the time window within which attempts are counted toward the maximum.
///
/// # Default values
///
/// | Field        | Default | Description                                  |
/// |--------------|---------|----------------------------------------------|
/// | `delay_ms`   | 2000    | Base delay in milliseconds between attempts. |
/// | `max_attempts` | 5     | Maximum number of respawn attempts.          |
/// | `window_s`   | 300     | Time window in seconds for attempt counting. |
#[derive(Debug, Clone)]
pub struct RespawnPolicy {
    pub delay_ms: u64,
    pub max_attempts: u32,
    pub window_s: u32,
}

impl RespawnPolicy {
    /// Determine whether the worker should be respawned.
    ///
    /// Returns `false` if `crash_count >= max_attempts` (maximum respawn
    /// attempts exceeded). Otherwise returns `true`.
    ///
    /// The caller is responsible for resetting `crash_count` to zero when
    /// the window expires (i.e., when `last_crash` is older than
    /// `window_s` seconds ago). This method does not mutate state.
    ///
    /// # Arguments
    ///
    /// * `crash_count` — The number of crashes that have occurred within
    ///   the current time window.
    /// * `last_crash` — The `Instant` when the most recent crash occurred.
    ///
    /// # Returns
    ///
    /// `true` if the worker should be respawned, `false` if max attempts
    /// have been reached.
    pub fn should_respawn(&self, crash_count: u32, last_crash: Instant) -> bool;

    /// Calculate the delay before the next respawn attempt using exponential backoff.
    ///
    /// Computes `delay_ms * 2^attempt`, capped at 30,000 ms (30 seconds).
    /// Uses saturating arithmetic to prevent overflow for very large attempt values.
    ///
    /// # Arguments
    ///
    /// * `attempt` — The zero-based index of the respawn attempt.
    ///
    /// # Returns
    ///
    /// The delay in milliseconds, between `delay_ms` and 30,000 ms inclusive.
    pub fn next_delay_ms(&self, attempt: u32) -> u64;
}

impl Default for RespawnPolicy {
    fn default() -> Self;
}
```

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-worker/src/respawn.rs` | Implement full `should_respawn` and `next_delay_ms`; make fields pub; update docs |
| Modify | `crates/anvilml-worker/Cargo.toml` | Bump patch version 0.1.6 → 0.1.7 |
| Create | `crates/anvilml-worker/tests/respawn_tests.rs` | Unit tests for RespawnPolicy (≥ 4 tests) |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-worker/tests/respawn_tests.rs` | `test_should_respawn_max_attempts_exceeded` | `should_respawn` returns `false` when crash_count equals max_attempts | Policy with `max_attempts = 3` | `crash_count = 3`, `last_crash = Instant::now()` | `false` | `cargo test -p anvilml-worker --features mock-hardware -- respawn` exits 0 |
| `crates/anvilml-worker/tests/respawn_tests.rs` | `test_should_respawn_within_window` | `should_respawn` returns `true` when within window and under max | Policy with `max_attempts = 5, window_s = 60` | `crash_count = 2`, `last_crash = Instant::now() - Duration::from_secs(30)` | `true` | same command |
| `crates/anvilml-worker/tests/respawn_tests.rs` | `test_should_respawn_window_reset` | `should_respawn` returns `true` when window expired (count resets) | Policy with `max_attempts = 5, window_s = 10` | `crash_count = 4`, `last_crash = Instant::now() - Duration::from_secs(15)` | `true` | same command |
| `crates/anvilml-worker/tests/respawn_tests.rs` | `test_next_delay_ms_exponential_backoff_and_cap` | Exponential backoff sequence and 30s cap | Policy with `delay_ms = 1000` | Attempts 0-5 and 10 | `[1000, 2000, 4000, 8000, 16000, 30000, 30000]` | same command |

## CI Impact

No CI changes required. The test module `respawn_tests.rs` follows the existing convention (separate test crate file in `crates/anvilml-worker/tests/`) and will be picked up automatically by `cargo test --workspace --features mock-hardware`. No new file types, gates, or test runners are introduced.

## Platform Considerations

None identified. The `std::time::Instant` API is cross-platform with identical semantics on Linux and Windows. The `checked_mul` / `saturating_mul` approach prevents overflow on both platforms. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are required. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `should_respawn` semantics ambiguity: the task context says "resets count if last_crash is older than window_s" but the method is pure (no mutation). If the ACT agent tries to mutate state, it will break the API contract. | Medium | High | The plan explicitly states the caller resets crash_count externally. The doc comment clarifies this. The test `test_should_respawn_window_reset` verifies the caller-side pattern: if window expired, caller resets count to 0 before the next call. |
| Exponential backoff overflow: `delay_ms * 2^attempt` overflows u64 for very large attempt values (e.g., attempt ≥ 64 with delay_ms = 1). | Low | High | Use `saturating_mul(2)` in a loop or `checked_pow` with saturating fallback. The test verifies attempt 10 is capped at 30,000, which exercises the overflow path since `1000 * 2^10 = 1_048_576` far exceeds the cap. |
| Test filter mismatch: the acceptance command uses `-- respawn` to filter tests. If test names don't contain "respawn", they won't be found. | Low | Medium | All four test names contain "respawn" (`test_should_respawn_*` and `test_next_delay_ms_*`). The filter is confirmed in the test table. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-worker --features mock-hardware -- respawn` exits 0 with ≥ 4 tests
- [ ] `cargo clippy --package anvilml-worker --features mock-hardware -- -D warnings` exits 0
- [ ] `cargo fmt --all -- --check` exits 0
- [ ] `head -1 crates/anvilml-worker/src/respawn.rs` prints `//! Respawn policy for managed worker subprocesses.`
- [ ] `grep '^pub ' crates/anvilml-worker/src/respawn.rs | wc -l` returns 3 (three pub fields)
- [ ] `grep '^version' crates/anvilml-worker/Cargo.toml` returns `version = "0.1.7"`
- [ ] `cargo check --workspace --features mock-hardware` exits 0
- [ ] `cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu` exits 0
