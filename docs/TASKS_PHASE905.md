[
  {
    "id": "P905-A1",
    "description": "anvilml-core: add F8E4M3 and F8E5M2 variants to DType enum",
    "phase": "905",
    "project": "anvilml",
    "prereqs": ["P20-A2"],
    "context": "In anvilml-core/src/types/model.rs add two DType variants after BF16: F8E4M3 (/// 8-bit float E4M3, torch float8_e4m3fn) and F8E5M2 (/// 8-bit float E5M2, torch float8_e5m2). serde snake_case serializes as f8_e4m3 / f8_e5m2. Update dtype_variants test: count 6->8, add both variants to vec and roundtrip array. Add test dtype_f8_serde_strings asserting exact JSON strings. Bump anvilml-core patch version. cargo test -p anvilml-core exits 0 with >=9 tests.",
    "tags": []
  },
  {
    "id": "P905-A2",
    "description": "anvilml-registry: extend infer_dtype with FP8 suffix matching and VRAM factor",
    "phase": "905",
    "project": "anvilml",
    "prereqs": ["P905-A1"],
    "context": "In anvilml-registry/src/scanner.rs extend infer_dtype: before bf16 check, match f8e4m3|fp8e4m3->F8E4M3, f8e5m2|fp8e5m2->F8E5M2, fp8|f8->F8E4M3 (default). Extend vram_estimate_mib: F8E4M3|F8E5M2 factor=0.5. Add tests: test_infer_dtype_fp8_suffixes covering fp8, f8, FP8, fp8e4m3, fp8e5m2, f8e4m3, f8e5m2 (case-insensitive). Update test_vram_estimate_mib for FP8. Bump anvilml-registry patch. cargo test -p anvilml-registry exits 0.",
    "tags": []
  },
  {
    "id": "P905-A3",
    "description": "anvilml-registry: safetensor header inspection for dtype detection",
    "phase": "905",
    "project": "anvilml",
    "prereqs": ["P905-A2"],
    "context": "In anvilml-registry/src/scanner.rs add fn read_safetensors_dtype(path:&Path)->Option<DType>: read 8-byte LE u64 header_len (guard >100MiB->None), read bytes, parse JSON, count dtype strings per key (skip __metadata__), return most-frequent mapped to DType (F32,F16,BF16,F8_E4M3->F8E4M3,F8_E5M2->F8E5M2,I8->Q8,I4->Q4), None on any error. In scan_dirs for .safetensors: call read_safetensors_dtype first; use result if Some and !=Unknown, else infer_dtype. Tests: header_wins_over_filename, header_fallback_on_malformed, safetensors_fp8_header. cargo test -p anvilml-registry exits 0.",
    "tags": ["reasoning"]
  },
  {
    "id": "P905-A4",
    "description": "anvilml-registry: remove stale model records on rescan",
    "phase": "905",
    "project": "anvilml",
    "prereqs": ["P905-A3"],
    "context": "In anvilml-registry/src/store.rs extend rescan(&self, dirs): after upserting all fresh ModelMeta, query DB for all model IDs whose path starts with any scanned dir root; compute stale_ids = db_ids minus fresh_ids; DELETE FROM models WHERE id=? for each stale id. Change rescan return type to Result<(usize,usize)> (upserted, removed). Update all callers in handlers/models.rs and main.rs. Add integration test tests/rescan_stale.rs: scan 2 files, delete 1, rescan, assert DB has 1 row and removed==1. cargo test -p anvilml-registry exits 0.",
    "tags": ["reasoning"]
  },
  {
    "id": "P905-A5",
    "description": "anvilml-registry: ModelMetaPatch type and store patch_meta method",
    "phase": "905",
    "project": "anvilml",
    "prereqs": ["P905-A4"],
    "context": "In anvilml-core/src/types/model.rs add ModelMetaPatch { dtype_hint:Option<DType>, kind:Option<ModelKind> } deriving Debug,Deserialize,ToSchema. In anvilml-registry/src/store.rs add async fn patch_meta(&self, id:&str, patch:ModelMetaPatch)->Result<Option<ModelMeta>>: get current record (None->Ok(None)); apply Some fields; recompute vram_estimate_mib via vram_estimate_mib(size_bytes,dtype_hint) from scanner; upsert; return updated. Add unit test patch_meta_updates_dtype_recomputes_vram. Bump anvilml-registry patch. cargo test -p anvilml-registry exits 0.",
    "tags": ["reasoning"]
  },
  {
    "id": "P905-A6",
    "description": "anvilml-server: PATCH /v1/models/:id metadata override endpoint",
    "phase": "905",
    "project": "anvilml",
    "prereqs": ["P905-A5"],
    "context": "In anvilml-server/src/handlers/models.rs add patch_model handler: PATCH /v1/models/:id, extract Json<ModelMetaPatch>, call registry.patch_meta; map None->404, Ok->200 with updated ModelMeta. Add #[utoipa::path] annotation. Wire route in lib.rs. Add tests: patch_model_updates_dtype_hint (dtype+vram change), patch_model_returns_404, patch_model_partial_preserves_other_fields. Bump anvilml-server patch. cargo test -p anvilml-server --features mock-hardware exits 0.",
    "tags": []
  },
  {
    "id": "P905-A7",
    "description": "backend: fix cancel_terminal_job_returns_409 CI failure",
    "phase": "905",
    "project": "anvilml",
    "prereqs": ["P20-A2"],
    "context": "In backend/tests/api_cancel.rs in cancel_terminal_job_returns_409: add (\"ANVILML_WORKER_MOCK\", Some(\"1\")) to the temp_env::async_with_vars vars array (alongside existing ANVILML_MOCK_DEVICE_TYPE and ANVILML_MOCK_VRAM_MIB). Add std::env::remove_var(\"ANVILML_WORKER_MOCK\") to the unconditional cleanup block at end of function. Bump backend patch version. cargo test --features mock-hardware --test api_cancel exits 0, both tests pass.",
    "tags": []
  }
]