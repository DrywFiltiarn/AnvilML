//! OpenAPI specification generator for AnvilML.
//!
//! Generates a complete OpenAPI 3.1 specification by building it programmatically
//! from handler path definitions and component schemas, then serializes the result
//! to pretty-printed JSON at `backend/openapi.json`.

use utoipa::openapi::path::PathsBuilder;
use utoipa::PartialSchema;

use anvilml_core::types::artifact::ArtifactMeta;
use anvilml_core::types::events::{
    GpuStatSnapshot, JobCancelledEvent, JobCompletedEvent, JobFailedEvent, JobImageReadyEvent,
    JobProgressEvent, JobQueuedEvent, JobStartedEvent, SystemStatsEvent, WorkerStatusChangedEvent,
    WsEvent,
};
use anvilml_core::types::hardware::{CapabilitySource, GpuDevice, HostInfo};
use anvilml_core::types::job::{Job, JobSettings, JobStatus, SubmitJobRequest, SubmitJobResponse};
use anvilml_core::types::model::DType;
use anvilml_core::{
    DeviceType, EnumerationSource, EnvReport, HardwareInfo, InferenceCaps, WorkerInfo, WorkerStatus,
};
use anvilml_server::handlers::artifacts;
use anvilml_server::handlers::health;
use anvilml_server::handlers::jobs;
use anvilml_server::handlers::models;
use anvilml_server::handlers::system;
use anvilml_server::handlers::workers;
use anvilml_server::ws::handler;

fn main() {
    // Build the OpenAPI spec programmatically by collecting path info from
    // handler functions that have #[utoipa::path] attributes.
    let paths = PathsBuilder::new()
        .path_from::<health::__path_health>()
        .path_from::<system::__path_get_env>()
        .path_from::<system::__path_get_system>()
        .path_from::<jobs::__path_submit_job>()
        .path_from::<jobs::__path_get_job>()
        .path_from::<jobs::__path_cancel_job>()
        .path_from::<jobs::__path_delete_job>()
        .path_from::<jobs::__path_list_jobs>()
        .path_from::<jobs::__path_clear_jobs>()
        .path_from::<models::__path_list_models>()
        .path_from::<models::__path_get_model>()
        .path_from::<models::__path_rescan_models>()
        .path_from::<workers::__path_list_workers>()
        .path_from::<workers::__path_restart_worker>()
        .path_from::<artifacts::__path_list_artifacts>()
        .path_from::<artifacts::__path_serve_artifact>()
        .path_from::<handler::__path_ws_events>()
        .build();

    // Build components with all schema types
    let components = utoipa::openapi::Components::builder()
        .schema("HealthResponse", health::HealthResponse::schema())
        .schema("EnvReport", EnvReport::schema())
        .schema("HardwareInfo", HardwareInfo::schema())
        .schema("HostInfo", HostInfo::schema())
        .schema("GpuDevice", GpuDevice::schema())
        .schema("InferenceCaps", InferenceCaps::schema())
        .schema("DeviceType", DeviceType::schema())
        .schema("EnumerationSource", EnumerationSource::schema())
        .schema("CapabilitySource", CapabilitySource::schema())
        .schema("SubmitJobRequest", SubmitJobRequest::schema())
        .schema("SubmitJobResponse", SubmitJobResponse::schema())
        .schema("JobSettings", JobSettings::schema())
        .schema("Job", Job::schema())
        .schema("JobStatus", JobStatus::schema())
        .schema("ErrorInline", jobs::ErrorInline::schema())
        .schema("ClearJobsResponse", jobs::ClearJobsResponse::schema())
        .schema("RescanResponse", models::RescanResponse::schema())
        .schema("ModelMeta", anvilml_core::ModelMeta::schema())
        .schema("DType", DType::schema())
        .schema("WorkerInfo", WorkerInfo::schema())
        .schema("WorkerStatus", WorkerStatus::schema())
        .schema("ArtifactMeta", ArtifactMeta::schema())
        .schema("WsEvent", WsEvent::schema())
        .schema("SystemStatsEvent", SystemStatsEvent::schema())
        .schema("GpuStatSnapshot", GpuStatSnapshot::schema())
        .schema("JobQueuedEvent", JobQueuedEvent::schema())
        .schema("JobStartedEvent", JobStartedEvent::schema())
        .schema("JobProgressEvent", JobProgressEvent::schema())
        .schema("JobImageReadyEvent", JobImageReadyEvent::schema())
        .schema("JobCompletedEvent", JobCompletedEvent::schema())
        .schema("JobFailedEvent", JobFailedEvent::schema())
        .schema("JobCancelledEvent", JobCancelledEvent::schema())
        .schema(
            "WorkerStatusChangedEvent",
            WorkerStatusChangedEvent::schema(),
        )
        .build();

    let api = utoipa::openapi::OpenApi::builder()
        .info(
            utoipa::openapi::Info::builder()
                .title("AnvilML API")
                .description(Some("AnvilML — GPU job orchestration server API"))
                .version("0.1.0")
                .build(),
        )
        .paths(paths)
        .components(Some(components))
        .build();

    let json = serde_json::to_string_pretty(&api).expect("OpenAPI spec must serialize");

    // Compute output path relative to workspace root.
    // CARGO_MANIFEST_DIR = .../crates/anvilml-openapi
    // Workspace root = CARGO_MANIFEST_DIR/../../
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR must be set");
    let workspace_root = std::path::Path::new(&manifest_dir)
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let output_path = workspace_root.join("backend/openapi.json");

    std::fs::write(&output_path, json).expect("must write openapi.json");

    println!("Generated OpenAPI spec: {}", output_path.display());
}
