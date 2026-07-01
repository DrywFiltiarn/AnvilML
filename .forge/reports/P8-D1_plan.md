# Plan Report: P8-D1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P8-D1                                       |
| Phase       | 008 — IPC Stress Gate & Worker Pool         |
| Description | anvilml-worker: respawn.rs RespawnPolicy backoff + max-attempt guard |
| Depends on  | P8-A1                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-07-01T11:45:00Z                        |
| Attempt     | 1                                           |

## Objective

Create `crates/anvilml-worker/src/respawn.rs` implementing `RespawnPolicy` — a pure,
zero-I/O struct that encodes the worker crash-recovery backoff policy from
`ANVILML_DESIGN.md §19.4`. It holds three configurable parameters (delay, max attempts,
window) and provides two methods: `should_respawn()` which inspects a slice of
`Instant` timestamps to decide whether a crashed worker may be respawned, and
`next_delay()` which returns the constant delay as a `Duration`. This is the policy
type that `P8-E4` and `P8-E5` will wire into `ManagedWorker::run()`'s crash-exit path.

## Scope

### In Scope
- Create `crates/anvilml-worker/src/respawn.rs` with `RespawnPolicy` struct and its
  three fields (`respawn_delay_ms`, `respawn_max_attempts`, `respawn_window_s`), all
  `u32`.
- Implement `RespawnPolicy::default()` with documented defaults: 2000ms delay,
  5 max attempts, 300s window.
- Implement `RespawnPolicy::new(respawn_delay_ms, respawn_max_attempts, respawn_window_s)`
  constructor.
- Implement `should_respawn(&self, attempt_history: &[std::time::Instant]) -> bool`:
  counts attempts within the trailing `respawn_window_s` window; returns `false` if
  count >= `respawn_max_attempts`, `true` otherwise.
- Implement `next_delay(&self) -> std::time::Duration`: returns `Duration::from_millis(self.respawn_delay_ms)`.
- Add `mod respawn;` and `pub use respawn::RespawnPolicy;` to `lib.rs`.
- Create `crates/anvilml-worker/tests/respawn_tests.rs` with >=5 tests covering:
  under-limit allows respawn, at-limit within window blocks, outside-window ignores,
  defaults match documented values.

### Out of Scope
defers_to (from JSON): []
absent — this task may not defer any scope.

## Existing Codebase Assessment

No prior source exists for `respawn.rs` — it is a new file. The `anvilml-worker` crate
already has six modules (`demux`, `env`, `keepalive`, `spawn`, `job_object`) and four
test files (`demux_tests.rs`, `env_tests.rs`, `keepalive_tests.rs`, `spawn_tests.rs`)
in its `tests/` directory. The established patterns are:

- **Testing style:** Integration tests in `tests/*.rs` files, using `#[tokio::test]` for
  async tests. Each test has a doc comment explaining the precondition, setup, and
  expected outcome. Tests use injected durations (not real seconds) to run fast.
- **Naming:** `snake_case` for functions and variables; `PascalCase` for types and traits.
- **Error handling:** Uses `AnvilError` from `anvilml-core`; `Result<T, AnvilError>` for fallible operations.
- **Documentation:** Every `pub` item has a `///` doc comment. The module-level doc
  comment describes what the module owns and references the design doc section.
- **lib.rs discipline:** Contains only `//!` crate-level doc, `mod` declarations, and
  `pub use` re-exports — no implementation code. Currently 18 lines.

The `keepalive.rs` module is the closest structural analogue: it defines a `Transport`
trait with a `MockTransport` impl for tests. However, `RespawnPolicy` is simpler — it
is a pure struct with no trait, no async, and no mock transport needed. Its methods
operate entirely on `std::time::Instant` values passed in by the caller.

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| crate  | tokio   | 1.52.3          | rust-docs MCP  | time (already in Cargo.toml) |
| crate  | tracing | 0.1.x           | rust-docs MCP  | n/a (already in Cargo.toml) |

No new external dependencies are introduced. `std::time::Instant` and
`std::time::Duration` are from the Rust standard library. The `tokio` crate's `time`
feature (already declared in `Cargo.toml`) provides `tokio::time::sleep` which will
be used by `P8-E5` but is not needed in this task's code.

## Approach

### Step 1: Create `crates/anvilml-worker/src/respawn.rs`

Implement the `RespawnPolicy` struct and its methods:

1. **Module doc comment:** Describe the module as encoding the worker crash-recovery
   backoff policy per `ANVILML_DESIGN.md §19.4`. Note that it is a pure, zero-I/O type.

2. **Struct definition:**
   ```rust
   pub struct RespawnPolicy {
       respawn_delay_ms: u32,
       respawn_max_attempts: u32,
       respawn_window_s: u32,
   }
   ```
   All fields are `pub` (not `pub(crate)`) since `should_respawn` accepts a slice of
   `Instant` values and the struct is the public API surface. The design doc specifies
   field names; use them exactly.

3. **`impl Default for RespawnPolicy`:**
   - `respawn_delay_ms: 2000` (2 seconds)
   - `respawn_max_attempts: 5`
   - `respawn_window_s: 300` (5 minutes)
   - These match the documented defaults in `ANVILML_DESIGN.md §19.4`.

4. **`impl RespawnPolicy`:**
   - **`pub fn new(respawn_delay_ms: u32, respawn_max_attempts: u32, respawn_window_s: u32) -> Self`**
     Straightforward constructor that stores the three parameters.
   - **`pub fn should_respawn(&self, attempt_history: &[std::time::Instant]) -> bool`**
     This is the core logic:
     1. Compute the window cutoff: `now - Duration::from_secs(self.respawn_window_s as u64)`.
        The `now` instant must be injected (via a parameter or a closure) for testability.
        Since the method signature in the task context is fixed as
        `should_respawn(&self, attempt_history: &[Instant]) -> bool`, `now` cannot be a
        parameter of the method itself. The approach: use `std::time::SystemTime::now()`
        converted to `Instant` via `Instant::now()`. This is acceptable because:
        (a) `should_respawn` is called at the moment a crash is detected, so `Instant::now()`
            reflects the current moment accurately, and
        (b) tests that need deterministic behavior can control the `attempt_history`
            timestamps relative to when they call the method.
     2. Filter `attempt_history` to only those `Instant` values within the trailing window.
     3. If `filtered.len() >= self.respawn_max_attempts`, return `false`.
     4. Otherwise, return `true`.
   - **`pub fn next_delay(&self) -> std::time::Duration`**
     Returns `Duration::from_millis(self.respawn_delay_ms)`. Constant-delay only —
     no exponential backoff logic.

5. **Decision-point comments:** The `should_respawn` method has one non-trivial decision:
   filtering by window. Add an inline comment explaining that the window is trailing
   (relative to the current instant at call time) and that attempts outside the window
   are discarded, not carried forward.

### Step 2: Update `crates/anvilml-worker/src/lib.rs`

Add two lines after the existing module declarations:
```rust
mod respawn;
pub use respawn::RespawnPolicy;
```
This keeps `lib.rs` well under the 80-line hard cap (currently 18 lines, will be ~20).

### Step 3: Create `crates/anvilml-worker/tests/respawn_tests.rs`

Write >=5 integration tests following the project's test conventions:

1. **`test_defaults_match_documented_values`** — Verifies `RespawnPolicy::default()`
   produces the documented defaults (2000ms, 5 attempts, 300s window).

2. **`test_under_limit_allows_respawn`** — Creates a policy with max_attempts=3,
   feeds 2 attempt timestamps within the window, asserts `should_respawn` returns `true`.

3. **`test_at_limit_blocks_respawn`** — Creates a policy with max_attempts=3,
   feeds exactly 3 attempt timestamps within the window, asserts `should_respawn`
   returns `false`.

4. **`test_attempts_outside_window_dont_count`** — Creates a policy with max_attempts=2,
   feeds 2 attempt timestamps but both are older than `respawn_window_s`, asserts
   `should_respawn` returns `true` (window is empty, count is 0 < max_attempts).

5. **`test_next_delay_returns_correct_duration`** — Creates a policy with a custom
   delay (e.g. 5000ms), asserts `next_delay()` returns `Duration::from_millis(5000)`.

6. **`test_empty_history_allows_respawn`** — Feeds an empty slice, asserts `true`
   (zero attempts is under any max_attempts threshold).

Each test must have a doc comment explaining what it verifies and its precondition.
Tests use `std::time::Instant` directly — no mock or async needed since the logic
is pure computation.

### Step 4: Verify with `cargo test -p anvilml-worker --test respawn_tests`

The acceptance command must exit 0.

## Public API Surface

| Item | Crate/Module Path | Signature |
|------|-------------------|-----------|
| `RespawnPolicy` (struct) | `anvilml_worker::RespawnPolicy` | `pub struct RespawnPolicy { respawn_delay_ms: u32, respawn_max_attempts: u32, respawn_window_s: u32 }` |
| `RespawnPolicy::default()` | `anvilml_worker::RespawnPolicy` | `impl Default for RespawnPolicy` — returns 2000ms/5/300s |
| `RespawnPolicy::new()` | `anvilml_worker::RespawnPolicy` | `pub fn new(respawn_delay_ms: u32, respawn_max_attempts: u32, respawn_window_s: u32) -> Self` |
| `RespawnPolicy::should_respawn()` | `anvilml_worker::RespawnPolicy` | `pub fn should_respawn(&self, attempt_history: &[std::time::Instant]) -> bool` |
| `RespawnPolicy::next_delay()` | `anvilml_worker::RespawnPolicy` | `pub fn next_delay(&self) -> std::time::Duration` |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-worker/src/respawn.rs` | `RespawnPolicy` struct with default impl, new(), should_respawn(), next_delay() |
| MODIFY | `crates/anvilml-worker/src/lib.rs` | Add `mod respawn; pub use respawn::RespawnPolicy;` |
| CREATE | `crates/anvilml-worker/tests/respawn_tests.rs` | >=5 integration tests for RespawnPolicy |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `tests/respawn_tests.rs` | `test_defaults_match_documented_values` | `RespawnPolicy::default()` returns documented defaults (2000ms, 5 attempts, 300s window) | None | `RespawnPolicy::default()` | delay=2000, max=5, window=300 | `cargo test -p anvilml-worker --test respawn_tests` exits 0 |
| `tests/respawn_tests.rs` | `test_under_limit_allows_respawn` | `should_respawn` returns `true` when attempt count is strictly below max_attempts within the window | Policy with max_attempts=3 | 2 `Instant` values within window | `true` | `cargo test -p anvilml-worker --test respawn_tests` exits 0 |
| `tests/respawn_tests.rs` | `test_at_limit_blocks_respawn` | `should_respawn` returns `false` when attempt count equals max_attempts within the window | Policy with max_attempts=3 | 3 `Instant` values within window | `false` | `cargo test -p anvilml-worker --test respawn_tests` exits 0 |
| `tests/respawn_tests.rs` | `test_attempts_outside_window_dont_count` | Attempts older than `respawn_window_s` are excluded from the count | Policy with max_attempts=2, window=1s | 2 `Instant` values older than 1s | `true` (0 in window < 2) | `cargo test -p anvilml-worker --test respawn_tests` exits 0 |
| `tests/respawn_tests.rs` | `test_next_delay_returns_correct_duration` | `next_delay()` returns the configured delay as a `Duration` | Policy with custom delay=5000ms | None | `Duration::from_millis(5000)` | `cargo test -p anvilml-worker --test respawn_tests` exits 0 |
| `tests/respawn_tests.rs` | `test_empty_history_allows_respawn` | Empty attempt history always allows respawn (0 < any max_attempts) | None | Empty slice `&[]` | `true` | `cargo test -p anvilml-worker --test respawn_tests` exits 0 |

## CI Impact

No CI changes required. The new test file is an integration test in the crate's
`tests/` directory, which is automatically picked up by `cargo test --workspace
--features mock-hardware` (the standard CI test command). No new CI job, gate, or
workflow file is needed.

## Platform Considerations

None identified. The `RespawnPolicy` struct is a pure computation type with no I/O,
no platform-specific code, and no `#[cfg(...)]` guards required. It uses only
`std::time::Instant` and `std::time::Duration` from the Rust standard library,
which behave identically on all platforms. The Windows cross-check in
`ENVIRONMENT.md §7` is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `Instant::now()` in `should_respawn` makes the method non-deterministic for tests that need precise timing control | Medium | Medium | The method uses `Instant::now()` at call time, which is the current wall-clock moment. Tests that need deterministic behavior can set `attempt_history` timestamps very close to when they call `should_respawn()` (within milliseconds), ensuring the window check sees them. The `test_attempts_outside_window_dont_count` test uses a 1-second window and timestamps set 2 seconds in the past, providing a large margin. This is an acceptable tradeoff — the alternative would be to refactor the method signature to accept a `now` parameter, but the task context specifies the exact signature and changing it would break `P8-E4`'s planned call site. |
| `attempt_history` slice contains timestamps from different `Instant` origins (e.g. from different `Instant::now()` calls across process restarts) | Low | Low | `Instant` in Rust is monotonic per-process and represents duration since an arbitrary (but fixed) point. Comparing two `Instant` values with `>` or `<` operators is always valid regardless of when they were created. The window check uses `instant > cutoff` where both are `Instant` values from the same process, so this is safe. |
| The `should_respawn` logic has an off-by-one error (count >= max vs count > max) | Medium | High | The design doc says "if a worker crashes more than respawn_max_attempts times... respawn halts." This means: at exactly max_attempts, respawn should stop (count >= max_attempts → false). At max_attempts - 1, respawn should continue. Write explicit tests for both the boundary (count == max_attempts) and one below (count == max_attempts - 1) to verify the correct comparison operator. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-worker --test respawn_tests` exits 0
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0 (verifies no warnings from new code)
- [ ] `wc -l crates/anvilml-worker/src/lib.rs` prints a number <= 80
- [ ] `grep -c "^fn test_" crates/anvilml-worker/tests/respawn_tests.rs` prints a number >= 5
