"""Candidate label-expansion FSV for #728 child failure-mode classes.

The runner is measurement-only. It materializes candidate accepted-registry rows
from deterministic labels emitted by the auto-label compiler and proves per-bug
coverage for one BugsInPy-derived class before any predictor or registry
promotion.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from collections import Counter
from pathlib import Path
from typing import Any, Iterable

from .python_bugsinpy_failure_mode_catalog_probe import (
    ARTIFACT_KIND as BASE_ARTIFACT_KIND,
    classify_bug,
    git_head,
    iter_bugs,
    label_has_registry_token,
    labels_from_patch,
    parse_patch_files,
    read_registry,
    sample_records,
    sha256_file,
    write_json,
    write_jsonl,
)
from .python_bugsinpy_unclassified_taxonomy_triage import triage_classes


ARTIFACT_KIND = "bugsinpy_label_expansion_candidate"
SCHEMA_VERSION = 1
FORMULA_VERSION = "deterministic_bugsinpy_label_expansion_candidate_v1"
TASK_ID = "BLINDSPOT-006-CHILD"

DEFAULT_REGISTRY = Path(
    "/zfs/archive/contextgraph/state/labels/task-py-g-117-auto-label-discovery-v1/"
    "accepted_label_registry.jsonl"
)
DEFAULT_OUTPUT_ROOT = Path(
    "/zfs/archive/contextgraph/fsv/blindspot-006-label-expansion-candidate-fsv/latest"
)

CLASS_LABEL_PREFIXES = {
    "api_deprecation_version_drift": ("api_version:",),
    "floating_point_precision": ("numeric_precision:",),
    "generator_iterator_exhaustion": ("iterator_flow:",),
    "http_protocol_io": ("protocol_io:",),
    "memory_aliasing_mutation": ("state_aliasing:",),
    "resource_leak": ("resource_lifecycle:",),
}

CLASS_REGISTRY_TOKENS = {
    "api_deprecation_version_drift": (
        "deprecation",
        "deprecated",
        "version",
        "compat",
        "api_drift",
        "collections_abc",
        "python_requires",
    ),
    "floating_point_precision": ("precision", "float", "floating", "dtype", "decimal", "nan", "inf"),
    "generator_iterator_exhaustion": (
        "generator",
        "iterator",
        "iterable",
        "stopiteration",
        "exhaustion",
        "materialization",
    ),
    "http_protocol_io": ("protocol", "http", "request", "response", "url", "download", "websocket"),
    "memory_aliasing_mutation": ("aliasing", "mutable", "inplace", "copy", "view", "shared"),
    "resource_leak": (
        "resource",
        "close",
        "cleanup",
        "socket",
        "session",
        "connection",
        "cursor",
        "handle",
        "release",
        "dispose",
    ),
}

TAXONOMY_DERIVED_TARGETS = {"http_protocol_io"}


def stable_label_hash(label_id: str) -> str:
    return "label:" + hashlib.sha256(label_id.encode("utf-8")).hexdigest()[:24]


def candidate_labels_for_class(target_class: str, emitted_labels: Iterable[str]) -> list[str]:
    prefixes = CLASS_LABEL_PREFIXES.get(target_class)
    if not prefixes:
        raise ValueError(f"Unsupported target class: {target_class}")
    return sorted(
        label
        for label in set(emitted_labels)
        if any(label.startswith(prefix) for prefix in prefixes)
    )


def registry_class_hits(target_class: str, labels: Iterable[str]) -> list[str]:
    tokens = CLASS_REGISTRY_TOKENS.get(target_class)
    if not tokens:
        raise ValueError(f"Unsupported target class: {target_class}")
    return sorted(
        label
        for label in set(labels)
        if any(label_has_registry_token(label, token) for token in tokens)
    )


def registry_rows_for_candidate_labels(target_class: str, support: Counter[str]) -> list[dict[str, Any]]:
    rows = []
    for label_id, count in sorted(support.items()):
        rows.append(
            {
                "artifact_kind": f"{ARTIFACT_KIND}_candidate_registry_row",
                "schema_version": SCHEMA_VERSION,
                "formula_version": FORMULA_VERSION,
                "target_class": target_class,
                "label_id": label_id,
                "label_hash": stable_label_hash(label_id),
                "support": count,
                "candidate_status": "candidate_defined_not_promoted",
                "learning_update_policy": {
                    "baseline_seeded_from": TASK_ID,
                    "target_labels_are_supervision_only": True,
                    "requires_extended_corpus_utility_gate": True,
                    "minimum_utility_delta": 0.005,
                },
            }
        )
    return rows


def target_membership(record: Any, patch_files: Iterable[Any], target_class: str) -> tuple[bool, dict[str, list[str]]]:
    base_classes, base_reasons = classify_bug(record, patch_files)
    if target_class in TAXONOMY_DERIVED_TARGETS:
        if base_classes != ["other_unclassified_real_bug"]:
            return False, {}
        taxonomy_classes, taxonomy_reasons = triage_classes(record, patch_files)
        return target_class in taxonomy_classes, taxonomy_reasons
    return target_class in base_classes, base_reasons


def unassigned_retriage_reason(classification_reasons: Iterable[str]) -> str:
    reasons = set(classification_reasons)
    if reasons <= {r"\bnp\.", r"\bnumpy\b", r"\bdtype\b"}:
        return "context_only_numpy_dtype_reference"
    if reasons <= {r"\bfloat\b"}:
        return "context_only_float_type_reference"
    if reasons <= {r"\bDecimal\b"}:
        return "context_only_decimal_reference"
    if reasons <= {r"\bnan\b", r"\binf\b", r"\bnp\."}:
        return "context_only_nan_inf_reference"
    if reasons <= {r"\bfile\b"}:
        return "context_only_file_reference"
    if reasons <= {r"\bwith\b"}:
        return "context_only_context_manager_reference"
    if reasons <= {r"\bSession\b", r"\bConnection\b", r"\bsocket\b", r"\bcursor\b"}:
        return "context_only_resource_handle_reference"
    if reasons <= {r"\bclose\(", r"\bcleanup\b", r"\brelease\b", r"\bdispose\b", r"\bfinally\b"}:
        return "context_only_cleanup_reference"
    if reasons <= {r"\bcopy\b", r"\bdeepcopy\b", r"\bview\b"}:
        return "context_only_copy_view_reference"
    if reasons <= {r"\bappend\(", r"\bextend\(", r"\bpop\(", r"\bclear\(", r"\bsetdefault\("}:
        return "context_only_mutable_container_reference"
    if reasons <= {r"\binplace\b"}:
        return "context_only_inplace_reference"
    if reasons <= {r"\bid\(", r"\bviews\b"}:
        return "context_only_shared_reference"
    if reasons <= {
        r"\biter\(",
        r"\bnext\(",
        r"\byield\b",
        r"\bgenerator\b",
        r"\bIterator\b",
        r"\bIterable\b",
        r"\bStopIteration\b",
        r"\bitertools\b",
    }:
        return "context_only_generator_iterator_reference"
    if reasons <= {r"\bhttp\b", r"\brequest\b", r"\bresponse\b", r"\bheader\b", r"\bcookie\b", r"\burl\b"}:
        return "context_only_http_reference"
    if reasons <= {r"\bdownload\b", r"\byoutube", r"\bscrapy\b", r"\btornado\b"}:
        return "context_only_protocol_project_reference"
    return "mixed_or_manual_triage_required"


def run_candidate_expansion(args: argparse.Namespace) -> dict[str, Any]:
    accepted_labels, registry_rows = read_registry(args.accepted_registry)
    previous_class_hits = registry_class_hits(args.target_class, accepted_labels)
    all_records = iter_bugs(args.bugsinpy_root)
    sampled_records = sample_records(all_records, args.sample_size)

    bug_rows: list[dict[str, Any]] = []
    candidate_support: Counter[str] = Counter()
    unassigned_rows: list[dict[str, Any]] = []
    unassigned_reasons: Counter[str] = Counter()
    class_count = 0
    assigned_count = 0
    generic_match_count = 0
    class_candidate_hit_count = 0
    mounted_drive_hits = 0

    for record in sampled_records:
        patch_files = parse_patch_files(record.patch_text)
        is_member, class_reasons = target_membership(record, patch_files, args.target_class)
        if not is_member:
            continue
        class_count += 1
        emitted_labels = labels_from_patch(patch_files)
        generic_matches = sorted(label for label in emitted_labels if label in accepted_labels)
        candidate_labels = candidate_labels_for_class(args.target_class, emitted_labels)
        candidate_class_hits = registry_class_hits(args.target_class, candidate_labels)
        if generic_matches:
            generic_match_count += 1
        if candidate_labels:
            assigned_count += 1
            candidate_support.update(candidate_labels)
        if candidate_class_hits:
            class_candidate_hit_count += 1
        else:
            retriage_reason = unassigned_retriage_reason(class_reasons.get(args.target_class, []))
            unassigned_reasons[retriage_reason] += 1
            unassigned_rows.append(
                {
                    "artifact_kind": f"{ARTIFACT_KIND}_unassigned_row",
                    "schema_version": SCHEMA_VERSION,
                    "formula_version": FORMULA_VERSION,
                    "record_id": record.record_id,
                    "project": record.project,
                    "bug_id": record.bug_id,
                    "target_class": args.target_class,
                    "classification_reasons": class_reasons.get(args.target_class, []),
                    "retriage_reason": retriage_reason,
                    "patch_sha256": record.patch_sha256,
                    "candidate_labels": candidate_labels,
                    "slot_identity_preserved": True,
                    "flat_vector_concat_used": False,
                    "target_labels_used_as_live_inputs": False,
                }
            )
        row_json = json.dumps(record.bug_info, sort_keys=True)
        if "/mnt/c" in row_json or "/mnt/d" in row_json:
            mounted_drive_hits += 1
        bug_rows.append(
            {
                "artifact_kind": f"{ARTIFACT_KIND}_bug_label_row",
                "schema_version": SCHEMA_VERSION,
                "formula_version": FORMULA_VERSION,
                "record_id": record.record_id,
                "project": record.project,
                "bug_id": record.bug_id,
                "target_class": args.target_class,
                "classification_reasons": class_reasons.get(args.target_class, []),
                "patch_sha256": record.patch_sha256,
                "accepted_generic_label_matches": generic_matches,
                "candidate_labels": candidate_labels,
                "candidate_class_hits": candidate_class_hits,
                "candidate_class_covered": bool(candidate_class_hits),
                "slot_identity_preserved": True,
                "flat_vector_concat_used": False,
                "target_labels_used_as_live_inputs": False,
            }
        )

    candidate_rows = registry_rows_for_candidate_labels(args.target_class, candidate_support)
    coverage_rows = [
        {
            "artifact_kind": f"{ARTIFACT_KIND}_coverage_matrix_row",
            "schema_version": SCHEMA_VERSION,
            "formula_version": FORMULA_VERSION,
            "target_class": args.target_class,
            "source_bug_count": class_count,
            "candidate_assigned_count": assigned_count,
            "candidate_unassigned_count": class_count - assigned_count,
            "candidate_assigned_fraction": assigned_count / class_count if class_count else 0.0,
            "candidate_class_covered_count": class_candidate_hit_count,
            "candidate_class_uncovered_count": class_count - class_candidate_hit_count,
            "candidate_class_uncovered_fraction": (
                (class_count - class_candidate_hit_count) / class_count if class_count else 1.0
            ),
            "previous_accepted_registry_class_hit_count": len(previous_class_hits),
            "candidate_registry_row_count": len(candidate_rows),
            "candidate_label_support": dict(sorted(candidate_support.items())),
            "unassigned_retriage_reason_counts": dict(sorted(unassigned_reasons.items())),
        }
    ]

    candidate_label_axis_defined = bool(class_candidate_hit_count >= args.min_support and candidate_rows)
    full_class_coverage = bool(class_count > 0 and class_candidate_hit_count == class_count)
    output_root = args.output_root
    output_root.mkdir(parents=True, exist_ok=True)
    bug_rows_path = output_root / "bug_label_rows.jsonl"
    registry_path = output_root / "candidate_registry_expansion.jsonl"
    coverage_path = output_root / "coverage_matrix.jsonl"
    unassigned_path = output_root / "unassigned_rows.jsonl"
    bug_row_count = write_jsonl(bug_rows_path, bug_rows)
    candidate_row_count = write_jsonl(registry_path, candidate_rows)
    coverage_row_count = write_jsonl(coverage_path, coverage_rows)
    unassigned_row_count = write_jsonl(unassigned_path, unassigned_rows)

    summary = {
        "artifact_kind": f"{ARTIFACT_KIND}_summary",
        "schema_version": SCHEMA_VERSION,
        "formula_version": FORMULA_VERSION,
        "task_id": TASK_ID,
        "target_class": args.target_class,
        "passes": candidate_label_axis_defined,
        "decision": {
            "candidate_label_axis_defined": candidate_label_axis_defined,
            "full_class_coverage": full_class_coverage,
            "promotion_allowed": False,
            "next_required_artifact": (
                "extended_corpus_utility_gate"
                if full_class_coverage
                else "unassigned_row_triage_then_extended_corpus_utility_gate"
            ),
        },
        "counts": {
            "bugsinpy_total_records": len(all_records),
            "sample_count": len(sampled_records),
            "target_class_bug_count": class_count,
            "candidate_assigned_count": assigned_count,
            "candidate_class_covered_count": class_candidate_hit_count,
            "candidate_registry_rows": candidate_row_count,
            "unassigned_rows": unassigned_row_count,
            "previous_accepted_registry_rows": len(registry_rows),
            "previous_accepted_registry_class_hit_count": len(previous_class_hits),
            "generic_accepted_label_match_count": generic_match_count,
            "mounted_drive_hits": mounted_drive_hits,
        },
        "fractions": {
            "candidate_assigned_fraction": assigned_count / class_count if class_count else 0.0,
            "candidate_class_uncovered_fraction": (
                (class_count - class_candidate_hit_count) / class_count if class_count else 1.0
            ),
        },
        "policy": {
            "measurement_only": True,
            "slot_identity_preserved": True,
            "flat_vector_concat_used": False,
            "target_labels_used_as_live_inputs": False,
            "inner_llm_used": False,
            "predictor_weights_changed": False,
            "candidate_registry_promoted": False,
            "utility_gate_required_before_promotion": True,
        },
    }
    summary_path = output_root / "summary.json"
    write_json(summary_path, summary)

    manifest = {
        "artifact_kind": f"{ARTIFACT_KIND}_manifest",
        "schema_version": SCHEMA_VERSION,
        "formula_version": FORMULA_VERSION,
        "base_artifact_kind": BASE_ARTIFACT_KIND,
        "task_id": TASK_ID,
        "passes": candidate_label_axis_defined,
        "source": {
            "bugsinpy_root": str(args.bugsinpy_root),
            "bugsinpy_git_head": git_head(args.bugsinpy_root),
            "accepted_registry": str(args.accepted_registry),
            "accepted_registry_sha256": sha256_file(args.accepted_registry),
        },
        "paths": {
            "bug_label_rows": str(bug_rows_path),
            "candidate_registry_expansion": str(registry_path),
            "coverage_matrix": str(coverage_path),
            "unassigned_rows": str(unassigned_path),
            "summary": str(summary_path),
        },
        "line_counts": {
            "bug_label_rows": bug_row_count,
            "candidate_registry_expansion": candidate_row_count,
            "coverage_matrix": coverage_row_count,
            "unassigned_rows": unassigned_row_count,
        },
        "stable_hashes": {
            "bug_label_rows": sha256_file(bug_rows_path),
            "candidate_registry_expansion": sha256_file(registry_path),
            "coverage_matrix": sha256_file(coverage_path),
            "unassigned_rows": sha256_file(unassigned_path),
            "summary": sha256_file(summary_path),
        },
    }
    manifest_path = output_root / "manifest.json"
    write_json(manifest_path, manifest)
    manifest["paths"]["manifest"] = str(manifest_path)
    manifest["stable_hashes"]["manifest"] = sha256_file(manifest_path)
    write_json(output_root / "artifact_hashes.json", manifest["stable_hashes"])
    return manifest


def add_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--bugsinpy-root", type=Path, required=True)
    parser.add_argument("--target-class", default="floating_point_precision", choices=sorted(CLASS_LABEL_PREFIXES))
    parser.add_argument("--accepted-registry", type=Path, default=DEFAULT_REGISTRY)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--sample-size", type=int, default=0)
    parser.add_argument("--min-support", type=int, default=50)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    add_arguments(parser)
    args = parser.parse_args(argv)
    manifest = run_candidate_expansion(args)
    print(json.dumps(manifest, sort_keys=True, indent=2))
    return 0


if __name__ == "__main__":  # pragma: no cover
    raise SystemExit(main())
