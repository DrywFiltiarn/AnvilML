# Plan Report: P906-A4

| Field       | Value                                               |
|-------------|-----------------------------------------------------|
| Task ID     | P906-A4                                             |
| Phase       | 906 — OpenAPI Spec Correctness Retrofit             |
| Description | Regenerate and commit corrected backend/openapi.json |
| Depends on  | P906-A2, P906-A3                                    |
| Project     | anvilml                                             |
| Planned at  | 2026-06-12T16:10:00Z                                |
| Attempt     | 1                                                   |

## Objective

Regenerate `backend/openapi.json` by running `cargo run -p anvilml-openapi` so that the
committed spec reflects all prior fixes from this phase: P906-A1 (ModelKind schema
registration), P906-A2 (png binary schema + required fields), and P906-A3 (BF16
serde rename from `b_f16` to `bf16`). Verify idempotency and run the full test suite.

## Scope

### In Scope
- Run `cargo run -p anvilml-openapi` to regenerate `backend/openapi.json`
- Verify the generated spec contains the four correctness checks:
  - `components/schemas/ModelKind` exists with correct enum variants
  - `components/schemas/DType.enum` contains `bf16` (not `b_f16`), `f8_e4m3`, `f8_e5m2`
  - `GET /v1/artifacts/{hash}` 200 response has `type: string, format: binary`
- Stage the regenerated file with `git add backend/openapi.json`
- Run `cargo test --workspace --features mock-hardware` to confirm zero failures

### Out of Scope
- Any source code changes (handled in P906-A1, A2, A3)
- Version bumps (handled in A1, A3)
- CI workflow modifications
- Windows path normalisation (P906-A5)

## Approach

1. **Regenerate the spec:**
   ```bash
   cargo run -p anvilml-openapi
   ```
   This reads all `#[utoipa::path]` annotations and `ToSchema`-derived types from the
   workspace, including the ModelKind schema registered in A1, the png binary schema
   from A2, and the BF16 rename from A3. Output is written to `backend/openapi.json`.

2. **Verify the file changed:**
   ```bash
   git diff --exit-code backend/openapi.json
   ```
   This must exit 1 (file differs from committed version). This confirms the generator
   produces a corrected spec different from the stale committed file.

3. **Inspect generated content for correctness:**
   - Confirm `components/schemas/ModelKind` exists with `type: string` and
     `enum: [clip, diffusion, vae, lora, control_net, unet, upscale]`
   - Confirm `components/schemas/DType.enum` contains `bf16` (not `b_f16`),
     `f8_e4m3`, `f8_e5m2`
   - Confirm `paths["/v1/artifacts/{hash}"].get.responses.200.content["image/png"].schema`
     has `type: string, format: binary`

4. **Stage the regenerated file:**
   ```bash
   git add backend/openapi.json
   ```
   The Forge orchestrator handles the commit.

5. **Run full test suite:**
   ```bash
   cargo test --workspace --features mock-hardware
   ```
   Must exit 0. This is the acceptance criterion gate.

6. **Verify idempotency (implicit acceptance criterion):**
   Running `cargo run -p anvilml-openapi` a second time must produce an identical file,
   confirmed by `git diff --exit-code backend/openapi.json` exiting 0.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Write | `backend/openapi.json` | Regenerated OpenAPI 3.1 spec from utoipa annotations |
| Stage | `backend/openapi.json` | `git add` (The Forge commits) |

No source files, test files, config files, or CI files are modified in this task.

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `cargo test --workspace --features mock-hardware` | All workspace tests | No regressions from the regenerated spec |

The regenerated `backend/openapi.json` is a data file — no new tests are written.
The existing full workspace test suite serves as the regression gate.

## CI Impact

No CI changes required. The regenerated `backend/openapi.json` is the expected output
of the OpenAPI drift gate (`Gate 2` in `docs/ENVIRONMENT.md §8`). CI already expects
this file to be committed and up-to-date. The gate command
`cargo run -p anvilml-openapi && git diff --exit-code backend/openapi.json` will exit 0
after this task stages the corrected file.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The generator produces a different spec than expected (regression in prior A1/A2/A3 fixes) | Low | Medium | Inspect the generated spec before staging; if incorrect, report blocker and stop |
| `cargo test --workspace --features mock-hardware` fails with unrelated pre-existing failure | Low | Medium | Diagnose and fix only if failure is caused by this task's changes; otherwise report as blocker |
| Generator output is not idempotent (non-deterministic field ordering) | Low | Low | The utoipa/serde_json serializer is deterministic for the same input types |
| git diff shows unexpected large diff | Low | Medium | Review diff carefully; if it includes unintended changes, stop and report |

## Acceptance Criteria

- [ ] `cargo run -p anvilml-openapi` completes successfully and writes `backend/openapi.json`
- [ ] `git diff --exit-code backend/openapi.json` exits 1 (file changed before staging)
- [ ] Generated spec contains `ModelKind` schema with correct enum values
- [ ] Generated spec `DType.enum` contains `bf16` (not `b_f16`), `f8_e4m3`, `f8_e5m2`
- [ ] Generated spec `/v1/artifacts/{hash}` 200 response has `image/png` binary schema
- [ ] `git add backend/openapi.json` stages the file
- [ ] `cargo test --workspace --features mock-hardware` exits 0
- [ ] Idempotency: second `cargo run -p anvilml-openapi` produces no diff (exits 0)
