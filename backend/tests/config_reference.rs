/// Integration test: the checked-in `anvilml.toml` has the same key set
/// as `ServerConfig::default()` serialised to TOML.
///
/// This test verifies the config drift guard (Gate 1): every field in
/// `ServerConfig` must appear in `anvilml.toml` and vice versa. Missing
/// or extra keys cause a test failure with a descriptive message listing
/// the mismatched keys.
///
/// Preconditions:
///   - Workspace builds with `mock-hardware` feature.
///   - `anvilml.toml` exists at the repo root.
///
/// Acceptance command:
///   `cargo test -p anvilml --features mock-hardware -- config_reference`
///   exits 0.
use std::collections::BTreeSet;

use anvilml_core::ServerConfig;

/// Collect all keys from a `toml::Value` tree recursively into a sorted set.
///
/// For `Table` variants, inserts each key and recurses into child values.
/// For non-table values, returns an empty set.
fn collect_keys(value: &toml::Value) -> BTreeSet<String> {
    let mut keys = BTreeSet::new();

    // Recursively collect keys from table entries.
    // Non-table values (arrays, scalars, null) contribute no keys of their
    // own — only the table keys above them in the tree.
    if let toml::Value::Table(table) = value {
        for (key, child) in table {
            keys.insert(key.clone());
            keys.extend(collect_keys(child));
        }
    }

    keys
}

#[test]
fn config_reference() {
    // Serialise the default config to a TOML string.
    // This produces the "source of truth" key set — every field in
    // `ServerConfig` (including nested structs and Option fields) is
    // represented in the output.
    let default_toml = toml::to_string_pretty(&ServerConfig::default())
        .expect("failed to serialise ServerConfig::default() to TOML");

    // Read the checked-in reference config.
    // The path `../anvilml.toml` is relative to this test file's directory
    // (`backend/tests/`), which resolves to the repo root.
    let reference_content = std::fs::read_to_string("../anvilml.toml")
        .expect("failed to read anvilml.toml — is it at the repo root?");

    // Parse both TOML strings into generic `toml::Value` trees.
    // Using `toml::Value` (rather than deserialising into a specific struct)
    // lets us compare key sets without caring about value equality.
    let default_value: toml::Value =
        toml::from_str(&default_toml).expect("failed to parse default TOML into toml::Value");
    let reference_value: toml::Value =
        toml::from_str(&reference_content).expect("failed to parse anvilml.toml into toml::Value");

    // Collect keys from both trees.
    let default_keys = collect_keys(&default_value);
    let reference_keys = collect_keys(&reference_value);

    // Assert the key sets are equal.
    // If they differ, panic with a message listing missing and extra keys
    // so the operator can fix `anvilml.toml` or `ServerConfig` accordingly.
    assert_eq!(
        default_keys,
        reference_keys,
        "Config key mismatch.\n\
         Missing from anvilml.toml (in ServerConfig but not in file): {:?}\n\
         Extra in anvilml.toml (in file but not in ServerConfig): {:?}",
        default_keys.difference(&reference_keys).collect::<Vec<_>>(),
        reference_keys.difference(&default_keys).collect::<Vec<_>>(),
    );
}
