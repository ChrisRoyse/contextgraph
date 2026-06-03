"""Second-pass taxonomy triage for BugsInPy rows left unclassified by #728."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
from collections import Counter
from pathlib import Path
from typing import Any, Iterable

from .python_bugsinpy_failure_mode_catalog_probe import (
    classify_bug,
    git_head,
    iter_bugs,
    parse_patch_files,
    sample_records,
    sha256_file,
    write_json,
    write_jsonl,
)


ARTIFACT_KIND = "bugsinpy_unclassified_taxonomy_triage"
SCHEMA_VERSION = 1
FORMULA_VERSION = "deterministic_bugsinpy_unclassified_taxonomy_triage_v1"
TASK_ID = "BLINDSPOT-006-TRIAGE"

DEFAULT_OUTPUT_ROOT = Path("/zfs/archive/contextgraph/fsv/blindspot-006-unclassified-taxonomy-triage-fsv/latest")

TAXONOMY_RULES: dict[str, tuple[str, ...]] = {
    "type_validation_serialization": (
        r"\bisinstance\b",
        r"\btype\(",
        r"\btyping\b",
        r"\bvalidate",
        r"\bvalidator",
        r"\bencoder\b",
        r"\bdecoder\b",
        r"\bjson\b",
        r"\bschema\b",
        r"\bconvert",
        r"\bcast\b",
    ),
    "index_shape_selection": (
        r"\bindex",
        r"\bidx\b",
        r"\baxis\b",
        r"\bshape\b",
        r"\bcolumn",
        r"\brow\b",
        r"\bslice\b",
        r"\bmask\b",
        r"\biloc\b",
        r"\bloc\b",
        r"\bdataframe\b",
        r"\bseries\b",
        r"\bgroupby\b",
        r"\bcategorical\b",
    ),
    "parser_formatting_syntax": (
        r"\bparse",
        r"\bparser\b",
        r"\btoken",
        r"\blexer",
        r"\bformat",
        r"\bregex\b",
        r"\bquote",
        r"\bindent",
        r"\bnewline\b",
        r"\bstring\b",
        r"\bblack\b",
    ),
    "scheduler_state_transition": (
        r"\bscheduler\b",
        r"\btask\b",
        r"\bworker\b",
        r"\bqueue\b",
        r"\bstatus\b",
        r"\bstate\b",
        r"\bjob\b",
        r"\bcallback\b",
        r"\bretry\b",
    ),
    "http_protocol_io": (
        r"\bhttp\b",
        r"\brequest\b",
        r"\bresponse\b",
        r"\bheader\b",
        r"\bcookie\b",
        r"\burl\b",
        r"\bproxy\b",
        r"\bdownload\b",
        r"\byoutube",
        r"\bscrapy\b",
        r"\btornado\b",
    ),
    "error_handling_exception": (
        r"\btry\b",
        r"\bexcept\b",
        r"\berror\b",
        r"\bexception\b",
        r"\braise\b",
        r"\bwarning\b",
        r"\btraceback\b",
    ),
    "configuration_cli_env": (
        r"\bconfig",
        r"\boption\b",
        r"\bcli\b",
        r"\barg",
        r"\benv\b",
        r"\bpath\b",
        r"\bsetting",
    ),
}


def stable_label_hash(label_id: str) -> str:
    return "label:" + hashlib.sha256(label_id.encode("utf-8")).hexdigest()[:24]


def classification_text(record: Any, patch_files: Iterable[Any]) -> str:
    affected_paths = " ".join(
        path
        for patch_file in patch_files
        for path in (patch_file.old_path, patch_file.new_path)
        if path and path != "/dev/null"
    )
    info_text = " ".join(f"{key}={value}" for key, value in sorted(record.bug_info.items()))
    return "\n".join([record.project, record.bug_id, affected_paths, info_text, record.patch_text])


def triage_classes(record: Any, patch_files: Iterable[Any]) -> tuple[list[str], dict[str, list[str]]]:
    text = classification_text(record, patch_files)
    matches: dict[str, list[str]] = {}
    for class_name, patterns in TAXONOMY_RULES.items():
        hit_patterns = sorted({pattern for pattern in patterns if re.search(pattern, text, re.IGNORECASE)})
        if hit_patterns:
            matches[class_name] = hit_patterns
    return sorted(matches), matches


def registry_rows(support: Counter[str]) -> list[dict[str, Any]]:
    rows = []
    for class_name, count in sorted(support.items()):
        label_id = f"taxonomy_triage:{class_name}"
        rows.append(
            {
                "artifact_kind": f"{ARTIFACT_KIND}_candidate_taxonomy_row",
                "schema_version": SCHEMA_VERSION,
                "formula_version": FORMULA_VERSION,
                "label_id": label_id,
                "label_hash": stable_label_hash(label_id),
                "support": count,
                "candidate_status": "candidate_triage_not_promoted",
                "learning_update_policy": {
                    "baseline_seeded_from": TASK_ID,
                    "target_labels_are_supervision_only": True,
                    "requires_child_issue_before_operator_or_slot_promotion": True,
                    "minimum_utility_delta": 0.005,
                },
            }
        )
    return rows


def run_triage(args: argparse.Namespace) -> dict[str, Any]:
    all_records = iter_bugs(args.bugsinpy_root)
    sampled_records = sample_records(all_records, args.sample_size)
    triage_rows: list[dict[str, Any]] = []
    unassigned_rows: list[dict[str, Any]] = []
    support: Counter[str] = Counter()
    original_other_count = 0
    assigned_count = 0
    mounted_drive_hits = 0

    for record in sampled_records:
        patch_files = parse_patch_files(record.patch_text)
        base_classes, _base_reasons = classify_bug(record, patch_files)
        if base_classes != ["other_unclassified_real_bug"]:
            continue
        original_other_count += 1
        classes, reasons = triage_classes(record, patch_files)
        if classes:
            assigned_count += 1
            support.update(classes)
        else:
            unassigned_rows.append(
                {
                    "artifact_kind": f"{ARTIFACT_KIND}_still_unclassified_row",
                    "schema_version": SCHEMA_VERSION,
                    "formula_version": FORMULA_VERSION,
                    "record_id": record.record_id,
                    "project": record.project,
                    "bug_id": record.bug_id,
                    "patch_sha256": record.patch_sha256,
                    "affected_paths": sorted(
                        {
                            path
                            for patch_file in patch_files
                            for path in (patch_file.old_path, patch_file.new_path)
                            if path and path != "/dev/null"
                        }
                    ),
                    "slot_identity_preserved": True,
                    "flat_vector_concat_used": False,
                    "target_labels_used_as_live_inputs": False,
                }
            )
        if "/mnt/c" in record.patch_text or "/mnt/d" in record.patch_text:
            mounted_drive_hits += 1
        triage_rows.append(
            {
                "artifact_kind": f"{ARTIFACT_KIND}_triage_row",
                "schema_version": SCHEMA_VERSION,
                "formula_version": FORMULA_VERSION,
                "record_id": record.record_id,
                "project": record.project,
                "bug_id": record.bug_id,
                "patch_sha256": record.patch_sha256,
                "triage_classes": classes,
                "triage_reasons": reasons,
                "candidate_labels": [f"taxonomy_triage:{class_name}" for class_name in classes],
                "slot_identity_preserved": True,
                "flat_vector_concat_used": False,
                "target_labels_used_as_live_inputs": False,
            }
        )

    candidate_rows = registry_rows(support)
    largest_class_support = max(support.values(), default=0)
    passes = bool(largest_class_support >= args.min_support)
    coverage_rows = [
        {
            "artifact_kind": f"{ARTIFACT_KIND}_coverage_matrix_row",
            "schema_version": SCHEMA_VERSION,
            "formula_version": FORMULA_VERSION,
            "source_other_unclassified_count": original_other_count,
            "assigned_count": assigned_count,
            "unassigned_count": original_other_count - assigned_count,
            "assigned_fraction": assigned_count / original_other_count if original_other_count else 0.0,
            "largest_class_support": largest_class_support,
            "candidate_taxonomy_support": dict(sorted(support.items())),
        }
    ]

    output_root = args.output_root
    output_root.mkdir(parents=True, exist_ok=True)
    triage_path = output_root / "triage_rows.jsonl"
    registry_path = output_root / "candidate_taxonomy_registry.jsonl"
    coverage_path = output_root / "coverage_matrix.jsonl"
    unassigned_path = output_root / "unassigned_rows.jsonl"
    triage_count = write_jsonl(triage_path, triage_rows)
    registry_count = write_jsonl(registry_path, candidate_rows)
    coverage_count = write_jsonl(coverage_path, coverage_rows)
    unassigned_count = write_jsonl(unassigned_path, unassigned_rows)

    summary = {
        "artifact_kind": f"{ARTIFACT_KIND}_summary",
        "schema_version": SCHEMA_VERSION,
        "formula_version": FORMULA_VERSION,
        "task_id": TASK_ID,
        "passes": passes,
        "decision": {
            "taxonomy_triage_has_high_support_child": passes,
            "promotion_allowed": False,
            "next_required_artifact": "file_child_issues_for_high_support_taxonomy_classes",
        },
        "counts": {
            "bugsinpy_total_records": len(all_records),
            "sample_count": len(sampled_records),
            "source_other_unclassified_count": original_other_count,
            "assigned_count": assigned_count,
            "unassigned_count": unassigned_count,
            "candidate_taxonomy_rows": registry_count,
            "coverage_rows": coverage_count,
            "mounted_drive_hits": mounted_drive_hits,
        },
        "fractions": {
            "assigned_fraction": assigned_count / original_other_count if original_other_count else 0.0,
            "unassigned_fraction": (original_other_count - assigned_count) / original_other_count
            if original_other_count
            else 1.0,
        },
        "taxonomy_support": dict(sorted(support.items())),
        "policy": {
            "measurement_only": True,
            "slot_identity_preserved": True,
            "flat_vector_concat_used": False,
            "target_labels_used_as_live_inputs": False,
            "inner_llm_used": False,
            "predictor_weights_changed": False,
            "candidate_registry_promoted": False,
        },
    }
    summary_path = output_root / "summary.json"
    write_json(summary_path, summary)

    manifest = {
        "artifact_kind": f"{ARTIFACT_KIND}_manifest",
        "schema_version": SCHEMA_VERSION,
        "formula_version": FORMULA_VERSION,
        "task_id": TASK_ID,
        "passes": passes,
        "source": {
            "bugsinpy_root": str(args.bugsinpy_root),
            "bugsinpy_git_head": git_head(args.bugsinpy_root),
        },
        "paths": {
            "triage_rows": str(triage_path),
            "candidate_taxonomy_registry": str(registry_path),
            "coverage_matrix": str(coverage_path),
            "unassigned_rows": str(unassigned_path),
            "summary": str(summary_path),
        },
        "line_counts": {
            "triage_rows": triage_count,
            "candidate_taxonomy_registry": registry_count,
            "coverage_matrix": coverage_count,
            "unassigned_rows": unassigned_count,
        },
        "stable_hashes": {
            "triage_rows": sha256_file(triage_path),
            "candidate_taxonomy_registry": sha256_file(registry_path),
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
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--sample-size", type=int, default=0)
    parser.add_argument("--min-support", type=int, default=50)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    add_arguments(parser)
    args = parser.parse_args(argv)
    manifest = run_triage(args)
    print(json.dumps(manifest, sort_keys=True, indent=2))
    return 0


if __name__ == "__main__":  # pragma: no cover
    raise SystemExit(main())
