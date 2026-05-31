# Implementation Report: P0-A2

| Field          | Value                                                       |
|----------------|-------------------------------------------------------------|
| Task ID        | P0-A2                                                       |
| Phase          | 000 — Repository Preamble                                   |
| Description    | anvilml: .gitattributes enforcing cross-platform line endings |
| Project        | anvilml                                                     |
| Implemented at | 2026-05-31T21:09:58Z                                        |
| Attempt        | 1                                                           |

## Summary

Created `.gitattributes` at the repository root (`AnvilML/.gitattributes`) to enforce consistent line endings across Linux and Windows checkouts. The file uses `* text=auto` as a catch-all normalisation rule (Git stores LF on commit, converts on checkout per-platform), then explicitly overrides line-ending behaviour per-extension: LF for source code and data files (`.rs`, `.py`, `.sh`, `.toml`, `.md`, `.json`, `.yml`), CRLF for Windows-native PowerShell scripts (`.ps1`), and `binary` mode for known binary asset types (`.png`, `.safetensors`, `.ckpt`). The file is structured with section comments for readability and has no trailing blank lines at EOF. This prevents CRLF/LF drift that previously caused rustfmt failures on Linux when files were committed from Windows (or vice versa), and prevents shell scripts from executing correctly due to embedded carriage-return characters.

## Files Changed

| Action   | Path                              | Description                                                       |
|----------|-----------------------------------|-------------------------------------------------------------------|
| CREATE   | `.gitattributes`                  | Repository gitattributes enforcing cross-platform line endings     |
| MODIFY   | `.forge/state/CURRENT_TASK.md`    | Updated by Forge orchestrator                                      |

## Test Results

No source code exists in this Phase 0 task. The format, lint, cross-check, and test suite steps from the instructions are not applicable (no Rust crates, no Python packages). Only a configuration file was created.

```
No test output — config-only task
```

## CI Changes

No CI changes made.

## Commit Log

```
A  .forge/reports/P0-A2_plan.md
M  .forge/state/CURRENT_TASK.md
M  .forge/state/state.json
A  .gitattributes
```

## Acceptance Criteria — Verification

| Criterion                                                       | Status | Evidence                                                |
|-----------------------------------------------------------------|--------|---------------------------------------------------------|
| `.gitattributes` created at repository root                     | PASS   | File exists at `AnvilML/.gitattributes`                 |
| `* text=auto` as first explicit normalisation rule              | PASS   | Line 3 in `.gitattributes`                              |
| `eol=lf` for `*.rs`                                             | PASS   | Line 6 in `.gitattributes`                              |
| `eol=lf` for `*.py`                                             | PASS   | Line 7 in `.gitattributes`                              |
| `eol=lf` for `*.sh`                                             | PASS   | Line 8 in `.gitattributes`                              |
| `eol=lf` for `*.toml`                                           | PASS   | Line 11 in `.gitattributes`                             |
| `eol=lf` for `*.json`                                           | PASS   | Line 12 in `.gitattributes`                             |
| `eol=lf` for `*.yml`                                            | PASS   | Line 13 in `.gitattributes`                             |
| `eol=lf` for `*.md`                                             | PASS   | Line 16 in `.gitattributes`                             |
| `eol=crlf` for `*.ps1`                                          | PASS   | Line 19 in `.gitattributes`                             |
| `binary` for `*.png`                                            | PASS   | Line 22 in `.gitattributes`                             |
| `binary` for `*.safetensors`                                    | PASS   | Line 23 in `.gitattributes`                             |
| `binary` for `*.ckpt`                                           | PASS   | Line 24 in `.gitattributes`                             |
| File structured with section comments for readability           | PASS   | Section headers present (Source code, Config & data, Documentation, Windows-native scripts, Binary assets) |
| No trailing blank lines at EOF                                  | PASS   | `tail -c 20 .gitattributes | xxd` confirms file ends with `*.ckpt binary\n` |
| Files staged with `git add -A`                                  | PASS   | `git status --short` shows all files under "Changes to be committed" |
