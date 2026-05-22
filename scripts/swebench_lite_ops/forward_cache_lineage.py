from __future__ import annotations

from typing import Any

from .source_readback import is_legacy_mounted_path


AIWONDER_CONTEXTGRAPH_ROOTS = ("/zfs/archive/contextgraph/", "/zfs/hot/contextgraph/")


class ForwardCacheLineageError(RuntimeError):
    def __init__(self, code: str, message: str, details: dict[str, Any] | None = None):
        super().__init__(f"{code}: {message}")
        self.code = code
        self.message = message
        self.details = details or {}


def row_key_from_fields(task_instance_id: str, workspace_state: str, chunk_id: str, source_sha256: str) -> str:
    return f"{task_instance_id}|{workspace_state}|{chunk_id}|{source_sha256}"


def _stable_key_from_parts(row: dict[str, Any], *, source: str) -> str:
    task_instance_id = str(row.get("task_instance_id") or "")
    workspace_state = str(row.get("workspace_state") or "")
    chunk_id = str(row.get("chunk_id") or "")
    source_sha256 = str(row.get("source_sha256") or "")
    if not (task_instance_id and workspace_state and chunk_id and source_sha256):
        raise ForwardCacheLineageError(
            "MEJEPA_FORWARD_CACHE_LINEAGE_STABLE_KEY_MISSING",
            f"{source} is missing one or more stable key fields",
            {
                "source": source,
                "task_instance_id_present": bool(task_instance_id),
                "workspace_state_present": bool(workspace_state),
                "chunk_id_present": bool(chunk_id),
                "source_sha256_present": bool(source_sha256),
            },
        )
    return row_key_from_fields(task_instance_id, workspace_state, chunk_id, source_sha256)


def packed_record_row_key(record: dict[str, Any]) -> str:
    key = record.get("key") or {}
    chunk = record.get("chunk") or {}
    return _stable_key_from_parts(
        {
            "task_instance_id": chunk.get("task_instance_id"),
            "workspace_state": chunk.get("workspace_state"),
            "chunk_id": key.get("chunk_id"),
            "source_sha256": key.get("source_sha256"),
        },
        source="packed_forward_cache_record",
    )


def canonical_chunk_row_key(chunk: dict[str, Any]) -> str:
    return _stable_key_from_parts(chunk, source="canonical_ast_chunk")


def _assert_active_path_is_canonical(
    *,
    field: str,
    value: object,
    context: str,
    require_aiwonder_root: bool,
) -> dict[str, Any]:
    text = str(value or "")
    if not text:
        return {"present": False, "legacy_mounted": False, "aiwonder_contextgraph_root": False}
    if is_legacy_mounted_path(text):
        raise ForwardCacheLineageError(
            "MEJEPA_FORWARD_CACHE_CANONICAL_LINEAGE_LEGACY_MOUNTED_PATH",
            f"canonical AST field {field} still points at a mounted drive",
            {"context": context, "field": field, "value": text},
        )
    rooted = text.startswith(AIWONDER_CONTEXTGRAPH_ROOTS)
    if require_aiwonder_root and text.startswith("/") and not rooted:
        raise ForwardCacheLineageError(
            "MEJEPA_FORWARD_CACHE_CANONICAL_LINEAGE_NOT_AIWONDER_ROOT",
            f"canonical AST field {field} is absolute but not under the aiwonder ContextGraph roots",
            {
                "context": context,
                "field": field,
                "value": text,
                "allowed_roots": list(AIWONDER_CONTEXTGRAPH_ROOTS),
            },
        )
    return {"present": True, "legacy_mounted": False, "aiwonder_contextgraph_root": rooted}


def guard_packed_record_lineage(
    record: dict[str, Any],
    canonical_chunk: dict[str, Any],
    *,
    context: str,
    require_aiwonder_root: bool = True,
) -> dict[str, Any]:
    """Validate that packed vectors are joined to canonical AST lineage before use.

    The 2026-05-15 forward cache preserves old WSL `/mnt/d` values inside the
    raw packed payload. Those values are admissible only as legacy provenance.
    Active consumers must derive source lineage from the #613 canonical AST row.
    """

    packed_key = packed_record_row_key(record)
    canonical_key = canonical_chunk_row_key(canonical_chunk)
    if packed_key != canonical_key:
        raise ForwardCacheLineageError(
            "MEJEPA_FORWARD_CACHE_LINEAGE_KEY_MISMATCH",
            "packed vector record does not match its canonical AST row",
            {"context": context, "packed_row_key": packed_key, "canonical_row_key": canonical_key},
        )

    packed_chunk = record.get("chunk") or {}
    raw_path = str(packed_chunk.get("absolute_path") or "")
    raw_dataschema = str(packed_chunk.get("dataschema") or "")
    raw_legacy_fields = [
        field
        for field, value in (("absolute_path", raw_path), ("dataschema", raw_dataschema))
        if is_legacy_mounted_path(value)
    ]

    canonical_fields = {
        "absolute_path": _assert_active_path_is_canonical(
            field="absolute_path",
            value=canonical_chunk.get("absolute_path"),
            context=context,
            require_aiwonder_root=require_aiwonder_root,
        ),
        "dataschema": _assert_active_path_is_canonical(
            field="dataschema",
            value=canonical_chunk.get("dataschema"),
            context=context,
            require_aiwonder_root=require_aiwonder_root,
        ),
    }
    if not canonical_fields["absolute_path"]["present"] and not str(canonical_chunk.get("relative_path") or ""):
        raise ForwardCacheLineageError(
            "MEJEPA_FORWARD_CACHE_CANONICAL_ABSOLUTE_PATH_MISSING",
            "canonical AST row is missing both absolute_path and relative_path",
            {"context": context, "canonical_row_key": canonical_key},
        )

    return {
        "packed_row_key": packed_key,
        "canonical_row_key": canonical_key,
        "canonical_ast_lineage_used": True,
        "raw_packed_path_metadata_legacy": bool(raw_legacy_fields),
        "raw_packed_legacy_fields": raw_legacy_fields,
        "canonical_fields": canonical_fields,
    }
