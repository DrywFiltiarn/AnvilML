## Summary

<!-- What does this PR do and why? Keep it focused on one logical change. -->

## Related issue

<!-- e.g. Closes #123 -->
Closes #

## Type of change

- [ ] Bug fix (non-breaking)
- [ ] New feature (non-breaking)
- [ ] Breaking change (API / IPC / schema / config contract changes)
- [ ] Documentation only
- [ ] Refactor / internal change (no behaviour change)

## Affected components

- [ ] `anvilml-core`
- [ ] `anvilml-hardware`
- [ ] `anvilml-registry`
- [ ] `anvilml-ipc`
- [ ] `anvilml-worker`
- [ ] `anvilml-scheduler`
- [ ] `anvilml-server`
- [ ] `anvilml-openapi`
- [ ] Python worker (`worker/`)
- [ ] Build / CI / scripts
- [ ] Documentation (`ANVILML_DESIGN.md`, README, etc.)

## How has this been tested?

<!-- Commands run and what you observed. Tick what applies. -->

- [ ] `cargo fmt --all --check`
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings`
- [ ] `cargo test --workspace --features mock-hardware`
- [ ] `cargo run -p anvilml-openapi` and `backend/openapi.json` is committed & unchanged
- [ ] `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests`
- [ ] Manual / end-to-end run on real hardware (describe below)

Details:

## Cross-platform

- [ ] Change is OS-agnostic, **or** I verified / accounted for both **Linux and Windows**
      (process spawn, signals/kill, stdio, paths) per `ANVILML_DESIGN.md` §22.4.
- How verified:

## Checklist

- [ ] My commits follow Conventional Commits (correct `type(scope):`).
- [ ] I updated documentation, including `ANVILML_DESIGN.md` if a contract changed.
- [ ] I added/updated tests for my change.
- [ ] No secrets, credentials, or large binaries are included.
- [ ] I have read the [Contributing guide](../CONTRIBUTING.md) and agree to the
      [Code of Conduct](../CODE_OF_CONDUCT.md).

## Screenshots / logs (optional)

<!-- For UI-adjacent or output-affecting changes. -->
