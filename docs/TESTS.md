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
**Tests:** A TOML file with `port = 9001` is loaded while `ANVILML_PORT=8080` is set; the env var value wins.
**Inputs:** TOML file with `port = 9001`, `ANVILML_PORT=8080`, `overrides = ConfigOverrides::default()`.
**Expected output:** `cfg.port == 8080` (env beats toml).

## test_cli_override_beats_env (anvilml-core)

**File:** `crates/anvilml-core/tests/config_load_tests.rs`
**Context:** CLI overrides take precedence over environment variables, which take precedence over TOML.
**Tests:** A TOML file with `port = 9001`, env `ANVILML_PORT=8080`, and `overrides.port = Some(7070)` — the CLI override wins.
**Inputs:** TOML `port = 9001`, `ANVILML_PORT=8080`, `overrides.port = Some(7070)`.
**Expected output:** `cfg.port == 7070` (CLI beats env beats toml).

## test_nested_env_var (anvilml-core)

**File:** `crates/anvilml-core/tests/config_load_tests.rs`
**Context:** Double-underscore nesting in env vars correctly maps to nested config fields.
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
