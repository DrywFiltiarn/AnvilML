/// Integration tests for database pool creation during server startup.
///
/// These tests spawn the built `anvilml` binary in its default (server) path,
/// which calls `create_pool()` to create the SQLite database and run migrations
/// before binding the TCP listener.
///
/// Preconditions: the `anvilml` binary has been compiled
/// (e.g. `cargo build -p anvilml`).
/// Expected output: a `.db` file is created on disk, and both
/// `models` and `device_capabilities` tables exist in the database.
#[cfg(test)]
mod tests {
    use std::io::BufRead;
    use std::process::Command;
    use std::process::Stdio;
    use tokio::time::{Duration, timeout};

    /// Verify that spawning the binary with a temp `db_path` creates the
    /// `.db` file on disk.
    ///
    /// Creates a temp directory, sets `ANVILML_DB_PATH` to a path within it,
    /// spawns the binary with `ANVILML_PORT=0` (ephemeral port), and waits
    /// up to 5 seconds for the "listening" log line on stderr. Then asserts
    /// the `.db` file exists on disk. The temp directory is dropped at the
    /// end of the test, cleaning up the `.db` file.
    #[tokio::test]
    async fn test_db_file_created_on_startup() {
        // Create a unique temp directory for this test.
        // The directory is cleaned up when `dir` is dropped, including
        // any `.db` file created inside it.
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let db_path = dir.path().join("test_anvilml.db");

        // Spawn the built binary with a temp db_path and an ephemeral port.
        // Port 0 tells the OS to pick any available port, avoiding conflicts
        // with other tests or services. Stdio is piped so we can read stderr
        // for the "listening" log line and diagnose startup failures.
        let mut child = Command::new(env!("CARGO_BIN_EXE_anvilml"))
            .env("ANVILML_DB_PATH", &db_path)
            .env("ANVILML_PORT", "0")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("failed to spawn anvilml binary");

        // Wait for the "listening" log line on stderr.
        // The binary prints "listening" to stderr after successfully binding
        // the TCP listener, which confirms that create_pool() completed
        // and the database was created.
        // A 5-second timeout prevents hanging if the binary fails to start.
        let started = timeout(Duration::from_secs(5), async {
            let stderr = child
                .stderr
                .take()
                .expect("stderr was not piped — test setup error");
            let reader = std::io::BufReader::new(stderr);
            for line in reader.lines() {
                let line = line.expect("failed to read stderr line");
                if line.contains("listening") {
                    return true;
                }
            }
            false
        })
        .await;

        // Terminate the process regardless of whether we saw the "listening"
        // line. This prevents the process from blocking the test runner.
        let _ = child.kill();
        let _ = child.wait();

        // Assert the binary produced the "listening" log line, confirming
        // that create_pool() ran and the TCP bind succeeded.
        assert!(
            started.is_ok() && started.unwrap(),
            "binary did not print 'listening' within 5 seconds"
        );

        // Assert the `.db` file exists on disk.
        // It was created by create_pool() during startup.
        assert!(
            db_path.exists(),
            "database file was not created at {}",
            db_path.display()
        );
    }

    /// Verify that the migrations create both required tables (`models`
    /// and `device_capabilities`) in the database.
    ///
    /// Creates a temp directory, sets `ANVILML_DB_PATH` within it, spawns
    /// the binary with `ANVILML_PORT=0`, waits for startup to complete,
    /// then queries `sqlite_master` to confirm both tables exist.
    /// The temp directory is dropped at the end of the test.
    #[tokio::test]
    async fn test_migrations_create_required_tables() {
        // Create a unique temp directory for this test.
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let db_path = dir.path().join("test_anvilml.db");

        // Spawn the built binary with a temp db_path and an ephemeral port.
        let mut child = Command::new(env!("CARGO_BIN_EXE_anvilml"))
            .env("ANVILML_DB_PATH", &db_path)
            .env("ANVILML_PORT", "0")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("failed to spawn anvilml binary");

        // Wait for the "listening" log line on stderr.
        // This confirms the binary has completed startup (including migrations)
        // and is ready to serve requests.
        let started = timeout(Duration::from_secs(5), async {
            let stderr = child
                .stderr
                .take()
                .expect("stderr was not piped — test setup error");
            let reader = std::io::BufReader::new(stderr);
            for line in reader.lines() {
                let line = line.expect("failed to read stderr line");
                if line.contains("listening") {
                    return true;
                }
            }
            false
        })
        .await;

        // Terminate the process to free the port and clean up.
        let _ = child.kill();
        let _ = child.wait();

        // Assert startup completed successfully.
        assert!(
            started.is_ok() && started.unwrap(),
            "binary did not print 'listening' within 5 seconds"
        );

        // Connect to the database and verify both tables exist.
        // Query sqlite_master for table names — this is the canonical
        // way to inspect SQLite schema.
        let pool = sqlx::SqlitePool::connect(&db_path.to_string_lossy())
            .await
            .expect("failed to connect to test database");

        let rows: Vec<(String,)> =
            sqlx::query_as("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
                .fetch_all(&pool)
                .await
                .expect("failed to query sqlite_master");

        let table_names: Vec<&str> = rows.iter().map(|r| r.0.as_str()).collect();

        // Assert that both required tables were created by migrations.
        // The `models` table stores discovered model metadata, and
        // `device_capabilities` stores GPU capability hints for scheduling.
        assert!(
            table_names.contains(&"models"),
            "sqlite_master missing 'models' table; found: {:?}",
            table_names
        );
        assert!(
            table_names.contains(&"device_capabilities"),
            "sqlite_master missing 'device_capabilities' table; found: {:?}",
            table_names
        );
    }
}
