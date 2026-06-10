# Plan Report: P15-A3

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P15-A3                                            |
| Phase       | 015 — Live Job Events                             |
| Description | anvilml: documented websocat/browser proof of live job events |
| Depends on  | P15-A2                                            |
| Project     | anvilml                                           |
| Planned at  | 2026-06-10T12:00:00Z                              |
| Attempt     | 1                                                 |

## Objective

Create `docs/PROOF_phase015.md`, a self-contained, human-readable guide that enables any developer to observe the full WebSocket job lifecycle (`job.queued` → `job.started` → `job.progress` → `job.image_ready` → `job.completed`) by running three terminal commands against a mock AnvilML server. No new source code is written — this task is documentation-only.

## Scope

### In Scope
- Create `docs/PROOF_phase015.md` with:
  - Prerequisites section (websocat, Python on PATH, `valid_zit_job.json`)
  - Three-terminal command sequence (server, websocat, curl)
  - Expected JSON frame sequence for each event type, with exact field names and example values drawn from the `WsEvent` type definitions in `crates/anvilml-core/src/types/events.rs`
  - Notes on `system.stats` interleaving
  - Troubleshooting tips (port conflict, Python missing, mock env vars)
- Verify the document is self-consistent and references the correct file paths used in the phase.

### Out of Scope
- Any source code changes (P15-A1 and P15-A2 handle wiring and testing).
- Modifying `valid_zit_job.json` (already exists at repo root).
- Browser-based proof (websocat is the documented tool; browser WebSocket devtools is mentioned as an alternative but not required).
- CI changes, tests, or formatter runs.

## Approach

1. **Read the event type definitions** in `crates/anvilml-core/src/types/events.rs` to extract the exact JSON field names and types for each of the five job lifecycle events (`JobQueuedEvent`, `JobStartedEvent`, `JobProgressEvent`, `JobImageReadyEvent`, `JobCompletedEvent`), plus `SystemStatsEvent` (interleaved).

2. **Read `valid_zit_job.json`** at the repo root to confirm the exact curl payload shape and include it in the proof as the file to POST.

3. **Read `docs/TASKS_PHASE015.md`** to confirm the Runnable Proof commands and ensure the plan matches the documented terminal sequence.

4. **Write `docs/PROOF_phase015.md`** with the following structure:
   - **Title & one-line summary**
   - **Prerequisites**: websocat installed, Python 3 on PATH, `valid_zit_job.json` present
   - **Step-by-step commands** (three terminals):
     - Terminal 1: `ANVILML_WORKER_MOCK=1 ANVILML_VENV_PATH=./venv cargo run --features mock-hardware`
     - Terminal 2: `websocat ws://127.0.0.1:8488/v1/events`
     - Terminal 3: `curl -s -X POST http://127.0.0.1:8488/v1/jobs -H 'content-type: application/json' -d @valid_zit_job.json`
   - **Expected output**: For each event in order, show the exact JSON shape with example values:
     - `job.queued`: `{ "JobQueued": { "event": "job.queued", "timestamp": "...", "job_id": "..." } }`
     - `job.started`: `{ "JobStarted": { "event": "job.started", "timestamp": "...", "job_id": "..." } }`
     - `job.progress`: `{ "JobProgress": { "event": "job.progress", "timestamp": "...", "job_id": "...", "node_index": 0, "node_total": 5, "node_type": "ZitLoadPipeline", "step": null, "step_total": null } }` (one or more)
     - `job.image_ready`: `{ "JobImageReady": { "event": "job.image_ready", "timestamp": "...", "job_id": "...", "artifact_hash": "sha256:...", "width": 1024, "height": 1024, "seed": 42 } }`
     - `job.completed`: `{ "JobCompleted": { "event": "job.completed", "timestamp": "...", "job_id": "..." } }`
   - Note that `system.stats` frames may appear between job events (every ~5 seconds).
   - **Troubleshooting**: common issues (port 8488 in use, Python not found, mock env vars missing).

5. **Pre-stop verification**: confirm the file exists and is well-formed.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Create | `docs/PROOF_phase015.md` | Documented websocat proof of live job events |

No existing files are modified. No source, test, config, or CI files are touched.

## Tests

None. This task is documentation-only; no test files are written or modified.

## CI Impact

No CI changes required. The new markdown file is committed alongside the phase's code changes and is not part of any automated gate.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| P15-A2 not yet merged, so the event pipeline is incomplete and the proof commands would not produce the expected frames | Low (P15-A2 is a prerequisite) | High | The plan assumes P15-A2 is complete per the phase dependency chain. If P15-A2 is not yet implemented, the ACT agent should flag this as a blocker. |
| websocat is not installed on the user's machine | Medium | Low | Document `websocat` installation instructions (e.g. `apt install websocat`, `brew install websocat`, or cargo install). |
| The mock server's default port (8488) conflicts with another local process | Low | Low | Include a troubleshooting note about changing the port via `ANVILML_PORT` env var or `--port` CLI flag. |
| Timestamps in example JSON frames are speculative | Medium | Low | Use placeholder timestamps (`2026-06-10T12:00:00Z`) and note that real timestamps will vary. |

## Acceptance Criteria

- [ ] `docs/PROOF_phase015.md` exists and begins with `#` heading
- [ ] Document contains the three-terminal command sequence verbatim (server, websocat, curl)
- [ ] Document shows the expected JSON frame for each of the five event types (`job.queued`, `job.started`, `job.progress`, `job.image_ready`, `job.completed`) with correct field names matching `crates/anvilml-core/src/types/events.rs`
- [ ] Document notes `system.stats` interleaving
- [ ] Document includes troubleshooting tips
- [ ] File is > 30 lines
