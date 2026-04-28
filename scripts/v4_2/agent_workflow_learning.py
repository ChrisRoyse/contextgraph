#!/usr/bin/env python3
"""Capture, pair, train, benchmark, and promote agent workflow learning events.

This is the bridge from real assistant activity to the existing Phase 6
Learning-as-UTL substrate:

    real event -> physical ledger -> RocksDB LearningEvent -> preference pairs
    -> policy candidate -> benchmark -> promote only if better

The script is intentionally fail-closed. It does not invent evidence, does not
silently skip storage, and records diagnostics whenever an operation fails.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import re
import shutil
import subprocess
import sys
import time
import uuid
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[2]
DEFAULT_HOME = Path("~/.contextgraph/agent_workflow_learning").expanduser()
DEFAULT_LEDGER = DEFAULT_HOME / "events.jsonl"
DEFAULT_LEARNING_JSONL = DEFAULT_HOME / "learning_events.jsonl"
DEFAULT_PAIR_JSONL = DEFAULT_HOME / "workflow_pairs.jsonl"
DEFAULT_POLICY = DEFAULT_HOME / "policy_candidate.json"
DEFAULT_ACTIVE_POLICY = DEFAULT_HOME / "active_policy.json"
DEFAULT_CLI = ROOT / "target/debug/context-graph-cli"
DEFAULT_REPORT_DIR = Path("~/.contextgraph/reports").expanduser()

NUM_EMBEDDERS = 14
NUM_CROSS_CORRELATIONS = 91
MAX_TEXT_CHARS = 65_536
VALID_LABELS = {"useful", "neutral", "harmful", "no_learning"}
SCHEMA_VERSION = 1


class WorkflowLearningError(RuntimeError):
    """Fail-closed workflow learning error."""


@dataclass(frozen=True)
class CommandResult:
    name: str
    command: list[str]
    returncode: int
    elapsed_sec: float
    stdout: str
    stderr: str

    def as_report(self) -> dict[str, Any]:
        return {
            "name": self.name,
            "command": self.command,
            "returncode": self.returncode,
            "elapsed_sec": round(self.elapsed_sec, 3),
            "stdout_tail": self.stdout.splitlines()[-80:],
            "stderr_tail": self.stderr.splitlines()[-80:],
        }


def utc_now() -> str:
    return datetime.now(timezone.utc).isoformat()


def utc_stamp() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%d_%H%M%S")


def sha256_text(value: str) -> str:
    return hashlib.sha256(value.encode("utf-8")).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def canonical_json(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def sha256_json(value: Any) -> str:
    return sha256_text(canonical_json(value))


def read_json(path: Path) -> Any:
    if not path.exists():
        raise WorkflowLearningError(f"JSON input does not exist: {path}")
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except Exception as exc:
        raise WorkflowLearningError(f"failed to parse JSON {path}: {exc}") from exc


def write_json_atomic(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_text(json.dumps(payload, indent=2, sort_keys=True), encoding="utf-8")
    tmp.replace(path)


def append_jsonl(path: Path, row: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a", encoding="utf-8") as handle:
        handle.write(json.dumps(row, ensure_ascii=False, sort_keys=True))
        handle.write("\n")


def count_jsonl(path: Path) -> int:
    if not path.exists():
        return 0
    with path.open("r", encoding="utf-8") as handle:
        return sum(1 for line in handle if line.strip())


def load_jsonl(path: Path) -> list[dict[str, Any]]:
    if not path.exists():
        raise WorkflowLearningError(f"JSONL file does not exist: {path}")
    rows: list[dict[str, Any]] = []
    with path.open("r", encoding="utf-8") as handle:
        for line_no, line in enumerate(handle, start=1):
            if not line.strip():
                continue
            try:
                rows.append(json.loads(line))
            except Exception as exc:
                raise WorkflowLearningError(f"{path}:{line_no}: invalid JSONL row: {exc}") from exc
    if not rows:
        raise WorkflowLearningError(f"JSONL file contains zero rows: {path}")
    return rows


def run_command(
    name: str,
    command: list[str],
    *,
    timeout: int = 120,
    allow_fail: bool = False,
    cwd: Path = ROOT,
) -> CommandResult:
    started = time.perf_counter()
    proc = subprocess.run(
        command,
        cwd=cwd,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=timeout,
    )
    elapsed = time.perf_counter() - started
    result = CommandResult(name, command, proc.returncode, elapsed, proc.stdout, proc.stderr)
    if proc.returncode != 0 and not allow_fail:
        raise WorkflowLearningError(
            f"{name} failed with exit code {proc.returncode}\n"
            f"stdout={proc.stdout[-2000:]}\nstderr={proc.stderr[-2000:]}"
        )
    return result


def parse_json_stdout(stdout: str, label: str) -> dict[str, Any]:
    start = stdout.find("{")
    if start < 0:
        raise WorkflowLearningError(f"{label} did not emit JSON object; stdout={stdout[-1000:]}")
    try:
        value = json.loads(stdout[start:])
    except Exception as exc:
        raise WorkflowLearningError(f"{label} JSON parse failed: {exc}; stdout={stdout[-1000:]}") from exc
    if not isinstance(value, dict):
        raise WorkflowLearningError(f"{label} JSON root is not an object")
    return value


def parse_ldb_count(stdout: str, cf: str) -> int:
    match = re.search(r"Keys in range:\s+(\d+)", stdout)
    if not match:
        raise WorkflowLearningError(f"ldb output for {cf} did not include 'Keys in range'")
    return int(match.group(1))


def ldb_count(storage: Path, cf: str, report: dict[str, Any], label: str) -> int:
    command = ["ldb", f"--db={storage}", f"--column_family={cf}", "dump", "--count_only"]
    if not (storage / "CURRENT").exists():
        report.setdefault("commands", {})[f"ldb_{label}_{cf}"] = {
            "command": command,
            "returncode": 0,
            "elapsed_sec": 0.0,
            "stdout_tail": ["Keys in range: 0"],
            "stderr_tail": ["RocksDB directory not initialized; physical count is zero."],
        }
        return 0
    result = run_command(f"ldb_{label}_{cf}", command, timeout=300)
    report.setdefault("commands", {})[result.name] = result.as_report()
    return parse_ldb_count(result.stdout, cf)


def record_failure(action: str, payload: dict[str, Any], error: BaseException) -> None:
    failure_dir = DEFAULT_HOME / "failures"
    failure_dir.mkdir(parents=True, exist_ok=True)
    report = {
        "status": "error",
        "action": action,
        "error": str(error),
        "error_type": type(error).__name__,
        "created_at": utc_now(),
        "payload": payload,
    }
    path = failure_dir / f"{utc_stamp()}_{action}.json"
    write_json_atomic(path, report)


def require(condition: bool, message: str) -> None:
    if not condition:
        raise WorkflowLearningError(message)


def require_nonempty_str(obj: dict[str, Any], key: str) -> str:
    value = obj.get(key)
    if not isinstance(value, str) or not value.strip():
        raise WorkflowLearningError(f"{key} must be a non-empty string")
    return value.strip()


def clamp01(value: float) -> float:
    if not math.isfinite(value):
        raise WorkflowLearningError(f"non-finite score: {value}")
    return max(0.0, min(1.0, value))


def normalized_float(value: Any, *, field: str, min_value: float, max_value: float) -> float:
    if not isinstance(value, (int, float)) or isinstance(value, bool):
        raise WorkflowLearningError(f"{field} must be numeric")
    out = float(value)
    if not math.isfinite(out) or not (min_value <= out <= max_value):
        raise WorkflowLearningError(f"{field} must be finite and in [{min_value}, {max_value}]")
    return out


def bool_field(obj: dict[str, Any], key: str, default: bool = False) -> bool:
    value = obj.get(key, default)
    if not isinstance(value, bool):
        raise WorkflowLearningError(f"{key} must be boolean")
    return value


def validate_text_limit(value: str, field: str) -> None:
    if len(value) > MAX_TEXT_CHARS:
        raise WorkflowLearningError(f"{field} length {len(value)} exceeds {MAX_TEXT_CHARS}")


def expand_path(raw: str) -> Path:
    return Path(raw).expanduser()


def evaluate_file_exists(check: dict[str, Any]) -> dict[str, Any]:
    path = expand_path(require_nonempty_str(check, "path"))
    expected = check.get("expected_exists", check.get("expected", True))
    if not isinstance(expected, bool):
        raise WorkflowLearningError("file_exists expected_exists must be boolean")
    actual = path.exists()
    result: dict[str, Any] = {
        "kind": "file_exists",
        "path": str(path),
        "expected_exists": expected,
        "actual_exists": actual,
        "passed": actual is expected,
    }
    if actual:
        result["is_file"] = path.is_file()
        result["is_dir"] = path.is_dir()
        result["size_bytes"] = path.stat().st_size
        if path.is_file():
            result["sha256"] = sha256_file(path)
    return result


def evaluate_file_contains(check: dict[str, Any]) -> dict[str, Any]:
    path = expand_path(require_nonempty_str(check, "path"))
    needle = require_nonempty_str(check, "contains")
    actual_exists = path.exists() and path.is_file()
    text = path.read_text(encoding="utf-8", errors="replace") if actual_exists else ""
    result = {
        "kind": "file_contains",
        "path": str(path),
        "contains_sha256": sha256_text(needle),
        "actual_exists": actual_exists,
        "actual_contains": needle in text,
        "passed": actual_exists and needle in text,
    }
    if actual_exists:
        result["size_bytes"] = path.stat().st_size
        result["sha256"] = sha256_file(path)
    return result


def json_pointer_get(value: Any, pointer: str) -> Any:
    if pointer == "":
        return value
    if not pointer.startswith("/"):
        raise WorkflowLearningError(f"json_pointer must start with '/': {pointer}")
    cur = value
    for raw_part in pointer.split("/")[1:]:
        part = raw_part.replace("~1", "/").replace("~0", "~")
        if isinstance(cur, list):
            if not part.isdigit():
                raise WorkflowLearningError(f"json pointer list segment is not numeric: {part}")
            idx = int(part)
            if idx >= len(cur):
                raise WorkflowLearningError(f"json pointer list index out of range: {idx}")
            cur = cur[idx]
        elif isinstance(cur, dict):
            if part not in cur:
                raise WorkflowLearningError(f"json pointer missing object key: {part}")
            cur = cur[part]
        else:
            raise WorkflowLearningError(f"json pointer reached scalar before segment: {part}")
    return cur


def evaluate_json_file_value(check: dict[str, Any]) -> dict[str, Any]:
    path = expand_path(require_nonempty_str(check, "path"))
    pointer = check.get("json_pointer", check.get("pointer"))
    if not isinstance(pointer, str):
        raise WorkflowLearningError("json_file_value requires json_pointer")
    expected = check.get("expected")
    actual_exists = path.exists() and path.is_file()
    actual_value = None
    passed = False
    if actual_exists:
        actual_value = json_pointer_get(json.loads(path.read_text(encoding="utf-8")), pointer)
        passed = actual_value == expected
    return {
        "kind": "json_file_value",
        "path": str(path),
        "json_pointer": pointer,
        "expected": expected,
        "actual": actual_value,
        "actual_exists": actual_exists,
        "passed": passed,
    }


def evaluate_command(check: dict[str, Any]) -> dict[str, Any]:
    command = check.get("command")
    if not isinstance(command, list) or not command or not all(isinstance(v, str) for v in command):
        raise WorkflowLearningError("command evidence requires command as a non-empty string list")
    timeout = int(check.get("timeout_sec", 30))
    expected_returncode = int(check.get("expected_returncode", 0))
    result = run_command(
        "evidence_command",
        command,
        timeout=timeout,
        allow_fail=True,
        cwd=ROOT,
    )
    passed = result.returncode == expected_returncode
    stdout_contains = check.get("stdout_contains")
    stderr_contains = check.get("stderr_contains")
    if stdout_contains is not None:
        if not isinstance(stdout_contains, str):
            raise WorkflowLearningError("stdout_contains must be a string")
        passed = passed and stdout_contains in result.stdout
    if stderr_contains is not None:
        if not isinstance(stderr_contains, str):
            raise WorkflowLearningError("stderr_contains must be a string")
        passed = passed and stderr_contains in result.stderr
    return {
        "kind": "command",
        "command": command,
        "expected_returncode": expected_returncode,
        "actual_returncode": result.returncode,
        "elapsed_sec": round(result.elapsed_sec, 3),
        "stdout_tail": result.stdout.splitlines()[-20:],
        "stderr_tail": result.stderr.splitlines()[-20:],
        "stdout_contains": stdout_contains,
        "stderr_contains": stderr_contains,
        "passed": passed,
    }


def evaluate_rocksdb_count(check: dict[str, Any]) -> dict[str, Any]:
    storage = expand_path(require_nonempty_str(check, "storage"))
    cf = require_nonempty_str(check, "column_family")
    report: dict[str, Any] = {}
    actual = ldb_count(storage, cf, report, "evidence")
    passed = True
    expected_exact = check.get("expected_exact")
    expected_min = check.get("expected_min")
    if expected_exact is not None:
        passed = actual == int(expected_exact)
    if expected_min is not None:
        passed = passed and actual >= int(expected_min)
    return {
        "kind": "rocksdb_count",
        "storage": str(storage),
        "column_family": cf,
        "expected_exact": expected_exact,
        "expected_min": expected_min,
        "actual_count": actual,
        "passed": passed,
        "commands": report.get("commands", {}),
    }


def evaluate_evidence(check: dict[str, Any]) -> dict[str, Any]:
    kind = require_nonempty_str(check, "kind")
    if kind == "file_exists":
        return evaluate_file_exists(check)
    if kind == "file_contains":
        return evaluate_file_contains(check)
    if kind == "json_file_value":
        return evaluate_json_file_value(check)
    if kind == "command":
        return evaluate_command(check)
    if kind == "rocksdb_count":
        return evaluate_rocksdb_count(check)
    raise WorkflowLearningError(f"unsupported physical evidence kind: {kind}")


def normalize_retrieved_contexts(raw: Any) -> list[dict[str, Any]]:
    if raw is None:
        return []
    if not isinstance(raw, list):
        raise WorkflowLearningError("retrieved_contexts must be a list")
    contexts: list[dict[str, Any]] = []
    for idx, item in enumerate(raw, start=1):
        if not isinstance(item, dict):
            raise WorkflowLearningError(f"retrieved_contexts[{idx}] must be an object")
        text = item.get("text", "")
        if not isinstance(text, str):
            raise WorkflowLearningError(f"retrieved_contexts[{idx}].text must be a string")
        validate_text_limit(text, f"retrieved_contexts[{idx}].text")
        rank = item.get("rank", idx)
        if not isinstance(rank, int) or isinstance(rank, bool) or rank <= 0:
            raise WorkflowLearningError(f"retrieved_contexts[{idx}].rank must be a positive integer")
        relevance = item.get("relevance", 0.5)
        contexts.append(
            {
                "context_id": str(item["context_id"]) if item.get("context_id") is not None else None,
                "source": str(item.get("source", "unknown")).strip() or "unknown",
                "rank": rank,
                "text": text,
                "used": bool_field(item, "used", default=False),
                "relevance": normalized_float(
                    relevance,
                    field=f"retrieved_contexts[{idx}].relevance",
                    min_value=0.0,
                    max_value=1.0,
                ),
            }
        )
    return contexts


def normalize_tool_calls(raw: Any) -> list[dict[str, Any]]:
    if raw is None:
        return []
    if not isinstance(raw, list):
        raise WorkflowLearningError("tool_calls must be a list")
    calls: list[dict[str, Any]] = []
    for idx, item in enumerate(raw, start=1):
        if not isinstance(item, dict):
            raise WorkflowLearningError(f"tool_calls[{idx}] must be an object")
        name = str(item.get("name", "")).strip()
        if not name:
            raise WorkflowLearningError(f"tool_calls[{idx}].name must be non-empty")
        calls.append(
            {
                "name": name,
                "success": bool_field(item, "success", default=False),
                "verified": bool_field(item, "verified", default=False),
                "purpose": str(item.get("purpose", "")),
                "args_summary": str(item.get("args_summary", "")),
                "output_summary": str(item.get("output_summary", "")),
            }
        )
    return calls


def normalize_outcome(raw: Any) -> dict[str, Any]:
    if not isinstance(raw, dict):
        raise WorkflowLearningError("outcome_signal must be an object")
    label = str(raw.get("label", "")).strip()
    if label not in VALID_LABELS:
        raise WorkflowLearningError(
            f"outcome_signal.label must be one of {sorted(VALID_LABELS)}"
        )
    utility_delta = normalized_float(
        raw.get("utility_delta"),
        field="outcome_signal.utility_delta",
        min_value=-1.0,
        max_value=1.0,
    )
    return {
        "label": label,
        "utility_delta": utility_delta,
        "accepted": bool_field(raw, "accepted", default=False),
        "correction_required": bool_field(raw, "correction_required", default=False),
        "reuse_observed": bool_field(raw, "reuse_observed", default=False),
        "failure_reason": str(raw.get("failure_reason", "")),
    }


def normalize_workflow_event(raw: dict[str, Any]) -> dict[str, Any]:
    if not isinstance(raw, dict):
        raise WorkflowLearningError("event input must be a JSON object")
    if raw.get("is_mock") is True or raw.get("data_source") == "mock":
        raise WorkflowLearningError("mock workflow events are not accepted")

    event_id = str(raw.get("event_id") or uuid.uuid4())
    try:
        uuid.UUID(event_id)
    except ValueError as exc:
        raise WorkflowLearningError(f"event_id is not a UUID: {event_id}") from exc

    user_query = require_nonempty_str(raw, "user_query")
    final_response = require_nonempty_str(raw, "final_response")
    validate_text_limit(user_query, "user_query")
    validate_text_limit(final_response, "final_response")

    evidence_input = raw.get("physical_evidence")
    if not isinstance(evidence_input, list) or not evidence_input:
        raise WorkflowLearningError("physical_evidence must be a non-empty list")
    evaluated_evidence = []
    for idx, check in enumerate(evidence_input, start=1):
        if not isinstance(check, dict):
            raise WorkflowLearningError(f"physical_evidence[{idx}] must be an object")
        evaluated_evidence.append(evaluate_evidence(check))
    evidence_passed = all(item.get("passed") is True for item in evaluated_evidence)

    outcome = normalize_outcome(raw.get("outcome_signal"))
    outcome["physical_outcome_verified"] = evidence_passed
    training_eligible = bool_field(raw, "training_eligible", default=True)
    if training_eligible and not evidence_passed:
        failed = [item for item in evaluated_evidence if item.get("passed") is not True]
        raise WorkflowLearningError(
            "training-eligible event has failing physical evidence: "
            f"{json.dumps(failed, sort_keys=True)[:1500]}"
        )

    row = {
        "schema_version": SCHEMA_VERSION,
        "record_kind": "agent_workflow_event",
        "event_id": event_id,
        "created_at": str(raw.get("created_at") or utc_now()),
        "session_id": require_nonempty_str(raw, "session_id"),
        "task_id": require_nonempty_str(raw, "task_id"),
        "domain": str(raw.get("domain", "agent_workflow")).strip() or "agent_workflow",
        "user_query": user_query,
        "intended_outcome": str(raw.get("intended_outcome", "")).strip(),
        "retrieved_contexts": normalize_retrieved_contexts(raw.get("retrieved_contexts", [])),
        "tool_calls": normalize_tool_calls(raw.get("tool_calls", [])),
        "final_response": final_response,
        "outcome_signal": outcome,
        "physical_evidence": evaluated_evidence,
        "training_eligible": training_eligible,
        "operator_notes": str(raw.get("operator_notes", "")),
    }
    row["provenance_sha256"] = sha256_json(row)
    return row


def context_text(row: dict[str, Any]) -> str:
    parts: list[str] = []
    for ctx in row["retrieved_contexts"]:
        header = (
            f"[rank={ctx['rank']} source={ctx['source']} used={ctx['used']} "
            f"relevance={ctx['relevance']:.3f}"
        )
        if ctx.get("context_id"):
            header += f" id={ctx['context_id']}"
        header += "]"
        parts.append(f"{header}\n{ctx['text']}")
    text = "\n\n".join(parts)
    validate_text_limit(text, "retrieved_context")
    return text


def render_workflow_response(row: dict[str, Any]) -> str:
    tool_lines = []
    for call in row["tool_calls"]:
        tool_lines.append(
            "- {name} success={success} verified={verified} purpose={purpose}".format(**call)
        )
    evidence_lines = []
    for check in row["physical_evidence"]:
        evidence_lines.append(
            f"- {check['kind']} passed={check['passed']} proof={sha256_json(check)[:16]}"
        )
    rendered = (
        "Final response:\n"
        f"{row['final_response']}\n\n"
        "Tool sequence:\n"
        f"{chr(10).join(tool_lines) if tool_lines else '- none'}\n\n"
        "Physical outcome evidence:\n"
        f"{chr(10).join(evidence_lines)}"
    )
    validate_text_limit(rendered, "assistant_response")
    return rendered


def cross_correlations(profile: list[float]) -> list[float]:
    out = []
    for i in range(NUM_EMBEDDERS):
        for j in range(i + 1, NUM_EMBEDDERS):
            out.append(clamp01(profile[i] * profile[j]))
    if len(out) != NUM_CROSS_CORRELATIONS:
        raise WorkflowLearningError(
            f"cross correlation shape mismatch: {len(out)} != {NUM_CROSS_CORRELATIONS}"
        )
    return out


def state_snapshot(
    profile: list[float],
    *,
    retrieval_rank: int | None,
    domain: str,
    contradiction_pressure: float,
    integration_confidence: float,
    recurrence_count: int,
    stability_score: float,
    successful_transfer_count: int,
) -> dict[str, Any]:
    if len(profile) != NUM_EMBEDDERS:
        raise WorkflowLearningError(f"profile has {len(profile)} dims, expected {NUM_EMBEDDERS}")
    profile = [clamp01(float(v)) for v in profile]
    return {
        "topicProfile": profile,
        "crossCorrelations": cross_correlations(profile),
        "retrievalRank": retrieval_rank,
        "embedderScores": profile,
        "contradictionPressure": clamp01(contradiction_pressure),
        "integrationConfidence": clamp01(integration_confidence),
        "recurrenceCount": int(max(0, recurrence_count)),
        "stabilityScore": clamp01(stability_score),
        "domain": domain,
        "successfulTransferCount": int(max(0, successful_transfer_count)),
    }


def mean(values: list[float], default: float = 0.0) -> float:
    return sum(values) / len(values) if values else default


def profile_after(row: dict[str, Any]) -> list[float]:
    contexts = row["retrieved_contexts"]
    tools = row["tool_calls"]
    evidence = row["physical_evidence"]
    outcome = row["outcome_signal"]
    ranks = [ctx["rank"] for ctx in contexts]
    best_rank_quality = 0.0 if not ranks else clamp01(1.0 / min(ranks))
    utility_pos = max(0.0, outcome["utility_delta"])
    utility_norm = (outcome["utility_delta"] + 1.0) / 2.0
    no_failure = 1.0 if outcome["label"] in {"useful", "neutral"} else 0.0
    return [
        mean([ctx["relevance"] for ctx in contexts], 0.0),
        mean([1.0 if ctx["used"] else 0.0 for ctx in contexts], 0.0),
        mean([1.0 if call["success"] else 0.0 for call in tools], 1.0 if not tools else 0.0),
        mean([1.0 if call["verified"] else 0.0 for call in tools], 0.0),
        1.0 if outcome["physical_outcome_verified"] else 0.0,
        1.0 if outcome["accepted"] else 0.0,
        0.0 if outcome["correction_required"] else 1.0,
        utility_pos,
        1.0 if outcome["reuse_observed"] else 0.0,
        min(1.0, len(evidence) / 3.0),
        best_rank_quality,
        1.0 if outcome["physical_outcome_verified"] and not outcome["correction_required"] else 0.0,
        no_failure,
        1.0 if row["training_eligible"] else 0.0,
    ]


def build_learning_event_row(row: dict[str, Any]) -> dict[str, Any]:
    after_profile = profile_after(row)
    before_profile = [0.5] * NUM_EMBEDDERS
    ranks = [ctx["rank"] for ctx in row["retrieved_contexts"]]
    before_rank = max(ranks) + 5 if ranks else None
    used_ranks = [ctx["rank"] for ctx in row["retrieved_contexts"] if ctx["used"]]
    after_rank = min(used_ranks or ranks) if ranks else None
    outcome = row["outcome_signal"]
    integration = (outcome["utility_delta"] + 1.0) / 2.0
    if outcome["physical_outcome_verified"]:
        integration = max(integration, 0.65)
    contradiction = 0.75 if outcome["correction_required"] or outcome["label"] == "harmful" else 0.1
    stability = min(1.0, len(row["physical_evidence"]) / 3.0)
    recurrence = min(10, len(row["tool_calls"]) + len(row["retrieved_contexts"]))

    return {
        "eventId": row["event_id"],
        "memoryIds": [],
        "sessionId": row["session_id"],
        "responseId": "sha256:" + sha256_text(row["final_response"])[:32],
        "taskId": row["task_id"],
        "query": row["user_query"],
        "retrievedContext": context_text(row),
        "assistantResponse": render_workflow_response(row),
        "before": state_snapshot(
            before_profile,
            retrieval_rank=before_rank,
            domain=row["domain"],
            contradiction_pressure=0.2,
            integration_confidence=0.5,
            recurrence_count=0,
            stability_score=0.5,
            successful_transfer_count=0,
        ),
        "after": state_snapshot(
            after_profile,
            retrieval_rank=after_rank,
            domain=row["domain"],
            contradiction_pressure=contradiction,
            integration_confidence=integration,
            recurrence_count=recurrence,
            stability_score=stability,
            successful_transfer_count=1 if outcome["reuse_observed"] else 0,
        ),
        "outcome": {
            "label": outcome["label"],
            "utilityDelta": outcome["utility_delta"],
            "correctionRequired": outcome["correction_required"],
            "reuseObserved": outcome["reuse_observed"],
        },
    }


def cli_count_learning_events(cli: Path, storage: Path, report: dict[str, Any], label: str) -> int:
    result = run_command(
        f"cli_count_{label}",
        [str(cli), "learning", "count", "--storage", str(storage)],
        timeout=120,
    )
    report.setdefault("commands", {})[result.name] = result.as_report()
    payload = parse_json_stdout(result.stdout, result.name)
    return int(payload["count"])


def cli_get_learning_event(
    cli: Path,
    storage: Path,
    event_id: str,
    report: dict[str, Any],
    label: str,
) -> dict[str, Any]:
    result = run_command(
        f"cli_get_{label}",
        [
            str(cli),
            "learning",
            "get",
            "--storage",
            str(storage),
            "--event-id",
            event_id,
            "--include-text",
            "true",
        ],
        timeout=120,
    )
    report.setdefault("commands", {})[result.name] = result.as_report()
    return parse_json_stdout(result.stdout, result.name)


def import_learning_event(
    *,
    cli: Path,
    storage: Path,
    learning_row: dict[str, Any],
    report: dict[str, Any],
) -> dict[str, Any]:
    if not cli.exists():
        raise WorkflowLearningError(f"context-graph-cli not found: {cli}")
    event_id = learning_row["eventId"]
    before_count = cli_count_learning_events(cli, storage, report, "before_import")
    tmp = DEFAULT_HOME / "tmp" / f"import_{event_id}.jsonl"
    tmp.parent.mkdir(parents=True, exist_ok=True)
    tmp.write_text(json.dumps(learning_row, sort_keys=True) + "\n", encoding="utf-8")
    try:
        result = run_command(
            "cli_record_events_jsonl",
            [
                str(cli),
                "learning",
                "record-events-jsonl",
                "--storage",
                str(storage),
                "--input",
                str(tmp),
            ],
            timeout=180,
        )
        report.setdefault("commands", {})[result.name] = result.as_report()
        record_payload = parse_json_stdout(result.stdout, result.name)
        after_count = cli_count_learning_events(cli, storage, report, "after_import")
        if after_count != before_count + 1:
            raise WorkflowLearningError(
                f"RocksDB learning_events count did not increment by 1: "
                f"before={before_count} after={after_count}"
            )
        readback = cli_get_learning_event(cli, storage, event_id, report, "after_import")
        if readback.get("found") is False:
            raise WorkflowLearningError(f"imported event {event_id} missing from RocksDB readback")
        if readback.get("event_id") != event_id:
            raise WorkflowLearningError(
                f"readback event_id mismatch: expected {event_id}, got {readback.get('event_id')}"
            )
        return {
            "before_count": before_count,
            "after_count": after_count,
            "record_payload": record_payload,
            "readback": readback,
        }
    finally:
        if tmp.exists():
            tmp.unlink()


def existing_event_ids(ledger: Path) -> set[str]:
    if not ledger.exists():
        return set()
    return {row["event_id"] for row in load_jsonl(ledger) if row.get("event_id")}


def record_event(
    *,
    event_path: Path,
    ledger: Path,
    learning_jsonl: Path,
    storage: Path | None,
    cli: Path,
) -> dict[str, Any]:
    raw = read_json(event_path)
    row = normalize_workflow_event(raw)
    if row["event_id"] in existing_event_ids(ledger):
        raise WorkflowLearningError(f"event_id already exists in ledger: {row['event_id']}")
    learning_row = build_learning_event_row(row)
    report: dict[str, Any] = {
        "status": "recorded",
        "source_of_truth": {
            "workflow_ledger": str(ledger),
            "learning_event_jsonl": str(learning_jsonl),
            "rocksdb": str(storage) if storage else None,
            "rocksdb_column_family": "learning_events" if storage else None,
        },
        "event_id": row["event_id"],
        "before": {
            "ledger_rows": count_jsonl(ledger),
            "learning_jsonl_rows": count_jsonl(learning_jsonl),
        },
    }
    append_jsonl(ledger, row)
    append_jsonl(learning_jsonl, learning_row)
    report["after"] = {
        "ledger_rows": count_jsonl(ledger),
        "learning_jsonl_rows": count_jsonl(learning_jsonl),
    }
    if storage is not None:
        report["rocksdb_import"] = import_learning_event(
            cli=cli,
            storage=storage,
            learning_row=learning_row,
            report=report,
        )
    return report


def group_key(row: dict[str, Any]) -> str:
    intended = row.get("intended_outcome") or ""
    return "\n".join([row["task_id"], row["user_query"].strip(), intended.strip()])


def effective_utility(row: dict[str, Any]) -> float:
    outcome = row["outcome_signal"]
    utility = float(outcome["utility_delta"])
    if outcome["correction_required"]:
        utility -= 0.25
    if not outcome["physical_outcome_verified"]:
        utility -= 0.35
    if not outcome["accepted"]:
        utility -= 0.10
    return max(-1.0, min(1.0, utility))


def mine_pairs(
    rows: list[dict[str, Any]],
    *,
    min_gap: float,
    min_chosen_utility: float,
    max_rejected_utility: float,
    split: str,
    holdout_ratio: float,
    split_salt: str,
) -> tuple[list[tuple[dict[str, Any], dict[str, Any], float]], dict[str, int]]:
    groups: dict[str, list[dict[str, Any]]] = {}
    for row in rows:
        if row.get("record_kind") != "agent_workflow_event":
            raise WorkflowLearningError(f"unexpected ledger record_kind: {row.get('record_kind')}")
        if not row.get("training_eligible", False):
            continue
        if split != "all" and split_for_group(group_key(row), holdout_ratio, split_salt) != split:
            continue
        groups.setdefault(group_key(row), []).append(row)

    stats = {
        "events_seen": len(rows),
        "eligible_groups": len(groups),
        "groups_too_small": 0,
        "groups_no_gap": 0,
        "groups_threshold_rejected": 0,
        "pairs_emitted": 0,
    }
    pairs: list[tuple[dict[str, Any], dict[str, Any], float]] = []
    for rows_for_group in groups.values():
        if len(rows_for_group) < 2:
            stats["groups_too_small"] += 1
            continue
        ordered = sorted(rows_for_group, key=effective_utility)
        rejected = ordered[0]
        chosen = ordered[-1]
        gap = effective_utility(chosen) - effective_utility(rejected)
        if gap < min_gap:
            stats["groups_no_gap"] += 1
            continue
        if effective_utility(chosen) < min_chosen_utility or effective_utility(rejected) > max_rejected_utility:
            stats["groups_threshold_rejected"] += 1
            continue
        pairs.append((chosen, rejected, gap))
        stats["pairs_emitted"] += 1
    return pairs, stats


def pair_prompt(row: dict[str, Any]) -> str:
    return (
        "User query:\n"
        f"{row['user_query']}\n\n"
        "Select the workflow that should be preferred for this repeated task."
    )


def write_pairs(
    *,
    ledger: Path,
    output: Path,
    min_gap: float,
    min_chosen_utility: float,
    max_rejected_utility: float,
    overwrite: bool,
    split: str,
    holdout_ratio: float,
    split_salt: str,
) -> dict[str, Any]:
    rows = load_jsonl(ledger)
    pairs, stats = mine_pairs(
        rows,
        min_gap=min_gap,
        min_chosen_utility=min_chosen_utility,
        max_rejected_utility=max_rejected_utility,
        split=split,
        holdout_ratio=holdout_ratio,
        split_salt=split_salt,
    )
    if not pairs:
        raise WorkflowLearningError(f"no workflow preference pairs emitted; stats={stats}")
    if output.exists() and not overwrite:
        raise WorkflowLearningError(f"output exists: {output}; pass --overwrite")
    output.parent.mkdir(parents=True, exist_ok=True)
    tmp = output.with_suffix(output.suffix + ".tmp")
    with tmp.open("w", encoding="utf-8") as handle:
        for chosen, rejected, gap in pairs:
            pair = {
                "prompt": pair_prompt(chosen),
                "chosen": render_workflow_response(chosen),
                "rejected": render_workflow_response(rejected),
                "meta": {
                    "source": "agent_workflow_outcome",
                    "chosen_event_id": chosen["event_id"],
                    "rejected_event_id": rejected["event_id"],
                    "task_id": chosen["task_id"],
                    "group_key_sha256": sha256_text(group_key(chosen)),
                    "chosen_effective_utility": effective_utility(chosen),
                    "rejected_effective_utility": effective_utility(rejected),
                    "utility_gap": gap,
                    "chosen_provenance_sha256": chosen["provenance_sha256"],
                    "rejected_provenance_sha256": rejected["provenance_sha256"],
                },
            }
            if not pair["chosen"].strip() or not pair["rejected"].strip():
                raise WorkflowLearningError("pair has empty chosen/rejected text")
            if pair["chosen"] == pair["rejected"]:
                raise WorkflowLearningError("pair chosen and rejected texts are identical")
            handle.write(json.dumps(pair, ensure_ascii=False, sort_keys=True))
            handle.write("\n")
    tmp.replace(output)
    return {
        "status": "ok",
        "source_of_truth": {"workflow_ledger": str(ledger), "pair_jsonl": str(output)},
        "pairs": len(pairs),
        "stats": stats,
        "bytes_written": output.stat().st_size,
        "sha256": sha256_file(output),
    }


def event_features(row: dict[str, Any]) -> list[str]:
    features: list[str] = []
    for ctx in row["retrieved_contexts"]:
        features.append(f"source:{ctx['source']}")
        if ctx["used"]:
            features.append(f"used_source:{ctx['source']}")
        if ctx["rank"] <= 3:
            features.append("rank:top3")
    for call in row["tool_calls"]:
        features.append(f"tool:{call['name']}")
        if call["verified"]:
            features.append(f"verified_tool:{call['name']}")
    for check in row["physical_evidence"]:
        features.append(f"evidence:{check['kind']}")
    if row["outcome_signal"]["physical_outcome_verified"]:
        features.append("physical_outcome_verified")
    if row["retrieved_contexts"]:
        features.append("has_retrieval")
    if row["tool_calls"]:
        features.append("has_tool_calls")
    return sorted(set(features))


def split_for_group(key: str, holdout_ratio: float, split_salt: str) -> str:
    if holdout_ratio <= 0:
        return "train"
    if holdout_ratio >= 1:
        return "holdout"
    digest = sha256_text(split_salt + "\n" + key)
    value = int(digest[:12], 16) / float(0xFFFFFFFFFFFF)
    return "holdout" if value < holdout_ratio else "train"


def train_policy(
    *,
    ledger: Path,
    output: Path,
    min_events: int,
    holdout_ratio: float,
    split_salt: str,
    overwrite: bool,
) -> dict[str, Any]:
    rows = [
        row for row in load_jsonl(ledger)
        if row.get("training_eligible") and split_for_group(group_key(row), holdout_ratio, split_salt) == "train"
    ]
    if len(rows) < min_events:
        raise WorkflowLearningError(f"not enough train events: {len(rows)} < {min_events}")
    sums: dict[str, float] = {}
    counts: dict[str, int] = {}
    for row in rows:
        utility = effective_utility(row)
        for feature in event_features(row):
            sums[feature] = sums.get(feature, 0.0) + utility
            counts[feature] = counts.get(feature, 0) + 1
    if not counts:
        raise WorkflowLearningError("no policy features were observed in training events")
    weights = {feature: sums[feature] / counts[feature] for feature in sorted(counts)}
    policy = {
        "schema_version": SCHEMA_VERSION,
        "record_kind": "agent_workflow_policy",
        "created_at": utc_now(),
        "training_ledger": str(ledger),
        "training_events": len(rows),
        "holdout_ratio": holdout_ratio,
        "split_salt": split_salt,
        "feature_weights": weights,
        "feature_counts": counts,
    }
    policy["policy_id"] = sha256_json(policy)
    if output.exists() and not overwrite:
        raise WorkflowLearningError(f"policy output exists: {output}; pass --overwrite")
    write_json_atomic(output, policy)
    readback = read_json(output)
    if readback.get("policy_id") != policy["policy_id"]:
        raise WorkflowLearningError("policy readback id mismatch after write")
    return {
        "status": "ok",
        "source_of_truth": {"workflow_ledger": str(ledger), "policy_file": str(output)},
        "policy_id": policy["policy_id"],
        "training_events": len(rows),
        "feature_count": len(weights),
        "policy_sha256": sha256_file(output),
    }


def score_event(policy: dict[str, Any], row: dict[str, Any]) -> float:
    weights = policy.get("feature_weights", {})
    if not isinstance(weights, dict):
        raise WorkflowLearningError("policy feature_weights must be an object")
    features = event_features(row)
    if not features:
        return 0.0
    return sum(float(weights.get(feature, 0.0)) for feature in features) / math.sqrt(len(features))


def load_policy_or_default(path: Path | None) -> dict[str, Any]:
    if path is None:
        return {
            "record_kind": "agent_workflow_policy",
            "policy_id": "baseline_zero_policy",
            "feature_weights": {},
        }
    value = read_json(path)
    if value.get("record_kind") != "agent_workflow_policy":
        raise WorkflowLearningError(f"not an agent_workflow_policy: {path}")
    return value


def benchmark_policy(
    *,
    ledger: Path,
    candidate: Path,
    baseline: Path | None,
    output: Path,
    min_gap: float,
    holdout_ratio: float,
    split_salt: str,
    split: str,
    overwrite: bool,
) -> dict[str, Any]:
    rows = load_jsonl(ledger)
    pairs, stats = mine_pairs(
        rows,
        min_gap=min_gap,
        min_chosen_utility=-1.0,
        max_rejected_utility=1.0,
        split=split,
        holdout_ratio=holdout_ratio,
        split_salt=split_salt,
    )
    if not pairs:
        raise WorkflowLearningError(f"no benchmark pairs available for split={split}; stats={stats}")
    candidate_policy = load_policy_or_default(candidate)
    baseline_policy = load_policy_or_default(baseline)
    cases = []
    candidate_points = 0.0
    baseline_points = 0.0
    for chosen, rejected, gap in pairs:
        cand_chosen = score_event(candidate_policy, chosen)
        cand_rejected = score_event(candidate_policy, rejected)
        base_chosen = score_event(baseline_policy, chosen)
        base_rejected = score_event(baseline_policy, rejected)
        cand_point = 1.0 if cand_chosen > cand_rejected else 0.5 if cand_chosen == cand_rejected else 0.0
        base_point = 1.0 if base_chosen > base_rejected else 0.5 if base_chosen == base_rejected else 0.0
        candidate_points += cand_point
        baseline_points += base_point
        cases.append(
            {
                "chosen_event_id": chosen["event_id"],
                "rejected_event_id": rejected["event_id"],
                "utility_gap": gap,
                "candidate_scores": {"chosen": cand_chosen, "rejected": cand_rejected, "point": cand_point},
                "baseline_scores": {"chosen": base_chosen, "rejected": base_rejected, "point": base_point},
            }
        )
    candidate_accuracy = candidate_points / len(pairs)
    baseline_accuracy = baseline_points / len(pairs)
    report = {
        "status": "ok",
        "source_of_truth": {
            "workflow_ledger": str(ledger),
            "candidate_policy": str(candidate),
            "baseline_policy": str(baseline) if baseline else "baseline_zero_policy",
            "benchmark_report": str(output),
        },
        "candidate_policy_id": candidate_policy.get("policy_id"),
        "baseline_policy_id": baseline_policy.get("policy_id"),
        "split": split,
        "pair_count": len(pairs),
        "stats": stats,
        "candidate_accuracy": candidate_accuracy,
        "baseline_accuracy": baseline_accuracy,
        "improvement": candidate_accuracy - baseline_accuracy,
        "cases": cases,
    }
    if output.exists() and not overwrite:
        raise WorkflowLearningError(f"benchmark output exists: {output}; pass --overwrite")
    write_json_atomic(output, report)
    readback = read_json(output)
    if readback.get("pair_count") != len(pairs):
        raise WorkflowLearningError("benchmark report readback pair_count mismatch")
    return report


def promote_policy(
    *,
    candidate: Path,
    benchmark: Path,
    active: Path,
    report_path: Path,
    min_accuracy: float,
    min_improvement: float,
    overwrite: bool,
) -> dict[str, Any]:
    candidate_policy = load_policy_or_default(candidate)
    bench = read_json(benchmark)
    candidate_accuracy = float(bench["candidate_accuracy"])
    improvement = float(bench["improvement"])
    if candidate_accuracy < min_accuracy:
        raise WorkflowLearningError(
            f"promotion rejected: candidate_accuracy {candidate_accuracy:.4f} < {min_accuracy:.4f}"
        )
    if improvement <= min_improvement:
        raise WorkflowLearningError(
            f"promotion rejected: improvement {improvement:.4f} <= {min_improvement:.4f}"
        )
    if active.exists() and not overwrite:
        raise WorkflowLearningError(f"active policy exists: {active}; pass --overwrite")
    active.parent.mkdir(parents=True, exist_ok=True)
    tmp = active.with_suffix(active.suffix + ".tmp")
    shutil.copyfile(candidate, tmp)
    tmp.replace(active)
    readback = load_policy_or_default(active)
    if readback.get("policy_id") != candidate_policy.get("policy_id"):
        raise WorkflowLearningError("active policy readback id mismatch after promotion")
    report = {
        "status": "promoted",
        "source_of_truth": {
            "candidate_policy": str(candidate),
            "benchmark_report": str(benchmark),
            "active_policy": str(active),
            "promotion_report": str(report_path),
        },
        "policy_id": candidate_policy.get("policy_id"),
        "candidate_accuracy": candidate_accuracy,
        "baseline_accuracy": bench["baseline_accuracy"],
        "improvement": improvement,
        "active_policy_sha256": sha256_file(active),
    }
    write_json_atomic(report_path, report)
    return report


def inspect_state(ledger: Path, learning_jsonl: Path, storage: Path | None, cli: Path) -> dict[str, Any]:
    report: dict[str, Any] = {
        "source_of_truth": {
            "workflow_ledger": str(ledger),
            "learning_event_jsonl": str(learning_jsonl),
            "rocksdb": str(storage) if storage else None,
            "rocksdb_column_family": "learning_events" if storage else None,
        },
        "ledger_rows": count_jsonl(ledger),
        "learning_jsonl_rows": count_jsonl(learning_jsonl),
    }
    if storage is not None and cli.exists():
        report["rocksdb_count_cli"] = cli_count_learning_events(cli, storage, report, "inspect")
        report["rocksdb_count_ldb"] = ldb_count(storage, "learning_events", report, "inspect")
    return report


def make_fsv_event(
    *,
    event_id: str,
    task_id: str,
    source: str,
    accepted: bool,
    utility: float,
    label: str,
    evidence_file: Path,
    expected_contains: str,
) -> dict[str, Any]:
    good = accepted and utility > 0
    return {
        "event_id": event_id,
        "session_id": "agent-workflow-fsv-session",
        "task_id": task_id,
        "domain": "agent_workflow_fsv",
        "user_query": "Find the repository verification marker and report whether it exists.",
        "intended_outcome": "marker existence is verified from physical storage",
        "retrieved_contexts": [
            {
                "source": source,
                "rank": 1 if good else 7,
                "text": f"{source} says marker phase6-ready should be checked.",
                "used": good,
                "relevance": 0.95 if good else 0.15,
            }
        ],
        "tool_calls": [
            {
                "name": "Read" if good else "Guess",
                "success": good,
                "verified": good,
                "purpose": "inspect evidence file" if good else "answer without inspection",
                "args_summary": str(evidence_file) if good else "none",
                "output_summary": "marker found" if good else "not inspected",
            }
        ],
        "final_response": (
            "The marker exists because the evidence file contains phase6-ready."
            if good
            else "The marker probably exists, but no file was inspected."
        ),
        "outcome_signal": {
            "label": label,
            "utility_delta": utility,
            "accepted": accepted,
            "correction_required": not accepted,
            "reuse_observed": accepted,
        },
        "physical_evidence": [
            {
                "kind": "file_contains",
                "path": str(evidence_file),
                "contains": expected_contains,
            }
        ],
        "training_eligible": True,
    }


def choose_fsv_task_ids(holdout_ratio: float, split_salt: str) -> tuple[str, str]:
    train_task = ""
    holdout_task = ""
    for idx in range(1, 500):
        task_id = f"agent-workflow-fsv-train-{idx}"
        key = "\n".join(
            [
                task_id,
                "Find the repository verification marker and report whether it exists.",
                "marker existence is verified from physical storage",
            ]
        )
        if split_for_group(key, holdout_ratio, split_salt) == "train":
            train_task = task_id
            break
    for idx in range(1, 500):
        task_id = f"agent-workflow-fsv-holdout-{idx}"
        key = "\n".join(
            [
                task_id,
                "Find the repository verification marker and report whether it exists.",
                "marker existence is verified from physical storage",
            ]
        )
        if split_for_group(key, holdout_ratio, split_salt) == "holdout":
            holdout_task = task_id
            break
    if not train_task or not holdout_task:
        raise WorkflowLearningError("could not find deterministic FSV train/holdout task ids")
    return train_task, holdout_task


def run_fsv(args: argparse.Namespace) -> dict[str, Any]:
    run_dir = args.run_dir or (DEFAULT_HOME / "fsv" / utc_stamp())
    if run_dir.exists():
        shutil.rmtree(run_dir)
    run_dir.mkdir(parents=True, exist_ok=False)
    ledger = run_dir / "events.jsonl"
    learning_jsonl = run_dir / "learning_events.jsonl"
    storage = run_dir / "rocksdb"
    pair_jsonl = run_dir / "workflow_pairs.jsonl"
    candidate_policy = run_dir / "policy_candidate.json"
    active_policy = run_dir / "active_policy.json"
    benchmark_report = run_dir / "benchmark.json"
    promotion_report = run_dir / "promotion.json"
    export_jsonl = run_dir / "exported_learning_events.jsonl"
    evidence_file = run_dir / "physical_evidence.txt"
    evidence_file.write_text("phase6-ready\n", encoding="utf-8")

    holdout_ratio = 0.5
    split_salt = "agent-workflow-learning-fsv"
    train_task, holdout_task = choose_fsv_task_ids(holdout_ratio, split_salt)

    report: dict[str, Any] = {
        "status": "running",
        "run_dir": str(run_dir),
        "source_of_truth": {
            "workflow_ledger": str(ledger),
            "learning_event_jsonl": str(learning_jsonl),
            "rocksdb_learning_events": str(storage),
            "pair_jsonl": str(pair_jsonl),
            "candidate_policy": str(candidate_policy),
            "active_policy": str(active_policy),
        },
        "manual_state_checks": [],
        "edge_cases": {},
        "commands": {},
    }

    def write_event_input(name: str, payload: dict[str, Any]) -> Path:
        path = run_dir / f"{name}.json"
        write_json_atomic(path, payload)
        return path

    def record_checked(name: str, payload: dict[str, Any]) -> dict[str, Any]:
        before = inspect_state(ledger, learning_jsonl, storage, args.cli)
        print(f"STATE BEFORE {name}: {json.dumps(before, sort_keys=True)}")
        result = record_event(
            event_path=write_event_input(name, payload),
            ledger=ledger,
            learning_jsonl=learning_jsonl,
            storage=storage,
            cli=args.cli,
        )
        after = inspect_state(ledger, learning_jsonl, storage, args.cli)
        print(f"STATE AFTER {name}: {json.dumps(after, sort_keys=True)}")
        report["manual_state_checks"].append({"name": name, "before": before, "after": after})
        return result

    happy_events = [
        make_fsv_event(
            event_id=str(uuid.uuid4()),
            task_id=train_task,
            source="repo_docs",
            accepted=True,
            utility=0.8,
            label="useful",
            evidence_file=evidence_file,
            expected_contains="phase6-ready",
        ),
        make_fsv_event(
            event_id=str(uuid.uuid4()),
            task_id=train_task,
            source="unverified_guess",
            accepted=False,
            utility=-0.6,
            label="harmful",
            evidence_file=evidence_file,
            expected_contains="phase6-ready",
        ),
        make_fsv_event(
            event_id=str(uuid.uuid4()),
            task_id=holdout_task,
            source="repo_docs",
            accepted=True,
            utility=0.7,
            label="useful",
            evidence_file=evidence_file,
            expected_contains="phase6-ready",
        ),
        make_fsv_event(
            event_id=str(uuid.uuid4()),
            task_id=holdout_task,
            source="unverified_guess",
            accepted=False,
            utility=-0.5,
            label="harmful",
            evidence_file=evidence_file,
            expected_contains="phase6-ready",
        ),
    ]

    for idx, event in enumerate(happy_events, start=1):
        record_checked(f"happy_{idx}", event)

    # Edge 1: empty input fails and does not mutate physical state.
    for edge_name, edge_payload in [
        ("empty_input", {}),
        (
            "invalid_utility",
            {
                **happy_events[0],
                "event_id": str(uuid.uuid4()),
                "outcome_signal": {**happy_events[0]["outcome_signal"], "utility_delta": 1.5},
            },
        ),
        (
            "failing_evidence",
            {
                **happy_events[0],
                "event_id": str(uuid.uuid4()),
                "physical_evidence": [
                    {
                        "kind": "file_contains",
                        "path": str(evidence_file),
                        "contains": "definitely-not-present",
                    }
                ],
            },
        ),
    ]:
        before = inspect_state(ledger, learning_jsonl, storage, args.cli)
        print(f"EDGE BEFORE {edge_name}: {json.dumps(before, sort_keys=True)}")
        try:
            record_event(
                event_path=write_event_input(edge_name, edge_payload),
                ledger=ledger,
                learning_jsonl=learning_jsonl,
                storage=storage,
                cli=args.cli,
            )
        except Exception as exc:
            after = inspect_state(ledger, learning_jsonl, storage, args.cli)
            print(f"EDGE AFTER {edge_name}: {json.dumps(after, sort_keys=True)}")
            if before["ledger_rows"] != after["ledger_rows"]:
                raise WorkflowLearningError(f"{edge_name} mutated ledger rows")
            if before["learning_jsonl_rows"] != after["learning_jsonl_rows"]:
                raise WorkflowLearningError(f"{edge_name} mutated learning JSONL rows")
            if before.get("rocksdb_count_ldb") != after.get("rocksdb_count_ldb"):
                raise WorkflowLearningError(f"{edge_name} mutated RocksDB learning_events")
            report["edge_cases"][edge_name] = {
                "error": str(exc),
                "before": before,
                "after": after,
                "state_unchanged": True,
            }
        else:
            raise WorkflowLearningError(f"{edge_name} unexpectedly succeeded")

    pairs_report = write_pairs(
        ledger=ledger,
        output=pair_jsonl,
        min_gap=0.2,
        min_chosen_utility=0.05,
        max_rejected_utility=0.0,
        overwrite=True,
        split="all",
        holdout_ratio=holdout_ratio,
        split_salt=split_salt,
    )
    policy_report = train_policy(
        ledger=ledger,
        output=candidate_policy,
        min_events=2,
        holdout_ratio=holdout_ratio,
        split_salt=split_salt,
        overwrite=True,
    )
    bench_report = benchmark_policy(
        ledger=ledger,
        candidate=candidate_policy,
        baseline=None,
        output=benchmark_report,
        min_gap=0.2,
        holdout_ratio=holdout_ratio,
        split_salt=split_salt,
        split="holdout",
        overwrite=True,
    )
    promoted = promote_policy(
        candidate=candidate_policy,
        benchmark=benchmark_report,
        active=active_policy,
        report_path=promotion_report,
        min_accuracy=0.9,
        min_improvement=0.4,
        overwrite=True,
    )

    export_result = run_command(
        "cli_export_events_jsonl",
        [
            str(args.cli),
            "learning",
            "export-events-jsonl",
            "--storage",
            str(storage),
            "--out",
            str(export_jsonl),
            "--overwrite",
        ],
        timeout=180,
    )
    report["commands"][export_result.name] = export_result.as_report()
    exported_rows = count_jsonl(export_jsonl)
    final_state = inspect_state(ledger, learning_jsonl, storage, args.cli)
    final_ldb = ldb_count(storage, "learning_events", report, "final")
    if exported_rows != 4 or final_state.get("rocksdb_count_cli") != 4 or final_ldb != 4:
        raise WorkflowLearningError(
            f"final physical counts unexpected: exported={exported_rows}, "
            f"cli={final_state.get('rocksdb_count_cli')}, ldb={final_ldb}"
        )
    report.update(
        {
            "status": "ok",
            "final_state": final_state,
            "rocksdb_ldb_count": final_ldb,
            "exported_learning_event_rows": exported_rows,
            "exported_learning_events_sha256": sha256_file(export_jsonl),
            "pairs": pairs_report,
            "policy": policy_report,
            "benchmark": bench_report,
            "promotion": promoted,
        }
    )
    write_json_atomic(args.report, report)
    print(json.dumps(report, indent=2, sort_keys=True))
    return report


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)

    record = sub.add_parser("record", help="Validate and persist one workflow event")
    record.add_argument("--event", type=Path, required=True)
    record.add_argument("--ledger", type=Path, default=DEFAULT_LEDGER)
    record.add_argument("--learning-jsonl", type=Path, default=DEFAULT_LEARNING_JSONL)
    record.add_argument("--storage", type=Path)
    record.add_argument("--cli", type=Path, default=DEFAULT_CLI)

    pairs = sub.add_parser("build-pairs", help="Mine workflow preference pairs")
    pairs.add_argument("--ledger", type=Path, default=DEFAULT_LEDGER)
    pairs.add_argument("--output", type=Path, default=DEFAULT_PAIR_JSONL)
    pairs.add_argument("--min-gap", type=float, default=0.2)
    pairs.add_argument("--min-chosen-utility", type=float, default=0.05)
    pairs.add_argument("--max-rejected-utility", type=float, default=0.0)
    pairs.add_argument("--split", choices=["all", "train", "holdout"], default="all")
    pairs.add_argument("--holdout-ratio", type=float, default=0.2)
    pairs.add_argument("--split-salt", default="agent-workflow-learning")
    pairs.add_argument("--overwrite", action="store_true")

    train = sub.add_parser("train-policy", help="Train a replay policy from workflow events")
    train.add_argument("--ledger", type=Path, default=DEFAULT_LEDGER)
    train.add_argument("--output", type=Path, default=DEFAULT_POLICY)
    train.add_argument("--min-events", type=int, default=10)
    train.add_argument("--holdout-ratio", type=float, default=0.2)
    train.add_argument("--split-salt", default="agent-workflow-learning")
    train.add_argument("--overwrite", action="store_true")

    bench = sub.add_parser("benchmark-policy", help="Benchmark candidate against baseline")
    bench.add_argument("--ledger", type=Path, default=DEFAULT_LEDGER)
    bench.add_argument("--candidate", type=Path, default=DEFAULT_POLICY)
    bench.add_argument("--baseline", type=Path)
    bench.add_argument("--output", type=Path, default=DEFAULT_HOME / "benchmark.json")
    bench.add_argument("--min-gap", type=float, default=0.2)
    bench.add_argument("--holdout-ratio", type=float, default=0.2)
    bench.add_argument("--split-salt", default="agent-workflow-learning")
    bench.add_argument("--split", choices=["all", "train", "holdout"], default="holdout")
    bench.add_argument("--overwrite", action="store_true")

    promote = sub.add_parser("promote", help="Promote candidate only if benchmark is better")
    promote.add_argument("--candidate", type=Path, default=DEFAULT_POLICY)
    promote.add_argument("--benchmark", type=Path, default=DEFAULT_HOME / "benchmark.json")
    promote.add_argument("--active", type=Path, default=DEFAULT_ACTIVE_POLICY)
    promote.add_argument("--report", type=Path, default=DEFAULT_HOME / "promotion.json")
    promote.add_argument("--min-accuracy", type=float, default=0.55)
    promote.add_argument("--min-improvement", type=float, default=0.01)
    promote.add_argument("--overwrite", action="store_true")

    inspect = sub.add_parser("inspect", help="Read back physical source-of-truth state")
    inspect.add_argument("--ledger", type=Path, default=DEFAULT_LEDGER)
    inspect.add_argument("--learning-jsonl", type=Path, default=DEFAULT_LEARNING_JSONL)
    inspect.add_argument("--storage", type=Path)
    inspect.add_argument("--cli", type=Path, default=DEFAULT_CLI)

    fsv = sub.add_parser("run-fsv", help="Run full-state verification for this pipeline")
    fsv.add_argument("--run-dir", type=Path)
    fsv.add_argument("--cli", type=Path, default=DEFAULT_CLI)
    fsv.add_argument(
        "--report",
        type=Path,
        default=DEFAULT_REPORT_DIR / "agent_workflow_learning_fsv_latest.json",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        if args.command == "record":
            payload = record_event(
                event_path=args.event,
                ledger=args.ledger,
                learning_jsonl=args.learning_jsonl,
                storage=args.storage,
                cli=args.cli,
            )
        elif args.command == "build-pairs":
            payload = write_pairs(
                ledger=args.ledger,
                output=args.output,
                min_gap=args.min_gap,
                min_chosen_utility=args.min_chosen_utility,
                max_rejected_utility=args.max_rejected_utility,
                overwrite=args.overwrite,
                split=args.split,
                holdout_ratio=args.holdout_ratio,
                split_salt=args.split_salt,
            )
        elif args.command == "train-policy":
            payload = train_policy(
                ledger=args.ledger,
                output=args.output,
                min_events=args.min_events,
                holdout_ratio=args.holdout_ratio,
                split_salt=args.split_salt,
                overwrite=args.overwrite,
            )
        elif args.command == "benchmark-policy":
            payload = benchmark_policy(
                ledger=args.ledger,
                candidate=args.candidate,
                baseline=args.baseline,
                output=args.output,
                min_gap=args.min_gap,
                holdout_ratio=args.holdout_ratio,
                split_salt=args.split_salt,
                split=args.split,
                overwrite=args.overwrite,
            )
        elif args.command == "promote":
            payload = promote_policy(
                candidate=args.candidate,
                benchmark=args.benchmark,
                active=args.active,
                report_path=args.report,
                min_accuracy=args.min_accuracy,
                min_improvement=args.min_improvement,
                overwrite=args.overwrite,
            )
        elif args.command == "inspect":
            payload = inspect_state(args.ledger, args.learning_jsonl, args.storage, args.cli)
        elif args.command == "run-fsv":
            payload = run_fsv(args)
        else:
            raise WorkflowLearningError(f"unknown command: {args.command}")
        print(json.dumps(payload, indent=2, sort_keys=True))
        return 0
    except Exception as exc:
        record_failure(args.command, vars(args), exc)
        print(
            json.dumps(
                {
                    "status": "error",
                    "command": args.command,
                    "error": str(exc),
                    "diagnostics_dir": str(DEFAULT_HOME / "failures"),
                },
                sort_keys=True,
            ),
            file=sys.stderr,
        )
        return 1


if __name__ == "__main__":
    sys.exit(main())
