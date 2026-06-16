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

## test_system_returns_200_with_hardware_info (anvilml-server)

**File:** `crates/anvilml-server/tests/system_tests.rs`
**Context:** The `GET /v1/system` handler returns the full `HardwareInfo` snapshot from `AppState.hardware` via the production `build_router` path. Uses `Router::oneshot` to exercise the full handler pipeline without a live TCP listener. `AppState` is constructed with `new_with_hardware` which accepts a pre-wrapped `Arc<RwLock<HardwareInfo>>`.
**Tests:** Builds `AppState` with `new_with_hardware` using a default `HardwareInfo`, sends a GET request to `/v1/system`, asserts HTTP 200, parses the JSON response, verifies `gpus` is a JSON array with at least one entry.
**Inputs:** GET `/v1/system`, `AppState::new_with_hardware("test-version", Arc<RwLock<HardwareInfo::default()>>)`.
**Expected output:** HTTP 200 with JSON body containing `gpus` array of length >= 1.
**Acceptance command:** `cargo test -p anvilml-server --test system_tests -- test_system_returns_200_with_hardware_info` exits 0.

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

## test_resolve_nvidia_ampere (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/device_db_tests.rs`
**Context:** `resolve_caps_from_row` looks up a known NVIDIA A100 (Ampere, PCI ID 0x10de/0x2204) in `DEVICE_DB` and verifies that `arch`, `fp8`, `flash_attention`, and `capabilities_source` are correctly populated.
**Tests:** Constructs a `GpuDevice` with `pci_vendor_id=0x10de`, `pci_device_id=0x2204`, calls `resolve_caps_from_row`, and asserts `arch=Some("Ampere")`, `caps.fp8=true`, `caps.flash_attention=true`, `capabilities_source=DeviceTable`.
**Inputs:** `GpuDevice{pci_vendor_id: 0x10de, pci_device_id: 0x2204, caps: InferenceCaps::default()}`.
**Expected output:** All capability fields correctly populated from DEVICE_DB entry.
**Acceptance command:** `cargo test -p anvilml-hardware --test device_db_tests test_resolve_nvidia_ampere` exits 0.

## test_resolve_amd_rdna3 (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/device_db_tests.rs`
**Context:** `resolve_caps_from_row` looks up a known AMD RX 7900 XTX (RDNA3, PCI ID 0x1002/0x74AF) in `DEVICE_DB` and verifies correct capability population.
**Tests:** Constructs a `GpuDevice` with `pci_vendor_id=0x1002`, `pci_device_id=0x74AF`, calls `resolve_caps_from_row`, and asserts `arch=Some("RDNA3")`, `caps.fp8=false`, `caps.flash_attention=true`.
**Inputs:** `GpuDevice{pci_vendor_id: 0x1002, pci_device_id: 0x74AF, caps: InferenceCaps::default()}`.
**Expected output:** `arch=Some("RDNA3")`, `fp8=false`, `flash_attention=true`.
**Acceptance command:** `cargo test -p anvilml-hardware --test device_db_tests test_resolve_amd_rdna3` exits 0.

## test_resolve_unknown_device (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/device_db_tests.rs`
**Context:** `resolve_caps_from_row` with an unknown PCI ID pair (0x9999/0x9999) is a no-op — it leaves `arch=None`, `caps` unchanged, and `capabilities_source` unchanged.
**Tests:** Constructs a `GpuDevice` with fabricated PCI IDs not in `DEVICE_DB`, calls `resolve_caps_from_row`, and asserts all fields remain at their initial values.
**Inputs:** `GpuDevice{pci_vendor_id: 0x9999, pci_device_id: 0x9999, caps: InferenceCaps::default(), capabilities_source: Fallback}`.
**Expected output:** `arch=None`, `caps` unchanged, `capabilities_source=Fallback`.
**Acceptance command:** `cargo test -p anvilml-hardware --test device_db_tests test_resolve_unknown_device` exits 0.

## test_resolve_cpu_fallback (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/device_db_tests.rs`
**Context:** CPU devices synthesised by `CpuDetector` have PCI IDs of zero (0x0/0x0). These must not match any entry in `DEVICE_DB`.
**Tests:** Constructs a `GpuDevice` with `pci_vendor_id=0`, `pci_device_id=0`, calls `resolve_caps_from_row`, and asserts `arch=None` and `caps` unchanged.
**Inputs:** `GpuDevice{pci_vendor_id: 0, pci_device_id: 0}`.
**Expected output:** No match in DEVICE_DB, all fields unchanged.
**Acceptance command:** `cargo test -p anvilml-hardware --test device_db_tests test_resolve_cpu_fallback` exits 0.

## test_resolve_vram_untouched (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/device_db_tests.rs`
**Context:** `resolve_caps_from_row` must never modify VRAM fields — they are set by the detector and must be preserved.
**Tests:** Constructs a `GpuDevice` with known RTX 4090 PCI IDs and specific VRAM values (24576 total, 20000 free), calls `resolve_caps_from_row`, and asserts VRAM values are unchanged.
**Inputs:** `GpuDevice{pci_vendor_id: 0x10de, pci_device_id: 0x2488, vram_total_mib: 24576, vram_free_mib: 20000}`.
**Expected output:** `vram_total_mib=24576`, `vram_free_mib=20000` after resolve.
**Acceptance command:** `cargo test -p anvilml-hardware --test device_db_tests test_resolve_vram_untouched` exits 0.

## test_resolve_name_overwrite (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/device_db_tests.rs`
**Context:** Resolving a known device overwrites the `name` field with the canonical name from `DEVICE_DB`.
**Tests:** Constructs a `GpuDevice` with `name="Unknown GPU"` and RTX 4090 PCI IDs, calls `resolve_caps_from_row`, and asserts the name changes to `"NVIDIA RTX 4090"`.
**Inputs:** `GpuDevice{name: "Unknown GPU", pci_vendor_id: 0x10de, pci_device_id: 0x2488}`.
**Expected output:** `name == "NVIDIA RTX 4090"` after resolve.
**Acceptance command:** `cargo test -p anvilml-hardware --test device_db_tests test_resolve_name_overwrite` exits 0.

## test_device_db_non_empty (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/device_db_tests.rs`
**Context:** `DEVICE_DB` is a compile-time constant table that must contain at least 12 curated entries covering NVIDIA, AMD, and Intel GPUs.
**Tests:** Asserts `DEVICE_DB.len() >= 12`.
**Inputs:** None (uses the `DEVICE_DB` constant directly).
**Expected output:** `DEVICE_DB.len() >= 12`.
**Acceptance command:** `cargo test -p anvilml-hardware --test device_db_tests test_device_db_non_empty` exits 0.

## test_mock_detect_cuda (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/mock_tests.rs`
**Context:** `MockDetector` synthesises a single `GpuDevice` from environment variables. This test verifies the CUDA path: setting `ANVILML_MOCK_DEVICE_TYPE=cuda` produces a device with `DeviceType::Cuda`, correct VRAM, and `EnumerationSource::Mock`. All tests in this file are annotated with `#[serial]` because they mutate process-global env vars.
**Tests:** Creates `MockDetector::new()`, sets env vars `ANVILML_MOCK_DEVICE_TYPE=cuda`, `ANVILML_MOCK_VRAM_MIB=16384`, `ANVILML_MOCK_DEVICE_NAME=Mock CUDA`, calls `detect()`, and asserts one device with correct fields.
**Inputs:** Env vars: `ANVILML_MOCK_DEVICE_TYPE=cuda`, `ANVILML_MOCK_VRAM_MIB=16384`, `ANVILML_MOCK_DEVICE_NAME=Mock CUDA`.
**Expected output:** `devices.len()==1`, `devices[0].device_type==Cuda`, `vram_total_mib==16384`, `enumeration_source==Mock`, `name=="Mock CUDA"`.
**Acceptance command:** `cargo test -p anvilml-hardware --features mock-hardware mock_tests::test_mock_detect_cuda` exits 0.

## test_mock_detect_rocm (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/mock_tests.rs`
**Context:** `MockDetector` with `ANVILML_MOCK_DEVICE_TYPE=rocm` produces a ROCm device. Verifies the ROCm mapping path.
**Tests:** Sets env vars `ANVILML_MOCK_DEVICE_TYPE=rocm`, `ANVILML_MOCK_VRAM_MIB=8192`, `ANVILML_MOCK_DEVICE_NAME=Mock ROCm`, calls `detect()`, and asserts one ROCm device.
**Inputs:** Env vars: `ANVILML_MOCK_DEVICE_TYPE=rocm`, `ANVILML_MOCK_VRAM_MIB=8192`, `ANVILML_MOCK_DEVICE_NAME=Mock ROCm`.
**Expected output:** `devices.len()==1`, `devices[0].device_type==Rocm`, `vram_total_mib==8192`.
**Acceptance command:** `cargo test -p anvilml-hardware --features mock-hardware mock_tests::test_mock_detect_rocm` exits 0.

## test_mock_detect_cpu (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/mock_tests.rs`
**Context:** `MockDetector` with `ANVILML_MOCK_DEVICE_TYPE=cpu` produces a CPU-type mock device. Verifies the CPU mapping path.
**Tests:** Sets env vars `ANVILML_MOCK_DEVICE_TYPE=cpu`, `ANVILML_MOCK_VRAM_MIB=0`, `ANVILML_MOCK_DEVICE_NAME=Mock CPU`, calls `detect()`, and asserts one CPU device.
**Inputs:** Env vars: `ANVILML_MOCK_DEVICE_TYPE=cpu`, `ANVILML_MOCK_VRAM_MIB=0`, `ANVILML_MOCK_DEVICE_NAME=Mock CPU`.
**Expected output:** `devices.len()==1`, `devices[0].device_type==Cpu`, `vram_total_mib==0`.
**Acceptance command:** `cargo test -p anvilml-hardware --features mock-hardware mock_tests::test_mock_detect_cpu` exits 0.

## test_mock_detect_invalid_type (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/mock_tests.rs`
**Context:** `MockDetector` with an invalid device type string returns an empty list (graceful fallback, no error). This verifies the error-handling path.
**Tests:** Sets env var `ANVILML_MOCK_DEVICE_TYPE=invalid`, calls `detect()`, and asserts the returned list is empty.
**Inputs:** Env var: `ANVILML_MOCK_DEVICE_TYPE=invalid`.
**Expected output:** `devices.is_empty()==true` — empty list, no error.
**Acceptance command:** `cargo test -p anvilml-hardware --features mock-hardware mock_tests::test_mock_detect_invalid_type` exits 0.

## test_detect_all_devices_mock_cuda (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/mock_tests.rs`
**Context:** Full pipeline: `detect_all_devices` with mock-hardware + cuda env var produces one CUDA GPU and one CPU device. Verifies the complete detection chain from mock through CPU fallback.
**Tests:** Sets env vars for mock CUDA, creates `ServerConfig::default()`, connects an in-memory SQLite pool, calls `detect_all_devices()`, and asserts the result has at least one GPU (CUDA) and one CPU, with correct host info.
**Inputs:** Env: `ANVILML_MOCK_DEVICE_TYPE=cuda`, `ANVILML_MOCK_VRAM_MIB=16384`, `ANVILML_MOCK_DEVICE_NAME=Mock CUDA`.
**Expected output:** `HardwareInfo` with `gpus.len() >= 2` (1 CUDA GPU + 1 CPU), `host.os` non-empty, `host.cpu` non-empty, `host.ram_total_mib > 0`.
**Acceptance command:** `cargo test -p anvilml-hardware --features mock-hardware mock_tests::test_detect_all_devices_mock_cuda` exits 0.

## test_detect_all_devices_hardware_override (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/mock_tests.rs`
**Context:** Hardware override takes priority over mock detector. When `ServerConfig.hardware_override` is set, the function returns the override device instead of attempting mock detection.
**Tests:** Sets `ANVILML_MOCK_DEVICE_TYPE=cuda` (but override should take priority), creates `ServerConfig` with `hardware_override: Some(Rocm, 32768 MiB)`, calls `detect_all_devices()`, and asserts the result has one ROCm override GPU + one CPU.
**Inputs:** Env: `ANVILML_MOCK_DEVICE_TYPE=cuda`. Config: `hardware_override = { device_type: "rocm", vram_total_mib: 32768 }`.
**Expected output:** `gpus.len()==2` (ROCm override + CPU), override device has `device_type==Rocm`, `vram_total_mib==32768`, `enumeration_source==Override`.
**Acceptance command:** `cargo test -p anvilml-hardware --features mock-hardware mock_tests::test_detect_all_devices_hardware_override` exits 0.

## test_detect_all_devices_cpu_fallback (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/mock_tests.rs`
**Context:** CPU device is always present even when GPU detection returns empty. When mock returns empty (invalid type), the CPU fallback still produces one device.
**Tests:** Sets `ANVILML_MOCK_DEVICE_TYPE=invalid` (mock returns empty), calls `detect_all_devices()`, and asserts at least one CPU device is present.
**Inputs:** Env: `ANVILML_MOCK_DEVICE_TYPE=invalid`.
**Expected output:** At least one `GpuDevice` with `device_type==Cpu`.
**Acceptance command:** `cargo test -p anvilml-hardware --features mock-hardware mock_tests::test_detect_all_devices_cpu_fallback` exits 0.

## test_detect_all_devices_inference_caps_union (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/mock_tests.rs`
**Context:** `inference_caps` is the union of all GPU caps. With mock devices (PCI IDs = 0), no device table match occurs, so caps remain at defaults. The test verifies the union logic produces a valid `InferenceCaps` struct.
**Tests:** Sets mock CUDA, calls `detect_all_devices()`, and asserts `inference_caps` is a well-formed struct (all fields are valid bools).
**Inputs:** Env: `ANVILML_MOCK_DEVICE_TYPE=cuda`, `ANVILML_MOCK_VRAM_MIB=8192`.
**Expected output:** `inference_caps` has valid bool fields (all `false` for mock devices with PCI ID 0).
**Acceptance command:** `cargo test -p anvilml-hardware --features mock-hardware mock_tests::test_detect_all_devices_inference_caps_union` exits 0.

## test_detect_all_devices_returns_ok (anvilml-hardware)

**File:** `crates/anvilml-hardware/tests/mock_tests.rs`
**Context:** `detect_all_devices` always returns `Ok` (never `Err`) under the mock-hardware feature. Detection failures are treated as "no device detected" rather than hard errors.
**Tests:** Calls `detect_all_devices()` with default config and in-memory pool, asserts the result is `Ok`.
**Inputs:** `ServerConfig::default()`, in-memory SQLite pool.
**Expected output:** `Result::Ok(HardwareInfo)`.
**Acceptance command:** `cargo test -p anvilml-hardware --features mock-hardware mock_tests::test_detect_all_devices_returns_ok` exits 0.

## test_open_creates_file (anvilml-registry)

**File:** `crates/anvilml-registry/tests/db_tests.rs`
**Context:** `open()` creates a file-backed SQLite database at the given path, enables WAL mode, runs all migrations, and resets ghost jobs. Uses a unique temp directory for isolation.
**Tests:** Calls `open()` with a temp dir path, verifies the DB file is created on disk, queries `sqlite_master` and asserts all five tables (jobs, models, artifacts, seed_history, device_capabilities) exist.
**Inputs:** Path to a unique temp directory (via `tempfile::tempdir()`).
**Expected output:** DB file exists on disk, `sqlite_master` contains exactly 5 tables matching expected names.
**Acceptance command:** `cargo test -p anvilml-registry --features mock-hardware -- db_tests::test_open_creates_file` exits 0.

## test_open_wal_mode (anvilml-registry)

**File:** `crates/anvilml-registry/tests/db_tests.rs`
**Context:** `open()` enables WAL (Write-Ahead Logging) journal mode via `SqliteConnectOptions::journal_mode(Wal)`. WAL mode provides better concurrent read performance and prevents "database is locked" errors.
**Tests:** Calls `open()` with a temp dir path, queries `PRAGMA journal_mode`, and asserts the result is `"wal"`.
**Inputs:** Path to a unique temp directory (via `tempfile::tempdir()`).
**Expected output:** `PRAGMA journal_mode` returns `"wal"`.
**Acceptance command:** `cargo test -p anvilml-registry --features mock-hardware -- db_tests::test_open_wal_mode` exits 0.

## test_open_in_memory (anvilml-registry)

**File:** `crates/anvilml-registry/tests/db_tests.rs`
**Context:** `open_in_memory()` creates a transient in-memory SQLite pool that is discarded when the pool is dropped. Runs the same migrations and ghost-job reset as `open()`.
**Tests:** Calls `open_in_memory()`, queries `sqlite_master`, and asserts all five tables exist.
**Inputs:** None (uses `sqlite::memory:` URL).
**Expected output:** `sqlite_master` contains exactly 5 tables matching expected names.
**Acceptance command:** `cargo test -p anvilml-registry --features mock-hardware -- db_tests::test_open_in_memory` exits 0.

## test_ghost_job_reset (anvilml-registry)

**File:** `crates/anvilml-registry/tests/db_tests.rs`
**Context:** Ghost-job reset targets jobs left in `Queued` or `Running` state from an unclean server shutdown. Sets them to `Failed` with `error = 'server_restart'` so the scheduler can re-queue or discard them. Uses `open_in_memory()` for a clean in-memory pool and executes the ghost-job reset SQL directly on the same connection (simulating what `open()` does after migrations). Each test uses its own in-memory pool — no shared connections.
**Tests:** Opens an in-memory pool, inserts a job with status `'Queued'`, executes the ghost-job reset SQL (`UPDATE jobs SET status = 'Failed', error = 'server_restart' WHERE status IN ('Queued', 'Running')`), then queries the job and verifies status changed to `'Failed'` with error `'server_restart'`.
**Inputs:** In-memory pool with a manually inserted job row (status='Queued').
**Expected output:** Job status is `'Failed'`, error is `'server_restart'`.
**Acceptance command:** `cargo test -p anvilml-registry --features mock-hardware -- db_tests::test_ghost_job_reset` exits 0.

## test_ghost_job_noop (anvilml-registry)

**File:** `crates/anvilml-registry/tests/db_tests.rs`
**Context:** Ghost-job reset only targets `Queued` and `Running` statuses. Jobs with `Completed` or `Failed` status must be left unchanged. Uses `open_in_memory()` for a clean in-memory pool and executes the ghost-job reset SQL directly on the same connection (simulating what `open()` does after migrations). Each test uses its own in-memory pool — no shared connections.
**Tests:** Opens an in-memory pool, inserts jobs with status `'Completed'` and `'Failed'`, executes the ghost-job reset SQL (`UPDATE jobs SET status = 'Failed', error = 'server_restart' WHERE status IN ('Queued', 'Running')`), then queries both jobs and verifies they are unchanged.
**Inputs:** In-memory pool with two manually inserted job rows (status='Completed', status='Failed').
**Expected output:** Completed job remains `status='Completed'`, Failed job remains `status='Failed'` with original error message.
**Acceptance command:** `cargo test -p anvilml-registry --features mock-hardware -- db_tests::test_ghost_job_noop` exits 0.

## test_seed_loader_applies_new_seed (anvilml-registry)

**File:** `crates/anvilml-registry/tests/seed_loader_tests.rs`
**Context:** The SHA256-gated seed loader discovers `.sql` files in a directory, computes SHA256 of each file, and either skips (up-to-date) or executes + records (new/changed). This test verifies the apply path: first run executes the seed SQL and records it in `seed_history`. Each test uses its own `open_in_memory()` pool and a unique temp directory for complete isolation.
**Tests:** Creates a temp directory with a `.sql` seed file containing 3 `INSERT OR IGNORE INTO device_capabilities` rows, calls `run()`, verifies `seed_history` has exactly 1 row, `device_capabilities` has 3 rows, the stored SHA256 matches the computed SHA256 of the seed content, and `applied_at` is a valid RFC3339 timestamp.
**Inputs:** In-memory pool, temp directory with one `.sql` file (3 INSERT statements).
**Expected output:** `seed_history` has 1 row, `device_capabilities` has 3 rows, SHA256 matches, `applied_at` parses as RFC3339.
**Acceptance command:** `cargo test -p anvilml-registry --features mock-hardware -- seed_loader_tests::test_seed_loader_applies_new_seed` exits 0.

## test_seed_loader_skips_up_to_date (anvilml-registry)

**File:** `crates/anvilml-registry/tests/seed_loader_tests.rs`
**Context:** The SHA256-gated seed loader skips seed files whose content hash matches the stored hash. This test verifies the skip path: second run on the same directory should not create duplicate entries in `seed_history` and should not re-execute the seed SQL. Each test uses its own `open_in_memory()` pool and a unique temp directory for complete isolation.
**Tests:** Creates a temp directory with a `.sql` seed file, runs `run()` twice, verifies `seed_history` still has exactly 1 row after both runs, and `device_capabilities` has exactly 1 row (the seed was not re-executed on the second run).
**Inputs:** In-memory pool, same temp directory with one `.sql` file, two sequential `run()` calls.
**Expected output:** `seed_history` has 1 row (no duplicate), `device_capabilities` has 1 row (seed skipped on second run).
**Acceptance command:** `cargo test -p anvilml-registry --features mock-hardware -- seed_loader_tests::test_seed_loader_skips_up_to_date` exits 0.

## test_infer_kind_diffusion (anvilml-registry)

**File:** `crates/anvilml-registry/tests/scanner_tests.rs`
**Context:** `ModelScanner::infer_kind()` maps directory names to `ModelKind` variants via case-insensitive matching. This test verifies the simplest mapping: `"diffusion"` → `ModelKind::Diffusion`.
**Tests:** Constructs `ModelScanner`, calls `infer_kind("diffusion")`, and asserts the result is `ModelKind::Diffusion`.
**Inputs:** `"diffusion"`.
**Expected output:** `ModelKind::Diffusion`.
**Acceptance command:** `cargo test -p anvilml-registry --test scanner_tests test_infer_kind_diffusion` exits 0.

## test_infer_kind_text_encoder (anvilml-registry)

**File:** `crates/anvilml-registry/tests/scanner_tests.rs`
**Context:** `ModelScanner::infer_kind()` maps both `"text_encoders"` and `"clip"` directory names to `ModelKind::TextEncoder` (the match arm uses `|` for multiple patterns). This test verifies both aliases.
**Tests:** Constructs `ModelScanner`, calls `infer_kind("text_encoders")` and `infer_kind("clip")`, and asserts both return `ModelKind::TextEncoder`.
**Inputs:** `"text_encoders"`, `"clip"`.
**Expected output:** Both calls return `ModelKind::TextEncoder`.
**Acceptance command:** `cargo test -p anvilml-registry --test scanner_tests test_infer_kind_text_encoder` exits 0.

## test_infer_dtype_fp8_before_fp16 (anvilml-registry)

**File:** `crates/anvilml-registry/tests/scanner_tests.rs`
**Context:** `ModelScanner::infer_dtype()` performs case-insensitive substring matching on filenames. The check order is critical: `fp8` must be checked before `fp16` to correctly handle filenames containing both substrings (e.g. `"model_fp16_fp8.safetensors"`).
**Tests:** Constructs `ModelScanner`, calls `infer_dtype()` with filenames containing `"fp16_fp8"`, `"fp16"`, `"bf16"`, `"fp32"`, and no precision indicator. Asserts `Fp8` for the combined filename (fp8 checked first), and correct variants for the others.
**Inputs:** `"model_fp16_fp8.safetensors"`, `"model_fp16.safetensors"`, `"model_bf16.safetensors"`, `"model_fp32.safetensors"`, `"model.safetensors"`.
**Expected output:** `Fp8`, `Fp16`, `Bf16`, `Fp32`, `Unknown` respectively.
**Acceptance command:** `cargo test -p anvilml-registry --test scanner_tests test_infer_dtype_fp8_before_fp16` exits 0.

## test_compute_id_deterministic (anvilml-registry)

**File:** `crates/anvilml-registry/tests/scanner_tests.rs`
**Context:** `ModelScanner::scan()` computes each model's ID by hashing the first 1 MiB of file content with SHA256. This test verifies that the ID is deterministic (same file → same ID across multiple scans) and has the correct format (64-character lowercase hex).
**Tests:** Creates a temp file with known content, scans it twice via `scan()`, and asserts both results have the same 64-character lowercase hex ID.
**Inputs:** Temp directory with one `.safetensors` file containing known bytes.
**Expected output:** Two `ModelMeta` entries with identical 64-char hex IDs.
**Acceptance command:** `cargo test -p anvilml-registry --test scanner_tests test_compute_id_deterministic` exits 0.

## test_scan_nonexistent_dir (anvilml-registry)

**File:** `crates/anvilml-registry/tests/scanner_tests.rs`
**Context:** When a configured model directory does not exist on disk, the scanner logs a DEBUG message and skips it without panicking or returning an error. This tests graceful degradation.
**Tests:** Calls `scan()` with a `ModelDirConfig` pointing to a non-existent path, and asserts the result is an empty vec.
**Inputs:** `ModelDirConfig{path: "/nonexistent/path/that/does/not/exist"}`.
**Expected output:** `Vec::new()` — empty results, no panic.
**Acceptance command:** `cargo test -p anvilml-registry --test scanner_tests test_scan_nonexistent_dir` exits 0.

## test_scan_with_files (anvilml-registry)

**File:** `crates/anvilml-registry/tests/scanner_tests.rs`
**Context:** Full scan path: creates temp directories with `.safetensors` files and a non-`.safetensors` file, scans both directories, and verifies each `ModelMeta` entry has the correct kind (from directory name), dtype (from filename), format (from extension), and a valid 64-char hex ID. Non-`.safetensors` files are skipped.
**Tests:** Creates `diffusion/` and `text_encoders/` dirs, writes `.safetensors` and `.pt` files, scans both dirs, asserts 2 results (`.pt` skipped), and verifies each result's kind, dtype, format, ID length, and timestamp freshness.
**Inputs:** Temp dirs with `diffusion/model_fp8.safetensors`, `text_encoders/clip_text.safetensors`, `diffusion/model.pt`.
**Expected output:** 2 `ModelMeta` entries: one with `kind=Diffusion, dtype=Fp8`, one with `kind=TextEncoder, dtype=Unknown`, both with valid IDs and recent timestamps.
**Acceptance command:** `cargo test -p anvilml-registry --test scanner_tests test_scan_with_files` exits 0.

## test_scan_empty_dir (anvilml-registry)

**File:** `crates/anvilml-registry/tests/scanner_tests.rs`
**Context:** An empty directory (exists but contains no files) should return an empty vec without errors. This tests the zero-file edge case.
**Tests:** Creates an empty temp directory, passes it to `scan()`, and asserts the result is an empty vec.
**Inputs:** Empty temp directory.
**Expected output:** `Vec::new()` — empty results, no errors.
**Acceptance command:** `cargo test -p anvilml-registry --test scanner_tests test_scan_empty_dir` exits 0.

## test_upsert_and_get (anvilml-registry)

**File:** `crates/anvilml-registry/tests/store_tests.rs`
**Context:** `ModelStore::upsert()` persists a model record via `INSERT OR REPLACE`, and `ModelStore::get()` retrieves it by ID via parameterised query. Each test uses its own `open_in_memory()` pool — no shared connections.
**Tests:** Constructs a `ModelMeta` for a diffusion model, upserts it via `store.upsert()`, then retrieves it via `store.get("model-1")` and asserts all 8 fields match the original.
**Inputs:** `ModelMeta{id="model-1", name="stable-diffusion-v1-5", kind=Diffusion, dtype=Fp16, format=Safetensors, size_bytes=1_073_741_824}`.
**Expected output:** `get()` returns `Some(meta)` with all fields matching the upserted record.
**Acceptance command:** `cargo test -p anvilml-registry --test store_tests test_upsert_and_get` exits 0.

## test_upsert_overwrites (anvilml-registry)

**File:** `crates/anvilml-registry/tests/store_tests.rs`
**Context:** `INSERT OR REPLACE` semantics: when the same model ID is upserted twice with different data, the second upsert overwrites the first. This is the correct behavior for the model scanner which re-scans directories and may produce updated metadata.
**Tests:** Upserts a model with name `"original-name"`, then upserts the same ID with name `"updated-name"`. Calls `get()` and asserts the returned name is `"updated-name"`.
**Inputs:** Two `ModelMeta` records with same ID `"model-1"` but different names.
**Expected output:** `get()` returns the second upserted version (name="updated-name").
**Acceptance command:** `cargo test -p anvilml-registry --test store_tests test_upsert_overwrites` exits 0.

## test_get_not_found (anvilml-registry)

**File:** `crates/anvilml-registry/tests/store_tests.rs`
**Context:** `get()` must return `None` (not an error) for a non-existent model ID. This distinguishes "not found" from "database error".
**Tests:** Creates a fresh in-memory database with no models, calls `get("non-existent-id")`, and asserts the result is `None`.
**Inputs:** Non-existent ID string `"non-existent-id"`.
**Expected output:** `get()` returns `None`.
**Acceptance command:** `cargo test -p anvilml-registry --test store_tests test_get_not_found` exits 0.

## test_list_all (anvilml-registry)

**File:** `crates/anvilml-registry/tests/store_tests.rs`
**Context:** `list(None)` returns all model records without filtering. Uses `SELECT * FROM models` when no kind filter is specified.
**Tests:** Upserts three models with different kinds (Diffusion, Vae, TextEncoder), calls `list(None)`, and asserts the returned vec has exactly 3 elements.
**Inputs:** Three `ModelMeta` records with distinct IDs and kinds.
**Expected output:** `list(None)` returns a vec of length 3.
**Acceptance command:** `cargo test -p anvilml-registry --test store_tests test_list_all` exits 0.

## test_list_filter_by_kind (anvilml-registry)

**File:** `crates/anvilml-registry/tests/store_tests.rs`
**Context:** `list(Some(kind))` appends `WHERE kind = ?` to the SELECT query, filtering results to only models of the specified kind. The kind is serialised to its snake_case string form.
**Tests:** Upserts two Diffusion models and one Vae model, calls `list(Some(ModelKind::Vae))`, and asserts exactly one model is returned with `kind == Vae`.
**Inputs:** Three `ModelMeta` records (2 Diffusion, 1 Vae), filter `Some(ModelKind::Vae)`.
**Expected output:** `list(Some(Vae))` returns a vec of length 1 containing only the Vae model.
**Acceptance command:** `cargo test -p anvilml-registry --test store_tests test_list_filter_by_kind` exits 0.

## test_delete_existing (anvilml-registry)

**File:** `crates/anvilml-registry/tests/store_tests.rs`
**Context:** `delete()` executes `DELETE FROM models WHERE id = ?` and checks `rows_affected() > 0`. Returns `true` when a row was deleted, `false` when no row matched. After deletion, `get()` must return `None`.
**Tests:** Upserts a model, calls `delete("model-1")` and asserts it returns `true`, then calls `get("model-1")` and asserts it returns `None`.
**Inputs:** One `ModelMeta` record with ID `"model-1"`.
**Expected output:** `delete()` returns `true`, `get()` returns `None`.
**Acceptance command:** `cargo test -p anvilml-registry --test store_tests test_delete_existing` exits 0.

## test_delete_not_found (anvilml-registry)

**File:** `crates/anvilml-registry/tests/store_tests.rs`
**Context:** `delete()` for a non-existent ID must return `false` without raising an error. SQLite's `DELETE` with a non-matching WHERE clause returns 0 rows affected.
**Tests:** Creates a fresh in-memory database with no models, calls `delete("non-existent-id")`, and asserts it returns `false`.
**Inputs:** Non-existent ID string `"non-existent-id"`.
**Expected output:** `delete()` returns `false`.
**Acceptance command:** `cargo test -p anvilml-registry --test store_tests test_delete_not_found` exits 0.

## test_get_existing_device (anvilml-registry)

**File:** `crates/anvilml-registry/tests/device_store_tests.rs`
**Context:** `DeviceCapabilityStore::get()` returns `Some(DeviceRow)` with all fields matching the database row for a known PCI vendor/device pair. Uses raw SQL INSERT to guarantee the row exists (independent of seed data coverage). Each test uses its own `open_in_memory()` pool with `max_connections(1)` — cloned pool is used for raw SQL inserts while the store also holds a reference to the same pool.
**Tests:** Inserts a device row via raw SQL (`vendor_id=4318, device_id=8994, name="NVIDIA H100-SXM5-80GB", arch="9.0", fp32=1, fp16=1, bf16=1, fp8=1, fp4=0, flash_attention=1`), calls `get(4318, 8994)`, and asserts all 10 fields match.
**Inputs:** Raw SQL INSERT into `device_capabilities` with H100 PCI pair.
**Expected output:** `get()` returns `Some(DeviceRow)` with `vendor_id=4318, device_id=8994, name="NVIDIA H100-SXM5-80GB", arch="9.0", fp32=true, fp16=true, bf16=true, fp8=true, fp4=false, flash_attention=true`.
**Acceptance command:** `cargo test -p anvilml-registry --test device_store_tests test_get_existing_device` exits 0.

## test_get_not_found (anvilml-registry)

**File:** `crates/anvilml-registry/tests/device_store_tests.rs`
**Context:** `DeviceCapabilityStore::get()` must return `Ok(None)` (not an error) for a vendor/device pair that has no matching row. This distinguishes "not found" from "database error".
**Tests:** Creates a fresh in-memory database with no device rows, calls `get(9999, 9999)`, and asserts the result is `None`.
**Inputs:** Non-existent PCI pair `(vendor_id=9999, device_id=9999)`.
**Expected output:** `get()` returns `Ok(None)`.
**Acceptance command:** `cargo test -p anvilml-registry --test device_store_tests test_get_not_found` exits 0.

## test_get_all_caps_true (anvilml-registry)

**File:** `crates/anvilml-registry/tests/device_store_tests.rs`
**Context:** Boolean flags stored as `INTEGER 1` in SQLite must map to `true` in the `DeviceRow` struct. This verifies the `row.get::<i64, _>("col") != 0` mapping pattern works correctly for all 6 boolean columns.
**Tests:** Inserts a device row with all capability flags set to `1`, calls `get()`, and asserts every boolean field is `true`.
**Inputs:** Raw SQL INSERT with `fp32=1, fp16=1, bf16=1, fp8=1, fp4=1, flash_attention=1`.
**Expected output:** All 6 boolean fields are `true`.
**Acceptance command:** `cargo test -p anvilml-registry --test device_store_tests test_get_all_caps_true` exits 0.

## test_get_all_caps_false (anvilml-registry)

**File:** `crates/anvilml-registry/tests/device_store_tests.rs`
**Context:** Boolean flags stored as `INTEGER 0` in SQLite must map to `false` in the `DeviceRow` struct. This is the inverse of `test_get_all_caps_true` and verifies the mapping works in both directions.
**Tests:** Inserts a device row with all capability flags set to `0`, calls `get()`, and asserts every boolean field is `false`.
**Inputs:** Raw SQL INSERT with `fp32=0, fp16=0, bf16=0, fp8=0, fp4=0, flash_attention=0`.
**Expected output:** All 6 boolean fields are `false`.
**Acceptance command:** `cargo test -p anvilml-registry --test device_store_tests test_get_all_caps_false` exits 0.

## test_list_models_empty (anvilml-server)

**File:** `crates/anvilml-server/tests/models_tests.rs`
**Context:** The `GET /v1/models` handler returns an empty JSON array when the model registry contains zero models. Exercises the production `build_router` path via `AppState::new()` which constructs an in-memory `ModelStore` with no models. Uses `Router::oneshot` to exercise the full handler pipeline without a live TCP listener.
**Tests:** Builds `AppState` with an empty in-memory database, sends a GET request to `/v1/models`, asserts HTTP 200, parses the JSON response, and verifies the body is an empty JSON array.
**Inputs:** GET `/v1/models`, `AppState::new("test-version")`.
**Expected output:** HTTP 200 with JSON body `[]`.
**Acceptance command:** `cargo test -p anvilml-server --test models_tests test_list_models_empty` exits 0.

## test_list_models_with_kind_filter (anvilml-server)

**File:** `crates/anvilml-server/tests/models_tests.rs`
**Context:** The `GET /v1/models?kind=` query parameter filters results to models of the specified kind. This test inserts a single diffusion model into an in-memory database, then verifies that `?kind=diffusion` returns the one model and `?kind=vae` returns an empty array. Uses `AppState::new_with_hardware` with a pre-built `Arc<ModelStore>` to avoid the sync/async boundary in the constructor.
**Tests:** Opens an in-memory pool, constructs a `ModelStore`, upserts one diffusion model, builds `AppState` with the registry, sends GET `/v1/models?kind=diffusion` (asserts 200 with 1 model), then sends GET `/v1/models?kind=vae` (asserts 200 with empty array).
**Inputs:** In-memory pool with one diffusion model; GET `/v1/models?kind=diffusion`, GET `/v1/models?kind=vae`.
**Expected output:** First request returns HTTP 200 with array of length 1 (id="diff-model-001"); second request returns HTTP 200 with empty array `[]`.
**Acceptance command:** `cargo test -p anvilml-server --test models_tests test_list_models_with_kind_filter` exits 0.

## test_get_model_not_found (anvilml-server)

**File:** `crates/anvilml-server/tests/models_tests.rs`
**Context:** The `GET /v1/models/:id` handler returns HTTP 404 with `{"error":"model_not_found"}` when the model ID does not exist in the registry. Uses `AppState::new()` with an empty in-memory database. Exercises the production `build_router` path via `Router::oneshot`.
**Tests:** Builds `AppState` with an empty in-memory database, sends a GET request to `/v1/models/nonexistent-id`, asserts HTTP 404, parses the JSON response, and verifies `error == "model_not_found"`.
**Inputs:** GET `/v1/models/nonexistent-id`, `AppState::new("test-version")`.
**Expected output:** HTTP 404 with JSON body `{"error":"model_not_found","message":"model not found: nonexistent-id","request_id":"<uuid>"}`.
**Acceptance command:** `cargo test -p anvilml-server --test models_tests test_get_model_not_found` exits 0.

## test_rescan_returns_202 (anvilml-server)

**File:** `crates/anvilml-server/tests/models_tests.rs`
**Context:** The `POST /v1/models/rescan` handler responds with HTTP 202 Accepted immediately and spawns a background task. Uses `AppState::new()` which has an empty `model_dirs` vec — the scanner scans zero directories.
**Tests:** Sends POST to `/v1/models/rescan`, asserts HTTP 202 status, parses JSON body, and verifies `status == "scanning"`.
**Inputs:** POST `/v1/models/rescan`, `AppState::new("test-version")`.
**Expected output:** HTTP 202 with JSON body `{"status":"scanning"}`.
**Acceptance command:** `cargo test -p anvilml-server --test models_tests -- rescan_returns_202` exits 0.

## test_rescan_populates_registry (anvilml-server)

**File:** `crates/anvilml-server/tests/models_tests.rs`
**Context:** After POST /v1/models/rescan with model files on disk, GET /v1/models returns the scanned models. Uses a temporary directory with a `.safetensors` file and configures `AppState` with that directory via `new_with_hardware`.
**Tests:** Creates a temp dir with `test-model.safetensors`, builds `AppState` with that dir, triggers rescan, waits for background task, then verifies the model appears in `GET /v1/models` with correct name and kind=unknown.
**Inputs:** Temp dir with `test-model.safetensors`, POST `/v1/models/rescan`, GET `/v1/models`.
**Expected output:** 200 response with JSON array containing one model with `name="test-model.safetensors"`, `kind="unknown"`.
**Acceptance command:** `cargo test -p anvilml-server --test models_tests -- rescan_populates` exits 0.

## test_rescan_infer_kind_and_dtype (anvilml-server)

**File:** `crates/anvilml-server/tests/models_tests.rs`
**Context:** Scanned models have correct `kind` (from directory name) and `dtype` (from filename). Creates two temp subdirectories (`diffusion/` with `model_fp8.safetensors`, `vae/` with `model.safetensors`) and passes each as a separate `ModelDirConfig`.
**Tests:** After rescan, verifies the diffusion model has `kind=diffusion, dtype=fp8` and the vae model has `kind=vae, dtype=unknown`.
**Inputs:** Two temp dirs with model files, POST `/v1/models/rescan`, GET `/v1/models`.
**Expected output:** 200 response with JSON array of 2 models with correct kind/dtype fields.
**Acceptance command:** `cargo test -p anvilml-server --test models_tests -- infer_kind_and_dtype` exits 0.

## test_broadcaster_new (anvilml-server)

**File:** `crates/anvilml-server/tests/broadcaster_tests.rs`
**Context:** `EventBroadcaster::new()` creates a valid broadcaster with channel capacity 1024. Verifies that `subscribe()` works and the receiver can receive a broadcast event. Also exercises the `Default` impl which delegates to `new()`.
**Tests:** Constructs `EventBroadcaster::new()`, calls `subscribe()`, sends a `WsEvent::SystemStats`, and asserts the receiver gets the event via `recv().await`.
**Inputs:** None (uses `EventBroadcaster::new()`).
**Expected output:** `recv().await` returns `Ok(WsEvent::SystemStats{...})` — the constructor and subscription are functional.
**Acceptance command:** `cargo test -p anvilml-server --features mock-hardware -- broadcaster` exits 0.

## test_broadcaster_send_and_receive (anvilml-server)

**File:** `crates/anvilml-server/tests/broadcaster_tests.rs`
**Context:** `send()` delivers an event to a subscriber; the received event matches the sent event exactly. Verifies the core broadcast path works correctly with a known event.
**Tests:** Creates a broadcaster, subscribes, sends a `WsEvent::SystemStats` with `cpu_pct=42.5, ram_used_mib=8192, workers=[]`, and asserts `recv().await` returns an identical event.
**Inputs:** `WsEvent::SystemStats{cpu_pct: 42.5, ram_used_mib: 8192, workers: []}`.
**Expected output:** `received == expected` — the event roundtrips through the broadcast channel without modification.
**Acceptance command:** `cargo test -p anvilml-server --features mock-hardware -- broadcaster` exits 0.

## test_broadcaster_lagged_receiver (anvilml-server)

**File:** `crates/anvilml-server/tests/broadcaster_tests.rs`
**Context:** When all subscribers drop while the channel is full, `send()` returns `Err(SendError)` and the event is dropped. Verifies the error path for lagged receivers.
**Tests:** Creates a broadcaster, subscribes, sends 1024 events to fill the buffer (evicting older events), drops the subscriber, then sends one more event. The final `send()` must return `Err` because the channel is full and there are no receivers.
**Inputs:** 1025 events sent, subscriber dropped after 1024.
**Expected output:** `send(1025th)` returns `Err(SendError)` — the error return path is exercised when all subscribers are gone.
**Acceptance command:** `cargo test -p anvilml-server --features mock-hardware -- broadcaster` exits 0.
