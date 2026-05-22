"""Fail-closed admissibility overlay for per-embedder geometry labels."""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
from collections import Counter
from pathlib import Path
from typing import Any, Iterable

from .core import SwebenchOpsError, utc_now


SCHEMA_VERSION = 1
ARTIFACT_KIND = "geometry_label_admissibility"
FORMULA_VERSION = "valid_independent_embedder_policy_v1"

DEFAULT_LABEL_ROOT = Path("/zfs/archive/contextgraph/state/labels/task-py-g-136-per-embedder-geometry-labels-v1")
DEFAULT_OUTPUT_ROOT = Path("/zfs/archive/contextgraph/state/labels/task-py-g-143-geometry-label-admissibility-v1")
DEFAULT_EVIDENCE_ROOT = Path("/zfs/archive/contextgraph/readback/task-py-g-143-geometry-label-admissibility-readback")
DEFAULT_ELIGIBLE_INDEPENDENT = "e1,e6,e7,e9,e10,e12,e13,e14"
DEFAULT_ALIAS_MAP = "e8:e1"
DEFAULT_DEGENERATE = "e2,e3,e4"


class GeometryLabelAdmissibilityError(SwebenchOpsError):
    def __init__(self, code: str, message: str, details: dict[str, Any] | None = None):
        super().__init__(
            code,
            message,
            "Inspect the source geometry-label root, vector-space-health policy, and alias map; rebuild or scrub the label artifacts before downstream consumption.",
            details,
        )


def require(condition: bool, code: str, message: str, details: dict[str, Any] | None = None) -> None:
    if not condition:
        raise GeometryLabelAdmissibilityError(code, message, details)


def parse_csv(raw: str) -> tuple[str, ...]:
    values = tuple(item.strip() for item in raw.split(",") if item.strip())
    require(len(values) == len(set(values)), "GEOMETRY_LABEL_DUPLICATE_EMBEDDER", raw)
    return values


def parse_alias_map(raw: str) -> dict[str, str]:
    aliases: dict[str, str] = {}
    if not raw.strip():
        return aliases
    for item in raw.split(","):
        if not item.strip():
            continue
        parts = [part.strip() for part in item.split(":", 1)]
        require(len(parts) == 2 and all(parts), "GEOMETRY_LABEL_BAD_ALIAS_MAP", item)
        alias, target = parts
        require(alias not in aliases, "GEOMETRY_LABEL_DUPLICATE_ALIAS", alias)
        aliases[alias] = target
    return aliases


def iter_jsonl(path: Path) -> Iterable[tuple[int, dict[str, Any]]]:
    require(path.is_file(), "GEOMETRY_LABEL_INPUT_MISSING", str(path))
    with path.open("r", encoding="utf-8") as fh:
        for line_no, line in enumerate(fh, start=1):
            if not line.strip():
                continue
            try:
                row = json.loads(line)
            except json.JSONDecodeError as exc:
                raise GeometryLabelAdmissibilityError(
                    "GEOMETRY_LABEL_BAD_JSONL",
                    f"{path}:{line_no}: {exc}",
                    {"path": str(path), "line_no": line_no},
                ) from exc
            require(isinstance(row, dict), "GEOMETRY_LABEL_ROW_NOT_OBJECT", f"{path}:{line_no}")
            yield line_no, row


def atomic_json_dump(path: Path, value: dict[str, Any]) -> str:
    path.parent.mkdir(parents=True, exist_ok=True)
    payload = json.dumps(value, sort_keys=True, indent=2, separators=(",", ": ")) + "\n"
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_text(payload, encoding="utf-8")
    tmp.replace(path)
    return "sha256:" + hashlib.sha256(payload.encode("utf-8")).hexdigest()


def stream_jsonl(path: Path, rows: Iterable[dict[str, Any]]) -> dict[str, Any]:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    digest = hashlib.sha256()
    count = 0
    with tmp.open("w", encoding="utf-8") as fh:
        for row in rows:
            encoded = json.dumps(row, sort_keys=True, separators=(",", ":")).encode("utf-8")
            fh.write(encoded.decode("utf-8"))
            fh.write("\n")
            digest.update(encoded)
            digest.update(b"\n")
            count += 1
    tmp.replace(path)
    return {"path": str(path), "rows": count, "sha256": "sha256:" + digest.hexdigest(), "bytes": path.stat().st_size}


def input_inventory(path: Path) -> dict[str, Any]:
    require(path.is_file(), "GEOMETRY_LABEL_INPUT_MISSING", str(path))
    return {"path": str(path), "bytes": path.stat().st_size}


def source_kind(embedder_id: str, eligible: set[str], aliases: dict[str, str], degenerate: set[str]) -> tuple[str, str | None]:
    if embedder_id in eligible:
        return "independent", None
    if embedder_id in aliases:
        return "alias", aliases[embedder_id]
    if embedder_id in degenerate:
        return "degenerate", None
    return "unknown", None


def scrub_registry_rows(
    registry_path: Path,
    *,
    eligible: set[str],
    aliases: dict[str, str],
    degenerate: set[str],
    max_rows: int | None,
    stats: Counter[str],
    embedder_counts: Counter[str],
) -> Iterable[dict[str, Any]]:
    for line_no, row in iter_jsonl(registry_path):
        if max_rows is not None and stats["registry_rows_seen"] >= max_rows:
            break
        embedder_id = str(row.get("embedder_id") or "")
        label_id = str(row.get("label_id") or "")
        require(embedder_id and label_id, "GEOMETRY_LABEL_REGISTRY_KEY_MISSING", f"{registry_path}:{line_no}")
        kind, alias_target = source_kind(embedder_id, eligible, aliases, degenerate)
        require(kind != "unknown", "GEOMETRY_LABEL_UNKNOWN_EMBEDDER", f"{registry_path}:{line_no}:{embedder_id}")
        source_live = bool(row.get("live_input_allowed"))
        source_status = str(row.get("label_status") or "")
        independent_live = kind == "independent" and source_live and source_status != "rejected"
        if kind == "alias" and source_live:
            stats["alias_source_live_rows"] += 1
        if kind == "degenerate" and source_live:
            stats["degenerate_source_live_rows"] += 1
        stats["registry_rows_seen"] += 1
        stats[f"{kind}_rows"] += 1
        stats[f"source_status:{source_status or 'missing'}"] += 1
        if independent_live:
            stats["independent_live_rows"] += 1
        else:
            stats["non_independent_or_non_live_rows"] += 1
        embedder_counts[embedder_id] += 1
        yield {
            "artifact_kind": f"{ARTIFACT_KIND}_row",
            "schema_version": SCHEMA_VERSION,
            "formula_version": FORMULA_VERSION,
            "label_id": label_id,
            "label_name": row.get("label_name"),
            "cluster_id": row.get("cluster_id"),
            "embedder_id": embedder_id,
            "vector_sha256": row.get("vector_sha256"),
            "support_count": row.get("support_count"),
            "source_label_status": source_status,
            "source_live_input_allowed": source_live,
            "source_duplicate_space_alias_of": row.get("duplicate_space_alias_of"),
            "space_admissibility": kind,
            "alias_of_embedder_id": alias_target,
            "independent_live_input_allowed": independent_live,
            "admissibility_reason": (
                "eligible_independent_space"
                if kind == "independent"
                else "duplicate_alias_space_not_independent"
                if kind == "alias"
                else "degenerate_space_not_independent"
            ),
            "leakage_policy": row.get("leakage_policy"),
            "parameter_hash": row.get("parameter_hash"),
        }


def rejection_rows(
    rejection_path: Path,
    *,
    degenerate: set[str],
    max_rows: int | None,
    stats: Counter[str],
) -> Iterable[dict[str, Any]]:
    for line_no, row in iter_jsonl(rejection_path):
        if max_rows is not None and stats["rejection_rows_seen"] >= max_rows:
            break
        embedder_id = str(row.get("embedder_id") or "")
        require(embedder_id, "GEOMETRY_LABEL_REJECTION_EMBEDDER_MISSING", f"{rejection_path}:{line_no}")
        require(embedder_id in degenerate, "GEOMETRY_LABEL_REJECTION_UNEXPECTED_EMBEDDER", f"{rejection_path}:{line_no}:{embedder_id}")
        stats["rejection_rows_seen"] += 1
        stats[f"rejection_embedder:{embedder_id}"] += 1
        out = dict(row)
        out["artifact_kind"] = f"{ARTIFACT_KIND}_health_rejection"
        out["schema_version"] = SCHEMA_VERSION
        out["formula_version"] = FORMULA_VERSION
        out["independent_live_input_allowed"] = False
        out["admissibility_reason"] = "degenerate_space_not_independent"
        yield out


def command_python_geometry_label_admissibility(args: argparse.Namespace) -> dict[str, Any]:
    started_at = utc_now()
    eligible = set(parse_csv(args.eligible_independent_embedders))
    aliases = parse_alias_map(args.alias_map)
    degenerate = set(parse_csv(args.degenerate_embedders))
    require(not (eligible & set(aliases)), "GEOMETRY_LABEL_ALIAS_MARKED_INDEPENDENT", str(sorted(eligible & set(aliases))))
    require(not (eligible & degenerate), "GEOMETRY_LABEL_DEGENERATE_MARKED_INDEPENDENT", str(sorted(eligible & degenerate)))
    require(not (set(aliases) & degenerate), "GEOMETRY_LABEL_ALIAS_MARKED_DEGENERATE", str(sorted(set(aliases) & degenerate)))
    require(set(aliases.values()).issubset(eligible), "GEOMETRY_LABEL_ALIAS_TARGET_NOT_INDEPENDENT", str(aliases))
    require(args.max_registry_rows is None or args.max_registry_rows > 0, "GEOMETRY_LABEL_BAD_MAX_REGISTRY_ROWS", str(args.max_registry_rows))

    registry_path = args.label_root / "embedder_geometry_label_registry.jsonl"
    rejection_path = args.label_root / "embedder_geometry_label_rejections.jsonl"
    manifest_path = args.label_root / "embedder_geometry_label_manifest.json"
    require(registry_path.is_file(), "GEOMETRY_LABEL_INPUT_MISSING", str(registry_path))
    require(rejection_path.is_file(), "GEOMETRY_LABEL_INPUT_MISSING", str(rejection_path))
    before = {
        "output_root_exists": args.output_root.exists(),
        "output_root_files": sorted(path.name for path in args.output_root.iterdir()) if args.output_root.exists() else [],
    }
    if args.clean_output and args.output_root.exists():
        shutil.rmtree(args.output_root)
    args.output_root.mkdir(parents=True, exist_ok=True)

    stats: Counter[str] = Counter()
    embedder_counts: Counter[str] = Counter()
    admissibility_artifact = stream_jsonl(
        args.output_root / "geometry_label_admissibility.jsonl",
        scrub_registry_rows(
            registry_path,
            eligible=eligible,
            aliases=aliases,
            degenerate=degenerate,
            max_rows=args.max_registry_rows,
            stats=stats,
            embedder_counts=embedder_counts,
        ),
    )
    rejection_artifact = stream_jsonl(
        args.output_root / "geometry_label_health_rejections.jsonl",
        rejection_rows(rejection_path, degenerate=degenerate, max_rows=None, stats=stats),
    )

    partial = args.max_registry_rows is not None
    if not partial:
        require(stats["alias_source_live_rows"] > 0, "GEOMETRY_LABEL_ALIAS_LIVE_EDGE_NOT_OBSERVED", "expected duplicate alias rows to prove scrub")
        require(stats["rejection_rows_seen"] == len(degenerate), "GEOMETRY_LABEL_DEGENERATE_REJECTION_COUNT_MISMATCH", str(dict(stats)))
        require(all(embedder_counts[item] > 0 for item in eligible), "GEOMETRY_LABEL_ELIGIBLE_EMBEDDER_MISSING", str(dict(embedder_counts)))
        require(all(embedder_counts[item] > 0 for item in aliases), "GEOMETRY_LABEL_ALIAS_EMBEDDER_MISSING", str(dict(embedder_counts)))
        require(all(embedder_counts[item] > 0 for item in degenerate), "GEOMETRY_LABEL_DEGENERATE_EMBEDDER_MISSING", str(dict(embedder_counts)))

    policy = {
        "artifact_kind": f"{ARTIFACT_KIND}_space_policy",
        "schema_version": SCHEMA_VERSION,
        "formula_version": FORMULA_VERSION,
        "created_at_utc": started_at,
        "eligible_independent_embedder_ids": sorted(eligible),
        "duplicate_alias_map": dict(sorted(aliases.items())),
        "degenerate_embedder_ids": sorted(degenerate),
        "valid_independent_embedder_count": len(eligible),
        "valid_independent_pair_denominator": len(eligible) * (len(eligible) - 1) // 2,
        "source_policy": "duplicate and degenerate spaces cannot count as independent live geometry labels",
    }
    policy_sha = atomic_json_dump(args.output_root / "geometry_label_space_policy.json", policy)

    expected_pair_denominator = len(eligible) * (len(eligible) - 1) // 2
    checks = [
        {
            "check": "alias_source_live_rows_are_scrubbed",
            "passed": stats["alias_source_live_rows"] > 0 if not partial else True,
            "actual_alias_source_live_rows": stats["alias_source_live_rows"],
            "expected": "alias rows may exist in source but independent_live_input_allowed is false in overlay",
        },
        {
            "check": "degenerate_rejections_present",
            "passed": stats["rejection_rows_seen"] == len(degenerate) if not partial else True,
            "actual": stats["rejection_rows_seen"],
            "expected": len(degenerate),
        },
        {
            "check": "independent_pair_denominator_matches_policy",
            "passed": policy["valid_independent_pair_denominator"] == expected_pair_denominator,
            "actual": policy["valid_independent_pair_denominator"],
            "expected": expected_pair_denominator,
        },
    ]
    all_passed = all(bool(check["passed"]) for check in checks)
    require(all_passed, "GEOMETRY_LABEL_ADMISSIBILITY_CHECK_FAILED", str(checks))

    manifest = {
        "artifact_kind": f"{ARTIFACT_KIND}_manifest",
        "schema_version": SCHEMA_VERSION,
        "formula_version": FORMULA_VERSION,
        "created_at_utc": started_at,
        "all_passed": True,
        "source_of_truth": {
            "label_root": str(args.label_root),
            "registry": input_inventory(registry_path),
            "rejections": input_inventory(rejection_path),
            "source_manifest": input_inventory(manifest_path) if manifest_path.exists() else None,
        },
        "output_root": str(args.output_root),
        "before": before,
        "policy": policy,
        "partial_read": partial,
        "counts": {
            **dict(sorted(stats.items())),
            "by_embedder": dict(sorted(embedder_counts.items())),
        },
        "outputs": {
            "admissibility": admissibility_artifact,
            "health_rejections": rejection_artifact,
            "space_policy": {
                "path": str(args.output_root / "geometry_label_space_policy.json"),
                "sha256": policy_sha,
                "bytes": (args.output_root / "geometry_label_space_policy.json").stat().st_size,
            },
        },
        "checks": checks,
    }
    manifest_sha = atomic_json_dump(args.output_root / "geometry_label_admissibility_manifest.json", manifest)
    manifest["outputs"]["manifest"] = {
        "path": str(args.output_root / "geometry_label_admissibility_manifest.json"),
        "sha256": manifest_sha,
        "bytes": (args.output_root / "geometry_label_admissibility_manifest.json").stat().st_size,
    }
    return manifest


def add_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--label-root", type=Path, default=DEFAULT_LABEL_ROOT)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--evidence-root", type=Path, default=DEFAULT_EVIDENCE_ROOT)
    parser.add_argument("--eligible-independent-embedders", default=DEFAULT_ELIGIBLE_INDEPENDENT)
    parser.add_argument("--alias-map", default=DEFAULT_ALIAS_MAP)
    parser.add_argument("--degenerate-embedders", default=DEFAULT_DEGENERATE)
    parser.add_argument("--max-registry-rows", type=int, default=None)
    parser.add_argument("--clean-output", action="store_true")
