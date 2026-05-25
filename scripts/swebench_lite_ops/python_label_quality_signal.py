"""Rank geometry labels by physical quality and embedder signal/noise."""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any, Iterable

from .core import SwebenchOpsError, sha256_file, utc_now


SCHEMA_VERSION = 1
ARTIFACT_KIND = "label_quality_embedder_signal"
FORMULA_VERSION = "physical_label_quality_signal_v1"

DEFAULT_LABEL_ROOT = Path(
    "/zfs/archive/contextgraph/state/labels/task-py-g-136-per-embedder-geometry-labels-v2-static-ast-v1"
)
DEFAULT_ADMISSIBILITY_ROOT = Path(
    "/zfs/archive/contextgraph/state/labels/task-py-g-143-geometry-label-admissibility-v2-static-ast-v1"
)
DEFAULT_OUTPUT_ROOT = Path(
    "/zfs/archive/contextgraph/state/labels/task-py-g-673-label-quality-embedder-signal-v2-static-ast-v1"
)
DEFAULT_EVIDENCE_ROOT = Path(
    "/zfs/archive/contextgraph/fsv/task-py-g-136-per-embedder-geometry-labels-fsv/v2-static-ast-label-quality-v1"
)

INDEPENDENT_EMBEDDERS = frozenset(("e1", "e6", "e7", "e8", "e9", "e10", "e12", "e13", "e14"))
ALIAS_EMBEDDERS: dict[str, str] = {}
DEGENERATE_EMBEDDERS = frozenset(())
RETIRED_STATIC_AST_EMBEDDERS = frozenset(("e2", "e3", "e4"))
SAFE_LEAKAGE_POLICY = "live_descriptor_excludes_partition_and_mutation_category; audit_fields_not_live_input"
TARGET_LEAKAGE_TOKENS = (
    "oracle",
    "docker",
    "holdout",
    "test_outcome",
    "failure_label",
    "mutation_category_live",
    "partition_live",
)


class LabelQualitySignalError(SwebenchOpsError):
    def __init__(self, code: str, message: str, details: dict[str, Any] | None = None):
        super().__init__(
            code,
            message,
            "Inspect the persisted geometry-label, admissibility, descriptor, and exemplar artifacts; rebuild the source root or repair the quality-audit command before promotion.",
            details,
        )


def require(condition: bool, code: str, message: str, details: dict[str, Any] | None = None) -> None:
    if not condition:
        raise LabelQualitySignalError(code, message, details)


def iter_jsonl(path: Path) -> Iterable[tuple[int, dict[str, Any]]]:
    require(path.is_file(), "LABEL_QUALITY_INPUT_MISSING", str(path))
    with path.open("r", encoding="utf-8") as fh:
        for line_no, line in enumerate(fh, start=1):
            if not line.strip():
                continue
            try:
                row = json.loads(line)
            except json.JSONDecodeError as exc:
                raise LabelQualitySignalError(
                    "LABEL_QUALITY_BAD_JSONL",
                    f"{path}:{line_no}: {exc}",
                    {"path": str(path), "line_no": line_no},
                ) from exc
            require(isinstance(row, dict), "LABEL_QUALITY_ROW_NOT_OBJECT", f"{path}:{line_no}")
            yield line_no, row


def json_sha(value: dict[str, Any]) -> str:
    payload = json.dumps(value, sort_keys=True, separators=(",", ":")).encode("utf-8")
    return "sha256:" + hashlib.sha256(payload).hexdigest()


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
        require(self.handle is not None, "LABEL_QUALITY_SINK_NOT_OPEN", str(self.path))
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


def input_inventory(path: Path, *, optional: bool = False) -> dict[str, Any] | None:
    if not path.exists():
        require(optional, "LABEL_QUALITY_INPUT_MISSING", str(path))
        return None
    require(path.is_file(), "LABEL_QUALITY_INPUT_NOT_FILE", str(path))
    return {"path": str(path), "bytes": path.stat().st_size, "sha256": sha256_file(path)}


def load_json(path: Path) -> dict[str, Any]:
    require(path.is_file(), "LABEL_QUALITY_INPUT_MISSING", str(path))
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise LabelQualitySignalError(
            "LABEL_QUALITY_BAD_JSON",
            f"{path}: {exc}",
            {"path": str(path), "error": str(exc)},
        ) from exc
    require(isinstance(value, dict), "LABEL_QUALITY_JSON_NOT_OBJECT", str(path))
    return value


def optional_bool(row: dict[str, Any], key: str) -> bool | None:
    value = row.get(key)
    return value if isinstance(value, bool) else None


def load_descriptor_index(path: Path, *, max_rows: int | None) -> dict[str, dict[str, Any]]:
    out: dict[str, dict[str, Any]] = {}
    for line_no, row in iter_jsonl(path):
        if max_rows is not None and len(out) >= max_rows:
            break
        label_id = str(row.get("label_id") or "")
        require(label_id, "LABEL_QUALITY_DESCRIPTOR_LABEL_MISSING", f"{path}:{line_no}")
        terms = row.get("descriptor_terms") or []
        ast_surfaces = row.get("ast_surfaces") or {}
        require(isinstance(terms, list), "LABEL_QUALITY_DESCRIPTOR_TERMS_NOT_LIST", f"{path}:{line_no}")
        require(isinstance(ast_surfaces, dict), "LABEL_QUALITY_AST_SURFACES_NOT_OBJECT", f"{path}:{line_no}")
        out[label_id] = {
            "descriptor_hash": row.get("descriptor_hash"),
            "descriptor_term_count": len([item for item in terms if str(item).strip()]),
            "ast_surface_count": len([key for key, value in ast_surfaces.items() if value not in (None, "")]),
            "descriptor_terms_sample": [str(item) for item in terms[:8]],
            "live_input_allowed": optional_bool(row, "live_input_allowed"),
            "independent_live_input_allowed": optional_bool(row, "independent_live_input_allowed"),
            "leakage_policy": row.get("leakage_policy"),
            "source_lineage_policy": row.get("source_lineage_policy"),
            "canonical_ast_lineage_used": optional_bool(row, "canonical_ast_lineage_used"),
            "stability_status": row.get("stability_status"),
        }
    return out


def load_exemplar_index(path: Path, *, max_rows: int | None) -> dict[str, dict[str, Any]]:
    out: dict[str, dict[str, Any]] = {}
    for line_no, row in iter_jsonl(path):
        if max_rows is not None and len(out) >= max_rows:
            break
        label_id = str(row.get("label_id") or "")
        require(label_id, "LABEL_QUALITY_EXEMPLAR_LABEL_MISSING", f"{path}:{line_no}")
        out[label_id] = {
            "exemplar_row_key": row.get("exemplar_row_key"),
            "exemplar_chunk_id": row.get("exemplar_chunk_id"),
            "exemplar_source_sha256": row.get("exemplar_source_sha256"),
            "exemplar_record_ref": row.get("exemplar_record_ref"),
            "exemplar_record_index_ref": row.get("exemplar_record_index_ref"),
            "relative_path": row.get("relative_path"),
            "line_start": row.get("line_start"),
            "line_end": row.get("line_end"),
            "byte_start": row.get("byte_start"),
            "byte_end": row.get("byte_end"),
            "source_text_sha256": row.get("source_text_sha256"),
            "source_text_snippet": row.get("source_text_snippet"),
            "canonical_ast_lineage_used": optional_bool(row, "canonical_ast_lineage_used"),
            "source_lineage_policy": row.get("source_lineage_policy"),
            "live_input_allowed": optional_bool(row, "live_input_allowed"),
            "independent_live_input_allowed": optional_bool(row, "independent_live_input_allowed"),
            "leakage_policy": row.get("leakage_policy"),
        }
    return out


def load_admissibility_index(path: Path, *, max_rows: int | None) -> dict[str, dict[str, Any]]:
    out: dict[str, dict[str, Any]] = {}
    for line_no, row in iter_jsonl(path):
        if max_rows is not None and len(out) >= max_rows:
            break
        label_id = str(row.get("label_id") or "")
        require(label_id, "LABEL_QUALITY_ADMISSIBILITY_LABEL_MISSING", f"{path}:{line_no}")
        out[label_id] = {
            "space_admissibility": row.get("space_admissibility"),
            "alias_of_embedder_id": row.get("alias_of_embedder_id"),
            "independent_live_input_allowed": bool(row.get("independent_live_input_allowed")),
            "admissibility_reason": row.get("admissibility_reason"),
            "source_live_input_allowed": bool(row.get("source_live_input_allowed")),
            "source_label_status": row.get("source_label_status"),
            "leakage_policy": row.get("leakage_policy"),
        }
    return out


def source_space(embedder_id: str, row: dict[str, Any], admissibility: dict[str, Any] | None) -> tuple[str, str | None]:
    if admissibility:
        return str(admissibility.get("space_admissibility") or ""), admissibility.get("alias_of_embedder_id")
    if embedder_id in INDEPENDENT_EMBEDDERS:
        return "independent", None
    if embedder_id in ALIAS_EMBEDDERS:
        return "alias", ALIAS_EMBEDDERS[embedder_id]
    if embedder_id in DEGENERATE_EMBEDDERS:
        return "degenerate", None
    return str(row.get("space_admissibility") or "unknown"), row.get("alias_of_embedder_id")


def support_band(support_count: int) -> str:
    if support_count <= 0:
        return "none"
    if support_count == 1:
        return "singleton"
    if support_count <= 3:
        return "tiny_2_3"
    if support_count <= 9:
        return "small_4_9"
    if support_count <= 49:
        return "stable_10_49"
    return "large_50_plus"


def safe_leakage_policy(policy: Any) -> bool:
    raw = str(policy or "")
    lowered = raw.lower()
    if raw != SAFE_LEAKAGE_POLICY:
        return False
    return not any(token in lowered for token in TARGET_LEAKAGE_TOKENS)


def physical_score(
    *,
    descriptor: dict[str, Any] | None,
    exemplar: dict[str, Any] | None,
    admissibility: dict[str, Any] | None,
    registry: dict[str, Any],
) -> float:
    points = 0
    total = 7
    if admissibility is not None:
        points += 1
    if descriptor is not None:
        points += 1
    if exemplar is not None:
        points += 1
    if registry.get("vector_sha256"):
        points += 1
    if exemplar and exemplar.get("exemplar_chunk_id"):
        points += 1
    if exemplar and (exemplar.get("exemplar_record_ref") or exemplar.get("exemplar_record_index_ref")):
        points += 1
    if exemplar and exemplar.get("exemplar_source_sha256"):
        points += 1
    return points / total


def interpretability_score(descriptor: dict[str, Any] | None, exemplar: dict[str, Any] | None) -> float:
    if descriptor is None:
        return 0.0
    term_count = int(descriptor.get("descriptor_term_count") or 0)
    ast_surface_count = int(descriptor.get("ast_surface_count") or 0)
    snippet_bonus = 1 if exemplar and exemplar.get("source_text_snippet") else 0
    return min(1.0, (min(term_count, 6) / 6.0) * 0.60 + (min(ast_surface_count, 4) / 4.0) * 0.30 + snippet_bonus * 0.10)


def stability_score(label_status: str, stability_status: str, support_count: int) -> float:
    support = support_band(support_count)
    if label_status == "accepted" or support in {"stable_10_49", "large_50_plus"}:
        return 1.0
    if support in {"small_4_9", "tiny_2_3"}:
        return 0.62
    if support == "singleton":
        return 0.42 if "singleton" in stability_status else 0.35
    return 0.0


def classify_label(
    *,
    registry: dict[str, Any],
    descriptor: dict[str, Any] | None,
    exemplar: dict[str, Any] | None,
    admissibility: dict[str, Any] | None,
) -> dict[str, Any]:
    embedder_id = str(registry.get("embedder_id") or "")
    label_status = str(registry.get("label_status") or "")
    stability = str(registry.get("stability_status") or "")
    try:
        support_count = int(registry.get("support_count") or 0)
    except (TypeError, ValueError):
        support_count = 0
    space, alias_target = source_space(embedder_id, registry, admissibility)
    independent_live = bool(admissibility.get("independent_live_input_allowed")) if admissibility else bool(
        registry.get("independent_live_input_allowed")
    )
    leakage_ok = safe_leakage_policy(registry.get("leakage_policy")) and (
        descriptor is None or safe_leakage_policy(descriptor.get("leakage_policy"))
    )
    source_chain_score = physical_score(
        descriptor=descriptor,
        exemplar=exemplar,
        admissibility=admissibility,
        registry=registry,
    )
    meaning_score = interpretability_score(descriptor, exemplar)
    stability = str(descriptor.get("stability_status") or stability) if descriptor else stability
    stable_score = stability_score(label_status, stability, support_count)

    reasons: list[str] = []
    if admissibility is None:
        reasons.append("missing_admissibility_row")
    if descriptor is None:
        reasons.append("missing_descriptor_row")
    if exemplar is None:
        reasons.append("missing_exemplar_row")
    if not leakage_ok:
        reasons.append("leakage_policy_not_promotion_safe")
    if space == "alias":
        reasons.append("duplicate_alias_space_not_independent")
    if space == "degenerate":
        reasons.append("degenerate_space_not_independent")
    if label_status == "rejected":
        reasons.append("source_label_rejected")
    if source_chain_score < 1.0:
        reasons.append("source_chain_incomplete")
    if meaning_score < 0.35:
        reasons.append("low_descriptor_interpretability")

    if label_status == "rejected" or space == "degenerate":
        quality_class = "rejected_noise"
        live_use_allowed = False
    elif space == "alias":
        quality_class = "redundant_alias_noise"
        live_use_allowed = False
    elif not independent_live:
        quality_class = "diagnostic_only_nonindependent"
        live_use_allowed = False
    elif not leakage_ok:
        quality_class = "leakage_blocked"
        live_use_allowed = False
    elif descriptor is None or exemplar is None or admissibility is None:
        quality_class = "low_quality_missing_source_chain"
        live_use_allowed = False
    elif support_count <= 3 and source_chain_score >= 0.95 and meaning_score >= 0.55:
        quality_class = "tiny_high_value_provisional"
        live_use_allowed = True
    elif label_status == "accepted" and stable_score >= 0.95 and meaning_score >= 0.55:
        quality_class = "high_quality_accepted"
        live_use_allowed = True
    elif source_chain_score >= 0.95 and meaning_score >= 0.45:
        quality_class = "usable_signal_provisional"
        live_use_allowed = True
    else:
        quality_class = "low_quality_interpretability_or_stability"
        reasons.append("insufficient_interpretability_or_stability_for_live_promotion")
        live_use_allowed = False

    signal_score = 0.0
    if live_use_allowed:
        signal_score = round(
            0.35 * source_chain_score + 0.25 * meaning_score + 0.25 * stable_score + 0.15 * float(leakage_ok),
            6,
        )
    noise_score = 0.0
    if not live_use_allowed:
        noise_score += 0.35
    if space in {"alias", "degenerate"}:
        noise_score += 0.35
    if label_status == "rejected":
        noise_score += 0.30
    if source_chain_score < 1.0:
        noise_score += (1.0 - source_chain_score) * 0.20
    if meaning_score < 0.35:
        noise_score += 0.15
    if not leakage_ok:
        noise_score += 0.60
    noise_score = round(min(1.0, noise_score), 6)

    return {
        "quality_class": quality_class,
        "quality_reasons": reasons,
        "space_admissibility": space,
        "alias_of_embedder_id": alias_target,
        "independent_live_input_allowed": independent_live,
        "live_prediction_feature_allowed": live_use_allowed,
        "search_region_allowed": quality_class not in {"rejected_noise", "leakage_blocked"},
        "support_band": support_band(support_count),
        "physical_validity_score": round(source_chain_score, 6),
        "interpretability_score": round(meaning_score, 6),
        "stability_score": round(stable_score, 6),
        "leakage_safety_score": 1.0 if leakage_ok else 0.0,
        "signal_score": signal_score,
        "noise_score": noise_score,
    }


def update_summary(summary: dict[str, Any], score_row: dict[str, Any]) -> None:
    summary["total_label_rows"] += 1
    summary["signal_score_sum"] += float(score_row["signal_score"])
    summary["noise_score_sum"] += float(score_row["noise_score"])
    quality_class = str(score_row["quality_class"])
    summary["quality_classes"][quality_class] += 1
    summary["support_bands"][str(score_row["support_band"])] += 1
    if score_row["independent_live_input_allowed"]:
        summary["independent_admissible_rows"] += 1
    if score_row["live_prediction_feature_allowed"]:
        summary["live_prediction_candidate_rows"] += 1
    if quality_class in {"high_quality_accepted", "tiny_high_value_provisional", "usable_signal_provisional"}:
        summary["usable_signal_rows"] += 1
    if quality_class in {
        "rejected_noise",
        "redundant_alias_noise",
        "low_quality_missing_source_chain",
        "low_quality_interpretability_or_stability",
        "leakage_blocked",
    }:
        summary["noisy_or_blocked_rows"] += 1
    if str(score_row["space_admissibility"]) == "alias":
        summary["alias_rows"] += 1
    if str(score_row["space_admissibility"]) == "degenerate":
        summary["degenerate_rows"] += 1
    if "missing_descriptor_row" in score_row["quality_reasons"]:
        summary["missing_descriptor_rows"] += 1
    if "missing_exemplar_row" in score_row["quality_reasons"]:
        summary["missing_exemplar_rows"] += 1
    if "missing_admissibility_row" in score_row["quality_reasons"]:
        summary["missing_admissibility_rows"] += 1


def finalize_summary(embedder_id: str, raw: dict[str, Any]) -> dict[str, Any]:
    total = int(raw["total_label_rows"])
    signal_rows = int(raw["usable_signal_rows"])
    live_rows = int(raw["live_prediction_candidate_rows"])
    noise_rows = int(raw["noisy_or_blocked_rows"])
    avg_signal = float(raw["signal_score_sum"]) / total if total else 0.0
    avg_noise = float(raw["noise_score_sum"]) / total if total else 0.0
    signal_yield = signal_rows / total if total else 0.0
    noise_ratio = noise_rows / total if total else 0.0
    signal_noise_index = round((avg_signal * 0.55 + signal_yield * 0.35) - (avg_noise * 0.10), 6)
    if embedder_id in DEGENERATE_EMBEDDERS:
        action = "retire_or_repair_degenerate_space_before_prediction"
    elif embedder_id in RETIRED_STATIC_AST_EMBEDDERS:
        action = "retired_for_static_ast_not_expected_in_label_source"
    elif embedder_id in ALIAS_EMBEDDERS:
        action = "exclude_as_independent_signal; use_only_for_redundancy_diagnostic"
    elif live_rows == 0:
        action = "block_prediction_promotion_until_source_chain_or_leakage_is_repaired"
    else:
        action = "eligible_for_search_and_candidate_feature_gate; prediction_promotion_requires_out_of_fold_lift"
    return {
        "embedder_id": embedder_id,
        "total_label_rows": total,
        "independent_admissible_rows": int(raw["independent_admissible_rows"]),
        "live_prediction_candidate_rows": live_rows,
        "usable_signal_rows": signal_rows,
        "noisy_or_blocked_rows": noise_rows,
        "alias_rows": int(raw["alias_rows"]),
        "degenerate_rows": int(raw["degenerate_rows"]),
        "missing_descriptor_rows": int(raw["missing_descriptor_rows"]),
        "missing_exemplar_rows": int(raw["missing_exemplar_rows"]),
        "missing_admissibility_rows": int(raw["missing_admissibility_rows"]),
        "quality_classes": dict(sorted(raw["quality_classes"].items())),
        "support_bands": dict(sorted(raw["support_bands"].items())),
        "avg_signal_score": round(avg_signal, 6),
        "avg_noise_score": round(avg_noise, 6),
        "signal_yield": round(signal_yield, 6),
        "noise_ratio": round(noise_ratio, 6),
        "signal_noise_index": signal_noise_index,
        "recommended_action": action,
    }


def maybe_sample(samples: dict[tuple[str, str], int], sink: JsonlSink, row: dict[str, Any], *, limit: int) -> None:
    key = (str(row["embedder_id"]), str(row["quality_class"]))
    if samples[key] >= limit:
        return
    samples[key] += 1
    sink.write(
        {
            "artifact_kind": "label_quality_source_sample",
            "schema_version": SCHEMA_VERSION,
            "formula_version": FORMULA_VERSION,
            "sample_key": f"{row['embedder_id']}:{row['quality_class']}:{samples[key]}",
            "label_id": row["label_id"],
            "label_name": row["label_name"],
            "embedder_id": row["embedder_id"],
            "quality_class": row["quality_class"],
            "quality_reasons": row["quality_reasons"],
            "registry_source": row["source_chain"]["registry"],
            "admissibility_source": row["source_chain"]["admissibility"],
            "descriptor_source": row["source_chain"]["descriptor"],
            "exemplar_source": row["source_chain"]["exemplar"],
            "packed_vector_evidence": {
                "vector_sha256": row["vector_sha256"],
                "record_ref": row["source_chain"]["exemplar"].get("exemplar_record_ref")
                if row["source_chain"].get("exemplar")
                else None,
            },
            "ast_chunk_evidence": {
                "chunk_id": row["source_chain"]["exemplar"].get("exemplar_chunk_id")
                if row["source_chain"].get("exemplar")
                else None,
                "source_sha256": row["source_chain"]["exemplar"].get("exemplar_source_sha256")
                if row["source_chain"].get("exemplar")
                else None,
                "relative_path": row["source_chain"]["exemplar"].get("relative_path")
                if row["source_chain"].get("exemplar")
                else None,
                "line_start": row["source_chain"]["exemplar"].get("line_start")
                if row["source_chain"].get("exemplar")
                else None,
                "line_end": row["source_chain"]["exemplar"].get("line_end")
                if row["source_chain"].get("exemplar")
                else None,
            },
        }
    )


def command_python_label_quality_signal(args: argparse.Namespace) -> dict[str, Any]:
    started_at = utc_now()
    require(args.max_registry_rows is None or args.max_registry_rows > 0, "LABEL_QUALITY_BAD_MAX_REGISTRY_ROWS", str(args.max_registry_rows))
    require(args.sample_limit_per_class >= 0, "LABEL_QUALITY_BAD_SAMPLE_LIMIT", str(args.sample_limit_per_class))

    registry_path = args.label_root / "embedder_geometry_label_registry.jsonl"
    descriptor_path = args.label_root / "embedder_geometry_label_descriptors.jsonl"
    exemplar_path = args.label_root / "embedder_geometry_label_exemplars.jsonl"
    rejection_path = args.label_root / "embedder_geometry_label_rejections.jsonl"
    label_manifest_path = args.label_root / "embedder_geometry_label_manifest.json"
    derived_label_path = args.label_root / "embedder_geometry_derived_region_labels.jsonl"
    admissibility_path = args.admissibility_root / "geometry_label_admissibility.jsonl"
    health_rejection_path = args.admissibility_root / "geometry_label_health_rejections.jsonl"
    admissibility_manifest_path = args.admissibility_root / "geometry_label_admissibility_manifest.json"
    policy_path = args.admissibility_root / "geometry_label_space_policy.json"

    for path in (
        registry_path,
        descriptor_path,
        exemplar_path,
        rejection_path,
        label_manifest_path,
        admissibility_path,
        health_rejection_path,
        admissibility_manifest_path,
        policy_path,
    ):
        require(path.is_file(), "LABEL_QUALITY_INPUT_MISSING", str(path))

    before = {
        "output_root_exists": args.output_root.exists(),
        "output_root_files": sorted(path.name for path in args.output_root.iterdir()) if args.output_root.exists() else [],
    }
    if args.clean_output and args.output_root.exists():
        shutil.rmtree(args.output_root)
    args.output_root.mkdir(parents=True, exist_ok=True)

    descriptor_index = load_descriptor_index(descriptor_path, max_rows=args.max_registry_rows)
    exemplar_index = load_exemplar_index(exemplar_path, max_rows=args.max_registry_rows)
    admissibility_index = load_admissibility_index(admissibility_path, max_rows=args.max_registry_rows)
    label_manifest = load_json(label_manifest_path)
    admissibility_manifest = load_json(admissibility_manifest_path)
    policy = load_json(policy_path)

    stats: Counter[str] = Counter()
    summaries: dict[str, dict[str, Any]] = defaultdict(
        lambda: {
            "total_label_rows": 0,
            "independent_admissible_rows": 0,
            "live_prediction_candidate_rows": 0,
            "usable_signal_rows": 0,
            "noisy_or_blocked_rows": 0,
            "alias_rows": 0,
            "degenerate_rows": 0,
            "missing_descriptor_rows": 0,
            "missing_exemplar_rows": 0,
            "missing_admissibility_rows": 0,
            "signal_score_sum": 0.0,
            "noise_score_sum": 0.0,
            "quality_classes": Counter(),
            "support_bands": Counter(),
        }
    )
    sample_counts: dict[tuple[str, str], int] = defaultdict(int)

    scores_path = args.output_root / "label_quality_scores.jsonl"
    samples_path = args.output_root / "label_quality_source_samples.jsonl"
    with JsonlSink(scores_path) as score_sink, JsonlSink(samples_path) as sample_sink:
        for line_no, registry in iter_jsonl(registry_path):
            if args.max_registry_rows is not None and stats["registry_rows_seen"] >= args.max_registry_rows:
                break
            label_id = str(registry.get("label_id") or "")
            embedder_id = str(registry.get("embedder_id") or "")
            require(label_id and embedder_id, "LABEL_QUALITY_REGISTRY_KEY_MISSING", f"{registry_path}:{line_no}")
            descriptor = descriptor_index.get(label_id)
            exemplar = exemplar_index.get(label_id)
            admissibility = admissibility_index.get(label_id)
            quality = classify_label(
                registry=registry,
                descriptor=descriptor,
                exemplar=exemplar,
                admissibility=admissibility,
            )
            row = {
                "artifact_kind": "label_quality_score",
                "schema_version": SCHEMA_VERSION,
                "formula_version": FORMULA_VERSION,
                "label_id": label_id,
                "label_name": registry.get("label_name"),
                "embedder_id": embedder_id,
                "cluster_id": registry.get("cluster_id"),
                "vector_sha256": registry.get("vector_sha256"),
                "support_count": registry.get("support_count"),
                "source_label_status": registry.get("label_status"),
                "source_stability_status": registry.get("stability_status"),
                "descriptor_term_count": descriptor.get("descriptor_term_count") if descriptor else 0,
                "ast_surface_count": descriptor.get("ast_surface_count") if descriptor else 0,
                "descriptor_terms_sample": descriptor.get("descriptor_terms_sample") if descriptor else [],
                "source_chain": {
                    "registry": {
                        "path": str(registry_path),
                        "line_no": line_no,
                        "row_sha256": json_sha(registry),
                    },
                    "admissibility": admissibility,
                    "descriptor": descriptor,
                    "exemplar": exemplar,
                },
                **quality,
            }
            score_sink.write(row)
            stats["registry_rows_seen"] += 1
            stats[f"quality_class:{row['quality_class']}"] += 1
            stats[f"embedder:{embedder_id}"] += 1
            update_summary(summaries[embedder_id], row)
            if args.sample_limit_per_class > 0:
                maybe_sample(sample_counts, sample_sink, row, limit=args.sample_limit_per_class)
    score_artifact = score_sink.artifact()
    sample_artifact = sample_sink.artifact()

    finalized = [finalize_summary(embedder_id, raw) for embedder_id, raw in sorted(summaries.items())]
    ranked = sorted(finalized, key=lambda row: (-float(row["signal_noise_index"]), -int(row["usable_signal_rows"]), row["embedder_id"]))
    summary = {
        "artifact_kind": "embedder_signal_noise_summary",
        "schema_version": SCHEMA_VERSION,
        "formula_version": FORMULA_VERSION,
        "created_at_utc": started_at,
        "ranking_policy": "rank by signal_noise_index, then usable signal rows; degenerate and alias spaces are excluded from independent signal",
        "prediction_promotion_allowed": False,
        "prediction_promotion_blocker": "out_of_fold_weak_cell_lift_and_ece_brier_deltas_not_computed_in_this_structural_quality_slice",
        "by_embedder_ranked": ranked,
        "counts": dict(sorted(stats.items())),
    }
    summary_sha = atomic_json_dump(args.output_root / "embedder_signal_noise_summary.json", summary)

    downstream_path = args.output_root / "label_quality_downstream_lift.jsonl"
    with JsonlSink(downstream_path) as downstream_sink:
        for row in ranked:
            downstream_sink.write(
                {
                    "artifact_kind": "label_quality_downstream_lift_status",
                    "schema_version": SCHEMA_VERSION,
                    "formula_version": FORMULA_VERSION,
                    "embedder_id": row["embedder_id"],
                    "out_of_fold_lift_status": "not_computed_in_structural_quality_slice",
                    "diagnostic_only": True,
                    "prediction_promotion_allowed": False,
                    "required_next_measurements": [
                        "weak_cell_out_of_fold_association",
                        "abstain_pool_ece_delta",
                        "brier_delta",
                        "per_cell_regression_check",
                    ],
                }
            )
    downstream_artifact = downstream_sink.artifact()

    checks = [
        {
            "check": "score_rows_match_registry_denominator",
            "passed": score_artifact["rows"] == stats["registry_rows_seen"],
            "actual": score_artifact["rows"],
            "expected": stats["registry_rows_seen"],
        },
        {
            "check": "alias_embedder_excluded_from_live_prediction_signal",
            "passed": not ALIAS_EMBEDDERS
            or all(row["live_prediction_candidate_rows"] == 0 for row in finalized if row["embedder_id"] in ALIAS_EMBEDDERS),
            "actual": [row for row in finalized if row["embedder_id"] in ALIAS_EMBEDDERS],
            "expected": "alias rows may exist but must have zero live prediction candidates; static AST v2 has no e8 alias",
        },
        {
            "check": "degenerate_embedders_excluded_from_live_prediction_signal",
            "passed": not DEGENERATE_EMBEDDERS
            or all(row["live_prediction_candidate_rows"] == 0 for row in finalized if row["embedder_id"] in DEGENERATE_EMBEDDERS),
            "actual": [row for row in finalized if row["embedder_id"] in DEGENERATE_EMBEDDERS],
            "expected": "configured degenerate embedders must have zero live prediction candidates; e2/e3/e4 are retired before static AST v2 labeling",
        },
        {
            "check": "singleton_or_tiny_labels_retained_when_physically_grounded",
            "passed": any(
                (row["quality_classes"].get("tiny_high_value_provisional") or 0) > 0 for row in finalized
            ),
            "actual": {
                row["embedder_id"]: row["quality_classes"].get("tiny_high_value_provisional", 0) for row in finalized
            },
            "expected": "at least one independent tiny/singleton label is retained as high-value provisional",
        },
        {
            "check": "independent_embedders_have_quality_rows",
            "passed": all(any(row["embedder_id"] == embedder_id for row in finalized) for embedder_id in INDEPENDENT_EMBEDDERS),
            "actual": sorted(row["embedder_id"] for row in finalized),
            "expected": sorted(INDEPENDENT_EMBEDDERS),
        },
        {
            "check": "samples_written_for_manual_source_chain_fsv",
            "passed": sample_artifact["rows"] > 0,
            "actual": sample_artifact["rows"],
            "expected": ">0",
        },
        {
            "check": "downstream_promotion_explicitly_blocked_until_lift",
            "passed": summary["prediction_promotion_allowed"] is False and downstream_artifact["rows"] == len(ranked),
            "actual": {"prediction_promotion_allowed": summary["prediction_promotion_allowed"], "rows": downstream_artifact["rows"]},
            "expected": "structural label quality can inform search/audit, but predictor promotion remains blocked until out-of-fold lift is measured",
        },
    ]
    all_passed = all(bool(check["passed"]) for check in checks)
    require(all_passed, "LABEL_QUALITY_CHECK_FAILED", str(checks))

    outputs = {
        "label_quality_scores": score_artifact,
        "label_quality_source_samples": sample_artifact,
        "embedder_signal_noise_summary": {
            "path": str(args.output_root / "embedder_signal_noise_summary.json"),
            "bytes": (args.output_root / "embedder_signal_noise_summary.json").stat().st_size,
            "sha256": summary_sha,
        },
        "label_quality_downstream_lift": downstream_artifact,
    }
    manifest = {
        "artifact_kind": "label_quality_manifest",
        "schema_version": SCHEMA_VERSION,
        "formula_version": FORMULA_VERSION,
        "created_at_utc": started_at,
        "all_passed": True,
        "acceptance_status": "structural_label_quality_audit_passed; downstream_lift_pending_before_prediction_promotion",
        "source_of_truth": {
            "label_root": str(args.label_root),
            "admissibility_root": str(args.admissibility_root),
            "registry": input_inventory(registry_path),
            "descriptors": input_inventory(descriptor_path),
            "exemplars": input_inventory(exemplar_path),
            "rejections": input_inventory(rejection_path),
            "label_manifest": input_inventory(label_manifest_path),
            "derived_region_labels": input_inventory(derived_label_path, optional=True),
            "admissibility": input_inventory(admissibility_path),
            "health_rejections": input_inventory(health_rejection_path),
            "admissibility_manifest": input_inventory(admissibility_manifest_path),
            "space_policy": input_inventory(policy_path),
        },
        "source_manifest_facts": {
            "label_manifest_counts": label_manifest.get("counts"),
            "label_manifest_status_counts": label_manifest.get("status_counts"),
            "admissibility_manifest_all_passed": admissibility_manifest.get("all_passed"),
            "policy_valid_independent_embedder_count": policy.get("valid_independent_embedder_count"),
            "policy_valid_independent_pair_denominator": policy.get("valid_independent_pair_denominator"),
        },
        "output_root": str(args.output_root),
        "before": before,
        "parameters": {
            "max_registry_rows": args.max_registry_rows,
            "sample_limit_per_class": args.sample_limit_per_class,
            "safe_leakage_policy": SAFE_LEAKAGE_POLICY,
            "independent_embedders": sorted(INDEPENDENT_EMBEDDERS),
            "alias_embedders": dict(sorted(ALIAS_EMBEDDERS.items())),
            "degenerate_embedders": sorted(DEGENERATE_EMBEDDERS),
            "retired_static_ast_embedders": sorted(RETIRED_STATIC_AST_EMBEDDERS),
        },
        "counts": {
            **dict(sorted(stats.items())),
            "descriptor_index_rows": len(descriptor_index),
            "exemplar_index_rows": len(exemplar_index),
            "admissibility_index_rows": len(admissibility_index),
        },
        "outputs": outputs,
        "checks": checks,
    }
    manifest_sha = atomic_json_dump(args.output_root / "label_quality_manifest.json", manifest)
    manifest["outputs"]["label_quality_manifest"] = {
        "path": str(args.output_root / "label_quality_manifest.json"),
        "bytes": (args.output_root / "label_quality_manifest.json").stat().st_size,
        "sha256": manifest_sha,
    }
    return manifest


def add_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--label-root", type=Path, default=DEFAULT_LABEL_ROOT)
    parser.add_argument("--admissibility-root", type=Path, default=DEFAULT_ADMISSIBILITY_ROOT)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--evidence-root", type=Path, default=DEFAULT_EVIDENCE_ROOT)
    parser.add_argument("--max-registry-rows", type=int, default=None)
    parser.add_argument("--sample-limit-per-class", type=int, default=2)
    parser.add_argument("--clean-output", action="store_true")
