#!/usr/bin/env python3
"""Run official SWE-bench evaluation and persist the readback as evidence.

This script is the bridge between ContextGraph's local refinement loop and the
official Docker-based SWE-bench harness. It does not treat a local validation
result as final. It runs the official harness, reads the official report/logs
from disk, and appends the official outcome into each task/model BlackBoxFix
ledger as a source-of-truth verifier result.
"""
from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import textwrap
import time
from pathlib import Path
from typing import Any

from swe_lite_refinement_loop import (
    DATASET_NAME,
    DEFAULT_CONTEXT_GRAPH_BIN,
    RefinementError,
    ProcessResult,
    file_sot,
    ledger_counts,
    now_ns,
    run_blackbox,
    score_and_reward,
    sha256_file,
    sqlite_sot,
    write_json,
    write_text_checked,
)


DEFAULT_SWEBENCH_PYTHON = Path("/home/cabdru/.cache/contextgraph-swebench/venv/bin/python")
DEFAULT_DOCKER_CLI_DIR = Path("/mnt/wsl/docker-desktop/cli-tools/usr/bin")
DEFAULT_DOCKER_SOCKET = Path("/mnt/wsl/docker-desktop-bind-mounts/Ubuntu-24.04/docker.sock")
DEFAULT_DOCKER_WINDOWS_BIN = Path("/mnt/c/Program Files/Docker/Docker/resources/bin")


def safe_model_name(model: str) -> str:
    return model.replace(":", "-").replace("/", "-")


def run_process(
    command: list[str],
    *,
    cwd: Path,
    timeout: float,
    env: dict[str, str] | None = None,
) -> ProcessResult:
    started = time.perf_counter()
    try:
        output = subprocess.run(
            command,
            cwd=str(cwd),
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=timeout,
            check=False,
            env=env,
        )
        exit_code = output.returncode
        stdout = output.stdout
        stderr = output.stderr
    except subprocess.TimeoutExpired as err:
        exit_code = 124
        stdout = err.stdout if isinstance(err.stdout, str) else ""
        stderr = (err.stderr if isinstance(err.stderr, str) else "") + f"\nTIMEOUT after {timeout}s"
    elapsed_ms = int((time.perf_counter() - started) * 1000)
    return ProcessResult(command, str(cwd), exit_code, stdout, stderr, elapsed_ms)


def docker_desktop_env(
    *,
    docker_cli_dir: Path,
    docker_socket: Path,
    docker_windows_bin: Path,
) -> dict[str, str]:
    docker_bin = docker_cli_dir / "docker"
    if not docker_bin.exists():
        raise RefinementError(
            "SWE_OFFICIAL_DOCKER_DESKTOP_CLI_MISSING",
            "Docker Desktop CLI was not found inside WSL",
            "docker_cli_dir",
            "start Docker Desktop for Windows and verify WSL integration for this distro",
            details={"docker_bin": str(docker_bin)},
            source_of_truth=file_sot(docker_bin),
        )
    if not docker_socket.exists():
        raise RefinementError(
            "SWE_OFFICIAL_DOCKER_DESKTOP_SOCKET_MISSING",
            "Docker Desktop socket was not found inside WSL",
            "docker_socket",
            "start Docker Desktop for Windows and verify WSL integration for this distro",
            details={"docker_socket": str(docker_socket)},
            source_of_truth=file_sot(docker_socket),
        )
    path_parts = [str(docker_cli_dir)]
    if docker_windows_bin.exists():
        path_parts.append(str(docker_windows_bin))
    path_parts.append(os.environ.get("PATH", ""))
    env = dict(os.environ)
    env["PATH"] = os.pathsep.join(path_parts)
    env["DOCKER_HOST"] = f"unix://{docker_socket}"
    return env


def verify_docker_desktop(env: dict[str, str], docker_cli_dir: Path, cwd: Path) -> dict[str, Any]:
    docker = docker_cli_dir / "docker"
    version = run_process(
        [str(docker), "version", "--format", "{{json .Server}}"],
        cwd=cwd,
        timeout=60,
        env=env,
    )
    if version.exit_code != 0:
        raise RefinementError(
            "SWE_OFFICIAL_DOCKER_DESKTOP_UNAVAILABLE",
            "Docker Desktop did not answer `docker version` from WSL",
            "docker.version",
            "start Docker Desktop and rerun after WSL integration is healthy",
            details={"process": version.to_json()},
        )
    try:
        server = json.loads(version.stdout)
    except json.JSONDecodeError as err:
        raise RefinementError(
            "SWE_OFFICIAL_DOCKER_VERSION_JSON_INVALID",
            "Docker Desktop version output was not JSON",
            "docker.version.stdout",
            "inspect Docker Desktop CLI output before running SWE-bench",
            details={"stdout": version.stdout, "stderr": version.stderr},
        ) from err
    return {
        "status": "ok",
        "server": server,
        "docker_host": env["DOCKER_HOST"],
        "docker_cli": str(docker),
    }


def read_predictions(path: Path) -> list[dict[str, Any]]:
    records = []
    for line_no, line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
        if not line.strip():
            continue
        try:
            record = json.loads(line)
        except json.JSONDecodeError as err:
            raise RefinementError(
                "SWE_OFFICIAL_PREDICTION_JSON_INVALID",
                "prediction JSONL contained invalid JSON",
                f"predictions.line_{line_no}",
                "rewrite predictions from the refinement loop before official evaluation",
                details={"line_preview": line[:500]},
                source_of_truth=file_sot(path),
            ) from err
        for field in ("instance_id", "model_name_or_path", "model_patch"):
            if field not in record:
                raise RefinementError(
                    "SWE_OFFICIAL_PREDICTION_FIELD_MISSING",
                    "prediction JSONL row was missing a required SWE-bench field",
                    f"predictions.line_{line_no}.{field}",
                    "rewrite predictions from the refinement loop before official evaluation",
                    details={"record": record},
                    source_of_truth=file_sot(path),
                )
        records.append(record)
    if not records:
        raise RefinementError(
            "SWE_OFFICIAL_PREDICTIONS_EMPTY",
            "prediction JSONL did not contain any model patches",
            "predictions",
            "run the refinement loop until at least one candidate patch exists",
            source_of_truth=file_sot(path),
        )
    return records


def load_loop_summary(run_root: Path) -> dict[str, Any]:
    path = run_root / "refinement-loop-summary.json"
    if not path.exists():
        raise RefinementError(
            "SWE_OFFICIAL_LOOP_SUMMARY_MISSING",
            "refinement-loop-summary.json was not found",
            "run_root.refinement_loop_summary",
            "run gemmafrontend/swe_lite_refinement_loop.py before official evaluation",
            source_of_truth=file_sot(path),
        )
    return json.loads(path.read_text(encoding="utf-8"))


def run_official_eval(
    *,
    swebench_python: Path,
    predictions_path: Path,
    instance_ids: list[str],
    run_id: str,
    official_dir: Path,
    env: dict[str, str],
    max_workers: int,
    timeout: int,
    cache_level: str,
    clean: bool,
) -> dict[str, Any]:
    if not swebench_python.exists():
        raise RefinementError(
            "SWE_OFFICIAL_SWEBENCH_PYTHON_MISSING",
            "SWE-bench Python executable was not found",
            "swebench_python",
            "install the SWE-bench harness venv before running official evaluation",
            source_of_truth=file_sot(swebench_python),
        )
    command = [
        str(swebench_python),
        "-m",
        "swebench.harness.run_evaluation",
        "--dataset_name",
        DATASET_NAME,
        "--split",
        "test",
        "--predictions_path",
        str(predictions_path),
        "--max_workers",
        str(max_workers),
        "--timeout",
        str(timeout),
        "--cache_level",
        cache_level,
        "--clean",
        "true" if clean else "false",
        "--run_id",
        run_id,
        "--report_dir",
        str(official_dir),
    ]
    if instance_ids:
        command.extend(["--instance_ids", *instance_ids])
    result = run_process(command, cwd=official_dir, timeout=max(timeout + 900, 1800), env=env)
    write_json(
        official_dir / "official-command.json",
        {
            "status": "ok" if result.exit_code == 0 else "error",
            "process": result.to_json(),
            "source_of_truth": file_sot(official_dir / "official-command.json"),
        },
    )
    if result.exit_code != 0:
        raise RefinementError(
            "SWE_OFFICIAL_EVALUATION_COMMAND_FAILED",
            "official SWE-bench evaluation command failed",
            "swebench.harness.run_evaluation",
            "inspect official-command.json and Docker logs before retrying",
            details={"process": result.to_json()},
            source_of_truth=file_sot(official_dir / "official-command.json"),
        )
    return result.to_json()


def find_final_report(official_dir: Path, run_id: str) -> Path:
    reports = sorted(path for path in official_dir.glob(f"*.{run_id}.json") if path.is_file())
    if len(reports) != 1:
        raise RefinementError(
            "SWE_OFFICIAL_REPORT_NOT_UNIQUE",
            "official SWE-bench final report was not found exactly once",
            "official_report",
            "inspect official evaluation output directory before ingesting evidence",
            details={"report_candidates": [str(path) for path in reports]},
            source_of_truth=file_sot(official_dir),
        )
    return reports[0]


def read_json_file(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def relative_to_run_root(run_root: Path, path: Path) -> str:
    return str(path.resolve().relative_to(run_root.resolve()))


def instance_log_paths(official_dir: Path, run_id: str, model: str, instance_id: str) -> dict[str, str | None]:
    base = official_dir / "logs" / "run_evaluation" / run_id / model / instance_id
    paths = {
        "run_instance_log": base / "run_instance.log",
        "test_output": base / "test_output.txt",
        "instance_report": base / "report.json",
        "patch": base / "patch.diff",
        "eval_script": base / "eval.sh",
    }
    return {key: str(path) if path.exists() else None for key, path in paths.items()}


def write_official_verifier(official_dir: Path) -> Path:
    verifier = official_dir / "verify_official_swebench_result.py"
    write_text_checked(
        verifier,
        textwrap.dedent(
            """\
            #!/usr/bin/env python3
            import argparse
            import json
            from pathlib import Path

            parser = argparse.ArgumentParser()
            parser.add_argument("--report", required=True)
            parser.add_argument("--instance-id", required=True)
            args = parser.parse_args()

            report_path = Path(args.report)
            report = json.loads(report_path.read_text(encoding="utf-8"))
            instance_id = args.instance_id
            resolved = instance_id in set(report.get("resolved_ids") or [])
            unresolved = instance_id in set(report.get("unresolved_ids") or [])
            errored = instance_id in set(report.get("error_ids") or [])
            if not (resolved or unresolved or errored):
                print(json.dumps({
                    "status": "error",
                    "error_code": "SWE_OFFICIAL_INSTANCE_RESULT_MISSING",
                    "field_path": "official_report.instance_id",
                    "message": "instance id was not present in resolved, unresolved, or error ids",
                    "instance_id": instance_id,
                    "source_of_truth": f"file:{report_path}",
                }, sort_keys=True))
                raise SystemExit(43)
            status = "passed" if resolved else "failed"
            print(json.dumps({
                "status": status,
                "resolved": resolved,
                "unresolved": unresolved,
                "errored": errored,
                "instance_id": instance_id,
                "source_of_truth": f"file:{report_path}",
            }, sort_keys=True))
            raise SystemExit(0 if resolved else 42)
            """
        ),
    )
    verifier.chmod(0o755)
    return verifier


def persist_official_readback(
    *,
    run_root: Path,
    official_dir: Path,
    report_path: Path,
    report: dict[str, Any],
    prediction: dict[str, Any],
    context_graph_bin: Path,
    run_id: str,
) -> dict[str, Any]:
    model = prediction["model_name_or_path"]
    instance_id = prediction["instance_id"]
    safe_model = safe_model_name(model)
    task_root = run_root / "tasks" / instance_id / safe_model
    ledger = task_root / "blackbox.sqlite"
    loop_summary_path = task_root / "loop-summary.json"
    if not ledger.exists() or not loop_summary_path.exists():
        raise RefinementError(
            "SWE_OFFICIAL_TASK_LEDGER_MISSING",
            "could not find the task/model BlackBoxFix ledger for official evidence",
            "tasks.instance.model.blackbox.sqlite",
            "run the refinement loop for this model and instance before official evaluation",
            details={"instance_id": instance_id, "model": model, "ledger": str(ledger)},
            source_of_truth=sqlite_sot(ledger),
        )
    loop_summary = read_json_file(loop_summary_path)
    blackbox_run_id = loop_summary["blackbox_run_id"]
    resolved = instance_id in set(report.get("resolved_ids") or [])
    official_status = "passed" if resolved else "failed"
    readback_dir = task_root / "official-eval" / run_id
    readback_dir.mkdir(parents=True, exist_ok=True)
    log_paths = instance_log_paths(official_dir, run_id, model, instance_id)
    readback = {
        "status": official_status,
        "run_id": run_id,
        "model": model,
        "instance_id": instance_id,
        "resolved": resolved,
        "unresolved": instance_id in set(report.get("unresolved_ids") or []),
        "errored": instance_id in set(report.get("error_ids") or []),
        "official_report": {
            "path": str(report_path),
            "sha256": sha256_file(report_path),
            "source_of_truth": file_sot(report_path),
        },
        "logs": {
            key: {
                "path": value,
                "sha256": sha256_file(Path(value)) if value else None,
                "source_of_truth": file_sot(Path(value)) if value else None,
            }
            for key, value in log_paths.items()
        },
        "source_of_truth": file_sot(report_path),
    }
    readback_path = readback_dir / "official-readback.json"
    write_json(readback_path, readback)
    tool_input_path = readback_dir / "official-tool-call-input.json"
    write_json(
        tool_input_path,
        {
            "tool": "swebench_official_docker_eval",
            "expected_delta": {
                "official_report": "instance resolved",
                "official_docker_harness": "completed",
            },
            "actual_official_status": official_status,
            "official_readback_sha256": sha256_file(readback_path),
            "official_report_sha256": sha256_file(report_path),
            "source_of_truth": file_sot(report_path),
        },
    )
    tool_call = run_blackbox(
        context_graph_bin,
        [
            "blackbox",
            "record",
            "tool-call",
            "--ledger",
            str(ledger),
            "--run-id",
            blackbox_run_id,
            "--tool",
            "swebench_official_docker_eval",
            "--capability",
            "contextgraph.swe_lite.official_docker_eval",
            "--input",
            str(tool_input_path),
            "--status",
            "ok" if official_status == "passed" else "failed",
            "--agent",
            "contextgraph_swe_lite_refiner",
            "--model",
            model,
        ],
        cwd=task_root,
    )
    verifier = write_official_verifier(official_dir)
    artifacts = [
        relative_to_run_root(run_root, report_path),
        relative_to_run_root(run_root, readback_path),
        relative_to_run_root(run_root, tool_input_path),
        relative_to_run_root(run_root, verifier),
    ]
    for value in log_paths.values():
        if value:
            artifacts.append(relative_to_run_root(run_root, Path(value)))
    spec = {
        "verifier_id": "cmd_exit_zero_v1",
        "run_id": blackbox_run_id,
        "tool_call_id": tool_call["tool_call_id"],
        "source": {"cwd": str(run_root)},
        "expected": {
            "command": [
                sys.executable,
                relative_to_run_root(run_root, verifier),
                "--report",
                relative_to_run_root(run_root, report_path),
                "--instance-id",
                instance_id,
            ],
            "artifacts": artifacts,
        },
    }
    spec_path = readback_dir / "official-blackbox-verify-spec.json"
    write_json(spec_path, spec)
    verifier_result = run_blackbox(
        context_graph_bin,
        ["blackbox", "verify", "--ledger", str(ledger), "--input", str(spec_path), "--model", model],
        cwd=task_root,
        allow_error=True,
    )
    verifier_error_code = verifier_result.get("error_code")
    if verifier_error_code and verifier_error_code != "CMD_EXIT_NON_ZERO":
        raise RefinementError(
            "SWE_OFFICIAL_BLACKBOX_VERIFIER_CONFIG_INVALID",
            "official SWE-bench verifier failed before evaluating the official report",
            "blackbox.verify",
            "inspect verifier registration and the generated official-blackbox-verify-spec.json",
            details={"verifier": verifier_result, "spec_path": str(spec_path)},
            source_of_truth=file_sot(spec_path),
        )
    if official_status == "passed" and verifier_result.get("status") == "error":
        raise RefinementError(
            "SWE_OFFICIAL_PASSED_BUT_VERIFIER_FAILED",
            "official report resolved the instance but BlackBoxFix verifier failed",
            "official_report.resolved_ids",
            "inspect the official report and verifier script before trusting the run",
            details={"verifier": verifier_result, "spec_path": str(spec_path)},
            source_of_truth=file_sot(report_path),
        )
    score_reward = score_and_reward(context_graph_bin, ledger, blackbox_run_id, model, task_root)
    return {
        "model": model,
        "instance_id": instance_id,
        "official_status": official_status,
        "resolved": resolved,
        "readback_path": str(readback_path),
        "readback_sha256": sha256_file(readback_path),
        "tool_call": tool_call,
        "verifier": verifier_result,
        "score_reward": score_reward,
        "ledger": {
            "path": str(ledger),
            "sha256": sha256_file(ledger),
            "counts": ledger_counts(ledger),
            "source_of_truth": sqlite_sot(ledger),
        },
    }


def build_summary(
    *,
    run_root: Path,
    official_dir: Path,
    predictions_path: Path,
    predictions: list[dict[str, Any]],
    run_id: str,
    docker: dict[str, Any],
    command_result: dict[str, Any],
    report_path: Path,
    report: dict[str, Any],
    ledger_ingests: list[dict[str, Any]],
) -> dict[str, Any]:
    resolved_ids = report.get("resolved_ids") or []
    unresolved_ids = report.get("unresolved_ids") or []
    error_ids = report.get("error_ids") or []
    return {
        "status": "ok",
        "schema_version": 1,
        "created_at_unix": time.time(),
        "dataset": DATASET_NAME,
        "run_id": run_id,
        "run_root": str(run_root),
        "official_dir": str(official_dir),
        "predictions": {
            "path": str(predictions_path),
            "sha256": sha256_file(predictions_path),
            "records": len(predictions),
            "source_of_truth": file_sot(predictions_path),
        },
        "docker_desktop": docker,
        "official_command": command_result,
        "official_report": {
            "path": str(report_path),
            "sha256": sha256_file(report_path),
            "source_of_truth": file_sot(report_path),
            "total_instances": report.get("total_instances"),
            "submitted_instances": report.get("submitted_instances"),
            "completed_instances": report.get("completed_instances"),
            "resolved_instances": report.get("resolved_instances"),
            "unresolved_instances": report.get("unresolved_instances"),
            "error_instances": report.get("error_instances"),
            "resolved_ids": resolved_ids,
            "unresolved_ids": unresolved_ids,
            "error_ids": error_ids,
        },
        "ledger_ingests": ledger_ingests,
        "source_of_truth": file_sot(official_dir / "official-eval-summary.json"),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run-root", type=Path, required=True)
    parser.add_argument("--predictions-path", type=Path)
    parser.add_argument("--swebench-python", type=Path, default=DEFAULT_SWEBENCH_PYTHON)
    parser.add_argument("--context-graph-bin", type=Path, default=DEFAULT_CONTEXT_GRAPH_BIN)
    parser.add_argument("--docker-cli-dir", type=Path, default=DEFAULT_DOCKER_CLI_DIR)
    parser.add_argument("--docker-socket", type=Path, default=DEFAULT_DOCKER_SOCKET)
    parser.add_argument("--docker-windows-bin", type=Path, default=DEFAULT_DOCKER_WINDOWS_BIN)
    parser.add_argument("--instance-id", action="append", dest="instance_ids", default=None)
    parser.add_argument("--run-id")
    parser.add_argument("--max-workers", type=int, default=1)
    parser.add_argument("--timeout", type=int, default=900)
    parser.add_argument("--cache-level", default="env")
    parser.add_argument("--clean", action="store_true")
    args = parser.parse_args()

    run_root = args.run_root.resolve()
    predictions_path = (args.predictions_path or run_root / "predictions" / "latest-predictions.jsonl").resolve()
    run_id = args.run_id or f"contextgraph_official_{run_root.name}_{now_ns()}"
    official_dir = run_root / "official-eval" / run_id
    official_dir.mkdir(parents=True, exist_ok=False)

    try:
        if not run_root.exists():
            raise RefinementError(
                "SWE_OFFICIAL_RUN_ROOT_MISSING",
                "run root does not exist",
                "run_root",
                "run the refinement loop before official evaluation",
                source_of_truth=file_sot(run_root),
            )
        if not predictions_path.exists():
            raise RefinementError(
                "SWE_OFFICIAL_PREDICTIONS_MISSING",
                "predictions file does not exist",
                "predictions_path",
                "run the refinement loop until it exports predictions/latest-predictions.jsonl",
                source_of_truth=file_sot(predictions_path),
            )
        load_loop_summary(run_root)
        predictions = read_predictions(predictions_path)
        instance_ids = args.instance_ids or sorted({str(record["instance_id"]) for record in predictions})
        env = docker_desktop_env(
            docker_cli_dir=args.docker_cli_dir,
            docker_socket=args.docker_socket,
            docker_windows_bin=args.docker_windows_bin,
        )
        docker = verify_docker_desktop(env, args.docker_cli_dir, official_dir)
        command_result = run_official_eval(
            swebench_python=args.swebench_python,
            predictions_path=predictions_path,
            instance_ids=instance_ids,
            run_id=run_id,
            official_dir=official_dir,
            env=env,
            max_workers=args.max_workers,
            timeout=args.timeout,
            cache_level=args.cache_level,
            clean=args.clean,
        )
        report_path = find_final_report(official_dir, run_id)
        report = read_json_file(report_path)
        ledger_ingests = [
            persist_official_readback(
                run_root=run_root,
                official_dir=official_dir,
                report_path=report_path,
                report=report,
                prediction=prediction,
                context_graph_bin=args.context_graph_bin.resolve(),
                run_id=run_id,
            )
            for prediction in predictions
            if prediction["instance_id"] in instance_ids
        ]
        summary = build_summary(
            run_root=run_root,
            official_dir=official_dir,
            predictions_path=predictions_path,
            predictions=predictions,
            run_id=run_id,
            docker=docker,
            command_result=command_result,
            report_path=report_path,
            report=report,
            ledger_ingests=ledger_ingests,
        )
        summary_path = official_dir / "official-eval-summary.json"
        write_json(summary_path, summary)
        readback = json.loads(summary_path.read_text(encoding="utf-8"))
        if readback != summary:
            raise RefinementError(
                "SWE_OFFICIAL_SUMMARY_READBACK_MISMATCH",
                "official eval summary readback did not match persisted payload",
                "official_eval_summary",
                "inspect filesystem durability before using official evidence",
                source_of_truth=file_sot(summary_path),
            )
        print("SWE_OFFICIAL_EVAL_SUMMARY=" + str(summary_path), flush=True)
        return 0
    except RefinementError as err:
        error = err.to_json()
        error_path = official_dir / "official-eval-error.json"
        write_json(error_path, error)
        print("SWE_OFFICIAL_EVAL_ERROR=" + str(error_path), flush=True)
        print(json.dumps(error, indent=2, sort_keys=True), flush=True)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
