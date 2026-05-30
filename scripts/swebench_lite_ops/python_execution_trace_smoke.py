#!/usr/bin/env python3
"""One-row live execution-trace smoke for INV-010 / #774."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shlex
from collections import Counter
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


SCHEMA_VERSION = 1
TASK_ID = "INV-010"
ARTIFACT_KIND = "execution_trace_smoke"
AIWONDER_ARCHIVE_ROOT = "/zfs/archive/contextgraph/"
AIWONDER_HOT_ROOT = "/zfs/hot/contextgraph/"
DEFAULT_DATASET = "SWE-bench/SWE-bench_Lite"
DEFAULT_SPLIT = "test"
DEFAULT_CORPUS_ROOT = Path("/zfs/archive/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511")
DEFAULT_OUTPUT_ROOT = Path("/zfs/archive/contextgraph/fsv/inv-010-execution-aware-fsv")
DEFAULT_STATE_ROOT = Path("/zfs/archive/contextgraph/state/execution-traces/inv-010-v1")
DEFAULT_VENV_PYTHON = Path("/zfs/hot/contextgraph/venv/bin/python3")
DEFAULT_TASK_ID = "astropy__astropy-12907"
DEFAULT_CATEGORY = "known_good"
TRACE_BEGIN = "CONTEXTGRAPH_EXECUTION_TRACE_BEGIN"
TRACE_END = "CONTEXTGRAPH_EXECUTION_TRACE_END"
TRACE_OUTPUT_PATH = "/tmp/contextgraph_execution_trace.json"
TRACE_RUNNER_PATH = "/tmp/contextgraph_trace_pytest.py"
SUPPORTED_PYTEST_PREFIX = "pytest"


class ExecutionTraceSmokeError(RuntimeError):
    pass


def utc_stamp() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")


def utc_now() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def require(condition: bool, code: str, detail: Any = None) -> None:
    if not condition:
        suffix = "" if detail is None else f": {detail}"
        raise ExecutionTraceSmokeError(f"{code}{suffix}")


def require_archive_path(path: Path, label: str, allow_non_aiwonder_roots: bool) -> None:
    if allow_non_aiwonder_roots:
        return
    require(
        str(path).startswith(AIWONDER_ARCHIVE_ROOT),
        "EXECUTION_TRACE_SMOKE_NON_AIWONDER_DURABLE_PATH",
        {label: str(path), "required_prefix": AIWONDER_ARCHIVE_ROOT},
    )


def require_hot_path(path: Path, label: str, allow_non_aiwonder_roots: bool) -> None:
    if allow_non_aiwonder_roots:
        return
    require(
        str(path).startswith(AIWONDER_HOT_ROOT),
        "EXECUTION_TRACE_SMOKE_NON_AIWONDER_HOT_PATH",
        {label: str(path), "required_prefix": AIWONDER_HOT_ROOT},
    )


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return "sha256:" + digest.hexdigest()


def write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    tmp.replace(path)


def read_json(path: Path, code: str) -> Any:
    require(path.is_file(), code, {"path": str(path)})
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise ExecutionTraceSmokeError(f"{code}: invalid JSON at {path}: {exc}") from exc


def load_corpus_entry(corpus_root: Path, task_id: str, category: str) -> dict[str, Any]:
    doc = read_json(corpus_root / "index.json", "EXECUTION_TRACE_SMOKE_CORPUS_INDEX_MISSING")
    entries = doc.get("entries")
    require(isinstance(entries, list), "EXECUTION_TRACE_SMOKE_CORPUS_INDEX_BAD_SCHEMA")
    matches = [
        row
        for row in entries
        if isinstance(row, dict)
        and str(row.get("task_id") or "") == task_id
        and str(row.get("category") or "") == category
    ]
    require(
        len(matches) == 1,
        "EXECUTION_TRACE_SMOKE_CORPUS_ROW_MATCH_COUNT",
        {"task_id": task_id, "category": category, "matches": len(matches)},
    )
    row = dict(matches[0])
    patch_path = str(row.get("patch_path") or "")
    require(patch_path, "EXECUTION_TRACE_SMOKE_PATCH_PATH_MISSING", {"task_id": task_id, "category": category})
    patch = corpus_root / patch_path
    require(patch.is_file(), "EXECUTION_TRACE_SMOKE_PATCH_FILE_MISSING", {"patch": str(patch)})
    row["patch_abspath"] = str(patch)
    row["patch_sha256"] = sha256_file(patch)
    return row


def build_trace_runner_source() -> str:
    return r'''from __future__ import annotations

import json
import os
import sys
import time
from collections import Counter

import pytest


def main() -> int:
    output_path = os.environ.get("CONTEXTGRAPH_TRACE_OUTPUT", "/tmp/contextgraph_execution_trace.json")
    repo_root = os.environ.get("CONTEXTGRAPH_TRACE_REPO_ROOT", "/testbed")
    max_events = int(os.environ.get("CONTEXTGRAPH_TRACE_MAX_EVENTS", "5000"))
    events = []
    line_counts = Counter()
    event_counts = Counter()
    started = time.monotonic()

    def tracer(frame, event, arg):
        if event not in {"call", "line", "return", "exception"}:
            return tracer
        path = frame.f_code.co_filename
        if not path.startswith(repo_root):
            return tracer
        rel = os.path.relpath(path, repo_root)
        lineno = int(frame.f_lineno)
        event_counts[event] += 1
        if event == "line":
            line_counts[f"{rel}:{lineno}"] += 1
        if len(events) < max_events:
            events.append(
                {
                    "event": event,
                    "file": rel,
                    "function": frame.f_code.co_name,
                    "line": lineno,
                    "local_names": sorted(frame.f_locals.keys())[:16],
                }
            )
        return tracer

    exit_code = 2
    sys.settrace(tracer)
    try:
        exit_code = int(pytest.main(sys.argv[1:]))
    finally:
        sys.settrace(None)
        elapsed_ms = int((time.monotonic() - started) * 1000)
        line_items = sorted(line_counts.items())
        payload = {
            "schema_version": 1,
            "producer": "contextgraph.execution_trace_smoke.pytest_sys_settrace",
            "coverage": {
                "files_touched": len({item.split(":", 1)[0] for item, _count in line_items}),
                "lines_executed": len(line_items),
                "events_recorded": len(events),
                "events_seen": int(sum(event_counts.values())),
                "truncated": int(sum(event_counts.values())) > len(events),
            },
            "line_coverage": [{"location": item, "count": count} for item, count in line_items[:2000]],
            "runtime_metrics": {
                "elapsed_ms": elapsed_ms,
                "pytest_exit_code": exit_code,
                "max_events": max_events,
            },
            "steps": {
                "event_counts": dict(sorted(event_counts.items())),
                "pytest_args": sys.argv[1:],
            },
            "trace_events": events,
            "variable_states": [
                {
                    "event_index": idx,
                    "file": event["file"],
                    "line": event["line"],
                    "function": event["function"],
                    "local_names": event["local_names"],
                }
                for idx, event in enumerate(events[:200])
                if event.get("local_names")
            ],
        }
        with open(output_path, "w", encoding="utf-8") as handle:
            json.dump(payload, handle, sort_keys=True)
            handle.write("\n")
    return exit_code


if __name__ == "__main__":
    raise SystemExit(main())
'''


def _heredoc_script_lines(max_events: int) -> list[str]:
    return [
        f"cat > {TRACE_RUNNER_PATH} <<'CONTEXTGRAPH_TRACE_PY'",
        build_trace_runner_source().rstrip("\n"),
        "CONTEXTGRAPH_TRACE_PY",
        f"export CONTEXTGRAPH_TRACE_OUTPUT={shlex.quote(TRACE_OUTPUT_PATH)}",
        "export CONTEXTGRAPH_TRACE_REPO_ROOT=/testbed",
        f"export CONTEXTGRAPH_TRACE_MAX_EVENTS={int(max_events)}",
        f"rm -f {shlex.quote(TRACE_OUTPUT_PATH)}",
    ]


def _trace_emit_lines() -> list[str]:
    return [
        f"if [ -s {shlex.quote(TRACE_OUTPUT_PATH)} ]; then",
        f"  echo {TRACE_BEGIN}",
        f"  cat {shlex.quote(TRACE_OUTPUT_PATH)}",
        f"  echo {TRACE_END}",
        "else",
        "  echo CONTEXTGRAPH_EXECUTION_TRACE_MISSING",
        "fi",
    ]


def transform_eval_script_list(eval_script_list: list[str], max_events: int) -> tuple[list[str], dict[str, Any]]:
    pytest_indices: list[tuple[int, str]] = []
    for idx, command in enumerate(eval_script_list):
        stripped = command.strip()
        if stripped.startswith(f"{SUPPORTED_PYTEST_PREFIX} ") or stripped == SUPPORTED_PYTEST_PREFIX:
            pytest_indices.append((idx, command))
    require(
        len(pytest_indices) == 1,
        "EXECUTION_TRACE_SMOKE_UNSUPPORTED_TEST_COMMAND",
        {"pytest_command_count": len(pytest_indices)},
    )
    command_index, original_command = pytest_indices[0]
    parts = shlex.split(original_command)
    require(parts and parts[0] == SUPPORTED_PYTEST_PREFIX, "EXECUTION_TRACE_SMOKE_BAD_PYTEST_COMMAND", original_command)
    wrapped_command = shlex.join(["python", TRACE_RUNNER_PATH, *parts[1:]])
    transformed = list(eval_script_list)
    transformed[command_index] = wrapped_command
    heredoc_lines = _heredoc_script_lines(max_events)
    transformed[command_index:command_index] = heredoc_lines
    emit_index = command_index + len(heredoc_lines) + 2
    require(
        emit_index <= len(transformed),
        "EXECUTION_TRACE_SMOKE_END_MARKER_POSITION_UNSUPPORTED",
        {"pytest_command_index": command_index, "eval_script_len": len(eval_script_list)},
    )
    transformed[emit_index:emit_index] = _trace_emit_lines()
    return transformed, {
        "original_test_command": original_command,
        "wrapped_test_command": wrapped_command,
        "pytest_command_index": command_index,
        "max_events": int(max_events),
    }


def extract_trace_payload(test_output: str) -> dict[str, Any]:
    begin = test_output.find(TRACE_BEGIN)
    end = test_output.find(TRACE_END)
    require(begin >= 0 and end > begin, "EXECUTION_TRACE_SMOKE_TRACE_MARKERS_MISSING")
    body = test_output[begin + len(TRACE_BEGIN):end].strip()
    require(body, "EXECUTION_TRACE_SMOKE_TRACE_PAYLOAD_EMPTY")
    try:
        payload = json.loads(body)
    except json.JSONDecodeError as exc:
        raise ExecutionTraceSmokeError(f"EXECUTION_TRACE_SMOKE_TRACE_PAYLOAD_BAD_JSON: {exc}") from exc
    validate_trace_payload(payload)
    return payload


def validate_trace_payload(payload: Any) -> None:
    require(isinstance(payload, dict), "EXECUTION_TRACE_SMOKE_TRACE_PAYLOAD_NOT_OBJECT")
    for key in ("coverage", "line_coverage", "runtime_metrics", "steps", "trace_events"):
        require(key in payload, "EXECUTION_TRACE_SMOKE_TRACE_KEY_MISSING", key)
    require(isinstance(payload["trace_events"], list), "EXECUTION_TRACE_SMOKE_TRACE_EVENTS_NOT_ARRAY")
    require(len(payload["trace_events"]) > 0, "EXECUTION_TRACE_SMOKE_TRACE_EVENTS_EMPTY")
    coverage = payload["coverage"]
    require(isinstance(coverage, dict), "EXECUTION_TRACE_SMOKE_COVERAGE_NOT_OBJECT")
    require(int(coverage.get("lines_executed") or 0) > 0, "EXECUTION_TRACE_SMOKE_LINE_COVERAGE_EMPTY")


def trace_summary(payload: dict[str, Any]) -> dict[str, Any]:
    events = payload.get("trace_events") or []
    coverage = payload.get("coverage") or {}
    event_counts = Counter(str(event.get("event") or "") for event in events if isinstance(event, dict))
    return {
        "trace_events_recorded": len(events),
        "trace_event_types": dict(sorted(event_counts.items())),
        "lines_executed": int(coverage.get("lines_executed") or 0),
        "files_touched": int(coverage.get("files_touched") or 0),
        "events_seen": int(coverage.get("events_seen") or 0),
        "truncated": bool(coverage.get("truncated")),
    }


def import_swebench_runtime() -> dict[str, Any]:
    try:
        import docker
        from swebench.harness.constants import KEY_INSTANCE_ID, KEY_MODEL, KEY_PREDICTION, LOG_REPORT, LOG_TEST_OUTPUT, RUN_EVALUATION_LOG_DIR
        from swebench.harness.run_evaluation import run_instance
        from swebench.harness.test_spec.test_spec import make_test_spec
        from swebench.harness.utils import load_swebench_dataset
    except Exception as exc:
        raise ExecutionTraceSmokeError(f"EXECUTION_TRACE_SMOKE_SWEBENCH_IMPORT_FAILED: {type(exc).__name__}: {exc}") from exc
    return {
        "docker": docker,
        "KEY_INSTANCE_ID": KEY_INSTANCE_ID,
        "KEY_MODEL": KEY_MODEL,
        "KEY_PREDICTION": KEY_PREDICTION,
        "LOG_REPORT": LOG_REPORT,
        "LOG_TEST_OUTPUT": LOG_TEST_OUTPUT,
        "RUN_EVALUATION_LOG_DIR": RUN_EVALUATION_LOG_DIR,
        "run_instance": run_instance,
        "make_test_spec": make_test_spec,
        "load_swebench_dataset": load_swebench_dataset,
    }


def load_dataset_instance(runtime: dict[str, Any], dataset_name: str, split: str, task_id: str) -> dict[str, Any]:
    dataset = runtime["load_swebench_dataset"](dataset_name, split, [task_id])
    matches = [item for item in dataset if item.get(runtime["KEY_INSTANCE_ID"]) == task_id]
    require(
        len(matches) == 1,
        "EXECUTION_TRACE_SMOKE_DATASET_INSTANCE_MATCH_COUNT",
        {"task_id": task_id, "matches": len(matches)},
    )
    return dict(matches[0])


def run_trace_smoke(args: argparse.Namespace) -> dict[str, Any]:
    allow_non_aiwonder = bool(getattr(args, "allow_non_aiwonder_roots", False))
    require_archive_path(args.corpus_root, "corpus_root", allow_non_aiwonder)
    require_archive_path(args.output_root, "output_root", allow_non_aiwonder)
    require_archive_path(args.state_root, "state_root", allow_non_aiwonder)
    require_hot_path(args.venv_python, "venv_python", allow_non_aiwonder)
    require(args.venv_python.is_file(), "EXECUTION_TRACE_SMOKE_VENV_PYTHON_MISSING", {"venv_python": str(args.venv_python)})

    run_id = args.run_id or f"run-{utc_stamp()}"
    fsv_run_root = args.output_root / run_id / "trace-smoke"
    state_run_root = args.state_root / run_id / "trace-smoke"
    require(
        not fsv_run_root.exists() or not any(fsv_run_root.iterdir()),
        "EXECUTION_TRACE_SMOKE_OUTPUT_ROOT_NOT_EMPTY",
        {"fsv_run_root": str(fsv_run_root)},
    )
    require(
        not state_run_root.exists() or not any(state_run_root.iterdir()),
        "EXECUTION_TRACE_SMOKE_STATE_ROOT_NOT_EMPTY",
        {"state_run_root": str(state_run_root)},
    )
    fsv_run_root.mkdir(parents=True, exist_ok=True)
    state_run_root.mkdir(parents=True, exist_ok=True)

    corpus_entry = load_corpus_entry(args.corpus_root, args.task_id, args.category)
    patch_path = Path(corpus_entry["patch_abspath"])
    patch_text = patch_path.read_text(encoding="utf-8")
    runtime = import_swebench_runtime()
    dataset_instance = load_dataset_instance(runtime, args.dataset_name, args.split, args.task_id)
    test_spec = runtime["make_test_spec"](
        dataset_instance,
        namespace=args.namespace,
        instance_image_tag=args.tag,
        env_image_tag=args.tag,
        arch=args.arch,
    )
    transformed, transform_metadata = transform_eval_script_list(
        list(test_spec.eval_script_list),
        max_events=args.max_events,
    )
    test_spec.eval_script_list = transformed

    previous_cwd = Path.cwd()
    os.chdir(fsv_run_root)
    try:
        client = runtime["docker"].from_env()
        prediction = {
            runtime["KEY_INSTANCE_ID"]: args.task_id,
            runtime["KEY_MODEL"]: "contextgraph-execution-trace-smoke",
            runtime["KEY_PREDICTION"]: patch_text,
        }
        result = runtime["run_instance"](
            test_spec,
            prediction,
            False,
            False,
            client,
            run_id,
            args.instance_timeout,
            False,
        )
    finally:
        os.chdir(previous_cwd)

    model_name = "contextgraph-execution-trace-smoke"
    log_dir = fsv_run_root / runtime["RUN_EVALUATION_LOG_DIR"] / run_id / model_name / args.task_id
    test_output_path = log_dir / runtime["LOG_TEST_OUTPUT"]
    report_path = log_dir / runtime["LOG_REPORT"]
    eval_script_path = log_dir / "eval.sh"
    require(test_output_path.is_file(), "EXECUTION_TRACE_SMOKE_TEST_OUTPUT_MISSING", {"test_output": str(test_output_path)})
    require(report_path.is_file(), "EXECUTION_TRACE_SMOKE_REPORT_MISSING", {"report": str(report_path)})
    test_output = test_output_path.read_text(encoding="utf-8", errors="replace")
    trace_payload = extract_trace_payload(test_output)
    trace_payload.update(
        {
            "task_id": args.task_id,
            "category": args.category,
            "candidate_workspace_state": args.category,
            "dataset_name": args.dataset_name,
            "split": args.split,
        }
    )
    trace_rel = Path(args.task_id) / f"{args.category}.execution-trace.json"
    state_trace_path = state_run_root / trace_rel
    fsv_trace_path = fsv_run_root / "extracted-traces" / trace_rel
    write_json(state_trace_path, trace_payload)
    write_json(fsv_trace_path, trace_payload)

    report_doc = read_json(report_path, "EXECUTION_TRACE_SMOKE_REPORT_BAD_JSON")
    manifest = {
        "schema_version": SCHEMA_VERSION,
        "artifact_kind": ARTIFACT_KIND,
        "task_id": TASK_ID,
        "generated_at": utc_now(),
        "run_id": run_id,
        "inputs": {
            "dataset_name": args.dataset_name,
            "split": args.split,
            "corpus_root": str(args.corpus_root),
            "task_id": args.task_id,
            "category": args.category,
            "patch_path": str(patch_path),
            "patch_sha256": corpus_entry["patch_sha256"],
        },
        "execution": {
            "fsv_run_root": str(fsv_run_root),
            "state_run_root": str(state_run_root),
            "namespace": args.namespace,
            "arch": args.arch,
            "tag": args.tag,
            "instance_timeout": args.instance_timeout,
            "run_instance_result": result,
            "log_dir": str(log_dir),
            "test_output": str(test_output_path),
            "report": str(report_path),
            "eval_script": str(eval_script_path),
            "transform": transform_metadata,
        },
        "trace_artifacts": {
            "state_trace": str(state_trace_path),
            "fsv_trace": str(fsv_trace_path),
            "state_trace_sha256": sha256_file(state_trace_path),
            "fsv_trace_sha256": sha256_file(fsv_trace_path),
        },
        "trace_summary": trace_summary(trace_payload),
        "oracle_report_summary": {
            "resolved": bool((report_doc.get(args.task_id) or {}).get("resolved")),
            "tests_status_present": "tests_status" in (report_doc.get(args.task_id) or {}),
        },
        "decision": {
            "execution_trace_smoke_passed": True,
            "full_corpus_trace_generation_complete": False,
            "promotion_allowed": False,
            "next_action": "scale this pytest trace path to all supported pytest rows, then define separate handlers for non-pytest SWE-bench commands",
            "decision_reasons": [
                "one_real_pytest_row_has_live_sys_settrace_payload",
                "full_300x8_trace_coverage_not_yet_generated",
                "execution_trace_not_yet_compared_for_per_cell_lift",
            ],
        },
        "doctrine_policy": {
            "measurement_only": True,
            "target_labels_used_as_live_inputs": False,
            "oracle_verdicts_classified_target_side_only": True,
            "flat_vector_concat_used": False,
            "slot_identity_preserved": True,
            "predictor_weights_changed": False,
            "inner_llm_used": False,
            "aiwonder_durable_root_required": not allow_non_aiwonder,
        },
    }
    manifest_path = fsv_run_root / "execution_trace_smoke_manifest.json"
    state_manifest_path = state_run_root / "execution_trace_smoke_manifest.json"
    write_json(manifest_path, manifest)
    write_json(state_manifest_path, manifest)
    manifest["manifest_sha256"] = sha256_file(manifest_path)
    manifest["state_manifest_sha256"] = sha256_file(state_manifest_path)
    write_json(manifest_path, manifest)
    write_json(state_manifest_path, manifest)

    return {
        "ok": True,
        "manifest": str(manifest_path),
        "state_manifest": str(state_manifest_path),
        "state_trace": str(state_trace_path),
        "fsv_trace": str(fsv_trace_path),
        "trace_summary": manifest["trace_summary"],
        "decision": manifest["decision"],
    }


def add_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--dataset-name", default=DEFAULT_DATASET)
    parser.add_argument("--split", default=DEFAULT_SPLIT)
    parser.add_argument("--corpus-root", type=Path, default=DEFAULT_CORPUS_ROOT)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--state-root", type=Path, default=DEFAULT_STATE_ROOT)
    parser.add_argument("--venv-python", type=Path, default=DEFAULT_VENV_PYTHON)
    parser.add_argument("--task-id", default=DEFAULT_TASK_ID)
    parser.add_argument("--category", default=DEFAULT_CATEGORY)
    parser.add_argument("--run-id")
    parser.add_argument("--namespace", default="swebench")
    parser.add_argument("--arch", default="x86_64")
    parser.add_argument("--tag", default="latest")
    parser.add_argument("--instance-timeout", type=int, default=1800)
    parser.add_argument("--max-events", type=int, default=5000)
    parser.add_argument("--allow-non-aiwonder-roots", action="store_true")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    add_arguments(parser)
    args = parser.parse_args(argv)
    try:
        result = run_trace_smoke(args)
    except ExecutionTraceSmokeError as exc:
        print(str(exc), flush=True)
        return 2
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
