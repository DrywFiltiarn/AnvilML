//! Node type descriptor types for the AnvilML node registry.
//!
//! Defines `NodeTypeDescriptor` (a complete description of a node type in the
//! Python worker's node registry), `SlotDescriptor` (metadata about a single
//! input or output slot), and `SlotType` (the data type of a slot). These types
//! are exchanged between the Python worker and the Rust supervisor during the
//! worker's `Ready` event to populate the scheduler's node registry.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// The data type of a slot in a node's input or output signature.
///
/// Each variant corresponds to a specific data category used in generative AI
/// pipelines. The `SCREAMING_SNAKE_CASE` JSON representation matches the Python
/// worker's `SlotType` convention exactly — this is critical for cross-language
/// compatibility between the Rust supervisor and Python worker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SlotType {
    /// A diffusion model (e.g. UNet, DiT).
    Model,
    /// A CLIP text encoder model.
    Clip,
    /// A variational autoencoder (VAE) for encoding/decoding latent space.
    Vae,
    /// A conditioning tensor (e.g. from a text encoder or control net).
    Conditioning,
    /// A latent space tensor (compressed representation of image data).
    Latent,
    /// A pixel-space image tensor.
    Image,
    /// A text string (e.g. prompt, negative prompt).
    String,
    /// A 32-bit integer value.
    Int,
    /// A 32-bit floating point value.
    Float,
    /// A boolean flag.
    Bool,
    /// Any data type — used for slots that accept multiple types.
    Any,
}

/// Metadata describing a single input or output slot of a node.
///
/// Each slot has a name, a `SlotType` specifying the expected data category,
/// and an `optional` flag indicating whether the slot may be left unconnected
/// (relevant for inputs). The optional flag is preserved through JSON
/// serialisation so the Python worker can distinguish required from optional
/// connections.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SlotDescriptor {
    /// Human-readable slot name (e.g. `"samples"`, `"prompt"`).
    pub name: String,
    /// The data type this slot carries.
    pub slot_type: SlotType,
    /// Whether this slot may be left unconnected. Only meaningful for inputs;
    /// outputs are always required.
    pub optional: bool,
}

/// A complete description of a node type in the Python worker's node registry.
///
/// Produced by the Python worker during its `Ready` event. Contains the node's
/// identity fields (`type_name`, `display_name`, `category`, `description`) and
/// its full input/output signature as lists of `SlotDescriptor`. The scheduler
/// uses this information to validate job graphs before dispatching them to
/// workers.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct NodeTypeDescriptor {
    /// Unique type identifier used by the node registry (e.g. `"KSampler"`).
    pub type_name: String,
    /// Human-readable display name for UI purposes (e.g. `"KSampler"`).
    pub display_name: String,
    /// Category this node belongs to (e.g. `"sampling"`, `"conditioning"`).
    pub category: String,
    /// Human-readable description of what this node does.
    pub description: String,
    /// List of input slots accepted by this node.
    pub inputs: Vec<SlotDescriptor>,
    /// List of output slots produced by this node.
    pub outputs: Vec<SlotDescriptor>,
}
