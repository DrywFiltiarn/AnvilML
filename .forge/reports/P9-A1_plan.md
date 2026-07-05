# Plan Report: P9-A1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P9-A1                                             |
| Phase       | 9 — Real Worker Startup                           |
| Description | worker/: requirements/base.txt (no torch, core deps only) |
| Depends on  | P8-H1                                             |
| Project     | anvilml                                           |
| Planned at  | 2026-07-05T10:58:00Z                              |
| Attempt     | 1                                                 |

## Objective

Create the worker's core dependency manifest (`worker/requirements/base.txt`) with the one absolute constraint that makes the mock-mode CI jobs possible: `torch` must never appear in this file. Also create two empty placeholder files (`cpu-linux-agent.txt` and `cpu-runner-reqs.txt`) for later torch CPU wheel pins. Every package version is resolved live via the PyPI MCP registry, not from memory.

## Scope

### In Scope
- Update `worker/requirements/base.txt` with MCP-resolved versions for: diffusers, transformers, safetensors, Pillow, msgpack, pyzmq, pytest. No `torch` anywhere in this file.
- Create `worker/requirements/cpu-linux-agent.txt` as an empty placeholder file (singular name per ANVILML_DESIGN.md §3.1).
- Create `worker/requirements/cpu-runner-reqs.txt` as an empty placeholder file.

### Out of Scope
- Populating the `cpu-*` placeholder files with torch pins (that is P9-A2's scope).
- Creating or modifying any other requirements files (`cuda.txt`, `rocm-linux.txt`, `rocm-windows.txt`) — those already exist and are out of scope.
- Creating any Python source files under `worker/` — those are later Phase 9 tasks.
- Registering the `real_mode` pytest marker — that is P9-A3's scope.

## Existing Codebase Assessment

No prior source exists for `base.txt`'s content beyond a pre-existing file at `worker/requirements/base.txt` with stale version pins (`>=27.0`, `>=1.2`, `>=12.2`, `>=0.8`, `>=0.38.0`, `>=5.12`, `>=9.1`) that were not resolved via live MCP lookup. The file already correctly excludes `torch`, which is the one constraint this task must maintain.

The directory `worker/requirements/` already contains `cpu-runner-reqs.txt` (with torch pins) and `cpu-linux-agents.txt` (with torch pins, note the plural "agents" — a likely typo; the design doc at ANVILML_DESIGN.md §3.1 uses the singular `cpu-linux-agent.txt`). The plan addresses this by creating the correctly-named `cpu-linux-agent.txt` (singular) as an empty file and clearing `cpu-runner-reqs.txt` to be empty per the task's explicit instruction to create it as a placeholder.

The `install_worker_deps.sh` script already references `worker/requirements/base.txt` for pip install, confirming this is the file the provisioning system consumes.

No established patterns for requirements files exist beyond the one constraint: `torch` must never appear in `base.txt` (ANVILML_DESIGN.md §18.6).

## Resolved Dependencies

| Type   | Name        | Version verified | MCP source     | Feature flags confirmed |
|--------|-------------|-----------------|----------------|------------------------|
| python | diffusers   | 0.39.0          | pypi-query MCP | n/a                    |
| python | transformers| 5.13.0          | pypi-query MCP | n/a                    |
| python | safetensors | 0.8.0           | pypi-query MCP | n/a                    |
| python | Pillow      | 12.3.0          | pypi-query MCP | n/a                    |
| python | msgpack     | 1.2.1           | pypi-query MCP | n/a                    |
| python | pyzmq       | 27.1.0          | pypi-query MCP | n/a                    |
| python | pytest      | 9.1.1           | pypi-query MCP | n/a                    |

All versions were resolved via the `pypi-query_get_package_info` MCP tool at planning time. The existing `base.txt` used `>=` constraints with stale lower bounds — the plan updates these to the current versions (using `==` pins, which is the standard practice for reproducible CI installs; if `>=` is preferred, the ACT agent may adjust to `>=<version>` but should not pin below the MCP-resolved version).

Note: The task context mentions `pytest` as a core dependency. While pytest is typically a dev dependency, the task explicitly lists it in `base.txt`. This is intentional — the mock-mode CI jobs (`worker-linux-mock`, `worker-windows-mock`) install `base.txt` and then run `pytest` to execute mock-mode tests, so pytest must be present in the base install.

## Approach

1. **Write `worker/requirements/base.txt`** with one package per line, using `==` pins at the MCP-resolved versions:
   ```
   diffusers==0.39.0
   transformers==5.13.0
   safetensors==0.8.0
   Pillow==12.3.0
   msgpack==1.2.1
   pyzmq==27.1.0
   pytest==9.1.1
   ```
   No `torch`, no `--index-url`, no comments. Seven lines total, one per package. The order is alphabetical by package name for consistency.

2. **Create `worker/requirements/cpu-linux-agent.txt`** as an empty file (zero bytes). This is the correctly-named file per ANVILML_DESIGN.md §3.1 (singular "agent"). The existing `cpu-linux-agents.txt` (plural) is a different file, likely a typo from a prior phase; it is outside this task's scope to rename or delete it — the ACT agent should note its existence but only create the singular-named file.

3. **Clear `worker/requirements/cpu-runner-reqs.txt`** to be empty (zero bytes). The task explicitly states to create it as a "currently-empty placeholder file." The existing content (torch pins with `--index-url`) must be removed per the task instruction: "Create worker/requirements/cpu-linux-agent.txt and cpu-runner-reqs.txt as separate, currently-empty placeholder files (torch CPU wheel pins are added by a later task once real-mode tests exist to justify them — do not pre-pin torch speculatively in this task)."

4. **Verify** that `base.txt` contains no occurrence of `torch` (case-insensitive grep).

defers_to (from JSON): []

## Public API Surface

None. This task creates configuration/manifest files only — no Python modules, no Rust crates, no public APIs.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | worker/requirements/base.txt | Update with MCP-resolved version pins; ensure no torch |
| CREATE | worker/requirements/cpu-linux-agent.txt | Empty placeholder (0 bytes) per design doc naming |
| MODIFY | worker/requirements/cpu-runner-reqs.txt | Clear to empty placeholder (0 bytes) |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| (manual) | base_no_torch | base.txt contains no occurrence of "torch" | `grep -i torch worker/requirements/base.txt; echo $?` exits 1 (no match) |
| (manual) | base_dry_run | `pip install --dry-run -r base.txt` resolves successfully with no torch index | `pip install --dry-run -r worker/requirements/base.txt` exits 0 |
| (manual) | cpu_files_empty | Both placeholder files are empty (0 bytes) | `wc -c worker/requirements/cpu-linux-agent.txt worker/requirements/cpu-runner-reqs.txt` shows `0` for both |

## CI Impact

No CI changes required. The mock-mode CI jobs (`worker-linux-mock`, `worker-windows-mock`) already install `base.txt` and run the mock test suite. Updating version pins in `base.txt` does not change CI job structure — only the installed versions change. The `worker-*-real` jobs install `base.txt` first, then `cpu-runner-reqs.txt` (which will be populated by P9-A2 with torch pins).

## Platform Considerations

None identified. The requirements files are platform-neutral — they install the same packages on Linux and Windows. The `pyzmq` package builds from source on platforms without pre-built wheels but does not require GPU drivers or torch. This is the exact property that enables the mock CI jobs to run.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Pinning exact versions (`==`) may be too rigid for downstream consumers who want flexibility. Some packages in the ecosystem use `>=` constraints. | Low | Low | The plan uses `==` for reproducibility (CI determinism). If the ACT agent or reviewer prefers `>=` constraints, they may change to `>=<version>` but must not pin below the MCP-resolved version. |
| The existing `cpu-linux-agents.txt` (plural) file is not addressed by this task, creating a naming inconsistency with the design doc's singular `cpu-linux-agent.txt`. | High | Low | The plan explicitly creates the correctly-named singular file. The plural file is a pre-existing artifact from a prior phase; renaming or deleting it is outside this task's scope and should be noted for a follow-up cleanup task. |
| `transformers==5.13.0` may have transitive dependencies that pull in `torch` (e.g. via optional extras). | Low | Medium | The MCP-resolved `transformers` 5.13.0 declares `torch` only under the `torch` extra (`torch>=2.4; extra == "torch"`), which is not activated by default. A dry-run install without extras will not pull in torch. The acceptance criterion `pip install --dry-run` will confirm this. |

## Acceptance Criteria

- [ ] `grep -i torch worker/requirements/base.txt` exits 1 (no match found)
- [ ] `pip install --dry-run -r worker/requirements/base.txt` exits 0
- [ ] `test -s worker/requirements/cpu-linux-agent.txt` fails (file exists but is empty)
- [ ] `test -s worker/requirements/cpu-runner-reqs.txt` fails (file exists but is empty)
- [ ] `wc -c worker/requirements/cpu-linux-agent.txt worker/requirements/cpu-runner-reqs.txt` shows `0` bytes for both files
