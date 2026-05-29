# Tasks: Phase 002 — Core Types & IPC

| Field            | Value                                                              |
|------------------|--------------------------------------------------------------------|
| Phase            | 002                                                                |
| Name             | Core Types & IPC                                                   |
| ANVIL Milestone  | M1 (part 1)                                                        |
| Status           | Draft                                                              |
| Depends on phases| 1                                                                  |
| Task file        | `forge/tasks/tasks_phase002.json`                                  |
| Design reference | `ANVILML_DESIGN.md` §3 (Config), §4 (Domain Types), §7 (IPC)      |

---

## Overview

Phase 002 implements the two crates that form the data layer and communication contract of the entire AnvilML system: `anvilml-core` and `anvilml-ipc`. No I/O, no async runtime, and no external processes are involved. Every type defined here is pure data — serializable, cloneable, well-tested structs and enums.

This phase must precede all other crates because `anvilml-core` is at the root of the dependency graph. Every crate from `anvilml-hardware` through `anvilml-server` imports its types. Implementing hardware detection, persistence, scheduling, or the HTTP server before these shared types exist would require each crate to invent its own interim representations and then rewrite them, producing avoidable churn. Defining the types first gives all subsequent phases a stable, tested API surface to build against.

`anvilml-ipc` is included in this phase rather than a later one because the IPC framing layer (the 4-byte length-prefix + msgpack encoding) is independent of the worker supervisor logic that wraps it. The framing can be fully unit-tested in isolation — including the oversize-frame rejection path — without spawning any process. Separating framing correctness from worker lifecycle correctness makes later debugging easier.

At the end of this phase, `cargo test -p anvilml-core` and `cargo test -p anvilml-ipc` both pass. Every domain type required by later phases is exported from `anvilml-core::prelude` or directly from `anvilml-core`. Every IPC message variant is defined and can be serialized and deserialized through the framing layer.

---

## Group Reference

| Group | Subsystem    | Tasks          | Summary                                                        |
|-------|--------------|----------------|----------------------------------------------------------------|
| A     | anvilml-core | P2-A1 … P2-A4  | Error types, config, domain types, hardware/worker/event types |
| B     | anvilml-ipc  | P2-B1 … P2-B2  | IPC message enums, msgpack framing with size enforcement       |

---

## Prerequisites

- P1-A2 complete: CI workflow is in place and enforcing fmt + clippy + test.
- The Cargo workspace compiles with `--features mock-hardware`.
- No domain types, config structs, or IPC types exist yet — this phase creates them from scratch.

---

## Contract Documents Applicable to This Phase

| Document section          | Relevant tasks      | What must match                                                              |
|---------------------------|---------------------|------------------------------------------------------------------------------|
| `ANVILML_DESIGN.md` §3.1  | P2-A2               | All `ServerConfig` fields, types, and defaults exactly as specified          |
| `ANVILML_DESIGN.md` §4.1  | P2-A3               | `Job`, `JobStatus`, `JobSettings`, `SubmitJobRequest`, `SubmitJobResponse`   |
| `ANVILML_DESIGN.md` §4.2  | P2-A3               | `ModelMeta`, `ModelKind`, `DType`, `ArtifactMeta`                            |
| `ANVILML_DESIGN.md` §4.3  | P2-A4               | `HardwareInfo`, `GpuDevice`, `DeviceType`, `HostInfo`, `InferenceCaps`       |
| `ANVILML_DESIGN.md` §4.4  | P2-A4               | `WorkerInfo`, `WorkerStatus`                                                 |
| `ANVILML_DESIGN.md` §4.5  | P2-A4               | All `WsEvent` variants and their exact field sets                            |
| `ANVILML_DESIGN.md` §7.2  | P2-B1               | All `WorkerMessage` variants and fields                                      |
| `ANVILML_DESIGN.md` §7.3  | P2-B1               | All `WorkerEvent` variants and fields                                        |
| `ANVILML_DESIGN.md` §7.1  | P2-B2               | Frame format: 4-byte big-endian u32 length + N bytes msgpack payload         |

---

## Task Descriptions

### Group A — anvilml-core

#### P2-A1: anvilml-core — error types and crate-level re-exports

**Goal:** Define the `AnvilError` type that all crates use for error propagation, and establish the re-export pattern from `lib.rs`.

**Files to create or modify:**
- `crates/anvilml-core/src/error.rs` — `AnvilError` enum
- `crates/anvilml-core/src/lib.rs` — re-exports `pub use error::AnvilError`
- `crates/anvilml-core/Cargo.toml` — add `thiserror` dependency

**Key implementation notes:**
- Use `thiserror::Error` derive macro. Each variant needs a `#[error("...")]` attribute with a meaningful message. Variants: `ConfigLoad(String)`, `Io(#[from] std::io::Error)`, `Json(#[from] serde_json::Error)`, `InvalidGraph(String)`, `WorkerDead(String)`, `JobNotFound(uuid::Uuid)`, `ArtifactNotFound(String)`, `DbError(String)`, `PayloadTooLarge { size_mib: u32, limit_mib: u32 }`.
- `AnvilError` must implement `Send + Sync` (it will cross async task boundaries). Verify no variant holds a non-Send type.
- Do not add `serde_json` or `uuid` to `anvilml-core` yet — those are added in P2-A3. The `Json` and `JobNotFound` variants should use `String` for now if needed to avoid premature deps, or use `#[cfg]` placeholders.

**Acceptance criterion:** `cargo test -p anvilml-core` exits 0.

---

#### P2-A2: anvilml-core — configuration types

**Goal:** Implement the full `ServerConfig` struct and all nested config types, matching the field names, types, and defaults specified in `ANVILML_DESIGN.md §3.1`.

**Files to create or modify:**
- `crates/anvilml-core/src/config.rs` — all config structs
- `crates/anvilml-core/src/lib.rs` — add `pub mod config; pub use config::ServerConfig`
- `crates/anvilml-core/Cargo.toml` — add `serde` (features: derive), `toml`

**Key implementation notes:**
- Every field that has a default value must implement it via `#[serde(default = "...")]` or a `Default` impl. The defaults must match exactly: `host = 127.0.0.1`, `port = 8488`, `artifact_dir = ./artifacts`, `db_path = ./sindristudio.db`, `venv_path = ./venv`, `worker_log_dir = ./logs`, `num_threads = 14`, `num_interop_threads = 4`.
- `FrontendConfig.mode` is an enum: `Local`, `Proxy { url: String }`, `Headless`. Serialize as a TOML inline table or string tag.
- Write one round-trip test: construct a `ServerConfig`, serialize to TOML string, deserialize back, assert fields match.

**Acceptance criterion:** `cargo test -p anvilml-core -- config` exits 0 with a config round-trip test passing.

---

#### P2-A3: anvilml-core — domain types — Job, Model, Artifact

**Goal:** Define the Job lifecycle types, model metadata types, and artifact metadata type that the registry, scheduler, server, and worker all share.

**Files to create or modify:**
- `crates/anvilml-core/src/types/job.rs` — Job, JobStatus, JobSettings, SubmitJobRequest, SubmitJobResponse
- `crates/anvilml-core/src/types/model.rs` — ModelMeta, ModelKind, DType
- `crates/anvilml-core/src/types/artifact.rs` — ArtifactMeta
- `crates/anvilml-core/src/types/mod.rs` — re-exports
- `crates/anvilml-core/src/lib.rs` — add `pub mod types`
- `crates/anvilml-core/Cargo.toml` — add `uuid` (features: v4, serde), `chrono` (features: serde), `utoipa`

**Key implementation notes:**
- `Job.graph` is `serde_json::Value` — the raw graph submitted by the client. It is stored as-is and forwarded to the worker; validation happens in the scheduler (phase 006), not here.
- `JobStatus` must derive `PartialEq` and `Eq` in addition to the standard derives, because the scheduler and server compare statuses directly.
- `ArtifactMeta.hash` is a `String` containing the SHA256 hex of the PNG bytes. It is content-addressed — two artifacts with identical pixel data share a hash. This field is set by the server artifact store (phase 007), not by this crate.
- All types must derive `utoipa::ToSchema` so the OpenAPI generator in phase 007 can reflect them.

**Acceptance criterion:** `cargo test -p anvilml-core -- types` exits 0.

---

#### P2-A4: anvilml-core — hardware and worker types + WebSocket event types

**Goal:** Define the hardware detection output types, worker state types, and the full WebSocket event enum. These types close out `anvilml-core` for the MVP feature set.

**Files to create or modify:**
- `crates/anvilml-core/src/types/hardware.rs` — HardwareInfo, GpuDevice, DeviceType, HostInfo, InferenceCaps
- `crates/anvilml-core/src/types/worker.rs` — WorkerInfo, WorkerStatus
- `crates/anvilml-core/src/types/events.rs` — WsEvent enum and all event structs per §4.5
- `crates/anvilml-core/src/types/mod.rs` — add re-exports for new modules

**Key implementation notes:**
- `WsEvent` must serialize with an `"event"` discriminator field and a `"timestamp"` field on every variant. Use `#[serde(tag = "event", rename_all = "snake_case")]` or a manual implementation — whichever produces the exact wire format `{ "event": "system.stats", "timestamp": "...", ... }` as specified in §4.5.
- `SystemStatsEvent` has `gpus: Vec<GpuStatSnapshot>` where `GpuStatSnapshot { index: u32, vram_used_mib: u32, vram_total_mib: u32 }`. Add this struct to `events.rs`.
- `JobProgressEvent.step` and `step_total` are `Option<u32>` and always `None` in the MVP. They exist to reserve the wire field for the fast-follow per-step progress feature (§25).
- Write a serialization test for `WsEvent::SystemStats` that asserts the JSON output contains `"event": "system.stats"` as a top-level key.

**Acceptance criterion:** `cargo test -p anvilml-core` exits 0 with ≥10 tests total across all modules.

---

### Group B — anvilml-ipc

#### P2-B1: anvilml-ipc — message types (Rust→Python and Python→Rust)

**Goal:** Define the two message enums that are the complete IPC contract between the Rust supervisor and the Python worker.

**Files to create or modify:**
- `crates/anvilml-ipc/src/messages.rs` — WorkerMessage and WorkerEvent enums
- `crates/anvilml-ipc/src/lib.rs` — re-exports
- `crates/anvilml-ipc/Cargo.toml` — add `serde` (features: derive), `rmp-serde`, `uuid` (features: serde), `anvilml-core` (path dep)

**Key implementation notes:**
- `WorkerMessage` variants per §7.2: `Ping { seq: u64 }`, `Shutdown {}`, `InitializeHardware { device_str: String }`, `Execute { job_id: Uuid, graph: serde_json::Value, settings: JobSettings, device_index: u32 }`, `CancelJob { job_id: Uuid }`, `MemoryQuery {}`.
- `WorkerEvent` variants per §7.3: `Pong { seq: u64 }`, `Ready {}`, `Progress { job_id: Uuid, node_index: u32, node_total: u32, node_type: String, step: Option<u32>, step_total: Option<u32> }`, `ImageReady { job_id: Uuid, image_bytes: Vec<u8>, width: u32, height: u32, seed: i64 }`, `MemoryReport { device_index: u32, vram_used_mib: u32, vram_total_mib: u32 }`, `Completed { job_id: Uuid }`, `Failed { job_id: Uuid, error: String, traceback: Option<String> }`, `Cancelled { job_id: Uuid }`, `Dying {}`.
- Use `#[serde(rename_all = "snake_case")]` on both enums for consistent Python interop.
- Write a msgpack serialization round-trip test for at least one `WorkerMessage` and one `WorkerEvent` variant.

**Acceptance criterion:** `cargo test -p anvilml-ipc -- messages` exits 0.

---

#### P2-B2: anvilml-ipc — length-prefixed msgpack framing

**Goal:** Implement the async read/write framing layer that wraps raw pipe I/O with the 4-byte length-prefix protocol, including the oversize-frame rejection guard and the read-fully loop required for correct Windows pipe behaviour.

**Files to create or modify:**
- `crates/anvilml-ipc/src/framing.rs` — `write_frame`, `read_frame`
- `crates/anvilml-ipc/src/lib.rs` — re-export `framing::{write_frame, read_frame}`
- `crates/anvilml-ipc/Cargo.toml` — add `tokio` (features: io-util), `bytes`

**Key implementation notes:**
- `write_frame<W: AsyncWrite + Unpin>(writer: &mut W, msg: &WorkerMessage) -> Result<(), AnvilError>`: serialize `msg` to msgpack bytes via `rmp_serde::to_vec_named`, prepend a 4-byte big-endian `u32` length, write the header and payload to `writer`. Use `write_all` (not `write`) to guarantee the entire buffer is flushed — a partial write would desync the stream.
- `read_frame<R: AsyncRead + Unpin>(reader: &mut R, max_mib: u32) -> Result<WorkerEvent, AnvilError>`: read exactly 4 bytes using `read_exact` (not `read`), decode big-endian `u32` length N, check `N <= max_mib * 1024 * 1024` before any allocation — return `AnvilError::PayloadTooLarge` if exceeded, then read exactly N bytes using `read_exact` and deserialize as `WorkerEvent`.
- **Both `read_exact` and `write_all` are mandatory**, not optional. On Linux, `read` on a pipe typically returns the full requested size. On Windows, named pipe and anonymous pipe reads frequently return fewer bytes than requested even when the data is available. A single `read` call is not sufficient on Windows; `read_exact` loops internally until the buffer is full or EOF. This is the cross-platform invariant per `ANVILML_DESIGN.md §7.1` ("the framing reader must read-fully").
- The oversize check must happen **before** any heap allocation for the payload. This is the security invariant: a malformed or malicious worker cannot cause unbounded memory allocation.
- Write two tests using `tokio::io::duplex`: (1) a round-trip test that writes a `WorkerMessage::Ping { seq: 1 }` frame and reads it back as `WorkerEvent::Pong { seq: 1 }`; (2) an oversize-rejection test that writes a 4-byte header encoding `65 * 1024 * 1024 + 1` bytes and asserts `read_frame` returns `AnvilError::PayloadTooLarge` without reading the (absent) payload.

**Acceptance criterion:** `cargo test -p anvilml-ipc -- framing` exits 0 with round-trip test and oversize-rejection test both passing on both Linux and Windows (verified via CI).

---

## Phase Acceptance Criteria

```
cargo test -p anvilml-core
cargo test -p anvilml-ipc
cargo clippy --workspace --features mock-hardware -- -D warnings
cargo fmt --all --check
```

---

## Known Constraints and Gotchas

- `utoipa::ToSchema` requires the `utoipa` crate as a dependency of `anvilml-core`. Add it at this phase even though the OpenAPI generator binary is not implemented until phase 007. This avoids a breaking change to the type definitions later.
- `serde_json::Value` is used for `Job.graph` and `WorkerMessage::Execute.graph`. Add `serde_json` to both `anvilml-core` and `anvilml-ipc` Cargo.toml files. It is a transitive dep of many crates, but it must be explicit.
- The `WsEvent` serde tag approach requires careful attention. Using `#[serde(tag = "event")]` with enum variants that also have a `timestamp` field works with serde's internally tagged representation, but all variant fields must be structs (not tuples) for this to work with serde's internal tagging. Verify that all `WsEvent` variant payloads are named structs before committing.
- `rmp-serde` serializes by default in the compact array format. Use `rmp_serde::to_vec_named` (named map format) so the Python `msgpack` library can decode fields by name rather than by position. This is critical for Python worker interop.
- Tokio must be available as a dev-dependency in `anvilml-ipc` for the framing tests. Add `tokio = { version = "1", features = ["rt", "macros", "io-util"] }` under `[dev-dependencies]`.
- **Windows pipe reads are partial.** `tokio::io::AsyncReadExt::read` on a Windows anonymous pipe may return fewer bytes than requested even when the data is fully available. Always use `read_exact` in the framing layer. `tokio::io::duplex` (used in tests) exhibits the same partial-read behaviour under load, so the tests will catch any single-call `read` bugs on both platforms.
