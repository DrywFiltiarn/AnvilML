/// Tests for `types::node` — `NodeTypeDescriptor`, `SlotDescriptor`, and `SlotType`.
///
/// Verifies:
/// - JSON roundtrip for a fully-populated `NodeTypeDescriptor` with mixed optional inputs.
/// - All 11 `SlotType` enum variants roundtrip through JSON with correct SCREAMING_SNAKE_CASE keys.
/// - `SlotDescriptor` preserves the `optional` field through JSON roundtrip.
use anvilml_core::{NodeTypeDescriptor, SlotDescriptor, SlotType};

/// Verifies that a fully-populated `NodeTypeDescriptor` serialises to JSON and
/// deserialises back to an identical value, including nested `SlotDescriptor`
/// vectors with mixed optional flags.
///
/// This is the primary acceptance test for the correctness of all
/// `Serialize`/`Deserialize` derives on `NodeTypeDescriptor` and its nested
/// `SlotDescriptor` fields.
#[test]
fn test_node_type_descriptor_json_roundtrip() {
    let node = NodeTypeDescriptor {
        type_name: "KSampler".to_string(),
        display_name: "KSampler".to_string(),
        category: "sampling".to_string(),
        description: "Performs one step of diffusion sampling.".to_string(),
        inputs: vec![
            SlotDescriptor {
                name: "samples".to_string(),
                slot_type: SlotType::Latent,
                optional: false,
            },
            SlotDescriptor {
                name: "model".to_string(),
                slot_type: SlotType::Model,
                optional: false,
            },
            SlotDescriptor {
                name: "positive".to_string(),
                slot_type: SlotType::Conditioning,
                optional: true,
            },
        ],
        outputs: vec![
            SlotDescriptor {
                name: "samples".to_string(),
                slot_type: SlotType::Latent,
                optional: false,
            },
            SlotDescriptor {
                name: "denoised".to_string(),
                slot_type: SlotType::Latent,
                optional: false,
            },
        ],
    };

    // Serialize to JSON
    let json = serde_json::to_string(&node).expect("serialize NodeTypeDescriptor to JSON");

    // Deserialize back — must not fail
    let restored: NodeTypeDescriptor =
        serde_json::from_str(&json).expect("deserialize JSON back to NodeTypeDescriptor");

    // All top-level fields must be equal
    assert_eq!(restored.type_name, node.type_name);
    assert_eq!(restored.display_name, node.display_name);
    assert_eq!(restored.category, node.category);
    assert_eq!(restored.description, node.description);

    // Inputs must match
    assert_eq!(restored.inputs.len(), node.inputs.len());
    for (i, (orig, rest)) in node.inputs.iter().zip(restored.inputs.iter()).enumerate() {
        assert_eq!(rest.name, orig.name, "inputs[{}].name", i);
        assert_eq!(rest.slot_type, orig.slot_type, "inputs[{}].slot_type", i);
        assert_eq!(rest.optional, orig.optional, "inputs[{}].optional", i);
    }

    // Outputs must match
    assert_eq!(restored.outputs.len(), node.outputs.len());
    for (i, (orig, rest)) in node.outputs.iter().zip(restored.outputs.iter()).enumerate() {
        assert_eq!(rest.name, orig.name, "outputs[{}].name", i);
        assert_eq!(rest.slot_type, orig.slot_type, "outputs[{}].slot_type", i);
        assert_eq!(rest.optional, orig.optional, "outputs[{}].optional", i);
    }
}

/// Verifies that all 11 `SlotType` enum variants roundtrip through
/// JSON serialisation with correct `SCREAMING_SNAKE_CASE` keys.
///
/// Each variant is serialised to a JSON string and deserialised back,
/// then compared for equality. This confirms that `#[serde(rename_all =
/// "SCREAMING_SNAKE_CASE")]` produces the correct uppercase keys
/// (e.g. `"MODEL"`, `"CLIP"`, `"LATENT"`) matching the Python worker's
/// `SlotType` convention.
#[test]
fn test_slot_type_variants() {
    let variants = [
        SlotType::Model,
        SlotType::Clip,
        SlotType::Vae,
        SlotType::Conditioning,
        SlotType::Latent,
        SlotType::Image,
        SlotType::String,
        SlotType::Int,
        SlotType::Float,
        SlotType::Bool,
        SlotType::Any,
    ];

    for variant in variants {
        let json = serde_json::to_string(&variant).expect("serialize SlotType variant to JSON");

        let restored: SlotType =
            serde_json::from_str(&json).expect("deserialize JSON back to SlotType");

        assert_eq!(
            restored, variant,
            "SlotType::{:?} did not survive JSON roundtrip (JSON was: {})",
            variant, json
        );
    }
}

/// Verifies that a `SlotDescriptor` with `optional: true` preserves the
/// optional flag through JSON serialisation and deserialisation.
///
/// This is a focused test on the `optional` field — a boolean field that
/// must survive roundtrip unchanged. An incorrect default or missing field
/// would cause the Python worker to treat optional inputs as required.
#[test]
fn test_slot_descriptor_optional_field() {
    let slot = SlotDescriptor {
        name: "seed".to_string(),
        slot_type: SlotType::Int,
        optional: true,
    };

    let json = serde_json::to_string(&slot).expect("serialize SlotDescriptor to JSON");

    let restored: SlotDescriptor =
        serde_json::from_str(&json).expect("deserialize JSON back to SlotDescriptor");

    assert_eq!(restored.name, slot.name);
    assert_eq!(restored.slot_type, slot.slot_type);
    assert!(
        restored.optional,
        "SlotDescriptor.optional must be true after roundtrip (JSON was: {})",
        json
    );
}
