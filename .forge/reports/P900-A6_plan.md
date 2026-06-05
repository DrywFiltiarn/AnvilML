# Plan Report: P900-A6

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P900-A6                                           |
| Phase       | 900 — Logging Retrofit                            |
| Description | anvilml-registry: retrofit WARN discipline and DEBUG per-file log to scanner.rs |
| Depends on  | none                                              |
| Project     | anvilml                                           |
| Planned at  | 2026-06-06T00:00:00Z                              |
| Attempt     | 1                                                 |

## Objective

Retrofit `crates/anvilml-registry/src/scanner.rs` with two logging improvements required by FORGE_AGENT_RULES §11: (1) apply WARN field discipline — split the three existing `tracing::warn!` call sites so that `io::ErrorKind::NotFound` omits the redundant `error=` field while all other errors retain it; (2) add mandatory DEBUG per-file logging so every examined file is logged at DEBUG level, whether accepted or skipped.

## Scope

### In Scope
- Modify three existing `tracing::warn!` sites in `scan_dirs()` to distinguish `NotFound` from other `io::ErrorKind` values, omitting `error=` for `NotFound`.
- Add `tracing::debug!` call before each `push()` (accepted file) and before each `continue` or fallback path (skipped file).
- No changes to test files. No changes to logic, control flow, or data structures.

### Out of Scope
- Any change to `Cargo.toml` — `tracing` is already a dependency.
- Changes to any other crate or file.
- New tests (phase 900 tasks are logging-only).
- Changes to the scanner's public API surface.

## Approach

1. **WARN discipline — walkdir entry error (line ~82–86):**
   Change from:
   ```rust
   tracing::warn!(path = %dir_config.path.display(), error = %e, "scanner: skipping unreadable entry");
   ```
   To a branch on the wrapped I/O error kind:
   ```rust
   let path = dir_config.path.clone();
   if e.io_error().map(|inner| inner.kind()) == Some(io::ErrorKind::NotFound) {
       tracing::warn!(path = %path.display(), "scanner: skipping missing path");
   } else {
       tracing::warn!(path = %path.display(), error = %e, "scanner: skipping unreadable entry");
   }
   ```

2. **WARN discipline — metadata error (line ~105–108):**
   Change from:
   ```rust
   tracing::warn!(path = %entry.path().display(), error = %e, "scanner: skipping file with unreadable metadata");
   ```
   To a branch on `io::ErrorKind`:
   ```rust
   if e.kind() == io::ErrorKind::NotFound {
       tracing::warn!(path = %entry.path().display(), "scanner: skipping missing path");
   } else {
       tracing::warn!(path = %entry.path().display(), error = %e, "scanner: skipping file with unreadable metadata");
   }
   ```

3. **WARN discipline — canonicalize error (line ~122–125):**
   Change from:
   ```rust
   tracing::warn!(path = %entry.path().display(), error = %e, "scanner: canonicalize failed, using raw path");
   ```
   To a branch on `io::ErrorKind`:
   ```rust
   if e.kind() == io::ErrorKind::NotFound {
       tracing::warn!(path = %entry.path().display(), "scanner: skipping missing path");
   } else {
       tracing::warn!(path = %entry.path().display(), error = %e, "scanner: canonicalize failed, using raw path");
   }
   ```

4. **DEBUG — accepted file:**
   After the `results.push(ModelMeta { … });` block (line ~152–161), add:
   ```rust
   tracing::debug!(path = %canonical_path.display(), id = %id, "scanner: accepted");
   ```

5. **DEBUG — skipped files at each continue/fail site:**
   - After `if !entry.file_type().is_file() { continue; }` (line ~89–91):
     ```rust
     tracing::debug!(path = %entry.path().display(), reason = "not a file", "scanner: skipped");
     ```
   - After the `None` extension branch (line ~96–97):
     ```rust
     tracing::debug!(path = %entry.path().display(), reason = "extension not matched", "scanner: skipped");
     continue;
     ```
   - After the allowed-extension check (line ~98–100):
     ```rust
     tracing::debug!(path = %entry.path().display(), reason = "extension not matched", "scanner: skipped");
     continue;
     ```

6. **Verify compilation and tests:**
   Run `cargo test -p anvilml-registry -- scanner` to confirm exit 0.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-registry/src/scanner.rs` | Split WARN calls at 3 sites by error kind; add DEBUG per-file logs for accepted and skipped paths |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `crates/anvilml-registry/src/scanner.rs` (inline) | `test_infer_kind_matches` | Unit tests pass after logging changes |
| `crates/anvilml-registry/src/scanner.rs` (inline) | `test_infer_kind_case_insensitive` | Same |
| `crates/anvilml-registry/src/scanner.rs` (inline) | `test_infer_kind_fallback` | Same |
| `crates/anvilml-registry/src/scanner.rs` (inline) | `test_infer_dtype_matches` | Same |
| `crates/anvilml-registry/src/scanner.rs` (inline) | `test_infer_dtype_case_insensitive` | Same |
| `crates/anvilml-registry/src/scanner.rs` (inline) | `test_infer_dtype_unknown` | Same |
| `crates/anvilml-registry/src/scanner.rs` (inline) | `test_vram_estimate_mib` | Same |
| `crates/anvilml-registry/src/scanner.rs` (inline) | `test_sha256_hex` | Same |

## CI Impact

No CI changes required. This task modifies only source code in the scanner module; no new dependencies, no test files added or removed, no CI workflow file touched. All existing CI gates (`cargo test`, `cargo clippy`, `cargo fmt --check`) apply normally to the modified crate.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `walkdir::Error` does not expose `.io_error()` on all Rust versions | Low | Compilation failure | Verify API via rust-docs MCP; fallback to `.io_error_description()` or string matching if needed |
| DEBUG logs for skipped files change test output (e.g. if tests capture stderr) | Very Low | Test failure | Task uses `-- scanner` filter which only runs unit tests — these do not invoke `scan_dirs()` and produce no log output |
| Splitting WARN calls changes the log message text for non-NotFound errors, causing downstream log aggregation mismatches | Low | Operational (not CI) | Messages are identical to existing ones; this is a corrective change, not a new convention |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-registry -- scanner` exits 0
- [ ] All three WARN sites distinguish `NotFound` from other error kinds per §11.4
- [ ] Every accepted file emits a DEBUG log with `path=` and `id=` fields
- [ ] Every skipped file emits a DEBUG log with `path=` and `reason=` fields
- [ ] No test files modified
- [ ] No logic changes — control flow identical to pre-task state
