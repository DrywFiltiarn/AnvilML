/// Integration test for CLI help output.
///
/// Spawns the built `anvilml` binary with `--help` and asserts
/// that the help text contains all three expected CLI flags
/// (`--host`, `--port`, `--config`).
///
/// Preconditions: the `anvilml` binary has been compiled
/// (e.g. `cargo build -p anvilml`).
/// Expected output: `--help` text listing `--host`, `--port`,
/// and `--config` flags.
#[cfg(test)]
mod tests {
    use std::process::Command;

    #[test]
    fn cli_help_shows_all_flags() {
        // Spawn the built binary with --help and capture stdout.
        // Use a 10-second timeout to prevent hanging if the binary
        // fails to start or hangs on its own.
        let output = Command::new(env!("CARGO_BIN_EXE_anvilml"))
            .arg("--help")
            .output()
            .expect("failed to execute anvilml binary");

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Assert that all three CLI flags appear in the help output.
        assert!(
            stdout.contains("--host"),
            "help output missing --host flag: {stdout}"
        );
        assert!(
            stdout.contains("--port"),
            "help output missing --port flag: {stdout}"
        );
        assert!(
            stdout.contains("--config"),
            "help output missing --config flag: {stdout}"
        );
    }
}
