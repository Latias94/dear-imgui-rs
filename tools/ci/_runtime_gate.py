"""Native Test Engine and multi-viewport runtime release gates."""

from _runtime_gate_common import (
    GateCategory,
    GateResult,
    RuntimeContractError,
    record_runtime_preparation_failure,
)
from _runtime_gate_test_engine import run_test_engine_runtime
from _runtime_gate_viewport import (
    run_ash_vulkan_validation_smoke,
    run_multi_viewport_smoke,
    run_sdl3_glow_viewport_smoke,
)

__all__ = (
    "GateCategory",
    "GateResult",
    "RuntimeContractError",
    "record_runtime_preparation_failure",
    "run_ash_vulkan_validation_smoke",
    "run_multi_viewport_smoke",
    "run_sdl3_glow_viewport_smoke",
    "run_test_engine_runtime",
)
