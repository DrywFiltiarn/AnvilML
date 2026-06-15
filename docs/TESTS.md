# Test Catalogue

## test_default_values (anvilml-core)

**File:** `crates/anvilml-core/tests/config_tests.rs`
**Context:** `ServerConfig::default()` is callable — no I/O, no subprocess, no network.
**Tests:** All `ServerConfig` and nested struct fields match documented defaults from `ENVIRONMENT.md §4`.
**Inputs:** None (uses `ServerConfig::default()`).
**Expected output:** Every assertion passes — no field deviates from its documented default.

## test_serialisation_roundtrip (anvilml-core)

**File:** `crates/anvilml-core/tests/config_tests.rs`
**Context:** `ServerConfig` derives `Serialize` and `Deserialize` correctly; `PathBuf` fields round-trip as JSON strings via the `path_as_string` helper.
**Tests:** `Serialize`/`Deserialize` roundtrip preserves all field values including `PathBuf` and `Option` fields.
**Inputs:** `ServerConfig::default()` serialised to JSON via `serde_json::to_string`, deserialised back via `serde_json::from_str`.
**Expected output:** `from_str(&to_string(&cfg)) == cfg` — the roundtripped config is byte-identical to the original.

## test_env_override_values (anvilml-core)

**File:** `crates/anvilml-core/tests/config_tests.rs`
**Context:** Config struct correctly handles non-default values including `Option::Some` variants — mimics what environment variable overrides would produce.
**Tests:** All overridden values survive a JSON serialisation roundtrip.
**Inputs:** `ServerConfig` constructed with `host = "0.0.0.0"`, `port = 9001`, `max_ipc_payload_mib = 512`, `rocm = Some(RocmConfig { hsa_override_gfx_version: Some("gfx942") })`, `hardware_override = Some(HardwareOverrideConfig { device_type: "cuda", vram_total_mib: 16384 })`, plus custom paths and model_dirs.
**Expected output:** All overridden values are preserved after `to_string` → `from_str` roundtrip.

## test_missing_file_uses_defaults (anvilml-core)

**File:** `crates/anvilml-core/tests/config_load_tests.rs`
**Context:** `load()` is callable with a nonexistent TOML path — no I/O failure, no panic.
**Tests:** When the TOML file does not exist, `load()` returns `ServerConfig::default()` with all compiled-in defaults intact.
**Inputs:** `path = "/nonexistent/path.toml"`, `overrides = ConfigOverrides::default()`.
**Expected output:** `Result::Ok(ServerConfig::default())` — every field matches the documented default.

## test_env_var_beats_toml (anvilml-core)

**File:** `crates/anvilml-core/tests/config_load_tests.rs`
**Context:** Environment variable overrides take precedence over TOML file values for the same field.
Process-global `std::env` is non-atomic; concurrent threads can observe `set_var` mid-flight. Annotated with `#[serial]` to serialise execution and eliminate the race window.
**Tests:** A TOML file with `port = 9001` is loaded while `ANVILML_PORT=8080` is set; the env var value wins.
**Inputs:** TOML file with `port = 9001`, `ANVILML_PORT=8080`, `overrides = ConfigOverrides::default()`.
**Expected output:** `cfg.port == 8080` (env beats toml).

## test_cli_override_beats_env (anvilml-core)

**File:** `crates/anvilml-core/tests/config_load_tests.rs`
**Context:** CLI overrides take precedence over environment variables, which take precedence over TOML.
Process-global `std::env` is non-atomic; concurrent threads can observe `set_var` mid-flight. Annotated with `#[serial]` to serialise execution and eliminate the race window.
**Tests:** A TOML file with `port = 9001`, env `ANVILML_PORT=8080`, and `overrides.port = Some(7070)` — the CLI override wins.
**Inputs:** TOML `port = 9001`, `ANVILML_PORT=8080`, `overrides.port = Some(7070)`.
**Expected output:** `cfg.port == 7070` (CLI beats env beats toml).

## test_nested_env_var (anvilml-core)

**File:** `crates/anvilml-core/tests/config_load_tests.rs`
**Context:** Double-underscore nesting in env vars correctly maps to nested config fields.
Process-global `std::env` is non-atomic; concurrent threads can observe `set_var` mid-flight. Annotated with `#[serial]` to serialise execution and eliminate the race window.
**Tests:** A TOML file without a `gpu_selection` section is loaded with `ANVILML_GPU_SELECTION__DEFAULT_DEVICE=cpu`; the nested field is set via the env var.
**Inputs:** TOML without `gpu_selection`, `ANVILML_GPU_SELECTION__DEFAULT_DEVICE=cpu`.
**Expected output:** `cfg.gpu_selection.default_device == "cpu"`.

## test_job_json_roundtrip (anvilml-core)

**File:** `crates/anvilml-core/tests/job_tests.rs`
**Context:** `Job` derives `Serialize` and `Deserialize` correctly; all fields including `Option` variants and nested `JobSettings` round-trip through JSON.
**Tests:** A fully-populated `Job` (with `id`, `status=Running`, graph JSON, `settings={device_preference: Some("cuda")}`, `created_at`, `started_at`, `completed_at=None`, `worker_id=Some("worker-0")`, `error=None`, `queue_position=Some(1)`) serialises to JSON and deserialises back to an identical value.
**Inputs:** `Job` constructed with all fields set to non-trivial values.
**Expected output:** `from_str(&to_string(&job)) == job` — every field matches the original exactly.

## test_job_settings_default (anvilml-core)

**File:** `crates/anvilml-core/tests/job_tests.rs`
**Context:** `JobSettings::default()` implements `Default` correctly, producing `device_preference: None` which means auto-select by VRAM.
**Tests:** `JobSettings::default().device_preference` is `None`.
**Inputs:** `JobSettings::default()`.
**Expected output:** `device_preference == None`.

## test_job_status_variants (anvilml-core)

**File:** `crates/anvilml-core/tests/job_tests.rs`
**Context:** `JobStatus` enum derives `Serialize` and `Deserialize` correctly; all five variants round-trip through JSON without data loss.
**Tests:** Each of the five variants (`Queued`, `Running`, `Completed`, `Failed`, `Cancelled`) serialises to its snake_case string and deserialises back to the same variant.
**Inputs:** Each `JobStatus` variant individually.
**Expected output:** Each variant survives `to_string` → `from_str` roundtrip unchanged.

## test_submit_job_request_default (anvilml-core)

**File:** `crates/anvilml-core/tests/job_tests.rs`
**Context:** `SubmitJobRequest::default()` produces a well-formed request with `graph = Null` and `settings.device_preference = None`.
**Tests:** Default `SubmitJobRequest` has a null graph and no device preference.
**Inputs:** `SubmitJobRequest::default()`.
**Expected output:** `graph.is_null() == true` and `settings.device_preference.is_none() == true`.

## test_submit_job_response_default (anvilml-core)

**File:** `crates/anvilml-core/tests/job_tests.rs`
**Context:** `SubmitJobResponse::default()` produces a well-formed response with `job_id` as the UUID zero value and `queue_position = 0`.
**Tests:** Default `SubmitJobResponse` has a zero UUID and zero queue position.
**Inputs:** `SubmitJobResponse::default()`.
**Expected output:** `job_id == Uuid::default()` and `queue_position == 0`.

## test_custom_port_health (anvilml)

**File:** `backend/tests/cli_tests.rs`
**Context:** The server binary accepts `--port` CLI override, binds to the OS-assigned port, and the health endpoint returns HTTP 200.
**Tests:** Spawns the pre-built anvilml binary with `--port 0` (OS-assigned port), detects the bound port via platform-specific tooling (`lsof` on Unix, `netstat` on Windows), sends `GET /health`, and asserts HTTP 200 with `{"status":"ok"}`.
**Inputs:** Binary path from `CARGO_TARGET_DIR` (or `target/debug/anvilml`), `--port 0`, `--log-format plain`.
**Expected output:** HTTP 200 response with JSON body containing `"status":"ok"`.
**Acceptance command:** `cargo test -p anvilml --features mock-hardware -- cli_tests` exits 0.

**Environment isolation:** The test clears all `ANVILML_*` env vars at startup and restores them at teardown to prevent pollution of parallel test runs. The subprocess is killed unconditionally on test exit.

## test_config_reference (anvilml)

**File:** `backend/tests/config_reference.rs`
**Context:** The checked-in `anvilml.toml` has the same key set as `ServerConfig::default()` serialised to TOML. This is the config drift guard (Gate 1).
**Tests:** Serialises `ServerConfig::default()` to a TOML string via `toml::to_string_pretty`, reads `anvilml.toml` from the repo root, parses both into `toml::Value`, recursively collects all keys from each tree into a `BTreeSet<String>`, and asserts the two key sets are equal.
**Inputs:** `ServerConfig::default()` serialised to TOML; file content of `anvilml.toml`.
**Expected output:** Both key sets are equal — the test exits 0.
**Acceptance command:** `cargo test -p anvilml --features mock-hardware -- config_reference` exits 0.

## test_model_meta_json_roundtrip (anvilml-core)

**File:** `crates/anvilml-core/tests/model_tests.rs`
**Context:** `ModelMeta` derives `Serialize` and `Deserialize` correctly; all fields including `PathBuf`, `DateTime<Utc>`, and enum types round-trip through JSON.
**Tests:** A fully-populated `ModelMeta` (with `id`, `name`, `path`, `kind=Diffusion`, `dtype=Fp32`, `format=Safetensors`, `size_bytes`, `scanned_at`) serialises to JSON and deserialises back to an identical value.
**Inputs:** `ModelMeta` constructed with all fields set to non-trivial values.
**Expected output:** `from_str(&to_string(&meta)) == meta` — every field matches the original exactly.

## test_model_kind_variants (anvilml-core)

**File:** `crates/anvilml-core/tests/model_tests.rs`
**Context:** `ModelKind` enum derives `Serialize` and `Deserialize` correctly with `#[serde(rename_all = "snake_case")]`; all seven variants round-trip through JSON without data loss.
**Tests:** Each of the seven variants (`Diffusion`, `TextEncoder`, `Vae`, `Lora`, `ControlNet`, `Upscale`, `Unknown`) serialises to its snake_case string and deserialises back to the same variant.
**Inputs:** Each `ModelKind` variant individually.
**Expected output:** Each variant survives `to_string` → `from_str` roundtrip unchanged.

## test_model_dtype_format_variants (anvilml-core)

**File:** `crates/anvilml-core/tests/model_tests.rs`
**Context:** `ModelDtype` and `ModelFormat` enums derive `Serialize` and `Deserialize` correctly with `#[serde(rename_all = "snake_case")]`; all variants round-trip through JSON.
**Tests:** All `ModelDtype` variants (`Fp32`, `Fp16`, `Bf16`, `Fp8`, `Fp4`, `Unknown`) and all `ModelFormat` variants (`Safetensors`, `Ckpt`, `Pt`, `Bin`, `Unknown`) serialise to their snake_case strings and deserialise back.
**Inputs:** Each `ModelDtype` and `ModelFormat` variant individually.
**Expected output:** Each variant survives `to_string` → `from_str` roundtrip unchanged.

## test_artifact_meta_json_roundtrip (anvilml-core)

**File:** `crates/anvilml-core/tests/artifact_tests.rs`
**Context:** `ArtifactMeta` derives `Serialize` and `Deserialize` correctly; all fields including `Uuid`, `PathBuf`, `DateTime<Utc>`, and the SHA-256 hash string round-trip through JSON.
**Tests:** A fully-populated `ArtifactMeta` (with `id`, `job_id`, `hash` (64 hex chars), `path`, `size_bytes`, `created_at`) serialises to JSON and deserialises back to an identical value.
**Inputs:** `ArtifactMeta` constructed with all fields set to non-trivial values.
**Expected output:** `from_str(&to_string(&artifact)) == artifact` — every field matches the original exactly.

## test_artifact_meta_default (anvilml-core)

**File:** `crates/anvilml-core/tests/artifact_tests.rs`
**Context:** `ArtifactMeta` derives `Default` correctly, producing a well-formed struct with zero/empty defaults.
**Tests:** `ArtifactMeta::default()` produces `id = ""`, `job_id = Uuid::default()`, `hash = ""`, `path = PathBuf::new()`, `size_bytes = 0`.
**Inputs:** `ArtifactMeta::default()`.
**Expected output:** All default fields are zero/empty as documented.

## test_artifact_hash_format (anvilml-core)

**File:** `crates/anvilml-core/tests/artifact_tests.rs`
**Context:** The `hash: String` field correctly serialises and deserialises a SHA-256 hex digest (64 lowercase hex characters) without any unexpected escaping, truncation, or case transformation.
**Tests:** An `ArtifactMeta` with a 64-character lowercase hex hash roundtrips through JSON, and the restored hash matches the original exactly.
**Inputs:** `ArtifactMeta` with `hash = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789"`.
**Expected output:** `restored.hash == original.hash` — the SHA-256 hex string survives JSON roundtrip unchanged.

## test_hardware_info_json_roundtrip (anvilml-core)

**File:** `crates/anvilml-core/tests/hardware_tests.rs`
**Context:** `HardwareInfo` derives `Serialize` and `Deserialize` correctly; all fields including nested `GpuDevice`, `HostInfo`, and `InferenceCaps` structs round-trip through JSON.
**Tests:** A fully-populated `HardwareInfo` (with `host` containing OS/CPU/RAM, two `GpuDevice` entries with mixed `Option<String>` values for `arch`, and `inference_caps` as the union of per-device capabilities) serialises to JSON and deserialises back to an identical value.
**Inputs:** `HardwareInfo` constructed with two `GpuDevice` entries, one with `fp8=true`, one with `fp8=false`, to test the union logic.
**Expected output:** `from_str(&to_string(&hardware)) == hardware` — every field matches the original exactly.

## test_device_type_variants (anvilml-core)

**File:** `crates/anvilml-core/tests/hardware_tests.rs`
**Context:** `DeviceType` enum derives `Serialize` and `Deserialize` correctly with `#[serde(rename_all = "snake_case")]`; all three variants round-trip through JSON without data loss.
**Tests:** Each of the three variants (`Cuda`, `Rocm`, `Cpu`) serialises to its snake_case string and deserialises back to the same variant.
**Inputs:** Each `DeviceType` variant individually.
**Expected output:** Each variant survives `to_string` → `from_str` roundtrip unchanged.

## test_inference_caps_default (anvilml-core)

**File:** `crates/anvilml-core/tests/hardware_tests.rs`
**Context:** `InferenceCaps` derives `Default` correctly, producing all-false bool fields representing the "unknown" initial state before the Python worker reports actual capabilities.
**Tests:** `InferenceCaps::default()` has all six bool fields (`fp32`, `fp16`, `bf16`, `fp8`, `fp4`, `flash_attention`) set to `false`.
**Inputs:** `InferenceCaps::default()`.
**Expected output:** All six bool fields are `false`.

## test_enum_variants_roundtrip (anvilml-core)

**File:** `crates/anvilml-core/tests/hardware_tests.rs`
**Context:** `EnumerationSource` and `CapabilitySource` enums derive `Serialize` and `Deserialize` correctly with `#[serde(rename_all = "snake_case")]`; all variants round-trip through JSON.
**Tests:** All 6 `EnumerationSource` variants (`Vulkan`, `Dxgi`, `Sysfs`, `Nvml`, `Mock`, `Override`) and all 3 `CapabilitySource` variants (`PyTorch`, `DeviceTable`, `Fallback`) serialise to their snake_case strings and deserialise back.
**Inputs:** Each of the 9 enum variants individually.
**Expected output:** Each variant survives `to_string` → `from_str` roundtrip unchanged.

## test_node_type_descriptor_json_roundtrip (anvilml-core)

**File:** `crates/anvilml-core/tests/node_tests.rs`
**Context:** `NodeTypeDescriptor` derives `Serialize` and `Deserialize` correctly; all fields including nested `SlotDescriptor` vectors with mixed optional flags round-trip through JSON.
**Tests:** A fully-populated `NodeTypeDescriptor` (with `type_name`, `display_name`, `category`, `description`, 3 inputs including one optional, and 2 outputs) serialises to JSON and deserialises back to an identical value.
**Inputs:** `NodeTypeDescriptor` constructed with inputs `samples` (Latent, required), `model` (Model, required), `positive` (Conditioning, optional) and outputs `samples` (Latent, required), `denoised` (Latent, required).
**Expected output:** `from_str(&to_string(&node)) == node` — every field matches the original exactly.

## test_slot_type_variants (anvilml-core)

**File:** `crates/anvilml-core/tests/node_tests.rs`
**Context:** `SlotType` enum derives `Serialize` and `Deserialize` correctly with `#[serde(rename_all = "SCREAMING_SNAKE_CASE")]`; all 11 variants round-trip through JSON with correct uppercase keys matching the Python worker's convention.
**Tests:** Each of the 11 variants (`Model`, `Clip`, `Vae`, `Conditioning`, `Latent`, `Image`, `String`, `Int`, `Float`, `Bool`, `Any`) serialises to its SCREAMING_SNAKE_CASE string and deserialises back to the same variant.
**Inputs:** Each `SlotType` variant individually.
**Expected output:** Each variant survives `to_string` → `from_str` roundtrip unchanged.

## test_slot_descriptor_optional_field (anvilml-core)

**File:** `crates/anvilml-core/tests/node_tests.rs`
**Context:** `SlotDescriptor` derives `Serialize` and `Deserialize` correctly; the `optional` boolean field is preserved through JSON roundtrip.
**Tests:** A `SlotDescriptor` with `optional: true` roundtrips through JSON, and the restored `optional` field equals `true`.
**Inputs:** `SlotDescriptor{name="seed", slot_type=Int, optional=true}`.
**Expected output:** `restored.optional == true` — the optional flag survives JSON roundtrip unchanged.

## test_worker_info_json_roundtrip (anvilml-core)

**File:** `crates/anvilml-core/tests/worker_tests.rs`
**Context:** `WorkerInfo` derives `Serialize` and `Deserialize` correctly; all fields including `Option` variants with `Some` values round-trip through JSON.
**Tests:** A fully-populated `WorkerInfo` (with `id`, `device_index=0`, `device_name`, `status=Busy`, `current_job_id=Some(uuid)`, `vram_used_mib=Some(12288)`) serialises to JSON and deserialises back to an identical value.
**Inputs:** `WorkerInfo` constructed with all fields set to non-trivial values.
**Expected output:** `from_str(&to_string(&worker)) == worker` — every field matches the original exactly.

## test_worker_status_variants (anvilml-core)

**File:** `crates/anvilml-core/tests/worker_tests.rs`
**Context:** `WorkerStatus` enum derives `Serialize` and `Deserialize` correctly with `#[serde(rename_all = "snake_case")]`; all 5 variants round-trip through JSON without data loss.
**Tests:** Each of the 5 variants (`Initializing`, `Idle`, `Busy`, `Dead`, `Respawning`) serialises to its snake_case string and deserialises back to the same variant.
**Inputs:** Each `WorkerStatus` variant individually.
**Expected output:** Each variant survives `to_string` → `from_str` roundtrip unchanged.

## test_env_report_default_preflight (anvilml-core)

**File:** `crates/anvilml-core/tests/worker_tests.rs`
**Context:** `EnvReport` derives `Serialize` and `Deserialize` correctly; all `Option` fields with `None` values and the `provisioning` enum round-trip through JSON.
**Tests:** An `EnvReport` with `preflight_ok=false`, `provisioning=NotStarted`, `reason=Some("Python not yet launched")`, and empty `node_types` vector roundtrips correctly.
**Inputs:** `EnvReport{python_path: None, python_version: None, torch_version: None, provisioning: NotStarted, preflight_ok: false, reason: Some("Python not yet launched"), node_types: []}`.
**Expected output:** `from_str(&to_string(&report)) == report` — every field matches, and `node_types` is an empty vec.

## test_ws_event_roundtrip_job_image_ready (anvilml-core)

**File:** `crates/anvilml-core/tests/events_tests.rs`
**Context:** `WsEvent::JobImageReady` is the most data-rich variant with 6 fields (`job_id`, `artifact_hash`, `width`, `height`, `seed`, `steps`). Verifies all fields survive JSON serialisation.
**Tests:** A fully-populated `WsEvent::JobImageReady` serialises to JSON and deserialises back to an identical value. Each field is individually asserted for equality.
**Inputs:** `WsEvent::JobImageReady{job_id: 550e8400-e29b-41d4-a716-446655440000, artifact_hash: "a1b2c3d4e5f6", width: 1024, height: 768, seed: 42, steps: 30}`.
**Expected output:** All 6 fields match after `to_string` → `from_str` roundtrip.

## test_ws_event_tag_field_present (anvilml-core)

**File:** `crates/anvilml-core/tests/events_tests.rs`
**Context:** The `#[serde(tag = "type", rename_all = "snake_case")]` attribute on `WsEvent` causes each variant to serialise with a `"type"` key whose value is the snake_case variant name. This verifies the discriminator key is `"type"` (not `"_type"`).
**Tests:** Serialise `WsEvent::JobQueued` to JSON, parse as generic JSON value, and assert that `"type"` key exists with value `"job_queued"`.
**Inputs:** `WsEvent::JobQueued{job_id: 550e8400-e29b-41d4-a716-446655440000, queue_position: 1}`.
**Expected output:** `parsed["type"] == "job_queued"`.

## test_ws_event_all_variants_roundtrip (anvilml-core)

**File:** `crates/anvilml-core/tests/events_tests.rs`
**Context:** All 10 `WsEvent` enum variants must survive JSON roundtrip. This tests every variant in a single loop, ensuring no serde mapping bug in any variant.
**Tests:** Each of the 10 variants is constructed with minimal but non-default values, serialised to JSON, deserialised back, and asserted for equality.
**Inputs:** One instance of each variant: `JobQueued`, `JobStarted`, `JobProgress`, `JobImageReady`, `JobCompleted`, `JobFailed`, `JobCancelled`, `WorkerStatusChanged`, `SystemStats`, `ProvisioningProgress`.
**Expected output:** All 10 deserialised events equal their originals.

## test_ws_event_system_stats_roundtrip (anvilml-core)

**File:** `crates/anvilml-core/tests/events_tests.rs`
**Context:** `WsEvent::SystemStats` contains a `Vec<WorkerInfo>` — this tests that the enum correctly handles cross-type references and nested serialisation. `WorkerInfo` must implement `Serialize`/`Deserialize` for this to compile and pass.
**Tests:** A `WsEvent::SystemStats` with two `WorkerInfo` entries (one idle, one busy with a job) roundtrips through JSON. All nested fields of both workers are individually verified.
**Inputs:** `WsEvent::SystemStats{cpu_pct: 67.3, ram_used_mib: 16384, workers: [WorkerInfo{worker-0, idle}, WorkerInfo{worker-1, busy, job=550e8400-e29b-41d4-a716-446655440001}]}`.
**Expected output:** All fields including nested workers match after roundtrip.

## test_db_status_code (anvilml-core)

**File:** `crates/anvilml-core/tests/error_tests.rs`
**Context:** `AnvilError::Db` wraps a `sqlx::Error` and maps to HTTP 500 — database failures are server-side errors the client cannot fix.
**Tests:** `AnvilError::Db(SqlxError::PoolTimedOut).status_code()` returns `StatusCode::INTERNAL_SERVER_ERROR`.
**Inputs:** `AnvilError::Db(SqlxError::PoolTimedOut)`.
**Expected output:** `status_code() == StatusCode::INTERNAL_SERVER_ERROR`.

## test_io_status_code (anvilml-core)

**File:** `crates/anvilml-core/tests/error_tests.rs`
**Context:** `AnvilError::Io` wraps a `std::io::Error` and maps to HTTP 500 — I/O errors on server-owned files indicate a server-side problem.
**Tests:** `AnvilError::Io(std::io::Error::other("test")).status_code()` returns `StatusCode::INTERNAL_SERVER_ERROR`.
**Inputs:** `AnvilError::Io(std::io::Error::other("test io error"))`.
**Expected output:** `status_code() == StatusCode::INTERNAL_SERVER_ERROR`.

## test_serde_status_code (anvilml-core)

**File:** `crates/anvilml-core/tests/error_tests.rs`
**Context:** `AnvilError::Serde` is a string-wrapped serialization error mapping to HTTP 500 — serialization failures indicate a programming error.
**Tests:** `AnvilError::Serde("bad json".to_string()).status_code()` returns `StatusCode::INTERNAL_SERVER_ERROR`.
**Inputs:** `AnvilError::Serde("bad json".to_string())`.
**Expected output:** `status_code() == StatusCode::INTERNAL_SERVER_ERROR`.

## test_ipc_status_code (anvilml-core)

**File:** `crates/anvilml-core/tests/error_tests.rs`
**Context:** `AnvilError::Ipc` is a string-wrapped IPC error mapping to HTTP 500 — IPC failures with Python workers are server-side operational errors.
**Tests:** `AnvilError::Ipc("connection lost".to_string()).status_code()` returns `StatusCode::INTERNAL_SERVER_ERROR`.
**Inputs:** `AnvilError::Ipc("connection lost".to_string())`.
**Expected output:** `status_code() == StatusCode::INTERNAL_SERVER_ERROR`.

## test_payload_too_large_status_code (anvilml-core)

**File:** `crates/anvilml-core/tests/error_tests.rs`
**Context:** `AnvilError::PayloadTooLarge` maps to HTTP 413 — the only client-side error that is not 404 or 400.
**Tests:** `AnvilError::PayloadTooLarge("256MiB".to_string()).status_code()` returns `StatusCode::PAYLOAD_TOO_LARGE`.
**Inputs:** `AnvilError::PayloadTooLarge("256MiB".to_string())`.
**Expected output:** `status_code() == StatusCode::PAYLOAD_TOO_LARGE`.

## test_worker_not_found_status_code (anvilml-core)

**File:** `crates/anvilml-core/tests/error_tests.rs`
**Context:** `AnvilError::WorkerNotFound` maps to HTTP 404 — the worker resource does not exist.
**Tests:** `AnvilError::WorkerNotFound("worker-1".to_string()).status_code()` returns `StatusCode::NOT_FOUND`.
**Inputs:** `AnvilError::WorkerNotFound("worker-1".to_string())`.
**Expected output:** `status_code() == StatusCode::NOT_FOUND`.

## test_job_not_found_status_code (anvilml-core)

**File:** `crates/anvilml-core/tests/error_tests.rs`
**Context:** `AnvilError::JobNotFound` maps to HTTP 404 — the job resource does not exist.
**Tests:** `AnvilError::JobNotFound("job-abc".to_string()).status_code()` returns `StatusCode::NOT_FOUND`.
**Inputs:** `AnvilError::JobNotFound("job-abc".to_string())`.
**Expected output:** `status_code() == StatusCode::NOT_FOUND`.

## test_invalid_graph_status_code (anvilml-core)

**File:** `crates/anvilml-core/tests/error_tests.rs`
**Context:** `AnvilError::InvalidGraph` maps to HTTP 400 — the client submitted a graph with validation errors.
**Tests:** `AnvilError::InvalidGraph(vec!["missing node".to_string()]).status_code()` returns `StatusCode::BAD_REQUEST`.
**Inputs:** `AnvilError::InvalidGraph(vec!["missing node".to_string()])`.
**Expected output:** `status_code() == StatusCode::BAD_REQUEST`.

## test_cycle_detected_status_code (anvilml-core)

**File:** `crates/anvilml-core/tests/error_tests.rs`
**Context:** `AnvilError::CycleDetected` maps to HTTP 400 — the client submitted a graph with a cycle.
**Tests:** `AnvilError::CycleDetected(vec!["A→B→A".to_string()]).status_code()` returns `StatusCode::BAD_REQUEST`.
**Inputs:** `AnvilError::CycleDetected(vec!["A→B→A".to_string()])`.
**Expected output:** `status_code() == StatusCode::BAD_REQUEST`.

## test_model_not_found_status_code (anvilml-core)

**File:** `crates/anvilml-core/tests/error_tests.rs`
**Context:** `AnvilError::ModelNotFound` maps to HTTP 404 — the model resource does not exist in any configured directory.
**Tests:** `AnvilError::ModelNotFound("model-x".to_string()).status_code()` returns `StatusCode::NOT_FOUND`.
**Inputs:** `AnvilError::ModelNotFound("model-x".to_string())`.
**Expected output:** `status_code() == StatusCode::NOT_FOUND`.

## test_workers_unavailable_status_code (anvilml-core)

**File:** `crates/anvilml-core/tests/error_tests.rs`
**Context:** `AnvilError::WorkersUnavailable` maps to HTTP 503 — all workers are busy or dead, the service is temporarily unable to process the request.
**Tests:** `AnvilError::WorkersUnavailable("no idle".to_string()).status_code()` returns `StatusCode::SERVICE_UNAVAILABLE`.
**Inputs:** `AnvilError::WorkersUnavailable("no idle".to_string())`.
**Expected output:** `status_code() == StatusCode::SERVICE_UNAVAILABLE`.

## test_internal_status_code (anvilml-core)

**File:** `crates/anvilml-core/tests/error_tests.rs`
**Context:** `AnvilError::Internal` maps to HTTP 500 — unexpected internal failures indicate a bug in the server.
**Tests:** `AnvilError::Internal("panic caught".to_string()).status_code()` returns `StatusCode::INTERNAL_SERVER_ERROR`.
**Inputs:** `AnvilError::Internal("panic caught".to_string())`.
**Expected output:** `status_code() == StatusCode::INTERNAL_SERVER_ERROR`.

## test_toml_status_code (anvilml-core)

**File:** `crates/anvilml-core/tests/error_tests.rs`
**Context:** `AnvilError::Toml` maps to HTTP 400 — TOML deserialisation errors mean the config file is malformed.
**Tests:** `AnvilError::Toml(toml_err).status_code()` returns `StatusCode::BAD_REQUEST` where `toml_err` is created from deserializing invalid TOML.
**Inputs:** `AnvilError::Toml(toml::from_str::<toml::Value>("[invalid toml content {{{").unwrap_err())`.
**Expected output:** `status_code() == StatusCode::BAD_REQUEST`.

## test_env_var_status_code (anvilml-core)

**File:** `crates/anvilml-core/tests/error_tests.rs`
**Context:** `AnvilError::EnvVar` maps to HTTP 400 — invalid environment variable values mean the operator set a config variable to an unparseable value.
**Tests:** `AnvilError::EnvVar { name: "PORT", value: "abc" }.status_code()` returns `StatusCode::BAD_REQUEST`.
**Inputs:** `AnvilError::EnvVar { name: "PORT".to_string(), value: "abc".to_string() }`.
**Expected output:** `status_code() == StatusCode::BAD_REQUEST`.

## test_response_body_structure (anvilml-core)

**File:** `crates/anvilml-core/tests/error_tests.rs`
**Context:** The JSON response body produced by `IntoResponse` must contain three keys (`"error"`, `"message"`, `"request_id"`) with correct types and a valid v4 UUID for `request_id`.
**Tests:** Constructs the same body that `into_response()` would build, validates all three keys are present with correct types, and asserts `request_id` is a valid v4 UUID.
**Inputs:** `AnvilError::JobNotFound("x".to_string())` — used to verify error kind string and message format.
**Expected output:** All three keys present, all strings, `request_id` is a valid v4 UUID.

## test_unique_request_ids (anvilml-core)

**File:** `crates/anvilml-core/tests/error_tests.rs`
**Context:** Each call to `IntoResponse` must generate a fresh UUID — no caching of request_id across calls.
**Tests:** Generates 10 UUIDs via `uuid::Uuid::new_v4()` and asserts all are unique.
**Inputs:** None (uses `uuid::Uuid::new_v4()` directly).
**Expected output:** 10 unique UUID strings.

## test_from_sqlx_error (anvilml-core)

**File:** `crates/anvilml-core/tests/error_tests.rs`
**Context:** `From<sqlx::Error>` must correctly convert to `AnvilError::Db` via the `#[from]` attribute — required by downstream crates using `?` to propagate sqlx errors.
**Tests:** Converts `SqlxError::PoolTimedOut` into `AnvilError` and asserts the result is `AnvilError::Db(SqlxError::PoolTimedOut)`.
**Inputs:** `SqlxError::PoolTimedOut`.
**Expected output:** `AnvilError::Db(SqlxError::PoolTimedOut)`.

## test_system_env_returns_200_with_default_report (anvilml-server)

**File:** `crates/anvilml-server/tests/system_tests.rs`
**Context:** The `GET /v1/system/env` handler returns the default `EnvReport` stub via the production `build_router` path. Uses `Router::oneshot` to exercise the full handler pipeline without a live TCP listener.
**Tests:** Builds the router with a default `AppState`, sends a GET request to `/v1/system/env`, asserts HTTP 200, parses the JSON response, and verifies `preflight_ok` is `false` and `provisioning` is `"not_started"`.
**Inputs:** GET `/v1/system/env`, `AppState::new("test-version")`.
**Expected output:** HTTP 200 with JSON body `{"preflight_ok":false,"provisioning":"not_started",...}`.
**Acceptance command:** `cargo test -p anvilml-server --test system_tests -- --nocapture` exits 0.

## test_app_state_new (anvilml-server)

**File:** `crates/anvilml-server/tests/state_tests.rs`
**Context:** `AppState::new()` sets `start_time` to a recent instant and stores the version string correctly. No I/O, no subprocess, no network.
**Tests:** Constructs `AppState::new("0.1.0")` and verifies `version == "0.1.0"` and `start_time` is within one second of the construction call.
**Inputs:** `"0.1.0"`.
**Expected output:** `version == "0.1.0"` and elapsed time between `Instant::now()` calls is less than 1 second.

## test_app_state_clone (anvilml-server)

**File:** `crates/anvilml-server/tests/state_tests.rs`
**Context:** `AppState` derives `Clone` correctly — the cloned `version` field must match the original.
**Tests:** Clones an `AppState` and verifies `version` is identical. `Instant` does not compare equal across clones, so only the String field is checked.
**Inputs:** `AppState::new("0.1.0")`.
**Expected output:** `cloned.version == state.version`.

## test_app_state_version_from_env (anvilml-server)

**File:** `crates/anvilml-server/tests/state_tests.rs`
**Context:** `AppState::new()` accepts a `&'static str` from `CARGO_PKG_VERSION` via `impl Into<String>` and stores it correctly.
**Tests:** Constructs `AppState` using `env!("CARGO_PKG_VERSION")` and asserts the stored version matches.
**Inputs:** `env!("CARGO_PKG_VERSION")` (a compile-time constant).
**Expected output:** `state.version == crate_version`.

## test_health_returns_200_with_status_key (anvilml-server)

**File:** `crates/anvilml-server/tests/health_tests.rs`
**Context:** The health handler returns HTTP 200 with a JSON body containing a `status` key set to `"ok"`. Exercises the production `build_router` path via `Router::oneshot`.
**Tests:** Builds the router with `AppState::new("test-version")`, sends GET `/health`, asserts HTTP 200, parses JSON, and verifies `status == "ok"`.
**Inputs:** GET `/health`, `AppState::new("test-version")`.
**Expected output:** HTTP 200 with JSON body containing `"status":"ok"`.
**Acceptance command:** `cargo test -p anvilml-server --test health_tests -- --nocapture` exits 0.

## test_cpu_detector_detect_returns_one_device (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/cpu_tests.rs`
**Context:** `CpuDetector` implements `DeviceDetector` and uses `sysinfo` to read host-level information. All tests in this file are annotated with `#[serial]` because sysinfo reads process-global system state.
**Tests:** Creates a `CpuDetector`, calls `detect()`, and verifies the returned vec has exactly one element with `device_type == DeviceType::Cpu` and `index == 0`.
**Inputs:** None (uses `CpuDetector::new()`).
**Expected output:** `devices.len() == 1`, `devices[0].device_type == DeviceType::Cpu`, `devices[0].index == 0`.

## test_cpu_detector_refresh_vram_returns_zero (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/cpu_tests.rs`
**Context:** CPUs have no dedicated video memory, so `refresh_vram` always returns `(0, 0)`.
**Tests:** Creates a `CpuDetector`, calls `refresh_vram(0)`, and verifies both total and free VRAM are zero.
**Inputs:** `index = 0`.
**Expected output:** `(total, free) == (0, 0)`.

## test_cpu_detector_is_send_sync (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/cpu_tests.rs`
**Context:** The `DeviceDetector` trait requires `Send + Sync`. This is a compile-time check that verifies the impl satisfies the trait bounds.
**Tests:** Defines a generic function `fn assert_send_sync<T: Send + Sync>() {}` and calls it with `CpuDetector`. If `CpuDetector` does not implement `Send + Sync`, this will not compile.
**Inputs:** None (zero-cost compile-time assertion).
**Expected output:** Compiles successfully.

## test_vulkan_detector_new (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/vulkan_tests.rs`
**Context:** `VulkanDetector::new()` constructs a zero-sized unit struct with no allocation, no I/O, and no system calls.
**Tests:** Constructs `VulkanDetector::new()` and verifies construction succeeds without panic.
**Inputs:** None (zero-cost unit struct construction).
**Expected output:** `VulkanDetector` value constructed successfully.

## test_vulkan_detector_detect_returns_empty_or_devices (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/vulkan_tests.rs`
**Context:** `VulkanDetector::detect()` loads the Vulkan loader at runtime and enumerates physical GPUs. On systems without Vulkan (CI, WSL2), it returns `Ok(vec![])`. On systems with Vulkan GPUs, it returns detected devices. The key invariant is that the method never panics or returns `Err`.
**Tests:** Calls `detect()` and asserts the result is `Ok`. The device list may be empty (no Vulkan loader) or populated (Vulkan GPUs present).
**Inputs:** None (uses `VulkanDetector::new()`).
**Expected output:** `Ok(vec![])` on systems without Vulkan; `Ok([devices...])` on systems with Vulkan GPUs. Never `Err`.

## test_vulkan_detector_refresh_vram_returns_zero (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/vulkan_tests.rs`
**Context:** Live VRAM refresh requires a Vulkan device context (queue, command buffer) which this task does not create. Returns `(0, 0)` as a best-effort placeholder.
**Tests:** Calls `refresh_vram(0)` and verifies both total and free VRAM are zero.
**Inputs:** `index = 0`.
**Expected output:** `(total, free) == (0, 0)`.

## test_vulkan_detector_is_send_sync (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/vulkan_tests.rs`
**Context:** The `DeviceDetector` trait requires `Send + Sync`. This is a compile-time check that verifies the impl satisfies the trait bounds.
**Tests:** Defines a generic function `fn assert_send_sync<T: Send + Sync>() {}` and calls it with `VulkanDetector`. If `VulkanDetector` does not implement `Send + Sync`, this will not compile.
**Inputs:** None (zero-cost compile-time assertion).
**Expected output:** Compiles successfully.

## test_dxgi_detector_new (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/dxgi_sysfs_tests.rs`
**Context:** `DxgiDetector::new()` constructs a zero-sized unit struct on Windows. This is a zero-cost check — no allocation, no I/O, no system calls.
**Tests:** Constructs `DxgiDetector::new()` and verifies construction succeeds without panic.
**Inputs:** None (zero-cost unit struct construction).
**Expected output:** `DxgiDetector` value constructed successfully.
**Platform:** Windows only (`#[cfg(windows)]`).

## test_dxgi_detector_default (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/dxgi_sysfs_tests.rs`
**Context:** `DxgiDetector::default()` constructs a zero-sized unit struct via the `Default` trait.
**Tests:** Constructs `DxgiDetector::default()` and verifies construction succeeds.
**Inputs:** None (zero-cost unit struct construction).
**Expected output:** `DxgiDetector` value constructed successfully.
**Platform:** Windows only (`#[cfg(windows)]`).

## test_dxgi_detect_no_panic (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/dxgi_sysfs_tests.rs`
**Context:** `DxgiDetector::detect()` initialises COM, creates a DXGI factory, and enumerates adapters. On Windows systems without GPUs, it returns `Ok(vec![])`. On systems with GPUs, it returns detected devices. The key invariant is that the method never panics or returns `Err`.
**Tests:** Calls `detect()` and asserts the result is `Ok`.
**Inputs:** None (uses `DxgiDetector::new()`).
**Expected output:** `Ok(vec![])` on systems without GPUs; `Ok([devices...])` on systems with GPUs. Never `Err`.
**Platform:** Windows only (`#[cfg(windows)]`).

## test_dxgi_detector_is_send_sync (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/dxgi_sysfs_tests.rs`
**Context:** The `DeviceDetector` trait requires `Send + Sync`. This is a compile-time check that verifies the impl satisfies the trait bounds.
**Tests:** Defines a generic function `fn assert_send_sync<T: Send + Sync>() {}` and calls it with `DxgiDetector`.
**Inputs:** None (zero-cost compile-time assertion).
**Expected output:** Compiles successfully.
**Platform:** Windows only (`#[cfg(windows)]`).

## test_sysfs_detector_new (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/dxgi_sysfs_tests.rs`
**Context:** `SysfsPciDetector::new()` constructs a zero-sized unit struct on Unix. This is a zero-cost check — no allocation, no I/O, no system calls.
**Tests:** Constructs `SysfsPciDetector::new()` and verifies construction succeeds without panic.
**Inputs:** None (zero-cost unit struct construction).
**Expected output:** `SysfsPciDetector` value constructed successfully.
**Platform:** Unix only (`#[cfg(unix)]`).

## test_sysfs_detector_default (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/dxgi_sysfs_tests.rs`
**Context:** `SysfsPciDetector::default()` constructs a zero-sized unit struct via the `Default` trait.
**Tests:** Constructs `SysfsPciDetector::default()` and verifies construction succeeds.
**Inputs:** None (zero-cost unit struct construction).
**Expected output:** `SysfsPciDetector` value constructed successfully.
**Platform:** Unix only (`#[cfg(unix)]`).

## test_sysfs_detect_no_panic (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/dxgi_sysfs_tests.rs`
**Context:** `SysfsPciDetector::detect()` walks `/sys/bus/pci/devices/` and reads vendor/device files. On systems with PCI GPUs, it returns detected devices. On systems without PCI (WSL2, some VMs), it returns `Ok(vec![])`. The key invariant is that the method never panics or returns `Err`.
**Tests:** Calls `detect()` and asserts the result is `Ok`.
**Inputs:** None (uses `SysfsPciDetector::new()`).
**Expected output:** `Ok(vec![])` on systems without PCI; `Ok([devices...])` on systems with PCI GPUs. Never `Err`.
**Platform:** Unix only (`#[cfg(unix)]`).

## test_sysfs_refresh_vram_returns_zero (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/dxgi_sysfs_tests.rs`
**Context:** Sysfs doesn't provide live VRAM data — VRAM is queried via NVML on NVIDIA systems. `refresh_vram` always returns `(0, 0)`.
**Tests:** Calls `refresh_vram(0)` and verifies both total and free VRAM are zero.
**Inputs:** `index = 0`.
**Expected output:** `(total, free) == (0, 0)`.
**Platform:** Unix only (`#[cfg(unix)]`).

## test_sysfs_detect_vendor_mapping (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/dxgi_sysfs_tests.rs`
**Context:** The sysfs detector maps PCI vendor IDs to `DeviceType` variants: `0x10de` → `Cuda`, `0x1002` → `Rocm`. This test verifies the mapping is correct by checking the `device_type` field of any detected PCI devices.
**Tests:** Calls `detect()`, iterates detected devices, and asserts that NVIDIA GPUs have `DeviceType::Cuda` and AMD GPUs have `DeviceType::Rocm`.
**Inputs:** None (uses `SysfsPciDetector::new()`).
**Expected output:** All detected NVIDIA GPUs (vendor 0x10de) have `device_type == Cuda`; all detected AMD GPUs (vendor 0x1002) have `device_type == Rocm`.
**Platform:** Unix only (`#[cfg(unix)]`).

## test_sysfs_detector_is_send_sync (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/dxgi_sysfs_tests.rs`
**Context:** The `DeviceDetector` trait requires `Send + Sync`. This is a compile-time check that verifies the impl satisfies the trait bounds.
**Tests:** Defines a generic function `fn assert_send_sync<T: Send + Sync>() {}` and calls it with `SysfsPciDetector`.
**Inputs:** None (zero-cost compile-time assertion).
**Expected output:** Compiles successfully.
**Platform:** Unix only (`#[cfg(unix)]`).

## test_nvml_detector_new (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/dxgi_sysfs_tests.rs`
**Context:** `NvmlDetector::new()` constructs a zero-sized unit struct on Unix. This is a zero-cost check — no allocation, no I/O, no system calls.
**Tests:** Constructs `NvmlDetector::new()` and verifies construction succeeds without panic.
**Inputs:** None (zero-cost unit struct construction).
**Expected output:** `NvmlDetector` value constructed successfully.
**Platform:** Unix only (`#[cfg(unix)]`).

## test_nvml_detector_default (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/dxgi_sysfs_tests.rs`
**Context:** `NvmlDetector::default()` constructs a zero-sized unit struct via the `Default` trait.
**Tests:** Constructs `NvmlDetector::default()` and verifies construction succeeds.
**Inputs:** None (zero-cost unit struct construction).
**Expected output:** `NvmlDetector` value constructed successfully.
**Platform:** Unix only (`#[cfg(unix)]`).

## test_nvml_detect_returns_empty (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/dxgi_sysfs_tests.rs`
**Context:** NVML is a VRAM refresh supplement, not a device enumerator. `detect()` always returns an empty list.
**Tests:** Calls `detect()` and asserts the returned list is empty.
**Inputs:** None (uses `NvmlDetector::new()`).
**Expected output:** `Ok(vec![])` — always empty.
**Platform:** Unix only (`#[cfg(unix)]`).

## test_nvml_refresh_vram_no_library (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/dxgi_sysfs_tests.rs`
**Context:** On systems without `libnvidia-ml.so` (non-NVIDIA systems), `refresh_vram()` returns `(0, 0)` gracefully. On NVIDIA systems, it returns actual VRAM values. The key invariant is that the method never returns an error.
**Tests:** Calls `refresh_vram(0)` and asserts the result is `Ok` with valid VRAM values.
**Inputs:** `index = 0`.
**Expected output:** `(total, free) == (0, 0)` on non-NVIDIA systems; actual VRAM values on NVIDIA systems. Never `Err`.
**Platform:** Unix only (`#[cfg(unix)]`).

## test_nvml_refresh_vram_no_panic (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/dxgi_sysfs_tests.rs`
**Context:** `NvmlDetector::refresh_vram()` must never panic regardless of system state.
**Tests:** Calls `refresh_vram(0)` and verifies no panic occurs.
**Inputs:** `index = 0`.
**Expected output:** No panic, method returns `Ok`.
**Platform:** Unix only (`#[cfg(unix)]`).

## test_nvml_detector_is_send_sync (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/dxgi_sysfs_tests.rs`
**Context:** The `DeviceDetector` trait requires `Send + Sync`. This is a compile-time check that verifies the impl satisfies the trait bounds.
**Tests:** Defines a generic function `fn assert_send_sync<T: Send + Sync>() {}` and calls it with `NvmlDetector`.
**Inputs:** None (zero-cost compile-time assertion).
**Expected output:** Compiles successfully.
**Platform:** Unix only (`#[cfg(unix)]`).
