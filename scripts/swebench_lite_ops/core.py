from __future__ import annotations

import hashlib
import json
import math
import os
import subprocess
import time
from collections import Counter
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[2]
DEFAULT_SWEBENCH_ROOT = Path("/zfs/archive/contextgraph/cache/contextgraph-swebench/SWE-bench")
DEFAULT_SWEBENCH_VENV = Path("/zfs/archive/contextgraph/cache/contextgraph-swebench/venv")
DEFAULT_TASKS_DIR = REPO_ROOT / "tasks" / "swebench-lite"
DEFAULT_EVIDENCE_ROOT = Path("/zfs/archive/contextgraph/swebench-artifacts/swebench-lite-fsv")
DEFAULT_CORPUS_INDEX_JSON = Path("/zfs/archive/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/index.json")
DEFAULT_CONSTELLATION_TRAINING_ARRAY_ROOT = Path(
    "/zfs/archive/contextgraph/state/training-arrays/constellation-training-arrays-v1-realinput"
)
DEFAULT_DATASET = "princeton-nlp/SWE-bench_Lite"
DEFAULT_SPLIT = "test"
DEFAULT_SMOKE_TASK = "sympy__sympy-20590"
EXPECTED_LITE_COUNT = 300
CURRENT_VALID_BASELINE_CORRELATION = 0.7942190170288086
HISTORICAL_BASELINE_CORRELATION = CURRENT_VALID_BASELINE_CORRELATION
EPS = 1.0e-12
FORBIDDEN_SYNTHETIC_BASELINE_CORRELATIONS = (0.8666666746139526, 0.866667)
REQUIRED_CONSTELLATION_TRAINING_ARTIFACTS = (
    "constellation_training_certificate.json",
    "constellation_training_rows.jsonl",
    "constellation_slot_vectors.jsonl",
    "constellation_pairwise_observations.jsonl",
    "constellation_pairwise_relationships.jsonl",
)


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


def normalize_category(value: str) -> str:
    return value.strip().lower().replace("-", "_")


def finite_float(value: float) -> float:
    out = float(value)
    if not math.isfinite(out):
        raise SwebenchOpsError(
            "SWEBENCH_NONFINITE_FLOAT",
            f"non-finite float: {value!r}",
            "Fix the source metric before writing evidence.",
            {"value": repr(value)},
        )
    return out


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return "sha256:" + digest.hexdigest()


def forbidden_synthetic_baseline_hits(value: Any, *, path: str = "$") -> list[dict[str, Any]]:
    hits: list[dict[str, Any]] = []
    if isinstance(value, bool) or value is None:
        return hits
    if isinstance(value, (int, float)):
        numeric = float(value)
        for forbidden in FORBIDDEN_SYNTHETIC_BASELINE_CORRELATIONS:
            if abs(numeric - forbidden) <= 1.0e-9:
                hits.append({"path": path, "value": numeric, "forbidden_value": forbidden})
                break
        return hits
    if isinstance(value, dict):
        for key, child in value.items():
            hits.extend(forbidden_synthetic_baseline_hits(child, path=f"{path}.{key}"))
    elif isinstance(value, list):
        for index, child in enumerate(value):
            hits.extend(forbidden_synthetic_baseline_hits(child, path=f"{path}[{index}]"))
    return hits


def validate_constellation_training_artifact_root(root: Path, *, consumer: str) -> dict[str, Any]:
    if not root.exists():
        raise SwebenchOpsError(
            "SWEBENCH_CONSTELLATION_TRAINING_ROOT_MISSING",
            f"constellation training artifact root missing: {root}",
            "Materialize the sanitized post-fix training-array root on aiwonder before consuming it.",
            {"root": str(root), "consumer": consumer},
        )
    if not root.is_dir():
        raise SwebenchOpsError(
            "SWEBENCH_CONSTELLATION_TRAINING_ROOT_NOT_DIR",
            f"constellation training artifact root is not a directory: {root}",
            "Point --artifact-root at the sanitized training-array directory.",
            {"root": str(root), "consumer": consumer},
        )
    file_sizes: dict[str, int] = {}
    hashes: dict[str, str] = {}
    for name in REQUIRED_CONSTELLATION_TRAINING_ARTIFACTS:
        path = root / name
        if not path.is_file():
            raise SwebenchOpsError(
                "SWEBENCH_CONSTELLATION_TRAINING_ARTIFACT_MISSING",
                f"required constellation training artifact missing: {path}",
                "Rebuild or copy the sanitized training-array artifact set before running this consumer.",
                {"root": str(root), "missing": name, "consumer": consumer},
            )
        size = path.stat().st_size
        if size <= 0:
            raise SwebenchOpsError(
                "SWEBENCH_CONSTELLATION_TRAINING_ARTIFACT_EMPTY",
                f"required constellation training artifact is empty: {path}",
                "Replace the empty artifact with the aiwonder source-of-truth file.",
                {"root": str(root), "empty": name, "consumer": consumer},
            )
        file_sizes[name] = size
        hashes[name] = sha256_file(path)

    certificate_path = root / "constellation_training_certificate.json"
    try:
        certificate = json.loads(certificate_path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise SwebenchOpsError(
            "SWEBENCH_CONSTELLATION_TRAINING_CERTIFICATE_BAD_JSON",
            f"constellation training certificate is not valid JSON: {certificate_path}",
            "Regenerate the sanitized certificate from the real training-array source.",
            {"path": str(certificate_path), "consumer": consumer, "error": str(exc)},
        ) from exc

    if certificate.get("passes") is not True:
        raise SwebenchOpsError(
            "SWEBENCH_CONSTELLATION_TRAINING_CERTIFICATE_NOT_PASSING",
            f"constellation training certificate is not passing: {certificate_path}",
            "Use only a passing post-fix training-array certificate.",
            {"path": str(certificate_path), "consumer": consumer},
        )
    policy = certificate.get("objective_policy") or {}
    required_false = (
        "flat_vector_concat_used",
        "human_intent_inference_used",
        "synthetic_seed_features_used",
    )
    for key in required_false:
        if policy.get(key) is not False:
            raise SwebenchOpsError(
                "SWEBENCH_CONSTELLATION_TRAINING_POLICY_INVALID",
                f"constellation training policy must set {key}=false",
                "Reject flat/synthetic/intent-derived training-array certificates on active consumers.",
                {"path": str(certificate_path), "consumer": consumer, "key": key, "value": policy.get(key)},
            )
    if policy.get("slot_identity_preserved") is not True:
        raise SwebenchOpsError(
            "SWEBENCH_CONSTELLATION_TRAINING_SLOT_IDENTITY_INVALID",
            "constellation training policy does not preserve slot identity",
            "Use the sanitized post-fix array root that preserves per-embedder slots.",
            {"path": str(certificate_path), "consumer": consumer},
        )
    forbidden_hits = forbidden_synthetic_baseline_hits(certificate)
    if forbidden_hits:
        raise SwebenchOpsError(
            "SWEBENCH_CONSTELLATION_TRAINING_FORBIDDEN_BASELINE",
            f"constellation training certificate still contains invalid synthetic baseline values: {certificate_path}",
            "Sanitize or replace the certificate; active consumers must not propagate the invalid 0.866667 baseline.",
            {"path": str(certificate_path), "consumer": consumer, "hits": forbidden_hits[:20]},
        )

    baseline_context = (certificate.get("baseline_ladder") or {}).get("current_post_fix_ship_gate_context")
    return {
        "root": str(root),
        "consumer": consumer,
        "certificate": certificate,
        "hashes": hashes,
        "file_sizes": file_sizes,
        "objective_policy": policy,
        "baseline_context": baseline_context,
        "forbidden_baseline_hits": [],
    }


def load_oracle_index(path: Path) -> tuple[dict[tuple[str, str], dict[str, Any]], dict[str, Any]]:
    if not path.exists():
        raise SwebenchOpsError(
            "SWEBENCH_ORACLE_INDEX_MISSING",
            f"oracle index missing: {path}",
            "Materialize the aiwonder SWE-bench corpus index before running this command.",
            {"path": str(path)},
        )
    payload = json.loads(path.read_text())
    entries = list(payload.get("entries") or [])
    if not entries:
        raise SwebenchOpsError(
            "SWEBENCH_ORACLE_INDEX_EMPTY",
            f"oracle index has no entries: {path}",
            "Rebuild the corpus index before running this command.",
            {"path": str(path)},
        )
    by_key: dict[tuple[str, str], dict[str, Any]] = {}
    oracle_counts = Counter()
    category_counts = Counter()
    bucket_counts = Counter()
    for entry in entries:
        task_id = str(entry.get("task_id") or "")
        category = normalize_category(str(entry.get("category") or ""))
        if not task_id or not category:
            raise SwebenchOpsError(
                "SWEBENCH_ORACLE_INDEX_KEY_MISSING",
                "oracle index entry is missing task_id or category",
                "Repair the corpus index row before using it as source of truth.",
                {"entry": entry},
            )
        key = (task_id, category)
        if key in by_key:
            raise SwebenchOpsError(
                "SWEBENCH_ORACLE_INDEX_DUPLICATE_KEY",
                f"duplicate oracle index key: {task_id}:{category}",
                "Deduplicate the corpus index before using it as source of truth.",
                {"task_id": task_id, "category": category},
            )
        if not isinstance(entry.get("oracle_all_passed"), bool):
            raise SwebenchOpsError(
                "SWEBENCH_ORACLE_BOOL_MISSING",
                f"oracle_all_passed missing or not bool for {task_id}:{category}",
                "Repair the corpus index row before using it as source of truth.",
                {"task_id": task_id, "category": category},
            )
        by_key[key] = entry
        oracle_counts["pass" if entry["oracle_all_passed"] else "fail_or_error"] += 1
        category_counts[category] += 1
        bucket_counts[str(entry.get("bucket") or "unknown")] += 1
    return by_key, {
        "path": str(path),
        "sha256": sha256_file(path),
        "complete": bool(payload.get("complete")),
        "entry_count": len(entries),
        "oracle_counts": dict(sorted(oracle_counts.items())),
        "category_counts": dict(sorted(category_counts.items())),
        "bucket_counts": dict(sorted(bucket_counts.items())),
        "corpus_sha256": payload.get("corpus_sha256"),
    }


def brier(labels: list[int], probabilities: list[float]) -> float:
    if len(labels) != len(probabilities) or not labels:
        raise SwebenchOpsError(
            "SWEBENCH_METRIC_LABEL_PROB_MISMATCH",
            "labels/probabilities must be same non-zero length",
            "Fix the metric inputs before writing evidence.",
            {"labels": len(labels), "probabilities": len(probabilities)},
        )
    return finite_float(sum((float(p) - float(y)) ** 2 for y, p in zip(labels, probabilities)) / len(labels))


def accuracy_at_half(labels: list[int], probabilities: list[float]) -> float:
    if len(labels) != len(probabilities) or not labels:
        raise SwebenchOpsError(
            "SWEBENCH_METRIC_LABEL_PROB_MISMATCH",
            "labels/probabilities must be same non-zero length",
            "Fix the metric inputs before writing evidence.",
            {"labels": len(labels), "probabilities": len(probabilities)},
        )
    hits = sum(1 for label, prob in zip(labels, probabilities) if (1 if prob >= 0.5 else 0) == int(label))
    return finite_float(hits / len(labels))


def pearson_or_none(labels: list[int], probabilities: list[float]) -> tuple[float | None, str | None]:
    if len(labels) < 3:
        return None, "TOO_FEW_ROWS"
    x_mean = sum(float(value) for value in labels) / len(labels)
    y_mean = sum(float(value) for value in probabilities) / len(probabilities)
    x_var = sum((float(value) - x_mean) ** 2 for value in labels)
    y_var = sum((float(value) - y_mean) ** 2 for value in probabilities)
    if x_var <= EPS or y_var <= EPS:
        return None, "DEGENERATE_VARIANCE"
    cov = sum((float(x) - x_mean) * (float(y) - y_mean) for x, y in zip(labels, probabilities))
    return finite_float(cov / math.sqrt(x_var * y_var)), None


def calibration_ece(labels: list[int], probabilities: list[float], bins: int) -> float:
    if bins <= 0:
        raise SwebenchOpsError(
            "SWEBENCH_INVALID_ECE_BINS",
            f"invalid ECE bins: {bins}",
            "Use at least one ECE bin.",
            {"bins": bins},
        )
    if len(labels) != len(probabilities) or not labels:
        raise SwebenchOpsError(
            "SWEBENCH_METRIC_LABEL_PROB_MISMATCH",
            "labels/probabilities must be same non-zero length",
            "Fix the metric inputs before writing evidence.",
            {"labels": len(labels), "probabilities": len(probabilities)},
        )
    total = len(labels)
    ece = 0.0
    for idx in range(bins):
        low = idx / bins
        high = (idx + 1) / bins
        bucket = [
            (label, prob)
            for label, prob in zip(labels, probabilities)
            if (low <= prob < high) or (idx == bins - 1 and prob == 1.0)
        ]
        if not bucket:
            continue
        conf = sum(float(prob) for _, prob in bucket) / len(bucket)
        observed = sum(float(label) for label, _ in bucket) / len(bucket)
        ece += (len(bucket) / total) * abs(conf - observed)
    return finite_float(ece)


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
