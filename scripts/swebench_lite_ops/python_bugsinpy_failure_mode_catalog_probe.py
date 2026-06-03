"""BLINDSPOT-006 BugsInPy failure-mode catalog coverage probe.

This script is intentionally measurement-only. It samples real Python bug-fix
patches from BugsInPy, emits the generic Level-0 labels that the existing
auto-label compiler can assign to patch hunks, and separately checks whether the
accepted registry contains class-specific labels for #728's real-bug classes.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
from collections import Counter, defaultdict
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable

try:
    from .python_auto_label_discovery import source_surface_labels
except ImportError:  # pragma: no cover - direct script execution fallback
    from python_auto_label_discovery import source_surface_labels  # type: ignore


ARTIFACT_KIND = "bugsinpy_failure_mode_catalog_probe"
SCHEMA_VERSION = 1
FORMULA_VERSION = "deterministic_bugsinpy_catalog_coverage_v1"
TASK_ID = "BLINDSPOT-006"

DEFAULT_REGISTRY = Path(
    "/zfs/archive/contextgraph/state/labels/task-py-g-117-auto-label-discovery-v1/"
    "accepted_label_registry.jsonl"
)
DEFAULT_OUTPUT_ROOT = Path(
    "/zfs/archive/contextgraph/fsv/blindspot-006-failure-mode-catalog-fsv/latest"
)

BUG_CLASS_RULES: dict[str, tuple[str, ...]] = {
    "async_await_misuse": (
        r"\basync\b",
        r"\bawait\b",
        r"\basyncio\b",
        r"\bevent_loop\b",
        r"\bcoroutine\b",
        r"\bWebSocket\b",
        r"\bwebsocket\b",
    ),
    "concurrency_race_deadlock": (
        r"\bthread",
        r"\blocked\b",
        r"\block\b",
        r"\bdeadlock\b",
        r"\brace\b",
        r"\bconcurrent",
        r"\bmultiprocessing\b",
        r"\bQueue\b",
        r"\btimeout\b",
    ),
    "memory_aliasing_mutation": (
        r"\bcopy\b",
        r"\bdeepcopy\b",
        r"\bmutable\b",
        r"\binplace\b",
        r"\bappend\(",
        r"\bextend\(",
        r"\bpop\(",
        r"\bclear\(",
        r"\bsetdefault\(",
        r"\bid\(",
        r"\bview\b",
        r"\bviews\b",
    ),
    "floating_point_precision": (
        r"\bfloat\b",
        r"\bnumpy\b",
        r"\bnp\.",
        r"\bisclose\b",
        r"\ballclose\b",
        r"\bDecimal\b",
        r"\bround\(",
        r"\bnan\b",
        r"\binf\b",
        r"\bdtype\b",
        r"\bprecision\b",
        r"\beps\b",
    ),
    "api_deprecation_version_drift": (
        r"\bdeprecat",
        r"\bversion\b",
        r"\bcompat\b",
        r"\bapi\b",
        r"\bimportlib\b",
        r"\brequires\b",
        r"\bpackaging\.version\b",
        r"\b__version__\b",
        r"\bsys\.version_info\b",
        r"\bcollections\.abc\b",
    ),
    "resource_leak": (
        r"\bopen\(",
        r"\bclose\(",
        r"\bwith\b",
        r"\bfinally\b",
        r"\bsocket\b",
        r"\bConnection\b",
        r"\bSession\b",
        r"\bfile\b",
        r"\bfd\b",
        r"\btempfile\b",
        r"\bcleanup\b",
        r"\bdispose\b",
        r"\brelease\b",
        r"\bcursor\b",
    ),
    "generator_iterator_exhaustion": (
        r"\biter\(",
        r"\bnext\(",
        r"\byield\b",
        r"\bgenerator\b",
        r"\bIterator\b",
        r"\bIterable\b",
        r"\bStopIteration\b",
        r"\bitertools\b",
    ),
    "test_only_pass_mocked_dependency": (
        r"\bmock\b",
        r"\bMagicMock\b",
        r"\bmonkeypatch\b",
        r"\bpatch\(",
        r"\bpytest\b",
        r"\bunittest\b",
    ),
}

REGISTRY_CLASS_TOKENS: dict[str, tuple[str, ...]] = {
    "async_await_misuse": ("async", "await", "coroutine", "event_loop", "blocking_call_in_coroutine"),
    "concurrency_race_deadlock": ("concurrency", "race", "deadlock", "thread", "lock"),
    "memory_aliasing_mutation": ("alias", "mutable", "inplace", "copy", "view"),
    "floating_point_precision": ("float", "floating", "precision", "isclose", "allclose", "dtype", "decimal"),
    "api_deprecation_version_drift": ("deprecation", "deprecated", "version", "compat", "api_drift"),
    "resource_leak": ("resource", "socket", "file_handle", "close", "cleanup", "leak"),
    "generator_iterator_exhaustion": ("generator", "iterator", "iterable", "stopiteration", "exhaust"),
    "test_only_pass_mocked_dependency": ("mock", "monkeypatch", "weak_test", "test_only"),
    "other_unclassified_real_bug": (),
}

GENERIC_LABEL_FAMILIES = {
    "ast_surface",
    "path_surface",
    "dominant_surface",
    "group_file_count",
    "group_chunk_count",
    "source_site_relation",
}


@dataclass(frozen=True)
class PatchFile:
    old_path: str
    new_path: str
    added_lines: tuple[str, ...]
    deleted_lines: tuple[str, ...]


@dataclass(frozen=True)
class BugRecord:
    project: str
    bug_id: str
    bug_dir: Path
    bug_info: dict[str, str]
    patch_text: str
    patch_sha256: str

    @property
    def record_id(self) -> str:
        return f"{self.project}:{self.bug_id}"


def sha256_text(text: str) -> str:
    return "sha256:" + hashlib.sha256(text.encode("utf-8")).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(1024 * 1024), b""):
            digest.update(chunk)
    return "sha256:" + digest.hexdigest()


def canonical_json(value: Any) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, sort_keys=True, indent=2) + "\n", encoding="utf-8")


def write_jsonl(path: Path, rows: Iterable[dict[str, Any]]) -> int:
    path.parent.mkdir(parents=True, exist_ok=True)
    count = 0
    with path.open("w", encoding="utf-8") as fh:
        for row in rows:
            fh.write(canonical_json(row))
            fh.write("\n")
            count += 1
    return count


def parse_info(text: str) -> dict[str, str]:
    out: dict[str, str] = {}
    for line in text.splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith("#") or "=" not in stripped:
            continue
        key, value = stripped.split("=", 1)
        out[key.strip()] = value.strip().strip('"').strip("'")
    return out


def iter_bugs(bugsinpy_root: Path) -> list[BugRecord]:
    projects_root = bugsinpy_root / "projects"
    if not projects_root.is_dir():
        raise FileNotFoundError(f"BugsInPy projects root missing: {projects_root}")
    records: list[BugRecord] = []
    for bug_info_path in sorted(projects_root.glob("*/bugs/*/bug.info")):
        patch_path = bug_info_path.with_name("bug_patch.txt")
        if not patch_path.is_file():
            continue
        bug_dir = bug_info_path.parent
        project = bug_dir.parent.parent.name
        bug_id = bug_dir.name
        patch_text = patch_path.read_text(encoding="utf-8", errors="replace")
        records.append(
            BugRecord(
                project=project,
                bug_id=bug_id,
                bug_dir=bug_dir,
                bug_info=parse_info(bug_info_path.read_text(encoding="utf-8", errors="replace")),
                patch_text=patch_text,
                patch_sha256=sha256_text(patch_text),
            )
        )
    if not records:
        raise ValueError(f"No BugsInPy bug records found under {bugsinpy_root}")
    return records


def sample_records(records: list[BugRecord], sample_size: int | None) -> list[BugRecord]:
    if sample_size is None or sample_size <= 0 or sample_size >= len(records):
        return sorted(records, key=lambda row: row.record_id)
    ranked = sorted(records, key=lambda row: hashlib.sha256(row.record_id.encode("utf-8")).hexdigest())
    return sorted(ranked[:sample_size], key=lambda row: row.record_id)


def git_head(root: Path) -> str | None:
    try:
        out = subprocess.check_output(
            ["git", "-C", str(root), "rev-parse", "HEAD"],
            text=True,
            stderr=subprocess.DEVNULL,
        ).strip()
    except (OSError, subprocess.CalledProcessError):
        return None
    return out or None


def parse_patch_files(patch_text: str) -> list[PatchFile]:
    files: list[PatchFile] = []
    old_path = ""
    new_path = ""
    added: list[str] = []
    deleted: list[str] = []

    def flush() -> None:
        nonlocal added, deleted, old_path, new_path
        if old_path or new_path or added or deleted:
            files.append(PatchFile(old_path, new_path, tuple(added), tuple(deleted)))
        old_path = ""
        new_path = ""
        added = []
        deleted = []

    for line in patch_text.splitlines():
        if line.startswith("diff --git "):
            flush()
            parts = line.split()
            if len(parts) >= 4:
                old_path = parts[2].removeprefix("a/")
                new_path = parts[3].removeprefix("b/")
            continue
        if line.startswith("--- "):
            old_path = line[4:].strip().removeprefix("a/")
            continue
        if line.startswith("+++ "):
            new_path = line[4:].strip().removeprefix("b/")
            continue
        if line.startswith("+") and not line.startswith("+++"):
            added.append(line[1:])
        elif line.startswith("-") and not line.startswith("---"):
            deleted.append(line[1:])
    flush()
    return files


def infer_node_kind(source_text: str) -> str:
    stripped = source_text.strip()
    if stripped.startswith("async def "):
        return "AsyncFunctionDef"
    if stripped.startswith("def "):
        return "FunctionDef"
    if stripped.startswith("class "):
        return "ClassDef"
    if stripped.startswith(("import ", "from ")):
        return "Import"
    if stripped.startswith("assert "):
        return "Assert"
    if stripped.startswith("return ") or stripped == "return":
        return "Return"
    if stripped.startswith(("if ", "elif ")):
        return "If"
    if stripped.startswith("for "):
        return "For"
    if stripped.startswith("while "):
        return "While"
    if stripped.startswith(("try:", "except ", "finally:")):
        return "Try"
    if stripped.startswith("with "):
        return "With"
    if stripped.startswith("raise "):
        return "Raise"
    if "=" in stripped:
        return "Assign"
    if "(" in stripped and ")" in stripped:
        return "Call"
    return "Expr"


def labels_from_patch(files: Iterable[PatchFile]) -> list[str]:
    labels: set[str] = set()
    for patch_file in files:
        rel = patch_file.new_path if patch_file.new_path != "/dev/null" else patch_file.old_path
        for line in tuple(patch_file.added_lines) + tuple(patch_file.deleted_lines):
            if not line.strip():
                continue
            row = {
                "source_text": line,
                "relative_path": rel,
                "node_kind": infer_node_kind(line),
                "chunk_kind": "patch_hunk_line",
            }
            labels.update(source_surface_labels(row))
    return sorted(labels)


def text_for_classification(record: BugRecord, files: Iterable[PatchFile]) -> str:
    affected_paths = " ".join(
        path
        for patch_file in files
        for path in (patch_file.old_path, patch_file.new_path)
        if path and path != "/dev/null"
    )
    info_text = " ".join(f"{k}={v}" for k, v in sorted(record.bug_info.items()))
    return "\n".join([record.project, record.bug_id, affected_paths, info_text, record.patch_text])


def classify_bug(record: BugRecord, files: Iterable[PatchFile]) -> tuple[list[str], dict[str, list[str]]]:
    haystack = text_for_classification(record, files)
    matches: dict[str, list[str]] = {}
    for bug_class, patterns in BUG_CLASS_RULES.items():
        hit_patterns = sorted({pattern for pattern in patterns if re.search(pattern, haystack, re.IGNORECASE)})
        if hit_patterns:
            matches[bug_class] = hit_patterns
    if not matches:
        matches["other_unclassified_real_bug"] = []
    return sorted(matches), matches


def read_registry(path: Path) -> tuple[set[str], list[dict[str, Any]]]:
    labels: set[str] = set()
    rows: list[dict[str, Any]] = []
    with path.open(encoding="utf-8") as fh:
        for line in fh:
            if not line.strip():
                continue
            row = json.loads(line)
            label_id = str(row.get("label_id") or "")
            if label_id:
                labels.add(label_id)
                rows.append(row)
    if not labels:
        raise ValueError(f"No accepted labels read from {path}")
    return labels, rows


def registry_hits_by_bug_class(accepted_labels: Iterable[str]) -> dict[str, list[str]]:
    labels = sorted(accepted_labels)
    out: dict[str, list[str]] = {}
    for bug_class, tokens in REGISTRY_CLASS_TOKENS.items():
        hits = [
            label
            for label in labels
            if any(label_has_registry_token(label, token) for token in tokens)
        ]
        out[bug_class] = hits
    return out


def label_has_registry_token(label_id: str, token: str) -> bool:
    label_parts = [part for part in re.split(r"[^a-z0-9]+", label_id.lower()) if part]
    token_parts = [part for part in re.split(r"[^a-z0-9]+", token.lower()) if part]
    if not token_parts:
        return False
    if len(token_parts) == 1:
        return token_parts[0] in label_parts
    width = len(token_parts)
    return any(label_parts[idx : idx + width] == token_parts for idx in range(0, len(label_parts) - width + 1))


def label_family(label_id: str) -> str:
    return label_id.split(":", 1)[0] if ":" in label_id else label_id


def class_hits_from_emitted_labels(labels: Iterable[str]) -> dict[str, list[str]]:
    label_list = list(labels)
    out: dict[str, list[str]] = {}
    for bug_class, tokens in REGISTRY_CLASS_TOKENS.items():
        hits = [
            label
            for label in label_list
            if any(label_has_registry_token(label, token) for token in tokens)
        ]
        if hits:
            out[bug_class] = sorted(set(hits))
    return out


def decision_from_fraction(unlabeled_fraction: float) -> dict[str, str]:
    if unlabeled_fraction < 0.10:
        return {
            "classification": "catalog_gaps_minor_defer",
            "next_action": "defer_label_expansion",
        }
    if unlabeled_fraction <= 0.30:
        return {
            "classification": "targeted_label_expansion_required",
            "next_action": "file_targeted_label_expansion_subissues",
        }
    return {
        "classification": "structural_blind_spot_major_corpus_expansion_required",
        "next_action": "major_real_python_bug_corpus_and_label_expansion_required",
    }


def run_probe(args: argparse.Namespace) -> dict[str, Any]:
    accepted_labels, registry_rows = read_registry(args.accepted_registry)
    registry_class_hits = registry_hits_by_bug_class(accepted_labels)
    all_records = iter_bugs(args.bugsinpy_root)
    sampled_records = sample_records(all_records, args.sample_size)

    bug_rows: list[dict[str, Any]] = []
    class_counts: Counter[str] = Counter()
    class_uncovered_counts: Counter[str] = Counter()
    class_generic_counts: Counter[str] = Counter()
    class_specific_match_counts: Counter[str] = Counter()
    emitted_family_counts: dict[str, Counter[str]] = defaultdict(Counter)
    project_counts: Counter[str] = Counter()
    generic_unlabeled = 0
    class_specific_unlabeled = 0
    misclassified_proxy = 0
    mounted_drive_hits = 0

    for record in sampled_records:
        files = parse_patch_files(record.patch_text)
        emitted_labels = labels_from_patch(files)
        emitted_accepted = sorted(label for label in emitted_labels if label in accepted_labels)
        generic_matches = [
            label for label in emitted_accepted if label_family(label) in GENERIC_LABEL_FAMILIES
        ]
        bug_classes, class_reasons = classify_bug(record, files)
        class_specific_hits = {
            bug_class: registry_class_hits.get(bug_class, [])
            for bug_class in bug_classes
            if registry_class_hits.get(bug_class)
        }
        emitted_class_hits = class_hits_from_emitted_labels(emitted_accepted)
        wrong_emitted_classes = sorted(set(emitted_class_hits) - set(bug_classes))
        class_specific_covered = bool(class_specific_hits)
        generic_covered = bool(emitted_accepted)
        if not generic_covered:
            generic_unlabeled += 1
        if not class_specific_covered:
            class_specific_unlabeled += 1
        if wrong_emitted_classes and not class_specific_covered:
            misclassified_proxy += 1
        project_counts[record.project] += 1
        for bug_class in bug_classes:
            class_counts[bug_class] += 1
            if generic_covered:
                class_generic_counts[bug_class] += 1
            if class_specific_covered and bug_class in class_specific_hits:
                class_specific_match_counts[bug_class] += 1
            else:
                class_uncovered_counts[bug_class] += 1
            for label in emitted_accepted:
                emitted_family_counts[bug_class][label_family(label)] += 1

        affected_paths = sorted(
            {
                path
                for patch_file in files
                for path in (patch_file.old_path, patch_file.new_path)
                if path and path != "/dev/null"
            }
        )
        if "/mnt/c" in canonical_json(record.bug_info) or "/mnt/d" in canonical_json(record.bug_info):
            mounted_drive_hits += 1
        bug_rows.append(
            {
                "artifact_kind": f"{ARTIFACT_KIND}_bug_row",
                "schema_version": SCHEMA_VERSION,
                "formula_version": FORMULA_VERSION,
                "record_id": record.record_id,
                "project": record.project,
                "bug_id": record.bug_id,
                "bug_info": record.bug_info,
                "patch_sha256": record.patch_sha256,
                "patch_file_count": len(files),
                "affected_paths": affected_paths,
                "added_line_count": sum(len(item.added_lines) for item in files),
                "deleted_line_count": sum(len(item.deleted_lines) for item in files),
                "inferred_bug_classes": bug_classes,
                "classification_reasons": class_reasons,
                "emitted_level0_labels": emitted_labels,
                "accepted_level0_label_matches": emitted_accepted,
                "accepted_level0_label_match_count": len(emitted_accepted),
                "generic_level0_covered": generic_covered,
                "generic_level0_matches": generic_matches,
                "class_specific_registry_hits": class_specific_hits,
                "class_specific_covered": class_specific_covered,
                "wrong_class_specific_emitted_labels": wrong_emitted_classes,
                "slot_identity_preserved": True,
                "flat_vector_concat_used": False,
                "target_labels_used_as_live_inputs": False,
            }
        )

    coverage_rows = []
    for bug_class in sorted(class_counts):
        count = class_counts[bug_class]
        coverage_rows.append(
            {
                "artifact_kind": f"{ARTIFACT_KIND}_coverage_matrix_row",
                "schema_version": SCHEMA_VERSION,
                "formula_version": FORMULA_VERSION,
                "bug_class": bug_class,
                "sample_count": count,
                "generic_level0_covered_count": class_generic_counts[bug_class],
                "class_specific_covered_count": class_specific_match_counts[bug_class],
                "class_specific_uncovered_count": class_uncovered_counts[bug_class],
                "class_specific_uncovered_fraction": class_uncovered_counts[bug_class] / count,
                "accepted_registry_class_hit_count": len(registry_class_hits.get(bug_class, [])),
                "accepted_registry_class_hit_sample": registry_class_hits.get(bug_class, [])[:20],
                "emitted_label_family_counts": dict(sorted(emitted_family_counts[bug_class].items())),
            }
        )

    sample_count = len(sampled_records)
    generic_unlabeled_fraction = generic_unlabeled / sample_count
    class_specific_unlabeled_fraction = class_specific_unlabeled / sample_count
    misclassified_proxy_fraction = misclassified_proxy / sample_count
    decision = decision_from_fraction(class_specific_unlabeled_fraction)

    output_root = args.output_root
    output_root.mkdir(parents=True, exist_ok=True)
    bug_rows_path = output_root / "bug_rows.jsonl"
    coverage_path = output_root / "coverage_matrix.jsonl"
    bug_row_count = write_jsonl(bug_rows_path, bug_rows)
    coverage_row_count = write_jsonl(coverage_path, coverage_rows)

    summary = {
        "artifact_kind": f"{ARTIFACT_KIND}_summary",
        "schema_version": SCHEMA_VERSION,
        "formula_version": FORMULA_VERSION,
        "task_id": TASK_ID,
        "passes": True,
        "decision": decision,
        "counts": {
            "bugsinpy_total_records": len(all_records),
            "sample_count": sample_count,
            "bug_row_count": bug_row_count,
            "coverage_row_count": coverage_row_count,
            "accepted_registry_rows": len(registry_rows),
            "generic_level0_unlabeled_count": generic_unlabeled,
            "class_specific_unlabeled_count": class_specific_unlabeled,
            "misclassified_proxy_count": misclassified_proxy,
            "mounted_drive_hits": mounted_drive_hits,
        },
        "fractions": {
            "generic_level0_unlabeled_fraction": generic_unlabeled_fraction,
            "class_specific_unlabeled_fraction": class_specific_unlabeled_fraction,
            "misclassified_proxy_fraction": misclassified_proxy_fraction,
        },
        "class_counts": dict(sorted(class_counts.items())),
        "project_counts": dict(sorted(project_counts.items())),
        "policy": {
            "measurement_only": True,
            "slot_identity_preserved": True,
            "flat_vector_concat_used": False,
            "target_labels_used_as_live_inputs": False,
            "inner_llm_used": False,
            "predictor_weights_changed": False,
            "decision_fraction": "class_specific_unlabeled_fraction",
        },
        "notes": {
            "generic_level0_unlabeled_fraction": "Existing compiler surface labels can be assigned to patch hunks.",
            "class_specific_unlabeled_fraction": "Decision-grade #728 metric: inferred real bug class lacks any class-specific accepted registry label.",
            "misclassified_proxy_fraction": "Proxy only; BugsInPy metadata does not include authoritative bug-class ground truth.",
        },
    }
    summary_path = output_root / "summary.json"
    write_json(summary_path, summary)

    manifest = {
        "artifact_kind": f"{ARTIFACT_KIND}_manifest",
        "schema_version": SCHEMA_VERSION,
        "formula_version": FORMULA_VERSION,
        "task_id": TASK_ID,
        "passes": True,
        "source": {
            "bugsinpy_root": str(args.bugsinpy_root),
            "bugsinpy_git_head": git_head(args.bugsinpy_root),
            "accepted_registry": str(args.accepted_registry),
            "accepted_registry_sha256": sha256_file(args.accepted_registry),
        },
        "paths": {
            "bug_rows": str(bug_rows_path),
            "coverage_matrix": str(coverage_path),
            "summary": str(summary_path),
        },
        "line_counts": {
            "bug_rows": bug_row_count,
            "coverage_matrix": coverage_row_count,
        },
        "stable_hashes": {
            "bug_rows": sha256_file(bug_rows_path),
            "coverage_matrix": sha256_file(coverage_path),
            "summary": sha256_file(summary_path),
        },
    }
    manifest_path = output_root / "manifest.json"
    write_json(manifest_path, manifest)
    manifest["paths"]["manifest"] = str(manifest_path)
    manifest["stable_hashes"]["manifest"] = sha256_file(manifest_path)
    write_json(output_root / "artifact_hashes.json", manifest["stable_hashes"])
    return manifest


def add_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--bugsinpy-root", type=Path, required=True)
    parser.add_argument("--accepted-registry", type=Path, default=DEFAULT_REGISTRY)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument(
        "--sample-size",
        type=int,
        default=50,
        help="Deterministic sample size. Use 0 or a value >= corpus size for all records.",
    )


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    add_arguments(parser)
    args = parser.parse_args(argv)
    manifest = run_probe(args)
    print(json.dumps(manifest, sort_keys=True, indent=2))
    return 0


if __name__ == "__main__":  # pragma: no cover
    raise SystemExit(main())
