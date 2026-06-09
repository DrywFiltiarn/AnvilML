# Plan Report: P14-A3

| Field       | Value                                         |
|-------------|-----------------------------------------------|
| Task ID     | P14-A3                                        |
| Phase       | 014 — Artifact Storage                        |
| Description | anvilml-scheduler: handle ImageReady → ArtifactStore.save + JobImageReady |
| Depends on  | P14-A2                                        |
| Project     | anvilml                                       |
| Planned at  | 2026-06-09T16:50:00Z                          |
| Attempt     | 1                                             |

## Objective

Wire `ArtifactStore` into the job scheduler so that when the dispatch loop receives a `WorkerEvent::ImageReady` from a Python worker, it persists the artifact (decode, hash, write, DB insert) and broadcasts a `JobImageReady` WebSocket event containing only metadata (hash, dimensions, seed) — no image bytes.

## Scope

### In Scope
- Define `ArtifactSave` trait and `ArtifactSaveInput` struct in `anvilml-core` (avoids circular dependency between `anvilml-scheduler` and `anvilml-server`)
- Add `ArtifactSave` generic parameter to `JobScheduler` in `anvilml-scheduler`
- Handle `WorkerEvent::ImageReady` in dispatch loop: call `artifact_store.save()`, then broadcast `WsEvent::JobImageReady`
- Make `AppState` generic over `A: ArtifactSave` in `anvilml-server`
- Implement `ArtifactSave` trait on `ArtifactStore` in `anvilml-server`
- Wire `ArtifactStore` into `AppState` and `JobScheduler` in `backend/src/main.rs`
- Add unit test for `ImageReady` → `JobImageReady` flow in `anvilml-scheduler`
- Bump patch versions: `anvilml-core` (0.1.13 → 0.1.14), `anvilml-scheduler` (0.1.13 → 0.1.14)

### Out of Scope
- GET /v1/artifacts/:hash endpoint (P14-A4)
- GET /v1/artifacts list endpoint (P14-A5)
- Worker-side SaveImage mock (P14-A1)
- ArtifactStore.save implementation (P14-A2)

## Approach

### Step 1: Define trait in `anvilml-core`

**File:** `crates/anvilml-core/src/types/artifact.rs`

Add two items:

```rust
/// Input metadata carried into `ArtifactSave::save`.
#[derive(Debug, Clone)]
pub struct ArtifactSaveInput {
    pub width: i64,
    pub height: i64,
    pub seed: i64,
    pub steps: i64,
    pub prompt: String,
}

/// Trait for saving an artifact produced by a generation job.
///
/// Implemented by `ArtifactStore` in `anvilml-server`. Placed here to avoid
/// a circular dependency: `anvilml-scheduler` → `anvilml-server` would create
/// a cycle since `anvilml-server` already depends on `anvilml-scheduler`.
#[async_trait::async_trait]
pub trait ArtifactSave: Send + Sync {
    /// Decode, hash, persist, and record a single artifact.
    ///
    /// Returns the artifact hash on success.
    async fn save(&self, job_id: &str, image_b64: &str, meta: ArtifactSaveInput) -> Result<String, String>;
}
```

**File:** `crates/anvilml-core/src/lib.rs`

Add re-exports:
```rust
pub use types::artifact::{ArtifactSave, ArtifactSaveInput};
```

**File:** `crates/anvilml-core/Cargo.toml`

Add `async-trait` dependency (workspace or inline).

### Step 2: Update `JobScheduler` to be generic over `A: ArtifactSave`

**File:** `crates/anvilml-scheduler/src/scheduler.rs`

- Add `use anvilml_core::types::artifact::{ArtifactSave, ArtifactSaveInput};`
- Change struct definition:
  ```rust
  pub struct JobScheduler<A: ArtifactSave> {
      // ... existing fields ...
      artifact_store: A,
  }
  ```
- Update `new()` signature to accept `artifact_store: A`
- Clone `artifact_store` in `start_dispatch_loop()` closure
- Add `WorkerEvent::ImageReady` arm in the `tokio::select!` event handler block:

  ```rust
  WorkerEvent::ImageReady {
      job_id,
      image_b64,
      width,
      height,
      format: _,
      seed,
      steps,
      prompt,
  } => {
      let now = Utc::now();
      handle_image_ready(&self.artifact_store, &self.db, &self.broadcaster, &self.dispatch_notify,
          job_id, &image_b64, width, height, seed, steps as i64, &prompt, now).await;
  }
  ```

- Add `handle_image_ready()` async function:
  1. Build `ArtifactSaveInput { width, height, seed, steps, prompt }`
  2. Call `artifact_store.save(job_id_str, &image_b64, input).await`
  3. On success: broadcast `WsEvent::JobImageReady(JobImageReadyEvent { event: "job.image_ready", timestamp: now, job_id, artifact_hash: hash, width, height, seed })`
  4. On error: log `WARN` with `error = %e, job_id = %job_id, "artifact save failed"`
  5. Wake dispatch loop via `notify.notify_one()`

### Step 3: Implement `ArtifactSave` on `ArtifactStore`

**File:** `crates/anvilml-server/src/artifact/store.rs`

Add `async-trait` dependency to `anvilml-server/Cargo.toml` if not present, then:

```rust
use anvilml_core::types::artifact::{ArtifactSave, ArtifactSaveInput};

#[async_trait::async_trait]
impl ArtifactSave for ArtifactStore {
    async fn save(&self, job_id: &str, image_b64: &str, meta: ArtifactSaveInput) -> Result<String, String> {
        let meta_input = ArtifactStoreInput {
            width: meta.width,
            height: meta.height,
            seed: meta.seed,
            steps: meta.steps,
            prompt: meta.prompt,
        };
        let artifact = self.save(job_id, image_b64, meta_input)
            .await
            .map_err(|e| e.to_string())?;
        Ok(artifact.hash)
    }
}
```

Note: `ArtifactStore::save()` returns `Result<ArtifactMeta, ArtifactError>`. The trait method returns `Result<String, String>` — hash on success, error message on failure.

### Step 4: Make `AppState` generic over `A: ArtifactSave`

**File:** `crates/anvilml-server/src/state.rs`

- Add `use anvilml_core::types::artifact::{ArtifactSave, ArtifactSaveInput};`
- Change struct definition:
  ```rust
  pub struct AppState<A: ArtifactSave> {
      // ... existing fields ...
      pub artifact_store: A,
  }
  ```
- Update `new()` and `new_with_hardware()` to accept `artifact_store: A`
- Update `Clone` impl (no change needed since generic is preserved)

**File:** `crates/anvilml-server/src/lib.rs`

- Update `build_router()` signature — the `AppState` generic must flow through. Since `build_router` takes `AppState` by value and wraps it in `Arc`, the generic parameter is preserved automatically.

### Step 5: Wire everything in `backend/src/main.rs`

**File:** `backend/src/main.rs`

- Import `ArtifactStore` from `anvilml_server::artifact::store::ArtifactStore`
- Create `artifact_store`:
  ```rust
  let artifact_store = Arc::new(ArtifactStore::new(
      cfg.artifact_dir.clone().into(),
      db.clone(),
  ));
  ```
- Pass `artifact_store` to `JobScheduler::new()` and `AppState::new_with_hardware()`
- Since both structs are now generic over `ArtifactStore`, the type inference handles it

### Step 6: Add test for ImageReady → JobImageReady flow

**File:** `crates/anvilml-scheduler/src/scheduler.rs` (tests module)

Add a new test `test_image_ready_broadcasts_event`:

1. Create `MockArtifactStore` that implements `ArtifactSave` — stores the hash it receives
2. Build `JobScheduler<MockArtifactStore>` with `make_scheduler_with_artifact_store()`
3. Start dispatch loop
4. Submit a job (triggers dispatch → Running)
5. Inject `WorkerEvent::ImageReady` via `pool.publish_event()`
6. Wait for event handler to process
7. Assert:
   - Mock received the correct `job_id`, `image_b64`, and metadata
   - A `WsEvent::JobImageReady` was broadcast with matching `job_id`, `artifact_hash`, `width`, `height`, `seed`
   - Job status is still Running (ImageReady does not change status)

### Step 7: Bump patch versions

- `crates/anvilml-core/Cargo.toml`: `0.1.13` → `0.1.14`
- `crates/anvilml-scheduler/Cargo.toml`: `0.1.13` → `0.1.14`

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Create | `crates/anvilml-core/src/types/artifact.rs` | Append `ArtifactSave` trait + `ArtifactSaveInput` struct |
| Modify | `crates/anvilml-core/src/lib.rs` | Re-export `ArtifactSave`, `ArtifactSaveInput` |
| Modify | `crates/anvilml-core/Cargo.toml` | Add `async-trait` dependency |
| Modify | `crates/anvilml-scheduler/src/scheduler.rs` | Generic `A: ArtifactSave`, `ImageReady` handler, `handle_image_ready()`, test |
| Modify | `crates/anvilml-scheduler/Cargo.toml` | Add `anvilml-core` dep (already present), `async-trait` dep |
| Modify | `crates/anvilml-server/src/artifact/store.rs` | `impl ArtifactSave for ArtifactStore` |
| Modify | `crates/anvilml-server/Cargo.toml` | Add `async-trait` dep, `anvilml-core` dep |
| Modify | `crates/anvilml-server/src/state.rs` | Generic `A: ArtifactSave` on `AppState` |
| Modify | `backend/src/main.rs` | Wire `ArtifactStore` into `JobScheduler` + `AppState` |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `crates/anvilml-scheduler/src/scheduler.rs` | `test_image_ready_broadcasts_event` | ImageReady event → artifact save → JobImageReady WS broadcast with correct fields |

## CI Impact

No CI workflow files are modified. The test suite command `cargo test --workspace --features mock-hardware` must exit 0. The `async-trait` dependency is lightweight and already widely used in the Rust ecosystem. The trait-based approach avoids adding any new runtime dependencies beyond what `anvilml-server` already uses.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `async-trait` not in workspace deps | Medium | Low | Add inline dependency in each crate that needs it; no workspace polluting |
| Generic parameter propagation causes type inference issues in `main.rs` | Medium | Medium | Use explicit type annotation `JobScheduler<ArtifactStore>` and `AppState<ArtifactStore>` |
| Breaking change to `AppState::new()` / `new_with_hardware()` signatures | High | Medium | All call sites are in `main.rs` — update them together in one change |
| Circular dependency if trait placed in wrong crate | High | Critical | Trait placed in `anvilml-core` (leaf crate); both `anvilml-scheduler` and `anvilml-server` depend on it independently |

## Acceptance Criteria

- [ ] `ArtifactSave` trait defined in `anvilml-core` with `save(job_id, image_b64, meta) -> Result<String, String>`
- [ ] `JobScheduler` handles `WorkerEvent::ImageReady` — calls `artifact_store.save()` and broadcasts `WsEvent::JobImageReady` (no image bytes in event)
- [ ] `AppState` holds `Arc<dyn ArtifactSave>` or generic `A: ArtifactSave`
- [ ] `ArtifactStore` implements `ArtifactSave` trait
- [ ] `backend/src/main.rs` wires `ArtifactStore` through to both `JobScheduler` and `AppState`
- [ ] Unit test `test_image_ready_broadcasts_event` passes
- [ ] `cargo test --workspace --features mock-hardware` exits 0
