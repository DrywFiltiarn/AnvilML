# Plan Report: P11-A1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P11-A1                                        |
| Phase       | 011 — Graph Validation                        |
| Description | anvilml-scheduler: KNOWN_NODE_TYPES + node slot table |
| Depends on  | P10-A4                                       |
| Project     | anvilml                                      |
| Planned at  | 2026-06-07T01:15:00Z                         |
| Attempt     | 1                                            |

## Objective

Create `crates/anvilml-scheduler/src/nodes.rs` defining the authoritative set of known node types (`KNOWN_NODE_TYPES`, 9 entries) and a node slot lookup table (`NODE_SLOTS`) mapping each type to its input and output slot names, per ANVILML_DESIGN §14.6. Add unit tests verifying all 9 types are present and that `ZitSampler` outputs include `latents` and `seed`.

## Scope

### In Scope
- Create `crates/anvilml-scheduler/src/nodes.rs` with:
  - `const KNOWN_NODE_TYPES: [&str; 9]` containing the nine node type names.
  - A `NODE_SLOTS` data structure (array of `(type, inputs, outputs)` tuples or a `HashMap`) providing input and output slot lists per node type, as specified in ANVILML_DESIGN §14.6.
- Expose both from `crates/anvilml-scheduler/src/lib.rs` (`pub mod nodes; pub use nodes::{KNOWN_NODE_TYPES, NODE_SLOTS};`).
- Add unit tests in `nodes.rs` under a `#[cfg(test)]` module:
  - `test_all_nine_types_present`: asserts `KNOWN_NODE_TYPES.len() == 9`.
  - `test_zitsampler_outputs_include_latents_seed`: asserts the output slots for `ZitSampler` contain both `"latents"` and `"seed"`.
- Bump `anvilml-scheduler` crate version from `0.1.0` to `0.1.1` in `Cargo.toml`.

### Out of Scope
- DAG validation logic (duplicate-id, unknown-type, edge refs, cycle detection) — tasks P11-A2 through P11-A4.
- HTTP handler wiring (`POST /v1/jobs`) — task P11-A5.
- Any changes to `anvilml-core`, `anvilml-registry`, or `anvilml-worker`.
- Any Python-side changes.

## Approach

1. **Read current state.** Confirm `crates/anvilml-scheduler/Cargo.toml` already lists `anvilml-core = { path = "../anvilml-core" }` (already confirmed). Confirm `src/lib.rs` currently only contains `pub fn stub() {}`.

2. **Write `crates/anvilml-scheduler/src/nodes.rs`.** Define:
   - `pub const KNOWN_NODE_TYPES: [&str; 9]` with the nine names in this order:
     `ZitLoadPipeline`, `ZitTextEncode`, `ZitSampler`, `ZitDecode`, `SdxlLoadPipeline`, `SdxlTextEncode`, `SdxlSampler`, `SdxlDecode`, `SaveImage`.
   - A struct `NodeSlots { pub inputs: &'static [&'static str], pub outputs: &'static [&'static str] }` (or equivalent tuple-based approach) and a `pub const NODE_SLOTS: &[(&&str, NodeSlots)]` array with one entry per node type, using the slot data from ANVILML_DESIGN §14.6:

     | Type | Inputs | Outputs |
     |------|--------|---------|
     | ZitLoadPipeline | `["model_id"]` | `["pipeline"]` |
     | ZitTextEncode | `["pipeline", "prompt"]` | `["conditioning"]` |
     | ZitSampler | `["pipeline", "conditioning", "steps", "seed"]` | `["latents", "seed"]` |
     | ZitDecode | `["pipeline", "latents"]` | `["image"]` |
     | SdxlLoadPipeline | `["model_id"]` | `["pipeline"]` |
     | SdxlTextEncode | `["pipeline", "prompt", "negative_prompt"]` | `["conditioning"]` |
     | SdxlSampler | `["pipeline", "conditioning", "steps", "guidance_scale", "seed"]` | `["latents", "seed"]` |
     | SdxlDecode | `["pipeline", "latents"]` | `["image"]` |
     | SaveImage | `["image", "prompt", "seed", "steps"]` | `[]` (empty) |

   - A helper function: `pub fn get_node_slots(type_name: &str) -> Option<&NodeSlots>` that looks up a type in the array.

3. **Wire into `lib.rs`.** Replace the stub with:
   ```rust
   pub mod nodes;
   pub use nodes::{KNOWN_NODE_TYPES, NODE_SLOTS};
   ```

4. **Add unit tests.** In `nodes.rs`, under `#[cfg(test)]`:
   - `test_all_nine_types_present` — asserts length is 9 and iterates to confirm all names are non-empty.
   - `test_zitsampler_outputs_include_latents_seed` — calls `get_node_slots("ZitSampler")`, asserts outputs contain `"latents"` and `"seed"`.

5. **Bump version.** Change `version = "0.1.0"` to `version = "0.1.1"` in `crates/anvilml-scheduler/Cargo.toml` (patch bump per FORGE_AGENT_RULES §12).

6. **Verify.** Run `cargo test -p anvilml-scheduler -- nodes` and confirm exit 0 with both tests passing.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Create | `crates/anvilml-scheduler/src/nodes.rs` | New file: KNOWN_NODE_TYPES const, NODE_SLOTS map, helper fn, unit tests |
| Modify | `crates/anvilml-scheduler/src/lib.rs` | Replace stub with `pub mod nodes; pub use` declarations |
| Modify | `crates/anvilml-scheduler/Cargo.toml` | Bump version `0.1.0 → 0.1.1` |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `crates/anvilml-scheduler/src/nodes.rs` | `test_all_nine_types_present` | `KNOWN_NODE_TYPES.len() == 9`; all entries non-empty |
| `crates/anvilml-scheduler/src/nodes.rs` | `test_zitsampler_outputs_include_latents_seed` | `ZitSampler` output slots contain `"latents"` and `"seed"` |

## CI Impact

No CI changes required. The task adds only a new module and unit tests within an existing crate. The existing CI gates (`cargo test --workspace --features mock-hardware`, `cargo clippy`, `cargo fmt`) will cover the new code automatically. No new CI jobs or steps are needed.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Data structure choice for NODE_SLOTS causes borrow-checking issues with `&str` lifetimes in const context | Medium | Low — would need to use indices or a different layout | Use a flat tuple array `&[(&str, &[&str], &[&str])]` which is const-compatible in Rust 2021; avoid custom struct with borrowed slices in const. |
| Test runs against stale Cargo.lock causing unexpected dependency resolution | Low | Low — standard `cargo test` regenerates lockfile | Run `cargo check -p anvilml-scheduler` first, then `cargo test`. |
| `anvilml-core` already listed as dependency but unused imports cause clippy warning in lib.rs | Low | Low — only add the `pub mod nodes; pub use` lines | Keep lib.rs minimal; no extra imports. |

## Acceptance Criteria

- [ ] `crates/anvilml-scheduler/src/nodes.rs` exists and defines `KNOWN_NODE_TYPES` (9 entries) and `NODE_SLOTS`
- [ ] `KNOWN_NODE_TYPES` contains exactly: ZitLoadPipeline, ZitTextEncode, ZitSampler, ZitDecode, SdxlLoadPipeline, SdxlTextEncode, SdxlSampler, SdxlDecode, SaveImage
- [ ] `NODE_SLOTS` provides input and output slot lists for all 9 types per ANVILML_DESIGN §14.6
- [ ] `ZitSampler` outputs include both `"latents"` and `"seed"` (verified by test)
- [ ] `cargo test -p anvilml-scheduler -- nodes` exits 0 with both tests passing
- [ ] `anvilml-scheduler` version bumped to `0.1.1` in Cargo.toml
