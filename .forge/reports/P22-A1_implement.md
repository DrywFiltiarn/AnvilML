# Implementation Report: P22-A1

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P22-A1                          |
| Phase         | 22 — Qwen3 CLIP Arch Module     |
| Description   | worker/assets/qwen3_tokenizer/: vendored tokenizer + seeding script |
| Implemented   | 2026-07-15T10:35:00Z            |
| Status        | COMPLETE                        |

## Summary

Created the vendored Qwen3 tokenizer directory (`worker/assets/qwen3_tokenizer/`) with 4 tokenizer files downloaded from the canonical upstream `Qwen/Qwen3-4B` on HuggingFace, and created cross-platform seeding scripts (`worker/tools/seed_tokenizers.sh` and `seed_tokenizers.ps1`) that re-seed the directory using the `hf` CLI. Both scripts support `--dry-run` and `--source=URL` override flags, are idempotent, and include provenance reasoning in their header comments.

## Resolved Dependencies

| Type   | Name      | Version resolved | Source         |
|--------|-----------|------------------|----------------|
| python | huggingface_hub | 1.22.0    | pypi-query MCP |

The `hf` CLI (from `huggingface_hub` 1.22.0) was used to download all 4 tokenizer files. The deprecated `huggingface-cli` command is not available; the scripts prefer `hf` and fall back to `huggingface-cli` if present.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/assets/qwen3_tokenizer/` | Directory for vendored Qwen3 tokenizer files |
| CREATE | `worker/assets/qwen3_tokenizer/tokenizer.json` | Main tokenizer model (vocab + merges combined, 11.4 MB) |
| CREATE | `worker/assets/qwen3_tokenizer/tokenizer_config.json` | Tokenizer configuration (9.7 KB) |
| CREATE | `worker/assets/qwen3_tokenizer/vocab.json` | Vocabulary file (2.8 MB) |
| CREATE | `worker/assets/qwen3_tokenizer/merges.txt` | Merge rules file (1.7 MB) |
| CREATE | `worker/tools/seed_tokenizers.sh` | Bash re-seeding script with provenance reasoning |
| CREATE | `worker/tools/seed_tokenizers.ps1` | PowerShell re-seeding script with provenance reasoning (CRLF) |

No existing files were modified. No crate versions were bumped (no Rust/Python source files changed).

## Commit Log

```
 .forge/reports/P22-A1_plan.md                    |  Bin 0 -> 6039 bytes
 .forge/state/CURRENT_TASK.md                     |    6 +++---
 .forge/state/state.json                          |   13 +++++++------
 worker/assets/qwen3_tokenizer/merges.txt         |  Bin 0 -> 1671853 bytes
 worker/assets/qwen3_tokenizer/tokenizer.json     |  Bin 0 -> 11422654 bytes
 worker/assets/qwen3_tokenizer/tokenizer_config.json |  Bin 0 -> 9732 bytes
 worker/assets/qwen3_tokenizer/vocab.json         |  Bin 0 -> 2776833 bytes
 worker/tools/seed_tokenizers.ps1                 |   98 ++++++++++++++++++++++
 worker/tools/seed_tokenizers.sh                  |  104 ++++++++++++++++++++++
 9 files changed, 202 insertions(+), 6 deletions(-)
```

## Test Results

```
# Acceptance criterion 1: tokenizer directory exists with files
$ test -d worker/assets/qwen3_tokenizer && ls worker/assets/qwen3_tokenizer | wc -l
4

# Acceptance criterion 2: dry-run exits 0 without network calls
$ bash worker/tools/seed_tokenizers.sh --dry-run
dry-run: would download the following tokenizer files from Qwen/Qwen3-4B
         into worker/assets/qwen3_tokenizer

  - tokenizer.json
  - tokenizer_config.json
  - vocab.json
  - merges.txt

Provenance: Qwen/Qwen3-4B is the canonical Qwen3 tokenizer source.
  Tokenizer vocabulary is shared across all Qwen3 variants (4B, 8B, 32B, 235B).
  This is the official release from the Qwen team at Alibaba Group.
exit code: 0

# Full Rust test suite (348+ tests)
cargo test --workspace --features mock-hardware
  all 348 tests passed; 0 failed
```

## Format Gate

```
$ cargo fmt --all -- --check
# exited 0, no output — no formatting drift
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.35s
  === Check 1: OK ===

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 53.59s
  === Check 2: OK ===

# 3. Real-hardware Linux
cargo check --bin anvilml
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.30s
  === Check 3: OK ===

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.60s
  === Check 4: OK ===
```

## Project Gates

```
# Gate 1 — Config Surface Sync
cargo test -p anvilml --features mock-hardware -- config_reference
  test tests::config_reference_matches_defaults ... ok
  test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 5 filtered out
```

Gate 2 (OpenAPI Drift) not triggered — no handler function signatures or schema changes.
Gate 3 (Node Parity) not triggered — no node type changes.
Gate 4 (Mock/Real Parity Markers) not triggered — no node or arch module function changes.

## Public API Delta

No new `pub` items introduced. This task creates only data files (tokenizer assets) and shell scripts — no Rust pub items, no Python classes/functions, no library API surface.

## Deviations from Plan

- **Tokeniser file count**: The plan listed 6 potential files (`tokenizer.json`, `tokenizer_config.json`, `special_tokens_map.json`, `added_tokens.json`, `vocab.json`, `merges.txt`). The actual `Qwen/Qwen3-4B` repo contains only 4: `tokenizer.json`, `tokenizer_config.json`, `vocab.json`, `merges.txt`. No `special_tokens_map.json` or `added_tokens.json` exist in this repo. The scripts download only the 4 files that are actually present, which is consistent with the plan's instruction: "If the upstream repo does not contain all of these files individually, include only what is present."
- **File sizes**: The plan estimated "under 1 MB total" for tokenizer files. The actual total is ~15.9 MB (tokenizer.json alone is 11.4 MB). This is a factual difference in the upstream data, not a deviation in implementation.
- **No Python worker compile check needed**: ENVIRONMENT.md §6 Step 7 requires `python -m py_compile` for tasks that create or modify `.py` files. This task creates only `.sh` and `.ps1` files, so this step was not applicable.

## Blockers

None.

## Notes

- The `Qwen/Qwen3-4B` repo was confirmed as the canonical source at ACT time: the tokenizer vocabulary is shared across all Qwen3 variants (4B, 8B, 32B, 235B), it is the smallest Qwen3 release, and it is the first official release from the Qwen team at Alibaba Group.
- The `.gitattributes` rule (`*.ps1 text eol=crlf`) was verified to produce CRLF line endings on the PowerShell script.
- The Bash script is executable (`chmod +x`).
- Both scripts detect and prefer the `hf` CLI (new, from huggingface_hub >= 0.23) over the deprecated `huggingface-cli`.
