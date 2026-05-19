"""TASK-PY-G-076 oracle-noise and ship-gate power FSV.

This harness is deliberately read-only with respect to model state. It inspects
the current aiwonder corpus/oracle index plus completed FSV metric artifacts and
emits a fail-closed statistical-power report for the Python 300x8 ship gate.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import re
import shutil
from collections import Counter, defaultdict
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


SCHEMA_VERSION = 1
TASK_ID = "TASK-PY-G-076"
ARTIFACT_KIND = "python_oracle_noise_power_fsv"
DEFAULT_CORPUS_INDEX = Path("/zfs/archive/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/index.json")
DEFAULT_ORACLE_STORE = Path("/zfs/archive/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/oracle-store")
DEFAULT_FSV_ROOT = Path("/zfs/archive/contextgraph/fsv/task-py-g-076-oracle-noise-power-fsv")
DEFAULT_METRIC_ARTIFACTS = (
    Path("/zfs/archive/contextgraph/fsv/task-py-g-093-constellation-ablation-suite-fsv/artifacts/constellation_ablation_metrics.jsonl"),
    Path("/zfs/archive/contextgraph/fsv/task-py-g-097-constellation-ablation-with-calibration-fsv/artifacts/constellation_ablation_with_calibration_metrics.jsonl"),
)
DEFAULT_COMPLETED_FSVS = (
    Path("/zfs/archive/contextgraph/fsv/task-py-g-096-constellation-training-objective-fsv/python_constellation_training_objective_fsv.json"),
    Path("/zfs/archive/contextgraph/fsv/task-py-g-093-constellation-ablation-suite-fsv/python_constellation_ablation_suite_fsv.json"),
    Path("/zfs/archive/contextgraph/fsv/task-py-g-097-constellation-ablation-with-calibration-fsv/python_constellation_ablation_with_calibration_fsv.json"),
    Path("/zfs/archive/contextgraph/fsv/task-py-g-057-ood-harvest-fsv/run-20260519T022705Z/ood_harvest_fsv.json"),
)
TARGET_CORRELATION = 0.95
CONFIDENCE_LEVEL = 0.95
Z_95 = 1.959963984540054
EPS = 1.0e-12


class OracleNoisePowerError(RuntimeError):
    pass


def require(condition: bool, code: str, detail: str | None = None) -> None:
    if not condition:
        raise OracleNoisePowerError(code if detail is None else f"{code}: {detail}")


def utc_now() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(1024 * 1024), b""):
            h.update(chunk)
    return "sha256:" + h.hexdigest()


def read_json(path: Path) -> Any:
    with path.open("r", encoding="utf-8") as fh:
        return json.load(fh)


def dump_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    with tmp.open("w", encoding="utf-8") as fh:
        json.dump(value, fh, sort_keys=True, indent=2, separators=(",", ": "))
        fh.write("\n")
    tmp.replace(path)


def iter_jsonl(path: Path):
    with path.open("r", encoding="utf-8") as fh:
        for line_no, line in enumerate(fh, start=1):
            if line.strip():
                yield line_no, json.loads(line)


def tree_state(path: Path) -> dict[str, Any]:
    if not path.exists():
        return {"path": str(path), "exists": False, "file_count": 0, "byte_count": 0, "hash_sample": []}
    files = sorted(p for p in path.rglob("*") if p.is_file())
    return {
        "path": str(path),
        "exists": True,
        "file_count": len(files),
        "byte_count": sum(p.stat().st_size for p in files),
        "hash_sample": [f"{p.relative_to(path)}:{sha256_file(p)}" for p in files[:20]],
        "hash_sample_truncated": len(files) > 20,
    }


def fisher_correlation_ci(r: float, n: int, z: float = Z_95) -> dict[str, Any]:
    if n < 4:
        return {"valid": False, "reason": "TOO_FEW_ROWS_FOR_FISHER_CI", "n": n, "r": r}
    if not math.isfinite(r) or abs(r) >= 1.0:
        return {"valid": False, "reason": "CORRELATION_OUT_OF_OPEN_INTERVAL", "n": n, "r": r}
    transformed = math.atanh(r)
    half_width = z / math.sqrt(n - 3)
    lower = math.tanh(transformed - half_width)
    upper = math.tanh(transformed + half_width)
    return {
        "valid": True,
        "method": "fisher_z_asymptotic_95_ci",
        "n": n,
        "r": r,
        "lower": lower,
        "upper": upper,
        "target": TARGET_CORRELATION,
        "lower_bound_clears_target": lower >= TARGET_CORRELATION,
    }


def wilson_ci(successes: int, n: int, z: float = Z_95) -> dict[str, Any]:
    if n <= 0:
        return {"valid": False, "reason": "EMPTY_BINOMIAL_SAMPLE", "successes": successes, "n": n}
    p = successes / n
    denom = 1.0 + (z * z / n)
    center = (p + (z * z) / (2.0 * n)) / denom
    half = z * math.sqrt((p * (1.0 - p) / n) + ((z * z) / (4.0 * n * n))) / denom
    return {"valid": True, "method": "wilson_95_ci", "successes": successes, "n": n, "rate": p, "lower": center - half, "upper": center + half}


def clopper_upper_zero_failures(n: int, alpha: float = 0.05) -> float:
    require(n > 0, "MEJEPA_ORACLE_POWER_NONPOSITIVE_SAMPLE")
    return 1.0 - (alpha ** (1.0 / n))


def min_zero_failure_sample(max_upper: float = 0.05, alpha: float = 0.05) -> int:
    n = 1
    while clopper_upper_zero_failures(n, alpha) > max_upper:
        n += 1
    return n


def min_n_for_observed_correlation(observed_r: float) -> int | None:
    if observed_r <= TARGET_CORRELATION or observed_r >= 1.0:
        return None
    n = 4
    while n < 100000:
        ci = fisher_correlation_ci(observed_r, n)
        if ci.get("valid") and ci["lower"] >= TARGET_CORRELATION:
            return n
        n += 1
    return None


def load_corpus_index(path: Path) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    require(path.exists(), "MEJEPA_ORACLE_POWER_CORPUS_INDEX_MISSING", str(path))
    payload = read_json(path)
    entries = list(payload.get("entries") or [])
    require(entries, "MEJEPA_ORACLE_POWER_CORPUS_INDEX_EMPTY", str(path))
    required = {"task_id", "category", "bucket", "oracle_all_passed", "oracle_verdict_sha256"}
    category_counts: Counter[str] = Counter()
    bucket_counts: Counter[str] = Counter()
    oracle_counts: Counter[str] = Counter()
    per_category: dict[str, Counter[str]] = defaultdict(Counter)
    per_bucket_category: dict[str, Counter[str]] = defaultdict(Counter)
    duplicate_keys: list[str] = []
    seen: set[tuple[str, str]] = set()
    missing_required: Counter[str] = Counter()
    for row in entries:
        for key in required:
            if key not in row:
                missing_required[key] += 1
        task_id = str(row.get("task_id") or "")
        category = str(row.get("category") or "")
        bucket = str(row.get("bucket") or "")
        key = (task_id, category)
        if key in seen:
            duplicate_keys.append(f"{task_id}:{category}")
        seen.add(key)
        category_counts[category] += 1
        bucket_counts[bucket] += 1
        oracle_name = "pass" if row.get("oracle_all_passed") is True else "fail_or_error"
        oracle_counts[oracle_name] += 1
        per_category[category][oracle_name] += 1
        per_bucket_category[f"{bucket}:{category}"][oracle_name] += 1
    summary = {
        "path": str(path),
        "sha256": sha256_file(path),
        "complete": bool(payload.get("complete")),
        "entry_count": len(entries),
        "unique_task_category_keys": len(seen),
        "duplicate_key_count": len(duplicate_keys),
        "duplicate_key_sample": duplicate_keys[:20],
        "missing_required_fields": dict(sorted(missing_required.items())),
        "category_counts": dict(sorted(category_counts.items())),
        "bucket_counts": dict(sorted(bucket_counts.items())),
        "oracle_counts": dict(sorted(oracle_counts.items())),
        "per_category_oracle_counts": {key: dict(sorted(counts.items())) for key, counts in sorted(per_category.items())},
        "per_bucket_category_oracle_counts": {key: dict(sorted(counts.items())) for key, counts in sorted(per_bucket_category.items())},
    }
    return entries, summary


def oracle_store_inventory(path: Path) -> dict[str, Any]:
    state = tree_state(path)
    if not path.exists():
        return {**state, "cf_names_from_log": [], "row_readback_available": False}
    cf_names: set[str] = set()
    log_path = path / "LOG"
    if log_path.exists():
        text = log_path.read_text(encoding="utf-8", errors="replace")
        for match in re.finditer(r"Column family \[([^\]]+)\]", text):
            cf_names.add(match.group(1))
    return {
        **state,
        "cf_names_from_log": sorted(cf_names),
        "row_readback_available": False,
        "row_readback_note": "This harness does not decode RocksDB internals; it treats index.json as the exported oracle label SoT and records missing repeated-run rows fail-closed.",
    }


def metric_records(paths: list[Path]) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    inventories = []
    rows = []
    for path in paths:
        if not path.exists():
            inventories.append({"path": str(path), "exists": False})
            continue
        count = 0
        for _line_no, row in iter_jsonl(path):
            count += 1
            rows.append({"source_path": str(path), "source_sha256": sha256_file(path), **row})
        inventories.append({"path": str(path), "exists": True, "sha256": sha256_file(path), "row_count": count})
    return rows, inventories


def collect_metric_cis(rows: list[dict[str, Any]]) -> list[dict[str, Any]]:
    out: list[dict[str, Any]] = []
    for row in rows:
        metric_sets = [("test_metrics", row.get("test_metrics") or {})]
        for cell, metrics in sorted((row.get("per_cell_metrics") or {}).items()):
            metric_sets.append((f"per_cell_metrics.{cell}", metrics or {}))
        for metric_name, metrics in metric_sets:
            corr = metrics.get("prediction_oracle_correlation")
            n = int(metrics.get("row_count") or 0)
            validity = metrics.get("prediction_oracle_correlation_validity")
            if isinstance(corr, (int, float)) and math.isfinite(float(corr)):
                ci = fisher_correlation_ci(float(corr), n)
            else:
                ci = {"valid": False, "reason": validity or "MISSING_CORRELATION", "n": n, "r": None}
            accuracy = metrics.get("accuracy_at_0_5")
            accuracy_ci = None
            if isinstance(accuracy, (int, float)) and n > 0:
                accuracy_ci = wilson_ci(round(float(accuracy) * n), n)
            out.append(
                {
                    "source_path": row.get("source_path"),
                    "model_or_rate_name": row.get("name"),
                    "metric_scope": metric_name,
                    "ship_gate_eligible": bool(row.get("ship_gate_eligible")),
                    "flat_vector_concat_used": bool(row.get("flat_vector_concat_used")),
                    "row_count": n,
                    "correlation_ci": ci,
                    "accuracy_ci": accuracy_ci,
                }
            )
    return out


def completed_fsv_inventory(paths: list[Path]) -> list[dict[str, Any]]:
    out = []
    for path in paths:
        if not path.exists():
            out.append({"path": str(path), "exists": False})
            continue
        payload = read_json(path)
        status = payload.get("passes")
        if status is None:
            status = payload.get("allPassed")
        if status is None:
            status = payload.get("all_passed")
        out.append(
            {
                "path": str(path),
                "exists": True,
                "sha256": sha256_file(path),
                "status": status,
                "artifact_kind": payload.get("artifact_kind") or payload.get("task") or payload.get("issue"),
                "summary": payload.get("summary") or payload.get("dataset_summary") or payload.get("readback"),
            }
        )
    return out


def power_requirements(corpus_summary: dict[str, Any]) -> dict[str, Any]:
    zero_flake_n = min_zero_failure_sample(0.05, 0.05)
    return {
        "correlation_ci_rule": "Each claimed gate window/cell must include paired prediction score + oracle label rows with non-degenerate oracle labels and Fisher-z 95% lower bound >= 0.95.",
        "min_rows_if_observed_r": {
            "0.960": min_n_for_observed_correlation(0.960),
            "0.970": min_n_for_observed_correlation(0.970),
            "0.980": min_n_for_observed_correlation(0.980),
            "0.985": min_n_for_observed_correlation(0.985),
            "0.990": min_n_for_observed_correlation(0.990),
        },
        "oracle_repeat_rule": "Before #165 treats the gate as real, each ship-gate cell/window needs repeated oracle evidence or an explicit quarantine/filter for rows without it.",
        "min_zero_flake_repeats_for_upper_95pct_flake_rate_le_0_05": zero_flake_n,
        "min_zero_flake_units_for_8_categories_x_4_windows": zero_flake_n * 8 * 4,
        "current_corpus_rows": corpus_summary.get("entry_count"),
        "current_holdout_rows": corpus_summary.get("bucket_counts", {}).get("holdout"),
        "current_holdout_rows_per_category": {
            key.split(":", 1)[1]: sum(counts.values())
            for key, counts in sorted(corpus_summary.get("per_bucket_category_oracle_counts", {}).items())
            if key.startswith("holdout:")
        },
    }


def conclusion(metric_cis: list[dict[str, Any]], flakiness: dict[str, Any]) -> dict[str, Any]:
    eligible = [row for row in metric_cis if row["ship_gate_eligible"] and row["correlation_ci"].get("valid")]
    lower_bound_passes = [row for row in eligible if row["correlation_ci"].get("lower_bound_clears_target")]
    repeated_rows_present = bool(flakiness.get("repeated_oracle_rows_found"))
    if not repeated_rows_present:
        result = "underpowered"
        reason = "No current aiwonder repeated-oracle rows were found, so irreducible oracle noise is unbounded."
    elif not eligible:
        result = "underpowered"
        reason = "No ship-gate-eligible aiwonder metric artifact with paired prediction/oracle rows and valid correlation was found."
    elif len(lower_bound_passes) < len(eligible):
        result = "underpowered"
        reason = "At least one available ship-gate-eligible correlation has a 95% lower bound below 0.95."
    else:
        result = "attainable"
        reason = "Available eligible correlations clear the target lower bound and repeated-oracle rows exist; four-window completeness still must be checked by #165/#167."
    return {
        "fail_closed_result": result,
        "reason": reason,
        "ship_gate_claim_allowed_now": False,
        "ship_gate_claim_blockers": [
            "four consecutive production window artifacts with paired prediction/oracle rows were not found under /zfs/archive",
            "repeated-oracle/flakiness rows for the current 300x8 corpus were not found under /zfs/archive",
            "available high-correlation label-rate baselines are explicitly ship_gate_eligible=false and cannot prove model quality",
        ],
    }


def build_report(args: argparse.Namespace) -> dict[str, Any]:
    before = tree_state(args.fsv_root)
    entries, corpus_summary = load_corpus_index(args.corpus_index)
    metric_rows, metric_inventory = metric_records(args.metric_artifacts)
    metric_cis = collect_metric_cis(metric_rows)
    fsv_inventory = completed_fsv_inventory(args.completed_fsvs)
    store_inventory = oracle_store_inventory(args.oracle_store)
    flakiness_inventory = {
        "repeated_oracle_rows_found": False,
        "current_production_search_roots": [str(args.oracle_store), str(args.fsv_root.parent)],
        "oracle_store_inventory": store_inventory,
        "legacy_or_synthetic_flakiness_note": "TASK-G-019 legacy/local synthetic flakiness FSV is not current production noise evidence unless revalidated under /zfs/archive.",
    }
    report = {
        "schema_version": SCHEMA_VERSION,
        "artifact_kind": ARTIFACT_KIND,
        "task_id": TASK_ID,
        "created_at_utc": utc_now(),
        "source_of_truth": {
            "corpus_index": str(args.corpus_index),
            "oracle_store": str(args.oracle_store),
            "fsv_root": str(args.fsv_root),
        },
        "before_state": {"fsv_root": before},
        "source_rows": corpus_summary,
        "metric_artifacts": metric_inventory,
        "completed_fsv_inventory": fsv_inventory,
        "correlation_confidence_intervals": metric_cis,
        "repeated_oracle_flakiness_inventory": flakiness_inventory,
        "minimum_evidence_before_165_claim": power_requirements(corpus_summary),
        "edge_cases": edge_cases(),
    }
    report["conclusion"] = conclusion(metric_cis, flakiness_inventory)
    report["passes"] = all(case["passes"] for case in report["edge_cases"])
    report["all_passed"] = report["passes"]
    return report


def edge_cases() -> list[dict[str, Any]]:
    cases = []
    ci = fisher_correlation_ci(0.985, 78)
    cases.append({"name": "known_high_correlation_ci_is_finite", "passes": ci["valid"] and ci["lower"] > 0.95, "result": ci})
    ci_low = fisher_correlation_ci(0.95, 100000)
    cases.append({"name": "observed_exact_threshold_cannot_clear_lower_bound", "passes": ci_low["valid"] and ci_low["lower"] < 0.95, "result": ci_low})
    zero_n = min_zero_failure_sample()
    cases.append({"name": "zero_flake_sample_bound_is_exact_for_5pct_upper", "passes": zero_n == 59, "result": {"n": zero_n, "upper": clopper_upper_zero_failures(zero_n)}})
    invalid = fisher_correlation_ci(0.5, 3)
    cases.append({"name": "too_few_rows_fails_closed", "passes": invalid["valid"] is False and invalid["reason"] == "TOO_FEW_ROWS_FOR_FISHER_CI", "result": invalid})
    return cases


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--corpus-index", type=Path, default=DEFAULT_CORPUS_INDEX)
    parser.add_argument("--oracle-store", type=Path, default=DEFAULT_ORACLE_STORE)
    parser.add_argument("--fsv-root", type=Path, default=DEFAULT_FSV_ROOT)
    parser.add_argument("--metric-artifact", action="append", type=Path, dest="metric_artifacts")
    parser.add_argument("--completed-fsv", action="append", type=Path, dest="completed_fsvs")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    args.metric_artifacts = args.metric_artifacts or list(DEFAULT_METRIC_ARTIFACTS)
    args.completed_fsvs = args.completed_fsvs or list(DEFAULT_COMPLETED_FSVS)
    args.fsv_root.mkdir(parents=True, exist_ok=True)
    report = build_report(args)
    report_path = args.fsv_root / "python_oracle_noise_power_fsv.json"
    dump_json(report_path, report)
    readback = read_json(report_path)
    require(readback == report, "MEJEPA_ORACLE_POWER_READBACK_MISMATCH", str(report_path))
    readback_path = args.fsv_root / "independent_readback.json"
    dump_json(
        readback_path,
        {
            "path": str(report_path),
            "sha256": sha256_file(report_path),
            "readback_equal": True,
            "conclusion": readback["conclusion"],
            "after_state": tree_state(args.fsv_root),
        },
    )
    final = read_json(report_path)
    final["readback"] = {"path": str(readback_path), "sha256": sha256_file(readback_path), "readback_equal": True}
    final["after_state"] = {"fsv_root": tree_state(args.fsv_root)}
    dump_json(report_path, final)
    print(json.dumps({"report": str(report_path), "sha256": sha256_file(report_path), "passes": final["passes"], "conclusion": final["conclusion"]}, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
