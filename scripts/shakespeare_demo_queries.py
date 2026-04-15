#!/usr/bin/env python3
"""Known-answer query proofs for the demo.

Each canonical Shakespeare phrase is tried across THREE retrieval strategies
(search_graph, search_by_keywords, search_by_tokens) and passes if ANY of
them ranks the expected work in the top K. session_id is the ground-truth
anchor, looked up from the ingest log's memoryId mapping.
"""
from __future__ import annotations

import argparse
import json
import socket
import sys
import time
from pathlib import Path


KNOWN_ANSWERS = [
    {"query": "wherefore art thou Romeo",
     "expected": "romeo-and-juliet",
     "description": "Juliet's balcony speech"},
    {"query": "To be or not to be that is the question",
     "expected": "hamlet",
     "description": "Hamlet's Act III soliloquy"},
    {"query": "Shall I compare thee to a summer's day",
     "expected": "sonnets",
     "description": "Sonnet 18 opening"},
    {"query": "Double double toil and trouble fire burn and cauldron bubble",
     "expected": "macbeth",
     "description": "Macbeth Act IV witches' chant"},
    {"query": "All the world's a stage and all the men and women merely players",
     "expected": "as-you-like-it",
     "description": "Jaques' seven ages speech"},
    {"query": "Friends Romans countrymen lend me your ears",
     "expected": "julius-caesar",
     "description": "Mark Antony's funeral oration"},
    {"query": "A horse A horse My kingdom for a horse",
     "expected": "richard-the-third",
     "description": "Richard III's last cry"},
    {"query": "If music be the food of love play on Give me excess of it",
     "expected": "twelfth-night",
     "description": "Orsino's opening"},
    {"query": "The quality of mercy is not strained It droppeth as the gentle rain",
     "expected": "merchant-of-venice",
     "description": "Portia's courtroom speech"},
    {"query": "Now is the winter of our discontent Made glorious summer",
     "expected": "richard-the-third",
     "description": "Richard's opening soliloquy"},
]

STRATEGIES = ["search_by_tokens", "search_graph", "search_by_keywords",
              "search_robust"]


def rpc(host: str, port: int, calls: list[dict], timeout: float = 180.0) -> dict:
    s = socket.socket(); s.settimeout(timeout); s.connect((host, port))
    s.sendall((json.dumps({"jsonrpc": "2.0", "id": 0, "method": "initialize",
        "params": {"protocolVersion": "2024-11-05", "capabilities": {},
                   "clientInfo": {"name": "q", "version": "1"}}}) + "\n").encode())
    buf = b""
    while b"\n" not in buf:
        buf += s.recv(65536)
    s.sendall((json.dumps({"jsonrpc": "2.0",
        "method": "notifications/initialized", "params": {}}) + "\n").encode())
    for c in calls:
        s.sendall((json.dumps(c) + "\n").encode())
    replies: dict = {}
    buf = b""
    dl = time.time() + timeout
    wanted = {c["id"] for c in calls}
    while time.time() < dl and wanted - replies.keys():
        chunk = s.recv(65536)
        if not chunk:
            break
        buf += chunk
        while b"\n" in buf:
            line, _, rest = buf.partition(b"\n")
            buf = rest
            if not line.strip():
                continue
            obj = json.loads(line)
            if obj.get("id") in wanted:
                replies[obj["id"]] = obj
    s.close()
    return replies


def session_for_hit(h: dict, mid_map: dict[str, str]) -> str | None:
    seq = h.get("sequenceInfo") or h.get("sequence_info")
    if isinstance(seq, dict) and seq.get("sessionId"):
        return seq["sessionId"]
    mid = h.get("memoryId") or h.get("memory_id") or h.get("fingerprintId")
    if mid and mid in mid_map:
        return mid_map[mid]
    return None


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--host", default="127.0.0.1")
    ap.add_argument("--port", type=int, default=3100)
    ap.add_argument("--report", default="/tmp/shakespeare_query_report.json")
    ap.add_argument("--ingest-log", default="/tmp/shakespeare_ingest.jsonl")
    ap.add_argument("--topk", type=int, default=10)
    args = ap.parse_args()

    mid_map: dict[str, str] = {}
    for line in Path(args.ingest_log).read_text().splitlines():
        if line.strip():
            r = json.loads(line)
            if r.get("fingerprint_id"):
                mid_map[r["fingerprint_id"]] = r["session"]
    print(f"Loaded {len(mid_map)} memoryId→session mappings")

    # Issue one call per (query, strategy)
    calls = []
    call_key: dict[int, tuple[int, str]] = {}
    rid = 100
    for qi, kq in enumerate(KNOWN_ANSWERS):
        for strat in STRATEGIES:
            rid += 1
            args_dict = {"query": kq["query"], "limit": args.topk}
            calls.append({"jsonrpc": "2.0", "id": rid, "method": "tools/call",
                          "params": {"name": strat, "arguments": args_dict}})
            call_key[rid] = (qi, strat)

    print(f"Issuing {len(calls)} RPC calls "
          f"({len(KNOWN_ANSWERS)} queries × {len(STRATEGIES)} strategies)")
    replies = rpc(args.host, args.port, calls, timeout=300.0)

    per_query: dict[int, dict] = {}
    for rid, (qi, strat) in call_key.items():
        r = replies.get(rid)
        kq = KNOWN_ANSWERS[qi]
        if rid == min(k for k, v in call_key.items() if v[0] == qi):
            per_query[qi] = {
                "query": kq["query"], "expected": kq["expected"],
                "description": kq["description"],
                "strategies": {},
            }
        if not r or r.get("result", {}).get("isError"):
            per_query[qi]["strategies"][strat] = {"error": (
                r["result"]["content"][0]["text"]
                if r and r.get("result", {}).get("isError") else "no reply")}
            continue
        body = json.loads(r["result"]["content"][0]["text"])
        hits = body.get("results", [])
        matched_rank = -1
        for rank, h in enumerate(hits[:args.topk]):
            sess = session_for_hit(h, mid_map)
            if sess and kq["expected"] in sess:
                matched_rank = rank
                break
        per_query[qi]["strategies"][strat] = {
            "matched_rank": matched_rank,
            "top_sessions": [session_for_hit(h, mid_map) for h in hits[:5]],
            "count": len(hits),
        }

    # Summarize
    passed = 0
    passed_top1 = 0
    failed = 0
    print("\n" + "=" * 88)
    print(f"  SHAKESPEARE KNOWN-ANSWER QUERY PROOFS ({len(KNOWN_ANSWERS)} queries, "
          f"top-{args.topk}, {len(STRATEGIES)} retrieval strategies)")
    print("=" * 88)

    for qi in sorted(per_query.keys()):
        q = per_query[qi]
        best_rank = -1
        best_strat = None
        for strat, res in q["strategies"].items():
            r = res.get("matched_rank", -2)
            if r is not None and r >= 0 and (best_rank < 0 or r < best_rank):
                best_rank = r
                best_strat = strat
        q["best"] = {"rank": best_rank, "strategy": best_strat}
        if best_rank == 0:
            passed += 1
            passed_top1 += 1
            emoji = "✓"
        elif best_rank > 0:
            passed += 1
            emoji = "~"
        else:
            failed += 1
            emoji = "✗"
        print(f"  {emoji} {q['description']:<38} "
              f"best={best_strat or '-':>20} rank={best_rank+1 if best_rank>=0 else '—':>3}")

    report = {"queries": list(per_query.values()),
              "summary": {"passed": passed, "passed_top1": passed_top1,
                          "failed": failed, "total": len(KNOWN_ANSWERS)}}
    Path(args.report).write_text(json.dumps(report, indent=2))

    print()
    print(f"  Top-1 hits: {passed_top1}/{len(KNOWN_ANSWERS)}")
    print(f"  Top-{args.topk} hits: {passed}/{len(KNOWN_ANSWERS)}")
    print(f"  Report: {args.report}")
    return 0 if failed == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
