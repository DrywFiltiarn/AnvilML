pub mod config;
pub mod config_load;
pub mod error;
pub mod types;

pub use config::*;
pub use config_load::{load_config, ConfigError, ConfigOverrides};
pub use error::AnvilError;

// Re-export model and artifact domain types for convenience.
pub use types::artifact::ArtifactMeta;
pub use types::model::{DType, ModelKind, ModelMeta};

pub fn stub() {}
