#!/usr/bin/env python3
"""Run real ContextGraph proof tasks across multiple local Ollama models.

This is intentionally not a synthetic benchmark. Each model must use the live
ContextGraph model frontend API, select the governed proof tool, create a real
DynamicJEPA/BlackBoxFix run, and pass independent SQLite/file readback.
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
PROOF_PROMPT = (
    "Run the full ContextGraph proof pipeline. Use scenario counter_world_full_agentops "
    "and the exact capability ids: data_ingest, panel_materialization, "
    "action_conditioned_training, planning, surprise_recording, artifact_provenance, "
    "blackboxfix_fsv."
)


class BenchmarkError(RuntimeError):
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


def request_json(url: str, body: dict[str, Any] | None = None, *, timeout: float = 2400.0) -> tuple[int, dict[str, Any]]:
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
    if status >= 400 or value.get("status") == "error":
        raise BenchmarkError(
            str(value.get("error_code") or "MODEL_BENCHMARK_HTTP_ERROR"),
            str(value.get("message") or "benchmark API request failed"),
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


def require_models(base_url: str, models: list[str]) -> dict[str, Any]:
    _, health = request_json(base_url + "/api/health", timeout=30.0)
    ollama_url = health.get("ollama_url")
    _, tags = request_json(str(ollama_url).rstrip("/") + "/api/tags", timeout=30.0)
    installed = {item.get("name"): item for item in tags.get("models", []) if isinstance(item, dict)}
    missing = [model for model in models if model not in installed]
    if missing:
        raise BenchmarkError(
            "MODEL_BENCHMARK_MODEL_MISSING",
            "one or more required Ollama models are not installed",
            {"missing": missing, "installed": sorted(installed)},
        )
    return {"health": health, "installed_models": {model: installed[model] for model in models}}


def verify_snapshot(snapshot: dict[str, Any], model: str) -> dict[str, Any]:
    agent = snapshot.get("chat_agent") or {}
    selected = (agent.get("selected_tool") or {}).get("name")
    if selected != "run_proof_pipeline":
        raise BenchmarkError(
            "MODEL_BENCHMARK_TOOL_SELECTION_FAILED",
            "model did not select the proof pipeline tool",
            {"model": model, "selected_tool": selected, "chat_agent": agent},
        )
    identity = snapshot.get("identity") or {}
    if identity.get("model") != model:
        raise BenchmarkError(
            "MODEL_BENCHMARK_MODEL_READBACK_MISMATCH",
            "harness report model did not match requested model",
            {"requested": model, "actual": identity.get("model")},
        )
    ledger = snapshot.get("ledger") or {}
    ledger_path = Path(str(ledger.get("path")))
    direct_counts = direct_ledger_counts(ledger_path)
    if direct_counts != ledger.get("physical_counts"):
        raise BenchmarkError(
            "MODEL_BENCHMARK_LEDGER_COUNT_MISMATCH",
            "snapshot ledger counts did not match direct SQLite readback",
            {"model": model, "snapshot": ledger.get("physical_counts"), "direct": direct_counts},
        )
    reward = ((snapshot.get("metrics") or {}).get("reward") or {})
    if not isinstance(reward, dict) or reward.get("raw") is None:
        raise BenchmarkError(
            "MODEL_BENCHMARK_REWARD_MISSING",
            "model run did not produce a persisted reward_compilation row",
            {"model": model, "ledger_counts": direct_counts},
        )
    return {
        "blackbox_run_id": identity.get("blackbox_run_id"),
        "source_root": snapshot.get("source_root"),
        "snapshot_path": snapshot.get("frontend_persistence", {}).get("snapshot_path"),
        "snapshot_sha256": sha256_file(Path(str(snapshot.get("frontend_persistence", {}).get("snapshot_path")))),
        "ledger_path": str(ledger_path),
        "ledger_sha256": sha256_file(ledger_path),
        "ledger_counts": direct_counts,
        "score": (snapshot.get("metrics") or {}).get("score"),
        "reward": reward,
        "selected_tool": selected,
    }


def run_model(base_url: str, model: str, index: int) -> dict[str, Any]:
    request_id = f"model-benchmark-{index}-{model.replace(':', '-')}-{time.time_ns()}"
    started = time.time()
    _, snapshot = request_json(
        base_url + "/api/chat",
        {
            "message": PROOF_PROMPT,
            "model": model,
            "client_id": "model-benchmark",
            "request_id": request_id,
        },
        timeout=2400.0,
    )
    elapsed_ms = int((time.time() - started) * 1000)
    verified = verify_snapshot(snapshot, model)
    return {
        "model": model,
        "request_id": request_id,
        "elapsed_ms": elapsed_ms,
        "verified": verified,
        "source_of_truth": verified["snapshot_path"],
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base-url", default="http://127.0.0.1:8787")
    parser.add_argument("--model", action="append", dest="models")
    parser.add_argument("--out-root", type=Path, default=Path("/tmp/gemmafrontend-runs/model-benchmarks"))
    args = parser.parse_args()

    models = args.models or DEFAULT_MODELS
    benchmark_id = f"model_benchmark_{time.time_ns()}_{hashlib.sha256('|'.join(models).encode()).hexdigest()[:8]}"
    out_dir = args.out_root / benchmark_id
    out_dir.mkdir(parents=True, exist_ok=False)
    try:
        preflight = require_models(args.base_url.rstrip("/"), models)
        runs = []
        for index, model in enumerate(models, start=1):
            print(f"RUNNING_MODEL={model}", flush=True)
            runs.append(run_model(args.base_url.rstrip("/"), model, index))
            print(f"MODEL_DONE={model}", flush=True)
        _, pack = request_json(
            args.base_url.rstrip("/") + "/api/category/build",
            {
                "audience": "multi-model verified outcome benchmark arena",
                "max_runs": min(12, max(1, len(models))),
                "client_id": "model-benchmark",
                "request_id": f"model-benchmark-category-{time.time_ns()}",
            },
            timeout=300.0,
        )
        arena_models = {row.get("model") for row in pack.get("model_arena", {}).get("rows", [])}
        missing_from_arena = sorted(set(models) - arena_models)
        if missing_from_arena:
            raise BenchmarkError(
                "MODEL_BENCHMARK_ARENA_MODEL_MISSING",
                "product category arena did not include every benchmarked model",
                {"missing": missing_from_arena, "arena_models": sorted(arena_models), "pack_file": pack.get("pack_file")},
            )
        result = {
            "status": "ok",
            "benchmark_id": benchmark_id,
            "created_at_unix": time.time(),
            "source_of_truth": f"file:{out_dir / 'model-benchmark-summary.json'}",
            "preflight": preflight,
            "models": models,
            "runs": runs,
            "product_category_pack": {
                "pack_id": pack.get("pack_id"),
                "pack_file": pack.get("pack_file"),
                "model_arena": pack.get("model_arena"),
                "leaderboard": pack.get("verified_outcome_leaderboard"),
            },
        }
        summary_path = out_dir / "model-benchmark-summary.json"
        summary_path.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        readback = json.loads(summary_path.read_text(encoding="utf-8"))
        if readback.get("benchmark_id") != benchmark_id:
            raise BenchmarkError(
                "MODEL_BENCHMARK_SUMMARY_READBACK_MISMATCH",
                "summary readback did not match benchmark id",
                {"expected": benchmark_id, "actual": readback.get("benchmark_id"), "path": str(summary_path)},
            )
        print("MODEL_BENCHMARK_SUMMARY=" + str(summary_path), flush=True)
        return 0
    except BenchmarkError as err:
        error_path = out_dir / "model-benchmark-error.json"
        error_path.write_text(json.dumps(err.to_json(), indent=2, sort_keys=True) + "\n", encoding="utf-8")
        print("MODEL_BENCHMARK_ERROR=" + str(error_path), flush=True)
        print(json.dumps(err.to_json(), indent=2, sort_keys=True), flush=True)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
