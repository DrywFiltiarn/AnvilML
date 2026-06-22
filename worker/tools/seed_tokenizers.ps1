#!/usr/bin/env pwsh
<#
.SYNOPSIS
    Imports tokenizer-only files (vocab/merges/config -- NOT model
    weights) into worker/assets/<name>/ so that LoadClip's single-file
    real path (arch/clip/*.py) never needs a Hugging Face Hub call at
    worker runtime.

.DESCRIPTION
    This is run ONCE by whoever is updating a vendored tokenizer
    asset, not by every developer on every machine. The OUTPUT of
    this script (worker/assets/<name>/) is committed to git, unlike a
    typical "seed my local dev environment" script. Re-run only when
    adding a new tokenizer variant or intentionally refreshing an
    existing one.

    ---------------------------------------------------------------
    Provenance notes (verified, not guessed, for all three variants):
    ---------------------------------------------------------------

    qwen25_tokenizer (clip_type="qwen3"):
      AnvilML's "qwen3" clip_type uses the Qwen2/Qwen2.5 BPE
      tokenizer (class Qwen2Tokenizer) -- this is unchanged across
      Qwen2.5 -> Qwen3, which is why a single tokenizer asset serves
      both ZiT's Qwen3-4B text encoder and Flux 2 Klein's Qwen3-8B
      text encoder (model SIZE does not determine tokenizer choice;
      tokenizer LINEAGE does). Cross-checked against ComfyUI's own
      vendored copy (comfy/text_encoders/qwen25_tokenizer/) and
      Qwen/Qwen2.5-VL-7B-Instruct's live tokenizer_config.json --
      exact field-for-field match. Only 3 files needed because the
      code instantiates Qwen2Tokenizer directly (the slow/non-Auto
      class). Qwen2Tokenizer.vocab_files_names confirms
      {'vocab_file': 'vocab.json', 'merges_file': 'merges.txt'}.

    clip_l_tokenizer (clip_type="clip_l"):
      CLIPTokenizer.vocab_files_names confirms
      {'vocab_file': 'vocab.json', 'merges_file': 'merges.txt'} --
      same shape as Qwen's, but a DIFFERENT vocabulary (CLIP-L's own
      BPE, not Qwen's). openai/clip-vit-large-patch14 is the
      original OpenAI repo, ungated, public. special_tokens_map.json
      is additionally vendored (the repo's own tokenizer_config.json
      references it via special_tokens_map_file, and ComfyUI's
      working vendored copy includes it).

    t5_tokenizer (clip_type="t5"):
      AnvilML's LoadClip code was switched from T5Tokenizer (slow,
      requires spiece.model) to T5TokenizerFast (fast, Rust-backed)
      -- see the P18-D11 task. T5TokenizerFast accepts a consolidated
      tokenizer.json directly. google/t5-v1_1-xxl does NOT ship a
      tokenizer.json (only spiece.model), so it is not used here.
      ComfyUI's actual working t5_tokenizer/ was sourced from
      black-forest-labs/FLUX.1-dev's tokenizer_2/ subfolder -- but
      that repo is GATED (requires HF auth + accepting a
      non-commercial license), unacceptable for an anonymous
      Invoke-WebRequest import. InvokeAI/t5-v1_1-xxl is an UNGATED,
      Apache-2.0-licensed repo maintained by the InvokeAI project
      specifically to provide redistributable copies of this exact
      T5-XXL encoder+tokenizer; its own README states the bfloat16/
      variant was copied from black-forest-labs/FLUX.1-schnell's
      text_encoder_2 -- a legitimate, explicitly-licensed re-host.
      The bfloat16/tokenizer_2/ subfolder path is confirmed via a
      real InvokeAI download log.

.NOTES
    Adding a future variant (e.g. a Qwen3.5-based tokenizer,
    mirroring ComfyUI's separate qwen35_tokenizer/):
      1. Add a new entry to $Variants below (DirName, RepoId,
         Subfolder, Files).
      2. Re-run this script.
      3. Wire the new directory name into the relevant arch/clip/*.py
         module and commit worker/assets/<name>/ alongside the code
         change, as its own task -- do not silently grow this
         script's scope without a corresponding Forge task.
#>

[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"

# Each variant carries its own repo, optional subfolder, and exact
# file list -- verified per-class requirements differ between
# tokenizer families (see provenance notes above). Do not assume one
# family's file list applies to another.
$Variants = @(
    [PSCustomObject]@{
        DirName   = "qwen25_tokenizer"
        RepoId    = "Qwen/Qwen2.5-VL-7B-Instruct"
        Subfolder = ""
        Files     = @("merges.txt", "tokenizer_config.json", "vocab.json")
    },
    [PSCustomObject]@{
        DirName   = "clip_l_tokenizer"
        RepoId    = "openai/clip-vit-large-patch14"
        Subfolder = ""
        Files     = @("merges.txt", "tokenizer_config.json", "vocab.json", "special_tokens_map.json")
    },
    [PSCustomObject]@{
        DirName   = "t5_tokenizer"
        RepoId    = "InvokeAI/t5-v1_1-xxl"
        Subfolder = "bfloat16/tokenizer_2"
        Files     = @("tokenizer.json", "tokenizer_config.json", "special_tokens_map.json")
    }
)

# Resolve this script's directory so it can be run from anywhere and
# still find the repo root reliably (assumes this script lives at
# worker/tools/seed_tokenizers.ps1 -- adjust the parent walk if
# relocated).
$ScriptDir  = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot   = Resolve-Path (Join-Path $ScriptDir "..\..")
$AssetsRoot = Join-Path $RepoRoot "worker\assets"

Write-Host "AnvilML -- tokenizer asset import"
Write-Host "Repo root:    $RepoRoot"
Write-Host "Assets root:  $AssetsRoot"
Write-Host ""

foreach ($variant in $Variants) {
    $dirName   = $variant.DirName
    $repoId    = $variant.RepoId
    $subfolder = $variant.Subfolder
    $destDir   = Join-Path $AssetsRoot $dirName

    if ($subfolder) {
        Write-Host "--- $dirName ($repoId/$subfolder) -> $destDir ---"
    } else {
        Write-Host "--- $dirName ($repoId) -> $destDir ---"
    }
    New-Item -ItemType Directory -Force -Path $destDir | Out-Null

    foreach ($fname in $variant.Files) {
        if ($subfolder) {
            $url = "https://huggingface.co/$repoId/resolve/main/$subfolder/$fname"
        } else {
            $url = "https://huggingface.co/$repoId/resolve/main/$fname"
        }
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
foreach ($variant in $Variants) {
    Write-Host "  git add worker/assets/$($variant.DirName)"
}
Write-Host ""
Write-Host "Review the diff before committing (e.g. confirm no unexpected"
Write-Host "binary/encoding issues from the download) and note the source"
Write-Host "repo + commit/revision in your commit message for provenance."