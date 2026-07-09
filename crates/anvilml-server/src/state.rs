use std::sync::Arc;

use anvilml_artifacts::ArtifactStore;
use anvilml_core::{NodeTypeRegistry, ServerConfig};
use anvilml_ipc::EventBroadcaster;
use anvilml_scheduler::JobScheduler;
use anvilml_worker::WorkerPool;
use sqlx::SqlitePool;

/// Shared application state for the AnvilML HTTP server.
///
/// `AppState` holds all mutable and read-only data that server handlers
/// need access to. Every field is wrapped in `Arc` so that it can be
/// cloned cheaply into multiple handler clones without sharing `&mut`
/// references.
///
/// This struct grows incrementally across tasks — each task adds one
/// or more fields and the corresponding integration tests. Only the
/// fields present at compile time are available; future fields are
/// added by later phase tasks.
#[derive(Clone)]
pub struct AppState {
    /// Server configuration loaded from `anvilml.toml` and env vars.
    pub config: Arc<ServerConfig>,

    /// Dynamic registry of Python-worker node types, populated at
    /// worker Ready time.
    pub node_registry: Arc<NodeTypeRegistry>,

    /// Monotonic clock instant captured at process startup.
    /// Used by the `/health` handler to compute elapsed uptime.
    pub start_time: std::time::Instant,

    /// Central async dispatcher for generation jobs.
    ///
    /// Owns the in-memory job queue, VRAM ledger, and database-backed
    /// job persistence. The scheduler validates computation graphs,
    /// selects idle workers, and dispatches `Execute` messages to
    /// Python worker subprocesses via the shared `WorkerPool`.
    pub scheduler: Arc<JobScheduler>,

    /// Pool of Python worker subprocesses, one per GPU device.
    ///
    /// Manages worker lifecycle (spawn, supervise, respawn, shutdown)
    /// and provides the shared `RouterTransport` for IPC. The scheduler
    /// queries the pool for idle-worker discovery and VRAM metadata
    /// during job dispatch.
    pub workers: Arc<WorkerPool>,

    /// SQLite connection pool for job persistence.
    ///
    /// Shared with `JobStore` (via `JobScheduler`) for CRUD operations
    /// on the `jobs` table. Uses an in-memory database for tests and a
    /// file-backed database in production.
    pub db: SqlitePool,

    /// Content-addressed PNG artifact storage, shared by HTTP handlers
    /// and the event loop.
    ///
    /// Stores generated PNG artifacts by SHA-256 content hash in a
    /// configurable directory, with metadata persisted in the same
    /// SQLite database used by `JobStore`.
    pub artifact_store: Arc<ArtifactStore>,

    /// Central event broadcaster for WebSocket subscribers.
    ///
    /// The same `Arc<EventBroadcaster>` instance is shared with the
    /// scheduler's event loop (`spawn_event_loop`), so HTTP-layer
    /// subscribers receive all events the scheduler publishes.
    pub broadcaster: Arc<EventBroadcaster>,
}
