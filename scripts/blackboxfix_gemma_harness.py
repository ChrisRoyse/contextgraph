#!/usr/bin/env python3
"""Run a local Gemma model through BlackBoxFix full-state verification.

This harness sits between Ollama and BlackBoxFix:
1. preflight the physical ledger, Ollama server, model inventory, and target file;
2. ask a real Gemma model for a structured artifact;
3. write the artifact to the filesystem source of truth;
4. record agent_run, agent_step, model_decision, and tool_call CloudEvents;
5. verify the persisted file by an independent command readback;
6. export evidence, episodes, OpenTelemetry, dashboard, scorecard, and FSV proof.

No mock connectors are used. Any failed source-of-truth read exits non-zero with
a typed JSON error on stderr.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import secrets
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


HARNESS_VERSION = "0.1.0"
SDK_METADATA = {"name": "blackboxfix-gemma-harness", "version": HARNESS_VERSION}
EVENT_AGENT_RUN = "dev.contextgraph.blackbox.agent_run.v1"
EVENT_AGENT_STEP = "dev.contextgraph.blackbox.agent_step.v1"
EVENT_MODEL_DECISION = "dev.contextgraph.blackbox.model_decision.v1"
EVENT_TOOL_CALL = "dev.contextgraph.blackbox.tool_call.v1"
MODEL_OUTPUT_SCHEMA = {
    "type": "object",
    "properties": {
        "summary": {"type": "string", "minLength": 1},
        "file_content": {"type": "string", "minLength": 1},
        "confidence": {"type": "number", "minimum": 0.0, "maximum": 1.0},
    },
    "required": ["summary", "file_content", "confidence"],
    "additionalProperties": False,
}


class HarnessError(Exception):
    def __init__(
        self,
        error_code: str,
        message: str,
        field_path: str,
        remediation: str,
        *,
        source_of_truth: str | None = None,
        details: dict[str, Any] | None = None,
        exit_code: int = 2,
    ) -> None:
        super().__init__(message)
        self.error_code = error_code
        self.message = message
        self.field_path = field_path
        self.remediation = remediation
        self.source_of_truth = source_of_truth
        self.details = details or {}
        self.exit_code = exit_code

    def to_json(self) -> dict[str, Any]:
        return {
            "status": "error",
            "error_code": self.error_code,
            "message": self.message,
            "field_path": self.field_path,
            "remediation": self.remediation,
            "source_of_truth": self.source_of_truth,
            "details": self.details,
        }


@dataclass
class CommandResult:
    args: list[str]
    stdout: str
    stderr: str
    exit_code: int

    def json_stdout(self) -> dict[str, Any]:
        try:
            value = json.loads(self.stdout)
        except json.JSONDecodeError as err:
            raise HarnessError(
                "GEMMA_HARNESS_COMMAND_STDOUT_JSON_INVALID",
                f"command stdout was not valid JSON: {err}",
                "stdout",
                "inspect the command stdout/stderr and rerun only after the command emits one JSON object",
                details={"command": self.args, "stdout": self.stdout, "stderr": self.stderr},
            ) from err
        if not isinstance(value, dict):
            raise HarnessError(
                "GEMMA_HARNESS_COMMAND_STDOUT_SHAPE_INVALID",
                "command stdout JSON was not an object",
                "stdout",
                "inspect the CLI contract; BlackBoxFix commands must return a JSON object",
                details={"command": self.args, "stdout": self.stdout},
            )
        return value


def now_rfc3339() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def print_json(value: dict[str, Any], *, stream: Any = sys.stdout) -> None:
    stream.write(json.dumps(value, indent=2, sort_keys=True) + "\n")
    stream.flush()


def sha256_bytes(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def sha256_file(path: Path) -> str:
    try:
        return sha256_bytes(path.read_bytes())
    except OSError as err:
        raise HarnessError(
            "GEMMA_HARNESS_FILE_READ_FAILED",
            f"failed to read file for SHA-256: {err}",
            str(path),
            "fix filesystem permissions and rerun verification",
            source_of_truth=file_source_of_truth(path),
        ) from err


def file_source_of_truth(path: Path) -> str:
    return f"file:{path.resolve()}"


def ledger_source_of_truth(ledger: Path) -> str:
    return f"sqlite:{ledger.resolve()}#ledger_records"


def write_json_file(path: Path, value: Any) -> None:
    data = json.dumps(value, indent=2, sort_keys=True).encode("utf-8") + b"\n"
    atomic_write_bytes(path, data)


def atomic_write_bytes(path: Path, data: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    fd = None
    temp_path = path.with_name(f".{path.name}.tmp-{os.getpid()}-{secrets.token_hex(6)}")
    try:
        fd = os.open(temp_path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        with os.fdopen(fd, "wb") as handle:
            fd = None
            handle.write(data)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temp_path, path)
        dir_fd = os.open(path.parent, os.O_RDONLY)
        try:
            os.fsync(dir_fd)
        finally:
            os.close(dir_fd)
    except OSError as err:
        try:
            temp_path.unlink(missing_ok=True)
        except OSError:
            pass
        raise HarnessError(
            "GEMMA_HARNESS_ATOMIC_WRITE_FAILED",
            f"failed to atomically write {path}: {err}",
            str(path),
            "check filesystem permissions, free space, and parent directory state",
            source_of_truth=file_source_of_truth(path),
        ) from err
    finally:
        if fd is not None:
            os.close(fd)


def run_command(args: list[str], *, cwd: Path | None = None, timeout: float = 300.0) -> CommandResult:
    started = time.time()
    try:
        proc = subprocess.run(
            args,
            cwd=str(cwd) if cwd else None,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=timeout,
            check=False,
        )
    except subprocess.TimeoutExpired as err:
        raise HarnessError(
            "GEMMA_HARNESS_COMMAND_TIMEOUT",
            f"command timed out after {timeout}s",
            "command",
            "inspect the blocked command and rerun only after source-of-truth reads complete within timeout",
            details={"command": args, "timeout_seconds": timeout},
        ) from err
    except OSError as err:
        raise HarnessError(
            "GEMMA_HARNESS_COMMAND_EXEC_FAILED",
            f"failed to execute command: {err}",
            "command[0]",
            "install the executable or pass the correct --blackbox-bin path",
            details={"command": args},
        ) from err
    result = CommandResult(args=args, stdout=proc.stdout, stderr=proc.stderr, exit_code=proc.returncode)
    if proc.returncode != 0:
        parsed_stderr: Any = None
        try:
            parsed_stderr = json.loads(proc.stderr)
        except json.JSONDecodeError:
            parsed_stderr = proc.stderr
        raise HarnessError(
            "GEMMA_HARNESS_COMMAND_FAILED",
            f"command exited non-zero after {time.time() - started:.3f}s",
            "command",
            "inspect the BlackBoxFix error_code/field_path/remediation and fix the root cause before rerunning",
            details={
                "command": args,
                "exit_code": proc.returncode,
                "stdout": proc.stdout,
                "stderr": parsed_stderr,
            },
            exit_code=proc.returncode,
        )
    return result


def run_blackbox(args: argparse.Namespace, command: list[str], *, timeout: float = 300.0) -> dict[str, Any]:
    result = run_command([str(args.blackbox_bin), "blackbox", *command], timeout=timeout)
    return result.json_stdout()


def http_json(url: str, body: dict[str, Any] | None = None, *, timeout: float = 300.0) -> dict[str, Any]:
    data = None if body is None else json.dumps(body).encode("utf-8")
    method = "GET" if body is None else "POST"
    request = urllib.request.Request(
        url,
        data=data,
        headers={"Content-Type": "application/json"},
        method=method,
    )
    try:
        with urllib.request.urlopen(request, timeout=timeout) as response:
            raw = response.read()
    except urllib.error.HTTPError as err:
        body_text = err.read().decode("utf-8", errors="replace")
        parsed_body: Any = body_text
        try:
            parsed_body = json.loads(body_text)
        except json.JSONDecodeError:
            pass
        raise HarnessError(
            "GEMMA_HARNESS_OLLAMA_HTTP_ERROR",
            f"Ollama API returned HTTP {err.code}",
            "ollama_response.status",
            "inspect the Ollama error body, fix the model request/runtime issue, and rerun from a clean ledger or after reviewing partial state",
            source_of_truth=url,
            details={"http_status": err.code, "body": parsed_body},
        ) from err
    except urllib.error.URLError as err:
        raise HarnessError(
            "GEMMA_HARNESS_OLLAMA_UNREACHABLE",
            f"Ollama API request failed: {err}",
            "ollama_url",
            "start Ollama, verify it listens on the requested URL, and rerun without changing ledger state",
            source_of_truth=url,
        ) from err
    try:
        value = json.loads(raw)
    except json.JSONDecodeError as err:
        raise HarnessError(
            "GEMMA_HARNESS_OLLAMA_JSON_INVALID",
            f"Ollama response was not valid JSON: {err}",
            "ollama_response",
            "inspect the Ollama server response and rerun when it emits valid API JSON",
            source_of_truth=url,
            details={"raw_response": raw.decode("utf-8", errors="replace")},
        ) from err
    if not isinstance(value, dict):
        raise HarnessError(
            "GEMMA_HARNESS_OLLAMA_SHAPE_INVALID",
            "Ollama response JSON was not an object",
            "ollama_response",
            "inspect the Ollama API endpoint and rerun with the generate/tags API",
            source_of_truth=url,
        )
    return value


def ollama_tags(args: argparse.Namespace) -> dict[str, Any]:
    return http_json(f"{args.ollama_url.rstrip('/')}/api/tags", timeout=args.timeout_seconds)


def ensure_ollama_model(args: argparse.Namespace, model: str) -> dict[str, Any]:
    tags = ollama_tags(args)
    models = tags.get("models")
    if not isinstance(models, list):
        raise HarnessError(
            "GEMMA_HARNESS_OLLAMA_TAGS_INVALID",
            "Ollama /api/tags response did not contain a models array",
            "ollama.tags.models",
            "inspect Ollama API compatibility and rerun",
            source_of_truth=f"{args.ollama_url.rstrip()}/api/tags",
            details={"response": tags},
        )
    names = sorted(item.get("name") for item in models if isinstance(item, dict) and item.get("name"))
    if model not in names:
        raise HarnessError(
            "GEMMA_HARNESS_MODEL_NOT_FOUND",
            f"required model {model!r} is not present in Ollama",
            "model",
            "install the exact model tag with ollama pull or pass a model name already present in /api/tags",
            source_of_truth=f"{args.ollama_url.rstrip()}/api/tags",
            details={"available_models": names},
        )
    return tags


def validate_prompt(prompt: str) -> None:
    if not prompt or not prompt.strip():
        raise HarnessError(
            "GEMMA_HARNESS_PROMPT_EMPTY",
            "prompt must be non-empty",
            "prompt",
            "pass a concrete task prompt so the model action can be recorded and verified",
        )


def validate_workspace(workspace: Path) -> None:
    if not workspace.exists() or not workspace.is_dir():
        raise HarnessError(
            "GEMMA_HARNESS_WORKSPACE_NOT_FOUND",
            "workspace must exist and be a directory",
            "workspace",
            "create the workspace directory explicitly and rerun",
            source_of_truth=file_source_of_truth(workspace),
        )


def resolve_target(workspace: Path, target: str) -> Path:
    target_path = Path(target)
    if target_path.is_absolute() or not target or target_path.name in {"", ".", ".."}:
        raise HarnessError(
            "GEMMA_HARNESS_TARGET_INVALID",
            "target must be a non-empty relative file path",
            "target",
            "pass a relative path under --workspace",
        )
    if any(part == ".." for part in target_path.parts):
        raise HarnessError(
            "GEMMA_HARNESS_TARGET_TRAVERSAL",
            "target must not contain '..'",
            "target",
            "write only to a relative path inside the declared workspace source of truth",
        )
    resolved = (workspace / target_path).resolve()
    workspace_resolved = workspace.resolve()
    if workspace_resolved != resolved and workspace_resolved not in resolved.parents:
        raise HarnessError(
            "GEMMA_HARNESS_TARGET_OUTSIDE_WORKSPACE",
            "target resolved outside the workspace",
            "target",
            "choose a path inside the workspace source of truth",
            source_of_truth=file_source_of_truth(workspace),
            details={"resolved_target": str(resolved)},
        )
    if not resolved.parent.exists() or not resolved.parent.is_dir():
        raise HarnessError(
            "GEMMA_HARNESS_TARGET_PARENT_NOT_FOUND",
            "target parent directory does not exist",
            "target",
            "create the parent directory explicitly and rerun",
            source_of_truth=file_source_of_truth(resolved.parent),
        )
    return resolved


def ensure_target_available(target_path: Path, force_target: bool) -> None:
    if target_path.exists() and not force_target:
        raise HarnessError(
            "GEMMA_HARNESS_TARGET_EXISTS",
            "target file already exists and --force-target was not set",
            "target",
            "choose a new target path or pass --force-target after confirming replacement is intended",
            source_of_truth=file_source_of_truth(target_path),
        )


def ledger_state(ledger: Path) -> dict[str, Any]:
    if not ledger.exists():
        raise HarnessError(
            "GEMMA_HARNESS_LEDGER_NOT_FOUND",
            "ledger file does not exist",
            "ledger",
            "run with --init-ledger or pass an existing initialized BlackBoxFix ledger",
            source_of_truth=ledger_source_of_truth(ledger),
        )
    try:
        conn = sqlite3.connect(str(ledger))
        try:
            count = conn.execute("SELECT COUNT(*) FROM ledger_records").fetchone()[0]
            counts = {
                row[0]: row[1]
                for row in conn.execute(
                    "SELECT record_kind, COUNT(*) FROM ledger_records GROUP BY record_kind ORDER BY record_kind"
                ).fetchall()
            }
            tip_row = conn.execute(
                "SELECT value FROM blackbox_metadata WHERE key = 'ledger_tip_sha256'"
            ).fetchone()
        finally:
            conn.close()
    except sqlite3.Error as err:
        raise HarnessError(
            "GEMMA_HARNESS_LEDGER_READ_FAILED",
            f"failed to read physical ledger state: {err}",
            "ledger",
            "inspect ledger initialization/integrity and rerun",
            source_of_truth=ledger_source_of_truth(ledger),
        ) from err
    state: dict[str, Any] = {
        "ledger_record_count": int(count),
        "record_kind_counts": counts,
    }
    if tip_row and tip_row[0]:
        state["ledger_tip_sha256"] = tip_row[0]
    return state


def delta(before: dict[str, Any], after: dict[str, Any]) -> dict[str, Any]:
    before_counts = before.get("record_kind_counts", {})
    after_counts = after.get("record_kind_counts", {})
    keys = sorted(set(before_counts) | set(after_counts))
    kind_delta = {
        key: int(after_counts.get(key, 0)) - int(before_counts.get(key, 0))
        for key in keys
        if int(after_counts.get(key, 0)) - int(before_counts.get(key, 0)) != 0
    }
    return {
        "ledger_record_count": int(after["ledger_record_count"]) - int(before["ledger_record_count"]),
        "record_kind_counts": kind_delta,
    }


def zero_delta() -> dict[str, Any]:
    return {"ledger_record_count": 0, "record_kind_counts": {}}


def read_payload_by_sdk_id(ledger: Path, event_id: str) -> dict[str, Any]:
    try:
        conn = sqlite3.connect(str(ledger))
        try:
            row = conn.execute(
                """
                SELECT payload_json
                FROM ledger_records
                WHERE json_extract(payload_json, '$.sdk_event.id') = ?
                ORDER BY sequence DESC
                LIMIT 1
                """,
                (event_id,),
            ).fetchone()
        finally:
            conn.close()
    except sqlite3.Error as err:
        raise HarnessError(
            "GEMMA_HARNESS_LEDGER_EVENT_READ_FAILED",
            f"failed to read SDK event payload from ledger: {err}",
            "ledger_records.payload_json",
            "inspect the SQLite ledger and rerun once JSON1 readback works",
            source_of_truth=ledger_source_of_truth(ledger),
        ) from err
    if row is None:
        raise HarnessError(
            "GEMMA_HARNESS_LEDGER_EVENT_NOT_FOUND",
            f"SDK event id {event_id!r} was not found in ledger readback",
            "ledger_records.payload_json.sdk_event.id",
            "inspect SDK ingest output and rerun if the ledger write did not persist",
            source_of_truth=ledger_source_of_truth(ledger),
        )
    return json.loads(row[0])


def latest_payload(ledger: Path, record_kind: str, run_id: str | None = None) -> dict[str, Any]:
    query = "SELECT payload_json FROM ledger_records WHERE record_kind = ?"
    params: tuple[Any, ...]
    if run_id is None:
        query += " ORDER BY sequence DESC LIMIT 1"
        params = (record_kind,)
    else:
        query += " AND run_id = ? ORDER BY sequence DESC LIMIT 1"
        params = (record_kind, run_id)
    try:
        conn = sqlite3.connect(str(ledger))
        try:
            row = conn.execute(query, params).fetchone()
        finally:
            conn.close()
    except sqlite3.Error as err:
        raise HarnessError(
            "GEMMA_HARNESS_LEDGER_PAYLOAD_READ_FAILED",
            f"failed to read latest {record_kind} payload: {err}",
            "ledger_records.payload_json",
            "inspect the SQLite ledger and rerun",
            source_of_truth=ledger_source_of_truth(ledger),
        ) from err
    if row is None:
        raise HarnessError(
            "GEMMA_HARNESS_LEDGER_RECORD_NOT_FOUND",
            f"no {record_kind} row exists for run readback",
            "ledger_records.record_kind",
            "rerun after the BlackBoxFix command has durably written the expected row",
            source_of_truth=ledger_source_of_truth(ledger),
            details={"record_kind": record_kind, "run_id": run_id},
        )
    return json.loads(row[0])


def count_kind_for_run(ledger: Path, run_id: str) -> dict[str, int]:
    try:
        conn = sqlite3.connect(str(ledger))
        try:
            rows = conn.execute(
                """
                SELECT record_kind, COUNT(*)
                FROM ledger_records
                WHERE run_id = ?
                GROUP BY record_kind
                ORDER BY record_kind
                """,
                (run_id,),
            ).fetchall()
        finally:
            conn.close()
    except sqlite3.Error as err:
        raise HarnessError(
            "GEMMA_HARNESS_LEDGER_RUN_COUNTS_FAILED",
            f"failed to count run rows from ledger: {err}",
            "ledger_records.run_id",
            "inspect the SQLite ledger and rerun",
            source_of_truth=ledger_source_of_truth(ledger),
        ) from err
    return {row[0]: int(row[1]) for row in rows}


def inspect_file(path: Path) -> dict[str, Any]:
    if not path.exists() or not path.is_file():
        raise HarnessError(
            "GEMMA_HARNESS_TARGET_READBACK_MISSING",
            "target file does not exist during source-of-truth readback",
            "target",
            "inspect the filesystem write and rerun",
            source_of_truth=file_source_of_truth(path),
        )
    data = path.read_bytes()
    try:
        utf8 = data.decode("utf-8")
    except UnicodeDecodeError:
        utf8 = None
    return {
        "path": str(path),
        "source_of_truth": file_source_of_truth(path),
        "exists": True,
        "size_bytes": len(data),
        "sha256": sha256_bytes(data),
        "utf8": utf8,
    }


def cloud_event(event_id: str, source: str, event_type: str, traceparent: str, data: dict[str, Any]) -> dict[str, Any]:
    return {
        "specversion": "1.0",
        "id": event_id,
        "source": source,
        "type": event_type,
        "datacontenttype": "application/json",
        "subject": "blackboxfix-gemma-runtime",
        "time": now_rfc3339(),
        "traceparent": traceparent,
        "data": data,
    }


def traceparent() -> str:
    trace_id = secrets.token_hex(16)
    parent_id = secrets.token_hex(8)
    while int(trace_id, 16) == 0:
        trace_id = secrets.token_hex(16)
    while int(parent_id, 16) == 0:
        parent_id = secrets.token_hex(8)
    return f"00-{trace_id}-{parent_id}-01"


def event_source() -> str:
    return f"urn:contextgraph:blackboxfix:gemma-harness:{secrets.token_hex(12)}"


def ingest_event(args: argparse.Namespace, event: dict[str, Any], workspace: Path, name: str) -> dict[str, Any]:
    path = workspace / f"{name}.cloudevent.json"
    write_json_file(path, event)
    return run_blackbox(
        args,
        ["sdk", "ingest", "--ledger", str(args.ledger), "--input", str(path)],
        timeout=args.timeout_seconds,
    )


def ingest_event_batch(args: argparse.Namespace, events: list[dict[str, Any]], workspace: Path, name: str) -> dict[str, Any]:
    path = workspace / f"{name}.cloudevents.json"
    write_json_file(path, events)
    return run_blackbox(
        args,
        ["sdk", "ingest", "--ledger", str(args.ledger), "--input", str(path)],
        timeout=args.timeout_seconds,
    )


def call_gemma(args: argparse.Namespace, prompt: str) -> tuple[dict[str, Any], dict[str, Any]]:
    system = (
        "You are a BlackBoxFix demo agent. Return only a JSON object matching the provided schema. "
        "The file_content field must be concise plain text that satisfies the user's task. "
        "Do not include markdown fences or explanatory text outside JSON."
    )
    body = {
        "model": args.model,
        "prompt": prompt,
        "system": system,
        "stream": False,
        "format": MODEL_OUTPUT_SCHEMA,
        "keep_alive": args.keep_alive,
        "think": False,
        "options": {
            "temperature": args.temperature,
            "num_predict": args.max_tokens,
        },
    }
    response = http_json(
        f"{args.ollama_url.rstrip('/')}/api/generate",
        body,
        timeout=args.timeout_seconds,
    )
    raw_text = response.get("response")
    if not isinstance(raw_text, str) or not raw_text.strip():
        raise HarnessError(
            "GEMMA_HARNESS_MODEL_RESPONSE_EMPTY",
            "Ollama generate response did not contain non-empty text",
            "ollama.response",
            "inspect model output and rerun only when the model emits the required structured response",
            source_of_truth=f"{args.ollama_url.rstrip()}/api/generate",
            details={"response": response},
        )
    try:
        parsed = json.loads(raw_text)
    except json.JSONDecodeError as err:
        raise HarnessError(
            "GEMMA_HARNESS_MODEL_RESPONSE_JSON_INVALID",
            f"Gemma response was not valid JSON: {err}",
            "ollama.response",
            "tighten the prompt/schema or inspect the model runtime; do not record unverifiable output as success",
            source_of_truth=f"{args.ollama_url.rstrip()}/api/generate",
            details={"raw_response": raw_text},
        ) from err
    validate_model_output(parsed)
    return parsed, response


def validate_model_output(value: Any) -> None:
    if not isinstance(value, dict):
        raise HarnessError(
            "GEMMA_HARNESS_MODEL_RESPONSE_SHAPE_INVALID",
            "Gemma structured response must be a JSON object",
            "ollama.response",
            "rerun only after the model emits the exact object schema",
        )
    unexpected = sorted(set(value) - {"summary", "file_content", "confidence"})
    if unexpected:
        raise HarnessError(
            "GEMMA_HARNESS_MODEL_RESPONSE_ADDITIONAL_FIELDS",
            "Gemma response contained fields outside the allowed schema",
            "ollama.response",
            "remove extra fields from the structured model response before recording it",
            details={"unexpected_fields": unexpected},
        )
    summary = value.get("summary")
    file_content = value.get("file_content")
    confidence = value.get("confidence")
    if not isinstance(summary, str) or not summary.strip():
        raise HarnessError(
            "GEMMA_HARNESS_MODEL_SUMMARY_INVALID",
            "Gemma summary must be a non-empty string",
            "ollama.response.summary",
            "rerun only after the model emits a non-empty summary",
        )
    if len(summary.encode("utf-8")) > 512:
        raise HarnessError(
            "GEMMA_HARNESS_MODEL_SUMMARY_TOO_LARGE",
            "Gemma summary exceeded 512 UTF-8 bytes",
            "ollama.response.summary",
            "ask the model for a concise summary and rerun before recording the action",
        )
    if not isinstance(file_content, str) or not file_content.strip():
        raise HarnessError(
            "GEMMA_HARNESS_MODEL_FILE_CONTENT_INVALID",
            "Gemma file_content must be a non-empty string",
            "ollama.response.file_content",
            "rerun only after the model emits durable file content",
        )
    if len(file_content.encode("utf-8")) > 20000:
        raise HarnessError(
            "GEMMA_HARNESS_MODEL_FILE_CONTENT_TOO_LARGE",
            "Gemma file_content exceeded 20000 UTF-8 bytes",
            "ollama.response.file_content",
            "reduce --max-tokens or ask for a smaller artifact before recording the action",
        )
    if not isinstance(confidence, (int, float)) or isinstance(confidence, bool) or not (0.0 <= float(confidence) <= 1.0):
        raise HarnessError(
            "GEMMA_HARNESS_MODEL_CONFIDENCE_INVALID",
            "Gemma confidence must be numeric in [0,1]",
            "ollama.response.confidence",
            "rerun only after the model emits a valid confidence score",
        )


def verify_file_with_blackbox(
    args: argparse.Namespace,
    workspace: Path,
    target_rel: str,
    target_readback: dict[str, Any],
    run_id: str,
    tool_call_id: str,
    artifacts_dir: Path,
) -> dict[str, Any]:
    check_code = (
        "import hashlib,json,pathlib,sys;"
        "p=pathlib.Path(sys.argv[1]);"
        "expected_sha=sys.argv[2];"
        "expected_bytes=int(sys.argv[3]);"
        "data=p.read_bytes();"
        "actual='sha256:'+hashlib.sha256(data).hexdigest();"
        "ok=(actual==expected_sha and len(data)==expected_bytes);"
        "print(json.dumps({'path':str(p),'sha256':actual,'bytes':len(data),'expected_sha256':expected_sha,'expected_bytes':expected_bytes,'ok':ok},sort_keys=True));"
        "sys.exit(0 if ok else 41)"
    )
    verifier_spec = {
        "verifier_id": "cmd_exit_zero_v1",
        "run_id": run_id,
        "tool_call_id": tool_call_id,
        "source": {"cwd": str(workspace)},
        "expected": {
            "command": [
                "python3",
                "-c",
                check_code,
                target_rel,
                target_readback["sha256"],
                str(target_readback["size_bytes"]),
            ],
            "artifacts": [target_rel],
        },
    }
    spec_path = artifacts_dir / "file-verifier-spec.json"
    write_json_file(spec_path, verifier_spec)
    return run_blackbox(
        args,
        [
            "verify",
            "--ledger",
            str(args.ledger),
            "--input",
            str(spec_path),
            "--tenant",
            args.tenant,
            "--agent",
            f"{args.agent}_verifier",
            "--model",
            args.model,
        ],
        timeout=args.timeout_seconds,
    )


def artifact_readback(path: Path) -> dict[str, Any]:
    if not path.exists():
        raise HarnessError(
            "GEMMA_HARNESS_ARTIFACT_MISSING",
            "expected artifact does not exist",
            str(path),
            "inspect the export command and rerun after it writes the declared artifact",
            source_of_truth=file_source_of_truth(path),
        )
    stat = path.stat()
    return {
        "path": str(path),
        "source_of_truth": file_source_of_truth(path),
        "sha256": sha256_file(path),
        "bytes": stat.st_size,
    }


def export_artifacts(args: argparse.Namespace, run_id: str, artifacts_dir: Path) -> dict[str, Any]:
    evidence_path = artifacts_dir / "evidence.json"
    replay_path = artifacts_dir / "replay.json"
    score_path = artifacts_dir / "scorecard.json"
    inspect_path = artifacts_dir / "inspect-ledger.json"
    episodes_dir = artifacts_dir / "episodes"
    otel_dir = artifacts_dir / "otel"
    dashboard_dir = artifacts_dir / "dashboard"

    score = run_blackbox(
        args,
        [
            "score",
            "run",
            "--ledger",
            str(args.ledger),
            "--run-id",
            run_id,
            "--tenant",
            args.tenant,
            "--agent",
            f"{args.agent}_scorer",
            "--model",
            args.model,
        ],
        timeout=args.timeout_seconds,
    )
    write_json_file(score_path, score)

    evidence = run_blackbox(
        args,
        ["export", "evidence", "--ledger", str(args.ledger), "--run-id", run_id, "--out", str(evidence_path)],
        timeout=args.timeout_seconds,
    )
    replay = run_blackbox(
        args,
        ["replay", "--ledger", str(args.ledger), "--run-id", run_id],
        timeout=args.timeout_seconds,
    )
    write_json_file(replay_path, replay)
    episodes = run_blackbox(
        args,
        ["export", "episodes", "--ledger", str(args.ledger), "--out", str(episodes_dir), "--force"],
        timeout=args.timeout_seconds,
    )
    otel = run_blackbox(
        args,
        ["export", "otel", "--ledger", str(args.ledger), "--out", str(otel_dir), "--force"],
        timeout=args.timeout_seconds,
    )
    dashboard = run_blackbox(
        args,
        ["dashboard", "build", "--ledger", str(args.ledger), "--out", str(dashboard_dir), "--force"],
        timeout=args.timeout_seconds,
    )
    inspect = run_blackbox(
        args,
        ["inspect", "ledger", "--ledger", str(args.ledger), "--run-id", run_id],
        timeout=args.timeout_seconds,
    )
    write_json_file(inspect_path, inspect)

    expected_files = {
        "evidence": evidence_path,
        "replay": replay_path,
        "scorecard": score_path,
        "inspect_ledger": inspect_path,
        "episodes_manifest": episodes_dir / "manifest.json",
        "otel_manifest": otel_dir / "manifest.json",
        "dashboard_manifest": dashboard_dir / "manifest.json",
        "dashboard_index": dashboard_dir / "index.html",
    }
    readbacks = {name: artifact_readback(path) for name, path in expected_files.items()}
    return {
        "commands": {
            "score": score,
            "evidence": evidence,
            "replay": replay,
            "episodes": episodes,
            "otel": otel,
            "dashboard": dashboard,
            "inspect": inspect,
        },
        "paths": {name: str(path) for name, path in expected_files.items()},
        "readbacks": readbacks,
    }


def simulate_edge_cases(
    args: argparse.Namespace,
    workspace: Path,
    target_path: Path,
    target_rel: str,
    post_state: dict[str, Any],
) -> list[dict[str, Any]]:
    edge_cases: list[dict[str, Any]] = []

    def edge_case(category: str, name: str, command: str, error: HarnessError, before: dict[str, Any], after: dict[str, Any]) -> dict[str, Any]:
        return {
            "category": category,
            "name": name,
            "command": command,
            "exit_code": error.exit_code,
            "error_code": error.error_code,
            "field_path": error.field_path,
            "message": error.message,
            "pre_state": before,
            "post_state": after,
            "expected_delta": zero_delta(),
            "actual_delta": delta(before, after),
        }

    before = ledger_state(args.ledger)
    try:
        ensure_ollama_model(args, f"{args.model}-definitely-missing-{secrets.token_hex(4)}")
    except HarnessError as err:
        after = ledger_state(args.ledger)
        edge_cases.append(
            edge_case(
                "missing_target_state",
                "missing Ollama model tag",
                "GET /api/tags and require absent model",
                err,
                before,
                after,
            )
        )
    else:
        raise HarnessError(
            "GEMMA_HARNESS_EDGE_CASE_DID_NOT_FAIL",
            "missing model edge case unexpectedly succeeded",
            "edge_cases[0]",
            "inspect Ollama model inventory; the synthetic impossible model name should not exist",
        )

    before = ledger_state(args.ledger)
    try:
        ensure_target_available(target_path, force_target=False)
    except HarnessError as err:
        after = ledger_state(args.ledger)
        edge_cases.append(
            edge_case(
                "wrong_target_state",
                "existing target without force",
                f"{sys.argv[0]} --target {target_rel} without --force-target",
                err,
                before,
                after,
            )
        )
    else:
        raise HarnessError(
            "GEMMA_HARNESS_EDGE_CASE_DID_NOT_FAIL",
            "existing target edge case unexpectedly succeeded",
            "edge_cases[1]",
            "inspect filesystem state; the happy path should have created the target file",
            source_of_truth=file_source_of_truth(target_path),
        )

    before = ledger_state(args.ledger)
    try:
        validate_prompt("")
    except HarnessError as err:
        after = ledger_state(args.ledger)
        edge_cases.append(
            edge_case(
                "permission_or_schema_failure",
                "empty prompt schema rejection",
                f"{sys.argv[0]} --prompt ''",
                err,
                before,
                after,
            )
        )
    else:
        raise HarnessError(
            "GEMMA_HARNESS_EDGE_CASE_DID_NOT_FAIL",
            "empty prompt edge case unexpectedly succeeded",
            "edge_cases[2]",
            "restore fail-closed prompt validation before rerunning",
        )

    for index, item in enumerate(edge_cases):
        if item["pre_state"] != item["post_state"] or item["actual_delta"] != zero_delta():
            raise HarnessError(
                "GEMMA_HARNESS_EDGE_CASE_MUTATED_LEDGER",
                "edge case mutated ledger despite failing preflight",
                f"edge_cases[{index}]",
                "fix validation order so failing edge cases happen before any ledger mutation",
                source_of_truth=ledger_source_of_truth(args.ledger),
                details=item,
            )
    if post_state != ledger_state(args.ledger):
        raise HarnessError(
            "GEMMA_HARNESS_EDGE_CASE_POST_STATE_DRIFT",
            "ledger state changed during edge case audit",
            "edge_cases",
            "inspect edge-case validation paths and remove ledger writes before failing validation",
            source_of_truth=ledger_source_of_truth(args.ledger),
            details={"expected_post_state": post_state, "actual_state": ledger_state(args.ledger)},
        )
    return edge_cases


def build_fsv_evidence(
    args: argparse.Namespace,
    run_id: str,
    pre_state: dict[str, Any],
    post_state: dict[str, Any],
    trigger_command: str,
    target_readback: dict[str, Any],
    export_readbacks: dict[str, Any],
    edge_cases: list[dict[str, Any]],
) -> dict[str, Any]:
    observed_delta = delta(pre_state, post_state)
    read_queries = [
        {
            "source_of_truth": ledger_source_of_truth(args.ledger),
            "query": "SELECT record_kind, COUNT(*) FROM ledger_records GROUP BY record_kind ORDER BY record_kind",
            "output": json.dumps(post_state["record_kind_counts"], sort_keys=True),
        },
        {
            "source_of_truth": ledger_source_of_truth(args.ledger),
            "query": "SELECT value FROM blackbox_metadata WHERE key='ledger_tip_sha256'",
            "output": post_state.get("ledger_tip_sha256", ""),
        },
        {
            "source_of_truth": target_readback["source_of_truth"],
            "query": "read target file bytes and compute SHA-256",
            "output": json.dumps(
                {
                    "sha256": target_readback["sha256"],
                    "size_bytes": target_readback["size_bytes"],
                    "exists": target_readback["exists"],
                },
                sort_keys=True,
            ),
        },
    ]
    artifacts = [
        {"path": target_readback["path"], "sha256": target_readback["sha256"], "bytes": target_readback["size_bytes"]}
    ]
    for readback in export_readbacks.values():
        artifacts.append({"path": readback["path"], "sha256": readback["sha256"], "bytes": readback["bytes"]})
    return {
        "format": "blackbox_full_state_verification_evidence_json_v1",
        "phase": args.fsv_phase,
        "tenant_id": args.tenant,
        "run_id": run_id,
        "source_of_truth": ledger_source_of_truth(args.ledger),
        "trigger": {
            "command": trigger_command,
            "exit_code": 0,
            "stdout": "Gemma artifact was written, SDK events were ingested, verifier passed, exports were read back",
            "stderr": "",
        },
        "pre_state": pre_state,
        "post_state": post_state,
        "expected_delta": observed_delta,
        "actual_delta": observed_delta,
        "read_queries": read_queries,
        "artifacts": artifacts,
        "edge_cases": edge_cases,
    }


def certify_fsv(args: argparse.Namespace, run_id: str, evidence_path: Path, fsv_dir: Path) -> dict[str, Any]:
    return run_blackbox(
        args,
        [
            "fsv",
            "certify",
            "--ledger",
            str(args.ledger),
            "--tenant",
            args.tenant,
            "--phase",
            str(args.fsv_phase),
            "--evidence",
            str(evidence_path),
            "--out",
            str(fsv_dir),
            "--force",
            "--run-id",
            run_id,
            "--agent",
            f"{args.agent}_fsv_certifier",
            "--model",
            args.model,
        ],
        timeout=args.timeout_seconds,
    )


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Run Gemma through BlackBoxFix full-state verification.")
    parser.add_argument("--ledger", required=True, type=Path, help="BlackBoxFix SQLite ledger path.")
    parser.add_argument("--workspace", required=True, type=Path, help="Existing workspace directory for source-of-truth files.")
    parser.add_argument("--target", required=True, help="Relative file path under --workspace to write from Gemma output.")
    parser.add_argument("--prompt", required=True, help="Task prompt for the Gemma model.")
    parser.add_argument("--model", default="gemma4:e4b", help="Exact Ollama model tag to use.")
    parser.add_argument("--ollama-url", default="http://127.0.0.1:11434", help="Ollama API base URL.")
    parser.add_argument("--blackbox-bin", default=Path("target/debug/context-graph"), type=Path, help="context-graph binary path.")
    parser.add_argument("--tenant", default="tenant_local", help="BlackBoxFix tenant id.")
    parser.add_argument("--agent", default="blackboxfix_gemma_harness", help="Agent identifier for ledger records.")
    parser.add_argument("--risk-tier", default="high", choices=["low", "medium", "high"], help="Risk tier for the agent_run.")
    parser.add_argument("--keep-alive", default="15m", help="Ollama model keep_alive value.")
    parser.add_argument("--temperature", default=0.0, type=float, help="Generation temperature.")
    parser.add_argument("--max-tokens", default=512, type=int, help="Maximum generated tokens.")
    parser.add_argument("--timeout-seconds", default=300.0, type=float, help="Command/API timeout.")
    parser.add_argument("--init-ledger", action="store_true", help="Initialize the ledger before running.")
    parser.add_argument("--force-target", action="store_true", help="Replace target file if it already exists.")
    parser.add_argument("--fsv-phase", default=24, type=int, help="Phase number to use for FSV certification evidence.")
    return parser.parse_args()


def initialize_or_check_ledger(args: argparse.Namespace) -> None:
    if args.init_ledger:
        run_blackbox(args, ["init", "--ledger", str(args.ledger)], timeout=args.timeout_seconds)
    else:
        if not args.ledger.exists():
            raise HarnessError(
                "GEMMA_HARNESS_LEDGER_NOT_FOUND",
                "ledger file does not exist and --init-ledger was not set",
                "ledger",
                "run with --init-ledger or pass an existing initialized BlackBoxFix ledger",
                source_of_truth=ledger_source_of_truth(args.ledger),
            )
        run_blackbox(args, ["sdk", "schema", "--ledger", str(args.ledger)], timeout=args.timeout_seconds)


def main() -> int:
    args = parse_args()
    args.ledger = args.ledger.resolve()
    args.workspace = args.workspace.resolve()
    args.blackbox_bin = args.blackbox_bin.resolve()

    validate_prompt(args.prompt)
    validate_workspace(args.workspace)
    if not args.blackbox_bin.exists():
        raise HarnessError(
            "GEMMA_HARNESS_BLACKBOX_BIN_NOT_FOUND",
            "BlackBoxFix binary path does not exist",
            "blackbox_bin",
            "build context-graph or pass --blackbox-bin to the compiled binary",
            source_of_truth=file_source_of_truth(args.blackbox_bin),
        )
    target_path = resolve_target(args.workspace, args.target)
    ensure_target_available(target_path, args.force_target)

    initialize_or_check_ledger(args)
    ensure_ollama_model(args, args.model)

    pre_state = ledger_state(args.ledger)
    source = event_source()
    trace = traceparent()
    prompt_hash = sha256_bytes(args.prompt.encode("utf-8"))
    run_event_id = f"gemma-run-{secrets.token_hex(8)}"
    run_event = cloud_event(
        run_event_id,
        source,
        EVENT_AGENT_RUN,
        trace,
        {
            "tenant": args.tenant,
            "agent": args.agent,
            "model": args.model,
            "task": f"Local Gemma verified artifact generation; prompt_sha256={prompt_hash}",
            "task_kind": "gemma-local-full-state-verification",
            "risk_tier": args.risk_tier,
            "sdk": SDK_METADATA,
        },
    )
    run_ingest = ingest_event(args, run_event, args.workspace, "gemma-run")
    run_id = run_ingest["records"][0]["identifiers"]["run_id"]
    if not isinstance(run_id, str) or not run_id:
        raise HarnessError(
            "GEMMA_HARNESS_RUN_ID_MISSING",
            "SDK ingest did not return a run_id",
            "sdk_ingest.records[0].identifiers.run_id",
            "inspect SDK ingest output and rerun after BlackBoxFix returns run identifiers",
            source_of_truth=ledger_source_of_truth(args.ledger),
            details={"run_ingest": run_ingest},
        )
    run_payload = read_payload_by_sdk_id(args.ledger, run_event_id)
    if run_payload["run_id"] != run_id:
        raise HarnessError(
            "GEMMA_HARNESS_RUN_READBACK_MISMATCH",
            "run_id returned by ingest did not match SQLite readback",
            "ledger_records.payload_json.run_id",
            "inspect ledger durability and rerun only after run readback is consistent",
            source_of_truth=ledger_source_of_truth(args.ledger),
            details={"returned_run_id": run_id, "readback_run_id": run_payload.get("run_id")},
        )

    model_output, ollama_response = call_gemma(args, args.prompt)
    content_bytes = model_output["file_content"].encode("utf-8")
    atomic_write_bytes(target_path, content_bytes)
    target_readback = inspect_file(target_path)
    if target_readback["sha256"] != sha256_bytes(content_bytes) or target_readback["size_bytes"] != len(content_bytes):
        raise HarnessError(
            "GEMMA_HARNESS_TARGET_WRITE_READBACK_MISMATCH",
            "target file readback did not match bytes written",
            "target",
            "inspect filesystem consistency and rerun after storage is reliable",
            source_of_truth=file_source_of_truth(target_path),
            details={"target_readback": target_readback},
        )

    rel_target = str(target_path.relative_to(args.workspace))
    child_base = secrets.token_hex(8)
    step_event_id = f"gemma-step-{child_base}"
    decision_event_id = f"gemma-decision-{child_base}"
    tool_event_id = f"gemma-tool-{child_base}"
    expected_delta = {
        "source_of_truth": target_readback["source_of_truth"],
        "filesystem": {
            "operation": "write_text",
            "workspace": str(args.workspace),
            "path": rel_target,
            "sha256": target_readback["sha256"],
            "size_bytes": target_readback["size_bytes"],
        },
    }
    child_events = [
        cloud_event(
            step_event_id,
            source,
            EVENT_AGENT_STEP,
            trace,
            {
                "tenant": args.tenant,
                "agent": args.agent,
                "model": args.model,
                "run_id": run_id,
                "step_kind": "model_generate_write",
                "summary": model_output["summary"],
                "context": {
                    "prompt_sha256": prompt_hash,
                    "prompt_bytes": len(args.prompt.encode("utf-8")),
                    "response_sha256": sha256_bytes(json.dumps(model_output, sort_keys=True).encode("utf-8")),
                    "target": rel_target,
                    "target_sha256": target_readback["sha256"],
                    "ollama": {
                        "model": ollama_response.get("model"),
                        "done": ollama_response.get("done"),
                        "done_reason": ollama_response.get("done_reason"),
                        "total_duration": ollama_response.get("total_duration"),
                        "load_duration": ollama_response.get("load_duration"),
                        "prompt_eval_count": ollama_response.get("prompt_eval_count"),
                        "eval_count": ollama_response.get("eval_count"),
                    },
                },
                "sdk": SDK_METADATA,
            },
        ),
        cloud_event(
            decision_event_id,
            source,
            EVENT_MODEL_DECISION,
            trace,
            {
                "tenant": args.tenant,
                "agent": args.agent,
                "model": args.model,
                "run_id": run_id,
                "tool_name": "fs.write_text",
                "capability_id": "blackboxfix.local_file.write_text",
                "confidence": float(model_output["confidence"]),
                "input": {
                    "policy_context": {
                        "mutating": True,
                        "source_of_truth": target_readback["source_of_truth"],
                        "requires_full_state_verification": True,
                    },
                    "prompt_sha256": prompt_hash,
                    "target": rel_target,
                    "content_sha256": target_readback["sha256"],
                    "content_bytes": target_readback["size_bytes"],
                },
                "sdk": SDK_METADATA,
            },
        ),
        cloud_event(
            tool_event_id,
            source,
            EVENT_TOOL_CALL,
            trace,
            {
                "tenant": args.tenant,
                "agent": args.agent,
                "model": args.model,
                "run_id": run_id,
                "tool_name": "fs.write_text",
                "capability_id": "blackboxfix.local_file.write_text",
                "input": {
                    "workspace": str(args.workspace),
                    "target": rel_target,
                    "content_sha256": target_readback["sha256"],
                    "content_bytes": target_readback["size_bytes"],
                },
                "mutating": True,
                "expected_delta": expected_delta,
                "status": "ok",
                "external_request_id": f"gemma-local-write-{child_base}",
                "sdk": SDK_METADATA,
            },
        ),
    ]
    child_ingest = ingest_event_batch(args, child_events, args.workspace, "gemma-child-events")
    records = child_ingest.get("records", [])
    tool_record = next(
        (
            item
            for item in records
            if item.get("event", {}).get("id") == tool_event_id
            and item.get("ledger", {}).get("record_kind") == "tool_call"
        ),
        None,
    )
    if not tool_record:
        raise HarnessError(
            "GEMMA_HARNESS_TOOL_CALL_READBACK_MISSING",
            "SDK child ingest did not return the tool_call readback",
            "sdk_ingest.records",
            "inspect SDK ingest output and rerun after tool_call identifiers are returned",
            source_of_truth=ledger_source_of_truth(args.ledger),
            details={"child_ingest": child_ingest},
        )
    tool_call_id = tool_record["identifiers"]["tool_call_id"]
    tool_payload = read_payload_by_sdk_id(args.ledger, tool_event_id)
    if tool_payload["tool_call_id"] != tool_call_id:
        raise HarnessError(
            "GEMMA_HARNESS_TOOL_CALL_ID_MISMATCH",
            "tool_call_id returned by ingest did not match SQLite readback",
            "ledger_records.payload_json.tool_call_id",
            "inspect ledger durability and rerun only after tool readback is consistent",
            source_of_truth=ledger_source_of_truth(args.ledger),
            details={"returned_tool_call_id": tool_call_id, "readback_tool_call_id": tool_payload.get("tool_call_id")},
        )

    artifacts_dir = args.workspace / "blackboxfix-gemma-artifacts" / run_id
    if artifacts_dir.exists():
        raise HarnessError(
            "GEMMA_HARNESS_ARTIFACT_DIR_EXISTS",
            "artifact directory already exists for run_id",
            "artifacts_dir",
            "inspect duplicate run state before rerunning",
            source_of_truth=file_source_of_truth(artifacts_dir),
        )
    artifacts_dir.mkdir(parents=True)

    verify_output = verify_file_with_blackbox(args, args.workspace, rel_target, target_readback, run_id, tool_call_id, artifacts_dir)
    verifier_payload = latest_payload(args.ledger, "verifier_result", run_id)
    if verifier_payload.get("status") != "passed":
        raise HarnessError(
            "GEMMA_HARNESS_VERIFIER_NOT_PASSED",
            "latest verifier_result did not pass",
            "ledger_records.payload_json.status",
            "inspect the verifier readback and fix the source-of-truth mismatch before claiming success",
            source_of_truth=ledger_source_of_truth(args.ledger),
            details={"verify_output": verify_output, "verifier_payload": verifier_payload},
        )

    exports = export_artifacts(args, run_id, artifacts_dir)
    post_state = ledger_state(args.ledger)
    required_delta = {
        "agent_run": 1,
        "agent_step": 1,
        "model_decision": 1,
        "tool_call": 1,
        "expected_delta": 1,
        "readback_snapshot": 1,
        "verifier_result": 1,
        "evidence_packet": 1,
        "scorecard": 1,
    }
    actual_run_counts = count_kind_for_run(args.ledger, run_id)
    missing_counts = {
        kind: expected
        for kind, expected in required_delta.items()
        if actual_run_counts.get(kind, 0) < expected
    }
    if missing_counts:
        raise HarnessError(
            "GEMMA_HARNESS_LEDGER_CAPABILITY_ROWS_MISSING",
            "run ledger readback is missing required BlackBoxFix capability rows",
            "ledger_records.record_kind",
            "inspect the run ledger and rerun after all recording, verification, and scoring rows persist",
            source_of_truth=ledger_source_of_truth(args.ledger),
            details={"run_id": run_id, "actual_run_counts": actual_run_counts, "missing_counts": missing_counts},
        )

    edge_cases = simulate_edge_cases(args, args.workspace, target_path, rel_target, post_state)
    fsv_evidence = build_fsv_evidence(
        args,
        run_id,
        pre_state,
        post_state,
        "blackboxfix_gemma_harness.py model_generate_write_verify_export",
        target_readback,
        exports["readbacks"],
        edge_cases,
    )
    evidence_path = artifacts_dir / "fsv-evidence.json"
    write_json_file(evidence_path, fsv_evidence)
    fsv_dir = artifacts_dir / "fsv-certification"
    fsv_certification = certify_fsv(args, run_id, evidence_path, fsv_dir)
    final_state = ledger_state(args.ledger)
    final_run_counts = count_kind_for_run(args.ledger, run_id)

    report = {
        "status": "ok",
        "harness": {
            "name": "blackboxfix_gemma_harness",
            "version": HARNESS_VERSION,
        },
        "model": {
            "provider": "ollama",
            "name": args.model,
            "ollama_url": args.ollama_url,
            "keep_alive": args.keep_alive,
        },
        "source_of_truth": {
            "ledger": ledger_source_of_truth(args.ledger),
            "target_file": target_readback["source_of_truth"],
            "ollama_tags": f"{args.ollama_url.rstrip()}/api/tags",
            "ollama_generate": f"{args.ollama_url.rstrip()}/api/generate",
            "artifacts_dir": file_source_of_truth(artifacts_dir),
        },
        "run_id": run_id,
        "tool_call_id": tool_call_id,
        "traceparent": trace,
        "prompt_sha256": prompt_hash,
        "model_output_summary": model_output["summary"],
        "target_readback": target_readback,
        "ledger": {
            "pre_state": pre_state,
            "post_state_before_fsv": post_state,
            "final_state": final_state,
            "run_counts_before_fsv": actual_run_counts,
            "run_counts_final": final_run_counts,
        },
        "blackbox_outputs": {
            "run_ingest": run_ingest,
            "child_ingest": child_ingest,
            "verify": verify_output,
            "exports": exports["commands"],
            "fsv_certification": fsv_certification,
        },
        "edge_cases": edge_cases,
        "artifacts": {
            "directory": str(artifacts_dir),
            "fsv_evidence": artifact_readback(evidence_path),
            "fsv_manifest": artifact_readback(fsv_dir / "manifest.json"),
            "exports": exports["readbacks"],
        },
    }
    report_path = artifacts_dir / "harness-report.json"
    write_json_file(report_path, report)
    report["artifacts"]["harness_report"] = artifact_readback(report_path)
    print_json(report)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except HarnessError as err:
        print_json(err.to_json(), stream=sys.stderr)
        raise SystemExit(err.exit_code)
    except KeyboardInterrupt:
        print_json(
            {
                "status": "error",
                "error_code": "GEMMA_HARNESS_INTERRUPTED",
                "message": "harness was interrupted",
                "field_path": "process",
                "remediation": "rerun only after inspecting partially written source-of-truth state",
                "source_of_truth": None,
                "details": {},
            },
            stream=sys.stderr,
        )
        raise SystemExit(130)
