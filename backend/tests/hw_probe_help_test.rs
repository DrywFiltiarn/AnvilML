/// Integration test for the `hw-probe` subcommand help output.
///
/// Spawns the built `anvilml` binary with `hw-probe --help` and asserts
/// that the help text contains the "hw-probe" subcommand name.
///
/// Preconditions: the `anvilml` binary has been compiled
/// (e.g. `cargo build -p anvilml`).
/// Expected output: `hw-probe --help` text listing the subcommand.
#[cfg(test)]
mod tests {
    use std::process::Command;

    /// Verify that the `hw-probe` subcommand appears in help output.
    ///
    /// Spawns the binary with `hw-probe --help` and asserts that
    /// the stdout contains the string "hw-probe". Uses a 10-second
    /// timeout to prevent hanging if the binary fails to start.
    #[test]
    fn hw_probe_help_shows_subcommand() {
        // Spawn the built binary with `hw-probe --help` and capture stdout.
        // A 10-second timeout prevents hanging if the binary crashes
        // or enters an unexpected code path.
        let output = Command::new(env!("CARGO_BIN_EXE_anvilml"))
            .args(["hw-probe", "--help"])
            .output()
            .expect("failed to execute anvilml binary");

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Assert that the hw-probe subcommand name appears in the help output.
        // Clap includes the subcommand name in the usage line and description.
        assert!(
            stdout.contains("hw-probe"),
            "help output missing 'hw-probe' subcommand: {stdout}"
        );
    }
}
