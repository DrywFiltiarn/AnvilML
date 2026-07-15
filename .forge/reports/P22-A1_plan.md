# Plan Report: P22-A1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P22-A1                                      |
| Phase       | 22 — Qwen3 CLIP Arch Module                 |
| Description | worker/assets/qwen3_tokenizer/: vendored tokenizer + seeding script |
| Depends on  | P19-D1                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-07-15T09:55:00Z                        |
| Attempt     | 1                                           |

## Objective

Create the vendored Qwen3 tokenizer directory (`worker/assets/qwen3_tokenizer/`) containing the tokenizer files committed to git, and create cross-platform seeding scripts (`worker/tools/seed_tokenizers.sh` and `seed_tokenizers.ps1`) that can re-seed this directory from the canonical HuggingFace upstream source. This is the prerequisite for keeping the Python worker fully offline-capable for text encoding — all subsequent tasks in Phase 22 depend on these files being present.

## Scope

### In Scope
- Create `worker/assets/qwen3_tokenizer/` directory with at least one tokenizer file (the ACT agent resolves the full set during implementation).
- Create `worker/tools/seed_tokenizers.sh` — Bash script that re-seeds the tokenizer directory from the canonical upstream, with provenance reasoning as an inline comment.
- Create `worker/tools/seed_tokenizers.ps1` — PowerShell equivalent for Windows.
- Both scripts support `--dry-run` (or equivalent no-op flag) that exits 0 without making network calls.
- Both scripts are idempotent against an already-seeded directory.
- Provenance reasoning recorded as a comment in each script explaining: which repo/release is the canonical source, and why it is the correct source (not just a URL with no justification).

### Out of Scope
None. `defers_to (from JSON): []` — this task has no deferrals and implements its full scope.

## Existing Codebase Assessment

**What already exists:** The `worker/tests/fixtures/` directory exists with a well-documented convention (`README.md`) covering fixture sizing, naming, builder scripts, and the metadata-fallback regression case. The `scripts/` directory contains `install_worker_deps.sh` and `install_worker_deps.ps1` which establish the project's shell script conventions: `set -euo pipefail` (bash), argument parsing with `--mode=`, environment variable defaults, and idempotent behavior. The `worker/assets/` and `worker/tools/` directories do not yet exist — this task creates them.

**Established patterns:** Bash scripts use `set -euo pipefail`, argument parsing via case statements, environment variable defaults with `${VAR:-default}`, and clear usage/error messages to stderr. PowerShell scripts mirror this structure using `param()` blocks and `$ErrorActionPreference = 'Stop'`. Scripts include a header comment block describing purpose, usage, and environment variables. The project uses LF line endings for `.sh` files and CRLF for `.ps1` files (enforced by `.gitattributes`).

**Gap between design doc and source:** The design doc (§10.5) states the tokenizer directory and seed scripts must exist but does not specify which exact Qwen3 model variant's tokenizer to vendor (4B or 8B) or which specific files to include. This is intentionally deferred to ACT time for live resolution, as noted in the task context ("confirm at ACT time"). The ACT agent must resolve the canonical upstream source and file set during implementation.

## Resolved Dependencies

None. This task creates only data files (tokenizer assets) and shell scripts. No Rust crates, Python packages, or other external dependencies are introduced. The tokenizer files themselves are sourced from HuggingFace at ACT time via the seed scripts, not as a runtime dependency of this task.

## Approach

1. **Resolve the canonical upstream source (ACT time).** The ACT agent determines which Qwen3 model to use as the tokenizer source. The canonical choice is `Qwen/Qwen3-4B` on HuggingFace — it is the smallest Qwen3 variant, the tokenizer is shared across all Qwen3 variants (4B, 8B, 32B, 235B all use the same tokenizer vocabulary), and it is the first Qwen3 release. The ACT agent confirms the tokenizer files exist on this repo at the latest release tag. Record this choice in the script's provenance comment.

2. **Create `worker/assets/qwen3_tokenizer/` directory.** Create the directory with at least the core tokenizer files:
   - `tokenizer.json` — the main sentencepiece/BPE model file (contains vocab and merges combined)
   - `tokenizer_config.json` — configuration for the tokenizer (model class, special tokens, etc.)
   - `special_tokens_map.json` — special token mappings
   - `added_tokens.json` — any added tokens
   - `vocab.json` and `merges.txt` — if the tokenizer uses BPE with separate vocab/merge files (Qwen3's `Qwen2TokenizerFast` typically uses `tokenizer.json` as the primary file, but having the individual files is useful for inspection)
   
   The ACT agent copies these files from the resolved upstream repo. If the upstream repo does not contain all of these files individually, include only what is present.

3. **Create `worker/tools/seed_tokenizers.sh`.** Bash script following the established patterns from `scripts/install_worker_deps.sh`:
   - Header comment block: purpose, usage, environment variables, provenance reasoning.
   - `set -euo pipefail` at the top.
   - Parse `--dry-run` flag (and `--source=URL` flag for override).
   - If `--dry-run`: print what would be downloaded and exit 0 without network calls.
   - If not `--dry-run`: use `huggingface-cli download` to fetch the tokenizer files from the canonical source (`Qwen/Qwen3-4B`), placing them into `worker/assets/qwen3_tokenizer/`.
   - The provenance reasoning comment must explain: why `Qwen/Qwen3-4B` is the canonical source (tokenizer is shared across all Qwen3 variants; 4B is the smallest and most widely referenced variant; it is the official release from the Qwen team at Alibaba Group), and that this is not just a URL but the authoritative upstream.
   - Idempotent: if files already exist, overwrite them (or skip with a message — both approaches are acceptable; overwriting is simpler and guarantees freshness).

4. **Create `worker/tools/seed_tokenizers.ps1`.** PowerShell equivalent:
   - `param()` block for `--dry-run` and `--source` flags.
   - `$ErrorActionPreference = 'Stop'`.
   - Same structure as the Bash script: header comment, provenance reasoning, argument parsing, dry-run path, download path.
   - Use `huggingface-cli` (same tool as Bash, since it is a Python package available on both platforms).
   - CRLF line endings (enforced by `.gitattributes`).

5. **Verify acceptance criteria.** Run:
   - `test -d worker/assets/qwen3_tokenizer && ls worker/assets/qwen3_tokenizer | wc -l` — must show >=1.
   - `bash worker/tools/seed_tokenizers.sh --dry-run` — must exit 0 without network calls.

## Public API Surface

None. This task creates data files and shell scripts — no Rust pub items, no Python classes/functions, no library API surface.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/assets/qwen3_tokenizer/` | Directory for vendored Qwen3 tokenizer files |
| CREATE | `worker/assets/qwen3_tokenizer/tokenizer.json` | Main tokenizer model (vocab + merges) |
| CREATE | `worker/assets/qwen3_tokenizer/tokenizer_config.json` | Tokenizer configuration |
| CREATE | `worker/assets/qwen3_tokenizer/special_tokens_map.json` | Special token mappings |
| CREATE | `worker/assets/qwen3_tokenizer/added_tokens.json` | Added tokens (if present in upstream) |
| CREATE | `worker/assets/qwen3_tokenizer/vocab.json` | Vocabulary file (if present in upstream) |
| CREATE | `worker/assets/qwen3_tokenizer/merges.txt` | Merge rules file (if present in upstream) |
| CREATE | `worker/tools/seed_tokenizers.sh` | Bash re-seeding script with provenance reasoning |
| CREATE | `worker/tools/seed_tokenizers.ps1` | PowerShell re-seeding script with provenance reasoning |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| (shell check) | test_tokenizer_dir_exists | The vendored tokenizer directory exists and contains at least one file | `test -d worker/assets/qwen3_tokenizer && ls worker/assets/qwen3_tokenizer | wc -l` exits with value >= 1 |
| (shell check) | test_dry_run_no_network | The seed script's --dry-run flag exits 0 without making network calls | `bash worker/tools/seed_tokenizers.sh --dry-run` exits 0 |

No Rust tests or Python tests are needed — this task produces only data files and shell scripts.

## CI Impact

No CI changes required. The new files are data files and shell scripts already covered by the existing CI matrix:
- `rust-linux` and `rust-windows` jobs do not touch `worker/` files.
- `worker-linux-mock` and `worker-windows-mock` install `requirements/base.txt` (no torch) and run mock-mode tests — they do not import or execute tokenizer seed scripts.
- `worker-linux-real` and `worker-windows-real` install torch and run real-mode tests — they do not execute tokenizer seed scripts at runtime (the files are already committed).
- The `.gitattributes` rule (`*.sh / *.py / *.rs = LF; *.ps1 = CRLF`) already covers these new files.

## Platform Considerations

- **Bash script:** Uses `set -euo pipefail`, POSIX-compatible argument parsing via `case`. Runs on Linux (primary) and WSL2/Windows (via Git Bash or WSL).
- **PowerShell script:** Uses `param()` blocks, runs natively on Windows. The same `huggingface-cli` tool is used on both platforms (it is a Python package installed in the worker venv).
- **Line endings:** `.sh` files use LF (enforced by `.gitattributes`). `.ps1` files use CRLF (enforced by `.gitattributes`).
- **No `#[cfg(...)]` guards needed** — this task is purely data files and shell scripts, not Rust or Python code.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The ACT agent resolves a Qwen3 variant whose tokenizer files differ from what subsequent tasks expect (e.g., missing `vocab.json` or `merges.txt` when `qwen3.py`'s tokenizer loading code expects them). | Medium | High | The ACT agent should use `Qwen/Qwen3-4B` which is the canonical source and uses the same tokenizer as all other Qwen3 variants. The seed script downloads all available tokenizer files, and subsequent tasks in Phase 22 (P22-C1) will use `transformers.AutoTokenizer.from_pretrained(tokenizer_dir)` which handles any subset of available files. |
| `huggingface-cli` is not available on the target platform, causing the seed script to fail. | Low | Medium | The seed script should check for `huggingface-cli` availability and provide a clear error message with instructions to install it (`pip install huggingface_hub`). The `--dry-run` path does not require it. |
| Tokenizer files are large and slow to commit/download, causing CI or agent VM issues. | Low | Low | Qwen3 tokenizer files are text-based (JSON/TXT) and small — typically under 1 MB total. This is well within the fixture convention's <10 MB guideline. |

## Acceptance Criteria

- [ ] `test -d worker/assets/qwen3_tokenizer && ls worker/assets/qwen3_tokenizer | wc -l` outputs a number >= 1
- [ ] `bash worker/tools/seed_tokenizers.sh --dry-run` exits with code 0
- [ ] `bash worker/tools/seed_tokenizers.sh --dry-run` produces no network traffic (verified by the fact that `--dry-run` path does not invoke any download commands)
