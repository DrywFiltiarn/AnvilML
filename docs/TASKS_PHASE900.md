# Tasks: Phase 900 — Logging Retrofit

| Field | Value |
|-------|-------|
| Phase | 900 |
| Name | Logging Retrofit |
| Project(s) | anvilml |
| Status | Approved |
| Depends on phases | 0–8 (via prereqs; see Prerequisites) |

---

## Overview

Phase 900 is a retrofit phase (see `FORGE_TASK_AUTHORING_SPEC.md §6` — phases 900–999 are reserved for retrofit, correction, and adjustment work inserted between already-executed primary phases). It is placed between Phase 008 (IPC Framing) and Phase 009 (Worker Spawn & Handshake) and must complete before Phase 009 begins.

Its sole purpose is to retrofit the logging obligations introduced by `FORGE_AGENT_RULES.md §11` across all subsystems implemented in phases 000–008. Those phases predate §11 and cannot be retroactively changed — their task JSON files are immutable once executed. Rather than relying on the §11 "fix logging in files you already touch" provision — which would scatter fixes unpredictably across future tasks — this phase performs a clean, targeted sweep of every file with a known gap, leaving the codebase fully §11-compliant before Phase 009 adds the worker and scheduler layers on top.

Seven files have identified gaps. They are arranged in a single linear chain (Group A, P900-A1 through P900-A7), ordered so that crates lower in the dependency graph are addressed first. Each task modifies exactly one file and makes no logic changes — only log call additions or corrections. All existing tests must pass without modification after each task.

Phase 009's first task (`P9-A1`) has its prereq updated from `P8-A4` to `P900-A7`, ensuring Phase 009 cannot begin until the logging retrofit is complete.

---

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | Logging retrofit | P900-A1 … P900-A7 | Add missing INFO/DEBUG/WARN/ERROR calls across seven files from phases 000–008 |

---

## Prerequisites

All tasks in phases 000 through 008 must be complete. Specifically:
- `crates/anvilml-registry/src/db.rs` must exist with `open()` and `reset_ghost_jobs()` (P5-A2, P5-A3, P7-D1)
- `crates/anvilml-registry/src/seed_loader.rs` must exist with `run()` fully implemented (P7-G2a, P7-G2b)
- `crates/anvilml-hardware/src/lib.rs` must have `detect_all_devices()` with the platform fallback chain (P4-A5, P7-F4)
- `crates/anvilml-hardware/src/device_db.rs` must have `resolve_caps_from_row()` (P7-F3, P7-G4)
- `crates/anvilml-core/src/config_load.rs` must have `load_config()` with all override layers (P2-A2)
- `crates/anvilml-registry/src/scanner.rs` must have the P7-D3 warn fixes in place
- `crates/anvilml-ipc/src/framing.rs` must have `write_frame` and `read_frame` implemented (P8-A2, P8-A3)
- `tracing` must be in `[workspace.dependencies]` (established by P7-C1)

---

## Interfaces and Contracts

| Contract document | Relevant to tasks | What must match |
|---|---|---|
| `docs/FORGE_AGENT_RULES.md §11` | All tasks | Level assignment (§11.2), mandatory INFO points (§11.3), WARN field discipline (§11.4), mandatory DEBUG points (§11.5), structured field notation (§11.6) |
| `docs/ENVIRONMENT.md §9` | P900-A1, P900-A2, P900-A6 | Required field names for DB, seed, and scanner log calls |

---

## Task Descriptions

### Group A — Logging Retrofit

#### P900-A1: anvilml-registry: retrofit INFO logging to db.rs

**Goal:** `db::open` creates the database file, runs migrations, and resets ghost jobs — three mandatory INFO log points per §11.3 — but currently logs none of them. This task adds the missing calls without changing any behaviour.

**Files to create or modify:**
- `crates/anvilml-registry/src/db.rs` — add DB-created INFO, per-migration INFO, and up-to-date INFO

**Key implementation notes:**
- Check `path.exists()` *before* passing it to `SqliteConnectOptions`. If absent: `tracing::info!(path=%path.display(), "database created")`; if present: `tracing::debug!(path=%path.display(), "database exists")`.
- After `sqlx::migrate!` returns, query `SELECT version, description FROM _sqlx_migrations WHERE success = TRUE ORDER BY installed_on` to get applied migrations. If rows are returned, log each at INFO with `migration=` and `version=` fields. If zero rows (all already applied), log `tracing::info!(migrations_applied=0, "database schema up to date")`.
- All log fields must use structured `=` notation per §11.6. No string interpolation.
- `tracing` is already a workspace dependency; no `Cargo.toml` changes needed.

**Acceptance criterion:** `cargo test -p anvilml-registry -- db` exits 0 with no regressions.

---

#### P900-A2: anvilml-registry: retrofit INFO logging to seed_loader.rs

**Goal:** The seed loader skips or applies seed files based on SHA256 comparison — two mandatory INFO log points per §11.3 — but currently emits nothing visible at the default log level.

**Files to create or modify:**
- `crates/anvilml-registry/src/seed_loader.rs` — add seed-skipped and seed-applied INFO calls

**Key implementation notes:**
- SHA256-match (skip) branch: `tracing::info!(file=%filename, status="up-to-date", "seed skipped")`.
- Execution branch (apply): `tracing::info!(file=%filename, sha256=%hex_str, "seed applied")` where `hex_str` is the hex-encoded SHA256 already computed by the existing sha2 usage.
- Both calls placed after the branch decision and before the `seed_history` upsert.
- No changes to logic, strategy selection, transaction execution, or test files.

**Acceptance criterion:** `cargo test -p anvilml-registry -- seed` exits 0 with no regressions.

---

#### P900-A3: anvilml-hardware: retrofit DEBUG fallback log to lib.rs

**Goal:** When `detect_all_devices` falls back from Vulkan to DXGI or sysfs+NVML, it does so silently. Per §11.5, the fallback path taken must be logged at DEBUG.

**Files to create or modify:**
- `crates/anvilml-hardware/src/lib.rs` — add DEBUG fallback log in the real-hardware enumeration path

**Key implementation notes:**
- Place inside the real-hardware branch (not reachable under `--features mock-hardware`), after `VulkanDetector` returns `Ok(vec![])`, before the fallback detector is invoked.
- `#[cfg(windows)]`: `tracing::debug!(fallback="dxgi", "Vulkan returned no devices; using DXGI")`.
- `#[cfg(unix)]`: `tracing::debug!(fallback="sysfs_nvml", "Vulkan returned no devices; using sysfs+NVML")`.
- Emit only when actually falling back — not on every run.
- No changes to mock path, test files, or detection logic.

**Acceptance criterion:** `cargo test -p anvilml-hardware --features mock-hardware` exits 0. `cargo check --bin anvilml` and `cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu` exit 0.

---

#### P900-A4: anvilml-hardware: retrofit DEBUG caps resolution log to device_db.rs

**Goal:** `resolve_caps_from_row` makes a binary decision — DeviceTable hit or Fallback — for every detected device, silently on the hit path. Per §11.1 the decision must be observable at DEBUG.

**Files to create or modify:**
- `crates/anvilml-hardware/src/device_db.rs` — add DEBUG resolution log on both paths of `resolve_caps_from_row`

**Key implementation notes:**
- Hit path: `tracing::debug!(vendor_id=%format_args!("0x{:04X}", dev.pci_vendor_id), device_id=%format_args!("0x{:04X}", dev.pci_device_id), name=%dev.name, source="DeviceTable", "caps resolved")`.
- Miss path: same fields with `source="Fallback"`. The existing `tracing::warn!` on miss is unchanged.
- No changes to capability assignment logic or tests.

**Acceptance criterion:** `cargo test -p anvilml-hardware` exits 0 with no regressions.

---

#### P900-A5: anvilml-core: retrofit DEBUG resolved config log to config_load.rs

**Goal:** `load_config` applies four override layers silently. Per §11.1, the final resolved values for key fields must be observable at DEBUG so an operator can verify which config source was authoritative.

**Files to create or modify:**
- `crates/anvilml-core/src/config_load.rs` — add one DEBUG call after the final `ServerConfig` is assembled

**Key implementation notes:**
- Place after the overrides block is applied and before the function returns.
- Fields: `host=%cfg.host`, `port=cfg.port`, `db_path=%cfg.db_path.display()`, `frontend_mode=?cfg.frontend.mode`.
- Do not log `venv_path` or any field that could contain a user-specified path treated as sensitive.
- No logic changes.

**Acceptance criterion:** `cargo test -p anvilml-core -- config_load` exits 0 with no regressions.

---

#### P900-A6: anvilml-registry: retrofit WARN discipline and DEBUG per-file log to scanner.rs

**Goal:** Two distinct gaps. First, the P7-D3 WARN calls do not distinguish "file not found" (routine) from unexpected OS errors — per §11.4, `error=` must be omitted when it is redundant. Second, per §11.5 the scanner must log each examined file at DEBUG, which is entirely absent.

**Files to create or modify:**
- `crates/anvilml-registry/src/scanner.rs` — split WARN on error kind; add DEBUG per examined file

**Key implementation notes:**
- **WARN discipline (three sites):** For `walkdir::Error`, check `.io_error().map(|e| e.kind()) == Some(io::ErrorKind::NotFound)`; for direct `io::Error`, check `e.kind() == io::ErrorKind::NotFound`. NotFound: emit `tracing::warn!(path=%path.display(), "scanner: skipping missing path")` without `error=`. All other error kinds: retain `error=%e`.
- **DEBUG per file:** After a `ModelMeta` is computed and before it is pushed: `tracing::debug!(path=%entry.path().display(), id=%meta.id, "scanner: accepted")`. At each `continue` site: `tracing::debug!(path=%entry.path().display(), reason=%reason_str, "scanner: skipped")` where `reason_str` is a short static string (e.g. `"extension not matched"`, `"metadata error"`).
- No changes to test assertions.

**Acceptance criterion:** `cargo test -p anvilml-registry -- scanner` exits 0 with no regressions.

---

#### P900-A7: anvilml-ipc: retrofit WARN/ERROR logging to framing.rs error paths

**Goal:** Framing errors are symptoms of worker bugs or protocol violations. Every error path in `framing.rs` currently returns `Err(...)` silently. Per §11.1, these must be logged before returning.

**Files to create or modify:**
- `crates/anvilml-ipc/src/framing.rs` — add WARN on PayloadTooLarge, ERROR on deserialize and write failures
- `crates/anvilml-ipc/Cargo.toml` — add `tracing = { workspace = true }`

**Key implementation notes:**
- `read_frame` PayloadTooLarge: `tracing::warn!(payload_mib=payload_len/1024/1024, limit_mib=max_mib, "IPC frame rejected: payload too large")` before returning the error.
- `read_frame` deserialize failure: `tracing::error!(error=%e, "IPC frame deserialize failed")`.
- `write_frame` failure: `tracing::error!(error=%e, "IPC frame write failed")`.
- Per-frame DEBUG (every frame sent/received) is intentionally NOT added here — that belongs in `managed.rs` (Phase 009, P9-A4) where worker context is available.
- `tracing` is in `[workspace.dependencies]` (P7-C1); the `Cargo.toml` change adds the dep to this crate only.

**Acceptance criterion:** `cargo test -p anvilml-ipc` exits 0. `cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu` exits 0.

---

## Phase Acceptance Criteria

```
cargo test --workspace --features mock-hardware
cargo clippy --workspace --features mock-hardware -- -D warnings
cargo clippy --bin anvilml -- -D warnings
cargo check --bin anvilml
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
cargo check --bin anvilml --target x86_64-pc-windows-gnu
```

All six commands must exit 0. No new tests are required by this phase — all existing tests must continue to pass without modification.

---

## Known Constraints and Gotchas

- **No logic changes in any task.** Every task is logging-only. If a task requires restructuring a function to add a log call, document it as a blocker rather than restructuring in-scope.
- **P900-A1 queries `_sqlx_migrations` directly.** The query `SELECT version, description FROM _sqlx_migrations WHERE success = TRUE ORDER BY installed_on` is stable across sqlx versions in use. If the column names differ in the installed sqlx version, consult `mcp-rust-docs` for the correct schema before writing the query.
- **P900-A3 log calls are inside `#[cfg(...)]` blocks.** The DEBUG calls must be placed inside the same `#[cfg(windows)]` and `#[cfg(unix)]` guards as the fallback detectors they describe. Verify both platforms compile using the four `cargo check` commands in the Phase Acceptance Criteria.
- **P900-A6 WARN split requires matching on error kind.** `walkdir::Error` wraps an `Option<io::Error>` — use `.io_error().map(|e| e.kind())`. For direct `io::Error` values use `.kind()` directly.
- **P900-A7 adds `tracing` to `anvilml-ipc` for the first time.** Verify `tracing` is in `[workspace.dependencies]` before writing the `Cargo.toml` change. The workspace version does not change; only the per-crate dependency entry is added.
- **Phase 009 prereq update required.** `P9-A1` in `tasks_phase009.json` currently prereqs `P8-A4`. Update it to `["P900-A7"]` so Phase 009 cannot begin until this retrofit is complete. No other Phase 009 task requires a prereq change.