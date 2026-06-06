# Implementation Report: P9-A1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P9-A1                                             |
| Phase       | 009 — Worker Spawn & Handshake                    |
| Description | worker: Python package skeleton + ipc.py binary-stdio guard + framing |
| Implemented | 2026-06-06T10:45:00Z                              |
| Status      | COMPLETE                                          |

## Summary

Created the Python worker package (`worker/`) with directory structure, package init files, four PyTorch selector requirements files, core dependency manifest (base.txt), and `worker/ipc.py` implementing the stdin/stdout msgpack framing protocol with Windows binary-stdio guard. Added five unit tests exercising read_frame/write_frame roundtrip with dict, bytes, and empty-dict payloads, plus a source-code presence check for the Windows guard. All Rust build/lint/cross-check gates pass; all Python tests pass (4 passed, 1 skipped on Linux).

## Resolved Dependencies

| Type   | Name        | Version resolved | Source         |
|--------|-------------|-----------------|----------------|
| python | msgpack     | 1.1.2           | pypi-query MCP |
| python | Pillow      | 12.2.0          | pypi-query MCP |
| python | numpy       | 2.4.6           | pypi-query MCP |
| python | safetensors | 0.7.0           | pypi-query MCP |
| python | diffusers   | 0.38.0          | pypi-query MCP |
| python | transformers| 5.10.2          | pypi-query MCP |
| python | torch       | 2.12.0          | pypi-query MCP |

All versions are compatible with Python >=3.10 (the project's target). The plan's version constraints (`msgpack>=1.0`, `Pillow>=10.0`, `torch>=2.5.0`) are satisfied by the latest releases.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Create | `worker/__init__.py` | Package marker (empty) |
| Create | `worker/nodes/__init__.py` | Node registry placeholder (empty) |
| Create | `worker/tests/__init__.py` | Test package marker (empty) |
| Create | `worker/ipc.py` | Stdio framing + Windows binary-mode guard |
| Create | `worker/requirements/base.txt` | Core Python deps for worker |
| Create | `worker/requirements/cuda.txt` | PyTorch CUDA selector |
| Create | `worker/requirements/cpu.txt` | PyTorch CPU-only selector |
| Create | `worker/requirements/rocm-linux.txt` | PyTorch ROCm Linux selector |
| Create | `worker/requirements/rocm-windows.txt` | AMD PyTorch-on-Windows selector |
| Create | `worker/tests/test_ipc.py` | Unit tests for ipc.py framing functions |

## Commit Log

```
 worker/__init__.py                   |   0
 worker/ipc.py                        |  69 ++++++++++++++++++++
 worker/nodes/__init__.py             |   0
 worker/requirements/base.txt         |   7 +++
 worker/requirements/cpu.txt          |   1 +
 worker/requirements/cuda.txt         |   2 +
 worker/requirements/rocm-linux.txt   |   2 +
 worker/requirements/rocm-windows.txt |   2 +
 worker/tests/__init__.py             |   0
 worker/tests/test_ipc.py             | 118 +++++++++++++++++++++++++++++++++++
 10 files changed, 201 insertions(+)
```

## Test Results

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.0.3, pluggy-1.6.0 -- /home/dryw/forge/.venv/bin/python3
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML
plugins: anyio-4.12.1
collecting ... collected 5 items

worker/tests/test_ipc.py::TestReadFrame::test_write_read_roundtrip PASSED [ 20%]
worker/tests/test_ipc.py::TestReadFrame::test_roundtrip_with_bytes PASSED [ 40%]
worker/tests/test_ipc.py::TestReadFrame::test_roundtrip_empty_dict PASSED [ 60%]
worker/tests/test_ipc.py::TestWindowsGuard::test_windows_binary_mode_guard_present SKIPPED [ 80%]
worker/tests/test_ipc.py::TestWindowsGuard::test_guard_code_exists_in_source PASSED [100%]

========================= 4 passed, 1 skipped in 0.03s =========================
```

## Format Gate

```
(exit 0 — no formatting drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux check
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.34s

# 2. Mock-hardware Windows cross-check
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.52s

# 3. Real-hardware Linux check
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.23s

# 4. Real-hardware Windows cross-check
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.23s
```

All four platform cross-checks exit 0.

## Project Gates

Config drift gate (`cargo test -p backend --features mock-hardware`):
```
     Running tests/config_reference.rs (target/debug/deps/config_reference-dc805b1491e7b502c11e)
running 1 test
test test_toml_key_set_matches_default ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Deviations from Plan

- `ipc.py` uses `msgpack.unpackb(payload, raw=False)` (positional first argument) instead of the plan's implied `msgpack.unpackb(raw=False, data=payload)` keyword form. The plan described the API shape but the actual `msgpack` 1.x API takes data as the first positional argument; this was corrected during implementation after MCP lookup confirmed the signature.

## Blockers

None.
