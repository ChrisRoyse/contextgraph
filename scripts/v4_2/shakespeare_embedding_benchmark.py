#!/usr/bin/env python3
"""Benchmark and analyze the real Shakespeare extended-embedding store.

This is intentionally a persisted, full-state-verification tool, not a
notebook. It reads the real LMDB sidecar, the real Shakespeare CAMD pairs, and
the real sonnets centroid file, then writes one bounded JSON report that can be
reopened and inspected as the source of truth.

Hard rules:
  * no mock data on the happy path;
  * no fallback if an input is missing, empty, malformed, or dimensionally wrong;
  * atomic report writes, with partial cleanup on failure;
  * explicit close/gc cleanup so repeated runs do not accumulate processes or
    large Python artifacts.
"""
from __future__ import annotations

import argparse
import gc
import hashlib
import json
import logging
import math
import os
import sys
import tempfile
import time
import traceback
from pathlib import Path
from typing import Any

sys.dont_write_bytecode = True

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from v4_2.extended_store import DEFAULT_PATH as DEFAULT_STORE_PATH
from v4_2.extended_store import ExtendedStore


logger = logging.getLogger("v4_2.shakespeare_embedding_benchmark")


DEFAULT_CENTROIDS_PATH = Path(
    "~/.contextgraph/models/extended_ensemble/sonnets_constellation_extended.npz"
).expanduser()
DEFAULT_PAIRS_PATH = Path("~/.contextgraph/training/v4_2_camd_lmdb.jsonl").expanduser()
DEFAULT_OUTPUT_PATH = Path(
    "~/.contextgraph/reports/shakespeare_embedding_benchmark_latest.json"
).expanduser()

EXTENDED_STYLE_WEIGHTS = {
    "bge_m3_dense": 0.15,
    "bge_m3_colbert": 0.15,
    "jina_v3_classif": 0.30,
    "qwen3_instruct": 0.30,
    "bge_m3_sparse": 0.10,
}

VIEW_ORDER = [
    "bge_m3_dense",
    "bge_m3_colbert",
    "bge_m3_sparse",
    "jina_v3_classif",
    "qwen3_instruct",
    "reranker_score_vs_sonnets",
]

VIEW_MEANINGS = {
    "bge_m3_dense": {
        "number_type": "cosine_to_sonnets_centroid",
        "range_note": "Cosine-like score; higher means closer to the sonnets dense centroid.",
        "meaning": (
            "Global semantic/style proximity in BGE-M3 multilingual dense space. "
            "It tends to move with broad thematic and genre similarity, not just exact words."
        ),
    },
    "bge_m3_colbert": {
        "number_type": "cosine_of_mean_token_vectors_to_colbert_centroid",
        "range_note": "Cosine-like score; higher means local token-vector geometry resembles the centroid.",
        "meaning": (
            "Token-level phrase and local wording signal. The benchmark uses the same "
            "mean-pooled centroid proxy used by the CAMD LMDB miner."
        ),
    },
    "bge_m3_sparse": {
        "number_type": "sparse_cosine_to_sonnets_lexical_centroid",
        "range_note": "Sparse cosine; usually lower because only overlapping weighted terms contribute.",
        "meaning": (
            "Lexical overlap with the sonnets anchor. High values mean the record shares "
            "weighted archaic/style terms; low values can still be good if other views see style."
        ),
    },
    "jina_v3_classif": {
        "number_type": "cosine_to_sonnets_centroid",
        "range_note": "Cosine-like score; higher means the classification-task embedding sees the same style class.",
        "meaning": (
            "Task-LoRA classification geometry. This is a strong style-class signal, "
            "less tied to exact lexical overlap than the sparse view."
        ),
    },
    "qwen3_instruct": {
        "number_type": "cosine_to_instruction_conditioned_sonnets_centroid",
        "range_note": "Cosine-like score; higher means closer under the Shakespeare-vs-modern instruction.",
        "meaning": (
            "Instruction-conditioned style judgment. This view asks whether text behaves "
            "like early modern Shakespearean verse rather than ordinary prose."
        ),
    },
    "reranker_score_vs_sonnets": {
        "number_type": "cross_encoder_logit",
        "range_note": "Raw logit, not a cosine and not currently part of extended_combined_reward.",
        "meaning": (
            "BGE reranker relevance score against the Shakespeare reference query. "
            "It should be analyzed separately because logits are not on the cosine scale."
        ),
    },
    "extended_combined_reward": {
        "number_type": "weighted_sum_of_extended_cosines",
        "range_note": "Weighted aggregate using EXTENDED_STYLE_WEIGHTS.",
        "meaning": (
            "Current CAMD extended-style reward. It intentionally blends dense, token, lexical, "
            "classification, and instruction-conditioned evidence."
        ),
    },
}


class BenchmarkError(RuntimeError):
    """Raised when verification or real-data validation fails."""


def _json_default(obj: Any) -> Any:
    if isinstance(obj, np.integer):
        return int(obj)
    if isinstance(obj, np.floating):
        return float(obj)
    if isinstance(obj, np.ndarray):
        return obj.tolist()
    raise TypeError(f"object of type {type(obj).__name__} is not JSON serializable")


def _sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def _path_state(path: Path) -> dict[str, Any]:
    path = path.expanduser()
    if not path.exists():
        return {"path": str(path), "exists": False}
    if path.is_file():
        return {
            "path": str(path),
            "exists": True,
            "kind": "file",
            "size_bytes": path.stat().st_size,
            "sha256": _sha256_file(path),
        }
    if path.is_dir():
        total = 0
        files = 0
        for root, _, names in os.walk(path):
            for name in names:
                files += 1
                total += (Path(root) / name).stat().st_size
        return {
            "path": str(path),
            "exists": True,
            "kind": "directory",
            "file_count": files,
            "size_bytes": total,
        }
    return {"path": str(path), "exists": True, "kind": "other"}


def _require_file(path: Path, label: str) -> None:
    if not path.exists():
        raise BenchmarkError(f"{label} does not exist: {path}")
    if not path.is_file():
        raise BenchmarkError(f"{label} is not a file: {path}")
    if path.stat().st_size == 0:
        raise BenchmarkError(f"{label} is empty: {path}")


def _cosine(a: np.ndarray, b: np.ndarray, label: str) -> float:
    if a.shape != b.shape:
        raise BenchmarkError(f"{label}: shape mismatch {a.shape} != {b.shape}")
    if not np.isfinite(a).all() or not np.isfinite(b).all():
        raise BenchmarkError(f"{label}: non-finite vector values")
    na = float(np.linalg.norm(a))
    nb = float(np.linalg.norm(b))
    if na <= 1e-12 or nb <= 1e-12:
        raise BenchmarkError(f"{label}: zero-norm vector")
    return float(np.dot(a, b) / (na * nb))


def _sparse_cosine(a: dict[int, float], b: dict[int, float], label: str) -> float:
    if not a:
        raise BenchmarkError(f"{label}: empty record sparse vector")
    if not b:
        raise BenchmarkError(f"{label}: empty centroid sparse vector")
    ks = set(a) & set(b)
    dot = sum(float(a[k]) * float(b[k]) for k in ks)
    na = math.sqrt(sum(float(v) * float(v) for v in a.values()))
    nb = math.sqrt(sum(float(v) * float(v) for v in b.values()))
    if na <= 1e-12 or nb <= 1e-12:
        raise BenchmarkError(f"{label}: zero-norm sparse vector")
    return float(dot / (na * nb))


def _check_dense(name: str, arr: np.ndarray, expected_shape: tuple[int, ...]) -> np.ndarray:
    if arr.shape != expected_shape:
        raise BenchmarkError(f"{name}: expected shape {expected_shape}, got {arr.shape}")
    if arr.dtype != np.float32:
        raise BenchmarkError(f"{name}: expected dtype float32, got {arr.dtype}")
    if not np.isfinite(arr).all():
        raise BenchmarkError(f"{name}: contains non-finite values")
    return arr


def _load_centroids(path: Path) -> dict[str, Any]:
    _require_file(path, "centroids file")
    z = np.load(path, allow_pickle=True)
    required = {
        "bge_m3_dense_centroid",
        "jina_v3_centroid",
        "qwen3_centroid",
        "bge_m3_colbert_centroid",
        "bge_m3_sparse_centroid_ids",
        "bge_m3_sparse_centroid_weights",
        "n_members",
        "constellation_id",
    }
    missing = required - set(z.files)
    if missing:
        raise BenchmarkError(f"centroids file missing keys: {sorted(missing)}")
    sparse_ids = z["bge_m3_sparse_centroid_ids"]
    sparse_weights = z["bge_m3_sparse_centroid_weights"]
    if sparse_ids.ndim != 1 or sparse_weights.ndim != 1:
        raise BenchmarkError("centroid sparse ids/weights must be 1D")
    if sparse_ids.shape[0] != sparse_weights.shape[0]:
        raise BenchmarkError(
            "centroid sparse ids/weights length mismatch: "
            f"{sparse_ids.shape[0]} != {sparse_weights.shape[0]}"
        )
    if sparse_ids.shape[0] == 0:
        raise BenchmarkError("centroid sparse vector is empty")
    return {
        "bge_m3_dense": _check_dense(
            "bge_m3_dense_centroid", z["bge_m3_dense_centroid"], (1024,)
        ),
        "jina_v3_classif": _check_dense(
            "jina_v3_centroid", z["jina_v3_centroid"], (1024,)
        ),
        "qwen3_instruct": _check_dense(
            "qwen3_centroid", z["qwen3_centroid"], (1024,)
        ),
        "bge_m3_colbert": _check_dense(
            "bge_m3_colbert_centroid", z["bge_m3_colbert_centroid"], (1024,)
        ),
        "bge_m3_sparse": {
            int(tid): float(weight)
            for tid, weight in zip(sparse_ids.tolist(), sparse_weights.tolist())
        },
        "n_members": int(z["n_members"]),
        "constellation_id": str(z["constellation_id"]),
    }


def _distribution(values: list[float]) -> dict[str, float]:
    if not values:
        raise BenchmarkError("cannot summarize empty value list")
    arr = np.asarray(values, dtype=np.float64)
    if not np.isfinite(arr).all():
        raise BenchmarkError("distribution contains non-finite values")
    return {
        "min": float(np.min(arr)),
        "p05": float(np.quantile(arr, 0.05)),
        "p25": float(np.quantile(arr, 0.25)),
        "mean": float(np.mean(arr)),
        "median": float(np.median(arr)),
        "p75": float(np.quantile(arr, 0.75)),
        "p95": float(np.quantile(arr, 0.95)),
        "max": float(np.max(arr)),
        "std": float(np.std(arr, ddof=1)) if arr.size > 1 else 0.0,
    }


def _correlation_matrix(rows: list[dict[str, Any]], names: list[str]) -> dict[str, Any]:
    if len(rows) < 2:
        raise BenchmarkError("need at least two records for correlation matrix")
    mat = np.asarray([[float(r[name]) for name in names] for r in rows], dtype=np.float64)
    if not np.isfinite(mat).all():
        raise BenchmarkError("correlation matrix input contains non-finite values")
    corr = np.corrcoef(mat, rowvar=False)
    if not np.isfinite(corr).all():
        raise BenchmarkError("correlation matrix contains non-finite values")
    pairs = []
    for i, left in enumerate(names):
        for j in range(i + 1, len(names)):
            pairs.append({
                "left": left,
                "right": names[j],
                "pearson": float(corr[i, j]),
                "abs_pearson": float(abs(corr[i, j])),
            })
    pairs.sort(key=lambda x: (-x["abs_pearson"], x["left"], x["right"]))
    return {
        "names": names,
        "matrix": corr.tolist(),
        "strongest_pairs": pairs[:10],
        "weakest_pairs": sorted(pairs, key=lambda x: (x["abs_pearson"], x["left"], x["right"]))[:10],
    }


def _pca(rows: list[dict[str, Any]], names: list[str], n_components: int = 3) -> dict[str, Any]:
    if len(rows) < 2:
        raise BenchmarkError("need at least two records for PCA")
    mat = np.asarray([[float(r[name]) for name in names] for r in rows], dtype=np.float64)
    mat = (mat - mat.mean(axis=0)) / mat.std(axis=0, ddof=1)
    if not np.isfinite(mat).all():
        raise BenchmarkError("PCA standardized matrix contains non-finite values")
    _, singular, vt = np.linalg.svd(mat, full_matrices=False)
    variance = singular * singular
    ratios = variance / variance.sum()
    components = []
    for idx in range(min(n_components, vt.shape[0])):
        loadings = {
            names[j]: float(vt[idx, j])
            for j in range(len(names))
        }
        components.append({
            "component": idx + 1,
            "explained_variance_ratio": float(ratios[idx]),
            "loadings": loadings,
        })
    return {"features": names, "components": components}


def _cohen_d(chosen: list[float], rejected: list[float]) -> float:
    if len(chosen) < 2 or len(rejected) < 2:
        raise BenchmarkError("need at least two chosen and rejected values for Cohen's d")
    a = np.asarray(chosen, dtype=np.float64)
    b = np.asarray(rejected, dtype=np.float64)
    pooled = math.sqrt(((a.size - 1) * float(np.var(a, ddof=1)) +
                        (b.size - 1) * float(np.var(b, ddof=1))) / (a.size + b.size - 2))
    if pooled <= 1e-12:
        raise BenchmarkError("pooled standard deviation is zero for Cohen's d")
    return float((float(np.mean(a)) - float(np.mean(b))) / pooled)


def _recommended_weights(pair_effects: dict[str, dict[str, float]]) -> dict[str, float]:
    raw = {
        name: max(0.0, effect["cohen_d"])
        for name, effect in pair_effects.items()
        if name in EXTENDED_STYLE_WEIGHTS
    }
    total = sum(raw.values())
    if total <= 1e-12:
        raise BenchmarkError("cannot recommend weights: all pair effects are non-positive")
    return {name: float(value / total) for name, value in sorted(raw.items())}


def _load_pair_stats(path: Path, pair_limit: int | None) -> dict[str, Any]:
    _require_file(path, "CAMD pairs JSONL")
    if pair_limit is not None and pair_limit <= 0:
        raise BenchmarkError(f"pair_limit must be positive when set, got {pair_limit}")
    chosen_by_view = {name: [] for name in EXTENDED_STYLE_WEIGHTS}
    rejected_by_view = {name: [] for name in EXTENDED_STYLE_WEIGHTS}
    gaps_by_view = {name: [] for name in EXTENDED_STYLE_WEIGHTS}
    combined_chosen = []
    combined_rejected = []
    combined_gaps = []
    ids_seen = set()
    first_rows = []
    total = 0

    with path.open("r", encoding="utf-8") as fh:
        for line_no, line in enumerate(fh, start=1):
            if pair_limit is not None and total >= pair_limit:
                break
            if not line.strip():
                raise BenchmarkError(f"{path}: blank JSONL line at {line_no}")
            try:
                obj = json.loads(line)
            except json.JSONDecodeError as exc:
                raise BenchmarkError(f"{path}: invalid JSON at line {line_no}: {exc}") from exc
            meta = obj.get("meta")
            if not isinstance(meta, dict):
                raise BenchmarkError(f"{path}: line {line_no} missing object meta")
            required = [
                "chosen_id",
                "rejected_id",
                "chosen_extended_combined",
                "rejected_extended_combined",
                "chosen_extended_per_view",
                "rejected_extended_per_view",
            ]
            missing = [key for key in required if key not in meta]
            if missing:
                raise BenchmarkError(f"{path}: line {line_no} missing meta keys {missing}")
            chosen_views = meta["chosen_extended_per_view"]
            rejected_views = meta["rejected_extended_per_view"]
            if not isinstance(chosen_views, dict) or not isinstance(rejected_views, dict):
                raise BenchmarkError(f"{path}: line {line_no} per-view fields must be objects")
            for name in EXTENDED_STYLE_WEIGHTS:
                if name not in chosen_views or name not in rejected_views:
                    raise BenchmarkError(f"{path}: line {line_no} missing view {name}")
                cv = float(chosen_views[name])
                rv = float(rejected_views[name])
                if not math.isfinite(cv) or not math.isfinite(rv):
                    raise BenchmarkError(f"{path}: line {line_no} non-finite view {name}")
                chosen_by_view[name].append(cv)
                rejected_by_view[name].append(rv)
                gaps_by_view[name].append(cv - rv)
            cc = float(meta["chosen_extended_combined"])
            rc = float(meta["rejected_extended_combined"])
            if not math.isfinite(cc) or not math.isfinite(rc):
                raise BenchmarkError(f"{path}: line {line_no} non-finite combined reward")
            combined_chosen.append(cc)
            combined_rejected.append(rc)
            combined_gaps.append(cc - rc)
            ids_seen.add(str(meta["chosen_id"]))
            ids_seen.add(str(meta["rejected_id"]))
            if len(first_rows) < 3:
                first_rows.append({
                    "line": line_no,
                    "chosen_id": str(meta["chosen_id"]),
                    "rejected_id": str(meta["rejected_id"]),
                    "combined_gap": cc - rc,
                    "chosen_preview": str(obj.get("chosen", ""))[:120],
                    "rejected_preview": str(obj.get("rejected", ""))[:120],
                })
            total += 1

    if total == 0:
        raise BenchmarkError(f"CAMD pairs JSONL has zero usable rows: {path}")

    pair_effects = {}
    for name in EXTENDED_STYLE_WEIGHTS:
        pair_effects[name] = {
            "chosen": _distribution(chosen_by_view[name]),
            "rejected": _distribution(rejected_by_view[name]),
            "gap": _distribution(gaps_by_view[name]),
            "mean_gap": float(np.mean(np.asarray(gaps_by_view[name], dtype=np.float64))),
            "cohen_d": _cohen_d(chosen_by_view[name], rejected_by_view[name]),
        }
    return {
        "path": str(path),
        "sha256": _sha256_file(path),
        "rows_read": total,
        "pair_limit": pair_limit,
        "unique_memory_ids_referenced": len(ids_seen),
        "first_rows": first_rows,
        "combined": {
            "chosen": _distribution(combined_chosen),
            "rejected": _distribution(combined_rejected),
            "gap": _distribution(combined_gaps),
            "mean_gap": float(np.mean(np.asarray(combined_gaps, dtype=np.float64))),
        },
        "per_view_effect": pair_effects,
        "recommended_extended_weights_from_pair_effect": _recommended_weights(pair_effects),
    }


def _score_records(
    store_path: Path,
    centroids: dict[str, Any],
    max_records: int | None,
) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    if max_records is not None and max_records <= 0:
        raise BenchmarkError(f"max_records must be positive when set, got {max_records}")
    store = None
    try:
        store = ExtendedStore(store_path, readonly=True)
        store_stats = store.stats()
        counts = store_stats["counts_per_subdb"]
        expected_count = counts.get("meta", 0)
        if expected_count <= 0:
            raise BenchmarkError(f"LMDB store has zero meta records: {store_path}")
        mismatched = {
            name: count for name, count in counts.items()
            if count != expected_count
        }
        if mismatched:
            raise BenchmarkError(
                f"LMDB sub-DB counts do not match meta count {expected_count}: {mismatched}"
            )
        memory_ids = sorted(store.iter_memory_ids())
        if len(memory_ids) != expected_count:
            raise BenchmarkError(
                f"iter_memory_ids count {len(memory_ids)} != meta count {expected_count}"
            )
        if max_records is not None:
            memory_ids = memory_ids[:max_records]
        if not memory_ids:
            raise BenchmarkError("selected zero memory IDs")

        rows = []
        for idx, memory_id in enumerate(memory_ids, start=1):
            meta = store.get_meta(memory_id)
            bge_dense = store.get_dense(memory_id, embedder="bge_m3_dense")
            jina = store.get_dense(memory_id, embedder="jina_v3_classif")
            qwen = store.get_dense(memory_id, embedder="qwen3_instruct")
            colbert = store.get_colbert(memory_id)
            sparse = store.get_sparse(memory_id)
            _check_dense(f"{memory_id}:bge_m3_dense", bge_dense, (1024,))
            _check_dense(f"{memory_id}:jina_v3_classif", jina, (1024,))
            _check_dense(f"{memory_id}:qwen3_instruct", qwen, (1024,))
            if colbert.ndim != 2 or colbert.shape[1] != 1024:
                raise BenchmarkError(
                    f"{memory_id}:bge_m3_colbert expected shape [seq,1024], got {colbert.shape}"
                )
            if colbert.dtype != np.float32:
                raise BenchmarkError(f"{memory_id}:bge_m3_colbert expected float32, got {colbert.dtype}")
            if not np.isfinite(colbert).all():
                raise BenchmarkError(f"{memory_id}:bge_m3_colbert contains non-finite values")

            views = {
                "bge_m3_dense": _cosine(
                    bge_dense, centroids["bge_m3_dense"], f"{memory_id}:bge_m3_dense"
                ),
                "bge_m3_colbert": _cosine(
                    colbert.mean(axis=0), centroids["bge_m3_colbert"], f"{memory_id}:bge_m3_colbert"
                ),
                "bge_m3_sparse": _sparse_cosine(
                    sparse, centroids["bge_m3_sparse"], f"{memory_id}:bge_m3_sparse"
                ),
                "jina_v3_classif": _cosine(
                    jina, centroids["jina_v3_classif"], f"{memory_id}:jina_v3_classif"
                ),
                "qwen3_instruct": _cosine(
                    qwen, centroids["qwen3_instruct"], f"{memory_id}:qwen3_instruct"
                ),
            }
            combined = sum(EXTENDED_STYLE_WEIGHTS[name] * views[name] for name in EXTENDED_STYLE_WEIGHTS)
            if not math.isfinite(combined):
                raise BenchmarkError(f"{memory_id}: non-finite combined reward")
            row = {
                "rank_input_order": idx,
                "memory_id": memory_id,
                "content_sha256": meta.content_sha256,
                "content_length": int(meta.content_length),
                "computed_at": float(meta.computed_at),
                "bge_m3_version": meta.bge_m3_version,
                "jina_v3_version": meta.jina_v3_version,
                "qwen3_version": meta.qwen3_version,
                "reranker_version": meta.reranker_version,
                "task_prompt_qwen3": meta.task_prompt_qwen3,
                "reranker_score_vs_sonnets": float(meta.reranker_score_vs_sonnets),
                "bge_m3_dense_norm": float(np.linalg.norm(bge_dense)),
                "bge_m3_colbert_seq_len": int(colbert.shape[0]),
                "bge_m3_sparse_terms": int(len(sparse)),
                "jina_v3_norm": float(np.linalg.norm(jina)),
                "qwen3_norm": float(np.linalg.norm(qwen)),
                "extended_combined_reward": float(combined),
                **views,
            }
            rows.append(row)
        return rows, store_stats
    finally:
        if store is not None:
            store.close()


def _record_anomalies(rows: list[dict[str, Any]]) -> dict[str, list[dict[str, Any]]]:
    names = ["bge_m3_dense", "bge_m3_colbert", "bge_m3_sparse", "jina_v3_classif", "qwen3_instruct"]
    arr = np.asarray([[float(r[name]) for name in names] for r in rows], dtype=np.float64)
    z = (arr - arr.mean(axis=0)) / arr.std(axis=0, ddof=1)
    idx = {name: pos for pos, name in enumerate(names)}

    def emit(score: np.ndarray, label: str) -> list[dict[str, Any]]:
        order = np.argsort(-score)[:10]
        result = []
        for pos in order:
            row = rows[int(pos)]
            result.append({
                "memory_id": row["memory_id"],
                "content_sha256": row["content_sha256"],
                "content_length": row["content_length"],
                "score": float(score[int(pos)]),
                "extended_combined_reward": row["extended_combined_reward"],
                "views": {name: row[name] for name in names},
                "interpretation": label,
            })
        return result

    return {
        "instruction_high_sparse_low": emit(
            z[:, idx["qwen3_instruct"]] - z[:, idx["bge_m3_sparse"]],
            "Instruction-conditioned style is high while exact lexical overlap is low.",
        ),
        "sparse_high_classifier_low": emit(
            z[:, idx["bge_m3_sparse"]] - z[:, idx["jina_v3_classif"]],
            "Lexical Shakespeare markers are high while classification-style geometry is lower.",
        ),
        "dense_high_colbert_low": emit(
            z[:, idx["bge_m3_dense"]] - z[:, idx["bge_m3_colbert"]],
            "Global dense proximity is high while local token geometry is lower.",
        ),
        "colbert_high_dense_low": emit(
            z[:, idx["bge_m3_colbert"]] - z[:, idx["bge_m3_dense"]],
            "Local token geometry is high while global dense proximity is lower.",
        ),
    }


def build_report(args: argparse.Namespace) -> dict[str, Any]:
    store_path = args.store_path.expanduser()
    centroids_path = args.centroids_path.expanduser()
    pairs_path = args.pairs.expanduser()
    if not store_path.exists() or not store_path.is_dir():
        raise BenchmarkError(f"LMDB store directory does not exist: {store_path}")
    centroids = _load_centroids(centroids_path)
    logger.info("source_of_truth.lmdb=%s", store_path)
    logger.info("source_of_truth.centroids=%s", centroids_path)
    logger.info("source_of_truth.pairs=%s", pairs_path)
    rows, store_stats = _score_records(store_path, centroids, args.max_records)
    pair_stats = _load_pair_stats(pairs_path, args.pair_limit)

    independent_views = [
        "bge_m3_dense",
        "bge_m3_colbert",
        "bge_m3_sparse",
        "jina_v3_classif",
        "qwen3_instruct",
        "reranker_score_vs_sonnets",
    ]
    distributions = {
        name: _distribution([float(r[name]) for r in rows])
        for name in independent_views + [
            "extended_combined_reward",
            "content_length",
            "bge_m3_colbert_seq_len",
            "bge_m3_sparse_terms",
        ]
    }
    sorted_rows = sorted(rows, key=lambda r: -float(r["extended_combined_reward"]))
    correlations = _correlation_matrix(rows, independent_views)
    pca = _pca(rows, independent_views)

    strongest_effect = max(
        pair_stats["per_view_effect"].items(),
        key=lambda item: item[1]["cohen_d"],
    )
    strongest_corr = correlations["strongest_pairs"][0]
    weakest_corr = correlations["weakest_pairs"][0]
    report = {
        "schema_version": "1.0",
        "created_at_unix": time.time(),
        "source_of_truth": {
            "report_path": str(args.output.expanduser()),
            "lmdb_store_path": str(store_path),
            "centroids_path": str(centroids_path),
            "camd_pairs_path": str(pairs_path),
        },
        "input_provenance": {
            "lmdb_store_state": _path_state(store_path),
            "centroids_sha256": _sha256_file(centroids_path),
            "camd_pairs_sha256": pair_stats["sha256"],
            "constellation_id": centroids["constellation_id"],
            "constellation_members": centroids["n_members"],
            "extended_style_weights_current": EXTENDED_STYLE_WEIGHTS,
        },
        "store_stats": store_stats,
        "summary": {
            "records_analyzed": len(rows),
            "pairs_analyzed": pair_stats["rows_read"],
            "max_records": args.max_records,
            "pair_limit": args.pair_limit,
            "top_combined_record": sorted_rows[0]["memory_id"],
            "bottom_combined_record": sorted_rows[-1]["memory_id"],
            "strongest_pair_effect_view": {
                "view": strongest_effect[0],
                "cohen_d": strongest_effect[1]["cohen_d"],
                "mean_gap": strongest_effect[1]["mean_gap"],
            },
            "strongest_record_correlation": strongest_corr,
            "weakest_record_correlation": weakest_corr,
        },
        "view_meanings": VIEW_MEANINGS,
        "record_level": {
            "distributions": distributions,
            "correlations": correlations,
            "pca": pca,
            "top_by_extended_combined_reward": sorted_rows[:10],
            "bottom_by_extended_combined_reward": sorted_rows[-10:],
            "anomalies": _record_anomalies(rows),
        },
        "pair_level": pair_stats,
        "insights": [
            {
                "kind": "strongest_discriminator",
                "text": (
                    f"{strongest_effect[0]} has the largest chosen-vs-rejected "
                    f"effect in the CAMD pairs (Cohen's d={strongest_effect[1]['cohen_d']:.3f})."
                ),
            },
            {
                "kind": "strongest_record_correlation",
                "text": (
                    f"{strongest_corr['left']} and {strongest_corr['right']} are the most aligned "
                    f"record-level views (Pearson r={strongest_corr['pearson']:.3f})."
                ),
            },
            {
                "kind": "view_tension",
                "text": (
                    f"{weakest_corr['left']} and {weakest_corr['right']} are the least aligned "
                    f"record-level views (Pearson r={weakest_corr['pearson']:.3f}), so their "
                    "disagreements are useful anomaly-mining candidates."
                ),
            },
        ],
        "records": rows,
    }
    return report


def write_report_atomic(report: dict[str, Any], output: Path) -> None:
    output = output.expanduser()
    output.parent.mkdir(parents=True, exist_ok=True)
    partial = output.with_suffix(output.suffix + ".partial")
    try:
        with partial.open("w", encoding="utf-8") as fh:
            json.dump(report, fh, indent=2, sort_keys=True, default=_json_default)
            fh.write("\n")
            fh.flush()
            os.fsync(fh.fileno())
        os.replace(partial, output)
    except BaseException:
        try:
            partial.unlink(missing_ok=True)
        finally:
            raise


def verify_report_file(output: Path, expected_records: int, expected_pairs: int) -> dict[str, Any]:
    output = output.expanduser()
    _require_file(output, "benchmark report")
    with output.open("r", encoding="utf-8") as fh:
        loaded = json.load(fh)
    records = loaded.get("records")
    if not isinstance(records, list):
        raise BenchmarkError(f"report {output} missing records list")
    if len(records) != expected_records:
        raise BenchmarkError(
            f"report {output} records count {len(records)} != expected {expected_records}"
        )
    summary = loaded.get("summary") or {}
    if summary.get("records_analyzed") != expected_records:
        raise BenchmarkError("summary.records_analyzed does not match records list length")
    if summary.get("pairs_analyzed") != expected_pairs:
        raise BenchmarkError(
            f"summary.pairs_analyzed {summary.get('pairs_analyzed')} != expected {expected_pairs}"
        )
    if not loaded.get("record_level", {}).get("correlations", {}).get("matrix"):
        raise BenchmarkError("report missing record correlation matrix")
    return {
        "path": str(output),
        "sha256": _sha256_file(output),
        "records_read_back": len(records),
        "pairs_read_back": summary.get("pairs_analyzed"),
        "top_combined_record": summary.get("top_combined_record"),
        "bottom_combined_record": summary.get("bottom_combined_record"),
    }


def run_once(args: argparse.Namespace) -> dict[str, Any]:
    report = build_report(args)
    write_report_atomic(report, args.output)
    verification = verify_report_file(
        args.output,
        expected_records=report["summary"]["records_analyzed"],
        expected_pairs=report["summary"]["pairs_analyzed"],
    )
    return {
        "status": "ok",
        "source_of_truth": report["source_of_truth"],
        "summary": report["summary"],
        "report_verification": verification,
    }


def _print_json(label: str, data: Any) -> None:
    print(f"\n=== {label} ===")
    print(json.dumps(data, indent=2, sort_keys=True, default=_json_default))


def run_fsv(args: argparse.Namespace) -> int:
    output = args.output.expanduser()
    _print_json("SOURCE OF TRUTH", {
        "primary_report_json": str(output),
        "lmdb_store": str(args.store_path.expanduser()),
        "camd_pairs": str(args.pairs.expanduser()),
        "centroids": str(args.centroids_path.expanduser()),
    })

    before = {
        "report": _path_state(output),
        "lmdb": _path_state(args.store_path.expanduser()),
        "pairs": _path_state(args.pairs.expanduser()),
    }
    _print_json("HAPPY PATH BEFORE", before)
    result = run_once(args)
    after = {
        "report": _path_state(output),
        "verified": result["report_verification"],
    }
    _print_json("HAPPY PATH AFTER", after)
    _print_json("HAPPY PATH SUMMARY", result["summary"])

    with tempfile.TemporaryDirectory(prefix="contextgraph-shake-bench-fsv.") as tmpdir_s:
        tmpdir = Path(tmpdir_s)

        def run_expected_failure(label: str, mutate: dict[str, Any], expected_fragment: str) -> None:
            edge_args = argparse.Namespace(**vars(args))
            for key, value in mutate.items():
                setattr(edge_args, key, value)
            state_before = {
                "output": _path_state(edge_args.output.expanduser()),
                "lmdb_counts": result["summary"]["records_analyzed"],
            }
            _print_json(f"{label} BEFORE", state_before)
            try:
                run_once(edge_args)
            except Exception as exc:
                text = f"{type(exc).__name__}: {exc}"
                if expected_fragment not in text:
                    raise BenchmarkError(
                        f"{label}: failure did not contain {expected_fragment!r}: {text}"
                    ) from exc
                _print_json(f"{label} FAILURE", {
                    "expected_failure": True,
                    "error": text,
                })
            else:
                raise BenchmarkError(f"{label}: expected failure, command succeeded")
            state_after = {
                "output": _path_state(edge_args.output.expanduser()),
                "canonical_report_still": _path_state(output),
            }
            _print_json(f"{label} AFTER", state_after)

        empty_pairs = tmpdir / "empty_pairs.jsonl"
        empty_pairs.write_text("", encoding="utf-8")
        run_expected_failure(
            "EDGE 1 EMPTY PAIRS",
            {"pairs": empty_pairs, "output": tmpdir / "empty_report.json"},
            "CAMD pairs JSONL is empty",
        )

        invalid_pairs = tmpdir / "invalid_pairs.jsonl"
        invalid_pairs.write_text('{"prompt":"missing meta"}\n', encoding="utf-8")
        run_expected_failure(
            "EDGE 2 INVALID PAIR FORMAT",
            {"pairs": invalid_pairs, "output": tmpdir / "invalid_report.json"},
            "missing object meta",
        )

        boundary_output = tmpdir / "small_limit_report.json"
        boundary_args = argparse.Namespace(**vars(args))
        boundary_args.max_records = 2
        boundary_args.pair_limit = 2
        boundary_args.output = boundary_output
        _print_json("EDGE 3 SMALL LIMIT BEFORE", {
            "output": _path_state(boundary_output),
            "max_records": boundary_args.max_records,
            "pair_limit": boundary_args.pair_limit,
        })
        boundary_result = run_once(boundary_args)
        _print_json("EDGE 3 SMALL LIMIT AFTER", {
            "output": _path_state(boundary_output),
            "verified": boundary_result["report_verification"],
            "summary": boundary_result["summary"],
        })

    _print_json("FSV CLEANUP", {
        "temporary_directory_removed": not Path(tmpdir_s).exists(),
        "canonical_report": _path_state(output),
    })
    return 0


def parse_args(argv: list[str]) -> argparse.Namespace:
    ap = argparse.ArgumentParser(
        description=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    ap.add_argument("--store-path", type=Path, default=DEFAULT_STORE_PATH)
    ap.add_argument("--centroids-path", type=Path, default=DEFAULT_CENTROIDS_PATH)
    ap.add_argument("--pairs", type=Path, default=DEFAULT_PAIRS_PATH)
    ap.add_argument("--output", type=Path, default=DEFAULT_OUTPUT_PATH)
    ap.add_argument("--max-records", type=int, default=None)
    ap.add_argument("--pair-limit", type=int, default=None)
    ap.add_argument("--fsv", action="store_true", help="run happy path plus edge-case verification")
    ap.add_argument("--log-level", default="INFO")
    return ap.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(sys.argv[1:] if argv is None else argv)
    logging.basicConfig(
        level=getattr(logging, str(args.log_level).upper(), logging.INFO),
        format="%(asctime)s [%(name)s] %(levelname)s: %(message)s",
    )
    try:
        if args.fsv:
            return run_fsv(args)
        result = run_once(args)
        _print_json("BENCHMARK COMPLETE", result)
        return 0
    except Exception as exc:
        logger.error("benchmark failed: %s: %s", type(exc).__name__, exc)
        logger.error("%s", traceback.format_exc())
        return 1
    finally:
        gc.collect()


if __name__ == "__main__":
    raise SystemExit(main())
