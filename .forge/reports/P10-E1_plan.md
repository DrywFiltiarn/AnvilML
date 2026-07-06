# Plan Report: P10-E1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P10-E1                                        |
| Phase       | 10 — Generic Node Groundwork                  |
| Description | FORGE_TASK_AUTHORING_SPEC.md-style marker convention doc note |
| Depends on  | P10-D1                                        |
| Project     | anvilml                                       |
| Planned at  | 2026-07-06T09:00:00Z                          |
| Attempt     | 1                                             |

## Objective

Create `worker/nodes/MARKER_CONVENTION.md`, a short pointer-style documentation file
that documents the `REAL_PATH_VERIFIED`/`MOCK_PATH_VERIFIED` marker pair (defined in
`ANVILML_DESIGN.md §10.6`) as a quick-reference for the next phase's node-authoring
tasks. The file provides the exact comment-pair format and a one-line note that the
markers are mechanically checked by Gate 4 in `ENVIRONMENT.md`, avoiding duplication of
the full rule so the two documents do not drift apart.

## Scope

### In Scope
- Create `worker/nodes/MARKER_CONVENTION.md` containing:
  - A brief description of the marker convention (what the markers are, where they
    appear, and what they assert).
  - The exact comment-pair example from `ANVILML_DESIGN.md §10.6`:
    ```
    # REAL_PATH_VERIFIED: worker/tests/test_<module>.py::test_<name>_real_<fixture>
    # MOCK_PATH_VERIFIED: worker/tests/test_<module>.py::test_<name>_mock_<pattern>
    ```
    placed as a comment immediately above the class or function it verifies.
  - A one-line note that these markers are mechanically checked by Gate 4
    (`ENVIRONMENT.md §8`) — not just a convention — so future node authors know the
    markers are enforced, not optional.
  - A pointer to `ANVILML_DESIGN.md §10.6` for the full rule (the canonical source).

### Out of Scope
None. `defers_to (from JSON): []` — this task must implement its full scope. No
functionality is deferred to another task, and no stubs are permitted under an empty
`defers_to`. This is a pure documentation task with no source code changes.

## Existing Codebase Assessment

No prior source exists for `MARKER_CONVENTION.md` — this file is being created for the
first time. The marker convention itself is fully specified in `ANVILML_DESIGN.md §10.6`
and mechanically enforced by Gate 4 in `ENVIRONMENT.md §8`. The existing `worker/nodes/`
directory (created by Phase 10's earlier tasks) contains `base.py`, `__init__.py`, and
the three arch family `__init__.py` files (`diffusion/`, `clip/`, `vae/`), none of
which define concrete node `execute()` or arch module `load()`/`sample()`/`decode()`
functions yet — so no markers are present in the codebase at this phase. The convention
is being documented now so that when concrete nodes are authored in a later phase, the
quick-reference is already in place next to the files that will use it.

## Resolved Dependencies

None. This task creates a Markdown documentation file only. No external crates, Python
packages, or other dependencies are introduced or referenced.

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| (none) | (none)  | (n/a)           | (n/a)          | (n/a)                    |

## Approach

1. **Create `worker/nodes/MARKER_CONVENTION.md`** with the following structure:
   - A one-sentence header describing what the file documents.
   - The exact comment-pair format from `ANVILML_DESIGN.md §10.6`, shown as a code
     block with the `# REAL_PATH_VERIFIED:` and `# MOCK_PATH_VERIFIED:` lines, using
     the `<module>::<test_function>` placeholder pattern.
   - A brief note explaining placement: the markers go as comments immediately above
     the class or function they verify (node `execute()`, arch module `load()`/`sample()`/`decode()`).
   - A one-line note that Gate 4 in `ENVIRONMENT.md §8` mechanically validates these
     markers (checks that named tests exist and are collectible, and that every public
     function in scope carries both markers).
   - A pointer line: "Full rule: `ANVILML_DESIGN.md §10.6`."

2. **Verify the file** by running `test -s worker/nodes/MARKER_CONVENTION.md` — it must
   exit 0 (file exists and is non-empty).

No code changes, no source files modified, no tests written. This is a pure documentation
task.

## Public API Surface

None. This task creates a Markdown file only; no Python or Rust public API items are
introduced.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/nodes/MARKER_CONVENTION.md` | Marker convention quick-reference doc |

## Tests

None. This task creates a documentation file only. No source code is written, so no
tests are required. The acceptance criterion (`test -s worker/nodes/MARKER_CONVENTION.md`)
is a file-existence check, not a test.

## CI Impact

No CI changes required. A Markdown file under `worker/nodes/` is not picked up by any
existing CI job's file-type or test-module logic. The Rust CI jobs run no Python tests
against `worker/nodes/`, and the Python CI jobs do not lint or validate standalone
Markdown files.

## Platform Considerations

None identified. The file is a pure Markdown document with no platform-specific content,
path handling, or line-ending concerns. The Windows cross-check in `ENVIRONMENT.md §7`
is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The file content diverges from `ANVILML_DESIGN.md §10.6` over time, creating two sources of truth | Medium | Low | The file is intentionally short (pointer-style, not a duplicate) and includes a direct pointer to the canonical source. A future task that updates the convention should update both files in the same task. |
| The acceptance criterion (`test -s`) only checks non-empty, not content correctness | Low | Low | The plan specifies the exact content to include. The ACT agent writes the file per the Approach; a reviewer or the next phase's tasks will catch any content issues. |

## Acceptance Criteria

- [ ] `test -s worker/nodes/MARKER_CONVENTION.md` exits 0
