"""Rebuild per-embedder exact-vector geometry label atlases.

This materializer is intentionally conservative: it does not try to choose a
final semantic clustering algorithm. It rebuilds the maximum-density
source-layer labels we can prove from current bytes: one occupied microcell per
``(embedder_id, vector_sha256)`` plus deterministic descriptors and exemplars.
Downstream hierarchy/Leiden/HDBSCAN work can add parent labels on top of this
floor, but cannot safely skip this provenance layer.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
from collections import Counter
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Iterable

from .core import SwebenchOpsError, sha256_file, utc_now, write_json_checked
from .forward_cache_lineage import guard_packed_record_lineage


SCHEMA_VERSION = 1
ARTIFACT_KIND = "embedder_geometry_label_atlas"
CLUSTER_ALGORITHM = "exact_vector_sha_microcells"
CLUSTER_ALGORITHM_FAMILY = "occupied_vector_equivalence_classes"
CLUSTER_STATUS = "exact_occupied_vector_microcell_source_layer_not_final_semantic_labels"
LABEL_ALGORITHM = "deterministic_exact_microcell_ast_descriptor_v2"
DERIVED_LABEL_ALGORITHM = "deterministic_multi_resolution_region_labels_v1"
MASK_POLICY = "slot_identity_preserved_missingness_mask_v1"
DEFAULT_FORWARD_ROOT = Path(
    "/zfs/archive/contextgraph/corpus/"
    "python-forward-cache-v2-static-ast-e7lang-fa703b89-20260523T204324Z"
)
DEFAULT_CHUNKS_JSONL = Path("/zfs/archive/contextgraph/corpus/python-ast-chunks-v1/python_ast_chunks.jsonl")
DEFAULT_ADMISSIBILITY_ROOT = Path(
    "/zfs/archive/contextgraph/state/labels/task-py-g-143-geometry-label-admissibility-v2-static-ast-v1"
)
DEFAULT_OUTPUT_ROOT = Path(
    "/zfs/archive/contextgraph/state/labels/task-py-g-136-per-embedder-geometry-labels-v2-static-ast-v1"
)
DEFAULT_EVIDENCE_ROOT = Path(
    "/zfs/archive/contextgraph/fsv/task-py-g-136-per-embedder-geometry-labels-fsv/v2-static-ast-v1"
)
DEFAULT_ELIGIBLE_INDEPENDENT = "e1,e6,e7,e8,e9,e10,e12,e13,e14"
DEFAULT_ALIAS_MAP = ""
DEFAULT_DEGENERATE = ""
DEFAULT_RETIRED_STATIC_AST = "e2,e3,e4"
TOKEN_RE = re.compile(r"[A-Za-z_][A-Za-z0-9_]{1,63}")
STOP_TERMS = {
    "self",
    "cls",
    "true",
    "false",
    "none",
    "return",
    "from",
    "import",
    "with",
    "for",
    "while",
    "class",
    "def",
    "test",
    "tests",
    "python",
    "core",
    "utils",
    "common",
}


class GeometryLabelAtlasError(SwebenchOpsError):
    def __init__(self, code: str, message: str, details: dict[str, Any] | None = None):
        super().__init__(
            code,
            message,
            "Re-run with valid aiwonder forward-cache, AST chunk, #614 completeness-mask, and #143 admissibility-policy inputs.",
            details,
        )


@dataclass
class JsonlSink:
    path: Path
    count: int = 0
    digest: hashlib._hashlib.HASH = field(default_factory=hashlib.sha256)
    _fh: Any = None
    _tmp: Path | None = None

    def __enter__(self) -> "JsonlSink":
        self.path.parent.mkdir(parents=True, exist_ok=True)
        self._tmp = self.path.with_suffix(self.path.suffix + ".tmp")
        self._fh = self._tmp.open("w", encoding="utf-8")
        return self

    def write(self, row: dict[str, Any]) -> None:
        payload = json.dumps(row, sort_keys=True, separators=(",", ":"))
        self._fh.write(payload)
        self._fh.write("\n")
        self.digest.update(payload.encode("utf-8"))
        self.digest.update(b"\n")
        self.count += 1

    def __exit__(self, exc_type: Any, exc: Any, tb: Any) -> None:
        if self._fh is not None:
            self._fh.close()
        if exc_type is None and self._tmp is not None:
            self._tmp.replace(self.path)
        elif self._tmp is not None and self._tmp.exists():
            self._tmp.unlink()

    def artifact(self) -> dict[str, Any]:
        return {
            "path": str(self.path),
            "rows": self.count,
            "bytes": self.path.stat().st_size if self.path.exists() else 0,
            "sha256": "sha256:" + self.digest.hexdigest(),
        }


@dataclass
class ClusterAgg:
    embedder_id: str
    vector_sha256: str
    cluster_id: str
    space_admissibility: str
    alias_of_embedder_id: str | None
    duplicate_space_alias_of: str | None
    live_input_safe: bool
    independent_live_input_allowed: bool
    metric_contract: str
    support_count: int = 0
    partition_counts: Counter[str] = field(default_factory=Counter)
    mutation_category_counts: Counter[str] = field(default_factory=Counter)
    chunk_kind_counts: Counter[str] = field(default_factory=Counter)
    node_kind_counts: Counter[str] = field(default_factory=Counter)
    repo_counts: Counter[str] = field(default_factory=Counter)
    exemplar: dict[str, Any] | None = None


def require(condition: bool, code: str, message: str, details: dict[str, Any] | None = None) -> None:
    if not condition:
        raise GeometryLabelAtlasError(code, message, details)


def read_json(path: Path) -> dict[str, Any]:
    require(path.is_file(), "GEOMETRY_LABEL_ATLAS_INPUT_MISSING", str(path))
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise GeometryLabelAtlasError(
            "GEOMETRY_LABEL_ATLAS_BAD_JSON",
            f"{path}: {exc}",
            {"path": str(path), "error": str(exc)},
        ) from exc
    require(isinstance(value, dict), "GEOMETRY_LABEL_ATLAS_JSON_NOT_OBJECT", str(path))
    return value


def iter_jsonl(path: Path) -> Iterable[tuple[int, dict[str, Any]]]:
    require(path.is_file(), "GEOMETRY_LABEL_ATLAS_INPUT_MISSING", str(path))
    with path.open("r", encoding="utf-8") as fh:
        for line_no, line in enumerate(fh, start=1):
            if not line.strip():
                continue
            try:
                row = json.loads(line)
            except json.JSONDecodeError as exc:
                raise GeometryLabelAtlasError(
                    "GEOMETRY_LABEL_ATLAS_BAD_JSONL",
                    f"{path}:{line_no}: {exc}",
                    {"path": str(path), "line_no": line_no},
                ) from exc
            require(isinstance(row, dict), "GEOMETRY_LABEL_ATLAS_ROW_NOT_OBJECT", f"{path}:{line_no}")
            yield line_no, row


def sha256_json(value: Any) -> str:
    payload = json.dumps(value, sort_keys=True, separators=(",", ":")).encode("utf-8")
    return "sha256:" + hashlib.sha256(payload).hexdigest()


def sha256_text(value: str) -> str:
    return "sha256:" + hashlib.sha256(value.encode("utf-8")).hexdigest()


def short_hash(value: str, length: int = 12) -> str:
    raw = value.removeprefix("sha256:")
    return raw[:length]


def parse_csv(raw: str | None) -> list[str]:
    if raw is None or not raw.strip():
        return []
    values = [item.strip() for item in raw.split(",") if item.strip()]
    require(len(values) == len(set(values)), "GEOMETRY_LABEL_ATLAS_DUPLICATE_EMBEDDER", str(values))
    return values


def row_key_from_fields(task_instance_id: str, workspace_state: str, chunk_id: str, source_sha256: str) -> str:
    return f"{task_instance_id}|{workspace_state}|{chunk_id}|{source_sha256}"


def canonical_row_key(row: dict[str, Any]) -> str:
    return row_key_from_fields(
        str(row.get("task_instance_id") or ""),
        str(row.get("workspace_state") or ""),
        str(row.get("chunk_id") or ""),
        str(row.get("source_sha256") or ""),
    )


def tree_state(path: Path) -> dict[str, Any]:
    if not path.exists():
        return {"path": str(path), "exists": False, "file_count": 0, "byte_count": 0}
    if path.is_file():
        return {"path": str(path), "exists": True, "file_count": 1, "byte_count": path.stat().st_size}
    file_count = 0
    byte_count = 0
    names: list[str] = []
    for child in sorted(path.rglob("*")):
        if child.is_file():
            file_count += 1
            byte_count += child.stat().st_size
            if len(names) < 20:
                names.append(str(child.relative_to(path)))
    return {
        "path": str(path),
        "exists": True,
        "file_count": file_count,
        "byte_count": byte_count,
        "sample_files": names,
        "sample_truncated": file_count > len(names),
    }


def parse_alias_map(raw: str | None) -> dict[str, str]:
    aliases: dict[str, str] = {}
    if raw is None or not raw.strip():
        return aliases
    for item in raw.split(","):
        if not item.strip():
            continue
        parts = [part.strip() for part in item.split(":", 1)]
        require(len(parts) == 2 and all(parts), "GEOMETRY_LABEL_ATLAS_BAD_ALIAS_MAP", item)
        alias, target = parts
        require(alias not in aliases, "GEOMETRY_LABEL_ATLAS_DUPLICATE_ALIAS", alias)
        aliases[alias] = target
    return aliases


def validate_policy(eligible: tuple[str, ...], aliases: dict[str, str], degenerate: tuple[str, ...], context: str) -> None:
    require(eligible, "GEOMETRY_LABEL_ATLAS_POLICY_NO_ELIGIBLE_EMBEDDERS", context)
    require(set(aliases.values()).issubset(set(eligible)), "GEOMETRY_LABEL_ATLAS_ALIAS_TARGET_NOT_INDEPENDENT", str(aliases))
    require(not (set(aliases) & set(eligible)), "GEOMETRY_LABEL_ATLAS_ALIAS_MARKED_INDEPENDENT", str(aliases))
    require(not (set(degenerate) & set(eligible)), "GEOMETRY_LABEL_ATLAS_DEGENERATE_MARKED_INDEPENDENT", str(degenerate))


def cli_policy(args: argparse.Namespace) -> dict[str, Any]:
    eligible = tuple(sorted(parse_csv(args.eligible_independent_embedders)))
    aliases = parse_alias_map(args.alias_map)
    degenerate = tuple(sorted(parse_csv(args.degenerate_embedders)))
    retired = tuple(sorted(parse_csv(args.retired_static_ast_embedders)))
    validate_policy(eligible, aliases, degenerate, "cli static-AST geometry policy")
    policy = {
        "artifact_kind": "geometry_label_admissibility_space_policy",
        "schema_version": SCHEMA_VERSION,
        "formula_version": "static_ast_v2_independent_embedder_policy_v1",
        "created_at_utc": utc_now(),
        "eligible_independent_embedder_ids": list(eligible),
        "duplicate_alias_map": dict(sorted(aliases.items())),
        "degenerate_embedder_ids": list(degenerate),
        "retired_for_static_ast_embedder_ids": list(retired),
        "valid_independent_embedder_count": len(eligible),
        "valid_independent_pair_denominator": len(eligible) * (len(eligible) - 1) // 2,
        "source_policy": "static AST v2 cache treats e8 as independent and retires e2/e3/e4 from the label source space",
    }
    return {
        "path": "cli:static_ast_v2_policy",
        "sha256": sha256_json(policy),
        "manifest_path": None,
        "manifest_sha256": None,
        "policy": policy,
        "source_manifest": None,
        "eligible": eligible,
        "aliases": aliases,
        "degenerate": degenerate,
        "retired": retired,
    }


def load_policy(args: argparse.Namespace) -> dict[str, Any]:
    admissibility_root = args.admissibility_root
    policy_path = admissibility_root / "geometry_label_space_policy.json"
    manifest_path = admissibility_root / "geometry_label_admissibility_manifest.json"
    if not policy_path.is_file() or args.use_cli_policy:
        return cli_policy(args)
    policy = read_json(policy_path)
    manifest = read_json(manifest_path)
    require(
        policy.get("artifact_kind") == "geometry_label_admissibility_space_policy",
        "GEOMETRY_LABEL_ATLAS_BAD_POLICY_KIND",
        str(policy_path),
    )
    eligible = tuple(str(item) for item in policy.get("eligible_independent_embedder_ids") or [])
    aliases = {str(k): str(v) for k, v in (policy.get("duplicate_alias_map") or {}).items()}
    degenerate = tuple(str(item) for item in policy.get("degenerate_embedder_ids") or [])
    validate_policy(eligible, aliases, degenerate, str(policy_path))
    return {
        "path": str(policy_path),
        "sha256": sha256_file(policy_path),
        "manifest_path": str(manifest_path),
        "manifest_sha256": sha256_file(manifest_path),
        "policy": policy,
        "source_manifest": manifest,
        "eligible": eligible,
        "aliases": aliases,
        "degenerate": degenerate,
        "retired": tuple(str(item) for item in policy.get("retired_for_static_ast_embedder_ids") or []),
    }


def completed_shard_entries(args: argparse.Namespace) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    checkpoint_path = args.forward_root / "python_forward_cache_checkpoint.json"
    shard_plan_path = args.forward_root / "python_forward_cache_shard_plan.json"
    checkpoint = read_json(checkpoint_path)
    plan = read_json(shard_plan_path)
    require(checkpoint.get("passes") is True, "GEOMETRY_LABEL_ATLAS_CHECKPOINT_NOT_PASSING", str(checkpoint_path))
    require(plan.get("artifact_kind") == "python_forward_shard_scheduler_plan", "GEOMETRY_LABEL_ATLAS_BAD_SHARD_PLAN", str(shard_plan_path))
    progress = checkpoint.get("progress") or {}
    completed_count = int(progress.get("completed_shards") or 0)
    plan_shards = list(plan.get("shards") or [])
    require(completed_count > 0, "GEOMETRY_LABEL_ATLAS_NO_COMPLETED_SHARDS", str(progress))
    require(len(plan_shards) >= completed_count, "GEOMETRY_LABEL_ATLAS_SHARD_PLAN_TOO_SHORT", str(len(plan_shards)))
    if args.shards:
        wanted = set(parse_csv(args.shards))
        selected = [shard for shard in plan_shards[:completed_count] if str(shard.get("shard_id") or "") in wanted]
        require(len(selected) == len(wanted), "GEOMETRY_LABEL_ATLAS_REQUESTED_SHARD_MISSING", str(sorted(wanted)))
    elif args.all_completed_shards:
        selected = plan_shards[:completed_count]
    else:
        require(args.max_completed_shards > 0, "GEOMETRY_LABEL_ATLAS_BAD_MAX_COMPLETED_SHARDS", str(args.max_completed_shards))
        selected = plan_shards[: min(completed_count, args.max_completed_shards)]
    require(selected, "GEOMETRY_LABEL_ATLAS_NO_SHARDS_SELECTED", "")
    for shard in selected:
        shard_id = str(shard.get("shard_id") or "")
        require(shard_id, "GEOMETRY_LABEL_ATLAS_SHARD_ID_MISSING", str(shard))
        require(
            (args.forward_root / "shards" / shard_id / "python_forward_cache_packed_manifest.json").is_file(),
            "GEOMETRY_LABEL_ATLAS_PACKED_MANIFEST_MISSING",
            shard_id,
        )
    return selected, {
        "checkpoint_path": str(checkpoint_path),
        "checkpoint_sha256": sha256_file(checkpoint_path),
        "shard_plan_path": str(shard_plan_path),
        "shard_plan_sha256": sha256_file(shard_plan_path),
        "completed_shards": completed_count,
        "selected_shards": [str(shard.get("shard_id")) for shard in selected],
        "progress": progress,
    }


def selected_ranges(shards: list[dict[str, Any]]) -> list[tuple[int, int]]:
    ranges: list[tuple[int, int]] = []
    for shard in shards:
        start = int(shard.get("chunk_offset") or 0)
        count = int(shard.get("max_chunks") or 0)
        require(count > 0, "GEOMETRY_LABEL_ATLAS_BAD_SHARD_RANGE", str(shard))
        ranges.append((start, start + count))
    return ranges


def in_ranges(index: int, ranges: list[tuple[int, int]]) -> bool:
    return any(start <= index < end for start, end in ranges)


def load_window_jsonl(path: Path, ranges: list[tuple[int, int]]) -> tuple[dict[str, dict[str, Any]], dict[str, Any]]:
    max_needed = max(end for _, end in ranges)
    by_key: dict[str, dict[str, Any]] = {}
    digest = hashlib.sha256()
    rows = 0
    for idx, row in enumerate(row for _, row in iter_jsonl(path)):
        if idx >= max_needed:
            break
        if not in_ranges(idx, ranges):
            continue
        key = canonical_row_key(row)
        require("||" not in key and key.count("|") == 3, "GEOMETRY_LABEL_ATLAS_ROW_KEY_MISSING", f"{path}:{idx + 1}")
        require(key not in by_key, "GEOMETRY_LABEL_ATLAS_DUPLICATE_ROW_KEY", key)
        by_key[key] = row
        digest.update(json.dumps(row, sort_keys=True, separators=(",", ":")).encode("utf-8"))
        digest.update(b"\n")
        rows += 1
    expected_capacity = sum(end - start for start, end in ranges)
    require(
        rows > 0 and rows <= expected_capacity,
        "GEOMETRY_LABEL_ATLAS_WINDOW_ROW_COUNT_MISMATCH",
        f"{path}: expected at most {expected_capacity}, found {rows}",
    )
    return by_key, {
        "path": str(path),
        "bytes": path.stat().st_size,
        "selected_rows": rows,
        "selected_capacity_rows": expected_capacity,
        "selected_window_sha256": "sha256:" + digest.hexdigest(),
    }


def load_jsonl_by_keys(path: Path, required_keys: set[str]) -> tuple[dict[str, dict[str, Any]], dict[str, Any]]:
    by_key: dict[str, dict[str, Any]] = {}
    digest = hashlib.sha256()
    scanned = 0
    for _, row in iter_jsonl(path):
        scanned += 1
        key = canonical_row_key(row)
        if key not in required_keys:
            continue
        require(key not in by_key, "GEOMETRY_LABEL_ATLAS_DUPLICATE_ROW_KEY", key)
        by_key[key] = row
        digest.update(json.dumps(row, sort_keys=True, separators=(",", ":")).encode("utf-8"))
        digest.update(b"\n")
        if len(by_key) == len(required_keys):
            break
    missing = sorted(required_keys - set(by_key))
    require(
        not missing,
        "GEOMETRY_LABEL_ATLAS_REQUIRED_MASK_KEYS_MISSING",
        f"{path}: missing {len(missing)} selected row keys",
        {"missing_sample": missing[:20]},
    )
    return by_key, {
        "path": str(path),
        "bytes": path.stat().st_size,
        "scanned_rows": scanned,
        "selected_rows": len(by_key),
        "selected_keyset_sha256": "sha256:" + digest.hexdigest(),
    }


def pair_key_from_record(row: dict[str, Any]) -> tuple[str, str]:
    key = row.get("key") or {}
    chunk = row.get("chunk") or {}
    return (
        row_key_from_fields(
            str(chunk.get("task_instance_id") or row.get("task_instance_id") or ""),
            str(chunk.get("workspace_state") or row.get("workspace_state") or ""),
            str(key.get("chunk_id") or row.get("chunk_id") or ""),
            str(key.get("source_sha256") or row.get("source_sha256") or ""),
        ),
        str(key.get("embedder_id") or row.get("embedder_id") or ""),
    )


def pair_key_from_quarantine(row: dict[str, Any]) -> tuple[str, str]:
    return (
        row_key_from_fields(
            str(row.get("task_instance_id") or ""),
            str(row.get("workspace_state") or ""),
            str(row.get("chunk_id") or ""),
            str(row.get("source_sha256") or ""),
        ),
        str(row.get("embedder_id") or ""),
    )


def mask_row_from_forward_cache(
    row_key: str,
    chunk: dict[str, Any],
    *,
    included: tuple[str, ...],
    retired: tuple[str, ...],
    present_slots: set[str],
    quarantine_reasons: dict[str, dict[str, Any]],
) -> dict[str, Any]:
    present = tuple(slot for slot in included if slot in present_slots)
    missing = tuple(slot for slot in included if slot not in present_slots)
    missing_reasons = {slot: quarantine_reasons[slot] for slot in missing if slot in quarantine_reasons}
    require(len(missing_reasons) == len(missing), "GEOMETRY_LABEL_ATLAS_UNACCOUNTED_MISSING_PAIR", row_key)
    if not missing:
        status = "complete"
    elif not present:
        status = "zero_slot"
    else:
        status = "partial"
    return {
        "artifact_kind": "geometry_label_forward_cache_derived_mask_row",
        "schema_version": SCHEMA_VERSION,
        "mask_policy_version": "static_ast_v2_forward_cache_derived_missingness_mask_v1",
        "row_key": row_key,
        "task_instance_id": chunk.get("task_instance_id"),
        "workspace_state": chunk.get("workspace_state"),
        "chunk_id": chunk.get("chunk_id"),
        "source_sha256": chunk.get("source_sha256"),
        "file_sha256": chunk.get("file_sha256"),
        "repo": chunk.get("repo"),
        "relative_path": chunk.get("relative_path"),
        "mutation_category": chunk.get("mutation_category"),
        "partition": chunk.get("partition") or "unknown",
        "chunk_kind": chunk.get("chunk_kind"),
        "node_kind": chunk.get("node_kind"),
        "line_start": chunk.get("line_start"),
        "line_end": chunk.get("line_end"),
        "byte_start": chunk.get("byte_start"),
        "byte_end": chunk.get("byte_end"),
        "source_text_bytes": chunk.get("source_text_bytes"),
        "expected_slots": list(included),
        "retired_slots": list(retired),
        "present_slots": list(present),
        "missing_slots": list(missing),
        "present_slot_count": len(present),
        "missing_slot_count": len(missing),
        "row_status": status,
        "complete_static_ast_9slot_row": status == "complete" and len(included) == 9,
        "missing_slot_reasons": missing_reasons,
    }


def derive_mask_from_forward_cache(
    args: argparse.Namespace,
    shards: list[dict[str, Any]],
    chunk_by_key: dict[str, dict[str, Any]],
    included: tuple[str, ...],
    retired: tuple[str, ...],
) -> tuple[dict[str, dict[str, Any]], dict[str, Any]]:
    active = set(included)
    present: dict[str, set[str]] = {key: set() for key in chunk_by_key}
    quarantines: dict[str, dict[str, dict[str, Any]]] = {key: {} for key in chunk_by_key}
    stats: Counter[str] = Counter()
    packed_inputs: list[dict[str, Any]] = []

    for shard in sorted(shards, key=lambda item: str(item.get("shard_id"))):
        shard_id = str(shard.get("shard_id") or "")
        shard_root = args.forward_root / "shards" / shard_id
        packed_manifest_path = shard_root / "python_forward_cache_packed_manifest.json"
        packed_manifest = read_json(packed_manifest_path)
        require(packed_manifest.get("passes") is True, "GEOMETRY_LABEL_ATLAS_PACKED_MANIFEST_NOT_PASSING", shard_id)
        per_embedder = packed_manifest.get("per_embedder") or {}
        require(
            active.issubset(set(per_embedder)),
            "GEOMETRY_LABEL_ATLAS_MASK_EMBEDDER_MISSING_FROM_PACKED_MANIFEST",
            f"{shard_id}:{sorted(active - set(per_embedder))}",
        )
        packed_inputs.append(
            {
                "shard_id": shard_id,
                "packed_manifest_path": str(packed_manifest_path),
                "packed_manifest_sha256": sha256_file(packed_manifest_path),
            }
        )
        for embedder_id in included:
            entry = per_embedder[embedder_id]
            record_path = Path(entry["record_path"])
            record_count = 0
            for line_no, record in iter_jsonl(record_path):
                row_key, actual_embedder = pair_key_from_record(record)
                require(
                    actual_embedder == embedder_id,
                    "GEOMETRY_LABEL_ATLAS_MASK_RECORD_EMBEDDER_MISMATCH",
                    f"{record_path}:{line_no}:{actual_embedder}",
                )
                if row_key not in chunk_by_key:
                    continue
                require(embedder_id not in present[row_key], "GEOMETRY_LABEL_ATLAS_DUPLICATE_PRESENT_PAIR", f"{row_key}:{embedder_id}")
                present[row_key].add(embedder_id)
                stats[f"present_rows:{embedder_id}"] += 1
                record_count += 1
            require(
                record_count == int(entry.get("row_count") or -1),
                "GEOMETRY_LABEL_ATLAS_MASK_PACKED_ROW_COUNT_MISMATCH",
                f"{record_path}:{record_count}!={entry.get('row_count')}",
            )

        quarantine_path = shard_root / "python_forward_cache_quarantine.jsonl"
        require(quarantine_path.is_file(), "GEOMETRY_LABEL_ATLAS_QUARANTINE_MISSING", str(quarantine_path))
        for line_no, row in iter_jsonl(quarantine_path):
            row_key, embedder_id = pair_key_from_quarantine(row)
            if embedder_id not in active or row_key not in chunk_by_key:
                continue
            require(embedder_id not in quarantines[row_key], "GEOMETRY_LABEL_ATLAS_DUPLICATE_QUARANTINE_PAIR", f"{row_key}:{embedder_id}")
            quarantines[row_key][embedder_id] = {
                "error_code": row.get("error_code"),
                "reason": row.get("reason"),
                "instrument_id": row.get("instrument_id"),
                "source_text_bytes": row.get("source_text_bytes"),
                "max_forward_source_text_bytes": row.get("max_forward_source_text_bytes"),
                "shard_id": shard_id,
            }
            stats[f"quarantine_rows:{embedder_id}"] += 1

    overlap = [(key, slot) for key, slots in present.items() for slot in slots if slot in quarantines.get(key, {})]
    require(not overlap, "GEOMETRY_LABEL_ATLAS_PRESENT_AND_QUARANTINED_PAIR", str(overlap[:20]))

    mask_by_key: dict[str, dict[str, Any]] = {}
    status_counts: Counter[str] = Counter()
    present_pairs = 0
    missing_pairs = 0
    digest = hashlib.sha256()
    for row_key, chunk in sorted(chunk_by_key.items()):
        row = mask_row_from_forward_cache(
            row_key,
            chunk,
            included=included,
            retired=retired,
            present_slots=present[row_key],
            quarantine_reasons=quarantines[row_key],
        )
        mask_by_key[row_key] = row
        status_counts[str(row["row_status"])] += 1
        present_pairs += len(row["present_slots"])
        missing_pairs += len(row["missing_slots"])
        digest.update(json.dumps(row, sort_keys=True, separators=(",", ":")).encode("utf-8"))
        digest.update(b"\n")

    return mask_by_key, {
        "path": "derived-from-forward-cache",
        "forward_root": str(args.forward_root),
        "selected_rows": len(mask_by_key),
        "expected_slots": list(included),
        "retired_slots": list(retired),
        "present_pairs": present_pairs,
        "missing_pairs": missing_pairs,
        "accounted_pairs": present_pairs + missing_pairs,
        "row_status_counts": dict(sorted(status_counts.items())),
        "selected_mask_sha256": "sha256:" + digest.hexdigest(),
        "packed_inputs": packed_inputs,
        "pair_stats": dict(sorted(stats.items())),
    }


def source_kind(embedder_id: str, eligible: set[str], aliases: dict[str, str], degenerate: set[str]) -> tuple[str, str | None]:
    if embedder_id in eligible:
        return "independent", None
    if embedder_id in aliases:
        return "alias", aliases[embedder_id]
    if embedder_id in degenerate:
        return "degenerate", None
    return "unknown", None


def metric_contract(embedder_id: str) -> str:
    if embedder_id in {"e6", "e13"}:
        return "sparse_or_splade_vector_sha_microcell; parent clustering must use sparse cosine/topk diagnostics"
    if embedder_id == "e9":
        return "hdc_vector_sha_microcell; parent clustering must audit hamming_or_cosine_collision_geometry"
    if embedder_id in {"e2", "e3", "e4"}:
        return "degenerate_temporal_vector_sha_microcell; not independent for current static AST corpus"
    return "l2_normalized_cosine"


def partition_for(row: dict[str, Any]) -> str:
    return str(row.get("partition") or "unknown")


def line_hash_from_record(raw_line: str, index_row: dict[str, Any] | None) -> str:
    if index_row and index_row.get("record_sha256"):
        return str(index_row["record_sha256"])
    return sha256_text(raw_line.rstrip("\n"))


def compact_terms(values: Iterable[str], limit: int = 8) -> list[dict[str, Any]]:
    counts: Counter[str] = Counter()
    for value in values:
        for token in TOKEN_RE.findall(value or ""):
            token = token.lower().strip("_")
            if len(token) < 2 or token in STOP_TERMS or token.isdigit():
                continue
            counts[token] += 1
    ordered = sorted(counts.items(), key=lambda item: (-item[1], item[0]))[:limit]
    return [{"term": term, "count": count} for term, count in ordered]


def slug_terms(terms: list[dict[str, Any]], fallback: str) -> str:
    selected = [str(item["term"]) for item in terms[:5]]
    if not selected:
        selected = [fallback]
    return "_".join(re.sub(r"[^a-z0-9_]+", "_", term.lower()).strip("_")[:32] for term in selected if term)


def slug_value(value: str) -> str:
    slug = re.sub(r"[^a-z0-9_]+", "_", value.lower()).strip("_")
    return slug[:80] or "unknown"


def support_band(support_count: int) -> str:
    if support_count <= 1:
        return "singleton"
    if support_count <= 3:
        return "tiny_2_3"
    if support_count <= 9:
        return "small_4_9"
    if support_count <= 19:
        return "meso_10_19"
    if support_count <= 99:
        return "repeated_20_99"
    if support_count <= 499:
        return "large_100_499"
    return "massive_500_plus"


def top_counter_value(counter: Counter[str]) -> tuple[str, int] | None:
    filtered = [(str(key), int(value)) for key, value in counter.items() if str(key) and str(key) != "unknown"]
    if not filtered:
        return None
    return sorted(filtered, key=lambda item: (-item[1], item[0]))[0]


def descriptor_terms_for_cluster(cluster: ClusterAgg, limit: int) -> list[dict[str, Any]]:
    exemplar = cluster.exemplar or {}
    return compact_terms(
        [
            str(exemplar.get("source_text") or ""),
            str(exemplar.get("relative_path") or ""),
            str(exemplar.get("name") or ""),
            str(exemplar.get("qualname") or ""),
            str(exemplar.get("node_kind") or ""),
            str(exemplar.get("chunk_kind") or ""),
        ],
        limit=limit,
    )


def derived_region_label_rows(
    cluster: ClusterAgg,
    *,
    base_label_id: str,
    base_label_name: str,
    label_param_hash: str,
    max_descriptor_terms: int,
) -> list[dict[str, Any]]:
    if not cluster.independent_live_input_allowed:
        return []

    values: list[tuple[str, str, str, int | None]] = [
        ("support_band", support_band(cluster.support_count), "exact_vector_support_count", cluster.support_count),
        ("metric_contract", slug_value(cluster.metric_contract), "embedder_metric_contract", None),
        ("admissibility", cluster.space_admissibility, "vector_space_policy", None),
    ]
    chunk_kind = top_counter_value(cluster.chunk_kind_counts)
    if chunk_kind is not None:
        values.append(("dominant_chunk_kind", chunk_kind[0], "canonical_ast_chunk_kind_counts", chunk_kind[1]))
    node_kind = top_counter_value(cluster.node_kind_counts)
    if node_kind is not None:
        values.append(("dominant_node_kind", node_kind[0], "canonical_ast_node_kind_counts", node_kind[1]))
    repo = top_counter_value(cluster.repo_counts)
    if repo is not None:
        values.append(("dominant_repo", repo[0], "canonical_ast_repo_counts", repo[1]))
    for term in descriptor_terms_for_cluster(cluster, max_descriptor_terms):
        values.append(("descriptor_term", str(term["term"]), "ast_source_descriptor_terms", int(term["count"])))

    rows: list[dict[str, Any]] = []
    seen: set[tuple[str, str]] = set()
    for family, value, evidence_source, evidence_count in values:
        value = slug_value(str(value))
        key = (family, value)
        if key in seen:
            continue
        seen.add(key)
        label_seed = (
            f"{DERIVED_LABEL_ALGORITHM}|{label_param_hash}|{cluster.embedder_id}|"
            f"{cluster.cluster_id}|{family}|{value}"
        )
        label_id = "geo_region_label:" + hashlib.sha256(label_seed.encode("utf-8")).hexdigest()[:32]
        rows.append(
            {
                "artifact_kind": "embedder_geometry_derived_region_label",
                "schema_version": SCHEMA_VERSION,
                "algorithm": DERIVED_LABEL_ALGORITHM,
                "source_algorithm": LABEL_ALGORITHM,
                "parameter_hash": label_param_hash,
                "source_cluster_parameter_hash": cluster.exemplar.get("cluster_parameter_hash") if cluster.exemplar else None,
                "embedder_id": cluster.embedder_id,
                "cluster_id": cluster.cluster_id,
                "vector_sha256": cluster.vector_sha256,
                "source_label_id": base_label_id,
                "source_label_name": base_label_name,
                "label_id": label_id,
                "label_name": f"{cluster.embedder_id}_{family}_{value}_{short_hash(label_id, 8)}"[:160],
                "label_family": family,
                "label_value": value,
                "evidence_source": evidence_source,
                "evidence_count": evidence_count,
                "support_count": cluster.support_count,
                "support_band": support_band(cluster.support_count),
                "space_admissibility": cluster.space_admissibility,
                "alias_of_embedder_id": cluster.alias_of_embedder_id,
                "duplicate_space_alias_of": cluster.duplicate_space_alias_of,
                "live_input_allowed": True,
                "independent_live_input_allowed": True,
                "leakage_policy": (
                    "derived label uses only vector identity, canonical AST/source descriptors, "
                    "embedder metric policy, and unsupervised support; excludes partition, mutation, oracle, and outcome fields"
                ),
            }
        )
    return rows


def make_descriptor(cluster: ClusterAgg, label_id: str, label_name: str, label_param_hash: str) -> tuple[dict[str, Any], dict[str, Any]]:
    exemplar = cluster.exemplar or {}
    terms = compact_terms(
        [
            str(exemplar.get("source_text") or ""),
            str(exemplar.get("relative_path") or ""),
            str(exemplar.get("name") or ""),
            str(exemplar.get("qualname") or ""),
            str(exemplar.get("node_kind") or ""),
            str(exemplar.get("chunk_kind") or ""),
        ]
    )
    descriptor_basis = {
        "cluster_id": cluster.cluster_id,
        "label_id": label_id,
        "terms": terms,
        "ast_surfaces": {
            "chunk_kind": exemplar.get("chunk_kind"),
            "node_kind": exemplar.get("node_kind"),
            "name": exemplar.get("name"),
            "qualname": exemplar.get("qualname"),
            "relative_path": exemplar.get("relative_path"),
        },
    }
    descriptor_hash = sha256_json(descriptor_basis)
    descriptor = {
        "artifact_kind": "embedder_geometry_label_descriptor",
        "schema_version": SCHEMA_VERSION,
        "algorithm": LABEL_ALGORITHM,
        "parameter_hash": label_param_hash,
        "source_cluster_parameter_hash": cluster.exemplar.get("cluster_parameter_hash") if cluster.exemplar else None,
        "embedder_id": cluster.embedder_id,
        "cluster_id": cluster.cluster_id,
        "vector_sha256": cluster.vector_sha256,
        "label_id": label_id,
        "label_name": label_name,
        "support_count": cluster.support_count,
        "descriptor_hash": descriptor_hash,
        "descriptor_terms": terms,
        "descriptor_source_counts": {
            "support_count": cluster.support_count,
            "exemplar_terms": len(terms),
        },
        "ast_surfaces": descriptor_basis["ast_surfaces"],
        "audit_only_fields": {
            "partition_counts": dict(sorted(cluster.partition_counts.items())),
            "mutation_category_counts": dict(sorted(cluster.mutation_category_counts.items())),
        },
        "space_admissibility": cluster.space_admissibility,
        "alias_of_embedder_id": cluster.alias_of_embedder_id,
        "duplicate_space_alias_of": cluster.duplicate_space_alias_of,
        "canonical_ast_lineage_used": exemplar.get("canonical_ast_lineage_used"),
        "raw_packed_path_metadata_legacy": exemplar.get("raw_packed_path_metadata_legacy"),
        "raw_packed_legacy_fields": exemplar.get("raw_packed_legacy_fields") or [],
        "source_lineage_policy": exemplar.get("source_lineage_policy"),
        "live_input_allowed": cluster.live_input_safe,
        "independent_live_input_allowed": cluster.independent_live_input_allowed,
        "leakage_policy": "live_descriptor_excludes_partition_and_mutation_category; audit_fields_not_live_input",
        "stability_status": stability_status(cluster.support_count, cluster.space_admissibility),
    }
    exemplar_row = {
        "artifact_kind": "embedder_geometry_label_exemplar",
        "schema_version": SCHEMA_VERSION,
        "algorithm": LABEL_ALGORITHM,
        "parameter_hash": label_param_hash,
        "source_cluster_parameter_hash": cluster.exemplar.get("cluster_parameter_hash") if cluster.exemplar else None,
        "embedder_id": cluster.embedder_id,
        "cluster_id": cluster.cluster_id,
        "vector_sha256": cluster.vector_sha256,
        "label_id": label_id,
        "label_name": label_name,
        "support_count": cluster.support_count,
        "exemplar_row_key": exemplar.get("row_key"),
        "exemplar_chunk_id": exemplar.get("chunk_id"),
        "exemplar_source_sha256": exemplar.get("source_sha256"),
        "exemplar_record_ref": exemplar.get("record_ref"),
        "exemplar_record_index_ref": exemplar.get("record_index_ref"),
        "relative_path": exemplar.get("relative_path"),
        "line_start": exemplar.get("line_start"),
        "line_end": exemplar.get("line_end"),
        "byte_start": exemplar.get("byte_start"),
        "byte_end": exemplar.get("byte_end"),
        "source_text_sha256": exemplar.get("source_sha256"),
        "source_text_snippet": exemplar.get("source_text_snippet"),
        "source_text_truncated": exemplar.get("source_text_truncated"),
        "space_admissibility": cluster.space_admissibility,
        "alias_of_embedder_id": cluster.alias_of_embedder_id,
        "duplicate_space_alias_of": cluster.duplicate_space_alias_of,
        "canonical_ast_lineage_used": exemplar.get("canonical_ast_lineage_used"),
        "raw_packed_path_metadata_legacy": exemplar.get("raw_packed_path_metadata_legacy"),
        "raw_packed_legacy_fields": exemplar.get("raw_packed_legacy_fields") or [],
        "source_lineage_policy": exemplar.get("source_lineage_policy"),
        "live_input_allowed": cluster.live_input_safe,
        "independent_live_input_allowed": cluster.independent_live_input_allowed,
    }
    return descriptor, exemplar_row


def stability_status(support_count: int, kind: str) -> str:
    if kind == "degenerate":
        return "rejected_degenerate_space"
    if kind == "alias":
        return "alias_non_independent_exact_cell"
    if support_count >= 20:
        return "stable_repeated_exact_cell"
    return "low_support_exact_cell_provisional"


def cluster_label(cluster: ClusterAgg, label_param_hash: str) -> tuple[str, str]:
    exemplar = cluster.exemplar or {}
    terms = compact_terms(
        [
            str(exemplar.get("source_text") or ""),
            str(exemplar.get("relative_path") or ""),
            str(exemplar.get("name") or ""),
            str(exemplar.get("qualname") or ""),
        ],
        limit=6,
    )
    kind = str(exemplar.get("chunk_kind") or "chunk").lower()
    slug = slug_terms(terms, "microcell")
    label_id = "geo_label:" + hashlib.sha256(
        f"{LABEL_ALGORITHM}|{label_param_hash}|{cluster.embedder_id}|{cluster.vector_sha256}".encode("utf-8")
    ).hexdigest()[:32]
    label_name = f"{cluster.embedder_id}_{kind}_{slug}_{short_hash(label_id, 8)}"
    if cluster.space_admissibility == "degenerate":
        label_id = "geo_label_rejected:" + hashlib.sha256(
            f"rejected|{label_param_hash}|{cluster.embedder_id}|{cluster.vector_sha256}".encode("utf-8")
        ).hexdigest()[:12]
        label_name = f"{cluster.embedder_id}_rejected_degenerate_exact_microcell"
    return label_id, label_name[:160]


def exemplar_from(
    row: dict[str, Any],
    chunk: dict[str, Any],
    lineage: dict[str, Any],
    record_path: Path,
    index_path: Path,
    record_sha: str,
    vector_sha: str,
    cluster_param_hash: str,
) -> dict[str, Any]:
    key = row.get("key") or {}
    output = row.get("output") or {}
    text = str(chunk.get("source_text") or "")
    truncated = len(text.encode("utf-8")) > 512
    return {
        "row_key": canonical_row_key(chunk),
        "chunk_id": str(chunk.get("chunk_id") or key.get("chunk_id") or ""),
        "task_instance_id": str(chunk.get("task_instance_id") or ""),
        "workspace_state": str(chunk.get("workspace_state") or ""),
        "repo": str(chunk.get("repo") or ""),
        "relative_path": str(chunk.get("relative_path") or ""),
        "chunk_kind": str(chunk.get("chunk_kind") or ""),
        "node_kind": str(chunk.get("node_kind") or ""),
        "name": chunk.get("name"),
        "qualname": chunk.get("qualname"),
        "mutation_category": str(chunk.get("mutation_category") or ""),
        "partition": str(chunk.get("partition") or "unknown"),
        "source_sha256": str(chunk.get("source_sha256") or key.get("source_sha256") or ""),
        "file_sha256": str(chunk.get("file_sha256") or key.get("file_sha256") or ""),
        "line_start": chunk.get("line_start"),
        "line_end": chunk.get("line_end"),
        "byte_start": chunk.get("byte_start"),
        "byte_end": chunk.get("byte_end"),
        "source_text": text,
        "source_text_bytes": int(chunk.get("source_text_bytes") or len(text.encode("utf-8"))),
        "source_text_snippet": text[:512],
        "source_text_truncated": truncated,
        "record_ref": f"{record_path}#{record_sha}",
        "record_index_ref": f"{index_path}#{record_sha}",
        "record_line_sha256": record_sha,
        "canonical_ast_lineage_used": bool(lineage.get("canonical_ast_lineage_used")),
        "raw_packed_path_metadata_legacy": bool(lineage.get("raw_packed_path_metadata_legacy")),
        "raw_packed_legacy_fields": list(lineage.get("raw_packed_legacy_fields") or []),
        "source_lineage_policy": "packed record chunk.absolute_path/dataschema is legacy provenance only; active source lineage comes from canonical AST rows",
        "runtime_model_version": str(output.get("runtime_model_version") or key.get("model_hash") or "unknown"),
        "precision_class": str(output.get("precision_class") or "unknown"),
        "vector_sha256": vector_sha,
        "cluster_parameter_hash": cluster_param_hash,
    }


def membership_row(
    *,
    row: dict[str, Any],
    chunk: dict[str, Any],
    mask: dict[str, Any],
    embedder_id: str,
    vector_sha: str,
    lineage: dict[str, Any],
    cluster_id: str,
    record_path: Path,
    record_sha: str,
    cluster_param_hash: str,
    kind: str,
    alias_target: str | None,
    duplicate_alias: str | None,
) -> dict[str, Any]:
    key = row.get("key") or {}
    live = kind != "degenerate" and kind != "alias"
    return {
        "artifact_kind": "embedder_cluster_membership",
        "schema_version": SCHEMA_VERSION,
        "algorithm": CLUSTER_ALGORITHM,
        "algorithm_family": CLUSTER_ALGORITHM_FAMILY,
        "algorithm_status": CLUSTER_STATUS,
        "parameter_hash": cluster_param_hash,
        "embedder_id": embedder_id,
        "cluster_id": cluster_id,
        "vector_sha256": vector_sha,
        "row_key": canonical_row_key(chunk),
        "task_instance_id": chunk.get("task_instance_id"),
        "workspace_state": chunk.get("workspace_state"),
        "chunk_id": chunk.get("chunk_id"),
        "source_sha256": chunk.get("source_sha256"),
        "repo": chunk.get("repo"),
        "relative_path": chunk.get("relative_path"),
        "mutation_category": chunk.get("mutation_category"),
        "partition": mask.get("partition") or chunk.get("partition") or "unknown",
        "chunk_kind": chunk.get("chunk_kind"),
        "node_kind": chunk.get("node_kind"),
        "source_text_bytes": chunk.get("source_text_bytes"),
        "record_ref": f"{record_path}#{record_sha}",
        "record_line_sha256": record_sha,
        "canonical_ast_lineage_used": bool(lineage.get("canonical_ast_lineage_used")),
        "raw_packed_path_metadata_legacy": bool(lineage.get("raw_packed_path_metadata_legacy")),
        "raw_packed_legacy_fields": list(lineage.get("raw_packed_legacy_fields") or []),
        "source_lineage_policy": "packed record chunk.absolute_path/dataschema is legacy provenance only; active source lineage comes from canonical AST rows",
        "resolution": "exact_vector_sha256",
        "metric_contract": metric_contract(embedder_id),
        "space_admissibility": kind,
        "alias_of_embedder_id": alias_target,
        "duplicate_space_alias_of": duplicate_alias,
        "live_input_safe": live,
        "independent_live_input_allowed": kind == "independent",
        "mask_policy": mask.get("mask_policy_version") or MASK_POLICY,
        "cache_key_sha256": row.get("cache_key_sha256"),
        "model_hash": key.get("model_hash"),
        "input_sha256": key.get("input_sha256"),
    }


def materialize(args: argparse.Namespace) -> dict[str, Any]:
    started_at = utc_now()
    policy_info = load_policy(args)
    eligible = set(policy_info["eligible"])
    aliases = dict(policy_info["aliases"])
    degenerate = set(policy_info["degenerate"])
    retired = tuple(policy_info.get("retired") or parse_csv(args.retired_static_ast_embedders))
    policy_embedders = tuple(sorted(eligible | set(aliases) | degenerate))
    included = tuple(parse_csv(args.included_embedders) or policy_embedders)
    unknown = sorted(set(included) - set(policy_embedders))
    require(not unknown, "GEOMETRY_LABEL_ATLAS_INCLUDED_EMBEDDER_NOT_IN_POLICY", str(unknown))
    require(args.max_candidate_rows is None or args.max_candidate_rows > 0, "GEOMETRY_LABEL_ATLAS_BAD_MAX_CANDIDATE_ROWS", str(args.max_candidate_rows))
    require(
        args.max_derived_descriptor_terms >= 0,
        "GEOMETRY_LABEL_ATLAS_BAD_MAX_DERIVED_DESCRIPTOR_TERMS",
        str(args.max_derived_descriptor_terms),
    )

    shards, shard_summary = completed_shard_entries(args)
    ranges = selected_ranges(shards)
    chunk_by_key, chunk_summary = load_window_jsonl(args.chunks_jsonl, ranges)
    if args.completeness_mask_jsonl is None:
        mask_by_key, mask_summary = derive_mask_from_forward_cache(args, shards, chunk_by_key, included, retired)
    else:
        mask_by_key, mask_summary = load_jsonl_by_keys(args.completeness_mask_jsonl, set(chunk_by_key))
    require(set(chunk_by_key) == set(mask_by_key), "GEOMETRY_LABEL_ATLAS_CHUNK_MASK_KEY_MISMATCH", "selected windows differ")

    before = {"output_root": tree_state(args.output_root)}
    work_root = args.output_root.with_name(f"{args.output_root.name}.tmp-{os.getpid()}")
    if work_root.exists():
        shutil.rmtree(work_root)
    work_root.mkdir(parents=True)
    if args.output_root.exists() and not args.clean_output:
        raise GeometryLabelAtlasError(
            "GEOMETRY_LABEL_ATLAS_OUTPUT_EXISTS",
            f"output root already exists: {args.output_root}",
            {"output_root": str(args.output_root), "use_clean_output": True},
        )

    cluster_param_hash = sha256_json(
        {
            "algorithm": CLUSTER_ALGORITHM,
            "family": CLUSTER_ALGORITHM_FAMILY,
            "mask_policy": MASK_POLICY,
            "included_embedders": included,
            "policy_sha256": policy_info["sha256"],
            "selected_shards": shard_summary["selected_shards"],
        }
    )
    label_param_hash = sha256_json(
        {
            "algorithm": LABEL_ALGORITHM,
            "cluster_parameter_hash": cluster_param_hash,
            "descriptor": "exemplar_source_text_identifier_path_terms",
        }
    )

    clusters: dict[tuple[str, str], ClusterAgg] = {}
    stats: Counter[str] = Counter()
    packed_inputs: list[dict[str, Any]] = []
    cluster_membership_path = work_root / "embedder_cluster_memberships.jsonl"
    noise_path = work_root / "embedder_cluster_noise_ood.jsonl"

    with JsonlSink(cluster_membership_path) as membership_sink:
        for shard in sorted(shards, key=lambda item: str(item.get("shard_id"))):
            shard_id = str(shard.get("shard_id") or "")
            shard_root = args.forward_root / "shards" / shard_id
            packed_manifest_path = shard_root / "python_forward_cache_packed_manifest.json"
            packed_manifest = read_json(packed_manifest_path)
            require(packed_manifest.get("passes") is True, "GEOMETRY_LABEL_ATLAS_PACKED_MANIFEST_NOT_PASSING", shard_id)
            packed_inputs.append(
                {
                    "shard_id": shard_id,
                    "packed_manifest_path": str(packed_manifest_path),
                    "packed_manifest_sha256": sha256_file(packed_manifest_path),
                }
            )
            per_embedder = packed_manifest.get("per_embedder") or {}
            for embedder_id in included:
                require(embedder_id in per_embedder, "GEOMETRY_LABEL_ATLAS_EMBEDDER_MISSING_FROM_PACKED_MANIFEST", f"{shard_id}:{embedder_id}")
                entry = per_embedder[embedder_id]
                index_path = Path(entry["index_path"])
                record_path = Path(entry["record_path"])
                index_by_cache: dict[str, dict[str, Any]] = {}
                for _, index_row in iter_jsonl(index_path):
                    cache_key = str(index_row.get("cache_key_sha256") or "")
                    require(cache_key, "GEOMETRY_LABEL_ATLAS_INDEX_CACHE_KEY_MISSING", str(index_path))
                    require(cache_key not in index_by_cache, "GEOMETRY_LABEL_ATLAS_DUPLICATE_INDEX_CACHE_KEY", cache_key)
                    index_by_cache[cache_key] = index_row
                seen_cache_keys: set[str] = set()
                for line_no, record in iter_jsonl(record_path):
                    cache_key = str(record.get("cache_key_sha256") or "")
                    index_row = index_by_cache.get(cache_key)
                    require(index_row is not None, "GEOMETRY_LABEL_ATLAS_RECORD_INDEX_MISSING", f"{record_path}:{line_no}:{cache_key}")
                    seen_cache_keys.add(cache_key)
                    key = record.get("key") or {}
                    chunk = record.get("chunk") or {}
                    chunk_id = str(key.get("chunk_id") or record.get("chunk_id") or "")
                    source_sha256 = str(key.get("source_sha256") or record.get("source_sha256") or "")
                    task_instance_id = str(chunk.get("task_instance_id") or record.get("task_instance_id") or "")
                    workspace_state = str(chunk.get("workspace_state") or record.get("workspace_state") or "")
                    row_key = row_key_from_fields(task_instance_id, workspace_state, chunk_id, source_sha256)
                    chunk_row = chunk_by_key.get(row_key)
                    mask_row = mask_by_key.get(row_key)
                    require(chunk_row is not None, "GEOMETRY_LABEL_ATLAS_RECORD_CHUNK_JOIN_MISSING", f"{record_path}:{line_no}:{row_key}")
                    require(mask_row is not None, "GEOMETRY_LABEL_ATLAS_RECORD_MASK_JOIN_MISSING", f"{record_path}:{line_no}:{row_key}")
                    lineage = guard_packed_record_lineage(record, chunk_row, context=f"{record_path}:{line_no}")
                    stats["canonical_ast_lineage_guarded_records"] += 1
                    if lineage["raw_packed_path_metadata_legacy"]:
                        stats["raw_packed_path_metadata_legacy_records"] += 1
                        for legacy_field in lineage["raw_packed_legacy_fields"]:
                            stats[f"raw_packed_path_metadata_legacy_field:{legacy_field}"] += 1
                    present_slots = set(mask_row.get("present_slots") or [])
                    require(embedder_id in present_slots, "GEOMETRY_LABEL_ATLAS_RECORD_NOT_PRESENT_IN_MASK", f"{record_path}:{line_no}:{embedder_id}")
                    vector_sha = str(index_row.get("vector_sha256") or "")
                    require(vector_sha.startswith("sha256:"), "GEOMETRY_LABEL_ATLAS_VECTOR_SHA_MISSING", f"{record_path}:{line_no}")
                    kind, alias_target = source_kind(embedder_id, eligible, aliases, degenerate)
                    require(kind != "unknown", "GEOMETRY_LABEL_ATLAS_UNKNOWN_EMBEDDER", embedder_id)
                    duplicate_alias = alias_target if kind == "alias" else None
                    cluster_id = f"{embedder_id}:exact_vector_sha_microcell:{short_hash(cluster_param_hash)}:{short_hash(vector_sha, 24)}"
                    record_sha = str(index_row.get("record_sha256") or "")
                    agg_key = (embedder_id, vector_sha)
                    agg = clusters.get(agg_key)
                    if agg is None:
                        live = kind == "independent"
                        agg = ClusterAgg(
                            embedder_id=embedder_id,
                            vector_sha256=vector_sha,
                            cluster_id=cluster_id,
                            space_admissibility=kind,
                            alias_of_embedder_id=alias_target,
                            duplicate_space_alias_of=duplicate_alias,
                            live_input_safe=live,
                            independent_live_input_allowed=kind == "independent",
                            metric_contract=metric_contract(embedder_id),
                            exemplar=exemplar_from(record, chunk_row, lineage, record_path, index_path, record_sha, vector_sha, cluster_param_hash),
                        )
                        clusters[agg_key] = agg
                    agg.support_count += 1
                    agg.partition_counts[partition_for(mask_row)] += 1
                    agg.mutation_category_counts[str(chunk_row.get("mutation_category") or "unknown")] += 1
                    agg.chunk_kind_counts[str(chunk_row.get("chunk_kind") or "unknown")] += 1
                    agg.node_kind_counts[str(chunk_row.get("node_kind") or "unknown")] += 1
                    agg.repo_counts[str(chunk_row.get("repo") or "unknown")] += 1
                    stats[f"record_rows:{embedder_id}"] += 1
                    stats[f"space_admissibility:{kind}"] += 1
                    membership_sink.write(
                        membership_row(
                            row=record,
                            chunk=chunk_row,
                            mask=mask_row,
                            embedder_id=embedder_id,
                            vector_sha=vector_sha,
                            lineage=lineage,
                            cluster_id=cluster_id,
                            record_path=record_path,
                            record_sha=record_sha,
                            cluster_param_hash=cluster_param_hash,
                            kind=kind,
                            alias_target=alias_target,
                            duplicate_alias=duplicate_alias,
                        )
                    )
                require(seen_cache_keys == set(index_by_cache), "GEOMETRY_LABEL_ATLAS_INDEX_RECORD_MISMATCH", str(index_path))

    with JsonlSink(noise_path) as noise_sink:
        for row_key, mask_row in sorted(mask_by_key.items()):
            present = set(mask_row.get("present_slots") or [])
            reasons = mask_row.get("missing_slot_reasons") or {}
            for embedder_id in included:
                if embedder_id in present:
                    continue
                kind, alias_target = source_kind(embedder_id, eligible, aliases, degenerate)
                reason = reasons.get(embedder_id) or {"status": "missing_or_quarantined"}
                noise_sink.write(
                    {
                        "artifact_kind": "embedder_cluster_noise_ood",
                        "schema_version": SCHEMA_VERSION,
                        "algorithm": CLUSTER_ALGORITHM,
                        "parameter_hash": cluster_param_hash,
                        "mask_policy": mask_row.get("mask_policy_version") or MASK_POLICY,
                        "embedder_id": embedder_id,
                        "row_key": row_key,
                        "task_instance_id": mask_row.get("task_instance_id"),
                        "workspace_state": mask_row.get("workspace_state"),
                        "chunk_id": mask_row.get("chunk_id"),
                        "source_sha256": mask_row.get("source_sha256"),
                        "repo": mask_row.get("repo"),
                        "relative_path": mask_row.get("relative_path"),
                        "mutation_category": mask_row.get("mutation_category"),
                        "partition": mask_row.get("partition"),
                        "space_admissibility": kind,
                        "alias_of_embedder_id": alias_target,
                        "live_input_safe": False,
                        "independent_live_input_allowed": False,
                        "reason": reason,
                    }
                )
                stats[f"noise_rows:{embedder_id}"] += 1

    candidate_path = work_root / "embedder_cluster_candidates.jsonl"
    registry_path = work_root / "embedder_geometry_label_registry.jsonl"
    descriptor_path = work_root / "embedder_geometry_label_descriptors.jsonl"
    exemplar_path = work_root / "embedder_geometry_label_exemplars.jsonl"
    derived_region_label_path = work_root / "embedder_geometry_derived_region_labels.jsonl"
    rejection_path = work_root / "embedder_geometry_label_rejections.jsonl"
    sample_chains: list[dict[str, Any]] = []
    with JsonlSink(candidate_path) as candidate_sink, JsonlSink(registry_path) as registry_sink, JsonlSink(
        descriptor_path
    ) as descriptor_sink, JsonlSink(exemplar_path) as exemplar_sink, JsonlSink(
        derived_region_label_path
    ) as derived_region_label_sink, JsonlSink(rejection_path) as rejection_sink:
        sorted_clusters = sorted(clusters.values(), key=lambda item: (item.embedder_id, item.vector_sha256))
        for cluster in sorted_clusters:
            if args.max_candidate_rows is not None and candidate_sink.count >= args.max_candidate_rows:
                break
            label_id, label_name = cluster_label(cluster, label_param_hash)
            candidate = {
                "artifact_kind": "embedder_cluster_candidate",
                "schema_version": SCHEMA_VERSION,
                "algorithm": CLUSTER_ALGORITHM,
                "algorithm_family": CLUSTER_ALGORITHM_FAMILY,
                "algorithm_status": CLUSTER_STATUS,
                "parameter_hash": cluster_param_hash,
                "embedder_id": cluster.embedder_id,
                "cluster_id": cluster.cluster_id,
                "vector_sha256": cluster.vector_sha256,
                "support_count": cluster.support_count,
                "metric_contract": cluster.metric_contract,
                "microcluster_policy": "retain_every_exact_vector_cell_for_label_discovery",
                "space_admissibility": cluster.space_admissibility,
                "alias_of_embedder_id": cluster.alias_of_embedder_id,
                "duplicate_space_alias_of": cluster.duplicate_space_alias_of,
                "live_input_safe": cluster.live_input_safe,
                "independent_live_input_allowed": cluster.independent_live_input_allowed,
                "partition_counts": dict(sorted(cluster.partition_counts.items())),
                "mutation_category_counts": dict(sorted(cluster.mutation_category_counts.items())),
                "chunk_kind_counts": dict(sorted(cluster.chunk_kind_counts.items())),
                "node_kind_counts": dict(sorted(cluster.node_kind_counts.items())),
                "repo_counts": dict(sorted(cluster.repo_counts.items())),
                "exemplar": {
                    key: value
                    for key, value in (cluster.exemplar or {}).items()
                    if key not in {"source_text"}
                },
            }
            candidate_sink.write(candidate)
            source_label_status = "rejected" if cluster.space_admissibility == "degenerate" else "provisional"
            live_allowed = cluster.independent_live_input_allowed
            registry = {
                "artifact_kind": "embedder_geometry_label_registry",
                "schema_version": SCHEMA_VERSION,
                "algorithm": LABEL_ALGORITHM,
                "parameter_hash": label_param_hash,
                "source_cluster_parameter_hash": cluster_param_hash,
                "embedder_id": cluster.embedder_id,
                "cluster_id": cluster.cluster_id,
                "vector_sha256": cluster.vector_sha256,
                "label_id": label_id,
                "label_name": label_name,
                "label_status": source_label_status,
                "support_count": cluster.support_count,
                "descriptor_source": "ast_exemplar_source_text_and_path_terms",
                "stability_status": stability_status(cluster.support_count, cluster.space_admissibility),
                "space_admissibility": cluster.space_admissibility,
                "alias_of_embedder_id": cluster.alias_of_embedder_id,
                "duplicate_space_alias_of": cluster.duplicate_space_alias_of,
                "live_input_allowed": live_allowed,
                "independent_live_input_allowed": cluster.independent_live_input_allowed,
                "leakage_policy": "live_descriptor_excludes_partition_and_mutation_category; audit_fields_not_live_input",
            }
            if cluster.space_admissibility == "degenerate":
                rejection = {
                    "artifact_kind": "embedder_geometry_label_rejection",
                    "schema_version": SCHEMA_VERSION,
                    "algorithm": LABEL_ALGORITHM,
                    "parameter_hash": label_param_hash,
                    "source_cluster_parameter_hash": cluster_param_hash,
                    "embedder_id": cluster.embedder_id,
                    "cluster_id": cluster.cluster_id,
                    "vector_sha256": cluster.vector_sha256,
                    "label_id": label_id,
                    "support_count": cluster.support_count,
                    "live_input_allowed": False,
                    "independent_live_input_allowed": False,
                    "edge_case": "degenerate_embedder_slot",
                    "reason": "degenerate_constant_vector_space_from_source_microcell_layer",
                }
                registry["descriptor_hash"] = None
                rejection_sink.write(rejection)
            else:
                descriptor, exemplar = make_descriptor(cluster, label_id, label_name, label_param_hash)
                registry["descriptor_hash"] = descriptor["descriptor_hash"]
                descriptor_sink.write(descriptor)
                exemplar_sink.write(exemplar)
                derived_rows = derived_region_label_rows(
                    cluster,
                    base_label_id=label_id,
                    base_label_name=label_name,
                    label_param_hash=label_param_hash,
                    max_descriptor_terms=args.max_derived_descriptor_terms,
                )
                for derived_row in derived_rows:
                    derived_region_label_sink.write(derived_row)
                    stats[f"derived_label_family:{derived_row['label_family']}"] += 1
                if len(sample_chains) < 12:
                    sample_chains.append(
                        {
                            "label_id": label_id,
                            "cluster_id": cluster.cluster_id,
                            "embedder_id": cluster.embedder_id,
                            "descriptor_hash": descriptor["descriptor_hash"],
                            "exemplar_row_key": exemplar["exemplar_row_key"],
                            "exemplar_record_ref": exemplar["exemplar_record_ref"],
                            "exemplar_record_index_ref": exemplar["exemplar_record_index_ref"],
                            "space_admissibility": cluster.space_admissibility,
                            "independent_live_input_allowed": cluster.independent_live_input_allowed,
                        }
                    )
            registry_sink.write(registry)

    artifacts = {
        "cluster_candidates": candidate_sink.artifact(),
        "cluster_memberships": membership_sink.artifact(),
        "cluster_noise_ood": noise_sink.artifact(),
        "geometry_label_registry": registry_sink.artifact(),
        "geometry_label_descriptors": descriptor_sink.artifact(),
        "geometry_label_exemplars": exemplar_sink.artifact(),
        "geometry_derived_region_labels": derived_region_label_sink.artifact(),
        "geometry_label_rejections": rejection_sink.artifact(),
    }

    candidate_counts = Counter(cluster.space_admissibility for cluster in clusters.values())
    expected_noise_rows = sum(value for key, value in stats.items() if key.startswith("noise_rows:"))
    edge_checks = [
        {
            "name": "independent_spaces_emit_live_labels",
            "expected": "at least one candidate from an eligible independent embedder has independent_live_input_allowed=true",
            "actual": {"independent_candidates": candidate_counts.get("independent", 0)},
            "passes": candidate_counts.get("independent", 0) > 0,
        },
        {
            "name": "alias_spaces_are_not_independent_live_signal",
            "expected": "alias candidates exist only when alias embedders are included and carry independent_live_input_allowed=false",
            "actual": {"alias_candidates": candidate_counts.get("alias", 0), "aliases": aliases},
            "passes": (not (set(included) & set(aliases))) or candidate_counts.get("alias", 0) > 0,
        },
        {
            "name": "degenerate_spaces_are_rejected",
            "expected": "degenerate candidate count equals one exact vector per included degenerate slot",
            "actual": {"degenerate_candidates": candidate_counts.get("degenerate", 0), "included_degenerate": sorted(set(included) & degenerate)},
            "passes": (not (set(included) & degenerate)) or candidate_counts.get("degenerate", 0) == len(set(included) & degenerate),
        },
        {
            "name": "missing_or_quarantined_slots_are_noise_not_memberships",
            "expected": "missing mask events are emitted to embedder_cluster_noise_ood.jsonl exactly once per missing included slot",
            "actual": {"noise_rows": artifacts["cluster_noise_ood"]["rows"], "expected_noise_rows": expected_noise_rows},
            "passes": artifacts["cluster_noise_ood"]["rows"] == expected_noise_rows,
        },
        {
            "name": "independent_microcells_have_derived_region_labels",
            "expected": "every independent candidate receives at least one derived support/AST/descriptor label",
            "actual": {
                "independent_candidates": candidate_counts.get("independent", 0),
                "derived_region_label_rows": artifacts["geometry_derived_region_labels"]["rows"],
            },
            "passes": candidate_counts.get("independent", 0) == 0
            or artifacts["geometry_derived_region_labels"]["rows"] >= candidate_counts.get("independent", 0),
        },
        {
            "name": "packed_records_join_canonical_ast_lineage_before_labeling",
            "expected": "every emitted membership row passed the forward-cache lineage guard and treats raw packed paths as provenance only",
            "actual": {
                "membership_rows": artifacts["cluster_memberships"]["rows"],
                "canonical_ast_lineage_guarded_records": stats.get("canonical_ast_lineage_guarded_records", 0),
                "raw_packed_path_metadata_legacy_records": stats.get("raw_packed_path_metadata_legacy_records", 0),
            },
            "passes": stats.get("canonical_ast_lineage_guarded_records", 0) == artifacts["cluster_memberships"]["rows"],
        },
    ]
    all_passed = all(item["passes"] for item in edge_checks)

    manifest = {
        "artifact_kind": "embedder_geometry_label_manifest",
        "schema_version": SCHEMA_VERSION,
        "created_at_utc": utc_now(),
        "started_at_utc": started_at,
        "algorithm": LABEL_ALGORITHM,
        "cluster_algorithm": CLUSTER_ALGORITHM,
        "cluster_parameter_hash": cluster_param_hash,
        "label_parameter_hash": label_param_hash,
        "all_passed": all_passed,
        "policy": {
            "admissibility_policy_path": policy_info["path"],
            "admissibility_policy_sha256": policy_info["sha256"],
            "eligible_independent_embedder_ids": sorted(eligible),
            "duplicate_alias_map": aliases,
            "degenerate_embedder_ids": sorted(degenerate),
            "included_embedders": list(included),
            "forward_cache_lineage_policy": "packed record chunk.absolute_path/dataschema is historical/audit provenance only; active source lineage must be joined from canonical AST rows before label materialization",
        },
        "counts": {
            "selected_chunk_rows": len(chunk_by_key),
            "selected_mask_rows": len(mask_by_key),
            "candidate_rows": artifacts["cluster_candidates"]["rows"],
            "membership_rows": artifacts["cluster_memberships"]["rows"],
            "noise_ood_rows": artifacts["cluster_noise_ood"]["rows"],
            "registry_rows": artifacts["geometry_label_registry"]["rows"],
            "descriptor_rows": artifacts["geometry_label_descriptors"]["rows"],
            "exemplar_rows": artifacts["geometry_label_exemplars"]["rows"],
            "derived_region_label_rows": artifacts["geometry_derived_region_labels"]["rows"],
            "rejection_rows": artifacts["geometry_label_rejections"]["rows"],
            "candidate_space_admissibility": dict(sorted(candidate_counts.items())),
            "record_stats": dict(sorted(stats.items())),
        },
        "inputs": {
            "forward_root": str(args.forward_root),
            "shards": shard_summary,
            "packed_inputs": packed_inputs,
            "chunks": chunk_summary,
            "completeness_mask": mask_summary,
            "admissibility": {
                "policy_path": policy_info["path"],
                "policy_sha256": policy_info["sha256"],
                "manifest_path": policy_info["manifest_path"],
                "manifest_sha256": policy_info["manifest_sha256"],
            },
        },
        "artifacts": artifacts,
        "sample_label_descriptor_exemplar_chains": sample_chains,
        "edge_checks": edge_checks,
        "bounded_run": not args.all_completed_shards,
        "full_corpus_command_hint": (
            "Run the same command with --all-completed-shards on aiwonder after confirming disk budget; "
            "the materializer streams memberships but stores one aggregate per occupied vector microcell."
        ),
    }
    manifest_path = work_root / "embedder_geometry_label_manifest.json"
    write_json_checked(manifest_path, manifest)

    if args.output_root.exists() and args.clean_output:
        shutil.rmtree(args.output_root)
    work_root.replace(args.output_root)
    after = {"output_root": tree_state(args.output_root)}
    return {
        "ok": all_passed,
        "command": "python-geometry-label-atlas-materializer",
        "artifact_kind": f"{ARTIFACT_KIND}_command_evidence",
        "checked_at": utc_now(),
        "all_passed": all_passed,
        "before": before,
        "after": after,
        "manifest_path": str(args.output_root / "embedder_geometry_label_manifest.json"),
        "manifest_sha256": sha256_file(args.output_root / "embedder_geometry_label_manifest.json"),
        "counts": manifest["counts"],
        "edge_checks": edge_checks,
        "sample_label_descriptor_exemplar_chains": sample_chains,
        "output_root": str(args.output_root),
    }


def command_python_geometry_label_atlas_materializer(args: argparse.Namespace) -> dict[str, Any]:
    return materialize(args)


def add_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--forward-root", type=Path, default=DEFAULT_FORWARD_ROOT)
    parser.add_argument("--chunks-jsonl", type=Path, default=DEFAULT_CHUNKS_JSONL)
    parser.add_argument("--completeness-mask-jsonl", type=Path, default=None)
    parser.add_argument("--admissibility-root", type=Path, default=DEFAULT_ADMISSIBILITY_ROOT)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--evidence-root", type=Path, default=DEFAULT_EVIDENCE_ROOT)
    parser.add_argument("--included-embedders", default=None, help="Comma-separated subset. Default: all embedders in #143 policy.")
    parser.add_argument("--eligible-independent-embedders", default=DEFAULT_ELIGIBLE_INDEPENDENT)
    parser.add_argument("--alias-map", default=DEFAULT_ALIAS_MAP)
    parser.add_argument("--degenerate-embedders", default=DEFAULT_DEGENERATE)
    parser.add_argument("--retired-static-ast-embedders", default=DEFAULT_RETIRED_STATIC_AST)
    parser.add_argument("--use-cli-policy", action="store_true")
    parser.add_argument("--max-completed-shards", type=int, default=1)
    parser.add_argument("--all-completed-shards", action="store_true")
    parser.add_argument("--shards", default=None, help="Comma-separated shard IDs, e.g. shard-000000-offset-000000000")
    parser.add_argument("--max-candidate-rows", type=int, default=None)
    parser.add_argument("--max-derived-descriptor-terms", type=int, default=4)
    parser.add_argument("--clean-output", action="store_true")
