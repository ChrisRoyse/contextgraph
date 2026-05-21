#!/usr/bin/env python3
"""FSV harness for #569 delete_test_call assertion-duty repair.

The harness is intentionally narrow: consume the #570 v2 support folds, test
small live-safe assertion-duty policies, and either promote a split-legal lift
or leave a precise blocker. Holdout labels are used only for final evaluation.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import math
import re
from collections import Counter
from pathlib import Path
from typing import Any, Callable


ISSUE = 569
TASK_ID = "TASK-PY-G-092D2"

INDEX_PATH = Path(
    "/zfs/archive/contextgraph/corpus/"
    "swebench-lite-python-300x8-v1-20260511/index.json"
)
REPORT_392_PATH = Path(
    "/zfs/archive/contextgraph/fsv/"
    "task-py-g-392-temporal-trajectory-features-fsv/"
    "run-20260521T171548Z/task_py_g_392_temporal_trajectory_features_fsv.json"
)
REPORT_567_PATH = Path(
    "/zfs/archive/contextgraph/fsv/"
    "task-py-g-092d-delete-test-call-cell-repair-fsv/"
    "run-20260521T181604Z/task_py_g_092d_delete_test_call_cell_repair_fsv.json"
)
REPORT_570_PATH = Path(
    "/zfs/archive/contextgraph/fsv/"
    "task-py-g-092d3-delete-test-call-support-fsv/"
    "run-20260521T184638Z/task_py_g_092d3_delete_test_call_support_fsv.json"
)
SUPPORT_ROOT = Path(
    "/zfs/archive/contextgraph/state/splits/"
    "task-py-g-092d3-delete-test-call-support-v2"
)
INVALID_SUPPORT_ROOT = Path(
    "/zfs/archive/contextgraph/state/splits/"
    "task-py-g-092d3-delete-test-call-support-v1"
)
OUTPUT_ROOT = Path(
    "/zfs/archive/contextgraph/fsv/"
    "task-py-g-092d2-delete-test-call-assertion-duty-fsv"
)
KNOWN_INVALID_PREDECESSOR = Path(
    "/zfs/archive/contextgraph/fsv/"
    "task-py-g-092d2-delete-test-call-assertion-duty-fsv/"
    "run-20260521T190837Z/task_py_g_092d2_delete_test_call_assertion_duty_fsv.json"
)

EXPECTED_HASHES = {
    INDEX_PATH: "9bf8c80ea105936787186d178541b0d2fa8675d9e282d8ef8ba0fb5ed6da597f",
    REPORT_392_PATH: "1081128ec04064ea89f2be7ab7677117e965afdbce1ed029ba39e683b8adfd8c",
    REPORT_567_PATH: "7ee938fcead66002e283dfe315a8d95aa14b7194e0b89b7bc78a1a440c9a0d5d",
    REPORT_570_PATH: "922dd433ab3b2f0df0fa617dc9be31782a1afe2db2ac3a254f04774e32257cfa",
    SUPPORT_ROOT / "delete_test_call_support_manifest.json": (
        "7aab6411ac65d30757643c081b2aeebdfaa84557b4520b669396b675c5412bfc"
    ),
    SUPPORT_ROOT / "delete_test_call_support_folds.jsonl": (
        "ff564dc566f1449164874f48d181aad56c161bd0be3b89f177d4e7694fb1374f"
    ),
    SUPPORT_ROOT / "delete_test_call_support_rows.jsonl": (
        "8047e0c55ae0fad25ebce29b3a33070fc6599317e1f8ae03f7b35fab788b0eaa"
    ),
}


class FsvError(RuntimeError):
    pass


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def load_json(path: Path) -> Any:
    with path.open("r", encoding="utf-8") as f:
        return json.load(f)


def load_jsonl(path: Path) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    with path.open("r", encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if line:
                rows.append(json.loads(line))
    return rows


def now_stamp() -> str:
    return dt.datetime.now(dt.UTC).strftime("%Y%m%dT%H%M%SZ")


def row_key(row: dict[str, Any]) -> tuple[str, str]:
    patch = str(row.get("patch_sha256") or "").replace("sha256:", "")
    return (str(row.get("task_id") or ""), patch)


def parse_source_fields(row: dict[str, Any]) -> dict[str, Any]:
    note = str(row.get("mutation_note") or "")
    source_path_match = re.search(r"source_path=file:([^ ]+)", note)
    source_len_match = re.search(r"source_site_len=([0-9]+)", note)
    source_path = source_path_match.group(1) if source_path_match else ""
    source_len = int(source_len_match.group(1)) if source_len_match else -1
    is_self_assert = "self.assert" in note
    is_plain_assert = "assert " in note and not is_self_assert
    is_test_path = bool(
        re.search(
            r"(^|/)(tests?|testing)/|(^|/)test_|_test[.]py$|tests[.]py$|/tests[.]py$",
            source_path,
        )
    )
    parts = source_path.split("/") if source_path else []
    return {
        "source_path": source_path,
        "source_site_len": source_len,
        "assert_kind": "self_assert"
        if is_self_assert
        else ("plain_assert" if is_plain_assert else "other"),
        "plain_non_test_assert": is_plain_assert and not is_test_path,
        "is_test_path": is_test_path,
        "path_depth": len(parts),
        "prefix2": "/".join(parts[:2]),
        "prefix3": "/".join(parts[:3]),
        "path_segments": parts,
    }


def enrich_index_rows(index_data: Any) -> list[dict[str, Any]]:
    rows = index_data if isinstance(index_data, list) else (
        index_data.get("rows") or index_data.get("entries") or index_data.get("items") or []
    )
    enriched: list[dict[str, Any]] = []
    for row in rows:
        if row.get("category") != "delete_test_call":
            continue
        item = dict(row)
        item.update(parse_source_fields(item))
        item["actual_fail"] = not bool(item.get("oracle_all_passed"))
        item["key"] = row_key(item)
        enriched.append(item)
    return enriched


def pearson(scores: list[float], actual_pass: list[float]) -> float | None:
    n = len(scores)
    if n == 0:
        return None
    mx = sum(scores) / n
    my = sum(actual_pass) / n
    vx = sum((x - mx) ** 2 for x in scores)
    vy = sum((y - my) ** 2 for y in actual_pass)
    if vx == 0 or vy == 0:
        return None
    return sum((x - mx) * (y - my) for x, y in zip(scores, actual_pass)) / math.sqrt(vx * vy)


def brier(scores: list[float], actual_pass: list[float]) -> float | None:
    if not scores:
        return None
    return sum((s - y) ** 2 for s, y in zip(scores, actual_pass)) / len(scores)


def ece(scores: list[float], actual_pass: list[float], bins: int = 10) -> float | None:
    if not scores:
        return None
    total = len(scores)
    acc = 0.0
    for b in range(bins):
        lo = b / bins
        hi = (b + 1) / bins
        members = [
            (s, y)
            for s, y in zip(scores, actual_pass)
            if (lo <= s < hi) or (b == bins - 1 and lo <= s <= hi)
        ]
        if not members:
            continue
        conf = sum(s for s, _ in members) / len(members)
        observed = sum(y for _, y in members) / len(members)
        acc += (len(members) / total) * abs(conf - observed)
    return acc


def metric_block(rows: list[dict[str, Any]]) -> dict[str, Any]:
    scores = [float(r["score"]) for r in rows if r.get("active", True)]
    actual = [1.0 if r["actual_pass"] else 0.0 for r in rows if r.get("active", True)]
    predicted = [s >= 0.5 for s in scores]
    actual_bool = [bool(y) for y in actual]
    actual_fail = actual_bool.count(False)
    actual_pass = actual_bool.count(True)
    predicted_fail = predicted.count(False)
    predicted_pass = predicted.count(True)
    true_fail = sum((not p) and (not a) for p, a in zip(predicted, actual_bool))
    true_pass = sum(p and a for p, a in zip(predicted, actual_bool))
    return {
        "sample_count": len(scores),
        "actual_fail_count": actual_fail,
        "actual_pass_count": actual_pass,
        "predicted_fail_count": predicted_fail,
        "predicted_pass_count": predicted_pass,
        "fail_recall": true_fail / actual_fail if actual_fail else None,
        "pass_recall": true_pass / actual_pass if actual_pass else None,
        "prediction_oracle_correlation": pearson(scores, actual),
        "brier": brier(scores, actual),
        "ece": ece(scores, actual),
    }


def row_diagnostics(
    report_392: dict[str, Any],
    delete_rows_by_task: dict[str, dict[str, Any]],
) -> list[dict[str, Any]]:
    diagnostics = report_392.get("score_sharpness_abstain_holdout_row_diagnostics") or []
    out = []
    for row in diagnostics:
        task_id = str(row["task_id"])
        category = str(row["category"])
        if category == "delete_test_call":
            index_row = delete_rows_by_task.get(task_id)
            if not index_row:
                raise FsvError(f"delete_test_call diagnostic task_id missing from index: {task_id}")
            merged = dict(index_row)
        else:
            merged = {
                "task_id": task_id,
                "category": category,
                "bucket": "holdout",
                "source_path": "",
                "source_site_len": -1,
                "assert_kind": "not_applicable",
                "plain_non_test_assert": False,
                "is_test_path": False,
                "path_depth": 0,
                "prefix2": "",
                "prefix3": "",
                "path_segments": [],
                "actual_fail": not bool(row["actual_oracle_pass"]),
            }
        merged.update(
            {
                "diagnostic_row": row,
                "score": float(row["predicted_oracle_pass"]),
                "active": bool(row["active_after_abstain"]),
                "actual_pass": bool(row["actual_oracle_pass"]),
            }
        )
        out.append(merged)
    return out


def learned_pass_only_prefix_model(
    train_calibration_rows: list[dict[str, Any]], prefix_field: str
) -> Callable[[dict[str, Any]], bool]:
    counts: Counter[tuple[str, bool]] = Counter()
    for row in train_calibration_rows:
        if row["plain_non_test_assert"]:
            counts[(row[prefix_field], row["actual_fail"])] += 1
    pass_only = {
        prefix
        for (prefix, fail), count in counts.items()
        if not fail and count > 0 and counts.get((prefix, True), 0) == 0
    }

    def predicate(row: dict[str, Any]) -> bool:
        return row["plain_non_test_assert"] and row[prefix_field] not in pass_only

    predicate.model = {  # type: ignore[attr-defined]
        "prefix_field": prefix_field,
        "pass_only_prefixes": sorted(pass_only),
    }
    return predicate


def learned_failure_prefix_model(
    train_calibration_rows: list[dict[str, Any]], prefix_field: str
) -> Callable[[dict[str, Any]], bool]:
    counts: Counter[tuple[str, bool]] = Counter()
    for row in train_calibration_rows:
        if row["plain_non_test_assert"]:
            counts[(row[prefix_field], row["actual_fail"])] += 1
    failure_only = {
        prefix
        for (prefix, fail), count in counts.items()
        if fail and count > 0 and counts.get((prefix, False), 0) == 0
    }

    def predicate(row: dict[str, Any]) -> bool:
        return row["plain_non_test_assert"] and row[prefix_field] in failure_only

    predicate.model = {"prefix_field": prefix_field, "failure_only_prefixes": sorted(failure_only)}  # type: ignore[attr-defined]
    return predicate


def static_depth_len_band(row: dict[str, Any]) -> bool:
    return (
        row["plain_non_test_assert"]
        and row["path_depth"] == 3
        and (34 <= row["source_site_len"] <= 41 or 70 <= row["source_site_len"] <= 80)
    )


def static_plain_non_test(row: dict[str, Any]) -> bool:
    return row["plain_non_test_assert"]


def illegal_oracle_peek_token(row: dict[str, Any]) -> bool:
    # This intentionally includes the holdout-only "solvers" token to prove that
    # the harness rejects oracle-peek policies even when they look good.
    return row["plain_non_test_assert"] and any(
        token in row["path_segments"] for token in ("dispatch", "sets", "solvers")
    )


def build_fold_rows(
    rows: list[dict[str, Any]], support_rows: list[dict[str, Any]], fold_id: str
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    overrides: dict[tuple[str, str], str] = {}
    for entry in support_rows:
        if entry.get("fold_id") != fold_id:
            continue
        row = dict(entry["row"])
        row.update(parse_source_fields(row))
        overrides[row_key(row)] = entry["move"]["to"]

    train_calibration = []
    calibration = []
    for row in rows:
        bucket = overrides.get(row["key"], row["bucket"])
        if bucket in ("train", "calibration"):
            train_calibration.append(row)
        if bucket == "calibration":
            calibration.append(row)
    return train_calibration, calibration


def confusion_for_flags(rows: list[dict[str, Any]], predicate: Callable[[dict[str, Any]], bool]) -> dict[str, Any]:
    c: Counter[tuple[bool, bool]] = Counter()
    matched = []
    for row in rows:
        flag = bool(predicate(row))
        actual_fail = bool(row["actual_fail"])
        c[(flag, actual_fail)] += 1
        if flag:
            matched.append(
                {
                    "task_id": row["task_id"],
                    "source_path": row["source_path"],
                    "actual_fail": actual_fail,
                    "bucket": row.get("bucket"),
                }
            )
    return {
        "true_positive_fail": c[(True, True)],
        "false_positive_pass": c[(True, False)],
        "false_negative_fail": c[(False, True)],
        "true_negative_pass": c[(False, False)],
        "matched_rows": matched,
    }


def apply_candidate_to_holdout(
    holdout_rows: list[dict[str, Any]],
    predicate: Callable[[dict[str, Any]], bool],
    action: str,
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    mutated = []
    matched = []
    for row in holdout_rows:
        item = dict(row)
        flag = row["category"] == "delete_test_call" and bool(predicate(row))
        if flag:
            matched.append(
                {
                    "task_id": row["task_id"],
                    "category": row["category"],
                    "source_path": row["source_path"],
                    "actual_pass": row["actual_pass"],
                    "old_score": row["score"],
                    "action": action,
                }
            )
            if action == "demote_to_fail":
                item["score"] = min(float(item["score"]), 0.05)
            elif action == "abstain":
                item["active"] = False
            else:
                raise FsvError(f"unknown action: {action}")
        mutated.append(item)
    return mutated, matched


def evaluate_candidate(
    name: str,
    action: str,
    rows: list[dict[str, Any]],
    support_rows: list[dict[str, Any]],
    fold_ids: list[str],
    holdout_rows: list[dict[str, Any]],
    factory: Callable[[list[dict[str, Any]]], Callable[[dict[str, Any]], bool]],
    legal: bool = True,
    illegal_reason: str | None = None,
) -> dict[str, Any]:
    fold_results = []
    predicates = []
    for fold_id in fold_ids:
        train_calibration, calibration = build_fold_rows(rows, support_rows, fold_id)
        predicate = factory(train_calibration)
        predicates.append(predicate)
        fold_results.append(
            {
                "fold_id": fold_id,
                "calibration": confusion_for_flags(calibration, predicate),
                "model": getattr(predicate, "model", {}),
            }
        )

    # A policy must pass every support fold before holdout evaluation can be
    # considered for promotion. Use the first fold model for holdout reporting
    # only if all folds share the same support verdict.
    support_passed = all(
        f["calibration"]["true_positive_fail"] >= 1
        and f["calibration"]["false_positive_pass"] == 0
        and f["calibration"]["false_negative_fail"] == 0
        for f in fold_results
    )
    fold_models_equal = all(
        fold_results[0].get("model") == item.get("model") for item in fold_results[1:]
    )
    if fold_models_equal:
        holdout_predicate = predicates[0]
    else:
        # Fold models can differ; use consensus voting for final diagnostics.
        def holdout_predicate(row: dict[str, Any]) -> bool:
            return all(predicate(row) for predicate in predicates)

    holdout_after, holdout_matches = apply_candidate_to_holdout(
        holdout_rows, holdout_predicate, action
    )
    before_metrics = metric_block(holdout_rows)
    after_metrics = metric_block(holdout_after)
    delete_before = metric_block([r for r in holdout_rows if r["category"] == "delete_test_call"])
    delete_after = metric_block([r for r in holdout_after if r["category"] == "delete_test_call"])
    aggregate_non_regressed = (
        after_metrics["prediction_oracle_correlation"] is not None
        and before_metrics["prediction_oracle_correlation"] is not None
        and after_metrics["prediction_oracle_correlation"]
        >= before_metrics["prediction_oracle_correlation"]
        and (after_metrics["brier"] or 1.0) <= (before_metrics["brier"] or 0.0)
        and (after_metrics["ece"] or 1.0) <= (before_metrics["ece"] or 0.0)
    )
    aggregate_improved = aggregate_non_regressed and (
        after_metrics["prediction_oracle_correlation"]
        > before_metrics["prediction_oracle_correlation"]
        or (after_metrics["brier"] or 1.0) < (before_metrics["brier"] or 0.0)
        or (after_metrics["ece"] or 1.0) < (before_metrics["ece"] or 0.0)
    )
    cell_improved = (
        delete_after["prediction_oracle_correlation"] is not None
        and delete_before["prediction_oracle_correlation"] is not None
        and delete_after["prediction_oracle_correlation"]
        > delete_before["prediction_oracle_correlation"]
        and (delete_after["fail_recall"] or 0.0) >= (delete_before["fail_recall"] or 0.0)
    )
    promotable = bool(
        legal and support_passed and fold_models_equal and aggregate_improved and cell_improved
    )
    return {
        "candidate": name,
        "action": action,
        "legal": legal,
        "illegal_reason": illegal_reason,
        "support_passed": support_passed,
        "fold_models_equal": fold_models_equal,
        "fold_results": fold_results,
        "holdout_matches": holdout_matches,
        "before_holdout_active_metrics": before_metrics,
        "after_holdout_active_metrics": after_metrics,
        "before_delete_test_call_metrics": delete_before,
        "after_delete_test_call_metrics": delete_after,
        "aggregate_improved": aggregate_improved,
        "aggregate_non_regressed": aggregate_non_regressed,
        "cell_improved": cell_improved,
        "promotable": promotable,
    }


def validate_support_root(path: Path) -> None:
    if path != SUPPORT_ROOT:
        raise FsvError(f"support root is not #570 v2 root: {path}")
    if "v1" in str(path):
        raise FsvError("invalid v1 support root requested")


def make_readback(report_path: Path, report: dict[str, Any]) -> dict[str, Any]:
    return {
        "schema_version": 1,
        "task_id": TASK_ID,
        "issue": ISSUE,
        "report_path": str(report_path),
        "report_sha256": sha256_file(report_path),
        "repair_promoted": report["repair_promoted"],
        "residual_blocker": report.get("residual_blocker"),
        "candidate_count": len(report["candidate_results"]),
        "best_legal_candidate": report.get("best_legal_candidate", {}).get("candidate"),
        "no_mounted_drive_paths": not any(
            "/mnt/d" in json.dumps(value, sort_keys=True)
            for value in (report["source_artifacts"], report["output"])
        ),
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output-root", type=Path, default=OUTPUT_ROOT)
    args = parser.parse_args()

    validate_support_root(SUPPORT_ROOT)
    source_artifacts = {}
    for path, expected in EXPECTED_HASHES.items():
        actual = sha256_file(path)
        source_artifacts[str(path)] = {"sha256": actual, "expected_sha256": expected}
        if actual != expected:
            raise FsvError(f"hash mismatch for {path}: {actual} != {expected}")

    index_rows = enrich_index_rows(load_json(INDEX_PATH))
    rows_by_task = {row["task_id"]: row for row in index_rows}
    report_392 = load_json(REPORT_392_PATH)
    report_567 = load_json(REPORT_567_PATH)
    report_570 = load_json(REPORT_570_PATH)
    support_manifest = load_json(SUPPORT_ROOT / "delete_test_call_support_manifest.json")
    support_rows = [
        row
        for row in load_jsonl(SUPPORT_ROOT / "delete_test_call_support_rows.jsonl")
        if "fold_id" in row
    ]
    fold_ids = sorted({row["fold_id"] for row in support_rows})
    holdout_rows = row_diagnostics(report_392, rows_by_task)

    candidate_specs = [
        {
            "name": "plain_non_test_assert_demote",
            "action": "demote_to_fail",
            "factory": lambda _rows: static_plain_non_test,
            "legal": True,
            "illegal_reason": None,
            "runtime_feature_keys": ["mutation_note.assert_kind", "source_path.is_test_path"],
            "training_feature_keys": [],
        },
        {
            "name": "depth3_len_band_assert_demote",
            "action": "demote_to_fail",
            "factory": lambda _rows: static_depth_len_band,
            "legal": True,
            "illegal_reason": None,
            "runtime_feature_keys": [
                "mutation_note.assert_kind",
                "source_path.is_test_path",
                "source_path.depth",
                "mutation_note.source_site_len",
            ],
            "training_feature_keys": [],
        },
        {
            "name": "pass_only_prefix_gap_p2_demote",
            "action": "demote_to_fail",
            "factory": lambda fold_rows: learned_pass_only_prefix_model(fold_rows, "prefix2"),
            "legal": True,
            "illegal_reason": None,
            "runtime_feature_keys": [
                "mutation_note.assert_kind",
                "source_path.is_test_path",
                "source_path.prefix2",
            ],
            "training_feature_keys": ["oracle_all_passed"],
        },
        {
            "name": "pass_only_prefix_gap_p2_abstain",
            "action": "abstain",
            "factory": lambda fold_rows: learned_pass_only_prefix_model(fold_rows, "prefix2"),
            "legal": True,
            "illegal_reason": None,
            "runtime_feature_keys": [
                "mutation_note.assert_kind",
                "source_path.is_test_path",
                "source_path.prefix2",
            ],
            "training_feature_keys": ["oracle_all_passed"],
        },
        {
            "name": "failure_only_prefix_p2_demote",
            "action": "demote_to_fail",
            "factory": lambda fold_rows: learned_failure_prefix_model(fold_rows, "prefix2"),
            "legal": True,
            "illegal_reason": None,
            "runtime_feature_keys": [
                "mutation_note.assert_kind",
                "source_path.is_test_path",
                "source_path.prefix2",
            ],
            "training_feature_keys": ["oracle_all_passed"],
        },
        {
            "name": "oracle_peek_invariant_token_demote",
            "action": "demote_to_fail",
            "factory": lambda _rows: illegal_oracle_peek_token,
            "legal": False,
            "illegal_reason": "contains holdout-only source token 'solvers'; diagnostic only",
            "runtime_feature_keys": [
                "mutation_note.assert_kind",
                "source_path.is_test_path",
                "source_path.path_segments",
            ],
            "training_feature_keys": ["holdout_row_inspection"],
        },
    ]
    candidate_results = [
        evaluate_candidate(
            name=spec["name"],
            action=spec["action"],
            rows=index_rows,
            support_rows=support_rows,
            fold_ids=fold_ids,
            holdout_rows=holdout_rows,
            factory=spec["factory"],
            legal=spec["legal"],
            illegal_reason=spec["illegal_reason"],
        )
        | {
            "runtime_feature_keys": spec["runtime_feature_keys"],
            "training_feature_keys": spec["training_feature_keys"],
        }
        for spec in candidate_specs
    ]
    promotable = [item for item in candidate_results if item["promotable"]]
    legal_candidates = [item for item in candidate_results if item["legal"]]
    best_legal = max(
        legal_candidates,
        key=lambda item: (
            item["support_passed"],
            item["cell_improved"],
            item["after_delete_test_call_metrics"]["prediction_oracle_correlation"] or -1.0,
            -(item["after_holdout_active_metrics"]["brier"] or 1.0),
        ),
    )

    excluded_runtime_keys = {
        "oracle_all_passed",
        "oracle_per_test_count",
        "actual_oracle_pass",
        "predicted_oracle_pass",
        "actual_fail",
        "actual_pass",
    }
    runtime_keys = sorted(
        {
            key
            for item in candidate_results
            if item["legal"]
            for key in item["runtime_feature_keys"]
        }
    )
    edge_audits = {
        "happy_path_v2_support_loaded": {
            "passed": support_manifest["support_decision"]["eligible"]
            and len(fold_ids) == 2
            and support_manifest["holdout_policy"]["row_level_holdout_labels_persisted"] is False,
        },
        "v1_support_root_rejected": {"passed": False},
        "holdout_token_policy_rejected": {
            "passed": not any(
                item["candidate"] == "oracle_peek_invariant_token_demote" and item["promotable"]
                for item in candidate_results
            )
        },
        "single_fold_promotion_forbidden": {
            "passed": all(len(item["fold_results"]) == 2 for item in candidate_results)
        },
        "target_labels_excluded_from_runtime_keys": {
            "passed": not (set(runtime_keys) & excluded_runtime_keys),
            "runtime_feature_keys": runtime_keys,
            "excluded_runtime_keys": sorted(excluded_runtime_keys),
            "supervision_only_training_keys": sorted(
                {
                    key
                    for item in candidate_results
                    if item["legal"]
                    for key in item["training_feature_keys"]
                }
            ),
        },
        "fold_model_identity_required_for_promotion": {
            "passed": all(not item["promotable"] or item["fold_models_equal"] for item in candidate_results)
        },
        "candidate_scope_limited_to_delete_test_call": {
            "passed": all(
                match["category"] == "delete_test_call"
                for item in candidate_results
                for match in item["holdout_matches"]
            )
        },
    }
    try:
        validate_support_root(INVALID_SUPPORT_ROOT)
    except FsvError as exc:
        edge_audits["v1_support_root_rejected"] = {"passed": True, "error": str(exc)}

    repair_promoted = bool(promotable)
    residual = None
    if not repair_promoted:
        residual = {
            "code": "TASK_PY_G_092D2_NO_PROMOTABLE_ASSERTION_DUTY_SIGNAL",
            "message": (
                "Both #570 v2 support folds can identify moved train/calibration failures, "
                "but the legal policies either over-flag passing holdout rows, abstain away "
                "the only holdout failure, or fail to improve aggregate active metrics. "
                "The only perfect-looking token policy requires a holdout-only source token "
                "and is rejected as oracle-peek evidence."
            ),
            "next_gate_predicate": "#521/#405 Panel B measurement or #372 aiwonder root readiness",
        }

    run_dir = args.output_root / f"run-{now_stamp()}"
    run_dir.mkdir(parents=True, exist_ok=False)
    report_path = run_dir / "task_py_g_092d2_delete_test_call_assertion_duty_fsv.json"
    readback_path = run_dir / "task_py_g_092d2_delete_test_call_assertion_duty_readback.json"
    report = {
        "schema_version": 1,
        "task_id": TASK_ID,
        "issue": ISSUE,
        "created_at": dt.datetime.now(dt.UTC).isoformat(),
        "all_passed": all(item["passed"] for item in edge_audits.values()),
        "repair_promoted": repair_promoted,
        "selected_policy": promotable[0]["candidate"] if promotable else None,
        "best_legal_candidate": best_legal,
        "candidate_results": candidate_results,
        "residual_blocker": residual,
        "source_artifacts": source_artifacts,
        "source_reports": {
            "g392_keys": sorted(report_392.keys()),
            "d567_residual": report_567.get("residual_blocker"),
            "d570_support_decision": report_570.get("support_decision"),
        },
        "support_root": str(SUPPORT_ROOT),
        "fold_ids": fold_ids,
        "edge_audits": edge_audits,
        "invalidated_predecessor": {
            "path": str(KNOWN_INVALID_PREDECESSOR),
            "exists": KNOWN_INVALID_PREDECESSOR.exists(),
            "reason": (
                "The predecessor run joined #392 holdout diagnostics by task_id only. "
                "Rows from other mutation categories can share task IDs, so that run "
                "incorrectly allowed non-delete_test_call rows to inherit delete_test_call "
                "source-path features. This replacement run gates candidate flags by "
                "category == delete_test_call."
            ),
        },
        "output": {
            "run_dir": str(run_dir),
            "report_path": str(report_path),
            "readback_path": str(readback_path),
        },
    }
    report_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    readback = make_readback(report_path, report)
    readback_path.write_text(json.dumps(readback, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps({"report": str(report_path), "readback": str(readback_path), **readback}, indent=2))
    return 0 if report["all_passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
