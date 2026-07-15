#!/usr/bin/env bash
# seed_tokenizers.sh — Re-seed the Qwen3 tokenizer directory from the canonical
# HuggingFace upstream source.
#
# Usage:
#   bash worker/tools/seed_tokenizers.sh                    # download to worker/assets/qwen3_tokenizer/
#   bash worker/tools/seed_tokenizers.sh --dry-run          # print what would be downloaded, no network calls
#   bash worker/tools/seed_tokenizers.sh --source=REPO      # use a different HuggingFace repo (default: Qwen/Qwen3-4B)
#
# Environment variables:
#   HF_TOKEN  HuggingFace authentication token (optional — unauthenticated
#             requests work but are rate-limited; set this for higher rate
#             limits and faster downloads).
#
# Provenance — why Qwen/Qwen3-4B is the canonical source:
#   - The tokenizer vocabulary is shared across ALL Qwen3 variants (4B, 8B, 32B,
#     235B). They all use the identical tokenizer files.
#   - Qwen/Qwen3-4B is the smallest Qwen3 release and the first official release
#     from the Qwen team at Alibaba Group.
#   - This is the authoritative upstream; the tokeniser is not a third-party
#     fork or community conversion.
#
# This script is idempotent: if files already exist in the output directory,
# they are overwritten to guarantee freshness.
#
# Requires: `hf` CLI (huggingface_hub >= 0.23) or `huggingface-cli` on PATH.

set -euo pipefail

# ── Defaults ──────────────────────────────────────────────────────────────────
SOURCE="Qwen/Qwen3-4B"
DRY_RUN=false
OUTPUT_DIR="worker/assets/qwen3_tokenizer"

# Tokenizer files to download from the upstream repo.
# These are the exact files present in the Qwen/Qwen3-4B repo that constitute
# the complete tokenizer — no special_tokens_map.json or added_tokens.json
# exist in this repo, so they are not requested.
TOKENIZER_FILES=(
    tokenizer.json
    tokenizer_config.json
    vocab.json
    merges.txt
)

# ── Parse arguments ───────────────────────────────────────────────────────────
for arg in "$@"; do
    case "$arg" in
        --dry-run)
            DRY_RUN=true
            ;;
        --source=*)
            SOURCE="${arg#--source=}"
            ;;
        *)
            echo "error: unrecognized argument: $arg" >&2
            echo "usage: bash worker/tools/seed_tokenizers.sh [--dry-run] [--source=REPO]" >&2
            exit 1
            ;;
    esac
done

# ── Dry-run path ──────────────────────────────────────────────────────────────
if $DRY_RUN; then
    echo "dry-run: would download the following tokenizer files from $SOURCE"
    echo "         into $OUTPUT_DIR"
    echo ""
    for f in "${TOKENIZER_FILES[@]}"; do
        echo "  - $f"
    done
    echo ""
    echo "Provenance: Qwen/Qwen3-4B is the canonical Qwen3 tokenizer source."
    echo "  Tokenizer vocabulary is shared across all Qwen3 variants (4B, 8B, 32B, 235B)."
    echo "  This is the official release from the Qwen team at Alibaba Group."
    exit 0
fi

# ── Resolve CLI tool ─────────────────────────────────────────────────────────
# Prefer `hf` (the new CLI from huggingface_hub >= 0.23) over the deprecated
# `huggingface-cli`. Both are Python packages available on any platform.
if command -v hf >/dev/null 2>&1; then
    HF_CLI="hf"
elif command -v huggingface-cli >/dev/null 2>&1; then
    HF_CLI="huggingface-cli"
else
    echo "error: neither 'hf' nor 'huggingface-cli' found on PATH" >&2
    echo "install with: pip install huggingface_hub" >&2
    exit 1
fi

# ── Download ──────────────────────────────────────────────────────────────────
echo "downloading tokenizer files from $SOURCE into $OUTPUT_DIR"

# Create output directory if it doesn't exist.
mkdir -p "$OUTPUT_DIR"

# Build the positional file arguments for the CLI.
# The `hf` CLI takes filenames as positional args after --local-dir.
# The deprecated `huggingface-cli` takes the repo id as the first positional arg,
# then --include or file names.
if [ "$HF_CLI" = "hf" ]; then
    "$HF_CLI" download "$SOURCE" "${TOKENIZER_FILES[@]}" --local-dir "$OUTPUT_DIR"
else
    # huggingface-cli legacy syntax: download REPO FILE1 FILE2 ... --local-dir DIR
    "$HF_CLI" download "$SOURCE" "${TOKENIZER_FILES[@]}" --local-dir "$OUTPUT_DIR"
fi

echo "done: $(ls "$OUTPUT_DIR" | wc -l) file(s) in $OUTPUT_DIR"
