#!/usr/bin/env python3
"""Big Unlock Harness 3/4: Shakespeare learning event and L/M trace FSV."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from scripts.v4_2.bigunlock_fsv_lib import (  # noqa: E402
    DEFAULT_GO_EMOTIONS,
    DEFAULT_MCP,
    DEFAULT_MODELS,
    DEFAULT_REPORT_DIR,
    DEFAULT_RUN_ROOT,
    DEFAULT_SHAKESPEARE_DB,
    FsvFailure,
    LEARNER_CFS,
    LEARNING_CFS,
    Rpc,
    copy_db,
    edge_no_mutation,
    ensure_clean_dir,
    event_snapshots_from_search_result,
    extract_fingerprint_id,
    ldb_counts,
    learner_counts,
    learning_event_count,
    load_go_emotions,
    observation_args,
    path_state,
    readback_search_results,
    require_path,
    start_daemon,
    stop_daemon,
    utc_stamp,
    write_json_atomic,
)


QUERY = "Explain Macbeth's ambition using evidence from the play."


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-db", type=Path, default=DEFAULT_SHAKESPEARE_DB)
    parser.add_argument("--mcp-binary", type=Path, default=DEFAULT_MCP)
    parser.add_argument("--models", type=Path, default=DEFAULT_MODELS)
    parser.add_argument("--go-emotions", type=Path, default=DEFAULT_GO_EMOTIONS)
    parser.add_argument("--run-dir", type=Path, default=None)
    parser.add_argument(
        "--report",
        type=Path,
        default=DEFAULT_REPORT_DIR / "shakespeare_learning_event_fsv_latest.json",
    )
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=3323)
    parser.add_argument("--startup-timeout-sec", type=int, default=420)
    parser.add_argument("--request-timeout-sec", type=int, default=600)
    return parser


def delta_m_counts(rpc: Rpc) -> dict[str, int]:
    counts = learner_counts(rpc)
    return {
        "learner_delta_log": counts.get("learner_delta_log", 0),
        "learner_m_per_trace": counts.get("learner_m_per_trace", 0),
    }


def learning_counts(rpc: Rpc) -> dict[str, int]:
    return {
        "learning_events": learning_event_count(rpc),
        **delta_m_counts(rpc),
    }


def scalar_delta(component: Any, field: str) -> float:
    if isinstance(component, dict):
        value = component.get(field)
        if value is None:
            raise FsvFailure(f"missing {field} in {component}")
        return float(value)
    raise FsvFailure(f"expected object for {field}, got {component}")


def main() -> int:
    args = build_parser().parse_args()
    run_dir = args.run_dir or (DEFAULT_RUN_ROOT / "learning_event" / utc_stamp())
    storage = run_dir / "shakespeare_learning_db"
    report: dict[str, Any] = {
        "status": "running",
        "created_at": datetime.now(timezone.utc).isoformat(timespec="seconds"),
        "repo": str(Path(__file__).resolve().parents[2]),
        "run_dir": str(run_dir),
        "storage": str(storage),
        "sourceOfTruth": {
            "columnFamilies": ["learning_events", "learner_delta_log", "learner_m_per_trace"],
            "sourceDb": str(args.source_db),
        },
        "inputs": {
            "source_db": path_state(args.source_db),
            "mcp_binary": path_state(args.mcp_binary),
            "models": path_state(args.models),
            "go_emotions": path_state(args.go_emotions),
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
        require_path(args.source_db, "verified Shakespeare RocksDB", file=False)
        require_path(args.mcp_binary, "context-graph-mcp binary", file=True)
        require_path(args.models, "models directory", file=False)
        require_path(args.go_emotions, "GoEmotions calibration sample", file=True)
        ensure_clean_dir(run_dir)
        copy_db(args.source_db, storage)
        rows = load_go_emotions(args.go_emotions, limit=2)

        report["before"]["ldb_learning"] = ldb_counts(storage, LEARNING_CFS, report, "before_learning")
        report["before"]["ldb_learner"] = ldb_counts(storage, LEARNER_CFS, report, "before_learner")

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

        learner = rpc.call(
            "register_learner",
            {
                "handle": "bigunlock-learning-event-shakespeare",
                "consentState": "consented-local-first",
                "modalitiesEnabled": ["affect_text", "self_report"],
            },
            timeout=120.0,
        )
        learner_id = learner["learner_id"]
        before_session = 2_200_000_001
        after_session = 2_200_000_002
        before_components = {
            "plasticityWindow": 0.64,
            "hrvCoherence": 0.66,
            "valence": 0.05,
            "arousal": 0.25,
            "stressFloor": 0.30,
            "kSleep": 1.0,
        }
        after_components = {
            "plasticityWindow": 0.76,
            "hrvCoherence": 0.72,
            "valence": 0.22,
            "arousal": 0.22,
            "stressFloor": 0.18,
            "kSleep": 1.0,
        }
        rpc.call(
            "record_session_observation",
            observation_args(learner_id, before_session, str(rows[0]["text"]), before_components),
            timeout=120.0,
        )
        rpc.call(
            "record_session_observation",
            observation_args(learner_id, after_session, str(rows[1]["text"]), after_components),
            timeout=120.0,
        )

        search = rpc.call(
            "search_graph",
            {
                "query": QUERY,
                "topK": 5,
                "includeContent": True,
                "includeProvenance": True,
                "strategy": "multi_space",
                "weightProfile": "semantic_search",
                "stateConditionedProfile": True,
                "learnerId": learner_id,
                "learnerSessionTs": before_session,
            },
            timeout=240.0,
        )
        results = search.get("results") or []
        if not results:
            raise FsvFailure("state-conditioned search returned no results")
        top_result = results[0]
        memory_id = extract_fingerprint_id(top_result)
        content_readback = readback_search_results(rpc, [top_result], sample=1)[0]
        retrieved_context = str(top_result.get("content") or content_readback["content_prefix"])

        trace_id = "77777777-7777-4777-8777-777777777777"
        l_result = rpc.call(
            "compute_L",
            {
                "learnerId": learner_id,
                "sessionTs": after_session,
                "traceId": trace_id,
                "predictedText": "Macbeth wants honor but hesitates before harming Duncan.",
                "actualText": "Macbeth's ambition grows as prophecy and persuasion lead him toward Duncan's murder.",
                "recentScores": [0.45, 0.60, 0.72],
                "hrvCoherence": after_components["hrvCoherence"],
                "panelAgreement": 0.74,
                "contradiction": 0.10,
                "components": after_components,
                "persist": True,
                "retrievalCorrect": True,
            },
            timeout=120.0,
        )
        comp = l_result["computation"]
        delta_s = scalar_delta(comp["delta_s"], "delta_s")
        delta_c = scalar_delta(comp["delta_c"], "delta_c")
        delta_e = scalar_delta(comp["delta_e"], "delta_e")
        expected_l = delta_s * delta_c * delta_e
        if abs(float(comp["l"]) - expected_l) > 1e-5:
            raise FsvFailure(f"L mismatch: got {comp['l']} expected {expected_l}")
        m_trace = rpc.call("get_learner_M", {"learnerId": learner_id, "traceId": trace_id}, timeout=120.0)
        if not m_trace.get("found"):
            raise FsvFailure("get_learner_M did not find persisted trace")
        next_review = rpc.call("next_review_for_trace", {"learnerId": learner_id, "traceId": trace_id}, timeout=120.0)
        state_with_delta = rpc.call(
            "get_learner_state",
            {"learnerId": learner_id, "sessionTs": after_session, "traceId": trace_id, "includeVectors": True},
            timeout=120.0,
        )
        if state_with_delta["session"]["delta_log"] is None:
            raise FsvFailure("get_learner_state readback did not include delta_log")

        before_snapshot, after_snapshot = event_snapshots_from_search_result(top_result, 0.25)
        event = rpc.call(
            "record_learning_event",
            {
                "memoryIds": [memory_id],
                "sessionId": "bigunlock-shakespeare-tutor-session",
                "responseId": "bigunlock-response-macbeth-ambition",
                "taskId": "macbeth-ambition",
                "query": QUERY,
                "retrievedContext": retrieved_context,
                "assistantResponse": (
                    "Macbeth's ambition turns from possibility into action when prophecy, "
                    "private desire, and persuasion align around Duncan's murder."
                ),
                "before": before_snapshot,
                "after": after_snapshot,
                "outcome": {
                    "label": "useful",
                    "utilityDelta": 0.25,
                    "correctionRequired": False,
                    "reuseObserved": True,
                },
            },
            timeout=120.0,
        )
        event_id = event["event_id"]
        event_readback = rpc.call(
            "get_learning_event",
            {"eventId": event_id, "includeText": True, "includeSignals": True},
            timeout=120.0,
        )
        if event_readback["memory_ids"] != [memory_id]:
            raise FsvFailure("learning event memory id readback mismatch")
        if event_readback["query"] != QUERY:
            raise FsvFailure("learning event query text readback mismatch")

        report["actions"] = [
            {"tool": "record_session_observation", "session_ts": before_session},
            {"tool": "record_session_observation", "session_ts": after_session},
            {"tool": "search_graph", "memory_id": memory_id},
            {"tool": "compute_L", "trace_id": trace_id},
            {"tool": "record_learning_event", "event_id": event_id},
        ]
        report["readback"] = {
            "search": {"selected_profile": search["searchParameters"]["stateConditionedProfile"]["selected_profile"], "top_memory_id": memory_id, "content": content_readback},
            "compute_L": {"response": l_result, "expected_l": expected_l},
            "m_trace": m_trace,
            "next_review": next_review,
            "learner_state_with_delta": state_with_delta,
            "learning_event": event_readback,
        }

        report["edgeCases"].append(
            edge_no_mutation(
                rpc,
                name="malformed_topic_profile",
                tool="record_learning_event",
                args={
                    "before": {**before_snapshot, "topicProfile": before_snapshot["topicProfile"][:-1]},
                    "after": after_snapshot,
                    "outcome": {"label": "useful"},
                },
                count_fn=learning_event_count,
            )
        )
        report["edgeCases"].append(
            edge_no_mutation(
                rpc,
                name="malformed_cross_correlations",
                tool="record_learning_event",
                args={
                    "before": {**before_snapshot, "crossCorrelations": before_snapshot["crossCorrelations"][:-1]},
                    "after": after_snapshot,
                    "outcome": {"label": "useful"},
                },
                count_fn=learning_event_count,
            )
        )
        report["edgeCases"].append(
            edge_no_mutation(
                rpc,
                name="invalid_outcome_label",
                tool="record_learning_event",
                args={"before": before_snapshot, "after": after_snapshot, "outcome": {"label": "great"}},
                count_fn=learning_event_count,
            )
        )
        report["edgeCases"].append(
            edge_no_mutation(
                rpc,
                name="too_many_memory_ids",
                tool="record_learning_event",
                args={
                    "memoryIds": [memory_id] * 1025,
                    "before": before_snapshot,
                    "after": after_snapshot,
                    "outcome": {"label": "useful"},
                },
                count_fn=learning_event_count,
                timeout=240.0,
            )
        )
        report["edgeCases"].append(
            edge_no_mutation(
                rpc,
                name="compute_l_missing_components",
                tool="compute_L",
                args={
                    "learnerId": learner_id,
                    "sessionTs": after_session + 10,
                    "traceId": "88888888-8888-4888-8888-888888888888",
                    "persist": True,
                },
                count_fn=delta_m_counts,
            )
        )
        bad_components = dict(after_components)
        bad_components["kSleep"] = 6.0
        report["edgeCases"].append(
            edge_no_mutation(
                rpc,
                name="compute_l_k_sleep_out_of_range",
                tool="compute_L",
                args={
                    "learnerId": learner_id,
                    "sessionTs": after_session + 11,
                    "traceId": "99999999-9999-4999-8999-999999999999",
                    "components": bad_components,
                    "persist": True,
                },
                count_fn=delta_m_counts,
            )
        )

        report["after"]["mcp_counts"] = learning_counts(rpc)

        rpc.close()
        rpc = None
        stop_daemon(daemon, report)
        daemon = None

        after_learning_ldb = ldb_counts(storage, LEARNING_CFS, report, "after_learning")
        report["after"]["ldb_learning"] = after_learning_ldb
        before_learning_ldb = report["before"]["ldb_learning"]
        for cf in ["learning_events", "learner_delta_log", "learner_m_per_trace"]:
            expected = before_learning_ldb.get(cf, 0) + 1
            if after_learning_ldb.get(cf) != expected:
                raise FsvFailure(f"physical {cf} expected {expected}, got {after_learning_ldb.get(cf)}")

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
        print(f"SHAKESPEARE LEARNING EVENT FSV FAILED: {exc}", file=sys.stderr)
        print(f"REPORT_WRITTEN={args.report}", file=sys.stderr)
        return 1

    write_json_atomic(args.report, report)
    print(json.dumps({
        "status": "ok",
        "report": str(args.report),
        "storage": str(storage),
        "event_id": report["readback"]["learning_event"]["event_id"],
        "memory_ids": report["readback"]["learning_event"]["memory_ids"],
        "l": report["readback"]["compute_L"]["response"]["computation"]["l"],
        "m_trace": report["readback"]["m_trace"],
        "physicalAfter": report["after"]["ldb_learning"],
        "edgeCases": [
            {"name": edge["name"], "before": edge["before"], "after": edge["after"], "is_error": edge["is_error"]}
            for edge in report["edgeCases"]
        ],
    }, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
