#!/usr/bin/env python3
"""Big Unlock Harness 2: state-conditioned Shakespeare retrieval FSV."""

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
    CONTENT_CFS,
    DEFAULT_GO_EMOTIONS,
    DEFAULT_MCP,
    DEFAULT_MODELS,
    DEFAULT_REPORT_DIR,
    DEFAULT_RUN_ROOT,
    DEFAULT_SHAKESPEARE_DB,
    FsvFailure,
    LEARNER_CFS,
    Rpc,
    copy_db,
    edge_no_mutation,
    ensure_clean_dir,
    extract_fingerprint_id,
    ldb_counts,
    learner_counts,
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


QUERY = "Explain Macbeth's ambition using a passage from the play."


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
        default=DEFAULT_REPORT_DIR / "shakespeare_state_conditioned_fsv_latest.json",
    )
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=3322)
    parser.add_argument("--startup-timeout-sec", type=int, default=420)
    parser.add_argument("--request-timeout-sec", type=int, default=600)
    return parser


def count_state_and_content(rpc: Rpc) -> dict[str, Any]:
    status = rpc.call("get_memetic_status", {}, timeout=120.0)
    return {
        "learner": learner_counts(rpc),
        "fingerprintCount": int(status["fingerprintCount"]),
    }


def result_ids(search_body: dict[str, Any]) -> list[str]:
    return [extract_fingerprint_id(item) for item in search_body.get("results", [])[:5]]


def selected_profile(search_body: dict[str, Any]) -> str:
    policy = search_body["searchParameters"]["stateConditionedProfile"]
    return str(policy["selected_profile"])


def main() -> int:
    args = build_parser().parse_args()
    run_dir = args.run_dir or (DEFAULT_RUN_ROOT / "state_conditioned_search" / utc_stamp())
    storage = run_dir / "shakespeare_learner_db"
    report: dict[str, Any] = {
        "status": "running",
        "created_at": datetime.now(timezone.utc).isoformat(timespec="seconds"),
        "repo": str(Path(__file__).resolve().parents[2]),
        "run_dir": str(run_dir),
        "storage": str(storage),
        "sourceOfTruth": {
            "columnFamilies": ["learner_state_history", "fingerprints", "content", "source_metadata"],
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
        rows = load_go_emotions(args.go_emotions, limit=3)

        report["before"]["ldb_content"] = ldb_counts(storage, CONTENT_CFS, report, "before_content")
        report["before"]["ldb_learner"] = ldb_counts(storage, LEARNER_CFS, report, "before_learner")
        if report["before"]["ldb_content"]["fingerprints"] <= 0:
            raise FsvFailure("source DB has no Shakespeare fingerprints")

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
                "handle": "bigunlock-state-conditioned-shakespeare",
                "consentState": "consented-local-first",
                "modalitiesEnabled": ["affect_text", "self_report"],
            },
            timeout=120.0,
        )
        learner_id = learner["learner_id"]

        states = [
            (
                "optimal",
                2_100_000_001,
                {
                    "plasticityWindow": 0.78,
                    "hrvCoherence": 0.76,
                    "valence": 0.25,
                    "arousal": 0.20,
                    "stressFloor": 0.18,
                    "kSleep": 1.0,
                },
                "semantic_search",
            ),
            (
                "stressed",
                2_100_000_002,
                {
                    "plasticityWindow": 0.50,
                    "hrvCoherence": 0.42,
                    "valence": 0.05,
                    "arousal": 0.30,
                    "stressFloor": 0.70,
                    "kSleep": 1.0,
                },
                "affect_repair",
            ),
            (
                "negative_valence",
                2_100_000_003,
                {
                    "plasticityWindow": 0.64,
                    "hrvCoherence": 0.70,
                    "valence": -0.55,
                    "arousal": 0.18,
                    "stressFloor": 0.24,
                    "kSleep": 1.0,
                },
                "affect_priming",
            ),
        ]

        for idx, (name, session_ts, components, _) in enumerate(states):
            rpc.call(
                "record_session_observation",
                observation_args(learner_id, session_ts, str(rows[idx]["text"]), components),
                timeout=120.0,
            )
            report["actions"].append(
                {"tool": "record_session_observation", "state": name, "session_ts": session_ts, "components": components}
            )

        policy_evidence = {}
        search_evidence = {}
        before_search_counts = count_state_and_content(rpc)
        for name, session_ts, components, expected_profile in states:
            policy = rpc.call(
                "resolve_learner_retrieval_policy",
                {
                    "learnerId": learner_id,
                    "sessionTs": session_ts,
                    "baseWeightProfile": "semantic_search",
                    "includeWeights": True,
                },
                timeout=120.0,
            )
            if policy["policy"]["selected_profile"] != expected_profile:
                raise FsvFailure(
                    f"{name} expected selected profile {expected_profile}, got {policy['policy']['selected_profile']}"
                )
            policy_evidence[name] = policy

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
                    "learnerSessionTs": session_ts,
                },
                timeout=240.0,
            )
            if selected_profile(search) != expected_profile:
                raise FsvFailure(
                    f"{name} search expected profile {expected_profile}, got {selected_profile(search)}"
                )
            ids = result_ids(search)
            if len(ids) != 5:
                raise FsvFailure(f"{name} search expected 5 results, got {len(ids)}")
            readbacks = readback_search_results(rpc, search["results"], sample=5)
            search_evidence[name] = {
                "session_ts": session_ts,
                "expected_profile": expected_profile,
                "selected_profile": selected_profile(search),
                "top5_ids": ids,
                "top_result": search["results"][0],
                "content_readback": readbacks,
            }

        after_search_counts = count_state_and_content(rpc)
        if after_search_counts != before_search_counts:
            raise FsvFailure(
                f"state-conditioned search mutated source counts: before={before_search_counts} after={after_search_counts}"
            )
        if search_evidence["optimal"]["top5_ids"] == search_evidence["stressed"]["top5_ids"]:
            raise FsvFailure("optimal and stressed top-5 rankings were identical; expected retrieval output to change")

        report["readback"]["policy"] = policy_evidence
        report["readback"]["search"] = search_evidence
        report["readback"]["no_mutation_during_search"] = {
            "before": before_search_counts,
            "after": after_search_counts,
        }

        report["edgeCases"].append(
            edge_no_mutation(
                rpc,
                name="missing_learner_id",
                tool="search_graph",
                args={
                    "query": QUERY,
                    "topK": 5,
                    "stateConditionedProfile": True,
                    "learnerSessionTs": states[0][1],
                },
                count_fn=count_state_and_content,
            )
        )
        report["edgeCases"].append(
            edge_no_mutation(
                rpc,
                name="missing_session_timestamp",
                tool="search_graph",
                args={
                    "query": QUERY,
                    "topK": 5,
                    "stateConditionedProfile": True,
                    "learnerId": learner_id,
                },
                count_fn=count_state_and_content,
            )
        )
        report["edgeCases"].append(
            edge_no_mutation(
                rpc,
                name="nonexistent_learner_state",
                tool="search_graph",
                args={
                    "query": QUERY,
                    "topK": 5,
                    "stateConditionedProfile": True,
                    "learnerId": learner_id,
                    "learnerSessionTs": 9_999_999_999,
                    "weightProfile": "semantic_search",
                },
                count_fn=count_state_and_content,
            )
        )
        report["edgeCases"].append(
            edge_no_mutation(
                rpc,
                name="custom_weights_with_state_conditioning",
                tool="search_graph",
                args={
                    "query": QUERY,
                    "topK": 5,
                    "stateConditionedProfile": True,
                    "learnerId": learner_id,
                    "learnerSessionTs": states[0][1],
                    "customWeights": {"E1": 1.0},
                },
                count_fn=count_state_and_content,
            )
        )

        report["after"]["mcp_counts"] = count_state_and_content(rpc)

        rpc.close()
        rpc = None
        stop_daemon(daemon, report)
        daemon = None

        report["after"]["ldb_content"] = ldb_counts(storage, CONTENT_CFS, report, "after_content")
        report["after"]["ldb_learner"] = ldb_counts(storage, LEARNER_CFS, report, "after_learner")
        if report["after"]["ldb_content"]["fingerprints"] != report["before"]["ldb_content"]["fingerprints"]:
            raise FsvFailure("physical content fingerprint count mutated during search FSV")
        if report["after"]["ldb_learner"]["learner_state_history"] != 3:
            raise FsvFailure(
                f"physical learner_state_history expected 3, got {report['after']['ldb_learner']['learner_state_history']}"
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
        print(f"STATE-CONDITIONED SHAKESPEARE FSV FAILED: {exc}", file=sys.stderr)
        print(f"REPORT_WRITTEN={args.report}", file=sys.stderr)
        return 1

    write_json_atomic(args.report, report)
    print(json.dumps({
        "status": "ok",
        "report": str(args.report),
        "storage": str(storage),
        "selected_profiles": {
            state: evidence["selected_profile"]
            for state, evidence in report["readback"]["search"].items()
        },
        "top5": {
            state: evidence["top5_ids"]
            for state, evidence in report["readback"]["search"].items()
        },
        "edgeCases": [
            {"name": edge["name"], "before": edge["before"], "after": edge["after"], "is_error": edge["is_error"]}
            for edge in report["edgeCases"]
        ],
        "physicalAfter": report["after"],
    }, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
