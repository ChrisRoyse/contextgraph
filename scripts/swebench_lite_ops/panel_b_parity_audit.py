from __future__ import annotations

import json
from collections import Counter
from pathlib import Path
from typing import Any

from .core import SwebenchOpsError, sha256_file, utc_now, write_json_checked
from .python_pair_feature_audit import write_jsonl
from .python_per_cell_smooth_ece import metric_block, reject_mounted_drive


DEFAULT_ROW_DIAGNOSTICS_REPORT = Path(
    "/zfs/archive/contextgraph/readback/task-py-g-392-temporal-trajectory-features-readback/"
    "run-20260521T171548Z/task_py_g_392_temporal_trajectory_features_readback.json"
)
DEFAULT_FULL_PANEL_B_ROWS = Path(
    "/zfs/archive/contextgraph/readback/task-py-g-111e-panel-b-quality-repair-readback/"
    "k30_t0p15_m512_panel_b_score_rows.jsonl"
)
DEFAULT_ACTIVE_PANEL_B_ROWS = Path(
    "/zfs/archive/contextgraph/readback/task-py-g-111e-panel-b-quality-repair-readback/"
    "active-392-k30-20260521T2338Z/active_392_panel_b_score_rows.jsonl"
)
DEFAULT_MODEL_SCORE_ROWS = Path(
    "/zfs/archive/contextgraph/readback/task-py-g-111e-panel-b-quality-repair-readback/"
    "mean-sweep-20260521T2332Z/k30_t0p15_m512_scores.jsonl"
)
DEFAULT_OUTPUT_ROOT = Path("/zfs/archive/contextgraph/readback/task-py-g-111f-panel-b-parity-scorer-readback")


def add_arguments(parser: Any) -> None:
    parser.add_argument("--row-diagnostics-report", type=Path, default=DEFAULT_ROW_DIAGNOSTICS_REPORT)
    parser.add_argument("--full-panel-b-rows", type=Path, default=DEFAULT_FULL_PANEL_B_ROWS)
    parser.add_argument("--active-panel-b-rows", type=Path, default=DEFAULT_ACTIVE_PANEL_B_ROWS)
    parser.add_argument("--model-score-rows", type=Path, default=DEFAULT_MODEL_SCORE_ROWS)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--evidence-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--expected-full-count", type=int, default=240)
    parser.add_argument("--expected-active-count", type=int, default=205)
    parser.add_argument("--expected-calibration-diagnostics", type=int, default=240)
    parser.add_argument("--expected-holdout-diagnostics", type=int, default=240)


def command_panel_b_parity_audit(args: Any) -> dict[str, Any]:
    row_diagnostics_report = args.row_diagnostics_report.expanduser().resolve(strict=False)
    full_panel_b_rows = args.full_panel_b_rows.expanduser().resolve(strict=False)
    active_panel_b_rows = args.active_panel_b_rows.expanduser().resolve(strict=False)
    model_score_rows = args.model_score_rows.expanduser().resolve(strict=False)
    output_root = args.output_root.expanduser().resolve(strict=False)
    for name, path in (
        ("row_diagnostics_report", row_diagnostics_report),
        ("full_panel_b_rows", full_panel_b_rows),
        ("active_panel_b_rows", active_panel_b_rows),
        ("model_score_rows", model_score_rows),
        ("output_root", output_root),
    ):
        reject_mounted_drive(name, path)

    diagnostics = read_json_object(row_diagnostics_report, "row_diagnostics_report")
    calibration_diag = require_array(
        diagnostics.get("score_sharpness_abstain_calibration_row_diagnostics"),
        "score_sharpness_abstain_calibration_row_diagnostics",
    )
    holdout_diag = require_array(
        diagnostics.get("score_sharpness_abstain_holdout_row_diagnostics"),
        "score_sharpness_abstain_holdout_row_diagnostics",
    )
    if len(calibration_diag) != args.expected_calibration_diagnostics:
        raise_count("calibration diagnostics", args.expected_calibration_diagnostics, len(calibration_diag))
    if len(holdout_diag) != args.expected_holdout_diagnostics:
        raise_count("holdout diagnostics", args.expected_holdout_diagnostics, len(holdout_diag))

    full_rows = load_panel_rows(full_panel_b_rows, expected=args.expected_full_count)
    active_rows = load_panel_rows(active_panel_b_rows, expected=args.expected_active_count)
    model_rows = load_model_score_rows(model_score_rows)
    output_root.mkdir(parents=True, exist_ok=True)
    parity_gap_rows = build_parity_gap_rows(full_rows, active_rows, calibration_diag, holdout_diag, model_rows)
    gap_rows_path = output_root / "panel_b_parity_gap_rows.jsonl"
    write_jsonl(gap_rows_path, parity_gap_rows)

    full_metrics = panel_metrics(full_rows)
    active_metrics = panel_metrics(active_rows)
    report = {
        "checked_at": utc_now(),
        "artifactKind": "task_py_g_111f_panel_b_parity_audit",
        "githubIssue": 582,
        "ok": True,
        "rowDiagnosticsReport": str(row_diagnostics_report),
        "rowDiagnosticsReportSha256": sha256_file(row_diagnostics_report),
        "fullPanelBRows": str(full_panel_b_rows),
        "fullPanelBRowsSha256": sha256_file(full_panel_b_rows),
        "activePanelBRows": str(active_panel_b_rows),
        "activePanelBRowsSha256": sha256_file(active_panel_b_rows),
        "modelScoreRows": str(model_score_rows),
        "modelScoreRowsSha256": sha256_file(model_score_rows),
        "rowCounts": {
            "calibrationDiagnostics": len(calibration_diag),
            "holdoutDiagnostics": len(holdout_diag),
            "fullPanelBRows": len(full_rows),
            "activePanelBRows": len(active_rows),
            "modelScoreRows": len(model_rows),
        },
        "metrics": {
            "full": full_metrics,
            "active392": active_metrics,
        },
        "labelSurface": label_surface(model_rows),
        "parityGap": {
            "rawKnnScoreRowsAreParityScorer": False,
            "calibrationPanelBScoreRowsPresent": False,
            "panelBRowsHaveConsequenceEvidenceKeys": False,
            "panelBRowsHaveCauseResidualFeatures": False,
            "panelBRowsHaveActiveAbstainPolicyInputs": False,
            "panelBRowsHavePerSlotModelScores": "model score JSONL only; strict score rows lose per-slot fields",
            "canReuseLiveSafeLabels": True,
            "requiredNextImplementation": [
                "persist Panel B calibration split score rows, not only holdout rows",
                "join Panel B scores to #392 row diagnostics by prediction identity or task/category/patch identity",
                "reuse accepted labels, failure evidence IDs, evidence-key ladder, cause residual counts, and active/abstain policy",
                "fit parity calibration on calibration split only and report holdout full/active per-cell metrics",
            ],
        },
        "outputArtifacts": {
            "panelBParityGapRowsJsonl": str(gap_rows_path),
            "panelBParityGapRowsSha256": sha256_file(gap_rows_path),
        },
        "promotionRecommendation": {
            "promote": False,
            "reason": "current artifacts are raw KNN score rows, not consequence-stack parity rows",
            "nextIssueStep": "modify Panel B scorer to persist calibration+holdout parity inputs, then fit split-legal parity scorer",
        },
        "pathPolicy": {
            "aiwonderOnly": str(row_diagnostics_report).startswith("/zfs/archive/contextgraph/")
            and str(full_panel_b_rows).startswith("/zfs/archive/contextgraph/")
            and str(active_panel_b_rows).startswith("/zfs/archive/contextgraph/")
            and str(model_score_rows).startswith("/zfs/archive/contextgraph/")
            and str(output_root).startswith("/zfs/archive/contextgraph/"),
            "mntDActiveReadsOrWrites": False,
        },
    }
    report_path = output_root / "panel_b_parity_audit_report.json"
    report["reportPath"] = str(report_path)
    write_json_checked(report_path, report)
    report["reportSha256"] = sha256_file(report_path)
    return report


def load_panel_rows(path: Path, *, expected: int) -> list[dict[str, Any]]:
    rows = load_jsonl(path)
    if len(rows) != expected:
        raise_count(str(path), expected, len(rows))
    out = []
    seen = set()
    for index, row in enumerate(rows):
        prediction_id = required_string(row, "predictionIdHex")
        if prediction_id in seen:
            raise SwebenchOpsError(
                "PANEL_B_PARITY_DUPLICATE_PREDICTION_ID",
                "Panel B score rows contain duplicate predictionIdHex",
                "Regenerate Panel B rows from a unique row-aligned source.",
                {"path": str(path), "predictionIdHex": prediction_id, "row_index": index},
            )
        seen.add(prediction_id)
        out.append(
            {
                "predictionIdHex": prediction_id,
                "scoreA": required_float(row, "scoreA"),
                "scoreB": required_float(row, "scoreB"),
                "oraclePass": required_bool(row, "oraclePass"),
                "acceptedLabelIds": require_string_array(row.get("acceptedLabelIds"), "acceptedLabelIds"),
                "failureEvidenceSetIds": require_string_array(
                    row.get("failureEvidenceSetIds"),
                    "failureEvidenceSetIds",
                ),
            }
        )
    return out


def load_model_score_rows(path: Path) -> list[dict[str, Any]]:
    rows = load_jsonl(path)
    out = []
    for row in rows:
        out.append(
            {
                "predictionIdHex": required_string(row, "predictionIdHex"),
                "taskId": required_string(row, "taskId"),
                "category": required_string(row, "category"),
                "scoreB": required_float(row, "scoreB"),
                "panelBModelScores": require_object(row.get("panelBModelScores"), "panelBModelScores"),
                "acceptedLabelIds": require_string_array(row.get("acceptedLabelIds"), "acceptedLabelIds"),
                "failureEvidenceSetIds": require_string_array(
                    row.get("failureEvidenceSetIds"),
                    "failureEvidenceSetIds",
                ),
            }
        )
    return out


def panel_metrics(rows: list[dict[str, Any]]) -> dict[str, Any]:
    labels = [1 if row["oraclePass"] else 0 for row in rows]
    scores_a = [float(row["scoreA"]) for row in rows]
    scores_b = [float(row["scoreB"]) for row in rows]
    return {
        "rowCount": len(rows),
        "panelA": metric_block(labels, scores_a),
        "panelB": metric_block(labels, scores_b),
        "absCorrelationDelta": abs_delta(
            metric_block(labels, scores_a).get("prediction_oracle_correlation"),
            metric_block(labels, scores_b).get("prediction_oracle_correlation"),
        ),
        "brierDeltaBMinusA": none_delta(metric_block(labels, scores_a).get("brier"), metric_block(labels, scores_b).get("brier")),
        "smoothEceDeltaBMinusA": none_delta(
            metric_block(labels, scores_a).get("smooth_ece"),
            metric_block(labels, scores_b).get("smooth_ece"),
        ),
    }


def label_surface(model_rows: list[dict[str, Any]]) -> dict[str, Any]:
    accepted = Counter()
    failure = Counter()
    category = Counter()
    slot_presence = Counter()
    for row in model_rows:
        category[row["category"]] += 1
        for label in row["acceptedLabelIds"]:
            accepted[label] += 1
        for label in row["failureEvidenceSetIds"]:
            failure[label] += 1
        for slot in row["panelBModelScores"]:
            slot_presence[slot] += 1
    return {
        "categoryRows": dict(sorted(category.items())),
        "slotScorePresence": dict(sorted(slot_presence.items())),
        "topAcceptedLabels": [{"label": key, "count": count} for key, count in accepted.most_common(20)],
        "topFailureEvidence": [{"label": key, "count": count} for key, count in failure.most_common(20)],
    }


def build_parity_gap_rows(
    full_rows: list[dict[str, Any]],
    active_rows: list[dict[str, Any]],
    calibration_diag: list[Any],
    holdout_diag: list[Any],
    model_rows: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    active_ids = {row["predictionIdHex"] for row in active_rows}
    model_by_id = {row["predictionIdHex"]: row for row in model_rows}
    rows = []
    for row in full_rows:
        model = model_by_id.get(row["predictionIdHex"], {})
        rows.append(
            {
                "predictionIdHex": row["predictionIdHex"],
                "inActive392Subset": row["predictionIdHex"] in active_ids,
                "scoreA": row["scoreA"],
                "scoreB": row["scoreB"],
                "oraclePass": row["oraclePass"],
                "hasAcceptedLabels": bool(row["acceptedLabelIds"]),
                "hasFailureEvidence": bool(row["failureEvidenceSetIds"]),
                "hasPanelBPerSlotScoresInStrictRow": False,
                "hasPanelBPerSlotScoresInModelRow": bool(model.get("panelBModelScores")),
                "has392EvidenceKeys": False,
                "has392CauseResidualFeatures": False,
                "calibrationPanelBScoreRowsAvailable": False,
            }
        )
    rows.append(
        {
            "summary": "diagnostic_row_counts",
            "calibrationDiagnostics": len(calibration_diag),
            "holdoutDiagnostics": len(holdout_diag),
            "fullPanelBRows": len(full_rows),
            "activePanelBRows": len(active_rows),
            "modelScoreRows": len(model_rows),
        }
    )
    return rows


def load_jsonl(path: Path) -> list[dict[str, Any]]:
    if not path.is_file():
        raise SwebenchOpsError(
            "PANEL_B_PARITY_INPUT_MISSING",
            f"input JSONL is missing: {path}",
            "Use retained aiwonder Panel B artifacts only.",
            {"path": str(path)},
        )
    rows = []
    with path.open("r", encoding="utf-8") as handle:
        for line_number, line in enumerate(handle, 1):
            if not line.strip():
                continue
            value = json.loads(line)
            rows.append(require_object(value, f"{path}:{line_number}"))
    return rows


def read_json_object(path: Path, label: str) -> dict[str, Any]:
    if not path.is_file():
        raise SwebenchOpsError(
            "PANEL_B_PARITY_INPUT_MISSING",
            f"{label} missing: {path}",
            "Use retained aiwonder READBACK artifacts only.",
            {"path": str(path)},
        )
    return require_object(json.loads(path.read_text(encoding="utf-8")), label)


def raise_count(label: str, expected: int, actual: int) -> None:
    raise SwebenchOpsError(
        "PANEL_B_PARITY_ROW_COUNT_MISMATCH",
        f"{label} row count mismatch",
        "Use the current row-aligned Panel B/#392 artifacts or update the expected count after proving the surface changed.",
        {"label": label, "expected": expected, "actual": actual},
    )


def require_object(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise SwebenchOpsError(
            "PANEL_B_PARITY_OBJECT_REQUIRED",
            f"{label} must be an object",
            "Fix the artifact schema before parity scoring.",
            {"label": label, "actual_type": type(value).__name__},
        )
    return value


def require_array(value: Any, label: str) -> list[Any]:
    if not isinstance(value, list):
        raise SwebenchOpsError(
            "PANEL_B_PARITY_ARRAY_REQUIRED",
            f"{label} must be an array",
            "Fix the artifact schema before parity scoring.",
            {"label": label, "actual_type": type(value).__name__},
        )
    return value


def require_string_array(value: Any, label: str) -> list[str]:
    raw = require_array(value, label)
    out = []
    for item in raw:
        if not isinstance(item, str):
            raise SwebenchOpsError(
                "PANEL_B_PARITY_STRING_ARRAY_REQUIRED",
                f"{label} contains a non-string item",
                "Fix the artifact schema before parity scoring.",
                {"label": label, "item": item},
            )
        out.append(item)
    return out


def required_string(row: dict[str, Any], key: str) -> str:
    value = row.get(key)
    if not isinstance(value, str) or not value:
        raise SwebenchOpsError(
            "PANEL_B_PARITY_STRING_REQUIRED",
            f"{key} must be a non-empty string",
            "Fix the artifact schema before parity scoring.",
            {"key": key, "actual": value},
        )
    return value


def required_bool(row: dict[str, Any], key: str) -> bool:
    value = row.get(key)
    if not isinstance(value, bool):
        raise SwebenchOpsError(
            "PANEL_B_PARITY_BOOL_REQUIRED",
            f"{key} must be boolean",
            "Fix the artifact schema before parity scoring.",
            {"key": key, "actual": value},
        )
    return value


def required_float(row: dict[str, Any], key: str) -> float:
    value = row.get(key)
    if not isinstance(value, (int, float)) or isinstance(value, bool):
        raise SwebenchOpsError(
            "PANEL_B_PARITY_FLOAT_REQUIRED",
            f"{key} must be numeric",
            "Fix the artifact schema before parity scoring.",
            {"key": key, "actual": value},
        )
    return float(value)


def none_delta(before: Any, after: Any) -> float | None:
    if before is None or after is None:
        return None
    return float(after) - float(before)


def abs_delta(before: Any, after: Any) -> float | None:
    delta = none_delta(before, after)
    return None if delta is None else abs(delta)
