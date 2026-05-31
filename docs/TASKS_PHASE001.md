# Tasks: Phase 001 — Walking Skeleton

| Field | Value |
|-------|-------|
| Phase | 001 |
| Name | Walking Skeleton |
| Milestone group | Runnable server skeleton |
| Depends on phases | none |
| Task file | `forge/tasks/tasks_phase001.json` |
| Tasks | 5 |

## Overview

Phase 1 produces the smallest possible *running* AnvilML: a Cargo workspace, the `anvilml` binary, and an axum server that binds a port and answers `GET /health`. There is no config, no database, no worker — but the binary starts and responds to HTTP, which every later phase builds on. This is deliberately the opposite of the previous layer-first approach: we get something runnable on day one and thicken it.

Every task in this phase implements **one module or one endpoint** plus its test. No task touches more than its named file(s). `cargo test` and `cargo clippy` are per-task gates; the phase as a whole is only complete when the **Runnable Proof** below passes.

## Tasks

| Task | Module / File | Summary |
|------|---------------|---------|
| P1-A1 | `src/lib.rs` | anvilml: Cargo workspace root, crate skeletons, .gitattributes |
| P1-A2 | `backend/src/main.rs` | anvilml: backend binary crate with anvilml bin name and tokio main stub |
| P1-A3 | `src/state.rs` | anvilml-server: build_router with /health handler and AppState skeleton |
| P1-A4 | `backend/src/main.rs` | anvilml: wire main.rs to bind axum server on 127.0.0.1:8488 |
| P1-A5 | `.github/workflows/ci.yml` | anvilml: CI workflow (Linux fmt+clippy+test, Windows clippy+test) |

## Task details

#### P1-A1: anvilml: Cargo workspace root, crate skeletons, .gitattributes

- **Prereqs:** P0-A4
- **Tags:** scaffold

Create workspace Cargo.toml (members backend + crates/*). 8 crate dirs (anvilml-core,-hardware,-registry,-ipc,-worker,-scheduler,-server,-openapi) each with a minimal Cargo.toml + src/lib.rs stub (anvilml-openapi gets src/main.rs instead). Declare [features] mock-hardware=[] in anvilml-hardware. Cargo.lock is committed (binary app). The rust-toolchain.toml pin and .gitattributes already exist from phase 000; do not recreate them. Verify: cargo build --workspace --features mock-hardware exits 0; rustc --version reports 1.95.0.

#### P1-A2: anvilml: backend binary crate with anvilml bin name and tokio main stub

- **Prereqs:** P1-A1
- **Tags:** scaffold

Create backend/Cargo.toml with [[bin]] name='anvilml'. Add deps: tokio (features full), anvilml-server (path). backend/src/main.rs: #[tokio::main] async fn main() that prints 'AnvilML vX.Y.Z starting' using env!(CARGO_PKG_VERSION) and exits 0. This is a placeholder to be replaced in P1-A4. cargo build --release produces target/release/anvilml (or anvilml.exe on Windows).

#### P1-A3: anvilml-server: build_router with /health handler and AppState skeleton

- **Prereqs:** P1-A1
- **Tags:** —

In anvilml-server: add axum, tower, tokio deps. Create src/state.rs: minimal AppState { start_time: Instant, version: String } (full fields added in later phases). Create src/handlers/health.rs: async fn health(State<AppState>) -> Json returning {status:'ok', version, uptime_s} where uptime_s = start_time.elapsed().as_secs(). Create src/lib.rs: pub fn build_router(state: AppState) -> axum::Router with GET /health. cargo test -p anvilml-server exits 0 with a handler unit test using axum::body.

#### P1-A4: anvilml: wire main.rs to bind axum server on 127.0.0.1:8488

- **Prereqs:** P1-A2, P1-A3
- **Tags:** —

Replace backend/src/main.rs body: build AppState{start_time:Instant::now(), version}, call anvilml_server::build_router(state), bind TcpListener on 127.0.0.1:8488, axum::serve(listener, router).await. Log 'Listening on http://127.0.0.1:8488' via println for now. No graceful shutdown yet (added phase 2). Verify: cargo run, then curl http://127.0.0.1:8488/health returns 200 with status ok. cargo build --release exits 0.

#### P1-A5: anvilml: CI workflow (Linux fmt+clippy+test, Windows clippy+test)

- **Prereqs:** P1-A4
- **Tags:** scaffold

Create .github/workflows/ci.yml. The pinned rust-toolchain.toml (1.95.0) is auto-respected by all jobs. Job rust-linux (ubuntu): cargo fmt --all --check; cargo clippy --workspace --features mock-hardware -- -D warnings; cargo test --workspace --features mock-hardware; then apt install mingw-w64 + rustup target add x86_64-pc-windows-gnu, run cargo check --target x86_64-pc-windows-gnu --workspace --features mock-hardware (fast cross-check gate). Job rust-windows (windows-latest): clippy + test --features mock-hardware (native, no fmt). Cache cargo per-OS. All jobs pass.


## Runnable Proof

Run the binary and confirm the health endpoint answers over HTTP.

```bash
cargo run
# in another terminal:
curl -s http://127.0.0.1:8488/health
```

Expected response (200):

```json
{"status":"ok","version":"0.1.0","uptime_s":3}
```

The phase is done when `curl /health` returns 200 with a JSON body containing `status`, `version`, and an increasing `uptime_s`, and CI (Linux + Windows) is green.