//! Integration tests for `ModelStore` — CRUD operations on the `models` table.
//!
//! Each test creates its own in-memory SQLite pool with migrations applied,
//! so there is no cross-test shared state and no `#[serial]` annotation is needed.

use anvilml_core::{ModelDtype, ModelFormat, ModelKind, ModelMeta};
use anvilml_registry::ModelStore;
use chrono::Utc;
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::path::PathBuf;

/// Create an in-memory SQLite pool with migrations applied.
///
/// Each test gets its own pool — the in-memory database is isolated per connection
/// by using a unique cache name (uuid-based) so parallel tests don't collide on
/// the shared `:memory:` database.
///
/// The migration from `database/migrations/001_initial.sql` is applied so the
/// `models` table exists before any CRUD operation.
async fn make_pool() -> SqlitePool {
    // Use a unique in-memory database name per test to avoid the shared `:memory:`
    // database problem: without a unique name, all connections in the same process
    // share the same in-memory database, causing cross-test interference.
    let unique_name = uuid::Uuid::new_v4().to_string();

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(format!("file:{unique_name}?mode=memory&cache=shared"))
                .create_if_missing(true),
        )
        .await
        .expect("should be able to create in-memory SQLite pool");

    // Apply the migration so the `models` table exists.
    // sqlx::migrate!() embeds the migration at compile time; .run() applies
    // any pending migrations (idempotent — running against an already-migrated
    // database is a no-op).
    let migrator = sqlx::migrate!("../../database/migrations");
    migrator.run(&pool).await.expect("migration should succeed");

    pool
}

/// Construct a `ModelMeta` with test values.
///
/// The path is a synthetic temporary path — it does not need to exist on disk
/// because the store only persists metadata, not the file itself.
fn test_meta(id: &str, name: &str, kind: ModelKind) -> ModelMeta {
    ModelMeta {
        id: id.to_string(),
        name: name.to_string(),
        path: PathBuf::from(format!("/tmp/models/{}.safetensors", name)),
        kind,
        dtype: ModelDtype::Fp32,
        format: ModelFormat::Safetensors,
        size_bytes: 1024,
        mtime_unix: 0,
        scanned_at: Utc::now(),
    }
}

/// `upsert` followed by `get` returns the same `ModelMeta` values.
///
/// Inserts a model, then retrieves it by ID and asserts that every field
/// (id, name, path, kind, dtype, format, size_bytes) matches the original.
#[tokio::test]
async fn test_upsert_get_roundtrip() {
    let pool = make_pool().await;
    let store = ModelStore::new(pool);

    let meta = test_meta("test-1", "test-model", ModelKind::Diffusion);
    store.upsert(&meta).await.expect("upsert should succeed");

    let fetched = store
        .get("test-1")
        .await
        .expect("get should succeed")
        .expect("row should exist");

    assert_eq!(fetched.id, meta.id);
    assert_eq!(fetched.name, meta.name);
    assert_eq!(fetched.path, meta.path);
    assert_eq!(fetched.kind, meta.kind);
    assert_eq!(fetched.dtype, meta.dtype);
    assert_eq!(fetched.format, meta.format);
    assert_eq!(fetched.size_bytes, meta.size_bytes);
    // scanned_at may differ by a few milliseconds due to time passage.
    let elapsed = fetched
        .scanned_at
        .signed_duration_since(meta.scanned_at)
        .num_milliseconds()
        .abs();
    assert!(
        elapsed < 2000,
        "scanned_at should be within 2s of original, diff: {elapsed}ms"
    );
}

/// `list(None)` without a kind filter returns all inserted rows.
///
/// Inserts three models with different kinds (Diffusion, TextEncoder, Vae),
/// then calls `list(None)` and asserts the result contains exactly 3 rows.
#[tokio::test]
async fn test_list_no_filter() {
    let pool = make_pool().await;
    let store = ModelStore::new(pool);

    store
        .upsert(&test_meta("1", "diffusion", ModelKind::Diffusion))
        .await
        .unwrap();
    store
        .upsert(&test_meta("2", "text-encoder", ModelKind::TextEncoder))
        .await
        .unwrap();
    store
        .upsert(&test_meta("3", "vae", ModelKind::Vae))
        .await
        .unwrap();

    let all = store.list(None).await.expect("list should succeed");
    assert_eq!(all.len(), 3, "expected 3 models, got {}", all.len());
}

/// `list(Some(kind))` filters to only matching rows.
///
/// Inserts three models with different kinds, then calls `list(Some(Diffusion))`
/// and asserts the result contains exactly 1 row (the diffusion model).
#[tokio::test]
async fn test_list_with_kind_filter() {
    let pool = make_pool().await;
    let store = ModelStore::new(pool);

    store
        .upsert(&test_meta("1", "diffusion", ModelKind::Diffusion))
        .await
        .unwrap();
    store
        .upsert(&test_meta("2", "text-encoder", ModelKind::TextEncoder))
        .await
        .unwrap();
    store
        .upsert(&test_meta("3", "vae", ModelKind::Vae))
        .await
        .unwrap();

    let diffusion = store
        .list(Some(ModelKind::Diffusion))
        .await
        .expect("list with filter should succeed");
    assert_eq!(
        diffusion.len(),
        1,
        "expected 1 diffusion model, got {}",
        diffusion.len()
    );
    assert_eq!(diffusion[0].kind, ModelKind::Diffusion);
}

/// `delete` removes the row; a subsequent `get` returns `None`.
///
/// Inserts a model, deletes it by ID, then retrieves it and asserts that
/// the row no longer exists.
#[tokio::test]
async fn test_delete_removes_row() {
    let pool = make_pool().await;
    let store = ModelStore::new(pool);

    let meta = test_meta("del-1", "to-delete", ModelKind::Lora);
    store.upsert(&meta).await.expect("upsert should succeed");

    // Verify the row exists before deletion.
    assert!(
        store
            .get("del-1")
            .await
            .expect("get should succeed")
            .is_some(),
        "row should exist before delete"
    );

    store.delete("del-1").await.expect("delete should succeed");

    // After deletion, the row should be gone.
    let result = store
        .get("del-1")
        .await
        .expect("get after delete should succeed");
    assert!(
        result.is_none(),
        "row should not exist after delete: {:?}",
        result
    );
}

/// `get` on a non-existent ID returns `None`.
///
/// Does not insert any rows; directly queries for a nonexistent ID and
/// asserts that the result is `None` rather than an error.
#[tokio::test]
async fn test_get_missing_id_returns_none() {
    let pool = make_pool().await;
    let store = ModelStore::new(pool);

    let result = store
        .get("nonexistent-id")
        .await
        .expect("get should not error for missing ID");
    assert!(
        result.is_none(),
        "expected None for nonexistent ID, got {:?}",
        result
    );
}
