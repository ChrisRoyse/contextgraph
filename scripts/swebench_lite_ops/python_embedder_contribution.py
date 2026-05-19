"""Quantify smoke-corpus embedder contribution to Q1-Q5 targets."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import shutil
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any

import numpy as np

from .python_pairwise_dda_mi import SCALAR_FEATURES
from .smoke_intelligence_matrices import artifact_state, dump_json, iter_jsonl, read_json, sha256_file, tree_state, utc_now, write_jsonl


SCHEMA_VERSION = 1
TASK_ID = "TASK-PY-DATA-055"
ARTIFACT_KIND = "python_embedder_q_head_contribution"
FORMULA_VERSION = "python_embedder_contribution_v1"
DEFAULT_REWARD_ROOT = Path("/zfs/archive/contextgraph/corpus/python-reward-tensors-v1")
DEFAULT_DDA_ROOT = Path("/zfs/archive/contextgraph/corpus/python-dda-signals-v1/per-embedder")
DEFAULT_PAIRWISE_ROOT = Path("/zfs/archive/contextgraph/corpus/python-dda-signals-v1/pairwise")
DEFAULT_MATRIX_ROOT = Path("/zfs/archive/contextgraph/corpus/python-embedder-intelligence-smoke-1000-v2/intelligence-matrices")
DEFAULT_OUTPUT_ROOT = Path("/zfs/archive/contextgraph/corpus/python-embedder-intelligence-v1/contribution")
DEFAULT_FSV_ROOT = Path("/zfs/archive/contextgraph/fsv/task-py-data-055-embedder-contribution-fsv")
DEFAULT_REWARD_FSV = Path("/zfs/archive/contextgraph/fsv/task-py-data-021-reward-tensors-fsv/python_reward_tensors_fsv.json")
DEFAULT_DDA_FSV = Path("/zfs/archive/contextgraph/fsv/task-py-data-017-dda-signals-fsv/python_dda_code_signals_fsv.json")
DEFAULT_PAIRWISE_FSV = Path("/zfs/archive/contextgraph/fsv/task-py-data-018-pairwise-dda-mi-fsv/python_pairwise_dda_mi_fsv.json")
DEFAULT_TCT_FSV = Path("/zfs/archive/contextgraph/fsv/task-py-data-020-tct-cells-fsv/python_tct_cells_fsv.json")
DEFAULT_MATRIX_FSV = Path("/zfs/archive/contextgraph/fsv/task-py-data-062-post249-smoke-intelligence-fsv/smoke_intelligence_matrices_fsv.json")
DEFAULT_EXPECTED_EMBEDDERS = "e1,e6,e7,e8,e10,e12,e13,e14,e2,e3,e4,e9"
EPS = 1.0e-12


class EmbedderContributionError(RuntimeError):
    pass


def require(condition: bool, code: str, message: str, details: dict[str, Any] | None = None) -> None:
    if not condition:
        raise EmbedderContributionError(json.dumps({"error_code": code, "message": message, "details": details or {}}, sort_keys=True))


def canonical_json(payload: Any) -> str:
    return json.dumps(payload, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def sha256_text(payload: Any) -> str:
    return "sha256:" + hashlib.sha256(canonical_json(payload).encode("utf-8")).hexdigest()


def safe_float(value: Any) -> float | None:
    if value is None:
        return None
    try:
        out = float(value)
    except (TypeError, ValueError):
        return None
    if math.isfinite(out):
        return out
    return None


def finite_number(value: Any) -> float | None:
    if isinstance(value, bool):
        return None
    return safe_float(value)


def ratio(numerator: int, denominator: int) -> float:
    return float(numerator / denominator) if denominator else 0.0


def load_passing_json(path: Path, code: str) -> tuple[dict[str, Any], str]:
    doc = read_json(path)
    require(doc.get("passes") is True, code, f"required FSV does not pass: {path}", {"path": str(path)})
    return doc, sha256_file(path)


def load_jsonl(path: Path) -> list[dict[str, Any]]:
    return [row for _, row in iter_jsonl(path)]


def pearson_abs(x: np.ndarray, y: np.ndarray) -> tuple[float | None, str | None]:
    if x.size < 2:
        return None, "TOO_FEW_FINITE_VALUES"
    if float(np.std(x)) <= EPS or float(np.std(y)) <= EPS:
        return None, "DEGENERATE_VARIANCE"
    value = float(np.corrcoef(x, y)[0, 1])
    if not math.isfinite(value):
        return None, "NONFINITE_CORRELATION"
    return abs(value), None


def correlation_ratio_eta2(feature: np.ndarray, labels: list[str]) -> tuple[float | None, str | None]:
    if feature.size < 2:
        return None, "TOO_FEW_FINITE_VALUES"
    groups: dict[str, list[float]] = defaultdict(list)
    for value, label in zip(feature.tolist(), labels, strict=True):
        groups[label].append(float(value))
    if len(groups) < 2:
        return None, "DEGENERATE_CATEGORICAL_TARGET"
    grand = float(np.mean(feature))
    ss_between = 0.0
    ss_total = float(np.sum((feature - grand) ** 2))
    if ss_total <= EPS:
        return None, "DEGENERATE_VARIANCE"
    for values in groups.values():
        arr = np.asarray(values, dtype=np.float64)
        ss_between += float(arr.size) * float((arr.mean() - grand) ** 2)
    eta2 = ss_between / ss_total
    if not math.isfinite(eta2):
        return None, "NONFINITE_CORRELATION_RATIO"
    return max(0.0, min(1.0, eta2)), None


def confidence_band(values: list[float]) -> dict[str, Any]:
    if not values:
        return {"mean": None, "lower_95": None, "upper_95": None, "n": 0}
    arr = np.asarray(values, dtype=np.float64)
    mean = float(arr.mean())
    if arr.size == 1:
        return {"mean": mean, "lower_95": mean, "upper_95": mean, "n": 1}
    se = float(arr.std(ddof=1) / math.sqrt(arr.size))
    return {
        "mean": mean,
        "lower_95": max(0.0, mean - 1.96 * se),
        "upper_95": min(1.0, mean + 1.96 * se),
        "n": int(arr.size),
    }


def top_mean(values: list[float], k: int = 5) -> float | None:
    if not values:
        return None
    selected = sorted(values, reverse=True)[:k]
    return float(np.asarray(selected, dtype=np.float64).mean())


def parse_embedders(value: str) -> list[str]:
    embedders = [item.strip() for item in value.split(",") if item.strip()]
    require(bool(embedders), "EMBEDDER_CONTRIBUTION_NO_EXPECTED_EMBEDDERS", "expected embedder list is empty")
    require(len(embedders) == len(set(embedders)), "EMBEDDER_CONTRIBUTION_DUPLICATE_EMBEDDER", "expected embedder list has duplicates", {"embedders": embedders})
    return embedders


def q5_burden(head: dict[str, Any]) -> float:
    evidence = head.get("evidence") or {}
    source_byte_len = max(0.0, float(evidence.get("source_byte_len") or 0.0))
    line_span = max(0.0, float(evidence.get("line_span") or 0.0))
    fail_to_pass = max(0.0, float(evidence.get("fail_to_pass_count") or 0.0))
    pass_to_pass = max(0.0, float(evidence.get("pass_to_pass_count") or 0.0))
    return math.log1p(source_byte_len) + (0.1 * line_span) + (5.0 * fail_to_pass) + (0.25 * pass_to_pass)


def target_for_head(row: dict[str, Any], head: str) -> tuple[Any | None, str | None]:
    heads = row.get("heads") or {}
    if head == "q1":
        q1 = heads.get("q1") or {}
        if q1.get("label_state") != "code_label":
            return None, "NON_CODE_LABEL"
        return 1.0 if q1.get("label") is True else 0.0, None
    if head == "q2":
        q2 = heads.get("q2") or {}
        if q2.get("label_state") != "code_label":
            return None, "NON_CODE_LABEL"
        if q2.get("label") is None:
            return None, "NULL_Q2_LABEL"
        return 1.0 if q2.get("label") is True else 0.0, None
    if head == "q3":
        q3 = heads.get("q3") or {}
        if q3.get("label_state") != "code_label":
            return None, "NON_CODE_LABEL"
        value = str(q3.get("primary_failure_reason") or "")
        if not value or value in {"none", "excluded_infra"}:
            return None, "NO_FAILURE_REASON_TARGET"
        return value, None
    if head == "q4":
        q4 = heads.get("q4") or {}
        if q4.get("label_state") != "code_label":
            return None, "NON_CODE_LABEL"
        concerns = sorted(str(item) for item in (q4.get("concern_targets") or []) if str(item))
        if not concerns:
            return None, "NO_Q4_CONCERNS"
        return "|".join(concerns), None
    if head == "q5":
        q5 = heads.get("q5") or {}
        if q5.get("label_state") != "code_label":
            return None, "NON_CODE_LABEL"
        return q5_burden(q5), None
    raise AssertionError(f"unsupported head {head}")


def build_targets(chunk_rows: list[dict[str, Any]], heads: tuple[str, ...]) -> dict[str, dict[str, dict[str, Any]]]:
    targets: dict[str, dict[str, dict[str, Any]]] = {head: {} for head in heads}
    for row in chunk_rows:
        row_key = str(row.get("row_key") or "")
        require(row_key, "EMBEDDER_CONTRIBUTION_REWARD_ROW_KEY_MISSING", "reward tensor row missing row_key")
        for head in heads:
            value, invalid_reason = target_for_head(row, head)
            targets[head][row_key] = {
                "value": value,
                "valid": invalid_reason is None,
                "invalid_reason": invalid_reason,
                "mutation_category": str(row.get("mutation_category") or "unknown"),
            }
    return targets


def load_present_signals(path: Path) -> dict[str, dict[str, dict[str, Any]]]:
    by_embedder: dict[str, dict[str, dict[str, Any]]] = defaultdict(dict)
    for _, row in iter_jsonl(path):
        require(row.get("status") == "present", "EMBEDDER_CONTRIBUTION_UNEXPECTED_SIGNAL_STATUS", "DDA present store contains non-present row", {"status": row.get("status")})
        embedder = str(row.get("embedder_id") or "")
        row_key = str(row.get("row_key") or "")
        require(embedder and row_key, "EMBEDDER_CONTRIBUTION_DDA_KEY_MISSING", "DDA row missing embedder_id or row_key")
        require(row_key not in by_embedder[embedder], "EMBEDDER_CONTRIBUTION_DUPLICATE_DDA_ROW", "duplicate DDA row for embedder", {"embedder_id": embedder, "row_key": row_key})
        by_embedder[embedder][row_key] = row
    return dict(by_embedder)


def load_missing_rows(path: Path) -> tuple[dict[str, int], dict[str, Counter[str]]]:
    counts: Counter[str] = Counter()
    reasons: dict[str, Counter[str]] = defaultdict(Counter)
    for _, row in iter_jsonl(path):
        embedder = str(row.get("embedder_id") or "unknown")
        counts[embedder] += 1
        reasons[embedder][str(row.get("reason") or row.get("status") or "unknown")] += 1
    return dict(sorted(counts.items())), {key: Counter(value) for key, value in sorted(reasons.items())}


def target_profile(targets: dict[str, dict[str, Any]], target_kind: str) -> dict[str, Any]:
    values = [entry["value"] for entry in targets.values() if entry["valid"]]
    if target_kind == "binary":
        counts = Counter(str(int(float(value))) for value in values)
        valid = len(values) > 0 and len(counts) == 2
    elif target_kind == "categorical":
        counts = Counter(str(value) for value in values)
        valid = len(values) > 0 and len(counts) >= 2
    else:
        numeric = np.asarray([float(value) for value in values], dtype=np.float64)
        counts = Counter({"numeric": len(values)})
        valid = numeric.size > 1 and float(np.std(numeric)) > EPS
    return {"valid_target_rows": len(values), "target_counts": dict(sorted(counts.items())), "has_required_variety": valid}


def compute_feature_metric(
    head: str,
    target_kind: str,
    embedder_id: str,
    feature: str,
    signal_rows: dict[str, dict[str, Any]],
    head_targets: dict[str, dict[str, Any]],
    min_label_count: int,
) -> dict[str, Any]:
    feature_values = []
    target_values = []
    skipped_missing_feature = 0
    skipped_invalid_target = 0
    for row_key, target in head_targets.items():
        signal = signal_rows.get(row_key)
        if signal is None:
            continue
        if target.get("valid") is not True:
            skipped_invalid_target += 1
            continue
        feature_value = finite_number((signal.get("signals") or {}).get(feature))
        if feature_value is None:
            skipped_missing_feature += 1
            continue
        feature_values.append(feature_value)
        target_values.append(target["value"])
    row: dict[str, Any] = {
        "schema_version": SCHEMA_VERSION,
        "artifact_kind": "python_embedder_feature_contribution_metric",
        "task_id": TASK_ID,
        "formula_version": FORMULA_VERSION,
        "q_head": head,
        "target_kind": target_kind,
        "embedder_id": embedder_id,
        "feature": feature,
        "finite_support_rows": len(feature_values),
        "skipped_invalid_target_rows": skipped_invalid_target,
        "skipped_missing_feature_rows": skipped_missing_feature,
        "association_score": None,
        "association_method": None,
        "valid": False,
        "invalid_reason": None,
    }
    if len(feature_values) < min_label_count:
        row["invalid_reason"] = "LOW_FINITE_SUPPORT"
        return row
    x = np.asarray(feature_values, dtype=np.float64)
    if target_kind == "binary":
        y = np.asarray([float(value) for value in target_values], dtype=np.float64)
        class_counts = Counter(int(value) for value in y.tolist())
        if len(class_counts) < 2:
            row["invalid_reason"] = "DEGENERATE_BINARY_TARGET"
            return row
        if min(class_counts.values()) < min_label_count:
            row["invalid_reason"] = "LOW_BINARY_CLASS_SUPPORT"
            row["target_counts"] = dict(sorted((str(key), int(value)) for key, value in class_counts.items()))
            return row
        score, reason = pearson_abs(x, y)
        row.update({"association_score": score, "association_method": "abs_pearson_binary", "invalid_reason": reason})
    elif target_kind == "categorical":
        labels = [str(value) for value in target_values]
        label_counts = Counter(labels)
        if len(label_counts) < 2:
            row["invalid_reason"] = "DEGENERATE_CATEGORICAL_TARGET"
            row["target_counts"] = dict(sorted(label_counts.items()))
            return row
        score, reason = correlation_ratio_eta2(x, labels)
        row.update({"association_score": score, "association_method": "correlation_ratio_eta2", "invalid_reason": reason, "target_counts": dict(sorted(label_counts.items()))})
    else:
        y = np.asarray([float(value) for value in target_values], dtype=np.float64)
        score, reason = pearson_abs(x, y)
        row.update({"association_score": score, "association_method": "abs_pearson_numeric", "invalid_reason": reason})
    row["valid"] = row["association_score"] is not None and row["invalid_reason"] is None
    return row


def high_redundancy_pairs(pairwise_rows: list[dict[str, Any]], matrix_rows: list[dict[str, Any]], threshold: float) -> tuple[list[dict[str, Any]], dict[str, set[str]]]:
    matrix_by_pair = {
        "|".join(sorted([str(row.get("left_embedder_id") or ""), str(row.get("right_embedder_id") or "")])): row
        for row in matrix_rows
        if row.get("valid") is True
    }
    out = []
    shared: dict[str, set[str]] = defaultdict(set)
    for row in pairwise_rows:
        if row.get("valid") is not True:
            continue
        left = str(row.get("left_embedder_id") or "")
        right = str(row.get("right_embedder_id") or "")
        if not left or not right:
            continue
        pair_key = "|".join(sorted([left, right]))
        redundancy = safe_float(row.get("redundancy_score")) or 0.0
        matrix = matrix_by_pair.get(pair_key) or {}
        matrix_redundancy = safe_float(matrix.get("redundancy_score"))
        collapsed = row.get("collapsed_duplicate_candidate") is True
        if redundancy < threshold and not collapsed and (matrix_redundancy is None or matrix_redundancy < threshold):
            continue
        shared[left].add(right)
        shared[right].add(left)
        out.append(
            {
                "schema_version": SCHEMA_VERSION,
                "artifact_kind": "python_embedder_shared_contribution_pair",
                "task_id": TASK_ID,
                "formula_version": FORMULA_VERSION,
                "pair_key": pair_key,
                "left_embedder_id": left,
                "right_embedder_id": right,
                "dda_redundancy_score": redundancy,
                "matrix_redundancy_score": matrix_redundancy,
                "dda_disagreement_score": row.get("disagreement_score"),
                "matrix_disagreement_score": matrix.get("disagreement_score"),
                "overlap_rows": row.get("overlap_rows"),
                "collapsed_duplicate_candidate": collapsed,
                "action": "share_credit_not_double_count",
            }
        )
    return sorted(out, key=lambda item: (-float(item.get("dda_redundancy_score") or 0.0), item["pair_key"])), shared


def contribution_rows(
    chunk_rows: list[dict[str, Any]],
    signal_by_embedder: dict[str, dict[str, dict[str, Any]]],
    missing_counts: dict[str, int],
    missing_reasons: dict[str, Counter[str]],
    shared_by_embedder: dict[str, set[str]],
    expected_embedders: list[str],
    min_label_count: int,
) -> tuple[list[dict[str, Any]], list[dict[str, Any]], dict[str, Any]]:
    heads = ("q1", "q2", "q3", "q4", "q5")
    kinds = {"q1": "binary", "q2": "binary", "q3": "categorical", "q4": "categorical", "q5": "numeric"}
    targets = build_targets(chunk_rows, heads)
    target_profiles = {head: target_profile(targets[head], kinds[head]) for head in heads}
    feature_rows: list[dict[str, Any]] = []
    head_rows: list[dict[str, Any]] = []
    for head in heads:
        for embedder in expected_embedders:
            signals = signal_by_embedder.get(embedder, {})
            metrics = [
                compute_feature_metric(head, kinds[head], embedder, feature, signals, targets[head], min_label_count)
                for feature in SCALAR_FEATURES
            ]
            feature_rows.extend(metrics)
            valid_scores = [float(row["association_score"]) for row in metrics if row.get("valid") is True and row.get("association_score") is not None]
            raw_score = top_mean(valid_scores)
            shared_with = sorted(shared_by_embedder.get(embedder, set()))
            shared_credit_divisor = 1 + len(shared_with)
            shared_adjusted = (raw_score / shared_credit_divisor) if raw_score is not None else None
            valid = raw_score is not None
            invalid_reason = None if valid else "NO_VALID_FEATURE_ASSOCIATIONS"
            row = {
                "schema_version": SCHEMA_VERSION,
                "artifact_kind": "python_embedder_q_head_contribution",
                "task_id": TASK_ID,
                "formula_version": FORMULA_VERSION,
                "q_head": head,
                "target_kind": kinds[head],
                "embedder_id": embedder,
                "present_signal_rows": len(signals),
                "missing_signal_rows": int(missing_counts.get(embedder, 0)),
                "missing_signal_reasons": dict(sorted(missing_reasons.get(embedder, Counter()).items())),
                "excluded_missing_rows": int(missing_counts.get(embedder, 0)),
                "target_profile": target_profiles[head],
                "valid_feature_metric_count": len(valid_scores),
                "feature_metric_count": len(metrics),
                "raw_contribution_score": raw_score,
                "raw_contribution_band": confidence_band(valid_scores),
                "shared_with_embedder_ids": shared_with,
                "shared_credit_divisor": shared_credit_divisor,
                "shared_adjusted_contribution_score": shared_adjusted,
                "valid": valid,
                "invalid_reason": invalid_reason,
            }
            row["contribution_id"] = sha256_text({"head": head, "embedder": embedder, "score": raw_score})
            head_rows.append(row)
    return head_rows, feature_rows, {"target_profiles": target_profiles}


def stratified_rows(
    chunk_rows: list[dict[str, Any]],
    signal_by_embedder: dict[str, dict[str, dict[str, Any]]],
    expected_embedders: list[str],
    min_label_count: int,
) -> list[dict[str, Any]]:
    rows = []
    by_category: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for row in chunk_rows:
        by_category[str(row.get("mutation_category") or "unknown")].append(row)
    for category, subset in sorted(by_category.items()):
        targets = build_targets(subset, ("q2",))
        for embedder in expected_embedders:
            metrics = [
                compute_feature_metric("q2", "binary", embedder, feature, signal_by_embedder.get(embedder, {}), targets["q2"], min_label_count)
                for feature in SCALAR_FEATURES
            ]
            valid_scores = [float(row["association_score"]) for row in metrics if row.get("valid") is True and row.get("association_score") is not None]
            rows.append(
                {
                    "schema_version": SCHEMA_VERSION,
                    "artifact_kind": "python_embedder_stratified_q2_contribution",
                    "task_id": TASK_ID,
                    "formula_version": FORMULA_VERSION,
                    "mutation_category": category,
                    "embedder_id": embedder,
                    "row_count": len(subset),
                    "valid_feature_metric_count": len(valid_scores),
                    "raw_contribution_score": top_mean(valid_scores),
                    "valid": bool(valid_scores),
                    "invalid_reason": None if valid_scores else "INSUFFICIENT_STRATIFIED_LABEL_VARIETY_OR_SUPPORT",
                }
            )
    return rows


def build_estimator_validity(head_rows: list[dict[str, Any]], feature_rows: list[dict[str, Any]], shared_pairs: list[dict[str, Any]]) -> list[dict[str, Any]]:
    rows = []
    feature_invalid = Counter(str(row.get("invalid_reason") or "valid") for row in feature_rows)
    head_invalid = Counter(str(row.get("invalid_reason") or "valid") for row in head_rows)
    rows.append(
        {
            "schema_version": SCHEMA_VERSION,
            "artifact_kind": "python_embedder_estimator_validity",
            "task_id": TASK_ID,
            "formula_version": FORMULA_VERSION,
            "case": "feature_metric_validity",
            "before": {"feature_metric_rows": len(feature_rows)},
            "action": "count feature metric invalid reasons after scoring",
            "after": {"invalid_reason_counts": dict(sorted(feature_invalid.items()))},
            "pass": feature_invalid.get("valid", 0) > 0 and sum(feature_invalid.values()) == len(feature_rows),
        }
    )
    rows.append(
        {
            "schema_version": SCHEMA_VERSION,
            "artifact_kind": "python_embedder_estimator_validity",
            "task_id": TASK_ID,
            "formula_version": FORMULA_VERSION,
            "case": "head_contribution_validity",
            "before": {"head_embedder_rows": len(head_rows)},
            "action": "verify every q_head/embedder row is present and validity is explicit",
            "after": {"invalid_reason_counts": dict(sorted(head_invalid.items()))},
            "pass": sum(head_invalid.values()) == len(head_rows) and len(head_rows) == 60,
        }
    )
    rows.append(
        {
            "schema_version": SCHEMA_VERSION,
            "artifact_kind": "python_embedder_estimator_validity",
            "task_id": TASK_ID,
            "formula_version": FORMULA_VERSION,
            "case": "shared_credit_guard",
            "before": {"shared_pairs": len(shared_pairs)},
            "action": "detect high-redundancy pairs and mark them for shared credit",
            "after": {"actions": Counter(str(row.get("action")) for row in shared_pairs)},
            "pass": bool(shared_pairs) and all(row.get("action") == "share_credit_not_double_count" for row in shared_pairs),
        }
    )
    return rows


def edge_cases(
    head_rows: list[dict[str, Any]],
    feature_rows: list[dict[str, Any]],
    shared_pairs: list[dict[str, Any]],
    missing_counts: dict[str, int],
) -> list[dict[str, Any]]:
    too_few_targets = {"a": {"valid": True, "value": 1.0}, "b": {"valid": True, "value": 1.0}}
    too_few_metric = compute_feature_metric(
        "q2",
        "binary",
        "synthetic",
        "vector_norm_l2",
        {
            "a": {"signals": {"vector_norm_l2": 0.1}},
            "b": {"signals": {"vector_norm_l2": 0.2}},
        },
        too_few_targets,
        20,
    )
    q1_rows = [row for row in head_rows if row["q_head"] == "q1"]
    missing_total = sum(missing_counts.values())
    excluded_missing_total = sum(int(row.get("excluded_missing_rows") or 0) for row in head_rows if row["q_head"] == "q2")
    return [
        {
            "case": "too_few_labels_fail_closed",
            "before": {"target_rows": len(too_few_targets), "min_label_count": 20},
            "action": "score synthetic undersized binary target set",
            "after": too_few_metric,
            "expected": "metric invalid; no fabricated contribution",
            "pass": too_few_metric["valid"] is False and too_few_metric["invalid_reason"] == "LOW_FINITE_SUPPORT",
        },
        {
            "case": "correlated_embedders_share_contribution",
            "before": {"shared_pair_threshold_rows": len(shared_pairs)},
            "action": "inspect DDA/matrix redundancy pairs",
            "after": {"shared_pairs": shared_pairs[:5]},
            "expected": "high-redundancy pairs are marked share_credit_not_double_count",
            "pass": bool(shared_pairs) and all(row.get("action") == "share_credit_not_double_count" for row in shared_pairs),
        },
        {
            "case": "quarantined_missing_rows_excluded_from_scoring",
            "before": {"missing_signal_rows": missing_total},
            "action": "sum excluded_missing_rows from q2 head contribution records",
            "after": {"q2_excluded_missing_rows": excluded_missing_total},
            "expected": "missing rows are carried as masks and not imputed as signal values",
            "pass": missing_total > 0 and excluded_missing_total == missing_total,
        },
        {
            "case": "q1_all_true_existence_head_is_not_overclaimed",
            "before": {"q1_rows": len(q1_rows)},
            "action": "verify degenerate Q1 target yields explicit invalid contribution rows",
            "after": {"valid_q1_rows": sum(1 for row in q1_rows if row["valid"]), "invalid_q1_rows": sum(1 for row in q1_rows if not row["valid"])},
            "expected": "Q1 contribution is present but invalid until negative existence examples are added",
            "pass": len(q1_rows) == 12 and all(row["valid"] is False for row in q1_rows),
        },
        {
            "case": "feature_metrics_cover_all_q_heads_and_embedders",
            "before": {"expected_rows": 5 * 12 * len(SCALAR_FEATURES)},
            "action": "read feature contribution metric row count",
            "after": {"feature_metric_rows": len(feature_rows)},
            "expected": "one feature metric row per q_head/embedder/scalar feature",
            "pass": len(feature_rows) == 5 * 12 * len(SCALAR_FEATURES),
        },
    ]


def report_summary(
    head_rows: list[dict[str, Any]],
    feature_rows: list[dict[str, Any]],
    shared_pairs: list[dict[str, Any]],
    stratified: list[dict[str, Any]],
) -> dict[str, Any]:
    top_by_head = {}
    for head in ("q1", "q2", "q3", "q4", "q5"):
        valid = [row for row in head_rows if row["q_head"] == head and row["valid"]]
        top_by_head[head] = sorted(
            [
                {
                    "embedder_id": row["embedder_id"],
                    "raw_contribution_score": row["raw_contribution_score"],
                    "shared_adjusted_contribution_score": row["shared_adjusted_contribution_score"],
                    "valid_feature_metric_count": row["valid_feature_metric_count"],
                    "shared_with_embedder_ids": row["shared_with_embedder_ids"],
                }
                for row in valid
            ],
            key=lambda row: (-(row["shared_adjusted_contribution_score"] or 0.0), row["embedder_id"]),
        )[:5]
    valid_head_rows = [row for row in head_rows if row["valid"]]
    return {
        "head_embedder_rows": len(head_rows),
        "valid_head_embedder_rows": len(valid_head_rows),
        "feature_metric_rows": len(feature_rows),
        "valid_feature_metric_rows": sum(1 for row in feature_rows if row.get("valid") is True),
        "shared_contribution_pair_rows": len(shared_pairs),
        "stratified_contribution_rows": len(stratified),
        "top_by_q_head": top_by_head,
    }


def command_python_embedder_contribution(args: argparse.Namespace) -> int:
    started = utc_now()
    if args.clean_output and args.output_root.exists():
        shutil.rmtree(args.output_root)
    args.output_root.mkdir(parents=True, exist_ok=True)
    args.fsv_root.mkdir(parents=True, exist_ok=True)
    before = {"output_root": tree_state(args.output_root), "fsv_root": tree_state(args.fsv_root)}

    expected_embedders = parse_embedders(args.expected_embedders)
    _, reward_fsv_sha = load_passing_json(args.reward_fsv, "EMBEDDER_CONTRIBUTION_REWARD_FSV_NOT_PASSING")
    _, dda_fsv_sha = load_passing_json(args.dda_fsv, "EMBEDDER_CONTRIBUTION_DDA_FSV_NOT_PASSING")
    _, pairwise_fsv_sha = load_passing_json(args.pairwise_fsv, "EMBEDDER_CONTRIBUTION_PAIRWISE_FSV_NOT_PASSING")
    _, tct_fsv_sha = load_passing_json(args.tct_fsv, "EMBEDDER_CONTRIBUTION_TCT_FSV_NOT_PASSING")
    _, matrix_fsv_sha = load_passing_json(args.matrix_fsv, "EMBEDDER_CONTRIBUTION_MATRIX_FSV_NOT_PASSING")

    chunk_rows = load_jsonl(args.reward_root / "chunk_reward_tensors.jsonl")
    signal_by_embedder = load_present_signals(args.dda_root / "per_embedder_signals.jsonl")
    missing_counts, missing_reasons = load_missing_rows(args.dda_root / "missing_signal_states.jsonl")
    pairwise_rows = load_jsonl(args.pairwise_root / "pairwise_interactions.jsonl")
    matrix_rows = load_jsonl(args.matrix_root / "pairwise_metrics.jsonl")

    require(len(chunk_rows) == args.expected_chunk_count, "EMBEDDER_CONTRIBUTION_UNEXPECTED_CHUNK_COUNT", "reward tensor row count does not match expected smoke count", {"expected": args.expected_chunk_count, "actual": len(chunk_rows)})
    require(set(expected_embedders).issubset(set(signal_by_embedder) | set(missing_counts)), "EMBEDDER_CONTRIBUTION_MISSING_EXPECTED_EMBEDDER", "one or more expected embedders are absent from DDA stores", {"expected": expected_embedders, "present": sorted(signal_by_embedder)})

    shared_pairs, shared_by_embedder = high_redundancy_pairs(pairwise_rows, matrix_rows, args.shared_redundancy_threshold)
    head_rows, feature_rows, target_meta = contribution_rows(
        chunk_rows,
        signal_by_embedder,
        missing_counts,
        missing_reasons,
        shared_by_embedder,
        expected_embedders,
        args.min_label_count,
    )
    stratified = stratified_rows(chunk_rows, signal_by_embedder, expected_embedders, args.min_label_count)
    validity = build_estimator_validity(head_rows, feature_rows, shared_pairs)
    edges = edge_cases(head_rows, feature_rows, shared_pairs, missing_counts)
    summary = report_summary(head_rows, feature_rows, shared_pairs, stratified)

    report = {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "artifact_kind": ARTIFACT_KIND,
        "formula_version": FORMULA_VERSION,
        "created_at_utc": utc_now(),
        "scope": "smoke_1000_chunks",
        "source_of_truth": {
            "output_root": str(args.output_root),
            "reward_root": str(args.reward_root),
            "dda_root": str(args.dda_root),
            "pairwise_root": str(args.pairwise_root),
            "matrix_root": str(args.matrix_root),
        },
        "source_hashes": {
            "reward_fsv_sha256": reward_fsv_sha,
            "dda_fsv_sha256": dda_fsv_sha,
            "pairwise_fsv_sha256": pairwise_fsv_sha,
            "tct_fsv_sha256": tct_fsv_sha,
            "matrix_fsv_sha256": matrix_fsv_sha,
            "chunk_reward_tensors_sha256": sha256_file(args.reward_root / "chunk_reward_tensors.jsonl"),
            "per_embedder_signals_sha256": sha256_file(args.dda_root / "per_embedder_signals.jsonl"),
            "missing_signal_states_sha256": sha256_file(args.dda_root / "missing_signal_states.jsonl"),
            "pairwise_interactions_sha256": sha256_file(args.pairwise_root / "pairwise_interactions.jsonl"),
            "matrix_pairwise_metrics_sha256": sha256_file(args.matrix_root / "pairwise_metrics.jsonl"),
        },
        "estimator_policy": {
            "min_label_count": args.min_label_count,
            "shared_redundancy_threshold": args.shared_redundancy_threshold,
            "shared_credit_policy": "share_credit_not_double_count",
            "missing_signal_policy": "mask_and_count; never impute as zero",
            "q5_numeric_target_formula": "log1p(source_byte_len)+0.1*line_span+5*fail_to_pass_count+0.25*pass_to_pass_count",
            "scope_limit": "smoke result only; full-run contribution must be recomputed after #199 final pass",
        },
        "target_meta": target_meta,
        "summary": summary,
    }

    write_jsonl(args.output_root / "per_head_embedder_contribution.jsonl", head_rows)
    write_jsonl(args.output_root / "feature_contribution_metrics.jsonl", feature_rows)
    write_jsonl(args.output_root / "shared_contribution_pairs.jsonl", shared_pairs)
    write_jsonl(args.output_root / "stratified_contribution.jsonl", stratified)
    write_jsonl(args.output_root / "estimator_validity.jsonl", validity)
    write_jsonl(args.output_root / "edge_case_states.jsonl", edges)
    dump_json(args.output_root / "embedder_contribution_report.json", report)

    after = {"output_root": tree_state(args.output_root), "fsv_root": tree_state(args.fsv_root)}
    readback_counts = {
        "per_head_embedder_contribution": sum(1 for _ in iter_jsonl(args.output_root / "per_head_embedder_contribution.jsonl")),
        "feature_contribution_metrics": sum(1 for _ in iter_jsonl(args.output_root / "feature_contribution_metrics.jsonl")),
        "shared_contribution_pairs": sum(1 for _ in iter_jsonl(args.output_root / "shared_contribution_pairs.jsonl")),
        "stratified_contribution": sum(1 for _ in iter_jsonl(args.output_root / "stratified_contribution.jsonl")),
        "estimator_validity": sum(1 for _ in iter_jsonl(args.output_root / "estimator_validity.jsonl")),
        "edge_case_states": sum(1 for _ in iter_jsonl(args.output_root / "edge_case_states.jsonl")),
    }
    readback_report = read_json(args.output_root / "embedder_contribution_report.json")
    passes = (
        readback_counts["per_head_embedder_contribution"] == 5 * len(expected_embedders)
        and readback_counts["feature_contribution_metrics"] == 5 * len(expected_embedders) * len(SCALAR_FEATURES)
        and readback_counts["shared_contribution_pairs"] >= 1
        and readback_counts["stratified_contribution"] == 8 * len(expected_embedders)
        and readback_counts["estimator_validity"] >= 3
        and readback_counts["edge_case_states"] >= 4
        and readback_report["summary"]["valid_head_embedder_rows"] >= len(expected_embedders)
        and readback_report["summary"]["top_by_q_head"]["q2"]
        and all(case["pass"] for case in edges)
        and all(row["pass"] for row in validity)
    )
    fsv = {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "artifact_kind": f"{ARTIFACT_KIND}_fsv",
        "formula_version": FORMULA_VERSION,
        "started_at_utc": started,
        "created_at_utc": utc_now(),
        "passes": passes,
        "before_state": before,
        "after_state": after,
        "source_of_truth": {
            **report["source_of_truth"],
            "report": str(args.output_root / "embedder_contribution_report.json"),
            "fsv": str(args.fsv_root / "python_embedder_contribution_fsv.json"),
        },
        "trigger": {"command": "python3 scripts/swebench_lite_ops.py python-embedder-contribution"},
        "readback": {
            **readback_counts,
            "report_sha256": sha256_file(args.output_root / "embedder_contribution_report.json"),
            "per_head_embedder_contribution_sha256": sha256_file(args.output_root / "per_head_embedder_contribution.jsonl"),
            "feature_contribution_metrics_sha256": sha256_file(args.output_root / "feature_contribution_metrics.jsonl"),
            "shared_contribution_pairs_sha256": sha256_file(args.output_root / "shared_contribution_pairs.jsonl"),
            "artifact_state": artifact_state(args.output_root),
            "summary": readback_report["summary"],
        },
        "edge_cases": edges,
        "estimator_validity": validity,
    }
    dump_json(args.fsv_root / "python_embedder_contribution_fsv.json", fsv)
    final = read_json(args.fsv_root / "python_embedder_contribution_fsv.json")
    require(final.get("passes") is True, "EMBEDDER_CONTRIBUTION_FSV_FAILED", "embedder contribution FSV failed", {"path": str(args.fsv_root / "python_embedder_contribution_fsv.json")})
    print(json.dumps(final, sort_keys=True, indent=2))
    return 0


def add_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--reward-root", type=Path, default=DEFAULT_REWARD_ROOT)
    parser.add_argument("--dda-root", type=Path, default=DEFAULT_DDA_ROOT)
    parser.add_argument("--pairwise-root", type=Path, default=DEFAULT_PAIRWISE_ROOT)
    parser.add_argument("--matrix-root", type=Path, default=DEFAULT_MATRIX_ROOT)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--fsv-root", type=Path, default=DEFAULT_FSV_ROOT)
    parser.add_argument("--reward-fsv", type=Path, default=DEFAULT_REWARD_FSV)
    parser.add_argument("--dda-fsv", type=Path, default=DEFAULT_DDA_FSV)
    parser.add_argument("--pairwise-fsv", type=Path, default=DEFAULT_PAIRWISE_FSV)
    parser.add_argument("--tct-fsv", type=Path, default=DEFAULT_TCT_FSV)
    parser.add_argument("--matrix-fsv", type=Path, default=DEFAULT_MATRIX_FSV)
    parser.add_argument("--expected-embedders", default=DEFAULT_EXPECTED_EMBEDDERS)
    parser.add_argument("--expected-chunk-count", type=int, default=1000)
    parser.add_argument("--min-label-count", type=int, default=20)
    parser.add_argument("--shared-redundancy-threshold", type=float, default=0.85)
    parser.add_argument("--clean-output", action="store_true")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    add_arguments(parser)
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        return command_python_embedder_contribution(args)
    except EmbedderContributionError as exc:
        print(f"PYTHON_EMBEDDER_CONTRIBUTION_FAILED: {exc}")
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
