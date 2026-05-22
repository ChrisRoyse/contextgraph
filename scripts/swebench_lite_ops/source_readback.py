from __future__ import annotations

import hashlib
from pathlib import Path
from typing import Any

from .core import SwebenchOpsError


LEGACY_MOUNT_PREFIXES = ("/mnt/d/", "/mnt/c/")


class SourceReadbackError(SwebenchOpsError):
    pass


def is_legacy_mounted_path(value: object) -> bool:
    text = str(value or "")
    return any(text.startswith(prefix) for prefix in LEGACY_MOUNT_PREFIXES)


def sha256_prefixed_bytes(raw: bytes) -> str:
    return "sha256:" + hashlib.sha256(raw).hexdigest()


def read_full_source_bytes(
    record: dict[str, Any],
    *,
    path_field: str = "absolute_path",
    expected_sha_field: str = "file_sha256",
    legacy_code: str,
    missing_code: str,
    hash_mismatch_code: str,
    remediation: str,
) -> tuple[Path, bytes]:
    path_value = str(record.get(path_field) or "")
    if not path_value:
        raise SourceReadbackError(
            missing_code,
            f"source path field {path_field} is empty",
            remediation,
            {"path_field": path_field, "record_subject": record.get("subject")},
        )
    if is_legacy_mounted_path(path_value):
        raise SourceReadbackError(
            legacy_code,
            f"legacy mounted-drive source path is not an active source of truth: {path_value}",
            "Canonicalize the row to an aiwonder /zfs path or use embedded source text with sha verification.",
            {"path": path_value, "path_field": path_field, "record_subject": record.get("subject")},
        )

    path = Path(path_value)
    if not path.is_file():
        raise SourceReadbackError(
            missing_code,
            f"source file is missing: {path}",
            remediation,
            {"path": str(path), "path_field": path_field, "record_subject": record.get("subject")},
        )

    raw = path.read_bytes()
    expected_sha = str(record.get(expected_sha_field) or "")
    if expected_sha:
        actual_sha = sha256_prefixed_bytes(raw)
        if actual_sha != expected_sha:
            raise SourceReadbackError(
                hash_mismatch_code,
                f"source file hash does not match {expected_sha_field}: {path}",
                remediation,
                {"path": str(path), "expected": expected_sha, "actual": actual_sha},
            )
    return path, raw


def embedded_chunk_source_proof(row: dict[str, Any]) -> dict[str, Any]:
    text = row.get("source_text")
    if not isinstance(text, str):
        raise SourceReadbackError(
            "PY_SOURCE_READBACK_SOURCE_TEXT_MISSING",
            "chunk row source_text is missing or not a string",
            "Regenerate AST chunks from stable source bytes before using this row.",
            {"chunk_id": row.get("chunk_id"), "actual_type": type(text).__name__},
        )
    encoding = str(row.get("encoding") or "utf-8")
    try:
        source_bytes = text.encode(encoding)
    except UnicodeEncodeError as exc:
        raise SourceReadbackError(
            "PY_SOURCE_READBACK_SOURCE_TEXT_ENCODING_FAILED",
            "chunk row source_text cannot be encoded with its declared encoding",
            "Regenerate AST chunks from stable source bytes before using this row.",
            {"chunk_id": row.get("chunk_id"), "encoding": encoding, "error": str(exc)},
        ) from exc
    actual_sha = sha256_prefixed_bytes(source_bytes)
    expected_sha = str(row.get("source_sha256") or "")
    if actual_sha != expected_sha:
        raise SourceReadbackError(
            "PY_SOURCE_READBACK_SOURCE_TEXT_HASH_MISMATCH",
            "chunk row source_text hash does not match source_sha256",
            "Regenerate AST chunks from stable source bytes before using this row.",
            {"chunk_id": row.get("chunk_id"), "expected": expected_sha, "actual": actual_sha},
        )
    return {
        "source_mode": "durable_ast_chunk_jsonl_embedded_source",
        "durable_source_byte_len": len(source_bytes),
        "durable_source_sha256": actual_sha,
        "source_text_matches_declared_hash": True,
    }
