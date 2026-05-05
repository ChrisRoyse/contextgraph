#!/usr/bin/env python3
"""Run local Ollama models in a ContextGraph-guided SWE-bench Lite repair loop.

This is a hard-task benchmark harness, not a deterministic tool-choice matrix.
For each selected SWE-bench Lite task, a local model proposes a unified diff.
The harness applies that patch to a real repository checkout, applies the
official test patch, runs the FAIL_TO_PASS tests, records the attempt in a
BlackBoxFix ledger, and feeds verified failure evidence into the next prompt.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import sqlite3
import subprocess
import sys
import textwrap
import time
import socket
import urllib.error
import urllib.request
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from reality_prompt_compiler import PromptCompilerError, compile_reality_prompt


DATASET_NAME = "princeton-nlp/SWE-bench_Lite"
DEFAULT_MODELS = ["gemma4:e4b", "qwen3:8b", "mistral-nemo:12b"]
DEFAULT_RUNTIME_ROOT = Path("/tmp/contextgraph-swe-lite-refinement")
DEFAULT_CONTEXT_GRAPH_BIN = Path("target/debug/context-graph")
DEFAULT_OFFICIAL_EVAL_SCRIPT = Path(__file__).resolve().with_name("swe_lite_official_eval.py")
DEFAULT_SWEBENCH_PYTHON = Path("/home/cabdru/.cache/contextgraph-swebench/venv/bin/python")
DEFAULT_DOCKER_CLI_DIR = Path("/mnt/wsl/docker-desktop/cli-tools/usr/bin")
DEFAULT_DOCKER_SOCKET = Path("/mnt/wsl/docker-desktop-bind-mounts/Ubuntu-24.04/docker.sock")
DEFAULT_DOCKER_WINDOWS_BIN = Path("/mnt/c/Program Files/Docker/Docker/resources/bin")
REPO_ROOT = Path(__file__).resolve().parent.parent
MAX_FAILURE_TEXT_CHARS = 6000
MAX_REPO_CONTEXT_CHARS = 24000
DEFAULT_AGENT_TOOL_ROUNDS = 2
MAX_AGENT_TOOL_CALLS_PER_ROUND = 4
MAX_AGENT_TOOL_FILE_CHARS = 12000
MAX_AGENT_TOOL_SEARCH_RESULTS = 40
TEXT_CONTEXT_EXTENSIONS = {
    ".cfg",
    ".ini",
    ".md",
    ".py",
    ".pyi",
    ".rst",
    ".toml",
    ".txt",
    ".yaml",
    ".yml",
}
LOW_SIGNAL_TERMS = {
    "also",
    "args",
    "because",
    "being",
    "between",
    "binary",
    "broken",
    "command",
    "converts",
    "difference",
    "doesn",
    "exist",
    "found",
    "from",
    "gets",
    "handled",
    "have",
    "here",
    "including",
    "literal",
    "longer",
    "makes",
    "method",
    "problem",
    "python",
    "right",
    "request",
    "requests",
    "response",
    "safe",
    "seems",
    "sessions",
    "some",
    "starting",
    "string",
    "sure",
    "that",
    "there",
    "this",
    "tried",
    "tries",
    "uses",
    "using",
    "utils",
    "value",
    "values",
    "when",
    "which",
    "with",
    "would",
    "work",
}


class RefinementError(RuntimeError):
    def __init__(
        self,
        error_code: str,
        message: str,
        field_path: str,
        remediation: str,
        *,
        details: dict[str, Any] | None = None,
        source_of_truth: str | None = None,
    ) -> None:
        super().__init__(message)
        self.error_code = error_code
        self.message = message
        self.field_path = field_path
        self.remediation = remediation
        self.details = details or {}
        self.source_of_truth = source_of_truth

    def to_json(self) -> dict[str, Any]:
        return {
            "status": "error",
            "error_code": self.error_code,
            "message": self.message,
            "field_path": self.field_path,
            "remediation": self.remediation,
            "details": self.details,
            "source_of_truth": self.source_of_truth,
        }


@dataclass
class ProcessResult:
    command: list[str]
    cwd: str
    exit_code: int
    stdout: str
    stderr: str
    elapsed_ms: int

    def to_json(self) -> dict[str, Any]:
        return {
            "command": self.command,
            "cwd": self.cwd,
            "exit_code": self.exit_code,
            "stdout": self.stdout,
            "stderr": self.stderr,
            "elapsed_ms": self.elapsed_ms,
        }


def now_ns() -> int:
    return time.time_ns()


def file_sot(path: Path) -> str:
    return f"file:{path}"


def sqlite_sot(path: Path) -> str:
    return f"sqlite:{path}#ledger_records"


def git_sot(path: Path, ref: str) -> str:
    return f"git:{path}#{ref}"


def sha256_bytes(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def sha256_text(text: str) -> str:
    return sha256_bytes(text.encode("utf-8"))


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return "sha256:" + digest.hexdigest()


def write_json(path: Path, value: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_name(f".{path.name}.tmp-{os.getpid()}-{now_ns()}")
    tmp.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    os.replace(tmp, path)
    readback = json.loads(path.read_text(encoding="utf-8"))
    if readback != value:
        raise RefinementError(
            "SWE_REFINEMENT_JSON_READBACK_MISMATCH",
            "persisted JSON readback did not match the in-memory payload",
            str(path),
            "inspect filesystem writes before continuing the benchmark",
            source_of_truth=file_sot(path),
        )


def write_text_checked(path: Path, value: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_name(f".{path.name}.tmp-{os.getpid()}-{now_ns()}")
    tmp.write_text(value, encoding="utf-8")
    os.replace(tmp, path)
    readback = path.read_text(encoding="utf-8")
    if readback != value:
        raise RefinementError(
            "SWE_REFINEMENT_TEXT_READBACK_MISMATCH",
            "persisted text readback did not match the in-memory payload",
            str(path),
            "inspect filesystem writes before continuing the benchmark",
            source_of_truth=file_sot(path),
        )


def run_cmd(
    command: list[str],
    *,
    cwd: Path,
    timeout: float,
    input_text: str | None = None,
) -> ProcessResult:
    started = time.perf_counter()
    try:
        output = subprocess.run(
            command,
            cwd=str(cwd),
            input=input_text,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=timeout,
            check=False,
        )
        exit_code = output.returncode
        stdout = output.stdout
        stderr = output.stderr
    except subprocess.TimeoutExpired as err:
        exit_code = 124
        stdout = err.stdout if isinstance(err.stdout, str) else ""
        stderr = (err.stderr if isinstance(err.stderr, str) else "") + f"\nTIMEOUT after {timeout}s"
    elapsed_ms = int((time.perf_counter() - started) * 1000)
    return ProcessResult(command, str(cwd), exit_code, stdout, stderr, elapsed_ms)


def require_context_graph(path: Path) -> Path:
    resolved = path if path.is_absolute() else Path.cwd() / path
    if not resolved.exists():
        raise RefinementError(
            "SWE_REFINEMENT_CONTEXT_GRAPH_BIN_MISSING",
            "context-graph binary does not exist",
            "context_graph_bin",
            "build the CLI with `cargo build --package context-graph-cli --bin context-graph`",
            source_of_truth=file_sot(resolved),
        )
    return resolved


def resolve_repo_path(path: Path) -> Path:
    return path if path.is_absolute() else REPO_ROOT / path


def request_json(
    url: str,
    body: dict[str, Any],
    *,
    timeout: float,
) -> dict[str, Any]:
    request = urllib.request.Request(
        url,
        data=json.dumps(body).encode("utf-8"),
        headers={"Content-Type": "application/json", "Accept": "application/json"},
        method="POST",
    )
    try:
        with urllib.request.urlopen(request, timeout=timeout) as response:
            raw = response.read()
            status = response.status
    except urllib.error.HTTPError as err:
        raw = err.read()
        status = err.code
    except (urllib.error.URLError, TimeoutError, socket.timeout) as err:
        raise RefinementError(
            "SWE_REFINEMENT_OLLAMA_TIMEOUT",
            "Ollama did not return a model response before the configured timeout",
            "ollama.response",
            "reduce prompt context, increase --model-timeout, or inspect Ollama/GPU health before rerunning",
            details={"url": url, "timeout_seconds": timeout, "error": str(err)},
            source_of_truth=url,
        ) from err
    try:
        value = json.loads(raw.decode("utf-8"))
    except json.JSONDecodeError as err:
        raise RefinementError(
            "SWE_REFINEMENT_OLLAMA_JSON_INVALID",
            "Ollama returned a response that was not valid JSON",
            "ollama.response",
            "inspect Ollama server logs and response truncation before rerunning",
            details={"url": url, "status": status, "raw_preview": raw[:2000].decode("utf-8", errors="replace")},
            source_of_truth=url,
        ) from err
    if status >= 400 or value.get("status") == "error":
        raise RefinementError(
            "SWE_REFINEMENT_OLLAMA_HTTP_ERROR",
            "Ollama returned an error during model generation",
            "ollama.response",
            "inspect the model tag, prompt size, and Ollama server state",
            details={"status": status, "response": value},
            source_of_truth=url,
        )
    return value


def swe_list_field(item: dict[str, Any], field: str) -> list[str]:
    value = item.get(field)
    if value is None:
        return []
    if isinstance(value, list):
        return [str(part) for part in value]
    if isinstance(value, str):
        stripped = value.strip()
        if not stripped:
            return []
        try:
            parsed = json.loads(stripped)
        except json.JSONDecodeError as err:
            raise RefinementError(
                "SWE_REFINEMENT_DATASET_FIELD_INVALID",
                f"SWE-bench field {field} was not valid JSON list text",
                field,
                "inspect the dataset row before running validation",
                details={"value_preview": stripped[:500]},
                source_of_truth=f"huggingface:{DATASET_NAME}:test",
            ) from err
        if not isinstance(parsed, list):
            raise RefinementError(
                "SWE_REFINEMENT_DATASET_FIELD_NOT_LIST",
                f"SWE-bench field {field} did not decode to a list",
                field,
                "inspect the dataset row before running validation",
                details={"decoded_type": type(parsed).__name__},
                source_of_truth=f"huggingface:{DATASET_NAME}:test",
            )
        return [str(part) for part in parsed]
    raise RefinementError(
        "SWE_REFINEMENT_DATASET_FIELD_UNSUPPORTED",
        f"SWE-bench field {field} had an unsupported type",
        field,
        "inspect the dataset row before running validation",
        details={"type": type(value).__name__},
        source_of_truth=f"huggingface:{DATASET_NAME}:test",
    )


def normalize_swe_task(item: dict[str, Any]) -> dict[str, Any]:
    task = dict(item)
    task["FAIL_TO_PASS"] = swe_list_field(task, "FAIL_TO_PASS")
    task["PASS_TO_PASS"] = swe_list_field(task, "PASS_TO_PASS")
    return task


def load_swe_lite_tasks(
    *,
    instance_ids: list[str],
    repo_filter: list[str],
    max_tasks: int,
) -> list[dict[str, Any]]:
    try:
        from datasets import load_dataset
    except ImportError as err:
        raise RefinementError(
            "SWE_REFINEMENT_DATASETS_MISSING",
            "Python package `datasets` is required to read SWE-bench Lite metadata",
            "python.imports.datasets",
            "install datasets or run in an environment that already has it",
        ) from err

    wanted_ids = set(instance_ids)
    wanted_repos = set(repo_filter)
    tasks: list[dict[str, Any]] = []
    for item in load_dataset(DATASET_NAME, split="test", streaming=True):
        if wanted_ids and item["instance_id"] not in wanted_ids:
            continue
        if wanted_repos and item["repo"] not in wanted_repos:
            continue
        tasks.append(normalize_swe_task(dict(item)))
        if len(tasks) >= max_tasks:
            break
    if not tasks:
        raise RefinementError(
            "SWE_REFINEMENT_TASK_SELECTION_EMPTY",
            "no SWE-bench Lite tasks matched the selection",
            "task_selection",
            "choose a valid --instance-id or --repo-filter from SWE-bench Lite",
            details={"instance_ids": instance_ids, "repo_filter": repo_filter, "max_tasks": max_tasks},
            source_of_truth=f"huggingface:{DATASET_NAME}:test",
        )
    return tasks


def truncate(text: str, limit: int = MAX_FAILURE_TEXT_CHARS) -> str:
    if len(text) <= limit:
        return text
    return text[:limit] + f"\n... truncated {len(text) - limit} chars ..."


def unique_strings(values: list[str], *, limit: int | None = None) -> list[str]:
    output: list[str] = []
    seen: set[str] = set()
    for value in values:
        normalized = str(value).strip()
        key = normalized.lower()
        if normalized and key not in seen:
            seen.add(key)
            output.append(normalized)
        if limit is not None and len(output) >= limit:
            break
    return output


def problem_terms(task: dict[str, Any]) -> list[str]:
    text = task.get("problem_statement") or ""
    terms: list[str] = []
    for quoted in re.findall(r"`([^`]{3,120})`|\"([^\"]{3,120})\"|'([^']{3,120})'|“([^”]{3,120})”", text):
        value = next((part for part in quoted if part), "").strip()
        if value:
            terms.append(value)
    for line in text.splitlines():
        stripped = line.strip()
        if 3 <= len(stripped) <= 140 and re.search(r"[._=()/:]", stripped):
            terms.append(stripped)
    for token in re.findall(r"\b[A-Za-z_][A-Za-z0-9_]{3,}\b", text):
        token_lower = token.lower()
        if token_lower not in LOW_SIGNAL_TERMS:
            terms.append(token)
    return unique_strings(terms, limit=80)


def term_weight(term: str) -> int:
    lower = term.lower().strip()
    if lower in LOW_SIGNAL_TERMS:
        return 0
    if "=" in term:
        return 800
    if "(" in term or ")" in term:
        return 220
    if "/" in term:
        return 180
    if "." in term:
        return 90
    if any(ch in term for ch in "_'\""):
        return 80
    if len(term) >= 12:
        return 18
    if len(term) >= 7:
        return 8
    return 2


def line_distance_ok(candidate: int, selected: list[int], radius: int) -> bool:
    return all(abs(candidate - existing) > radius for existing in selected)


def add_context_hit(
    path_hits: dict[str, dict[str, Any]],
    rel: str,
    *,
    line_no: int,
    score: int,
    reason: str,
    term: str | None = None,
) -> None:
    entry = path_hits.setdefault(
        rel,
        {"score": 0, "line_scores": {}, "reasons": set(), "terms": set()},
    )
    entry["score"] += score
    entry["line_scores"][line_no] = entry["line_scores"].get(line_no, 0) + score
    entry["reasons"].add(reason)
    if term:
        entry["terms"].add(term)


def repo_text_files(repo_dir: Path) -> list[Path]:
    files: list[Path] = []
    for path in repo_dir.rglob("*"):
        if ".git" in path.parts or not path.is_file():
            continue
        if path.suffix not in TEXT_CONTEXT_EXTENSIONS:
            continue
        try:
            if path.stat().st_size > 2_000_000:
                continue
        except OSError:
            continue
        files.append(path)
    return sorted(files)


def read_text_lines(path: Path) -> list[str]:
    try:
        return path.read_text(encoding="utf-8").splitlines()
    except UnicodeDecodeError:
        return path.read_text(encoding="latin-1").splitlines()


def parse_failed_hunks(validation: dict[str, Any] | None) -> list[dict[str, Any]]:
    if not validation:
        return []
    stderr = (
        validation.get("details", {})
        .get("git_apply", {})
        .get("stderr", "")
    )
    hunks: list[dict[str, Any]] = []
    for match in re.finditer(r"error: patch failed: ([^:\n]+):(\d+)", stderr):
        hunks.append({"path": match.group(1), "line": int(match.group(2)), "reason": "candidate_patch_failed_here"})
    return hunks


def terms_from_official_evidence(official_evidence: dict[str, Any] | None) -> list[str]:
    if not official_evidence:
        return []
    terms: list[str] = []
    tests = []
    tests.extend(official_evidence.get("failed_fail_to_pass") or [])
    tests.extend(official_evidence.get("failed_pass_to_pass") or [])
    tests.extend(official_evidence.get("failed_fail_to_fail") or [])
    tests.extend(official_evidence.get("failed_pass_to_fail") or [])
    for test in tests:
        value = str(test)
        terms.append(value)
        terms.extend(re.findall(r"\b[A-Za-z_][A-Za-z0-9_]{2,}\b", value))
        for path_token in re.findall(r"\b[\w./-]+\.(?:py|pyi)\b", value):
            terms.append(path_token)
    excerpt = str(official_evidence.get("test_output_excerpt") or "")
    for token in re.findall(r"\b[A-Za-z_][A-Za-z0-9_]{3,}\b", excerpt):
        if token.lower() not in LOW_SIGNAL_TERMS:
            terms.append(token)
    return unique_strings(terms, limit=80)


def line_window(lines: list[str], center: int, radius: int) -> tuple[int, int, list[str]]:
    start = max(1, center - radius)
    end = min(len(lines), center + radius)
    numbered = [f"{line_no:5d}: {lines[line_no - 1]}" for line_no in range(start, end + 1)]
    return start, end, numbered


def collect_repository_context(
    task: dict[str, Any],
    repo_dir: Path,
    *,
    validation: dict[str, Any] | None,
    official_evidence: dict[str, Any] | None = None,
    max_files: int,
    window_radius: int,
) -> dict[str, Any]:
    official_terms = terms_from_official_evidence(official_evidence)
    terms = unique_strings(problem_terms(task) + official_terms, limit=140)
    failed_hunks = parse_failed_hunks(validation)
    path_hits: dict[str, dict[str, Any]] = {}

    for hunk in failed_hunks:
        rel = hunk["path"]
        add_context_hit(
            path_hits,
            rel,
            line_no=hunk["line"],
            score=1000,
            reason=hunk["reason"],
        )

    for path_token in re.findall(r"\b[\w./-]+\.(?:py|pyi|cfg|ini|toml|yaml|yml|rst|md|txt)\b", task.get("problem_statement") or ""):
        rel = path_token.lstrip("./")
        if (repo_dir / rel).exists():
            add_context_hit(
                path_hits,
                rel,
                line_no=1,
                score=25,
                reason="problem_statement_path",
                term=rel,
            )

    for path_token in official_terms:
        if not re.search(r"\.(?:py|pyi|cfg|ini|toml|yaml|yml|rst|md|txt)$", path_token):
            continue
        candidates = [path_token.lstrip("./")]
        if "/" not in path_token:
            candidates.extend(str(path.relative_to(repo_dir)) for path in repo_dir.rglob(path_token) if path.is_file())
        for rel in unique_strings(candidates, limit=12):
            if (repo_dir / rel).exists():
                add_context_hit(
                    path_hits,
                    rel,
                    line_no=1,
                    score=120,
                    reason="official_failed_test_path",
                    term=rel,
                )

    for path in repo_text_files(repo_dir):
        rel = str(path.relative_to(repo_dir))
        try:
            lines = read_text_lines(path)
        except OSError:
            continue
        for line_no, line in enumerate(lines, start=1):
            lower_line = line.lower()
            for term in terms:
                weight = term_weight(term)
                if weight <= 0:
                    continue
                term_lower = term.lower()
                if not term_lower or term_lower not in lower_line:
                    continue
                add_context_hit(
                    path_hits,
                    rel,
                    line_no=line_no,
                    score=weight,
                    reason="problem_statement_term",
                    term=term,
                )

    selected = sorted(
        path_hits.items(),
        key=lambda item: (-item[1]["score"], item[0]),
    )[:max_files]
    files = []
    for rel, meta in selected:
        path = repo_dir / rel
        if not path.exists():
            continue
        lines = read_text_lines(path)
        ranked_lines = [
            line_no
            for line_no, _score in sorted(
                meta["line_scores"].items(),
                key=lambda item: (-item[1], item[0]),
            )
        ]
        centers = []
        for line_no in ranked_lines:
            if line_distance_ok(line_no, centers, max(4, window_radius // 2)):
                centers.append(line_no)
            if len(centers) >= 5:
                break
        if not centers:
            centers = [1]
        snippets = []
        for center in centers:
            start, end, numbered = line_window(lines, center, window_radius)
            snippets.append({"start": start, "end": end, "text": "\n".join(numbered)})
        files.append(
            {
                "path": rel,
                "sha256": sha256_file(path),
                "score": meta["score"],
                "matched_terms": sorted(meta["terms"])[:12],
                "reasons": sorted(meta["reasons"]),
                "snippets": snippets,
            }
        )

    payload = {
        "status": "ok",
        "repo": task["repo"],
        "base_commit": task["base_commit"],
        "source_of_truth": git_sot(repo_dir, task["base_commit"]),
        "terms": terms[:30],
        "official_evidence_terms": official_terms[:30],
        "failed_hunks": failed_hunks,
        "files": files,
    }
    return payload


def repository_context_text(context: dict[str, Any]) -> str:
    if not context.get("files"):
        return "No repository context matched the problem statement. The model must still produce a patch against the exact base commit."
    sections = []
    for item in context["files"]:
        snippets = "\n\n".join(snippet["text"] for snippet in item["snippets"])
        sections.append(
            f"File: {item['path']}\n"
            f"Reasons: {', '.join(item['reasons'])}\n"
            f"Matched terms: {', '.join(item['matched_terms'])}\n"
            f"SHA-256: {item['sha256']}\n"
            f"{snippets}"
        )
    return truncate("\n\n---\n\n".join(sections), MAX_REPO_CONTEXT_CHARS)


def build_initial_prompt(task: dict[str, Any], *, include_hints: bool, repo_context: dict[str, Any]) -> str:
    hints = task.get("hints_text") or ""
    hint_block = f"\n\nHints from SWE-bench metadata:\n{hints}" if include_hints and hints.strip() else ""
    repo_context_block = repository_context_text(repo_context)
    return textwrap.dedent(
        f"""
        You are repairing a real SWE-bench Lite issue.

        Repository: {task['repo']}
        Base commit: {task['base_commit']}
        Instance id: {task['instance_id']}

        Problem statement:
        {task['problem_statement']}
        {hint_block}

        Repository context read from the exact base commit:
        {repo_context_block}

        Produce exactly one JSON object with these keys:
        - "patch": a unified git diff patch that applies to the base repository.
        - "notes": one short sentence explaining the intended fix.

        Success means the official SWE-bench Docker report puts this instance id in resolved_ids.
        Local patch application or model confidence is not success.
        The patch should be minimal. Do not include repeated no-op hunks. Do not include markdown outside the JSON object.
        Do not include tests from the hidden test patch.
        """
    ).strip()


def build_refinement_prompt(task: dict[str, Any], validation: dict[str, Any], repo_context: dict[str, Any]) -> str:
    evidence = {
        "status": validation.get("status"),
        "error_code": validation.get("error_code"),
        "field_path": validation.get("field_path"),
        "message": validation.get("message"),
        "remediation": validation.get("remediation"),
        "details": validation.get("details"),
    }
    repo_context_block = repository_context_text(repo_context)
    return textwrap.dedent(
        f"""
        Continue the same SWE-bench Lite repair task. Your previous attempt did not pass verification.

        Repository: {task['repo']}
        Base commit: {task['base_commit']}
        Instance id: {task['instance_id']}

        Problem statement:
        {task['problem_statement']}

        Verified failure evidence from ContextGraph:
        {json.dumps(evidence, indent=2, sort_keys=True)[:MAX_FAILURE_TEXT_CHARS]}

        Repository context re-read from durable workspace state:
        {repo_context_block}

        Produce a new patch that addresses the verified failure. Respond with exactly one JSON object:
        {{"patch": "<unified git diff>", "notes": "<short explanation>"}}

        Success means the official SWE-bench Docker report puts this instance id in resolved_ids.
        Local patch application or model confidence is not success.
        The patch should be minimal. Do not repeat the same patch if the failure evidence shows it did not apply or did not pass tests.
        Do not include repeated no-op hunks.
        """
    ).strip()


def build_official_refinement_prompt(
    task: dict[str, Any],
    official_evidence: dict[str, Any],
    repo_context: dict[str, Any],
    *,
    previous_patch_hashes: list[str],
) -> str:
    evidence = {
        "official_status": official_evidence.get("official_status"),
        "resolved": official_evidence.get("resolved"),
        "errored": official_evidence.get("errored"),
        "patch_successfully_applied": official_evidence.get("patch_successfully_applied"),
        "failed_fail_to_pass": official_evidence.get("failed_fail_to_pass"),
        "failed_pass_to_pass": official_evidence.get("failed_pass_to_pass"),
        "test_output_excerpt": official_evidence.get("test_output_excerpt"),
        "previous_patch_sha256": official_evidence.get("previous_patch_sha256"),
        "reward": official_evidence.get("reward"),
        "score": official_evidence.get("score"),
        "source_of_truth": official_evidence.get("source_of_truth"),
    }
    repo_context_block = repository_context_text(repo_context)
    no_repeat = "\n".join(f"- {value}" for value in previous_patch_hashes) or "- none"
    previous_patch = official_evidence.get("previous_patch_preview") or ""
    return textwrap.dedent(
        f"""
        Continue the same SWE-bench Lite repair task. Your previous attempt was evaluated by the official Docker SWE-bench harness.

        Repository: {task['repo']}
        Base commit: {task['base_commit']}
        Instance id: {task['instance_id']}

        Success condition:
        The official SWE-bench Docker report must put {task['instance_id']} in resolved_ids.
        Local patch application, local pytest, or model confidence is not success.

        Problem statement:
        {task['problem_statement']}

        Verified official failure evidence from ContextGraph:
        {json.dumps(evidence, indent=2, sort_keys=True)[:MAX_FAILURE_TEXT_CHARS]}

        Previous patch preview:
        {truncate(str(previous_patch), 4000)}

        Patch hashes already tried. Do not repeat an unchanged patch:
        {no_repeat}

        Repository context re-read from the base checkout after the official failure:
        {repo_context_block}

        Produce a new patch that addresses the official failure. Respond with exactly one JSON object:
        {{"patch": "<unified git diff>", "notes": "<short explanation>"}}

        The patch should be minimal. Do not include markdown outside the JSON object.
        Do not edit tests. Do not include repeated no-op hunks. Do not repeat an unchanged patch.
        """
    ).strip()


def write_prompt_manifest(
    attempt_dir: Path,
    *,
    attempt: int,
    prompt_path: Path,
    prompt_kind: str,
    prompt_template_id: str,
    included_evidence: list[dict[str, Any]],
    excluded_evidence: list[dict[str, Any]],
    no_repeat_patch_sha256: list[str],
    prompt_compiler: dict[str, Any] | None = None,
) -> dict[str, Any]:
    manifest_path = attempt_dir / "prompt-manifest.json"
    payload = {
        "status": "ok",
        "attempt": attempt,
        "prompt_kind": prompt_kind,
        "prompt_template_id": prompt_template_id,
        "prompt_path": str(prompt_path),
        "prompt_sha256": sha256_file(prompt_path),
        "included_evidence": included_evidence,
        "excluded_evidence": excluded_evidence,
        "no_repeat_patch_sha256": no_repeat_patch_sha256,
        "prompt_compiler": prompt_compiler,
        "source_of_truth": file_sot(manifest_path),
    }
    write_json(manifest_path, payload)
    payload["sha256"] = sha256_file(manifest_path)
    return payload


def write_next_prompt_candidate(
    task_root: Path,
    *,
    next_attempt: int,
    source_attempt: int,
    prompt: str,
    prompt_context: dict[str, Any],
    prompt_kind: str,
    prompt_template_id: str,
    included_evidence: list[dict[str, Any]],
    excluded_evidence: list[dict[str, Any]],
    no_repeat_patch_sha256: list[str],
    reason: str,
    reality_branch_id: str,
    parent_reality_branch_id: str,
    prompt_compiler: dict[str, Any] | None = None,
) -> dict[str, Any]:
    candidate_dir = task_root / "prompt-candidates" / f"attempt-{next_attempt:03d}"
    candidate_path = candidate_dir / "next-prompt-candidate.json"
    candidate_dir.mkdir(parents=True, exist_ok=False)
    context_path = candidate_dir / "repository-context.json"
    prompt_path = candidate_dir / "prompt.txt"
    write_json(context_path, prompt_context)
    write_text_checked(prompt_path, prompt)
    prompt_manifest = write_prompt_manifest(
        candidate_dir,
        attempt=next_attempt,
        prompt_path=prompt_path,
        prompt_kind=prompt_kind,
        prompt_template_id=prompt_template_id,
        included_evidence=included_evidence,
        excluded_evidence=excluded_evidence,
        no_repeat_patch_sha256=no_repeat_patch_sha256,
        prompt_compiler=prompt_compiler,
    )
    candidate = {
        "status": "ok",
        "source_attempt": source_attempt,
        "next_attempt": next_attempt,
        "reason": reason,
        "reality_branch_id": reality_branch_id,
        "parent_reality_branch_id": parent_reality_branch_id,
        "promotion_status": "candidate_prompt",
        "prompt": {
            "path": str(prompt_path),
            "sha256": sha256_file(prompt_path),
            "source_of_truth": file_sot(prompt_path),
        },
        "prompt_manifest": {
            "path": str(candidate_dir / "prompt-manifest.json"),
            "sha256": prompt_manifest["sha256"],
            "source_of_truth": file_sot(candidate_dir / "prompt-manifest.json"),
        },
        "repository_context": {
            "path": str(context_path),
            "sha256": sha256_file(context_path),
            "source_of_truth": file_sot(context_path),
        },
        "prompt_compiler": prompt_compiler,
        "source_of_truth": file_sot(candidate_path),
    }
    candidate["payload_sha256"] = sha256_text(json_compact(candidate))
    write_json(candidate_path, candidate)
    candidate["file_sha256"] = sha256_file(candidate_path)
    return candidate


def prompt_compiler_manifest(compiled: dict[str, Any]) -> dict[str, Any]:
    return {
        "compiler_version": compiled.get("compiler_version"),
        "prompt_template_id": compiled.get("prompt_template_id"),
        "prompt_sha256": compiled.get("prompt_sha256"),
        "payload_sha256": compiled.get("payload_sha256"),
        "quality": compiled.get("quality"),
        "input_refs": compiled.get("input_refs"),
    }


def json_candidates(response_text: str) -> list[str]:
    stripped = response_text.strip()
    candidates = [stripped]
    fence = re.search(r"```(?:json)?\s*(\{.*?\})\s*```", stripped, flags=re.DOTALL)
    if fence:
        candidates.insert(0, fence.group(1))
    if "{" in stripped and "}" in stripped:
        candidates.append(stripped[stripped.find("{") : stripped.rfind("}") + 1])
    return candidates


def parse_model_patch(response_text: str) -> tuple[str | None, dict[str, Any]]:
    parse_errors = []
    for candidate in json_candidates(response_text):
        try:
            value = json.loads(candidate)
        except json.JSONDecodeError as err:
            parse_errors.append(str(err))
            continue
        patch = value.get("patch")
        if isinstance(patch, str) and patch.strip():
            return patch, {"status": "ok", "format": "json", "notes": value.get("notes")}
        return None, {
            "status": "error",
            "error_code": "SWE_MODEL_PATCH_FIELD_MISSING",
            "message": "model JSON did not contain a non-empty patch string",
            "parsed_keys": sorted(value) if isinstance(value, dict) else None,
        }
    return None, {
        "status": "error",
        "error_code": "SWE_MODEL_JSON_INVALID",
        "message": "model response was not parseable as the required JSON object",
        "parse_errors": parse_errors[:3],
    }


def parse_model_tool_calls(response_text: str) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    parse_errors = []
    for candidate in json_candidates(response_text):
        try:
            value = json.loads(candidate)
        except json.JSONDecodeError as err:
            parse_errors.append(str(err))
            continue
        if not isinstance(value, dict):
            return [], {
                "status": "error",
                "error_code": "SWE_MODEL_TOOL_JSON_NOT_OBJECT",
                "message": "model tool response was not a JSON object",
                "parsed_type": type(value).__name__,
            }
        tool_calls = value.get("tool_calls")
        if tool_calls is None:
            return [], {
                "status": "no_tool_calls",
                "parsed_keys": sorted(value),
            }
        if not isinstance(tool_calls, list) or not tool_calls:
            return [], {
                "status": "error",
                "error_code": "SWE_MODEL_TOOL_CALLS_INVALID",
                "message": "tool_calls must be a non-empty list",
                "parsed_tool_calls_type": type(tool_calls).__name__,
            }
        parsed = []
        for index, call in enumerate(tool_calls[:MAX_AGENT_TOOL_CALLS_PER_ROUND], start=1):
            if not isinstance(call, dict):
                parsed.append({"name": None, "arguments": {}, "index": index, "parse_error": "call was not an object"})
                continue
            name = call.get("name")
            arguments = call.get("arguments") or {}
            parsed.append(
                {
                    "name": name if isinstance(name, str) else None,
                    "arguments": arguments if isinstance(arguments, dict) else {},
                    "index": index,
                    "parse_error": None if isinstance(name, str) and isinstance(arguments, dict) else "invalid name or arguments",
                }
            )
        return parsed, {
            "status": "ok",
            "total_requested": len(tool_calls),
            "executed_count": len(parsed),
            "truncated": len(tool_calls) > MAX_AGENT_TOOL_CALLS_PER_ROUND,
        }
    return [], {
        "status": "error",
        "error_code": "SWE_MODEL_TOOL_JSON_INVALID",
        "message": "model response was not parseable as a tool-call JSON object",
        "parse_errors": parse_errors[:3],
    }


def ollama_chat(
    *,
    ollama_url: str,
    model: str,
    messages: list[dict[str, str]],
    timeout: float,
) -> dict[str, Any]:
    body = {
        "model": model,
        "messages": messages,
        "stream": False,
        "think": False,
        "options": {"temperature": 0, "num_predict": 4096},
    }
    response = request_json(f"{ollama_url.rstrip('/')}/api/chat", body, timeout=timeout)
    message = response.get("message")
    if not isinstance(message, dict) or not isinstance(message.get("content"), str):
        raise RefinementError(
            "SWE_REFINEMENT_MODEL_RESPONSE_INVALID",
            "Ollama response did not contain message.content",
            "ollama.response.message.content",
            "inspect the selected model and Ollama API response",
            details={"response": response},
            source_of_truth=f"{ollama_url.rstrip('/')}/api/chat",
        )
    return {
        "content": message["content"],
        "content_sha256": sha256_text(message["content"]),
        "metadata": {
            "model": response.get("model"),
            "created_at": response.get("created_at"),
            "done": response.get("done"),
            "done_reason": response.get("done_reason"),
            "total_duration": response.get("total_duration"),
            "load_duration": response.get("load_duration"),
            "prompt_eval_count": response.get("prompt_eval_count"),
            "eval_count": response.get("eval_count"),
        },
    }


def ollama_generate_patch(
    *,
    ollama_url: str,
    model: str,
    prompt: str,
    timeout: float,
) -> dict[str, Any]:
    return ollama_chat(
        ollama_url=ollama_url,
        model=model,
        messages=[
            {
                "role": "system",
                "content": (
                    "You are a software repair model. Output only the required JSON object. "
                    "The patch must be a unified git diff against the provided base repository."
                ),
            },
            {"role": "user", "content": prompt},
        ],
        timeout=timeout,
    )


def agent_tool_surface() -> list[dict[str, Any]]:
    return [
        {
            "name": "list_files",
            "mode": "read_only",
            "arguments": {"path": "optional repo-relative directory", "pattern": "optional substring", "max_results": "optional integer"},
            "source_of_truth": "git workspace filesystem",
        },
        {
            "name": "read_file",
            "mode": "read_only",
            "arguments": {"path": "required repo-relative file", "start_line": "optional integer", "end_line": "optional integer"},
            "source_of_truth": "git workspace file bytes plus sha256",
        },
        {
            "name": "search_repo",
            "mode": "read_only",
            "arguments": {"query": "required literal text", "max_results": "optional integer"},
            "source_of_truth": "git workspace scanned files plus sha256",
        },
        {
            "name": "inspect_repository_context",
            "mode": "read_only",
            "arguments": {},
            "source_of_truth": "pre-materialized repository context JSON",
        },
        {
            "name": "read_failure_evidence",
            "mode": "read_only",
            "arguments": {"kind": "optional evidence kind"},
            "source_of_truth": "persisted validation or official SWE-bench evidence files",
        },
    ]


def agent_tool_surface_text() -> str:
    return json.dumps(agent_tool_surface(), indent=2, sort_keys=True)


def repo_path_from_argument(repo_dir: Path, raw_path: Any) -> tuple[Path | None, dict[str, Any] | None]:
    if not isinstance(raw_path, str) or not raw_path.strip():
        return None, {
            "status": "blocked",
            "error_code": "SWE_AGENT_TOOL_PATH_REQUIRED",
            "field_path": "arguments.path",
            "message": "tool path must be a non-empty repo-relative string",
            "remediation": "request a real repo-relative path from list_files or repository context",
        }
    rel = Path(raw_path.strip())
    if rel.is_absolute() or ".." in rel.parts:
        return None, {
            "status": "blocked",
            "error_code": "SWE_AGENT_TOOL_PATH_OUTSIDE_REPO",
            "field_path": "arguments.path",
            "message": "tool path must stay inside the checked-out repository",
            "remediation": "use only repo-relative paths returned by list_files or search_repo",
            "details": {"path": raw_path},
            "source_of_truth": file_sot(repo_dir),
        }
    resolved = (repo_dir / rel).resolve()
    repo_resolved = repo_dir.resolve()
    if not str(resolved).startswith(str(repo_resolved) + os.sep) and resolved != repo_resolved:
        return None, {
            "status": "blocked",
            "error_code": "SWE_AGENT_TOOL_PATH_ESCAPE",
            "field_path": "arguments.path",
            "message": "resolved tool path escaped the repository root",
            "remediation": "use only repo-relative paths returned by list_files or search_repo",
            "details": {"path": raw_path, "resolved": str(resolved)},
            "source_of_truth": file_sot(repo_dir),
        }
    return resolved, None


def bounded_int(value: Any, *, default: int, minimum: int, maximum: int) -> int:
    if value is None:
        return default
    try:
        parsed = int(value)
    except (TypeError, ValueError):
        return default
    return max(minimum, min(maximum, parsed))


def tool_result_blocked(name: str | None, arguments: dict[str, Any], error: dict[str, Any], *, index: int) -> dict[str, Any]:
    return {
        "tool_call_index": index,
        "tool_name": name,
        "arguments": arguments,
        "status": "blocked",
        "error": error,
        "source_of_truth_refs": {},
    }


def execute_agent_tool_call(
    *,
    repo_dir: Path,
    repo_context: dict[str, Any],
    included_evidence: list[dict[str, Any]],
    tool_call: dict[str, Any],
) -> dict[str, Any]:
    name = tool_call.get("name")
    arguments = tool_call.get("arguments") if isinstance(tool_call.get("arguments"), dict) else {}
    index = int(tool_call.get("index") or 0)
    if tool_call.get("parse_error"):
        return tool_result_blocked(
            name,
            arguments,
            {
                "status": "blocked",
                "error_code": "SWE_AGENT_TOOL_CALL_PARSE_ERROR",
                "field_path": f"tool_calls[{index}]",
                "message": str(tool_call["parse_error"]),
                "remediation": "emit tool calls as {\"tool_calls\":[{\"name\":\"read_file\",\"arguments\":{\"path\":\"...\"}}]}",
            },
            index=index,
        )
    if name not in {tool["name"] for tool in agent_tool_surface()}:
        return tool_result_blocked(
            name,
            arguments,
            {
                "status": "blocked",
                "error_code": "SWE_AGENT_TOOL_UNKNOWN",
                "field_path": f"tool_calls[{index}].name",
                "message": "requested tool is not registered in the governed ContextGraph tool surface",
                "remediation": "choose one of the registered read-only tools",
                "details": {"available": [tool["name"] for tool in agent_tool_surface()]},
            },
            index=index,
        )

    if name == "list_files":
        root_path, error = repo_path_from_argument(repo_dir, arguments.get("path") or ".")
        if error:
            return tool_result_blocked(name, arguments, error, index=index)
        assert root_path is not None
        if not root_path.exists():
            return tool_result_blocked(
                name,
                arguments,
                {
                    "status": "blocked",
                    "error_code": "SWE_AGENT_TOOL_LIST_PATH_MISSING",
                    "field_path": "arguments.path",
                    "message": "requested list_files path does not exist in the workspace",
                    "remediation": "request a path present in repository context",
                    "source_of_truth": file_sot(root_path),
                },
                index=index,
            )
        pattern = str(arguments.get("pattern") or "").lower()
        max_results = bounded_int(arguments.get("max_results"), default=60, minimum=1, maximum=200)
        candidates = root_path.rglob("*") if root_path.is_dir() else [root_path]
        files = []
        for path in sorted(candidates):
            if ".git" in path.parts or not path.is_file():
                continue
            rel = str(path.relative_to(repo_dir))
            if pattern and pattern not in rel.lower():
                continue
            files.append({"path": rel, "size_bytes": path.stat().st_size, "sha256": sha256_file(path)})
            if len(files) >= max_results:
                break
        return {
            "tool_call_index": index,
            "tool_name": name,
            "arguments": arguments,
            "status": "ok",
            "result": {"files": files, "truncated": len(files) >= max_results},
            "source_of_truth_refs": {"repo": git_sot(repo_dir, "workspace"), "root": file_sot(root_path)},
        }

    if name == "read_file":
        path, error = repo_path_from_argument(repo_dir, arguments.get("path"))
        if error:
            return tool_result_blocked(name, arguments, error, index=index)
        assert path is not None
        if not path.exists() or not path.is_file():
            return tool_result_blocked(
                name,
                arguments,
                {
                    "status": "blocked",
                    "error_code": "SWE_AGENT_TOOL_FILE_MISSING",
                    "field_path": "arguments.path",
                    "message": "requested file does not exist in the workspace",
                    "remediation": "request a path returned by list_files or search_repo",
                    "source_of_truth": file_sot(path),
                },
                index=index,
            )
        lines = read_text_lines(path)
        start_line = bounded_int(arguments.get("start_line"), default=1, minimum=1, maximum=max(1, len(lines)))
        end_line = bounded_int(arguments.get("end_line"), default=min(len(lines), start_line + 200), minimum=start_line, maximum=len(lines))
        selected = "\n".join(f"{line_no:5d}: {lines[line_no - 1]}" for line_no in range(start_line, end_line + 1))
        truncated = len(selected) > MAX_AGENT_TOOL_FILE_CHARS
        return {
            "tool_call_index": index,
            "tool_name": name,
            "arguments": arguments,
            "status": "ok",
            "result": {
                "path": str(path.relative_to(repo_dir)),
                "sha256": sha256_file(path),
                "start_line": start_line,
                "end_line": end_line,
                "text": truncate(selected, MAX_AGENT_TOOL_FILE_CHARS),
                "truncated": truncated,
            },
            "source_of_truth_refs": {"file": file_sot(path), "repo": git_sot(repo_dir, "workspace")},
        }

    if name == "search_repo":
        query = arguments.get("query")
        if not isinstance(query, str) or not query.strip():
            return tool_result_blocked(
                name,
                arguments,
                {
                    "status": "blocked",
                    "error_code": "SWE_AGENT_TOOL_QUERY_REQUIRED",
                    "field_path": "arguments.query",
                    "message": "search_repo query must be a non-empty string",
                    "remediation": "search for a symbol, error text, option name, or failed test token",
                },
                index=index,
            )
        max_results = bounded_int(arguments.get("max_results"), default=MAX_AGENT_TOOL_SEARCH_RESULTS, minimum=1, maximum=MAX_AGENT_TOOL_SEARCH_RESULTS)
        query_lower = query.lower()
        hits = []
        for path in repo_text_files(repo_dir):
            try:
                lines = read_text_lines(path)
            except OSError:
                continue
            for line_no, line in enumerate(lines, start=1):
                if query_lower in line.lower():
                    hits.append(
                        {
                            "path": str(path.relative_to(repo_dir)),
                            "line": line_no,
                            "text": line[:500],
                            "file_sha256": sha256_file(path),
                        }
                    )
                    if len(hits) >= max_results:
                        break
            if len(hits) >= max_results:
                break
        return {
            "tool_call_index": index,
            "tool_name": name,
            "arguments": arguments,
            "status": "ok",
            "result": {"query": query, "hits": hits, "truncated": len(hits) >= max_results},
            "source_of_truth_refs": {"repo": git_sot(repo_dir, "workspace")},
        }

    if name == "inspect_repository_context":
        return {
            "tool_call_index": index,
            "tool_name": name,
            "arguments": arguments,
            "status": "ok",
            "result": repo_context,
            "source_of_truth_refs": {"repository_context": repo_context.get("source_of_truth"), "repo": git_sot(repo_dir, "workspace")},
        }

    if name == "read_failure_evidence":
        requested_kind = arguments.get("kind")
        evidence_rows = []
        for item in included_evidence:
            if requested_kind and item.get("kind") != requested_kind:
                continue
            source = item.get("source_of_truth")
            if not isinstance(source, str) or not source.startswith("file:"):
                evidence_rows.append({"kind": item.get("kind"), "status": "blocked", "reason": "non-file evidence source", "source_of_truth": source})
                continue
            path = Path(source[len("file:") :])
            if not path.exists():
                evidence_rows.append({"kind": item.get("kind"), "status": "blocked", "reason": "evidence file missing", "source_of_truth": source})
                continue
            text = path.read_text(encoding="utf-8", errors="replace")
            evidence_rows.append(
                {
                    "kind": item.get("kind"),
                    "status": "ok",
                    "path": str(path),
                    "sha256": sha256_file(path),
                    "preview": truncate(text, MAX_FAILURE_TEXT_CHARS),
                    "source_of_truth": source,
                }
            )
        return {
            "tool_call_index": index,
            "tool_name": name,
            "arguments": arguments,
            "status": "ok",
            "result": {"evidence": evidence_rows},
            "source_of_truth_refs": {"included_evidence": [item.get("source_of_truth") for item in included_evidence]},
        }

    raise AssertionError(f"unhandled tool {name}")


def run_model_tool_rounds(
    *,
    ollama_url: str,
    model: str,
    prompt: str,
    timeout: float,
    repo_dir: Path,
    repo_context: dict[str, Any],
    included_evidence: list[dict[str, Any]],
    attempt_dir: Path,
    max_rounds: int,
) -> dict[str, Any]:
    tool_dir = attempt_dir / "agent-tools"
    transcript_path = tool_dir / "agent-tool-transcript.json"
    transcript = {
        "status": "disabled" if max_rounds <= 0 else "running",
        "max_rounds": max_rounds,
        "tool_surface": agent_tool_surface(),
        "rounds": [],
        "source_of_truth": file_sot(transcript_path),
    }
    if max_rounds <= 0:
        response = ollama_generate_patch(ollama_url=ollama_url, model=model, prompt=prompt, timeout=timeout)
        transcript["status"] = "disabled"
        write_json(transcript_path, transcript)
        response["tool_transcript"] = {
            "path": str(transcript_path),
            "sha256": sha256_file(transcript_path),
            "source_of_truth": file_sot(transcript_path),
            "rounds_total": 0,
        }
        return response

    messages = [
        {
            "role": "system",
            "content": (
                "You are a software repair agent operating through a governed ContextGraph tool gateway. "
                "You may request read-only tools, then must submit a final unified git diff as JSON. "
                "Do not claim success; success is only the official SWE-bench Docker report."
            ),
        },
        {
            "role": "user",
            "content": (
                prompt
                + "\n\nGoverned tools available before final patch:\n"
                + agent_tool_surface_text()
                + "\n\nTo request tools, output exactly JSON like "
                + '{"tool_calls":[{"name":"search_repo","arguments":{"query":"timeout","max_results":10}}]}. '
                + 'When ready, output exactly {"patch":"<unified git diff>","notes":"<short explanation>"}.\n'
            ),
        },
    ]

    final_response: dict[str, Any] | None = None
    for round_index in range(1, max_rounds + 1):
        round_dir = tool_dir / f"round-{round_index:03d}"
        round_dir.mkdir(parents=True, exist_ok=False)
        response = ollama_chat(ollama_url=ollama_url, model=model, messages=messages, timeout=timeout)
        request_path = round_dir / "model-tool-request.txt"
        write_text_checked(request_path, response["content"])
        patch, patch_parse = parse_model_patch(response["content"])
        if patch:
            final_response = response
            transcript["rounds"].append(
                {
                    "round": round_index,
                    "status": "final_patch_submitted",
                    "model_response": {"path": str(request_path), "sha256": sha256_file(request_path), "source_of_truth": file_sot(request_path)},
                    "patch_parse": patch_parse,
                }
            )
            break
        tool_calls, tool_parse = parse_model_tool_calls(response["content"])
        tool_results = []
        for call_index, tool_call in enumerate(tool_calls, start=1):
            result = execute_agent_tool_call(
                repo_dir=repo_dir,
                repo_context=repo_context,
                included_evidence=included_evidence,
                tool_call=tool_call,
            )
            result["payload_sha256"] = sha256_text(json_compact(result))
            result_path = round_dir / f"tool-result-{call_index:03d}.json"
            write_json(result_path, result)
            result["file"] = {"path": str(result_path), "sha256": sha256_file(result_path), "source_of_truth": file_sot(result_path)}
            tool_results.append(result)
        transcript["rounds"].append(
            {
                "round": round_index,
                "status": "tools_executed" if tool_results else "no_tools_requested",
                "model_response": {"path": str(request_path), "sha256": sha256_file(request_path), "source_of_truth": file_sot(request_path)},
                "tool_parse": tool_parse,
                "tool_results": [
                    {
                        "tool_name": result.get("tool_name"),
                        "status": result.get("status"),
                        "file": result.get("file"),
                        "source_of_truth_refs": result.get("source_of_truth_refs"),
                    }
                    for result in tool_results
                ],
            }
        )
        if not tool_results:
            messages.append({"role": "assistant", "content": response["content"]})
            messages.append(
                {
                    "role": "user",
                    "content": (
                        "ContextGraph did not receive a valid tool request. "
                        "Now submit the final patch JSON only. If you cannot repair it, submit the smallest patch you can justify from durable evidence."
                    ),
                }
            )
            break
        messages.append({"role": "assistant", "content": response["content"]})
        messages.append(
            {
                "role": "user",
                "content": (
                    "Verified tool results from ContextGraph source-of-truth readback:\n"
                    + json.dumps(tool_results, indent=2, sort_keys=True)[:MAX_REPO_CONTEXT_CHARS]
                    + "\n\nRequest more tools or submit the final patch JSON only."
                ),
            }
        )

    if final_response is None:
        messages.append(
            {
                "role": "user",
                "content": 'Tool budget is exhausted. Submit exactly {"patch":"<unified git diff>","notes":"<short explanation>"} now.',
            }
        )
        final_response = ollama_chat(ollama_url=ollama_url, model=model, messages=messages, timeout=timeout)

    transcript["status"] = "ok"
    transcript["rounds_total"] = len(transcript["rounds"])
    transcript["final_response_sha256"] = final_response["content_sha256"]
    transcript["payload_sha256"] = sha256_text(json_compact(transcript))
    write_json(transcript_path, transcript)
    final_response["tool_transcript"] = {
        "path": str(transcript_path),
        "sha256": sha256_file(transcript_path),
        "source_of_truth": file_sot(transcript_path),
        "rounds_total": transcript["rounds_total"],
        "tool_surface": [tool["name"] for tool in agent_tool_surface()],
    }
    return final_response


def ensure_workspace(task: dict[str, Any], workspaces: Path, *, timeout: float) -> Path:
    repo_dir = workspaces / task["instance_id"] / "repo"
    marker = repo_dir / ".contextgraph-swe-lite-workspace.json"
    if marker.exists():
        marker_payload = json.loads(marker.read_text(encoding="utf-8"))
        if marker_payload.get("base_commit") == task["base_commit"] and (repo_dir / ".git").exists():
            return repo_dir
    if repo_dir.exists():
        shutil.rmtree(repo_dir)
    repo_dir.parent.mkdir(parents=True, exist_ok=True)
    clone = run_cmd(
        ["git", "clone", "--filter=blob:none", f"https://github.com/{task['repo']}.git", str(repo_dir)],
        cwd=repo_dir.parent,
        timeout=timeout,
    )
    if clone.exit_code != 0:
        raise RefinementError(
            "SWE_REFINEMENT_GIT_CLONE_FAILED",
            "failed to clone SWE-bench repository",
            "git.clone",
            "inspect network access, disk space, and repository availability",
            details={"clone": clone.to_json()},
            source_of_truth=file_sot(repo_dir),
        )
    checkout = run_cmd(["git", "checkout", task["base_commit"]], cwd=repo_dir, timeout=timeout)
    if checkout.exit_code != 0:
        raise RefinementError(
            "SWE_REFINEMENT_GIT_CHECKOUT_FAILED",
            "failed to checkout SWE-bench base commit",
            "git.checkout",
            "inspect repository history and base_commit",
            details={"checkout": checkout.to_json()},
            source_of_truth=git_sot(repo_dir, task["base_commit"]),
        )
    write_json(
        marker,
        {
            "dataset": DATASET_NAME,
            "instance_id": task["instance_id"],
            "repo": task["repo"],
            "base_commit": task["base_commit"],
            "created_at_unix": time.time(),
        },
    )
    return repo_dir


def reset_workspace(repo_dir: Path, base_commit: str, *, timeout: float) -> list[dict[str, Any]]:
    commands = [
        ["git", "reset", "--hard", base_commit],
        ["git", "clean", "-fdx"],
    ]
    results = []
    for command in commands:
        result = run_cmd(command, cwd=repo_dir, timeout=timeout)
        results.append(result.to_json())
        if result.exit_code != 0:
            raise RefinementError(
                "SWE_REFINEMENT_WORKSPACE_RESET_FAILED",
                "failed to reset repository workspace to base commit",
                "git.reset",
                "inspect workspace permissions and git state",
                details={"command": result.to_json()},
                source_of_truth=git_sot(repo_dir, base_commit),
            )
    return results


def git_changed_files(repo_dir: Path) -> list[str]:
    result = run_cmd(["git", "diff", "--name-only"], cwd=repo_dir, timeout=60)
    if result.exit_code != 0:
        return []
    return [line for line in result.stdout.splitlines() if line.strip()]


def write_validation_status(
    path: Path,
    *,
    status: str,
    error_code: str | None,
    field_path: str,
    message: str,
    remediation: str,
    details: dict[str, Any],
) -> dict[str, Any]:
    payload = {
        "status": status,
        "error_code": error_code,
        "field_path": field_path,
        "message": message,
        "remediation": remediation,
        "details": details,
        "source_of_truth": file_sot(path),
        "created_at_unix": time.time(),
    }
    write_json(path, payload)
    payload["sha256"] = sha256_file(path)
    return payload


def validate_attempt(
    task: dict[str, Any],
    repo_dir: Path,
    attempt_dir: Path,
    patch: str | None,
    parse_status: dict[str, Any],
    *,
    pytest_timeout: float,
    git_timeout: float,
) -> dict[str, Any]:
    validation_path = attempt_dir / "validation.json"
    if not patch:
        status = "blocked" if str(parse_status.get("error_code", "")).startswith("SWE_REFINEMENT_OLLAMA_") else "failed"
        return write_validation_status(
            validation_path,
            status=status,
            error_code=parse_status.get("error_code", "SWE_MODEL_PATCH_MISSING"),
            field_path="model_response.patch",
            message=parse_status.get("message", "model did not produce a usable patch"),
            remediation=parse_status.get(
                "remediation",
                "reprompt with the parse error and require one JSON object containing a unified diff patch",
            ),
            details={"parse_status": parse_status},
        )

    patch_path = attempt_dir / "candidate.patch"
    write_text_checked(patch_path, patch)
    reset = reset_workspace(repo_dir, task["base_commit"], timeout=git_timeout)

    apply_candidate = run_cmd(["git", "apply", "--whitespace=nowarn", str(patch_path)], cwd=repo_dir, timeout=git_timeout)
    if apply_candidate.exit_code != 0:
        return write_validation_status(
            validation_path,
            status="failed",
            error_code="SWE_PATCH_APPLY_FAILED",
            field_path="candidate.patch",
            message="candidate patch did not apply to the base repository",
            remediation="use the git apply stderr to produce a patch against the exact base commit",
            details={
                "patch_sha256": sha256_file(patch_path),
                "candidate_patch_preview": truncate(patch, 4000),
                "workspace_reset": reset,
                "git_apply": apply_candidate.to_json(),
            },
        )

    diff = run_cmd(["git", "diff", "--no-ext-diff", "--binary"], cwd=repo_dir, timeout=git_timeout)
    actual_patch_path = attempt_dir / "actual-applied.patch"
    write_text_checked(actual_patch_path, diff.stdout)
    changed_files = git_changed_files(repo_dir)

    test_patch_path = attempt_dir / "official-test.patch"
    write_text_checked(test_patch_path, task.get("test_patch") or "")
    apply_tests = run_cmd(["git", "apply", "--whitespace=nowarn", str(test_patch_path)], cwd=repo_dir, timeout=git_timeout)
    if apply_tests.exit_code != 0:
        return write_validation_status(
            validation_path,
            status="blocked",
            error_code="SWE_TEST_PATCH_APPLY_FAILED",
            field_path="task.test_patch",
            message="official SWE-bench test patch did not apply after candidate patch",
            remediation="verify the selected task/base commit and run with the official SWE-bench harness if local git application differs",
            details={
                "patch_sha256": sha256_file(patch_path),
                "candidate_patch_preview": truncate(patch, 4000),
                "actual_patch_sha256": sha256_file(actual_patch_path),
                "actual_patch_preview": truncate(diff.stdout, 4000),
                "changed_files": changed_files,
                "test_patch_apply": apply_tests.to_json(),
            },
        )

    tests = list(task.get("FAIL_TO_PASS") or [])
    if not tests:
        return write_validation_status(
            validation_path,
            status="blocked",
            error_code="SWE_FAIL_TO_PASS_EMPTY",
            field_path="task.FAIL_TO_PASS",
            message="SWE-bench task does not declare FAIL_TO_PASS tests",
            remediation="choose a task with explicit FAIL_TO_PASS tests",
            details={"changed_files": changed_files},
        )

    test_command = [sys.executable, "-m", "pytest", "-q", *tests]
    test_result = run_cmd(test_command, cwd=repo_dir, timeout=pytest_timeout)
    stderr_combined = f"{test_result.stdout}\n{test_result.stderr}"
    if "No module named pytest" in stderr_combined:
        status = "blocked"
        error_code = "SWE_PYTEST_UNAVAILABLE"
        message = "local Python environment cannot run SWE-bench tests because pytest is unavailable"
        remediation = "install the official SWE-bench environment or provide a task-specific test environment"
    elif test_result.exit_code == 0:
        status = "passed"
        error_code = None
        message = "candidate patch passed the FAIL_TO_PASS tests in the local validation environment"
        remediation = "record as solved and evaluate on the official SWE-bench harness before claiming leaderboard score"
    else:
        status = "failed"
        error_code = "SWE_FAIL_TO_PASS_TESTS_FAILED"
        message = "candidate patch applied but did not pass FAIL_TO_PASS tests"
        remediation = "use stdout/stderr failure evidence to generate the next patch attempt"

    return write_validation_status(
        validation_path,
        status=status,
        error_code=error_code,
        field_path="pytest.FAIL_TO_PASS",
        message=message,
        remediation=remediation,
        details={
            "patch_sha256": sha256_file(patch_path),
            "candidate_patch_preview": truncate(patch, 4000),
            "actual_patch_sha256": sha256_file(actual_patch_path),
            "actual_patch_preview": truncate(diff.stdout, 4000),
            "changed_files": changed_files,
            "test_command": test_result.to_json(),
            "fail_to_pass": tests,
        },
    )


def write_attempt_verifier(run_root: Path) -> Path:
    verifier = run_root / "verify_attempt_status.py"
    write_text_checked(
        verifier,
        textwrap.dedent(
            """\
            #!/usr/bin/env python3
            import json
            import sys
            from pathlib import Path

            path = Path(sys.argv[1])
            payload = json.loads(path.read_text(encoding="utf-8"))
            print(json.dumps({
                "status": payload.get("status"),
                "error_code": payload.get("error_code"),
                "message": payload.get("message"),
                "source_of_truth": f"file:{path}",
            }, sort_keys=True))
            raise SystemExit(0 if payload.get("status") == "passed" else 42)
            """
        ),
    )
    verifier.chmod(0o755)
    return verifier


def run_blackbox(bin_path: Path, args: list[str], *, cwd: Path, allow_error: bool = False) -> dict[str, Any]:
    result = run_cmd([str(bin_path), *args], cwd=cwd, timeout=180)
    if result.exit_code != 0:
        if allow_error:
            raw = result.stdout.strip() or result.stderr.strip()
            try:
                payload = json.loads(raw)
            except json.JSONDecodeError:
                payload = {
                    "status": "error",
                    "error_code": "BLACKBOX_NON_JSON_ERROR",
                    "message": "context-graph blackbox command failed without JSON output",
                    "process": result.to_json(),
                }
            payload["blackbox_process"] = result.to_json()
            return payload
        raise RefinementError(
            "SWE_REFINEMENT_BLACKBOX_COMMAND_FAILED",
            "context-graph blackbox command failed",
            "blackbox.command",
            "inspect stdout/stderr and ledger path",
            details={"process": result.to_json()},
        )
    return json.loads(result.stdout)


def init_blackbox(bin_path: Path, ledger: Path, cwd: Path) -> None:
    run_blackbox(bin_path, ["blackbox", "init", "--ledger", str(ledger)], cwd=cwd)


def record_run(bin_path: Path, ledger: Path, task: dict[str, Any], model: str, cwd: Path) -> str:
    output = run_blackbox(
        bin_path,
        [
            "blackbox",
            "record",
            "run",
            "--ledger",
            str(ledger),
            "--task",
            f"SWE-bench Lite refinement {task['instance_id']}",
            "--agent",
            "contextgraph_swe_lite_refiner",
            "--model",
            model,
            "--task-kind",
            "swe_bench_lite_refinement_loop",
            "--risk-tier",
            "high",
        ],
        cwd=cwd,
    )
    return str(output["run_id"])


def record_reality_run(
    bin_path: Path,
    ledger: Path,
    *,
    loop_id: str,
    models: list[str],
    tasks: list[dict[str, Any]],
    cwd: Path,
) -> str:
    output = run_blackbox(
        bin_path,
        [
            "blackbox",
            "record",
            "run",
            "--ledger",
            str(ledger),
            "--task",
            f"Reality loop builder for {len(models)} model(s) and {len(tasks)} SWE-bench task(s): {loop_id}",
            "--agent",
            "contextgraph_reality_loop_builder",
            "--model",
            "contextgraph_system",
            "--task-kind",
            "contextgraph_reality_loop_builder",
            "--risk-tier",
            "high",
        ],
        cwd=cwd,
    )
    return str(output["run_id"])


def record_model_decision(
    bin_path: Path,
    ledger: Path,
    run_id: str,
    model: str,
    decision_input: Path,
    cwd: Path,
) -> dict[str, Any]:
    return run_blackbox(
        bin_path,
        [
            "blackbox",
            "record",
            "model-decision",
            "--ledger",
            str(ledger),
            "--run-id",
            run_id,
            "--tool-name",
            "generate_swe_patch",
            "--capability",
            "contextgraph.swe_lite.generate_patch",
            "--input",
            str(decision_input),
            "--agent",
            "contextgraph_swe_lite_refiner",
            "--model",
            model,
        ],
        cwd=cwd,
    )


def record_tool_call(
    bin_path: Path,
    ledger: Path,
    run_id: str,
    model: str,
    tool_input: Path,
    status: str,
    cwd: Path,
) -> dict[str, Any]:
    return run_blackbox(
        bin_path,
        [
            "blackbox",
            "record",
            "tool-call",
            "--ledger",
            str(ledger),
            "--run-id",
            run_id,
            "--tool",
            "apply_and_test_swe_patch",
            "--capability",
            "contextgraph.swe_lite.apply_and_test",
            "--input",
            str(tool_input),
            "--status",
            status,
            "--agent",
            "contextgraph_swe_lite_refiner",
            "--model",
            model,
        ],
        cwd=cwd,
    )


def verify_attempt_with_blackbox(
    bin_path: Path,
    ledger: Path,
    run_id: str,
    tool_call_id: str,
    run_root: Path,
    attempt_dir: Path,
    cwd: Path,
) -> dict[str, Any]:
    validation_rel = attempt_dir.relative_to(run_root) / "validation.json"
    verifier_rel = Path("verify_attempt_status.py")
    artifacts = [
        str(validation_rel),
        str(attempt_dir.relative_to(run_root) / "prompt.txt"),
        str(attempt_dir.relative_to(run_root) / "repository-context.json"),
        str(attempt_dir.relative_to(run_root) / "model-response.txt"),
    ]
    candidate = attempt_dir.relative_to(run_root) / "candidate.patch"
    if (run_root / candidate).exists():
        artifacts.append(str(candidate))
    spec = {
        "verifier_id": "cmd_exit_zero_v1",
        "run_id": run_id,
        "tool_call_id": tool_call_id,
        "source": {"cwd": str(run_root)},
        "expected": {
            "command": [sys.executable, str(verifier_rel), str(validation_rel)],
            "artifacts": artifacts,
        },
    }
    spec_path = attempt_dir / "blackbox-verify-spec.json"
    write_json(spec_path, spec)
    return run_blackbox(
        bin_path,
        ["blackbox", "verify", "--ledger", str(ledger), "--input", str(spec_path), "--model", "contextgraph_swe_lite_refiner"],
        cwd=cwd,
        allow_error=True,
    )


def ledger_counts(ledger: Path) -> dict[str, int]:
    conn = sqlite3.connect(ledger)
    try:
        return {
            str(row[0]): int(row[1])
            for row in conn.execute("SELECT record_kind, COUNT(*) FROM ledger_records GROUP BY record_kind ORDER BY record_kind")
        }
    finally:
        conn.close()


def score_and_reward(bin_path: Path, ledger: Path, run_id: str, model: str, cwd: Path) -> dict[str, Any]:
    score = run_blackbox(
        bin_path,
        ["blackbox", "score", "run", "--ledger", str(ledger), "--run-id", run_id, "--model", model],
        cwd=cwd,
    )
    reward_spec = REPO_ROOT / "gemmafrontend" / "reward_spec.verified_outcome.json"
    reward = run_blackbox(
        bin_path,
        ["blackbox", "reward", "compile", "--ledger", str(ledger), "--run-id", run_id, "--input", str(reward_spec), "--model", model],
        cwd=cwd,
    )
    return {"score": score, "reward": reward}


def attempt_status_for_tool(validation: dict[str, Any]) -> str:
    if validation.get("status") == "passed":
        return "ok"
    if validation.get("status") == "blocked":
        return "blocked"
    return "failed"


def safe_id(value: str) -> str:
    return re.sub(r"[^A-Za-z0-9_.-]+", "-", value).strip("-")[:160]


def root_reality_branch_id(loop_id: str) -> str:
    return safe_id(f"{loop_id}:root")


def task_reality_branch_id(loop_id: str, model: str, instance_id: str) -> str:
    return safe_id(f"{loop_id}:{model}:{instance_id}:task-root")


def attempt_reality_branch_id(loop_id: str, model: str, instance_id: str, attempt: int) -> str:
    return safe_id(f"{loop_id}:{model}:{instance_id}:attempt-{attempt:03d}")


def prompt_candidate_branch_id(loop_id: str, model: str, instance_id: str, attempt: int) -> str:
    return safe_id(f"{loop_id}:{model}:{instance_id}:prompt-candidate-{attempt:03d}")


def reward_payload(score_reward: dict[str, Any] | None) -> dict[str, Any] | None:
    if not score_reward:
        return None
    reward = score_reward.get("reward")
    if not isinstance(reward, dict):
        return None
    value = reward.get("reward")
    return value if isinstance(value, dict) else None


def score_payload(score_reward: dict[str, Any] | None) -> dict[str, Any] | None:
    if not score_reward:
        return None
    score = score_reward.get("score")
    if not isinstance(score, dict):
        return None
    value = score.get("score")
    return value if isinstance(value, dict) else None


def extract_test_output_excerpt(path: Path | None) -> str | None:
    if path is None or not path.exists():
        return None
    text = path.read_text(encoding="utf-8", errors="replace")
    interesting = []
    for line in text.splitlines():
        if re.search(r"\b(FAILED|ERROR|Traceback|AssertionError|assert |E\s+|FAILURES)\b", line):
            interesting.append(line)
    tail = "\n".join(text.splitlines()[-80:])
    parts = []
    if interesting:
        parts.append("\n".join(interesting[-80:]))
    parts.append(tail)
    return truncate("\n\n".join(parts), 6000)


def instance_payload(instance_report: dict[str, Any], instance_id: str) -> dict[str, Any]:
    value = instance_report.get(instance_id)
    if isinstance(value, dict):
        return value
    return instance_report


def tests_status_list(payload: dict[str, Any], group: str, status: str) -> list[str]:
    tests_status = payload.get("tests_status")
    if not isinstance(tests_status, dict):
        return []
    group_value = tests_status.get(group)
    if not isinstance(group_value, dict):
        return []
    values = group_value.get(status) or []
    return [str(item) for item in values]


def write_task_loop_summary(
    task_root: Path,
    *,
    status: str,
    terminal_reason: str,
    model: str,
    task: dict[str, Any],
    attempts: list[dict[str, Any]],
    ledger: Path,
    blackbox_run_id: str,
) -> None:
    summary = {
        "status": status,
        "terminal_reason": terminal_reason,
        "model": model,
        "task": {
            "instance_id": task["instance_id"],
            "repo": task["repo"],
            "base_commit": task["base_commit"],
        },
        "attempts_total": len(attempts),
        "attempts": attempts,
        "ledger": {
            "path": str(ledger),
            "sha256": sha256_file(ledger) if ledger.exists() else None,
            "source_of_truth": sqlite_sot(ledger),
            "physical_counts": ledger_counts(ledger) if ledger.exists() else {},
        },
        "blackbox_run_id": blackbox_run_id,
        "source_of_truth": {
            "task_manifest": file_sot(task_root / "task-manifest.json"),
            "ledger": sqlite_sot(ledger),
            "loop_summary": file_sot(task_root / "loop-summary.json"),
        },
    }
    write_json(task_root / "loop-summary.json", summary)


def json_compact(value: dict[str, Any]) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"))


def read_jsonl(path: Path) -> list[dict[str, Any]]:
    if not path.exists():
        return []
    rows = []
    for line_no, line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
        if not line.strip():
            continue
        try:
            rows.append(json.loads(line))
        except json.JSONDecodeError as err:
            raise RefinementError(
                "SWE_REFINEMENT_REALITY_EVENT_JSON_INVALID",
                "reality event stream contained invalid JSON",
                f"reality_events.line_{line_no}",
                "stop the loop and inspect the event stream before continuing optimization",
                details={"line_preview": line[:500]},
                source_of_truth=file_sot(path),
            ) from err
    return rows


def write_reality_event_state(
    *,
    run_root: Path,
    event_log: Path,
    events: list[dict[str, Any]],
    reality_ledger: Path,
    latest_event: dict[str, Any],
) -> dict[str, Any]:
    event_counts: dict[str, int] = {}
    layer_counts: dict[str, int] = {}
    for event in events:
        event_type = str(event.get("event_type"))
        loop_layer = str(event.get("loop_layer"))
        event_counts[event_type] = event_counts.get(event_type, 0) + 1
        layer_counts[loop_layer] = layer_counts.get(loop_layer, 0) + 1
    state = {
        "status": "ok",
        "events_total": len(events),
        "event_counts": event_counts,
        "layer_counts": layer_counts,
        "latest_event": latest_event,
        "event_log": {
            "path": str(event_log),
            "sha256": sha256_file(event_log),
            "source_of_truth": file_sot(event_log),
        },
        "reality_ledger": {
            "path": str(reality_ledger),
            "sha256": sha256_file(reality_ledger) if reality_ledger.exists() else None,
            "physical_counts": ledger_counts(reality_ledger) if reality_ledger.exists() else {},
            "source_of_truth": sqlite_sot(reality_ledger),
        },
        "source_of_truth": file_sot(run_root / "reality-state.json"),
    }
    write_json(run_root / "reality-state.json", state)
    return state


def append_reality_event(
    *,
    run_root: Path,
    loop_id: str,
    context_graph_bin: Path,
    reality_ledger: Path,
    reality_run_id: str,
    loop_layer: str,
    event_type: str,
    status: str,
    payload: dict[str, Any],
    source_of_truth_refs: dict[str, Any],
    model: str | None = None,
    instance_id: str | None = None,
    attempt: int | None = None,
    reality_branch_id: str | None = None,
    parent_reality_branch_id: str | None = None,
    promotion_status: str = "candidate",
) -> dict[str, Any]:
    event_log = run_root / "reality-events.jsonl"
    prior_events = read_jsonl(event_log)
    sequence = len(prior_events) + 1
    payload_sha256 = sha256_text(json_compact(payload))
    event = {
        "schema_version": 1,
        "sequence": sequence,
        "created_at_unix": time.time(),
        "loop_id": loop_id,
        "loop_layer": loop_layer,
        "event_type": event_type,
        "status": status,
        "model": model,
        "instance_id": instance_id,
        "attempt": attempt,
        "reality_branch_id": reality_branch_id,
        "parent_reality_branch_id": parent_reality_branch_id,
        "promotion_status": promotion_status,
        "payload": payload,
        "payload_sha256": payload_sha256,
        "source_of_truth_refs": source_of_truth_refs,
        "source_of_truth": file_sot(event_log),
    }
    event_log.parent.mkdir(parents=True, exist_ok=True)
    with event_log.open("a", encoding="utf-8") as handle:
        handle.write(json_compact(event) + "\n")
    readback_events = read_jsonl(event_log)
    if not readback_events or readback_events[-1] != event:
        raise RefinementError(
            "SWE_REFINEMENT_REALITY_EVENT_READBACK_MISMATCH",
            "latest reality event readback did not match the event just appended",
            "reality_events.latest",
            "inspect event stream durability before continuing optimization",
            details={"expected": event, "actual": readback_events[-1] if readback_events else None},
            source_of_truth=file_sot(event_log),
        )
    summary_parts = [loop_layer, event_type, status]
    if model:
        summary_parts.append(model)
    if instance_id:
        summary_parts.append(instance_id)
    if attempt is not None:
        summary_parts.append(f"attempt={attempt}")
    run_blackbox(
        context_graph_bin,
        [
            "blackbox",
            "record",
            "step",
            "--ledger",
            str(reality_ledger),
            "--run-id",
            reality_run_id,
            "--step-kind",
            event_type,
            "--summary",
            " | ".join(summary_parts),
            "--agent",
            "contextgraph_reality_loop_builder",
            "--model",
            model or "contextgraph_system",
        ],
        cwd=run_root,
    )
    state = write_reality_event_state(
        run_root=run_root,
        event_log=event_log,
        events=read_jsonl(event_log),
        reality_ledger=reality_ledger,
        latest_event=event,
    )
    event["state_after_event"] = {
        "path": str(run_root / "reality-state.json"),
        "sha256": sha256_file(run_root / "reality-state.json"),
        "events_total": state["events_total"],
        "reality_ledger_counts": state["reality_ledger"]["physical_counts"],
        "source_of_truth": file_sot(run_root / "reality-state.json"),
    }
    return event


def write_attempt_prediction(
    *,
    attempt_dir: Path,
    task: dict[str, Any],
    model: str,
    patch: str,
) -> Path:
    prediction = {
        "instance_id": task["instance_id"],
        "model_name_or_path": model,
        "model_patch": patch,
    }
    path = attempt_dir / "official-prediction.jsonl"
    write_text_checked(path, json.dumps(prediction, sort_keys=True) + "\n")
    return path


def parse_summary_path(stdout: str) -> Path | None:
    for line in stdout.splitlines():
        if line.startswith("SWE_OFFICIAL_EVAL_SUMMARY="):
            return Path(line.split("=", 1)[1].strip())
    return None


def parse_error_path(stdout: str) -> Path | None:
    for line in stdout.splitlines():
        if line.startswith("SWE_OFFICIAL_EVAL_ERROR="):
            return Path(line.split("=", 1)[1].strip())
    return None


def run_official_eval_for_attempt(
    *,
    run_root: Path,
    attempt_dir: Path,
    task: dict[str, Any],
    model: str,
    patch: str,
    context_graph_bin: Path,
    official_eval_script: Path,
    swebench_python: Path,
    docker_cli_dir: Path,
    docker_socket: Path,
    docker_windows_bin: Path,
    max_workers: int,
    timeout: int,
    cache_level: str,
) -> dict[str, Any]:
    official_eval_script = resolve_repo_path(official_eval_script)
    if not official_eval_script.exists():
        raise RefinementError(
            "SWE_REFINEMENT_OFFICIAL_EVAL_SCRIPT_MISSING",
            "official SWE-bench eval bridge script does not exist",
            "official_eval_script",
            "restore gemmafrontend/swe_lite_official_eval.py before enabling official per-attempt evaluation",
            source_of_truth=file_sot(official_eval_script),
        )
    predictions_path = write_attempt_prediction(attempt_dir=attempt_dir, task=task, model=model, patch=patch)
    run_id = safe_id(f"{run_root.name}_{model}_{task['instance_id']}_attempt_{attempt_dir.name[-3:]}")
    command = [
        sys.executable,
        str(official_eval_script),
        "--run-root",
        str(run_root),
        "--predictions-path",
        str(predictions_path),
        "--swebench-python",
        str(swebench_python),
        "--context-graph-bin",
        str(context_graph_bin),
        "--docker-cli-dir",
        str(docker_cli_dir),
        "--docker-socket",
        str(docker_socket),
        "--docker-windows-bin",
        str(docker_windows_bin),
        "--instance-id",
        task["instance_id"],
        "--run-id",
        run_id,
        "--max-workers",
        str(max_workers),
        "--timeout",
        str(timeout),
        "--cache-level",
        cache_level,
    ]
    result = run_cmd(command, cwd=REPO_ROOT, timeout=max(float(timeout) + 2400.0, 3600.0))
    command_path = attempt_dir / "official-eval-command.json"
    write_json(
        command_path,
        {
            "status": "ok" if result.exit_code == 0 else "error",
            "process": result.to_json(),
            "predictions_path": str(predictions_path),
            "predictions_sha256": sha256_file(predictions_path),
            "source_of_truth": file_sot(command_path),
        },
    )
    if result.exit_code != 0:
        error_path = parse_error_path(result.stdout)
        error_payload = None
        if error_path and error_path.exists():
            error_payload = json.loads(error_path.read_text(encoding="utf-8"))
        raise RefinementError(
            "SWE_REFINEMENT_OFFICIAL_EVAL_FAILED",
            "official SWE-bench eval bridge failed during per-attempt evaluation",
            "official_eval_each_attempt",
            "inspect official-eval-command.json and the typed official-eval-error.json before retrying",
            details={"process": result.to_json(), "official_error": error_payload},
            source_of_truth=file_sot(command_path),
        )
    summary_path = parse_summary_path(result.stdout)
    if summary_path is None or not summary_path.exists():
        raise RefinementError(
            "SWE_REFINEMENT_OFFICIAL_SUMMARY_MISSING",
            "official eval bridge completed but did not print a readable summary path",
            "official_eval.stdout",
            "inspect official-eval-command.json and the official eval output directory",
            details={"process": result.to_json()},
            source_of_truth=file_sot(command_path),
        )
    summary = json.loads(summary_path.read_text(encoding="utf-8"))
    summary["summary_path"] = str(summary_path)
    summary["summary_sha256"] = sha256_file(summary_path)
    return summary


def build_official_evidence(
    *,
    attempt_dir: Path,
    task: dict[str, Any],
    model: str,
    patch: str,
    official_summary: dict[str, Any],
) -> dict[str, Any]:
    instance_id = task["instance_id"]
    ledger_ingests = [
        item
        for item in official_summary.get("ledger_ingests", [])
        if item.get("instance_id") == instance_id and item.get("model") == model
    ]
    if len(ledger_ingests) != 1:
        raise RefinementError(
            "SWE_REFINEMENT_OFFICIAL_LEDGER_INGEST_NOT_UNIQUE",
            "official eval summary did not contain exactly one ledger ingest for this model/task",
            "official_summary.ledger_ingests",
            "inspect the official eval summary before using it as prompt feedback",
            details={"model": model, "instance_id": instance_id, "ledger_ingests": official_summary.get("ledger_ingests")},
            source_of_truth=file_sot(Path(str(official_summary.get("summary_path", "")))),
        )
    ingest = ledger_ingests[0]
    readback_path = Path(str(ingest["readback_path"]))
    readback = json.loads(readback_path.read_text(encoding="utf-8"))
    final_report_path = Path(str(official_summary["official_report"]["path"]))
    final_report = json.loads(final_report_path.read_text(encoding="utf-8"))
    logs = readback.get("logs")
    if not isinstance(logs, dict):
        raise RefinementError(
            "SWE_REFINEMENT_OFFICIAL_READBACK_LOGS_INVALID",
            "official eval readback did not contain a logs object",
            "official_readback.logs",
            "inspect the official readback before using this attempt as feedback",
            details={"readback_path": str(readback_path), "logs": logs},
            source_of_truth=file_sot(readback_path),
        )
    instance_report_entry = logs.get("instance_report")
    test_output_entry = logs.get("test_output")
    run_instance_log_entry = logs.get("run_instance_log")
    patch_log_entry = logs.get("patch")
    instance_report_path = (
        Path(str(instance_report_entry.get("path")))
        if isinstance(instance_report_entry, dict) and instance_report_entry.get("path")
        else None
    )
    test_output_path = (
        Path(str(test_output_entry.get("path")))
        if isinstance(test_output_entry, dict) and test_output_entry.get("path")
        else None
    )
    run_instance_log_path = (
        Path(str(run_instance_log_entry.get("path")))
        if isinstance(run_instance_log_entry, dict) and run_instance_log_entry.get("path")
        else None
    )
    patch_log_path = (
        Path(str(patch_log_entry.get("path")))
        if isinstance(patch_log_entry, dict) and patch_log_entry.get("path")
        else None
    )
    missing_artifacts = []
    if instance_report_path is None or not instance_report_path.exists():
        missing_artifacts.append("instance_report")
        instance_report = {}
        instance = {}
    else:
        instance_report = json.loads(instance_report_path.read_text(encoding="utf-8"))
        instance = instance_payload(instance_report, instance_id)
    if test_output_path is None or not test_output_path.exists():
        missing_artifacts.append("test_output")
    log_text = (
        run_instance_log_path.read_text(encoding="utf-8", errors="replace")
        if run_instance_log_path and run_instance_log_path.exists()
        else ""
    )
    official_error_code = None
    if missing_artifacts:
        official_error_code = "SWE_OFFICIAL_INSTANCE_REPORT_MISSING" if "instance_report" in missing_artifacts else "SWE_OFFICIAL_TEST_OUTPUT_MISSING"
    if "Patch Apply Failed" in log_text:
        official_error_code = "SWE_OFFICIAL_PATCH_APPLY_FAILED"
    official_status = "passed" if instance_id in set(final_report.get("resolved_ids") or []) else "failed"
    evidence = {
        "evidence_kind": "official_swe_result_for_reprompt",
        "attempt": int(attempt_dir.name.rsplit("-", 1)[-1]),
        "model": model,
        "instance_id": instance_id,
        "official_status": official_status,
        "official_error_code": official_error_code,
        "missing_artifacts": missing_artifacts,
        "resolved": instance_id in set(final_report.get("resolved_ids") or []),
        "unresolved": instance_id in set(final_report.get("unresolved_ids") or []),
        "errored": instance_id in set(final_report.get("error_ids") or []),
        "patch_exists": bool(instance.get("patch_exists")) if instance else bool(patch_log_path and patch_log_path.exists()),
        "patch_successfully_applied": bool(instance.get("patch_successfully_applied")) if instance else False,
        "failed_fail_to_pass": tests_status_list(instance, "FAIL_TO_PASS", "failure"),
        "passed_fail_to_pass": tests_status_list(instance, "FAIL_TO_PASS", "success"),
        "failed_pass_to_pass": tests_status_list(instance, "PASS_TO_PASS", "failure"),
        "passed_pass_to_pass_count": len(tests_status_list(instance, "PASS_TO_PASS", "success")),
        "previous_patch_sha256": sha256_text(patch),
        "previous_patch_preview": truncate(patch, 4000),
        "test_output_excerpt": extract_test_output_excerpt(test_output_path) or truncate(log_text, 6000) or None,
        "score": score_payload(ingest.get("score_reward")),
        "reward": reward_payload(ingest.get("score_reward")),
        "source_of_truth": {
            "official_summary": file_sot(Path(str(official_summary["summary_path"]))),
            "official_report": file_sot(final_report_path),
            "official_readback": file_sot(readback_path),
            "instance_report": file_sot(instance_report_path) if instance_report_path else None,
            "test_output": file_sot(test_output_path) if test_output_path else None,
            "run_instance_log": file_sot(run_instance_log_path) if run_instance_log_path else None,
            "patch_log": file_sot(patch_log_path) if patch_log_path else None,
            "ledger": ingest.get("ledger", {}).get("source_of_truth"),
        },
        "sha256": {
            "official_summary": official_summary.get("summary_sha256"),
            "official_report": sha256_file(final_report_path),
            "official_readback": sha256_file(readback_path),
            "instance_report": sha256_file(instance_report_path) if instance_report_path and instance_report_path.exists() else None,
            "test_output": sha256_file(test_output_path) if test_output_path and test_output_path.exists() else None,
            "run_instance_log": sha256_file(run_instance_log_path) if run_instance_log_path and run_instance_log_path.exists() else None,
            "patch_log": sha256_file(patch_log_path) if patch_log_path and patch_log_path.exists() else None,
        },
    }
    evidence["payload_sha256"] = sha256_text(json_compact(evidence))
    path = attempt_dir / ("official-success-evidence.json" if evidence["resolved"] else "official-failure-evidence.json")
    write_json(path, evidence)
    evidence["evidence_path"] = str(path)
    evidence["evidence_sha256"] = sha256_file(path)
    return evidence


def run_model_task_loop(
    *,
    task: dict[str, Any],
    model: str,
    run_root: Path,
    workspaces: Path,
    context_graph_bin: Path,
    ollama_url: str,
    max_attempts: int,
    include_hints: bool,
    model_timeout: float,
    git_timeout: float,
    pytest_timeout: float,
    max_context_files: int,
    context_window_radius: int,
    official_eval_each_attempt: bool,
    official_eval_script: Path,
    swebench_python: Path,
    docker_cli_dir: Path,
    docker_socket: Path,
    docker_windows_bin: Path,
    official_max_workers: int,
    official_timeout: int,
    official_cache_level: str,
    agent_tool_rounds: int,
    loop_id: str,
    reality_ledger: Path,
    reality_run_id: str,
) -> dict[str, Any]:
    task_branch_id = task_reality_branch_id(loop_id, model, task["instance_id"])
    task_parent_branch_id = root_reality_branch_id(loop_id)
    parent_branch_id = task_branch_id
    safe_model = model.replace(":", "-").replace("/", "-")
    task_root = run_root / "tasks" / task["instance_id"] / safe_model
    task_root.mkdir(parents=True, exist_ok=True)
    ledger = task_root / "blackbox.sqlite"
    init_blackbox(context_graph_bin, ledger, task_root)
    blackbox_run_id = record_run(context_graph_bin, ledger, task, model, task_root)
    append_reality_event(
        run_root=run_root,
        loop_id=loop_id,
        context_graph_bin=context_graph_bin,
        reality_ledger=reality_ledger,
        reality_run_id=reality_run_id,
        loop_layer="inner_swe_repair_loop",
        event_type="task_loop_started",
        status="running",
        model=model,
        instance_id=task["instance_id"],
        attempt=None,
        reality_branch_id=task_branch_id,
        parent_reality_branch_id=task_parent_branch_id,
        promotion_status="candidate_task",
        payload={
            "blackbox_run_id": blackbox_run_id,
            "task_root": str(task_root),
            "task_ledger": str(ledger),
            "repo": task["repo"],
            "base_commit": task["base_commit"],
        },
        source_of_truth_refs={
            "task_ledger": sqlite_sot(ledger),
            "task_root": file_sot(task_root),
        },
    )
    write_attempt_verifier(task_root)
    task_manifest = {
        "dataset": DATASET_NAME,
        "instance_id": task["instance_id"],
        "repo": task["repo"],
        "base_commit": task["base_commit"],
        "problem_statement": task["problem_statement"],
        "hints_included": include_hints,
        "fail_to_pass": task.get("FAIL_TO_PASS") or [],
        "pass_to_pass_count": len(task.get("PASS_TO_PASS") or []),
        "oracle_patch_sha256": sha256_text(task.get("patch") or ""),
        "official_test_patch_sha256": sha256_text(task.get("test_patch") or ""),
        "source_of_truth": f"huggingface:{DATASET_NAME}:test:{task['instance_id']}",
    }
    write_json(task_root / "task-manifest.json", task_manifest)
    repo_dir = ensure_workspace(task, workspaces, timeout=git_timeout)
    reset_workspace(repo_dir, task["base_commit"], timeout=git_timeout)

    prompt_context = collect_repository_context(
        task,
        repo_dir,
        validation=None,
        max_files=max_context_files,
        window_radius=context_window_radius,
    )
    write_json(task_root / "repository-context-initial.json", prompt_context)
    compiled_prompt = compile_reality_prompt(
        prompt_kind="initial",
        task=task,
        repo_context=prompt_context,
        include_hints=include_hints,
        previous_patch_hashes=[],
    )
    prompt = compiled_prompt["prompt_text"]
    append_reality_event(
        run_root=run_root,
        loop_id=loop_id,
        context_graph_bin=context_graph_bin,
        reality_ledger=reality_ledger,
        reality_run_id=reality_run_id,
        loop_layer="inner_swe_repair_loop",
        event_type="initial_context_materialized",
        status="ok",
        model=model,
        instance_id=task["instance_id"],
        reality_branch_id=task_branch_id,
        parent_reality_branch_id=task_parent_branch_id,
        promotion_status="candidate_task",
        payload={
            "repository_context_path": str(task_root / "repository-context-initial.json"),
            "repository_context_sha256": sha256_file(task_root / "repository-context-initial.json"),
            "context_files": len(prompt_context.get("files", [])),
            "prompt_compiler": prompt_compiler_manifest(compiled_prompt),
        },
        source_of_truth_refs={
            "repository_context": file_sot(task_root / "repository-context-initial.json"),
            "workspace": git_sot(repo_dir, task["base_commit"]),
        },
    )
    attempts = []
    prompt_kind = str(compiled_prompt["prompt_kind"])
    prompt_template_id = str(compiled_prompt["prompt_template_id"])
    included_evidence: list[dict[str, Any]] = list(compiled_prompt["included_evidence"])
    excluded_evidence: list[dict[str, Any]] = list(compiled_prompt["excluded_evidence"])
    prompt_compiler = prompt_compiler_manifest(compiled_prompt)
    previous_patch_hashes: list[str] = []
    solved = False
    terminal_reason = "attempt_budget_exhausted"
    write_task_loop_summary(
        task_root,
        status="running",
        terminal_reason=terminal_reason,
        model=model,
        task=task,
        attempts=attempts,
        ledger=ledger,
        blackbox_run_id=blackbox_run_id,
    )
    for attempt_index in range(1, max_attempts + 1):
        attempt_branch_id = attempt_reality_branch_id(loop_id, model, task["instance_id"], attempt_index)
        attempt_dir = task_root / "attempts" / f"attempt-{attempt_index:03d}"
        attempt_dir.mkdir(parents=True, exist_ok=False)
        append_reality_event(
            run_root=run_root,
            loop_id=loop_id,
            context_graph_bin=context_graph_bin,
            reality_ledger=reality_ledger,
            reality_run_id=reality_run_id,
            loop_layer="inner_swe_repair_loop",
            event_type="attempt_started",
            status="running",
            model=model,
            instance_id=task["instance_id"],
            attempt=attempt_index,
            reality_branch_id=attempt_branch_id,
            parent_reality_branch_id=parent_branch_id,
            promotion_status="candidate_attempt",
            payload={
                "attempt_dir": str(attempt_dir),
                "prompt_kind": prompt_kind,
                "prompt_template_id": prompt_template_id,
                "previous_patch_hashes": previous_patch_hashes,
            },
            source_of_truth_refs={
                "attempt_dir": file_sot(attempt_dir),
                "task_ledger": sqlite_sot(ledger),
            },
        )
        repo_context_path = attempt_dir / "repository-context.json"
        write_json(repo_context_path, prompt_context)
        prompt_path = attempt_dir / "prompt.txt"
        write_text_checked(prompt_path, prompt)
        prompt_manifest = write_prompt_manifest(
            attempt_dir,
            attempt=attempt_index,
            prompt_path=prompt_path,
            prompt_kind=prompt_kind,
            prompt_template_id=prompt_template_id,
            included_evidence=included_evidence,
            excluded_evidence=excluded_evidence,
            no_repeat_patch_sha256=previous_patch_hashes,
            prompt_compiler=prompt_compiler,
        )
        append_reality_event(
            run_root=run_root,
            loop_id=loop_id,
            context_graph_bin=context_graph_bin,
            reality_ledger=reality_ledger,
            reality_run_id=reality_run_id,
            loop_layer="inner_swe_repair_loop",
            event_type="prompt_manifest_persisted",
            status="ok",
            model=model,
            instance_id=task["instance_id"],
            attempt=attempt_index,
            reality_branch_id=attempt_branch_id,
            parent_reality_branch_id=parent_branch_id,
            promotion_status="candidate_prompt",
            payload={
                "prompt_sha256": sha256_file(prompt_path),
                "prompt_manifest_sha256": prompt_manifest["sha256"],
                "prompt_template_id": prompt_template_id,
                "included_evidence_count": len(included_evidence),
                "excluded_evidence_count": len(excluded_evidence),
                "prompt_compiler": prompt_compiler,
            },
            source_of_truth_refs={
                "prompt": file_sot(prompt_path),
                "prompt_manifest": file_sot(attempt_dir / "prompt-manifest.json"),
                "repository_context": file_sot(repo_context_path),
            },
        )
        model_started = time.perf_counter()
        generation_error = None
        try:
            if agent_tool_rounds > 0:
                model_response = run_model_tool_rounds(
                    ollama_url=ollama_url,
                    model=model,
                    prompt=prompt,
                    timeout=model_timeout,
                    repo_dir=repo_dir,
                    repo_context=prompt_context,
                    included_evidence=included_evidence,
                    attempt_dir=attempt_dir,
                    max_rounds=agent_tool_rounds,
                )
            else:
                model_response = ollama_generate_patch(
                    ollama_url=ollama_url,
                    model=model,
                    prompt=prompt,
                    timeout=model_timeout,
                )
            response_text = model_response["content"]
            response_sha256 = model_response["content_sha256"]
            model_metadata = model_response["metadata"]
        except RefinementError as err:
            generation_error = err.to_json()
            response_text = json.dumps(generation_error, indent=2, sort_keys=True)
            response_sha256 = sha256_text(response_text)
            model_metadata = {"model": model, "generation_error": generation_error}
        model_elapsed_ms = int((time.perf_counter() - model_started) * 1000)
        response_path = attempt_dir / "model-response.txt"
        write_text_checked(response_path, response_text)
        if generation_error:
            patch = None
            parse_status = {
                "status": "error",
                "error_code": generation_error["error_code"],
                "message": generation_error["message"],
                "field_path": generation_error["field_path"],
                "remediation": generation_error["remediation"],
                "details": generation_error["details"],
            }
        else:
            patch, parse_status = parse_model_patch(response_text)
        if patch:
            write_text_checked(attempt_dir / "candidate.patch", patch)
        patch_sha256 = sha256_text(patch) if patch else None
        repeated_patch = bool(patch_sha256 and patch_sha256 in set(previous_patch_hashes))
        if patch_sha256:
            previous_patch_hashes.append(patch_sha256)
        append_reality_event(
            run_root=run_root,
            loop_id=loop_id,
            context_graph_bin=context_graph_bin,
            reality_ledger=reality_ledger,
            reality_run_id=reality_run_id,
            loop_layer="inner_swe_repair_loop",
            event_type="model_output_materialized",
            status="blocked" if generation_error else "ok",
            model=model,
            instance_id=task["instance_id"],
            attempt=attempt_index,
            reality_branch_id=attempt_branch_id,
            parent_reality_branch_id=parent_branch_id,
            promotion_status="candidate_output",
            payload={
                "response_sha256": sha256_file(response_path),
                "model_elapsed_ms": model_elapsed_ms,
                "parse_status": parse_status,
                "candidate_patch_sha256": patch_sha256,
                "repeated_patch": repeated_patch,
                "agent_tool_transcript": model_response.get("tool_transcript") if not generation_error else None,
            },
            source_of_truth_refs={
                "model_response": file_sot(response_path),
                "candidate_patch": file_sot(attempt_dir / "candidate.patch") if patch else None,
                "agent_tool_transcript": model_response.get("tool_transcript", {}).get("source_of_truth") if not generation_error and model_response.get("tool_transcript") else None,
            },
        )

        decision_input = {
            "attempt": attempt_index,
            "reality_branch_id": attempt_branch_id,
            "parent_reality_branch_id": parent_branch_id,
            "prompt_sha256": sha256_file(prompt_path),
            "prompt_manifest_sha256": prompt_manifest["sha256"],
            "repository_context_sha256": sha256_file(repo_context_path),
            "repository_context_source_of_truth": prompt_context["source_of_truth"],
            "response_sha256": sha256_file(response_path),
            "parse_status": parse_status,
            "model_metadata": model_metadata,
            "agent_tool_transcript": model_response.get("tool_transcript") if not generation_error else None,
            "candidate_patch_sha256": patch_sha256,
            "repeated_patch": repeated_patch,
            "task": {
                "instance_id": task["instance_id"],
                "repo": task["repo"],
                "base_commit": task["base_commit"],
            },
        }
        decision_input_path = attempt_dir / "model-decision-input.json"
        write_json(decision_input_path, decision_input)
        model_decision = record_model_decision(context_graph_bin, ledger, blackbox_run_id, model, decision_input_path, task_root)

        validation = validate_attempt(
            task,
            repo_dir,
            attempt_dir,
            patch,
            parse_status,
            pytest_timeout=pytest_timeout,
            git_timeout=git_timeout,
        )
        append_reality_event(
            run_root=run_root,
            loop_id=loop_id,
            context_graph_bin=context_graph_bin,
            reality_ledger=reality_ledger,
            reality_run_id=reality_run_id,
            loop_layer="inner_swe_repair_loop",
            event_type="local_validation_readback",
            status=validation["status"],
            model=model,
            instance_id=task["instance_id"],
            attempt=attempt_index,
            reality_branch_id=attempt_branch_id,
            parent_reality_branch_id=parent_branch_id,
            promotion_status="candidate_validated",
            payload={
                "validation_status": validation["status"],
                "error_code": validation.get("error_code"),
                "validation_sha256": sha256_file(attempt_dir / "validation.json"),
                "patch_sha256": patch_sha256,
            },
            source_of_truth_refs={
                "validation": file_sot(attempt_dir / "validation.json"),
                "workspace": git_sot(repo_dir, task["base_commit"]),
            },
        )
        tool_input = {
            "attempt": attempt_index,
            "expected_delta": {
                "repo_patch": "applies",
                "official_test_patch": "applies",
                "fail_to_pass": "passed",
            },
            "candidate_patch_sha256": sha256_file(attempt_dir / "candidate.patch") if (attempt_dir / "candidate.patch").exists() else None,
            "validation_status": validation["status"],
            "validation_source_of_truth": validation["source_of_truth"],
        }
        tool_input_path = attempt_dir / "tool-call-input.json"
        write_json(tool_input_path, tool_input)
        tool_call = record_tool_call(
            context_graph_bin,
            ledger,
            blackbox_run_id,
            model,
            tool_input_path,
            attempt_status_for_tool(validation),
            task_root,
        )
        verifier = verify_attempt_with_blackbox(
            context_graph_bin,
            ledger,
            blackbox_run_id,
            str(tool_call["tool_call_id"]),
            task_root,
            attempt_dir,
            task_root,
        )
        score_reward = score_and_reward(context_graph_bin, ledger, blackbox_run_id, model, task_root)
        append_reality_event(
            run_root=run_root,
            loop_id=loop_id,
            context_graph_bin=context_graph_bin,
            reality_ledger=reality_ledger,
            reality_run_id=reality_run_id,
            loop_layer="blackbox_verification",
            event_type="local_blackbox_verifier_and_reward",
            status="ok",
            model=model,
            instance_id=task["instance_id"],
            attempt=attempt_index,
            reality_branch_id=attempt_branch_id,
            parent_reality_branch_id=parent_branch_id,
            promotion_status="candidate_scored",
            payload={
                "verifier_status": verifier.get("status"),
                "score": score_payload(score_reward),
                "reward": reward_payload(score_reward),
                "task_ledger_counts": ledger_counts(ledger),
            },
            source_of_truth_refs={
                "task_ledger": sqlite_sot(ledger),
                "blackbox_verify_spec": file_sot(attempt_dir / "blackbox-verify-spec.json"),
            },
        )
        official_summary = None
        official_evidence = None
        if official_eval_each_attempt and patch:
            write_task_loop_summary(
                task_root,
                status="running",
                terminal_reason=f"official_eval_attempt_{attempt_index:03d}",
                model=model,
                task=task,
                attempts=attempts,
                ledger=ledger,
                blackbox_run_id=blackbox_run_id,
            )
            append_reality_event(
                run_root=run_root,
                loop_id=loop_id,
                context_graph_bin=context_graph_bin,
                reality_ledger=reality_ledger,
                reality_run_id=reality_run_id,
                loop_layer="official_swe_evaluator",
                event_type="official_eval_started",
                status="running",
                model=model,
                instance_id=task["instance_id"],
                attempt=attempt_index,
                reality_branch_id=attempt_branch_id,
                parent_reality_branch_id=parent_branch_id,
                promotion_status="candidate_official_eval",
                payload={
                    "patch_sha256": patch_sha256,
                    "official_eval_script": str(resolve_repo_path(official_eval_script)),
                    "timeout": official_timeout,
                    "cache_level": official_cache_level,
                },
                source_of_truth_refs={
                    "candidate_patch": file_sot(attempt_dir / "candidate.patch"),
                    "run_summary": file_sot(run_root / "refinement-loop-summary.json"),
                    "task_loop_summary": file_sot(task_root / "loop-summary.json"),
                },
            )
            try:
                official_summary = run_official_eval_for_attempt(
                    run_root=run_root,
                    attempt_dir=attempt_dir,
                    task=task,
                    model=model,
                    patch=patch,
                    context_graph_bin=context_graph_bin,
                    official_eval_script=official_eval_script,
                    swebench_python=swebench_python,
                    docker_cli_dir=docker_cli_dir,
                    docker_socket=docker_socket,
                    docker_windows_bin=docker_windows_bin,
                    max_workers=official_max_workers,
                    timeout=official_timeout,
                    cache_level=official_cache_level,
                )
            except RefinementError as err:
                append_reality_event(
                    run_root=run_root,
                    loop_id=loop_id,
                    context_graph_bin=context_graph_bin,
                    reality_ledger=reality_ledger,
                    reality_run_id=reality_run_id,
                    loop_layer="official_swe_evaluator",
                    event_type="official_eval_failed",
                    status="error",
                    model=model,
                    instance_id=task["instance_id"],
                    attempt=attempt_index,
                    reality_branch_id=attempt_branch_id,
                    parent_reality_branch_id=parent_branch_id,
                    promotion_status="candidate_error",
                    payload=err.to_json(),
                    source_of_truth_refs={
                        "attempt_dir": file_sot(attempt_dir),
                        "task_ledger": sqlite_sot(ledger),
                    },
                )
                raise
            official_evidence = build_official_evidence(
                attempt_dir=attempt_dir,
                task=task,
                model=model,
                patch=patch,
                official_summary=official_summary,
            )
            append_reality_event(
                run_root=run_root,
                loop_id=loop_id,
                context_graph_bin=context_graph_bin,
                reality_ledger=reality_ledger,
                reality_run_id=reality_run_id,
                loop_layer="official_swe_evaluator",
                event_type="official_evidence_persisted",
                status=official_evidence["official_status"],
                model=model,
                instance_id=task["instance_id"],
                attempt=attempt_index,
                reality_branch_id=attempt_branch_id,
                parent_reality_branch_id=parent_branch_id,
                promotion_status="verified_candidate" if official_evidence["resolved"] else "failed_candidate",
                payload={
                    "resolved": official_evidence["resolved"],
                    "official_status": official_evidence["official_status"],
                    "failed_fail_to_pass": official_evidence.get("failed_fail_to_pass"),
                    "failed_pass_to_pass": official_evidence.get("failed_pass_to_pass"),
                    "reward": official_evidence.get("reward"),
                    "score": official_evidence.get("score"),
                    "evidence_sha256": official_evidence.get("evidence_sha256"),
                },
                source_of_truth_refs={
                    **official_evidence.get("source_of_truth", {}),
                    "official_evidence": file_sot(Path(str(official_evidence["evidence_path"]))),
                    "task_ledger": sqlite_sot(ledger),
                },
            )
        attempt_summary = {
            "attempt": attempt_index,
            "reality_branch": {
                "branch_id": attempt_branch_id,
                "parent_branch_id": parent_branch_id,
                "promotion_status": (
                    "promoted"
                    if official_evidence and official_evidence.get("resolved")
                    else "failed_candidate"
                    if official_evidence
                    else "blocked_candidate"
                    if validation["status"] == "blocked"
                    else "local_candidate"
                ),
            },
            "status": validation["status"],
            "error_code": validation.get("error_code"),
            "model_elapsed_ms": model_elapsed_ms,
            "prompt_path": str(prompt_path),
            "prompt_manifest_path": str(attempt_dir / "prompt-manifest.json"),
            "prompt_manifest_sha256": prompt_manifest["sha256"],
            "repository_context_path": str(repo_context_path),
            "repository_context_sha256": sha256_file(repo_context_path),
            "response_path": str(response_path),
            "agent_tool_transcript": model_response.get("tool_transcript") if not generation_error else None,
            "validation_path": str(attempt_dir / "validation.json"),
            "validation_sha256": sha256_file(attempt_dir / "validation.json"),
            "parse_status": parse_status,
            "candidate_patch_sha256": patch_sha256,
            "repeated_patch": repeated_patch,
            "model_decision_record": model_decision,
            "tool_call_record": tool_call,
            "blackbox_verifier": verifier,
            "score_reward": score_reward,
            "official_eval": {
                "enabled": official_eval_each_attempt,
                "ran": official_summary is not None,
                "summary_path": official_summary.get("summary_path") if official_summary else None,
                "summary_sha256": official_summary.get("summary_sha256") if official_summary else None,
                "evidence_path": official_evidence.get("evidence_path") if official_evidence else None,
                "evidence_sha256": official_evidence.get("evidence_sha256") if official_evidence else None,
                "resolved": official_evidence.get("resolved") if official_evidence else None,
                "official_status": official_evidence.get("official_status") if official_evidence else None,
            },
            "ledger_counts": ledger_counts(ledger),
        }
        write_json(attempt_dir / "attempt-summary.json", attempt_summary)
        attempts.append(attempt_summary)
        append_reality_event(
            run_root=run_root,
            loop_id=loop_id,
            context_graph_bin=context_graph_bin,
            reality_ledger=reality_ledger,
            reality_run_id=reality_run_id,
            loop_layer="inner_swe_repair_loop",
            event_type="attempt_summary_persisted",
            status=attempt_summary["official_eval"]["official_status"] or validation["status"],
            model=model,
            instance_id=task["instance_id"],
            attempt=attempt_index,
            reality_branch_id=attempt_branch_id,
            parent_reality_branch_id=parent_branch_id,
            promotion_status=attempt_summary["reality_branch"]["promotion_status"],
            payload={
                "attempt_summary_sha256": sha256_file(attempt_dir / "attempt-summary.json"),
                "terminal_candidate": bool(official_evidence and official_evidence.get("resolved")),
                "local_validation_status": validation["status"],
                "official_eval": attempt_summary["official_eval"],
            },
            source_of_truth_refs={
                "attempt_summary": file_sot(attempt_dir / "attempt-summary.json"),
                "task_ledger": sqlite_sot(ledger),
            },
        )
        write_task_loop_summary(
            task_root,
            status="running",
            terminal_reason=terminal_reason,
            model=model,
            task=task,
            attempts=attempts,
            ledger=ledger,
            blackbox_run_id=blackbox_run_id,
        )
        if official_eval_each_attempt and official_evidence:
            if official_evidence["resolved"]:
                solved = True
                terminal_reason = "official_resolved"
                break
        elif validation["status"] == "passed":
            solved = True
            terminal_reason = "local_verified_solution_found"
            break
        if validation["status"] == "blocked" and not official_evidence:
            terminal_reason = f"blocked:{validation.get('error_code')}"
            break
        reset_workspace(repo_dir, task["base_commit"], timeout=git_timeout)
        if official_evidence:
            prompt_context = collect_repository_context(
                task,
                repo_dir,
                validation=validation,
                official_evidence=official_evidence,
                max_files=max_context_files,
                window_radius=context_window_radius,
            )
            compiled_prompt = compile_reality_prompt(
                prompt_kind="official_failure_refinement",
                task=task,
                repo_context=prompt_context,
                official_evidence=official_evidence,
                previous_patch_hashes=previous_patch_hashes,
                source_attempt=attempt_index,
                previous_prompt_sha=sha256_file(prompt_path),
                previous_output_sha=sha256_file(response_path),
            )
            prompt = compiled_prompt["prompt_text"]
            prompt_kind = str(compiled_prompt["prompt_kind"])
            prompt_template_id = str(compiled_prompt["prompt_template_id"])
            included_evidence = list(compiled_prompt["included_evidence"])
            excluded_evidence = list(compiled_prompt["excluded_evidence"])
            prompt_compiler = prompt_compiler_manifest(compiled_prompt)
            next_prompt_candidate = write_next_prompt_candidate(
                task_root,
                next_attempt=attempt_index + 1,
                source_attempt=attempt_index,
                prompt=prompt,
                prompt_context=prompt_context,
                prompt_kind=prompt_kind,
                prompt_template_id=prompt_template_id,
                included_evidence=included_evidence,
                excluded_evidence=excluded_evidence,
                no_repeat_patch_sha256=previous_patch_hashes,
                reason="official_swe_failure_feedback",
                reality_branch_id=prompt_candidate_branch_id(loop_id, model, task["instance_id"], attempt_index + 1),
                parent_reality_branch_id=attempt_branch_id,
                prompt_compiler=prompt_compiler,
            )
            append_reality_event(
                run_root=run_root,
                loop_id=loop_id,
                context_graph_bin=context_graph_bin,
                reality_ledger=reality_ledger,
                reality_run_id=reality_run_id,
                loop_layer="outer_reality_loop",
                event_type="next_prompt_compiled_from_official_evidence",
                status="ok",
                model=model,
                instance_id=task["instance_id"],
                attempt=attempt_index + 1,
                reality_branch_id=next_prompt_candidate["reality_branch_id"],
                parent_reality_branch_id=attempt_branch_id,
                promotion_status="candidate_prompt",
                payload={
                    "source_attempt": attempt_index,
                    "official_evidence_path": official_evidence["evidence_path"],
                    "official_evidence_sha256": official_evidence["evidence_sha256"],
                    "next_prompt_template_id": prompt_template_id,
                    "failed_fail_to_pass": official_evidence.get("failed_fail_to_pass"),
                    "previous_patch_hashes": previous_patch_hashes,
                    "prompt_compiler": prompt_compiler,
                    "next_prompt_candidate": next_prompt_candidate,
                },
                source_of_truth_refs={
                    "official_evidence": file_sot(Path(str(official_evidence["evidence_path"]))),
                    "repository_context": prompt_context["source_of_truth"],
                    "next_prompt_candidate": next_prompt_candidate["source_of_truth"],
                    "next_prompt": next_prompt_candidate["prompt"]["source_of_truth"],
                    "next_prompt_manifest": next_prompt_candidate["prompt_manifest"]["source_of_truth"],
                },
            )
        else:
            prompt_context = collect_repository_context(
                task,
                repo_dir,
                validation=validation,
                max_files=max_context_files,
                window_radius=context_window_radius,
            )
            compiled_prompt = compile_reality_prompt(
                prompt_kind="local_failure_refinement",
                task=task,
                repo_context=prompt_context,
                validation=validation,
                previous_patch_hashes=previous_patch_hashes,
                source_attempt=attempt_index,
                previous_prompt_sha=sha256_file(prompt_path),
                previous_output_sha=sha256_file(response_path),
            )
            prompt = compiled_prompt["prompt_text"]
            prompt_kind = str(compiled_prompt["prompt_kind"])
            prompt_template_id = str(compiled_prompt["prompt_template_id"])
            included_evidence = list(compiled_prompt["included_evidence"])
            excluded_evidence = list(compiled_prompt["excluded_evidence"])
            prompt_compiler = prompt_compiler_manifest(compiled_prompt)
            next_prompt_candidate = write_next_prompt_candidate(
                task_root,
                next_attempt=attempt_index + 1,
                source_attempt=attempt_index,
                prompt=prompt,
                prompt_context=prompt_context,
                prompt_kind=prompt_kind,
                prompt_template_id=prompt_template_id,
                included_evidence=included_evidence,
                excluded_evidence=excluded_evidence,
                no_repeat_patch_sha256=previous_patch_hashes,
                reason="local_validation_failure_feedback",
                reality_branch_id=prompt_candidate_branch_id(loop_id, model, task["instance_id"], attempt_index + 1),
                parent_reality_branch_id=attempt_branch_id,
                prompt_compiler=prompt_compiler,
            )
            append_reality_event(
                run_root=run_root,
                loop_id=loop_id,
                context_graph_bin=context_graph_bin,
                reality_ledger=reality_ledger,
                reality_run_id=reality_run_id,
                loop_layer="outer_reality_loop",
                event_type="next_prompt_compiled_from_local_evidence",
                status="ok",
                model=model,
                instance_id=task["instance_id"],
                attempt=attempt_index + 1,
                reality_branch_id=next_prompt_candidate["reality_branch_id"],
                parent_reality_branch_id=attempt_branch_id,
                promotion_status="candidate_prompt",
                payload={
                    "source_attempt": attempt_index,
                    "validation_status": validation["status"],
                    "validation_error_code": validation.get("error_code"),
                    "next_prompt_template_id": prompt_template_id,
                    "prompt_compiler": prompt_compiler,
                    "next_prompt_candidate": next_prompt_candidate,
                },
                source_of_truth_refs={
                    "validation": validation["source_of_truth"],
                    "repository_context": prompt_context["source_of_truth"],
                    "next_prompt_candidate": next_prompt_candidate["source_of_truth"],
                    "next_prompt": next_prompt_candidate["prompt"]["source_of_truth"],
                    "next_prompt_manifest": next_prompt_candidate["prompt_manifest"]["source_of_truth"],
                },
            )
        parent_branch_id = attempt_branch_id

    write_task_loop_summary(
        task_root,
        status="solved" if solved else "unsolved",
        terminal_reason=terminal_reason,
        model=model,
        task=task,
        attempts=attempts,
        ledger=ledger,
        blackbox_run_id=blackbox_run_id,
    )
    summary = json.loads((task_root / "loop-summary.json").read_text(encoding="utf-8"))
    summary["source_of_truth"]["workspace"] = git_sot(repo_dir, task["base_commit"])
    summary["source_of_truth"]["initial_repository_context"] = file_sot(task_root / "repository-context-initial.json")
    write_json(task_root / "loop-summary.json", summary)
    append_reality_event(
        run_root=run_root,
        loop_id=loop_id,
        context_graph_bin=context_graph_bin,
        reality_ledger=reality_ledger,
        reality_run_id=reality_run_id,
        loop_layer="inner_swe_repair_loop",
        event_type="task_loop_completed",
        status="solved" if solved else "unsolved",
        model=model,
        instance_id=task["instance_id"],
        reality_branch_id=task_branch_id,
        parent_reality_branch_id=task_parent_branch_id,
        promotion_status="promoted_task" if solved else "unresolved_task",
        payload={
            "terminal_reason": terminal_reason,
            "attempts_total": len(attempts),
            "loop_summary_sha256": sha256_file(task_root / "loop-summary.json"),
            "ledger_counts": ledger_counts(ledger),
        },
        source_of_truth_refs={
            "task_loop_summary": file_sot(task_root / "loop-summary.json"),
            "task_ledger": sqlite_sot(ledger),
        },
    )
    return summary


def build_report(run_root: Path, summary: dict[str, Any]) -> Path:
    lines = [
        "# ContextGraph SWE-bench Lite Refinement Loop",
        "",
        f"Loop id: `{summary['loop_id']}`",
        f"Status: `{summary['status']}`",
        f"Source of truth: `{summary['source_of_truth']}`",
        "",
        "## Results",
        "",
        "| Model | Task | Status | Attempts | Terminal Reason | Ledger |",
        "|---|---|---:|---:|---|---|",
    ]
    for item in summary["results"]:
        lines.append(
            f"| `{item['model']}` | `{item['task']['instance_id']}` | `{item['status']}` | "
            f"{item['attempts_total']} | `{item['terminal_reason']}` | `{item['ledger']['path']}` |"
        )
    lines.extend(
        [
            "",
            "## Interpretation",
            "",
            "When `official_eval_each_attempt` is true, a task is solved only when the official Docker SWE-bench report places the instance id in `resolved_ids`.",
            "Local patch application, local pytest output, and model confidence are evidence, but they are not the promotion source of truth.",
            "Failed official attempts are still useful because their report, instance log, test output, score, reward, and ledger rows become the next prompt's evidence.",
            "Blocked environment rows are not counted as model success; they identify missing execution prerequisites.",
        ]
    )
    path = run_root / "refinement-loop-report.md"
    write_text_checked(path, "\n".join(lines) + "\n")
    return path


def prediction_record_for_attempt(result: dict[str, Any], attempt: dict[str, Any]) -> dict[str, Any] | None:
    candidate_path = Path(attempt["validation_path"]).with_name("candidate.patch")
    if not candidate_path.exists():
        return None
    patch = candidate_path.read_text(encoding="utf-8")
    if not patch.strip():
        return None
    return {
        "instance_id": result["task"]["instance_id"],
        "model_name_or_path": result["model"],
        "model_patch": patch,
    }


def write_predictions_exports(run_root: Path, results: list[dict[str, Any]]) -> dict[str, Any]:
    predictions_dir = run_root / "predictions"
    predictions_dir.mkdir(parents=True, exist_ok=True)
    all_records = []
    latest_by_model_task = {}
    for result in results:
        for attempt in result.get("attempts", []):
            record = prediction_record_for_attempt(result, attempt)
            if not record:
                continue
            record_with_attempt = {
                **record,
                "contextgraph_attempt": attempt["attempt"],
                "contextgraph_validation_status": attempt["status"],
                "contextgraph_validation_error_code": attempt.get("error_code"),
            }
            all_records.append(record_with_attempt)
            latest_by_model_task[(result["model"], result["task"]["instance_id"])] = record
    all_attempts_path = predictions_dir / "all-attempts-predictions.jsonl"
    latest_path = predictions_dir / "latest-predictions.jsonl"
    write_text_checked(
        all_attempts_path,
        "".join(json.dumps(record, sort_keys=True) + "\n" for record in all_records),
    )
    latest_records = [latest_by_model_task[key] for key in sorted(latest_by_model_task)]
    write_text_checked(
        latest_path,
        "".join(json.dumps(record, sort_keys=True) + "\n" for record in latest_records),
    )
    manifest = {
        "status": "ok",
        "prediction_schema": "swebench.harness.run_evaluation.predictions_jsonl",
        "all_attempts": {
            "path": str(all_attempts_path),
            "sha256": sha256_file(all_attempts_path),
            "records": len(all_records),
            "source_of_truth": file_sot(all_attempts_path),
        },
        "latest": {
            "path": str(latest_path),
            "sha256": sha256_file(latest_path),
            "records": len(latest_records),
            "source_of_truth": file_sot(latest_path),
        },
        "official_eval_command": [
            sys.executable,
            "-m",
            "swebench.harness.run_evaluation",
            "--dataset_name",
            DATASET_NAME,
            "--predictions_path",
            str(latest_path),
            "--max_workers",
            "1",
            "--run_id",
            run_root.name,
        ],
        "source_of_truth": file_sot(predictions_dir / "predictions-manifest.json"),
    }
    write_json(predictions_dir / "predictions-manifest.json", manifest)
    return manifest


def official_evidence_from_attempt(attempt: dict[str, Any]) -> dict[str, Any] | None:
    evidence_path = attempt.get("official_eval", {}).get("evidence_path")
    if not evidence_path:
        return None
    path = Path(str(evidence_path))
    if not path.exists():
        raise RefinementError(
            "SWE_REFINEMENT_OFFICIAL_EVIDENCE_MISSING_FOR_SCOREBOARD",
            "attempt summary referenced official evidence that does not exist",
            "attempt.official_eval.evidence_path",
            "inspect attempt-summary.json before generating the scoreboard",
            details={"attempt": attempt.get("attempt"), "evidence_path": str(path)},
            source_of_truth=file_sot(path),
        )
    return json.loads(path.read_text(encoding="utf-8"))


def attempt_reward(attempt: dict[str, Any], official_evidence: dict[str, Any] | None) -> dict[str, Any] | None:
    if official_evidence and isinstance(official_evidence.get("reward"), dict):
        return official_evidence["reward"]
    return reward_payload(attempt.get("score_reward"))


def count_failed_tests(official_evidence: dict[str, Any] | None, key: str) -> int | None:
    if official_evidence is None:
        return None
    value = official_evidence.get(key)
    return len(value) if isinstance(value, list) else None


def write_reality_scoreboard(
    *,
    run_root: Path,
    loop_id: str,
    results: list[dict[str, Any]],
    reality_ledger: Path,
) -> dict[str, Any]:
    scoreboard_dir = run_root / "reality-scoreboard"
    scoreboard_dir.mkdir(parents=True, exist_ok=True)
    attempt_rows = []
    model_task_rows: dict[str, dict[str, Any]] = {}
    for result in results:
        model = result["model"]
        instance_id = result["task"]["instance_id"]
        first_reward: float | None = None
        best_reward: float | None = None
        best_attempt: int | None = None
        resolved_attempt: int | None = None
        repeated_patch_count = 0
        initial_failure_mode = None
        final_failure_mode = None
        for attempt in result.get("attempts", []):
            official_evidence = official_evidence_from_attempt(attempt)
            reward = attempt_reward(attempt, official_evidence)
            reward_raw = reward.get("raw") if isinstance(reward, dict) else None
            if isinstance(reward_raw, (int, float)):
                reward_value = float(reward_raw)
                if first_reward is None:
                    first_reward = reward_value
                if best_reward is None or reward_value > best_reward:
                    best_reward = reward_value
                    best_attempt = int(attempt["attempt"])
            if attempt.get("repeated_patch"):
                repeated_patch_count += 1
            failure_mode = (
                "official_resolved"
                if official_evidence and official_evidence.get("resolved")
                else official_evidence.get("official_status")
                if official_evidence
                else attempt.get("error_code")
                or attempt.get("status")
            )
            if initial_failure_mode is None:
                initial_failure_mode = failure_mode
            final_failure_mode = failure_mode
            if official_evidence and official_evidence.get("resolved") and resolved_attempt is None:
                resolved_attempt = int(attempt["attempt"])
            ledger_count = attempt.get("ledger_counts") if isinstance(attempt.get("ledger_counts"), dict) else {}
            reality_branch = attempt.get("reality_branch") if isinstance(attempt.get("reality_branch"), dict) else {}
            agent_tool_ref = attempt.get("agent_tool_transcript") if isinstance(attempt.get("agent_tool_transcript"), dict) else {}
            agent_tool_rounds_total = agent_tool_ref.get("rounds_total")
            agent_tool_names_used: list[str] = []
            agent_tool_result_count = 0
            agent_tool_path = Path(str(agent_tool_ref.get("path"))) if agent_tool_ref.get("path") else None
            if agent_tool_path and agent_tool_path.exists():
                agent_tool_transcript = json.loads(agent_tool_path.read_text(encoding="utf-8"))
                for round_item in agent_tool_transcript.get("rounds", []):
                    for result_item in round_item.get("tool_results", []):
                        name = result_item.get("tool_name")
                        if isinstance(name, str):
                            agent_tool_names_used.append(name)
                            agent_tool_result_count += 1
            row = {
                "suite_id": loop_id,
                "split": "dev_smoke",
                "model": model,
                "instance_id": instance_id,
                "attempt": attempt["attempt"],
                "reality_branch_id": reality_branch.get("branch_id"),
                "parent_reality_branch_id": reality_branch.get("parent_branch_id"),
                "promotion_status": reality_branch.get("promotion_status"),
                "patch_sha256": attempt.get("candidate_patch_sha256"),
                "patch_parse_status": attempt.get("parse_status", {}).get("status"),
                "local_validation_status": attempt.get("status"),
                "local_validation_error_code": attempt.get("error_code"),
                "official_eval_ran": bool(attempt.get("official_eval", {}).get("ran")),
                "official_status": official_evidence.get("official_status") if official_evidence else None,
                "official_resolved": bool(official_evidence.get("resolved")) if official_evidence else None,
                "patch_applied_official": official_evidence.get("patch_successfully_applied") if official_evidence else None,
                "failed_fail_to_pass_count": count_failed_tests(official_evidence, "failed_fail_to_pass"),
                "failed_pass_to_pass_count": count_failed_tests(official_evidence, "failed_pass_to_pass"),
                "reward": reward_raw,
                "loss": reward.get("loss") if isinstance(reward, dict) else None,
                "incident_count": int(ledger_count.get("incident_packet", 0)),
                "agent_tool_rounds_total": agent_tool_rounds_total,
                "agent_tool_result_count": agent_tool_result_count,
                "agent_tool_names_used": sorted(set(agent_tool_names_used)),
                "agent_tool_transcript_sha256": agent_tool_ref.get("sha256"),
                "prompt_template_id": json.loads(Path(attempt["prompt_manifest_path"]).read_text(encoding="utf-8")).get("prompt_template_id")
                if Path(attempt["prompt_manifest_path"]).exists()
                else None,
                "repeated_patch": bool(attempt.get("repeated_patch")),
                "source_of_truth_refs": {
                    "attempt_summary": file_sot(Path(attempt["validation_path"]).with_name("attempt-summary.json")),
                    "prompt_manifest": file_sot(Path(attempt["prompt_manifest_path"])),
                    "validation": file_sot(Path(attempt["validation_path"])),
                    "agent_tool_transcript": agent_tool_ref.get("source_of_truth"),
                    "official_evidence": file_sot(Path(str(attempt.get("official_eval", {}).get("evidence_path"))))
                    if attempt.get("official_eval", {}).get("evidence_path")
                    else None,
                    "ledger": result["ledger"]["source_of_truth"],
                },
            }
            attempt_rows.append(row)
        model_task_rows[f"{model}|{instance_id}"] = {
            "suite_id": loop_id,
            "split": "dev_smoke",
            "model": model,
            "instance_id": instance_id,
            "status": result["status"],
            "terminal_reason": result["terminal_reason"],
            "reality_branch_id": task_reality_branch_id(loop_id, model, instance_id),
            "parent_reality_branch_id": root_reality_branch_id(loop_id),
            "promotion_status": "promoted_task" if result["status"] == "solved" else "unresolved_task",
            "best_attempt": best_attempt,
            "resolved_attempt": resolved_attempt,
            "attempts_to_resolve": resolved_attempt,
            "best_reward": best_reward,
            "reward_delta": (best_reward - first_reward) if best_reward is not None and first_reward is not None else None,
            "failure_mode_initial": initial_failure_mode,
            "failure_mode_final": final_failure_mode,
            "repeated_patch_count": repeated_patch_count,
            "attempts_total": result["attempts_total"],
            "ledger": result["ledger"],
        }
    attempt_path = scoreboard_dir / "attempt-scoreboard.jsonl"
    write_text_checked(
        attempt_path,
        "".join(json_compact(row) + "\n" for row in attempt_rows),
    )
    model_task_path = scoreboard_dir / "model-task-scoreboard.json"
    write_json(model_task_path, {"status": "ok", "rows": model_task_rows})
    branches = [
        {
            "reality_branch_id": root_reality_branch_id(loop_id),
            "parent_reality_branch_id": None,
            "branch_kind": "root",
            "promotion_status": "canonical_root",
        }
    ]
    for row in model_task_rows.values():
        branches.append(
            {
                "reality_branch_id": row["reality_branch_id"],
                "parent_reality_branch_id": row["parent_reality_branch_id"],
                "branch_kind": "model_task",
                "model": row["model"],
                "instance_id": row["instance_id"],
                "promotion_status": row["promotion_status"],
                "source_of_truth": row["ledger"]["source_of_truth"],
            }
        )
    for row in attempt_rows:
        branches.append(
            {
                "reality_branch_id": row["reality_branch_id"],
                "parent_reality_branch_id": row["parent_reality_branch_id"],
                "branch_kind": "attempt",
                "model": row["model"],
                "instance_id": row["instance_id"],
                "attempt": row["attempt"],
                "promotion_status": row["promotion_status"],
                "official_resolved": row["official_resolved"],
                "reward": row["reward"],
                "source_of_truth_refs": row["source_of_truth_refs"],
            }
        )
    for result in results:
        model = result["model"]
        instance_id = result["task"]["instance_id"]
        ledger_path = Path(str(result["ledger"]["path"]))
        task_root = ledger_path.parent
        candidate_root = task_root / "prompt-candidates"
        for candidate_path in sorted(candidate_root.glob("attempt-*/next-prompt-candidate.json")):
            try:
                candidate = json.loads(candidate_path.read_text(encoding="utf-8"))
            except json.JSONDecodeError as err:
                raise RefinementError(
                    "SWE_REFINEMENT_PROMPT_CANDIDATE_JSON_INVALID",
                    "prompt candidate branch file was not valid JSON",
                    str(candidate_path),
                    "stop optimization and inspect prompt candidate persistence before continuing",
                    source_of_truth=file_sot(candidate_path),
                ) from err
            required_fields = [
                "reality_branch_id",
                "parent_reality_branch_id",
                "promotion_status",
                "source_attempt",
                "next_attempt",
                "prompt",
                "prompt_manifest",
                "repository_context",
            ]
            missing = [field for field in required_fields if field not in candidate]
            if missing:
                raise RefinementError(
                    "SWE_REFINEMENT_PROMPT_CANDIDATE_BRANCH_INCOMPLETE",
                    "prompt candidate branch was missing required provenance fields",
                    str(candidate_path),
                    "regenerate the candidate with the current refinement loop before continuing optimization",
                    details={"missing": missing},
                    source_of_truth=file_sot(candidate_path),
                )
            branches.append(
                {
                    "reality_branch_id": candidate["reality_branch_id"],
                    "parent_reality_branch_id": candidate["parent_reality_branch_id"],
                    "branch_kind": "prompt_candidate",
                    "model": model,
                    "instance_id": instance_id,
                    "source_attempt": candidate["source_attempt"],
                    "next_attempt": candidate["next_attempt"],
                    "promotion_status": candidate["promotion_status"],
                    "payload_sha256": candidate.get("payload_sha256"),
                    "file_sha256": sha256_file(candidate_path),
                    "source_of_truth_refs": {
                        "prompt_candidate": file_sot(candidate_path),
                        "prompt": candidate["prompt"]["source_of_truth"],
                        "prompt_manifest": candidate["prompt_manifest"]["source_of_truth"],
                        "repository_context": candidate["repository_context"]["source_of_truth"],
                    },
                }
            )
    branches_path = scoreboard_dir / "reality-branches.json"
    write_json(branches_path, {"status": "ok", "branches": branches})
    artifact_rows = [
        {
            "artifact_kind": "refinement_loop",
            "artifact_version": "phase1_official_feedback_v1",
            "artifact_path": str(Path(__file__).resolve()),
            "artifact_sha256": sha256_file(Path(__file__).resolve()),
            "decision": "baseline",
        },
        {
            "artifact_kind": "reward_spec",
            "artifact_version": "verified_outcome_demo_v1",
            "artifact_path": str(REPO_ROOT / "gemmafrontend" / "reward_spec.verified_outcome.json"),
            "artifact_sha256": sha256_file(REPO_ROOT / "gemmafrontend" / "reward_spec.verified_outcome.json"),
            "decision": "baseline",
        },
    ]
    artifact_path = scoreboard_dir / "artifact-scoreboard.json"
    write_json(artifact_path, {"status": "ok", "rows": artifact_rows})
    resolved = sum(1 for row in attempt_rows if row["official_resolved"])
    official_attempts = sum(1 for row in attempt_rows if row["official_eval_ran"])
    repeated = sum(1 for row in attempt_rows if row["repeated_patch"])
    incidents = sum(int(row["incident_count"]) for row in attempt_rows)
    agent_tool_results = sum(int(row["agent_tool_result_count"] or 0) for row in attempt_rows)
    agent_tool_names = sorted({name for row in attempt_rows for name in row.get("agent_tool_names_used", [])})
    aggregate = {
        "official_attempts": official_attempts,
        "official_resolved_attempts": resolved,
        "official_resolve_rate": (resolved / official_attempts) if official_attempts else None,
        "repeated_patch_rate": (repeated / len(attempt_rows)) if attempt_rows else None,
        "incident_total": incidents,
        "attempt_rows": len(attempt_rows),
        "agent_tool_results": agent_tool_results,
        "agent_tool_names_used": agent_tool_names,
    }
    report_path = scoreboard_dir / "reality-scoreboard.md"
    report_lines = [
        "# Reality Scoreboard",
        "",
        f"Loop id: `{loop_id}`",
        f"Official attempts: `{official_attempts}`",
        f"Official resolved attempts: `{resolved}`",
        f"Repeated patch attempts: `{repeated}`",
        f"Incident rows observed across attempt ledgers: `{incidents}`",
        f"Agent tool results: `{agent_tool_results}`",
        f"Agent tools used: `{', '.join(agent_tool_names) if agent_tool_names else 'none'}`",
        "",
        "## Source Of Truth",
        "",
        f"- Attempt rows: `{file_sot(attempt_path)}`",
        f"- Model/task rows: `{file_sot(model_task_path)}`",
        f"- Artifact rows: `{file_sot(artifact_path)}`",
        f"- Reality ledger: `{sqlite_sot(reality_ledger)}`",
    ]
    write_text_checked(report_path, "\n".join(report_lines) + "\n")
    manifest_path = scoreboard_dir / "source-of-truth-manifest.json"
    manifest = {
        "status": "ok",
        "loop_id": loop_id,
        "aggregate": aggregate,
        "files": {
            "attempt_scoreboard": {
                "path": str(attempt_path),
                "sha256": sha256_file(attempt_path),
                "source_of_truth": file_sot(attempt_path),
            },
            "model_task_scoreboard": {
                "path": str(model_task_path),
                "sha256": sha256_file(model_task_path),
                "source_of_truth": file_sot(model_task_path),
            },
            "artifact_scoreboard": {
                "path": str(artifact_path),
                "sha256": sha256_file(artifact_path),
                "source_of_truth": file_sot(artifact_path),
            },
            "reality_branches": {
                "path": str(branches_path),
                "sha256": sha256_file(branches_path),
                "source_of_truth": file_sot(branches_path),
            },
            "reality_scoreboard": {
                "path": str(report_path),
                "sha256": sha256_file(report_path),
                "source_of_truth": file_sot(report_path),
            },
            "reality_events": {
                "path": str(run_root / "reality-events.jsonl"),
                "sha256": sha256_file(run_root / "reality-events.jsonl") if (run_root / "reality-events.jsonl").exists() else None,
                "source_of_truth": file_sot(run_root / "reality-events.jsonl"),
            },
            "reality_ledger": {
                "path": str(reality_ledger),
                "sha256": sha256_file(reality_ledger),
                "source_of_truth": sqlite_sot(reality_ledger),
                "physical_counts": ledger_counts(reality_ledger),
            },
        },
        "source_of_truth": file_sot(manifest_path),
    }
    write_json(manifest_path, manifest)
    return {
        "status": "ok",
        "aggregate": aggregate,
        "attempt_scoreboard": manifest["files"]["attempt_scoreboard"],
        "model_task_scoreboard": manifest["files"]["model_task_scoreboard"],
        "artifact_scoreboard": manifest["files"]["artifact_scoreboard"],
        "reality_branches": manifest["files"]["reality_branches"],
        "reality_scoreboard": manifest["files"]["reality_scoreboard"],
        "source_of_truth_manifest": {
            "path": str(manifest_path),
            "sha256": sha256_file(manifest_path),
            "source_of_truth": file_sot(manifest_path),
        },
    }


def write_root_loop_summary(
    *,
    run_root: Path,
    loop_id: str,
    status: str,
    dataset: str,
    models: list[str],
    tasks: list[dict[str, Any]],
    max_attempts: int,
    include_hints: bool,
    official_eval: dict[str, Any],
    agent_tools: dict[str, Any] | None,
    results: list[dict[str, Any]],
    reality_ledger: Path | None = None,
    reality_run_id: str | None = None,
    predictions: dict[str, Any] | None = None,
    scoreboard: dict[str, Any] | None = None,
    report_path: Path | None = None,
) -> dict[str, Any]:
    path = run_root / "refinement-loop-summary.json"
    summary = {
        "status": status,
        "loop_id": loop_id,
        "created_at_unix": time.time(),
        "source_of_truth": file_sot(path),
        "dataset": dataset,
        "models": models,
        "tasks": tasks,
        "max_attempts": max_attempts,
        "include_hints": include_hints,
        "official_eval": official_eval,
        "agent_tools": agent_tools,
        "reality_loop": {
            "run_id": reality_run_id,
            "ledger": {
                "path": str(reality_ledger) if reality_ledger else None,
                "sha256": sha256_file(reality_ledger) if reality_ledger and reality_ledger.exists() else None,
                "physical_counts": ledger_counts(reality_ledger) if reality_ledger and reality_ledger.exists() else {},
                "source_of_truth": sqlite_sot(reality_ledger) if reality_ledger else None,
            },
            "event_log": {
                "path": str(run_root / "reality-events.jsonl"),
                "sha256": sha256_file(run_root / "reality-events.jsonl") if (run_root / "reality-events.jsonl").exists() else None,
                "source_of_truth": file_sot(run_root / "reality-events.jsonl"),
            },
            "state": {
                "path": str(run_root / "reality-state.json"),
                "sha256": sha256_file(run_root / "reality-state.json") if (run_root / "reality-state.json").exists() else None,
                "source_of_truth": file_sot(run_root / "reality-state.json"),
            },
        },
        "results": results,
    }
    if predictions is not None:
        summary["predictions"] = predictions
    if scoreboard is not None:
        summary["scoreboard"] = scoreboard
    if report_path is not None:
        summary["report"] = {
            "path": str(report_path),
            "sha256": sha256_file(report_path),
            "source_of_truth": file_sot(report_path),
        }
    write_json(path, summary)
    return summary


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--model", action="append", dest="models", default=None)
    parser.add_argument("--instance-id", action="append", dest="instance_ids", default=None)
    parser.add_argument("--repo-filter", action="append", dest="repo_filter", default=None)
    parser.add_argument("--max-tasks", type=int, default=1)
    parser.add_argument("--max-attempts", type=int, default=3)
    parser.add_argument("--runtime-root", type=Path, default=DEFAULT_RUNTIME_ROOT)
    parser.add_argument("--ollama-url", default="http://127.0.0.1:11434")
    parser.add_argument("--context-graph-bin", type=Path, default=DEFAULT_CONTEXT_GRAPH_BIN)
    parser.add_argument("--include-hints", action="store_true")
    parser.add_argument("--model-timeout", type=float, default=300.0)
    parser.add_argument("--git-timeout", type=float, default=600.0)
    parser.add_argument("--pytest-timeout", type=float, default=600.0)
    parser.add_argument("--max-context-files", type=int, default=5)
    parser.add_argument("--context-window-radius", type=int, default=24)
    parser.add_argument("--official-eval-each-attempt", action="store_true")
    parser.add_argument("--official-eval-script", type=Path, default=DEFAULT_OFFICIAL_EVAL_SCRIPT)
    parser.add_argument("--swebench-python", type=Path, default=DEFAULT_SWEBENCH_PYTHON)
    parser.add_argument("--docker-cli-dir", type=Path, default=DEFAULT_DOCKER_CLI_DIR)
    parser.add_argument("--docker-socket", type=Path, default=DEFAULT_DOCKER_SOCKET)
    parser.add_argument("--docker-windows-bin", type=Path, default=DEFAULT_DOCKER_WINDOWS_BIN)
    parser.add_argument("--official-max-workers", type=int, default=1)
    parser.add_argument("--official-timeout", type=int, default=900)
    parser.add_argument("--official-cache-level", default="env")
    parser.add_argument("--agent-tool-rounds", type=int, default=DEFAULT_AGENT_TOOL_ROUNDS)
    args = parser.parse_args()

    if args.max_attempts < 1:
        raise SystemExit("--max-attempts must be at least 1")
    if args.max_context_files < 1:
        raise SystemExit("--max-context-files must be at least 1")
    if args.context_window_radius < 1:
        raise SystemExit("--context-window-radius must be at least 1")
    if args.official_max_workers < 1:
        raise SystemExit("--official-max-workers must be at least 1")
    if args.official_timeout < 1:
        raise SystemExit("--official-timeout must be at least 1")
    if args.agent_tool_rounds < 0:
        raise SystemExit("--agent-tool-rounds must be at least 0")
    models = args.models or DEFAULT_MODELS
    loop_id = f"swe_lite_refine_{now_ns()}_{hashlib.sha256('|'.join(models).encode()).hexdigest()[:8]}"
    run_root = args.runtime_root / loop_id
    run_root.mkdir(parents=True, exist_ok=False)
    workspaces = args.runtime_root / "workspaces"
    results = []
    reality_ledger = run_root / "reality-loop.sqlite"
    reality_run_id = None
    try:
        context_graph_bin = require_context_graph(args.context_graph_bin)
        official_eval = {
            "enabled_each_attempt": args.official_eval_each_attempt,
            "script": str(resolve_repo_path(args.official_eval_script)),
            "swebench_python": str(args.swebench_python),
            "docker_cli_dir": str(args.docker_cli_dir),
            "docker_socket": str(args.docker_socket),
            "docker_windows_bin": str(args.docker_windows_bin),
            "max_workers": args.official_max_workers,
            "timeout": args.official_timeout,
            "cache_level": args.official_cache_level,
        }
        agent_tools = {
            "enabled": args.agent_tool_rounds > 0,
            "max_rounds": args.agent_tool_rounds,
            "surface": agent_tool_surface(),
        }
        tasks = load_swe_lite_tasks(
            instance_ids=args.instance_ids or [],
            repo_filter=args.repo_filter or [],
            max_tasks=args.max_tasks,
        )
        selected = [
            {"instance_id": task["instance_id"], "repo": task["repo"], "base_commit": task["base_commit"]}
            for task in tasks
        ]
        write_json(
            run_root / "selection.json",
            {
                "dataset": DATASET_NAME,
                "selected_tasks": selected,
                "models": models,
                "max_attempts": args.max_attempts,
                "include_hints": args.include_hints,
                "official_eval": official_eval,
                "agent_tools": agent_tools,
            },
        )
        init_blackbox(context_graph_bin, reality_ledger, run_root)
        reality_run_id = record_reality_run(
            context_graph_bin,
            reality_ledger,
            loop_id=loop_id,
            models=models,
            tasks=selected,
            cwd=run_root,
        )
        append_reality_event(
            run_root=run_root,
            loop_id=loop_id,
            context_graph_bin=context_graph_bin,
            reality_ledger=reality_ledger,
            reality_run_id=reality_run_id,
            loop_layer="outer_reality_loop",
            event_type="root_loop_started",
            status="running",
            reality_branch_id=root_reality_branch_id(loop_id),
            parent_reality_branch_id=None,
            promotion_status="canonical_root",
            payload={
                "models": models,
                "tasks": selected,
                "max_attempts": args.max_attempts,
                "official_eval_each_attempt": args.official_eval_each_attempt,
                "agent_tool_rounds": args.agent_tool_rounds,
            },
            source_of_truth_refs={
                "selection": file_sot(run_root / "selection.json"),
                "reality_ledger": sqlite_sot(reality_ledger),
            },
        )
        write_root_loop_summary(
            run_root=run_root,
            loop_id=loop_id,
            status="running",
            dataset=DATASET_NAME,
            models=models,
            tasks=selected,
            max_attempts=args.max_attempts,
            include_hints=args.include_hints,
            official_eval=official_eval,
            agent_tools=agent_tools,
            results=results,
            reality_ledger=reality_ledger,
            reality_run_id=reality_run_id,
        )
        append_reality_event(
            run_root=run_root,
            loop_id=loop_id,
            context_graph_bin=context_graph_bin,
            reality_ledger=reality_ledger,
            reality_run_id=reality_run_id,
            loop_layer="outer_reality_loop",
            event_type="root_summary_persisted",
            status="ok",
            reality_branch_id=root_reality_branch_id(loop_id),
            parent_reality_branch_id=None,
            promotion_status="canonical_root",
            payload={
                "summary_path": str(run_root / "refinement-loop-summary.json"),
                "summary_sha256": sha256_file(run_root / "refinement-loop-summary.json"),
            },
            source_of_truth_refs={
                "root_summary": file_sot(run_root / "refinement-loop-summary.json"),
                "reality_state": file_sot(run_root / "reality-state.json"),
            },
        )
        for task in tasks:
            for model in models:
                print(f"RUNNING={model}:{task['instance_id']}", flush=True)
                result = run_model_task_loop(
                    task=task,
                    model=model,
                    run_root=run_root,
                    workspaces=workspaces,
                    context_graph_bin=context_graph_bin,
                    ollama_url=args.ollama_url,
                    max_attempts=args.max_attempts,
                    include_hints=args.include_hints,
                    model_timeout=args.model_timeout,
                    git_timeout=args.git_timeout,
                    pytest_timeout=args.pytest_timeout,
                    max_context_files=args.max_context_files,
                    context_window_radius=args.context_window_radius,
                    official_eval_each_attempt=args.official_eval_each_attempt,
                    official_eval_script=args.official_eval_script,
                    swebench_python=args.swebench_python,
                    docker_cli_dir=args.docker_cli_dir,
                    docker_socket=args.docker_socket,
                    docker_windows_bin=args.docker_windows_bin,
                    official_max_workers=args.official_max_workers,
                    official_timeout=args.official_timeout,
                    official_cache_level=args.official_cache_level,
                    agent_tool_rounds=args.agent_tool_rounds,
                    loop_id=loop_id,
                    reality_ledger=reality_ledger,
                    reality_run_id=reality_run_id,
                )
                results.append(result)
                write_root_loop_summary(
                    run_root=run_root,
                    loop_id=loop_id,
                    status="running",
                    dataset=DATASET_NAME,
                    models=models,
                    tasks=selected,
                    max_attempts=args.max_attempts,
                    include_hints=args.include_hints,
                    official_eval=official_eval,
                    agent_tools=agent_tools,
                    results=results,
                    reality_ledger=reality_ledger,
                    reality_run_id=reality_run_id,
                )
                append_reality_event(
                    run_root=run_root,
                    loop_id=loop_id,
                    context_graph_bin=context_graph_bin,
                    reality_ledger=reality_ledger,
                    reality_run_id=reality_run_id,
                    loop_layer="outer_reality_loop",
                    event_type="root_summary_updated_after_task",
                    status=result["status"],
                    model=model,
                    instance_id=task["instance_id"],
                    reality_branch_id=task_reality_branch_id(loop_id, model, task["instance_id"]),
                    parent_reality_branch_id=root_reality_branch_id(loop_id),
                    promotion_status="promoted_task" if result["status"] == "solved" else "unresolved_task",
                    payload={
                        "terminal_reason": result["terminal_reason"],
                        "attempts_total": result["attempts_total"],
                        "summary_sha256": sha256_file(run_root / "refinement-loop-summary.json"),
                    },
                    source_of_truth_refs={
                        "root_summary": file_sot(run_root / "refinement-loop-summary.json"),
                        "task_loop_summary": result["source_of_truth"]["loop_summary"],
                        "task_ledger": result["ledger"]["source_of_truth"],
                    },
                )
                print(f"DONE={model}:{task['instance_id']}:{result['status']}:{result['terminal_reason']}", flush=True)
        predictions = write_predictions_exports(run_root, results)
        scoreboard = write_reality_scoreboard(
            run_root=run_root,
            loop_id=loop_id,
            results=results,
            reality_ledger=reality_ledger,
        )
        summary = write_root_loop_summary(
            run_root=run_root,
            loop_id=loop_id,
            status="ok",
            dataset=DATASET_NAME,
            models=models,
            tasks=selected,
            max_attempts=args.max_attempts,
            include_hints=args.include_hints,
            official_eval=official_eval,
            agent_tools=agent_tools,
            results=results,
            reality_ledger=reality_ledger,
            reality_run_id=reality_run_id,
            predictions=predictions,
            scoreboard=scoreboard,
        )
        report = build_report(run_root, summary)
        summary = write_root_loop_summary(
            run_root=run_root,
            loop_id=loop_id,
            status="ok",
            dataset=DATASET_NAME,
            models=models,
            tasks=selected,
            max_attempts=args.max_attempts,
            include_hints=args.include_hints,
            official_eval=official_eval,
            agent_tools=agent_tools,
            results=results,
            reality_ledger=reality_ledger,
            reality_run_id=reality_run_id,
            predictions=predictions,
            scoreboard=scoreboard,
            report_path=report,
        )
        append_reality_event(
            run_root=run_root,
            loop_id=loop_id,
            context_graph_bin=context_graph_bin,
            reality_ledger=reality_ledger,
            reality_run_id=reality_run_id,
            loop_layer="outer_reality_loop",
            event_type="root_loop_completed",
            status="ok",
            reality_branch_id=root_reality_branch_id(loop_id),
            parent_reality_branch_id=None,
            promotion_status="canonical_root",
            payload={
                "results_total": len(results),
                "predictions": predictions,
                "scoreboard": scoreboard,
                "report_path": str(report),
                "report_sha256": sha256_file(report),
            },
            source_of_truth_refs={
                "root_summary": file_sot(run_root / "refinement-loop-summary.json"),
                "scoreboard_manifest": scoreboard["source_of_truth_manifest"]["source_of_truth"],
                "report": file_sot(report),
                "reality_ledger": sqlite_sot(reality_ledger),
            },
        )
        scoreboard = write_reality_scoreboard(
            run_root=run_root,
            loop_id=loop_id,
            results=results,
            reality_ledger=reality_ledger,
        )
        write_root_loop_summary(
            run_root=run_root,
            loop_id=loop_id,
            status="ok",
            dataset=DATASET_NAME,
            models=models,
            tasks=selected,
            max_attempts=args.max_attempts,
            include_hints=args.include_hints,
            official_eval=official_eval,
            agent_tools=agent_tools,
            results=results,
            reality_ledger=reality_ledger,
            reality_run_id=reality_run_id,
            predictions=predictions,
            scoreboard=scoreboard,
            report_path=report,
        )
        print("SWE_REFINEMENT_SUMMARY=" + str(run_root / "refinement-loop-summary.json"), flush=True)
        print("SWE_REFINEMENT_REPORT=" + str(report), flush=True)
        return 0
    except (RefinementError, PromptCompilerError) as err:
        error = err.to_json()
        error["partial_results"] = results
        error["reality_loop"] = {
            "run_id": reality_run_id,
            "ledger": sqlite_sot(reality_ledger),
            "event_log": file_sot(run_root / "reality-events.jsonl"),
        }
        write_json(run_root / "refinement-loop-error.json", error)
        if reality_run_id is not None and reality_ledger.exists():
            append_reality_event(
                run_root=run_root,
                loop_id=loop_id,
                context_graph_bin=context_graph_bin,
                reality_ledger=reality_ledger,
                reality_run_id=reality_run_id,
                loop_layer="outer_reality_loop",
                event_type="root_loop_failed",
                status="error",
                reality_branch_id=root_reality_branch_id(loop_id),
                parent_reality_branch_id=None,
                promotion_status="failed_root",
                payload=error,
                source_of_truth_refs={
                    "error": file_sot(run_root / "refinement-loop-error.json"),
                    "reality_ledger": sqlite_sot(reality_ledger),
                },
            )
        print("SWE_REFINEMENT_ERROR=" + str(run_root / "refinement-loop-error.json"), flush=True)
        print(json.dumps(error, indent=2, sort_keys=True), flush=True)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
