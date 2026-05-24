"""Shared helpers for forward-cache completeness mask materialization.

Split out of python_forward_cache_completeness_mask.py for #747 file-size
tech debt (doctrine §14.8 file ≤500 lines). Exports the error class, the
streaming JSONL sink, low-level JSON / row-key helpers, and the corpus /
links / chunks / shard loaders. Importing module composes these into the
mask materialization command.
"""

from __future__ import annotations

import hashlib
import json
import os
import re
from collections import Counter
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Iterable

from .core import SwebenchOpsError, sha256_file

CANDIDATE_ID_RE = re.compile(r"/source-files/([0-9a-f]{64})/")


class ForwardCompletenessError(SwebenchOpsError):
    def __init__(self, code: str, message: str, details: dict[str, Any] | None = None):
        super().__init__(
            code,
            message,
            "Use a passing aiwonder forward-cache root, candidate-keyed AST chunk rows, and quarantine records; do not infer missingness.",
            details,
        )


@dataclass
class JsonlSink:
    path: Path
    count: int = 0
    digest: hashlib._hashlib.HASH = field(default_factory=hashlib.sha256)
    _fh: Any = None
    _tmp: Path | None = None

    def __enter__(self) -> "JsonlSink":
        self.path.parent.mkdir(parents=True, exist_ok=True)
        self._tmp = self.path.with_suffix(self.path.suffix + ".tmp")
        self._fh = self._tmp.open("w", encoding="utf-8")
        return self

    def write(self, row: dict[str, Any]) -> None:
        payload = json.dumps(row, sort_keys=True, separators=(",", ":"))
        self._fh.write(payload)
        self._fh.write("\n")
        self.digest.update(payload.encode("utf-8"))
        self.digest.update(b"\n")
        self.count += 1

    def __exit__(self, exc_type: Any, exc: Any, tb: Any) -> None:
        if self._fh is not None:
            self._fh.flush()
            os.fsync(self._fh.fileno())
            self._fh.close()
        if exc_type is None and self._tmp is not None:
            self._tmp.replace(self.path)
        elif self._tmp is not None and self._tmp.exists():
            self._tmp.unlink()

    def artifact(self) -> dict[str, Any]:
        return {
            "path": str(self.path),
            "rows": self.count,
            "bytes": self.path.stat().st_size if self.path.exists() else 0,
            "sha256": "sha256:" + self.digest.hexdigest(),
        }


def require(condition: bool, code: str, message: str, details: dict[str, Any] | None = None) -> None:
    if not condition:
        raise ForwardCompletenessError(code, message, details)


def read_json(path: Path) -> dict[str, Any]:
    require(path.is_file(), "FORWARD_COMPLETENESS_INPUT_MISSING", str(path))
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise ForwardCompletenessError(
            "FORWARD_COMPLETENESS_BAD_JSON",
            f"{path}: {exc}",
            {"path": str(path), "error": str(exc)},
        ) from exc
    require(isinstance(value, dict), "FORWARD_COMPLETENESS_JSON_NOT_OBJECT", str(path))
    return value


def iter_jsonl(path: Path) -> Iterable[tuple[int, dict[str, Any]]]:
    require(path.is_file(), "FORWARD_COMPLETENESS_INPUT_MISSING", str(path))
    with path.open("r", encoding="utf-8") as handle:
        for line_no, line in enumerate(handle, start=1):
            if not line.strip():
                continue
            try:
                value = json.loads(line)
            except json.JSONDecodeError as exc:
                raise ForwardCompletenessError(
                    "FORWARD_COMPLETENESS_BAD_JSONL",
                    f"{path}:{line_no}: {exc}",
                    {"path": str(path), "line_number": line_no},
                ) from exc
            require(isinstance(value, dict), "FORWARD_COMPLETENESS_ROW_NOT_OBJECT", f"{path}:{line_no}")
            yield line_no, value


def parse_csv(raw: str | None) -> tuple[str, ...]:
    if raw is None or not raw.strip():
        return ()
    values = tuple(item.strip() for item in raw.split(",") if item.strip())
    require(len(values) == len(set(values)), "FORWARD_COMPLETENESS_DUPLICATE_EMBEDDER", str(values))
    return values


def row_key_from_fields(task_instance_id: str, workspace_state: str, chunk_id: str, source_sha256: str) -> str:
    return f"{task_instance_id}|{workspace_state}|{chunk_id}|{source_sha256}"


def canonical_row_key(row: dict[str, Any]) -> str:
    return row_key_from_fields(
        str(row.get("task_instance_id") or ""),
        str(row.get("workspace_state") or ""),
        str(row.get("chunk_id") or ""),
        str(row.get("source_sha256") or ""),
    )


def require_row_key(key: str, context: str) -> None:
    require("||" not in key and key.count("|") == 3, "FORWARD_COMPLETENESS_ROW_KEY_MISSING", context)


def candidate_id_from_chunk(row: dict[str, Any], context: str) -> str:
    for field_name in ("candidate_row_id",):
        value = row.get(field_name)
        if isinstance(value, str) and value.startswith("sha256:"):
            return value
    for field_name in ("absolute_path", "dataschema"):
        value = str(row.get(field_name) or "")
        match = CANDIDATE_ID_RE.search(value)
        if match:
            return "sha256:" + match.group(1)
    raise ForwardCompletenessError(
        "FORWARD_COMPLETENESS_CANDIDATE_ID_MISSING",
        f"candidate row id missing for {context}",
        {"context": context},
    )


def tree_state(path: Path) -> dict[str, Any]:
    if not path.exists():
        return {"path": str(path), "exists": False, "file_count": 0, "byte_count": 0}
    if path.is_file():
        return {"path": str(path), "exists": True, "file_count": 1, "byte_count": path.stat().st_size}
    file_count = 0
    byte_count = 0
    sample: list[str] = []
    for child in sorted(path.rglob("*")):
        if child.is_file():
            file_count += 1
            byte_count += child.stat().st_size
            if len(sample) < 20:
                sample.append(str(child.relative_to(path)))
    return {
        "path": str(path),
        "exists": True,
        "file_count": file_count,
        "byte_count": byte_count,
        "sample_files": sample,
        "sample_truncated": file_count > len(sample),
    }


def load_corpus_rows(path: Path) -> tuple[dict[str, dict[str, Any]], dict[str, Any]]:
    by_candidate: dict[str, dict[str, Any]] = {}
    for line_no, row in iter_jsonl(path):
        candidate_id = str(row.get("candidate_row_id") or "")
        require(candidate_id.startswith("sha256:"), "FORWARD_COMPLETENESS_BAD_CANDIDATE_ROW_ID", f"{path}:{line_no}")
        require(candidate_id not in by_candidate, "FORWARD_COMPLETENESS_DUPLICATE_CANDIDATE_ROW", candidate_id)
        by_candidate[candidate_id] = row
    require(by_candidate, "FORWARD_COMPLETENESS_CORPUS_ROWS_EMPTY", str(path))
    return by_candidate, {"path": str(path), "sha256": sha256_file(path), "rows": len(by_candidate)}


def load_links(path: Path) -> tuple[dict[tuple[str, str, str], dict[str, Any]], dict[str, Any]]:
    links: dict[tuple[str, str, str], dict[str, Any]] = {}
    for line_no, row in iter_jsonl(path):
        candidate_id = str(row.get("candidate_row_id") or "")
        chunk_id = str(row.get("chunk_id") or "")
        source_sha = str(row.get("chunk_source_sha256") or "")
        key = (candidate_id, chunk_id, source_sha)
        require(all(key), "FORWARD_COMPLETENESS_LINK_KEY_MISSING", f"{path}:{line_no}")
        require(key not in links, "FORWARD_COMPLETENESS_DUPLICATE_CHUNK_LINK", str(key))
        links[key] = row
    require(links, "FORWARD_COMPLETENESS_CHUNK_LINKS_EMPTY", str(path))
    return links, {"path": str(path), "sha256": sha256_file(path), "rows": len(links)}


def load_chunks(
    path: Path,
    links: dict[tuple[str, str, str], dict[str, Any]],
    corpus: dict[str, dict[str, Any]],
) -> tuple[dict[str, dict[str, Any]], dict[str, Any]]:
    chunks: dict[str, dict[str, Any]] = {}
    by_candidate = Counter()
    for line_no, row in iter_jsonl(path):
        key = canonical_row_key(row)
        require_row_key(key, f"{path}:{line_no}")
        require(key not in chunks, "FORWARD_COMPLETENESS_DUPLICATE_CHUNK_ROW_KEY", key)
        candidate_id = candidate_id_from_chunk(row, f"{path}:{line_no}")
        link = links.get((candidate_id, str(row.get("chunk_id") or ""), str(row.get("source_sha256") or "")))
        require(link is not None, "FORWARD_COMPLETENESS_CHUNK_LINK_MISSING", f"{path}:{line_no}:{key}")
        corpus_row = corpus.get(candidate_id)
        require(corpus_row is not None, "FORWARD_COMPLETENESS_CORPUS_ROW_MISSING", candidate_id)
        enriched = dict(row)
        enriched["_candidate_row_id"] = candidate_id
        enriched["_chunk_link"] = link
        enriched["_corpus_row"] = corpus_row
        chunks[key] = enriched
        by_candidate[candidate_id] += 1
    require(chunks, "FORWARD_COMPLETENESS_CHUNKS_EMPTY", str(path))
    return chunks, {
        "path": str(path),
        "sha256": sha256_file(path),
        "rows": len(chunks),
        "distinct_candidate_row_ids": len(by_candidate),
        "chunk_rows_by_candidate_min": min(by_candidate.values()),
        "chunk_rows_by_candidate_max": max(by_candidate.values()),
    }


def completed_shards(forward_root: Path) -> tuple[list[dict[str, Any]], dict[str, Any], dict[str, Any], dict[str, Any]]:
    checkpoint_path = forward_root / "python_forward_cache_checkpoint.json"
    plan_path = forward_root / "python_forward_cache_shard_plan.json"
    manifest_path = forward_root / "python_forward_cache_corpus_manifest.json"
    checkpoint = read_json(checkpoint_path)
    plan = read_json(plan_path)
    manifest = read_json(manifest_path)
    require(checkpoint.get("passes") is True, "FORWARD_COMPLETENESS_CHECKPOINT_NOT_PASSING", str(checkpoint_path))
    require(manifest.get("passes") is True, "FORWARD_COMPLETENESS_CORPUS_MANIFEST_NOT_PASSING", str(manifest_path))
    progress = checkpoint.get("progress") or {}
    completed = int(progress.get("completed_shards") or 0)
    total = int(progress.get("total_shards") or manifest.get("summary", {}).get("shard_count") or 0)
    require(completed == total and total > 0, "FORWARD_COMPLETENESS_SHARDS_NOT_COMPLETE", str(progress))
    require(progress.get("next_shard") is None, "FORWARD_COMPLETENESS_NEXT_SHARD_NOT_NULL", str(progress))
    shards = list(plan.get("shards") or [])
    require(len(shards) >= completed, "FORWARD_COMPLETENESS_SHARD_PLAN_TOO_SHORT", str(len(shards)))
    selected = shards[:completed]
    for shard in selected:
        shard_id = str(shard.get("shard_id") or "")
        require(shard_id, "FORWARD_COMPLETENESS_SHARD_ID_MISSING", str(shard))
        require(
            (forward_root / "shards" / shard_id / "python_forward_cache_packed_manifest.json").is_file(),
            "FORWARD_COMPLETENESS_PACKED_MANIFEST_MISSING",
            shard_id,
        )
    summary = {
        "checkpoint_path": str(checkpoint_path),
        "checkpoint_sha256": sha256_file(checkpoint_path),
        "shard_plan_path": str(plan_path),
        "shard_plan_sha256": sha256_file(plan_path),
        "corpus_manifest_path": str(manifest_path),
        "corpus_manifest_sha256": sha256_file(manifest_path),
        "completed_shards": completed,
        "total_shards": total,
        "progress": progress,
    }
    return selected, checkpoint, manifest, summary


def index_line_count(path: Path) -> int:
    return sum(1 for _line_no, _row in iter_jsonl(path))
