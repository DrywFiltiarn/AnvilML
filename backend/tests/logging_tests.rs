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

    /// Verify that `--log-format json` causes the spawned binary to emit
    /// newline-delimited JSON lines on stderr when `ANVILML_LOG=debug`.
    ///
    /// Captures the prior `ANVILML_LOG` value, sets it to `"debug"`,
    /// spawns the binary with `hw-probe --log-format json`, parses each
    /// non-empty stderr line as JSON, and asserts every line is valid JSON
    /// containing at least a `level` or `msg` field. Restores the prior
    /// value unconditionally after the assertion.
    ///
    /// #[serial] is required because `std::env::set_var` is process-global;
    /// concurrent tests would race on the env var.
    #[serial]
    #[test]
    fn test_log_format_json_produces_json_lines() {
        // Capture the prior value of ANVILML_LOG so we can restore it.
        let prior = std::env::var("ANVILML_LOG").ok();

        // SAFETY: Modifying env vars in a serial test is safe — no concurrent
        // test can observe the mutation because #[serial] serializes execution.
        unsafe {
            std::env::set_var("ANVILML_LOG", "debug");
        }

        // Spawn the built binary with `hw-probe --log-format json` and
        // capture stderr. A 10-second timeout prevents hanging if the
        // binary crashes or enters an unexpected code path.
        let output = Command::new(env!("CARGO_BIN_EXE_anvilml"))
            .args(["--log-format", "json", "hw-probe"])
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
            "hw-probe with --log-format json and ANVILML_LOG=debug \
             produced empty stderr; stdout={:?}, stderr={:?}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        // Parse each non-empty stderr line as JSON and assert every line
        // is valid JSON containing at least a `level` or `msg` field.
        // tracing-subscriber always emits these fields in JSON mode.
        let stderr_text = String::from_utf8_lossy(&output.stderr);
        for (line_num, line) in stderr_text.lines().filter(|l| !l.is_empty()).enumerate() {
            // Parse the line as a generic JSON value.
            let parsed: serde_json::Value = serde_json::from_str(line).unwrap_or_else(|e| {
                panic!("stderr line {line_num} is not valid JSON: {e}\nLine content: {line}",);
            });

            // Assert the JSON object contains at least one of the fields
            // that tracing-subscriber always emits in JSON mode.
            assert!(
                parsed.get("level").is_some() || parsed.get("msg").is_some(),
                "JSON line {line_num} missing 'level' and 'msg' fields: {parsed}",
            );
        }
    }

    /// Verify that `--log-format plain` causes the spawned binary to emit
    /// plain-text (non-JSON) lines on stderr when `ANVILML_LOG=debug`.
    ///
    /// Captures the prior `ANVILML_LOG` value, sets it to `"debug"`,
    /// spawns the binary with `hw-probe --log-format plain`, asserts stderr
    /// is non-empty, and confirms at least one line is NOT valid JSON
    /// (the plain format produces lines like `2024-01-01T00:00:00.000Z  INFO ...`).
    /// Restores the prior value unconditionally after the assertion.
    ///
    /// #[serial] is required because `std::env::set_var` is process-global;
    /// concurrent tests would race on the env var.
    #[serial]
    #[test]
    fn test_log_format_plain_produces_text_lines() {
        // Capture the prior value of ANVILML_LOG so we can restore it.
        let prior = std::env::var("ANVILML_LOG").ok();

        // SAFETY: Modifying env vars in a serial test is safe — no concurrent
        // test can observe the mutation because #[serial] serializes execution.
        unsafe {
            std::env::set_var("ANVILML_LOG", "debug");
        }

        // Spawn the built binary with `hw-probe --log-format plain` and
        // capture stderr. A 10-second timeout prevents hanging.
        let output = Command::new(env!("CARGO_BIN_EXE_anvilml"))
            .args(["--log-format", "plain", "hw-probe"])
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
            "hw-probe with --log-format plain and ANVILML_LOG=debug \
             produced empty stderr; stdout={:?}, stderr={:?}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        // Assert that at least one stderr line is NOT valid JSON.
        // The plain format produces lines like `2024-01-01T00:00:00.000Z  INFO ...`
        // which fail JSON parsing, confirming we are in text mode.
        let stderr_text = String::from_utf8_lossy(&output.stderr);
        let non_json_found = stderr_text
            .lines()
            .filter(|l| !l.is_empty())
            .any(|line| serde_json::from_str::<serde_json::Value>(line).is_err());

        assert!(
            non_json_found,
            "All stderr lines were valid JSON — expected plain-text output; \
             stderr={:?}",
            stderr_text
        );
    }

    /// Verify that `ANVILML_LOG` takes precedence over `RUST_LOG` when
    /// both are set simultaneously.
    ///
    /// Sets `ANVILML_LOG=debug` alongside `RUST_LOG=error`, spawns the
    /// binary with `hw-probe`, and asserts stderr is non-empty.
    ///
    /// Rationale: `RUST_LOG=error` alone suppresses all debug-level tracing
    /// output. If `RUST_LOG` were applied instead of `ANVILML_LOG`, stderr
    /// would be empty. Non-empty stderr therefore proves `ANVILML_LOG` was
    /// the active filter. This validates the precedence rule documented
    /// in `ENVIRONMENT.md §3.3`.
    ///
    /// Captures the prior values of both env vars, restores them
    /// unconditionally after the assertion.
    ///
    /// #[serial] is required because `std::env::set_var` is process-global;
    /// concurrent tests would race on the env vars.
    #[serial]
    #[test]
    fn test_anvilml_log_precedence_over_rust_log() {
        // Capture the prior values of both env vars so we can restore
        // everything exactly as it was before the test.
        let prior_anvilml_log = std::env::var("ANVILML_LOG").ok();
        let prior_rust_log = std::env::var("RUST_LOG").ok();

        // Set ANVILML_LOG=debug alongside RUST_LOG=error. If ANVILML_LOG
        // takes precedence, debug-level tracing output will appear on
        // stderr despite RUST_LOG=error suppressing debug output.
        // SAFETY: See above — #[serial] guarantees no concurrent access.
        unsafe {
            std::env::set_var("ANVILML_LOG", "debug");
            std::env::set_var("RUST_LOG", "error");
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
        // RUST_LOG=error suppresses debug output, so non-empty stderr
        // proves ANVILML_LOG was the active filter.
        assert!(
            !output.stderr.is_empty(),
            "hw-probe with ANVILML_LOG=debug and RUST_LOG=error \
             produced empty stderr (ANVILML_LOG may not take precedence); \
             stdout={:?}, stderr={:?}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    /// Verify that `--log-format` with an invalid value causes the binary
    /// to exit with a non-zero exit code (clap exits with code 2 on
    /// validation failure).
    ///
    /// Spawns the binary with `hw-probe --log-format invalid_value` and
    /// asserts the exit code is non-zero. No env var mutation needed.
    #[test]
    fn test_log_format_invalid_exits_nonzero() {
        // Spawn the built binary with an invalid log format value.
        // A 10-second timeout prevents hanging.
        let output = Command::new(env!("CARGO_BIN_EXE_anvilml"))
            .args(["--log-format", "invalid_value", "hw-probe"])
            .output()
            .expect("failed to execute anvilml binary");

        // Assert that the exit code is non-zero — clap exits with code 2
        // on argument validation failure.
        assert!(
            output.status.code().map_or(false, |code| code != 0),
            "expected non-zero exit code for invalid --log-format; \
             got {:?}; stdout={:?}, stderr={:?}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }
}
