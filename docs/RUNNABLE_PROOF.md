# AnvilML v4 — Runnable Proof Log

This file aggregates the Runnable Proof for each phase, in execution order. Each
entry's commands are reproduced verbatim from that phase's `TASKS_PHASE<NNN>.md`.
This delivery covers Phases 1-30 — the complete v4 roadmap — plus Phase 900, a
retrofit phase inserted between Phases 6 and 7.

Every phase with a live-instance proof gives both a bash block (Linux/macOS) and a
PowerShell block (Windows) — the two exercise the identical capability, adapted to
each shell's idioms (background process + kill, `curl`/`Invoke-WebRequest`, env var
syntax). Phases that qualify for the narrow exemption in
`FORGE_TASK_AUTHORING_SPEC.md §9` show the `not applicable` line instead, with no
shell-specific variant needed since there is nothing to run.

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

```powershell
# Runnable Proof (manual, PowerShell):
cargo build --release -p anvilml
$proc = Start-Process -FilePath .\target\release\anvilml.exe -PassThru
Start-Sleep -Seconds 1
(Invoke-WebRequest -Uri http://127.0.0.1:8488/health -UseBasicParsing).StatusCode
# -> 200
Stop-Process -Id $proc.Id
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

```powershell
# Runnable Proof (manual, PowerShell):
cargo build --release -p anvilml
$env:ANVILML_PORT = "9999"
$proc = Start-Process -FilePath .\target\release\anvilml.exe -PassThru
Start-Sleep -Seconds 1
(Invoke-WebRequest -Uri http://127.0.0.1:9999/health -UseBasicParsing).StatusCode
# -> 200
Stop-Process -Id $proc.Id
Remove-Item Env:\ANVILML_PORT
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

```powershell
# Runnable Proof (manual, PowerShell):
cargo build --release -p anvilml --features mock-hardware
$env:ANVILML_MOCK_DEVICE_TYPE = "cuda"
$env:ANVILML_MOCK_VRAM_MIB = "24576"
$json = .\target\release\anvilml.exe hw-probe | Out-String
$d = $json | ConvertFrom-Json
if ($d.gpus.Count -lt 2) { throw "expected >=2 gpus" }
if (-not ($d.gpus | Where-Object { $_.device_type -eq "cpu" })) { throw "missing cpu device" }
if (-not ($d.gpus | Where-Object { $_.device_type -eq "cuda" })) { throw "missing cuda device" }
# -> no exception thrown
Remove-Item Env:\ANVILML_MOCK_DEVICE_TYPE, Env:\ANVILML_MOCK_VRAM_MIB
```

---

## Phase 6 — Model Registry & Artifacts

**Capability proved:** `database/migrations/001_initial.sql` (the `models` and
`device_capabilities` tables) and `database/seeds/devices.sql` (the one-time
`SUPPORTED_DEVICES_DB.md` conversion) are both real, loadable SQL files — applying
either to a live SQLite instance succeeds and produces real, queryable tables and
rows. Per `FORGE_TASK_AUTHORING_SPEC.md §9`'s exemption clause (c), the load
command itself is the full proof for this kind of file-on-disk artifact; no HTTP
handler exists yet to wrap it (`/v1/models` and `/v1/artifacts` are Phase 15/18's
scope).

```bash
# Runnable Proof (manual):
sqlite3 anvilml_proof.db < database/migrations/001_initial.sql
sqlite3 anvilml_proof.db < database/seeds/devices.sql
sqlite3 anvilml_proof.db "SELECT count(*) FROM device_capabilities;"
# -> a non-zero row count, confirming the seed data actually loaded
sqlite3 anvilml_proof.db ".schema models"
# -> shows the models table with id/name/path/kind/dtype/format/size_bytes/mtime_unix/scanned_at
rm anvilml_proof.db
```

```powershell
# Runnable Proof (manual, PowerShell):
Get-Content database\migrations\001_initial.sql | sqlite3 anvilml_proof.db
Get-Content database\seeds\devices.sql | sqlite3 anvilml_proof.db
sqlite3 anvilml_proof.db "SELECT count(*) FROM device_capabilities;"
# -> a non-zero row count, confirming the seed data actually loaded
sqlite3 anvilml_proof.db ".schema models"
# -> shows the models table with id/name/path/kind/dtype/format/size_bytes/mtime_unix/scanned_at
Remove-Item anvilml_proof.db
```

---

## Phase 7 — IPC Foundations

**Capability proved:** `RouterTransport::bind()` opens a real ZeroMQ ROUTER socket
on a real, OS-assigned TCP port — observable from outside the test process via a
live port scan — even though no worker subprocess exists yet to complete a round
trip with it (the full 1000-round-trip stress proof is Phase 8's explicit gate).

```bash
# Runnable Proof (manual):
cat > /tmp/anvilml_bind_proof.rs <<'EOF'
use anvilml_ipc::RouterTransport;
#[tokio::main]
async fn main() {
    let t = RouterTransport::bind().await.expect("bind failed");
    println!("{}", t.port);
}
EOF
cargo run -q --manifest-path crates/anvilml-ipc/Cargo.toml --example anvilml_bind_proof 2>/dev/null \
  || (mkdir -p crates/anvilml-ipc/examples && cp /tmp/anvilml_bind_proof.rs crates/anvilml-ipc/examples/ \
      && PORT=$(cargo run -q -p anvilml-ipc --example anvilml_bind_proof) \
      && ss -ltn | grep -q ":$PORT " && echo "bound and listening on $PORT")
# -> prints a real port number, and ss confirms a live LISTEN socket on it
```

```powershell
# Runnable Proof (manual, PowerShell):
$proofSrc = @'
use anvilml_ipc::RouterTransport;
#[tokio::main]
async fn main() {
    let t = RouterTransport::bind().await.expect("bind failed");
    println!("{}", t.port);
}
'@
New-Item -ItemType Directory -Force -Path crates\anvilml-ipc\examples | Out-Null
Set-Content -Path crates\anvilml-ipc\examples\anvilml_bind_proof.rs -Value $proofSrc
$portOut = cargo run -q -p anvilml-ipc --example anvilml_bind_proof
$listening = Get-NetTCPConnection -State Listen | Where-Object { $_.LocalPort -eq [int]$portOut }
if (-not $listening) { throw "no listener found on port $portOut" }
# -> $portOut prints a real port number, and Get-NetTCPConnection confirms LISTEN state
```

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

```powershell
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

```powershell
# Runnable Proof (manual, PowerShell):
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

```powershell
# Runnable Proof (manual, PowerShell):
cargo build --release -p anvilml --features mock-hardware
$proc = Start-Process -FilePath .\target\release\anvilml.exe -PassThru
Start-Sleep -Seconds 1
(Invoke-WebRequest -Uri http://127.0.0.1:8488/v1/nodes -UseBasicParsing).StatusCode
# -> 200
(Invoke-WebRequest -Uri http://127.0.0.1:8488/v1/nodes -UseBasicParsing).Content
# -> []
Stop-Process -Id $proc.Id
```

---

## Phase 12 — Graph Validation

**Capability proved:** Not applicable — this phase implements `validate_graph()`
as a pure function with no HTTP handler or other externally observable surface
wired up yet. See `TASKS_PHASE012.md`'s Phase Acceptance Criteria for the full
test-suite proof. `POST /v1/jobs` (a later phase) is the eventual real consumer.

---

## Phase 13 — Job Queue

**Capability proved:** `reset_ghost_jobs()` runs against a real SQLite database at
every server startup: a job left in `Running` status by a prior, uncleanly-terminated
process is observably reset before the server accepts its first request, with the
reset count logged at INFO level — a real, externally observable startup-time
behaviour change, not merely an in-memory queue primitive.

```bash
# Runnable Proof (manual):
cargo build --release -p anvilml --features mock-hardware
rm -f anvilml_proof.db
sqlite3 anvilml_proof.db < database/migrations/001_initial.sql
sqlite3 anvilml_proof.db < database/migrations/003_jobs.sql
sqlite3 anvilml_proof.db "INSERT INTO jobs (id, status, graph, settings, created_at) VALUES ('11111111-1111-1111-1111-111111111111', 'running', '{}', '{}', '2026-01-01T00:00:00Z');"
ANVILML_DB_PATH=./anvilml_proof.db RUST_LOG=info ./target/release/anvilml > /tmp/anvilml_proof.log 2>&1 &
sleep 1
grep -i "ghost" /tmp/anvilml_proof.log
# -> an INFO log line reporting 1 ghost job reset
sqlite3 anvilml_proof.db "SELECT status FROM jobs WHERE id='11111111-1111-1111-1111-111111111111';"
# -> failed
kill %1
rm -f anvilml_proof.db /tmp/anvilml_proof.log
```

```powershell
# Runnable Proof (manual, PowerShell):
cargo build --release -p anvilml --features mock-hardware
Remove-Item -ErrorAction SilentlyContinue anvilml_proof.db
Get-Content database\migrations\001_initial.sql | sqlite3 anvilml_proof.db
Get-Content database\migrations\003_jobs.sql | sqlite3 anvilml_proof.db
sqlite3 anvilml_proof.db "INSERT INTO jobs (id, status, graph, settings, created_at) VALUES ('11111111-1111-1111-1111-111111111111', 'running', '{}', '{}', '2026-01-01T00:00:00Z');"
$env:ANVILML_DB_PATH = ".\anvilml_proof.db"
$env:RUST_LOG = "info"
$proc = Start-Process -FilePath .\target\release\anvilml.exe -PassThru -RedirectStandardOutput proof.log -RedirectStandardError proof_err.log
Start-Sleep -Seconds 1
Select-String -Path proof.log,proof_err.log -Pattern "ghost"
# -> an INFO log line reporting 1 ghost job reset
sqlite3 anvilml_proof.db "SELECT status FROM jobs WHERE id='11111111-1111-1111-1111-111111111111';"
# -> failed
Stop-Process -Id $proc.Id
Remove-Item anvilml_proof.db, proof.log, proof_err.log
Remove-Item Env:\ANVILML_DB_PATH, Env:\RUST_LOG
```

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

```powershell
# Runnable Proof (manual, PowerShell):
cargo build --release -p anvilml --features mock-hardware
$proc = Start-Process -FilePath .\target\release\anvilml.exe -PassThru
Start-Sleep -Seconds 2
$body = '{"graph":{"nodes":[{"id":"n0","type":"PassThrough","inputs":{"value":1}}]},"settings":{}}'
$resp = Invoke-WebRequest -Uri http://127.0.0.1:8488/v1/jobs -Method Post -Body $body -ContentType 'application/json' -UseBasicParsing
$jobId = ($resp.Content | ConvertFrom-Json).job_id
Start-Sleep -Seconds 3
$job = Invoke-WebRequest -Uri "http://127.0.0.1:8488/v1/jobs/$jobId" -UseBasicParsing
if ((($job.Content | ConvertFrom-Json).status) -ne "Completed") { throw "job did not complete" }
# -> no exception thrown
Stop-Process -Id $proc.Id
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

```powershell
# Runnable Proof (manual, PowerShell):
cargo build --release -p anvilml --features mock-hardware
$proc = Start-Process -FilePath .\target\release\anvilml.exe -PassThru
Start-Sleep -Seconds 1
(Invoke-WebRequest -Uri http://127.0.0.1:8488/v1/artifacts -UseBasicParsing).StatusCode
# -> 200
(Invoke-WebRequest -Uri http://127.0.0.1:8488/v1/artifacts -UseBasicParsing).Content
# -> []
Stop-Process -Id $proc.Id
```

---

## Phase 16 — Live Events

**Capability proved:** A WebSocket client connected to `GET /v1/events` observes a
real `JobCompleted` event for a `PassThrough` job submitted via `POST /v1/jobs`,
delivered live as the job actually completes — not inferred by polling a REST
endpoint afterward.

```bash
# Runnable Proof (manual):
cargo build --release -p anvilml --features mock-hardware
./target/release/anvilml &
sleep 1
python3 - <<'EOF'
import asyncio, json, urllib.request
import websockets

async def main():
    async with websockets.connect("ws://127.0.0.1:8488/v1/events") as ws:
        await ws.recv()  # initial SystemStats frame
        req = urllib.request.Request(
            "http://127.0.0.1:8488/v1/jobs",
            data=json.dumps({
                "graph": {"nodes": [{"id": "n0", "type": "PassThrough", "inputs": {"value": 1}}]},
                "settings": {}
            }).encode(),
            headers={"Content-Type": "application/json"},
        )
        job_id = json.loads(urllib.request.urlopen(req).read())["job_id"]
        async with asyncio.timeout(10):
            while True:
                frame = json.loads(await ws.recv())
                if frame.get("type") == "job_completed" and frame.get("job_id") == job_id:
                    return

asyncio.run(main())
EOF
# -> script exits 0; a job_completed frame with the matching job_id arrived within 10s
kill %1
```

```powershell
# Runnable Proof (manual, PowerShell):
cargo build --release -p anvilml --features mock-hardware
$proc = Start-Process -FilePath .\target\release\anvilml.exe -PassThru
Start-Sleep -Seconds 1
$proofScript = @'
import asyncio, json, urllib.request
import websockets

async def main():
    async with websockets.connect("ws://127.0.0.1:8488/v1/events") as ws:
        await ws.recv()  # initial SystemStats frame
        req = urllib.request.Request(
            "http://127.0.0.1:8488/v1/jobs",
            data=json.dumps({
                "graph": {"nodes": [{"id": "n0", "type": "PassThrough", "inputs": {"value": 1}}]},
                "settings": {}
            }).encode(),
            headers={"Content-Type": "application/json"},
        )
        job_id = json.loads(urllib.request.urlopen(req).read())["job_id"]
        async with asyncio.timeout(10):
            while True:
                frame = json.loads(await ws.recv())
                if frame.get("type") == "job_completed" and frame.get("job_id") == job_id:
                    return

asyncio.run(main())
'@
Set-Content -Path proof_events.py -Value $proofScript
python proof_events.py
# -> script exits 0; a job_completed frame with the matching job_id arrived within 10s
Stop-Process -Id $proc.Id
Remove-Item proof_events.py
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

```powershell
# Runnable Proof (manual, PowerShell):
cargo build --release -p anvilml --features mock-hardware
$proc = Start-Process -FilePath .\target\release\anvilml.exe -PassThru
Start-Sleep -Seconds 1
$body = '{"graph":{"nodes":[{"id":"n0","type":"PassThrough","inputs":{"value":1}}]},"settings":{}}'
$resp = Invoke-WebRequest -Uri http://127.0.0.1:8488/v1/jobs -Method Post -Body $body -ContentType 'application/json' -UseBasicParsing
$jobId = ($resp.Content | ConvertFrom-Json).job_id
(Invoke-WebRequest -Uri "http://127.0.0.1:8488/v1/jobs/$jobId/cancel" -Method Post -UseBasicParsing).StatusCode
# -> 202
try {
    (Invoke-WebRequest -Uri "http://127.0.0.1:8488/v1/jobs/$jobId/cancel" -Method Post -UseBasicParsing).StatusCode
} catch {
    $_.Exception.Response.StatusCode.value__
}
# -> 409
Stop-Process -Id $proc.Id
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

```powershell
# Runnable Proof (manual, PowerShell):
cargo build --release -p anvilml --features mock-hardware
$env:ANVILML_MOCK_DEVICE_TYPE = "cuda"
$proc = Start-Process -FilePath .\target\release\anvilml.exe -PassThru
Start-Sleep -Seconds 2
$system = (Invoke-WebRequest -Uri http://127.0.0.1:8488/v1/system -UseBasicParsing).Content | ConvertFrom-Json
if ($system.gpus.Count -lt 1) { throw "expected >=1 gpu" }
$workers = (Invoke-WebRequest -Uri http://127.0.0.1:8488/v1/workers -UseBasicParsing).Content | ConvertFrom-Json
if ($workers -isnot [System.Array]) { throw "expected a JSON array" }
# -> no exception thrown
Stop-Process -Id $proc.Id
Remove-Item Env:\ANVILML_MOCK_DEVICE_TYPE
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

```powershell
# Runnable Proof (manual, PowerShell):
worker\.venv\Scripts\python -m pytest worker/tests/test_arch_zit.py worker/tests/test_nodes_loader.py -v -m real_mode
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

```powershell
# Runnable Proof (manual, PowerShell):
worker\.venv\Scripts\python -m pytest worker/tests/test_arch_zit.py worker/tests/test_nodes_sampler.py -v -m real_mode
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

```powershell
# Runnable Proof (manual, PowerShell):
worker\.venv\Scripts\python -m pytest worker/tests/test_arch_clip_qwen3.py worker/tests/test_nodes_loader.py -v -m real_mode
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

```powershell
# Runnable Proof (manual, PowerShell):
worker\.venv\Scripts\python -m pytest worker/tests/test_e2e_zit_pipeline.py -v -m real_mode
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
# Runnable Proof (manual): real (non-mock-hardware) mode, fixture checkpoints
# already registered in the model registry under their SHA256 ids.
cargo build --release -p anvilml
./target/release/anvilml &
sleep 2
ZIT_ID=$(sha256sum worker/tests/fixtures/zit_tiny.safetensors | head -c1048576 | cut -d' ' -f1)
VAE_ID=$(sha256sum worker/tests/fixtures/zit_vae_tiny.safetensors | head -c1048576 | cut -d' ' -f1)
CLIP_ID=$(sha256sum worker/tests/fixtures/qwen3_tiny.safetensors | head -c1048576 | cut -d' ' -f1)
JOB_ID=$(curl -s -X POST http://127.0.0.1:8488/v1/jobs -H 'Content-Type: application/json' \
  -d "{\"graph\":{\"nodes\":[
    {\"id\":\"model\",\"type\":\"LoadModel\",\"inputs\":{\"model_id\":\"$ZIT_ID\"}},
    {\"id\":\"vae\",\"type\":\"LoadVae\",\"inputs\":{\"model_id\":\"$VAE_ID\"}},
    {\"id\":\"encoder\",\"type\":\"LoadClip\",\"inputs\":{\"model_id\":\"$CLIP_ID\",\"clip_type\":\"qwen3\"}},
    {\"id\":\"latent\",\"type\":\"EmptyLatent\",\"inputs\":{\"width\":64,\"height\":64,\"model\":{\"node_id\":\"model\",\"output_slot\":\"model\"}}},
    {\"id\":\"cond\",\"type\":\"ClipTextEncode\",\"inputs\":{\"clip\":{\"node_id\":\"encoder\",\"output_slot\":\"clip\"},\"positive_text\":\"a photograph of a red fox in a snowy forest\"}},
    {\"id\":\"sampled\",\"type\":\"Sampler\",\"inputs\":{\"model\":{\"node_id\":\"model\",\"output_slot\":\"model\"},\"conditioning\":{\"node_id\":\"cond\",\"output_slot\":\"conditioning\"},\"clip\":{\"node_id\":\"encoder\",\"output_slot\":\"clip\"},\"latent\":{\"node_id\":\"latent\",\"output_slot\":\"latent\"},\"steps\":4,\"cfg\":1.0,\"seed\":-1}},
    {\"id\":\"decoded\",\"type\":\"VaeDecode\",\"inputs\":{\"vae\":{\"node_id\":\"vae\",\"output_slot\":\"vae\"},\"latent\":{\"node_id\":\"sampled\",\"output_slot\":\"latent\"}}},
    {\"id\":\"saved\",\"type\":\"SaveImage\",\"inputs\":{\"image\":{\"node_id\":\"decoded\",\"output_slot\":\"image\"},\"seed\":{\"node_id\":\"sampled\",\"output_slot\":\"seed\"}}}
  ]},\"settings\":{}}" \
  | python3 -c "import sys,json;print(json.load(sys.stdin)['job_id'])")
sleep 5
HASH=$(curl -s "http://127.0.0.1:8488/v1/jobs/$JOB_ID" | python3 -c "
import sys,json
d=json.load(sys.stdin)
assert d['status']=='Completed'
print(d.get('artifact_hash') or d.get('result',{}).get('artifact_hash'))
")
curl -s -o saved_proof.png "http://127.0.0.1:8488/v1/artifacts/$HASH"
python3 -c "from PIL import Image; im=Image.open('saved_proof.png'); assert im.size==(64,64)"
# -> exits 0; a real, retrievable 64x64 PNG was produced
kill %1
rm -f saved_proof.png
```

```powershell
# Runnable Proof (manual, PowerShell): real (non-mock-hardware) mode, fixture
# checkpoints already registered in the model registry under their SHA256 ids.
cargo build --release -p anvilml
$proc = Start-Process -FilePath .\target\release\anvilml.exe -PassThru
Start-Sleep -Seconds 2
$zitId = (Get-FileHash -Algorithm SHA256 worker\tests\fixtures\zit_tiny.safetensors).Hash.ToLower()
$vaeId = (Get-FileHash -Algorithm SHA256 worker\tests\fixtures\zit_vae_tiny.safetensors).Hash.ToLower()
$clipId = (Get-FileHash -Algorithm SHA256 worker\tests\fixtures\qwen3_tiny.safetensors).Hash.ToLower()
$graph = @{
    nodes = @(
        @{ id="model"; type="LoadModel"; inputs=@{ model_id=$zitId } }
        @{ id="vae"; type="LoadVae"; inputs=@{ model_id=$vaeId } }
        @{ id="encoder"; type="LoadClip"; inputs=@{ model_id=$clipId; clip_type="qwen3" } }
        @{ id="latent"; type="EmptyLatent"; inputs=@{ width=64; height=64; model=@{ node_id="model"; output_slot="model" } } }
        @{ id="cond"; type="ClipTextEncode"; inputs=@{ clip=@{ node_id="encoder"; output_slot="clip" }; positive_text="a photograph of a red fox in a snowy forest" } }
        @{ id="sampled"; type="Sampler"; inputs=@{ model=@{ node_id="model"; output_slot="model" }; conditioning=@{ node_id="cond"; output_slot="conditioning" }; clip=@{ node_id="encoder"; output_slot="clip" }; latent=@{ node_id="latent"; output_slot="latent" }; steps=4; cfg=1.0; seed=-1 } }
        @{ id="decoded"; type="VaeDecode"; inputs=@{ vae=@{ node_id="vae"; output_slot="vae" }; latent=@{ node_id="sampled"; output_slot="latent" } } }
        @{ id="saved"; type="SaveImage"; inputs=@{ image=@{ node_id="decoded"; output_slot="image" }; seed=@{ node_id="sampled"; output_slot="seed" } } }
    )
}
$body = @{ graph = $graph; settings = @{} } | ConvertTo-Json -Depth 10
$resp = Invoke-WebRequest -Uri http://127.0.0.1:8488/v1/jobs -Method Post -Body $body -ContentType 'application/json' -UseBasicParsing
$jobId = ($resp.Content | ConvertFrom-Json).job_id
Start-Sleep -Seconds 5
$job = (Invoke-WebRequest -Uri "http://127.0.0.1:8488/v1/jobs/$jobId" -UseBasicParsing).Content | ConvertFrom-Json
if ($job.status -ne "Completed") { throw "job did not complete" }
$hash = $job.artifact_hash
Invoke-WebRequest -Uri "http://127.0.0.1:8488/v1/artifacts/$hash" -OutFile saved_proof.png
Add-Type -AssemblyName System.Drawing
$img = [System.Drawing.Image]::FromFile((Resolve-Path saved_proof.png))
if ($img.Width -ne 64 -or $img.Height -ne 64) { throw "unexpected dimensions" }
$img.Dispose()
# -> no exception thrown; a real, retrievable 64x64 PNG was produced
Stop-Process -Id $proc.Id
Remove-Item saved_proof.png
```

---

## Phase 25 — Flux 2 Klein 4B Diffusion + Flux 2 VAE

**Capability proved:** A Flux 2 Klein 4B + Flux 2 VAE generation graph, submitted
through the exact same generic-node `POST /v1/jobs` pipeline Phase 24 used for ZiT,
produces a real, retrievable PNG artifact — confirming the generic node layer is
genuinely architecture-agnostic, with zero changes needed to add this second
diffusion architecture.

```bash
# Runnable Proof (manual): identical to Phase 24's sequence, with model_id values
# pointing at the Flux 2 Klein 4B + Flux 2 VAE fixtures, reusing Qwen3 4B (Phase 22)
# for the text encoder unchanged.
cargo build --release -p anvilml
./target/release/anvilml &
sleep 2
DIFF_ID=$(sha256sum worker/tests/fixtures/flux2klein4b_tiny.safetensors | head -c1048576 | cut -d' ' -f1)
VAE_ID=$(sha256sum worker/tests/fixtures/flux2_vae_tiny.safetensors | head -c1048576 | cut -d' ' -f1)
CLIP_ID=$(sha256sum worker/tests/fixtures/qwen3_tiny.safetensors | head -c1048576 | cut -d' ' -f1)
JOB_ID=$(curl -s -X POST http://127.0.0.1:8488/v1/jobs -H 'Content-Type: application/json' \
  -d "{\"graph\":{\"nodes\":[
    {\"id\":\"model\",\"type\":\"LoadModel\",\"inputs\":{\"model_id\":\"$DIFF_ID\"}},
    {\"id\":\"vae\",\"type\":\"LoadVae\",\"inputs\":{\"model_id\":\"$VAE_ID\"}},
    {\"id\":\"encoder\",\"type\":\"LoadClip\",\"inputs\":{\"model_id\":\"$CLIP_ID\",\"clip_type\":\"qwen3\"}},
    {\"id\":\"latent\",\"type\":\"EmptyLatent\",\"inputs\":{\"width\":64,\"height\":64,\"model\":{\"node_id\":\"model\",\"output_slot\":\"model\"}}},
    {\"id\":\"cond\",\"type\":\"ClipTextEncode\",\"inputs\":{\"clip\":{\"node_id\":\"encoder\",\"output_slot\":\"clip\"},\"positive_text\":\"a photograph of a red fox in a snowy forest\"}},
    {\"id\":\"sampled\",\"type\":\"Sampler\",\"inputs\":{\"model\":{\"node_id\":\"model\",\"output_slot\":\"model\"},\"conditioning\":{\"node_id\":\"cond\",\"output_slot\":\"conditioning\"},\"clip\":{\"node_id\":\"encoder\",\"output_slot\":\"clip\"},\"latent\":{\"node_id\":\"latent\",\"output_slot\":\"latent\"},\"steps\":4,\"cfg\":1.0,\"seed\":-1}},
    {\"id\":\"decoded\",\"type\":\"VaeDecode\",\"inputs\":{\"vae\":{\"node_id\":\"vae\",\"output_slot\":\"vae\"},\"latent\":{\"node_id\":\"sampled\",\"output_slot\":\"latent\"}}},
    {\"id\":\"saved\",\"type\":\"SaveImage\",\"inputs\":{\"image\":{\"node_id\":\"decoded\",\"output_slot\":\"image\"},\"seed\":{\"node_id\":\"sampled\",\"output_slot\":\"seed\"}}}
  ]},\"settings\":{}}" \
  | python3 -c "import sys,json;print(json.load(sys.stdin)['job_id'])")
sleep 5
HASH=$(curl -s "http://127.0.0.1:8488/v1/jobs/$JOB_ID" | python3 -c "
import sys,json
d=json.load(sys.stdin)
assert d['status']=='Completed'
print(d.get('artifact_hash') or d.get('result',{}).get('artifact_hash'))
")
curl -s -o saved_proof.png "http://127.0.0.1:8488/v1/artifacts/$HASH"
python3 -c "from PIL import Image; im=Image.open('saved_proof.png'); assert im.size==(64,64)"
# -> exits 0; a real, retrievable 64x64 PNG was produced via the unmodified generic
#    node pipeline, now serving a second diffusion architecture
kill %1
rm -f saved_proof.png
```

```powershell
# Runnable Proof (manual, PowerShell): identical to Phase 24's sequence, with
# model_id values pointing at the Flux 2 Klein 4B + Flux 2 VAE fixtures, reusing
# Qwen3 4B (Phase 22) for the text encoder unchanged.
cargo build --release -p anvilml
$proc = Start-Process -FilePath .\target\release\anvilml.exe -PassThru
Start-Sleep -Seconds 2
$diffId = (Get-FileHash -Algorithm SHA256 worker\tests\fixtures\flux2klein4b_tiny.safetensors).Hash.ToLower()
$vaeId = (Get-FileHash -Algorithm SHA256 worker\tests\fixtures\flux2_vae_tiny.safetensors).Hash.ToLower()
$clipId = (Get-FileHash -Algorithm SHA256 worker\tests\fixtures\qwen3_tiny.safetensors).Hash.ToLower()
$graph = @{
    nodes = @(
        @{ id="model"; type="LoadModel"; inputs=@{ model_id=$diffId } }
        @{ id="vae"; type="LoadVae"; inputs=@{ model_id=$vaeId } }
        @{ id="encoder"; type="LoadClip"; inputs=@{ model_id=$clipId; clip_type="qwen3" } }
        @{ id="latent"; type="EmptyLatent"; inputs=@{ width=64; height=64; model=@{ node_id="model"; output_slot="model" } } }
        @{ id="cond"; type="ClipTextEncode"; inputs=@{ clip=@{ node_id="encoder"; output_slot="clip" }; positive_text="a photograph of a red fox in a snowy forest" } }
        @{ id="sampled"; type="Sampler"; inputs=@{ model=@{ node_id="model"; output_slot="model" }; conditioning=@{ node_id="cond"; output_slot="conditioning" }; clip=@{ node_id="encoder"; output_slot="clip" }; latent=@{ node_id="latent"; output_slot="latent" }; steps=4; cfg=1.0; seed=-1 } }
        @{ id="decoded"; type="VaeDecode"; inputs=@{ vae=@{ node_id="vae"; output_slot="vae" }; latent=@{ node_id="sampled"; output_slot="latent" } } }
        @{ id="saved"; type="SaveImage"; inputs=@{ image=@{ node_id="decoded"; output_slot="image" }; seed=@{ node_id="sampled"; output_slot="seed" } } }
    )
}
$body = @{ graph = $graph; settings = @{} } | ConvertTo-Json -Depth 10
$resp = Invoke-WebRequest -Uri http://127.0.0.1:8488/v1/jobs -Method Post -Body $body -ContentType 'application/json' -UseBasicParsing
$jobId = ($resp.Content | ConvertFrom-Json).job_id
Start-Sleep -Seconds 5
$job = (Invoke-WebRequest -Uri "http://127.0.0.1:8488/v1/jobs/$jobId" -UseBasicParsing).Content | ConvertFrom-Json
if ($job.status -ne "Completed") { throw "job did not complete" }
$hash = $job.artifact_hash
Invoke-WebRequest -Uri "http://127.0.0.1:8488/v1/artifacts/$hash" -OutFile saved_proof.png
Add-Type -AssemblyName System.Drawing
$img = [System.Drawing.Image]::FromFile((Resolve-Path saved_proof.png))
if ($img.Width -ne 64 -or $img.Height -ne 64) { throw "unexpected dimensions" }
$img.Dispose()
# -> no exception thrown; a real, retrievable 64x64 PNG was produced via the
#    unmodified generic node pipeline, now serving a second diffusion architecture
Stop-Process -Id $proc.Id
Remove-Item saved_proof.png
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
# Runnable Proof (manual): identical to Phases 24/25's sequence, with model_id
# values pointing at the Flux 2 Klein 9B + Qwen3-8B fixtures (Phase 25's Flux 2
# VAE fixture is reused unchanged — VAE has no size variant per the model matrix).
cargo build --release -p anvilml
./target/release/anvilml &
sleep 2
DIFF_ID=$(sha256sum worker/tests/fixtures/flux2klein9b_tiny.safetensors | head -c1048576 | cut -d' ' -f1)
VAE_ID=$(sha256sum worker/tests/fixtures/flux2_vae_tiny.safetensors | head -c1048576 | cut -d' ' -f1)
CLIP_ID=$(sha256sum worker/tests/fixtures/qwen3_8b_tiny.safetensors | head -c1048576 | cut -d' ' -f1)
JOB_ID=$(curl -s -X POST http://127.0.0.1:8488/v1/jobs -H 'Content-Type: application/json' \
  -d "{\"graph\":{\"nodes\":[
    {\"id\":\"model\",\"type\":\"LoadModel\",\"inputs\":{\"model_id\":\"$DIFF_ID\"}},
    {\"id\":\"vae\",\"type\":\"LoadVae\",\"inputs\":{\"model_id\":\"$VAE_ID\"}},
    {\"id\":\"encoder\",\"type\":\"LoadClip\",\"inputs\":{\"model_id\":\"$CLIP_ID\",\"clip_type\":\"qwen3\"}},
    {\"id\":\"latent\",\"type\":\"EmptyLatent\",\"inputs\":{\"width\":64,\"height\":64,\"model\":{\"node_id\":\"model\",\"output_slot\":\"model\"}}},
    {\"id\":\"cond\",\"type\":\"ClipTextEncode\",\"inputs\":{\"clip\":{\"node_id\":\"encoder\",\"output_slot\":\"clip\"},\"positive_text\":\"a photograph of a red fox in a snowy forest\"}},
    {\"id\":\"sampled\",\"type\":\"Sampler\",\"inputs\":{\"model\":{\"node_id\":\"model\",\"output_slot\":\"model\"},\"conditioning\":{\"node_id\":\"cond\",\"output_slot\":\"conditioning\"},\"clip\":{\"node_id\":\"encoder\",\"output_slot\":\"clip\"},\"latent\":{\"node_id\":\"latent\",\"output_slot\":\"latent\"},\"steps\":4,\"cfg\":1.0,\"seed\":-1}},
    {\"id\":\"decoded\",\"type\":\"VaeDecode\",\"inputs\":{\"vae\":{\"node_id\":\"vae\",\"output_slot\":\"vae\"},\"latent\":{\"node_id\":\"sampled\",\"output_slot\":\"latent\"}}},
    {\"id\":\"saved\",\"type\":\"SaveImage\",\"inputs\":{\"image\":{\"node_id\":\"decoded\",\"output_slot\":\"image\"},\"seed\":{\"node_id\":\"sampled\",\"output_slot\":\"seed\"}}}
  ]},\"settings\":{}}" \
  | python3 -c "import sys,json;print(json.load(sys.stdin)['job_id'])")
sleep 5
HASH=$(curl -s "http://127.0.0.1:8488/v1/jobs/$JOB_ID" | python3 -c "
import sys,json
d=json.load(sys.stdin)
assert d['status']=='Completed'
print(d.get('artifact_hash') or d.get('result',{}).get('artifact_hash'))
")
curl -s -o saved_proof.png "http://127.0.0.1:8488/v1/artifacts/$HASH"
python3 -c "from PIL import Image; im=Image.open('saved_proof.png'); assert im.size==(64,64)"
# -> exits 0; a real, retrievable 64x64 PNG was produced, closing all three rows
#    of the MVP model matrix
kill %1
rm -f saved_proof.png
```

```powershell
# Runnable Proof (manual, PowerShell): identical to Phases 24/25's sequence, with
# model_id values pointing at the Flux 2 Klein 9B + Qwen3-8B fixtures (Phase 25's
# Flux 2 VAE fixture is reused unchanged — VAE has no size variant per the model
# matrix).
cargo build --release -p anvilml
$proc = Start-Process -FilePath .\target\release\anvilml.exe -PassThru
Start-Sleep -Seconds 2
$diffId = (Get-FileHash -Algorithm SHA256 worker\tests\fixtures\flux2klein9b_tiny.safetensors).Hash.ToLower()
$vaeId = (Get-FileHash -Algorithm SHA256 worker\tests\fixtures\flux2_vae_tiny.safetensors).Hash.ToLower()
$clipId = (Get-FileHash -Algorithm SHA256 worker\tests\fixtures\qwen3_8b_tiny.safetensors).Hash.ToLower()
$graph = @{
    nodes = @(
        @{ id="model"; type="LoadModel"; inputs=@{ model_id=$diffId } }
        @{ id="vae"; type="LoadVae"; inputs=@{ model_id=$vaeId } }
        @{ id="encoder"; type="LoadClip"; inputs=@{ model_id=$clipId; clip_type="qwen3" } }
        @{ id="latent"; type="EmptyLatent"; inputs=@{ width=64; height=64; model=@{ node_id="model"; output_slot="model" } } }
        @{ id="cond"; type="ClipTextEncode"; inputs=@{ clip=@{ node_id="encoder"; output_slot="clip" }; positive_text="a photograph of a red fox in a snowy forest" } }
        @{ id="sampled"; type="Sampler"; inputs=@{ model=@{ node_id="model"; output_slot="model" }; conditioning=@{ node_id="cond"; output_slot="conditioning" }; clip=@{ node_id="encoder"; output_slot="clip" }; latent=@{ node_id="latent"; output_slot="latent" }; steps=4; cfg=1.0; seed=-1 } }
        @{ id="decoded"; type="VaeDecode"; inputs=@{ vae=@{ node_id="vae"; output_slot="vae" }; latent=@{ node_id="sampled"; output_slot="latent" } } }
        @{ id="saved"; type="SaveImage"; inputs=@{ image=@{ node_id="decoded"; output_slot="image" }; seed=@{ node_id="sampled"; output_slot="seed" } } }
    )
}
$body = @{ graph = $graph; settings = @{} } | ConvertTo-Json -Depth 10
$resp = Invoke-WebRequest -Uri http://127.0.0.1:8488/v1/jobs -Method Post -Body $body -ContentType 'application/json' -UseBasicParsing
$jobId = ($resp.Content | ConvertFrom-Json).job_id
Start-Sleep -Seconds 5
$job = (Invoke-WebRequest -Uri "http://127.0.0.1:8488/v1/jobs/$jobId" -UseBasicParsing).Content | ConvertFrom-Json
if ($job.status -ne "Completed") { throw "job did not complete" }
$hash = $job.artifact_hash
Invoke-WebRequest -Uri "http://127.0.0.1:8488/v1/artifacts/$hash" -OutFile saved_proof.png
Add-Type -AssemblyName System.Drawing
$img = [System.Drawing.Image]::FromFile((Resolve-Path saved_proof.png))
if ($img.Width -ne 64 -or $img.Height -ne 64) { throw "unexpected dimensions" }
$img.Dispose()
# -> no exception thrown; a real, retrievable 64x64 PNG was produced, closing all
#    three rows of the MVP model matrix
Stop-Process -Id $proc.Id
Remove-Item saved_proof.png
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

```powershell
# Runnable Proof (manual, PowerShell):
Remove-Item -Recurse -Force -ErrorAction SilentlyContinue worker\.venv
cargo build --release -p anvilml
$proc = Start-Process -FilePath .\target\release\anvilml.exe -PassThru
Start-Sleep -Seconds 90
Stop-Process -Id $proc.Id -ErrorAction SilentlyContinue
.\target\release\anvilml.exe --version
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

```powershell
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

```powershell
# See P30-D1 for the complete final gate sequence — every command exits 0.
```

---

## Phase 900 — Spec-Drift & Logging Retrofit: tracing-subscriber, /health Body, Missing ToSchema & DB Wiring

**Capability proved:** The `anvilml` binary emits real log output honouring
`ANVILML_LOG`/`RUST_LOG`, selectable as plain text or JSON via `--log-format`
(previously silent regardless of either variable, and the flag did not exist);
`GET /health` returns the `ANVILML_DESIGN.md §13.4`-specified JSON body
(`status`, `version`, `uptime_s`) instead of a bare `200` with no body; and the
binary now creates its SQLite database, runs migrations, and loads the
device-capability seed data on every real startup (previously no `.db` file was
ever produced, despite the pool/migration/seed code existing and passing its own
unit tests since Phase 6).

```bash
rm -f /tmp/anvilml-proof.db
cargo build --release -p anvilml --features mock-hardware
ANVILML_LOG=debug ANVILML_DB_PATH=/tmp/anvilml-proof.db ./target/release/anvilml --log-format json &
sleep 1
curl -s http://127.0.0.1:8488/health | python3 -c "import sys,json; d=json.load(sys.stdin); assert d['status']=='ok' and isinstance(d['version'],str) and isinstance(d['uptime_s'],int)"
# -> exits 0; status/version/uptime_s all present and correctly typed; stderr
#    shows real DEBUG-level output as JSON from the same run
kill %1
sleep 1
test -f /tmp/anvilml-proof.db
sqlite3 /tmp/anvilml-proof.db "SELECT COUNT(*) FROM device_capabilities;" | grep -qv '^0$'
# -> both exit 0; the .db file exists and device_capabilities has at least one
#    row — previously no .db file was ever created by the running binary
```

```powershell
Remove-Item -Path "$env:TEMP\anvilml-proof.db" -ErrorAction SilentlyContinue
cargo build --release -p anvilml --features mock-hardware
$env:ANVILML_LOG = "debug"
$env:ANVILML_DB_PATH = "$env:TEMP\anvilml-proof.db"
$proc = Start-Process -FilePath .\target\release\anvilml.exe -ArgumentList "--log-format","json" -PassThru
Start-Sleep -Seconds 1
$health = (Invoke-WebRequest -Uri http://127.0.0.1:8488/health -UseBasicParsing).Content | ConvertFrom-Json
if ($health.status -ne "ok") { throw "status mismatch" }
if ($health.version -isnot [string]) { throw "version is not a string" }
if ($health.uptime_s -isnot [int] -and $health.uptime_s -isnot [long]) { throw "uptime_s is not an integer" }
# -> no exception thrown; status/version/uptime_s all present and correctly typed
Stop-Process -Id $proc.Id
Start-Sleep -Seconds 1
if (-not (Test-Path "$env:TEMP\anvilml-proof.db")) { throw "db file was not created" }
# -> no exception thrown; the .db file exists — previously no .db file was ever
#    created by the running binary
```