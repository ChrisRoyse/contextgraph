"""TASK-PY-G-089 per-slot calibration and OOD guard artifacts."""

from __future__ import annotations

import argparse
import json
import math
import shutil
import sys
from collections import Counter, defaultdict
from datetime import datetime, timedelta, timezone
from pathlib import Path
from typing import Any, Callable

import numpy as np

from .core import (
    DEFAULT_CONSTELLATION_TRAINING_ARRAY_ROOT,
    SwebenchOpsError,
    accuracy_at_half,
    brier,
    calibration_ece,
    pearson_or_none,
    validate_constellation_training_artifact_root,
)
from .python_constellation_tensor_builders import parse_embedder_set
from .python_real_corpus_constellation_smoke import finite_float, sha256_text, write_jsonl
from .smoke_intelligence_matrices import dump_json, iter_jsonl, read_json, sha256_file, utc_now


SCHEMA_VERSION = 1
TASK_ID = "TASK-PY-G-089"
ARTIFACT_KIND = "python_per_slot_calibration_ood"
FORMULA_VERSION = "per_slot_calibration_ood_guard_v1"
DEFAULT_TENSOR_ROOT = Path("/zfs/archive/contextgraph/corpus/constellation-tensors-v1")
DEFAULT_ARTIFACT_ROOT = DEFAULT_CONSTELLATION_TRAINING_ARRAY_ROOT
DEFAULT_OUTPUT_ROOT = Path("/zfs/archive/contextgraph/state/calibration/per-slot-v1")
EXPECTED_ACTIVE_EMBEDDERS = ("e1", "e2", "e3", "e4", "e6", "e7", "e8", "e9", "e10", "e12", "e13", "e14")
EPS = 1.0e-9


class PerSlotCalibrationOodError(RuntimeError):
    pass


def require(condition: bool, code: str, detail: str | None = None) -> None:
    if not condition:
        raise PerSlotCalibrationOodError(code if detail is None else f"{code}: {detail}")


def parse_utc(value: str) -> datetime:
    try:
        parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError as exc:
        raise PerSlotCalibrationOodError(f"MEJEPA_PER_SLOT_CALIBRATION_BAD_TIMESTAMP: {value}") from exc
    if parsed.tzinfo is None:
        parsed = parsed.replace(tzinfo=timezone.utc)
    return parsed.astimezone(timezone.utc)


def quantile(values: list[float], q: float) -> float:
    require(bool(values), "MEJEPA_PER_SLOT_CALIBRATION_EMPTY_QUANTILE")
    return finite_float(float(np.quantile(np.asarray(values, dtype=np.float64), q)))


def load_required_jsonl(path: Path, code: str) -> list[dict[str, Any]]:
    require(path.exists(), code, str(path))
    rows = [row for _line_no, row in iter_jsonl(path)]
    require(bool(rows), code.replace("MISSING", "EMPTY"), str(path))
    return rows


def label(row: dict[str, Any]) -> int:
    value = int(row.get("labels", {}).get("oracle_known_bad"))
    require(value in {0, 1}, "MEJEPA_PER_SLOT_CALIBRATION_BAD_LABEL", str(value))
    return value


def slot_for(row: dict[str, Any], embedder_id: str) -> dict[str, Any] | None:
    for slot in row.get("slots") or []:
        if str(slot.get("embedder_id")) == embedder_id:
            return slot
    return None


def slot_score(slot: dict[str, Any]) -> float:
    value = float(slot.get("slot_anomaly_score"))
    require(math.isfinite(value), "MEJEPA_PER_SLOT_CALIBRATION_NONFINITE_SLOT_SCORE")
    return finite_float(value)


def sigmoid(value: float) -> float:
    return finite_float(1.0 / (1.0 + math.exp(max(-40.0, min(40.0, -value)))))


def probability(score: float, model: dict[str, float]) -> float:
    return sigmoid(float(model["direction"]) * (score - float(model["center"])) / max(EPS, float(model["scale"])))


def fit_model(good: list[float], bad: list[float]) -> dict[str, float]:
    values = np.asarray(good + bad, dtype=np.float64)
    good_mean = float(np.mean(good))
    bad_mean = float(np.mean(bad))
    return {
        "center": finite_float((good_mean + bad_mean) / 2.0),
        "scale": finite_float(max(EPS, float(np.std(values)))),
        "direction": finite_float(1.0 if bad_mean >= good_mean else -1.0),
        "train_good_mean": finite_float(good_mean),
        "train_bad_mean": finite_float(bad_mean),
    }


def metric_block(labels: list[int], probabilities: list[float], bins: int) -> dict[str, Any]:
    corr, corr_reason = pearson_or_none(labels, probabilities)
    return {
        "brier": brier(labels, probabilities),
        "ece": calibration_ece(labels, probabilities, bins),
        "accuracy_at_0_5": accuracy_at_half(labels, probabilities),
        "prediction_oracle_correlation": corr,
        "prediction_oracle_correlation_validity": corr_reason,
    }


def summarize_counts(rows: list[dict[str, Any]], required: tuple[str, ...]) -> dict[str, Any]:
    slot_counts = Counter()
    labels_by_slot: dict[str, Counter] = defaultdict(Counter)
    for row in rows:
        for embedder_id in required:
            slot = slot_for(row, embedder_id)
            if slot is None:
                continue
            slot_counts[embedder_id] += 1
            labels_by_slot[embedder_id][label(row)] += 1
    return {
        "row_count": len(rows),
        "slot_counts": dict(sorted(slot_counts.items())),
        "labels_by_slot": {key: dict(sorted(value.items())) for key, value in sorted(labels_by_slot.items())},
    }


def load_sources(args: argparse.Namespace) -> dict[str, Any]:
    try:
        artifact_guard = validate_constellation_training_artifact_root(
            args.artifact_root,
            consumer="python_per_slot_calibration_ood",
        )
    except SwebenchOpsError as exc:
        raise PerSlotCalibrationOodError(f"{exc.code}: {exc.message}") from exc
    slot_path = args.tensor_root / "slot_distribution_tensors.jsonl"
    pair_path = args.tensor_root / "pairwise_correlation_tensors.jsonl"
    report_path = args.tensor_root / "constellation_tensor_report.json"
    manifest_path = args.tensor_root / "constellation_tensor_manifest.json"
    rows_path = args.artifact_root / "constellation_training_rows.jsonl"
    cert_path = args.artifact_root / "constellation_training_certificate.json"
    slots_path = args.artifact_root / "constellation_slot_vectors.jsonl"
    require(report_path.exists(), "MEJEPA_PER_SLOT_CALIBRATION_TENSOR_REPORT_MISSING", str(report_path))
    require(manifest_path.exists(), "MEJEPA_PER_SLOT_CALIBRATION_TENSOR_MANIFEST_MISSING", str(manifest_path))
    report = read_json(report_path)
    manifest = read_json(manifest_path)
    certificate = artifact_guard["certificate"]
    require(report.get("passes") is True, "MEJEPA_PER_SLOT_CALIBRATION_TENSOR_REPORT_NOT_PASSING")
    require(report.get("constellation_policy", {}).get("flat_vector_concat_used") is False, "MEJEPA_PER_SLOT_CALIBRATION_TENSOR_FLAT_FLAGGED")
    freshness = report.get("freshness") or {}
    stale_after = parse_utc(str(freshness.get("stale_after_utc")))
    require(stale_after > datetime.now(timezone.utc), "MEJEPA_PER_SLOT_CALIBRATION_STALE_TENSOR_INPUT", str(stale_after))
    return {
        "slot_tensors": load_required_jsonl(slot_path, "MEJEPA_PER_SLOT_CALIBRATION_SLOT_TENSORS_MISSING"),
        "pair_tensors": load_required_jsonl(pair_path, "MEJEPA_PER_SLOT_CALIBRATION_PAIR_TENSORS_MISSING"),
        "training_rows": load_required_jsonl(rows_path, "MEJEPA_PER_SLOT_CALIBRATION_TRAINING_ROWS_MISSING"),
        "report": report,
        "manifest": manifest,
        "certificate": certificate,
        "artifact_guard": {
            "root": artifact_guard["root"],
            "consumer": artifact_guard["consumer"],
            "hashes": artifact_guard["hashes"],
            "file_sizes": artifact_guard["file_sizes"],
            "objective_policy": artifact_guard["objective_policy"],
            "baseline_context": artifact_guard["baseline_context"],
            "forbidden_baseline_hits": artifact_guard["forbidden_baseline_hits"],
        },
        "paths": {
            "slot_tensors": str(slot_path),
            "pair_tensors": str(pair_path),
            "tensor_report": str(report_path),
            "tensor_manifest": str(manifest_path),
            "training_rows": str(rows_path),
            "slot_vectors": str(slots_path),
            "objective_certificate": str(cert_path),
        },
        "hashes": {
            "slot_tensors": sha256_file(slot_path),
            "pair_tensors": sha256_file(pair_path),
            "tensor_report": sha256_file(report_path),
            "tensor_manifest": sha256_file(manifest_path),
            "training_rows": artifact_guard["hashes"]["constellation_training_rows.jsonl"],
            "objective_certificate": artifact_guard["hashes"]["constellation_training_certificate.json"],
            "slot_vectors": artifact_guard["hashes"]["constellation_slot_vectors.jsonl"],
        },
    }


def tensor_stats(slot_tensors: list[dict[str, Any]]) -> tuple[dict[str, dict[str, Any]], dict[tuple[str, str], dict[str, Any]]]:
    by_slot: dict[str, list[dict[str, Any]]] = defaultdict(list)
    by_cell: dict[tuple[str, str], list[dict[str, Any]]] = defaultdict(list)
    for row in slot_tensors:
        if row.get("valid") is not True:
            continue
        slot = str(row.get("embedder_id"))
        cell = str(row.get("cell_key"))
        by_slot[slot].append(row)
        by_cell[(cell, slot)].append(row)
    return (
        {slot: tensor_stat_block(rows) for slot, rows in by_slot.items()},
        {key: tensor_stat_block(rows) for key, rows in by_cell.items()},
    )


def tensor_stat_block(rows: list[dict[str, Any]]) -> dict[str, Any]:
    return {
        "tensor_rows": len(rows),
        "cell_keys": sorted({str(row.get("cell_key")) for row in rows}),
        "window_ids": sorted({str(row.get("window_id")) for row in rows}),
        "distance_mean_tau": quantile([float(row["distance_mean"]) for row in rows], 0.95),
        "distance_max_tau": quantile([float(row["distance_max"]) for row in rows], 0.95),
        "variance_mean_tau": quantile([float(row["variance_mean"]) for row in rows], 0.95),
    }


def calibration_row(
    *,
    scope: str,
    embedder_id: str,
    train: list[dict[str, Any]],
    test: list[dict[str, Any]],
    tensor: dict[str, Any] | None,
    args: argparse.Namespace,
    created_at: str,
    stale_after: str,
    cell_key: str | None = None,
) -> dict[str, Any]:
    train_pairs = [(row, slot_for(row, embedder_id)) for row in train]
    test_pairs = [(row, slot_for(row, embedder_id)) for row in test]
    train_pairs = [(row, slot) for row, slot in train_pairs if slot is not None]
    test_pairs = [(row, slot) for row, slot in test_pairs if slot is not None]
    train_labels = Counter(label(row) for row, _slot in train_pairs)
    test_labels = Counter(label(row) for row, _slot in test_pairs)
    base = {
        "artifact_kind": "python_per_slot_calibration",
        "schema_version": SCHEMA_VERSION,
        "formula_version": FORMULA_VERSION,
        "scope": scope,
        "cell_key": cell_key,
        "slot_id": embedder_id,
        "embedder_id": embedder_id,
        "created_at_utc": created_at,
        "stale_after_utc": stale_after,
        "support": {
            "train_rows": len(train_pairs),
            "test_rows": len(test_pairs),
            "train_label_counts": dict(sorted(train_labels.items())),
            "test_label_counts": dict(sorted(test_labels.items())),
            "tensor_rows": 0 if tensor is None else int(tensor["tensor_rows"]),
        },
        "slot_identity_preserved": True,
        "flat_vector_concat_used": False,
    }
    valid = (
        len(train_pairs) >= args.min_train_rows_per_slot
        and len(test_pairs) >= args.min_test_rows_per_slot
        and set(train_labels) == {0, 1}
        and set(test_labels) == {0, 1}
        and tensor is not None
    )
    if not valid:
        base.update(
            {
                "calibration_state": "insufficient_support",
                "valid": False,
                "invalid_reason": "INSUFFICIENT_LABEL_OR_TENSOR_SUPPORT",
                "thresholds": None,
                "metrics": None,
                "model": None,
                "reference_good_score": None,
            }
        )
        return with_hash(base)
    good = [slot_score(slot) for row, slot in train_pairs if label(row) == 0]
    bad = [slot_score(slot) for row, slot in train_pairs if label(row) == 1]
    model = fit_model(good, bad)
    labels = [label(row) for row, _slot in test_pairs]
    probs = [probability(slot_score(slot), model) for _row, slot in test_pairs]
    good_scores = sorted(good)
    thresholds = {
        "ood_tau": quantile(good_scores, args.ood_quantile),
        "conformal_low": quantile(good_scores, args.conformal_alpha / 2.0),
        "conformal_high": quantile(good_scores, 1.0 - args.conformal_alpha / 2.0),
        "distance_mean_tau": tensor["distance_mean_tau"],
        "distance_max_tau": tensor["distance_max_tau"],
        "variance_mean_tau": tensor["variance_mean_tau"],
        "decision_rule": "reject_if_slot_score_above_ood_tau; aggregate score cannot override slot violation",
    }
    base.update(
        {
            "calibration_state": "calibrated",
            "valid": True,
            "invalid_reason": None,
            "feature": "slot_anomaly_score",
            "model": model,
            "thresholds": thresholds,
            "metrics": metric_block(labels, probs, args.ece_bins),
            "reference_good_score": quantile(good_scores, 0.50),
            "reality_prediction_ood_reason_template": "slot={slot_id} score={score:.6f} exceeded tau={ood_tau:.6f}",
        }
    )
    return with_hash(base)


def with_hash(row: dict[str, Any]) -> dict[str, Any]:
    row["record_sha256"] = sha256_text({k: v for k, v in row.items() if k != "record_sha256"})
    return row


def build_calibrations(args: argparse.Namespace, sources: dict[str, Any], required: tuple[str, ...], created_at: str, stale_after: str) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    train = [row for row in sources["training_rows"] if row.get("partition") == "train"]
    test = [row for row in sources["training_rows"] if row.get("partition") == "test"]
    slot_stats, cell_slot_stats = tensor_stats(sources["slot_tensors"])
    global_rows = [
        calibration_row(scope="global", embedder_id=embedder, train=train, test=test, tensor=slot_stats.get(embedder), args=args, created_at=created_at, stale_after=stale_after)
        for embedder in required
    ]
    cells = sorted({str(row.get("cell_key")) for row in sources["training_rows"]})
    cell_rows: list[dict[str, Any]] = []
    for cell in cells:
        cell_train = [row for row in train if str(row.get("cell_key")) == cell]
        cell_test = [row for row in test if str(row.get("cell_key")) == cell]
        for embedder in required:
            cell_rows.append(
                calibration_row(
                    scope="cell",
                    cell_key=cell,
                    embedder_id=embedder,
                    train=cell_train,
                    test=cell_test,
                    tensor=cell_slot_stats.get((cell, embedder)),
                    args=args,
                    created_at=created_at,
                    stale_after=stale_after,
                )
            )
    return global_rows, cell_rows


def build_pair_calibrations(pair_tensors: list[dict[str, Any]], created_at: str, stale_after: str) -> list[dict[str, Any]]:
    grouped: dict[tuple[str, str], list[dict[str, Any]]] = defaultdict(list)
    for row in pair_tensors:
        if row.get("valid") is True:
            grouped[(str(row.get("left_embedder_id")), str(row.get("right_embedder_id")))].append(row)
    out = []
    for (left, right), rows in sorted(grouped.items()):
        disagreement = finite_values(rows, "disagreement_score")
        redundancy = finite_values(rows, "redundancy_score")
        cosine = finite_values(rows, "same_dim_centroid_cosine")
        require(bool(disagreement), "MEJEPA_PER_SLOT_CALIBRATION_PAIR_DISAGREEMENT_MISSING", f"{left}:{right}")
        surface_counts = Counter(str(row.get("surface_class")) for row in rows)
        row = {
            "artifact_kind": "python_per_pair_calibration",
            "schema_version": SCHEMA_VERSION,
            "formula_version": FORMULA_VERSION,
            "created_at_utc": created_at,
            "stale_after_utc": stale_after,
            "left_slot_id": left,
            "right_slot_id": right,
            "support_rows": len(rows),
            "valid": True,
            "support": {
                "disagreement_rows": len(disagreement),
                "redundancy_rows": len(redundancy),
                "same_dim_centroid_cosine_rows": len(cosine),
            },
            "thresholds": {
                "disagreement_tau": quantile(disagreement, 0.95),
                "redundancy_floor": None if not redundancy else quantile(redundancy, 0.05),
                "same_dim_centroid_cosine_floor": None if not cosine else quantile(cosine, 0.05),
            },
            "surface_counts": dict(sorted(surface_counts.items())),
            "slot_identity_preserved": True,
            "flat_vector_concat_used": False,
        }
        out.append(with_hash(row))
    return out


def finite_values(rows: list[dict[str, Any]], key: str) -> list[float]:
    values = []
    for row in rows:
        value = row.get(key)
        if isinstance(value, (int, float)) and math.isfinite(float(value)):
            values.append(finite_float(float(value)))
    return values


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


def write_outputs(args: argparse.Namespace, global_rows: list[dict[str, Any]], cell_rows: list[dict[str, Any]], pair_rows: list[dict[str, Any]], policy: dict[str, Any], report: dict[str, Any]) -> None:
    write_jsonl(args.output_root / "per_slot_calibration.jsonl", global_rows)
    write_jsonl(args.output_root / "per_cell_slot_calibration.jsonl", cell_rows)
    write_jsonl(args.output_root / "per_pair_calibration.jsonl", pair_rows)
    dump_json(args.output_root / "ood_guard_policy.json", policy)
    dump_json(args.output_root / "per_slot_calibration_report.json", report)


def command_python_per_slot_calibration_ood(args: argparse.Namespace) -> int:
    started = utc_now()
    created_at = utc_now()
    stale_after = (datetime.now(timezone.utc) + timedelta(days=args.calibration_ttl_days)).isoformat(timespec="seconds")
    required = parse_embedder_set(args.expected_embedders)
    require(tuple(sorted(required)) == tuple(sorted(EXPECTED_ACTIVE_EMBEDDERS)), "MEJEPA_PER_SLOT_CALIBRATION_UNEXPECTED_EMBEDDER_SET", str(required))
    if args.clean_output and args.output_root.exists():
        shutil.rmtree(args.output_root)
    args.output_root.mkdir(parents=True, exist_ok=True)
    sources = load_sources(args)
    global_rows, cell_rows = build_calibrations(args, sources, required, created_at, stale_after)
    pair_rows = build_pair_calibrations(sources["pair_tensors"], created_at, stale_after)
    cases = edge_cases(global_rows, require=require)
    policy = build_policy(
        required,
        global_rows,
        pair_rows,
        created_at,
        stale_after,
        cases,
        schema_version=SCHEMA_VERSION,
        task_id=TASK_ID,
        formula_version=FORMULA_VERSION,
    )
    report = build_report(args, sources, global_rows, cell_rows, pair_rows, policy, cases, started)
    write_outputs(args, global_rows, cell_rows, pair_rows, policy, report)
    report_path = args.output_root / "per_slot_calibration_report.json"
    policy_path = args.output_root / "ood_guard_policy.json"
    final_report = read_json(report_path)
    final_policy = read_json(policy_path)
    require(final_report.get("passes") is True, "MEJEPA_PER_SLOT_CALIBRATION_REPORT_FAILED", str(report_path))
    require(final_policy.get("aggregate_score_never_overrides_slot_violation") is True, "MEJEPA_PER_SLOT_CALIBRATION_POLICY_WEAK", str(policy_path))
    print(json.dumps(final_report, sort_keys=True, indent=2))
    return 0


def build_report(args: argparse.Namespace, sources: dict[str, Any], global_rows: list[dict[str, Any]], cell_rows: list[dict[str, Any]], pair_rows: list[dict[str, Any]], policy: dict[str, Any], cases: list[dict[str, Any]], started: str) -> dict[str, Any]:
    cell_ready = sum(1 for row in cell_rows if row.get("valid") is True)
    passes = policy["global_slot_calibration_ready"] and len(pair_rows) == 66 and all(case.get("passes") is True for case in cases)
    return {
        "schema_version": SCHEMA_VERSION,
        "task_id": args.task_id,
        "artifact_kind": f"{ARTIFACT_KIND}_report",
        "formula_version": FORMULA_VERSION,
        "started_at_utc": started,
        "created_at_utc": utc_now(),
        "passes": passes,
        "source_of_truth": {"tensor_root": str(args.tensor_root), "artifact_root": str(args.artifact_root), "output_root": str(args.output_root), **sources["paths"]},
        "source_hashes": sources["hashes"],
        "source_artifact_guard": sources["artifact_guard"],
        "dataset_summary": summarize_counts(sources["training_rows"], parse_embedder_set(args.expected_embedders)),
        "calibration_summary": {
            "global_slot_rows": len(global_rows),
            "global_calibrated_slots": sum(1 for row in global_rows if row.get("valid") is True),
            "per_cell_slot_rows": len(cell_rows),
            "per_cell_calibrated_slots": cell_ready,
            "pair_calibration_rows": len(pair_rows),
            "strict_per_cell_gate_status": "fail_closed_until_all_required_cell_slots_have_support" if cell_ready < len(cell_rows) else "ready",
        },
        "policy": policy,
        "edge_cases": cases,
        "slot_identity_preserved": True,
        "flat_vector_concat_used": False,
    }


def add_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--tensor-root", type=Path, default=DEFAULT_TENSOR_ROOT)
    parser.add_argument("--artifact-root", type=Path, default=DEFAULT_ARTIFACT_ROOT)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--task-id", default=TASK_ID)
    parser.add_argument("--expected-embedders", default=",".join(EXPECTED_ACTIVE_EMBEDDERS))
    parser.add_argument("--min-train-rows-per-slot", type=int, default=20)
    parser.add_argument("--min-test-rows-per-slot", type=int, default=5)
    parser.add_argument("--ood-quantile", type=float, default=0.95)
    parser.add_argument("--conformal-alpha", type=float, default=0.10)
    parser.add_argument("--ece-bins", type=int, default=8)
    parser.add_argument("--calibration-ttl-days", type=int, default=14)
    parser.add_argument("--clean-output", action="store_true")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    add_arguments(parser)
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        return command_python_per_slot_calibration_ood(args)
    except PerSlotCalibrationOodError as exc:
        print(f"PYTHON_PER_SLOT_CALIBRATION_OOD_FAILED: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
