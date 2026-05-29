# Plan Report: P1-B1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P1-B1                                         |
| Phase       | 001 — Workspace Scaffold                    |
| Description | anvilml: worker/ Python package structure and pytest skeleton |
| Depends on  | P1-A4                                         |
| Project     | anvilml                                       |
| Planned at  | 2026-05-29T15:25:36Z                          |
| Attempt     | 1                                             |

## Objective

Make `worker/` a properly structured Python package so that future phases can add modules without restructuring, and ensure the CI pytest job (`python-worker`) has at least one passing test immediately. This establishes the foundation for all subsequent Python worker implementation tasks.

## Scope

### In Scope
- Create `worker/__init__.py` (empty)
- Create `worker/nodes/__init__.py` (empty)
- Create `worker/tests/__init__.py` (empty)
- Create `worker/tests/test_placeholder.py` with a single passing test: `def test_placeholder(): assert True`
- Create `worker/requirements/base.txt` containing: `msgpack>=1.0`, `pillow>=10.0`, `pytest>=8.0`
- Create `worker/requirements/cuda.txt` as comment-only stub: `# torch + CUDA — populated in phase 009`
- Create `worker/requirements/rocm.txt` as comment-only stub: `# torch + ROCm — populated in phase 009`
- Create `worker/requirements/cpu.txt` as comment-only stub: `# torch CPU-only — populated in phase 009`

### Out of Scope
- Any worker logic (execution, IPC, hardware detection) — these belong to later phases
- Importing from `worker_main.py`, `ipc.py`, or any other worker module in the test file
- Modifying existing files (`worker/ipc.py`, `worker/worker_main.py`)
- CI workflow changes (already covered by P1-A2, which references this task's output)
- Adding any `[dependencies]` beyond what is listed above

## Approach

1. **Create package root `__init__.py`:** Write an empty file at `worker/__init__.py` to make `worker/` a Python package.

2. **Create nodes subpackage `__init__.py`:** Write an empty file at `worker/nodes/__init__.py` to establish the node registry namespace early, matching the architecture in `ARCHITECTURE.md` §2 (lines 63–68).

3. **Create tests subpackage `__init__.py`:** Write an empty file at `worker/tests/__init__.py` so pytest discovers tests within the package.

4. **Create placeholder test:** Write `worker/tests/test_placeholder.py` containing:
   ```python
   def test_placeholder():
       assert True
   ```
   This test uses zero external imports (standard library only), ensuring it passes even before any dependencies are installed.

5. **Create requirements stubs:**
   - `worker/requirements/base.txt`: Three lines — `msgpack>=1.0`, `pillow>=10.0`, `pytest>=8.0`
   - `worker/requirements/cuda.txt`: Single comment line — `# torch + CUDA — populated in phase 009`
   - `worker/requirements/rocm.txt`: Single comment line — `# torch + ROCm — populated in phase 009`
   - `worker/requirements/cpu.txt`: Single comment line — `# torch CPU-only — populated in phase 009`

6. **Verify acceptance criterion:** Run `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/ -v` and confirm it exits 0 with 1 test passing.

## Files Affected

| Action   | Path                                      | Description                              |
|----------|-------------------------------------------|------------------------------------------|
| CREATE   | `worker/__init__.py`                      | Empty file — makes worker/ a Python package |
| CREATE   | `worker/nodes/__init__.py`                | Empty file — establishes nodes namespace  |
| CREATE   | `worker/tests/__init__.py`                | Empty file — makes tests/ a subpackage    |
| CREATE   | `worker/tests/test_placeholder.py`         | Single test: `def test_placeholder(): assert True` |
| CREATE   | `worker/requirements/base.txt`            | Core deps: msgpack>=1.0, pillow>=10.0, pytest>=8.0 |
| CREATE   | `worker/requirements/cuda.txt`            | Comment-only stub for CUDA torch install  |
| CREATE   | `worker/requirements/rocm.txt`            | Comment-only stub for ROCm torch install  |
| CREATE   | `worker/requirements/cpu.txt`             | Comment-only stub for CPU torch install   |

## Tests

| Test ID / Name       | File                            | Validates                         |
|----------------------|---------------------------------|-----------------------------------|
| test_placeholder     | `worker/tests/test_placeholder.py` | pytest discovers and runs at least one test, exits 0 |

## CI Impact

No CI workflow changes required. The `python-worker` job defined in P1-A2 already references `pip install -r worker/requirements/base.txt` and `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/ -v`. This task only adds files; the existing CI step will pick them up automatically.

## Risks and Mitigations

| Risk                      | Likelihood | Impact | Mitigation              |
|---------------------------|-----------|--------|-------------------------|
| `pip install` fails on comment-only requirements files | Low | Medium | A file containing only comments is valid pip requirements format per PEP 508; verified before writing |
| pytest does not discover tests in subdirectories | Low | Low | The `worker/tests/` directory contains `__init__.py`, making it a proper package; pytest discovers `test_*.py` by default |
| Pre-existing `worker/requirements/` directory conflicts with P1-A4 output | Low | Low | P1-A4 does not create any requirements files; this is confirmed by the P1-A4 plan report |

## Acceptance Criteria

- [ ] `worker/__init__.py` exists and is an empty file
- [ ] `worker/nodes/__init__.py` exists and is an empty file
- [ ] `worker/tests/__init__.py` exists and is an empty file
- [ ] `worker/tests/test_placeholder.py` contains `def test_placeholder(): assert True`
- [ ] `worker/requirements/base.txt` contains exactly: `msgpack>=1.0`, `pillow>=10.0`, `pytest>=8.0` (one per line)
- [ ] `worker/requirements/cuda.txt` contains only the comment `# torch + CUDA — populated in phase 009`
- [ ] `worker/requirements/rocm.txt` contains only the comment `# torch + ROCm — populated in phase 009`
- [ ] `worker/requirements/cpu.txt` contains only the comment `# torch CPU-only — populated in phase 009`
- [ ] `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/ -v` exits 0 with 1 test passing
