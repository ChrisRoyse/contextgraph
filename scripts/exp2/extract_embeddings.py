#!/usr/bin/env python3
"""T-006..T-008 strict embedding extraction through real MCP/RocksDB state."""

from __future__ import annotations

import argparse
from pathlib import Path
import sys
from typing import Any

import numpy as np

if __package__ in {None, ""}:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from scripts.exp2.common import (
    EMBEDDER_KEYS,
    EXPECTED_DIMS,
    Exp2Error,
    load_config,
    output_dir,
    read_json,
    repo_path,
    setup_logging,
    write_json,
)
from scripts.exp2.mcp_client import McpClient


DENSE_MAP = {
    "e1_semantic": "e1_semantic",
    "e2_temporal_recent": "e2_temporal_recent",
    "e3_temporal_periodic": "e3_temporal_periodic",
    "e4_temporal_positional": "e4_temporal_positional",
    "e5_causal_cause": "e5_causal_cause",
    "e7_code": "e7_code",
    "e8_graph_source": "e8_graph_source",
    "e9_hdc": "e9_hdc",
    "e10_paraphrase": "e10_paraphrase",
    "e11_entity": "e11_entity",
    "e14_bge_m3_dense": "e14_bge_m3_dense",
}


def sparse_top_k(record: dict[str, Any], prefix: str, k: int = 128) -> np.ndarray:
    values = record.get(f"{prefix}_values")
    indices = record.get(f"{prefix}_indices")
    if not isinstance(values, list) or not isinstance(indices, list):
        raise Exp2Error(f"Training record missing sparse arrays for {prefix}")
    if len(values) == 0:
        raise Exp2Error(f"{prefix} has zero sparse entries")
    pairs = sorted(zip(indices, values), key=lambda pair: abs(float(pair[1])), reverse=True)
    top = [float(v) for _, v in pairs[:k]]
    if len(top) < k:
        top.extend([0.0] * (k - len(top)))
    arr = np.asarray(top, dtype=np.float32)
    if not np.isfinite(arr).all() or np.linalg.norm(arr) <= 0:
        raise Exp2Error(f"{prefix} top-k projection is invalid")
    return arr


def token_mean(record: dict[str, Any]) -> np.ndarray:
    tokens = record.get("e12_token_embeddings")
    if not isinstance(tokens, list) or not tokens:
        raise Exp2Error("E12 token embeddings missing or empty")
    arr = np.asarray(tokens, dtype=np.float32)
    if arr.ndim != 2 or arr.shape[1] != EXPECTED_DIMS["e12_token_pooled"]:
        raise Exp2Error(f"E12 token shape invalid: {arr.shape}")
    pooled = arr.mean(axis=0)
    if not np.isfinite(pooled).all() or np.linalg.norm(pooled) <= 0:
        raise Exp2Error("E12 pooled vector invalid")
    return pooled.astype(np.float32)


def vector_from_record(record: dict[str, Any], key: str) -> np.ndarray:
    if key in DENSE_MAP:
        raw = record.get(DENSE_MAP[key])
        if not isinstance(raw, list):
            raise Exp2Error(f"Training record missing dense vector: {key}")
        arr = np.asarray(raw, dtype=np.float32)
    elif key == "e6_sparse_top_k":
        arr = sparse_top_k(record, "e6_sparse")
    elif key == "e13_splade_top_k":
        arr = sparse_top_k(record, "e13_splade")
    elif key == "e12_token_pooled":
        arr = token_mean(record)
    else:
        raise Exp2Error(f"Unknown embedder key: {key}")
    if arr.shape != (EXPECTED_DIMS[key],):
        raise Exp2Error(f"{key}: expected dim {EXPECTED_DIMS[key]}, got {arr.shape}")
    if not np.isfinite(arr).all() or np.linalg.norm(arr) <= 0:
        raise Exp2Error(f"{key}: non-finite or zero-norm vector")
    return arr


def extract_manifest(cfg: dict[str, Any], manifest_path: Path, output_npz: Path, storage_name: str) -> None:
    out = output_dir(cfg)
    log = setup_logging(out)
    manifest = read_json(manifest_path)
    inputs = manifest.get("inputs")
    if not isinstance(inputs, list) or not inputs:
        raise Exp2Error(f"Manifest has no inputs: {manifest_path}")
    expected_count = int(cfg["pca"]["fit_corpus_count"]) if storage_name == "pca" else int(cfg["corpus"]["expected_count"])
    if len(inputs) != expected_count:
        raise Exp2Error(f"{manifest_path}: expected {expected_count} inputs for {storage_name}, got {len(inputs)}")
    if storage_name != "pca":
        strata = manifest.get("strata")
        if not isinstance(strata, dict) or len(strata) != int(cfg["corpus"]["expected_strata"]):
            raise Exp2Error(f"{manifest_path}: expected {cfg['corpus']['expected_strata']} strata, got {strata}")
        bad_counts = {name: entry.get("count") for name, entry in strata.items() if entry.get("count") != 100}
        if bad_counts:
            raise Exp2Error(f"{manifest_path}: invalid per-stratum counts: {bad_counts}")
        if "deviations" in manifest:
            raise Exp2Error(f"{manifest_path}: production manifest contains deviations: {manifest['deviations']}")
    session_id = f"exp2-{storage_name}-{manifest.get('manifest_sha256', 'unsigned')[:16]}"
    storage_path = out / f"mcp_{storage_name}_rocksdb"
    extraction_log: dict[str, Any] = {
        "manifest": str(manifest_path),
        "output_npz": str(output_npz),
        "session_id": session_id,
        "source_of_truth": ["CF_FINGERPRINTS", "CF_TRAINING_RECORDS"],
        "batches": [],
    }
    with McpClient(cfg["run"]["mcp_binary"], storage_path, cfg["run"]["models_path"]) as client:
        extraction_log["fingerprints_before"] = client.call_tool("get_memetic_status", {})
        before_count = client.call_tool("count_training_records", {})
        extraction_log["training_records_before"] = before_count
        memory_ids: list[str] = []
        # Batch size 16: keeps each store_memories call <120s wall on cold cache,
        # well under server-side request_timeout=1800s. 64 was too aggressive on
        # first batch (timeout/deadlock seen on 14-embedder cold start).
        for start in range(0, len(inputs), 16):
            batch = inputs[start : start + 16]
            payload = {
                "sessionId": session_id,
                "operatorId": cfg["operator"],
                "causalHints": False,
                "items": [
                    {
                        "content": row["text"],
                        "importance": 0.5,
                        "rationale": f"EXP-2 {row['input_id']} {row['stratum']}",
                    }
                    for row in batch
                ],
            }
            result = client.call_tool("store_memories", payload, timeout=1800)
            if result.get("storedCount") != len(batch) or result.get("embedderCount") != 14:
                raise Exp2Error(f"store_memories returned unexpected result: {result}")
            ids = [item["fingerprintId"] for item in result["items"]]
            memory_ids.extend(ids)
            extraction_log["batches"].append({"start": start, "count": len(batch), "memory_ids": ids})
            log.info("Stored batch %s..%s through MCP", start, start + len(batch) - 1)

        for mid in memory_ids:
            fp = client.call_tool("get_memory_fingerprint", {"memoryId": mid, "includeVectorNorms": True})
            if fp.get("embeddersPresent") != 14 and fp.get("embedders_present") != 14:
                raise Exp2Error(f"Fingerprint {mid} does not report all 14 embedders: {fp}")

        export = client.call_tool(
            "export_training_corpus",
            {
                "filter": "session",
                "filterId": session_id,
                "maxMemories": len(inputs),
                "includeEmbeddings": True,
                "includeSparseVectors": True,
                "includeTokenEmbeddings": True,
                "includeEdges": False,
                "includeCausalLabels": False,
                "includeIncomingEdges": False,
                "includeTemporalLabels": True,
                "includeTuckerCore": False,
                "clearExisting": True,
            },
            timeout=1800,
        )
        if export.get("exported") != len(inputs) or export.get("errors") != 0:
            raise Exp2Error(f"export_training_corpus failed strict count check: {export}")
        extraction_log["export_training_corpus"] = export
        after_count = client.call_tool("count_training_records", {})
        if after_count.get("count") != len(inputs):
            raise Exp2Error(f"CF_TRAINING_RECORDS count mismatch after export: {after_count}")
        extraction_log["training_records_after"] = after_count
        fingerprints_after = client.call_tool("get_memetic_status", {})
        fingerprint_count = fingerprints_after.get("fingerprintCount", fingerprints_after.get("total_fingerprints"))
        if fingerprint_count != len(inputs):
            raise Exp2Error(f"CF_FINGERPRINTS count mismatch after extraction: {fingerprints_after}")
        extraction_log["fingerprints_after"] = fingerprints_after

        arrays: dict[str, list[np.ndarray]] = {key: [] for key in EMBEDDER_KEYS}
        for mid in memory_ids:
            rec = client.call_tool(
                "get_training_record",
                {"memoryId": mid, "includeVectors": True, "includeTokenEmbeddings": True, "includeEdges": False},
                timeout=300,
            )
            if not rec.get("found"):
                raise Exp2Error(f"Training record missing for {mid}")
            for key in EMBEDDER_KEYS:
                arrays[key].append(vector_from_record(rec, key))

    output_npz.parent.mkdir(parents=True, exist_ok=True)
    np.savez_compressed(
        output_npz,
        input_ids=np.asarray([row["input_id"] for row in inputs]),
        memory_ids=np.asarray(memory_ids),
        manifest_sha256=np.asarray([manifest.get("manifest_sha256", "")]),
        **{key: np.stack(value).astype(np.float32) for key, value in arrays.items()},
    )
    write_json(out / f"{storage_name}_extraction_log.json", extraction_log)
    log.info("Saved embedding archive: %s", output_npz)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--config", default="scripts/exp2/exp2.yml")
    parser.add_argument("--manifest")
    parser.add_argument("--output")
    parser.add_argument("--storage-name", default="audit")
    args = parser.parse_args()
    cfg = load_config(args.config)
    manifest = repo_path(args.manifest or cfg["corpus"]["manifest"])
    output = repo_path(args.output or (output_dir(cfg) / "embeddings_per_input.npz"))
    extract_manifest(cfg, manifest, output, args.storage_name)


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:
        raise SystemExit(f"EXP-2 embedding extraction failed: {exc}") from exc
