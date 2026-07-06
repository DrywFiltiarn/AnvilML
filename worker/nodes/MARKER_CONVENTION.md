# Marker Convention — `REAL_PATH_VERIFIED` / `MOCK_PATH_VERIFIED`

Every node's `execute()` method and every arch module's `load()` / `sample()` / `decode()`
function carries a pair of comment markers declaring which tests exercise each code path,
placed immediately above the function or class definition:

```python
# REAL_PATH_VERIFIED: worker/tests/test_<module>.py::test_<name>_real_<fixture>
# MOCK_PATH_VERIFIED: worker/tests/test_<module>.py::test_<name>_mock_<pattern>
```

The markers go as `#` comments immediately above the class or function they verify
(e.g. `BaseNode.execute()`, `arch/diffusion/__init__.py` `load()`/`sample()`/`decode()`).

Gate 4 (`ENVIRONMENT.md §8`) mechanically validates these markers — it checks that every
named test is collectible via `pytest --collect-only`, and that every public function in
scope carries both markers.

Full rule: `ANVILML_DESIGN.md §10.6`.
