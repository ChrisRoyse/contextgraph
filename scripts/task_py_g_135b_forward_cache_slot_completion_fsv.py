#!/usr/bin/env python3
"""TASK-PY-G-135B forward-cache slot-completion FSV.

The #612 lineage audit proved the 300x8 Python forward cache is real but not
complete for every active embedder slot. This harness classifies that gap
without reading packed vector payloads:

* canonical AST rows define the expected chunk universe;
* forward-cache index rows define present chunk/embedder slots;
* quarantine rows define missing slots with typed reasons.

It writes a retained FSV report plus a row-level completeness mask. The mask is
the legal handoff to pair/constellation mining: partial rows may contribute
missingness-aware features, but must not be represented as complete all-pair
constellations.
"""

from __future__ import annotations

import datetime as dt
import hashlib
import json
import math
import os
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any


TASK_ID = "TASK-PY-G-135B"
WEAK_CELLS = ("delete_test_call", "off_by_one", "subtle_flip")
REQUIRED_EMBEDDERS = ("e1", "e2", "e3", "e4", "e6", "e7", "e8", "e9", "e10", "e12", "e13", "e14")
EXPECTED_ACTIVE_PAIR_COUNT = len(REQUIRED_EMBEDDERS) * (len(REQUIRED_EMBEDDERS) - 1) // 2
FULL_PANEL_PAIR_COUNT_15_SLOTS = 15 * 14 // 2

AST_CHUNKS_PATH = Path(
    os.environ.get(
        "CG_AST_CHUNKS",
        "/zfs/archive/contextgraph/corpus/python-ast-chunks-v1/python_ast_chunks.jsonl",
    )
)
FORWARD_ROOT = Path(
    os.environ.get(
        "CG_FORWARD_CACHE",
        "/zfs/archive/contextgraph/corpus/python-forward-cache-v1-aiwonder-prod-20260515-191116Z",
    )
)
FSV_ROOT = Path(os.environ.get("CG_FSV", "/zfs/archive/contextgraph/fsv")) / "task-py-g-135b-forward-cache-slot-completion-fsv"
MASK_ROOT = Path(
    os.environ.get(
        "CG_FORWARD_CACHE_COMPLETENESS_ROOT",
        "/zfs/archive/contextgraph/state/forward-cache-completeness/task-py-g-135b-forward-cache-slot-completion-v1",
    )
)

REPORT_NAME = "task_py_g_135b_forward_cache_slot_completion_fsv.json"
READBACK_NAME = "task_py_g_135b_forward_cache_slot_completion_readback.json"
MASK_NAME = "forward_cache_slot_completeness_mask.jsonl"
MASK_MANIFEST_NAME = "forward_cache_slot_completeness_manifest.json"


def utc_now_iso() -> str:
    return dt.datetime.now(dt.UTC).isoformat().replace("+00:00", "Z")


def utc_stamp() -> str:
    return dt.datetime.now(dt.UTC).strftime("%Y%m%dT%H%M%SZ")


def sha256_bytes(payload: bytes) -> str:
    return "sha256:" + hashlib.sha256(payload).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return "sha256:" + digest.hexdigest()


def sha256_text(value: Any) -> str:
    return sha256_bytes(json.dumps(value, sort_keys=True, separators=(",", ":")).encode("utf-8"))


def read_json(path: Path) -> Any:
    with path.open("r", encoding="utf-8") as handle:
        return json.load(handle)


def write_json_0600(path: Path, payload: dict[str, Any]) -> None:
    path.write_text(json.dumps(payload, indent=2, sort_keys=True), encoding="utf-8")
    os.chmod(path, 0o600)


def iter_jsonl(path: Path):
    with path.open("r", encoding="utf-8", errors="replace") as handle:
        for line_number, raw in enumerate(handle, start=1):
            line = raw.strip()
            if not line:
                continue
            yield line_number, json.loads(line)


def stable_partition(row_key: str) -> str:
    bucket = int(hashlib.sha256(row_key.encode("utf-8")).hexdigest()[:8], 16) % 10
    if bucket == 0:
        return "holdout"
    if bucket == 1:
        return "calibration"
    return "train"


def length_bucket(byte_count: int) -> str:
    if byte_count <= 32_768:
        return "000000-032768"
    if byte_count <= 65_536:
        return "032769-065536"
    if byte_count <= 131_072:
        return "065537-131072"
    if byte_count <= 262_144:
        return "131073-262144"
    return "262145-plus"


def row_key4(row: dict[str, Any]) -> str:
    return "|".join(
        [
            str(row.get("task_instance_id") or ""),
            str(row.get("workspace_state") or ""),
            str(row.get("chunk_id") or ""),
            str(row.get("source_sha256") or ""),
        ]
    )


def row_key5(row: dict[str, Any]) -> tuple[str, str, str, str, str]:
    return (
        str(row.get("chunk_id") or ""),
        str(row.get("file_sha256") or ""),
        str(row.get("source_sha256") or ""),
        str(row.get("task_instance_id") or ""),
        str(row.get("workspace_state") or ""),
    )


def slot_bit_map() -> dict[str, int]:
    return {slot: idx for idx, slot in enumerate(REQUIRED_EMBEDDERS)}


def present_slots(mask: int, bits: dict[str, int]) -> list[str]:
    return [slot for slot, bit in bits.items() if mask & (1 << bit)]


def missing_slots(mask: int, bits: dict[str, int]) -> list[str]:
    return [slot for slot, bit in bits.items() if not (mask & (1 << bit))]


def classify_recovery(row: dict[str, Any]) -> str:
    code = str(row.get("error_code") or "")
    source_bytes = int(row.get("source_text_bytes") or 0)
    max_bytes = int(row.get("max_forward_source_text_bytes") or 0)
    upper = code.upper()
    if "OOM" in upper or "OUT_OF_MEMORY" in upper:
        return "rerun_lower_batch_or_single_row"
    if "TOKEN_TOO_LONG" in upper or "INPUT_TOO_LARGE" in upper or (max_bytes > 0 and source_bytes > max_bytes):
        return "requires_deterministic_subchunk_forwarding_or_legal_missingness_mask"
    return "requires_manual_quarantine_root_cause"


def scan_ast_chunks() -> tuple[dict[str, Any], dict[tuple[str, str, str, str, str], dict[str, Any]], dict[str, list[tuple[str, str, str, str, str]]]]:
    meta: dict[tuple[str, str, str, str, str], dict[str, Any]] = {}
    qkey_to_keys: dict[str, list[tuple[str, str, str, str, str]]] = defaultdict(list)
    category_counts: Counter[str] = Counter()
    split_counts: dict[str, Counter[str]] = defaultdict(Counter)
    chunk_kind_counts: Counter[str] = Counter()
    workspace_counts: Counter[str] = Counter()
    source_bytes_by_category: Counter[str] = Counter()
    source_bytes_bucket_by_category: dict[str, Counter[str]] = defaultdict(Counter)
    mounted_absolute_path_rows = 0
    mounted_dataschema_rows = 0
    legacy_provenance_rows = 0
    duplicate_key5 = 0
    examples: list[dict[str, Any]] = []

    for _, row in iter_jsonl(AST_CHUNKS_PATH):
        key5 = row_key5(row)
        key4 = row_key4(row)
        category = str(row.get("mutation_category") or "unknown")
        chunk_kind = str(row.get("chunk_kind") or "unknown")
        workspace = str(row.get("workspace_state") or "unknown")
        source_text = str(row.get("source_text") or "")
        source_bytes = len(source_text.encode("utf-8"))
        split = stable_partition(key4)
        if key5 in meta:
            duplicate_key5 += 1
            continue
        mounted_active = str(row.get("absolute_path") or "").startswith(("/mnt/d/", "/mnt/c/"))
        mounted_schema = str(row.get("dataschema") or "").startswith(("/mnt/d/", "/mnt/c/"))
        mounted_absolute_path_rows += 1 if mounted_active else 0
        mounted_dataschema_rows += 1 if mounted_schema else 0
        legacy_provenance_rows += 1 if str(row.get("legacy_provenance_path") or "").startswith(("/mnt/d/", "/mnt/c/")) else 0
        if (mounted_active or mounted_schema) and len(examples) < 8:
            examples.append(
                {
                    "row_key": key4,
                    "absolute_path": row.get("absolute_path"),
                    "dataschema": row.get("dataschema"),
                    "legacy_provenance_path": row.get("legacy_provenance_path"),
                    "mutation_category": category,
                }
            )
        category_counts[category] += 1
        split_counts[category][split] += 1
        chunk_kind_counts[chunk_kind] += 1
        workspace_counts[workspace] += 1
        source_bytes_by_category[category] += source_bytes
        source_bytes_bucket_by_category[category][length_bucket(source_bytes)] += 1
        meta[key5] = {
            "row_key": key4,
            "chunk_id": key5[0],
            "file_sha256": key5[1],
            "source_sha256": key5[2],
            "task_instance_id": key5[3],
            "workspace_state": key5[4],
            "repo": str(row.get("repo") or ""),
            "relative_path": str(row.get("relative_path") or ""),
            "mutation_category": category,
            "chunk_kind": chunk_kind,
            "node_kind": str(row.get("node_kind") or ""),
            "source_text_bytes": source_bytes,
            "source_text_sha256": str(row.get("source_text_sha256") or ""),
            "partition": split,
        }
        qkey_to_keys[key4].append(key5)

    ambiguous_qkeys = sum(1 for keys in qkey_to_keys.values() if len(keys) != 1)
    return (
        {
            "path": str(AST_CHUNKS_PATH),
            "sha256": sha256_file(AST_CHUNKS_PATH),
            "row_count": len(meta),
            "duplicate_key5_rows_skipped": duplicate_key5,
            "mounted_active_absolute_path_rows": mounted_absolute_path_rows,
            "mounted_active_dataschema_rows": mounted_dataschema_rows,
            "legacy_provenance_path_rows": legacy_provenance_rows,
            "mounted_active_examples": examples,
            "category_counts": dict(sorted(category_counts.items())),
            "split_counts_by_category": {category: dict(sorted(counter.items())) for category, counter in sorted(split_counts.items())},
            "chunk_kind_counts_top": dict(chunk_kind_counts.most_common(30)),
            "workspace_state_counts": dict(sorted(workspace_counts.items())),
            "source_text_bytes_by_category": dict(sorted(source_bytes_by_category.items())),
            "source_text_byte_buckets_by_category": {
                category: dict(sorted(counter.items())) for category, counter in sorted(source_bytes_bucket_by_category.items())
            },
            "row_key4_unique_count": len(qkey_to_keys),
            "row_key4_ambiguous_count": ambiguous_qkeys,
        },
        meta,
        qkey_to_keys,
    )


def scan_forward_manifest() -> dict[str, Any]:
    manifest_path = FORWARD_ROOT / "python_forward_cache_corpus_manifest.json"
    checkpoint_path = FORWARD_ROOT / "python_forward_cache_checkpoint.json"
    plan_path = FORWARD_ROOT / "python_forward_cache_shard_plan.json"
    manifest = read_json(manifest_path)
    summary = dict(manifest.get("summary") or {})
    return {
        "root": str(FORWARD_ROOT),
        "manifest_path": str(manifest_path),
        "manifest_sha256": sha256_file(manifest_path),
        "checkpoint_path": str(checkpoint_path),
        "checkpoint_sha256": sha256_file(checkpoint_path) if checkpoint_path.exists() else None,
        "shard_plan_path": str(plan_path),
        "shard_plan_sha256": sha256_file(plan_path) if plan_path.exists() else None,
        "status": manifest.get("status"),
        "passes": manifest.get("passes"),
        "summary": summary,
        "shard_count": len(manifest.get("shards") or []),
    }


def scan_forward_index(meta: dict[tuple[str, str, str, str, str], dict[str, Any]]) -> tuple[dict[str, Any], dict[tuple[str, str, str, str, str], int]]:
    bits = slot_bit_map()
    masks: dict[tuple[str, str, str, str, str], int] = {key: 0 for key in meta}
    index_rows = 0
    known_rows = 0
    unknown_rows = 0
    duplicate_slot_rows = 0
    slot_counts: Counter[str] = Counter()
    vector_len_by_slot: dict[str, Counter[int]] = defaultdict(Counter)
    unknown_samples: list[dict[str, Any]] = []

    for index_path in sorted((FORWARD_ROOT / "shards").glob("shard-*/python_forward_cache_index.jsonl")):
        for _, row in iter_jsonl(index_path):
            index_rows += 1
            slot = str(row.get("embedder_id") or "")
            key5 = row_key5(row)
            bit = bits.get(slot)
            if key5 not in masks or bit is None:
                unknown_rows += 1
                if len(unknown_samples) < 8:
                    unknown_samples.append(
                        {
                            "index_path": str(index_path),
                            "embedder_id": slot,
                            "chunk_id": row.get("chunk_id"),
                            "task_instance_id": row.get("task_instance_id"),
                            "workspace_state": row.get("workspace_state"),
                        }
                    )
                continue
            old = masks[key5]
            masks[key5] |= 1 << bit
            if old & (1 << bit):
                duplicate_slot_rows += 1
            else:
                known_rows += 1
                slot_counts[slot] += 1
                vector_len_by_slot[slot][int(row.get("vector_len") or 0)] += 1

    return (
        {
            "index_rows_scanned": index_rows,
            "known_unique_chunk_embedder_rows": known_rows,
            "unknown_index_rows": unknown_rows,
            "duplicate_slot_index_rows": duplicate_slot_rows,
            "present_rows_by_embedder": dict(sorted(slot_counts.items())),
            "vector_len_distribution_by_embedder": {
                slot: dict(sorted(counter.items())) for slot, counter in sorted(vector_len_by_slot.items())
            },
            "unknown_index_row_samples": unknown_samples,
        },
        masks,
    )


def scan_quarantines(
    meta: dict[tuple[str, str, str, str, str], dict[str, Any]],
    qkey_to_keys: dict[str, list[tuple[str, str, str, str, str]]],
) -> tuple[dict[str, Any], dict[tuple[tuple[str, str, str, str, str], str], dict[str, Any]]]:
    by_pair: dict[tuple[tuple[str, str, str, str, str], str], dict[str, Any]] = {}
    rows = 0
    joined_rows = 0
    unknown_rows = 0
    ambiguous_rows = 0
    legacy_absolute_path_rows = 0
    duplicates = 0
    by_embedder: Counter[str] = Counter()
    by_error: Counter[str] = Counter()
    by_recovery: Counter[str] = Counter()
    by_category: dict[str, Counter[str]] = defaultdict(Counter)
    by_split: dict[str, Counter[str]] = defaultdict(Counter)
    by_chunk_kind: dict[str, Counter[str]] = defaultdict(Counter)
    by_source_bucket: dict[str, Counter[str]] = defaultdict(Counter)
    weak_cell_quarantines: dict[str, Counter[str]] = defaultdict(Counter)
    samples: list[dict[str, Any]] = []

    for qpath in sorted((FORWARD_ROOT / "shards").glob("shard-*/python_forward_cache_quarantine.jsonl")):
        for _, row in iter_jsonl(qpath):
            rows += 1
            slot = str(row.get("embedder_id") or "unknown")
            error = str(row.get("error_code") or "UNKNOWN_QUARANTINE_ERROR")
            qkey = row_key4(row)
            keys = qkey_to_keys.get(qkey) or []
            if not keys:
                unknown_rows += 1
                info = None
            elif len(keys) > 1:
                ambiguous_rows += 1
                info = None
            else:
                key5 = keys[0]
                info = meta[key5]
                pair_key = (key5, slot)
                if pair_key in by_pair:
                    duplicates += 1
                by_pair[pair_key] = {
                    "status": "quarantined",
                    "error_code": error,
                    "reason": str(row.get("reason") or ""),
                    "recovery_class": classify_recovery(row),
                    "source_text_bytes": int(row.get("source_text_bytes") or info.get("source_text_bytes") or 0),
                    "max_forward_source_text_bytes": row.get("max_forward_source_text_bytes"),
                    "quarantine_path": str(qpath),
                }
                joined_rows += 1
            recovery = classify_recovery(row)
            source_bytes = int(row.get("source_text_bytes") or (info or {}).get("source_text_bytes") or 0)
            category = str((info or {}).get("mutation_category") or "unknown")
            split = str((info or {}).get("partition") or "unknown")
            chunk_kind = str((info or {}).get("chunk_kind") or "unknown")
            by_embedder[slot] += 1
            by_error[error] += 1
            by_recovery[recovery] += 1
            by_category[category][slot] += 1
            by_split[split][slot] += 1
            by_chunk_kind[chunk_kind][slot] += 1
            by_source_bucket[length_bucket(source_bytes)][slot] += 1
            if category in WEAK_CELLS:
                weak_cell_quarantines[category][slot] += 1
            absolute_path = str(row.get("absolute_path") or "")
            if absolute_path.startswith(("/mnt/d/", "/mnt/c/")):
                legacy_absolute_path_rows += 1
            if len(samples) < 16:
                samples.append(
                    {
                        "embedder_id": slot,
                        "error_code": error,
                        "recovery_class": recovery,
                        "row_key": qkey,
                        "mutation_category": category,
                        "partition": split,
                        "chunk_kind": chunk_kind,
                        "source_text_bytes": source_bytes,
                        "max_forward_source_text_bytes": row.get("max_forward_source_text_bytes"),
                        "absolute_path_is_legacy_metadata": absolute_path.startswith(("/mnt/d/", "/mnt/c/")),
                    }
                )

    return (
        {
            "quarantine_rows_scanned": rows,
            "joined_quarantine_rows": joined_rows,
            "unknown_quarantine_rows": unknown_rows,
            "ambiguous_quarantine_rows": ambiguous_rows,
            "duplicate_quarantine_pairs": duplicates,
            "legacy_absolute_path_metadata_rows": legacy_absolute_path_rows,
            "quarantine_rows_by_embedder": dict(sorted(by_embedder.items())),
            "quarantine_rows_by_error_code": dict(sorted(by_error.items())),
            "quarantine_rows_by_recovery_class": dict(sorted(by_recovery.items())),
            "quarantine_rows_by_category_embedder": {
                category: dict(sorted(counter.items())) for category, counter in sorted(by_category.items())
            },
            "quarantine_rows_by_split_embedder": {
                split: dict(sorted(counter.items())) for split, counter in sorted(by_split.items())
            },
            "quarantine_rows_by_chunk_kind_embedder_top": {
                chunk_kind: dict(sorted(counter.items()))
                for chunk_kind, counter in sorted(by_chunk_kind.items(), key=lambda item: sum(item[1].values()), reverse=True)[:30]
            },
            "quarantine_rows_by_source_byte_bucket_embedder": {
                bucket: dict(sorted(counter.items())) for bucket, counter in sorted(by_source_bucket.items())
            },
            "weak_cell_quarantines_by_embedder": {
                category: dict(sorted(counter.items())) for category, counter in sorted(weak_cell_quarantines.items())
            },
            "samples": samples,
        },
        by_pair,
    )


def compute_and_write_mask(
    run_dir: Path,
    meta: dict[tuple[str, str, str, str, str], dict[str, Any]],
    masks: dict[tuple[str, str, str, str, str], int],
    quarantine_by_pair: dict[tuple[tuple[str, str, str, str, str], str], dict[str, Any]],
) -> tuple[dict[str, Any], Path, Path]:
    MASK_ROOT.mkdir(parents=True, exist_ok=True)
    mask_path = MASK_ROOT / MASK_NAME
    mask_manifest_path = MASK_ROOT / MASK_MANIFEST_NAME
    run_mask_copy = run_dir / MASK_NAME
    bits = slot_bit_map()
    row_count = 0
    full_rows = 0
    partial_rows = 0
    zero_rows = 0
    missing_slot_events = 0
    quarantined_missing_slot_events = 0
    unaccounted_missing_slot_events = 0
    row_present_dist: Counter[int] = Counter()
    category_present_dist: dict[str, Counter[int]] = defaultdict(Counter)
    weak_cell_present_dist: dict[str, Counter[int]] = defaultdict(Counter)
    weak_cell_split_present_dist: dict[str, dict[str, Counter[int]]] = defaultdict(lambda: defaultdict(Counter))
    weak_cell_missing_by_slot: dict[str, Counter[str]] = defaultdict(Counter)
    missing_by_slot: Counter[str] = Counter()
    missing_by_category: dict[str, Counter[str]] = defaultdict(Counter)
    missing_by_split: dict[str, Counter[str]] = defaultdict(Counter)
    missing_by_chunk_kind: dict[str, Counter[str]] = defaultdict(Counter)
    missing_by_recovery_class: Counter[str] = Counter()
    mask_samples: list[dict[str, Any]] = []

    with mask_path.open("w", encoding="utf-8") as out, run_mask_copy.open("w", encoding="utf-8") as run_out:
        for key5, info in sorted(meta.items(), key=lambda item: item[1]["row_key"]):
            row_count += 1
            mask = masks.get(key5, 0)
            present = present_slots(mask, bits)
            missing = missing_slots(mask, bits)
            present_count = len(present)
            row_present_dist[present_count] += 1
            category = str(info["mutation_category"])
            split = str(info["partition"])
            chunk_kind = str(info["chunk_kind"])
            category_present_dist[category][present_count] += 1
            if category in WEAK_CELLS:
                weak_cell_present_dist[category][present_count] += 1
                weak_cell_split_present_dist[category][split][present_count] += 1
            full_rows += 1 if present_count == len(REQUIRED_EMBEDDERS) else 0
            partial_rows += 1 if 0 < present_count < len(REQUIRED_EMBEDDERS) else 0
            zero_rows += 1 if present_count == 0 else 0
            missing_reasons: dict[str, dict[str, Any]] = {}
            for slot in missing:
                missing_slot_events += 1
                missing_by_slot[slot] += 1
                missing_by_category[category][slot] += 1
                missing_by_split[split][slot] += 1
                missing_by_chunk_kind[chunk_kind][slot] += 1
                if category in WEAK_CELLS:
                    weak_cell_missing_by_slot[category][slot] += 1
                quarantine = quarantine_by_pair.get((key5, slot))
                if quarantine:
                    quarantined_missing_slot_events += 1
                    recovery = str(quarantine.get("recovery_class") or "unknown")
                    missing_by_recovery_class[recovery] += 1
                    missing_reasons[slot] = {
                        "status": "quarantined",
                        "error_code": quarantine.get("error_code"),
                        "recovery_class": recovery,
                        "source_text_bytes": quarantine.get("source_text_bytes"),
                        "max_forward_source_text_bytes": quarantine.get("max_forward_source_text_bytes"),
                    }
                else:
                    unaccounted_missing_slot_events += 1
                    missing_by_recovery_class["unaccounted_missing_forward_slot"] += 1
                    missing_reasons[slot] = {
                        "status": "missing",
                        "error_code": "UNACCOUNTED_MISSING_FORWARD_SLOT",
                        "recovery_class": "investigate_missing_index_and_quarantine_gap",
                    }
            available_pair_count = math.comb(present_count, 2) if present_count >= 2 else 0
            complete = present_count == len(REQUIRED_EMBEDDERS)
            row = {
                "schema_version": 1,
                "artifact_kind": "python_forward_cache_slot_completeness_mask",
                "task_id": TASK_ID,
                "mask_policy_version": "slot_identity_preserved_missingness_mask_v1",
                "row_key": info["row_key"],
                "chunk_id": info["chunk_id"],
                "file_sha256": info["file_sha256"],
                "source_sha256": info["source_sha256"],
                "task_instance_id": info["task_instance_id"],
                "workspace_state": info["workspace_state"],
                "repo": info["repo"],
                "relative_path": info["relative_path"],
                "mutation_category": category,
                "partition": split,
                "chunk_kind": chunk_kind,
                "node_kind": info["node_kind"],
                "source_text_bytes": info["source_text_bytes"],
                "required_active_embedder_slots": list(REQUIRED_EMBEDDERS),
                "present_slots": present,
                "missing_slots": missing,
                "missing_slot_reasons": missing_reasons,
                "present_slot_count": present_count,
                "expected_active_embedder_count": len(REQUIRED_EMBEDDERS),
                "complete_12_slot_forward_constellation": complete,
                "expected_active_forward_pair_count": EXPECTED_ACTIVE_PAIR_COUNT,
                "available_active_forward_pair_count": available_pair_count,
                "all_active_forward_pairs_available": available_pair_count == EXPECTED_ACTIVE_PAIR_COUNT,
                "full_105_panel_pair_count": FULL_PANEL_PAIR_COUNT_15_SLOTS,
                "full_105_panel_pair_claimed_by_forward_cache": False,
                "all_pair_feature_policy": "block_complete_pair_claims_when_missing_slots_non_empty",
                "zero_vector_fill_used": False,
                "flat_vector_concat_used": False,
                "slot_identity_preserved": True,
            }
            text = json.dumps(row, sort_keys=True, separators=(",", ":"))
            out.write(text + "\n")
            run_out.write(text + "\n")
            if missing and len(mask_samples) < 16:
                mask_samples.append(row)

    manifest = {
        "schema_version": 1,
        "artifact_kind": "python_forward_cache_slot_completeness_manifest",
        "task_id": TASK_ID,
        "created_at_utc": utc_now_iso(),
        "mask_path": str(mask_path),
        "mask_sha256": sha256_file(mask_path),
        "run_mask_copy_path": str(run_mask_copy),
        "run_mask_copy_sha256": sha256_file(run_mask_copy),
        "row_count": row_count,
        "required_active_embedder_slots": list(REQUIRED_EMBEDDERS),
        "expected_active_pair_count": EXPECTED_ACTIVE_PAIR_COUNT,
        "full_105_panel_pair_count": FULL_PANEL_PAIR_COUNT_15_SLOTS,
        "full_rows": full_rows,
        "partial_rows": partial_rows,
        "zero_slot_rows": zero_rows,
        "missing_slot_events": missing_slot_events,
        "quarantined_missing_slot_events": quarantined_missing_slot_events,
        "unaccounted_missing_slot_events": unaccounted_missing_slot_events,
        "present_slot_count_distribution": dict(sorted(row_present_dist.items())),
        "present_slot_count_distribution_by_category": {
            category: dict(sorted(counter.items())) for category, counter in sorted(category_present_dist.items())
        },
        "weak_cell_present_slot_count_distribution": {
            category: dict(sorted(counter.items())) for category, counter in sorted(weak_cell_present_dist.items())
        },
        "weak_cell_present_slot_count_distribution_by_split": {
            category: {split: dict(sorted(counter.items())) for split, counter in sorted(split_map.items())}
            for category, split_map in sorted(weak_cell_split_present_dist.items())
        },
        "missing_slots_by_embedder": dict(sorted(missing_by_slot.items())),
        "missing_slots_by_category_embedder": {
            category: dict(sorted(counter.items())) for category, counter in sorted(missing_by_category.items())
        },
        "missing_slots_by_split_embedder": {
            split: dict(sorted(counter.items())) for split, counter in sorted(missing_by_split.items())
        },
        "missing_slots_by_chunk_kind_embedder_top": {
            chunk_kind: dict(sorted(counter.items()))
            for chunk_kind, counter in sorted(missing_by_chunk_kind.items(), key=lambda item: sum(item[1].values()), reverse=True)[:30]
        },
        "weak_cell_missing_slots_by_embedder": {
            category: dict(sorted(counter.items())) for category, counter in sorted(weak_cell_missing_by_slot.items())
        },
        "missing_slots_by_recovery_class": dict(sorted(missing_by_recovery_class.items())),
        "incomplete_row_samples": mask_samples,
    }
    write_json_0600(mask_manifest_path, manifest)
    write_json_0600(run_dir / MASK_MANIFEST_NAME, manifest)
    return manifest, mask_path, mask_manifest_path


def make_decision(ast_scan: dict[str, Any], manifest_scan: dict[str, Any], index_scan: dict[str, Any], quarantine_scan: dict[str, Any], mask_manifest: dict[str, Any]) -> dict[str, Any]:
    expected_chunks = int((manifest_scan.get("summary") or {}).get("chunk_count") or ast_scan["row_count"])
    expected_pairs = expected_chunks * len(REQUIRED_EMBEDDERS)
    present = int(index_scan["known_unique_chunk_embedder_rows"])
    missing = int(mask_manifest["missing_slot_events"])
    unaccounted = int(mask_manifest["unaccounted_missing_slot_events"])
    quarantined = int(mask_manifest["quarantined_missing_slot_events"])
    mounted_active_rows = int(ast_scan["mounted_active_absolute_path_rows"]) + int(ast_scan["mounted_active_dataschema_rows"])
    typed_blockers: list[str] = []
    if mounted_active_rows:
        typed_blockers.append("TASK_PY_G_135B_ACTIVE_AST_STILL_HAS_MOUNTED_PATHS")
    if expected_pairs != present + missing:
        typed_blockers.append("TASK_PY_G_135B_PRESENT_PLUS_MISSING_DOES_NOT_EQUAL_EXPECTED")
    if unaccounted:
        typed_blockers.append("TASK_PY_G_135B_UNACCOUNTED_MISSING_FORWARD_SLOTS")
    if int(quarantine_scan["unknown_quarantine_rows"]) or int(quarantine_scan["ambiguous_quarantine_rows"]):
        typed_blockers.append("TASK_PY_G_135B_QUARANTINE_ROWS_NOT_JOINED_TO_CANONICAL_AST")
    if missing:
        typed_blockers.append("TASK_PY_G_135B_FORWARD_CACHE_IS_LEGAL_MASK_NOT_COMPLETE_COVERAGE")

    legal_mask_promoted = (
        mounted_active_rows == 0
        and expected_pairs == present + missing
        and unaccounted == 0
        and quarantined == missing
        and Path(str(mask_manifest["mask_path"])).exists()
    )
    complete_coverage = missing == 0
    return {
        "all_passed": legal_mask_promoted or complete_coverage,
        "slot_completion_promoted": complete_coverage,
        "legal_missingness_mask_promoted": legal_mask_promoted,
        "expected_chunks": expected_chunks,
        "expected_chunk_embedder_pairs": expected_pairs,
        "present_chunk_embedder_pairs": present,
        "missing_chunk_embedder_pairs": missing,
        "quarantined_missing_chunk_embedder_pairs": quarantined,
        "unaccounted_missing_chunk_embedder_pairs": unaccounted,
        "active_forward_pair_universe_per_complete_row": EXPECTED_ACTIVE_PAIR_COUNT,
        "full_panel_pair_universe_per_complete_15_slot_row": FULL_PANEL_PAIR_COUNT_15_SLOTS,
        "full_105_pair_claims_allowed_from_forward_cache_only": False,
        "typed_blockers": typed_blockers,
        "gate_rerun_condition": (
            "rerun #392/#611/#150/#610 only with the completeness mask joined into pair/constellation consumers; "
            "complete all-pair claims are forbidden for rows with missing_slots"
        )
        if legal_mask_promoted and not complete_coverage
        else "complete coverage available; rerun downstream gates after readback",
        "root_cause_decision": (
            "long-source quarantines are the missing-slot substrate; old mounted paths remain historical metadata in quarantines/packed records but no active AST dereference is required"
            if legal_mask_promoted and not complete_coverage
            else "forward-cache slot coverage complete"
            if complete_coverage
            else "forward-cache missingness is not fully accounted"
        ),
    }


def independent_readback(report_path: Path, mask_manifest_path: Path) -> dict[str, Any]:
    report = read_json(report_path)
    manifest = read_json(mask_manifest_path)
    mask_path = Path(str(manifest["mask_path"]))
    line_count = 0
    bad_complete_rows = 0
    bad_missing_reason_rows = 0
    zero_vector_fill_rows = 0
    full_pair_claim_rows = 0
    with mask_path.open("r", encoding="utf-8") as handle:
        for raw in handle:
            if not raw.strip():
                continue
            line_count += 1
            row = json.loads(raw)
            missing = row.get("missing_slots") or []
            if missing and bool(row.get("complete_12_slot_forward_constellation")):
                bad_complete_rows += 1
            reasons = row.get("missing_slot_reasons") or {}
            if missing and sorted(reasons) != sorted(missing):
                bad_missing_reason_rows += 1
            if row.get("zero_vector_fill_used") is not False:
                zero_vector_fill_rows += 1
            if row.get("full_105_panel_pair_claimed_by_forward_cache") is not False:
                full_pair_claim_rows += 1
    return {
        "artifact_kind": "python_forward_cache_slot_completion_readback",
        "task_id": TASK_ID,
        "report_path": str(report_path),
        "report_sha256": sha256_file(report_path),
        "mask_manifest_path": str(mask_manifest_path),
        "mask_manifest_sha256": sha256_file(mask_manifest_path),
        "mask_path": str(mask_path),
        "mask_sha256": sha256_file(mask_path),
        "mask_line_count": line_count,
        "expected_line_count": int(report["ast_scan"]["row_count"]),
        "mask_line_count_matches_ast": line_count == int(report["ast_scan"]["row_count"]),
        "bad_complete_rows_with_missing_slots": bad_complete_rows,
        "bad_missing_reason_rows": bad_missing_reason_rows,
        "zero_vector_fill_rows": zero_vector_fill_rows,
        "full_105_pair_claim_rows": full_pair_claim_rows,
        "readback_passed": (
            line_count == int(report["ast_scan"]["row_count"])
            and bad_complete_rows == 0
            and bad_missing_reason_rows == 0
            and zero_vector_fill_rows == 0
            and full_pair_claim_rows == 0
        ),
    }


def main() -> int:
    run_dir = FSV_ROOT / f"run-{utc_stamp()}"
    run_dir.mkdir(parents=True, exist_ok=True)
    ast_scan, meta, qkey_to_keys = scan_ast_chunks()
    manifest_scan = scan_forward_manifest()
    index_scan, masks = scan_forward_index(meta)
    quarantine_scan, quarantine_by_pair = scan_quarantines(meta, qkey_to_keys)
    mask_manifest, mask_path, mask_manifest_path = compute_and_write_mask(run_dir, meta, masks, quarantine_by_pair)
    decision = make_decision(ast_scan, manifest_scan, index_scan, quarantine_scan, mask_manifest)
    report = {
        "schema_version": 1,
        "artifact_kind": "task_py_g_135b_forward_cache_slot_completion_fsv",
        "task_id": TASK_ID,
        "created_at_utc": utc_now_iso(),
        "inputs": {
            "ast_chunks_path": str(AST_CHUNKS_PATH),
            "forward_root": str(FORWARD_ROOT),
            "fsv_run_dir": str(run_dir),
            "mask_root": str(MASK_ROOT),
        },
        "required_active_embedders": list(REQUIRED_EMBEDDERS),
        "expected_active_pair_count_per_complete_row": EXPECTED_ACTIVE_PAIR_COUNT,
        "full_105_panel_pair_count_per_15_slot_row": FULL_PANEL_PAIR_COUNT_15_SLOTS,
        "ast_scan": ast_scan,
        "forward_manifest_scan": manifest_scan,
        "forward_index_scan": index_scan,
        "quarantine_scan": quarantine_scan,
        "mask_manifest": mask_manifest,
        "decision": decision,
        "all_passed": bool(decision["all_passed"]),
    }
    report_path = run_dir / REPORT_NAME
    write_json_0600(report_path, report)
    readback = independent_readback(report_path, mask_manifest_path)
    readback_path = run_dir / READBACK_NAME
    write_json_0600(readback_path, readback)
    report["readback"] = readback
    report["all_passed"] = bool(decision["all_passed"]) and bool(readback["readback_passed"])
    write_json_0600(report_path, report)
    print(
        json.dumps(
            {
                "all_passed": report["all_passed"],
                "report_path": str(report_path),
                "report_sha256": sha256_file(report_path),
                "readback_path": str(readback_path),
                "readback_sha256": sha256_file(readback_path),
                "mask_path": str(mask_path),
                "mask_manifest_path": str(mask_manifest_path),
                "decision": report["decision"],
            },
            indent=2,
            sort_keys=True,
        )
    )
    return 0 if report["all_passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
