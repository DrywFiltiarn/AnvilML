# Plan Report: P900-A5

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P900-A5                                     |
| Phase       | 900 — Spec-Drift & Logging Retrofit         |
| Description | backend: add --log-format plain|json CLI flag per ENVIRONMENT.md §3.3 |
| Depends on  | P900-A1, P900-A4                            |
| Project     | anvilml                                     |
| Planned at  | 2026-06-30T14:50:00Z                        |
| Attempt     | 1                                           |

## Objective

Add the `--log-format plain|json` CLI flag to the AnvilML backend binary, allowing users
to select the tracing-subscriber output encoding. When `--log-format json` is passed, the
subscriber builder emits newline-delimited JSON on stderr; when `--log-format plain` (the
default) or no flag is passed, the existing plain-text `fmt()` layout is used unchanged.
This closes a spec-drift gap found during the same audit that identified P900-A1:
`ENVIRONMENT.md §3.3` documents this flag but no task ever implemented it.

## Scope

### In Scope
- Add `json` feature to `tracing-subscriber` dependency in `backend/Cargo.toml`.
- Add `log_format: String` field to the `Cli` struct in `backend/src/cli.rs`, with
  `#[arg(long, default_value = "plain")]` and a `value_parser` that accepts only
  `"plain"` or `"json"`, exiting with code 2 on any other value.
- Branch the `tracing_subscriber::fmt()` builder in `backend/src/main.rs` on the parsed
  `log_format` value: `"plain"` keeps the existing `.with_writer(std::io::stderr).init()`
  chain; `"json"` inserts `.json()` before `.init()`. The `EnvFilter` precedence
  (ANVILML_LOG → RUST_LOG → "info") is unchanged.
- Extend `backend/tests/logging_tests.rs` with ≥3 new tests (bringing total to ≥4):
  `--log-format=json` produces valid-JSON stderr lines, `--log-format=plain` works
  identically to the existing plain-text default, and an invalid value exits non-zero.

### Out of Scope
None. This task implements its full scope with no deferrals.
defers_to (from JSON): []

## Existing Codebase Assessment

**What already exists:** `P900-A1` already wired `tracing-subscriber` into the binary
(`backend/Cargo.toml` declares it with `["env-filter"]` feature; `main.rs` calls
`tracing_subscriber::fmt().with_env_filter(...).with_writer(std::io::stderr).init()` at
startup). The `Cli` struct in `cli.rs` has four fields (`command`, `host`, `port`,
`config`). The existing `logging_tests.rs` has two serialised integration tests that
spawn the `anvilml` binary with `ANVILML_LOG=debug` and `RUST_LOG=debug` respectively,
asserting non-empty stderr.

**Established patterns:** Tests in `backend/tests/` use `Command::new(env!("CARGO_BIN_EXE_anvilml"))`
for reliable binary path resolution, `#[serial]` annotation for env-var-mutating tests,
and capture-and-restore for environment variable isolation. The CLI uses clap derive
macros with `#[arg(long)]` for optional override flags. Error handling on invalid CLI
input delegates to clap's built-in exit-with-code-2 behavior.

**Gap between design doc and source:** `ENVIRONMENT.md §3.3` explicitly documents the
`--log-format plain|json` flag (default `plain`) as controlling output format, but
`cli.rs` has no `log_format` field and `main.rs` has no JSON branch — the subscriber
always uses plain-text format. This is the exact spec-drift gap this task closes.

## Resolved Dependencies

| Type   | Name               | Version verified | MCP source     | Feature flags confirmed       |
|--------|--------------------|------------------|----------------|-------------------------------|
| crate  | tracing-subscriber | 0.3.23           | rust-docs MCP  | json (to be added), env-filter (existing) |
| crate  | clap               | 4.6.1            | rust-docs MCP  | derive (already declared)     |

The `json` feature on `tracing-subscriber 0.3.23` provides the `SubscriberBuilder::json()`
method (confirmed via `rust-docs_search_docs` in the `fmt` module: `fn json(self) ->
SubscriberBuilder<format::JsonFields, format::Format<format::Json, T>, F, W>`). The
existing `env-filter` feature is retained. `clap 4.6.1` with `derive` feature supports
`#[arg(value_parser = ...)]` for input validation.

## Approach

### Step 1 — Add `json` feature to `tracing-subscriber` in `backend/Cargo.toml`

Change line 21 from:
```toml
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
```
to:
```toml
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
```

The `json` feature pulls in `tracing-serde`, `serde`, and `serde_json` as transitive
dependencies — `serde_json` is already declared in `backend/Cargo.toml` so no new
top-level dependency is introduced.

### Step 2 — Add `log_format` field to `Cli` struct in `backend/src/cli.rs`

Append a new field to the `Cli` struct (after the `config` field, before the closing
brace):

```rust
    /// Log output format: "plain" for human-readable text or "json" for newline-delimited JSON.
    ///
    /// Defaults to "plain". Any other value causes clap to exit with usage information
    /// and exit code 2, matching the existing CLI error convention.
    #[arg(long, default_value = "plain", value_parser = validate_log_format)]
    pub log_format: String,
```

Add a helper function after `parse()` (or at module level):

```rust
/// Validate that the log format string is one of the supported values.
///
/// Returns the input string unchanged if it is "plain" or "json";
/// otherwise returns an error, which clap converts to an exit-with-code-2 message.
fn validate_log_format(s: &str) -> Result<String, String> {
    match s {
        "plain" | "json" => Ok(s.to_owned()),
        other => Err(format!("invalid log format '{other}': expected 'plain' or 'json'")),
    }
}
```

The `value_parser` attribute on the `#[arg(...)]` field accepts a function pointer.
When validation fails, clap prints the error message and exits with code 2 — matching
the existing CLI error convention referenced in the task context.

### Step 3 — Branch the subscriber builder in `backend/src/main.rs`

Modify the subscriber initialization block (lines 36–43) to branch on `log_format`:

```rust
    // Initialize the tracing subscriber as the very first startup step.
    // Reads filter from ANVILML_LOG (primary) or RUST_LOG (fallback),
    // defaulting to "info" when neither is set — matching the precedence
    // documented in ENVIRONMENT.md §3.3.
    // Output format is controlled by --log-format (plain or json), not by
    // an environment variable, per ENVIRONMENT.md §3.3.
    // Write to stderr so tracing output does not mix with stdout data
    // (e.g. `hw-probe` JSON output goes to stdout, logs go to stderr).
    let builder = tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_env("ANVILML_LOG")
                .or_else(|_| EnvFilter::try_from_env("RUST_LOG"))
                .unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr);

    // Branch on the parsed log_format value.
    // "plain" keeps the default text formatter; "json" switches to
    // newline-delimited JSON via the tracing-subscriber json feature.
    // The EnvFilter precedence is identical in both branches.
    let subscriber = match cli.log_format.as_str() {
        "json" => builder.json().init(),
        _ => builder.init(), // "plain" — default text formatter
    };
```

Rationale: extracting `builder` into a local variable avoids duplicating the
`with_env_filter(...)` and `with_writer(...)` calls. The match arm for `"json"` inserts
`.json()` which swaps the formatter to `JsonFields` + `Format<Json>`. The `_` arm
covers `"plain"` (the validated default) and is future-proof against any additional
validated values.

### Step 4 — Extend `backend/tests/logging_tests.rs` with ≥3 new tests

Append three new test functions to the existing `mod tests` block:

**Test 4a — `test_log_format_json_produces_json_lines`:**
Spawn the binary with `--log-format json` and `ANVILML_LOG=debug`. Parse each non-empty
stderr line as JSON (using `serde_json::from_str::<serde_json::Value>`). Assert every
line is valid JSON and contains at least a `msg` or `level` field (the minimum fields
tracing-subscriber always emits in JSON mode). Use `#[serial]` and capture/restore env
vars.

**Test 4b — `test_log_format_plain_produces_text_lines`:**
Spawn with `--log-format plain` and `ANVILML_LOG=debug`. Assert stderr is non-empty and
contains at least one line that is NOT valid JSON (the plain format produces lines like
`2024-01-01T00:00:00.000Z  INFO ...` which fail JSON parsing). Use `#[serial]` and
capture/restore env vars.

**Test 4c — `test_log_format_invalid_exits_nonzero`:**
Spawn with `--log-format invalid_value`. Assert the exit code is non-zero (clap exits
with code 2 on validation failure). No env var mutation needed for this test.

Each test uses `Command::new(env!("CARGO_BIN_EXE_anvilml"))` per the existing pattern.
The JSON test validates output format by parsing each line; the plain test validates by
confirming output is non-JSON text; the invalid test validates exit code.

## Public API Surface

No new `pub` items are introduced. The only change to public API surface is the addition
of one field to an existing struct:

```rust
// backend/src/cli.rs — added to existing Cli struct:
pub log_format: String
```

This field is consumed internally by `main.rs` and is not exposed as a standalone pub
function or type.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `backend/Cargo.toml` | Add `json` feature to `tracing-subscriber` dependency |
| Modify | `backend/src/cli.rs` | Add `log_format` field to `Cli` struct with validation |
| Modify | `backend/src/main.rs` | Branch subscriber builder on `log_format` value |
| Modify | `backend/tests/logging_tests.rs` | Add 3 new integration tests for log format flag |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|--------------------|
| `backend/tests/logging_tests.rs` | `test_log_format_json_produces_json_lines` | `--log-format json` produces valid-JSON stderr lines when `ANVILML_LOG=debug` | `cargo test -p anvilml --test logging_tests -- test_log_format_json` exits 0 |
| `backend/tests/logging_tests.rs` | `test_log_format_plain_produces_text_lines` | `--log-format plain` produces plain-text (non-JSON) stderr lines when `ANVILML_LOG=debug` | `cargo test -p anvilml --test logging_tests -- test_log_format_plain` exits 0 |
| `backend/tests/logging_tests.rs` | `test_log_format_invalid_exits_nonzero` | `--log-format` with an invalid value causes the binary to exit non-zero (clap code 2) | `cargo test -p anvilml --test logging_tests -- test_log_format_invalid` exits 0 |

These three new tests, combined with P900-A1's existing two tests, bring the total to
≥4 tests in the file as required by the task acceptance criterion.

## CI Impact

No CI changes required. The task modifies only the `backend` crate and its tests.
The existing CI jobs (`rust-linux`, `rust-windows`) already run `cargo test --workspace
--features mock-hardware`, which includes `backend/tests/logging_tests.rs`. The new
`json` feature on `tracing-subscriber` is a compile-time feature flag — it does not
affect runtime behavior on any platform, so no new CI jobs or platform guards are needed.

## Platform Considerations

None identified. The `--log-format` flag and its plain/json branching are fully
platform-neutral. The `tracing-subscriber` `json` feature works identically on Linux
and Windows. The Windows cross-check in ENVIRONMENT.md §7 (`cargo check --target
x86_64-pc-windows-gnu`) exercises this code path without any `#[cfg]` guards.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `tracing-subscriber::fmt().json()` requires the `json` feature to be enabled; if omitted, compilation fails with a missing method error. | Low | High (build break) | Feature is added in Step 1 before any code referencing `.json()` is written. The compiler will fail fast at the first reference if the feature is missing. |
| The `value_parser` closure on `#[arg(...)]` may not exit with code 2 as expected — clap's derive behavior on `value_parser` errors depends on whether the function returns `Result<T, E>` where `E: Display`. | Low | Medium (wrong exit code) | The helper function returns `Result<String, String>` — clap 4.x converts `Display` errors to exit code 2 with usage output, matching the existing CLI convention. Verified against clap 4.6.1 behavior. |
| JSON-mode stderr output from `hw-probe` may contain no tracing events at all (if hardware detection doesn't emit any), causing the JSON test to find zero lines and pass vacuously. | Medium | Medium (false positive) | The test asserts that `ANVILML_LOG=debug` is set AND that the binary emitted at least one tracing event (e.g., the "listening" log from server startup or the hardware detection logs). If hw-probe produces zero events in JSON mode, the test should assert `lines.len() > 0`. |
| The existing plain-text tests in logging_tests.rs may break if `--log-format plain` is not explicitly passed (the default path still uses plain text). | Low | Low (test failure) | The default path in main.rs uses the `_ => builder.init()` arm which preserves plain-text behavior. The new `test_log_format_plain_produces_text_lines` test explicitly passes `--log-format plain` to confirm this path works. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml --test logging_tests` exits 0 (≥4 tests total pass)
- [ ] `cargo test -p anvilml --test logging_tests -- test_log_format_json` exits 0
- [ ] `cargo test -p anvilml --test logging_tests -- test_log_format_plain` exits 0
- [ ] `cargo test -p anvilml --test logging_tests -- test_log_format_invalid` exits 0
- [ ] `cargo test --workspace --features mock-hardware` exits 0 (phase-closing regression)
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
- [ ] `cargo fmt --all -- --check` exits 0
