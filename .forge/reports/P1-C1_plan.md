# Plan Report: P1-C1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P1-C1                                       |
| Phase       | 1 — Repository Scaffold                     |
| Description | anvilml.toml checked-in reference config (scaffold defaults) |
| Depends on  | P1-A2                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-06-26T14:15:00Z                        |
| Attempt     | 1                                           |

## Objective

Create the `anvilml.toml` reference configuration file at the repository root with exactly two scaffold-relevant fields — `host = "127.0.0.1"` and `port = 8488` — at their documented defaults. This file serves as the canonical config reference that the `config_reference` test (added in a later phase when `ServerConfig` exists) will validate against. No other fields are added at this phase; `db_path`, `artifact_dir`, and other fields belong to tasks that introduce their corresponding `ServerConfig` struct fields in Phase 2.

## Scope

### In Scope
- Create `anvilml.toml` at the repository root with exactly two keys: `host = "127.0.0.1"` and `port = 8488`.
- Add a TOML comment header (lines starting with `#`) noting that this file is the canonical config reference checked by the `config_reference` test.
- No source code changes, no dependency changes, no test changes.

### Out of Scope
- No `ServerConfig` struct or config loading code — that is Phase 2 scope.
- No additional fields (`db_path`, `artifact_dir`, `venv_path`, `model_scan_depth`, `max_ipc_payload_mib`, `num_threads`, `[model_dirs]`, `[gpu_selection]`, `[limits]`, `[rocm]`, `[hardware_override]`) — each is added by the task that introduces its matching `ServerConfig` field to keep this file from drifting ahead of the actual config schema.
- No `config_reference` test — that is added in a later phase once `ServerConfig` exists.
- No changes to `docs/ENVIRONMENT.md §4` — the field reference table is updated when the first `ServerConfig` fields are introduced (Phase 2), not at this scaffold stage.

## Existing Codebase Assessment

No prior source exists for this task. The workspace scaffold (P1-A1 through P1-B6) has already established the Cargo workspace, all ten crates as empty stubs, the `backend` binary with CLI parsing and shutdown signal, the `anvilml-openapi` stub, and the `api/` directory. The repository root contains `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `.gitattributes`, and `LICENSE` — but no `anvilml.toml`. This task is the first to create a non-code file at the repository root, establishing the config reference pattern that subsequent phases will extend.

## Resolved Dependencies

None. This task creates a plain TOML configuration file with no external dependencies.

## Approach

defers_to (from JSON): []

1. **Create `anvilml.toml` at the repository root.** Write a single TOML file containing:
   - A multi-line comment header (4–5 lines starting with `#`) explaining that this file is the canonical config reference checked by the `config_reference` test (added in a later phase when `ServerConfig` exists), and that fields beyond `host` and `port` are intentionally omitted at this phase.
   - `host = "127.0.0.1"` on its own line.
   - `port = 8488` on its own line.
   - No trailing keys, no sections, no blank lines after `port`.

   The file content will be:

   ```toml
   # anvilml.toml — Canonical config reference for AnvilML.
   #
   # This file is the source of truth for the `config_reference` test
   # (added in a later phase once ServerConfig exists). The test asserts
   # that ServerConfig::default() serialises to exactly the same key set
   # as this file. Fields are added one-at-a-time alongside their
   # ServerConfig struct fields to prevent drift.
   #
   # Phase 1: only host and port exist. db_path, artifact_dir, and all
   # other fields are introduced by Phase 2+ tasks that add the matching
   # ServerConfig fields.

   host = "127.0.0.1"
   port = 8488
   ```

   This is a pure file-creation step — no code, no build, no runtime behavior. The TOML format uses standard toml-rs syntax which is the same format used by the `config_load.rs` module (Phase 2) when it deserialises this file via `toml::from_str`.

## Public API Surface

None. This task creates a data file, not source code.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `anvilml.toml` | Root-level reference config with `host` and `port` scaffold defaults |

## Tests

None. This task creates a plain configuration file with no executable code, no functions, and no types. The `config_reference` test that validates this file against `ServerConfig::default()` is added in a later phase once the config struct exists.

## CI Impact

No CI changes required. The `config-drift` CI job (defined in P1-E2) is a placeholder echo at this phase and does not yet run the `config_reference` test. No new file types or test modules are introduced.

## Platform Considerations

None identified. The TOML file is platform-neutral — no line-ending handling, path separators, or `#[cfg]` guards are needed. The `.gitattributes` file (created in P1-A1) already enforces LF line endings for all text files at the repo root, which includes `anvilml.toml`.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The TOML file is accidentally parsed by a build tool that expects a specific format (e.g. Cargo.toml) | Low | Low | The filename `anvilml.toml` is not a standard Cargo manifest name; Cargo only reads `Cargo.toml`, `Cargo.lock`, and files specified via `--manifest-path`. No build tool in this project reads arbitrary `.toml` files at the root. |
| Future `config_reference` test fails because the comment header is parsed as a TOML key | Low | Medium | TOML comment lines (starting with `#`) are ignored by all standard TOML parsers including `toml-rs`. The test in Phase 2 will parse the file and compare key sets, which naturally excludes comments. |

## Acceptance Criteria

- [ ] `test -f anvilml.toml` exits 0
- [ ] `grep -c '^host = ' anvilml.toml` equals 1 (exactly one host key)
- [ ] `grep -c '^port = ' anvilml.toml` equals 1 (exactly one port key)
- [ ] `grep -cE '^(db_path|artifact_dir|venv_path|model_scan_depth|max_ipc_payload_mib|num_threads|\[)' anvilml.toml` equals 0 (no other keys or sections)
- [ ] `cat anvilml.toml` shows exactly `host = "127.0.0.1"` and `port = 8488` with comment header above
