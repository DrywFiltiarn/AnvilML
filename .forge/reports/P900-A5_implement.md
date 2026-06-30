# Implementation Report: P900-A5

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P900-A5                         |
| Phase         | 900 — Spec-Drift & Logging Retrofit |
| Description   | backend: add --log-format plain|json CLI flag per ENVIRONMENT.md §3.3 |
| Implemented   | 2026-06-30T15:30:00Z            |
| Status        | COMPLETE                          |

## Summary

Added the `--log-format plain|json` CLI flag to the AnvilML backend binary. The flag
accepts `"plain"` (default) and `"json"` values, with clap exiting with code 2 on any
other input. When `--log-format json` is passed, the tracing-subscriber builder uses
`.json()` to emit newline-delimited JSON on stderr; the `"plain"` default keeps the
existing text formatter. Three new integration tests verify JSON output format, plain-text
output format, and invalid-value rejection.

## Resolved Dependencies

| Type   | Name               | Version resolved | Source         |
|--------|--------------------|------------------|----------------|
| crate  | tracing-subscriber | 0.3.23           | rust-docs MCP  |
| crate  | clap               | 4.6.1            | rust-docs MCP  |

The `json` feature on `tracing-subscriber 0.3.23` provides the `SubscriberBuilder::json()`
method (confirmed via `rust-docs_search_docs` in the `fmt` module). The `clap 4.6.1`
`derive` feature supports `#[arg(value_parser = ...)]` for input validation.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | backend/Cargo.toml | Added `json` feature to `tracing-subscriber`; bumped version 0.1.5 → 0.1.6 |
| Modify | backend/src/cli.rs | Added `log_format: String` field to `Cli` struct with validation helper |
| Modify | backend/src/main.rs | Parsed CLI before subscriber init; branched subscriber builder on `log_format` |
| Modify | backend/tests/logging_tests.rs | Added 3 new integration tests (JSON format, plain format, invalid value) |
| Modify | docs/TESTS.md | Added 3 new test catalogue entries |

## Commit Log

```
 .forge/state/CURRENT_TASK.md   |   6 +-
 .forge/state/state.json        |  13 ++--
 Cargo.lock                     |  15 +++-
 backend/Cargo.toml             |   4 +-
 backend/src/cli.rs             |  23 ++++++
 backend/src/main.rs            |  21 +++++-
 backend/tests/logging_tests.rs | 166 +++++++++++++++++++++++++++++++++++++++++
 docs/TESTS.md                  |  36 +++++++++
 8 files changed, 268 insertions(+), 16 deletions(-)
```

## Test Results

```
     Running tests/logging_tests.rs (target/debug/deps/logging_tests-bdfa4e76dcf9de2f)

running 5 tests
test tests::test_anvilml_log_debug_yields_stderr ... ok
test tests::test_log_format_invalid_exits_nonzero ... ok
test tests::test_log_format_json_produces_json_lines ... ok
test tests::test_log_format_plain_produces_text_lines ... ok
test tests::test_rust_log_debug_yields_stderr ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
```

Full workspace test suite: 171 tests passed, 0 failed.

## Format Gate

```
(no output — exit 0)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.61s

# 2. Mock-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 29.62s

# 3. Real-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 25.50s

# 4. Real-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 22.96s
```

All four platform cross-checks exit 0.

## Project Gates

### Gate 1 — Config Surface Sync
```
test tests::config_reference_matches_defaults ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

### Gate 2 — OpenAPI Drift
Not triggered — task does not modify handler function signatures, `#[utoipa::path]`
annotations, or `AppState` fields.

### Gate 3 — Node Parity
Not triggered — task does not add, remove, or rename node types.

### Gate 4 — Mock/Real Parity Markers
Not triggered — task does not add or modify a node's `execute()` or an arch module's
`load()`/`sample()`/`decode()`/`compute_latent_shape()`.

## Public API Delta

```
+    pub log_format: String,
```

One new `pub` field added to the existing `Cli` struct in `backend/src/cli.rs`.
This matches the plan's `## Public API Surface` table exactly. No new `pub` functions,
types, or traits were introduced.

## Deviations from Plan

- **Argument ordering in tests**: The plan specified `.args(["hw-probe", "--log-format", "json"])`
  but `--log-format` is a top-level CLI flag (not a subcommand flag), so the correct
  ordering is `.args(["--log-format", "json", "hw-probe"])`. This was confirmed by
  testing and the binary's clap help output.
- **CLI parsing moved before subscriber init**: The plan's Step 3 showed the subscriber
  builder being created first and the CLI parsed after. However, since the subscriber
  builder needs the `log_format` value from the parsed CLI, the CLI parse was moved
  before the subscriber init. This is a necessary ordering change.

## Blockers

None.
