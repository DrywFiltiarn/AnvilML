# Plan Report: P902-A1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P902-A1                                           |
| Phase       | 902 — Stabilisation Retrofit                      |
| Description | Fix ipc-probe binary to use write_frame/read_frame correctly |
| Depends on  | none                                              |
| Project     | anvilml                                           |
| Planned at  | 2026-06-07T20:01:42Z                              |
| Attempt     | 1                                                 |

## Objective

Fix the `ipc-probe` binary so it writes frames in the Python-compatible flat dict format (with `_type` discriminator key) that `read_frame()` expects, instead of hand-rolling a raw `WorkerEvent::Pong` frame using `rmp_serde::to_vec_named(&event)` which produces serde native enum encoding (`{"Pong":[7]}`). This eliminates the `_type field missing or not a string` error.

## Scope

### In Scope
- Modify `crates/anvilml-ipc/src/bin/ipc-probe.rs`: replace the hand-rolled write side with code that produces flat-dict Pong event format (`{"_type":"Pong","seq":7}`), keeping the existing `read_frame` call unchanged on the read side.
- Update imports in `ipc-probe.rs`: add `serde_json::json`, remove unused `WorkerEvent` import (no longer needed since we construct the flat dict directly).
- Bump `anvilml-ipc` crate patch version from `0.1.1` to `0.1.2` in `Cargo.toml`.

### Out of Scope
- No changes to `framing.rs` — `write_frame` and `read_frame` are already correct.
- No changes to `messages.rs` — no enum modifications needed.
- No changes to any other crate or file.
- No new test files (the existing `framing.rs::read_frame_roundtrip` test exercises the same format).

## Approach

1. **Replace write logic** (current lines 9–15):
   - Remove: `let event = WorkerEvent::Pong { seq: 7 }; let payload = rmp_serde::to_vec_named(&event)?;`
   - Replace with flat-dict construction that produces the same format as the Python worker:
     ```rust
     let pong = serde_json::json!({ "_type": "Pong", "seq": 7u64 });
     let payload = rmp_serde::to_vec_named(&pong)?;
     ```
   - Keep the header length writing (4 bytes big-endian u32) and payload write — these are framing operations, not message serialization.

2. **Update imports**:
   - Remove: `use anvilml_ipc::WorkerEvent;` (no longer needed — the match arm can use full path or we keep it since the match result still needs the type). Actually, `WorkerEvent` IS still needed for the match arm pattern on lines 20–21 (`WorkerEvent::Pong { seq: 7 }`). So keep the import.
   - Add: `use serde_json::json;` (for constructing flat-dict Pong)

3. **Update comment** (lines 9–10): adjust to reflect that we're now writing a flat-dict Pong event frame compatible with Python's serialization format.

4. **Bump version**: change `version = "0.1.1"` → `version = "0.1.2"` in `crates/anvilml-ipc/Cargo.toml`.

Note: The task description suggests using `write_frame(&mut tx, &WorkerMessage::Ping { seq: 7 })`, but `write_frame` only serializes `WorkerMessage` types (Rust→Python direction), and there is no `WorkerEvent::Ping` variant for the read side to match against. The ACT agent should use the flat-dict Pong event format approach instead, which produces a frame that `read_frame()` can correctly deserialize as `WorkerEvent::Pong { seq: 7 }`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-ipc/src/bin/ipc-probe.rs` | Replace manual serde-native serialization with flat-dict Pong construction; update imports and comment |
| Modify | `crates/anvilml-ipc/Cargo.toml` | Bump patch version `0.1.1 → 0.1.2` |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `crates/anvilml-ipc/src/framing.rs` (inline tests) | `read_frame_roundtrip` | Flat-dict Pong frame round-trips correctly through write+read — same format the fixed probe will produce |

## CI Impact

No CI changes required. The task modifies only the `ipc-probe` binary and one `Cargo.toml`. No new gates or workflow files are affected. Existing test suite (`cargo test -p anvilml-ipc`) continues to pass.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Task description suggests `write_frame(&mut tx, &WorkerMessage::Ping { seq: 7 })` which would write a Ping frame, but `read_frame` cannot deserialize `_type: "Ping"` (no `WorkerEvent::Ping` variant exists). | High | The described approach would fail at read/match time. | ACT agent should use flat-dict Pong event format (`serde_json::json!({ "_type": "Pong", "seq": 7u64 })`) which is parseable by `read_frame`. This matches the existing `framing.rs::read_frame_roundtrip` test pattern. |
| Removing `WorkerEvent` import breaks the match arm on lines 20–21. | Low | Compilation error. | Keep `use anvilml_ipc::WorkerEvent;` — it's still needed for the match pattern. The task description says "no longer needed on the write side" but it IS still needed on the read (match) side. |
| Flat-dict serialization produces slightly different msgpack bytes than Python's implementation, causing `read_frame` deserialization to fail. | Low | Runtime error on probe execution. | Use the exact same approach as the passing `framing.rs::read_frame_roundtrip` test (`serde_json::json!` + `rmp_serde::to_vec_named`), which has been verified to work with `read_frame`. |

## Acceptance Criteria

- [ ] `cargo run -p anvilml-ipc --bin ipc-probe` prints `OK seq=7` and exits 0
- [ ] `cargo test -p anvilml-ipc` exits 0 (all existing tests pass)
- [ ] Cross-platform: `cargo check --package anvilml-ipc --target x86_64-pc-windows-gnu` exits 0
