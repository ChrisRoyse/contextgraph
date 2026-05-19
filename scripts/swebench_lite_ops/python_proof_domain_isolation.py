"""Audit that proof-domain harnesses do not gate the Python ship path."""

from __future__ import annotations

import argparse
import json
import re
import shutil
import subprocess
from pathlib import Path
from typing import Any

from .smoke_intelligence_matrices import dump_json, iter_jsonl, sha256_file, tree_state, utc_now, write_jsonl


SCHEMA_VERSION = 1
TASK_ID = "TASK-PY-G-037"
ARTIFACT_KIND = "python_proof_domain_isolation"
FORMULA_VERSION = "python_proof_domain_isolation_v1"
DEFAULT_OUTPUT_ROOT = Path("/zfs/archive/contextgraph/corpus/python-proof-domain-isolation-v1")
DEFAULT_FSV_ROOT = Path("/zfs/archive/contextgraph/fsv/task-py-g-037-proof-domain-isolation-fsv")

PROOF_DOMAIN_PATTERNS: tuple[tuple[str, re.Pattern[str]], ...] = (
    ("gridworld", re.compile(r"\bgridworld\b", re.IGNORECASE)),
    ("counter_world", re.compile(r"\bcounter[_ -]?world\b", re.IGNORECASE)),
    ("onet", re.compile(r"\bO\*?NET\b|\bONET\b", re.IGNORECASE)),
    ("career_taxonomy", re.compile(r"\bcareer[- ]taxonomy\b|\bcareer taxonomy\b", re.IGNORECASE)),
    ("dynamicjepa", re.compile(r"\bdynamicjepa\b", re.IGNORECASE)),
    ("proof_domain", re.compile(r"\bproof[- ]domain\b|\bproof domain\b", re.IGNORECASE)),
)

ALLOW_CONTEXT_PATTERNS: tuple[re.Pattern[str], ...] = (
    re.compile(r"\barchiv(?:e|ed|al)\b", re.IGNORECASE),
    re.compile(r"\barchived_", re.IGNORECASE),
    re.compile(r"\bdeferred\b", re.IGNORECASE),
    re.compile(r"\bout of scope\b", re.IGNORECASE),
    re.compile(r"\bnot active\b", re.IGNORECASE),
    re.compile(r"\bnot part of the active\b", re.IGNORECASE),
    re.compile(r"\bnot .*gridworld\b", re.IGNORECASE),
    re.compile(r"\bno .*proof[- ]domain\b", re.IGNORECASE),
    re.compile(r"\bno .*gridworld\b", re.IGNORECASE),
    re.compile(r"\bdoes not belong\b", re.IGNORECASE),
    re.compile(r"\bdo not belong\b", re.IGNORECASE),
    re.compile(r"\bnot .*active .*prerequisite", re.IGNORECASE),
    re.compile(r"\bexcluded\b", re.IGNORECASE),
    re.compile(r"\breject(?:s|ed|ion)?\b", re.IGNORECASE),
    re.compile(r"\bfail[- ]closed\b", re.IGNORECASE),
    re.compile(r"\bstale\b", re.IGNORECASE),
    re.compile(r"\bquarantine(?:d|s)?\b", re.IGNORECASE),
    re.compile(r"\bsibling\b", re.IGNORECASE),
    re.compile(r"\bhistorical\b", re.IGNORECASE),
    re.compile(r"\bfuture issue\b", re.IGNORECASE),
    re.compile(r"\bfuture-generalization\b", re.IGNORECASE),
    re.compile(r"\bguard issue\b", re.IGNORECASE),
    re.compile(r"\bproof-domain isolation\b", re.IGNORECASE),
    re.compile(r"\bproof-domain-isolation\b", re.IGNORECASE),
)

FORBIDDEN_PREREQUISITE_PATTERNS: tuple[re.Pattern[str], ...] = (
    re.compile(r"\brequired before\b", re.IGNORECASE),
    re.compile(r"\bmust pass\b", re.IGNORECASE),
    re.compile(r"\brun .*local_ci_gate", re.IGNORECASE),
    re.compile(r"\blocal_ci_gate\.sh\b", re.IGNORECASE),
)

DEFAULT_ACTIVE_PATHS = (
    "docs/futurebuild",
    "scripts/swebench_lite_ops",
    "scripts/render_ship_gate_dashboard.py",
    ".github/workflows",
    "crates/context-graph-cli/examples",
)

TEXT_EXTENSIONS = {
    "",
    ".md",
    ".py",
    ".rs",
    ".toml",
    ".yml",
    ".yaml",
    ".json",
    ".sh",
    ".txt",
}


class ProofDomainIsolationError(RuntimeError):
    pass


def require(condition: bool, code: str, message: str, details: dict[str, Any] | None = None) -> None:
    if not condition:
        raise ProofDomainIsolationError(json.dumps({"error_code": code, "message": message, "details": details or {}}, sort_keys=True))


def parse_paths(value: str) -> list[str]:
    paths = [item.strip() for item in value.split(",") if item.strip()]
    require(bool(paths), "PY_PROOF_DOMAIN_EMPTY_ACTIVE_SCOPE", "active scope path list is empty")
    require(len(paths) == len(set(paths)), "PY_PROOF_DOMAIN_DUPLICATE_ACTIVE_SCOPE", "active scope path list contains duplicates", {"paths": paths})
    return paths


def token_hits(text: str) -> list[str]:
    hits = []
    for token, pattern in PROOF_DOMAIN_PATTERNS:
        if pattern.search(text):
            hits.append(token)
    return hits


def allowed_by_context(text: str) -> tuple[bool, str | None]:
    for pattern in ALLOW_CONTEXT_PATTERNS:
        if pattern.search(text):
            return True, pattern.pattern
    return False, None


def has_forbidden_prerequisite_language(text: str) -> bool:
    return any(pattern.search(text) for pattern in FORBIDDEN_PREREQUISITE_PATTERNS)


def classify_reference(
    *,
    source_kind: str,
    source_id: str,
    path: str | None,
    line_number: int | None,
    text: str,
    current_guard_issue: int,
) -> dict[str, Any] | None:
    hits = token_hits(text)
    if not hits:
        return None

    normalized_path = path or ""
    allowed_context, allow_pattern = allowed_by_context(text)
    if normalized_path in {
        "scripts/swebench_lite_ops/python_proof_domain_isolation.py",
        "scripts/swebench_lite_ops/cli.py",
        "scripts/swebench_lite_ops/python_instrument_registry.py",
    }:
        status = "allowed_current_guard_implementation"
        active_python_prerequisite = False
    elif source_kind == "github_issue" and source_id == str(current_guard_issue):
        status = "allowed_current_guard_issue"
        active_python_prerequisite = False
    elif normalized_path.startswith(".github/workflows/ci_smoke_"):
        status = "allowed_legacy_sibling_workflow_not_python_gate"
        active_python_prerequisite = False
    elif allowed_context and not has_forbidden_prerequisite_language(text):
        status = "allowed_archived_or_fail_closed_reference"
        active_python_prerequisite = False
    else:
        status = "forbidden_active_python_prerequisite_reference"
        active_python_prerequisite = True

    return {
        "schema_version": SCHEMA_VERSION,
        "artifact_kind": "python_proof_domain_reference",
        "task_id": TASK_ID,
        "source_kind": source_kind,
        "source_id": source_id,
        "path": path,
        "line_number": line_number,
        "tokens": hits,
        "text": text.strip()[:500],
        "status": status,
        "active_python_prerequisite": active_python_prerequisite,
        "allow_pattern": allow_pattern,
    }


def iter_active_files(repo_root: Path, active_paths: list[str]) -> tuple[list[dict[str, Any]], list[Path]]:
    states: list[dict[str, Any]] = []
    files: list[Path] = []
    for raw in active_paths:
        path = (repo_root / raw).resolve()
        state = {
            "requested_path": raw,
            "resolved_path": str(path),
            "exists": path.exists(),
            "file_count": 0,
        }
        if path.is_file():
            if path.suffix in TEXT_EXTENSIONS:
                files.append(path)
                state["file_count"] = 1
        elif path.is_dir():
            for child in sorted(path.rglob("*")):
                if not child.is_file():
                    continue
                if any(part in {".git", "__pycache__", "target"} for part in child.parts):
                    continue
                if child.suffix not in TEXT_EXTENSIONS:
                    continue
                files.append(child)
            state["file_count"] = sum(1 for child in files if raw in str(child))
        states.append(state)
    return states, sorted(set(files))


def scan_files(repo_root: Path, active_paths: list[str], current_guard_issue: int) -> tuple[list[dict[str, Any]], list[dict[str, Any]], dict[str, Any]]:
    path_states, files = iter_active_files(repo_root, active_paths)
    rows: list[dict[str, Any]] = []
    scanned_files = 0
    read_errors = []
    for path in files:
        rel = path.relative_to(repo_root).as_posix()
        try:
            lines = path.read_text(encoding="utf-8", errors="strict").splitlines()
        except UnicodeDecodeError as exc:
            read_errors.append({"path": rel, "error": str(exc)})
            continue
        scanned_files += 1
        for line_number, line in enumerate(lines, start=1):
            if not token_hits(line):
                continue
            start = max(0, line_number - 5)
            end = min(len(lines), line_number + 8)
            context = "\n".join(lines[start:end])
            row = classify_reference(
                source_kind="repo_file",
                source_id=rel,
                path=rel,
                line_number=line_number,
                text=context,
                current_guard_issue=current_guard_issue,
            )
            if row is not None:
                rows.append(row)
    summary = {
        "active_scope_paths": active_paths,
        "path_states": path_states,
        "scanned_file_count": scanned_files,
        "read_error_count": len(read_errors),
        "read_errors": read_errors,
    }
    return rows, path_states, summary


def load_github_issues(repo: str, max_issues: int) -> list[dict[str, Any]]:
    require(max_issues > 0, "PY_PROOF_DOMAIN_INVALID_MAX_ISSUES", "max GitHub issues must be positive", {"max_issues": max_issues})
    require(max_issues <= 500, "PY_PROOF_DOMAIN_OVER_LIMIT_GITHUB_ISSUES", "max GitHub issues exceeds the fail-closed limit", {"max_issues": max_issues, "limit": 500})
    command = [
        "gh",
        "issue",
        "list",
        "--repo",
        repo,
        "--state",
        "open",
        "--json",
        "number,title,body,labels,assignees",
        "--limit",
        str(max_issues),
    ]
    result = subprocess.run(command, check=False, text=True, capture_output=True)
    require(result.returncode == 0, "PY_PROOF_DOMAIN_GH_ISSUE_LIST_FAILED", "gh issue list failed", {"stderr": result.stderr.strip(), "returncode": result.returncode})
    payload = json.loads(result.stdout)
    require(isinstance(payload, list), "PY_PROOF_DOMAIN_GH_ISSUE_LIST_NOT_LIST", "gh issue list did not return a JSON list")
    return payload


def scan_github_issues(issues: list[dict[str, Any]], current_guard_issue: int) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for issue in issues:
        number = int(issue.get("number") or 0)
        title = str(issue.get("title") or "")
        body = str(issue.get("body") or "")
        lines = [title, *body.splitlines()]
        for line_number, line in enumerate(lines, start=1):
            if not token_hits(line):
                continue
            start = max(0, line_number - 5)
            end = min(len(lines), line_number + 8)
            context = "\n".join(lines[start:end])
            row = classify_reference(
                source_kind="github_issue",
                source_id=str(number),
                path=None,
                line_number=line_number,
                text=context,
                current_guard_issue=current_guard_issue,
            )
            if row is not None:
                row["issue_title"] = title
                rows.append(row)
    return rows


def edge_case_states(current_guard_issue: int) -> list[dict[str, Any]]:
    empty_scope_pass = False
    try:
        parse_paths("")
    except ProofDomainIsolationError:
        empty_scope_pass = True
    misleading = classify_reference(
        source_kind="synthetic",
        source_id="misleading-prerequisite",
        path="synthetic.md",
        line_number=1,
        text="gridworld is required before the Python ship gate can pass",
        current_guard_issue=current_guard_issue,
    )
    archived = classify_reference(
        source_kind="synthetic",
        source_id="archived-reference",
        path="synthetic.md",
        line_number=2,
        text="gridworld is archived and not active for the Python ship gate",
        current_guard_issue=current_guard_issue,
    )
    guard = classify_reference(
        source_kind="github_issue",
        source_id=str(current_guard_issue),
        path=None,
        line_number=None,
        text="TASK-PY-G-037 isolates proof-domain harnesses from Python ship gate",
        current_guard_issue=current_guard_issue,
    )
    multiline_exclusion = classify_reference(
        source_kind="synthetic",
        source_id="multiline-exclusion",
        path="synthetic.md",
        line_number=3,
        text="Gridworld, counter-world, O*NET,\ncareer-taxonomy, and other proof-domain harnesses are archived research\nfixtures; they are not active prerequisites for the Python ship gate.",
        current_guard_issue=current_guard_issue,
    )
    over_limit_pass = False
    try:
        load_github_issues("ChrisRoyse/contextgraph", 501)
    except ProofDomainIsolationError:
        over_limit_pass = True
    return [
        {
            "case": "empty_active_scope_rejected",
            "before": {"active_scope": ""},
            "action": "parse active scope paths",
            "after": {"rejected": empty_scope_pass},
            "expected": "empty scope cannot produce a fake pass",
            "pass": empty_scope_pass,
        },
        {
            "case": "misleading_proof_domain_prerequisite_forbidden",
            "before": {"line": "gridworld is required before the Python ship gate can pass"},
            "action": "classify reference",
            "after": misleading,
            "expected": "active_python_prerequisite is true",
            "pass": bool(misleading and misleading["active_python_prerequisite"] is True),
        },
        {
            "case": "archived_reference_allowed",
            "before": {"line": "gridworld is archived and not active for the Python ship gate"},
            "action": "classify reference",
            "after": archived,
            "expected": "archived/not-active language makes it non-gating",
            "pass": bool(archived and archived["active_python_prerequisite"] is False),
        },
        {
            "case": "current_guard_issue_allowed",
            "before": {"issue": current_guard_issue},
            "action": "classify current guard issue text",
            "after": guard,
            "expected": "current guard issue can mention proof-domain scope",
            "pass": bool(guard and guard["status"] == "allowed_current_guard_issue"),
        },
        {
            "case": "multiline_active_scope_exclusion_allowed",
            "before": {"line": "Gridworld, counter-world, O*NET, ... they are not active prerequisites for the Python ship gate."},
            "action": "classify multiline exclusion reference",
            "after": multiline_exclusion,
            "expected": "proof-domain list is allowed when the same paragraph explicitly excludes it from active prerequisites",
            "pass": bool(multiline_exclusion and multiline_exclusion["active_python_prerequisite"] is False),
        },
        {
            "case": "github_issue_batch_over_limit_rejected",
            "before": {"max_issues": 501, "limit": 500},
            "action": "load GitHub issues",
            "after": {"rejected": over_limit_pass},
            "expected": "over-limit issue batch is rejected",
            "pass": over_limit_pass,
        },
    ]


def add_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--repo-root", type=Path, default=Path.cwd())
    parser.add_argument("--repo", default="ChrisRoyse/contextgraph")
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--fsv-root", type=Path, default=DEFAULT_FSV_ROOT)
    parser.add_argument("--active-paths", default=",".join(DEFAULT_ACTIVE_PATHS))
    parser.add_argument("--current-guard-issue", type=int, default=179)
    parser.add_argument("--max-github-issues", type=int, default=200)
    parser.add_argument("--skip-github", action="store_true")
    parser.add_argument("--clean-output", action="store_true")


def command_python_proof_domain_isolation(args: argparse.Namespace) -> int:
    if args.clean_output:
        if args.output_root.exists():
            shutil.rmtree(args.output_root)
        if args.fsv_root.exists():
            shutil.rmtree(args.fsv_root)
    args.output_root.mkdir(parents=True, exist_ok=True)
    args.fsv_root.mkdir(parents=True, exist_ok=True)
    before = {"output_root": tree_state(args.output_root), "fsv_root": tree_state(args.fsv_root)}

    active_paths = parse_paths(args.active_paths)
    repo_root = args.repo_root.resolve()
    repo_rows, path_states, file_scan_summary = scan_files(repo_root, active_paths, args.current_guard_issue)
    issue_rows: list[dict[str, Any]] = []
    issue_summary: dict[str, Any] = {"skipped": bool(args.skip_github), "open_issue_count": None}
    if not args.skip_github:
        issues = load_github_issues(args.repo, args.max_github_issues)
        issue_summary["open_issue_count"] = len(issues)
        issue_rows = scan_github_issues(issues, args.current_guard_issue)

    forbidden_repo_rows = [row for row in repo_rows if row["active_python_prerequisite"]]
    forbidden_issue_rows = [row for row in issue_rows if row["active_python_prerequisite"]]
    allowed_rows = [row for row in repo_rows + issue_rows if not row["active_python_prerequisite"]]
    edges = edge_case_states(args.current_guard_issue)
    report = {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "artifact_kind": ARTIFACT_KIND,
        "formula_version": FORMULA_VERSION,
        "created_at_utc": utc_now(),
        "python_ship_gate_active_domain": "python_swebench_lite_300x8",
        "policy": {
            "proof_domains": ["gridworld", "counter_world", "onet", "career_taxonomy", "dynamicjepa", "proof_domain"],
            "allowed_only_when": "archived_deferred_excluded_fail_closed_sibling_or_current_guard_issue",
            "generic_ci_must_not_run": "scripts/5090jepa/local_ci_gate.sh",
        },
        "source_of_truth": {
            "repo_root": str(repo_root),
            "output_root": str(args.output_root),
            "fsv_root": str(args.fsv_root),
            "github_repo": args.repo,
            "active_paths": active_paths,
        },
        "summary": {
            "repo_reference_rows": len(repo_rows),
            "github_issue_reference_rows": len(issue_rows),
            "allowed_reference_rows": len(allowed_rows),
            "forbidden_repo_reference_rows": len(forbidden_repo_rows),
            "forbidden_github_issue_reference_rows": len(forbidden_issue_rows),
            "scanned_file_count": file_scan_summary["scanned_file_count"],
            "active_scope_existing_paths": sum(1 for row in path_states if row["exists"]),
            "github_scan_skipped": bool(args.skip_github),
            "edge_case_count": len(edges),
            "all_edge_cases_pass": all(row["pass"] for row in edges),
        },
    }

    write_jsonl(args.output_root / "active_reference_audit.jsonl", repo_rows)
    write_jsonl(args.output_root / "github_issue_scope_audit.jsonl", issue_rows)
    write_jsonl(args.output_root / "allowed_reference_sample.jsonl", allowed_rows[:200])
    write_jsonl(args.output_root / "forbidden_reference_rows.jsonl", forbidden_repo_rows + forbidden_issue_rows)
    write_jsonl(args.output_root / "edge_case_states.jsonl", edges)
    dump_json(args.output_root / "proof_domain_isolation_report.json", report)

    readback_report = json.loads((args.output_root / "proof_domain_isolation_report.json").read_text(encoding="utf-8"))
    readback_counts = {
        "active_reference_audit": sum(1 for _ in iter_jsonl(args.output_root / "active_reference_audit.jsonl")),
        "github_issue_scope_audit": sum(1 for _ in iter_jsonl(args.output_root / "github_issue_scope_audit.jsonl")),
        "forbidden_reference_rows": sum(1 for _ in iter_jsonl(args.output_root / "forbidden_reference_rows.jsonl")),
        "edge_case_states": sum(1 for _ in iter_jsonl(args.output_root / "edge_case_states.jsonl")),
    }
    passes = (
        readback_report["summary"]["scanned_file_count"] > 0
        and readback_report["summary"]["active_scope_existing_paths"] > 0
        and readback_report["summary"]["forbidden_repo_reference_rows"] == 0
        and readback_report["summary"]["forbidden_github_issue_reference_rows"] == 0
        and readback_counts["forbidden_reference_rows"] == 0
        and readback_counts["edge_case_states"] == len(edges)
        and all(row["pass"] for row in edges)
    )
    fsv = {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "artifact_kind": f"{ARTIFACT_KIND}_fsv",
        "formula_version": FORMULA_VERSION,
        "created_at_utc": utc_now(),
        "passes": passes,
        "source_of_truth": report["source_of_truth"],
        "before": before,
        "after": {"output_root": tree_state(args.output_root), "fsv_root": tree_state(args.fsv_root)},
        "readback_counts": readback_counts,
        "summary": readback_report["summary"],
        "edge_cases": edges,
        "report_sha256": sha256_file(args.output_root / "proof_domain_isolation_report.json"),
    }
    fsv_path = args.fsv_root / "python_proof_domain_isolation_fsv.json"
    dump_json(fsv_path, fsv)
    if not passes:
        raise ProofDomainIsolationError(json.dumps({"error_code": "PY_PROOF_DOMAIN_ISOLATION_FAILED", "message": "proof-domain isolation audit found active Python prerequisite drift", "fsv_path": str(fsv_path), "summary": fsv["summary"]}, sort_keys=True))
    print(json.dumps({"passes": True, "fsv_path": str(fsv_path), "report_path": str(args.output_root / "proof_domain_isolation_report.json"), "summary": report["summary"]}, indent=2, sort_keys=True))
    return 0
