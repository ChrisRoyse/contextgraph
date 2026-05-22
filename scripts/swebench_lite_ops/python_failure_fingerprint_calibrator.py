"""Calibrate TASK-FP-003 per-fingerprint G-tau thresholds from seed catalog."""

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

from .python_failure_fingerprint_materializer import (
    DEFAULT_FORWARD_ROOT,
    EPS,
    EXPECTED_EMBEDDERS,
    FingerprintMaterializerError,
    build_fixture,
    canonical_json,
    finite_float,
    iter_packed_record_paths,
    materialize_catalog,
    row_chunk_id,
    row_embedder_id,
    sha256_text,
    vector_from_record,
)
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
TASK_ID = "TASK-FP-003"
ARTIFACT_KIND = "python_failure_fingerprint_tau_calibration"
FORMULA_VERSION = "same_session_band_cosine_nearest_rank_p10_v1"
DEFAULT_SEED_ROOT = Path("/zfs/archive/contextgraph/corpus/python-failure-fingerprints-seed-v1")
DEFAULT_OUTPUT_ROOT = Path("/zfs/archive/contextgraph/corpus/python-failure-fingerprints-calibrated-v1")
DEFAULT_READBACK_ROOT = Path("/zfs/archive/contextgraph/readback/task-fp-003-fingerprint-tau-calibration-readback")
DEFAULT_SOURCE_CORPUS = "aiwonder-prod-20260515-191116Z"


class FingerprintCalibrationError(SmokeMatrixError):
    pass


def require(condition: bool, code: str, detail: str | None = None) -> None:
    if not condition:
        raise FingerprintCalibrationError(code if detail is None else f"{code}: {detail}")


def load_seed_catalog(seed_root: Path) -> tuple[list[dict[str, Any]], list[dict[str, Any]], dict[str, Any]]:
    fingerprints_path = seed_root / "failure_fingerprints.jsonl"
    references_path = seed_root / "fingerprint_references.jsonl"
    report_path = seed_root / "fingerprint_materialization_report.json"
    require(fingerprints_path.exists(), "FINGERPRINT_CALIBRATION_SEED_FINGERPRINTS_MISSING", str(fingerprints_path))
    require(references_path.exists(), "FINGERPRINT_CALIBRATION_REFERENCES_MISSING", str(references_path))
    require(report_path.exists(), "FINGERPRINT_CALIBRATION_REPORT_MISSING", str(report_path))
    fingerprints = [row for _, row in iter_jsonl(fingerprints_path)]
    references = [row for _, row in iter_jsonl(references_path)]
    report = json.loads(report_path.read_text(encoding="utf-8"))
    require(bool(fingerprints), "FINGERPRINT_CALIBRATION_EMPTY_SEED")
    require(bool(references), "FINGERPRINT_CALIBRATION_EMPTY_REFERENCES")
    ids = [str(row.get("fingerprint_id") or "") for row in fingerprints]
    require(all(ids), "FINGERPRINT_CALIBRATION_EMPTY_FINGERPRINT_ID")
    require(len(set(ids)) == len(ids), "FINGERPRINT_CALIBRATION_DUPLICATE_FINGERPRINT_ID")
    known_good = sum(1 for row in fingerprints if (row.get("kind") or {}).get("type") == "known_good")
    known_bad = sum(1 for row in fingerprints if (row.get("kind") or {}).get("type") == "known_bad")
    summary = {
        "fingerprint_count": len(fingerprints),
        "known_good_count": known_good,
        "known_bad_count": known_bad,
        "reference_row_count": len(references),
        "fingerprints_sha256": sha256_file(fingerprints_path),
        "references_sha256": sha256_file(references_path),
        "report_sha256": sha256_file(report_path),
        "seed_report_summary": report.get("summary", {}),
    }
    return fingerprints, references, summary


def validate_seed_fingerprints(
    fingerprints: list[dict[str, Any]],
    expected_embedders: tuple[str, ...],
    expected_fingerprints: int,
    expected_known_good: int,
    expected_known_bad: int,
    allow_partial: bool,
) -> None:
    expected_set = set(expected_embedders)
    if not allow_partial:
        require(
            len(fingerprints) == expected_fingerprints,
            "FINGERPRINT_CALIBRATION_SEED_COUNT_MISMATCH",
            f"expected {expected_fingerprints} got {len(fingerprints)}",
        )
        known_good = sum(1 for row in fingerprints if (row.get("kind") or {}).get("type") == "known_good")
        known_bad = sum(1 for row in fingerprints if (row.get("kind") or {}).get("type") == "known_bad")
        require(
            known_good == expected_known_good,
            "FINGERPRINT_CALIBRATION_KNOWN_GOOD_COUNT_MISMATCH",
            str(known_good),
        )
        require(
            known_bad == expected_known_bad,
            "FINGERPRINT_CALIBRATION_KNOWN_BAD_COUNT_MISMATCH",
            str(known_bad),
        )
    for row in fingerprints:
        fid = str(row.get("fingerprint_id") or "")
        centroids = row.get("centroid_by_embedder") or {}
        variances = row.get("variance_by_embedder") or {}
        taus = row.get("tau_by_embedder") or {}
        require(set(centroids) == expected_set, "FINGERPRINT_CALIBRATION_CENTROID_EMBEDDER_MISMATCH", fid)
        require(set(variances) == expected_set, "FINGERPRINT_CALIBRATION_VARIANCE_EMBEDDER_MISMATCH", fid)
        require(set(taus) == expected_set, "FINGERPRINT_CALIBRATION_TAU_EMBEDDER_MISMATCH", fid)
        for embedder_id, centroid in centroids.items():
            require(isinstance(centroid, list) and centroid, "FINGERPRINT_CALIBRATION_EMPTY_CENTROID", f"{fid}:{embedder_id}")
            arr = np.asarray(centroid, dtype=np.float64)
            require(arr.ndim == 1 and arr.size > 0, "FINGERPRINT_CALIBRATION_BAD_CENTROID", f"{fid}:{embedder_id}")
            require(bool(np.isfinite(arr).all()), "FINGERPRINT_CALIBRATION_NONFINITE_CENTROID", f"{fid}:{embedder_id}")
            norm = float(np.linalg.norm(arr))
            require(norm > EPS, "FINGERPRINT_CALIBRATION_ZERO_CENTROID", f"{fid}:{embedder_id}")


def build_reference_indexes(
    references: list[dict[str, Any]],
    fingerprint_ids: set[str],
    expected_embedders: tuple[str, ...],
) -> tuple[dict[str, dict[str, list[str]]], dict[str, int], dict[str, dict[str, int]]]:
    expected_set = set(expected_embedders)
    chunk_to_fingerprints: dict[str, dict[str, set[str]]] = defaultdict(lambda: defaultdict(set))
    reference_counts: Counter[str] = Counter()
    reference_counts_by_embedder: dict[str, Counter[str]] = defaultdict(Counter)
    for row in references:
        fid = str(row.get("fingerprint_id") or "")
        chunk_id = str(row.get("chunk_id") or "")
        embedder_ids = set(row.get("embedder_ids") or [])
        require(fid in fingerprint_ids, "FINGERPRINT_CALIBRATION_UNKNOWN_REFERENCE_FINGERPRINT", fid)
        require(chunk_id.startswith("sha256:"), "FINGERPRINT_CALIBRATION_BAD_REFERENCE_CHUNK", chunk_id)
        require(bool(embedder_ids), "FINGERPRINT_CALIBRATION_REFERENCE_EMBEDDERS_EMPTY", f"{fid}:{chunk_id}")
        require(
            embedder_ids.issubset(expected_set),
            "FINGERPRINT_CALIBRATION_REFERENCE_EMBEDDER_MISMATCH",
            f"{fid}:{chunk_id}",
        )
        seen_for_chunk = any(fid in ids for ids in chunk_to_fingerprints[chunk_id].values())
        if not seen_for_chunk:
            reference_counts[fid] += 1
        for embedder_id in embedder_ids:
            chunk_to_fingerprints[chunk_id][embedder_id].add(fid)
            reference_counts_by_embedder[fid][embedder_id] += 1
    missing = sorted(fingerprint_ids - set(reference_counts))
    require(not missing, "FINGERPRINT_CALIBRATION_FINGERPRINT_WITHOUT_REFERENCES", str(missing[:10]))
    chunk_index = {
        chunk_id: {embedder_id: sorted(ids) for embedder_id, ids in by_embedder.items()}
        for chunk_id, by_embedder in chunk_to_fingerprints.items()
    }
    embedder_counts = {
        fid: {embedder_id: int(counts.get(embedder_id, 0)) for embedder_id in expected_embedders}
        for fid, counts in reference_counts_by_embedder.items()
    }
    return chunk_index, dict(reference_counts), embedder_counts


def same_session_tau(values: list[float], percentile: float) -> float:
    require(0.0 < percentile <= 1.0, "FINGERPRINT_CALIBRATION_BAD_PERCENTILE", str(percentile))
    require(bool(values), "FINGERPRINT_CALIBRATION_EMPTY_SAMPLE")
    ordered = sorted(finite_float(value) for value in values)
    rank = int(math.ceil(percentile * len(ordered)))
    index = max(0, min(len(ordered) - 1, rank - 1))
    return finite_float(ordered[index])


def cosine_to_centroid(vector: np.ndarray, centroid: np.ndarray) -> float:
    require(vector.ndim == 1 and centroid.ndim == 1, "FINGERPRINT_CALIBRATION_VECTOR_RANK_MISMATCH")
    require(vector.size == centroid.size, "FINGERPRINT_CALIBRATION_VECTOR_DIM_MISMATCH", f"{vector.size}!={centroid.size}")
    norm = float(np.linalg.norm(vector))
    require(norm > EPS, "FINGERPRINT_CALIBRATION_ZERO_VECTOR")
    value = float(np.dot(vector, centroid) / norm)
    return finite_float(max(-1.0, min(1.0, value)))


def stable_record_sha(row: dict[str, Any]) -> str:
    return "sha256:" + hashlib.sha256(
        canonical_json({k: v for k, v in row.items() if k != "record_sha256"}).encode("utf-8")
    ).hexdigest()


def calibrate_catalog(args: argparse.Namespace) -> dict[str, Any]:
    started = utc_now()
    expected_embedders = tuple(item.strip() for item in args.expected_embedders.split(",") if item.strip())
    expected_set = set(expected_embedders)
    require(bool(expected_embedders), "FINGERPRINT_CALIBRATION_EXPECTED_EMBEDDERS_EMPTY")
    require(0.0 < args.same_session_band_percentile <= 1.0, "FINGERPRINT_CALIBRATION_BAD_PERCENTILE")

    if args.clean_output and args.output_root.exists():
        shutil.rmtree(args.output_root)
    args.output_root.mkdir(parents=True, exist_ok=True)
    args.readback_root.mkdir(parents=True, exist_ok=True)
    before = {"output_root": tree_state(args.output_root), "readback_root": tree_state(args.readback_root)}

    fingerprints, references, seed_summary = load_seed_catalog(args.seed_root)
    validate_seed_fingerprints(
        fingerprints,
        expected_embedders,
        args.expected_fingerprints,
        args.expected_known_good,
        args.expected_known_bad,
        args.allow_partial,
    )
    fingerprint_by_id = {str(row["fingerprint_id"]): row for row in fingerprints}
    chunk_to_fingerprints, reference_counts, reference_counts_by_embedder = build_reference_indexes(
        references,
        set(fingerprint_by_id),
        expected_embedders,
    )
    centroid_arrays: dict[str, dict[str, np.ndarray]] = {}
    for fid, row in fingerprint_by_id.items():
        centroid_arrays[fid] = {
            embedder_id: np.asarray(row["centroid_by_embedder"][embedder_id], dtype=np.float64)
            for embedder_id in expected_embedders
        }

    similarity_samples: dict[str, dict[str, list[float]]] = {
        fid: {embedder_id: [] for embedder_id in expected_embedders}
        for fid in fingerprint_by_id
    }
    records_seen = 0
    joined_records = 0
    calibrated_vector_observations = 0
    rows_by_embedder: Counter[str] = Counter()
    rows_by_fingerprint: Counter[str] = Counter()
    missing_reference_vectors: Counter[str] = Counter()

    paths = iter_packed_record_paths(args.forward_root, expected_set, args.max_shards)
    for path in paths:
        for _, record in iter_jsonl(path):
            records_seen += 1
            if args.progress_every_records > 0 and records_seen % args.progress_every_records == 0:
                print(
                    json.dumps(
                        {
                            "event": "fingerprint_tau_calibration_progress",
                            "records_seen": records_seen,
                            "joined_records": joined_records,
                            "calibrated_vector_observations": calibrated_vector_observations,
                            "path": str(path),
                            "created_at_utc": utc_now(),
                        },
                        sort_keys=True,
                    ),
                    file=sys.stderr,
                    flush=True,
                )
            embedder_id = row_embedder_id(path, record)
            if embedder_id not in expected_set:
                raise FingerprintCalibrationError(f"FINGERPRINT_CALIBRATION_UNEXPECTED_EMBEDDER: {embedder_id}")
            chunk_id = row_chunk_id(record)
            fingerprint_ids = (chunk_to_fingerprints.get(chunk_id) or {}).get(embedder_id)
            if not fingerprint_ids:
                continue
            vector = vector_from_record(record)
            joined_records += 1
            rows_by_embedder[embedder_id] += 1
            for fid in fingerprint_ids:
                similarity = cosine_to_centroid(vector, centroid_arrays[fid][embedder_id])
                similarity_samples[fid][embedder_id].append(similarity)
                rows_by_fingerprint[fid] += 1
                calibrated_vector_observations += 1
            if args.max_records is not None and joined_records >= args.max_records:
                break
        if args.max_records is not None and joined_records >= args.max_records:
            break

    calibrated_rows: list[dict[str, Any]] = []
    calibration_rows: list[dict[str, Any]] = []
    tau_values: list[float] = []
    calibrated_at_utc = utc_now()
    calibrated_at_unix_ms = int(__import__("time").time() * 1000)
    missing_samples: list[dict[str, Any]] = []

    for fid, row in sorted(fingerprint_by_id.items()):
        if args.allow_partial and all(not similarity_samples[fid][embedder_id] for embedder_id in expected_embedders):
            continue
        tau_by_embedder: dict[str, float] = {}
        sample_count_by_embedder: dict[str, int] = {}
        for embedder_id in expected_embedders:
            samples = similarity_samples[fid][embedder_id]
            if not samples:
                missing_samples.append({"fingerprint_id": fid, "embedder_id": embedder_id})
                continue
            tau = same_session_tau(samples, args.same_session_band_percentile)
            tau_by_embedder[embedder_id] = tau
            tau_values.append(tau)
            sample_count_by_embedder[embedder_id] = len(samples)
        if not args.allow_partial and fid in reference_counts:
            expected_reference_counts = reference_counts_by_embedder.get(fid, {})
            for embedder_id, count in sample_count_by_embedder.items():
                expected_reference_count = expected_reference_counts.get(embedder_id, 0)
                if count != expected_reference_count:
                    missing_reference_vectors[f"{fid}:{embedder_id}"] = expected_reference_count - count
        calibration_observations = min(sample_count_by_embedder.values()) if sample_count_by_embedder else 0
        calibrated = dict(row)
        confidence = dict(calibrated.get("confidence") or {})
        confidence["calibration_observations"] = calibration_observations
        confidence["calibration_state"] = "calibrated"
        calibrated["tau_by_embedder"] = tau_by_embedder
        calibrated["tau_state"] = "calibrated_same_session_band_p10"
        calibrated["confidence"] = confidence
        calibrated["calibration"] = {
            "task_id": TASK_ID,
            "formula_version": FORMULA_VERSION,
            "calibrated_at_utc": calibrated_at_utc,
            "calibrated_at_unix_ms": calibrated_at_unix_ms,
            "same_session_band_percentile": args.same_session_band_percentile,
            "percentile_method": "nearest_rank_floor",
            "sample_count": calibration_observations,
            "sample_count_by_embedder": sample_count_by_embedder,
            "source_seed_root": str(args.seed_root),
            "source_seed_fingerprints_sha256": seed_summary["fingerprints_sha256"],
        }
        calibrated["record_sha256"] = None
        calibrated["record_sha256"] = stable_record_sha(calibrated)
        calibrated_rows.append(calibrated)
        calibration_payload = {
            "schema_version": SCHEMA_VERSION,
            "artifact_kind": "failure_fingerprint_tau_calibration",
            "task_id": TASK_ID,
            "formula_version": FORMULA_VERSION,
            "fingerprint_id": fid,
            "calibrated_at_utc": calibrated_at_utc,
            "calibrated_at_unix_ms": calibrated_at_unix_ms,
            "tau_by_embedder": tau_by_embedder,
            "same_session_band_percentile": args.same_session_band_percentile,
            "percentile_method": "nearest_rank_floor",
            "sample_count": calibration_observations,
            "sample_count_by_embedder": sample_count_by_embedder,
            "source_seed_record_sha256": row.get("record_sha256"),
            "record_sha256": None,
        }
        calibration_payload["record_sha256"] = sha256_text(
            {k: v for k, v in calibration_payload.items() if k != "record_sha256"}
        )
        calibration_rows.append(calibration_payload)

    require(not missing_samples, "FINGERPRINT_CALIBRATION_MISSING_SAMPLES", str(missing_samples[:10]))
    require(not missing_reference_vectors, "FINGERPRINT_CALIBRATION_REFERENCE_VECTOR_GAP", str(missing_reference_vectors.most_common(10)))
    require(bool(tau_values), "FINGERPRINT_CALIBRATION_NO_TAU_VALUES")

    known_good_count = sum(1 for row in calibrated_rows if (row.get("kind") or {}).get("type") == "known_good")
    known_bad_count = sum(1 for row in calibrated_rows if (row.get("kind") or {}).get("type") == "known_bad")
    if not args.allow_partial:
        require(
            len(calibrated_rows) == args.expected_fingerprints,
            "FINGERPRINT_CALIBRATION_OUTPUT_COUNT_MISMATCH",
            str(len(calibrated_rows)),
        )
        require(known_good_count == args.expected_known_good, "FINGERPRINT_CALIBRATION_OUTPUT_KNOWN_GOOD_MISMATCH")
        require(known_bad_count == args.expected_known_bad, "FINGERPRINT_CALIBRATION_OUTPUT_KNOWN_BAD_MISMATCH")

    write_jsonl(args.output_root / "failure_fingerprints.jsonl", calibrated_rows)
    write_jsonl(args.output_root / "fingerprint_calibration.jsonl", calibration_rows)
    shutil.copy2(args.seed_root / "fingerprint_references.jsonl", args.output_root / "fingerprint_references.jsonl")

    report = {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "artifact_kind": ARTIFACT_KIND,
        "formula_version": FORMULA_VERSION,
        "started_at_utc": started,
        "created_at_utc": utc_now(),
        "source_of_truth": {
            "seed_root": str(args.seed_root),
            "output_root": str(args.output_root),
            "fingerprints": str(args.output_root / "failure_fingerprints.jsonl"),
            "references": str(args.output_root / "fingerprint_references.jsonl"),
            "calibration": str(args.output_root / "fingerprint_calibration.jsonl"),
            "report": str(args.output_root / "fingerprint_calibration_report.json"),
        },
        "inputs": {
            "forward_root": str(args.forward_root),
            "source_corpus": args.source_corpus,
            "expected_embedders": list(expected_embedders),
            "same_session_band_percentile": args.same_session_band_percentile,
            "percentile_method": "nearest_rank_floor",
            "max_shards": args.max_shards,
            "max_records": args.max_records,
            "allow_partial": args.allow_partial,
            "seed_summary": seed_summary,
        },
        "summary": {
            "records_seen": records_seen,
            "joined_records": joined_records,
            "calibrated_vector_observations": calibrated_vector_observations,
            "fingerprint_count": len(calibrated_rows),
            "calibration_record_count": len(calibration_rows),
            "known_good_count": known_good_count,
            "known_bad_count": known_bad_count,
            "reference_row_count": len(references),
            "rows_by_embedder": dict(sorted(rows_by_embedder.items())),
            "rows_by_fingerprint": dict(sorted(rows_by_fingerprint.items())),
            "tau_min": finite_float(min(tau_values)),
            "tau_max": finite_float(max(tau_values)),
            "tau_mean": finite_float(sum(tau_values) / len(tau_values)),
        },
    }
    dump_json(args.output_root / "fingerprint_calibration_report.json", report)

    readback_fingerprints = [row for _, row in iter_jsonl(args.output_root / "failure_fingerprints.jsonl")]
    readback_calibrations = [row for _, row in iter_jsonl(args.output_root / "fingerprint_calibration.jsonl")]
    readback_report = json.loads((args.output_root / "fingerprint_calibration_report.json").read_text(encoding="utf-8"))
    readback_references_hash = sha256_file(args.output_root / "fingerprint_references.jsonl")
    all_tau_sets_match = all(set(row.get("tau_by_embedder") or {}) == expected_set for row in readback_fingerprints)
    all_tau_in_range = all(
        -1.0 <= float(value) <= 1.0
        for row in readback_fingerprints
        for value in (row.get("tau_by_embedder") or {}).values()
    )
    all_calibrated = all(
        row.get("tau_state") == "calibrated_same_session_band_p10"
        and (row.get("confidence") or {}).get("calibration_state") == "calibrated"
        for row in readback_fingerprints
    )
    readback_passes = (
        len(readback_fingerprints) == len(calibrated_rows)
        and len(readback_calibrations) == len(calibration_rows)
        and readback_report.get("summary", {}).get("fingerprint_count") == len(calibrated_rows)
        and readback_references_hash == seed_summary["references_sha256"]
        and all_tau_sets_match
        and all_tau_in_range
        and all_calibrated
    )
    after = {"output_root": tree_state(args.output_root), "readback_root": tree_state(args.readback_root)}
    readback = {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "artifact_kind": f"{ARTIFACT_KIND}_readback",
        "formula_version": FORMULA_VERSION,
        "started_at_utc": started,
        "created_at_utc": utc_now(),
        "passes": readback_passes,
        "all_passed": readback_passes,
        "trigger": {
            "command": "python3 scripts/swebench_lite_ops.py python-failure-fingerprint-calibrate",
            "source_corpus": args.source_corpus,
        },
        "source_of_truth": {**report["source_of_truth"], "readback": str(args.readback_root / "fingerprint_tau_calibration_readback.json")},
        "before_state": before,
        "after_state": after,
        "expected": {
            "fingerprints": args.expected_fingerprints,
            "known_good": args.expected_known_good,
            "known_bad": args.expected_known_bad,
            "embedder_count_per_fingerprint": len(expected_embedders),
            "same_session_band_percentile": args.same_session_band_percentile,
            "allow_partial": args.allow_partial,
        },
        "actual": report["summary"],
        "readback": {
            "fingerprints_sha256": sha256_file(args.output_root / "failure_fingerprints.jsonl"),
            "references_sha256": readback_references_hash,
            "calibration_sha256": sha256_file(args.output_root / "fingerprint_calibration.jsonl"),
            "report_sha256": sha256_file(args.output_root / "fingerprint_calibration_report.json"),
            "all_tau_sets_match": all_tau_sets_match,
            "all_tau_in_range": all_tau_in_range,
            "all_calibrated": all_calibrated,
            "first_calibrated_fingerprint": readback_fingerprints[0] if readback_fingerprints else None,
            "artifact_state": tree_state(args.output_root),
        },
        "boundary_cases": edge_cases(args.readback_root),
    }
    readback["passes"] = bool(readback["passes"] and all(case["pass"] for case in readback["boundary_cases"]))
    readback["all_passed"] = readback["passes"]
    dump_json(args.readback_root / "fingerprint_tau_calibration_readback.json", readback)
    final = json.loads((args.readback_root / "fingerprint_tau_calibration_readback.json").read_text(encoding="utf-8"))
    require(final.get("all_passed") is True, "FINGERPRINT_CALIBRATION_READBACK_FAILED", str(args.readback_root / "fingerprint_tau_calibration_readback.json"))
    return final


def edge_cases(readback_root: Path) -> list[dict[str, Any]]:
    cases: list[dict[str, Any]] = []
    try:
        same_session_tau([], 0.10)
        actual = None
    except FingerprintCalibrationError as exc:
        actual = str(exc).split(":", 1)[0]
    cases.append(
        {
            "case": "empty_tau_sample",
            "action": "compute same-session tau",
            "expected": "FINGERPRINT_CALIBRATION_EMPTY_SAMPLE",
            "actual": actual,
            "pass": actual == "FINGERPRINT_CALIBRATION_EMPTY_SAMPLE",
        }
    )
    try:
        same_session_tau([0.1, 0.2], 0.0)
        actual = None
    except FingerprintCalibrationError as exc:
        actual = str(exc).split(":", 1)[0]
    cases.append(
        {
            "case": "bad_percentile",
            "action": "compute same-session tau",
            "expected": "FINGERPRINT_CALIBRATION_BAD_PERCENTILE",
            "actual": actual,
            "pass": actual == "FINGERPRINT_CALIBRATION_BAD_PERCENTILE",
        }
    )
    try:
        cosine_to_centroid(np.asarray([0.0, 0.0], dtype=np.float64), np.asarray([1.0, 0.0], dtype=np.float64))
        actual = None
    except FingerprintCalibrationError as exc:
        actual = str(exc).split(":", 1)[0]
    cases.append(
        {
            "case": "zero_vector_cosine",
            "action": "score calibration vector against centroid",
            "expected": "FINGERPRINT_CALIBRATION_ZERO_VECTOR",
            "actual": actual,
            "pass": actual == "FINGERPRINT_CALIBRATION_ZERO_VECTOR",
        }
    )
    missing = readback_root / "missing-seed-root"
    if missing.exists():
        shutil.rmtree(missing)
    try:
        load_seed_catalog(missing)
        actual = None
    except FingerprintCalibrationError as exc:
        actual = str(exc).split(":", 1)[0]
    cases.append(
        {
            "case": "missing_seed_catalog",
            "action": "load seed catalog",
            "expected": "FINGERPRINT_CALIBRATION_SEED_FINGERPRINTS_MISSING",
            "actual": actual,
            "pass": actual == "FINGERPRINT_CALIBRATION_SEED_FINGERPRINTS_MISSING",
        }
    )
    return cases


def prepare_fixture(args: argparse.Namespace) -> None:
    embedders = tuple(item.strip() for item in args.expected_embedders.split(",") if item.strip())
    fixture_root = args.readback_root / "fixture-input"
    chunks_path, forward_root = build_fixture(fixture_root, embedders)
    seed_root = args.readback_root / "fixture-seed-output"
    seed_readback_root = args.readback_root / "fixture-seed-readback"
    seed_args = argparse.Namespace(
        forward_root=forward_root,
        chunks_jsonl=chunks_path,
        output_root=seed_root,
        readback_root=seed_readback_root,
        source_corpus="task-fp-003-fixture",
        expected_embedders=",".join(embedders),
        expected_fingerprints=5,
        expected_known_good=3,
        expected_known_bad=2,
        max_shards=None,
        max_records=None,
        progress_every_records=0,
        allow_partial=False,
        clean_output=True,
    )
    materialize_catalog(seed_args)
    args.seed_root = seed_root
    args.forward_root = forward_root
    args.output_root = args.readback_root / "fixture-calibrated-output"
    args.source_corpus = "task-fp-003-fixture"
    args.expected_fingerprints = 5
    args.expected_known_good = 3
    args.expected_known_bad = 2
    args.clean_output = True


def command_python_failure_fingerprint_calibrator(args: argparse.Namespace) -> int:
    if args.fixture_mode:
        prepare_fixture(args)
    try:
        final = calibrate_catalog(args)
    except (SmokeMatrixError, FingerprintMaterializerError) as exc:
        print(f"PYTHON_FAILURE_FINGERPRINT_CALIBRATION_FAILED: {exc}", file=sys.stderr)
        return 1
    print(json.dumps(final, sort_keys=True, indent=2))
    return 0


def add_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--forward-root", type=Path, default=DEFAULT_FORWARD_ROOT)
    parser.add_argument("--seed-root", type=Path, default=DEFAULT_SEED_ROOT)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--readback-root", type=Path, default=DEFAULT_READBACK_ROOT)
    parser.add_argument("--source-corpus", default=DEFAULT_SOURCE_CORPUS)
    parser.add_argument("--expected-embedders", default=",".join(EXPECTED_EMBEDDERS))
    parser.add_argument("--expected-fingerprints", type=int, default=97)
    parser.add_argument("--expected-known-good", type=int, default=13)
    parser.add_argument("--expected-known-bad", type=int, default=84)
    parser.add_argument("--same-session-band-percentile", type=float, default=0.10)
    parser.add_argument("--max-shards", type=int, default=None)
    parser.add_argument("--max-records", type=int, default=None)
    parser.add_argument("--progress-every-records", type=int, default=100_000)
    parser.add_argument("--allow-partial", action="store_true", help="Permit bounded smoke runs with fewer than the full 97 seed fingerprints.")
    parser.add_argument("--clean-output", action="store_true")
    parser.add_argument(
        "--fixture-mode",
        action="store_true",
        help="Build a deterministic seed fixture, then calibrate it end to end.",
    )
