"""Shared helpers for the candidate-keyed delete_test_call label rebuild.

Split out of python_delete_test_call_candidate_labels.py for #748 file-size
tech debt (doctrine §14.8 ≤500 lines). Exports the error class, dataclasses,
low-level JSON / argparse helpers, all load_* corpus / chunks / mask /
forward-slots loaders, label-derivation helpers (relation, supervision,
missing-slot, observe_aggregate, failure_candidate_score, aggregate_row),
and bookkeeping helpers (bucket_count, counter_to_rows,
target_leakage_violations, split_support_gate).

Importers compose these into the mask materialization command.
"""

from __future__ import annotations

import argparse
import json
from collections import Counter, defaultdict
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Iterable

import numpy as np

from .core import SwebenchOpsError, normalize_category, sha256_file, write_json_checked
from .forward_cache_lineage import guard_packed_record_lineage
from .python_auto_label_discovery import (
    SCALAR_FEATURES,
    finite_float,
    is_live_predictor_label,
    slot_labels,
    split_live_and_supervision_labels,
    vector_space_blockers,
    vector_space_health_from_hash_counts,
)
from .python_real_corpus_constellation_smoke import vector_scalar_features
from .smoke_intelligence_matrices import read_json, row_key_from_fields, sha256_bytes


ARTIFACT_KIND = "delete_test_call_candidate_keyed_label_rebuild"
FORMULA_VERSION = "candidate_keyed_delete_test_call_label_rebuild_v1"


class CandidateLabelError(SwebenchOpsError):
    def __init__(self, code: str, message: str, details: dict[str, Any] | None = None):
        super().__init__(
            code,
            message,
            "Inspect the candidate-keyed corpus, forward-cache packed records, and completeness mask; rerun only after the physical SoT mismatch is fixed.",
            details or {},
        )


@dataclass
class ChunkWork:
    chunk: dict[str, Any]
    corpus: dict[str, Any]
    mask: dict[str, Any]
    slot_features: dict[str, dict[str, float]] = field(default_factory=dict)
    slot_labels: list[str] = field(default_factory=list)
    slot_vector_hashes: dict[str, str] = field(default_factory=dict)
    slot_dims: dict[str, int] = field(default_factory=dict)
    duplicate_slots: list[str] = field(default_factory=list)
    raw_legacy_slots: list[str] = field(default_factory=list)
    source_readback_unverified: list[str] = field(default_factory=list)


@dataclass
class CandidateAggregate:
    corpus: dict[str, Any]
    chunk_count: int = 0
    file_paths: Counter[str] = field(default_factory=Counter)
    surface_counts: Counter[str] = field(default_factory=Counter)
    slot_label_counts: Counter[str] = field(default_factory=Counter)
    pair_label_counts: Counter[str] = field(default_factory=Counter)
    group_label_counts: Counter[str] = field(default_factory=Counter)
    slot_feature_sums: dict[str, Counter[str]] = field(default_factory=lambda: defaultdict(Counter))
    slot_feature_counts: Counter[str] = field(default_factory=Counter)
    chunk_key_sample: list[str] = field(default_factory=list)
    failure_candidates: list[dict[str, Any]] = field(default_factory=list)


def iter_jsonl(path: Path) -> Iterable[tuple[int, dict[str, Any]]]:
    if not path.is_file():
        raise CandidateLabelError("DTC_LABEL_JSONL_MISSING", f"required JSONL missing: {path}", {"path": str(path)})
    with path.open("r", encoding="utf-8") as handle:
        for line_number, line in enumerate(handle, start=1):
            if not line.strip():
                continue
            try:
                yield line_number, json.loads(line)
            except json.JSONDecodeError as exc:
                raise CandidateLabelError(
                    "DTC_LABEL_JSONL_INVALID",
                    f"invalid JSONL at {path}:{line_number}",
                    {"path": str(path), "line_number": line_number, "error": str(exc)},
                ) from exc


def write_jsonl(path: Path, rows: Iterable[dict[str, Any]]) -> int:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    count = 0
    with tmp.open("w", encoding="utf-8") as handle:
        for row in rows:
            handle.write(json.dumps(row, sort_keys=True, separators=(",", ":"), allow_nan=False))
            handle.write("\n")
            count += 1
    tmp.replace(path)
    return count


def json_safe_payload(payload: dict[str, Any], *, context: str) -> dict[str, Any]:
    try:
        return json.loads(json.dumps(payload, sort_keys=True, allow_nan=False))
    except (TypeError, ValueError) as exc:
        raise CandidateLabelError(
            "DTC_LABEL_NON_JSON_EVIDENCE_PAYLOAD",
            f"evidence payload is not canonical JSON-safe: {context}",
            {"context": context, "error": str(exc)},
        ) from exc


def write_json_checked_safe(path: Path, payload: dict[str, Any]) -> None:
    write_json_checked(path, json_safe_payload(payload, context=str(path)))


def count_jsonl(path: Path) -> int:
    return sum(1 for _line, _row in iter_jsonl(path))


def require(condition: bool, code: str, message: str, details: dict[str, Any] | None = None) -> None:
    if not condition:
        raise CandidateLabelError(code, message, details)


def validate_args(args: argparse.Namespace) -> None:
    require(args.min_support > 0, "DTC_LABEL_MIN_SUPPORT_INVALID", "--min-support must be positive")
    require(args.min_lift >= 0.0, "DTC_LABEL_MIN_LIFT_INVALID", "--min-lift must be >= 0")
    for path in (args.corpus_root, args.forward_root, args.completeness_mask_jsonl):
        require(path.exists(), "DTC_LABEL_INPUT_MISSING", f"input path missing: {path}", {"path": str(path)})


def load_corpus(root: Path) -> tuple[dict[str, dict[str, Any]], dict[str, Any]]:
    corpus_path = root / "candidate_keyed_corpus_rows.jsonl"
    rows: dict[str, dict[str, Any]] = {}
    support = Counter()
    for line_number, row in iter_jsonl(corpus_path):
        candidate_id = str(row.get("candidate_row_id") or "")
        require(candidate_id.startswith("sha256:"), "DTC_LABEL_BAD_CANDIDATE_ID", "candidate_row_id is missing or malformed", {"line_number": line_number})
        require(candidate_id not in rows, "DTC_LABEL_DUPLICATE_CANDIDATE_ID", f"duplicate candidate_row_id: {candidate_id}")
        rows[candidate_id] = row
        status = "pass" if bool(row.get("oracle_all_passed")) else "fail"
        support[(str(row.get("stable_partition")), str(row.get("assertion_duty_sublabel")), status)] += 1
    require(bool(rows), "DTC_LABEL_CORPUS_EMPTY", f"empty candidate corpus: {corpus_path}")
    return rows, {"path": str(corpus_path), "sha256": sha256_file(corpus_path), "candidate_count": len(rows), "support": counter_to_rows(support)}


def load_chunks(root: Path, corpus: dict[str, dict[str, Any]]) -> tuple[dict[str, dict[str, Any]], dict[str, dict[str, Any]], dict[str, Any]]:
    chunks_path = root / "candidate_keyed_ast_chunks.jsonl"
    links_path = root / "candidate_keyed_ast_chunk_links.jsonl"
    chunks_by_join: dict[tuple[str, str, str], dict[str, Any]] = {}
    for _line_number, row in iter_jsonl(chunks_path):
        chunks_by_join[(str(row.get("chunk_id")), str(row.get("source_sha256")), str(row.get("workspace_state")))] = row

    chunks: dict[str, dict[str, Any]] = {}
    chunk_to_candidate: dict[str, dict[str, Any]] = {}
    for line_number, link in iter_jsonl(links_path):
        candidate_id = str(link.get("candidate_row_id") or "")
        require(candidate_id in corpus, "DTC_LABEL_LINK_UNKNOWN_CANDIDATE", "chunk link candidate not in corpus", {"line_number": line_number, "candidate_row_id": candidate_id})
        corpus_row = corpus[candidate_id]
        workspace_state = str(corpus_row.get("workspace_state") or "")
        join_key = (str(link.get("chunk_id")), str(link.get("chunk_source_sha256")), workspace_state)
        chunk = chunks_by_join.get(join_key)
        require(chunk is not None, "DTC_LABEL_LINK_CHUNK_MISSING", "chunk link has no matching AST chunk", {"line_number": line_number, "join_key": list(join_key)})
        row_key = row_key_from_fields(
            str(chunk.get("task_instance_id") or ""),
            str(chunk.get("workspace_state") or ""),
            str(chunk.get("chunk_id") or ""),
            str(chunk.get("source_sha256") or ""),
        )
        require(row_key not in chunks, "DTC_LABEL_DUPLICATE_CHUNK_ROW_KEY", f"duplicate chunk row_key: {row_key}")
        enriched = dict(chunk)
        enriched["_row_key"] = row_key
        enriched["_candidate_row_id"] = candidate_id
        enriched["_candidate_corpus_row_id"] = str(link.get("candidate_corpus_row_id") or corpus_row.get("candidate_corpus_row_id") or "")
        chunks[row_key] = enriched
        chunk_to_candidate[row_key] = corpus_row
    require(bool(chunks), "DTC_LABEL_CHUNKS_EMPTY", f"empty AST chunk links: {links_path}")
    return chunks, chunk_to_candidate, {
        "chunks_path": str(chunks_path),
        "chunks_sha256": sha256_file(chunks_path),
        "links_path": str(links_path),
        "links_sha256": sha256_file(links_path),
        "chunk_count": len(chunks),
    }


def load_masks(path: Path, chunks: dict[str, dict[str, Any]]) -> tuple[dict[str, dict[str, Any]], dict[str, Any]]:
    masks: dict[str, dict[str, Any]] = {}
    counts = Counter()
    for line_number, row in iter_jsonl(path):
        row_key = str(row.get("row_key") or "")
        require(row_key, "DTC_LABEL_MASK_ROW_KEY_MISSING", "mask row missing row_key", {"line_number": line_number})
        require(row_key not in masks, "DTC_LABEL_DUPLICATE_MASK_ROW", f"duplicate mask row_key: {row_key}")
        if row_key in chunks:
            chunk = chunks[row_key]
            require(
                str(row.get("candidate_row_id")) == str(chunk.get("_candidate_row_id")),
                "DTC_LABEL_MASK_CANDIDATE_MISMATCH",
                "mask candidate_row_id does not match chunk link",
                {"line_number": line_number, "row_key": row_key},
            )
            masks[row_key] = row
            counts["complete" if bool(row.get("complete_static_ast_9slot_row")) else "partial"] += 1
            counts[f"present_{int(row.get('present_slot_count') or 0)}"] += 1
    missing = sorted(set(chunks) - set(masks))
    require(not missing, "DTC_LABEL_MASK_MISSING_CHUNKS", "completeness mask missing chunk rows", {"missing_sample": missing[:10], "missing_count": len(missing)})
    return masks, {"path": str(path), "sha256": sha256_file(path), "mask_rows_joined": len(masks), "counts": dict(sorted(counts.items()))}


def load_forward_slots(forward_root: Path, chunks: dict[str, dict[str, Any]], masks: dict[str, dict[str, Any]], required_embedders: list[str]) -> tuple[dict[str, ChunkWork], dict[str, Any]]:
    plan = read_json(forward_root / "python_forward_cache_shard_plan.json")
    manifest = read_json(forward_root / "python_forward_cache_corpus_manifest.json")
    require(manifest.get("passes") is True, "DTC_LABEL_FORWARD_MANIFEST_NOT_PASSING", "forward-cache manifest is not passing")
    work = {row_key: ChunkWork(chunk=chunk, corpus={}, mask=masks[row_key]) for row_key, chunk in chunks.items()}
    record_counts = Counter()
    vector_hash_counts: dict[str, Counter[str]] = defaultdict(Counter)
    raw_legacy_events = 0
    source_unverified_events = 0
    duplicate_events = 0

    for shard in plan.get("shards") or []:
        shard_id = str(shard.get("shard_id") or "")
        packed_manifest = read_json(forward_root / "shards" / shard_id / "python_forward_cache_packed_manifest.json")
        require(packed_manifest.get("passes") is True, "DTC_LABEL_PACKED_MANIFEST_NOT_PASSING", f"packed manifest not passing: {shard_id}")
        per_embedder = packed_manifest.get("per_embedder") or {}
        for embedder_id in required_embedders:
            entry = per_embedder.get(embedder_id)
            if entry is None:
                continue
            record_path = Path(str(entry.get("record_path") or ""))
            require(record_path.is_file(), "DTC_LABEL_RECORD_PATH_MISSING", f"packed record path missing: {record_path}")
            local_rows = 0
            for line_number, record in iter_jsonl(record_path):
                local_rows += 1
                chunk_info = record.get("chunk") or {}
                key = record.get("key") or {}
                row_key = row_key_from_fields(
                    str(chunk_info.get("task_instance_id") or ""),
                    str(chunk_info.get("workspace_state") or ""),
                    str(key.get("chunk_id") or ""),
                    str(key.get("source_sha256") or ""),
                )
                if row_key not in work:
                    continue
                item = work[row_key]
                require(
                    str(chunk_info.get("candidate_row_id") or "") == str(item.chunk.get("_candidate_row_id") or ""),
                    "DTC_LABEL_RECORD_CANDIDATE_MISMATCH",
                    "packed record candidate_row_id does not match chunk link",
                    {"record_path": str(record_path), "line_number": line_number, "row_key": row_key},
                )
                lineage = guard_packed_record_lineage(record, item.chunk, context=f"{record_path}:{line_number}")
                if lineage["raw_packed_path_metadata_legacy"]:
                    item.raw_legacy_slots.append(embedder_id)
                    raw_legacy_events += 1
                if not bool(record.get("source_readback_verified", True)):
                    item.source_readback_unverified.append(embedder_id)
                    source_unverified_events += 1
                output = record.get("output") or {}
                vector = np.asarray(output.get("vector"), dtype="<f4")
                require(vector.ndim == 1 and vector.size > 0, "DTC_LABEL_VECTOR_MISSING", f"missing vector at {record_path}:{line_number}")
                if embedder_id in item.slot_features:
                    item.duplicate_slots.append(embedder_id)
                    duplicate_events += 1
                features = vector_scalar_features(vector)
                item.slot_features[embedder_id] = features
                item.slot_labels.extend(slot_labels(embedder_id, vector, features))
                vector_sha = str(output.get("vector_sha256") or sha256_bytes(vector.astype("<f4", copy=False).tobytes()))
                item.slot_vector_hashes[embedder_id] = vector_sha
                item.slot_dims[embedder_id] = int(vector.size)
                record_counts[embedder_id] += 1
                vector_hash_counts[embedder_id][vector_sha] += 1
            require(local_rows == int(entry.get("row_count") or 0), "DTC_LABEL_RECORD_ROW_COUNT_MISMATCH", f"record count mismatch: {record_path}")

    mismatches = []
    for row_key, item in work.items():
        present = set(str(slot) for slot in item.mask.get("present_slots") or [])
        actual = set(item.slot_features)
        if present != actual:
            mismatches.append({"row_key": row_key, "mask_present": sorted(present), "actual_present": sorted(actual)})
    require(not mismatches, "DTC_LABEL_MASK_RECORD_MISMATCH", "mask present slots differ from packed records", {"sample": mismatches[:10], "count": len(mismatches)})
    health = vector_space_health_from_hash_counts(required_embedders, record_counts, vector_hash_counts)
    return work, {
        "forward_root": str(forward_root),
        "forward_manifest_sha256": sha256_file(forward_root / "python_forward_cache_corpus_manifest.json"),
        "record_counts": dict(sorted(record_counts.items())),
        "raw_legacy_events": raw_legacy_events,
        "source_unverified_events": source_unverified_events,
        "duplicate_slot_events": duplicate_events,
        "slot_vector_space_health": health,
        "slot_vector_space_blockers": vector_space_blockers(health),
    }


def candidate_relation_labels(chunk: dict[str, Any], corpus_row: dict[str, Any]) -> list[str]:
    labels = ["source_site_relation:candidate_keyed_regeneration"]
    rel = str(chunk.get("relative_path") or "")
    source_path = str(corpus_row.get("source_path") or "")
    if rel != source_path:
        labels.append("source_site_relation:other_file_context")
        return sorted(set(labels))
    labels.append("source_site_relation:changed_file_context")
    selected = corpus_row.get("selected_statement") or {}
    site_start = int(selected.get("start_byte") or corpus_row.get("source_site_effective_offset") or -1)
    site_end = int(selected.get("end_byte") or site_start + 1)
    start = int(chunk.get("byte_start") or -1)
    end = int(chunk.get("byte_end") or -1)
    if start <= site_start < end or start < site_end <= end or (site_start <= start and end <= site_end):
        labels.append("source_site_relation:exact_patch_line_overlap")
    elif site_start >= 0 and start >= 0:
        distance = min(abs(start - site_start), abs(end - site_end))
        if distance <= 500:
            labels.append("source_site_relation:near_patch_site_500b")
        elif distance <= 2_000:
            labels.append("source_site_relation:near_patch_site_2kb")
        else:
            labels.append("source_site_relation:same_file_distant_context")
    return sorted(set(labels))


def outcome_supervision_labels(corpus_row: dict[str, Any]) -> list[str]:
    passed = bool(corpus_row.get("oracle_all_passed"))
    labels = [
        "oracle:pass" if passed else "oracle:fail_or_error",
        "docker:oracle_resolved_clean" if passed else "docker:oracle_unresolved",
        "code_state_outcome:working_as_intended" if passed else "code_state_outcome:failed_or_problematic",
        f"partition:{corpus_row.get('stable_partition')}",
        "leakage_policy:target_side_only",
        "mutation:delete_test_call",
        f"mutation_micro:assertion_duty:{normalize_category(str(corpus_row.get('assertion_duty_sublabel') or 'unknown'))}",
    ]
    return sorted(set(labels))


def missing_slot_labels(mask: dict[str, Any]) -> list[str]:
    labels = []
    reasons = mask.get("missing_slot_reasons") or {}
    for slot in mask.get("missing_slots") or []:
        labels.append(f"slot:{slot}:missing")
        reason = reasons.get(slot) or {}
        code = normalize_category(str(reason.get("error_code") or "unknown"))
        labels.append(f"slot:{slot}:missing_reason:{code}")
    return labels


def category_key(corpus_row: dict[str, Any]) -> str:
    return f"delete_test_call::{normalize_category(str(corpus_row.get('assertion_duty_sublabel') or 'unknown'))}"


def observe_aggregate(agg: CandidateAggregate, row_key: str, item: ChunkWork, surface: list[str], slot_values: list[str], pair_labels: list[str], group_labels: list[str]) -> None:
    agg.chunk_count += 1
    agg.file_paths[str(item.chunk.get("relative_path") or "unknown")] += 1
    agg.surface_counts.update(surface)
    agg.slot_label_counts.update(slot_values)
    agg.pair_label_counts.update(pair_labels)
    agg.group_label_counts.update(group_labels)
    for embedder_id, features in item.slot_features.items():
        agg.slot_feature_counts[embedder_id] += 1
        for name in SCALAR_FEATURES:
            if name in features:
                agg.slot_feature_sums[embedder_id][name] += float(features[name])
    if len(agg.chunk_key_sample) < 5:
        agg.chunk_key_sample.append(row_key)


def failure_candidate_score(chunk: dict[str, Any], corpus_row: dict[str, Any], surface: list[str], slot_values: list[str], pair_labels: list[str]) -> tuple[float, list[str]]:
    score = 0.0
    reasons = []
    relation = candidate_relation_labels(chunk, corpus_row)
    if "source_site_relation:exact_patch_line_overlap" in relation:
        score += 6.0
        reasons.append("source_site_overlap")
    elif "source_site_relation:changed_file_context" in relation:
        score += 3.0
        reasons.append("changed_file_context")
    risky = {
        "ast_surface:assertion_surface",
        "ast_surface:numeric_boundary_or_comparison_surface",
        "ast_surface:boolean_logic_surface",
        "ast_surface:return_value_surface",
        "ast_surface:assignment_state_surface",
    }
    if any(label in risky for label in surface):
        score += 0.5
        reasons.append("risk_surface")
    if any("zero_heavy" in label or "entropy_low" in label or ":missing" in label for label in slot_values):
        score += 0.25
        reasons.append("slot_abnormality_or_missingness")
    high_pairs = sum(1 for label in pair_labels if ":high_cross_slot_delta:" in label or ":extreme_cross_slot_delta:" in label)
    if high_pairs >= 8:
        score += 0.5
        reasons.append("pair_disagreement_cluster")
    return finite_float(score), reasons


def aggregate_row(candidate_id: str, agg: CandidateAggregate) -> dict[str, Any]:
    corpus_row = agg.corpus
    labels = [
        "group_scope:candidate_code_state",
        f"code_state_identity:repo:{normalize_category(str(corpus_row.get('repo') or 'unknown'))}",
        "code_state_identity:mutation_category:delete_test_call",
        f"group_chunk_count:{bucket_count(agg.chunk_count)}",
        f"group_file_count:{bucket_count(len(agg.file_paths))}",
    ]
    labels.extend(f"dominant_surface:{label.split(':', 1)[1]}" for label, _count in agg.surface_counts.most_common(3))
    labels.extend(f"dominant_group_signature:{label.split(':', 1)[1]}" for label, _count in agg.group_label_counts.most_common(3))
    labels = sorted(set(labels + outcome_supervision_labels(corpus_row)))
    live, supervision = split_live_and_supervision_labels(labels)
    slot_means = {}
    for embedder_id, sums in sorted(agg.slot_feature_sums.items()):
        count = max(1, agg.slot_feature_counts[embedder_id])
        slot_means[embedder_id] = {name: finite_float(float(sums[name]) / count) for name in SCALAR_FEATURES if name in sums}
    return {
        "artifact_kind": f"{ARTIFACT_KIND}_candidate_code_state_constellation",
        "schema_version": 1,
        "formula_version": FORMULA_VERSION,
        "candidate_row_id": candidate_id,
        "candidate_corpus_row_id": corpus_row.get("candidate_corpus_row_id"),
        "source_task_id": corpus_row.get("source_task_id"),
        "mutation_category": "delete_test_call",
        "assertion_duty_sublabel": corpus_row.get("assertion_duty_sublabel"),
        "partition": corpus_row.get("stable_partition"),
        "workspace_state": corpus_row.get("workspace_state"),
        "repo": corpus_row.get("repo"),
        "chunk_count": agg.chunk_count,
        "file_count": len(agg.file_paths),
        "oracle_all_passed": bool(corpus_row.get("oracle_all_passed")),
        "oracle_status": corpus_row.get("oracle_status"),
        "labels": labels,
        "live_predictor_labels": live,
        "target_supervision_labels": supervision,
        "surface_counts_top": dict(agg.surface_counts.most_common(16)),
        "slot_label_counts_top": dict(agg.slot_label_counts.most_common(16)),
        "pair_label_counts_top": dict(agg.pair_label_counts.most_common(16)),
        "group_label_counts_top": dict(agg.group_label_counts.most_common(16)),
        "slot_feature_means": slot_means,
        "chunk_key_sample": agg.chunk_key_sample,
        "target_oracle_labels_used_as_features": False,
        "slot_identity_preserved": True,
        "flat_vector_concat_used": False,
    }


def bucket_count(value: int) -> str:
    if value <= 1:
        return "one"
    if value <= 4:
        return "few"
    if value <= 16:
        return "small"
    if value <= 64:
        return "medium"
    if value <= 256:
        return "large"
    return "very_large"


def counter_to_rows(counter: Counter[Any]) -> list[dict[str, Any]]:
    rows = []
    for key, count in sorted(counter.items(), key=lambda item: repr(item[0])):
        if isinstance(key, tuple):
            rows.append({"key": list(key), "count": count})
        else:
            rows.append({"key": key, "count": count})
    return rows


def target_leakage_violations(rows: Iterable[dict[str, Any]]) -> list[dict[str, Any]]:
    bad = []
    for row in rows:
        for label in row.get("live_predictor_labels") or []:
            if not is_live_predictor_label(str(label)):
                bad.append({"row_key": row.get("row_key") or row.get("candidate_row_id"), "label": label})
    return bad


def split_support_gate(corpus: dict[str, dict[str, Any]]) -> dict[str, Any]:
    support = Counter()
    sublabels = set()
    for row in corpus.values():
        sublabel = str(row.get("assertion_duty_sublabel") or "unknown")
        sublabels.add(sublabel)
        status = "pass" if bool(row.get("oracle_all_passed")) else "fail"
        support[(str(row.get("stable_partition")), sublabel, status)] += 1
    failure_bearing = sorted(
        sublabel
        for sublabel in sublabels
        if any(support[(partition, sublabel, "fail")] > 0 for partition in ("train", "calibration", "holdout"))
    )
    missing = [
        sublabel
        for sublabel in failure_bearing
        if support[("train", sublabel, "fail")] == 0 or support[("calibration", sublabel, "fail")] == 0
    ]
    return {
        "support_rows": counter_to_rows(support),
        "failure_bearing_sublabels": failure_bearing,
        "failure_bearing_sublabels_missing_train_or_calibration_failure": missing,
        "passes": not missing and bool(failure_bearing),
    }
