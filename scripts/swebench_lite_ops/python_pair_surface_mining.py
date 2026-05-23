from __future__ import annotations

import json
import math
from collections import Counter, defaultdict
from itertools import combinations
from pathlib import Path
from typing import Any

from .core import SwebenchOpsError, finite_float, sha256_file, utc_now, write_json_checked
from .python_pair_feature_audit import (
    read_json_object,
    target_workspace_state,
    write_jsonl,
)
from .python_per_cell_smooth_ece import metric_block, reject_mounted_drive


DEFAULT_CODE_STATES_JSONL = Path(
    "/zfs/archive/contextgraph/state/labels/task-py-g-117-auto-label-discovery-v2-live-split/"
    "code_state_constellations.jsonl"
)
DEFAULT_LABEL_MANIFEST = Path(
    "/zfs/archive/contextgraph/state/labels/task-py-g-117-auto-label-discovery-v2-live-split/"
    "auto_label_discovery_manifest.json"
)
DEFAULT_ROW_DIAGNOSTICS_REPORT = Path(
    "/zfs/archive/contextgraph/readback/task-py-g-392-temporal-trajectory-features-readback/"
    "run-20260521T171548Z/task_py_g_392_temporal_trajectory_features_readback.json"
)
DEFAULT_PAIR_AUDIT_REPORT = Path(
    "/zfs/archive/contextgraph/readback/task-py-g-124-cross-embedder-pair-features-readback/"
    "run-20260522T010513Z/pair_feature_audit_report.json"
)
DEFAULT_GEOMETRY_LABEL_SPACE_POLICY = Path(
    "/zfs/archive/contextgraph/state/labels/task-py-g-143-geometry-label-admissibility-v1/"
    "geometry_label_space_policy.json"
)
DEFAULT_OUTPUT_ROOT = Path("/zfs/archive/contextgraph/readback/task-py-g-124b-105-pair-mining-readback")

SCALAR_FEATURES = ("norm_l2", "std", "abs_mean", "entropy_proxy", "zero_fraction")
EXPECTED_DOCUMENTED_SLOTS = tuple(f"e{idx}" for idx in range(1, 16))
CALIBRATION_KEY = "score_sharpness_abstain_calibration_row_diagnostics"
HOLDOUT_KEY = "score_sharpness_abstain_holdout_row_diagnostics"


def add_arguments(parser: Any) -> None:
    parser.add_argument("--code-states-jsonl", type=Path, default=DEFAULT_CODE_STATES_JSONL)
    parser.add_argument("--label-manifest", type=Path, default=DEFAULT_LABEL_MANIFEST)
    parser.add_argument("--row-diagnostics-report", type=Path, default=DEFAULT_ROW_DIAGNOSTICS_REPORT)
    parser.add_argument("--pair-audit-report", type=Path, default=DEFAULT_PAIR_AUDIT_REPORT)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--evidence-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--expected-code-state-count", type=int, default=3000)
    parser.add_argument("--expected-calibration-count", type=int, default=240)
    parser.add_argument("--expected-holdout-count", type=int, default=240)
    parser.add_argument("--min-calibration-support", type=int, default=5)
    parser.add_argument("--top-candidate-count", type=int, default=80)
    parser.add_argument("--expected-pair-count", type=int, default=105)


def command_python_pair_surface_mining(args: Any) -> dict[str, Any]:
    code_states_jsonl = args.code_states_jsonl.expanduser().resolve(strict=False)
    label_manifest = args.label_manifest.expanduser().resolve(strict=False)
    row_diagnostics_report = args.row_diagnostics_report.expanduser().resolve(strict=False)
    pair_audit_report = args.pair_audit_report.expanduser().resolve(strict=False)
    output_root = args.output_root.expanduser().resolve(strict=False)
    for name, path in (
        ("code_states_jsonl", code_states_jsonl),
        ("label_manifest", label_manifest),
        ("row_diagnostics_report", row_diagnostics_report),
        ("pair_audit_report", pair_audit_report),
        ("output_root", output_root),
    ):
        reject_mounted_drive(name, path)
    if args.min_calibration_support <= 0 or args.top_candidate_count <= 0:
        raise SwebenchOpsError(
            "PY_G_124B_INVALID_MINING_CONFIG",
            "support and top-candidate limits must be positive",
            "Use positive mining configuration values.",
            {"min_calibration_support": args.min_calibration_support, "top_candidate_count": args.top_candidate_count},
        )

    output_root.mkdir(parents=True, exist_ok=True)
    label_manifest_payload = read_json_object(label_manifest, "label_manifest")
    pair_audit_payload = read_json_object(pair_audit_report, "pair_audit_report")
    states = load_code_states(code_states_jsonl, expected=int(args.expected_code_state_count))
    diagnostics = read_json_object(row_diagnostics_report, "row_diagnostics_report")
    calibration = load_diagnostic_rows(diagnostics, CALIBRATION_KEY, expected=int(args.expected_calibration_count))
    holdout = load_diagnostic_rows(diagnostics, HOLDOUT_KEY, expected=int(args.expected_holdout_count))

    universe_rows, universe_summary = classify_pair_universe(states, pair_audit_payload, int(args.expected_pair_count))
    calibration_rows = attach_pair_features(calibration, states)
    holdout_rows = attach_pair_features(holdout, states)
    candidate_rows = mine_candidates(
        calibration_rows,
        holdout_rows,
        min_support=int(args.min_calibration_support),
    )
    top_candidates = candidate_rows[: int(args.top_candidate_count)]

    weak_cells = weak_cell_summary(candidate_rows)
    motif_summary = motif_effect_summary(candidate_rows)
    baseline = {
        "calibration": split_report(calibration_rows, "predicted_oracle_pass"),
        "holdout": split_report(holdout_rows, "predicted_oracle_pass"),
    }

    universe_path = output_root / "pair_universe_classification.jsonl"
    candidates_path = output_root / "pair_candidate_effects.jsonl"
    top_path = output_root / "pair_top_candidates.jsonl"
    row_sample_path = output_root / "pair_row_feature_samples.jsonl"
    report_path = output_root / "pair_surface_mining_report.json"
    write_jsonl(universe_path, universe_rows)
    write_jsonl(candidates_path, candidate_rows)
    write_jsonl(top_path, top_candidates)
    write_jsonl(row_sample_path, row_feature_samples(calibration_rows, holdout_rows, limit=40))

    report = {
        "artifactKind": "task_py_g_124b_105_pair_surface_mining",
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
            "pairAuditReport": str(pair_audit_report),
            "pairAuditReportSha256": sha256_file(pair_audit_report),
        },
        "labelManifestLineCounts": label_manifest_payload.get("line_counts"),
        "pairUniverse": universe_summary,
        "baseline": baseline,
        "candidateSummary": {
            "candidateCount": len(candidate_rows),
            "topCandidateCount": len(top_candidates),
            "minCalibrationSupport": int(args.min_calibration_support),
            "selectionSplit": "calibration_only",
            "holdoutUse": "reporting_only",
            "topCandidates": top_candidates[:20],
        },
        "weakCellSummary": weak_cells,
        "motifSummary": motif_summary,
        "pathPolicy": {"aiwonderOnly": True, "mntDActiveReadsOrWrites": False},
        "policy": {
            "noNewEmbedders": True,
            "noFlatVectorShortcut": True,
            "preservesSlotIdentity": True,
            "topKExpansionRejected": True,
            "miningPurpose": "conditional_pair_meaning_before_any_gate_scorer_wiring",
        },
        "outputArtifacts": {
            "pairUniverseClassificationJsonl": str(universe_path),
            "pairUniverseClassificationSha256": sha256_file(universe_path),
            "pairCandidateEffectsJsonl": str(candidates_path),
            "pairCandidateEffectsSha256": sha256_file(candidates_path),
            "pairTopCandidatesJsonl": str(top_path),
            "pairTopCandidatesSha256": sha256_file(top_path),
            "pairRowFeatureSamplesJsonl": str(row_sample_path),
            "pairRowFeatureSamplesSha256": sha256_file(row_sample_path),
        },
        "nextRecommendation": recommendation(universe_summary, top_candidates),
        "reportPath": str(report_path),
    }
    write_json_checked(report_path, report)
    report["reportSha256"] = sha256_file(report_path)
    return report


def load_code_states(path: Path, *, expected: int) -> dict[tuple[str, str], dict[str, Any]]:
    if not path.is_file():
        raise SwebenchOpsError(
            "PY_G_124B_CODE_STATES_MISSING",
            f"code-state constellations missing: {path}",
            "Use the current aiwonder label root.",
            {"path": str(path)},
        )
    out: dict[tuple[str, str], dict[str, Any]] = {}
    with path.open("r", encoding="utf-8") as handle:
        for line_number, line in enumerate(handle, 1):
            if not line.strip():
                continue
            row = json.loads(line)
            task_id = required_string(row, "task_instance_id")
            workspace_state = required_string(row, "workspace_state")
            key = (task_id, workspace_state)
            if key in out:
                raise SwebenchOpsError(
                    "PY_G_124B_DUPLICATE_CODE_STATE",
                    "duplicate code-state row identity",
                    "Regenerate the label root with unique task/workspace rows.",
                    {"line": line_number, "task_id": task_id, "workspace_state": workspace_state},
                )
            out[key] = row
    if len(out) != expected:
        raise SwebenchOpsError(
            "PY_G_124B_CODE_STATE_COUNT_MISMATCH",
            "code-state constellation count does not match expected rows",
            "Use the current aiwonder label root or update the expected count after proving the corpus changed.",
            {"expected": expected, "actual": len(out)},
        )
    return out


def load_diagnostic_rows(payload: dict[str, Any], key: str, *, expected: int) -> list[dict[str, Any]]:
    raw = payload.get(key)
    if not isinstance(raw, list):
        raise SwebenchOpsError(
            "PY_G_124B_DIAGNOSTIC_ROWS_MISSING",
            f"{key} missing or not an array",
            "Use the retained #392 active/abstain row diagnostics artifact.",
            {"key": key},
        )
    if len(raw) != expected:
        raise SwebenchOpsError(
            "PY_G_124B_DIAGNOSTIC_ROW_COUNT_MISMATCH",
            f"{key} row count mismatch",
            "Use the current #392 report or update the expected count after proving the split changed.",
            {"key": key, "expected": expected, "actual": len(raw)},
        )
    return [normalize_diagnostic_row(row, key, index) for index, row in enumerate(raw)]


def normalize_diagnostic_row(row: Any, key: str, index: int) -> dict[str, Any]:
    if not isinstance(row, dict):
        raise SwebenchOpsError(
            "PY_G_124B_DIAGNOSTIC_ROW_OBJECT_REQUIRED",
            f"{key}[{index}] must be an object",
            "Fix the source row diagnostics before mining pairs.",
            {"key": key, "index": index, "actual": type(row).__name__},
        )
    active = row.get("active_after_abstain")
    if not isinstance(active, bool):
        raise SwebenchOpsError(
            "PY_G_124B_ACTIVE_FLAG_REQUIRED",
            f"{key}[{index}] active_after_abstain must be boolean",
            "Use score_sharpness_abstain row diagnostics.",
            {"key": key, "index": index, "actual": active},
        )
    score = clamp_prob(required_float(row, "predicted_oracle_pass"))
    actual = 1 if required_bool(row, "actual_oracle_pass") else 0
    return {
        "row_index": int(row.get("row_index", index)),
        "task_id": required_string(row, "task_id"),
        "category": required_string(row, "category"),
        "patch_path": required_string(row, "patch_path"),
        "actual": actual,
        "predicted_oracle_pass": score,
        "active_after_abstain": active,
        "absolute_error": abs(score - actual),
        "brier_residual": (score - actual) ** 2,
        "score_bin": score_bin(score),
    }


def classify_pair_universe(
    states: dict[tuple[str, str], dict[str, Any]],
    audit: dict[str, Any],
    expected_pair_count: int,
    space_policy: dict[str, Any] | None = None,
) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    active_slots = sorted(
        {
            slot
            for row in states.values()
            for slot in slot_features(row).keys()
        }
    )
    documented_slots = sorted(EXPECTED_DOCUMENTED_SLOTS)
    documented_pairs = [pair_id(left, right) for left, right in combinations(documented_slots, 2)]
    active_pairs = {pair_id(left, right) for left, right in combinations(active_slots, 2)}
    valid_independent_slots: list[str] = []
    valid_independent_pairs: set[str] = set()
    degenerate_slots: set[str] = set()
    duplicate_alias_map: dict[str, str] = {}
    if space_policy is not None:
        valid_independent_slots = sorted(space_policy["eligible_independent_embedder_ids"])
        degenerate_slots = set(space_policy["degenerate_embedder_ids"])
        duplicate_alias_map = dict(space_policy["duplicate_alias_map"])
        valid_independent_pairs = {
            pair_id(left, right) for left, right in combinations(valid_independent_slots, 2)
        }
        expected_valid_pair_count = int(space_policy["valid_independent_pair_denominator"])
        if len(valid_independent_pairs) != expected_valid_pair_count:
            raise SwebenchOpsError(
                "PY_G_124B_VALID_PAIR_DENOMINATOR_MISMATCH",
                "valid-space policy denominator does not match eligible independent embedders",
                "Regenerate or repair the geometry label space policy before pair mining.",
                {
                    "expected": expected_valid_pair_count,
                    "actual": len(valid_independent_pairs),
                    "eligible_independent_embedder_ids": valid_independent_slots,
                },
            )
    top_pair_counts = Counter()
    for row in states.values():
        for label, count in pair_label_counts(row).items():
            top_pair_counts[pair_id_from_label(label)] += int(count)
    audit_consumed = audit.get("currentConsumption", {}).get("headProjection", {})
    rows = []
    for pair in documented_pairs:
        left, right = pair.split("/", 1)
        status = []
        if pair in active_pairs:
            status.append("derivable_from_slot_feature_means")
        else:
            status.append("inactive_missing_current_label_root")
        if pair in top_pair_counts:
            status.append("materialized_in_code_state_top_pair_counts")
        elif pair in active_pairs:
            status.append("dark_not_in_top_pair_counts")
        valid_policy_status = "valid_space_policy_not_supplied"
        valid_independent = False
        alias_slots = sorted(slot for slot in (left, right) if slot in duplicate_alias_map)
        degenerate_pair_slots = sorted(slot for slot in (left, right) if slot in degenerate_slots)
        if space_policy is not None:
            if pair in valid_independent_pairs:
                valid_policy_status = "valid_independent_forward_cache_geometry_pair"
                valid_independent = True
                status.append(valid_policy_status)
            elif degenerate_pair_slots:
                valid_policy_status = "invalid_degenerate_embedder_space"
                status.append(valid_policy_status)
            elif alias_slots:
                valid_policy_status = "invalid_duplicate_alias_embedder_space"
                status.append(valid_policy_status)
            else:
                valid_policy_status = "not_in_valid_independent_forward_cache_policy"
                status.append(valid_policy_status)
        rows.append(
            {
                "pairId": pair,
                "leftSlot": left,
                "rightSlot": right,
                "status": status,
                "codeStateTopCount": int(top_pair_counts.get(pair, 0)),
                "slotFamilyLeft": slot_family(left),
                "slotFamilyRight": slot_family(right),
                "validIndependentForwardCacheGeometry": valid_independent,
                "validSpacePolicyStatus": valid_policy_status,
                "degenerateSlotsInPair": degenerate_pair_slots,
                "aliasSlotsInPair": alias_slots,
            }
        )
    status_counts = Counter(status for row in rows for status in row["status"])
    if len(documented_pairs) != expected_pair_count:
        raise SwebenchOpsError(
            "PY_G_124B_EXPECTED_PAIR_COUNT_MISMATCH",
            "documented expected slot universe does not produce the requested pair count",
            "Fix expected documented slots or expected-pair-count.",
            {"expected": expected_pair_count, "actual": len(documented_pairs), "slots": documented_slots},
        )
    return rows, {
        "documentedSlotCount": len(documented_slots),
        "documentedExpectedPairCount": len(documented_pairs),
        "activeSlotCountInLabelRoot": len(active_slots),
        "activeSlotsInLabelRoot": active_slots,
        "activeDerivablePairCount": len(active_pairs),
        "validSpacePolicySupplied": space_policy is not None,
        "validIndependentSlotCount": len(valid_independent_slots) if space_policy is not None else None,
        "validIndependentSlots": valid_independent_slots if space_policy is not None else None,
        "validIndependentPairDenominator": len(valid_independent_pairs) if space_policy is not None else None,
        "degenerateEmbedderIds": sorted(degenerate_slots) if space_policy is not None else None,
        "duplicateAliasMap": duplicate_alias_map if space_policy is not None else None,
        "invalidDegeneratePairCount": sum(
            1 for row in rows if row["validSpacePolicyStatus"] == "invalid_degenerate_embedder_space"
        )
        if space_policy is not None
        else None,
        "invalidAliasPairCount": sum(
            1 for row in rows if row["validSpacePolicyStatus"] == "invalid_duplicate_alias_embedder_space"
        )
        if space_policy is not None
        else None,
        "materializedTopPairIdCount": len(top_pair_counts),
        "darkDerivablePairCount": sum(1 for row in rows if "dark_not_in_top_pair_counts" in row["status"]),
        "inactiveMissingPairCount": sum(1 for row in rows if "inactive_missing_current_label_root" in row["status"]),
        "statusCounts": dict(sorted(status_counts.items())),
        "auditUniquePairIds": audit.get("pairInventory", {}).get("uniquePairIds"),
        "auditUniquePairLabels": audit.get("pairInventory", {}).get("uniquePairLabels"),
        "explicitPairEvidenceKeysIn392Diagnostics": audit.get("currentConsumption", {})
        .get("explicitPairEvidenceKeysIn392Diagnostics", {})
        .get("uniqueExplicitPairEvidenceKeys"),
        "hardCodedHeadProjectionPairSpecCount": audit_consumed.get("consumedPairSpecCount"),
        "hardCodedHeadProjectionPairSpecs": audit_consumed.get("consumedPairSpecs"),
    }


def load_geometry_label_space_policy(path: Path) -> dict[str, Any]:
    payload = read_json_object(path, "geometry_label_space_policy")
    eligible = payload.get("eligible_independent_embedder_ids")
    degenerate = payload.get("degenerate_embedder_ids")
    aliases = payload.get("duplicate_alias_map")
    denominator = payload.get("valid_independent_pair_denominator")
    if not isinstance(eligible, list) or not all(isinstance(item, str) and item for item in eligible):
        raise SwebenchOpsError(
            "PY_G_124B_INVALID_GEOMETRY_SPACE_POLICY",
            "eligible independent embedder ids are missing or invalid",
            "Use the #143 geometry label space policy.",
            {"path": str(path), "eligible_independent_embedder_ids": eligible},
        )
    if not isinstance(degenerate, list) or not all(isinstance(item, str) and item for item in degenerate):
        raise SwebenchOpsError(
            "PY_G_124B_INVALID_GEOMETRY_SPACE_POLICY",
            "degenerate embedder ids are missing or invalid",
            "Use the #143 geometry label space policy.",
            {"path": str(path), "degenerate_embedder_ids": degenerate},
        )
    if not isinstance(aliases, dict) or not all(
        isinstance(key, str) and key and isinstance(value, str) and value for key, value in aliases.items()
    ):
        raise SwebenchOpsError(
            "PY_G_124B_INVALID_GEOMETRY_SPACE_POLICY",
            "duplicate alias map is missing or invalid",
            "Use the #143 geometry label space policy.",
            {"path": str(path), "duplicate_alias_map": aliases},
        )
    if isinstance(denominator, bool) or not isinstance(denominator, int) or denominator < 0:
        raise SwebenchOpsError(
            "PY_G_124B_INVALID_GEOMETRY_SPACE_POLICY",
            "valid independent pair denominator is missing or invalid",
            "Use the #143 geometry label space policy.",
            {"path": str(path), "valid_independent_pair_denominator": denominator},
        )
    normalized = {
        "path": str(path),
        "artifact_kind": payload.get("artifact_kind"),
        "formula_version": payload.get("formula_version"),
        "eligible_independent_embedder_ids": sorted(set(eligible)),
        "degenerate_embedder_ids": sorted(set(degenerate)),
        "duplicate_alias_map": {str(key): str(value) for key, value in sorted(aliases.items())},
        "valid_independent_pair_denominator": int(denominator),
    }
    expected = len(normalized["eligible_independent_embedder_ids"]) * (
        len(normalized["eligible_independent_embedder_ids"]) - 1
    ) // 2
    if expected != normalized["valid_independent_pair_denominator"]:
        raise SwebenchOpsError(
            "PY_G_124B_VALID_PAIR_DENOMINATOR_MISMATCH",
            "valid-space policy denominator does not match eligible independent embedders",
            "Regenerate or repair the geometry label space policy before pair mining.",
            {"expected": expected, "actual": normalized["valid_independent_pair_denominator"]},
        )
    return normalized


def attach_pair_features(rows: list[dict[str, Any]], states: dict[tuple[str, str], dict[str, Any]]) -> list[dict[str, Any]]:
    out = []
    for row in rows:
        task_id = row["task_id"]
        category = row["category"]
        base = require_state(states, task_id, "base")
        target = require_state(states, task_id, target_workspace_state(category))
        base_pairs = derived_pair_records(base)
        target_pairs = derived_pair_records(target)
        features = pair_feature_set(base_pairs, target_pairs)
        out.append({**row, "pair_features": sorted(features), "pair_feature_count": len(features)})
    return out


def pair_feature_set(
    base_pairs: dict[str, dict[str, Any]],
    target_pairs: dict[str, dict[str, Any]],
) -> set[str]:
    features = set()
    for pair in sorted(set(base_pairs) | set(target_pairs)):
        base = base_pairs.get(pair)
        target = target_pairs.get(pair)
        if target is not None:
            features.add(f"target::{target['label']}")
            features.add(f"target_state::{pair}:{target['state']}")
            features.add(f"target_driver::{pair}:{target['driver']}")
            features.add(f"target_family::{slot_family(target['left'])}/{slot_family(target['right'])}:{target['state']}")
        if base is not None and target is not None:
            transition = f"{base['state']}->{target['state']}"
            features.add(f"transition::{pair}:{transition}")
            if base["driver"] != target["driver"]:
                features.add(f"driver_transition::{pair}:{base['driver']}->{target['driver']}")
            delta_bucket = delta_shift_bucket(float(target["max_delta"]) - float(base["max_delta"]))
            features.add(f"magnitude_shift::{pair}:{delta_bucket}")
    return features


def mine_candidates(
    calibration: list[dict[str, Any]],
    holdout: list[dict[str, Any]],
    *,
    min_support: int,
) -> list[dict[str, Any]]:
    all_features = sorted({feature for row in calibration for feature in row["pair_features"]})
    candidates = []
    for feature in all_features:
        cal = effect_for_feature(calibration, feature)
        if cal["support"] < min_support:
            continue
        hold = effect_for_feature(holdout, feature)
        per_cell = per_cell_effects(calibration, holdout, feature, min_support=min_support)
        sign_stability = score_bin_sign_stability(calibration, feature)
        rejection = rejection_reasons(cal, sign_stability)
        candidates.append(
            {
                "feature": feature,
                "pairId": pair_id_from_feature(feature),
                "featureKind": feature.split("::", 1)[0],
                "motif": motif_from_feature(feature),
                "calibration": cal,
                "holdoutReportingOnly": hold,
                "perCell": per_cell,
                "scoreBinSignStability": sign_stability,
                "selectorScore": selector_score(cal, per_cell, sign_stability, rejection),
                "rejectionReasons": rejection,
            }
        )
    candidates.sort(key=lambda row: (-float(row["selectorScore"]), row["feature"]))
    return candidates


def effect_for_feature(rows: list[dict[str, Any]], feature: str) -> dict[str, Any]:
    present = [row for row in rows if feature in row["pair_features"]]
    absent = [row for row in rows if feature not in row["pair_features"]]
    labels = [row["actual"] for row in rows]
    indicator = [1 if feature in row["pair_features"] else 0 for row in rows]
    abs_errors = [row["absolute_error"] for row in rows]
    briers = [row["brier_residual"] for row in rows]
    active_flags = [1 if row["active_after_abstain"] else 0 for row in rows]
    return {
        "n": len(rows),
        "support": len(present),
        "nonzeroFraction": finite_float(len(present) / len(rows)) if rows else None,
        "labelRatePresent": mean([row["actual"] for row in present]),
        "labelRateAbsent": mean([row["actual"] for row in absent]),
        "labelRateDelta": none_delta(mean([row["actual"] for row in absent]), mean([row["actual"] for row in present])),
        "absErrorPresent": mean([row["absolute_error"] for row in present]),
        "absErrorAbsent": mean([row["absolute_error"] for row in absent]),
        "absErrorDeltaPresentMinusAbsent": none_delta(mean([row["absolute_error"] for row in absent]), mean([row["absolute_error"] for row in present])),
        "brierPresent": mean([row["brier_residual"] for row in present]),
        "brierAbsent": mean([row["brier_residual"] for row in absent]),
        "brierDeltaPresentMinusAbsent": none_delta(mean([row["brier_residual"] for row in absent]), mean([row["brier_residual"] for row in present])),
        "activeRatePresent": mean([1 if row["active_after_abstain"] else 0 for row in present]),
        "activeRateAbsent": mean([1 if row["active_after_abstain"] else 0 for row in absent]),
        "activeRateDelta": none_delta(mean([1 if row["active_after_abstain"] else 0 for row in absent]), mean([1 if row["active_after_abstain"] else 0 for row in present])),
        "labelCorrelation": point_biserial(indicator, labels),
        "absErrorCorrelation": point_biserial(indicator, abs_errors),
        "brierCorrelation": point_biserial(indicator, briers),
        "activeCorrelation": point_biserial(indicator, active_flags),
    }


def per_cell_effects(
    calibration: list[dict[str, Any]],
    holdout: list[dict[str, Any]],
    feature: str,
    *,
    min_support: int,
) -> dict[str, Any]:
    cells = {}
    for cell, cal_rows in grouped_by_cell(calibration).items():
        hold_rows = grouped_by_cell(holdout).get(cell, [])
        cal_effect = effect_for_feature(cal_rows, feature)
        if cal_effect["support"] < min_support:
            continue
        cells[cell] = {
            "calibration": cal_effect,
            "holdoutReportingOnly": effect_for_feature(hold_rows, feature) if hold_rows else None,
        }
    return cells


def score_bin_sign_stability(rows: list[dict[str, Any]], feature: str) -> dict[str, Any]:
    signs = []
    details = {}
    for bin_name, bin_rows in grouped_by_score_bin(rows).items():
        effect = effect_for_feature(bin_rows, feature)
        delta = effect.get("brierDeltaPresentMinusAbsent")
        if delta is None or effect["support"] < 3:
            continue
        sign = 1 if delta > 0 else -1 if delta < 0 else 0
        signs.append(sign)
        details[bin_name] = {"support": effect["support"], "brierDeltaPresentMinusAbsent": delta, "sign": sign}
    nonzero = [sign for sign in signs if sign != 0]
    stable = bool(nonzero) and (all(sign > 0 for sign in nonzero) or all(sign < 0 for sign in nonzero))
    return {"stable": stable, "observedBins": len(details), "details": details}


def rejection_reasons(effect: dict[str, Any], sign_stability: dict[str, Any]) -> list[str]:
    reasons = []
    frac = effect.get("nonzeroFraction")
    if frac is not None and frac > 0.90:
        reasons.append("GENERIC_HIGH_SUPPORT_GT_90PCT")
    if frac is not None and frac < 0.02:
        reasons.append("TOO_RARE_LT_2PCT")
    if sign_stability["observedBins"] >= 2 and not sign_stability["stable"]:
        reasons.append("SCORE_BIN_SIGN_UNSTABLE")
    return reasons


def selector_score(
    effect: dict[str, Any],
    per_cell: dict[str, Any],
    sign_stability: dict[str, Any],
    rejection: list[str],
) -> float:
    score = 0.0
    for key, weight in (
        ("labelCorrelation", 1.0),
        ("absErrorCorrelation", 1.0),
        ("brierCorrelation", 1.0),
        ("activeCorrelation", 0.7),
    ):
        value = effect.get(key)
        if value is not None:
            score += weight * abs(float(value))
    weak_lift = 0.0
    for cell in ("delete_test_call::python", "off_by_one::python", "subtle_flip::python"):
        cell_effect = per_cell.get(cell, {}).get("calibration")
        if cell_effect and cell_effect.get("brierCorrelation") is not None:
            weak_lift += abs(float(cell_effect["brierCorrelation"]))
    score += 1.5 * weak_lift
    if sign_stability["stable"]:
        score *= 1.1
    if rejection:
        score *= 0.25
    return finite_float(score)


def weak_cell_summary(candidates: list[dict[str, Any]]) -> dict[str, Any]:
    out = {}
    for cell in ("delete_test_call::python", "off_by_one::python", "subtle_flip::python"):
        rows = [
            {
                "feature": row["feature"],
                "pairId": row["pairId"],
                "featureKind": row["featureKind"],
                "selectorScore": row["selectorScore"],
                "cellCalibration": row["perCell"][cell]["calibration"],
                "cellHoldoutReportingOnly": row["perCell"][cell]["holdoutReportingOnly"],
                "rejectionReasons": row["rejectionReasons"],
            }
            for row in candidates
            if cell in row["perCell"]
        ]
        rows.sort(key=lambda row: (-float(row["selectorScore"]), row["feature"]))
        out[cell] = rows[:12]
    return out


def motif_effect_summary(candidates: list[dict[str, Any]]) -> dict[str, Any]:
    grouped: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for row in candidates:
        grouped[row["motif"]].append(row)
    out = {}
    for motif, rows in grouped.items():
        rows.sort(key=lambda row: (-float(row["selectorScore"]), row["feature"]))
        out[motif] = {
            "candidateCount": len(rows),
            "topSelectorScore": rows[0]["selectorScore"],
            "topFeature": rows[0]["feature"],
            "topPairId": rows[0]["pairId"],
        }
    return dict(sorted(out.items(), key=lambda item: (-float(item[1]["topSelectorScore"]), item[0]))[:30])


def split_report(rows: list[dict[str, Any]], score_key: str) -> dict[str, Any]:
    active = [row for row in rows if row["active_after_abstain"] is True]
    abstain = [row for row in rows if row["active_after_abstain"] is False]
    return {
        "full": metric_block([row["actual"] for row in rows], [row[score_key] for row in rows]),
        "active392": metric_block([row["actual"] for row in active], [row[score_key] for row in active]) if active else None,
        "abstainPool": metric_block([row["actual"] for row in abstain], [row[score_key] for row in abstain]) if abstain else None,
        "rowCounts": {"full": len(rows), "active392": len(active), "abstainPool": len(abstain)},
    }


def recommendation(universe: dict[str, Any], top_candidates: list[dict[str, Any]]) -> dict[str, Any]:
    blockers = []
    if universe["documentedExpectedPairCount"] != 105:
        blockers.append("DOCUMENTED_PAIR_UNIVERSE_NOT_105")
    if universe["activeDerivablePairCount"] < universe["documentedExpectedPairCount"]:
        blockers.append("CURRENT_LABEL_ROOT_HAS_ONLY_ACTIVE_DERIVABLE_SUBSET")
    usable = [row for row in top_candidates if not row["rejectionReasons"]]
    if not usable:
        blockers.append("NO_NON_REJECTED_PAIR_CANDIDATES")
    return {
        "recommendedAction": "build_calibration_selected_conditional_pair_admission_probe" if usable else "inspect_pair_label_materialization_gap",
        "blockers": blockers,
        "candidateFeatureCountWithoutRejection": len(usable),
        "topUsableCandidate": usable[0] if usable else None,
    }


def derived_pair_records(row: dict[str, Any]) -> dict[str, dict[str, Any]]:
    features = slot_features(row)
    out = {}
    for left, right in combinations(sorted(features), 2):
        record = pair_record(left, features[left], right, features[right])
        out[record["pair_id"]] = record
    return out


def pair_record(left_id: str, left: dict[str, float], right_id: str, right: dict[str, float]) -> dict[str, Any]:
    deltas = {feature: abs(float(left[feature]) - float(right[feature])) for feature in SCALAR_FEATURES}
    driver, max_delta = max(deltas.items(), key=lambda item: item[1])
    if max_delta < 0.005:
        state = "near_identity_cross_slot_delta"
    elif max_delta < 0.020:
        state = "low_cross_slot_delta"
    elif max_delta < 0.050:
        state = "moderate_cross_slot_delta"
    elif max_delta < 0.100:
        state = "high_cross_slot_delta"
    else:
        state = "extreme_cross_slot_delta"
    pair = pair_id(left_id, right_id)
    return {
        "pair_id": pair,
        "left": left_id,
        "right": right_id,
        "state": state,
        "driver": driver,
        "max_delta": finite_float(max_delta),
        "label": f"pair:{pair}:{state}:driver:{driver}",
    }


def slot_features(row: dict[str, Any]) -> dict[str, dict[str, float]]:
    raw = row.get("slot_feature_means")
    if not isinstance(raw, dict):
        return {}
    out = {}
    for slot, values in raw.items():
        if not isinstance(values, dict):
            continue
        parsed = {}
        ok = True
        for feature in SCALAR_FEATURES:
            value = values.get(feature)
            if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(float(value)):
                ok = False
                break
            parsed[feature] = float(value)
        if ok:
            out[str(slot)] = parsed
    return out


def pair_label_counts(row: dict[str, Any]) -> dict[str, int]:
    raw = row.get("pair_label_counts_top")
    if not isinstance(raw, dict):
        return {}
    return {str(key): int(value) for key, value in raw.items() if isinstance(value, int)}


def require_state(states: dict[tuple[str, str], dict[str, Any]], task_id: str, workspace_state: str) -> dict[str, Any]:
    row = states.get((task_id, workspace_state))
    if row is None:
        raise SwebenchOpsError(
            "PY_G_124B_CODE_STATE_ROW_MISSING",
            "required code-state row is missing",
            "Use a complete #117 code-state constellation root.",
            {"task_id": task_id, "workspace_state": workspace_state},
        )
    return row


def grouped_by_cell(rows: list[dict[str, Any]]) -> dict[str, list[dict[str, Any]]]:
    grouped: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for row in rows:
        grouped[f"{row['category']}::python"].append(row)
    return grouped


def grouped_by_score_bin(rows: list[dict[str, Any]]) -> dict[str, list[dict[str, Any]]]:
    grouped: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for row in rows:
        grouped[row["score_bin"]].append(row)
    return grouped


def row_feature_samples(calibration: list[dict[str, Any]], holdout: list[dict[str, Any]], *, limit: int) -> list[dict[str, Any]]:
    rows = []
    for split, source in (("calibration", calibration), ("holdout", holdout)):
        for row in source[: max(1, limit // 2)]:
            rows.append(
                {
                    "split": split,
                    "task_id": row["task_id"],
                    "category": row["category"],
                    "active_after_abstain": row["active_after_abstain"],
                    "pair_feature_count": row["pair_feature_count"],
                    "pair_feature_sample": row["pair_features"][:20],
                }
            )
    return rows


def point_biserial(indicator: list[int], values: list[float | int]) -> float | None:
    if len(indicator) < 3 or len(indicator) != len(values) or len(set(indicator)) < 2:
        return None
    xs = [float(value) for value in indicator]
    ys = [float(value) for value in values]
    x_mean = sum(xs) / len(xs)
    y_mean = sum(ys) / len(ys)
    x_var = sum((value - x_mean) ** 2 for value in xs)
    y_var = sum((value - y_mean) ** 2 for value in ys)
    if x_var <= 1.0e-12 or y_var <= 1.0e-12:
        return None
    cov = sum((x - x_mean) * (y - y_mean) for x, y in zip(xs, ys))
    return finite_float(cov / math.sqrt(x_var * y_var))


def mean(values: list[float | int]) -> float | None:
    if not values:
        return None
    return finite_float(sum(float(value) for value in values) / len(values))


def none_delta(before: float | None, after: float | None) -> float | None:
    if before is None or after is None:
        return None
    return finite_float(after - before)


def pair_id(left: str, right: str) -> str:
    return f"{left}/{right}"


def pair_id_from_label(label: str) -> str:
    parts = label.split(":")
    return parts[1] if len(parts) > 1 else "unknown"


def pair_id_from_feature(feature: str) -> str:
    body = feature.split("::", 1)[-1]
    if body.startswith("pair:"):
        return pair_id_from_label(body)
    parts = body.split(":")
    return parts[0] if "/" in parts[0] else "unknown"


def motif_from_feature(feature: str) -> str:
    if feature.startswith("target_family::"):
        return feature.split("::", 1)[1].rsplit(":", 1)[0]
    pair = pair_id_from_feature(feature)
    if "/" in pair:
        left, right = pair.split("/", 1)
        return f"{slot_family(left)}/{slot_family(right)}"
    return "unknown"


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


def score_bin(score: float) -> str:
    if score < 0.2:
        return "score_bin:very_low"
    if score < 0.4:
        return "score_bin:low"
    if score < 0.6:
        return "score_bin:middle"
    if score < 0.8:
        return "score_bin:high"
    return "score_bin:very_high"


def delta_shift_bucket(value: float) -> str:
    if value <= -0.05:
        return "large_decrease"
    if value <= -0.01:
        return "small_decrease"
    if value < 0.01:
        return "stable"
    if value < 0.05:
        return "small_increase"
    return "large_increase"


def clamp_prob(value: float) -> float:
    return min(0.999, max(0.001, finite_float(value)))


def required_string(row: dict[str, Any], key: str) -> str:
    value = row.get(key)
    if not isinstance(value, str) or not value:
        raise SwebenchOpsError(
            "PY_G_124B_STRING_REQUIRED",
            f"{key} must be a non-empty string",
            "Fix the source artifact schema before using it for gate evidence.",
            {"key": key, "actual": value},
        )
    return value


def required_bool(row: dict[str, Any], key: str) -> bool:
    value = row.get(key)
    if not isinstance(value, bool):
        raise SwebenchOpsError(
            "PY_G_124B_BOOL_REQUIRED",
            f"{key} must be boolean",
            "Fix the source artifact schema before using it for gate evidence.",
            {"key": key, "actual": value},
        )
    return value


def required_float(row: dict[str, Any], key: str) -> float:
    value = row.get(key)
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(float(value)):
        raise SwebenchOpsError(
            "PY_G_124B_FLOAT_REQUIRED",
            f"{key} must be a finite number",
            "Fix the source artifact schema before using it for gate evidence.",
            {"key": key, "actual": value},
        )
    return float(value)
