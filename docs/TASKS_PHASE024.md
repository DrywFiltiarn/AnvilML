# Tasks: Phase 024 — Release Packaging & Automation

| Field | Value |
|-------|-------|
| Phase | 024 |
| Name | Release Packaging & Automation |
| Milestone group | Distribution readiness |
| Depends on phases | 1-23 |
| Task file | `forge/tasks/tasks_phase024.json` |
| Tasks | 8 |

## Overview

Phase 24 turns a version bump into a published, signed, cross-platform GitHub Release. A version-watch workflow detects a change to `[workspace.package] version` on `main` and auto-creates the matching `vX.Y.Z` tag; the tag triggers the release workflow, which builds the Linux (`x86_64-unknown-linux-gnu`) and Windows (`x86_64-pc-windows-msvc`) binaries, assembles each into a self-contained zip — binary, `headless` default config, full `worker/` source with the baseline `requirements`, the provisioning scripts, `openapi.json`, QUICKSTART, LICENSE, a pre-created `models/` substructure (one dir per `ModelKind`) and runtime `logs/`+`artifacts/` dirs — then generates `SHA256SUMS` and detached GPG signatures and publishes a GitHub Release with auto-generated notes (commits since the previous tag), flagged pre-release when the version carries a `-suffix`. Real Authenticode/cosign signing and a worker-dependency update facility are recorded as deferred scope.

Every task in this phase implements **one module or one endpoint** plus its test. No task touches more than its named file(s). `cargo test` and `cargo clippy` are per-task gates; the phase as a whole is only complete when the **Runnable Proof** below passes.

## Tasks

| Task | Module / File | Summary |
|------|-------------|---------|
| P24-A1 | `scripts/package_release.sh (+ .ps1)` | release: packaging manifest and staging layout script |
| P24-A2 | `dist/QUICKSTART.md` | release: QUICKSTART and first-run docs included in package |
| P24-A3 | `.github/workflows/release-tag.yml` | release: version-watch job auto-creates the git tag on workspace bump |
| P24-A4 | `.github/workflows/release.yml` | release: Linux x86_64 build job |
| P24-A5 | `.github/workflows/release.yml` | release: Windows x86_64 MSVC build job |
| P24-A6 | `.github/workflows/release.yml` | release: checksums and GPG signatures for both artifacts |
| P24-A7 | `.github/workflows/release.yml` | release: publish GitHub Release with notes, assets, and pre-release flag |
| P24-A8 | `docs/RELEASE.md` | release: end-to-end packaging proof and deferred-signing note |

## Task details

#### P24-A1: release: packaging manifest and staging layout script

- **Prereqs:** P23-A7
- **Tags:** reasoning

Create scripts/package_release.sh (+ .ps1 mirror) assembling a platform staging dir: copy the built binary (anvilml/anvilml.exe), anvilml.toml (frontend.mode forced 'headless'), worker/ (full Python source incl requirements/*.txt baseline), scripts/install_worker_deps.{sh,ps1}, backend/openapi.json, QUICKSTART.md, LICENSE. Pre-create logs/ and artifacts/ (each .gitkeep) and models/ subdirs diffusion,lora,vae,controlnet,clip,unet,upscale (mirror ModelKind) each with a README.txt. Args: --target --binary-path --release-version --out-dir. bash -n passes; run produces full tree.

#### P24-A2: release: QUICKSTART and first-run docs included in package

- **Prereqs:** P24-A1
- **Tags:** —

Create dist/QUICKSTART.md (bundled by P24-A1). Content: extract zip; Linux chmod +x anvilml then ./anvilml (Windows anvilml.exe); on first run AnvilML auto-provisions the worker venv in the background (needs Python 3.12 + internet); API is reachable immediately at http://127.0.0.1:8488; provisioning progress at GET /v1/system/env and over /v1/events; where models go (models/<kind>/); how to edit anvilml.toml; how to verify SHA256SUMS and the GPG signature. Verify: file exists, valid markdown, referenced by package_release.sh.

#### P24-A3: release: version-watch job auto-creates the git tag on workspace bump

- **Prereqs:** P24-A2
- **Tags:** reasoning

Create .github/workflows/release-tag.yml triggered on push to main. Step: read [workspace.package] version from Cargo.toml at HEAD and at HEAD~1 (git show HEAD~1:Cargo.toml). If unchanged, exit 0 (no-op). If changed to vNEW: verify tag vNEW does not already exist; create and push annotated tag vNEW using the default GITHUB_TOKEN. Guard against first-commit (no HEAD~1). This is the ONLY trigger for releases. Verify (documented): bumping workspace.package.version and pushing to main creates tag v<new>; an unchanged push creates no tag.

#### P24-A4: release: Linux x86_64 build job

- **Prereqs:** P24-A3
- **Tags:** —

Create .github/workflows/release.yml triggered on tag push matching 'v*'. Job build-linux (ubuntu-latest, toolchain 1.95.0 from rust-toolchain.toml): cargo build --release --target x86_64-unknown-linux-gnu -p anvilml (binary only, NO mock-hardware feature). Run scripts/package_release.sh to stage, then zip to anvilml-<version>-linux-x64.zip. Upload as a workflow artifact for the publish job. Verify (documented): tag push produces the linux zip artifact containing the binary + worker/ + models structure + config.

#### P24-A5: release: Windows x86_64 MSVC build job

- **Prereqs:** P24-A4
- **Tags:** —

Add job build-windows (windows-latest, toolchain 1.95.0) to release.yml: cargo build --release --target x86_64-pc-windows-msvc -p anvilml (NO mock-hardware). Run scripts/package_release.ps1 to stage (anvilml.exe, install_worker_deps.ps1 as the active script), zip to anvilml-<version>-windows-x64.zip. Upload as workflow artifact. Note: release uses MSVC (native, conventional Windows artifact) even though the dev cross-check uses windows-gnu. Verify (documented): tag push produces the windows zip artifact with anvilml.exe + worker/ + structure.

#### P24-A6: release: checksums and GPG signatures for both artifacts

- **Prereqs:** P24-A5
- **Tags:** reasoning

Add job sign (ubuntu, needs build-linux+build-windows) to release.yml: download both zip artifacts, generate SHA256SUMS (one file covering both zips), and a detached GPG signature per zip (.asc) plus SHA256SUMS.asc, using a GPG private key + passphrase from repo secrets (ANVILML_GPG_KEY, ANVILML_GPG_PASSPHRASE) imported via gnupg. If the secret is absent, still produce SHA256SUMS and warn that signing was skipped (do not fail the release). Verify (documented): SHA256SUMS verifies both zips; gpg --verify checks each .asc against the published public key.

#### P24-A7: release: publish GitHub Release with notes, assets, and pre-release flag

- **Prereqs:** P24-A6
- **Tags:** reasoning

Add job publish (needs sign) to release.yml: create a GitHub Release for the pushed tag. Release notes = commit summary since the previous tag (git log <prevtag>..<tag> --pretty); use softprops/action-gh-release or gh CLI. Attach both platform zips, SHA256SUMS, and all .asc signatures. Mark as pre-release IF the version contains a hyphen suffix (e.g. 0.2.0-rc1). Title 'AnvilML <version>'. Verify (documented): pushing tag v<x> publishes a Release with both zips + sums + sigs, auto-generated notes, correctly flagged stable vs pre-release.

#### P24-A8: release: end-to-end packaging proof and deferred-signing note

- **Prereqs:** P24-A7
- **Tags:** —

No build code. Write docs/RELEASE.md documenting the full flow: bump [workspace.package] version -> push main -> release-tag.yml tags v<x> -> release.yml builds both platforms, signs, publishes. Include: how to download+verify (SHA256SUMS + gpg --verify), the published GPG public key location, and the standalone-headless note. Record DEFERRED items in ANVILML_DESIGN.md §25: (1) Authenticode/cosign 'genuine' signing for SmartScreen trust; (2) a worker-dependency UPDATE facility for when requirements baselines change post-release. Verify: docs/RELEASE.md exists; §25 lists both deferred items.


## Runnable Proof

Trigger a full release by bumping the workspace version, and verify the published assets.

```bash
# 1. Bump the release version and push:
#    edit Cargo.toml  ->  [workspace.package] version = "0.2.0"
git commit -am "release: 0.2.0" && git push origin main
# 2. release-tag.yml creates and pushes tag v0.2.0 automatically (watch Actions).
# 3. release.yml builds both platforms, signs, and publishes. On the Releases page:
#    - anvilml-0.2.0-linux-x64.zip
#    - anvilml-0.2.0-windows-x64.zip
#    - SHA256SUMS  (+ .asc signatures)
# 4. Verify an asset locally:
sha256sum -c SHA256SUMS
gpg --verify anvilml-0.2.0-linux-x64.zip.asc anvilml-0.2.0-linux-x64.zip
unzip -l anvilml-0.2.0-linux-x64.zip   # binary, anvilml.toml, worker/, models/<kind>/, logs/, artifacts/, QUICKSTART.md, LICENSE, openapi.json
```

Confirm: pushing an *unchanged* version creates no tag/release; a `0.2.0-rc1` version is marked **pre-release**; release notes list commits since the prior tag. Phase done when a workspace-version bump yields a complete, signed, correctly-flagged GitHub Release containing runnable zips for both platforms.
