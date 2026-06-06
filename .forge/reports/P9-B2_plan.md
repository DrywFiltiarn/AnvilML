# Plan Report: P9-B2

| Field       | Value                                         |
|-------------|-----------------------------------------------|
| Task ID     | P9-B2                                         |
| Phase       | 009 — Worker Spawn & Handshake                |
| Description | ci: add python-worker job (Linux + Windows pytest, ANVILML_WORKER_MOCK=1) |
| Depends on  | P9-A1, P9-A2                                  |
| Project     | anvilml                                       |
| Planned at  | 2026-06-06T13:50:00Z                          |
| Attempt     | 1                                             |

## Objective

Add a new `python-worker` CI job to `.github/workflows/ci.yml` that runs the Python worker test suite (`worker/tests/`) on both Linux and Windows using `ANVILML_WORKER_MOCK=1`. This is an independent, hermetic job (no torch, no GPU) as specified in ANVILML_DESIGN §20.2.

## Scope

### In Scope
- Add a new `python-worker` job to `.github/workflows/ci.yml` with a matrix strategy for `os: [ubuntu-latest, windows-latest]`.
- Steps within the job: `actions/checkout@v6`, `actions/setup-python@v5` (Python 3.12), `pip install msgpack pillow pytest`, and `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/ -v`.
- No other CI jobs are modified, disabled, or renamed.
- No source code, test files, config files, or Rust crate changes.

### Out of Scope
- Modifying existing `rust-linux` or `rust-windows` jobs (those were handled in P9-B1).
- Adding or modifying any Python worker source code (`worker/worker_main.py`, `worker/ipc.py`, etc.).
- Adding new test files under `worker/tests/`.
- Installing torch or any GPU-related packages.
- Changing the CI trigger configuration (`on:` block).

## Approach

1. **Read current ci.yml** (already read at `/home/dryw/AnvilML/.github/workflows/ci.yml`). Confirm it has two jobs: `rust-linux` and `rust-windows`. No existing job named `python-worker` exists.

2. **Append the new `python-worker` job** to the end of `.github/workflows/ci.yml`, after the `rust-windows` job. The job will use a matrix strategy with `os: [ubuntu-latest, windows-latest]`.

3. **Job structure** (exact steps):
   - `actions/checkout@v6` — check out the repository.
   - `actions/setup-python@v5` with `python-version: "3.12"` — install a standalone Python 3.12 (no venv needed since we only need msgpack, pillow, pytest).
   - Run step: `pip install msgpack pillow pytest` — install the three test dependencies (no torch; mock mode skips the torch import as documented in ANVILML_DESIGN §20.2 and ENVIRONMENT.md §3.6).
   - Run step: `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/ -v` — run the Python worker test suite in mock mode. The existing tests (`test_ipc.py`, `test_worker_main.py`) use msgpack for framing and spawn `worker_main.py` as a subprocess with `ANVILML_WORKER_MOCK=1`.

4. **Preserve all existing jobs** — per FORGE_AGENT_RULES §5.5, the two existing Rust CI jobs (`rust-linux`, `rust-windows`) remain untouched. The new job is appended after them.

5. **No version lookups needed** — all dependency versions (msgpack, pillow, pytest) are already established in the project's `worker/requirements/base.txt` and are used by the existing P9-B1 venv steps. No MCP tool call required.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `.github/workflows/ci.yml` | Append new `python-worker` job with OS matrix (ubuntu-latest, windows-latest) and pytest run step |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `worker/tests/test_ipc.py` | IPC framing round-trips | `read_frame`/`write_frame` correctness, length-prefix encoding |
| `worker/tests/test_worker_main.py::TestWorkerMain::test_ready_on_init_hardware` | InitializeHardware → Ready + Shutdown → Dying | Worker responds to Init/Shutdown protocol |
| `worker/tests/test_worker_main.py::TestWorkerMain::test_mock_values` | Mock Ready payload values | vram=8192, arch=gfx1100, fp16/bf16/flash_attention flags match spec |
| `worker/tests/test_worker_main.py::TestWorkerMain::test_ping_pong` | Ping{seq} → Pong{seq} | Keepalive protocol round-trip |
| `worker/tests/test_worker_main.py::TestWorkerMain::test_memory_query_report` | MemoryQuery → MemoryReport{0,0} | Memory query handling in mock mode |
| `worker/tests/test_worker_main.py::TestWorkerMain::test_shutdown_dying_exit` | Shutdown → Dying + exit 0 | Clean shutdown with correct exit code |

## CI Impact

A new independent CI job (`python-worker`) is added to the existing CI workflow. It runs on every push and pull request to `main`, alongside the two existing Rust jobs (`rust-linux`, `rust-windows`). The new job does not affect the execution, timing, or outcome of any existing job. If this job fails, it will mark the overall CI check as failed for that run, which is the intended behavior — it catches regressions in the Python worker test suite independently from the Rust tests.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `worker/tests/` does not exist or is empty when this job first runs | Low | Job fails with "no tests collected" | P9-A1 already created `test_ipc.py`; P9-A2 already created `test_worker_main.py`. Both are committed. The job will only be activated after those tasks land on the target branch. |
| `actions/setup-python@v5` unavailable or slow on windows-latest runner | Low | CI delay, not failure | This is a standard GitHub-hosted action; well-maintained and cached by GitHub. |
| pytest discovers no tests due to missing `__init__.py` in `worker/tests/` | Low | Job fails with "no tests collected" | `worker/tests/__init__.py` already exists (confirmed in glob results). |
| Test hangs due to subprocess not receiving Shutdown frame | Medium | CI timeout (6 min default), job fails | Tests are designed to send Shutdown after Init; all 5 test methods close stdin and call `proc.wait(timeout=5)`. If a test hangs, it will be caught by the CI timeout and reported as a failure for investigation. |
| Windows path handling in tests (CRLF line endings via `.gitattributes`) | Low | Test assertion mismatch | `.gitattributes` already enforces LF for `.py` files; no CRLF issues expected. |

## Acceptance Criteria

- [ ] `python-worker` job appears in `.github/workflows/ci.yml` after the `rust-windows` job
- [ ] Job uses matrix strategy with `os: [ubuntu-latest, windows-latest]`
- [ ] Job includes checkout@v6, setup-python 3.12, pip install msgpack pillow pytest, and pytest run step
- [ ] No existing jobs (rust-linux, rust-windows) are modified in any way
- [ ] `ANVILML_WORKER_MOCK=1` is set on the pytest run step
- [ ] Local Linux verification passes: `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/ -v` exits 0
