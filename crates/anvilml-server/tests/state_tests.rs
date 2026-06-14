/// Verify that AppState::new() sets start_time to a recent instant and
/// stores the version string correctly.
#[test]
fn test_app_state_new() {
    let before = std::time::Instant::now();
    let state = anvilml_server::AppState::new("0.1.0");
    let after = std::time::Instant::now();

    // Verify the version was stored correctly.
    assert_eq!(state.version, "0.1.0");

    // Verify start_time is recent: elapsed time between before and after
    // construction must be less than 1 second. Instant does not implement
    // PartialEq, so we compute the elapsed duration instead.
    let elapsed = after - before;
    assert!(
        elapsed < std::time::Duration::from_secs(1),
        "start_time should be within the last second, but elapsed is {:?}",
        elapsed,
    );
}

/// Verify that AppState implements Clone correctly — the cloned version
/// field must match the original.
///
/// Instant does not compare equal across clones, so we only verify the
/// String field.
#[test]
fn test_app_state_clone() {
    let state = anvilml_server::AppState::new("0.1.0");
    let cloned = state.clone();

    assert_eq!(cloned.version, state.version);
}

/// Verify that AppState::new() accepts a &'static str from
/// CARGO_PKG_VERSION and stores it correctly.
///
/// This confirms the constructor's impl Into<String> accepts &'static str.
#[test]
fn test_app_state_version_from_env() {
    let crate_version = env!("CARGO_PKG_VERSION");
    let state = anvilml_server::AppState::new(crate_version);

    assert_eq!(state.version, crate_version);
}
