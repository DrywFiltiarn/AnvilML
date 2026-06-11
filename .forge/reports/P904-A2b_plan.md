# Plan Report: P904-A2b

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P904-A2b                                    |
| Phase       | 904 — Test Isolation Hardening              |
| Description | backend: fix resolve_interpreter_unix test running on Windows without platform guard |
| Depends on  | P18-A4                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-06-11T07:30:00Z                        |
| Attempt     | 1                                           |

## Objective

Add a `#[cfg(not(windows))]` attribute to the `resolve_interpreter_unix` unit test in `backend/src/preflight.rs` so it is skipped on Windows, preventing a panic caused by the test asserting a Unix interpreter path (`/opt/myvenv/bin/python3`) while `resolve_interpreter()` correctly returns the Windows path on that platform.

## Scope

### In Scope
- Add `#[cfg(not(windows))]` attribute to the `resolve_interpreter_unix` test function in `backend/src/preflight.rs` (lines 196–202).
- No changes to production code (`resolve_interpreter` function or `run_preflight`).
- No changes to `resolve_interpreter_windows` (already uses `#[cfg(windows)]` inside its body).
- No new test files, no new dependencies.

### Out of Scope
- Any changes to `backend/tests/` integration test files (handled by P904-A2).
- Any changes to `crates/anvilml-scheduler/` (handled by P904-A1).
- Any workspace-wide test verification (handled by P904-A3).
- Any CI workflow modifications.

## Approach

1. Open `backend/src/preflight.rs`, locate the `resolve_interpreter_unix` test function within the `#[cfg(test)] mod tests` block (currently at lines 196–202).
2. Insert `#[cfg(not(windows))]` as an attribute on the function, placing it between `#[test]` and `fn resolve_interpreter_unix() {`.
3. The resulting test block will be:
   ```rust
   #[test]
   #[cfg(not(windows))]
   fn resolve_interpreter_unix() {
       let venv = Path::new("/opt/myvenv");
       let result = resolve_interpreter(venv);
       assert_eq!(result, PathBuf::from("/opt/myvenv/bin/python3"));
   }
   ```
4. Verify the file still compiles: `cargo check -p backend --features mock-hardware`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `backend/src/preflight.rs` | Add `#[cfg(not(windows))]` to `resolve_interpreter_unix` test function |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|-----------------|
| `backend/src/preflight.rs` (inline) | `resolve_interpreter_unix` | On Unix: asserts `resolve_interpreter()` returns `bin/python3` path. On Windows: test is skipped (no assertion). |
| `backend/src/preflight.rs` (inline) | `resolve_interpreter_windows` | On Windows: asserts `resolve_interpreter()` returns `Scripts/python.exe` path. On Unix: no-op (cfg inside body). |

## CI Impact

No CI changes required. The existing CI gates (`cargo test --workspace --features mock-hardware`, `cargo clippy --workspace --features mock-hardware -- -D warnings`, etc.) will automatically pick up the fix. The test was previously running on Windows and panicking — this change simply makes it skip on that platform, which is the correct behavior.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The `#[cfg(not(windows))]` attribute is placed incorrectly, causing a compile error | Low | Build failure | The placement is straightforward — between `#[test]` and `fn`. Verify with `cargo check` immediately after edit. |
| The existing `resolve_interpreter_windows` test body uses `#[cfg(windows)]` inside the function (not on the function itself), meaning the function still runs on Unix as a no-op. A future reader might be confused | Low | Confusion (not correctness) | This is pre-existing behavior documented in TASKS_PHASE904.md §Known Constraints. Out of scope for this task. |
| No risk of regressions — only a test attribute change, zero production code touched | n/a | n/a | Minimal change surface; isolated to one test function. |

## Acceptance Criteria

- [ ] `cargo check -p backend --features mock-hardware` exits 0
- [ ] `cargo test -p backend --features mock-hardware -- preflight` exits 0 on Linux
- [ ] Cross-check: `cargo test -p backend --features mock-hardware --target x86_64-pc-windows-gnu -- preflight` exits 0 on Linux (cross-compilation target)
