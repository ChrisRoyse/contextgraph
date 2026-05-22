"""Materialize TASK-FP-002 seed failure-shape fingerprints from packed forwards."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import shutil
import sys
from collections import Counter, defaultdict
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

import numpy as np

from .forward_cache_lineage import guard_packed_record_lineage
from .smoke_intelligence_matrices import (
    SmokeMatrixError,
    dump_json,
    iter_jsonl,
    sha256_file,
    tree_state,
    utc_now,
    write_jsonl,
)


SCHEMA_VERSION = 1
TASK_ID = "TASK-FP-002"
ARTIFACT_KIND = "python_failure_fingerprint_seed_catalog"
FORMULA_VERSION = "failure_fingerprint_centroid_variance_v1"
DEFAULT_FORWARD_ROOT = Path("/zfs/archive/contextgraph/corpus/python-forward-cache-v1-aiwonder-prod-20260515-191116Z")
DEFAULT_CHUNKS_JSONL = Path("/zfs/archive/contextgraph/corpus/python-ast-chunks-v1/python_ast_chunks.jsonl")
DEFAULT_OUTPUT_ROOT = Path("/zfs/archive/contextgraph/corpus/python-failure-fingerprints-seed-v1")
DEFAULT_FSV_ROOT = Path("/zfs/archive/contextgraph/fsv/task-fp-002-seed-fingerprint-materializer-fsv")
LOCAL_FALLBACK_CHUNKS_JSONL = Path("/zfs/archive/contextgraph/corpus/python-ast-chunks-v1/python_ast_chunks.jsonl")
EXPECTED_EMBEDDERS = ("e1", "e6", "e7", "e8", "e10", "e12", "e13", "e14", "e2", "e3", "e4", "e9")
EXPECTED_MUTATION_CATEGORIES = (
    "known_good",
    "subtle_flip",
    "off_by_one",
    "swap_variable",
    "delete_test_call",
    "wrong_file",
    "over_engineer",
    "compile_error",
)
KNOWN_BAD_FAILURE_MODE = {
    "subtle_flip": "semantic_error",
    "off_by_one": "off_by_one",
    "swap_variable": "wrong_algorithm",
    "delete_test_call": "wrong_test_updated",
    "wrong_file": "other",
    "over_engineer": "api_misuse",
    "compile_error": "compile_error",
}
EPS = 1.0e-12


class FingerprintMaterializerError(SmokeMatrixError):
    pass


@dataclass(frozen=True)
class ChunkMeta:
    chunk_id: str
    task_instance_id: str
    repo: str
    mutation_category: str
    workspace_state: str
    source_sha256: str
    file_sha256: str
    relative_path: str
    chunk_kind: str
    node_kind: str
    task_id: str
    absolute_path: str
    dataschema: str

    def as_canonical_chunk_row(self) -> dict[str, Any]:
        return {
            "task_instance_id": self.task_instance_id,
            "workspace_state": self.workspace_state,
            "chunk_id": self.chunk_id,
            "source_sha256": self.source_sha256,
            "relative_path": self.relative_path,
            "absolute_path": self.absolute_path,
            "dataschema": self.dataschema,
        }


@dataclass(frozen=True)
class FingerprintSpec:
    spec_key: str
    kind_type: str
    repo: str | None
    mutation_category: str
    failure_mode: str | None
    oracle_outcome: str
    name: str

    def class_slug(self) -> str:
        if self.kind_type == "known_good":
            return f"known_good:{self.repo or 'global'}"
        exception = "no_exception"
        return f"known_bad:{self.repo}:{self.mutation_category}:{self.failure_mode}:{exception}"

    def kind_payload(self, task_count: int) -> dict[str, Any]:
        if self.kind_type == "known_good":
            return {
                "type": "known_good",
                "repo": self.repo,
                "gold_patch_count": task_count,
            }
        return {
            "type": "known_bad",
            "repo": self.repo,
            "mutation_category": self.mutation_category,
            "failure_mode": self.failure_mode,
            "exception_class": None,
        }


@dataclass
class RunningVectorStats:
    dim: int | None = None
    count: int = 0
    vector_sum: np.ndarray | None = None
    norm_sq_sum: float = 0.0

    def add(self, vector: np.ndarray) -> None:
        if vector.ndim != 1 or vector.size == 0:
            raise FingerprintMaterializerError("FINGERPRINT_VECTOR_EMPTY")
        if not np.isfinite(vector).all():
            raise FingerprintMaterializerError("FINGERPRINT_VECTOR_NONFINITE")
        if self.dim is None:
            self.dim = int(vector.size)
            self.vector_sum = np.zeros(self.dim, dtype=np.float64)
        if int(vector.size) != self.dim:
            raise FingerprintMaterializerError("FINGERPRINT_VECTOR_DIM_MISMATCH")
        assert self.vector_sum is not None
        self.vector_sum += vector.astype(np.float64, copy=False)
        self.norm_sq_sum += float(np.dot(vector, vector))
        self.count += 1

    def finalize(self) -> tuple[list[float], float]:
        if self.count == 0 or self.vector_sum is None or self.dim is None:
            raise FingerprintMaterializerError("FINGERPRINT_EMPTY_EMBEDDER_STATS")
        mean = self.vector_sum / float(self.count)
        mean_norm = float(np.linalg.norm(mean))
        if mean_norm <= EPS:
            raise FingerprintMaterializerError("FINGERPRINT_ZERO_CENTROID")
        centroid = mean / mean_norm
        variance = self.norm_sq_sum / float(self.count) - (2.0 * float(np.dot(centroid, mean))) + 1.0
        return [finite_float(v) for v in centroid.astype(float).tolist()], finite_float(max(0.0, variance))


@dataclass
class FingerprintAccumulator:
    spec: FingerprintSpec
    by_embedder: dict[str, RunningVectorStats] = field(default_factory=dict)
    reference_embedders: dict[str, set[str]] = field(default_factory=lambda: defaultdict(set))
    task_instance_ids: set[str] = field(default_factory=set)
    repos: set[str] = field(default_factory=set)

    def add(self, embedder_id: str, vector: np.ndarray, meta: ChunkMeta) -> None:
        self.by_embedder.setdefault(embedder_id, RunningVectorStats()).add(vector)
        self.reference_embedders[meta.chunk_id].add(embedder_id)
        self.task_instance_ids.add(meta.task_instance_id)
        self.repos.add(meta.repo)


def finite_float(value: float) -> float:
    out = float(value)
    if not math.isfinite(out):
        raise FingerprintMaterializerError("FINGERPRINT_NONFINITE_SCALAR")
    return out


def canonical_json(value: Any) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def sha256_text(value: Any) -> str:
    return "sha256:" + hashlib.sha256(canonical_json(value).encode("utf-8")).hexdigest()


def rust_fingerprint_id(source_corpus: str, class_slug: str) -> str:
    hasher = hashlib.sha256()
    hasher.update(b"MEJEPA_FAILURE_FINGERPRINT_V1")
    for part in (source_corpus, class_slug):
        encoded = part.encode("utf-8")
        hasher.update(len(encoded).to_bytes(8, "big"))
        hasher.update(encoded)
    return hasher.hexdigest()


def slug_text(value: str) -> str:
    return "".join(ch if ch.isalnum() else "_" for ch in value.lower()).strip("_")


def require(condition: bool, code: str, detail: str | None = None) -> None:
    if not condition:
        raise FingerprintMaterializerError(code if detail is None else f"{code}: {detail}")


def row_chunk_id(record: dict[str, Any]) -> str:
    key = record.get("key") or {}
    chunk_id = key.get("chunk_id") or (record.get("chunk") or {}).get("chunk_id")
    return str(chunk_id or "")


def row_embedder_id(path: Path, record: dict[str, Any]) -> str:
    key = record.get("key") or {}
    return str(key.get("embedder_id") or path.stem.replace(".index", ""))


def load_chunk_metadata(chunks_jsonl: Path) -> tuple[dict[str, ChunkMeta], dict[str, Any]]:
    if not chunks_jsonl.exists() and chunks_jsonl == DEFAULT_CHUNKS_JSONL and LOCAL_FALLBACK_CHUNKS_JSONL.exists():
        chunks_jsonl = LOCAL_FALLBACK_CHUNKS_JSONL
    require(chunks_jsonl.exists(), "FINGERPRINT_CHUNKS_JSONL_MISSING", str(chunks_jsonl))
    by_chunk_id: dict[str, ChunkMeta] = {}
    repos: Counter[str] = Counter()
    categories: Counter[str] = Counter()
    for _, row in iter_jsonl(chunks_jsonl):
        chunk_id = str(row.get("chunk_id") or "")
        mutation_category = str(row.get("mutation_category") or "")
        repo = str(row.get("repo") or "")
        require(chunk_id.startswith("sha256:"), "FINGERPRINT_CHUNK_ID_MISSING", chunk_id)
        require(mutation_category, "FINGERPRINT_MUTATION_CATEGORY_MISSING", chunk_id)
        require(repo, "FINGERPRINT_REPO_MISSING", chunk_id)
        require(chunk_id not in by_chunk_id, "FINGERPRINT_DUPLICATE_CHUNK_ID", chunk_id)
        meta = ChunkMeta(
            chunk_id=chunk_id,
            task_instance_id=str(row.get("task_instance_id") or ""),
            repo=repo,
            mutation_category=mutation_category,
            workspace_state=str(row.get("workspace_state") or ""),
            source_sha256=str(row.get("source_sha256") or ""),
            file_sha256=str(row.get("file_sha256") or ""),
            relative_path=str(row.get("relative_path") or ""),
            chunk_kind=str(row.get("chunk_kind") or ""),
            node_kind=str(row.get("node_kind") or ""),
            task_id=str(row.get("task_id") or row.get("task_instance_id") or ""),
            absolute_path=str(row.get("absolute_path") or ""),
            dataschema=str(row.get("dataschema") or ""),
        )
        by_chunk_id[chunk_id] = meta
        repos[repo] += 1
        categories[mutation_category] += 1
    require(bool(by_chunk_id), "FINGERPRINT_EMPTY_CHUNK_METADATA", str(chunks_jsonl))
    return by_chunk_id, {
        "path": str(chunks_jsonl),
        "sha256": sha256_file(chunks_jsonl),
        "chunk_count": len(by_chunk_id),
        "repo_count": len(repos),
        "mutation_category_count": len(categories),
        "repo_counts": dict(sorted(repos.items())),
        "mutation_category_counts": dict(sorted(categories.items())),
    }


def fingerprint_specs_for_meta(meta: ChunkMeta) -> list[FingerprintSpec]:
    if meta.mutation_category == "known_good":
        repo_slug = slug_text(meta.repo)
        return [
            FingerprintSpec("known_good:global", "known_good", None, "known_good", None, "pass", "known_good_global"),
            FingerprintSpec(
                f"known_good:{meta.repo}",
                "known_good",
                meta.repo,
                "known_good",
                None,
                "pass",
                f"known_good__{repo_slug}",
            ),
        ]
    failure_mode = KNOWN_BAD_FAILURE_MODE.get(meta.mutation_category, "other")
    repo_slug = slug_text(meta.repo)
    return [
        FingerprintSpec(
            f"known_bad:{meta.repo}:{meta.mutation_category}",
            "known_bad",
            meta.repo,
            meta.mutation_category,
            failure_mode,
            "fail",
            f"{meta.mutation_category}__{repo_slug}",
        )
    ]


def iter_packed_record_paths(forward_root: Path, expected_embedders: set[str], max_shards: int | None) -> list[Path]:
    require(forward_root.exists(), "FINGERPRINT_FORWARD_ROOT_MISSING", str(forward_root))
    shards = sorted((forward_root / "shards").glob("shard-*"))
    if max_shards is not None:
        shards = shards[:max_shards]
    paths: list[Path] = []
    for shard in shards:
        packed = shard / "packed_records"
        if not packed.is_dir():
            continue
        for path in sorted(packed.glob("*.jsonl")):
            if path.name.endswith(".index.jsonl"):
                continue
            if expected_embedders and path.stem not in expected_embedders:
                continue
            paths.append(path)
    require(bool(paths), "FINGERPRINT_PACKED_RECORDS_MISSING", str(forward_root))
    return paths


def vector_from_record(record: dict[str, Any]) -> np.ndarray:
    output = record.get("output") or {}
    vector = output.get("vector")
    if not isinstance(vector, list) or not vector:
        raise FingerprintMaterializerError("FINGERPRINT_VECTOR_MISSING")
    try:
        arr = np.asarray(vector, dtype=np.float64)
    except (TypeError, ValueError) as exc:
        raise FingerprintMaterializerError("FINGERPRINT_VECTOR_BAD_TYPE") from exc
    if arr.ndim != 1 or arr.size == 0:
        raise FingerprintMaterializerError("FINGERPRINT_VECTOR_EMPTY")
    if not np.isfinite(arr).all():
        raise FingerprintMaterializerError("FINGERPRINT_VECTOR_NONFINITE")
    return arr


def materialize_catalog(args: argparse.Namespace) -> dict[str, Any]:
    started = utc_now()
    expected_embedders = tuple(item.strip() for item in args.expected_embedders.split(",") if item.strip())
    expected_embedder_set = set(expected_embedders)
    require(bool(expected_embedders), "FINGERPRINT_EXPECTED_EMBEDDERS_EMPTY")
    if args.clean_output and args.output_root.exists():
        shutil.rmtree(args.output_root)
    args.output_root.mkdir(parents=True, exist_ok=True)
    args.fsv_root.mkdir(parents=True, exist_ok=True)

    before = {"output_root": tree_state(args.output_root), "fsv_root": tree_state(args.fsv_root)}
    metadata, metadata_summary = load_chunk_metadata(args.chunks_jsonl)
    paths = iter_packed_record_paths(args.forward_root, expected_embedder_set, args.max_shards)

    accumulators: dict[str, FingerprintAccumulator] = {}
    records_seen = 0
    joined_records = 0
    missing_metadata: Counter[str] = Counter()
    rows_by_embedder: Counter[str] = Counter()
    rows_by_category: Counter[str] = Counter()
    rows_by_spec: Counter[str] = Counter()
    raw_forward_cache_legacy_path_metadata = 0

    for path in paths:
        for line_no, record in iter_jsonl(path):
            records_seen += 1
            if args.progress_every_records > 0 and records_seen % args.progress_every_records == 0:
                print(
                    json.dumps(
                        {
                            "event": "fingerprint_materializer_progress",
                            "records_seen": records_seen,
                            "joined_records": joined_records,
                            "fingerprints_so_far": len(accumulators),
                            "path": str(path),
                            "created_at_utc": utc_now(),
                        },
                        sort_keys=True,
                    ),
                    file=sys.stderr,
                    flush=True,
                )
            embedder_id = row_embedder_id(path, record)
            if embedder_id not in expected_embedder_set:
                raise FingerprintMaterializerError(f"FINGERPRINT_UNEXPECTED_EMBEDDER: {embedder_id}")
            chunk_id = row_chunk_id(record)
            meta = metadata.get(chunk_id)
            if meta is None:
                missing_metadata[chunk_id or "<empty>"] += 1
                continue
            lineage = guard_packed_record_lineage(
                record,
                meta.as_canonical_chunk_row(),
                context=f"{path}:{line_no}",
            )
            raw_forward_cache_legacy_path_metadata += 1 if lineage["raw_packed_path_metadata_legacy"] else 0
            vector = vector_from_record(record)
            joined_records += 1
            rows_by_embedder[embedder_id] += 1
            rows_by_category[meta.mutation_category] += 1
            for spec in fingerprint_specs_for_meta(meta):
                acc = accumulators.setdefault(spec.spec_key, FingerprintAccumulator(spec))
                acc.add(embedder_id, vector, meta)
                rows_by_spec[spec.spec_key] += 1
            if args.max_records is not None and joined_records >= args.max_records:
                break
        if args.max_records is not None and joined_records >= args.max_records:
            break

    require(not missing_metadata, "FINGERPRINT_FORWARD_RECORD_METADATA_MISSING", str(missing_metadata.most_common(10)))

    source_corpus = args.source_corpus
    source_manifest_sha256 = None
    manifest_path = args.forward_root / "python_forward_cache_corpus_manifest.json"
    if manifest_path.exists():
        source_manifest_sha256 = sha256_file(manifest_path)

    fingerprint_rows: list[dict[str, Any]] = []
    reference_rows: list[dict[str, Any]] = []
    incomplete_fingerprints: list[dict[str, Any]] = []

    for spec_key, acc in sorted(accumulators.items()):
        centroid_by_embedder: dict[str, list[float]] = {}
        variance_by_embedder: dict[str, float] = {}
        embedder_observation_counts: dict[str, int] = {}
        for embedder_id in expected_embedders:
            stats = acc.by_embedder.get(embedder_id)
            if stats is None:
                continue
            centroid, variance = stats.finalize()
            centroid_by_embedder[embedder_id] = centroid
            variance_by_embedder[embedder_id] = variance
            embedder_observation_counts[embedder_id] = stats.count
        missing_embedders = sorted(expected_embedder_set - set(centroid_by_embedder))
        if missing_embedders:
            incomplete_fingerprints.append({"spec_key": spec_key, "missing_embedders": missing_embedders})
            continue
        reference_chunks = sorted(acc.reference_embedders)
        fingerprint_id = rust_fingerprint_id(source_corpus, acc.spec.class_slug())
        task_count = len(acc.task_instance_ids)
        row = {
            "schema_version": SCHEMA_VERSION,
            "artifact_kind": "failure_shape_fingerprint_seed",
            "task_id": TASK_ID,
            "formula_version": FORMULA_VERSION,
            "fingerprint_id": fingerprint_id,
            "kind": acc.spec.kind_payload(task_count),
            "name": acc.spec.name,
            "source_corpus": source_corpus,
            "source_manifest_sha256": source_manifest_sha256,
            "centroid_by_embedder": centroid_by_embedder,
            "variance_by_embedder": variance_by_embedder,
            "tau_by_embedder": {embedder_id: 0.0 for embedder_id in expected_embedders},
            "tau_state": "uncalibrated_placeholder_task_fp_003",
            "pairwise_cosine": [],
            "pairwise_mutual_information": [],
            "reference_chunks": reference_chunks,
            "n_references": len(reference_chunks),
            "oracle_outcome": acc.spec.oracle_outcome,
            "is_canonical": True,
            "confidence": {
                "classification_accuracy": None,
                "classification_precision": None,
                "unknown_recall": None,
                "calibration_observations": 0,
                "calibration_state": "uncalibrated",
            },
            "materializer": {
                "embedder_observation_counts": embedder_observation_counts,
                "task_instance_count": task_count,
                "repo_count": len(acc.repos),
            },
            "record_sha256": None,
        }
        row["record_sha256"] = sha256_text({k: v for k, v in row.items() if k != "record_sha256"})
        fingerprint_rows.append(row)
        for chunk_id, embedders in sorted(acc.reference_embedders.items()):
            meta = metadata[chunk_id]
            ref_payload = {
                "schema_version": SCHEMA_VERSION,
                "artifact_kind": "failure_fingerprint_reference",
                "task_id": TASK_ID,
                "fingerprint_id": fingerprint_id,
                "chunk_id": chunk_id,
                "repo": meta.repo,
                "mutation_category": meta.mutation_category,
                "task_instance_id": meta.task_instance_id,
                "workspace_state": meta.workspace_state,
                "source_sha256": meta.source_sha256,
            }
            reference_rows.append(
                {
                    **ref_payload,
                    "reference_id": sha256_text(ref_payload),
                    "embedder_ids": sorted(embedders),
                    "oracle_outcome": acc.spec.oracle_outcome,
                    "source_manifest_sha256": metadata_summary["sha256"],
                }
            )

    require(not incomplete_fingerprints, "FINGERPRINT_INCOMPLETE_EMBEDDER_COVERAGE", str(incomplete_fingerprints[:10]))
    if args.allow_partial:
        require(bool(fingerprint_rows), "FINGERPRINT_PARTIAL_RUN_EMPTY")
    else:
        require(len(fingerprint_rows) == args.expected_fingerprints, "FINGERPRINT_SEED_COUNT_MISMATCH", f"expected {args.expected_fingerprints} got {len(fingerprint_rows)}")
    known_good_count = sum(1 for row in fingerprint_rows if row["kind"]["type"] == "known_good")
    known_bad_count = sum(1 for row in fingerprint_rows if row["kind"]["type"] == "known_bad")
    if not args.allow_partial:
        require(known_good_count == args.expected_known_good, "FINGERPRINT_KNOWN_GOOD_COUNT_MISMATCH", str(known_good_count))
        require(known_bad_count == args.expected_known_bad, "FINGERPRINT_KNOWN_BAD_COUNT_MISMATCH", str(known_bad_count))

    write_jsonl(args.output_root / "failure_fingerprints.jsonl", fingerprint_rows)
    write_jsonl(args.output_root / "fingerprint_references.jsonl", reference_rows)
    report = {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "artifact_kind": ARTIFACT_KIND,
        "formula_version": FORMULA_VERSION,
        "started_at_utc": started,
        "created_at_utc": utc_now(),
        "source_of_truth": {
            "output_root": str(args.output_root),
            "fingerprints": str(args.output_root / "failure_fingerprints.jsonl"),
            "references": str(args.output_root / "fingerprint_references.jsonl"),
            "report": str(args.output_root / "fingerprint_materialization_report.json"),
        },
        "inputs": {
            "forward_root": str(args.forward_root),
            "chunks_jsonl": metadata_summary,
            "source_corpus": source_corpus,
            "expected_embedders": list(expected_embedders),
            "max_shards": args.max_shards,
            "max_records": args.max_records,
            "allow_partial": args.allow_partial,
        },
        "summary": {
            "records_seen": records_seen,
            "joined_records": joined_records,
            "fingerprint_count": len(fingerprint_rows),
            "known_good_count": known_good_count,
            "known_bad_count": known_bad_count,
            "reference_row_count": len(reference_rows),
            "rows_by_embedder": dict(sorted(rows_by_embedder.items())),
            "rows_by_mutation_category": dict(sorted(rows_by_category.items())),
            "rows_by_spec": dict(sorted(rows_by_spec.items())),
            "raw_forward_cache_legacy_path_metadata": raw_forward_cache_legacy_path_metadata,
            "forward_cache_lineage_policy": "packed record chunk.absolute_path/dataschema is legacy provenance only; active lineage comes from canonical AST rows",
        },
    }
    dump_json(args.output_root / "fingerprint_materialization_report.json", report)

    readback_fingerprints = [row for _, row in iter_jsonl(args.output_root / "failure_fingerprints.jsonl")]
    readback_references = [row for _, row in iter_jsonl(args.output_root / "fingerprint_references.jsonl")]
    readback_report = json.loads((args.output_root / "fingerprint_materialization_report.json").read_text(encoding="utf-8"))
    after = {"output_root": tree_state(args.output_root), "fsv_root": tree_state(args.fsv_root)}
    readback_passes = (
        len(readback_fingerprints) == len(fingerprint_rows)
        and len(readback_references) == len(reference_rows)
        and readback_report.get("summary", {}).get("fingerprint_count") == len(fingerprint_rows)
        and all(set(row["centroid_by_embedder"]) == expected_embedder_set for row in readback_fingerprints)
        and all(row["n_references"] == len(row["reference_chunks"]) for row in readback_fingerprints)
    )
    fsv = {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "artifact_kind": f"{ARTIFACT_KIND}_fsv",
        "formula_version": FORMULA_VERSION,
        "started_at_utc": started,
        "created_at_utc": utc_now(),
        "passes": readback_passes,
        "all_passed": readback_passes,
        "trigger": {
            "command": "python3 scripts/swebench_lite_ops.py python-failure-fingerprint-materialize",
            "source_corpus": source_corpus,
        },
        "source_of_truth": {**report["source_of_truth"], "fsv": str(args.fsv_root / "failure_fingerprint_materializer_fsv.json")},
        "before_state": before,
        "after_state": after,
        "expected": {
            "fingerprints": args.expected_fingerprints,
            "known_good": args.expected_known_good,
            "known_bad": args.expected_known_bad,
            "embedder_count_per_fingerprint": len(expected_embedders),
            "allow_partial": args.allow_partial,
        },
        "actual": report["summary"],
        "readback": {
            "fingerprints_sha256": sha256_file(args.output_root / "failure_fingerprints.jsonl"),
            "references_sha256": sha256_file(args.output_root / "fingerprint_references.jsonl"),
            "report_sha256": sha256_file(args.output_root / "fingerprint_materialization_report.json"),
            "first_fingerprint": readback_fingerprints[0] if readback_fingerprints else None,
            "artifact_state": tree_state(args.output_root),
        },
        "boundary_cases": edge_cases(args.fsv_root),
    }
    fsv["passes"] = bool(fsv["passes"] and all(case["pass"] for case in fsv["boundary_cases"]))
    fsv["all_passed"] = fsv["passes"]
    dump_json(args.fsv_root / "failure_fingerprint_materializer_fsv.json", fsv)
    final = json.loads((args.fsv_root / "failure_fingerprint_materializer_fsv.json").read_text(encoding="utf-8"))
    require(final.get("all_passed") is True, "FINGERPRINT_MATERIALIZER_FSV_FAILED", str(args.fsv_root / "failure_fingerprint_materializer_fsv.json"))
    return final


def edge_cases(fsv_root: Path) -> list[dict[str, Any]]:
    cases: list[dict[str, Any]] = []
    for name, vector, expected in [
        ("empty_vector", [], "FINGERPRINT_VECTOR_MISSING"),
        ("nonfinite_vector", [1.0, float("nan")], "FINGERPRINT_VECTOR_NONFINITE"),
    ]:
        before = {"vector": str(vector)}
        try:
            vector_from_record({"output": {"vector": vector}})
            actual = None
        except FingerprintMaterializerError as exc:
            actual = str(exc).split(":", 1)[0]
        cases.append(
            {
                "case": name,
                "before": before,
                "action": "parse vector from packed forward record",
                "expected": expected,
                "actual": actual,
                "pass": actual == expected,
            }
        )
    try:
        acc = RunningVectorStats()
        acc.add(np.asarray([1.0, 0.0], dtype=np.float64))
        acc.add(np.asarray([1.0, 0.0, 0.0], dtype=np.float64))
        actual = None
    except FingerprintMaterializerError as exc:
        actual = str(exc).split(":", 1)[0]
    cases.append(
        {
            "case": "dimension_mismatch",
            "before": {"first_dim": 2, "second_dim": 3},
            "action": "add two vectors to one fingerprint/embedder accumulator",
            "expected": "FINGERPRINT_VECTOR_DIM_MISMATCH",
            "actual": actual,
            "pass": actual == "FINGERPRINT_VECTOR_DIM_MISMATCH",
        }
    )
    missing = fsv_root / "missing-fixture.jsonl"
    missing.parent.mkdir(parents=True, exist_ok=True)
    missing.write_text("", encoding="utf-8")
    try:
        load_chunk_metadata(missing)
        actual = None
    except FingerprintMaterializerError as exc:
        actual = str(exc).split(":", 1)[0]
    cases.append(
        {
            "case": "empty_metadata",
            "before": {"path": str(missing)},
            "action": "load chunk metadata",
            "expected": "FINGERPRINT_EMPTY_CHUNK_METADATA",
            "actual": actual,
            "pass": actual == "FINGERPRINT_EMPTY_CHUNK_METADATA",
        }
    )
    return cases


def build_fixture(root: Path, embedders: tuple[str, ...]) -> tuple[Path, Path]:
    if root.exists():
        shutil.rmtree(root)
    chunks_path = root / "chunks.jsonl"
    forward_root = root / "forward"
    packed_root = forward_root / "shards" / "shard-000000-offset-000000000" / "packed_records"
    packed_root.mkdir(parents=True, exist_ok=True)
    rows = [
        ("sha256:" + "a" * 64, "django/django", "known_good", "django__django-1", [1.0, 0.0, 0.0]),
        ("sha256:" + "b" * 64, "sympy/sympy", "known_good", "sympy__sympy-1", [0.0, 1.0, 0.0]),
        ("sha256:" + "c" * 64, "django/django", "subtle_flip", "django__django-1", [0.0, 0.0, 1.0]),
        ("sha256:" + "d" * 64, "sympy/sympy", "subtle_flip", "sympy__sympy-1", [1.0, 1.0, 0.0]),
    ]
    chunk_lines = []
    for idx, (chunk_id, repo, category, task_instance, _) in enumerate(rows):
        chunk_lines.append(
            {
                "schema_version": SCHEMA_VERSION,
                "chunk_id": chunk_id,
                "task_instance_id": task_instance,
                "task_id": task_instance,
                "repo": repo,
                "mutation_category": category,
                "workspace_state": "base" if category == "known_good" else "mutated",
                "source_sha256": "sha256:" + f"{idx + 1:064x}"[-64:],
                "file_sha256": "sha256:" + f"{idx + 101:064x}"[-64:],
                "relative_path": f"pkg/file_{idx}.py",
                "chunk_kind": "function",
                "node_kind": "FunctionDef",
            }
        )
    write_jsonl(chunks_path, chunk_lines)
    for eidx, embedder_id in enumerate(embedders):
        record_rows = []
        for chunk_id, repo, category, task_instance, base_vec in rows:
            vector = [finite_float(v + (0.01 * eidx)) for v in base_vec]
            record_rows.append(
                {
                    "schema_version": SCHEMA_VERSION,
                    "cache_key_sha256": sha256_text({"chunk_id": chunk_id, "embedder_id": embedder_id}),
                    "instrument_id": f"fixture_{embedder_id}",
                    "instrument_kind": "fixture",
                    "key": {
                        "chunk_id": chunk_id,
                        "source_sha256": next(row["source_sha256"] for row in chunk_lines if row["chunk_id"] == chunk_id),
                        "file_sha256": next(row["file_sha256"] for row in chunk_lines if row["chunk_id"] == chunk_id),
                        "embedder_id": embedder_id,
                    },
                    "chunk": {
                        "task_instance_id": task_instance,
                        "repo": repo,
                        "mutation_category": category,
                        "workspace_state": "base" if category == "known_good" else "mutated",
                        "relative_path": "pkg/file.py",
                    },
                    "output": {
                        "vector": vector,
                        "vector_len": len(vector),
                        "vector_sha256": sha256_text(vector),
                    },
                    "source_readback_verified": True,
                }
            )
        write_jsonl(packed_root / f"{embedder_id}.jsonl", record_rows)
    dump_json(
        forward_root / "python_forward_cache_corpus_manifest.json",
        {
            "schema_version": SCHEMA_VERSION,
            "artifact_kind": "fixture_forward_cache_manifest",
            "status": "passed",
            "passes": True,
            "summary": {"shard_count": 1, "cache_record_count": len(rows) * len(embedders), "active_embedder_count": len(embedders)},
        },
    )
    return chunks_path, forward_root


def command_python_failure_fingerprint_materializer(args: argparse.Namespace) -> int:
    if args.fixture_mode:
        embedders = tuple(item.strip() for item in args.expected_embedders.split(",") if item.strip())
        chunks_path, forward_root = build_fixture(args.fsv_root / "fixture-input", embedders)
        args.chunks_jsonl = chunks_path
        args.forward_root = forward_root
        args.output_root = args.fsv_root / "fixture-output"
        args.source_corpus = "task-fp-002-fixture"
        args.expected_fingerprints = 5
        args.expected_known_good = 3
        args.expected_known_bad = 2
        args.clean_output = True
    try:
        final = materialize_catalog(args)
    except SmokeMatrixError as exc:
        print(f"PYTHON_FAILURE_FINGERPRINT_MATERIALIZER_FAILED: {exc}", file=sys.stderr)
        return 1
    print(json.dumps(final, sort_keys=True, indent=2))
    return 0


def add_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--forward-root", type=Path, default=DEFAULT_FORWARD_ROOT)
    parser.add_argument("--chunks-jsonl", type=Path, default=DEFAULT_CHUNKS_JSONL)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--fsv-root", type=Path, default=DEFAULT_FSV_ROOT)
    parser.add_argument("--source-corpus", default="aiwonder-prod-20260515-191116Z")
    parser.add_argument("--expected-embedders", default=",".join(EXPECTED_EMBEDDERS))
    parser.add_argument("--expected-fingerprints", type=int, default=97)
    parser.add_argument("--expected-known-good", type=int, default=13)
    parser.add_argument("--expected-known-bad", type=int, default=84)
    parser.add_argument("--max-shards", type=int, default=None)
    parser.add_argument("--max-records", type=int, default=None)
    parser.add_argument("--progress-every-records", type=int, default=100_000)
    parser.add_argument("--allow-partial", action="store_true", help="Permit bounded smoke runs with fewer than the full 97 seed fingerprints.")
    parser.add_argument("--clean-output", action="store_true")
    parser.add_argument(
        "--fixture-mode",
        action="store_true",
        help="Build and verify a deterministic tiny fixture instead of reading the production substrate.",
    )
