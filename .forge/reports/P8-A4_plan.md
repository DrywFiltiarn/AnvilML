# Plan Report: P8-A4

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P8-A4                                             |
| Phase       | 008 — IPC Framing                                 |
| Description | ipc-probe: standalone CLI binary proving frame round-trip |
| Depends on  | P8-A1, P8-A2, P8-A3                               |
| Project     | anvilml                                            |
| Planned at  | 2026-06-05T19:20:00Z                              |
| Attempt     | 1                                                  |

## Objective

Create a small `[[bin]]` target named `ipc-probe` inside the existing `anvilml-ipc` crate. The binary must write a `Ping{seq:7}` frame to an in-process `tokio::io::duplex`, read it back via `read_frame`, verify the result is `Pong { seq: 7 }`, print `OK seq=7` and exit 0; on any mismatch or error exit 1. This provides a runnable proof-of-framing independent of any Python worker process.

## Scope

### In Scope
- Add a `[[bin]]` entry to `crates/anvilml-ipc/Cargo.toml` pointing to `src/bin/ipc-probe.rs`.
- Create `crates/anvilml-ipc/src/bin/ipc-probe.rs` with the round-trip logic:
  - Use `tokio::io::duplex(4096)` for an in-process bidirectional pipe.
  - Call `write_frame(tx, &WorkerMessage::Ping { seq: 7 })`.
  - Call `read_frame(rx, 64)` to read the response.
  - Compare the result with `WorkerEvent::Pong { seq: 7 }`.
  - Print `OK seq=7` on match; exit 1 on mismatch or error.
- Ensure `cargo test -p anvilml-ipc` remains green after changes.
- Ensure `cargo clippy --workspace --features mock-hardware -- -D warnings` passes.

### Out of Scope
- Any Python worker interaction (this is purely in-process).
- Adding new message types or framing logic (already implemented by P8-A1–A3).
- Modifying `messages.rs`, `framing.rs`, `lib.rs`, or `Cargo.toml` dependencies.
- Cross-platform cross-checks beyond what `cargo test -p anvilml-ipc` exercises (the binary is pure Rust with no `#[cfg]` gates).
- CI workflow changes — no CI files are affected by adding a single bin target.

## Approach

1. **Append `[[bin]]` to `anvilml-ipc/Cargo.toml`.** Add one line:
   ```toml
   [[bin]]
   name = "ipc-probe"
   path = "src/bin/ipc-probe.rs"
   ```
   This follows the convention already used by `backend/` and `anvilml-openapi/` crates.

2. **Create `crates/anvilml-ipc/src/bin/ipc-probe.rs`.** The file will:
   - Import `tokio::main`, `tokio::io::{AsyncWriteExt, AsyncReadExt}`, and the crate's `write_frame` / `read_frame` / `WorkerMessage` / `WorkerEvent`.
   - Define a single `#[tokio::main]` async main function.
   - Call `tokio::io::duplex(4096)` to get `(tx, rx)`.
   - `write_frame(&mut tx, &WorkerMessage::Ping { seq: 7 }).await?` — write the frame.
   - `let result = read_frame(&mut rx, 64).await?` — read back the response.
   - Match on `result`:
     - `WorkerEvent::Pong { seq } if seq == 7 => println!("OK seq=7")`
     - `other => { eprintln!("mismatch: {:?}", other); std::process::exit(1) }`
   - The `?` propagation will cause a panic on I/O or framing errors (appropriate for a proof-of-concept binary — it should never fail in normal operation).

3. **Verify locally.** Run `cargo run -p anvilml-ipc --bin ipc-probe` and confirm output is exactly `OK seq=7`. Then run `cargo test -p anvilml-ipc` to ensure no regressions.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-ipc/Cargo.toml` | Append `[[bin]]` table for `ipc-probe` |
| Create | `crates/anvilml-ipc/src/bin/ipc-probe.rs` | New binary: in-process frame round-trip proof |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| *(none — this task creates a binary, not a test file)* | | |

The existing `framing.rs` tests (`write_frame`, `read_frame_roundtrip`, `read_frame_oversize_rejected`) already exercise the framing layer. The `ipc-probe` binary itself serves as the Runnable Proof for Phase 8 and is verified by running it directly, not via `cargo test`.

## CI Impact

No CI changes required. Adding a new `[[bin]]` target to an existing crate is automatically discovered by Cargo's workspace tooling — no `.github/workflows/` files, no Makefile, and no CI config modifications are needed. The existing CI gates (`cargo test --workspace`, `cargo clippy`, cross-checks) will naturally include the new binary in their build without any configuration change.

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| `tokio::io::duplex` requires the `rt` feature on tokio (for `#[tokio::main]`). The crate's `Cargo.toml` uses `{ workspace = true }` which pulls in `tokio` with `features = ["full"]` — already satisfied. | No action needed; verified against workspace definition. |
| Binary might not compile if `write_frame`/`read_frame` are not `pub`. Both functions are `pub async fn` in `framing.rs` — confirmed by reading the source. | No action needed. |
| `WorkerEvent::Pong` variant name or field structure could differ from expectation, causing a pattern-match failure at compile time. The design doc (§7.3) and actual `messages.rs` confirm the variant is `Pong { seq: u64 }`. | Compile-time safety — Rust will reject mismatches before runtime. |
| Duplex buffer size too small for the frame payload causing a deadlock. A `Ping{seq:7}` serializes to well under 100 bytes; duplex(4096) is more than sufficient. | No action needed; 4096 is large enough by orders of magnitude. |

## Acceptance Criteria

- [ ] `cargo run -p anvilml-ipc --bin ipc-probe` prints exactly `OK seq=7` and exits 0
- [ ] `cargo test -p anvilml-ipc` passes (no regressions from existing tests)
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` produces zero warnings
- [ ] Only two files are touched: `crates/anvilml-ipc/Cargo.toml` (append bin table) and `crates/anvilml-ipc/src/bin/ipc-probe.rs` (new file)
