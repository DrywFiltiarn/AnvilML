# Contributing to AnvilML

Thanks for your interest in improving AnvilML. This guide covers how to set up a development
environment, the checks your contribution must pass, and our conventions. By participating you agree
to abide by our [Code of Conduct](./CODE_OF_CONDUCT.md).

> AnvilML is in early development. The authoritative design is
> [`ANVILML_DESIGN.md`](./ANVILML_DESIGN.md); when you change a contract (API, IPC, schema, config),
> update that document in the same pull request.

## Ways to contribute

- **Report bugs** and **request features** using the issue templates.
- **Improve documentation** — including `ANVILML_DESIGN.md` and this guide.
- **Submit code** — pick up a roadmap item (§23) or a triaged issue. For anything large or
  contract-changing, open an issue to discuss the approach first.

## Development setup

### Prerequisites

- A Rust toolchain matching `rust-toolchain.toml` (installed automatically by `rustup` on first
  build).
- Python **3.12.x**.
- Optional: an NVIDIA (CUDA) or AMD (ROCm) GPU. **Not required** — all tests run in mock
  modes on CPU-only machines.

### Build

```bash
git clone https://github.com/DrywFiltiarn/AnvilML.git
cd AnvilML
cargo build --workspace
```

### Python worker (only needed for worker work or end-to-end runs)

```bash
# Linux / macOS
./backend/scripts/install_worker_deps.sh
# Windows (PowerShell)
powershell -ExecutionPolicy Bypass -File .\backend\scripts\install_worker_deps.ps1
```

## Required checks before opening a PR

Your branch must pass everything CI runs. Run it locally first:

```bash
# Rust
cargo fmt --all --check
cargo clippy --workspace --features mock-hardware -- -D warnings
cargo test  --workspace --features mock-hardware

# OpenAPI must stay in sync (CI diffs the committed file)
cargo run -p anvilml-openapi
git diff --exit-code backend/openapi.json   # must be clean

# Python worker (mock mode — no GPU needed)
#   Linux/macOS:
ANVILML_WORKER_MOCK=1 python -m pytest worker/tests
#   Windows (PowerShell):  $env:ANVILML_WORKER_MOCK=1; python -m pytest worker/tests
```

If you change any public type, route, IPC message, or event, regenerate `backend/openapi.json` and
commit it.

## Coding standards

### Rust

- Format with `rustfmt`; no `clippy` warnings (CI treats them as errors).
- Respect the crate boundaries and dependency direction in `ANVILML_DESIGN.md` §2 — e.g. don't make
  `anvilml-core` depend on anything; don't reach across the layering.
- Prefer `thiserror` for library error types and `anyhow` at boundaries.
- Public items carry doc comments; `#[utoipa::path]` / `ToSchema` annotations stay current.

### Python worker

- Target Python 3.12; type-hint public functions.
- New nodes inherit `BaseNode`, self-register via `@register`, and declare `INPUT_SLOTS` /
  `OUTPUT_SLOTS`. Keep tunable defaults in `worker/defaults.py`, not scattered in node code.
- Support mock mode (`ANVILML_WORKER_MOCK=1`) for every node.

### Cross-platform

AnvilML targets **Linux and Windows** as co-equal platforms. Before submitting OS-sensitive code
(process spawning, signals/kill, stdio, paths, file serving), review `ANVILML_DESIGN.md` §22.4 and
ensure both platforms are handled. State in your PR how you verified this.

## Commit and branch conventions

We use [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <summary>
```

- **type**: `feat`, `fix`, `docs`, `refactor`, `test`, `perf`, `build`, `ci`, `chore`.
- **scope**: the crate name **without** the `anvilml-` prefix — `core`, `hardware`, `registry`,
  `ipc`, `worker`, `scheduler`, `server`, `openapi` — plus `py-worker`, `bloomeryui`, or `root`.

Examples: `feat(scheduler): add cooperative job cancellation`,
`fix(py-worker): set binary stdio mode on Windows`.

Branch names: `feat/<short-desc>`, `fix/<short-desc>`, `docs/<short-desc>`.

## Pull request process

1. Fork (or branch, if you have access) and create a focused branch — one logical change per PR.
2. Add or update tests for your change; keep coverage of new logic.
3. Fill in the pull request template completely.
4. Link the issue it closes (`Closes #123`).
5. Ensure CI is green. A maintainer will review; please respond to feedback promptly.
6. Squash-merge is the default; keep the final commit message Conventional-Commit compliant.

## Reporting security issues

Please do **not** file public issues for vulnerabilities. Email
`trinity3dtech@gmail.com` instead.

## Questions

Open an issue and apply the `question` label. Thanks for contributing!
