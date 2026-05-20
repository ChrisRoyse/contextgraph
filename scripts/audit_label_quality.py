#!/usr/bin/env python3
"""Audit label quality rows from CF_MEJEPA_FAILURE_EXEMPLARS.

The source of truth is a RocksDB column family. Python RocksDB bindings are
not assumed to exist on operator machines, so this reader shells out to the
local `ldb` tool and decodes the JSON values stored by the Rust flywheel path.
"""

from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


CF_NAME = "CF_MEJEPA_FAILURE_EXEMPLARS"
DEFAULT_EXPORT_ROOT = Path("/zfs/archive/contextgraph/exports/eval")
LANGUAGES = (
    "python",
    "rust",
    "javascript",
    "typescript",
    "go",
    "java",
    "c",
    "cpp",
    "c_sharp",
    "ruby",
    "php",
)
LANGUAGE_ALIASES = {
    "py": "python",
    "js": "javascript",
    "ts": "typescript",
    "c++": "cpp",
    "cc": "cpp",
    "cxx": "cpp",
    "c#": "c_sharp",
    "cs": "c_sharp",
}


class AuditError(RuntimeError):
    pass


@dataclass
class LanguageStats:
    language: str
    cf_bucket_count: int = 0
    row_count: int = 0
    label_count_total: int = 0
    labels_per_row_min: int | None = None
    labels_per_row_max: int | None = None
    non_trivial_signal_count: int = 0
    unavailable_signal_count: int = 0
    examples: list[str] = field(default_factory=list)

    def add_row(self, task_id: str, label_count: int, non_trivial: bool) -> None:
        self.row_count += 1
        self.label_count_total += label_count
        self.labels_per_row_min = (
            label_count
            if self.labels_per_row_min is None
            else min(self.labels_per_row_min, label_count)
        )
        self.labels_per_row_max = (
            label_count
            if self.labels_per_row_max is None
            else max(self.labels_per_row_max, label_count)
        )
        if non_trivial:
            self.non_trivial_signal_count += 1
        if label_count == 0:
            self.unavailable_signal_count += 1
        if len(self.examples) < 3:
            self.examples.append(task_id)

    def as_dict(self) -> dict[str, Any]:
        mean = 0.0
        non_trivial_ratio = 0.0
        unavailable_ratio = 0.0
        if self.row_count:
            mean = self.label_count_total / self.row_count
            non_trivial_ratio = self.non_trivial_signal_count / self.row_count
            unavailable_ratio = self.unavailable_signal_count / self.row_count
        return {
            "language": self.language,
            "cfBucketCount": self.cf_bucket_count,
            "rows": self.row_count,
            "labelsPerRowMean": round(mean, 6),
            "labelsPerRowMin": self.labels_per_row_min or 0,
            "labelsPerRowMax": self.labels_per_row_max or 0,
            "nonTrivialSignalRows": self.non_trivial_signal_count,
            "nonTrivialSignalRatio": round(non_trivial_ratio, 6),
            "unavailableSignalRows": self.unavailable_signal_count,
            "unavailableSignalRatio": round(unavailable_ratio, 6),
            "examples": tuple(self.examples),
        }


def main(argv: list[str]) -> int:
    args = parse_args(argv)
    payload = build_audit(args.db, args.ldb)
    output_path = args.output or default_output_path(args.date)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    markdown = render_markdown(payload)
    output_path.write_text(markdown, encoding="utf-8")

    if args.json_output:
        args.json_output.parent.mkdir(parents=True, exist_ok=True)
        args.json_output.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")

    print(json.dumps({"output": str(output_path), "sourceOfTruth": payload["sourceOfTruth"], "allPassed": True}, indent=2, sort_keys=True))
    return 0


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Audit ME-JEPA failure-exemplar label quality")
    parser.add_argument("--db", required=True, type=Path, help="RocksDB path")
    parser.add_argument("--date", default=datetime.now(timezone.utc).date().isoformat())
    parser.add_argument("--output", type=Path, default=None)
    parser.add_argument("--json-output", type=Path, default=None)
    parser.add_argument("--ldb", default=shutil.which("ldb") or "ldb")
    return parser.parse_args(argv)


def default_output_path(date: str) -> Path:
    return DEFAULT_EXPORT_ROOT / date / "label_quality.md"


def build_audit(db_path: Path, ldb: str) -> dict[str, Any]:
    if not db_path.exists():
        raise AuditError(f"RocksDB path does not exist: {db_path}")
    buckets = read_failure_exemplar_buckets(db_path, ldb)
    stats_by_language = {language: LanguageStats(language) for language in LANGUAGES}
    stats_by_language["unknown"] = LanguageStats("unknown")
    malformed_rows: list[str] = []

    for bucket_index, bucket in enumerate(buckets):
        bucket_languages = set()
        for row_index, row in enumerate(bucket):
            try:
                task_id = require_string(row, "task_id")
                language = infer_language(row, task_id)
                oracle = require_dict(row, "oracle_verdict")
                label_count, non_trivial = label_signal(oracle)
            except AuditError as exc:
                malformed_rows.append(f"bucket {bucket_index} row {row_index}: {exc}")
                continue
            stats = stats_by_language.setdefault(language, LanguageStats(language))
            stats.add_row(task_id, label_count, non_trivial)
            bucket_languages.add(language)
        for language in bucket_languages:
            stats_by_language[language].cf_bucket_count += 1

    rows = [
        stats_by_language[language].as_dict()
        for language in sorted(
            stats_by_language,
            key=lambda item: (item == "unknown", LANGUAGES.index(item) if item in LANGUAGES else 999, item),
        )
        if stats_by_language[language].row_count > 0 or language == "unknown"
    ]
    total_rows = sum(row["rows"] for row in rows)
    total_unavailable = sum(row["unavailableSignalRows"] for row in rows)
    total_non_trivial = sum(row["nonTrivialSignalRows"] for row in rows)
    return {
        "schemaVersion": 1,
        "sourceOfTruth": {
            "dbPath": str(db_path),
            "columnFamily": CF_NAME,
            "reader": "ldb scan --hex",
        },
        "cfBucketCount": len(buckets),
        "totalRows": total_rows,
        "totalUnavailableSignalRows": total_unavailable,
        "totalNonTrivialSignalRows": total_non_trivial,
        "malformedRows": malformed_rows,
        "languages": rows,
    }


def read_failure_exemplar_buckets(db_path: Path, ldb: str) -> list[list[dict[str, Any]]]:
    cmd = [
        ldb,
        f"--db={db_path}",
        f"--column_family={CF_NAME}",
        "scan",
        "--hex",
    ]
    proc = subprocess.run(cmd, check=False, text=True, capture_output=True)
    if proc.returncode != 0:
        raise AuditError(proc.stderr.strip() or proc.stdout.strip() or f"ldb exited {proc.returncode}")
    buckets: list[list[dict[str, Any]]] = []
    for line in proc.stdout.splitlines():
        if " ==>" not in line:
            continue
        _, value_hex = line.split(" ==>", 1)
        value_hex = value_hex.strip()
        if not value_hex.startswith("0x"):
            raise AuditError(f"unexpected ldb value format: {line}")
        raw = bytes.fromhex(value_hex[2:]).decode("utf-8")
        value = json.loads(raw)
        if not isinstance(value, list):
            raise AuditError("failure exemplar value is not a JSON array")
        bucket: list[dict[str, Any]] = []
        for item in value:
            if not isinstance(item, dict):
                raise AuditError("failure exemplar array contains a non-object")
            bucket.append(item)
        buckets.append(bucket)
    return buckets


def infer_language(row: dict[str, Any], task_id: str) -> str:
    for field in ("language", "lang", "repo_language"):
        value = row.get(field)
        if isinstance(value, str):
            return normalize_language(value)
    lowered = task_id.lower()
    for language in LANGUAGES:
        prefixes = (
            f"{language}::",
            f"{language}__",
            f"{language}-",
            f"{language}/",
            f"{language}:",
        )
        if lowered.startswith(prefixes):
            return language
    for alias, language in LANGUAGE_ALIASES.items():
        if lowered.startswith((f"{alias}::", f"{alias}__", f"{alias}-", f"{alias}/", f"{alias}:")):
            return language
    return "unknown"


def normalize_language(value: str) -> str:
    normalized = value.strip().lower().replace("-", "_")
    return LANGUAGE_ALIASES.get(normalized, normalized if normalized in LANGUAGES else "unknown")


def label_signal(oracle: dict[str, Any]) -> tuple[int, bool]:
    per_test = oracle.get("per_test")
    if per_test is None:
        per_test = []
    if not isinstance(per_test, list):
        raise AuditError("oracle_verdict.per_test must be an array")
    exception = oracle.get("exception")
    label_count = len(per_test) + (1 if exception is not None else 0)
    non_trivial = exception is not None
    for idx, test in enumerate(per_test):
        if not isinstance(test, dict):
            raise AuditError(f"oracle_verdict.per_test[{idx}] must be an object")
        outcome = str(test.get("outcome", "")).lower()
        if outcome in {"fail", "error", "skip", "flaky"}:
            non_trivial = True
    return label_count, non_trivial


def require_string(row: dict[str, Any], field: str) -> str:
    value = row.get(field)
    if not isinstance(value, str) or not value:
        raise AuditError(f"{field} must be a non-empty string")
    return value


def require_dict(row: dict[str, Any], field: str) -> dict[str, Any]:
    value = row.get(field)
    if not isinstance(value, dict):
        raise AuditError(f"{field} must be an object")
    return value


def render_markdown(payload: dict[str, Any]) -> str:
    lines = [
        "# ME-JEPA Label Quality Audit",
        "",
        "| Field | Value |",
        "|---|---|",
        f"| Source of truth | `{payload['sourceOfTruth']['columnFamily']}` |",
        f"| RocksDB path | `{payload['sourceOfTruth']['dbPath']}` |",
        f"| CF buckets | `{payload['cfBucketCount']}` |",
        f"| Total rows | `{payload['totalRows']}` |",
        f"| Unavailable signal rows | `{payload['totalUnavailableSignalRows']}` |",
        f"| Malformed rows | `{len(payload['malformedRows'])}` |",
        "",
        "## Per-Language Stats",
        "",
        "| Language | CF buckets | Rows | Labels/row mean | Min | Max | Non-trivial signal ratio | Unavailable rows | Examples |",
        "|---|---:|---:|---:|---:|---:|---:|---:|---|",
    ]
    for row in payload["languages"]:
        examples = ", ".join(f"`{item}`" for item in row["examples"]) or "-"
        lines.append(
            f"| `{row['language']}` | {row['cfBucketCount']} | {row['rows']} | "
            f"{row['labelsPerRowMean']:.3f} | {row['labelsPerRowMin']} | "
            f"{row['labelsPerRowMax']} | {row['nonTrivialSignalRatio']:.3f} | "
            f"{row['unavailableSignalRows']} | {examples} |"
        )
    if payload["malformedRows"]:
        lines += ["", "## Malformed Rows", ""]
        lines += [f"- {item}" for item in payload["malformedRows"]]
    lines.append("")
    return "\n".join(lines)


if __name__ == "__main__":
    try:
        raise SystemExit(main(sys.argv[1:]))
    except AuditError as exc:
        print(f"MEJEPA_LABEL_QUALITY_AUDIT_FAILED: {exc}", file=sys.stderr)
        raise SystemExit(1) from exc
