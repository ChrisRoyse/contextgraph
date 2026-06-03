"""TASK-PY-G-117 deterministic auto-label discovery and usefulness scoring.

The command materializes a reusable baseline label compiler over the aiwonder
Python corpus.  It preserves embedder slot identity: vectors are reduced to
per-slot scalar/region labels, and cross-embedder meaning is expressed through
pair/group relationship labels rather than a flat fused vector.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import re
import shutil
import sys
from collections import Counter, defaultdict
from dataclasses import dataclass, field
from itertools import combinations
from pathlib import Path
from typing import Any, Iterable

import numpy as np

from .core import SwebenchOpsError, normalize_category
from .forward_cache_lineage import guard_packed_record_lineage
from .python_real_corpus_constellation_smoke import (
    DEFAULT_CHUNKS_JSONL,
    DEFAULT_FORWARD_ROOT,
    EPS,
    EXPECTED_EMBEDDERS,
    finite_float,
    parse_embedder_list,
    sha256_text,
    vector_scalar_features,
)
from .smoke_intelligence_matrices import (
    dump_json,
    iter_jsonl,
    read_json,
    row_key_from_fields,
    sha256_bytes,
    sha256_file,
    tree_state,
    utc_now,
)


SCHEMA_VERSION = 1
TASK_ID = "TASK-PY-G-117"
ARTIFACT_KIND = "python_auto_label_discovery"
FORMULA_VERSION = "deterministic_slot_label_compiler_v1"

DEFAULT_CORPUS_INDEX_JSON = Path("/zfs/archive/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/index.json")
DEFAULT_SWEBENCH_ROOT = Path("/zfs/archive/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511")
DEFAULT_MANUAL_REVIEW = Path(
    "/zfs/archive/contextgraph/readback/task-py-g-110-manual-pattern-review/"
    "run-20260519T141122Z/manual_pattern_review.json"
)
DEFAULT_OUTPUT_ROOT = Path("/zfs/archive/contextgraph/state/labels/task-py-g-117-auto-label-discovery-v1")
DEFAULT_READBACK_ROOT = Path("/zfs/archive/contextgraph/readback/task-py-g-117-auto-label-discovery-usefulness-readback")

DEFAULT_EXPECTED_CHUNK_COUNT = 399_777
DEFAULT_EXPECTED_FORWARD_RECORD_COUNT = 4_665_772
DEFAULT_EXPECTED_EMBEDDER_COUNT = 12
DEFAULT_MIN_SUPPORT = 20
DEFAULT_MIN_LIFT = 0.05

SCALAR_FEATURES = ("norm_l2", "std", "abs_mean", "entropy_proxy", "zero_fraction")
TARGET_LEAKY_FAMILIES = {
    "oracle",
    "oracle_exception",
    "docker",
    "test_phase",
    "test_result",
    "code_state_outcome",
    "partition",
    "leakage_policy",
    "ground_truth",
    "truth",
    "post_tool_use",
}
CORPUS_ONLY_LIVE_FAMILIES = {
    "code_state_identity",
    "mutation",
    "mutation_micro",
    "mutation_mechanism",
}
GROUP_SLOT_SETS = {
    "temporal_triad": ("e2", "e3", "e4"),
    "semantic_content_panel": ("e1", "e7", "e8", "e10", "e12", "e14"),
    "sparse_lexical_panel": ("e6", "e13"),
    "sparse_hdc_bridge": ("e6", "e9", "e13"),
}


class AutoLabelDiscoveryError(SwebenchOpsError):
    def __init__(
        self,
        code: str,
        detail: str | None = None,
        *,
        remediation: str = "Inspect the generated readback/report artifacts, fix the listed source-of-truth blocker, and rerun the label compiler.",
        details: dict[str, Any] | None = None,
    ):
        message = code if detail is None else f"{code}: {detail}"
        super().__init__(code, message, remediation, details)


@dataclass
class LabelStats:
    label_id: str
    family: str
    support: int = 0
    fail_count: int = 0
    pass_count: int = 0
    leaky_target: bool = False
    scope_counts: Counter[str] = field(default_factory=Counter)
    category_counts: Counter[str] = field(default_factory=Counter)
    category_fail_counts: Counter[str] = field(default_factory=Counter)
    docker_counts: Counter[str] = field(default_factory=Counter)
    partition_counts: Counter[str] = field(default_factory=Counter)
    partition_fail_counts: Counter[str] = field(default_factory=Counter)
    examples: list[str] = field(default_factory=list)

    def observe(
        self,
        *,
        failed: bool,
        category: str,
        docker_label: str,
        partition: str,
        scope: str,
        row_key: str,
        leaky_target: bool,
    ) -> None:
        self.support += 1
        self.fail_count += 1 if failed else 0
        self.pass_count += 0 if failed else 1
        self.leaky_target = self.leaky_target or leaky_target
        self.scope_counts[scope] += 1
        self.category_counts[category] += 1
        self.category_fail_counts[category] += 1 if failed else 0
        self.docker_counts[docker_label] += 1
        self.partition_counts[partition] += 1
        self.partition_fail_counts[partition] += 1 if failed else 0
        if len(self.examples) < 5:
            self.examples.append(row_key)


@dataclass
class AggregateState:
    group_key: str
    scope: str
    task_instance_id: str
    mutation_category: str
    workspace_state: str
    repo: str
    relative_path: str | None
    docker_label: str
    oracle_all_passed: bool | None
    chunk_count: int = 0
    file_paths: set[str] = field(default_factory=set)
    surface_counts: Counter[str] = field(default_factory=Counter)
    slot_label_counts: Counter[str] = field(default_factory=Counter)
    pair_label_counts: Counter[str] = field(default_factory=Counter)
    group_label_counts: Counter[str] = field(default_factory=Counter)
    slot_feature_sums: dict[str, Counter[str]] = field(default_factory=lambda: defaultdict(Counter))
    slot_feature_counts: Counter[str] = field(default_factory=Counter)
    chunk_key_sample: list[str] = field(default_factory=list)

    def observe(
        self,
        row_key: str,
        chunk: dict[str, Any],
        surface_labels: Iterable[str],
        slot_labels: Iterable[str],
        pair_labels: Iterable[str],
        group_labels: Iterable[str],
        slot_features: dict[str, dict[str, float]],
    ) -> None:
        self.chunk_count += 1
        rel = str(chunk.get("relative_path") or "unknown")
        self.file_paths.add(rel)
        self.surface_counts.update(surface_labels)
        self.slot_label_counts.update(slot_labels)
        self.pair_label_counts.update(pair_labels)
        self.group_label_counts.update(group_labels)
        for embedder_id, features in slot_features.items():
            self.slot_feature_counts[embedder_id] += 1
            for name in SCALAR_FEATURES:
                if name in features:
                    self.slot_feature_sums[embedder_id][name] += float(features[name])
        if len(self.chunk_key_sample) < 5:
            self.chunk_key_sample.append(row_key)


@dataclass
class FailureCandidate:
    row_key: str
    chunk_id: str
    relative_path: str
    byte_start: int
    byte_end: int
    node_kind: str
    source_text_preview: str
    score: float
    reasons: list[str]
    labels: list[str]


@dataclass
class CodeStateFailureEvidence:
    code_state_key: str
    task_instance_id: str
    mutation_category: str
    workspace_state: str
    docker_label: str
    oracle_all_passed: bool | None
    source_relative_path: str | None
    candidates: list[FailureCandidate] = field(default_factory=list)


def require(condition: bool, code: str, detail: str | None = None) -> None:
    if not condition:
        raise AutoLabelDiscoveryError(code, detail)


def canonical_json(value: Any) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def stable_id(prefix: str, payload: Any) -> str:
    return f"{prefix}:{hashlib.sha256(canonical_json(payload).encode('utf-8')).hexdigest()[:24]}"


def write_jsonl_iter(path: Path, rows: Iterable[dict[str, Any]]) -> int:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    count = 0
    with tmp.open("w", encoding="utf-8") as fh:
        for row in rows:
            fh.write(json.dumps(row, sort_keys=True, separators=(",", ":")))
            fh.write("\n")
            count += 1
    tmp.replace(path)
    return count


def append_jsonl(fh: Any, row: dict[str, Any]) -> None:
    fh.write(json.dumps(row, sort_keys=True, separators=(",", ":")))
    fh.write("\n")


def parse_mutation_note(note: str) -> dict[str, Any]:
    out: dict[str, Any] = {}
    if note == "official SWE-bench patch used unchanged":
        out["micro_label"] = "known_good"
        out["source_relative_path"] = None
        return out
    if "source_site_note=" in (note or ""):
        out["source_site_note_full"] = note.split("source_site_note=", 1)[1].strip()
    for key in (
        "category",
        "repo",
        "base_commit",
        "source_path",
        "candidate_scope",
        "patch_builder",
        "source_site_offset",
        "source_site_len",
        "source_site_note",
    ):
        match = re.search(rf"(?:^|\s){re.escape(key)}=([^\s]+)", note or "")
        if match:
            out[key] = match.group(1)
    out["micro_label"] = normalize_category(str(out.get("category") or "unknown"))
    source_path = str(out.get("source_path") or "")
    if source_path.startswith("file:"):
        out["source_relative_path"] = source_path[5:]
    return out


def mutation_micro_labels(entry: dict[str, Any] | None) -> tuple[dict[str, Any], list[str]]:
    parsed = parse_mutation_note(str((entry or {}).get("mutation_note") or ""))
    category = normalize_category(str((entry or {}).get("category") or parsed.get("category") or "unknown"))
    note = str(parsed.get("source_site_note_full") or parsed.get("source_site_note") or "")
    if category == "delete_test_call":
        micro = "assertion_or_test_call_deleted"
    elif category == "compile_error":
        micro = "syntax_or_compile_break"
    elif category == "wrong_file" or category == "over_engineer" or note.startswith("over_engineer:"):
        micro = "extra_surface_complexity"
    elif category == "swap_variable":
        micro = "variable_reference_swap"
    elif category == "off_by_one":
        micro = "numeric_boundary_shift"
    elif category == "subtle_flip" and "equality flipped to inequality" in note:
        micro = "equality_to_inequality_flip"
    elif category == "subtle_flip" and "flipped to" in note:
        micro = "subtle_flip_flipped_to"
    else:
        micro = normalize_category(str(parsed.get("micro_label") or category))
    labels = [f"mutation:{normalize_category(str((entry or {}).get('category') or micro))}", f"mutation_micro:{micro}"]
    if "==" in note or "!=" in note:
        labels.append("mutation_mechanism:equality_operator_boundary")
    if any(op in note for op in ("<=", ">=", "<", ">")):
        labels.append("mutation_mechanism:numeric_or_ordering_boundary")
    if category == "wrong_file":
        labels.append("mutation_mechanism:wrong_file_edit")
    if category == "over_engineer" or note.startswith("over_engineer:"):
        labels.append("mutation_mechanism:extra_surface_complexity")
    if "source_site_offset" in parsed:
        labels.append("mutation_mechanism:source_site_located")
    if parsed.get("candidate_scope") == "alternate_tracked_python":
        labels.append("mutation_mechanism:wrong_file_candidate_scope")
    return parsed, sorted(set(labels))


def source_site_relation_labels(chunk: dict[str, Any], parsed_note: dict[str, Any], category: str) -> list[str]:
    rel = str(chunk.get("relative_path") or "")
    workspace_state = str(chunk.get("workspace_state") or "")
    source_relative_path = str(parsed_note.get("source_relative_path") or "")
    labels: list[str] = []
    if workspace_state == "test_patch":
        labels.append("source_site_relation:test_patch_surface")
    if category == "wrong_file" or parsed_note.get("candidate_scope") == "alternate_tracked_python":
        labels.append("source_site_relation:wrong_file_edit")
    if not source_relative_path:
        labels.append("source_site_relation:no_compact_source_site")
        return sorted(set(labels))

    if rel == source_relative_path:
        labels.append("source_site_relation:changed_file_context")
        offset = int(parsed_note.get("source_site_offset") or -1)
        length = int(parsed_note.get("source_site_len") or 0)
        start = int(chunk.get("byte_start") or -1)
        end = int(chunk.get("byte_end") or -1)
        site_end = offset + max(1, length)
        if offset >= 0 and start >= 0 and (start <= offset < end or start < site_end <= end or (offset <= start and end <= site_end)):
            labels.append("source_site_relation:exact_patch_line_overlap")
        elif offset >= 0 and start >= 0:
            distance = min(abs(start - offset), abs(end - site_end))
            if distance <= 500:
                labels.append("source_site_relation:near_patch_site_500b")
            elif distance <= 2_000:
                labels.append("source_site_relation:near_patch_site_2kb")
            else:
                labels.append("source_site_relation:same_file_distant_context")
    else:
        labels.append("source_site_relation:other_file_context")
        if category == "wrong_file":
            labels.append("source_site_relation:missing_expected_target_site")
    return sorted(set(labels))


def load_oracle_index(path: Path) -> tuple[dict[tuple[str, str], dict[str, Any]], dict[str, Any]]:
    require(path.exists(), "MEJEPA_AUTO_LABEL_ORACLE_INDEX_MISSING", str(path))
    payload = read_json(path)
    entries = list(payload.get("entries") or [])
    require(bool(entries), "MEJEPA_AUTO_LABEL_ORACLE_INDEX_EMPTY", str(path))
    out: dict[tuple[str, str], dict[str, Any]] = {}
    counts = Counter()
    buckets = Counter()
    for entry in entries:
        task_id = str(entry.get("task_id") or "")
        category = normalize_category(str(entry.get("category") or ""))
        require(task_id and category, "MEJEPA_AUTO_LABEL_ORACLE_INDEX_KEY_MISSING")
        key = (task_id, category)
        require(key not in out, "MEJEPA_AUTO_LABEL_ORACLE_INDEX_DUPLICATE", f"{task_id}:{category}")
        out[key] = entry
        counts["pass" if bool(entry.get("oracle_all_passed")) else "fail_or_error"] += 1
        buckets[str(entry.get("bucket") or "unknown")] += 1
    return out, {
        "path": str(path),
        "sha256": sha256_file(path),
        "entry_count": len(entries),
        "oracle_counts": dict(sorted(counts.items())),
        "bucket_counts": dict(sorted(buckets.items())),
        "complete": bool(payload.get("complete")),
    }


def report_category(path: Path) -> str | None:
    for part in path.parts:
        if part.startswith("mejepa-verify-"):
            return normalize_category(part.removeprefix("mejepa-verify-"))
        if part.startswith("mejepa-") and not part.startswith("mejepaPhase"):
            return normalize_category(part.removeprefix("mejepa-"))
    return None


def load_docker_reports(root: Path) -> tuple[dict[tuple[str, str], dict[str, Any]], dict[str, Any]]:
    require(root.exists(), "MEJEPA_AUTO_LABEL_SWEBENCH_ROOT_MISSING", str(root))
    by_key: dict[tuple[str, str], dict[str, Any]] = {}
    scan_counts = Counter()
    for path in sorted(root.rglob("report.json")):
        category = report_category(path)
        if not category:
            scan_counts["unparsed_category"] += 1
            continue
        try:
            payload = json.loads(path.read_text(encoding="utf-8"))
        except json.JSONDecodeError:
            scan_counts["bad_json"] += 1
            continue
        for task_id, row in payload.items():
            key = (str(task_id), category)
            current = by_key.get(key)
            if current is None or str(path) > str(current.get("_report_path")):
                row = dict(row)
                row["_report_path"] = str(path)
                by_key[key] = row
            scan_counts["report_rows"] += 1
    label_counts = Counter()
    for row in by_key.values():
        label, _phase = docker_outcome(row)
        label_counts[label] += 1
    return by_key, {
        "root": str(root),
        "report_rows_scanned": scan_counts["report_rows"],
        "latest_report_count": len(by_key),
        "scan_counts": dict(sorted(scan_counts.items())),
        "docker_label_counts": dict(sorted(label_counts.items())),
    }


def docker_outcome(row: dict[str, Any] | None) -> tuple[str, dict[str, Any]]:
    if row is None:
        return "docker_report_missing", {}
    statuses = row.get("tests_status") or {}
    phase: dict[str, Any] = {}
    for name in ("FAIL_TO_PASS", "PASS_TO_PASS", "FAIL_TO_FAIL", "PASS_TO_FAIL"):
        bucket = statuses.get(name) or {}
        failures = list(bucket.get("failure") or [])
        successes = list(bucket.get("success") or [])
        phase[name] = {
            "success_count": len(successes),
            "failure_count": len(failures),
            "failure_sample_sha256": [sha256_text(item) for item in failures[:5]],
        }
    if not bool(row.get("patch_successfully_applied", True)):
        return "patch_apply_failure", phase
    if bool(row.get("resolved")):
        return "docker_resolved_clean", phase
    ftp_failed = phase["FAIL_TO_PASS"]["failure_count"] > 0
    ptp_failed = phase["PASS_TO_PASS"]["failure_count"] > 0
    if ftp_failed and ptp_failed:
        return "target_and_regression_tests_failed", phase
    if ftp_failed:
        return "target_fail_to_pass_tests_failed", phase
    if ptp_failed:
        return "regression_pass_to_pass_tests_failed", phase
    return "docker_unresolved_without_test_failures", phase


def stable_partition(row_key: str) -> str:
    bucket = int(hashlib.sha256(row_key.encode("utf-8")).hexdigest()[:8], 16) % 10
    if bucket == 0:
        return "holdout"
    if bucket == 1:
        return "calibration"
    return "train"


def numeric_precision_surface_labels(text: str) -> list[str]:
    lowered = text.lower()
    labels: list[str] = []
    if re.search(r"\b\d+\.\d+\b", text) or re.search(r"\b(float|decimal)\b", lowered):
        labels.append("numeric_precision:float_literal_or_decimal_surface")
    if any(token in lowered for token in ("np.", "numpy", "dtype", "astype", "float32", "float64")):
        labels.append("numeric_precision:dtype_or_numpy_surface")
    if any(token in lowered for token in ("isclose", "allclose", "rtol", "atol", "epsilon", "eps", "precision", "tolerance")):
        labels.append("numeric_precision:tolerance_threshold_surface")
    if re.search(r"\b(isnan|isinf|nan|inf)\b", lowered):
        labels.append("numeric_precision:nan_inf_surface")
    if any(token in lowered for token in ("sum(", "cumsum", "accumulate", "mean(", "std(", "variance", "var(")):
        labels.append("numeric_precision:accumulation_order_surface")
    if labels:
        labels.append("numeric_precision:floating_point_precision_surface")
    return sorted(set(labels))


def resource_lifecycle_surface_labels(text: str) -> list[str]:
    lowered = text.lower()
    labels: list[str] = []
    if any(token in lowered for token in ("open(", ".open(", "close(", ".close(", "closed")):
        labels.append("resource_lifecycle:open_close_surface")
    if re.search(r"\bwith\b", lowered):
        labels.append("resource_lifecycle:context_manager_surface")
    if any(
        token in lowered
        for token in (
            "socket",
            "session",
            "connection",
            "cursor",
            "file",
            "fd",
            "handle",
            "tempfile",
            "stream",
        )
    ):
        labels.append("resource_lifecycle:resource_handle_surface")
    if any(
        token in lowered
        for token in (
            "finally",
            "cleanup",
            "tear_down",
            "teardown",
            "dispose",
            "release",
            "flush",
            "commit",
            "rollback",
        )
    ):
        labels.append("resource_lifecycle:cleanup_release_surface")
    if labels:
        labels.append("resource_lifecycle:resource_leak_surface")
    return sorted(set(labels))


def source_surface_labels(row: dict[str, Any]) -> list[str]:
    text = str(row.get("source_text") or "").strip()
    rel = str(row.get("relative_path") or "").lower()
    node = str(row.get("node_kind") or "")
    chunk_kind = str(row.get("chunk_kind") or "")
    labels: list[str] = []
    if chunk_kind == "syntax_error_file" or node == "SyntaxError":
        labels.append("ast_surface:syntax_error_file")
    if node in {"Import", "ImportFrom"} or text.startswith(("import ", "from ")):
        labels.append("ast_surface:import_dependency_surface")
    if node == "Assert" or text.startswith("assert ") or " assert " in text:
        labels.append("ast_surface:assertion_surface")
    if node in {"FunctionDef", "AsyncFunctionDef"}:
        labels.append("ast_surface:function_contract_surface")
    if node == "ClassDef":
        labels.append("ast_surface:class_type_surface")
    if node in {"If", "For", "While", "Try", "With", "ExceptHandler"} or re.search(
        r"\b(if|for|while|try|with|except)\b", text
    ):
        labels.append("ast_surface:control_flow_surface")
    if node in {"Try", "ExceptHandler", "Raise"} or "except " in text or "raise " in text:
        labels.append("ast_surface:exception_handling_surface")
    if node == "Return" or text.startswith("return ") or re.search(r"\breturn\b", text):
        labels.append("ast_surface:return_value_surface")
    if node == "Call" or ("(" in text and ")" in text):
        labels.append("ast_surface:call_api_surface")
    if node in {"Assign", "AnnAssign", "AugAssign"} or "=" in text:
        labels.append("ast_surface:assignment_state_surface")
    if any(ch.isdigit() for ch in text) or any(token in text for token in ("'", '"', "True", "False", "None")):
        labels.append("ast_surface:literal_config_surface")
    if any(op in text for op in ("<=", ">=", "==", "!=", "<", ">")) or re.search(r"\b\d+\b", text):
        labels.append("ast_surface:numeric_boundary_or_comparison_surface")
    if any(token in text for token in (" and ", " or ", " not ", "True", "False")):
        labels.append("ast_surface:boolean_logic_surface")
    if "test" in rel or text.startswith("assert "):
        labels.append("ast_surface:test_oracle_surface")
    if "settings" in rel or "config" in rel:
        labels.append("path_surface:configuration_path")
    if "test" in rel:
        labels.append("path_surface:test_path")
    labels.extend(numeric_precision_surface_labels(text))
    labels.extend(resource_lifecycle_surface_labels(text))
    if not labels:
        labels.append("ast_surface:generic_code_surface")
    return sorted(set(labels))


def bucket_count(value: int) -> str:
    if value <= 1:
        return "one"
    if value <= 4:
        return "few"
    if value <= 16:
        return "small"
    if value <= 64:
        return "medium"
    if value <= 256:
        return "large"
    return "very_large"


def scalar_bucket(feature: str, value: float) -> str:
    if feature == "zero_fraction":
        if value <= 0.001:
            return "zero_none"
        if value < 0.05:
            return "zero_low"
        if value < 0.20:
            return "zero_medium"
        return "zero_heavy"
    if feature == "entropy_proxy":
        if value < 0.75:
            return "entropy_low"
        if value < 0.92:
            return "entropy_mid"
        return "entropy_high"
    if feature == "norm_l2":
        if value < 0.90:
            return "norm_low"
        if value <= 1.10:
            return "norm_unit_band"
        return "norm_high"
    if feature in {"std", "abs_mean"}:
        if value < 0.005:
            return f"{feature}_tiny"
        if value < 0.025:
            return f"{feature}_low"
        if value < 0.075:
            return f"{feature}_mid"
        return f"{feature}_high"
    return "unknown"


def slot_family_label(embedder_id: str) -> str:
    if embedder_id in {"e2", "e3", "e4"}:
        return f"slot:{embedder_id}:family:temporal_coordinate"
    if embedder_id in {"e6", "e13"}:
        return f"slot:{embedder_id}:family:sparse_lexical_coordinate"
    if embedder_id == "e9":
        return "slot:e9:family:hdc_sparse_coordinate"
    return f"slot:{embedder_id}:family:semantic_or_content_coordinate"


def top_abs_label(embedder_id: str, vector: np.ndarray) -> str:
    idx = int(np.argmax(np.abs(vector)))
    value = float(vector[idx])
    sign = "pos" if value >= 0.0 else "neg"
    bucket = idx // max(1, max(1, vector.size) // 16)
    return f"slot:{embedder_id}:top_abs:{sign}:bucket_{bucket:02d}"


def slot_labels(embedder_id: str, vector: np.ndarray, features: dict[str, float]) -> list[str]:
    labels = [
        f"slot:{embedder_id}:present",
        f"slot:{embedder_id}:dim:{int(vector.size)}",
        slot_family_label(embedder_id),
        top_abs_label(embedder_id, vector),
    ]
    for feature in SCALAR_FEATURES:
        labels.append(f"slot:{embedder_id}:{scalar_bucket(feature, float(features[feature]))}")
    return labels


def vector_space_health_from_hash_counts(
    embedder_ids: Iterable[str],
    row_counts: Counter[str],
    vector_hash_counts: dict[str, Counter[str]],
) -> list[dict[str, Any]]:
    hash_sets = {
        embedder_id: set(vector_hash_counts.get(embedder_id, Counter()).keys())
        for embedder_id in embedder_ids
    }
    first_owner_by_hash_set: dict[frozenset[str], str] = {}
    duplicate_of: dict[str, str] = {}
    for embedder_id in sorted(hash_sets):
        frozen = frozenset(hash_sets[embedder_id])
        if not frozen:
            continue
        owner = first_owner_by_hash_set.setdefault(frozen, embedder_id)
        if owner != embedder_id:
            duplicate_of[embedder_id] = owner

    rows: list[dict[str, Any]] = []
    for embedder_id in sorted(hash_sets):
        counts = vector_hash_counts.get(embedder_id, Counter())
        unique_count = len(hash_sets[embedder_id])
        row_count = int(row_counts.get(embedder_id, 0))
        status = "usable"
        blocker = None
        if row_count == 0:
            status = "missing"
            blocker = "NO_VECTOR_ROWS"
        elif unique_count <= 1:
            status = "degenerate"
            blocker = "DEGENERATE_VECTOR_SPACE_UNIQUE_VECTOR_SHA256_LE_ONE"
        elif embedder_id in duplicate_of:
            status = "duplicate"
            blocker = "DUPLICATE_VECTOR_HASH_SPACE"
        rows.append(
            {
                "artifact_kind": f"{ARTIFACT_KIND}_slot_vector_space_health",
                "schema_version": SCHEMA_VERSION,
                "formula_version": FORMULA_VERSION,
                "embedder_id": embedder_id,
                "row_count": row_count,
                "unique_vector_sha256_count": unique_count,
                "status": status,
                "blocker": blocker,
                "duplicate_of_embedder_id": duplicate_of.get(embedder_id),
                "top_vector_sha256_counts": [
                    {"vector_sha256": vector_sha, "count": count}
                    for vector_sha, count in counts.most_common(5)
                ],
            }
        )
    return rows


def vector_space_blockers(health_rows: list[dict[str, Any]]) -> list[dict[str, Any]]:
    return [
        row
        for row in health_rows
        if row.get("status") in {"missing", "degenerate", "duplicate"}
    ]


def pair_relationship(left_id: str, left: dict[str, float], right_id: str, right: dict[str, float]) -> str:
    deltas = {feature: abs(float(left[feature]) - float(right[feature])) for feature in SCALAR_FEATURES}
    driver, max_delta = max(deltas.items(), key=lambda item: item[1])
    if max_delta < 0.005:
        state = "near_identity_cross_slot_delta"
    elif max_delta < 0.020:
        state = "low_cross_slot_delta"
    elif max_delta < 0.050:
        state = "moderate_cross_slot_delta"
    elif max_delta < 0.100:
        state = "high_cross_slot_delta"
    else:
        state = "extreme_cross_slot_delta"
    return f"pair:{left_id}/{right_id}:{state}:driver:{driver}"


def group_signatures(slot_features: dict[str, dict[str, float]], pair_labels: list[str]) -> list[str]:
    labels: list[str] = []
    present = set(slot_features)
    for group_name, slots in GROUP_SLOT_SETS.items():
        if set(slots).issubset(present):
            labels.append(f"group:{group_name}:present")
    if {"e2", "e3", "e4"}.issubset(present):
        temporal_zero = [slot_features[slot]["zero_fraction"] for slot in ("e2", "e3", "e4")]
        if max(temporal_zero) - min(temporal_zero) < 0.02:
            labels.append("group:temporal_triad:stable_coordinate")
    if "e9" in present and slot_features["e9"]["zero_fraction"] >= 0.20:
        labels.append("group:sparse_hdc_bridge:e9_zero_heavy")
    if {"e6", "e13"}.issubset(present):
        if slot_features["e6"]["entropy_proxy"] < 0.92 and slot_features["e13"]["entropy_proxy"] < 0.92:
            labels.append("group:sparse_lexical_panel:low_entropy_consensus")
    if {"e1", "e8"}.issubset(present):
        near_e1_e8 = any(label.startswith("pair:e1/e8:near_identity") or label.startswith("pair:e1/e8:low_cross") for label in pair_labels)
        if near_e1_e8:
            labels.append("group:semantic_content_panel:e1_e8_redundant_surface")
    high_or_extreme = sum(1 for label in pair_labels if ":high_cross_slot_delta:" in label or ":extreme_cross_slot_delta:" in label)
    if high_or_extreme >= 8:
        labels.append(f"group:cross_panel:high_disagreement_cluster:{bucket_count(high_or_extreme)}")
    if not labels:
        labels.append("group:cross_panel:low_signal_background")
    return sorted(set(labels))


def failure_outcome(docker_label: str, oracle_all_passed: bool | None) -> bool:
    if docker_label == "docker_resolved_clean":
        return False
    if docker_label == "docker_report_missing" and oracle_all_passed is not None:
        return not bool(oracle_all_passed)
    return True


def outcome_labels(docker_label: str, phase: dict[str, Any], oracle_entry: dict[str, Any] | None) -> list[str]:
    labels = [
        f"docker:{docker_label}",
        "code_state_outcome:working_as_intended" if docker_label == "docker_resolved_clean" else "code_state_outcome:failed_or_problematic",
    ]
    if oracle_entry is not None:
        labels.append("oracle:pass" if bool(oracle_entry.get("oracle_all_passed")) else "oracle:fail_or_error")
        if oracle_entry.get("oracle_exception"):
            labels.append(f"oracle_exception:{str(oracle_entry.get('oracle_exception'))[:80]}")
    for bucket, item in phase.items():
        if int(item.get("failure_count") or 0) > 0:
            labels.append(f"test_phase:{bucket.lower()}:failed")
        if int(item.get("success_count") or 0) > 0:
            labels.append(f"test_phase:{bucket.lower()}:passed")
    return sorted(set(labels))


def group_key(task_id: str, category: str, workspace_state: str) -> str:
    return f"{task_id}|{category}|{workspace_state}"


def file_group_key(task_id: str, category: str, workspace_state: str, relative_path: str) -> str:
    return f"{task_id}|{category}|{workspace_state}|{relative_path}"


def failure_candidate_score(
    chunk: dict[str, Any],
    parsed_note: dict[str, Any],
    surface_labels: list[str],
    slot_labels_for_chunk: list[str],
    pair_labels_for_chunk: list[str],
) -> tuple[float, list[str]]:
    score = 0.0
    reasons: list[str] = []
    source_relative_path = parsed_note.get("source_relative_path")
    if source_relative_path and str(chunk.get("relative_path") or "") == str(source_relative_path):
        score += 3.0
        reasons.append("source_path_match")
        offset = int(parsed_note.get("source_site_offset") or -1)
        length = int(parsed_note.get("source_site_len") or 0)
        site_end = offset + max(1, length)
        start = int(chunk.get("byte_start") or -1)
        end = int(chunk.get("byte_end") or -1)
        if start <= offset < end or start < site_end <= end or (offset <= start and end <= site_end):
            score += 6.0
            reasons.append("source_site_overlap")
        elif offset >= 0 and start >= 0:
            distance = min(abs(start - offset), abs(end - site_end))
            if distance <= 500:
                score += 2.0
                reasons.append("source_site_near_500b")
            elif distance <= 2_000:
                score += 0.75
                reasons.append("source_site_near_2kb")
    risk_surfaces = {
        "ast_surface:assertion_surface",
        "ast_surface:numeric_boundary_or_comparison_surface",
        "ast_surface:boolean_logic_surface",
        "ast_surface:return_value_surface",
        "ast_surface:assignment_state_surface",
    }
    if any(label in risk_surfaces for label in surface_labels):
        score += 0.5
        reasons.append("risk_surface")
    if any("zero_heavy" in label or "entropy_low" in label for label in slot_labels_for_chunk):
        score += 0.25
        reasons.append("slot_abnormality")
    high_pairs = sum(1 for label in pair_labels_for_chunk if ":high_cross_slot_delta:" in label or ":extreme_cross_slot_delta:" in label)
    if high_pairs >= 8:
        score += 0.5
        reasons.append("pair_disagreement_cluster")
    return finite_float(score), reasons


def localization_mode(candidates: list[FailureCandidate], failed: bool) -> str:
    strong = [candidate for candidate in candidates if candidate.score >= 6.0]
    moderate = [candidate for candidate in candidates if candidate.score >= 3.0]
    if len(strong) == 1:
        return "localization:single_point_candidate"
    if len(strong) > 1 or len(moderate) >= 3:
        return "localization:multi_point_evidence_set"
    if failed:
        return "localization:diffuse_code_state_risk"
    return "localization:working_state_no_failure_localization"


def active_input_preflight(args: argparse.Namespace) -> None:
    require(args.forward_root.exists(), "MEJEPA_AUTO_LABEL_FORWARD_ROOT_MISSING", str(args.forward_root))
    require(args.chunks_jsonl.exists(), "MEJEPA_AUTO_LABEL_CHUNKS_JSONL_MISSING", str(args.chunks_jsonl))
    require(args.corpus_index_json.exists(), "MEJEPA_AUTO_LABEL_ORACLE_INDEX_MISSING", str(args.corpus_index_json))
    require(args.swebench_root.exists(), "MEJEPA_AUTO_LABEL_SWEBENCH_ROOT_MISSING", str(args.swebench_root))
    require(args.min_support > 0, "MEJEPA_AUTO_LABEL_MIN_SUPPORT_INVALID", str(args.min_support))
    require(args.min_lift >= 0.0, "MEJEPA_AUTO_LABEL_MIN_LIFT_INVALID", str(args.min_lift))


def select_shards(forward_root: Path, max_shards: int | None) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    checkpoint_path = forward_root / "python_forward_cache_checkpoint.json"
    plan_path = forward_root / "python_forward_cache_shard_plan.json"
    manifest_path = forward_root / "python_forward_cache_corpus_manifest.json"
    checkpoint = read_json(checkpoint_path)
    plan = read_json(plan_path)
    manifest = read_json(manifest_path)
    require(checkpoint.get("passes") is True, "MEJEPA_AUTO_LABEL_CHECKPOINT_NOT_PASSING")
    require(manifest.get("passes") is True, "MEJEPA_AUTO_LABEL_CORPUS_MANIFEST_NOT_PASSING")
    shards = list(plan.get("shards") or [])
    require(bool(shards), "MEJEPA_AUTO_LABEL_SHARD_PLAN_EMPTY")
    if max_shards is not None:
        require(max_shards > 0, "MEJEPA_AUTO_LABEL_MAX_SHARDS_INVALID", str(max_shards))
        shards = shards[:max_shards]
    return shards, {
        "checkpoint_path": str(checkpoint_path),
        "checkpoint_sha256": sha256_file(checkpoint_path),
        "shard_plan_path": str(plan_path),
        "shard_plan_sha256": sha256_file(plan_path),
        "corpus_manifest_path": str(manifest_path),
        "corpus_manifest_sha256": sha256_file(manifest_path),
        "corpus_manifest_summary": manifest.get("summary") or {},
        "manifest_chunk_count": manifest.get("chunk_count"),
        "manifest_cache_record_count": manifest.get("cache_record_count"),
        "selected_shard_count": len(shards),
        "full_shard_count": len(plan.get("shards") or []),
    }


def load_chunks(path: Path, max_rows: int | None = None) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    counts = {
        "repos": Counter(),
        "mutation_categories": Counter(),
        "workspace_states": Counter(),
        "chunk_kinds": Counter(),
    }
    for idx, row in enumerate(row for _line, row in iter_jsonl(path)):
        if max_rows is not None and idx >= max_rows:
            break
        task_id = str(row.get("task_instance_id") or "")
        workspace_state = str(row.get("workspace_state") or "")
        chunk_id = str(row.get("chunk_id") or "")
        source_sha256 = str(row.get("source_sha256") or "")
        require(task_id and workspace_state and chunk_id and source_sha256, "MEJEPA_AUTO_LABEL_CHUNK_KEY_MISSING", f"{path}:{idx + 1}")
        row["_row_key"] = row_key_from_fields(task_id, workspace_state, chunk_id, source_sha256)
        rows.append(row)
        counts["repos"][str(row.get("repo") or "unknown")] += 1
        counts["mutation_categories"][normalize_category(str(row.get("mutation_category") or "unknown"))] += 1
        counts["workspace_states"][workspace_state] += 1
        counts["chunk_kinds"][str(row.get("chunk_kind") or "unknown")] += 1
    return rows, {
        "path": str(path),
        "sha256": sha256_file(path),
        "rows_loaded": len(rows),
        "repos": dict(sorted(counts["repos"].items())),
        "mutation_categories": dict(sorted(counts["mutation_categories"].items())),
        "workspace_states": dict(sorted(counts["workspace_states"].items())),
        "chunk_kinds": dict(sorted(counts["chunk_kinds"].items())),
    }


def load_manual_targets(path: Path | None) -> tuple[dict[str, dict[str, Any]], dict[str, Any]]:
    if path is None or not path.exists():
        return {}, {"present": False, "path": str(path) if path is not None else None}
    payload = read_json(path)
    targets: dict[str, dict[str, Any]] = {}
    for sample in payload.get("samples") or []:
        chunk = sample.get("chunk") or {}
        join = sample.get("join_key") or {}
        task_id = str(join.get("task_instance_id") or "")
        workspace_state = str(chunk.get("workspace_state") or "")
        chunk_id = str(chunk.get("chunk_id") or "")
        source_sha256 = str(chunk.get("source_sha256") or "")
        if not (task_id and workspace_state and chunk_id and source_sha256):
            continue
        key = row_key_from_fields(task_id, workspace_state, chunk_id, source_sha256)
        targets[key] = sample
    return targets, {
        "present": True,
        "path": str(path),
        "sha256": sha256_file(path),
        "sample_count": len(targets),
        "artifact_kind": payload.get("artifact_kind"),
    }


def register_label(
    stats: dict[str, LabelStats],
    label_id: str,
    *,
    failed: bool,
    category: str,
    docker_label: str,
    partition: str,
    scope: str,
    row_key: str,
    leaky_target: bool,
) -> None:
    family = label_id.split(":", 1)[0]
    stats.setdefault(label_id, LabelStats(label_id=label_id, family=family)).observe(
        failed=failed,
        category=category,
        docker_label=docker_label,
        partition=partition,
        scope=scope,
        row_key=row_key,
        leaky_target=leaky_target or not is_live_predictor_label(label_id),
    )


def register_labels(
    stats: dict[str, LabelStats],
    labels: Iterable[str],
    *,
    failed: bool,
    category: str,
    docker_label: str,
    partition: str,
    scope: str,
    row_key: str,
    leaky_target: bool = False,
) -> None:
    for label in labels:
        register_label(
            stats,
            label,
            failed=failed,
            category=category,
            docker_label=docker_label,
            partition=partition,
            scope=scope,
            row_key=row_key,
            leaky_target=leaky_target,
        )


def is_live_predictor_label(label_id: str) -> bool:
    family = label_id.split(":", 1)[0]
    return family not in TARGET_LEAKY_FAMILIES and family not in CORPUS_ONLY_LIVE_FAMILIES


def split_live_and_supervision_labels(labels: Iterable[str]) -> tuple[list[str], list[str]]:
    live: list[str] = []
    supervision: list[str] = []
    for label in sorted(set(labels)):
        if is_live_predictor_label(label):
            live.append(label)
        else:
            supervision.append(label)
    return live, supervision


def aggregate_outcome_supervision_labels(agg: AggregateState) -> list[str]:
    labels = [f"docker:{agg.docker_label}"]
    if agg.oracle_all_passed is not None:
        labels.append("oracle:pass" if bool(agg.oracle_all_passed) else "oracle:fail_or_error")
    labels.append(
        "code_state_outcome:working_as_intended"
        if agg.docker_label == "docker_resolved_clean"
        else "code_state_outcome:failed_or_problematic"
    )
    return sorted(set(labels))


def code_state_labels(agg: AggregateState) -> list[str]:
    labels = [
        "group_scope:code_state",
        f"code_state_identity:repo:{normalize_category(agg.repo)}",
        f"code_state_identity:mutation_category:{agg.mutation_category}",
        f"code_state_identity:workspace_state:{normalize_category(agg.workspace_state)}",
        f"group_chunk_count:{bucket_count(agg.chunk_count)}",
        f"group_file_count:{bucket_count(len(agg.file_paths))}",
        f"code_state_outcome:{'working_as_intended' if agg.docker_label == 'docker_resolved_clean' else 'failed_or_problematic'}",
    ]
    labels.extend(f"dominant_surface:{label.split(':', 1)[1]}" for label, _count in agg.surface_counts.most_common(3))
    labels.extend(f"dominant_group_signature:{label.split(':', 1)[1]}" for label, _count in agg.group_label_counts.most_common(3))
    return sorted(set(labels))


def file_path_labels(agg: AggregateState) -> list[str]:
    labels = [
        "group_scope:file_path",
        f"code_state_identity:repo:{normalize_category(agg.repo)}",
        f"code_state_identity:mutation_category:{agg.mutation_category}",
        f"code_state_identity:workspace_state:{normalize_category(agg.workspace_state)}",
        f"group_chunk_count:{bucket_count(agg.chunk_count)}",
        f"code_state_outcome:{'working_as_intended' if agg.docker_label == 'docker_resolved_clean' else 'failed_or_problematic'}",
    ]
    labels.extend(f"dominant_surface:{label.split(':', 1)[1]}" for label, _count in agg.surface_counts.most_common(2))
    return sorted(set(labels))


def aggregate_row(agg: AggregateState, labels: list[str]) -> dict[str, Any]:
    slot_feature_means: dict[str, dict[str, float]] = {}
    for embedder_id, sums in sorted(agg.slot_feature_sums.items()):
        count = max(1, agg.slot_feature_counts[embedder_id])
        slot_feature_means[embedder_id] = {name: finite_float(float(sums[name]) / count) for name in SCALAR_FEATURES if name in sums}
    live_labels, supervision_labels = split_live_and_supervision_labels(labels)
    supervision_labels = sorted(set(supervision_labels + aggregate_outcome_supervision_labels(agg)))
    return {
        "artifact_kind": f"{ARTIFACT_KIND}_{agg.scope}_constellation",
        "schema_version": SCHEMA_VERSION,
        "formula_version": FORMULA_VERSION,
        "group_key": agg.group_key,
        "scope": agg.scope,
        "task_instance_id": agg.task_instance_id,
        "mutation_category": agg.mutation_category,
        "workspace_state": agg.workspace_state,
        "repo": agg.repo,
        "relative_path": agg.relative_path,
        "chunk_count": agg.chunk_count,
        "file_count": len(agg.file_paths),
        "docker_label": agg.docker_label,
        "oracle_all_passed": agg.oracle_all_passed,
        "labels": labels,
        "live_predictor_labels": live_labels,
        "target_supervision_labels": supervision_labels,
        "surface_counts_top": dict(agg.surface_counts.most_common(16)),
        "slot_label_counts_top": dict(agg.slot_label_counts.most_common(16)),
        "pair_label_counts_top": dict(agg.pair_label_counts.most_common(16)),
        "group_label_counts_top": dict(agg.group_label_counts.most_common(16)),
        "slot_feature_means": slot_feature_means,
        "chunk_key_sample": agg.chunk_key_sample,
        "slot_identity_preserved": True,
        "flat_vector_concat_used": False,
    }


def summarize_manual_reproduction(
    manual_targets: dict[str, dict[str, Any]],
    generated_manual_labels: dict[str, set[str]],
    *,
    require_all_samples: bool,
) -> dict[str, Any]:
    cases = []
    for row_key, sample in sorted(manual_targets.items()):
        generated = generated_manual_labels.get(row_key, set())
        expected: list[str] = []
        for label in sample.get("chunk_source_labels") or []:
            expected.append(f"ast_surface:{label}")
        micro = str((sample.get("mutation") or {}).get("micro_label") or "")
        if micro:
            expected.append(f"mutation_micro:{normalize_category(micro)}")
        docker_label = str((sample.get("docker") or {}).get("label") or "")
        if docker_label:
            expected.append(f"docker:{docker_label}")
        expected_present = sorted(set(label for label in expected if label))
        in_selected_window = row_key in generated_manual_labels
        missing = [label for label in expected_present if label not in generated] if in_selected_window else []
        cases.append(
            {
                "row_key": row_key,
                "in_selected_window": in_selected_window,
                "expected_labels": expected_present,
                "missing_expected_labels": missing,
                "generated_label_count": len(generated),
                "passes": (not missing) if in_selected_window else (not require_all_samples),
            }
        )
    return {
        "artifact_kind": f"{ARTIFACT_KIND}_manual_review_reproduction",
        "schema_version": SCHEMA_VERSION,
        "formula_version": FORMULA_VERSION,
        "manual_sample_count": len(manual_targets),
        "matched_sample_count": sum(1 for key in manual_targets if key in generated_manual_labels),
        "require_all_samples": require_all_samples,
        "passes": all(case["passes"] for case in cases) if cases else True,
        "cases": cases,
    }


def usefulness_rows(
    stats: dict[str, LabelStats],
    category_totals: Counter[str],
    category_fail_totals: Counter[str],
    min_support: int,
    min_lift: float,
) -> tuple[list[dict[str, Any]], list[dict[str, Any]], dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    accepted: list[dict[str, Any]] = []
    status_counts = Counter()
    for label_id, item in sorted(stats.items()):
        fail_rate = item.fail_count / item.support if item.support else 0.0
        weighted_baseline = 0.0
        if item.support:
            for category, count in item.category_counts.items():
                category_total = max(1, category_totals[category])
                weighted_baseline += (count / item.support) * (category_fail_totals[category] / category_total)
        lift = fail_rate - weighted_baseline
        partition_rates = {
            part: finite_float(item.partition_fail_counts[part] / count)
            for part, count in sorted(item.partition_counts.items())
            if count > 0
        }
        if item.leaky_target or not is_live_predictor_label(label_id):
            status = "rejected_leaky_target"
            reasons = ["target_or_supervision_label_not_live_predictor_input"]
        elif item.support < min_support:
            status = "rejected_low_support"
            reasons = [f"support<{min_support}"]
        elif len(item.partition_counts) < 2:
            status = "rejected_partition_underpowered"
            reasons = ["too_few_deterministic_partitions"]
        elif abs(lift) >= min_lift:
            status = "accepted_predictive_candidate"
            reasons = []
        else:
            status = "accepted_descriptive"
            reasons = ["lift_below_predictive_floor"]
        status_counts[status] += 1
        row = {
            "artifact_kind": f"{ARTIFACT_KIND}_label_usefulness_metric",
            "schema_version": SCHEMA_VERSION,
            "formula_version": FORMULA_VERSION,
            "label_id": label_id,
            "label_hash": stable_id("label", label_id),
            "family": item.family,
            "support": item.support,
            "fail_count": item.fail_count,
            "pass_count": item.pass_count,
            "fail_rate": finite_float(fail_rate),
            "weighted_category_baseline_fail_rate": finite_float(weighted_baseline),
            "lift_over_weighted_category_baseline": finite_float(lift),
            "partition_counts": dict(sorted(item.partition_counts.items())),
            "partition_fail_rates": partition_rates,
            "docker_counts": dict(sorted(item.docker_counts.items())),
            "scope_counts": dict(sorted(item.scope_counts.items())),
            "category_counts": dict(sorted(item.category_counts.items())),
            "leaky_target": item.leaky_target or item.family in TARGET_LEAKY_FAMILIES,
            "status": status,
            "rejection_or_status_reasons": reasons,
            "example_row_keys": item.examples,
            "online_update_policy": "rescore_on_new_durable_reality_outcomes",
        }
        rows.append(row)
        if status.startswith("accepted"):
            accepted.append(
                {
                    "artifact_kind": f"{ARTIFACT_KIND}_accepted_label_registry_row",
                    "schema_version": SCHEMA_VERSION,
                    "formula_version": FORMULA_VERSION,
                    "label_id": label_id,
                    "label_hash": row["label_hash"],
                    "family": item.family,
                    "status": status,
                    "support": item.support,
                    "fail_rate": row["fail_rate"],
                    "lift_over_weighted_category_baseline": row["lift_over_weighted_category_baseline"],
                    "live_prediction_input_allowed": not row["leaky_target"],
                    "learning_update_policy": {
                        "baseline_seeded_from": TASK_ID,
                        "online_rescore": True,
                        "promote_demote_with_new_outcomes": True,
                        "target_labels_are_supervision_only": True,
                    },
                }
            )
    return rows, accepted, {"label_status_counts": dict(sorted(status_counts.items()))}


def build_learning_bridge_manifest(
    accepted_registry_path: Path,
    usefulness_path: Path,
    chunk_labels_path: Path,
    code_state_path: Path,
    failure_sets_path: Path,
) -> dict[str, Any]:
    return {
        "artifact_kind": f"{ARTIFACT_KIND}_learning_bridge_manifest",
        "schema_version": SCHEMA_VERSION,
        "formula_version": FORMULA_VERSION,
        "purpose": "map deterministic baseline labels into existing UTML/replay/constellation learning machinery",
        "paths": {
            "accepted_label_registry": str(accepted_registry_path),
            "label_usefulness_metrics": str(usefulness_path),
            "chunk_constellation_labels": str(chunk_labels_path),
            "code_state_constellations": str(code_state_path),
            "failure_evidence_sets": str(failure_sets_path),
        },
        "consumers": {
            "delta_p.per_chunk_values": "per-chunk and per-evidence-set label prediction error",
            "delta_xi.constellation_violation_rate": "slot/pair/group label violations and anti-compensation failures",
            "ReplayBufferRow.cell_id": "language + code-state cell + accepted label signature + named failure mode",
            "CF_MEJEPA_CONSTELLATION_PATTERNS": "accepted pair/group relationship labels with lift over per-slot baseline",
            "CF_MEJEPA_ACTIVE_LEARNING_QUEUE": "unknown, low-support, OOD, and reality-contradicted labels",
            "CF_MEJEPA_ONTOLOGY_GROWTH_AUDIT": "promote/demote history for label IDs as new outcomes arrive",
        },
        "dynamic_learning_policy": {
            "baseline_only": True,
            "new_data_uses_same_compiler": True,
            "usefulness_rescored_over_time": True,
            "target_outcomes_supervise_but_are_not_live_inputs": True,
            "slot_identity_preserved": True,
            "flat_vector_concat_used": False,
        },
    }


def command_python_auto_label_discovery(args: argparse.Namespace) -> int:
    started_at = utc_now()
    active_input_preflight(args)
    required_embedders = parse_embedder_list(args.expected_embedders)
    require(len(required_embedders) == args.expected_embedder_count, "MEJEPA_AUTO_LABEL_EXPECTED_EMBEDDER_COUNT_MISMATCH")

    before = {"output_root": tree_state(args.output_root), "readback_root": tree_state(args.readback_root)}
    if args.clean_output and args.output_root.exists():
        shutil.rmtree(args.output_root)
    args.output_root.mkdir(parents=True, exist_ok=True)
    args.readback_root.mkdir(parents=True, exist_ok=True)

    shards, forward_summary = select_shards(args.forward_root, args.max_shards)
    max_chunk_window = max(int(shard.get("chunk_offset") or 0) + int(shard.get("max_chunks") or 0) for shard in shards)
    chunks, chunk_summary = load_chunks(args.chunks_jsonl, max_chunk_window if args.max_shards is not None else None)
    oracle_index, oracle_summary = load_oracle_index(args.corpus_index_json)
    docker_reports, docker_summary = load_docker_reports(args.swebench_root)
    manual_targets, manual_summary = load_manual_targets(args.manual_review if args.manual_review else None)

    chunk_labels_path = args.output_root / "chunk_constellation_labels.jsonl"
    file_constellations_path = args.output_root / "file_path_constellations.jsonl"
    code_state_constellations_path = args.output_root / "code_state_constellations.jsonl"
    failure_sets_path = args.output_root / "failure_evidence_sets.jsonl"
    usefulness_path = args.output_root / "label_usefulness_metrics.jsonl"
    registry_path = args.output_root / "accepted_label_registry.jsonl"
    slot_vector_space_health_path = args.output_root / "slot_vector_space_health.jsonl"
    manual_repro_path = args.output_root / "manual_review_reproduction.json"
    bridge_path = args.output_root / "learning_bridge_manifest.json"
    manifest_path = args.output_root / "auto_label_discovery_manifest.json"
    report_path = args.output_root / "auto_label_discovery_report.json"

    label_stats: dict[str, LabelStats] = {}
    category_totals: Counter[str] = Counter()
    category_fail_totals: Counter[str] = Counter()
    code_states: dict[str, AggregateState] = {}
    file_states: dict[str, AggregateState] = {}
    failure_evidence: dict[str, CodeStateFailureEvidence] = {}
    generated_manual_labels: dict[str, set[str]] = {}
    row_count = 0
    slot_record_count = 0
    missing_slot_events = 0
    missing_slot_rows = 0
    duplicate_slot_events = 0
    duplicate_slot_rows = 0
    source_readback_unverified_events = 0
    source_readback_unverified_rows = 0
    raw_forward_cache_legacy_path_metadata_events = 0
    raw_forward_cache_legacy_path_metadata_rows = 0
    missing_required_embedder_manifest_entry_count = 0
    missing_required_embedder_manifest_entry_samples: list[dict[str, str]] = []
    pair_label_count = 0
    group_label_count = 0
    slot_record_counts = Counter()
    slot_vector_hash_counts: dict[str, Counter[str]] = defaultdict(Counter)

    with chunk_labels_path.open("w", encoding="utf-8") as chunk_fh:
        for shard in shards:
            shard_id = str(shard.get("shard_id") or "")
            start = int(shard.get("chunk_offset") or 0)
            size = int(shard.get("max_chunks") or 0)
            shard_chunks = chunks[start : min(start + size, len(chunks))]
            row_work: dict[str, dict[str, Any]] = {}
            for chunk in shard_chunks:
                row_work[str(chunk["_row_key"])] = {
                    "chunk": chunk,
                    "slot_features": {},
                    "slot_labels": [],
                    "slot_vector_hashes": {},
                    "slot_dims": {},
                    "source_readback_unverified": [],
                    "raw_forward_cache_legacy_path_metadata": [],
                    "duplicate_slots": [],
                }

            shard_root = args.forward_root / "shards" / shard_id
            packed_manifest = read_json(shard_root / "python_forward_cache_packed_manifest.json")
            require(packed_manifest.get("passes") is True, "MEJEPA_AUTO_LABEL_PACKED_MANIFEST_NOT_PASSING", shard_id)
            per_embedder = packed_manifest.get("per_embedder") or {}

            for embedder_id in required_embedders:
                entry = per_embedder.get(embedder_id)
                if entry is None:
                    missing_required_embedder_manifest_entry_count += 1
                    if len(missing_required_embedder_manifest_entry_samples) < 32:
                        missing_required_embedder_manifest_entry_samples.append(
                            {"shard_id": shard_id, "embedder_id": embedder_id}
                        )
                    continue
                record_path = Path(str(entry.get("record_path") or ""))
                require(record_path.exists(), "MEJEPA_AUTO_LABEL_RECORD_PATH_MISSING", str(record_path))
                manifest_rows = int(entry.get("row_count") or 0)
                local_rows = 0
                with record_path.open("r", encoding="utf-8") as record_fh:
                    for line_no, line in enumerate(record_fh, start=1):
                        if not line.strip():
                            continue
                        local_rows += 1
                        record = json.loads(line)
                        key = record.get("key") or {}
                        chunk_info = record.get("chunk") or {}
                        output = record.get("output") or {}
                        row_key = row_key_from_fields(
                            str(chunk_info.get("task_instance_id") or ""),
                            str(chunk_info.get("workspace_state") or ""),
                            str(key.get("chunk_id") or ""),
                            str(key.get("source_sha256") or ""),
                        )
                        work = row_work.get(row_key)
                        if work is None:
                            continue
                        lineage = guard_packed_record_lineage(
                            record,
                            work["chunk"],
                            context=f"{record_path}:{line_no}",
                        )
                        if lineage["raw_packed_path_metadata_legacy"]:
                            work["raw_forward_cache_legacy_path_metadata"].append(embedder_id)
                        vector = np.asarray(output.get("vector"), dtype="<f4")
                        require(vector.ndim == 1 and vector.size > 0, "MEJEPA_AUTO_LABEL_VECTOR_MISSING", f"{record_path}:{line_no}")
                        features = vector_scalar_features(vector)
                        labels = slot_labels(embedder_id, vector, features)
                        if embedder_id in work["slot_features"]:
                            work["duplicate_slots"].append(embedder_id)
                        work["slot_features"][embedder_id] = features
                        work["slot_labels"].extend(labels)
                        vector_sha = str(output.get("vector_sha256") or sha256_bytes(vector.astype("<f4", copy=False).tobytes()))
                        work["slot_vector_hashes"][embedder_id] = vector_sha
                        work["slot_dims"][embedder_id] = int(vector.size)
                        if not bool(record.get("source_readback_verified", True)):
                            work["source_readback_unverified"].append(embedder_id)
                        slot_record_count += 1
                        slot_record_counts[embedder_id] += 1
                        slot_vector_hash_counts[embedder_id][vector_sha] += 1
                require(local_rows == manifest_rows, "MEJEPA_AUTO_LABEL_RECORD_ROW_COUNT_MISMATCH", f"{record_path}: manifest={manifest_rows} read={local_rows}")

            for row_key, work in sorted(row_work.items()):
                chunk = work["chunk"]
                task_id = str(chunk.get("task_instance_id") or "")
                category = normalize_category(str(chunk.get("mutation_category") or "unknown"))
                workspace_state = str(chunk.get("workspace_state") or "")
                partition = stable_partition(row_key)
                oracle_entry = oracle_index.get((task_id, category))
                docker_label, phase = docker_outcome(docker_reports.get((task_id, category)))
                failed = failure_outcome(docker_label, bool(oracle_entry.get("oracle_all_passed")) if oracle_entry is not None else None)
                category_totals[category] += 1
                category_fail_totals[category] += 1 if failed else 0
                parsed_note, mutation_labels = mutation_micro_labels(oracle_entry)
                surface = source_surface_labels(chunk)
                relation_labels = source_site_relation_labels(chunk, parsed_note, category)
                target_labels = outcome_labels(docker_label, phase, oracle_entry) + ["leakage_policy:target_side_only"]
                missing_slots = [embedder_id for embedder_id in required_embedders if embedder_id not in work["slot_features"]]
                missing_slot_events += len(missing_slots)
                missing_slot_rows += 1 if missing_slots else 0
                duplicate_slots = sorted(set(work["duplicate_slots"]))
                duplicate_slot_events += len(work["duplicate_slots"])
                duplicate_slot_rows += 1 if duplicate_slots else 0
                source_readback_unverified = sorted(set(work["source_readback_unverified"]))
                source_readback_unverified_events += len(work["source_readback_unverified"])
                source_readback_unverified_rows += 1 if source_readback_unverified else 0
                raw_forward_cache_legacy_path_metadata = sorted(set(work["raw_forward_cache_legacy_path_metadata"]))
                raw_forward_cache_legacy_path_metadata_events += len(work["raw_forward_cache_legacy_path_metadata"])
                raw_forward_cache_legacy_path_metadata_rows += 1 if raw_forward_cache_legacy_path_metadata else 0
                missing_labels = [f"slot:{embedder_id}:missing" for embedder_id in missing_slots]
                slot_label_values = sorted(set(work["slot_labels"] + missing_labels))
                available = sorted(work["slot_features"])
                pair_labels = [
                    pair_relationship(left, work["slot_features"][left], right, work["slot_features"][right])
                    for left, right in combinations(available, 2)
                ]
                pair_label_count += len(pair_labels)
                group_labels = group_signatures(work["slot_features"], pair_labels)
                group_label_count += len(group_labels)
                corpus_supervision_labels = mutation_labels
                live_candidates = surface + relation_labels + slot_label_values + pair_labels + group_labels
                all_live_labels, live_rejected_supervision = split_live_and_supervision_labels(live_candidates)
                all_target_supervision_labels = sorted(set(target_labels + corpus_supervision_labels + live_rejected_supervision))
                all_labels = sorted(set(all_live_labels + all_target_supervision_labels))

                register_labels(
                    label_stats,
                    live_candidates,
                    failed=failed,
                    category=category,
                    docker_label=docker_label,
                    partition=partition,
                    scope="chunk",
                    row_key=row_key,
                )
                register_labels(
                    label_stats,
                    all_target_supervision_labels + [f"partition:{partition}"],
                    failed=failed,
                    category=category,
                    docker_label=docker_label,
                    partition=partition,
                    scope="chunk",
                    row_key=row_key,
                    leaky_target=True,
                )

                code_key = group_key(task_id, category, workspace_state)
                file_key = file_group_key(task_id, category, workspace_state, str(chunk.get("relative_path") or "unknown"))
                code_states.setdefault(
                    code_key,
                    AggregateState(
                        group_key=code_key,
                        scope="code_state",
                        task_instance_id=task_id,
                        mutation_category=category,
                        workspace_state=workspace_state,
                        repo=str(chunk.get("repo") or "unknown"),
                        relative_path=None,
                        docker_label=docker_label,
                        oracle_all_passed=bool(oracle_entry.get("oracle_all_passed")) if oracle_entry is not None else None,
                    ),
                ).observe(row_key, chunk, surface + relation_labels, slot_label_values, pair_labels, group_labels, work["slot_features"])
                file_states.setdefault(
                    file_key,
                    AggregateState(
                        group_key=file_key,
                        scope="file_path",
                        task_instance_id=task_id,
                        mutation_category=category,
                        workspace_state=workspace_state,
                        repo=str(chunk.get("repo") or "unknown"),
                        relative_path=str(chunk.get("relative_path") or "unknown"),
                        docker_label=docker_label,
                        oracle_all_passed=bool(oracle_entry.get("oracle_all_passed")) if oracle_entry is not None else None,
                    ),
                ).observe(row_key, chunk, surface + relation_labels, slot_label_values, pair_labels, group_labels, work["slot_features"])

                candidate_score, candidate_reasons = failure_candidate_score(chunk, parsed_note, surface, slot_label_values, pair_labels)
                if candidate_score > 0.0:
                    evidence = failure_evidence.setdefault(
                        code_key,
                        CodeStateFailureEvidence(
                            code_state_key=code_key,
                            task_instance_id=task_id,
                            mutation_category=category,
                            workspace_state=workspace_state,
                            docker_label=docker_label,
                            oracle_all_passed=bool(oracle_entry.get("oracle_all_passed")) if oracle_entry is not None else None,
                            source_relative_path=parsed_note.get("source_relative_path"),
                        ),
                    )
                    evidence.candidates.append(
                        FailureCandidate(
                            row_key=row_key,
                            chunk_id=str(chunk.get("chunk_id") or ""),
                            relative_path=str(chunk.get("relative_path") or ""),
                            byte_start=int(chunk.get("byte_start") or 0),
                            byte_end=int(chunk.get("byte_end") or 0),
                            node_kind=str(chunk.get("node_kind") or ""),
                            source_text_preview=str(chunk.get("source_text") or "")[:240],
                            score=candidate_score,
                            reasons=candidate_reasons,
                            labels=sorted(set(surface + relation_labels + mutation_labels + group_labels)),
                        )
                    )

                if row_key in manual_targets:
                    generated_manual_labels[row_key] = set(all_labels)

                append_jsonl(
                    chunk_fh,
                    {
                        "artifact_kind": f"{ARTIFACT_KIND}_chunk_constellation_labels",
                        "schema_version": SCHEMA_VERSION,
                        "formula_version": FORMULA_VERSION,
                        "row_key": row_key,
                        "task_instance_id": task_id,
                        "mutation_category": category,
                        "workspace_state": workspace_state,
                        "repo": str(chunk.get("repo") or "unknown"),
                        "relative_path": str(chunk.get("relative_path") or "unknown"),
                        "chunk_id": str(chunk.get("chunk_id") or ""),
                        "source_sha256": str(chunk.get("source_sha256") or ""),
                        "source_text_sha256": sha256_text(str(chunk.get("source_text") or "")),
                        "node_kind": str(chunk.get("node_kind") or "unknown"),
                        "chunk_kind": str(chunk.get("chunk_kind") or "unknown"),
                        "byte_span": [int(chunk.get("byte_start") or 0), int(chunk.get("byte_end") or 0)],
                        "partition": partition,
                        "labels": all_labels,
                        "live_predictor_labels": all_live_labels,
                        "target_supervision_labels": all_target_supervision_labels,
                        "source_site_relation_labels": relation_labels,
                        "slot_labels": slot_label_values,
                        "missing_slots": missing_slots,
                        "duplicate_slots": duplicate_slots,
                        "source_readback_unverified": source_readback_unverified,
                        "source_readback_verified": not source_readback_unverified,
                        "canonical_ast_lineage_used": True,
                        "raw_forward_cache_path_metadata_legacy": bool(raw_forward_cache_legacy_path_metadata),
                        "raw_forward_cache_path_metadata_legacy_slots": raw_forward_cache_legacy_path_metadata,
                        "slot_evidence_complete": not missing_slots and not duplicate_slots and not source_readback_unverified,
                        "slot_dims": dict(sorted(work["slot_dims"].items())),
                        "slot_vector_sha256": dict(sorted(work["slot_vector_hashes"].items())),
                        "slot_scalar_features": work["slot_features"],
                        "pair_relationships": pair_labels,
                        "group_signatures": group_labels,
                        "docker_label": docker_label,
                        "test_phase_vector": phase,
                        "oracle_all_passed": bool(oracle_entry.get("oracle_all_passed")) if oracle_entry is not None else None,
                        "failure_candidate_score": candidate_score,
                        "failure_candidate_reasons": candidate_reasons,
                        "slot_identity_preserved": True,
                        "flat_vector_concat_used": False,
                    },
                )
                row_count += 1

    file_rows = []
    for agg in sorted(file_states.values(), key=lambda item: item.group_key):
        labels = file_path_labels(agg)
        live_labels, supervision_labels = split_live_and_supervision_labels(labels)
        failed = failure_outcome(agg.docker_label, agg.oracle_all_passed)
        partition = stable_partition(agg.group_key)
        register_labels(
            label_stats,
            live_labels,
            failed=failed,
            category=agg.mutation_category,
            docker_label=agg.docker_label,
            partition=partition,
            scope="file_path",
            row_key=agg.group_key,
        )
        register_labels(
            label_stats,
            sorted(set(supervision_labels + aggregate_outcome_supervision_labels(agg))),
            failed=failed,
            category=agg.mutation_category,
            docker_label=agg.docker_label,
            partition=partition,
            scope="file_path",
            row_key=agg.group_key,
            leaky_target=True,
        )
        file_rows.append(aggregate_row(agg, labels))

    code_rows = []
    for agg in sorted(code_states.values(), key=lambda item: item.group_key):
        labels = code_state_labels(agg)
        live_labels, supervision_labels = split_live_and_supervision_labels(labels)
        failed = failure_outcome(agg.docker_label, agg.oracle_all_passed)
        partition = stable_partition(agg.group_key)
        register_labels(
            label_stats,
            live_labels,
            failed=failed,
            category=agg.mutation_category,
            docker_label=agg.docker_label,
            partition=partition,
            scope="code_state",
            row_key=agg.group_key,
        )
        register_labels(
            label_stats,
            sorted(set(supervision_labels + aggregate_outcome_supervision_labels(agg))),
            failed=failed,
            category=agg.mutation_category,
            docker_label=agg.docker_label,
            partition=partition,
            scope="code_state",
            row_key=agg.group_key,
            leaky_target=True,
        )
        code_rows.append(aggregate_row(agg, labels))

    failure_rows = []
    for code_key, evidence in sorted(failure_evidence.items()):
        failed = failure_outcome(evidence.docker_label, evidence.oracle_all_passed)
        if not failed:
            continue
        candidates = sorted(evidence.candidates, key=lambda item: (-item.score, item.relative_path, item.byte_start))
        mode = localization_mode(candidates, failed)
        partition = stable_partition(code_key)
        mode_labels = [mode] + [f"evidence:{reason}" for candidate in candidates[:10] for reason in candidate.reasons]
        live_mode_labels, supervision_mode_labels = split_live_and_supervision_labels(mode_labels)
        outcome_supervision_labels = [
            f"docker:{evidence.docker_label}",
            "code_state_outcome:failed_or_problematic",
        ]
        if evidence.oracle_all_passed is not None:
            outcome_supervision_labels.append(
                "oracle:pass" if bool(evidence.oracle_all_passed) else "oracle:fail_or_error"
            )
        register_labels(
            label_stats,
            live_mode_labels,
            failed=failed,
            category=evidence.mutation_category,
            docker_label=evidence.docker_label,
            partition=partition,
            scope="failure_evidence_set",
            row_key=code_key,
        )
        register_labels(
            label_stats,
            sorted(set(supervision_mode_labels + outcome_supervision_labels)),
            failed=failed,
            category=evidence.mutation_category,
            docker_label=evidence.docker_label,
            partition=partition,
            scope="failure_evidence_set",
            row_key=code_key,
            leaky_target=True,
        )
        failure_rows.append(
            {
                "artifact_kind": f"{ARTIFACT_KIND}_failure_evidence_set",
                "schema_version": SCHEMA_VERSION,
                "formula_version": FORMULA_VERSION,
                "code_state_key": code_key,
                "task_instance_id": evidence.task_instance_id,
                "mutation_category": evidence.mutation_category,
                "workspace_state": evidence.workspace_state,
                "docker_label": evidence.docker_label,
                "oracle_all_passed": evidence.oracle_all_passed,
                "source_relative_path": evidence.source_relative_path,
                "localization_mode": mode,
                "labels": sorted(set(mode_labels)),
                "live_predictor_labels": live_mode_labels,
                "target_supervision_labels": sorted(set(supervision_mode_labels + outcome_supervision_labels)),
                "candidate_count": len(candidates),
                "candidate_sample": [
                    {
                        "row_key": item.row_key,
                        "chunk_id": item.chunk_id,
                        "relative_path": item.relative_path,
                        "byte_span": [item.byte_start, item.byte_end],
                        "node_kind": item.node_kind,
                        "source_text_preview": item.source_text_preview,
                        "score": item.score,
                        "reasons": item.reasons,
                        "labels": item.labels,
                        "live_predictor_labels": split_live_and_supervision_labels(item.labels)[0],
                        "target_supervision_labels": split_live_and_supervision_labels(item.labels)[1],
                    }
                    for item in candidates[:20]
                ],
                "slot_identity_preserved": True,
                "flat_vector_concat_used": False,
            }
        )

    file_row_count = write_jsonl_iter(file_constellations_path, file_rows)
    code_state_row_count = write_jsonl_iter(code_state_constellations_path, code_rows)
    failure_row_count = write_jsonl_iter(failure_sets_path, failure_rows)

    metric_rows, accepted_rows, usefulness_summary = usefulness_rows(
        label_stats,
        category_totals,
        category_fail_totals,
        args.min_support,
        args.min_lift,
    )
    metric_count = write_jsonl_iter(usefulness_path, metric_rows)
    # #711: compute slot admissibility BEFORE writing the accepted-label
    # registry, so any slot:<inadmissible_embedder>:* label is dropped at
    # write time. The accepted-label registry is the consumer surface that
    # was leaking degenerate (E2/E3/E4) and aliased (E8≡E1) slot labels;
    # filtering here makes the registry strictly admissibility-aware
    # without requiring a two-pass refactor of the per-chunk emit loop.
    # (Chunk-level `chunk_constellation_labels.jsonl` still contains the
    # raw slot labels; downstream consumers of that file are already
    # admissibility-aware via `vector_space_blockers`.)
    early_slot_health = vector_space_health_from_hash_counts(
        required_embedders,
        slot_record_counts,
        slot_vector_hash_counts,
    )
    inadmissible_embedders = {
        row["embedder_id"]
        for row in early_slot_health
        if row.get("status") in {"missing", "degenerate", "duplicate"}
    }
    inadmissible_label_prefixes = tuple(
        f"slot:{eid}:" for eid in sorted(inadmissible_embedders)
    )
    if inadmissible_label_prefixes:
        accepted_rows_filtered = [
            row
            for row in accepted_rows
            if not str(row.get("label_id", "")).startswith(inadmissible_label_prefixes)
        ]
        accepted_rows_dropped_711 = len(accepted_rows) - len(accepted_rows_filtered)
        accepted_rows = accepted_rows_filtered
    else:
        accepted_rows_dropped_711 = 0
    accepted_count = write_jsonl_iter(registry_path, accepted_rows)

    selected_full = args.max_shards is None
    manual_repro = summarize_manual_reproduction(
        manual_targets,
        generated_manual_labels,
        require_all_samples=selected_full,
    )
    dump_json(manual_repro_path, manual_repro)
    bridge = build_learning_bridge_manifest(
        registry_path,
        usefulness_path,
        chunk_labels_path,
        code_state_constellations_path,
        failure_sets_path,
    )
    dump_json(bridge_path, bridge)

    expected_chunk_count = args.expected_chunk_count if selected_full else row_count
    expected_forward_count = args.expected_forward_record_count if selected_full else slot_record_count
    required_embedder_record_counts = {embedder_id: int(slot_record_counts.get(embedder_id, 0)) for embedder_id in required_embedders}
    required_embedder_record_counts_complete = all(count > 0 for count in required_embedder_record_counts.values())
    slot_vector_space_health = vector_space_health_from_hash_counts(
        required_embedders,
        slot_record_counts,
        slot_vector_hash_counts,
    )
    slot_vector_space_blockers = vector_space_blockers(slot_vector_space_health)
    slot_vector_space_health_count = write_jsonl_iter(slot_vector_space_health_path, slot_vector_space_health)
    partial_slot_evidence_allowed = bool(getattr(args, "allow_partial_slot_evidence", False))
    slot_evidence_complete = (
        missing_required_embedder_manifest_entry_count == 0
        and (partial_slot_evidence_allowed or missing_slot_events == 0)
        and duplicate_slot_events == 0
        and source_readback_unverified_events == 0
        and required_embedder_record_counts_complete
        and not slot_vector_space_blockers
    )
    validation_failures: list[str] = []
    if missing_required_embedder_manifest_entry_count:
        validation_failures.append("MEJEPA_AUTO_LABEL_REQUIRED_EMBEDDER_MISSING_FROM_PACKED_MANIFEST")
    if missing_slot_events and not partial_slot_evidence_allowed:
        validation_failures.append("MEJEPA_AUTO_LABEL_MISSING_SLOT_EVIDENCE")
    if duplicate_slot_events:
        validation_failures.append("MEJEPA_AUTO_LABEL_DUPLICATE_SLOT_EVIDENCE")
    if source_readback_unverified_events:
        validation_failures.append("MEJEPA_AUTO_LABEL_SOURCE_READBACK_UNVERIFIED")
    if not required_embedder_record_counts_complete:
        validation_failures.append("MEJEPA_AUTO_LABEL_REQUIRED_EMBEDDER_ZERO_RECORDS")
    if slot_vector_space_blockers:
        validation_failures.append("MEJEPA_AUTO_LABEL_VECTOR_SPACE_BLOCKERS")
    summary = {
        "chunk_label_rows": row_count,
        "slot_record_count": slot_record_count,
        "slot_record_counts": dict(sorted(slot_record_counts.items())),
        "required_embedders": list(required_embedders),
        "required_embedder_record_counts": required_embedder_record_counts,
        "required_embedder_record_counts_complete": required_embedder_record_counts_complete,
        "pair_label_count": pair_label_count,
        "group_label_count": group_label_count,
        "missing_slot_events": missing_slot_events,
        "missing_slot_rows": missing_slot_rows,
        "duplicate_slot_events": duplicate_slot_events,
        "duplicate_slot_rows": duplicate_slot_rows,
        "source_readback_unverified_events": source_readback_unverified_events,
        "source_readback_unverified_rows": source_readback_unverified_rows,
        "raw_forward_cache_legacy_path_metadata_events": raw_forward_cache_legacy_path_metadata_events,
        "raw_forward_cache_legacy_path_metadata_rows": raw_forward_cache_legacy_path_metadata_rows,
        "forward_cache_lineage_policy": "packed record chunk.absolute_path/dataschema is legacy provenance only; active lineage comes from canonical AST rows",
        "missing_required_embedder_manifest_entry_count": missing_required_embedder_manifest_entry_count,
        "missing_required_embedder_manifest_entry_samples": missing_required_embedder_manifest_entry_samples,
        "partial_slot_evidence_allowed": partial_slot_evidence_allowed,
        "slot_evidence_complete": slot_evidence_complete,
        "slot_vector_space_health_rows": slot_vector_space_health_count,
        "slot_vector_space_blocker_count": len(slot_vector_space_blockers),
        "slot_vector_space_blockers": [
            {
                "embedder_id": row.get("embedder_id"),
                "status": row.get("status"),
                "blocker": row.get("blocker"),
                "duplicate_of_embedder_id": row.get("duplicate_of_embedder_id"),
                "unique_vector_sha256_count": row.get("unique_vector_sha256_count"),
            }
            for row in slot_vector_space_blockers
        ],
        "validation_failures": validation_failures,
        "file_path_constellation_rows": file_row_count,
        "code_state_constellation_rows": code_state_row_count,
        "failure_evidence_set_rows": failure_row_count,
        "label_metric_rows": metric_count,
        "accepted_label_rows": accepted_count,
        "label_family_count": len({row["family"] for row in metric_rows}),
        "selected_full_corpus": selected_full,
        **usefulness_summary,
    }
    passes = (
        row_count == expected_chunk_count
        and (not selected_full or slot_record_count == expected_forward_count)
        and slot_evidence_complete
        and file_row_count > 0
        and code_state_row_count > 0
        and failure_row_count > 0
        and metric_count > 0
        and accepted_count > 0
        and manual_repro.get("passes") is True
    )
    report = {
        "artifact_kind": f"{ARTIFACT_KIND}_report",
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "formula_version": FORMULA_VERSION,
        "created_at_utc": utc_now(),
        "passes": passes,
        "source_of_truth": {
            "forward_root": str(args.forward_root),
            "chunks_jsonl": str(args.chunks_jsonl),
            "corpus_index_json": str(args.corpus_index_json),
            "swebench_root": str(args.swebench_root),
            "output_root": str(args.output_root),
            "readback_root": str(args.readback_root),
        },
        "policy": {
            "slot_identity_preserved": True,
            "flat_vector_concat_used": False,
            "no_inner_llm": True,
            "no_human_intent_labels": True,
            "baseline_seed_not_static_ontology": True,
            "new_data_uses_same_label_compiler": True,
            "target_labels_are_supervision_only": True,
            "corpus_only_mutation_labels_are_supervision_only": True,
            "live_predictor_labels_are_explicit": True,
            "partial_slot_evidence_allowed": partial_slot_evidence_allowed,
            "partial_slot_rows_emit_missing_slot_labels": True,
        },
        "summary": summary,
        "forward_summary": forward_summary,
        "chunk_summary": chunk_summary,
        "oracle_summary": oracle_summary,
        "docker_summary": docker_summary,
        "manual_review_summary": manual_summary,
        "manual_reproduction": manual_repro,
    }
    dump_json(report_path, report)
    manifest = {
        "artifact_kind": f"{ARTIFACT_KIND}_manifest",
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "formula_version": FORMULA_VERSION,
        "created_at_utc": utc_now(),
        "paths": {
            "chunk_constellation_labels": str(chunk_labels_path),
            "file_path_constellations": str(file_constellations_path),
            "code_state_constellations": str(code_state_constellations_path),
            "failure_evidence_sets": str(failure_sets_path),
            "label_usefulness_metrics": str(usefulness_path),
            "accepted_label_registry": str(registry_path),
            "slot_vector_space_health": str(slot_vector_space_health_path),
            "manual_review_reproduction": str(manual_repro_path),
            "learning_bridge_manifest": str(bridge_path),
            "report": str(report_path),
        },
        "line_counts": {
            "chunk_constellation_labels": sum(1 for _line, _row in iter_jsonl(chunk_labels_path)),
            "file_path_constellations": sum(1 for _line, _row in iter_jsonl(file_constellations_path)),
            "code_state_constellations": sum(1 for _line, _row in iter_jsonl(code_state_constellations_path)),
            "failure_evidence_sets": sum(1 for _line, _row in iter_jsonl(failure_sets_path)),
            "label_usefulness_metrics": sum(1 for _line, _row in iter_jsonl(usefulness_path)),
            "accepted_label_registry": sum(1 for _line, _row in iter_jsonl(registry_path)),
            "slot_vector_space_health": sum(1 for _line, _row in iter_jsonl(slot_vector_space_health_path)),
        },
        "stable_hashes": {
            "chunk_constellation_labels": sha256_file(chunk_labels_path),
            "file_path_constellations": sha256_file(file_constellations_path),
            "code_state_constellations": sha256_file(code_state_constellations_path),
            "failure_evidence_sets": sha256_file(failure_sets_path),
            "label_usefulness_metrics": sha256_file(usefulness_path),
            "accepted_label_registry": sha256_file(registry_path),
            "slot_vector_space_health": sha256_file(slot_vector_space_health_path),
            "manual_review_reproduction": sha256_file(manual_repro_path),
            "learning_bridge_manifest": sha256_file(bridge_path),
            "report": sha256_file(report_path),
        },
        "passes": passes,
    }
    dump_json(manifest_path, manifest)
    readback = {
        "artifact_kind": f"{ARTIFACT_KIND}_readback",
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "formula_version": FORMULA_VERSION,
        "started_at_utc": started_at,
        "completed_at_utc": utc_now(),
        "passes": passes and read_json(manifest_path).get("passes") is True,
        "before": before,
        "after": {"output_root": tree_state(args.output_root), "readback_root": tree_state(args.readback_root)},
        "report_path": str(report_path),
        "manifest_path": str(manifest_path),
        "report_sha256": sha256_file(report_path),
        "manifest_sha256": sha256_file(manifest_path),
        "manifest": manifest,
        "summary": summary,
        "readback": {
            "manifest_readback_equal": read_json(manifest_path) == manifest,
            "report_readback_equal": read_json(report_path) == report,
            "line_counts": manifest["line_counts"],
            "stable_hashes": manifest["stable_hashes"],
        },
    }
    readback_path = args.readback_root / f"{ARTIFACT_KIND}_readback.json"
    dump_json(readback_path, readback)
    final = read_json(readback_path)
    require(final.get("passes") is True, "MEJEPA_AUTO_LABEL_READBACK_FAILED", str(readback_path))
    print(json.dumps(final, indent=2, sort_keys=True))
    return 0


def add_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--forward-root", type=Path, default=DEFAULT_FORWARD_ROOT)
    parser.add_argument("--chunks-jsonl", type=Path, default=DEFAULT_CHUNKS_JSONL)
    parser.add_argument("--corpus-index-json", type=Path, default=DEFAULT_CORPUS_INDEX_JSON)
    parser.add_argument("--swebench-root", type=Path, default=DEFAULT_SWEBENCH_ROOT)
    parser.add_argument("--manual-review", type=Path, default=DEFAULT_MANUAL_REVIEW)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--readback-root", type=Path, default=DEFAULT_READBACK_ROOT)
    parser.add_argument("--expected-chunk-count", type=int, default=DEFAULT_EXPECTED_CHUNK_COUNT)
    parser.add_argument("--expected-forward-record-count", type=int, default=DEFAULT_EXPECTED_FORWARD_RECORD_COUNT)
    parser.add_argument("--expected-embedder-count", type=int, default=DEFAULT_EXPECTED_EMBEDDER_COUNT)
    parser.add_argument("--expected-embedders", default=",".join(EXPECTED_EMBEDDERS))
    parser.add_argument("--min-support", type=int, default=DEFAULT_MIN_SUPPORT)
    parser.add_argument("--min-lift", type=float, default=DEFAULT_MIN_LIFT)
    parser.add_argument("--max-shards", type=int, default=None, help="Optional smoke limiter. Omit for full corpus.")
    parser.add_argument(
        "--allow-partial-slot-evidence",
        action="store_true",
        help="Allow rows with explicit slot:<embedder>:missing labels to pass readback when vector-space health is otherwise clean.",
    )
    parser.add_argument("--clean-output", action="store_true")
