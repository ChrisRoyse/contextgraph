#!/usr/bin/env python3
"""Shared library for the Gemma+Shakespeare demo scripts.

Provides:
    Rpc           — TCP JSON-RPC client for the Context Graph daemon (port 3100)
    GemmaClient   — wraps `ollama generate` HTTP API (no shell dependency)
    score_text    — embed text via daemon, score against all Shakespeare
                    constellations, return ranked list

Hard rules: every error path fails fast with a descriptive RuntimeError.
NO fallbacks, NO mock data, NO swallowed exceptions.
"""
from __future__ import annotations

import json
import socket
import time
import urllib.error
import urllib.request
from dataclasses import dataclass
from typing import Iterable


# ---------------------------------------------------------------------------
# Context Graph TCP JSON-RPC client
# ---------------------------------------------------------------------------
class Rpc:
    def __init__(self, host: str = "127.0.0.1", port: int = 3100,
                 timeout: float = 240.0) -> None:
        self.s = socket.socket(); self.s.settimeout(timeout)
        try:
            self.s.connect((host, port))
        except ConnectionRefusedError as e:
            raise RuntimeError(
                f"Context Graph daemon refused connection on {host}:{port}. "
                f"Start it: `cd /home/cabdru/contextgraph && CUDARC_CUDA_VERSION=13000 "
                f"./target/release/context-graph-mcp --daemon-server-only --daemon-port 3100`"
            ) from e
        self._rid = 0
        # Init handshake — daemon panics if we skip this
        self._req("initialize", {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {"name": "gemma-shakespeare-demo", "version": "1"}})
        self.s.sendall((json.dumps({
            "jsonrpc": "2.0", "method": "notifications/initialized",
            "params": {}}) + "\n").encode())

    def _next_id(self) -> int:
        self._rid += 1
        return self._rid

    def _req(self, method: str, params: dict, timeout: float = 120.0) -> dict:
        rid = self._next_id()
        self.s.sendall((json.dumps({
            "jsonrpc": "2.0", "id": rid, "method": method,
            "params": params}) + "\n").encode())
        return self._read(rid, timeout)

    def _read(self, rid: int, timeout: float = 120.0) -> dict:
        buf = b""
        dl = time.time() + timeout
        while time.time() < dl:
            chunk = self.s.recv(65536)
            if not chunk:
                raise RuntimeError("daemon closed the socket mid-call")
            buf += chunk
            while b"\n" in buf:
                line, _, rest = buf.partition(b"\n")
                buf = rest
                if not line.strip():
                    continue
                obj = json.loads(line)
                if obj.get("id") == rid:
                    return obj
        raise TimeoutError(
            f"daemon did not reply to id={rid} within {timeout}s")

    def call(self, name: str, args: dict, timeout: float = 120.0) -> dict:
        r = self._req("tools/call",
                      {"name": name, "arguments": args}, timeout=timeout)
        if r.get("result", {}).get("isError"):
            raise RuntimeError(
                f"tool '{name}' returned error: "
                f"{r['result']['content'][0]['text']}")
        return json.loads(r["result"]["content"][0]["text"])

    def close(self) -> None:
        self.s.close()


# ---------------------------------------------------------------------------
# Ollama HTTP client for Gemma
# ---------------------------------------------------------------------------
@dataclass
class GemmaResponse:
    text: str
    eval_count: int
    eval_duration_ms: float
    total_duration_ms: float


class GemmaClient:
    def __init__(self, model: str = "gemma4:e4b",
                 host: str = "http://127.0.0.1:11434",
                 keep_alive: str = "5m") -> None:
        self.model = model
        self.host = host
        self.keep_alive = keep_alive

    def generate(self, prompt: str, system: str | None = None,
                 temperature: float = 0.85, max_tokens: int = 512,
                 stop: Iterable[str] | None = None,
                 think: bool = False, timeout: float = 180.0) -> GemmaResponse:
        body: dict = {
            "model": self.model,
            "prompt": prompt,
            "stream": False,
            "keep_alive": self.keep_alive,
            "options": {
                "temperature": temperature,
                "num_predict": max_tokens,
            },
        }
        if system:
            body["system"] = system
        if stop:
            body["options"]["stop"] = list(stop)
        # Gemma 4 has a "thinking" mode; disable for short demo outputs to
        # avoid the model wandering through self-reflection token chains.
        if not think:
            body["think"] = False
        req = urllib.request.Request(
            f"{self.host}/api/generate",
            data=json.dumps(body).encode(),
            headers={"Content-Type": "application/json"},
            method="POST")
        try:
            with urllib.request.urlopen(req, timeout=timeout) as resp:
                obj = json.loads(resp.read())
        except urllib.error.URLError as e:
            raise RuntimeError(
                f"Ollama unreachable at {self.host}: {e}. "
                f"Check `systemctl status ollama` and `ollama list`.") from e
        return GemmaResponse(
            text=obj.get("response", ""),
            eval_count=obj.get("eval_count", 0),
            eval_duration_ms=obj.get("eval_duration", 0) / 1e6,
            total_duration_ms=obj.get("total_duration", 0) / 1e6,
        )


# ---------------------------------------------------------------------------
# Constellation scoring helper
# ---------------------------------------------------------------------------
SHAKESPEARE_LABEL_PREFIX = "Shakespeare"


@dataclass
class Score:
    work: str               # human-readable work label
    constellation_id: str
    combined_score: float
    in_spread_p95: bool
    per_embedder: list[float]   # 13 cosines
    members: int


def list_shakespeare_constellations(rpc: Rpc) -> list[dict]:
    body = rpc.call("list_constellations", {"limit": 200}, timeout=30.0)
    return [c for c in body["constellations"]
            if (c.get("label") or "").startswith(SHAKESPEARE_LABEL_PREFIX)]


def store_probe(rpc: Rpc, text: str, session: str = "demo-probe",
                rationale: str = "auto-probe") -> str:
    """Store a transient memory and return its fingerprintId."""
    if not text or not text.strip():
        raise ValueError("store_probe: text cannot be empty")
    body = rpc.call("store_memory", {
        "content": text, "sessionId": session,
        "importance": 0.5, "rationale": rationale[:1024],
    }, timeout=120.0)
    fp = body.get("fingerprintId")
    if not fp:
        raise RuntimeError(f"store_memory returned no fingerprintId: {body}")
    return fp


def score_text(rpc: Rpc, text: str,
               cons_cache: list[dict] | None = None) -> tuple[str, list[Score]]:
    """Embed `text`, score against every Shakespeare constellation.

    Returns (fingerprint_id, sorted_scores_descending).
    Caller is responsible for soft-deleting the fingerprint via forget_concept
    if they don't want it persisted.
    """
    cons = cons_cache or list_shakespeare_constellations(rpc)
    if not cons:
        raise RuntimeError(
            "No Shakespeare constellations found in CF_CONSTELLATIONS. "
            "Run shakespeare_extract.py first to compile them.")
    fp = store_probe(rpc, text, rationale=f"score_text: {text[:60]}")
    scores: list[Score] = []
    for c in cons:
        r = rpc.call("score_against_constellation", {
            "constellationId": c["constellation_id"],
            "memoryId": fp,
        }, timeout=30.0)
        scores.append(Score(
            work=c["label"].replace("Shakespeare — ", ""),
            constellation_id=c["constellation_id"],
            combined_score=r.get("combined_score") or 0.0,
            in_spread_p95=bool(r.get("in_spread_p95")),
            per_embedder=r.get("per_embedder_cosine") or [],
            members=c.get("member_count") or 0,
        ))
    scores.sort(key=lambda s: -s.combined_score)
    return fp, scores


def authenticity_verdict(scores: list[Score]) -> tuple[str, str, dict]:
    """Return (short, long, metrics) for the verdict logic.

    Used by the standalone authenticity tool AND the constellation guard.
    """
    if not scores:
        raise ValueError("authenticity_verdict: empty scores list")
    top = scores[0]
    median = sorted(s.combined_score for s in scores)[len(scores) // 2]
    spread = top.combined_score - median
    metrics = {
        "top_score": top.combined_score,
        "top_work": top.work,
        "median": median,
        "spread": spread,
        "in_p95": top.in_spread_p95,
        "second_score": scores[1].combined_score if len(scores) > 1 else None,
        "second_work": scores[1].work if len(scores) > 1 else None,
    }
    if top.in_spread_p95:
        return ("AUTHENTIC",
                f"AUTHENTIC SHAKESPEARE — within 95th-pct spread of {top.work}",
                metrics)
    if spread >= 0.08 and top.combined_score >= 0.45:
        return ("SHAKESPEAREAN",
                f"STYLISTICALLY SHAKESPEAREAN (peak={top.work}, spread=+{spread:.3f})",
                metrics)
    if spread >= 0.04 and top.combined_score >= 0.40:
        return ("ARCHAIC", "ARCHAIC / SHAKESPEAREAN-ADJACENT", metrics)
    return ("REJECTED",
            "MODERN / OUT-OF-DISTRIBUTION — would be REJECTED by constellation guard",
            metrics)


def soft_delete(rpc: Rpc, fp: str) -> None:
    """Best-effort cleanup of a probe memory. Logs but does not raise."""
    try:
        # forget_concept's required field is `node_id`, not `memoryId`
        rpc.call("forget_concept",
                 {"node_id": fp, "soft_delete": True,
                  "operator_id": "shakespeare-demo-probe"})
    except Exception as e:
        # Print but do not crash — the caller's data is fine either way
        print(f"  (warn: soft_delete({fp[:8]}…) failed: {e})", file=__import__("sys").stderr)
