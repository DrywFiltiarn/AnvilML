# Fixture Checkpoint Conventions

## Purpose

This directory holds tiny synthetic `.safetensors` checkpoint files used by real-mode
tests to exercise the model-loading contract end-to-end. These files are **never** real
downloaded model weights — they are deliberately small, structurally valid tensors
constructed deterministically for testing. See `ANVILML_DESIGN.md §17.5` for the full
design rationale.

## Sizing Rules

- Tensor shapes must be **structurally valid** for the architecture's shape-inference
  formula. The loader node's dispatch path will attempt to load the file; shapes that
  fail shape inference are not useful fixtures. For example, a latent tensor should use
  shapes like `(1, 4, 8, 8)` (batch, channels, height, width) and a diffusion weight
  might use `(768, 768)`.
- Shapes must **NOT** be a miniaturized copy of real model shapes verbatim. The purpose
  is structural validity — confirming the shape-inference logic works — not dimensional
  accuracy. A `(1, 4, 8, 8)` latent tensor is sufficient to exercise the same inference
  path as a real `(4, 64, 64, 64)` latent.
- Peak RAM to construct a fixture must stay **well under 1 GB**. The CI/agent VM has
  10 GB of RAM (`ANVILML_DESIGN.md §17.5`); oversized fixtures are exactly what
  produced the near-OOM incident on that machine. Each individual fixture file should be
  under 10 MB to leave ample headroom.

## Arch Families and Naming

Three arch families are covered, each with its own dispatch module path:

| Family   | Dispatch module path              | Fixture file pattern        | Example                |
|----------|-----------------------------------|-----------------------------|------------------------|
| diffusion| `worker/nodes/arch/diffusion/`    | `fixture_<family>.safetensors` | `fixture_zit.safetensors` |
| clip     | `worker/nodes/arch/clip/`         | `fixture_<family>.safetensors` | `fixture_qwen3.safetensors` |
| vae      | `worker/nodes/arch/vae/`          | `fixture_<family>.safetensors` | `fixture_zit_vae.safetensors` |

The `<family>` placeholder should be replaced with a lowercase, hyphenated name that
identifies the specific model variant (e.g., `zit`, `qwen3`, `zit_vae`, `flux2klein`).

## Metadata-Fallback Regression Case

**This is the most critical convention — do not skip it.**

v3 shipped a bug where `st.metadata` (a property reference on a
`safetensors` `SafeTensors` object) was called as `st.metadata()` (a method call with
parentheses), producing incorrect results when the loader attempted to read the `arch`
metadata key from the checkpoint header. The bug lived at
`worker/nodes/loader.py:702` in that codebase. It was never caught because every real
fixture used so far had a recognizable key prefix (e.g., `diffusion_model.output.weight`)
that let the loader identify the architecture without needing the `arch` metadata key at
all.

**Rule: at least one fixture per diffusion/CLIP/VAE family MUST have a non-recognizable
key prefix AND no `arch` metadata key, forcing the loader to use the metadata-fallback
path.**

For example, instead of keys like `diffusion_model.input.weight`, use a prefix such as
`xyz_random_tensor_data` — a prefix that does not match any known architecture pattern.
The fixture file must also omit the `arch` metadata key entirely from the safetensors
header. This combination forces the loader's shape/arch inference to exercise the
metadata-fallback code path (the exact path that the v3 bug lived in).

This is called the **metadata-fallback regression case**. It exists to ensure the bug
class that caused the v3 `st.metadata` vs `st.metadata()` incident can never silently
return: every new arch family's fixture suite must include a variant that exercises this
path from the start.

## Builder Script Convention

When a Phase 20+ author needs to create a fixture, they should write a small Python
script (e.g., `worker/tests/fixtures/build_<family>.py`) that uses the `safetensors`
library to create a tiny `.safetensors` file deterministically.

The builder script must:

- **Accept no arguments** — it runs end-to-end as-is.
- **Write the fixture file** to `worker/tests/fixtures/` (the directory containing this
  README).
- **Be idempotent** — safe to re-run without side effects. If the file already exists,
  overwrite it.
- **Use shapes that are structurally valid but small** — e.g., `(1, 4, 8, 8)` for a
  latent tensor, `(768, 768)` for a diffusion model weight.

The script should be committed alongside the generated `.safetensors` file so that
future authors can reproduce or modify the fixture.

Example structure of a builder script:

```python
#!/usr/bin/env python3
"""Build the zit diffusion fixture checkpoint.

Usage: python worker/tests/fixtures/build_zit_fixture.py
"""

from safetensors.torch import save_file
import torch

def build():
    tensors = {
        "xyz_random_tensor_data": torch.randn(1, 4, 8, 8),
        "xyz_another_tensor": torch.randn(768, 768),
    }
    # No 'arch' key in metadata — forces the metadata-fallback path.
    save_file(tensors, "worker/tests/fixtures/fixture_zit.safetensors")

if __name__ == "__main__":
    build()
```

## How to Add a New Fixture

Follow this checklist when creating a new fixture for a new arch family:

1. **Identify the arch family** and the key the loader will use (diffusion, clip, or vae).
2. **Choose tensor shapes** that are structurally valid for that family's shape-inference
   formula. Consult the arch module's `_infer_hyperparams()` or equivalent for the
   expected tensor dimensions.
3. **Write the builder script** (`worker/tests/fixtures/build_<family>.py`) using the
   `safetensors` library, following the conventions above.
4. **Create at least one "fallback" fixture** with non-recognizable key prefixes and
   no `arch` metadata key. This is the metadata-fallback regression case — required for
   every family.
5. **Run the builder script** to generate the `.safetensors` file(s).
6. **Verify the fixture loads correctly in mock mode** — set
   `ANVILML_WORKER_MOCK=1` and confirm the loader's mock branch handles the file.
   No torch import is needed for this step.
7. **Verify the fixture loads correctly in real mode** — unset `ANVILML_WORKER_MOCK`
   and confirm the loader's real branch exercises `load()` against the fixture on torch
   CPU.

## What This Directory Does NOT Contain

- **No real model weights** — not even scaled-down versions. Every file is a tiny
  synthetic construction of random tensors.
- **No checkpoints larger than what's needed for shape-inference validation** — each
  file should be under 10 MB.
- **No fixture files from v3 or earlier versions of the codebase** — this directory is
  new in v4. Any pre-existing fixture files should not be copied here.
