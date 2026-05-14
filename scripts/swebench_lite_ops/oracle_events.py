from __future__ import annotations

import hashlib
import json
import os
import re
import shutil
import subprocess
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from .core import SwebenchOpsError, ensure_file, run, utc_now, write_json_checked


SCHEMA_VERSION = 1
TASK_ID = "TASK-PY-DATA-008"
PRODUCER = "contextgraph.swebench_lite_ops.oracle_events"
DATACONTENTTYPE = "application/json"
HOT_ALLOWED_ROOTS = (
    Path("/home/cabdru/.cache/contextgraph"),
    Path("/var/tmp/contextgraph"),
)
EDGE_BASE_ROW_ID = "astropy__astropy-12907::known_good"
ARTIFACT_KEYS = (
    "command_log",
    "edge_after",
    "edge_before",
    "patch_diff",
    "report",
    "run_instance_log",
    "summary",
    "test_output",
)
EVENT_TYPES = (
    "oracle.run_started",
    "oracle.patch_apply",
    "oracle.test_run",
    "oracle.test_outcome",
    "oracle.timeout",
    "oracle.infra_error",
    "oracle.input_rejected",
    "oracle.final_verdict",
)


class OracleEventError(SwebenchOpsError):
    pass


@dataclass(frozen=True)
class CorpusLink:
    row: dict[str, Any]
    kind: str


def canonical_json(payload: Any) -> str:
    return json.dumps(payload, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def sha256_prefixed_text(text: str) -> str:
    return "sha256:" + hashlib.sha256(text.encode("utf-8")).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return "sha256:" + digest.hexdigest()


def read_json(path: Path, code: str) -> Any:
    ensure_file(path, code, "required JSON artifact")
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise OracleEventError(
            code,
            f"invalid JSON at {path}",
            "Regenerate the upstream artifact before materializing oracle events.",
            {"path": str(path), "error": str(exc)},
        ) from exc


def load_outcomes(path: Path) -> list[dict[str, Any]]:
    ensure_file(path, "MEJEPA_ORACLE_EVENT_OUTCOMES_MISSING", "oracle outcomes JSONL")
    rows: list[dict[str, Any]] = []
    for line_number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
        if not line.strip():
            continue
        try:
            row = json.loads(line)
        except json.JSONDecodeError as exc:
            raise OracleEventError(
                "MEJEPA_ORACLE_EVENT_OUTCOME_JSONL_INVALID",
                f"invalid outcome JSONL line {line_number} in {path}",
                "Regenerate oracle_outcomes.jsonl before materializing typed events.",
                {"path": str(path), "line_number": line_number, "error": str(exc)},
            ) from exc
        rows.append(row)
    if not rows:
        raise OracleEventError(
            "MEJEPA_ORACLE_EVENT_OUTCOMES_EMPTY",
            f"no oracle outcomes found in {path}",
            "Run TASK-PY-DATA-007 before TASK-PY-DATA-008.",
            {"path": str(path)},
        )
    return rows


def load_jsonschema_validator(schema: dict[str, Any]):
    try:
        from jsonschema import Draft202012Validator
    except ImportError as exc:
        raise OracleEventError(
            "MEJEPA_ORACLE_EVENT_JSONSCHEMA_MISSING",
            "jsonschema is required for strict typed oracle event validation",
            "Install jsonschema in the active Python environment; do not bypass schema validation.",
            {"module": "jsonschema"},
        ) from exc
    validator = Draft202012Validator(schema)
    Draft202012Validator.check_schema(schema)
    return validator


def oracle_event_schema(schema_path: Path) -> dict[str, Any]:
    nullable_string = {"type": ["string", "null"]}
    sha = {"type": "string", "pattern": "^sha256:[0-9a-f]{64}$"}
    nullable_sha = {"type": ["string", "null"], "pattern": "^sha256:[0-9a-f]{64}$"}
    git_tree = {"type": "string", "pattern": "^[0-9a-f]{40}$"}
    artifact_properties = {key: nullable_string for key in ARTIFACT_KEYS}
    artifact_hash_properties = {key: nullable_sha for key in ARTIFACT_KEYS}
    properties: dict[str, Any] = {
        "schema_version": {"const": SCHEMA_VERSION},
        "event_id": sha,
        "event_index": {"type": "integer", "minimum": 0},
        "event_type": {"enum": list(EVENT_TYPES)},
        "source": {"type": "string", "minLength": 1},
        "subject": {"type": "string", "minLength": 1},
        "dataschema": {"type": "string", "minLength": 1},
        "datacontenttype": {"const": DATACONTENTTYPE},
        "time": {"type": "string", "minLength": 1},
        "producer": {"const": PRODUCER},
        "instance_id": {"type": "string", "minLength": 1},
        "task_id": {"const": TASK_ID},
        "repo": {"type": "string", "minLength": 1},
        "base_commit": {"type": "string", "minLength": 1},
        "mutation_category": {"type": "string", "minLength": 1},
        "corpus_link_kind": {
            "enum": [
                "exact_manifest_row",
                "base_control_for_manifest_row",
                "edge_case_for_manifest_row",
            ]
        },
        "cleaned_manifest_row_id": {"type": "string", "minLength": 1},
        "cleaned_manifest_row_sha256": sha,
        "workspace_tree_hash": git_tree,
        "workspace_state": {"type": "string", "minLength": 1},
        "run_id": {"type": "string", "minLength": 1},
        "variant": {"type": "string", "minLength": 1},
        "command": nullable_string,
        "duration_ms": {"type": ["integer", "null"], "minimum": 0},
        "exit_code": {"type": ["integer", "null"]},
        "log_hash": nullable_sha,
        "artifact_paths": {
            "type": "object",
            "properties": artifact_properties,
            "additionalProperties": False,
        },
        "artifact_hashes": {
            "type": "object",
            "properties": artifact_hash_properties,
            "additionalProperties": False,
        },
        "parent_event_id": nullable_sha,
        "sequence_in_run": {"type": "integer", "minimum": 0},
        "patch_successfully_applied": {"type": ["boolean", "null"]},
        "resolved": {"type": ["boolean", "null"]},
        "completed": {"type": "boolean"},
        "infra_error": {"type": "boolean"},
        "malformed_input": {"type": "boolean"},
        "timeout": {"type": "boolean"},
        "empty_patch": {"type": "boolean"},
        "is_code_failure_label": {"type": "boolean"},
        "verdict_kind": {
            "enum": [
                "started",
                "patch_applied",
                "patch_not_applied",
                "test_run",
                "test_pass",
                "test_fail",
                "timeout",
                "infra_error",
                "input_rejected",
                "empty_patch",
                "pass",
                "fail",
            ]
        },
        "test_id": nullable_string,
        "test_transition": {
            "enum": ["FAIL_TO_PASS", "PASS_TO_PASS", "FAIL_TO_FAIL", "PASS_TO_FAIL", None]
        },
        "test_observed_outcome": {"enum": ["success", "failure", None]},
        "failure_mode_class": {
            "enum": ["assertion_mismatch", "timeout", "infra_error", "malformed_input", None]
        },
        "reason": {"type": "string"},
        "previous_chain_sha256": nullable_sha,
        "payload_sha256": sha,
        "chain_sha256": sha,
    }
    return {
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": str(schema_path),
        "title": "ContextGraph Python SWE-bench OracleEvent v1",
        "type": "object",
        "properties": properties,
        "required": sorted(properties),
        "additionalProperties": False,
    }


def assert_hot_root_allowed(hot_root: Path) -> Path:
    resolved = hot_root.resolve()
    for allowed in HOT_ALLOWED_ROOTS:
        if resolved == allowed or allowed in resolved.parents:
            return resolved
    raise OracleEventError(
        "MEJEPA_ORACLE_EVENT_HOT_ROOT_FORBIDDEN",
        f"hot root is not on approved WSL ext4 cache paths: {hot_root}",
        "Use /home/cabdru/.cache/contextgraph or /var/tmp/contextgraph for hot staging.",
        {"hot_root": str(hot_root), "allowed": [str(path) for path in HOT_ALLOWED_ROOTS]},
    )


def capacity_snapshot(paths: list[Path]) -> dict[str, Any]:
    result = run(["df", "-B1", "--output=source,size,used,avail,pcent,target", *[str(p) for p in paths]], timeout=30)
    rows = []
    for line in result.stdout_tail.splitlines()[1:]:
        parts = line.split()
        if len(parts) < 6:
            continue
        rows.append(
            {
                "source": parts[0],
                "size_bytes": int(parts[1]),
                "used_bytes": int(parts[2]),
                "available_bytes": int(parts[3]),
                "use_percent": parts[4],
                "target": parts[5],
            }
        )
    return {"command": result.to_json(), "rows": rows}


def path_bytes(path: Path) -> int:
    if not path.exists():
        return 0
    if path.is_file():
        return path.stat().st_size
    total = 0
    for root, dirs, files in os.walk(path):
        root_path = Path(root)
        for name in dirs:
            try:
                total += (root_path / name).lstat().st_size
            except FileNotFoundError:
                continue
        for name in files:
            try:
                total += (root_path / name).lstat().st_size
            except FileNotFoundError:
                continue
    return total


def cleaned_index(cleaned_manifest: dict[str, Any]) -> dict[str, dict[str, Any]]:
    rows = cleaned_manifest.get("rows")
    if not isinstance(rows, list):
        raise OracleEventError(
            "MEJEPA_ORACLE_EVENT_CLEANED_MANIFEST_INVALID",
            "cleaned manifest is missing its rows array",
            "Regenerate TASK-PY-DATA-004 cleaned manifest before materializing oracle events.",
            {"keys": sorted(cleaned_manifest.keys())},
        )
    return {row["row_id"]: row for row in rows}


def workspace_base(workspace_manifest: dict[str, Any], instance_id: str) -> dict[str, Any]:
    for state in workspace_manifest.get("states", []):
        if state.get("task_id") == instance_id and state.get("state") == "base":
            ensure_file(Path(state["hot_workspace_path"]) / ".git" / "HEAD", "MEJEPA_ORACLE_EVENT_BASE_WORKSPACE_MISSING", "base workspace git HEAD")
            return state
    raise OracleEventError(
        "MEJEPA_ORACLE_EVENT_BASE_WORKSPACE_NOT_FOUND",
        f"no base workspace state found for {instance_id}",
        "Run TASK-PY-DATA-006 before materializing oracle events.",
        {"instance_id": instance_id},
    )


def corpus_link(outcome: dict[str, Any], rows_by_id: dict[str, dict[str, Any]]) -> CorpusLink:
    instance_id = outcome["instance_id"]
    variant = outcome["variant"]
    if variant == "wrong_file":
        row_id = f"{instance_id}::wrong_file"
        kind = "exact_manifest_row"
    elif variant == "known_good":
        row_id = f"{instance_id}::known_good"
        kind = "exact_manifest_row"
    elif variant == "base_control":
        row_id = f"{instance_id}::known_good"
        kind = "base_control_for_manifest_row"
    else:
        row_id = f"{instance_id}::known_good"
        kind = "edge_case_for_manifest_row"
    row = rows_by_id.get(row_id)
    if row is None:
        raise OracleEventError(
            "MEJEPA_ORACLE_EVENT_MANIFEST_LINK_MISSING",
            f"oracle outcome {variant} cannot link to cleaned manifest row {row_id}",
            "Regenerate cleaned corpus manifest or fix the oracle outcome variant mapping.",
            {"variant": variant, "row_id": row_id},
        )
    return CorpusLink(row=row, kind=kind)


def validate_outcome(outcome: dict[str, Any]) -> None:
    required = {"classification", "completed", "instance_id", "paths", "run_id", "variant"}
    missing = sorted(required - set(outcome))
    if missing:
        raise OracleEventError(
            "MEJEPA_ORACLE_EVENT_OUTCOME_FIELD_MISSING",
            f"oracle outcome is missing required fields: {missing}",
            "Regenerate TASK-PY-DATA-007 outcomes before materializing typed events.",
            {"missing": missing, "outcome": outcome},
        )
    flags = {
        "infra_error": bool(outcome.get("infra_error")),
        "malformed_input": bool(outcome.get("malformed_input")),
        "timeout": bool(outcome.get("timeout")),
        "empty_patch": bool(outcome.get("empty_patch")),
    }
    if sum(1 for flag in flags.values() if flag) > 1:
        raise OracleEventError(
            "MEJEPA_ORACLE_EVENT_AMBIGUOUS_VERDICT",
            "oracle outcome has multiple terminal edge-case flags",
            "Fix the oracle parser so every outcome has exactly one terminal class.",
            {"variant": outcome["variant"], "flags": flags},
        )
    classification = outcome["classification"]
    if classification in {"resolved", "unresolved"}:
        if flags != {"infra_error": False, "malformed_input": False, "timeout": False, "empty_patch": False}:
            raise OracleEventError(
                "MEJEPA_ORACLE_EVENT_AMBIGUOUS_VERDICT",
                "code verdict carries an infrastructure/input terminal flag",
                "Fix the oracle parser classification before using this as reward data.",
                {"variant": outcome["variant"], "classification": classification, "flags": flags},
            )
        if not bool(outcome.get("completed")) or outcome.get("resolved") != (classification == "resolved"):
            raise OracleEventError(
                "MEJEPA_ORACLE_EVENT_AMBIGUOUS_VERDICT",
                "oracle classification disagrees with completed/resolved fields",
                "Fix the oracle parser so classification is a deterministic function of report fields.",
                {
                    "variant": outcome["variant"],
                    "classification": classification,
                    "completed": outcome.get("completed"),
                    "resolved": outcome.get("resolved"),
                },
            )
    elif classification not in {"infra_error", "malformed_input", "empty_patch", "timeout"}:
        raise OracleEventError(
            "MEJEPA_ORACLE_EVENT_UNKNOWN_CLASSIFICATION",
            f"unknown oracle classification: {classification}",
            "Update the strict schema and classifier intentionally before accepting a new class.",
            {"variant": outcome["variant"], "classification": classification},
        )
    expected_flag = {
        "infra_error": "infra_error",
        "malformed_input": "malformed_input",
        "empty_patch": "empty_patch",
        "timeout": "timeout",
    }.get(classification)
    if expected_flag and not flags[expected_flag]:
        raise OracleEventError(
            "MEJEPA_ORACLE_EVENT_AMBIGUOUS_VERDICT",
            "edge-case classification does not set its corresponding terminal flag",
            "Fix the oracle parser before accepting this event as reward data.",
            {"variant": outcome["variant"], "classification": classification, "flags": flags},
        )


def artifact_maps(outcome: dict[str, Any], require_log: bool = True) -> tuple[dict[str, str | None], dict[str, str | None]]:
    paths = {key: None for key in ARTIFACT_KEYS}
    hashes = {key: None for key in ARTIFACT_KEYS}
    for key, value in sorted(outcome.get("paths", {}).items()):
        if key not in paths:
            raise OracleEventError(
                "MEJEPA_ORACLE_EVENT_UNKNOWN_ARTIFACT_KEY",
                f"unknown oracle artifact key: {key}",
                "Update the strict artifact schema intentionally before accepting this artifact.",
                {"variant": outcome.get("variant"), "key": key},
            )
        if value is None:
            continue
        path = Path(value)
        if key == "command_log" or path.exists():
            ensure_file(path, "MEJEPA_ORACLE_EVENT_MISSING_LOG" if key == "command_log" else "MEJEPA_ORACLE_EVENT_ARTIFACT_MISSING", f"oracle {key}")
            hashes[key] = sha256_file(path)
        paths[key] = str(path)
    if require_log and paths["command_log"] is None:
        raise OracleEventError(
            "MEJEPA_ORACLE_EVENT_MISSING_LOG",
            "oracle outcome has no command_log path",
            "Regenerate TASK-PY-DATA-007 with command-log capture enabled.",
            {"variant": outcome.get("variant")},
        )
    return paths, hashes


def parse_duration_ms(command_log: str | None) -> int | None:
    if not command_log:
        return None
    path = Path(command_log)
    if not path.is_file():
        return None
    text = path.read_text(encoding="utf-8", errors="replace")
    matches = re.findall(r"([0-9]+(?:\.[0-9]+)?)s/it", text)
    if matches:
        return int(float(matches[-1]) * 1000)
    return None


def command_string(outcome: dict[str, Any]) -> str:
    timeout = "1" if outcome.get("timeout") else "900"
    prediction = outcome.get("paths", {}).get("summary") or outcome.get("paths", {}).get("patch_diff") or "gold"
    return " ".join(
        [
            "/mnt/d/contextgraph/cache/contextgraph-swebench/venv/bin/python",
            "-m swebench.harness.run_evaluation",
            "--dataset_name SWE-bench/SWE-bench_Lite",
            "--split test",
            f"--predictions_path {prediction}",
            f"--run_id {outcome['run_id']}",
            f"--instance_ids {outcome['instance_id']}",
            "--max_workers 1",
            f"--timeout {timeout}",
            "--namespace swebench",
        ]
    )


def run_git(command: list[str], cwd: Path) -> str:
    completed = subprocess.run(command, cwd=str(cwd), text=True, capture_output=True, check=False)
    if completed.returncode != 0:
        raise OracleEventError(
            "MEJEPA_ORACLE_EVENT_GIT_COMMAND_FAILED",
            f"git command failed in hot workspace: {' '.join(command)}",
            "Inspect stdout/stderr and repair the hot workspace or patch artifact.",
            {
                "cwd": str(cwd),
                "command": command,
                "exit_code": completed.returncode,
                "stdout_tail": completed.stdout[-4000:],
                "stderr_tail": completed.stderr[-4000:],
            },
        )
    return completed.stdout.strip()


def compute_tree_hashes(
    outcomes: list[dict[str, Any]],
    workspace_manifest: dict[str, Any],
    hot_root: Path,
    budget_bytes: int,
) -> dict[str, dict[str, Any]]:
    hot_root = assert_hot_root_allowed(hot_root)
    staging_root = hot_root / "tree-staging"
    if staging_root.exists():
        shutil.rmtree(staging_root)
    staging_root.mkdir(parents=True, exist_ok=True)
    tree_by_variant: dict[str, dict[str, Any]] = {}
    base_by_instance: dict[str, dict[str, Any]] = {}

    for outcome in outcomes:
        instance_id = outcome["instance_id"]
        if instance_id not in base_by_instance:
            base_by_instance[instance_id] = workspace_base(workspace_manifest, instance_id)
        base_state = base_by_instance[instance_id]
        base_tree = base_state["base_tree_hash"]
        variant = outcome["variant"]
        patch_path = outcome.get("paths", {}).get("patch_diff")
        if not outcome.get("patch_successfully_applied") or not patch_path:
            tree_by_variant[variant] = {
                "workspace_tree_hash": base_tree,
                "workspace_state": "base",
                "status": "",
                "hot_workspace_path": None,
            }
            continue

        stage = staging_root / variant
        shutil.copytree(base_state["hot_workspace_path"], stage)
        patch = Path(patch_path)
        ensure_file(patch, "MEJEPA_ORACLE_EVENT_PATCH_DIFF_MISSING", "oracle patch diff")
        run_git(["git", "apply", "--whitespace=nowarn", str(patch)], stage)
        run_git(["git", "add", "-A"], stage)
        tree_hash = run_git(["git", "write-tree"], stage)
        status = run_git(["git", "status", "--short"], stage)
        tree_by_variant[variant] = {
            "workspace_tree_hash": tree_hash,
            "workspace_state": "post_patch",
            "status": status,
            "hot_workspace_path": str(stage),
        }

    actual_bytes = path_bytes(hot_root)
    if actual_bytes > budget_bytes:
        raise OracleEventError(
            "MEJEPA_ORACLE_EVENT_HOT_BUDGET_EXCEEDED",
            f"hot oracle-event staging used {actual_bytes} bytes, over budget {budget_bytes}",
            "Reduce hot staging volume or increase the explicit task budget with capacity evidence.",
            {"hot_root": str(hot_root), "actual_bytes": actual_bytes, "budget_bytes": budget_bytes},
        )
    return tree_by_variant


def report_tests(outcome: dict[str, Any]) -> list[dict[str, Any]]:
    report_path = outcome.get("paths", {}).get("report")
    if not report_path:
        return []
    report = read_json(Path(report_path), "MEJEPA_ORACLE_EVENT_REPORT_INVALID")
    if sorted(report.keys()) != [outcome["instance_id"]]:
        raise OracleEventError(
            "MEJEPA_ORACLE_EVENT_REPORT_ROOT_AMBIGUOUS",
            "SWE-bench report root must contain exactly the outcome instance id",
            "Regenerate oracle reports; do not accept ambiguous report roots.",
            {"variant": outcome["variant"], "report_keys": sorted(report.keys())},
        )
    tests_status = report[outcome["instance_id"]].get("tests_status", {})
    tests: list[dict[str, Any]] = []
    for transition in ("FAIL_TO_PASS", "PASS_TO_PASS", "FAIL_TO_FAIL", "PASS_TO_FAIL"):
        block = tests_status.get(transition, {})
        for observed in ("success", "failure"):
            for test_id in block.get(observed, []):
                tests.append({"transition": transition, "observed": observed, "test_id": test_id})
    return tests


def final_verdict_kind(outcome: dict[str, Any]) -> str:
    if outcome.get("infra_error"):
        return "infra_error"
    if outcome.get("malformed_input"):
        return "input_rejected"
    if outcome.get("empty_patch"):
        return "empty_patch"
    if outcome.get("timeout"):
        return "timeout"
    return "pass" if outcome.get("resolved") else "fail"


def is_code_failure(outcome: dict[str, Any]) -> bool:
    return bool(outcome.get("completed")) and not bool(outcome.get("resolved")) and not any(
        bool(outcome.get(flag)) for flag in ("infra_error", "malformed_input", "timeout", "empty_patch")
    )


def make_base_event(
    *,
    outcome: dict[str, Any],
    link: CorpusLink,
    schema_path: Path,
    event_type: str,
    sequence: int,
    parent_event_id: str | None,
    tree: dict[str, Any],
    artifact_paths: dict[str, str | None],
    artifact_hashes: dict[str, str | None],
    duration_ms: int | None,
    event_time: str,
    overrides: dict[str, Any],
) -> dict[str, Any]:
    row = link.row
    base = {
        "schema_version": SCHEMA_VERSION,
        "event_id": None,
        "event_index": -1,
        "event_type": event_type,
        "source": f"contextgraph://swebench-lite-python/{outcome['run_id']}",
        "subject": f"{outcome['instance_id']}#{outcome['variant']}#{sequence}",
        "dataschema": str(schema_path),
        "datacontenttype": DATACONTENTTYPE,
        "time": event_time,
        "producer": PRODUCER,
        "instance_id": outcome["instance_id"],
        "task_id": TASK_ID,
        "repo": row["repo"],
        "base_commit": row["base_commit"],
        "mutation_category": row["mutation_category"],
        "corpus_link_kind": link.kind,
        "cleaned_manifest_row_id": row["row_id"],
        "cleaned_manifest_row_sha256": row["row_sha256"],
        "workspace_tree_hash": tree["workspace_tree_hash"],
        "workspace_state": tree["workspace_state"],
        "run_id": outcome["run_id"],
        "variant": outcome["variant"],
        "command": command_string(outcome),
        "duration_ms": duration_ms,
        "exit_code": outcome.get("return_code") if "return_code" in outcome else (0 if outcome.get("completed") else None),
        "log_hash": artifact_hashes.get("command_log"),
        "artifact_paths": artifact_paths,
        "artifact_hashes": artifact_hashes,
        "parent_event_id": parent_event_id,
        "sequence_in_run": sequence,
        "patch_successfully_applied": outcome.get("patch_successfully_applied"),
        "resolved": outcome.get("resolved"),
        "completed": bool(outcome.get("completed")),
        "infra_error": bool(outcome.get("infra_error")),
        "malformed_input": bool(outcome.get("malformed_input")),
        "timeout": bool(outcome.get("timeout")),
        "empty_patch": bool(outcome.get("empty_patch")),
        "is_code_failure_label": is_code_failure(outcome),
        "verdict_kind": "started",
        "test_id": None,
        "test_transition": None,
        "test_observed_outcome": None,
        "failure_mode_class": None,
        "reason": "",
        "previous_chain_sha256": None,
        "payload_sha256": None,
        "chain_sha256": None,
    }
    base.update(overrides)
    identity = {
        "event_type": base["event_type"],
        "instance_id": base["instance_id"],
        "run_id": base["run_id"],
        "sequence_in_run": base["sequence_in_run"],
        "test_id": base["test_id"],
        "variant": base["variant"],
    }
    base["event_id"] = sha256_prefixed_text(canonical_json(identity))
    return base


def finalize_events(events: list[dict[str, Any]], validator: Any) -> None:
    previous: str | None = None
    for index, event in enumerate(events):
        event["event_index"] = index
        event["previous_chain_sha256"] = previous
        payload = {key: value for key, value in event.items() if key not in {"payload_sha256", "chain_sha256"}}
        event["payload_sha256"] = sha256_prefixed_text(canonical_json(payload))
        event["chain_sha256"] = sha256_prefixed_text((previous or "GENESIS") + "\n" + event["payload_sha256"])
        errors = sorted(validator.iter_errors(event), key=lambda err: list(err.path))
        if errors:
            raise OracleEventError(
                "MEJEPA_ORACLE_EVENT_SCHEMA_VALIDATION_FAILED",
                "generated oracle event failed strict JSON Schema validation",
                "Fix the event builder before writing reward data.",
                {"event_type": event.get("event_type"), "errors": [err.message for err in errors[:10]]},
            )
        previous = event["chain_sha256"]


def build_events(
    outcomes: list[dict[str, Any]],
    rows_by_id: dict[str, dict[str, Any]],
    workspace_manifest: dict[str, Any],
    schema_path: Path,
    hot_root: Path,
    hot_budget_bytes: int,
    validator: Any,
) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    for outcome in outcomes:
        validate_outcome(outcome)
    trees = compute_tree_hashes(outcomes, workspace_manifest, hot_root, hot_budget_bytes)
    events: list[dict[str, Any]] = []
    event_time = utc_now()

    for outcome in outcomes:
        link = corpus_link(outcome, rows_by_id)
        paths, hashes = artifact_maps(outcome)
        duration_ms = parse_duration_ms(paths["command_log"])
        tree = trees[outcome["variant"]]
        sequence = 0
        run_event = make_base_event(
            outcome=outcome,
            link=link,
            schema_path=schema_path,
            event_type="oracle.run_started",
            sequence=sequence,
            parent_event_id=None,
            tree=tree,
            artifact_paths=paths,
            artifact_hashes=hashes,
            duration_ms=duration_ms,
            event_time=event_time,
            overrides={"verdict_kind": "started", "reason": "SWE-bench oracle run started from persisted outcome row."},
        )
        events.append(run_event)
        parent = run_event["event_id"]

        if outcome.get("malformed_input") or outcome.get("empty_patch"):
            sequence += 1
            kind = "empty_patch" if outcome.get("empty_patch") else "input_rejected"
            events.append(
                make_base_event(
                    outcome=outcome,
                    link=link,
                    schema_path=schema_path,
                    event_type="oracle.input_rejected",
                    sequence=sequence,
                    parent_event_id=parent,
                    tree=tree,
                    artifact_paths=paths,
                    artifact_hashes=hashes,
                    duration_ms=duration_ms,
                    event_time=event_time,
                    overrides={
                        "verdict_kind": kind,
                        "failure_mode_class": "malformed_input" if outcome.get("malformed_input") else None,
                        "reason": f"SWE-bench rejected input as {kind}.",
                    },
                )
            )
        else:
            sequence += 1
            patch_applied = bool(outcome.get("patch_successfully_applied"))
            events.append(
                make_base_event(
                    outcome=outcome,
                    link=link,
                    schema_path=schema_path,
                    event_type="oracle.patch_apply",
                    sequence=sequence,
                    parent_event_id=parent,
                    tree=tree,
                    artifact_paths=paths,
                    artifact_hashes=hashes,
                    duration_ms=duration_ms,
                    event_time=event_time,
                    overrides={
                        "verdict_kind": "patch_applied" if patch_applied else "patch_not_applied",
                        "reason": "Patch artifact applied successfully." if patch_applied else "Patch was not applied before terminal oracle error.",
                    },
                )
            )
            if outcome.get("infra_error"):
                sequence += 1
                events.append(
                    make_base_event(
                        outcome=outcome,
                        link=link,
                        schema_path=schema_path,
                        event_type="oracle.infra_error",
                        sequence=sequence,
                        parent_event_id=parent,
                        tree=tree,
                        artifact_paths=paths,
                        artifact_hashes=hashes,
                        duration_ms=duration_ms,
                        event_time=event_time,
                        overrides={
                            "verdict_kind": "infra_error",
                            "failure_mode_class": "infra_error",
                            "reason": "Docker/SWE-bench infrastructure failed before a code verdict.",
                        },
                    )
                )
            else:
                sequence += 1
                events.append(
                    make_base_event(
                        outcome=outcome,
                        link=link,
                        schema_path=schema_path,
                        event_type="oracle.test_run",
                        sequence=sequence,
                        parent_event_id=parent,
                        tree=tree,
                        artifact_paths=paths,
                        artifact_hashes=hashes,
                        duration_ms=duration_ms,
                        event_time=event_time,
                        overrides={"verdict_kind": "test_run", "reason": "SWE-bench test command reached execution."},
                    )
                )
                if outcome.get("timeout"):
                    sequence += 1
                    events.append(
                        make_base_event(
                            outcome=outcome,
                            link=link,
                            schema_path=schema_path,
                            event_type="oracle.timeout",
                            sequence=sequence,
                            parent_event_id=parent,
                            tree=tree,
                            artifact_paths=paths,
                            artifact_hashes=hashes,
                            duration_ms=duration_ms,
                            event_time=event_time,
                            overrides={
                                "verdict_kind": "timeout",
                                "failure_mode_class": "timeout",
                                "reason": "SWE-bench test execution timed out before a code verdict.",
                            },
                        )
                    )
                else:
                    for test in report_tests(outcome):
                        sequence += 1
                        passed = test["observed"] == "success"
                        events.append(
                            make_base_event(
                                outcome=outcome,
                                link=link,
                                schema_path=schema_path,
                                event_type="oracle.test_outcome",
                                sequence=sequence,
                                parent_event_id=parent,
                                tree=tree,
                                artifact_paths=paths,
                                artifact_hashes=hashes,
                                duration_ms=duration_ms,
                                event_time=event_time,
                                overrides={
                                    "verdict_kind": "test_pass" if passed else "test_fail",
                                    "test_id": test["test_id"],
                                    "test_transition": test["transition"],
                                    "test_observed_outcome": test["observed"],
                                    "failure_mode_class": None if passed else "assertion_mismatch",
                                    "reason": f"{test['transition']} test observed {test['observed']}.",
                                },
                            )
                        )

        sequence += 1
        final_kind = final_verdict_kind(outcome)
        events.append(
            make_base_event(
                outcome=outcome,
                link=link,
                schema_path=schema_path,
                event_type="oracle.final_verdict",
                sequence=sequence,
                parent_event_id=parent,
                tree=tree,
                artifact_paths=paths,
                artifact_hashes=hashes,
                duration_ms=duration_ms,
                event_time=event_time,
                overrides={
                    "verdict_kind": final_kind,
                    "failure_mode_class": "assertion_mismatch" if final_kind == "fail" else ("timeout" if final_kind == "timeout" else ("infra_error" if final_kind == "infra_error" else ("malformed_input" if final_kind == "input_rejected" else None))),
                    "reason": f"Final deterministic oracle verdict: {final_kind}.",
                },
            )
        )

    finalize_events(events, validator)
    return events, {"tree_hashes": trees, "event_time": event_time}


def write_jsonl_checked(path: Path, events: list[dict[str, Any]]) -> dict[str, Any]:
    path.parent.mkdir(parents=True, exist_ok=True)
    text = "\n".join(canonical_json(event) for event in events) + "\n"
    tmp = path.with_suffix(path.suffix + ".tmp")
    with tmp.open("w", encoding="utf-8") as handle:
        handle.write(text)
        handle.flush()
        os.fsync(handle.fileno())
    tmp.replace(path)
    readback = [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line.strip()]
    if readback != events:
        raise OracleEventError(
            "MEJEPA_ORACLE_EVENT_JSONL_READBACK_MISMATCH",
            f"oracle event JSONL readback mismatch at {path}",
            "Check filesystem integrity and concurrent writers before using this event store.",
            {"path": str(path)},
        )
    return {"path": str(path), "row_count": len(readback), "sha256": sha256_file(path)}


def count_event_file(path: Path) -> int:
    if not path.is_file():
        return 0
    return sum(1 for line in path.read_text(encoding="utf-8").splitlines() if line.strip())


def file_state(path: Path) -> dict[str, Any]:
    if not path.is_file():
        return {"path": str(path), "exists": False, "bytes": 0, "sha256": None}
    return {
        "path": str(path),
        "exists": True,
        "bytes": path.stat().st_size,
        "sha256": sha256_file(path),
    }


def event_counts(events: list[dict[str, Any]]) -> dict[str, Any]:
    counts: dict[str, int] = {}
    final_by_variant: dict[str, int] = {}
    tests_by_variant: dict[str, int] = {}
    for event in events:
        counts[event["event_type"]] = counts.get(event["event_type"], 0) + 1
        if event["event_type"] == "oracle.final_verdict":
            final_by_variant[event["variant"]] = final_by_variant.get(event["variant"], 0) + 1
        if event["event_type"] == "oracle.test_outcome":
            tests_by_variant[event["variant"]] = tests_by_variant.get(event["variant"], 0) + 1
    return {
        "total": len(events),
        "by_event_type": counts,
        "final_by_variant": final_by_variant,
        "test_outcomes_by_variant": tests_by_variant,
    }


def edge_audit(
    *,
    outcomes: list[dict[str, Any]],
    events: list[dict[str, Any]],
    event_path: Path,
    validator: Any,
) -> list[dict[str, Any]]:
    audit: list[dict[str, Any]] = []

    before = count_event_file(event_path)
    ambiguous = dict(outcomes[0])
    ambiguous["classification"] = "resolved"
    ambiguous["resolved"] = False
    try:
        validate_outcome(ambiguous)
        error = None
    except SwebenchOpsError as exc:
        error = exc.code
    audit.append(
        {
            "case": "ambiguous_parser_output",
            "input": {"variant": ambiguous["variant"], "classification": ambiguous["classification"], "resolved": ambiguous["resolved"]},
            "before_state": {"event_count": before},
            "after_state": {"event_count": count_event_file(event_path)},
            "expected": "MEJEPA_ORACLE_EVENT_AMBIGUOUS_VERDICT and no event write",
            "observed_error_code": error,
            "pass": error == "MEJEPA_ORACLE_EVENT_AMBIGUOUS_VERDICT" and before == count_event_file(event_path),
        }
    )

    before = count_event_file(event_path)
    missing_log = json.loads(canonical_json(outcomes[1]))
    missing_log["paths"]["command_log"] = "/mnt/d/contextgraph/corpus/python-oracle-baselines-v1/runs/missing-command-log.log"
    try:
        artifact_maps(missing_log)
        error = None
    except SwebenchOpsError as exc:
        error = exc.code
    audit.append(
        {
            "case": "missing_log_file",
            "input": {"variant": missing_log["variant"], "command_log": missing_log["paths"]["command_log"]},
            "before_state": {"event_count": before},
            "after_state": {"event_count": count_event_file(event_path)},
            "expected": "MEJEPA_ORACLE_EVENT_MISSING_LOG and no event write",
            "observed_error_code": error,
            "pass": error == "MEJEPA_ORACLE_EVENT_MISSING_LOG" and before == count_event_file(event_path),
        }
    )

    before = count_event_file(event_path)
    rogue = dict(events[0])
    rogue["rogue_field"] = "must fail closed"
    schema_errors = sorted(validator.iter_errors(rogue), key=lambda err: list(err.path))
    audit.append(
        {
            "case": "unknown_field_rejected",
            "input": {"rogue_field": rogue["rogue_field"]},
            "before_state": {"event_count": before},
            "after_state": {"event_count": count_event_file(event_path)},
            "expected": "strict schema rejects additionalProperties and no event write",
            "observed_error_count": len(schema_errors),
            "observed_errors": [err.message for err in schema_errors[:3]],
            "pass": bool(schema_errors) and before == count_event_file(event_path),
        }
    )

    before = count_event_file(event_path)
    infra_final = [
        event for event in events
        if event["variant"] == "docker_unavailable" and event["event_type"] == "oracle.final_verdict"
    ]
    audit.append(
        {
            "case": "infra_error_not_code_failure",
            "input": {"variant": "docker_unavailable"},
            "before_state": {"event_count": before},
            "after_state": {"event_count": count_event_file(event_path), "final_event": infra_final},
            "expected": "verdict_kind=infra_error, is_code_failure_label=false",
            "pass": len(infra_final) == 1 and infra_final[0]["verdict_kind"] == "infra_error" and infra_final[0]["is_code_failure_label"] is False,
        }
    )
    return audit


def command_oracle_events(args: Any) -> dict[str, Any]:
    checked_at = utc_now()
    args.event_root.mkdir(parents=True, exist_ok=True)
    args.fsv_root.mkdir(parents=True, exist_ok=True)
    schema_path = args.event_root / "oracle_event_schema.json"
    event_path = args.event_root / "oracle_events.jsonl"
    manifest_path = args.event_root / "oracle_event_manifest.json"
    fsv_path = args.fsv_root / "oracle_events_fsv.json"

    before_state = {
        "checked_at": checked_at,
        "event_root_exists": args.event_root.exists(),
        "event_count": count_event_file(event_path),
        "event_jsonl": file_state(event_path),
        "event_schema": file_state(schema_path),
        "event_manifest": file_state(manifest_path),
        "event_fsv": file_state(fsv_path),
        "disk": capacity_snapshot([Path("/"), Path("/mnt/c"), Path("/mnt/d")]),
        "hot_bytes": path_bytes(args.hot_root),
        "event_root_bytes": path_bytes(args.event_root),
    }

    outcomes = load_outcomes(args.outcomes_jsonl)
    cleaned_manifest = read_json(args.cleaned_manifest, "MEJEPA_ORACLE_EVENT_CLEANED_MANIFEST_JSON_INVALID")
    workspace_manifest = read_json(args.workspace_manifest, "MEJEPA_ORACLE_EVENT_WORKSPACE_MANIFEST_JSON_INVALID")
    baseline_manifest = read_json(args.outcomes_jsonl.parent / "oracle_baselines_manifest.json", "MEJEPA_ORACLE_EVENT_BASELINE_MANIFEST_JSON_INVALID")
    rows_by_id = cleaned_index(cleaned_manifest)
    schema = oracle_event_schema(schema_path)
    validator = load_jsonschema_validator(schema)
    write_json_checked(schema_path, schema)

    events, build_meta = build_events(
        outcomes,
        rows_by_id,
        workspace_manifest,
        schema_path,
        args.hot_root,
        args.hot_budget_bytes,
        validator,
    )
    event_store = write_jsonl_checked(event_path, events)
    counts = event_counts(events)
    happy_known_good = {
        "final_verdict_count": counts["final_by_variant"].get("known_good", 0),
        "test_outcome_count": counts["test_outcomes_by_variant"].get("known_good", 0),
        "pass": counts["final_by_variant"].get("known_good", 0) == 1 and counts["test_outcomes_by_variant"].get("known_good", 0) == 15,
    }
    edge_cases = edge_audit(outcomes=outcomes, events=events, event_path=event_path, validator=validator)

    all_checks_pass = happy_known_good["pass"] and all(case["pass"] for case in edge_cases)
    if not all_checks_pass:
        raise OracleEventError(
            "MEJEPA_ORACLE_EVENT_FSV_FAILED",
            "oracle event FSV checks failed",
            "Fix the typed event materializer before closing TASK-PY-DATA-008.",
            {"happy_known_good": happy_known_good, "edge_cases": edge_cases},
        )

    manifest = {
        "schema_version": SCHEMA_VERSION,
        "created_at_utc": checked_at,
        "task_id": TASK_ID,
        "issue": 191,
        "source_of_truth": {
            "event_schema": str(schema_path),
            "event_jsonl": str(event_path),
            "manifest": str(manifest_path),
            "fsv": str(fsv_path),
        },
        "inputs": {
            "outcomes_jsonl": str(args.outcomes_jsonl),
            "cleaned_manifest": str(args.cleaned_manifest),
            "workspace_manifest": str(args.workspace_manifest),
            "baseline_manifest_run_root": baseline_manifest["source_of_truth"].get("run_root"),
        },
        "storage_policy": {
            "decision_issue": 212,
            "durable_root": "/mnt/d/contextgraph",
            "hot_root": str(args.hot_root),
            "hot_budget_bytes": args.hot_budget_bytes,
        },
        "counts": counts,
        "chain_head_sha256": events[-1]["chain_sha256"],
        "event_jsonl_sha256": event_store["sha256"],
        "tree_hashes": build_meta["tree_hashes"],
        "passes": True,
    }
    write_json_checked(manifest_path, manifest)

    after_state = {
        "checked_at": utc_now(),
        "event_count": count_event_file(event_path),
        "event_jsonl": file_state(event_path),
        "event_schema": file_state(schema_path),
        "event_manifest": file_state(manifest_path),
        "event_fsv": file_state(fsv_path),
        "disk": capacity_snapshot([Path("/"), Path("/mnt/c"), Path("/mnt/d")]),
        "hot_bytes": path_bytes(args.hot_root),
        "event_root_bytes": path_bytes(args.event_root),
        "fsv_root_bytes": path_bytes(args.fsv_root),
        "event_store": event_store,
    }

    fsv = {
        "schema_version": 1,
        "checked_at_utc": utc_now(),
        "task_id": TASK_ID,
        "issue": 191,
        "source_of_truth": manifest["source_of_truth"],
        "before_state": before_state,
        "trigger": {
            "command": "python3 scripts/swebench_lite_ops.py oracle-events",
            "outcome_count": len(outcomes),
            "schema_validation": "jsonschema Draft 2020-12, additionalProperties=false",
        },
        "after_state": after_state,
        "happy_path": {
            "case": "known_good_real_oracle_log",
            "expected": "exactly one final verdict event and 15 detailed test outcome events",
            "observed": happy_known_good,
            "pass": happy_known_good["pass"],
        },
        "edge_cases": edge_cases,
        "counts": counts,
        "chain_head_sha256": events[-1]["chain_sha256"],
        "passes": True,
    }
    write_json_checked(fsv_path, fsv)

    return {
        "ok": True,
        "checked_at": utc_now(),
        "source_of_truth": manifest["source_of_truth"],
        "before_state": before_state,
        "trigger": fsv["trigger"],
        "after_state": after_state,
        "counts": counts,
        "happy_path": fsv["happy_path"],
        "edge_cases": edge_cases,
        "manifest_sha256": sha256_file(manifest_path),
        "fsv_sha256": sha256_file(fsv_path),
        "passes": True,
    }
