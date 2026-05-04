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


DATASET_NAME = "princeton-nlp/SWE-bench_Lite"
DEFAULT_MODELS = ["gemma4:e4b", "qwen3:8b", "mistral-nemo:12b"]
DEFAULT_RUNTIME_ROOT = Path("/tmp/contextgraph-swe-lite-refinement")
DEFAULT_CONTEXT_GRAPH_BIN = Path("target/debug/context-graph")
MAX_FAILURE_TEXT_CHARS = 6000
MAX_REPO_CONTEXT_CHARS = 24000
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
    deduped: list[str] = []
    seen: set[str] = set()
    for term in terms:
        normalized = term.strip()
        key = normalized.lower()
        if normalized and key not in seen:
            seen.add(key)
            deduped.append(normalized)
    return deduped[:80]


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
    max_files: int,
    window_radius: int,
) -> dict[str, Any]:
    terms = problem_terms(task)
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

        The patch should be minimal. Do not repeat the same patch if the failure evidence shows it did not apply or did not pass tests.
        Do not include repeated no-op hunks.
        """
    ).strip()


def parse_model_patch(response_text: str) -> tuple[str | None, dict[str, Any]]:
    stripped = response_text.strip()
    candidates = [stripped]
    fence = re.search(r"```(?:json)?\s*(\{.*?\})\s*```", stripped, flags=re.DOTALL)
    if fence:
        candidates.insert(0, fence.group(1))
    if "{" in stripped and "}" in stripped:
        candidates.append(stripped[stripped.find("{") : stripped.rfind("}") + 1])
    parse_errors = []
    for candidate in candidates:
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


def ollama_generate_patch(
    *,
    ollama_url: str,
    model: str,
    prompt: str,
    timeout: float,
) -> dict[str, Any]:
    body = {
        "model": model,
        "messages": [
            {
                "role": "system",
                "content": (
                    "You are a software repair model. Output only the required JSON object. "
                    "The patch must be a unified git diff against the provided base repository."
                ),
            },
            {"role": "user", "content": prompt},
        ],
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
    reward_spec = Path.cwd() / "gemmafrontend" / "reward_spec.verified_outcome.json"
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
) -> dict[str, Any]:
    safe_model = model.replace(":", "-").replace("/", "-")
    task_root = run_root / "tasks" / task["instance_id"] / safe_model
    task_root.mkdir(parents=True, exist_ok=True)
    ledger = task_root / "blackbox.sqlite"
    init_blackbox(context_graph_bin, ledger, task_root)
    blackbox_run_id = record_run(context_graph_bin, ledger, task, model, task_root)
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
    prompt = build_initial_prompt(task, include_hints=include_hints, repo_context=prompt_context)
    attempts = []
    solved = False
    terminal_reason = "attempt_budget_exhausted"
    for attempt_index in range(1, max_attempts + 1):
        attempt_dir = task_root / "attempts" / f"attempt-{attempt_index:03d}"
        attempt_dir.mkdir(parents=True, exist_ok=False)
        repo_context_path = attempt_dir / "repository-context.json"
        write_json(repo_context_path, prompt_context)
        prompt_path = attempt_dir / "prompt.txt"
        write_text_checked(prompt_path, prompt)
        model_started = time.perf_counter()
        generation_error = None
        try:
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

        decision_input = {
            "attempt": attempt_index,
            "prompt_sha256": sha256_file(prompt_path),
            "repository_context_sha256": sha256_file(repo_context_path),
            "repository_context_source_of_truth": prompt_context["source_of_truth"],
            "response_sha256": sha256_file(response_path),
            "parse_status": parse_status,
            "model_metadata": model_metadata,
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
        attempt_summary = {
            "attempt": attempt_index,
            "status": validation["status"],
            "error_code": validation.get("error_code"),
            "model_elapsed_ms": model_elapsed_ms,
            "prompt_path": str(prompt_path),
            "repository_context_path": str(repo_context_path),
            "repository_context_sha256": sha256_file(repo_context_path),
            "response_path": str(response_path),
            "validation_path": str(attempt_dir / "validation.json"),
            "validation_sha256": sha256_file(attempt_dir / "validation.json"),
            "parse_status": parse_status,
            "model_decision_record": model_decision,
            "tool_call_record": tool_call,
            "blackbox_verifier": verifier,
            "score_reward": score_reward,
            "ledger_counts": ledger_counts(ledger),
        }
        write_json(attempt_dir / "attempt-summary.json", attempt_summary)
        attempts.append(attempt_summary)
        if validation["status"] == "passed":
            solved = True
            terminal_reason = "verified_solution_found"
            break
        if validation["status"] == "blocked":
            terminal_reason = f"blocked:{validation.get('error_code')}"
            break
        reset_workspace(repo_dir, task["base_commit"], timeout=git_timeout)
        prompt_context = collect_repository_context(
            task,
            repo_dir,
            validation=validation,
            max_files=max_context_files,
            window_radius=context_window_radius,
        )
        prompt = build_refinement_prompt(task, validation, prompt_context)

    summary = {
        "status": "solved" if solved else "unsolved",
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
            "sha256": sha256_file(ledger),
            "source_of_truth": sqlite_sot(ledger),
            "physical_counts": ledger_counts(ledger),
        },
        "blackbox_run_id": blackbox_run_id,
        "source_of_truth": {
            "task_manifest": file_sot(task_root / "task-manifest.json"),
            "workspace": git_sot(repo_dir, task["base_commit"]),
            "ledger": sqlite_sot(ledger),
            "initial_repository_context": file_sot(task_root / "repository-context-initial.json"),
        },
    }
    write_json(task_root / "loop-summary.json", summary)
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
            "A task is solved only when the candidate patch applies, the official SWE-bench test patch applies, and the FAIL_TO_PASS command exits zero.",
            "Failed and blocked attempts are still valuable because their verifier output becomes the next prompt's evidence.",
            "Blocked environment rows are not counted as model success; they identify missing local execution prerequisites.",
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
    args = parser.parse_args()

    if args.max_attempts < 1:
        raise SystemExit("--max-attempts must be at least 1")
    if args.max_context_files < 1:
        raise SystemExit("--max-context-files must be at least 1")
    if args.context_window_radius < 1:
        raise SystemExit("--context-window-radius must be at least 1")
    models = args.models or DEFAULT_MODELS
    loop_id = f"swe_lite_refine_{now_ns()}_{hashlib.sha256('|'.join(models).encode()).hexdigest()[:8]}"
    run_root = args.runtime_root / loop_id
    run_root.mkdir(parents=True, exist_ok=False)
    workspaces = args.runtime_root / "workspaces"
    results = []
    try:
        context_graph_bin = require_context_graph(args.context_graph_bin)
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
                )
                results.append(result)
                print(f"DONE={model}:{task['instance_id']}:{result['status']}:{result['terminal_reason']}", flush=True)
        summary = {
            "status": "ok",
            "loop_id": loop_id,
            "created_at_unix": time.time(),
            "source_of_truth": file_sot(run_root / "refinement-loop-summary.json"),
            "dataset": DATASET_NAME,
            "models": models,
            "tasks": selected,
            "results": results,
        }
        summary["predictions"] = write_predictions_exports(run_root, results)
        write_json(run_root / "refinement-loop-summary.json", summary)
        report = build_report(run_root, summary)
        print("SWE_REFINEMENT_SUMMARY=" + str(run_root / "refinement-loop-summary.json"), flush=True)
        print("SWE_REFINEMENT_REPORT=" + str(report), flush=True)
        return 0
    except RefinementError as err:
        error = err.to_json()
        error["partial_results"] = results
        write_json(run_root / "refinement-loop-error.json", error)
        print("SWE_REFINEMENT_ERROR=" + str(run_root / "refinement-loop-error.json"), flush=True)
        print(json.dumps(error, indent=2, sort_keys=True), flush=True)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
