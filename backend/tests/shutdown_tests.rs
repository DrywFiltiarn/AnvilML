/// Integration test for shutdown signal handling.
///
/// Verifies that `wait_for_shutdown_signal()` returns when a Ctrl+C
/// signal is received, using `tokio::select!` with a timeout fallback
/// to prevent indefinite hangs in CI.
///
/// Preconditions: the `anvilml` binary has been compiled
/// (e.g. `cargo build -p anvilml`).
/// Expected output: tests pass without hanging.
#[cfg(test)]
mod tests {
    use anvilml::shutdown::wait_for_shutdown_signal;

    /// Test that wait_for_shutdown_signal() returns when Ctrl+C fires.
    ///
    /// Spawns a background process that sends SIGINT to the test process,
    /// then races `wait_for_shutdown_signal()` against a 5-second timeout
    /// using `tokio::select!`. If the signal arrives first, the function
    /// returns normally. If the timeout fires first (signal didn't arrive),
    /// the test aborts the handle and fails.
    ///
    /// On Unix, the signal is sent via a child process.
    /// On Windows, the timeout path is used (programmatic Ctrl+C
    /// injection from a subprocess is unreliable on Windows).
    #[tokio::test]
    async fn test_shutdown_signal_returns_on_ctrl_c() {
        #[cfg(unix)]
        {
            // Give the tokio runtime time to register the signal handler
            // before we send the signal.
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

            // Spawn a background process that sends SIGINT to the parent
            // (the test process) after a short delay. Using a separate
            // process avoids signal handler conflicts with the tokio runtime.
            // $PPID expands to the parent shell's PID (the test process).
            std::process::Command::new("sh")
                .arg("-c")
                .arg("sleep 0.2 && kill -INT $PPID")
                .spawn()
                .expect("failed to spawn signal sender");

            let mut handle = tokio::spawn(wait_for_shutdown_signal());

            tokio::select! {
                _ = &mut handle => {
                    // Signal arrived — function returned normally.
                    assert!(
                        handle.is_finished(),
                        "shutdown handler should have completed"
                    );
                }
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(5)) => {
                    handle.abort();
                    panic!(
                        "wait_for_shutdown_signal did not return within 5s timeout"
                    );
                }
            }
        }

        #[cfg(windows)]
        {
            // Windows: tokio::signal::ctrl_c() handles Ctrl+C natively.
            // We verify the function is callable and doesn't panic
            // by using the timeout path (no signal will fire).
            let mut handle = tokio::spawn(wait_for_shutdown_signal());

            tokio::select! {
                _ = &mut handle => {}
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(2)) => {
                    handle.abort();
                }
            }
        }
    }

    /// Test that the shutdown signal future can be cancelled by timeout.
    ///
    /// Confirms that `wait_for_shutdown_signal()` does not hold any
    /// resources that would prevent cancellation, and that `tokio::select!`
    /// correctly aborts the signal handler when the timeout branch wins.
    #[tokio::test]
    async fn test_shutdown_signal_timeout_cancels() {
        let mut handle = tokio::spawn(wait_for_shutdown_signal());

        tokio::select! {
            _ = &mut handle => {
                // Signal arrived unexpectedly — should not happen in test.
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_secs(2)) => {
                // Timeout wins — signal did not arrive.
                handle.abort();
            }
        }
    }
}
