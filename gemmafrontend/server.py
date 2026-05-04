#!/usr/bin/env python3
"""Local two-pane ContextGraph + Gemma proof frontend.

This server intentionally avoids framework dependencies. It has two modes:

- live chat: call the real Gemma/Ollama harness and then read back durable
  artifacts;
- inspect: load an existing real run root and verify files plus SQLite state.

It does not create synthetic success data. Missing artifacts or broken source
of truth reads are typed errors.
"""
from __future__ import annotations

import argparse
import base64
import hashlib
import json
import mimetypes
import os
import queue
import re
import shutil
import sqlite3
import subprocess
import sys
import threading
import time
import urllib.error
import urllib.parse
import urllib.request
from datetime import datetime, timezone
from http import HTTPStatus
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any


APP_ROOT = Path(__file__).resolve().parent
REPO_ROOT = APP_ROOT.parent
STATIC_ROOT = APP_ROOT / "static"
DEFAULT_RUNTIME_ROOT = Path(os.environ.get("GEMMAFRONTEND_RUNTIME_ROOT", "/tmp/gemmafrontend-runs"))
DEFAULT_MODEL = os.environ.get("GEMMAFRONTEND_MODEL", "gemma4:e4b")
DEFAULT_OLLAMA_URL = os.environ.get("GEMMAFRONTEND_OLLAMA_URL", "http://127.0.0.1:11434")
MAX_PROMPT_CHARS = 4096
MIN_FREE_BYTES_FOR_LIVE_RUN = 750 * 1024 * 1024
REWARD_LEDGER_SCHEMA_VERSION = "14"
SCENARIO = "counter_world_full_agentops"
INTERNAL_PROOF_TOOL_NAME = "run_dynamicjepa_blackboxfix_pipeline"
REQUIRED_CAPABILITIES = [
    "data_ingest",
    "panel_materialization",
    "action_conditioned_training",
    "planning",
    "surprise_recording",
    "artifact_provenance",
    "blackboxfix_fsv",
]
AGENT_TOOL_NAMES = [
    "run_proof_pipeline",
    "inspect_latest_run",
    "compile_reward_for_run",
    "explain_scorecard",
    "analyze_incident",
    "suggest_reward_improvements",
    "compare_recent_runs",
    "export_buyer_report",
    "evaluate_tool_policy",
    "build_investor_category_pack",
]

AGENT_TOOL_DEFINITIONS: list[dict[str, Any]] = [
    {
        "type": "function",
        "function": {
            "name": "run_proof_pipeline",
            "description": (
                "Run the full verified ContextGraph proof pipeline. This is the only expensive mutating demo tool. "
                "It creates real DynamicJEPA state, BlackBoxFix ledger rows, evidence, incident, scorecard, and reward artifacts."
            ),
            "parameters": {
                "type": "object",
                "properties": {
                    "scenario": {"type": "string", "enum": [SCENARIO]},
                    "requested_capabilities": {
                        "type": "array",
                        "description": "Exact capability ids required by the proof harness. Use every enum value exactly once.",
                        "items": {"type": "string", "enum": REQUIRED_CAPABILITIES},
                    },
                    "reason": {"type": "string"},
                },
                "required": ["scenario", "requested_capabilities", "reason"],
                "additionalProperties": False,
            },
        },
    },
    {
        "type": "function",
        "function": {
            "name": "inspect_latest_run",
            "description": "Read the latest completed real run and report what physical files and SQLite ledger rows prove.",
            "parameters": {
                "type": "object",
                "properties": {
                    "root": {"type": "string", "description": "Optional absolute run root. If omitted, use latest complete run."},
                    "question": {"type": "string"},
                },
                "required": ["question"],
                "additionalProperties": False,
            },
        },
    },
    {
        "type": "function",
        "function": {
            "name": "compile_reward_for_run",
            "description": "Compile or read back the reward/loss sample for a real schema-14 run.",
            "parameters": {
                "type": "object",
                "properties": {
                    "root": {"type": "string", "description": "Optional absolute run root. If omitted, use latest complete run."},
                    "reason": {"type": "string"},
                },
                "required": ["reason"],
                "additionalProperties": False,
            },
        },
    },
    {
        "type": "function",
        "function": {
            "name": "explain_scorecard",
            "description": "Read the latest scorecard and explain trust, risk, correctness, verification confidence, and evidence inputs.",
            "parameters": {
                "type": "object",
                "properties": {
                    "root": {"type": "string", "description": "Optional absolute run root. If omitted, use latest complete run."},
                    "question": {"type": "string"},
                },
                "required": ["question"],
                "additionalProperties": False,
            },
        },
    },
    {
        "type": "function",
        "function": {
            "name": "analyze_incident",
            "description": "Read verifier and incident rows to explain why a run entered incident state.",
            "parameters": {
                "type": "object",
                "properties": {
                    "root": {"type": "string", "description": "Optional absolute run root. If omitted, use latest complete run."},
                    "question": {"type": "string"},
                },
                "required": ["question"],
                "additionalProperties": False,
            },
        },
    },
    {
        "type": "function",
        "function": {
            "name": "suggest_reward_improvements",
            "description": "Read reward components and scorecard evidence, then identify concrete changes that would improve the reward.",
            "parameters": {
                "type": "object",
                "properties": {
                    "root": {"type": "string", "description": "Optional absolute run root. If omitted, use latest complete run."},
                    "objective": {"type": "string"},
                },
                "required": ["objective"],
                "additionalProperties": False,
            },
        },
    },
    {
        "type": "function",
        "function": {
            "name": "compare_recent_runs",
            "description": "Compare two completed real runs using physical snapshots, ledger counts, incidents, risk, and reward.",
            "parameters": {
                "type": "object",
                "properties": {
                    "root_a": {"type": "string"},
                    "root_b": {"type": "string"},
                    "question": {"type": "string"},
                },
                "required": ["question"],
                "additionalProperties": False,
            },
        },
    },
    {
        "type": "function",
        "function": {
            "name": "export_buyer_report",
            "description": "Create a buyer-facing Markdown evidence report from the latest real verified run and read the report back from disk.",
            "parameters": {
                "type": "object",
                "properties": {
                    "root": {"type": "string", "description": "Optional absolute run root. If omitted, use latest complete run."},
                    "audience": {"type": "string"},
                },
                "required": ["audience"],
                "additionalProperties": False,
            },
        },
    },
    {
        "type": "function",
        "function": {
            "name": "evaluate_tool_policy",
            "description": "Persist a real BlackBoxFix policy_decision row for the proof tool and read the policy outcome back from SQLite.",
            "parameters": {
                "type": "object",
                "properties": {
                    "root": {"type": "string", "description": "Optional absolute run root. If omitted, use latest complete run."},
                    "mode": {"type": "string", "enum": ["observe_only", "supervised", "bounded_autonomous", "blocked"]},
                    "question": {"type": "string"},
                },
                "required": ["mode", "question"],
                "additionalProperties": False,
            },
        },
    },
    {
        "type": "function",
        "function": {
            "name": "build_investor_category_pack",
            "description": (
                "Materialize the product-category investor pack from real runs: model arena, verified outcome leaderboard, "
                "failure studio, governance evidence map, connector contracts, reward dashboard, executive view, and signed attestation."
            ),
            "parameters": {
                "type": "object",
                "properties": {
                    "audience": {"type": "string"},
                    "max_runs": {"type": "integer", "minimum": 1, "maximum": 12},
                },
                "required": ["audience"],
                "additionalProperties": False,
            },
        },
    },
]


def agent_tool_catalog() -> dict[str, Any]:
    tools = []
    for definition in AGENT_TOOL_DEFINITIONS:
        function = definition.get("function") if isinstance(definition, dict) else None
        if not isinstance(function, dict):
            continue
        parameters = function.get("parameters") if isinstance(function.get("parameters"), dict) else {}
        properties = parameters.get("properties") if isinstance(parameters.get("properties"), dict) else {}
        required = parameters.get("required") if isinstance(parameters.get("required"), list) else []
        tool_properties = []
        for name, schema in properties.items():
            if not isinstance(schema, dict):
                continue
            items = schema.get("items") if isinstance(schema.get("items"), dict) else {}
            tool_properties.append(
                {
                    "name": name,
                    "type": schema.get("type"),
                    "required": name in required,
                    "description": schema.get("description"),
                    "enum": schema.get("enum"),
                    "items_enum": items.get("enum"),
                }
            )
        tools.append(
            {
                "name": function.get("name"),
                "description": function.get("description"),
                "required": required,
                "properties": tool_properties,
            }
        )
    return {
        "status": "ok",
        "source_of_truth": "process:gemmafrontend.server.AGENT_TOOL_DEFINITIONS",
        "tool_count": len(tools),
        "contract": {
            "action_boundary": "the model may request one tool; ContextGraph validates, executes, and verifies it",
            "unsupported_tool_behavior": "fail_closed_with_typed_error",
            "prompting_rule": "type your own request in natural language; these hints are not executable presets",
        },
        "tools": tools,
    }


EVENT_LOG: list[dict[str, Any]] = []
EVENT_LOCK = threading.Lock()
EVENT_QUEUES: list[dict[str, Any]] = []


class FrontendError(Exception):
    def __init__(
        self,
        error_code: str,
        message: str,
        field_path: str,
        remediation: str,
        *,
        source_of_truth: str | None = None,
        details: dict[str, Any] | None = None,
        http_status: int = 400,
    ) -> None:
        super().__init__(message)
        self.error_code = error_code
        self.message = message
        self.field_path = field_path
        self.remediation = remediation
        self.source_of_truth = source_of_truth
        self.details = details or {}
        self.http_status = http_status

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


def now_rfc3339() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def file_sot(path: Path) -> str:
    return f"file:{path.resolve()}"


def ledger_sot(ledger: Path) -> str:
    return f"sqlite:{ledger.resolve()}#ledger_records"


def sha256_bytes(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def sha256_file(path: Path) -> str:
    try:
        digest = hashlib.sha256()
        with path.open("rb") as handle:
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                digest.update(chunk)
        return "sha256:" + digest.hexdigest()
    except OSError as err:
        raise FrontendError(
            "GEMMAFRONTEND_FILE_READ_FAILED",
            f"failed to read file for SHA-256: {err}",
            str(path),
            "fix filesystem permissions or regenerate the source artifact",
            source_of_truth=file_sot(path),
        ) from err


def atomic_write(path: Path, data: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_name(f".{path.name}.tmp-{os.getpid()}-{time.time_ns()}")
    try:
        with tmp.open("xb") as handle:
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
        raise FrontendError(
            "GEMMAFRONTEND_ATOMIC_WRITE_FAILED",
            f"failed to atomically write {path}: {err}",
            str(path),
            "check disk space and directory permissions",
            source_of_truth=file_sot(path),
        ) from err


def write_json(path: Path, value: Any) -> None:
    atomic_write(path, json.dumps(value, indent=2, sort_keys=True).encode("utf-8") + b"\n")


def load_json(path: Path, field_path: str) -> Any:
    if not path.exists():
        raise FrontendError(
            "GEMMAFRONTEND_ARTIFACT_MISSING",
            "required run artifact is missing",
            field_path,
            "rerun the real demo harness or inspect a complete run root",
            source_of_truth=file_sot(path),
            details={"path": str(path)},
        )
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except OSError as err:
        raise FrontendError(
            "GEMMAFRONTEND_JSON_READ_FAILED",
            f"failed to read JSON artifact: {err}",
            field_path,
            "fix filesystem access and retry",
            source_of_truth=file_sot(path),
        ) from err
    except json.JSONDecodeError as err:
        raise FrontendError(
            "GEMMAFRONTEND_JSON_INVALID",
            f"JSON artifact is invalid: {err}",
            field_path,
            "fix the artifact producer and rerun from a clean root",
            source_of_truth=file_sot(path),
        ) from err


def redact_preview(value: str, *, limit: int = 180) -> str:
    redacted = re.sub(r"(?i)bearer\s+[A-Za-z0-9._~+/=-]+", "Bearer [REDACTED]", value)
    redacted = re.sub(r"(?i)(api[_-]?key|token|password|secret)\s*[:=]\s*['\"]?[^\s,'\"]+", r"\1=[REDACTED]", redacted)
    redacted = re.sub(r"\b(?:sk|ghp|github_pat|glpat)-[A-Za-z0-9_-]{12,}\b", "[REDACTED_TOKEN]", redacted)
    redacted = " ".join(redacted.split())
    if len(redacted) > limit:
        return redacted[: limit - 3] + "..."
    return redacted


def event_matches(event: dict[str, Any], client_id: str | None, request_id: str | None = None) -> bool:
    if event.get("scope") == "global":
        return request_id is None
    if not client_id or event.get("client_id") != client_id:
        return False
    if request_id is not None and event.get("request_id") != request_id:
        return False
    return True


def publish(
    stage: str,
    message: str,
    *,
    level: str = "info",
    details: dict[str, Any] | None = None,
    client_id: str | None = None,
    request_id: str | None = None,
    scope: str | None = None,
) -> dict[str, Any]:
    event_scope = scope or ("client" if client_id else "global")
    event = {
        "id": f"evt_{time.time_ns()}",
        "created_at": now_rfc3339(),
        "stage": stage,
        "level": level,
        "message": message,
        "details": details or {},
        "scope": event_scope,
        "client_id": client_id,
        "request_id": request_id,
    }
    with EVENT_LOCK:
        EVENT_LOG.append(event)
        del EVENT_LOG[:-250]
        subscribers = list(EVENT_QUEUES)
    for subscriber in subscribers:
        if not event_matches(event, subscriber.get("client_id")):
            continue
        try:
            subscriber["queue"].put_nowait(event)
        except queue.Full:
            pass
    return event


def run_process(command: list[str], *, timeout: float) -> dict[str, Any]:
    try:
        proc = subprocess.run(
            command,
            cwd=str(REPO_ROOT),
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=timeout,
            check=False,
        )
    except subprocess.TimeoutExpired as err:
        raise FrontendError(
            "GEMMAFRONTEND_COMMAND_TIMEOUT",
            f"command timed out after {timeout}s",
            "command",
            "inspect the running process and increase timeout only if the source-of-truth operation is expected to take longer",
            details={"command": command, "timeout_seconds": timeout},
            http_status=500,
        ) from err
    except OSError as err:
        raise FrontendError(
            "GEMMAFRONTEND_COMMAND_EXEC_FAILED",
            f"failed to execute command: {err}",
            "command[0]",
            "install the executable or correct the configured path",
            details={"command": command},
            http_status=500,
        ) from err

    stdout_value: Any = None
    stderr_value: Any = None
    for attr, raw in [("stdout", proc.stdout), ("stderr", proc.stderr)]:
        text = raw.strip()
        if not text:
            continue
        try:
            value = json.loads(text)
        except json.JSONDecodeError:
            value = text[:4000]
        if attr == "stdout":
            stdout_value = value
        else:
            stderr_value = value
    if proc.returncode != 0:
        typed = stderr_value if isinstance(stderr_value, dict) else stdout_value
        error_code = typed.get("error_code") if isinstance(typed, dict) else "GEMMAFRONTEND_COMMAND_FAILED"
        field_path = typed.get("field_path") if isinstance(typed, dict) else "command"
        remediation = typed.get("remediation") if isinstance(typed, dict) else "inspect stdout/stderr and fix the failing source-of-truth operation"
        raise FrontendError(
            str(error_code),
            "command exited non-zero",
            str(field_path),
            str(remediation),
            details={"command": command, "exit_code": proc.returncode, "stdout": stdout_value, "stderr": stderr_value},
            http_status=500,
        )
    return {
        "command": command,
        "exit_code": proc.returncode,
        "stdout": stdout_value,
        "stderr": stderr_value,
    }


def http_json(url: str, *, timeout: float = 5.0) -> Any:
    try:
        with urllib.request.urlopen(url, timeout=timeout) as response:
            raw = response.read()
    except urllib.error.URLError as err:
        raise FrontendError(
            "GEMMAFRONTEND_OLLAMA_UNREACHABLE",
            f"Ollama API request failed: {err}",
            "ollama_url",
            "start Ollama and verify the configured API URL",
            source_of_truth=url,
            http_status=503,
        ) from err
    try:
        return json.loads(raw)
    except json.JSONDecodeError as err:
        raise FrontendError(
            "GEMMAFRONTEND_OLLAMA_JSON_INVALID",
            f"Ollama returned invalid JSON: {err}",
            "ollama.response",
            "inspect the local Ollama API response",
            source_of_truth=url,
            http_status=502,
        ) from err


def http_post_json(url: str, body: dict[str, Any], *, timeout: float = 300.0) -> Any:
    data = json.dumps(body).encode("utf-8")
    request = urllib.request.Request(
        url,
        data=data,
        headers={"Content-Type": "application/json", "Accept": "application/json"},
        method="POST",
    )
    try:
        with urllib.request.urlopen(request, timeout=timeout) as response:
            raw = response.read()
    except urllib.error.HTTPError as err:
        raw_body = err.read().decode("utf-8", errors="replace")
        parsed: Any = raw_body
        try:
            parsed = json.loads(raw_body)
        except json.JSONDecodeError:
            pass
        raise FrontendError(
            "GEMMAFRONTEND_OLLAMA_HTTP_ERROR",
            f"Ollama API returned HTTP {err.code}",
            "ollama.response.status",
            "inspect the local Ollama error body and model/tool-call compatibility",
            source_of_truth=url,
            details={"http_status": err.code, "body": parsed},
            http_status=502,
        ) from err
    except urllib.error.URLError as err:
        raise FrontendError(
            "GEMMAFRONTEND_OLLAMA_UNREACHABLE",
            f"Ollama API request failed: {err}",
            "ollama_url",
            "start Ollama and verify the configured API URL",
            source_of_truth=url,
            http_status=503,
        ) from err
    try:
        return json.loads(raw)
    except json.JSONDecodeError as err:
        raise FrontendError(
            "GEMMAFRONTEND_OLLAMA_JSON_INVALID",
            f"Ollama returned invalid JSON: {err}",
            "ollama.response",
            "inspect the local Ollama API response",
            source_of_truth=url,
            details={"raw": raw.decode("utf-8", errors="replace")},
            http_status=502,
        ) from err


def check_required_file(path: Path, field_path: str) -> None:
    if not path.exists():
        raise FrontendError(
            "GEMMAFRONTEND_REQUIRED_FILE_MISSING",
            "required local project file is missing",
            field_path,
            "build the project or restore the missing file before running the live demo",
            source_of_truth=file_sot(path),
            details={"path": str(path)},
            http_status=500,
        )


def validate_prompt(message: Any) -> str:
    if not isinstance(message, str):
        raise FrontendError(
            "GEMMAFRONTEND_PROMPT_INVALID",
            "message must be a string",
            "request.message",
            "send a non-empty text prompt",
        )
    stripped = message.strip()
    if not stripped:
        raise FrontendError(
            "GEMMAFRONTEND_PROMPT_EMPTY",
            "message is empty",
            "request.message",
            "send a prompt that describes the action to verify",
        )
    if len(stripped) > MAX_PROMPT_CHARS:
        raise FrontendError(
            "GEMMAFRONTEND_PROMPT_TOO_LONG",
            f"message exceeds {MAX_PROMPT_CHARS} characters",
            "request.message",
            "shorten the prompt or add governed raw-prompt storage before increasing the limit",
            details={"chars": len(stripped), "max_chars": MAX_PROMPT_CHARS},
        )
    return stripped


def validate_run_root(root_value: Any) -> Path:
    if not isinstance(root_value, str) or not root_value.strip():
        raise FrontendError(
            "GEMMAFRONTEND_RUN_ROOT_INVALID",
            "root must be a non-empty string",
            "request.root",
            "pass an absolute path to a completed harness run root",
        )
    root = Path(root_value).expanduser().resolve()
    if not root.exists():
        raise FrontendError(
            "GEMMAFRONTEND_RUN_ROOT_MISSING",
            "run root does not exist",
            "request.root",
            "run the real demo harness or choose an existing completed run",
            source_of_truth=file_sot(root),
        )
    marker = root / ".blackboxfix_agentops_demo_root"
    if not marker.exists():
        raise FrontendError(
            "GEMMAFRONTEND_RUN_ROOT_MARKER_MISSING",
            "run root is not marked as a BlackBoxFix AgentOps demo root",
            "request.root",
            "choose a directory produced by scripts/blackboxfix_gemma_tool_demo.py",
            source_of_truth=file_sot(marker),
        )
    return root


def validate_correlation_id(value: Any, field_path: str) -> str | None:
    if value is None:
        return None
    if not isinstance(value, str) or not re.fullmatch(r"[A-Za-z0-9._:-]{1,120}", value):
        raise FrontendError(
            "GEMMAFRONTEND_CORRELATION_ID_INVALID",
            "correlation id must be a short ASCII token",
            field_path,
            "send a client/request id containing only letters, digits, dot, dash, underscore, or colon",
            details={"value": value},
        )
    return value


def parse_tool_arguments(raw: Any, field_path: str) -> dict[str, Any]:
    if isinstance(raw, str):
        try:
            raw = json.loads(raw)
        except json.JSONDecodeError as err:
            raise FrontendError(
                "GEMMAFRONTEND_TOOL_ARGUMENTS_JSON_INVALID",
                f"tool arguments string is invalid JSON: {err}",
                field_path,
                "rerun only after the model emits JSON object tool arguments",
                details={"arguments": raw},
                http_status=502,
            ) from err
    if raw is None:
        return {}
    if not isinstance(raw, dict):
        raise FrontendError(
            "GEMMAFRONTEND_TOOL_ARGUMENTS_SHAPE_INVALID",
            "tool arguments must be a JSON object",
            field_path,
            "rerun only after the model emits object-shaped tool arguments",
            details={"arguments": raw},
            http_status=502,
        )
    return raw


def validate_optional_root_argument(value: Any, runtime_root: Path) -> Path:
    if value is None or value == "":
        return latest_complete_run(runtime_root)
    return validate_run_root(value)


def validate_agent_tool_call(tool_call: dict[str, Any]) -> tuple[str, dict[str, Any]]:
    function = tool_call.get("function")
    if not isinstance(function, dict):
        raise FrontendError(
            "GEMMAFRONTEND_TOOL_CALL_SHAPE_INVALID",
            "model tool call did not contain a function object",
            "ollama.response.message.tool_calls[0].function",
            "use a model/runtime that emits native function tool calls",
            details={"tool_call": tool_call},
            http_status=502,
        )
    name = function.get("name")
    if name not in AGENT_TOOL_NAMES:
        raise FrontendError(
            "GEMMAFRONTEND_TOOL_UNSUPPORTED",
            "Gemma requested a tool outside the ContextGraph allowlist",
            "ollama.response.message.tool_calls[0].function.name",
            "choose only one of the declared ContextGraph tools",
            details={"requested": name, "allowlist": AGENT_TOOL_NAMES},
            http_status=502,
        )
    args = parse_tool_arguments(function.get("arguments"), "ollama.response.message.tool_calls[0].function.arguments")
    if name == "run_proof_pipeline":
        if args.get("scenario") != SCENARIO:
            raise FrontendError(
                "GEMMAFRONTEND_PROOF_SCENARIO_INVALID",
                "proof pipeline scenario did not match the only allowlisted scenario",
                "tool.arguments.scenario",
                f"use scenario={SCENARIO!r}",
                details={"arguments": args},
                http_status=502,
            )
        caps = args.get("requested_capabilities")
        if not isinstance(caps, list) or not all(isinstance(item, str) for item in caps):
            raise FrontendError(
                "GEMMAFRONTEND_PROOF_CAPABILITIES_INVALID",
                "requested_capabilities must be an array of strings",
                "tool.arguments.requested_capabilities",
                "request every required proof capability explicitly",
                details={"arguments": args},
                http_status=502,
            )
        missing = sorted(set(REQUIRED_CAPABILITIES) - set(caps))
        extra = sorted(set(caps) - set(REQUIRED_CAPABILITIES))
        if missing or extra:
            raise FrontendError(
                "GEMMAFRONTEND_PROOF_CAPABILITIES_MISMATCH",
                "proof pipeline capabilities did not exactly match the allowlist",
                "tool.arguments.requested_capabilities",
                "call the proof tool with the complete required capability set and no extras",
                details={"missing": missing, "extra": extra, "arguments": args},
                http_status=502,
            )
    if name == "evaluate_tool_policy" and args.get("mode") not in {"observe_only", "supervised", "bounded_autonomous", "blocked"}:
        raise FrontendError(
            "GEMMAFRONTEND_POLICY_MODE_INVALID",
            "policy mode is invalid",
            "tool.arguments.mode",
            "use observe_only, supervised, bounded_autonomous, or blocked",
            details={"arguments": args},
            http_status=502,
        )
    if name == "build_investor_category_pack":
        audience = args.get("audience")
        if not isinstance(audience, str) or not audience.strip():
            raise FrontendError(
                "GEMMAFRONTEND_CATEGORY_AUDIENCE_INVALID",
                "category pack audience must be a non-empty string",
                "tool.arguments.audience",
                "provide the investor, buyer, or technical audience for the pack",
                details={"arguments": args},
                http_status=502,
            )
        max_runs = args.get("max_runs", 8)
        if not isinstance(max_runs, int) or isinstance(max_runs, bool) or max_runs < 1 or max_runs > 12:
            raise FrontendError(
                "GEMMAFRONTEND_CATEGORY_MAX_RUNS_INVALID",
                "category pack max_runs must be an integer from 1 through 12",
                "tool.arguments.max_runs",
                "request between 1 and 12 real completed runs",
                details={"arguments": args},
                http_status=502,
            )
    return str(name), args


def artifact_record(root: Path, rel: str) -> dict[str, Any]:
    path = root / rel
    if not path.exists():
        raise FrontendError(
            "GEMMAFRONTEND_ARTIFACT_MISSING",
            "required run artifact is missing",
            f"artifacts.{rel}",
            "rerun the real harness from a clean root",
            source_of_truth=file_sot(path),
        )
    stat = path.stat()
    return {
        "name": rel,
        "path": str(path),
        "source_of_truth": file_sot(path),
        "sha256": sha256_file(path) if path.is_file() else None,
        "bytes": stat.st_size,
        "kind": "file" if path.is_file() else "directory",
    }


def inspect_ledger(ledger: Path, run_id: str | None) -> dict[str, Any]:
    if not ledger.exists():
        raise FrontendError(
            "GEMMAFRONTEND_LEDGER_MISSING",
            "SQLite ledger does not exist",
            "blackbox.ledger",
            "rerun the BlackBoxFix demo harness from a clean root",
            source_of_truth=ledger_sot(ledger),
        )
    try:
        conn = sqlite3.connect(ledger)
        conn.row_factory = sqlite3.Row
        table_exists = conn.execute(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='ledger_records'"
        ).fetchone()
        if not table_exists:
            raise FrontendError(
                "GEMMAFRONTEND_LEDGER_TABLE_MISSING",
                "ledger_records table is missing",
                "sqlite.ledger_records",
                "reinitialize the BlackBoxFix ledger and rerun the demo",
                source_of_truth=ledger_sot(ledger),
            )
        metadata: dict[str, str] = {}
        metadata_exists = conn.execute(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='blackbox_metadata'"
        ).fetchone()
        if metadata_exists:
            metadata = {
                str(row["key"]): str(row["value"])
                for row in conn.execute("SELECT key, value FROM blackbox_metadata ORDER BY key")
            }
        counts = {
            str(row["record_kind"]): int(row["count"])
            for row in conn.execute(
                "SELECT record_kind, COUNT(*) AS count FROM ledger_records GROUP BY record_kind ORDER BY record_kind"
            )
        }
        total = conn.execute("SELECT COUNT(*) FROM ledger_records").fetchone()[0]
        tip = conn.execute(
            "SELECT sequence, record_sha256, run_chain_sha256 FROM ledger_records ORDER BY sequence DESC LIMIT 1"
        ).fetchone()
        latest_rows = [
            dict(row)
            for row in conn.execute(
                "SELECT sequence, record_kind, run_id, record_id, payload_sha256, record_sha256 "
                "FROM ledger_records ORDER BY sequence DESC LIMIT 16"
            )
        ]
        run_counts: dict[str, int] = {}
        if run_id:
            run_counts = {
                str(row["record_kind"]): int(row["count"])
                for row in conn.execute(
                    "SELECT record_kind, COUNT(*) AS count FROM ledger_records WHERE run_id=? GROUP BY record_kind ORDER BY record_kind",
                    (run_id,),
                )
            }
        return {
            "source_of_truth": ledger_sot(ledger),
            "path": str(ledger),
            "sha256": sha256_file(ledger),
            "metadata": metadata,
            "schema_version": metadata.get("schema_version"),
            "physical_counts": counts,
            "run_counts": run_counts,
            "record_count": int(total),
            "tip": dict(tip) if tip else None,
            "latest_rows": latest_rows,
        }
    except sqlite3.Error as err:
        raise FrontendError(
            "GEMMAFRONTEND_SQLITE_READ_FAILED",
            f"failed to read SQLite ledger: {err}",
            "sqlite.ledger_records",
            "inspect ledger integrity and rerun from a clean root",
            source_of_truth=ledger_sot(ledger),
        ) from err
    finally:
        try:
            conn.close()
        except Exception:
            pass


def latest_payload(ledger: Path, run_id: str, kind: str) -> dict[str, Any] | None:
    try:
        conn = sqlite3.connect(ledger)
        conn.row_factory = sqlite3.Row
        row = conn.execute(
            "SELECT sequence, record_id, payload_json, payload_sha256, record_sha256 "
            "FROM ledger_records WHERE run_id=? AND record_kind=? ORDER BY sequence DESC LIMIT 1",
            (run_id, kind),
        ).fetchone()
        if not row:
            return None
        payload = json.loads(row["payload_json"])
        return {
            "sequence": row["sequence"],
            "record_id": row["record_id"],
            "payload_sha256": row["payload_sha256"],
            "record_sha256": row["record_sha256"],
            "payload": payload,
        }
    except (sqlite3.Error, json.JSONDecodeError) as err:
        raise FrontendError(
            "GEMMAFRONTEND_LEDGER_PAYLOAD_READ_FAILED",
            f"failed to read ledger payload: {err}",
            f"ledger_records.{kind}",
            "inspect the ledger row and payload JSON",
            source_of_truth=ledger_sot(ledger),
        ) from err
    finally:
        try:
            conn.close()
        except Exception:
            pass


def payloads_by_kind(ledger: Path, run_id: str, kind: str, *, limit: int = 10) -> list[dict[str, Any]]:
    try:
        conn = sqlite3.connect(ledger)
        conn.row_factory = sqlite3.Row
        rows = conn.execute(
            "SELECT sequence, record_id, payload_json, payload_sha256, record_sha256 "
            "FROM ledger_records WHERE run_id=? AND record_kind=? ORDER BY sequence DESC LIMIT ?",
            (run_id, kind, limit),
        ).fetchall()
        result = []
        for row in rows:
            result.append(
                {
                    "sequence": row["sequence"],
                    "record_id": row["record_id"],
                    "payload_sha256": row["payload_sha256"],
                    "record_sha256": row["record_sha256"],
                    "payload": json.loads(row["payload_json"]),
                }
            )
        return result
    except (sqlite3.Error, json.JSONDecodeError) as err:
        raise FrontendError(
            "GEMMAFRONTEND_LEDGER_PAYLOADS_READ_FAILED",
            f"failed to read ledger payloads: {err}",
            f"ledger_records.{kind}",
            "inspect the ledger rows and payload JSON",
            source_of_truth=ledger_sot(ledger),
        ) from err
    finally:
        try:
            conn.close()
        except Exception:
            pass


def collect_artifacts(root: Path) -> list[dict[str, Any]]:
    required = [
        "harness-report.json",
        "blackbox/blackbox.sqlite",
        "blackbox/exports/evidence.json",
        "blackbox/exports/fsv-evidence.json",
        "blackbox/exports/scorecard.json",
        "blackbox/exports/replay.json",
        "blackbox/exports/inspect-ledger.json",
        "blackbox/exports/fsv-certification/manifest.json",
        "blackbox/exports/dashboard/index.html",
        "dynamicjepa/pipeline-result.json",
        "dynamicjepa/exports/dynamicjepa-episodes.jsonl",
        "dynamicjepa/exports/dynamicjepa-episodes.manifest.json",
        "provenance/artifact-provenance.json",
        "intelligence/correlation-intelligence.json",
    ]
    return [artifact_record(root, rel) for rel in required]


def dynamic_counts(pipeline: dict[str, Any]) -> dict[str, int]:
    counts = pipeline.get("after_counts", {}).get("counts")
    if not isinstance(counts, dict):
        raise FrontendError(
            "GEMMAFRONTEND_DYNAMIC_COUNTS_MISSING",
            "DynamicJEPA pipeline artifact does not contain after_counts.counts",
            "dynamicjepa.after_counts.counts",
            "rerun verify-counter-world and regenerate pipeline-result.json",
        )
    return {str(key): int(value) for key, value in counts.items() if isinstance(value, int)}


def build_timeline(
    harness: dict[str, Any],
    scorecard: dict[str, Any],
    intelligence: dict[str, Any],
    ledger: dict[str, Any],
    reward: dict[str, Any] | None,
) -> list[dict[str, Any]]:
    calc = intelligence.get("calculated_intelligence", {})
    model = calc.get("model_behavior", {})
    verification = calc.get("verification", {})
    training = calc.get("training", {})
    planning = calc.get("planning", {})
    surprise = calc.get("surprise", {})
    counts = ledger.get("run_counts", {})
    score = scorecard.get("score", {})
    reward_payload = reward.get("payload") if reward else None
    reward_summary = None
    if isinstance(reward_payload, dict):
        reward_summary = {
            "sequence": reward.get("sequence"),
            "record_id": reward.get("record_id"),
            "payload_sha256": reward.get("payload_sha256"),
            "record_sha256": reward.get("record_sha256"),
            "reward": reward_payload.get("reward"),
            "loss_convention": "loss = -reward so the same scalar can be minimized by training code",
            "components": [
                {
                    "id": item.get("id"),
                    "feature": item.get("feature"),
                    "value": item.get("value"),
                    "weight": item.get("weight"),
                    "contribution": item.get("contribution"),
                }
                for item in reward_payload.get("components", [])
                if isinstance(item, dict)
            ],
            "training_sample_kind": reward_payload.get("training_sample", {}).get("sample_kind"),
        }
    reward_blocked_by_schema = (
        reward is None
        and ledger.get("schema_version") is not None
        and ledger.get("schema_version") != REWARD_LEDGER_SCHEMA_VERSION
    )
    timeline = [
        {
            "stage": "Gemma",
            "status": "passed",
            "title": "Native tool call requested",
            "value": model.get("tool_name"),
            "meaning": "Gemma produced a structured tool call instead of a prose-only answer.",
            "evidence": {
                "prompt_sha256": model.get("prompt_sha256"),
                "tool_call_argument_status": model.get("tool_call_argument_status"),
            },
        },
        {
            "stage": "Harness",
            "status": "passed",
            "title": "Arguments validated against the allowlist",
            "value": f"{model.get('requested_capability_count', 0)} capabilities requested",
            "meaning": "The app executed only the narrow ContextGraph tool contract.",
            "evidence": harness.get("model", {}).get("tool_call"),
        },
        {
            "stage": "DynamicJEPA",
            "status": "passed",
            "title": "Real state, training, planning, and surprise recorded",
            "value": f"{training.get('dataset_rows_total', 0)} dataset rows, {surprise.get('surprise_events', 0)} surprise event",
            "meaning": "The system wrote and read back operational memory from the DynamicJEPA source of truth.",
            "evidence": {
                "raw_events": calc.get("data_yield", {}).get("raw_events"),
                "latent_panels": calc.get("data_yield", {}).get("latent_panels"),
                "selected_highest_utility_action": planning.get("selected_highest_utility_action"),
            },
        },
        {
            "stage": "BlackBoxFix",
            "status": "incident" if counts.get("incident_packet", 0) else "passed",
            "title": "Ledger, verifier, evidence, and incident rows persisted",
            "value": f"{counts.get('verifier_result', 0)} verifier rows, {counts.get('incident_packet', 0)} incident rows",
            "meaning": "The intentional bad verifier created an incident, proving mismatch detection works.",
            "evidence": {
                "source_of_truth_coverage": verification.get("source_of_truth_coverage"),
                "readback_ratio": verification.get("readback_ratio"),
                "verifier_pass_rate": verification.get("verifier_pass_rate"),
                "verifier_failure_rate": verification.get("verifier_failure_rate"),
            },
        },
        {
            "stage": "Scorecard",
            "status": "incident" if score.get("trust_level") == "incident" else "passed",
            "title": "Outcome scored from physical evidence",
            "value": f"trust={score.get('trust_level')} risk={score.get('risk')}",
            "meaning": "The scorecard grades the run from ledger/readback state, not model confidence.",
            "evidence": score,
        },
        {
            "stage": "Reward",
            "status": "passed" if reward else ("blocked" if reward_blocked_by_schema else "missing"),
            "title": (
                "Reward/loss sample materialized"
                if reward
                else (
                    "Reward blocked by stale ledger schema"
                    if reward_blocked_by_schema
                    else "Reward not materialized for this run yet"
                )
            ),
            "value": (
                f"reward={reward['payload'].get('reward', {}).get('raw')} loss={reward['payload'].get('reward', {}).get('loss')}"
                if reward
                else (
                    f"ledger schema={ledger.get('schema_version')} reward requires={REWARD_LEDGER_SCHEMA_VERSION}"
                    if reward_blocked_by_schema
                    else "run live or request reward compilation"
                )
            ),
            "meaning": "The reward compiler turns verified outcome features into a trainable signal.",
            "evidence": reward_summary or {"ledger_schema_version": ledger.get("schema_version"), "required_schema_version": REWARD_LEDGER_SCHEMA_VERSION},
        },
    ]
    return timeline


def proof_status(value: bool | None) -> str:
    if value is True:
        return "passed"
    if value is False:
        return "incident"
    return "missing"


def build_end_user_proof(
    root: Path,
    harness: dict[str, Any],
    scorecard: dict[str, Any],
    intelligence: dict[str, Any],
    ledger: dict[str, Any],
    reward: dict[str, Any] | None,
    artifacts: list[dict[str, Any]],
    source_of_truth: dict[str, Any],
    model_io: dict[str, Any],
) -> dict[str, Any]:
    calc = intelligence.get("calculated_intelligence", {})
    model = calc.get("model_behavior", {})
    verification = calc.get("verification", {})
    training = calc.get("training", {})
    planning = calc.get("planning", {})
    surprise = calc.get("surprise", {})
    counts = ledger.get("run_counts", {})
    score = scorecard.get("score", {})
    score_inputs = scorecard.get("score_inputs", {})
    reward_payload = reward.get("payload", {}).get("reward") if reward else None
    artifact_total = len(artifacts)
    artifact_existing = sum(1 for item in artifacts if item.get("exists"))
    verifier_total = int(counts.get("verifier_result", 0) or 0)
    verifier_failed = int(score_inputs.get("verifier_status_counts", {}).get("failed", 0) or 0)
    incident_count = int(counts.get("incident_packet", 0) or 0)
    readback_ratio = verification.get("readback_ratio")
    sot_coverage = verification.get("source_of_truth_coverage")
    evidence_hash_status = score_inputs.get("evidence_hash_verification", {}).get("status")
    selected_tool = model.get("tool_name") or harness.get("model", {}).get("tool_call", {}).get("function", {}).get("name")
    policy_compliance = score.get("policy_compliance") or score_inputs.get("policy_compliance")
    trust_level = score.get("trust_level")
    clean_success = (
        trust_level in {"verified", "verified_with_policy"}
        and incident_count == 0
        and verifier_failed == 0
    )
    incident_demo = incident_count > 0
    status = "verified" if clean_success else ("incident_caught" if incident_demo else "incomplete")
    if clean_success:
        headline = "The model action was verified against durable source-of-truth state."
        plain_result = "Every verifier passed and no incident packet exists for this run."
    elif incident_demo:
        headline = "ContextGraph caught a real mismatch and preserved the proof."
        plain_result = (
            "This run contains at least one failed verifier row and at least one incident packet. "
            "That means the system did not merely trust the model's answer; it compared expected state with readback state and recorded the mismatch."
        )
    else:
        headline = "The run has not reached a clean verified state yet."
        plain_result = "Some proof inputs are missing, blocked, or not yet compiled."

    proof_cards = [
        {
            "label": "Model action",
            "value": selected_tool or "n/a",
            "status": proof_status(bool(selected_tool) if selected_tool else None),
            "meaning": "The selected model had to choose a declared ContextGraph tool instead of only producing prose.",
            "source": "ollama native tool call + harness-report.json",
        },
        {
            "label": "SQLite ledger rows",
            "value": ledger.get("record_count"),
            "status": proof_status((ledger.get("record_count") or 0) > 0),
            "meaning": "The BlackBoxFix ledger is a physical SQLite table read separately by the frontend.",
            "source": ledger.get("source_of_truth"),
        },
        {
            "label": "Source-of-truth coverage",
            "value": sot_coverage,
            "format": "ratio",
            "status": proof_status(sot_coverage == 1),
            "meaning": "Actions were checked against durable files, command output, SQLite, RocksDB, or artifact hashes.",
            "source": "scorecard.score_inputs + correlation intelligence",
        },
        {
            "label": "Verifier readback",
            "value": readback_ratio,
            "format": "ratio",
            "status": proof_status(readback_ratio == 1),
            "meaning": "Verifier rows contain actual readback evidence, not just return values.",
            "source": ledger.get("source_of_truth"),
        },
        {
            "label": "Incident detection",
            "value": incident_count,
            "status": "incident" if incident_count else "passed",
            "meaning": "A nonzero value shows the system caught and persisted a mismatch packet.",
            "source": ledger.get("source_of_truth"),
        },
        {
            "label": "Training signal",
            "value": training.get("dataset_rows_total"),
            "status": proof_status((training.get("dataset_rows_total") or 0) > 0),
            "meaning": "Verified work was converted into action-conditioned training/eval material.",
            "source": source_of_truth.get("dynamicjepa_episodes"),
        },
        {
            "label": "Reward",
            "value": reward_payload.get("raw") if isinstance(reward_payload, dict) else None,
            "status": proof_status(isinstance(reward_payload, dict)),
            "meaning": "The reward compiler converts verified outcome features into a trainable scalar.",
            "source": ledger.get("source_of_truth"),
        },
        {
            "label": "Artifact hashes",
            "value": f"{artifact_existing}/{artifact_total}",
            "status": proof_status(artifact_total > 0 and artifact_existing == artifact_total),
            "meaning": "Expected output files were read back from disk and hashed.",
            "source": str(root),
        },
    ]

    self_verify_steps = [
        {
            "title": "Read the ledger",
            "what_to_check": "Open the SQLite ledger and count ledger_records. The UI count should match the physical table.",
            "source_of_truth": ledger.get("source_of_truth"),
        },
        {
            "title": "Inspect the run artifacts",
            "what_to_check": "The listed files must exist on disk with the displayed byte counts and SHA-256 hashes.",
            "source_of_truth": file_sot(root),
        },
        {
            "title": "Check the DynamicJEPA state",
            "what_to_check": "The pipeline result and RocksDB path show state, panel, planning, surprise, and training outputs.",
            "source_of_truth": source_of_truth.get("dynamicjepa_rocksdb"),
        },
        {
            "title": "Check verifier evidence",
            "what_to_check": "Verifier rows contain expected deltas and actual readback data; failed verifier rows create incidents.",
            "source_of_truth": ledger.get("source_of_truth"),
        },
    ]

    questions = [
        {
            "question": "Did the model really trigger an action?",
            "answer": f"Yes. The model selected `{selected_tool}` and ContextGraph executed only after validating that tool call."
            if selected_tool
            else "No selected tool was found in the current snapshot.",
        },
        {
            "question": "Where is reality checked?",
            "answer": "In the source-of-truth readbacks: SQLite ledger rows, DynamicJEPA RocksDB/files, command verifier output, provenance files, and hashed artifacts.",
        },
        {
            "question": "What did the system catch?",
            "answer": (
                f"It caught {incident_count} incident packet(s) from {verifier_failed} failed verifier row(s)."
                if incident_count
                else "No incident packet is present in this run."
            ),
        },
        {
            "question": "What can this become for training?",
            "answer": (
                f"It produced reward={reward_payload.get('raw')} and loss={reward_payload.get('loss')} from verified scorecard features."
                if isinstance(reward_payload, dict)
                else "Reward has not been compiled for this snapshot."
            ),
        },
    ]

    return {
        "status": status,
        "headline": headline,
        "plain_result": plain_result,
        "honesty_note": (
            "An incident state is not a hidden failure. It is the product proving that it can detect and preserve evidence of a mismatch."
            if incident_demo
            else "Clean verified status requires source-of-truth readbacks, verifier pass evidence, and no incident packets."
        ),
        "model": harness.get("model", {}).get("name"),
        "operator_prompt_sha256": model_io.get("operator_request_sha256") or model.get("prompt_sha256"),
        "run_id": harness.get("blackbox_run_id"),
        "source_root": str(root),
        "trust_level": trust_level,
        "policy_compliance": policy_compliance,
        "verifier_total": verifier_total,
        "verifier_failed": verifier_failed,
        "evidence_hash_verification": evidence_hash_status,
        "planning": {
            "selected_highest_utility_action": planning.get("selected_highest_utility_action"),
        },
        "surprise": {
            "events": surprise.get("surprise_events"),
            "latest_kind": surprise.get("latest_kind"),
        },
        "proof_cards": proof_cards,
        "self_verify_steps": self_verify_steps,
        "questions": questions,
    }


def build_snapshot(root: Path, *, frontend_context: dict[str, Any] | None = None) -> dict[str, Any]:
    harness = load_json(root / "harness-report.json", "harness_report")
    pipeline = load_json(root / "dynamicjepa/pipeline-result.json", "dynamicjepa.pipeline_result")
    scorecard_export = load_json(root / "blackbox/exports/scorecard.json", "blackbox.scorecard")
    evidence = load_json(root / "blackbox/exports/evidence.json", "blackbox.evidence")
    provenance = load_json(root / "provenance/artifact-provenance.json", "provenance")
    intelligence = load_json(root / "intelligence/correlation-intelligence.json", "intelligence")

    run_id = harness.get("blackbox_run_id")
    if not isinstance(run_id, str) or not run_id:
        raise FrontendError(
            "GEMMAFRONTEND_RUN_ID_MISSING",
            "harness report does not contain blackbox_run_id",
            "harness_report.blackbox_run_id",
            "rerun the harness so BlackBoxFix emits a run id",
            source_of_truth=file_sot(root / "harness-report.json"),
        )
    ledger_path = root / "blackbox/blackbox.sqlite"
    ledger = inspect_ledger(ledger_path, run_id)
    latest_scorecard = latest_payload(ledger_path, run_id, "scorecard")
    scorecard = latest_scorecard["payload"] if latest_scorecard else scorecard_export
    reward = latest_payload(ledger_path, run_id, "reward_compilation")
    artifacts = collect_artifacts(root)
    calc = intelligence.get("calculated_intelligence", {})
    counts = dynamic_counts(pipeline)
    source_of_truth = harness.get("source_of_truth", {})
    model_io = harness.get("model_io", {})
    end_user_proof = build_end_user_proof(
        root,
        harness,
        scorecard,
        intelligence,
        ledger,
        reward,
        artifacts,
        source_of_truth,
        model_io,
    )
    tool_contract = model_io.get("tool_contract") if isinstance(model_io.get("tool_contract"), dict) else {}
    actual_model_message = (
        model_io.get("actual_model_message")
        if isinstance(model_io.get("actual_model_message"), dict)
        else {
            "role": "assistant",
            "content_redacted_preview": "",
            "tool_call_count": 1 if harness.get("model", {}).get("tool_call") else 0,
            "tool_calls": [harness.get("model", {}).get("tool_call")] if harness.get("model", {}).get("tool_call") else [],
        }
    )

    snapshot = {
        "status": "ok",
        "generated_at": now_rfc3339(),
        "source_root": str(root),
        "source_root_sha256_basis": intelligence.get("source_root_sha256_basis"),
        "frontend_context": frontend_context or {},
        "identity": {
            "model": harness.get("model", {}).get("name"),
            "provider": harness.get("model", {}).get("provider"),
            "dynamic_run_id": harness.get("dynamic_run_id"),
            "blackbox_run_id": run_id,
            "tool_call_id": harness.get("tool_call_id"),
            "prompt_sha256": model_io.get("prompt_sha256") or calc.get("model_behavior", {}).get("prompt_sha256"),
            "operator_request_sha256": model_io.get("operator_request_sha256"),
            "operator_request_redacted_preview": model_io.get("operator_request_redacted_preview"),
        },
        "model_interaction": {
            "truth_statement": "Gemma requests a native tool call; ContextGraph validates and executes the allowlisted tool, then reads source-of-truth evidence.",
            "operator_prompt_redacted_preview": model_io.get("operator_request_redacted_preview")
            or (frontend_context or {}).get("operator_prompt_redacted_preview"),
            "operator_prompt_sha256": model_io.get("operator_request_sha256")
            or (frontend_context or {}).get("operator_prompt_sha256"),
            "prompt_sha256": model_io.get("prompt_sha256") or calc.get("model_behavior", {}).get("prompt_sha256"),
            "system_message_redacted_preview": model_io.get("system_message_redacted_preview"),
            "tool_prompt_redacted_preview": model_io.get("tool_prompt_redacted_preview"),
            "actual_model_message": actual_model_message,
            "tool_contract": {
                "mode": tool_contract.get("mode", "single_allowlisted_mutating_tool"),
                "available_tool_names": tool_contract.get("available_tool_names", ["run_dynamicjepa_blackboxfix_pipeline"]),
                "required_scenario": tool_contract.get("required_scenario", "counter_world_full_agentops"),
                "required_capabilities": tool_contract.get("required_capabilities", []),
                "unsupported_tool_behavior": tool_contract.get("unsupported_tool_behavior", "fail_closed"),
                "executor": tool_contract.get(
                    "executor",
                    "ContextGraph harness executes the tool after validation; Gemma only requests it.",
                ),
            },
        },
        "source_of_truth": {
            "ledger": source_of_truth.get("blackbox_ledger") or ledger_sot(ledger_path),
            "dynamicjepa_rocksdb": source_of_truth.get("dynamicjepa_rocksdb") or pipeline.get("source_of_truth", {}).get("rocksdb"),
            "dynamicjepa_episodes": source_of_truth.get("dynamicjepa_episodes") or pipeline.get("source_of_truth", {}).get("dynamicjepa_episodes"),
            "provenance": source_of_truth.get("provenance") or pipeline.get("source_of_truth", {}).get("provenance"),
        },
        "system_map": {
            "nodes": ["Gemma", "ContextGraph Harness", "DynamicJEPA", "BlackBoxFix", "Evidence", "Reward"],
            "edges": [
                {"from": "Gemma", "to": "ContextGraph Harness", "label": "native tool call"},
                {"from": "ContextGraph Harness", "to": "DynamicJEPA", "label": "validated action"},
                {"from": "DynamicJEPA", "to": "BlackBoxFix", "label": "expected delta + state readbacks"},
                {"from": "BlackBoxFix", "to": "Evidence", "label": "ledger, verifier, incident, FSV"},
                {"from": "Evidence", "to": "Reward", "label": "scorecard -> reward/loss"},
            ],
        },
        "end_user_proof": end_user_proof,
        "timeline": build_timeline(harness, scorecard, intelligence, ledger, reward),
        "metrics": {
            "verification": calc.get("verification", {}),
            "governance": calc.get("governance", {}),
            "data_yield": calc.get("data_yield", {}),
            "training": calc.get("training", {}),
            "planning": calc.get("planning", {}),
            "surprise": calc.get("surprise", {}),
            "training_episode_yield": calc.get("training_episode_yield", {}),
            "score": scorecard.get("score", {}),
            "score_inputs": scorecard.get("score_inputs", {}),
            "dynamic_counts": counts,
            "reward": reward.get("payload", {}).get("reward") if reward else None,
            "scorecard_source": {
                "kind": "ledger_latest_row" if latest_scorecard else "export_file",
                "source_of_truth": ledger_sot(ledger_path) if latest_scorecard else file_sot(root / "blackbox/exports/scorecard.json"),
                "sequence": latest_scorecard.get("sequence") if latest_scorecard else None,
                "record_id": latest_scorecard.get("record_id") if latest_scorecard else None,
            },
        },
        "ledger": ledger,
        "correlations": intelligence.get("correlations", []),
        "current_gaps": intelligence.get("current_gaps", []),
        "data_inventory": intelligence.get("data_inventory", []),
        "artifacts": artifacts,
        "provenance": {
            "format": provenance.get("format"),
            "agents": len(provenance.get("agents", [])),
            "entities": len(provenance.get("entities", [])),
            "activities": len(provenance.get("activities", [])),
            "relations": len(provenance.get("relations", [])),
        },
        "evidence_summary": {
            "format": evidence.get("format"),
            "source_of_truth": evidence.get("source_of_truth"),
            "pdf_ready": evidence.get("pdf_ready"),
        },
    }
    return snapshot


def append_index(runtime_root: Path, entry: dict[str, Any]) -> None:
    runtime_root.mkdir(parents=True, exist_ok=True)
    path = runtime_root / "frontend-run-index.jsonl"
    data = json.dumps(entry, sort_keys=True) + "\n"
    try:
        with path.open("ab") as handle:
            handle.write(data.encode("utf-8"))
            handle.flush()
            os.fsync(handle.fileno())
    except OSError as err:
        raise FrontendError(
            "GEMMAFRONTEND_INDEX_WRITE_FAILED",
            f"failed to append frontend run index: {err}",
            "runtime.frontend-run-index.jsonl",
            "check disk space and runtime directory permissions",
            source_of_truth=file_sot(path),
            http_status=500,
        ) from err


def persist_snapshot(runtime_root: Path, snapshot: dict[str, Any], mode: str) -> dict[str, Any]:
    run_id = f"frontend_{time.time_ns()}_{hashlib.sha256((mode + snapshot['source_root']).encode()).hexdigest()[:8]}"
    out_dir = runtime_root / "frontend-runs" / run_id
    snapshot_path = out_dir / "system-snapshot.json"
    entry = {
        "frontend_run_id": run_id,
        "created_at": now_rfc3339(),
        "mode": mode,
        "source_root": snapshot["source_root"],
        "snapshot_path": str(snapshot_path),
        "blackbox_run_id": snapshot["identity"]["blackbox_run_id"],
        "status": snapshot["status"],
        "source_of_truth": file_sot(snapshot_path),
    }
    snapshot["frontend_persistence"] = entry
    write_json(snapshot_path, snapshot)
    append_index(runtime_root, entry)
    return snapshot


def scan_real_runs(runtime_root: Path | None = None) -> list[dict[str, Any]]:
    candidates: dict[Path, float] = {}
    search_roots = [Path("/tmp")]
    if runtime_root is not None:
        search_roots.append(runtime_root / "live-runs")
    for base in search_roots:
        if not base.exists():
            continue
        for candidate in base.glob("blackboxfix-agentops-demo*"):
            try:
                if not candidate.is_dir():
                    continue
                if not (candidate / ".blackboxfix_agentops_demo_root").exists():
                    continue
                candidates[candidate.resolve()] = candidate.stat().st_mtime
            except OSError:
                continue
    roots = []
    for candidate, mtime in sorted(candidates.items(), key=lambda item: item[1], reverse=True):
        harness = candidate / "harness-report.json"
        complete = (
            harness.exists()
            and (candidate / "blackbox/blackbox.sqlite").exists()
            and (candidate / "dynamicjepa/pipeline-result.json").exists()
            and (candidate / "provenance/artifact-provenance.json").exists()
        )
        entry = {
            "root": str(candidate),
            "complete": complete,
            "mtime": mtime,
            "source_of_truth": file_sot(candidate),
        }
        if complete:
            try:
                report = load_json(harness, "harness_report")
                entry["blackbox_run_id"] = report.get("blackbox_run_id")
                entry["model"] = report.get("model", {}).get("name")
                entry["has_intelligence"] = (candidate / "intelligence/correlation-intelligence.json").exists()
                entry["has_reward"] = (candidate / "blackbox/blackbox.sqlite").exists()
            except FrontendError:
                entry["complete"] = False
        roots.append(entry)
        if len(roots) >= 30:
            break
    return roots


def latest_complete_run(runtime_root: Path) -> Path:
    for item in scan_real_runs(runtime_root):
        if item.get("complete"):
            root = Path(str(item["root"])).resolve()
            if not (root / "intelligence/correlation-intelligence.json").exists():
                raise FrontendError(
                    "GEMMAFRONTEND_LATEST_RUN_INTELLIGENCE_MISSING",
                    "latest completed run is missing correlation intelligence artifact",
                    "latest_run.intelligence",
                    "inspect a run created by the current frontend or rerun the intelligence materializer",
                    source_of_truth=file_sot(root / "intelligence/correlation-intelligence.json"),
                    details={"root": str(root)},
                    http_status=409,
                )
            return root
    raise FrontendError(
        "GEMMAFRONTEND_NO_COMPLETE_RUNS",
        "no completed real ContextGraph run exists",
        "runtime.real_runs",
        "run the full proof pipeline first, then ask read-only evidence questions",
        source_of_truth=file_sot(runtime_root),
        http_status=409,
    )


def materialize_intelligence(root: Path, *, client_id: str | None = None, request_id: str | None = None) -> None:
    script = REPO_ROOT / "scripts/blackboxfix_demo_intelligence_report.py"
    check_required_file(script, "scripts.blackboxfix_demo_intelligence_report")
    publish(
        "intelligence",
        "materializing correlation intelligence from run artifacts",
        details={"root": str(root)},
        client_id=client_id,
        request_id=request_id,
    )
    run_process([sys.executable, str(script), "--root", str(root)], timeout=120.0)
    artifact_record(root, "intelligence/correlation-intelligence.json")


def compile_reward(
    root: Path,
    model: str,
    *,
    client_id: str | None = None,
    request_id: str | None = None,
) -> dict[str, Any]:
    harness = load_json(root / "harness-report.json", "harness_report")
    run_id = harness.get("blackbox_run_id")
    if not isinstance(run_id, str) or not run_id:
        raise FrontendError(
            "GEMMAFRONTEND_REWARD_RUN_ID_MISSING",
            "cannot compile reward without blackbox_run_id",
            "harness_report.blackbox_run_id",
            "rerun the harness and ensure the BlackBoxFix run id is persisted",
            source_of_truth=file_sot(root / "harness-report.json"),
        )
    ledger = root / "blackbox/blackbox.sqlite"
    existing = latest_payload(ledger, run_id, "reward_compilation")
    if existing:
        return existing
    ledger_state = inspect_ledger(ledger, run_id)
    if ledger_state.get("schema_version") != REWARD_LEDGER_SCHEMA_VERSION:
        raise FrontendError(
            "GEMMAFRONTEND_REWARD_SCHEMA_INCOMPATIBLE",
            "reward compilation requires a fresh schema-14 BlackBoxFix ledger",
            "blackbox_metadata.schema_version",
            "run a fresh live demo with the current context-graph binary, then compile reward on that run",
            source_of_truth=ledger_sot(ledger),
            details={
                "actual_schema_version": ledger_state.get("schema_version"),
                "required_schema_version": REWARD_LEDGER_SCHEMA_VERSION,
            },
            http_status=409,
        )
    check_required_file(REPO_ROOT / "target/debug/context-graph", "target.debug.context-graph")
    spec = APP_ROOT / "reward_spec.verified_outcome.json"
    check_required_file(spec, "gemmafrontend.reward_spec")
    publish(
        "reward",
        "compiling reward/loss from the scorecard and ledger",
        details={"run_id": run_id},
        client_id=client_id,
        request_id=request_id,
    )
    run_process(
        [
            str(REPO_ROOT / "target/debug/context-graph"),
            "blackbox",
            "reward",
            "compile",
            "--ledger",
            str(ledger),
            "--run-id",
            run_id,
            "--input",
            str(spec),
            "--tenant",
            "tenant_local",
            "--agent",
            "gemmafrontend_reward_compiler",
            "--model",
            model,
        ],
        timeout=120.0,
    )
    compiled = latest_payload(ledger, run_id, "reward_compilation")
    if not compiled:
        raise FrontendError(
            "GEMMAFRONTEND_REWARD_READBACK_MISSING",
            "reward compile command succeeded but no reward_compilation row was physically readable",
            "ledger_records.reward_compilation",
            "inspect the reward compiler readback path before trusting the result",
            source_of_truth=ledger_sot(ledger),
            http_status=500,
        )
    return compiled


def ensure_live_preflight(runtime_root: Path, model: str, ollama_url: str) -> None:
    check_required_file(REPO_ROOT / "scripts/blackboxfix_gemma_tool_demo.py", "scripts.blackboxfix_gemma_tool_demo")
    check_required_file(REPO_ROOT / "scripts/blackboxfix_demo_intelligence_report.py", "scripts.blackboxfix_demo_intelligence_report")
    check_required_file(REPO_ROOT / "target/debug/context-graph", "target.debug.context-graph")
    runtime_root.mkdir(parents=True, exist_ok=True)
    usage = shutil.disk_usage(runtime_root)
    if usage.free < MIN_FREE_BYTES_FOR_LIVE_RUN:
        raise FrontendError(
            "GEMMAFRONTEND_DISK_SPACE_LOW",
            "not enough free disk space for a live model + DynamicJEPA run",
            "runtime_root.free_bytes",
            "free disk space before starting a live run",
            source_of_truth=file_sot(runtime_root),
            details={"free_bytes": usage.free, "required_bytes": MIN_FREE_BYTES_FOR_LIVE_RUN},
            http_status=507,
        )
    tags = http_json(f"{ollama_url.rstrip('/')}/api/tags", timeout=5.0)
    models = [item.get("name") for item in tags.get("models", []) if isinstance(item, dict)]
    if model not in models:
        raise FrontendError(
            "GEMMAFRONTEND_OLLAMA_MODEL_MISSING",
            "configured model is not present in Ollama",
            "ollama.models",
            "pull/load the exact model tag before running the live demo",
            source_of_truth=f"{ollama_url.rstrip('/')}/api/tags",
            details={"model": model, "available_models": models},
            http_status=503,
        )


def run_live_demo(
    runtime_root: Path,
    message: str,
    model: str,
    ollama_url: str,
    *,
    compile_reward_after: bool,
    client_id: str | None,
    request_id: str | None,
    prevalidated_tool_call: dict[str, Any] | None = None,
) -> dict[str, Any]:
    prompt = validate_prompt(message)
    ensure_live_preflight(runtime_root, model, ollama_url)
    root = runtime_root / "live-runs" / f"blackboxfix-agentops-demo-ui-{int(time.time())}-{hashlib.sha256(prompt.encode()).hexdigest()[:8]}"
    publish(
        "chat",
        "accepted operator prompt",
        details={"prompt_sha256": sha256_bytes(prompt.encode()), "preview": redact_preview(prompt)},
        client_id=client_id,
        request_id=request_id,
    )
    command = [
        sys.executable,
        str(REPO_ROOT / "scripts/blackboxfix_gemma_tool_demo.py"),
        "--root",
        str(root),
        "--model",
        model,
        "--ollama-url",
        ollama_url,
        "--blackbox-bin",
        str(REPO_ROOT / "target/debug/context-graph"),
        "--user-message",
        prompt,
    ]
    if prevalidated_tool_call is not None:
        prevalidated_path = runtime_root / "prevalidated-tool-calls" / f"{request_id or time.time_ns()}.json"
        write_json(prevalidated_path, prevalidated_tool_call)
        readback = load_json(prevalidated_path, "prevalidated_tool_call")
        if readback.get("tool_call", {}).get("function", {}).get("name") != INTERNAL_PROOF_TOOL_NAME:
            raise FrontendError(
                "GEMMAFRONTEND_PREVALIDATED_TOOL_READBACK_MISMATCH",
                "prevalidated tool-call readback did not match the proof tool",
                "prevalidated_tool_call.tool_call.function.name",
                "fix the prevalidated tool-call writer before executing the harness",
                source_of_truth=file_sot(prevalidated_path),
                details={"readback": readback},
                http_status=500,
            )
        command.extend(["--prevalidated-tool-call", str(prevalidated_path)])
    publish(
        "gemma",
        (
            "executing prevalidated model tool call through the proof harness"
            if prevalidated_tool_call is not None
            else "calling Ollama and requiring one native tool call"
        ),
        details={"model": model, "prevalidated": prevalidated_tool_call is not None},
        client_id=client_id,
        request_id=request_id,
    )
    run_process(command, timeout=1800.0)
    publish(
        "harness",
        "live harness completed; reading generated artifacts",
        details={"root": str(root)},
        client_id=client_id,
        request_id=request_id,
    )
    materialize_intelligence(root, client_id=client_id, request_id=request_id)
    if compile_reward_after:
        compile_reward(root, model, client_id=client_id, request_id=request_id)
    snapshot = build_snapshot(
        root,
        frontend_context={
            "mode": "live",
            "operator_prompt_sha256": sha256_bytes(prompt.encode()),
            "operator_prompt_redacted_preview": redact_preview(prompt),
        },
    )
    persisted = persist_snapshot(runtime_root, snapshot, "live")
    publish(
        "evidence",
        "system snapshot persisted after source-of-truth readback",
        details=persisted["frontend_persistence"],
        client_id=client_id,
        request_id=request_id,
    )
    return persisted


def inspect_real_run(
    runtime_root: Path,
    root: Path,
    model: str,
    *,
    compile_reward_requested: bool,
    client_id: str | None,
    request_id: str | None,
) -> dict[str, Any]:
    publish(
        "inspect",
        "reading completed real run artifacts",
        details={"root": str(root)},
        client_id=client_id,
        request_id=request_id,
    )
    if compile_reward_requested:
        compile_reward(root, model, client_id=client_id, request_id=request_id)
    snapshot = build_snapshot(root, frontend_context={"mode": "inspect"})
    persisted = persist_snapshot(runtime_root, snapshot, "inspect")
    publish(
        "inspect",
        "inspection snapshot persisted after SQLite/file readback",
        details=persisted["frontend_persistence"],
        client_id=client_id,
        request_id=request_id,
    )
    return persisted


def ensure_agent_preflight(runtime_root: Path, model: str, ollama_url: str) -> None:
    runtime_root.mkdir(parents=True, exist_ok=True)
    tags = http_json(f"{ollama_url.rstrip('/')}/api/tags", timeout=5.0)
    models = [item.get("name") for item in tags.get("models", []) if isinstance(item, dict)]
    if model not in models:
        raise FrontendError(
            "GEMMAFRONTEND_OLLAMA_MODEL_MISSING",
            "configured model is not present in Ollama",
            "ollama.models",
            "pull/load the exact model tag before running the agent demo",
            source_of_truth=f"{ollama_url.rstrip('/')}/api/tags",
            details={"model": model, "available_models": models},
            http_status=503,
        )


def call_agent_tool_router(
    prompt: str,
    model: str,
    ollama_url: str,
    *,
    client_id: str | None,
    request_id: str | None,
) -> dict[str, Any]:
    system = (
        "You are the ContextGraph tool router. Choose exactly one provided tool when the operator asks about "
        "ContextGraph, BlackBoxFix, DynamicJEPA, evidence, incidents, reward, policy, reports, or comparing runs. "
        "Use run_proof_pipeline only when the operator asks to run or create the full proof. "
        "Use inspect_latest_run for read-only evidence questions. Use explain_scorecard for trust/risk scoring. "
        "Use analyze_incident for failures/incidents. Use suggest_reward_improvements for optimization. "
        "Use compare_recent_runs for comparisons. Use export_buyer_report for buyer-facing report generation. "
        "Use evaluate_tool_policy for approval, authorization, or policy questions. "
        "Use build_investor_category_pack for investor demos, product category, model arena, leaderboard, "
        "failure injection, governance packs, signed attestations, or executive views. "
        "Do not ask for optional root paths; tools use the latest completed real run or the latest two completed real runs when roots are omitted. "
        "Do not invent tool results and do not answer in prose before the tool call. "
        "Emit a native tool call in the tool_calls field; never write function-call syntax as plain text."
    )
    publish(
        "model",
        "asking selected model to choose one ContextGraph tool",
        details={"model": model, "available_tools": AGENT_TOOL_NAMES, "prompt_sha256": sha256_bytes(prompt.encode())},
        client_id=client_id,
        request_id=request_id,
    )
    body = {
        "model": model,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": prompt},
        ],
        "tools": AGENT_TOOL_DEFINITIONS,
        "stream": False,
        "think": False,
        "options": {"temperature": 0},
    }
    response = http_post_json(f"{ollama_url.rstrip('/')}/api/chat", body, timeout=300.0)
    message = response.get("message")
    if not isinstance(message, dict):
        raise FrontendError(
            "GEMMAFRONTEND_MODEL_RESPONSE_INVALID",
            "Ollama chat response did not contain a message object",
            "ollama.response.message",
            "inspect Ollama chat API compatibility",
            source_of_truth=f"{ollama_url.rstrip('/')}/api/chat",
            details={"response": response},
            http_status=502,
        )
    tool_calls = message.get("tool_calls")
    if not isinstance(tool_calls, list) or not tool_calls:
        raise FrontendError(
            "GEMMAFRONTEND_MODEL_TOOL_CALL_MISSING",
            "selected model did not emit a native ContextGraph tool call",
            "ollama.response.message.tool_calls",
            "ask a ContextGraph evidence question or use a model/runtime with native tool-calling support",
            source_of_truth=f"{ollama_url.rstrip('/')}/api/chat",
            details={"response": response},
            http_status=502,
        )
    if len(tool_calls) != 1:
        raise FrontendError(
            "GEMMAFRONTEND_MODEL_TOOL_CALL_COUNT_INVALID",
            "selected model emitted more than one tool call; this UI executes one audited action per turn",
            "ollama.response.message.tool_calls",
            "ask a narrower question so one source-of-truth action is verified at a time",
            details={"tool_call_count": len(tool_calls), "tool_calls": tool_calls},
            http_status=502,
        )
    tool_name, args = validate_agent_tool_call(tool_calls[0])
    content = message.get("content") if isinstance(message.get("content"), str) else ""
    router = {
        "system_message_redacted_preview": redact_preview(system, limit=360),
        "operator_prompt_sha256": sha256_bytes(prompt.encode()),
        "operator_prompt_redacted_preview": redact_preview(prompt),
        "router_prompt_sha256": sha256_bytes(
            json.dumps(
                {"system": system, "prompt_sha256": sha256_bytes(prompt.encode()), "tools": AGENT_TOOL_NAMES},
                sort_keys=True,
            ).encode()
        ),
        "actual_model_message": {
            "role": message.get("role"),
            "content_redacted_preview": redact_preview(content),
            "tool_call_count": 1,
            "tool_calls": tool_calls,
        },
        "selected_tool": {
            "name": tool_name,
            "arguments": args,
            "tool_call_id": tool_calls[0].get("id"),
            "source": "ollama.message.tool_calls[0]",
        },
        "ollama_response_metadata": {
            "model": response.get("model"),
            "created_at": response.get("created_at"),
            "done": response.get("done"),
            "done_reason": response.get("done_reason"),
            "total_duration": response.get("total_duration"),
            "load_duration": response.get("load_duration"),
            "prompt_eval_count": response.get("prompt_eval_count"),
            "eval_count": response.get("eval_count"),
        },
        "tool_contract": {
            "mode": "multi_tool_contextgraph_gateway",
            "available_tool_names": AGENT_TOOL_NAMES,
            "unsupported_tool_behavior": "fail_closed_with_typed_error",
            "executor": "the selected model chooses exactly one tool; ContextGraph validates arguments, executes the tool, and verifies source-of-truth evidence.",
        },
    }
    publish(
        "gateway",
        "selected model chose a ContextGraph tool",
        details={"tool_name": tool_name, "arguments": args},
        client_id=client_id,
        request_id=request_id,
    )
    return router


def prevalidated_harness_tool_call(router: dict[str, Any], prompt: str) -> dict[str, Any]:
    args = router["selected_tool"]["arguments"]
    harness_args = {
        "scenario": args.get("scenario"),
        "requested_capabilities": args.get("requested_capabilities"),
    }
    inner_tool_call = {
        "type": "function",
        "id": router["selected_tool"].get("tool_call_id") or f"call_{hashlib.sha256(prompt.encode()).hexdigest()[:12]}",
        "function": {
            "name": INTERNAL_PROOF_TOOL_NAME,
            "arguments": harness_args,
        },
    }
    return {
        "ollama_response": {
            "message": router["actual_model_message"],
            "metadata": router["ollama_response_metadata"],
        },
        "tool_call": inner_tool_call,
        "arguments": harness_args,
        "prompt_sha256": router["router_prompt_sha256"],
        "operator_request_sha256": router["operator_prompt_sha256"],
        "operator_request_redacted_preview": router["operator_prompt_redacted_preview"],
        "system_message_redacted_preview": router["system_message_redacted_preview"],
        "tool_prompt_redacted_preview": redact_preview(
            json.dumps({"operator_prompt_sha256": router["operator_prompt_sha256"], "selected_tool": router["selected_tool"]}, sort_keys=True),
            limit=360,
        ),
        "actual_model_message": router["actual_model_message"],
        "tool_contract": {
            **router["tool_contract"],
            "selected_gateway_tool_name": "run_proof_pipeline",
            "internal_executor_tool_name": INTERNAL_PROOF_TOOL_NAME,
            "required_scenario": SCENARIO,
            "required_capabilities": REQUIRED_CAPABILITIES,
        },
    }


def snapshot_summary(snapshot: dict[str, Any]) -> dict[str, Any]:
    metrics = snapshot.get("metrics", {})
    score = metrics.get("score", {}) if isinstance(metrics, dict) else {}
    reward = metrics.get("reward") if isinstance(metrics, dict) else None
    ledger = snapshot.get("ledger", {})
    return {
        "source_root": snapshot.get("source_root"),
        "blackbox_run_id": snapshot.get("identity", {}).get("blackbox_run_id"),
        "model": snapshot.get("identity", {}).get("model"),
        "source_of_truth": snapshot.get("source_of_truth"),
        "ledger_source_of_truth": ledger.get("source_of_truth"),
        "ledger_record_count": ledger.get("record_count"),
        "ledger_run_counts": ledger.get("run_counts"),
        "score": score,
        "reward": reward,
        "verification": metrics.get("verification", {}) if isinstance(metrics, dict) else {},
        "training": metrics.get("training_episode_yield", {}) if isinstance(metrics, dict) else {},
        "surprise": metrics.get("surprise", {}) if isinstance(metrics, dict) else {},
        "timeline": [
            {
                "stage": item.get("stage"),
                "status": item.get("status"),
                "title": item.get("title"),
                "value": item.get("value"),
            }
            for item in snapshot.get("timeline", [])
            if isinstance(item, dict)
        ],
        "snapshot_source_of_truth": snapshot.get("frontend_persistence", {}).get("source_of_truth"),
    }


def tool_result_from_snapshot(tool_name: str, snapshot: dict[str, Any], *, action: str, extra: dict[str, Any] | None = None) -> dict[str, Any]:
    result = {
        "status": "ok",
        "tool_name": tool_name,
        "action": action,
        "source_of_truth": snapshot.get("frontend_persistence", {}).get("source_of_truth"),
        "snapshot_path": snapshot.get("frontend_persistence", {}).get("snapshot_path"),
        "summary": snapshot_summary(snapshot),
        **(extra or {}),
    }
    result["verified_summary_text"] = deterministic_tool_summary(result)
    return result


def compact_product_category_pack(pack: dict[str, Any]) -> dict[str, Any]:
    attestation = pack.get("attestation", {}) if isinstance(pack.get("attestation"), dict) else {}
    tamper = attestation.get("tamper_test", {}) if isinstance(attestation.get("tamper_test"), dict) else {}
    return {
        "pack_id": pack.get("pack_id"),
        "created_at": pack.get("created_at"),
        "audience": pack.get("audience"),
        "pack_file": pack.get("pack_file"),
        "source_of_truth": pack.get("source_of_truth"),
        "executive_view": pack.get("executive_view"),
        "model_arena": {
            "category": pack.get("model_arena", {}).get("category"),
            "model_count": pack.get("model_arena", {}).get("model_count"),
            "rows": pack.get("model_arena", {}).get("rows", []),
            "honesty_note": pack.get("model_arena", {}).get("honesty_note"),
        },
        "leaderboard_top": (pack.get("verified_outcome_leaderboard", {}).get("rows") or [])[:5],
        "failure_case_status": [
            {"id": item.get("id"), "status": item.get("status")}
            for item in pack.get("failure_injection_studio", {}).get("failure_cases", [])
            if isinstance(item, dict)
        ],
        "reward_latest": pack.get("reward_dashboard", {}).get("latest_reward"),
        "attestation": {
            "status": attestation.get("signed_attestation", {}).get("status"),
            "tamper_test_status": tamper.get("status"),
            "tamper_exit_code": tamper.get("verify_command", {}).get("exit_code") if isinstance(tamper.get("verify_command"), dict) else None,
        },
    }


def deterministic_tool_summary(tool_result: dict[str, Any]) -> str:
    summary = tool_result.get("summary", {}) if isinstance(tool_result.get("summary"), dict) else {}
    score = summary.get("score", {}) if isinstance(summary.get("score"), dict) else {}
    reward = summary.get("reward", {}) if isinstance(summary.get("reward"), dict) else {}
    counts = summary.get("ledger_run_counts", {}) if isinstance(summary.get("ledger_run_counts"), dict) else {}
    base = (
        f"{tool_result.get('tool_name')} completed action={tool_result.get('action')}; "
        f"run={summary.get('blackbox_run_id')}; "
        f"trust={score.get('trust_level')}; "
        f"risk={score.get('risk')}; "
        f"incidents={counts.get('incident_packet', 0)}; "
        f"reward={reward.get('raw') if isinstance(reward, dict) else None}; "
        f"source_of_truth={tool_result.get('source_of_truth')}."
    )
    pack = tool_result.get("product_category_pack")
    if isinstance(pack, dict):
        executive = pack.get("executive_view", {}) if isinstance(pack.get("executive_view"), dict) else {}
        arena = pack.get("model_arena", {}) if isinstance(pack.get("model_arena"), dict) else {}
        attestation = pack.get("attestation", {}) if isinstance(pack.get("attestation"), dict) else {}
        tamper = attestation.get("tamper_test", {}) if isinstance(attestation.get("tamper_test"), dict) else {}
        return (
            f"Product category pack {pack.get('pack_id')} was built from {len(pack.get('input_runs', []))} real runs; "
            f"models_measured={arena.get('model_count')}; "
            f"actions_attempted={executive.get('actions_attempted')}; "
            f"incidents_caught={executive.get('incident_caught')}; "
            f"evidence_packets={executive.get('evidence_packages')}; "
            f"training_episodes={executive.get('training_episodes_generated')}; "
            f"average_reward={executive.get('average_reward')}; "
            f"attestation_status={attestation.get('signed_attestation', {}).get('status') if isinstance(attestation.get('signed_attestation'), dict) else None}; "
            f"tamper_status={tamper.get('status')}; "
            f"pack_source={pack.get('pack_file', {}).get('source_of_truth') if isinstance(pack.get('pack_file'), dict) else pack.get('source_of_truth')}."
        )
    return base


def compare_snapshots(snapshot_a: dict[str, Any], snapshot_b: dict[str, Any]) -> dict[str, Any]:
    def features(snapshot: dict[str, Any]) -> dict[str, Any]:
        metrics = snapshot.get("metrics", {})
        score = metrics.get("score", {})
        reward = metrics.get("reward") or {}
        counts = snapshot.get("ledger", {}).get("run_counts", {})
        return {
            "blackbox_run_id": snapshot.get("identity", {}).get("blackbox_run_id"),
            "source_root": snapshot.get("source_root"),
            "trust_level": score.get("trust_level"),
            "risk": score.get("risk"),
            "verification_confidence": score.get("verification_confidence"),
            "reward": reward.get("raw") if isinstance(reward, dict) else None,
            "loss": reward.get("loss") if isinstance(reward, dict) else None,
            "incident_count": counts.get("incident_packet", 0),
            "verifier_failures": metrics.get("verification", {}).get("verifier_failure_rate"),
            "source_of_truth_coverage": metrics.get("verification", {}).get("source_of_truth_coverage"),
        }

    a = features(snapshot_a)
    b = features(snapshot_b)

    def safety_key(item: dict[str, Any]) -> tuple[float, float, float, float]:
        reward = item.get("reward")
        return (
            float(item.get("incident_count") or 0),
            float(item.get("risk") or 0),
            -float(item.get("verification_confidence") or 0),
            -float(reward if reward is not None else -999),
        )

    safer = "a" if safety_key(a) <= safety_key(b) else "b"
    return {
        "run_a": a,
        "run_b": b,
        "safer_run": safer,
        "decision_basis": "lower incident count, lower risk, higher verification confidence, then higher reward",
    }


def validate_category_pack_request(audience_value: Any, max_runs_value: Any) -> tuple[str, int]:
    if not isinstance(audience_value, str) or not audience_value.strip():
        raise FrontendError(
            "GEMMAFRONTEND_CATEGORY_AUDIENCE_INVALID",
            "category pack audience must be a non-empty string",
            "request.audience",
            "provide the investor, buyer, or technical audience for the pack",
        )
    audience = audience_value.strip()
    if len(audience) > 240:
        raise FrontendError(
            "GEMMAFRONTEND_CATEGORY_AUDIENCE_TOO_LONG",
            "category pack audience is too long",
            "request.audience",
            "shorten the audience description to 240 characters or fewer",
            details={"chars": len(audience), "max_chars": 240},
        )
    max_runs = 8 if max_runs_value is None else max_runs_value
    if not isinstance(max_runs, int) or isinstance(max_runs, bool) or max_runs < 1 or max_runs > 12:
        raise FrontendError(
            "GEMMAFRONTEND_CATEGORY_MAX_RUNS_INVALID",
            "category pack max_runs must be an integer from 1 through 12",
            "request.max_runs",
            "request between 1 and 12 real completed runs",
            details={"max_runs": max_runs_value},
        )
    return audience, max_runs


def numeric(value: Any) -> float | None:
    if isinstance(value, bool):
        return None
    if isinstance(value, (int, float)):
        return float(value)
    return None


def mean(values: list[float]) -> float | None:
    if not values:
        return None
    return round(sum(values) / len(values), 6)


def category_run_record(snapshot: dict[str, Any]) -> dict[str, Any]:
    metrics = snapshot.get("metrics", {})
    verification = metrics.get("verification", {}) if isinstance(metrics, dict) else {}
    score = metrics.get("score", {}) if isinstance(metrics, dict) else {}
    reward = metrics.get("reward") if isinstance(metrics, dict) else None
    reward_payload = reward if isinstance(reward, dict) else {}
    counts = snapshot.get("ledger", {}).get("run_counts", {})
    agent = snapshot.get("chat_agent") if isinstance(snapshot.get("chat_agent"), dict) else {}
    selected_tool = (agent.get("selected_tool") or {}).get("name") if isinstance(agent, dict) else None
    return {
        "blackbox_run_id": snapshot.get("identity", {}).get("blackbox_run_id"),
        "dynamic_run_id": snapshot.get("identity", {}).get("dynamic_run_id"),
        "source_root": snapshot.get("source_root"),
        "model": snapshot.get("identity", {}).get("model") or "unknown",
        "provider": snapshot.get("identity", {}).get("provider") or "unknown",
        "selected_tool": selected_tool or snapshot.get("model_interaction", {}).get("tool_contract", {}).get("selected_gateway_tool_name"),
        "ledger_source_of_truth": snapshot.get("ledger", {}).get("source_of_truth"),
        "snapshot_source_of_truth": snapshot.get("frontend_persistence", {}).get("source_of_truth"),
        "source_of_truth_coverage": verification.get("source_of_truth_coverage"),
        "readback_ratio": verification.get("readback_ratio"),
        "verifier_pass_rate": verification.get("verifier_pass_rate"),
        "verifier_failure_rate": verification.get("verifier_failure_rate"),
        "trust_level": score.get("trust_level"),
        "correctness": score.get("correctness"),
        "risk": score.get("risk"),
        "verification_confidence": score.get("verification_confidence"),
        "policy_compliance": score.get("policy_compliance"),
        "reward": reward_payload.get("raw"),
        "loss": reward_payload.get("loss"),
        "reward_class": reward_payload.get("class"),
        "incident_count": counts.get("incident_packet", 0),
        "policy_decision_count": counts.get("policy_decision", 0),
        "evidence_packet_count": counts.get("evidence_packet", 0),
        "verifier_result_count": counts.get("verifier_result", 0),
        "tool_call_count": counts.get("tool_call", 0),
        "training_episode_count": metrics.get("training_episode_yield", {}).get("dynamicjepa_episode_count")
        if isinstance(metrics.get("training_episode_yield"), dict)
        else None,
        "surprise_event_count": metrics.get("surprise", {}).get("surprise_events")
        if isinstance(metrics.get("surprise"), dict)
        else None,
        "cost_usd": None,
        "cost_source": "not captured by local Ollama run artifacts",
        "latency_ms": None,
        "latency_source": "not captured by completed run artifacts",
    }


def build_model_arena(records: list[dict[str, Any]]) -> dict[str, Any]:
    grouped: dict[str, list[dict[str, Any]]] = {}
    for record in records:
        grouped.setdefault(str(record.get("model") or "unknown"), []).append(record)
    rows = []
    for model_name, model_records in sorted(grouped.items()):
        rows.append(
            {
                "model": model_name,
                "runs_total": len(model_records),
                "tool_choice_accuracy": mean(
                    [1.0 if numeric(item.get("tool_call_count")) and float(item["tool_call_count"]) > 0 else 0.0 for item in model_records]
                ),
                "verifier_pass_rate": mean([value for item in model_records if (value := numeric(item.get("verifier_pass_rate"))) is not None]),
                "incident_rate": mean([1.0 if int(item.get("incident_count") or 0) > 0 else 0.0 for item in model_records]),
                "average_reward": mean([value for item in model_records if (value := numeric(item.get("reward"))) is not None]),
                "average_loss": mean([value for item in model_records if (value := numeric(item.get("loss"))) is not None]),
                "average_risk": mean([value for item in model_records if (value := numeric(item.get("risk"))) is not None]),
                "average_verification_confidence": mean(
                    [value for item in model_records if (value := numeric(item.get("verification_confidence"))) is not None]
                ),
                "average_source_of_truth_coverage": mean(
                    [value for item in model_records if (value := numeric(item.get("source_of_truth_coverage"))) is not None]
                ),
                "cost_usd": None,
                "cost_note": "not measured because local Ollama artifacts do not currently persist per-token pricing",
                "latency_ms": None,
                "latency_note": "not measured in completed run artifacts; API router events capture request timing separately",
            }
        )
    return {
        "schema_version": 1,
        "category": "Any LLM Verified Agent Benchmark Arena",
        "source": "real completed ContextGraph run snapshots",
        "model_count": len(rows),
        "rows": rows,
        "honesty_note": (
            "Only models with real measured runs appear here. Missing models are not backfilled or estimated."
        ),
    }


def build_verified_outcome_leaderboard(records: list[dict[str, Any]]) -> dict[str, Any]:
    def rank_key(item: dict[str, Any]) -> tuple[float, float, float, float]:
        reward = numeric(item.get("reward"))
        confidence = numeric(item.get("verification_confidence"))
        return (
            float(item.get("incident_count") or 0),
            float(item.get("risk") or 0),
            -float(confidence if confidence is not None else 0),
            -float(reward if reward is not None else -999),
        )

    ranked = sorted(records, key=rank_key)
    rows = []
    for index, record in enumerate(ranked, start=1):
        rows.append(
            {
                "rank": index,
                "blackbox_run_id": record.get("blackbox_run_id"),
                "model": record.get("model"),
                "trust_level": record.get("trust_level"),
                "verifier_pass_rate": record.get("verifier_pass_rate"),
                "source_of_truth_coverage": record.get("source_of_truth_coverage"),
                "incident_count": record.get("incident_count"),
                "policy_decision_count": record.get("policy_decision_count"),
                "reward": record.get("reward"),
                "loss": record.get("loss"),
                "risk": record.get("risk"),
                "verification_confidence": record.get("verification_confidence"),
                "source_root": record.get("source_root"),
            }
        )
    return {
        "schema_version": 1,
        "ranking_basis": "lower incidents, lower risk, higher verification confidence, then higher reward",
        "rows": rows,
    }


def latest_run_payloads(snapshot: dict[str, Any], kind: str, *, limit: int = 20) -> list[dict[str, Any]]:
    ledger = Path(str(snapshot["ledger"]["path"]))
    run_id = str(snapshot["identity"]["blackbox_run_id"])
    return payloads_by_kind(ledger, run_id, kind, limit=limit)


def build_failure_injection_studio(snapshot: dict[str, Any]) -> dict[str, Any]:
    run_id = str(snapshot["identity"]["blackbox_run_id"])
    incidents = latest_run_payloads(snapshot, "incident_packet", limit=12)
    verifiers = latest_run_payloads(snapshot, "verifier_result", limit=20)
    failed_verifiers = [item for item in verifiers if item.get("payload", {}).get("status") == "failed"]
    policies = latest_run_payloads(snapshot, "policy_decision", limit=20)
    denied_policies = [item for item in policies if item.get("payload", {}).get("decision") == "deny"]
    scorecard_source = snapshot.get("metrics", {}).get("scorecard_source", {})
    stale_scorecard_check = {
        "status": "passed" if scorecard_source.get("kind") == "ledger_latest_row" else "blocked",
        "source": scorecard_source,
        "meaning": "UI scoring reads the latest SQLite scorecard row, so stale export files cannot silently win.",
    }
    return {
        "schema_version": 1,
        "run_id": run_id,
        "source_of_truth": snapshot.get("ledger", {}).get("source_of_truth"),
        "failure_cases": [
            {
                "id": "verifier_expectation_mismatch",
                "status": "caught" if failed_verifiers else "not_observed",
                "evidence_rows": failed_verifiers,
                "meaning": "A verifier expected impossible physical state and BlackBoxFix created failed verifier evidence.",
            },
            {
                "id": "incident_packet_created",
                "status": "caught" if incidents else "not_observed",
                "evidence_rows": incidents,
                "meaning": "Failed source-of-truth verification became an incident packet instead of a hidden warning.",
            },
            {
                "id": "policy_gate_denial",
                "status": "caught" if denied_policies else "not_observed",
                "evidence_rows": denied_policies,
                "meaning": "Policy decisions are durable ledger records and can deny actions before score/reward refresh.",
            },
            {
                "id": "stale_scorecard_export",
                "status": stale_scorecard_check["status"],
                "evidence": stale_scorecard_check,
                "meaning": "The UI and pack builder read latest scorecards from SQLite rather than trusting export files.",
            },
            {
                "id": "unsupported_tool_call",
                "status": "fail_closed_by_contract",
                "evidence": {
                    "allowlist": AGENT_TOOL_NAMES,
                    "validator": "validate_agent_tool_call",
                    "typed_error": "GEMMAFRONTEND_TOOL_UNSUPPORTED",
                },
                "meaning": "Tool-call hallucinations cannot execute because the gateway validates the declared allowlist first.",
            },
        ],
    }


def build_connector_contracts() -> dict[str, Any]:
    return {
        "schema_version": 1,
        "claim": "Bring any LLM. Route its tools through ContextGraph. Every mutating action becomes verified evidence.",
        "hard_requirement": "The LLM or agent runtime must route mutating tools through the ContextGraph action gateway.",
        "integration_modes": [
            {
                "mode": "openai_compatible_proxy",
                "status": "contract_materialized_not_executed_in_local_pack",
                "captures": ["model_io", "tool_call", "token_metadata_when_available", "latency_when_available"],
            },
            {
                "mode": "mcp_server",
                "status": "contract_materialized_not_executed_in_local_pack",
                "captures": ["tool_schema", "tool_result", "resource_refs", "operator_approval_context"],
            },
            {
                "mode": "python_sdk",
                "status": "contract_materialized_not_executed_in_local_pack",
                "captures": ["expected_delta", "executor", "source_of_truth_verifier", "evidence_packet"],
            },
            {
                "mode": "typescript_sdk",
                "status": "contract_materialized_not_executed_in_local_pack",
                "captures": ["browser_agent_tool_calls", "service_tool_calls", "policy_decisions"],
            },
            {
                "mode": "cli_gateway",
                "status": "available_for_local_harness",
                "captures": ["command", "exit_code", "stdout", "stderr", "readback_snapshot"],
            },
        ],
        "tool_contract_fields": {
            "tool_name": "stable tool id",
            "input_schema": "strict JSON schema",
            "executor": "real system mutation or read-only operation",
            "expected_persisted_delta": "typed intended outcome",
            "source_of_truth": "durable system that must be read independently",
            "verifier": "separate read path that proves or rejects the outcome",
            "policy": "allow, deny, supervise, or block before mutation",
        },
        "unsupported_behavior": "fail_closed_with_typed_error",
    }


def build_governance_evidence_map(records: list[dict[str, Any]], snapshot: dict[str, Any]) -> dict[str, Any]:
    ledger_sot_value = snapshot.get("ledger", {}).get("source_of_truth")
    return {
        "schema_version": 1,
        "frameworks": {
            "nist_ai_rmf": {
                "govern": {
                    "evidence": ["policy_decision", "scorecard", "source_of_truth coverage"],
                    "source_of_truth": ledger_sot_value,
                },
                "map": {
                    "evidence": ["model", "tool_call", "source_root", "capability ids"],
                    "source_of_truth": [item.get("source_root") for item in records],
                },
                "measure": {
                    "evidence": ["verifier_result", "readback_snapshot", "reward_compilation", "leaderboard"],
                    "source_of_truth": ledger_sot_value,
                },
                "manage": {
                    "evidence": ["incident_packet", "buyer_report", "failure_injection_studio"],
                    "source_of_truth": ledger_sot_value,
                },
            },
            "soc2_style_audit_evidence": {
                "change_control": ["tool_call", "expected_delta", "actual_readback"],
                "monitoring": ["scorecard", "incident_packet", "dashboard"],
                "integrity": ["hash_chain", "artifact_sha256", "signed_attestation"],
            },
            "iso_42001_style_ai_governance": {
                "risk_management": ["risk score", "policy decision", "incident"],
                "evaluation": ["reward sample", "leaderboard", "training episode export"],
                "traceability": ["provenance graph", "ledger refs", "artifact hashes"],
            },
        },
        "control_to_record_kind": {
            "authorized_action": "policy_decision",
            "real_action": "tool_call",
            "expected_outcome": "expected_delta",
            "actual_readback": "readback_snapshot",
            "verification": "verifier_result",
            "incident_response": "incident_packet",
            "training_signal": "reward_compilation",
        },
    }


def build_reward_dashboard(snapshot: dict[str, Any]) -> dict[str, Any]:
    rewards = list(reversed(latest_run_payloads(snapshot, "reward_compilation", limit=30)))
    rows = []
    for item in rewards:
        payload = item.get("payload", {})
        reward = payload.get("reward", {}) if isinstance(payload, dict) else {}
        rows.append(
            {
                "sequence": item.get("sequence"),
                "record_id": item.get("record_id"),
                "payload_sha256": item.get("payload_sha256"),
                "reward": reward.get("raw"),
                "loss": reward.get("loss"),
                "class": reward.get("class"),
                "components": [
                    {
                        "id": component.get("id"),
                        "feature": component.get("feature"),
                        "value": component.get("value"),
                        "weight": component.get("weight"),
                        "contribution": component.get("contribution"),
                    }
                    for component in payload.get("components", [])
                    if isinstance(component, dict)
                ],
            }
        )
    return {
        "schema_version": 1,
        "loss_convention": "loss = -reward for this scalar compiler because training optimizers normally minimize loss",
        "objective": "maximize verified source-of-truth outcomes while penalizing incidents, risk, and missing governance",
        "source_of_truth": snapshot.get("ledger", {}).get("source_of_truth"),
        "reward_history": rows,
        "latest_reward": rows[-1] if rows else None,
    }


def build_self_improvement_loop(records: list[dict[str, Any]], reward_dashboard: dict[str, Any]) -> dict[str, Any]:
    latest_reward = reward_dashboard.get("latest_reward") or {}
    return {
        "schema_version": 1,
        "controlled_loop": [
            "agent attempts task through ContextGraph",
            "ContextGraph executes only governed tools",
            "BlackBoxFix verifies actual source-of-truth state",
            "reward compiler scores the verified trajectory",
            "agent proposes a fix, but cannot edit verifiers, reward specs, held-out tasks, or promotion rules",
            "candidate run must improve verified reward and pass held-out source-of-truth checks",
        ],
        "current_observation": {
            "latest_reward": latest_reward.get("reward"),
            "latest_loss": latest_reward.get("loss"),
            "measured_runs": len(records),
            "guardrails": [
                "verifier definitions immutable to the evaluated agent",
                "reward spec immutable to the evaluated agent",
                "source-of-truth readbacks performed on a separate path",
                "policy gates persisted as ledger rows",
            ],
        },
    }


def build_executive_view(records: list[dict[str, Any]], failure_studio: dict[str, Any]) -> dict[str, Any]:
    actions = sum(int(item.get("tool_call_count") or 0) for item in records)
    incidents = sum(int(item.get("incident_count") or 0) for item in records)
    evidence_packets = sum(int(item.get("evidence_packet_count") or 0) for item in records)
    training_episodes = sum(int(item.get("training_episode_count") or 0) for item in records)
    policy_decisions = sum(int(item.get("policy_decision_count") or 0) for item in records)
    denied_policy_cases = [
        case
        for case in failure_studio.get("failure_cases", [])
        if case.get("id") == "policy_gate_denial" and case.get("status") == "caught"
    ]
    return {
        "schema_version": 1,
        "product_category": "Verified Outcome Intelligence for Tool-Using AI Agents",
        "positioning": "ContextGraph is the verification, benchmark, evidence, and reward layer for any LLM routed through its action gateway.",
        "actions_attempted": actions,
        "verified_successful": sum(1 for item in records if item.get("trust_level") in {"verified", "verified_with_policy"}),
        "incident_caught": incidents,
        "blocked_or_denied": len(denied_policy_cases),
        "evidence_packages": evidence_packets,
        "training_episodes_generated": training_episodes,
        "policy_decisions_recorded": policy_decisions,
        "average_verification_confidence": mean(
            [value for item in records if (value := numeric(item.get("verification_confidence"))) is not None]
        ),
        "average_reward": mean([value for item in records if (value := numeric(item.get("reward"))) is not None]),
        "budget_reason": "replaces prose-only agent demos with durable proof, audit evidence, incident response, and trainable reward signal",
    }


def build_positioning_markdown(pack: dict[str, Any]) -> str:
    executive = pack["executive_view"]
    arena = pack["model_arena"]
    leaderboard = pack["verified_outcome_leaderboard"]
    return (
        "# ContextGraph Product Category Pack\n\n"
        f"Generated: {pack['created_at']}\n\n"
        "## Category\n\n"
        "Verified Outcome Intelligence for Tool-Using AI Agents.\n\n"
        "## Core Claim\n\n"
        "Bring any LLM. Route its tools through ContextGraph. Every action becomes verified evidence.\n\n"
        "## What This Pack Proves\n\n"
        f"- Real measured runs: `{len(pack['input_runs'])}`\n"
        f"- Models measured: `{arena['model_count']}`\n"
        f"- Actions attempted: `{executive['actions_attempted']}`\n"
        f"- Evidence packets: `{executive['evidence_packages']}`\n"
        f"- Incidents caught: `{executive['incident_caught']}`\n"
        f"- Training episodes generated: `{executive['training_episodes_generated']}`\n\n"
        "## Leaderboard Basis\n\n"
        f"{leaderboard['ranking_basis']}.\n\n"
        "## Source Of Truth\n\n"
        "Every number in this pack is derived from completed run directories and SQLite ledger rows. "
        "No missing model rows are estimated.\n"
    )


def run_openssl(command: list[str]) -> subprocess.CompletedProcess[str]:
    try:
        return subprocess.run(
            command,
            cwd=str(REPO_ROOT),
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
            timeout=60.0,
        )
    except (OSError, subprocess.TimeoutExpired) as err:
        raise FrontendError(
            "GEMMAFRONTEND_OPENSSL_EXEC_FAILED",
            f"failed to execute OpenSSL command: {err}",
            "openssl",
            "install OpenSSL and retry signed attestation generation",
            details={"command": command},
            http_status=500,
        ) from err


def create_signed_attestation(runtime_root: Path, pack_dir: Path, artifact_rel_paths: list[str], pack_id: str) -> dict[str, Any]:
    openssl = shutil.which("openssl")
    if not openssl:
        raise FrontendError(
            "GEMMAFRONTEND_OPENSSL_MISSING",
            "signed attestation requires openssl",
            "system.openssl",
            "install openssl before building the product category pack",
            http_status=500,
        )
    key_dir = runtime_root / "attestation-keys"
    key_dir.mkdir(parents=True, exist_ok=True)
    private_key = key_dir / "contextgraph-demo-attestation-private.pem"
    public_key = key_dir / "contextgraph-demo-attestation-public.pem"
    if not private_key.exists():
        gen = run_openssl([openssl, "genpkey", "-algorithm", "RSA", "-pkeyopt", "rsa_keygen_bits:2048", "-out", str(private_key)])
        if gen.returncode != 0:
            raise FrontendError(
                "GEMMAFRONTEND_ATTESTATION_KEYGEN_FAILED",
                "OpenSSL failed to generate an attestation key",
                "attestation.private_key",
                "inspect OpenSSL stderr and filesystem permissions",
                details={"stderr": gen.stderr[-2000:], "stdout": gen.stdout[-2000:]},
                http_status=500,
            )
        private_key.chmod(0o600)
    if not public_key.exists():
        pub = run_openssl([openssl, "rsa", "-in", str(private_key), "-pubout", "-out", str(public_key)])
        if pub.returncode != 0:
            raise FrontendError(
                "GEMMAFRONTEND_ATTESTATION_PUBLIC_KEY_FAILED",
                "OpenSSL failed to derive the attestation public key",
                "attestation.public_key",
                "inspect OpenSSL stderr and filesystem permissions",
                details={"stderr": pub.stderr[-2000:], "stdout": pub.stdout[-2000:]},
                http_status=500,
            )
    subjects = []
    for rel in artifact_rel_paths:
        artifact = artifact_record(pack_dir, rel)
        subjects.append(
            {
                "name": rel,
                "uri": artifact["source_of_truth"],
                "sha256": artifact["sha256"],
                "bytes": artifact["bytes"],
            }
        )
    manifest_path = pack_dir / "attestation-manifest.json"
    manifest = {
        "schema_version": 1,
        "pack_id": pack_id,
        "created_at": now_rfc3339(),
        "signing_key": file_sot(public_key),
        "signature_algorithm": "openssl dgst -sha256 -sign rsa2048",
        "subjects": subjects,
    }
    write_json(manifest_path, manifest)
    signature_path = pack_dir / "attestation-signature.bin"
    sign = run_openssl([openssl, "dgst", "-sha256", "-sign", str(private_key), "-out", str(signature_path), str(manifest_path)])
    if sign.returncode != 0:
        raise FrontendError(
            "GEMMAFRONTEND_ATTESTATION_SIGN_FAILED",
            "OpenSSL failed to sign the attestation manifest",
            "attestation.signature",
            "inspect OpenSSL stderr and retry after fixing signing inputs",
            details={"stderr": sign.stderr[-2000:], "stdout": sign.stdout[-2000:]},
            http_status=500,
        )
    verify = run_openssl(
        [openssl, "dgst", "-sha256", "-verify", str(public_key), "-signature", str(signature_path), str(manifest_path)]
    )
    if verify.returncode != 0:
        raise FrontendError(
            "GEMMAFRONTEND_ATTESTATION_VERIFY_FAILED",
            "OpenSSL could not verify the signed attestation manifest",
            "attestation.verify",
            "do not trust the pack until signing and verification are fixed",
            details={"stderr": verify.stderr[-2000:], "stdout": verify.stdout[-2000:]},
            http_status=500,
        )
    tampered_path = pack_dir / "attestation-manifest.tampered.json"
    tampered_manifest = {**manifest, "tamper_probe": f"modified-{time.time_ns()}"}
    write_json(tampered_path, tampered_manifest)
    tamper_verify = run_openssl(
        [openssl, "dgst", "-sha256", "-verify", str(public_key), "-signature", str(signature_path), str(tampered_path)]
    )
    if tamper_verify.returncode == 0:
        raise FrontendError(
            "GEMMAFRONTEND_TAMPER_TEST_FAILED_OPEN",
            "tampered attestation manifest still verified",
            "attestation.tamper_test",
            "replace the attestation implementation before using signed evidence",
            details={"stdout": tamper_verify.stdout[-2000:], "stderr": tamper_verify.stderr[-2000:]},
            http_status=500,
        )
    signed_attestation = {
        "schema_version": 1,
        "status": "verified",
        "pack_id": pack_id,
        "manifest": artifact_record(pack_dir, "attestation-manifest.json"),
        "signature": {
            "path": str(signature_path),
            "source_of_truth": file_sot(signature_path),
            "sha256": sha256_file(signature_path),
            "base64": base64.b64encode(signature_path.read_bytes()).decode("ascii"),
        },
        "public_key": {
            "path": str(public_key),
            "source_of_truth": file_sot(public_key),
            "sha256": sha256_file(public_key),
        },
        "verify_command": {
            "command": [openssl, "dgst", "-sha256", "-verify", str(public_key), "-signature", str(signature_path), str(manifest_path)],
            "exit_code": verify.returncode,
            "stdout": verify.stdout.strip(),
            "stderr": verify.stderr.strip(),
        },
    }
    write_json(pack_dir / "signed-attestation.json", signed_attestation)
    tamper_result = {
        "schema_version": 1,
        "status": "tamper_detected",
        "meaning": "The original signature fails against a modified manifest.",
        "tampered_manifest": artifact_record(pack_dir, "attestation-manifest.tampered.json"),
        "verify_command": {
            "command": [openssl, "dgst", "-sha256", "-verify", str(public_key), "-signature", str(signature_path), str(tampered_path)],
            "exit_code": tamper_verify.returncode,
            "stdout": tamper_verify.stdout.strip(),
            "stderr": tamper_verify.stderr.strip(),
        },
    }
    write_json(pack_dir / "attestation-tamper-test.json", tamper_result)
    return {
        "signed_attestation": load_json(pack_dir / "signed-attestation.json", "product_category.signed_attestation"),
        "tamper_test": load_json(pack_dir / "attestation-tamper-test.json", "product_category.tamper_test"),
    }


def build_product_category_pack(
    runtime_root: Path,
    *,
    audience: Any,
    max_runs: Any,
    client_id: str | None,
    request_id: str | None,
) -> dict[str, Any]:
    audience_text, max_run_count = validate_category_pack_request(audience, max_runs)
    runtime_root.mkdir(parents=True, exist_ok=True)
    publish(
        "category",
        "reading real runs for product category pack",
        details={"audience": redact_preview(audience_text), "max_runs": max_run_count},
        client_id=client_id,
        request_id=request_id,
    )
    scan = scan_real_runs(runtime_root)
    complete_roots = [Path(str(item["root"])).resolve() for item in scan if item.get("complete")]
    snapshots: list[dict[str, Any]] = []
    rejected: list[dict[str, Any]] = []
    for root in complete_roots:
        if len(snapshots) >= max_run_count:
            break
        try:
            snapshots.append(build_snapshot(root, frontend_context={"mode": "product_category_pack_scan"}))
        except FrontendError as err:
            rejected.append({"root": str(root), "error": err.to_json()})
    if not snapshots:
        raise FrontendError(
            "GEMMAFRONTEND_CATEGORY_NO_REAL_RUNS",
            "no completed real runs could be read into the product category pack",
            "runtime.real_runs",
            "run the full proof pipeline, materialize intelligence, and retry category pack generation",
            source_of_truth=file_sot(runtime_root),
            details={"scan": scan, "rejected": rejected},
            http_status=409,
        )
    records = [category_run_record(snapshot) for snapshot in snapshots]
    latest_snapshot = snapshots[0]
    pack_basis = json.dumps(
        {
            "audience": audience_text,
            "roots": [record.get("source_root") for record in records],
            "created_at_ns": time.time_ns(),
        },
        sort_keys=True,
    )
    pack_id = f"category_{time.time_ns()}_{hashlib.sha256(pack_basis.encode()).hexdigest()[:10]}"
    pack_dir = runtime_root / "product-category" / pack_id
    pack_dir.mkdir(parents=True, exist_ok=False)

    model_arena = build_model_arena(records)
    leaderboard = build_verified_outcome_leaderboard(records)
    failure_studio = build_failure_injection_studio(latest_snapshot)
    connector_contracts = build_connector_contracts()
    governance_map = build_governance_evidence_map(records, latest_snapshot)
    reward_dashboard = build_reward_dashboard(latest_snapshot)
    self_improvement = build_self_improvement_loop(records, reward_dashboard)
    executive_view = build_executive_view(records, failure_studio)
    partial_pack = {
        "pack_id": pack_id,
        "created_at": now_rfc3339(),
        "audience": audience_text,
        "source_of_truth": file_sot(pack_dir),
        "input_runs": records,
        "rejected_candidates": rejected,
        "model_arena": model_arena,
        "verified_outcome_leaderboard": leaderboard,
        "failure_injection_studio": failure_studio,
        "connector_contracts": connector_contracts,
        "governance_evidence_map": governance_map,
        "reward_dashboard": reward_dashboard,
        "self_improvement_loop": self_improvement,
        "executive_view": executive_view,
    }
    artifacts: dict[str, Any] = {
        "model-comparison-arena.json": model_arena,
        "verified-outcome-leaderboard.json": leaderboard,
        "failure-injection-studio.json": failure_studio,
        "any-llm-connector-contracts.json": connector_contracts,
        "governance-evidence-map.json": governance_map,
        "reward-compiler-dashboard.json": reward_dashboard,
        "self-improvement-loop-demo.json": self_improvement,
        "executive-category-view.json": executive_view,
    }
    for rel, payload in artifacts.items():
        write_json(pack_dir / rel, payload)
    atomic_write(pack_dir / "product-category-positioning.md", build_positioning_markdown(partial_pack).encode("utf-8"))
    signed = create_signed_attestation(
        runtime_root,
        pack_dir,
        [
            *artifacts.keys(),
            "product-category-positioning.md",
        ],
        pack_id,
    )
    pack = {
        **partial_pack,
        "attestation": signed,
        "artifacts": {
            rel: artifact_record(pack_dir, rel)
            for rel in [
                *artifacts.keys(),
                "product-category-positioning.md",
                "attestation-manifest.json",
                "signed-attestation.json",
                "attestation-tamper-test.json",
            ]
        },
    }
    write_json(pack_dir / "product-category-pack.json", pack)
    pack_readback = load_json(pack_dir / "product-category-pack.json", "product_category.pack")
    if pack_readback.get("pack_id") != pack_id:
        raise FrontendError(
            "GEMMAFRONTEND_CATEGORY_PACK_READBACK_MISMATCH",
            "product category pack readback did not match the pack id",
            "product_category.pack_id",
            "fix pack persistence before trusting the category output",
            source_of_truth=file_sot(pack_dir / "product-category-pack.json"),
            details={"expected": pack_id, "actual": pack_readback.get("pack_id")},
            http_status=500,
        )
    pack_readback["pack_file"] = artifact_record(pack_dir, "product-category-pack.json")
    publish(
        "category",
        "product category pack persisted and attestation verified",
        details={
            "pack_id": pack_id,
            "pack_file": pack_readback["pack_file"]["source_of_truth"],
            "measured_runs": len(records),
            "tamper_status": signed["tamper_test"]["status"],
        },
        client_id=client_id,
        request_id=request_id,
    )
    return pack_readback


def execute_agent_tool(
    router: dict[str, Any],
    runtime_root: Path,
    prompt: str,
    model: str,
    ollama_url: str,
    *,
    client_id: str | None,
    request_id: str | None,
) -> tuple[dict[str, Any], dict[str, Any]]:
    tool_name = router["selected_tool"]["name"]
    args = router["selected_tool"]["arguments"]
    if tool_name == "run_proof_pipeline":
        snapshot = run_live_demo(
            runtime_root,
            prompt,
            model,
            ollama_url,
            compile_reward_after=True,
            client_id=client_id,
            request_id=request_id,
            prevalidated_tool_call=prevalidated_harness_tool_call(router, prompt),
        )
        return snapshot, tool_result_from_snapshot(tool_name, snapshot, action="ran full proof pipeline")

    if tool_name == "build_investor_category_pack":
        pack = build_product_category_pack(
            runtime_root,
            audience=args.get("audience"),
            max_runs=args.get("max_runs"),
            client_id=client_id,
            request_id=request_id,
        )
        latest_root = Path(str(pack["input_runs"][0]["source_root"])).resolve()
        snapshot = build_snapshot(latest_root, frontend_context={"mode": "agent_build_investor_category_pack"})
        snapshot["product_category_pack"] = pack
        snapshot = persist_snapshot(runtime_root, snapshot, "agent_build_investor_category_pack")
        return snapshot, tool_result_from_snapshot(
            tool_name,
            snapshot,
            action="built product category pack from real run artifacts",
            extra={"product_category_pack": pack},
        )

    if tool_name in {"inspect_latest_run", "explain_scorecard", "analyze_incident", "suggest_reward_improvements", "compile_reward_for_run", "export_buyer_report", "evaluate_tool_policy"}:
        root = validate_optional_root_argument(args.get("root"), runtime_root)
    else:
        root = latest_complete_run(runtime_root)

    if tool_name == "inspect_latest_run":
        snapshot = inspect_real_run(
            runtime_root,
            root,
            model,
            compile_reward_requested=False,
            client_id=client_id,
            request_id=request_id,
        )
        return snapshot, tool_result_from_snapshot(tool_name, snapshot, action="inspected latest real run")

    if tool_name == "compile_reward_for_run":
        reward = compile_reward(root, model, client_id=client_id, request_id=request_id)
        snapshot = build_snapshot(root, frontend_context={"mode": "agent_compile_reward"})
        snapshot = persist_snapshot(runtime_root, snapshot, "agent_compile_reward")
        return snapshot, tool_result_from_snapshot(
            tool_name,
            snapshot,
            action="compiled or read existing reward",
            extra={"reward_record": reward},
        )

    if tool_name == "explain_scorecard":
        snapshot = build_snapshot(root, frontend_context={"mode": "agent_explain_scorecard"})
        snapshot = persist_snapshot(runtime_root, snapshot, "agent_explain_scorecard")
        scorecard = latest_payload(
            Path(snapshot["ledger"]["path"]),
            snapshot["identity"]["blackbox_run_id"],
            "scorecard",
        )
        return snapshot, tool_result_from_snapshot(
            tool_name,
            snapshot,
            action="read latest scorecard row from the SQLite ledger",
            extra={"scorecard": scorecard},
        )

    if tool_name == "analyze_incident":
        snapshot = build_snapshot(root, frontend_context={"mode": "agent_analyze_incident"})
        snapshot = persist_snapshot(runtime_root, snapshot, "agent_analyze_incident")
        run_id = snapshot["identity"]["blackbox_run_id"]
        ledger_path = Path(snapshot["ledger"]["path"])
        incidents = payloads_by_kind(ledger_path, run_id, "incident_packet", limit=8)
        verifiers = payloads_by_kind(ledger_path, run_id, "verifier_result", limit=12)
        failed = [item for item in verifiers if item.get("payload", {}).get("status") == "failed"]
        return snapshot, tool_result_from_snapshot(
            tool_name,
            snapshot,
            action="read incident and failed verifier rows",
            extra={"incident_packets": incidents, "failed_verifier_results": failed},
        )

    if tool_name == "suggest_reward_improvements":
        snapshot = build_snapshot(root, frontend_context={"mode": "agent_suggest_reward_improvements"})
        snapshot = persist_snapshot(runtime_root, snapshot, "agent_suggest_reward_improvements")
        run_id = snapshot["identity"]["blackbox_run_id"]
        ledger_path = Path(snapshot["ledger"]["path"])
        reward = latest_payload(ledger_path, run_id, "reward_compilation")
        if not reward:
            raise FrontendError(
                "GEMMAFRONTEND_REWARD_MISSING",
                "latest run does not have a reward_compilation row",
                "ledger_records.reward_compilation",
                "ask Gemma to compile reward for this run before optimizing reward",
                source_of_truth=ledger_sot(ledger_path),
                http_status=409,
            )
        components = reward["payload"].get("components", [])
        negative_components = sorted(
            [item for item in components if isinstance(item, dict) and float(item.get("contribution", 0) or 0) < 0],
            key=lambda item: float(item.get("contribution", 0) or 0),
        )
        improvement_plan = []
        for item in negative_components:
            feature = item.get("feature")
            if feature == "incident_present":
                improvement_plan.append("remove the verifier mismatch that creates the incident packet")
            elif feature == "risk":
                improvement_plan.append("reduce score.risk by eliminating failed verifier evidence and missing governance")
            elif feature == "policy_missing":
                improvement_plan.append("persist a first-class policy_decision before scoring the run")
            else:
                improvement_plan.append(f"improve feature {feature}")
        return snapshot, tool_result_from_snapshot(
            tool_name,
            snapshot,
            action="derived reward improvement targets from persisted reward components",
            extra={"reward_record": reward, "negative_components": negative_components, "improvement_plan": improvement_plan},
        )

    if tool_name == "compare_recent_runs":
        root_a = validate_run_root(args["root_a"]) if args.get("root_a") else None
        root_b = validate_run_root(args["root_b"]) if args.get("root_b") else None
        if root_a is None or root_b is None:
            complete = [Path(item["root"]).resolve() for item in scan_real_runs(runtime_root) if item.get("complete")]
            if len(complete) < 2:
                raise FrontendError(
                    "GEMMAFRONTEND_COMPARE_NEEDS_TWO_RUNS",
                    "compare_recent_runs requires at least two completed real runs",
                    "runtime.real_runs",
                    "run the full proof pipeline twice or pass two completed run roots",
                    source_of_truth=file_sot(runtime_root),
                    details={"complete_runs": [str(item) for item in complete]},
                    http_status=409,
                )
            root_a = root_a or complete[0]
            root_b = root_b or next(item for item in complete if item != root_a)
        snapshot_a = build_snapshot(root_a, frontend_context={"mode": "agent_compare_recent_runs", "compare_role": "a"})
        snapshot_b = build_snapshot(root_b, frontend_context={"mode": "agent_compare_recent_runs", "compare_role": "b"})
        comparison = compare_snapshots(snapshot_a, snapshot_b)
        comparison_path = runtime_root / "comparisons" / f"compare-{int(time.time())}-{hashlib.sha256((str(root_a)+str(root_b)).encode()).hexdigest()[:8]}.json"
        write_json(comparison_path, comparison)
        comparison_readback = load_json(comparison_path, "comparisons.latest")
        snapshot_a["comparison"] = {
            **comparison_readback,
            "source_of_truth": file_sot(comparison_path),
            "sha256": sha256_file(comparison_path),
        }
        snapshot = persist_snapshot(runtime_root, snapshot_a, "agent_compare_recent_runs")
        return snapshot, tool_result_from_snapshot(
            tool_name,
            snapshot,
            action="compared two physical run snapshots",
            extra={"comparison": snapshot["comparison"]},
        )

    if tool_name == "export_buyer_report":
        snapshot = build_snapshot(root, frontend_context={"mode": "agent_export_buyer_report"})
        snapshot = persist_snapshot(runtime_root, snapshot, "agent_export_buyer_report")
        summary = snapshot_summary(snapshot)
        report = (
            f"# ContextGraph Buyer Evidence Report\n\n"
            f"Generated: {now_rfc3339()}\n\n"
            f"Audience: {redact_preview(str(args.get('audience', 'buyer')))}\n\n"
            f"## Run Identity\n\n"
            f"- BlackBoxFix run: `{summary['blackbox_run_id']}`\n"
            f"- Source root: `{summary['source_root']}`\n"
            f"- Ledger: `{summary['ledger_source_of_truth']}`\n\n"
            f"## What Was Proven\n\n"
            f"ContextGraph routed a model-requested action through durable state writes, independent source-of-truth readbacks, "
            f"ledger evidence, scorecard calculation, incident detection, and reward compilation.\n\n"
            f"## Metrics\n\n"
            f"- Trust: `{summary['score'].get('trust_level')}`\n"
            f"- Risk: `{summary['score'].get('risk')}`\n"
            f"- Verification confidence: `{summary['score'].get('verification_confidence')}`\n"
            f"- Reward: `{(summary.get('reward') or {}).get('raw') if isinstance(summary.get('reward'), dict) else None}`\n"
            f"- Incidents: `{(summary.get('ledger_run_counts') or {}).get('incident_packet', 0)}`\n\n"
            f"## Source-Of-Truth Readbacks\n\n"
            f"```json\n{json.dumps(summary['source_of_truth'], indent=2, sort_keys=True)}\n```\n"
        )
        report_path = runtime_root / "buyer-reports" / f"buyer-report-{summary['blackbox_run_id']}-{int(time.time())}.md"
        atomic_write(report_path, report.encode("utf-8"))
        report_readback = report_path.read_text(encoding="utf-8")
        if summary["blackbox_run_id"] not in report_readback:
            raise FrontendError(
                "GEMMAFRONTEND_BUYER_REPORT_READBACK_MISMATCH",
                "buyer report was written but readback did not contain the run id",
                "buyer_report.blackbox_run_id",
                "fix the buyer report writer before using the artifact",
                source_of_truth=file_sot(report_path),
                http_status=500,
            )
        artifact = {
            "path": str(report_path),
            "source_of_truth": file_sot(report_path),
            "sha256": sha256_file(report_path),
            "bytes": report_path.stat().st_size,
        }
        snapshot["buyer_report"] = artifact
        return snapshot, tool_result_from_snapshot(
            tool_name,
            snapshot,
            action="exported buyer-facing evidence report and read it back",
            extra={"buyer_report": artifact},
        )

    if tool_name == "evaluate_tool_policy":
        snapshot_before = build_snapshot(root, frontend_context={"mode": "agent_evaluate_tool_policy_before"})
        run_id = snapshot_before["identity"]["blackbox_run_id"]
        ledger_path = Path(snapshot_before["ledger"]["path"])
        policy_before = int(snapshot_before["ledger"]["run_counts"].get("policy_decision", 0))
        result = run_process(
            [
                str(REPO_ROOT / "target/debug/context-graph"),
                "blackbox",
                "policy",
                "evaluate",
                "--ledger",
                str(ledger_path),
                "--mode",
                str(args.get("mode")),
                "--tool",
                INTERNAL_PROOF_TOOL_NAME,
                "--capability",
                "blackboxfix.demo.full_agentops_dynamicjepa",
                "--run-id",
                run_id,
                "--mutating",
                "--verifier-id",
                "cmd_exit_zero_v1",
                "--source-scope",
                "urn:contextgraph:dynamicjepa:counter_world_full_agentops",
                "--tenant",
                "tenant_local",
                "--agent",
                "gemmafrontend_policy_tool",
                "--model",
                model,
            ],
            timeout=120.0,
        )
        policy_stdout = result.get("stdout") if isinstance(result.get("stdout"), dict) else {}
        score_refresh = run_process(
            [
                str(REPO_ROOT / "target/debug/context-graph"),
                "blackbox",
                "score",
                "run",
                "--ledger",
                str(ledger_path),
                "--run-id",
                run_id,
                "--tenant",
                "tenant_local",
                "--agent",
                "gemmafrontend_policy_rescore",
                "--model",
                model,
            ],
            timeout=120.0,
        )
        reward_refresh = run_process(
            [
                str(REPO_ROOT / "target/debug/context-graph"),
                "blackbox",
                "reward",
                "compile",
                "--ledger",
                str(ledger_path),
                "--run-id",
                run_id,
                "--input",
                str(APP_ROOT / "reward_spec.verified_outcome.json"),
                "--tenant",
                "tenant_local",
                "--agent",
                "gemmafrontend_policy_reward_refresh",
                "--model",
                model,
            ],
            timeout=120.0,
        )
        snapshot = build_snapshot(root, frontend_context={"mode": "agent_evaluate_tool_policy"})
        policy_after = int(snapshot["ledger"]["run_counts"].get("policy_decision", 0))
        latest_policy = latest_payload(ledger_path, run_id, "policy_decision")
        if policy_after != policy_before + 1:
            raise FrontendError(
                "GEMMAFRONTEND_POLICY_READBACK_COUNT_MISMATCH",
                "policy evaluate command returned but policy_decision count did not increase by one",
                "ledger_records.policy_decision",
                "inspect policy evaluate and SQLite append/readback logic",
                source_of_truth=ledger_sot(ledger_path),
                details={"before": policy_before, "after": policy_after, "stdout": policy_stdout},
                http_status=500,
            )
        snapshot = persist_snapshot(runtime_root, snapshot, "agent_evaluate_tool_policy")
        return snapshot, tool_result_from_snapshot(
            tool_name,
            snapshot,
            action="persisted and read back a BlackBoxFix policy_decision row",
            extra={
                "policy_command": policy_stdout,
                "score_refresh": score_refresh.get("stdout"),
                "reward_refresh": reward_refresh.get("stdout"),
                "latest_policy_decision": latest_policy,
            },
        )

    raise FrontendError(
        "GEMMAFRONTEND_TOOL_EXECUTOR_MISSING",
        "selected tool has no executor",
        "selected_tool.name",
        "implement the tool before exposing it to Gemma",
        details={"selected_tool": router.get("selected_tool")},
        http_status=500,
    )


def model_tool_result_payload(tool_result: dict[str, Any]) -> dict[str, Any]:
    payload = {
        "status": tool_result.get("status"),
        "tool_name": tool_result.get("tool_name"),
        "action": tool_result.get("action"),
        "source_of_truth": tool_result.get("source_of_truth"),
        "verified_summary_text": tool_result.get("verified_summary_text"),
        "summary": tool_result.get("summary"),
    }
    for key in [
        "comparison",
        "buyer_report",
        "policy_command",
        "latest_policy_decision",
        "negative_components",
        "improvement_plan",
        "failed_verifier_results",
        "incident_packets",
        "product_category_pack",
    ]:
        if key in tool_result:
            if key == "product_category_pack" and isinstance(tool_result[key], dict):
                payload[key] = compact_product_category_pack(tool_result[key])
            else:
                payload[key] = tool_result[key]
    return payload


def call_gemma_final_response(
    prompt: str,
    router: dict[str, Any],
    tool_result: dict[str, Any],
    model: str,
    ollama_url: str,
    *,
    client_id: str | None,
    request_id: str | None,
) -> dict[str, Any]:
    system = (
        "You are the ContextGraph evidence explainer. Use only the JSON tool result. "
        "If verified_summary_text is present, copy that sentence exactly after the label 'Verified summary:'. "
        "Then add at most one short sentence saying the detailed source-of-truth records are shown in the evidence pane. "
        "Do not add any path, hash, metric, count, status, or conclusion that is not copied from verified_summary_text."
    )
    assistant_message = {
        "role": "assistant",
        "content": "",
        "tool_calls": router["actual_model_message"]["tool_calls"],
    }
    body = {
        "model": model,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": prompt},
            assistant_message,
            {
                "role": "tool",
                "tool_name": router["selected_tool"]["name"],
                "content": json.dumps(model_tool_result_payload(tool_result), sort_keys=True),
            },
        ],
        "tools": AGENT_TOOL_DEFINITIONS,
        "stream": False,
        "think": False,
        "options": {"temperature": 0},
    }
    publish(
        "model",
        "asking selected model to explain verified tool result",
        details={"tool_name": router["selected_tool"]["name"]},
        client_id=client_id,
        request_id=request_id,
    )
    response = http_post_json(f"{ollama_url.rstrip('/')}/api/chat", body, timeout=300.0)
    message = response.get("message")
    if not isinstance(message, dict):
        raise FrontendError(
            "GEMMAFRONTEND_FINAL_RESPONSE_INVALID",
            "Ollama final response did not contain a message object",
            "ollama.final_response.message",
            "inspect Ollama chat API compatibility",
            source_of_truth=f"{ollama_url.rstrip('/')}/api/chat",
            details={"response": response},
            http_status=502,
        )
    if message.get("tool_calls"):
        raise FrontendError(
            "GEMMAFRONTEND_FINAL_RESPONSE_TOOL_CALL_UNEXPECTED",
            "selected model attempted another tool call instead of explaining the verified result",
            "ollama.final_response.message.tool_calls",
            "rerun with a narrower prompt or strengthen the final-response system instruction",
            details={"response": response},
            http_status=502,
        )
    content = message.get("content")
    if not isinstance(content, str) or not content.strip():
        raise FrontendError(
            "GEMMAFRONTEND_FINAL_RESPONSE_EMPTY",
            "selected model final response was empty after tool execution",
            "ollama.final_response.message.content",
            "inspect the local model output and retry",
            details={"response": response},
            http_status=502,
        )
    return {
        "content_redacted_preview": redact_preview(content, limit=2000),
        "content": content,
        "ollama_response_metadata": {
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


def attach_agent_trace(
    snapshot: dict[str, Any],
    router: dict[str, Any],
    tool_result: dict[str, Any],
    final_response: dict[str, Any],
    *,
    client_id: str | None,
    request_id: str | None,
) -> dict[str, Any]:
    snapshot["chat_agent"] = {
        "mode": "multi_tool_contextgraph_gateway",
        "client_id": client_id,
        "request_id": request_id,
        "operator_prompt_sha256": router["operator_prompt_sha256"],
        "operator_prompt_redacted_preview": router["operator_prompt_redacted_preview"],
        "available_tool_names": AGENT_TOOL_NAMES,
        "selected_tool": router["selected_tool"],
        "router_model_message": router["actual_model_message"],
        "tool_contract": router["tool_contract"],
        "tool_result_summary": model_tool_result_payload(tool_result),
        "final_model_response": final_response,
        "source_of_truth": snapshot.get("frontend_persistence", {}).get("source_of_truth"),
    }
    interaction = snapshot.setdefault("model_interaction", {})
    interaction["truth_statement"] = (
        "The selected model chose one ContextGraph tool, ContextGraph validated and executed it, "
        "then the model explained the result from verified tool evidence."
    )
    interaction["gateway_tool_router"] = {
        "available_tool_names": AGENT_TOOL_NAMES,
        "selected_tool": router["selected_tool"],
        "final_response_redacted_preview": final_response["content_redacted_preview"],
    }
    snapshot.setdefault("timeline", []).insert(
        0,
        {
            "stage": "Model",
            "status": "passed",
            "title": "Selected one ContextGraph tool",
            "value": router["selected_tool"]["name"],
            "meaning": "The selected model chose from the exposed tool surface; ContextGraph validated the name and arguments before execution.",
            "evidence": {
                "operator_prompt_sha256": router["operator_prompt_sha256"],
                "tool_call_id": router["selected_tool"].get("tool_call_id"),
                "arguments": router["selected_tool"].get("arguments"),
            },
        },
    )
    persistence = snapshot.get("frontend_persistence")
    if isinstance(persistence, dict) and persistence.get("snapshot_path"):
        path = Path(str(persistence["snapshot_path"]))
        write_json(path, snapshot)
        readback = load_json(path, "frontend_persistence.snapshot_path")
        readback_tool = readback.get("chat_agent", {}).get("selected_tool", {}).get("name")
        if readback_tool != router["selected_tool"]["name"]:
            raise FrontendError(
                "GEMMAFRONTEND_AGENT_TRACE_READBACK_MISMATCH",
                "persisted snapshot did not contain the selected agent tool after rewrite",
                "snapshot.chat_agent.selected_tool.name",
                "fix final snapshot persistence before trusting the UI state",
                source_of_truth=file_sot(path),
                details={"expected": router["selected_tool"]["name"], "actual": readback_tool},
                http_status=500,
            )
    publish(
        "evidence",
        "agent trace persisted after source-of-truth readback",
        details={
            "selected_tool": router["selected_tool"]["name"],
            "snapshot_path": snapshot.get("frontend_persistence", {}).get("snapshot_path"),
        },
        client_id=client_id,
        request_id=request_id,
    )
    return snapshot


def run_contextgraph_agent_chat(
    runtime_root: Path,
    message: Any,
    model: str,
    ollama_url: str,
    *,
    client_id: str | None,
    request_id: str | None,
) -> dict[str, Any]:
    prompt = validate_prompt(message)
    ensure_agent_preflight(runtime_root, model, ollama_url)
    publish(
        "chat",
        "accepted operator prompt",
        details={"prompt_sha256": sha256_bytes(prompt.encode()), "preview": redact_preview(prompt)},
        client_id=client_id,
        request_id=request_id,
    )
    router = call_agent_tool_router(prompt, model, ollama_url, client_id=client_id, request_id=request_id)
    snapshot, tool_result = execute_agent_tool(
        router,
        runtime_root,
        prompt,
        model,
        ollama_url,
        client_id=client_id,
        request_id=request_id,
    )
    final_response = call_gemma_final_response(
        prompt,
        router,
        tool_result,
        model,
        ollama_url,
        client_id=client_id,
        request_id=request_id,
    )
    return attach_agent_trace(
        snapshot,
        router,
        tool_result,
        final_response,
        client_id=client_id,
        request_id=request_id,
    )


def health(runtime_root: Path, model: str, ollama_url: str) -> dict[str, Any]:
    checks: dict[str, Any] = {
        "status": "ok",
        "generated_at": now_rfc3339(),
        "repo_root": str(REPO_ROOT),
        "runtime_root": str(runtime_root),
        "model": model,
        "ollama_url": ollama_url,
        "files": {},
        "disk": {},
        "ollama": {},
        "real_runs": scan_real_runs(runtime_root),
    }
    for name, path in {
        "context_graph_bin": REPO_ROOT / "target/debug/context-graph",
        "gemma_harness": REPO_ROOT / "scripts/blackboxfix_gemma_tool_demo.py",
        "intelligence_report": REPO_ROOT / "scripts/blackboxfix_demo_intelligence_report.py",
        "reward_spec": APP_ROOT / "reward_spec.verified_outcome.json",
    }.items():
        checks["files"][name] = {"path": str(path), "exists": path.exists(), "source_of_truth": file_sot(path)}
    runtime_root.mkdir(parents=True, exist_ok=True)
    disk = shutil.disk_usage(runtime_root)
    checks["disk"] = {"total": disk.total, "used": disk.used, "free": disk.free, "source_of_truth": file_sot(runtime_root)}
    try:
        tags = http_json(f"{ollama_url.rstrip('/')}/api/tags", timeout=2.5)
        models = [item.get("name") for item in tags.get("models", []) if isinstance(item, dict)]
        model_rows = [
            {
                "name": item.get("name"),
                "size": item.get("size"),
                "modified_at": item.get("modified_at"),
                "parameter_size": (item.get("details") or {}).get("parameter_size") if isinstance(item.get("details"), dict) else None,
                "quantization_level": (item.get("details") or {}).get("quantization_level") if isinstance(item.get("details"), dict) else None,
            }
            for item in tags.get("models", [])
            if isinstance(item, dict)
        ]
        checks["ollama"] = {
            "status": "ok",
            "model_present": model in models,
            "model_count": len(models),
            "models": model_rows,
            "source_of_truth": f"{ollama_url.rstrip('/')}/api/tags",
        }
    except FrontendError as err:
        checks["ollama"] = err.to_json()
    if any(not item["exists"] for item in checks["files"].values()):
        checks["status"] = "blocked"
    return checks


class AppHandler(BaseHTTPRequestHandler):
    server_version = "GemmaFrontend/0.1"

    def _send_json(self, value: Any, status: int = 200) -> None:
        data = json.dumps(value, indent=2, sort_keys=True).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Cache-Control", "no-store")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def _read_json(self) -> dict[str, Any]:
        length = int(self.headers.get("Content-Length", "0") or "0")
        if length > 1024 * 1024:
            raise FrontendError(
                "GEMMAFRONTEND_REQUEST_TOO_LARGE",
                "request body is too large",
                "http.content_length",
                "send a smaller JSON request",
                details={"content_length": length},
            )
        raw = self.rfile.read(length)
        try:
            value = json.loads(raw.decode("utf-8"))
        except json.JSONDecodeError as err:
            raise FrontendError(
                "GEMMAFRONTEND_REQUEST_JSON_INVALID",
                f"request JSON is invalid: {err}",
                "request.body",
                "send a valid JSON object",
            ) from err
        if not isinstance(value, dict):
            raise FrontendError(
                "GEMMAFRONTEND_REQUEST_SHAPE_INVALID",
                "request body must be a JSON object",
                "request.body",
                "send a JSON object",
            )
        return value

    @property
    def app_config(self) -> argparse.Namespace:
        return self.server.app_config  # type: ignore[attr-defined]

    def log_message(self, fmt: str, *args: Any) -> None:
        sys.stderr.write("[%s] %s\n" % (now_rfc3339(), fmt % args))

    def do_GET(self) -> None:
        try:
            parsed = urllib.parse.urlparse(self.path)
            if parsed.path == "/":
                return self._serve_static(STATIC_ROOT / "index.html")
            if parsed.path.startswith("/static/"):
                rel = parsed.path.removeprefix("/static/")
                return self._serve_static((STATIC_ROOT / rel).resolve())
            if parsed.path == "/api/health":
                return self._send_json(health(self.app_config.runtime_root, self.app_config.model, self.app_config.ollama_url))
            if parsed.path == "/api/tools":
                return self._send_json(agent_tool_catalog())
            if parsed.path == "/api/runs":
                return self._send_json({"status": "ok", "runs": scan_real_runs(self.app_config.runtime_root)})
            if parsed.path == "/api/category/latest":
                category_root = self.app_config.runtime_root / "product-category"
                packs = sorted(
                    category_root.glob("*/product-category-pack.json") if category_root.exists() else [],
                    key=lambda path: path.stat().st_mtime,
                    reverse=True,
                )
                if not packs:
                    raise FrontendError(
                        "GEMMAFRONTEND_CATEGORY_PACK_MISSING",
                        "no product category pack has been materialized yet",
                        "runtime.product_category",
                        "build the investor category pack from real runs first",
                        source_of_truth=file_sot(category_root),
                        http_status=404,
                    )
                latest_pack = load_json(packs[0], "product_category.latest")
                latest_pack["pack_file"] = artifact_record(packs[0].parent, "product-category-pack.json")
                return self._send_json(latest_pack)
            if parsed.path == "/api/events":
                return self._serve_events()
            if parsed.path == "/api/events/recent":
                query = urllib.parse.parse_qs(parsed.query)
                client_id = validate_correlation_id((query.get("client_id") or [None])[0], "query.client_id")
                request_id = validate_correlation_id((query.get("request_id") or [None])[0], "query.request_id")
                with EVENT_LOCK:
                    events = [
                        event
                        for event in EVENT_LOG
                        if event_matches(event, client_id, request_id)
                    ][-100:]
                return self._send_json(
                    {
                        "status": "ok",
                        "source_of_truth": "process:gemmafrontend.server.EVENT_LOG",
                        "client_id": client_id,
                        "request_id": request_id,
                        "events": events,
                    }
                )
            self.send_error(HTTPStatus.NOT_FOUND)
        except FrontendError as err:
            self._send_json(err.to_json(), err.http_status)

    def do_POST(self) -> None:
        try:
            parsed = urllib.parse.urlparse(self.path)
            body = self._read_json()
            client_id = validate_correlation_id(body.get("client_id"), "request.client_id")
            request_id = validate_correlation_id(body.get("request_id"), "request.request_id")
            if parsed.path == "/api/chat":
                snapshot = run_contextgraph_agent_chat(
                    self.app_config.runtime_root,
                    body.get("message"),
                    str(body.get("model") or self.app_config.model),
                    str(body.get("ollama_url") or self.app_config.ollama_url),
                    client_id=client_id,
                    request_id=request_id,
                )
                return self._send_json(snapshot)
            if parsed.path == "/api/inspect":
                root = validate_run_root(body.get("root"))
                snapshot = inspect_real_run(
                    self.app_config.runtime_root,
                    root,
                    str(body.get("model") or self.app_config.model),
                    compile_reward_requested=bool(body.get("compile_reward", False)),
                    client_id=client_id,
                    request_id=request_id,
                )
                return self._send_json(snapshot)
            if parsed.path == "/api/category/build":
                pack = build_product_category_pack(
                    self.app_config.runtime_root,
                    audience=body.get("audience"),
                    max_runs=body.get("max_runs"),
                    client_id=client_id,
                    request_id=request_id,
                )
                return self._send_json(pack)
            self.send_error(HTTPStatus.NOT_FOUND)
        except FrontendError as err:
            publish(
                "error",
                err.message,
                level="error",
                details=err.to_json(),
                client_id=locals().get("client_id"),
                request_id=locals().get("request_id"),
            )
            self._send_json(err.to_json(), err.http_status)

    def _serve_static(self, path: Path) -> None:
        resolved = path.resolve()
        if not str(resolved).startswith(str(STATIC_ROOT.resolve())):
            self.send_error(HTTPStatus.FORBIDDEN)
            return
        if not resolved.exists() or not resolved.is_file():
            self.send_error(HTTPStatus.NOT_FOUND)
            return
        data = resolved.read_bytes()
        content_type = mimetypes.guess_type(str(resolved))[0] or "application/octet-stream"
        self.send_response(200)
        self.send_header("Content-Type", content_type)
        self.send_header("Cache-Control", "no-store")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def _serve_events(self) -> None:
        parsed = urllib.parse.urlparse(self.path)
        query = urllib.parse.parse_qs(parsed.query)
        client_id = validate_correlation_id((query.get("client_id") or [None])[0], "query.client_id")
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream; charset=utf-8")
        self.send_header("Cache-Control", "no-cache")
        self.send_header("Connection", "keep-alive")
        self.end_headers()
        event_queue: queue.Queue[dict[str, Any]] = queue.Queue(maxsize=100)
        subscriber = {"client_id": client_id, "queue": event_queue}
        with EVENT_LOCK:
            for event in [item for item in EVENT_LOG if event_matches(item, client_id)][-100:]:
                event_queue.put_nowait(event)
            EVENT_QUEUES.append(subscriber)
        try:
            while True:
                try:
                    event = event_queue.get(timeout=10.0)
                    payload = json.dumps(event, sort_keys=True)
                    self.wfile.write(f"event: system\ndata: {payload}\n\n".encode("utf-8"))
                except queue.Empty:
                    self.wfile.write(b": keepalive\n\n")
                self.wfile.flush()
        except (BrokenPipeError, ConnectionResetError):
            pass
        finally:
            with EVENT_LOCK:
                if subscriber in EVENT_QUEUES:
                    EVENT_QUEUES.remove(subscriber)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=8787)
    parser.add_argument("--runtime-root", type=Path, default=DEFAULT_RUNTIME_ROOT)
    parser.add_argument("--model", default=DEFAULT_MODEL)
    parser.add_argument("--ollama-url", default=DEFAULT_OLLAMA_URL)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    args.runtime_root = args.runtime_root.resolve()
    args.runtime_root.mkdir(parents=True, exist_ok=True)
    publish("server", "ContextGraph model frontend server starting", details={"runtime_root": str(args.runtime_root), "model": args.model})
    server = ThreadingHTTPServer((args.host, args.port), AppHandler)
    server.app_config = args  # type: ignore[attr-defined]
    print(json.dumps({"status": "ok", "url": f"http://{args.host}:{args.port}", "runtime_root": str(args.runtime_root)}, sort_keys=True), flush=True)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        return 130
    finally:
        server.server_close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
