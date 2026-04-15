#!/usr/bin/env python3
"""Stream Shakespeare chunks into the Context Graph daemon via TCP JSON-RPC.

One memory per chunk. sessionId = `shakespeare-<work-slug>` so each play
becomes a coherent session for downstream compile_constellation calls.
"""
from __future__ import annotations

import argparse
import json
import socket
import sys
import time
from pathlib import Path


def connect(host: str, port: int, timeout: float = 120.0) -> socket.socket:
    s = socket.socket()
    s.settimeout(timeout)
    s.connect((host, port))
    return s


def send(s: socket.socket, obj: dict) -> None:
    s.sendall((json.dumps(obj) + "\n").encode())


def reply(s: socket.socket, rid: int, timeout: float = 60.0) -> dict:
    buf = b""
    dl = time.time() + timeout
    while time.time() < dl:
        chunk = s.recv(65536)
        if not chunk:
            raise RuntimeError("socket closed")
        buf += chunk
        # Scan complete lines for matching id
        while b"\n" in buf:
            line, _, rest = buf.partition(b"\n")
            buf = rest
            if not line.strip():
                continue
            obj = json.loads(line)
            if obj.get("id") == rid:
                return obj
    raise TimeoutError(f"no reply for id={rid}")


def handshake(s: socket.socket) -> None:
    send(s, {
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {"protocolVersion": "2024-11-05",
                   "capabilities": {},
                   "clientInfo": {"name": "shakespeare-ingest", "version": "1"}},
    })
    reply(s, 1, timeout=120.0)
    send(s, {"jsonrpc": "2.0", "method": "notifications/initialized",
             "params": {}})


def build_rationale(chunk: dict) -> str:
    t = chunk["chunk_type"]
    if t == "sonnet":
        return f"Shakespeare sonnet #{chunk['sonnet_number']}"
    if t == "scene":
        return f"{chunk['title']} - Act {chunk['act']}, Scene {chunk['scene']}"
    return f"{chunk['title']} (chunk {chunk['chunk_index']})"


def importance_for(chunk: dict) -> float:
    # Slight bias: scenes weighted a bit higher than sonnets (more plot density);
    # sonnet 18/130 and Hamlet/Macbeth scenes are canonically salient, but we
    # don't special-case — uniform 0.8 lets the system learn salience from data.
    if chunk["chunk_type"] == "sonnet":
        return 0.75
    if chunk["chunk_type"] == "poem_chunk":
        return 0.70
    return 0.80


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--input", default="/tmp/shakespeare_chunks.jsonl")
    ap.add_argument("--host", default="127.0.0.1")
    ap.add_argument("--port", type=int, default=3100)
    ap.add_argument("--limit", type=int, default=None,
                    help="Max chunks to ingest (for smoke-tests)")
    ap.add_argument("--progress-every", type=int, default=25)
    ap.add_argument("--output", default="/tmp/shakespeare_ingest.jsonl",
                    help="Per-chunk result log with fingerprintId (default "
                         "matches extract.py / demo_queries.py / summary.py)")
    args = ap.parse_args()

    chunks = [json.loads(ln) for ln in Path(args.input).read_text().splitlines()
              if ln.strip()]
    if args.limit:
        chunks = chunks[:args.limit]
    print(f"Ingesting {len(chunks)} chunks from {args.input}")

    out = Path(args.output).open("w")

    s = connect(args.host, args.port, timeout=180.0)
    handshake(s)

    t0 = time.time()
    n_ok = 0
    n_err = 0
    rid = 100
    for i, chunk in enumerate(chunks):
        rid += 1
        session = f"shakespeare-{chunk['slug']}"
        send(s, {
            "jsonrpc": "2.0", "id": rid, "method": "tools/call",
            "params": {
                "name": "store_memory",
                "arguments": {
                    "content": chunk["content"],
                    "sessionId": session,
                    "importance": importance_for(chunk),
                    "rationale": build_rationale(chunk)[:1024],
                },
            },
        })
        try:
            r = reply(s, rid, timeout=90.0)
        except Exception as e:
            n_err += 1
            out.write(json.dumps({
                "index": i, "slug": chunk["slug"],
                "chunk_type": chunk["chunk_type"],
                "chunk_index": chunk["chunk_index"],
                "error": str(e)}) + "\n")
            continue
        if r.get("result", {}).get("isError"):
            n_err += 1
            out.write(json.dumps({"index": i, "slug": chunk["slug"],
                                   "error": r["result"]["content"][0]["text"]
                                   }) + "\n")
            continue
        body = json.loads(r["result"]["content"][0]["text"])
        fp = body.get("fingerprintId")
        if not fp:
            n_err += 1
            out.write(json.dumps({"index": i, "slug": chunk["slug"],
                                   "error": "no fingerprintId",
                                   "raw": body}) + "\n")
            continue
        n_ok += 1
        out.write(json.dumps({
            "index": i, "slug": chunk["slug"], "session": session,
            "chunk_type": chunk["chunk_type"],
            "chunk_index": chunk["chunk_index"],
            "fingerprint_id": fp,
            "embedder_count": body.get("embedderCount"),
            "embedding_latency_ms": body.get("embeddingLatencyMs"),
        }) + "\n")

        if (i + 1) % args.progress_every == 0:
            elapsed = time.time() - t0
            rate = (i + 1) / elapsed if elapsed > 0 else 0.0
            eta = (len(chunks) - i - 1) / rate if rate > 0 else 0.0
            print(f"  [{i+1:4d}/{len(chunks)}] ok={n_ok} err={n_err} "
                  f"elapsed={elapsed:.1f}s rate={rate:.1f}/s eta={eta:.0f}s",
                  flush=True)

    out.close()
    s.close()
    elapsed = time.time() - t0
    print(f"\nDone: {n_ok} ingested, {n_err} errors, {elapsed:.1f}s total")
    print(f"Rate: {n_ok / elapsed:.1f} memories/sec")
    print(f"Log: {args.output}")
    return 0 if n_err == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
