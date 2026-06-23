#!/usr/bin/env bash
# run_all.sh - run every real-path verification script in order.
#
# Each script is also independently runnable; this just chains them so you
# get one combined report instead of running five commands by hand. Unlike
# `set -e`, this deliberately continues past a failing script (e.g. LoadClip
# failing on the tokenizer path bug should not prevent you from also seeing
# whether LoadModel and LoadVae are fine) and reports an overall pass/fail
# count at the end.
#
# Usage:
#   export ANVILML_MODELS_DIR=/path/to/models
#   export ANVILML_ZIT_MODEL=zit_fp8.safetensors
#   export ANVILML_ZIT_VAE=zit_vae.safetensors
#   export ANVILML_ZIT_CLIP=qwen3_4b.safetensors
#   export ANVILML_DEVICE=cuda:0       # optional, defaults to cuda:0
#   ./run_all.sh

set -u  # unset vars are errors; missing-file errors are handled by the
        # scripts themselves with clear FATAL messages, not by this wrapper

export ANVILML_MODELS_DIR=/path/to/models
export ANVILML_ZIT_MODEL=zit_fp8.safetensors
export ANVILML_ZIT_VAE=zit_vae.safetensors
export ANVILML_ZIT_CLIP=qwen3_4b.safetensors
export ANVILML_DEVICE=cuda:0

cd "$(dirname "$0")"

if [ "${ANVILML_WORKER_MOCK:-}" = "1" ]; then
    echo "FATAL: ANVILML_WORKER_MOCK=1 is set - unset it before running this harness." >&2
    exit 2
fi

SCRIPTS=(
    "01_loaders.py"
    "02_clip_encode.py"
    "03_empty_latent.py"
    "04_sampler.py"
    "05_vae_decode.py"
)

declare -a RESULTS

for script in "${SCRIPTS[@]}"; do
    echo ""
    echo "########################################################################"
    echo "# $script"
    echo "########################################################################"
    python3 "$script"
    rc=$?
    if [ $rc -eq 0 ]; then
        RESULTS+=("PASS  $script")
    else
        RESULTS+=("FAIL  $script  (exit $rc)")
    fi
done

echo ""
echo "========================================================================"
echo "OVERALL"
echo "========================================================================"
overall_rc=0
for r in "${RESULTS[@]}"; do
    echo "  $r"
    case "$r" in
        FAIL*) overall_rc=1 ;;
    esac
done
echo "========================================================================"

export ANVILML_MODELS_DIR=""
export ANVILML_ZIT_MODEL=""
export ANVILML_ZIT_VAE=""
export ANVILML_ZIT_CLIP=""
export ANVILML_DEVICE=""

exit $overall_rc
