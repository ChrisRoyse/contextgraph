"""Assemble Q1-Q5 RealityPrediction reward tensors from Python DDA/TCT artifacts."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import shutil
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any

import numpy as np

from .code_constellation_schema import FAILURE_REASON_BY_CATEGORY, MUTATION_CATEGORIES, RISK_BY_CATEGORY
from .smoke_intelligence_matrices import (
    DEFAULT_SMOKE_MANIFEST,
    artifact_state,
    dump_json,
    iter_jsonl,
    read_json,
    row_key_from_fields,
    sha256_file,
    tree_state,
    utc_now,
    write_jsonl,
)


SCHEMA_VERSION = 1
TASK_ID = "TASK-PY-DATA-021"
ARTIFACT_KIND = "python_q1_q5_reward_tensors"
DEFAULT_DDA_ROOT = Path("/zfs/archive/contextgraph/corpus/python-dda-signals-v1/per-embedder")
DEFAULT_TCT_ROOT = Path("/zfs/archive/contextgraph/corpus/python-tct-cells-v1")
DEFAULT_OUTPUT_ROOT = Path("/zfs/archive/contextgraph/corpus/python-reward-tensors-v1")
DEFAULT_FSV_ROOT = Path("/zfs/archive/contextgraph/fsv/task-py-data-021-reward-tensors-fsv")
DEFAULT_FULL_AST_FSV = Path("/zfs/archive/contextgraph/fsv/task-py-data-058-full-ast-corpus-fsv/full_ast_corpus_fsv.json")
DEFAULT_SMOKE_MANIFEST_FSV = Path("/zfs/archive/contextgraph/fsv/task-py-data-060-post249-smoke-manifest-fsv/smoke_manifest_fsv.json")
DEFAULT_DDA_FSV = Path("/zfs/archive/contextgraph/fsv/task-py-data-017-dda-signals-fsv/python_dda_code_signals_fsv.json")
DEFAULT_TCT_FSV = Path("/zfs/archive/contextgraph/fsv/task-py-data-020-tct-cells-fsv/python_tct_cells_fsv.json")
DEFAULT_SCHEMA_PATH = Path("/zfs/archive/contextgraph/corpus/python-code-constellation-schema-v1/code_constellation_schema.json")


class RewardTensorError(RuntimeError):
    pass


def require(condition: bool, code: str, message: str, details: dict[str, Any] | None = None) -> None:
    if not condition:
        raise RewardTensorError(json.dumps({"error_code": code, "message": message, "details": details or {}}, sort_keys=True))


def canonical_json(payload: Any) -> str:
    return json.dumps(payload, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def sha256_text(payload: Any) -> str:
    return "sha256:" + hashlib.sha256(canonical_json(payload).encode("utf-8")).hexdigest()


def load_passing_json(path: Path, code: str) -> tuple[dict[str, Any], str]:
    doc = read_json(path)
    require(doc.get("passes") is True, code, f"required FSV does not pass: {path}", {"path": str(path)})
    return doc, sha256_file(path)


def load_manifest(path: Path) -> tuple[dict[str, dict[str, Any]], dict[str, Any]]:
    rows: dict[str, dict[str, Any]] = {}
    counts = {"mutation_categories": Counter(), "repos": Counter(), "chunk_kinds": Counter()}
    for _, row in iter_jsonl(path):
        key = row_key_from_fields(
            str(row.get("task_instance_id") or ""),
            str(row.get("workspace_state") or ""),
            str(row.get("chunk_id") or ""),
            str(row.get("source_sha256") or ""),
        )
        require(key not in rows, "REWARD_TENSOR_DUPLICATE_MANIFEST_KEY", "duplicate manifest stable key", {"row_key": key})
        rows[key] = row
        counts["mutation_categories"][str(row.get("mutation_category") or "unknown")] += 1
        counts["repos"][str(row.get("repo") or "unknown")] += 1
        counts["chunk_kinds"][str(row.get("chunk_kind") or "unknown")] += 1
    return rows, {
        "rows": len(rows),
        "sha256": sha256_file(path),
        "mutation_categories": dict(sorted(counts["mutation_categories"].items())),
        "repos": dict(sorted(counts["repos"].items())),
        "chunk_kinds": dict(sorted(counts["chunk_kinds"].items())),
        "path": str(path),
    }


def load_memberships(path: Path) -> dict[str, dict[str, Any]]:
    rows: dict[str, dict[str, Any]] = {}
    for _, row in iter_jsonl(path):
        key = str(row.get("row_key") or "")
        require(key, "REWARD_TENSOR_TCT_KEY_MISSING", "TCT membership missing row_key", {"path": str(path)})
        require(key not in rows, "REWARD_TENSOR_TCT_DUPLICATE", "duplicate TCT membership row", {"row_key": key})
        rows[key] = row
    return rows


def load_cell_support(path: Path) -> dict[str, dict[str, Any]]:
    rows = {}
    for _, row in iter_jsonl(path):
        rows[str(row.get("cell_id") or "")] = row
    return rows


def load_dda(path: Path) -> tuple[dict[str, list[dict[str, Any]]], dict[str, dict[str, Any]]]:
    present: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for _, row in iter_jsonl(path / "per_embedder_signals.jsonl"):
        present[str(row.get("row_key") or "")].append(row)
    missing: dict[str, dict[str, Any]] = defaultdict(dict)
    for _, row in iter_jsonl(path / "missing_signal_states.jsonl"):
        key = str(row.get("row_key") or "")
        embedder = str(row.get("embedder_id") or "")
        if key and embedder:
            missing[key][embedder] = row
    return present, missing


def q1_target(manifest: dict[str, Any]) -> dict[str, Any]:
    proof = manifest.get("source_proof") or {}
    exists = bool(proof.get("live_workspace_present") and proof.get("source_text_matches_declared_hash"))
    return {
        "head": "Q1_EXISTENCE",
        "label": exists,
        "label_state": "code_label",
        "evidence": {
            "relative_path": manifest.get("relative_path"),
            "absolute_path": manifest.get("absolute_path"),
            "source_sha256": manifest.get("source_sha256"),
            "source_slice_verified_claim": proof.get("source_slice_verified_claim"),
            "live_workspace_verdict": proof.get("live_workspace_verdict"),
        },
    }


def q2_target(manifest: dict[str, Any]) -> dict[str, Any]:
    oracle = manifest.get("oracle_linkage") or {}
    if oracle.get("infra_error") is True or oracle.get("error_class") == "infra":
        return {"head": "Q2_WORKS", "label": None, "label_state": "excluded_infra", "evidence": oracle}
    works = bool(oracle.get("all_passed") is True and oracle.get("resolved") is True)
    return {"head": "Q2_WORKS", "label": works, "label_state": "code_label", "evidence": oracle}


def q3_target(manifest: dict[str, Any], axes: dict[str, str], q2: dict[str, Any]) -> dict[str, Any]:
    oracle = manifest.get("oracle_linkage") or {}
    fail_count = int(oracle.get("fail_to_pass_count") or len(oracle.get("fail_to_pass") or []))
    if q2["label"] is True:
        reasons: list[str] = []
        primary = "none"
        state = "not_applicable_pass"
    elif q2["label_state"] == "excluded_infra":
        reasons = []
        primary = "excluded_infra"
        state = "excluded_infra"
    else:
        primary = axes.get("failure_reason") or FAILURE_REASON_BY_CATEGORY.get(str(manifest.get("mutation_category") or ""), "unknown")
        reasons = [primary]
        if fail_count > 1:
            reasons.append("multi_test_failure")
        state = "code_label"
    return {
        "head": "Q3_FAILURE_REASON",
        "primary_failure_reason": primary,
        "failure_reason_targets": reasons,
        "failure_cardinality": fail_count,
        "label_state": state,
        "evidence": {"fail_to_pass": oracle.get("fail_to_pass") or [], "command": oracle.get("command")},
    }


def q4_target(manifest: dict[str, Any], membership: dict[str, Any], cell_support: dict[str, Any], missing_count: int) -> dict[str, Any]:
    axes = membership.get("axes") or {}
    concerns = [axes.get("edge_risk_concern") or RISK_BY_CATEGORY.get(str(manifest.get("mutation_category") or ""), "unknown_risk")]
    if str(manifest.get("chunk_kind") or "").startswith("test_") or "/tests/" in str(manifest.get("relative_path") or ""):
        concerns.append("test_surface_change")
    if missing_count:
        concerns.append("missing_embedder_signal")
    if cell_support.get("support_policy") not in {None, "eligible"}:
        concerns.append(f"tct_{cell_support.get('support_policy')}")
    return {
        "head": "Q4_ELSE_RISK",
        "concern_targets": sorted(set(concerns)),
        "label_state": "code_label",
        "evidence": {
            "cell_id": membership.get("cell_id"),
            "cell_support_policy": cell_support.get("support_policy"),
            "embedder_missing_count": missing_count,
            "chunk_kind": manifest.get("chunk_kind"),
        },
    }


def q5_target(manifest: dict[str, Any], axes: dict[str, str]) -> dict[str, Any]:
    oracle = manifest.get("oracle_linkage") or {}
    line_range = manifest.get("line_range") or {}
    line_span = max(1, int(line_range.get("end") or 0) - int(line_range.get("start") or 0) + 1)
    return {
        "head": "Q5_REALITY_IMPACT",
        "impact_target": axes.get("impact_reality", "oracle_bound_patch_and_tests"),
        "label_state": "code_label",
        "evidence": {
            "source_byte_len": int(manifest.get("source_byte_len") or 0),
            "token_estimate": int(manifest.get("token_estimate") or 0),
            "line_span": line_span,
            "fail_to_pass_count": int(oracle.get("fail_to_pass_count") or 0),
            "pass_to_pass_count": int(oracle.get("pass_to_pass_count") or 0),
            "relative_path": manifest.get("relative_path"),
        },
    }


def dda_summary(rows: list[dict[str, Any]]) -> dict[str, Any]:
    if not rows:
        return {"present_signal_count": 0, "mean_vector_norm_l2": None, "mean_entropy_proxy": None}
    norms = [float((row.get("signals") or {}).get("vector_norm_l2") or 0.0) for row in rows]
    entropy = [float((row.get("signals") or {}).get("vector_entropy_proxy") or 0.0) for row in rows]
    return {
        "present_signal_count": len(rows),
        "mean_vector_norm_l2": float(np.asarray(norms, dtype=np.float64).mean()),
        "mean_entropy_proxy": float(np.asarray(entropy, dtype=np.float64).mean()),
        "embedder_ids": sorted(str(row.get("embedder_id") or "") for row in rows),
    }


def build_chunk_tensor(
    row_key: str,
    manifest: dict[str, Any],
    membership: dict[str, Any],
    cell_support: dict[str, Any],
    dda_rows: list[dict[str, Any]],
    missing_rows: dict[str, Any],
    source_hashes: dict[str, str],
) -> dict[str, Any]:
    axes = membership.get("axes") or {}
    q1 = q1_target(manifest)
    q2 = q2_target(manifest)
    q3 = q3_target(manifest, axes, q2)
    q4 = q4_target(manifest, membership, cell_support, len(missing_rows))
    q5 = q5_target(manifest, axes)
    material = {"row_key": row_key, "cell_id": membership.get("cell_id"), "chunk_id": manifest.get("chunk_id")}
    return {
        "schema_version": SCHEMA_VERSION,
        "artifact_kind": "python_chunk_q1_q5_reward_tensor",
        "task_id": TASK_ID,
        "tensor_id": sha256_text(material),
        "row_key": row_key,
        "patch_key": f"{manifest.get('task_instance_id')}::{manifest.get('mutation_category')}",
        "task_instance_id": str(manifest.get("task_instance_id") or ""),
        "repo": str(manifest.get("repo") or ""),
        "mutation_category": str(manifest.get("mutation_category") or ""),
        "chunk_id": str(manifest.get("chunk_id") or ""),
        "relative_path": str(manifest.get("relative_path") or ""),
        "cell_id": membership.get("cell_id"),
        "axes": axes,
        "heads": {"q1": q1, "q2": q2, "q3": q3, "q4": q4, "q5": q5},
        "dda": dda_summary(dda_rows),
        "missing_embedder_count": len(missing_rows),
        "tct": {
            "membership_id": membership.get("membership_id"),
            "cell_support_policy": cell_support.get("support_policy"),
            "cell_density": cell_support.get("density"),
            "cell_chunk_membership_count": cell_support.get("chunk_membership_count"),
        },
        "input_hashes": source_hashes,
    }


def build_patch_tensors(chunk_rows: list[dict[str, Any]]) -> list[dict[str, Any]]:
    groups: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for row in chunk_rows:
        groups[str(row["patch_key"])].append(row)
    out = []
    for patch_key, rows in sorted(groups.items()):
        q2_labels = [row["heads"]["q2"]["label"] for row in rows if row["heads"]["q2"]["label_state"] == "code_label"]
        q4_targets = sorted({target for row in rows for target in row["heads"]["q4"]["concern_targets"]})
        q3_targets = sorted({target for row in rows for target in row["heads"]["q3"]["failure_reason_targets"]})
        source_bytes = sum(int(row["heads"]["q5"]["evidence"]["source_byte_len"]) for row in rows)
        material = {"patch_key": patch_key, "chunk_tensor_ids": [row["tensor_id"] for row in rows]}
        out.append(
            {
                "schema_version": SCHEMA_VERSION,
                "artifact_kind": "python_patch_q1_q5_reward_tensor",
                "task_id": TASK_ID,
                "patch_tensor_id": sha256_text(material),
                "patch_key": patch_key,
                "task_instance_id": rows[0]["task_instance_id"],
                "repo": rows[0]["repo"],
                "mutation_category": rows[0]["mutation_category"],
                "chunk_tensor_count": len(rows),
                "cell_count": len({row["cell_id"] for row in rows}),
                "heads": {
                    "q1": {"label": all(row["heads"]["q1"]["label"] for row in rows), "label_state": "code_label"},
                    "q2": {"label": all(q2_labels) if q2_labels else None, "label_state": "code_label" if q2_labels else "excluded_infra"},
                    "q3": {"failure_reason_targets": q3_targets, "label_state": "code_label" if q3_targets else "not_applicable_pass"},
                    "q4": {"concern_targets": q4_targets, "label_state": "code_label"},
                    "q5": {"impact_targets": sorted({row["heads"]["q5"]["impact_target"] for row in rows}), "aggregate_source_byte_len": source_bytes, "label_state": "code_label"},
                },
                "input_tensor_ids": [row["tensor_id"] for row in rows],
            }
        )
    return out


def edge_cases() -> list[dict[str, Any]]:
    q1_row = {"relative_path": "missing.py", "source_proof": {"live_workspace_present": False, "source_text_matches_declared_hash": False}}
    q2_row = {"oracle_linkage": {"infra_error": True, "error_class": "infra", "all_passed": None}}
    q3_manifest = {"mutation_category": "wrong_file", "oracle_linkage": {"all_passed": False, "resolved": False, "fail_to_pass_count": 2, "fail_to_pass": ["a::test_x", "b::test_y"]}}
    q3 = q3_target(q3_manifest, {"failure_reason": "wrong_blast_radius"}, {"label": False, "label_state": "code_label"})
    return [
        {
            "case": "q1_false_no_file_change",
            "before": {"live_workspace_present": False, "source_text_matches_declared_hash": False},
            "action": "build Q1 target",
            "after": q1_target(q1_row),
            "expected": "Q1 label false",
            "pass": q1_target(q1_row)["label"] is False,
        },
        {
            "case": "q2_infra_error_excluded",
            "before": q2_row["oracle_linkage"],
            "action": "build Q2 target",
            "after": q2_target(q2_row),
            "expected": "excluded_infra with null label",
            "pass": q2_target(q2_row)["label"] is None and q2_target(q2_row)["label_state"] == "excluded_infra",
        },
        {
            "case": "q3_multi_failure_outcome",
            "before": q3_manifest["oracle_linkage"],
            "action": "build Q3 target",
            "after": q3,
            "expected": "multi_test_failure included",
            "pass": "multi_test_failure" in q3["failure_reason_targets"] and q3["failure_cardinality"] == 2,
        },
    ]


def command_python_reward_tensors(args: argparse.Namespace) -> int:
    started = utc_now()
    if args.clean_output and args.output_root.exists():
        shutil.rmtree(args.output_root)
    args.output_root.mkdir(parents=True, exist_ok=True)
    args.fsv_root.mkdir(parents=True, exist_ok=True)
    before = {"output_root": tree_state(args.output_root), "fsv_root": tree_state(args.fsv_root)}
    _, full_ast_sha = load_passing_json(args.full_ast_fsv, "REWARD_FULL_AST_FSV_NOT_PASSING")
    _, smoke_sha = load_passing_json(args.smoke_manifest_fsv, "REWARD_SMOKE_MANIFEST_FSV_NOT_PASSING")
    _, dda_fsv_sha = load_passing_json(args.dda_fsv, "REWARD_DDA_FSV_NOT_PASSING")
    _, tct_fsv_sha = load_passing_json(args.tct_fsv, "REWARD_TCT_FSV_NOT_PASSING")
    manifest_by_key, manifest_summary = load_manifest(args.smoke_manifest)
    memberships = load_memberships(args.tct_root / "tct_chunk_memberships.jsonl")
    cell_support = load_cell_support(args.tct_root / "tct_cell_support.jsonl")
    dda_present, dda_missing = load_dda(args.dda_root)
    require(len(manifest_by_key) == args.expected_chunk_count, "REWARD_MANIFEST_COUNT_MISMATCH", "manifest row count mismatch", {"summary": manifest_summary})
    require(set(manifest_by_key) == set(memberships), "REWARD_TCT_MEMBERSHIP_MISMATCH", "TCT memberships must cover every manifest row", {"manifest": len(manifest_by_key), "memberships": len(memberships)})
    source_hashes = {
        "full_ast_fsv_sha256": full_ast_sha,
        "smoke_manifest_fsv_sha256": smoke_sha,
        "dda_fsv_sha256": dda_fsv_sha,
        "tct_fsv_sha256": tct_fsv_sha,
        "schema_sha256": sha256_file(args.constellation_schema),
        "smoke_manifest_sha256": manifest_summary["sha256"],
        "tct_report_sha256": sha256_file(args.tct_root / "tct_cell_report.json"),
        "dda_summary_sha256": sha256_file(args.dda_root / "dda_signal_summary.json"),
    }
    chunk_rows = []
    for key, manifest in sorted(manifest_by_key.items()):
        membership = memberships[key]
        support = cell_support.get(str(membership.get("cell_id") or ""), {})
        chunk_rows.append(build_chunk_tensor(key, manifest, membership, support, dda_present.get(key, []), dda_missing.get(key, {}), source_hashes))
    patch_rows = build_patch_tensors(chunk_rows)
    edges = edge_cases()
    write_jsonl(args.output_root / "chunk_reward_tensors.jsonl", chunk_rows)
    write_jsonl(args.output_root / "patch_reward_tensors.jsonl", patch_rows)
    write_jsonl(args.output_root / "edge_case_states.jsonl", edges)
    category_counts = Counter(row["mutation_category"] for row in chunk_rows)
    head_population = {
        head: sum(1 for row in chunk_rows if row["heads"][head] is not None)
        for head in ("q1", "q2", "q3", "q4", "q5")
    }
    known_patch = next((row for row in patch_rows if row["heads"]["q2"]["label"] is False), patch_rows[0] if patch_rows else None)
    report = {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "artifact_kind": ARTIFACT_KIND,
        "created_at_utc": utc_now(),
        "source_of_truth": {
            "output_root": str(args.output_root),
            "chunk_reward_tensors": str(args.output_root / "chunk_reward_tensors.jsonl"),
            "patch_reward_tensors": str(args.output_root / "patch_reward_tensors.jsonl"),
        },
        "source_hashes": source_hashes,
        "manifest": manifest_summary,
        "summary": {
            "chunk_tensor_count": len(chunk_rows),
            "patch_tensor_count": len(patch_rows),
            "category_counts": dict(sorted(category_counts.items())),
            "head_population": head_population,
            "q2_pass_count": sum(1 for row in chunk_rows if row["heads"]["q2"]["label"] is True),
            "q2_fail_count": sum(1 for row in chunk_rows if row["heads"]["q2"]["label"] is False),
            "q3_labeled_count": sum(1 for row in chunk_rows if row["heads"]["q3"]["failure_reason_targets"]),
        },
        "known_patch_with_all_heads": known_patch,
    }
    dump_json(args.output_root / "reward_tensor_report.json", report)
    after = {"output_root": tree_state(args.output_root), "fsv_root": tree_state(args.fsv_root)}
    readback_counts = {
        "chunk_reward_tensors": sum(1 for _ in iter_jsonl(args.output_root / "chunk_reward_tensors.jsonl")),
        "patch_reward_tensors": sum(1 for _ in iter_jsonl(args.output_root / "patch_reward_tensors.jsonl")),
        "edge_case_states": sum(1 for _ in iter_jsonl(args.output_root / "edge_case_states.jsonl")),
    }
    passes = (
        readback_counts["chunk_reward_tensors"] == args.expected_chunk_count
        and readback_counts["patch_reward_tensors"] > 0
        and set(category_counts) == set(MUTATION_CATEGORIES)
        and all(count == args.expected_chunk_count for count in head_population.values())
        and known_patch is not None
        and all(case["pass"] for case in edges)
    )
    fsv = {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "artifact_kind": f"{ARTIFACT_KIND}_fsv",
        "started_at_utc": started,
        "created_at_utc": utc_now(),
        "passes": passes,
        "before_state": before,
        "after_state": after,
        "source_of_truth": {**report["source_of_truth"], "report": str(args.output_root / "reward_tensor_report.json"), "fsv": str(args.fsv_root / "python_reward_tensors_fsv.json")},
        "trigger": {"command": "python3 scripts/swebench_lite_ops.py python-reward-tensors", "input_mode": "post249_smoke_v2"},
        "readback": {
            **readback_counts,
            "report_sha256": sha256_file(args.output_root / "reward_tensor_report.json"),
            "chunk_reward_tensors_sha256": sha256_file(args.output_root / "chunk_reward_tensors.jsonl"),
            "patch_reward_tensors_sha256": sha256_file(args.output_root / "patch_reward_tensors.jsonl"),
            "artifact_state": artifact_state(args.output_root),
            "head_population": head_population,
            "category_counts": dict(sorted(category_counts.items())),
            "known_patch_with_all_heads": known_patch,
        },
        "edge_cases": edges,
    }
    dump_json(args.fsv_root / "python_reward_tensors_fsv.json", fsv)
    final = read_json(args.fsv_root / "python_reward_tensors_fsv.json")
    require(final.get("passes") is True, "REWARD_TENSOR_FSV_FAILED", "reward tensor FSV failed", {"path": str(args.fsv_root / "python_reward_tensors_fsv.json")})
    print(json.dumps(final, sort_keys=True, indent=2))
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--smoke-manifest", type=Path, default=DEFAULT_SMOKE_MANIFEST)
    parser.add_argument("--dda-root", type=Path, default=DEFAULT_DDA_ROOT)
    parser.add_argument("--tct-root", type=Path, default=DEFAULT_TCT_ROOT)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--fsv-root", type=Path, default=DEFAULT_FSV_ROOT)
    parser.add_argument("--constellation-schema", type=Path, default=DEFAULT_SCHEMA_PATH)
    parser.add_argument("--full-ast-fsv", type=Path, default=DEFAULT_FULL_AST_FSV)
    parser.add_argument("--smoke-manifest-fsv", type=Path, default=DEFAULT_SMOKE_MANIFEST_FSV)
    parser.add_argument("--dda-fsv", type=Path, default=DEFAULT_DDA_FSV)
    parser.add_argument("--tct-fsv", type=Path, default=DEFAULT_TCT_FSV)
    parser.add_argument("--expected-chunk-count", type=int, default=1000)
    parser.add_argument("--clean-output", action="store_true")
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        return command_python_reward_tensors(args)
    except RewardTensorError as exc:
        print(f"PYTHON_REWARD_TENSORS_FAILED: {exc}")
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
