from __future__ import annotations

import json
import math
from collections import defaultdict
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from .core import SwebenchOpsError, sha256_file, utc_now
from .python_per_cell_smooth_ece import (
    brier,
    fixed_bin_ece,
    metric_block,
    pearson_or_none,
    reject_mounted_drive,
    require_array,
    require_object,
    required_bool,
    required_score,
    required_string,
    smooth_ece,
    spearman_or_none,
    write_json_checked,
)


DEFAULT_SHIP_GATE_REPORT = Path(
    "/zfs/archive/contextgraph/readback/swebench-300x8-ship-gate-readback/"
    "run-20260521T132308Z/ship-gate-readback.json"
)
DEFAULT_OUTPUT_ROOT = Path(
    "/zfs/archive/contextgraph/readback/task-py-g-122-calibration-active-gate-readback"
)
ECE_BINS = 10
COVERAGE_GRID = (1.0, 0.95, 0.9, 0.85, 0.8, 0.75, 0.7, 0.65, 0.6, 0.55, 0.5, 0.4, 0.3, 0.2)
IDENTITY_BETA_PARAMS = (1.0, -1.0, 0.0)


@dataclass(frozen=True)
class Row:
    row_index: int
    task_id: str
    repo: str
    category: str
    patch_path: str
    patch_sha256: str
    label: int
    score: float


@dataclass(frozen=True)
class FittedPolicy:
    name: str
    global_beta: tuple[float, float, float] | None
    cell_beta: dict[str, tuple[float, float, float]]
    cell_shift: dict[str, float]
    global_shift: float


@dataclass(frozen=True)
class CellSelection:
    cell: str
    policy_name: str
    coverage: float
    margin_threshold: float | None
    reason: str
    calibration_metrics: dict[str, Any]


def add_arguments(parser: Any) -> None:
    parser.add_argument("--ship-gate-report", type=Path, default=DEFAULT_SHIP_GATE_REPORT)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--evidence-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--expected-calibration-count", type=int, default=240)
    parser.add_argument("--expected-holdout-count", type=int, default=240)
    parser.add_argument("--target-correlation", type=float, default=0.95)
    parser.add_argument("--smooth-ece-regression-budget", type=float, default=0.003)
    parser.add_argument("--shrinkage-strength", type=float, default=60.0)


def command_python_calibration_active_gate(args: Any) -> dict[str, Any]:
    ship_gate_report = args.ship_gate_report.expanduser().resolve(strict=False)
    output_root = args.output_root.expanduser().resolve(strict=False)
    reject_mounted_drive("ship_gate_report", ship_gate_report)
    reject_mounted_drive("output_root", output_root)
    if args.expected_calibration_count <= 0 or args.expected_holdout_count <= 0:
        raise SwebenchOpsError(
            "PY_G_122_EXPECTED_COUNT_INVALID",
            "expected calibration and holdout counts must be positive",
            "Pass the current split sizes, normally 240 and 240.",
            {
                "expected_calibration_count": args.expected_calibration_count,
                "expected_holdout_count": args.expected_holdout_count,
            },
        )
    if not 0.0 < args.target_correlation <= 1.0:
        raise SwebenchOpsError(
            "PY_G_122_TARGET_INVALID",
            "--target-correlation must be in (0, 1]",
            "Use the current ship-gate target, normally 0.95.",
            {"target_correlation": args.target_correlation},
        )

    payload = read_json_object(ship_gate_report)
    execute = require_object(payload.get("execute_and_inspect"), "execute_and_inspect")
    calibration_rows = load_rows(
        execute,
        "calibration_rank_lift_predictions",
        "calibration_predictions",
        expected=args.expected_calibration_count,
    )
    holdout_rows = load_rows(
        execute,
        "holdout_predictions",
        "holdout_rank_lift_predictions",
        expected=args.expected_holdout_count,
    )

    policies = fit_policies(
        calibration_rows,
        shrinkage_strength=float(args.shrinkage_strength),
    )
    selections = select_cells(
        calibration_rows,
        policies,
        target=float(args.target_correlation),
        smooth_ece_regression_budget=float(args.smooth_ece_regression_budget),
    )
    baseline_calibration = split_report(calibration_rows, policy=None, selections=None)
    baseline_holdout = split_report(holdout_rows, policy=None, selections=None)
    after_calibration = split_report(calibration_rows, policies=policies, selections=selections)
    after_holdout = split_report(holdout_rows, policies=policies, selections=selections)

    embedded_active = parse_embedded_active(execute)
    regression_cells = smooth_ece_regressions(
        before=baseline_holdout["perCell"],
        after=after_holdout["perCell"],
        budget=float(args.smooth_ece_regression_budget),
    )
    holdout_gate = holdout_gate_report(
        after_holdout,
        target=float(args.target_correlation),
        smooth_ece_regressions=regression_cells,
    )
    active_delta = None
    if isinstance(embedded_active.get("active_row_count"), int):
        active_delta = after_holdout["activeRowCount"] - int(embedded_active["active_row_count"])

    report = {
        "checked_at": utc_now(),
        "artifactKind": "task_py_g_122_calibration_active_gate",
        "githubIssue": 576,
        "ok": True,
        "shipGateReport": str(ship_gate_report),
        "shipGateReportSha256": sha256_file(ship_gate_report),
        "fitPartition": {
            "trainRowsAvailableInSourceReport": len(execute.get("train_predictions") or []),
            "calibrationRowsUsed": len(calibration_rows),
            "holdoutRowsUsedForFitting": 0,
            "thresholdSource": "calibration_rank_lift_predictions",
            "holdoutUsedOnlyForReporting": True,
        },
        "policy": {
            "candidatePolicies": list(policies.keys()),
            "coverageGrid": list(COVERAGE_GRID),
            "cellSelectionSource": "calibration_only",
            "targetCorrelation": float(args.target_correlation),
            "smoothEceRegressionBudget": float(args.smooth_ece_regression_budget),
            "shrinkageStrength": float(args.shrinkage_strength),
            "noNewPredictorFeatures": True,
            "noModelRetrain": True,
            "mountedDriveReadsOrWrites": False,
        },
        "selectedCells": {
            cell: {
                "policy": selection.policy_name,
                "coverage": selection.coverage,
                "marginThreshold": selection.margin_threshold,
                "reason": selection.reason,
                "calibrationMetrics": selection.calibration_metrics,
            }
            for cell, selection in sorted(selections.items())
        },
        "before": {
            "calibration": baseline_calibration,
            "holdout": baseline_holdout,
        },
        "after": {
            "calibration": after_calibration,
            "holdout": after_holdout,
        },
        "embeddedActiveDiagnostic": embedded_active,
        "holdoutGate": holdout_gate,
        "admissibleCoverageVsEmbeddedActiveRows": active_delta,
        "abstainPoolConclusion": abstain_pool_conclusion(embedded_active, after_holdout, holdout_gate),
        "stopRule": {
            "triggered": not holdout_gate["promotionAllowed"],
            "nextIssue": 577,
            "reason": "Move to TracIn/TRAK falsification before adding features if calibration-only repair is not promotable.",
        },
        "pathPolicy": {
            "aiwonderOnly": str(ship_gate_report).startswith("/zfs/archive/contextgraph/")
            and str(output_root).startswith("/zfs/archive/contextgraph/"),
            "mntDActiveReadsOrWrites": False,
        },
    }
    output_root.mkdir(parents=True, exist_ok=True)
    report_path = output_root / "calibration_active_gate_report.json"
    report["reportPath"] = str(report_path)
    write_json_checked(report_path, report)
    report["reportSha256"] = sha256_file(report_path)
    return report


def read_json_object(path: Path) -> dict[str, Any]:
    if not path.is_file():
        raise SwebenchOpsError(
            "PY_G_122_REPORT_MISSING",
            f"required ship-gate report missing: {path}",
            "Run or point at the current aiwonder ship-gate READBACK report.",
            {"path": str(path)},
        )
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise SwebenchOpsError(
            "PY_G_122_REPORT_BAD_JSON",
            f"ship-gate report is not valid JSON: {path}",
            "Regenerate the READBACK report before consuming it.",
            {"path": str(path), "error": str(exc)},
        ) from exc
    return require_object(value, str(path))


def load_rows(execute: dict[str, Any], preferred_key: str, fallback_key: str, *, expected: int) -> list[Row]:
    key = preferred_key if isinstance(execute.get(preferred_key), list) else fallback_key
    rows = require_array(execute.get(key), key)
    if len(rows) != expected:
        raise SwebenchOpsError(
            "PY_G_122_SPLIT_COUNT_MISMATCH",
            "source report split row count does not match expected count",
            "Use the current aiwonder ship-gate report or pass the expected split count explicitly.",
            {"key": key, "expected": expected, "actual": len(rows)},
        )
    out: list[Row] = []
    for index, value in enumerate(rows):
        row = require_object(value, f"{key}[{index}]")
        patch_path = required_string(row, "patch_path")
        reject_mounted_drive("patch_path", Path(patch_path))
        out.append(
            Row(
                row_index=index,
                task_id=required_string(row, "task_id"),
                repo=required_string(row, "repo"),
                category=required_string(row, "category"),
                patch_path=patch_path,
                patch_sha256=required_string(row, "patch_sha256"),
                label=1 if required_bool(row, "actual_oracle_pass") else 0,
                score=required_score(row, "predicted_oracle_pass"),
            )
        )
    return out


def fit_policies(rows: list[Row], *, shrinkage_strength: float) -> dict[str, FittedPolicy]:
    global_beta = fit_beta(rows, prior=IDENTITY_BETA_PARAMS)
    global_shift = sum(row.label - row.score for row in rows) / len(rows)
    by_cell = group_rows(rows)
    cell_beta: dict[str, tuple[float, float, float]] = {}
    cell_shift: dict[str, float] = {}
    for cell, cell_rows in by_cell.items():
        weight = len(cell_rows) / (len(cell_rows) + max(1.0, shrinkage_strength))
        cell_residual = sum(row.label - row.score for row in cell_rows) / len(cell_rows)
        cell_shift[cell] = (weight * cell_residual) + ((1.0 - weight) * global_shift)
        labels = {row.label for row in cell_rows}
        if len(labels) == 2 and min(sum(row.label for row in cell_rows), len(cell_rows) - sum(row.label for row in cell_rows)) >= 2:
            local = fit_beta(cell_rows, prior=global_beta)
            cell_beta[cell] = tuple((weight * local[idx]) + ((1.0 - weight) * global_beta[idx]) for idx in range(3))
        else:
            cell_beta[cell] = global_beta
    return {
        "identity": FittedPolicy("identity", None, {}, {}, 0.0),
        "global_beta": FittedPolicy("global_beta", global_beta, {}, {}, 0.0),
        "empirical_bayes_shift": FittedPolicy(
            "empirical_bayes_shift",
            None,
            {},
            cell_shift,
            global_shift,
        ),
        "empirical_bayes_beta": FittedPolicy(
            "empirical_bayes_beta",
            global_beta,
            cell_beta,
            {},
            0.0,
        ),
    }


def fit_beta(rows: list[Row], *, prior: tuple[float, float, float]) -> tuple[float, float, float]:
    params = [float(prior[0]), float(prior[1]), float(prior[2])]
    l2 = 0.03
    for step in range(900):
        grads = [0.0, 0.0, 0.0]
        for row in rows:
            features = beta_features(row.score)
            pred = sigmoid(sum(param * feature for param, feature in zip(params, features)))
            err = pred - row.label
            for idx in range(3):
                grads[idx] += err * features[idx]
        n = max(1, len(rows))
        lr = 0.08 / math.sqrt(1.0 + step / 100.0)
        for idx in range(3):
            grad = (grads[idx] / n) + l2 * (params[idx] - prior[idx])
            params[idx] -= lr * grad
            params[idx] = max(-12.0, min(12.0, params[idx]))
    return (params[0], params[1], params[2])


def select_cells(
    rows: list[Row],
    policies: dict[str, FittedPolicy],
    *,
    target: float,
    smooth_ece_regression_budget: float,
) -> dict[str, CellSelection]:
    selections: dict[str, CellSelection] = {}
    for cell, cell_rows in sorted(group_rows(rows).items()):
        baseline_scores = [row.score for row in cell_rows]
        baseline_labels = [row.label for row in cell_rows]
        baseline_smooth = smooth_ece(baseline_labels, baseline_scores) or 0.0
        best: CellSelection | None = None
        failures: list[str] = []
        for policy_name, policy in policies.items():
            scored = [(row, calibrate(policy, cell, row.score)) for row in cell_rows]
            ordered = sorted(scored, key=lambda item: (-abs(item[1] - 0.5), item[0].row_index))
            for coverage in COVERAGE_GRID:
                take = max(1, min(len(ordered), math.ceil(len(ordered) * coverage)))
                selected = ordered[:take]
                labels = [row.label for row, _score in selected]
                scores = [score for _row, score in selected]
                metrics = metric_block(labels, scores)
                passed, reason = cell_gate_passed(
                    labels,
                    scores,
                    metrics,
                    target=target,
                    max_smooth_ece=baseline_smooth + smooth_ece_regression_budget,
                )
                if not passed:
                    failures.append(f"{policy_name}@{coverage:.2f}:{reason}")
                    continue
                margin_threshold = min(abs(score - 0.5) for _row, score in selected)
                candidate = CellSelection(
                    cell=cell,
                    policy_name=policy_name,
                    coverage=coverage,
                    margin_threshold=margin_threshold,
                    reason="calibration_cell_gate_passed",
                    calibration_metrics=metrics,
                )
                if best is None or candidate.coverage > best.coverage:
                    best = candidate
                break
        if best is None:
            selections[cell] = CellSelection(
                cell=cell,
                policy_name="abstain_all",
                coverage=0.0,
                margin_threshold=None,
                reason="no_calibration_only_policy_met_cell_gate",
                calibration_metrics={"failures": failures[:24]},
            )
        else:
            selections[cell] = best
    return selections


def split_report(
    rows: list[Row],
    policy: FittedPolicy | None = None,
    policies: dict[str, FittedPolicy] | None = None,
    selections: dict[str, CellSelection] | None = None,
) -> dict[str, Any]:
    active_rows: list[tuple[Row, float]] = []
    per_cell: dict[str, Any] = {}
    for cell, cell_rows in sorted(group_rows(rows).items()):
        if selections is None:
            scored = [(row, calibrate(policy, cell, row.score) if policy else row.score) for row in cell_rows]
            selected = scored
            selection = None
        else:
            selection = selections[cell]
            if selection.policy_name == "abstain_all":
                selected = []
            else:
                assert policies is not None
                selected_policy = policies[selection.policy_name]
                scored = [(row, calibrate(selected_policy, cell, row.score)) for row in cell_rows]
                selected = [
                    (row, score)
                    for row, score in scored
                    if selection.margin_threshold is not None and abs(score - 0.5) >= selection.margin_threshold
                ]
        active_rows.extend(selected)
        per_cell[cell] = selection_report(cell_rows, selected, selection)
    labels = [row.label for row, _score in active_rows]
    scores = [score for _row, score in active_rows]
    return {
        "rowCount": len(rows),
        "activeRowCount": len(active_rows),
        "abstainRowCount": len(rows) - len(active_rows),
        "coverage": len(active_rows) / len(rows) if rows else None,
        "metrics": metric_block(labels, scores) if active_rows else empty_metric_block(),
        "perCell": per_cell,
    }


def selection_report(
    all_rows: list[Row],
    selected: list[tuple[Row, float]],
    selection: CellSelection | None,
) -> dict[str, Any]:
    labels = [row.label for row, _score in selected]
    scores = [score for _row, score in selected]
    all_labels = [row.label for row in all_rows]
    all_scores = [row.score for row in all_rows]
    return {
        "rowCount": len(all_rows),
        "activeRowCount": len(selected),
        "abstainRowCount": len(all_rows) - len(selected),
        "coverage": len(selected) / len(all_rows) if all_rows else None,
        "passCount": sum(all_labels),
        "failCount": len(all_labels) - sum(all_labels),
        "selectedPassCount": sum(labels),
        "selectedFailCount": len(labels) - sum(labels),
        "selection": None
        if selection is None
        else {
            "policy": selection.policy_name,
            "coverage": selection.coverage,
            "marginThreshold": selection.margin_threshold,
            "reason": selection.reason,
        },
        "metrics": metric_block(labels, scores) if selected else empty_metric_block(),
        "fullCellBeforeMetrics": metric_block(all_labels, all_scores),
    }


def holdout_gate_report(
    report: dict[str, Any],
    *,
    target: float,
    smooth_ece_regressions: list[dict[str, Any]],
) -> dict[str, Any]:
    blockers: list[str] = []
    metrics = report["metrics"]
    corr = metrics.get("prediction_oracle_correlation")
    if corr is None or corr < target:
        blockers.append(f"global_active_correlation {corr} < {target}")
    if smooth_ece_regressions:
        blockers.append(f"{len(smooth_ece_regressions)} per-cell SmoothECE regressions beyond budget")
    for cell, cell_report in report["perCell"].items():
        if cell_report["activeRowCount"] == 0:
            blockers.append(f"{cell} all rows abstained")
            continue
        cell_metrics = cell_report["metrics"]
        labels = [1] * int(cell_report["selectedPassCount"]) + [0] * int(cell_report["selectedFailCount"])
        passed, reason = cell_gate_passed(
            labels,
            [],
            cell_metrics,
            target=target,
            max_smooth_ece=1.0,
        )
        if not passed:
            blockers.append(f"{cell} {reason}")
    return {
        "promotionAllowed": not blockers,
        "blockers": blockers,
        "smoothEceRegressions": smooth_ece_regressions,
    }


def cell_gate_passed(
    labels: list[int],
    scores: list[float],
    metrics: dict[str, Any],
    *,
    target: float,
    max_smooth_ece: float,
) -> tuple[bool, str]:
    if len(labels) < 3:
        return False, "too_few_active_rows"
    smooth = metrics.get("smooth_ece")
    if smooth is None or smooth > max_smooth_ece:
        return False, f"smooth_ece {smooth} > {max_smooth_ece}"
    if len(set(labels)) == 2:
        corr = metrics.get("prediction_oracle_correlation")
        if corr is None or corr < target:
            return False, f"correlation {corr} < {target}"
        return True, "mixed_cell_passed"
    if scores:
        target_label = labels[0]
        recall = sum(1 for label, score in zip(labels, scores) if (score >= 0.5) == bool(target_label)) / len(labels)
        brier_value = brier(labels, scores)
    else:
        recall = metrics.get("accuracy_at_half")
        brier_value = metrics.get("brier")
    if recall is None or recall < target:
        return False, f"single_class_recall {recall} < {target}"
    if brier_value is None or brier_value > 0.05:
        return False, f"single_class_brier {brier_value} > 0.05"
    return True, "single_class_cell_passed"


def smooth_ece_regressions(before: dict[str, Any], after: dict[str, Any], *, budget: float) -> list[dict[str, Any]]:
    regressions = []
    for cell, before_cell in before.items():
        after_cell = after.get(cell)
        if not after_cell or after_cell["activeRowCount"] == 0:
            continue
        before_value = before_cell["metrics"].get("smooth_ece")
        after_value = after_cell["metrics"].get("smooth_ece")
        if before_value is None or after_value is None:
            continue
        delta = after_value - before_value
        if delta > budget:
            regressions.append(
                {
                    "cell": cell,
                    "beforeSmoothEce": before_value,
                    "afterSmoothEce": after_value,
                    "delta": delta,
                    "budget": budget,
                }
            )
    return regressions


def abstain_pool_conclusion(
    embedded_active: dict[str, Any],
    after_holdout: dict[str, Any],
    holdout_gate: dict[str, Any],
) -> str:
    embedded_count = embedded_active.get("active_row_count")
    if holdout_gate["promotionAllowed"]:
        return "calibration_only_policy_makes_holdout_admissible"
    if isinstance(embedded_count, int) and after_holdout["activeRowCount"] <= embedded_count:
        return "abstain_pool_remains_abstain_or_loses_coverage_under_calibration_only_policy"
    return "calibration_only_policy_adds_rows_but_holdout_gate_still_fails"


def parse_embedded_active(execute: dict[str, Any]) -> dict[str, Any]:
    raw = execute.get("task_py_g_392_active_abstain_ship_gate_diagnostic")
    if not isinstance(raw, dict):
        return {"present": False}
    metrics = raw.get("active_metrics") if isinstance(raw.get("active_metrics"), dict) else {}
    return {
        "present": True,
        "active_row_count": raw.get("active_row_count"),
        "abstained_row_count": raw.get("abstained_row_count"),
        "active_coverage": raw.get("active_coverage"),
        "active_correlation": metrics.get("prediction_oracle_correlation"),
        "active_ece": metrics.get("ece"),
        "active_brier": metrics.get("brier"),
        "active_metric_blockers": raw.get("active_metric_blockers"),
    }


def calibrate(policy: FittedPolicy | None, cell: str, score: float) -> float:
    if policy is None or policy.name == "identity":
        return clamp01(score)
    if policy.name == "global_beta":
        assert policy.global_beta is not None
        return beta_apply(policy.global_beta, score)
    if policy.name == "empirical_bayes_shift":
        shift = policy.cell_shift.get(cell, policy.global_shift)
        return clamp01(score + shift)
    if policy.name == "empirical_bayes_beta":
        params = policy.cell_beta.get(cell) or policy.global_beta
        assert params is not None
        return beta_apply(params, score)
    raise AssertionError(f"unknown policy: {policy.name}")


def beta_apply(params: tuple[float, float, float], score: float) -> float:
    features = beta_features(score)
    return sigmoid(sum(param * feature for param, feature in zip(params, features)))


def beta_features(score: float) -> tuple[float, float, float]:
    clipped = min(1.0 - 1.0e-6, max(1.0e-6, score))
    return (math.log(clipped), math.log(1.0 - clipped), 1.0)


def sigmoid(value: float) -> float:
    if value >= 0:
        z = math.exp(-value)
        return 1.0 / (1.0 + z)
    z = math.exp(value)
    return z / (1.0 + z)


def clamp01(value: float) -> float:
    return min(1.0, max(0.0, float(value)))


def group_rows(rows: list[Row]) -> dict[str, list[Row]]:
    grouped: dict[str, list[Row]] = defaultdict(list)
    for row in rows:
        grouped[f"{row.category}::python"].append(row)
    return grouped


def empty_metric_block() -> dict[str, Any]:
    return {
        "n": 0,
        "prediction_oracle_correlation": None,
        "spearman_prediction_oracle_correlation": None,
        "brier": None,
        "fixed_bin_ece": None,
        "smooth_ece": None,
        "accuracy_at_half": None,
        "mean_score": None,
        "mean_label": None,
    }
