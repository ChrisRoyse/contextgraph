#!/usr/bin/env python3
"""Full-state verification harness for conceptual capability MCP tools.

This script is intentionally strict:
- It runs the real Rust MCP FSV tests, which open real RocksDB stores.
- It parses the printed source-of-truth evidence instead of trusting return
  codes alone.
- It runs the unified real-data CAMD/learner FSV against the local Shakespeare,
  GoEmotions, and LMDB assets.
- It writes a JSON report with timings, parsed evidence, and command logs.

The controlled vectors/events in the Rust tests are synthetic inputs, but not
mock storage. They are persisted to real RocksDB and then read back from the
same column families the production tools use.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[2]
DEFAULT_REPORT = (
    Path.home()
    / ".contextgraph"
    / "reports"
    / "conceptual_capabilities_fsv_latest.json"
)


class FsvFailure(RuntimeError):
    pass


@dataclass
class CommandResult:
    name: str
    command: list[str]
    elapsed_sec: float
    returncode: int
    stdout: str
    stderr: str

    def tail(self, text: str, lines: int = 80) -> list[str]:
        return text.splitlines()[-lines:]

    def as_report(self, include_full_logs: bool) -> dict[str, Any]:
        out: dict[str, Any] = {
            "name": self.name,
            "command": self.command,
            "elapsed_sec": round(self.elapsed_sec, 3),
            "returncode": self.returncode,
            "stdout_tail": self.tail(self.stdout),
            "stderr_tail": self.tail(self.stderr),
        }
        if include_full_logs:
            out["stdout"] = self.stdout
            out["stderr"] = self.stderr
        return out


def run_command(name: str, command: list[str], timeout_sec: int) -> CommandResult:
    print(f"\n=== RUN {name} ===")
    print("COMMAND:", " ".join(command))
    start = time.perf_counter()
    proc = subprocess.run(
        command,
        cwd=ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=timeout_sec,
        env=os.environ.copy(),
    )
    elapsed = time.perf_counter() - start
    result = CommandResult(name, command, elapsed, proc.returncode, proc.stdout, proc.stderr)
    print(f"RETURN={proc.returncode} ELAPSED={elapsed:.3f}s")
    print_tail(result.stdout, "STDOUT")
    if result.stderr.strip():
        print_tail(result.stderr, "STDERR")
    if proc.returncode != 0:
        raise FsvFailure(f"{name} failed with exit code {proc.returncode}")
    return result


def print_tail(text: str, label: str, lines: int = 50) -> None:
    body = "\n".join(text.splitlines()[-lines:])
    if body:
        print(f"--- {label} tail ---")
        print(body)


def require_regex(text: str, pattern: str, label: str) -> re.Match[str]:
    match = re.search(pattern, text)
    if not match:
        raise FsvFailure(f"missing evidence for {label}: /{pattern}/")
    return match


def parse_derive_fsv(text: str) -> dict[str, Any]:
    require_regex(text, r"SOURCE OF TRUTH: RocksDB CF_CONSTELLATIONS before=0", "derive source")
    seed = require_regex(text, r"AFTER SEED constellations=(\d+)", "derive seed count")
    interp = require_regex(
        text,
        r"HAPPY INTERPOLATE after=(\d+) derived=([0-9a-f-]+) e1\[0\]=([\-0-9.]+) e1\[1\]=([\-0-9.]+)",
        "derive interpolate readback",
    )
    invalid_alpha = require_regex(
        text,
        r"EDGE INVALID_ALPHA before=(\d+) after=(\d+) isError=true",
        "derive invalid alpha edge",
    )
    missing_other = require_regex(
        text,
        r"EDGE MISSING_OTHER before=(\d+) after=(\d+) isError=true",
        "derive missing other edge",
    )
    anti = require_regex(
        text,
        r"HAPPY ANTI_POLE after=(\d+) derived=([0-9a-f-]+) e1\[0\]=([\-0-9.]+)",
        "derive anti-pole readback",
    )
    short_pool = require_regex(
        text,
        r"EDGE SHORT_ANTI_POOL before=(\d+) after=(\d+) isError=true",
        "derive short anti-pool edge",
    )

    e10 = float(interp.group(3))
    e11 = float(interp.group(4))
    anti_e10 = float(anti.group(3))
    if int(seed.group(1)) != 2:
        raise FsvFailure(f"derive seed count expected 2, got {seed.group(1)}")
    if abs(e10 - 0.75) > 1e-5 or abs(e11 - 0.25) > 1e-5:
        raise FsvFailure(f"interpolation centroid mismatch: e1[0]={e10}, e1[1]={e11}")
    if abs(anti_e10 + 1.0) > 1e-5:
        raise FsvFailure(f"anti-pole centroid expected -1.0, got {anti_e10}")
    for label, match in [
        ("invalid_alpha", invalid_alpha),
        ("missing_other", missing_other),
        ("short_pool", short_pool),
    ]:
        if match.group(1) != match.group(2):
            raise FsvFailure(f"{label} mutated state: before={match.group(1)} after={match.group(2)}")

    return {
        "source_of_truth": "RocksDB CF_CONSTELLATIONS + CF_CONSTELLATION_BY_SELECTOR",
        "seed_count": int(seed.group(1)),
        "interpolate": {
            "after_count": int(interp.group(1)),
            "derived_id": interp.group(2),
            "e1_0": e10,
            "e1_1": e11,
        },
        "anti_pole": {
            "after_count": int(anti.group(1)),
            "derived_id": anti.group(2),
            "e1_0": anti_e10,
        },
        "edge_cases": {
            "invalid_alpha_count_unchanged": invalid_alpha.group(1) == invalid_alpha.group(2),
            "missing_other_count_unchanged": missing_other.group(1) == missing_other.group(2),
            "short_anti_pool_count_unchanged": short_pool.group(1) == short_pool.group(2),
        },
    }


def parse_outcome_fsv(text: str) -> dict[str, Any]:
    require_regex(text, r"SOURCE OF TRUTH: RocksDB CF_LEARNING_EVENTS before=0", "outcome source")
    after = require_regex(
        text,
        r"AFTER RECORD learning_events=(\d+) useful_delta=([\-0-9.]+) feature_delta_e=([\-0-9.]+)",
        "outcome readback",
    )
    happy = require_regex(
        text,
        r"HAPPY ESTIMATE before=(\d+) after=(\d+) predicted=([\-0-9.]+) label=\"?([a-z_]+)\"? neighbors=(\d+)",
        "outcome estimate",
    )
    invalid_dims = require_regex(
        text,
        r"EDGE INVALID_DIMS before=(\d+) after=(\d+) isError=true",
        "outcome invalid dimensions edge",
    )
    no_match = require_regex(
        text,
        r"EDGE NO_MATCH before=(\d+) after=(\d+) isError=true",
        "outcome no match edge",
    )
    bad_limit = require_regex(
        text,
        r"EDGE BAD_LIMIT before=(\d+) after=(\d+) isError=true",
        "outcome bad limit edge",
    )

    if int(after.group(1)) != 3:
        raise FsvFailure(f"learning event count expected 3, got {after.group(1)}")
    if abs(float(after.group(2)) - 0.8) > 1e-5:
        raise FsvFailure(f"useful readback utility delta mismatch: {after.group(2)}")
    if happy.group(1) != happy.group(2):
        raise FsvFailure(f"estimate mutated state: before={happy.group(1)} after={happy.group(2)}")
    predicted = float(happy.group(3))
    if predicted < 0.79 or happy.group(4) != "useful":
        raise FsvFailure(f"unexpected estimate: predicted={predicted} label={happy.group(4)}")
    for label, match in [
        ("invalid_dims", invalid_dims),
        ("no_match", no_match),
        ("bad_limit", bad_limit),
    ]:
        if match.group(1) != match.group(2):
            raise FsvFailure(f"{label} mutated state: before={match.group(1)} after={match.group(2)}")

    return {
        "source_of_truth": "RocksDB CF_LEARNING_EVENTS",
        "after_record_count": int(after.group(1)),
        "useful_delta": float(after.group(2)),
        "feature_delta_e": float(after.group(3)),
        "estimate": {
            "before_count": int(happy.group(1)),
            "after_count": int(happy.group(2)),
            "predicted_utility_delta": predicted,
            "predicted_label": happy.group(4),
            "neighbors": int(happy.group(5)),
        },
        "edge_cases": {
            "invalid_dims_count_unchanged": invalid_dims.group(1) == invalid_dims.group(2),
            "no_match_count_unchanged": no_match.group(1) == no_match.group(2),
            "bad_limit_count_unchanged": bad_limit.group(1) == bad_limit.group(2),
        },
    }


def parse_tools_list(text: str) -> dict[str, Any]:
    found = require_regex(text, r"Found (\d+) tools:", "tools count")
    count = int(found.group(1))
    if count != 103:
        raise FsvFailure(f"expected 103 MCP tools, found {count}")
    for tool in ["derive_constellation", "estimate_learning_outcome", "resolve_learner_retrieval_policy"]:
        require_regex(text, rf"- {tool} \(desc: true, schema: true\)", f"{tool} listed")
    return {
        "tool_count": count,
        "required_tools_present": [
            "derive_constellation",
            "estimate_learning_outcome",
            "resolve_learner_retrieval_policy",
        ],
    }


def parse_unified_realdata(text: str) -> dict[str, Any]:
    require_regex(text, r"SOURCE OF TRUTH learning_events=RocksDB:", "unified source")
    json_start = text.find("{")
    if json_start < 0:
        raise FsvFailure("unified real-data FSV did not emit JSON")
    payload = json.loads(text[json_start:])
    if payload.get("status") != "ok":
        raise FsvFailure(f"unified real-data FSV status not ok: {payload.get('status')}")
    happy = payload["happy_path"]
    edge = payload["edge_cases"]
    lmdb = payload["lmdb"]
    if happy["after_import_count"] != 2 or happy["learner_pair_count"] != 1:
        raise FsvFailure(f"unexpected unified happy-path counts: {happy}")
    if lmdb["records"] <= 0:
        raise FsvFailure(f"LMDB source has no records: {lmdb}")
    for name in ["empty_import", "invalid_dimension", "limit_export"]:
        if edge[name]["before_count"] != edge[name]["after_count"]:
            raise FsvFailure(f"unified edge {name} mutated DB count: {edge[name]}")
    return {
        "source_of_truth": {
            "learning_events": payload["run_dir"] + "/rocksdb",
            "lmdb": lmdb["source_of_truth"],
        },
        "datasets": payload["datasets"],
        "happy_path": {
            "after_import_count": happy["after_import_count"],
            "learner_pair_count": happy["learner_pair_count"],
            "exported_tensor_lens": happy["exported_tensor_lens"],
            "utility_delta": happy["good_readback"]["utility_delta"],
        },
        "edge_cases": edge,
        "lmdb_records": lmdb["records"],
        "run_dir": payload["run_dir"],
    }


def analyze_benchmarks(results: dict[str, Any], budgets: dict[str, float]) -> dict[str, Any]:
    timings = {
        key: value["elapsed_sec"]
        for key, value in results["commands"].items()
    }
    budget_results = {}
    for name, elapsed in timings.items():
        budget = budgets.get(name)
        if budget is None:
            continue
        budget_results[name] = {
            "elapsed_sec": elapsed,
            "budget_sec": budget,
            "within_budget": elapsed <= budget,
            "headroom_sec": round(budget - elapsed, 3),
        }
        if elapsed > budget:
            raise FsvFailure(f"{name} exceeded benchmark budget: {elapsed:.3f}s > {budget:.3f}s")

    outcome = results["evidence"]["learning_outcome"]
    derive = results["evidence"]["derive_constellation"]
    quality = {
        "outcome_prediction_abs_error_vs_expected": round(
            abs(outcome["estimate"]["predicted_utility_delta"] - 0.8), 8
        ),
        "interpolation_e1_0_abs_error_vs_expected": round(
            abs(derive["interpolate"]["e1_0"] - 0.75), 8
        ),
        "interpolation_e1_1_abs_error_vs_expected": round(
            abs(derive["interpolate"]["e1_1"] - 0.25), 8
        ),
        "anti_pole_e1_0_abs_error_vs_expected": round(
            abs(derive["anti_pole"]["e1_0"] - -1.0), 8
        ),
    }
    return {
        "timings": timings,
        "budgets": budget_results,
        "quality": quality,
        "benchmark_note": (
            "The expensive step is model/provider initialization inside Rust MCP tests. "
            "Functional quality metrics are exact or near-exact; runtime improvements should "
            "come from grouping FSV cases into fewer Rust process launches, not weakening checks."
        ),
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--report", type=Path, default=DEFAULT_REPORT)
    parser.add_argument("--include-full-logs", action="store_true")
    parser.add_argument("--skip-realdata", action="store_true")
    parser.add_argument("--timeout-sec", type=int, default=240)
    parser.add_argument("--budget-derive-sec", type=float, default=80.0)
    parser.add_argument("--budget-outcome-sec", type=float, default=80.0)
    parser.add_argument("--budget-tools-sec", type=float, default=80.0)
    parser.add_argument("--budget-unified-sec", type=float, default=60.0)
    args = parser.parse_args()

    report: dict[str, Any] = {
        "status": "running",
        "created_at_unix": int(time.time()),
        "repo": str(ROOT),
        "data_plan": {
            "derive_constellation": {
                "source_of_truth": "temporary real RocksDB CF_CONSTELLATIONS and CF_FINGERPRINTS",
                "inputs": "controlled persisted vectors with known centroids: source e1=(1,0), other e1=(0,1), anti-pole e1=(-1,0)",
                "expected": "interpolate alpha=0.25 => e1=(0.75,0.25); anti-pole => e1[0]=-1",
            },
            "estimate_learning_outcome": {
                "source_of_truth": "temporary real RocksDB CF_LEARNING_EVENTS",
                "inputs": "three controlled persisted LearningEvent rows with useful/harmful/neutral labels",
                "expected": "candidate matching useful trajectory predicts utility_delta ~= 0.8 and leaves event count unchanged",
            },
            "unified_realdata": {
                "source_of_truth": "temporary real RocksDB plus local LMDB/JSONL assets",
                "inputs": [
                    "~/.contextgraph/training/v4_2_camd_lmdb.jsonl",
                    "data/utl_calibration/go_emotions/go_emotions_sample.jsonl",
                    "~/.contextgraph/extended_embeddings.lmdb",
                ],
                "expected": "2 imported LearningEvents, 1 learner DPO pair, trainer dry-run validates CAMD + learner pairs",
            },
        },
        "commands": {},
        "evidence": {},
        "analysis": {},
    }

    commands = [
        (
            "derive_constellation_fsv",
            [
                "cargo",
                "test",
                "-p",
                "context-graph-mcp",
                "test_e2e_derive_constellation_fsv",
                "--features",
                "llm",
                "--",
                "--nocapture",
            ],
            parse_derive_fsv,
        ),
        (
            "learning_outcome_fsv",
            [
                "cargo",
                "test",
                "-p",
                "context-graph-mcp",
                "test_e2e_estimate_learning_outcome_fsv",
                "--features",
                "llm",
                "--",
                "--nocapture",
            ],
            parse_outcome_fsv,
        ),
        (
            "mcp_tool_list",
            [
                "cargo",
                "test",
                "-p",
                "context-graph-mcp",
                "test_e2e_tools_list_all_tools",
                "--features",
                "llm",
                "--",
                "--nocapture",
            ],
            parse_tools_list,
        ),
    ]
    if not args.skip_realdata:
        commands.append(
            (
                "unified_realdata_fsv",
                ["python3", "scripts/v4_2/unified_realdata_fsv.py", "--keep-run-dir"],
                parse_unified_realdata,
            )
        )

    try:
        for name, command, parser_fn in commands:
            result = run_command(name, command, args.timeout_sec)
            report["commands"][name] = result.as_report(args.include_full_logs)
            report["evidence"][name.replace("_fsv", "")] = parser_fn(
                result.stdout + "\n" + result.stderr
            )

        report["analysis"] = analyze_benchmarks(
            report,
            {
                "derive_constellation_fsv": args.budget_derive_sec,
                "learning_outcome_fsv": args.budget_outcome_sec,
                "mcp_tool_list": args.budget_tools_sec,
                "unified_realdata_fsv": args.budget_unified_sec,
            },
        )
        report["status"] = "ok"
    except Exception as exc:
        report["status"] = "failed"
        report["error"] = f"{type(exc).__name__}: {exc}"
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps(report, indent=2, sort_keys=True), encoding="utf-8")
        print(f"\nFSV FAILED: {exc}", file=sys.stderr)
        print(f"REPORT_WRITTEN={args.report}", file=sys.stderr)
        return 1

    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2, sort_keys=True), encoding="utf-8")
    print("\n=== FSV SUMMARY ===")
    print(json.dumps({
        "status": report["status"],
        "report": str(args.report),
        "analysis": report["analysis"],
    }, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
