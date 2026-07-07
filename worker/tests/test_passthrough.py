"""Tests for worker.nodes.passthrough — PassThrough node class and registration."""

import subprocess
import sys
import threading
import importlib

from worker.nodes.base import NodeContext, SlotSpec


def _make_ctx(mock: bool = True) -> NodeContext:
    """Construct a minimal NodeContext for testing.

    Args:
        mock: The mock flag value for the context.

    Returns:
        A NodeContext with all required attributes populated with
        minimal placeholder values.
    """
    return NodeContext(
        job_id="test-job",
        device="cpu",
        caps={"bf16": True, "fp8": False},
        cancel_flag=threading.Event(),
        emit=lambda e: None,
        pipeline_cache={},
        mock=mock,
    )


def test_class_attributes() -> None:
    """All six required class attributes exist with correct values.

    Verifies that PassThrough defines NODE_TYPE, CATEGORY, DISPLAY_NAME,
    DESCRIPTION, INPUT_SLOTS, and OUTPUT_SLOTS with the exact values
    specified in the node contract.

    Expected outcome: All six attributes match their expected values.
    """
    from worker.nodes.passthrough import PassThrough

    assert PassThrough.NODE_TYPE == "PassThrough"
    assert PassThrough.CATEGORY == "Debug"
    assert PassThrough.DISPLAY_NAME == "Pass Through"
    assert (
        PassThrough.DESCRIPTION
        == "A trivial no-op node that passes its input value through unchanged. "
           "Used to verify the dispatch pipeline and marker convention."
    )
    assert len(PassThrough.INPUT_SLOTS) == 1
    assert PassThrough.INPUT_SLOTS[0] == SlotSpec("value", "ANY")
    assert len(PassThrough.OUTPUT_SLOTS) == 1
    assert PassThrough.OUTPUT_SLOTS[0] == SlotSpec("value", "ANY")


def test_execute_mock_returns_input() -> None:
    """Mock-mode execute() returns the input value unchanged.

    Constructs a NodeContext with mock=True, calls execute() with
    {"value": "hello"}, and asserts the return dict matches.

    This test exercises the mock code path and satisfies the
    MOCK_PATH_VERIFIED marker.

    Expected outcome: {"value": "hello"} is returned.
    """
    from worker.nodes.passthrough import PassThrough

    node = PassThrough()
    ctx = _make_ctx(mock=True)
    result = node.execute(ctx, value="hello")
    assert result == {"value": "hello"}


def test_execute_real_returns_input() -> None:
    """Real-mode execute() returns the input value unchanged.

    Constructs a NodeContext with mock=False, calls execute() with
    {"value": 42}, and asserts the return dict matches.

    This test exercises the real code path and satisfies the
    REAL_PATH_VERIFIED marker.

    Expected outcome: {"value": 42} is returned.
    """
    from worker.nodes.passthrough import PassThrough

    node = PassThrough()
    ctx = _make_ctx(mock=False)
    result = node.execute(ctx, value=42)
    assert result == {"value": 42}


def test_node_in_registry_after_import() -> None:
    """PassThrough appears in NODE_REGISTRY after importing the module.

    Imports worker.nodes.passthrough (triggering @register at module load),
    then checks that NODE_REGISTRY["PassThrough"] exists and is the
    PassThrough class. This proves auto-import and registration work
    end-to-end.

    Uses subprocess isolation to avoid cross-test pollution from prior
    imports, following the pattern in test_ipc.py::test_module_no_torch_import.

    Expected outcome: NODE_REGISTRY contains "PassThrough" as a key.
    """
    code = (
        "import importlib; "
        "mod = importlib.import_module('worker.nodes.passthrough'); "
        "from worker.nodes.base import NODE_REGISTRY; "
        "assert 'PassThrough' in NODE_REGISTRY; "
        "assert NODE_REGISTRY['PassThrough'] is mod.PassThrough; "
        "print('OK')"
    )
    result = subprocess.run(
        [sys.executable, "-c", code],
        capture_output=True, text=True, timeout=10,
    )
    assert result.returncode == 0, f"subprocess failed: {result.stderr}"
    assert "OK" in result.stdout


def test_markers_name_collectible_tests() -> None:
    """Both REAL_PATH_VERIFIED and MOCK_PATH_VERIFIED marker test IDs are collectible.

    Reads the marker comments from the passthrough.py source file, extracts
    the test identifiers (e.g. "worker/tests/test_passthrough.py::test_name"),
    and runs pytest --collect-only on each to confirm they are collectible.

    This is the mechanical validation that Gate 4 performs.

    Expected outcome: Both named tests are collectible by pytest (exit 0).
    """
    import os
    import re

    source_path = os.path.join(
        os.path.dirname(__file__), "..", "nodes", "passthrough.py"
    )
    source_path = os.path.normpath(source_path)

    with open(source_path, "r") as f:
        source = f.read()

    # Extract test identifiers from both marker types.
    pattern = r"(?:REAL|MOCK)_PATH_VERIFIED:\s*(\S+)"
    matches = re.findall(pattern, source)
    assert len(matches) == 2, f"Expected 2 markers, found {len(matches)}"

    # Use sys.executable rather than a hardcoded worker/.venv path: this
    # matches the pattern already established by
    # test_node_in_registry_after_import and
    # TestNoTorchImport.test_module_no_torch_import above, and is what
    # actually needs to be true — the currently-running interpreter has
    # every dependency this test suite needs already installed, since
    # it's the same interpreter that just collected and ran this test.
    # A hardcoded "../.venv/bin/python" only exists as a local-dev
    # convention (matching ManagedWorker's default venv_path used for
    # spawning *real* workers in production); CI's worker-test job
    # installs dependencies directly into the runner-managed Python via
    # `pip install`, without ever provisioning worker/.venv, so this
    # would (and did) fail there with FileNotFoundError. It was also
    # POSIX-only ("bin/python", not "Scripts\\python.exe"), so it would
    # have failed identically on the Windows leg of the mock-mode matrix
    # even had .venv existed.
    for test_id in matches:
        result = subprocess.run(
            [sys.executable, "-m", "pytest", "--collect-only", test_id, "-q"],
            capture_output=True, text=True, timeout=10,
        )
        assert result.returncode == 0, (
            f"Test '{test_id}' is not collectible. "
            f"stderr={result.stderr}"
        )


def test_execute_returns_new_dict() -> None:
    """Each execute() call returns a new dict (no shared singleton state).

    Calls execute() twice with the same input and asserts the two return
    dicts are different objects (not a shared singleton), confirming no
    accidental state leakage between calls.

    Expected outcome: Two distinct dict objects are returned.
    """
    from worker.nodes.passthrough import PassThrough

    node = PassThrough()
    ctx = _make_ctx(mock=True)

    result1 = node.execute(ctx, value="same")
    result2 = node.execute(ctx, value="same")

    assert result1 is not result2
    assert result1 == result2
