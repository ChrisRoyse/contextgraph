from __future__ import annotations

import json
import math
import re
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any

from .core import SwebenchOpsError, sha256_file, utc_now, write_json_checked
from .python_per_cell_smooth_ece import reject_mounted_drive


DEFAULT_CODE_STATES_JSONL = Path(
    "/zfs/archive/contextgraph/state/labels/"
    "task-py-g-117-auto-label-discovery-v2-live-split/code_state_constellations.jsonl"
)
DEFAULT_ROW_DIAGNOSTICS_REPORT = Path(
    "/zfs/archive/contextgraph/fsv/task-py-g-392-temporal-trajectory-features-fsv/"
    "run-20260521T171548Z/task_py_g_392_temporal_trajectory_features_fsv.json"
)
DEFAULT_OUTPUT_ROOT = Path(
    "/zfs/archive/contextgraph/fsv/task-py-g-124-cross-embedder-pair-features-fsv"
)
PAIR_LABEL_RE = re.compile(r"^pair:([^:]+):")
HEAD_PAIR_RE = re.compile(r"pairwise\(([^,]+),\s*([^)]+)\)")


def add_arguments(parser: Any) -> None:
    parser.add_argument("--code-states-jsonl", type=Path, default=DEFAULT_CODE_STATES_JSONL)
    parser.add_argument("--row-diagnostics-report", type=Path, default=DEFAULT_ROW_DIAGNOSTICS_REPORT)
    parser.add_argument("--repo-root", type=Path, default=Path("."))
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--evidence-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--top-k", type=int, default=24)
    parser.add_argument("--min-calibration-support", type=int, default=5)
    parser.add_argument("--expected-code-state-count", type=int, default=3000)
    parser.add_argument("--expected-calibration-count", type=int, default=240)
    parser.add_argument("--expected-holdout-count", type=int, default=240)


def command_python_pair_feature_audit(args: Any) -> dict[str, Any]:
    code_states_jsonl = args.code_states_jsonl.expanduser().resolve(strict=False)
    row_diagnostics_report = args.row_diagnostics_report.expanduser().resolve(strict=False)
    repo_root = args.repo_root.expanduser().resolve(strict=False)
    output_root = args.output_root.expanduser().resolve(strict=False)
    for name, path in (
        ("code_states_jsonl", code_states_jsonl),
        ("row_diagnostics_report", row_diagnostics_report),
        ("repo_root", repo_root),
        ("output_root", output_root),
    ):
        reject_mounted_drive(name, path)
    if args.top_k <= 0:
        raise SwebenchOpsError(
            "PY_G_124_TOP_K_INVALID",
            "--top-k must be positive",
            "Use a small positive top-k for the audit selector.",
            {"top_k": args.top_k},
        )
    if args.min_calibration_support <= 0:
        raise SwebenchOpsError(
            "PY_G_124_MIN_SUPPORT_INVALID",
            "--min-calibration-support must be positive",
            "Use enough calibration support to avoid one-row pair promotion.",
            {"min_calibration_support": args.min_calibration_support},
        )

    states = load_code_states(code_states_jsonl)
    if len(states) != args.expected_code_state_count:
        raise SwebenchOpsError(
            "PY_G_124_CODE_STATE_COUNT_MISMATCH",
            "code-state constellation count does not match expected 3000 rows",
            "Use the current aiwonder label root or update the expected count after proving the corpus changed.",
            {"expected": args.expected_code_state_count, "actual": len(states)},
        )

    diagnostics = read_json_object(row_diagnostics_report, "row_diagnostics_report")
    calibration_rows = load_diagnostic_rows(
        diagnostics,
        "score_sharpness_abstain_calibration_row_diagnostics",
        expected=args.expected_calibration_count,
    )
    holdout_rows = load_diagnostic_rows(
        diagnostics,
        "score_sharpness_abstain_holdout_row_diagnostics",
        expected=args.expected_holdout_count,
    )
    calibration_features = [
        pair_feature_row(row, states) for row in calibration_rows
    ]
    holdout_features = [pair_feature_row(row, states) for row in holdout_rows]

    top_candidates = rank_pair_candidates(
        calibration_features,
        holdout_features,
        top_k=args.top_k,
        min_support=args.min_calibration_support,
    )
    output_root.mkdir(parents=True, exist_ok=True)
    candidates_path = output_root / "pair_feature_candidates.jsonl"
    write_jsonl(candidates_path, top_candidates)

    pair_inventory = inventory_pair_labels(states.values())
    head_projection = scan_head_projection(repo_root)
    evidence_use = explicit_pair_evidence_use(calibration_rows + holdout_rows)
    report = {
        "checked_at": utc_now(),
        "artifactKind": "task_py_g_124_pair_feature_audit",
        "githubIssue": 578,
        "ok": True,
        "codeStatesJsonl": str(code_states_jsonl),
        "codeStatesJsonlSha256": sha256_file(code_states_jsonl),
        "rowDiagnosticsReport": str(row_diagnostics_report),
        "rowDiagnosticsReportSha256": sha256_file(row_diagnostics_report),
        "rowCounts": {
            "codeStates": len(states),
            "calibrationDiagnostics": len(calibration_rows),
            "holdoutDiagnostics": len(holdout_rows),
        },
        "pairInventory": pair_inventory,
        "currentConsumption": {
            "headProjection": head_projection,
            "explicitPairEvidenceKeysIn392Diagnostics": evidence_use,
            "known392Use": {
                "targetPairLabelsAreHashedIntoGenericVector": True,
                "transitionPairIdentityEvidenceKeys": False,
                "transitionPairCauseResidualFeatures": False,
                "interpretation": (
                    "Pair labels exist in code-state constellations, but the current #392 diagnostics "
                    "do not expose explicit pair evidence keys. The active scorer therefore cannot explain "
                    "which cross-embedder pair caused a consequence prediction."
                ),
            },
        },
        "selectorPolicy": {
            "selectionSplit": "calibration only",
            "holdoutUse": "reporting only",
            "topK": args.top_k,
            "minCalibrationSupport": args.min_calibration_support,
            "featureDefinition": "target pair count plus base->target signed pair-count delta, preserving pair labels",
            "noFlatVectorShortcut": True,
            "noNewEmbedders": True,
        },
        "topPairFeatureCandidates": top_candidates,
        "pairTopFeatureCandidates": pair_top_candidates(top_candidates),
        "outputArtifacts": {
            "pairFeatureCandidatesJsonl": str(candidates_path),
            "pairFeatureCandidatesSha256": sha256_file(candidates_path),
        },
        "runtimeFinding": {
            "rejectedAttempt": "directly adding broad pair identity evidence into #392 evidence keys/cause residuals",
            "result": "stopped after retained aiwonder run produced no report in the gate-loop window",
            "nextImplementation": (
                "wire only a train/calibration-selected top-K pair lane, then ECE-gate it per cell; "
                "do not add blanket pair expansion"
            ),
        },
        "pathPolicy": {
            "aiwonderOnly": str(code_states_jsonl).startswith("/zfs/archive/contextgraph/")
            and str(row_diagnostics_report).startswith("/zfs/archive/contextgraph/")
            and str(output_root).startswith("/zfs/archive/contextgraph/"),
            "mntDActiveReadsOrWrites": False,
        },
    }
    report_path = output_root / "pair_feature_audit_report.json"
    report["reportPath"] = str(report_path)
    write_json_checked(report_path, report)
    report["reportSha256"] = sha256_file(report_path)
    return report


def load_code_states(path: Path) -> dict[tuple[str, str], dict[str, Any]]:
    if not path.is_file():
        raise SwebenchOpsError(
            "PY_G_124_CODE_STATES_MISSING",
            f"code-state constellations missing: {path}",
            "Use the aiwonder label root from #404/#117.",
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
                    "PY_G_124_DUPLICATE_CODE_STATE",
                    "duplicate code-state row identity",
                    "Regenerate the label root with unique task/workspace rows.",
                    {"line": line_number, "task_id": task_id, "workspace_state": workspace_state},
                )
            out[key] = row
    return out


def load_diagnostic_rows(payload: dict[str, Any], key: str, *, expected: int) -> list[dict[str, Any]]:
    rows = require_array(payload.get(key), key)
    if len(rows) != expected:
        raise SwebenchOpsError(
            "PY_G_124_DIAGNOSTIC_ROW_COUNT_MISMATCH",
            f"{key} row count mismatch",
            "Use the current #392 report or update the expected count after proving the split changed.",
            {"key": key, "expected": expected, "actual": len(rows)},
        )
    return [require_diagnostic_row(row, f"{key}[{index}]") for index, row in enumerate(rows)]


def require_diagnostic_row(value: Any, label: str) -> dict[str, Any]:
    row = require_object(value, label)
    return {
        "task_id": required_string(row, "task_id"),
        "category": required_string(row, "category"),
        "bucket": required_string(row, "bucket"),
        "actual_oracle_pass": required_bool(row, "actual_oracle_pass"),
        "predicted_oracle_pass": required_float(row, "predicted_oracle_pass"),
        "active_after_abstain": row.get("active_after_abstain"),
        "evidence_key_sample": list(require_array(row.get("evidence_key_sample"), "evidence_key_sample")),
    }


def pair_feature_row(row: dict[str, Any], states: dict[tuple[str, str], dict[str, Any]]) -> dict[str, Any]:
    task_id = row["task_id"]
    category = row["category"]
    base = required_state(states, task_id, "base")
    target = required_state(states, task_id, target_workspace_state(category))
    target_pairs = pair_counts(target)
    base_pairs = pair_counts(base)
    features: dict[str, float] = {}
    for key in set(target_pairs) | set(base_pairs):
        features[f"target::{key}"] = float(target_pairs.get(key, 0))
        delta = float(target_pairs.get(key, 0) - base_pairs.get(key, 0))
        if delta:
            features[f"delta::{key}"] = delta
    return {
        "task_id": task_id,
        "category": category,
        "label": 1 if row["actual_oracle_pass"] else 0,
        "score": float(row["predicted_oracle_pass"]),
        "active_after_abstain": row.get("active_after_abstain"),
        "features": features,
    }


def rank_pair_candidates(
    calibration: list[dict[str, Any]],
    holdout: list[dict[str, Any]],
    *,
    top_k: int,
    min_support: int,
) -> list[dict[str, Any]]:
    feature_names = sorted({name for row in calibration for name in row["features"]})
    candidates = []
    labels = [row["label"] for row in calibration]
    errors = [abs(row["score"] - row["label"]) for row in calibration]
    holdout_by_name = feature_values_by_name(holdout)
    for name in feature_names:
        values = [float(row["features"].get(name, 0.0)) for row in calibration]
        support = sum(1 for value in values if abs(value) > 1.0e-12)
        if support < min_support:
            continue
        label_corr = pearson_float_or_none([float(v) for v in labels], values)
        error_corr = pearson_float_or_none(errors, values)
        if label_corr is None and error_corr is None:
            continue
        pair_id = pair_id_from_feature_name(name)
        holdout_values = holdout_by_name.get(name, [])
        candidates.append(
            {
                "feature": name,
                "pairId": pair_id,
                "featureKind": name.split("::", 1)[0],
                "calibrationSupport": support,
                "calibrationNonZeroFraction": support / len(calibration),
                "calibrationLabelCorrelation": label_corr,
                "calibrationErrorCorrelation": error_corr,
                "calibrationAbsSelectorScore": max(abs_or_zero(label_corr), abs_or_zero(error_corr)),
                "holdoutSupportReportingOnly": sum(1 for value in holdout_values if abs(value) > 1.0e-12),
                "holdoutNonZeroFractionReportingOnly": (
                    sum(1 for value in holdout_values if abs(value) > 1.0e-12) / len(holdout)
                    if holdout
                    else 0.0
                ),
            }
        )
    candidates.sort(
        key=lambda item: (
            -float(item["calibrationAbsSelectorScore"]),
            -int(item["calibrationSupport"]),
            str(item["feature"]),
        )
    )
    return candidates[:top_k]


def pair_top_candidates(candidates: list[dict[str, Any]]) -> dict[str, list[dict[str, Any]]]:
    by_pair: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for item in candidates:
        by_pair[str(item["pairId"])].append(item)
    return {pair: rows[:3] for pair, rows in sorted(by_pair.items())}


def feature_values_by_name(rows: list[dict[str, Any]]) -> dict[str, list[float]]:
    names = sorted({name for row in rows for name in row["features"]})
    return {name: [float(row["features"].get(name, 0.0)) for row in rows] for name in names}


def inventory_pair_labels(rows: Any) -> dict[str, Any]:
    pair_label_counter: Counter[str] = Counter()
    pair_id_counter: Counter[str] = Counter()
    state_counter: Counter[str] = Counter()
    for row in rows:
        state_counter[required_string(row, "workspace_state")] += 1
        for key, count in pair_counts(row).items():
            pair_label_counter[key] += int(count)
            pair_id_counter[pair_id_from_label(key)] += int(count)
    return {
        "stateRowsByWorkspace": dict(sorted(state_counter.items())),
        "uniquePairLabels": len(pair_label_counter),
        "uniquePairIds": len(pair_id_counter),
        "topPairIds": [
            {"pairId": pair_id, "count": count} for pair_id, count in pair_id_counter.most_common(20)
        ],
        "topPairLabels": [
            {"label": label, "count": count} for label, count in pair_label_counter.most_common(20)
        ],
    }


def scan_head_projection(repo_root: Path) -> dict[str, Any]:
    path = repo_root / "crates" / "context-graph-mejepa" / "src" / "head_projection.rs"
    if not path.is_file():
        return {"path": str(path), "available": False, "consumedPairSpecs": []}
    text = path.read_text(encoding="utf-8")
    specs = []
    for left, right in HEAD_PAIR_RE.findall(text):
        clean_left = left.strip()
        clean_right = right.strip()
        if clean_left.startswith("E_") and clean_right.startswith("E_"):
            specs.append({"left": clean_left, "right": clean_right})
    return {
        "path": str(path),
        "available": True,
        "consumedPairSpecCount": len(specs),
        "consumedPairSpecs": specs,
        "scalarFeatureDoorPresent": "scalar_features" in text,
    }


def explicit_pair_evidence_use(rows: list[dict[str, Any]]) -> dict[str, Any]:
    total_keys = 0
    pair_keys = Counter()
    for row in rows:
        for key in row["evidence_key_sample"]:
            total_keys += 1
            text = str(key)
            if "|pair:" in text or text.startswith("pair:"):
                pair_keys[text] += 1
    return {
        "diagnosticRows": len(rows),
        "sampledEvidenceKeyCount": total_keys,
        "explicitPairEvidenceKeyCount": sum(pair_keys.values()),
        "uniqueExplicitPairEvidenceKeys": len(pair_keys),
        "topExplicitPairEvidenceKeys": [
            {"key": key, "count": count} for key, count in pair_keys.most_common(10)
        ],
    }


def pair_counts(row: dict[str, Any]) -> dict[str, int]:
    value = row.get("pair_label_counts_top")
    if not isinstance(value, dict):
        return {}
    return {str(key): int(count) for key, count in value.items()}


def pair_id_from_feature_name(name: str) -> str:
    return pair_id_from_label(name.split("::", 1)[-1])


def pair_id_from_label(label: str) -> str:
    match = PAIR_LABEL_RE.match(label)
    if not match:
        return "unknown"
    return match.group(1)


def target_workspace_state(category: str) -> str:
    return "gold" if category == "known_good" else f"mutation_{category}"


def required_state(
    states: dict[tuple[str, str], dict[str, Any]],
    task_id: str,
    workspace_state: str,
) -> dict[str, Any]:
    row = states.get((task_id, workspace_state))
    if row is None:
        raise SwebenchOpsError(
            "PY_G_124_CODE_STATE_ROW_MISSING",
            "required code-state row is missing",
            "Use a complete #404/#117 code-state constellation root.",
            {"task_id": task_id, "workspace_state": workspace_state},
        )
    return row


def read_json_object(path: Path, label: str) -> dict[str, Any]:
    if not path.is_file():
        raise SwebenchOpsError(
            "PY_G_124_INPUT_MISSING",
            f"{label} missing: {path}",
            "Use retained aiwonder FSV artifacts only.",
            {"path": str(path)},
        )
    value = json.loads(path.read_text(encoding="utf-8"))
    return require_object(value, label)


def require_object(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise SwebenchOpsError(
            "PY_G_124_OBJECT_REQUIRED",
            f"{label} must be an object",
            "Fix the source artifact schema before using it for gate evidence.",
            {"label": label, "actual_type": type(value).__name__},
        )
    return value


def require_array(value: Any, label: str) -> list[Any]:
    if not isinstance(value, list):
        raise SwebenchOpsError(
            "PY_G_124_ARRAY_REQUIRED",
            f"{label} must be an array",
            "Fix the source artifact schema before using it for gate evidence.",
            {"label": label, "actual_type": type(value).__name__},
        )
    return value


def required_string(row: dict[str, Any], key: str) -> str:
    value = row.get(key)
    if not isinstance(value, str) or not value:
        raise SwebenchOpsError(
            "PY_G_124_STRING_REQUIRED",
            f"{key} must be a non-empty string",
            "Fix the source artifact schema before using it for gate evidence.",
            {"key": key, "actual": value},
        )
    return value


def required_bool(row: dict[str, Any], key: str) -> bool:
    value = row.get(key)
    if not isinstance(value, bool):
        raise SwebenchOpsError(
            "PY_G_124_BOOL_REQUIRED",
            f"{key} must be boolean",
            "Fix the source artifact schema before using it for gate evidence.",
            {"key": key, "actual": value},
        )
    return value


def required_float(row: dict[str, Any], key: str) -> float:
    value = row.get(key)
    if not isinstance(value, (int, float)) or isinstance(value, bool) or not math.isfinite(float(value)):
        raise SwebenchOpsError(
            "PY_G_124_FLOAT_REQUIRED",
            f"{key} must be a finite number",
            "Fix the source artifact schema before using it for gate evidence.",
            {"key": key, "actual": value},
        )
    return float(value)


def pearson_float_or_none(xs: list[float], ys: list[float]) -> float | None:
    if len(xs) < 3 or len(xs) != len(ys):
        return None
    x_mean = sum(xs) / len(xs)
    y_mean = sum(ys) / len(ys)
    x_var = sum((value - x_mean) ** 2 for value in xs)
    y_var = sum((value - y_mean) ** 2 for value in ys)
    if x_var <= 1.0e-12 or y_var <= 1.0e-12:
        return None
    cov = sum((x - x_mean) * (y - y_mean) for x, y in zip(xs, ys))
    return cov / math.sqrt(x_var * y_var)


def abs_or_zero(value: float | None) -> float:
    return abs(float(value)) if value is not None else 0.0


def write_jsonl(path: Path, rows: list[dict[str, Any]]) -> None:
    tmp = path.with_suffix(path.suffix + ".tmp")
    with tmp.open("w", encoding="utf-8") as handle:
        for row in rows:
            handle.write(json.dumps(row, sort_keys=True))
            handle.write("\n")
    tmp.replace(path)
    readback = []
    with path.open("r", encoding="utf-8") as handle:
        for line in handle:
            if line.strip():
                readback.append(json.loads(line))
    if readback != rows:
        raise SwebenchOpsError(
            "PY_G_124_JSONL_READBACK_MISMATCH",
            f"readback mismatch for {path}",
            "Repair the filesystem or writer before trusting evidence.",
            {"path": str(path)},
        )
