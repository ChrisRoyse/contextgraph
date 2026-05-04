#!/usr/bin/env python3
"""Run a real 10-prompt x model matrix through the ContextGraph frontend.

The matrix verifies model tool choice, persisted frontend snapshots, and direct
SQLite ledger readback. It does not score prose answers; it scores verified
tool execution and durable evidence.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import sqlite3
import time
import urllib.error
import urllib.request
from pathlib import Path
from typing import Any


DEFAULT_MODELS = ["gemma4:e4b", "qwen3:8b", "mistral-nemo:12b"]
MAX_PROMPT_CHARS = 4096

PROMPTS: list[dict[str, Any]] = [
    {
        "id": "run_full_proof",
        "expected_tool": "run_proof_pipeline",
        "message": (
            "Run the full ContextGraph proof pipeline now. Use scenario counter_world_full_agentops "
            "and include every required capability exactly once: data_ingest, panel_materialization, "
            "action_conditioned_training, planning, surprise_recording, artifact_provenance, blackboxfix_fsv."
        ),
    },
    {
        "id": "inspect_latest_evidence",
        "expected_tool": "inspect_latest_run",
        "message": "Inspect the latest completed run and tell me what the physical SQLite ledger and files prove.",
    },
    {
        "id": "compile_reward",
        "expected_tool": "compile_reward_for_run",
        "message": "Compile or read back the reward/loss sample for the latest real run using the persisted scorecard.",
    },
    {
        "id": "explain_scorecard",
        "expected_tool": "explain_scorecard",
        "message": "Explain the latest scorecard from the SQLite ledger: trust, risk, correctness, and verification confidence.",
    },
    {
        "id": "analyze_incident",
        "expected_tool": "analyze_incident",
        "message": "Analyze why the latest run has an incident. Use verifier rows and incident packets only.",
    },
    {
        "id": "reward_improvements",
        "expected_tool": "suggest_reward_improvements",
        "message": "Read the latest reward components and identify the concrete evidence-backed changes that would improve reward.",
    },
    {
        "id": "compare_runs",
        "expected_tool": "compare_recent_runs",
        "message": "Compare the two most recent completed runs and tell me which is safer based on real evidence.",
    },
    {
        "id": "export_buyer_report",
        "expected_tool": "export_buyer_report",
        "message": "Export a buyer-facing evidence report for an investor and read it back from disk.",
    },
    {
        "id": "evaluate_policy_observe",
        "expected_tool": "evaluate_tool_policy",
        "expected_policy_decision": "allowed",
        "message": "Evaluate observe_only policy for the proof tool and read the policy_decision row back from SQLite.",
    },
    {
        "id": "build_category_pack",
        "expected_tool": "build_investor_category_pack",
        "message": "Build the investor product category pack from real completed runs with max_runs set to 8.",
    },
]


class MatrixError(RuntimeError):
    def __init__(self, error_code: str, message: str, details: dict[str, Any] | None = None) -> None:
        super().__init__(message)
        self.error_code = error_code
        self.message = message
        self.details = details or {}

    def to_json(self) -> dict[str, Any]:
        return {
            "status": "error",
            "error_code": self.error_code,
            "message": self.message,
            "details": self.details,
        }


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return "sha256:" + digest.hexdigest()


def request_json(
    url: str,
    body: dict[str, Any] | None = None,
    *,
    timeout: float = 2400.0,
    allow_error: bool = False,
) -> tuple[int, dict[str, Any]]:
    data = None if body is None else json.dumps(body).encode("utf-8")
    request = urllib.request.Request(
        url,
        data=data,
        headers={"Content-Type": "application/json", "Accept": "application/json"},
        method="GET" if body is None else "POST",
    )
    try:
        with urllib.request.urlopen(request, timeout=timeout) as response:
            raw = response.read()
            status = response.status
    except urllib.error.HTTPError as err:
        raw = err.read()
        status = err.code
    value = json.loads(raw.decode("utf-8"))
    if not allow_error and (status >= 400 or value.get("status") == "error"):
        raise MatrixError(
            str(value.get("error_code") or "PROMPT_MATRIX_HTTP_ERROR"),
            str(value.get("message") or "matrix API request failed"),
            {"url": url, "status": status, "response": value},
        )
    return status, value


def direct_ledger_counts(ledger: Path) -> dict[str, int]:
    conn = sqlite3.connect(ledger)
    try:
        return {
            str(row[0]): int(row[1])
            for row in conn.execute(
                "SELECT record_kind, COUNT(*) FROM ledger_records GROUP BY record_kind ORDER BY record_kind"
            )
        }
    finally:
        conn.close()


def latest_policy_status(ledger: Path, run_id: str) -> dict[str, Any] | None:
    conn = sqlite3.connect(ledger)
    try:
        row = conn.execute(
            "SELECT sequence, payload_json, payload_sha256, record_sha256 "
            "FROM ledger_records WHERE run_id=? AND record_kind='policy_decision' "
            "ORDER BY sequence DESC LIMIT 1",
            (run_id,),
        ).fetchone()
    finally:
        conn.close()
    if row is None:
        return None
    payload = json.loads(row[1])
    return {
        "sequence": int(row[0]),
        "status": payload.get("status"),
        "error_code": payload.get("error_code"),
        "payload_sha256": row[2],
        "record_sha256": row[3],
        "payload": payload,
    }


def require_models(base_url: str, models: list[str]) -> dict[str, Any]:
    _, health = request_json(base_url + "/api/health", timeout=30.0)
    ollama_url = str(health.get("ollama_url")).rstrip("/")
    _, tags = request_json(ollama_url + "/api/tags", timeout=30.0)
    installed = {item.get("name"): item for item in tags.get("models", []) if isinstance(item, dict)}
    missing = [model for model in models if model not in installed]
    if missing:
        raise MatrixError(
            "PROMPT_MATRIX_MODEL_MISSING",
            "one or more required Ollama models are not installed",
            {"missing": missing, "installed": sorted(installed)},
        )
    return {
        "health": health,
        "ollama_url": ollama_url,
        "installed_models": {model: installed[model] for model in models},
    }


def verify_snapshot(snapshot: dict[str, Any], test: dict[str, Any], model: str) -> dict[str, Any]:
    agent = snapshot.get("chat_agent") or {}
    selected = (agent.get("selected_tool") or {}).get("name")
    expected = test["expected_tool"]
    identity = snapshot.get("identity") or {}
    ledger = snapshot.get("ledger") or {}
    ledger_path = Path(str(ledger.get("path")))
    snapshot_path = Path(str(snapshot.get("frontend_persistence", {}).get("snapshot_path")))
    snapshot_readback = json.loads(snapshot_path.read_text(encoding="utf-8"))
    direct_counts = direct_ledger_counts(ledger_path)
    if direct_counts != ledger.get("physical_counts"):
        raise MatrixError(
            "PROMPT_MATRIX_LEDGER_COUNT_MISMATCH",
            "snapshot ledger counts did not match direct SQLite readback",
            {"model": model, "prompt_id": test["id"], "snapshot": ledger.get("physical_counts"), "direct": direct_counts},
        )
    if snapshot_readback.get("identity", {}).get("blackbox_run_id") != identity.get("blackbox_run_id"):
        raise MatrixError(
            "PROMPT_MATRIX_SNAPSHOT_READBACK_MISMATCH",
            "persisted snapshot readback did not match response run id",
            {
                "model": model,
                "prompt_id": test["id"],
                "response_run": identity.get("blackbox_run_id"),
                "file_run": snapshot_readback.get("identity", {}).get("blackbox_run_id"),
            },
        )
    if (snapshot_readback.get("chat_agent") or {}).get("selected_tool", {}).get("name") != selected:
        raise MatrixError(
            "PROMPT_MATRIX_AGENT_TRACE_READBACK_MISMATCH",
            "persisted snapshot readback did not match selected tool",
            {"model": model, "prompt_id": test["id"], "selected": selected},
        )
    policy = None
    if test.get("expected_policy_decision"):
        policy = latest_policy_status(ledger_path, str(identity.get("blackbox_run_id")))
        if not policy or policy.get("status") != test["expected_policy_decision"]:
            raise MatrixError(
                "PROMPT_MATRIX_POLICY_STATUS_MISMATCH",
                "policy tool did not persist the expected policy status",
                {
                    "model": model,
                    "prompt_id": test["id"],
                    "expected": test["expected_policy_decision"],
                    "actual": policy,
                },
            )
    proof = snapshot.get("end_user_proof") or {}
    if not proof.get("headline") or not proof.get("proof_cards"):
        raise MatrixError(
            "PROMPT_MATRIX_END_USER_PROOF_MISSING",
            "snapshot is missing the plain-language proof section",
            {"model": model, "prompt_id": test["id"], "snapshot_path": str(snapshot_path)},
        )
    return {
        "prompt_id": test["id"],
        "model": model,
        "expected_tool": expected,
        "selected_tool": selected,
        "tool_choice_correct": selected == expected,
        "blackbox_run_id": identity.get("blackbox_run_id"),
        "snapshot_path": str(snapshot_path),
        "snapshot_sha256": sha256_file(snapshot_path),
        "ledger_path": str(ledger_path),
        "ledger_sha256": sha256_file(ledger_path),
        "ledger_counts": direct_counts,
        "trust_level": ((snapshot.get("metrics") or {}).get("score") or {}).get("trust_level"),
        "policy_compliance": ((snapshot.get("metrics") or {}).get("score") or {}).get("policy_compliance"),
        "reward": ((snapshot.get("metrics") or {}).get("reward") or {}).get("raw"),
        "loss": ((snapshot.get("metrics") or {}).get("reward") or {}).get("loss"),
        "proof_status": proof.get("status"),
        "proof_headline": proof.get("headline"),
        "policy_status": policy,
        "source_of_truth": snapshot.get("frontend_persistence", {}).get("source_of_truth"),
    }


def run_case(base_url: str, model: str, test: dict[str, Any], index: int) -> dict[str, Any]:
    request_id = f"prompt-matrix-{index}-{model.replace(':', '-')}-{test['id']}-{time.time_ns()}"
    started = time.perf_counter()
    _, snapshot = request_json(
        base_url + "/api/chat",
        {
            "message": test["message"],
            "model": model,
            "compile_reward": True,
            "client_id": "prompt-matrix",
            "request_id": request_id,
        },
        timeout=2400.0,
    )
    verified = verify_snapshot(snapshot, test, model)
    verified["elapsed_ms"] = int((time.perf_counter() - started) * 1000)
    verified["request_id"] = request_id
    return verified


def runtime_state(base_url: str) -> dict[str, Any]:
    _, runs = request_json(base_url + "/api/runs", timeout=30.0)
    _, health = request_json(base_url + "/api/health", timeout=30.0)
    runtime_root = Path(str(health.get("runtime_root")))
    index = runtime_root / "frontend-run-index.jsonl"
    index_lines = 0
    if index.exists():
        index_lines = len(index.read_text(encoding="utf-8").splitlines())
    return {
        "run_count": len(runs.get("runs") or []),
        "complete_run_count": sum(1 for item in (runs.get("runs") or []) if item.get("complete")),
        "runtime_root": str(runtime_root),
        "frontend_index_lines": index_lines,
        "source_of_truth": f"file:{index}",
    }


def run_edge_cases(base_url: str) -> list[dict[str, Any]]:
    cases = [
        ("empty_prompt", {"message": "", "model": DEFAULT_MODELS[0]}, "GEMMAFRONTEND_PROMPT_EMPTY"),
        ("overlong_prompt", {"message": "x" * (MAX_PROMPT_CHARS + 1), "model": DEFAULT_MODELS[0]}, "GEMMAFRONTEND_PROMPT_TOO_LONG"),
        ("missing_model", {"message": "Inspect the latest run.", "model": "missing-model:does-not-exist"}, "GEMMAFRONTEND_OLLAMA_MODEL_MISSING"),
    ]
    results = []
    for name, payload, expected_code in cases:
        before = runtime_state(base_url)
        status, body = request_json(
            base_url + "/api/chat",
            {**payload, "client_id": "prompt-matrix-edge", "request_id": f"prompt-matrix-edge-{name}-{time.time_ns()}"},
            timeout=60.0,
            allow_error=True,
        )
        after = runtime_state(base_url)
        actual_code = body.get("error_code")
        state_unchanged = before == after
        if actual_code != expected_code or not state_unchanged:
            raise MatrixError(
                "PROMPT_MATRIX_EDGE_CASE_FAILED",
                "edge case did not fail closed with unchanged runtime state",
                {
                    "case": name,
                    "expected_code": expected_code,
                    "actual_code": actual_code,
                    "status": status,
                    "before": before,
                    "after": after,
                    "body": body,
                },
            )
        results.append(
            {
                "case": name,
                "expected_error_code": expected_code,
                "actual_error_code": actual_code,
                "http_status": status,
                "before": before,
                "after": after,
                "state_unchanged": state_unchanged,
            }
        )
    return results


def summarize(results: list[dict[str, Any]]) -> dict[str, Any]:
    by_model: dict[str, dict[str, Any]] = {}
    for item in results:
        model = item["model"]
        entry = by_model.setdefault(model, {"total": 0, "tool_choice_correct": 0, "rewards": [], "latencies_ms": []})
        entry["total"] += 1
        entry["tool_choice_correct"] += 1 if item["tool_choice_correct"] else 0
        if isinstance(item.get("reward"), (int, float)):
            entry["rewards"].append(item["reward"])
        entry["latencies_ms"].append(item["elapsed_ms"])
    for entry in by_model.values():
        total = entry["total"]
        rewards = entry.pop("rewards")
        latencies = entry.pop("latencies_ms")
        entry["tool_choice_accuracy"] = round(entry["tool_choice_correct"] / total, 4) if total else None
        entry["average_reward"] = round(sum(rewards) / len(rewards), 4) if rewards else None
        entry["average_latency_ms"] = int(sum(latencies) / len(latencies)) if latencies else None
    return by_model


def write_markdown(path: Path, summary: dict[str, Any]) -> None:
    lines = [
        "# ContextGraph Model Prompt Matrix",
        "",
        f"Matrix id: `{summary['matrix_id']}`",
        f"Status: `{summary['status']}`",
        f"Source of truth: `{summary['source_of_truth']}`",
        "",
        "## Model Comparison",
        "",
        "| Model | Tool accuracy | Avg reward | Avg latency ms |",
        "|---|---:|---:|---:|",
    ]
    for model, data in summary["by_model"].items():
        lines.append(
            f"| `{model}` | {data['tool_choice_accuracy']:.2%} | {data['average_reward']} | {data['average_latency_ms']} |"
        )
    lines.extend(["", "## Prompt Results", ""])
    for item in summary["results"]:
        lines.append(
            f"- `{item['model']}` / `{item['prompt_id']}`: selected `{item['selected_tool']}` "
            f"(expected `{item['expected_tool']}`), proof `{item['proof_status']}`, reward `{item['reward']}`, "
            f"snapshot `{item['snapshot_path']}`"
        )
    failures = summary.get("failures") or []
    if failures:
        lines.extend(["", "## Failures", ""])
        for item in failures:
            lines.append(
                f"- `{item.get('model')}` / `{item.get('prompt_id')}`: `{item.get('error_code')}` - {item.get('message')}"
            )
    lines.extend(["", "## Edge Cases", ""])
    for item in summary["edge_cases"]:
        lines.append(
            f"- `{item['case']}`: `{item['actual_error_code']}`, state_unchanged=`{item['state_unchanged']}`"
        )
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base-url", default="http://127.0.0.1:8787")
    parser.add_argument("--model", action="append", dest="models")
    parser.add_argument("--out-root", type=Path, default=Path("/tmp/gemmafrontend-runs/prompt-matrix"))
    args = parser.parse_args()

    base_url = args.base_url.rstrip("/")
    models = args.models or DEFAULT_MODELS
    matrix_id = f"prompt_matrix_{time.time_ns()}_{hashlib.sha256('|'.join(models).encode()).hexdigest()[:8]}"
    out_dir = args.out_root / matrix_id
    out_dir.mkdir(parents=True, exist_ok=False)
    try:
        preflight = require_models(base_url, models)
        results = []
        failures = []
        index = 0
        for model in models:
            for test in PROMPTS:
                index += 1
                print(f"RUNNING={model}:{test['id']}", flush=True)
                try:
                    result = run_case(base_url, model, test, index)
                    results.append(result)
                    print(f"DONE={model}:{test['id']}:{result['selected_tool']}", flush=True)
                except MatrixError as err:
                    failure = {
                        "model": model,
                        "prompt_id": test["id"],
                        "expected_tool": test["expected_tool"],
                        **err.to_json(),
                    }
                    failures.append(failure)
                    print(f"FAILED={model}:{test['id']}:{err.error_code}", flush=True)
        wrong = [item for item in results if not item["tool_choice_correct"]]
        if wrong:
            failures.append(
                {
                    "model": "*",
                    "prompt_id": "*",
                    **MatrixError(
                        "PROMPT_MATRIX_TOOL_CHOICE_MISMATCH",
                        "one or more model prompts selected the wrong ContextGraph tool",
                        {"wrong": wrong},
                    ).to_json(),
                }
            )
        edge_cases = run_edge_cases(base_url)
        summary = {
            "status": "ok" if not failures else "failed",
            "matrix_id": matrix_id,
            "created_at_unix": time.time(),
            "source_of_truth": f"file:{out_dir / 'prompt-matrix-summary.json'}",
            "preflight": preflight,
            "models": models,
            "prompts": PROMPTS,
            "results": results,
            "failures": failures,
            "by_model": summarize(results),
            "edge_cases": edge_cases,
        }
        summary_path = out_dir / "prompt-matrix-summary.json"
        summary_path.write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        readback = json.loads(summary_path.read_text(encoding="utf-8"))
        if readback.get("matrix_id") != matrix_id:
            raise MatrixError(
                "PROMPT_MATRIX_SUMMARY_READBACK_MISMATCH",
                "summary readback did not match matrix id",
                {"expected": matrix_id, "actual": readback.get("matrix_id"), "path": str(summary_path)},
            )
        markdown_path = out_dir / "prompt-matrix-summary.md"
        write_markdown(markdown_path, summary)
        print("PROMPT_MATRIX_SUMMARY=" + str(summary_path), flush=True)
        print("PROMPT_MATRIX_MARKDOWN=" + str(markdown_path), flush=True)
        return 0 if not failures else 1
    except MatrixError as err:
        error_path = out_dir / "prompt-matrix-error.json"
        error_path.write_text(json.dumps(err.to_json(), indent=2, sort_keys=True) + "\n", encoding="utf-8")
        print("PROMPT_MATRIX_ERROR=" + str(error_path), flush=True)
        print(json.dumps(err.to_json(), indent=2, sort_keys=True), flush=True)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
