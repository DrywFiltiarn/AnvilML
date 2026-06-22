#!/usr/bin/env bash
#
# seed_tokenizers.sh
#
# Imports tokenizer-only files (vocab/merges/config — NOT model
# weights) into worker/assets/<name>/ so that LoadClip's single-file
# real path (arch/clip/*.py) never needs a Hugging Face Hub call at
# worker runtime.
#
# This is run ONCE by whoever is updating a vendored tokenizer asset,
# not by every developer on every machine. The OUTPUT of this script
# (worker/assets/<name>/) is committed to git, unlike a typical
# "seed my local dev environment" script. Re-run only when adding a
# new tokenizer variant or intentionally refreshing an existing one.
#
# ---------------------------------------------------------------------
# Provenance notes (verified, not guessed, for all three variants):
# ---------------------------------------------------------------------
#
# qwen25_tokenizer (clip_type="qwen3"):
#   AnvilML's "qwen3" clip_type uses the Qwen2/Qwen2.5 BPE tokenizer
#   (class Qwen2Tokenizer) — this is unchanged across Qwen2.5 ->
#   Qwen3, which is why a single tokenizer asset serves both ZiT's
#   Qwen3-4B text encoder and Flux 2 Klein's Qwen3-8B text encoder
#   (model SIZE does not determine tokenizer choice; tokenizer
#   LINEAGE does). Cross-checked against ComfyUI's own vendored copy
#   (comfy/text_encoders/qwen25_tokenizer/) and Qwen/Qwen2.5-VL-7B-
#   Instruct's live tokenizer_config.json — exact field-for-field
#   match (model_max_length, tokenizer_class, pad_token, eos_token,
#   additional_special_tokens, chat_template structure). Only 3 files
#   needed (no tokenizer.json, no special_tokens_map.json) because
#   the code instantiates Qwen2Tokenizer directly (the slow/non-Auto
#   class), which only consumes vocab.json + merges.txt + its own
#   tokenizer_config.json. Qwen2Tokenizer.vocab_files_names confirms
#   {'vocab_file': 'vocab.json', 'merges_file': 'merges.txt'}.
#
# clip_l_tokenizer (clip_type="clip_l"):
#   CLIPTokenizer.vocab_files_names confirms
#   {'vocab_file': 'vocab.json', 'merges_file': 'merges.txt'} — same
#   shape as Qwen's, but a DIFFERENT vocabulary (CLIP-L's own BPE,
#   not Qwen's). openai/clip-vit-large-patch14 is the original OpenAI
#   repo, ungated, public. special_tokens_map.json is additionally
#   vendored (not strictly required by vocab_files_names, but the
#   repo's own tokenizer_config.json references it via
#   special_tokens_map_file, and ComfyUI's working vendored copy
#   (comfy/sd1_tokenizer/) includes it — matching the proven-working
#   precedent rather than the bare mechanical minimum).
#
# t5_tokenizer (clip_type="t5"):
#   AnvilML's LoadClip code was switched from T5Tokenizer (slow,
#   requires spiece.model) to T5TokenizerFast (fast, Rust-backed) —
#   see the P18-D11 task for the code-side change. T5TokenizerFast
#   accepts a consolidated tokenizer.json directly (no SentencePiece
#   conversion needed at load time). google/t5-v1_1-xxl does NOT
#   ship a tokenizer.json (only spiece.model), so it is not used
#   here. ComfyUI's actual working t5_tokenizer/ (used by flux.py)
#   was sourced from black-forest-labs/FLUX.1-dev's tokenizer_2/
#   subfolder — but that repo is GATED (requires HF auth + accepting
#   a non-commercial license, confirmed via multiple real
#   GatedRepoError reports), which is unacceptable for an anonymous
#   curl-based import. InvokeAI/t5-v1_1-xxl is an UNGATED,
#   Apache-2.0-licensed repo maintained by the InvokeAI project
#   specifically to provide redistributable copies of this exact
#   T5-XXL encoder+tokenizer in several formats; its own README
#   states the bfloat16/ variant was itself copied from
#   black-forest-labs/FLUX.1-schnell's text_encoder_2 — i.e. it is a
#   legitimate, explicitly-licensed re-host of the same content,
#   not an unrelated substitute. The bfloat16/tokenizer_2/ subfolder
#   path is confirmed via a real InvokeAI download log referencing
#   bfloat16/text_encoder_2/model-00002-of-00002.safetensors and a
#   separate bnb_llm_int8/tokenizer_2/special_tokens_map.json log
#   line, establishing the {variant}/tokenizer_2/{file} layout.
# ---------------------------------------------------------------------
#
# Adding a future variant (e.g. a Qwen3.5-based tokenizer, mirroring
# ComfyUI's separate qwen35_tokenizer/):
#   1. Add a new entry to VARIANT_DIRS / VARIANT_REPOS /
#      VARIANT_SUBFOLDERS / VARIANT_FILES below (same index across
#      all four arrays).
#   2. Re-run this script.
#   3. Wire the new directory name into the relevant arch/clip/*.py
#      module and commit worker/assets/<name>/ alongside the code
#      change, as its own task — do not silently grow this script's
#      scope without a corresponding Forge task.

set -euo pipefail

# Parallel arrays, one entry per variant, all indexed identically.
# (Bash 3.2-compatible — no associative-array-of-arrays available,
# and this must run on whatever bash ships with the dev box, not
# assume bash 4+.)

VARIANT_DIRS=(
  "qwen25_tokenizer"
  "clip_l_tokenizer"
  "t5_tokenizer"
)

VARIANT_REPOS=(
  "Qwen/Qwen2.5-VL-7B-Instruct"
  "openai/clip-vit-large-patch14"
  "InvokeAI/t5-v1_1-xxl"
)

# Subfolder within the repo, or "" for repo root.
VARIANT_SUBFOLDERS=(
  ""
  ""
  "bfloat16/tokenizer_2"
)

# Space-separated file list per variant (matched by index to the
# arrays above). Verified per-class requirements — see provenance
# notes above; do not assume one tokenizer family's file list
# applies to another.
VARIANT_FILES=(
  "merges.txt tokenizer_config.json vocab.json"
  "merges.txt tokenizer_config.json vocab.json special_tokens_map.json"
  "tokenizer.json tokenizer_config.json special_tokens_map.json"
)

# Resolve this script's directory so it can be run from anywhere and
# still find the repo root reliably (assumes this script lives at
# worker/tools/seed_tokenizers.sh — adjust the parent walk if
# relocated).
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
ASSETS_ROOT="${REPO_ROOT}/worker/assets"

echo "AnvilML — tokenizer asset import"
echo "Repo root:    ${REPO_ROOT}"
echo "Assets root:  ${ASSETS_ROOT}"
echo

num_variants=${#VARIANT_DIRS[@]}

for ((i = 0; i < num_variants; i++)); do
  dir_name="${VARIANT_DIRS[$i]}"
  repo_id="${VARIANT_REPOS[$i]}"
  subfolder="${VARIANT_SUBFOLDERS[$i]}"
  files="${VARIANT_FILES[$i]}"
  dest_dir="${ASSETS_ROOT}/${dir_name}"

  if [[ -n "${subfolder}" ]]; then
    echo "--- ${dir_name} (${repo_id}/${subfolder}) -> ${dest_dir} ---"
  else
    echo "--- ${dir_name} (${repo_id}) -> ${dest_dir} ---"
  fi
  mkdir -p "${dest_dir}"

  for fname in ${files}; do
    if [[ -n "${subfolder}" ]]; then
      url="https://huggingface.co/${repo_id}/resolve/main/${subfolder}/${fname}"
    else
      url="https://huggingface.co/${repo_id}/resolve/main/${fname}"
    fi
    dest_path="${dest_dir}/${fname}"

    echo "  fetching ${fname} ..."
    # -f: fail (non-zero exit) on HTTP 4xx/5xx instead of writing an
    #     HTML error page to disk and reporting success.
    # -L: follow redirects (HF resolve URLs redirect to S3/CDN).
    # -sS: silent except for errors, so failures are still visible.
    curl -fLsS -o "${dest_path}" "${url}"
  done

  echo "  done: $(ls -1 "${dest_dir}" | wc -l) files in ${dest_dir}"
  echo
done

echo "Import complete. These files are now intended to be COMMITTED:"
for dir_name in "${VARIANT_DIRS[@]}"; do
  echo "  git add worker/assets/${dir_name}"
done
echo
echo "Review the diff before committing (e.g. confirm no unexpected"
echo "binary/encoding issues from the download) and note the source"
echo "repo + commit/revision in your commit message for provenance."