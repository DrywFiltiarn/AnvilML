#!/usr/bin/env pwsh
<#
.SYNOPSIS
    Imports tokenizer-only files (vocab/merges/config -- NOT model
    weights) into worker/assets/<name>/ so that LoadClip's single-file
    real path (P18-D9) never needs a Hugging Face Hub call at worker
    runtime.

.DESCRIPTION
    This is run ONCE by whoever is updating a vendored tokenizer
    asset, not by every developer on every machine. The OUTPUT of
    this script (worker/assets/<name>/) is committed to git, unlike a
    typical "seed my local dev environment" script. Re-run only when
    adding a new tokenizer variant or intentionally refreshing an
    existing one.

    ---------------------------------------------------------------
    Provenance note (verified, not guessed):
    ---------------------------------------------------------------
    AnvilML's "qwen3" clip_type uses the Qwen2/Qwen2.5 BPE tokenizer
    (class Qwen2Tokenizer) -- this is unchanged across Qwen2.5 ->
    Qwen3, which is why a single tokenizer asset serves both ZiT's
    Qwen3-4B text encoder and Flux 2 Klein's Qwen3-8B text encoder
    (model SIZE does not determine tokenizer choice; tokenizer
    LINEAGE does).

    This was cross-checked against ComfyUI's own vendored copy
    (comfy/text_encoders/qwen25_tokenizer/), which carries
    processor_class: "Qwen2_5_VLProcessor" and
    model_max_length: 131072 in its tokenizer_config.json. Fetching
    Qwen/Qwen2.5-VL-7B-Instruct's own tokenizer_config.json directly
    from the Hub confirms an exact field-for-field match
    (model_max_length, tokenizer_class, pad_token, eos_token,
    additional_special_tokens, chat_template structure) --
    Qwen/Qwen2.5-VL-7B-Instruct is the verified canonical source
    repo, not a guess and not a re-host.

    Only 3 files are needed (NOT tokenizer.json, NOT
    special_tokens_map.json) because the code path explicitly
    instantiates Qwen2Tokenizer directly (the slow/non-Auto
    tokenizer class), which only consumes vocab.json + merges.txt +
    its own tokenizer_config.json.

.NOTES
    Adding a future variant (e.g. a Qwen3.5-based tokenizer,
    mirroring ComfyUI's separate qwen35_tokenizer/):
      1. Add a new entry to the $Variants hashtable below.
      2. Re-run this script.
      3. Wire the new directory name into LoadClip's clip_type
         dispatch and commit worker/assets/qwen35_tokenizer/
         alongside the code change, as its own task -- do not
         silently grow this script's scope without a corresponding
         Forge task.
#>

[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"

# Format: directory-name = huggingface-repo-id
# Only one variant is in scope for AnvilML today (see provenance
# note above). Add more entries here as new tokenizer lineages are
# needed.
$Variants = [ordered]@{
    "qwen25_tokenizer" = "Qwen/Qwen2.5-VL-7B-Instruct"
}

# Tokenizer-only files. Verified against Qwen2Tokenizer's actual
# from_pretrained() requirements and ComfyUI's working vendored copy.
$TokenizerFiles = @(
    "merges.txt",
    "tokenizer_config.json",
    "vocab.json"
)

# Resolve this script's directory so it can be run from anywhere and
# still find the repo root reliably (assumes this script lives at
# worker/tools/import_tokenizer_assets.ps1 -- adjust the parent walk
# if relocated).
$ScriptDir  = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot   = Resolve-Path (Join-Path $ScriptDir "..\..")
$AssetsRoot = Join-Path $RepoRoot "worker\assets"

Write-Host "AnvilML -- tokenizer asset import"
Write-Host "Repo root:    $RepoRoot"
Write-Host "Assets root:  $AssetsRoot"
Write-Host ""

foreach ($dirName in $Variants.Keys) {
    $repoId  = $Variants[$dirName]
    $destDir = Join-Path $AssetsRoot $dirName

    Write-Host "--- $dirName ($repoId) -> $destDir ---"
    New-Item -ItemType Directory -Force -Path $destDir | Out-Null

    foreach ($fname in $TokenizerFiles) {
        $url      = "https://huggingface.co/$repoId/resolve/main/$fname"
        $destPath = Join-Path $destDir $fname

        Write-Host "  fetching $fname ..."
        try {
            # -ErrorAction Stop (set globally above) means a non-2xx
            # response throws a terminating error instead of writing
            # an HTML error page to disk and reporting success.
            Invoke-WebRequest -Uri $url -OutFile $destPath -UseBasicParsing
        }
        catch {
            Write-Error "Failed to fetch '$fname' from '$url': $_"
            throw
        }
    }

    $count = (Get-ChildItem -Path $destDir -File).Count
    Write-Host "  done: $count files in $destDir"
    Write-Host ""
}

Write-Host "Import complete. These files are now intended to be COMMITTED:"
foreach ($dirName in $Variants.Keys) {
    Write-Host "  git add worker/assets/$dirName"
}
Write-Host ""
Write-Host "Review the diff before committing (e.g. confirm no unexpected"
Write-Host "binary/encoding issues from the download) and note the source"
Write-Host "repo + commit/revision in your commit message for provenance."