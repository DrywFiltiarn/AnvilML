# CONVENTIONS.md — SindriStudio Forge Orchestrator
<!-- ═══════════════════════════════════════════════════════════════════════════
     This file governs ALL aider sessions invoked by the Forge orchestrator.
     Loaded read-only via --read CONVENTIONS.md on every invocation.
     Rules are absolute. Deviations will break the orchestration pipeline.
     ═══════════════════════════════════════════════════════════════════════════ -->


## 0. IDENTITY AND ROLE

The agent is an implementation agent. It does not make project-level decisions.
It executes exactly what The Forge assigns: plan OR implement, never both in
one session. The Forge owns git, Discord, and all approval gates.

Permitted output types:
- PLAN session  → exactly one markdown report file, then STOP
- ACT session   → source code, tests, CI updates, one report file, local
                  git stages in the project repo, then STOP

The agent MUST NEVER:
- Commit or push to any repository — git is exclusively The Forge's domain
- Send messages to Discord
- Edit forge.py, state.json, or any file under forge/tasks/
- Delete or rename report files already written
- Exceed the scope of the current task as defined in the task context
- Access the internet or fetch any URL — dependency versions are pre-resolved
  and injected into the prompt by The Forge (see §8)


## 1. TASK IDENTIFICATION

Every session begins with a structured header injected by The Forge:

```
SindriStudio Task: <TASK_ID>
Description: <description>
Phase: <NNN>
Project: <name>
```

TASK_ID format:  `P<phase>-<letter><number>`   e.g. P1-A3, P2-B7

On session start, the agent MUST:
1. Read `.forge/state/CURRENT_TASK.md`
2. Confirm the Task field matches the injected TASK_ID
3. If mismatch: write a one-line error to
   `.forge/reports/<TASK_ID>_plan.md` and STOP
4. Read `docs/ENVIRONMENT.md`, `docs/ARCHITECTURE.md`, and the relevant
   `docs/TASKS_PHASE<NNN>.md` before writing a single line of output


## 2. SESSION MODES

The active mode is determined by the prompt header injected by The Forge:
- PLAN sessions begin with `Instructions — PLAN SESSION ONLY:`
- ACT sessions begin with `Instructions — ACT SESSION:`

The agent MUST NOT infer or switch modes on its own.

---

### MODE A: PLAN

**Goal:** produce the plan report. Nothing else.

**Permitted actions:**
- Read any file in the repository
- Write `.forge/reports/<TASK_ID>_plan.md`  (the only permitted write)
- Update `.forge/state/CURRENT_TASK.md`

**Forbidden actions:**
- Writing any source code, test, config, or CI file
- Running compilers, test runners, linters, or any build tool
- Any git operation
- Any network access (dependency versions are already in the prompt — see §8)

**Termination:** After writing the report and updating CURRENT_TASK.md — STOP.

---

### MODE B: ACT

**Goal:** implement the approved plan, run tests to zero failures, stage
changes (`git add -A`), write the implementation report — then STOP.

**Permitted actions:**
- Read any file in the repository
- Write/modify source, test, and CI files within the task's project repo
- Run build tools, compilers, test runners, linters
- `git add -A` inside the project repo (STAGE ONLY — do not commit)
- Write `.forge/reports/<TASK_ID>_implement.md`
- Update `.forge/state/CURRENT_TASK.md`

**Forbidden actions:**
- `git commit` — The Forge is the sole author of all commits
- `git push` — The Forge pushes after push approval
- Any git operation outside the task's project repo
- Deviating from the approved plan
- Any network access


## 3. PLAN REPORT FORMAT

**Output path:** `.forge/reports/<TASK_ID>_plan.md`

**SINGLE WRITE RULE:** Write the complete, finished document in a single
write operation. Do NOT create the file empty and fill it incrementally.
Do NOT write notes, progress updates, or partial drafts to this file.
The file must not exist on disk until the complete plan is ready to write.
The first line must be exactly `# Plan Report: <TASK_ID>`.

**PRE-STOP CHECK — verify before updating CURRENT_TASK.md:**
- [ ] First line is exactly `# Plan Report: <TASK_ID>`
- [ ] Header table with all seven fields present
- [ ] `## Objective` present
- [ ] `## Scope` with `### In Scope` and `### Out of Scope`
- [ ] `## Approach` with numbered steps
- [ ] `## Files Affected` table
- [ ] `## Tests` table (or "None.")
- [ ] `## CI Impact` present
- [ ] `## Risks and Mitigations` table
- [ ] `## Acceptance Criteria` checklist

---

```markdown
# Plan Report: <TASK_ID>

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | <TASK_ID>                                   |
| Phase       | <NNN> — <Phase Name>                        |
| Description | <task description>                          |
| Depends on  | <comma-separated prereq IDs or "none">      |
| Project     | <project name>                              |
| Planned at  | <ISO 8601 UTC timestamp>                    |
| Attempt     | <integer>                                   |

## Objective

One paragraph. What this task achieves and why it is needed.

## Scope

### In Scope
- <item>

### Out of Scope
- <item>

## Approach

1. <step>
2. <step>

## Files Affected

| Action   | Path                              | Description            |
|----------|-----------------------------------|------------------------|
| CREATE   | <path>                            | <what it contains>     |
| MODIFY   | <path>                            | <what changes>         |

## Tests

| Test ID / Name            | File                     | Validates               |
|---------------------------|--------------------------|-------------------------|
| <name>                    | <path>                   | <behaviour>             |

## CI Impact

Describe any changes to .github/workflows/ci.yml required.
If no changes: "No CI changes required."

## Risks and Mitigations

| Risk                      | Likelihood | Impact | Mitigation              |
|---------------------------|-----------|--------|-------------------------|
| <risk>                    | Low/Med/Hi | L/M/H  | <mitigation>            |

## Acceptance Criteria

- [ ] <verifiable criterion with exact command>
```


## 4. IMPLEMENTATION REPORT FORMAT

**Output path:** `.forge/reports/<TASK_ID>_implement.md`

**SINGLE WRITE RULE:** Same as §3. Write once, complete, after all tests pass
and files are staged. First line must be `# Implementation Report: <TASK_ID>`.

**SECTION DISTINCTION — `## Files Changed` vs `## Commit Log`:**
Both are always required. They serve different purposes and are NOT interchangeable.
- `## Files Changed` — static table of what this task touched (Action/Path/Description)
- `## Commit Log` — raw output of `git status --short` in a fenced code block

**PRE-STOP CHECK:**
- [ ] Header table with all six fields
- [ ] `## Summary` present
- [ ] `## Files Changed` — table (NOT git output)
- [ ] `## Test Results` — verbatim test runner output in a code block
- [ ] `## CI Changes` — present (or "No CI changes made.")
- [ ] `## Commit Log` — raw `git status --short` in a code block (NOT a table)
- [ ] `## Acceptance Criteria — Verification` table with PASS/FAIL for every criterion

---

```markdown
# Implementation Report: <TASK_ID>

| Field          | Value                                       |
|----------------|---------------------------------------------|
| Task ID        | <TASK_ID>                                   |
| Phase          | <NNN> — <Phase Name>                        |
| Description    | <task description>                          |
| Project        | <project name>                              |
| Implemented at | <ISO 8601 UTC timestamp>                    |
| Attempt        | <integer>                                   |

## Summary

One paragraph. What was implemented and any notable decisions.

## Files Changed

| Action   | Path                              | Description            |
|----------|-----------------------------------|------------------------|
| CREATE   | <path>                            | <what it contains>     |
| MODIFY   | <path>                            | <what changed>         |

## Test Results

```
<test runner output — verbatim, showing 0 failures>
```

## CI Changes

List every change to CI workflow files. If none: "No CI changes made."

## Commit Log

```
<git status --short output>
```

## Acceptance Criteria — Verification

| Criterion                 | Status | Evidence                        |
|---------------------------|--------|---------------------------------|
| <criterion from plan>     | PASS   | `cargo test -p anvilml-core`    |
```


## 5. GIT RULES

**5.1** NEVER run `git commit`. The Forge commits exclusively.  
**5.2** NEVER run `git push`. The Forge pushes after approval.  
**5.3** NEVER perform any git operation outside the task's project repo.  
**5.4** Do not create, delete, or rename branches.  
**5.5** Do not amend, rebase, or force-push any commit.  
**5.6** Do not modify `.gitmodules` or CI workflow files unless explicitly
        listed in the plan's "Files Affected" table.  
**5.7** Stage only: `git add -A` inside the project repo, after all tests pass.


## 6. TASK ATOMICITY RULES

**6.1** Implement exactly what is in the plan's "In Scope" section — no more.  
**6.2** Do not refactor code outside "Files Affected" unless a test requires it.  
**6.3** Do not upgrade dependencies unless the task explicitly requires it.  
**6.4** Do not modify unrelated tests. Do not delete tests.  
**6.5** If a prerequisite task's output is missing, write a `## Blockers` section
        and STOP. Do not attempt to compensate.


## 7. TEST AND CI REQUIREMENTS

**7.1** Every task that writes source code MUST include tests.

**7.2** The test suite for the affected crate/package must exit 0 before
        writing the implementation report.

**7.3** The full workspace suite must exit 0. Fix any regressions.

**7.4** Test file naming:
- Rust: unit tests in same file (`mod tests {}`), integration tests in
  `tests/<crate_name>_<feature>.rs`
- Python: `tests/test_<module>.py`

**7.5** WINDOWS CROSS-CHECK (prevents Linux-passes / Windows-breaks drift):

```
cargo check --target x86_64-pc-windows-gnu --workspace --features mock-hardware
```

The `x86_64-pc-windows-gnu` target and `gcc-mingw-w64` linker are installed
locally. Zero errors required before writing the report. A clean Linux build
is NOT sufficient — this must also pass.

**7.6** CONFIG DRIFT GATE: If a task adds/renames/removes any `ServerConfig`
field, it MUST also update `./anvilml.toml` and `docs/ENVIRONMENT.md §2` in
the same task. The `config_reference` test enforces this. Fix `anvilml.toml`
rather than weakening the test.

**7.7** If tests fail after implementation, fix them before writing the report.


## 8. DEPENDENCY VERSIONS

The Forge resolves current Rust crate and Python package versions before each
session and injects them into the prompt under `## Current Dependency Versions`.

**8.1** Use ONLY the versions listed in that section for any Cargo.toml or
        requirements file entries. Do not assume any version not listed there.

**8.2** Do not fetch any URL or query any registry yourself — version data is
        already in the prompt. Network access is not available and not needed.

**8.3** If a listed version appears incompatible with the task requirements,
        document the conflict under `## Dependency Notes` in the report,
        set `Status=BLOCKED` in CURRENT_TASK.md, and STOP. Do not invent
        a workaround using an unlisted version.

**8.4** If `## Current Dependency Versions` is absent from the prompt, proceed
        using only versions already present in the existing Cargo.toml and
        requirements files. Do not introduce new version pins.


## 9. CONTEXT WINDOW MANAGEMENT

**9.1** Do NOT emit reasoning traces or internal planning notes to any output
        file. All written output must be the final clean report.

**9.2** SINGLE WRITE RULE (see §3 and §4). Never write partial or incremental
        content to report files.

**9.3** PARTIAL fallback: If an unrecoverable resource constraint prevents
        completing the full task, the agent MUST:
        - Finish the current file or function
        - Run tests, stage changes (`git add -A`)
        - Write a partial report with a `## Continuation` section listing what
          remains
        - Update CURRENT_TASK.md with `Status=PARTIAL`
        - STOP. The Forge will resume in a fresh session.

**9.4** Output structure discipline (35B A3B): This model variant tends to
        abbreviate report sections on simple tasks. This is never acceptable —
        report structure is fixed regardless of task size. Use the PRE-STOP
        CHECKs in §3 and §4 as a mechanical gate before stopping. Specific
        patterns to avoid:
        - Omitting `## Files Changed` because `## Commit Log` is present
        - Writing prose instead of the header table
        - Skipping `## Risks and Mitigations` with "no risks identified"
          (write the table with at least one row)
        - Writing `## Test Results` as a sentence rather than verbatim output


## 10. PROHIBITED BEHAVIOURS

**10.1** No `git commit`, `git push`, or any remote write.  
**10.2** No modifications to `forge.py`, `state.json`, or `forge/tasks/`.  
**10.3** No modifications to files outside the task's `project` repo.  
**10.4** No use of undocumented environment variables or API keys.  
**10.5** No network access of any kind (registries, docs, search, etc.).  
**10.6** No interactive prompts. All commands must be non-interactive (`-y`,
         `--yes`, `--non-interactive` where applicable).  
**10.7** No background processes that outlive the session.  
**10.8** No modifications to `.env` files unless the plan's "Files Affected"
         table explicitly lists a `.env.example` change.  
**10.9** No self-modification: treat `CONVENTIONS.md` as read-only.


## 11. ERROR HANDLING AND STOPPING

**11.1** Unrecoverable errors (missing prereq output, env misconfiguration,
         unresolvable build failure):
         a. Write `## Blockers` to the in-progress report file
         b. Update CURRENT_TASK.md with `Status=BLOCKED`
         c. STOP — do not guess, retry indefinitely, or work around

**11.2** Build failures caused by code written in this session are not blockers;
         fix them before writing the report.

**11.3** Pre-existing build failures not introduced by this task ARE blockers.

**11.4** Flaky tests: document in `## Test Results`, ensure the final run
         shows 0 failures.


## 12. FILE AND PATH CONVENTIONS

**12.1** Report files (inside the TARGET REPOSITORY):
```
.forge/reports/<TASK_ID>_plan.md
.forge/reports/<TASK_ID>_implement.md
```

**12.2** State file (inside the TARGET REPOSITORY):
```
.forge/state/CURRENT_TASK.md
```
Format:
```
Task: <TASK_ID>
Step: <PLAN|IMPLEMENT>
Status: <COMPLETE|PARTIAL|BLOCKED>
Updated: <ISO 8601 UTC>
```

**12.3** The `.forge/` directory is dot-prefixed (hidden):
- Correct:   `.forge/reports/P1-A1_plan.md`
- Incorrect: `forge/reports/P1-A1_plan.md`

**12.4** Phase documents: `docs/TASKS_PHASE<NNN>.md` — read-only to the agent.

**12.5** Task JSON files: `.forge/tasks/tasks_phase<NNN>.json` — read-only.

<!-- ── END OF CONVENTIONS.md ──────────────────────────────────────────────── -->
