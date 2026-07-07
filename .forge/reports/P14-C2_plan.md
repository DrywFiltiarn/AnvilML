# Plan Report: P14-C2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P14-C2                                       |
| Phase       | 14 — Dispatch & Execute                     |
| Description | backend: main.rs spawns real WorkerPool + JobScheduler at startup |
| Depends on  | P14-C1                                       |
| Project     | anvilml                                      |
| Planned at  | 2026-07-07T22:30:00Z                         |
| Attempt     | 1                                            |

## Objective

Wire `backend/src/main.rs`'s normal (non-hw-probe) server-start path to actually detect hardware devices, spawn Python worker subprocesses via `WorkerPool::spawn_all()`, and start the scheduler's dispatch loop via `JobScheduler::start_dispatch_loop()`. This is the first task where the binary's normal run path spawns real worker subprocesses instead of constructing an empty pool and an idle scheduler. After this task completes, a built `anvilml` binary (compiled with `--features mock-hardware`) will start, detect mock GPU/CPU devices, spawn mock workers, and begin accepting job dispatches — while all existing tests continue to pass.

## Scope

### In Scope
- Call `detect_all_devices(&config).await` after the ghost-job reset, extract `GpuDevice`s from the result
- Call `workers.spawn_all(&devices, &config).await` to spawn one worker per device
- Construct `JobScheduler` wrapped in `Arc`, call `start_dispatch_loop(Arc::clone(&workers))` to start the dispatch background task
- Pass the scheduler `Arc` to the existing `AppState` constructor (unchanged)
- Bump `backend/Cargo.toml` patch version from `0.1.10` to `0.1.11`
- Add `#[tracing::instrument]` logging to the new startup code path (INFO for lifecycle events, DEBUG for details)

### Out of Scope
None. This task has an empty `defers_to` field and must implement its full scope. No deferred functionality.

## Existing Codebase Assessment

The codebase already has all the subsystems this task wires together:

**(a) What already exists:** `main.rs` (214 lines) already imports `WorkerPool`, `JobScheduler`, `detect_all_devices`, and constructs both with `Arc`. It creates `AppState` with `scheduler` and `workers` fields (added by P14-C1). The `WorkerPool` is constructed via `new()` but never populated — `spawn_all()` is never called. The `JobScheduler` is constructed but `start_dispatch_loop()` is never invoked. The `AppState` construction, `build_router()` call, TCP binding, and `tokio::select!` serve loop are all present and correct.

**(b) Established patterns:** Error handling uses `.map_err(|e| { eprintln!(...); std::process::exit(1) }).unwrap()` for startup-critical operations (seen on `config_load::load`, `create_pool`, seed loader, ghost-job reset). Logging uses `tracing::info!` with structured field notation. The `#[tracing::instrument]` attribute is used on all public async functions. `Arc` is used for shared state. The `mock-hardware` feature flag is forwarded through the dependency chain.

**(c) Gap between design and source:** The design doc states `spawn_all()` should be called with device metadata and config, and `start_dispatch_loop()` should be called with the workers pool — but these calls are absent from `main.rs`. The current code constructs empty pool and idle scheduler but never activates them. This gap is exactly what this task fills.

## Resolved Dependencies

No new external crates or packages are introduced. All types used are from existing workspace dependencies that are already declared in `backend/Cargo.toml`.

| Type   | Name              | Version verified | MCP source     | Feature flags confirmed |
|--------|-------------------|-----------------|----------------|------------------------|
| crate  | (workspace local) | —               | source read    | mock-hardware forwarded via chain |

All API shapes confirmed by reading source files directly:
- `anvilml_hardware::detect_all_devices(&ServerConfig) -> Result<HardwareInfo, AnvilError>` (async)
- `WorkerPool::new() -> Result<Self, AnvilError>` (async)
- `WorkerPool::spawn_all(&mut self, &[GpuDevice], &ServerConfig) -> Result<(), AnvilError>` (async)
- `JobScheduler::new(JobStore, Arc<NodeTypeRegistry>) -> Self`
- `JobScheduler::start_dispatch_loop(Arc<Self>, Arc<WorkerPool>) -> JoinHandle<()>`

## Approach

### Step 1: Call `detect_all_devices()` after ghost-job reset

After the existing ghost-job reset block (line 163), insert code to detect hardware devices:

```rust
// Detect all hardware devices (GPU/CPU) using the loaded config.
// The mock-hardware feature replaces real detectors with MockDetector,
// which returns synthetic device info driven by ANVILML_MOCK_* env vars.
// On Linux/Windows, real detectors (Vulkan, DXGI, sysfs) enumerate
// actual GPUs; CPU detection always returns one CPU device as fallback.
let hw_info = detect_all_devices(&config)
    .await
    .map_err(|e| {
        eprintln!("Failed to detect hardware devices: {e}");
        std::process::exit(1);
    })
    .unwrap();

// Log the detected devices for operator visibility.
// device_count is the number of GPUs/CPU detected; MockDetector
// returns 1 device (mock GPU) when mock-hardware is active.
tracing::info!(
    device_count = hw_info.devices.len(),
    "hardware devices detected"
);
```

Extract the `GpuDevice` list: `let devices: &[GpuDevice] = &hw_info.devices;`

**Rationale:** `detect_all_devices()` is already imported and used in the `hw-probe` branch (line 92). Using it in the normal path ensures consistency. The `HardwareInfo` struct has a `.devices` field of type `Vec<GpuDevice>` which `spawn_all()` accepts as `&[GpuDevice]`.

### Step 2: Call `workers.spawn_all()` to spawn workers

After the device detection, call `spawn_all()` on the already-constructed `workers` pool:

```rust
// Spawn a Python worker subprocess for each detected device.
// Each worker connects to the RouterTransport's DEALER socket,
// registers itself, and enters the message dispatch loop.
// With mock-hardware: spawns mock workers (no real Python interpreter).
// Without mock-hardware: spawns real Python workers that import torch.
workers
    .spawn_all(&devices, &config)
    .await
    .map_err(|e| {
        eprintln!("Failed to spawn workers: {e}");
        std::process::exit(1);
    })
    .unwrap();

// Log worker count for operator visibility.
tracing::info!(
    worker_count = workers.handles().len(),
    "workers spawned"
);
```

**Rationale:** `spawn_all(&mut self, devices, cfg)` is `async` and takes `&mut self`. Since `workers` is an `Arc<WorkerPool>`, we cannot call `&mut self` directly on an `Arc`. However, looking at the current code, `workers` is constructed as `Arc::new(WorkerPool::new().await...)` on line 182-186. To call `spawn_all`, we need access to `&mut WorkerPool`.

**Critical finding:** `workers` is `Arc<WorkerPool>`. The `spawn_all` method takes `&mut self`. We cannot call `spawn_all` on an `Arc<WorkerPool>` directly — we need a mutable reference. The solution: construct `WorkerPool` without wrapping in `Arc` first, call `spawn_all`, then wrap in `Arc`.

Revised approach for Step 2:
- Before constructing `AppState`, construct `WorkerPool` (not yet Arc-wrapped)
- Call `spawn_all()` on the non-Arc'd pool
- Then wrap in `Arc` for sharing with `AppState` and `start_dispatch_loop`

### Revised Step 1-2 combined (correct ownership):

```rust
// Detect all hardware devices.
let hw_info = detect_all_devices(&config)
    .await
    .map_err(|e| {
        eprintln!("Failed to detect hardware devices: {e}");
        std::process::exit(1);
    })
    .unwrap();

tracing::info!(
    device_count = hw_info.devices.len(),
    "hardware devices detected"
);

// Construct the worker pool (empty), spawn workers for each device,
// then wrap in Arc for sharing with AppState and the dispatch loop.
// WorkerPool::new() binds a RouterTransport and spawns the bridge;
// spawn_all() populates the pool with one ManagedWorker per device.
let mut pool = WorkerPool::new()
    .await
    .expect("WorkerPool::new() must succeed at startup");

pool.spawn_all(&hw_info.devices, &config)
    .await
    .expect("WorkerPool::spawn_all() must succeed at startup");

tracing::info!(
    worker_count = pool.handles().len(),
    "workers spawned"
);

let workers = Arc::new(pool);
```

### Step 3: Start the dispatch loop

After constructing `workers` and before constructing `AppState`, start the scheduler's dispatch loop:

```rust
// Construct the job scheduler with the database pool.
// The scheduler owns the in-memory job queue and dispatch loop;
// it uses the shared `pool` for job persistence via `JobStore`.
let job_store = JobStore::new(pool.clone());
let scheduler = Arc::new(JobScheduler::new(job_store, Arc::clone(&node_registry)));

// Start the dispatch loop as a background tokio task.
// The loop wakes on submit() notification, pops queued jobs,
// selects idle workers, and dispatches Execute messages.
// Returns a JoinHandle — we discard it since the scheduler
// lives for the lifetime of the process (held in AppState).
let _dispatch_handle = scheduler.start_dispatch_loop(Arc::clone(&workers));

tracing::info!("dispatch loop started");
```

**Rationale:** `start_dispatch_loop` takes `Arc<Self>` (consuming the Arc) and `Arc<WorkerPool>`. We need to clone the scheduler's Arc before passing it to `start_dispatch_loop` so we can still use it in `AppState`. The dispatch handle (`JoinHandle<()>`) is stored in a `_`-prefixed variable to suppress the unused warning — the task keeps running as long as the scheduler Arc is alive.

### Step 4: Construct AppState (existing code, unchanged)

The existing `AppState` construction (lines 191-198) is already correct — it takes `scheduler`, `workers`, and `db` among other fields. No changes needed to this block.

### Step 5: Bump version

Update `backend/Cargo.toml` patch version from `0.1.10` to `0.1.11`.

### Step 6: Logging

The new code adds three `tracing::info!` log calls:
1. `"hardware devices detected"` with `device_count` field
2. `"workers spawned"` with `worker_count` field
3. `"dispatch loop started"` (no structured fields)

These are mandatory INFO log points for the worker-spawning subsystem. The `#[tracing::instrument]` attribute on `spawn_all()` and `start_dispatch_loop()` provides additional DEBUG-level span logging automatically.

## Public API Surface

No new public items are introduced. This task only modifies `main.rs`, which is a binary entry point (not a library). All types used (`WorkerPool`, `JobScheduler`, `detect_all_devices`, `HardwareInfo`, `GpuDevice`, `JobStore`, `Arc`) are existing public APIs from workspace-local crates.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `backend/src/main.rs` | Insert device detection, worker spawning, and dispatch loop startup into the normal server-start path |
| Modify | `backend/Cargo.toml` | Bump patch version 0.1.10 → 0.1.11 |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `backend/tests/db_startup_tests.rs` | `test_db_file_created_on_startup` | Binary starts, creates DB file, prints "listening" | `anvilml` binary built | Temp `ANVILML_DB_PATH`, `ANVILML_PORT=0` | `.db` file exists, "listening" on stderr | `cargo test --workspace --features mock-hardware --test db_startup_tests` exits 0 |
| `backend/tests/db_startup_tests.rs` | `test_migrations_create_required_tables` | Migrations create `models` and `device_capabilities` tables | Binary built | Temp `ANVILML_DB_PATH`, `ANVILML_PORT=0` | Both tables in sqlite_master | Same command as above exits 0 |
| `backend/tests/db_startup_tests.rs` | `test_seed_populates_device_capabilities` | Seed loader populates device_capabilities | Binary built | Temp `ANVILML_DB_PATH`, `ANVILML_PORT=0` | Row count > 0 | Same command as above exits 0 |
| `backend/tests/db_startup_tests.rs` | `test_seed_idempotent_second_run` | Seed is idempotent across restarts | Binary built | Same temp `ANVILML_DB_PATH`, `ANVILML_PORT=0` | First count == second count | Same command as above exits 0 |
| `backend/tests/db_startup_tests.rs` | `test_missing_seed_file_causes_startup_failure` | Missing seed causes non-zero exit | Binary built | Non-existent `ANVILML_SEED_PATH`, `ANVILML_PORT=0` | Exit code != 0 | Same command as above exits 0 |
| `backend/tests/logging_tests.rs` | (existing) | Logging structure and mandatory log points | Binary built | Default config | Structured log output | `cargo test --workspace --features mock-hardware --test logging_tests` exits 0 |
| `backend/tests/shutdown_tests.rs` | (existing) | Graceful shutdown signal handling | Binary built | SIGINT after startup | Clean exit | `cargo test --workspace --features mock-hardware --test shutdown_tests` exits 0 |
| `backend/tests/config_reference.rs` | (existing) | Config surface sync (Gate 1) | Binary built | `anvilml.toml` matches `ServerConfig::default()` | No config drift | `cargo test --workspace --features mock-hardware --test config_reference` exits 0 |
| `backend/tests/hw_probe_help_test.rs` | (existing) | hw-probe subcommand still works | Binary built | `--help` on hw-probe | Help text output | `cargo test --workspace --features mock-hardware --test hw_probe_help_test` exits 0 |
| `backend/tests/cli_help_test.rs` | (existing) | CLI argument parsing | Binary built | `--help` | Help text output | `cargo test --workspace --features mock-hardware --test cli_help_test` exits 0 |

## CI Impact

No CI changes required. The task only modifies `backend/src/main.rs` and `backend/Cargo.toml`. The existing CI jobs (`rust-linux`, `rust-windows`) already compile with `--features mock-hardware` and run the full test suite, which includes the `backend/tests/` integration tests that spawn the binary. No new file types, gates, or test modules are added.

## Platform Considerations

None identified. The `detect_all_devices()` function already handles platform differences internally (Vulkan on Linux, DXGI on Windows, CPU fallback). The `mock-hardware` feature replaces all platform-specific detectors with `MockDetector`, which is platform-neutral. The Windows cross-check in ENVIRONMENT.md §7 (`cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu`) is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `spawn_all()` with real hardware (no mock-hardware feature) may hang waiting for Python workers to connect, since `spawn_all()` spawns subprocesses that need to import torch and connect to the ROUTER socket. If torch is not installed or the venv path is wrong, the worker subprocess may fail to start, and the dispatch loop will have no idle workers. | Low (only affects non-mock builds) | Medium | The acceptance criteria run with `--features mock-hardware`, which compiles MockDetector and skips real Python spawning. Real-hardware builds are expected to have a proper venv with torch installed. The existing integration tests already exercise the mock path successfully. |
| The `start_dispatch_loop()` consumes `Arc<Self>`, requiring us to clone the Arc before passing it. If we forget to clone, the code will fail to compile — this is a compile-time error, not a runtime risk. | Low | Low | The compiler will catch this. The approach explicitly clones the Arc before calling `start_dispatch_loop`. |
| The existing integration tests (`db_startup_tests.rs`) spawn the binary and wait for "listening" within 5 seconds. If `spawn_all()` takes longer than 5 seconds (e.g., slow mock worker startup), the test may timeout. | Low | Medium | Mock workers start quickly (no real Python interpreter). The mock path in `spawn_all_impl()` still constructs `ManagedWorker` and spawns its `run()` task, but this is lightweight. If timing becomes an issue, the timeout in the test (5s) may need adjustment — but this is a test concern, not a production concern. |
| `detect_all_devices()` may return an empty device list on systems with no GPU and no CPU detector. This would cause `spawn_all()` to spawn zero workers, leaving the dispatch loop with no idle workers to select. | Very Low | Low | `detect_all_devices()` always returns at least one CPU device (per ARCHITECTURE.md: "Never panics on missing driver. Always returns >=1 CPU device."). Zero-device return is impossible by design. |

## Acceptance Criteria

- [ ] `cargo build -p anvilml --features mock-hardware` exits 0
- [ ] `cargo test --workspace --features mock-hardware` exits 0
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0 (no new warnings)
- [ ] `cargo fmt --all -- --check` exits 0 (formatted code)
- [ ] `backend/Cargo.toml` version is `0.1.11`
