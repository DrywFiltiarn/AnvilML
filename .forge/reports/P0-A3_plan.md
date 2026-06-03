# Plan Report: P0-A3

| Field       | Value                                                       |
|-------------|-------------------------------------------------------------|
| Task ID     | P0-A3                                                       |
| Phase       | 000 — Repository Preamble                                   |
| Description | anvilml: rust-toolchain.toml pinned to 1.95.0 with windows-gnu target |
| Depends on  | P0-A2                                                       |
| Project     | anvilml                                                     |
| Planned at  | 2026-05-31T23:20:00Z                                        |
| Attempt     | 1                                                           |

## Objective

Create `rust-toolchain.toml` at the repository root to pin the Rust toolchain to version 1.95.0 with the `rustfmt` and `clippy` components and the `x86_64-pc-windows-gnu` target. The explicit channel pin prevents rustfmt/clippy version drift between local development and CI — a previously observed failure mode where different Rust versions produced different formatting or lint results. The windows-gnu target enables the local cross-check (`cargo check --target x86_64-pc-windows-gnu`) described in `docs/FORGE_AGENT_RULES.md` §7.7, which catches `#[cfg(windows)]` / `#[cfg(unix)]` API breakage before the native Windows CI job runs.

## Scope

### In Scope
- Create `rust-toolchain.toml` at repository root with exact content: `channel = "1.95.0"`, `components = ["rustfmt", "clippy"]`, `targets = ["x86_64-pc-windows-gnu"]`.
- Verify the toolchain is correctly configured by confirming `rustup show active-toolchain` and `rustc --version` output.

### Out of Scope
- No source code changes (Phase 0 config-only).
- No test files to write or modify.
- No CI workflow modifications.
- No git operations (staging handled in ACT session).
- No dependency lookups via MCP tools (Rust version is explicitly specified; no external crate APIs referenced).

## Approach

1. **Read prerequisite state.** Confirm that P0-A2 (`.gitattributes`) has been completed and the file exists at `AnvilML/.gitattributes`.
2. **Create `rust-toolchain.toml`.** Write the file at `AnvilML/rust-toolchain.toml` with the following exact TOML content:
   ```toml
   [toolchain]
   channel = "1.95.0"
   components = ["rustfmt", "clippy"]
   targets = ["x86_64-pc-windows-gnu"]
   ```
3. **Verify toolchain pin.** Run `rustup show active-toolchain` in the repo root and confirm it reports `1.95.0` (overridden by `rust-toolchain.toml`). Run `rustc --version` and confirm output starts with `rustc 1.95.0`.
4. **Verify cross-check target.** Confirm that `x86_64-pc-windows-gnu` is an available target via `rustup target list --installed | grep windows-gnu`. This target must already be installed (Phase 0 instructions state the environment has gcc-mingw-w64 linker available).
5. **Write implementation report** and update `CURRENT_TASK.md` in the ACT session.

## Files Affected

| Action   | Path                         | Description                                          |
|----------|------------------------------|------------------------------------------------------|
| CREATE   | `rust-toolchain.toml`        | Pinned Rust 1.95.0 toolchain with rustfmt, clippy, and x86_64-pc-windows-gnu target |

## Tests

None.

## CI Impact

No CI changes required. The `rust-toolchain.toml` file is automatically picked up by all CI jobs (GitHub Actions uses the pinned version when no explicit `rust-toolchain-file` is set in the `actions-rs/toolchain` step). No workflow modifications are needed.

## Risks and Mitigations

| Risk                      | Likelihood | Impact | Mitigation              |
|---------------------------|-----------|--------|-------------------------|
| Rust 1.95.0 not yet installed on the local machine | Low | Medium | `rustup install 1.95.0` will be run in ACT session before verification; the version is confirmed available (system reports 1.95.0 on stable channel) |
| `x86_64-pc-windows-gnu` target not installed locally | Low | Medium | `rustup target add x86_64-pc-windows-gnu` will be run in ACT session if needed; the instructions state gcc-mingw-w64 linker is available |
| Toolchain pin conflicts with a newer Rust version already pinned as default | Low | Low | The `rust-toolchain.toml` file in the repo root takes precedence over the global default (confirmed by `rustup show` output showing `stable` as default but overridden by TOML) |

## Acceptance Criteria

- [ ] `rust-toolchain.toml` exists at repository root (`AnvilML/rust-toolchain.toml`)
- [ ] File contains `[toolchain]` section with `channel = "1.95.0"`
- [ ] File contains `components = ["rustfmt", "clippy"]`
- [ ] File contains `targets = ["x86_64-pc-windows-gnu"]`
- [ ] `rustup show active-toolchain` in repo root reports 1.95.0
- [ ] `rustc --version` prints `rustc 1.95.0` (or `rustc 1.95.0 (...)`)
