# Plan Report: P4-A6

| Field | Value |
|-------|-------|
| Task ID | P4-A6 |
| Phase | 004 — Hardware Detection |
| Description | anvilml: detect hardware at startup and serve GET /v1/system |
| Depends on | P4-A5 (detect_all_devices orchestrator) |
| Project | anvilml |
| Planned at | 2026-06-03T14:38:00Z |
| Attempt | 1 |

## Objective

Wire the existing `anvilml-hardware::detect_all_devices()` into the server startup path so that hardware information is detected once at boot, stored in `AppState`, and exposed via a new `GET /v1/system` endpoint. Additionally add a `--print-hardware` CLI subcommand that detects hardware, prints a formatted table to stdout, and exits 0 without binding any network socket.

## Scope

### In Scope
- Add `anvilml-hardware` as a direct dependency of the `backend` crate (currently only reachable transitively through `anvilml-server`).
- Forward the `mock-hardware` feature flag from `backend` → `anvilml-server` → `anvilml-hardware`.
- Extend `AppState` with `hardware: Arc<RwLock<HardwareInfo>>` and a getter method.
- Modify `backend/src/main.rs` to call `detect_all_devices(&cfg)` after config load, log each device's key fields (name, ids, vram, enumeration_source, capabilities_source), and store the result in `AppState`.
- Add `get_system()` handler in `anvilml-server/src/handlers/system.rs` returning `Json<HardwareInfo>` from `AppState::hardware`.
- Wire `GET /v1/system` route in `anvilml-server/src/lib.rs` alongside existing `/health` and `/v1/system/env`.
- Add `--print-hardware` CLI flag to `backend/src/cli.rs` (`Args`) that, when present, calls `detect_all_devices`, prints a human-readable table to stdout, and exits 0 without starting the server or binding.
- Add integration test for `GET /v1/system` in `anvilml-server` (same pattern as existing `/health` and `/v1/system/env` tests).

### Out of Scope
- Modifying any detector implementation files (`vulkan.rs`, `mock.rs`, `cpu.rs`, etc.) — those belong to P4-A1 through P4-A5.
- Adding WebSocket events for hardware changes.
- Modifying the OpenAPI generation binary or manually editing `openapi.json`.
- Any worker-side hardware reporting (that is Phase 9).
- Retrofit tasks P4-B1 and P4-B2.

## Approach

### Step 1 — Add `anvilml-hardware` to backend crate
Edit `backend/Cargo.toml`:
- Add `anvilml-hardware = { path = "../crates/anvilml-hardware" }` to `[dependencies]`.
- Update the `[features]` section to forward: `mock-hardware = ["anvilml-server/mock-hardware", "anvilml-hardware/mock-hardware"]`.

### Step 2 — Extend `AppState` with hardware state
Edit `crates/anvilml-server/src/state.rs`:
- Add `use anvilml_core::HardwareInfo;` and `use std::sync::{Arc, RwLock};` (the latter is already imported).
- Add field: `hardware: Arc<RwLock<HardwareInfo>>`.
- Update `new()` to accept a `HardwareInfo` parameter (or add a separate `new_with_hardware()` constructor). The existing `new()` keeps the current signature for backward compatibility with test code; add `pub fn new_with_hardware(version: impl Into<String>, hardware: HardwareInfo) -> Self`.
- Add getter: `pub fn hardware(&self) -> HardwareInfo { self.hardware.read().unwrap().clone() }`.

### Step 3 — Wire hardware detection into `main.rs`
Edit `backend/src/main.rs`:
- After `let cfg = load_config(...)` and before `AppState::new(...)`, call:
  ```rust
  let hw_info = anvilml_hardware::detect_all_devices(&cfg).expect("hardware detection failed");
  ```
- Log each device:
  ```rust
  for dev in &hw_info.gpus {
      tracing::info!(
          device.name = %dev.name,
          index = dev.index,
          device_type = ?dev.device_type,
          vram_total_mib = dev.vram_total_mib,
          enumeration_source = ?dev.enumeration_source,
          capabilities_source = ?dev.capabilities_source,
      );
  }
  ```
- Replace `AppState::new(env!("CARGO_PKG_VERSION"))` with `AppState::new_with_hardware(env!("CARGO_PKG_VERSION"), hw_info)`.

### Step 4 — Add `get_system` handler
Edit `crates/anvilml-server/src/handlers/system.rs`:
- Import `anvilml_core::HardwareInfo`.
- Add function:
  ```rust
  pub async fn get_system(
      State(state): State<Arc<crate::state::AppState>>,
  ) -> (StatusCode, Json<HardwareInfo>) {
      let info = state.hardware();
      (StatusCode::OK, Json(info))
  }
  ```

### Step 5 — Wire `GET /v1/system` route
Edit `crates/anvilml-server/src/lib.rs`:
- Add `.route("/v1/system", get(handlers::system::get_system))` to the router in `build_router()`.

### Step 6 — Add `--print-hardware` CLI subcommand
Edit `backend/src/cli.rs`:
- Add `pub print_hardware: bool` field to `Args` with `#[arg(long)]` and default `false`.
- Edit `backend/src/main.rs`: wrap the server startup path in a conditional. When `args.print_hardware` is true, call `detect_all_devices(&cfg)`, format and print a table to stdout (device index, name, device_type, vram_total_mib, enumeration_source, capabilities_source), then `std::process::exit(0)` before any server binding code.

### Step 7 — Add integration test
Edit `crates/anvilml-server/src/lib.rs` (add to existing `#[cfg(test)] mod tests`):
- Test that `GET /v1/system` returns 200 with a valid `HardwareInfo` JSON containing at least one GPU device and populated host fields.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Edit | `backend/Cargo.toml` | Add `anvilml-hardware` dependency, forward `mock-hardware` feature |
| Edit | `backend/src/main.rs` | Call `detect_all_devices`, log devices, pass to `AppState::new_with_hardware`, handle `--print-hardware` |
| Edit | `backend/src/cli.rs` | Add `print_hardware: bool` CLI flag |
| Edit | `crates/anvilml-server/Cargo.toml` | No changes needed (already has `anvilml-hardware` dep and feature forwarding) |
| Edit | `crates/anvilml-server/src/state.rs` | Add `hardware` field, constructor variant, getter |
| Edit | `crates/anvilml-server/src/handlers/system.rs` | Add `get_system` handler |
| Edit | `crates/anvilml-server/src/lib.rs` | Wire `/v1/system` route, add integration test |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `crates/anvilml-server/src/lib.rs` (existing) | `health_returns_200` | Unchanged — baseline regression guard |
| `crates/anvilml-server/src/lib.rs` (existing) | `env_returns_200_with_stub_report` | Unchanged — baseline regression guard |
| `crates/anvilml-server/src/lib.rs` (new) | `system_returns_200_with_hardware_info` | GET /v1/system returns 200, JSON has `host`, `gpus` non-empty, `inference_caps` present |

## CI Impact

No CI workflow changes required. The existing CI matrix (`rust` job: `cargo test … --features mock-hardware`) will automatically exercise the new code paths since `anvilml-server` already declares `mock-hardware = ["anvilml-hardware/mock-hardware", ...]`. The `openapi-diff` job may produce a diff in `backend/openapi.json` if utoipa schemas are auto-generated — this is expected and the generated file should be committed by The Forge.

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| `detect_all_devices()` panics on systems without Vulkan and no mock feature (not compiled with `mock-hardware`). | The function already returns `Result<HardwareInfo, AnvilError>` and has a test (`detect_all_devices_never_errs`) proving it never returns Err. CI always uses `--features mock-hardware`. Safe. |
| `--print-hardware` exits before any tracing subscriber is fully initialised, so log output may be lost. | Logging happens *before* exit only on the normal server path; `--print-hardware` prints to stdout via `println!`, not tracing. No risk. |
| Adding `anvilml-hardware` as a direct backend dependency increases build time and compile-time feature surface. | Minimal — the crate is already transitively compiled through `anvilml-server`. The only additional cost is one extra `#[cfg]` gate for `mock-hardware`, which is already in place. |
| `AppState::new()` signature change breaks existing tests that construct `AppState::new()`. | Provide both `new()` (backward-compatible, with empty/default HardwareInfo) and `new_with_hardware()`. The test module uses `AppState::new("0.1.0")` which still works. |

## Acceptance Criteria

- [ ] `cargo check --workspace --features mock-hardware` passes (Linux + x86_64-pc-windows-gnu cross-check)
- [ ] `cargo test -p anvilml-server --features mock-hardware` passes (including new `/v1/system` integration test)
- [ ] `ANVILML_MOCK_DEVICE_TYPE=cuda cargo run --features mock-hardware` starts server; `curl -s http://127.0.0.1:8488/v1/system` returns 200 with `gpus[0].device_type == "cuda"` and populated host block
- [ ] `ANVILML_MOCK_DEVICE_TYPE=cuda cargo run --features mock-hardware -- --print-hardware` prints hardware table to stdout and exits 0 without binding
- [ ] `cargo clippy --workspace --features mock-hardware -D warnings` passes
