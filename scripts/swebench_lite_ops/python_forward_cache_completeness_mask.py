"""Materialize a candidate-keyed forward-cache slot-completeness mask."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
from collections import Counter, defaultdict
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Iterable

from .core import SwebenchOpsError, sha256_file, utc_now, write_json_checked


SCHEMA_VERSION = 1
ARTIFACT_KIND = "candidate_keyed_forward_cache_slot_completeness"
MASK_POLICY = "candidate_keyed_slot_identity_preserved_missingness_mask_v1"
DEFAULT_CANDIDATE_CORPUS_ROOT = Path(
    "/zfs/archive/contextgraph/state/regeneration-plans/"
    "task-py-g-135c-candidate-keyed-corpus-v1/run-20260523T012459Z-current52"
)
DEFAULT_FORWARD_ROOT = Path(
    "/zfs/archive/contextgraph/state/regeneration-plans/"
    "task-py-g-135c-candidate-keyed-forward-cache-v1/run-20260524T044119Z-current52"
)
DEFAULT_OUTPUT_ROOT = Path(
    "/zfs/archive/contextgraph/state/forward-cache-completeness/"
    "task-py-g-135c-candidate-keyed-forward-cache-slot-completion-v1"
)
DEFAULT_READBACK_ROOT = Path(
    "/zfs/archive/contextgraph/fsv/"
    "task-py-g-135c-candidate-keyed-forward-cache-slot-completion-fsv/manual-readback"
)
STATIC_AST_ACTIVE_EMBEDDERS = ("e1", "e10", "e12", "e13", "e14", "e6", "e7", "e8", "e9")
RETIRED_STATIC_AST_EMBEDDERS = ("e2", "e3", "e4")
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
    for field in ("candidate_row_id",):
        value = row.get(field)
        if isinstance(value, str) and value.startswith("sha256:"):
            return value
    for field in ("absolute_path", "dataschema"):
        value = str(row.get(field) or "")
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


def pair_key_from_record(row: dict[str, Any], embedder_id: str) -> tuple[str, str]:
    key = row.get("key") or {}
    chunk = row.get("chunk") or {}
    actual_embedder = str(key.get("embedder_id") or row.get("embedder_id") or embedder_id)
    return (
        row_key_from_fields(
            str(chunk.get("task_instance_id") or row.get("task_instance_id") or ""),
            str(chunk.get("workspace_state") or row.get("workspace_state") or ""),
            str(key.get("chunk_id") or row.get("chunk_id") or ""),
            str(key.get("source_sha256") or row.get("source_sha256") or ""),
        ),
        actual_embedder,
    )


def pair_key_from_quarantine(row: dict[str, Any]) -> tuple[str, str]:
    return (
        row_key_from_fields(
            str(row.get("task_instance_id") or ""),
            str(row.get("workspace_state") or ""),
            str(row.get("chunk_id") or ""),
            str(row.get("source_sha256") or ""),
        ),
        str(row.get("embedder_id") or ""),
    )


def load_forward_pairs(
    forward_root: Path,
    shards: list[dict[str, Any]],
    chunks: dict[str, dict[str, Any]],
    active: tuple[str, ...],
) -> tuple[dict[str, set[str]], dict[str, dict[str, dict[str, Any]]], dict[str, Any]]:
    present: dict[str, set[str]] = defaultdict(set)
    quarantines: dict[str, dict[str, dict[str, Any]]] = defaultdict(dict)
    per_slot_present = Counter()
    per_slot_quarantine = Counter()
    packed_inputs: list[dict[str, Any]] = []
    active_set = set(active)
    record_rows = 0
    quarantine_rows = 0

    for shard in shards:
        shard_id = str(shard.get("shard_id") or "")
        shard_root = forward_root / "shards" / shard_id
        packed_manifest_path = shard_root / "python_forward_cache_packed_manifest.json"
        packed_manifest = read_json(packed_manifest_path)
        require(packed_manifest.get("passes") is True, "FORWARD_COMPLETENESS_PACKED_MANIFEST_NOT_PASSING", shard_id)
        per_embedder = packed_manifest.get("per_embedder") or {}
        require(set(per_embedder) == active_set, "FORWARD_COMPLETENESS_PACKED_ACTIVE_SET_MISMATCH", str(sorted(per_embedder)))
        packed_inputs.append({"shard_id": shard_id, "packed_manifest_path": str(packed_manifest_path), "sha256": sha256_file(packed_manifest_path)})
        for embedder_id in active:
            entry = per_embedder[embedder_id]
            record_path = Path(entry["record_path"])
            index_path = Path(entry["index_path"])
            record_count = 0
            for line_no, row in iter_jsonl(record_path):
                row_key, actual_embedder = pair_key_from_record(row, embedder_id)
                require(actual_embedder == embedder_id, "FORWARD_COMPLETENESS_RECORD_EMBEDDER_MISMATCH", f"{record_path}:{line_no}")
                require_row_key(row_key, f"{record_path}:{line_no}")
                require(row_key in chunks, "FORWARD_COMPLETENESS_RECORD_CHUNK_JOIN_MISSING", f"{record_path}:{line_no}:{row_key}")
                candidate_id = str((row.get("chunk") or {}).get("candidate_row_id") or "")
                expected_candidate = str(chunks[row_key].get("_candidate_row_id") or "")
                require(candidate_id == expected_candidate, "FORWARD_COMPLETENESS_RECORD_CANDIDATE_MISMATCH", f"{record_path}:{line_no}:{row_key}")
                require(embedder_id not in present[row_key], "FORWARD_COMPLETENESS_DUPLICATE_PRESENT_PAIR", f"{row_key}:{embedder_id}")
                present[row_key].add(embedder_id)
                per_slot_present[embedder_id] += 1
                record_count += 1
            index_count = index_line_count(index_path)
            require(record_count == index_count, "FORWARD_COMPLETENESS_INDEX_RECORD_COUNT_MISMATCH", f"{embedder_id}:{record_count}!={index_count}")
            require(record_count == int(entry.get("row_count") or -1), "FORWARD_COMPLETENESS_PACKED_ROW_COUNT_MISMATCH", f"{embedder_id}:{record_count}!={entry.get('row_count')}")
            record_rows += record_count

        quarantine_path = shard_root / "python_forward_cache_quarantine.jsonl"
        require(quarantine_path.is_file(), "FORWARD_COMPLETENESS_QUARANTINE_MISSING", str(quarantine_path))
        for line_no, row in iter_jsonl(quarantine_path):
            row_key, embedder_id = pair_key_from_quarantine(row)
            require(embedder_id in active_set, "FORWARD_COMPLETENESS_QUARANTINE_UNKNOWN_EMBEDDER", f"{quarantine_path}:{line_no}:{embedder_id}")
            require_row_key(row_key, f"{quarantine_path}:{line_no}")
            require(row_key in chunks, "FORWARD_COMPLETENESS_QUARANTINE_CHUNK_JOIN_MISSING", f"{quarantine_path}:{line_no}:{row_key}")
            candidate_id = str(row.get("candidate_row_id") or "")
            expected_candidate = str(chunks[row_key].get("_candidate_row_id") or "")
            require(candidate_id == expected_candidate, "FORWARD_COMPLETENESS_QUARANTINE_CANDIDATE_MISMATCH", f"{quarantine_path}:{line_no}:{row_key}")
            require(embedder_id not in quarantines[row_key], "FORWARD_COMPLETENESS_DUPLICATE_QUARANTINE_PAIR", f"{row_key}:{embedder_id}")
            quarantines[row_key][embedder_id] = {
                "error_code": row.get("error_code"),
                "reason": row.get("reason"),
                "instrument_id": row.get("instrument_id"),
                "source_text_bytes": row.get("source_text_bytes"),
                "max_forward_source_text_bytes": row.get("max_forward_source_text_bytes"),
                "candidate_row_id": candidate_id,
                "shard_id": shard_id,
            }
            per_slot_quarantine[embedder_id] += 1
            quarantine_rows += 1

    overlap = [(key, slot) for key, slots in present.items() for slot in slots if slot in quarantines.get(key, {})]
    require(not overlap, "FORWARD_COMPLETENESS_PRESENT_AND_QUARANTINED_PAIR", str(overlap[:20]))
    return present, quarantines, {
        "packed_inputs": packed_inputs,
        "record_rows": record_rows,
        "quarantine_rows": quarantine_rows,
        "present_by_embedder": dict(sorted(per_slot_present.items())),
        "quarantine_by_embedder": dict(sorted(per_slot_quarantine.items())),
    }


def mask_row(
    row_key: str,
    chunk: dict[str, Any],
    active: tuple[str, ...],
    retired: tuple[str, ...],
    present_slots: set[str],
    quarantine_reasons: dict[str, dict[str, Any]],
) -> dict[str, Any]:
    candidate_id = str(chunk["_candidate_row_id"])
    corpus_row = chunk["_corpus_row"]
    link = chunk["_chunk_link"]
    present = tuple(sorted(present_slots))
    missing = tuple(slot for slot in active if slot not in present_slots)
    missing_reasons = {slot: quarantine_reasons[slot] for slot in missing if slot in quarantine_reasons}
    require(len(missing_reasons) == len(missing), "FORWARD_COMPLETENESS_UNACCOUNTED_MISSING_PAIR", row_key)
    if not missing:
        status = "complete"
    elif not present:
        status = "zero_slot"
    else:
        status = "partial"
    return {
        "artifact_kind": "candidate_keyed_forward_cache_slot_completeness_mask_row",
        "schema_version": SCHEMA_VERSION,
        "mask_policy_version": MASK_POLICY,
        "row_key": row_key,
        "task_instance_id": chunk.get("task_instance_id"),
        "workspace_state": chunk.get("workspace_state"),
        "chunk_id": chunk.get("chunk_id"),
        "source_sha256": chunk.get("source_sha256"),
        "file_sha256": chunk.get("file_sha256"),
        "repo": chunk.get("repo"),
        "relative_path": chunk.get("relative_path"),
        "mutation_category": chunk.get("mutation_category"),
        "partition": corpus_row.get("stable_partition"),
        "chunk_kind": chunk.get("chunk_kind"),
        "node_kind": chunk.get("node_kind"),
        "line_start": chunk.get("line_start"),
        "line_end": chunk.get("line_end"),
        "byte_start": chunk.get("byte_start"),
        "byte_end": chunk.get("byte_end"),
        "source_text_bytes": chunk.get("source_text_bytes") or len(str(chunk.get("source_text") or "").encode("utf-8")),
        "candidate_row_id": candidate_id,
        "candidate_corpus_row_id": corpus_row.get("candidate_corpus_row_id") or link.get("candidate_corpus_row_id"),
        "source_task_id": corpus_row.get("source_task_id") or link.get("source_task_id"),
        "assertion_duty_sublabel": corpus_row.get("assertion_duty_sublabel"),
        "target_oracle_labels_included": False,
        "target_oracle_labels_used_as_features": False,
        "expected_slots": list(active),
        "retired_slots": list(retired),
        "present_slots": list(present),
        "missing_slots": list(missing),
        "present_slot_count": len(present),
        "missing_slot_count": len(missing),
        "row_status": status,
        "complete_static_ast_9slot_row": status == "complete",
        "missing_slot_reasons": missing_reasons,
    }


def summarize_mask(rows: list[dict[str, Any]], active: tuple[str, ...]) -> dict[str, Any]:
    status_counts = Counter(str(row["row_status"]) for row in rows)
    present_by_slot = Counter()
    missing_by_slot = Counter()
    partition_sublabel_status = Counter()
    candidate_ids = set()
    for row in rows:
        candidate_ids.add(str(row["candidate_row_id"]))
        for slot in row["present_slots"]:
            present_by_slot[str(slot)] += 1
        for slot in row["missing_slots"]:
            missing_by_slot[str(slot)] += 1
        partition_sublabel_status[
            f"{row.get('partition') or 'unknown'}|{row.get('assertion_duty_sublabel') or 'unknown'}|{row['row_status']}"
        ] += 1
    present_pairs = sum(present_by_slot.values())
    missing_pairs = sum(missing_by_slot.values())
    return {
        "mask_rows": len(rows),
        "distinct_candidate_row_ids": len(candidate_ids),
        "expected_slots": list(active),
        "expected_pairs": len(rows) * len(active),
        "present_pairs": present_pairs,
        "missing_pairs": missing_pairs,
        "accounted_pairs": present_pairs + missing_pairs,
        "complete_rows": status_counts.get("complete", 0),
        "partial_rows": status_counts.get("partial", 0),
        "zero_slot_rows": status_counts.get("zero_slot", 0),
        "row_status_counts": dict(sorted(status_counts.items())),
        "present_by_embedder": dict(sorted(present_by_slot.items())),
        "missing_by_embedder": dict(sorted(missing_by_slot.items())),
        "partition_sublabel_status_counts": dict(sorted(partition_sublabel_status.items())),
    }


def readback_mask(mask_path: Path, manifest_path: Path) -> dict[str, Any]:
    manifest = read_json(manifest_path)
    status_counts = Counter()
    present_pairs = 0
    missing_pairs = 0
    candidate_ids = set()
    rows = 0
    for _line_no, row in iter_jsonl(mask_path):
        rows += 1
        status_counts[str(row.get("row_status") or "unknown")] += 1
        present_pairs += len(row.get("present_slots") or [])
        missing_pairs += len(row.get("missing_slots") or [])
        candidate_ids.add(str(row.get("candidate_row_id") or ""))
    return {
        "artifact_kind": "candidate_keyed_forward_cache_slot_completeness_readback",
        "schema_version": SCHEMA_VERSION,
        "created_at": utc_now(),
        "passes": True,
        "manifest_path": str(manifest_path),
        "manifest_sha256": sha256_file(manifest_path),
        "mask_path": str(mask_path),
        "mask_sha256": sha256_file(mask_path),
        "manifest_artifact_kind": manifest.get("artifact_kind"),
        "mask_rows": rows,
        "distinct_candidate_row_ids": len(candidate_ids - {""}),
        "present_pairs": present_pairs,
        "missing_pairs": missing_pairs,
        "accounted_pairs": present_pairs + missing_pairs,
        "row_status_counts": dict(sorted(status_counts.items())),
    }


def command_python_forward_cache_completeness_mask(args: argparse.Namespace) -> dict[str, Any]:
    started_at = utc_now()
    output_root = args.output_root
    before = {"output_root": tree_state(output_root), "readback_root": tree_state(args.readback_root)}
    if output_root.exists() and not args.clean_output:
        raise ForwardCompletenessError(
            "FORWARD_COMPLETENESS_OUTPUT_EXISTS",
            f"output root already exists: {output_root}",
            {"output_root": str(output_root), "use_clean_output": True},
        )
    work_root = output_root.with_name(f"{output_root.name}.tmp-{os.getpid()}")
    if work_root.exists():
        shutil.rmtree(work_root)
    if output_root.exists() and args.clean_output:
        shutil.rmtree(output_root)
    work_root.mkdir(parents=True)

    try:
        corpus, corpus_summary = load_corpus_rows(args.corpus_rows_jsonl)
        links, link_summary = load_links(args.chunk_links_jsonl)
        chunks, chunk_summary = load_chunks(args.chunks_jsonl, links, corpus)
        shards, _checkpoint, corpus_manifest, shard_summary = completed_shards(args.forward_root)
        manifest_summary = corpus_manifest.get("summary") or {}
        active = parse_csv(args.active_embedders) or tuple(str(item) for item in manifest_summary.get("active_embedder_ids") or [])
        retired = tuple(str(item) for item in manifest_summary.get("retired_embedder_ids") or RETIRED_STATIC_AST_EMBEDDERS)
        require(active, "FORWARD_COMPLETENESS_ACTIVE_EMBEDDERS_EMPTY", "active embedder list is empty")
        require(set(active) == set(STATIC_AST_ACTIVE_EMBEDDERS), "FORWARD_COMPLETENESS_STATIC_ACTIVE_SET_MISMATCH", str(active))
        require(set(retired) == set(RETIRED_STATIC_AST_EMBEDDERS), "FORWARD_COMPLETENESS_RETIRED_SET_MISMATCH", str(retired))
        require(int(manifest_summary.get("chunk_count") or -1) == len(chunks), "FORWARD_COMPLETENESS_MANIFEST_CHUNK_COUNT_MISMATCH", str(manifest_summary))
        present, quarantines, forward_summary = load_forward_pairs(args.forward_root, shards, chunks, active)

        rows: list[dict[str, Any]] = []
        for row_key, chunk in sorted(chunks.items()):
            rows.append(mask_row(row_key, chunk, active, retired, present.get(row_key, set()), quarantines.get(row_key, {})))
        summary = summarize_mask(rows, active)
        require(summary["expected_pairs"] == summary["accounted_pairs"], "FORWARD_COMPLETENESS_ACCOUNTING_MISMATCH", str(summary))
        require(summary["present_pairs"] == forward_summary["record_rows"], "FORWARD_COMPLETENESS_RECORD_ACCOUNTING_MISMATCH", str(summary))
        require(summary["missing_pairs"] == forward_summary["quarantine_rows"], "FORWARD_COMPLETENESS_QUARANTINE_ACCOUNTING_MISMATCH", str(summary))
        require(summary["present_pairs"] == int(manifest_summary.get("cache_record_count") or -1), "FORWARD_COMPLETENESS_MANIFEST_CACHE_COUNT_MISMATCH", str(manifest_summary))
        require(summary["missing_pairs"] == int(manifest_summary.get("quarantine_row_count") or -1), "FORWARD_COMPLETENESS_MANIFEST_QUARANTINE_COUNT_MISMATCH", str(manifest_summary))

        mask_path = work_root / "forward_cache_slot_completeness_mask.jsonl"
        with JsonlSink(mask_path) as sink:
            for row in rows:
                sink.write(row)
        mask_artifact = sink.artifact()
        manifest = {
            "artifact_kind": "candidate_keyed_forward_cache_slot_completeness_manifest",
            "schema_version": SCHEMA_VERSION,
            "mask_policy_version": MASK_POLICY,
            "created_at": utc_now(),
            "started_at": started_at,
            "passes": True,
            "candidate_row_id_keyed": True,
            "category_collapse_prevented": summary["distinct_candidate_row_ids"] == len(corpus),
            "target_oracle_labels_included": False,
            "target_oracle_labels_used_as_features": False,
            "forward_root": str(args.forward_root),
            "chunks_jsonl": str(args.chunks_jsonl),
            "chunk_links_jsonl": str(args.chunk_links_jsonl),
            "corpus_rows_jsonl": str(args.corpus_rows_jsonl),
            "mask_artifact": mask_artifact,
            "summary": summary,
            "inputs": {
                "corpus_rows": corpus_summary,
                "chunk_links": link_summary,
                "chunks": chunk_summary,
                "shards": shard_summary,
                "forward_pairs": forward_summary,
            },
        }
        manifest_path = work_root / "candidate_keyed_forward_cache_slot_completeness_manifest.json"
        write_json_checked(manifest_path, manifest)
        output_root.parent.mkdir(parents=True, exist_ok=True)
        work_root.replace(output_root)

        readback_root = args.readback_root
        if readback_root.exists() and args.clean_output:
            shutil.rmtree(readback_root)
        readback_root.mkdir(parents=True, exist_ok=True)
        readback = readback_mask(
            output_root / "forward_cache_slot_completeness_mask.jsonl",
            output_root / "candidate_keyed_forward_cache_slot_completeness_manifest.json",
        )
        readback["expected_summary"] = summary
        readback_path = readback_root / "candidate_keyed_forward_cache_slot_completeness_readback.json"
        write_json_checked(readback_path, readback)
        after = {"output_root": tree_state(output_root), "readback_root": tree_state(readback_root)}
        return {
            "command": "python-forward-cache-completeness-mask",
            "all_passed": True,
            "started_at": started_at,
            "completed_at": utc_now(),
            "before_state": before,
            "after_state": after,
            "output_root": str(output_root),
            "readback_root": str(readback_root),
            "manifest_path": str(output_root / "candidate_keyed_forward_cache_slot_completeness_manifest.json"),
            "mask_path": str(output_root / "forward_cache_slot_completeness_mask.jsonl"),
            "readback_path": str(readback_path),
            "summary": summary,
            "readback": readback,
        }
    except Exception:
        if work_root.exists():
            shutil.rmtree(work_root)
        raise


def add_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--forward-root", type=Path, default=DEFAULT_FORWARD_ROOT)
    parser.add_argument("--chunks-jsonl", type=Path, default=DEFAULT_CANDIDATE_CORPUS_ROOT / "candidate_keyed_ast_chunks.jsonl")
    parser.add_argument("--chunk-links-jsonl", type=Path, default=DEFAULT_CANDIDATE_CORPUS_ROOT / "candidate_keyed_ast_chunk_links.jsonl")
    parser.add_argument("--corpus-rows-jsonl", type=Path, default=DEFAULT_CANDIDATE_CORPUS_ROOT / "candidate_keyed_corpus_rows.jsonl")
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--readback-root", "--fsv-root", dest="readback_root", type=Path, default=DEFAULT_READBACK_ROOT)
    parser.add_argument("--evidence-root", type=Path, default=DEFAULT_READBACK_ROOT)
    parser.add_argument("--active-embedders", default=",".join(STATIC_AST_ACTIVE_EMBEDDERS))
    parser.add_argument("--clean-output", action="store_true")
