//! Drift guard test ensuring `anvilml.toml` stays structurally in sync with
//! `ServerConfig` from `anvilml-core`.
//!
//! Reads the committed TOML into a `toml::Value`, serializes
//! `ServerConfig::default()` into a `toml::Value`, and asserts their
//! key-sets match recursively — catching any config field added to the
//! struct but missing from the TOML, or any unknown key introduced in
//! the TOML.

use std::collections::HashSet;
use std::path::Path;

/// Recursively collect all keys from a `toml::Value` tree.
///
/// Arrays are treated as opaque leaf values (their element fields are not
/// recursed into), per the plan: "Ignoring `[[model_dirs]]` array contents."
fn collect_keys(value: &toml::Value) -> HashSet<String> {
    let mut keys = HashSet::new();
    match value {
        toml::Value::Table(table) => {
            for (k, v) in table {
                keys.insert(k.clone());
                // Only recurse into sub-tables; arrays are opaque leaf values.
                collect_keys_recursive(v, &mut keys);
            }
        }
        _ => {}
    }
    keys
}

fn collect_keys_recursive(value: &toml::Value, keys: &mut HashSet<String>) {
    if let toml::Value::Table(table) = value {
        for (k, v) in table {
            keys.insert(k.clone());
            collect_keys_recursive(v, keys);
        }
    }
}

/// Assert that every key in `ServerConfig::default()` serialized TOML
/// exists in the committed `anvilml.toml`, and vice versa.
#[test]
fn test_toml_key_set_matches_default() {
    // Resolve anvilml.toml: CARGO_MANIFEST_DIR = backend/
    // parent = workspace root
    let toml_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("backend has a parent")
        .join("anvilml.toml");

    let toml_str = std::fs::read_to_string(&toml_path)
        .unwrap_or_else(|e| panic!("cannot read {:?}: {e}", toml_path));

    // Parse committed TOML
    let file_value: toml::Value =
        toml::from_str(&toml_str).expect("anvilml.toml must be valid TOML");

    // Serialize ServerConfig::default()
    let default_config = anvilml_core::config::ServerConfig::default();
    let default_toml_str = toml::ser::to_string_pretty(&default_config)
        .expect("serialize ServerConfig::default() to TOML");
    let default_value: toml::Value = toml::from_str(&default_toml_str).expect("parse default TOML");

    // Collect key-sets
    let file_keys = collect_keys(&file_value);
    let default_keys = collect_keys(&default_value);

    // Find discrepancies
    let missing_in_file: Vec<_> = default_keys
        .iter()
        .filter(|k| !file_keys.contains(*k))
        .collect();
    let extra_in_file: Vec<_> = file_keys
        .iter()
        .filter(|k| !default_keys.contains(*k))
        .collect();

    assert!(
        missing_in_file.is_empty(),
        "Keys in ServerConfig::default() but missing from anvilml.toml:\n  {:?}\n\nSerialized default:\n{}",
        missing_in_file,
        default_toml_str
    );

    assert!(
        extra_in_file.is_empty(),
        "Keys in anvilml.toml but not in ServerConfig::default():\n  {:?}\n\nParsed file keys:\n{}",
        extra_in_file,
        toml::ser::to_string_pretty(&file_value).expect("re-serialize file value")
    );
}
