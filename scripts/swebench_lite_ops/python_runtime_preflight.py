from __future__ import annotations

import importlib
import importlib.metadata as importlib_metadata
import re
import sys
from pathlib import Path
from typing import Any

from .core import REPO_ROOT, SwebenchOpsError, utc_now


DEFAULT_REQUIREMENTS = REPO_ROOT / "requirements" / "aiwonder-swebench-lite-ops.txt"
DEFAULT_EXPECTED_PYTHON = Path("/zfs/hot/contextgraph/venv/bin/python3")
PIN_RE = re.compile(r"^([A-Za-z0-9_.-]+)==([^\\s#]+)$")
REQUIRED_IMPORTS = {
    "jsonschema": ("jsonschema", "Draft202012Validator"),
}


class PythonRuntimePreflightError(SwebenchOpsError):
    pass


def _absolute(path: Path) -> Path:
    return Path(path).expanduser().absolute()


def _realpath(path: Path) -> Path:
    try:
        return path.resolve()
    except FileNotFoundError:
        return _absolute(path)


def _read_pins(path: Path) -> dict[str, str]:
    if not path.is_file():
        raise PythonRuntimePreflightError(
            "AIWONDER_PYTHON_REQUIREMENTS_MISSING",
            f"requirements file is missing: {path}",
            "Restore requirements/aiwonder-swebench-lite-ops.txt before running aiwonder Python commands.",
            {"requirements_path": str(path)},
        )

    pins: dict[str, str] = {}
    invalid: list[dict[str, Any]] = []
    for line_number, raw_line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
        line = raw_line.strip()
        if not line or line.startswith("#"):
            continue
        match = PIN_RE.fullmatch(line)
        if match is None:
            invalid.append({"line_number": line_number, "line": raw_line})
            continue
        name, version = match.groups()
        pins[name] = version

    if invalid:
        raise PythonRuntimePreflightError(
            "AIWONDER_PYTHON_REQUIREMENTS_UNPINNED",
            "aiwonder requirements must be exact pins only",
            "Replace unpinned or complex entries with explicit name==version pins.",
            {"requirements_path": str(path), "invalid": invalid},
        )
    if not pins:
        raise PythonRuntimePreflightError(
            "AIWONDER_PYTHON_REQUIREMENTS_EMPTY",
            "aiwonder requirements file has no package pins",
            "Add the required runtime pins before running aiwonder Python commands.",
            {"requirements_path": str(path)},
        )
    return pins


def _package_versions(pins: dict[str, str]) -> tuple[dict[str, str], list[dict[str, str]]]:
    versions: dict[str, str] = {}
    mismatches: list[dict[str, str]] = []
    for dist_name, expected in sorted(pins.items()):
        try:
            actual = importlib_metadata.version(dist_name)
        except importlib_metadata.PackageNotFoundError:
            actual = "MISSING"
        versions[dist_name] = actual
        if actual != expected:
            mismatches.append({"package": dist_name, "expected": expected, "actual": actual})
    return versions, mismatches


def _required_import_status() -> tuple[dict[str, str], list[dict[str, str]]]:
    status: dict[str, str] = {}
    failures: list[dict[str, str]] = []
    for label, (module_name, attr_name) in sorted(REQUIRED_IMPORTS.items()):
        try:
            module = importlib.import_module(module_name)
            getattr(module, attr_name)
        except Exception as exc:  # pragma: no cover - exercised manually in isolated venv FSV.
            status[label] = f"{type(exc).__name__}: {exc}"
            failures.append({"import": label, "module": module_name, "attribute": attr_name, "error": status[label]})
        else:
            status[label] = "ok"
    return status, failures


def add_arguments(parser: Any) -> None:
    parser.add_argument("--requirements-file", type=Path, default=DEFAULT_REQUIREMENTS)
    parser.add_argument(
        "--expected-python",
        type=Path,
        default=DEFAULT_EXPECTED_PYTHON,
        help="Fail unless this process is running under the expected aiwonder interpreter.",
    )
    parser.add_argument(
        "--allow-any-python",
        action="store_true",
        help="Only for local/unit verification; aiwonder operational runs must not set this.",
    )
    parser.add_argument(
        "--evidence-root",
        type=Path,
        default=Path("/zfs/archive/contextgraph/fsv/aiwonder-python-runtime-preflight"),
    )


def command_python_runtime_preflight(args: Any) -> dict[str, Any]:
    executable = _absolute(Path(sys.executable))
    expected = _absolute(args.expected_python)
    executable_realpath = _realpath(executable)
    expected_realpath = _realpath(expected)
    if not args.allow_any_python and executable != expected:
        raise PythonRuntimePreflightError(
            "AIWONDER_PYTHON_INTERPRETER_MISMATCH",
            f"running under {executable}, expected {expected}",
            "Run through contextgraph-shell with /zfs/hot/contextgraph/venv/bin/python3; do not let system Python mask venv gaps.",
            {
                "actual_python": str(executable),
                "expected_python": str(expected),
                "actual_python_realpath": str(executable_realpath),
                "expected_python_realpath": str(expected_realpath),
            },
        )

    pins = _read_pins(args.requirements_file)
    versions, mismatches = _package_versions(pins)
    imports, import_failures = _required_import_status()

    if mismatches or import_failures:
        raise PythonRuntimePreflightError(
            "AIWONDER_PYTHON_RUNTIME_DEPENDENCY_MISMATCH",
            "active Python runtime does not match the aiwonder swebench_lite_ops dependency contract",
            "Install exactly `python3 -m pip install -r requirements/aiwonder-swebench-lite-ops.txt` in the aiwonder venv before corpus/FSV work.",
            {
                "requirements_file": str(args.requirements_file),
                "actual_python": str(executable),
                "expected_python": str(expected),
                "actual_python_realpath": str(executable_realpath),
                "expected_python_realpath": str(expected_realpath),
                "version_mismatches": mismatches,
                "import_failures": import_failures,
                "versions": versions,
                "imports": imports,
            },
        )

    return {
        "ok": True,
        "checked_at": utc_now(),
        "command": "aiwonder-python-runtime-preflight",
        "requirements_file": str(args.requirements_file),
        "actual_python": str(executable),
        "expected_python": str(expected),
        "actual_python_realpath": str(executable_realpath),
        "expected_python_realpath": str(expected_realpath),
        "allow_any_python": bool(args.allow_any_python),
        "pins": pins,
        "versions": versions,
        "imports": imports,
    }
