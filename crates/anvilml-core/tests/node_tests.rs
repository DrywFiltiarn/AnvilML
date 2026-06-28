//! Tests for `NodeTypeDescriptor`, `SlotDescriptor`, and `SlotType` serde roundtrips.
//!
//! All tests construct types via the public API, serialise to JSON,
//! deserialise back, and assert equality. No I/O or env vars are used.

use anvilml_core::types::*;

/// Each of the eleven `SlotType` variants serialises to the correct
/// `SCREAMING_SNAKE_CASE` JSON string and roundtrips back to an equal value.
#[test]
fn test_slot_type_screaming_snake_case_serde() {
    let variants: [(SlotType, &str); 11] = [
        (SlotType::Model, "MODEL"),
        (SlotType::Clip, "CLIP"),
        (SlotType::Vae, "VAE"),
        (SlotType::Conditioning, "CONDITIONING"),
        (SlotType::Latent, "LATENT"),
        (SlotType::Image, "IMAGE"),
        (SlotType::String, "STRING"),
        (SlotType::Int, "INT"),
        (SlotType::Float, "FLOAT"),
        (SlotType::Bool, "BOOL"),
        (SlotType::Any, "ANY"),
    ];

    for (slot_type, expected_json) in variants {
        let json = serde_json::to_string(&slot_type).expect("failed to serialise SlotType");
        assert_eq!(
            json,
            format!("\"{expected_json}\""),
            "SlotType::{:?} JSON mismatch",
            slot_type
        );

        let roundtripped: SlotType =
            serde_json::from_str(&json).expect("failed to deserialise SlotType");
        assert_eq!(
            roundtripped, slot_type,
            "SlotType::{:?} roundtrip mismatch",
            slot_type
        );
    }
}

/// A `SlotDescriptor` with a required slot (`optional: false`) and an
/// optional slot (`optional: true`) both serialise to JSON with the
/// correct field names (`name`, `slot_type`, `optional`) and roundtrip
/// back to equal values.
#[test]
fn test_slot_descriptor_serde_roundtrip() {
    let required = SlotDescriptor {
        name: "positive".to_string(),
        slot_type: SlotType::Conditioning,
        optional: false,
    };

    let optional = SlotDescriptor {
        name: "seed".to_string(),
        slot_type: SlotType::Int,
        optional: true,
    };

    for (slot, label) in [(required, "required"), (optional, "optional")] {
        let json = serde_json::to_string(&slot).expect("failed to serialise SlotDescriptor");
        let roundtripped: SlotDescriptor =
            serde_json::from_str(&json).expect("failed to deserialise SlotDescriptor");
        assert_eq!(
            roundtripped, slot,
            "{label} SlotDescriptor roundtrip mismatch"
        );

        // Verify JSON field names.
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("json is valid");
        assert!(
            parsed.get("name").is_some(),
            "{label}: missing 'name' field"
        );
        assert!(
            parsed.get("slot_type").is_some(),
            "{label}: missing 'slot_type' field"
        );
        assert!(
            parsed.get("optional").is_some(),
            "{label}: missing 'optional' field"
        );
    }
}

/// A `NodeTypeDescriptor` modelled after `LoadModel` — one required
/// `model_id` input and one `MODEL` output — serialises to JSON,
/// roundtrips back to an equal value, and contains all expected
/// top-level field names (`type_name`, `display_name`, `category`,
/// `description`, `inputs`, `outputs`).
#[test]
fn test_node_type_descriptor_construction() {
    let node = NodeTypeDescriptor {
        type_name: "LoadModel".to_string(),
        display_name: "Load Checkpoint".to_string(),
        category: "loaders".to_string(),
        description: "Loads a model checkpoint from disk.".to_string(),
        inputs: vec![SlotDescriptor {
            name: "model_id".to_string(),
            slot_type: SlotType::String,
            optional: false,
        }],
        outputs: vec![SlotDescriptor {
            name: "model".to_string(),
            slot_type: SlotType::Model,
            optional: false,
        }],
    };

    let json = serde_json::to_string(&node).expect("failed to serialise NodeTypeDescriptor");
    let roundtripped: NodeTypeDescriptor =
        serde_json::from_str(&json).expect("failed to deserialise NodeTypeDescriptor");
    assert_eq!(
        roundtripped, node,
        "roundtripped NodeTypeDescriptor does not equal original"
    );

    // Verify the JSON contains all expected top-level field names.
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("json is valid");
    assert_eq!(parsed["type_name"], "LoadModel");
    assert_eq!(parsed["display_name"], "Load Checkpoint");
    assert_eq!(parsed["category"], "loaders");
    assert_eq!(parsed["description"], "Loads a model checkpoint from disk.");
    assert!(parsed["inputs"].is_array(), "inputs must be a JSON array");
    assert_eq!(parsed["inputs"].as_array().unwrap().len(), 1);
    assert!(parsed["outputs"].is_array(), "outputs must be a JSON array");
    assert_eq!(parsed["outputs"].as_array().unwrap().len(), 1);
}

/// A `NodeTypeDescriptor` with empty `inputs` and `outputs` vectors
/// serialises to JSON containing `"inputs": []` and `"outputs": []`,
/// roundtrips back to an equal value, proving the edge case of a node
/// with no slots is handled correctly.
#[test]
fn test_node_type_descriptor_empty_slots() {
    let node = NodeTypeDescriptor {
        type_name: "EmptyNode".to_string(),
        display_name: "Empty Node".to_string(),
        category: "utility".to_string(),
        description: "A node with no slots.".to_string(),
        inputs: vec![],
        outputs: vec![],
    };

    let json = serde_json::to_string(&node).expect("failed to serialise NodeTypeDescriptor");

    // Verify the JSON contains empty arrays for inputs and outputs.
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("json is valid");
    assert_eq!(parsed["inputs"], serde_json::Value::Array(vec![]));
    assert_eq!(parsed["outputs"], serde_json::Value::Array(vec![]));

    let roundtripped: NodeTypeDescriptor =
        serde_json::from_str(&json).expect("failed to deserialise NodeTypeDescriptor");
    assert_eq!(
        roundtripped, node,
        "roundtripped NodeTypeDescriptor with empty slots does not equal original"
    );
}
