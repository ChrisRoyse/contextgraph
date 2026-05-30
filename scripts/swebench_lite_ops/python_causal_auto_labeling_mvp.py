#!/usr/bin/env python3
"""TASK-CR-006 causal auto-labeling MVP runner.

This is the bounded first slice for #744. It ingests the Python shard of
CommitPackFT, materializes before/after source bytes, reuses the existing AST
parse/chunk primitives, and builds a directional transition tensor over
non-target AST-surface labels. The full issue still needs per-embedder forward
cache execution; this runner establishes the aiwonder Source-of-Truth shape,
leakage fence, filtering policy, transition math, and edge-case behavior before
launching that larger GPU job.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import re
import shutil
import sys
import tempfile
from collections import Counter, defaultdict
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path, PurePosixPath
from typing import Any, Iterable

from .python_ast_chunks import chunk_schema, chunks_for_record
from .python_ast_parse import parse_file, record_schema


TASK_ID = "TASK-CR-006"
ARTIFACT_KIND = "causal_auto_labeling_mvp"
SCHEMA_VERSION = 1

DEFAULT_INPUT_JSONL = Path(
    "/zfs/hot/contextgraph/.cache/huggingface/hub/"
    "datasets--bigcode--commitpackft/snapshots/fc56fe33c030c6daa414c2b112c932b8eed085e6/"
    "data/python/data.jsonl"
)
DEFAULT_CORPUS_PARENT = Path("/zfs/archive/contextgraph/corpus")
DEFAULT_STATE_ROOT = Path("/zfs/archive/contextgraph/state/cross-cluster-transitions/task-cr-006-v1")
DEFAULT_FSV_PARENT = Path("/zfs/archive/contextgraph/fsv/task-cr-006-causal-auto-labeling-mvp-fsv")

BUGFIX_RE = re.compile(
    r"\b(fix(?:e[sd])?|bug(?:s)?|defect|patch|repair(?:ed|s)?|resolve(?:d|s)?|"
    r"crash(?:es|ed)?|error(?:s)?|regression|issue\s*#?\d+)\b",
    re.IGNORECASE,
)
EXCLUDE_MESSAGE_RE = re.compile(
    r"\b(refactor(?:ing|ed)?|format(?:ting|ted)?|style|typo|docs?|documentation|"
    r"readme|license|changelog|whitespace|black|pep8|lint(?:ing|ed)?)\b",
    re.IGNORECASE,
)
DOC_PATH_RE = re.compile(r"(^|/)(docs?|documentation|readme|changelog|license)(/|\.|$)", re.IGNORECASE)
TOKEN_RE = re.compile(r"[A-Za-z_][A-Za-z0-9_]{1,40}")
TARGET_LEAK_LABEL_PREFIXES = (
    "bug_fix",
    "non_bug_fix",
    "oracle",
    "docker",
    "ground_truth",
    "target",
    "label:",
)


class CausalAutoLabelError(RuntimeError):
    pass


@dataclass
class PairRecord:
    pair_id: str
    source_index: int
    commit: str
    repo: str
    license: str
    message: str
    old_file: str
    new_file: str
    bug_fix_label: bool
    split: str
    old_sha256: str
    new_sha256: str


@dataclass
class PairLabels:
    pair: PairRecord
    before_labels: set[str] = field(default_factory=set)
    after_labels: set[str] = field(default_factory=set)
    before_chunk_count: int = 0
    after_chunk_count: int = 0
    syntax_error_chunks: int = 0


def utc_stamp() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")


def utc_now() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def canonical_json(value: Any) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def sha256_bytes(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def sha256_prefixed(payload: bytes) -> str:
    return "sha256:" + sha256_bytes(payload)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    tmp.replace(path)


def append_jsonl(path: Path, rows: Iterable[dict[str, Any]]) -> int:
    path.parent.mkdir(parents=True, exist_ok=True)
    count = 0
    with path.open("w", encoding="utf-8") as handle:
        for row in rows:
            handle.write(canonical_json(row) + "\n")
            count += 1
    return count


def iter_jsonl(path: Path, limit: int | None) -> Iterable[tuple[int, dict[str, Any]]]:
    with path.open("r", encoding="utf-8") as handle:
        for index, line in enumerate(handle, start=1):
            if limit is not None and index > limit:
                break
            if not line.strip():
                continue
            try:
                value = json.loads(line)
            except json.JSONDecodeError as exc:
                raise CausalAutoLabelError(f"CAUSAL_LABEL_INPUT_JSON_INVALID at line {index}: {exc}") from exc
            if not isinstance(value, dict):
                raise CausalAutoLabelError(f"CAUSAL_LABEL_INPUT_ROW_NOT_OBJECT at line {index}")
            yield index, value


def safe_component(value: str, fallback: str = "unknown") -> str:
    cleaned = "".join(ch if ch.isalnum() or ch in {"-", "_", "."} else "_" for ch in value)
    cleaned = cleaned.strip("._-")
    return cleaned[:120] or fallback


def safe_relative_path(value: str) -> str | None:
    if not isinstance(value, str) or not value.strip():
        return None
    normalized = value.replace("\\", "/").strip()
    path = PurePosixPath(normalized)
    if path.is_absolute() or ".." in path.parts or not normalized.endswith(".py"):
        return None
    return path.as_posix()


def primary_repo(raw: str) -> str:
    if not raw:
        return "unknown"
    return raw.split(",", 1)[0].strip() or "unknown"


def split_for_pair(pair_id: str) -> str:
    bucket = int(hashlib.sha256(pair_id.encode("utf-8")).hexdigest()[:8], 16) % 10_000
    if bucket < 7000:
        return "train"
    if bucket < 8500:
        return "validation"
    return "test"


def bugfix_label(message: str, subject: str) -> bool:
    text = f"{subject}\n{message}"
    return bool(BUGFIX_RE.search(text))


def excluded_reason(row: dict[str, Any], old_file: str, new_file: str, message: str, subject: str, label: bool) -> str | None:
    if DOC_PATH_RE.search(old_file) and DOC_PATH_RE.search(new_file):
        return "doc_path_only"
    text = f"{subject}\n{message}"
    if EXCLUDE_MESSAGE_RE.search(text) and not label:
        return "message_blocklisted_non_bugfix"
    return None


def load_pairs(input_jsonl: Path, *, input_limit: int | None, max_pairs: int | None, max_source_bytes: int) -> tuple[list[tuple[PairRecord, bytes, bytes]], dict[str, Any]]:
    if not input_jsonl.is_file():
        raise CausalAutoLabelError(f"CAUSAL_LABEL_INPUT_JSONL_MISSING: {input_jsonl}")
    accepted: list[tuple[PairRecord, bytes, bytes]] = []
    rejection_counts: Counter[str] = Counter()
    label_counts: Counter[str] = Counter()
    split_counts: Counter[str] = Counter()
    repo_counts: Counter[str] = Counter()
    scanned = 0
    for source_index, row in iter_jsonl(input_jsonl, input_limit):
        scanned += 1
        if str(row.get("lang") or "").lower() != "python":
            rejection_counts["non_python_lang"] += 1
            continue
        old_file = safe_relative_path(str(row.get("old_file") or ""))
        new_file = safe_relative_path(str(row.get("new_file") or ""))
        if old_file is None or new_file is None:
            rejection_counts["non_python_or_unsafe_path"] += 1
            continue
        old_contents = row.get("old_contents")
        new_contents = row.get("new_contents")
        if not isinstance(old_contents, str) or not isinstance(new_contents, str):
            rejection_counts["missing_source_text"] += 1
            continue
        old_bytes = old_contents.encode("utf-8", errors="surrogatepass")
        new_bytes = new_contents.encode("utf-8", errors="surrogatepass")
        if len(old_bytes) == 0 or len(new_bytes) == 0:
            rejection_counts["empty_source"] += 1
            continue
        if len(old_bytes) > max_source_bytes or len(new_bytes) > max_source_bytes:
            rejection_counts["source_too_large"] += 1
            continue
        if old_bytes == new_bytes:
            rejection_counts["unchanged_source"] += 1
            continue
        message = str(row.get("message") or "")
        subject = str(row.get("subject") or "")
        label = bugfix_label(message, subject)
        reason = excluded_reason(row, old_file, new_file, message, subject, label)
        if reason is not None:
            rejection_counts[reason] += 1
            continue
        identity = {
            "commit": str(row.get("commit") or ""),
            "old_file": old_file,
            "new_file": new_file,
            "old_sha256": sha256_prefixed(old_bytes),
            "new_sha256": sha256_prefixed(new_bytes),
        }
        pair_id = "commitpackft-" + sha256_bytes(canonical_json(identity).encode("utf-8"))[:24]
        split = split_for_pair(pair_id)
        pair = PairRecord(
            pair_id=pair_id,
            source_index=source_index,
            commit=str(row.get("commit") or ""),
            repo=primary_repo(str(row.get("repos") or "")),
            license=str(row.get("license") or ""),
            message=message,
            old_file=old_file,
            new_file=new_file,
            bug_fix_label=label,
            split=split,
            old_sha256=identity["old_sha256"],
            new_sha256=identity["new_sha256"],
        )
        accepted.append((pair, old_bytes, new_bytes))
        label_counts["bug_fix" if label else "non_bug_fix"] += 1
        split_counts[split] += 1
        repo_counts[pair.repo] += 1
        if max_pairs is not None and len(accepted) >= max_pairs:
            break
    if not accepted:
        raise CausalAutoLabelError("CAUSAL_LABEL_NO_ACCEPTED_PAIRS")
    return accepted, {
        "input_jsonl": str(input_jsonl),
        "input_sha256": sha256_file(input_jsonl),
        "input_size_bytes": input_jsonl.stat().st_size,
        "rows_scanned": scanned,
        "accepted_pairs": len(accepted),
        "rejection_counts": dict(sorted(rejection_counts.items())),
        "label_counts": dict(sorted(label_counts.items())),
        "split_counts": dict(sorted(split_counts.items())),
        "top_repos": dict(repo_counts.most_common(12)),
        "max_source_bytes": max_source_bytes,
    }


def write_source(path: Path, payload: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(payload)
    if path.read_bytes() != payload:
        raise CausalAutoLabelError(f"CAUSAL_LABEL_SOURCE_READBACK_MISMATCH: {path}")


def workspace_for(pair: PairRecord, state: str, root: Path) -> dict[str, Any]:
    state_key = "commit_before" if state == "before" else "commit_after"
    source_hash = pair.old_sha256 if state == "before" else pair.new_sha256
    tree_hash = hashlib.sha1(f"{pair.pair_id}:{state_key}:{source_hash}".encode("utf-8")).hexdigest()
    return {
        "task_id": pair.pair_id,
        "state": state_key,
        "result_tree_hash": tree_hash,
        "mutation_category": "bug_fix" if pair.bug_fix_label else "non_bug_fix",
        "repo": pair.repo,
        "base_commit": pair.commit or "unknown",
        "hot_workspace_path": str(root),
    }


def materialize_chunks(
    pairs: list[tuple[PairRecord, bytes, bytes]],
    *,
    corpus_root: Path,
    schema_root: Path,
) -> tuple[list[PairLabels], dict[str, Any]]:
    parse_schema_path = schema_root / "python_ast_parse_schema.json"
    chunk_schema_path = schema_root / "python_ast_chunk_schema.json"
    write_json(parse_schema_path, record_schema(parse_schema_path))
    write_json(chunk_schema_path, chunk_schema(chunk_schema_path))

    parse_records: list[dict[str, Any]] = []
    chunks: list[dict[str, Any]] = []
    diagnostics: list[dict[str, Any]] = []
    chunk_error_rows: list[dict[str, Any]] = []
    pair_labels: list[PairLabels] = []
    now = utc_now()
    source_root = corpus_root / "source-files"

    for pair, old_bytes, new_bytes in pairs:
        pair_label = PairLabels(pair=pair)
        pair_parse_records: list[dict[str, Any]] = []
        pair_chunks: list[dict[str, Any]] = []
        pair_diagnostics: list[dict[str, Any]] = []
        pair_failed = False
        for state, rel_path, payload in (("before", pair.old_file, old_bytes), ("after", pair.new_file, new_bytes)):
            state_root = source_root / pair.pair_id / state
            source_path = state_root / rel_path
            write_source(source_path, payload)
            workspace = workspace_for(pair, state, state_root)
            record = parse_file(parse_schema_path, workspace, source_path, now)
            try:
                record_chunks, record_diagnostics = chunks_for_record(record, chunk_schema_path, now)
            except Exception as exc:
                chunk_error_rows.append(
                    {
                        "schema_version": SCHEMA_VERSION,
                        "artifact_kind": "causal_pair_chunk_error",
                        "pair_id": pair.pair_id,
                        "state": state,
                        "relative_path": rel_path,
                        "error_type": type(exc).__name__,
                        "error": str(exc),
                    }
                )
                pair_failed = True
                break
            pair_parse_records.append(record)
            pair_chunks.extend(record_chunks)
            pair_diagnostics.extend(record_diagnostics)
            label_set: set[str] = set()
            for chunk in record_chunks:
                label_set.update(chunk_surface_labels(chunk))
                if chunk.get("chunk_kind") == "syntax_error_file":
                    pair_label.syntax_error_chunks += 1
            if state == "before":
                pair_label.before_labels = label_set
                pair_label.before_chunk_count = len(record_chunks)
            else:
                pair_label.after_labels = label_set
                pair_label.after_chunk_count = len(record_chunks)
        if pair_failed:
            continue
        parse_records.extend(pair_parse_records)
        chunks.extend(pair_chunks)
        diagnostics.extend(pair_diagnostics)
        pair_labels.append(pair_label)

    parse_path = corpus_root / "python_ast_parse_records.jsonl"
    chunks_path = corpus_root / "python_ast_chunks.jsonl"
    diagnostics_path = corpus_root / "python_ast_chunk_diagnostics.jsonl"
    chunk_errors_path = corpus_root / "chunk_error_pairs.jsonl"
    append_jsonl(parse_path, parse_records)
    append_jsonl(chunks_path, chunks)
    append_jsonl(diagnostics_path, diagnostics)
    append_jsonl(chunk_errors_path, chunk_error_rows)
    return pair_labels, {
        "source_root": str(source_root),
        "parse_records_jsonl": str(parse_path),
        "chunks_jsonl": str(chunks_path),
        "diagnostics_jsonl": str(diagnostics_path),
        "chunk_errors_jsonl": str(chunk_errors_path),
        "parse_record_count": len(parse_records),
        "chunk_count": len(chunks),
        "diagnostic_count": len(diagnostics),
        "chunk_error_pair_count": len(chunk_error_rows),
        "syntax_error_chunk_count": sum(1 for chunk in chunks if chunk.get("chunk_kind") == "syntax_error_file"),
        "parse_records_sha256": sha256_file(parse_path),
        "chunks_sha256": sha256_file(chunks_path),
        "diagnostics_sha256": sha256_file(diagnostics_path),
        "chunk_errors_sha256": sha256_file(chunk_errors_path),
    }


def chunk_surface_labels(chunk: dict[str, Any]) -> set[str]:
    labels = {
        f"ast_surface:chunk_kind:{chunk.get('chunk_kind')}",
        f"ast_surface:node_kind:{chunk.get('node_kind')}",
    }
    rel = str(chunk.get("relative_path") or "")
    if "/test" in rel or rel.startswith("test"):
        labels.add("ast_surface:path:test")
    else:
        labels.add("ast_surface:path:non_test")
    source = str(chunk.get("source_text") or "")
    if "assert " in source or chunk.get("chunk_kind") == "assertion":
        labels.add("ast_surface:assertion_present")
    if "except " in source or "raise " in source:
        labels.add("ast_surface:error_handling_present")
    if "import " in source or "from " in source:
        labels.add("ast_surface:import_surface")
    name = str(chunk.get("name") or "")
    for token in TOKEN_RE.findall(name)[:4]:
        if len(token) >= 3:
            labels.add(f"ast_surface:name_token:{token.lower()}")
    return {label for label in labels if not label_has_target_leak(label)}


def label_has_target_leak(label: str) -> bool:
    lowered = label.lower()
    return any(lowered.startswith(prefix) for prefix in TARGET_LEAK_LABEL_PREFIXES)


def cap_labels(labels: set[str], limit: int) -> list[str]:
    return sorted(labels)[:limit]


def build_transition_model(pair_labels: list[PairLabels], *, max_labels_per_side: int, smoothing: float) -> tuple[dict[tuple[str, str], int], Counter[str], Counter[str], dict[tuple[str, str], float]]:
    transitions: Counter[tuple[str, str]] = Counter()
    before_totals: Counter[str] = Counter()
    after_totals: Counter[str] = Counter()
    for item in pair_labels:
        if item.pair.split != "train":
            continue
        before = cap_labels(item.before_labels, max_labels_per_side)
        after = cap_labels(item.after_labels, max_labels_per_side)
        for left in before:
            before_totals[left] += len(after)
            for right in after:
                transitions[(left, right)] += 1
        for right in after:
            after_totals[right] += len(before)
    vocab_before = max(1, len(before_totals))
    vocab_after = max(1, len(after_totals))
    asymmetry: dict[tuple[str, str], float] = {}
    for edge, count in transitions.items():
        left, right = edge
        forward = (count + smoothing) / (before_totals[left] + smoothing * vocab_after)
        reverse_count = transitions.get((right, left), 0)
        reverse_total = after_totals[right] + smoothing * vocab_before
        reverse = (reverse_count + smoothing) / reverse_total
        asymmetry[edge] = math.log(forward) - math.log(reverse)
    return dict(transitions), before_totals, after_totals, asymmetry


def score_pair(item: PairLabels, asymmetry: dict[tuple[str, str], float], *, max_labels_per_side: int) -> float:
    scores: list[float] = []
    for left in cap_labels(item.before_labels, max_labels_per_side):
        for right in cap_labels(item.after_labels, max_labels_per_side):
            value = asymmetry.get((left, right))
            if value is not None:
                scores.append(value)
    if not scores:
        return 0.0
    positives = [value for value in scores if value > 0]
    pool = positives or scores
    return sum(pool) / len(pool)


def auc(rows: list[tuple[float, bool]]) -> float | None:
    positives = [score for score, label in rows if label]
    negatives = [score for score, label in rows if not label]
    if not positives or not negatives:
        return None
    wins = 0.0
    total = len(positives) * len(negatives)
    for pos in positives:
        for neg in negatives:
            if pos > neg:
                wins += 1.0
            elif pos == neg:
                wins += 0.5
    return wins / total


def write_transition_artifacts(
    pair_labels: list[PairLabels],
    *,
    state_root: Path,
    max_labels_per_side: int,
    smoothing: float,
) -> dict[str, Any]:
    transitions, before_totals, after_totals, asymmetry = build_transition_model(
        pair_labels,
        max_labels_per_side=max_labels_per_side,
        smoothing=smoothing,
    )
    tensor_rows = []
    for (left, right), count in sorted(transitions.items()):
        tensor_rows.append(
            {
                "schema_version": SCHEMA_VERSION,
                "artifact_kind": "causal_cross_cluster_transition_tensor_row",
                "pseudo_embedder_id": "ast_surface",
                "label_before": left,
                "label_after": right,
                "transition_count": count,
                "before_total": before_totals[left],
                "after_total": after_totals[right],
                "asymmetry_log_ratio": asymmetry[(left, right)],
            }
        )
    score_rows = []
    split_scores: dict[str, list[tuple[float, bool]]] = defaultdict(list)
    for item in pair_labels:
        score = score_pair(item, asymmetry, max_labels_per_side=max_labels_per_side)
        split_scores[item.pair.split].append((score, item.pair.bug_fix_label))
        score_rows.append(
            {
                "schema_version": SCHEMA_VERSION,
                "artifact_kind": "causal_pair_directional_score",
                "pair_id": item.pair.pair_id,
                "split": item.pair.split,
                "bug_fix_label": item.pair.bug_fix_label,
                "directional_score": score,
                "before_label_count": len(item.before_labels),
                "after_label_count": len(item.after_labels),
                "before_chunk_count": item.before_chunk_count,
                "after_chunk_count": item.after_chunk_count,
            }
        )
    high_edges = sorted(tensor_rows, key=lambda row: row["asymmetry_log_ratio"], reverse=True)[:100]
    transition_path = state_root / "cross_cluster_transition_tensor.jsonl"
    score_path = state_root / "heldout_directional_scores.jsonl"
    high_edges_path = state_root / "high_asymmetry_edges.jsonl"
    append_jsonl(transition_path, tensor_rows)
    append_jsonl(score_path, score_rows)
    append_jsonl(high_edges_path, high_edges)
    split_metrics = {
        split: {
            "row_count": len(rows),
            "positive_count": sum(1 for _score, label in rows if label),
            "negative_count": sum(1 for _score, label in rows if not label),
            "auc": auc(rows),
        }
        for split, rows in sorted(split_scores.items())
    }
    return {
        "state_root": str(state_root),
        "transition_tensor_jsonl": str(transition_path),
        "heldout_scores_jsonl": str(score_path),
        "high_asymmetry_edges_jsonl": str(high_edges_path),
        "transition_rows": len(tensor_rows),
        "score_rows": len(score_rows),
        "high_edge_rows": len(high_edges),
        "split_metrics": split_metrics,
        "transition_tensor_sha256": sha256_file(transition_path),
        "heldout_scores_sha256": sha256_file(score_path),
        "high_asymmetry_edges_sha256": sha256_file(high_edges_path),
        "decision_note": (
            "This runner reports ast_surface pseudo-embedder AUC only. "
            "The #744 promotion rule still requires real per-embedder forward-cache AUC."
        ),
    }


def write_pair_artifacts(pairs: Iterable[PairRecord], corpus_root: Path) -> dict[str, Any]:
    path = corpus_root / "commitpackft_python_pairs.jsonl"
    rows = []
    for pair in pairs:
        rows.append(
            {
                "schema_version": SCHEMA_VERSION,
                "artifact_kind": "commitpackft_python_pair",
                "pair_id": pair.pair_id,
                "source_index": pair.source_index,
                "commit": pair.commit,
                "repo": pair.repo,
                "license": pair.license,
                "old_file": pair.old_file,
                "new_file": pair.new_file,
                "old_sha256": pair.old_sha256,
                "new_sha256": pair.new_sha256,
                "bug_fix_label": pair.bug_fix_label,
                "split": pair.split,
                "leakage_policy": "target_side_only",
                "message_sha256": sha256_prefixed(pair.message.encode("utf-8")),
            }
        )
    append_jsonl(path, rows)
    return {"pairs_jsonl": str(path), "pair_rows": len(rows), "pairs_sha256": sha256_file(path)}


def tree_state(path: Path) -> dict[str, Any]:
    if not path.exists():
        return {"path": str(path), "exists": False}
    files = [p for p in path.rglob("*") if p.is_file()]
    return {
        "path": str(path),
        "exists": True,
        "file_count": len(files),
        "bytes": sum(p.stat().st_size for p in files),
    }


def run_pipeline(args: argparse.Namespace) -> dict[str, Any]:
    run_id = args.run_id or f"run-{utc_stamp()}"
    corpus_root = args.corpus_root or (args.corpus_parent / f"commitpackft-v1-{run_id}")
    fsv_root = args.fsv_root or (args.fsv_parent / run_id)
    state_root = args.state_root / run_id
    before = {
        "corpus_root": tree_state(corpus_root),
        "fsv_root": tree_state(fsv_root),
        "state_root": tree_state(state_root),
    }
    if args.clean:
        for root in (corpus_root, fsv_root, state_root):
            if root.exists():
                shutil.rmtree(root)
    corpus_root.mkdir(parents=True, exist_ok=True)
    fsv_root.mkdir(parents=True, exist_ok=True)
    state_root.mkdir(parents=True, exist_ok=True)
    status_path = fsv_root / "run_status.json"
    write_json(status_path, {"status": "running", "updated_at_utc": utc_now(), "run_id": run_id})

    pairs, ingest_summary = load_pairs(
        args.input_jsonl,
        input_limit=args.input_limit,
        max_pairs=args.max_pairs,
        max_source_bytes=args.max_source_bytes,
    )
    pair_labels, chunk_summary = materialize_chunks(pairs, corpus_root=corpus_root, schema_root=fsv_root / "schemas")
    pair_summary = write_pair_artifacts((item.pair for item in pair_labels), corpus_root)
    transition_summary = write_transition_artifacts(
        pair_labels,
        state_root=state_root,
        max_labels_per_side=args.max_labels_per_side,
        smoothing=args.smoothing,
    )

    leakage_violations = sorted(
        label
        for item in pair_labels
        for label in (item.before_labels | item.after_labels)
        if label_has_target_leak(label)
    )
    manifest = {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "artifact_kind": ARTIFACT_KIND,
        "created_at_utc": utc_now(),
        "run_id": run_id,
        "source_of_truth": {
            "corpus_root": str(corpus_root),
            "state_root": str(state_root),
            "fsv_root": str(fsv_root),
            **pair_summary,
            **{key: value for key, value in chunk_summary.items() if key.endswith("_jsonl")},
            **{
                "transition_tensor_jsonl": transition_summary["transition_tensor_jsonl"],
                "heldout_scores_jsonl": transition_summary["heldout_scores_jsonl"],
                "high_asymmetry_edges_jsonl": transition_summary["high_asymmetry_edges_jsonl"],
            },
        },
        "input": ingest_summary,
        "pairs": pair_summary,
        "chunks": chunk_summary,
        "transition_summary": transition_summary,
        "leakage_policy": {
            "bug_fix_label_policy": "target_side_only",
            "live_input_label_prefix_violations": leakage_violations[:20],
            "violation_count": len(leakage_violations),
        },
        "doctrine": {
            "slot_identity_preserved": True,
            "flat_vector_concat_used": False,
            "target_labels_used_as_live_inputs": False,
            "aiwonder_durable_root_required": str(corpus_root).startswith("/zfs/archive/contextgraph")
            and str(fsv_root).startswith("/zfs/archive/contextgraph")
            if not args.allow_non_aiwonder_roots
            else True,
        },
        "before": before,
        "after": {
            "corpus_root": tree_state(corpus_root),
            "fsv_root": tree_state(fsv_root),
            "state_root": tree_state(state_root),
        },
    }
    passes = (
        pair_summary["pair_rows"] > 0
        and chunk_summary["chunk_count"] > 0
        and transition_summary["transition_rows"] > 0
        and len(leakage_violations) == 0
        and manifest["doctrine"]["aiwonder_durable_root_required"]
    )
    manifest["passes"] = passes
    manifest_path = fsv_root / "causal_auto_labeling_manifest.json"
    write_json(manifest_path, manifest)
    hashes = {
        "causal_auto_labeling_manifest.json": sha256_file(manifest_path),
        "commitpackft_python_pairs.jsonl": pair_summary["pairs_sha256"],
        "python_ast_chunks.jsonl": chunk_summary["chunks_sha256"],
        "cross_cluster_transition_tensor.jsonl": transition_summary["transition_tensor_sha256"],
        "heldout_directional_scores.jsonl": transition_summary["heldout_scores_sha256"],
        "high_asymmetry_edges.jsonl": transition_summary["high_asymmetry_edges_sha256"],
    }
    hashes_path = fsv_root / "artifact_hashes.json"
    write_json(hashes_path, hashes)
    readback = {
        "passes": passes,
        "manifest_path": str(manifest_path),
        "manifest_sha256": hashes["causal_auto_labeling_manifest.json"],
        "hashes_path": str(hashes_path),
        "hashes_sha256": sha256_file(hashes_path),
        "line_counts": {
            "pairs": pair_summary["pair_rows"],
            "chunks": chunk_summary["chunk_count"],
            "transition_rows": transition_summary["transition_rows"],
            "score_rows": transition_summary["score_rows"],
        },
        "split_metrics": transition_summary["split_metrics"],
    }
    readback_path = fsv_root / "manual_readback_summary.json"
    write_json(readback_path, readback)
    write_json(status_path, {"status": "complete" if passes else "failed", "updated_at_utc": utc_now(), "run_id": run_id})
    if not passes:
        raise CausalAutoLabelError(f"CAUSAL_LABEL_RUN_FAILED: {manifest_path}")
    return readback


def write_synthetic_input(path: Path) -> None:
    rows = [
        {
            "commit": "a" * 40,
            "lang": "Python",
            "license": "mit",
            "message": "Fix crash when queue is empty",
            "subject": "Fix empty queue crash",
            "repos": "example/project",
            "old_file": "pkg/queue.py",
            "new_file": "pkg/queue.py",
            "old_contents": "def pop(q):\n    return q[0]\n",
            "new_contents": "def pop(q):\n    if not q:\n        raise ValueError('empty')\n    return q[0]\n",
        },
        {
            "commit": "b" * 40,
            "lang": "Python",
            "license": "mit",
            "message": "Add queue size helper",
            "subject": "Add helper",
            "repos": "example/project",
            "old_file": "pkg/queue.py",
            "new_file": "pkg/queue.py",
            "old_contents": "def pop(q):\n    return q[0]\n",
            "new_contents": "def pop(q):\n    return q[0]\n\ndef size(q):\n    return len(q)\n",
        },
        {
            "commit": "c" * 40,
            "lang": "Python",
            "license": "mit",
            "message": "Documentation cleanup",
            "subject": "Update docs",
            "repos": "example/project",
            "old_file": "docs/conf.py",
            "new_file": "docs/conf.py",
            "old_contents": "project = 'old'\n",
            "new_contents": "project = 'new'\n",
        },
        {
            "commit": "d" * 40,
            "lang": "Python",
            "license": "mit",
            "message": "Fix parser error handling",
            "subject": "Fix parser",
            "repos": "example/project",
            "old_file": "pkg/parser.py",
            "new_file": "pkg/parser.py",
            "old_contents": "def parse(x):\n    if x:\n        return x\n",
            "new_contents": "def parse(x):\n    if x:\n        return x\n    else print('bad')\n",
        },
    ]
    append_jsonl(path, rows)


def run_self_test(args: argparse.Namespace) -> int:
    root = args.self_test_root or Path(tempfile.mkdtemp(prefix="task-cr-006-self-test-"))
    if args.clean and root.exists():
        shutil.rmtree(root)
    root.mkdir(parents=True, exist_ok=True)
    synthetic = root / "synthetic_commitpackft_python.jsonl"
    write_synthetic_input(synthetic)
    run_args = argparse.Namespace(
        input_jsonl=synthetic,
        input_limit=None,
        max_pairs=None,
        max_source_bytes=50_000,
        corpus_parent=root / "corpus-parent",
        corpus_root=root / "corpus" / "commitpackft-v1-self-test",
        state_root=root / "state" / "cross-cluster-transitions" / "task-cr-006-v1",
        fsv_parent=root / "fsv-parent",
        fsv_root=root / "fsv" / "task-cr-006-causal-auto-labeling-mvp-fsv" / "run-self-test",
        run_id="run-self-test",
        clean=True,
        max_labels_per_side=32,
        smoothing=0.5,
        allow_non_aiwonder_roots=True,
    )
    readback = run_pipeline(run_args)
    manifest = json.loads((run_args.fsv_root / "causal_auto_labeling_manifest.json").read_text(encoding="utf-8"))
    edge_cases = [
        {
            "case": "doc_only_commit_excluded",
            "pass": manifest["input"]["rejection_counts"].get("doc_path_only", 0) >= 1,
            "before": "synthetic docs/conf.py row present",
            "after": manifest["input"]["rejection_counts"],
        },
        {
            "case": "syntax_error_after_state_retained_as_chunk",
            "pass": manifest["chunks"]["syntax_error_chunk_count"] >= 1,
            "before": "synthetic parser.py after state has invalid syntax",
            "after": {"syntax_error_chunk_count": manifest["chunks"]["syntax_error_chunk_count"]},
        },
        {
            "case": "target_label_not_live_input_label",
            "pass": manifest["leakage_policy"]["violation_count"] == 0,
            "before": "bug_fix_label exists on pair metadata",
            "after": manifest["leakage_policy"],
        },
    ]
    empty_input = root / "empty.jsonl"
    empty_input.write_text("", encoding="utf-8")
    try:
        load_pairs(empty_input, input_limit=None, max_pairs=None, max_source_bytes=50_000)
        empty_pass = False
        empty_after = "unexpected success"
    except CausalAutoLabelError as exc:
        empty_pass = "CAUSAL_LABEL_NO_ACCEPTED_PAIRS" in str(exc)
        empty_after = str(exc)
    edge_cases.append(
        {
            "case": "empty_input_rejected",
            "pass": empty_pass,
            "before": "empty JSONL",
            "after": empty_after,
        }
    )
    report = {"passes": all(row["pass"] for row in edge_cases), "readback": readback, "edge_cases": edge_cases}
    write_json(root / "self_test_report.json", report)
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0 if report["passes"] else 1


def add_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--input-jsonl", type=Path, default=DEFAULT_INPUT_JSONL)
    parser.add_argument("--input-limit", type=int, default=None)
    parser.add_argument("--max-pairs", type=int, default=500)
    parser.add_argument("--max-source-bytes", type=int, default=250_000)
    parser.add_argument("--corpus-parent", type=Path, default=DEFAULT_CORPUS_PARENT)
    parser.add_argument("--corpus-root", type=Path, default=None)
    parser.add_argument("--state-root", type=Path, default=DEFAULT_STATE_ROOT)
    parser.add_argument("--fsv-parent", type=Path, default=DEFAULT_FSV_PARENT)
    parser.add_argument("--fsv-root", type=Path, default=None)
    parser.add_argument("--run-id", default=None)
    parser.add_argument("--max-labels-per-side", type=int, default=48)
    parser.add_argument("--smoothing", type=float, default=0.5)
    parser.add_argument("--clean", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--self-test-root", type=Path, default=None)
    parser.add_argument("--allow-non-aiwonder-roots", action="store_true")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    add_arguments(parser)
    args = parser.parse_args(argv)
    if args.self_test:
        return run_self_test(args)
    readback = run_pipeline(args)
    print(json.dumps(readback, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
