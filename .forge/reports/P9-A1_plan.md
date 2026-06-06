# Plan Report: P9-A1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P9-A1                                             |
| Phase       | 009 — Worker Spawn & Handshake                    |
| Description | worker: Python package skeleton + ipc.py binary-stdio guard + framing |
| Depends on  | P8-A4                                              |
| Project     | anvilml                                            |
| Planned at  | 2026-06-06T10:15:00Z                              |
| Attempt     | 1                                                  |

## Objective

Create the Python worker package skeleton (`worker/`) with directory structure, a `requirements/` subdirectory containing base.txt and four torch-selector files (cuda.txt, rocm-linux.txt, rocm-windows.txt, cpu.txt), and `worker/ipc.py` implementing the stdin/stdout msgpack framing protocol with the Windows binary-stdio guard. Include a test file `worker/tests/test_ipc.py` that exercises read_frame/write_frame and exits 0 under pytest.

## Scope

### In Scope
- Create directory structure: `worker/`, `worker/nodes/`, `worker/tests/`, `worker/requirements/`
- Create `worker/__init__.py` (empty)
- Create `worker/nodes/__init__.py` (empty — NODE_REGISTRY placeholder for later tasks)
- Create `worker/tests/__init__.py` (empty)
- Create `worker/ipc.py`:
  - Windows binary-stdio guard via `msvcrt.setmode(os.O_BINARY)` on stdin and stdout at module import time
  - `read_frame()` function: read 4-byte big-endian length prefix, then N bytes of msgpack payload, return `msgpack.unpackb(raw=False)` result
  - `write_frame()` function: `msgpack.packb(use_bin_type=True)`, prepend 4-byte big-endian length, write to `sys.stdout.buffer`, flush
- Create `worker/requirements/base.txt` with core deps: `msgpack>=1.0`, `Pillow>=10.0`, `numpy`, `safetensors`, `diffusers`, `transformers`, `pytest`
- Create `worker/requirements/cuda.txt` — PyTorch CUDA index URL + torch package (compatible with base.txt)
- Create `worker/requirements/rocm-linux.txt` — PyTorch ROCm Linux pip index + torch package (ROCm ≥ 7.2)
- Create `worker/requirements/rocm-windows.txt` — AMD PyTorch-on-Windows package (ROCm ≥ 7.2, AMD-hosted wheels), not the Linux ROCm pip index
- Create `worker/requirements/cpu.txt` — PyTorch CPU-only package
- Create `worker/tests/test_ipc.py`: test read_frame/write_frame roundtrip via subprocess or mock pipes; exits 0

### Out of Scope
- `worker/worker_main.py` (task P9-A2)
- Rust `anvilml-worker` crate (`env.rs`, `managed.rs`, `pool.rs`) — tasks P9-A3, P9-A4, P9-A5
- Node implementations (`common.py`, `zit.py`, `sdxl.py`) — later phases
- Provisioning scripts (`install_worker_deps.sh/ps1`) — task P22-A1
- Any Rust-side IPC code (handled in `anvilml-ipc` crate by earlier tasks)

## Approach

1. **Create directory structure.** Create the four directories: `worker/`, `worker/nodes/`, `worker/tests/`, `worker/requirements/`.

2. **Create package init files.** Write three empty `__init__.py` files at `worker/__init__.py`, `worker/nodes/__init__.py`, and `worker/tests/__init__.py`.

3. **Create base.txt.** Write `worker/requirements/base.txt` with six core dependencies plus pytest:
   ```
   msgpack>=1.0
   Pillow>=10.0
   numpy
   safetensors
   diffusers
   transformers
   pytest
   ```

4. **Create torch selector files.** Each file contains only the PyTorch package line (and optionally an index URL). The install scripts (later tasks) will `pip install -r base.txt` then `pip install -r {selector}.txt`.

   - `worker/requirements/cuda.txt`:
     ```
     --index-url https://download.pytorch.org/whl/cu124
     torch>=2.5.0
     ```
   - `worker/requirements/rocm-linux.txt`:
     ```
     --index-url https://download.pytorch.org/whl/rocm6.2
     torch>=2.5.0
     ```
   - `worker/requirements/rocm-windows.txt`:
     ```
     --extra-index-url https://mirrors.tuna.tsinghua.edu.cn/pypi/web/simple  # AMD PyTorch on Windows fallback
     torch>=2.5.0
     ```
     (Note: the actual AMD-hosted URL and package naming for PyTorch-on-Windows will be confirmed during ACT; this plan uses a placeholder index URL that should be replaced with the correct AMD PyTorch-on-Windows distribution endpoint — see Risks.)

   - `worker/requirements/cpu.txt`:
     ```
     torch>=2.5.0
     ```

5. **Create `worker/ipc.py`.** The file contains:
   - Module-level imports (`sys`, `struct`, `msgpack`)
   - Windows binary-stdio guard (guarded by `if sys.platform == "win32": import msvcrt, os; msvcrt.setmode(sys.stdin.fileno(), os.O_BINARY); msvcrt.setmode(sys.stdout.fileno(), os.O_BINARY)`)
   - `read_frame()` function that: reads exactly 4 bytes for the length prefix (looping on short reads), unpacks as big-endian u32, then reads exactly N bytes (looping on short reads), calls `msgpack.unpackb(raw=False)` on the payload, and returns the result.
   - `write_frame(data)` function that: calls `msgpack.packb(use_bin_type=True, default=str)` on the data, packs the length as 4-byte big-endian u32, writes the combined header+payload to `sys.stdout.buffer`, and flushes.

6. **Create `worker/tests/test_ipc.py`.** Tests:
   - `test_write_read_roundtrip`: construct a dict payload, call `write_frame(payload)`, then `read_frame()`, assert equality. This test runs by mocking `sys.stdin.buffer` and `sys.stdout.buffer` with `io.BytesIO` objects to avoid requiring a real subprocess.
   - `test_windows_binary_mode_guard_present`: asserts the `msvcrt.setmode` call pattern exists in the source code (simple AST or regex check), ensuring the guard cannot be accidentally removed. This test is skipped on non-Windows platforms.

7. **Verify with pytest.** Run `python3 -m pytest worker/tests/test_ipc.py -v` and confirm exit 0.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Create | `worker/__init__.py` | Package marker (empty) |
| Create | `worker/nodes/__init__.py` | Node registry placeholder (empty) |
| Create | `worker/tests/__init__.py` | Test package marker (empty) |
| Create | `worker/ipc.py` | Stdio framing + Windows binary-mode guard |
| Create | `worker/requirements/base.txt` | Core Python deps for worker |
| Create | `worker/requirements/cuda.txt` | PyTorch CUDA selector |
| Create | `worker/requirements/rocm-linux.txt` | PyTorch ROCm Linux selector |
| Create | `worker/requirements/rocm-windows.txt` | AMD PyTorch-on-Windows selector (ROCm ≥ 7.2) |
| Create | `worker/requirements/cpu.txt` | PyTorch CPU-only selector |
| Create | `worker/tests/test_ipc.py` | Unit tests for ipc.py framing functions |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `worker/tests/test_ipc.py` | `test_write_read_roundtrip` | `write_frame()` + `read_frame()` roundtrip preserves msgpack data correctly (dict, nested dict, bytes) |
| `worker/tests/test_ipc.py` | `test_windows_binary_mode_guard_present` | Source code contains the `msvcrt.setmode` binary-mode guard (skipped on non-Windows) |

## CI Impact

No CI workflow files are modified. The new Python test file will be picked up by the existing CI gate command documented in `docs/ENVIRONMENT.md`:

```bash
ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/ -v
```

This command was previously configured but had no tests; now it will execute `test_ipc.py`. No Rust CI gates are affected since this task only touches Python files.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| AMD PyTorch-on-Windows index URL is not yet known precisely | High | Low (plan placeholder; ACT will resolve with correct URL) | Plan notes the gap; during ACT, verify the exact AMD-hosted wheel URL and package name. The `rocm-windows.txt` file can be updated without breaking other files. |
| `msgpack` version compatibility on Python 3.12 | Low | None | Verified via MCP: msgpack >=1.0 supports Python >=3.9, so 3.12 is compatible. |
| `io.BytesIO` mock approach for testing may not exercise real pipe behavior | Medium | Low | The roundtrip test validates the framing logic (length prefix, big-endian encoding, msgpack serialization). Real pipe behavior is exercised by integration tests in later tasks (P9-A2, P9-A4). |
| `transformers` and `diffusers` have transitive dependencies that may conflict | Low | Medium | These are specified without version pins in base.txt to allow pip resolver flexibility. The ACT session will note any pinning suggestions if conflicts arise during installation testing. |

## Acceptance Criteria

- [ ] All six files exist: `worker/__init__.py`, `worker/nodes/__init__.py`, `worker/tests/__init__.py`, `worker/ipc.py`, `worker/requirements/base.txt`, and all four torch selector files
- [ ] `worker/ipc.py` contains the Windows binary-stdio guard (`msvcrt.setmode` with `os.O_BINARY`) guarded by `sys.platform == "win32"`
- [ ] `read_frame()` reads a 4-byte big-endian length prefix, then N bytes of payload, and returns `msgpack.unpackb(raw=False)` result
- [ ] `write_frame()` serialises with `msgpack.packb(use_bin_type=True)`, prepends a 4-byte big-endian length, writes to `sys.stdout.buffer`, and flushes
- [ ] `worker/requirements/base.txt` contains: msgpack>=1.0, Pillow>=10.0, numpy, safetensors, diffusers, transformers, pytest
- [ ] Each torch selector file exists and references a PyTorch version compatible with Python 3.12 (torch>=2.5.0)
- [ ] `worker/tests/test_ipc.py` contains at least one test that exercises read_frame/write_frame roundtrip
- [ ] `python3 -m pytest worker/tests/test_ipc.py -v` exits 0
