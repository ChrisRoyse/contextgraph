from __future__ import annotations

import json
import os
import subprocess
import time
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[2]
DEFAULT_SWEBENCH_ROOT = Path("/mnt/d/contextgraph/cache/contextgraph-swebench/SWE-bench")
DEFAULT_SWEBENCH_VENV = Path("/mnt/d/contextgraph/cache/contextgraph-swebench/venv")
DEFAULT_TASKS_DIR = REPO_ROOT / "tasks" / "swebench-lite"
DEFAULT_EVIDENCE_ROOT = Path("/mnt/d/contextgraph/swebench-artifacts/swebench-lite-fsv")
DEFAULT_DATASET = "princeton-nlp/SWE-bench_Lite"
DEFAULT_SPLIT = "test"
DEFAULT_SMOKE_TASK = "sympy__sympy-20590"
EXPECTED_LITE_COUNT = 300


class SwebenchOpsError(RuntimeError):
    def __init__(
        self,
        code: str,
        message: str,
        remediation: str,
        details: dict[str, Any] | None = None,
    ):
        super().__init__(message)
        self.code = code
        self.message = message
        self.remediation = remediation
        self.details = details or {}

    def to_json(self) -> dict[str, Any]:
        return {
            "ok": False,
            "error_code": self.code,
            "message": self.message,
            "remediation": self.remediation,
            "details": self.details,
        }


@dataclass
class CommandResult:
    command: list[str]
    cwd: str | None
    exit_code: int
    duration_ms: int
    stdout_tail: str
    stderr_tail: str

    def to_json(self) -> dict[str, Any]:
        return {
            "command": self.command,
            "cwd": self.cwd,
            "exit_code": self.exit_code,
            "duration_ms": self.duration_ms,
            "stdout_tail": self.stdout_tail,
            "stderr_tail": self.stderr_tail,
        }


def utc_now() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="seconds")


def tail(text: str | bytes | None, limit: int = 6000) -> str:
    if text is None:
        return ""
    if isinstance(text, bytes):
        text = text.decode("utf-8", errors="replace")
    return text[-limit:]


def run(
    command: list[str],
    *,
    cwd: Path | None = None,
    timeout: int = 300,
    env: dict[str, str] | None = None,
    check: bool = True,
    stdout_limit: int = 6000,
    stderr_limit: int = 6000,
) -> CommandResult:
    started = time.monotonic()
    try:
        completed = subprocess.run(
            command,
            cwd=str(cwd) if cwd else None,
            env=env,
            text=True,
            capture_output=True,
            timeout=timeout,
            check=False,
        )
    except FileNotFoundError as exc:
        raise SwebenchOpsError(
            "SWEBENCH_COMMAND_NOT_FOUND",
            f"command not found: {command[0]}",
            "Install the missing executable and rerun the SWE-bench operation.",
            {"command": command},
        ) from exc
    except subprocess.TimeoutExpired as exc:
        raise SwebenchOpsError(
            "SWEBENCH_COMMAND_TIMEOUT",
            f"command timed out after {timeout}s: {' '.join(command)}",
            "Fix the hung subprocess or increase the timeout after profiling the run.",
            {
                "command": command,
                "stdout_tail": tail(exc.stdout),
                "stderr_tail": tail(exc.stderr),
            },
        ) from exc

    result = CommandResult(
        command=command,
        cwd=str(cwd) if cwd else None,
        exit_code=completed.returncode,
        duration_ms=int((time.monotonic() - started) * 1000),
        stdout_tail=tail(completed.stdout, stdout_limit),
        stderr_tail=tail(completed.stderr, stderr_limit),
    )
    if check and completed.returncode != 0:
        raise SwebenchOpsError(
            "SWEBENCH_COMMAND_FAILED",
            f"command exited {completed.returncode}: {' '.join(command)}",
            "Inspect stdout_tail/stderr_tail and fix the failing dependency.",
            result.to_json(),
        )
    return result


def write_json_checked(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    text = json.dumps(payload, indent=2, sort_keys=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    with tmp.open("w", encoding="utf-8") as handle:
        handle.write(text)
        handle.write("\n")
        handle.flush()
        os.fsync(handle.fileno())
    tmp.replace(path)
    readback = json.loads(path.read_text(encoding="utf-8"))
    if readback != payload:
        raise SwebenchOpsError(
            "SWEBENCH_EVIDENCE_READBACK_MISMATCH",
            f"evidence readback mismatch at {path}",
            "Check the filesystem for partial writes or concurrent writers.",
            {"path": str(path)},
        )


def parse_json_tail(result: CommandResult, field: str = "stdout_tail") -> Any:
    raw = getattr(result, field)
    for line in reversed([line for line in raw.splitlines() if line.strip()]):
        try:
            return json.loads(line)
        except json.JSONDecodeError:
            continue
    raise SwebenchOpsError(
        "SWEBENCH_JSON_OUTPUT_MISSING",
        f"no JSON object found in {field}",
        "Fix the probe command so its final output line is JSON.",
        result.to_json(),
    )


def ensure_file(path: Path, code: str, label: str) -> None:
    if not path.is_file():
        raise SwebenchOpsError(
            code,
            f"{label} is missing: {path}",
            f"Create or restore {label}, then rerun the operation.",
            {"path": str(path)},
        )


def ensure_dir(path: Path, code: str, label: str) -> None:
    if not path.is_dir():
        raise SwebenchOpsError(
            code,
            f"{label} is missing: {path}",
            f"Create or restore {label}, then rerun the operation.",
            {"path": str(path)},
        )


def validate_instance_id(instance_id: str) -> None:
    if not instance_id or instance_id.strip() != instance_id:
        raise SwebenchOpsError(
            "SWEBENCH_INVALID_INSTANCE_ID",
            f"invalid instance_id: {instance_id!r}",
            "Use an exact SWE-bench Lite instance id like sympy__sympy-20590.",
        )
    if "__" not in instance_id or any(ch.isspace() for ch in instance_id):
        raise SwebenchOpsError(
            "SWEBENCH_INVALID_INSTANCE_ID",
            f"invalid instance_id: {instance_id!r}",
            "Use the official SWE-bench double-underscore instance-id format.",
        )


def official_image_ref(
    instance_id: str,
    namespace: str = "swebench",
    arch: str = "x86_64",
    tag: str = "latest",
) -> str:
    validate_instance_id(instance_id)
    remote_instance = instance_id.lower().replace("__", "_1776_")
    return f"{namespace}/sweb.eval.{arch}.{remote_instance}:{tag}"


def python_bin(venv: Path) -> Path:
    return venv / "bin" / "python"


def load_task_manifests(tasks_dir: Path) -> dict[str, dict[str, Any]]:
    ensure_dir(tasks_dir, "SWEBENCH_TASK_MANIFEST_DIR_MISSING", "SWE-bench Lite task manifest directory")
    manifests: dict[str, dict[str, Any]] = {}
    for path in sorted(tasks_dir.glob("*.json")):
        try:
            payload = json.loads(path.read_text(encoding="utf-8"))
        except json.JSONDecodeError as exc:
            raise SwebenchOpsError(
                "SWEBENCH_TASK_MANIFEST_INVALID_JSON",
                f"invalid task manifest JSON: {path}",
                "Fix or regenerate the checked-in SWE-bench Lite task manifest.",
                {"path": str(path), "error": str(exc)},
            ) from exc
        instance_id = path.stem
        if payload.get("instance_id") and payload["instance_id"] != instance_id:
            raise SwebenchOpsError(
                "SWEBENCH_TASK_MANIFEST_ID_MISMATCH",
                f"manifest filename/id mismatch for {path}",
                "Make the manifest filename and instance_id field agree.",
                {"path": str(path), "instance_id": payload.get("instance_id")},
            )
        manifests[instance_id] = payload
    if len(manifests) != EXPECTED_LITE_COUNT:
        raise SwebenchOpsError(
            "SWEBENCH_TASK_MANIFEST_COUNT_MISMATCH",
            f"expected {EXPECTED_LITE_COUNT} task manifests, found {len(manifests)}",
            "Restore the complete SWE-bench Lite task manifest set under tasks/swebench-lite.",
            {"tasks_dir": str(tasks_dir), "count": len(manifests)},
        )
    return manifests
