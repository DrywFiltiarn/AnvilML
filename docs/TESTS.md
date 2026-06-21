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
**Context:** The server binary accepts `--port` CLI override, binds to the OS-assigned port, and the health endpoint returns HTTP 200. Since P9-C1, the server process also binds an unrelated second TCP listener (the ZeroMQ ROUTER socket for worker IPC), so port detection cannot rely on OS-level socket-table scans scoped only by PID — it must identify the HTTP listener specifically.
**Tests:** Spawns the pre-built anvilml binary with `--port 0` (OS-assigned port), recovers the bound port by reading the mandatory `"listening"` INFO log line (`addr=...`) on the subprocess's stderr, sends `GET /health`, and asserts HTTP 200 with `{"status":"ok"}`.
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

## test_events_route_returns_101 (anvilml-server)

**File:** `crates/anvilml-server/tests/handler_tests.rs`
**Context:** The `/v1/events` route exists and returns HTTP 101 on a WebSocket upgrade request. Tests use a real TCP listener (`axum::serve`) because axum's `WebSocketUpgrade` extractor requires the `hyper::upgrade::OnUpgrade` extension which is only set up when the server processes a real HTTP connection. `Router::oneshot` does not set up this extension.
**Tests:** Starts a real HTTP server on a random port, sends a raw HTTP request with WebSocket upgrade headers (`Upgrade: websocket`, `Connection: Upgrade`, `Sec-WebSocket-Key`, `Sec-WebSocket-Version: 13`), and asserts the response status line contains "101".
**Inputs:** GET `/v1/events` with WebSocket upgrade headers, `AppState::new("test-version")`.
**Expected output:** HTTP 101 Switching Protocols response — the route accepts WebSocket upgrades.
**Acceptance command:** `cargo test -p anvilml-server --features mock-hardware --test handler_tests test_events_route_returns_101` exits 0.

## test_events_delivers_broadcast_event (anvilml-server)

**File:** `crates/anvilml-server/tests/handler_tests.rs`
**Context:** A WebSocket client connected to `/v1/events` receives broadcast events as JSON text frames. The test uses a real TCP listener and raw TCP I/O to verify the end-to-end event delivery path: broadcaster → handler subscription → JSON serialization → WebSocket text frame → client. The handler uses `ConnectInfo` to extract the client's socket address, which requires `into_make_service_with_connect_info`.
**Tests:** Starts a real HTTP server, connects with a raw HTTP request containing WebSocket upgrade headers, verifies the 101 response, broadcasts a `WsEvent::SystemStats` through the broadcaster, reads the raw WebSocket frame from the client socket (skipping the 2-byte frame header), parses the JSON payload, and asserts the event type and fields match the broadcast event.
**Inputs:** `WsEvent::SystemStats{cpu_pct: 42.5, ram_used_mib: 8192, workers: []}`.
**Expected output:** Client receives `{"type":"system_stats","cpu_pct":42.5,"ram_used_mib":8192,"workers":[]}` as a WebSocket text frame.
**Acceptance command:** `cargo test -p anvilml-server --features mock-hardware --test handler_tests test_events_delivers_broadcast_event` exits 0.

## test_stats_tick_broadcasts_system_stats (anvilml-server)

**File:** `crates/anvilml-server/tests/stats_tick_tests.rs`
**Context:** The `stats_tick::start()` function spawns a tokio task that broadcasts `WsEvent::SystemStats` events every 5 seconds. This test verifies the event actually arrives on the broadcast channel by subscribing to the broadcaster and waiting for the first event.
**Tests:** Creates an `EventBroadcaster`, subscribes, calls `start()`, waits up to 6 seconds for a `SystemStats` event, and asserts that the event was received with the correct variant.
**Inputs:** None (uses `EventBroadcaster::new()` and `start()`).
**Expected output:** A `WsEvent::SystemStats` event received on the subscriber within 6 seconds.
**Acceptance command:** `cargo test -p anvilml-server --features mock-hardware -- stats_tick` exits 0.

## test_stats_tick_cpu_pct_is_finite (anvilml-server)

**File:** `crates/anvilml-server/tests/stats_tick_tests.rs`
**Context:** The CPU percentage value read from `sysinfo::System::global_cpu_usage()` is cast from `f64` to `f32`. This test verifies the resulting value is a finite `f32` (not NaN or infinity), which would indicate a bug in the sysinfo API usage or the cast.
**Tests:** Waits for one `SystemStats` event and asserts that `cpu_pct.is_finite()` is `true`.
**Inputs:** None (uses `EventBroadcaster::new()` and `start()`).
**Expected output:** Event received with `cpu_pct.is_finite() == true`.
**Acceptance command:** `cargo test -p anvilml-server --features mock-hardware -- stats_tick` exits 0.

## test_stats_tick_ram_used_mib_is_non_negative (anvilml-server)

**File:** `crates/anvilml-server/tests/stats_tick_tests.rs`
**Context:** The RAM usage in mebibytes is computed as `sys.used_memory() / (1024 * 1024)`. Since `used_memory()` returns `u64`, the result is always non-negative. This test documents that invariant by asserting `ram_used_mib >= 0`.
**Tests:** Waits for one `SystemStats` event and asserts that `ram_used_mib` is non-negative.
**Inputs:** None (uses `EventBroadcaster::new()` and `start()`).
**Expected output:** Event received with `ram_used_mib >= 0`.
**Acceptance command:** `cargo test -p anvilml-server --features mock-hardware -- stats_tick` exits 0.

## test_ping_roundtrip (anvilml-ipc)

**File:** `crates/anvilml-ipc/tests/roundtrip_tests.rs`
**Context:** `WorkerMessage::Ping` uses `#[serde(tag = "_type")]` for the discriminated union format. The `encode_message()` function uses `rmp_serde::to_vec_named` to produce a flat msgpack dict, and the roundtrip uses `rmp_serde::from_slice::<WorkerMessage>` to decode back to the same type. No I/O, no subprocess, no network.
**Tests:** Constructs `WorkerMessage::Ping { seq: 42 }`, encodes via `encode_message()`, decodes via `rmp_serde::from_slice::<WorkerMessage>`, and asserts the decoded message matches the original.
**Inputs:** `WorkerMessage::Ping { seq: 42 }`.
**Expected output:** `decoded == Ping { seq: 42 }` — the seq field is preserved through msgpack roundtrip.
**Acceptance command:** `cargo test -p anvilml-ipc -- messages` exits 0.

## test_shutdown_roundtrip (anvilml-ipc)

**File:** `crates/anvilml-ipc/tests/roundtrip_tests.rs`
**Context:** `WorkerMessage::Shutdown` is a unit variant (no fields). Verifies that unit variants serialize to a flat dict with only the `_type` key.
**Tests:** Constructs `WorkerMessage::Shutdown`, encodes via `encode_message()`, decodes via `rmp_serde::from_slice::<WorkerMessage>`, and asserts the decoded message is `Shutdown`.
**Inputs:** `WorkerMessage::Shutdown`.
**Expected output:** `decoded == Shutdown`.
**Acceptance command:** `cargo test -p anvilml-ipc -- messages` exits 0.

## test_execute_roundtrip (anvilml-ipc)

**File:** `crates/anvilml-ipc/tests/roundtrip_tests.rs`
**Context:** `WorkerMessage::Execute` is the most data-rich variant with 4 fields (`job_id: Uuid`, `graph: serde_json::Value`, `settings: JobSettings`, `device_index: u32`). The `graph` field contains nested JSON objects and arrays. Verifies all fields including the deeply-nested graph structure survive msgpack roundtrip.
**Tests:** Constructs `WorkerMessage::Execute` with a UUID, a graph containing nodes and links arrays, `JobSettings` with a device preference, and `device_index: 0`. Encodes via `encode_message()`, decodes via `rmp_serde::from_slice::<WorkerMessage>`, and asserts all fields match.
**Inputs:** `WorkerMessage::Execute{job_id: Uuid::new_v4(), graph: {"nodes": [...], "links": [...]}, settings: {device_preference: Some("cuda")}, device_index: 0}`.
**Expected output:** All 4 fields match after roundtrip, including the nested graph structure.
**Acceptance command:** `cargo test -p anvilml-ipc -- messages` exits 0.

## test_cancel_job_roundtrip (anvilml-ipc)

**File:** `crates/anvilml-ipc/tests/roundtrip_tests.rs`
**Context:** `WorkerMessage::CancelJob` carries a single `Uuid` field. Verifies UUID serialization through msgpack flat-dict format.
**Tests:** Constructs `WorkerMessage::CancelJob` with a v4 UUID, encodes, decodes, and asserts the job_id matches.
**Inputs:** `WorkerMessage::CancelJob{job_id: Uuid::new_v4()}`.
**Expected output:** `decoded.job_id == original.job_id`.
**Acceptance command:** `cargo test -p anvilml-ipc -- messages` exits 0.

## test_memory_query_roundtrip (anvilml-ipc)

**File:** `crates/anvilml-ipc/tests/roundtrip_tests.rs`
**Context:** `WorkerMessage::MemoryQuery` is a unit variant. Verifies that unit variants serialize to a flat dict with only the `_type` key.
**Tests:** Constructs `WorkerMessage::MemoryQuery`, encodes via `encode_message()`, decodes via `rmp_serde::from_slice::<WorkerMessage>`, and asserts the decoded message is `MemoryQuery`.
**Inputs:** `WorkerMessage::MemoryQuery`.
**Expected output:** `decoded == MemoryQuery`.
**Acceptance command:** `cargo test -p anvilml-ipc -- messages` exits 0.

## test_ready_roundtrip (anvilml-ipc)

**File:** `crates/anvilml-ipc/tests/roundtrip_tests.rs`
**Context:** `WorkerEvent::Ready` is the most data-rich event variant with 12 fields including `Vec<NodeTypeDescriptor>` (nested structs). This is the synchronization event between Rust and Python. Verifies all fields including the node type descriptors survive msgpack roundtrip.
**Tests:** Constructs `WorkerEvent::Ready` with all fields set to realistic values (worker_id, device info, torch_version, fp16/bf16/fp8/flash_attention bools, and a vector of NodeTypeDescriptor), encodes via `rmp_serde::to_vec_named()`, decodes via `decode_event()`, and asserts all 12 fields match.
**Inputs:** `WorkerEvent::Ready{worker_id: "worker-0", device_index: 0, device_name: "NVIDIA RTX 4090", device_type: "cuda", vram_total_mib: 24576, vram_free_mib: 24000, torch_version: "2.5.1", fp16: true, bf16: true, fp8: false, flash_attention: true, node_types: [NodeTypeDescriptor{type_name: "KSampler", ...}]}`.
**Expected output:** All 12 fields match after roundtrip.
**Acceptance command:** `cargo test -p anvilml-ipc -- messages` exits 0.

## test_pong_roundtrip (anvilml-ipc)

**File:** `crates/anvilml-ipc/tests/roundtrip_tests.rs`
**Context:** `WorkerEvent::Pong` is a simple two-field variant (`seq: u64`). Verifies u64 serialization through msgpack flat-dict format.
**Tests:** Constructs `WorkerEvent::Pong { seq: 42 }`, encodes via `rmp_serde::to_vec_named()`, decodes via `decode_event()`, and asserts the seq field matches.
**Inputs:** `WorkerEvent::Pong { seq: 42 }`.
**Expected output:** `decoded.seq == 42`.
**Acceptance command:** `cargo test -p anvilml-ipc -- messages` exits 0.

## test_dying_roundtrip (anvilml-ipc)

**File:** `crates/anvilml-ipc/tests/roundtrip_tests.rs`
**Context:** `WorkerEvent::Dying` carries a single `String` field (`reason`). Verifies string serialization through msgpack flat-dict format.
**Tests:** Constructs `WorkerEvent::Dying { reason: "SIGTERM" }`, encodes via `rmp_serde::to_vec_named()`, decodes via `decode_event()`, and asserts the reason matches.
**Inputs:** `WorkerEvent::Dying { reason: "SIGTERM" }`.
**Expected output:** `decoded.reason == "SIGTERM"`.
**Acceptance command:** `cargo test -p anvilml-ipc -- messages` exits 0.

## test_completed_roundtrip (anvilml-ipc)

**File:** `crates/anvilml-ipc/tests/roundtrip_tests.rs`
**Context:** `WorkerEvent::Completed` carries a `Uuid` and a `u64` (elapsed_ms). Verifies both types survive msgpack roundtrip.
**Tests:** Constructs `WorkerEvent::Completed` with a v4 UUID and elapsed_ms=1234, encodes, decodes, and asserts both fields match.
**Inputs:** `WorkerEvent::Completed{job_id: Uuid::new_v4(), elapsed_ms: 1234}`.
**Expected output:** `decoded.job_id == original.job_id` and `decoded.elapsed_ms == 1234`.
**Acceptance command:** `cargo test -p anvilml-ipc -- messages` exits 0.

## test_failed_roundtrip (anvilml-ipc)

**File:** `crates/anvilml-ipc/tests/roundtrip_tests.rs`
**Context:** `WorkerEvent::Failed` carries a `Uuid`, a `String` error, and an `Option<String>` traceback. Verifies that `Some` values are preserved through msgpack roundtrip.
**Tests:** Constructs `WorkerEvent::Failed` with a v4 UUID, error="OOM", and a non-None traceback, encodes, decodes, and asserts all fields match.
**Inputs:** `WorkerEvent::Failed{job_id: Uuid::new_v4(), error: "OOM", traceback: Some("Traceback...")}`.
**Expected output:** All 3 fields match after roundtrip.
**Acceptance command:** `cargo test -p anvilml-ipc -- messages` exits 0.

## test_cancelled_roundtrip (anvilml-ipc)

**File:** `crates/anvilml-ipc/tests/roundtrip_tests.rs`
**Context:** `WorkerEvent::Cancelled` carries a single `Uuid` field. Verifies UUID serialization through msgpack flat-dict format.
**Tests:** Constructs `WorkerEvent::Cancelled` with a v4 UUID, encodes, decodes, and asserts the job_id matches.
**Inputs:** `WorkerEvent::Cancelled{job_id: Uuid::new_v4()}`.
**Expected output:** `decoded.job_id == original.job_id`.
**Acceptance command:** `cargo test -p anvilml-ipc -- messages` exits 0.

## test_image_ready_roundtrip (anvilml-ipc)

**File:** `crates/anvilml-ipc/tests/roundtrip_tests.rs`
**Context:** `WorkerEvent::ImageReady` carries 7 fields including a base64 string, dimensions, format, seed (i64), and steps (u32). This is the most data-rich non-Ready event. Verifies all field types survive msgpack roundtrip.
**Tests:** Constructs `WorkerEvent::ImageReady` with all fields set, encodes via `rmp_serde::to_vec_named()`, decodes via `decode_event()`, and asserts all 7 fields match.
**Inputs:** `WorkerEvent::ImageReady{job_id: Uuid::new_v4(), image_b64: "dGVzdCBpbWFnZQ==", width: 512, height: 512, format: "png", seed: 42, steps: 20}`.
**Expected output:** All 7 fields match after roundtrip.
**Acceptance command:** `cargo test -p anvilml-ipc -- messages` exits 0.

## test_progress_roundtrip (anvilml-ipc)

**File:** `crates/anvilml-ipc/tests/roundtrip_tests.rs`
**Context:** `WorkerEvent::Progress` carries a `Uuid`, two `u32` fields (step, total_steps), and an `Option<String>` preview_b64. Tests the `None` case for the optional field.
**Tests:** Constructs `WorkerEvent::Progress` with preview_b64=None, encodes, decodes, and asserts all fields match.
**Inputs:** `WorkerEvent::Progress{job_id: Uuid::new_v4(), step: 5, total_steps: 20, preview_b64: None}`.
**Expected output:** `decoded.preview_b64.is_none() == true` and all other fields match.
**Acceptance command:** `cargo test -p anvilml-ipc -- messages` exits 0.

## test_progress_with_preview_roundtrip (anvilml-ipc)

**File:** `crates/anvilml-ipc/tests/roundtrip_tests.rs`
**Context:** `WorkerEvent::Progress` with a non-None `preview_b64` value. Tests that the `Some` variant of the optional field survives msgpack roundtrip.
**Tests:** Constructs `WorkerEvent::Progress` with preview_b64=Some("aW1hZ2UgZGF0YQ=="), encodes, decodes, and asserts all fields match.
**Inputs:** `WorkerEvent::Progress{job_id: Uuid::new_v4(), step: 10, total_steps: 20, preview_b64: Some("aW1hZ2UgZGF0YQ==")}`.
**Expected output:** `decoded.preview_b64 == Some("aW1hZ2UgZGF0YQ==")` and all other fields match.
**Acceptance command:** `cargo test -p anvilml-ipc -- messages` exits 0.

## test_memory_report_roundtrip (anvilml-ipc)

**File:** `crates/anvilml-ipc/tests/roundtrip_tests.rs`
**Context:** `WorkerEvent::MemoryReport` carries two fields (`vram_used_mib: u32`, `ram_used_mib: u64`). Verifies both integer types survive msgpack roundtrip.
**Tests:** Constructs `WorkerEvent::MemoryReport` with vram_used_mib=4096, ram_used_mib=8192, encodes, decodes, and asserts both fields match.
**Inputs:** `WorkerEvent::MemoryReport{vram_used_mib: 4096, ram_used_mib: 8192}`.
**Expected output:** `decoded.vram_used_mib == 4096` and `decoded.ram_used_mib == 8192`.
**Acceptance command:** `cargo test -p anvilml-ipc -- messages` exits 0.

## test_mock_startup_sends_ready (worker)

**File:** `worker/tests/test_worker_main.py`
**Context:** The worker_main.py module spawns a subprocess that connects to a ROUTER socket, emits a Ready event with mock hardware values, and enters a dispatch loop. Each test creates its own ROUTER socket on a random port and spawns the worker as a subprocess with explicit env vars (os.environ is not inherited through subprocess unless env is passed). The Ready event is received via `_recv_with_timeout()`, which sets `zmq.RCVTIMEO` and surfaces the worker subprocess's stderr if no message arrives within 5s — this prevents an indefinite hang if the worker dies on startup (e.g. a `SyntaxError`) before sending Ready. See `docs/ENVIRONMENT.md §11.5`.
**Tests:** Spawns `worker_main.py` as a subprocess with `ANVILML_WORKER_MOCK=1`, reads the Ready event from the ROUTER socket, and asserts all 12 required fields are present with correct values (worker_id="worker-0", device_index=0, device_name="Mock", device_type="cpu", vram values=8192, torch_version="mock", fp16/bf16/fp8/flash_attention=True, node_types=[]).
**Inputs:** Subprocess with env vars `ANVILML_IPC_PORT=<random port>`, `ANVILML_WORKER_ID=worker-0`, `ANVILML_DEVICE_INDEX=0`, `ANVILML_DEVICE_TYPE=cpu`.
**Expected output:** Ready event received with `_type="Ready"` and all fields matching the mock mode spec.
**Acceptance command:** `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_worker_main.py::test_mock_startup_sends_ready -v` exits 0.

## test_ping_returns_pong (worker)

**File:** `worker/tests/test_worker_main.py`
**Context:** The worker dispatch loop responds to Ping messages with Pong containing the same sequence number. This verifies the heartbeat mechanism works end-to-end through the ROUTER/DEALER transport. Both the Ready-drain receive and the Pong receive go through `_recv_with_timeout()` (see `docs/ENVIRONMENT.md §11.5`) rather than a raw, unguarded `router.recv()`.
**Tests:** Starts the worker in mock mode, sends a `Ping{seq: 42}` message via the ROUTER, receives the Pong response, and asserts `_type == "Pong"` and `seq == 42`.
**Inputs:** `Ping{seq: 42}` sent via ROUTER to the worker subprocess.
**Expected output:** `Pong{seq: 42}` received, `_type == "Pong"`, `seq == 42`.
**Acceptance command:** `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_worker_main.py::test_ping_returns_pong -v` exits 0.

## test_shutdown_exits_cleanly (worker)

**File:** `worker/tests/test_worker_main.py`
**Context:** The worker exits with code 0 when it receives a Shutdown message. This verifies the graceful shutdown contract with the Rust supervisor. This test has no ROUTER `recv()` calls; its only blocking wait is `proc.wait(timeout=10)`, which was already correctly bounded and is the reference pattern cited in `docs/ENVIRONMENT.md §11.5`.
**Tests:** Starts the worker, sends a Shutdown message via ROUTER, asserts the subprocess exits with code 0 within a 10-second timeout.
**Inputs:** `Shutdown` sent via ROUTER to the worker subprocess.
**Expected output:** Subprocess exit code == 0 within timeout.
**Acceptance command:** `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_worker_main.py::test_shutdown_exits_cleanly -v` exits 0.

## test_env_vars_read_from_environment (worker)

**File:** `worker/tests/test_worker_main.py`
**Context:** The worker reads identity and connection parameters from environment variables and includes them in the Ready event. Verifies the env var passthrough path works correctly with custom values. The Ready event is received via `_recv_with_timeout()` (see `docs/ENVIRONMENT.md §11.5`) rather than a raw, unguarded `router.recv()`.
**Tests:** Sets `ANVILML_WORKER_ID`, `ANVILML_DEVICE_INDEX`, `ANVILML_DEVICE_TYPE` to custom values before launching the worker, then verifies the Ready event contains those values in the corresponding fields.
**Inputs:** `ANVILML_WORKER_ID=custom-worker`, `ANVILML_DEVICE_INDEX=3`, `ANVILML_DEVICE_TYPE=cuda`.
**Expected output:** Ready event `worker_id == "custom-worker"`, `device_index == 3`, `device_type == "cuda"`.
**Acceptance command:** `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_worker_main.py::test_env_vars_read_from_environment -v` exits 0.

## test_encode_produces_non_empty_bytes (anvilml-ipc)

**File:** `crates/anvilml-ipc/tests/roundtrip_tests.rs`
**Context:** Every `WorkerMessage` variant must produce a non-empty byte vector when encoded. This verifies the encoding function works for all variants including unit variants.
**Tests:** Iterates over all 5 `WorkerMessage` variants, encodes each via `encode_message()`, and asserts the result is non-empty.
**Inputs:** All 5 `WorkerMessage` variants: `Ping`, `Shutdown`, `MemoryQuery`, `CancelJob`, `Execute`.
**Expected output:** All encoded byte vectors have length > 0.
**Acceptance command:** `cargo test -p anvilml-ipc -- messages` exits 0.

## test_ipc_error_display (anvilml-ipc)

**File:** `crates/anvilml-ipc/tests/roundtrip_tests.rs`
**Context:** `IpcError` derives `thiserror::Error` which implements `Display`. Verifies that the error messages contain the original string for debugging.
**Tests:** Constructs both `IpcError::Serialize("test error")` and `IpcError::Deserialize("test error")`, formats them with `{}`, and asserts the formatted string contains "test error".
**Inputs:** `IpcError::Serialize("test error")`, `IpcError::Deserialize("test error")`.
**Expected output:** Both error display strings contain "test error".
**Acceptance command:** `cargo test -p anvilml-ipc -- messages` exits 0.

## bind_returns_nonzero_port (anvilml-ipc)

**File:** `crates/anvilml-ipc/tests/transport_tests.rs`
**Context:** `RouterTransport::bind()` creates a ZeroMQ ROUTER socket and binds to `tcp://127.0.0.1:0`, which causes the OS to assign an available port.
**Tests:** The bound port is greater than zero, confirming the OS-assigned port was extracted correctly from the `Endpoint::Tcp(_, port)`.
**Inputs:** None.
**Expected output:** `transport.port > 0`.
**Acceptance command:** `cargo test -p anvilml-ipc -- bind_returns_nonzero_port` exits 0.

## send_delivers_message_to_dealer (anvilml-ipc)

**File:** `crates/anvilml-ipc/tests/transport_tests.rs`
**Context:** A ZeroMQ ROUTER socket routes messages to connected peers by identity. The DEALER socket auto-generates a random 5-byte identity on connect. This test discovers the identity by having the ROUTER receive a probe message, then uses that identity to send a real message.
**Tests:** The ROUTER successfully delivers a msgpack-encoded `WorkerMessage::Ping { seq: 1 }` to a connected DEALER socket. The DEALER receives the payload and it decodes to the original message.
**Inputs:** `RouterTransport::bind()`, DEALER socket connected to the bound address, `WorkerMessage::Ping { seq: 1 }`.
**Expected output:** DEALER receives a single-frame message that decodes to `WorkerMessage::Ping { seq: 1 }`.
**Acceptance command:** `cargo test -p anvilml-ipc -- send_delivers_message_to_dealer` exits 0.

## send_to_unknown_worker_returns_error (anvilml-ipc)

**File:** `crates/anvilml-ipc/tests/transport_tests.rs`
**Context:** The ROUTER socket only routes to peers with a known identity in its internal peer table. Sending to an unknown identity returns `ZmqError::Other("Destination client not found by identity")`.
**Tests:** `RouterTransport::send()` returns a `TransportError::Zmq` when the worker identity is not connected.
**Inputs:** `RouterTransport::bind()`, worker_id = `"nonexistent-worker"`, any `WorkerMessage`.
**Expected output:** `Err(TransportError::Zmq(ZmqError::Other("Destination client not found by identity")))`.
**Acceptance command:** `cargo test -p anvilml-ipc -- send_to_unknown_worker_returns_error` exits 0.

## recv_roundtrip (anvilml-ipc)

**File:** `crates/anvilml-ipc/tests/transport_tests.rs`
**Context:** `RouterTransport::recv()` receives a multipart message from the ZeroMQ ROUTER socket, extracts the identity frame as a UTF-8 string, and decodes the msgpack payload into a `WorkerEvent`. This test verifies the full identity routing path: a DEALER socket with a known identity sends a `WorkerEvent::Pong{seq:42}` through the ROUTER, and `recv()` returns the correct `(worker_id, event)` tuple.
**Tests:** Creates a `RouterTransport` via `bind()`, creates a `DealerSocket` with identity `"test-worker-0"` via `SocketOptions::peer_identity()`, connects to the ROUTER, sends a msgpack-encoded `Pong{seq:42}` as a multipart message `[identity, payload]`, then calls `recv()` and asserts `worker_id == "test-worker-0"` and `event == Pong{seq:42}`.
**Inputs:** `RouterTransport::bind()`, DEALER identity `"test-worker-0"`, `WorkerEvent::Pong{seq:42}`.
**Expected output:** `recv()` returns `("test-worker-0", WorkerEvent::Pong{seq:42})`.
**Acceptance command:** `cargo test -p anvilml-ipc -- recv_roundtrip` exits 0.

## test_connect_succeeds (worker)

**File:** `worker/tests/test_ipc.py`
**Context:** `ipc.connect(port, worker_id)` creates a DEALER socket, sets the identity, and connects to the ROUTER at the given port. Uses an in-process ROUTER socket bound on a random ephemeral port — no shared state with other tests. The `mock_mode` autouse fixture ensures `ANVILML_WORKER_MOCK=1` is set.
**Tests:** Creates a `zmq.Context()` and `zmq.ROUTER` socket bound on a random port, calls `ipc.connect(port, "test-worker")`, and asserts `ipc._sock` and `ipc._ctx` are not None.
**Inputs:** Random ephemeral port, worker_id = `"test-worker"`.
**Expected output:** `ipc._sock is not None` and `ipc._ctx is not None`.
**Acceptance command:** `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_ipc.py::test_connect_succeeds -v` exits 0.

## test_connect_sets_identity (worker)

**File:** `worker/tests/test_ipc.py`
**Context:** ZeroMQ DEALER sockets prepend the identity frame to every message received by the ROUTER. The identity is set via `setsockopt(zmq.IDENTITY, worker_id.encode())` before `connect()`. Uses ROUTER/DEALER sockets (not PAIR) because PAIR has no identity frames.
**Tests:** Creates a ROUTER socket bound on a random port, connects via `ipc.connect(port, "test-worker")`, sends a message via `ipc.send_event()`, reads the ROUTER's multipart frame, and asserts the identity frame equals `b"test-worker"`.
**Inputs:** In-process ROUTER socket, worker_id = `"test-worker"`.
**Expected output:** `router.recv() == b"test-worker"` (identity frame of multipart message).
**Acceptance command:** `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_ipc.py::test_connect_sets_identity -v` exits 0.

## test_send_event_encodes_type_discriminator (worker)

**File:** `worker/tests/test_ipc.py`
**Context:** The `_type` key in the msgpack-serialised dict survives the roundtrip intact. This is the event discriminator used by the Rust supervisor to route messages. Uses ROUTER socket to receive the identity frame and raw msgpack payload.
**Tests:** Sends `{"_type": "Ready", "node_types": ["LoadModel"]}` via `ipc.send_event()`, receives from ROUTER, deserialises with `msgpack.unpackb(raw, raw=False)`, and asserts `_type == "Ready"` and all payload fields are preserved.
**Inputs:** Dict with `_type: "Ready"` and a node type list.
**Expected output:** `received["_type"] == "Ready"` and `received["node_types"] == ["LoadModel"]`.
**Acceptance command:** `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_ipc.py::test_send_event_encodes_type_discriminator -v` exits 0.

## test_recv_message_deserialises_correctly (worker)

**File:** `worker/tests/test_ipc.py`
**Context:** `ipc.recv_message()` receives raw bytes from the DEALER socket and deserialises them with `msgpack.unpackb(data, raw=False)`. Tests the deserialisation path by sending a msgpack-serialised dict from the ROUTER side.
**Tests:** Connects via `ipc.connect()`, sends a msgpack-serialised dict from the ROUTER side via `router.send_multipart([b"test-worker", msgpack.packb(payload)])`, calls `ipc.recv_message()`, and asserts the returned dict matches the payload.
**Inputs:** Dict `{"_type": "DispatchJob", "job_id": "abc-123"}` sent from ROUTER.
**Expected output:** `recv_message() == {"_type": "DispatchJob", "job_id": "abc-123"}`.
**Acceptance command:** `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_ipc.py::test_recv_message_deserialises_correctly -v` exits 0.

## test_roundtrip_via_pair_sockets (worker)

**File:** `worker/tests/test_ipc.py`
**Context:** Verifies the msgpack encoding/decoding mechanism that `ipc.py` relies on, without involving the ROUTER/DEALER identity routing layer. Uses two in-process PAIR sockets connected via bind/connect pattern.
**Tests:** Creates two PAIR sockets connected in-process, packs data with `msgpack.packb(data, use_bin_type=True)` on one end, receives on the other with `p2.recv()`, unpacks with `msgpack.unpackb(raw, raw=False)`, and asserts the result matches the original.
**Inputs:** Dict `{"_type": "Ping", "seq": 42}`, in-process PAIR pair.
**Expected output:** `msgpack.unpackb(msgpack.packb(data)) == data`.
**Acceptance command:** `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_ipc.py::test_roundtrip_via_pair_sockets -v` exits 0.

## test_send_before_connect_raises (worker)

**File:** `worker/tests/test_ipc.py`
**Context:** `ipc._sock` is `None` at module level before `connect()` is called. The guard check in `send_event()` must raise `RuntimeError` to prevent silent failures. Uses `_reset_ipc_state()` to ensure clean state.
**Tests:** Calls `_reset_ipc_state()`, then `ipc.send_event({})`, and asserts `RuntimeError` is raised.
**Inputs:** None (module-level `_sock` is `None`).
**Expected output:** `RuntimeError` is raised.
**Acceptance command:** `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_ipc.py::test_send_before_connect_raises -v` exits 0.

## test_recv_before_connect_raises (worker)

**File:** `worker/tests/test_ipc.py`
**Context:** `ipc._sock` is `None` at module level before `connect()` is called. The guard check in `recv_message()` must raise `RuntimeError` to prevent silent failures. Uses `_reset_ipc_state()` to ensure clean state.
**Tests:** Calls `_reset_ipc_state()`, then `ipc.recv_message()`, and asserts `RuntimeError` is raised.
**Inputs:** None (module-level `_sock` is `None`).
**Expected output:** `RuntimeError` is raised.
**Acceptance command:** `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_ipc.py::test_recv_before_connect_raises -v` exits 0.

## test_stress_test_1000_trips (anvilml-ipc)

**File:** `crates/anvilml-ipc/tests/stress_test.rs`
**Context:** Exercises the full Rust-to-Python IPC path: `RouterTransport` (Rust, ZeroMQ ROUTER) ↔ `ipc.py` DEALER (Python) over msgpack-serialised messages. Spawns a minimal Python echo worker (`worker/ipc_echo.py`) subprocess that connects to the bound ROUTER socket, echoes each `WorkerMessage::Ping` as a `WorkerEvent::Pong`, then sends 1000 Ping messages and asserts all 1000 Pong responses arrive with matching `seq` values in order. The test must complete within 30 seconds. No environment variables are mutated — the worker identity is hardcoded and the port is passed via CLI argument.
**Tests:** Binds a `RouterTransport::bind()`, spawns `worker/ipc_echo.py` from the worker venv with the bound port as a CLI argument, waits 500ms for the Python startup Ready message, then enters a loop sending `WorkerMessage::Ping { seq: 0..999 }` and asserting each `WorkerEvent::Pong { seq }` matches in order. Sends a Shutdown message on completion.
**Inputs:** 1000 `WorkerMessage::Ping { seq: 0..999 }` messages sent to worker identity `stress-test-worker`.
**Expected output:** All 1000 Pongs received with matching seq in order; test completes in < 30s; stdout contains "stress test passed: 1000/1000".
**Acceptance command:** `cargo test -p anvilml-ipc --features mock-hardware --test stress_test` exits 0.

## test_ipc_port (anvilml-worker)

**File:** `crates/anvilml-worker/tests/env_tests.rs`
**Context:** `build_worker_env()` is callable — no I/O, no subprocess, no network. Pure data transformation.
**Tests:** Constructs a `GpuDevice` with index=0 and `ServerConfig::default()`, calls `build_worker_env` with port=9000, and asserts `ANVILML_IPC_PORT` equals `"9000"`.
**Inputs:** port=9000, device.index=0, default config.
**Expected output:** `map["ANVILML_IPC_PORT"] == "9000"`.
**Acceptance command:** `cargo test -p anvilml-worker --features mock-hardware -- env::test_ipc_port` exits 0.

## test_worker_id (anvilml-worker)

**File:** `crates/anvilml-worker/tests/env_tests.rs`
**Context:** `build_worker_env()` produces `ANVILML_WORKER_ID` from the device index.
**Tests:** Constructs a `GpuDevice` with index=0, calls `build_worker_env`, and asserts `ANVILML_WORKER_ID` equals `"0"`.
**Inputs:** device.index=0, port=8488, default config.
**Expected output:** `map["ANVILML_WORKER_ID"] == "0"`.
**Acceptance command:** `cargo test -p anvilml-worker --features mock-hardware -- env::test_worker_id` exits 0.

## test_device_index (anvilml-worker)

**File:** `crates/anvilml-worker/tests/env_tests.rs`
**Context:** `build_worker_env()` produces `ANVILML_DEVICE_INDEX` from the device index.
**Tests:** Constructs a `GpuDevice` with index=0, calls `build_worker_env`, and asserts `ANVILML_DEVICE_INDEX` equals `"0"`.
**Inputs:** device.index=0, port=8488, default config.
**Expected output:** `map["ANVILML_DEVICE_INDEX"] == "0"`.
**Acceptance command:** `cargo test -p anvilml-worker --features mock-hardware -- env::test_device_index` exits 0.

## test_device_type_cuda (anvilml-worker)

**File:** `crates/anvilml-worker/tests/env_tests.rs`
**Context:** `build_worker_env()` maps `DeviceType::Cuda` to `"cuda"` via `device_type_label()`.
**Tests:** Constructs a `GpuDevice` with `DeviceType::Cuda`, calls `build_worker_env`, and asserts `ANVILML_DEVICE_TYPE` equals `"cuda"`.
**Inputs:** device_type=DeviceType::Cuda, port=8488, default config.
**Expected output:** `map["ANVILML_DEVICE_TYPE"] == "cuda"`.
**Acceptance command:** `cargo test -p anvilml-worker --features mock-hardware -- env::test_device_type_cuda` exits 0.

## test_device_type_rocm (anvilml-worker)

**File:** `crates/anvilml-worker/tests/env_tests.rs`
**Context:** `build_worker_env()` maps `DeviceType::Rocm` to `"rocm"` via `device_type_label()`.
**Tests:** Constructs a `GpuDevice` with `DeviceType::Rocm`, calls `build_worker_env`, and asserts `ANVILML_DEVICE_TYPE` equals `"rocm"`.
**Inputs:** device_type=DeviceType::Rocm, port=8488, default config.
**Expected output:** `map["ANVILML_DEVICE_TYPE"] == "rocm"`.
**Acceptance command:** `cargo test -p anvilml-worker --features mock-hardware -- env::test_device_type_rocm` exits 0.

## test_device_type_cpu (anvilml-worker)

**File:** `crates/anvilml-worker/tests/env_tests.rs`
**Context:** `build_worker_env()` maps `DeviceType::Cpu` to `"cpu"` via `device_type_label()`.
**Tests:** Constructs a `GpuDevice` with `DeviceType::Cpu`, calls `build_worker_env`, and asserts `ANVILML_DEVICE_TYPE` equals `"cpu"`.
**Inputs:** device_type=DeviceType::Cpu, port=8488, default config.
**Expected output:** `map["ANVILML_DEVICE_TYPE"] == "cpu"`.
**Acceptance command:** `cargo test -p anvilml-worker --features mock-hardware -- env::test_device_type_cpu` exits 0.

## test_log_level (anvilml-worker)

**File:** `crates/anvilml-worker/tests/env_tests.rs`
**Context:** `build_worker_env()` forwards `cfg.log_level` to `ANVILML_LOG_LEVEL`.
**Tests:** Constructs a `ServerConfig` with `log_level = "debug"`, calls `build_worker_env`, and asserts `ANVILML_LOG_LEVEL` equals `"debug"`.
**Inputs:** cfg.log_level="debug", device.index=0, port=8488.
**Expected output:** `map["ANVILML_LOG_LEVEL"] == "debug"`.
**Acceptance command:** `cargo test -p anvilml-worker --features mock-hardware -- env::test_log_level` exits 0.

## test_max_ipc_payload_mib (anvilml-worker)

**File:** `crates/anvilml-worker/tests/env_tests.rs`
**Context:** `build_worker_env()` forwards `cfg.max_ipc_payload_mib` to `ANVILML_MAX_IPC_PAYLOAD_MIB`.
**Tests:** Constructs a `ServerConfig` with `max_ipc_payload_mib = 512`, calls `build_worker_env`, and asserts `ANVILML_MAX_IPC_PAYLOAD_MIB` equals `"512"`.
**Inputs:** cfg.max_ipc_payload_mib=512, device.index=0, port=8488.
**Expected output:** `map["ANVILML_MAX_IPC_PAYLOAD_MIB"] == "512"`.
**Acceptance command:** `cargo test -p anvilml-worker --features mock-hardware -- env::test_max_ipc_payload_mib` exits 0.

## test_mock_hardware_flag (anvilml-worker)

**File:** `crates/anvilml-worker/tests/env_tests.rs`
**Context:** When compiled with `mock-hardware` feature, `build_worker_env()` injects `ANVILML_WORKER_MOCK=1`.
**Tests:** With `mock-hardware` feature enabled, calls `build_worker_env` and asserts `ANVILML_WORKER_MOCK` equals `"1"`.
**Inputs:** Any device, any config, any port. Feature `mock-hardware` enabled.
**Expected output:** `map["ANVILML_WORKER_MOCK"] == "1"`.
**Acceptance command:** `cargo test -p anvilml-worker --features mock-hardware -- env::test_mock_hardware_flag` exits 0.

## test_total_count (anvilml-worker)

**File:** `crates/anvilml-worker/tests/env_tests.rs`
**Context:** `build_worker_env()` produces exactly 6 env vars normally, 7 with `mock-hardware`.
**Tests:** Calls `build_worker_env` and asserts the HashMap length. With `mock-hardware`: 7. Without: 6.
**Inputs:** Any device, any config, any port.
**Expected output:** `map.len() == 7` (with mock-hardware) or `6` (without).
**Acceptance command:** `cargo test -p anvilml-worker --features mock-hardware -- env::test_total_count` exits 0.

## test_python_path_unix (anvilml-worker)

**File:** `crates/anvilml-worker/tests/spawn_tests.rs`
**Context:** `build_command()` constructs a `tokio::process::Command` with the venv Python interpreter. On Unix, the interpreter path is `{venv_path}/bin/python3`. This test verifies the program name and full path are correct by inspecting the Command's internal state via `get_program()` and `get_args()`.
**Tests:** Constructs `ServerConfig` with `venv_path = /test/venv`, calls `build_command()`, asserts `.get_program()` returns `python3`, and asserts the first argument contains `/test/venv/bin/python3`.
**Inputs:** `venv_path = /test/venv`, port=9000, device.index=0.
**Expected output:** `.get_program() == "python3"`, first arg contains `/test/venv/bin/python3`.
**Acceptance command:** `cargo test -p anvilml-worker --features mock-hardware -- spawn::test_python_path_unix` exits 0.

## test_python_path_windows (anvilml-worker)

**File:** `crates/anvilml-worker/tests/spawn_tests.rs`
**Context:** On Windows, the venv interpreter path is `{venv_path}\Scripts\python.exe`. This test is `#[cfg(windows)]` — only runs on Windows targets.
**Tests:** Constructs `ServerConfig` with `venv_path = C:\test\venv`, calls `build_command()`, asserts `.get_program()` returns `python.exe`, and asserts the first argument contains `Scripts\python.exe`.
**Inputs:** `venv_path = C:\test\venv`, port=9000, device.index=0.
**Expected output:** `.get_program() == "python.exe"`, first arg contains `Scripts\python.exe`.
**Acceptance command:** `cargo test -p anvilml-worker --features mock-hardware -- spawn::test_python_path_windows` exits 0.

## test_module_invocation (anvilml-worker)

**File:** `crates/anvilml-worker/tests/spawn_tests.rs`
**Context:** The worker is launched as a module (`-m worker.worker_main`) rather than a script path, matching the invocation convention already proven by the Python test suite (`[sys.executable, "-m", "worker.worker_main"]`). Running `worker_main.py` as a bare script breaks its `from worker.ipc import ...` package-relative import, since the script's own directory — not its parent — lands on `sys.path[0]`. This test verifies the command's two arguments are exactly `-m` and `worker.worker_main`.
**Tests:** Calls `build_command()` with default config, asserts `.get_args()` has exactly 2 elements, and asserts they are `"-m"` and `"worker.worker_main"`.
**Inputs:** Default config, port=9000, device.index=0.
**Expected output:** `.get_args() == ["-m", "worker.worker_main"]`.
**Acceptance command:** `cargo test -p anvilml-worker --features mock-hardware -- spawn::test_module_invocation` exits 0.

## test_env_injection (anvilml-worker)

**File:** `crates/anvilml-worker/tests/spawn_tests.rs`
**Context:** `build_command()` injects all environment variables from `build_worker_env()` into the subprocess. This test verifies the key env vars are present by inspecting the Command's environment via `get_env()`.
**Tests:** Calls `build_command()` with port=9000, device.index=0, and asserts `ANVILML_IPC_PORT` is `"9000"`, `ANVILML_DEVICE_INDEX` is `"0"`, and `ANVILML_WORKER_MOCK` is `"1"` (with mock-hardware feature).
**Inputs:** port=9000, device.index=0, default config.
**Expected output:** `get_env("ANVILML_IPC_PORT") == "9000"`, `get_env("ANVILML_DEVICE_INDEX") == "0"`, `get_env("ANVILML_WORKER_MOCK") == "1"` (with mock-hardware).
**Acceptance command:** `cargo test -p anvilml-worker --features mock-hardware -- spawn::test_env_injection` exits 0.

## test_stdin_not_piped (anvilml-worker)

**File:** `crates/anvilml-worker/tests/spawn_tests.rs`
**Context:** Stdin is left as the default (`Inherit`) because the Python worker is non-interactive. This test verifies that stdin is not piped.
**Tests:** Calls `build_command()` and asserts `.get_stdin()` returns `Stdio::Inherit`.
**Inputs:** Default config, port=9000, device.index=0.
**Expected output:** `.get_stdin() == Stdio::Inherit`.
**Acceptance command:** `cargo test -p anvilml-worker --features mock-hardware -- spawn::test_stdin_not_piped` exits 0.

## test_stdout_piped (anvilml-worker)

**File:** `crates/anvilml-worker/tests/spawn_tests.rs`
**Context:** Stdout is piped so the supervisor can capture worker logs and surface them through the server's log channel.
**Tests:** Calls `build_command()` and asserts `.get_stdout()` returns `Stdio::Piped`.
**Inputs:** Default config, port=9000, device.index=0.
**Expected output:** `.get_stdout() == Stdio::Piped`.
**Acceptance command:** `cargo test -p anvilml-worker --features mock-hardware -- spawn::test_stdout_piped` exits 0.

## test_stderr_piped (anvilml-worker)

**File:** `crates/anvilml-worker/tests/spawn_tests.rs`
**Context:** Stderr is piped so the supervisor captures worker error output for log aggregation.
**Tests:** Calls `build_command()` and asserts `.get_stderr()` returns `Stdio::Piped`.
**Inputs:** Default config, port=9000, device.index=0.
**Expected output:** `.get_stderr() == Stdio::Piped`.
**Acceptance command:** `cargo test -p anvilml-worker --features mock-hardware -- spawn::test_stderr_piped` exits 0.

## test_writer_sends_message (anvilml-worker)

**File:** `crates/anvilml-worker/tests/bridge_tests.rs`
**Context:** The bridge writer task receives messages from an `mpsc::Receiver` and forwards them to the `RouterTransport`. Uses a real ZeroMQ ROUTER socket and DEALER client to exercise the actual routing path. The writer terminates when the mpsc sender is dropped.
**Tests:** Binds a `RouterTransport`, connects a DEALER socket, discovers the DEALER's identity via a probe message, spawns the bridge writer, sends `WorkerMessage::Ping { seq: 1 }` through the mpsc channel, drops the sender, reads from the DEALER side, and verifies the decoded message matches.
**Inputs:** `WorkerMessage::Ping { seq: 1 }` sent through mpsc channel.
**Expected output:** DEALER receives a single-frame message that decodes to `WorkerMessage::Ping { seq: 1 }`; writer task exits cleanly.
**Acceptance command:** `cargo test -p anvilml-worker --features mock-hardware -- bridge_tests::test_writer_sends_message` exits 0.

## test_handle_drops_cleanly (anvilml-worker)

**File:** `crates/anvilml-worker/tests/bridge_tests.rs`
**Context:** Dropping the bridge writer's handle does not panic. The writer exits on its own once its mpsc sender is dropped — there is no reader handle here anymore, since `bridge::start` no longer reads from the transport (see `crate::demux`).
**Tests:** Binds a `RouterTransport`, spawns the bridge writer with a dummy mpsc channel, drops the sender, drops the handle, and asserts no panic.
**Inputs:** None (uses `RouterTransport::bind()` and a dummy channel).
**Expected output:** The handle resolves without panic.
**Acceptance command:** `cargo test -p anvilml-worker --features mock-hardware -- bridge_tests::test_handle_drops_cleanly` exits 0.

## test_timeout_fires (anvilml-worker)

**File:** `crates/anvilml-worker/tests/keepalive_tests.rs`
**Context:** The keepalive heartbeat loop sends a Ping and waits for a matching Pong. When no Pong arrives within `pong_timeout`, the `on_timeout` callback is invoked. Uses in-memory channels (mpsc + broadcast) — no ZeroMQ transport needed since the heartbeat logic is purely about sequence matching and deadline timing.
**Tests:** Creates a keepalive with `pong_timeout=500ms`, `ping_interval=100ms`, and a shared `AtomicUsize` counter. Spawns the keepalive, waits for the counter to increment (indicating timeout fired), and asserts it happens within 1 second.
**Inputs:** `pong_timeout=500ms`, `ping_interval=100ms`, no Pong events sent.
**Expected output:** `on_timeout` callback fires within 1 second (pong_timeout + 100ms buffer).
**Acceptance command:** `cargo test -p anvilml-worker --features mock-hardware -- keepalive_tests::test_timeout_fires` exits 0.

## test_pong_resets_deadline (anvilml-worker)

**File:** `crates/anvilml-worker/tests/keepalive_tests.rs`
**Context:** When a matching Pong is received for each Ping, the deadline is reset and the timeout callback is never invoked. This test verifies the pong-matching logic across multiple ping cycles.
**Tests:** Creates a keepalive with `pong_timeout=500ms`, `ping_interval=100ms`, and an `AtomicUsize` counter. Spawns the keepalive, then in a loop receives each Ping and sends back a matching Pong. Waits 1 second and asserts the counter is still 0.
**Inputs:** `pong_timeout=500ms`, `ping_interval=100ms`, Pong sent for each Ping.
**Expected output:** `on_timeout` never fires — counter remains 0 after 1 second.
**Acceptance command:** `cargo test -p anvilml-worker --features mock-hardware -- keepalive_tests::test_pong_resets_deadline` exits 0.

## test_seq_increments (anvilml-worker)

**File:** `crates/anvilml-worker/tests/keepalive_tests.rs`
**Context:** The sequence number increments monotonically across ping sends. Each ping cycle starts with `seq` incremented from the previous cycle. This test verifies the sequence number progression by collecting pings from the mpsc channel.
**Tests:** Creates a keepalive with `ping_interval=100ms`, `pong_timeout=1000ms`, and collects pings from the mpsc receiver for 2 seconds. Asserts at least 5 pings received, first seq is 1, and all seq values are strictly increasing.
**Inputs:** `ping_interval=100ms`, `pong_timeout=1000ms`, no Pong events sent.
**Expected output:** Sequence numbers are strictly increasing (1, 2, 3, ...) with at least 5 values in 2 seconds.
**Acceptance command:** `cargo test -p anvilml-worker --features mock-hardware -- keepalive_tests::test_seq_increments` exits 0.

## test_no_ping_before_ready (anvilml-worker)

**File:** `crates/anvilml-worker/tests/keepalive_tests.rs`
**Context:** `keepalive::start()` takes a `ready_rx: oneshot::Receiver<()>` and awaits it once before entering the ping loop, so no `Ping` is ever sent to a worker that hasn't finished initializing — closes a defect found via a live `cargo run` trace where pings were sent before the corresponding `Ready` event was processed.
**Tests:** Spawns a keepalive with an unfired `ready_tx`/`ready_rx` pair, waits briefly, asserts no `Ping` has arrived on the mpsc receiver, then fires `ready_tx` and asserts a `Ping` arrives promptly afterward.
**Inputs:** `ready_rx` not yet resolved, then resolved via `ready_tx.send(())`.
**Expected output:** No `Ping` sent while `ready_rx` is unresolved; a `Ping` is sent shortly after `ready_tx` fires.
**Acceptance command:** `cargo test -p anvilml-worker --features mock-hardware -- keepalive_tests::test_no_ping_before_ready` exits 0.

## test_dropped_ready_tx_skips_heartbeat_entirely (anvilml-worker)

**File:** `crates/anvilml-worker/tests/keepalive_tests.rs`
**Context:** If the `ready_tx` sender is dropped without ever firing — the worker hit the ready timeout or exited before reporting `Ready` — `ready_rx.await` resolves to `Err`, and the keepalive task must exit immediately rather than entering the ping loop at all, since there is no live worker left to heartbeat.
**Tests:** Spawns a keepalive with a `ready_rx` whose matching `ready_tx` is dropped immediately (never sent), and asserts the task exits on its own without ever sending a `Ping`.
**Inputs:** `ready_tx` dropped without calling `.send(())`.
**Expected output:** The keepalive task returns immediately; the mpsc channel's `recv()` resolves to `Ok(None)` (sender side closed), not a timeout error — no `Ping` is ever sent.
**Acceptance command:** `cargo test -p anvilml-worker --features mock-hardware -- keepalive_tests::test_dropped_ready_tx_skips_heartbeat_entirely` exits 0.

## test_spawn_reaches_idle (anvilml-worker)

**File:** `crates/anvilml-worker/tests/managed_tests.rs`
**Context:** The `ManagedWorker` state machine transitions from `Initializing` to `Idle` on receipt of a `Ready` event. This is the primary synchronization point between the Rust supervisor and the Python worker. The test creates a worker with pre-built channels (bypassing subprocess spawning) and sends a `Ready` event through the broadcast channel.
**Tests:** Creates a `ManagedWorker` in `Initializing` status via `new()`, clones the `Arc<RwLock>` for post-run status check, sends a `Ready` event, spawns `run()`, waits for completion, and verifies status is `Idle`.
**Inputs:** `Ready` event with `worker_id="test-worker-ready"`, `device_name="test-device"`, `torch_version="2.4.0"`.
**Expected output:** Status transitions to `Idle` after processing the `Ready` event.
**Acceptance command:** `cargo test -p anvilml-worker --features mock-hardware -- managed_tests::test_spawn_reaches_idle` exits 0.

## test_ready_timeout_dead (anvilml-worker)

**File:** `crates/anvilml-worker/tests/managed_tests.rs`
**Context:** The design doc mandates a 60-second timeout for the `Ready` event. If no `Ready` is received within this window, the worker is considered unresponsive and transitions to `Dead`. This test sends a `Ready` event within 1 second so the timeout is cancelled early, verifying that the `Ready` event causes the `Initializing` → `Idle` transition (proving the timeout mechanism is in place).
**Tests:** Creates a `ManagedWorker` in `Initializing` status, sends a `Ready` event, spawns `run()`, uses a 70-second outer timeout as a safety net, and verifies status is `Idle`.
**Inputs:** `Ready` event sent within 1 second.
**Expected output:** Status transitions to `Idle` (not `Dead`), proving the `Ready` event cancels the timeout.
**Acceptance command:** `cargo test -p anvilml-worker --features mock-hardware -- managed_tests::test_ready_timeout_dead` exits 0.

## test_dying_event_transitions_dead (anvilml-worker)

**File:** `crates/anvilml-worker/tests/managed_tests.rs`
**Context:** A `Dying` event received while the worker is in `Idle` state causes an immediate transition to `Dead`. This verifies the graceful shutdown path.
**Tests:** Creates a `ManagedWorker` in `Idle` status, sends a `Dying` event with `reason="SIGTERM"`, spawns `run()`, and verifies status becomes `Dead`.
**Inputs:** `Dying` event with `reason="SIGTERM"`.
**Expected output:** Status transitions to `Dead`.
**Acceptance command:** `cargo test -p anvilml-worker --features mock-hardware -- managed_tests::test_dying_event_transitions_dead` exits 0.

## test_keepalive_timeout_sets_dead (anvilml-worker)

**File:** `crates/anvilml-worker/tests/managed_tests.rs`
**Context:** The keepalive heartbeat sends `Ping` messages at 30-second intervals and waits for `Pong` responses within a 10-second timeout. If no pong is received, the `on_timeout` callback is invoked, which transitions the worker status to `Dead`. This test creates a worker without sending any pongs, so the timeout fires and the status transitions to `Dead`.
**Tests:** Creates a `ManagedWorker` with an actual keepalive task (`pong_timeout=10s`, `ping_interval=30s`) and a callback that records its invocation. Spawns `run()`, waits 15 seconds, and verifies both the callback fired and status is `Dead`.
**Inputs:** No `Pong` events sent; keepalive runs with default intervals.
**Expected output:** `on_timeout` callback fires and status transitions to `Dead` within 15 seconds.
**Acceptance command:** `cargo test -p anvilml-worker --features mock-hardware -- managed_tests::test_keepalive_timeout_sets_dead` exits 0.

## test_status_transitions_idle_to_busy_to_idle (anvilml-worker)

**File:** `crates/anvilml-worker/tests/managed_tests.rs`
**Context:** The worker transitions from `Idle` to `Busy` when a job is dispatched, and back to `Idle` when the job completes, fails, or is cancelled. This test verifies the `Completed` → `Idle` transition.
**Tests:** Creates a `ManagedWorker` in `Idle` status, sends a `Ready` event, manually transitions to `Busy` (simulating job dispatch), sends a `Completed` event, and verifies status returns to `Idle`.
**Inputs:** `Ready` event, manual `Busy` transition, `Completed` event with `elapsed_ms=5000`.
**Expected output:** Status transitions `Idle` → `Busy` (manual) → `Idle` (on `Completed`).
**Acceptance command:** `cargo test -p anvilml-worker --features mock-hardware -- managed_tests::test_status_transitions_idle_to_busy_to_idle` exits 0.

## test_shutdown_cleans_up_handles (anvilml-worker)

**File:** `crates/anvilml-worker/tests/managed_tests.rs`
**Context:** Shutdown is driven entirely through `run()`'s own shutdown arm — firing a `oneshot::Sender<()>` the caller holds, not a separate `shutdown()` method — and must complete its full sequence (stop the keepalive promptly, abort its handle, send the `Shutdown` IPC message, await the bridge writer with a bounded timeout, deregister any route, then return) without panicking, for a worker with real bridge writer and keepalive task handles.
**Tests:** Creates a `ManagedWorker` with a real bridge writer task and a real keepalive task (its `ready_tx` fired immediately, since this test exercises the shutdown sequence itself, not the Ready gate), spawns `run()` with a real `oneshot` shutdown channel, waits briefly for `run()` to enter its select loop, fires the shutdown sender, and asserts `run()` returns within a bounded timeout without panicking.
**Inputs:** Worker with active bridge writer and keepalive handles, `ready_tx` pre-fired, shutdown signalled via `oneshot::Sender::send(())`.
**Expected output:** `run()` completes its shutdown sequence and returns within 10 seconds (the sequence's own internal bounds are roughly 7 seconds: a 2-second bridge-writer-await timeout plus up to 5 seconds for child teardown, though `child` is `None` in this test so that step is instant); the returned `JoinHandle`'s result is `Ok`, not a panic.
**Acceptance command:** `cargo test -p anvilml-worker --features mock-hardware -- managed_tests::test_shutdown_cleans_up_handles` exits 0.

## test_spawned_task_updates_status (anvilml-worker)

**File:** `crates/anvilml-worker/tests/managed_tests.rs`
**Context:** The keepalive's `on_timeout` callback is synchronous, but updating worker status requires an async write-lock acquisition — the callback works around this by spawning a separate async task that does the actual status update. This is a regression test for that spawned-task mechanism, relocated from a `#[cfg(test)]` module formerly embedded directly in `managed.rs` (production source files contain no test code per coding standards).
**Tests:** Constructs an `on_timeout` callback that records its own invocation in an `AtomicBool` and spawns a task that sets a shared `RwLock<WorkerStatus>` to `Dead` via a weak reference, invokes the callback directly, awaits an unrelated 2-second task to give the spawned task time to run, then asserts both that the callback fired and that the status is `Dead`.
**Inputs:** A weak reference to a `RwLock<WorkerStatus>` initialised to `Idle`.
**Expected output:** The callback's `AtomicBool` flag is `true`; the status is `Dead`.
**Acceptance command:** `cargo test -p anvilml-worker --features mock-hardware -- managed_tests::test_spawned_task_updates_status` exits 0.

## test_run_shutdown_deregisters_route (anvilml-worker)

**File:** `crates/anvilml-worker/tests/managed_tests.rs`
**Context:** `run()`'s shutdown arm deregisters the worker's route from the shared demux routing table when `routes`/`route_key` are populated — this verifies that `run()` itself calls `demux::deregister` as part of its shutdown sequence (see `demux_tests::test_deregister_removes_route` for `deregister()`'s own isolated behaviour). Constructed via `ManagedWorker::new()` with `routes`/`route_key` supplied directly rather than through `ManagedWorker::spawn()`, since that would also require a real Python subprocess to launch — this is still a faithful test of the deregistration step, which only depends on the two fields being populated, not on how they got that way.
**Tests:** Pre-registers a route in a shared `RouteTable`, constructs a worker with that table and the matching key, spawns `run()` with a real shutdown channel, fires shutdown, awaits completion within a bounded timeout, and asserts the route is no longer present in the table.
**Inputs:** A `RouteTable` with one pre-registered entry; a worker constructed with `routes`/`route_key` pointing at that same entry.
**Expected output:** `run()` completes its shutdown sequence within 10 seconds; the route is absent from the table afterward.
**Acceptance command:** `cargo test -p anvilml-worker --features mock-hardware -- managed_tests::test_run_shutdown_deregisters_route` exits 0.

## test_run_ready_event_releases_keepalive_gate (anvilml-worker)

**File:** `crates/anvilml-worker/tests/managed_tests.rs`
**Context:** Verifies, end-to-end through `run()` itself rather than `keepalive::start()` in isolation (see `keepalive_tests::test_no_ping_before_ready` for the unit-level check), that `run()`'s `Initializing → Idle` transition actually fires the worker's `ready_tx`, releasing a real keepalive task's start gate.
**Tests:** Constructs a worker `Initializing`, with a real keepalive task and an unfired `ready_tx`, spawns `run()`, asserts no `Ping` arrives on the shared mpsc channel while still `Initializing`, sends a `Ready` event through the broadcast channel, and asserts a `Ping` arrives promptly afterward.
**Inputs:** Worker in `Initializing` state with a real keepalive task; a `Ready` event sent after an initial no-ping assertion window.
**Expected output:** No `Ping` sent before the `Ready` event is processed; a `Ping` is sent within 300ms after it is.
**Acceptance command:** `cargo test -p anvilml-worker --features mock-hardware -- managed_tests::test_run_ready_event_releases_keepalive_gate` exits 0.

## test_spawn_all_workers_idle (anvilml-worker)

**File:** `crates/anvilml-worker/tests/pool_tests.rs`
**Context:** `WorkerPool` manages a collection of `ManagedWorker` instances. This test verifies that constructing a pool with N mock workers results in N workers all reporting `Idle` status. Uses `ManagedWorker::new()` with pre-built channels (bypassing subprocess spawning).
**Tests:** Creates 3 mock workers in `Idle` status, constructs a `WorkerPool` via the test constructor, calls `get_worker_infos()`, and verifies all 3 workers report `Idle` with correct IDs, device names, and device indices.
**Inputs:** 3 `ManagedWorker` instances in `Idle` status, `RouterTransport::bind()`, `EventBroadcaster::new()`.
**Expected output:** `get_worker_infos()` returns 3 workers, all with `status: Idle`, correct `id` and `device_name` fields, `current_job_id: None`, `vram_used_mib: None`.
**Acceptance command:** `cargo test -p anvilml-worker --features mock-hardware -- pool_tests::test_spawn_all_workers_idle` exits 0.

## test_broadcaster_returns_reference (anvilml-worker)

**File:** `crates/anvilml-worker/tests/pool_tests.rs`
**Context:** `WorkerPool::broadcaster()` must return a reference to the same `Arc<EventBroadcaster>` that was passed during construction. This verifies the pool stores and exposes the broadcaster correctly.
**Tests:** Constructs a pool with a known `EventBroadcaster` Arc, calls `broadcaster()`, and verifies pointer equality with the original Arc.
**Inputs:** 1 `ManagedWorker` in `Idle` status, `RouterTransport::bind()`, `EventBroadcaster::new()`.
**Expected output:** `Arc::ptr_eq(pool.broadcaster(), &original)` — the returned reference is the same Arc.
**Acceptance command:** `cargo test -p anvilml-worker --features mock-hardware -- pool_tests::test_broadcaster_returns_reference` exits 0.

## test_pool_broadcasts_status_change (anvilml-worker)

**File:** `crates/anvilml-worker/tests/pool_tests.rs`
**Context:** The pool's background monitoring task must detect status changes and broadcast `WsEvent::WorkerStatusChanged`. This test verifies the broadcast mechanism by manually spawning a monitoring task and checking for the event.
**Tests:** Creates a pool with one worker in `Idle` status, spawns a monitoring task (100ms poll interval), sets the worker's status to `Busy` via the RwLock, waits for detection, and verifies the broadcaster received a `WorkerStatusChanged` event with correct fields.
**Inputs:** 1 `ManagedWorker` in `Idle` status, manually set to `Busy` via RwLock.
**Expected output:** Broadcaster received `WsEvent::WorkerStatusChanged{worker_id: "test-worker-broadcast", status: Busy, device_index: 0}`.
**Acceptance command:** `cargo test -p anvilml-worker --features mock-hardware -- pool_tests::test_pool_broadcasts_status_change` exits 0.

## test_reexport_worker_pool (anvilml-worker)

**File:** `crates/anvilml-worker/tests/pool_tests.rs`
**Context:** `pub use pool::WorkerPool;` in `lib.rs` must make `WorkerPool` accessible via `anvilml_worker::WorkerPool`. This is a compile-time check that verifies the re-export.
**Tests:** Constructs a `WorkerPool` using the re-exported type name `anvilml_worker::WorkerPool`. If it compiles, the re-export is correct.
**Inputs:** 1 `ManagedWorker` in `Idle` status, `RouterTransport::bind()`, `EventBroadcaster::new()`.
**Expected output:** Compiles successfully — no compilation error.
**Acceptance command:** `cargo test -p anvilml-worker --features mock-hardware -- pool_tests::test_reexport_worker_pool` exits 0.

## test_shutdown_all_completes_against_inert_handles (anvilml-worker)

**File:** `crates/anvilml-worker/tests/pool_tests.rs`
**Context:** `WorkerPool::shutdown_all()` must return promptly even for a worker whose `run()` task was never actually started (an "inert" handle) — without a real run loop to drive shutdown, the only thing this test can verify is that `shutdown_all()` itself doesn't hang waiting on something that will never resolve, which is exactly the regression a bounded internal timeout is meant to prevent.
**Tests:** Constructs a pool with one worker built via the `make_test_worker` helper (no real `run()` task spawned), calls `shutdown_all()` wrapped in a 15-second outer timeout, asserts it completes within that bound, then calls `get_worker_infos()` and asserts the pool's worker list is empty afterward.
**Inputs:** 1 `ManagedWorker` in `Idle` status with no `run()` task ever spawned for it.
**Expected output:** `shutdown_all()` resolves within 15 seconds; `get_worker_infos()` returns an empty list afterward (the pool's worker list is drained via `mem::take` and never repopulated).
**Acceptance command:** `cargo test -p anvilml-worker --features mock-hardware -- pool_tests::test_shutdown_all_completes_against_inert_handles` exits 0.

## test_list_workers_returns_empty_when_no_pool (anvilml-server)

**File:** `crates/anvilml-server/tests/workers_tests.rs`
**Context:** The `GET /v1/workers` handler returns an empty JSON array when `AppState.workers` is `None`. Exercises the production `build_router` path via `Router::oneshot` without a live TCP listener.
**Tests:** Builds the router with `AppState::new("test-version")` (which sets `workers = None`), sends GET `/v1/workers`, asserts HTTP 200, parses the JSON response, and verifies the body is an empty JSON array `[]`.
**Inputs:** GET `/v1/workers`, `AppState::new("test-version")`.
**Expected output:** HTTP 200 with JSON body `[]`.
**Acceptance command:** `cargo test -p anvilml-server --test workers_tests -- test_list_workers_returns_empty_when_no_pool` exits 0.

## test_list_workers_returns_pool_data (anvilml-server)

**File:** `crates/anvilml-server/tests/workers_tests.rs`
**Context:** The `GET /v1/workers` handler returns worker info from the `WorkerPool` when `AppState.workers` is `Some(pool)`. Exercises the production `build_router` path via `Router::oneshot`. Uses a mock `ManagedWorker` in `Idle` status to avoid spawning a real Python subprocess.
**Tests:** Creates a `WorkerPool` with one mock `ManagedWorker` (status=`Idle`, id=`"worker-0"`, device=`"mock-device"`), builds `AppState` with `new_with_hardware` including the pool, sends GET `/v1/workers`, asserts HTTP 200, parses the JSON response, and verifies the body is a JSON array with one entry containing `status: "idle"` and `id: "worker-0"`.
**Inputs:** GET `/v1/workers`, `AppState::new_with_hardware(...)` with a mock pool containing one worker.
**Expected output:** HTTP 200 with JSON array of length 1, first entry has `status="idle"` and `id="worker-0"`.
**Acceptance command:** `cargo test -p anvilml-server --test workers_tests -- test_list_workers_returns_pool_data` exits 0.

## test_stats_tick_broadcasts_system_stats (anvilml-server)

**File:** `crates/anvilml-server/tests/stats_tick_tests.rs`
**Context:** The `stats_tick::start()` function now takes `Arc<WorkerPool>` instead of `Arc<EventBroadcaster>`. This test verifies the tick task broadcasts a `SystemStats` event within 10 seconds. Uses a minimal `WorkerPool` with zero workers (created via `test_pool()` helper that binds a `RouterTransport` on port 0 and creates a fresh `EventBroadcaster`).
**Tests:** Creates a minimal `WorkerPool`, subscribes to its broadcaster, calls `start()`, then waits up to 10 seconds for a `SystemStats` event. The event must have the correct `SystemStats` variant and field types.
**Inputs:** Minimal `WorkerPool` (0 workers, bound transport, fresh broadcaster).
**Expected output:** `SystemStats` event received within 10 seconds.
**Acceptance command:** `cargo test -p anvilml-server --test stats_tick_tests -- test_stats_tick_broadcasts_system_stats` exits 0.

## test_stats_tick_cpu_pct_is_finite (anvilml-server)

**File:** `crates/anvilml-server/tests/stats_tick_tests.rs`
**Context:** The `stats_tick::start()` function uses `Arc<WorkerPool>` as its parameter. This test verifies the CPU percentage value in a `SystemStats` event is a finite `f32` (not NaN or infinity).
**Tests:** Creates a minimal `WorkerPool`, subscribes to its broadcaster, calls `start()`, waits for one event, and asserts `cpu_pct.is_finite() == true`.
**Inputs:** Minimal `WorkerPool` (0 workers).
**Expected output:** `cpu_pct.is_finite() == true`.
**Acceptance command:** `cargo test -p anvilml-server --test stats_tick_tests -- test_stats_tick_cpu_pct_is_finite` exits 0.

## test_stats_tick_ram_used_mib_is_non_negative (anvilml-server)

**File:** `crates/anvilml-server/tests/stats_tick_tests.rs`
**Context:** The `stats_tick::start()` function uses `Arc<WorkerPool>` as its parameter. This test verifies the RAM usage value in a `SystemStats` event is always non-negative.
**Tests:** Creates a minimal `WorkerPool`, subscribes to its broadcaster, calls `start()`, waits for one event, and asserts `ram_used_mib > 0`.
**Inputs:** Minimal `WorkerPool` (0 workers).
**Expected output:** `ram_used_mib > 0`.
**Acceptance command:** `cargo test -p anvilml-server --test stats_tick_tests -- test_stats_tick_ram_used_mib_is_non_negative` exits 0.

## test_should_respawn_max_attempts_exceeded (anvilml-worker)

**File:** `crates/anvilml-worker/tests/respawn_tests.rs`
**Context:** `RespawnPolicy::should_respawn()` returns `false` when `crash_count >= max_attempts`. This is the maximum-attempt guard that prevents infinite respawn loops. The method now takes `crash_count` by mutable reference and owns the window-reset contract.
**Tests:** Constructs `RespawnPolicy { max_attempts: 3, ... }`, calls `should_respawn(&mut count, Instant::now())` with `count = 3`, and asserts `false`.
**Inputs:** `policy.max_attempts = 3`, `crash_count = 3` (mutable ref), `last_crash = Instant::now()`.
**Expected output:** `false` — the worker should not be respawned; `crash_count` unchanged at 3.
**Acceptance command:** `cargo test -p anvilml-worker --features mock-hardware --test respawn_tests test_should_respawn_max_attempts_exceeded` exits 0.

## test_should_respawn_within_window (anvilml-worker)

**File:** `crates/anvilml-worker/tests/respawn_tests.rs`
**Context:** `RespawnPolicy::should_respawn()` returns `true` when `crash_count < max_attempts` and the crash window has not expired. The window is `last_crash + window_s > now`. The method now takes `crash_count` by mutable reference and increments it on each allow.
**Tests:** Constructs `RespawnPolicy { max_attempts: 5, window_s: 60, ... }`, calls `should_respawn(&mut count, Instant::now() - Duration::from_secs(30))` with `count = 2`, asserts `true`, and asserts `count == 3`.
**Inputs:** `policy.max_attempts = 5`, `policy.window_s = 60`, `crash_count = 2` (mutable ref), `last_crash = 30 seconds ago`.
**Expected output:** `true` — the worker should be respawned; `crash_count` incremented to 3.
**Acceptance command:** `cargo test -p anvilml-worker --features mock-hardware --test respawn_tests test_should_respawn_within_window` exits 0.

## test_should_respawn_window_reset (anvilml-worker)

**File:** `crates/anvilml-worker/tests/respawn_tests.rs`
**Context:** `RespawnPolicy::should_respawn()` resets `crash_count` to `0` when the window has expired, then increments it to `1` and returns `true`. This test asserts both the boolean return value and the counter mutation — the old buggy implementation returned `true` but never mutated the count.
**Tests:** Constructs `RespawnPolicy { max_attempts: 5, window_s: 10, ... }`, calls `should_respawn(&mut count, Instant::now() - Duration::from_secs(15))` with `count = 4`, asserts `true`, and asserts `count == 1` (reset to 0 by window expiry, then incremented to 1).
**Inputs:** `policy.max_attempts = 5`, `policy.window_s = 10`, `crash_count = 4` (mutable ref), `last_crash = 15 seconds ago`.
**Expected output:** `true`; `crash_count == 1` (reset to 0 by window expiry, then incremented to 1 by the allow step).
**Acceptance command:** `cargo test -p anvilml-worker --features mock-hardware --test respawn_tests test_should_respawn_window_reset` exits 0.

## test_next_delay_ms_exponential_backoff_and_cap (anvilml-worker)

**File:** `crates/anvilml-worker/tests/respawn_tests.rs`
**Context:** `RespawnPolicy::next_delay_ms()` computes exponential backoff (`delay_ms * 2^attempt`) with a 30-second cap. Verifies the sequence grows correctly and caps at the right attempt.
**Tests:** Constructs `RespawnPolicy { delay_ms: 1000, ... }`, calls `next_delay_ms()` for attempts 0–5 and 10, and asserts the expected values including the cap at attempt 5 (30,000 ms).
**Inputs:** `policy.delay_ms = 1000`, attempts 0, 1, 2, 3, 4, 5, 10.
**Expected output:** 1000, 2000, 4000, 8000, 16000, 30000 (capped), 30000 (capped).
**Acceptance command:** `cargo test -p anvilml-worker --test respawn_tests test_next_delay_ms_exponential_backoff_and_cap` exits 0.

## test_run_processes_multiple_sequential_events (anvilml-worker)

**File:** `crates/anvilml-worker/tests/managed_tests.rs`
**Context:** `ManagedWorker::run()` has been refactored into a continuous loop (P901-A1) that processes events until the broadcast channel closes. Existing tests send one event then `drop(event_tx)` — a pattern compatible with both the old one-shot `select!` and the new loop. This test sends two sequential events on a single `run()` call to prove the loop is real.
**Tests:** Creates a worker in `Initializing` state, spawns `run()`, sends a `Ready` event (triggering `Initializing → Idle`), manually sets status to `Busy`, sends a `Completed` event (triggering `Busy → Idle`), and asserts the final status is `Idle`. If `run()` exited after the first event, the second event would never be received and the status would remain `Busy`.
**Inputs:** Worker in `Initializing` state, `Ready` event followed by `Completed` event (sent sequentially on the same `event_tx`), manual `Busy` status set between events.
**Expected output:** Status transitions: `Initializing → Idle` (on Ready) → `Busy` (manual) → `Idle` (on Completed). Final status is `Idle`.
**Acceptance command:** `cargo test -p anvilml-worker --features mock-hardware -- test_run_processes_multiple_sequential_events` exits 0.

## test_child_exit_transitions_dead (anvilml-worker)

**File:** `crates/anvilml-worker/tests/managed_tests.rs`
**Context:** The `ManagedWorker::run()` loop has a `child.wait()` arm in its `tokio::select!` block that detects unexpected subprocess exit. This test creates a real child process that sleeps briefly then exits and passes it to `ManagedWorker::new()`. The run loop's `child.wait()` arm fires when the child exits, transitioning the status to `Dead`. The spawn command is `cfg`-gated by platform — `sh` is not on `PATH` by default on Windows — but the test's assertions are identical either way, since it never checks the child's exit code, only that the status transition fires once the process exits at all.
**Tests:** A real child subprocess is spawned via `tokio::process::Command` (`sh -c "sleep 0.5 && exit 1"` on non-Windows; `ping -n 1 -w 500 127.0.0.1` on Windows, a dependency-free sleep substitute since `ping.exe` ships with every Windows install), passed to `ManagedWorker::new()` with `Initializing` status and no demux table (so no `Ready` event can ever arrive), and the run loop is spawned. The test polls the status every 100ms until it becomes `Dead` (or times out after 5 seconds). Asserts the final status is `Dead`.
**Inputs:** A child process that exits after ~0.5 seconds; `ManagedWorker` in `Initializing` state, no bridge reader or demux task running.
**Expected output:** Status transitions from `Initializing` to `Dead` within 5 seconds — the `child.wait()` arm fires and sets the status. This test does not assert on any broadcast event; only the status transition is checked.
**Acceptance command:** `cargo test -p anvilml-worker --features mock-hardware -- managed_tests::test_child_exit_transitions_dead` exits 0.

## test_demux_dispatches_event_to_registered_route (anvilml-worker)

**File:** `crates/anvilml-worker/tests/demux_tests.rs`
**Context:** The demux task is the single reader of the shared `RouterTransport`; it looks up each received event's wire identity in a routing table and forwards it to that route's `broadcast::Sender`. Uses a real ZeroMQ ROUTER socket and DEALER client. Replaces what `test_reader_broadcasts_event` formerly verified in `bridge_tests.rs`, against `bridge::start`'s reader — that reader no longer exists; this test exercises `demux::start` instead.
**Tests:** Binds a `RouterTransport`, connects a DEALER socket, discovers the DEALER's identity, registers a route for that identity, starts the demux task, sends `WorkerEvent::Pong { seq: 42 }` from the DEALER side, reads from the route's broadcast channel, and verifies the event matches and the broadcast worker_id is the wire identity (not the display label).
**Inputs:** `WorkerEvent::Pong { seq: 42 }` sent from DEALER to ROUTER, one pre-registered route.
**Expected output:** Broadcast channel receives `(wire_identity, WorkerEvent::Pong { seq: 42 })`.
**Acceptance command:** `cargo test -p anvilml-worker --features mock-hardware -- demux_tests::test_demux_dispatches_event_to_registered_route` exits 0.

## test_demux_drops_event_for_unregistered_identity (anvilml-worker)

**File:** `crates/anvilml-worker/tests/demux_tests.rs`
**Context:** Regression test for the cross-worker misrouting bug the demux task exists to fix: before a single shared reader existed, multiple per-worker readers raced the same ROUTER socket and could receive each other's events. With one reader and an explicit routing table, an identity with no registered route has nowhere to go and must be dropped, not guessed at or delivered to the wrong route.
**Tests:** Binds a `RouterTransport`, connects a DEALER socket, registers a route under an unrelated identity (not the DEALER's own), starts the demux task, sends a message from the DEALER, and asserts no panic or hang.
**Inputs:** A message from an identity with no matching entry in the routing table.
**Expected output:** The event is dropped (logged at WARN); no panic, no delivery to the unrelated registered route.
**Acceptance command:** `cargo test -p anvilml-worker --features mock-hardware -- demux_tests::test_demux_drops_event_for_unregistered_identity` exits 0.

## test_demux_survives_undecodable_payload (anvilml-worker)

**File:** `crates/anvilml-worker/tests/demux_tests.rs`
**Context:** Regression test for the bug described in `anvilml_ipc::RecvError`'s doc comment: `demux::start()`'s loop used to treat every `recv()` failure as fatal to the transport as a whole, breaking the loop (and stopping the only demux task in the process) over a single malformed message from any one peer. Only a genuine socket-level failure is actually fatal — a bad payload from one peer is a per-message problem that should be logged and skipped, with every other worker's events unaffected.
**Tests:** Connects two DEALER sockets to a bound `RouterTransport`. Sends plain ASCII bytes (not valid msgpack) from the first ("bad") DEALER, asserts the demux task's `JoinHandle::is_finished()` is still `false` afterward, then sends a real, registered `WorkerEvent::Pong { seq: 99 }` from the second ("good") DEALER and asserts it is still correctly delivered to that worker's broadcast channel, identified by wire identity.
**Inputs:** Plain ASCII bytes (`b"not valid msgpack"`) from one unregistered peer, followed by a real encoded `WorkerEvent::Pong { seq: 99 }` from a second, registered peer.
**Expected output:** The demux task survives the undecodable payload (`is_finished() == false`); the second peer's real event is still broadcast correctly afterward, with the correct wire identity and event contents.
**Acceptance command:** `cargo test -p anvilml-worker --features mock-hardware -- demux_tests::test_demux_survives_undecodable_payload` exits 0.

## test_deregister_removes_route (anvilml-worker)

**File:** `crates/anvilml-worker/tests/demux_tests.rs`
**Context:** Regression test for the memory-leak concern that motivated adding `deregister()` in the first place — before it existed, the routing table only ever grew, so a crashed or shut-down worker's entry (and the broadcast channel it holds open) would persist for the lifetime of the process across every respawn. Unlike the other tests in this file, this one only touches the in-memory table directly — no real transport or DEALER socket is needed, since `register`/`deregister` don't depend on anything `start()`'s task does with the table.
**Tests:** Registers a route, asserts it is present, deregisters it, asserts it is absent, then calls `deregister()` again on the same now-absent key and asserts this does not panic.
**Inputs:** One route registered under key `"0"`; the same key deregistered twice in succession.
**Expected output:** The route is present after `register()`, absent after `deregister()`, and a second `deregister()` call on an already-absent key completes without panicking — covering the case of a worker crashing before its own `spawn()` call ever reaches registration.
**Acceptance command:** `cargo test -p anvilml-worker --features mock-hardware -- demux_tests::test_deregister_removes_route` exits 0.
## test_update_populates_registry (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/node_registry_tests.rs`
**Context:** `NodeTypeRegistry::update_from_worker` inserts `NodeTypeDescriptor` values into the internal hash map keyed by `type_name`. `get`, `all_types`, and `is_empty` reflect the updated state. No I/O, no subprocess, no env vars — pure in-memory operations.
**Tests:** Creates an empty registry, calls `update_from_worker` with two descriptors (`LoadModel`, `KSampler`), asserts `get` returns each by name, `all_types().len() == 2`, and `is_empty() == false`.
**Inputs:** `worker_id = "worker-0"`, two `NodeTypeDescriptor` values with distinct `type_name`.
**Expected output:** `get("LoadModel")` returns the descriptor; `get("KSampler")` returns the descriptor; `all_types().len() == 2`; `is_empty() == false`.
**Acceptance command:** `cargo test -p anvilml-scheduler --features mock-hardware -- node_registry` exits 0.

## test_get_returns_none_for_unknown_type (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/node_registry_tests.rs`
**Context:** `get` returns `None` when the requested `type_name` has never been registered. Tests the lookup path with an empty registry.
**Tests:** Creates a default (empty) registry and calls `get("NonExistent")`.
**Inputs:** `type_name = "NonExistent"`.
**Expected output:** `get("NonExistent") == None`.
**Acceptance command:** `cargo test -p anvilml-scheduler --features mock-hardware -- node_registry` exits 0.

## test_all_types_returns_all_descriptors (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/node_registry_tests.rs`
**Context:** `all_types` returns all registered descriptors. Verifies both count and content match the inputs.
**Tests:** Populates registry with 3 descriptors (A, B, C), calls `all_types`, asserts length is 3 and each type_name is present.
**Inputs:** `worker_id = "worker-0"`, three `NodeTypeDescriptor` values.
**Expected output:** `all_types().len() == 3`; type names A, B, and C are all present in the returned vec.
**Acceptance command:** `cargo test -p anvilml-scheduler --features mock-hardware -- node_registry` exits 0.

## test_is_empty_before_and_after_update (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/node_registry_tests.rs`
**Context:** `is_empty` correctly reports the registry state — `true` on a fresh registry, `false` after any update.
**Tests:** Asserts `is_empty()` is `true` on default, calls `update_from_worker` with one descriptor, asserts `is_empty()` is `false`.
**Inputs:** `worker_id = "worker-0"`, one `NodeTypeDescriptor`.
**Expected output:** `is_empty() == true` before update; `is_empty() == false` after.
**Acceptance command:** `cargo test -p anvilml-scheduler --features mock-hardware -- node_registry` exits 0.

## test_update_from_worker_merges (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/node_registry_tests.rs`
**Context:** `update_from_worker` implements merge semantics: existing entries are preserved when a new batch arrives that does not contain them. This is critical because different workers may register different node type subsets.
**Tests:** Updates registry with type A from worker-0, then updates with type B from worker-1 (no A). Verifies both A and B are still present.
**Inputs:** First update: `[A]` from `worker-0`; second update: `[B]` from `worker-1`.
**Expected output:** After both updates, `get("A") == Some(...)` and `get("B") == Some(...)` and `all_types().len() == 2`.
**Acceptance command:** `cargo test -p anvilml-scheduler --features mock-hardware -- node_registry` exits 0.

## test_managed_worker_forwards_to_node_registry (anvilml-worker)

**File:** `crates/anvilml-worker/tests/pool_tests.rs`
**Context:** P11-A2 wires `NodeTypeRegistry` (relocated to `anvilml-core` to break a dependency cycle — see `anvilml_core::node_registry`'s module doc) into `ManagedWorker`'s `Ready` event handler. This test verifies the wiring's static shape — `ManagedWorker::new()` accepts `Some(Arc<NodeTypeRegistry>)` without a compile error — and verifies `update_from_worker`'s contract directly, including the mock-worker empty-`node_types` case. It does **not** drive a real `Ready` event through `run()`'s `select!` loop; see the test's own doc comment for why that approach was tried and abandoned, and what `test_run_ready_event_releases_keepalive_gate` (in `managed_tests.rs`) already covers instead for the loop-delivery side of this wiring.
**Tests:** Constructs a `ManagedWorker` with `Some(registry)`, then calls `update_from_worker` with two `NodeTypeDescriptor`s and asserts both are retrievable and `all_types().len() == 2`. Separately constructs a second, empty registry and asserts `is_empty()` stays `true` and `has_been_updated()` flips from `false` to `true` after an update with an empty `Vec` (the mock-hardware `Ready` event case) — see `test_has_been_updated_distinguishes_never_updated_from_empty_update` for the focused unit test of this exact distinction.
**Inputs:** Two descriptors (`LoadModel`, `KSampler`) on the first registry; an empty `Vec<NodeTypeDescriptor>` on the second.
**Expected output:** `all_types().len() == 2`, `get("LoadModel")`/`get("KSampler")` both `Some(...)`; second registry's `is_empty()` is `true` both before and after its update (the map gains no entries); `has_been_updated()` is `false` before and `true` after.
**Acceptance command:** `cargo test -p anvilml-worker --features mock-hardware -- test_managed_worker_forwards_to_node_registry` exits 0.

## test_has_been_updated_distinguishes_never_updated_from_empty_update (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/node_registry_tests.rs`
**Context:** `is_empty()` reflects only the underlying map's contents, so it cannot distinguish "no worker has ever reached `Ready`" from "a worker reached `Ready` and reported zero node types" — both leave the map empty. `NodeTypeRegistry` gained a separate `has_been_updated()` method (and an internal `AtomicBool` flag, set once on the first `update_from_worker` call and never unset) specifically for this distinction, which P11-A3's `GET /v1/nodes` 503-vs-200 logic depends on.
**Tests:** Asserts both `is_empty()` and `has_been_updated()` are in their initial state on a fresh registry; calls `update_from_worker` with an empty `Vec`; asserts `is_empty()` stays `true` but `has_been_updated()` becomes `true`; calls `update_from_worker` again with one real descriptor; asserts `is_empty()` becomes `false` and `has_been_updated()` remains `true`.
**Inputs:** First update: empty `Vec` from `"mock-worker"`; second update: one `NodeTypeDescriptor` from `"worker-1"`.
**Expected output:** `is_empty()` sequence: `true → true → false`. `has_been_updated()` sequence: `false → true → true`.
**Acceptance command:** `cargo test -p anvilml-scheduler --features mock-hardware -- node_registry` exits 0.

## test_nodes_returns_503_when_registry_not_updated (anvilml-server)

**File:** `crates/anvilml-server/tests/nodes_tests.rs`
**Context:** The `GET /v1/nodes` handler returns 503 when no worker has ever reached `Ready`. The registry's `has_been_updated()` flag is `false` on a fresh registry, so the handler returns `WorkersUnavailable`. Uses `Router::oneshot` to exercise the full handler pipeline without a live TCP listener.
**Tests:** Builds `AppState` with a fresh `NodeTypeRegistry` (never updated), sends GET `/v1/nodes`, asserts HTTP 503, parses the JSON error body, and verifies `"error" == "workers_unavailable"` and the message mentions "no worker has reached Ready".
**Inputs:** GET `/v1/nodes`, `AppState::new("test-version", Arc::new(NodeTypeRegistry::new().await))`.
**Expected output:** HTTP 503 with JSON body `{"error":"workers_unavailable","message":"no worker has reached Ready","request_id":"..."}`.
**Acceptance command:** `cargo test -p anvilml-server --features mock-hardware --test nodes_tests test_nodes_returns_503_when_registry_not_updated` exits 0.

## test_nodes_returns_200_after_worker_ready (anvilml-server)

**File:** `crates/anvilml-server/tests/nodes_tests.rs`
**Context:** The `GET /v1/nodes` handler returns 200 with an empty array when a mock worker has reached `Ready` with zero node types. The registry's `has_been_updated()` flag is `true` (set by `update_from_worker` even with an empty types list), so the handler returns 200. Uses `Router::oneshot` to exercise the full handler pipeline without a live TCP listener.
**Tests:** Builds a `NodeTypeRegistry`, calls `update_from_worker("worker-0", vec![])` to simulate a mock worker reaching Ready with zero node types, wraps it in `Arc`, passes it to `AppState::new`, sends GET `/v1/nodes`, asserts HTTP 200, and verifies the body is an empty JSON array `[]`.
**Inputs:** GET `/v1/nodes`, `AppState::new("test-version", Arc::new(registry))` where `registry.update_from_worker("worker-0", vec![]).await` was called.
**Expected output:** HTTP 200 with JSON body `[]`.
**Acceptance command:** `cargo test -p anvilml-server --features mock-hardware --test nodes_tests test_nodes_returns_200_after_worker_ready` exits 0.

## test_registry_populated_after_import (worker)

**File:** `worker/tests/test_nodes_base.py`
**Context:** The `worker.nodes` package provides the dynamic node registration infrastructure. On first import, ``_ensure_imported()`` scans the package directory for sibling ``.py`` files and imports each one. This test verifies that importing ``worker.nodes`` does not raise and ``NODE_REGISTRY`` is accessible as a dict. At this stage no concrete node modules exist, so the registry is empty — this test verifies the import path works without errors.
**Tests:** Imports ``worker.nodes``, asserts ``NODE_REGISTRY`` is a dict.
**Inputs:** ``import worker.nodes``.
**Expected output:** No exception raised; ``NODE_REGISTRY`` is a ``dict``.
**Acceptance command:** ``ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_base.py::test_registry_populated_after_import`` exits 0.

## test_register_decorator_adds_class (worker)

**File:** `worker/tests/test_nodes_base.py`
**Context:** The ``@register`` decorator validates that a node class exposes all six required metadata attributes (``NODE_TYPE``, ``CATEGORY``, ``DISPLAY_NAME``, ``DESCRIPTION``, ``INPUT_SLOTS``, ``OUTPUT_SLOTS``), raises ``TypeError`` if any is missing, then stores the class in ``NODE_REGISTRY`` keyed by ``NODE_TYPE``. This test verifies the registration path works correctly with a minimal concrete node class. Each test uses the ``registry_clean`` autouse fixture that clears ``NODE_REGISTRY`` before each test to ensure isolation.
**Tests:** Defines a concrete node class with all six required attributes, applies ``@register``, and asserts the class appears in ``NODE_REGISTRY`` under the correct key.
**Inputs:** A class decorated with ``@register`` and all required attributes.
**Expected output:** ``NODE_REGISTRY["TestNode"]`` returns the test class.
**Acceptance command:** ``ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_base.py::test_register_decorator_adds_class`` exits 0.

## test_base_node_cannot_be_instantiated (worker)

**File:** `worker/tests/test_nodes_base.py`
**Context:** ``BaseNode`` is an abstract base class (ABC) that enforces implementation of the ``execute()`` method. Direct instantiation must fail with ``TypeError``, preventing accidental use of the abstract class.
**Tests:** Attempts to call ``BaseNode()`` directly and asserts that ``TypeError`` is raised.
**Inputs:** ``BaseNode()`` call.
**Expected output:** ``TypeError`` raised — ABC enforcement works.
**Acceptance command:** ``ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_base.py::test_base_node_cannot_be_instantiated`` exits 0.

## test_slot_spec_dataclass (worker)

**File:** `worker/tests/test_nodes_base.py`
**Context:** ``SlotSpec`` is a ``@dataclass`` that declares one input or output slot on a node. It has three fields: ``name: str``, ``slot_type: str``, and ``optional: bool = False``. This test verifies the dataclass creates instances with correct field values and defaults.
**Tests:** Constructs a ``SlotSpec`` with just name and slot_type, asserts the optional field defaults to ``False``. Also constructs one with explicit ``optional=True`` and verifies that value.
**Inputs:** ``SlotSpec("input1", "MODEL")``, ``SlotSpec("seed", "Int", optional=True)``.
**Expected output:** ``name="input1"``, ``slot_type="MODEL"``, ``optional=False`` for the first; ``optional=True`` for the second.
**Acceptance command:** ``ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_base.py::test_slot_spec_dataclass`` exits 0.

## test_missing_nodes_array (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/dag_tests.rs`
**Context:** `validate_graph` receives a graph JSON without a `"nodes"` field. Verifies the function returns an error about the missing nodes array and does not panic on malformed input.
**Tests:** Submits a graph with only `"edges": []`, asserts `validate_graph` returns `Err` with exactly one error message containing "nodes" and "missing".
**Inputs:** `{"edges": []}` (no `"nodes"` field).
**Expected output:** `Err` with one message about missing nodes array.
**Acceptance command:** `cargo test -p anvilml-scheduler --features mock-hardware -- test_missing_nodes_array` exits 0.

## test_duplicate_node_ids (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/dag_tests.rs`
**Context:** `validate_graph` receives a graph with two nodes sharing the same `"id"` value. Verifies that the duplicate ID is detected and reported.
**Tests:** Populates registry with `LoadModel`, submits a graph with two nodes both having `"id": "n1"`, asserts the error list contains a message with "duplicate" and "n1".
**Inputs:** Two nodes with `"id": "n1"`, `"type": "LoadModel"`.
**Expected output:** `Err` with duplicate ID error naming "n1".
**Acceptance command:** `cargo test -p anvilml-scheduler --features mock-hardware -- test_duplicate_node_ids` exits 0.

## test_unknown_node_type (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/dag_tests.rs`
**Context:** `validate_graph` receives a graph with a node whose type is not registered in the node type registry. Verifies the unknown type is reported.
**Tests:** Populates registry with only `LoadModel`, submits a graph with a node of type `"NonExistent"`, asserts the error list contains a message with "NonExistent" and "unknown type".
**Inputs:** Node with `"id": "n1"`, `"type": "NonExistent"`.
**Expected output:** `Err` with unknown type error naming "NonExistent".
**Acceptance command:** `cargo test -p anvilml-scheduler --features mock-hardware -- test_unknown_node_type` exits 0.

## test_bad_edge_ref_missing_node (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/dag_tests.rs`
**Context:** `validate_graph` receives a graph with an edge referencing a source node that does not exist in the nodes list. Verifies the missing node is reported.
**Tests:** Populates registry with `LoadModel`, submits a graph with one node and an edge whose `"node_id"` is `"ghost"`, asserts the error list contains a message with "ghost" and "missing source node".
**Inputs:** Edge with `"node_id": "ghost"`, `"output_slot": "model"`, `"target": "sampler"`, `"target_slot": "model"`.
**Expected output:** `Err` with missing node error naming "ghost".
**Acceptance command:** `cargo test -p anvilml-scheduler --features mock-hardware -- test_bad_edge_ref_missing_node` exits 0.

## test_bad_edge_ref_missing_slot (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/dag_tests.rs`
**Context:** `validate_graph` receives a graph with an edge referencing an output slot that does not exist on the source node's type descriptor. Verifies the missing slot is reported.
**Tests:** Populates registry with `LoadModel` (outputs `"model"` only), submits a graph with an edge whose `"output_slot"` is `"nonexistent"`, asserts the error list contains a message with "nonexistent" and "no output slot".
**Inputs:** Edge with `"node_id": "model"`, `"output_slot": "nonexistent"`, `"target": "sampler"`, `"target_slot": "model"`.
**Expected output:** `Err` with missing slot error naming "nonexistent".
**Acceptance command:** `cargo test -p anvilml-scheduler --features mock-hardware -- test_bad_edge_ref_missing_slot` exits 0.

## test_slot_type_mismatch (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/dag_tests.rs`
**Context:** `validate_graph` receives a graph with an edge connecting a `Model` output slot to an `Image` input slot. These types are incompatible (neither is `Any`), so validation fails.
**Tests:** Populates registry with `LoadModel` (outputs `Model`) and `SaveImage` (inputs `Image`), submits a graph connecting them, asserts the error list contains "type mismatch", "Model", and "Image".
**Inputs:** Edge from `LoadModel.model` (Model) to `SaveImage.image` (Image).
**Expected output:** `Err` with type mismatch error naming both types.
**Acceptance command:** `cargo test -p anvilml-scheduler --features mock-hardware -- test_slot_type_mismatch` exits 0.

## test_cycle_detected (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/dag_tests.rs`
**Context:** `validate_graph` receives a graph with a cycle: A → B → C → A. Verifies that Kahn's algorithm detects the cycle and reports all three nodes.
**Tests:** Populates registry with `NodeA`, `NodeB`, `NodeC` (each with Latent input/output), submits a graph with a cyclic edge set, asserts the error list contains "cycle" and names all three nodes.
**Inputs:** Nodes A, B, C with edges A→B, B→C, C→A.
**Expected output:** `Err` with cycle error naming all three nodes.
**Acceptance command:** `cargo test -p anvilml-scheduler --features mock-hardware -- test_cycle_detected` exits 0.

## test_valid_graph_returns_ok (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/dag_tests.rs`
**Context:** `validate_graph` receives a complete valid DAG: LoadModel → Sampler → VaeDecode → SaveImage. All six validation checks pass, so the function returns `Ok(ValidatedGraph)`.
**Tests:** Populates registry with all four node types and their correct slot signatures, submits a fully-connected DAG with matching slot types, asserts `validate_graph` returns `Ok(ValidatedGraph)`.
**Inputs:** Full valid graph with LoadModel, Sampler, VaeDecode, SaveImage and correct edges.
**Expected output:** `Ok(ValidatedGraph)` wrapping the original graph value.
**Acceptance command:** `cargo test -p anvilml-scheduler --features mock-hardware -- test_valid_graph_returns_ok` exits 0.

## test_multiple_errors_collected (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/dag_tests.rs`
**Context:** `validate_graph` receives a graph with both duplicate IDs and an unknown type in the same submission. Verifies that both errors are returned in a single `Err` response (non-fail-fast behaviour).
**Tests:** Populates registry with only `LoadModel`, submits a graph with two nodes sharing `"id": "n1"`, one of type `LoadModel` and one of type `NonExistent`, asserts the error list has ≥ 2 entries containing both "duplicate" and "NonExistent".
**Inputs:** Two nodes with `"id": "n1"`, types `LoadModel` and `NonExistent`.
**Expected output:** `Err` with ≥ 2 error strings (duplicate ID + unknown type).
**Acceptance command:** `cargo test -p anvilml-scheduler --features mock-hardware -- test_multiple_errors_collected` exits 0.

## test_any_slot_type_compatible (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/dag_tests.rs`
**Context:** `validate_graph` receives a graph with an edge connecting a `SlotType::Any` output to a `SlotType::Model` input. Verifies that the `Any` type is compatible with any concrete type, so no type mismatch error is produced.
**Tests:** Populates registry with `NodeAny` (outputs `Any`) and `NodeModel` (inputs `Model`), submits a graph connecting them, asserts `validate_graph` returns `Ok(ValidatedGraph)`.
**Inputs:** Edge from `NodeAny.out` (Any) to `NodeModel.model` (Model).
**Expected output:** `Ok(ValidatedGraph)` (no type error).
**Acceptance command:** `cargo test -p anvilml-scheduler --features mock-hardware -- test_any_slot_type_compatible` exits 0.

## test_submit_job_returns_503_when_no_workers (anvilml-server)

**File:** `crates/anvilml-server/tests/jobs_tests.rs`
**Context:** `POST /v1/jobs` handler returns 503 when the node type registry is empty (no workers have ever reached Ready). The handler checks `is_empty()` before attempting validation.
**Tests:** Builds `AppState` with a fresh `NodeTypeRegistry` that has never had `update_from_worker` called, sends POST `/v1/jobs` with an empty graph, asserts 503 with error body `{"error": "workers_unavailable"}`.
**Inputs:** POST body `{"graph": {}, "settings": {}}`, fresh `NodeTypeRegistry`.
**Expected output:** HTTP 503, `error: "workers_unavailable"`.
**Acceptance command:** `cargo test -p anvilml-server --features mock-hardware -- test_submit_job_returns_503_when_no_workers` exits 0.

## test_submit_job_returns_422_with_unknown_node_type (anvilml-server)

**File:** `crates/anvilml-server/tests/jobs_tests.rs`
**Context:** `POST /v1/jobs` handler returns 422 when the submitted graph contains an unknown node type. The handler calls `validate_graph` which checks type registration against the registry.
**Tests:** Builds registry with `LoadModel` registered, sends POST `/v1/jobs` with a graph containing a node of type `"GhostNode"`, asserts 422 with error body `{"error": "invalid_graph"}` and message containing "GhostNode".
**Inputs:** POST body `{"graph": {"nodes": [{"id": "n1", "type": "GhostNode"}]}, "settings": {}}`, registry with `LoadModel`.
**Expected output:** HTTP 422, `error: "invalid_graph"`, message contains "GhostNode".
**Acceptance command:** `cargo test -p anvilml-server --features mock-hardware -- test_submit_job_returns_422_with_unknown_node_type` exits 0.

## test_submit_job_returns_202_with_valid_graph (anvilml-server)

**File:** `crates/anvilml-server/tests/jobs_tests.rs`
**Context:** `POST /v1/jobs` handler returns 202 when the submitted graph is valid. The handler calls `validate_graph` which passes all six validation checks.
**Tests:** Builds registry with `LoadModel` registered, sends POST `/v1/jobs` with a valid graph containing a single `LoadModel` node, asserts 202 with a response body containing a valid `job_id` UUID and `queue_position: 0`.
**Inputs:** POST body `{"graph": {"nodes": [{"id": "n1", "type": "LoadModel"}]}, "settings": {}}`, registry with `LoadModel`.
**Expected output:** HTTP 202, `job_id` is a valid UUID, `queue_position: 0`.
**Acceptance command:** `cargo test -p anvilml-server --features mock-hardware -- test_submit_job_returns_202_with_valid_graph` exits 0.

## test_push_pop_fifo_order (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/queue_tests.rs`
**Context:** `JobQueue::push` and `JobQueue::pop_front` maintain FIFO ordering. Three jobs are pushed, then popped three times and the order is verified.
**Tests:** Push three jobs with distinct UUIDs, pop three times, assert each popped job's UUID matches the push order.
**Inputs:** Three `Job` values with distinct UUIDs.
**Expected output:** Pop order matches push order.
**Acceptance command:** `cargo test -p anvilml-scheduler --features mock-hardware -- test_push_pop_fifo_order` exits 0.

## test_pop_empty_returns_none (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/queue_tests.rs`
**Context:** Popping from an empty queue must return `None` without panicking. This is the base case for `pop_front`.
**Tests:** Constructs a fresh `JobQueue`, calls `pop_front`, asserts `None`.
**Inputs:** None (empty queue).
**Expected output:** `None`.
**Acceptance command:** `cargo test -p anvilml-scheduler --features mock-hardware -- test_pop_empty_returns_none` exits 0.

## test_cancel_returns_true_and_removes (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/queue_tests.rs`
**Context:** `JobQueue::cancel` returns `true` for an existing job and removes it from the queue. After cancellation, `get` returns `None`, `len` decreases, and `pop_front` returns `None`.
**Tests:** Push one job, cancel it by UUID, assert `true` return, verify `get` returns `None`, `len == 0`, and `pop_front` returns `None`.
**Inputs:** One `Job` value.
**Expected output:** `cancel` returns `true`, queue is empty afterward.
**Acceptance command:** `cargo test -p anvilml-scheduler --features mock-hardware -- test_cancel_returns_true_and_removes` exits 0.

## test_cancel_returns_false_for_missing_id (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/queue_tests.rs`
**Context:** `JobQueue::cancel` returns `false` when the UUID does not match any job in the queue. The queue state must be unchanged.
**Tests:** Push one job, cancel with a different UUID, assert `false` return and that the existing job is still accessible.
**Inputs:** One `Job` value plus an unknown UUID.
**Expected output:** `cancel` returns `false`, queue unchanged.
**Acceptance command:** `cargo test -p anvilml-scheduler --features mock-hardware -- test_cancel_returns_false_for_missing_id` exits 0.

## test_get_returns_job_by_id (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/queue_tests.rs`
**Context:** `JobQueue::get` returns a reference to the job matching the given UUID via the index map lookup.
**Tests:** Push one job, call `get` with its UUID, assert the returned reference has matching `id` and `status` fields.
**Inputs:** One `Job` value.
**Expected output:** `get` returns `Some(&job)` with matching fields.
**Acceptance command:** `cargo test -p anvilml-scheduler --features mock-hardware -- test_get_returns_job_by_id` exits 0.

## test_list_returns_all_jobs_in_order (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/queue_tests.rs`
**Context:** `JobQueue::list` returns all jobs in FIFO dispatch order as a `Vec<&Job>`.
**Tests:** Push three jobs, call `list`, assert length is 3 and each element's UUID matches the push order.
**Inputs:** Three `Job` values.
**Expected output:** `Vec` of length 3, order matches push order.
**Acceptance command:** `cargo test -p anvilml-scheduler --features mock-hardware -- test_list_returns_all_jobs_in_order` exits 0.

## test_len_after_operations (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/queue_tests.rs`
**Context:** `JobQueue::len` correctly tracks the count through push, pop_front, and cancel operations.
**Tests:** Push three jobs (len=3), pop one (len=2), cancel one (len=1), pop one (len=0). Asserts len at each step.
**Inputs:** Three `Job` values, series of push/pop/cancel calls.
**Expected output:** `len()` matches expected count at each step.
**Acceptance command:** `cargo test -p anvilml-scheduler --features mock-hardware -- test_len_after_operations` exits 0.

## test_cancel_last_item (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/queue_tests.rs`
**Context:** Cancelling the last item in the queue exercises the `index == last_index` branch in `cancel`, where the swap is a no-op.
**Tests:** Push two jobs, cancel the last one, assert the first job remains and len is 1.
**Inputs:** Two `Job` values.
**Expected output:** Last item removed, first item remains, len == 1.
**Acceptance command:** `cargo test -p anvilml-scheduler --features mock-hardware -- test_cancel_last_item` exits 0.

## test_cancel_first_item_with_displacement (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/queue_tests.rs`
**Context:** Cancelling the first item triggers swap-remove with displacement: the last item moves to position 0. This verifies the displaced item's index is correctly updated in `by_id`.
**Tests:** Push three jobs, cancel the first, then pop two items and verify the remaining two are in the correct order (last item displaced to front, then middle item).
**Inputs:** Three `Job` values.
**Expected output:** After cancel first, pop order is: last item, then middle item.
**Acceptance command:** `cargo test -p anvilml-scheduler --features mock-hardware -- test_cancel_first_item_with_displacement` exits 0.

## test_multiple_cancellations (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/queue_tests.rs`
**Context:** Multiple consecutive cancellations must maintain queue integrity. Cancelling non-sequential items tests that index tracking remains correct after each swap-remove.
**Tests:** Push five jobs, cancel three (indices 1, 2, 3), assert remaining two are accessible and cancelled ones are not.
**Inputs:** Five `Job` values.
**Expected output:** Jobs 0 and 4 remain; jobs 1, 2, 3 are gone; len == 2.
**Acceptance command:** `cargo test -p anvilml-scheduler --features mock-hardware -- test_multiple_cancellations` exits 0.

## test_register_device_and_would_fit (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/ledger_tests.rs`
**Context:** `VramLedger::register_device` stores the device's total VRAM and initialises its reservation counter to zero. `would_fit` then checks whether a requested amount would fit within the unreserved portion. This is the happy path — the core registration + capacity check flow.
**Tests:** Registers device 0 with 24576 MiB (24 GB), then checks that an 8192 MiB request fits.
**Inputs:** `VramLedger::new()`, `register_device(0, 24576)`, `would_fit(0, 8192)`.
**Expected output:** `would_fit` returns `true`.
**Acceptance command:** `cargo test -p anvilml-scheduler --features mock-hardware -- test_register_device_and_would_fit` exits 0.

## test_would_fit_unknown_device_returns_false (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/ledger_tests.rs`
**Context:** `VramLedger::would_fit` returns `false` for a device index that was never registered. This tests the negative path — calling `would_fit` with an unregistered device must return `false` rather than panicking or returning a misleading value.
**Tests:** Creates a fresh ledger (no devices registered), calls `would_fit(99, 1024)`.
**Inputs:** `VramLedger::new()`, `would_fit(99, 1024)`.
**Expected output:** `would_fit` returns `false`.
**Acceptance command:** `cargo test -p anvilml-scheduler --features mock-hardware -- test_would_fit_unknown_device_returns_false` exits 0.

## test_reserve_reduces_free_vram (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/ledger_tests.rs`
**Context:** `VramLedger::reserve` increments the reservation counter, reducing the available free VRAM. This test verifies the full reserve lifecycle: register, reserve, check `would_fit` before and after.
**Tests:** Registers a 24 GB device, reserves 8 GB, verifies 16 GB still fits but 17 GB does not.
**Inputs:** `VramLedger::new()`, `register_device(0, 24576)`, `reserve(0, 8192)`, `would_fit(0, 16384)`, `would_fit(0, 16385)`.
**Expected output:** 16 GB fits, 17 GB does not fit.
**Acceptance command:** `cargo test -p anvilml-scheduler --features mock-hardware -- test_reserve_reduces_free_vram` exits 0.

## test_release_restores_free_vram (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/ledger_tests.rs`
**Context:** `VramLedger::release` decrements the reservation counter, restoring the previously reserved VRAM to the free pool. This tests the full reserve → release lifecycle.
**Tests:** Registers a 24 GB device, reserves 8 GB, releases 4 GB, verifies the remaining free capacity increased accordingly.
**Inputs:** `VramLedger::new()`, `register_device(0, 24576)`, `reserve(0, 8192)`, `release(0, 4096)`, `would_fit(0, 20480)`, `would_fit(0, 20481)`.
**Expected output:** 20 GB fits, 20481 MiB does not fit.
**Acceptance command:** `cargo test -p anvilml-scheduler --features mock-hardware -- test_release_restores_free_vram` exits 0.

## test_reserve_overflow_panics (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/ledger_tests.rs`
**Context:** `VramLedger::reserve` panics with `assert!` when the reservation would exceed total VRAM. This is intentional — over-reservation represents a programming error in the dispatch loop (which should have called `would_fit` first).
**Tests:** Registers a 24 GB device, reserves 20 GB, then attempts to reserve 8 GB more (total would be 28 GB > 24 GB). The `reserve` method must panic.
**Inputs:** `VramLedger::new()`, `register_device(0, 24576)`, `reserve(0, 20480)`, `reserve(0, 8192)`.
**Expected output:** `reserve` panics with message containing "VRAM reservation overflow".
**Acceptance command:** `cargo test -p anvilml-scheduler --features mock-hardware -- test_reserve_overflow_panics` exits 0.

## test_duplicate_registration_is_noop (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/ledger_tests.rs`
**Context:** `VramLedger::register_device` is idempotent — calling it twice for the same device index is a no-op. This prevents duplicate registration errors from repeated discovery scans.
**Tests:** Registers device 0 with 24 GB twice, then checks that `would_fit(0, 24576)` still returns `true` (reservation is zero, not double-counted).
**Inputs:** `VramLedger::new()`, `register_device(0, 24576)` called twice.
**Expected output:** `would_fit(0, 24576)` returns `true`.
**Acceptance command:** `cargo test -p anvilml-scheduler --features mock-hardware -- test_duplicate_registration_is_noop` exits 0.

## test_release_underflow_panics (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/ledger_tests.rs`
**Context:** `VramLedger::release` panics with `assert!` when the release would underflow (reservation cannot go negative). This represents a bug in the release logic.
**Tests:** Registers a 24 GB device, reserves 4 GB, then attempts to release 8 GB. The `release` method must panic.
**Inputs:** `VramLedger::new()`, `register_device(0, 24576)`, `reserve(0, 4096)`, `release(0, 8192)`.
**Expected output:** `release` panics with message containing "VRAM release underflow".
**Acceptance command:** `cargo test -p anvilml-scheduler --features mock-hardware -- test_release_underflow_panics` exits 0.

## test_multiple_devices_independent (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/ledger_tests.rs`
**Context:** `VramLedger` tracks reservations per device index independently. Reserving on device 0 must not affect device 1's available capacity.
**Tests:** Registers two devices with different VRAM totals (24 GB and 12 GB), reserves 20 GB on device 0, verifies device 1's capacity is unaffected.
**Inputs:** `VramLedger::new()`, `register_device(0, 24576)`, `register_device(1, 12288)`, `reserve(0, 20480)`.
**Expected output:** Device 1: 12 GB fits. Device 0: 4 GB fits, 4097 does not.
**Acceptance command:** `cargo test -p anvilml-scheduler --features mock-hardware -- test_multiple_devices_independent` exits 0.

## test_submit_valid_graph (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/scheduler_tests.rs`
**Context:** `JobScheduler::submit()` validates a computation graph, persists the job to SQLite, enqueues it, and broadcasts a `JobQueued` WebSocket event. Uses `open_in_memory()` for database isolation. Annotated with `#[serial]` because `open_in_memory()` creates a single-connection SQLite pool that cannot be safely shared across concurrent Tokio tasks.
**Tests:** Submits a valid graph containing a single `LoadModel` node. Verifies `submit()` returns `Ok(SubmitJobResponse)` with a valid UUID and queue position 1. Then calls `get_job()` to verify the persisted job has `status=Queued`, `queue_position=Some(1)`, and a `created_at` timestamp within the current second.
**Inputs:** `SubmitJobRequest{graph: {nodes: [{id: "model", type: "LoadModel"}]}, settings: JobSettings::default()}`.
**Expected output:** `Ok(SubmitJobResponse{job_id: <valid UUID>, queue_position: 1})` and `get_job()` returns `Some(Job{status: Queued, queue_position: Some(1)})`.
**Acceptance command:** `cargo test -p anvilml-scheduler --features mock-hardware -- scheduler` exits 0.

## test_submit_invalid_graph (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/scheduler_tests.rs`
**Context:** Graph validation runs before any database INSERT or queue push. A graph with an unknown node type must fail validation and produce `AnvilError::InvalidGraph` without persisting anything.
**Tests:** Submits a graph containing a `NonExistent` node type. Verifies `submit()` returns `Err(AnvilError::InvalidGraph(errors))` where `errors` mentions "NonExistent". Then calls `list_jobs()` to verify no jobs were persisted.
**Inputs:** `SubmitJobRequest{graph: {nodes: [{id: "ghost", type: "NonExistent"}]}, settings: JobSettings::default()}`.
**Expected output:** `Err(AnvilError::InvalidGraph([error mentioning "NonExistent"]))` and `list_jobs()` returns empty vec.
**Acceptance command:** `cargo test -p anvilml-scheduler --features mock-hardware -- scheduler` exits 0.

## test_get_job_returns_job (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/scheduler_tests.rs`
**Context:** The round-trip from `submit()` → persist → `get_job()` preserves all job fields correctly, including custom settings.
**Tests:** Submits a job with `device_preference: Some("cuda:0")`, then calls `get_job()` and verifies the returned job has the same `id`, `status`, `settings.device_preference`, and `queue_position`.
**Inputs:** `SubmitJobRequest{graph: valid LoadModel graph, settings: {device_preference: Some("cuda:0")}}`.
**Expected output:** `get_job()` returns `Some(Job{id: <submitted_id>, status: Queued, settings: {device_preference: Some("cuda:0")}, queue_position: Some(1)})`.
**Acceptance command:** `cargo test -p anvilml-scheduler --features mock-hardware -- scheduler` exits 0.

## test_get_job_missing_returns_none (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/scheduler_tests.rs`
**Context:** Querying for a UUID that was never submitted should return `Ok(None)`, not an error.
**Tests:** Calls `get_job()` with a freshly generated UUID that was never submitted. Verifies the result is `Ok(None)`.
**Inputs:** `Uuid::new_v4()` (a random UUID never submitted).
**Expected output:** `Ok(None)` — no error, just not found.
**Acceptance command:** `cargo test -p anvilml-scheduler --features mock-hardware -- scheduler` exits 0.

## test_list_jobs_returns_all (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/scheduler_tests.rs`
**Context:** `list_jobs()` returns all submitted jobs in descending `created_at` order (most recent first).
**Tests:** Submits three jobs with 10ms delays between each, then calls `list_jobs(None, None, None)` and verifies the result has length 3 and is ordered by `created_at` descending.
**Inputs:** Three valid `LoadModel` graphs submitted with small delays.
**Expected output:** `Ok(vec![job3, job2, job1])` where `job3.created_at >= job2.created_at >= job1.created_at`.
**Acceptance command:** `cargo test -p anvilml-scheduler --features mock-hardware -- scheduler` exits 0.

## test_list_jobs_filter_by_status (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/scheduler_tests.rs`
**Context:** `list_jobs(status=Some(...))` filters jobs by their status. After submitting two jobs, one is manually updated to `Failed` in the database (simulating dispatch loop behavior).
**Tests:** Submits two jobs, manually updates one to `Failed` via direct SQL, then calls `list_jobs(Some(Queued), None, None)` and verifies only the Queued job is returned. Also calls `list_jobs(Some(Failed), None, None)` and verifies the failed job is returned with its error message.
**Inputs:** Two valid `LoadModel` graphs, one manually updated to `Failed` status with `error = "test failure"`.
**Expected output:** `list_jobs(Queued)` returns 1 job; `list_jobs(Failed)` returns 1 job with `error = Some("test failure")`.
**Acceptance command:** `cargo test -p anvilml-scheduler --features mock-hardware -- scheduler` exits 0.

## test_list_jobs_with_limit (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/scheduler_tests.rs`
**Context:** `list_jobs(limit=Some(n))` returns at most `n` jobs, ordered by `created_at` descending.
**Tests:** Submits five jobs with 10ms delays, then calls `list_jobs(None, Some(2), None)` and verifies exactly 2 jobs are returned (the most recent ones).
**Inputs:** Five valid `LoadModel` graphs with small delays, `limit = Some(2)`.
**Expected output:** `Ok(vec![job5, job4])` — exactly 2 jobs, the most recent ones.
**Acceptance command:** `cargo test -p anvilml-scheduler --features mock-hardware -- scheduler` exits 0.

## test_list_jobs_with_before_filter (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/scheduler_tests.rs`
**Context:** `list_jobs(before=Some(t))` returns only jobs created strictly before the given time.
**Tests:** Submits one job, records the current time, waits 50ms, submits two more jobs. Calls `list_jobs(None, None, Some(after_first))` and verifies only the first job is returned.
**Inputs:** Three valid `LoadModel` graphs submitted with a 50ms gap after the first, `before = <time between first and second>`.
**Expected output:** `Ok(vec![job1])` — only the first job, created before the filter time.
**Acceptance command:** `cargo test -p anvilml-scheduler --features mock-hardware -- scheduler` exits 0.

## test_submit_job_returns_503_when_no_workers (anvilml-server)

**File:** `crates/anvilml-server/tests/jobs_tests.rs`
**Context:** The `POST /v1/jobs` handler delegates to `JobScheduler::submit()`. With an empty node registry, the scheduler's `validate_graph()` returns errors for any graph missing the `nodes` array, producing a 422 response instead of the old 503. Uses `test_scheduler()` helper which creates a `JobScheduler` backed by an in-memory SQLite pool.
**Tests:** Builds `AppState` with an empty `NodeTypeRegistry` and a scheduler, sends POST `/v1/jobs` with an empty graph `{}`, asserts HTTP 422 with `error: "invalid_graph"`.
**Inputs:** POST `/v1/jobs` with `{"graph": {}, "settings": {}}`, `AppState::new("test-version", empty_registry, scheduler)`.
**Expected output:** HTTP 422 with JSON body `{"error":"invalid_graph",...}`.
**Acceptance command:** `cargo test -p anvilml-server --test jobs_tests -- test_submit_job_returns_503_when_no_workers` exits 0.

## test_submit_job_returns_422_with_unknown_node_type (anvilml-server)

**File:** `crates/anvilml-server/tests/jobs_tests.rs`
**Context:** The `POST /v1/jobs` handler delegates to `JobScheduler::submit()` which validates the graph against the node type registry. A graph containing an unknown node type (`GhostNode`) fails validation. Uses `test_scheduler()` helper with a registry that has `LoadModel` registered.
**Tests:** Builds a registry with `LoadModel` registered, sends POST `/v1/jobs` with a graph containing `GhostNode`, asserts HTTP 422 with `error: "invalid_graph"` and message mentioning `GhostNode`.
**Inputs:** POST `/v1/jobs` with a graph containing `{"id": "n1", "type": "GhostNode"}`, `AppState::new("test-version", registry_with_loadmodel, scheduler)`.
**Expected output:** HTTP 422 with JSON body `{"error":"invalid_graph","message":"...",...}` where message contains `GhostNode`.
**Acceptance command:** `cargo test -p anvilml-server --test jobs_tests -- test_submit_job_returns_422_with_unknown_node_type` exits 0.

## test_submit_job_returns_202_with_valid_graph (anvilml-server)

**File:** `crates/anvilml-server/tests/jobs_tests.rs`
**Context:** The `POST /v1/jobs` handler delegates to `JobScheduler::submit()` which validates, persists, enqueues, and broadcasts a `JobQueued` event. Returns 202 with a real job ID and queue position (1-based). Uses `test_scheduler()` helper with a registry that has `LoadModel` registered.
**Tests:** Builds a registry with `LoadModel` registered, sends POST `/v1/jobs` with a valid graph containing a `LoadModel` node, asserts HTTP 202 with a valid `job_id` UUID and `queue_position: 1`.
**Inputs:** POST `/v1/jobs` with a valid graph containing `{"id": "n1", "type": "LoadModel"}`, `AppState::new("test-version", registry_with_loadmodel, scheduler)`.
**Expected output:** HTTP 202 with JSON body containing valid `job_id` (UUID string) and `queue_position: 1`.
**Acceptance command:** `cargo test -p anvilml-server --test jobs_tests -- test_submit_job_returns_202_with_valid_graph` exits 0.

## test_list_jobs_returns_queued_jobs (anvilml-server)

**File:** `crates/anvilml-server/tests/jobs_tests.rs`
**Context:** The `GET /v1/jobs` handler delegates to `JobScheduler::list_jobs()` which builds a dynamic SQL query with optional filters. Returns jobs ordered by `created_at` descending. Uses `test_scheduler()` helper with a registry that has `LoadModel` registered.
**Tests:** Submits a job via POST `/v1/jobs`, then calls GET `/v1/jobs`, asserts HTTP 200 with a JSON array containing at least one job with `status: "Queued"`.
**Inputs:** GET `/v1/jobs`, `AppState::new("test-version", registry_with_loadmodel, scheduler)`.
**Expected output:** HTTP 200 with JSON array containing at least one job where `status == "Queued"`.
**Acceptance command:** `cargo test -p anvilml-server --test jobs_tests -- test_list_jobs_returns_queued_jobs` exits 0.

## test_get_job_returns_404_for_unknown_id (anvilml-server)

**File:** `crates/anvilml-server/tests/jobs_tests.rs`
**Context:** The `GET /v1/jobs/{id}` handler delegates to `JobScheduler::get_job()` which queries the database. Returns 404 with `error: "job_not_found"` when no matching job exists. Uses `test_scheduler()` helper with an empty registry.
**Tests:** Calls GET `/v1/jobs/{uuid}` with a random UUID that was never submitted, asserts HTTP 404 with `error: "job_not_found"`.
**Inputs:** GET `/v1/jobs/{random_uuid}`, `AppState::new("test-version", empty_registry, scheduler)`.
**Expected output:** HTTP 404 with JSON body `{"error":"job_not_found",...}`.
**Acceptance command:** `cargo test -p anvilml-server --test jobs_tests -- test_get_job_returns_404_for_unknown_id` exits 0.

## test_run_graph_topo_order (worker)

**File:** `worker/tests/test_executor.py`
**Context:** `run_graph` executes nodes in topological order, resolving inputs from prior node outputs. Uses a `registry_clean` fixture to clear NODE_REGISTRY and two test node classes (NodeA, NodeB) where B depends on A's output.
**Tests:** A graph with two nodes where node 2 depends on node 1's output is executed. The execution order and input resolution are verified.
**Inputs:** Graph with NodeA → NodeB dependency, NodeB's inputs reference `["A", "value"]`.
**Expected output:** Execution order is `["A", "B"]`, node B receives node A's computed output.
**Acceptance command:** `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_executor.py::test_run_graph_topo_order -v` exits 0.

## test_saveimage_emits_image_ready (worker)

**File:** `worker/tests/test_executor.py`
**Context:** SaveImage node generates a 64×64 black PNG using only stdlib and emits an ImageReady event. The test verifies the PNG binary structure (signature, IHDR dimensions) and event fields.
**Tests:** A graph with a single SaveImage node is executed. The emitted event is captured and inspected for correct fields and valid PNG structure.
**Inputs:** Graph with single SaveImage node, `image` input set to `None`.
**Expected output:** ImageReady event with `job_id="test-job-1"`, `width=64`, `height=64`, `image_b64` decodes to a valid 64×64 PNG.
**Acceptance command:** `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_executor.py::test_saveimage_emits_image_ready -v` exits 0.

## test_completed_sent_after_run_graph (worker)

**File:** `worker/tests/test_executor.py`
**Context:** `run_graph` returns normally on successful execution, simulating the Completed path in worker_main. Uses a no-op test node with no inputs or outputs.
**Tests:** A graph with a single no-op node is executed. The function should return without raising.
**Inputs:** Graph with single NoOp node, empty inputs and outputs.
**Expected output:** No exception raised; function returns None.
**Acceptance command:** `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_executor.py::test_completed_sent_after_run_graph -v` exits 0.

## test_failed_sent_on_node_error (worker)

**File:** `worker/tests/test_executor.py`
**Context:** `run_graph` raises when a node's `execute()` fails, simulating the Failed path in worker_main. Uses a test node that always raises ValueError.
**Tests:** A graph with one failing node is executed. The exception should propagate from run_graph.
**Inputs:** Graph with single Failing node that raises `ValueError("simulated node failure")`.
**Expected output:** ValueError raised with message "simulated node failure".
**Acceptance command:** `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_executor.py::test_failed_sent_on_node_error -v` exits 0.

## test_topo_sort_cycle_detection (worker)

**File:** `worker/tests/test_executor.py`
**Context:** `_topo_sort` detects cycles using Kahn's algorithm. A cyclic graph (A→B→A) cannot be topologically sorted.
**Tests:** A graph with a cycle is passed to `_topo_sort`.
**Inputs:** Two nodes with circular input references: `A.inputs.x = ["B", "y"]`, `B.inputs.y = ["A", "x"]`.
**Expected output:** ValueError with message "graph contains a cycle".
**Acceptance command:** `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_executor.py::test_topo_sort_cycle_detection -v` exits 0.

## test_topo_sort_linear_chain (worker)

**File:** `worker/tests/test_executor.py`
**Context:** `_topo_sort` produces correct order for a linear dependency chain (A→B→C) even when nodes are listed in reverse order in the graph.
**Tests:** A graph with three nodes in reverse order (C, A, B) with linear dependencies is sorted.
**Inputs:** Nodes listed as [C, A, B] with C→B, B→A dependencies.
**Expected output:** Sorted order is `["A", "B", "C"]`.
**Acceptance command:** `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_executor.py::test_topo_sort_linear_chain -v` exits 0.

## test_topo_sort_diamond (worker)

**File:** `worker/tests/test_executor.py`
**Context:** `_topo_sort` handles diamond dependencies correctly: A→B, B→{C,D}. A must come first, B second, C and D after B.
**Tests:** A diamond graph with nodes listed in arbitrary order is topologically sorted.
**Inputs:** Nodes [D, C, A, B] with dependencies D→B, C→B, B→A.
**Expected output:** A at index 0, B at index 1, {C, D} at indices 2-3.
**Acceptance command:** `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_executor.py::test_topo_sort_diamond -v` exits 0.

## test_run_graph_empty_graph (worker)

**File:** `worker/tests/test_executor.py`
**Context:** `run_graph` handles an empty node list gracefully — no nodes to execute means immediate return.
**Tests:** A graph with `"nodes": []` is executed.
**Inputs:** Empty graph.
**Expected output:** Function returns without error or exception.
**Acceptance command:** `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_executor.py::test_run_graph_empty_graph -v` exits 0.

## test_completed_event_updates_job_status (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/event_loop_tests.rs`
**Context:** The event loop receives `WorkerEvent::Completed` from the broadcaster's worker event channel and processes it: updating the job status to `completed`, setting `completed_at`, releasing VRAM reservation, and broadcasting `WsEvent::JobCompleted`. The test verifies all four outcomes.
**Tests:** Submits a job, manually sets it to `Running` in the database (simulating dispatch), starts the event loop, sends a `WorkerEvent::Completed{job_id, elapsed_ms: 1234}` through the broadcaster, then verifies: (1) DB status is `completed`, (2) `completed_at` is set, (3) VRAM reservation is released (0 MiB), (4) `WsEvent::JobCompleted` is broadcast on the WsEvent channel.
**Inputs:** `WorkerEvent::Completed{job_id, elapsed_ms: 1234}`, in-memory DB with a Running job, VRAM reserved (4096 MiB on device 0).
**Expected output:** DB status=`completed`, `completed_at` is set, reservation=0 MiB, `WsEvent::JobCompleted{job_id, elapsed_ms: 1234}` received.
**Acceptance command:** `cargo test -p anvilml-scheduler --features mock-hardware -- event_loop_tests::test_completed_event_updates_job_status` exits 0.

## test_failed_event_updates_job_status (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/event_loop_tests.rs`
**Context:** The event loop receives `WorkerEvent::Failed` from the broadcaster's worker event channel and processes it: updating the job status to `failed`, storing the error message, releasing VRAM reservation, and broadcasting `WsEvent::JobFailed`. The test verifies all four outcomes.
**Tests:** Submits a job, manually sets it to `Running` in the database, starts the event loop, sends a `WorkerEvent::Failed{job_id, error: "test failure", traceback: Some(...)}` through the broadcaster, then verifies: (1) DB status is `failed`, (2) `error` column is `"test failure"`, (3) VRAM reservation is released, (4) `WsEvent::JobFailed` is broadcast.
**Inputs:** `WorkerEvent::Failed{job_id, error: "test failure", traceback: Some("Traceback...")}`, in-memory DB with a Running job, VRAM reserved (4096 MiB on device 0).
**Expected output:** DB status=`failed`, error=`"test failure"`, reservation=0 MiB, `WsEvent::JobFailed{job_id, error: "test failure"}` received.
**Acceptance command:** `cargo test -p anvilml-scheduler --features mock-hardware -- event_loop_tests::test_failed_event_updates_job_status` exits 0.

## test_event_loop_ignores_unknown_event (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/event_loop_tests.rs`
**Context:** The event loop gracefully ignores `WorkerEvent` variants it doesn't yet handle (Pong, Ready, Progress, etc.). It logs at DEBUG and continues processing future events without crashing.
**Tests:** Submits a job, manually sets it to `Running` in the database, starts the event loop, sends a `WorkerEvent::Pong{seq: 42}` through the broadcaster, then verifies: (1) job status is still `running`, (2) VRAM reservation is unchanged (4096 MiB), (3) no `WsEvent` was broadcast.
**Inputs:** `WorkerEvent::Pong{seq: 42}`, in-memory DB with a Running job, VRAM reserved (4096 MiB on device 0).
**Expected output:** Job remains `running`, reservation=4096 MiB, no WsEvent broadcast.
**Acceptance command:** `cargo test -p anvilml-scheduler --features mock-hardware -- event_loop_tests::test_event_loop_ignores_unknown_event` exits 0.

## test_save_and_get_roundtrip (anvilml-server)

**File:** `crates/anvilml-server/tests/artifact_store_tests.rs`
**Context:** `ArtifactStore` is created with a fresh in-memory SQLite pool and a temporary directory. Tests the full save-and-get lifecycle: persist image bytes to disk, record metadata in the database, then retrieve the file path and verify content.
**Tests:** Creates a 64-byte PNG-like image, calls `save(job_id, bytes)`, calls `get(hash)`, verifies the returned path exists on disk, and reads the file to confirm the content matches the input bytes exactly.
**Inputs:** 64-byte byte slice (`0x89..=0xBE`), random `Uuid::v4()` for job_id.
**Expected output:** `get(hash)` returns `Some(PathBuf)`; file at path contains exact input bytes.
**Acceptance command:** `cargo test -p anvilml-server --features mock-hardware -- test_save_and_get_roundtrip` exits 0.

## test_hash_is_deterministic (anvilml-server)

**File:** `crates/anvilml-server/tests/artifact_store_tests.rs`
**Context:** `ArtifactStore` is created with a fresh in-memory pool and temp directory. Verifies that SHA-256 content-addressing produces the same hash for identical input bytes, regardless of job_id.
**Tests:** Calls `save` twice with identical bytes but different job_ids, and asserts both `ArtifactMeta` results have the same `hash` field.
**Inputs:** 256-byte byte slice (`0x00..=0xFF`), two distinct `Uuid::v4()` values.
**Expected output:** `meta1.hash == meta2.hash` — SHA-256 is deterministic.
**Acceptance command:** `cargo test -p anvilml-server --features mock-hardware -- test_hash_is_deterministic` exits 0.

## test_list_returns_saved_artifact (anvilml-server)

**File:** `crates/anvilml-server/tests/artifact_store_tests.rs`
**Context:** `ArtifactStore` is created with a fresh in-memory pool and temp directory. Verifies the `list` method correctly returns saved artifacts with accurate metadata.
**Tests:** Saves an artifact with a known job_id and known byte length, then calls `list(None)` and asserts the returned vec has exactly one entry with the correct job_id and size_bytes.
**Inputs:** 96-byte byte slice (`0xAA..=0xFF`), a `Uuid::v4()` for job_id.
**Expected output:** `list(None)` returns `Vec<ArtifactMeta>` of length 1 with matching job_id and size_bytes.
**Acceptance command:** `cargo test -p anvilml-server --features mock-hardware -- test_list_returns_saved_artifact` exits 0.

## test_save_is_idempotent (anvilml-server)

**File:** `crates/anvilml-server/tests/artifact_store_tests.rs`
**Context:** `ArtifactStore` is created with a fresh in-memory pool and temp directory. Verifies that `INSERT OR IGNORE` prevents duplicate database rows when the same image is saved twice.
**Tests:** Calls `save` twice with identical bytes and the same job_id, asserts both return the same hash, then calls `list(None)` and verifies exactly one row exists.
**Inputs:** 80-byte byte slice (`0x10..=0x5F`), one `Uuid::v4()` used for both saves.
**Expected output:** Both saves return the same hash; `list(None)` returns exactly 1 artifact (no duplicates).
**Acceptance command:** `cargo test -p anvilml-server --features mock-hardware -- test_save_is_idempotent` exits 0.

## test_get_returns_none_for_unknown_hash (anvilml-server)

**File:** `crates/anvilml-server/tests/artifact_store_tests.rs`
**Context:** `ArtifactStore` is created with a fresh in-memory pool and temp directory. No artifacts have been saved. Verifies that `get` returns `None` for unknown hashes without erroring.
**Tests:** Calls `get("nonexistent_hash")` with a hash that was never saved and asserts the result is `None`.
**Inputs:** String `"nonexistent_hash_abcdef1234567890"`.
**Expected output:** `get()` returns `None` — no error, no panic.
**Acceptance command:** `cargo test -p anvilml-server --features mock-hardware -- test_get_returns_none_for_unknown_hash` exits 0.

## test_list_artifacts_empty (anvilml-server)

**File:** `crates/anvilml-server/tests/artifacts_tests.rs`
**Context:** The `GET /v1/artifacts` handler returns an empty JSON array when the artifact store has no artifacts. Uses a real TCP listener with `axum::serve` and raw TCP streams (same pattern as `handler_tests.rs`).
**Tests:** Builds the router with a default `AppState` containing an empty `ArtifactStore`, sends `GET /v1/artifacts`, asserts HTTP 200, `Content-Type: application/json`, and body `[]`.
**Inputs:** `GET /v1/artifacts`, `AppState::new("test-version")` with empty store.
**Expected output:** HTTP 200, `Content-Type: application/json`, body `[]`.
**Acceptance command:** `cargo test -p anvilml-server --features mock-hardware --test artifacts_tests -- test_list_artifacts_empty --exact` exits 0.

## test_list_artifacts_filtered (anvilml-server)

**File:** `crates/anvilml-server/tests/artifacts_tests.rs`
**Context:** The `GET /v1/artifacts?job_id=<uuid>` handler filters artifacts by job ID. Uses a real TCP listener. Two artifacts are saved via `ArtifactStore` with different job IDs, then the list endpoint is called with and without the filter.
**Tests:** Saves two artifacts with different `job_id` values, calls `GET /v1/artifacts?job_id=<id1>` and asserts exactly 1 artifact returned, then calls `GET /v1/artifacts` (no filter) and asserts exactly 2 artifacts returned.
**Inputs:** Two artifacts saved via `ArtifactStore::save()` with distinct `Uuid::v4()` job IDs.
**Expected output:** Filtered list returns 1 artifact; unfiltered list returns 2 artifacts.
**Acceptance command:** `cargo test -p anvilml-server --features mock-hardware --test artifacts_tests -- test_list_artifacts_filtered --exact` exits 0.

## test_serve_artifact_returns_png (anvilml-server)

**File:** `crates/anvilml-server/tests/artifacts_tests.rs`
**Context:** The `GET /v1/artifacts/:hash` handler serves raw artifact bytes with `Content-Type: image/png`. Uses a real TCP listener. One artifact is saved via `ArtifactStore`, then served by hash.
**Tests:** Saves an artifact, retrieves its hash from the returned `ArtifactMeta`, calls `GET /v1/artifacts/<hash>`, asserts HTTP 200, `Content-Type: image/png`, non-empty body, and body matches the original bytes exactly.
**Inputs:** 20-byte PNG-like byte sequence saved via `ArtifactStore::save()`.
**Expected output:** HTTP 200, `Content-Type: image/png`, body length 20, body bytes identical to original.
**Acceptance command:** `cargo test -p anvilml-server --features mock-hardware --test artifacts_tests -- test_serve_artifact_returns_png --exact` exits 0.

## test_serve_artifact_not_found (anvilml-server)

**File:** `crates/anvilml-server/tests/artifacts_tests.rs`
**Context:** The `GET /v1/artifacts/:hash` handler returns HTTP 404 with `error: "artifact_not_found"` when the hash does not match any saved artifact. Uses a real TCP listener with an empty store.
**Tests:** Sends `GET /v1/artifacts/<fake_hash>` with a 64-char hex string that was never saved, asserts HTTP 404 and `error` field is `"artifact_not_found"`.
**Inputs:** `GET /v1/artifacts/0000000000000000000000000000000000000000000000000000000000000000`, empty store.
**Expected output:** HTTP 404, JSON body with `"error": "artifact_not_found"`.
**Acceptance command:** `cargo test -p anvilml-server --features mock-hardware --test artifacts_tests -- test_serve_artifact_not_found --exact` exits 0.

## test_artifact_not_found_status_code (anvilml-core)

**File:** `crates/anvilml-core/tests/error_tests.rs`
**Context:** `AnvilError::ArtifactNotFound` maps to HTTP 404 — the artifact resource does not exist in the store.
**Tests:** `AnvilError::ArtifactNotFound("abc123".to_string()).status_code()` returns `StatusCode::NOT_FOUND`.
**Inputs:** `AnvilError::ArtifactNotFound("abc123".to_string())`.
**Expected output:** `status_code() == StatusCode::NOT_FOUND`.
**Acceptance command:** `cargo test -p anvilml-core --features mock-hardware -- test_artifact_not_found_status_code` exits 0.

## test_save_and_get (anvilml-artifacts)

**File:** `crates/anvilml-artifacts/tests/store_tests.rs`
**Context:** `ArtifactStore::save()` persists an image file by SHA-256 hash and records metadata in the `artifacts` SQLite table. `get()` retrieves the file path by hash. Each test uses its own in-memory pool with `max_connections(1)` and a unique temp directory for artifact storage.
**Tests:** Creates a 128-byte PNG-like artifact, saves it via `store.save()`, then retrieves it via `store.get()` and asserts the path exists on disk and matches the expected `{dir}/{hash}.png` pattern. Verifies the hash is a 64-character lowercase hex string.
**Inputs:** 128-byte byte vector, new UUID job ID, in-memory SQLite pool, temp directory.
**Expected output:** `get()` returns `Some(PathBuf)` pointing to `{temp_dir}/{hash}.png`, hash is 64 lowercase hex chars.
**Acceptance command:** `cargo test -p anvilml-artifacts --test store_tests -- test_save_and_get --exact` exits 0.

## test_save_idempotency (anvilml-artifacts)

**File:** `crates/anvilml-artifacts/tests/store_tests.rs`
**Context:** `save()` is idempotent: saving identical bytes twice writes the file only once and produces a single database row (due to `INSERT OR IGNORE` on the `hash` UNIQUE constraint). Each test uses its own in-memory pool and temp directory.
**Tests:** Saves the same 256-byte image twice under two different job IDs, asserts both produce the same hash, then calls `list(None)` and verifies exactly one artifact is returned.
**Inputs:** Same 256-byte vector, two different UUID job IDs.
**Expected output:** Both saves produce identical hash; `list(None)` returns a vec of length 1.
**Acceptance command:** `cargo test -p anvilml-artifacts --test store_tests -- test_save_idempotency --exact` exits 0.

## test_list_all (anvilml-artifacts)

**File:** `crates/anvilml-artifacts/tests/store_tests.rs`
**Context:** `list(None)` returns all artifacts without filtering. Uses `SELECT * FROM artifacts` when no job filter is specified. Each test uses its own in-memory pool and temp directory.
**Tests:** Saves three artifacts for three different jobs, calls `list(None)`, and asserts the returned vec has exactly 3 elements.
**Inputs:** Three artifact byte vectors, three distinct UUID job IDs.
**Expected output:** `list(None)` returns a vec of length 3.
**Acceptance command:** `cargo test -p anvilml-artifacts --test store_tests -- test_list_all --exact` exits 0.

## test_list_filtered (anvilml-artifacts)

**File:** `crates/anvilml-artifacts/tests/store_tests.rs`
**Context:** `list(Some(job_id))` appends `WHERE job_id = ?` to the SELECT query, filtering results to only artifacts belonging to the specified job. Each test uses its own in-memory pool and temp directory.
**Tests:** Saves three artifacts (two for job A, one for job B), calls `list(Some(job_a))` and asserts exactly 2 are returned with correct job IDs, then calls `list(Some(job_b))` and asserts exactly 1 is returned.
**Inputs:** Three artifact byte vectors, two distinct UUID job IDs.
**Expected output:** `list(Some(job_a))` returns 2 artifacts; `list(Some(job_b))` returns 1 artifact; all returned artifacts have the correct `job_id`.
**Acceptance command:** `cargo test -p anvilml-artifacts --test store_tests -- test_list_filtered --exact` exits 0.

## test_get_missing_hash (anvilml-artifacts)

**File:** `crates/anvilml-artifacts/tests/store_tests.rs`
**Context:** `get()` must return `None` (not an error) for a hash that was never saved. This distinguishes "not found" from "database error". Each test uses its own in-memory pool and temp directory.
**Tests:** Creates a fresh store with no artifacts, calls `get()` with an all-zeros 64-char hex hash, and asserts the result is `None`.
**Inputs:** All-zeros 64-character hex hash string.
**Expected output:** `get()` returns `None`.
**Acceptance command:** `cargo test -p anvilml-artifacts --test store_tests -- test_get_missing_hash --exact` exits 0.

## test_progress_events_emitted_in_mock_mode (worker)

**File:** `worker/tests/test_executor.py`
**Context:** `run_graph()` checks `EMITS_PROGRESS` on each node class after calling `execute()` and emits Progress events via `ctx.emit`. In mock mode (`ANVILML_WORKER_MOCK=1`), it emits exactly 3 Progress events (step=1,2,3, total_steps=3, preview_b64=None). The `conftest.py` autouse fixture sets `ANVILML_WORKER_MOCK=1` and the `registry_clean` autouse fixture clears `NODE_REGISTRY` before each test.
**Tests:** A graph with a single node whose class has `EMITS_PROGRESS = True` is executed. The executor emits exactly 3 Progress events in order before the node's execution completes. Each event has `_type="Progress"`, `job_id="test-job-1"`, correct `step` (1, 2, 3), `total_steps=3`, and `preview_b64=None`.
**Inputs:** Graph with single `StepNode` (a dynamically-created test node with `EMITS_PROGRESS=True`).
**Expected output:** 3 Progress events captured by the emit capture, each with correct fields and in sequential order.
**Acceptance command:** `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_executor.py -v` exits 0.

## test_full_event_sequence_order (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/progress_tests.rs`
**Context:** The event loop now handles `WorkerEvent::Progress` by relaying it to WebSocket clients via `WsEvent::JobProgress`. The dispatch loop also broadcasts `WsEvent::JobStarted` after sending an Execute message. This test verifies the complete observable lifecycle sequence: JobStarted → JobProgress×3 → JobCompleted, with correct field values for each event.
**Tests:** Creates in-memory DB, scheduler, registry with LoadModel node. Subscribes to WsEvent channel. Starts event loop. Submits a job and manually sets it Running. Sends events in order: JobStarted (direct WsEvent broadcast), Progress(step=1, no preview), Progress(step=2, with preview), Progress(step=3, with preview), Completed. Collects all events via `ws_rx.recv()` with timeouts. Asserts each event's variant and field values.
**Inputs:** Job with LoadModel graph, 3 Progress events (steps 1-3, total=3), 1 Completed event (elapsed_ms=4567).
**Expected output:** 4 WsEvent variants received in correct order: JobStarted{job_id, worker_id="worker-0"}, JobProgress{step=1, preview=None}, JobProgress{step=2, preview=Some("preview-step-2")}, JobProgress{step=3, preview=Some("preview-step-3")}. Job status transitions to "completed". VRAM released.
**Acceptance command:** `cargo test -p anvilml-scheduler --features mock-hardware -- test_full_event_sequence_order` exits 0.

## test_progress_no_preview (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/progress_tests.rs`
**Context:** The most common Progress event has no preview image (preview_b64=None). This test verifies that such events are relayed correctly and do not affect job status or VRAM reservations.
**Tests:** Creates in-memory DB, scheduler, registry. Subscribes to WsEvent channel. Starts event loop. Submits a job, sets it Running. Sends a single Progress event (step=5, total_steps=50, preview_b64=None). Verifies WsEvent::JobProgress is broadcast with correct fields. Verifies job status remains "running" and VRAM reservation is unchanged.
**Inputs:** Progress event with step=5, total_steps=50, preview_b64=None.
**Expected output:** WsEvent::JobProgress{job_id, step=5, total_steps=50, preview_b64=None} broadcast. Job status remains "running". VRAM reservation unchanged at 4096 MiB.
**Acceptance command:** `cargo test -p anvilml-scheduler --features mock-hardware -- test_progress_no_preview` exits 0.

## test_cancel_queued_job (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/scheduler_cancel_tests.rs`
**Context:** Cancelling a Queued job removes it from the in-memory queue, updates the DB to 'cancelled', sets completed_at, and broadcasts WsEvent::JobCancelled. The scheduler is built without a worker pool (workers=None) since cancellation of queued jobs doesn't need IPC.
**Tests:** Submits a job (Queued in DB). Subscribes to WsEvent channel. Calls `cancel_job()`. Verifies queue is empty, DB status is 'cancelled', completed_at is set, and WsEvent::JobCancelled is broadcast with matching job_id.
**Inputs:** Valid job UUID from submit().
**Expected output:** Queue empty, DB status='cancelled', completed_at set, WsEvent::JobCancelled{job_id} broadcast.
**Acceptance command:** `cargo test -p anvilml-scheduler --features mock-hardware -- scheduler_cancel_tests::test_cancel_queued_job` exits 0.

## test_cancel_running_job_fails_without_worker (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/scheduler_cancel_tests.rs`
**Context:** Cancelling a Running job sends a CancelJob IPC message to the owning worker via the worker pool. When the worker pool's transport has no connected workers, the IPC send fails with an AnvilError::Ipc error. The test verifies the error is propagated and the job remains Running.
**Tests:** Submits a job, manually sets it Running in DB. Creates a WorkerPool with a Busy worker (no connected workers). Passes the pool to the scheduler. Calls `cancel_job()`. Verifies the result is an Err(AnvilError::Ipc) and the job status remains 'running'.
**Inputs:** Valid job UUID from submit(), manually set to Running status.
**Expected output:** Err(AnvilError::Ipc), job status remains 'running' (cancel not confirmed).
**Acceptance command:** `cargo test -p anvilml-scheduler --features mock-hardware -- scheduler_cancel_tests::test_cancel_running_job_fails_without_worker` exits 0.

## test_cancel_terminal_job_returns_error (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/scheduler_cancel_tests.rs`
**Context:** Cancelling a job in a terminal state (Completed, Failed, Cancelled) returns AnvilError::InvalidOperation with HTTP 409 Conflict. This prevents clients from cancelling already-finished jobs.
**Tests:** Submits a job, manually sets it to Completed in DB. Calls `cancel_job()`. Verifies the error has status code 409 and error_kind "invalid_operation". Verifies DB status remains 'completed'.
**Inputs:** Valid job UUID from submit(), manually set to Completed status.
**Expected output:** Err(InvalidOperation), status code 409, DB status remains 'completed'.
**Acceptance command:** `cargo test -p anvilml-scheduler --features mock-hardware -- scheduler_cancel_tests::test_cancel_terminal_job_returns_error` exits 0.

## test_cancel_unknown_job_returns_404 (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/scheduler_cancel_tests.rs`
**Context:** Cancelling a job that doesn't exist in the database returns AnvilError::JobNotFound with HTTP 404 Not Found.
**Tests:** Generates a random UUID that doesn't exist in the DB. Calls `cancel_job()` with that UUID. Verifies the error has status code 404 and error_kind "job_not_found".
**Inputs:** Random UUID (non-existent in DB).
**Expected output:** Err(JobNotFound), status code 404.
**Acceptance command:** `cargo test -p anvilml-scheduler --features mock-hardware -- scheduler_cancel_tests::test_cancel_unknown_job_returns_404` exits 0.

## test_cancelled_event_releases_vram (anvilml-scheduler)

**File:** `crates/anvilml-scheduler/tests/scheduler_cancel_tests.rs`
**Context:** The event loop's Cancelled event handler updates the job status to 'cancelled', releases VRAM reservation, and broadcasts WsEvent::JobCancelled. This is the async confirmation path for running-job cancellations.
**Tests:** Creates in-memory DB, scheduler, registry. Registers device with 16384 MiB VRAM and reserves 4096 MiB. Submits a job, sets it Running. Starts event loop. Sends a Cancelled event via broadcaster. Verifies DB status is 'cancelled', VRAM released (reservation = 0), and WsEvent::JobCancelled is broadcast.
**Inputs:** Valid job UUID from submit(), manually set to Running, VRAM reserved at 4096 MiB.
**Expected output:** DB status='cancelled', VRAM reservation=0, WsEvent::JobCancelled{job_id} broadcast.
**Acceptance command:** `cargo test -p anvilml-scheduler --features mock-hardware -- scheduler_cancel_tests::test_cancelled_event_releases_vram` exits 0.
