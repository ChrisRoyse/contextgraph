from __future__ import annotations

import json
import math
from collections import Counter, defaultdict
from itertools import combinations
from pathlib import Path
from typing import Any

from .core import SwebenchOpsError, finite_float, sha256_file, utc_now, write_json_checked
from .python_pair_feature_audit import read_json_object, write_jsonl
from .python_pair_surface_mining import (
    CALIBRATION_KEY,
    DEFAULT_CODE_STATES_JSONL,
    DEFAULT_LABEL_MANIFEST,
    DEFAULT_ROW_DIAGNOSTICS_REPORT,
    HOLDOUT_KEY,
    attach_pair_features,
    effect_for_feature,
    grouped_by_cell,
    load_code_states,
    load_diagnostic_rows,
    pair_id_from_feature,
    score_bin_sign_stability,
)
from .python_per_cell_smooth_ece import metric_block, power_blockers, reject_mounted_drive


DEFAULT_OUTPUT_ROOT = Path("/zfs/archive/contextgraph/readback/task-py-g-124b-pair-interaction-mining-readback")
WEAK_CELLS = ("delete_test_call::python", "off_by_one::python", "subtle_flip::python")


def add_arguments(parser: Any) -> None:
    parser.add_argument("--code-states-jsonl", type=Path, default=DEFAULT_CODE_STATES_JSONL)
    parser.add_argument("--label-manifest", type=Path, default=DEFAULT_LABEL_MANIFEST)
    parser.add_argument("--row-diagnostics-report", type=Path, default=DEFAULT_ROW_DIAGNOSTICS_REPORT)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--evidence-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--expected-code-state-count", type=int, default=3000)
    parser.add_argument("--expected-calibration-count", type=int, default=240)
    parser.add_argument("--expected-holdout-count", type=int, default=240)
    parser.add_argument("--min-calibration-support", type=int, default=5)
    parser.add_argument("--max-feature-fraction", type=float, default=0.90)
    parser.add_argument("--max-atomic-features", type=int, default=60)
    parser.add_argument("--top-candidate-count", type=int, default=80)
    parser.add_argument("--min-synergy", type=float, default=0.03)


def command_python_pair_interaction_mining(args: Any) -> dict[str, Any]:
    code_states_jsonl = args.code_states_jsonl.expanduser().resolve(strict=False)
    label_manifest = args.label_manifest.expanduser().resolve(strict=False)
    row_diagnostics_report = args.row_diagnostics_report.expanduser().resolve(strict=False)
    output_root = args.output_root.expanduser().resolve(strict=False)
    for name, path in (
        ("code_states_jsonl", code_states_jsonl),
        ("label_manifest", label_manifest),
        ("row_diagnostics_report", row_diagnostics_report),
        ("output_root", output_root),
    ):
        reject_mounted_drive(name, path)
    if args.min_calibration_support <= 0 or args.max_atomic_features <= 1 or args.top_candidate_count <= 0:
        raise SwebenchOpsError(
            "PY_G_124B_PAIR_INTERACTION_INVALID_CONFIG",
            "support, atomic-feature, and candidate limits must be positive",
            "Use positive pair interaction mining limits.",
            {
                "min_calibration_support": args.min_calibration_support,
                "max_atomic_features": args.max_atomic_features,
                "top_candidate_count": args.top_candidate_count,
            },
        )
    if not (0.0 < float(args.max_feature_fraction) <= 1.0):
        raise SwebenchOpsError(
            "PY_G_124B_PAIR_INTERACTION_INVALID_FRACTION",
            "max-feature-fraction must be in (0, 1]",
            "Use a valid fraction cap.",
            {"max_feature_fraction": args.max_feature_fraction},
        )

    output_root.mkdir(parents=True, exist_ok=True)
    label_manifest_payload = read_json_object(label_manifest, "label_manifest")
    states = load_code_states(code_states_jsonl, expected=int(args.expected_code_state_count))
    diagnostics = read_json_object(row_diagnostics_report, "row_diagnostics_report")
    calibration = attach_pair_features(
        load_diagnostic_rows(diagnostics, CALIBRATION_KEY, expected=int(args.expected_calibration_count)),
        states,
    )
    holdout = attach_pair_features(
        load_diagnostic_rows(diagnostics, HOLDOUT_KEY, expected=int(args.expected_holdout_count)),
        states,
    )

    atomic_pool = select_atomic_features(
        calibration,
        min_support=int(args.min_calibration_support),
        max_fraction=float(args.max_feature_fraction),
        max_features=int(args.max_atomic_features),
    )
    interaction_rows = mine_interactions(
        calibration,
        holdout,
        atomic_pool,
        min_support=int(args.min_calibration_support),
        min_synergy=float(args.min_synergy),
        top_count=int(args.top_candidate_count),
    )
    top_interactions = interaction_rows[: int(args.top_candidate_count)]
    weak_cells = weak_cell_interactions(interaction_rows)
    motif_summary = interaction_motif_summary(interaction_rows)
    baseline = {
        "calibration": split_report(calibration, "active_after_abstain"),
        "holdout": split_report(holdout, "active_after_abstain"),
    }

    atomic_path = output_root / "pair_interaction_atomic_pool.jsonl"
    candidates_path = output_root / "pair_interaction_candidates.jsonl"
    top_path = output_root / "pair_interaction_top_candidates.jsonl"
    sample_path = output_root / "pair_interaction_row_samples.jsonl"
    report_path = output_root / "pair_interaction_mining_report.json"
    write_jsonl(atomic_path, atomic_pool)
    write_jsonl(candidates_path, interaction_rows)
    write_jsonl(top_path, top_interactions)
    write_jsonl(sample_path, row_samples(calibration, holdout, top_interactions[:12]))

    report = {
        "artifactKind": "task_py_g_124b_pair_interaction_mining",
        "checkedAt": utc_now(),
        "githubIssue": 583,
        "ok": True,
        "inputArtifacts": {
            "codeStatesJsonl": str(code_states_jsonl),
            "codeStatesJsonlSha256": sha256_file(code_states_jsonl),
            "labelManifest": str(label_manifest),
            "labelManifestSha256": sha256_file(label_manifest),
            "rowDiagnosticsReport": str(row_diagnostics_report),
            "rowDiagnosticsReportSha256": sha256_file(row_diagnostics_report),
        },
        "labelManifestLineCounts": label_manifest_payload.get("line_counts"),
        "baseline": baseline,
        "selectionPolicy": {
            "selectionSplit": "calibration_only",
            "holdoutUse": "reporting_only",
            "minCalibrationSupport": int(args.min_calibration_support),
            "maxFeatureFraction": float(args.max_feature_fraction),
            "maxAtomicFeatures": int(args.max_atomic_features),
            "topCandidateCount": int(args.top_candidate_count),
            "minSynergy": float(args.min_synergy),
            "interactionKind": "logical_and_over_named_pair_features",
        },
        "atomicPoolSummary": {
            "count": len(atomic_pool),
            "topFeatures": atomic_pool[:20],
        },
        "candidateSummary": {
            "candidateCount": len(interaction_rows),
            "topCandidateCount": len(top_interactions),
            "nonRejectedTopCandidateCount": sum(1 for row in top_interactions if not row["rejectionReasons"]),
            "topCandidates": top_interactions[:20],
        },
        "weakCellSummary": weak_cells,
        "motifSummary": motif_summary,
        "pathPolicy": {"aiwonderOnly": True, "mntDActiveReadsOrWrites": False},
        "policy": {
            "noNewEmbedders": True,
            "noFlatVectorShortcut": True,
            "preservesSlotIdentity": True,
            "noHoldoutTuning": True,
            "topKExpansionRejected": True,
            "noPredictorWiring": True,
        },
        "outputArtifacts": {
            "pairInteractionAtomicPoolJsonl": str(atomic_path),
            "pairInteractionAtomicPoolSha256": sha256_file(atomic_path),
            "pairInteractionCandidatesJsonl": str(candidates_path),
            "pairInteractionCandidatesSha256": sha256_file(candidates_path),
            "pairInteractionTopCandidatesJsonl": str(top_path),
            "pairInteractionTopCandidatesSha256": sha256_file(top_path),
            "pairInteractionRowSamplesJsonl": str(sample_path),
            "pairInteractionRowSamplesSha256": sha256_file(sample_path),
        },
        "nextRecommendation": recommendation(top_interactions),
        "reportPath": str(report_path),
    }
    write_json_checked(report_path, report)
    report["reportSha256"] = sha256_file(report_path)
    return report


def select_atomic_features(
    rows: list[dict[str, Any]],
    *,
    min_support: int,
    max_fraction: float,
    max_features: int,
) -> list[dict[str, Any]]:
    selected = []
    all_features = sorted({feature for row in rows for feature in row["pair_features"]})
    for feature in all_features:
        effect = effect_for_feature(rows, feature)
        support = int(effect["support"])
        fraction = float(effect["nonzeroFraction"] or 0.0)
        if support < min_support or fraction <= 0.0 or fraction > max_fraction:
            continue
        score = atomic_selector_score(effect)
        if score <= 0.0:
            continue
        selected.append(
            {
                "feature": feature,
                "pairId": pair_id_from_feature(feature),
                "featureKind": feature.split("::", 1)[0],
                "motif": motif(feature),
                "support": support,
                "nonzeroFraction": finite_float(fraction),
                "selectorScore": finite_float(score),
                "calibrationEffect": effect,
            }
        )
    selected.sort(key=lambda row: (-float(row["selectorScore"]), row["feature"]))
    return selected[:max_features]


def atomic_selector_score(effect: dict[str, Any]) -> float:
    score = 0.0
    for key, weight in (
        ("brierCorrelation", 1.25),
        ("absErrorCorrelation", 1.0),
        ("activeCorrelation", 0.75),
        ("labelCorrelation", 0.50),
    ):
        value = effect.get(key)
        if value is not None:
            score += weight * abs(float(value))
    return score


def mine_interactions(
    calibration: list[dict[str, Any]],
    holdout: list[dict[str, Any]],
    atomic_pool: list[dict[str, Any]],
    *,
    min_support: int,
    min_synergy: float,
    top_count: int,
) -> list[dict[str, Any]]:
    atomic_by_feature = {row["feature"]: row for row in atomic_pool}
    candidates = []
    for left, right in combinations(atomic_pool, 2):
        left_feature = left["feature"]
        right_feature = right["feature"]
        interaction = interaction_key(left_feature, right_feature)
        cal_rows = rows_with_interaction(calibration, left_feature, right_feature, interaction)
        cal_effect = effect_for_feature(cal_rows, interaction)
        if int(cal_effect["support"]) < min_support:
            continue
        hold_rows = rows_with_interaction(holdout, left_feature, right_feature, interaction)
        hold_effect = effect_for_feature(hold_rows, interaction)
        sign_stability = score_bin_sign_stability(cal_rows, interaction)
        per_cell = per_cell_interactions(calibration, holdout, interaction, left_feature, right_feature, min_support)
        synergy = synergy_metrics(cal_effect, atomic_by_feature[left_feature]["calibrationEffect"], atomic_by_feature[right_feature]["calibrationEffect"])
        rejection = rejection_reasons(cal_effect, synergy, sign_stability, min_synergy)
        candidates.append(
            {
                "interaction": interaction,
                "leftFeature": left_feature,
                "rightFeature": right_feature,
                "leftPairId": left["pairId"],
                "rightPairId": right["pairId"],
                "pairGroup": pair_group(left["pairId"], right["pairId"]),
                "motif": interaction_motif(left_feature, right_feature),
                "calibration": cal_effect,
                "holdoutReportingOnly": hold_effect,
                "synergyVsAtomicCalibration": synergy,
                "perCell": per_cell,
                "scoreBinSignStability": sign_stability,
                "selectorScore": selector_score(cal_effect, synergy, per_cell, sign_stability, rejection),
                "rejectionReasons": rejection,
            }
        )
    candidates.sort(key=lambda row: (-float(row["selectorScore"]), row["interaction"]))
    return candidates[: max(top_count * 5, top_count)]


def rows_with_interaction(
    rows: list[dict[str, Any]],
    left_feature: str,
    right_feature: str,
    interaction: str,
) -> list[dict[str, Any]]:
    out = []
    for row in rows:
        features = row["pair_features"]
        if left_feature in features and right_feature in features:
            extended = sorted([*features, interaction])
        else:
            extended = features
        out.append({**row, "pair_features": extended})
    return out


def per_cell_interactions(
    calibration: list[dict[str, Any]],
    holdout: list[dict[str, Any]],
    interaction: str,
    left_feature: str,
    right_feature: str,
    min_support: int,
) -> dict[str, Any]:
    out = {}
    cal_grouped = grouped_by_cell(calibration)
    hold_grouped = grouped_by_cell(holdout)
    for cell, cal_rows in cal_grouped.items():
        cal_interaction_rows = rows_with_interaction(cal_rows, left_feature, right_feature, interaction)
        cal_effect = effect_for_feature(cal_interaction_rows, interaction)
        if int(cal_effect["support"]) < min_support:
            continue
        hold_rows = hold_grouped.get(cell, [])
        hold_effect = None
        if hold_rows:
            hold_effect = effect_for_feature(rows_with_interaction(hold_rows, left_feature, right_feature, interaction), interaction)
        out[cell] = {"calibration": cal_effect, "holdoutReportingOnly": hold_effect}
    return out


def synergy_metrics(interaction: dict[str, Any], left: dict[str, Any], right: dict[str, Any]) -> dict[str, Any]:
    out = {}
    for key in ("brierCorrelation", "absErrorCorrelation", "activeCorrelation", "labelCorrelation"):
        out[f"{key}Synergy"] = synergy_value(interaction.get(key), left.get(key), right.get(key))
    return out


def synergy_value(interaction: Any, left: Any, right: Any) -> float | None:
    if interaction is None:
        return None
    left_abs = abs(float(left)) if left is not None else 0.0
    right_abs = abs(float(right)) if right is not None else 0.0
    return finite_float(abs(float(interaction)) - max(left_abs, right_abs))


def rejection_reasons(
    effect: dict[str, Any],
    synergy: dict[str, Any],
    sign_stability: dict[str, Any],
    min_synergy: float,
) -> list[str]:
    reasons = []
    fraction = effect.get("nonzeroFraction")
    if fraction is not None and float(fraction) > 0.90:
        reasons.append("GENERIC_HIGH_SUPPORT_GT_90PCT")
    if fraction is not None and float(fraction) < 0.02:
        reasons.append("TOO_RARE_LT_2PCT")
    best_synergy = max((float(value) for value in synergy.values() if value is not None), default=0.0)
    if best_synergy < min_synergy:
        reasons.append("NO_ATOMIC_SYNERGY")
    if sign_stability["observedBins"] >= 2 and not sign_stability["stable"]:
        reasons.append("SCORE_BIN_SIGN_UNSTABLE")
    return reasons


def selector_score(
    effect: dict[str, Any],
    synergy: dict[str, Any],
    per_cell: dict[str, Any],
    sign_stability: dict[str, Any],
    rejection: list[str],
) -> float:
    score = atomic_selector_score(effect)
    for value in synergy.values():
        if value is not None and float(value) > 0.0:
            score += 2.0 * float(value)
    for cell in WEAK_CELLS:
        cell_effect = per_cell.get(cell, {}).get("calibration")
        if cell_effect and cell_effect.get("brierCorrelation") is not None:
            score += 1.25 * abs(float(cell_effect["brierCorrelation"]))
    if sign_stability["stable"]:
        score *= 1.1
    if rejection:
        score *= 0.25
    return finite_float(score)


def split_report(rows: list[dict[str, Any]], active_key: str) -> dict[str, Any]:
    active = [row for row in rows if row[active_key] is True]
    abstain = [row for row in rows if row[active_key] is False]
    return {
        "full": metrics(rows),
        "activePolicy": metrics(active),
        "abstainPool": metrics(abstain),
        "rowCounts": {"full": len(rows), "activePolicy": len(active), "abstainPool": len(abstain)},
        "activeCoverage": finite_float(len(active) / len(rows)) if rows else None,
        "perCellActive": per_cell_metrics(active),
        "perCellAbstain": per_cell_metrics(abstain),
    }


def metrics(rows: list[dict[str, Any]]) -> dict[str, Any]:
    block = metric_block([row["actual"] for row in rows], [row["predicted_oracle_pass"] for row in rows])
    block["powerBlockers"] = power_blockers(
        [row["actual"] for row in rows],
        [row["predicted_oracle_pass"] for row in rows],
    )
    return block


def per_cell_metrics(rows: list[dict[str, Any]]) -> dict[str, Any]:
    out = {}
    for cell, cell_rows in grouped_by_cell(rows).items():
        out[cell] = metrics(cell_rows)
    return dict(sorted(out.items()))


def weak_cell_interactions(candidates: list[dict[str, Any]]) -> dict[str, Any]:
    out = {}
    for cell in WEAK_CELLS:
        rows = [
            {
                "interaction": row["interaction"],
                "leftFeature": row["leftFeature"],
                "rightFeature": row["rightFeature"],
                "pairGroup": row["pairGroup"],
                "motif": row["motif"],
                "selectorScore": row["selectorScore"],
                "cellCalibration": row["perCell"][cell]["calibration"],
                "cellHoldoutReportingOnly": row["perCell"][cell]["holdoutReportingOnly"],
                "synergyVsAtomicCalibration": row["synergyVsAtomicCalibration"],
                "rejectionReasons": row["rejectionReasons"],
            }
            for row in candidates
            if cell in row["perCell"]
        ]
        rows.sort(key=lambda row: (-float(row["selectorScore"]), row["interaction"]))
        out[cell] = rows[:12]
    return out


def interaction_motif_summary(candidates: list[dict[str, Any]]) -> dict[str, Any]:
    grouped: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for row in candidates:
        grouped[row["motif"]].append(row)
    out = {}
    for motif_name, rows in grouped.items():
        rows.sort(key=lambda row: (-float(row["selectorScore"]), row["interaction"]))
        out[motif_name] = {
            "candidateCount": len(rows),
            "nonRejectedCount": sum(1 for row in rows if not row["rejectionReasons"]),
            "topSelectorScore": rows[0]["selectorScore"],
            "topInteraction": rows[0]["interaction"],
            "topPairGroup": rows[0]["pairGroup"],
        }
    return dict(sorted(out.items(), key=lambda item: (-float(item[1]["topSelectorScore"]), item[0]))[:30])


def row_samples(
    calibration: list[dict[str, Any]],
    holdout: list[dict[str, Any]],
    interactions: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    rows = []
    for candidate in interactions:
        left = candidate["leftFeature"]
        right = candidate["rightFeature"]
        for split, source in (("calibration", calibration), ("holdout", holdout)):
            matching = [row for row in source if left in row["pair_features"] and right in row["pair_features"]]
            for row in matching[:2]:
                rows.append(
                    {
                        "split": split,
                        "interaction": candidate["interaction"],
                        "task_id": row["task_id"],
                        "category": row["category"],
                        "actual": row["actual"],
                        "predicted_oracle_pass": row["predicted_oracle_pass"],
                        "active_after_abstain": row["active_after_abstain"],
                        "leftFeaturePresent": left in row["pair_features"],
                        "rightFeaturePresent": right in row["pair_features"],
                    }
                )
    return rows[:80]


def recommendation(top_interactions: list[dict[str, Any]]) -> dict[str, Any]:
    usable = [row for row in top_interactions if not row["rejectionReasons"]]
    if not usable:
        return {
            "recommendedAction": "retain_negative_no_pair_interaction_candidates",
            "reason": "no interaction candidate beat atomic pair features under calibration-only synergy/stability filters",
            "candidateFeatureCountWithoutRejection": 0,
            "topUsableCandidate": None,
        }
    best = usable[0]
    if any(cell in best["perCell"] for cell in WEAK_CELLS):
        action = "test_explicit_named_pair_interaction_evidence_in_392_cause_residual_path"
    else:
        action = "retain_interaction_as_diagnostic_before_predictor_wiring"
    return {
        "recommendedAction": action,
        "candidateFeatureCountWithoutRejection": len(usable),
        "topUsableCandidate": best,
    }


def interaction_key(left: str, right: str) -> str:
    ordered = sorted([left, right])
    return "interaction::" + json.dumps(ordered, separators=(",", ":"))


def motif(feature: str) -> str:
    pair_id = pair_id_from_feature(feature)
    if "/" not in pair_id:
        return "unknown"
    left, right = pair_id.split("/", 1)
    return f"{slot_family(left)}/{slot_family(right)}"


def interaction_motif(left_feature: str, right_feature: str) -> str:
    return f"{motif(left_feature)}+{motif(right_feature)}"


def pair_group(left_pair: str, right_pair: str) -> str:
    if left_pair == right_pair:
        return "same_pair_multi_feature"
    left_slots = set(left_pair.split("/")) if "/" in left_pair else set()
    right_slots = set(right_pair.split("/")) if "/" in right_pair else set()
    shared = left_slots & right_slots
    if shared:
        return "shared_slot:" + ",".join(sorted(shared))
    if left_slots and right_slots:
        return "disjoint_pair"
    return "unknown_pair_group"


def slot_family(slot: str) -> str:
    if slot in {"e2", "e3", "e4"}:
        return "temporal"
    if slot in {"e6", "e13"}:
        return "sparse_lexical"
    if slot in {"e1", "e7", "e8", "e10", "e12", "e14"}:
        return "semantic_content"
    if slot == "e9":
        return "hdc"
    if slot in {"e5", "e11"}:
        return "retired_or_disabled"
    return "other_or_inactive"
