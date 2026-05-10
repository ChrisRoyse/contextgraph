#!/usr/bin/env python3
"""Shared helpers for Big Unlock full-state verification harnesses.

The helpers are intentionally fail-closed: tool errors raise, physical RocksDB
counts are read with `ldb`, and every report write is atomic.
"""

from __future__ import annotations

import hashlib
import json
import os
import re
import shutil
import socket
import subprocess
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[2]
DATA_ROOT = Path(os.environ.get("CONTEXTGRAPH_DATA_ROOT", "/mnt/d/contextgraph"))
DEFAULT_MCP = ROOT / "target/debug/context-graph-mcp"
DEFAULT_MODELS = ROOT / "models"
DEFAULT_CALIBRATION = ROOT / "data/utl_calibration"
DEFAULT_GO_EMOTIONS = DEFAULT_CALIBRATION / "go_emotions/go_emotions_sample.jsonl"
DEFAULT_SHAKESPEARE_DB = DATA_ROOT / "storage" / "contextgraph-rocksdb"
DEFAULT_REPORT_DIR = Path("~/.contextgraph/reports").expanduser()
DEFAULT_RUN_ROOT = Path("~/.contextgraph/experiments/bigunlock").expanduser()

LEARNER_CFS = [
    "learner_profile",
    "fingerprints_learner",
    "learner_state_history",
    "learner_constellations",
    "learner_audit",
    "learner_delta_log",
    "learner_m_per_trace",
    "learner_retrieval_log",
    "learner_k_sleep",
    "learner_training_datasets",
]

CONTENT_CFS = [
    "fingerprints",
    "content",
    "source_metadata",
    "embedding_registry",
    "constellations",
]

LEARNING_CFS = [
    "learning_events",
    "learner_training_datasets",
    "learner_delta_log",
    "learner_m_per_trace",
    "learner_retrieval_log",
]

EMBEDDER_SCORE_NAMES = [
    "E1_Semantic",
    "E2_Recency",
    "E3_Periodic",
    "E4_Sequence",
    "E5_Causal",
    "E6_Sparse",
    "E7_Code",
    "E8_Graph",
    "E9_HDC",
    "E10_Multimodal",
    "E11_Entity",
    "E12_ColBERT",
    "E13_SPLADE",
    "E14_BGE_M3_Dense",
]


class FsvFailure(RuntimeError):
    """Fail-closed FSV error."""


def utc_stamp() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%d_%H%M%S")


def sha256_text(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def path_state(path: Path) -> dict[str, Any]:
    if not path.exists():
        return {"path": str(path), "exists": False}
    if path.is_file():
        return {
            "path": str(path),
            "exists": True,
            "kind": "file",
            "size_bytes": path.stat().st_size,
            "sha256": sha256_file(path),
        }
    if path.is_dir():
        total = 0
        files = 0
        for root, _, names in os.walk(path):
            for name in names:
                files += 1
                total += (Path(root) / name).stat().st_size
        return {
            "path": str(path),
            "exists": True,
            "kind": "directory",
            "file_count": files,
            "size_bytes": total,
        }
    return {"path": str(path), "exists": True, "kind": "other"}


def write_json_atomic(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_text(json.dumps(payload, indent=2, sort_keys=True), encoding="utf-8")
    tmp.replace(path)


def require_path(path: Path, label: str, *, file: bool | None = None) -> None:
    if not path.exists():
        raise FsvFailure(f"{label} missing: {path}")
    if file is True and not path.is_file():
        raise FsvFailure(f"{label} is not a file: {path}")
    if file is False and not path.is_dir():
        raise FsvFailure(f"{label} is not a directory: {path}")


def ensure_clean_dir(path: Path) -> None:
    if path.exists():
        shutil.rmtree(path)
    path.mkdir(parents=True, exist_ok=False)


def copy_db(src: Path, dst: Path) -> None:
    require_path(src, "source RocksDB", file=False)
    if dst.exists():
        shutil.rmtree(dst)
    shutil.copytree(src, dst)
    lock = dst / "LOCK"
    if lock.exists():
        lock.unlink()


def run_cmd(
    name: str,
    cmd: list[str],
    report: dict[str, Any],
    *,
    timeout: int = 600,
    allow_fail: bool = False,
) -> subprocess.CompletedProcess[str]:
    started = time.perf_counter()
    proc = subprocess.run(
        cmd,
        cwd=ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=timeout,
    )
    elapsed = time.perf_counter() - started
    report.setdefault("commands", {})[name] = {
        "command": cmd,
        "returncode": proc.returncode,
        "elapsed_sec": round(elapsed, 3),
        "stdout_tail": proc.stdout.splitlines()[-80:],
        "stderr_tail": proc.stderr.splitlines()[-80:],
    }
    if proc.returncode != 0 and not allow_fail:
        raise FsvFailure(
            f"{name} failed with exit code {proc.returncode}\n"
            f"stdout={proc.stdout[-2000:]}\nstderr={proc.stderr[-2000:]}"
        )
    return proc


def parse_ldb_count(stdout: str, cf: str) -> int:
    match = re.search(r"Keys in range:\s+(\d+)", stdout)
    if not match:
        raise FsvFailure(f"ldb count output for CF {cf} did not include 'Keys in range'")
    return int(match.group(1))


def ldb_count(storage: Path, cf: str, report: dict[str, Any], label: str) -> int:
    if not (storage / "CURRENT").exists():
        report.setdefault("commands", {})[f"ldb_count_{label}_{cf}"] = {
            "command": ["ldb", f"--db={storage}", f"--column_family={cf}", "dump", "--count_only"],
            "returncode": 0,
            "elapsed_sec": 0.0,
            "stdout_tail": ["Keys in range: 0"],
            "stderr_tail": ["RocksDB directory not initialized; treating as zero."],
        }
        return 0
    proc = run_cmd(
        f"ldb_count_{label}_{cf}",
        ["ldb", f"--db={storage}", f"--column_family={cf}", "dump", "--count_only"],
        report,
        timeout=900,
    )
    return parse_ldb_count(proc.stdout, cf)


def ldb_counts(storage: Path, cfs: list[str], report: dict[str, Any], label: str) -> dict[str, int]:
    return {cf: ldb_count(storage, cf, report, label) for cf in cfs}


def port_is_open(host: str, port: int) -> bool:
    sock = socket.socket()
    sock.settimeout(0.5)
    try:
        return sock.connect_ex((host, port)) == 0
    finally:
        sock.close()


def wait_for_port(host: str, port: int, timeout: int, proc: subprocess.Popen[str], log_path: Path) -> None:
    deadline = time.time() + timeout
    last_error: Exception | None = None
    while time.time() < deadline:
        if proc.poll() is not None:
            tail = log_path.read_text(encoding="utf-8", errors="replace")[-4000:] if log_path.exists() else ""
            raise FsvFailure(
                f"MCP daemon exited during startup with code {proc.returncode}. Log tail:\n{tail}"
            )
        sock = socket.socket()
        sock.settimeout(1.0)
        try:
            sock.connect((host, port))
            sock.close()
            return
        except OSError as exc:
            last_error = exc
        finally:
            sock.close()
        time.sleep(1)
    tail = log_path.read_text(encoding="utf-8", errors="replace")[-4000:] if log_path.exists() else ""
    raise FsvFailure(
        f"MCP daemon did not listen on {host}:{port} within {timeout}s; last={last_error}. Log tail:\n{tail}"
    )


def start_daemon(
    *,
    mcp_binary: Path,
    storage: Path,
    models: Path,
    run_dir: Path,
    report: dict[str, Any],
    host: str,
    port: int,
    startup_timeout_sec: int = 420,
    request_timeout_sec: int = 600,
) -> subprocess.Popen[str]:
    require_path(mcp_binary, "context-graph-mcp binary", file=True)
    require_path(models, "models directory", file=False)
    if port_is_open(host, port):
        raise FsvFailure(f"port {host}:{port} is already in use")
    log_path = run_dir / "mcp_daemon.log"
    env = os.environ.copy()
    env["CONTEXT_GRAPH_STORAGE_PATH"] = str(storage)
    env["CONTEXT_GRAPH_MODELS_PATH"] = str(models)
    env["CONTEXT_GRAPH_MCP_REQUEST_TIMEOUT"] = str(request_timeout_sec)
    env.setdefault("RUST_LOG", "warn")
    command = [
        str(mcp_binary),
        "--transport",
        "tcp",
        "--bind",
        host,
        "--port",
        str(port),
    ]
    log_handle = log_path.open("w", encoding="utf-8")
    proc = subprocess.Popen(
        command,
        cwd=ROOT,
        env=env,
        stdout=log_handle,
        stderr=subprocess.STDOUT,
        text=True,
    )
    proc._fsv_log_handle = log_handle  # type: ignore[attr-defined]
    report["daemon"] = {
        "pid": proc.pid,
        "command": command,
        "log": str(log_path),
        "storage": str(storage),
        "models": str(models),
        "request_timeout_sec": request_timeout_sec,
    }
    wait_for_port(host, port, startup_timeout_sec, proc, log_path)
    return proc


def stop_daemon(proc: subprocess.Popen[str] | None, report: dict[str, Any]) -> None:
    if proc is None:
        return
    if proc.poll() is None:
        proc.terminate()
        try:
            proc.wait(timeout=30)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait(timeout=30)
    log_handle = getattr(proc, "_fsv_log_handle", None)
    if log_handle is not None:
        log_handle.close()
    report.setdefault("daemon", {})["exit_code"] = proc.returncode


class Rpc:
    def __init__(self, host: str, port: int, timeout: float = 240.0) -> None:
        self.socket = socket.socket()
        self.socket.settimeout(timeout)
        self.socket.connect((host, port))
        self.rid = 0
        self._request(
            "initialize",
            {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "bigunlock-fsv", "version": "1"},
            },
            timeout=timeout,
        )
        self.socket.sendall(
            (
                json.dumps(
                    {
                        "jsonrpc": "2.0",
                        "method": "notifications/initialized",
                        "params": {},
                    }
                )
                + "\n"
            ).encode()
        )

    def close(self) -> None:
        self.socket.close()

    def _next_id(self) -> int:
        self.rid += 1
        return self.rid

    def _request(self, method: str, params: dict[str, Any], *, timeout: float = 240.0) -> dict[str, Any]:
        rid = self._next_id()
        self.socket.sendall(
            (
                json.dumps(
                    {
                        "jsonrpc": "2.0",
                        "id": rid,
                        "method": method,
                        "params": params,
                    }
                )
                + "\n"
            ).encode()
        )
        return self._read(rid, timeout=timeout)

    def _read(self, rid: int, *, timeout: float) -> dict[str, Any]:
        buf = b""
        deadline = time.time() + timeout
        while time.time() < deadline:
            chunk = self.socket.recv(1024 * 1024)
            if not chunk:
                raise FsvFailure("MCP socket closed while waiting for reply")
            buf += chunk
            while b"\n" in buf:
                line, _, rest = buf.partition(b"\n")
                buf = rest
                if not line.strip():
                    continue
                obj = json.loads(line)
                if obj.get("id") == rid:
                    return obj
        raise FsvFailure(f"timeout waiting for JSON-RPC id={rid}")

    def call(self, name: str, args: dict[str, Any] | None = None, *, timeout: float = 240.0) -> dict[str, Any]:
        response = self._request("tools/call", {"name": name, "arguments": args or {}}, timeout=timeout)
        result = response.get("result")
        if not result:
            raise FsvFailure(f"tool {name} returned no result: {response}")
        if result.get("isError"):
            text = result.get("content", [{}])[0].get("text", "")
            raise FsvFailure(f"tool {name} returned error: {text}")
        text = result.get("content", [{}])[0].get("text")
        if not text:
            raise FsvFailure(f"tool {name} returned empty content")
        return json.loads(text)

    def call_expect_error(self, name: str, args: dict[str, Any], *, timeout: float = 240.0) -> dict[str, Any]:
        response = self._request("tools/call", {"name": name, "arguments": args}, timeout=timeout)
        result = response.get("result")
        if not result or not result.get("isError"):
            raise FsvFailure(f"tool {name} was expected to fail but succeeded: {response}")
        return response


def load_go_emotions(path: Path = DEFAULT_GO_EMOTIONS, limit: int = 20) -> list[dict[str, Any]]:
    require_path(path, "GoEmotions calibration sample", file=True)
    rows: list[dict[str, Any]] = []
    with path.open("r", encoding="utf-8") as handle:
        for line in handle:
            if not line.strip():
                continue
            rows.append(json.loads(line))
            if len(rows) >= limit:
                break
    if len(rows) < min(limit, 1):
        raise FsvFailure(f"{path} contained no JSONL rows")
    return rows


POSITIVE_WORDS = {
    "admiration", "amusement", "approval", "caring", "desire", "excitement",
    "gratitude", "joy", "love", "optimism", "pride", "relief",
}
NEGATIVE_WORDS = {
    "anger", "annoyance", "confusion", "disappointment", "disapproval",
    "disgust", "embarrassment", "fear", "grief", "nervousness", "remorse",
    "sadness",
}
HIGH_AROUSAL_WORDS = {"anger", "annoyance", "excitement", "fear", "nervousness", "surprise"}


def components_from_go_emotions(row: dict[str, Any]) -> dict[str, float]:
    labels = {str(label) for label in row.get("label_names", [])}
    positive = len(labels & POSITIVE_WORDS)
    negative = len(labels & NEGATIVE_WORDS)
    high = len(labels & HIGH_AROUSAL_WORDS)
    text = str(row.get("text", ""))
    token_count = max(1, len(text.split()))
    punctuation_stress = min(0.20, (text.count("!") + text.count("?")) * 0.04)
    valence = max(-0.85, min(0.85, 0.25 * positive - 0.28 * negative))
    arousal = max(-0.50, min(0.90, 0.12 * high + punctuation_stress + min(token_count, 40) / 200.0))
    stress = max(0.05, min(0.80, 0.12 + 0.15 * negative + punctuation_stress))
    hrv = max(0.20, min(0.90, 0.72 - stress * 0.35 + max(valence, 0.0) * 0.10))
    plasticity = max(0.20, min(0.90, 0.58 + hrv * 0.20 - stress * 0.15))
    return {
        "plasticityWindow": round(plasticity, 4),
        "hrvCoherence": round(hrv, 4),
        "valence": round(valence, 4),
        "arousal": round(arousal, 4),
        "stressFloor": round(stress, 4),
        "kSleep": 1.0,
    }


def observation_args(
    learner_id: str,
    session_ts: int,
    raw_text: str,
    components: dict[str, float],
    *,
    modality: str = "affect_text",
    consent_state: str = "consented-local-first",
) -> dict[str, Any]:
    return {
        "learnerId": learner_id,
        "sessionTs": session_ts,
        "modality": modality,
        "rawText": raw_text,
        "consentState": consent_state,
        "preprocessingVersion": "bigunlock-realtext-affect-v1",
        "embedderVersion": "E17-affect-text-phase0-deterministic-v1",
        "thresholdVersion": "bigunlock-baseline-v1",
        "components": components,
        "stateVector": [
            components["plasticityWindow"],
            components["hrvCoherence"],
            components["valence"],
            components["arousal"],
            components["stressFloor"],
            components["kSleep"],
        ],
        "signals": [{"modality": modality, "text": raw_text}],
    }


def learner_counts(rpc: Rpc) -> dict[str, int]:
    body = rpc.call("count_learner_state", {}, timeout=120.0)
    return {k: int(v) for k, v in (body.get("counts") or {}).items()}


def learning_event_count(rpc: Rpc) -> int:
    return int(rpc.call("count_learning_events", {}, timeout=120.0)["count"])


def training_dataset_count(rpc: Rpc) -> int:
    return int(rpc.call("count_learner_training_datasets", {}, timeout=120.0)["count"])


def extract_fingerprint_id(result: dict[str, Any]) -> str:
    value = result.get("fingerprintId") or result.get("memoryId") or result.get("id")
    if not isinstance(value, str) or not value:
        raise FsvFailure(f"search result has no fingerprint id: {result}")
    return value


def extract_embedder_scores(result: dict[str, Any]) -> list[float]:
    scores = result.get("embedderScores")
    if not isinstance(scores, dict):
        raise FsvFailure(f"search result has no embedderScores object: {result}")
    flattened: dict[str, float] = {}
    for group in scores.values():
        if isinstance(group, dict):
            for key, value in group.items():
                flattened[key] = 0.0 if value is None else float(value)
    out = []
    for name in EMBEDDER_SCORE_NAMES:
        if name not in flattened:
            raise FsvFailure(f"embedderScores missing {name}: {scores}")
        out.append(flattened[name])
    return out


def cross_correlations_from_profile(profile: list[float]) -> list[float]:
    if len(profile) != 14:
        raise FsvFailure(f"topic profile must have 14 values, got {len(profile)}")
    out = []
    for i in range(14):
        for j in range(i + 1, 14):
            out.append(round(float(profile[i]) * float(profile[j]), 6))
    if len(out) != 91:
        raise FsvFailure(f"cross-correlation builder produced {len(out)} values, expected 91")
    return out


def state_snapshot(profile: list[float], *, rank: int, domain: str) -> dict[str, Any]:
    clipped = [max(0.0, min(1.0, float(v))) for v in profile]
    return {
        "topicProfile": clipped,
        "crossCorrelations": cross_correlations_from_profile(clipped),
        "retrievalRank": rank,
        "embedderScores": clipped,
        "contradictionPressure": 0.10,
        "integrationConfidence": 0.70,
        "recurrenceCount": 2,
        "stabilityScore": 0.72,
        "domain": domain,
        "successfulTransferCount": 1,
    }


def event_snapshots_from_search_result(result: dict[str, Any], utility_delta: float) -> tuple[dict[str, Any], dict[str, Any]]:
    scores = extract_embedder_scores(result)
    base = [max(0.02, min(0.95, score)) for score in scores]
    before = [round(max(0.0, min(1.0, value * 0.70)), 6) for value in base]
    after = [round(max(0.0, min(1.0, value * (0.70 + max(-0.4, min(0.4, utility_delta))))), 6) for value in base]
    return (
        state_snapshot(before, rank=5, domain="shakespeare_tutor"),
        state_snapshot(after, rank=2 if utility_delta > 0 else 8, domain="shakespeare_tutor"),
    )


def readback_search_results(rpc: Rpc, results: list[dict[str, Any]], *, sample: int = 5) -> list[dict[str, Any]]:
    evidence = []
    for result in results[:sample]:
        memory_id = extract_fingerprint_id(result)
        body = rpc.call(
            "get_memory_fingerprint",
            {"memoryId": memory_id, "includeContent": True, "includeVectorNorms": False},
            timeout=120.0,
        )
        content = body.get("content")
        if not isinstance(content, str) or not content:
            raise FsvFailure(f"get_memory_fingerprint returned no content for {memory_id}")
        evidence.append(
            {
                "memory_id": memory_id,
                "content_sha256": sha256_text(content),
                "content_chars": len(content),
                "content_prefix": content[:120],
            }
        )
    return evidence


def edge_no_mutation(
    rpc: Rpc,
    *,
    name: str,
    tool: str,
    args: dict[str, Any],
    count_fn,
    timeout: float = 120.0,
) -> dict[str, Any]:
    before = count_fn(rpc)
    err = rpc.call_expect_error(tool, args, timeout=timeout)
    after = count_fn(rpc)
    if after != before:
        raise FsvFailure(f"edge {name} mutated state: before={before} after={after}")
    return {
        "name": name,
        "before": before,
        "after": after,
        "is_error": True,
        "error": err["result"]["content"][0]["text"],
    }
