# Plan Report: P900-A2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P900-A2                                     |
| Phase       | 900 — Logging Retrofit                      |
| Description | anvilml-registry: retrofit INFO logging to seed_loader.rs (seed applied/skipped) |
| Depends on  | P900-A1                                     |
| Project     | anvilml                                     |
| Planned at  | 2026-06-05T21:45:00Z                        |
| Attempt     | 1                                           |

## Objective

Add two mandatory INFO log calls to `crates/anvilml-registry/src/seed_loader.rs` so that seed file apply and skip decisions are observable at the default log level, satisfying FORGE_AGENT_RULES §11.3 and ENVIRONMENT.md §9 (Seeds subsystem).

## Scope

### In Scope
- One file: `crates/anvilml-registry/src/seed_loader.rs`
- Add `tracing::info!` call in the SHA256-match (skip) branch, before `continue;`
- Add `tracing::info!` call in the execution (apply) branch, after `execute_seed()` returns and before the `INSERT OR REPLACE` upsert
- No changes to logic, control flow, tests, or any other file
- `tracing` is already a dependency of `anvilml-registry` (Cargo.toml line 13); no manifest change needed

### Out of Scope
- Any other source file (db.rs, scanner.rs, etc. belong to other P900-A tasks)
- Test additions or modifications
- Cargo.toml changes
- Logic changes to execute_seed, parse_header, compute_sha256, extract_body, or run

## Approach

1. **Locate the skip branch** in `run()` at line ~204 where `stored_hash == hash` is true and `continue;` executes. Insert a `tracing::info!` call immediately before `continue;`:
   ```rust
   tracing::info!(file = %filename, status = "up-to-date", "seed skipped");
   ```

2. **Locate the apply branch** in `run()` at line ~209 where `execute_seed()` is called. Insert a `tracing::info!` call immediately after `execute_seed()` returns successfully and before the `INSERT OR REPLACE` upsert:
   ```rust
   tracing::info!(file = %filename, sha256 = %hash, "seed applied");
   ```

3. **Verify** both calls use structured `=` notation with `%` for string values per §11.6. No string interpolation (`{}`) anywhere in the log call arguments.

4. **Run acceptance test:** `cargo test -p anvilml-registry -- seed` must exit 0 with no regressions.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-registry/src/seed_loader.rs` | Add two `tracing::info!` calls in `run()` — skip branch (before `continue;`) and apply branch (after `execute_seed()`, before upsert) |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|-----------------|
| `crates/anvilml-registry/src/seed_loader.rs` (existing tests in `mod tests`) | `test_parse_header_both_directives` | Parsing unchanged — no regression |
| `crates/anvilml-registry/src/seed_loader.rs` (existing tests) | `test_parse_header_defaults_strategy` | Parsing unchanged — no regression |
| `crates/anvilml-registry/src/seed_loader.rs` (existing tests) | `test_parse_header_missing_table` | Error path unchanged — no regression |
| `crates/anvilml-registry/src/seed_loader.rs` (existing tests) | `test_parse_header_empty_file` | Error path unchanged — no regression |
| `crates/anvilml-registry/src/seed_loader.rs` (existing tests) | `test_compute_sha256_known_value` | SHA256 computation unchanged — no regression |
| `crates/anvilml-registry/src/seed_loader.rs` (existing tests) | `test_compute_sha256_empty` | SHA256 computation unchanged — no regression |

## CI Impact

No CI changes required. No new dependencies, no Cargo.toml modifications, no new test files, and no changes to CI workflow files. The existing CI gates (format, clippy, tests) will continue to apply unchanged.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `tracing` macro not imported — module does not have `use tracing;` | Low | Build failure | The crate already depends on `tracing = { workspace = true }`; the `tracing::info!` macro is invoked with full path qualification, no explicit import needed |
| Log call placement changes control flow or variable lifetime | Low | Logic regression | Both calls are placed as standalone statements before existing operations (`continue;` and upsert); no variable borrowing affected |
| Hex format of `hash` is too long for INFO log readability | Low | Operator experience | The task spec explicitly uses `%hash` (full SHA256 hex string, 64 chars); this matches the ENVIRONMENT.md §9 requirement for `sha256=` field. No truncation planned. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-registry -- seed` exits 0 with no regressions
- [ ] Both log calls use structured `=` notation (no string interpolation)
- [ ] Skip branch logs `file=%filename, status="up-to-date", "seed skipped"`
- [ ] Apply branch logs `file=%filename, sha256=%hash, "seed applied"`
- [ ] No changes to any file other than `crates/anvilml-registry/src/seed_loader.rs`
- [ ] No logic changes — only log call additions
