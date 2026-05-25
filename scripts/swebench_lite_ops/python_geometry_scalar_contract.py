"""Materialize the geometry-scalar promotion contract for head projection."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import shutil
from pathlib import Path
from typing import Any, Iterable

from .core import SwebenchOpsError, sha256_file, utc_now
from .python_per_cell_smooth_ece import reject_mounted_drive


SCHEMA_VERSION = 1
ARTIFACT_KIND = "geometry_scalar_feature_contract"
FORMULA_VERSION = "geometry_scalar_feature_contract_v1"

DEFAULT_ADMISSIBILITY_ROOT = Path(
    "/zfs/archive/contextgraph/state/labels/task-py-g-143-geometry-label-admissibility-v2-static-ast-v1"
)
DEFAULT_GEOMETRY_LABEL_ROOT = Path(
    "/zfs/archive/contextgraph/state/labels/task-py-g-136-per-embedder-geometry-labels-v2-static-ast-v1"
)
DEFAULT_LABEL_QUALITY_ROOT = Path(
    "/zfs/archive/contextgraph/state/labels/task-py-g-673-label-quality-embedder-signal-v2-static-ast-v1"
)
DEFAULT_DOWNSTREAM_LIFT_ROOT = Path(
    "/zfs/archive/contextgraph/state/labels/task-py-g-679-label-quality-downstream-lift-v2-static-ast-v1"
)
DEFAULT_OUTPUT_ROOT = Path(
    "/zfs/archive/contextgraph/state/labels/task-py-g-672-geometry-scalar-contract-v2-static-ast-v1"
)

GEOMETRY_SCALAR_SCHEMA = "head-projection-geometry-scalar-contract-v1"
ALLOWED_GEOMETRY_SCALARS = (
    "geometry:label_quality_signal_minus_noise",
    "geometry:label_quality_mean_signal",
    "geometry:label_quality_mean_noise",
    "geometry:label_quality_live_label_ratio",
    "geometry:label_quality_noise_label_ratio",
    "geometry:downstream_weak_cell_regression_flag",
    "geometry:prediction_promotion_allowed",
    "geometry:missingness_or_quarantine_flag",
    "geometry:boundary_or_ood_score",
    "geometry:valid_space_consensus_score",
)
RAW_GEOMETRY_TOKENS = (
    "cluster_id",
    "region_id",
    "label_id",
    "geo_label:",
    "microcell_id",
    "singleton_label_id",
    "raw_cluster",
    "raw_region",
)
LEAKY_TOKENS = (
    "oracle",
    "docker",
    "test_outcome",
    "holdout",
    "target_side",
    "ground_truth",
    "partition",
)


class GeometryScalarContractError(SwebenchOpsError):
    def __init__(self, code: str, message: str, details: dict[str, Any] | None = None):
        super().__init__(
            code,
            message,
            "Repair the geometry-scalar source contract before any geometry label reaches head_projection.scalar_features.",
            details,
        )


def require(condition: bool, code: str, message: str, details: dict[str, Any] | None = None) -> None:
    if not condition:
        raise GeometryScalarContractError(code, message, details)


def load_json(path: Path) -> dict[str, Any]:
    require(path.is_file(), "GEOMETRY_SCALAR_INPUT_MISSING", str(path))
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise GeometryScalarContractError(
            "GEOMETRY_SCALAR_BAD_JSON",
            f"{path}: {exc}",
            {"path": str(path), "error": str(exc)},
        ) from exc
    require(isinstance(value, dict), "GEOMETRY_SCALAR_JSON_NOT_OBJECT", str(path))
    return value


def iter_jsonl(path: Path) -> Iterable[tuple[int, dict[str, Any]]]:
    require(path.is_file(), "GEOMETRY_SCALAR_INPUT_MISSING", str(path))
    with path.open("r", encoding="utf-8") as fh:
        for line_no, line in enumerate(fh, start=1):
            if not line.strip():
                continue
            try:
                row = json.loads(line)
            except json.JSONDecodeError as exc:
                raise GeometryScalarContractError(
                    "GEOMETRY_SCALAR_BAD_JSONL",
                    f"{path}:{line_no}: {exc}",
                    {"path": str(path), "line_no": line_no},
                ) from exc
            require(isinstance(row, dict), "GEOMETRY_SCALAR_JSONL_ROW_NOT_OBJECT", f"{path}:{line_no}")
            yield line_no, row


def atomic_json_dump(path: Path, value: dict[str, Any]) -> str:
    path.parent.mkdir(parents=True, exist_ok=True)
    payload = json.dumps(value, sort_keys=True, indent=2, separators=(",", ": ")) + "\n"
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_text(payload, encoding="utf-8")
    tmp.replace(path)
    return "sha256:" + hashlib.sha256(payload.encode("utf-8")).hexdigest()


class JsonlSink:
    def __init__(self, path: Path):
        self.path = path
        self.tmp = path.with_suffix(path.suffix + ".tmp")
        self.digest = hashlib.sha256()
        self.count = 0
        self.handle: Any = None

    def __enter__(self) -> "JsonlSink":
        self.path.parent.mkdir(parents=True, exist_ok=True)
        self.handle = self.tmp.open("w", encoding="utf-8")
        return self

    def write(self, row: dict[str, Any]) -> None:
        require(self.handle is not None, "GEOMETRY_SCALAR_SINK_NOT_OPEN", str(self.path))
        encoded = json.dumps(row, sort_keys=True, separators=(",", ":")).encode("utf-8")
        self.handle.write(encoded.decode("utf-8"))
        self.handle.write("\n")
        self.digest.update(encoded)
        self.digest.update(b"\n")
        self.count += 1

    def __exit__(self, exc_type: Any, exc: Any, tb: Any) -> None:
        if self.handle is not None:
            self.handle.close()
        if exc_type is None:
            self.tmp.replace(self.path)
        elif self.tmp.exists():
            self.tmp.unlink()

    def artifact(self) -> dict[str, Any]:
        return {
            "path": str(self.path),
            "rows": self.count,
            "bytes": self.path.stat().st_size if self.path.exists() else 0,
            "sha256": "sha256:" + self.digest.hexdigest(),
        }


def command_python_geometry_scalar_contract(args: argparse.Namespace) -> dict[str, Any]:
    started_at = utc_now()
    output_root = args.output_root.expanduser().resolve(strict=False)
    admissibility_root = args.admissibility_root.expanduser().resolve(strict=False)
    geometry_label_root = args.geometry_label_root.expanduser().resolve(strict=False)
    label_quality_root = args.label_quality_root.expanduser().resolve(strict=False)
    downstream_lift_root = args.downstream_lift_root.expanduser().resolve(strict=False)

    for field, path in (
        ("output_root", output_root),
        ("admissibility_root", admissibility_root),
        ("geometry_label_root", geometry_label_root),
        ("label_quality_root", label_quality_root),
        ("downstream_lift_root", downstream_lift_root),
    ):
        reject_mounted_drive(field, path)

    for candidate in args.candidate_scalar_name or []:
        validate_scalar_name(candidate)

    before = {
        "output_root_exists": output_root.exists(),
        "output_root_files": sorted(path.name for path in output_root.iterdir()) if output_root.exists() else [],
    }
    if args.clean_output and output_root.exists():
        shutil.rmtree(output_root)
    output_root.mkdir(parents=True, exist_ok=True)

    policy_path = admissibility_root / "geometry_label_space_policy.json"
    admissibility_manifest_path = admissibility_root / "geometry_label_admissibility_manifest.json"
    geometry_manifest_path = geometry_label_root / "embedder_geometry_label_manifest.json"
    signal_summary_path = label_quality_root / "embedder_signal_noise_summary.json"
    label_quality_manifest_path = label_quality_root / "label_quality_manifest.json"
    source_samples_path = label_quality_root / "label_quality_source_samples.jsonl"
    downstream_summary_path = downstream_lift_root / "label_quality_downstream_lift_summary.json"
    downstream_manifest_path = downstream_lift_root / "label_quality_downstream_lift_manifest.json"

    policy = load_json(policy_path)
    admissibility_manifest = load_json(admissibility_manifest_path)
    geometry_manifest = load_json(geometry_manifest_path)
    signal_summary = load_json(signal_summary_path)
    label_quality_manifest = load_json(label_quality_manifest_path)
    downstream_summary = load_json(downstream_summary_path)
    downstream_manifest = load_json(downstream_manifest_path)

    eligible_embedders = tuple(policy.get("eligible_independent_embedder_ids") or ())
    degenerate_embedders = tuple(policy.get("degenerate_embedder_ids") or ())
    duplicate_alias_map = dict(policy.get("duplicate_alias_map") or {})
    require(
        int(policy.get("valid_independent_embedder_count") or 0) == len(eligible_embedders),
        "GEOMETRY_SCALAR_BAD_POLICY_DENOMINATOR",
        "eligible embedder count does not match policy count",
    )
    require(
        int(policy.get("valid_independent_pair_denominator") or 0) == len(eligible_embedders) * (len(eligible_embedders) - 1) // 2,
        "GEOMETRY_SCALAR_BAD_PAIR_DENOMINATOR",
        "pair denominator does not match eligible embedder count",
    )
    require(
        bool(label_quality_manifest.get("all_passed")) is True,
        "GEOMETRY_SCALAR_LABEL_QUALITY_NOT_PASSED",
        str(label_quality_manifest_path),
    )
    require(
        bool(downstream_manifest.get("all_passed")) is True,
        "GEOMETRY_SCALAR_DOWNSTREAM_LIFT_NOT_PASSED",
        str(downstream_manifest_path),
    )

    by_signal = {str(row.get("embedder_id")): row for row in signal_summary.get("by_embedder_ranked", []) if isinstance(row, dict)}
    by_lift = {str(row.get("embedder_id")): row for row in downstream_summary.get("by_embedder", []) if isinstance(row, dict)}
    all_embedders = sorted(set(by_signal) | set(by_lift) | set(eligible_embedders) | set(degenerate_embedders) | set(duplicate_alias_map))

    contract_path = output_root / "geometry_scalar_feature_contract.json"
    rows_path = output_root / "geometry_scalar_feature_rows.jsonl"
    samples_out_path = output_root / "geometry_scalar_source_samples.jsonl"
    manifest_path = output_root / "geometry_scalar_feature_manifest.json"

    contract = build_contract(
        started_at=started_at,
        eligible_embedders=eligible_embedders,
        degenerate_embedders=degenerate_embedders,
        duplicate_alias_map=duplicate_alias_map,
        source_inventory={
            "admissibility_policy": inventory(policy_path),
            "admissibility_manifest": inventory(admissibility_manifest_path),
            "geometry_manifest": inventory(geometry_manifest_path),
            "label_quality_summary": inventory(signal_summary_path),
            "label_quality_manifest": inventory(label_quality_manifest_path),
            "downstream_lift_summary": inventory(downstream_summary_path),
            "downstream_lift_manifest": inventory(downstream_manifest_path),
        },
    )
    contract_sha = atomic_json_dump(contract_path, contract)

    row_count_by_embedder: dict[str, int] = {}
    with JsonlSink(rows_path) as sink:
        for embedder_id in all_embedders:
            signal = by_signal.get(embedder_id, {})
            lift = by_lift.get(embedder_id, {})
            for row in scalar_rows_for_embedder(
                embedder_id,
                signal=signal,
                lift=lift,
                eligible_embedders=set(eligible_embedders),
                degenerate_embedders=set(degenerate_embedders),
                duplicate_alias_map=duplicate_alias_map,
            ):
                sink.write(row)
                row_count_by_embedder[embedder_id] = row_count_by_embedder.get(embedder_id, 0) + 1
    rows_artifact = sink.artifact()

    sample_rows = select_source_samples(source_samples_path, set(eligible_embedders), set(degenerate_embedders), duplicate_alias_map, int(args.max_source_samples))
    with JsonlSink(samples_out_path) as sink:
        for row in sample_rows:
            sink.write(row)
    samples_artifact = sink.artifact()

    checks = [
        {
            "check": "allowed_scalar_names_validate",
            "passed": all(validate_scalar_name(name) is None for name in ALLOWED_GEOMETRY_SCALARS),
            "actual": list(ALLOWED_GEOMETRY_SCALARS),
            "expected": "all allowed names pass local contract",
        },
        {
            "check": "raw_identifier_names_rejected",
            "passed": all(name_is_rejected(name) for name in ("geometry:cluster_id:e7:42", "region_id", "label_id")),
            "actual": ["geometry:cluster_id:e7:42", "region_id", "label_id"],
            "expected": "raw high-cardinality ids rejected",
        },
        {
            "check": "target_leakage_names_rejected",
            "passed": all(name_is_rejected(name) for name in ("geometry:oracle_agreement", "label_quality_holdout_selected")),
            "actual": ["geometry:oracle_agreement", "label_quality_holdout_selected"],
            "expected": "target/holdout-derived names rejected",
        },
        {
            "check": "valid_policy_denominator_preserved",
            "passed": len(eligible_embedders) == int(policy.get("valid_independent_embedder_count"))
            and int(policy.get("valid_independent_pair_denominator")) == len(eligible_embedders) * (len(eligible_embedders) - 1) // 2,
            "actual": {
                "eligible": list(eligible_embedders),
                "count": policy.get("valid_independent_embedder_count"),
                "pair_denominator": policy.get("valid_independent_pair_denominator"),
            },
            "expected": "policy pair denominator matches current independent static-AST embedder set",
        },
        {
            "check": "alias_and_degenerate_rows_non_live",
            "passed": alias_and_degenerate_rows_non_live(rows_path, set(degenerate_embedders), set(duplicate_alias_map)),
            "actual": {
                "degenerate": list(degenerate_embedders),
                "aliases": duplicate_alias_map,
            },
            "expected": "configured alias/degenerate scalar rows are not independent live signal; static AST v2 has no e8 alias",
        },
        {
            "check": "source_samples_written",
            "passed": samples_artifact["rows"] > 0,
            "actual": samples_artifact["rows"],
            "expected": ">0 source-chain samples",
        },
    ]
    all_passed = all(bool(check["passed"]) for check in checks)
    require(all_passed, "GEOMETRY_SCALAR_CONTRACT_CHECK_FAILED", str(checks))

    manifest = {
        "artifact_kind": "geometry_scalar_feature_manifest",
        "schema_version": SCHEMA_VERSION,
        "formula_version": FORMULA_VERSION,
        "created_at_utc": started_at,
        "githubIssue": 672,
        "all_passed": True,
        "acceptance_status": "geometry_scalar_contract_materialized; raw_geometry_ids_and_leaky_label_scalars_fail_closed",
        "before": before,
        "source_of_truth": contract["source_artifacts"],
        "parameters": {
            "candidate_scalar_name": args.candidate_scalar_name or [],
            "max_source_samples": int(args.max_source_samples),
        },
        "outputs": {
            "geometry_scalar_feature_contract": {
                "path": str(contract_path),
                "bytes": contract_path.stat().st_size,
                "sha256": contract_sha,
            },
            "geometry_scalar_feature_rows": rows_artifact,
            "geometry_scalar_source_samples": samples_artifact,
        },
        "manifest_file": {
            "path": str(manifest_path),
            "sha256_policy": "recorded by command evidence after writing to avoid self-referential manifest hash",
        },
        "row_count_by_embedder": row_count_by_embedder,
        "checks": checks,
    }
    manifest_sha = atomic_json_dump(manifest_path, manifest)
    result = dict(manifest)
    result["manifest_readback"] = {
        "path": str(manifest_path),
        "bytes": manifest_path.stat().st_size,
        "sha256": manifest_sha,
    }
    return result


def build_contract(
    *,
    started_at: str,
    eligible_embedders: tuple[Any, ...],
    degenerate_embedders: tuple[Any, ...],
    duplicate_alias_map: dict[str, Any],
    source_inventory: dict[str, Any],
) -> dict[str, Any]:
    return {
        "artifact_kind": ARTIFACT_KIND,
        "schema_version": SCHEMA_VERSION,
        "formula_version": FORMULA_VERSION,
        "head_projection_geometry_scalar_schema": GEOMETRY_SCALAR_SCHEMA,
        "created_at_utc": started_at,
        "githubIssue": 672,
        "eligible_embedder_set": list(eligible_embedders),
        "denominator_policy": {
            "valid_independent_embedder_count": len(eligible_embedders),
            "valid_independent_pair_denominator": len(eligible_embedders) * (len(eligible_embedders) - 1) // 2,
            "degenerate_embedder_ids": list(degenerate_embedders),
            "duplicate_alias_map": duplicate_alias_map,
        },
        "leakage_policy": {
            "live_feature_forbidden_sources": [
                "oracle labels",
                "docker outcomes",
                "test outcomes",
                "target-side labels",
                "holdout-selected thresholds",
                "partition labels as scalar signal",
            ],
            "raw_high_cardinality_ids_forbidden": list(RAW_GEOMETRY_TOKENS),
        },
        "split_lineage_policy": {
            "feature_fit_partition": "calibration",
            "reported_partition": "holdout",
            "holdout_used_for_fitting": False,
            "promotion_requires_no_weak_cell_ece_brier_regression": True,
        },
        "missingness_policy": {
            "partial_or_quarantined_slots": "emit missingness_or_quarantine only; never normal density",
            "alias_or_degenerate_spaces": "non-independent diagnostic rows only",
        },
        "allowed_scalar_features": [
            {
                "feature_name": name,
                "feature_family": "geometry_label_quality_numeric",
                "acceptable_value_range": [0.0, 1.0],
                "finite_value_required": True,
                "downstream_consumer_path": "head_projection.scalar_features",
                "consumer_status": "declared_name_only; #662 still controls promotion",
            }
            for name in ALLOWED_GEOMETRY_SCALARS
        ],
        "source_artifacts": source_inventory,
    }


def scalar_rows_for_embedder(
    embedder_id: str,
    *,
    signal: dict[str, Any],
    lift: dict[str, Any],
    eligible_embedders: set[str],
    degenerate_embedders: set[str],
    duplicate_alias_map: dict[str, Any],
) -> list[dict[str, Any]]:
    independent = embedder_id in eligible_embedders
    degenerate = embedder_id in degenerate_embedders
    alias_of = duplicate_alias_map.get(embedder_id)
    total_rows = safe_float(signal.get("total_label_rows"))
    live_rows = safe_float(signal.get("live_prediction_candidate_rows"))
    noisy_rows = safe_float(signal.get("noisy_or_blocked_rows"))
    weak_regression = 1.0 if lift.get("weak_cell_regressions") else 0.0
    promotion_allowed = 1.0 if independent and lift.get("prediction_promotion_allowed") is True else 0.0
    base = {
        "artifact_kind": "geometry_scalar_feature_row",
        "schema_version": SCHEMA_VERSION,
        "formula_version": FORMULA_VERSION,
        "embedder_id": embedder_id,
        "independent_live_input_allowed": independent,
        "space_admissibility": "independent" if independent else ("degenerate" if degenerate else "alias" if alias_of else "unknown"),
        "alias_of_embedder_id": alias_of,
        "prediction_promotion_allowed": bool(promotion_allowed),
        "promotion_policy": "declared scalar name only; #662 blocks live promotion unless out-of-fold gate passes",
    }
    values = {
        "geometry:label_quality_signal_minus_noise": clamp01(safe_float(signal.get("signal_noise_index"))),
        "geometry:label_quality_mean_signal": clamp01(safe_float(signal.get("avg_signal_score"))),
        "geometry:label_quality_mean_noise": clamp01(safe_float(signal.get("avg_noise_score"))),
        "geometry:label_quality_live_label_ratio": clamp01(live_rows / total_rows if total_rows else 0.0),
        "geometry:label_quality_noise_label_ratio": clamp01(noisy_rows / total_rows if total_rows else 1.0),
        "geometry:downstream_weak_cell_regression_flag": weak_regression,
        "geometry:prediction_promotion_allowed": promotion_allowed,
        "geometry:missingness_or_quarantine_flag": 0.0 if independent else 1.0,
        "geometry:boundary_or_ood_score": 1.0 if not independent else clamp01(noisy_rows / total_rows if total_rows else 1.0),
        "geometry:valid_space_consensus_score": 1.0 if independent else 0.0,
    }
    out = []
    for name, value in values.items():
        validate_scalar_name(name)
        require(math.isfinite(value), "GEOMETRY_SCALAR_VALUE_NOT_FINITE", name)
        row = dict(base)
        row.update(
            {
                "feature_name": name,
                "feature_value": value,
                "feature_family": "geometry_label_quality_numeric",
                "leakage_policy": "no oracle/docker/test/target/holdout-selected fields in scalar value",
            }
        )
        out.append(row)
    return out


def select_source_samples(
    path: Path,
    eligible_embedders: set[str],
    degenerate_embedders: set[str],
    duplicate_alias_map: dict[str, Any],
    max_rows: int,
) -> list[dict[str, Any]]:
    samples: list[dict[str, Any]] = []
    seen_classes: set[str] = set()
    for _line_no, row in iter_jsonl(path):
        embedder_id = str(row.get("embedder_id") or "")
        if embedder_id in eligible_embedders:
            sample_class = "independent_source_chain"
        elif embedder_id in degenerate_embedders:
            sample_class = "degenerate_non_live_source_chain"
        elif embedder_id in duplicate_alias_map:
            sample_class = "alias_non_live_source_chain"
        else:
            sample_class = "other_source_chain"
        if sample_class in seen_classes and len(samples) >= max_rows:
            continue
        sample = {
            "artifact_kind": "geometry_scalar_source_sample",
            "schema_version": SCHEMA_VERSION,
            "sample_class": sample_class,
            "source_label_quality_sample": row,
            "scalar_contract_projection": {
                "embedder_id": embedder_id,
                "eligible_for_independent_geometry_scalar": embedder_id in eligible_embedders,
                "alias_of_embedder_id": duplicate_alias_map.get(embedder_id),
                "degenerate": embedder_id in degenerate_embedders,
            },
        }
        samples.append(sample)
        seen_classes.add(sample_class)
        if len(samples) >= max_rows and "independent_source_chain" in seen_classes:
            break
    return samples


def validate_scalar_name(name: str) -> None:
    trimmed = str(name).strip()
    require(bool(trimmed), "GEOMETRY_SCALAR_EMPTY_NAME", "scalar name must be non-empty")
    lower = trimmed.lower()
    if lower.startswith("geometry:"):
        require(
            trimmed in ALLOWED_GEOMETRY_SCALARS,
            "GEOMETRY_SCALAR_UNDECLARED_NAME",
            trimmed,
            {"allowed": list(ALLOWED_GEOMETRY_SCALARS)},
        )
        return
    if any(token in lower for token in RAW_GEOMETRY_TOKENS):
        raise GeometryScalarContractError("GEOMETRY_SCALAR_RAW_IDENTIFIER_FORBIDDEN", trimmed)
    looks_label = any(token in lower for token in ("geometry", "label", "cluster", "region"))
    if looks_label and any(token in lower for token in LEAKY_TOKENS):
        raise GeometryScalarContractError("GEOMETRY_SCALAR_TARGET_LEAK_FORBIDDEN", trimmed)


def name_is_rejected(name: str) -> bool:
    try:
        validate_scalar_name(name)
    except GeometryScalarContractError:
        return True
    return False


def alias_and_degenerate_rows_non_live(path: Path, degenerate: set[str], aliases: set[str]) -> bool:
    if not degenerate and not aliases:
        return True
    checked = 0
    for _line_no, row in iter_jsonl(path):
        embedder_id = str(row.get("embedder_id") or "")
        if embedder_id in degenerate or embedder_id in aliases:
            checked += 1
            if row.get("independent_live_input_allowed") is not False:
                return False
            if row.get("prediction_promotion_allowed") is not False:
                return False
    return checked > 0


def safe_float(value: Any) -> float:
    try:
        out = float(value)
    except (TypeError, ValueError):
        return 0.0
    return out if math.isfinite(out) else 0.0


def clamp01(value: float) -> float:
    if value < 0.0:
        return 0.0
    if value > 1.0:
        return 1.0
    return value


def inventory(path: Path) -> dict[str, Any]:
    require(path.is_file(), "GEOMETRY_SCALAR_INPUT_MISSING", str(path))
    return {
        "path": str(path),
        "bytes": path.stat().st_size,
        "sha256": sha256_file(path),
    }


def add_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--admissibility-root", type=Path, default=DEFAULT_ADMISSIBILITY_ROOT)
    parser.add_argument("--geometry-label-root", type=Path, default=DEFAULT_GEOMETRY_LABEL_ROOT)
    parser.add_argument("--label-quality-root", type=Path, default=DEFAULT_LABEL_QUALITY_ROOT)
    parser.add_argument("--downstream-lift-root", type=Path, default=DEFAULT_DOWNSTREAM_LIFT_ROOT)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--evidence-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--candidate-scalar-name", action="append", default=[])
    parser.add_argument("--max-source-samples", type=int, default=12)
    parser.add_argument("--clean-output", action="store_true", default=False)
