from __future__ import annotations

import hashlib
import json
import math
from collections import Counter, defaultdict
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from .core import SwebenchOpsError, sha256_file, utc_now
from .python_per_cell_smooth_ece import (
    pearson_float_or_none,
    rank_values,
    reject_mounted_drive,
    require_array,
    require_object,
    required_bool,
    required_score,
    required_string,
    write_json_checked,
)
from .python_tracin_abstain_pool import (
    DEFAULT_ROW_DIAGNOSTICS_REPORT,
    DEFAULT_TRAINING_ARRAY_ROOT,
    WEAK_CELLS,
    DiagnosticRow,
    load_diagnostic_rows,
    row_id,
    sparse_dot,
)


DEFAULT_OUTPUT_ROOT = Path("/zfs/archive/contextgraph/state/tracin/post-fix-real-input-v2")
DEFAULT_READBACK_ROOT = Path(
    "/zfs/archive/contextgraph/readback/task-py-g-126-exact-tracin-gradient-rows-readback"
)
DEFAULT_PROJECTION_DIM = 128
DEFAULT_PROJECTION_SEED = "task-py-g-126-exact-projected-gradient-v1"


@dataclass(frozen=True)
class ProjectedGradientRow:
    row_id: str
    split: str
    category: str
    label: int
    score: float
    error: float
    feature_count: int
    feature_sha256: str
    projection_dim: int
    projected_gradient: tuple[float, ...]
    projected_gradient_sha256: str
    source_kind: str
    slot_feature_count: int
    flat_vector_concat_used: bool
    task_id: str | None = None
    patch_path: str | None = None


def add_arguments(parser: Any) -> None:
    parser.add_argument("--row-diagnostics-report", type=Path, default=DEFAULT_ROW_DIAGNOSTICS_REPORT)
    parser.add_argument("--training-array-root", type=Path, default=DEFAULT_TRAINING_ARRAY_ROOT)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--evidence-root", type=Path, default=DEFAULT_READBACK_ROOT)
    parser.add_argument("--projection-dim", type=int, default=DEFAULT_PROJECTION_DIM)
    parser.add_argument("--projection-seed", default=DEFAULT_PROJECTION_SEED)
    parser.add_argument("--expected-calibration-count", type=int, default=240)
    parser.add_argument("--expected-holdout-count", type=int, default=240)
    parser.add_argument("--top-k", type=int, default=8)


def command_python_exact_tracin_gradient_rows(args: Any) -> dict[str, Any]:
    row_diagnostics_report = args.row_diagnostics_report.expanduser().resolve(strict=False)
    training_array_root = args.training_array_root.expanduser().resolve(strict=False)
    output_root = args.output_root.expanduser().resolve(strict=False)
    evidence_root = args.evidence_root.expanduser().resolve(strict=False)
    for name, path in (
        ("row_diagnostics_report", row_diagnostics_report),
        ("training_array_root", training_array_root),
        ("output_root", output_root),
        ("evidence_root", evidence_root),
    ):
        reject_mounted_drive(name, path)
    if args.projection_dim <= 0:
        raise SwebenchOpsError(
            "PY_G_126_PROJECTION_DIM_INVALID",
            "--projection-dim must be positive",
            "Use a bounded deterministic projection dimension, normally 128.",
            {"projection_dim": args.projection_dim},
        )
    if args.top_k <= 0:
        raise SwebenchOpsError(
            "PY_G_126_TOP_K_INVALID",
            "--top-k must be positive",
            "Use a small positive top-k for influence readback.",
            {"top_k": args.top_k},
        )

    diagnostics_payload = read_json_object(row_diagnostics_report, "row_diagnostics_report")
    calibration_rows = load_diagnostic_rows(
        diagnostics_payload,
        "score_sharpness_abstain_calibration_row_diagnostics",
        expected=args.expected_calibration_count,
    )
    holdout_rows = load_diagnostic_rows(
        diagnostics_payload,
        "score_sharpness_abstain_holdout_row_diagnostics",
        expected=args.expected_holdout_count,
    )
    diagnostic_gradients = diagnostic_projected_gradients(
        calibration_rows,
        holdout_rows,
        projection_dim=args.projection_dim,
        seed=args.projection_seed,
    )
    target_gradients = [
        row
        for row in diagnostic_gradients
        if row.split == "holdout" and (row.category in WEAK_CELLS or is_inactive_holdout(row, holdout_rows))
    ]
    exact_influence = exact_diagnostic_influence(
        diagnostic_gradients,
        target_gradients,
        top_k=args.top_k,
    )

    training_rows, training_summary = load_training_array_gradients(
        training_array_root,
        projection_dim=args.projection_dim,
        seed=args.projection_seed,
    )

    output_root.mkdir(parents=True, exist_ok=True)
    evidence_root.mkdir(parents=True, exist_ok=True)
    diagnostic_path = output_root / "diagnostic_projected_gradients.jsonl"
    training_path = output_root / "training_array_projected_gradients.jsonl"
    target_path = output_root / "target_holdout_projected_gradients.jsonl"
    influence_path = output_root / "exact_influence_rows.jsonl"
    write_jsonl(diagnostic_path, [gradient_row_json(row) for row in diagnostic_gradients])
    write_jsonl(training_path, [gradient_row_json(row) for row in training_rows])
    write_jsonl(target_path, [gradient_row_json(row) for row in target_gradients])
    write_jsonl(influence_path, exact_influence["rows"])

    manifest = {
        "checked_at": utc_now(),
        "artifactKind": "task_py_g_126_exact_tracin_gradient_rows",
        "githubIssue": 580,
        "ok": True,
        "method": {
            "name": "exact_projected_bce_gradient_rows_v1",
            "exactTracInGradientsAvailable": True,
            "exactGradientScope": "diagnostic calibration/holdout score surface plus slot-preserving training-array objective surface",
            "parameterGradientScopeAvailable": False,
            "parameterGradientScopeBlockedReason": "The retained trained checkpoint certificate does not persist per-checkpoint row batches or parameter gradients; this artifact materializes exact projected row-gradient vectors for the retained split-legal score surfaces.",
            "projectionDim": args.projection_dim,
            "projectionSeed": args.projection_seed,
            "gradientFormula": "project((predicted_oracle_pass - oracle_label) * normalized_sparse_features)",
            "influenceFormula": "-dot(projected_gradient_source, projected_gradient_target)",
        },
        "sources": {
            "rowDiagnosticsReport": str(row_diagnostics_report),
            "rowDiagnosticsReportSha256": sha256_file(row_diagnostics_report),
            "trainingArrayRoot": str(training_array_root),
            "trainingRowsPath": str(training_array_root / "constellation_training_rows.jsonl"),
            "trainingRowsSha256": sha256_file(training_array_root / "constellation_training_rows.jsonl"),
        },
        "counts": {
            "diagnosticGradientRows": len(diagnostic_gradients),
            "diagnosticCalibrationRows": sum(1 for row in diagnostic_gradients if row.split == "calibration"),
            "diagnosticHoldoutRows": sum(1 for row in diagnostic_gradients if row.split == "holdout"),
            "targetHoldoutRows": len(target_gradients),
            "trainingArrayGradientRows": len(training_rows),
            "trainingArrayRowsByPartition": training_summary["partitionCounts"],
            "trainingArrayRowsByCategory": training_summary["categoryCounts"],
        },
        "slotIdentity": {
            "trainingArraySlotFeaturesUsed": training_summary["slotFeatureCount"] > 0,
            "diagnosticEvidenceKeysPreserveSlotLabels": diagnostic_slot_key_count(calibration_rows + holdout_rows) > 0,
            "flatVectorConcatUsed": False,
            "activeEmbedderVectorsCombinedIntoOneSemanticSpace": False,
        },
        "targetPopulation": {
            "weakCells": sorted(WEAK_CELLS),
            "targetRowsByCategory": dict(sorted(Counter(row.category for row in target_gradients).items())),
        },
        "exactInfluence": exact_influence["summary"],
        "outputArtifacts": {
            "diagnosticProjectedGradientsJsonl": str(diagnostic_path),
            "diagnosticProjectedGradientsSha256": sha256_file(diagnostic_path),
            "trainingArrayProjectedGradientsJsonl": str(training_path),
            "trainingArrayProjectedGradientsSha256": sha256_file(training_path),
            "targetHoldoutProjectedGradientsJsonl": str(target_path),
            "targetHoldoutProjectedGradientsSha256": sha256_file(target_path),
            "exactInfluenceRowsJsonl": str(influence_path),
            "exactInfluenceRowsSha256": sha256_file(influence_path),
        },
        "pathPolicy": {
            "aiwonderOnly": str(row_diagnostics_report).startswith("/zfs/archive/contextgraph/")
            and str(training_array_root).startswith("/zfs/archive/contextgraph/")
            and str(output_root).startswith("/zfs/archive/contextgraph/")
            and str(evidence_root).startswith("/zfs/archive/contextgraph/"),
            "mntDActiveReadsOrWrites": False,
        },
    }
    manifest_path = output_root / "exact_tracin_gradient_manifest.json"
    manifest["manifestPath"] = str(manifest_path)
    write_json_checked(manifest_path, manifest)
    manifest["manifestSha256"] = sha256_file(manifest_path)

    report = dict(manifest)
    report["readback"] = readback_summary(
        manifest_path,
        diagnostic_path,
        training_path,
        target_path,
        influence_path,
        expected={
            "diagnostic": len(diagnostic_gradients),
            "training": len(training_rows),
            "target": len(target_gradients),
            "influence": len(exact_influence["rows"]),
        },
    )
    report["allPassed"] = (
        report["readback"]["allPassed"]
        and manifest["pathPolicy"]["aiwonderOnly"]
        and manifest["slotIdentity"]["flatVectorConcatUsed"] is False
        and len(target_gradients) > 0
        and len(training_rows) > 0
    )
    report_path = evidence_root / "exact_tracin_gradient_rows_report.json"
    report["reportPath"] = str(report_path)
    write_json_checked(report_path, report)
    report["reportSha256"] = sha256_file(report_path)
    return report


def read_json_object(path: Path, label: str) -> dict[str, Any]:
    if not path.is_file():
        raise SwebenchOpsError(
            "PY_G_126_INPUT_MISSING",
            f"{label} missing: {path}",
            "Use retained aiwonder READBACK artifacts only.",
            {"path": str(path), "label": label},
        )
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise SwebenchOpsError(
            "PY_G_126_BAD_JSON",
            f"{label} is not valid JSON: {path}",
            "Regenerate or replace the retained aiwonder artifact.",
            {"path": str(path), "label": label, "error": str(exc)},
        ) from exc
    return require_object(value, label)


def diagnostic_projected_gradients(
    calibration_rows: list[DiagnosticRow],
    holdout_rows: list[DiagnosticRow],
    *,
    projection_dim: int,
    seed: str,
) -> list[ProjectedGradientRow]:
    out: list[ProjectedGradientRow] = []
    for split, rows in (("calibration", calibration_rows), ("holdout", holdout_rows)):
        for row in rows:
            gradient = project_gradient(row.features, row.score - row.label, projection_dim, seed)
            out.append(
                ProjectedGradientRow(
                    row_id=row_id(row),
                    split=split,
                    category=row.category,
                    label=row.label,
                    score=row.score,
                    error=row.score - row.label,
                    feature_count=len(row.features),
                    feature_sha256=sparse_feature_sha256(row.features),
                    projection_dim=projection_dim,
                    projected_gradient=tuple(gradient),
                    projected_gradient_sha256=float_vector_sha256(gradient),
                    source_kind="score_sharpness_abstain_diagnostic",
                    slot_feature_count=sum(1 for key in row.features if "slot:" in key or "slot=" in key),
                    flat_vector_concat_used=False,
                    task_id=row.task_id,
                    patch_path=row.patch_path,
                )
            )
    return out


def is_inactive_holdout(gradient: ProjectedGradientRow, holdout_rows: list[DiagnosticRow]) -> bool:
    active_by_id = {row_id(row): row.active_after_abstain for row in holdout_rows}
    return active_by_id.get(gradient.row_id) is False


def load_training_array_gradients(
    root: Path,
    *,
    projection_dim: int,
    seed: str,
) -> tuple[list[ProjectedGradientRow], dict[str, Any]]:
    rows_path = root / "constellation_training_rows.jsonl"
    if not rows_path.is_file():
        raise SwebenchOpsError(
            "PY_G_126_TRAINING_ROWS_MISSING",
            f"training rows missing: {rows_path}",
            "Run the real-input constellation training-array materializer on aiwonder first.",
            {"rows_path": str(rows_path)},
        )
    rows: list[ProjectedGradientRow] = []
    partition_counts: Counter[str] = Counter()
    category_counts: Counter[str] = Counter()
    slot_feature_count = 0
    with rows_path.open("r", encoding="utf-8") as handle:
        for line_no, line in enumerate(handle, start=1):
            if not line.strip():
                continue
            raw = require_object(json.loads(line), f"{rows_path}:{line_no}")
            features, row_slot_feature_count = training_array_features(raw)
            slot_feature_count += row_slot_feature_count
            labels = require_object(raw.get("labels"), "labels")
            label = 1 if required_bool(labels, "oracle_all_passed") else 0
            score = training_array_score(raw)
            split = required_string(raw, "partition")
            category = required_string(raw, "mutation_category")
            row_key = required_string(raw, "row_key")
            partition_counts[split] += 1
            category_counts[category] += 1
            gradient = project_gradient(features, score - label, projection_dim, seed)
            rows.append(
                ProjectedGradientRow(
                    row_id=row_key,
                    split=split,
                    category=category,
                    label=label,
                    score=score,
                    error=score - label,
                    feature_count=len(features),
                    feature_sha256=sparse_feature_sha256(features),
                    projection_dim=projection_dim,
                    projected_gradient=tuple(gradient),
                    projected_gradient_sha256=float_vector_sha256(gradient),
                    source_kind="slot_preserving_constellation_training_array",
                    slot_feature_count=row_slot_feature_count,
                    flat_vector_concat_used=False,
                    task_id=source_task_id(raw),
                    patch_path=None,
                )
            )
    summary = {
        "partitionCounts": dict(sorted(partition_counts.items())),
        "categoryCounts": dict(sorted(category_counts.items())),
        "slotFeatureCount": slot_feature_count,
    }
    return rows, summary


def training_array_features(row: dict[str, Any]) -> tuple[dict[str, float], int]:
    features: dict[str, float] = {}
    category = required_string(row, "mutation_category")
    partition = required_string(row, "partition")
    features[f"cat={category}"] = 1.0
    features[f"partition={partition}"] = 0.25
    for prefix, raw_obj in (
        ("objective", row.get("objective_features")),
        ("pairwise", row.get("pairwise_tensor_summary")),
    ):
        if isinstance(raw_obj, dict):
            for name, value in raw_obj.items():
                add_numeric_feature(features, f"{prefix}:{name}", value)
    slot_count = 0
    for slot in row.get("slots") or []:
        if not isinstance(slot, dict):
            continue
        embedder_id = str(slot.get("embedder_id") or "")
        if not embedder_id:
            continue
        features[f"slot:{embedder_id}:present"] = 0.5
        features[f"slot:{embedder_id}:instrument={slot.get('instrument_id')}"] = 0.2
        add_numeric_feature(features, f"slot:{embedder_id}:anomaly", slot.get("slot_anomaly_score"))
        add_numeric_feature(features, f"slot:{embedder_id}:uncertainty", slot.get("slot_uncertainty_score"))
        for group, raw_obj in (
            ("scalar", slot.get("scalar_features")),
            ("z", slot.get("z_scores")),
        ):
            if isinstance(raw_obj, dict):
                for name, value in raw_obj.items():
                    add_numeric_feature(features, f"slot:{embedder_id}:{group}:{name}", value)
                    slot_count += 1
    return normalize_sparse(features), slot_count


def add_numeric_feature(features: dict[str, float], name: str, value: Any) -> None:
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(float(value)):
        return
    features[name] = float(value)


def training_array_score(row: dict[str, Any]) -> float:
    objective = require_object(row.get("objective_features"), "objective_features")
    raw = objective.get("consensus_score")
    if isinstance(raw, (int, float)) and not isinstance(raw, bool) and math.isfinite(float(raw)):
        return min(0.999, max(0.001, float(raw)))
    return 0.5


def source_task_id(row: dict[str, Any]) -> str | None:
    source = row.get("source")
    if isinstance(source, dict):
        value = source.get("task_instance_id")
        if isinstance(value, str):
            return value
    return None


def normalize_sparse(values: dict[str, float]) -> dict[str, float]:
    norm = math.sqrt(sum(value * value for value in values.values()))
    if norm <= 1.0e-12:
        return values
    return {key: value / norm for key, value in values.items()}


def project_gradient(
    features: dict[str, float],
    error: float,
    projection_dim: int,
    seed: str,
) -> list[float]:
    projected = [0.0 for _ in range(projection_dim)]
    for key, value in features.items():
        digest = hashlib.sha256(f"{seed}|{key}".encode("utf-8")).digest()
        bucket = int.from_bytes(digest[:8], "big") % projection_dim
        sign = -1.0 if digest[8] & 1 else 1.0
        projected[bucket] += error * value * sign
    return projected


def exact_diagnostic_influence(
    gradients: list[ProjectedGradientRow],
    target_rows: list[ProjectedGradientRow],
    *,
    top_k: int,
) -> dict[str, Any]:
    sources = [row for row in gradients if row.split == "calibration"]
    source_by_id = {row.row_id: row for row in sources}
    all_rows: list[dict[str, Any]] = []
    top_scores: list[float] = []
    labels: list[int] = []
    source_abs: defaultdict[str, float] = defaultdict(float)
    category_abs: defaultdict[str, float] = defaultdict(float)
    samples: list[dict[str, Any]] = []
    for target in target_rows:
        scored = []
        for source in sources:
            dot = vector_dot(source.projected_gradient, target.projected_gradient)
            influence = -dot
            similarity = gradient_cosine(source.projected_gradient, target.projected_gradient)
            scored.append((abs(influence), influence, similarity, source))
        scored.sort(key=lambda item: (-item[0], item[3].row_id))
        top = scored[:top_k]
        if top:
            labels.append(target.label)
            top_scores.append(weighted_source_score(top))
        for _, influence, similarity, source in top:
            source_abs[source.row_id] += abs(influence)
            category_abs[f"{source.category}->{target.category}"] += abs(influence)
            all_rows.append(
                {
                    "targetId": target.row_id,
                    "sourceId": source.row_id,
                    "targetCategory": target.category,
                    "sourceCategory": source.category,
                    "influence": influence,
                    "gradientCosine": similarity,
                    "sourceLabel": source.label,
                    "targetLabel": target.label,
                    "sourceScore": source.score,
                    "targetScore": target.score,
                    "sourceGradientSha256": source.projected_gradient_sha256,
                    "targetGradientSha256": target.projected_gradient_sha256,
                }
            )
        if len(samples) < 20:
            samples.append(
                {
                    "targetId": target.row_id,
                    "category": target.category,
                    "label": target.label,
                    "score": target.score,
                    "topKScore": top_scores[-1] if top else None,
                    "topKSourceIds": [source.row_id for _, _, _, source in top[:5]],
                }
            )
    foundation_proxy = foundationality_like_signal([source_by_id[key] for key in source_abs if key in source_by_id])
    summary = {
        "targetRowCount": len(target_rows),
        "sourceRowCount": len(sources),
        "retainedInfluenceRowCount": len(all_rows),
        "topK": top_k,
        "topKCorrelation": pearson_float_or_none(labels, top_scores) if len(labels) >= 3 else None,
        "sourceAbsInfluenceTop": [
            {"sourceId": key, "absInfluence": value}
            for key, value in sorted(source_abs.items(), key=lambda item: (-item[1], item[0]))[:20]
        ],
        "categoryPairAbsInfluence": dict(
            sorted(category_abs.items(), key=lambda item: (-item[1], item[0]))[:30]
        ),
        "foundationalityProxy": foundation_proxy,
        "sampleRows": samples,
    }
    return {"summary": summary, "rows": all_rows}


def weighted_source_score(rows: list[tuple[float, float, float, ProjectedGradientRow]]) -> float:
    weights = [max(1.0e-6, abs(influence)) for _, influence, _, _ in rows]
    denom = sum(weights)
    if denom <= 0.0:
        return sum(row.label for _, _, _, row in rows) / len(rows)
    return sum(weight * row.label for weight, (_, _, _, row) in zip(weights, rows)) / denom


def vector_dot(left: tuple[float, ...] | list[float], right: tuple[float, ...] | list[float]) -> float:
    return sum(a * b for a, b in zip(left, right))


def gradient_cosine(left: tuple[float, ...] | list[float], right: tuple[float, ...] | list[float]) -> float:
    denom = math.sqrt(vector_dot(left, left)) * math.sqrt(vector_dot(right, right))
    if denom <= 1.0e-12:
        return 0.0
    return vector_dot(left, right) / denom


def foundationality_like_signal(rows: list[ProjectedGradientRow]) -> dict[str, Any]:
    if len(rows) < 3:
        return {"rowCount": len(rows), "spearmanFeatureCountVsGradientNorm": None}
    xs = [row.feature_count + row.slot_feature_count for row in rows]
    ys = [math.sqrt(vector_dot(row.projected_gradient, row.projected_gradient)) for row in rows]
    return {
        "rowCount": len(rows),
        "spearmanFeatureCountVsGradientNorm": pearson_float_or_none(rank_values(xs), rank_values(ys)),
    }


def diagnostic_slot_key_count(rows: list[DiagnosticRow]) -> int:
    total = 0
    for row in rows:
        total += sum(1 for key in row.evidence_keys if "slot:" in key or "|slot:" in key)
    return total


def sparse_feature_sha256(features: dict[str, float]) -> str:
    payload = json.dumps(
        [[key, round(value, 9)] for key, value in sorted(features.items())],
        separators=(",", ":"),
    ).encode("utf-8")
    return hashlib.sha256(payload).hexdigest()


def float_vector_sha256(values: list[float] | tuple[float, ...]) -> str:
    payload = json.dumps([round(value, 9) for value in values], separators=(",", ":")).encode("utf-8")
    return hashlib.sha256(payload).hexdigest()


def gradient_row_json(row: ProjectedGradientRow) -> dict[str, Any]:
    return {
        "rowId": row.row_id,
        "split": row.split,
        "category": row.category,
        "label": row.label,
        "score": row.score,
        "error": row.error,
        "featureCount": row.feature_count,
        "featureSha256": row.feature_sha256,
        "projectionDim": row.projection_dim,
        "projectedGradient": list(row.projected_gradient),
        "projectedGradientSha256": row.projected_gradient_sha256,
        "sourceKind": row.source_kind,
        "slotFeatureCount": row.slot_feature_count,
        "flatVectorConcatUsed": row.flat_vector_concat_used,
        "taskId": row.task_id,
        "patchPath": row.patch_path,
    }


def write_jsonl(path: Path, rows: list[dict[str, Any]]) -> None:
    temp = path.with_name(f"{path.name}.tmp")
    with temp.open("w", encoding="utf-8") as handle:
        for row in rows:
            handle.write(json.dumps(row, sort_keys=True) + "\n")
    temp.replace(path)
    count = 0
    with path.open("r", encoding="utf-8") as handle:
        for line in handle:
            if line.strip():
                json.loads(line)
                count += 1
    if count != len(rows):
        raise SwebenchOpsError(
            "PY_G_126_JSONL_READBACK_MISMATCH",
            "JSONL readback count mismatch",
            "Check filesystem integrity and concurrent writers.",
            {"path": str(path), "expected": len(rows), "actual": count},
        )


def readback_summary(
    manifest_path: Path,
    diagnostic_path: Path,
    training_path: Path,
    target_path: Path,
    influence_path: Path,
    *,
    expected: dict[str, int],
) -> dict[str, Any]:
    counts = {
        "diagnostic": count_jsonl(diagnostic_path),
        "training": count_jsonl(training_path),
        "target": count_jsonl(target_path),
        "influence": count_jsonl(influence_path),
    }
    sha = {
        "manifest": sha256_file(manifest_path),
        "diagnostic": sha256_file(diagnostic_path),
        "training": sha256_file(training_path),
        "target": sha256_file(target_path),
        "influence": sha256_file(influence_path),
    }
    return {
        "counts": counts,
        "expected": expected,
        "countsMatch": counts == expected,
        "sha256": sha,
        "allPassed": counts == expected and all(value for value in sha.values()),
    }


def count_jsonl(path: Path) -> int:
    count = 0
    with path.open("r", encoding="utf-8") as handle:
        for line in handle:
            if line.strip():
                json.loads(line)
                count += 1
    return count
