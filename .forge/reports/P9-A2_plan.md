# Plan Report: P9-A2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P9-A2                                       |
| Phase       | 009 — Real Worker Startup                   |
| Description | worker/requirements/: real torch CPU wheel pin in cpu-* files |
| Depends on  | P9-A1                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-07-05T11:30:00Z                        |
| Attempt     | 1                                           |

## Objective

Populate the two empty placeholder requirement files created by P9-A1 (`worker/requirements/cpu-linux-agent.txt` and `worker/requirements/cpu-runner-reqs.txt`) with an identical torch CPU wheel pin, using `--index-url https://download.pytorch.org/whl/cpu` to ensure only the CPU-only PyTorch build is resolved. This closes a real gap: without this pin, every real-mode test from P9-C1 onward fails with `ImportError: No module named 'torch'` in environments that install strictly from these files.

## Scope

### In Scope
- Write the identical torch CPU wheel pin into `worker/requirements/cpu-linux-agent.txt`.
- Write the identical torch CPU wheel pin into `worker/requirements/cpu-runner-reqs.txt`.
- Both files contain: `--index-url https://download.pytorch.org/whl/cpu` followed by `torch==<resolved_version>` on the next line.
- The resolved version is looked up live via the PyPI MCP tool at plan time.
- Acceptance: `pip install --dry-run -r worker/requirements/cpu-linux-agent.txt` exits 0, resolving to a CPU-only torch wheel.
- Both files are byte-identical (verified by `cmp` or `diff`).

### Out of Scope
None. The `defers_to` field for this task is `[]` (empty). No functionality is deferred.

## Existing Codebase Assessment

P9-A1 created the `worker/requirements/` directory and populated `base.txt` with core non-torch dependencies (diffusers, msgpack, pillow, pyzmq, pytest, safetensors, transformers). The two target files (`cpu-linux-agent.txt` and `cpu-runner-reqs.txt`) were left as empty placeholders, as specified in P9-A1's instructions.

A third file, `cpu-linux-agents.txt` (note the plural 's'), already exists with content — it includes `--index-url https://download.pytorch.org/whl/cpu` plus `torch`, `torchaudio`, and `torchvision` without version pins. This file is **not** one of the task's target files and is not modified by this task. Its existence is noted but irrelevant to the plan.

The ROCm requirement files (`rocm-linux.txt`, `rocm-windows.txt`) use the same structural pattern: a comment header block, `--index-url` directive, then package names. The CPU files will follow the same pattern but with the PyTorch CPU index and a pinned torch version.

No source code is modified by this task — only two plain-text requirements files. No dual-mode parity markers, no Rust crates, no Python modules, no test files, and no logging changes are involved.

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| python | torch   | 2.12.1          | pypi-query MCP | n/a                    |

The current stable version of `torch` was resolved via `pypi-query_get_package_info` which reports `latest_version: "2.12.1"`. This version is compatible with Python 3.10+ (task target is Python 3.12.x per ENVIRONMENT.md §1). The CPU-only build is served from PyTorch's custom index (`https://download.pytorch.org/whl/cpu`) rather than the default PyPI index — the `--index-url` directive in the requirements file ensures pip resolves against this index exclusively, avoiding any CUDA-bundled wheels.

## Approach

1. **Resolve torch version via MCP.** Query `pypi-query_get_package_info` for `torch`. The MCP reports `latest_version: "2.12.1"`. Record this as the version to pin.

2. **Write `worker/requirements/cpu-linux-agent.txt`.** Create the file with two lines:
   ```
   --index-url https://download.pytorch.org/whl/cpu
   
   torch==2.12.1
   ```
   The empty line between `--index-url` and the package pin matches the pattern used in `cpu-linux-agents.txt` (which already exists with this structure). The `--index-url` directive tells pip to resolve all packages from PyTorch's CPU wheel index, ensuring only the CPU-only build is selected.

3. **Write `worker/requirements/cpu-runner-reqs.txt` with identical content.** Copy the exact content from step 2 into this file. Both files must be byte-identical.

4. **Verify acceptance criteria.** Run `pip install --dry-run -r worker/requirements/cpu-linux-agent.txt` — must exit 0. Run `cmp worker/requirements/cpu-linux-agent.txt worker/requirements/cpu-runner-reqs.txt` — must produce no diff output (exit 0).

## Public API Surface

None. This task modifies plain-text requirements files only; no code is written.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/requirements/cpu-linux-agent.txt` | Torch CPU wheel pin for Forge agent CI worker-test job |
| CREATE | `worker/requirements/cpu-runner-reqs.txt` | Identical torch CPU wheel pin for GitHub CI runner real-mode tests |

## Tests

None. This task creates plain-text requirements files with no executable code. The acceptance criteria are shell commands (`pip install --dry-run` and `cmp`), not test functions. No test file is created or modified.

## CI Impact

This task directly enables the `worker-linux-real` and `worker-windows-real` CI jobs defined in `.github/workflows/ci.yml`. These jobs install `cpu-runner-reqs.txt` as part of their setup. Without this task's content, those jobs would fail at the `pip install -r cpu-runner-reqs.txt` step because the file is empty (no packages to install, but the real-mode tests later fail with `ImportError: No module named 'torch'`). The `worker-linux-mock` and `worker-windows-mock` jobs are unaffected since they install only `base.txt` (which explicitly excludes torch).

No CI workflow file is modified by this task — it only populates a data file that the existing CI jobs already reference.

## Platform Considerations

None identified. The `--index-url https://download.pytorch.org/whl/cpu` URL is platform-neutral — PyTorch publishes CPU wheels for both Linux and Windows on this index. The requirements file format (`--index-url` followed by package pins) is identical on all platforms. No `#[cfg(...)]` guards, path separators, or line-ending handling are involved.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Torch 2.12.1 CPU wheel is not yet published on `download.pytorch.org/whl/cpu` — the MCP reports it as the latest PyPI version but PyTorch may not have released a CPU build for it yet. | Low | High | The ACT agent verifies at session start by running `pip install --dry-run`. If the CPU wheel is not available, the ACT agent should try the next most recent version (2.9.1 per the MCP version list) and re-verify. |
| `pip install --dry-run` resolves the default PyPI index instead of the CPU index, pulling in a CUDA-bundled wheel. | Low | Medium | The `--index-url` directive in the requirements file takes precedence over any pip config. The ACT agent verifies by checking that the resolved package metadata identifies it as a CPU build (no CUDA dependencies). |
| Byte-identity mismatch between the two files causes subtle differences in how CI runners consume them. | Very Low | Low | The plan writes identical content to both files from the same source, ensuring byte-identity. The `cmp` acceptance check catches any drift. |

## Acceptance Criteria

- [ ] `pip install --dry-run -r worker/requirements/cpu-linux-agent.txt` exits 0
- [ ] `cmp worker/requirements/cpu-linux-agent.txt worker/requirements/cpu-runner-reqs.txt` exits 0 (files are byte-identical)
- [ ] `grep -c 'torch==2.12.1' worker/requirements/cpu-linux-agent.txt` outputs `1`
- [ ] `grep -c 'torch==2.12.1' worker/requirements/cpu-runner-reqs.txt` outputs `1`
- [ ] `grep -c 'download.pytorch.org/whl/cpu' worker/requirements/cpu-linux-agent.txt` outputs `1`
- [ ] `grep -c 'download.pytorch.org/whl/cpu' worker/requirements/cpu-runner-reqs.txt` outputs `1`
