# Plan Report: P7-D3

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P7-D3                                         |
| Phase       | 007 — WebSocket Event Stream                |
| Description | anvilml-registry + anvilml-server: silent error discard fixes |
| Depends on  | P7-D2                                         |
| Project     | anvilml                                       |
| Planned at  | 2026-06-04T21:30:00Z                          |
| Attempt     | 1                                             |

## Objective

Replace silent error discards in two crates so that scanner walkdir/metadata errors and canonicalize failures emit `tracing::warn!` with the affected path, and model HTTP handlers log database errors and return informative JSON error bodies instead of silently swallowing them.

## Scope

### In Scope
- `crates/anvilml-registry/src/scanner.rs` — three silent discard sites in `scan_dirs()`:
  1. Walkdir entry iterator `Err(_)` arm (line ~82): replace with `tracing::warn!` including directory path, then `continue`.
  2. `entry.metadata()` `Err(_)` arm (line ~102): replace with `tracing::warn!` including the file path, then `continue`.
  3. `canonicalize().unwrap_or_else(|_| entry.path().to_path_buf())` (lines ~114-117): replace with an explicit `match` that warns on error before falling back to the raw path.
- `crates/anvilml-server/src/handlers/models.rs` — two database error discard sites:
  1. `list_models` handler `Err(_e)` arm (line ~26): add `tracing::error!` log and change response body from `Json(vec![])` to a JSON error object `{ "error": "internal_error", "message": ... }`.
  2. `get_model` handler `Err(_e)` arm (lines ~48-51): add `tracing::error!` log and include the error message in the response body.

### Out of Scope
- Any changes to `anvilml-hardware` (handled by P7-D2).
- New tests — the task specifies that existing tests must continue to pass without modification.
- Changes to CI, Cargo.toml, or any other crate.
- Refactoring of scanner logic beyond the three error sites.

## Approach

### 1. scanner.rs — walkdir entry error (lines 80-83)

**Current code:**
```rust
let entry = match entry {
    Ok(e) => e,
    Err(_) => continue,
};
```

**Change:** Replace with a match that binds the error and emits a warn log:
```rust
let entry = match entry {
    Ok(e) => e,
    Err(e) => {
        tracing::warn!(path = %dir_config.path.display(), error = %e, "scanner: skipping unreadable entry");
        continue;
    }
};
```

This requires adding `use tracing;` at the top of the file (the crate's dependency on `tracing` is already declared in its Cargo.toml since other modules use it).

### 2. scanner.rs — metadata error (lines 100-103)

**Current code:**
```rust
let size_bytes = match entry.metadata() {
    Ok(m) => m.len(),
    Err(_) => continue,
};
```

**Change:**
```rust
let size_bytes = match entry.metadata() {
    Ok(m) => m.len(),
    Err(e) => {
        tracing::warn!(path = %entry.path().display(), error = %e, "scanner: skipping file with unreadable metadata");
        continue;
    }
};
```

### 3. scanner.rs — canonicalize silent fallback (lines 114-117)

**Current code:**
```rust
let canonical_path = entry
    .path()
    .canonicalize()
    .unwrap_or_else(|_| entry.path().to_path_buf());
```

**Change:**
```rust
let canonical_path = match entry.path().canonicalize() {
    Ok(p) => p,
    Err(e) => {
        tracing::warn!(path = %entry.path().display(), error = %e, "scanner: canonicalize failed, using raw path");
        entry.path().to_path_buf()
    }
};
```

### 4. models.rs — list_models handler (lines 20-28)

**Current code:**
```rust
pub async fn list_models(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ModelsListQuery>,
) -> (StatusCode, Json<Vec<anvilml_core::ModelMeta>>) {
    match state.registry.list(query.kind).await {
        Ok(models) => (StatusCode::OK, Json(models)),
        Err(_e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(vec![])),
    }
}
```

**Change:** The return type must change to `(StatusCode, Json<serde_json::Value>)` so both arms can return different JSON shapes. The success arm wraps `Vec<ModelMeta>` into a `Json(serde_json::Value)` via `.into()` (which is valid since `Vec<T: Serialize` implements `Into<serde_json::Value>`).

```rust
pub async fn list_models(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ModelsListQuery>,
) -> (StatusCode, Json<serde_json::Value>) {
    match state.registry.list(query.kind).await {
        Ok(models) => (
            StatusCode::OK,
            Json(serde_json::to_value(&models).unwrap()),
        ),
        Err(e) => {
            tracing::error!(error = %e, "list_models: registry query failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "internal_error",
                    "message": e.to_string()
                })),
            )
        }
    }
}
```

### 5. models.rs — get_model handler (lines 35-53)

**Current code:**
```rust
Err(_e) => (
    StatusCode::INTERNAL_SERVER_ERROR,
    Json(serde_json::json!({"error": "internal_error"})),
),
```

**Change:** Add logging and include the error message:
```rust
Err(e) => {
    tracing::error!(error = %e, "get_model: registry query failed");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({
            "error": "internal_error",
            "message": e.to_string()
        })),
    )
}
```

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-registry/src/scanner.rs` | Add `use tracing;`, replace 3 silent discard sites with warn+continue/fallback |
| Modify | `crates/anvilml-server/src/handlers/models.rs` | Add `tracing::error!` to both error arms, update `list_models` return type, include error messages in JSON bodies |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|-----------------|
| `crates/anvilml-registry/src/scanner.rs` (unit tests) | `test_infer_kind_matches`, `test_infer_kind_case_insensitive`, `test_infer_kind_fallback`, `test_infer_dtype_matches`, `test_infer_dtype_case_insensitive`, `test_infer_dtype_unknown`, `test_vram_estimate_mib`, `test_sha256_hex` | All existing unit tests in scanner.rs pass — no regression from added logging (logging does not affect testable behavior) |
| `crates/anvilml-server/tests/api_models.rs` | `list_models_returns_scanned_models` | Success path still returns JSON array (the `Value` type can represent an array; success arm unchanged semantically) |
| `crates/anvilml-server/tests/api_models.rs` | `list_models_kind_filter_diffusion` | Same as above, with kind filter |
| `crates/anvilml-server/tests/api_models.rs` | `list_models_kind_filter_no_match` | Same as above, empty array case |

No new test files are written. The task scope does not require them — the changes only add logging and improve error response bodies; all success paths remain functionally identical.

## CI Impact

No CI workflow files are modified. The existing CI matrix (rust fmt+clippy+test on Linux, clippy+test on Windows) already covers these crates. The `cargo clippy --workspace --features mock-hardware` lint gate will catch any new warnings introduced by the changes. No new CI jobs or steps are added.

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Changing `list_models` return type from `(StatusCode, Json<Vec<ModelMeta>>)` to `(StatusCode, Json<serde_json::Value>)` could break the existing test assertions if `Vec<ModelMeta>` does not serialize correctly into `serde_json::Value`. | The success arm serializes via `serde_json::to_value(&models).unwrap()` which produces an identical JSON array to what `Json(Vec<ModelMeta>)` produced. The tests parse as `serde_json::Value` and assert `.is_array()`, so they remain compatible. Verified by running the test suite after implementation. |
| Adding `use tracing;` to scanner.rs requires that the crate already depends on `tracing`. If it does not, compilation will fail. | `anvilml-registry` already uses `tracing::info!` in `store.rs`, so the dependency exists. Verified by reading store.rs which contains `tracing::warn!("background rescan failed: {e}")`. |
| The `canonicalize()` warn message could be noisy if many files have permission issues. | `tracing::warn!` is appropriate — it surfaces the problem without being as severe as error level, and includes the path so operators can identify which directory/file is affected. |
| `e.to_string()` on `AnvilError` could produce very long messages in the JSON response body. | Acceptable for a 500 error response; the message is already wrapped in an error JSON object with `"error": "internal_error"` as the primary key. The client can decide how to display it. |

## Acceptance Criteria

- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
- [ ] `cargo test --workspace --features mock-hardware` exits 0
- [ ] `list_models` error arm returns `{ "error": "internal_error", "message": "<details>" }` at 500 status (verified by inspection of code; no new test required)
- [ ] `get_model` error arm includes the error message in the JSON body and logs via `tracing::error!`
- [ ] `scanner.rs` walkdir entry errors emit `tracing::warn!(path = ..., error = ...)` before continuing
- [ ] `scanner.rs` metadata errors emit `tracing::warn!(path = ..., error = ...)` before continuing
- [ ] `scanner.rs` canonicalize failures emit `tracing::warn!(path = ..., error = ...)` before falling back
