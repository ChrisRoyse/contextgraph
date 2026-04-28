#!/usr/bin/env python3
"""Full-state verification for ingesting Shakespeare's complete works.

Happy path uses only the real Project Gutenberg corpus at
`shakespear/complete_works.txt`. The script:

1. Parses the complete works into scene/sonnet/poem chunks.
2. Starts a real context-graph MCP TCP server against a real RocksDB path.
3. Stores every chunk through `store_memories` batches.
4. Performs independent readback through MCP tools.
5. Exercises retrieval, constellation compilation, and training export.
6. Stops the daemon and inspects RocksDB column families with `ldb`.
7. Writes a JSON report with command logs, persisted paths, counts, and timings.

No mock corpus data is used. The only synthetic inputs are negative edge cases
that must fail without mutating the source-of-truth column families.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import socket
import subprocess
import sys
import time
from collections import Counter, defaultdict
from dataclasses import asdict
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

sys.dont_write_bytecode = True


ROOT = Path(__file__).resolve().parents[2]
SCRIPTS = ROOT / "scripts"
sys.path.insert(0, str(SCRIPTS))

import shakespeare_chunker  # noqa: E402


DEFAULT_CORPUS = ROOT / "shakespear/complete_works.txt"
DEFAULT_STORAGE = ROOT / "contextgraph_data"
DEFAULT_MCP = ROOT / "target/debug/context-graph-mcp"
DEFAULT_MODELS = ROOT / "models"
DEFAULT_REPORT = Path("~/.contextgraph/reports/shakespeare_corpus_fsv_latest.json").expanduser()
DEFAULT_RUN_ROOT = Path("~/.contextgraph/experiments/shakespeare_corpus_fsv").expanduser()

REQUIRED_CFS = [
    "fingerprints",
    "content",
    "source_metadata",
    "e1_matryoshka_128",
    "e12_late_interaction",
    "embedding_registry",
    "audit_log",
    "audit_by_target",
    "training_records",
    "constellations",
]


class CorpusFsvFailure(RuntimeError):
    """Fail-closed FSV error."""


def utc_stamp() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%d_%H%M%S")


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def sha256_text(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


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
        raise CorpusFsvFailure(f"{label} missing: {path}")
    if file is True and not path.is_file():
        raise CorpusFsvFailure(f"{label} is not a file: {path}")
    if file is False and not path.is_dir():
        raise CorpusFsvFailure(f"{label} is not a directory: {path}")


def run_cmd(name: str, cmd: list[str], report: dict[str, Any], *, timeout: int = 600) -> subprocess.CompletedProcess[str]:
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
    if proc.returncode != 0:
        raise CorpusFsvFailure(
            f"{name} failed with exit code {proc.returncode}: {proc.stderr[-2000:]}"
        )
    return proc


def parse_ldb_count(stdout: str, cf: str) -> int:
    match = re.search(r"Keys in range:\s+(\d+)", stdout)
    if not match:
        raise CorpusFsvFailure(f"ldb count output for CF {cf} did not include 'Keys in range'")
    return int(match.group(1))


def ldb_count(storage: Path, cf: str, report: dict[str, Any], label: str) -> int:
    if label == "before" and not (storage / "CURRENT").exists():
        report.setdefault("commands", {})[f"ldb_count_{label}_{cf}"] = {
            "command": ["ldb", f"--db={storage}", f"--column_family={cf}", "dump", "--count_only"],
            "returncode": 0,
            "elapsed_sec": 0.0,
            "stdout_tail": ["Keys in range: 0"],
            "stderr_tail": ["RocksDB directory not initialized yet; treating before-state as zero."],
        }
        return 0
    proc = run_cmd(
        f"ldb_count_{label}_{cf}",
        ["ldb", f"--db={storage}", f"--column_family={cf}", "dump", "--count_only"],
        report,
        timeout=900,
    )
    return parse_ldb_count(proc.stdout, cf)


def ldb_counts(storage: Path, report: dict[str, Any], label: str) -> dict[str, int]:
    return {cf: ldb_count(storage, cf, report, label) for cf in REQUIRED_CFS}


def wait_for_port(host: str, port: int, timeout: int, proc: subprocess.Popen[str], log_path: Path) -> None:
    deadline = time.time() + timeout
    last_error: Exception | None = None
    while time.time() < deadline:
        if proc.poll() is not None:
            tail = log_path.read_text(encoding="utf-8", errors="replace")[-4000:] if log_path.exists() else ""
            raise CorpusFsvFailure(
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
    raise CorpusFsvFailure(
        f"MCP daemon did not listen on {host}:{port} within {timeout}s; last={last_error}. Log tail:\n{tail}"
    )


def port_is_open(host: str, port: int) -> bool:
    sock = socket.socket()
    sock.settimeout(0.5)
    try:
        return sock.connect_ex((host, port)) == 0
    finally:
        sock.close()


class Rpc:
    def __init__(self, host: str, port: int, timeout: float = 240.0) -> None:
        self.socket = socket.socket()
        self.socket.settimeout(timeout)
        self.socket.connect((host, port))
        self.rid = 0
        self._request("initialize", {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {"name": "shakespeare-corpus-fsv", "version": "1"},
        }, timeout=timeout)
        self.socket.sendall((json.dumps({
            "jsonrpc": "2.0",
            "method": "notifications/initialized",
            "params": {},
        }) + "\n").encode())

    def close(self) -> None:
        self.socket.close()

    def _next_id(self) -> int:
        self.rid += 1
        return self.rid

    def _request(self, method: str, params: dict[str, Any], *, timeout: float = 240.0) -> dict[str, Any]:
        rid = self._next_id()
        self.socket.sendall((json.dumps({
            "jsonrpc": "2.0",
            "id": rid,
            "method": method,
            "params": params,
        }) + "\n").encode())
        return self._read(rid, timeout=timeout)

    def _read(self, rid: int, *, timeout: float) -> dict[str, Any]:
        buf = b""
        deadline = time.time() + timeout
        while time.time() < deadline:
            chunk = self.socket.recv(1024 * 1024)
            if not chunk:
                raise CorpusFsvFailure("MCP socket closed while waiting for reply")
            buf += chunk
            while b"\n" in buf:
                line, _, rest = buf.partition(b"\n")
                buf = rest
                if not line.strip():
                    continue
                obj = json.loads(line)
                if obj.get("id") == rid:
                    return obj
        raise CorpusFsvFailure(f"timeout waiting for JSON-RPC id={rid}")

    def call(self, name: str, args: dict[str, Any] | None = None, *, timeout: float = 240.0) -> dict[str, Any]:
        response = self._request(
            "tools/call",
            {"name": name, "arguments": args or {}},
            timeout=timeout,
        )
        result = response.get("result")
        if not result:
            raise CorpusFsvFailure(f"tool {name} returned no result: {response}")
        if result.get("isError"):
            text = result.get("content", [{}])[0].get("text", "")
            raise CorpusFsvFailure(f"tool {name} returned error: {text}")
        text = result.get("content", [{}])[0].get("text")
        if not text:
            raise CorpusFsvFailure(f"tool {name} returned empty content")
        return json.loads(text)

    def call_expect_error(self, name: str, args: dict[str, Any], *, timeout: float = 240.0) -> dict[str, Any]:
        response = self._request(
            "tools/call",
            {"name": name, "arguments": args},
            timeout=timeout,
        )
        result = response.get("result")
        if not result or not result.get("isError"):
            raise CorpusFsvFailure(f"tool {name} was expected to fail but succeeded: {response}")
        return response


def start_daemon(args: argparse.Namespace, run_dir: Path, report: dict[str, Any]) -> subprocess.Popen[str]:
    if port_is_open(args.host, args.port):
        raise CorpusFsvFailure(f"port {args.host}:{args.port} is already in use")
    log_path = run_dir / "mcp_daemon.log"
    env = os.environ.copy()
    env["CONTEXT_GRAPH_STORAGE_PATH"] = str(args.storage)
    env["CONTEXT_GRAPH_MODELS_PATH"] = str(args.models)
    env["CONTEXT_GRAPH_MCP_REQUEST_TIMEOUT"] = str(args.mcp_request_timeout_sec)
    env.setdefault("RUST_LOG", "warn")
    command = [
        str(args.mcp_binary),
        "--transport",
        "tcp",
        "--bind",
        args.host,
        "--port",
        str(args.port),
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
        "storage": str(args.storage),
        "models": str(args.models),
        "request_timeout_sec": args.mcp_request_timeout_sec,
    }
    wait_for_port(args.host, args.port, args.startup_timeout_sec, proc, log_path)
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


def summarize_chunks(chunks: list[Any]) -> dict[str, Any]:
    by_type = Counter(c.chunk_type for c in chunks)
    by_title = Counter(c.title for c in chunks)
    by_slug = Counter(c.slug for c in chunks)
    return {
        "count": len(chunks),
        "by_type": dict(sorted(by_type.items())),
        "works": len(by_title),
        "by_title": dict(sorted(by_title.items())),
        "by_slug": dict(sorted(by_slug.items())),
        "min_chars": min(len(c.content) for c in chunks),
        "max_chars": max(len(c.content) for c in chunks),
        "total_chars": sum(len(c.content) for c in chunks),
    }


def write_chunks(chunks: list[Any], path: Path) -> None:
    with path.open("w", encoding="utf-8") as handle:
        for chunk in chunks:
            handle.write(json.dumps(asdict(chunk), ensure_ascii=False, sort_keys=True))
            handle.write("\n")


def importance_for(chunk: Any) -> float:
    if chunk.chunk_type == "sonnet":
        return 0.75
    if chunk.chunk_type == "poem_chunk":
        return 0.70
    return 0.80


def rationale_for(chunk: Any) -> str:
    if chunk.chunk_type == "sonnet":
        return f"Shakespeare sonnet #{chunk.sonnet_number}"
    if chunk.chunk_type == "scene":
        return f"{chunk.title} - Act {chunk.act}, Scene {chunk.scene}"
    return f"{chunk.title} (chunk {chunk.chunk_index})"


def percentile(values: list[float], q: float) -> float:
    if not values:
        return 0.0
    ordered = sorted(values)
    idx = min(len(ordered) - 1, max(0, round((len(ordered) - 1) * q)))
    return float(ordered[idx])


def ingest_chunks(
    rpc: Rpc,
    chunks: list[Any],
    ingest_log: Path,
    *,
    batch_size: int,
    progress_every: int,
) -> dict[str, Any]:
    latencies = []
    batch_latencies = []
    embedder_counts: Counter[int] = Counter()
    by_type: Counter[str] = Counter()
    by_slug: Counter[str] = Counter()
    rows = []
    start = time.perf_counter()
    next_progress = progress_every if progress_every > 0 else None
    with ingest_log.open("w", encoding="utf-8") as handle:
        for batch_start in range(0, len(chunks), batch_size):
            batch = chunks[batch_start: batch_start + batch_size]
            items = []
            for chunk in batch:
                items.append({
                    "content": chunk.content,
                    "sessionId": f"shakespeare-{chunk.slug}",
                    "importance": importance_for(chunk),
                    "rationale": rationale_for(chunk)[:1024],
                })
            call_start = time.perf_counter()
            body = rpc.call(
                "store_memories",
                {
                    "items": items,
                    "operatorId": "shakespeare-corpus-fsv",
                    "causalHints": False,
                },
                timeout=max(300.0, 180.0 * len(batch)),
            )
            elapsed_ms = (time.perf_counter() - call_start) * 1000.0
            embedder_count = int(body.get("embedderCount") or 0)
            if embedder_count != 14:
                raise CorpusFsvFailure(
                    f"store_memories embedderCount={embedder_count}, expected 14 "
                    f"at batch starting {batch_start}"
                )
            response_items = body.get("items") or []
            if int(body.get("requestedCount") or 0) != len(batch):
                raise CorpusFsvFailure(
                    f"store_memories requestedCount={body.get('requestedCount')}, expected {len(batch)}"
                )
            if int(body.get("storedCount") or 0) != len(batch):
                raise CorpusFsvFailure(
                    f"store_memories storedCount={body.get('storedCount')}, expected {len(batch)}"
                )
            if len(response_items) != len(batch):
                raise CorpusFsvFailure(
                    f"store_memories returned {len(response_items)} item rows for {len(batch)} inputs"
                )
            batch_latencies.append(elapsed_ms)
            for offset, (chunk, response_item) in enumerate(zip(batch, response_items)):
                idx = batch_start + offset
                if int(response_item.get("index")) != offset:
                    raise CorpusFsvFailure(
                        f"store_memories response item index={response_item.get('index')} "
                        f"expected {offset} at chunk {idx}"
                    )
                fingerprint_id = response_item.get("fingerprintId")
                if not fingerprint_id:
                    raise CorpusFsvFailure(
                        f"store_memories returned no fingerprintId at chunk {idx}: {body}"
                    )
                session = f"shakespeare-{chunk.slug}"
                if response_item.get("sessionId") != session:
                    raise CorpusFsvFailure(
                        f"store_memories sessionId={response_item.get('sessionId')} "
                        f"expected {session} at chunk {idx}"
                    )
                row = {
                    "index": idx,
                    "fingerprint_id": fingerprint_id,
                    "session": session,
                    "title": chunk.title,
                    "slug": chunk.slug,
                    "chunk_type": chunk.chunk_type,
                    "chunk_index": chunk.chunk_index,
                    "content_sha256": sha256_text(chunk.content),
                    "content_chars": len(chunk.content),
                    "embedder_count": embedder_count,
                    "embedding_latency_ms": response_item.get("embeddingLatencyMs"),
                    "batch_embedding_latency_ms": body.get("embeddingLatencyMs"),
                    "round_trip_ms": elapsed_ms,
                    "batch_size": len(batch),
                }
                rows.append(row)
                handle.write(json.dumps(row, sort_keys=True))
                handle.write("\n")
                latencies.append(elapsed_ms / len(batch))
                embedder_counts[embedder_count] += 1
                by_type[chunk.chunk_type] += 1
                by_slug[chunk.slug] += 1
            handle.flush()
            completed = batch_start + len(batch)
            if next_progress is not None and completed >= next_progress:
                total_elapsed = time.perf_counter() - start
                rate = completed / total_elapsed if total_elapsed else 0.0
                print(
                    f"[ingest] {completed}/{len(chunks)} "
                    f"rate={rate:.2f}/s p50={percentile(latencies, 0.50):.1f}ms "
                    f"p95={percentile(latencies, 0.95):.1f}ms "
                    f"batch_p95={percentile(batch_latencies, 0.95):.1f}ms",
                    flush=True,
                )
                while next_progress <= completed:
                    next_progress += progress_every
    elapsed = time.perf_counter() - start
    return {
        "source_of_truth": str(ingest_log),
        "rows": len(rows),
        "sha256": sha256_file(ingest_log),
        "elapsed_sec": elapsed,
        "rate_per_sec": len(rows) / elapsed if elapsed else 0.0,
        "batch_size": batch_size,
        "round_trip_ms": {
            "min": min(latencies),
            "p50": percentile(latencies, 0.50),
            "p95": percentile(latencies, 0.95),
            "max": max(latencies),
        },
        "batch_round_trip_ms": {
            "min": min(batch_latencies),
            "p50": percentile(batch_latencies, 0.50),
            "p95": percentile(batch_latencies, 0.95),
            "max": max(batch_latencies),
        },
        "embedder_count_distribution": dict(embedder_counts),
        "by_type": dict(sorted(by_type.items())),
        "by_slug": dict(sorted(by_slug.items())),
        "first_rows": rows[:3],
        "last_rows": rows[-3:],
    }


def load_ingest_rows(path: Path) -> list[dict[str, Any]]:
    rows = []
    with path.open("r", encoding="utf-8") as handle:
        for line in handle:
            if line.strip():
                rows.append(json.loads(line))
    return rows


def verify_sample_fingerprints(rpc: Rpc, chunks: list[Any], rows: list[dict[str, Any]]) -> list[dict[str, Any]]:
    indexes = sorted(set([0, len(rows) // 2, len(rows) - 1]))
    evidence = []
    for idx in indexes:
        row = rows[idx]
        chunk = chunks[idx]
        body = rpc.call(
            "get_memory_fingerprint",
            {
                "memoryId": row["fingerprint_id"],
                "includeContent": True,
                "includeVectorNorms": True,
            },
            timeout=120.0,
        )
        content = body.get("content") or ""
        if sha256_text(content) != row["content_sha256"]:
            raise CorpusFsvFailure(f"content readback hash mismatch for index {idx}")
        embedders = body.get("embedders") or []
        if body.get("embeddersPresent") != 14 or len(embedders) != 14:
            raise CorpusFsvFailure(
                f"fingerprint {row['fingerprint_id']} reports "
                f"embeddersPresent={body.get('embeddersPresent')} len={len(embedders)}, expected 14"
            )
        missing = [e for e in embedders if not e.get("present")]
        if missing:
            raise CorpusFsvFailure(f"fingerprint {row['fingerprint_id']} missing embedders: {missing}")
        evidence.append({
            "index": idx,
            "memory_id": row["fingerprint_id"],
            "title": chunk.title,
            "chunk_type": chunk.chunk_type,
            "content_sha256": row["content_sha256"],
            "content_chars": len(content),
            "embedders_present": body["embeddersPresent"],
            "dimensions": {e["embedder"]: e["actualDimension"] for e in embedders},
            "created_at": body["createdAt"],
        })
    return evidence


def timed_tool(rpc: Rpc, name: str, args: dict[str, Any], *, timeout: float = 240.0) -> tuple[dict[str, Any], float]:
    started = time.perf_counter()
    body = rpc.call(name, args, timeout=timeout)
    return body, time.perf_counter() - started


def run_search_checks(rpc: Rpc) -> list[dict[str, Any]]:
    cases = [
        {
            "name": "hamlet_quote",
            "query": "To be, or not to be",
            "must_contain_any": ["to be", "hamlet"],
        },
        {
            "name": "macbeth_witches",
            "query": "Double, double toil and trouble",
            "must_contain_any": ["double", "trouble", "macbeth"],
        },
        {
            "name": "romeo_juliet",
            "query": "star-crossed lovers Romeo Juliet",
            "must_contain_any": ["romeo", "juliet", "star-cross"],
        },
    ]
    evidence = []
    for case in cases:
        body, elapsed = timed_tool(
            rpc,
            "search_graph",
            {
                "query": case["query"],
                "topK": 5,
                "includeContent": True,
                "strategy": "multi_space",
                "weightProfile": "semantic_search",
            },
            timeout=240.0,
        )
        results = body.get("results") or body.get("nodes") or []
        if not results:
            raise CorpusFsvFailure(f"search case {case['name']} returned zero results")
        joined = "\n".join(json.dumps(r, ensure_ascii=False).lower() for r in results[:3])
        if not any(term in joined for term in case["must_contain_any"]):
            raise CorpusFsvFailure(
                f"search case {case['name']} top results did not contain expected terms"
            )
        evidence.append({
            "name": case["name"],
            "query": case["query"],
            "elapsed_sec": elapsed,
            "result_count": len(results),
            "top_result": results[0],
        })
    return evidence


def compile_constellations(
    rpc: Rpc,
    chunk_summary: dict[str, Any],
    *,
    max_works: int | None,
) -> dict[str, Any]:
    by_slug: dict[str, int] = chunk_summary["by_slug"]
    eligible = [(slug, count) for slug, count in sorted(by_slug.items()) if count >= 3]
    if max_works is not None:
        eligible = eligible[:max_works]
    before = rpc.call("list_constellations", {"limit": 1, "offset": 0})
    before_total = int(before.get("total") or 0)
    compiled = []
    for slug, expected_count in eligible:
        body, elapsed = timed_tool(
            rpc,
            "compile_constellation",
            {
                "selector": "session",
                "sessionId": f"shakespeare-{slug}",
                "label": f"Shakespeare Complete Works - {slug}",
                "maxMembers": expected_count,
                "rebuildIfExists": False,
            },
            timeout=300.0,
        )
        if body.get("member_count") != expected_count:
            raise CorpusFsvFailure(
                f"constellation {slug} member_count={body.get('member_count')} expected={expected_count}"
            )
        readback = rpc.call(
            "get_constellation",
            {"constellationId": body["constellation_id"], "includeCentroids": False},
            timeout=120.0,
        )
        if readback.get("member_count") != expected_count:
            raise CorpusFsvFailure(f"constellation readback member mismatch for {slug}")
        compiled.append({
            "slug": slug,
            "constellation_id": body["constellation_id"],
            "member_count": body["member_count"],
            "coherence": body["coherence"],
            "elapsed_sec": elapsed,
        })
    after = rpc.call("list_constellations", {"limit": 1, "offset": 0})
    after_total = int(after.get("total") or 0)
    if after_total < before_total + len(compiled):
        raise CorpusFsvFailure(
            f"constellation total did not increase enough: before={before_total} "
            f"after={after_total} compiled={len(compiled)}"
        )
    return {
        "before_total": before_total,
        "after_total": after_total,
        "compiled": compiled,
    }


def export_training_records(rpc: Rpc, expected_count: int) -> dict[str, Any]:
    before = rpc.call("count_training_records", {}, timeout=120.0)
    before_count = int(before.get("count") or 0)
    body, elapsed = timed_tool(
        rpc,
        "export_training_corpus",
        {
            "filter": "all",
            "maxMemories": expected_count,
            "includeEmbeddings": True,
            "includeSparseVectors": True,
            "includeTokenEmbeddings": False,
            "includeEdges": True,
            "includeCausalLabels": True,
            "includeIncomingEdges": False,
            "includeTemporalLabels": True,
            "includeTuckerCore": False,
            "clearExisting": True,
        },
        timeout=1800.0,
    )
    after = rpc.call("count_training_records", {}, timeout=120.0)
    after_count = int(after.get("count") or 0)
    if after_count != expected_count:
        raise CorpusFsvFailure(
            f"CF_TRAINING_RECORDS count={after_count}, expected {expected_count}"
        )
    sample = rpc.call("list_training_records", {"limit": 3, "includeShape": True}, timeout=120.0)
    return {
        "before_count": before_count,
        "after_count": after_count,
        "elapsed_sec": elapsed,
        "export_response": body,
        "sample": sample,
    }


def run_edge_cases(rpc: Rpc, *, phoenix_session: str | None) -> list[dict[str, Any]]:
    edge_cases = []

    before = rpc.call("get_memetic_status", {}, timeout=120.0)
    before_count = int(before["fingerprintCount"])
    err = rpc.call_expect_error("store_memory", {"content": "", "sessionId": "edge-empty"}, timeout=120.0)
    after = rpc.call("get_memetic_status", {}, timeout=120.0)
    after_count = int(after["fingerprintCount"])
    if after_count != before_count:
        raise CorpusFsvFailure("empty content edge case mutated fingerprint count")
    edge_cases.append({
        "name": "empty_content_store",
        "before_count": before_count,
        "after_count": after_count,
        "is_error": True,
        "error": err["result"]["content"][0]["text"],
    })

    before = rpc.call("get_memetic_status", {}, timeout=120.0)
    before_count = int(before["fingerprintCount"])
    oversized = "A" * 10001
    err = rpc.call_expect_error(
        "store_memory",
        {"content": oversized, "sessionId": "edge-oversized"},
        timeout=120.0,
    )
    after = rpc.call("get_memetic_status", {}, timeout=120.0)
    after_count = int(after["fingerprintCount"])
    if after_count != before_count:
        raise CorpusFsvFailure("oversized content edge case mutated fingerprint count")
    edge_cases.append({
        "name": "oversized_content_store",
        "before_count": before_count,
        "after_count": after_count,
        "is_error": True,
        "error": err["result"]["content"][0]["text"],
    })

    before = rpc.call("get_memetic_status", {}, timeout=120.0)
    before_count = int(before["fingerprintCount"])
    err = rpc.call_expect_error("store_memories", {"items": []}, timeout=120.0)
    after = rpc.call("get_memetic_status", {}, timeout=120.0)
    after_count = int(after["fingerprintCount"])
    if after_count != before_count:
        raise CorpusFsvFailure("empty batch edge case mutated fingerprint count")
    edge_cases.append({
        "name": "empty_batch_store_memories",
        "before_count": before_count,
        "after_count": after_count,
        "is_error": True,
        "error": err["result"]["content"][0]["text"],
    })

    before = rpc.call("get_memetic_status", {}, timeout=120.0)
    before_count = int(before["fingerprintCount"])
    err = rpc.call_expect_error(
        "store_memories",
        {
            "items": [{"content": "This should not store."}],
            "causalHints": "yes",
        },
        timeout=120.0,
    )
    after = rpc.call("get_memetic_status", {}, timeout=120.0)
    after_count = int(after["fingerprintCount"])
    if after_count != before_count:
        raise CorpusFsvFailure("invalid causalHints edge case mutated fingerprint count")
    edge_cases.append({
        "name": "invalid_causal_hints_store_memories",
        "before_count": before_count,
        "after_count": after_count,
        "is_error": True,
        "error": err["result"]["content"][0]["text"],
    })

    before = rpc.call("get_memetic_status", {}, timeout=120.0)
    before_count = int(before["fingerprintCount"])
    too_many = [{"content": f"Batch limit probe {i}"} for i in range(65)]
    err = rpc.call_expect_error("store_memories", {"items": too_many}, timeout=120.0)
    after = rpc.call("get_memetic_status", {}, timeout=120.0)
    after_count = int(after["fingerprintCount"])
    if after_count != before_count:
        raise CorpusFsvFailure("too-many-items edge case mutated fingerprint count")
    edge_cases.append({
        "name": "too_many_items_store_memories",
        "before_count": before_count,
        "after_count": after_count,
        "is_error": True,
        "error": err["result"]["content"][0]["text"],
    })

    if phoenix_session is not None:
        before_list = rpc.call("list_constellations", {"limit": 1}, timeout=120.0)
        before_total = int(before_list.get("total") or 0)
        err = rpc.call_expect_error(
            "compile_constellation",
            {
                "selector": "session",
                "sessionId": phoenix_session,
                "label": "Should fail - Phoenix has too few chunks",
                "maxMembers": 3,
            },
            timeout=240.0,
        )
        after_list = rpc.call("list_constellations", {"limit": 1}, timeout=120.0)
        after_total = int(after_list.get("total") or 0)
        if after_total != before_total:
            raise CorpusFsvFailure("too-few-members constellation edge mutated constellation count")
        edge_cases.append({
            "name": "too_few_members_constellation",
            "before_count": before_total,
            "after_count": after_total,
            "is_error": True,
            "error": err["result"]["content"][0]["text"],
        })

    return edge_cases


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--corpus", type=Path, default=DEFAULT_CORPUS)
    parser.add_argument("--storage", type=Path, default=DEFAULT_STORAGE)
    parser.add_argument("--mcp-binary", type=Path, default=DEFAULT_MCP)
    parser.add_argument("--models", type=Path, default=DEFAULT_MODELS)
    parser.add_argument("--run-dir", type=Path, default=None)
    parser.add_argument("--report", type=Path, default=DEFAULT_REPORT)
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=3310)
    parser.add_argument("--startup-timeout-sec", type=int, default=420)
    parser.add_argument("--mcp-request-timeout-sec", type=int, default=600,
                        help="Server-side MCP request timeout for long real-embedder batches.")
    parser.add_argument("--progress-every", type=int, default=25)
    parser.add_argument("--batch-size", type=int, default=8,
                        help="Batch size for store_memories. Must be 1-64.")
    parser.add_argument("--append", action="store_true",
                        help="Allow non-empty fingerprint CF. Default requires an empty target.")
    parser.add_argument("--limit", type=int, default=None,
                        help="Optional positive chunk limit for local debugging. Omit for full corpus.")
    parser.add_argument("--max-constellation-works", type=int, default=None,
                        help="Optional cap on work constellations. Omit to compile every eligible work.")
    parser.add_argument("--skip-training-export", action="store_true")
    return parser


def main() -> int:
    args = build_parser().parse_args()
    run_dir = args.run_dir or (DEFAULT_RUN_ROOT / utc_stamp())
    run_dir.mkdir(parents=True, exist_ok=False)
    chunks_path = run_dir / "shakespeare_chunks.jsonl"
    ingest_log = run_dir / "shakespeare_ingest.jsonl"

    report: dict[str, Any] = {
        "status": "running",
        "created_at": datetime.now(timezone.utc).isoformat(timespec="seconds"),
        "repo": str(ROOT),
        "run_dir": str(run_dir),
        "source_of_truth": {
            "corpus": str(args.corpus),
            "rocksdb": str(args.storage),
            "chunk_manifest": str(chunks_path),
            "ingest_log": str(ingest_log),
            "column_families": REQUIRED_CFS,
        },
        "inputs": {
            "corpus": path_state(args.corpus),
            "storage": path_state(args.storage),
            "mcp_binary": path_state(args.mcp_binary),
            "models": path_state(args.models),
        },
        "commands": {},
        "evidence": {},
        "benchmarks": {},
    }

    daemon: subprocess.Popen[str] | None = None
    rpc: Rpc | None = None
    try:
        require_path(args.corpus, "complete works corpus", file=True)
        require_path(args.storage, "RocksDB storage", file=False)
        require_path(args.mcp_binary, "context-graph-mcp binary", file=True)
        require_path(args.models, "models directory", file=False)
        if not 1 <= args.batch_size <= 64:
            raise CorpusFsvFailure("--batch-size must be between 1 and 64")
        if args.mcp_request_timeout_sec <= 0:
            raise CorpusFsvFailure("--mcp-request-timeout-sec must be positive")

        chunks = shakespeare_chunker.parse_complete_works(args.corpus)
        if args.limit is not None:
            if args.limit <= 0:
                raise CorpusFsvFailure("--limit must be positive when provided")
            chunks = chunks[:args.limit]
        if len(chunks) < 3:
            raise CorpusFsvFailure(f"need at least 3 chunks, got {len(chunks)}")
        write_chunks(chunks, chunks_path)
        chunk_summary = summarize_chunks(chunks)
        report["evidence"]["chunks"] = {
            **chunk_summary,
            "manifest_state": path_state(chunks_path),
        }

        before_ldb = ldb_counts(args.storage, report, "before")
        report["evidence"]["ldb_before"] = before_ldb
        if not args.append and before_ldb["fingerprints"] != 0:
            raise CorpusFsvFailure(
                f"target CF_FINGERPRINTS is not empty ({before_ldb['fingerprints']}); "
                "rerun with --append only if duplication is intentional"
            )

        daemon = start_daemon(args, run_dir, report)
        rpc = Rpc(args.host, args.port)
        daemon_status = rpc.call("daemon_status", {}, timeout=120.0)
        status_before = rpc.call("get_memetic_status", {}, timeout=120.0)
        report["evidence"]["mcp_before"] = {
            "daemon_status": daemon_status,
            "memetic_status": status_before,
        }
        if not args.append and int(status_before["fingerprintCount"]) != 0:
            raise CorpusFsvFailure(
                f"MCP status reports non-empty fingerprintCount={status_before['fingerprintCount']}"
            )

        edge_before = run_edge_cases(
            rpc,
            phoenix_session=None,
        )
        report["evidence"]["edge_cases_before_ingest"] = edge_before

        ingest_evidence = ingest_chunks(
            rpc,
            chunks,
            ingest_log,
            batch_size=args.batch_size,
            progress_every=args.progress_every,
        )
        report["evidence"]["ingest"] = ingest_evidence
        report["benchmarks"]["ingest"] = {
            "elapsed_sec": ingest_evidence["elapsed_sec"],
            "rate_per_sec": ingest_evidence["rate_per_sec"],
            "round_trip_ms": ingest_evidence["round_trip_ms"],
        }

        status_after_ingest = rpc.call("get_memetic_status", {}, timeout=120.0)
        expected_count = before_ldb["fingerprints"] + len(chunks)
        if int(status_after_ingest["fingerprintCount"]) != expected_count:
            raise CorpusFsvFailure(
                f"MCP fingerprintCount={status_after_ingest['fingerprintCount']}, "
                f"expected {expected_count}"
            )
        index_status = rpc.call("list_embedder_indexes", {}, timeout=120.0)
        if int(index_status.get("totalMemories") or 0) != expected_count:
            raise CorpusFsvFailure(
                f"list_embedder_indexes totalMemories={index_status.get('totalMemories')} "
                f"expected {expected_count}"
            )
        if len(index_status.get("indexes") or []) != 14:
            raise CorpusFsvFailure("list_embedder_indexes did not report 14 indexes")

        ingest_rows = load_ingest_rows(ingest_log)
        sample_readbacks = verify_sample_fingerprints(rpc, chunks, ingest_rows)
        search_evidence = run_search_checks(rpc)
        constellation_evidence = compile_constellations(
            rpc,
            chunk_summary,
            max_works=args.max_constellation_works,
        )
        phoenix_session = "shakespeare-the-phoenix-and-the-turtle"
        edge_after = run_edge_cases(rpc, phoenix_session=phoenix_session)

        report["evidence"]["mcp_after_ingest"] = status_after_ingest
        report["evidence"]["embedder_indexes"] = index_status
        report["evidence"]["sample_fingerprint_readbacks"] = sample_readbacks
        report["evidence"]["search"] = search_evidence
        report["evidence"]["constellations"] = constellation_evidence
        report["evidence"]["edge_cases_after_ingest"] = edge_after
        report["benchmarks"]["search"] = [
            {"name": row["name"], "elapsed_sec": row["elapsed_sec"]}
            for row in search_evidence
        ]
        report["benchmarks"]["constellations"] = {
            "compiled": len(constellation_evidence["compiled"]),
            "total_elapsed_sec": sum(row["elapsed_sec"] for row in constellation_evidence["compiled"]),
        }

        if not args.skip_training_export:
            training_evidence = export_training_records(rpc, expected_count)
            report["evidence"]["training_export"] = training_evidence
            report["benchmarks"]["training_export"] = {
                "elapsed_sec": training_evidence["elapsed_sec"],
                "records": training_evidence["after_count"],
            }

        rpc.close()
        rpc = None
        stop_daemon(daemon, report)
        daemon = None

        after_ldb = ldb_counts(args.storage, report, "after")
        report["evidence"]["ldb_after"] = after_ldb
        if after_ldb["fingerprints"] != before_ldb["fingerprints"] + len(chunks):
            raise CorpusFsvFailure(
                f"physical ldb CF_FINGERPRINTS count={after_ldb['fingerprints']}, "
                f"expected {before_ldb['fingerprints'] + len(chunks)}"
            )
        if after_ldb["content"] != before_ldb["content"] + len(chunks):
            raise CorpusFsvFailure(
                f"physical ldb CF_CONTENT count={after_ldb['content']}, "
                f"expected {before_ldb['content'] + len(chunks)}"
            )
        if after_ldb["source_metadata"] != before_ldb["source_metadata"] + len(chunks):
            raise CorpusFsvFailure(
                f"physical ldb CF_SOURCE_METADATA count={after_ldb['source_metadata']}, "
                f"expected {before_ldb['source_metadata'] + len(chunks)}"
            )
        if after_ldb["embedding_registry"] != before_ldb["embedding_registry"] + len(chunks):
            raise CorpusFsvFailure(
                f"physical ldb CF_EMBEDDING_REGISTRY count={after_ldb['embedding_registry']}, "
                f"expected {before_ldb['embedding_registry'] + len(chunks)}"
            )
        if after_ldb["audit_log"] < before_ldb["audit_log"] + len(chunks):
            raise CorpusFsvFailure(
                f"physical ldb CF_AUDIT_LOG count={after_ldb['audit_log']}, "
                f"expected at least {before_ldb['audit_log'] + len(chunks)}"
            )
        if after_ldb["audit_by_target"] < before_ldb["audit_by_target"] + len(chunks):
            raise CorpusFsvFailure(
                f"physical ldb CF_AUDIT_BY_TARGET count={after_ldb['audit_by_target']}, "
                f"expected at least {before_ldb['audit_by_target'] + len(chunks)}"
            )
        if not args.skip_training_export and after_ldb["training_records"] != expected_count:
            raise CorpusFsvFailure(
                f"physical ldb CF_TRAINING_RECORDS count={after_ldb['training_records']}, expected {expected_count}"
            )

        report["status"] = "ok"
    except Exception as exc:
        if rpc is not None:
            try:
                rpc.close()
            except Exception:
                pass
        stop_daemon(daemon, report)
        report["status"] = "failed"
        report["error"] = f"{type(exc).__name__}: {exc}"
        write_json_atomic(args.report, report)
        print(f"\nSHAKESPEARE CORPUS FSV FAILED: {exc}", file=sys.stderr)
        print(f"REPORT_WRITTEN={args.report}", file=sys.stderr)
        return 1

    write_json_atomic(args.report, report)
    print("\n=== SHAKESPEARE CORPUS FSV SUMMARY ===")
    print(json.dumps({
        "status": report["status"],
        "report": str(args.report),
        "run_dir": str(run_dir),
        "chunks": report["evidence"]["chunks"]["count"],
        "fingerprints": report["evidence"]["ldb_after"]["fingerprints"],
        "embedding_registry": report["evidence"]["ldb_after"]["embedding_registry"],
        "audit_log": report["evidence"]["ldb_after"]["audit_log"],
        "training_records": report["evidence"]["ldb_after"]["training_records"],
        "constellations": report["evidence"]["ldb_after"]["constellations"],
        "benchmarks": report["benchmarks"],
    }, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
