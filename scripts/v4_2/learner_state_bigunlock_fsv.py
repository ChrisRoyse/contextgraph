#!/usr/bin/env python3
"""Big Unlock Harness 1: learner-state full-state verification.

Happy path uses real GoEmotions calibration rows as E17 affect-text
observations. Negative cases are controlled malformed requests that must fail
without mutating learner-state column families.
"""

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
    DEFAULT_CALIBRATION,
    DEFAULT_GO_EMOTIONS,
    DEFAULT_MCP,
    DEFAULT_MODELS,
    DEFAULT_REPORT_DIR,
    DEFAULT_RUN_ROOT,
    FsvFailure,
    LEARNER_CFS,
    Rpc,
    components_from_go_emotions,
    edge_no_mutation,
    ensure_clean_dir,
    ldb_counts,
    learner_counts,
    load_go_emotions,
    observation_args,
    path_state,
    require_path,
    start_daemon,
    stop_daemon,
    utc_stamp,
    write_json_atomic,
)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mcp-binary", type=Path, default=DEFAULT_MCP)
    parser.add_argument("--models", type=Path, default=DEFAULT_MODELS)
    parser.add_argument("--calibration-root", type=Path, default=DEFAULT_CALIBRATION)
    parser.add_argument("--go-emotions", type=Path, default=DEFAULT_GO_EMOTIONS)
    parser.add_argument("--run-dir", type=Path, default=None)
    parser.add_argument(
        "--report",
        type=Path,
        default=DEFAULT_REPORT_DIR / "learner_state_bigunlock_fsv_latest.json",
    )
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=3321)
    parser.add_argument("--startup-timeout-sec", type=int, default=420)
    parser.add_argument("--request-timeout-sec", type=int, default=600)
    return parser


def count_subset(rpc: Rpc) -> dict[str, int]:
    counts = learner_counts(rpc)
    return {
        "learner_profile": counts.get("learner_profile", 0),
        "fingerprints_learner": counts.get("fingerprints_learner", 0),
        "learner_state_history": counts.get("learner_state_history", 0),
        "learner_constellations": counts.get("learner_constellations", 0),
        "learner_audit": counts.get("learner_audit", 0),
    }


def assert_delta(before: dict[str, int], after: dict[str, int], expected: dict[str, int]) -> None:
    for key, delta in expected.items():
        actual = after.get(key, 0) - before.get(key, 0)
        if actual != delta:
            raise FsvFailure(f"{key} delta expected {delta}, got {actual}; before={before} after={after}")


def main() -> int:
    args = build_parser().parse_args()
    run_dir = args.run_dir or (DEFAULT_RUN_ROOT / "learner_state" / utc_stamp())
    storage = run_dir / "rocksdb"
    report: dict[str, Any] = {
        "status": "running",
        "created_at": datetime.now(timezone.utc).isoformat(timespec="seconds"),
        "repo": str(Path(__file__).resolve().parents[2]),
        "run_dir": str(run_dir),
        "storage": str(storage),
        "sourceOfTruth": {
            "columnFamilies": [
                "learner_profile",
                "fingerprints_learner",
                "learner_state_history",
                "learner_constellations",
                "learner_audit",
            ],
            "rawData": str(args.go_emotions),
        },
        "inputs": {
            "mcp_binary": path_state(args.mcp_binary),
            "models": path_state(args.models),
            "calibration_root": path_state(args.calibration_root),
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
        require_path(args.mcp_binary, "context-graph-mcp binary", file=True)
        require_path(args.models, "models directory", file=False)
        require_path(args.calibration_root, "calibration root", file=False)
        require_path(args.go_emotions, "GoEmotions calibration sample", file=True)
        ensure_clean_dir(run_dir)
        storage.mkdir(parents=True, exist_ok=False)

        rows = load_go_emotions(args.go_emotions, limit=5)
        report["before"]["ldb"] = ldb_counts(storage, LEARNER_CFS, report, "before")

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

        capability = rpc.call("get_capability_matrix", {"includeToolSchemas": True}, timeout=120.0)
        if capability["embedders"]["contentCount"] != 14:
            raise FsvFailure("capability matrix did not report 14 content embedders")
        if capability["embedders"]["learnerStateCount"] != 7:
            raise FsvFailure("capability matrix did not report 7 learner-state embedders")
        if capability["embedders"]["totalSlots"] != 21:
            raise FsvFailure("capability matrix did not report 21 total slots")
        required_tools = {
            "store_memories",
            "register_learner",
            "record_session_observation",
            "compile_learner_constellation",
            "resolve_learner_retrieval_policy",
        }
        missing_tools = required_tools - set(capability["mcp"]["toolNames"])
        if missing_tools:
            raise FsvFailure(f"capability matrix missing tools: {sorted(missing_tools)}")
        report["readback"]["capability_matrix"] = {
            "toolCount": capability["mcp"]["toolCount"],
            "contentCount": capability["embedders"]["contentCount"],
            "learnerStateCount": capability["embedders"]["learnerStateCount"],
            "totalSlots": capability["embedders"]["totalSlots"],
        }

        preflight = rpc.call(
            "preflight_learner_assets",
            {
                "modelsRoot": str(args.models),
                "calibrationRoot": str(args.calibration_root),
                "allowMissing": False,
            },
            timeout=120.0,
        )
        if not preflight.get("ready"):
            raise FsvFailure(f"learner asset preflight returned not ready: {preflight}")
        report["readback"]["preflight"] = {
            "ready": preflight["ready"],
            "missing_count": preflight["missing_count"],
            "source_of_truth": preflight["source_of_truth"],
        }

        before_counts = count_subset(rpc)
        report["before"]["mcp_counts"] = before_counts
        learner = rpc.call(
            "register_learner",
            {
                "handle": "bigunlock-real-goemotions-learner",
                "consentState": "consented-local-first",
                "modalitiesEnabled": ["affect_text", "self_report"],
            },
            timeout=120.0,
        )
        learner_id = learner["learner_id"]
        report["actions"].append({"tool": "register_learner", "learner_id": learner_id})
        after_register = count_subset(rpc)
        assert_delta(before_counts, after_register, {"learner_profile": 1, "learner_audit": 1})

        session_ts_list: list[int] = []
        observation_readbacks = []
        for idx, row in enumerate(rows):
            session_ts = 2_000_000_000 + idx
            session_ts_list.append(session_ts)
            components = components_from_go_emotions(row)
            raw_text = str(row["text"])
            rpc.call(
                "record_session_observation",
                observation_args(learner_id, session_ts, raw_text, components),
                timeout=120.0,
            )
            state = rpc.call(
                "get_learner_state",
                {"learnerId": learner_id, "sessionTs": session_ts, "includeVectors": True},
                timeout=120.0,
            )
            read_components = state["session"]["state_vector"]["components"]
            if abs(read_components["stress_floor"] - components["stressFloor"]) > 1e-5:
                raise FsvFailure(f"stress floor readback mismatch for session {session_ts}")
            observation_readbacks.append(
                {
                    "session_ts": session_ts,
                    "source_goemotions_id": row.get("id"),
                    "labels": row.get("label_names"),
                    "raw_sha256": __import__("hashlib").sha256(raw_text.encode("utf-8")).hexdigest(),
                    "components": read_components,
                    "state_values": state["session"]["state_vector"]["values"],
                }
            )

        after_observations = count_subset(rpc)
        assert_delta(
            after_register,
            after_observations,
            {"fingerprints_learner": 5, "learner_state_history": 5, "learner_audit": 5},
        )
        report["readback"]["observations"] = observation_readbacks

        constellation = rpc.call(
            "compile_learner_constellation",
            {
                "learnerId": learner_id,
                "sessionTsList": session_ts_list,
                "label": "bigunlock-goemotions-regulated-baseline",
            },
            timeout=120.0,
        )
        if constellation["constellation"]["sample_count"] != 5:
            raise FsvFailure("learner constellation sample_count did not equal 5")
        after_constellation = count_subset(rpc)
        assert_delta(after_observations, after_constellation, {"learner_constellations": 1, "learner_audit": 1})
        report["readback"]["learner_constellation"] = constellation["constellation"]

        report["edgeCases"].append(
            edge_no_mutation(
                rpc,
                name="invalid_learner_uuid",
                tool="record_session_observation",
                args={
                    "learnerId": "not-a-uuid",
                    "sessionTs": 9,
                    "components": components_from_go_emotions(rows[0]),
                },
                count_fn=count_subset,
            )
        )
        report["edgeCases"].append(
            edge_no_mutation(
                rpc,
                name="missing_required_components",
                tool="record_session_observation",
                args={
                    "learnerId": learner_id,
                    "sessionTs": 9_000_000_001,
                    "modality": "affect_text",
                    "rawText": "This must fail because components are absent.",
                },
                count_fn=count_subset,
            )
        )
        bad_components = dict(components_from_go_emotions(rows[1]))
        bad_components["stressFloor"] = 1.2
        report["edgeCases"].append(
            edge_no_mutation(
                rpc,
                name="out_of_range_stress_floor",
                tool="record_session_observation",
                args=observation_args(
                    learner_id,
                    9_000_000_002,
                    "This must fail because stressFloor is outside [0, 1].",
                    bad_components,
                ),
                count_fn=count_subset,
            )
        )
        report["edgeCases"].append(
            edge_no_mutation(
                rpc,
                name="unsupported_modality",
                tool="record_session_observation",
                args={
                    "learnerId": learner_id,
                    "sessionTs": 9_000_000_003,
                    "modality": "unsupported_modality",
                    "rawText": "Unsupported modality must fail.",
                    "components": components_from_go_emotions(rows[2]),
                },
                count_fn=count_subset,
            )
        )
        report["edgeCases"].append(
            edge_no_mutation(
                rpc,
                name="empty_session_list_for_constellation",
                tool="compile_learner_constellation",
                args={"learnerId": learner_id, "sessionTsList": [], "label": "must-fail"},
                count_fn=count_subset,
            )
        )

        final_counts = count_subset(rpc)
        expected = {
            "learner_profile": 1,
            "fingerprints_learner": 5,
            "learner_state_history": 5,
            "learner_constellations": 1,
            "learner_audit": 7,
        }
        for key, value in expected.items():
            if final_counts.get(key) != value:
                raise FsvFailure(f"final MCP {key} expected {value}, got {final_counts.get(key)}")
        report["after"]["mcp_counts"] = final_counts

        rpc.close()
        rpc = None
        stop_daemon(daemon, report)
        daemon = None

        after_ldb = ldb_counts(storage, LEARNER_CFS, report, "after")
        report["after"]["ldb"] = after_ldb
        for key, value in expected.items():
            if after_ldb.get(key) != value:
                raise FsvFailure(f"physical ldb {key} expected {value}, got {after_ldb.get(key)}")

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
        print(f"LEARNER STATE FSV FAILED: {exc}", file=sys.stderr)
        print(f"REPORT_WRITTEN={args.report}", file=sys.stderr)
        return 1

    write_json_atomic(args.report, report)
    print(json.dumps({
        "status": report["status"],
        "report": str(args.report),
        "storage": str(storage),
        "sourceOfTruth": report["sourceOfTruth"],
        "after": report["after"],
        "edgeCases": [
            {"name": edge["name"], "before": edge["before"], "after": edge["after"], "is_error": edge["is_error"]}
            for edge in report["edgeCases"]
        ],
    }, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
