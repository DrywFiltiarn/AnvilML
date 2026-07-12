//! OpenAPI schema generation via utoipa derive macros.
//!
//! The `ApiDoc` struct derives `utoipa::OpenApi` and lists every handler
//! path, every response schema, and every API tag. This is used by the
//! `anvilml-openapi` binary to produce a machine-readable OpenAPI 3.1
//! JSON document at `api/openapi.json`.

use utoipa::OpenApi;

/// OpenAPI 3.1 schema document for the AnvilML HTTP API.
///
/// Derives `utoipa::OpenApi` to automatically generate the complete
/// OpenAPI specification from `#[utoipa::path]` annotations on handler
/// functions and `#[derive(ToSchema)]` on response types. The generated
/// spec is written to `api/openapi.json` by the `anvilml-openapi` binary.
#[derive(OpenApi)]
#[openapi(
    paths(
        crate::handlers::health::health,
        crate::handlers::system::get_system,
        crate::handlers::system::get_system_env,
        crate::handlers::system::get_system_versions,
        crate::handlers::jobs::submit_job,
        crate::handlers::jobs::list_jobs,
        crate::handlers::jobs::get_job,
        crate::handlers::jobs::cancel_job,
        crate::handlers::jobs::delete_job,
        crate::handlers::jobs::bulk_clear_jobs,
        crate::handlers::models::list_models,
        crate::handlers::models::get_model,
        crate::handlers::models::rescan_models,
        crate::handlers::workers::list_workers,
        crate::handlers::workers::restart_worker,
        crate::handlers::artifacts::list_artifacts,
        crate::handlers::artifacts::get_artifact,
        crate::handlers::nodes::list_nodes,
    ),
    components(
        schemas(
            crate::handlers::health::HealthResponse,
            crate::handlers::system::ComponentVersions,
            crate::handlers::jobs::SubmitJobRequest,
            crate::handlers::jobs::SubmitJobResponse,
            crate::handlers::jobs::ListJobsParams,
            crate::handlers::jobs::BulkClearParams,
            crate::handlers::jobs::RemovedCount,
            crate::handlers::models::ListModelsParams,
            crate::handlers::artifacts::ListArtifactsParams,
            anvilml_core::Job,
            anvilml_core::JobStatus,
            anvilml_core::ModelMeta,
            anvilml_core::ModelKind,
            anvilml_core::ModelDtype,
            anvilml_core::ModelFormat,
            anvilml_core::HardwareInfo,
            anvilml_core::EnvReport,
            anvilml_core::WorkerInfo,
            anvilml_core::types::worker::WorkerStatus,
            anvilml_core::ArtifactMeta,
            anvilml_core::NodeTypeDescriptor,
            anvilml_core::types::hardware::InferenceCaps,
            anvilml_core::types::hardware::CapabilitySource,
            anvilml_core::types::node::SlotType,
        )
    ),
    tags(
        (name = "Health", description = "Health check endpoints"),
        (name = "System", description = "System information endpoints"),
        (name = "Jobs", description = "Job management endpoints"),
        (name = "Models", description = "Model registry endpoints"),
        (name = "Workers", description = "Worker management endpoints"),
        (name = "Artifacts", description = "Artifact storage endpoints"),
        (name = "Nodes", description = "Node type registry endpoints"),
    ),
)]
pub struct ApiDoc;
