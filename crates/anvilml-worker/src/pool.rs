//! `WorkerPool`: owns every `ManagedWorker` in the process, the shared
//! `RouterTransport`, and the pool-wide IPC bridge (P8-F1).
//!
//! Per `ANVILML_DESIGN.md §9.2`: "`WorkerPool` owns `Vec<WorkerHandle>` and
//! the shared `Arc<RouterTransport>`."
//!
//! # Construction shape
//!
//! This task's own spec states `spawn_all(&mut self, devices, cfg)` — a
//! method on an already-existing `&mut self` — while also describing
//! `spawn_all()` itself as the place `bridge::spawn_bridge()` gets called
//! "once for the pool." Those two things don't fully reconcile: something
//! has to construct `self` (with its non-`Option` `transport` and
//! `bridge_handles` fields) *before* `spawn_all()` can ever run on it.
//!
//! Resolved as: `WorkerPool::new()` binds a fresh `RouterTransport` and
//! spawns the bridge immediately — neither depends on `devices` or `cfg`,
//! both are pool-wide resources that exist independently of which workers
//! end up in the pool. `spawn_all()` then only handles the per-device
//! worker-spawning loop, reusing what `new()` already set up. This keeps
//! the struct's fields exactly as specified (no `Option` wrapping needed,
//! since `new()` always fully populates them) and is still consistent with
//! "the pool's bridge exists by the time `spawn_all()` has run," just not
//! literally inside `spawn_all()`'s own function body.

use std::sync::Arc;
use std::time::Duration;

use anvilml_core::NodeTypeRegistry;
use anvilml_core::types::worker::{WorkerInfo, WorkerStatus};
use anvilml_core::{AnvilError, GpuDevice, ServerConfig};
use anvilml_ipc::{RouterTransport, WorkerMessage};
use tokio::sync::{RwLock, mpsc, oneshot};
use tokio::task::JoinHandle;

use crate::bridge::spawn_bridge;
use crate::demux::Demux;
use crate::env::WorkerEnv;
use crate::managed::{
    DEFAULT_GRACEFUL_SHUTDOWN_TIMEOUT, DEFAULT_INIT_TIMEOUT, DEFAULT_WATCHDOG_PING_INTERVAL,
    DEFAULT_WATCHDOG_PONG_TIMEOUT, ManagedWorker, ManagedWorkerConfig, WorkerHandle,
};
use crate::respawn::RespawnPolicy;
use crate::spawn::{ProcessWorkerSpawner, WorkerSpawner};

/// Owns every `ManagedWorker` in the process, the shared `RouterTransport`
/// all of them communicate over, and the pool-wide IPC bridge (P8-F1) that
/// routes each worker's events to its own, individually-registered demux
/// channel — see `bridge.rs`'s own doc comment for why one shared bridge,
/// not one per worker.
pub struct WorkerPool {
    /// Interior-mutable so `spawn_worker()`/`restart_worker()` (P18-D2/
    /// P18-D3) can add or replace a handle through a shared `&self` —
    /// e.g. via `Arc<WorkerPool>`, which is how `AppState.workers` holds
    /// this pool and thus how an HTTP handler ever reaches it. `std::sync`
    /// (not `tokio::sync`) deliberately: every access here is a brief,
    /// synchronous read-or-mutate (never held across an `.await`), so a
    /// blocking lock avoids the overhead of an async one; see
    /// `handles()`'s own doc comment for how callers on the hot dispatch
    /// path (`anvilml-scheduler`'s `dispatch_one()`) avoid ever holding
    /// this lock across an `.await` at all.
    handles: std::sync::RwLock<Vec<WorkerHandle>>,
    /// Device metadata for all workers in this pool, populated at spawn time
    /// by `spawn_all()`. The device at index `i` corresponds to the worker
    /// handle at `handles()[i]`. Used by the scheduler's dispatch loop to
    /// select workers based on device type and VRAM availability (P14-A4).
    devices: Vec<GpuDevice>,
    transport: Arc<RouterTransport>,
    demux: Arc<Demux>,
    /// Sender half of the bridge's writer channel. See `bridge_sender()`'s
    /// own doc comment for the full explanation of its current (unused)
    /// status and why it's kept regardless.
    bridge_tx: mpsc::Sender<(String, WorkerMessage)>,
    /// (writer task, reader task) — see `bridge::spawn_bridge()`'s own
    /// doc comment for what each does.
    bridge_handles: (JoinHandle<()>, JoinHandle<()>),
    /// Per-device construction context captured by `spawn_all_impl()` the
    /// first time it runs, and reused by `spawn_worker()` (P18-D2) for any
    /// later single-device (re)spawn — e.g. P18-D3's restart handler. Kept
    /// as `Option` because a freshly-`new()`-ed pool hasn't spawned
    /// anything yet and so has no context to reuse; `spawn_worker()` on
    /// such a pool returns `AnvilError::Internal` rather than panicking.
    /// `None` after `new()`, `Some` after the first `spawn_all()`/
    /// `spawn_all_with_spawner()` call. Never mutated again after that
    /// first call, so — unlike `handles` — a plain field is fine: every
    /// read of it (from `spawn_worker()`) happens strictly after the one
    /// write (from `spawn_all_impl()`), by construction (`spawn_worker()`
    /// has no other caller that could run concurrently with the pool's
    /// initial startup spawn).
    spawn_config: Option<PoolSpawnConfig>,
    /// Serializes `restart_worker()` calls (P18-D3) — held for a whole
    /// restart (shutdown request → await exit → respawn → splice into
    /// the pool). Restarts are a rare, operator-triggered administrative
    /// action, not a hot path, so this simple global serialization is
    /// deliberately preferred over finer-grained per-worker locking: it
    /// guarantees that `spawn_worker()`'s tail-append (see that method's
    /// own doc comment on how it mutates `handles`) can never be raced by
    /// a second, concurrently-in-flight restart of a *different* worker,
    /// which would otherwise make "which tail entry is mine" ambiguous.
    restart_lock: tokio::sync::Mutex<()>,
}

/// Everything `spawn_worker()` needs to construct one more `ManagedWorker`
/// identically to how `spawn_all_impl()`'s original per-device loop body
/// built each one — captured once (per `spawn_all_impl()` call) so that a
/// later single-device restart doesn't require its caller to re-supply
/// `ServerConfig`, the `WorkerSpawner`, and the shared `NodeTypeRegistry`.
/// `Clone` is cheap: `Arc` bumps plus a `PathBuf`/`String` clone.
#[derive(Clone)]
struct PoolSpawnConfig {
    venv_path: std::path::PathBuf,
    max_ipc_payload_mib: u32,
    log_level: String,
    mock: bool,
    spawner: Arc<dyn WorkerSpawner>,
    node_registry: Arc<NodeTypeRegistry>,
}

/// Whether an `ANVILML_FORCE_WORKER_MOCK` env var value should force mock
/// mode, per `ENVIRONMENT.md §3.5`'s documented contract: `"1"` = force
/// mock; unset = no effect. Any other value (`"0"`, `"true"`, empty
/// string, etc.) is also treated as no effect — this mirrors the table's
/// own binary framing exactly, it is not a general-purpose truthy-string
/// parser, so don't extend it to accept other "truthy" spellings without
/// first checking whether `ENVIRONMENT.md` documents them.
///
/// Takes the value as a parameter rather than reading
/// `std::env::var(...)` directly inside this function, specifically so
/// this parsing logic is unit-testable without mutating process-global
/// environment state — `cargo test`'s default parallel execution makes
/// directly setting env vars inside a test unsafe (another concurrently-
/// running test could observe or clobber the same process-wide value).
fn force_mock_from_env_value(value: Option<&str>) -> bool {
    value == Some("1")
}

/// Outcome of `WorkerPool::restart_worker()` (P18-D3), mirroring the
/// established `CancelOutcome` pattern (`anvilml-scheduler`'s
/// `JobScheduler::cancel()`) so the HTTP handler's own status-code mapping
/// stays a simple match, not ad-hoc error-string inspection.
pub enum RestartOutcome {
    /// The old generation exited (or was force-timed-out waiting) and a
    /// replacement was spawned and spliced into the same slot.
    Accepted(WorkerHandle),
    /// No worker with the given `worker_id` exists in the pool.
    NotFound,
    /// The worker is already `Dying` — a shutdown (this restart's own, a
    /// concurrent one, or `shutdown_all()`) is already in flight for it.
    Conflict,
}

impl WorkerPool {
    /// Construct an empty pool: binds a fresh `RouterTransport`, creates a
    /// fresh `Demux`, and spawns the pool-wide bridge (P8-F1) against
    /// them. No workers exist yet — call `spawn_all()` to populate the
    /// pool from a device list.
    ///
    /// # Errors
    ///
    /// Returns `AnvilError::Ipc` if `RouterTransport::bind()` fails (e.g.
    /// no available ports).
    pub async fn new() -> Result<Self, AnvilError> {
        let transport = Arc::new(
            RouterTransport::bind()
                .await
                .map_err(|e| AnvilError::Ipc(e.to_string()))?,
        );
        let demux = Arc::new(Demux::new());
        let (bridge_tx, writer_handle, reader_handle) =
            spawn_bridge(Arc::clone(&transport), Arc::clone(&demux));

        Ok(Self {
            handles: std::sync::RwLock::new(Vec::new()),
            devices: Vec::new(),
            transport,
            demux,
            bridge_tx,
            bridge_handles: (writer_handle, reader_handle),
            spawn_config: None,
            restart_lock: tokio::sync::Mutex::new(()),
        })
    }

    /// A snapshot of the pool's currently-tracked worker handles.
    ///
    /// Empty immediately after `new()`; populated by `spawn_all()`, one
    /// handle per device.
    ///
    /// Returns an owned `Vec<WorkerHandle>` (each element a cheap `Clone`
    /// — a few `Arc` bumps plus a small `String`), not a borrowed
    /// `&[WorkerHandle]` — a deliberate change from this method's
    /// pre-P18-D2/D3 shape. `handles` is now interior-mutable (a
    /// `std::sync::RwLock`, see that field's own doc comment) so
    /// `spawn_worker()`/`restart_worker()` can mutate it through a shared
    /// `&self`; a borrowed slice can't outlive the lock guard that
    /// produces it, so returning one is no longer possible. Returning a
    /// snapshot instead is also strictly *safer* for this method's
    /// existing callers than a guard would have been: `dispatch_one()`
    /// (`anvilml-scheduler`, the hot per-job dispatch path) iterates this
    /// return value while calling `.await` on each handle — holding a
    /// `std::sync::RwLockReadGuard` across those awaits would be a real
    /// anti-pattern (a blocking guard held across suspension points); an
    /// owned snapshot has no such hazard. Every existing call site
    /// (`for handle in workers.handles()`, `.iter()`, `.len()`,
    /// `.is_empty()`) is already source-compatible with this signature
    /// change — Rust's iteration and Deref-based method resolution work
    /// identically whether `handles()` returns owned data or a borrow.
    pub fn handles(&self) -> Vec<WorkerHandle> {
        self.handles
            .read()
            .expect("WorkerPool handles lock poisoned")
            .clone()
    }

    /// Return the device list for all workers in this pool.
    ///
    /// Each `GpuDevice` carries `device_type`, `vram_free_mib`, and `index`.
    /// The device at index `i` corresponds to the worker handle at
    /// `handles()[i]`. This is used by the scheduler's dispatch loop to
    /// select workers based on device type and VRAM availability (per
    /// `ANVILML_DESIGN.md §12.5`).
    ///
    /// Empty immediately after `new()`; populated by `spawn_all()`.
    pub fn devices(&self) -> &[GpuDevice] {
        &self.devices
    }

    /// Build a `Vec<WorkerInfo>` snapshot of every worker currently in the
    /// pool — used by `ws::stats_tick::spawn_stats_tick()` (`P16-D1`) for
    /// the periodic `SystemStats` heartbeat, and available to any future
    /// `/v1/workers` listing endpoint.
    ///
    /// `handles()[i]` and `devices()[i]` correspond by construction (see
    /// this struct's own `devices` field doc comment) — zipping them is
    /// therefore safe without any separate index-matching lookup.
    ///
    /// `pid` and `current_job_id` are always `None` here: neither is
    /// tracked at the `WorkerHandle`/`WorkerPool` layer today. `pid`
    /// lives inside the OS process handle owned by `ManagedWorker`'s own
    /// supervisor task; per-worker job assignment lives in
    /// `JobScheduler`'s dispatch state. Populating either field
    /// accurately would mean threading data across a layer boundary this
    /// method doesn't otherwise cross — left as a known, explicit gap
    /// rather than fabricated data.
    pub async fn list(&self) -> Vec<WorkerInfo> {
        // Snapshot handles() first (releases the lock immediately — see
        // that method's own doc comment) rather than iterating the locked
        // field directly: this loop calls handle.status().await per
        // element, and a std::sync lock must never be held across an
        // .await.
        let handles = self.handles();
        let mut out = Vec::with_capacity(handles.len());
        for (handle, device) in handles.iter().zip(self.devices.iter()) {
            out.push(WorkerInfo {
                worker_id: handle.worker_id.clone(),
                status: handle.status().await,
                device_index: device.index,
                device_type: device.device_type,
                pid: None,
                current_job_id: None,
            });
        }
        out
    }

    /// The pool-wide `RouterTransport` every worker in this pool shares.
    pub fn transport(&self) -> &Arc<RouterTransport> {
        &self.transport
    }

    /// The pool-wide `Demux` every worker in this pool routes its events
    /// through, and the *only* sanctioned way for a subsystem outside
    /// `anvilml-worker` to observe this pool's `WorkerEvent`s — via
    /// `Demux::subscribe()` (`ANVILML_DESIGN.md §9.8`,
    /// `docs/ADDENDUM_DEMUX_FANOUT.md`). Calling `transport().recv()`
    /// directly instead races `bridge.rs`'s own `reader_task` for every
    /// incoming frame; see that module's doc comment for why.
    pub fn demux(&self) -> &Arc<Demux> {
        &self.demux
    }

    /// The pool-wide IPC bridge's writer channel — queuing a `(worker_id,
    /// message)` pair here sends it via `RouterTransport::send()`, the
    /// same way `bridge.rs`'s own writer task always has (see
    /// `bridge::spawn_bridge()`'s own doc comment).
    ///
    /// Not consumed by anything in `anvilml-worker` itself as of this
    /// task — every current outgoing message
    /// (`graceful_shutdown_child()`'s `WorkerMessage::Shutdown`,
    /// `KeepaliveWatchdog`'s Pings) goes through a `ManagedWorker`'s own
    /// direct `transport.send()` call instead, not through this pool-wide
    /// channel. Exposed regardless, matching this task's own description
    /// of `spawn_all()` "keeping its writer" — a future caller with a
    /// genuine pool-wide send need (e.g. broadcasting to every worker at
    /// once) can use this rather than re-deriving a sender from
    /// `transport()`/re-spawning a second bridge.
    pub fn bridge_sender(&self) -> &mpsc::Sender<(String, WorkerMessage)> {
        &self.bridge_tx
    }

    /// Spawn one `ManagedWorker` per device, registering a `WorkerHandle`
    /// for each into this pool. Thin wrapper around `spawn_all_impl()`
    /// using the real `ProcessWorkerSpawner` — see that method's own doc
    /// comment for the full behavior. Calls `spawn_all_impl()` directly
    /// rather than `spawn_all_with_spawner()`: the latter is
    /// `test-utils`-gated, so a plain, ungated build (e.g. `cargo clippy
    /// --workspace` without that feature) would fail to compile this
    /// method at all if it called through the gated wrapper.
    ///
    /// `node_registry` is the shared registry populated by worker Ready
    /// events — each worker registers its own node types into this same
    /// `Arc` so the scheduler and server handlers can query it.
    pub async fn spawn_all(
        &mut self,
        devices: &[GpuDevice],
        cfg: &ServerConfig,
        node_registry: Arc<NodeTypeRegistry>,
    ) -> Result<(), AnvilError> {
        self.spawn_all_impl(devices, cfg, Arc::new(ProcessWorkerSpawner), node_registry)
            .await
    }

    /// Spawn one `ManagedWorker` per device, using the given `spawner`
    /// rather than always constructing a `ProcessWorkerSpawner`.
    ///
    /// `spawn_all()` itself always passes a real `ProcessWorkerSpawner` —
    /// this is a `test-utils`-gated variant, not a general-purpose public
    /// API, needed because `ProcessWorkerSpawner` launches a real Python
    /// interpreter from a real virtualenv, which no test environment has.
    /// Matches the same test-only-injectable-dependency pattern already
    /// established in `managed.rs` (`run_once_for_test()`,
    /// `child_pid_for_test()`, `take_child_for_test()`) for the identical
    /// reason: `#[cfg(test)]` doesn't work for code called from
    /// integration tests in `tests/`, since those compile as separate
    /// crates.
    ///
    /// `ProcessWorkerSpawner` is stateless (a unit struct), so production's
    /// own `spawn_all()` shares one `Arc` across every device rather than
    /// constructing a fresh instance per worker — matching what this
    /// method does for whatever `spawner` a caller (test or otherwise)
    /// provides.
    ///
    /// `log_level` is read from `ANVILML_LOG`, falling back to `RUST_LOG`,
    /// falling back to `"info"` — matching `ENVIRONMENT.md`'s own
    /// documented precedence for the supervisor's own log level. Neither
    /// env var is a `ServerConfig` field, so this can't come from `cfg`
    /// directly; reading the same variables the supervisor itself would
    /// use means every worker logs at the same level as the process
    /// supervising it, which is the reasonable default absent any
    /// per-worker override mechanism.
    ///
    /// `mock` (whether to inject `ANVILML_WORKER_MOCK`, per
    /// `WorkerEnv::build()`'s own doc comment) is `true` if either the
    /// `mock-hardware` cargo feature is active at compile time, or the
    /// supervisor's own environment has `ANVILML_FORCE_WORKER_MOCK` set to
    /// exactly `"1"` at runtime — the latter is `env.rs`'s own documented
    /// deferral ("a runtime override... not set by this builder") to
    /// whichever caller actually constructs each worker's environment,
    /// which is here. Neither is read from `cfg: &ServerConfig`.
    ///
    /// Each worker gets `RespawnPolicy::default()` (`ANVILML_DESIGN.md
    /// §19.4`'s documented defaults: 2s delay, 5 max attempts, 5-minute
    /// window) and the production `DEFAULT_*` timeout constants — this
    /// task's own spec doesn't mention per-device timeout overrides, and
    /// none of the config types checked (`ServerConfig`, `GpuDevice`)
    /// carry any.
    #[cfg(feature = "test-utils")]
    pub async fn spawn_all_with_spawner(
        &mut self,
        devices: &[GpuDevice],
        cfg: &ServerConfig,
        spawner: Arc<dyn WorkerSpawner>,
        node_registry: Arc<NodeTypeRegistry>,
    ) -> Result<(), AnvilError> {
        self.spawn_all_impl(devices, cfg, spawner, node_registry)
            .await
    }

    /// Inject mock worker handles and devices into the pool for testing.
    ///
    /// `WorkerPool::new()` constructs an empty pool; `spawn_all()` populates
    /// it from a device list by spawning real Python worker subprocesses,
    /// which no test environment can do. This method lets integration tests
    /// populate the pool with pre-constructed handles (e.g. handles whose
    /// status is manually set to `Idle` or `Busy`) so that `dispatch_one()`
    /// can exercise the worker-selection logic.
    ///
    /// The input is a list of `(WorkerHandle, GpuDevice)` pairs. Each handle
    /// is pushed into `self.handles` and each device into `self.devices`,
    /// preserving the invariant that `handles()[i]` corresponds to
    /// `devices()[i]`. This matches the invariant established in
    /// `spawn_all_impl()`.
    ///
    /// The pool's transport and bridge remain untouched — they were set up
    /// by `new()`. This method only populates the handles and devices lists.
    #[cfg(feature = "test-utils")]
    pub fn set_up_test_workers(&mut self, workers: Vec<(WorkerHandle, GpuDevice)>) {
        // `&mut self` already guarantees exclusive access, so `get_mut()`
        // bypasses the lock entirely — no blocking, no need for this to
        // be async.
        let handles = self
            .handles
            .get_mut()
            .expect("WorkerPool handles lock poisoned");
        for (handle, device) in workers {
            handles.push(handle);
            self.devices.push(device);
        }
    }

    /// Shared implementation for `spawn_all()` and
    /// `spawn_all_with_spawner()` — see either's own doc comment for the
    /// full behavior. Private: not `test-utils`-gated, since it isn't
    /// itself part of any public contract, only reached through the two
    /// gated/ungated public wrappers above.
    ///
    /// `node_registry` is the shared registry that every worker registers
    /// its node types into via the Ready event path — the scheduler and
    /// server handlers query this same registry, so it must be shared
    /// rather than a fresh per-worker instance.
    async fn spawn_all_impl(
        &mut self,
        devices: &[GpuDevice],
        cfg: &ServerConfig,
        spawner: Arc<dyn WorkerSpawner>,
        node_registry: Arc<NodeTypeRegistry>,
    ) -> Result<(), AnvilError> {
        let log_level = std::env::var("ANVILML_LOG")
            .or_else(|_| std::env::var("RUST_LOG"))
            .unwrap_or_else(|_| "info".to_string());

        // mock-hardware determines this at compile time; ANVILML_FORCE_WORKER_MOCK
        // is a runtime override on top of that, read here specifically because
        // env.rs's own WorkerEnv::build() doc comment (and P8-B1's own task
        // text, which built that function) explicitly deferred this
        // responsibility to "the caller" — this is that caller. Per
        // ENVIRONMENT.md §3.5's documented contract: forces mock mode into
        // every worker even when compiled without mock-hardware, if and only
        // if the supervisor's own environment has this set to exactly "1"
        // ("unset = no effect" — any other value, including "0" or "true",
        // is treated as unset, matching the table's own binary framing).
        let force_mock =
            force_mock_from_env_value(std::env::var("ANVILML_FORCE_WORKER_MOCK").ok().as_deref());
        let mock = cfg!(feature = "mock-hardware") || force_mock;

        // Capture this call's construction context so a later single-device
        // spawn_worker() call (P18-D2/P18-D3) can rebuild an identical
        // ManagedWorker without its caller re-supplying ServerConfig, the
        // WorkerSpawner, or the shared NodeTypeRegistry. Overwritten on
        // every spawn_all_impl() call — in practice this runs exactly once
        // per pool, at startup.
        self.spawn_config = Some(PoolSpawnConfig {
            venv_path: cfg.venv_path.clone(),
            max_ipc_payload_mib: cfg.max_ipc_payload_mib,
            log_level,
            mock,
            spawner,
            node_registry,
        });

        // Thin loop: spawn_worker() does the actual per-device construction
        // (WorkerEnv build, ManagedWorker::new(), tokio::spawn(worker.run()),
        // WorkerHandle construction, push into self.handles) and device
        // tracking is kept here, preserving the invariant that
        // handles()[i] corresponds to devices()[i].
        for device in devices {
            self.spawn_worker(device.clone()).await?;
            self.devices.push(device.clone());
        }

        Ok(())
    }

    /// Spawn one `ManagedWorker` for `device`, register a `WorkerHandle` for
    /// it into `self.handles`, and return that handle.
    ///
    /// Pure extraction of `spawn_all_impl()`'s original per-device loop
    /// body (P18-D2) — no behavior change from what `spawn_all()` already
    /// did, just callable for a single device on its own. Reuses the
    /// `ServerConfig`/`WorkerSpawner`/`NodeTypeRegistry` context captured by
    /// the most recent `spawn_all_impl()` call (`self.spawn_config`) rather
    /// than taking them as parameters — needed so a later caller (P18-D3's
    /// restart handler) can spawn a single replacement worker knowing only
    /// which `device` it's replacing.
    ///
    /// Does **not** push `device` into `self.devices` — `spawn_all_impl()`'s
    /// own thin loop does that (bulk spawn), and P18-D3's restart handler
    /// does its own device-slot bookkeeping (it isn't adding a new device,
    /// it's replacing an existing slot's worker).
    ///
    /// # Errors
    ///
    /// Returns `AnvilError::Internal` if called before any
    /// `spawn_all()`/`spawn_all_with_spawner()` call has populated
    /// `self.spawn_config` — `spawn_worker()` has no `ServerConfig`/
    /// `WorkerSpawner`/`NodeTypeRegistry` of its own to build a worker from
    /// in that case.
    ///
    /// Takes `&self`, not `&mut self`: `handles` is interior-mutable
    /// (`std::sync::RwLock`, see that field's own doc comment)
    /// specifically so this method is callable through a shared
    /// `Arc<WorkerPool>` — e.g. `AppState.workers` — which is how
    /// `restart_worker()` (P18-D3) reaches it from a live HTTP handler,
    /// where exclusive `&mut` access to the pool is never available.
    /// `spawn_all_impl()` (still `&mut self`, unchanged) calls this via
    /// an automatic `&mut self -> &self` reborrow, so its own callers
    /// (`spawn_all()`/`spawn_all_with_spawner()`) needed no signature
    /// change.
    pub async fn spawn_worker(&self, device: GpuDevice) -> Result<WorkerHandle, AnvilError> {
        let cfg = self.spawn_config.clone().ok_or_else(|| {
            AnvilError::Internal(
                "spawn_worker() called before spawn_all()/spawn_all_with_spawner() \
                 populated the pool's spawn context"
                    .to_string(),
            )
        })?;

        let worker_id = device.index.to_string();

        let env = WorkerEnv::build(
            self.transport.port,
            &worker_id,
            device.index,
            device.device_type,
            cfg.mock,
            &cfg.log_level,
            cfg.max_ipc_payload_mib,
        );

        let status = Arc::new(RwLock::new(WorkerStatus::Initializing));
        let (pong_tx, _pong_rx) = mpsc::channel(16);
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let (force_shutdown_tx, force_shutdown_rx) = oneshot::channel();

        let worker = ManagedWorker::new(ManagedWorkerConfig {
            worker_id: worker_id.clone(),
            transport: Arc::clone(&self.transport),
            demux: Arc::clone(&self.demux),
            status: Arc::clone(&status),
            // Share the node registry with the scheduler and server —
            // worker Ready events populate this same Arc so that
            // `node_registry.is_empty()` in the scheduler is false
            // once any worker has sent Ready.
            node_registry: Arc::clone(&cfg.node_registry),
            respawn_policy: RespawnPolicy::default(),
            init_timeout: DEFAULT_INIT_TIMEOUT,
            pong_tx,
            watchdog_ping_interval: DEFAULT_WATCHDOG_PING_INTERVAL,
            watchdog_pong_timeout: DEFAULT_WATCHDOG_PONG_TIMEOUT,
            graceful_shutdown_timeout: DEFAULT_GRACEFUL_SHUTDOWN_TIMEOUT,
            venv_path: cfg.venv_path.clone(),
            env,
            spawner: Arc::clone(&cfg.spawner),
        });

        let join_handle = tokio::spawn(worker.run(shutdown_rx, force_shutdown_rx));
        let join_handle = Arc::new(tokio::sync::Mutex::new(Some(join_handle)));
        let handle = WorkerHandle::new(
            worker_id,
            status,
            Some(shutdown_tx),
            Some(force_shutdown_tx),
            join_handle,
        );
        // Store the ORIGINAL handle — the one carrying live shutdown_tx/
        // force_shutdown_tx — and return a CLONE to the caller, not the
        // reverse. WorkerHandle::clone() always sets both to None (see
        // that impl's own doc comment), so storing a clone here would
        // leave every handle inside self.handles permanently unable to
        // request shutdown, while the real, sender-carrying original
        // would exist only as this method's return value — dropped
        // almost immediately by spawn_all_impl(), which discards it
        // (`self.spawn_worker(device.clone()).await?;`, no `let`).
        // Dropping an oneshot::Sender resolves the paired Receiver
        // exactly like a real send — ManagedWorker::run()'s
        // `_ = &mut *shutdown_rx =>` branch can't distinguish "sender
        // sent ()" from "sender was dropped" (both just resolve the
        // future), so the worker would flip straight to Dying and exit
        // within moments of every single spawn, pool-wide, not only
        // during a restart. Appends to the tail — see this method's own
        // doc comment for why `&self` (not `&mut self`) works here.
        // `restart_worker()` (P18-D3) relies specifically on "tail" (not
        // "some unspecified position") and on `restart_lock` serializing
        // every call into this method from that path, so the tail entry
        // after this push is unambiguously the one just constructed here.
        let handle_for_caller = handle.clone();
        self.handles
            .write()
            .expect("WorkerPool handles lock poisoned")
            .push(handle);
        Ok(handle_for_caller)
    }

    /// Restart the worker currently occupying `worker_id`'s slot: request
    /// its graceful shutdown, wait for the outgoing generation to exit,
    /// then spawn a fresh replacement into the same slot (P18-D3).
    ///
    /// # Why this exists — the audit finding it closes
    ///
    /// `WorkerHandle::request_shutdown()` alone does **not** restart a
    /// worker. Per `managed.rs`'s own `RunOutcome` documentation,
    /// `request_shutdown()` drives `ManagedWorker::run()` into
    /// `RunOutcome::ShutdownRequested`, which breaks its loop
    /// permanently — that generation simply exits. Only the crash path
    /// (`RunOutcome::Crashed { should_respawn: true }`, consulting
    /// `RespawnPolicy`) causes `run()` to loop back and spawn a new
    /// generation on its own. So a caller that only calls
    /// `request_shutdown()` and waits gets a worker that gracefully
    /// exits and *stays* exited — not a restart. This method performs the
    /// actual two-step restart explicitly: shut the old generation down,
    /// then call `spawn_worker()` itself.
    ///
    /// # Concurrency
    ///
    /// The whole sequence runs under `self.restart_lock` — see that
    /// field's own doc comment for why a simple global serialization
    /// (rather than finer-grained per-worker locking) is the right
    /// tradeoff here.
    ///
    /// # Returns
    ///
    /// * `RestartOutcome::NotFound` if no handle with `worker_id` exists.
    /// * `RestartOutcome::Conflict` if that worker is already `Dying` —
    ///   some shutdown (this restart's own retry, or `shutdown_all()`) is
    ///   already in flight for it.
    /// * `RestartOutcome::Accepted(new_handle)` once the replacement has
    ///   been spawned and spliced into the slot. "Accepted" mirrors
    ///   `cancel_job()`'s own `202`-not-`200` framing (`ANVILML_DESIGN.md
    ///   §13.5`) — spawning is async under the hood (`spawn_worker()`
    ///   only waits for the `tokio::spawn()` call to register, not for
    ///   the new generation to reach `Ready`/`Idle`), so the caller gets
    ///   "the restart was accepted and is proceeding," not "the worker is
    ///   ready again."
    ///
    /// # Errors
    ///
    /// Propagates whatever `spawn_worker()` returns, including
    /// `AnvilError::Internal` if (unreachably, in practice — a worker
    /// existing at all means `spawn_all_impl()` already ran once)
    /// `self.spawn_config` was never populated.
    pub async fn restart_worker(&self, worker_id: &str) -> Result<RestartOutcome, AnvilError> {
        let _guard = self.restart_lock.lock().await;

        // Locate the slot and snapshot its current handle. Brief,
        // synchronous critical section — no .await while the lock is
        // held.
        let found = {
            let handles = self
                .handles
                .read()
                .expect("WorkerPool handles lock poisoned");
            handles
                .iter()
                .position(|h| h.worker_id == worker_id)
                .map(|pos| (pos, handles[pos].clone()))
        };
        let Some((pos, old_handle)) = found else {
            return Ok(RestartOutcome::NotFound);
        };

        // Already shutting down (this worker's own prior request, or a
        // concurrent shutdown_all()) — a second restart on top of that is
        // a conflicting operation, not a queued-up retry.
        if old_handle.status().await == WorkerStatus::Dying {
            return Ok(RestartOutcome::Conflict);
        }

        let Some(device) = self.devices.get(pos).cloned() else {
            // Can't happen in practice — every handle is pushed alongside
            // its device at the same index by spawn_all_impl()'s thin
            // loop — but surfaced as an Internal error rather than a
            // panic/unwrap, matching this crate's established
            // never-panic-on-a-live-request discipline.
            return Err(AnvilError::Internal(format!(
                "worker {worker_id} has a handle at index {pos} but no \
                 corresponding device — pool's handles/devices invariant violated"
            )));
        };

        // Request graceful shutdown on the ORIGINAL handle still sitting
        // in self.handles[pos] — old_handle (the snapshot above) is a
        // clone, and clones never carry shutdown_tx (see
        // WorkerHandle::clone()'s own doc comment), so calling
        // request_shutdown() on old_handle would silently do nothing.
        {
            let mut handles = self
                .handles
                .write()
                .expect("WorkerPool handles lock poisoned");
            handles[pos].request_shutdown();
        }

        // Await the outgoing generation's exit, bounded by the same
        // production graceful-shutdown timeout every other exit path
        // uses. old_handle shares the same underlying join_handle Arc as
        // self.handles[pos] (WorkerHandle::clone() clones that Arc, not
        // the task itself), so awaiting on this clone observes the same
        // task exiting.
        old_handle
            .await_exit(DEFAULT_GRACEFUL_SHUTDOWN_TIMEOUT)
            .await;

        // Spawn the replacement. spawn_worker() appends it to the tail of
        // self.handles (its own documented contract) — restart_lock
        // guarantees no other spawn_worker() call (from a concurrent
        // restart_worker()) can interleave here, so that tail entry is
        // unambiguously this call's own.
        let new_handle = self.spawn_worker(device).await?;
        {
            let mut handles = self
                .handles
                .write()
                .expect("WorkerPool handles lock poisoned");
            // Move the popped tail entry itself into `pos` — NOT another
            // clone of `new_handle`. `spawn_worker()` already fixed this
            // exact mistake once for its own push (see that method's own
            // doc comment): `new_handle` (this call's return value) is a
            // clone with no shutdown_tx/force_shutdown_tx, but the tail
            // entry it just pushed onto self.handles is the real
            // original carrying both. Splicing in another clone here
            // would leave the pool's own copy for this slot permanently
            // unable to request shutdown — the same bug, one call site
            // later.
            let spawned = handles
                .pop()
                .expect("spawn_worker() must have pushed a handle onto the tail");
            handles[pos] = spawned;
        }

        Ok(RestartOutcome::Accepted(new_handle))
    }

    /// Gracefully shut down every worker in the pool, per
    /// `ANVILML_DESIGN.md §19.3` steps 2-4.
    ///
    /// 1. `request_shutdown()` on every handle — each worker's own
    ///    `ManagedWorker::run()` then runs its own graceful-then-force-kill
    ///    sequence internally (`graceful_shutdown_child()`, added in the
    ///    orphan-cleanup work preceding this task).
    /// 2. Await every handle's `run()` task, bounded by **one shared**
    ///    `timeout` for the whole pool — not `timeout` per worker,
    ///    sequentially, which would let total wall-clock shutdown time
    ///    scale with worker count instead of staying bounded at `timeout`
    ///    overall. Matches §19.3's own framing: "server waits up to 30
    ///    seconds for workers \[plural\] to exit" describes one shared
    ///    window, not one per worker. Each handle is cloned into its own
    ///    spawned task so all of them run concurrently — cloning is fine
    ///    for *this* phase specifically, since `await_exit()` only needs
    ///    `&self`.
    /// 3. Any handle that didn't exit within `timeout` is a straggler.
    ///    Stragglers are force-shut-down via `force_shutdown()` — **on
    ///    their original handle**, not a clone (`oneshot::Sender` isn't
    ///    `Clone`, matching `shutdown_tx`'s own, already-established
    ///    pattern — see `WorkerHandle`'s `Clone` impl) — then given a
    ///    short, bounded grace period for the force-kill itself to
    ///    resolve. `force_kill_child()` (what actually runs once
    ///    `force_shutdown()`'s signal reaches
    ///    `graceful_shutdown_child()`) is fast — a direct `.kill().await`,
    ///    no internal bounded wait of its own — so this grace period
    ///    exists only to avoid hanging forever if something is more
    ///    deeply wrong, not because force-killing is expected to be slow.
    ///    Only if even *that* bound elapses does this fall back to
    ///    `abort()` — a true last resort at that point, since something
    ///    has gone wrong beyond what this method can reasonably account
    ///    for. See `WorkerHandle::abort()`'s own doc comment for why
    ///    externally cancelling the task is not, on its own, an adequate
    ///    substitute for the `force_shutdown()` path above.
    /// 4. The bridge's own reader and writer tasks are aborted last, after
    ///    every worker's `run()` task has exited or been aborted — nothing
    ///    should still need `transport.send()` (`KeepaliveWatchdog`) or the
    ///    bridge's own event routing by that point.
    pub async fn shutdown_all(&mut self, timeout: Duration) {
        // `&mut self` already guarantees exclusive access, so `get_mut()`
        // bypasses the lock entirely (no blocking, nothing async needed
        // just to read/mutate it) — the lock only matters for the
        // concurrent-`&self` paths `spawn_worker()`/`restart_worker()`
        // (P18-D2/P18-D3) use.
        let handles = self
            .handles
            .get_mut()
            .expect("WorkerPool handles lock poisoned");

        for handle in handles.iter_mut() {
            handle.request_shutdown();
        }

        // Phase 1: await every handle concurrently, bounded by one shared
        // timeout. Report stragglers back by index rather than acting on
        // them directly inside each spawned task — force_shutdown() needs
        // the ORIGINAL handle's own force_shutdown_tx, which a clone
        // doesn't have.
        let (report_tx, mut report_rx) = mpsc::channel(handles.len().max(1));
        for (index, handle) in handles.iter().cloned().enumerate() {
            let report_tx = report_tx.clone();
            tokio::spawn(async move {
                if !handle.await_exit(timeout).await {
                    let _ = report_tx.send(index).await;
                }
            });
        }
        drop(report_tx);
        let mut stragglers = Vec::new();
        while let Some(index) = report_rx.recv().await {
            stragglers.push(index);
        }

        // Phase 2: force-shut-down every straggler via its ORIGINAL
        // handle. Sending the signal itself is fast (no .await inside
        // force_shutdown() at all), so doing this sequentially across
        // stragglers is fine — the actual waiting happens concurrently
        // next.
        for &index in &stragglers {
            let handle = &mut handles[index];
            tracing::warn!(
                worker_id = %handle.worker_id,
                "worker did not exit within shutdown_all()'s timeout — forcing"
            );
            handle.force_shutdown();
        }

        // Phase 3: await every straggler's force-kill concurrently,
        // bounded by one shared grace period — same reasoning as phase 1
        // for why this must be concurrent, not sequential.
        let mut force_tasks = Vec::with_capacity(stragglers.len());
        for &index in &stragglers {
            let handle = handles[index].clone();
            force_tasks.push(tokio::spawn(async move {
                if !handle.await_exit(Duration::from_secs(5)).await {
                    tracing::error!(
                        worker_id = %handle.worker_id,
                        "worker still did not exit after force_shutdown() — \
                         aborting task as a last resort (may leak its child process)"
                    );
                    handle.abort().await;
                }
            }));
        }
        for task in force_tasks {
            let _ = task.await;
        }

        self.bridge_handles.0.abort();
        self.bridge_handles.1.abort();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Exactly `"1"` forces mock mode — the one documented, working case.
    #[test]
    fn test_force_mock_exactly_one_is_true() {
        assert!(force_mock_from_env_value(Some("1")));
    }

    /// Unset (`None`, matching `std::env::var(...).ok()` on a missing var)
    /// has no effect, per `ENVIRONMENT.md §3.5`'s own "unset = no effect".
    #[test]
    fn test_force_mock_unset_is_false() {
        assert!(!force_mock_from_env_value(None));
    }

    /// Every other value is also "no effect", matching the table's binary
    /// framing exactly — not a general-purpose truthy-string parser. This
    /// is the specific class of mistake worth guarding against: it would
    /// be easy to accidentally accept "true"/"yes"/"0" as truthy without
    /// ever noticing, since ENVIRONMENT.md's own contract is undocumented
    /// for anything other than "1" and "unset".
    #[test]
    fn test_force_mock_other_values_are_false() {
        for value in ["0", "true", "TRUE", "yes", "", "2", " 1", "1 "] {
            assert!(
                !force_mock_from_env_value(Some(value)),
                "value {value:?} should not force mock mode"
            );
        }
    }
}
