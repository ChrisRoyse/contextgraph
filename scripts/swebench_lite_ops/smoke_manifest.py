from __future__ import annotations

import hashlib
import json
import re
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any

from .core import SwebenchOpsError, ensure_file, utc_now, write_json_checked
from .oracle_events import (
    canonical_json,
    capacity_snapshot,
    file_state,
    path_bytes,
    sha256_file,
    write_jsonl_checked,
)
from .source_readback import embedded_chunk_source_proof, is_legacy_mounted_path


SCHEMA_VERSION = 1
TASK_ID = "TASK-PY-DATA-060"
DEFAULT_TARGET_COUNT = 1000
DEFAULT_SEED = "python-embedder-intelligence-smoke-1000-v2"
SHA256_RE = re.compile(r"^sha256:[0-9a-f]{64}$")
EXPECTED_MUTATION_CATEGORIES = (
    "compile_error",
    "delete_test_call",
    "known_good",
    "off_by_one",
    "over_engineer",
    "subtle_flip",
    "swap_variable",
    "wrong_file",
)


class SmokeManifestError(SwebenchOpsError):
    pass


def stable_hash(payload: Any) -> str:
    return "sha256:" + hashlib.sha256(canonical_json(payload).encode("utf-8")).hexdigest()


def stable_sort_key(seed: str, row: dict[str, Any], salt: str) -> str:
    return hashlib.sha256(
        canonical_json(
            {
                "seed": seed,
                "salt": salt,
                "chunk_id": row.get("chunk_id"),
                "subject": row.get("subject"),
                "workspace_state": row.get("workspace_state"),
            }
        ).encode("utf-8")
    ).hexdigest()


def count_jsonl(path: Path) -> int:
    if not path.is_file():
        return 0
    return sum(1 for line in path.read_text(encoding="utf-8").splitlines() if line.strip())


def load_jsonl(path: Path, code: str) -> list[dict[str, Any]]:
    ensure_file(path, code, "required JSONL artifact")
    rows: list[dict[str, Any]] = []
    for line_number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
        if not line.strip():
            continue
        try:
            payload = json.loads(line)
        except json.JSONDecodeError as exc:
            raise SmokeManifestError(
                code,
                f"invalid JSONL line {line_number} at {path}",
                "Regenerate the upstream JSONL artifact before selecting a smoke manifest.",
                {"path": str(path), "line_number": line_number, "error": str(exc)},
            ) from exc
        if not isinstance(payload, dict):
            raise SmokeManifestError(
                code,
                f"JSONL line {line_number} is not an object at {path}",
                "Regenerate the upstream JSONL artifact before selecting a smoke manifest.",
                {"path": str(path), "line_number": line_number, "actual_type": type(payload).__name__},
            )
        rows.append(payload)
    return rows


def load_json_object(path: Path, code: str, description: str) -> dict[str, Any]:
    ensure_file(path, code, description)
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise SmokeManifestError(
            code,
            f"{description} is invalid JSON: {path}",
            "Regenerate the upstream artifact before selecting a smoke manifest.",
            {"path": str(path), "error": str(exc)},
        ) from exc
    if not isinstance(payload, dict):
        raise SmokeManifestError(
            code,
            f"{description} must be a JSON object: {path}",
            "Regenerate the upstream artifact before selecting a smoke manifest.",
            {"path": str(path), "actual_type": type(payload).__name__},
        )
    return payload


def require_source_hash_match(expected_sha: str, actual_sha: str, chunks_jsonl: Path, full_ast_corpus_readback: Path) -> None:
    if expected_sha != actual_sha:
        raise SmokeManifestError(
            "SMOKE_MANIFEST_STALE_SOURCE_CHUNKS_HASH",
            "smoke manifest source chunks hash does not match #249 READBACK",
            "Regenerate the smoke manifest from the accepted #249 chunk Source of Truth.",
            {
                "expected": expected_sha,
                "actual": actual_sha,
                "chunks_jsonl": str(chunks_jsonl),
                "full_ast_corpus_readback": str(full_ast_corpus_readback),
            },
        )


def load_full_ast_corpus_gate(path: Path, chunks_jsonl: Path, expected_task_count: int) -> dict[str, Any]:
    payload = load_json_object(path, "SMOKE_MANIFEST_FULL_AST_READBACK_INVALID", "#249 full AST corpus READBACK")
    if payload.get("passes") is not True:
        raise SmokeManifestError(
            "SMOKE_MANIFEST_FULL_AST_READBACK_NOT_PASSING",
            "#249 full AST corpus READBACK is not passing",
            "Resolve #249 before selecting the post-#249 smoke manifest.",
            {"path": str(path), "passes": payload.get("passes")},
        )
    after_state = payload.get("after_state")
    if not isinstance(after_state, dict):
        raise SmokeManifestError(
            "SMOKE_MANIFEST_FULL_AST_READBACK_AFTER_STATE_MISSING",
            "#249 full AST corpus READBACK has no after_state object",
            "Regenerate #249 READBACK before selecting the post-#249 smoke manifest.",
            {"path": str(path)},
        )
    chunks_state = after_state.get("chunks")
    coverage = after_state.get("chunk_coverage")
    if not isinstance(chunks_state, dict) or not isinstance(coverage, dict):
        raise SmokeManifestError(
            "SMOKE_MANIFEST_FULL_AST_READBACK_SOURCE_MISSING",
            "#249 full AST corpus READBACK is missing chunk source or coverage state",
            "Regenerate #249 READBACK before selecting the post-#249 smoke manifest.",
            {"path": str(path), "after_state_keys": sorted(after_state.keys())},
        )
    required_sha = chunks_state.get("sha256")
    required_path = chunks_state.get("path")
    require_sha(required_sha, "full_ast_corpus_readback.after_state.chunks.sha256", {"chunk_id": required_sha, "subject": str(path)})
    if str(chunks_jsonl) != str(required_path):
        raise SmokeManifestError(
            "SMOKE_MANIFEST_SOURCE_CHUNKS_PATH_MISMATCH",
            "smoke manifest source chunks path does not match #249 READBACK",
            "Use the accepted #249 chunk Source of Truth path.",
            {"expected": required_path, "actual": str(chunks_jsonl), "full_ast_corpus_readback": str(path)},
        )
    actual_sha = sha256_file(chunks_jsonl)
    require_source_hash_match(required_sha, actual_sha, chunks_jsonl, path)
    task_count = coverage.get("task_count")
    pair_count = coverage.get("task_category_pair_count")
    if task_count != expected_task_count or pair_count != expected_task_count * len(EXPECTED_MUTATION_CATEGORIES):
        raise SmokeManifestError(
            "SMOKE_MANIFEST_FULL_AST_COVERAGE_MISMATCH",
            "#249 full AST corpus READBACK does not prove the expected 300x8 coverage",
            "Resolve #249 coverage before selecting the post-#249 smoke manifest.",
            {"expected_task_count": expected_task_count, "expected_pair_count": expected_task_count * len(EXPECTED_MUTATION_CATEGORIES), "task_count": task_count, "pair_count": pair_count},
        )
    return {
        "full_ast_corpus_readback": str(path),
        "source_chunks_jsonl": str(chunks_jsonl),
        "source_chunks_sha256": required_sha,
        "actual_source_chunks_sha256": actual_sha,
        "source_chunks_bytes": chunks_state.get("bytes"),
        "coverage": coverage,
    }


def load_cleaned_manifest(path: Path) -> dict[str, dict[str, Any]]:
    payload = load_json_object(path, "SMOKE_MANIFEST_CLEANED_MANIFEST_INVALID_JSON", "cleaned Python corpus manifest")
    rows = payload.get("rows")
    if not isinstance(rows, list) or not rows:
        raise SmokeManifestError(
            "SMOKE_MANIFEST_CLEANED_ROWS_MISSING",
            "cleaned manifest has no rows array",
            "Regenerate TASK-PY-DATA-004 before selecting a smoke manifest.",
            {"path": str(path), "keys": sorted(payload.keys())},
        )
    return {row["row_id"]: row for row in rows}


def require_sha(value: Any, field: str, row: dict[str, Any]) -> None:
    if not isinstance(value, str) or not SHA256_RE.match(value):
        raise SmokeManifestError(
            "SMOKE_MANIFEST_CHUNK_HASH_INVALID",
            f"chunk candidate has invalid {field}",
            "Reject this candidate and regenerate upstream chunk artifacts if this is widespread.",
            {
                "field": field,
                "value": value,
                "chunk_id": row.get("chunk_id"),
                "subject": row.get("subject"),
            },
        )


def validate_candidate_shape(row: dict[str, Any]) -> None:
    required = (
        "chunk_id",
        "task_instance_id",
        "workspace_state",
        "mutation_category",
        "repo",
        "relative_path",
        "absolute_path",
        "file_sha256",
        "source_sha256",
        "source_text",
        "chunk_kind",
        "node_kind",
        "line_start",
        "line_end",
        "byte_start",
        "byte_end",
        "source_slice_verified",
    )
    missing = [key for key in required if key not in row]
    if missing:
        raise SmokeManifestError(
            "SMOKE_MANIFEST_CHUNK_FIELD_MISSING",
            "chunk candidate is missing required fields",
            "Reject this candidate and regenerate upstream chunk artifacts if this is widespread.",
            {"missing": missing, "chunk_id": row.get("chunk_id"), "subject": row.get("subject")},
        )
    require_sha(row["chunk_id"], "chunk_id", row)
    require_sha(row["file_sha256"], "file_sha256", row)
    require_sha(row["source_sha256"], "source_sha256", row)
    if not isinstance(row["source_text"], str):
        raise SmokeManifestError(
            "SMOKE_MANIFEST_SOURCE_TEXT_INVALID",
            "chunk candidate source_text is not a string",
            "Regenerate AST chunk artifacts before using this candidate as reward data.",
            {"chunk_id": row["chunk_id"], "subject": row.get("subject"), "actual_type": type(row["source_text"]).__name__},
        )
    if row["source_slice_verified"] is not True:
        raise SmokeManifestError(
            "SMOKE_MANIFEST_SOURCE_SLICE_NOT_VERIFIED",
            "chunk candidate does not have verified source slice",
            "Regenerate AST chunk artifacts before using this candidate as reward data.",
            {"chunk_id": row["chunk_id"], "subject": row.get("subject")},
        )
    byte_start = int(row["byte_start"])
    byte_end = int(row["byte_end"])
    if byte_start < 0 or byte_end < byte_start:
        raise SmokeManifestError(
            "SMOKE_MANIFEST_BYTE_RANGE_INVALID",
            "chunk candidate has an invalid byte range",
            "Regenerate AST chunk artifacts before using this candidate as reward data.",
            {
                "chunk_id": row["chunk_id"],
                "subject": row.get("subject"),
                "byte_start": row["byte_start"],
                "byte_end": row["byte_end"],
            },
        )


def validate_candidate_source(row: dict[str, Any]) -> dict[str, Any]:
    validate_candidate_shape(row)
    encoding = row.get("encoding") or "utf-8"
    try:
        expected_source_bytes = row["source_text"].encode(encoding)
    except UnicodeEncodeError as exc:
        raise SmokeManifestError(
            "SMOKE_MANIFEST_SOURCE_TEXT_ENCODING_FAILED",
            "chunk candidate source_text cannot be encoded with its declared encoding",
            "Regenerate AST chunk artifacts before using this candidate as reward data.",
            {"chunk_id": row["chunk_id"], "subject": row.get("subject"), "encoding": encoding, "error": str(exc)},
        ) from exc
    expected_source_sha = "sha256:" + hashlib.sha256(expected_source_bytes).hexdigest()
    if expected_source_sha != row["source_sha256"]:
        raise SmokeManifestError(
            "SMOKE_MANIFEST_SOURCE_TEXT_HASH_MISMATCH",
            "chunk candidate source_text hash does not match source_sha256",
            "Regenerate AST chunks from stable source bytes before selecting the smoke manifest.",
            {
                "chunk_id": row["chunk_id"],
                "subject": row.get("subject"),
                "expected_source_sha256": row["source_sha256"],
                "actual_source_sha256": expected_source_sha,
            },
        )
    byte_start = int(row["byte_start"])
    byte_end = int(row["byte_end"])
    byte_span_width = byte_end - byte_start
    if byte_span_width != len(expected_source_bytes):
        raise SmokeManifestError(
            "SMOKE_MANIFEST_SOURCE_TEXT_BYTE_WIDTH_MISMATCH",
            "chunk candidate source_text byte length does not match byte range width",
            "Regenerate AST chunks from stable source bytes before selecting the smoke manifest.",
            {
                "chunk_id": row["chunk_id"],
                "subject": row.get("subject"),
                "byte_start": byte_start,
                "byte_end": byte_end,
                "byte_span_width": byte_span_width,
                "source_text_byte_len": len(expected_source_bytes),
            },
        )
    path = Path(row["absolute_path"])
    if is_legacy_mounted_path(row["absolute_path"]):
        proof = embedded_chunk_source_proof(row)
        proof.update(
            {
                "byte_span_width": byte_span_width,
                "source_slice_verified_claim": True,
                "live_workspace_present": False,
                "live_workspace_verdict": "legacy_mounted_path_rejected",
                "legacy_provenance_path": row["absolute_path"],
            }
        )
        return proof
    if not path.is_file():
        return {
            "source_mode": "durable_ast_chunk_jsonl_embedded_source",
            "durable_source_byte_len": len(expected_source_bytes),
            "durable_source_sha256": expected_source_sha,
            "byte_span_width": byte_span_width,
            "source_text_matches_declared_hash": True,
            "source_slice_verified_claim": True,
            "live_workspace_present": False,
            "live_workspace_verdict": "not_available",
        }
    actual_file_sha = sha256_file(path)
    if actual_file_sha != row["file_sha256"]:
        raise SmokeManifestError(
            "SMOKE_MANIFEST_SOURCE_FILE_HASH_MISMATCH",
            "selected chunk source file hash does not match chunk row",
            "Regenerate AST chunks from stable source bytes before selecting the smoke manifest.",
            {
                "chunk_id": row["chunk_id"],
                "path": str(path),
                "expected": row["file_sha256"],
                "actual": actual_file_sha,
            },
        )
    raw = path.read_bytes()
    actual_source_bytes = raw[byte_start:byte_end]
    actual_source_sha = "sha256:" + hashlib.sha256(actual_source_bytes).hexdigest()
    if actual_source_sha != row["source_sha256"] or actual_source_bytes != expected_source_bytes:
        raise SmokeManifestError(
            "SMOKE_MANIFEST_SOURCE_SLICE_MISMATCH",
            "selected chunk source slice does not re-read from disk",
            "Reject this candidate and regenerate AST chunks before selecting a manifest.",
            {
                "chunk_id": row["chunk_id"],
                "path": str(path),
                "byte_start": byte_start,
                "byte_end": byte_end,
                "expected_source_sha256": row["source_sha256"],
                "actual_source_sha256": actual_source_sha,
            },
        )
    return {
        "source_mode": "durable_ast_chunk_jsonl_plus_live_workspace_readback",
        "durable_source_byte_len": len(expected_source_bytes),
        "durable_source_sha256": expected_source_sha,
        "byte_span_width": byte_span_width,
        "source_text_matches_declared_hash": True,
        "source_slice_verified_claim": True,
        "live_workspace_present": True,
        "live_workspace_verdict": "verified",
        "source_file_bytes": path.stat().st_size,
        "source_file_sha256": actual_file_sha,
        "source_slice_sha256": actual_source_sha,
    }


def dedupe_by_chunk_id(rows: list[dict[str, Any]], seed: str) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    chosen: dict[str, dict[str, Any]] = {}
    duplicate_counts: Counter[str] = Counter()
    for row in rows:
        validate_candidate_shape(row)
        chunk_id = row["chunk_id"]
        if chunk_id in chosen:
            duplicate_counts[chunk_id] += 1
            current = chosen[chunk_id]
            if stable_sort_key(seed, row, "dedupe") < stable_sort_key(seed, current, "dedupe"):
                chosen[chunk_id] = row
        else:
            chosen[chunk_id] = row
    deduped = sorted(chosen.values(), key=lambda row: stable_sort_key(seed, row, "deduped-order"))
    return deduped, {
        "input_count": len(rows),
        "unique_chunk_id_count": len(deduped),
        "duplicate_row_count": sum(duplicate_counts.values()),
        "duplicate_chunk_id_count": len(duplicate_counts),
        "duplicate_sample": [
            {"chunk_id": chunk_id, "extra_rows": count}
            for chunk_id, count in duplicate_counts.most_common(10)
        ],
    }


def token_estimate(text: str) -> int:
    return len(re.findall(r"[A-Za-z_][A-Za-z_0-9]*|\\d+|\\S", text))


def source_size_bucket(byte_len: int) -> str:
    if byte_len == 0:
        return "empty"
    if byte_len <= 128:
        return "tiny"
    if byte_len <= 512:
        return "small"
    if byte_len <= 2048:
        return "medium"
    return "large"


def row_source_size_bucket(row: dict[str, Any]) -> str:
    return source_size_bucket(len(str(row["source_text"]).encode(row.get("encoding") or "utf-8")))


def row_dimension_value(row: dict[str, Any], dimension: str) -> str:
    if dimension == "source_size_bucket":
        return row_source_size_bucket(row)
    value = row.get(dimension)
    if not isinstance(value, str) or not value:
        raise SmokeManifestError(
            "SMOKE_MANIFEST_STRATIFICATION_FIELD_INVALID",
            "chunk candidate has invalid stratification field",
            "Regenerate AST chunks before selecting a stratified smoke manifest.",
            {"dimension": dimension, "chunk_id": row.get("chunk_id"), "value": value},
        )
    return value


def is_syntax_error_row(row: dict[str, Any]) -> bool:
    return row.get("chunk_kind") == "syntax_error_file" or row.get("node_kind") == "syntax_error_file"


def select_representative(
    rows: list[dict[str, Any]],
    seed: str,
    salt: str,
    selected_ids: set[str],
) -> dict[str, Any] | None:
    candidates = [row for row in rows if row["chunk_id"] not in selected_ids]
    if not candidates:
        return None
    return sorted(candidates, key=lambda row: stable_sort_key(seed, row, salt))[0]


def source_row_id(row: dict[str, Any]) -> str:
    return f"{row['task_instance_id']}::{row['mutation_category']}"


def select_smoke_rows(rows: list[dict[str, Any]], target_count: int, seed: str) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    if target_count <= 0:
        raise SmokeManifestError(
            "SMOKE_MANIFEST_TARGET_COUNT_INVALID",
            "target count must be positive",
            "Use a positive target count such as 1000.",
            {"target_count": target_count},
        )
    if not rows:
        raise SmokeManifestError(
            "SMOKE_MANIFEST_EMPTY_CANDIDATE_POOL",
            "no accepted chunk candidates were available",
            "Run the AST chunk pipeline before selecting a smoke manifest.",
            {"target_count": target_count},
        )
    deduped, dedupe = dedupe_by_chunk_id(rows, seed)
    if len(deduped) < target_count:
        raise SmokeManifestError(
            "SMOKE_MANIFEST_INSUFFICIENT_UNIQUE_CHUNKS",
            f"need {target_count} unique chunks, found {len(deduped)}",
            "Regenerate or broaden the accepted AST chunk corpus.",
            {"target_count": target_count, "dedupe": dedupe},
        )

    selected: list[dict[str, Any]] = []
    selected_ids: set[str] = set()
    coverage_prefill: list[dict[str, Any]] = []
    coverage_dimensions = (
        "task_instance_id",
        "repo",
        "mutation_category",
        "workspace_state",
        "chunk_kind",
        "source_size_bucket",
    )
    for dimension in coverage_dimensions:
        dimension_groups: dict[str, list[dict[str, Any]]] = defaultdict(list)
        for row in deduped:
            dimension_groups[row_dimension_value(row, dimension)].append(row)
        for dimension_rows in dimension_groups.values():
            dimension_rows.sort(key=lambda row: stable_sort_key(seed, row, f"coverage-{dimension}"))
        values = sorted(dimension_groups)
        if len(values) > target_count:
            raise SmokeManifestError(
                "SMOKE_MANIFEST_COVERAGE_DIMENSION_TOO_WIDE",
                "target count is too small to represent every value in a required stratification dimension",
                "Increase target count or narrow the source corpus before selecting a smoke manifest.",
                {"dimension": dimension, "value_count": len(values), "target_count": target_count},
            )
        represented_values = {row_dimension_value(row, dimension) for row in selected}
        for value in values:
            if value in represented_values:
                continue
            representative = next((row for row in dimension_groups[value] if row["chunk_id"] not in selected_ids), None)
            if representative is None:
                raise SmokeManifestError(
                    "SMOKE_MANIFEST_COVERAGE_PREFILL_EXHAUSTED",
                    "could not select a unique representative for a required stratification value",
                    "Inspect chunk_id dedupe and coverage dimensions before using the smoke manifest.",
                    {"dimension": dimension, "value": value, "selected_count": len(selected)},
            )
            selected.append(representative)
            selected_ids.add(representative["chunk_id"])
            represented_values.add(value)
            coverage_prefill.append(
                {
                    "dimension": dimension,
                    "value": value,
                    "chunk_id": representative["chunk_id"],
                    "task_instance_id": representative["task_instance_id"],
                    "mutation_category": representative["mutation_category"],
                }
            )
    syntax_error_candidates = [row for row in deduped if is_syntax_error_row(row)]
    if syntax_error_candidates and not any(is_syntax_error_row(row) for row in selected):
        representative = select_representative(syntax_error_candidates, seed, "coverage-syntax-error-file", selected_ids)
        if representative is None:
            raise SmokeManifestError(
                "SMOKE_MANIFEST_SYNTAX_ERROR_PREFILL_EXHAUSTED",
                "syntax-error chunks exist but no unique syntax-error representative could be selected",
                "Inspect chunk_id dedupe and syntax-error chunk generation before using the smoke manifest.",
                {"syntax_error_candidate_count": len(syntax_error_candidates), "selected_count": len(selected)},
            )
        selected.append(representative)
        selected_ids.add(representative["chunk_id"])
        coverage_prefill.append(
            {
                "dimension": "syntax_error_file",
                "value": "present",
                "chunk_id": representative["chunk_id"],
                "task_instance_id": representative["task_instance_id"],
                "mutation_category": representative["mutation_category"],
            }
        )

    if len(selected) > target_count:
        raise SmokeManifestError(
            "SMOKE_MANIFEST_COVERAGE_PREFILL_TOO_LARGE",
            "required stratification representatives exceed target count",
            "Increase target count before selecting a smoke manifest.",
            {"target_count": target_count, "prefill_count": len(selected)},
        )

    buckets: dict[tuple[str, str, str], list[dict[str, Any]]] = defaultdict(list)
    for row in deduped:
        buckets[(row["repo"], row["mutation_category"], row["chunk_kind"])].append(row)
    for bucket_rows in buckets.values():
        bucket_rows.sort(key=lambda row: stable_sort_key(seed, row, "bucket-row"))
    bucket_keys = sorted(
        buckets,
        key=lambda key: hashlib.sha256(canonical_json({"seed": seed, "bucket": key}).encode("utf-8")).hexdigest(),
    )
    exhausted = False
    while len(selected) < target_count and not exhausted:
        exhausted = True
        for key in bucket_keys:
            bucket_rows = buckets[key]
            while bucket_rows and bucket_rows[0]["chunk_id"] in selected_ids:
                bucket_rows.pop(0)
            if not bucket_rows:
                continue
            exhausted = False
            row = bucket_rows.pop(0)
            selected.append(row)
            selected_ids.add(row["chunk_id"])
            if len(selected) == target_count:
                break
    if len(selected) != target_count:
        raise SmokeManifestError(
            "SMOKE_MANIFEST_SELECTION_SHORT",
            f"selection ended at {len(selected)} chunks, expected {target_count}",
            "Inspect bucket exhaustion and target-count logic before using the smoke manifest.",
            {"target_count": target_count, "selected_count": len(selected), "bucket_count": len(bucket_keys)},
        )
    selection_summary = {
        "dedupe": dedupe,
        "bucket_count": len(bucket_keys),
        "target_count": target_count,
        "selected_count": len(selected),
        "coverage_prefill_count": len(coverage_prefill),
        "coverage_prefill": coverage_prefill,
        "coverage_dimensions": list(coverage_dimensions) + ["syntax_error_file_if_present"],
        "selection_algorithm": "deterministic coverage prefill across required dimensions, then round-robin over (repo, mutation_category, chunk_kind) after chunk_id dedupe",
    }
    return selected, selection_summary


def select_source_verified_smoke_rows(
    rows: list[dict[str, Any]],
    target_count: int,
    seed: str,
) -> tuple[list[dict[str, Any]], list[dict[str, Any]], dict[str, Any]]:
    selected, selection_summary = select_smoke_rows(rows, target_count, seed)
    source_proofs = [validate_candidate_source(row) for row in selected]
    mode_counts = Counter(proof["source_mode"] for proof in source_proofs)
    live_workspace_count = sum(1 for proof in source_proofs if proof["live_workspace_present"])
    selection_summary = {
        **selection_summary,
        "source_validation_attempts": [
            {
                "attempt": 1,
                "candidate_pool_count": len(rows),
                "selected_count": len(selected),
                "durable_source_verified_count": len(source_proofs),
                "live_workspace_verified_count": live_workspace_count,
                "source_mode_counts": dict(sorted(mode_counts.items())),
            }
        ],
        "source_rejection_count": 0,
        "source_rejection_sample": [],
        "source_mode_counts": dict(sorted(mode_counts.items())),
        "source_of_truth_policy": "accepted AST chunk JSONL source_text/source_sha256/source_slice_verified are durable SoT; live workspace files are opportunistic extra readback only",
    }
    return selected, source_proofs, selection_summary


def stratification_counts(rows: list[dict[str, Any]]) -> dict[str, Any]:
    return {
        "total": len(rows),
        "by_repo": dict(sorted(Counter(row["repo"] for row in rows).items())),
        "by_task_instance_id": dict(sorted(Counter(row["task_instance_id"] for row in rows).items())),
        "by_mutation_category": dict(sorted(Counter(row["mutation_category"] for row in rows).items())),
        "by_workspace_state": dict(sorted(Counter(row["workspace_state"] for row in rows).items())),
        "by_chunk_kind": dict(sorted(Counter(row["chunk_kind"] for row in rows).items())),
        "by_source_size_bucket": dict(sorted(Counter(row_source_size_bucket(row) for row in rows).items())),
        "syntax_error_file_count": sum(1 for row in rows if is_syntax_error_row(row)),
    }


def stratification_requirements(
    candidates: list[dict[str, Any]],
    selected: list[dict[str, Any]],
    expected_task_count: int,
    expected_categories: tuple[str, ...],
) -> dict[str, Any]:
    dimensions = (
        "task_instance_id",
        "repo",
        "mutation_category",
        "workspace_state",
        "chunk_kind",
        "source_size_bucket",
    )
    dimension_results: dict[str, Any] = {}
    missing_by_dimension: dict[str, list[str]] = {}
    for dimension in dimensions:
        candidate_values = sorted({row_dimension_value(row, dimension) for row in candidates})
        selected_values = sorted({row_dimension_value(row, dimension) for row in selected})
        missing = sorted(set(candidate_values) - set(selected_values))
        dimension_results[dimension] = {
            "candidate_value_count": len(candidate_values),
            "selected_value_count": len(selected_values),
            "candidate_values": candidate_values,
            "selected_values": selected_values,
            "missing_values": missing,
            "passes": not missing,
        }
        if missing:
            missing_by_dimension[dimension] = missing
    candidate_categories = sorted({row["mutation_category"] for row in candidates})
    selected_categories = sorted({row["mutation_category"] for row in selected})
    candidate_syntax_error_count = sum(1 for row in candidates if is_syntax_error_row(row))
    selected_syntax_error_count = sum(1 for row in selected if is_syntax_error_row(row))
    expected_category_result = {
        "expected_categories": list(expected_categories),
        "candidate_categories": candidate_categories,
        "selected_categories": selected_categories,
        "missing_from_candidates": sorted(set(expected_categories) - set(candidate_categories)),
        "missing_from_selected": sorted(set(expected_categories) - set(selected_categories)),
    }
    task_result = {
        "expected_task_count": expected_task_count,
        "candidate_task_count": len({row["task_instance_id"] for row in candidates}),
        "selected_task_count": len({row["task_instance_id"] for row in selected}),
    }
    syntax_error_result = {
        "candidate_syntax_error_file_count": candidate_syntax_error_count,
        "selected_syntax_error_file_count": selected_syntax_error_count,
        "required": candidate_syntax_error_count > 0,
        "passes": candidate_syntax_error_count == 0 or selected_syntax_error_count > 0,
    }
    expected_category_passes = not expected_category_result["missing_from_candidates"] and not expected_category_result["missing_from_selected"]
    task_passes = task_result["candidate_task_count"] == expected_task_count and task_result["selected_task_count"] == expected_task_count
    passes = (
        all(result["passes"] for result in dimension_results.values())
        and syntax_error_result["passes"]
        and expected_category_passes
        and task_passes
    )
    return {
        "dimensions": dimension_results,
        "missing_by_dimension": missing_by_dimension,
        "expected_categories": {**expected_category_result, "passes": expected_category_passes},
        "task_coverage": {**task_result, "passes": task_passes},
        "syntax_error_file": syntax_error_result,
        "passes": passes,
    }


def stratification_warnings(
    candidates: list[dict[str, Any]],
    selected: list[dict[str, Any]],
    expected_task_count: int,
    expected_categories: tuple[str, ...],
) -> list[dict[str, Any]]:
    candidate_tasks = {row["task_instance_id"] for row in candidates}
    candidate_categories = {row["mutation_category"] for row in candidates}
    selected_tasks = {row["task_instance_id"] for row in selected}
    selected_categories = {row["mutation_category"] for row in selected}
    warnings: list[dict[str, Any]] = []
    if len(candidate_tasks) < expected_task_count:
        warnings.append(
            {
                "code": "SMOKE_MANIFEST_SOURCE_TASK_COVERAGE_SHORTFALL",
                "severity": "warning",
                "message": "accepted chunk SoT covers fewer task instances than the intended 300 SWE-bench Lite tasks",
                "expected_task_count": expected_task_count,
                "candidate_task_count": len(candidate_tasks),
                "selected_task_count": len(selected_tasks),
                "follow_up_issue": 249,
            }
        )
    missing_categories = sorted(set(expected_categories) - candidate_categories)
    if missing_categories:
        warnings.append(
            {
                "code": "SMOKE_MANIFEST_SOURCE_CATEGORY_COVERAGE_SHORTFALL",
                "severity": "warning",
                "message": "accepted chunk SoT is missing intended mutation categories",
                "expected_categories": list(expected_categories),
                "candidate_categories": sorted(candidate_categories),
                "selected_categories": sorted(selected_categories),
                "missing_categories": missing_categories,
                "follow_up_issue": 249,
            }
        )
    return warnings


def enrich_selected_row(
    row: dict[str, Any],
    cleaned_rows: dict[str, dict[str, Any]],
    selection_index: int,
    source_proof: dict[str, Any],
) -> dict[str, Any]:
    row_id = source_row_id(row)
    cleaned = cleaned_rows.get(row_id)
    if cleaned is None:
        raise SmokeManifestError(
            "SMOKE_MANIFEST_CLEANED_ROW_MISSING",
            "selected chunk has no matching cleaned manifest row",
            "Regenerate or reconcile the cleaned manifest and AST chunk artifacts.",
            {"row_id": row_id, "chunk_id": row["chunk_id"]},
        )
    text = str(row["source_text"])
    oracle = cleaned.get("oracle", {})
    expected_labels = cleaned.get("expected_labels", {})
    return {
        "schema_version": SCHEMA_VERSION,
        "manifest_id": stable_hash({"task": TASK_ID, "selection_index": selection_index, "chunk_id": row["chunk_id"]}),
        "selection_index": selection_index,
        "task_id": TASK_ID,
        "source_row_id": row_id,
        "source_row_sha256": cleaned.get("row_sha256"),
        "task_instance_id": row["task_instance_id"],
        "repo": row["repo"],
        "base_commit": cleaned.get("base_commit"),
        "mutation_category": row["mutation_category"],
        "workspace_state": row["workspace_state"],
        "workspace_tree_hash": row["workspace_tree_hash"],
        "chunk_id": row["chunk_id"],
        "chunk_hash": row["chunk_id"],
        "chunk_kind": row["chunk_kind"],
        "node_kind": row["node_kind"],
        "name": row.get("name"),
        "qualname": row.get("qualname"),
        "relative_path": row["relative_path"],
        "absolute_path": row["absolute_path"],
        "file_sha256": row["file_sha256"],
        "source_sha256": row["source_sha256"],
        "byte_range": {"start": row["byte_start"], "end": row["byte_end"]},
        "line_range": {"start": row["line_start"], "end": row["line_end"]},
        "col_range": {"start": row["col_start"], "end": row["col_end"]},
        "token_estimate": token_estimate(text),
        "source_byte_len": len(text.encode(row.get("encoding") or "utf-8")),
        "source_size_bucket": source_size_bucket(len(text.encode(row.get("encoding") or "utf-8"))),
        "oracle_linkage": {
            "resolved": oracle.get("resolved"),
            "all_passed": oracle.get("all_passed"),
            "command": oracle.get("command"),
            "verdict_sha256": oracle.get("verdict_sha256"),
            "fail_to_pass": expected_labels.get("fail_to_pass", []),
            "fail_to_pass_count": expected_labels.get("fail_to_pass_count", 0),
            "pass_to_pass_count": expected_labels.get("pass_to_pass_count", 0),
        },
        "source_proof": source_proof,
        "original_chunk_subject": row.get("subject"),
    }


def edge_audits(
    selected: list[dict[str, Any]],
    warnings: list[dict[str, Any]],
    stratification: dict[str, Any],
    source_gate: dict[str, Any],
    chunks_jsonl: Path,
    full_ast_corpus_readback: Path,
    seed: str,
) -> dict[str, Any]:
    duplicate_probe_rows = selected[:4] + [selected[0], selected[1]]
    duplicate_deduped, duplicate_summary = dedupe_by_chunk_id(duplicate_probe_rows, seed)
    stale_source_hash_error: dict[str, Any]
    try:
        require_source_hash_match(
            "sha256:" + ("0" * 64),
            str(source_gate["actual_source_chunks_sha256"]),
            chunks_jsonl,
            full_ast_corpus_readback,
        )
    except SmokeManifestError as exc:
        stale_source_hash_error = exc.to_json()
    else:
        raise SmokeManifestError(
            "SMOKE_MANIFEST_EDGE_AUDIT_FAILED",
            "stale-source-hash edge audit did not fail",
            "Fix #249 source hash validation before trusting smoke manifest READBACK.",
            {"chunks_jsonl": str(chunks_jsonl), "full_ast_corpus_readback": str(full_ast_corpus_readback)},
        )
    bad_hash_error: dict[str, Any]
    bad = dict(selected[0])
    bad["source_text"] = f"{bad['source_text']}# synthetic hash mismatch"
    try:
        validate_candidate_source(bad)
    except SmokeManifestError as exc:
        bad_hash_error = exc.to_json()
    else:
        raise SmokeManifestError(
            "SMOKE_MANIFEST_EDGE_AUDIT_FAILED",
            "source-hash edge audit did not fail",
            "Fix source validation before trusting smoke manifest READBACK.",
            {"chunk_id": selected[0]["chunk_id"]},
        )
    unverified_source_error: dict[str, Any]
    unverified = dict(selected[0])
    unverified["source_slice_verified"] = False
    try:
        validate_candidate_source(unverified)
    except SmokeManifestError as exc:
        unverified_source_error = exc.to_json()
    else:
        raise SmokeManifestError(
            "SMOKE_MANIFEST_EDGE_AUDIT_FAILED",
            "unverified-source edge audit did not fail",
            "Fix source validation before trusting smoke manifest READBACK.",
            {"chunk_id": selected[0]["chunk_id"]},
        )
    missing_source_text_error: dict[str, Any]
    missing = dict(selected[0])
    missing.pop("source_text", None)
    try:
        validate_candidate_source(missing)
    except SmokeManifestError as exc:
        missing_source_text_error = exc.to_json()
    else:
        raise SmokeManifestError(
            "SMOKE_MANIFEST_EDGE_AUDIT_FAILED",
            "missing-source-text edge audit did not fail",
            "Fix required field validation before trusting smoke manifest READBACK.",
            {"chunk_id": selected[0]["chunk_id"]},
        )
    empty_error: dict[str, Any]
    try:
        select_smoke_rows([], 1, seed)
    except SmokeManifestError as exc:
        empty_error = exc.to_json()
    else:
        raise SmokeManifestError(
            "SMOKE_MANIFEST_EDGE_AUDIT_FAILED",
            "empty-pool edge audit did not fail",
            "Fix candidate-pool validation before trusting smoke manifest READBACK.",
            {},
        )
    return {
        "empty_candidate_pool": {
            "before_state": {"candidate_count": 0},
            "after_state": {"accepted": False, "error_code": empty_error["error_code"]},
            "expected_error_code": "SMOKE_MANIFEST_EMPTY_CANDIDATE_POOL",
            "pass": empty_error["error_code"] == "SMOKE_MANIFEST_EMPTY_CANDIDATE_POOL",
        },
        "duplicate_chunk_ids": {
            "before_state": {"input_count": len(duplicate_probe_rows)},
            "after_state": duplicate_summary,
            "expected": {"unique_chunk_id_count": len(duplicate_deduped), "duplicate_row_count": 2},
            "pass": duplicate_summary["duplicate_row_count"] == 2 and len(duplicate_deduped) == 4,
        },
        "stale_full_ast_source_hash": {
            "before_state": {
                "full_ast_corpus_readback": str(full_ast_corpus_readback),
                "expected_source_chunks_sha256": "sha256:" + ("0" * 64),
                "actual_source_chunks_sha256": source_gate["actual_source_chunks_sha256"],
            },
            "after_state": {"accepted": False, "error_code": stale_source_hash_error["error_code"]},
            "expected_error_code": "SMOKE_MANIFEST_STALE_SOURCE_CHUNKS_HASH",
            "pass": stale_source_hash_error["error_code"] == "SMOKE_MANIFEST_STALE_SOURCE_CHUNKS_HASH",
        },
        "source_text_hash_mismatch": {
            "before_state": {"chunk_id": selected[0]["chunk_id"], "source_sha256": selected[0]["source_sha256"]},
            "after_state": {"accepted": False, "error_code": bad_hash_error["error_code"]},
            "expected_error_code": "SMOKE_MANIFEST_SOURCE_TEXT_HASH_MISMATCH",
            "pass": bad_hash_error["error_code"] == "SMOKE_MANIFEST_SOURCE_TEXT_HASH_MISMATCH",
        },
        "unverified_source_slice": {
            "before_state": {"chunk_id": selected[0]["chunk_id"], "source_slice_verified": False},
            "after_state": {"accepted": False, "error_code": unverified_source_error["error_code"]},
            "expected_error_code": "SMOKE_MANIFEST_SOURCE_SLICE_NOT_VERIFIED",
            "pass": unverified_source_error["error_code"] == "SMOKE_MANIFEST_SOURCE_SLICE_NOT_VERIFIED",
        },
        "missing_source_text_field": {
            "before_state": {"chunk_id": selected[0]["chunk_id"], "source_text_present": False},
            "after_state": {"accepted": False, "error_code": missing_source_text_error["error_code"]},
            "expected_error_code": "SMOKE_MANIFEST_CHUNK_FIELD_MISSING",
            "pass": missing_source_text_error["error_code"] == "SMOKE_MANIFEST_CHUNK_FIELD_MISSING",
        },
        "post249_full_coverage_has_no_warning": {
            "before_state": {"warnings": warnings},
            "after_state": {"warning_count": len(warnings), "codes": [warning["code"] for warning in warnings]},
            "expected": "no source coverage warnings after accepted #249 full 300x8 corpus",
            "pass": len(warnings) == 0,
        },
        "required_stratification_coverage": {
            "before_state": {"required_dimensions": sorted(stratification["dimensions"].keys())},
            "after_state": {
                "passes": stratification["passes"],
                "missing_by_dimension": stratification["missing_by_dimension"],
                "syntax_error_file": stratification["syntax_error_file"],
            },
            "expected": "selected 1000 chunks cover every candidate value for required stratification dimensions and at least one syntax-error chunk if present",
            "pass": stratification["passes"] is True,
        },
    }


def assert_edge_pass(edge_results: dict[str, Any]) -> None:
    failed = [name for name, result in edge_results.items() if not result.get("pass")]
    if failed:
        raise SmokeManifestError(
            "SMOKE_MANIFEST_EDGE_AUDIT_FAILED",
            "one or more smoke manifest edge audits failed",
            "Fix selection/validation before using the smoke manifest.",
            {"failed": failed, "edge_results": edge_results},
        )


def command_smoke_manifest(args: Any) -> dict[str, Any]:
    checked_at = utc_now()
    args.manifest_root.mkdir(parents=True, exist_ok=True)
    args.readback_root.mkdir(parents=True, exist_ok=True)
    chunks_out = args.manifest_root / "smoke_chunks.jsonl"
    manifest_out = args.manifest_root / "smoke_manifest.jsonl"
    summary_out = args.manifest_root / "smoke_manifest_summary.json"
    readback_out = args.readback_root / "smoke_manifest_readback.json"
    before_state = {
        "checked_at": checked_at,
        "source_chunks": file_state(args.chunks_jsonl),
        "full_ast_corpus_readback": file_state(args.full_ast_corpus_readback),
        "cleaned_manifest": file_state(args.cleaned_manifest),
        "chunks_out": file_state(chunks_out),
        "manifest_out": file_state(manifest_out),
        "summary_out": file_state(summary_out),
        "readback_out": file_state(readback_out),
        "manifest_root_bytes": path_bytes(args.manifest_root),
        "readback_root_bytes": path_bytes(args.readback_root),
        "disk": capacity_snapshot([Path("/"), Path("/zfs/hot"), Path("/zfs/archive")]),
    }

    expected_categories = tuple(category.strip() for category in args.expected_mutation_categories.split(",") if category.strip())
    source_gate = load_full_ast_corpus_gate(args.full_ast_corpus_readback, args.chunks_jsonl, args.expected_task_count)
    chunk_rows = load_jsonl(args.chunks_jsonl, "SMOKE_MANIFEST_CHUNKS_JSONL_INVALID")
    cleaned_rows = load_cleaned_manifest(args.cleaned_manifest)
    selected, source_proofs, selection_summary = select_source_verified_smoke_rows(
        chunk_rows,
        args.target_count,
        args.seed,
    )
    enriched = [
        enrich_selected_row(row, cleaned_rows, index, proof)
        for index, (row, proof) in enumerate(zip(selected, source_proofs), start=1)
    ]
    warnings = stratification_warnings(
        chunk_rows,
        selected,
        args.expected_task_count,
        expected_categories,
    )
    stratification = stratification_requirements(chunk_rows, selected, args.expected_task_count, expected_categories)
    edge_results = edge_audits(selected, warnings, stratification, source_gate, args.chunks_jsonl, args.full_ast_corpus_readback, args.seed)
    assert_edge_pass(edge_results)

    chunk_store = write_jsonl_checked(chunks_out, selected)
    manifest_store = write_jsonl_checked(manifest_out, enriched)
    selected_counts = stratification_counts(selected)
    candidate_counts = stratification_counts(chunk_rows)
    selected_hash = stable_hash(enriched)
    source_hashes = {
        "chunks_jsonl": sha256_file(args.chunks_jsonl),
        "full_ast_corpus_readback": sha256_file(args.full_ast_corpus_readback),
        "required_source_chunks_sha256": source_gate["source_chunks_sha256"],
        "actual_source_chunks_sha256": source_gate["actual_source_chunks_sha256"],
        "cleaned_manifest": sha256_file(args.cleaned_manifest),
    }
    summary = {
        "schema_version": SCHEMA_VERSION,
        "created_at_utc": checked_at,
        "task_id": TASK_ID,
        "issue": 251,
        "seed": args.seed,
        "target_count": args.target_count,
        "source_of_truth": {
            "source_policy": "accepted #249 full AST corpus READBACK and chunk JSONL are the durable source of truth for smoke selection; embedded source_text is validated against source_sha256 and source_slice_verified",
            "full_ast_corpus_readback": str(args.full_ast_corpus_readback),
            "source_chunks_jsonl": str(args.chunks_jsonl),
            "cleaned_manifest": str(args.cleaned_manifest),
            "smoke_chunks_jsonl": str(chunks_out),
            "smoke_manifest_jsonl": str(manifest_out),
            "summary": str(summary_out),
            "readback": str(readback_out),
        },
        "source_hashes": source_hashes,
        "source_gate": source_gate,
        "selection": selection_summary,
        "candidate_counts": candidate_counts,
        "selected_counts": selected_counts,
        "stratification_requirements": stratification,
        "stratification_warnings": warnings,
        "selected_manifest_sha256": selected_hash,
        "smoke_chunks_sha256": chunk_store["sha256"],
        "smoke_manifest_sha256": manifest_store["sha256"],
        "passes": True,
    }
    write_json_checked(summary_out, summary)
    after_state = {
        "checked_at": utc_now(),
        "source_chunks": file_state(args.chunks_jsonl),
        "full_ast_corpus_readback": file_state(args.full_ast_corpus_readback),
        "cleaned_manifest": file_state(args.cleaned_manifest),
        "chunks_out": file_state(chunks_out),
        "manifest_out": file_state(manifest_out),
        "summary_out": file_state(summary_out),
        "readback_out_before_current_write": file_state(readback_out),
        "readback_self_hash_note": "final smoke_manifest_readback.json hash is recorded by the command evidence and external readback, not embedded self-referentially",
        "smoke_chunks_count": count_jsonl(chunks_out),
        "smoke_manifest_count": count_jsonl(manifest_out),
        "manifest_root_bytes": path_bytes(args.manifest_root),
        "readback_root_bytes": path_bytes(args.readback_root),
        "disk": capacity_snapshot([Path("/"), Path("/zfs/hot"), Path("/zfs/archive")]),
    }
    happy_path = {
        "expected": {
            "selected_count": args.target_count,
            "unique_chunk_ids": args.target_count,
            "durable_source_chunks_verified": args.target_count,
            "smoke_chunks_count": args.target_count,
            "smoke_manifest_count": args.target_count,
            "source_chunks_sha256": source_gate["source_chunks_sha256"],
            "stratification_requirements_pass": True,
            "source_coverage_warning_count": 0,
        },
        "observed": {
            "selected_count": len(selected),
            "unique_chunk_ids": len({row["chunk_id"] for row in selected}),
            "durable_source_chunks_verified": len(source_proofs),
            "source_chunks_sha256": source_hashes["chunks_jsonl"],
            "full_ast_corpus_required_sha256": source_gate["source_chunks_sha256"],
            "source_mode_counts": selection_summary["source_mode_counts"],
            "smoke_chunks_count": after_state["smoke_chunks_count"],
            "smoke_manifest_count": after_state["smoke_manifest_count"],
            "selected_counts": selected_counts,
            "stratification_requirements": stratification,
            "source_coverage_warning_count": len(warnings),
        },
        "pass": (
            len(selected) == args.target_count
            and len({row["chunk_id"] for row in selected}) == args.target_count
            and len(source_proofs) == args.target_count
            and after_state["smoke_chunks_count"] == args.target_count
            and after_state["smoke_manifest_count"] == args.target_count
            and source_hashes["chunks_jsonl"] == source_gate["source_chunks_sha256"]
            and stratification["passes"] is True
            and len(warnings) == 0
        ),
    }
    if not happy_path["pass"]:
        raise SmokeManifestError(
            "SMOKE_MANIFEST_HAPPY_PATH_FAILED",
            "smoke manifest happy-path readback failed",
            "Inspect output files and selection logic before continuing to #252.",
            {"happy_path": happy_path},
        )
    readback = {
        "schema_version": SCHEMA_VERSION,
        "checked_at_utc": utc_now(),
        "task_id": TASK_ID,
        "issue": 251,
        "source_of_truth": summary["source_of_truth"],
        "before_state": before_state,
        "trigger": {
            "command": "python3 scripts/swebench_lite_ops.py smoke-manifest",
            "target_count": args.target_count,
            "seed": args.seed,
            "selection_algorithm": selection_summary["selection_algorithm"],
        },
        "after_state": after_state,
        "happy_path": happy_path,
        "edge_cases": edge_results,
        "summary": summary,
        "passes": True,
    }
    write_json_checked(readback_out, readback)
    final_readback_state = file_state(readback_out)
    return {
        "ok": True,
        "checked_at": utc_now(),
        "source_of_truth": summary["source_of_truth"],
        "before_state": before_state,
        "after_state": {**after_state, "readback_out": final_readback_state},
        "happy_path": happy_path,
        "edge_cases": edge_results,
        "summary": summary,
        "passes": True,
    }
