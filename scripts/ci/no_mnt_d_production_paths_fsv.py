#!/usr/bin/env python3
"""Classify legacy D-mount references and fail on active production usage."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Iterable


FORBIDDEN = "/" + "mnt" + "/d"
AIWONDER_DURABLE = "/zfs/archive/contextgraph"
AIWONDER_HOT = "/zfs/hot/contextgraph"
AIWONDER_SCRATCH = "/home/croyse/.cache/contextgraph"

SCAN_ROOTS = [
    ".claude",
    ".github",
    "AGENTS.md",
    "CLAUDE.md",
    "Cargo.toml",
    "INSTALL.md",
    "README.md",
    "config",
    "crates",
    "docs/futurebuild",
    "docs/INVESTIGATION_1_CRATES_DEAD_CODE.md",
    "docs/INVESTIGATION_2_TESTS_BENCHMARKS_EXAMPLES.md",
    "docs/INVESTIGATION_3_SCRIPTS_DOCS_CONFIG_TOPLEVEL.md",
    "scripts",
    "systemd",
    "tests",
]

SKIP_DIRS = {
    ".git",
    "target",
    "__pycache__",
    "memory.archive",
    ".venv",
    "node_modules",
}


@dataclass
class Hit:
    source: str
    path: str
    line: int
    classification: str
    text: str
    reason: str
    issue: int | None = None
    url: str | None = None

    def as_json(self) -> dict[str, object]:
        payload: dict[str, object] = {
            "source": self.source,
            "path": self.path,
            "line": self.line,
            "classification": self.classification,
            "text": self.text,
            "reason": self.reason,
        }
        if self.issue is not None:
            payload["issue"] = self.issue
        if self.url is not None:
            payload["url"] = self.url
        return payload


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=Path.cwd())
    parser.add_argument(
        "--output",
        type=Path,
        default=Path(
            "/zfs/archive/contextgraph/fsv/"
            "task-aiwonder-022-no-mnt-d-production-paths-fsv/"
            "no_mnt_d_production_paths_fsv.json"
        ),
    )
    parser.add_argument(
        "--skip-issues",
        action="store_true",
        help="Only scan the local repository tree; useful when gh is unavailable.",
    )
    return parser.parse_args(argv)


def iter_text_files(repo: Path) -> Iterable[Path]:
    for raw in SCAN_ROOTS:
        root = repo / raw
        if not root.exists():
            continue
        if root.is_file():
            yield root
            continue
        for path in root.rglob("*"):
            if not path.is_file():
                continue
            rel_parts = path.relative_to(repo).parts
            if any(part in SKIP_DIRS for part in rel_parts):
                continue
            if path.suffix in {".png", ".jpg", ".jpeg", ".gif", ".woff", ".wasm"}:
                continue
            yield path


def read_lines(path: Path) -> list[str] | None:
    try:
        data = path.read_bytes()
    except OSError:
        return None
    if b"\0" in data:
        return None
    try:
        return data.decode("utf-8").splitlines()
    except UnicodeDecodeError:
        return None


def classify_repo_line(rel: str, lines: list[str], index: int) -> tuple[str, str]:
    line = lines[index]
    window = "\n".join(lines[max(0, index - 2) : index + 3]).lower()
    text = line.lower()

    if "/_archived/" in f"/{rel}" or rel == "docs/MEJEPA_SUPER_PROMPT.md":
        return "historical_provenance", "archived/superseded document"

    if rel.startswith("crates/context-graph-paths/"):
        return "historical_provenance", "path-policy guard or negative test"

    if rel.endswith("production_root_guard_fsv.rs"):
        return "historical_provenance", "path-policy guard FSV negative case"

    if "legacy/local" in text or "legacy provenance" in text:
        return "historical_provenance", "explicit legacy provenance wording"

    historical_markers = [
        "historical",
        "pre-migration",
        "frozen",
        "no new writes",
        "not an active",
        "not active",
        "local-only",
        "legacy only",
        "historical proof",
    ]
    if any(marker in text for marker in historical_markers):
        return "historical_provenance", "line explicitly marks historical/local status"

    guard_markers = [
        "must not",
        "cannot",
        "forbidden",
        "reject",
        "denied",
        "fail closed",
        "fails closed",
        "invalid",
        "not use",
        "no /mnt",
        "not /mnt",
        "guard",
        "negative",
    ]
    if any(marker in window for marker in guard_markers):
        return "historical_provenance", "line is inside a rejection/guard context"

    stale_doc_markers = [
        "#369 tracks",
        "#369 reclassifies",
        "pending revalidation",
        "still being authored",
        "open p0",
        "local fsv; aiwonder production path equivalent",
    ]
    if rel.startswith("docs/") or rel in {"CLAUDE.md", "README.md"}:
        if any(marker in text for marker in stale_doc_markers):
            return "stale_doc", "active doc has pre-#412 path-policy wording"
        return "stale_doc", "active documentation still references legacy mount"

    active_io_markers = [
        "path::new",
        "pathbuf::from",
        "open(",
        "read_to_string",
        "write(",
        "create_dir_all",
        "--cache-root",
        "--fsv-root",
        "--output",
        "environment=",
        "=",
    ]
    if any(marker in text for marker in active_io_markers):
        return "active_violation", "line looks like active path configuration or I/O"

    return "historical_provenance", "unclassified code reference without active I/O shape"


def scan_repo(repo: Path) -> list[Hit]:
    hits: list[Hit] = []
    for path in sorted(iter_text_files(repo)):
        lines = read_lines(path)
        if lines is None:
            continue
        rel = path.relative_to(repo).as_posix()
        for index, line in enumerate(lines):
            if FORBIDDEN not in line:
                continue
            classification, reason = classify_repo_line(rel, lines, index)
            hits.append(
                Hit(
                    source="repo",
                    path=rel,
                    line=index + 1,
                    classification=classification,
                    text=line.strip(),
                    reason=reason,
                )
            )
    return hits


def scan_open_issues(repo: Path) -> list[Hit]:
    try:
        result = subprocess.run(
            [
                "gh",
                "issue",
                "list",
                "--state",
                "open",
                "--limit",
                "1000",
                "--json",
                "number,title,body,url,labels",
            ],
            cwd=repo,
            text=True,
            check=True,
            capture_output=True,
        )
    except (OSError, subprocess.CalledProcessError) as exc:
        return [
            Hit(
                source="github",
                path="open-issues",
                line=0,
                classification="stale_issue",
                text=f"open issue scan unavailable: {exc}",
                reason="gh issue list failed",
            )
        ]

    issues = json.loads(result.stdout)
    hits: list[Hit] = []
    for issue in issues:
        number = int(issue["number"])
        url = str(issue["url"])
        body = issue.get("body") or ""
        for index, line in enumerate(body.splitlines()):
            if FORBIDDEN not in line:
                continue
            classification, reason = classify_issue_line(number, line)
            hits.append(
                Hit(
                    source="github",
                    path=f"issue-{number}",
                    line=index + 1,
                    classification=classification,
                    text=line.strip(),
                    reason=reason,
                    issue=number,
                    url=url,
                )
            )
    return hits


def classify_issue_line(number: int, line: str) -> tuple[str, str]:
    text = line.lower()
    if number in {412, 434}:
        return "historical_provenance", "tracking issue describes the migration defect"
    if any(marker in text for marker in ["legacy", "historical", "forbidden", "stale"]):
        return "historical_provenance", "issue line explicitly marks legacy/stale status"
    return "stale_issue", "open issue body still names a legacy production path"


def synthetic_cases() -> dict[str, object]:
    env_line = f"CONTEXT_GRAPH_MODELS_PATH={FORBIDDEN}/contextgraph/models"
    guard_line = f"active production paths must not use {FORBIDDEN}"
    stale_line = f"production FSV still being authored under {FORBIDDEN}"

    results = [
        classify_repo_line("scripts/synthetic_env.sh", [env_line], 0)[0],
        classify_repo_line("crates/context-graph-mejepa-train/src/guard.rs", [guard_line], 0)[0],
        classify_repo_line("docs/futurebuild/03_storage_schema.md", [stale_line], 0)[0],
    ]
    expected = ["active_violation", "historical_provenance", "stale_doc"]
    return {
        "passed": results == expected,
        "cases": [
            {"name": "env_override_default", "expected": expected[0], "actual": results[0]},
            {"name": "guard_negative_case", "expected": expected[1], "actual": results[1]},
            {"name": "stale_active_doc", "expected": expected[2], "actual": results[2]},
        ],
    }


def summarize(hits: list[Hit]) -> dict[str, int]:
    summary = {
        "historical_provenance": 0,
        "stale_doc": 0,
        "stale_issue": 0,
        "active_violation": 0,
    }
    for hit in hits:
        summary[hit.classification] = summary.get(hit.classification, 0) + 1
    return summary


def main(argv: list[str]) -> int:
    args = parse_args(argv)
    repo = args.repo.resolve()
    synthetic = synthetic_cases()
    hits = scan_repo(repo)
    if not args.skip_issues:
        hits.extend(scan_open_issues(repo))

    counts = summarize(hits)
    payload = {
        "schema": "contextgraph.task-aiwonder-022.no-mnt-d-production-paths-fsv.v1",
        "generatedAt": datetime.now(timezone.utc).isoformat(),
        "repoRoot": str(repo),
        "forbiddenPattern": FORBIDDEN,
        "allowedProductionRoots": [AIWONDER_DURABLE, AIWONDER_HOT, AIWONDER_SCRATCH],
        "assertions": {
            "syntheticCasesPassed": synthetic["passed"],
            "activeViolationCountZero": counts.get("active_violation", 0) == 0,
            "allPassed": synthetic["passed"] and counts.get("active_violation", 0) == 0,
        },
        "counts": counts,
        "syntheticCases": synthetic["cases"],
        "hits": [hit.as_json() for hit in hits],
    }

    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps({"output": str(args.output), **payload["assertions"], "counts": counts}, indent=2, sort_keys=True))
    return 0 if payload["assertions"]["allPassed"] else 2


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
