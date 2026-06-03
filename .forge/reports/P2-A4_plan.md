# Plan Report: P2-A4

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P2-A4                                       |
| Phase       | 002 — Config & Graceful Shutdown            |
| Description | anvilml: tracing subscriber init (plain/json, ANVILML_LOG env filter) |
| Depends on  | P2-A3                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-06-01T10:30:00Z                        |
| Attempt     | 1                                           |

## Objective

Introduce structured logging to the AnvilML backend by adding `tracing` + `tracing-subscriber` with the `env-filter` and `json` features. Initialise a `tracing_subscriber::fmt::Subscriber` at the top of `main.rs` before any server logic runs, sourcing the log-level filter from the `ANVILML_LOG` environment variable (falling back to `RUST_LOG`, defaulting to `info`). Wire the existing `--log-format plain|json` CLI flag so that `plain` uses `fmt()` and `json` uses `fmt().json()`. Replace the existing `println!` startup log with `tracing::info!`.

## Scope

### In Scope
- Add `tracing` (v0.1) and `tracing-subscriber` (v0.3) with features `env-filter` and `json` to `backend/Cargo.toml`
- Initialise the tracing subscriber in `backend/src/main.rs` before config load / server bind
- Resolve log filter: `ANVILML_LOG` → `RUST_LOG` → `info` default, using `EnvFilter::try_new`
- Route `--log-format plain` to `fmt()` and `--log-format json` to `fmt().json()`
- Replace the `println!("Listening on http://...")` line with `tracing::info!`

### Out of Scope
- Adding `TraceLayer` for axum request logging (deferred to a later task)
- Creating a separate `logging.rs` module (keep init inline in `main.rs` per task spec)
- Worker-side or Python-side logging changes (P2-A5 and P9 tasks handle those)
- Any changes to `.github/workflows/ci.yml`
- Unit tests for the subscriber init (verification is runtime-based via `cargo run`)

## Approach

1. **Add dependencies to `backend/Cargo.toml`:**
   - Add `tracing = "0.1"` to `[dependencies]`
   - Add `tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }` to `[dependencies]`

2. **Initialise the subscriber in `backend/src/main.rs`:**
   - After `let args = cli::parse();` on line 8, add the subscriber init code:
     ```rust
     let env_filter = std::env::var("ANVILML_LOG")
         .or_else(|_| std::env::var("RUST_LOG"))
         .unwrap_or_else(|_| "info".to_string());
     let filter = tracing_subscriber::EnvFilter::try_new(env_filter)
         .unwrap_or_else(|e| {
             eprintln!("Invalid RUST_LOG/ANVILML_LOG value: {e}, falling back to info");
             tracing_subscriber::EnvFilter::new("info")
         });
     let subscriber = match args.log_format {
         cli::LogFormat::Plain => {
             tracing_subscriber::fmt()
                 .with_env_filter(filter)
                 .finish()
         }
         cli::LogFormat::Json => {
             tracing_subscriber::fmt()
                 .json()
                 .with_env_filter(filter)
                 .finish()
         }
     };
     tracing::subscriber::set_global_default(subscriber)
         .expect("Failed to set global default tracing subscriber");
     ```
   - The existing `let _log_format = args.log_format;` on line 10 provides the value needed for the match.

3. **Replace `println!` with `tracing::info!`:**
   - Change line 29 from `println!("Listening on http://{bind_addr}");` to:
     ```rust
     tracing::info!("Listening on http://{bind_addr}");
     ```

4. **Verify compilation:**
   - Run `cargo check -p backend` to ensure everything compiles.
   - Run `cargo check --target x86_64-pc-windows-gnu --features mock-hardware` (per docs/FORGE_AGENT_RULES.md.7 Windows cross-check).

## Files Affected

| Action   | Path                    | Description                                       |
|----------|-------------------------|---------------------------------------------------|
| MODIFY   | backend/Cargo.toml      | Add tracing + tracing-subscriber dependencies     |
| MODIFY   | backend/src/main.rs     | Initialise subscriber; replace println! with tracing::info! |

## Tests

| Test ID / Name            | File                     | Validates               |
|---------------------------|--------------------------|-------------------------|
| None.                     | N/A                      | Verification is runtime-based via `cargo run` commands (see Acceptance Criteria). |

## CI Impact

No CI changes required.

## Risks and Mitigations

| Risk                      | Likelihood | Impact | Mitigation              |
|---------------------------|-----------|--------|-------------------------|
| `tracing-subscriber` feature names (`env-filter`, `json`) differ from expected crate API | Low | High | Verify feature names against crates.io docs.rs before writing; the task spec explicitly names them, and they are well-established features. |
| Subscriber init panics on invalid `ANVILML_LOG` value, crashing the binary at startup | Medium | Medium | Use `unwrap_or_else` with a fallback to `EnvFilter::new("info")` so an invalid filter produces a warning + sensible default rather than a panic. |
| `args.log_format` used before being assigned (order of operations in main) | Low | High | Ensure the `let _log_format = args.log_format;` line is already present on line 10 and move subscriber init after it — no reordering needed since main.rs already declares this binding. |
| JSON formatter adds significant startup overhead | Low | Low | Only enabled when explicitly requested via `--log-format json`; plain mode has negligible overhead vs println. |

## Acceptance Criteria

- [ ] `cargo check -p backend` exits 0 with no warnings
- [ ] `cargo check --target x86_64-pc-windows-gnu --features mock-hardware` exits 0 (Windows cross-check per docs/FORGE_AGENT_RULES.md.7)
- [ ] `ANVILML_LOG=debug cargo run -- --port 9000` shows debug-level log lines in plain-text format
- [ ] `cargo run -- --port 9000 --log-format json` emits structured JSON log lines (each line is valid JSON with a `msg` field containing "Listening on...")
- [ ] Default `cargo run -- --port 9000` (no env vars set) shows only info-level and above (no debug lines)
- [ ] The `println!("Listening on...")` call has been replaced with `tracing::info!` (grep confirms zero remaining println! calls in main.rs)
