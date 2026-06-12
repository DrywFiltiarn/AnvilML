/// The nine known node type names, in canonical order.
pub const KNOWN_NODE_TYPES: [&str; 9] = [
    "ZitLoadPipeline",
    "ZitTextEncode",
    "ZitSampler",
    "ZitDecode",
    "SdxlLoadPipeline",
    "SdxlTextEncode",
    "SdxlSampler",
    "SdxlDecode",
    "SaveImage",
];

/// Input and output slot names for a single node type.
#[derive(Debug, Clone, Copy)]
pub struct NodeSlots {
    pub inputs: &'static [&'static str],
    pub outputs: &'static [&'static str],
}

/// Slot table: one entry per known node type, keyed by name.
pub const NODE_SLOTS: &[(&&str, NodeSlots)] = &[
    (
        &"ZitLoadPipeline",
        NodeSlots {
            inputs: &["model_id"],
            outputs: &["pipeline"],
        },
    ),
    (
        &"ZitTextEncode",
        NodeSlots {
            inputs: &["pipeline", "prompt"],
            outputs: &["conditioning"],
        },
    ),
    (
        &"ZitSampler",
        NodeSlots {
            inputs: &["pipeline", "conditioning", "steps", "seed"],
            outputs: &["latents", "seed"],
        },
    ),
    (
        &"ZitDecode",
        NodeSlots {
            inputs: &["pipeline", "latents"],
            outputs: &["image"],
        },
    ),
    (
        &"SdxlLoadPipeline",
        NodeSlots {
            inputs: &["model_id"],
            outputs: &["pipeline"],
        },
    ),
    (
        &"SdxlTextEncode",
        NodeSlots {
            inputs: &["pipeline", "prompt", "negative_prompt"],
            outputs: &["conditioning"],
        },
    ),
    (
        &"SdxlSampler",
        NodeSlots {
            inputs: &[
                "pipeline",
                "conditioning",
                "steps",
                "guidance_scale",
                "seed",
            ],
            outputs: &["latents", "seed"],
        },
    ),
    (
        &"SdxlDecode",
        NodeSlots {
            inputs: &["pipeline", "latents"],
            outputs: &["image"],
        },
    ),
    (
        &"SaveImage",
        NodeSlots {
            inputs: &["image", "prompt", "seed", "steps"],
            outputs: &[],
        },
    ),
];

/// Look up the slot definitions for a given node type name.
///
/// Returns `None` if the type is not in the known set.
pub fn get_node_slots(type_name: &str) -> Option<&'static NodeSlots> {
    NODE_SLOTS
        .iter()
        .find(|(name, _)| **name == type_name)
        .map(|(_, slots)| slots)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_nine_types_present() {
        assert_eq!(KNOWN_NODE_TYPES.len(), 9);
        for name in &KNOWN_NODE_TYPES {
            assert!(
                !name.is_empty(),
                "Node type name must be non-empty: {name:?}"
            );
        }
    }

    #[test]
    fn test_zitsampler_outputs_include_latents_seed() {
        let slots = get_node_slots("ZitSampler").expect("ZitSampler should be a known node type");
        assert!(
            slots.outputs.contains(&"latents"),
            "ZitSampler outputs must include 'latents'"
        );
        assert!(
            slots.outputs.contains(&"seed"),
            "ZitSampler outputs must include 'seed'"
        );
    }

    #[test]
    fn test_node_parity() {
        use std::collections::HashSet;
        use std::path::Path;

        let json_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .unwrap()
            .join("backend")
            .join("tests")
            .join("known_node_types.json");
        let content = std::fs::read_to_string(&json_path)
            .expect("known_node_types.json must exist at backend/tests/");
        let json_values: Vec<String> = serde_json::from_str(&content)
            .expect("known_node_types.json must be a valid JSON array of strings");
        let rust_set: HashSet<&str> = KNOWN_NODE_TYPES.iter().copied().collect();
        let json_set: HashSet<&str> = json_values.iter().map(|s| s.as_str()).collect();
        assert_eq!(
            rust_set, json_set,
            "KNOWN_NODE_TYPES {:?} do not match known_node_types.json {:?}",
            KNOWN_NODE_TYPES, json_values
        );
    }
}
