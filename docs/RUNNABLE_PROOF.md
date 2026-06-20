# RUNNABLE_PROOF.md — Project-Wide Runnable Proof Index

**Document:** `docs/RUNNABLE_PROOF.md`
**Spec authority:** `docs/FORGE_TASK_AUTHORING_SPEC.md §9a`
**Phase registry:** `docs/PHASES.md`

This document indexes every phase's Runnable Proof — the command sequence that
demonstrates the phase's new capability against a *live running instance*
(a bound server, a real subprocess, a real file on disk), as distinct from the
standard `cargo test` / `pytest` / `clippy` / Windows cross-check gates that
every phase also requires. Those standard gates are **not** reproduced here —
see `TASKS_PHASE<NNN>.md`'s own "Phase Acceptance Criteria" section for the
full gate list. This document is a fast, low-noise reference for "how do I
manually verify phase N actually works."

`TASKS_PHASE<NNN>.md` is always the source of truth. If this document and a
phase's `TASKS_PHASE<NNN>.md` ever disagree, `TASKS_PHASE<NNN>.md` wins and
this document is stale and must be corrected to match.

Phases that are legitimately exempt from a live-instance proof (per the
narrow exemption in `FORGE_TASK_AUTHORING_SPEC.md §9`) are listed with their
`not applicable` reason rather than omitted, so this index is always complete.

---

## Phase 000 — Repository Preamble

**Runnable Proof:** not applicable — pure repository scaffolding; nothing is
running yet, so there is no live instance to exercise.

---

## Phase 001 — Walking Skeleton

Capability: the axum server binds and serves `GET /health`.

```bash
cargo run --features mock-hardware &
sleep 2
curl -s http://127.0.0.1:8488/health | python3 -c "import sys,json; d=json.load(sys.stdin); assert d['status']=='ok'"
kill %1
```

---

## Phase 002 — Config & Graceful Shutdown

Capability: `--port` CLI override takes effect, and SIGTERM produces a clean exit.

```bash
cargo run --features mock-hardware -- --port 9001 &
sleep 2
curl -s http://127.0.0.1:9001/health | python3 -c "import sys,json; d=json.load(sys.stdin); assert d['status']=='ok'"
kill -SIGTERM %1
wait %1
echo "Exit code: $?"
```

---

## Phase 003 — Core Domain Types

Capability: `GET /v1/system/env` returns a stub `EnvReport`.

```bash
cargo run --features mock-hardware &
sleep 2
curl -s http://127.0.0.1:8488/v1/system/env | python3 -c "import sys,json; d=json.load(sys.stdin); assert 'preflight_ok' in d"
kill %1
```

---

## Phase 004 — Hardware Detection

Capability: `GET /v1/system` returns real (or mock-injected) `HardwareInfo` with at least one device.

```bash
ANVILML_MOCK_DEVICE_TYPE=cuda cargo run --features mock-hardware &
sleep 2
curl -s http://127.0.0.1:8488/v1/system | python3 -c "import sys,json; d=json.load(sys.stdin); assert len(d['gpus'])>=1"
kill %1
```

---

## Phase 005 — SQLite Persistence

Capability: the database file is created on first run, and ghost-job reset is visible in the startup log.

```bash
cargo run --features mock-hardware &
sleep 2
ls anvilml.db
grep -q "database" /dev/stdin <<< "$(cargo run --features mock-hardware 2>&1 | head -20)"
kill %1
```

---

## Phase 006 — Model Registry

Capability: a `.safetensors` file dropped into `models/diffusion/` is discovered by the scanner and listed via `GET /v1/models`.

```bash
mkdir -p models/diffusion models/text_encoders models/vae
dd if=/dev/urandom of=models/diffusion/test_model_fp8.safetensors bs=1M count=2
cargo run --features mock-hardware &
sleep 2
curl -s http://127.0.0.1:8488/v1/models | python3 -c "import sys,json; items=json.load(sys.stdin); assert len(items)>=1"
kill %1
```

---

## Phase 007 — WebSocket Event Stream

Capability: `GET /v1/events` upgrades to WebSocket and emits `system_stats` frames every 5 seconds.

```bash
cargo run --features mock-hardware &
sleep 2
timeout 12 websocat ws://127.0.0.1:8488/v1/events | head -3 | python3 -c "import sys,json; [json.loads(l) for l in sys.stdin]"
kill %1
```

---

## Phase 008 — ZeroMQ IPC Transport

Capability: the ROUTER/DEALER transport survives 1000 round trips with zero errors. This is the highest-risk phase in the project — no subsequent phase begins until this passes on both Linux and the Windows cross-check target. The stress test itself, not a separate manual step, is the proof.

```bash
cargo test -p anvilml-ipc --features mock-hardware --test stress_test
# -> log line "stress test passed: 1000/1000"; all 1000 round trips complete within 30s
```

---

## Phase 009 — Worker Spawn & Handshake

Capability: a mock Python worker is spawned, completes the IPC handshake, and reaches `Idle`.

```bash
cargo run --features mock-hardware &
sleep 30
curl -s http://127.0.0.1:8488/v1/workers | python3 -c "import sys,json; workers=json.load(sys.stdin); assert any(w['status']=='Idle' for w in workers)"
kill %1
```

---

## Phase 010 — Worker Crash Recovery

Capability: killing the worker process triggers automatic respawn back to `Idle`; a manual restart endpoint also works.

```bash
# Manual smoke test with a running server:
# Kill a Python worker in the OS task manager → observe respawn via GET /v1/workers
# curl -X POST http://localhost:8488/v1/workers/worker-0/restart → HTTP 202
```

---

## Phase 011 — Dynamic Node Registry

Capability: `GET /v1/nodes` reflects registry state — 503 before any worker reaches `Ready`, 200 with a JSON array afterward.

```bash
cargo run --features mock-hardware &
sleep 1
curl -s -o /dev/null -w "%{http_code}" http://127.0.0.1:8488/v1/nodes
# -> 503 (no worker has reached Ready yet)
sleep 5
curl -s http://127.0.0.1:8488/v1/nodes | python3 -c "import sys,json; d=json.load(sys.stdin); assert isinstance(d, list)"
# -> 200 with a JSON array of registered NodeTypeDescriptor entries
kill %1
```

---

## Phase 012 — Graph Validation

Capability: `POST /v1/jobs` rejects an unknown node type with 422, and accepts a structurally valid graph with 202.

```bash
cargo run --features mock-hardware &
sleep 5
curl -s -X POST http://127.0.0.1:8488/v1/jobs -H 'Content-Type: application/json' \
  -d '{"graph":{"nodes":[{"id":"n1","type":"GhostNode"}]},"settings":{}}' \
  | python3 -c "import sys,json; d=json.load(sys.stdin); assert d['error']=='invalid_graph'"
# -> 422 {"error":"invalid_graph",...} (unknown node type)
curl -s -o /dev/null -w "%{http_code}" -X POST http://127.0.0.1:8488/v1/jobs -H 'Content-Type: application/json' \
  -d '{"graph":{"nodes":[]},"settings":{}}'
# -> 202 (an empty-nodes graph is structurally valid)
kill %1
```

---

## Phase 013 — Job Queue & Persistence

Capability: a submitted job is persisted to SQLite and retrievable by id with status `Queued`.

```bash
cargo run --features mock-hardware &
sleep 5
JOB_ID=$(curl -s -X POST http://127.0.0.1:8488/v1/jobs -H 'Content-Type: application/json' \
  -d '{"graph":{"nodes":[]},"settings":{}}' | python3 -c "import sys,json; print(json.load(sys.stdin)['job_id'])")
curl -s "http://127.0.0.1:8488/v1/jobs/$JOB_ID" | python3 -c "import sys,json; d=json.load(sys.stdin); assert d['status']=='Queued'"
# -> 200 {"status":"Queued",...} (job persisted in SQLite, retrievable by id)
kill %1
```

---

## Phase 014 — Dispatch & Mock Execute

Capability: a submitted mock job is dispatched to a worker and reaches `Completed`.

```bash
cargo run --features mock-hardware &
sleep 30
JOB_ID=$(curl -s -X POST http://127.0.0.1:8488/v1/jobs -H 'Content-Type: application/json' \
  -d '{"graph":{"nodes":[]},"settings":{}}' | python3 -c "import sys,json; print(json.load(sys.stdin)['job_id'])")
for i in $(seq 1 10); do
  STATUS=$(curl -s "http://127.0.0.1:8488/v1/jobs/$JOB_ID" | python3 -c "import sys,json; print(json.load(sys.stdin)['status'])")
  [ "$STATUS" = "Completed" ] && break
  sleep 1
done
[ "$STATUS" = "Completed" ]
# -> loop exits with $STATUS == "Completed" within 10s of dispatch
kill %1
```

---

## Phase 015 — Artifact Storage

Capability: a `SaveImage` node's output is stored content-addressed and retrievable as `image/png` via HTTP.

```bash
cargo run --features mock-hardware &
sleep 30
JOB_ID=$(curl -s -X POST http://127.0.0.1:8488/v1/jobs -H 'Content-Type: application/json' \
  -d '{"graph":{"nodes":[{"id":"n1","type":"SaveImage"}]},"settings":{}}' | python3 -c "import sys,json; print(json.load(sys.stdin)['job_id'])")
sleep 3
HASH=$(curl -s "http://127.0.0.1:8488/v1/artifacts?job_id=$JOB_ID" | python3 -c "import sys,json; print(json.load(sys.stdin)[0]['hash'])")
curl -s -o /dev/null -w "%{content_type}" "http://127.0.0.1:8488/v1/artifacts/$HASH"
# -> image/png (non-empty body; PNG bytes retrievable by hash)
kill %1
```

---

## Phase 016 — Live Job Events

Capability: the full job lifecycle (`JobQueued` → ... → `JobImageReady` → `JobCompleted`) is observable in order over the WebSocket event stream.

```bash
cargo run --features mock-hardware &
sleep 30
( timeout 10 websocat ws://127.0.0.1:8488/v1/events > /tmp/events.log & )
sleep 1
curl -s -X POST http://127.0.0.1:8488/v1/jobs -H 'Content-Type: application/json' \
  -d '{"graph":{"nodes":[{"id":"n1","type":"SaveImage"}]},"settings":{}}' > /dev/null
sleep 9
grep -q "JobQueued" /tmp/events.log && grep -q "JobCompleted" /tmp/events.log
# -> /tmp/events.log contains JobQueued ... JobImageReady ... JobCompleted in order
kill %1
```

---

## Phase 017 — Cancellation

Capability: a job can be cancelled via `POST /v1/jobs/:id/cancel` and reaches a terminal state.

```bash
cargo run --features mock-hardware &
sleep 30
JOB_ID=$(curl -s -X POST http://127.0.0.1:8488/v1/jobs -H 'Content-Type: application/json' \
  -d '{"graph":{"nodes":[]},"settings":{}}' | python3 -c "import sys,json; print(json.load(sys.stdin)['job_id'])")
curl -s -o /dev/null -w "%{http_code}" -X POST "http://127.0.0.1:8488/v1/jobs/$JOB_ID/cancel"
# -> 202 (cancel accepted, whether job was still Queued or already Running)
sleep 1
curl -s "http://127.0.0.1:8488/v1/jobs/$JOB_ID" | python3 -c "import sys,json; d=json.load(sys.stdin); assert d['status'] in ('Cancelled','Completed')"
# -> status is a terminal state (Cancelled, or Completed if the race favored execution)
kill %1
```

---

## Phase 018 — ZiT Generic Nodes

Capability: a real ZiT FP8 workflow, submitted to a real-hardware build, produces a PNG artifact. Requires ZiT FP8 safetensors in `models/` — not runnable in CI.

```bash
# Real hardware proof (manual, requires ZiT FP8 safetensors in models/):
cargo run --features real-hardware
curl -X POST http://127.0.0.1:8488/v1/jobs -H 'Content-Type: application/json' -d @docs/example_workflows/zit_fp8.json
# poll /v1/jobs/:id until Completed; curl /v1/artifacts/:hash -> image/png
```

Full documented command sequence and expected output: `docs/PROOF_phase018.md`.

---

## Phase 019 — Flux 2 Klein Nodes

Capability: a real Flux 2 Klein FP8 workflow, submitted to a real-hardware build, produces a PNG artifact. Requires Flux 2 Klein FP8 safetensors in `models/` — not runnable in CI.

```bash
# Real hardware proof (manual, requires Flux 2 Klein FP8 safetensors in models/):
# Submit flux_klein_fp8.json; verify Completed + PNG artifact
```

Full documented command sequence and expected output: `docs/PROOF_phase019.md`.

---

## Phase 020 — End-to-End Validation

Capability: the OpenAPI spec is regenerated and matches the committed file with zero drift; on real hardware, both model families produce a real PNG.

```bash
cargo run -p anvilml-openapi && git diff --exit-code api/openapi.json
# -> regenerated spec is byte-identical to the committed api/openapi.json
# On target hardware: ZiT and Flux workflows (Phases 018-019) each still produce a real PNG.
```

---

## Phase 021 — Auto-Provisioning & Version Introspection

Capability: `GET /v1/system/versions` reports non-empty Rust, Python, torch, and IPC protocol versions on a live server.

```bash
cargo run --features mock-hardware &
sleep 5
curl -s http://127.0.0.1:8488/v1/system/versions | python3 -c "import sys,json; d=json.load(sys.stdin); assert all(d[k] for k in ('anvilml','python','torch','worker_protocol'))"
# -> 200 with anvilml, python, torch, worker_protocol all non-empty
kill %1
```

---

## Phase 022 — Release Packaging

Capability: a release archive is produced and its checksums verify.

```bash
cargo build --release
# -> release zip produced; sha256sum --check SHA256SUMS exits 0
```

---

## Phase 023 — Documentation Site

Capability: the mdBook documentation site builds and all internal links resolve. For a docs-only phase, the build command itself is the full proof.

```bash
mdbook build docs-site
mdbook test docs-site
# -> exits 0; no broken internal links reported
```

---

## Phase 900 — CLI and Config Test Retrofit

Capability: the Windows port-detection fix is correct on both CI runners, and the env-var test race is eliminated under repeated runs. Test-only retrofit; its own proof is a repeated-run gate rather than a live-server interaction.

```bash
cargo test -p anvilml --features mock-hardware --test cli_tests
for i in $(seq 1 50); do
  cargo test -p anvilml-core --test config_load_tests || exit 1
done
# -> both commands exit 0; the 50-run loop has zero failures
```

---

## Phase 901 — ManagedWorker Run-Loop and RespawnPolicy Retrofit

**Runnable Proof:** not applicable — pure internal correctness fix to `run()`'s
event loop and `should_respawn`'s reset logic, with no new `pub` API and no
new HTTP-, WebSocket-, or CLI-observable behaviour. Coverage is via the unit
and integration test suite only (see `TASKS_PHASE901.md`'s Phase Acceptance
Criteria for the test commands).

---

## Phase 902 — ArtifactStore Relocation Retrofit

**Runnable Proof:** not applicable — pure internal refactor relocating
`ArtifactStore` to its own crate with no change to its struct shape, method
signatures, or behaviour, and no change to any HTTP-, WebSocket-, or
CLI-observable surface. Coverage is via the existing test suite, which must
continue to pass unchanged after every call site is repointed.
