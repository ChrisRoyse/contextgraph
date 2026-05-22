"""TASK-PY-G-077 secret/delayed holdout Goodhart guard FSV.

The harness is read-only with respect to model state. It inspects current
aiwonder gate/window artifacts when present, proves stable-ID disjointness
between train-visible and holdout evidence, and fails closed when the public
gate is green but the secret/delayed holdout is missing, overlapping, or red.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
from collections import Counter
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


SCHEMA_VERSION = 1
TASK_ID = "TASK-PY-G-077"
ARTIFACT_KIND = "python_secret_holdout_fsv"
DEFAULT_CORPUS_INDEX = Path("/zfs/archive/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/index.json")
DEFAULT_FSV_ROOT = Path("/zfs/archive/contextgraph/fsv/task-py-g-077-secret-holdout-fsv")
DEFAULT_WINDOW_INPUTS = (
    Path("/zfs/archive/contextgraph/runtime/ship-gate-windows.jsonl"),
    Path("/zfs/archive/contextgraph/state/ship-gate/windows.jsonl"),
    Path("/zfs/archive/contextgraph/fsv/task-py-g-076-oracle-noise-power-fsv/python_oracle_noise_power_fsv.json"),
    Path("/zfs/archive/contextgraph/fsv/task-py-g-093-constellation-ablation-suite-fsv/artifacts/constellation_ablation_metrics.jsonl"),
    Path("/zfs/archive/contextgraph/fsv/task-py-g-097-constellation-ablation-with-calibration-fsv/artifacts/constellation_ablation_with_calibration_metrics.jsonl"),
)
TARGET_CORRELATION = 0.95
MIN_CONSECUTIVE_WINDOWS = 4
TRAIN_VISIBLE_BUCKETS = {"train", "calibration", "public", "visible", "train_visible"}
HOLDOUT_BUCKETS = {"holdout", "secret", "hidden", "delayed", "blind"}


class SecretHoldoutFsvError(RuntimeError):
    pass


def require(condition: bool, code: str, detail: str | None = None) -> None:
    if not condition:
        raise SecretHoldoutFsvError(code if detail is None else f"{code}: {detail}")


def utc_now() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(1024 * 1024), b""):
            h.update(chunk)
    return "sha256:" + h.hexdigest()


def stable_hash(value: Any) -> str:
    payload = json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True).encode("utf-8")
    return "sha256:" + hashlib.sha256(payload).hexdigest()


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


def finite_float(value: Any) -> float | None:
    if isinstance(value, bool) or value is None:
        return None
    if isinstance(value, (int, float)):
        out = float(value)
        return out if math.isfinite(out) else None
    return None


def pearson(xs: list[float], ys: list[float]) -> tuple[float | None, str]:
    if len(xs) != len(ys) or len(xs) < 2:
        return None, "TOO_FEW_PAIRED_ROWS"
    mx = sum(xs) / len(xs)
    my = sum(ys) / len(ys)
    sx = math.sqrt(sum((x - mx) ** 2 for x in xs))
    sy = math.sqrt(sum((y - my) ** 2 for y in ys))
    if sx == 0.0 or sy == 0.0:
        return None, "DEGENERATE_VARIANCE"
    return sum((x - mx) * (y - my) for x, y in zip(xs, ys)) / (sx * sy), "ok"


def canonical_stable_id(row: dict[str, Any]) -> tuple[str | None, str]:
    for key in ("stable_id", "stableId", "row_id", "rowId", "sample_id", "sampleId", "prediction_id"):
        value = row.get(key)
        if isinstance(value, str) and value.strip():
            return value.strip(), key
    task = row.get("task_instance_id") or row.get("task_id") or row.get("instance_id")
    category = row.get("mutation_category") or row.get("category")
    if isinstance(task, str) and task.strip() and isinstance(category, str) and category.strip():
        return f"{task.strip()}::{category.strip()}", "task_id+category"
    return None, "missing"


def ids_from_value(value: Any) -> tuple[set[str], Counter[str]]:
    ids: set[str] = set()
    sources: Counter[str] = Counter()
    if isinstance(value, str) and value.strip():
        ids.add(value.strip())
        sources["string"] += 1
    elif isinstance(value, dict):
        item, source = canonical_stable_id(value)
        if item:
            ids.add(item)
        sources[source] += 1
    elif isinstance(value, list):
        for item in value:
            child, child_sources = ids_from_value(item)
            ids.update(child)
            sources.update(child_sources)
    return ids, sources


def extract_stable_ids(row: dict[str, Any]) -> tuple[set[str], dict[str, Any]]:
    ids: set[str] = set()
    sources: Counter[str] = Counter()
    for key in (
        "stable_ids",
        "stableIds",
        "sample_ids",
        "sampleIds",
        "row_ids",
        "rowIds",
        "prediction_row_ids",
        "task_category_keys",
        "member_ids",
        "holdout_ids",
        "train_visible_ids",
    ):
        child, child_sources = ids_from_value(row.get(key))
        ids.update(child)
        sources.update(child_sources)
    for key in ("rows", "examples", "samples", "predictions"):
        child, child_sources = ids_from_value(row.get(key))
        ids.update(child)
        sources.update(child_sources)
    direct, direct_source = canonical_stable_id(row)
    if direct:
        ids.add(direct)
    sources[direct_source] += 1
    return ids, {"count": len(ids), "sources": dict(sorted(sources.items()))}


def role_from_row(row: dict[str, Any]) -> str:
    parts = []
    for key in ("visibility", "split", "bucket", "window_role", "gate_role", "dataset_split", "audience", "role"):
        value = row.get(key)
        if isinstance(value, str):
            parts.append(value.lower().replace("-", "_"))
    text = " ".join(parts)
    if any(token in text for token in HOLDOUT_BUCKETS):
        return "holdout"
    if any(token in text for token in TRAIN_VISIBLE_BUCKETS):
        return "train_visible"
    return "unknown"


def nested_get(row: dict[str, Any], *keys: str) -> Any:
    for key in keys:
        if key in row:
            return row[key]
    for parent in ("metrics", "test_metrics", "correlation_ci", "window_metrics"):
        child = row.get(parent)
        if isinstance(child, dict):
            for key in keys:
                if key in child:
                    return child[key]
    return None


def row_metric(row: dict[str, Any], stable_ids: set[str], target: float) -> dict[str, Any]:
    corr = finite_float(nested_get(row, "prediction_oracle_correlation", "correlation", "r"))
    validity = nested_get(row, "prediction_oracle_correlation_validity", "validity", "reason") or "reported"
    rows = row.get("rows") if isinstance(row.get("rows"), list) else row.get("predictions")
    if corr is None and isinstance(rows, list):
        xs, ys = [], []
        for item in rows:
            if not isinstance(item, dict):
                continue
            score = finite_float(item.get("prediction_score") or item.get("score") or item.get("predicted_probability"))
            label = item.get("oracle_all_passed")
            if label is None:
                label = item.get("oracle_label")
            if score is not None and isinstance(label, bool):
                xs.append(score)
                ys.append(1.0 if label else 0.0)
        corr, validity = pearson(xs, ys)
    threshold = finite_float(nested_get(row, "threshold", "ship_gate_target", "target_correlation")) or target
    explicit_pass = nested_get(row, "ship_gate_passed", "shipGatePassed", "passed", "passes", "passed_window")
    explicit_pass_bool = explicit_pass if isinstance(explicit_pass, bool) else None
    passed = bool(corr is not None and corr >= threshold)
    eligible = bool(row.get("ship_gate_eligible")) or any(key in row for key in ("ship_gate_passed", "shipGatePassed", "ship_gate_failures"))
    return {
        "correlation": corr,
        "correlation_validity": validity,
        "threshold": threshold,
        "passed": passed,
        "explicit_pass_diagnostic": explicit_pass_bool,
        "explicit_pass_policy": "diagnostic_only_recomputed_from_measured_correlation",
        "ship_gate_eligible": eligible,
        "stable_id_count": len(stable_ids),
    }


def flatten_json_records(payload: Any) -> list[dict[str, Any]]:
    if isinstance(payload, list):
        return [item for item in payload if isinstance(item, dict)]
    if not isinstance(payload, dict):
        return []
    for key in ("gate_windows", "window_rows", "windows", "rows", "correlation_confidence_intervals"):
        value = payload.get(key)
        if isinstance(value, list):
            return [item for item in value if isinstance(item, dict)]
    return [payload]


def load_window_records(paths: list[Path], target: float) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    records: list[dict[str, Any]] = []
    inventory: list[dict[str, Any]] = []
    for path in paths:
        if not path.exists():
            inventory.append({"path": str(path), "exists": False})
            continue
        raw_records = []
        if path.suffix == ".jsonl":
            raw_records = [row for _line_no, row in iter_jsonl(path)]
        else:
            raw_records = flatten_json_records(read_json(path))
        source_sha = sha256_file(path)
        for idx, row in enumerate(raw_records):
            stable_ids, id_proof = extract_stable_ids(row)
            role = role_from_row(row)
            metric = row_metric(row, stable_ids, target)
            records.append(
                {
                    "record_key": f"{path}#{idx}",
                    "source_path": str(path),
                    "source_sha256": source_sha,
                    "record_index": idx,
                    "window_id": str(row.get("window_id") or row.get("windowId") or f"{path.name}:{idx}"),
                    "generated_at": row.get("generated_at_utc") or row.get("created_at_utc") or row.get("report_date"),
                    "role": role,
                    "stable_ids": sorted(stable_ids),
                    "stable_id_proof": id_proof,
                    **metric,
                }
            )
        inventory.append({"path": str(path), "exists": True, "sha256": source_sha, "raw_record_count": len(raw_records)})
    return records, inventory


def load_corpus_split(path: Path) -> dict[str, Any]:
    require(path.exists(), "MEJEPA_SECRET_HOLDOUT_CORPUS_INDEX_MISSING", str(path))
    payload = read_json(path)
    entries = payload.get("entries") or []
    require(isinstance(entries, list) and entries, "MEJEPA_SECRET_HOLDOUT_CORPUS_INDEX_EMPTY", str(path))
    by_bucket: dict[str, set[str]] = {}
    missing_ids = 0
    duplicate_ids: Counter[str] = Counter()
    seen: set[str] = set()
    for row in entries:
        require(isinstance(row, dict), "MEJEPA_SECRET_HOLDOUT_CORPUS_ROW_NOT_OBJECT", str(path))
        stable_id, _source = canonical_stable_id(row)
        if not stable_id:
            missing_ids += 1
            continue
        bucket = str(row.get("bucket") or "unknown")
        by_bucket.setdefault(bucket, set()).add(stable_id)
        duplicate_ids[stable_id] += 1
        seen.add(stable_id)
    train_ids = set().union(*(by_bucket.get(bucket, set()) for bucket in TRAIN_VISIBLE_BUCKETS))
    holdout_ids = set().union(*(by_bucket.get(bucket, set()) for bucket in HOLDOUT_BUCKETS))
    overlap = train_ids & holdout_ids
    return {
        "path": str(path),
        "sha256": sha256_file(path),
        "entry_count": len(entries),
        "unique_stable_ids": len(seen),
        "missing_stable_id_rows": missing_ids,
        "bucket_counts": {bucket: len(ids) for bucket, ids in sorted(by_bucket.items())},
        "train_visible_stable_id_count": len(train_ids),
        "holdout_stable_id_count": len(holdout_ids),
        "overlap_count": len(overlap),
        "overlap_sample": sorted(overlap)[:20],
        "duplicate_stable_id_count": sum(1 for count in duplicate_ids.values() if count > 1),
        "physically_disjoint_by_stable_id": bool(train_ids and holdout_ids and not overlap),
    }


def latest_green_window_set(records: list[dict[str, Any]], role: str, min_windows: int) -> dict[str, Any]:
    eligible = [row for row in records if row["role"] == role and row["ship_gate_eligible"] is True]
    eligible.sort(key=lambda row: (str(row.get("generated_at") or ""), str(row["window_id"]), str(row["source_path"]), int(row["record_index"])))
    latest = eligible[-min_windows:] if len(eligible) >= min_windows else eligible
    blockers = []
    if len(eligible) < min_windows:
        blockers.append(f"NEEDS_{min_windows}_ELIGIBLE_{role.upper()}_WINDOWS")
    if any(row["stable_id_count"] <= 0 for row in latest):
        blockers.append(f"{role.upper()}_WINDOW_MISSING_STABLE_IDS")
    if any(row["correlation"] is None for row in latest):
        blockers.append(f"{role.upper()}_WINDOW_MISSING_CORRELATION")
    if any(row["passed"] is not True for row in latest):
        blockers.append(f"{role.upper()}_WINDOW_BELOW_THRESHOLD")
    green = not blockers and len(latest) == min_windows
    return {
        "role": role,
        "eligible_window_count": len(eligible),
        "required_consecutive_windows": min_windows,
        "latest_record_keys": [row["record_key"] for row in latest],
        "latest_window_ids": [row["window_id"] for row in latest],
        "latest_correlations": [row["correlation"] for row in latest],
        "latest_passed": [row["passed"] for row in latest],
        "stable_id_count": len(set().union(*(set(row["stable_ids"]) for row in latest))) if latest else 0,
        "green": green,
        "blockers": blockers,
    }


def window_disjointness(records: list[dict[str, Any]], train_record_keys: list[str], holdout_record_keys: list[str]) -> dict[str, Any]:
    train_ids = set()
    holdout_ids = set()
    for row in records:
        if row["record_key"] in train_record_keys:
            train_ids.update(row["stable_ids"])
        if row["record_key"] in holdout_record_keys:
            holdout_ids.update(row["stable_ids"])
    overlap = train_ids & holdout_ids
    return {
        "train_visible_window_stable_id_count": len(train_ids),
        "holdout_window_stable_id_count": len(holdout_ids),
        "overlap_count": len(overlap),
        "overlap_sample": sorted(overlap)[:20],
        "physically_disjoint_by_stable_id": bool(train_ids and holdout_ids and not overlap),
    }


def conclude(train_gate: dict[str, Any], holdout_gate: dict[str, Any], disjointness: dict[str, Any]) -> dict[str, Any]:
    blockers = []
    if not train_gate["green"]:
        blockers.extend(train_gate["blockers"])
    if train_gate["green"] and not holdout_gate["green"]:
        blockers.append("PUBLIC_GATE_GREEN_BUT_HOLDOUT_MISSING_OR_RED")
        blockers.extend(holdout_gate["blockers"])
    if train_gate["green"] and holdout_gate["green"] and not disjointness["physically_disjoint_by_stable_id"]:
        blockers.append("TRAIN_VISIBLE_AND_HOLDOUT_WINDOWS_NOT_PHYSICALLY_DISJOINT")
    if not blockers:
        result = "guard_satisfied"
        reason = "Train-visible and holdout windows are independently green and physically disjoint by stable ID."
    elif train_gate["green"]:
        result = "blocked_public_green_holdout_guard_failed"
        reason = "A public/train-visible green gate cannot be accepted because the secret/delayed holdout guard failed closed."
    else:
        result = "underpowered"
        reason = "No complete train-visible public gate is currently available; the secret/delayed holdout guard cannot turn green."
    return {
        "fail_closed_result": result,
        "reason": reason,
        "secret_holdout_guard_passed": result == "guard_satisfied",
        "ship_gate_claim_allowed_by_this_guard": result == "guard_satisfied",
        "blocks_165_closeout": result != "guard_satisfied",
        "blockers": sorted(set(blockers)),
    }


def synthetic_records(case: str) -> list[dict[str, Any]]:
    def row(role: str, window: int, corr: float | None, ids: list[str]) -> dict[str, Any]:
        return {
            "record_key": f"synthetic:{case}#{role}-{window}",
            "source_path": f"synthetic:{case}",
            "source_sha256": "sha256:synthetic",
            "record_index": window,
            "window_id": f"{role}-{window}",
            "generated_at": f"2026-05-{window + 1:02d}T00:00:00Z",
            "role": role,
            "stable_ids": ids,
            "stable_id_proof": {"count": len(ids), "sources": {"fixture": len(ids)}},
            "correlation": corr,
            "correlation_validity": "synthetic_known_answer",
            "threshold": TARGET_CORRELATION,
            "passed": bool(corr is not None and corr >= TARGET_CORRELATION),
            "ship_gate_eligible": True,
            "stable_id_count": len(ids),
        }
    train = [row("train_visible", idx, 0.97, [f"train-{idx}-{j}" for j in range(3)]) for idx in range(4)]
    holdout = [row("holdout", idx, 0.96, [f"holdout-{idx}-{j}" for j in range(3)]) for idx in range(4)]
    if case == "missing_holdout":
        return train
    if case == "red_holdout":
        holdout[-1]["correlation"] = 0.90
        holdout[-1]["passed"] = False
    if case == "overlap":
        holdout[-1]["stable_ids"][0] = train[-1]["stable_ids"][0]
    if case == "missing_ids":
        train[-1]["stable_ids"] = []
        train[-1]["stable_id_count"] = 0
    if case == "insufficient_windows":
        return train[:3] + holdout
    return train + holdout


def evaluate_records(records: list[dict[str, Any]], min_windows: int) -> tuple[dict[str, Any], dict[str, Any], dict[str, Any], dict[str, Any]]:
    train_gate = latest_green_window_set(records, "train_visible", min_windows)
    holdout_gate = latest_green_window_set(records, "holdout", min_windows)
    disjointness = window_disjointness(records, train_gate["latest_record_keys"], holdout_gate["latest_record_keys"])
    return train_gate, holdout_gate, disjointness, conclude(train_gate, holdout_gate, disjointness)


def edge_cases() -> list[dict[str, Any]]:
    expectations = {
        "happy_disjoint_green": "guard_satisfied",
        "missing_holdout": "blocked_public_green_holdout_guard_failed",
        "red_holdout": "blocked_public_green_holdout_guard_failed",
        "overlap": "blocked_public_green_holdout_guard_failed",
        "missing_ids": "underpowered",
        "insufficient_windows": "underpowered",
    }
    cases = []
    for name, expected in expectations.items():
        records = synthetic_records(name)
        train_gate, holdout_gate, disjointness, conclusion = evaluate_records(records, MIN_CONSECUTIVE_WINDOWS)
        cases.append(
            {
                "name": name,
                "expected_result": expected,
                "actual_result": conclusion["fail_closed_result"],
                "passes": conclusion["fail_closed_result"] == expected,
                "train_visible_gate": train_gate,
                "holdout_gate": holdout_gate,
                "disjointness": disjointness,
                "conclusion": conclusion,
            }
        )
    explicit_pass_row = {
        "stable_ids": ["explicit-pass-below-threshold-row"],
        "prediction_oracle_correlation": 0.90,
        "threshold": TARGET_CORRELATION,
        "ship_gate_passed": True,
        "ship_gate_eligible": True,
    }
    stable_ids, stable_id_proof = extract_stable_ids(explicit_pass_row)
    metric = row_metric(explicit_pass_row, stable_ids, TARGET_CORRELATION)
    cases.append(
        {
            "name": "explicit_pass_bool_below_threshold_recomputed_red",
            "expected_result": "passed_false_even_with_explicit_true",
            "actual_result": f"passed_{str(metric['passed']).lower()}",
            "passes": metric["passed"] is False
            and metric["explicit_pass_diagnostic"] is True
            and metric["correlation"] == 0.90
            and metric["threshold"] == TARGET_CORRELATION,
            "metric": metric,
            "stable_id_proof": stable_id_proof,
        }
    )
    return cases


def build_report(args: argparse.Namespace) -> dict[str, Any]:
    before = tree_state(args.fsv_root)
    if args.self_test:
        corpus = {"path": None, "self_test": True, "physically_disjoint_by_stable_id": True}
        records = synthetic_records("happy_disjoint_green")
        inventory = [{"path": "synthetic:self_test", "exists": True, "raw_record_count": len(records)}]
    else:
        corpus = load_corpus_split(args.corpus_index)
        records, inventory = load_window_records(args.window_inputs, args.target_correlation)
    train_gate, holdout_gate, disjointness, conclusion = evaluate_records(records, args.min_consecutive_windows)
    cases = edge_cases()
    edge_cases_passed = all(case["passes"] for case in cases)
    guard_passed = bool(
        conclusion["secret_holdout_guard_passed"]
        and conclusion["ship_gate_claim_allowed_by_this_guard"]
        and not conclusion["blocks_165_closeout"]
    )
    passes = edge_cases_passed and guard_passed
    return {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "artifact_kind": ARTIFACT_KIND,
        "created_at_utc": utc_now(),
        "target_correlation": args.target_correlation,
        "min_consecutive_windows": args.min_consecutive_windows,
        "source_of_truth": {
            "corpus_index": str(args.corpus_index) if not args.self_test else "synthetic:self_test",
            "window_inputs": [str(path) for path in args.window_inputs],
            "fsv_root": str(args.fsv_root),
        },
        "before_state": {"fsv_root": before},
        "corpus_split_disjointness": corpus,
        "window_input_inventory": inventory,
        "normalized_window_record_count": len(records),
        "normalized_window_records": [
            {key: value for key, value in row.items() if key != "stable_ids"} | {"stable_id_hash": stable_hash(row["stable_ids"])}
            for row in records[:200]
        ],
        "normalized_window_records_truncated": len(records) > 200,
        "train_visible_gate": train_gate,
        "holdout_gate": holdout_gate,
        "window_stable_id_disjointness": disjointness,
        "conclusion": conclusion,
        "edge_cases": cases,
        "edge_cases_passed": edge_cases_passed,
        "guard_passed": guard_passed,
        "passes": passes,
        "all_passed": passes,
    }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--corpus-index", type=Path, default=DEFAULT_CORPUS_INDEX)
    parser.add_argument("--fsv-root", type=Path, default=DEFAULT_FSV_ROOT)
    parser.add_argument("--window-input", action="append", type=Path, dest="window_inputs")
    parser.add_argument("--target-correlation", type=float, default=TARGET_CORRELATION)
    parser.add_argument("--min-consecutive-windows", type=int, default=MIN_CONSECUTIVE_WINDOWS)
    parser.add_argument("--self-test", action="store_true", help="Run only deterministic synthetic protocol cases.")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    args.window_inputs = args.window_inputs or list(DEFAULT_WINDOW_INPUTS)
    require(args.min_consecutive_windows > 0, "MEJEPA_SECRET_HOLDOUT_MIN_WINDOWS_NONPOSITIVE")
    args.fsv_root.mkdir(parents=True, exist_ok=True)
    report = build_report(args)
    report_path = args.fsv_root / "python_secret_holdout_fsv.json"
    dump_json(report_path, report)
    readback = read_json(report_path)
    require(readback == report, "MEJEPA_SECRET_HOLDOUT_READBACK_MISMATCH", str(report_path))
    final = {**report, "after_state": {"fsv_root": tree_state(args.fsv_root)}, "readback": {"path": str(args.fsv_root / "independent_readback.json")}}
    dump_json(report_path, final)
    readback_payload = {
        "path": str(report_path),
        "sha256": sha256_file(report_path),
        "readback_equal": read_json(report_path) == final,
        "conclusion": final["conclusion"],
        "edge_cases_passed": all(case["passes"] for case in final["edge_cases"]),
        "after_state": tree_state(args.fsv_root),
    }
    dump_json(args.fsv_root / "independent_readback.json", readback_payload)
    print(
        json.dumps(
            {
                "report": str(report_path),
                "sha256": sha256_file(report_path),
                "readback": str(args.fsv_root / "independent_readback.json"),
                "readback_sha256": sha256_file(args.fsv_root / "independent_readback.json"),
                "passes": final["passes"],
                "conclusion": final["conclusion"],
            },
            sort_keys=True,
        )
    )
    return 0 if final["passes"] else 1

if __name__ == "__main__":
    raise SystemExit(main())
