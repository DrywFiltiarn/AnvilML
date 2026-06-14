# Plan Report: P900-A1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P900-A1                                     |
| Phase       | 900 — CLI Test Windows Port-Detection Fix   |
| Description | backend: fix cli_tests port-detection to compile and pass on Windows |
| Depends on  | P3-A3                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-06-14T18:40:00Z                        |
| Attempt     | 1                                           |

## Objective

Replace the unconditional `lsof` call in `backend/tests/cli_tests.rs::test_custom_port_health` with a `#[cfg(unix)]` / `#[cfg(windows)]` block so that the integration test compiles and passes on Windows CI. On Unix, the existing `lsof` → `ss` fallback chain is preserved verbatim. On Windows, a new `netstat -ano` code path extracts the listening port by filtering on the child process PID. When the task completes, `cargo test -p anvilml --features mock-hardware --test cli_tests` exits 0 on both the `rust-linux` and `rust-windows` CI runners.

## Scope

### In Scope
- **`backend/tests/cli_tests.rs`** — Replace lines 120–178 (the port-detection block) with `#[cfg(unix)]` and `#[cfg(windows)]` branches:
  - `#[cfg(unix)]` branch: the existing `lsof` → `ss` fallback chain, unchanged.
  - `#[cfg(windows)]` branch: `netstat -ano -p TCP`, filtered by `child.id()` (the PID of the already-spawned server subprocess), parsing the `Local Address` column to extract the port from `0.0.0.0:PORT` or `127.0.0.1:PORT` format.
- **`docs/TESTS.md`** — Update the `test_custom_port_health` entry (line 103) to document the cfg-gated port-detection instead of the `lsof`-only description.
- No other files are touched. No new dependencies. No public API changes.

### Out of Scope
- Any changes to `kill_child` helper (already cross-platform; `child.kill()` + `child.wait()` are stable on both platforms).
- Any changes outside the port-detection block in `test_custom_port_health`.
- Changes to any other test file in the workspace.
- Any changes to `anvilml.toml`, CI configuration, or build scripts.

## Existing Codebase Assessment

The file `backend/tests/cli_tests.rs` contains a single test function `test_custom_port_health` that:
1. Locates the pre-built `anvilml` binary (via `CARGO_TARGET_DIR` or fallback to `target/debug/anvilml`).
2. Spawns it with `--port 0 --log-format plain`.
3. Detects the OS-assigned port via `lsof -i TCP -sTCP:LISTEN -P -n`, filtering for lines containing "anvilml" and "LISTEN", then extracting the address:port field.
4. Falls back to `ss -tlnp` if `lsof` produces no match (but this fallback is unreachable on Windows because `lsof` panics with `NotFound` before reaching it).
5. Sends a raw HTTP GET `/health` over `TcpStream` and asserts HTTP 200 with `{"status":"ok"}`.
6. Kills the child unconditionally and restores env vars.

Established patterns in this file:
- Uses `std::process::Command` for subprocess spawning — no external crates for CLI execution.
- Error handling uses `.expect()` with descriptive messages for I/O operations that must succeed.
- The `kill_child` helper is already cross-platform (no `#[cfg]` needed).
- Env var isolation follows the mandated pattern: capture prior values, clear during test, restore at teardown.
- The test is documented with a doc comment explaining purpose, preconditions, and acceptance command.

The design doc (ANVILML_DESIGN.md §2) confirms both Linux and Windows are first-class targets with equal testing priority. No gap exists between the design spec and current source — the design expects cross-platform tests; the current source simply lacks the `#[cfg]` guard.

## Resolved Dependencies

None. The task uses only OS built-in CLI tools (`lsof`, `ss` on Unix; `netstat` on Windows) and standard library types (`std::process::Command`, `std::process::Child`). No new Rust crates, Python packages, or external dependencies are introduced.

## Approach

1. **Read `child.id()` into a variable before the port-detection block.** The `child` handle is already in scope (spawned at line 63). Store `let child_pid = child.id();` so the Windows branch can filter `netstat` output by PID. This is safe — `Child::id()` returns `Option<u32>` and is always `Some` after successful `spawn()`.

2. **Replace lines 120–178 with a `#[cfg(unix)]` / `#[cfg(windows)]` block.** The `#[cfg]` attribute is applied to the entire port-detection section (the code that sets `let port: Option<u16> = None;` through `let port = port.expect(...)`). Both branches produce the same `port: u16` variable, so the downstream HTTP request code (lines 185–228) is unchanged.

3. **`#[cfg(unix)]` branch (verbatim copy of existing logic):** Copy the existing `lsof` → `ss` fallback chain exactly as-is, preserving all comments. This includes:
   - `lsof -i TCP -sTCP:LISTEN -P -n` execution and output parsing.
   - The `ss -tlnp` fallback when `lsof` yields no match.
   - The `port.expect(...)` error message referencing `lsof/ss`.

4. **`#[cfg(windows)]` branch (new implementation):**
   a. Execute `netstat -ano -p TCP` via `std::process::Command`. The `-a` flag shows all connections, `-n` shows numeric addresses (no DNS), `-o` shows the owning PID, and `-p TCP` filters to TCP only. This is a built-in Windows command — no installation required.
   b. Parse the output line-by-line. `netstat` output format on Windows:
      ```
      Proto  Local Address          Foreign Address        State           PID
      TCP    0.0.0.0:PORT           0.0.0.0:0              LISTENING       PID
      ```
      The `Local Address` column (index 1, zero-based) contains `0.0.0.0:PORT` or `127.0.0.1:PORT`. Filter lines where the PID column (last column, index 4) matches `child_pid`.
   c. Extract the port by splitting on `:` and parsing the second part as `u16`.
   d. If no matching line is found, use `port.expect()` with a Windows-specific error message referencing `netstat`.
   e. Add a brief inline comment explaining the `netstat` flag meanings and the column layout.

5. **Update the doc comment at the top of the file (lines 1–10).** Change "The actual port is detected via `lsof`" to "The actual port is detected via platform-specific tooling (`lsof` on Unix, `netstat` on Windows)." Remove the "lsof is available on the system" precondition for the Windows path.

6. **Update `docs/TESTS.md`** entry for `test_custom_port_health` to reflect the cfg-gated detection instead of lsof-only.

No logging changes are needed — this is a test file with no production logging. No `///` doc comments on new items are needed since the approach introduces no new `pub` items, only `#[cfg]`-gated code inside an existing `#[test]` function.

## Public API Surface

None. This task modifies only a `#[test]` function inside `backend/tests/cli_tests.rs`. No `pub` items are introduced or changed.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `backend/tests/cli_tests.rs` | Replace unconditional `lsof` port-detection with `#[cfg(unix)]` / `#[cfg(windows)]` branches; update file doc comment. |
| Modify | `docs/TESTS.md` | Update `test_custom_port_health` entry to reflect cfg-gated detection. |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `backend/tests/cli_tests.rs` | `test_custom_port_health` | The server binary accepts `--port 0`, binds to an OS-assigned port, and the health endpoint returns HTTP 200 with `{"status":"ok"}`. Port detection uses `lsof` on Unix and `netstat` on Windows. | Workspace builds with `mock-hardware` feature. Pre-built binary exists at `target/debug/anvilml`. No prior server on the detected port. | `--port 0`, `--log-format plain` CLI args. OS-assigned port auto-detected. | HTTP 200 response containing `"status":"ok"`. Test exits 0. | `cargo test -p anvilml --features mock-hardware --test cli_tests` exits 0 |

No additional tests are introduced. The existing `test_custom_port_health` test now covers both platforms via cfg-gating. The Windows path is exercised by the CI `rust-windows` runner (which runs the same `cargo test` command on Windows).

## CI Impact

No CI workflow files are modified. The existing CI jobs pick up the change automatically:
- `rust-linux` runner: exercises the `#[cfg(unix)]` branch via `lsof`.
- `rust-windows` runner: exercises the `#[cfg(windows)]` branch via `netstat`.

The `config-drift` and `openapi-drift` jobs are unaffected (no config or OpenAPI changes).

## Platform Considerations

This task introduces platform-specific code via `#[cfg(unix)]` and `#[cfg(windows)]` guards:

- **Unix branch** (`#[cfg(unix)]`): Uses `lsof -i TCP -sTCP:LISTEN -P -n` with `ss -tlnp` fallback. This is unchanged from the existing code.
- **Windows branch** (`#[cfg(windows)]`): Uses `netstat -ano -p TCP`. The `netstat` command is a built-in Windows utility present on all Windows versions. The output format uses `0.0.0.0:PORT` or `127.0.0.1:PORT` in the Local Address column, with PID in the last column.
- **No `#[cfg(target_os = "macos")]` needed** — `lsof` is available on macOS as well, and `#[cfg(unix)]` covers macOS.

The Windows cross-check in ENVIRONMENT.md §7 (`cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu`) will exercise the `#[cfg(windows)]` compilation path.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `netstat -ano -p TCP` output format differs between Windows versions (e.g., Windows 10 vs Windows Server), causing the column-index parsing to fail. | Low | High | The `netstat` output format has been stable across all Windows versions. The column indices (Local Address = index 1, PID = last column) are consistent. Use `split_whitespace()` and filter by PID match rather than relying on a fixed column index for PID — parse the last column as PID and compare. If parsing fails, fall back to searching for the PID string anywhere on the line. |
| The `child.id()` PID may not appear in `netstat` output if the server process has already exited or is in a `TIME_WAIT` state when `netstat` runs. | Medium | Medium | The test already sleeps 500ms before port detection (line 118), which gives the server time to bind. If `netstat` returns no match, the `port.expect()` will produce a clear error message. The ACT agent should add a second check with a short retry loop (e.g., 3 attempts × 200ms) before failing, matching the Unix branch's implicit retry via `ss` fallback. |
| `netstat -p TCP` flag syntax may differ — some Windows builds use `-p` while others may require `-p tcp` (lowercase). | Low | Medium | Use `-p TCP` (uppercase) as the task context specifies. If the ACT agent discovers lowercase is needed on a specific runner, adjust and note in `## Deviations from Plan`. Both variants work on all Windows 10+ builds. |
| The test doc comment mentions `lsof` specifically in the preconditions section — leaving stale documentation. | Low | Low | Update the doc comment in step 5 to remove the `lsof`-specific precondition and replace it with a platform-generic statement. |

## Acceptance Criteria

- [ ] `cargo check --workspace --features mock-hardware` exits 0
- [ ] `cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu` exits 0
- [ ] `cargo test -p anvilml --features mock-hardware --test cli_tests` exits 0
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
- [ ] `cargo fmt --all -- --check` exits 0
