"""Build Python teleological constellation cells from DDA-enriched code signals."""

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

from .code_constellation_schema import (
    CHUNK_KINDS,
    ENTITY_FAMILY_BY_CHUNK_KIND,
    FAILURE_REASON_BY_CATEGORY,
    MUTATION_CATEGORIES,
    RISK_BY_CATEGORY,
    SCHEMA_VERSION as CONSTELLATION_SCHEMA_VERSION,
)
from .smoke_intelligence_matrices import (
    DEFAULT_FORWARD_ROOT,
    DEFAULT_SMOKE_MANIFEST,
    artifact_state,
    dump_json,
    iter_jsonl,
    load_records,
    load_smoke_manifest,
    read_json,
    sha256_file,
    tree_state,
    utc_now,
    write_jsonl,
)


SCHEMA_VERSION = 1
TASK_ID = "TASK-PY-DATA-020"
ARTIFACT_KIND = "python_tct_cell_report"
EXPECTED_EMBEDDERS = ("e1", "e6", "e7", "e8", "e10", "e12", "e13", "e14", "e2", "e3", "e4", "e9")
DEFAULT_SCHEMA_PATH = Path("/mnt/d/contextgraph/corpus/python-code-constellation-schema-v1/code_constellation_schema.json")
DEFAULT_DDA_ROOT = Path("/mnt/d/contextgraph/corpus/python-dda-signals-v1/per-embedder")
DEFAULT_PAIRWISE_ROOT = Path("/mnt/d/contextgraph/corpus/python-dda-signals-v1/pairwise")
DEFAULT_MATRIX_ROOT = Path("/mnt/d/contextgraph/corpus/python-embedder-intelligence-smoke-1000-v2/intelligence-matrices")
DEFAULT_OUTPUT_ROOT = Path("/mnt/d/contextgraph/corpus/python-tct-cells-v1")
DEFAULT_FSV_ROOT = Path("/mnt/d/contextgraph/fsv/task-py-data-020-tct-cells-fsv")
DEFAULT_FULL_AST_FSV = Path("/mnt/d/contextgraph/fsv/task-py-data-058-full-ast-corpus-fsv/full_ast_corpus_fsv.json")
DEFAULT_SMOKE_MANIFEST_FSV = Path("/mnt/d/contextgraph/fsv/task-py-data-060-post249-smoke-manifest-fsv/smoke_manifest_fsv.json")
DEFAULT_FORWARD_FSV = Path("/mnt/d/contextgraph/fsv/task-py-data-061-post249-smoke-forward-fsv/python_forward_shard_scheduler_fsv.json")
DEFAULT_DDA_FSV = Path("/mnt/d/contextgraph/fsv/task-py-data-017-dda-signals-fsv/python_dda_code_signals_fsv.json")
DEFAULT_PAIRWISE_FSV = Path("/mnt/d/contextgraph/fsv/task-py-data-018-pairwise-dda-mi-fsv/python_pairwise_dda_mi_fsv.json")
DEFAULT_MATRIX_FSV = Path("/mnt/d/contextgraph/fsv/task-py-data-062-post249-smoke-intelligence-fsv/smoke_intelligence_matrices_fsv.json")
EPS = 1.0e-12


class TctCellError(RuntimeError):
    pass


def require(condition: bool, code: str, message: str, details: dict[str, Any] | None = None) -> None:
    if not condition:
        raise TctCellError(json.dumps({"error_code": code, "message": message, "details": details or {}}, sort_keys=True))


def canonical_json(payload: Any) -> str:
    return json.dumps(payload, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def sha256_text(payload: Any) -> str:
    return "sha256:" + hashlib.sha256(canonical_json(payload).encode("utf-8")).hexdigest()


def sha256_vector(vector: np.ndarray) -> str:
    return "sha256:" + hashlib.sha256(vector.astype("<f4", copy=False).tobytes()).hexdigest()


def finite(value: float | int | None) -> float | None:
    if value is None:
        return None
    out = float(value)
    if not math.isfinite(out):
        return None
    return out


def load_passing_json(path: Path, code: str) -> tuple[dict[str, Any], str]:
    doc = read_json(path)
    require(doc.get("passes") is True, code, f"required FSV does not pass: {path}", {"path": str(path)})
    return doc, sha256_file(path)


def validate_schema(schema: dict[str, Any]) -> dict[str, Any]:
    version = schema.get("schema_version")
    require(
        version == CONSTELLATION_SCHEMA_VERSION,
        "TCT_STALE_SCHEMA_VERSION",
        "constellation schema version is stale",
        {"expected": CONSTELLATION_SCHEMA_VERSION, "actual": version},
    )
    axes = schema.get("axes") or {}
    missing_categories = sorted(set(MUTATION_CATEGORIES) - set((axes.get("mutation_category") or {}).get("enum") or []))
    missing_chunk_kinds = sorted(set(CHUNK_KINDS) - set((axes.get("chunk_kind") or {}).get("enum") or []))
    require(not missing_categories, "TCT_SCHEMA_MISSING_MUTATION_CATEGORY", "schema does not enumerate all mutation categories", {"missing": missing_categories})
    require(not missing_chunk_kinds, "TCT_SCHEMA_MISSING_CHUNK_KIND", "schema does not enumerate all accepted chunk kinds", {"missing": missing_chunk_kinds})
    support = schema.get("minimum_support") or {}
    hard_min = int(support.get("cell_hard_min") or 0)
    shrink_under = int(support.get("bayesian_shrink_under") or 0)
    require(hard_min > 0, "TCT_SCHEMA_SUPPORT_POLICY_MISSING", "schema minimum_support.cell_hard_min must be positive", {"support": support})
    require(shrink_under >= hard_min, "TCT_SCHEMA_SUPPORT_POLICY_INVALID", "bayesian shrink threshold must be >= hard minimum", {"support": support})
    return {"hard_min": hard_min, "shrink_under": shrink_under}


def test_intent_axis(manifest: dict[str, Any]) -> str:
    oracle = manifest.get("oracle_linkage") or {}
    fail_to_pass = int(oracle.get("fail_to_pass_count") or 0)
    pass_to_pass = int(oracle.get("pass_to_pass_count") or 0)
    if fail_to_pass and pass_to_pass:
        return "fail_to_pass_and_pass_to_pass"
    if fail_to_pass:
        return "fail_to_pass_only"
    if pass_to_pass:
        return "pass_to_pass_only"
    return "no_expected_tests"


def q_targets(manifest: dict[str, Any]) -> list[str]:
    oracle = manifest.get("oracle_linkage") or {}
    targets = ["Q1_claim_existence", "Q2_work_status", "Q4_edge_risk", "Q5_reality_impact"]
    if oracle.get("all_passed") is False or oracle.get("resolved") is False:
        targets.append("Q3_failure_reason")
    return targets


def axes_for_manifest(manifest: dict[str, Any]) -> dict[str, str]:
    category = str(manifest.get("mutation_category") or "")
    chunk_kind = str(manifest.get("chunk_kind") or "")
    oracle = manifest.get("oracle_linkage") or {}
    require(category in MUTATION_CATEGORIES, "TCT_UNKNOWN_MUTATION_CATEGORY", "manifest row has unknown mutation category", {"category": category})
    require(chunk_kind in CHUNK_KINDS, "TCT_UNKNOWN_CHUNK_KIND", "manifest row has unknown chunk kind", {"chunk_kind": chunk_kind})
    if oracle.get("all_passed") is True and oracle.get("resolved") is True:
        oracle_status = "oracle_pass"
    else:
        oracle_status = "oracle_fail"
    impact = "oracle_bound_patch_and_tests"
    if int(oracle.get("fail_to_pass_count") or 0) == 0 and int(oracle.get("pass_to_pass_count") or 0) == 0:
        impact = "oracle_bound_patch_only"
    return {
        "claim_existence": "corpus_gold_patch_present",
        "oracle_work_status": oracle_status,
        "failure_reason": FAILURE_REASON_BY_CATEGORY[category],
        "edge_risk_concern": RISK_BY_CATEGORY[category],
        "impact_reality": impact,
        "mutation_category": category,
        "chunk_kind": chunk_kind,
        "entity_family": ENTITY_FAMILY_BY_CHUNK_KIND[chunk_kind],
        "test_intent": test_intent_axis(manifest),
        "instrument_panel_support": "ast_oracle_diff_test_static_embedder_panel",
    }


def cell_id_for_axes(schema_version: int, axes: dict[str, str]) -> str:
    material = {"schema_version": schema_version, "language": "python", "axes": axes, "valid": True}
    return f"tct:v{schema_version}:" + sha256_text(material)


def load_dda_rows(path: Path) -> tuple[list[dict[str, Any]], dict[tuple[str, str], dict[str, Any]]]:
    rows: list[dict[str, Any]] = []
    by_pair: dict[tuple[str, str], dict[str, Any]] = {}
    for _, row in iter_jsonl(path):
        key = str(row.get("row_key") or "")
        embedder = str(row.get("embedder_id") or "")
        require(key and embedder, "TCT_DDA_SIGNAL_KEY_MISSING", "DDA signal row is missing row_key/embedder_id", {"path": str(path)})
        pair = (key, embedder)
        require(pair not in by_pair, "TCT_DDA_SIGNAL_DUPLICATE", "duplicate DDA signal row for row/embedder", {"row_key": key, "embedder_id": embedder})
        rows.append(row)
        by_pair[pair] = row
    return rows, by_pair


def load_missing_rows(path: Path) -> dict[tuple[str, str], dict[str, Any]]:
    out: dict[tuple[str, str], dict[str, Any]] = {}
    for _, row in iter_jsonl(path):
        pair = (str(row.get("row_key") or ""), str(row.get("embedder_id") or ""))
        if pair[0] and pair[1]:
            out[pair] = row
    return out


def load_pairwise(path: Path) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    rows = [row for _, row in iter_jsonl(path)]
    top_unique = sorted(rows, key=lambda row: float(row.get("unique_contribution_score") or 0.0), reverse=True)[:10]
    top_redundant = sorted(rows, key=lambda row: float(row.get("redundancy_score") or 0.0), reverse=True)[:10]
    return rows, {
        "row_count": len(rows),
        "valid_count": sum(1 for row in rows if row.get("valid") is True),
        "top_unique_pairs": top_unique,
        "top_redundant_pairs": top_redundant,
        "sha256": sha256_file(path),
    }


def numeric_signal_features(rows: list[dict[str, Any]]) -> list[str]:
    names: set[str] = set()
    for row in rows:
        for key, value in (row.get("signals") or {}).items():
            if isinstance(value, bool):
                continue
            if isinstance(value, (int, float)) and math.isfinite(float(value)):
                names.add(key)
    return sorted(names)


def scalar_summary(rows: list[dict[str, Any]], feature_names: list[str]) -> dict[str, Any]:
    out: dict[str, Any] = {}
    for feature in feature_names:
        values = [float((row.get("signals") or {}).get(feature)) for row in rows if isinstance((row.get("signals") or {}).get(feature), (int, float)) and not isinstance((row.get("signals") or {}).get(feature), bool)]
        values = [value for value in values if math.isfinite(value)]
        if not values:
            continue
        arr = np.asarray(values, dtype=np.float64)
        out[feature] = {
            "count": int(arr.size),
            "mean": float(arr.mean()),
            "std": float(arr.std()),
            "min": float(arr.min()),
            "max": float(arr.max()),
        }
    return out


def vector_prototype(vectors: list[np.ndarray]) -> dict[str, Any]:
    if not vectors:
        return {"prototype_state": "empty", "vector_count": 0}
    dims = sorted({int(vector.size) for vector in vectors})
    if len(dims) != 1:
        return {"prototype_state": "dimension_mismatch", "vector_count": len(vectors), "dims": dims}
    matrix = np.stack(vectors, axis=0).astype(np.float32, copy=False)
    centroid = matrix.mean(axis=0, dtype=np.float32)
    norms = np.linalg.norm(matrix, axis=1)
    return {
        "prototype_state": "centroid",
        "vector_count": len(vectors),
        "vector_dim": int(matrix.shape[1]),
        "centroid_sha256": sha256_vector(centroid),
        "centroid_norm_l2": float(np.linalg.norm(centroid)),
        "member_norm_mean": float(norms.mean()),
        "member_norm_std": float(norms.std()),
    }


def support_policy(count: int, hard_min: int, shrink_under: int) -> str:
    if count <= 0:
        return "empty"
    if count < hard_min:
        return "low_support_fail_closed"
    if count < shrink_under:
        return "low_support_shrink"
    return "eligible"


def membership_rows(
    manifest_by_key: dict[str, dict[str, Any]],
    signal_by_pair: dict[tuple[str, str], dict[str, Any]],
    missing_by_pair: dict[tuple[str, str], dict[str, Any]],
    expected_embedders: tuple[str, ...],
    schema_sha: str,
    source_hashes: dict[str, str],
) -> tuple[list[dict[str, Any]], dict[str, str]]:
    rows: list[dict[str, Any]] = []
    cell_by_key: dict[str, str] = {}
    for key, manifest in sorted(manifest_by_key.items()):
        axes = axes_for_manifest(manifest)
        cell_id = cell_id_for_axes(CONSTELLATION_SCHEMA_VERSION, axes)
        present = [embedder for embedder in expected_embedders if (key, embedder) in signal_by_pair]
        missing = [embedder for embedder in expected_embedders if (key, embedder) in missing_by_pair]
        material = {"row_key": key, "cell_id": cell_id, "chunk_id": manifest.get("chunk_id")}
        rows.append(
            {
                "schema_version": SCHEMA_VERSION,
                "artifact_kind": "python_tct_chunk_membership",
                "task_id": TASK_ID,
                "membership_id": sha256_text(material),
                "cell_id": cell_id,
                "row_key": key,
                "chunk_id": str(manifest.get("chunk_id") or ""),
                "task_instance_id": str(manifest.get("task_instance_id") or ""),
                "repo": str(manifest.get("repo") or ""),
                "relative_path": str(manifest.get("relative_path") or ""),
                "workspace_state": str(manifest.get("workspace_state") or ""),
                "axes": axes,
                "q_targets": q_targets(manifest),
                "embedder_present_count": len(present),
                "embedder_missing_count": len(missing),
                "present_embedders": present,
                "missing_embedders": missing,
                "input_hashes": {
                    "schema_sha256": schema_sha,
                    "source_sha256": str(manifest.get("source_sha256") or ""),
                    **source_hashes,
                },
            }
        )
        cell_by_key[key] = cell_id
    return rows, cell_by_key


def build_support_rows(
    memberships: list[dict[str, Any]],
    signal_rows: list[dict[str, Any]],
    records: dict[str, dict[str, Any]],
    cell_by_key: dict[str, str],
    expected_embedders: tuple[str, ...],
    hard_min: int,
    shrink_under: int,
) -> tuple[list[dict[str, Any]], list[dict[str, Any]], list[dict[str, Any]]]:
    memberships_by_cell: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for row in memberships:
        memberships_by_cell[row["cell_id"]].append(row)
    signals_by_cell_embedder: dict[tuple[str, str], list[dict[str, Any]]] = defaultdict(list)
    for row in signal_rows:
        cell_id = cell_by_key.get(str(row.get("row_key") or ""))
        if cell_id:
            signals_by_cell_embedder[(cell_id, str(row.get("embedder_id")))].append(row)
    feature_names = numeric_signal_features(signal_rows)
    cell_rows: list[dict[str, Any]] = []
    embedder_rows: list[dict[str, Any]] = []
    warnings: list[dict[str, Any]] = []
    for cell_id, rows in sorted(memberships_by_cell.items()):
        chunk_count = len(rows)
        present_pairs = sum(int(row["embedder_present_count"]) for row in rows)
        expected_pairs = chunk_count * len(expected_embedders)
        policy = support_policy(chunk_count, hard_min, shrink_under)
        axes = rows[0]["axes"]
        cell_rows.append(
            {
                "schema_version": SCHEMA_VERSION,
                "artifact_kind": "python_tct_cell_support",
                "task_id": TASK_ID,
                "cell_id": cell_id,
                "axes": axes,
                "chunk_membership_count": chunk_count,
                "present_embedder_pair_count": present_pairs,
                "expected_embedder_pair_count": expected_pairs,
                "density": present_pairs / max(expected_pairs, 1),
                "support_policy": policy,
                "missing_axes": [],
                "q_targets": sorted({target for row in rows for target in row["q_targets"]}),
            }
        )
        if policy != "eligible":
            warnings.append({"warning": "TCT_SPARSE_CELL", "cell_id": cell_id, "support_policy": policy, "chunk_membership_count": chunk_count, "axes": axes})
        for embedder_id in expected_embedders:
            group = signals_by_cell_embedder.get((cell_id, embedder_id), [])
            vectors = []
            for signal in group:
                record = records.get(embedder_id, {}).get(str(signal.get("row_key") or ""))
                if record is not None:
                    vectors.append(record.vector)
            embedder_policy = support_policy(len(group), hard_min, shrink_under)
            embedder_rows.append(
                {
                    "schema_version": SCHEMA_VERSION,
                    "artifact_kind": "python_tct_embedder_cell_support",
                    "task_id": TASK_ID,
                    "cell_id": cell_id,
                    "embedder_id": embedder_id,
                    "axes": axes,
                    "signal_count": len(group),
                    "cell_chunk_membership_count": chunk_count,
                    "support_policy": embedder_policy,
                    "prototype": vector_prototype(vectors),
                    "dda_scalar_prototype": scalar_summary(group, feature_names),
                }
            )
            if embedder_policy in {"empty", "low_support_fail_closed"}:
                warnings.append({"warning": "TCT_SPARSE_EMBEDDER_CELL", "cell_id": cell_id, "embedder_id": embedder_id, "support_policy": embedder_policy, "signal_count": len(group)})
    return cell_rows, embedder_rows, warnings


def category_support_rows(
    memberships: list[dict[str, Any]],
    signal_rows: list[dict[str, Any]],
    hard_min: int,
) -> list[dict[str, Any]]:
    membership_counts = Counter(row["axes"]["mutation_category"] for row in memberships)
    signal_counts = Counter(str(row.get("mutation_category") or "") for row in signal_rows)
    cell_counts: dict[str, set[str]] = defaultdict(set)
    for row in memberships:
        cell_counts[row["axes"]["mutation_category"]].add(row["cell_id"])
    rows = []
    for category in MUTATION_CATEGORIES:
        chunk_count = int(membership_counts.get(category, 0))
        rows.append(
            {
                "schema_version": SCHEMA_VERSION,
                "artifact_kind": "python_tct_mutation_category_support",
                "task_id": TASK_ID,
                "mutation_category": category,
                "chunk_membership_count": chunk_count,
                "present_signal_count": int(signal_counts.get(category, 0)),
                "cell_count": len(cell_counts.get(category, set())),
                "required_ship_gate_cell": True,
                "required_min_support": hard_min,
                "passes": chunk_count >= hard_min and int(signal_counts.get(category, 0)) >= hard_min,
            }
        )
    return rows


def edge_cases(hard_min: int) -> list[dict[str, Any]]:
    stale_schema = {"schema_version": CONSTELLATION_SCHEMA_VERSION - 1, "axes": {"mutation_category": {"enum": list(MUTATION_CATEGORIES)}, "chunk_kind": {"enum": list(CHUNK_KINDS)}}, "minimum_support": {"cell_hard_min": hard_min, "bayesian_shrink_under": hard_min}}
    try:
        validate_schema(stale_schema)
        stale_after = {"error_code": None}
    except TctCellError as exc:
        stale_after = {"error_code": json.loads(str(exc))["error_code"]}
    return [
        {
            "case": "empty_cell",
            "before": {"cell_id": "synthetic-empty", "support_count": 0},
            "action": "evaluate TCT support policy",
            "after": {"support_policy": support_policy(0, hard_min, hard_min * 2)},
            "expected": "empty",
            "pass": support_policy(0, hard_min, hard_min * 2) == "empty",
        },
        {
            "case": "low_support_cell",
            "before": {"cell_id": "synthetic-low-support", "support_count": max(1, hard_min - 1)},
            "action": "evaluate TCT support policy",
            "after": {"support_policy": support_policy(max(1, hard_min - 1), hard_min, hard_min * 2)},
            "expected": "low_support_fail_closed",
            "pass": support_policy(max(1, hard_min - 1), hard_min, hard_min * 2) == "low_support_fail_closed",
        },
        {
            "case": "stale_schema_version",
            "before": {"schema_version": CONSTELLATION_SCHEMA_VERSION - 1},
            "action": "validate constellation schema version",
            "after": stale_after,
            "expected": "TCT_STALE_SCHEMA_VERSION",
            "pass": stale_after.get("error_code") == "TCT_STALE_SCHEMA_VERSION",
        },
    ]


def command_python_tct_cells(args: argparse.Namespace) -> int:
    started = utc_now()
    if args.clean_output and args.output_root.exists():
        shutil.rmtree(args.output_root)
    args.output_root.mkdir(parents=True, exist_ok=True)
    args.fsv_root.mkdir(parents=True, exist_ok=True)
    before = {"output_root": tree_state(args.output_root), "fsv_root": tree_state(args.fsv_root)}

    full_ast_fsv, full_ast_fsv_sha = load_passing_json(args.full_ast_fsv, "TCT_FULL_AST_FSV_NOT_PASSING")
    smoke_fsv, smoke_fsv_sha = load_passing_json(args.smoke_manifest_fsv, "TCT_SMOKE_MANIFEST_FSV_NOT_PASSING")
    forward_fsv, forward_fsv_sha = load_passing_json(args.forward_fsv, "TCT_FORWARD_FSV_NOT_PASSING")
    dda_fsv, dda_fsv_sha = load_passing_json(args.dda_fsv, "TCT_DDA_FSV_NOT_PASSING")
    pairwise_fsv, pairwise_fsv_sha = load_passing_json(args.pairwise_fsv, "TCT_PAIRWISE_FSV_NOT_PASSING")
    matrix_fsv, matrix_fsv_sha = load_passing_json(args.matrix_fsv, "TCT_MATRIX_FSV_NOT_PASSING")
    schema = read_json(args.constellation_schema)
    support = validate_schema(schema)
    schema_sha = sha256_file(args.constellation_schema)
    manifest_by_key, manifest_summary = load_smoke_manifest(args.smoke_manifest)
    require(manifest_summary["rows"] == args.expected_chunk_count, "TCT_MANIFEST_COUNT_MISMATCH", "smoke manifest row count does not match expected count", {"summary": manifest_summary})
    records, load_summary = load_records(args.forward_root, manifest_by_key)
    expected_embedders = tuple(item.strip() for item in args.expected_embedders.split(",") if item.strip())
    require(tuple(sorted(records)) == tuple(sorted(expected_embedders)), "TCT_EMBEDDER_SET_MISMATCH", "forward records do not cover expected embedder ids", {"found": sorted(records), "expected": sorted(expected_embedders)})
    signal_rows, signal_by_pair = load_dda_rows(args.dda_root / "per_embedder_signals.jsonl")
    missing_by_pair = load_missing_rows(args.dda_root / "missing_signal_states.jsonl")
    require(len(signal_rows) + len(missing_by_pair) == len(manifest_by_key) * len(expected_embedders), "TCT_DDA_PAIR_ACCOUNTING_FAILED", "DDA present+missing pair accounting is incomplete", {"signal_rows": len(signal_rows), "missing_rows": len(missing_by_pair)})
    pairwise_rows, pairwise_summary = load_pairwise(args.pairwise_root / "pairwise_interactions.jsonl")
    source_hashes = {
        "full_ast_fsv_sha256": full_ast_fsv_sha,
        "smoke_manifest_fsv_sha256": smoke_fsv_sha,
        "forward_fsv_sha256": forward_fsv_sha,
        "dda_fsv_sha256": dda_fsv_sha,
        "pairwise_fsv_sha256": pairwise_fsv_sha,
        "matrix_fsv_sha256": matrix_fsv_sha,
        "smoke_manifest_sha256": sha256_file(args.smoke_manifest),
        "dda_summary_sha256": sha256_file(args.dda_root / "dda_signal_summary.json"),
        "matrix_summary_sha256": sha256_file(args.matrix_root / "summary.json"),
    }
    memberships, cell_by_key = membership_rows(manifest_by_key, signal_by_pair, missing_by_pair, expected_embedders, schema_sha, source_hashes)
    cell_support, embedder_support, warnings = build_support_rows(memberships, signal_rows, records, cell_by_key, expected_embedders, support["hard_min"], support["shrink_under"])
    category_rows = category_support_rows(memberships, signal_rows, support["hard_min"])
    edges = edge_cases(support["hard_min"])
    category_failures = [row for row in category_rows if not row["passes"]]
    write_jsonl(args.output_root / "tct_chunk_memberships.jsonl", memberships)
    write_jsonl(args.output_root / "tct_cell_support.jsonl", cell_support)
    write_jsonl(args.output_root / "tct_embedder_cell_support.jsonl", embedder_support)
    write_jsonl(args.output_root / "tct_mutation_category_support.jsonl", category_rows)
    write_jsonl(args.output_root / "tct_sparse_cell_warnings.jsonl", warnings)
    write_jsonl(args.output_root / "tct_pairwise_embedder_context.jsonl", pairwise_rows)
    write_jsonl(args.output_root / "edge_case_states.jsonl", edges)
    report = {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "artifact_kind": ARTIFACT_KIND,
        "created_at_utc": utc_now(),
        "source_of_truth": {
            "output_root": str(args.output_root),
            "chunk_memberships": str(args.output_root / "tct_chunk_memberships.jsonl"),
            "cell_support": str(args.output_root / "tct_cell_support.jsonl"),
            "embedder_cell_support": str(args.output_root / "tct_embedder_cell_support.jsonl"),
            "mutation_category_support": str(args.output_root / "tct_mutation_category_support.jsonl"),
        },
        "source_hashes": source_hashes,
        "constellation_schema_sha256": schema_sha,
        "manifest": manifest_summary,
        "forward_load_summary": load_summary,
        "pairwise_summary": pairwise_summary,
        "summary": {
            "chunk_membership_count": len(memberships),
            "cell_count": len(cell_support),
            "embedder_cell_support_rows": len(embedder_support),
            "mutation_category_support_rows": len(category_rows),
            "sparse_warning_count": len(warnings),
            "pairwise_context_rows": len(pairwise_rows),
            "required_category_failure_count": len(category_failures),
            "category_counts": dict(sorted(Counter(row["axes"]["mutation_category"] for row in memberships).items())),
        },
        "matrix_context": {
            "matrix_fsv_sha256": matrix_fsv_sha,
            "matrix_summary_sha256": source_hashes["matrix_summary_sha256"],
        },
    }
    dump_json(args.output_root / "tct_cell_report.json", report)
    after = {"output_root": tree_state(args.output_root), "fsv_root": tree_state(args.fsv_root)}
    readback_counts = {
        "chunk_memberships": sum(1 for _ in iter_jsonl(args.output_root / "tct_chunk_memberships.jsonl")),
        "cell_support": sum(1 for _ in iter_jsonl(args.output_root / "tct_cell_support.jsonl")),
        "embedder_cell_support": sum(1 for _ in iter_jsonl(args.output_root / "tct_embedder_cell_support.jsonl")),
        "mutation_category_support": sum(1 for _ in iter_jsonl(args.output_root / "tct_mutation_category_support.jsonl")),
        "sparse_warnings": sum(1 for _ in iter_jsonl(args.output_root / "tct_sparse_cell_warnings.jsonl")),
        "edge_case_states": sum(1 for _ in iter_jsonl(args.output_root / "edge_case_states.jsonl")),
    }
    passes = (
        readback_counts["chunk_memberships"] == args.expected_chunk_count
        and readback_counts["mutation_category_support"] == len(MUTATION_CATEGORIES)
        and not category_failures
        and all(case["pass"] for case in edges)
        and report["summary"]["cell_count"] > 0
        and report["source_hashes"]["smoke_manifest_sha256"] == manifest_summary["sha256"]
        and full_ast_fsv.get("passes") is True
        and smoke_fsv.get("passes") is True
        and forward_fsv.get("passes") is True
        and dda_fsv.get("passes") is True
        and pairwise_fsv.get("passes") is True
        and matrix_fsv.get("passes") is True
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
        "source_of_truth": {**report["source_of_truth"], "report": str(args.output_root / "tct_cell_report.json"), "fsv": str(args.fsv_root / "python_tct_cells_fsv.json")},
        "trigger": {"command": "python3 scripts/swebench_lite_ops.py python-tct-cells", "input_mode": "post249_smoke_v2"},
        "readback": {
            **readback_counts,
            "report_sha256": sha256_file(args.output_root / "tct_cell_report.json"),
            "chunk_memberships_sha256": sha256_file(args.output_root / "tct_chunk_memberships.jsonl"),
            "cell_support_sha256": sha256_file(args.output_root / "tct_cell_support.jsonl"),
            "embedder_cell_support_sha256": sha256_file(args.output_root / "tct_embedder_cell_support.jsonl"),
            "mutation_category_support_sha256": sha256_file(args.output_root / "tct_mutation_category_support.jsonl"),
            "artifact_state": artifact_state(args.output_root),
            "category_failures": category_failures,
            "first_cell_support": cell_support[0] if cell_support else None,
        },
        "edge_cases": edges,
    }
    dump_json(args.fsv_root / "python_tct_cells_fsv.json", fsv)
    final = read_json(args.fsv_root / "python_tct_cells_fsv.json")
    require(final.get("passes") is True, "TCT_FSV_FAILED", "TCT cell FSV failed", {"path": str(args.fsv_root / "python_tct_cells_fsv.json")})
    print(json.dumps(final, sort_keys=True, indent=2))
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--constellation-schema", type=Path, default=DEFAULT_SCHEMA_PATH)
    parser.add_argument("--smoke-manifest", type=Path, default=DEFAULT_SMOKE_MANIFEST)
    parser.add_argument("--forward-root", type=Path, default=DEFAULT_FORWARD_ROOT)
    parser.add_argument("--dda-root", type=Path, default=DEFAULT_DDA_ROOT)
    parser.add_argument("--pairwise-root", type=Path, default=DEFAULT_PAIRWISE_ROOT)
    parser.add_argument("--matrix-root", type=Path, default=DEFAULT_MATRIX_ROOT)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--fsv-root", type=Path, default=DEFAULT_FSV_ROOT)
    parser.add_argument("--full-ast-fsv", type=Path, default=DEFAULT_FULL_AST_FSV)
    parser.add_argument("--smoke-manifest-fsv", type=Path, default=DEFAULT_SMOKE_MANIFEST_FSV)
    parser.add_argument("--forward-fsv", type=Path, default=DEFAULT_FORWARD_FSV)
    parser.add_argument("--dda-fsv", type=Path, default=DEFAULT_DDA_FSV)
    parser.add_argument("--pairwise-fsv", type=Path, default=DEFAULT_PAIRWISE_FSV)
    parser.add_argument("--matrix-fsv", type=Path, default=DEFAULT_MATRIX_FSV)
    parser.add_argument("--expected-chunk-count", type=int, default=1000)
    parser.add_argument("--expected-embedders", default=",".join(EXPECTED_EMBEDDERS))
    parser.add_argument("--clean-output", action="store_true")
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        return command_python_tct_cells(args)
    except TctCellError as exc:
        print(f"PYTHON_TCT_CELLS_FAILED: {exc}")
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
