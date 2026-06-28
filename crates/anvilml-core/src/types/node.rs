use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// The semantic type of a node slot.
///
/// Used by the scheduler to verify connected slots are type-compatible
/// at job submission time. `SlotType::Any` disables type checking for
/// that slot entirely, allowing any connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SlotType {
    /// A model weights tensor (UNet, transformer, etc.).
    Model,
    /// A CLIP / T5 text embedding.
    Clip,
    /// A VAE (Variational Autoencoder) latent or checkpoint.
    Vae,
    /// Conditioning data (e.g. cross-attention context from a text encoder).
    Conditioning,
    /// A latent tensor (compressed spatial representation).
    Latent,
    /// An image tensor (RGB pixels).
    Image,
    /// A free-form text string.
    String,
    /// A signed integer value.
    Int,
    /// A 32-bit or 64-bit floating point value.
    Float,
    /// A boolean true/false value.
    Bool,
    /// Disables type checking for this slot. Any type may connect here.
    Any,
}

/// Describes one input or output slot on a node type.
///
/// Each `SlotDescriptor` defines a slot's name, semantic type, and
/// whether it may be omitted. Optional slots enable a node to use
/// an internal default when the caller does not provide a value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SlotDescriptor {
    /// The slot's identifier within the node type (e.g. "positive").
    pub name: String,
    /// The semantic type this slot carries.
    pub slot_type: SlotType,
    /// True if this input can be omitted in favor of a node-internal default.
    pub optional: bool,
}

/// Description of a node type as reported by the Python worker at Ready.
///
/// A `NodeTypeDescriptor` captures the shape of a node: its unique
/// identifier, human-facing metadata, and the typed input/output slots
/// that the scheduler uses to validate graph connectivity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct NodeTypeDescriptor {
    /// Unique identifier for this node type (e.g. "LoadModel").
    pub type_name: String,
    /// Human-readable display name (e.g. "Load Checkpoint").
    pub display_name: String,
    /// The node's category in the UI (e.g. "loaders", "conditioning").
    pub category: String,
    /// A human-readable description of what this node does.
    pub description: String,
    /// The node's typed input slots.
    pub inputs: Vec<SlotDescriptor>,
    /// The node's typed output slots.
    pub outputs: Vec<SlotDescriptor>,
}
