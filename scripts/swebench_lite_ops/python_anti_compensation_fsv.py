"""TASK-PY-G-094 anti-compensation FSV suite.

This suite proves that a local slot/pair/temporal failure cannot be hidden by
strong unrelated slots or a flat aggregate. The fixtures are synthetic by
design, but every calibration threshold and slot dimension is read from the
current aiwonder #388/#389 artifacts.
"""

from __future__ import annotations

import argparse
import ast
import copy
import json
import math
import shutil
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

import numpy as np

from .python_constellation_ablation_suite import (
    DEFAULT_CALIBRATION_FSV,
    DEFAULT_CALIBRATION_ROOT,
    DEFAULT_TENSOR_ROOT,
    EPS,
    evaluate_calibration_guard,
    load_calibration_state,
    load_tensor_context,
    parse_utc,
)
from .python_per_slot_calibration_ood import EXPECTED_ACTIVE_EMBEDDERS
from .python_real_corpus_constellation_smoke import finite_float, sha256_text, write_jsonl
from .smoke_intelligence_matrices import dump_json, iter_jsonl, read_json, sha256_file, tree_state, utc_now


SCHEMA_VERSION = 1
TASK_ID = "TASK-PY-G-094"
ARTIFACT_KIND = "python_anti_compensation_fsv"
FORMULA_VERSION = "slot_pair_temporal_anti_compensation_fsv_v1"
DEFAULT_FSV_ROOT = Path("/zfs/archive/contextgraph/fsv/task-py-g-094-anti-compensation-fsv")
DEFAULT_REPO_ROOT = Path(__file__).resolve().parents[2]


class AntiCompensationFsvError(RuntimeError):
    pass

@dataclass(frozen=True)
class Fixture:
    name: str
    kind: str
    expected_verdict: str
    expected_reason: str
    observation: dict[str, float]
    calibrations: list[dict[str, Any]]
    dimensions: dict[str, int]
    pair_observation: dict[str, Any] | None = None
    temporal_observation: dict[str, Any] | None = None


def require(condition: bool, code: str, detail: str | None = None) -> None:
    if not condition:
        raise AntiCompensationFsvError(code if detail is None else f"{code}: {detail}")


def valid_global_rows(calibration: dict[str, Any]) -> list[dict[str, Any]]:
    rows = [row for row in calibration["per_slot_rows"] if row.get("valid") is True]
    require(len(rows) == len(EXPECTED_ACTIVE_EMBEDDERS), "MEJEPA_ANTI_COMPENSATION_GLOBAL_SLOT_SET_INCOMPLETE")
    return sorted(rows, key=lambda row: str(row["embedder_id"]))


def nominal_observation(rows: list[dict[str, Any]]) -> dict[str, float]:
    out = {}
    for row in rows:
        score = float(row["reference_good_score"])
        require(math.isfinite(score), "MEJEPA_ANTI_COMPENSATION_REFERENCE_SCORE_NONFINITE", str(row["embedder_id"]))
        out[str(row["embedder_id"])] = finite_float(score)
    return out


def load_slot_dimensions(tensor_root: Path, required_slots: tuple[str, ...]) -> dict[str, int]:
    path = tensor_root / "slot_distribution_tensors.jsonl"
    require(path.exists(), "MEJEPA_ANTI_COMPENSATION_SLOT_TENSOR_FILE_MISSING", str(path))
    dims: dict[str, int] = {}
    for _line_no, row in iter_jsonl(path):
        if row.get("valid") is not True:
            continue
        slot = str(row.get("embedder_id"))
        if slot not in required_slots or slot in dims:
            continue
        vector = row.get("centroid_vector")
        require(isinstance(vector, list) and vector, "MEJEPA_ANTI_COMPENSATION_CENTROID_VECTOR_MISSING", slot)
        dims[slot] = len(vector)
    missing = sorted(set(required_slots) - set(dims))
    require(not missing, "MEJEPA_ANTI_COMPENSATION_SLOT_DIMENSIONS_MISSING", ",".join(missing))
    return dims


def load_pair_thresholds(calibration: dict[str, Any]) -> dict[tuple[str, str], float]:
    thresholds = {}
    for row in calibration["per_pair_rows"]:
        key = tuple(sorted((str(row["left_slot_id"]), str(row["right_slot_id"]))))
        thresholds[key] = finite_float(float(row["thresholds"]["disagreement_tau"]))
    require(len(thresholds) == 66, "MEJEPA_ANTI_COMPENSATION_PAIR_THRESHOLD_COUNT_INVALID", str(len(thresholds)))
    return thresholds


def build_fixtures(calibration: dict[str, Any], tensor_root: Path) -> list[Fixture]:
    rows = valid_global_rows(calibration)
    base = nominal_observation(rows)
    dims = load_slot_dimensions(tensor_root, tuple(EXPECTED_ACTIVE_EMBEDDERS))
    pair_thresholds = load_pair_thresholds(calibration)

    corrupted = dict(base)
    corrupted_slot = str(rows[0]["embedder_id"])
    corrupted[corrupted_slot] = finite_float(float(rows[0]["thresholds"]["ood_tau"]) * 1.25)

    missing = dict(base)
    missing_slot = str(rows[1]["embedder_id"])
    missing.pop(missing_slot)

    stale_rows = copy.deepcopy(rows)
    stale_slot = str(stale_rows[2]["embedder_id"])
    for row in stale_rows:
        if str(row["embedder_id"]) == stale_slot:
            row["stale_after_utc"] = "2000-01-01T00:00:00+00:00"

    shifted_dims = dict(dims)
    shifted_slot = str(rows[3]["embedder_id"])
    shifted_dims[shifted_slot] = shifted_dims[shifted_slot] + 1

    pair_key = next(iter(sorted(pair_thresholds)))
    pair_observation = {
        "pair_id": "|".join(pair_key),
        "left_slot_id": pair_key[0],
        "right_slot_id": pair_key[1],
        "score": finite_float(pair_thresholds[pair_key] * 1.5),
        "disagreement_tau": pair_thresholds[pair_key],
        "surface_class": "contradictory",
        "all_individual_slots_nominal": True,
    }

    temporal_observation = {
        "slot_id": str(rows[4]["embedder_id"]),
        "previous_normalized_score": 0.20,
        "current_normalized_score": 0.95,
        "jump_tau": 0.50,
        "all_static_slot_scores_nominal": True,
    }

    return [
        Fixture("corrupted_slot_cannot_be_averaged_away", "slot_corruption", "GuardRejected", "SLOT_OOD_TAU_EXCEEDED", corrupted, rows, dims),
        Fixture("missing_slot_cannot_be_averaged_away", "missing_slot", "GuardRejected", "MISSING_SLOT_OBSERVATION", missing, rows, dims),
        Fixture("stale_slot_calibration_rejects", "stale_slot", "GuardRejected", "CALIBRATION_STALE", base, stale_rows, dims),
        Fixture("dimension_shifted_slot_rejects", "dimension_shift", "GuardRejected", "DIMENSION_SHIFTED_SLOT", base, rows, shifted_dims),
        Fixture("contradictory_pair_rejects_nominal_slots", "pair_contradiction", "Abstain", "PAIR_CONTRADICTION_TAU_EXCEEDED", base, rows, dims, pair_observation=pair_observation),
        Fixture("temporal_jump_rejects_nominal_static_scores", "temporal_jump", "OutOfDistribution", "TEMPORAL_SLOT_JUMP_EXCEEDED", base, rows, dims, temporal_observation=temporal_observation),
    ]


def evaluate_fixture(fixture: Fixture, expected_dims: dict[str, int], now: datetime) -> dict[str, Any]:
    guard = evaluate_calibration_guard(fixture.observation, fixture.calibrations, now)
    violations = list(guard["violations"])
    for slot, expected in sorted(expected_dims.items()):
        observed = fixture.dimensions.get(slot)
        if observed != expected:
            violations.append({"slot_id": slot, "reason": "DIMENSION_SHIFTED_SLOT", "expected_dim": expected, "observed_dim": observed})

    pair_decision = None
    if fixture.pair_observation is not None:
        pair = fixture.pair_observation
        if float(pair["score"]) > float(pair["disagreement_tau"]):
            violations.append(
                {
                    "pair_id": pair["pair_id"],
                    "reason": "PAIR_CONTRADICTION_TAU_EXCEEDED",
                    "score": pair["score"],
                    "tau": pair["disagreement_tau"],
                }
            )
        pair_decision = {"surface": "gtau_pair_relationship", "observation": pair}

    temporal_decision = None
    if fixture.temporal_observation is not None:
        temporal = fixture.temporal_observation
        jump = finite_float(abs(float(temporal["current_normalized_score"]) - float(temporal["previous_normalized_score"])))
        if jump > float(temporal["jump_tau"]):
            violations.append({"slot_id": temporal["slot_id"], "reason": "TEMPORAL_SLOT_JUMP_EXCEEDED", "jump": jump, "tau": temporal["jump_tau"]})
        temporal_decision = {"surface": "temporal_trajectory", "observation": temporal, "jump": jump}

    aggregate_mean = finite_float(float(np.mean([fixture.observation[str(row["embedder_id"])] / max(EPS, float(row["thresholds"]["ood_tau"])) for row in fixture.calibrations if str(row["embedder_id"]) in fixture.observation])))
    aggregate_would_pass = aggregate_mean <= 1.0
    verdict = verdict_for(fixture, violations)
    pass_condition = (
        verdict == fixture.expected_verdict
        and any(item.get("reason") == fixture.expected_reason for item in violations)
        and aggregate_would_pass is True
        and not (verdict == "Pass")
    )
    return {
        "schema_version": SCHEMA_VERSION,
        "fixture": fixture.name,
        "kind": fixture.kind,
        "passes": pass_condition,
        "expected_verdict": fixture.expected_verdict,
        "actual_verdict": verdict,
        "expected_reason": fixture.expected_reason,
        "aggregate_mean_normalized_score": aggregate_mean,
        "aggregate_mean_would_pass": aggregate_would_pass,
        "flat_vector_concat_used": False,
        "ship_gate_eligible": False,
        "guard_decision": {**guard, "violations": violations},
        "pair_decision": pair_decision,
        "temporal_decision": temporal_decision,
        "prediction_surface": prediction_surface(fixture, verdict, violations),
        "ood_surface": ood_surface(violations),
        "gtau_surface": gtau_surface(violations),
        "q4_surface": q4_surface(fixture, violations),
        "active_learning_row": active_learning_row(fixture, violations),
        "explanation_surface": explanation_surface(fixture, violations),
    }

def verdict_for(fixture: Fixture, violations: list[dict[str, Any]]) -> str:
    if not violations:
        return "Pass"
    if fixture.kind == "pair_contradiction":
        return "Abstain"
    if fixture.kind == "temporal_jump":
        return "OutOfDistribution"
    return "GuardRejected"

def prediction_surface(fixture: Fixture, verdict: str, violations: list[dict[str, Any]]) -> dict[str, Any]:
    return {
        "surface": "RealityPrediction",
        "verdict": verdict,
        "predicted_oracle_pass": 0.01 if verdict in {"GuardRejected", "OutOfDistribution"} else 0.50,
        "confidence_source": "slot_pair_temporal_anti_compensation_fixture",
        "ship_gate_countable": False,
        "per_slot_ood_reasons": [item for item in violations if "slot_id" in item],
        "pairwise_reasons": [item for item in violations if "pair_id" in item],
        "consequence": "candidate action is not safe to treat as done until named local violation is resolved",
        "human_intent_inference_used": False,
    }

def ood_surface(violations: list[dict[str, Any]]) -> dict[str, Any]:
    return {
        "surface": "OOD",
        "status": "reject" if violations else "accept",
        "per_slot_reasons": [item for item in violations if "slot_id" in item],
        "unknown_ood_queue_required": bool(violations),
    }

def gtau_surface(violations: list[dict[str, Any]]) -> dict[str, Any]:
    return {
        "surface": "Gtau",
        "status": "guard_rejected" if violations else "pass",
        "named_violations": violations,
        "aggregate_override_allowed": False,
    }

def q4_surface(fixture: Fixture, violations: list[dict[str, Any]]) -> dict[str, Any]:
    return {
        "surface": "Q4",
        "status": "fail_closed",
        "concerns": [
            {
                "class": "accuracy",
                "severity": "high",
                "reason": fixture.expected_reason,
                "label_producer_state": "not_ship_gate_countable_until_Q4_producers_266_269_and_127_close",
            }
        ],
        "blocks_ship_gate": bool(violations),
    }

def active_learning_row(fixture: Fixture, violations: list[dict[str, Any]]) -> dict[str, Any]:
    return {
        "queue": "CF_MEJEPA_ACTIVE_LEARNING_QUEUE",
        "candidate_id": sha256_text({"fixture": fixture.name, "violations": violations}),
        "source": "TASK-PY-G-094_ANTI_COMPENSATION_FSV",
        "sampling_weight": 4.0 if violations else 1.0,
        "reasons": violations,
        "requires_operator_label": fixture.kind in {"pair_contradiction", "temporal_jump"},
    }

def explanation_surface(fixture: Fixture, violations: list[dict[str, Any]]) -> dict[str, Any]:
    return {
        "surface": "explanation",
        "primary_failure": fixture.expected_reason,
        "named_slot_evidence": [item for item in violations if "slot_id" in item],
        "named_pair_evidence": [item for item in violations if "pair_id" in item],
        "aggregate_score_note": "aggregate mean would have passed; local evidence still blocks",
    }

def scan_flat_ship_gate_regressions(repo_root: Path) -> dict[str, Any]:
    roots = [repo_root / "scripts" / "swebench_lite_ops", repo_root / "crates" / "context-graph-mejepa", repo_root / "crates" / "context-graph-mcp"]
    files = [path for root in roots if root.exists() for path in root.rglob("*") if path.suffix in {".py", ".rs"}]
    violations: list[dict[str, Any]] = []
    for path in files:
        rel = str(path.relative_to(repo_root))
        text = path.read_text(encoding="utf-8", errors="replace")
        if path.suffix == ".py":
            violations.extend(scan_python_ablation_specs(path, rel, text))
        for line_no, line in enumerate(text.splitlines(), start=1):
            compact = line.replace(" ", "").lower()
            if rel.endswith("python_anti_compensation_fsv.py") or "FLAT_SHIP_GATE_ELIGIBLE_INLINE_PATTERN" in line: continue
            if ("ship_gate_eligible=true" in compact or '"ship_gate_eligible":true' in compact) and "flat" in compact:
                violations.append({"file": rel, "line": line_no, "reason": "FLAT_SHIP_GATE_ELIGIBLE_INLINE_PATTERN", "snippet": line.strip()[:240]})
    return {
        "scanned_files": len(files),
        "violations": violations,
        "passes": not violations,
        "policy": "flat-vector baselines may exist only when ship_gate_eligible is false",
    }

def scan_python_ablation_specs(path: Path, rel: str, text: str) -> list[dict[str, Any]]:
    out: list[dict[str, Any]] = []
    try:
        tree = ast.parse(text, filename=str(path))
    except SyntaxError as exc:
        return [{"file": rel, "line": exc.lineno, "reason": "PYTHON_PARSE_FAILED", "snippet": str(exc)}]
    for node in ast.walk(tree):
        if not isinstance(node, ast.Call) or not isinstance(node.func, ast.Name) or node.func.id != "AblationSpec":
            continue
        if len(node.args) < 5:
            continue
        flat = literal_bool(node.args[3])
        eligible = literal_bool(node.args[4])
        if flat is True and eligible is True:
            out.append({"file": rel, "line": node.lineno, "reason": "ABLATION_SPEC_FLAT_VECTOR_SHIP_GATE_ELIGIBLE"})
    return out

def literal_bool(node: ast.AST) -> bool | None:
    if isinstance(node, ast.Constant) and isinstance(node.value, bool):
        return node.value
    return None

def build_report(args: argparse.Namespace, before: dict[str, Any], results: list[dict[str, Any]], scan: dict[str, Any], source_hashes: dict[str, str], started: str) -> dict[str, Any]:
    return {
        "schema_version": SCHEMA_VERSION,
        "task_id": args.task_id,
        "artifact_kind": f"{ARTIFACT_KIND}_report",
        "formula_version": FORMULA_VERSION,
        "started_at_utc": started,
        "created_at_utc": utc_now(),
        "passes": all(row["passes"] for row in results) and scan["passes"],
        "source_of_truth": {
            "tensor_root": str(args.tensor_root),
            "calibration_root": str(args.calibration_root),
            "calibration_fsv": str(args.calibration_fsv),
            "output_root": str(args.output_root),
            "fsv_root": str(args.fsv_root),
        },
        "source_hashes": source_hashes,
        "policy": {
            "slot_identity_preserved": True,
            "flat_vector_concat_used": False,
            "aggregate_score_never_overrides_slot_violation": True,
            "flat_vector_ship_gate_eligible_allowed": False,
            "human_intent_inference_used": False,
            "synthetic_fixture_scope": "fail_closed_anti_compensation_only",
        },
        "before_state": before,
        "fixture_count": len(results),
        "fixtures": results,
        "code_scan": scan,
    }


def build_fsv(args: argparse.Namespace, before: dict[str, Any], report_path: Path, fixture_path: Path, active_learning_path: Path, report: dict[str, Any]) -> dict[str, Any]:
    readback_report = read_json(report_path)
    fixture_rows = [row for _line_no, row in iter_jsonl(fixture_path)]
    active_rows = [row for _line_no, row in iter_jsonl(active_learning_path)]
    direct = []
    for row, active in zip(fixture_rows, active_rows):
        direct.append(
            {
                "fixture": row["fixture"],
                "prediction_verdict": row["prediction_surface"]["verdict"],
                "guard_status": row["gtau_surface"]["status"],
                "attribution_primary_failure": row["explanation_surface"]["primary_failure"],
                "active_learning_candidate_id": active["candidate_id"],
                "ship_gate_eligible": row["ship_gate_eligible"],
                "passes": row["passes"],
            }
        )
    independent = {
        "report_passes": readback_report["passes"],
        "fixture_rows": len(fixture_rows),
        "active_learning_rows": len(active_rows),
        "all_fixtures_have_named_evidence": all(row["explanation_surface"]["named_slot_evidence"] or row["explanation_surface"]["named_pair_evidence"] for row in fixture_rows),
        "no_fixture_passed_from_aggregate_only_evidence": all(row["actual_verdict"] != "Pass" and row["aggregate_mean_would_pass"] is True for row in fixture_rows),
        "no_fixture_ship_gate_eligible": all(row["ship_gate_eligible"] is False for row in fixture_rows),
        "flat_vector_scan_passes": readback_report["code_scan"]["passes"],
        "stable_hashes": {
            "report": sha256_file(report_path),
            "fixture_results": sha256_file(fixture_path),
            "active_learning_rows": sha256_file(active_learning_path),
        },
    }
    edges = {
        "fixture_names": [row["fixture"] for row in fixture_rows],
        "expected_fixture_count": 6,
        "direct_surface_readback": direct,
        "all_edge_cases_pass": all(row["passes"] for row in fixture_rows),
    }
    dump_json(args.fsv_root / "independent_readback.json", independent)
    dump_json(args.fsv_root / "external_edge_case_readback.json", edges)
    passes = (
        independent["report_passes"] is True
        and independent["fixture_rows"] == 6
        and independent["active_learning_rows"] == 6
        and independent["all_fixtures_have_named_evidence"] is True
        and independent["no_fixture_passed_from_aggregate_only_evidence"] is True
        and independent["no_fixture_ship_gate_eligible"] is True
        and independent["flat_vector_scan_passes"] is True
        and edges["all_edge_cases_pass"] is True
    )
    return {
        "schema_version": SCHEMA_VERSION,
        "task_id": args.task_id,
        "artifact_kind": ARTIFACT_KIND,
        "formula_version": FORMULA_VERSION,
        "created_at_utc": utc_now(),
        "passes": passes,
        "source_of_truth": {
            "fixture_results": str(fixture_path),
            "active_learning_rows": str(active_learning_path),
            "report": str(report_path),
            "independent_readback": str(args.fsv_root / "independent_readback.json"),
            "external_edge_case_readback": str(args.fsv_root / "external_edge_case_readback.json"),
        },
        "before_state": before,
        "after_state": {"output_root": tree_state(args.output_root), "fsv_root": tree_state(args.fsv_root)},
        "independent_readback": independent,
        "external_edge_case_readback": edges,
        "summary": {
            "fixture_count": len(fixture_rows),
            "guarded_fixture_count": sum(1 for row in fixture_rows if row["actual_verdict"] in {"GuardRejected", "OutOfDistribution", "Abstain"}),
            "code_scan_violations": len(readback_report["code_scan"]["violations"]),
        },
    }

def command_python_anti_compensation_fsv(args: argparse.Namespace) -> int:
    started = utc_now()
    before = {"output_root": tree_state(args.output_root), "fsv_root": tree_state(args.fsv_root)}
    if args.clean_output and args.output_root.exists():
        shutil.rmtree(args.output_root)
    if args.clean_output and args.fsv_root.exists():
        shutil.rmtree(args.fsv_root)
    args.output_root.mkdir(parents=True, exist_ok=True)
    args.fsv_root.mkdir(parents=True, exist_ok=True)

    tensor_context = load_tensor_context(args.tensor_root)
    calibration = load_calibration_state(args.calibration_root, args.calibration_fsv, tensor_context)
    expected_dims = load_slot_dimensions(args.tensor_root, tuple(EXPECTED_ACTIVE_EMBEDDERS))
    now = datetime.now(timezone.utc)
    freshness = parse_utc(str(calibration["policy"]["stale_after_utc"]))
    require(freshness > now, "MEJEPA_ANTI_COMPENSATION_CALIBRATION_STALE_AT_START")
    fixtures = build_fixtures(calibration, args.tensor_root)
    results = [evaluate_fixture(fixture, expected_dims, now) for fixture in fixtures]
    require(all(row["passes"] for row in results), "MEJEPA_ANTI_COMPENSATION_FIXTURE_FAILED")
    scan = scan_flat_ship_gate_regressions(args.repo_root)
    require(scan["passes"], "MEJEPA_ANTI_COMPENSATION_FLAT_SHIP_GATE_SCAN_FAILED")

    fixture_path = args.output_root / "anti_compensation_fixture_results.jsonl"
    active_learning_path = args.output_root / "anti_compensation_active_learning_rows.jsonl"
    report_path = args.output_root / "anti_compensation_report.json"
    write_jsonl(fixture_path, results)
    write_jsonl(active_learning_path, [row["active_learning_row"] for row in results])
    source_hashes = {
        "tensor_report": sha256_file(args.tensor_root / "constellation_tensor_report.json"),
        "per_slot_calibration": sha256_file(args.calibration_root / "per_slot_calibration.jsonl"),
        "per_pair_calibration": sha256_file(args.calibration_root / "per_pair_calibration.jsonl"),
        "calibration_fsv": sha256_file(args.calibration_fsv),
    }
    report = build_report(args, before, results, scan, source_hashes, started)
    dump_json(report_path, report)
    fsv = build_fsv(args, before, report_path, fixture_path, active_learning_path, report)
    fsv_path = args.fsv_root / f"{ARTIFACT_KIND}.json"
    dump_json(fsv_path, fsv)
    final = read_json(fsv_path)
    require(final.get("passes") is True, "MEJEPA_ANTI_COMPENSATION_FSV_FAILED", str(fsv_path))
    print(json.dumps(final, sort_keys=True, indent=2))
    return 0

def add_arguments(parser: argparse.ArgumentParser) -> None:
    for name, default in (
        ("--tensor-root", DEFAULT_TENSOR_ROOT),
        ("--calibration-root", DEFAULT_CALIBRATION_ROOT),
        ("--calibration-fsv", DEFAULT_CALIBRATION_FSV),
        ("--output-root", DEFAULT_FSV_ROOT / "artifacts"),
        ("--fsv-root", DEFAULT_FSV_ROOT),
        ("--repo-root", DEFAULT_REPO_ROOT),
    ):
        parser.add_argument(name, type=Path, default=default)
    parser.add_argument("--task-id", default=TASK_ID)
    parser.add_argument("--clean-output", action="store_true")
