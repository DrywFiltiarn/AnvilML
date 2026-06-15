/// Integration tests for `store.rs` — `ModelStore` SQLite CRUD operations.
///
/// These tests verify the complete lifecycle of model metadata persistence:
/// - Upsert (insert and overwrite)
/// - Get (single lookup by ID)
/// - List (all or filtered by kind)
/// - Delete (by ID, with existence check)
///
/// Each test creates its own in-memory database via `open_in_memory()`,
/// ensuring complete database isolation between tests.
use anvilml_core::{ModelDtype, ModelFormat, ModelKind, ModelMeta};
use anvilml_registry::{open_in_memory, ModelStore};

use chrono::Utc;

/// Helper to construct a `ModelMeta` for tests.
///
/// Uses a fixed scanned_at timestamp and generates deterministic IDs.
fn make_meta(id: &str, name: &str, kind: ModelKind) -> ModelMeta {
    ModelMeta {
        id: id.to_string(),
        name: name.to_string(),
        path: format!("/models/{}", name),
        kind,
        dtype: ModelDtype::Fp16,
        format: ModelFormat::Safetensors,
        size_bytes: 1_073_741_824,
        scanned_at: Utc::now(),
    }
}

/// Verifies that `upsert()` persists a model and `get()` retrieves it with
/// all fields intact.
///
/// Creates a `ModelMeta` for a diffusion model, upserts it, then retrieves
/// it by ID and asserts that every field matches the original.
#[tokio::test]
async fn test_upsert_and_get() {
    let pool = open_in_memory().await.expect("open in-memory DB");
    let store = ModelStore::new(pool).await;

    let meta = make_meta("model-1", "stable-diffusion-v1-5", ModelKind::Diffusion);

    store.upsert(&meta).await.expect("upsert should succeed");

    let retrieved = store
        .get("model-1")
        .await
        .expect("get should succeed")
        .expect("model should exist");

    assert_eq!(retrieved.id, meta.id);
    assert_eq!(retrieved.name, meta.name);
    assert_eq!(retrieved.path, meta.path);
    assert_eq!(retrieved.kind, meta.kind);
    assert_eq!(retrieved.dtype, meta.dtype);
    assert_eq!(retrieved.format, meta.format);
    assert_eq!(retrieved.size_bytes, meta.size_bytes);
}

/// Verifies that `upsert()` overwrites an existing model record when the
/// same ID is used with different data.
///
/// Upserts a model with one name, then upserts the same ID with a
/// different name. Asserts that `get()` returns the second version.
#[tokio::test]
async fn test_upsert_overwrites() {
    let pool = open_in_memory().await.expect("open in-memory DB");
    let store = ModelStore::new(pool).await;

    let first = make_meta("model-1", "original-name", ModelKind::Diffusion);
    let second = make_meta("model-1", "updated-name", ModelKind::Diffusion);

    store
        .upsert(&first)
        .await
        .expect("first upsert should succeed");
    store
        .upsert(&second)
        .await
        .expect("second upsert should succeed");

    let retrieved = store
        .get("model-1")
        .await
        .expect("get should succeed")
        .expect("model should exist");

    assert_eq!(
        retrieved.name, "updated-name",
        "upsert with same ID must overwrite the previous record"
    );
}

/// Verifies that `get()` returns `None` for a non-existent model ID.
///
/// Creates a fresh in-memory database with no models inserted, then
/// calls `get()` with an arbitrary ID string. Asserts that the result
/// is `None`, not an error.
#[tokio::test]
async fn test_get_not_found() {
    let pool = open_in_memory().await.expect("open in-memory DB");
    let store = ModelStore::new(pool).await;

    let result = store
        .get("non-existent-id")
        .await
        .expect("get should not error for missing ID");

    assert!(result.is_none(), "get for non-existent ID must return None");
}

/// Verifies that `list(None)` returns all upserted models when no
/// kind filter is applied.
///
/// Upserts three models with different kinds (Diffusion, Vae, TextEncoder),
/// then calls `list(None)` and asserts that all three are returned.
#[tokio::test]
async fn test_list_all() {
    let pool = open_in_memory().await.expect("open in-memory DB");
    let store = ModelStore::new(pool).await;

    let diffusion = make_meta("model-1", "sd-v1-5", ModelKind::Diffusion);
    let vae = make_meta("model-2", "vae-model", ModelKind::Vae);
    let text_encoder = make_meta("model-3", "clip-text", ModelKind::TextEncoder);

    store.upsert(&diffusion).await.expect("upsert diffusion");
    store.upsert(&vae).await.expect("upsert vae");
    store
        .upsert(&text_encoder)
        .await
        .expect("upsert text_encoder");

    let all = store.list(None).await.expect("list should succeed");

    assert_eq!(
        all.len(),
        3,
        "list without filter must return all 3 models, found {}",
        all.len()
    );
}

/// Verifies that `list(Some(kind))` returns only models matching the
/// specified kind.
///
/// Upserts two Diffusion models and one Vae model, then calls
/// `list(Some(ModelKind::Vae))` and asserts that only the Vae model
/// is returned.
#[tokio::test]
async fn test_list_filter_by_kind() {
    let pool = open_in_memory().await.expect("open in-memory DB");
    let store = ModelStore::new(pool).await;

    let diffusion1 = make_meta("model-1", "sd-v1-5", ModelKind::Diffusion);
    let diffusion2 = make_meta("model-2", "sd-xl", ModelKind::Diffusion);
    let vae = make_meta("model-3", "vae-model", ModelKind::Vae);

    store.upsert(&diffusion1).await.expect("upsert diffusion1");
    store.upsert(&diffusion2).await.expect("upsert diffusion2");
    store.upsert(&vae).await.expect("upsert vae");

    let vae_only = store
        .list(Some(ModelKind::Vae))
        .await
        .expect("list with filter should succeed");

    assert_eq!(
        vae_only.len(),
        1,
        "list with Vae filter must return exactly 1 model, found {}",
        vae_only.len()
    );
    assert_eq!(
        vae_only[0].kind,
        ModelKind::Vae,
        "the returned model must be of kind Vae"
    );
}

/// Verifies that `delete()` returns `true` for an existing model and
/// that subsequent `get()` returns `None`.
///
/// Upserts a model, deletes it, asserts that `delete()` returned `true`,
/// then calls `get()` and asserts that it returns `None`.
#[tokio::test]
async fn test_delete_existing() {
    // Use a pool with max_connections(1) to ensure a single connection
    // is shared across all operations, avoiding SQLite in-memory per-
    // connection database issues.
    use sqlx::pool::PoolOptions;

    let pool = PoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("connect to in-memory DB");

    // Run migrations on the pool.
    sqlx::migrate!("../../database/migrations")
        .run(&pool)
        .await
        .expect("run migrations");

    // Clone the pool so we can use it directly for verification queries
    // while the store also holds a reference to the same pool.
    let store = ModelStore::new(pool.clone()).await;

    let meta = make_meta("model-1", "to-delete", ModelKind::Diffusion);

    store.upsert(&meta).await.expect("upsert should succeed");

    let deleted = store
        .delete("model-1")
        .await
        .expect("delete should succeed");

    assert!(deleted, "delete of existing model must return true");

    // Verify the model was actually deleted by querying the database
    // directly.
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM models WHERE id = ?")
        .bind("model-1")
        .fetch_one(&pool)
        .await
        .expect("query should succeed");

    assert_eq!(
        count, 0,
        "model should have been deleted, but COUNT(*) returned {}",
        count
    );

    // Also verify get() returns None.
    let after_delete = store.get("model-1").await.expect("get should succeed");

    assert!(after_delete.is_none(), "get after delete must return None");
}

/// Verifies that `delete()` returns `false` for a non-existent model ID
/// without raising an error.
///
/// Creates a fresh in-memory database with no models, then calls
/// `delete()` with an arbitrary ID. Asserts that the result is `false`.
#[tokio::test]
async fn test_delete_not_found() {
    let pool = open_in_memory().await.expect("open in-memory DB");
    let store = ModelStore::new(pool).await;

    let deleted = store
        .delete("non-existent-id")
        .await
        .expect("delete should not error for missing ID");

    assert!(!deleted, "delete of non-existent model must return false");
}
