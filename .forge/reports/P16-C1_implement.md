# Implementation Report: P16-C1

| Field         | Value                                                            |
|---------------|-------------------------------------------------------------------|
| Task ID       | P16-C1                                                             |
| Phase         | 016 — Live Events                                                   |
| Description   | anvilml-server: ws_handler skeleton + initial SystemStats frame     |
| Implemented   | 2026-07-09T16:45:00Z                                                |
| Status        | COMPLETE — gate verification pending Dryw's local toolchain (see Blockers) |

## Summary

Created `crates/anvilml-server/src/ws/mod.rs` and `ws/handler.rs`, implementing
`ws_handler()` (delegates to `WebSocketUpgrade::on_upgrade()`) and `handle_socket()`
(subscribes to `state.broadcaster`, sends one placeholder/zero-valued `WsEvent::SystemStats`
JSON text frame, returns). Registered `GET /v1/events` in `build_router()`. Enabled axum's
non-default `ws` feature, without which `axum::extract::ws` does not resolve at all — this
was missed in the first patch and caught by Dryw's own `cargo clippy --workspace` run
(`error[E0432]: unresolved import 'axum::extract::ws'`); corrected in a follow-up patch,
re-verified by re-applying to a fresh independent clone. Added 4 real-socket integration
tests in `crates/anvilml-server/tests/handler_tests.rs`, using `TcpListener` +
`axum::serve()` + `tokio_tungstenite::connect_async()` rather than this crate's usual
`tower::ServiceExt::oneshot()` pattern, because a WebSocket upgrade needs a genuine
bidirectional socket that `oneshot()`'s in-process call does not provide. Added the
corresponding `docs/TESTS.md` catalogue entries.

This work was taken over from a prior OpenCode/Qwen3.6 35B A3B session that stalled in a
repeating thinking loop while re-deriving (correctly, but without ever acting on) the same
conclusion about needing `tokio-tungstenite` for the test client, and exhausted its budget
without writing any code.

## Resolved Dependencies

| Type  | Name              | Version resolved | Source                                              |
|-------|-------------------|--------------------|-------------------------------------------------------|
| crate | tokio-tungstenite | 0.29.0             | crates.io sparse index (`index.crates.io/to/ki/tokio-tungstenite`) |
| crate | futures-util      | 0.3 (0.3.32 pinned transitively) | `Cargo.lock` (already present as a transitive dependency) |
| crate | axum `ws` feature | 0.8.9 (existing pin, feature flag added) | downloaded `axum-0.8.9` crate source (`Cargo.toml` feature table) |

No MCP tool was available in this session — a network-restricted sandbox with no
`rust-docs` MCP configured. Per `FORGE_AGENT_RULES.md §6.4`'s fallback, versions and API
shapes were resolved against the live crates.io sparse index and downloaded crate source
tarballs (`axum-0.8.9`, `tungstenite-0.28.0`, `tokio-tungstenite-0.29.0`) instead of from
training-data memory.

## Files Changed

| Action | Path | Description |
|--------|------|--------------|
| CREATE | `crates/anvilml-server/src/ws/mod.rs` | Module declaration, re-exports `ws_handler` |
| CREATE | `crates/anvilml-server/src/ws/handler.rs` | `ws_handler()` + `handle_socket()` (initial frame only — forward loop is `P16-C2`) |
| MODIFY | `crates/anvilml-server/src/lib.rs` | Adds `pub mod ws;`, registers `GET /v1/events` before the CORS layer |
| MODIFY | `crates/anvilml-server/Cargo.toml` | `axum = { version = "0.8.9", features = ["ws"] }`; adds `tokio-tungstenite = "0.29.0"` and `futures-util = "0.3"` dev-deps |
| CREATE | `crates/anvilml-server/tests/handler_tests.rs` | 4 real-socket integration tests |
| MODIFY | `docs/TESTS.md` | 4 new catalogue entries |

## Commit Log

```
 crates/anvilml-server/Cargo.toml          |  4 ++
 crates/anvilml-server/src/lib.rs          |  6 +++
 crates/anvilml-server/src/ws/handler.rs   | 66 +++++++++++++++++++++++++++++
 crates/anvilml-server/src/ws/mod.rs       | 10 +++++
 crates/anvilml-server/tests/handler_tests.rs | 213 ++++++++++++++++++++++++++
 docs/TESTS.md                             | 48 ++++++++++++++++++++
 6 files changed, 347 insertions(+)
```
(Line counts from the delivered patch's own diff; `git diff --stat` against your working
tree will confirm exactly, since this session's clone is separate from yours.)

## Test Results

Not run in this session — see `## Blockers`. Staged verification steps taken instead:

1. `git apply --check` succeeded against two independent fresh clones of
   `DrywFiltiarn/AnvilML` at the same HEAD commit, both before and after the `ws`-feature
   correction.
2. `axum::extract::ws::Message::Text(Utf8Bytes)`, `Message::text<S: Into<Utf8Bytes>>`, and
   the `ws` feature's gate were confirmed by inspecting the actual downloaded `axum-0.8.9`
   source, not assumed.
3. `tokio_tungstenite::connect_async<R: IntoClientRequest>` and the default-feature set
   (`connect` + `handshake`, no TLS) were confirmed against the downloaded
   `tokio-tungstenite-0.29.0` source.
4. Dryw's own `cargo clippy --workspace --features mock-hardware -- -D warnings` run against
   the first patch surfaced the missing `ws` feature (`E0432`) and its downstream `E0277`
   consequence; both are resolved by the follow-up patch's `Cargo.toml` change.

**Action required from Dryw:** run
`cargo test -p anvilml-server --test handler_tests` locally and report the verbatim output
if any test fails — none should, given the fixes above, but this report cannot claim a
verbatim passing run it did not itself produce.

## Format Gate

Not run in this session — no Rust toolchain available in this sandbox. Run
`cargo fmt --all -- --check` locally before staging; the new files follow this crate's
existing formatting conventions (4-space indent, doc-comment style matching
`handlers/nodes.rs` and `handlers/health.rs`) but have not been passed through `rustfmt`
itself.

## Platform Cross-Check

Not run in this session — no Rust toolchain available. No platform-specific code was
introduced (no `#[cfg(unix)]`/`#[cfg(windows)]`), so no cross-check-specific risk is
expected, but this is not a substitute for actually running
`docs/ENVIRONMENT.md`'s defined cross-check commands.

## Project Gates

Not run in this session — no Rust toolchain available. No `ServerConfig` fields, no
`#[utoipa::path]`-annotated handler signatures, no node types, and no node
`execute()`/arch-module functions were touched, so Gates 1–4 (per prior reports'
convention) should not trigger — but this has not been mechanically confirmed by running
the gate commands themselves.

## Public API Delta

```
+pub mod ws;                                                          (lib.rs)
+pub mod handler;                                                     (ws/mod.rs)
+pub use handler::ws_handler;                                         (ws/mod.rs)
+pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response;
                                                                        (ws/handler.rs)
```
`handle_socket()` is a private `async fn` — not part of the public surface.
`build_router()`'s own signature is unchanged; only its internal route table gained one entry.

## Deviations from Plan

- **`axum`'s `ws` feature was missing from the first patch delivered.** The plan's own
  Approach step 4 correctly identifies this requirement, but the very first patch sent to
  Dryw omitted the `Cargo.toml` change before this plan report was written up — Dryw's
  `cargo clippy` run caught it (`E0432: unresolved import 'axum::extract::ws'`). Corrected
  immediately; both the plan and this report now reflect the corrected `Cargo.toml`. Flagging
  this explicitly rather than presenting the corrected patch as if it were right the first
  time.
- **No MCP tooling was available in this session** (see Resolved Dependencies) — the
  crates.io sparse index and downloaded crate source tarballs were used as the substitute
  live-version source, per `FORGE_AGENT_RULES.md §6.4`'s fallback guidance, rather than
  training-data recall.
- **Gate commands (test/format/lint/cross-check) were not executed by this session** — no
  Rust toolchain is available in this sandbox. This is a deviation from
  `agents/forge-act.md`'s normal ACT session contract, which assumes local build access.
  Flagged explicitly rather than fabricating verbatim output for sections that require it.

## Blockers

No Rust toolchain (`cargo`, `rustc`) is available in the sandbox this session ran in —
network access permits fetching crate sources and the sparse index (used for dependency
resolution), but not installing `rustup`/`cargo`. As a result, `## Test Results`,
`## Format Gate`, `## Platform Cross-Check`, and `## Project Gates` above could not be
populated with genuine verbatim command output in this session, only with the static
source-level verification steps actually performed. Dryw must run
`cargo test -p anvilml-server --test handler_tests` (and the full phase gate suite) locally
to close this out; if anything fails, the verbatim output should be pasted back so this
report — or a corrective one — can be finalized accurately.