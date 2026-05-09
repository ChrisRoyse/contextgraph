#!/usr/bin/env python3
"""Fail-closed smoke FSV for ME-JEPA embedder readiness.

This probe does not load neural weights. It proves the durable prerequisites
that must exist before a full warm-load/full-corpus run: GPU state, SWE-bench
Lite manifest state, and real on-disk model asset state.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import struct
import subprocess
import sys
import tempfile
import time
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_MODELS_DIR = REPO_ROOT / "models"
DEFAULT_TASKS_DIR = REPO_ROOT / "tasks" / "swebench-lite"
DEFAULT_EVIDENCE_ROOT = Path("/tmp/contextgraph-mejepa-embedders-fsv")
EXPECTED_SWEBENCH_LITE_COUNT = 300
MAX_SAFETENSORS_HEADER_BYTES = 100 * 1024 * 1024


PRODUCTION_ASSETS: list[dict[str, Any]] = [
    {
        "slot": "E1",
        "model_id": "semantic",
        "repo": "intfloat/e5-large-v2",
        "directory": "semantic",
        "projected_dimension": 1024,
        "required": ["config.json", "model.safetensors", "tokenizer.json"],
    },
    {
        "slot": "E2",
        "model_id": "temporal_recent",
        "repo": None,
        "directory": None,
        "projected_dimension": 512,
        "custom": True,
        "required": [],
    },
    {
        "slot": "E3",
        "model_id": "temporal_periodic",
        "repo": None,
        "directory": None,
        "projected_dimension": 512,
        "custom": True,
        "required": [],
    },
    {
        "slot": "E4",
        "model_id": "temporal_positional",
        "repo": None,
        "directory": None,
        "projected_dimension": 512,
        "custom": True,
        "required": [],
    },
    {
        "slot": "E5",
        "model_id": "causal",
        "repo": "nomic-ai/nomic-embed-text-v1.5",
        "directory": "causal",
        "projected_dimension": 768,
        "required": [
            "config.json",
            "model.safetensors",
            "tokenizer.json",
            "trained/lora_best.safetensors",
            "trained/projection_best.safetensors",
        ],
    },
    {
        "slot": "E6",
        "model_id": "sparse",
        "repo": "naver/splade-cocondenser-ensembledistil",
        "directory": "sparse",
        "projected_dimension": 1536,
        "required": ["config.json", "model.safetensors", "tokenizer.json"],
    },
    {
        "slot": "E7",
        "model_id": "code",
        "repo": "Qodo/Qodo-Embed-1-1.5B",
        "directory": "code",
        "projected_dimension": 1536,
        "required": ["config.json", "model.safetensors", "tokenizer.json"],
    },
    {
        "slot": "E8",
        "model_id": "graph",
        "repo": "intfloat/e5-large-v2",
        "directory": "semantic",
        "shared_with": "E1",
        "projected_dimension": 1024,
        "required": ["config.json", "model.safetensors", "tokenizer.json"],
    },
    {
        "slot": "E9",
        "model_id": "hdc",
        "repo": None,
        "directory": None,
        "projected_dimension": 1024,
        "custom": True,
        "required": [],
    },
    {
        "slot": "E10",
        "model_id": "contextual",
        "repo": "intfloat/e5-base-v2",
        "directory": "contextual",
        "projected_dimension": 768,
        "required": ["config.json", "model.safetensors", "tokenizer.json"],
    },
    {
        "slot": "E11",
        "model_id": "kepler",
        "repo": "THU-KEG/KEPLER-Wiki5M-KE",
        "directory": "kepler",
        "projected_dimension": 768,
        "required": ["config.json", "model.safetensors", "tokenizer.json"],
    },
    {
        "slot": "E12",
        "model_id": "late_interaction",
        "repo": "colbert-ir/colbertv2.0",
        "directory": "late-interaction",
        "projected_dimension": 128,
        "required": ["config.json", "model.safetensors", "tokenizer.json"],
    },
    {
        "slot": "E13",
        "model_id": "splade",
        "repo": "prithivida/Splade_PP_en_v1",
        "directory": "splade-v3",
        "projected_dimension": 1536,
        "required": ["config.json", "model.safetensors", "tokenizer.json"],
    },
    {
        "slot": "E14",
        "model_id": "bge_m3_dense",
        "repo": "BAAI/bge-m3",
        "directory": "bge-m3-dense",
        "projected_dimension": 1024,
        "required": ["config.json", "model.safetensors", "tokenizer.json"],
    },
]


class SmokeError(RuntimeError):
    def __init__(self, code: str, message: str, remediation: str, details: dict[str, Any] | None = None):
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
    exit_code: int
    duration_ms: int
    stdout: str
    stderr: str

    def to_json(self) -> dict[str, Any]:
        return {
            "command": self.command,
            "exit_code": self.exit_code,
            "duration_ms": self.duration_ms,
            "stdout_tail": self.stdout[-4000:],
            "stderr_tail": self.stderr[-4000:],
        }


def utc_now() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="seconds")


def run(command: list[str], timeout: int = 30) -> CommandResult:
    started = time.monotonic()
    try:
        completed = subprocess.run(
            command,
            text=True,
            capture_output=True,
            check=False,
            timeout=timeout,
        )
    except FileNotFoundError as exc:
        raise SmokeError(
            "MEJEPA_SMOKE_COMMAND_NOT_FOUND",
            f"command not found: {command[0]}",
            "Install the missing executable and rerun the smoke FSV.",
            {"command": command},
        ) from exc
    except subprocess.TimeoutExpired as exc:
        raise SmokeError(
            "MEJEPA_SMOKE_COMMAND_TIMEOUT",
            f"command timed out after {timeout}s: {' '.join(command)}",
            "Fix the hung command or raise the timeout after profiling.",
            {"command": command, "stdout_tail": (exc.stdout or "")[-4000:], "stderr_tail": (exc.stderr or "")[-4000:]},
        ) from exc

    return CommandResult(
        command=command,
        exit_code=completed.returncode,
        duration_ms=int((time.monotonic() - started) * 1000),
        stdout=completed.stdout,
        stderr=completed.stderr,
    )


def write_json_checked(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    text = json.dumps(payload, indent=2, sort_keys=True)
    with tmp.open("w", encoding="utf-8") as handle:
        handle.write(text)
        handle.write("\n")
        handle.flush()
        os.fsync(handle.fileno())
    tmp.replace(path)
    readback = json.loads(path.read_text(encoding="utf-8"))
    if readback != payload:
        raise SmokeError(
            "MEJEPA_SMOKE_EVIDENCE_READBACK_MISMATCH",
            f"evidence readback mismatch at {path}",
            "Check filesystem integrity and concurrent writers.",
            {"path": str(path)},
        )


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def inspect_json(path: Path) -> dict[str, Any]:
    data = path.read_text(encoding="utf-8")
    value = json.loads(data)
    if not isinstance(value, dict):
        raise SmokeError(
            "MEJEPA_SMOKE_JSON_ROOT_INVALID",
            f"JSON root is not an object: {path}",
            "Replace the file with the original model JSON asset.",
            {"path": str(path)},
        )
    return {"json_object_keys_sample": sorted(value.keys())[:12], "json_key_count": len(value)}


def inspect_safetensors_header(path: Path) -> dict[str, Any]:
    with path.open("rb") as handle:
        prefix = handle.read(8)
        if len(prefix) != 8:
            raise SmokeError(
                "MEJEPA_SMOKE_SAFETENSORS_PREFIX_SHORT",
                f"SafeTensors file is too short to contain an 8-byte header length: {path}",
                "Re-download the model.safetensors file.",
                {"path": str(path), "bytes_read": len(prefix)},
            )
        header_len = struct.unpack("<Q", prefix)[0]
        size = path.stat().st_size
        if header_len == 0 or header_len > MAX_SAFETENSORS_HEADER_BYTES:
            raise SmokeError(
                "MEJEPA_SMOKE_SAFETENSORS_HEADER_INVALID",
                f"invalid SafeTensors header length {header_len}: {path}",
                "Re-download the model.safetensors file.",
                {"path": str(path), "header_len": header_len},
            )
        if size < 8 + header_len:
            raise SmokeError(
                "MEJEPA_SMOKE_SAFETENSORS_TRUNCATED",
                f"SafeTensors file is truncated: {path}",
                "Re-download the model.safetensors file.",
                {"path": str(path), "size": size, "header_len": header_len},
            )
        header = handle.read(header_len)
    value = json.loads(header.decode("utf-8"))
    if not isinstance(value, dict):
        raise SmokeError(
            "MEJEPA_SMOKE_SAFETENSORS_HEADER_ROOT_INVALID",
            f"SafeTensors header root is not an object: {path}",
            "Re-download the model.safetensors file.",
            {"path": str(path)},
        )
    tensor_names = [key for key in value if key != "__metadata__"]
    if not tensor_names:
        raise SmokeError(
            "MEJEPA_SMOKE_SAFETENSORS_NO_TENSORS",
            f"SafeTensors header contains no tensor entries: {path}",
            "Re-download the model.safetensors file.",
            {"path": str(path)},
        )
    return {
        "header_len": header_len,
        "tensor_count": len(tensor_names),
        "tensor_sample": sorted(tensor_names)[:10],
    }


def inspect_asset_file(path: Path) -> dict[str, Any]:
    before = {"path": str(path), "exists": path.exists()}
    if not path.is_file():
        return {"before": before, "after": {"ready": False, "failure": "missing_or_not_file"}}
    stat = path.stat()
    after: dict[str, Any] = {
        "ready": stat.st_size > 0,
        "size_bytes": stat.st_size,
        "sha256": sha256_file(path),
    }
    if stat.st_size == 0:
        after["failure"] = "empty_file"
    elif path.name.endswith(".json"):
        try:
            after.update(inspect_json(path))
        except Exception as exc:
            after["ready"] = False
            after["failure"] = str(exc)
    elif path.name.endswith(".safetensors"):
        try:
            after.update(inspect_safetensors_header(path))
        except Exception as exc:
            after["ready"] = False
            after["failure"] = str(exc)
    return {"before": before, "after": after}


def inspect_model_assets(models_dir: Path) -> dict[str, Any]:
    rows = []
    failures = []
    before = {"models_dir": str(models_dir), "exists": models_dir.exists(), "is_dir": models_dir.is_dir()}
    for asset in PRODUCTION_ASSETS:
        row = {
            "slot": asset["slot"],
            "model_id": asset["model_id"],
            "repo": asset.get("repo"),
            "directory": asset.get("directory"),
            "projected_dimension": asset["projected_dimension"],
            "custom": bool(asset.get("custom", False)),
            "shared_with": asset.get("shared_with"),
            "required_files": asset["required"],
            "files": [],
            "ready": True,
        }
        if asset.get("custom"):
            row["ready"] = True
            row["note"] = "custom algorithmic embedder; no pretrained model file required"
            rows.append(row)
            continue
        model_dir = models_dir / asset["directory"]
        row["model_dir"] = str(model_dir)
        for rel in asset["required"]:
            file_report = inspect_asset_file(model_dir / rel)
            row["files"].append(file_report)
            if not file_report["after"]["ready"]:
                row["ready"] = False
                failures.append(
                    {
                        "slot": asset["slot"],
                        "model_id": asset["model_id"],
                        "path": file_report["before"]["path"],
                        "failure": file_report["after"].get("failure"),
                    }
                )
        rows.append(row)
    return {
        "before_state": before,
        "after_state": {
            "ready": not failures,
            "asset_count": len(rows),
            "failure_count": len(failures),
            "failures": failures,
            "assets": rows,
        },
    }


def inspect_gpu() -> dict[str, Any]:
    result = run(
        [
            "nvidia-smi",
            "--query-gpu=name,memory.total,memory.free,compute_cap",
            "--format=csv,noheader,nounits",
        ],
        timeout=15,
    )
    before = {"command": result.command}
    rows = []
    failures = []
    if result.exit_code != 0:
        failures.append({"failure": "nvidia-smi_failed", "stderr_tail": result.stderr[-2000:]})
    for line in [line.strip() for line in result.stdout.splitlines() if line.strip()]:
        parts = [part.strip() for part in line.split(",")]
        if len(parts) != 4:
            failures.append({"failure": "unexpected_nvidia_smi_row", "row": line})
            continue
        name, total_mib, free_mib, compute_cap = parts
        total = int(float(total_mib))
        free = int(float(free_mib))
        cc_major, cc_minor = [int(part) for part in compute_cap.split(".", 1)]
        ready = cc_major > 12 or (cc_major == 12 and cc_minor >= 0)
        ready = ready and total >= 31 * 1024
        if not ready:
            failures.append({"failure": "gpu_requirement_not_met", "name": name, "total_mib": total, "compute_cap": compute_cap})
        rows.append(
            {
                "name": name,
                "total_mib": total,
                "free_mib": free,
                "compute_capability": compute_cap,
                "ready_for_rtx5090_target": ready,
            }
        )
    return {
        "before_state": before,
        "command": result.to_json(),
        "after_state": {"ready": bool(rows) and not failures, "gpu_count": len(rows), "failures": failures, "gpus": rows},
    }


def inspect_swebench_tasks(tasks_dir: Path, expected_count: int) -> dict[str, Any]:
    before = {"tasks_dir": str(tasks_dir), "exists": tasks_dir.exists(), "is_dir": tasks_dir.is_dir()}
    files = sorted(tasks_dir.glob("*.json")) if tasks_dir.is_dir() else []
    failures = []
    ids = []
    samples = []
    for path in files:
        try:
            payload = json.loads(path.read_text(encoding="utf-8"))
        except Exception as exc:
            failures.append({"path": str(path), "failure": f"invalid_json: {exc}"})
            continue
        instance_id = payload.get("instance_id")
        if instance_id != path.stem:
            failures.append({"path": str(path), "failure": "filename_instance_id_mismatch", "instance_id": instance_id})
        for required in ["dataset", "repo", "base_commit", "problem_statement", "fail_to_pass", "oracle_patch_sha256"]:
            if required not in payload:
                failures.append({"path": str(path), "failure": f"missing_field:{required}"})
        ids.append(path.stem)
        if len(samples) < 3:
            samples.append(
                {
                    "path": str(path),
                    "instance_id": path.stem,
                    "repo": payload.get("repo"),
                    "fail_to_pass_count": len(payload.get("fail_to_pass", [])) if isinstance(payload.get("fail_to_pass"), list) else None,
                    "problem_sha256": hashlib.sha256(str(payload.get("problem_statement", "")).encode("utf-8")).hexdigest(),
                }
            )
    duplicate_ids = sorted({item for item in ids if ids.count(item) > 1})
    if duplicate_ids:
        failures.append({"failure": "duplicate_instance_ids", "ids": duplicate_ids[:20]})
    if len(files) != expected_count:
        failures.append({"failure": "manifest_count_mismatch", "expected": expected_count, "actual": len(files)})
    return {
        "before_state": before,
        "after_state": {
            "ready": not failures,
            "manifest_count": len(files),
            "expected_count": expected_count,
            "failure_count": len(failures),
            "failures": failures,
            "sample_manifests": samples,
            "id_sha256": hashlib.sha256("\n".join(ids).encode("utf-8")).hexdigest(),
        },
    }


def run_edge_cases() -> list[dict[str, Any]]:
    edge_cases: list[dict[str, Any]] = []
    with tempfile.TemporaryDirectory(prefix="mejepa-embedder-smoke-edge-") as tmp_raw:
        tmp = Path(tmp_raw)

        missing_root = tmp / "missing-models"
        before = {"models_dir": str(missing_root), "exists": missing_root.exists()}
        report = inspect_model_assets(missing_root)
        after = {"ready": report["after_state"]["ready"], "failure_count": report["after_state"]["failure_count"]}
        edge_cases.append(
            {
                "name": "missing_model_root",
                "expected": "fail_closed_with_missing_asset_failures",
                "before_state": before,
                "after_state": after,
                "passed": (not after["ready"]) and after["failure_count"] > 0,
            }
        )
        print(f"EDGE missing_model_root BEFORE={before} AFTER={after}")

        bad_st = tmp / "bad.safetensors"
        bad_st.write_bytes(b"not-a-safetensors-file")
        before = {"path": str(bad_st), "size_bytes": bad_st.stat().st_size}
        report = inspect_asset_file(bad_st)
        after = report["after"]
        edge_cases.append(
            {
                "name": "invalid_safetensors_header",
                "expected": "fail_closed_with_safetensors_header_error",
                "before_state": before,
                "after_state": after,
                "passed": not after["ready"] and "failure" in after,
            }
        )
        print(f"EDGE invalid_safetensors_header BEFORE={before} AFTER={after}")

        bad_tasks = tmp / "tasks"
        bad_tasks.mkdir()
        (bad_tasks / "bad__task-1.json").write_text("{not-json", encoding="utf-8")
        before = {"tasks_dir": str(bad_tasks), "files": sorted(path.name for path in bad_tasks.glob('*'))}
        report = inspect_swebench_tasks(bad_tasks, expected_count=1)
        after = {
            "ready": report["after_state"]["ready"],
            "failure_count": report["after_state"]["failure_count"],
            "failures": report["after_state"]["failures"],
        }
        edge_cases.append(
            {
                "name": "invalid_swebench_manifest_json",
                "expected": "fail_closed_with_manifest_json_error",
                "before_state": before,
                "after_state": after,
                "passed": not after["ready"] and after["failure_count"] > 0,
            }
        )
        print(f"EDGE invalid_swebench_manifest_json BEFORE={before} AFTER={after}")
    return edge_cases


def main() -> int:
    parser = argparse.ArgumentParser(description="ME-JEPA embedder smoke full-state verification")
    parser.add_argument("--models-dir", type=Path, default=DEFAULT_MODELS_DIR)
    parser.add_argument("--tasks-dir", type=Path, default=DEFAULT_TASKS_DIR)
    parser.add_argument("--evidence-root", type=Path, default=DEFAULT_EVIDENCE_ROOT)
    parser.add_argument("--expected-swebench-lite-count", type=int, default=EXPECTED_SWEBENCH_LITE_COUNT)
    args = parser.parse_args()

    evidence_path = args.evidence_root / "embedder_smoke_preflight.json"
    payload: dict[str, Any] = {
        "source_of_truth": {
            "evidence_json": str(evidence_path),
            "models_dir": str(args.models_dir),
            "tasks_dir": str(args.tasks_dir),
        },
        "started_at": utc_now(),
        "checks": {},
        "edge_cases": [],
    }

    try:
        payload["checks"]["gpu"] = inspect_gpu()
        payload["checks"]["swebench_tasks"] = inspect_swebench_tasks(args.tasks_dir, args.expected_swebench_lite_count)
        payload["checks"]["model_assets"] = inspect_model_assets(args.models_dir)
        payload["edge_cases"] = run_edge_cases()

        failures = []
        for name, check in payload["checks"].items():
            if not check["after_state"]["ready"]:
                failures.append({"check": name, "after_state": check["after_state"]})
        for edge in payload["edge_cases"]:
            if not edge["passed"]:
                failures.append({"edge_case": edge["name"], "after_state": edge["after_state"]})

        payload["completed_at"] = utc_now()
        payload["ok"] = not failures
        payload["failure_count"] = len(failures)
        payload["failures"] = failures
        write_json_checked(evidence_path, payload)

        readback = json.loads(evidence_path.read_text(encoding="utf-8"))
        print(
            json.dumps(
                {
                    "source_of_truth": str(evidence_path),
                    "ok": readback["ok"],
                    "failure_count": readback["failure_count"],
                    "gpu_ready": readback["checks"]["gpu"]["after_state"]["ready"],
                    "swebench_ready": readback["checks"]["swebench_tasks"]["after_state"]["ready"],
                    "model_assets_ready": readback["checks"]["model_assets"]["after_state"]["ready"],
                    "model_asset_failure_count": readback["checks"]["model_assets"]["after_state"]["failure_count"],
                },
                indent=2,
                sort_keys=True,
            )
        )
        if failures:
            return 1
        return 0
    except SmokeError as exc:
        payload["completed_at"] = utc_now()
        payload["ok"] = False
        payload["error"] = exc.to_json()
        write_json_checked(evidence_path, payload)
        print(json.dumps({"source_of_truth": str(evidence_path), **exc.to_json()}, indent=2, sort_keys=True), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
