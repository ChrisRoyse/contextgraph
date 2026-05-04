#!/usr/bin/env python3
"""Run the full Gemma + BlackBoxFix AgentOps research demo.

The demo uses a local Ollama Gemma model to request one allowlisted tool. That
tool runs the real DynamicJEPA counter_world pipeline:

- data ingest into RocksDB;
- panel materialization;
- trajectory and action-conditioned dataset compilation;
- CUDA tiny training and artifact registry;
- prediction, planning, guard decisions, and surprise recording;
- DynamicJEPA episode export;
- provenance artifact creation;
- BlackBoxFix ledger recording, verification, export, scoring, dashboard, and
  FSV certification.

No mock connectors are used. Every success claim is followed by a separate
source-of-truth read from SQLite, RocksDB-facing CLI reads, and file hashes.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import secrets
import shutil
import sqlite3
import subprocess
import sys
import time
import urllib.error
import urllib.request
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


DEMO_VERSION = "0.2.0"
EVENT_AGENT_RUN = "dev.contextgraph.blackbox.agent_run.v1"
EVENT_AGENT_STEP = "dev.contextgraph.blackbox.agent_step.v1"
EVENT_MODEL_DECISION = "dev.contextgraph.blackbox.model_decision.v1"
EVENT_TOOL_CALL = "dev.contextgraph.blackbox.tool_call.v1"
SDK_METADATA = {"name": "blackboxfix-gemma-agentops-demo", "version": DEMO_VERSION}
SCENARIO = "counter_world_full_agentops"
DOMAIN = "counter_world"
REQUIRED_CAPABILITIES = [
    "data_ingest",
    "panel_materialization",
    "action_conditioned_training",
    "planning",
    "surprise_recording",
    "artifact_provenance",
    "blackboxfix_fsv",
]
IMPORTANT_DJ_CFS = [
    "dj_raw_events",
    "dj_adapter_runs",
    "dj_normalized_states",
    "dj_actions",
    "dj_outcomes",
    "dj_transitions",
    "dj_instrument_readings",
    "dj_pairwise_readings",
    "dj_latent_panels",
    "dj_trajectories",
    "dj_dataset_shards",
    "dj_training_runs",
    "dj_model_artifacts",
    "dj_predictions",
    "dj_skill_policies",
    "dj_guard_decisions",
    "dj_plan_traces",
    "dj_surprise_events",
    "dj_verification_runs",
    "dj_audit_log",
]


class DemoError(Exception):
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
            "source_of_truth": self.source_of_truth,
            "remediation": self.remediation,
            "details": self.details,
        }


@dataclass
class CommandResult:
    args: list[str]
    stdout: str
    stderr: str
    exit_code: int

    def json_value(self) -> dict[str, Any]:
        raw = self.stdout.strip() or self.stderr.strip()
        try:
            value = json.loads(raw)
        except json.JSONDecodeError as err:
            raise DemoError(
                "DEMO_COMMAND_JSON_INVALID",
                f"command did not emit a JSON object: {err}",
                "command.stdout",
                "inspect command stdout/stderr and fix the command before rerunning the demo",
                details={"command": self.args, "stdout": self.stdout, "stderr": self.stderr},
            ) from err
        if not isinstance(value, dict):
            raise DemoError(
                "DEMO_COMMAND_JSON_SHAPE_INVALID",
                "command JSON output was not an object",
                "command.stdout",
                "fix the command contract so it emits one JSON object",
                details={"command": self.args, "output": value},
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
        raise DemoError(
            "DEMO_FILE_READ_FAILED",
            f"failed to read file for SHA-256: {err}",
            str(path),
            "fix filesystem permissions and rerun the source-of-truth read",
            source_of_truth=file_sot(path),
        ) from err


def redacted_preview(value: str, *, limit: int = 180) -> str:
    redacted = re.sub(r"(?i)bearer\s+[A-Za-z0-9._~+/=-]+", "Bearer [REDACTED]", value)
    redacted = re.sub(r"(?i)(api[_-]?key|token|password|secret)\s*[:=]\s*['\"]?[^\s,'\"]+", r"\1=[REDACTED]", redacted)
    redacted = re.sub(r"\b(?:sk|ghp|github_pat|glpat)-[A-Za-z0-9_-]{12,}\b", "[REDACTED_TOKEN]", redacted)
    redacted = " ".join(redacted.split())
    if len(redacted) > limit:
        return redacted[: limit - 3] + "..."
    return redacted


def file_sot(path: Path) -> str:
    return f"file:{path.resolve()}"


def ledger_sot(ledger: Path) -> str:
    return f"sqlite:{ledger.resolve()}#ledger_records"


def rocksdb_sot(db: Path, suffix: str = "dynamicjepa") -> str:
    return f"rocksdb:{db.resolve()}#{suffix}"


def atomic_write(path: Path, data: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_name(f".{path.name}.tmp-{os.getpid()}-{secrets.token_hex(6)}")
    fd: int | None = None
    try:
        fd = os.open(tmp, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        with os.fdopen(fd, "wb") as handle:
            fd = None
            handle.write(data)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(tmp, path)
        dir_fd = os.open(path.parent, os.O_RDONLY)
        try:
            os.fsync(dir_fd)
        finally:
            os.close(dir_fd)
    except OSError as err:
        try:
            tmp.unlink(missing_ok=True)
        except OSError:
            pass
        raise DemoError(
            "DEMO_ATOMIC_WRITE_FAILED",
            f"failed to atomically write {path}: {err}",
            str(path),
            "check filesystem permissions, free space, and parent directory state",
            source_of_truth=file_sot(path),
        ) from err
    finally:
        if fd is not None:
            os.close(fd)


def write_json(path: Path, value: Any) -> None:
    atomic_write(path, json.dumps(value, indent=2, sort_keys=True).encode("utf-8") + b"\n")


def read_json(path: Path) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except OSError as err:
        raise DemoError(
            "DEMO_JSON_FILE_READ_FAILED",
            f"failed to read JSON file: {err}",
            str(path),
            "fix filesystem access and rerun the readback",
            source_of_truth=file_sot(path),
        ) from err
    except json.JSONDecodeError as err:
        raise DemoError(
            "DEMO_JSON_FILE_INVALID",
            f"failed to parse JSON file: {err}",
            str(path),
            "fix the JSON artifact producer and rerun from a clean demo root",
            source_of_truth=file_sot(path),
        ) from err


def artifact_readback(path: Path) -> dict[str, Any]:
    if not path.exists():
        raise DemoError(
            "DEMO_ARTIFACT_MISSING",
            "expected artifact is missing from disk",
            str(path),
            "rerun only after the producer writes this artifact",
            source_of_truth=file_sot(path),
        )
    return {
        "path": str(path),
        "source_of_truth": file_sot(path),
        "sha256": sha256_file(path),
        "bytes": path.stat().st_size,
    }


def run_process(args: list[str], *, cwd: Path | None = None, timeout: float = 300.0) -> CommandResult:
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
        raise DemoError(
            "DEMO_COMMAND_TIMEOUT",
            f"command timed out after {timeout}s",
            "command",
            "increase timeout only after confirming the source-of-truth operation is expected to take longer",
            details={"command": args, "timeout_seconds": timeout},
        ) from err
    except OSError as err:
        raise DemoError(
            "DEMO_COMMAND_EXEC_FAILED",
            f"failed to execute command: {err}",
            "command[0]",
            "install the executable or pass the correct binary path",
            details={"command": args},
        ) from err
    return CommandResult(args=args, stdout=proc.stdout, stderr=proc.stderr, exit_code=proc.returncode)


def run_json(args: list[str], *, timeout: float, expect_success: bool = True) -> dict[str, Any]:
    result = run_process(args, timeout=timeout)
    value = result.json_value()
    if expect_success and result.exit_code != 0:
        raise DemoError(
            "DEMO_COMMAND_FAILED",
            "command exited non-zero",
            "command",
            "inspect the typed error and fix the source-of-truth mismatch before rerunning",
            details={"command": args, "exit_code": result.exit_code, "output": value},
            exit_code=result.exit_code,
        )
    if not expect_success and result.exit_code == 0:
        raise DemoError(
            "DEMO_EDGE_CASE_DID_NOT_FAIL",
            "edge-case command unexpectedly succeeded",
            "command",
            "tighten validation so the invalid edge case fails closed",
            details={"command": args, "output": value},
        )
    value["_process"] = {"exit_code": result.exit_code}
    return value


def http_json(url: str, body: dict[str, Any] | None = None, *, timeout: float = 300.0) -> dict[str, Any]:
    data = None if body is None else json.dumps(body).encode("utf-8")
    request = urllib.request.Request(
        url,
        data=data,
        headers={"Content-Type": "application/json"},
        method="GET" if data is None else "POST",
    )
    try:
        with urllib.request.urlopen(request, timeout=timeout) as response:
            raw = response.read()
    except urllib.error.HTTPError as err:
        body_text = err.read().decode("utf-8", errors="replace")
        parsed: Any = body_text
        try:
            parsed = json.loads(body_text)
        except json.JSONDecodeError:
            pass
        raise DemoError(
            "DEMO_OLLAMA_HTTP_ERROR",
            f"Ollama returned HTTP {err.code}",
            "ollama_response.status",
            "inspect the Ollama error body and fix the model/runtime issue",
            source_of_truth=url,
            details={"http_status": err.code, "body": parsed},
        ) from err
    except urllib.error.URLError as err:
        raise DemoError(
            "DEMO_OLLAMA_UNREACHABLE",
            f"Ollama API request failed: {err}",
            "ollama_url",
            "start Ollama and verify the local API URL",
            source_of_truth=url,
        ) from err
    try:
        value = json.loads(raw)
    except json.JSONDecodeError as err:
        raise DemoError(
            "DEMO_OLLAMA_JSON_INVALID",
            f"Ollama response was not valid JSON: {err}",
            "ollama_response",
            "inspect the Ollama API response and rerun after it emits valid JSON",
            source_of_truth=url,
            details={"raw": raw.decode("utf-8", errors="replace")},
        ) from err
    if not isinstance(value, dict):
        raise DemoError(
            "DEMO_OLLAMA_SHAPE_INVALID",
            "Ollama response JSON was not an object",
            "ollama_response",
            "inspect the Ollama API endpoint compatibility",
            source_of_truth=url,
        )
    return value


def ensure_ollama_model(ollama_url: str, model: str, timeout: float) -> dict[str, Any]:
    tags = http_json(f"{ollama_url.rstrip('/')}/api/tags", timeout=timeout)
    models = tags.get("models")
    if not isinstance(models, list):
        raise DemoError(
            "DEMO_OLLAMA_TAGS_INVALID",
            "Ollama /api/tags did not contain a models array",
            "ollama.tags.models",
            "inspect Ollama API compatibility",
            source_of_truth=f"{ollama_url.rstrip('/')}/api/tags",
            details={"response": tags},
        )
    names = sorted(item.get("name") for item in models if isinstance(item, dict) and item.get("name"))
    if model not in names:
        raise DemoError(
            "DEMO_MODEL_NOT_FOUND",
            f"required model {model!r} is not present in Ollama",
            "model",
            "pull the exact model tag or pass a model already present in Ollama",
            source_of_truth=f"{ollama_url.rstrip('/')}/api/tags",
            details={"available_models": names},
        )
    return tags


def ledger_state(ledger: Path) -> dict[str, Any]:
    if not ledger.exists():
        return {"ledger_record_count": 0, "record_kind_counts": {}}
    try:
        conn = sqlite3.connect(str(ledger))
        try:
            count = conn.execute("SELECT COUNT(*) FROM ledger_records").fetchone()[0]
            counts = {
                row[0]: int(row[1])
                for row in conn.execute(
                    "SELECT record_kind, COUNT(*) FROM ledger_records GROUP BY record_kind ORDER BY record_kind"
                ).fetchall()
            }
            tip = conn.execute(
                "SELECT value FROM blackbox_metadata WHERE key='ledger_tip_sha256'"
            ).fetchone()
        finally:
            conn.close()
    except sqlite3.Error as err:
        raise DemoError(
            "DEMO_SQLITE_READBACK_FAILED",
            f"failed to read BlackBoxFix ledger state: {err}",
            "ledger_records",
            "inspect the SQLite ledger integrity and rerun after fixing it",
            source_of_truth=ledger_sot(ledger),
        ) from err
    state: dict[str, Any] = {"ledger_record_count": int(count), "record_kind_counts": counts}
    if tip and tip[0]:
        state["ledger_tip_sha256"] = tip[0]
    return state


def ledger_delta(before: dict[str, Any], after: dict[str, Any]) -> dict[str, Any]:
    before_counts = before.get("record_kind_counts", {})
    after_counts = after.get("record_kind_counts", {})
    keys = sorted(set(before_counts) | set(after_counts))
    return {
        "ledger_record_count": int(after["ledger_record_count"]) - int(before["ledger_record_count"]),
        "record_kind_counts": {
            key: int(after_counts.get(key, 0)) - int(before_counts.get(key, 0))
            for key in keys
            if int(after_counts.get(key, 0)) - int(before_counts.get(key, 0)) != 0
        },
    }


def zero_ledger_delta() -> dict[str, Any]:
    return {"ledger_record_count": 0, "record_kind_counts": {}}


def latest_payload(ledger: Path, record_kind: str, run_id: str | None = None) -> dict[str, Any]:
    query = "SELECT payload_json FROM ledger_records WHERE record_kind = ?"
    params: tuple[Any, ...]
    if run_id:
        query += " AND run_id = ? ORDER BY sequence DESC LIMIT 1"
        params = (record_kind, run_id)
    else:
        query += " ORDER BY sequence DESC LIMIT 1"
        params = (record_kind,)
    try:
        conn = sqlite3.connect(str(ledger))
        try:
            row = conn.execute(query, params).fetchone()
        finally:
            conn.close()
    except sqlite3.Error as err:
        raise DemoError(
            "DEMO_SQLITE_PAYLOAD_READ_FAILED",
            f"failed to read latest {record_kind}: {err}",
            "ledger_records.payload_json",
            "inspect SQLite JSON1/readback support",
            source_of_truth=ledger_sot(ledger),
        ) from err
    if row is None:
        raise DemoError(
            "DEMO_LEDGER_RECORD_NOT_FOUND",
            f"no {record_kind} row exists",
            "ledger_records.record_kind",
            "rerun after the expected ledger row is durably appended",
            source_of_truth=ledger_sot(ledger),
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
        raise DemoError(
            "DEMO_SQLITE_RUN_COUNTS_FAILED",
            f"failed to count records for run: {err}",
            "ledger_records.run_id",
            "inspect SQLite ledger state",
            source_of_truth=ledger_sot(ledger),
        ) from err
    return {row[0]: int(row[1]) for row in rows}


def traceparent() -> str:
    trace_id = secrets.token_hex(16)
    parent_id = secrets.token_hex(8)
    while int(trace_id, 16) == 0:
        trace_id = secrets.token_hex(16)
    while int(parent_id, 16) == 0:
        parent_id = secrets.token_hex(8)
    return f"00-{trace_id}-{parent_id}-01"


def cloud_event(event_id: str, source: str, event_type: str, trace: str, data: dict[str, Any]) -> dict[str, Any]:
    return {
        "specversion": "1.0",
        "id": event_id,
        "source": source,
        "type": event_type,
        "datacontenttype": "application/json",
        "subject": "blackboxfix-gemma-agentops-demo",
        "time": now_rfc3339(),
        "traceparent": trace,
        "data": data,
    }


def run_blackbox(args: argparse.Namespace, command: list[str], *, timeout: float | None = None) -> dict[str, Any]:
    return run_json(
        [str(args.blackbox_bin), "blackbox", *command],
        timeout=args.timeout_seconds if timeout is None else timeout,
    )


def run_dynamicjepa(args: argparse.Namespace, command: list[str], *, timeout: float | None = None) -> dict[str, Any]:
    return run_json(
        [str(args.blackbox_bin), "dynamicjepa", *command],
        timeout=args.timeout_seconds if timeout is None else timeout,
    )


def run_export(args: argparse.Namespace, command: list[str], *, timeout: float | None = None) -> dict[str, Any]:
    return run_json(
        [str(args.blackbox_bin), "export", *command],
        timeout=args.timeout_seconds if timeout is None else timeout,
    )


def ingest_event(args: argparse.Namespace, event: dict[str, Any], event_dir: Path, name: str) -> dict[str, Any]:
    path = event_dir / f"{name}.cloudevent.json"
    write_json(path, event)
    return run_blackbox(args, ["sdk", "ingest", "--ledger", str(args.ledger), "--input", str(path)])


def ingest_batch(args: argparse.Namespace, events: list[dict[str, Any]], event_dir: Path, name: str) -> dict[str, Any]:
    path = event_dir / f"{name}.cloudevents.json"
    write_json(path, events)
    return run_blackbox(args, ["sdk", "ingest", "--ledger", str(args.ledger), "--input", str(path)])


TOOL_DEFINITION = {
    "type": "function",
    "function": {
        "name": "run_dynamicjepa_blackboxfix_pipeline",
        "description": (
            "Run the verified BlackBoxFix + DynamicJEPA AgentOps demo with data ingest, "
            "panel materialization, action-conditioned training, planning, surprise recording, "
            "artifact provenance, and FSV evidence."
        ),
        "parameters": {
            "type": "object",
            "properties": {
                "scenario": {"type": "string", "enum": [SCENARIO]},
                "requested_capabilities": {
                    "type": "array",
                    "items": {"type": "string"},
                },
            },
            "required": ["scenario", "requested_capabilities"],
            "additionalProperties": False,
        },
    },
}


def parse_tool_arguments(raw: Any) -> dict[str, Any]:
    if isinstance(raw, str):
        try:
            raw = json.loads(raw)
        except json.JSONDecodeError as err:
            raise DemoError(
                "DEMO_TOOL_ARGUMENTS_INVALID",
                f"tool arguments string was not valid JSON: {err}",
                "ollama.message.tool_calls[].function.arguments",
                "rerun only after Gemma emits valid JSON tool arguments",
                details={"arguments": raw},
            ) from err
    if not isinstance(raw, dict):
        raise DemoError(
            "DEMO_TOOL_ARGUMENTS_INVALID",
            "tool arguments must be a JSON object",
            "ollama.message.tool_calls[].function.arguments",
            "rerun only after Gemma emits object-shaped tool arguments",
            details={"arguments": raw},
        )
    return raw


def validate_tool_arguments(value: dict[str, Any]) -> None:
    if value.get("scenario") != SCENARIO:
        raise DemoError(
            "DEMO_TOOL_ARGUMENTS_INVALID",
            "tool scenario did not match the only allowlisted scenario",
            "tool.arguments.scenario",
            f"use scenario={SCENARIO!r}",
            details={"arguments": value},
        )
    caps = value.get("requested_capabilities")
    if not isinstance(caps, list) or not all(isinstance(item, str) for item in caps):
        raise DemoError(
            "DEMO_TOOL_ARGUMENTS_INVALID",
            "requested_capabilities must be an array of strings",
            "tool.arguments.requested_capabilities",
            "request every required demo capability explicitly",
            details={"arguments": value},
        )
    missing = sorted(set(REQUIRED_CAPABILITIES) - set(caps))
    if missing:
        raise DemoError(
            "DEMO_TOOL_ARGUMENTS_INVALID",
            "Gemma did not request the complete capability set",
            "tool.arguments.requested_capabilities",
            "include all required capabilities before executing the demo tool",
            details={"missing_capabilities": missing, "arguments": value},
        )


def call_gemma_for_tool(args: argparse.Namespace) -> dict[str, Any]:
    system = (
        "You are the controller for a BlackBoxFix research demo. "
        "You must call the provided tool exactly once. Do not answer in prose before calling it."
    )
    user = {
        "task": "Run the full local AgentOps proof pipeline.",
        "operator_request_sha256": sha256_bytes(args.user_message.encode("utf-8")),
        "operator_request_redacted_preview": redacted_preview(args.user_message),
        "scenario": SCENARIO,
        "required_capabilities": REQUIRED_CAPABILITIES,
        "instruction": (
            "Treat operator_request_redacted_preview as trace context only. "
            "Call run_dynamicjepa_blackboxfix_pipeline with scenario and requested_capabilities "
            "exactly matching the values above."
        ),
    }
    body = {
        "model": args.model,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": json.dumps(user, sort_keys=True)},
        ],
        "tools": [TOOL_DEFINITION],
        "stream": False,
        "think": False,
        "keep_alive": args.keep_alive,
        "options": {"temperature": 0},
    }
    response = http_json(f"{args.ollama_url.rstrip('/')}/api/chat", body, timeout=args.timeout_seconds)
    message = response.get("message")
    if not isinstance(message, dict):
        raise DemoError(
            "DEMO_MODEL_RESPONSE_INVALID",
            "Ollama chat response did not contain a message object",
            "ollama.response.message",
            "inspect Ollama chat API compatibility",
            source_of_truth=f"{args.ollama_url.rstrip('/')}/api/chat",
            details={"response": response},
        )
    tool_calls = message.get("tool_calls")
    if not isinstance(tool_calls, list) or not tool_calls:
        raise DemoError(
            "DEMO_MODEL_TOOL_CALL_MISSING",
            "Gemma did not emit a native tool call",
            "ollama.response.message.tool_calls",
            "use a model/runtime with native tool-calling support and rerun",
            source_of_truth=f"{args.ollama_url.rstrip('/')}/api/chat",
            details={"response": response},
        )
    if len(tool_calls) != 1:
        raise DemoError(
            "DEMO_MODEL_TOOL_CALL_COUNT_INVALID",
            "Gemma emitted more than one tool call for a single-shot demo",
            "ollama.response.message.tool_calls",
            "rerun after constraining the prompt/tool surface to one call",
            details={"tool_call_count": len(tool_calls), "response": response},
        )
    function = tool_calls[0].get("function") if isinstance(tool_calls[0], dict) else None
    if not isinstance(function, dict) or function.get("name") != "run_dynamicjepa_blackboxfix_pipeline":
        raise DemoError(
            "DEMO_TOOL_UNSUPPORTED",
            "Gemma requested a tool outside the allowlist",
            "ollama.response.message.tool_calls[0].function.name",
            "use only the declared run_dynamicjepa_blackboxfix_pipeline tool",
            details={"tool_call": tool_calls[0]},
        )
    arguments = parse_tool_arguments(function.get("arguments"))
    validate_tool_arguments(arguments)
    return {
        "ollama_response": response,
        "tool_call": tool_calls[0],
        "arguments": arguments,
        "prompt_sha256": sha256_bytes(json.dumps(user, sort_keys=True).encode("utf-8")),
        "operator_request_sha256": user["operator_request_sha256"],
        "operator_request_redacted_preview": user["operator_request_redacted_preview"],
    }


def sanitize_run_id(value: str) -> str:
    if not re.fullmatch(r"[A-Za-z0-9._-]+", value):
        raise DemoError(
            "DEMO_RUN_ID_INVALID",
            "run id must contain only ASCII letters, digits, dot, dash, or underscore",
            "run_id",
            "pass a filesystem-safe run id",
            details={"run_id": value},
        )
    return value


def setup_root(args: argparse.Namespace) -> None:
    root = args.root
    if root.exists():
        marker = root / ".blackboxfix_agentops_demo_root"
        if not args.force:
            raise DemoError(
                "DEMO_ROOT_EXISTS",
                "demo root already exists",
                "root",
                "choose a new --root or pass --force for a previous demo root with the marker file",
                source_of_truth=file_sot(root),
            )
        if not marker.exists():
            raise DemoError(
                "DEMO_ROOT_FORCE_REFUSED",
                "refusing to remove a directory that is not marked as this demo root",
                "root",
                "choose a fresh --root; --force only removes previous harness-created roots",
                source_of_truth=file_sot(root),
            )
        shutil.rmtree(root)
    root.mkdir(parents=True)
    atomic_write(root / ".blackboxfix_agentops_demo_root", b"blackboxfix-agentops-demo\n")
    for subdir in [
        "blackbox/events",
        "blackbox/verifiers",
        "blackbox/exports",
        "dynamicjepa/dbs",
        "dynamicjepa/artifacts",
        "dynamicjepa/evidence",
        "dynamicjepa/bundle",
        "dynamicjepa/exports",
        "provenance",
    ]:
        (root / subdir).mkdir(parents=True, exist_ok=True)


def preflight(args: argparse.Namespace) -> dict[str, Any]:
    missing_paths = [
        str(path)
        for path in [
            args.blackbox_bin,
            args.repo_root / "configs/dynamicjepa/domain_packs/counter_world.v1.toml",
            args.repo_root / "configs/dynamicjepa/verification/tiny_train_config.json",
        ]
        if not path.exists()
    ]
    if missing_paths:
        raise DemoError(
            "DEMO_PREFLIGHT_FILE_MISSING",
            "required local project file was missing",
            "preflight.paths",
            "build the project and restore the DynamicJEPA config files before running the demo",
            details={"missing_paths": missing_paths},
        )
    tags = ensure_ollama_model(args.ollama_url, args.model, args.timeout_seconds)
    return {
        "blackbox_bin": str(args.blackbox_bin),
        "repo_root": str(args.repo_root),
        "ollama_model_present": args.model,
        "ollama_model_count": len(tags.get("models", [])),
    }


def read_dynamic_counts(args: argparse.Namespace, db_path: Path, *, allow_missing: bool = False) -> dict[str, Any]:
    command = ["inspect-counts", "--db", str(db_path), "--json"]
    if allow_missing:
        command.insert(3, "--allow-missing")
    return run_dynamicjepa(args, command)


def compact_counts(value: dict[str, Any]) -> dict[str, int]:
    counts = value.get("counts") or value.get("after_counts")
    if not isinstance(counts, dict):
        raise DemoError(
            "DEMO_DYNAMICJEPA_COUNTS_INVALID",
            "DynamicJEPA command did not contain counts",
            "dynamicjepa.counts",
            "inspect DynamicJEPA CLI output and fix the command contract",
            details={"output": value},
        )
    return {key: int(counts.get(key, 0)) for key in IMPORTANT_DJ_CFS}


def find_decoded_file(evidence_dir: Path, name: str) -> Path:
    path = evidence_dir / "decoded" / f"{name}.json"
    if not path.exists():
        raise DemoError(
            "DEMO_DYNAMICJEPA_EVIDENCE_MISSING",
            "expected DynamicJEPA decoded evidence file is missing",
            str(path),
            "rerun verify-counter-world and inspect its evidence_dir",
            source_of_truth=file_sot(path),
        )
    return path


def build_bundle(args: argparse.Namespace, dynamic_run_id: str, db_root: Path, verify_output: dict[str, Any], counts: dict[str, Any]) -> dict[str, Path]:
    bundle = args.root / "dynamicjepa/bundle"
    evidence_dir = Path(verify_output["evidence_dir"])
    edge_results = []
    for index, edge_path in enumerate(sorted(evidence_dir.glob("edge_*.json"))):
        edge_results.append(
            {
                "domain": DOMAIN,
                "edge_index": index,
                "evidence_file": str(edge_path),
                "edge": read_json(edge_path),
            }
        )
    if len(edge_results) < 3:
        raise DemoError(
            "DEMO_DYNAMICJEPA_EDGE_EVIDENCE_INCOMPLETE",
            "DynamicJEPA verification did not produce at least three edge evidence files",
            "dynamicjepa.evidence.edge_*",
            "rerun verify-counter-world and inspect edge fixture execution",
            source_of_truth=file_sot(evidence_dir),
            details={"edge_count": len(edge_results)},
        )
    write_json(
        bundle / "run_manifest.json",
        {
            "bundle_schema_version": 2,
            "status": "passed",
            "run_id": dynamic_run_id,
            "db_path": str(db_root),
            "created_by": "blackboxfix_gemma_tool_demo.py",
        },
    )
    write_json(
        bundle / "fsv_report.json",
        {
            "status": "passed",
            "verification_run_ids": [verify_output["verification_run_id"]],
            "evidence_dir": str(evidence_dir),
        },
    )
    write_json(bundle / "db_counts_after.json", {DOMAIN: counts["counts"]})
    write_json(bundle / "edge_case_results.json", edge_results)
    return {
        "bundle": bundle,
        "run_manifest": bundle / "run_manifest.json",
        "fsv_report": bundle / "fsv_report.json",
        "db_counts_after": bundle / "db_counts_after.json",
        "edge_case_results": bundle / "edge_case_results.json",
    }


def write_verifier_script(path: Path) -> None:
    script = r'''#!/usr/bin/env python3
import argparse
import hashlib
import json
import subprocess
import sys
from pathlib import Path

IMPORTANT = [
    "dj_raw_events", "dj_latent_panels", "dj_dataset_shards", "dj_training_runs",
    "dj_model_artifacts", "dj_predictions", "dj_guard_decisions", "dj_plan_traces",
    "dj_surprise_events", "dj_verification_runs", "dj_audit_log"
]

def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            h.update(chunk)
    return "sha256:" + h.hexdigest()

def run_json(args):
    proc = subprocess.run(args, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False)
    raw = proc.stdout.strip() or proc.stderr.strip()
    try:
        value = json.loads(raw)
    except json.JSONDecodeError as err:
        print(json.dumps({"status":"error","error_code":"DEMO_VERIFY_JSON_INVALID","command":args,"stdout":proc.stdout,"stderr":proc.stderr,"message":str(err)}))
        sys.exit(41)
    if proc.returncode != 0:
        print(json.dumps({"status":"error","error_code":"DEMO_VERIFY_COMMAND_FAILED","command":args,"output":value}))
        sys.exit(proc.returncode)
    return value

def fail(code, field, message, details=None):
    print(json.dumps({"status":"error","error_code":code,"field_path":field,"message":message,"details":details or {}}, sort_keys=True))
    sys.exit(42)

def uuid_key_hex(value):
    return value.replace("-", "")

parser = argparse.ArgumentParser()
parser.add_argument("--manifest", required=True)
parser.add_argument("--expect-plan-traces", type=int)
args = parser.parse_args()

manifest_path = Path(args.manifest)
manifest = json.loads(manifest_path.read_text())
bin_path = manifest["context_graph_bin"]
db = manifest["db_path"]

counts = run_json([bin_path, "dynamicjepa", "inspect-counts", "--db", db, "--json"])
actual_counts = counts["counts"]
expected_counts = manifest["expected_counts"]
for cf in IMPORTANT:
    if int(actual_counts.get(cf, 0)) != int(expected_counts.get(cf, 0)):
        fail("DEMO_VERIFY_DYNAMICJEPA_COUNT_MISMATCH", f"counts.{cf}", "DynamicJEPA physical count mismatch", {"expected": expected_counts.get(cf), "actual": actual_counts.get(cf)})
if args.expect_plan_traces is not None and int(actual_counts.get("dj_plan_traces", 0)) != args.expect_plan_traces:
    fail("DEMO_VERIFY_BAD_PLAN_TRACE_EXPECTATION", "counts.dj_plan_traces", "intentional bad verifier expectation did not match physical RocksDB state", {"expected": args.expect_plan_traces, "actual": actual_counts.get("dj_plan_traces")})

artifact = run_json([bin_path, "dynamicjepa", "get-artifact", "--db", db, "--id", manifest["artifact_id"], "--verify-files", "--json"])
for item in artifact.get("artifact_hashes", []):
    if item.get("equal") is not True:
        fail("DEMO_VERIFY_ARTIFACT_HASH_MISMATCH", "artifact_hashes", "artifact registry file hash mismatch", {"item": item})

training = run_json([bin_path, "dynamicjepa", "get-training-run", "--db", db, "--id", manifest["training_run_id"], "--json"])
if manifest["artifact_id"] not in training["decoded_records"]["training_run"]["artifact_ids"]:
    fail("DEMO_VERIFY_TRAINING_ARTIFACT_LINK_MISSING", "training_run.artifact_ids", "training run does not link to artifact id")

plan = run_json([bin_path, "dynamicjepa", "get-plan-trace", "--db", db, "--id", manifest["plan_trace_id"], "--include-predictions", "--include-guards", "--json"])
if len(plan["decoded_records"]["predictions"]) != 5 or len(plan["decoded_records"]["guard_decisions"]) != 5:
    fail("DEMO_VERIFY_PLAN_LINK_COUNT_MISMATCH", "plan_trace.links", "plan did not read back five predictions and five guard decisions", {"plan": plan["summary"]})

surprise = run_json([bin_path, "dynamicjepa", "get-surprise", "--db", db, "--id", manifest["surprise_event_id"], "--json"])
if surprise["decoded_records"]["surprise_event"]["surprise_event_id"] != manifest["surprise_event_id"]:
    fail("DEMO_VERIFY_SURPRISE_READBACK_MISMATCH", "surprise_event_id", "surprise event readback id mismatch")

for cf, record_id in [
    ("dj_plan_traces", manifest["plan_trace_id"]),
    ("dj_predictions", manifest["prediction_id"]),
    ("dj_surprise_events", manifest["surprise_event_id"]),
]:
    cf_row = run_json([bin_path, "dynamicjepa", "inspect-cf", "--db", db, "--cf", cf, "--key-hex", uuid_key_hex(record_id), "--json"])
    decoded = cf_row["decoded_records"]["row"]["decoded"]
    id_field = {"dj_plan_traces": "plan_trace_id", "dj_predictions": "prediction_id", "dj_surprise_events": "surprise_event_id"}[cf]
    if decoded[id_field] != record_id:
        fail("DEMO_VERIFY_CF_KEY_READBACK_MISMATCH", f"{cf}.{id_field}", "inspect-cf key readback mismatch", {"record_id": record_id, "decoded": decoded})

episodes = Path(manifest["dynamicjepa_episodes"])
episode_manifest = Path(manifest["dynamicjepa_episode_manifest"])
if not episodes.exists() or not episode_manifest.exists():
    fail("DEMO_VERIFY_EPISODE_EXPORT_MISSING", "dynamicjepa_episodes", "episode export artifacts are missing")
lines = [json.loads(line) for line in episodes.read_text().splitlines() if line.strip()]
manifest_json = json.loads(episode_manifest.read_text())
if len(lines) != int(manifest["dynamicjepa_episode_count"]):
    fail("DEMO_VERIFY_EPISODE_COUNT_MISMATCH", "episodes.line_count", "episode JSONL line count mismatch", {"expected": manifest["dynamicjepa_episode_count"], "actual": len(lines)})
if sha256_file(episodes) != manifest_json["output"]["sha256"]:
    fail("DEMO_VERIFY_EPISODE_HASH_MISMATCH", "episodes.sha256", "episode JSONL hash differs from manifest")

provenance = Path(manifest["provenance"])
prov = json.loads(provenance.read_text())
if prov.get("format") != "blackboxfix_agentops_provenance_json_v1":
    fail("DEMO_VERIFY_PROVENANCE_FORMAT_INVALID", "provenance.format", "provenance format mismatch", {"format": prov.get("format")})

print(json.dumps({
    "status": "ok",
    "counts_checked": {cf: actual_counts.get(cf, 0) for cf in IMPORTANT},
    "artifact_id": manifest["artifact_id"],
    "plan_trace_id": manifest["plan_trace_id"],
    "surprise_event_id": manifest["surprise_event_id"],
    "episode_count": len(lines),
    "provenance_sha256": sha256_file(provenance),
}, sort_keys=True))
'''
    atomic_write(path, script.encode("utf-8"))
    path.chmod(0o755)


def execute_dynamicjepa_pipeline(args: argparse.Namespace, dynamic_run_id: str) -> dict[str, Any]:
    db_root = args.root / "dynamicjepa/dbs"
    db_path = db_root / f"{dynamic_run_id}_{DOMAIN}_rocksdb"
    artifact_root = args.root / "dynamicjepa/artifacts"
    evidence_root = args.root / "dynamicjepa/evidence"
    dynamic_exports = args.root / "dynamicjepa/exports"

    before_counts = read_dynamic_counts(args, db_path, allow_missing=True)
    verify_output = run_dynamicjepa(
        args,
        [
            "verify-counter-world",
            "--db",
            str(db_path),
            "--fixtures-root",
            str(args.repo_root / "configs/dynamicjepa"),
            "--artifact-root",
            str(artifact_root),
            "--evidence-root",
            str(evidence_root),
            "--json",
        ],
        timeout=args.dynamic_timeout_seconds,
    )
    after_counts = read_dynamic_counts(args, db_path)
    expected_counts = compact_counts(after_counts)
    summary = verify_output.get("summary")
    if not isinstance(summary, dict):
        raise DemoError(
            "DEMO_DYNAMICJEPA_VERIFY_OUTPUT_INVALID",
            "verify-counter-world output did not contain summary",
            "dynamicjepa.verify.summary",
            "inspect verify-counter-world output",
            details={"output": verify_output},
        )
    evidence_dir = Path(verify_output["evidence_dir"])
    surprise_readback = read_json(find_decoded_file(evidence_dir, "surprise"))
    surprise_event_id = surprise_readback["decoded_records"]["surprise_event"]["surprise_event_id"]

    bundle_paths = build_bundle(args, dynamic_run_id, db_root, verify_output, after_counts)
    episode_path = dynamic_exports / "dynamicjepa-episodes.jsonl"
    episode_manifest_path = dynamic_exports / "dynamicjepa-episodes.manifest.json"
    export_output = run_export(
        args,
        [
            "dynamicjepa-episodes",
            "--bundle",
            str(bundle_paths["bundle"]),
            "--out",
            str(episode_path),
            "--manifest-out",
            str(episode_manifest_path),
            "--overwrite",
        ],
        timeout=args.timeout_seconds,
    )

    get_artifact = run_dynamicjepa(
        args,
        ["get-artifact", "--db", str(db_path), "--id", summary["artifact_id"], "--verify-files", "--json"],
    )
    get_plan = run_dynamicjepa(
        args,
        [
            "get-plan-trace",
            "--db",
            str(db_path),
            "--id",
            summary["plan_trace_id"],
            "--include-predictions",
            "--include-guards",
            "--json",
        ],
    )
    get_surprise = run_dynamicjepa(
        args,
        ["get-surprise", "--db", str(db_path), "--id", surprise_event_id, "--json"],
    )
    raw_sample = run_dynamicjepa(
        args,
        ["inspect-cf", "--db", str(db_path), "--cf", "dj_raw_events", "--limit", "1", "--json"],
    )
    panel_sample = run_dynamicjepa(
        args,
        ["inspect-cf", "--db", str(db_path), "--cf", "dj_latent_panels", "--limit", "1", "--json"],
    )

    provenance_path = args.root / "provenance/artifact-provenance.json"
    provenance = {
        "format": "blackboxfix_agentops_provenance_json_v1",
        "standard_basis": "W3C PROV-style entity/activity/agent graph",
        "generated_at": now_rfc3339(),
        "agents": [
            {"id": "agent:gemma", "type": "prov:SoftwareAgent", "model": args.model},
            {"id": "agent:context-graph-cli", "type": "prov:SoftwareAgent", "path": str(args.blackbox_bin)},
            {"id": "agent:blackboxfix-harness", "type": "prov:SoftwareAgent", "version": DEMO_VERSION},
        ],
        "entities": [
            {"id": "entity:domain-pack", "path": str(args.repo_root / "configs/dynamicjepa/domain_packs/counter_world.v1.toml"), "sha256": sha256_file(args.repo_root / "configs/dynamicjepa/domain_packs/counter_world.v1.toml")},
            {"id": "entity:rocksdb", "path": str(db_path), "source_of_truth": rocksdb_sot(db_path)},
            {"id": "entity:model-artifact", "artifact_id": summary["artifact_id"], "artifact_hashes": get_artifact.get("artifact_hashes")},
            {"id": "entity:plan-trace", "plan_trace_id": summary["plan_trace_id"]},
            {"id": "entity:surprise-event", "surprise_event_id": surprise_event_id},
            {"id": "entity:dynamicjepa-episodes", "path": str(episode_path), "sha256": sha256_file(episode_path)},
        ],
        "activities": [
            {"id": "activity:gemma-tool-call", "used": ["agent:gemma"], "generated": ["activity:dynamicjepa-verify"]},
            {"id": "activity:dynamicjepa-verify", "used": ["entity:domain-pack"], "generated": ["entity:rocksdb", "entity:model-artifact", "entity:plan-trace", "entity:surprise-event"]},
            {"id": "activity:export-action-conditioned-episodes", "used": ["entity:rocksdb"], "generated": ["entity:dynamicjepa-episodes"]},
        ],
        "relations": [
            {"type": "wasGeneratedBy", "entity": "entity:model-artifact", "activity": "activity:dynamicjepa-verify"},
            {"type": "wasDerivedFrom", "generated": "entity:dynamicjepa-episodes", "used": "entity:rocksdb"},
            {"type": "wasAssociatedWith", "activity": "activity:dynamicjepa-verify", "agent": "agent:context-graph-cli"},
        ],
    }
    write_json(provenance_path, provenance)

    verifier_script = args.root / "blackbox/verifiers/verify_agentops_state.py"
    write_verifier_script(verifier_script)
    verifier_manifest_path = args.root / "blackbox/verifiers/expected-agentops-state.json"
    verifier_manifest = {
        "context_graph_bin": str(args.blackbox_bin),
        "db_path": str(db_path),
        "expected_counts": expected_counts,
        "verification_run_id": verify_output["verification_run_id"],
        "training_run_id": summary["training_run_id"],
        "artifact_id": summary["artifact_id"],
        "prediction_id": summary["prediction_id"],
        "plan_trace_id": summary["plan_trace_id"],
        "surprise_event_id": surprise_event_id,
        "dynamicjepa_episodes": str(episode_path),
        "dynamicjepa_episode_manifest": str(episode_manifest_path),
        "dynamicjepa_episode_count": export_output["total_records"],
        "provenance": str(provenance_path),
    }
    write_json(verifier_manifest_path, verifier_manifest)

    pipeline_result_path = args.root / "dynamicjepa/pipeline-result.json"
    pipeline_result = {
        "status": "ok",
        "dynamic_run_id": dynamic_run_id,
        "source_of_truth": {
            "rocksdb": rocksdb_sot(db_path),
            "artifact_root": file_sot(artifact_root),
            "dynamicjepa_episodes": file_sot(episode_path),
            "provenance": file_sot(provenance_path),
        },
        "before_counts": before_counts,
        "after_counts": after_counts,
        "verify_counter_world": verify_output,
        "dynamicjepa_episode_export": export_output,
        "readbacks": {
            "artifact": get_artifact,
            "plan_trace": get_plan,
            "surprise": get_surprise,
            "raw_event_sample": raw_sample,
            "panel_sample": panel_sample,
        },
        "artifacts": {
            "bundle_root": {
                "path": str(bundle_paths["bundle"]),
                "source_of_truth": file_sot(bundle_paths["bundle"]),
                "artifact_kind": "directory",
            },
            "bundle_files": {
                key: artifact_readback(path)
                for key, path in bundle_paths.items()
                if path.is_file()
            },
            "episodes": artifact_readback(episode_path),
            "episode_manifest": artifact_readback(episode_manifest_path),
            "provenance": artifact_readback(provenance_path),
            "verifier_script": artifact_readback(verifier_script),
            "verifier_manifest": artifact_readback(verifier_manifest_path),
        },
    }
    write_json(pipeline_result_path, pipeline_result)
    pipeline_result["artifacts"]["pipeline_result"] = artifact_readback(pipeline_result_path)
    return pipeline_result


def record_blackbox_events(
    args: argparse.Namespace,
    gemma: dict[str, Any],
    pipeline: dict[str, Any],
    dynamic_run_id: str,
) -> tuple[str, str, dict[str, Any], dict[str, Any]]:
    event_dir = args.root / "blackbox/events"
    source = f"urn:contextgraph:blackboxfix:gemma-agentops-demo:{secrets.token_hex(12)}"
    trace = traceparent()
    run_event_id = f"agentops-run-{secrets.token_hex(8)}"
    run_event = cloud_event(
        run_event_id,
        source,
        EVENT_AGENT_RUN,
        trace,
        {
            "tenant": args.tenant,
            "agent": args.agent,
            "model": args.model,
            "task": "Gemma requested full BlackBoxFix AgentOps research demo execution",
            "task_kind": "blackboxfix-gemma-dynamicjepa-full-agentops",
            "risk_tier": "high",
            "sdk": SDK_METADATA,
        },
    )
    run_ingest = ingest_event(args, run_event, event_dir, "agentops-run")
    run_id = run_ingest["records"][0]["identifiers"]["run_id"]
    plan_readback = pipeline["readbacks"]["plan_trace"]["decoded_records"]
    plan_trace = plan_readback["plan_trace"]
    plan_prediction_count = len(plan_readback.get("predictions", []))
    plan_guard_count = len(plan_readback.get("guard_decisions", []))
    plan_candidate_count = len(plan_trace.get("candidate_action_ids", []))
    expected_delta = {
        "source_of_truth": pipeline["source_of_truth"],
        "dynamicjepa": {
            "dynamic_run_id": dynamic_run_id,
            "expected_counts": compact_counts(pipeline["after_counts"]),
            "created_ids": pipeline["verify_counter_world"]["summary"],
            "verification_run_id": pipeline["verify_counter_world"]["verification_run_id"],
        },
        "training": {
            "action_conditioned_dataset_rows": pipeline["verify_counter_world"]["summary"]["split_counts"],
            "artifact_id": pipeline["verify_counter_world"]["summary"]["artifact_id"],
        },
        "planning": {
            "plan_trace_id": pipeline["verify_counter_world"]["summary"]["plan_trace_id"],
            "candidate_count": plan_candidate_count,
            "prediction_count": plan_prediction_count,
            "guard_decision_count": plan_guard_count,
        },
        "surprise": {
            "surprise_count": pipeline["verify_counter_world"]["summary"]["surprise_count"],
            "surprise_event_id": pipeline["readbacks"]["surprise"]["decoded_records"]["surprise_event"]["surprise_event_id"],
        },
        "exports": {
            "dynamicjepa_episode_count": pipeline["dynamicjepa_episode_export"]["total_records"],
            "dynamicjepa_episodes_sha256": pipeline["artifacts"]["episodes"]["sha256"],
            "provenance_sha256": pipeline["artifacts"]["provenance"]["sha256"],
        },
    }
    child_token = secrets.token_hex(8)
    events = [
        cloud_event(
            f"agentops-step-{child_token}",
            source,
            EVENT_AGENT_STEP,
            trace,
            {
                "tenant": args.tenant,
                "agent": args.agent,
                "model": args.model,
                "run_id": run_id,
                "step_kind": "gemma_tool_call_dynamicjepa_pipeline",
                "summary": "Gemma requested the full DynamicJEPA AgentOps tool and the harness executed it.",
                "context": {
                    "gemma_tool_call": gemma["tool_call"],
                    "dynamic_run_id": dynamic_run_id,
                    "required_capabilities": REQUIRED_CAPABILITIES,
                    "pipeline_result": pipeline["artifacts"]["pipeline_result"],
                },
                "sdk": SDK_METADATA,
            },
        ),
        cloud_event(
            f"agentops-decision-{child_token}",
            source,
            EVENT_MODEL_DECISION,
            trace,
            {
                "tenant": args.tenant,
                "agent": args.agent,
                "model": args.model,
                "run_id": run_id,
                "tool_name": "run_dynamicjepa_blackboxfix_pipeline",
                "capability_id": "blackboxfix.demo.full_agentops_dynamicjepa",
                "confidence": 1.0,
                "input": {
                    "scenario": SCENARIO,
                    "requested_capabilities": REQUIRED_CAPABILITIES,
                    "prompt_sha256": gemma["prompt_sha256"],
                },
                "sdk": SDK_METADATA,
            },
        ),
        cloud_event(
            f"agentops-tool-{child_token}",
            source,
            EVENT_TOOL_CALL,
            trace,
            {
                "tenant": args.tenant,
                "agent": args.agent,
                "model": args.model,
                "run_id": run_id,
                "tool_name": "run_dynamicjepa_blackboxfix_pipeline",
                "capability_id": "blackboxfix.demo.full_agentops_dynamicjepa",
                "input": gemma["arguments"],
                "mutating": True,
                "expected_delta": expected_delta,
                "status": "ok",
                "external_request_id": dynamic_run_id,
                "sdk": SDK_METADATA,
            },
        ),
    ]
    child_ingest = ingest_batch(args, events, event_dir, "agentops-child-events")
    tool_record = next(
        (
            item
            for item in child_ingest.get("records", [])
            if item.get("ledger", {}).get("record_kind") == "tool_call"
        ),
        None,
    )
    if not tool_record:
        raise DemoError(
            "DEMO_BLACKBOX_TOOL_CALL_ID_MISSING",
            "SDK ingest did not return a tool_call record",
            "sdk_ingest.records",
            "inspect BlackBoxFix SDK ingest output",
            source_of_truth=ledger_sot(args.ledger),
            details={"child_ingest": child_ingest},
        )
    return run_id, tool_record["identifiers"]["tool_call_id"], run_ingest, child_ingest


def verify_blackbox_tool_call(args: argparse.Namespace, pipeline: dict[str, Any], run_id: str, tool_call_id: str) -> dict[str, Any]:
    spec_path = args.root / "blackbox/verifiers/agentops-verifier-spec.json"
    manifest_path = Path(pipeline["artifacts"]["verifier_manifest"]["path"])
    script_path = Path(pipeline["artifacts"]["verifier_script"]["path"])
    spec = {
        "verifier_id": "cmd_exit_zero_v1",
        "run_id": run_id,
        "tool_call_id": tool_call_id,
        "source": {"cwd": str(args.root)},
        "expected": {
            "command": ["python3", str(script_path), "--manifest", str(manifest_path)],
            "artifacts": [
                str(Path(pipeline["artifacts"]["pipeline_result"]["path"]).relative_to(args.root)),
                str(Path(pipeline["artifacts"]["episodes"]["path"]).relative_to(args.root)),
                str(Path(pipeline["artifacts"]["episode_manifest"]["path"]).relative_to(args.root)),
                str(Path(pipeline["artifacts"]["provenance"]["path"]).relative_to(args.root)),
                str(manifest_path.relative_to(args.root)),
            ],
        },
    }
    write_json(spec_path, spec)
    output = run_blackbox(
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
    payload = latest_payload(args.ledger, "verifier_result", run_id)
    if payload.get("status") != "passed":
        raise DemoError(
            "DEMO_BLACKBOX_VERIFIER_NOT_PASSED",
            "BlackBoxFix verifier_result did not pass",
            "ledger_records.payload_json.status",
            "inspect verifier actual_readback and fix source-of-truth mismatch",
            source_of_truth=ledger_sot(args.ledger),
            details={"verify_output": output, "verifier_payload": payload},
        )
    return {"spec": artifact_readback(spec_path), "output": output, "payload": payload}


def run_intentional_bad_verifier(args: argparse.Namespace, pipeline: dict[str, Any], run_id: str, tool_call_id: str) -> dict[str, Any]:
    before = ledger_state(args.ledger)
    spec_path = args.root / "blackbox/verifiers/agentops-bad-verifier-spec.json"
    manifest_path = Path(pipeline["artifacts"]["verifier_manifest"]["path"])
    script_path = Path(pipeline["artifacts"]["verifier_script"]["path"])
    spec = {
        "verifier_id": "cmd_exit_zero_v1",
        "run_id": run_id,
        "tool_call_id": tool_call_id,
        "source": {"cwd": str(args.root)},
        "expected": {
            "command": [
                "python3",
                str(script_path),
                "--manifest",
                str(manifest_path),
                "--expect-plan-traces",
                "999",
            ],
            "artifacts": [str(manifest_path.relative_to(args.root))],
        },
    }
    write_json(spec_path, spec)
    result = run_process(
        [
            str(args.blackbox_bin),
            "blackbox",
            "verify",
            "--ledger",
            str(args.ledger),
            "--input",
            str(spec_path),
            "--tenant",
            args.tenant,
            "--agent",
            f"{args.agent}_bad_verifier",
            "--model",
            args.model,
        ],
        timeout=args.timeout_seconds,
    )
    parsed: Any = None
    try:
        parsed = json.loads(result.stderr.strip() or result.stdout.strip())
    except json.JSONDecodeError:
        parsed = {"stdout": result.stdout, "stderr": result.stderr}
    if result.exit_code == 0:
        raise DemoError(
            "DEMO_BAD_VERIFIER_DID_NOT_FAIL",
            "intentional bad verifier unexpectedly passed",
            "blackbox.verify.bad_expected_state",
            "fix the verifier script so wrong target state fails closed",
            source_of_truth=ledger_sot(args.ledger),
            details={"output": parsed},
        )
    after = ledger_state(args.ledger)
    failed = latest_payload(args.ledger, "verifier_result", run_id)
    incident = latest_payload(args.ledger, "incident_packet", run_id)
    if failed.get("status") != "failed":
        raise DemoError(
            "DEMO_BAD_VERIFIER_LEDGER_STATUS_INVALID",
            "intentional bad verifier did not persist failed verifier_result",
            "ledger_records.payload_json.status",
            "inspect BlackBoxFix failed verifier persistence",
            source_of_truth=ledger_sot(args.ledger),
            details={"verifier_payload": failed, "command_output": parsed},
        )
    return {
        "spec": artifact_readback(spec_path),
        "exit_code": result.exit_code,
        "command_output": parsed,
        "verifier_payload": failed,
        "incident_payload": incident,
        "pre_state": before,
        "post_state": after,
        "actual_delta": ledger_delta(before, after),
    }


def simulate_no_mutation_edges(args: argparse.Namespace, post_bad_verifier_state: dict[str, Any]) -> list[dict[str, Any]]:
    edges: list[dict[str, Any]] = []

    def append_edge(category: str, name: str, command: str, err: DemoError, before: dict[str, Any], after: dict[str, Any]) -> None:
        item = {
            "category": category,
            "name": name,
            "command": command,
            "exit_code": err.exit_code,
            "error_code": err.error_code,
            "field_path": err.field_path,
            "message": err.message,
            "pre_state": before,
            "post_state": after,
            "expected_delta": zero_ledger_delta(),
            "actual_delta": ledger_delta(before, after),
        }
        if item["actual_delta"] != zero_ledger_delta():
            raise DemoError(
                "DEMO_EDGE_CASE_MUTATED_STATE",
                "no-mutation edge case changed the BlackBoxFix ledger",
                "edge_cases",
                "move validation before ledger writes for fail-closed edges",
                source_of_truth=ledger_sot(args.ledger),
                details=item,
            )
        edges.append(item)

    before = ledger_state(args.ledger)
    try:
        ensure_ollama_model(args.ollama_url, f"{args.model}-missing-{secrets.token_hex(4)}", args.timeout_seconds)
    except DemoError as err:
        append_edge(
            "missing_target_state",
            "missing Ollama model tag",
            "GET /api/tags and require impossible Gemma model name",
            err,
            before,
            ledger_state(args.ledger),
        )
    else:
        raise DemoError(
            "DEMO_EDGE_CASE_DID_NOT_FAIL",
            "missing model edge case unexpectedly succeeded",
            "edge_cases.missing_model",
            "use a synthetic impossible model name",
        )

    before = ledger_state(args.ledger)
    try:
        validate_tool_arguments({"scenario": "not_allowlisted", "requested_capabilities": REQUIRED_CAPABILITIES})
    except DemoError as err:
        append_edge(
            "permission_or_schema_failure",
            "invalid Gemma tool arguments",
            "validate_tool_arguments({scenario:not_allowlisted})",
            err,
            before,
            ledger_state(args.ledger),
        )
    else:
        raise DemoError(
            "DEMO_EDGE_CASE_DID_NOT_FAIL",
            "invalid tool arguments edge case unexpectedly succeeded",
            "edge_cases.invalid_tool_args",
            "restore strict scenario validation",
        )

    if ledger_state(args.ledger) != post_bad_verifier_state:
        raise DemoError(
            "DEMO_EDGE_CASE_POST_STATE_DRIFT",
            "no-mutation edge cases changed the ledger state",
            "edge_cases",
            "inspect edge-case code paths and remove ledger writes",
            source_of_truth=ledger_sot(args.ledger),
            details={"expected": post_bad_verifier_state, "actual": ledger_state(args.ledger)},
        )
    return edges


def export_blackbox_artifacts(args: argparse.Namespace, run_id: str) -> dict[str, Any]:
    out = args.root / "blackbox/exports"
    evidence_path = out / "evidence.json"
    replay_path = out / "replay.json"
    score_path = out / "scorecard.json"
    inspect_path = out / "inspect-ledger.json"
    episodes_dir = out / "episodes"
    otel_dir = out / "otel"
    dashboard_dir = out / "dashboard"

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
    )
    write_json(score_path, score)
    evidence = run_blackbox(
        args,
        ["export", "evidence", "--ledger", str(args.ledger), "--run-id", run_id, "--out", str(evidence_path)],
    )
    replay = run_blackbox(args, ["replay", "--ledger", str(args.ledger), "--run-id", run_id])
    write_json(replay_path, replay)
    episodes = run_blackbox(args, ["export", "episodes", "--ledger", str(args.ledger), "--out", str(episodes_dir), "--force"])
    otel = run_blackbox(args, ["export", "otel", "--ledger", str(args.ledger), "--out", str(otel_dir), "--force"])
    dashboard = run_blackbox(args, ["dashboard", "build", "--ledger", str(args.ledger), "--out", str(dashboard_dir), "--force"])
    inspect = run_blackbox(args, ["inspect", "ledger", "--ledger", str(args.ledger), "--run-id", run_id])
    write_json(inspect_path, inspect)
    readbacks = {
        "blackbox_evidence": artifact_readback(evidence_path),
        "blackbox_replay": artifact_readback(replay_path),
        "blackbox_scorecard": artifact_readback(score_path),
        "blackbox_inspect": artifact_readback(inspect_path),
        "blackbox_episodes_manifest": artifact_readback(episodes_dir / "manifest.json"),
        "blackbox_otel_manifest": artifact_readback(otel_dir / "manifest.json"),
        "blackbox_dashboard_manifest": artifact_readback(dashboard_dir / "manifest.json"),
        "blackbox_dashboard_index": artifact_readback(dashboard_dir / "index.html"),
    }
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
        "readbacks": readbacks,
    }


def build_fsv_evidence(
    args: argparse.Namespace,
    run_id: str,
    pre_state: dict[str, Any],
    post_state: dict[str, Any],
    pipeline: dict[str, Any],
    pass_verify: dict[str, Any],
    bad_verify: dict[str, Any],
    no_mutation_edges: list[dict[str, Any]],
    blackbox_exports: dict[str, Any],
) -> dict[str, Any]:
    actual_delta = ledger_delta(pre_state, post_state)
    bad_edge = {
        "category": "wrong_target_state",
        "name": "intentional bad source-of-truth verifier",
        "command": "blackbox verify with --expect-plan-traces 999",
        "exit_code": bad_verify["exit_code"],
        "error_code": bad_verify["command_output"].get("error_code", "DEMO_BAD_VERIFIER_FAILED"),
        "field_path": bad_verify["command_output"].get("field_path", "blackbox.verify"),
        "message": "BlackBoxFix persisted a failed verifier and incident for wrong expected state",
        "pre_state": bad_verify["pre_state"],
        "post_state": bad_verify["post_state"],
        "expected_delta": bad_verify["actual_delta"],
        "actual_delta": bad_verify["actual_delta"],
    }
    artifacts = [
        pipeline["artifacts"]["pipeline_result"],
        pipeline["artifacts"]["episodes"],
        pipeline["artifacts"]["episode_manifest"],
        pipeline["artifacts"]["provenance"],
        pipeline["artifacts"]["verifier_manifest"],
        pass_verify["spec"],
        bad_verify["spec"],
    ]
    artifacts.extend(blackbox_exports["readbacks"].values())
    return {
        "format": "blackbox_full_state_verification_evidence_json_v1",
        "phase": args.fsv_phase,
        "tenant_id": args.tenant,
        "run_id": run_id,
        "source_of_truth": ledger_sot(args.ledger),
        "trigger": {
            "command": "scripts/blackboxfix_gemma_tool_demo.py --demo full-agentops",
            "exit_code": 0,
            "stdout": "Gemma tool call executed DynamicJEPA AgentOps pipeline and BlackBoxFix readbacks succeeded",
            "stderr": "",
        },
        "pre_state": pre_state,
        "post_state": post_state,
        "expected_delta": actual_delta,
        "actual_delta": actual_delta,
        "read_queries": [
            {
                "source_of_truth": ledger_sot(args.ledger),
                "query": "SELECT record_kind, COUNT(*) FROM ledger_records GROUP BY record_kind ORDER BY record_kind",
                "output": json.dumps(post_state["record_kind_counts"], sort_keys=True),
            },
            {
                "source_of_truth": pipeline["source_of_truth"]["rocksdb"],
                "query": "context-graph dynamicjepa inspect-counts --db DB --json",
                "output": json.dumps(compact_counts(pipeline["after_counts"]), sort_keys=True),
            },
            {
                "source_of_truth": pipeline["source_of_truth"]["dynamicjepa_episodes"],
                "query": "read dynamicjepa-episodes.jsonl and compute SHA-256",
                "output": json.dumps(
                    {
                        "sha256": pipeline["artifacts"]["episodes"]["sha256"],
                        "bytes": pipeline["artifacts"]["episodes"]["bytes"],
                        "records": pipeline["dynamicjepa_episode_export"]["total_records"],
                    },
                    sort_keys=True,
                ),
            },
            {
                "source_of_truth": pipeline["source_of_truth"]["provenance"],
                "query": "read artifact-provenance.json and validate format",
                "output": json.dumps(
                    {
                        "sha256": pipeline["artifacts"]["provenance"]["sha256"],
                        "format": read_json(Path(pipeline["artifacts"]["provenance"]["path"]))["format"],
                    },
                    sort_keys=True,
                ),
            },
        ],
        "artifacts": [
            {"path": item["path"], "sha256": item["sha256"], "bytes": item["bytes"]}
            for item in artifacts
        ],
        "edge_cases": [bad_edge, *no_mutation_edges],
    }


def certify_fsv(args: argparse.Namespace, run_id: str, evidence_path: Path, out_dir: Path) -> dict[str, Any]:
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
            str(out_dir),
            "--force",
            "--run-id",
            run_id,
            "--agent",
            f"{args.agent}_fsv",
            "--model",
            args.model,
        ],
        timeout=args.timeout_seconds,
    )


def parse_args() -> argparse.Namespace:
    default_root = Path("/tmp") / f"blackboxfix-agentops-demo-{int(time.time())}-{secrets.token_hex(3)}"
    parser = argparse.ArgumentParser(description="Run the full Gemma + BlackBoxFix AgentOps demo.")
    parser.add_argument("--root", type=Path, default=default_root, help="Fresh demo root. Must not exist unless --force is used with a previous demo root.")
    parser.add_argument("--model", default="gemma4:e4b", help="Exact Ollama model tag.")
    parser.add_argument("--ollama-url", default="http://127.0.0.1:11434", help="Ollama API base URL.")
    parser.add_argument("--blackbox-bin", type=Path, default=Path("target/debug/context-graph"), help="context-graph binary path.")
    parser.add_argument("--tenant", default="tenant_local", help="BlackBoxFix tenant id.")
    parser.add_argument("--agent", default="blackboxfix_gemma_agentops_demo", help="Agent id for BlackBoxFix rows.")
    parser.add_argument("--run-id", help="Optional DynamicJEPA bundle run id.")
    parser.add_argument("--timeout-seconds", type=float, default=300.0, help="Default command/API timeout.")
    parser.add_argument("--dynamic-timeout-seconds", type=float, default=900.0, help="Timeout for DynamicJEPA verify-counter-world.")
    parser.add_argument("--keep-alive", default="15m", help="Ollama keep_alive.")
    parser.add_argument(
        "--user-message",
        default="Run the full local AgentOps proof pipeline.",
        help="Operator chat prompt to hash and include as redacted trace context for the native Gemma tool-call request.",
    )
    parser.add_argument("--force", action="store_true", help="Remove an existing previous demo root containing the marker file.")
    parser.add_argument("--fsv-phase", type=int, default=24, help="BlackBoxFix FSV phase number.")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    args.repo_root = Path(__file__).resolve().parents[1]
    args.root = args.root.resolve()
    args.blackbox_bin = args.blackbox_bin.resolve()
    args.ledger = (args.root / "blackbox/blackbox.sqlite").resolve()
    dynamic_run_id = sanitize_run_id(
        args.run_id or f"agentops_{datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%SZ')}_{secrets.token_hex(4)}"
    )

    setup_root(args)
    preflight_result = preflight(args)
    gemma = call_gemma_for_tool(args)

    run_blackbox(args, ["init", "--ledger", str(args.ledger)])
    pre_ledger = ledger_state(args.ledger)
    pipeline = execute_dynamicjepa_pipeline(args, dynamic_run_id)
    run_id, tool_call_id, run_ingest, child_ingest = record_blackbox_events(args, gemma, pipeline, dynamic_run_id)
    pass_verify = verify_blackbox_tool_call(args, pipeline, run_id, tool_call_id)
    bad_verify = run_intentional_bad_verifier(args, pipeline, run_id, tool_call_id)
    no_mutation_edges = simulate_no_mutation_edges(args, bad_verify["post_state"])
    blackbox_exports = export_blackbox_artifacts(args, run_id)
    post_ledger = ledger_state(args.ledger)
    run_counts_before_fsv = count_kind_for_run(args.ledger, run_id)

    required_counts = {
        "agent_run": 1,
        "agent_step": 1,
        "model_decision": 1,
        "tool_call": 1,
        "expected_delta": 1,
        "readback_snapshot": 2,
        "verifier_result": 2,
        "evidence_packet": 2,
        "incident_packet": 1,
        "scorecard": 1,
    }
    missing = {kind: count for kind, count in required_counts.items() if run_counts_before_fsv.get(kind, 0) < count}
    if missing:
        raise DemoError(
            "DEMO_LEDGER_CAPABILITY_ROWS_MISSING",
            "BlackBoxFix run is missing required capability rows",
            "ledger_records.record_kind",
            "inspect recording, verification, incident, and scoring commands",
            source_of_truth=ledger_sot(args.ledger),
            details={"run_counts": run_counts_before_fsv, "missing": missing},
        )

    fsv_evidence = build_fsv_evidence(
        args,
        run_id,
        pre_ledger,
        post_ledger,
        pipeline,
        pass_verify,
        bad_verify,
        no_mutation_edges,
        blackbox_exports,
    )
    fsv_evidence_path = args.root / "blackbox/exports/fsv-evidence.json"
    write_json(fsv_evidence_path, fsv_evidence)
    fsv_dir = args.root / "blackbox/exports/fsv-certification"
    fsv_cert = certify_fsv(args, run_id, fsv_evidence_path, fsv_dir)
    final_ledger = ledger_state(args.ledger)

    report = {
        "status": "ok",
        "harness": {"name": "blackboxfix_gemma_tool_demo", "version": DEMO_VERSION},
        "root": str(args.root),
        "preflight": preflight_result,
        "model": {"provider": "ollama", "name": args.model, "tool_call": gemma["tool_call"]},
        "model_io": {
            "operator_request_sha256": gemma["operator_request_sha256"],
            "operator_request_redacted_preview": gemma["operator_request_redacted_preview"],
            "prompt_sha256": gemma["prompt_sha256"],
            "raw_prompt_stored_in_ledger": False,
            "raw_output_stored_in_ledger": False,
        },
        "dynamic_run_id": dynamic_run_id,
        "blackbox_run_id": run_id,
        "tool_call_id": tool_call_id,
        "source_of_truth": {
            "blackbox_ledger": ledger_sot(args.ledger),
            "dynamicjepa_rocksdb": pipeline["source_of_truth"]["rocksdb"],
            "dynamicjepa_episodes": pipeline["source_of_truth"]["dynamicjepa_episodes"],
            "provenance": pipeline["source_of_truth"]["provenance"],
        },
        "dynamicjepa": {
            "verify_counter_world": pipeline["verify_counter_world"],
            "episode_export": pipeline["dynamicjepa_episode_export"],
            "readbacks": pipeline["readbacks"],
        },
        "blackbox": {
            "run_ingest": run_ingest,
            "child_ingest": child_ingest,
            "passed_verifier": pass_verify,
            "intentional_bad_verifier": bad_verify,
            "exports": blackbox_exports,
            "fsv_certification": fsv_cert,
        },
        "full_state_verification": {
            "pre_ledger": pre_ledger,
            "post_ledger_before_fsv": post_ledger,
            "final_ledger": final_ledger,
            "run_counts_before_fsv": run_counts_before_fsv,
            "edge_cases": [fsv_evidence["edge_cases"][0], *no_mutation_edges],
            "fsv_evidence": artifact_readback(fsv_evidence_path),
            "fsv_manifest": artifact_readback(fsv_dir / "manifest.json"),
        },
        "artifacts": {
            "pipeline_result": pipeline["artifacts"]["pipeline_result"],
            "dynamicjepa_episodes": pipeline["artifacts"]["episodes"],
            "dynamicjepa_episode_manifest": pipeline["artifacts"]["episode_manifest"],
            "provenance": pipeline["artifacts"]["provenance"],
            "blackbox_exports": blackbox_exports["readbacks"],
        },
    }
    report_path = args.root / "harness-report.json"
    write_json(report_path, report)
    report["artifacts"]["harness_report"] = artifact_readback(report_path)
    print_json(report)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except DemoError as err:
        print_json(err.to_json(), stream=sys.stderr)
        raise SystemExit(err.exit_code)
    except KeyboardInterrupt:
        print_json(
            {
                "status": "error",
                "error_code": "DEMO_INTERRUPTED",
                "message": "demo interrupted",
                "field_path": "process",
                "source_of_truth": None,
                "remediation": "inspect partial root state before rerunning",
                "details": {},
            },
            stream=sys.stderr,
        )
        raise SystemExit(130)
