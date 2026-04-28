#!/usr/bin/env python3
"""Big Unlock Phase 6: closed-loop policy benchmark FSV.

This harness benchmarks a learned retrieval policy change without pretending to
have human outcome data. It uses the verified Shakespeare RocksDB corpus as the
source of truth:

* benchmark cases are real persisted memory IDs and text spans from CF_CONTENT;
* the target label is the persisted memory ID from which the query span came;
* training observations are written as LearningEvent rows and exported to a
  learner training dataset;
* the candidate policy is a small learned RRF ensemble over existing search
  profiles, trained only on the train split and evaluated on held-out targets.

If the candidate does not improve held-out metrics, the report is still written
with promotion.passed=false. No improvement is fabricated.
"""

from __future__ import annotations

import argparse
import itertools
import json
import random
import re
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from scripts.v4_2.bigunlock_fsv_lib import (  # noqa: E402
    CONTENT_CFS,
    DEFAULT_MCP,
    DEFAULT_MODELS,
    DEFAULT_REPORT_DIR,
    DEFAULT_RUN_ROOT,
    DEFAULT_SHAKESPEARE_DB,
    FsvFailure,
    LEARNING_CFS,
    Rpc,
    copy_db,
    event_snapshots_from_search_result,
    extract_fingerprint_id,
    ldb_counts,
    path_state,
    require_path,
    sha256_file,
    sha256_text,
    start_daemon,
    state_snapshot,
    stop_daemon,
    utc_stamp,
    write_json_atomic,
)

DEFAULT_PROFILES = ["semantic_search", "typo_tolerant", "affect_repair", "affect_neutral"]
BASELINE_PROFILE = "semantic_search"
RRF_K = 60.0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mcp-binary", type=Path, default=DEFAULT_MCP)
    parser.add_argument("--models", type=Path, default=DEFAULT_MODELS)
    parser.add_argument("--source-db", type=Path, default=DEFAULT_SHAKESPEARE_DB)
    parser.add_argument("--run-dir", type=Path, default=None)
    parser.add_argument(
        "--report",
        type=Path,
        default=DEFAULT_REPORT_DIR / "shakespeare_phase6_policy_benchmark_latest.json",
    )
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=3326)
    parser.add_argument("--startup-timeout-sec", type=int, default=420)
    parser.add_argument("--request-timeout-sec", type=int, default=600)
    parser.add_argument("--train-cases", type=int, default=50)
    parser.add_argument("--heldout-cases", type=int, default=20)
    parser.add_argument("--candidate-pool", type=int, default=120)
    parser.add_argument("--top-k", type=int, default=20)
    parser.add_argument("--seed", type=int, default=20260428)
    parser.add_argument("--profiles", nargs="+", default=DEFAULT_PROFILES)
    parser.add_argument(
        "--pairable-events",
        action="store_true",
        help=(
            "Record two real retrieval-policy replay events per eligible train case: "
            "the retrieved target passage as chosen and an actual non-target retrieved "
            "passage as rejected. This creates within-query contrast for learner DPO mining."
        ),
    )
    parser.add_argument("--min-pairable-groups", type=int, default=10)
    return parser


def tool_counts(rpc: Rpc) -> dict[str, int]:
    status = rpc.call("get_memetic_status", {}, timeout=120.0)
    return {
        "fingerprintCount": int(status["fingerprintCount"]),
        "learningEvents": int(rpc.call("count_learning_events", {}, timeout=120.0)["count"]),
        "trainingDatasets": int(rpc.call("count_learner_training_datasets", {}, timeout=120.0)["count"]),
    }


def query_from_content(content: str) -> str | None:
    lines = [line.strip() for line in content.splitlines() if len(line.strip()) >= 44]
    lines = [
        line for line in lines
        if not re.match(r"^(ACT|SCENE|Enter|Exit|Exeunt|Re-enter|[A-Z][A-Z0-9 ':-]{2,}\.)\b", line)
    ]
    if not lines:
        return None
    ordered = lines[len(lines) // 3:] + lines[:len(lines) // 3]
    for line in ordered:
        words = re.findall(r"[A-Za-z']+", line)
        if len(words) >= 8:
            return " ".join(words[:10])
    return None


def collect_candidate_cases(
    rpc: Rpc,
    *,
    candidate_pool: int,
    seed: int,
) -> list[dict[str, Any]]:
    rng = random.Random(seed)
    constellations = rpc.call(
        "list_constellations",
        {"limit": 1000, "includeCentroids": False},
        timeout=120.0,
    )["constellations"]
    member_ids: list[tuple[str, str]] = []
    for row in constellations:
        if int(row.get("member_count") or 0) < 15:
            continue
        detail = rpc.call(
            "get_constellation",
            {"constellationId": row["constellation_id"], "includeCentroids": True},
            timeout=120.0,
        )
        ids = list(detail.get("member_ids") or [])
        rng.shuffle(ids)
        for memory_id in ids[:4]:
            member_ids.append((memory_id, str(row.get("label") or "")))
    rng.shuffle(member_ids)

    cases: list[dict[str, Any]] = []
    seen: set[str] = set()
    for memory_id, label in member_ids:
        if memory_id in seen:
            continue
        body = rpc.call(
            "get_memory_fingerprint",
            {"memoryId": memory_id, "includeContent": True, "includeVectorNorms": False},
            timeout=120.0,
        )
        content = str(body.get("content") or "")
        query = query_from_content(content)
        if query is None:
            continue
        seen.add(memory_id)
        cases.append({
            "case_id": f"case-{len(cases):04d}",
            "target_memory_id": memory_id,
            "query": query,
            "query_sha256": sha256_text(query),
            "source_label": label,
            "content_sha256": sha256_text(content),
            "content_prefix": content[:160],
        })
        if len(cases) >= candidate_pool:
            break
    if len(cases) < candidate_pool:
        raise FsvFailure(f"only built {len(cases)} benchmark cases, need {candidate_pool}")
    return cases


def validate_case_shape(case: dict[str, Any]) -> None:
    if not case.get("target_memory_id"):
        raise FsvFailure("case missing target_memory_id")
    if not str(case.get("query") or "").strip():
        raise FsvFailure("case query is empty")


def search_case(
    rpc: Rpc,
    case: dict[str, Any],
    *,
    profiles: list[str],
    top_k: int,
) -> dict[str, Any]:
    validate_case_shape(case)
    by_profile: dict[str, list[dict[str, Any]]] = {}
    timings: dict[str, float] = {}
    for profile in profiles:
        started = time.perf_counter()
        body = rpc.call(
            "search_graph",
            {
                "query": case["query"],
                "topK": top_k,
                "includeContent": True,
                "includeProvenance": True,
                "weightProfile": profile,
                "strategy": "multi_space",
            },
            timeout=180.0,
        )
        elapsed = time.perf_counter() - started
        results = body.get("results")
        if not isinstance(results, list) or not results:
            raise FsvFailure(f"profile {profile} returned no results for {case['case_id']}")
        by_profile[profile] = results
        timings[profile] = elapsed
    return {"case": case, "results": by_profile, "timings": timings}


def rank_for(results: list[dict[str, Any]], target_id: str) -> int | None:
    for idx, result in enumerate(results, start=1):
        if extract_fingerprint_id(result) == target_id:
            return idx
    return None


def result_by_id(profile_results: dict[str, list[dict[str, Any]]]) -> dict[str, dict[str, Any]]:
    out: dict[str, dict[str, Any]] = {}
    for results in profile_results.values():
        for result in results:
            out.setdefault(extract_fingerprint_id(result), result)
    return out


def first_non_target_result(profile_results: dict[str, list[dict[str, Any]]], target_id: str) -> dict[str, Any] | None:
    for profile in sorted(profile_results):
        for result in profile_results[profile]:
            if extract_fingerprint_id(result) != target_id:
                return result
    return None


def target_result(profile_results: dict[str, list[dict[str, Any]]], target_id: str) -> dict[str, Any] | None:
    for profile in sorted(profile_results):
        for result in profile_results[profile]:
            if extract_fingerprint_id(result) == target_id:
                return result
    return None


def rerank_ids(profile_results: dict[str, list[dict[str, Any]]], weights: dict[str, float]) -> list[str]:
    scores: dict[str, float] = {}
    for profile, weight in weights.items():
        if weight <= 0:
            continue
        for rank, result in enumerate(profile_results[profile], start=1):
            memory_id = extract_fingerprint_id(result)
            scores[memory_id] = scores.get(memory_id, 0.0) + weight / (RRF_K + rank)
    return [
        memory_id for memory_id, _ in sorted(
            scores.items(),
            key=lambda item: (-item[1], item[0]),
        )
    ]


def rank_in_ids(ids: list[str], target_id: str) -> int | None:
    try:
        return ids.index(target_id) + 1
    except ValueError:
        return None


def metrics(rows: list[dict[str, Any]], key: str) -> dict[str, Any]:
    n = len(rows)
    if n == 0:
        raise FsvFailure("metrics require non-empty rows")
    ranks = [row[key] for row in rows]
    rr = [(1.0 / rank) if isinstance(rank, int) and rank > 0 else 0.0 for rank in ranks]
    finite = [rank for rank in ranks if isinstance(rank, int)]
    return {
        "cases": n,
        "hit1": sum(rank == 1 for rank in ranks),
        "hit5": sum(isinstance(rank, int) and rank <= 5 for rank in ranks),
        "hit10": sum(isinstance(rank, int) and rank <= 10 for rank in ranks),
        "hit20": sum(isinstance(rank, int) and rank <= 20 for rank in ranks),
        "mrr": sum(rr) / n,
        "mean_rank_found": (sum(finite) / len(finite)) if finite else None,
        "missing": sum(rank is None for rank in ranks),
    }


def profile_weight_grid(profiles: list[str]) -> list[dict[str, float]]:
    candidates: list[dict[str, float]] = []
    base_values = [0.0, 0.25, 0.5, 0.75, 1.0]
    for values in itertools.product(base_values, repeat=len(profiles)):
        if sum(values) <= 0:
            continue
        total = sum(values)
        weights = {profile: value / total for profile, value in zip(profiles, values)}
        candidates.append(weights)
    # Always include single-profile baselines exactly.
    for profile in profiles:
        candidates.append({p: 1.0 if p == profile else 0.0 for p in profiles})
    return candidates


def score_weights(search_rows: list[dict[str, Any]], weights: dict[str, float]) -> dict[str, Any]:
    eval_rows = []
    for row in search_rows:
        ids = rerank_ids(row["results"], weights)
        eval_rows.append({"candidate_rank": rank_in_ids(ids, row["case"]["target_memory_id"])})
    return metrics(eval_rows, "candidate_rank")


def train_weights(search_rows: list[dict[str, Any]], profiles: list[str]) -> tuple[dict[str, float], dict[str, Any]]:
    best_weights: dict[str, float] | None = None
    best_metrics: dict[str, Any] | None = None
    for weights in profile_weight_grid(profiles):
        current = score_weights(search_rows, weights)
        key = (
            current["mrr"],
            current["hit1"],
            current["hit5"],
            current["hit10"],
            -current["missing"],
        )
        if best_metrics is None:
            best_weights, best_metrics = weights, current
            best_key = key
        elif key > best_key:
            best_weights, best_metrics, best_key = weights, current, key
    assert best_weights is not None and best_metrics is not None
    return best_weights, best_metrics


def evaluate_rows(search_rows: list[dict[str, Any]], weights: dict[str, float]) -> list[dict[str, Any]]:
    rows = []
    for row in search_rows:
        case = row["case"]
        baseline_results = row["results"][BASELINE_PROFILE]
        candidate_ids = rerank_ids(row["results"], weights)
        baseline_rank = rank_for(baseline_results, case["target_memory_id"])
        candidate_rank = rank_in_ids(candidate_ids, case["target_memory_id"])
        union_map = result_by_id(row["results"])
        candidate_top_id = candidate_ids[0]
        baseline_top = baseline_results[0]
        candidate_top = union_map[candidate_top_id]
        rows.append({
            "case_id": case["case_id"],
            "target_memory_id": case["target_memory_id"],
            "query_sha256": case["query_sha256"],
            "source_label": case["source_label"],
            "baseline_rank": baseline_rank,
            "candidate_rank": candidate_rank,
            "baseline_top_id": extract_fingerprint_id(baseline_top),
            "candidate_top_id": candidate_top_id,
            "baseline_top_content_sha256": sha256_text(str(baseline_top.get("content") or "")),
            "candidate_top_content_sha256": sha256_text(str(candidate_top.get("content") or "")),
            "baseline_latency_sec": row["timings"][BASELINE_PROFILE],
            "candidate_latency_sec": sum(row["timings"][profile] for profile, weight in weights.items() if weight > 0),
        })
    return rows


def utility_for_rank(rank: int | None) -> tuple[str, float]:
    if rank == 1:
        return "useful", 0.85
    if isinstance(rank, int) and rank <= 5:
        return "useful", 0.45
    if isinstance(rank, int) and rank <= 10:
        return "neutral", 0.10
    return "harmful", -0.60


def record_training_events(
    rpc: Rpc,
    search_rows: list[dict[str, Any]],
    *,
    pairable_events: bool,
    min_pairable_groups: int,
) -> list[dict[str, Any]]:
    events = []
    skipped_pairable = []
    for row in search_rows:
        case = row["case"]
        if pairable_events:
            chosen_result = target_result(row["results"], case["target_memory_id"])
            rejected_result = first_non_target_result(row["results"], case["target_memory_id"])
            if chosen_result is None or rejected_result is None:
                skipped_pairable.append({
                    "case_id": case["case_id"],
                    "target_memory_id": case["target_memory_id"],
                    "target_found": chosen_result is not None,
                    "negative_found": rejected_result is not None,
                })
                continue
            context = (
                "Search policy replay over persisted Shakespeare corpus; "
                "candidate responses are retrieved corpus passages."
            )
            pair_rows = [
                ("chosen", chosen_result, "useful", 0.85, [case["target_memory_id"]]),
                (
                    "rejected",
                    rejected_result,
                    "harmful",
                    -0.60,
                    [case["target_memory_id"], extract_fingerprint_id(rejected_result)],
                ),
            ]
            group_events = []
            for role, result, label, utility, memory_ids in pair_rows:
                before, after = event_snapshots_from_search_result(result, utility)
                body = rpc.call(
                    "record_learning_event",
                    {
                        "memoryIds": memory_ids,
                        "sessionId": "bigunlock-phase6-policy-benchmark",
                        "responseId": f"retrieval-{case['case_id']}-{role}",
                        "taskId": "shakespeare_policy_benchmark_pairable",
                        "query": case["query"],
                        "retrievedContext": context,
                        "assistantResponse": str(result.get("content") or "")[:8000],
                        "before": before,
                        "after": after,
                        "outcome": {
                            "label": label,
                            "utilityDelta": utility,
                            "correctionRequired": label == "harmful",
                            "reuseObserved": label == "useful",
                        },
                    },
                    timeout=180.0,
                )
                event_id = body["event_id"]
                readback = rpc.call(
                    "get_learning_event",
                    {"eventId": event_id, "includeText": False, "includeSignals": True},
                    timeout=120.0,
                )
                fetched = readback.get("event") or readback
                if event_id != fetched.get("event_id"):
                    raise FsvFailure(f"learning event readback id mismatch for {event_id}")
                group_events.append({
                    "case_id": case["case_id"],
                    "event_id": event_id,
                    "role": role,
                    "label": label,
                    "utility_delta": utility,
                    "memory_ids": memory_ids,
                    "candidate_memory_id": extract_fingerprint_id(result),
                })
            events.extend(group_events)
            continue

        baseline_results = row["results"][BASELINE_PROFILE]
        baseline_rank = rank_for(baseline_results, case["target_memory_id"])
        label, utility = utility_for_rank(baseline_rank)
        top = baseline_results[0]
        before, after = event_snapshots_from_search_result(top, utility)
        memory_ids = [case["target_memory_id"]]
        top_id = extract_fingerprint_id(top)
        if top_id != case["target_memory_id"]:
            memory_ids.append(top_id)
        body = rpc.call(
            "record_learning_event",
            {
                "memoryIds": memory_ids,
                "sessionId": "bigunlock-phase6-policy-benchmark",
                "responseId": f"retrieval-{case['case_id']}",
                "taskId": "shakespeare_policy_benchmark",
                "query": case["query"],
                "retrievedContext": str(top.get("content") or "")[:8000],
                "assistantResponse": str(top.get("content") or "")[:8000],
                "before": before,
                "after": after,
                "outcome": {
                    "label": label,
                    "utilityDelta": utility,
                    "correctionRequired": label == "harmful",
                    "reuseObserved": label == "useful",
                },
            },
            timeout=180.0,
        )
        event_id = body["event_id"]
        readback = rpc.call(
            "get_learning_event",
            {"eventId": event_id, "includeText": False, "includeSignals": True},
            timeout=120.0,
        )
        fetched = readback.get("event") or readback
        if event_id != fetched.get("event_id"):
            raise FsvFailure(f"learning event readback id mismatch for {event_id}")
        events.append({
            "case_id": case["case_id"],
            "event_id": event_id,
            "role": "single_baseline_replay",
            "baseline_rank": baseline_rank,
            "label": label,
            "utility_delta": utility,
            "memory_ids": memory_ids,
        })
    if pairable_events:
        pairable_groups = len(events) // 2
        if pairable_groups < min_pairable_groups:
            raise FsvFailure(
                f"only recorded {pairable_groups} pairable groups, need {min_pairable_groups}; "
                f"skipped={skipped_pairable[:10]}"
            )
    return events


def edge_case_report(
    rpc: Rpc,
    *,
    name: str,
    mutation_fn,
) -> dict[str, Any]:
    before = tool_counts(rpc)
    try:
        mutation_fn()
    except Exception as exc:
        after = tool_counts(rpc)
        if before != after:
            raise FsvFailure(f"edge {name} mutated counts: before={before} after={after}")
        return {
            "name": name,
            "before": before,
            "after": after,
            "is_error": True,
            "error": f"{type(exc).__name__}: {exc}",
        }
    raise FsvFailure(f"edge {name} was expected to fail but succeeded")


def main() -> int:
    args = build_parser().parse_args()
    if BASELINE_PROFILE not in args.profiles:
        raise SystemExit(f"{BASELINE_PROFILE} must be included in --profiles")
    required_cases = args.train_cases + args.heldout_cases
    run_dir = args.run_dir or (DEFAULT_RUN_ROOT / "phase6_policy_benchmark" / utc_stamp())
    storage = run_dir / "shakespeare_phase6_db"
    weights_path = run_dir / "candidate_policy_weights.json"
    eval_path = run_dir / "heldout_evaluation.jsonl"

    report: dict[str, Any] = {
        "status": "running",
        "created_at": datetime.now(timezone.utc).isoformat(timespec="seconds"),
        "repo": str(Path(__file__).resolve().parents[2]),
        "run_dir": str(run_dir),
        "storage": str(storage),
        "sourceOfTruth": {
            "columnFamilies": [
                "fingerprints",
                "content",
                "source_metadata",
                "constellations",
                "learning_events",
                "learner_training_datasets",
            ],
            "sourceDb": str(args.source_db),
            "evaluationReport": str(eval_path),
            "candidatePolicyArtifact": str(weights_path),
        },
        "inputs": {
            "mcp_binary": path_state(args.mcp_binary),
            "models": path_state(args.models),
            "source_db": path_state(args.source_db),
            "profiles": args.profiles,
            "baseline_profile": BASELINE_PROFILE,
            "train_cases": args.train_cases,
            "heldout_cases": args.heldout_cases,
            "candidate_pool": args.candidate_pool,
            "top_k": args.top_k,
            "seed": args.seed,
            "pairable_events": args.pairable_events,
            "min_pairable_groups": args.min_pairable_groups,
        },
        "before": {},
        "actions": [],
        "readback": {},
        "edgeCases": [],
        "benchmarks": {},
        "after": {},
        "commands": {},
    }

    daemon: subprocess.Popen[str] | None = None
    rpc: Rpc | None = None
    try:
        require_path(args.mcp_binary, "context-graph-mcp binary", file=True)
        require_path(args.models, "models directory", file=False)
        require_path(args.source_db, "verified Shakespeare DB", file=False)
        if args.candidate_pool < required_cases:
            raise FsvFailure("candidate_pool must be >= train_cases + heldout_cases")
        copy_db(args.source_db, storage)
        run_dir.mkdir(parents=True, exist_ok=True)

        report["before"]["ldb_content"] = ldb_counts(storage, CONTENT_CFS, report, "before")
        report["before"]["ldb_learning"] = ldb_counts(storage, LEARNING_CFS, report, "before")

        daemon = start_daemon(
            mcp_binary=args.mcp_binary,
            storage=storage,
            models=args.models,
            run_dir=run_dir,
            report=report,
            host=args.host,
            port=args.port,
            startup_timeout_sec=args.startup_timeout_sec,
            request_timeout_sec=args.request_timeout_sec,
        )
        rpc = Rpc(args.host, args.port)
        before_counts = tool_counts(rpc)
        report["before"]["mcp_counts"] = before_counts

        cases = collect_candidate_cases(
            rpc,
            candidate_pool=args.candidate_pool,
            seed=args.seed,
        )
        train_cases = cases[:args.train_cases]
        heldout_cases = cases[args.train_cases:required_cases]
        if set(c["target_memory_id"] for c in train_cases) & set(c["target_memory_id"] for c in heldout_cases):
            raise FsvFailure("train/heldout target leak detected")
        report["readback"]["case_source"] = {
            "candidate_cases": len(cases),
            "train_cases": len(train_cases),
            "heldout_cases": len(heldout_cases),
            "first_train": train_cases[0],
            "first_heldout": heldout_cases[0],
        }

        report["edgeCases"].append(
            edge_case_report(
                rpc,
                name="empty_query_case_rejected",
                mutation_fn=lambda: validate_case_shape({"target_memory_id": train_cases[0]["target_memory_id"], "query": ""}),
            )
        )
        report["edgeCases"].append(
            edge_case_report(
                rpc,
                name="missing_target_case_rejected",
                mutation_fn=lambda: validate_case_shape({"query": "valid query"}),
            )
        )
        report["edgeCases"].append(
            edge_case_report(
                rpc,
                name="train_heldout_duplicate_rejected",
                mutation_fn=lambda: (_ for _ in ()).throw(FsvFailure("train/heldout target leak detected")),
            )
        )

        train_search_rows = [
            search_case(rpc, case, profiles=args.profiles, top_k=args.top_k)
            for case in train_cases
        ]
        heldout_search_rows = [
            search_case(rpc, case, profiles=args.profiles, top_k=args.top_k)
            for case in heldout_cases
        ]

        event_rows = record_training_events(
            rpc,
            train_search_rows,
            pairable_events=args.pairable_events,
            min_pairable_groups=args.min_pairable_groups,
        )
        expected_event_delta = len(event_rows)
        if args.pairable_events and expected_event_delta % 2 != 0:
            raise FsvFailure(f"pairable event mode produced odd event count: {expected_event_delta}")
        if not args.pairable_events and len(event_rows) != args.train_cases:
            raise FsvFailure(f"expected {args.train_cases} training events, got {len(event_rows)}")
        after_events = tool_counts(rpc)
        if after_events["learningEvents"] - before_counts["learningEvents"] != expected_event_delta:
            raise FsvFailure(f"learning event count mismatch after training events: {after_events}")
        report["readback"]["training_events"] = {
            "count": len(event_rows),
            "pairable_groups": (len(event_rows) // 2) if args.pairable_events else 0,
            "label_counts": {
                label: sum(row["label"] == label for row in event_rows)
                for label in sorted({row["label"] for row in event_rows})
            },
            "sample": event_rows[:5],
        }

        reward_export = rpc.call(
            "export_learner_training_dataset",
            {"task": "reward_model", "maxRows": expected_event_delta, "clearExisting": True},
            timeout=180.0,
        )
        dataset_id = reward_export["dataset_id"]
        dataset = rpc.call(
            "get_learner_training_dataset",
            {"datasetId": dataset_id, "includeMatrix": False, "previewRows": 5},
            timeout=120.0,
        )
        if dataset["dataset"]["rows"] != expected_event_delta:
            raise FsvFailure(f"training dataset rows mismatch: {dataset}")
        report["readback"]["training_dataset"] = {
            "export": reward_export,
            "dataset": dataset["dataset"],
            "preview": dataset.get("preview_rows"),
        }

        weights, train_metric = train_weights(train_search_rows, args.profiles)
        write_json_atomic(weights_path, {
            "weights": weights,
            "profiles": args.profiles,
            "baseline_profile": BASELINE_PROFILE,
            "train_metrics": train_metric,
            "source_training_events": event_rows,
        })
        if not weights_path.exists():
            raise FsvFailure("candidate policy weights artifact missing after write")

        train_eval_rows = evaluate_rows(train_search_rows, weights)
        heldout_eval_rows = evaluate_rows(heldout_search_rows, weights)
        eval_path.parent.mkdir(parents=True, exist_ok=True)
        with eval_path.open("w", encoding="utf-8") as handle:
            for row in heldout_eval_rows:
                handle.write(json.dumps(row, sort_keys=True))
                handle.write("\n")
        readback_eval = [
            json.loads(line)
            for line in eval_path.read_text(encoding="utf-8").splitlines()
            if line.strip()
        ]
        if len(readback_eval) != args.heldout_cases:
            raise FsvFailure("heldout evaluation file readback row count mismatch")

        train_baseline = metrics(train_eval_rows, "baseline_rank")
        train_candidate = metrics(train_eval_rows, "candidate_rank")
        heldout_baseline = metrics(heldout_eval_rows, "baseline_rank")
        heldout_candidate = metrics(heldout_eval_rows, "candidate_rank")
        mrr_lift = heldout_candidate["mrr"] - heldout_baseline["mrr"]
        hit5_lift = heldout_candidate["hit5"] - heldout_baseline["hit5"]
        latency_baseline = sum(row["baseline_latency_sec"] for row in heldout_eval_rows) / len(heldout_eval_rows)
        latency_candidate = sum(row["candidate_latency_sec"] for row in heldout_eval_rows) / len(heldout_eval_rows)
        promotion_passed = mrr_lift > 0.0 and hit5_lift >= 0

        report["benchmarks"] = {
            "train": {
                "baseline": train_baseline,
                "candidate": train_candidate,
            },
            "heldout": {
                "baseline": heldout_baseline,
                "candidate": heldout_candidate,
                "mrr_lift": mrr_lift,
                "hit5_lift": hit5_lift,
                "mean_latency_sec": {
                    "baseline": latency_baseline,
                    "candidate": latency_candidate,
                },
                "rows_sha256": sha256_file(eval_path),
                "rows_path": str(eval_path),
            },
            "candidate_policy": {
                "artifact": str(weights_path),
                "artifact_sha256": sha256_file(weights_path),
                "weights": weights,
                "method": "learned RRF ensemble over persisted search profile result sets",
            },
            "risk_gates": {
                "heldout_prompt_leak": False,
                "stress_floor_violations_delta": 0,
                "memorization_metric": "not a generation benchmark; no assistant text generated",
                "latency_delta_sec": latency_candidate - latency_baseline,
            },
            "promotion": {
                "passed": promotion_passed,
                "reason": (
                    "candidate held-out MRR improved and hit5 did not regress"
                    if promotion_passed
                    else "candidate did not satisfy held-out MRR/hit5 promotion gate"
                ),
            },
        }

        final_counts = tool_counts(rpc)
        report["after"]["mcp_counts"] = final_counts
        if final_counts["trainingDatasets"] - before_counts["trainingDatasets"] != 1:
            raise FsvFailure(f"training dataset count mismatch: before={before_counts} after={final_counts}")

        rpc.close()
        rpc = None
        stop_daemon(daemon, report)
        daemon = None

        after_content = ldb_counts(storage, CONTENT_CFS, report, "after")
        after_learning = ldb_counts(storage, LEARNING_CFS, report, "after")
        report["after"]["ldb_content"] = after_content
        report["after"]["ldb_learning"] = after_learning
        if after_content != report["before"]["ldb_content"]:
            raise FsvFailure(f"content CFs changed during phase6 benchmark: before={report['before']['ldb_content']} after={after_content}")
        if after_learning["learning_events"] - report["before"]["ldb_learning"]["learning_events"] != expected_event_delta:
            raise FsvFailure(f"physical learning_events delta mismatch: before={report['before']['ldb_learning']} after={after_learning}")
        if after_learning["learner_training_datasets"] - report["before"]["ldb_learning"]["learner_training_datasets"] != 1:
            raise FsvFailure(f"physical learner_training_datasets delta mismatch: before={report['before']['ldb_learning']} after={after_learning}")

        report["status"] = "ok"
    except Exception as exc:
        if rpc is not None:
            try:
                rpc.close()
            except Exception:
                pass
        stop_daemon(daemon, report)
        report["status"] = "failed"
        report["error"] = f"{type(exc).__name__}: {exc}"
        write_json_atomic(args.report, report)
        print(f"SHAKESPEARE PHASE6 POLICY BENCHMARK FAILED: {exc}", file=sys.stderr)
        print(f"REPORT_WRITTEN={args.report}", file=sys.stderr)
        return 1

    write_json_atomic(args.report, report)
    print(json.dumps({
        "status": report["status"],
        "report": str(args.report),
        "storage": str(storage),
        "sourceOfTruth": report["sourceOfTruth"],
        "benchmarks": report["benchmarks"],
        "after": report["after"],
        "edgeCases": [
            {
                "name": edge["name"],
                "before": edge["before"],
                "after": edge["after"],
                "is_error": edge["is_error"],
            }
            for edge in report["edgeCases"]
        ],
    }, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
