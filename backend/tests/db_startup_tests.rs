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

    /// Resolve the seed file path relative to the repo root.
    /// `CARGO_MANIFEST_DIR` is the directory containing the backend crate's
    /// Cargo.toml (`backend/`); the seed file is at
    /// `database/seeds/devices.sql` relative to the repo root.
    fn seed_path() -> String {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        // CARGO_MANIFEST_DIR is an absolute path (e.g., /home/user/repo/backend).
        // Go up one level to reach the repo root, then into database/seeds/.
        format!("{manifest_dir}/../database/seeds/devices.sql")
    }

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
            // Set the seed file path explicitly — the test CWD (backend/) is not
            // the repo root where `database/seeds/devices.sql` lives.
            .env("ANVILML_SEED_PATH", &seed_path())
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
            .env("ANVILML_SEED_PATH", &seed_path())
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

    // ---------------------------------------------------------------------------
    // Seed loader integration tests — verify SeedLoader::run() wired at startup.
    // ---------------------------------------------------------------------------

    /// Verify that the first startup run of `SeedLoader::run()` populates the
    /// `device_capabilities` table with rows from `devices.sql`.
    ///
    /// Creates a temp directory, sets `ANVILML_DB_PATH` within it, spawns the
    /// binary with `ANVILML_PORT=0`, waits for startup to complete, then queries
    /// `device_capabilities` for the row count. The temp directory is dropped
    /// at the end of the test.
    #[tokio::test]
    async fn test_seed_populates_device_capabilities() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let db_path = dir.path().join("test_anvilml.db");

        // Spawn the binary with a temp db_path and an ephemeral port.
        let mut child = Command::new(env!("CARGO_BIN_EXE_anvilml"))
            .env("ANVILML_DB_PATH", &db_path)
            .env("ANVILML_PORT", "0")
            .env("ANVILML_SEED_PATH", &seed_path())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("failed to spawn anvilml binary");

        // Wait for the "listening" log line on stderr.
        // This confirms the binary has completed startup including seed loading.
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

        // Connect to the database and verify device_capabilities has rows.
        // The seed file (devices.sql) contains 353 INSERT statements,
        // so the count should be 353.
        let pool = sqlx::SqlitePool::connect(&db_path.to_string_lossy())
            .await
            .expect("failed to connect to test database");

        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM device_capabilities")
            .fetch_one(&pool)
            .await
            .expect("failed to query device_capabilities count");

        assert!(
            count.0 > 0,
            "device_capabilities table is empty — seed was not applied (count={})",
            count.0
        );
    }

    /// Verify that a second startup run against the same database is idempotent:
    /// the `device_capabilities` row count is unchanged after re-running the seed.
    ///
    /// Creates a temp directory, spawns the binary once to seed the database,
    /// then spawns it a second time with the same `db_path`. The row count is
    /// compared before and after the second spawn.
    #[tokio::test]
    async fn test_seed_idempotent_second_run() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let db_path = dir.path().join("test_anvilml.db");

        // First run: seed the database.
        let mut child = Command::new(env!("CARGO_BIN_EXE_anvilml"))
            .env("ANVILML_DB_PATH", &db_path)
            .env("ANVILML_PORT", "0")
            .env("ANVILML_SEED_PATH", &seed_path())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("failed to spawn anvilml binary");

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

        let _ = child.kill();
        let _ = child.wait();

        assert!(
            started.is_ok() && started.unwrap(),
            "binary did not print 'listening' within 5 seconds (first run)"
        );

        // Record the row count after the first run.
        let pool = sqlx::SqlitePool::connect(&db_path.to_string_lossy())
            .await
            .expect("failed to connect to test database");

        let first_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM device_capabilities")
            .fetch_one(&pool)
            .await
            .expect("failed to query device_capabilities count after first run");

        // Second run: same database path.
        let mut child = Command::new(env!("CARGO_BIN_EXE_anvilml"))
            .env("ANVILML_DB_PATH", &db_path)
            .env("ANVILML_PORT", "0")
            .env("ANVILML_SEED_PATH", &seed_path())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("failed to spawn anvilml binary (second run)");

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

        let _ = child.kill();
        let _ = child.wait();

        assert!(
            started.is_ok() && started.unwrap(),
            "binary did not print 'listening' within 5 seconds (second run)"
        );

        // Record the row count after the second run and compare.
        let pool = sqlx::SqlitePool::connect(&db_path.to_string_lossy())
            .await
            .expect("failed to connect to test database (second run)");

        let second_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM device_capabilities")
            .fetch_one(&pool)
            .await
            .expect("failed to query device_capabilities count after second run");

        // Idempotency: the seed file hash hasn't changed, so the second run
        // should skip re-application and leave the row count identical.
        assert_eq!(
            first_count.0, second_count.0,
            "device_capabilities count changed between runs (first={}, second={}) — seed is not idempotent",
            first_count.0, second_count.0
        );
    }

    /// Verify that a missing seed file causes startup to exit with a non-zero
    /// code. The seed loader calls `std::fs::read()` on the seed path and
    /// returns `AnvilError::Io` on file-not-found, which triggers the
    /// `eprintln!` + `exit(1)` path in `main()`.
    ///
    /// Sets `ANVILML_SEED_PATH` to a non-existent file, spawns the binary
    /// with `ANVILML_PORT=0`, and asserts the process exits non-zero within
    /// 10 seconds. This test does NOT wait for "listening" — the binary
    /// should never reach the TCP bind stage with a missing seed.
    #[tokio::test]
    async fn test_missing_seed_file_causes_startup_failure() {
        // Spawn the binary with a non-existent seed path.
        // ANVILML_SEED_PATH overrides the default seed path so that the
        // seed loader tries to read a file that does not exist.
        let mut child = Command::new(env!("CARGO_BIN_EXE_anvilml"))
            .env("ANVILML_SEED_PATH", "/tmp/nonexistent_seed.sql")
            .env("ANVILML_PORT", "0")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("failed to spawn anvilml binary");

        // Wait for the process to exit. The seed error should cause immediate
        // exit before TCP bind. We use a 10-second timeout to account for
        // slow CI environments.
        // `child.wait()` is a blocking call, so we run it in a blocking thread
        // pool via `spawn_blocking`, then apply the timeout.
        let exit_status = timeout(
            Duration::from_secs(10),
            tokio::task::spawn_blocking(move || child.wait()),
        )
        .await
        .expect("binary did not exit within 10 seconds after seed failure")
        .expect("spawn_blocking panicked");

        // Assert the process exited with a non-zero code.
        // `spawn_blocking` returns a `Result<ExitStatus, JoinError>` — unwrap
        // the JoinError first, then check the ExitStatus.
        let status = exit_status.expect("spawn_blocking panicked");
        assert!(
            !status.success(),
            "expected non-zero exit for missing seed file, got: {:?}",
            status.code()
        );
    }
}
