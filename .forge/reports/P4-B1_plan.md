# Plan Report: P4-B1

| Field       | Value                                                       |
|-------------|-------------------------------------------------------------|
| Task ID     | P4-B1                                                       |
| Phase       | 004 — Hardware Detection                                    |
| Description | anvilml: reconcile frontend.mode default to Headless (retrofit; corrects earlier phases) |
| Depends on  | P3-A6                                                       |
| Project     | anvilml                                                     |
| Planned at  | 2026-06-03T17:15:00Z                                        |
| Attempt     | 1                                                           |

## Objective

Retrofit the committed `config.rs` and `anvilml.toml` so that `FrontendMode::default()` is `Headless` (not `Local { path: "./bloomery" }`), update the `[frontend]` section in `./anvilml.toml` to match, and update `docs/ENVIRONMENT.md` frontend documentation — all in one atomic change so P3-B2's drift guard (`config_reference`) never sees a mismatch.

## Scope

### In Scope
- `crates/anvilml-core/src/config.rs`: Change `FrontendMode::default()` to return `Self::Headless`; update the doc comment on the `Local` variant to remove "default: ./bloomery"; update the test assertion at line 366 from `FrontendMode::Local { .. }` to `FrontendMode::Headless`.
- `anvilml.toml`: Replace the `[frontend.mode.Local]` section (lines 87–89) with a flat `[frontend] mode = "headless"` section matching `ServerConfig::default()` serialization.
- `docs/ENVIRONMENT.md`: Verify §2 `[frontend]` block already says `mode = "headless"` (it does per current read); verify §3.3 env table already shows default `headless` (it does); no changes needed if they already match.

### Out of Scope
- Any change to `config_load.rs` — the `parse_frontend_mode("local")` fallback path uses `"./bloomery"` as a hardcoded default for env-var parsing; this is separate from the built-in Default impl and is not changed (scope: only built-in defaults).
- Any change to `anvilml-server` frontend serving logic (file does not yet exist; will be added in a future phase).
- Any CI workflow changes.
- Any changes to hardware types, model dirs, ROCm config, GPU selection, or limits.

## Approach

1. **Edit `crates/anvilml-core/src/config.rs`** — three changes:
   - Line 82 (doc comment on `FrontendMode::Local`): Replace `"Serve static files from a local directory (default: ./bloomery adjacent to the binary)."` with `"Serve static files from a local directory (for a custom/third-party frontend). Not used for BloomeryUI, which SindriStudio runs as a separate server."`
   - Lines 90–96 (`impl Default for FrontendMode`): Replace the body so `default()` returns `Self::Headless` instead of `Self::Local { path: PathBuf::from("./bloomery") }`.
   - Line 366 (test `test_default_server_config`): Change `assert!(matches!(config.frontend.mode, FrontendMode::Local { .. }))` to `assert!(matches!(config.frontend.mode, FrontendMode::Headless))`.

2. **Edit `anvilml.toml`** — one change:
   - Lines 86–89: Replace the `[frontend.mode.Local]` section with a flat `[frontend]` section containing `mode = "headless"`, preceded by a comment explaining that AnvilML is head-by-default and BloomeryUI runs separately.

3. **Verify `docs/ENVIRONMENT.md`** — read §2 and §3.3 to confirm they already match the new default (`mode = "headless"`). If they do, no edit needed. If not, update accordingly.

4. **Run verification commands:**
   - `cargo test -p anvilml-core -- config` — ensures unit tests pass (the updated assertion and roundtrip test).
   - `cargo test -p backend --features mock-hardware --test config_reference` — ensures the drift guard sees matching key-sets between committed TOML and `ServerConfig::default()` serialization.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| EDIT | `crates/anvilml-core/src/config.rs` | Change `FrontendMode::default()` to `Headless`; update doc comment; update test assertion |
| EDIT | `anvilml.toml` | Replace `[frontend.mode.Local] path = "./bloomery"` with `[frontend] mode = "headless"` |
| VERIFY | `docs/ENVIRONMENT.md` | Confirm §2 and §3.3 already match (likely no edit needed) |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `crates/anvilml-core/src/config.rs` (`mod tests`) | `test_default_server_config` | `ServerConfig::default().frontend.mode == Headless` (updated assertion) |
| `crates/anvilml-core/src/config.rs` (`mod tests`) | `test_toml_roundtrip` | Round-trip serialization still works with headless default |
| `backend/tests/config_reference.rs` | `test_toml_key_set_matches_default` | Drift guard: committed `anvilml.toml` key-set matches `ServerConfig::default()` serialized key-set |

## CI Impact

No CI changes required. The existing CI jobs (`rust`, `rust-windows`) already run `cargo test --features mock-hardware` across all crates including `anvilml-core` and `backend/tests/config_reference.rs`. The change only modifies default values and tests — no new dependencies, features, or workflow files are introduced.

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| TOML key-set mismatch between `anvilml.toml` and `ServerConfig::default()` serialization causes `config_reference` test failure | Write both changes atomically in one session; the plan ensures `[frontend] mode = "headless"` matches exactly how serde serializes `FrontendConfig { mode: FrontendMode::Headless }` |
| Doc comment update on `FrontendMode::Local` could be missed by tests (non-code change) | The doc comment is informational only; the functional correctness is verified by the unit test and drift guard. No risk of runtime impact. |
| `config_load.rs` `parse_frontend_mode("local")` still uses `"./bloomery"` as hardcoded path — could confuse users who set `ANVILML_FRONTEND__MODE=local` | Out of scope for this task. Changing it would require a separate retrofit task. The env-var parsing fallback is independent of the built-in Default impl and does not affect the drift guard. |

## Acceptance Criteria

- [ ] `FrontendMode::default()` returns `FrontendMode::Headless`
- [ ] `ServerConfig::default().frontend.mode == FrontendMode::Headless` (verified by test)
- [ ] `anvilml.toml` contains `[frontend] mode = "headless"` and no leftover `[frontend.mode.Local]` section
- [ ] `cargo test -p anvilml-core -- config` exits 0 (all config unit tests pass)
- [ ] `cargo test -p backend --features mock-hardware --test config_reference` exits 0 (drift guard passes)
- [ ] `docs/ENVIRONMENT.md` §2 and §3.3 frontend section reflects `headless` as the default
