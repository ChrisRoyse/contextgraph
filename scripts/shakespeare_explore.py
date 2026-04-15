#!/usr/bin/env python3
"""Deep-exploration demo over the existing ingested Shakespeare data.

Shows what kinds of "derived data abundance" analytics the 13-embedder
pipeline enables. Every section answers a concrete question a prospect
would ask.
"""
from __future__ import annotations

import argparse
import json
import math
import socket
import sys
import time
from collections import defaultdict
from pathlib import Path


# ----------------------------------------------------------------------------
# Small RPC helper kept self-contained so the explore script can be shipped
# stand-alone.
# ----------------------------------------------------------------------------
class Rpc:
    def __init__(self, host: str, port: int, timeout: float = 180.0) -> None:
        self.s = socket.socket(); self.s.settimeout(timeout)
        self.s.connect((host, port))
        self._rid = 0
        self._send({"jsonrpc": "2.0", "id": self.next_rid(),
                    "method": "initialize",
                    "params": {"protocolVersion": "2024-11-05",
                               "capabilities": {},
                               "clientInfo": {"name": "explore", "version": "1"}}})
        self._read(1)
        self._send({"jsonrpc": "2.0", "method": "notifications/initialized",
                    "params": {}})

    def next_rid(self) -> int:
        self._rid += 1
        return self._rid

    def _send(self, obj: dict) -> None:
        self.s.sendall((json.dumps(obj) + "\n").encode())

    def _read(self, rid: int, timeout: float = 120.0) -> dict:
        buf = b""
        dl = time.time() + timeout
        while time.time() < dl:
            chunk = self.s.recv(65536)
            if not chunk:
                raise RuntimeError("socket closed")
            buf += chunk
            while b"\n" in buf:
                line, _, rest = buf.partition(b"\n")
                buf = rest
                if not line.strip():
                    continue
                obj = json.loads(line)
                if obj.get("id") == rid:
                    return obj
        raise TimeoutError(f"rid={rid}")

    def call(self, name: str, args: dict, timeout: float = 120.0) -> dict:
        rid = self.next_rid()
        self._send({"jsonrpc": "2.0", "id": rid, "method": "tools/call",
                    "params": {"name": name, "arguments": args}})
        r = self._read(rid, timeout)
        if r.get("result", {}).get("isError"):
            raise RuntimeError(f"{name}: {r['result']['content'][0]['text']}")
        return json.loads(r["result"]["content"][0]["text"])

    def close(self) -> None:
        self.s.close()


def section(title: str) -> None:
    print("\n" + "═" * 78)
    print(f"  {title}")
    print("═" * 78)


# ----------------------------------------------------------------------------
# 1. Corpus-wide accounting
# ----------------------------------------------------------------------------
def show_corpus_scale(rpc: Rpc, mid_map: dict[str, str]) -> None:
    section("1. CORPUS SCALE")
    daemon = rpc.call("daemon_status", {})
    cons_list = rpc.call("list_constellations", {"limit": 100})
    pairs = rpc.call("count_contrastive_pairs", {})
    tr = rpc.call("count_training_records", {})
    by_session: dict[str, int] = defaultdict(int)
    for v in mid_map.values():
        by_session[v] += 1
    print(f"  Daemon PID {daemon['pid']} ({daemon['uptime_secs']}s up, "
          f"{daemon['models_state']})")
    print(f"  Memories (from ingest log): {len(mid_map):,}")
    print(f"  Distinct works             : {len(by_session)}")
    print(f"  Constellations             : {cons_list['total']}")
    print(f"  Contrastive pairs          : {pairs['count']:,}")
    print(f"  Training records           : {tr['count']:,}")


# ----------------------------------------------------------------------------
# 2. Per-work coherence ranking with interpretation
# ----------------------------------------------------------------------------
def show_per_work_coherence(rpc: Rpc) -> None:
    section("2. PER-WORK TOPICAL COHERENCE")
    cons_list = rpc.call("list_constellations", {"limit": 100})
    rows = []
    for c in cons_list["constellations"]:
        if c.get("coherence") is None:
            continue
        rows.append((c["label"].replace("Shakespeare — ", ""),
                     c["member_count"], c["coherence"]))
    rows.sort(key=lambda r: -r[2])
    print("  The coherence score is the median cosine of each chunk in a work")
    print("  against that work's E1 centroid. High = tight stylistic/topical")
    print("  manifold. Low = stylistic diversity within the work.")
    print()
    print(f"  {'Work':<45} {'N':>4} {'Coherence':>10}")
    print(f"  {'─'*45} {'─'*4} {'─'*10}")
    for label, n, coh in rows[:10]:
        print(f"  {label[:45]:<45} {n:>4}  {coh:>9.4f}")
    print(f"  {'…':>45}")
    for label, n, coh in rows[-5:]:
        print(f"  {label[:45]:<45} {n:>4}  {coh:>9.4f}")


# ----------------------------------------------------------------------------
# 3. Cross-work stylistic similarity via constellation centroids
# ----------------------------------------------------------------------------
def show_cross_work_similarity(rpc: Rpc, mid_map: dict[str, str]) -> None:
    section("3. CROSS-WORK SIMILARITY (E1 centroid cosines)")
    # Pull every constellation with E1 centroid
    cons_list = rpc.call("list_constellations", {"limit": 100,
                                                  "includeCentroids": True})
    centroids: list[tuple[str, list[float]]] = []
    for c in cons_list["constellations"][:30]:  # bound the matrix
        if not c.get("found", True):
            continue
        full = rpc.call("get_constellation",
                        {"constellationId": c["constellation_id"],
                         "includeCentroids": True})
        # E1 is per_embedder[0]
        pe = full.get("per_embedder", [])
        if not pe:
            continue
        e1c = pe[0].get("centroid") or []
        if not e1c:
            continue
        centroids.append((full["label"].replace("Shakespeare — ", ""), e1c))

    if not centroids:
        print("  (no centroids available)")
        return

    def cos(a: list[float], b: list[float]) -> float:
        d = sum(x * y for x, y in zip(a, b))
        na = math.sqrt(sum(x * x for x in a))
        nb = math.sqrt(sum(x * x for x in b))
        if na == 0 or nb == 0:
            return 0.0
        return d / (na * nb)

    # Find top 5 most similar pairs + top 5 most distant
    pairs = []
    for i in range(len(centroids)):
        for j in range(i + 1, len(centroids)):
            pairs.append((centroids[i][0], centroids[j][0],
                          cos(centroids[i][1], centroids[j][1])))
    pairs.sort(key=lambda p: -p[2])
    print("  Works with the MOST SIMILAR E1 centroids (same style/themes):")
    for a, b, s in pairs[:6]:
        print(f"    {s:6.4f}  {a[:35]:<35} ↔ {b[:35]}")
    print()
    print("  Works with the MOST DISTANT E1 centroids (biggest style gap):")
    for a, b, s in pairs[-6:]:
        print(f"    {s:6.4f}  {a[:35]:<35} ↔ {b[:35]}")


# ----------------------------------------------------------------------------
# 4. Top anomalous contrastive pairs (with context)
# ----------------------------------------------------------------------------
def show_top_anomalies(rpc: Rpc, mid_map: dict[str, str]) -> None:
    section("4. TOP CROSS-EMBEDDER ANOMALIES (contrastive pairs)")
    pairs = rpc.call("list_contrastive_pairs", {"limit": 200,
                                                 "includeFull": True})
    # Rank by disagreement magnitude
    items = pairs.get("pairs", [])
    items.sort(key=lambda p: -(p.get("disagreement_magnitude") or 0))
    print("  These are (anchor, negative) pairs where 9+ embedders agree")
    print("  on HIGH similarity but 1-3 embedders disagree strongly — the")
    print("  cases a human labeler would never produce.")
    print()
    for i, p in enumerate(items[:6]):
        a = p.get("anchor_id")
        n = p.get("negative_id")
        sa = mid_map.get(a, "?")
        sn = mid_map.get(n, "?")
        dis = p.get("disagreement_magnitude") or 0
        kind = p.get("anomaly_kind", "?")
        highs = p.get("high_embedders") or []
        lows = p.get("low_embedders") or []
        print(f"  #{i+1}  disagreement={dis:.3f}  kind={kind}")
        print(f"       anchor   : {sa.replace('shakespeare-', '')[:60]}")
        print(f"       negative : {sn.replace('shakespeare-', '')[:60]}")
        print(f"       high={highs}  low={lows}")


# ----------------------------------------------------------------------------
# 5. Known-phrase retrieval with actual retrieved text
# ----------------------------------------------------------------------------
def show_retrieval_examples(rpc: Rpc, mid_map: dict[str, str]) -> None:
    section("5. RAG-STYLE RETRIEVAL OVER SHAKESPEARE")
    queries = [
        ("wherefore art thou Romeo",          "search_by_tokens"),
        ("To be or not to be",                 "search_by_tokens"),
        ("if music be the food of love",       "search_by_tokens"),
        ("Et tu Brute",                        "search_by_tokens"),
        ("What's in a name",                   "search_by_tokens"),
        ("All the world's a stage",            "search_by_tokens"),
    ]
    for q, strat in queries:
        body = rpc.call(strat, {"query": q, "limit": 3})
        hits = body.get("results", [])
        print(f"  Q: \"{q}\"  ({strat})")
        for i, h in enumerate(hits[:3]):
            mid = (h.get("memoryId") or h.get("memory_id")
                   or h.get("fingerprintId"))
            sess = mid_map.get(mid, "?").replace("shakespeare-", "")
            score = h.get("score") or h.get("similarity") or 0
            print(f"     [{i+1}] {score:6.3f}  {sess[:50]}")
        print()


# ----------------------------------------------------------------------------
# 6. Sample training record shape (what ends up in the Parquet)
# ----------------------------------------------------------------------------
def show_training_record_anatomy(rpc: Rpc, mid_map: dict[str, str]) -> None:
    section("6. ANATOMY OF ONE TRAINING RECORD")
    # Pick a famous chunk: Hamlet's Act 3 scene, or whatever we have
    # Search for "To be or not to be" and take the top
    body = rpc.call("search_by_tokens",
                    {"query": "To be or not to be", "limit": 1})
    hits = body.get("results", [])
    if not hits:
        print("  (no hits)")
        return
    mid = hits[0].get("memoryId") or hits[0].get("fingerprintId")
    sess = mid_map.get(mid, "?")
    print(f"  Memory ID: {mid}")
    print(f"  Session  : {sess}")
    print()
    rec = rpc.call("get_training_record",
                   {"memoryId": mid, "includeVectors": False,
                    "includeEdges": False})
    shape = rec.get("shape", {})
    tucker = rec.get("tucker_core", {})
    tl = rec.get("temporal_labels", {})
    tp = rec.get("topic_profile", [])
    ga = rec.get("group_alignments", [])
    print(f"  e1_len={shape.get('e1_len')}  e5_cause_len={shape.get('e5_cause_len')}  "
          f"e7_len={shape.get('e7_len')}  e11_len={shape.get('e11_len')}")
    print(f"  e6_sparse_nnz={shape.get('e6_sparse_nnz')}  "
          f"e13_sparse_nnz={shape.get('e13_sparse_nnz')}  "
          f"e12_token_count={shape.get('e12_token_count')}")
    print(f"  topic_profile[13]    = {[f'{x:.3f}' for x in tp]}")
    print(f"  group_alignments[6]  = {[f'{x:.3f}' for x in ga]}")
    print(f"  cross_correlations   : {rec.get('cross_correlations_len')} values (C(13,2)=78)")
    print(f"  tucker_core          : ranks={tucker.get('ranks')} "
          f"data_len={tucker.get('data_len')} "
          f"compression_ratio={tucker.get('compression_ratio')}")
    print(f"  temporal_labels      : stored_day_of_week={tl.get('stored_day_of_week')} "
          f"bucket={tl.get('periodic_bucket')} session_sequence={tl.get('session_sequence')}")
    print()
    print(f"  This record's bincode payload would be ~{rec.get('shape',{}).get('e1_len', 0) * 4 + 500_000:,} bytes inside the Parquet.")


# ----------------------------------------------------------------------------
# Main
# ----------------------------------------------------------------------------
def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--host", default="127.0.0.1")
    ap.add_argument("--port", type=int, default=3100)
    ap.add_argument("--ingest-log", default="/tmp/shakespeare_ingest.jsonl")
    args = ap.parse_args()

    mid_map: dict[str, str] = {}
    for line in Path(args.ingest_log).read_text().splitlines():
        if line.strip():
            r = json.loads(line)
            if r.get("fingerprint_id"):
                mid_map[r["fingerprint_id"]] = r["session"]

    rpc = Rpc(args.host, args.port)
    try:
        show_corpus_scale(rpc, mid_map)
        show_per_work_coherence(rpc)
        show_cross_work_similarity(rpc, mid_map)
        show_top_anomalies(rpc, mid_map)
        show_retrieval_examples(rpc, mid_map)
        show_training_record_anatomy(rpc, mid_map)
    finally:
        rpc.close()
    return 0


if __name__ == "__main__":
    sys.exit(main())
