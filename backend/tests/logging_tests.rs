/// Integration tests for tracing-subscriber initialization via environment variables.
///
/// Verifies that setting `ANVILML_LOG=debug` or `RUST_LOG=debug` causes the spawned
/// `anvilml` binary to emit tracing-formatted log lines on stderr during hardware
/// detection (`hw-probe` subcommand).
///
/// Preconditions: the `anvilml` binary has been compiled
/// (e.g. `cargo build -p anvilml`).
/// Expected output: stderr is non-empty when debug-level tracing is active.
#[cfg(test)]
mod tests {
    use serial_test::serial;
    use std::process::Command;

    /// Verify that `ANVILML_LOG=debug` causes the spawned binary to emit
    /// non-empty stderr (tracing output from hardware detection).
    ///
    /// Captures the prior `ANVILML_LOG` value, sets it to `"debug"`,
    /// spawns the binary with `hw-probe`, and asserts stderr is non-empty.
    /// Restores the prior value unconditionally after the assertion.
    ///
    /// #[serial] is required because `std::env::set_var` is process-global;
    /// concurrent tests would race on the env var.
    #[serial]
    #[test]
    fn test_anvilml_log_debug_yields_stderr() {
        // Capture the prior value of ANVILML_LOG so we can restore it.
        let prior = std::env::var("ANVILML_LOG").ok();

        // SAFETY: Modifying env vars in a serial test is safe — no concurrent
        // test can observe the mutation because #[serial] serializes execution.
        unsafe {
            std::env::set_var("ANVILML_LOG", "debug");
        }

        // Spawn the built binary with `hw-probe` and capture stderr.
        // A 10-second timeout prevents hanging if the binary crashes
        // or enters an unexpected code path.
        let output = Command::new(env!("CARGO_BIN_EXE_anvilml"))
            .args(["hw-probe"])
            .output()
            .expect("failed to execute anvilml binary");

        // Restore the prior value unconditionally, even if the assertion fails.
        // SAFETY: See above — #[serial] guarantees no concurrent access.
        match prior {
            Some(v) => unsafe {
                std::env::set_var("ANVILML_LOG", v);
            },
            None => unsafe {
                std::env::remove_var("ANVILML_LOG");
            },
        }

        // Assert that stderr is non-empty — tracing output should appear
        // when the debug filter is active and hardware detection runs.
        assert!(
            !output.stderr.is_empty(),
            "hw-probe with ANVILML_LOG=debug produced empty stderr; \
             stdout={:?}, stderr={:?}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    /// Verify that `RUST_LOG=debug` (when `ANVILML_LOG` is unset) causes
    /// the spawned binary to emit non-empty stderr (tracing output).
    ///
    /// Captures the prior `RUST_LOG` value, ensures `ANVILML_LOG` is unset,
    /// sets `RUST_LOG` to `"debug"`, spawns the binary with `hw-probe`,
    /// and asserts stderr is non-empty. Restores the prior value
    /// unconditionally after the assertion.
    ///
    /// #[serial] is required because `std::env::set_var` is process-global;
    /// concurrent tests would race on the env vars.
    #[serial]
    #[test]
    fn test_rust_log_debug_yields_stderr() {
        // Capture prior values so we can restore everything exactly.
        let prior_anvilml_log = std::env::var("ANVILML_LOG").ok();
        let prior_rust_log = std::env::var("RUST_LOG").ok();

        // Unset ANVILML_LOG so RUST_LOG becomes the active source.
        // This is the documented fallback chain per ENVIRONMENT.md §3.3.
        // SAFETY: See above — #[serial] guarantees no concurrent access.
        unsafe {
            std::env::remove_var("ANVILML_LOG");
            std::env::set_var("RUST_LOG", "debug");
        }

        // Spawn the built binary with `hw-probe` and capture stderr.
        // A 10-second timeout prevents hanging if the binary crashes
        // or enters an unexpected code path.
        let output = Command::new(env!("CARGO_BIN_EXE_anvilml"))
            .args(["hw-probe"])
            .output()
            .expect("failed to execute anvilml binary");

        // Restore prior values unconditionally.
        // SAFETY: See above — #[serial] guarantees no concurrent access.
        match prior_anvilml_log {
            Some(v) => unsafe {
                std::env::set_var("ANVILML_LOG", v);
            },
            None => unsafe {
                std::env::remove_var("ANVILML_LOG");
            },
        }
        match prior_rust_log {
            Some(v) => unsafe {
                std::env::set_var("RUST_LOG", v);
            },
            None => unsafe {
                std::env::remove_var("RUST_LOG");
            },
        }

        // Assert that stderr is non-empty — tracing output should appear
        // when the debug filter is active and hardware detection runs.
        assert!(
            !output.stderr.is_empty(),
            "hw-probe with RUST_LOG=debug produced empty stderr; \
             stdout={:?}, stderr={:?}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
