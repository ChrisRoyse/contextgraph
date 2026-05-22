from __future__ import annotations

import json
import math
import re
from collections import Counter, defaultdict
from itertools import combinations
from pathlib import Path
from typing import Any

from .core import SwebenchOpsError, finite_float, sha256_file, utc_now, write_json_checked
from .python_pair_feature_audit import read_json_object, scan_head_projection, write_jsonl
from .python_pair_surface_mining import (
    CALIBRATION_KEY,
    DEFAULT_CODE_STATES_JSONL,
    DEFAULT_LABEL_MANIFEST,
    DEFAULT_PAIR_AUDIT_REPORT,
    DEFAULT_ROW_DIAGNOSTICS_REPORT,
    EXPECTED_DOCUMENTED_SLOTS,
    HOLDOUT_KEY,
    classify_pair_universe,
    derived_pair_records,
    load_code_states,
    load_diagnostic_rows,
    pair_id,
    require_state,
    slot_family,
    target_workspace_state,
)
from .python_per_cell_smooth_ece import metric_block, power_blockers, reject_mounted_drive


DEFAULT_HEAD_PROJECTION_RS = Path("crates/context-graph-mejepa/src/head_projection.rs")
DEFAULT_OUTPUT_ROOT = Path("/zfs/archive/contextgraph/fsv/task-py-g-124d-mendeleev-pair-table-fsv")
PANEL_SLOT_ORDER = (
    "E_AST",
    "E_CFG",
    "E_DataFlow",
    "E_TypeGraph",
    "E_Test",
    "E_Trace",
    "E_Diff",
    "E_Witness",
    "E_Oracle",
    "E_Problem",
    "E_CommitMsg",
    "E_StaticAnalysis",
    "E_Runtime",
    "E_Reasoning",
    "Scalars",
)
HEAD_ORDER = (
    "panel",
    "oracle",
    "failure_mode",
    "edge_case",
    "tech_debt",
    "perf",
    "security",
    "accuracy",
    "cost",
    "reasoning",
)
HEAD_ENUM_TO_NAME = {
    "Panel": "panel",
    "Oracle": "oracle",
    "FailureMode": "failure_mode",
    "EdgeCase": "edge_case",
    "TechDebt": "tech_debt",
    "Perf": "perf",
    "Security": "security",
    "Accuracy": "accuracy",
    "Cost": "cost",
    "Reasoning": "reasoning",
}
PAIRWISE_RE = re.compile(r"pairwise\(\s*([^,\s]+)\s*,\s*([^)]+?)\s*\)")
HEAD_ARM_RE = re.compile(r"HeadId::([A-Za-z]+)\s*=>\s*vec!\[(.*?)\]", re.DOTALL)
HEAD_SLOT_CONST_RE = re.compile(r'pub const (E_[A-Z_]+): &str = "([^"]+)";')
WEAK_CELLS = {"delete_test_call::python", "off_by_one::python", "subtle_flip::python"}
TARGETED_392_CAUSE_PAIRS = {
    "e1/e6",
    "e10/e13",
    "e13/e6",
    "e13/e7",
    "e14/e6",
    "e2/e6",
    "e3/e6",
    "e4/e6",
    "e6/e7",
}
FOUNDATIONALITY_KEYS = (
    "mean_foundationality",
    "foundationality_mean",
    "chunk_foundationality_mean",
    "foundationalityScore",
    "foundationality_score",
)


def add_arguments(parser: Any) -> None:
    parser.add_argument("--code-states-jsonl", type=Path, default=DEFAULT_CODE_STATES_JSONL)
    parser.add_argument("--label-manifest", type=Path, default=DEFAULT_LABEL_MANIFEST)
    parser.add_argument("--row-diagnostics-report", type=Path, default=DEFAULT_ROW_DIAGNOSTICS_REPORT)
    parser.add_argument("--pair-audit-report", type=Path, default=DEFAULT_PAIR_AUDIT_REPORT)
    parser.add_argument("--head-projection-rs", type=Path, default=DEFAULT_HEAD_PROJECTION_RS)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--evidence-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--expected-code-state-count", type=int, default=3000)
    parser.add_argument("--expected-calibration-count", type=int, default=240)
    parser.add_argument("--expected-holdout-count", type=int, default=240)
    parser.add_argument("--expected-pair-count", type=int, default=105)
    parser.add_argument("--top-hole-count", type=int, default=100)


def command_python_mendeleev_pair_table(args: Any) -> dict[str, Any]:
    code_states_jsonl = args.code_states_jsonl.expanduser().resolve(strict=False)
    label_manifest = args.label_manifest.expanduser().resolve(strict=False)
    row_diagnostics_report = args.row_diagnostics_report.expanduser().resolve(strict=False)
    pair_audit_report = args.pair_audit_report.expanduser().resolve(strict=False)
    head_projection_rs = args.head_projection_rs.expanduser().resolve(strict=False)
    output_root = args.output_root.expanduser().resolve(strict=False)
    for name, path in (
        ("code_states_jsonl", code_states_jsonl),
        ("label_manifest", label_manifest),
        ("row_diagnostics_report", row_diagnostics_report),
        ("pair_audit_report", pair_audit_report),
        ("head_projection_rs", head_projection_rs),
        ("output_root", output_root),
    ):
        reject_mounted_drive(name, path)
    if int(args.top_hole_count) <= 0:
        raise SwebenchOpsError(
            "PY_G_124D_INVALID_TOP_HOLE_COUNT",
            "top-hole-count must be positive",
            "Use a positive top-hole-count.",
            {"top_hole_count": args.top_hole_count},
        )

    output_root.mkdir(parents=True, exist_ok=True)
    label_manifest_payload = read_json_object(label_manifest, "label_manifest")
    pair_audit_payload = read_json_object(pair_audit_report, "pair_audit_report")
    states = load_code_states(code_states_jsonl, expected=int(args.expected_code_state_count))
    diagnostics = read_json_object(row_diagnostics_report, "row_diagnostics_report")
    calibration = load_diagnostic_rows(
        diagnostics,
        CALIBRATION_KEY,
        expected=int(args.expected_calibration_count),
    )
    holdout = load_diagnostic_rows(
        diagnostics,
        HOLDOUT_KEY,
        expected=int(args.expected_holdout_count),
    )
    universe_rows, universe_summary = classify_pair_universe(
        states,
        pair_audit_payload,
        int(args.expected_pair_count),
    )
    universe_by_pair = {row["pairId"]: row for row in universe_rows}
    head_projection = parse_head_projection_specs(head_projection_rs)
    raw_head_projection = scan_head_projection(Path.cwd())

    panel_table = panel_head_consumption_table(head_projection)
    active_table, hole_rows = active_pair_structural_table(
        calibration,
        holdout,
        states,
        universe_by_pair,
        head_projection,
    )
    top_holes = sorted(hole_rows, key=lambda row: (-float(row["expectedLiftScore"]), row["head"], row["pairId"]))[
        : int(args.top_hole_count)
    ]

    panel_table_path = output_root / "head_projection_panel_consumption_table.jsonl"
    active_table_path = output_root / "active_pair_mendeleev_table.jsonl"
    top_holes_path = output_root / "structural_hole_rankings.jsonl"
    report_path = output_root / "mendeleev_pair_table_report.json"
    write_jsonl(panel_table_path, panel_table)
    write_jsonl(active_table_path, active_table)
    write_jsonl(top_holes_path, top_holes)

    report = {
        "artifactKind": "task_py_g_124d_mendeleev_pair_consumption_table",
        "checkedAt": utc_now(),
        "githubIssue": 585,
        "ok": True,
        "inputArtifacts": {
            "codeStatesJsonl": str(code_states_jsonl),
            "codeStatesJsonlSha256": sha256_file(code_states_jsonl),
            "labelManifest": str(label_manifest),
            "labelManifestSha256": sha256_file(label_manifest),
            "rowDiagnosticsReport": str(row_diagnostics_report),
            "rowDiagnosticsReportSha256": sha256_file(row_diagnostics_report),
            "pairAuditReport": str(pair_audit_report),
            "pairAuditReportSha256": sha256_file(pair_audit_report),
            "headProjectionRs": str(head_projection_rs),
            "headProjectionRsSha256": sha256_file(head_projection_rs),
        },
        "labelManifestLineCounts": label_manifest_payload.get("line_counts"),
        "pairUniverse": universe_summary,
        "headProjection": {
            "parsedConsumedPairSpecCount": sum(len(v) for v in head_projection.values()),
            "parsedByHead": head_projection,
            "rawAudit": raw_head_projection,
        },
        "namespaceFinding": {
            "panelSlotTable": "head_projection.rs uses E_* panel/projection names",
            "activePairTable": "current #117/#392 pair mining uses active e* corpus slot ids",
            "status": "both namespaces are reported separately; no synthetic mapping is inferred",
        },
        "baseline": {
            "calibration": split_report(calibration),
            "holdout": split_report(holdout),
        },
        "tableSummary": {
            "panelHeadPairRows": len(panel_table),
            "activeHeadPairRows": len(active_table),
            "rankedStructuralHoleRows": len(hole_rows),
            "topHoleCount": len(top_holes),
            "topHoles": top_holes[:20],
            "byHeadTopHoleCount": dict(Counter(row["head"] for row in top_holes)),
            "byCellTopHoleCount": dict(Counter(row["cell"] for row in top_holes)),
        },
        "weakCellSummary": weak_cell_summary(top_holes),
        "policy": {
            "readOnlyDiagnostic": True,
            "selectionSplit": "calibration_only",
            "holdoutUse": "reporting_only",
            "noNewEmbedders": True,
            "noFlatVectorShortcut": True,
            "preservesSlotIdentity": True,
            "noPredictorWiring": True,
        },
        "pathPolicy": {"aiwonderOnly": True, "mntDActiveReadsOrWrites": False},
        "outputArtifacts": {
            "headProjectionPanelConsumptionTableJsonl": str(panel_table_path),
            "headProjectionPanelConsumptionTableSha256": sha256_file(panel_table_path),
            "activePairMendeleevTableJsonl": str(active_table_path),
            "activePairMendeleevTableSha256": sha256_file(active_table_path),
            "structuralHoleRankingsJsonl": str(top_holes_path),
            "structuralHoleRankingsSha256": sha256_file(top_holes_path),
        },
        "nextRecommendation": recommendation(top_holes),
        "reportPath": str(report_path),
    }
    write_json_checked(report_path, report)
    report["reportSha256"] = sha256_file(report_path)
    return report


def parse_head_projection_specs(path: Path) -> dict[str, list[dict[str, str]]]:
    if not path.is_file():
        raise SwebenchOpsError(
            "PY_G_124D_HEAD_PROJECTION_MISSING",
            f"head projection source missing: {path}",
            "Run from a ContextGraph checkout or pass --head-projection-rs.",
            {"path": str(path)},
        )
    text = path.read_text(encoding="utf-8")
    out = {head: [] for head in HEAD_ORDER}
    slot_constants = dict(HEAD_SLOT_CONST_RE.findall(text))
    for enum_name, body in HEAD_ARM_RE.findall(text):
        head = HEAD_ENUM_TO_NAME.get(enum_name)
        if head is None:
            continue
        for left, right in PAIRWISE_RE.findall(body):
            left_symbol = left.strip()
            right_symbol = right.strip()
            left_clean = slot_constants.get(left_symbol, left_symbol)
            right_clean = slot_constants.get(right_symbol, right_symbol)
            out[head].append(
                {
                    "left": left_clean,
                    "right": right_clean,
                    "leftSourceSymbol": left_symbol,
                    "rightSourceSymbol": right_symbol,
                    "pairId": panel_pair_id(left_clean, right_clean),
                }
            )
    return out


def panel_head_consumption_table(head_projection: dict[str, list[dict[str, str]]]) -> list[dict[str, Any]]:
    pairs = [panel_pair_id(left, right) for left, right in combinations(PANEL_SLOT_ORDER, 2)]
    rows = []
    for head in HEAD_ORDER:
        consumed = {spec["pairId"] for spec in head_projection.get(head, [])}
        for pair in pairs:
            left, right = pair.split("/", 1)
            rows.append(
                {
                    "namespace": "panel_head_projection",
                    "head": head,
                    "pairId": pair,
                    "leftSlot": left,
                    "rightSlot": right,
                    "documentedUniverse": True,
                    "consumedByHeadProjection": pair in consumed,
                    "structuralHole": pair not in consumed,
                }
            )
    return rows


def active_pair_structural_table(
    calibration: list[dict[str, Any]],
    holdout: list[dict[str, Any]],
    states: dict[tuple[str, str], dict[str, Any]],
    universe_by_pair: dict[str, dict[str, Any]],
    head_projection: dict[str, list[dict[str, str]]],
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    rows = []
    holes = []
    cells = sorted({f"{row['category']}::python" for row in calibration})
    active_pairs = sorted(universe_by_pair)
    consumed_panel_specs = {spec["pairId"] for specs in head_projection.values() for spec in specs}
    for head in HEAD_ORDER:
        for pair in active_pairs:
            universe = universe_by_pair[pair]
            active_derivable = "derivable_from_slot_feature_means" in universe["status"]
            for cell in cells:
                cal_rows = [row for row in calibration if f"{row['category']}::python" == cell]
                hold_rows = [row for row in holdout if f"{row['category']}::python" == cell]
                stats = expected_lift_components(cal_rows, hold_rows, states, pair, head, cell)
                consumed_by_392 = pair in TARGETED_392_CAUSE_PAIRS
                dark_derivable = "dark_not_in_top_pair_counts" in universe["status"]
                inactive = "inactive_missing_current_label_root" in universe["status"]
                rejection = rejection_reasons(stats, active_derivable, inactive)
                score = 0.0 if rejection else stats["expectedLiftScore"]
                row = {
                    "namespace": "active_code_state_pairs",
                    "head": head,
                    "pairId": pair,
                    "leftSlot": pair.split("/")[0],
                    "rightSlot": pair.split("/")[1],
                    "leftFamily": slot_family(pair.split("/")[0]),
                    "rightFamily": slot_family(pair.split("/")[1]),
                    "cell": cell,
                    "documentedUniverse": True,
                    "activeDerivable": active_derivable,
                    "inactiveMissing": inactive,
                    "materializedInLabelTopCounts": universe.get("codeStateTopCount", 0) > 0,
                    "darkDerivable": dark_derivable,
                    "consumedBy392Evidence": consumed_by_392,
                    "consumedByHeadProjection": False,
                    "headProjectionNamespaceStatus": "unmapped_E_panel_pairs_reported_separately",
                    "headProjectionPanelConsumedPairSpecs": sorted(consumed_panel_specs),
                    **stats,
                    "expectedLiftScore": finite_float(score),
                    "rejectionReasons": rejection,
                    "structuralHole": active_derivable and not consumed_by_392 and not rejection and score > 0.0,
                }
                rows.append(row)
                if row["structuralHole"]:
                    holes.append(row)
    return rows, holes


def expected_lift_components(
    calibration: list[dict[str, Any]],
    holdout: list[dict[str, Any]],
    states: dict[tuple[str, str], dict[str, Any]],
    pair: str,
    head: str,
    cell: str,
) -> dict[str, Any]:
    cal_values = pair_measurements(calibration, states, pair)
    hold_values = pair_measurements(holdout, states, pair)
    n = len(cal_values)
    labels = [row["actual"] for row in calibration]
    scores = [row["predicted_oracle_pass"] for row in calibration]
    briers = [row["brier_residual"] for row in calibration]
    abs_errors = [row["absolute_error"] for row in calibration]
    active_flags = [1 if row["active_after_abstain"] else 0 for row in calibration]
    feature_names = ("targetMaxDelta", "deltaFromBase", "stateOrdinal", "driverChanged")
    dcor_by_feature = {}
    for name in feature_names:
        xs = [float(item[name]) for item in cal_values]
        dcor_by_feature[name] = {
            "brier": distance_correlation(xs, briers[: len(xs)]),
            "absError": distance_correlation(xs, abs_errors[: len(xs)]),
            "oracleLabel": distance_correlation(xs, labels[: len(xs)]),
            "active": distance_correlation(xs, active_flags[: len(xs)]),
        }
    best = best_dependence(dcor_by_feature, head)
    abstain_probability = (
        sum(1 for row in calibration if not row["active_after_abstain"]) / len(calibration)
        if calibration
        else 0.0
    )
    foundationality = mean_foundationality(states, calibration, pair)
    foundationality_used = foundationality if foundationality is not None else 1.0
    pressure = max(abstain_probability, 0.30 if cell in WEAK_CELLS else 0.05)
    power_weight = min(1.0, n / 30.0) if n > 0 else 0.0
    score = best * foundationality_used * pressure * power_weight * head_weight(head, cell)
    return {
        "calibrationSupport": n,
        "holdoutSupportReportingOnly": len(hold_values),
        "calibrationMetrics": metric_block(labels, scores) if calibration else None,
        "holdoutMetricsReportingOnly": metric_block(
            [row["actual"] for row in holdout],
            [row["predicted_oracle_pass"] for row in holdout],
        )
        if holdout
        else None,
        "calibrationPowerBlockers": power_blockers(labels, scores),
        "abstainPoolProbability": finite_float(abstain_probability),
        "meanFoundationality": finite_float(foundationality) if foundationality is not None else None,
        "foundationalityStatus": "used" if foundationality is not None else "foundationality_unavailable_reduced_score",
        "distanceCorrelationByFeature": dcor_by_feature,
        "bestDependence": finite_float(best),
        "powerWeight": finite_float(power_weight),
        "headWeight": finite_float(head_weight(head, cell)),
        "expectedLiftScore": finite_float(score),
        "holdoutDependenceReportingOnly": holdout_dependence(hold_values, holdout, head),
    }


def pair_measurements(
    rows: list[dict[str, Any]],
    states: dict[tuple[str, str], dict[str, Any]],
    pair: str,
) -> list[dict[str, Any]]:
    out = []
    for row in rows:
        base = require_state(states, row["task_id"], "base")
        target = require_state(states, row["task_id"], target_workspace_state(row["category"]))
        base_pair = derived_pair_records(base).get(pair)
        target_pair = derived_pair_records(target).get(pair)
        if base_pair is None or target_pair is None:
            continue
        out.append(
            {
                "targetMaxDelta": float(target_pair["max_delta"]),
                "deltaFromBase": float(target_pair["max_delta"]) - float(base_pair["max_delta"]),
                "stateOrdinal": pair_state_ordinal(str(target_pair["state"])),
                "driverChanged": 0.0 if target_pair["driver"] == base_pair["driver"] else 1.0,
            }
        )
    return out


def holdout_dependence(hold_values: list[dict[str, Any]], holdout: list[dict[str, Any]], head: str) -> dict[str, Any]:
    if not hold_values or not holdout:
        return {}
    briers = [row["brier_residual"] for row in holdout]
    abs_errors = [row["absolute_error"] for row in holdout]
    labels = [row["actual"] for row in holdout]
    active_flags = [1 if row["active_after_abstain"] else 0 for row in holdout]
    out = {}
    for name in ("targetMaxDelta", "deltaFromBase", "stateOrdinal", "driverChanged"):
        xs = [float(item[name]) for item in hold_values]
        values = {
            "brier": distance_correlation(xs, briers[: len(xs)]),
            "absError": distance_correlation(xs, abs_errors[: len(xs)]),
            "oracleLabel": distance_correlation(xs, labels[: len(xs)]),
            "active": distance_correlation(xs, active_flags[: len(xs)]),
        }
        out[name] = values
    out["bestDependence"] = finite_float(best_dependence(out, head))
    return out


def best_dependence(dcor_by_feature: dict[str, dict[str, float | None]], head: str) -> float:
    channels = ("brier", "absError", "oracleLabel", "active")
    if head in {"oracle", "panel", "accuracy"}:
        channels = ("brier", "oracleLabel", "active")
    elif head == "failure_mode":
        channels = ("brier", "absError")
    elif head == "reasoning":
        channels = ("active", "absError", "brier")
    best = 0.0
    for values in dcor_by_feature.values():
        for channel in channels:
            value = values.get(channel)
            if value is not None:
                best = max(best, float(value))
    return best


def rejection_reasons(stats: dict[str, Any], active_derivable: bool, inactive: bool) -> list[str]:
    reasons = []
    if inactive or not active_derivable:
        reasons.append("PAIR_NOT_DERIVABLE_FROM_CURRENT_ACTIVE_LABEL_ROOT")
    if stats["calibrationSupport"] < 5:
        reasons.append("LOW_SUPPORT_LT_5")
    if stats["bestDependence"] <= 0.0:
        reasons.append("NO_DISTANCE_DEPENDENCE")
    if stats["calibrationSupport"] < 30:
        reasons.append("POWER_FLOOR_LT_30")
    return reasons


def mean_foundationality(
    states: dict[tuple[str, str], dict[str, Any]],
    rows: list[dict[str, Any]],
    _pair: str,
) -> float | None:
    values = []
    for row in rows:
        target = require_state(states, row["task_id"], target_workspace_state(row["category"]))
        value = first_foundationality_value(target)
        if value is not None:
            values.append(value)
    if not values:
        return None
    return sum(values) / len(values)


def first_foundationality_value(row: dict[str, Any]) -> float | None:
    for key in FOUNDATIONALITY_KEYS:
        value = row.get(key)
        if isinstance(value, (int, float)) and not isinstance(value, bool) and math.isfinite(float(value)):
            return float(value)
    return None


def distance_correlation(xs: list[float], ys: list[float]) -> float | None:
    n = min(len(xs), len(ys))
    if n < 3:
        return None
    xs = xs[:n]
    ys = ys[:n]
    if len({round(x, 12) for x in xs}) < 2 or len({round(y, 12) for y in ys}) < 2:
        return None
    ax = centered_distance_matrix(xs)
    ay = centered_distance_matrix(ys)
    dcov2 = sum(ax[i][j] * ay[i][j] for i in range(n) for j in range(n)) / (n * n)
    dvarx2 = sum(ax[i][j] * ax[i][j] for i in range(n) for j in range(n)) / (n * n)
    dvary2 = sum(ay[i][j] * ay[i][j] for i in range(n) for j in range(n)) / (n * n)
    denom = math.sqrt(max(dvarx2, 0.0) * max(dvary2, 0.0))
    if denom <= 1.0e-12:
        return None
    return finite_float(math.sqrt(max(dcov2 / denom, 0.0)))


def centered_distance_matrix(values: list[float]) -> list[list[float]]:
    n = len(values)
    matrix = [[abs(values[i] - values[j]) for j in range(n)] for i in range(n)]
    row_means = [sum(row) / n for row in matrix]
    col_means = [sum(matrix[i][j] for i in range(n)) / n for j in range(n)]
    grand = sum(row_means) / n
    return [
        [matrix[i][j] - row_means[i] - col_means[j] + grand for j in range(n)]
        for i in range(n)
    ]


def head_weight(head: str, cell: str) -> float:
    base = {
        "panel": 1.0,
        "oracle": 1.25,
        "failure_mode": 1.20,
        "edge_case": 0.75,
        "tech_debt": 0.65,
        "perf": 0.55,
        "security": 0.55,
        "accuracy": 0.80,
        "cost": 0.40,
        "reasoning": 0.70,
    }.get(head, 0.5)
    if cell in WEAK_CELLS and head in {"panel", "oracle", "failure_mode", "accuracy"}:
        base *= 1.25
    return base


def pair_state_ordinal(state: str) -> float:
    return {
        "near_identity_cross_slot_delta": 0.0,
        "low_cross_slot_delta": 1.0,
        "moderate_cross_slot_delta": 2.0,
        "high_cross_slot_delta": 3.0,
        "extreme_cross_slot_delta": 4.0,
    }.get(state, 0.0)


def split_report(rows: list[dict[str, Any]]) -> dict[str, Any]:
    active = [row for row in rows if row["active_after_abstain"]]
    abstain = [row for row in rows if not row["active_after_abstain"]]
    return {
        "full": metric_block([row["actual"] for row in rows], [row["predicted_oracle_pass"] for row in rows]),
        "active392": metric_block(
            [row["actual"] for row in active],
            [row["predicted_oracle_pass"] for row in active],
        )
        if active
        else None,
        "abstainPool": metric_block(
            [row["actual"] for row in abstain],
            [row["predicted_oracle_pass"] for row in abstain],
        )
        if abstain
        else None,
        "rowCounts": {"full": len(rows), "active392": len(active), "abstainPool": len(abstain)},
    }


def weak_cell_summary(top_holes: list[dict[str, Any]]) -> dict[str, Any]:
    out = {}
    for cell in sorted(WEAK_CELLS):
        rows = [row for row in top_holes if row["cell"] == cell]
        out[cell] = rows[:10]
    return out


def recommendation(top_holes: list[dict[str, Any]]) -> dict[str, Any]:
    if not top_holes:
        return {
            "recommendedAction": "retain_negative_no_ranked_pair_holes",
            "reason": "no calibration-stable active pair holes survived support and dependence filters",
        }
    primary = [
        row
        for row in top_holes
        if row["head"] in {"oracle", "panel", "failure_mode", "accuracy"}
        and row["cell"] in WEAK_CELLS
    ]
    candidates = primary or top_holes
    return {
        "recommendedAction": "feed_top_ranked_holes_to_584_after_586_calibration_substrate",
        "topCandidateCount": len(candidates),
        "topCandidate": candidates[0],
        "blockedUntil": ["#586 per-cell calibration substrate", "#584 named-pair selector retest"],
    }


def panel_pair_id(left: str, right: str) -> str:
    order = {slot: idx for idx, slot in enumerate(PANEL_SLOT_ORDER)}
    left_idx = order.get(left, 10_000)
    right_idx = order.get(right, 10_000)
    if (left_idx, left) <= (right_idx, right):
        return f"{left}/{right}"
    return f"{right}/{left}"
