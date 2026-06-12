# Plan Report: P20-A3

| Field | Value |
|-------|-------|
| Task ID | P20-A3 |
| Phase | 020 — OpenAPI & Launcher Polish |
| Description | anvilml: browser auto-open at startup (unless --no-browser/Headless) |
| Depends on | P20-A2 |
| Project | anvilml |
| Planned at | 2026-06-12T08:33:00Z |
| Attempt | 1 |

## Objective

Add the `open` crate dependency and implement browser auto-open in the AnvilML launcher binary (`backend/src/main.rs`). After the HTTP server binds and is confirmed reachable, open the user's default browser to the server URL — unless `--no-browser` is passed or the frontend mode is `Headless` (the default).

## Scope

### In Scope
- Add `open` crate (v5.3.5) to workspace dependencies and backend `Cargo.toml`
- Add browser-open logic in `backend/src/main.rs` after server bind, before `axum::serve`
- Condition: skip when `args.no_browser` is true OR `cfg.frontend.mode == FrontendMode::Headless`
- Log failure with `tracing::warn!` (do not abort startup)
- Add DEBUG log for browser open attempt (success and skip cases)
- Bump `backend` crate patch version from `0.1.12` to `0.1.13`

### Out of Scope
- No new CLI flags (the `--no-browser` flag already exists in `cli.rs`)
- No changes to `cli.rs`, `anvilml-core`, or any crate other than `backend`
- No browser-open for `--print-hardware` path
- No changes to CI, tests, or other crates
- No platform-specific browser-open logic (the `open` crate handles cross-platform)

## Approach

1. **Add dependency.** Add `open = "5.3"` to `[workspace.dependencies]` in root `Cargo.toml`, then add `open = { workspace = true }` to `[dependencies]` in `backend/Cargo.toml`.

2. **Add browser-open logic in `main.rs`.** After the TCP listener is bound (line ~305) and before `axum::serve` is called (line ~309), insert a conditional block:
   - Check `args.no_browser` — if `true`, log at DEBUG level (`"skipping browser open: --no-browser flag set"`) and skip.
   - Check `cfg.frontend.mode` against `FrontendMode::Headless` — if headless, log at DEBUG level (`"skipping browser open: frontend mode is headless"`) and skip.
   - Otherwise, construct the URL string `http://host:port` and call `open::that(url)`.
   - On success: log at DEBUG level (`"browser opened to {url}"`).
   - On error: log at WARN level (`"failed to open browser: {error}"`) — do not abort.

3. **Import `FrontendMode`.** Add `FrontendMode` to the `anvilml_core::` import line in `main.rs` (currently imports `load_config, DeviceType, EnumerationSource, HardwareInfo`).

4. **Bump version.** Increment `backend/Cargo.toml` `[package] version` from `"0.1.12"` to `"0.1.13"`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `Cargo.toml` | Add `open = "5.3"` to `[workspace.dependencies]` |
| Modify | `backend/Cargo.toml` | Add `open = { workspace = true }` to `[dependencies]`; bump version to `0.1.13` |
| Modify | `backend/src/main.rs` | Add `FrontendMode` import; add browser-open logic after server bind |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|-----------------|
| `backend/src/cli.rs` (existing) | `test_args_to_overrides_with_values` (existing) | `no_browser: true` parses correctly — already covered, no new test needed |
| None | — | Browser-open is an OS-level side effect; the logic is a straightforward conditional with no branching complexity. The `open::that` crate itself is well-tested upstream. |

## CI Impact

No CI workflow files are modified. The change only affects `backend/Cargo.toml` and `backend/src/main.rs`. The existing CI gates (format, clippy, test, cross-checks) will automatically validate the change. The `open` crate compiles on all platforms (Linux, Windows, macOS) and is a no-op in headless/server environments where no display is available — it returns an error which we log but do not abort on.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `open` crate fails on headless server (no display) | High | Low | We log the error at WARN level and do not abort — per design spec. |
| `open` crate cross-compilation issue (Windows target from Linux) | Low | Medium | The `open` crate is well-tested on all major platforms; if it fails, we fall back to a `#[cfg(unix)]` / `#[cfg(windows)]` shim or skip the feature. |
| Browser opens unexpectedly on CI runner | Low | Low | CI runs with `--features mock-hardware` but the browser-open logic depends on `cfg.frontend.mode` which defaults to `Headless`, so no browser opens in CI. |
| Version bump conflicts with concurrent work on `backend/Cargo.toml` | Low | Low | Standard version bump; only the patch digit changes. |

## Acceptance Criteria

- [ ] `open` crate v5.3 added to workspace deps and backend deps
- [ ] `backend/src/main.rs` opens browser when `--no-browser` is absent AND `frontend.mode != Headless`
- [ ] `backend/src/main.rs` skips browser when `--no-browser` is present
- [ ] `backend/src/main.rs` skips browser when `frontend.mode == Headless` (default)
- [ ] Browser-open failure logs WARN but does not abort server startup
- [ ] DEBUG log emitted for browser open attempt (success or skip)
- [ ] `backend/Cargo.toml` version bumped from `0.1.12` to `0.1.13`
- [ ] `cargo check --workspace --features mock-hardware` exits 0
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
- [ ] `cargo fmt --all -- --check` exits 0
