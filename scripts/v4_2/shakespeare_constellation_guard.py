#!/usr/bin/env python3
"""Big Unlock Phase 5: Shakespeare constellation guard FSV.

The guard is intentionally implemented as a harness over existing MCP tools:
candidate text is fingerprinted with `store_memory`, scored against the real
Sonnets constellation, checked against nearest corpus passages for memorization
risk, and then hard-deleted. Blocked candidates write correction-required
LearningEvent rows tied to real Shakespeare memory IDs.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
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
    edge_no_mutation,
    event_snapshots_from_search_result,
    extract_fingerprint_id,
    ldb_counts,
    path_state,
    readback_search_results,
    require_path,
    sha256_text,
    start_daemon,
    stop_daemon,
    utc_stamp,
    write_json_atomic,
)

SONNETS_CONSTELLATION_ID = "fd2cb68f-8a4e-42bf-98d8-ca4bfc0a732f"
STYLE_SCORE_THRESHOLD = 0.45
MEMORIZATION_E1_THRESHOLD = 0.96
QUOTE_RE = re.compile(
    r"shall i compare thee|to be or not to be|friends romans countrymen|"
    r"now is the winter of our discontent|all the world's a stage",
    re.IGNORECASE,
)
STYLE_WEIGHTS = {
    6: 0.15,   # E7 code/syntax structure
    8: 0.15,   # E9 HDC
    9: 0.15,   # E10 paraphrase
    11: 0.30,  # E12 ColBERT
    12: 0.25,  # E13 SPLADE
}


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mcp-binary", type=Path, default=DEFAULT_MCP)
    parser.add_argument("--models", type=Path, default=DEFAULT_MODELS)
    parser.add_argument("--source-db", type=Path, default=DEFAULT_SHAKESPEARE_DB)
    parser.add_argument("--run-dir", type=Path, default=None)
    parser.add_argument(
        "--report",
        type=Path,
        default=DEFAULT_REPORT_DIR / "shakespeare_constellation_guard_fsv_latest.json",
    )
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=3325)
    parser.add_argument("--startup-timeout-sec", type=int, default=420)
    parser.add_argument("--request-timeout-sec", type=int, default=600)
    return parser


def mcp_guard_counts(rpc: Rpc) -> dict[str, int]:
    status = rpc.call("get_memetic_status", {}, timeout=120.0)
    events = rpc.call("count_learning_events", {}, timeout=120.0)
    constellations = rpc.call("list_constellations", {"limit": 1}, timeout=120.0)
    return {
        "fingerprintCount": int(status["fingerprintCount"]),
        "learningEvents": int(events["count"]),
        "constellations": int(constellations["total"]),
    }


def style_score(per_embedder: list[Any]) -> float:
    if len(per_embedder) < 13:
        raise FsvFailure(f"score returned {len(per_embedder)} embedders, expected at least 13")
    total = 0.0
    for idx, weight in STYLE_WEIGHTS.items():
        total += weight * max(0.0, float(per_embedder[idx]))
    return round(total, 6)


def top_search(rpc: Rpc, text: str) -> dict[str, Any]:
    body = rpc.call(
        "search_graph",
        {
            "query": text,
            "topK": 3,
            "includeContent": True,
            "includeProvenance": True,
            "weightProfile": "semantic_search",
            "strategy": "multi_space",
        },
        timeout=180.0,
    )
    results = body.get("results")
    if not isinstance(results, list) or not results:
        raise FsvFailure(f"search_graph returned no nearest source for guard text: {body}")
    evidence = readback_search_results(rpc, results, sample=3)
    top = results[0]
    source_text = str(top.get("content") or "")
    return {
        "count": body.get("count"),
        "top": top,
        "top_id": extract_fingerprint_id(top),
        "top_e1": float(top.get("e1Score") or 0.0),
        "top_content_sha256": sha256_text(source_text),
        "top_content_prefix": source_text[:160],
        "content_readback": evidence,
    }


def decide(text: str, score: dict[str, Any], nearest: dict[str, Any]) -> dict[str, Any]:
    per = score.get("per_embedder_cosine")
    if not isinstance(per, list):
        raise FsvFailure(f"score_against_constellation returned no per_embedder_cosine: {score}")
    s_score = style_score(per)
    exact_quote = bool(QUOTE_RE.search(text))
    nearest_e1 = float(nearest["top_e1"])
    memorization_risk = nearest_e1 >= MEMORIZATION_E1_THRESHOLD or exact_quote
    style_pass = s_score >= STYLE_SCORE_THRESHOLD
    if memorization_risk:
        action = "abort"
    elif not style_pass:
        action = "revise"
    else:
        action = "accept"
    return {
        "action": action,
        "style_score": s_score,
        "style_pass": style_pass,
        "style_threshold": STYLE_SCORE_THRESHOLD,
        "nearest_e1": round(nearest_e1, 6),
        "memorization_risk": memorization_risk,
        "memorization_e1_threshold": MEMORIZATION_E1_THRESHOLD,
        "known_quote_pattern": exact_quote,
    }


def record_block_event(
    rpc: Rpc,
    *,
    candidate_name: str,
    candidate_text: str,
    nearest: dict[str, Any],
    decision: dict[str, Any],
) -> str:
    top = nearest["top"]
    memory_id = extract_fingerprint_id(top)
    before, after = event_snapshots_from_search_result(top, -0.25)
    event = rpc.call(
        "record_learning_event",
        {
            "memoryIds": [memory_id],
            "sessionId": "bigunlock-constellation-guard",
            "responseId": f"guard-{candidate_name}",
            "taskId": "runtime_constellation_guard",
            "query": candidate_text,
            "retrievedContext": str(top.get("content") or "")[:8000],
            "assistantResponse": candidate_text,
            "before": before,
            "after": after,
            "outcome": {
                "label": "harmful",
                "utilityDelta": -0.25,
                "correctionRequired": True,
                "reuseObserved": False,
            },
        },
        timeout=180.0,
    )
    event_id = event["event_id"]
    readback = rpc.call(
        "get_learning_event",
        {"eventId": event_id, "includeText": True, "includeSignals": True},
        timeout=120.0,
    )
    fetched = readback.get("event") or readback
    outcome = fetched.get("outcome") or {}
    if outcome.get("correction_required") is not True:
        raise FsvFailure(f"blocked guard event {event_id} did not read back correction_required=true")
    if str(outcome.get("label") or "").lower() != "harmful":
        raise FsvFailure(f"blocked guard event {event_id} label readback mismatch: {outcome}")
    if memory_id not in fetched.get("memory_ids", []):
        raise FsvFailure(f"blocked guard event {event_id} did not retain source memory ID {memory_id}")
    decision["learning_event_readback"] = {
        "event_id": event_id,
        "memory_ids": fetched.get("memory_ids", []),
        "outcome": outcome,
        "query_sha256": sha256_text(candidate_text),
    }
    return event_id


def score_candidate(
    rpc: Rpc,
    *,
    constellation_id: str,
    name: str,
    text: str,
    expected_action: str,
) -> dict[str, Any]:
    before_counts = mcp_guard_counts(rpc)
    nearest = top_search(rpc, text)
    store = rpc.call(
        "store_memory",
        {
            "content": text,
            "sessionId": "bigunlock-constellation-guard-probe",
            "importance": 0.25,
            "rationale": f"BigUnlock guard probe {name}",
        },
        timeout=180.0,
    )
    probe_id = store.get("fingerprintId")
    if not isinstance(probe_id, str) or not probe_id:
        raise FsvFailure(f"store_memory returned no probe fingerprint for {name}: {store}")

    stored_counts = mcp_guard_counts(rpc)
    if stored_counts["fingerprintCount"] != before_counts["fingerprintCount"] + 1:
        raise FsvFailure(f"probe store count mismatch for {name}: before={before_counts} stored={stored_counts}")

    score: dict[str, Any] | None = None
    decision: dict[str, Any] | None = None
    event_id: str | None = None
    cleanup: dict[str, Any] = {}
    try:
        readback = rpc.call(
            "get_memory_fingerprint",
            {"memoryId": probe_id, "includeContent": True, "includeVectorNorms": False},
            timeout=120.0,
        )
        if readback.get("content") != text:
            raise FsvFailure(f"probe content readback mismatch for {name}")

        score = rpc.call(
            "score_against_constellation",
            {"constellationId": constellation_id, "memoryId": probe_id},
            timeout=120.0,
        )
        if score.get("memory_id") != probe_id:
            raise FsvFailure(f"score memory_id mismatch for {name}: {score}")
        decision = decide(text, score, nearest)
        if decision["action"] != expected_action:
            raise FsvFailure(
                f"guard decision for {name} expected {expected_action}, got {decision['action']}; "
                f"decision={decision}"
            )
        if decision["action"] in {"revise", "abort"}:
            event_id = record_block_event(
                rpc,
                candidate_name=name,
                candidate_text=text,
                nearest=nearest,
                decision=decision,
            )
    finally:
        rpc.call(
            "forget_concept",
            {
                "node_id": probe_id,
                "soft_delete": False,
                "reason": f"bigunlock guard probe cleanup {name}",
            },
            timeout=120.0,
        )
        missing = rpc.call_expect_error(
            "get_memory_fingerprint",
            {"memoryId": probe_id, "includeContent": True},
            timeout=120.0,
        )
        cleanup = {
            "probe_id": probe_id,
            "hard_deleted": True,
            "read_after_delete_error": missing["result"]["content"][0]["text"],
        }

    after_counts = mcp_guard_counts(rpc)
    expected_event_delta = 1 if event_id else 0
    expected_after = dict(before_counts)
    expected_after["learningEvents"] += expected_event_delta
    if after_counts != expected_after:
        raise FsvFailure(
            f"guard candidate {name} final count mismatch: before={before_counts} "
            f"expected={expected_after} after={after_counts}"
        )

    return {
        "name": name,
        "text_sha256": sha256_text(text),
        "text_chars": len(text),
        "before_counts": before_counts,
        "stored_counts": stored_counts,
        "after_counts": after_counts,
        "nearest_source": nearest,
        "score": {
            "constellation_id": score["constellation_id"] if score else None,
            "combined_score": score["combined_score"] if score else None,
            "in_spread_p95": score["in_spread_p95"] if score else None,
            "per_embedder_cosine": score["per_embedder_cosine"] if score else None,
        },
        "decision": decision,
        "learning_event_id": event_id,
        "cleanup": cleanup,
    }


def main() -> int:
    args = build_parser().parse_args()
    run_dir = args.run_dir or (DEFAULT_RUN_ROOT / "constellation_guard" / utc_stamp())
    storage = run_dir / "shakespeare_guard_db"
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
                "embedding_registry",
                "constellations",
                "learning_events",
            ],
            "constellationId": SONNETS_CONSTELLATION_ID,
            "sourceDb": str(args.source_db),
        },
        "inputs": {
            "mcp_binary": path_state(args.mcp_binary),
            "models": path_state(args.models),
            "source_db": path_state(args.source_db),
        },
        "before": {},
        "actions": [],
        "readback": {},
        "edgeCases": [],
        "after": {},
        "commands": {},
    }

    daemon: subprocess.Popen[str] | None = None
    rpc: Rpc | None = None
    try:
        require_path(args.mcp_binary, "context-graph-mcp binary", file=True)
        require_path(args.models, "models directory", file=False)
        require_path(args.source_db, "verified Shakespeare DB", file=False)
        copy_db(args.source_db, storage)

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

        before_counts = mcp_guard_counts(rpc)
        report["before"]["mcp_counts"] = before_counts
        target_constellation_id = SONNETS_CONSTELLATION_ID
        constellation = rpc.call(
            "get_constellation",
            {"constellationId": target_constellation_id, "includeCentroids": False},
            timeout=120.0,
        )
        if not constellation.get("found"):
            raise FsvFailure(f"Sonnets constellation missing: {constellation}")
        if int(constellation.get("member_count") or 0) < 3:
            raise FsvFailure(f"Sonnets constellation has too few members: {constellation}")
        if len(constellation.get("per_embedder") or []) < 13:
            raise FsvFailure(f"Sonnets constellation missing per-embedder stats: {constellation}")
        report["readback"]["target_constellation"] = {
            "constellation_id": constellation["constellation_id"],
            "label": constellation["label"],
            "member_count": constellation["member_count"],
            "coherence": constellation["coherence"],
            "per_embedder_count": len(constellation["per_embedder"]),
        }

        candidates = [
            {
                "name": "modern_revision_required",
                "expected": "revise",
                "text": (
                    "The release checklist should pin the dependency versions, run the "
                    "database migration once, and reject any request that cannot prove "
                    "its output through a source-of-truth read."
                ),
            },
            {
                "name": "quote_leak_abort",
                "expected": "abort",
                "text": (
                    "Shall I compare thee to a summer's day? The tutoring reply should "
                    "continue by copying the familiar cadence instead of explaining the image."
                ),
            },
        ]
        for candidate in candidates:
            row = score_candidate(
                rpc,
                constellation_id=target_constellation_id,
                name=candidate["name"],
                text=candidate["text"],
                expected_action=candidate["expected"],
            )
            report["actions"].append(row)

        event_ids = [row["learning_event_id"] for row in report["actions"] if row["learning_event_id"]]
        if len(event_ids) != 2:
            raise FsvFailure(f"expected two blocked guard learning events, got {event_ids}")

        report["edgeCases"].append(
            edge_no_mutation(
                rpc,
                name="empty_candidate_store_rejected",
                tool="store_memory",
                args={"content": "", "sessionId": "bigunlock-guard-edge"},
                count_fn=mcp_guard_counts,
            )
        )
        report["edgeCases"].append(
            edge_no_mutation(
                rpc,
                name="missing_constellation_id_rejected",
                tool="score_against_constellation",
                args={"memoryId": report["actions"][0]["nearest_source"]["top_id"]},
                count_fn=mcp_guard_counts,
            )
        )
        report["edgeCases"].append(
            edge_no_mutation(
                rpc,
                name="missing_memory_id_rejected",
                tool="score_against_constellation",
                args={"constellationId": target_constellation_id},
                count_fn=mcp_guard_counts,
            )
        )
        report["edgeCases"].append(
            edge_no_mutation(
                rpc,
                name="hard_delete_missing_probe_rejected",
                tool="forget_concept",
                args={
                    "node_id": "99999999-9999-4999-8999-999999999999",
                    "soft_delete": False,
                    "reason": "bigunlock guard missing probe edge",
                },
                count_fn=mcp_guard_counts,
            )
        )

        final_counts = mcp_guard_counts(rpc)
        expected_final = dict(before_counts)
        expected_final["learningEvents"] += 2
        if final_counts != expected_final:
            raise FsvFailure(f"final MCP counts mismatch: expected={expected_final} got={final_counts}")
        report["after"]["mcp_counts"] = final_counts

        rpc.close()
        rpc = None
        stop_daemon(daemon, report)
        daemon = None

        after_content = ldb_counts(storage, CONTENT_CFS, report, "after")
        after_learning = ldb_counts(storage, LEARNING_CFS, report, "after")
        report["after"]["ldb_content"] = after_content
        report["after"]["ldb_learning"] = after_learning
        for cf in CONTENT_CFS:
            if after_content.get(cf) != report["before"]["ldb_content"].get(cf):
                raise FsvFailure(f"probe cleanup changed physical {cf}: before={report['before']['ldb_content']} after={after_content}")
        learning_before = report["before"]["ldb_learning"].get("learning_events", 0)
        if after_learning.get("learning_events", 0) != learning_before + 2:
            raise FsvFailure(
                f"physical learning_events expected {learning_before + 2}, got "
                f"{after_learning.get('learning_events')}"
            )
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
        print(f"SHAKESPEARE CONSTELLATION GUARD FSV FAILED: {exc}", file=sys.stderr)
        print(f"REPORT_WRITTEN={args.report}", file=sys.stderr)
        return 1

    write_json_atomic(args.report, report)
    print(json.dumps({
        "status": report["status"],
        "report": str(args.report),
        "storage": str(storage),
        "sourceOfTruth": report["sourceOfTruth"],
        "decisions": [
            {
                "name": row["name"],
                "action": row["decision"]["action"],
                "style_score": row["decision"]["style_score"],
                "nearest_e1": row["decision"]["nearest_e1"],
                "learning_event_id": row["learning_event_id"],
            }
            for row in report["actions"]
        ],
        "after": report["after"],
        "edgeCases": [
            {"name": edge["name"], "before": edge["before"], "after": edge["after"], "is_error": edge["is_error"]}
            for edge in report["edgeCases"]
        ],
    }, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
