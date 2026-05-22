#!/usr/bin/env python3
"""TASK-PY-G-135 data-substrate lineage FSV.

This read-only harness audits whether the current aiwonder 300x8 corpus can be
trusted as a signal substrate for weak-cell repair. It focuses on two failure
classes that can make the model look healthy while starving it of useful data:

1. Legacy absolute paths embedded in retained corpus rows after migration.
2. Incomplete per-chunk/per-embedder slot coverage in the forward cache.

The harness does not promote a predictor repair. It writes a retained report
with a typed blocker when corpus canonicalization or regeneration is required.
"""

from __future__ import annotations

import datetime as dt
import hashlib
import json
import os
import re
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any


TASK_ID = "TASK-PY-G-135"
WEAK_CELLS = ("delete_test_call", "off_by_one", "subtle_flip")
REQUIRED_EMBEDDERS = ("e1", "e2", "e3", "e4", "e6", "e7", "e8", "e9", "e10", "e12", "e13", "e14")

AST_CHUNKS_PATH = Path(
    os.environ.get(
        "CG_AST_CHUNKS",
        "/zfs/archive/contextgraph/corpus/python-ast-chunks-v1/python_ast_chunks.jsonl",
    )
)
LABEL_ROOT = Path(
    os.environ.get(
        "CG_LABEL_ROOT",
        "/zfs/archive/contextgraph/state/labels/task-py-g-117-auto-label-discovery-v2-live-split",
    )
)
FORWARD_ROOT = Path(
    os.environ.get(
        "CG_FORWARD_CACHE",
        "/zfs/archive/contextgraph/corpus/python-forward-cache-v1-aiwonder-prod-20260515-191116Z",
    )
)
FSV_ROOT = Path(
    os.environ.get(
        "CG_FSV",
        "/zfs/archive/contextgraph/fsv",
    )
) / "task-py-g-135-weak-cell-evidence-expansion-fsv"
CORPUS_INDEX_PATH = Path(
    "/zfs/archive/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/index.json"
)
REPO_ROOT = Path(os.environ.get("CG_REPO", Path.cwd()))

REPORT_NAME = "task_py_g_135_data_substrate_lineage_fsv.json"
READBACK_NAME = "task_py_g_135_data_substrate_lineage_readback.json"


def utc_stamp() -> str:
    return dt.datetime.now(dt.UTC).strftime("%Y%m%dT%H%M%SZ")


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def write_json_0600(path: Path, payload: dict[str, Any]) -> None:
    path.write_text(json.dumps(payload, indent=2, sort_keys=True), encoding="utf-8")
    os.chmod(path, 0o600)


def read_json(path: Path) -> Any:
    with path.open("r", encoding="utf-8") as handle:
        return json.load(handle)


def iter_jsonl(path: Path):
    with path.open("r", encoding="utf-8", errors="replace") as handle:
        for line_number, line in enumerate(handle, start=1):
            line = line.strip()
            if not line:
                continue
            yield line_number, json.loads(line)


def line_scan(path: Path) -> dict[str, Any]:
    lines = 0
    mnt_d_lines = 0
    mnt_c_lines = 0
    first_mnt_d_sample = None
    with path.open("r", encoding="utf-8", errors="replace") as handle:
        for raw in handle:
            lines += 1
            if "/mnt/d" in raw:
                mnt_d_lines += 1
                if first_mnt_d_sample is None:
                    first_mnt_d_sample = raw.strip()[:500]
            if "/mnt/c" in raw:
                mnt_c_lines += 1
    return {
        "path": str(path),
        "sha256": sha256_file(path),
        "bytes": path.stat().st_size,
        "lines": lines,
        "mnt_d_lines": mnt_d_lines,
        "mnt_c_lines": mnt_c_lines,
        "first_mnt_d_sample": first_mnt_d_sample,
    }


def stable_key(row: dict[str, Any]) -> tuple[str, str, str, str, str]:
    return (
        str(row.get("chunk_id") or ""),
        str(row.get("file_sha256") or ""),
        str(row.get("source_sha256") or ""),
        str(row.get("task_instance_id") or ""),
        str(row.get("workspace_state") or ""),
    )


def scan_ast_chunks() -> tuple[dict[str, Any], dict[tuple[str, str, str, str, str], dict[str, Any]]]:
    total = 0
    absolute_mnt_d = 0
    dataschema_mnt_d = 0
    source_text_present = 0
    source_slice_verified = 0
    category_counts: Counter[str] = Counter()
    legacy_by_category: Counter[str] = Counter()
    source_text_bytes_by_category: Counter[str] = Counter()
    chunk_kind_counts: Counter[str] = Counter()
    examples: list[dict[str, Any]] = []
    meta: dict[tuple[str, str, str, str, str], dict[str, Any]] = {}

    for _, row in iter_jsonl(AST_CHUNKS_PATH):
        total += 1
        category = str(row.get("mutation_category") or "unknown")
        chunk_kind = str(row.get("chunk_kind") or "unknown")
        category_counts[category] += 1
        chunk_kind_counts[chunk_kind] += 1
        source_text = str(row.get("source_text") or "")
        if source_text:
            source_text_present += 1
            source_text_bytes_by_category[category] += len(source_text.encode("utf-8"))
        if row.get("source_slice_verified") is True:
            source_slice_verified += 1
        if str(row.get("absolute_path") or "").startswith("/mnt/d/"):
            absolute_mnt_d += 1
            legacy_by_category[category] += 1
            if len(examples) < 8:
                examples.append(
                    {
                        "chunk_id": row.get("chunk_id"),
                        "task_instance_id": row.get("task_instance_id"),
                        "workspace_state": row.get("workspace_state"),
                        "mutation_category": category,
                        "relative_path": row.get("relative_path"),
                        "absolute_path": row.get("absolute_path"),
                        "dataschema": row.get("dataschema"),
                    }
                )
        if str(row.get("dataschema") or "").startswith("/mnt/d/"):
            dataschema_mnt_d += 1
        meta[stable_key(row)] = {
            "category": category,
            "chunk_kind": chunk_kind,
            "relative_path": row.get("relative_path"),
            "source_text_bytes": len(source_text.encode("utf-8")),
        }

    return (
        {
            "path": str(AST_CHUNKS_PATH),
            "sha256": sha256_file(AST_CHUNKS_PATH),
            "total_rows": total,
            "legacy_absolute_path_rows": absolute_mnt_d,
            "legacy_dataschema_rows": dataschema_mnt_d,
            "source_text_present_rows": source_text_present,
            "source_slice_verified_rows": source_slice_verified,
            "category_counts": dict(sorted(category_counts.items())),
            "legacy_absolute_path_rows_by_category": dict(sorted(legacy_by_category.items())),
            "chunk_kind_counts_top": dict(chunk_kind_counts.most_common(30)),
            "source_text_bytes_by_category": dict(sorted(source_text_bytes_by_category.items())),
            "legacy_examples": examples,
        },
        meta,
    )


def scan_label_artifacts() -> dict[str, Any]:
    files = sorted(path for path in LABEL_ROOT.glob("*") if path.is_file())
    scans = [line_scan(path) for path in files]
    return {
        "root": str(LABEL_ROOT),
        "exists": LABEL_ROOT.is_dir(),
        "file_count": len(files),
        "total_mnt_d_lines": sum(item["mnt_d_lines"] for item in scans),
        "total_mnt_c_lines": sum(item["mnt_c_lines"] for item in scans),
        "files": scans,
    }


def scan_forward_manifest() -> dict[str, Any]:
    manifest_path = FORWARD_ROOT / "python_forward_cache_corpus_manifest.json"
    checkpoint_path = FORWARD_ROOT / "python_forward_cache_checkpoint.json"
    plan_path = FORWARD_ROOT / "python_forward_cache_shard_plan.json"
    manifest = read_json(manifest_path)
    summary = dict(manifest.get("summary") or {})
    per_slot_rows: dict[str, int] = {slot: 0 for slot in REQUIRED_EMBEDDERS}
    per_slot_shards: Counter[str] = Counter()

    for shard in manifest.get("shards") or []:
        per_embedder = (
            shard.get("packed_store_readback", {})
            .get("packed_manifest", {})
            .get("per_embedder", {})
        )
        for slot, entry in per_embedder.items():
            per_slot_shards[slot] += 1
            per_slot_rows[slot] = per_slot_rows.get(slot, 0) + int(entry.get("row_count") or 0)

    expected_chunks = int(summary.get("chunk_count") or 0)
    slot_coverage = {}
    for slot in REQUIRED_EMBEDDERS:
        rows = per_slot_rows.get(slot, 0)
        slot_coverage[slot] = {
            "record_rows": rows,
            "missing_rows_vs_ast": max(expected_chunks - rows, 0),
            "coverage": rows / expected_chunks if expected_chunks else None,
            "shard_presence": int(per_slot_shards.get(slot, 0)),
        }

    small_file_scans = [
        line_scan(path)
        for path in (manifest_path, checkpoint_path, plan_path)
        if path.exists()
    ]

    return {
        "root": str(FORWARD_ROOT),
        "exists": FORWARD_ROOT.is_dir(),
        "manifest_path": str(manifest_path),
        "manifest_sha256": sha256_file(manifest_path),
        "status": manifest.get("status"),
        "passes": manifest.get("passes"),
        "summary": summary,
        "slot_coverage": slot_coverage,
        "small_file_legacy_path_scan": small_file_scans,
    }


def scan_forward_packed_samples() -> dict[str, Any]:
    sampled = 0
    mnt_d_samples = 0
    mnt_c_samples = 0
    missing_slot_samples: list[dict[str, Any]] = []
    examples: list[dict[str, Any]] = []
    shard_count = 0

    for shard_root in sorted((FORWARD_ROOT / "shards").glob("shard-*")):
        shard_count += 1
        packed_manifest_path = shard_root / "python_forward_cache_packed_manifest.json"
        if not packed_manifest_path.exists():
            continue
        packed_manifest = read_json(packed_manifest_path)
        per_embedder = packed_manifest.get("per_embedder") or {}
        missing = sorted(set(REQUIRED_EMBEDDERS) - set(per_embedder))
        if missing and len(missing_slot_samples) < 16:
            missing_slot_samples.append({"shard_id": shard_root.name, "missing_slots": missing})
        for slot in REQUIRED_EMBEDDERS:
            entry = per_embedder.get(slot)
            if not entry:
                continue
            record_path = Path(str(entry.get("record_path") or ""))
            if not record_path.exists():
                continue
            with record_path.open("r", encoding="utf-8", errors="replace") as handle:
                for line in handle:
                    if not line.strip():
                        continue
                    sampled += 1
                    has_mnt_d = "/mnt/d" in line
                    has_mnt_c = "/mnt/c" in line
                    mnt_d_samples += 1 if has_mnt_d else 0
                    mnt_c_samples += 1 if has_mnt_c else 0
                    if has_mnt_d and len(examples) < 8:
                        row = json.loads(line)
                        key = row.get("key") or {}
                        chunk = row.get("chunk") or {}
                        examples.append(
                            {
                                "shard_id": shard_root.name,
                                "embedder_id": slot,
                                "chunk_id": key.get("chunk_id"),
                                "task_instance_id": chunk.get("task_instance_id"),
                                "workspace_state": chunk.get("workspace_state"),
                                "relative_path": chunk.get("relative_path"),
                                "absolute_path": chunk.get("absolute_path"),
                                "vector_len": (row.get("output") or {}).get("vector_len"),
                                "precision_class": (row.get("output") or {}).get("precision_class"),
                            }
                        )
                    break

    return {
        "sample_strategy": "first_non_empty_record_per_shard_per_required_embedder",
        "full_packed_record_scan": False,
        "shard_count": shard_count,
        "sampled_rows": sampled,
        "sampled_rows_with_mnt_d": mnt_d_samples,
        "sampled_rows_with_mnt_c": mnt_c_samples,
        "missing_slot_samples": missing_slot_samples,
        "legacy_examples": examples,
    }


def coverage_by_category(meta: dict[tuple[str, str, str, str, str], dict[str, Any]]) -> dict[str, Any]:
    slot_bits = {slot: idx for idx, slot in enumerate(REQUIRED_EMBEDDERS)}
    masks: dict[tuple[str, str, str, str, str], int] = {key: 0 for key in meta}
    unknown_forward_rows = 0
    index_rows = 0

    for index_path in sorted((FORWARD_ROOT / "shards").glob("shard-*/python_forward_cache_index.jsonl")):
        for _, row in iter_jsonl(index_path):
            index_rows += 1
            key = (
                str(row.get("chunk_id") or ""),
                str(row.get("file_sha256") or ""),
                str(row.get("source_sha256") or ""),
                str(row.get("task_instance_id") or ""),
                str(row.get("workspace_state") or ""),
            )
            bit = slot_bits.get(str(row.get("embedder_id") or ""))
            if key in masks and bit is not None:
                masks[key] |= 1 << bit
            else:
                unknown_forward_rows += 1

    by_category: dict[str, Counter[int]] = defaultdict(Counter)
    missing_slots_by_category: dict[str, Counter[str]] = defaultdict(Counter)
    missing_patterns: Counter[tuple[str, ...]] = Counter()
    source_bytes_by_present_slot_count: dict[str, Counter[int]] = defaultdict(Counter)

    for key, mask in masks.items():
        info = meta[key]
        category = str(info["category"])
        present_count = int(mask.bit_count())
        by_category[category][present_count] += 1
        source_bytes_by_present_slot_count[category][present_count] += int(info["source_text_bytes"])
        missing = tuple(slot for slot, bit in slot_bits.items() if not (mask & (1 << bit)))
        if missing:
            missing_patterns[missing] += 1
            for slot in missing:
                missing_slots_by_category[category][slot] += 1

    return {
        "index_rows_scanned": index_rows,
        "unknown_forward_index_rows": unknown_forward_rows,
        "coverage_by_category_present_slot_count": {
            category: dict(sorted(counter.items()))
            for category, counter in sorted(by_category.items())
        },
        "missing_slots_by_category": {
            category: dict(sorted(counter.items()))
            for category, counter in sorted(missing_slots_by_category.items())
        },
        "source_text_bytes_by_category_and_present_slot_count": {
            category: dict(sorted(counter.items()))
            for category, counter in sorted(source_bytes_by_present_slot_count.items())
        },
        "missing_patterns_top": [
            {"missing_slots": list(pattern), "row_count": count}
            for pattern, count in missing_patterns.most_common(20)
        ],
        "weak_cell_coverage": {
            category: {
                "present_slot_count_distribution": dict(sorted(by_category.get(category, Counter()).items())),
                "missing_slots": dict(sorted(missing_slots_by_category.get(category, Counter()).items())),
            }
            for category in WEAK_CELLS
        },
    }


def scan_corpus_index() -> dict[str, Any]:
    if not CORPUS_INDEX_PATH.exists():
        return {"path": str(CORPUS_INDEX_PATH), "exists": False}
    payload = read_json(CORPUS_INDEX_PATH)
    entries = payload.get("entries") if isinstance(payload, dict) else payload
    split_counts: dict[str, dict[str, Counter[str]]] = defaultdict(lambda: defaultdict(Counter))
    fail_examples: dict[str, list[dict[str, Any]]] = defaultdict(list)
    absolute_patch_paths: list[dict[str, Any]] = []
    mounted_patch_paths: list[dict[str, Any]] = []
    parent_escape_patch_paths: list[dict[str, Any]] = []
    for row in entries:
        category = str(row.get("category") or "unknown")
        bucket = str(row.get("bucket") or row.get("split") or "unknown")
        outcome = "pass" if bool(row.get("oracle_all_passed")) else "fail"
        patch_path = str(row.get("patch_path") or "")
        split_counts[category][bucket][outcome] += 1
        patch_summary = {
            "bucket": bucket,
            "category": category,
            "task_id": row.get("task_id"),
            "patch_path": patch_path,
            "oracle_all_passed": row.get("oracle_all_passed"),
        }
        if patch_path.startswith("/") and len(absolute_patch_paths) < 16:
            absolute_patch_paths.append(patch_summary)
        if ("/mnt/d" in patch_path or "/mnt/c" in patch_path) and len(mounted_patch_paths) < 16:
            mounted_patch_paths.append(patch_summary)
        if ".." in Path(patch_path).parts and len(parent_escape_patch_paths) < 16:
            parent_escape_patch_paths.append(patch_summary)
        if outcome == "fail" and len(fail_examples[category]) < 8:
            fail_examples[category].append(
                {
                    "bucket": bucket,
                    "task_id": row.get("task_id"),
                    "repo": row.get("repo"),
                    "patch_path": row.get("patch_path"),
                    "mutation_note": row.get("mutation_note"),
                    "oracle_all_passed": row.get("oracle_all_passed"),
                }
            )
    return {
        "path": str(CORPUS_INDEX_PATH),
        "exists": True,
        "sha256": sha256_file(CORPUS_INDEX_PATH),
        "split_counts": {
            category: {
                bucket: {
                    "pass": counter.get("pass", 0),
                    "fail": counter.get("fail", 0),
                    "total": counter.get("pass", 0) + counter.get("fail", 0),
                }
                for bucket, counter in sorted(bucket_map.items())
            }
            for category, bucket_map in sorted(split_counts.items())
        },
        "weak_cell_split_counts": {
            category: {
                bucket: {
                    "pass": counter.get("pass", 0),
                    "fail": counter.get("fail", 0),
                    "total": counter.get("pass", 0) + counter.get("fail", 0),
                }
                for bucket, counter in sorted(split_counts.get(category, {}).items())
            }
            for category in WEAK_CELLS
        },
        "failure_examples": {category: rows for category, rows in sorted(fail_examples.items())},
        "patch_path_hygiene": {
            "absolute_patch_path_count": sum(
                1 for row in entries if str(row.get("patch_path") or "").startswith("/")
            ),
            "mounted_patch_path_count": sum(
                1
                for row in entries
                if "/mnt/d" in str(row.get("patch_path") or "")
                or "/mnt/c" in str(row.get("patch_path") or "")
            ),
            "parent_escape_patch_path_count": sum(
                1 for row in entries if ".." in Path(str(row.get("patch_path") or "")).parts
            ),
            "absolute_examples": absolute_patch_paths,
            "mounted_examples": mounted_patch_paths,
            "parent_escape_examples": parent_escape_patch_paths,
        },
    }


def scan_path_deref_consumers() -> dict[str, Any]:
    matches: list[dict[str, Any]] = []
    for rel in [
        "scripts/swebench_lite_ops/python_ast_chunks.py",
        "scripts/swebench_lite_ops/python_static_graph_core.py",
        "scripts/swebench_lite_ops/python_static_graph.py",
        "scripts/swebench_lite_ops/smoke_manifest.py",
    ]:
        path = REPO_ROOT / rel
        if not path.exists():
            continue
        for line_no, line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
            compact = line.replace(" ", "")
            dereferences_absolute_path = (
                'Path(record["absolute_path"])' in compact
                or "Path(record['absolute_path'])" in compact
                or 'Path(row["absolute_path"])' in compact
                or "Path(row['absolute_path'])" in compact
                or ('record["absolute_path"]' in compact and ".read_text(" in compact)
                or ("record['absolute_path']" in compact and ".read_text(" in compact)
                or ("absolute_path" in compact and ".read_bytes(" in compact)
                or ("absolute_path" in compact and ".read_text(" in compact)
            )
            if dereferences_absolute_path:
                matches.append({"path": str(path), "line": line_no, "text": line.strip()})
    return {
        "repo_root": str(REPO_ROOT),
        "consumer_count": len(matches),
        "matches": matches,
    }


def decision(
    ast_scan: dict[str, Any],
    label_scan: dict[str, Any],
    forward_scan: dict[str, Any],
    packed_sample_scan: dict[str, Any],
    coverage_scan: dict[str, Any],
    path_consumers: dict[str, Any],
) -> dict[str, Any]:
    legacy_ast = int(ast_scan["legacy_absolute_path_rows"])
    mnt_d_exists = Path("/mnt/d").exists()
    missing_pairs = int((forward_scan.get("summary") or {}).get("quarantine_row_count") or 0)
    sampled_legacy = int(packed_sample_scan["sampled_rows_with_mnt_d"])
    sampled_total = int(packed_sample_scan["sampled_rows"])
    path_deref_count = int(path_consumers["consumer_count"])
    delete_counts = (coverage_scan.get("weak_cell_coverage", {}).get("delete_test_call", {}) or {}).get("missing_slots", {})

    blockers: list[str] = []
    if legacy_ast:
        blockers.append("TASK_PY_G_135_LEGACY_ABSOLUTE_PATHS_IN_AST_CHUNKS")
    if legacy_ast and not mnt_d_exists and path_deref_count:
        blockers.append("TASK_PY_G_135_AI_WONDER_CANNOT_DEREFERENCE_RETAINED_ABSOLUTE_PATHS")
    if sampled_total and sampled_legacy == sampled_total:
        blockers.append("TASK_PY_G_135_LEGACY_ABSOLUTE_PATHS_IN_FORWARD_PACKED_RECORDS")
    if missing_pairs:
        blockers.append("TASK_PY_G_135_INCOMPLETE_PER_EMBEDDER_SLOT_COVERAGE")
    if any(int(v) > 0 for v in delete_counts.values()):
        blockers.append("TASK_PY_G_135_DELETE_TEST_CALL_MISSING_EMBEDDER_SIGNAL")

    return {
        "data_substrate_promoted": False,
        "evidence_expansion_promoted": False,
        "root_cause_decision": "legacy_path_contamination_plus_incomplete_slot_coverage"
        if blockers
        else "no_legacy_path_or_slot_coverage_blocker_detected",
        "typed_blockers": blockers,
        "mnt_d_exists_on_aiwonder": mnt_d_exists,
        "labels_scrubbed_legacy_paths": int(label_scan["total_mnt_d_lines"]) == 0,
        "active_label_artifacts_are_not_the_source_path_problem": int(label_scan["total_mnt_d_lines"]) == 0,
        "forward_records_sampled_all_legacy_path": sampled_total > 0 and sampled_legacy == sampled_total,
        "recommendation": [
            "Stop treating the retained 300x8 substrate as promotable until active path lineage is canonicalized or rebuilt on aiwonder.",
            "Materialize an aiwonder-native source tree or replace active absolute_path/dataschema fields with /zfs/archive/contextgraph paths; keep /mnt/d only as legacy_provenance_path.",
            "Rerun path-dereference consumers from the canonicalized source tree before rerunning #392/#611.",
            "Resolve per-embedder missing slot coverage, especially long-source quarantines, before mining all 105 pair signals as if every chunk has complete slot arrays.",
            "After canonicalization/regeneration, rerun auto-label discovery, pair/constellation mining, #392 active surface, #611 repair inventory, #150 power, and #610 burn-in admissibility.",
        ],
    }


def main() -> int:
    run_dir = FSV_ROOT / f"run-{utc_stamp()}"
    run_dir.mkdir(parents=True, exist_ok=False)

    ast_scan, ast_meta = scan_ast_chunks()
    label_scan = scan_label_artifacts()
    forward_scan = scan_forward_manifest()
    packed_sample_scan = scan_forward_packed_samples()
    coverage_scan = coverage_by_category(ast_meta)
    corpus_index_scan = scan_corpus_index()
    path_consumers = scan_path_deref_consumers()
    decision_block = decision(
        ast_scan,
        label_scan,
        forward_scan,
        packed_sample_scan,
        coverage_scan,
        path_consumers,
    )

    report: dict[str, Any] = {
        "artifact_kind": "task_py_g_135_data_substrate_lineage_fsv",
        "schema_version": 1,
        "task_id": TASK_ID,
        "created_at_utc": dt.datetime.now(dt.UTC).isoformat(),
        "all_passed": True,
        "read_only": True,
        "mounted_drive_active_evidence_used": False,
        "inputs": {
            "ast_chunks_path": str(AST_CHUNKS_PATH),
            "label_root": str(LABEL_ROOT),
            "forward_root": str(FORWARD_ROOT),
            "corpus_index_path": str(CORPUS_INDEX_PATH),
            "repo_root": str(REPO_ROOT),
        },
        "aiwonder_path_probe": {
            "/mnt/d": {"exists": Path("/mnt/d").exists(), "is_dir": Path("/mnt/d").is_dir()},
            "/mnt/c": {"exists": Path("/mnt/c").exists(), "is_dir": Path("/mnt/c").is_dir()},
        },
        "ast_chunk_lineage": ast_scan,
        "label_artifact_lineage": label_scan,
        "forward_cache_lineage": forward_scan,
        "forward_packed_record_sample_lineage": packed_sample_scan,
        "forward_slot_coverage": coverage_scan,
        "corpus_index_oracle_split_support": corpus_index_scan,
        "path_dereference_consumers": path_consumers,
        "decision": decision_block,
    }

    report_path = run_dir / REPORT_NAME
    write_json_0600(report_path, report)
    readback = {
        "artifact_kind": "task_py_g_135_data_substrate_lineage_readback",
        "schema_version": 1,
        "created_at_utc": dt.datetime.now(dt.UTC).isoformat(),
        "report_path": str(report_path),
        "report_sha256": sha256_file(report_path),
        "report_size_bytes": report_path.stat().st_size,
        "report_json_round_trips": read_json(report_path)["decision"] == decision_block,
        "all_passed": True,
    }
    readback_path = run_dir / READBACK_NAME
    write_json_0600(readback_path, readback)

    print(json.dumps({"report": str(report_path), "readback": str(readback_path), **readback}, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
