from __future__ import annotations

import pytest

from ..python_runtime_preflight import (
    DEFAULT_REQUIREMENTS,
    REQUIRED_IMPORTS,
    PythonRuntimePreflightError,
    _read_pins,
)


def test_aiwonder_requirements_pin_pytest_runner_stack():
    pins = _read_pins(DEFAULT_REQUIREMENTS)

    assert pins["pytest"] == "9.0.3"
    assert pins["pluggy"] == "1.6.0"
    assert pins["iniconfig"] == "2.3.0"
    assert pins["packaging"] == "26.2"
    assert pins["pygments"] == "2.20.0"
    assert REQUIRED_IMPORTS["pytest"] == ("pytest", "main")


def test_aiwonder_requirements_reject_unpinned_pytest(tmp_path):
    requirements = tmp_path / "requirements.txt"
    requirements.write_text("pytest>=9\n", encoding="utf-8")

    with pytest.raises(PythonRuntimePreflightError) as exc_info:
        _read_pins(requirements)

    assert exc_info.value.code == "AIWONDER_PYTHON_REQUIREMENTS_UNPINNED"
