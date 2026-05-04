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

EVENT_LOG: list[dict[str, Any]] = []
EVENT_LOCK = threading.Lock()
EVENT_QUEUES: list[queue.Queue[dict[str, Any]]] = []


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


def publish(stage: str, message: str, *, level: str = "info", details: dict[str, Any] | None = None) -> dict[str, Any]:
    event = {
        "id": f"evt_{time.time_ns()}",
        "created_at": now_rfc3339(),
        "stage": stage,
        "level": level,
        "message": message,
        "details": details or {},
    }
    with EVENT_LOCK:
        EVENT_LOG.append(event)
        del EVENT_LOG[:-250]
        queues = list(EVENT_QUEUES)
    for item in queues:
        try:
            item.put_nowait(event)
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
            "evidence": reward or {"ledger_schema_version": ledger.get("schema_version"), "required_schema_version": REWARD_LEDGER_SCHEMA_VERSION},
        },
    ]
    return timeline


def build_snapshot(root: Path, *, frontend_context: dict[str, Any] | None = None) -> dict[str, Any]:
    harness = load_json(root / "harness-report.json", "harness_report")
    pipeline = load_json(root / "dynamicjepa/pipeline-result.json", "dynamicjepa.pipeline_result")
    scorecard = load_json(root / "blackbox/exports/scorecard.json", "blackbox.scorecard")
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
    reward = latest_payload(ledger_path, run_id, "reward_compilation")
    artifacts = collect_artifacts(root)
    calc = intelligence.get("calculated_intelligence", {})
    counts = dynamic_counts(pipeline)
    source_of_truth = harness.get("source_of_truth", {})
    model_io = harness.get("model_io", {})

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
    run_id = f"frontend_{int(time.time())}_{hashlib.sha256(snapshot['source_root'].encode()).hexdigest()[:8]}"
    out_dir = runtime_root / "frontend-runs" / run_id
    snapshot_path = out_dir / "system-snapshot.json"
    write_json(snapshot_path, snapshot)
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
    append_index(runtime_root, entry)
    snapshot["frontend_persistence"] = entry
    return snapshot


def scan_real_runs() -> list[dict[str, Any]]:
    roots = []
    for candidate in sorted(Path("/tmp").glob("blackboxfix-agentops-demo*"), key=lambda p: p.stat().st_mtime if p.exists() else 0, reverse=True):
        if not candidate.is_dir():
            continue
        if not (candidate / ".blackboxfix_agentops_demo_root").exists():
            continue
        harness = candidate / "harness-report.json"
        complete = harness.exists() and (candidate / "blackbox/blackbox.sqlite").exists()
        entry = {
            "root": str(candidate),
            "complete": complete,
            "mtime": candidate.stat().st_mtime,
            "source_of_truth": file_sot(candidate),
        }
        if complete:
            try:
                report = load_json(harness, "harness_report")
                entry["blackbox_run_id"] = report.get("blackbox_run_id")
                entry["model"] = report.get("model", {}).get("name")
            except FrontendError:
                entry["complete"] = False
        roots.append(entry)
        if len(roots) >= 20:
            break
    return roots


def materialize_intelligence(root: Path) -> None:
    script = REPO_ROOT / "scripts/blackboxfix_demo_intelligence_report.py"
    check_required_file(script, "scripts.blackboxfix_demo_intelligence_report")
    publish("intelligence", "materializing correlation intelligence from run artifacts", details={"root": str(root)})
    run_process([sys.executable, str(script), "--root", str(root)], timeout=120.0)
    artifact_record(root, "intelligence/correlation-intelligence.json")


def compile_reward(root: Path, model: str) -> dict[str, Any]:
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
    publish("reward", "compiling reward/loss from the scorecard and ledger", details={"run_id": run_id})
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
            "not enough free disk space for a live Gemma + DynamicJEPA run",
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
            "configured Gemma model is not present in Ollama",
            "ollama.models",
            "pull/load the exact model tag before running the live demo",
            source_of_truth=f"{ollama_url.rstrip('/')}/api/tags",
            details={"model": model, "available_models": models},
            http_status=503,
        )


def run_live_demo(runtime_root: Path, message: str, model: str, ollama_url: str, *, compile_reward_after: bool) -> dict[str, Any]:
    prompt = validate_prompt(message)
    ensure_live_preflight(runtime_root, model, ollama_url)
    root = runtime_root / "live-runs" / f"blackboxfix-agentops-demo-ui-{int(time.time())}-{hashlib.sha256(prompt.encode()).hexdigest()[:8]}"
    publish("chat", "accepted operator prompt", details={"prompt_sha256": sha256_bytes(prompt.encode()), "preview": redact_preview(prompt)})
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
    publish("gemma", "calling Ollama/Gemma and requiring one native tool call", details={"model": model})
    run_process(command, timeout=1800.0)
    publish("harness", "live harness completed; reading generated artifacts", details={"root": str(root)})
    materialize_intelligence(root)
    if compile_reward_after:
        compile_reward(root, model)
    snapshot = build_snapshot(
        root,
        frontend_context={
            "mode": "live",
            "operator_prompt_sha256": sha256_bytes(prompt.encode()),
            "operator_prompt_redacted_preview": redact_preview(prompt),
        },
    )
    persisted = persist_snapshot(runtime_root, snapshot, "live")
    publish("evidence", "system snapshot persisted after source-of-truth readback", details=persisted["frontend_persistence"])
    return persisted


def inspect_real_run(runtime_root: Path, root: Path, model: str, *, compile_reward_requested: bool) -> dict[str, Any]:
    publish("inspect", "reading completed real run artifacts", details={"root": str(root)})
    if compile_reward_requested:
        compile_reward(root, model)
    snapshot = build_snapshot(root, frontend_context={"mode": "inspect"})
    persisted = persist_snapshot(runtime_root, snapshot, "inspect")
    publish("inspect", "inspection snapshot persisted after SQLite/file readback", details=persisted["frontend_persistence"])
    return persisted


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
        "real_runs": scan_real_runs(),
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
        checks["ollama"] = {
            "status": "ok",
            "model_present": model in models,
            "model_count": len(models),
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
            if parsed.path == "/api/runs":
                return self._send_json({"status": "ok", "runs": scan_real_runs()})
            if parsed.path == "/api/events":
                return self._serve_events()
            self.send_error(HTTPStatus.NOT_FOUND)
        except FrontendError as err:
            self._send_json(err.to_json(), err.http_status)

    def do_POST(self) -> None:
        try:
            parsed = urllib.parse.urlparse(self.path)
            body = self._read_json()
            if parsed.path == "/api/chat":
                snapshot = run_live_demo(
                    self.app_config.runtime_root,
                    body.get("message"),
                    str(body.get("model") or self.app_config.model),
                    str(body.get("ollama_url") or self.app_config.ollama_url),
                    compile_reward_after=bool(body.get("compile_reward", True)),
                )
                return self._send_json(snapshot)
            if parsed.path == "/api/inspect":
                root = validate_run_root(body.get("root"))
                snapshot = inspect_real_run(
                    self.app_config.runtime_root,
                    root,
                    str(body.get("model") or self.app_config.model),
                    compile_reward_requested=bool(body.get("compile_reward", False)),
                )
                return self._send_json(snapshot)
            self.send_error(HTTPStatus.NOT_FOUND)
        except FrontendError as err:
            publish("error", err.message, level="error", details=err.to_json())
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
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream; charset=utf-8")
        self.send_header("Cache-Control", "no-cache")
        self.send_header("Connection", "keep-alive")
        self.end_headers()
        event_queue: queue.Queue[dict[str, Any]] = queue.Queue(maxsize=100)
        with EVENT_LOCK:
            for event in EVENT_LOG[-50:]:
                event_queue.put_nowait(event)
            EVENT_QUEUES.append(event_queue)
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
                if event_queue in EVENT_QUEUES:
                    EVENT_QUEUES.remove(event_queue)


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
    publish("server", "Gemma frontend server starting", details={"runtime_root": str(args.runtime_root), "model": args.model})
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
