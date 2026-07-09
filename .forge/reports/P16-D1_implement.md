# Implementation Report: P16-D1

| Field         | Value                                                            |
|---------------|-------------------------------------------------------------------|
| Task ID       | P16-D1                                                              |
| Phase         | 016 — Live Events                                                    |
| Description   | anvilml-server: stats_tick.rs background SystemStats every 5s       |
| Implemented   | 2026-07-09T20:40:00Z                                                 |
| Status        | COMPLETE — gate verification pending Dryw's local toolchain (see Blockers) |

## Summary

Created `crates/anvilml-server/src/ws/stats_tick.rs` implementing `spawn_stats_tick()`: a
background task holding one `sysinfo::System` for its whole lifetime, ticking on an
injected `Duration` via `tokio::time::interval` (`MissedTickBehavior::Delay`), publishing
`WsEvent::SystemStats { cpu_pct, ram_used_mib, workers }` to the shared `EventBroadcaster`
every tick. Added `WorkerPool::list() -> Vec<WorkerInfo>` (`crates/anvilml-worker/src/pool.rs`)
since no method returning worker snapshots existed anywhere in the codebase — confirmed by
inspection, matching the task context's own "added here if absent" clause. `pid` and
`current_job_id` are left `None` in every `WorkerInfo` this produces, documented explicitly
as a real gap rather than guessed at, since neither is tracked at this layer. Wired
`spawn_stats_tick(..., Duration::from_secs(5))` into `backend/src/main.rs` alongside the
dispatch and event loops, including the corresponding abort+await in the graceful shutdown
sequence — it holds its own `Arc<WorkerPool>` clone, same as `event_loop_handle`, and must
be released before `Arc::try_unwrap(workers)` can succeed. Added 5 tests in
`crates/anvilml-server/tests/stats_tick_tests.rs`, all using millisecond-scale injected
intervals; added the corresponding `docs/TESTS.md` entries; bumped the patch version of
every manifest whose source changed (`anvilml-server`, `anvilml-worker`, `backend`).

`ws/handler.rs` (`P16-C1`/`P16-C2`) was deliberately not touched — its own per-connection
initial `SystemStats` frame remains the placeholder that task established; this task's own
Files list never included it, and the two are genuinely separate concerns (one fires once
per new connection, this one fires on a fixed cadence to every already-subscribed client).

## Resolved Dependencies

| Type  | Name    | Version resolved | Source                                                         |
|-------|---------|--------------------|--------------------------------------------------------------------|
| crate | sysinfo | 0.39.6 (latest stable) | crates.io sparse index (`index.crates.io/sy/si/sysinfo`) + downloaded source |

No MCP tool was available in this session — same network-restricted sandbox as the prior
`P16-C1` session. `System::new_all()`, `refresh_cpu_usage()`, `refresh_memory()`,
`global_cpu_usage() -> f32`, and `used_memory() -> u64` were confirmed against the actual
downloaded `sysinfo-0.39.6` source rather than training-data recall, including the
documented `MINIMUM_CPU_UPDATE_INTERVAL` (200ms) requirement for a meaningful CPU-usage
delta, which shaped the decision to hold one `System` across the task's lifetime rather than
recreate it per tick.

## Files Changed

| Action | Path | Description |
|--------|------|--------------|
| CREATE | `crates/anvilml-server/src/ws/stats_tick.rs` | `spawn_stats_tick()` |
| CREATE | `crates/anvilml-server/tests/stats_tick_tests.rs` | 5 integration tests |
| MODIFY | `crates/anvilml-server/src/ws/mod.rs` | Declares `stats_tick`, re-exports `spawn_stats_tick` |
| MODIFY | `crates/anvilml-server/Cargo.toml` | `sysinfo = "0.39.6"`; `tokio` features `["sync","rt","time"]`; `anvilml-worker/test-utils` dev-dep; `0.1.14` → `0.1.15` |
| MODIFY | `crates/anvilml-worker/src/pool.rs` | Adds `WorkerPool::list()` |
| MODIFY | `crates/anvilml-worker/Cargo.toml` | `0.1.33` → `0.1.34` |
| MODIFY | `backend/src/main.rs` | Spawns `spawn_stats_tick()`; aborts/awaits it during shutdown |
| MODIFY | `backend/Cargo.toml` | `0.1.14` → `0.1.15` |
| MODIFY | `docs/TESTS.md` | 5 new catalogue entries |

## Commit Log

```
 backend/Cargo.toml                              |  2 +-
 backend/src/main.rs                             | 23 +++++++++++
 crates/anvilml-server/Cargo.toml                | 10 ++++-
 crates/anvilml-server/src/ws/mod.rs             |  9 +++--
 crates/anvilml-server/src/ws/stats_tick.rs      | 82 +++++++++++++++++++++++++++++++++++++++
 crates/anvilml-server/tests/stats_tick_tests.rs | 253 +++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
 crates/anvilml-worker/Cargo.toml                |  2 +-
 crates/anvilml-worker/src/pool.rs               | 33 +++++++++++++++
 docs/TESTS.md                                   | 55 ++++++++++++++++++++++++
 9 files changed, ~460 insertions(+), ~4 deletions(-)
```
(Approximate — `git diff --stat` against your working tree will confirm exactly, since this
session's clone is separate from yours.)

## Test Results

Not run in this session — no Rust toolchain available in this sandbox (same constraint as
the `P16-C1` implementation report). Static verification performed instead:

1. `git apply --check` succeeded against a fresh independent clone of `DrywFiltiarn/AnvilML`
   at HEAD `fef3f7451f3b42d221a277c026c47be6d8427165` (the same HEAD `P16-C2`'s completion
   left the repo at — confirmed no drift occurred during this session).
2. Every `sysinfo` API call was checked against the actual downloaded `sysinfo-0.39.6`
   source (see Resolved Dependencies), not assumed from memory.
3. `WorkerInfo`, `WorkerStatus`, `DeviceType`, and `GpuDevice`'s derives (`Debug`, `Clone`,
   `Copy` where applicable, `PartialEq`, `Eq`) were confirmed against their actual
   definitions in `anvilml-core/src/types/` before being used in `assert_eq!`/`matches!` in
   the new test file.
4. `WorkerPool::set_up_test_workers()`'s `test-utils`-gating and exact signature were
   confirmed against the live `crates/anvilml-worker/src/pool.rs` source, and the
   `anvilml-worker` dev-dependency override needed to reach it was copied from
   `crates/anvilml-scheduler/Cargo.toml`'s own existing, working precedent for the identical
   need.

**Action required from Dryw:** run
`cargo test -p anvilml-server --features mock-hardware --test stats_tick_tests` locally
(and the full phase gate suite before handing back to The Forge for `P16-E1`) and report
verbatim output if anything fails.

## Format Gate

Not run in this session — no Rust toolchain available. New/modified files follow this
crate's existing formatting and doc-comment conventions (matching `event_loop.rs` and
`keepalive.rs`'s style) but have not been passed through `rustfmt` itself. Note: the new
`use anvilml_server::ws::spawn_stats_tick;` line in `backend/src/main.rs` may get
reordered relative to the adjacent `use anvilml_server::{AppState, build_router};` line by
`cargo fmt`'s default import sorting — cosmetic only, not a compile risk.

## Platform Cross-Check

Not run in this session — no Rust toolchain available. `sysinfo`'s default feature set
(confirmed in Resolved Dependencies) covers both Linux and Windows without
platform-conditional code in `stats_tick.rs` itself, so no cross-check-specific risk is
expected — but this has not been mechanically confirmed by actually running the
cross-check commands.

## Project Gates

Not run in this session — no Rust toolchain available. No `ServerConfig` fields,
`#[utoipa::path]`-annotated handler signatures, node types, or node
`execute()`/arch-module functions were touched.

## Public API Delta

```
+pub mod stats_tick;                                                   (ws/mod.rs)
+pub use stats_tick::spawn_stats_tick;                                 (ws/mod.rs)
+pub fn spawn_stats_tick(
+    broadcaster: Arc<EventBroadcaster>,
+    workers: Arc<WorkerPool>,
+    interval: Duration,
+) -> tokio::task::JoinHandle<()>;                                     (ws/stats_tick.rs)
+pub async fn list(&self) -> Vec<WorkerInfo>;                          (anvilml-worker: pool.rs, impl WorkerPool)
```

## Deviations from Plan

- **None from the approved plan's In Scope, Files Affected, or Public API Surface
  sections.** `WorkerPool::list()`'s exact field-population choice (`pid`/`current_job_id`
  always `None`) was already flagged as a known gap in the plan's own Existing Codebase
  Assessment and Approach sections, not discovered mid-implementation.
- **No MCP tooling was available in this session** (see Resolved Dependencies) — the
  crates.io sparse index and downloaded crate source were used as the substitute
  live-version source, per `FORGE_AGENT_RULES.md §6.4`'s fallback guidance.
- **Gate commands (test/format/lint/cross-check) were not executed by this session** — no
  Rust toolchain is available in this sandbox. Flagged explicitly, as in the prior `P16-C1`
  report, rather than fabricating verbatim output for sections that require it.
- **`Cargo.lock` was not regenerated** — adding `sysinfo` and the `anvilml-worker`
  `test-utils` dev-dependency edge will change `Cargo.lock` on Dryw's first local build;
  this is expected, ordinary lockfile drift from a normal `cargo build`/`cargo test`, not a
  defect in the patch.

## Blockers

Same as `P16-C1`'s implementation report: no Rust toolchain (`cargo`, `rustc`) is available
in this sandbox. `## Test Results`, `## Format Gate`, `## Platform Cross-Check`, and
`## Project Gates` above contain the static source-level verification actually performed,
not genuine command output. Dryw must run
`cargo test -p anvilml-server --features mock-hardware --test stats_tick_tests` (and the
full phase gate suite) locally before handing back to The Forge for `P16-E1`; if anything
fails, the verbatim output should be pasted back so this report — or a corrective one — can
be finalized accurately.