"""Tests for worker.defaults — pure dataclass tests, no GPU/torch required."""

from worker.defaults import ModelDefaults, SDXL_DEFAULTS, ZIT_DEFAULTS


def test_zit_defaults_fields():
    """ZIT_DEFAULTS has the correct values for a zero-iteration distilled model."""
    assert ZIT_DEFAULTS.steps == 8
    assert ZIT_DEFAULTS.guidance_scale == 0.0
    assert ZIT_DEFAULTS.width == 1024
    assert ZIT_DEFAULTS.height == 1024
    assert ZIT_DEFAULTS.dtype == "bf16"
    assert ZIT_DEFAULTS.supports_negative_prompt is False


def test_sdxl_defaults_fields():
    """SDXL_DEFAULTS has the correct values for a standard SDXL model."""
    assert SDXL_DEFAULTS.steps == 20
    assert SDXL_DEFAULTS.guidance_scale == 7.5
    assert SDXL_DEFAULTS.width == 1024
    assert SDXL_DEFAULTS.height == 1024
    assert SDXL_DEFAULTS.dtype == "fp16"
    assert SDXL_DEFAULTS.supports_negative_prompt is True


def test_model_defaults_is_dataclass():
    """ModelDefaults is a valid dataclass with all expected fields."""
    instance = ModelDefaults(
        steps=4,
        guidance_scale=1.0,
        width=512,
        height=512,
        dtype="fp32",
    )
    assert instance.steps == 4
    assert instance.guidance_scale == 1.0
    assert instance.width == 512
    assert instance.height == 512
    assert instance.dtype == "fp32"
    assert instance.supports_negative_prompt is False
