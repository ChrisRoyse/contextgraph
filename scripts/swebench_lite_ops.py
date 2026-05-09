#!/usr/bin/env python3
"""Operational checks for ContextGraph's SWE-bench Lite Phase 0 gate.

This script intentionally uses the official SWE-bench harness and the real
SWE-bench Lite dataset. It does not synthesize oracle success. Every command
writes a JSON evidence file that can be read independently after the process
exits.
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import time
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_SWEBENCH_ROOT = Path("/home/cabdru/.cache/contextgraph-swebench/SWE-bench")
DEFAULT_SWEBENCH_VENV = Path("/home/cabdru/.cache/contextgraph-swebench/venv")
DEFAULT_TASKS_DIR = REPO_ROOT / "tasks" / "swebench-lite"
DEFAULT_EVIDENCE_ROOT = Path("/tmp/contextgraph-swebench-lite-fsv")
DEFAULT_DATASET = "princeton-nlp/SWE-bench_Lite"
DEFAULT_SPLIT = "test"
EXPECTED_LITE_COUNT = 300
DEFAULT_SMOKE_TASK = "sympy__sympy-20590"


class SwebenchOpsError(RuntimeError):
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


def tail(text: str, limit: int = 6000) -> str:
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
                "stdout_tail": tail(exc.stdout or ""),
                "stderr_tail": tail(exc.stderr or ""),
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
            "Inspect stdout_tail/stderr_tail and fix the failing dependency before continuing.",
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
            "Check the filesystem for partial writes or unexpected concurrent writers.",
            {"path": str(path)},
        )


def parse_json_tail(result: CommandResult, field: str = "stdout_tail") -> Any:
    raw = getattr(result, field)
    lines = [line for line in raw.splitlines() if line.strip()]
    for line in reversed(lines):
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


def official_image_ref(instance_id: str, namespace: str = "swebench", arch: str = "x86_64", tag: str = "latest") -> str:
    validate_instance_id(instance_id)
    remote_instance = instance_id.lower().replace("__", "_1776_")
    return f"{namespace}/sweb.eval.{arch}.{remote_instance}:{tag}"


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


def python_bin(venv: Path) -> Path:
    return venv / "bin" / "python"


def probe_python_env(venv: Path, dataset_name: str, split: str, timeout: int) -> dict[str, Any]:
    python = python_bin(venv)
    ensure_file(python, "SWEBENCH_VENV_PYTHON_MISSING", "SWE-bench venv Python")
    snippet = r"""
import json
import logging
logging.getLogger("httpx").setLevel(logging.WARNING)
logging.getLogger("huggingface_hub").setLevel(logging.WARNING)
import datasets
import docker
import swebench
from datasets import load_dataset
ds = load_dataset(DATASET_NAME, split=SPLIT)
print(json.dumps({
    "swebench_version": getattr(swebench, "__version__", "unknown"),
    "datasets_version": datasets.__version__,
    "docker_py_version": docker.__version__,
    "dataset_name": DATASET_NAME,
    "split": SPLIT,
    "count": len(ds),
    "ids": [row["instance_id"] for row in ds],
}, sort_keys=True))
""".replace("DATASET_NAME", repr(dataset_name)).replace("SPLIT", repr(split))
    env = os.environ.copy()
    env.setdefault("HF_HUB_DISABLE_PROGRESS_BARS", "1")
    result = run([str(python), "-c", snippet], timeout=timeout, env=env, stdout_limit=120000)
    payload = parse_json_tail(result)
    if payload["count"] != EXPECTED_LITE_COUNT:
        raise SwebenchOpsError(
            "SWEBENCH_DATASET_COUNT_MISMATCH",
            f"expected {EXPECTED_LITE_COUNT} dataset rows, got {payload['count']}",
            "Verify the dataset name/split and Hugging Face cache.",
            payload,
        )
    return {"probe": result.to_json(), "payload": payload}


def docker_info(timeout: int) -> dict[str, Any]:
    version = run(["docker", "version"], timeout=timeout)
    info = run(["docker", "info", "--format", "{{json .}}"], timeout=timeout, stdout_limit=500000)
    parsed_info = parse_json_tail(info)
    hello = run(["docker", "run", "--rm", "hello-world"], timeout=timeout)
    return {
        "version": version.to_json(),
        "info": parsed_info,
        "hello_world": hello.to_json(),
    }


def local_swebench_images() -> dict[str, dict[str, str]]:
    result = run(
        ["docker", "images", "--format", "{{.Repository}}:{{.Tag}}\t{{.ID}}\t{{.Size}}"],
        timeout=120,
        stdout_limit=300000,
    )
    images = {}
    for line in result.stdout_tail.splitlines():
        parts = line.split("\t")
        if len(parts) != 3:
            continue
        ref, image_id, size = parts
        if ref.startswith("swebench/"):
            images[ref] = {"id": image_id, "size": size}
    return images


def inspect_image(ref: str, timeout: int = 120) -> dict[str, Any]:
    result = run(["docker", "image", "inspect", ref], timeout=timeout)
    payload = json.loads(result.stdout_tail)
    if not payload:
        raise SwebenchOpsError(
            "SWEBENCH_DOCKER_IMAGE_INSPECT_EMPTY",
            f"docker image inspect returned no rows for {ref}",
            "Pull the expected image and rerun inspection.",
            {"image": ref},
        )
    image = payload[0]
    return {
        "id": image.get("Id"),
        "repo_tags": image.get("RepoTags", []),
        "size_bytes": image.get("Size"),
        "created": image.get("Created"),
    }


def check_remote_manifest(ref: str, timeout: int) -> dict[str, Any]:
    result = run(["docker", "manifest", "inspect", ref], timeout=timeout)
    payload = json.loads(result.stdout_tail)
    return {
        "image": ref,
        "schema_version": payload.get("schemaVersion"),
        "media_type": payload.get("mediaType"),
        "layers": len(payload.get("layers", [])),
        "manifests": len(payload.get("manifests", [])),
    }


def summarize_image_status(task_ids: list[str], namespace: str, arch: str, tag: str) -> dict[str, Any]:
    local = local_swebench_images()
    expected = [official_image_ref(task_id, namespace, arch, tag) for task_id in task_ids]
    present = [ref for ref in expected if ref in local]
    missing = [ref for ref in expected if ref not in local]
    return {
        "expected_count": len(expected),
        "present_count": len(present),
        "missing_count": len(missing),
        "present_images": {ref: local[ref] for ref in present},
        "missing_images_sample": missing[:20],
    }


def command_preflight(args: argparse.Namespace) -> dict[str, Any]:
    before = {
        "swebench_root_exists": args.swebench_root.is_dir(),
        "venv_python_exists": python_bin(args.venv).is_file(),
        "tasks_dir_exists": args.tasks_dir.is_dir(),
    }
    ensure_dir(args.swebench_root, "SWEBENCH_SOURCE_TREE_MISSING", "official SWE-bench source tree")
    manifests = load_task_manifests(args.tasks_dir)
    task_ids = sorted(manifests)
    python_probe = probe_python_env(args.venv, args.dataset_name, args.split, args.timeout)
    dataset_ids = set(python_probe["payload"]["ids"])
    manifest_ids = set(task_ids)
    if dataset_ids != manifest_ids:
        raise SwebenchOpsError(
            "SWEBENCH_DATASET_MANIFEST_MISMATCH",
            "checked-in SWE-bench Lite manifests do not match the official dataset ids",
            "Regenerate tasks/swebench-lite from the official dataset before corpus generation.",
            {
                "dataset_only": sorted(dataset_ids - manifest_ids)[:20],
                "manifest_only": sorted(manifest_ids - dataset_ids)[:20],
                "dataset_count": len(dataset_ids),
                "manifest_count": len(manifest_ids),
            },
        )

    sample_image = official_image_ref(args.sample_task, args.namespace, args.arch, args.tag)
    remote_manifest = check_remote_manifest(sample_image, args.timeout)
    docker = docker_info(args.timeout)
    image_status = summarize_image_status(task_ids, args.namespace, args.arch, args.tag)
    if args.require_local_images and image_status["missing_count"] != 0:
        raise SwebenchOpsError(
            "SWEBENCH_LOCAL_IMAGES_MISSING",
            f"{image_status['missing_count']} official SWE-bench Lite images are not local",
            "Run pull-images for the missing official images or disable require-local-images for readiness-only checks.",
            image_status,
        )

    return {
        "ok": True,
        "checked_at": utc_now(),
        "source_of_truth": {
            "official_dataset": args.dataset_name,
            "official_split": args.split,
            "task_manifests": str(args.tasks_dir),
            "swebench_root": str(args.swebench_root),
            "venv_python": str(python_bin(args.venv)),
            "docker_desktop": "docker info",
        },
        "before_state": before,
        "after_state": {
            "task_manifest_count": len(manifests),
            "dataset_count": python_probe["payload"]["count"],
            "dataset_first_id": python_probe["payload"]["ids"][0],
            "dataset_last_id": python_probe["payload"]["ids"][-1],
            "sample_image": sample_image,
            "remote_manifest": remote_manifest,
            "docker": {
                "server_version": docker["info"].get("ServerVersion"),
                "operating_system": docker["info"].get("OperatingSystem"),
                "architecture": docker["info"].get("Architecture"),
                "ncpu": docker["info"].get("NCPU"),
                "mem_total": docker["info"].get("MemTotal"),
                "docker_root_dir": docker["info"].get("DockerRootDir"),
                "hello_world_exit_code": docker["hello_world"]["exit_code"],
            },
            "image_status": image_status,
        },
        "python_probe": python_probe,
        "passes": True,
    }


def command_image_status(args: argparse.Namespace) -> dict[str, Any]:
    manifests = load_task_manifests(args.tasks_dir)
    task_ids = sorted(manifests)
    status = summarize_image_status(task_ids, args.namespace, args.arch, args.tag)
    payload = {
        "ok": True,
        "checked_at": utc_now(),
        "source_of_truth": {
            "task_manifests": str(args.tasks_dir),
            "docker_images": "docker images --format",
        },
        "image_naming": {
            "rule": "official SWE-bench TestSpec.instance_image_key replaces '__' with '_1776_' for remote namespace images",
            "sample": {
                args.sample_task: official_image_ref(args.sample_task, args.namespace, args.arch, args.tag)
            },
        },
        "after_state": status,
        "passes": True,
    }
    if args.require_all and status["missing_count"] != 0:
        raise SwebenchOpsError(
            "SWEBENCH_LOCAL_IMAGES_MISSING",
            f"{status['missing_count']} official SWE-bench Lite images are not local",
            "Run pull-images for the missing official images before claiming Phase 0 is operationally complete.",
            status,
        )
    return payload


def command_pull_images(args: argparse.Namespace) -> dict[str, Any]:
    manifests = load_task_manifests(args.tasks_dir)
    task_ids = sorted(manifests)
    if args.instance_ids:
        requested = args.instance_ids
        unknown = sorted(set(requested) - set(task_ids))
        if unknown:
            raise SwebenchOpsError(
                "SWEBENCH_UNKNOWN_INSTANCE_ID",
                f"requested instance ids are not in SWE-bench Lite: {unknown[:5]}",
                "Use ids from tasks/swebench-lite or the official Lite dataset.",
                {"unknown": unknown},
            )
        task_ids = requested
    if args.limit is not None:
        if args.limit <= 0:
            raise SwebenchOpsError(
                "SWEBENCH_INVALID_LIMIT",
                f"pull limit must be positive, got {args.limit}",
                "Pass a positive --limit or omit it to pull every requested image.",
            )
        task_ids = task_ids[: args.limit]

    before = summarize_image_status(task_ids, args.namespace, args.arch, args.tag)
    pulls = []
    for task_id in task_ids:
        ref = official_image_ref(task_id, args.namespace, args.arch, args.tag)
        pulls.append(run(["docker", "pull", ref], timeout=args.timeout).to_json())
    after = summarize_image_status(task_ids, args.namespace, args.arch, args.tag)
    if after["missing_count"] != 0:
        raise SwebenchOpsError(
            "SWEBENCH_PULL_INCOMPLETE",
            "one or more requested official images are still missing after docker pull",
            "Inspect the pull logs and rerun after resolving registry/network/storage errors.",
            after,
        )
    return {
        "ok": True,
        "checked_at": utc_now(),
        "source_of_truth": {
            "task_manifests": str(args.tasks_dir),
            "docker_images": "docker image inspect",
        },
        "before_state": before,
        "trigger": {"pulled_task_ids": task_ids, "count": len(task_ids)},
        "pulls": pulls,
        "after_state": after,
        "passes": True,
    }


def find_gold_report(run_root: Path, run_id: str) -> Path:
    expected = run_root / f"gold.{run_id}.json"
    ensure_file(expected, "SWEBENCH_GOLD_REPORT_MISSING", "SWE-bench gold run report")
    return expected


def command_gold_smoke(args: argparse.Namespace) -> dict[str, Any]:
    manifests = load_task_manifests(args.tasks_dir)
    if args.instance_id not in manifests:
        raise SwebenchOpsError(
            "SWEBENCH_UNKNOWN_INSTANCE_ID",
            f"instance id is not in SWE-bench Lite manifests: {args.instance_id}",
            "Use one of the checked-in SWE-bench Lite task ids.",
            {"instance_id": args.instance_id},
        )
    python = python_bin(args.venv)
    ensure_file(python, "SWEBENCH_VENV_PYTHON_MISSING", "SWE-bench venv Python")

    run_id = args.run_id or f"contextgraph-gold-smoke-{datetime.now(timezone.utc).strftime('%Y%m%d%H%M%S')}"
    run_root = args.evidence_root / "gold-smoke" / run_id
    if run_root.exists() and any(run_root.iterdir()):
        raise SwebenchOpsError(
            "SWEBENCH_RUN_ROOT_NOT_EMPTY",
            f"gold-smoke run root already exists and is non-empty: {run_root}",
            "Use a fresh run id so stale reports cannot mask a failed run.",
            {"run_root": str(run_root)},
        )
    run_root.mkdir(parents=True, exist_ok=True)

    image = official_image_ref(args.instance_id, args.namespace, args.arch, args.tag)
    before_images = summarize_image_status([args.instance_id], args.namespace, args.arch, args.tag)
    command = [
        str(python),
        "-m",
        "swebench.harness.run_evaluation",
        "--dataset_name",
        args.dataset_name,
        "--split",
        args.split,
        "--predictions_path",
        "gold",
        "--max_workers",
        "1",
        "--instance_ids",
        args.instance_id,
        "--run_id",
        run_id,
        "--timeout",
        str(args.instance_timeout),
        "--cache_level",
        "instance",
        "--clean",
        "False",
        "--namespace",
        args.namespace,
        "--report_dir",
        str(run_root),
    ]
    trigger = run(command, cwd=run_root, timeout=args.timeout)

    report_path = find_gold_report(run_root, run_id)
    report = json.loads(report_path.read_text(encoding="utf-8"))
    instance_report_path = (
        run_root
        / "logs"
        / "run_evaluation"
        / run_id
        / "gold"
        / args.instance_id
        / "report.json"
    )
    run_log_path = instance_report_path.with_name("run_instance.log")
    test_output_path = instance_report_path.with_name("test_output.txt")
    patch_path = instance_report_path.with_name("patch.diff")
    for path, label in [
        (instance_report_path, "instance report"),
        (run_log_path, "instance run log"),
        (test_output_path, "test output"),
        (patch_path, "gold patch"),
    ]:
        ensure_file(path, "SWEBENCH_GOLD_ARTIFACT_MISSING", label)
    instance_report = json.loads(instance_report_path.read_text(encoding="utf-8"))
    instance_row = instance_report.get(args.instance_id)
    if not instance_row or not instance_row.get("resolved"):
        raise SwebenchOpsError(
            "SWEBENCH_GOLD_SMOKE_UNRESOLVED",
            f"official gold smoke did not resolve {args.instance_id}",
            "Inspect the official run logs and Docker resource state before using the oracle.",
            {"report": report, "instance_report": instance_report},
        )
    if args.instance_id not in report.get("resolved_ids", []):
        raise SwebenchOpsError(
            "SWEBENCH_GOLD_SUMMARY_MISMATCH",
            f"overall report does not list {args.instance_id} as resolved",
            "Inspect the run report and per-instance report for harness inconsistency.",
            {"report": report, "instance_report": instance_report},
        )

    after_images = summarize_image_status([args.instance_id], args.namespace, args.arch, args.tag)
    image_inspect = inspect_image(image)
    return {
        "ok": True,
        "checked_at": utc_now(),
        "source_of_truth": {
            "official_dataset": args.dataset_name,
            "official_split": args.split,
            "official_image": image,
            "run_root": str(run_root),
            "overall_report": str(report_path),
            "instance_report": str(instance_report_path),
            "run_log": str(run_log_path),
            "test_output": str(test_output_path),
            "patch": str(patch_path),
            "docker_image_inspect": image,
        },
        "before_state": {
            "image_status": before_images,
        },
        "trigger": {
            "command": command,
            "result": trigger.to_json(),
        },
        "after_state": {
            "overall_report": {
                "completed_instances": report.get("completed_instances"),
                "resolved_instances": report.get("resolved_instances"),
                "error_instances": report.get("error_instances"),
                "incomplete_instances": report.get("incomplete_instances"),
                "resolved_ids": report.get("resolved_ids", []),
            },
            "instance_report": instance_row,
            "image_status": after_images,
            "image_inspect": image_inspect,
        },
        "passes": True,
    }


def add_common(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--swebench-root", type=Path, default=DEFAULT_SWEBENCH_ROOT)
    parser.add_argument("--venv", type=Path, default=DEFAULT_SWEBENCH_VENV)
    parser.add_argument("--tasks-dir", type=Path, default=DEFAULT_TASKS_DIR)
    parser.add_argument("--evidence-root", type=Path, default=DEFAULT_EVIDENCE_ROOT)
    parser.add_argument("--dataset-name", default=DEFAULT_DATASET)
    parser.add_argument("--split", default=DEFAULT_SPLIT)
    parser.add_argument("--namespace", default="swebench")
    parser.add_argument("--arch", default="x86_64")
    parser.add_argument("--tag", default="latest")
    parser.add_argument("--timeout", type=int, default=600)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)

    preflight = sub.add_parser("preflight", help="Verify official harness, Docker, dataset, manifests, and image naming.")
    add_common(preflight)
    preflight.add_argument("--sample-task", default=DEFAULT_SMOKE_TASK)
    preflight.add_argument("--require-local-images", action="store_true")
    preflight.set_defaults(func=command_preflight, evidence_name="preflight-evidence.json")

    status = sub.add_parser("image-status", help="Inspect which official SWE-bench Lite images are present locally.")
    add_common(status)
    status.add_argument("--sample-task", default=DEFAULT_SMOKE_TASK)
    status.add_argument("--require-all", action="store_true")
    status.set_defaults(func=command_image_status, evidence_name="image-status-evidence.json")

    pull = sub.add_parser("pull-images", help="Pull official SWE-bench Lite images by real instance id.")
    add_common(pull)
    pull.add_argument("--instance-ids", nargs="*")
    pull.add_argument("--limit", type=int)
    pull.set_defaults(func=command_pull_images, evidence_name="pull-images-evidence.json")

    smoke = sub.add_parser("gold-smoke", help="Run one official SWE-bench Lite gold evaluation through Docker.")
    add_common(smoke)
    smoke.add_argument("--instance-id", default=DEFAULT_SMOKE_TASK)
    smoke.add_argument("--run-id")
    smoke.add_argument("--instance-timeout", type=int, default=1800)
    smoke.set_defaults(func=command_gold_smoke, evidence_name="gold-smoke-evidence.json")

    return parser


def main() -> int:
    parser = build_parser()
    args = parser.parse_args()
    evidence_path = args.evidence_root / args.evidence_name
    try:
        payload = args.func(args)
        payload["evidence_path"] = str(evidence_path)
        write_json_checked(evidence_path, payload)
        print(json.dumps(payload, indent=2, sort_keys=True))
        return 0
    except SwebenchOpsError as exc:
        payload = {
            "checked_at": utc_now(),
            "command": args.command,
            "evidence_path": str(evidence_path),
            **exc.to_json(),
        }
        write_json_checked(evidence_path, payload)
        print(json.dumps(payload, indent=2, sort_keys=True), file=sys.stderr)
        return 2
    except Exception as exc:  # Fail closed for unexpected defects too.
        payload = {
            "checked_at": utc_now(),
            "command": args.command,
            "evidence_path": str(evidence_path),
            "ok": False,
            "error_code": "SWEBENCH_UNEXPECTED_ERROR",
            "message": str(exc),
            "remediation": "Fix the script defect or unexpected environment issue before continuing.",
            "details": {"type": type(exc).__name__},
        }
        write_json_checked(evidence_path, payload)
        print(json.dumps(payload, indent=2, sort_keys=True), file=sys.stderr)
        return 3


if __name__ == "__main__":
    sys.exit(main())
