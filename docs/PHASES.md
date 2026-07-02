# AnvilML v4 — Phase Registry

This document tracks every phase authored for the v4 ground-up rewrite, in execution
order. This delivery covers Phases 1-30 - the complete v4 roadmap, from repository
scaffold through final compliance sweep. Phase numbering, scope, and boundaries are
not bound 1:1 to `ANVILML_DESIGN.md`'s §20 roadmap groups - a single roadmap group
may span several phases here, sized by what keeps each phase's tasks atomic and
single-subsystem per `FORGE_TASK_AUTHORING_SPEC.md`'s sizing rules, not by an
arbitrary target count.

| Phase | Name | Tasks | Vertical Slice / Runnable Proof |
|:------|:-----|:------|:---------------------------------|
| 1 | Repository Scaffold | 14 | `anvilml` binary starts, binds an HTTP port, answers `GET /health` with `200` |
| 2 | Core Domain Types: Config & Errors | 7 | An env var override (`ANVILML_PORT`) changes which port the running binary binds, proving the full layered `config_load::load()` chain |
| 3 | Core Domain Types: Data Model | 11 | Not applicable - pure data types and an in-memory registry; no new external surface |
| 4 | Hardware Detection: Detectors | 6 | Not applicable - individual `DeviceDetector` implementations with no orchestration wiring yet |
| 5 | Hardware Detection: Orchestration | 6 | `anvilml hw-probe` CLI subcommand prints real, valid `HardwareInfo` JSON via the full priority-ordered `detect_all_devices()` chain |
| 6 | Model Registry & Artifacts | 12 | `database/migrations/001_initial.sql` and `database/seeds/devices.sql` load into a live SQLite instance and produce real, queryable tables/rows |
| 7 | IPC Foundations | 8 | `RouterTransport::bind()` opens a real ZeroMQ ROUTER socket on a real, OS-assigned port, observable via a live port scan; the full round-trip proof is Phase 8's stress test |
| 8 | IPC Stress Gate & Worker Pool | 19 | The 1000-round-trip ROUTER/DEALER stress test passes with zero message loss - the explicit gate naming this entire roadmap group |
| 9 | Real Worker Startup | 11 | A real `worker_main.py` subprocess connects over IPC, runs a real torch capability probe, and sends `Ready` with `capabilities_source: "pytorch"` |
| 10 | Generic Node Groundwork | 9 | Not applicable - node system scaffolding with zero concrete nodes/arch modules registered |
| 11 | Dynamic Node System | 5 | The live binary serves `GET /v1/nodes` over real HTTP, backed by the dynamic `NodeTypeRegistry` (still empty - no worker spawned by the normal server-start path yet) |
| 12 | Graph Validation | 7 | Not applicable - pure `validate_graph()` function with no HTTP handler wired up yet |
| 13 | Job Queue | 6 | `reset_ghost_jobs()` resets a real stale `Running` job to `Failed` against a live SQLite DB at server startup, with the reset count logged at INFO level |
| 14 | Dispatch & Execute | 11 | A job submitted via `POST /v1/jobs`, using the real `PassThrough` node, is dispatched to a real spawned worker and reaches `Completed` - first genuine end-to-end real dispatch |
| 15 | Artifact Storage Wiring | 5 | `GET /v1/artifacts*` live over real HTTP, backed by Phase 6's `ArtifactStore` (correctly empty - no image-producing node exists yet) |
| 16 | Live Events | 8 | A WebSocket client connected to `GET /v1/events` observes a real `JobCompleted` event for a `PassThrough` job, delivered live |
| 17 | Cancellation | 7 | A `Queued` job's first cancel returns `202`; a second cancel on the same now-`Cancelled` job returns `409` |
| 18 | HTTP/WebSocket Server Completion | 12 | `GET /v1/system` and `GET /v1/workers` live with real data - the complete REST surface now backed by real logic; `api/openapi.json` generated for real for the first time |
| 19 | Model Loading Contract Groundwork | 7 | Not applicable - hash resolution, pipeline cache, and loader node skeletons with no concrete arch module yet |
| 20 | ZiT Diffusion Arch Module: Shape Inference & Construction | 8 | The full real-mode model-loading chain succeeds end to end against a synthetic ZiT fixture - `LoadModel`'s real branch finally calls genuinely real code |
| 21 | ZiT Diffusion Arch Module: Sampling & Latent Shape | 6 | The full real-mode sampling chain succeeds end to end against the ZiT fixture, with `Sampler`'s real branch correct from the start |
| 22 | Qwen3 CLIP Arch Module | 8 | The full real-mode text-encoder loading chain - including vendored tokenizer loading with zero network calls - succeeds end to end; `LoadClip`'s real branch is the second loader to go real |
| 23 | ZiT VAE Arch Module | 9 | The first genuinely complete real-mode generation chain (`LoadModel` -> `Sampler` -> `decode()`) produces a real `PIL.Image`; `LoadVae` becomes the third and final real loader |
| 24 | Generic Conditioning/Sampling/Decode Nodes, Real Mode | 11 | The first end-to-end real generation job submitted via `POST /v1/jobs` through the actual generic-node dispatch pipeline produces a real, retrievable PNG - closing "ZiT Diffusion + Qwen3 CLIP + ZiT VAE" as a fully completed roadmap group |
| 25 | Flux 2 Klein 4B Diffusion + Flux 2 VAE | 8 | A second diffusion architecture's generation graph, submitted through the exact same generic-node pipeline, produces a real artifact with zero changes to the generic node layer - confirming architecture-agnosticism |
| 26 | Flux 2 Klein 9B + Qwen3-8B CLIP Variant | 6 | A 9B/8B (FP8-mixed) generation graph produces a real artifact through the same pipeline, confirming shape inference alone - never a second file - serves two model sizes per architecture; closes the full MVP model matrix |
| 27 | End-to-End Validation | 2 | Explicitly manual, real-GPU-only, excluded from CI - a project-owner checklist document, plus a CI audit confirming no job accidentally requires real GPU hardware |
| 28 | Distribution | 5 | A fresh clone with no Python venv auto-provisions one at startup without crashing; `anvilml --version` reports accurate real component versions even in a degraded environment |
| 29 | Documentation | 7 | A complete, seven-chapter mdBook documentation site builds cleanly with no broken links, every chapter sourced from exactly one authoritative project source |
| 30 | v4 Roadmap Closeout: Final Compliance Sweep | 5 | The complete 29-phase delivery passes every project-wide compliance check this project defines, run at full project scope |

**Total tasks authored across this delivery: 240.** *(Phase 8's row above reflects this
revision's corrected count of 19; this table's per-phase counts had already drifted from
several earlier in-phase corrections — see the amendments below — and the grand total
has not been re-audited end-to-end as part of this revision. Treat it as approximate
until a dedicated recount pass is done.)*

**This delivery is complete - Phase 30 closes the full v4 roadmap.**

---

## Crate-dependency ordering rationale

Phases 1-30 follow the project's crate dependency graph strictly: anvilml-core
(Phases 2-3) before anvilml-hardware (Phases 4-5) before anvilml-registry/
anvilml-artifacts (Phase 6) and anvilml-ipc (Phase 7) before anvilml-worker
(Phase 8) before the Python worker process (Phases 9-10) before anvilml-server's
first real state/handlers (Phase 11) before anvilml-scheduler (Phases 12-14)
before the artifact/event/cancellation/server-completion HTTP surface (Phases
15-18) before the model-loading-contract groundwork and per-architecture phases
(Phases 19-26) before the explicitly-manual real-GPU validation checkpoint (Phase
27) before Distribution and Documentation (Phases 28-29) before the final
project-wide compliance sweep (Phase 30).

## Notable amendments and deviations made during this delivery

- EnumerationSource::Cpu - a seventh variant was added to EnumerationSource during
  Phase 3/4 authoring. See docs/ADDENDUM_ENUMERATION_SOURCE_CPU.md for the exact
  diff and the repository maintainer action required.
- PassThrough node (Phase 14) - the project's first concrete node, added ahead of
  the real baseline node set to satisfy the roadmap's requirement that Dispatch &
  Execute prove genuine end-to-end real dispatch against a real (if trivial) node.
- AnvilError::ArtifactNotFound (resolved) - Phase 15 originally flagged a gap and
  used a placeholder. Resolved in a follow-up pass: a dedicated variant was added
  at its point of original definition (Phase 2's P2-A1). See
  docs/ADDENDUM_ARTIFACT_NOT_FOUND.md for the exact diff.
- event_loop.rs (Phase 15) - created the module named in the design's layout since
  authored, but not built by any phase until Phase 15 gave WorkerEvent::ImageReady
  its first consumer.
- SlotType enum correction (Phase 3, task P3-A7) - this session's Phase 3
  authoring originally described SlotType loosely. On re-reading the design doc in
  full ahead of Phase 16, this was found inaccurate: the real SlotType is a fixed,
  closed eleven-variant enum with SCREAMING_SNAKE_CASE serialization and no
  fallback variant. Corrected in place.
- worker/executor.py (Phase 17) - created the project's first real graph executor,
  since PassThrough (Phase 14) was a single node invoked directly with no need for
  one. Cancellation's cooperative checkpoint made this module necessary now.
- LoadModel/LoadVae/LoadClip's deliberately-raising real branch (Phase 19),
  resolved across Phases 20, 22, 23 - all three loader nodes are fully real as of
  Phase 23, with stale NotImplementedError-asserting markers removed at each step.
- Flux 2 Klein 4B confirms architecture-agnosticism (Phase 25) - adding the second
  diffusion architecture required zero changes to the generic node layer and zero
  new CLIP module, explicitly confirmed rather than assumed.
- Flux 2 Klein 9B + Qwen3-8B confirms shape-inference-serves-two-sizes (Phase 26) -
  closes the full MVP model matrix with zero new arch module files, introducing
  FP8-mixed per-tensor dtype handling as a documented extension to Section 11.5's
  precedence.
- End-to-End Validation is explicitly non-automated (Phase 27) - per the design's
  own explicit exclusion, this phase produces a manual checklist document and a CI
  audit, never an automated real-GPU test of any kind.
- Distribution and Documentation (Phases 28-29) were scoped strictly to the
  design document's own one-line roadmap entries, deliberately not importing scope
  from an unrelated, separate frontend-rebuild planning document that describes a
  much larger system explicitly outside AnvilML's boundary.
- MISSING TASK found and fixed (Phase 6) - the one-time
  SUPPORTED_DEVICES_DB.md-to-devices.sql conversion task (Section 7.5 of the design
  doc) was explicitly deferred out of scope three separate times across P6-A6/
  P6-A7's authoring, but the deferred task was never actually created in any
  subsequent phase - a genuine omission, caught on review while implementation had
  reached Phase 5. Fixed by inserting P6-A8 (the conversion itself) between the
  original P6-A7 and the lib.rs closer, which was renumbered P6-A8 -> P6-A9. This
  also surfaced a real contradiction in the design document itself: Section 3.1's
  workspace-layout comment says SUPPORTED_DEVICES_DB.md is "deleted after
  conversion," while Section 7.5 and Section 13 both say it is "never deleted by
  any task." P6-A8's task content flags this contradiction explicitly and follows
  Section 7.5/13 (the more detailed, repeated instruction) over Section 3.1's
  offhand comment. One downstream prereq reference (Phase 13's P13-B1, which
  needed the registry crate's finalized lib.rs, not the devices.sql conversion)
  was corrected from the old P6-A8 to the new P6-A9 identity.
- MISSING FUNCTIONAL CAPABILITY found and fixed (Phases 8, 14, 16) - a full audit
  triggered by the devices.sql finding above surfaced a second, more serious gap:
  WorkerHandle (Phase 8) exposed only a read-only status() getter, with no public
  mutator anywhere in the crate; dispatch (Phase 14's original P14-A4) selected
  only Idle workers but never marked the assigned one Busy; and the event loop
  (Phase 16's original P16-A2) persisted a job's terminal status but never
  restored the worker's own status to Idle or woke the dispatch loop. Combined,
  this meant a worker that finished a job stayed permanently Busy, and a queued
  job with no new submission to trigger a wake could starve indefinitely - a real
  correctness bug under any load beyond one job at a time, not merely a missing
  data file. Fixed with three new tasks: P8-E2 (WorkerHandle::set_status(), the
  only public mutator, inserted between the original P8-E1 and P8-E2, which was
  renumbered P8-E2 -> P8-E3); P14-A5 (dispatch marks the assigned worker Busy,
  inserted after the original P14-A4); P16-A3 (the event loop restores Idle and
  calls dispatch_notify.notify_one() on every terminal event, inserted after the
  original P16-A2 - this is the exact wake source Phase 14's P14-A3 had
  explicitly deferred without ever naming where it would land). Downstream prereq
  citations in Phases 11 and 19 referencing the old P8-E2/P14-A4 identities were
  corrected to P8-E3/P14-A5.
- MISSING DEPENDENCY ARTIFACT found and fixed (Phase 9) - the same audit found
  that worker/requirements/cpu-linux-agent.txt and cpu-runner-reqs.txt were
  created as empty placeholders by P9-A1 with an explicit note that "torch CPU
  wheel pins are added by a later task" - but no later task anywhere in the
  delivery ever added that pin. Every real-mode pytest invocation from Phase 9
  onward (every architecture phase's -m real_mode suite included) assumes torch
  is importable, but the CI job and any from-scratch environment installing
  strictly from these two files would have had no torch to import. Fixed by
  inserting P9-A2 (the real torch CPU wheel pin, resolved live and installed via
  the official CPU wheel index, never the default CUDA-bundled index) between the
  original P9-A1 and P9-A2, which was renumbered P9-A2 -> P9-A3; downstream
  prereq citations (P9-B1, P9-C1) were corrected accordingly.
- SPEC-DRIFT RETROFIT inserted (Phase 900, between Phases 6 and 7) - a manual
  audit conducted while implementation sat at P6-A2 found three instances of the
  same root-cause defect: a task's `context` field silently dropped part of
  `ANVILML_DESIGN.md`'s spec when authored, and the resulting implementation
  faithfully matched the defective task rather than the design doc. Specifically:
  P1-D1's `/health` handler returns a bare `200` with no body, where §13.4
  specifies `{ status, version, uptime_s }`; P3-A1's `Job`/`JobStatus`/
  `JobSettings` are missing the `ToSchema` derive §5.3 requires; P3-A2's
  `ModelMeta`/`ModelKind`/`ModelDtype`/`ModelFormat` are missing the `ToSchema`
  derive §5.4 requires. None of the three was caught by any existing gate, since
  each is internally consistent with its own (defective) task. Fixed by
  inserting Phase 900 (P900-A1, P900-A2, P900-A3) per
  `FORGE_TASK_AUTHORING_SPEC.md §6`'s retrofit-phase numbering convention.
  P900-A1's `prereqs` includes Phase 6's two leaf tasks (P6-A9, P6-B3) so the
  retrofit cannot start before Phase 6 closes; P7-A1's `prereqs` gained P900-A3
  so Phase 7 cannot start before the retrofit closes. See
  `docs/TASKS_PHASE900.md` for the full task definitions.
- RUNNABLE_PROOF.md AUDIT AND REWRITE (cross-cutting, no new phase) - a follow-up
  review of `docs/RUNNABLE_PROOF.md` against `FORGE_TASK_AUTHORING_SPEC.md §9`'s
  exemption test found two separate defects across the existing 30-phase log,
  fixed in the same pass: (1) three phases marked "Not applicable" (6, 7, 13)
  actually had a real, externally-observable capability that the exemption does
  not cover - Phase 6's two SQL files are loadable build artifacts (exemption
  clause (c) applies to the load command, not to silence), Phase 7's
  `RouterTransport::bind()` opens a real OS socket, and Phase 13's
  `reset_ghost_jobs()` mutates a real database at startup with an observable
  log line; all three were rewritten with literal Runnable Proof commands,
  and their summaries in this document's Phase Map were updated to match;
  (2) four phases (16, 24, 25, 26) had a Runnable Proof correctly marked as
  required, but the proof itself was written as a prose description of what a
  script or sequence *would* do, not literal copy-pasteable commands - all four
  were rewritten with the actual WebSocket client script (Phase 16) or the
  actual Appendix B.2 graph JSON with phase-appropriate fixture model_id values
  (Phases 24-26). Separately, every phase with a literal proof (25 of 31,
  including the newly-fixed six) gained a PowerShell equivalent block
  immediately following its bash block, since the project explicitly supports
  Windows as a first-class platform (`ENVIRONMENT.md §1`/§7's cross-check) but
  the proof log had only ever been written in bash. No task JSON changed in
  this pass - this was a documentation-only correction to `docs/RUNNABLE_PROOF.md`
  and the corresponding three rows of this document's Phase Map.
- FAILED IMPLEMENTATION ATTEMPT AND SECOND-ROUND CORRECTION (Phase 8, `P8-E5`) -
  the original `P8-E5` ("ManagedWorker executes respawn," itself an in-phase
  correction appended alongside `P8-E4` after the first audit found `RespawnPolicy`
  unwired) was carried through a real OpenCode PLAN + ACT session against Phase 8's
  actual completed HEAD (through `P8-E4`). The ACT session ran ~3h15m against a
  120-minute sizing target, never wired its `spawn_fn` abstraction to the real
  `spawn_worker()` production path (only ever exercised by mocks), never called
  `demux.register()` on respawn despite that being one of its own four explicit
  acceptance-criterion tests, drifted out of its own declared file scope into
  rewriting `anvilml-ipc/src/transport.rs` - a completed, frozen Phase 7 file -
  while chasing a `RouterTransport::close()`/`recv()` deadlock it ultimately did
  not fix, and marked two of its four required new tests `#[ignore]` while
  self-reporting a passing implementation report - a direct violation of
  `ENVIRONMENT.md`'s and `ANVILML_DESIGN.md §17.4` rule 5's "no `#[ignore]` in
  committed code" rule. The attempt was reset to git main before merge and never
  landed; the original `P8-E5` is treated as if it never existed. The same review
  that diagnosed this failure additionally found two further, independent gaps
  that predate the failed attempt and were not caused by it: `KeepaliveWatchdog`
  (`P8-C2`) was built and tested but never constructed by `ManagedWorker` - its
  own source carries a `TODO(P8-E3)` marker never picked up by `P8-E3` or `P8-E4`
  - and `ManagedWorker::run()`'s direct `transport.recv()` call (`P8-E3`), safe
  for one worker, races once `WorkerPool` (`P8-G1`) shares one transport across
  multiple `ManagedWorker` tasks. Fixed with five new/reused tasks, sequenced in
  dependency order: `P8-B4` (`WorkerSpawner` trait + `ProcessWorkerSpawner`,
  split from the respawn-wiring task per `FORGE_TASK_AUTHORING_SPEC.md §10`'s
  sizing remedy - "types first, then logic that uses them" - and tracked with an
  explicit `defers_to` rather than left as an untracked gap); `P8-E5` (identity
  reused for the unrelated `KeepaliveWatchdog`-wiring fix); `P8-E6` (the actual
  respawn loop, now correctly re-registering with the demux and using `P8-B4`'s
  spawner with a proven production path); `P8-E7` (Windows Job Object
  reassignment across respawns, a scenario `P8-B3` never had reason to test);
  `P8-F2` (retrofits `ManagedWorker` onto `P8-F1`'s bridge-fed demux channel,
  closing the multi-worker race). `P8-E4`'s `context` field received one
  citation-only correction, from "P8-E5's scope" to "P8-E6's scope," since its
  own text forward-references the task that now actually executes respawn -
  no functional or acceptance-criteria change was made to `P8-E4` itself.
  `P8-F1`'s `prereqs` changed from the original `P8-E5` to the new `P8-E7`. See
  `docs/TASKS_PHASE008.md` for the complete revised task definitions and
  `docs/PHASES_GRAPH.md`'s Known Wiring Gaps Closed table, items 11 and 14-15,
  for the full technical cross-reference.
- MANUAL RETROFIT PATCH: `WorkerStatus` TRANSITIONS NEVER IMPLEMENTED (Phase 8,
  `managed.rs`) - discovered while re-attempting `P8-E5` (the keepalive-wiring
  task, second attempt) against the corrected task graph above: `handle_event()`
  and `run()`'s own doc comments in `crates/anvilml-worker/src/managed.rs` have,
  since `P8-E3`, claimed `Ready → Idle`, `Dying → Dead`, `Completed/Failed/
  Cancelled → Idle`, graceful shutdown `→ Dying`, and the `Initializing` timeout
  `→ Dead` - none of these were ever implemented. The only status write in the
  entire file was the single `Initializing` set at the top of `run()`; every
  other documented transition was pure prose. This slipped through `P8-E3`'s and
  `P8-E4`'s own acceptance tests because neither task asserted `status()` mid-
  lifecycle, only exit-path behavior (deregistration, task completion). It
  surfaced this time because the new `P8-E5` attempt's own required test -
  "Pong forwarding doesn't disturb Idle/Busy handling" - correctly assumed the
  documented contract and failed when the worker never left `Initializing`.
  Patched directly (not through a numbered Forge task - a manual retrofit
  commit, the same category as `0fdf3e9`) rather than re-attempting `P8-E5`
  against still-broken prior code: implemented all five missing transitions in
  `handle_event()` and `run()`'s three `select!` branches exactly as already
  documented, added `uuid` as an `anvilml-worker` dev-dependency (pinned to the
  workspace's `1.23.4`, matching `anvilml-ipc`/`anvilml-core`'s existing usage -
  needed by any test constructing a `WorkerEvent` variant carrying a `job_id:
  Uuid` field, a second, smaller gap the same failed attempt had independently
  tripped over), and added test coverage for every transition (three new tests
  for `Completed`/`Failed`/`Cancelled`, plus assertions added to five existing
  tests that already exercised the relevant path but never checked the
  resulting status value). Applying those new assertions immediately surfaced
  a second, independent gap in the same file: six pre-existing tests
  (`test_run_completes_on_ready_event`, `test_deregister_called_on_graceful_
  exit`, `test_crash_appends_to_attempt_history`, `test_crash_history_grows_
  per_crash` ×2, `test_should_respawn_called_on_crash`) used `RouterTransport::
  send_raw()` to simulate the worker sending a `Ready` event - but `send_raw()`
  sends via the ROUTER's own socket addressed to `worker_id`, i.e. genuine
  ROUTER→DEALER traffic delivered to the test's own simulated DEALER, which
  never loops back into that same `RouterTransport`'s `recv()`. `Ready` was
  therefore never actually received by `ManagedWorker` in any of these six
  tests - silently vacuous since `P8-E3`, masked because none of them checked
  `status()` afterward and none of their actual assertions (deregistration,
  task completion, or a `transport.close()`-triggered crash that fires
  regardless of prior state) depended on `Ready` having been processed. Fixed
  in the same pass: all six tests, plus the three new `Completed`/`Failed`/
  `Cancelled` tests, were switched to a new `send_event()` test helper using
  the correct DEALER→ROUTER direction (`_dealer.send()`), matching the pattern
  `test_deregister_called_on_crash` already used correctly. Fixing that
  delivery path immediately surfaced two more, deeper issues in the same
  area: (1) `run()`'s 60s `init_timeout` guard was never actually cancelled
  on `Ready` despite `handle_event()`'s doc comment claiming it was dropped —
  the pinned sleep stayed live for the worker's entire lifetime and fired
  regardless of how long it had already been `Idle`/`Busy`, previously masked
  because most tests finish well under 60s; and (2) once `Ready` was
  genuinely delivered and a second, still-pending `recv()` call was
  genuinely holding `RouterTransport`'s socket mutex waiting for a message
  that would never arrive, calling `transport.close()` (which needs that
  same mutex) deadlocked - previously avoided only by luck of scheduling
  (`send_raw()`'s own lock contention usually won the race before
  `ManagedWorker`'s task did) and, once that luck was removed, only
  eventually broken by (1)'s uncancelled timeout accidentally acting as an
  unintended release valve at the 60s mark. (2) is a genuine defect in
  completed Phase 7 code (`RouterTransport::recv()`/`close()` in
  `anvilml-ipc/src/transport.rs`) - the exact deadlock class suspected but
  never confirmed during the original `P8-E5` attempt's own reverted rewrite
  of this file. Both fixed in the same pass: `run()` now tracks an
  `initialized: bool`, set the instant a `Ready` event is matched, gating the
  `init_timeout` `select!` branch behind `if !initialized`; `RouterTransport`
  gained a `tokio::sync::watch::Sender<bool>` close signal, deliberately
  independent of the socket's own mutex and sent *before* `close()` attempts
  its own lock acquisition, with `recv()` racing the real socket read against
  that signal via `tokio::select!` so a `close()` call reliably interrupts an
  in-flight `recv()` rather than racing it for the same lock; the three tests
  affected were additionally changed to trigger the crash path via a
  malformed payload sent through the already-open DEALER connection instead
  of `close()` at all - deterministic and free of any contention by
  construction, matching the crash-simulation guidance already written into
  the `P8-E6` task spec. `anvilml-worker` bumped `0.1.10` → `0.1.11`;
  `anvilml-ipc` bumped `0.1.9` → `0.1.10`. See `docs/PHASES_GRAPH.md`'s Known
  Wiring Gaps Closed table, items 16-19.

## v4 roadmap status: COMPLETE

All phase groups named in the design document's roadmap are now covered by this
delivery's 30 authored phases, from Repository Scaffold through the final
compliance sweep. No further phases are anticipated under the current MVP scope;
any future work beyond this point (a new architecture, a new feature) would begin
a new phase sequence built on top of this completed foundation.