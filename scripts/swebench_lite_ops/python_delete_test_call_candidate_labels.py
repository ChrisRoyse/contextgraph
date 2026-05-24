"""Candidate-keyed label rebuild for TASK-PY-G-135C delete_test_call rows.

#748: helpers (error class, ChunkWork/CandidateAggregate dataclasses,
load_corpus/load_chunks/load_masks/load_forward_slots, label derivation,
bucket_count/counter_to_rows/target_leakage_violations/split_support_gate)
are extracted to a sibling module
`python_delete_test_call_candidate_labels_helpers` to keep this file under
the doctrine §14.8 ≤500-line target. Public command surface
(`add_arguments`, `command_python_delete_test_call_candidate_labels`)
remains here.
"""

from __future__ import annotations

import argparse
import shutil
from collections import Counter, defaultdict
from itertools import combinations
from pathlib import Path
from typing import Any

from .core import normalize_category, sha256_file, utc_now, write_json_checked
from .python_auto_label_discovery import (
    FORMULA_VERSION as BASE_LABEL_FORMULA_VERSION,
    SCALAR_FEATURES,
    LabelStats,
    finite_float,
    group_signatures,
    is_live_predictor_label,
    pair_relationship,
    register_labels,
    source_surface_labels,
    split_live_and_supervision_labels,
    usefulness_rows,
)
from .python_real_corpus_constellation_smoke import parse_embedder_list
from .smoke_intelligence_matrices import dump_json, read_json, row_key_from_fields, tree_state
from .python_delete_test_call_candidate_labels_helpers import (
    ARTIFACT_KIND,
    FORMULA_VERSION,
    CandidateAggregate,
    CandidateLabelError,
    ChunkWork,
    aggregate_row,
    bucket_count,
    candidate_relation_labels,
    category_key,
    count_jsonl,
    counter_to_rows,
    failure_candidate_score,
    iter_jsonl,
    json_safe_payload,
    load_chunks,
    load_corpus,
    load_forward_slots,
    load_masks,
    missing_slot_labels,
    observe_aggregate,
    outcome_supervision_labels,
    require,
    split_support_gate,
    target_leakage_violations,
    validate_args,
    write_json_checked_safe,
    write_jsonl,
)


TASK_ID = "TASK-PY-G-135C"

DEFAULT_CORPUS_ROOT = Path(
    "/zfs/archive/contextgraph/state/regeneration-plans/"
    "task-py-g-135c-candidate-keyed-corpus-v1/run-20260523T012459Z-current52"
)
DEFAULT_FORWARD_ROOT = Path(
    "/zfs/archive/contextgraph/state/regeneration-plans/"
    "task-py-g-135c-candidate-keyed-forward-cache-v1/run-20260524T044119Z-current52"
)
DEFAULT_MASK_JSONL = Path(
    "/zfs/archive/contextgraph/state/forward-cache-completeness/"
    "task-py-g-135c-candidate-keyed-forward-cache-slot-completion-v1/forward_cache_slot_completeness_mask.jsonl"
)
DEFAULT_OUTPUT_ROOT = Path(
    "/zfs/archive/contextgraph/state/labels/task-py-g-135c-candidate-keyed-labels-v1"
)
DEFAULT_READBACK_ROOT = Path(
    "/zfs/archive/contextgraph/fsv/task-py-g-135c-candidate-keyed-labels-fsv/manual-readback"
)
DEFAULT_EXPECTED_EMBEDDERS = "e1,e6,e7,e8,e9,e10,e12,e13,e14"


def add_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--corpus-root", type=Path, default=DEFAULT_CORPUS_ROOT)
    parser.add_argument("--forward-root", type=Path, default=DEFAULT_FORWARD_ROOT)
    parser.add_argument("--completeness-mask-jsonl", type=Path, default=DEFAULT_MASK_JSONL)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--readback-root", "--fsv-root", dest="readback_root", type=Path, default=DEFAULT_READBACK_ROOT)
    parser.add_argument("--evidence-root", type=Path, default=DEFAULT_READBACK_ROOT)
    parser.add_argument("--expected-embedders", default=DEFAULT_EXPECTED_EMBEDDERS)
    parser.add_argument("--min-support", type=int, default=2)
    parser.add_argument("--min-lift", type=float, default=0.05)
    parser.add_argument("--clean-output", action="store_true")



def command_python_delete_test_call_candidate_labels(args: argparse.Namespace) -> dict[str, Any]:
    started_at = utc_now()
    validate_args(args)
    required_embedders = parse_embedder_list(args.expected_embedders)
    before = {"output_root": tree_state(args.output_root), "readback_root": tree_state(args.readback_root)}
    if args.clean_output and args.output_root.exists():
        shutil.rmtree(args.output_root)
    args.output_root.mkdir(parents=True, exist_ok=True)
    args.readback_root.mkdir(parents=True, exist_ok=True)

    corpus, corpus_summary = load_corpus(args.corpus_root)
    chunks, chunk_to_candidate, chunk_summary = load_chunks(args.corpus_root, corpus)
    masks, mask_summary = load_masks(args.completeness_mask_jsonl, chunks)
    work, forward_summary = load_forward_slots(args.forward_root, chunks, masks, required_embedders)
    for row_key, corpus_row in chunk_to_candidate.items():
        work[row_key].corpus = corpus_row

    chunk_label_rows: list[dict[str, Any]] = []
    aggregates: dict[str, CandidateAggregate] = {cid: CandidateAggregate(row) for cid, row in corpus.items()}
    label_stats: dict[str, LabelStats] = {}
    category_totals: Counter[str] = Counter()
    category_fail_totals: Counter[str] = Counter()
    duplicate_slot_events = 0
    source_unverified_events = 0
    mask_partial_rows = 0

    for row_key, item in sorted(work.items()):
        corpus_row = item.corpus
        candidate_id = str(corpus_row.get("candidate_row_id"))
        failed = not bool(corpus_row.get("oracle_all_passed"))
        cat_key = category_key(corpus_row)
        partition = str(corpus_row.get("stable_partition") or "unknown")
        category_totals[cat_key] += 1
        category_fail_totals[cat_key] += 1 if failed else 0

        surface = source_surface_labels(item.chunk)
        relation = candidate_relation_labels(item.chunk, corpus_row)
        missing = missing_slot_labels(item.mask)
        slot_values = sorted(set(item.slot_labels + missing))
        available = sorted(item.slot_features)
        pair_labels = [
            pair_relationship(left, item.slot_features[left], right, item.slot_features[right])
            for left, right in combinations(available, 2)
        ]
        group_labels = group_signatures(item.slot_features, pair_labels)
        live_candidates = surface + relation + slot_values + pair_labels + group_labels
        live_labels, live_rejected = split_live_and_supervision_labels(live_candidates)
        target_supervision = sorted(set(outcome_supervision_labels(corpus_row) + live_rejected))
        all_labels = sorted(set(live_labels + target_supervision))

        register_labels(
            label_stats,
            live_candidates,
            failed=failed,
            category=cat_key,
            docker_label=str(corpus_row.get("oracle_status") or "unknown"),
            partition=partition,
            scope="chunk",
            row_key=row_key,
        )
        register_labels(
            label_stats,
            target_supervision,
            failed=failed,
            category=cat_key,
            docker_label=str(corpus_row.get("oracle_status") or "unknown"),
            partition=partition,
            scope="chunk",
            row_key=row_key,
            leaky_target=True,
        )

        duplicate_slots = sorted(set(item.duplicate_slots))
        source_unverified = sorted(set(item.source_readback_unverified))
        duplicate_slot_events += len(item.duplicate_slots)
        source_unverified_events += len(item.source_readback_unverified)
        mask_partial_rows += 0 if bool(item.mask.get("complete_static_ast_9slot_row")) else 1
        score, reasons = failure_candidate_score(item.chunk, corpus_row, surface, slot_values, pair_labels)
        if failed and score > 0.0:
            aggregates[candidate_id].failure_candidates.append(
                {
                    "row_key": row_key,
                    "chunk_id": item.chunk.get("chunk_id"),
                    "relative_path": item.chunk.get("relative_path"),
                    "byte_span": [item.chunk.get("byte_start"), item.chunk.get("byte_end")],
                    "node_kind": item.chunk.get("node_kind"),
                    "score": score,
                    "reasons": reasons,
                    "source_text_preview": str(item.chunk.get("source_text") or "")[:240],
                }
            )
        observe_aggregate(aggregates[candidate_id], row_key, item, surface + relation, slot_values, pair_labels, group_labels)

        chunk_label_rows.append(
            {
                "artifact_kind": f"{ARTIFACT_KIND}_chunk_constellation_labels",
                "schema_version": 1,
                "base_formula_version": BASE_LABEL_FORMULA_VERSION,
                "formula_version": FORMULA_VERSION,
                "row_key": row_key,
                "candidate_row_id": candidate_id,
                "candidate_corpus_row_id": corpus_row.get("candidate_corpus_row_id"),
                "source_task_id": corpus_row.get("source_task_id"),
                "task_instance_id": item.chunk.get("task_instance_id"),
                "mutation_category": "delete_test_call",
                "assertion_duty_sublabel": corpus_row.get("assertion_duty_sublabel"),
                "partition": partition,
                "workspace_state": item.chunk.get("workspace_state"),
                "repo": item.chunk.get("repo"),
                "relative_path": item.chunk.get("relative_path"),
                "chunk_id": item.chunk.get("chunk_id"),
                "source_sha256": item.chunk.get("source_sha256"),
                "source_text_sha256": item.chunk.get("redacted_source_sha256") or item.chunk.get("source_sha256"),
                "node_kind": item.chunk.get("node_kind"),
                "chunk_kind": item.chunk.get("chunk_kind"),
                "byte_span": [item.chunk.get("byte_start"), item.chunk.get("byte_end")],
                "labels": all_labels,
                "live_predictor_labels": live_labels,
                "target_supervision_labels": target_supervision,
                "source_site_relation_labels": relation,
                "slot_labels": slot_values,
                "present_slots": sorted(item.slot_features),
                "missing_slots": list(item.mask.get("missing_slots") or []),
                "complete_static_ast_9slot_row": bool(item.mask.get("complete_static_ast_9slot_row")),
                "completeness_mask_row_status": item.mask.get("row_status"),
                "duplicate_slots": duplicate_slots,
                "source_readback_unverified": source_unverified,
                "canonical_ast_lineage_used": True,
                "raw_forward_cache_path_metadata_legacy": bool(item.raw_legacy_slots),
                "raw_forward_cache_path_metadata_legacy_slots": sorted(set(item.raw_legacy_slots)),
                "slot_evidence_complete": not duplicate_slots and not source_unverified and bool(item.mask.get("complete_static_ast_9slot_row")),
                "slot_dims": dict(sorted(item.slot_dims.items())),
                "slot_vector_sha256": dict(sorted(item.slot_vector_hashes.items())),
                "slot_scalar_features": item.slot_features,
                "pair_relationships": pair_labels,
                "group_signatures": group_labels,
                "oracle_all_passed": bool(corpus_row.get("oracle_all_passed")),
                "oracle_status": corpus_row.get("oracle_status"),
                "failure_candidate_score": score,
                "failure_candidate_reasons": reasons,
                "target_oracle_labels_included": False,
                "target_oracle_labels_used_as_features": False,
                "slot_identity_preserved": True,
                "flat_vector_concat_used": False,
            }
        )

    code_state_rows = [aggregate_row(candidate_id, agg) for candidate_id, agg in sorted(aggregates.items())]
    failure_rows = failure_evidence_rows(aggregates)
    metric_rows, accepted_rows, usefulness_summary = usefulness_rows(label_stats, category_totals, category_fail_totals, args.min_support, args.min_lift)
    split_gate = split_support_gate(corpus)

    chunk_labels_path = args.output_root / "candidate_chunk_constellation_labels.jsonl"
    code_state_path = args.output_root / "candidate_code_state_constellations.jsonl"
    failure_path = args.output_root / "candidate_failure_evidence_sets.jsonl"
    metrics_path = args.output_root / "candidate_label_usefulness_metrics.jsonl"
    registry_path = args.output_root / "candidate_accepted_label_registry.jsonl"
    health_path = args.output_root / "candidate_slot_vector_space_health.jsonl"
    manifest_path = args.output_root / "candidate_keyed_label_rebuild_manifest.json"
    report_path = args.output_root / "candidate_keyed_label_rebuild_report.json"
    readback_path = args.readback_root / "candidate_keyed_label_rebuild_readback.json"

    line_counts = {
        "candidate_chunk_constellation_labels": write_jsonl(chunk_labels_path, chunk_label_rows),
        "candidate_code_state_constellations": write_jsonl(code_state_path, code_state_rows),
        "candidate_failure_evidence_sets": write_jsonl(failure_path, failure_rows),
        "candidate_label_usefulness_metrics": write_jsonl(metrics_path, metric_rows),
        "candidate_accepted_label_registry": write_jsonl(registry_path, accepted_rows),
        "candidate_slot_vector_space_health": write_jsonl(health_path, forward_summary["slot_vector_space_health"]),
    }
    stable_hashes = {
        "candidate_chunk_constellation_labels": sha256_file(chunk_labels_path),
        "candidate_code_state_constellations": sha256_file(code_state_path),
        "candidate_failure_evidence_sets": sha256_file(failure_path),
        "candidate_label_usefulness_metrics": sha256_file(metrics_path),
        "candidate_accepted_label_registry": sha256_file(registry_path),
        "candidate_slot_vector_space_health": sha256_file(health_path),
    }
    leakage = target_leakage_violations(chunk_label_rows) + target_leakage_violations(code_state_rows)
    validation_failures = []
    if duplicate_slot_events:
        validation_failures.append("DTC_LABEL_DUPLICATE_SLOT_EVIDENCE")
    if source_unverified_events:
        validation_failures.append("DTC_LABEL_SOURCE_READBACK_UNVERIFIED")
    if forward_summary["slot_vector_space_blockers"]:
        validation_failures.append("DTC_LABEL_VECTOR_SPACE_BLOCKERS")
    if leakage:
        validation_failures.append("DTC_LABEL_TARGET_LEAKAGE_IN_LIVE_LABELS")
    if not split_gate["passes"]:
        validation_failures.append("DTC_LABEL_SPLIT_SUPPORT_GATE_FAILED")

    summary = {
        "candidate_count": len(corpus),
        "chunk_label_rows": len(chunk_label_rows),
        "code_state_rows": len(code_state_rows),
        "failure_evidence_rows": len(failure_rows),
        "accepted_label_rows": len(accepted_rows),
        "label_metric_rows": len(metric_rows),
        "required_embedders": required_embedders,
        "mask_partial_rows": mask_partial_rows,
        "duplicate_slot_events": duplicate_slot_events,
        "source_readback_unverified_events": source_unverified_events,
        "target_leakage_live_label_violations": len(leakage),
        "target_leakage_live_label_samples": leakage[:10],
        "category_key_policy": "delete_test_call plus assertion_duty_sublabel; no task/category collapse",
        "candidate_row_id_keyed": True,
        "category_collapse_prevented": len(corpus) == len(code_state_rows),
        "target_oracle_labels_used_as_features": False,
        "split_support_gate": split_gate,
        "validation_failures": validation_failures,
        **usefulness_summary,
    }
    passes = (
        line_counts["candidate_chunk_constellation_labels"] == len(chunks)
        and line_counts["candidate_code_state_constellations"] == len(corpus)
        and line_counts["candidate_accepted_label_registry"] > 0
        and not validation_failures
    )
    report = {
        "artifact_kind": f"{ARTIFACT_KIND}_report",
        "schema_version": 1,
        "task_id": TASK_ID,
        "formula_version": FORMULA_VERSION,
        "created_at_utc": utc_now(),
        "passes": passes,
        "source_of_truth": {
            "corpus_root": str(args.corpus_root),
            "forward_root": str(args.forward_root),
            "completeness_mask_jsonl": str(args.completeness_mask_jsonl),
            "output_root": str(args.output_root),
            "readback_root": str(args.readback_root),
        },
        "policy": {
            "candidate_row_id_keyed": True,
            "preserve_assertion_duty_sublabel": True,
            "preserve_stable_partition": True,
            "preserve_completeness_mask": True,
            "target_labels_are_supervision_only": True,
            "slot_identity_preserved": True,
            "flat_vector_concat_used": False,
            "no_inner_llm": True,
        },
        "summary": summary,
        "corpus_summary": corpus_summary,
        "chunk_summary": chunk_summary,
        "mask_summary": mask_summary,
        "forward_summary": forward_summary,
    }
    write_json_checked_safe(report_path, report)
    manifest = {
        "artifact_kind": f"{ARTIFACT_KIND}_manifest",
        "schema_version": 1,
        "task_id": TASK_ID,
        "formula_version": FORMULA_VERSION,
        "created_at_utc": utc_now(),
        "passes": passes,
        "paths": {
            "candidate_chunk_constellation_labels": str(chunk_labels_path),
            "candidate_code_state_constellations": str(code_state_path),
            "candidate_failure_evidence_sets": str(failure_path),
            "candidate_label_usefulness_metrics": str(metrics_path),
            "candidate_accepted_label_registry": str(registry_path),
            "candidate_slot_vector_space_health": str(health_path),
            "report": str(report_path),
        },
        "line_counts": line_counts,
        "stable_hashes": stable_hashes,
    }
    write_json_checked_safe(manifest_path, manifest)
    readback = {
        "artifact_kind": f"{ARTIFACT_KIND}_readback",
        "schema_version": 1,
        "task_id": TASK_ID,
        "formula_version": FORMULA_VERSION,
        "started_at_utc": started_at,
        "completed_at_utc": utc_now(),
        "passes": passes and read_json(manifest_path).get("passes") is True and read_json(report_path).get("passes") is True,
        "before": before,
        "after": {"output_root": tree_state(args.output_root), "readback_root": tree_state(args.readback_root)},
        "manifest_path": str(manifest_path),
        "report_path": str(report_path),
        "manifest_sha256": sha256_file(manifest_path),
        "report_sha256": sha256_file(report_path),
        "line_counts": {key: count_jsonl(Path(path)) for key, path in manifest["paths"].items() if path.endswith(".jsonl")},
        "stable_hashes": {key: sha256_file(Path(path)) for key, path in manifest["paths"].items() if path.endswith(".jsonl")},
        "summary": summary,
    }
    write_json_checked_safe(readback_path, readback)
    return json_safe_payload({
        "ok": passes,
        "all_passed": passes,
        "command": "python-delete-test-call-candidate-labels",
        "checked_at": utc_now(),
        "manifest_path": str(manifest_path),
        "report_path": str(report_path),
        "readback_path": str(readback_path),
        "manifest_sha256": sha256_file(manifest_path),
        "report_sha256": sha256_file(report_path),
        "readback_sha256": sha256_file(readback_path),
        "summary": summary,
    }, context="python-delete-test-call-candidate-labels return payload")


def failure_evidence_rows(aggregates: dict[str, CandidateAggregate]) -> list[dict[str, Any]]:
    rows = []
    for candidate_id, agg in sorted(aggregates.items()):
        if bool(agg.corpus.get("oracle_all_passed")):
            continue
        candidates = sorted(agg.failure_candidates, key=lambda row: (-float(row["score"]), str(row["relative_path"]), str(row["chunk_id"])))
        rows.append(
            {
                "artifact_kind": f"{ARTIFACT_KIND}_failure_evidence_set",
                "schema_version": 1,
                "formula_version": FORMULA_VERSION,
                "candidate_row_id": candidate_id,
                "candidate_corpus_row_id": agg.corpus.get("candidate_corpus_row_id"),
                "source_task_id": agg.corpus.get("source_task_id"),
                "mutation_category": "delete_test_call",
                "assertion_duty_sublabel": agg.corpus.get("assertion_duty_sublabel"),
                "partition": agg.corpus.get("stable_partition"),
                "oracle_all_passed": bool(agg.corpus.get("oracle_all_passed")),
                "oracle_status": agg.corpus.get("oracle_status"),
                "localization_mode": "localization:single_point_candidate"
                if len([row for row in candidates if float(row["score"]) >= 6.0]) == 1
                else "localization:multi_point_or_diffuse_candidate",
                "candidate_count": len(candidates),
                "candidate_sample": candidates[:20],
                "target_oracle_labels_used_as_features": False,
                "slot_identity_preserved": True,
                "flat_vector_concat_used": False,
            }
        )
    return rows
