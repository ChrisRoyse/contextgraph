#!/usr/bin/env python3
"""Big Unlock Harness 5: learner training export FSV over Shakespeare events."""

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
    require_path,
    start_daemon,
    stop_daemon,
    training_dataset_count,
    utc_stamp,
    write_json_atomic,
)


QUERIES = [
    "Explain Macbeth's ambition using evidence from the play.",
    "Explain Hamlet's hesitation using a passage from the play.",
    "Explain Romeo and Juliet's conflict using a passage.",
    "Explain King Lear's authority and betrayal using the text.",
    "Explain time and beauty in the Sonnets.",
    "Explain the witches' role in Macbeth's choices.",
    "Explain Ophelia's distress in Hamlet.",
    "Explain Mercutio's wit in Romeo and Juliet.",
    "Explain Cordelia's honesty in King Lear.",
    "Explain how a Shakespeare sonnet treats mortality.",
]

OUTCOMES = [
    ("useful", 0.35, False, True),
    ("useful", 0.25, False, True),
    ("useful", 0.18, False, True),
    ("harmful", -0.30, True, False),
    ("harmful", -0.25, True, False),
    ("no_learning", 0.00, False, False),
    ("neutral", 0.04, False, False),
    ("useful", 0.30, False, True),
    ("harmful", -0.20, True, False),
    ("neutral", 0.12, False, True),
]


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
        default=DEFAULT_REPORT_DIR / "shakespeare_learner_training_fsv_latest.json",
    )
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=3324)
    parser.add_argument("--startup-timeout-sec", type=int, default=420)
    parser.add_argument("--request-timeout-sec", type=int, default=600)
    return parser


def export_and_read(rpc: Rpc, task: str, *, clear_existing: bool) -> dict[str, Any]:
    result = rpc.call(
        "export_learner_training_dataset",
        {"task": task, "maxRows": 10000, "clearExisting": clear_existing},
        timeout=240.0,
    )
    dataset = rpc.call(
        "get_learner_training_dataset",
        {"datasetId": result["dataset_id"], "includeMatrix": False, "previewRows": 3},
        timeout=120.0,
    )
    if dataset["dataset"]["rows"] != 10:
        raise FsvFailure(f"{task} dataset rows expected 10, got {dataset['dataset']['rows']}")
    if not dataset["preview_rows"]:
        raise FsvFailure(f"{task} dataset preview_rows is empty")
    if dataset["dataset"]["row_major_values"] != dataset["dataset"]["rows"] * dataset["dataset"]["cols"]:
        raise FsvFailure(f"{task} row_major_values does not match rows*cols")
    for preview in dataset["preview_rows"]:
        row = preview["row"]
        event_id = row.get("event_id")
        if not event_id:
            raise FsvFailure(f"{task} preview row missing event_id: {row}")
        event = rpc.call("get_learning_event", {"eventId": event_id, "includeText": False}, timeout=120.0)
        if not event.get("found"):
            raise FsvFailure(f"{task} preview event_id {event_id} was not readable from CF_LEARNING_EVENTS")
    return {"export": result, "dataset": dataset}


def main() -> int:
    args = build_parser().parse_args()
    run_dir = args.run_dir or (DEFAULT_RUN_ROOT / "learner_training" / utc_stamp())
    storage = run_dir / "shakespeare_training_db"
    report: dict[str, Any] = {
        "status": "running",
        "created_at": datetime.now(timezone.utc).isoformat(timespec="seconds"),
        "repo": str(Path(__file__).resolve().parents[2]),
        "run_dir": str(run_dir),
        "storage": str(storage),
        "sourceOfTruth": {
            "columnFamilies": ["learning_events", "learner_training_datasets", "learner_state_history", "learner_delta_log"],
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
        rows = load_go_emotions(args.go_emotions, limit=10)

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

        report["edgeCases"].append(
            edge_no_mutation(
                rpc,
                name="no_eligible_learning_events",
                tool="export_learner_training_dataset",
                args={"task": "reward_model", "maxRows": 10000, "clearExisting": False},
                count_fn=training_dataset_count,
            )
        )

        learner = rpc.call(
            "register_learner",
            {
                "handle": "bigunlock-training-consented-shakespeare",
                "consentState": "consented-training",
                "modalitiesEnabled": ["affect_text", "self_report"],
            },
            timeout=120.0,
        )
        learner_id = learner["learner_id"]

        event_ids: list[str] = []
        for idx, (query, outcome) in enumerate(zip(QUERIES, OUTCOMES)):
            label, utility_delta, correction_required, reuse_observed = outcome
            session_ts = 2_300_000_000 + idx
            stress = 0.20 + (0.05 * (idx % 4))
            if label == "harmful":
                stress = 0.68
            components = {
                "plasticityWindow": max(0.35, min(0.85, 0.72 + utility_delta * 0.20)),
                "hrvCoherence": max(0.30, min(0.85, 0.70 + utility_delta * 0.15)),
                "valence": max(-0.75, min(0.75, 0.10 + utility_delta)),
                "arousal": 0.25 + (0.05 * (idx % 3)),
                "stressFloor": stress,
                "kSleep": 1.0,
            }
            rpc.call(
                "record_session_observation",
                observation_args(learner_id, session_ts, str(rows[idx]["text"]), components),
                timeout=120.0,
            )
            search = rpc.call(
                "search_graph",
                {
                    "query": query,
                    "topK": 5,
                    "includeContent": True,
                    "includeProvenance": True,
                    "strategy": "multi_space",
                    "weightProfile": "semantic_search",
                    "stateConditionedProfile": True,
                    "learnerId": learner_id,
                    "learnerSessionTs": session_ts,
                },
                timeout=240.0,
            )
            result = search["results"][0]
            memory_id = extract_fingerprint_id(result)
            before_snapshot, after_snapshot = event_snapshots_from_search_result(result, utility_delta)
            trace_id = f"70000000-0000-4000-8000-{idx + 1:012d}"
            rpc.call(
                "compute_L",
                {
                    "learnerId": learner_id,
                    "sessionTs": session_ts,
                    "traceId": trace_id,
                    "predictedText": "The learner gives a partial Shakespeare explanation.",
                    "actualText": "The response grounds the explanation in the retrieved Shakespeare passage.",
                    "recentScores": [0.40, 0.55, max(0.0, min(1.0, 0.55 + utility_delta))],
                    "hrvCoherence": components["hrvCoherence"],
                    "panelAgreement": 0.70,
                    "contradiction": 0.10 if not correction_required else 0.35,
                    "components": components,
                    "persist": True,
                    "retrievalCorrect": label != "harmful",
                },
                timeout=120.0,
            )
            rpc.call(
                "record_learner_retrieval",
                {
                    "learnerId": learner_id,
                    "traceId": trace_id,
                    "ts": session_ts + 30,
                    "sessionTs": session_ts,
                    "correct": label != "harmful",
                    "score": max(0.0, min(1.0, 0.55 + utility_delta)),
                },
                timeout=120.0,
            )
            event = rpc.call(
                "record_learning_event",
                {
                    "memoryIds": [memory_id],
                    "sessionId": f"bigunlock-training-session-{idx}",
                    "responseId": f"bigunlock-training-response-{idx}",
                    "taskId": "shakespeare-learner-training",
                    "query": query,
                    "retrievedContext": str(result.get("content") or ""),
                    "assistantResponse": f"Controlled real-content tutoring response for query {idx}.",
                    "before": before_snapshot,
                    "after": after_snapshot,
                    "outcome": {
                        "label": label,
                        "utilityDelta": utility_delta,
                        "correctionRequired": correction_required,
                        "reuseObserved": reuse_observed,
                    },
                },
                timeout=120.0,
            )
            event_ids.append(event["event_id"])

        if learning_event_count(rpc) != 10:
            raise FsvFailure(f"expected 10 learning events, got {learning_event_count(rpc)}")
        learner_after_events = learner_counts(rpc)
        if learner_after_events.get("learner_delta_log") != 10:
            raise FsvFailure(f"expected 10 delta logs, got {learner_after_events.get('learner_delta_log')}")
        if learner_after_events.get("learner_retrieval_log") != 10:
            raise FsvFailure(f"expected 10 retrieval logs, got {learner_after_events.get('learner_retrieval_log')}")

        reward = export_and_read(rpc, "reward_model", clear_existing=True)
        reranker = export_and_read(rpc, "reranker", clear_existing=False)
        dataset_count = training_dataset_count(rpc)
        if dataset_count != 2:
            raise FsvFailure(f"expected 2 learner training datasets, got {dataset_count}")

        report["readback"]["events"] = {
            "count": learning_event_count(rpc),
            "event_ids": event_ids,
            "labels": [row[0] for row in OUTCOMES],
        }
        report["readback"]["learner_counts_after_events"] = learner_after_events
        report["readback"]["exports"] = {
            "reward_model": reward,
            "reranker": reranker,
            "dataset_count": dataset_count,
        }

        report["edgeCases"].append(
            edge_no_mutation(
                rpc,
                name="max_rows_zero",
                tool="export_learner_training_dataset",
                args={"task": "reward_model", "maxRows": 0, "clearExisting": False},
                count_fn=training_dataset_count,
            )
        )
        report["edgeCases"].append(
            edge_no_mutation(
                rpc,
                name="invalid_task_enum",
                tool="export_learner_training_dataset",
                args={"task": "not_a_task", "maxRows": 10, "clearExisting": False},
                count_fn=training_dataset_count,
            )
        )
        revoked = rpc.call(
            "register_learner",
            {
                "handle": "bigunlock-revoked-training-edge",
                "consentState": "revoked",
                "modalitiesEnabled": ["affect_text"],
            },
            timeout=120.0,
        )
        report["edgeCases"].append(
            edge_no_mutation(
                rpc,
                name="revoked_consent_observation_rejected",
                tool="record_session_observation",
                args=observation_args(
                    revoked["learner_id"],
                    2_399_999_999,
                    "This revoked learner observation must not become a training source.",
                    {
                        "plasticityWindow": 0.50,
                        "hrvCoherence": 0.50,
                        "valence": 0.0,
                        "arousal": 0.0,
                        "stressFloor": 0.20,
                        "kSleep": 1.0,
                    },
                ),
                count_fn=lambda inner_rpc: learner_counts(inner_rpc).get("fingerprints_learner", 0),
            )
        )

        report["after"]["mcp_counts"] = {
            "learning_events": learning_event_count(rpc),
            "learner_training_datasets": training_dataset_count(rpc),
            "learner": learner_counts(rpc),
        }

        rpc.close()
        rpc = None
        stop_daemon(daemon, report)
        daemon = None

        after_learning = ldb_counts(storage, LEARNING_CFS, report, "after_learning")
        after_learner = ldb_counts(storage, LEARNER_CFS, report, "after_learner")
        report["after"]["ldb_learning"] = after_learning
        report["after"]["ldb_learner"] = after_learner
        before_learning = report["before"]["ldb_learning"]
        expected_deltas = {
            "learning_events": 10,
            "learner_training_datasets": 2,
            "learner_delta_log": 10,
            "learner_m_per_trace": 10,
            "learner_retrieval_log": 10,
        }
        for cf, delta in expected_deltas.items():
            expected = before_learning.get(cf, 0) + delta
            if after_learning.get(cf) != expected:
                raise FsvFailure(f"physical {cf} expected {expected}, got {after_learning.get(cf)}")

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
        print(f"SHAKESPEARE LEARNER TRAINING FSV FAILED: {exc}", file=sys.stderr)
        print(f"REPORT_WRITTEN={args.report}", file=sys.stderr)
        return 1

    write_json_atomic(args.report, report)
    print(json.dumps({
        "status": "ok",
        "report": str(args.report),
        "storage": str(storage),
        "events": report["readback"]["events"],
        "exports": {
            task: {
                "dataset_id": value["export"]["dataset_id"],
                "rows": value["dataset"]["dataset"]["rows"],
                "cols": value["dataset"]["dataset"]["cols"],
                "source_counts": value["dataset"]["dataset"]["source_counts"],
            }
            for task, value in {
                "reward_model": report["readback"]["exports"]["reward_model"],
                "reranker": report["readback"]["exports"]["reranker"],
            }.items()
        },
        "physicalAfter": report["after"]["ldb_learning"],
        "edgeCases": [
            {"name": edge["name"], "before": edge["before"], "after": edge["after"], "is_error": edge["is_error"]}
            for edge in report["edgeCases"]
        ],
    }, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
