# Tasks: Phase 003 — Core Domain Types

| Field | Value |
|-------|-------|
| Phase | 003 |
| Name | Core Domain Types |
| Project | anvilml |
| Status | Approved |
| Depends on phases | 2 |

## Overview

Phase 003 completes `anvilml-core` by defining all domain types used across the entire system: jobs, models, artifacts, hardware, workers, nodes, and WebSocket events. These types are the shared vocabulary of the project — every subsequent crate imports them. Getting them right here means no breaking changes downstream.

The types follow the specifications in `ANVILML_DESIGN.md §5` exactly: struct field names, enum variant names, and serde attribute names must match the OpenAPI contract. The `AnvilError` enum is also completed in this phase; it will be used by every other crate for error propagation.

Phase 003 also introduces the `config_reference` integration test that will guard against config schema drift for the remainder of the project, and a stub `/v1/system/env` endpoint that confirms the server can respond to the new API routes even before real data is available.

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | anvilml-core types | P3-A1 … P3-A5 | Job, Model, Artifact, Hardware, Worker + Node types |
| B | anvilml-core error | P3-B1 | `AnvilError` enum complete with `IntoResponse` |
| C | anvilml-server | P3-C1 | Stub `/v1/system/env` endpoint |
| D | backend tests | P3-D1 | `config_reference` drift guard integration test |

## Prerequisites

Phase 002 complete: `ServerConfig` and config loading exist. `anvilml-core/src/lib.rs` declared.

## Interfaces and Contracts

| Contract document | Relevant tasks | What must match |
|-------------------|---------------|-----------------|
| `ANVILML_DESIGN.md §5.3` | P3-A1 | `Job`, `JobStatus`, `JobSettings` field names |
| `ANVILML_DESIGN.md §5.4` | P3-A2 | `ModelMeta`, `ModelKind`, `ModelDtype`, `ModelFormat` |
| `ANVILML_DESIGN.md §5.5` | P3-A3 | `HardwareInfo`, `GpuDevice`, `InferenceCaps`, `SlotType` |
| `ANVILML_DESIGN.md §5.6` | P3-A4 | `NodeTypeDescriptor`, `SlotDescriptor`, `SlotType` |
| `ANVILML_DESIGN.md §5.7` | P3-A4 | `WorkerInfo`, `WorkerStatus`, `EnvReport`, `ProvisioningState` |
| `ANVILML_DESIGN.md §5.8` | P3-A5 | `WsEvent` variants and serde tag format |

## Task Descriptions

### Group A — anvilml-core types

#### P3-A1: anvilml-core: job types (Job, JobStatus, JobSettings)

**Goal:** Create `crates/anvilml-core/src/types/job.rs` with `Job`, `JobStatus`, `JobSettings`, `SubmitJobRequest`, `SubmitJobResponse` per `ANVILML_DESIGN.md §5.3`.

**Acceptance criterion:** `cargo test -p anvilml-core -- types::job` exits 0 with ≥ 3 tests (JSON roundtrip, default impl, status variants).

#### P3-A2: anvilml-core: model and artifact types

**Goal:** Create `types/model.rs` (`ModelMeta`, `ModelKind`, `ModelDtype`, `ModelFormat`) and `types/artifact.rs` (`ArtifactMeta`) per `ANVILML_DESIGN.md §5.4`.

**Acceptance criterion:** `cargo test -p anvilml-core -- types::model` and `types::artifact` exit 0 with ≥ 3 tests each.

#### P3-A3: anvilml-core: hardware types

**Goal:** Create `types/hardware.rs` with `HardwareInfo`, `GpuDevice`, `DeviceType`, `HostInfo`, `InferenceCaps`, `EnumerationSource`, `CapabilitySource` per `ANVILML_DESIGN.md §5.5`.

**Acceptance criterion:** `cargo test -p anvilml-core -- types::hardware` exits 0 with ≥ 4 tests.

#### P3-A4: anvilml-core: node and worker types

**Goal:** Create `types/node.rs` (`NodeTypeDescriptor`, `SlotDescriptor`, `SlotType`) and `types/worker.rs` (`WorkerInfo`, `WorkerStatus`, `EnvReport`, `ProvisioningState`) per `ANVILML_DESIGN.md §5.6–5.7`.

**Acceptance criterion:** `cargo test -p anvilml-core -- types::node` and `types::worker` exit 0 with ≥ 3 tests each.

#### P3-A5: anvilml-core: WsEvent enum

**Goal:** Create `types/events.rs` with `WsEvent` and all sub-event variants per `ANVILML_DESIGN.md §5.8`. Use `#[serde(tag = "type", rename_all = "snake_case")]` for the discriminator.

**Acceptance criterion:** `cargo test -p anvilml-core -- types::events` exits 0 with ≥ 4 tests (roundtrip each major variant, tag field present in JSON).

### Group B — anvilml-core error

#### P3-B1: anvilml-core: AnvilError enum complete

**Goal:** Implement `AnvilError` in `crates/anvilml-core/src/error.rs` with all variants per `ANVILML_DESIGN.md §5.2`. Implement `IntoResponse` for axum mapping each variant to its HTTP status and JSON error body `{error, message, request_id}`.

**Acceptance criterion:** `cargo test -p anvilml-core -- error` exits 0; each variant maps to its expected HTTP status code.

### Group C — anvilml-server

#### P3-C1: anvilml-server: stub GET /v1/system/env

**Goal:** Add `handlers/system.rs` with a stub `get_env` handler returning a default `EnvReport` (all fields empty/false, `provisioning: NotStarted`). Mount at `GET /v1/system/env` in `build_router`. Add `anvilml-core` as a dependency of `anvilml-server`.

**Acceptance criterion:** `curl http://127.0.0.1:8488/v1/system/env` → 200 with `{"preflight_ok":false,...}`.

### Group D — backend tests

#### P3-D1: backend: config_reference drift guard integration test

**Goal:** Create `backend/tests/config_reference.rs` that serialises `ServerConfig::default()` to a TOML string and recursively compares its key set against the checked-in `anvilml.toml`. Any key present in one but absent in the other fails the test.

**Files to create:**
- `backend/tests/config_reference.rs`
- `anvilml.toml` at repo root with all config keys at their documented defaults

**Acceptance criterion:** `cargo test -p anvilml --features mock-hardware -- config_reference` exits 0.

## Phase Acceptance Criteria

```bash
cargo test -p anvilml-core
cargo test -p anvilml --features mock-hardware -- config_reference
cargo run --features mock-hardware &
sleep 2
curl -s http://127.0.0.1:8488/v1/system/env | python3 -c "import sys,json; d=json.load(sys.stdin); assert 'preflight_ok' in d"
kill %1
```

## Known Constraints and Gotchas

- `WsEvent` uses `#[serde(tag = "type")]` — not `#[serde(tag = "_type")]`. The `_type` convention is used in IPC msgpack messages (Python ↔ Rust), not in WebSocket JSON events (Rust → client).
- All types that appear in HTTP responses need `ToSchema` (utoipa) derives for the OpenAPI binary. Add `utoipa` as a workspace dep with the `axum` feature.
- `NodeTypeDescriptor` and `SlotType` live in `anvilml-core` even though no scheduler or worker exists yet — they are part of the type contract that other crates will depend on.
