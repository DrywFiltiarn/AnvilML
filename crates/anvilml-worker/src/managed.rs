//! A cheap, `Clone`-able handle for interacting with a worker's lifecycle,
//! and the full lifecycle manager for a single Python worker subprocess.
//!
//! Two types are defined:
//!
//! - `WorkerHandle` — a cheap, `Clone`-able handle for interacting with a worker's
//!   lifecycle. Each handle owns an `Arc`-reference to the worker's status lock,
//!   a `oneshot::Sender` for requesting shutdown, and an `Arc<Mutex<Option<JoinHandle>>>`
//!   for tracking the worker task.
//! - `ManagedWorker` — the full lifecycle task that owns a worker's lifetime across
//!   every respawn generation. As of P8-E6, `run_once()` calls `demux.register()`
//!   itself immediately after each successful spawn (gen 0 and every respawn
//!   alike — see `run_once()`'s doc comment), and `run()`'s outer loop calls
//!   `demux.deregister()` on every exit from a generation. Before P8-E6, the
//!   very first registration was the external caller's responsibility alone;
//!   that external pre-registration is still harmless if a caller still does it
//!   (`Demux::register()` is documented idempotent), but is no longer required —
//!   `run_once()`'s own registration now covers gen 0 too.
//!
//! The `WorkerHandle` is a lightweight view into shared state; `ManagedWorker`
//! is the consuming task that runs the lifecycle loop.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::io::AsyncBufReadExt;
use tokio::sync::RwLock;
use tokio::sync::oneshot;

use anvilml_core::NodeTypeRegistry;
use anvilml_core::types::worker::WorkerStatus;
use anvilml_ipc::WorkerEvent;
use anvilml_ipc::WorkerMessage;

#[cfg(windows)]
use crate::JobObjectGuard;
use crate::demux::Demux;
use crate::keepalive::{KeepaliveWatchdog, RouterTransportAdapter};
use crate::respawn::RespawnPolicy;
use crate::spawn::WorkerSpawner;
use anvilml_ipc::RouterTransport;

/// Best-effort parse of a line this worker printed to its own
/// stdout/stderr into its Python `LEVELNAME` and the remainder of the
/// line after it.
///
/// `worker_main.py`'s `if __name__ == "__main__":` block configures
/// `logging.basicConfig(format="%(asctime)s %(levelname)s %(name)s: %(message)s")`,
/// so every line that actually went through Python's `logging` module (as
/// opposed to a bare `print()`, or an uncaught traceback dumped straight to
/// stderr by the interpreter) has the shape:
///
/// ```text
/// 2026-07-15 10:24:17,145 INFO __main__: worker: starting in real mode
/// ^---- token 0 --------^ ^-2-^ ^------------ remainder ------------^
///           ^--- token 1 (date+time) ---^
/// ```
///
/// Returns `None` for anything that doesn't match — most notably an
/// uncaught Python traceback, which is exactly the case the caller's
/// stderr-forwarding logic exists to make impossible to miss, so callers
/// should keep a `WARN`-or-higher fallback for the unparseable case rather
/// than silently defaulting to something quieter (and should log the full,
/// untouched line in that case, not a mangled remainder).
///
/// The remainder (everything after `LEVELNAME `, e.g.
/// `"__main__: worker: starting in real mode"`) is what callers should log
/// as the event's own message when a level was parsed: the date/time and
/// level are otherwise duplicated in the log output, since the Rust event
/// this produces already carries its own timestamp and (as of the level
/// this function returns) the correct level — logging the raw line in
/// full alongside that would show the same information twice.
///
/// Uses `split_once` rather than `str::split_whitespace().as_str()` (added
/// in a newer Rust than this workspace's MSRV assumes) so this doesn't
/// depend on a specific toolchain version.
fn parse_python_log_line(line: &str) -> Option<(tracing::Level, &str)> {
    let rest = line.trim_start();
    let (_date, rest) = rest.split_once(char::is_whitespace)?;
    let rest = rest.trim_start();
    let (_time, rest) = rest.split_once(char::is_whitespace)?;
    let rest = rest.trim_start();
    let (level_token, rest) = rest.split_once(char::is_whitespace)?;
    let level = match level_token {
        "DEBUG" => tracing::Level::DEBUG,
        "INFO" => tracing::Level::INFO,
        "WARNING" => tracing::Level::WARN,
        "ERROR" | "CRITICAL" => tracing::Level::ERROR,
        _ => return None,
    };
    Some((level, rest.trim_start()))
}

/// Production default for the Initializing→Idle grace period.
///
/// Per `ANVILML_DESIGN.md §9.2`: "A worker that fails to reach `Idle` within
/// 60 seconds is killed and respawned." This is real grace time for the
/// Python subprocess to load a diffusion model onto the GPU (CUDA init +
/// weight loading) — not an arbitrary value, and not one production callers
/// should shorten. It exists as an explicit constant (rather than being
/// buried inline in `run()`) so every caller of `ManagedWorker::new()` states
/// its intent plainly: `ManagedWorker::new(..., DEFAULT_INIT_TIMEOUT)` for
/// production, or a short `Duration` for tests exercising the timeout path
/// itself.
pub const DEFAULT_INIT_TIMEOUT: Duration = Duration::from_secs(60);

/// Production default for the keepalive watchdog ping interval.
///
/// Per `ANVILML_DESIGN.md §9.2`: "Keepalive pings every 30 seconds."
/// Tests override with a short duration so they don't block on real seconds.
pub const DEFAULT_WATCHDOG_PING_INTERVAL: Duration = Duration::from_secs(30);

/// Production default for the keepalive watchdog pong timeout.
///
/// Per `ANVILML_DESIGN.md §9.2`: "no pong within 10 seconds → dead."
/// Tests override with a short duration so they don't block on real seconds.
pub const DEFAULT_WATCHDOG_PONG_TIMEOUT: Duration = Duration::from_secs(10);

/// Production default bound for a graceful shutdown to complete on its own.
///
/// Per `ANVILML_DESIGN.md §19.3`: "Server waits up to 30 seconds for
/// workers to exit. Any worker not exited is killed." That section
/// describes this at the pool level ("workers", plural) — `WorkerPool`
/// doesn't exist yet (P8-G1). `ManagedWorker::run()` enforcing this same
/// 30s bound per-worker is intended to be the building block a future
/// `WorkerPool::shutdown_all()` awaits against, inheriting §19.3's overall
/// behavior as an emergent property of each worker's own correct
/// implementation — not something P8-G1 needs to separately re-implement
/// or duplicate a timeout for.
pub const DEFAULT_GRACEFUL_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(30);

/// The reason a single generation (`run_once()` call) ended.
///
/// Consumed by `run()`'s outer loop to decide whether to respawn or stop for
/// good. Only `Crashed` carries a respawn decision — the other three variants
/// are unconditional: `run()` never respawns after them, regardless of what
/// `RespawnPolicy` would otherwise allow. This matches this task's own scope
/// exactly: two crash sources (transport recv error, `KeepaliveWatchdog`'s
/// death signal) are respawn-eligible; graceful shutdown, the `Initializing`
/// timeout, and an explicit `Dying` event are not, unchanged from their
/// pre-P8-E6 behavior.
///
/// `pub`, not `pub(crate)`: `run_once()` itself is `pub(crate)` (matching
/// this task's specified visibility), but its `test-utils`-gated public
/// wrapper `run_once_for_test()` returns `RunOutcome` too — a `pub` function
/// cannot have a `pub(crate)` type in its signature (Rust's
/// `private_interfaces` lint), so this enum must be visible wherever that
/// wrapper is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunOutcome {
    /// `shutdown_rx` fired. No respawn, ever — this is supervisor-initiated.
    ShutdownRequested,
    /// `init_timeout` elapsed without a `Ready` event. No respawn — a worker
    /// that never reaches `Ready` may indicate a persistent environment
    /// problem a respawn wouldn't fix; this task's scope doesn't list this
    /// path as crash-eligible.
    InitTimedOut,
    /// The worker reported its own termination (`Dying` event). No respawn —
    /// same reasoning as `InitTimedOut`: not listed as crash-eligible.
    WorkerReportedDying,
    /// A crash was detected: `spawner.spawn()` itself failing, a transport
    /// recv error, or the keepalive watchdog's death signal. All three are
    /// treated identically here — `attempt_history` has already been
    /// appended and `RespawnPolicy::should_respawn()` already consulted
    /// before this variant is constructed; `should_respawn` carries that
    /// decision.
    Crashed { should_respawn: bool },
}

/// A cheap, `Clone`-able handle for interacting with a worker's lifecycle.
///
/// Each `WorkerHandle` owns an `Arc`-reference to the worker's status lock,
/// a pair of `oneshot::Sender`s for requesting graceful and forced
/// shutdown, and an `Arc<Mutex<Option<JoinHandle>>>` for tracking the
/// worker task. Cloning a handle produces a new handle that shares the
/// same status lock and join handle — both clones observe the same
/// status — but **not** the same ability to trigger shutdown: neither
/// `oneshot::Sender` is `Clone`-able, so only the original handle can call
/// `request_shutdown()`/`force_shutdown()`; a clone's calls to either are
/// silent no-ops. (This corrects an earlier version of this doc comment,
/// which claimed clones retain shutdown ability — contradicted by the
/// `Clone` impl's own, already-correct behavior below.) The `worker_id`
/// field is copied (not shared) across clones.
///
/// This handle does **not** own the `ManagedWorker` struct itself — it is a lightweight
/// view into the worker's shared state, designed to be freely shared across tasks and
/// API handlers without `Arc`-wrapping the full worker.
pub struct WorkerHandle {
    /// Stable worker identity — the bare device index as a string (e.g. `"0"`).
    /// Copied, not shared, across clones.
    pub worker_id: String,

    /// Shared status lock — all clones read from the same lock.
    /// Private: consumers must use `status()` to read the current state.
    status: Arc<RwLock<WorkerStatus>>,

    /// Optional shutdown trigger — `take()`n on the first call to `request_shutdown()`,
    /// making the operation idempotent (second call is a no-op).
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,

    /// Optional force-shutdown trigger — `take()`n on the first call to
    /// `force_shutdown()`, matching `shutdown_tx`'s own idempotency
    /// pattern. See `ManagedWorker::graceful_shutdown_child()`'s own doc
    /// comment for why this exists as a signal distinct from
    /// `shutdown_tx`, and `abort()`'s own doc comment for why externally
    /// cancelling the task is not an adequate substitute.
    force_shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,

    /// Shared join handle wrapper — allows the pool to extract and await the handle
    /// during shutdown with a bounded timeout.
    join_handle: Arc<tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

// Neither shutdown_tx nor force_shutdown_tx (both oneshot::Sender) is
// Clone — clones cannot request either kind of shutdown. Only the
// original handle retains the ability to trigger them.
impl Clone for WorkerHandle {
    fn clone(&self) -> Self {
        Self {
            worker_id: self.worker_id.clone(),
            status: Arc::clone(&self.status),
            // Neither sender can be taken — the clone loses the ability to
            // request either kind of shutdown, preserving the invariant
            // that only the original handle can trigger them.
            shutdown_tx: None,
            force_shutdown_tx: None,
            join_handle: Arc::clone(&self.join_handle),
        }
    }
}

impl WorkerHandle {
    /// Construct a new `WorkerHandle` from its component parts.
    ///
    /// # Arguments
    ///
    /// * `worker_id` — Stable worker identity string (e.g. `"0"`). Copied into the handle.
    /// * `status` — Shared `Arc<RwLock<WorkerStatus>>` for the worker's lifecycle state.
    ///   All clones of this handle share this same lock.
    /// * `shutdown_tx` — Optional `oneshot::Sender` used to signal the worker to shut down
    ///   gracefully. If `None`, `request_shutdown()` becomes a no-op.
    /// * `force_shutdown_tx` — Optional `oneshot::Sender` used to signal the worker to skip
    ///   the remainder of its own bounded graceful-shutdown wait and force-kill immediately.
    ///   Only meaningful after `request_shutdown()` has already been called — see
    ///   `ManagedWorker::run()`'s own doc comment on its `force_shutdown_rx` parameter.
    ///   If `None`, `force_shutdown()` becomes a no-op.
    /// * `join_handle` — Shared `Arc<Mutex<Option<JoinHandle>>>` for tracking the worker task.
    pub fn new(
        worker_id: String,
        status: Arc<RwLock<WorkerStatus>>,
        shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
        force_shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
        join_handle: Arc<tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>>,
    ) -> Self {
        Self {
            worker_id,
            status,
            shutdown_tx,
            force_shutdown_tx,
            join_handle,
        }
    }

    /// Read the worker's current lifecycle state.
    ///
    /// Acquires a read lock on the shared status, copies the `Copy` value,
    /// and releases the lock. The caller never holds the lock.
    pub async fn status(&self) -> WorkerStatus {
        *self.status.read().await
    }

    /// Set the worker's lifecycle state.
    ///
    /// Acquires a write lock on the shared status, overwrites the stored value,
    /// and releases the lock. This is the only public mutator on `WorkerHandle`.
    ///
    /// # Arguments
    ///
    /// * `new` — The new `WorkerStatus` value to set.
    pub async fn set_status(&self, new: WorkerStatus) {
        *self.status.write().await = new;
    }

    /// Request the worker to shut down gracefully.
    ///
    /// Takes the `oneshot::Sender` from `shutdown_tx` and sends `()` to the receiver.
    /// If `shutdown_tx` is already `None` (already called), this is a no-op — making
    /// the method idempotent. The result of `tx.send(())` is ignored because the
    /// receiver may already have been dropped (worker already exited).
    pub fn request_shutdown(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            // Ignore the result — the receiver may already be dropped if the worker
            // has already exited. This makes request_shutdown idempotent.
            let _ = tx.send(());
        }
    }

    /// Signal the worker to skip the remainder of its own bounded
    /// graceful-shutdown wait and force-kill its child immediately.
    ///
    /// Only meaningful *after* `request_shutdown()` — `graceful_shutdown_child()`
    /// (where `force_shutdown_rx` is actually consulted) only runs once
    /// `ShutdownRequested` has already fired; sending this signal first,
    /// on its own, does nothing, since nothing is listening for it yet.
    /// See `ManagedWorker::graceful_shutdown_child()`'s own doc comment
    /// for the full reasoning behind this signal's existence — briefly,
    /// it exists so `WorkerPool::shutdown_all()` can make a specific
    /// straggling worker force-kill immediately without externally
    /// aborting its `run()` task, which would skip that method's own
    /// cleanup and risk leaking the child process.
    ///
    /// Idempotent, matching `request_shutdown()`'s own pattern: `take()`s
    /// `force_shutdown_tx`, so a second call is a no-op. The result of
    /// `tx.send(())` is ignored because the receiver may already have
    /// been dropped (the worker already exited on its own, or the
    /// graceful wait already completed by the time this fires).
    pub fn force_shutdown(&mut self) {
        if let Some(tx) = self.force_shutdown_tx.take() {
            let _ = tx.send(());
        }
    }

    /// Wait for the worker's `run()` task to exit, bounded by `timeout`.
    ///
    /// Returns `true` if the task exited within the bound, `false` if the
    /// timeout elapsed first (the task is still running). Used by
    /// `WorkerPool::shutdown_all()` to detect stragglers that need to be
    /// force-killed per `ANVILML_DESIGN.md §19.3` step 4.
    ///
    /// Note this only awaits the `run()` *task* — it does not, by itself,
    /// force-kill anything. Each `ManagedWorker` generation already runs
    /// its own graceful-then-force-kill sequence internally on every exit
    /// path (`graceful_shutdown_child()`/`force_kill_child()`, added in the
    /// orphan-cleanup work preceding this task), so a `request_shutdown()`
    /// call followed by this method returning `true` within a reasonable
    /// bound means the underlying child process is already gone too, not
    /// merely that the Rust task ended.
    ///
    /// Idempotent: the `JoinHandle` is polled via `.as_mut()`, not taken,
    /// so a second call after a successful await resolves immediately
    /// (`true`) rather than panicking on an already-consumed handle.
    pub async fn await_exit(&self, timeout: Duration) -> bool {
        let mut guard = self.join_handle.lock().await;
        let Some(handle) = guard.as_mut() else {
            // Already awaited to completion by an earlier call, or never
            // spawned at all — either way, there's nothing left to wait on.
            return true;
        };
        match tokio::time::timeout(timeout, handle).await {
            Ok(_) => {
                *guard = None;
                true
            }
            Err(_) => false,
        }
    }

    /// Forcibly cancel the worker's `run()` task.
    ///
    /// Last-resort fallback for `WorkerPool::shutdown_all()` per
    /// `ANVILML_DESIGN.md §19.3` step 4 ("any worker not exited is
    /// killed"), used only when `await_exit()` has already timed out. This
    /// is a genuine safety net, not the primary shutdown mechanism —
    /// `tokio::task::JoinHandle::abort()` cancels the task at its next
    /// `.await` point without running any of `ManagedWorker`'s own cleanup
    /// logic, so a task aborted here can leave its child process orphaned
    /// exactly like the pre-orphan-cleanup-fix bugs this session already
    /// found and fixed elsewhere. In practice this should be rare: it only
    /// fires if a per-worker `graceful_shutdown_timeout` was configured
    /// longer than the pool-level `shutdown_all()` timeout, or a task is
    /// genuinely stuck.
    pub async fn abort(&self) {
        let guard = self.join_handle.lock().await;
        if let Some(handle) = guard.as_ref() {
            handle.abort();
        }
    }
}

/// Full lifecycle manager for a single Python worker subprocess.
///
/// `ManagedWorker` owns the worker's stable identity, a shared `RouterTransport` for
/// receiving events from the worker, a shared `Demux` for the routing table, and a
/// `RespawnPolicy` for crash-recovery decisions. It consumes `self` in `run()`, which
/// owns the worker's entire lifetime across every respawn generation.
///
/// The lifecycle, per generation:
/// 1. **Initializing** — `run_once()` spawns a fresh child process (gen 0 and every
///    respawn alike, via the same code path) and registers with the demux, then sets
///    this status. `init_timeout` (60s in production, via `DEFAULT_INIT_TIMEOUT`;
///    configurable per-instance, see `new()`) guards this state.
/// 2. **Idle** — the worker sent a `Ready` event; waiting for job assignment.
/// 3. **Busy** — the worker is executing a job.
/// 4. **Dying** — the worker received a shutdown signal or sent a `Dying` event.
/// 5. **Dead** — the worker has terminated.
/// 6. **Respawning** — set by `run()`'s outer loop between a respawn-eligible crash
///    and the next generation's spawn attempt, during the backoff delay.
///
/// `demux.register()` is called by `run_once()` itself, immediately after each
/// successful spawn. `demux.deregister()` is called by `run()`'s outer loop after
/// every generation exits, regardless of outcome. See `run()`'s and `run_once()`'s
/// own doc comments for the full per-generation contract.
///
/// `run()` is the consuming method — it takes `self` by value and returns only after
/// the worker has exited for good (no further respawn) and been deregistered.
pub struct ManagedWorker {
    /// Stable worker identity — the bare device index as a string (e.g. `"0"`).
    /// Used for register/deregister and as the spawned subprocess's expected
    /// ZeroMQ DEALER identity.
    worker_id: String,

    /// Shared ROUTER socket transport for receiving events from the worker.
    ///
    /// This is an `Arc` so the test can share the same transport with the worker's
    /// `ManagedWorker` and the event-sending test code.
    transport: Arc<RouterTransport>,

    /// Shared routing table.
    ///
    /// Wrapped in `Arc` so the test can inspect it after `run()` consumes the
    /// `ManagedWorker` and returns. `run_once()` registers on every successful
    /// spawn; `run()`'s outer loop deregisters after every generation exits.
    demux: Arc<Demux>,

    /// Dynamic node type registry — populated from worker Ready events.
    ///
    /// `register_all()` is called in `handle_event()` on every Ready event.
    node_registry: Arc<NodeTypeRegistry>,

    /// Shared status lock — used by `run()` to track the worker's lifecycle state.
    ///
    /// The pool creates this and passes it into `ManagedWorker`. The `WorkerHandle`
    /// that the pool returns to callers shares this same lock.
    status: Arc<RwLock<WorkerStatus>>,

    /// Crash-recovery backoff policy.
    ///
    /// Decides whether a crashed worker may be respawned based on the count of
    /// recent crash attempts within a sliding window. Consulted on every crash
    /// (spawn failure, transport recv error, or keepalive watchdog death signal)
    /// to determine if respawn is permissible, and for the backoff delay between
    /// a respawn-eligible crash and the next spawn attempt.
    respawn_policy: RespawnPolicy,

    /// Timestamps of crash transitions.
    ///
    /// Appended on every crash (spawn failure, transport recv error, or watchdog
    /// death signal). Consulted by `RespawnPolicy::should_respawn()` to decide
    /// whether a respawn is permissible. Persists across generations — this is
    /// the whole point of the sliding-window backoff policy.
    attempt_history: Vec<Instant>,

    /// Grace period for the Initializing→Idle transition.
    ///
    /// Production callers should pass `DEFAULT_INIT_TIMEOUT` (60s per
    /// `ANVILML_DESIGN.md §9.2`). Tests exercising the timeout path itself
    /// pass a short duration so the test doesn't block on a real 60s wait —
    /// see `KeepaliveWatchdog::new()`'s `ping_interval`/`pong_timeout` params
    /// for the same pattern elsewhere in this crate.
    init_timeout: Duration,

    /// Channel sender for forwarding Pong events to the watchdog.
    ///
    /// Each `WorkerEvent::Pong` from `handle_event()` is sent here. The watchdog
    /// filters for matching sequence numbers internally. A closed or full channel
    /// is not an error — the watchdog will eventually timeout and declare the
    /// worker dead, which is the correct failure mode. Reassigned to a fresh
    /// channel by `run_once()` at the start of phase 2 (once `Ready` is
    /// received), tied to that generation's freshly-spawned watchdog task —
    /// see `run_once()`'s doc comment for why the watchdog doesn't exist
    /// yet during phase 1. A stray `Pong` received during phase 1 is sent
    /// into whatever this field currently holds (the caller-provided sender
    /// on gen 0, or the previous generation's now-orphaned watchdog channel
    /// on a respawn) — harmless either way, since an unmatched seq is
    /// silently ignored by `wait_for_matching_pong()`.
    pong_tx: tokio::sync::mpsc::Sender<anvilml_ipc::WorkerEvent>,

    /// Time between consecutive Ping messages sent by the watchdog.
    ///
    /// Production default: 30 seconds per `ANVILML_DESIGN.md §9.2`.
    /// Tests override with a short duration.
    watchdog_ping_interval: Duration,

    /// Maximum time to wait for a matching Pong after sending a Ping.
    ///
    /// Production default: 10 seconds per `ANVILML_DESIGN.md §9.2`.
    /// Tests override with a short duration.
    watchdog_pong_timeout: Duration,

    /// Maximum time to wait for the worker to exit on its own after
    /// sending `WorkerMessage::Shutdown`, before force-killing it.
    ///
    /// Production default: 30 seconds per `ANVILML_DESIGN.md §19.3`.
    /// Tests override with a short duration.
    graceful_shutdown_timeout: Duration,

    /// Root of the Python virtual environment. Passed to `spawner.spawn()` on
    /// every generation — gen 0 and every respawn alike.
    venv_path: PathBuf,

    /// Environment variables for the worker subprocess.
    ///
    /// Built once (`WorkerEnv::build`) and held static across respawns.
    /// `WorkerSpawner::spawn()` consumes its `env` argument by value, so
    /// `run_once()` clones this at each call site rather than moving it out.
    env: HashMap<String, String>,

    /// Spawns the worker subprocess. `Arc<dyn ...>` so production
    /// (`ProcessWorkerSpawner`) and tests (a mock) share one field type.
    spawner: Arc<dyn WorkerSpawner>,

    /// The currently-spawned child process, if any.
    ///
    /// Set by `run_once()` immediately after a successful spawn, gen 0 and
    /// every respawn alike — this is what "Child stored, not orphaned" means:
    /// without this field, the `Child` returned by `spawner.spawn()` would be
    /// a temporary with no owner past the end of the expression that spawned
    /// it, and tokio drops an orphaned `Child` without killing the OS process
    /// it wraps. `None` only before the very first spawn attempt.
    ///
    /// On respawn, `run_once()` kills the previous generation's child
    /// (`.kill().await`) before overwriting this field — see `run_once()`'s
    /// own doc comment for why: overwriting without killing first would
    /// silently drop (not terminate) whatever this held, on every respawn.
    child: Option<tokio::process::Child>,

    /// Windows Job Object for orphan-cleanup (P8-E7). Absent entirely on
    /// non-Windows targets — no field, no behavior change there.
    ///
    /// One guard persists across every generation of this worker's
    /// lifetime — it is not recreated per respawn. `run_once()` calls
    /// `assign_process(&child)` on it after every successful spawn (gen 0
    /// and every respawn alike), reassigning the guard's kill-on-close
    /// protection to whichever child is currently live. A terminated
    /// process is automatically removed from a job object's process list
    /// by the OS, so this reassignment is always a clean "empty job
    /// object accepts a never-before-assigned process" case, not the
    /// cross-guard rejection scenario `spawn_tests.rs`'s
    /// `test_double_assignment_fails_cleanly` exercises — see this
    /// field's own construction in `new()` for why `JobObjectGuard::new()`
    /// failing doesn't fail construction of the whole `ManagedWorker`.
    #[cfg(windows)]
    job_guard: Option<JobObjectGuard>,
}

/// Construction parameters for `ManagedWorker::new()`.
///
/// Bundled into a config struct (rather than 12 positional parameters) as of
/// P8-E6 — named fields catch "wrong argument order" at compile time in a way
/// positional args of increasingly-similar types (multiple `Duration`s,
/// multiple `Arc<...>`s) do not. Fields are grouped by concern below, not
/// alphabetically or by arrival order — related fields sit together.
pub struct ManagedWorkerConfig {
    // --- Identity ---
    /// Stable worker identity (e.g. `"0"`, `"1"`), also used as the ZeroMQ
    /// DEALER identity the worker subprocess connects with.
    pub worker_id: String,

    // --- Shared infrastructure (constructed once, outside ManagedWorker) ---
    /// The ROUTER transport, shared across all workers in the pool.
    pub transport: Arc<RouterTransport>,
    /// The event-routing table, shared across all workers in the pool.
    pub demux: Arc<Demux>,
    /// Externally-readable status handle — cloned into `WorkerHandle` so
    /// callers can observe status without going through `ManagedWorker`.
    pub status: Arc<RwLock<WorkerStatus>>,
    /// Dynamic node type registry — populated from worker Ready events.
    ///
    /// `register_all()` is called exactly once per Ready event (in `handle_event()`),
    /// replacing the prior contents. This is the one and only call site that ever
    /// populates the registry, per ANVILML_DESIGN.md §10.2.
    pub node_registry: Arc<NodeTypeRegistry>,

    // --- Crash recovery ---
    /// Backoff/max-attempt policy consulted on every crash (spawn failure,
    /// transport error, or `KeepaliveWatchdog` death signal).
    pub respawn_policy: RespawnPolicy,
    /// Grace period for the Initializing→Idle transition. Production callers
    /// pass `DEFAULT_INIT_TIMEOUT` (60s, `ANVILML_DESIGN.md §9.2`).
    pub init_timeout: Duration,

    // --- Keepalive watchdog ---
    /// Forwarding channel for `Pong` events, read by the spawned
    /// `KeepaliveWatchdog` task via its paired receiver.
    pub pong_tx: tokio::sync::mpsc::Sender<anvilml_ipc::WorkerEvent>,
    /// How often the watchdog sends a Ping. Production default: 30s.
    pub watchdog_ping_interval: Duration,
    /// How long the watchdog waits for a matching Pong before declaring the
    /// worker dead. Production default: 10s.
    pub watchdog_pong_timeout: Duration,

    // --- Shutdown ---
    /// Maximum time to wait for the worker to exit on its own after
    /// sending `WorkerMessage::Shutdown`, before force-killing it.
    /// Production default: 30s (`ANVILML_DESIGN.md §19.3`).
    pub graceful_shutdown_timeout: Duration,

    // --- Respawn / process spawning (P8-E6) ---
    /// Root of the Python virtual environment. Passed to `spawner.spawn()`
    /// on every generation — gen 0 and every respawn alike.
    pub venv_path: PathBuf,
    /// Environment variables for the worker subprocess. Built once
    /// (`WorkerEnv::build`) and held static across respawns — `spawn()`
    /// consumes its `env` argument by value, so each call site clones this.
    pub env: HashMap<String, String>,
    /// Spawns the worker subprocess. `Arc<dyn ...>` so production
    /// (`ProcessWorkerSpawner`) and tests (a mock) share one field type.
    pub spawner: Arc<dyn WorkerSpawner>,
}

impl ManagedWorker {
    /// Construct a new `ManagedWorker` from a `ManagedWorkerConfig`.
    ///
    /// The worker does not need to be pre-registered in the demux by the
    /// caller — `run_once()` registers itself immediately after every
    /// successful spawn, gen 0 included. A caller that still registers before
    /// spawning `run()` (matching the pre-P8-E6 contract) is harmless:
    /// `Demux::register()` is documented idempotent, and `run_once()`'s own
    /// registration simply overwrites it with an equivalent entry.
    pub fn new(config: ManagedWorkerConfig) -> Self {
        #[cfg(windows)]
        let job_guard = match JobObjectGuard::new() {
            Ok(guard) => Some(guard),
            Err(e) => {
                // Graceful degradation, not a construction failure: this
                // guard is defense-in-depth against orphaned processes if
                // the supervisor dies unexpectedly, not something the
                // worker's own correct operation depends on. Failing
                // ManagedWorker::new() itself over a job-object creation
                // failure (rare — resource exhaustion or unusual security
                // policy) would be a disproportionate response, and would
                // force a breaking Result<Self, _> signature change onto
                // every platform, including non-Windows ones where this
                // failure mode can't even occur.
                tracing::warn!(
                    error = %e,
                    "JobObjectGuard::new() failed — this worker's child \
                     processes will not be protected against orphaning if \
                     the supervisor dies unexpectedly"
                );
                None
            }
        };

        Self {
            worker_id: config.worker_id,
            transport: config.transport,
            demux: config.demux,
            status: config.status,
            node_registry: config.node_registry,
            respawn_policy: config.respawn_policy,
            attempt_history: Vec::new(),
            init_timeout: config.init_timeout,
            pong_tx: config.pong_tx,
            watchdog_ping_interval: config.watchdog_ping_interval,
            watchdog_pong_timeout: config.watchdog_pong_timeout,
            graceful_shutdown_timeout: config.graceful_shutdown_timeout,
            venv_path: config.venv_path,
            env: config.env,
            spawner: config.spawner,
            child: None,
            #[cfg(windows)]
            job_guard,
        }
    }

    /// Returns the number of crash attempts tracked in `attempt_history`.
    ///
    /// Each crash (spawn failure, transport recv error, or watchdog death
    /// signal) appends an `Instant` to the history. This accessor is
    /// primarily for testing — it lets callers verify that crash-attempt
    /// tracking is working correctly without exposing the internal
    /// `Vec<Instant>` directly.
    pub fn attempt_count(&self) -> usize {
        self.attempt_history.len()
    }

    /// Run a single generation of the worker's lifecycle: spawn, register,
    /// initialize, and process events until this generation ends.
    ///
    /// Called by `run()`'s outer loop once per generation — gen 0 and every
    /// respawn alike, via the exact same code path, per this task's own
    /// scope ("spawner.spawn() at top of every generation"). Takes and
    /// returns `self` by value (rather than `&mut self`) because ownership
    /// needs to survive across the `.await` points inside, matching the
    /// pattern already established by `run()` pre-P8-E6.
    ///
    /// `shutdown_rx` is a mutable *reference*, not owned outright — it must
    /// survive across every generation of the outer loop, since a
    /// `oneshot::Receiver` becomes permanently unusable once it actually
    /// resolves (receives a value, or its sender drops), not merely from
    /// being polled without winning a `select!`. Owning it here would mean
    /// generation 2+ has no way to observe a shutdown request at all.
    ///
    /// Records a crash attempt and consults the respawn policy.
    ///
    /// Shared by all three crash sources — `spawner.spawn()` failure,
    /// transport recv error, and the keepalive watchdog's death signal —
    /// so the attempt_history/should_respawn/logging sequence is written
    /// (and can go wrong) in exactly one place, not three.
    fn record_crash_and_decide(&mut self) -> bool {
        self.attempt_history.push(Instant::now());
        let should = self.respawn_policy.should_respawn(&self.attempt_history);
        tracing::info!(worker_id = %self.worker_id, should_respawn = should, "crash_respawn_decision");
        should
    }

    /// Force-kill the currently-tracked child, if any.
    ///
    /// Used for exit paths where there's no expectation of a graceful,
    /// worker-initiated exit: `InitTimedOut` (nothing to gracefully finish
    /// — the worker never even reached `Ready`), `WorkerReportedDying`
    /// (the worker already told us it's terminating on its own — this is
    /// a safety net, not the primary mechanism), and
    /// `Crashed { should_respawn: false }` (the connection is presumably
    /// already broken by the time this fires — transport error or
    /// watchdog timeout — so there's no "ask nicely" step worth
    /// attempting). Also the fallback `graceful_shutdown_child()` calls if
    /// the worker doesn't exit on its own within the bound.
    async fn force_kill_child(&mut self) {
        if let Some(mut child) = self.child.take()
            && let Err(e) = child.kill().await
        {
            tracing::warn!(
                worker_id = %self.worker_id,
                error = %e,
                "failed to kill child on exit (it may have already exited)"
            );
        }
    }

    /// Attempt a graceful shutdown of the currently-tracked child.
    ///
    /// Sends `WorkerMessage::Shutdown` — documented as "the worker should
    /// finish its current step, then exit 0" — then races two things: the
    /// bounded `self.graceful_shutdown_timeout` wait for the worker to
    /// exit on its own (production default 30s, `ANVILML_DESIGN.md
    /// §19.3`), and `force_shutdown_rx` — an explicit "skip the rest of
    /// the graceful wait, force-kill now" signal. There is no IPC-level
    /// acknowledgment event for "the worker received Shutdown and is
    /// exiting" — `WorkerEvent` has no such variant — so process exit
    /// itself (`Child::wait()`), not any received message, is what the
    /// first race arm waits on.
    ///
    /// # Why `force_shutdown_rx` exists
    ///
    /// `WorkerPool::shutdown_all()` (P8-G1) enforces one shared, pool-wide
    /// timeout across every worker. If that pool-level timeout is shorter
    /// than a given worker's own `graceful_shutdown_timeout` — the exact
    /// "straggler" scenario `shutdown_all()` is built to handle — the pool
    /// needs a way to make *this specific* worker skip the remainder of
    /// its own bounded wait and force-kill immediately, without externally
    /// cancelling the `run()` task itself. Externally aborting the task
    /// (`WorkerHandle::abort()`, a genuine last resort — see that method's
    /// own doc comment) runs none of this method's own cleanup logic,
    /// which can leave `self.child` orphaned — the exact class of bug this
    /// session's earlier orphan-cleanup work already found and fixed
    /// elsewhere. Racing an explicit signal here means the *same*,
    /// already-correct `force_kill_child()` call this method already made
    /// for its own internal timeout is what runs, regardless of which of
    /// the two race arms actually fires.
    ///
    /// A `transport.send()` failure is treated the same as the worker not
    /// exiting in time: there's no point waiting for a worker we couldn't
    /// even ask to shut down, so this skips straight to the force-kill
    /// fallback.
    async fn graceful_shutdown_child(&mut self, force_shutdown_rx: &mut oneshot::Receiver<()>) {
        if self.child.is_none() {
            return;
        }

        if let Err(e) = self
            .transport
            .send(&self.worker_id, &WorkerMessage::Shutdown)
            .await
        {
            tracing::warn!(
                worker_id = %self.worker_id,
                error = %e,
                "failed to send Shutdown message — force-killing instead"
            );
            self.force_kill_child().await;
            return;
        }

        // Both race arms only decide an *outcome* here, rather than
        // calling self.force_kill_child() directly inside either arm:
        // the first arm's own future (child.wait()) holds self.child
        // borrowed via `child` for the whole select!, and
        // force_kill_child() needs a fresh &mut self (including
        // self.child) that would conflict with that borrow. Deferring
        // the actual call to after the select! — once `child`'s borrow
        // has ended — avoids the conflict entirely.
        enum Outcome {
            ExitedGracefully,
            NeedsForceKill,
        }

        let outcome = {
            let Some(child) = self.child.as_mut() else {
                return;
            };
            tokio::select! {
                result = tokio::time::timeout(self.graceful_shutdown_timeout, child.wait()) => {
                    match result {
                        Ok(Ok(_)) => Outcome::ExitedGracefully,
                        Ok(Err(e)) => {
                            tracing::warn!(
                                worker_id = %self.worker_id,
                                error = %e,
                                "error waiting for graceful exit — force-killing"
                            );
                            Outcome::NeedsForceKill
                        }
                        Err(_) => {
                            tracing::warn!(
                                worker_id = %self.worker_id,
                                timeout = ?self.graceful_shutdown_timeout,
                                "worker did not exit within the graceful shutdown timeout — force-killing"
                            );
                            Outcome::NeedsForceKill
                        }
                    }
                }
                _ = &mut *force_shutdown_rx => {
                    tracing::warn!(
                        worker_id = %self.worker_id,
                        "force shutdown requested — skipping remainder of graceful wait"
                    );
                    Outcome::NeedsForceKill
                }
            }
        };

        match outcome {
            Outcome::ExitedGracefully => {
                tracing::info!(worker_id = %self.worker_id, "worker_exited_gracefully");
                self.child = None;
            }
            Outcome::NeedsForceKill => {
                self.force_kill_child().await;
            }
        }
    }

    /// Steps, in order:
    /// 1. If a previous generation's child is still tracked (`self.child`),
    ///    kills it (`.kill().await`) before spawning the new one — a no-op
    ///    on gen 0. Without this, the old `Child` would be silently dropped
    ///    the moment step 3 overwrites `self.child`, potentially leaking a
    ///    still-running process (e.g. a hung worker whose IPC connection
    ///    died) on every respawn cycle. A kill failure (most commonly: the
    ///    process had already exited on its own) is logged and otherwise
    ///    ignored — it doesn't block the new spawn attempt.
    /// 2. Calls `self.spawner.spawn(&self.venv_path, self.env.clone())`. On
    ///    failure: appends to `attempt_history`, consults
    ///    `RespawnPolicy::should_respawn()`, and returns
    ///    `RunOutcome::Crashed` immediately — no registration, no
    ///    `Initializing` status, since no worker process exists to track.
    /// 3. On success: stores the `Child` (`self.child`, so it isn't orphaned
    ///    — an unstored `Child` is dropped without killing the OS process it
    ///    wraps), registers with the demux (`Demux::register()` is
    ///    documented idempotent — this is safe even on gen 0, where a caller
    ///    may already have registered), and sets status to `Initializing`.
    /// 4. **Phase 1 — pre-`Ready` loop.** `tokio::select!` between
    ///    `shutdown_rx`, `event_rx.recv()`, and the `init_timeout` guard.
    ///    No watchdog exists yet in this phase — see "Why the watchdog
    ///    waits for `Ready`" below. Any non-`Ready` event is processed via
    ///    `handle_event()` and phase 1 continues; `Ready` is processed the
    ///    same way and then phase 1 ends, moving to phase 2.
    /// 5. **Phase 2 — steady-state loop.** Spawns a fresh `KeepaliveWatchdog`
    ///    task (fresh `dead_tx`/`dead_rx` pair every generation — this is
    ///    *not* a struct field, unlike pre-P8-E6: a single oneshot pair
    ///    consumed via `.take()` once in `new()` would panic on generation
    ///    2's watchdog spawn, since the `Option` would already be `None`
    ///    from generation 1), then `tokio::select!`s between `shutdown_rx`,
    ///    `event_rx.recv()`, and the watchdog's `dead_rx`. No
    ///    `init_timeout` in this phase — `Ready` has already been received,
    ///    so that guard's job is already done.
    ///
    /// # Why the watchdog waits for `Ready`
    ///
    /// `KeepaliveWatchdog::run()`'s first ping fires essentially immediately
    /// once spawned (standard `tokio::time::interval` behavior — the first
    /// tick resolves right away, it does not wait a full `ping_interval`
    /// first). If the watchdog were spawned unconditionally at the top of
    /// this method (as it was pre-this-fix), that first ping can race the
    /// worker subprocess's own ZeroMQ DEALER-side connection handshake:
    /// `Demux::register()` succeeding (an in-memory Rust map insert) says
    /// nothing about whether the ROUTER *socket* itself has recorded the
    /// peer identity yet — that only happens once the DEALER's connection
    /// handshake genuinely completes at the ZeroMQ protocol level, which is
    /// a separate, slower thing. Losing that race makes `RouterTransport`'s
    /// `send()` fail with "Destination client not found by identity", which
    /// the watchdog cannot distinguish from a genuinely dead worker — a
    /// false-positive crash on a worker that hasn't even finished
    /// connecting. Waiting for `Ready` (the worker's own proof its
    /// connection is fully established) before spawning the watchdog
    /// eliminates this race structurally rather than narrowing it with an
    /// arbitrary grace period. `init_timeout` already exists specifically
    /// to catch "never started" — the watchdog's job becomes exactly
    /// "started, then died," with no overlap between the two.
    ///
    /// `shutdown_rx` is a mutable *reference*, not owned outright — it must
    /// survive across every generation of the outer loop, since a
    /// `oneshot::Receiver` becomes permanently unusable once it actually
    /// resolves (receives a value, or its sender drops), not merely from
    /// being polled without winning a `select!`. Owning it here would mean
    /// generation 2+ has no way to observe a shutdown request at all.
    ///
    /// # Exit paths (this generation only — see `run()` for what happens next)
    ///
    /// 1. **Graceful shutdown** — `shutdown_rx` receives `()`, in either
    ///    phase: status → `Dying`, returns `RunOutcome::ShutdownRequested`.
    /// 2. **Initializing timeout** — `init_timeout` elapses without `Ready`
    ///    (phase 1 only — this branch does not exist in phase 2): status →
    ///    `Dead`, returns `RunOutcome::InitTimedOut`.
    /// 3. **Worker crash (explicit)** — `Dying` event received, in either
    ///    phase: status → `Dead`, returns `RunOutcome::WorkerReportedDying`.
    /// 4. **Worker crash (event channel closed)** — `event_rx.recv()`
    ///    returns `None`, in either phase: status → `Dead`,
    ///    `attempt_history` appended, `should_respawn()` consulted,
    ///    returns `RunOutcome::Crashed`. As of P8-F2, this generation
    ///    consumes events from its own demux-routed channel rather than
    ///    calling `self.transport.recv()` directly — see this method's
    ///    `event_rx` binding, above, for why: `transport.recv()` races
    ///    once multiple `ManagedWorker` instances share one
    ///    `Arc<RouterTransport>` (`WorkerPool`, P8-G1). This path should
    ///    be rare in practice (see `event_rx`'s own binding comment for
    ///    when the channel can actually close), unlike the pre-P8-F2
    ///    transport error this replaces, which fired on every genuine
    ///    connection failure or malformed message.
    /// 5. **Keepalive watchdog timeout** — the watchdog's `dead_rx` becomes
    ///    ready (phase 2 only — the watchdog does not exist in phase 1):
    ///    status → `Dead`, `attempt_history` appended, `should_respawn()`
    ///    consulted, returns `RunOutcome::Crashed` (same handling as path 4).
    ///
    /// This method does **not** call `demux.deregister()` — that is `run()`'s
    /// outer loop's responsibility, called once per generation regardless of
    /// outcome, after this method returns.
    pub(crate) async fn run_once(
        mut self,
        shutdown_rx: &mut oneshot::Receiver<()>,
    ) -> (Self, RunOutcome) {
        // If a previous generation's child is still tracked, kill it before
        // spawning the new one. A no-op on gen 0 (self.child is None until
        // the first spawn). Without this, the old Child value would be
        // silently dropped the moment `self.child = Some(new_child)`
        // overwrites it below — and tokio::process::Child does not kill the
        // underlying OS process on drop unless `kill_on_drop(true)` was set
        // when it was built, which ProcessWorkerSpawner's current Command
        // construction does not do. Most crashes mean the old process has
        // already exited on its own — `.kill()` on an already-exited
        // process returns an error, which is expected and harmless here —
        // but a hung/deadlocked worker whose IPC connection died while the
        // process itself is still running would otherwise leak one more
        // zombie process on every single respawn cycle.
        if let Some(mut old_child) = self.child.take()
            && let Err(e) = old_child.kill().await
        {
            tracing::warn!(
                worker_id = %self.worker_id,
                error = %e,
                "failed to kill previous generation's child (it may have already exited)"
            );
        }

        // Step 1/2: spawn this generation's child process.
        let child = match self.spawner.spawn(&self.venv_path, self.env.clone()).await {
            Ok(child) => child,
            Err(e) => {
                tracing::error!(worker_id = %self.worker_id, error = %e, "spawn_failed");
                let should = self.record_crash_and_decide();
                return (
                    self,
                    RunOutcome::Crashed {
                        should_respawn: should,
                    },
                );
            }
        };
        // Store the Child so it isn't orphaned — an unstored Child is
        // dropped (and its OS process left running, unmanaged) the moment
        // this match arm ends.
        self.child = Some(child);

        // Take stdout/stderr out of the Child and spawn one background
        // reader task per stream, logging each line as it arrives.
        //
        // spawn.rs pipes both streams (Stdio::piped()) specifically so the
        // supervisor can read them, but nothing previously did — meaning a
        // worker's own crash traceback (import errors, torch/ROCm
        // failures, anything printed before or after Ready) was captured
        // into an OS pipe buffer and then silently discarded when the
        // Child was dropped. This was invisible for every failure mode
        // that doesn't also close the IPC channel or report via WorkerEvent
        // — which, per this generation's own doc comment on `event_rx`'s
        // binding, is not guaranteed for a worker that dies (or simply
        // never progresses) without ever completing its IPC handshake.
        //
        // .take() leaves None in the Child's own stdout/stderr fields —
        // harmless, since nothing else reads them afterward. Each task
        // ends naturally at EOF (the pipe closes when the child process
        // exits or closes the stream itself); no explicit cancellation is
        // needed since these are fire-and-forget background tasks scoped
        // to this generation's child lifetime.
        if let Some(stdout) = self
            .child
            .as_mut()
            .expect("self.child was just set to Some(child) immediately above")
            .stdout
            .take()
        {
            let worker_id = self.worker_id.clone();
            tokio::spawn(async move {
                let mut lines = tokio::io::BufReader::new(stdout).lines();
                loop {
                    match lines.next_line().await {
                        Ok(Some(line)) => {
                            // Use the worker's own logging.* level when the
                            // line is one, and log just the remainder after
                            // LEVELNAME as the message — otherwise the
                            // rendered line would show the date/time/level
                            // twice: once from this Rust event's own
                            // timestamp/level, once embedded verbatim in
                            // the forwarded text. Lines that don't parse
                            // (raw print(), etc.) fall back to logging the
                            // full original line, unmangled, at the
                            // previous unconditional debug level.
                            //
                            // This can't be `tracing::event!(level, ...)`
                            // with `level` as a runtime variable: tracing's
                            // callsite mechanism bakes the level into a
                            // `static` Metadata for each callsite (that's
                            // what makes its enabled-checks cheap), so the
                            // level must be a compile-time constant —
                            // dispatching to the matching literal-level
                            // macro per match arm is the standard pattern
                            // for a level that's only known at runtime.
                            match parse_python_log_line(&line) {
                                Some((tracing::Level::DEBUG, rest)) => {
                                    tracing::debug!(worker_id = %worker_id, stream = "stdout", "{rest}");
                                }
                                Some((tracing::Level::INFO, rest)) => {
                                    tracing::info!(worker_id = %worker_id, stream = "stdout", "{rest}");
                                }
                                Some((tracing::Level::WARN, rest)) => {
                                    tracing::warn!(worker_id = %worker_id, stream = "stdout", "{rest}");
                                }
                                Some((tracing::Level::ERROR, rest)) => {
                                    tracing::error!(worker_id = %worker_id, stream = "stdout", "{rest}");
                                }
                                // `None` (unparseable — logged in full,
                                // unmangled) and the unreachable
                                // `Some((Level::TRACE, _))` (parse_python_log_line
                                // never produces it — needed only so this
                                // match stays exhaustive over all 5 Level
                                // variants) share the previous default.
                                _ => {
                                    tracing::debug!(worker_id = %worker_id, stream = "stdout", "{line}");
                                }
                            }
                        }
                        Ok(None) => break, // EOF — child closed stdout or exited.
                        Err(e) => {
                            tracing::warn!(worker_id = %worker_id, stream = "stdout", error = %e, "worker_output_read_failed");
                            break;
                        }
                    }
                }
            });
        }
        if let Some(stderr) = self
            .child
            .as_mut()
            .expect("self.child was just set to Some(child) immediately above")
            .stderr
            .take()
        {
            let worker_id = self.worker_id.clone();
            tokio::spawn(async move {
                let mut lines = tokio::io::BufReader::new(stderr).lines();
                loop {
                    match lines.next_line().await {
                        Ok(Some(line)) => {
                            // Use the worker's own logging.* level when the
                            // line is one, so a routine INFO line no longer
                            // renders as WARN, and log just the remainder
                            // after LEVELNAME as the message — otherwise
                            // the date/time/level would show twice (once
                            // from this Rust event's own timestamp/level,
                            // once embedded verbatim in the forwarded
                            // text). Lines that don't parse — most notably
                            // an uncaught Python traceback, printed
                            // straight to stderr by the interpreter rather
                            // than going through logging.* — log the full,
                            // unmangled original line and keep the WARN
                            // fallback below unchanged: stderr is where
                            // that lands, a healthy worker's stderr should
                            // otherwise be silent, so this level makes real
                            // unstructured output impossible to miss even
                            // without debug logging enabled.
                            //
                            // See the stdout arm above for why this is a
                            // match dispatching to literal-level macros
                            // rather than `tracing::event!(level, ...)`.
                            match parse_python_log_line(&line) {
                                Some((tracing::Level::DEBUG, rest)) => {
                                    tracing::debug!(worker_id = %worker_id, stream = "stderr", "{rest}");
                                }
                                Some((tracing::Level::INFO, rest)) => {
                                    tracing::info!(worker_id = %worker_id, stream = "stderr", "{rest}");
                                }
                                Some((tracing::Level::WARN, rest)) => {
                                    tracing::warn!(worker_id = %worker_id, stream = "stderr", "{rest}");
                                }
                                Some((tracing::Level::ERROR, rest)) => {
                                    tracing::error!(worker_id = %worker_id, stream = "stderr", "{rest}");
                                }
                                _ => {
                                    tracing::warn!(worker_id = %worker_id, stream = "stderr", "{line}");
                                }
                            }
                        }
                        Ok(None) => break,
                        Err(e) => {
                            tracing::warn!(worker_id = %worker_id, stream = "stderr", error = %e, "worker_output_read_failed");
                            break;
                        }
                    }
                }
            });
        }

        // Reassign the Job Object's kill-on-close protection to this
        // generation's child (P8-E7). Every generation, gen 0 and every
        // respawn alike — see job_guard's own field doc comment for why
        // reassigning onto a fresh child is always the clean case, not
        // the cross-guard rejection scenario. assign_process() failing is
        // logged and non-fatal to this spawn attempt, matching job_guard
        // being Option (see new()'s own doc comment) — the worker still
        // runs, just without this specific orphan-cleanup protection for
        // this generation.
        #[cfg(windows)]
        if let Some(guard) = &self.job_guard
            && let Err(e) = guard.assign_process(
                self.child
                    .as_ref()
                    .expect("self.child was just set to Some(child) immediately above"),
            )
        {
            tracing::warn!(
                worker_id = %self.worker_id,
                error = %e,
                "JobObjectGuard::assign_process() failed — this generation's \
                 child will not be protected against orphaning if the \
                 supervisor dies unexpectedly"
            );
        }

        // Register with the demux now that a worker process genuinely
        // exists. Idempotent per Demux::register()'s own contract, so this
        // is safe even on gen 0 if an external caller already registered.
        //
        // event_rx is kept (not dropped, unlike pre-P8-F2) and consumed
        // directly by this generation's select! loops below, instead of
        // calling self.transport.recv() itself. This is P8-F2's actual
        // fix: self.transport.recv() races once multiple ManagedWorker
        // instances share one Arc<RouterTransport> (WorkerPool, P8-G1) —
        // whichever task's recv() call happens to win a given poll
        // consumes whatever message arrives, regardless of which worker
        // it was actually addressed to. Consuming from this worker's own
        // paired channel instead means events can only ever reach the
        // worker they were routed to, by construction — bridge.rs's
        // reader task (P8-F1) is now the sole caller of
        // transport.recv(), and it routes via Demux::route() using the
        // identity carried in each message, before this channel is ever
        // touched.
        let (event_tx, mut event_rx) = tokio::sync::mpsc::channel::<WorkerEvent>(16);
        self.demux.register(self.worker_id.clone(), event_tx);
        debug_assert!(
            self.demux.registered(&self.worker_id),
            "worker must be registered immediately after a successful spawn"
        );
        tracing::info!(
            worker_id = %self.worker_id,
            child_pid = ?self.child.as_ref().and_then(|c| c.id()),
            "worker_spawned_and_registered"
        );

        // The worker must transition through Initializing before it can
        // reach Idle.
        *self.status.write().await = WorkerStatus::Initializing;

        // Spawn the Initializing timeout guard — phase 1 only, see below.
        // If this sleep completes before a Ready event, the worker is
        // declared Dead.
        let init_timeout = tokio::time::sleep(self.init_timeout);
        tokio::pin!(init_timeout);

        // Phase 1: pre-Ready loop. No watchdog exists yet — see this
        // method's "Why the watchdog waits for Ready" doc section for why
        // that's deliberate, not an oversight.
        let early_exit = loop {
            tokio::select! {
                _ = &mut *shutdown_rx => {
                    *self.status.write().await = WorkerStatus::Dying;
                    tracing::info!(worker_id = %self.worker_id, "shutdown_requested");
                    break Some(RunOutcome::ShutdownRequested);
                }

                maybe_event = event_rx.recv() => {
                    match maybe_event {
                        Some(event) => {
                            let is_ready = matches!(event, WorkerEvent::Ready { .. });
                            if self.handle_event(event).await {
                                break Some(RunOutcome::WorkerReportedDying);
                            }
                            if is_ready {
                                // Ready received — end phase 1 cleanly, no
                                // RunOutcome yet, proceed to phase 2 below.
                                break None;
                            }
                        }
                        None => {
                            // The channel closed — nothing will ever send
                            // this generation another event again (see
                            // event_rx's own binding comment above for
                            // when this can happen; it should be rare
                            // under normal operation). Functionally
                            // equivalent to the old transport.recv()
                            // error case: this generation can no longer
                            // receive anything, so it's a crash.
                            tracing::error!(worker_id = %self.worker_id, "event channel closed");
                            *self.status.write().await = WorkerStatus::Dead;
                            let should = self.record_crash_and_decide();
                            break Some(RunOutcome::Crashed { should_respawn: should });
                        }
                    }
                }

                _ = &mut init_timeout => {
                    *self.status.write().await = WorkerStatus::Dead;
                    tracing::info!(worker_id = %self.worker_id, "worker_declared_dead");
                    break Some(RunOutcome::InitTimedOut);
                }
            }
        };

        if let Some(outcome) = early_exit {
            return (self, outcome);
        }

        // Phase 2: Ready has been received — spawn the watchdog now, past
        // the dealer-handshake race window (the worker has already proven
        // its connection is fully established by sending Ready), and enter
        // the steady-state loop.
        //
        // dead_tx/dead_rx are fresh locals for THIS generation only — see
        // this method's own doc comment for why they can't be struct
        // fields anymore.
        //
        // Production defaults: 30s ping interval, 10s pong timeout
        // (ANVILML_DESIGN.md §9.2).
        let (watchdog_dead_tx, mut watchdog_dead_rx) = tokio::sync::oneshot::channel();
        let (watchdog_pong_tx, watchdog_pong_rx) = tokio::sync::mpsc::channel(16);
        // Replace self.pong_tx with the sender for the watchdog's channel.
        // This ensures handle_event() forwards Pongs into the same channel
        // the watchdog is reading from.
        self.pong_tx = watchdog_pong_tx;
        let watchdog = KeepaliveWatchdog::new(
            self.worker_id.clone(),
            RouterTransportAdapter(Arc::clone(&self.transport)),
            watchdog_pong_rx,
            watchdog_dead_tx,
            self.watchdog_ping_interval,
            self.watchdog_pong_timeout,
        );
        tokio::spawn(watchdog.run());

        let outcome = loop {
            tokio::select! {
                _ = &mut *shutdown_rx => {
                    *self.status.write().await = WorkerStatus::Dying;
                    tracing::info!(worker_id = %self.worker_id, "shutdown_requested");
                    break RunOutcome::ShutdownRequested;
                }

                maybe_event = event_rx.recv() => {
                    match maybe_event {
                        Some(event) => {
                            // handle_event returns true when the Dying event is
                            // received, signaling this generation to end.
                            if self.handle_event(event).await {
                                break RunOutcome::WorkerReportedDying;
                            }
                        }
                        None => {
                            // The channel closed — see phase 1's identical
                            // branch above for the full explanation.
                            // Functionally equivalent to the old
                            // transport.recv() error case: a crash.
                            tracing::error!(worker_id = %self.worker_id, "event channel closed");
                            *self.status.write().await = WorkerStatus::Dead;
                            let should = self.record_crash_and_decide();
                            break RunOutcome::Crashed { should_respawn: should };
                        }
                    }
                }

                // Watchdog dead path — the keepalive watchdog detected a missing Pong.
                // This is a second crash source, independent of event_rx.
                // Handled identically to the event-channel-closed branch.
                _ = &mut watchdog_dead_rx => {
                    tracing::error!(worker_id = %self.worker_id, "watchdog timeout — worker declared dead");
                    *self.status.write().await = WorkerStatus::Dead;
                    let should = self.record_crash_and_decide();
                    break RunOutcome::Crashed { should_respawn: should };
                }
            }
        };

        (self, outcome)
    }

    /// Test-only public wrapper around `run_once()`.
    ///
    /// `run_once()` itself is `pub(crate)`, matching this task's own specified
    /// visibility — but `tests/managed_tests.rs` is an integration test crate,
    /// which cannot see `pub(crate)` items at all: Cargo compiles integration
    /// tests against this library as a normal external dependency, the same
    /// way any downstream consumer would. This wrapper exists solely to make
    /// single-generation state (e.g. `self.child`, one generation's exact
    /// `RunOutcome` in isolation, rather than inferred from `run()`'s
    /// multi-generation black-box behavior) inspectable from integration
    /// tests, without changing `run_once()`'s own production visibility.
    ///
    /// Gated behind the `test-utils` feature (`Cargo.toml`), not
    /// `#[cfg(test)]`. `#[cfg(test)]` only activates when this crate's own
    /// unit tests are compiled (`cargo test --lib`) — it does **not**
    /// activate when an integration test in `tests/` links against this
    /// library, because that link happens against a normal (non-`--cfg
    /// test`) build of the library. A `#[cfg(test)]`-gated item here would
    /// compile without error but simply not exist in the artifact
    /// `tests/managed_tests.rs` links against — a real Rust visibility
    /// gotcha, not a style choice. `test-utils` is enabled automatically for
    /// every `cargo test` build (unit and integration alike) via the
    /// self-referential `[dev-dependencies]` entry in `Cargo.toml`; no
    /// command-line changes are needed.
    #[cfg(feature = "test-utils")]
    pub async fn run_once_for_test(
        self,
        shutdown_rx: &mut oneshot::Receiver<()>,
    ) -> (Self, RunOutcome) {
        self.run_once(shutdown_rx).await
    }

    /// Test-only accessor for the currently-tracked child's OS process ID.
    ///
    /// `self.child` is a private field for the same reason `run_once()` is
    /// `pub(crate)` — see `run_once_for_test()`'s doc comment for the full
    /// explanation of why a `test-utils`-gated wrapper is needed at all
    /// rather than `#[cfg(test)]`. Returns `None` if no child has been
    /// spawned yet (only possible before the very first `run_once()` call
    /// completes its spawn step).
    #[cfg(feature = "test-utils")]
    pub fn child_pid_for_test(&self) -> Option<u32> {
        self.child.as_ref().and_then(|c| c.id())
    }

    /// Test-only accessor that takes ownership of the currently-tracked
    /// child, leaving `None` behind.
    ///
    /// Needed for P8-E7's `JobObjectGuard` tests specifically: verifying a
    /// job object's kill-on-close behavior means dropping the guard, then
    /// `.wait()`-ing on the *same* `Child` handle to see if the OS actually
    /// terminated it — matching the pattern already established in
    /// `spawn_tests.rs`'s `test_assigned_child_terminated_on_drop`. That
    /// requires the caller to hold the `Child` handle directly; `.wait()`
    /// isn't reachable through `child_pid_for_test()`'s `u32`.
    ///
    /// Taking the handle out of `self.child` does not affect the job
    /// object's OS-level association with that process — `AssignProcessToJobObject`
    /// operates on the OS process itself, independent of which Rust-level
    /// value currently owns the `Child` handle for it.
    #[cfg(feature = "test-utils")]
    pub fn take_child_for_test(&mut self) -> Option<tokio::process::Child> {
        self.child.take()
    }

    /// Run the worker's full lifecycle across every respawn generation.
    ///
    /// This method consumes `self` and owns the worker's entire lifetime. It
    /// is a thin outer loop around `run_once()` (see that method's own doc
    /// comment for the full per-generation contract):
    ///
    /// 1. Call `run_once()` for this generation.
    /// 2. Deregister — unconditional, on every generation's exit, regardless
    ///    of outcome.
    /// 3. If the outcome is `RunOutcome::Crashed { should_respawn: true }`:
    ///    set status to `Respawning`, sleep for
    ///    `RespawnPolicy::next_delay()`, and loop back to step 1 — the next
    ///    `run_once()` call spawns the next generation's child as its own
    ///    first step.
    /// 4. `RunOutcome::ShutdownRequested`: `graceful_shutdown_child()` —
    ///    send `WorkerMessage::Shutdown`, wait up to
    ///    `graceful_shutdown_timeout` for the worker to exit on its own,
    ///    force-kill as a fallback — then return.
    /// 5. Any other outcome (`InitTimedOut`, `WorkerReportedDying`, or
    ///    `Crashed { should_respawn: false }`): `force_kill_child()` — no
    ///    expectation of a graceful exit for these, see that method's own
    ///    doc comment for why — then return. The worker has exited for
    ///    good.
    ///
    /// Before this method's cleanup was added, `self.child` was only ever
    /// killed as part of the *next* generation's respawn — meaning every
    /// exit path in this list left the current generation's child process
    /// simply dropped, unkilled (`tokio::process::Child` does not kill on
    /// drop unless `kill_on_drop(true)` was set at spawn time, which it
    /// isn't). On Windows this was largely masked by `JobObjectGuard`'s own
    /// drop-triggered kill-on-close; on Linux/macOS, where no equivalent
    /// exists, every worker exit — not just crashes — orphaned its child
    /// process.
    ///
    /// # Arguments
    ///
    /// * `shutdown_rx` — A oneshot receiver; when `()` is sent, the current
    ///   generation transitions to `Dying` and the worker exits for good —
    ///   no respawn, regardless of `RespawnPolicy`. Held here, at the outer
    ///   loop's level, and passed to each `run_once()` call by mutable
    ///   reference so it survives across every generation — see
    ///   `run_once()`'s own doc comment for why owning it per-generation
    ///   would be incorrect.
    /// * `force_shutdown_rx` — A second, independent oneshot receiver.
    ///   Only consulted once `graceful_shutdown_child()` is already
    ///   running (i.e. after `ShutdownRequested` has already fired) —
    ///   sending on this *without* a prior `shutdown_rx` signal does
    ///   nothing, since nothing is listening for it before that point.
    ///   See `graceful_shutdown_child()`'s own doc comment for why this
    ///   exists: letting `WorkerPool::shutdown_all()` make a specific
    ///   straggling worker skip the remainder of its own bounded graceful
    ///   wait and force-kill immediately, without externally aborting the
    ///   task (which would skip this method's own cleanup entirely).
    #[tracing::instrument(skip(self, shutdown_rx, force_shutdown_rx), fields(worker_id = %self.worker_id))]
    pub async fn run(
        mut self,
        mut shutdown_rx: oneshot::Receiver<()>,
        mut force_shutdown_rx: oneshot::Receiver<()>,
    ) {
        loop {
            let (worker, outcome) = self.run_once(&mut shutdown_rx).await;
            self = worker;

            // Final action for this generation, on every exit path: deregister.
            self.demux.deregister(&self.worker_id);
            tracing::info!(worker_id = %self.worker_id, "worker_deregistered");

            match outcome {
                RunOutcome::Crashed {
                    should_respawn: true,
                } => {
                    *self.status.write().await = WorkerStatus::Respawning;
                    let delay = self.respawn_policy.next_delay();
                    tracing::info!(
                        worker_id = %self.worker_id,
                        delay_ms = %delay.as_millis(),
                        "respawn_scheduled"
                    );
                    tokio::time::sleep(delay).await;
                    // Loop back — run_once() spawns and registers the next
                    // generation as its own first steps.
                }
                RunOutcome::ShutdownRequested => {
                    self.graceful_shutdown_child(&mut force_shutdown_rx).await;
                    break;
                }
                RunOutcome::InitTimedOut
                | RunOutcome::WorkerReportedDying
                | RunOutcome::Crashed {
                    should_respawn: false,
                } => {
                    self.force_kill_child().await;
                    break;
                }
            }
        }
    }

    /// Handle a single `WorkerEvent` from the worker.
    ///
    /// Returns `true` if the main loop should break (on `Dying` event),
    /// `false` otherwise. Transitions the worker's status based on the event type:
    /// - `Ready`: status → `Idle`, log `worker_ready`.
    /// - `Dying`: status → `Dead`, log `worker_dying`, return `true` to break the loop.
    /// - `Completed`/`Failed`/`Cancelled`: status → `Idle`, log completion/failure/cancellation.
    /// - `Pong`: forward to the keepalive watchdog's pong channel via `try_send`
    ///   (best-effort; a failed send is not an error — the watchdog will timeout
    ///   if it misses a Pong).
    /// - Other events: log at DEBUG level.
    ///
    /// The `init_timeout` branch only exists in `run_once()`'s phase-1 loop
    /// (pre-`Ready`) — it is structurally absent from phase 2, not disarmed
    /// by a runtime flag, once a `Ready` event ends phase 1. See
    /// `run_once()`'s doc comment for the full two-phase mechanism.
    async fn handle_event(&mut self, event: WorkerEvent) -> bool {
        // Clone the event for forwarding to the watchdog before the match.
        // The match borrows `event` via `&event`, so we can't move it
        // into `try_send()` without first cloning.
        let pong_forward = event.clone();
        match &event {
            WorkerEvent::Ready { node_types, .. } => {
                // Populate the dynamic node registry from the worker's self-reported
                // node types. register_all() replaces (not merges) prior contents,
                // which is correct on respawn when the worker re-reports its full set.
                // Consume the node_types Vec — register_all takes ownership
                // because it needs to build a new HashMap from scratch.
                self.node_registry.register_all(node_types.clone());
                // Worker successfully initialized. run_once()'s phase-1 loop
                // ends the moment this event is matched in its own recv()
                // branch — not here, since handle_event() doesn't own that
                // control flow, it's only called from within it.
                *self.status.write().await = WorkerStatus::Idle;
                tracing::info!(worker_id = %self.worker_id, "worker_ready");
                false
            }
            WorkerEvent::Dying { reason } => {
                // Worker is terminating — the worker itself reported this, so it
                // is already gone: transition straight to Dead (not the Dying
                // status, which is reserved for a supervisor-initiated shutdown
                // still in flight — see the shutdown_rx branch in run_once()).
                *self.status.write().await = WorkerStatus::Dead;
                tracing::info!(worker_id = %self.worker_id, reason = %reason, "worker_dying");
                true
            }
            WorkerEvent::Completed { job_id, elapsed_ms } => {
                // Job completed successfully — transition back to Idle so the
                // worker can accept the next job.
                *self.status.write().await = WorkerStatus::Idle;
                tracing::info!(worker_id = %self.worker_id, job_id = %job_id, elapsed_ms = %elapsed_ms, "job_completed");
                false
            }
            WorkerEvent::Failed {
                job_id,
                error,
                traceback,
            } => {
                // Job failed — transition back to Idle. The traceback is logged
                // at DEBUG level for diagnostic purposes.
                *self.status.write().await = WorkerStatus::Idle;
                tracing::info!(worker_id = %self.worker_id, job_id = %job_id, error = %error, "job_failed");
                if let Some(tb) = traceback {
                    tracing::debug!(worker_id = %self.worker_id, traceback = %tb, "job failure traceback");
                }
                false
            }
            WorkerEvent::Cancelled { job_id } => {
                // Job was cancelled by the client — transition back to Idle.
                *self.status.write().await = WorkerStatus::Idle;
                tracing::info!(worker_id = %self.worker_id, job_id = %job_id, "job_cancelled");
                false
            }
            WorkerEvent::Pong { seq } => {
                // Forward the Pong to the keepalive watchdog's pong channel.
                // The watchdog filters for matching sequence numbers internally.
                // A failed send (closed/full channel) is best-effort — the watchdog
                // will timeout and declare the worker dead if it misses a Pong,
                // which is the correct failure mode.
                let _ = self.pong_tx.try_send(pong_forward);
                tracing::debug!(worker_id = %self.worker_id, seq = %seq, "pong_received");
                false
            }
            WorkerEvent::Progress { .. }
            | WorkerEvent::ImageReady { .. }
            | WorkerEvent::MemoryReport { .. } => {
                // These events are informational — log at DEBUG level.
                // They do not affect the worker's lifecycle state.
                tracing::debug!(worker_id = %self.worker_id, event = ?event, "unhandled_event");
                false
            }
        }
    }
}
