# Plan Report: P4-A2B

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P4-A2B                                      |
| Phase       | 004 — Persistence & Model Registry          |
| Description | anvilml — naming correction (binary `anvilml`, database `anvilml.db`) |
| Depends on  | P4-A1, P4-A2                                |
| Project     | anvilml                                     |
| Planned at  | 2026-05-30T16:15:00Z                        |
| Attempt     | 1                                           |

## Objective

Apply the naming corrections introduced by the `ANVILML_DESIGN.md` Rev 3 amendment: rename the launcher binary from `sindristudio` to `anvilml`, change the default database path from `./sindristudio.db` to `./anvilml.db`, and update all code-level references (doc comments, test assertions) that hardcode the old binary or DB name. This ensures the release binary is `target/release/anvilml` and the default SQLite database is `anvilml.db` before any later phase depends on these names.

## Scope

### In Scope
- `backend/Cargo.toml`: change `[[bin]] name = "sindristudio"` to `name = "anvilml"`; update description comment from `(sindristudio)` to `(anvilml)`
- `crates/anvilml-core/src/config.rs`: change `default_db_path()` to return `PathBuf::from("./anvilml.db")` (was `"./sindristudio.db"`); update the test assertion in `config_default_deserialize` that checks for the old path
- `backend/src/main.rs`: update doc comment from `(sindristudio)` to `(anvilml)`
- `anvilml.toml`: add `db_path = "./anvilml.db"` to replace the placeholder (the file currently has no config fields)

### Out of Scope
- `SindriStudio` (capitalised, the one-click launcher product) — this is a separate repository and its references are not modified
- Documentation files (`docs/ENVIRONMENT.md`, `docs/ARCHITECTURE.md`, `README.md`, `SECURITY.md`, etc.) — these describe the SindriStudio project architecture and are updated separately in a documentation task
- Python worker file `worker/ipc.py` — its doc comment references sindristudio but is outside the Rust codebase scope
- Any crate name, API path, IPC message field, environment variable, or config key — all remain `anvilml`-prefixed and are unaffected

## Approach

1. **Update `backend/Cargo.toml`**
   - Line 5: Change `description = "AnvilML launcher binary (sindristudio)"` to `description = "AnvilML launcher binary (anvilml)"`
   - Line 8: Change `name = "sindristudio"` to `name = "anvilml"`

2. **Update `crates/anvilml-core/src/config.rs`**
   - Line 222 in `default_db_path()`: Change `PathBuf::from("./sindristudio.db")` to `PathBuf::from("./anvilml.db")`
   - Line 409 in test `config_default_deserialize`: Change `assert_eq!(config.db_path, PathBuf::from("./sindristudio.db"))` to `assert_eq!(config.db_path, PathBuf::from("./anvilml.db"))`

3. **Update `backend/src/main.rs`**
   - Line 1 doc comment: Change `//! AnvilML launcher binary (sindristudio).` to `//! AnvilML launcher binary (anvilml).`

4. **Update `anvilml.toml`**
   - Replace the placeholder comment block with a minimal config containing `db_path = "./anvilml.db"` (the file currently contains only comments with no config fields)

5. **Verify tests pass**
   - Run `cargo test -p anvilml-core -- config::tests` to confirm the updated assertion passes

## Files Affected

| Action   | Path                              | Description                                            |
|----------|-----------------------------------|--------------------------------------------------------|
| MODIFY   | backend/Cargo.toml                | Rename binary from sindristudio → anvilml; update description comment |
| MODIFY   | crates/anvilml-core/src/config.rs | Update default_db_path() return value and test assertion |
| MODIFY   | backend/src/main.rs               | Update doc comment to reference anvilml instead of sindristudio |
| MODIFY   | anvilml.toml                      | Add db_path = "./anvilml.db" config field              |

## Tests

| Test ID / Name            | File                     | Validates               |
|---------------------------|--------------------------|-------------------------|
| config_default_deserialize (updated assertion) | crates/anvilml-core/src/config.rs | Empty TOML deserializes with db_path = "./anvilml.db" |
| config_round_trip (existing test) | crates/anvilml-core/src/config.rs | Round-trip still works after default change |
| config_frontend_modes (existing test) | crates/anvilml-core/src/config.rs | No regression in FrontendMode variants |

## CI Impact

No CI changes required. The `mock-hardware` feature flag, test commands, and workflow structure are unaffected by these naming corrections.

## Risks and Mitigations

| Risk                      | Likelihood | Impact | Mitigation              |
|---------------------------|-----------|--------|-------------------------|
| Test assertion in config_default_deserialize fails if old path is not updated | Low | High | The plan explicitly lists the line to update; verification step catches it |
| anvilml.toml placeholder removal breaks user expectations of a "to-be-populated" file | Low | Low | The new content adds only db_path (the task's focus); comments retained explaining the file is a reference config |
| Other crates reference the old binary name in doc comments or logs | Low | Low | Only the minimal set identified by the task and code search are changed; broader doc cleanup is out of scope |

## Acceptance Criteria

- [ ] `backend/Cargo.toml` has `[[bin]] name = "anvilml"` (line 8)
- [ ] `crates/anvilml-core/src/config.rs` `default_db_path()` returns `PathBuf::from("./anvilml.db")`
- [ ] Test `config_default_deserialize` asserts `db_path == PathBuf::from("./anvilml.db")`
- [ ] `backend/src/main.rs` doc comment references `anvilml` not `sindristudio`
- [ ] `anvilml.toml` contains `db_path = "./anvilml.db"`
- [ ] `cargo test -p anvilml-core -- config::tests` exits 0
