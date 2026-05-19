"""FSV and guard helpers for TASK-PY-G-089 per-slot calibration."""

from __future__ import annotations

import math
from datetime import datetime, timezone
from typing import Any, Callable

import numpy as np

from .python_real_corpus_constellation_smoke import finite_float, write_jsonl
from .smoke_intelligence_matrices import dump_json, iter_jsonl, read_json, sha256_file, tree_state, utc_now


EPS = 1.0e-9


def parse_utc(value: str) -> datetime:
    parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    if parsed.tzinfo is None:
        parsed = parsed.replace(tzinfo=timezone.utc)
    return parsed.astimezone(timezone.utc)


def evaluate_guard(observation: dict[str, float], calibrations: list[dict[str, Any]], now: datetime) -> dict[str, Any]:
    by_slot = {str(row["embedder_id"]): row for row in calibrations}
    violations = []
    normalized = []
    for embedder, row in sorted(by_slot.items()):
        if row.get("valid") is not True:
            violations.append({"slot_id": embedder, "reason": "CALIBRATION_INVALID"})
            continue
        if parse_utc(str(row["stale_after_utc"])) <= now:
            violations.append({"slot_id": embedder, "reason": "CALIBRATION_STALE"})
            continue
        if embedder not in observation:
            violations.append({"slot_id": embedder, "reason": "MISSING_SLOT_OBSERVATION"})
            continue
        score = float(observation[embedder])
        if not math.isfinite(score):
            violations.append({"slot_id": embedder, "reason": "NONFINITE_SLOT_SCORE"})
            continue
        tau = float(row["thresholds"]["ood_tau"])
        normalized.append(score / max(EPS, tau))
        if score > tau:
            violations.append({"slot_id": embedder, "reason": "SLOT_OOD_TAU_EXCEEDED", "score": finite_float(score), "tau": finite_float(tau)})
    aggregate_mean = finite_float(float(np.mean(normalized))) if normalized else 1.0
    return {
        "approved": not violations,
        "verdict": "Pass" if not violations else "GuardRejected",
        "violations": violations,
        "aggregate_mean_normalized_score": aggregate_mean,
        "aggregate_mean_would_pass": aggregate_mean <= 1.0,
    }


def edge_cases(global_rows: list[dict[str, Any]], *, require: Callable[[bool, str, str | None], None]) -> list[dict[str, Any]]:
    valid = [row for row in global_rows if row.get("valid") is True]
    require(len(valid) == len(global_rows), "MEJEPA_PER_SLOT_CALIBRATION_GLOBAL_SLOT_INVALID", None)
    now = datetime.now(timezone.utc)
    nominal = {row["embedder_id"]: float(row["reference_good_score"]) for row in valid}
    cases = []
    accepted = evaluate_guard(nominal, valid, now)
    cases.append({"name": "nominal_all_slots_passes", "passes": accepted["approved"] is True, "guard_decision": accepted})
    missing = dict(nominal)
    missing.pop(valid[0]["embedder_id"])
    rejected = evaluate_guard(missing, valid, now)
    cases.append({"name": "missing_required_slot_rejects", "passes": rejected["approved"] is False and rejected["violations"][0]["reason"] == "MISSING_SLOT_OBSERVATION", "guard_decision": rejected})
    stale_rows = [dict(row, stale_after_utc="2000-01-01T00:00:00+00:00") for row in valid]
    stale = evaluate_guard(nominal, stale_rows, now)
    cases.append({"name": "stale_calibration_rejects", "passes": stale["approved"] is False and stale["violations"][0]["reason"] == "CALIBRATION_STALE", "guard_decision": stale})
    bad = dict(nominal)
    bad_slot = valid[0]["embedder_id"]
    bad[bad_slot] = float(valid[0]["thresholds"]["ood_tau"]) * 1.25
    bad_result = evaluate_guard(bad, valid, now)
    cases.append({"name": "one_bad_slot_rejects_even_when_aggregate_passes", "passes": bad_result["approved"] is False and bad_result["aggregate_mean_would_pass"] is True, "bad_slot": bad_slot, "guard_decision": bad_result})
    nonfinite = dict(nominal)
    nonfinite[bad_slot] = float("nan")
    nonfinite_result = evaluate_guard(nonfinite, valid, now)
    cases.append({"name": "nonfinite_slot_score_rejects", "passes": nonfinite_result["approved"] is False and nonfinite_result["violations"][0]["reason"] == "NONFINITE_SLOT_SCORE", "guard_decision": nonfinite_result})
    return cases


def build_policy(
    required: tuple[str, ...],
    global_rows: list[dict[str, Any]],
    pair_rows: list[dict[str, Any]],
    created_at: str,
    stale_after: str,
    cases: list[dict[str, Any]],
    *,
    schema_version: int,
    task_id: str,
    formula_version: str,
) -> dict[str, Any]:
    valid_slots = [row for row in global_rows if row.get("valid") is True]
    return {
        "schema_version": schema_version,
        "task_id": task_id,
        "artifact_kind": "python_per_slot_ood_guard_policy",
        "formula_version": formula_version,
        "created_at_utc": created_at,
        "stale_after_utc": stale_after,
        "required_slots": list(required),
        "strict_conjunction": True,
        "aggregate_score_never_overrides_slot_violation": True,
        "reject_missing_or_stale_calibration": True,
        "global_slot_calibration_ready": len(valid_slots) == len(required),
        "pair_calibration_rows": len(pair_rows),
        "ship_gate_countable": False,
        "ship_gate_blockers_before_counting": ["rerun #127 under per-slot guard", "rerun #129 under per-slot guard", "rerun #130 under per-slot guard"],
        "reality_prediction_extension": {
            "field": "per_slot_ood_reasons",
            "status": "artifact_schema_materialized_for_next_live_Rust_surface",
            "reason_shape": {"slot_id": "e1", "reason": "SLOT_OOD_TAU_EXCEEDED", "score": 0.0, "tau": 0.0},
        },
        "edge_case_summary": {case["name"]: case["passes"] for case in cases},
    }


def write_outputs(args: Any, global_rows: list[dict[str, Any]], cell_rows: list[dict[str, Any]], pair_rows: list[dict[str, Any]], policy: dict[str, Any], report: dict[str, Any]) -> None:
    write_jsonl(args.output_root / "per_slot_calibration.jsonl", global_rows)
    write_jsonl(args.output_root / "per_cell_slot_calibration.jsonl", cell_rows)
    write_jsonl(args.output_root / "per_pair_calibration.jsonl", pair_rows)
    dump_json(args.output_root / "ood_guard_policy.json", policy)
    dump_json(args.output_root / "per_slot_calibration_report.json", report)


def build_fsv(
    args: Any,
    before: dict[str, Any],
    cases: list[dict[str, Any]],
    *,
    expected_active_embedders: tuple[str, ...],
    schema_version: int,
    artifact_kind: str,
    formula_version: str,
) -> dict[str, Any]:
    paths = {
        "per_slot_calibration": args.output_root / "per_slot_calibration.jsonl",
        "per_cell_slot_calibration": args.output_root / "per_cell_slot_calibration.jsonl",
        "per_pair_calibration": args.output_root / "per_pair_calibration.jsonl",
        "ood_guard_policy": args.output_root / "ood_guard_policy.json",
        "report": args.output_root / "per_slot_calibration_report.json",
    }
    readback = {
        "per_slot_rows": sum(1 for _ in iter_jsonl(paths["per_slot_calibration"])),
        "per_cell_slot_rows": sum(1 for _ in iter_jsonl(paths["per_cell_slot_calibration"])),
        "per_pair_rows": sum(1 for _ in iter_jsonl(paths["per_pair_calibration"])),
        "policy": read_json(paths["ood_guard_policy"]),
        "report": read_json(paths["report"]),
        "stable_hashes": {key: sha256_file(path) for key, path in paths.items()},
    }
    independent = {
        "per_slot_rows": readback["per_slot_rows"],
        "per_pair_rows": readback["per_pair_rows"],
        "global_slot_calibration_ready": readback["policy"]["global_slot_calibration_ready"],
        "flat_vector_concat_used": readback["report"].get("flat_vector_concat_used"),
    }
    external_edges = {"edge_cases": cases, "all_edge_cases_pass": all(case.get("passes") is True for case in cases)}
    dump_json(args.fsv_root / "independent_readback.json", independent)
    dump_json(args.fsv_root / "external_edge_case_readback.json", external_edges)
    passes = (
        readback["report"].get("passes") is True
        and readback["per_slot_rows"] == len(expected_active_embedders)
        and readback["per_pair_rows"] == 66
        and readback["policy"].get("aggregate_score_never_overrides_slot_violation") is True
        and external_edges["all_edge_cases_pass"] is True
        and readback["report"].get("flat_vector_concat_used") is False
    )
    return {
        "schema_version": schema_version,
        "task_id": args.task_id,
        "artifact_kind": f"{artifact_kind}_fsv",
        "formula_version": formula_version,
        "created_at_utc": utc_now(),
        "passes": passes,
        "source_of_truth": {key: str(path) for key, path in paths.items()} | {"fsv_root": str(args.fsv_root)},
        "before_state": before,
        "after_state": {"output_root": tree_state(args.output_root), "fsv_root": tree_state(args.fsv_root)},
        "readback": {key: value for key, value in readback.items() if key not in {"policy", "report"}},
        "independent_readback": independent,
        "external_edge_case_readback": external_edges,
        "summary": readback["report"].get("calibration_summary"),
        "policy": readback["policy"],
    }
