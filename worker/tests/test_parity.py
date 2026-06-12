"""Parity test: NODE_REGISTRY matches the canonical known_node_types.json fixture."""

from __future__ import annotations

import importlib
import json
import sys
from pathlib import Path

from worker.nodes.base import NODE_REGISTRY

_JSON_FIXTURE = Path(__file__).parent.parent.parent / "backend" / "tests" / "known_node_types.json"


def test_node_parity() -> None:
    """Assert that the Python NODE_REGISTRY keys match the JSON fixture.

    Force-reimport every node module so that the ``@register`` decorator
    populates ``NODE_REGISTRY`` regardless of whether previous tests
    already loaded the modules into ``sys.modules`` (which would skip
    re-execution of the decorator).
    """
    _package_dir = str(Path(__file__).parent.parent / "nodes")
    for _mod_name in ("common", "sdxl", "zit"):
        full_name = f"worker.nodes.{_mod_name}"
        # Remove from cache so importlib re-executes the module (and its
        # ``@register`` decorators) even if a prior test loaded it.
        sys.modules.pop(full_name, None)
        importlib.import_module(full_name)

    json_data = json.loads(_JSON_FIXTURE.read_text(encoding="utf-8"))
    assert set(NODE_REGISTRY.keys()) == set(json_data), (
        f"NODE_REGISTRY keys {sorted(NODE_REGISTRY.keys())} "
        f"do not match known_node_types.json {sorted(json_data)}"
    )
