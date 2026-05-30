#!/usr/bin/env python3
"""TASK-CR-006 per-embedder causal transition AUC runner.

This runner consumes a completed packed forward cache for CommitPackFT Python
pairs and computes causal-pair transition evidence without flattening slots.
It is intentionally measurement-only: target bug-fix labels are used as heldout
supervision for AUC, never as live vector labels.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import shutil
import struct
import tempfile
from collections import Counter, defaultdict
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Iterable


TASK_ID = "TASK-CR-006"
ARTIFACT_KIND = "causal_embedder_transition_auc"
SCHEMA_VERSION = 1

DEFAULT_ACTIVE_EMBEDDERS = ("e1", "e10", "e12", "e13", "e14", "e6", "e7", "e8", "e9")
DEFAULT_FORWARD_ROOT = Path("/zfs/archive/contextgraph/corpus/commitpackft-python-forward-cache-task-cr-006")
DEFAULT_PAIRS_JSONL = Path(
    "/zfs/archive/contextgraph/corpus/commitpackft-v1-run-20260530T003743Z-sample1000/"
    "commitpackft_python_pairs.jsonl"
)
DEFAULT_OUTPUT_ROOT = Path("/zfs/archive/contextgraph/fsv/task-cr-006-causal-embedder-auc-fsv")
DEFAULT_STATE_ROOT = Path("/zfs/archive/contextgraph/state/cross-cluster-transitions/task-cr-006-v1")
TARGET_LEAK_PREFIXES = (
    "bug_fix",
    "non_bug_fix",
    "oracle",
    "docker",
    "ground_truth",
    "target",
    "label",
)
FEATURE_NAMES = ("norm_l2", "abs_mean", "std", "zero_fraction")


class CausalEmbedderAucError(RuntimeError):
    pass


@dataclass(frozen=True)
class PairInfo:
    pair_id: str
    bug_fix_label: bool
    split: str
    repo: str
    old_file: str
    new_file: str


@dataclass
class ForwardRecord:
    pair_id: str
    embedder_id: str
    side: str
    split: str
    bug_fix_label: bool
    vector_sha256: str
    feature_values: dict[str, float]
    simhash: str
    source_record_path: str
    source_line_no: int
    cluster_label: str = ""


def utc_stamp() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")


def utc_now() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def canonical_json(value: Any) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def sha256_bytes(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + f".tmp-{hashlib.sha256(str(path).encode()).hexdigest()[:8]}")
    tmp.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    tmp.replace(path)


def write_jsonl(path: Path, rows: Iterable[dict[str, Any]]) -> int:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    count = 0
    with tmp.open("w", encoding="utf-8") as handle:
        for row in rows:
            handle.write(canonical_json(row) + "\n")
            count += 1
    tmp.replace(path)
    return count


def iter_jsonl(path: Path) -> Iterable[tuple[int, dict[str, Any]]]:
    with path.open("r", encoding="utf-8") as handle:
        for line_no, line in enumerate(handle, start=1):
            if not line.strip():
                continue
            yield line_no, json.loads(line)


def read_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def require(condition: bool, code: str, detail: Any = None) -> None:
    if not condition:
        suffix = "" if detail is None else f": {detail}"
        raise CausalEmbedderAucError(f"{code}{suffix}")


def parse_embedder_list(raw: str) -> tuple[str, ...]:
    values = tuple(item.strip() for item in raw.split(",") if item.strip())
    require(bool(values), "CAUSAL_AUC_EMPTY_EMBEDDER_LIST")
    require(len(values) == len(set(values)), "CAUSAL_AUC_DUPLICATE_EMBEDDER", values)
    return values


def tree_state(path: Path) -> dict[str, Any]:
    if not path.exists():
        return {"path": str(path), "exists": False}
    files = [item for item in path.rglob("*") if item.is_file()]
    return {
        "path": str(path),
        "exists": True,
        "file_count": len(files),
        "byte_count": sum(item.stat().st_size for item in files),
        "sample": [str(item.relative_to(path)) for item in sorted(files)[:20]],
    }


def load_pairs(path: Path) -> dict[str, PairInfo]:
    require(path.is_file(), "CAUSAL_AUC_PAIRS_JSONL_MISSING", path)
    pairs: dict[str, PairInfo] = {}
    split_counts: Counter[str] = Counter()
    label_counts: Counter[str] = Counter()
    for line_no, row in iter_jsonl(path):
        pair_id = str(row.get("pair_id") or "")
        split = str(row.get("split") or "")
        require(pair_id, "CAUSAL_AUC_PAIR_ID_MISSING", (path, line_no))
        require(split in {"train", "validation", "test"}, "CAUSAL_AUC_PAIR_SPLIT_INVALID", (pair_id, split))
        require(pair_id not in pairs, "CAUSAL_AUC_PAIR_ID_DUPLICATE", pair_id)
        label = bool(row.get("bug_fix_label"))
        pairs[pair_id] = PairInfo(
            pair_id=pair_id,
            bug_fix_label=label,
            split=split,
            repo=str(row.get("repo") or ""),
            old_file=str(row.get("old_file") or ""),
            new_file=str(row.get("new_file") or ""),
        )
        split_counts[split] += 1
        label_counts["bug_fix" if label else "non_bug_fix"] += 1
    require(bool(pairs), "CAUSAL_AUC_NO_PAIR_ROWS")
    return pairs


def f32_bytes(vector: list[float]) -> bytes:
    return struct.pack("<" + "f" * len(vector), *vector)


def vector_sha256(vector: list[float]) -> str:
    return hashlib.sha256(f32_bytes(vector)).hexdigest()


def feature_values(vector: list[float]) -> dict[str, float]:
    if not vector:
        return {"norm_l2": 0.0, "abs_mean": 0.0, "std": 0.0, "zero_fraction": 1.0}
    total = sum(vector)
    mean = total / len(vector)
    abs_mean = sum(abs(value) for value in vector) / len(vector)
    variance = sum((value - mean) ** 2 for value in vector) / len(vector)
    zero_fraction = sum(1 for value in vector if value == 0.0) / len(vector)
    return {
        "norm_l2": math.sqrt(sum(value * value for value in vector)),
        "abs_mean": abs_mean,
        "std": math.sqrt(max(variance, 0.0)),
        "zero_fraction": zero_fraction,
    }


def deterministic_simhash(vector: list[float], embedder_id: str, bits: int, probes_per_bit: int) -> str:
    if bits <= 0:
        return "0"
    require(bool(vector), "CAUSAL_AUC_VECTOR_EMPTY_FOR_SIMHASH", embedder_id)
    bit_values: list[str] = []
    dim = len(vector)
    for bit in range(bits):
        pos = 0.0
        neg = 0.0
        for probe in range(probes_per_bit):
            seed = f"{embedder_id}:{dim}:{bit}:{probe}".encode("utf-8")
            digest = hashlib.sha256(seed).digest()
            left = int.from_bytes(digest[:8], "little") % dim
            right = int.from_bytes(digest[8:16], "little") % dim
            pos += float(vector[left])
            neg += float(vector[right])
        bit_values.append("1" if pos >= neg else "0")
    return f"{int(''.join(bit_values), 2):0{max(1, math.ceil(bits / 4))}x}"


def side_from_workspace(workspace_state: str, source: str) -> str | None:
    joined = f"{workspace_state} {source}".lower()
    if "before" in joined:
        return "before"
    if "after" in joined:
        return "after"
    return None


def discover_shards(forward_root: Path) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    require(forward_root.is_dir(), "CAUSAL_AUC_FORWARD_ROOT_MISSING", forward_root)
    corpus_manifest_path = forward_root / "python_forward_cache_corpus_manifest.json"
    shard_plan_path = forward_root / "python_forward_cache_shard_plan.json"
    checkpoint_path = forward_root / "python_forward_cache_checkpoint.json"
    for path in (corpus_manifest_path, shard_plan_path, checkpoint_path):
        require(path.is_file(), "CAUSAL_AUC_FORWARD_ROOT_ARTIFACT_MISSING", path)
    corpus_manifest = read_json(corpus_manifest_path)
    checkpoint = read_json(checkpoint_path)
    shard_plan = read_json(shard_plan_path)
    require(corpus_manifest.get("passes") is True, "CAUSAL_AUC_FORWARD_CORPUS_MANIFEST_NOT_PASSING", corpus_manifest_path)
    require(checkpoint.get("passes") is True, "CAUSAL_AUC_FORWARD_CHECKPOINT_NOT_PASSING", checkpoint_path)
    shards = list(shard_plan.get("shards") or [])
    require(bool(shards), "CAUSAL_AUC_FORWARD_SHARD_PLAN_EMPTY", shard_plan_path)
    return shards, {
        "forward_root": str(forward_root),
        "corpus_manifest_path": str(corpus_manifest_path),
        "corpus_manifest_sha256": sha256_file(corpus_manifest_path),
        "checkpoint_path": str(checkpoint_path),
        "checkpoint_sha256": sha256_file(checkpoint_path),
        "shard_plan_path": str(shard_plan_path),
        "shard_plan_sha256": sha256_file(shard_plan_path),
        "summary": corpus_manifest.get("summary") or {},
        "shard_count": len(shards),
    }


def ingest_forward_records(
    *,
    forward_root: Path,
    pairs: dict[str, PairInfo],
    expected_embedders: tuple[str, ...],
    simhash_bits: int,
    simhash_probes_per_bit: int,
    max_records_per_embedder: int | None,
) -> tuple[list[ForwardRecord], dict[str, Any]]:
    shards, forward_manifest = discover_shards(forward_root)
    expected = set(expected_embedders)
    records: list[ForwardRecord] = []
    counts: Counter[str] = Counter()
    per_embedder_seen: Counter[str] = Counter()
    skipped_unknown_pair: Counter[str] = Counter()
    samples: list[dict[str, Any]] = []
    dims: dict[str, int] = {}

    for shard_index, shard in enumerate(shards):
        shard_root = Path(str(shard.get("cache_root") or ""))
        packed_manifest_path = shard_root / "python_forward_cache_packed_manifest.json"
        require(packed_manifest_path.is_file(), "CAUSAL_AUC_PACKED_MANIFEST_MISSING", packed_manifest_path)
        packed_manifest = read_json(packed_manifest_path)
        require(packed_manifest.get("passes") is True, "CAUSAL_AUC_PACKED_MANIFEST_NOT_PASSING", packed_manifest_path)
        per_embedder = packed_manifest.get("per_embedder") or {}
        require(expected.issubset(set(per_embedder)), "CAUSAL_AUC_PACKED_EMBEDDER_SET_MISSING", {
            "expected": sorted(expected),
            "actual": sorted(per_embedder),
            "shard": str(shard_root),
        })
        for embedder_id in expected_embedders:
            info = per_embedder[embedder_id]
            record_path = Path(str(info.get("record_path") or ""))
            require(record_path.is_file(), "CAUSAL_AUC_RECORD_PATH_MISSING", record_path)
            manifest_row_count = int(info.get("row_count") or 0)
            local_count = 0
            for line_no, record in iter_jsonl(record_path):
                local_count += 1
                if max_records_per_embedder is not None and per_embedder_seen[embedder_id] >= max_records_per_embedder:
                    counts["max_records_skipped"] += 1
                    continue
                key = record.get("key") or {}
                chunk = record.get("chunk") or {}
                output = record.get("output") or {}
                actual_embedder = str(key.get("embedder_id") or "")
                require(actual_embedder == embedder_id, "CAUSAL_AUC_RECORD_EMBEDDER_MISMATCH", (record_path, line_no))
                pair_id = str(chunk.get("task_instance_id") or "")
                pair = pairs.get(pair_id)
                if pair is None:
                    skipped_unknown_pair[pair_id or "<missing>"] += 1
                    counts["unknown_pair_skipped_records"] += 1
                    continue
                side = side_from_workspace(str(chunk.get("workspace_state") or ""), str(chunk.get("source") or ""))
                if side is None:
                    counts["unknown_side_skipped_records"] += 1
                    continue
                vector_raw = output.get("vector")
                require(isinstance(vector_raw, list) and vector_raw, "CAUSAL_AUC_VECTOR_MISSING", (record_path, line_no))
                require(
                    all(isinstance(value, (int, float)) for value in vector_raw),
                    "CAUSAL_AUC_VECTOR_INVALID",
                    (record_path, line_no),
                )
                vector = [float(value) for value in vector_raw]
                require(bool(vector), "CAUSAL_AUC_VECTOR_INVALID", (record_path, line_no))
                require(all(math.isfinite(value) for value in vector), "CAUSAL_AUC_VECTOR_NONFINITE", (record_path, line_no))
                observed_sha = vector_sha256(vector)
                expected_sha = str(output.get("vector_sha256") or record.get("vector_sha256") or "")
                if expected_sha.startswith("sha256:"):
                    expected_sha = expected_sha.split(":", 1)[1]
                require(not expected_sha or expected_sha == observed_sha, "CAUSAL_AUC_VECTOR_SHA_MISMATCH", {
                    "path": str(record_path),
                    "line_no": line_no,
                    "expected": expected_sha,
                    "observed": observed_sha,
                })
                if embedder_id in dims:
                    require(dims[embedder_id] == len(vector), "CAUSAL_AUC_EMBEDDER_DIM_CONFLICT", embedder_id)
                dims.setdefault(embedder_id, len(vector))
                features = feature_values(vector)
                rec = ForwardRecord(
                    pair_id=pair_id,
                    embedder_id=embedder_id,
                    side=side,
                    split=pair.split,
                    bug_fix_label=pair.bug_fix_label,
                    vector_sha256=observed_sha,
                    feature_values=features,
                    simhash=deterministic_simhash(vector, embedder_id, simhash_bits, simhash_probes_per_bit),
                    source_record_path=str(record_path),
                    source_line_no=line_no,
                )
                records.append(rec)
                per_embedder_seen[embedder_id] += 1
                counts["used_records"] += 1
                counts[f"{side}_records"] += 1
                if len(samples) < 16:
                    samples.append(
                        {
                            "pair_id": pair_id,
                            "embedder_id": embedder_id,
                            "side": side,
                            "split": pair.split,
                            "vector_sha256": observed_sha,
                            "simhash": rec.simhash,
                            "record_path": str(record_path),
                            "line_no": line_no,
                        }
                    )
            require(local_count == manifest_row_count, "CAUSAL_AUC_RECORD_ROW_COUNT_MISMATCH", {
                "path": str(record_path),
                "local_count": local_count,
                "manifest_row_count": manifest_row_count,
            })
        counts["shards_read"] += 1
        counts["last_shard_index_read"] = shard_index

    require(bool(records), "CAUSAL_AUC_NO_FORWARD_RECORDS")
    return records, {
        "forward_manifest": forward_manifest,
        "record_counts": dict(sorted(counts.items())),
        "per_embedder_record_counts": dict(sorted(per_embedder_seen.items())),
        "unknown_pair_samples": dict(sorted(skipped_unknown_pair.items())[:10]),
        "slot_dims": dict(sorted(dims.items())),
        "samples": samples,
    }


def quantile_thresholds(values: list[float]) -> list[float]:
    if not values:
        return []
    ordered = sorted(values)
    if len(ordered) == 1:
        return [ordered[0]] * 4
    thresholds: list[float] = []
    for q in (0.2, 0.4, 0.6, 0.8):
        position = q * (len(ordered) - 1)
        lower = math.floor(position)
        upper = math.ceil(position)
        if lower == upper:
            thresholds.append(float(ordered[lower]))
        else:
            weight = position - lower
            thresholds.append(float(ordered[lower] * (1.0 - weight) + ordered[upper] * weight))
    return thresholds


def bin_index(value: float, thresholds: list[float]) -> int:
    index = 0
    for threshold in thresholds:
        if value > threshold:
            index += 1
    return index


def assign_cluster_labels(records: list[ForwardRecord], expected_embedders: tuple[str, ...]) -> dict[str, Any]:
    train_values: dict[str, dict[str, list[float]]] = {
        embedder_id: {name: [] for name in FEATURE_NAMES}
        for embedder_id in expected_embedders
    }
    for record in records:
        if record.split != "train":
            continue
        for name in FEATURE_NAMES:
            train_values[record.embedder_id][name].append(record.feature_values[name])
    thresholds = {
        embedder_id: {
            name: quantile_thresholds(values)
            for name, values in by_feature.items()
        }
        for embedder_id, by_feature in train_values.items()
    }
    label_counts: Counter[str] = Counter()
    per_embedder_label_counts: dict[str, Counter[str]] = {embedder_id: Counter() for embedder_id in expected_embedders}
    for record in records:
        bins = {
            name: bin_index(record.feature_values[name], thresholds[record.embedder_id][name])
            for name in FEATURE_NAMES
        }
        record.cluster_label = (
            f"cluster:{record.embedder_id}:h{record.simhash}:"
            f"n{bins['norm_l2']}:a{bins['abs_mean']}:s{bins['std']}:z{bins['zero_fraction']}"
        )
        label_counts[record.cluster_label] += 1
        per_embedder_label_counts[record.embedder_id][record.cluster_label] += 1
    violations = [
        label for label in label_counts
        if label.split(":", 1)[0].lower().startswith(TARGET_LEAK_PREFIXES)
    ]
    return {
        "thresholds": thresholds,
        "label_count": len(label_counts),
        "per_embedder_label_count": {
            embedder_id: len(counts)
            for embedder_id, counts in sorted(per_embedder_label_counts.items())
        },
        "target_label_prefix_violations": violations[:20],
        "target_label_prefix_violation_count": len(violations),
    }


def pair_side_label_counts(records: list[ForwardRecord]) -> dict[tuple[str, str, str], Counter[str]]:
    grouped: dict[tuple[str, str, str], Counter[str]] = defaultdict(Counter)
    for record in records:
        grouped[(record.pair_id, record.embedder_id, record.side)][record.cluster_label] += 1
    return grouped


def normalized(counter: Counter[str], limit: int) -> list[tuple[str, float]]:
    if not counter:
        return []
    items = sorted(counter.items(), key=lambda item: (-item[1], item[0]))[:limit]
    total = float(sum(count for _label, count in items))
    if total <= 0.0:
        return []
    return [(label, count / total) for label, count in items]


def build_transition_models(
    *,
    pairs: dict[str, PairInfo],
    grouped: dict[tuple[str, str, str], Counter[str]],
    expected_embedders: tuple[str, ...],
    max_labels_per_side: int,
    smoothing: float,
) -> tuple[dict[str, Any], list[dict[str, Any]], dict[str, dict[tuple[str, str], dict[str, float]]]]:
    raw: dict[str, Counter[tuple[str, str]]] = {embedder_id: Counter() for embedder_id in expected_embedders}
    by_label: dict[str, dict[tuple[str, str], Counter[str]]] = {
        embedder_id: defaultdict(Counter)
        for embedder_id in expected_embedders
    }
    missing_train_sides: Counter[str] = Counter()
    for pair in pairs.values():
        if pair.split != "train":
            continue
        for embedder_id in expected_embedders:
            before = normalized(grouped.get((pair.pair_id, embedder_id, "before"), Counter()), max_labels_per_side)
            after = normalized(grouped.get((pair.pair_id, embedder_id, "after"), Counter()), max_labels_per_side)
            if not before or not after:
                missing_train_sides[embedder_id] += 1
                continue
            class_key = "bug_fix" if pair.bug_fix_label else "non_bug_fix"
            for before_label, before_weight in before:
                for after_label, after_weight in after:
                    weight = before_weight * after_weight
                    raw[embedder_id][(before_label, after_label)] += weight
                    by_label[embedder_id][(before_label, after_label)][class_key] += weight

    models: dict[str, dict[tuple[str, str], dict[str, float]]] = {}
    rows: list[dict[str, Any]] = []
    summaries: dict[str, Any] = {}
    for embedder_id in expected_embedders:
        before_totals: Counter[str] = Counter()
        reverse_raw: Counter[tuple[str, str]] = Counter()
        for (before_label, after_label), count in raw[embedder_id].items():
            before_totals[before_label] += count
            reverse_raw[(after_label, before_label)] += count
        vocab_size = max(1, len({label for pair in raw[embedder_id] for label in pair}))
        models[embedder_id] = {}
        for (before_label, after_label), count in raw[embedder_id].items():
            reverse_count = reverse_raw.get((before_label, after_label), 0.0)
            p_forward = (count + smoothing) / (before_totals[before_label] + smoothing * vocab_size)
            p_reverse = (reverse_count + smoothing) / (before_totals.get(after_label, 0.0) + smoothing * vocab_size)
            class_counts = by_label[embedder_id][(before_label, after_label)]
            bug = float(class_counts.get("bug_fix", 0.0))
            non = float(class_counts.get("non_bug_fix", 0.0))
            log_odds = math.log((bug + smoothing) / (non + smoothing))
            asymmetry = math.log(p_forward) - math.log(p_reverse)
            models[embedder_id][(before_label, after_label)] = {
                "weight": float(count),
                "asymmetry_log_ratio": float(asymmetry),
                "bugfix_log_odds": float(log_odds),
                "bug_fix_weight": bug,
                "non_bug_fix_weight": non,
            }
            rows.append(
                {
                    "schema_version": SCHEMA_VERSION,
                    "artifact_kind": "causal_embedder_transition_tensor_row",
                    "embedder_id": embedder_id,
                    "before_cluster_label": before_label,
                    "after_cluster_label": after_label,
                    "transition_weight": float(count),
                    "bug_fix_weight": bug,
                    "non_bug_fix_weight": non,
                    "bugfix_log_odds": float(log_odds),
                    "asymmetry_log_ratio": float(asymmetry),
                }
            )
        summaries[embedder_id] = {
            "transition_count": len(raw[embedder_id]),
            "missing_train_side_pairs": int(missing_train_sides[embedder_id]),
        }
    return summaries, rows, models


def score_pair(
    before_counter: Counter[str],
    after_counter: Counter[str],
    model: dict[tuple[str, str], dict[str, float]],
    max_labels_per_side: int,
) -> tuple[float, float, int]:
    before = normalized(before_counter, max_labels_per_side)
    after = normalized(after_counter, max_labels_per_side)
    if not before or not after:
        return 0.0, 0.0, 0
    bugfix_score = 0.0
    asymmetry_score = 0.0
    matched = 0
    for before_label, before_weight in before:
        for after_label, after_weight in after:
            weight = before_weight * after_weight
            row = model.get((before_label, after_label))
            if row is None:
                continue
            matched += 1
            bugfix_score += weight * float(row["bugfix_log_odds"])
            asymmetry_score += weight * float(row["asymmetry_log_ratio"])
    return bugfix_score, asymmetry_score, matched


def auc_score(pairs: list[tuple[float, bool]]) -> float | None:
    positives = [score for score, label in pairs if label]
    negatives = [score for score, label in pairs if not label]
    if not positives or not negatives:
        return None
    wins = 0.0
    total = 0
    for pos in positives:
        for neg in negatives:
            total += 1
            if pos > neg:
                wins += 1.0
            elif pos == neg:
                wins += 0.5
    return wins / total if total else None


def score_heldout(
    *,
    pairs: dict[str, PairInfo],
    grouped: dict[tuple[str, str, str], Counter[str]],
    expected_embedders: tuple[str, ...],
    models: dict[str, dict[tuple[str, str], dict[str, float]]],
    max_labels_per_side: int,
) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    score_rows: list[dict[str, Any]] = []
    metric_inputs: dict[str, dict[str, dict[str, list[tuple[float, bool]]]]] = {
        embedder_id: {
            split: {"bugfix_transition": [], "asymmetry": []}
            for split in ("train", "validation", "test")
        }
        for embedder_id in expected_embedders
    }
    missing_side_counts: Counter[str] = Counter()
    no_matched_transition_counts: Counter[str] = Counter()
    for pair in pairs.values():
        for embedder_id in expected_embedders:
            before_counter = grouped.get((pair.pair_id, embedder_id, "before"), Counter())
            after_counter = grouped.get((pair.pair_id, embedder_id, "after"), Counter())
            if not before_counter or not after_counter:
                missing_side_counts[embedder_id] += 1
                continue
            bugfix_score, asymmetry_score, matched = score_pair(
                before_counter,
                after_counter,
                models[embedder_id],
                max_labels_per_side,
            )
            if matched == 0:
                no_matched_transition_counts[embedder_id] += 1
            metric_inputs[embedder_id][pair.split]["bugfix_transition"].append((bugfix_score, pair.bug_fix_label))
            metric_inputs[embedder_id][pair.split]["asymmetry"].append((asymmetry_score, pair.bug_fix_label))
            score_rows.append(
                {
                    "schema_version": SCHEMA_VERSION,
                    "artifact_kind": "causal_embedder_heldout_score",
                    "pair_id": pair.pair_id,
                    "repo": pair.repo,
                    "split": pair.split,
                    "bug_fix_label": pair.bug_fix_label,
                    "embedder_id": embedder_id,
                    "bugfix_transition_log_odds_score": bugfix_score,
                    "asymmetry_score": asymmetry_score,
                    "matched_transition_count": matched,
                    "before_cluster_count": len(before_counter),
                    "after_cluster_count": len(after_counter),
                }
            )
    metrics: dict[str, Any] = {}
    for embedder_id in expected_embedders:
        metrics[embedder_id] = {}
        for split in ("train", "validation", "test"):
            split_rows = metric_inputs[embedder_id][split]
            for score_name, values in split_rows.items():
                metrics[embedder_id][f"{split}_{score_name}_auc"] = auc_score(values)
                metrics[embedder_id][f"{split}_{score_name}_row_count"] = len(values)
                metrics[embedder_id][f"{split}_{score_name}_positive_count"] = sum(1 for _score, label in values if label)
                metrics[embedder_id][f"{split}_{score_name}_negative_count"] = sum(1 for _score, label in values if not label)
        metrics[embedder_id]["missing_side_pair_count"] = int(missing_side_counts[embedder_id])
        metrics[embedder_id]["no_matched_transition_pair_count"] = int(no_matched_transition_counts[embedder_id])
    return score_rows, metrics


def metrics_summary(per_embedder: dict[str, Any], expected_embedders: tuple[str, ...]) -> dict[str, Any]:
    test_aucs = [
        float(per_embedder[embedder_id]["test_bugfix_transition_auc"])
        for embedder_id in expected_embedders
        if per_embedder[embedder_id].get("test_bugfix_transition_auc") is not None
    ]
    validation_aucs = [
        float(per_embedder[embedder_id]["validation_bugfix_transition_auc"])
        for embedder_id in expected_embedders
        if per_embedder[embedder_id].get("validation_bugfix_transition_auc") is not None
    ]
    return {
        "test_macro_bugfix_transition_auc": sum(test_aucs) / len(test_aucs) if test_aucs else None,
        "validation_macro_bugfix_transition_auc": sum(validation_aucs) / len(validation_aucs) if validation_aucs else None,
        "test_embedder_auc_count": len(test_aucs),
        "validation_embedder_auc_count": len(validation_aucs),
        "per_embedder_test_auc_min": min(test_aucs) if test_aucs else None,
        "per_embedder_test_auc_max": max(test_aucs) if test_aucs else None,
        "passing_embedder_count_at_0_7": sum(1 for value in test_aucs if value >= 0.7),
        "promotion_rule_note": (
            "Issue #744 requires per-embedder asymmetric AUC >= 0.7 on holdout. "
            "This artifact reports weak-supervised bugfix transition AUC and an unsupervised asymmetry AUC separately."
        ),
    }


def run_pipeline(args: argparse.Namespace) -> dict[str, Any]:
    expected_embedders = parse_embedder_list(args.expected_embedders)
    run_id = args.run_id or f"run-{utc_stamp()}"
    output_root = args.output_root / run_id if args.output_root.name != run_id else args.output_root
    state_root = args.state_root / run_id if args.state_root.name != run_id else args.state_root
    before = {
        "output_root": tree_state(output_root),
        "state_root": tree_state(state_root),
        "forward_root": tree_state(args.forward_root),
    }
    if args.clean:
        for root in (output_root, state_root):
            if root.exists():
                shutil.rmtree(root)
    output_root.mkdir(parents=True, exist_ok=True)
    state_root.mkdir(parents=True, exist_ok=True)
    status_path = output_root / "run_status.json"
    write_json(status_path, {"status": "running", "run_id": run_id, "updated_at_utc": utc_now()})

    pairs = load_pairs(args.pairs_jsonl)
    records, ingest_manifest = ingest_forward_records(
        forward_root=args.forward_root,
        pairs=pairs,
        expected_embedders=expected_embedders,
        simhash_bits=args.simhash_bits,
        simhash_probes_per_bit=args.simhash_probes_per_bit,
        max_records_per_embedder=args.max_records_per_embedder,
    )
    label_manifest = assign_cluster_labels(records, expected_embedders)
    grouped = pair_side_label_counts(records)
    transition_summary, transition_rows, models = build_transition_models(
        pairs=pairs,
        grouped=grouped,
        expected_embedders=expected_embedders,
        max_labels_per_side=args.max_labels_per_side,
        smoothing=args.smoothing,
    )
    score_rows, per_embedder_metrics = score_heldout(
        pairs=pairs,
        grouped=grouped,
        expected_embedders=expected_embedders,
        models=models,
        max_labels_per_side=args.max_labels_per_side,
    )
    summary = metrics_summary(per_embedder_metrics, expected_embedders)

    transition_path = state_root / "per_embedder_transition_tensor.jsonl"
    scores_path = state_root / "per_embedder_heldout_scores.jsonl"
    label_registry_path = state_root / "per_embedder_cluster_label_registry.jsonl"
    transition_rows_written = write_jsonl(transition_path, transition_rows)
    score_rows_written = write_jsonl(scores_path, score_rows)
    label_counts = Counter(record.cluster_label for record in records)
    label_rows = [
        {
            "schema_version": SCHEMA_VERSION,
            "artifact_kind": "causal_embedder_cluster_label",
            "cluster_label": label,
            "embedder_id": label.split(":", 2)[1] if ":" in label else "unknown",
            "support": count,
            "leakage_policy": "live_safe_vector_derived",
        }
        for label, count in sorted(label_counts.items())
    ]
    label_rows_written = write_jsonl(label_registry_path, label_rows)

    manifest = {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "artifact_kind": ARTIFACT_KIND,
        "created_at_utc": utc_now(),
        "run_id": run_id,
        "source_of_truth": {
            "forward_root": str(args.forward_root),
            "pairs_jsonl": str(args.pairs_jsonl),
            "output_root": str(output_root),
            "state_root": str(state_root),
            "transition_tensor_jsonl": str(transition_path),
            "heldout_scores_jsonl": str(scores_path),
            "cluster_label_registry_jsonl": str(label_registry_path),
        },
        "input": {
            "pair_count": len(pairs),
            "pairs_sha256": sha256_file(args.pairs_jsonl),
            "expected_embedder_ids": list(expected_embedders),
            "simhash_bits": args.simhash_bits,
            "simhash_probes_per_bit": args.simhash_probes_per_bit,
            "max_labels_per_side": args.max_labels_per_side,
            "smoothing": args.smoothing,
        },
        "ingest_manifest": ingest_manifest,
        "label_manifest": label_manifest,
        "transition_summary": transition_summary,
        "per_embedder_metrics": per_embedder_metrics,
        "summary": summary,
        "row_counts": {
            "forward_records_used": len(records),
            "transition_rows": transition_rows_written,
            "heldout_score_rows": score_rows_written,
            "cluster_label_rows": label_rows_written,
        },
        "leakage_policy": {
            "bug_fix_label_policy": "target_side_only_supervision",
            "live_cluster_labels_are_vector_derived": True,
            "ignored_target_metadata_fields": ["mutation_category"],
            "target_label_prefix_violations": label_manifest["target_label_prefix_violations"],
            "target_label_prefix_violation_count": label_manifest["target_label_prefix_violation_count"],
            "target_labels_used_as_live_inputs": False,
        },
        "doctrine": {
            "slot_identity_preserved": True,
            "flat_vector_concat_used": False,
            "target_labels_used_as_live_inputs": False,
            "no_inner_llm_used": True,
            "aiwonder_durable_root_required": (
                str(output_root).startswith("/zfs/archive/contextgraph")
                and str(state_root).startswith("/zfs/archive/contextgraph")
                and str(args.forward_root).startswith("/zfs/archive/contextgraph")
            )
            if not args.allow_non_aiwonder_roots
            else True,
        },
        "before": before,
        "after": {
            "output_root": tree_state(output_root),
            "state_root": tree_state(state_root),
        },
    }
    passes = (
        len(records) > 0
        and transition_rows_written > 0
        and score_rows_written > 0
        and label_manifest["target_label_prefix_violation_count"] == 0
        and manifest["doctrine"]["aiwonder_durable_root_required"] is True
    )
    manifest["passes"] = passes
    manifest_path = output_root / "causal_embedder_auc_manifest.json"
    write_json(manifest_path, manifest)
    hashes = {
        "causal_embedder_auc_manifest.json": sha256_file(manifest_path),
        "per_embedder_transition_tensor.jsonl": sha256_file(transition_path),
        "per_embedder_heldout_scores.jsonl": sha256_file(scores_path),
        "per_embedder_cluster_label_registry.jsonl": sha256_file(label_registry_path),
    }
    hashes_path = output_root / "artifact_hashes.json"
    write_json(hashes_path, hashes)
    readback = {
        "passes": passes,
        "run_id": run_id,
        "manifest_path": str(manifest_path),
        "manifest_sha256": hashes["causal_embedder_auc_manifest.json"],
        "hashes_path": str(hashes_path),
        "hashes_sha256": sha256_file(hashes_path),
        "row_counts": manifest["row_counts"],
        "summary": summary,
        "per_embedder_metrics": per_embedder_metrics,
    }
    readback_path = output_root / "manual_readback_summary.json"
    write_json(readback_path, readback)
    write_json(status_path, {"status": "complete" if passes else "failed", "run_id": run_id, "updated_at_utc": utc_now()})
    if not passes:
        raise CausalEmbedderAucError(f"CAUSAL_AUC_RUN_FAILED: {manifest_path}")
    print(json.dumps(readback, indent=2, sort_keys=True))
    return readback


def fixture_vector(kind: str, dim: int = 8) -> list[float]:
    base = {
        "before": [0.1, 0.2, 0.1, 0.2, 0.0, 0.0, 0.3, 0.2],
        "bug_after": [1.0, 1.2, 1.1, 0.9, 0.0, 0.2, 0.1, 0.1],
        "non_after": [-1.0, -1.1, -0.9, -1.2, 0.0, -0.2, -0.1, -0.1],
    }[kind]
    return base[:dim]


def write_fixture_forward_root(root: Path, pairs_path: Path, embedders: tuple[str, ...]) -> None:
    root.mkdir(parents=True, exist_ok=True)
    pairs = []
    split_cycle = [
        ("train", True), ("train", False), ("train", True), ("train", False),
        ("validation", True), ("validation", False), ("validation", True), ("validation", False),
        ("test", True), ("test", False), ("test", True), ("test", False),
    ]
    for idx, (split, label) in enumerate(split_cycle):
        pairs.append(
            {
                "schema_version": 1,
                "artifact_kind": "commitpackft_python_pair",
                "pair_id": f"fixture-pair-{idx:03d}",
                "bug_fix_label": label,
                "split": split,
                "repo": "fixture/repo",
                "old_file": "pkg/example.py",
                "new_file": "pkg/example.py",
                "leakage_policy": "target_side_only",
            }
        )
    pairs.append(
        {
            "schema_version": 1,
            "artifact_kind": "commitpackft_python_pair",
            "pair_id": "fixture-missing-after",
            "bug_fix_label": True,
            "split": "test",
            "repo": "fixture/repo",
            "old_file": "pkg/missing.py",
            "new_file": "pkg/missing.py",
            "leakage_policy": "target_side_only",
        }
    )
    write_jsonl(pairs_path, pairs)

    shard_root = root / "shards" / "shard-000000-offset-000000000"
    packed_root = shard_root / "packed_records"
    packed_root.mkdir(parents=True, exist_ok=True)
    per_embedder = {}
    for embedder_id in embedders:
        record_path = packed_root / f"{embedder_id}.jsonl"
        index_path = packed_root / f"{embedder_id}.index.jsonl"
        rows = []
        index_rows = []
        for pair in pairs:
            pair_id = pair["pair_id"]
            sides = ["before"] if pair_id == "fixture-missing-after" else ["before", "after"]
            for side in sides:
                kind = "before" if side == "before" else ("bug_after" if pair["bug_fix_label"] else "non_after")
                vector = fixture_vector(kind)
                observed = vector_sha256(vector)
                row = {
                    "key": {"embedder_id": embedder_id, "chunk_id": f"{pair_id}-{side}"},
                    "chunk": {
                        "task_instance_id": pair_id,
                        "workspace_state": f"commit_{side}",
                        "source": f"contextgraph://fixture/{pair_id}/commit_{side}",
                        "relative_path": pair["old_file"],
                        "mutation_category": "bug_fix" if pair["bug_fix_label"] else "non_bug_fix",
                    },
                    "output": {"vector": vector, "vector_sha256": observed},
                }
                rows.append(row)
                index_rows.append(
                    {
                        "embedder_id": embedder_id,
                        "vector_sha256": observed,
                        "input_sha256": sha256_bytes(pair_id.encode("utf-8")),
                    }
                )
        write_jsonl(record_path, rows)
        write_jsonl(index_path, index_rows)
        per_embedder[embedder_id] = {
            "record_path": str(record_path),
            "record_sha256": sha256_file(record_path),
            "index_path": str(index_path),
            "index_sha256": sha256_file(index_path),
            "row_count": len(rows),
        }
    write_json(shard_root / "python_forward_cache_packed_manifest.json", {
        "schema_version": 1,
        "artifact_kind": "python_forward_cache_packed_manifest",
        "layout": "packed-jsonl",
        "per_embedder": per_embedder,
        "row_count": sum(info["row_count"] for info in per_embedder.values()),
        "passes": True,
    })
    write_json(shard_root / "python_forward_cache_manifest.json", {
        "schema_version": 1,
        "artifact_kind": "python_forward_cache_manifest",
        "passes": True,
        "summary": {"chunk_count": len(pairs) * 2 - 1, "active_embedder_ids": list(embedders)},
    })
    shard_plan = {
        "schema_version": 1,
        "shards": [{"shard_id": "shard-000000-offset-000000000", "cache_root": str(shard_root)}],
    }
    write_json(root / "python_forward_cache_shard_plan.json", shard_plan)
    write_json(root / "python_forward_cache_checkpoint.json", {
        "schema_version": 1,
        "artifact_kind": "python_forward_shard_scheduler_checkpoint",
        "passes": True,
        "summary": {"shard_count": 1},
    })
    write_json(root / "python_forward_cache_corpus_manifest.json", {
        "schema_version": 1,
        "artifact_kind": "python_forward_cache_corpus_manifest",
        "passes": True,
        "summary": {"active_embedder_ids": list(embedders), "retired_embedder_ids": []},
    })


def run_self_test(args: argparse.Namespace) -> int:
    root = args.self_test_root or Path(tempfile.mkdtemp(prefix="causal-embedder-auc-self-test-"))
    if args.clean and root.exists():
        shutil.rmtree(root)
    root.mkdir(parents=True, exist_ok=True)
    embedders = ("e1", "e6")
    forward_root = root / "forward"
    pairs_path = root / "pairs.jsonl"
    write_fixture_forward_root(forward_root, pairs_path, embedders)
    run_args = argparse.Namespace(
        forward_root=forward_root,
        pairs_jsonl=pairs_path,
        output_root=root / "fsv",
        state_root=root / "state",
        run_id="run-self-test",
        expected_embedders=",".join(embedders),
        max_labels_per_side=16,
        simhash_bits=4,
        simhash_probes_per_bit=2,
        smoothing=0.5,
        max_records_per_embedder=None,
        clean=True,
        allow_non_aiwonder_roots=True,
    )
    readback = run_pipeline(run_args)
    manifest = read_json(root / "fsv" / "run-self-test" / "causal_embedder_auc_manifest.json")
    edge_cases = [
        {
            "case": "missing_after_side_excluded",
            "before": "fixture-missing-after has only commit_before records",
            "after": {
                embedder: row["missing_side_pair_count"]
                for embedder, row in manifest["per_embedder_metrics"].items()
            },
            "pass": all(row["missing_side_pair_count"] >= 1 for row in manifest["per_embedder_metrics"].values()),
        },
        {
            "case": "target_labels_not_live_cluster_labels",
            "before": "pairs JSONL carries target_side_only bug_fix_label",
            "after": manifest["leakage_policy"],
            "pass": manifest["leakage_policy"]["target_label_prefix_violation_count"] == 0
            and manifest["leakage_policy"]["target_labels_used_as_live_inputs"] is False,
        },
        {
            "case": "per_embedder_auc_computed",
            "before": "fixture has train/validation/test positive and negative pairs",
            "after": manifest["summary"],
            "pass": manifest["summary"]["test_embedder_auc_count"] == len(embedders),
        },
    ]
    bad_forward = root / "bad-forward"
    bad_pairs = root / "bad-pairs.jsonl"
    write_fixture_forward_root(bad_forward, bad_pairs, ("e1",))
    bad_record_path = bad_forward / "shards" / "shard-000000-offset-000000000" / "packed_records" / "e1.jsonl"
    rows = [row for _line, row in iter_jsonl(bad_record_path)]
    rows[0]["output"]["vector_sha256"] = "0" * 64
    write_jsonl(bad_record_path, rows)
    try:
        bad_pairs_loaded = load_pairs(bad_pairs)
        ingest_forward_records(
            forward_root=bad_forward,
            pairs=bad_pairs_loaded,
            expected_embedders=("e1",),
            simhash_bits=4,
            simhash_probes_per_bit=2,
            max_records_per_embedder=None,
        )
        hash_pass = False
        hash_after = "unexpected success"
    except CausalEmbedderAucError as exc:
        hash_pass = "CAUSAL_AUC_VECTOR_SHA_MISMATCH" in str(exc)
        hash_after = str(exc)
    edge_cases.append(
        {
            "case": "vector_hash_mismatch_rejected",
            "before": "first packed record vector_sha256 deliberately corrupted",
            "after": hash_after,
            "pass": hash_pass,
        }
    )
    report = {
        "passes": bool(readback["passes"]) and all(row["pass"] for row in edge_cases),
        "readback": readback,
        "edge_cases": edge_cases,
        "self_test_root": str(root),
    }
    write_json(root / "self_test_report.json", report)
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0 if report["passes"] else 1


def add_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--forward-root", type=Path, default=DEFAULT_FORWARD_ROOT)
    parser.add_argument("--pairs-jsonl", type=Path, default=DEFAULT_PAIRS_JSONL)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--state-root", type=Path, default=DEFAULT_STATE_ROOT)
    parser.add_argument("--run-id", default=None)
    parser.add_argument("--expected-embedders", default=",".join(DEFAULT_ACTIVE_EMBEDDERS))
    parser.add_argument("--max-labels-per-side", type=int, default=48)
    parser.add_argument("--simhash-bits", type=int, default=8)
    parser.add_argument("--simhash-probes-per-bit", type=int, default=4)
    parser.add_argument("--smoothing", type=float, default=0.5)
    parser.add_argument("--max-records-per-embedder", type=int, default=None)
    parser.add_argument("--clean", action="store_true")
    parser.add_argument("--allow-non-aiwonder-roots", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--self-test-root", type=Path, default=None)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    add_arguments(parser)
    args = parser.parse_args(argv)
    if args.self_test:
        return run_self_test(args)
    run_pipeline(args)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
