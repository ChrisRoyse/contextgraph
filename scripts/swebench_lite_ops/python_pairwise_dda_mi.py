"""Compute pairwise DDA interactions and MI audit from Python DDA signals."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import shutil
import sys
from collections import Counter
from itertools import combinations
from pathlib import Path
from typing import Any

import numpy as np

from .smoke_intelligence_matrices import (
    dump_json,
    iter_jsonl,
    read_json,
    require,
    sha256_file,
    tree_state,
    write_jsonl,
)


SCHEMA_VERSION = 1
TASK_ID = "TASK-PY-DATA-018"
ARTIFACT_KIND = "python_pairwise_dda_mi_audit"
FORMULA_VERSION = "python_pairwise_dda_mi_v1"
DEFAULT_INPUT_ROOT = Path("/zfs/archive/contextgraph/corpus/python-dda-signals-v1/per-embedder")
DEFAULT_OUTPUT_ROOT = Path("/zfs/archive/contextgraph/corpus/python-dda-signals-v1/pairwise")
DEFAULT_FSV_ROOT = Path("/zfs/archive/contextgraph/fsv/task-py-data-018-pairwise-dda-mi-fsv")
EXPECTED_EMBEDDERS = ("e1", "e6", "e7", "e8", "e10", "e12", "e13", "e14", "e2", "e3", "e4", "e9")
EPS = 1.0e-12
SCALAR_FEATURES = (
    "cosine_to_category_kind_centroid",
    "lexical_delta_score",
    "token_delta_score",
    "structural_delta_score",
    "vector_abs_mean",
    "vector_entropy_proxy",
    "vector_norm_l2",
    "vector_std",
    "vector_zero_fraction",
    "source_byte_len",
    "token_estimate",
    "line_span",
    "oracle_fail_to_pass_count",
    "oracle_pass_to_pass_count",
)


class PairwiseDdaError(RuntimeError):
    pass


def utc_now() -> str:
    import datetime as _dt

    return _dt.datetime.now(_dt.timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def canonical_json(payload: Any) -> str:
    return json.dumps(payload, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def sha256_text(payload: Any) -> str:
    return "sha256:" + hashlib.sha256(canonical_json(payload).encode("utf-8")).hexdigest()


def safe_float(value: float | np.floating[Any]) -> float | None:
    out = float(value)
    if math.isfinite(out):
        return out
    return None


def finite_number(value: Any) -> float | None:
    if isinstance(value, bool):
        return None
    if isinstance(value, (int, float, np.integer, np.floating)):
        out = float(value)
        if math.isfinite(out):
            return out
    return None


def quantile_bins(values: np.ndarray, bins: int) -> tuple[np.ndarray | None, str | None]:
    finite = values[np.isfinite(values)]
    if finite.size < bins * 3:
        return None, "TOO_FEW_FINITE_VALUES"
    quantiles = np.unique(np.quantile(finite, np.linspace(0.0, 1.0, bins + 1)[1:-1]))
    if quantiles.size < 2:
        return None, "DEGENERATE_QUANTILES"
    return np.digitize(values, quantiles, right=False), None


def normalized_mutual_info(x: np.ndarray, y: np.ndarray, bins: int) -> tuple[float | None, str | None]:
    x_bins, x_reason = quantile_bins(x, bins)
    y_bins, y_reason = quantile_bins(y, bins)
    if x_bins is None or y_bins is None:
        return None, x_reason or y_reason
    joint = Counter(zip(x_bins.tolist(), y_bins.tolist()))
    x_count = Counter(x_bins.tolist())
    y_count = Counter(y_bins.tolist())
    n = float(len(x_bins))
    mi = 0.0
    for (xb, yb), count in joint.items():
        pxy = count / n
        px = x_count[xb] / n
        py = y_count[yb] / n
        mi += pxy * math.log(pxy / (px * py))
    hx = -sum((count / n) * math.log(count / n) for count in x_count.values())
    hy = -sum((count / n) * math.log(count / n) for count in y_count.values())
    denom = math.sqrt(hx * hy)
    if denom <= EPS:
        return None, "DEGENERATE_ENTROPY"
    return safe_float(mi / denom), None


def pearson(x: np.ndarray, y: np.ndarray) -> tuple[float | None, str | None]:
    if x.size < 2:
        return None, "TOO_FEW_FINITE_VALUES"
    x_std = float(np.std(x))
    y_std = float(np.std(y))
    if x_std <= EPS or y_std <= EPS:
        return None, "DEGENERATE_VARIANCE"
    return safe_float(float(np.corrcoef(x, y)[0, 1])), None


def mean_abs_delta(x: np.ndarray, y: np.ndarray) -> float | None:
    return safe_float(float(np.mean(np.abs(x - y)))) if x.size else None


def confidence_band(values: list[float]) -> dict[str, Any]:
    if not values:
        return {"mean": None, "lower_95": None, "upper_95": None, "n": 0}
    arr = np.asarray(values, dtype=np.float64)
    mean = float(arr.mean())
    if arr.size == 1:
        return {"mean": safe_float(mean), "lower_95": safe_float(mean), "upper_95": safe_float(mean), "n": 1}
    se = float(arr.std(ddof=1) / math.sqrt(arr.size))
    return {
        "mean": safe_float(mean),
        "lower_95": safe_float(max(0.0, mean - 1.96 * se)),
        "upper_95": safe_float(min(1.0, mean + 1.96 * se)),
        "n": int(arr.size),
    }


def artifact_state(root: Path) -> dict[str, Any]:
    rows = []
    for child in sorted(root.rglob("*")):
        if child.is_file():
            rows.append({"path": str(child), "bytes": child.stat().st_size, "sha256": sha256_file(child)})
    return {"root": str(root), "file_count": len(rows), "byte_count": sum(row["bytes"] for row in rows), "files": rows}


def load_present_signals(path: Path) -> dict[str, dict[str, dict[str, Any]]]:
    by_embedder: dict[str, dict[str, dict[str, Any]]] = {}
    for _, row in iter_jsonl(path):
        require(row.get("status") == "present", f"unexpected non-present row in {path}")
        embedder_id = str(row.get("embedder_id") or "")
        row_key = str(row.get("row_key") or "")
        require(embedder_id and row_key, f"signal row missing embedder_id or row_key in {path}")
        by_embedder.setdefault(embedder_id, {})
        require(row_key not in by_embedder[embedder_id], f"duplicate signal row for {embedder_id} {row_key}")
        by_embedder[embedder_id][row_key] = row
    return by_embedder


def load_missing_counts(path: Path) -> dict[str, int]:
    counts: Counter = Counter()
    for _, row in iter_jsonl(path):
        counts[str(row.get("embedder_id") or "unknown")] += 1
    return dict(sorted(counts.items()))


def feature_arrays(
    left_rows: dict[str, dict[str, Any]],
    right_rows: dict[str, dict[str, Any]],
    keys: list[str],
    feature: str,
) -> tuple[np.ndarray, np.ndarray]:
    left_values = []
    right_values = []
    for key in keys:
        left = finite_number((left_rows[key].get("signals") or {}).get(feature))
        right = finite_number((right_rows[key].get("signals") or {}).get(feature))
        if left is None or right is None:
            continue
        left_values.append(left)
        right_values.append(right)
    return np.asarray(left_values, dtype=np.float64), np.asarray(right_values, dtype=np.float64)


def feature_metric(
    left_rows: dict[str, dict[str, Any]],
    right_rows: dict[str, dict[str, Any]],
    keys: list[str],
    feature: str,
    min_support: int,
    mi_bins: int,
) -> dict[str, Any]:
    x, y = feature_arrays(left_rows, right_rows, keys, feature)
    finite_n = int(x.size)
    row = {
        "feature": feature,
        "finite_overlap_rows": finite_n,
        "valid": finite_n >= min_support,
        "invalid_reason": None if finite_n >= min_support else "LOW_SUPPORT_SIGNAL",
        "pearson": None,
        "pearson_validity": None,
        "abs_pearson": None,
        "norm_quantile_nmi": None,
        "norm_quantile_nmi_validity": None,
        "mean_abs_delta": None,
        "feature_redundancy": None,
        "feature_unique_contribution": None,
    }
    if finite_n < min_support:
        return row
    corr, corr_reason = pearson(x, y)
    nmi, nmi_reason = normalized_mutual_info(x, y, mi_bins)
    abs_corr = abs(corr) if corr is not None else None
    redundancy_inputs = [value for value in (abs_corr, nmi) if value is not None]
    redundancy = float(np.mean(redundancy_inputs)) if redundancy_inputs else None
    unique = max(0.0, 1.0 - redundancy) if redundancy is not None else None
    row.update(
        {
            "pearson": corr,
            "pearson_validity": corr_reason,
            "abs_pearson": safe_float(abs_corr) if abs_corr is not None else None,
            "norm_quantile_nmi": nmi,
            "norm_quantile_nmi_validity": nmi_reason,
            "mean_abs_delta": mean_abs_delta(x, y),
            "feature_redundancy": safe_float(redundancy) if redundancy is not None else None,
            "feature_unique_contribution": safe_float(unique) if unique is not None else None,
        }
    )
    return row


def pair_row(
    left_id: str,
    right_id: str,
    left_rows: dict[str, dict[str, Any]],
    right_rows: dict[str, dict[str, Any]],
    manifest_rows: int,
    min_overlap: int,
    min_support: int,
    mi_bins: int,
) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    left_keys = set(left_rows)
    right_keys = set(right_rows)
    keys = sorted(left_keys & right_keys)
    pair_key = f"{left_id}|{right_id}"
    base = {
        "schema_version": SCHEMA_VERSION,
        "artifact_kind": "python_pairwise_dda_interaction",
        "task_id": TASK_ID,
        "formula_version": FORMULA_VERSION,
        "left_embedder_id": left_id,
        "right_embedder_id": right_id,
        "pair_key": pair_key,
        "pair_id": sha256_text({"formula_version": FORMULA_VERSION, "left": left_id, "right": right_id}),
        "manifest_rows": manifest_rows,
        "left_rows": len(left_rows),
        "right_rows": len(right_rows),
        "overlap_rows": len(keys),
        "missing_from_left": len(right_keys - left_keys),
        "missing_from_right": len(left_keys - right_keys),
        "support_ratio": safe_float(len(keys) / max(manifest_rows, 1)),
    }
    if len(keys) < min_overlap:
        row = {
            **base,
            "valid": False,
            "invalid_reason": "MISSING_PAIR_NO_OVERLAP" if not keys else "LOW_OVERLAP_PAIR",
            "feature_metric_count": 0,
            "valid_feature_metric_count": 0,
            "redundancy_score": None,
            "unique_contribution_score": None,
            "effective_pair_contribution": 0.0,
            "effective_pair_contribution_band": confidence_band([]),
            "disagreement_score": None,
            "blind_spot_ratio": None,
            "collapsed_duplicate_candidate": False,
        }
        return row, []
    feature_rows = [feature_metric(left_rows, right_rows, keys, feature, min_support, mi_bins) for feature in SCALAR_FEATURES]
    valid_feature_rows = [row for row in feature_rows if row["valid"] is True and row["feature_redundancy"] is not None]
    redundancies = [float(row["feature_redundancy"]) for row in valid_feature_rows]
    uniques = [float(row["feature_unique_contribution"]) for row in valid_feature_rows if row["feature_unique_contribution"] is not None]
    redundancy_score = float(np.mean(redundancies)) if redundancies else None
    unique_score = float(np.mean(uniques)) if uniques else None
    missing_asymmetry = abs(base["missing_from_left"] - base["missing_from_right"])
    blind_spot_ratio = missing_asymmetry / max(manifest_rows, 1)
    effective_pair_contribution = (unique_score or 0.0) * float(base["support_ratio"] or 0.0)
    row = {
        **base,
        "valid": bool(valid_feature_rows),
        "invalid_reason": None if valid_feature_rows else "NO_VALID_FEATURE_METRICS",
        "feature_metric_count": len(feature_rows),
        "valid_feature_metric_count": len(valid_feature_rows),
        "redundancy_score": safe_float(redundancy_score) if redundancy_score is not None else None,
        "unique_contribution_score": safe_float(unique_score) if unique_score is not None else None,
        "effective_pair_contribution": safe_float(effective_pair_contribution),
        "effective_pair_contribution_band": confidence_band([float(value) * float(base["support_ratio"] or 0.0) for value in uniques]),
        "disagreement_score": safe_float(1.0 - redundancy_score) if redundancy_score is not None else None,
        "blind_spot_ratio": safe_float(blind_spot_ratio),
        "collapsed_duplicate_candidate": bool(redundancy_score is not None and redundancy_score >= 0.95 and blind_spot_ratio <= 0.01),
    }
    validity = []
    for feature_row in feature_rows:
        validity.append(
            {
                "schema_version": SCHEMA_VERSION,
                "artifact_kind": "python_pairwise_dda_estimator_validity",
                "task_id": TASK_ID,
                "formula_version": FORMULA_VERSION,
                "left_embedder_id": left_id,
                "right_embedder_id": right_id,
                "pair_key": pair_key,
                **feature_row,
            }
        )
    return row, validity


def edge_cases(min_overlap: int, min_support: int, mi_bins: int) -> list[dict[str, Any]]:
    rows_a = {}
    rows_b = {}
    for idx in range(max(min_overlap, min_support, mi_bins * 3) + 5):
        key = f"synthetic|base|chunk-{idx}|source"
        value = float(idx + 1)
        row = {"row_key": key, "signals": {"vector_norm_l2": value, "vector_entropy_proxy": value / 100.0}}
        rows_a[key] = row
        rows_b[key] = {"row_key": key, "signals": {"vector_norm_l2": value, "vector_entropy_proxy": value / 100.0}}
    identical, _ = pair_row("synthetic_a", "synthetic_b", rows_a, rows_b, len(rows_a), min_overlap, min_support, mi_bins)
    missing_pair, _ = pair_row("synthetic_a", "synthetic_empty", rows_a, {}, len(rows_a), min_overlap, min_support, mi_bins)
    low_rows_a = {key: rows_a[key] for key in sorted(rows_a)[: max(1, min_support - 1)]}
    low_rows_b = {key: rows_b[key] for key in sorted(rows_b)[: max(1, min_support - 1)]}
    low_support, _ = pair_row("synthetic_low_a", "synthetic_low_b", low_rows_a, low_rows_b, len(rows_a), min_overlap=1, min_support=min_support, mi_bins=mi_bins)
    return [
        {
            "case": "identical_embeddings_detected",
            "before": {"rows": len(rows_a), "feature": "vector_norm_l2"},
            "action": "pairwise redundancy audit on identical scalar fields",
            "after": {"collapsed_duplicate_candidate": identical["collapsed_duplicate_candidate"], "redundancy_score": identical["redundancy_score"]},
            "expected": "collapsed duplicate candidate with high redundancy",
            "pass": identical["collapsed_duplicate_candidate"] is True and (identical["redundancy_score"] or 0.0) >= 0.95,
        },
        {
            "case": "missing_pair",
            "before": {"left_rows": len(rows_a), "right_rows": 0, "min_overlap": min_overlap},
            "action": "pairwise precondition check",
            "after": {"valid": missing_pair["valid"], "invalid_reason": missing_pair["invalid_reason"]},
            "expected": "MISSING_PAIR_NO_OVERLAP",
            "pass": missing_pair["valid"] is False and missing_pair["invalid_reason"] == "MISSING_PAIR_NO_OVERLAP",
        },
        {
            "case": "extremely_low_support_signal",
            "before": {"overlap_rows": len(low_rows_a), "min_support": min_support},
            "action": "feature metric support audit",
            "after": {"valid": low_support["valid"], "invalid_reason": low_support["invalid_reason"]},
            "expected": "NO_VALID_FEATURE_METRICS",
            "pass": low_support["valid"] is False and low_support["invalid_reason"] == "NO_VALID_FEATURE_METRICS",
        },
    ]


def command_python_pairwise_dda_mi(args: argparse.Namespace) -> int:
    started = utc_now()
    if args.clean_output and args.output_root.exists():
        shutil.rmtree(args.output_root)
    args.output_root.mkdir(parents=True, exist_ok=True)
    args.fsv_root.mkdir(parents=True, exist_ok=True)
    before = {"output_root": tree_state(args.output_root), "fsv_root": tree_state(args.fsv_root)}

    summary_path = args.input_root / "dda_signal_summary.json"
    signal_path = args.input_root / "per_embedder_signals.jsonl"
    missing_path = args.input_root / "missing_signal_states.jsonl"
    input_summary = read_json(summary_path)
    manifest_rows = int((input_summary.get("summary") or {}).get("manifest_rows") or 0)
    expected_embedders = tuple(item.strip() for item in args.expected_embedders.split(",") if item.strip())
    require(manifest_rows > 0, "DDA summary has no manifest rows")
    signals = load_present_signals(signal_path)
    missing_counts = load_missing_counts(missing_path)
    require(set(signals) == set(expected_embedders), f"signal store embedder ids do not match expected panel: {sorted(signals)}")

    pairwise_rows = []
    validity_rows = []
    for left_id, right_id in combinations(expected_embedders, 2):
        row, validity = pair_row(
            left_id,
            right_id,
            signals[left_id],
            signals[right_id],
            manifest_rows,
            args.min_overlap,
            args.min_support,
            args.mi_bins,
        )
        pairwise_rows.append(row)
        validity_rows.extend(validity)

    excluded_rows = [row for row in pairwise_rows if row["valid"] is not True]
    top_redundant = sorted([row for row in pairwise_rows if row["redundancy_score"] is not None], key=lambda row: (-row["redundancy_score"], row["left_embedder_id"], row["right_embedder_id"]))[: args.top_k]
    top_unique = sorted([row for row in pairwise_rows if row["effective_pair_contribution"] is not None], key=lambda row: (-row["effective_pair_contribution"], row["left_embedder_id"], row["right_embedder_id"]))[: args.top_k]
    contribution_values = [float(row["effective_pair_contribution"]) for row in pairwise_rows if row["effective_pair_contribution"] is not None]
    effective_pairwise = float(sum(contribution_values))
    possible_pair_count = len(expected_embedders) * (len(expected_embedders) - 1) // 2
    signal_count_report = {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "artifact_kind": "python_pairwise_dda_effective_signal_count_report",
        "created_at_utc": utc_now(),
        "formula_version": FORMULA_VERSION,
        "input_summary": {
            "input_root": str(args.input_root),
            "summary_sha256": sha256_file(summary_path),
            "per_embedder_signals_sha256": sha256_file(signal_path),
            "missing_signal_states_sha256": sha256_file(missing_path),
            "manifest_rows": manifest_rows,
            "expected_embedder_count": len(expected_embedders),
            "expected_pair_count": possible_pair_count,
            "missing_by_embedder": missing_counts,
        },
        "effective_signal_count": {
            "per_embedder_signal_count": len(expected_embedders),
            "pairwise_upper_bound": possible_pair_count,
            "effective_pairwise_signal_count": safe_float(effective_pairwise),
            "effective_total_signal_count": safe_float(len(expected_embedders) + effective_pairwise),
            "effective_pairwise_confidence_band": confidence_band(contribution_values),
            "valid_pair_count": sum(1 for row in pairwise_rows if row["valid"] is True),
            "excluded_pair_count": len(excluded_rows),
            "collapsed_duplicate_candidate_count": sum(1 for row in pairwise_rows if row["collapsed_duplicate_candidate"] is True),
        },
        "storage_policy": {
            "unbounded_pairwise_chunk_storage": False,
            "stored_rows": "one aggregate interaction row per embedder pair plus per-feature estimator validity rows",
            "reason": "DDA pairwise audit measures effective independence without materializing all chunk-to-chunk pairs",
        },
        "top_redundant_pairs": top_redundant,
        "top_unique_pairs": top_unique,
    }

    edges = edge_cases(args.min_overlap, args.min_support, args.mi_bins)
    write_jsonl(args.output_root / "pairwise_interactions.jsonl", pairwise_rows)
    write_jsonl(args.output_root / "estimator_validity.jsonl", validity_rows)
    write_jsonl(args.output_root / "excluded_pairs.jsonl", excluded_rows)
    write_jsonl(args.output_root / "edge_case_states.jsonl", edges)
    dump_json(args.output_root / "effective_signal_count_report.json", signal_count_report)
    dump_json(args.output_root / "top_redundant_pairs.json", top_redundant)
    dump_json(args.output_root / "top_unique_pairs.json", top_unique)

    after = {"output_root": tree_state(args.output_root), "fsv_root": tree_state(args.fsv_root)}
    readback_pairwise = sum(1 for _ in iter_jsonl(args.output_root / "pairwise_interactions.jsonl"))
    readback_validity = sum(1 for _ in iter_jsonl(args.output_root / "estimator_validity.jsonl"))
    readback_excluded = sum(1 for _ in iter_jsonl(args.output_root / "excluded_pairs.jsonl"))
    readback_edges = list(row for _, row in iter_jsonl(args.output_root / "edge_case_states.jsonl"))
    readback_report = read_json(args.output_root / "effective_signal_count_report.json")
    passes = (
        readback_pairwise == possible_pair_count
        and readback_validity >= possible_pair_count
        and readback_excluded == len(excluded_rows)
        and readback_report.get("effective_signal_count", {}).get("valid_pair_count") == possible_pair_count
        and all(row["valid_feature_metric_count"] > 0 for row in pairwise_rows)
        and all(edge.get("pass") is True for edge in readback_edges)
    )
    fsv = {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "artifact_kind": f"{ARTIFACT_KIND}_fsv",
        "started_at_utc": started,
        "created_at_utc": utc_now(),
        "passes": passes,
        "source_of_truth": {
            "input_root": str(args.input_root),
            "output_root": str(args.output_root),
            "pairwise_interactions": str(args.output_root / "pairwise_interactions.jsonl"),
            "estimator_validity": str(args.output_root / "estimator_validity.jsonl"),
            "effective_signal_count_report": str(args.output_root / "effective_signal_count_report.json"),
            "excluded_pairs": str(args.output_root / "excluded_pairs.jsonl"),
            "edge_case_states": str(args.output_root / "edge_case_states.jsonl"),
            "fsv": str(args.fsv_root / "python_pairwise_dda_mi_fsv.json"),
        },
        "trigger": {
            "command": "python3 scripts/swebench_lite_ops.py python-pairwise-dda-mi",
            "formula_version": FORMULA_VERSION,
            "input_summary_sha256": sha256_file(summary_path),
        },
        "before_state": before,
        "after_state": after,
        "readback": {
            "pairwise_rows": readback_pairwise,
            "expected_pairwise_rows": possible_pair_count,
            "estimator_validity_rows": readback_validity,
            "excluded_pair_rows": readback_excluded,
            "edge_case_rows": len(readback_edges),
            "valid_pair_count": readback_report.get("effective_signal_count", {}).get("valid_pair_count"),
            "effective_pairwise_signal_count": readback_report.get("effective_signal_count", {}).get("effective_pairwise_signal_count"),
            "effective_total_signal_count": readback_report.get("effective_signal_count", {}).get("effective_total_signal_count"),
            "top_redundant_first": top_redundant[0] if top_redundant else None,
            "top_unique_first": top_unique[0] if top_unique else None,
            "artifact_state": artifact_state(args.output_root),
        },
        "edge_cases": readback_edges,
    }
    dump_json(args.fsv_root / "python_pairwise_dda_mi_fsv.json", fsv)
    final = read_json(args.fsv_root / "python_pairwise_dda_mi_fsv.json")
    require(final.get("passes") is True, f"pairwise DDA MI FSV failed: {args.fsv_root / 'python_pairwise_dda_mi_fsv.json'}")
    print(json.dumps(final, sort_keys=True, indent=2))
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input-root", type=Path, default=DEFAULT_INPUT_ROOT)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--fsv-root", type=Path, default=DEFAULT_FSV_ROOT)
    parser.add_argument("--expected-embedders", default=",".join(EXPECTED_EMBEDDERS))
    parser.add_argument("--min-overlap", type=int, default=20)
    parser.add_argument("--min-support", type=int, default=20)
    parser.add_argument("--mi-bins", type=int, default=8)
    parser.add_argument("--top-k", type=int, default=20)
    parser.add_argument("--clean-output", action="store_true")
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        return command_python_pairwise_dda_mi(args)
    except (PairwiseDdaError, RuntimeError) as exc:
        print(f"PYTHON_PAIRWISE_DDA_MI_FAILED: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
