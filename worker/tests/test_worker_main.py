"""Tests for worker.worker_main — mock capability probe."""

import subprocess
import sys

import pytest

import worker.worker_main as worker_main


class TestMockProbeCapabilities:
    """Tests for _mock_probe_capabilities() return value."""

    def test_returns_six_required_keys(self) -> None:
        """Function returns a dict with exactly the 6 required keys.

        Calls ``_mock_probe_capabilities()`` and asserts the result has
        exactly the 6 expected keys matching ``InferenceCaps`` field names:
        ``fp32``, ``fp16``, ``bf16``, ``fp8``, ``fp4``, ``flash_attention``.

        Preconditions: None (pure function, no setup).
        Expected output: Dict with exactly 6 keys, no more, no fewer.
        """
        result = worker_main._mock_probe_capabilities()

        expected_keys = {
            "fp32",
            "fp16",
            "bf16",
            "fp8",
            "fp4",
            "flash_attention",
        }
        assert set(result.keys()) == expected_keys, (
            f"Expected keys {expected_keys}, got {set(result.keys())}"
        )

    def test_all_values_are_bool(self) -> None:
        """All 6 values in the returned dict are bool type.

        Iterates over all returned values and asserts each is
        ``isinstance(value, bool)`` — not ``int``, ``str``, or other type.

        Preconditions: None (pure function, no setup).
        Expected output: Every ``isinstance(v, bool)`` is True.
        """
        result = worker_main._mock_probe_capabilities()

        for key, value in result.items():
            assert isinstance(value, bool), (
                f"Value for key {key!r} must be bool, "
                f"got {type(value).__name__}"
            )

    def test_fp4_is_false(self) -> None:
        """The fp4 key specifically maps to False.

        Asserts that ``result["fp4"] is False`` — the one deliberate
        exception in the synthetic values. Torch 2.x has no native fp4
        dtype, so this is universally False.

        Preconditions: None (pure function, no setup).
        Expected output: ``result["fp4"] is False``.
        """
        result = worker_main._mock_probe_capabilities()
        assert result["fp4"] is False, (
            f"fp4 must be False (no native torch.float4 dtype), "
            f"got {result['fp4']!r}"
        )


class TestNoTorchImport:
    """Tests for module import isolation (no transitive torch dependency)."""

    def test_no_torch_import_on_module_load(self) -> None:
        """Importing worker_main does not transitively import torch.

        Spawns a fresh subprocess via ``subprocess.run()`` that imports
        ``worker.worker_main`` and asserts ``"torch" not in sys.modules``.
        This confirms the module has no transitive torch dependency at
        import time (required by the mock-mode CI jobs that install only
        base.txt without torch).

        Uses subprocess isolation (not ``sys.modules`` manipulation) per
        ENVIRONMENT.md §11.3 — ``sys.modules.pop("torch")`` crashed the
        WSL2 agent VM at the OS level in prior development.

        Uses ``timeout=10`` per ENVIRONMENT.md §11.3 bounded-wait pattern.

        Preconditions: None.
        Expected output: Subprocess exit code 0, stdout contains "OK".
        """
        result = subprocess.run(
            [
                sys.executable,
                "-c",
                "import worker.worker_main; import sys; "
                "assert 'torch' not in sys.modules; print('OK')",
            ],
            capture_output=True,
            text=True,
            timeout=10,
        )
        assert result.returncode == 0, (
            f"Subprocess failed: stdout={result.stdout!r}, "
            f"stderr={result.stderr!r}"
        )
