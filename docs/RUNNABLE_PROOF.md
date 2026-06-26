# AnvilML v4 — Runnable Proof Log

This file aggregates the Runnable Proof for each phase, in execution order. Each
entry's commands are reproduced verbatim from that phase's `TASKS_PHASE<NNN>.md`.
This delivery covers Phases 1-30 — the complete v4 roadmap.

---

## Phase 1 — Repository Scaffold

**Capability proved:** The built `anvilml` binary starts, binds an HTTP port, and
answers a real `GET /health` request with `200`.

```bash
# Runnable Proof (manual):
cargo build --release -p anvilml
./target/release/anvilml &
sleep 1
curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:8488/health
# -> 200
kill %1
```

---

## Phase 2 — Core Domain Types: Config & Errors

**Capability proved:** The running `anvilml` binary actually loads its bind address
and port through the full layered config_load::load() chain — an environment
variable override changes observable runtime behaviour (which port the server binds).

```bash
# Runnable Proof (manual):
cargo build --release -p anvilml
ANVILML_PORT=9999 ./target/release/anvilml &
sleep 1
curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:9999/health
# -> 200
kill %1
```

---

## Phase 3 — Core Domain Types: Data Model

**Capability proved:** Not applicable — this phase adds only pure data types and an
in-memory registry to `anvilml-core`, with no new externally observable behaviour.
See `TASKS_PHASE003.md`'s Phase Acceptance Criteria for the full test-suite proof.

---

## Phase 4 — Hardware Detection: Detectors

**Capability proved:** Not applicable — this phase implements individual
`DeviceDetector` trait objects in isolation, with no orchestration function or HTTP
surface yet wiring them into an externally observable capability. See
`TASKS_PHASE004.md`'s Phase Acceptance Criteria for the full test-suite proof. The
live, externally observable hardware-detection capability is proved in Phase 5.

---

## Phase 5 — Hardware Detection: Orchestration

**Capability proved:** The `anvilml hw-probe` CLI subcommand calls the real
`detect_all_devices()` priority chain and prints valid `HardwareInfo` JSON,
including the guaranteed CPU fallback device and (when mock env vars are set) the
mock GPU device.

```bash
# Runnable Proof (manual):
cargo build --release -p anvilml --features mock-hardware
ANVILML_MOCK_DEVICE_TYPE=cuda ANVILML_MOCK_VRAM_MIB=24576 ./target/release/anvilml hw-probe \
  | python3 -c "import sys,json; d=json.load(sys.stdin); assert len(d['gpus'])>=2; assert any(g['device_type']=='cpu' for g in d['gpus']); assert any(g['device_type']=='cuda' for g in d['gpus'])"
# -> exits 0
```

---

## Phase 6 — Model Registry & Artifacts

**Capability proved:** Not applicable — this phase implements `anvilml-registry` and
`anvilml-artifacts` as persistence-layer crates with no HTTP handler or other
externally observable surface wired up yet. See `TASKS_PHASE006.md`'s Phase
Acceptance Criteria for the full test-suite proof.

---

## Phase 7 — IPC Foundations

**Capability proved:** Not applicable — this phase implements the IPC message
types and `RouterTransport`/`EventBroadcaster` wrappers in isolation, with no
worker subprocess yet spawned to communicate with. The 1000-round-trip stress test
that proves this subsystem end-to-end is Phase 8's explicit gate. See
`TASKS_PHASE007.md`'s Phase Acceptance Criteria for this phase's own test-suite
proof.

---

## Phase 8 — IPC Stress Gate & Worker Pool

**Capability proved:** The IPC transport survives 1000 sustained ROUTER/DEALER
round trips with zero message loss or reordering — the explicit gate named in
`ANVILML_DESIGN.md §20`'s IPC Baseline roadmap entry. The worker supervision layer
(spawn, demux, keepalive, respawn, pool) is complete and tested against mock IPC
backends, though it has no real Python subprocess to supervise yet — that
integration is Phase 9's scope.

```bash
cargo test -p anvilml-ipc --test stress_test --release
# -> exits 0, all 1000 round trips complete with zero loss
```

---

## Phase 9 — Real Worker Startup

**Capability proved:** A real Python `worker_main.py` subprocess, spawned by the
Rust worker pool, connects over ZeroMQ, runs a real torch-level capability probe on
a CPU device, and sends a `Ready` event with `capabilities_source: "pytorch"` — the
first genuine end-to-end real-mode execution of the worker startup path.

```bash
# Runnable Proof (manual):
cargo test -p anvilml-worker --test real_startup_tests -- --test-threads=1
# -> exits 0
```

---

## Phase 10 — Generic Node Groundwork

**Capability proved:** Not applicable — this phase builds the node system's base
contract (`BaseNode`, `@register`, `SlotSpec`, `NodeContext`) and the three
architecture-family dispatch packages with zero concrete nodes or arch modules
registered. See `TASKS_PHASE010.md`'s Phase Acceptance Criteria for the full
test-suite proof. The first externally observable change to `node_types` (moving
from empty to populated) occurs in the later "Dynamic Node System" phase.

---

## Phase 11 — Dynamic Node System

**Capability proved:** The running `anvilml` binary serves `GET /v1/nodes` over a
real HTTP request, backed by the dynamic `NodeTypeRegistry` that `ManagedWorker`
populates on every `Ready` event — though the registry is still empty at this point
in the project, since no worker is spawned by the normal server-start path yet.

```bash
# Runnable Proof (manual):
cargo build --release -p anvilml --features mock-hardware
./target/release/anvilml &
sleep 1
curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:8488/v1/nodes
# -> 200
curl -s http://127.0.0.1:8488/v1/nodes
# -> []
kill %1
```

---

## Phase 12 — Graph Validation

**Capability proved:** Not applicable — this phase implements `validate_graph()`
as a pure function with no HTTP handler or other externally observable surface
wired up yet. See `TASKS_PHASE012.md`'s Phase Acceptance Criteria for the full
test-suite proof. `POST /v1/jobs` (a later phase) is the eventual real consumer.

---

## Phase 13 — Job Queue

**Capability proved:** Not applicable — this phase implements in-memory queue/
ledger primitives and job persistence with no HTTP handler wired up yet. The
ghost-job reset now runs at every server startup, observable only via its INFO log
line. See `TASKS_PHASE013.md`'s Phase Acceptance Criteria for the full test-suite
proof. `POST /v1/jobs` and the dispatch loop (later phases) are the eventual real
consumers.

---

## Phase 14 — Dispatch & Execute

**Capability proved:** A job submitted via `POST /v1/jobs`, referencing the real
`PassThrough` node, is validated, queued, dispatched to a real spawned worker
subprocess via the documented worker-selection algorithm, executed, and reaches
`Completed` — the first genuine end-to-end real dispatch in the project, not a
mocked stand-in for any link in the chain.

```bash
# Runnable Proof (manual):
cargo build --release -p anvilml --features mock-hardware
./target/release/anvilml &
sleep 2
JOB_ID=$(curl -s -X POST http://127.0.0.1:8488/v1/jobs -H 'Content-Type: application/json' \
  -d '{"graph":{"nodes":[{"id":"n0","type":"PassThrough","inputs":{"value":1}}]},"settings":{}}' \
  | python3 -c "import sys,json;print(json.load(sys.stdin)['job_id'])")
sleep 3
curl -s "http://127.0.0.1:8488/v1/jobs/$JOB_ID" \
  | python3 -c "import sys,json; assert json.load(sys.stdin)['status']=='Completed'"
# -> exits 0
kill %1
```

---

## Phase 15 — Artifact Storage Wiring

**Capability proved:** `GET /v1/artifacts` and `GET /v1/artifacts/:hash` are live
over a real HTTP server, backed by Phase 6's `ArtifactStore`. The proof shows a
correct empty-list response, since `PassThrough` (the only node in the project at
this point) produces no image output — the first populated response requires a
real image-producing node chain, added in a later phase.

```bash
# Runnable Proof (manual):
cargo build --release -p anvilml --features mock-hardware
./target/release/anvilml &
sleep 1
curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:8488/v1/artifacts
# -> 200
curl -s http://127.0.0.1:8488/v1/artifacts
# -> []
kill %1
```

---

## Phase 16 — Live Events

**Capability proved:** A WebSocket client connected to `GET /v1/events` observes a
real `JobCompleted` event for a `PassThrough` job submitted via `POST /v1/jobs`,
delivered live as the job actually completes — not inferred by polling a REST
endpoint afterward.

```bash
# Runnable Proof (manual): a short Python script using the websockets library
# connects to ws://127.0.0.1:8488/v1/events, submits a PassThrough job via a
# parallel HTTP POST, and asserts a job_completed frame with the matching job_id
# arrives within 10 seconds.
# -> script exits 0
```

---

## Phase 17 — Cancellation

**Capability proved:** A `Queued` job's first `POST /v1/jobs/:id/cancel` call
returns `202`; a second cancel call on the same now-`Cancelled` job returns `409` —
demonstrating both the success path and idempotent-cancel rejection against a live
server.

```bash
# Runnable Proof (manual):
cargo build --release -p anvilml --features mock-hardware
./target/release/anvilml &
sleep 1
JOB_ID=$(curl -s -X POST http://127.0.0.1:8488/v1/jobs -H 'Content-Type: application/json' \
  -d '{"graph":{"nodes":[{"id":"n0","type":"PassThrough","inputs":{"value":1}}]},"settings":{}}' \
  | python3 -c "import sys,json;print(json.load(sys.stdin)['job_id'])")
curl -s -o /dev/null -w '%{http_code}' -X POST "http://127.0.0.1:8488/v1/jobs/$JOB_ID/cancel"
# -> 202
curl -s -o /dev/null -w '%{http_code}' -X POST "http://127.0.0.1:8488/v1/jobs/$JOB_ID/cancel"
# -> 409
kill %1
```

---

## Phase 18 — HTTP/WebSocket Server Completion

**Capability proved:** `GET /v1/system` and `GET /v1/workers` are both live over a
real HTTP server with real (mock-detected) data — the final two routes confirming
the complete REST surface from `ANVILML_DESIGN.md §13.4` is now backed entirely by
real, non-stub logic.

```bash
# Runnable Proof (manual):
cargo build --release -p anvilml --features mock-hardware
ANVILML_MOCK_DEVICE_TYPE=cuda ./target/release/anvilml &
sleep 2
curl -s http://127.0.0.1:8488/v1/system | python3 -c "import sys,json; d=json.load(sys.stdin); assert len(d['gpus'])>=1"
curl -s http://127.0.0.1:8488/v1/workers | python3 -c "import sys,json; assert isinstance(json.load(sys.stdin), list)"
# -> both exit 0
kill %1
```

---

## Phase 19 — Model Loading Contract Groundwork

**Capability proved:** Not applicable — this phase builds groundwork
infrastructure (model-hash resolution, the pipeline cache, loader node skeletons)
with no concrete arch module yet to exercise it end-to-end. See
`TASKS_PHASE019.md`'s Phase Acceptance Criteria for the full test-suite proof. The
first end-to-end real model load is Phase 20's scope.

---

## Phase 20 — ZiT Diffusion Arch Module: Shape Inference & Construction

**Capability proved:** The full real-mode model-loading chain — shape inference,
meta-device construction, dtype selection, key remapping, and weight loading —
succeeds end to end against a tiny synthetic ZiT-shaped fixture checkpoint, with
`LoadModel`'s real branch calling genuinely real code for the first time in the
project.

```bash
# Runnable Proof (manual):
python -m pytest worker/tests/test_arch_zit.py worker/tests/test_nodes_loader.py -v -m real_mode
# -> exits 0, zero skips, zero xfails
```

---

## Phase 21 — ZiT Diffusion Arch Module: Sampling & Latent Shape

**Capability proved:** The full real-mode sampling chain — pipeline assembly,
denoising, and seed resolution — succeeds end to end against the ZiT fixture
checkpoint, with the generic `Sampler` node's real branch dispatching correctly to
`zit.py`'s `sample()` from the same task that introduced it.

```bash
# Runnable Proof (manual):
python -m pytest worker/tests/test_arch_zit.py worker/tests/test_nodes_sampler.py -v -m real_mode
# -> exits 0, zero skips, zero xfails
```

---

## Phase 22 — Qwen3 CLIP Arch Module

**Capability proved:** The full real-mode text-encoder loading chain — shape
inference, meta-device construction, vendored tokenizer loading (zero network
calls), dtype selection, key remapping, and weight loading — succeeds end to end
against a tiny synthetic Qwen3-shaped fixture checkpoint, with `LoadClip`'s real
branch calling genuinely real code for the first time in the project.

```bash
# Runnable Proof (manual):
python -m pytest worker/tests/test_arch_clip_qwen3.py worker/tests/test_nodes_loader.py -v -m real_mode
# -> exits 0, zero skips, zero xfails
```

---

## Phase 23 — ZiT VAE Arch Module

**Capability proved:** The first genuinely complete real-mode generation chain in
the project — `LoadModel` → `Sampler` → `zit_vae.py`'s `decode()` — produces a real
`PIL.Image` with correct dimensions, chained directly against the respective
fixture checkpoints, ahead of the generic node layer being wired through it.

```bash
# Runnable Proof (manual):
python -m pytest worker/tests/test_e2e_zit_pipeline.py -v -m real_mode
# -> exits 0, asserts a real, non-mock PIL Image with correct dimensions is produced
```

---

## Phase 24 — Generic Conditioning/Sampling/Decode Nodes, Real Mode

**Capability proved:** The first end-to-end real (non-mock, non-`PassThrough`)
generation job in the project — a full ZiT/Qwen3/ZiT-VAE graph submitted via
`POST /v1/jobs`, dispatched through the real pipeline, producing a real, retrievable
PNG artifact matching the requested dimensions. This closes "ZiT Diffusion + Qwen3
CLIP + ZiT VAE" as a fully completed roadmap group.

```bash
# Runnable Proof (manual): full submit -> poll -> retrieve sequence against a live
# server in real mode, using the Appendix B.2 example graph with fixture model_id
# values, producing a retrievable, valid PNG artifact.
```

---

## Phase 25 — Flux 2 Klein 4B Diffusion + Flux 2 VAE

**Capability proved:** A Flux 2 Klein 4B + Flux 2 VAE generation graph, submitted
through the exact same generic-node `POST /v1/jobs` pipeline Phase 24 used for ZiT,
produces a real, retrievable PNG artifact — confirming the generic node layer is
genuinely architecture-agnostic, with zero changes needed to add this second
diffusion architecture.

```bash
# Runnable Proof (manual): full submit -> poll -> retrieve sequence in real mode,
# using the Appendix B.2 graph with Flux 2 Klein 4B / Flux 2 VAE fixture model_id
# values, producing a retrievable, valid PNG matching the requested dimensions.
```

---

## Phase 26 — Flux 2 Klein 9B + Qwen3-8B CLIP Variant

**Capability proved:** A Flux 2 Klein 9B + Qwen3-8B (FP8-mixed) generation graph
produces a real, retrievable PNG artifact through the exact same generic-node
pipeline used for every prior architecture — confirming `flux2klein.py` and
`qwen3.py` serve two model sizes each via shape inference alone, with no second
file and no size-specific branching. This closes the full MVP model matrix from
`ANVILML_DESIGN.md §2.3`.

```bash
# Runnable Proof (manual): full submit -> poll -> retrieve sequence in real mode
# using the 9B/8B graph, producing a retrievable, valid PNG.
```

---

## Phase 27 — End-to-End Validation

**Capability proved:** Not applicable in the automated sense — this phase's actual
validation is explicitly manual, real-GPU-only, and excluded from CI per
`ANVILML_DESIGN.md §2.2`. `docs/E2E_VALIDATION.md` gives the project owner an exact
checklist to run by hand on their own hardware, covering all three rows of the MVP
model matrix. This phase's automated deliverable is the CI audit (`P27-B1`)
confirming no existing job accidentally requires real GPU hardware.

---

## Phase 28 — Distribution

**Capability proved:** A fresh clone with no Python venv present auto-provisions
one at startup without crashing, and `anvilml --version` subsequently reports
accurate, real component versions (Rust, Python, torch) — the full distribution
story, end to end, on a deliberately degraded starting environment.

```bash
# Runnable Proof (manual):
rm -rf worker/.venv
cargo build --release -p anvilml
timeout 120 ./target/release/anvilml &
sleep 90
kill %1
./target/release/anvilml --version
# -> shows a real (non-None) python_version and torch_version
```

---

## Phase 29 — Documentation

**Capability proved:** A complete, seven-chapter mdBook documentation site builds
cleanly with no broken internal links, with every chapter's content sourced from
exactly one authoritative project source (no independently-drifting copies), and a
new additive CI job confirms this on every push.

```bash
mdbook build docs/book
# -> exits 0, no broken internal links
```

---

## Phase 30 — v4 Roadmap Closeout: Final Compliance Sweep

**Capability proved:** The complete v4 delivery — all 29 prior phases combined —
passes every project-wide compliance check this project defines: the stub/marker
sweeps report their findings (zero, or fixed), the full standard gate suite and
both platform cross-checks pass against the final repository state, and the
delivery's own phase registry and runnable-proof log are confirmed internally
consistent.

```bash
# See P30-D1 for the complete final gate sequence — every command exits 0.
```
