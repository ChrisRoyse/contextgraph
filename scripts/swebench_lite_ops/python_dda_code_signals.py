"""Materialize first-order Python DDA code signals from frozen embedder outputs."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import shutil
import sys
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any

import numpy as np

from .smoke_intelligence_matrices import (
    DEFAULT_FORWARD_ROOT,
    DEFAULT_CHUNKS_JSONL,
    DEFAULT_SMOKE_MANIFEST,
    SmokeMatrixError,
    artifact_state,
    completed_shard_entries,
    dump_json,
    iter_jsonl,
    load_chunk_manifest_window,
    load_records,
    load_smoke_manifest,
    read_json,
    require,
    sha256_file,
    tree_state,
    utc_now,
    write_jsonl,
)


SCHEMA_VERSION = 1
TASK_ID = "TASK-PY-DATA-017"
ARTIFACT_KIND = "python_first_order_dda_code_signals"
FORMULA_VERSION = "python_dda_first_order_v1"
DEFAULT_OUTPUT_ROOT = Path("/mnt/d/contextgraph/corpus/python-dda-signals-v1/per-embedder")
DEFAULT_FSV_ROOT = Path("/mnt/d/contextgraph/fsv/task-py-data-017-dda-signals-fsv")
DEFAULT_CONSTELLATION_SCHEMA = Path("/mnt/d/contextgraph/corpus/python-code-constellation-schema-v1/code_constellation_schema.json")
EXPECTED_EMBEDDERS = ("e1", "e6", "e7", "e8", "e10", "e12", "e13", "e14", "e2", "e3", "e4", "e9")
EPS = 1.0e-12


class DdaSignalError(SmokeMatrixError):
    pass


def canonical_json(payload: Any) -> str:
    return json.dumps(payload, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def sha256_text(payload: Any) -> str:
    return "sha256:" + hashlib.sha256(canonical_json(payload).encode("utf-8")).hexdigest()


def finite_float(value: float) -> float:
    out = float(value)
    if not math.isfinite(out):
        raise DdaSignalError("non-finite DDA scalar produced")
    return out


def vector_stats(vector: np.ndarray) -> dict[str, Any]:
    if vector.ndim != 1 or vector.size == 0:
        raise DdaSignalError("DDA_UNSUPPORTED_INSTRUMENT_OUTPUT_TYPE")
    if not np.issubdtype(vector.dtype, np.number):
        raise DdaSignalError("DDA_UNSUPPORTED_INSTRUMENT_OUTPUT_TYPE")
    if not np.isfinite(vector).all():
        raise DdaSignalError("DDA_NONFINITE_VECTOR")
    norm = float(np.linalg.norm(vector))
    if norm <= EPS:
        raise DdaSignalError("DDA_ZERO_VECTOR_UNSUPPORTED")
    abs_vector = np.abs(vector)
    weights = abs_vector / max(float(abs_vector.sum()), EPS)
    entropy = -float(np.sum(weights * np.log(weights + EPS))) / math.log(max(int(vector.size), 2))
    return {
        "vector_dim": int(vector.size),
        "vector_norm_l2": finite_float(norm),
        "vector_mean": finite_float(float(vector.mean())),
        "vector_std": finite_float(float(vector.std())),
        "vector_min": finite_float(float(vector.min())),
        "vector_max": finite_float(float(vector.max())),
        "vector_abs_mean": finite_float(float(abs_vector.mean())),
        "vector_zero_fraction": finite_float(float(np.mean(abs_vector <= EPS))),
        "vector_positive_fraction": finite_float(float(np.mean(vector > EPS))),
        "vector_negative_fraction": finite_float(float(np.mean(vector < -EPS))),
        "vector_entropy_proxy": finite_float(entropy),
    }


def cosine_to_centroid(vector: np.ndarray, centroid: np.ndarray | None) -> tuple[float | None, str | None]:
    if centroid is None:
        return None, "LOW_SUPPORT_CENTROID"
    denom = float(np.linalg.norm(vector) * np.linalg.norm(centroid))
    if denom <= EPS:
        return None, "DEGENERATE_ZERO_NORM"
    return finite_float(float(np.dot(vector, centroid) / denom)), None


def manifest_features(row: dict[str, Any], max_source_bytes: int, max_token_estimate: int) -> dict[str, Any]:
    line_range = row.get("line_range") or {}
    oracle = row.get("oracle_linkage") or {}
    source_bytes = int(row.get("source_byte_len") or 0)
    tokens = int(row.get("token_estimate") or 0)
    line_span = max(1, int(line_range.get("end") or 0) - int(line_range.get("start") or 0) + 1)
    return {
        "source_byte_len": source_bytes,
        "token_estimate": tokens,
        "line_span": line_span,
        "lexical_delta_score": finite_float(source_bytes / max(max_source_bytes, 1)),
        "token_delta_score": finite_float(tokens / max(max_token_estimate, 1)),
        "structural_delta_score": finite_float(math.log1p(line_span) / math.log1p(5000)),
        "is_test_chunk": bool(str(row.get("chunk_kind") or "").startswith("test_") or "/tests/" in str(row.get("relative_path") or "")),
        "oracle_resolved": bool(oracle.get("resolved")),
        "oracle_all_passed": bool(oracle.get("all_passed")),
        "oracle_fail_to_pass_count": int(oracle.get("fail_to_pass_count") or 0),
        "oracle_pass_to_pass_count": int(oracle.get("pass_to_pass_count") or 0),
    }


def load_quarantine_by_key(forward_root: Path, shards: list[dict[str, Any]], manifest_by_key: dict[str, dict[str, Any]]) -> dict[tuple[str, str], dict[str, Any]]:
    out: dict[tuple[str, str], dict[str, Any]] = {}
    for shard in shards:
        path = forward_root / "shards" / str(shard.get("shard_id")) / "python_forward_cache_quarantine.jsonl"
        if not path.exists():
            continue
        for _, row in iter_jsonl(path):
            key = "|".join(
                [
                    str(row.get("task_instance_id") or ""),
                    str(row.get("workspace_state") or ""),
                    str(row.get("chunk_id") or ""),
                    str(row.get("source_sha256") or ""),
                ]
            )
            if key in manifest_by_key:
                out[(str(row.get("embedder_id") or "unknown"), key)] = row
    return out


def centroid_map(records: dict[str, dict[str, Any]], manifest_by_key: dict[str, dict[str, Any]]) -> dict[tuple[str, str, str], np.ndarray]:
    grouped: dict[tuple[str, str, str], list[np.ndarray]] = defaultdict(list)
    for embedder_id, rows in records.items():
        for key, record in rows.items():
            manifest = manifest_by_key[key]
            grouped[(embedder_id, str(manifest.get("mutation_category") or "unknown"), str(manifest.get("chunk_kind") or "unknown"))].append(record.vector)
    return {key: np.mean(np.stack(vectors, axis=0), axis=0) for key, vectors in grouped.items() if len(vectors) >= 2}


def q_targets(row: dict[str, Any]) -> list[str]:
    oracle = row.get("oracle_linkage") or {}
    targets = ["Q1_claim_existence", "Q2_work_status", "Q4_edge_risk", "Q5_reality_impact"]
    if oracle.get("all_passed") is False or oracle.get("resolved") is False:
        targets.append("Q3_failure_reason")
    return targets


def success_signal_row(
    *,
    embedder_id: str,
    key: str,
    record: Any,
    manifest: dict[str, Any],
    centroid: np.ndarray | None,
    constellation_schema_sha256: str,
    max_source_bytes: int,
    max_token_estimate: int,
) -> dict[str, Any]:
    stats = vector_stats(record.vector)
    cosine, cosine_reason = cosine_to_centroid(record.vector, centroid)
    features = manifest_features(manifest, max_source_bytes, max_token_estimate)
    payload = {
        "schema_version": SCHEMA_VERSION,
        "artifact_kind": "python_first_order_dda_signal",
        "task_id": TASK_ID,
        "formula_version": FORMULA_VERSION,
        "embedder_id": embedder_id,
        "instrument_id": record.instrument_id,
        "row_key": key,
        "chunk_id": record.chunk_id,
        "source_sha256": record.source_sha256,
        "record_sha256": record.record_sha256,
    }
    signal = {
        **stats,
        "cosine_to_category_kind_centroid": cosine,
        "cosine_to_category_kind_centroid_state": cosine_reason,
        **features,
    }
    return {
        **payload,
        "signal_id": sha256_text(payload),
        "status": "present",
        "unit": "unitless_or_declared_scalar",
        "q_targets": q_targets(manifest),
        "task_instance_id": record.task_instance_id,
        "repo": record.repo,
        "relative_path": record.relative_path,
        "workspace_state": record.workspace_state,
        "mutation_category": record.mutation_category,
        "chunk_kind": record.chunk_kind,
        "node_kind": record.node_kind,
        "oracle_all_passed": record.oracle_all_passed,
        "oracle_resolved": record.oracle_resolved,
        "input_hashes": {
            "cache_key_sha256": record.cache_key_sha256,
            "vector_sha256": record.vector_sha256,
            "record_sha256": record.record_sha256,
            "source_sha256": record.source_sha256,
            "constellation_schema_sha256": constellation_schema_sha256,
        },
        "signals": signal,
    }


def missing_signal_row(embedder_id: str, key: str, manifest: dict[str, Any], quarantine: dict[str, Any] | None) -> dict[str, Any]:
    reason = str((quarantine or {}).get("error_code") or "DDA_SIGNAL_MISSING_EMBEDDING")
    payload = {
        "schema_version": SCHEMA_VERSION,
        "artifact_kind": "python_first_order_dda_missing_signal_state",
        "task_id": TASK_ID,
        "formula_version": FORMULA_VERSION,
        "embedder_id": embedder_id,
        "row_key": key,
        "reason": reason,
    }
    return {
        **payload,
        "signal_state_id": sha256_text(payload),
        "status": "quarantined" if quarantine else "missing",
        "chunk_id": str(manifest.get("chunk_id") or ""),
        "task_instance_id": str(manifest.get("task_instance_id") or ""),
        "repo": str(manifest.get("repo") or ""),
        "workspace_state": str(manifest.get("workspace_state") or ""),
        "mutation_category": str(manifest.get("mutation_category") or ""),
        "chunk_kind": str(manifest.get("chunk_kind") or ""),
        "q_targets": q_targets(manifest),
        "quarantine": quarantine,
    }


def patch_rows(signal_rows: list[dict[str, Any]]) -> list[dict[str, Any]]:
    grouped: dict[tuple[str, str, str], list[dict[str, Any]]] = defaultdict(list)
    for row in signal_rows:
        grouped[(row["task_instance_id"], row["mutation_category"], row["embedder_id"])].append(row)
    out = []
    for (task_instance_id, mutation_category, embedder_id), rows in sorted(grouped.items()):
        norms = [row["signals"]["vector_norm_l2"] for row in rows]
        cosines = [row["signals"]["cosine_to_category_kind_centroid"] for row in rows if row["signals"]["cosine_to_category_kind_centroid"] is not None]
        payload = {
            "schema_version": SCHEMA_VERSION,
            "artifact_kind": "python_patch_first_order_dda_signal",
            "task_id": TASK_ID,
            "formula_version": FORMULA_VERSION,
            "task_instance_id": task_instance_id,
            "mutation_category": mutation_category,
            "embedder_id": embedder_id,
            "chunk_signal_count": len(rows),
        }
        out.append(
            {
                **payload,
                "patch_signal_id": sha256_text(payload),
                "repo": rows[0]["repo"],
                "q_targets": sorted({target for row in rows for target in row["q_targets"]}),
                "signals": {
                    "mean_vector_norm_l2": finite_float(float(np.mean(norms))),
                    "std_vector_norm_l2": finite_float(float(np.std(norms))),
                    "mean_cosine_to_category_kind_centroid": finite_float(float(np.mean(cosines))) if cosines else None,
                    "centroid_signal_count": len(cosines),
                    "oracle_pass_ratio": finite_float(sum(row["oracle_all_passed"] == "True" for row in rows) / len(rows)),
                    "test_chunk_ratio": finite_float(sum(row["signals"]["is_test_chunk"] for row in rows) / len(rows)),
                },
            }
        )
    return out


def edge_cases() -> list[dict[str, Any]]:
    cases = []
    for name, vector in [
        ("zero_vector", np.zeros(4, dtype=np.float32)),
        ("unsupported_instrument_output_type", np.asarray([object()], dtype=object)),
    ]:
        before = {"vector_repr": str(vector.tolist()), "dtype": str(vector.dtype)}
        try:
            vector_stats(vector)
            after = {"error_code": None}
        except DdaSignalError as exc:
            after = {"error_code": str(exc)}
        expected = "DDA_ZERO_VECTOR_UNSUPPORTED" if name == "zero_vector" else "DDA_UNSUPPORTED_INSTRUMENT_OUTPUT_TYPE"
        cases.append({"case": name, "before": before, "action": "compute first-order vector stats", "after": after, "expected": expected, "pass": after["error_code"] == expected})
    return cases


def command_python_dda_code_signals(args: argparse.Namespace) -> int:
    started = utc_now()
    if args.clean_output and args.output_root.exists():
        shutil.rmtree(args.output_root)
    args.output_root.mkdir(parents=True, exist_ok=True)
    args.fsv_root.mkdir(parents=True, exist_ok=True)
    before = {"output_root": tree_state(args.output_root), "fsv_root": tree_state(args.fsv_root)}
    if args.input_mode == "smoke-manifest":
        manifest_by_key, manifest_summary = load_smoke_manifest(args.smoke_manifest)
        records, load_summary = load_records(args.forward_root, manifest_by_key)
        shard_entries = list(read_json(args.forward_root / "python_forward_cache_corpus_manifest.json").get("shards") or [])
        source_checkpoint = None
    elif args.input_mode == "completed-shards":
        shard_entries, source_checkpoint = completed_shard_entries(args.forward_root, args.checkpoint, args.shard_plan, args.max_completed_shards, args.min_completed_shards)
        manifest_by_key, manifest_summary = load_chunk_manifest_window(args.chunks_jsonl, shard_entries)
        records, load_summary = load_records(args.forward_root, manifest_by_key, shard_entries)
    else:
        raise DdaSignalError(f"unsupported input mode: {args.input_mode}")
    expected_embedders = tuple(item.strip() for item in args.expected_embedders.split(",") if item.strip())
    require(set(records).issubset(set(expected_embedders)), "forward cache contains an unexpected embedder id")
    require(len(records) == len(expected_embedders), f"expected {len(expected_embedders)} embedders, found {len(records)}")
    constellation_sha = sha256_file(args.constellation_schema)
    centroids = centroid_map(records, manifest_by_key)
    max_source_bytes = max(int(row.get("source_byte_len") or 0) for row in manifest_by_key.values())
    max_token_estimate = max(int(row.get("token_estimate") or 0) for row in manifest_by_key.values())
    quarantines = load_quarantine_by_key(args.forward_root, shard_entries, manifest_by_key)
    signal_rows = []
    missing_rows = []
    for key, manifest in sorted(manifest_by_key.items()):
        for embedder_id in expected_embedders:
            record = records.get(embedder_id, {}).get(key)
            if record is None:
                missing_rows.append(missing_signal_row(embedder_id, key, manifest, quarantines.get((embedder_id, key))))
                continue
            centroid = centroids.get((embedder_id, str(manifest.get("mutation_category")), str(manifest.get("chunk_kind"))))
            try:
                signal_rows.append(
                    success_signal_row(
                        embedder_id=embedder_id,
                        key=key,
                        record=record,
                        manifest=manifest,
                        centroid=centroid,
                        constellation_schema_sha256=constellation_sha,
                        max_source_bytes=max_source_bytes,
                        max_token_estimate=max_token_estimate,
                    )
                )
            except DdaSignalError as exc:
                missing_rows.append(missing_signal_row(embedder_id, key, manifest, {"error_code": str(exc), "reason": "first-order DDA signal validation failed"}))
    patch_signal_rows = patch_rows(signal_rows)
    write_jsonl(args.output_root / "per_embedder_signals.jsonl", signal_rows)
    write_jsonl(args.output_root / "patch_signals.jsonl", patch_signal_rows)
    write_jsonl(args.output_root / "missing_signal_states.jsonl", missing_rows)
    edges = edge_cases()
    write_jsonl(args.output_root / "edge_case_signal_states.jsonl", edges)
    accounted_pairs = len(signal_rows) + len(missing_rows)
    expected_pairs = len(manifest_by_key) * len(expected_embedders)
    summary = {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "artifact_kind": ARTIFACT_KIND,
        "started_at_utc": started,
        "created_at_utc": utc_now(),
        "formula_version": FORMULA_VERSION,
        "source_of_truth": {
            "output_root": str(args.output_root),
            "per_embedder_signals": str(args.output_root / "per_embedder_signals.jsonl"),
            "patch_signals": str(args.output_root / "patch_signals.jsonl"),
            "missing_signal_states": str(args.output_root / "missing_signal_states.jsonl"),
        },
        "input_mode": args.input_mode,
        "source_checkpoint": source_checkpoint,
        "manifest": manifest_summary,
        "load_summary": load_summary,
        "summary": {
            "manifest_rows": len(manifest_by_key),
            "expected_embedder_count": len(expected_embedders),
            "expected_pairs": expected_pairs,
            "signal_row_count": len(signal_rows),
            "missing_signal_state_count": len(missing_rows),
            "accounted_pairs": accounted_pairs,
            "patch_signal_row_count": len(patch_signal_rows),
            "by_embedder": dict(sorted(Counter(row["embedder_id"] for row in signal_rows).items())),
            "missing_by_embedder": dict(sorted(Counter(row["embedder_id"] for row in missing_rows).items())),
            "q_target_counts": dict(sorted(Counter(target for row in signal_rows for target in row["q_targets"]).items())),
        },
    }
    dump_json(args.output_root / "dda_signal_summary.json", summary)
    after = {"output_root": tree_state(args.output_root), "fsv_root": tree_state(args.fsv_root)}
    readback_summary = read_json(args.output_root / "dda_signal_summary.json")
    readback_counts = {
        "per_embedder_signals": sum(1 for _ in iter_jsonl(args.output_root / "per_embedder_signals.jsonl")),
        "patch_signals": sum(1 for _ in iter_jsonl(args.output_root / "patch_signals.jsonl")),
        "missing_signal_states": sum(1 for _ in iter_jsonl(args.output_root / "missing_signal_states.jsonl")),
        "edge_case_signal_states": sum(1 for _ in iter_jsonl(args.output_root / "edge_case_signal_states.jsonl")),
    }
    known_key = sorted(manifest_by_key)[0]
    known_pair = [row for row in signal_rows if row["row_key"] == known_key][:2]
    missing_present = bool(missing_rows)
    passes = (
        readback_summary.get("summary", {}).get("accounted_pairs") == expected_pairs
        and readback_counts["per_embedder_signals"] == len(signal_rows)
        and readback_counts["missing_signal_states"] == len(missing_rows)
        and readback_counts["patch_signals"] == len(patch_signal_rows)
        and len(known_pair) == 2
        and missing_present
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
        "source_of_truth": {**summary["source_of_truth"], "summary": str(args.output_root / "dda_signal_summary.json"), "fsv": str(args.fsv_root / "python_dda_code_signals_fsv.json")},
        "trigger": {"command": "python3 scripts/swebench_lite_ops.py python-dda-code-signals", "formula_version": FORMULA_VERSION},
        "readback": {
            **readback_counts,
            "summary_sha256": sha256_file(args.output_root / "dda_signal_summary.json"),
            "per_embedder_signals_sha256": sha256_file(args.output_root / "per_embedder_signals.jsonl"),
            "missing_signal_states_sha256": sha256_file(args.output_root / "missing_signal_states.jsonl"),
            "patch_signals_sha256": sha256_file(args.output_root / "patch_signals.jsonl"),
            "known_chunk_pair": known_pair,
            "first_missing_signal_state": missing_rows[0] if missing_rows else None,
            "artifact_state": artifact_state(args.output_root),
        },
        "edge_cases": [
            {"case": "missing_embedding", "before": {"expected_pairs": expected_pairs}, "action": "account all manifest x embedder pairs", "after": {"missing_signal_state_count": len(missing_rows), "accounted_pairs": accounted_pairs}, "expected": "missing/quarantined states are explicit rows, not zeros", "pass": missing_present and accounted_pairs == expected_pairs},
            *edges,
        ],
    }
    dump_json(args.fsv_root / "python_dda_code_signals_fsv.json", fsv)
    final = read_json(args.fsv_root / "python_dda_code_signals_fsv.json")
    require(final.get("passes") is True, f"DDA signal FSV failed: {args.fsv_root / 'python_dda_code_signals_fsv.json'}")
    print(json.dumps(final, sort_keys=True, indent=2))
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--forward-root", type=Path, default=DEFAULT_FORWARD_ROOT)
    parser.add_argument("--input-mode", choices=["smoke-manifest", "completed-shards"], default="smoke-manifest")
    parser.add_argument("--smoke-manifest", type=Path, default=DEFAULT_SMOKE_MANIFEST)
    parser.add_argument("--chunks-jsonl", type=Path, default=DEFAULT_CHUNKS_JSONL)
    parser.add_argument("--checkpoint", type=Path, default=None)
    parser.add_argument("--shard-plan", type=Path, default=None)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--fsv-root", type=Path, default=DEFAULT_FSV_ROOT)
    parser.add_argument("--constellation-schema", type=Path, default=DEFAULT_CONSTELLATION_SCHEMA)
    parser.add_argument("--expected-embedders", default=",".join(EXPECTED_EMBEDDERS))
    parser.add_argument("--min-completed-shards", type=int, default=1)
    parser.add_argument("--max-completed-shards", type=int, default=None)
    parser.add_argument("--clean-output", action="store_true")
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        return command_python_dda_code_signals(args)
    except SmokeMatrixError as exc:
        print(f"PYTHON_DDA_CODE_SIGNALS_FAILED: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
