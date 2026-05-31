# Plan Report: P1-A2

| Field       | Value |
|-------------|-------|
| Task ID     | P1-A2 |
| Phase       | 1 |
| Description | anvilml: backend binary crate with anvilml bin name and tokio main stub |
| Depends on  | P1-A1 |
| Project     | anvilml |
| Attempt     | 1 |

## Plan

I'll analyze the context and plan this task. Let me start by gathering all necessary information.


Task verified as P1-A2. Now let me read the `.clinerules` for the required report structure, and check the existing workspace Cargo.toml and backend directory state.


Let me check the current state of the existing stub files and verify tokio availability in the server crate.


I now have all the context needed. Let me also quickly verify the phase name from PHASES.md, then write the plan report.


All context gathered. Now I'll write the complete plan report and update the state file. Per .clinerules §3, this must be a single write with the exact required structure.


Now I'll write the plan report file:


I cannot write files directly in this environment. Let me present the complete plan report that should be written to `.forge/reports/P1-A2_plan.md`:

---

```markdown
# Plan Report: P1-A2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P1-A2                                       |
| Phase       | 001 — Walking Skeleton                      |
| Description | backend binary crate with anvilml bin name and tokio main stub |
| Depends on  | P1-A1                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-05-31T21:58:38Z                        |
| Attempt     | 1                                           |

## Objective

Create the `anvilml` binary crate under `backend/` with a tokio-based async main stub that prints the version string and exits cleanly. This establishes the runnable entry point for Phase 001; the binary will later be wired to bind an axum HTTP server in P1-A4. The workspace version is `"0.1.0"` (set in root `Cargo.toml` `[workspace.package]`), which `CARGO_PKG_VERSION` will resolve to at compile time.

## Scope

### In Scope
- Add `tokio` dependency (feature set `full`) to `backend/Cargo.toml`.
- Replace `backend/src/main.rs` stub (`fn main() {}`) with a `#[tokio::main] async fn main()` that:
  - Prints `AnvilML vX.Y.Z starting` where `V.Y.Z` comes from `env!("CARGO_PKG_VERSION")`.
  - Exits 0 (normal process termination).
- Verify `cargo build --release` produces `target/release/anvilml`.

### Out of Scope
- HTTP server wiring (deferred to P1-A3 and P1-A4).
- Configuration loading, CLI argument parsing, graceful shutdown (phase 2+).
- Any tests for the binary (a print-and-exit stub has no testable logic).
- CI workflow changes (deferred to P1-A5).
- Modifications to `anvilml-server` crate (its deps — axum, tower, tokio — are added in P1-A3).

## Approach

1. **Modify `backend/Cargo.toml`:** Add the `tokio` dependency under `[dependencies]`. Use version `"1"` with `features = ["full"]`. The existing `[[bin]] name = "anvilml"` entry and `anvilml-server` path dependency are already present from P1-A1; they remain unchanged.

   The resulting `[dependencies]` block will contain:
   ```toml
   anvilml-server = { path = "../crates/anvilml-server" }
   tokio = { version = "1", features = ["full"] }
   ```

2. **Modify `backend/src/main.rs`:** Replace the current stub (`fn main() {}`) with:
   ```rust
   #[tokio::main]
   async fn main() {
       println!("AnvilML v{} starting", env!("CARGO_PKG_VERSION"));
   }
   ```
   This leverages the `#[tokio::main]` proc-macro to create a Tokio runtime from synchronous main, runs the async block, and drops the runtime on function return (clean exit with code 0).

3. **Verify build:** Run `cargo build --release -p backend` in the workspace root. Confirm:
   - Exit code 0.
   - Binary exists at `target/release/anvilml`.
   - Running `./target/release/anvilml` prints a line matching `AnvilML v0.1.0 starting` and exits 0.

## Files Affected

| Action   | Path                        | Description                                              |
|----------|-----------------------------|----------------------------------------------------------|
| MODIFY   | backend/Cargo.toml          | Add `tokio = { version = "1", features = ["full"] }` dep |
| MODIFY   | backend/src/main.rs         | Replace stub with `#[tokio::main] async fn main()`       |

## Tests

None. The task produces a minimal print-and-exit stub; there is no logic to unit-test. Integration verification is done via `cargo build --release` and running the binary.

## CI Impact

No CI changes required. P1-A5 handles CI workflow creation. This task only touches two files within the already-established workspace (P1-A1).

## Risks and Mitigations

| Risk                              | Likelihood | Impact | Mitigation                                            |
|-----------------------------------|-----------|--------|-------------------------------------------------------|
| `tokio` version incompatibility   | Low       | Low    | Use `version = "1"` (semver-compatible 1.x); verified via build. |
| `anvilml-server` not yet built    | None      | None   | P1-A3 adds axum/tower/tokio to the server crate; for P1-A2 the server crate is a dependency but its own build succeeds independently (it's a stub with no external deps beyond core crates). |
| `CARGO_PKG_VERSION` resolves incorrectly | None | None   | The workspace `[workspace.package] version = "0.1.0"` propagates to all members; verified at build time. |

## Acceptance Criteria

- [ ] `backend/Cargo.toml` contains a `tokio` dependency with `features = ["full"]`
- [ ] `backend/src/main.rs` contains `#[tokio::main] async fn main()` that prints the version
- [ ] `cargo build --release -p backend` exits 0
- [ ] `target/release/anvilml` binary exists (or `.exe` on Windows)
- [ ] Running `./target/release/anvilml` outputs a line containing `AnvilML v0.1.0 starting` and returns exit code 0
```

---

## Summary of the Plan

**What needs to happen:**

Two files are modified:

1. **`backend/Cargo.toml`** — Add `tokio = { version = "1", features = ["full"] }` to `[dependencies]`. The `[[bin]] name = "anvilml"` and `anvilml-server` dependency already exist from P1-A1.

2. **`backend/src/main.rs`** — Replace the current `fn main() {}` stub with:
   ```rust
   #[tokio::main]
   async fn main() {
       println!("AnvilML v{} starting", env!("CARGO_PKG_VERSION"));
   }
   ```

**Verification:** `cargo build --release -p backend` must succeed, producing the `anvilml` binary that prints `AnvilML v0.1.0 starting` and exits cleanly.

**Dependencies confirmed:** The workspace root declares `version = "0.1.0"` in `[workspace.package]`, which propagates to `backend`. P1-A1 has already created the workspace structure with all 8 crate directories. P1-A3 will add axum/tower/tokio to `anvilml-server`.

**No tests needed** — this is a placeholder stub per the task description ("This is a placeholder to be replaced in P1-A4").

The plan report should be written to `.forge/reports/P1-A2_plan.md` and `.forge/state/CURRENT_TASK.md` should be updated with `Status: COMPLETE`, then STOP for Forge approval.
