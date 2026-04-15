#!/usr/bin/env python3
"""Post-ingest extraction cascade: constellations per play + contrastive mining
+ training-corpus export metrics. Prints the demo-facing numbers.

Requires: shakespeare_ingest.py has already populated the daemon's RocksDB.
"""
from __future__ import annotations

import argparse
import json
import socket
import sys
import time
from collections import defaultdict
from pathlib import Path


def connect(host: str, port: int, timeout: float = 180.0) -> socket.socket:
    s = socket.socket()
    s.settimeout(timeout)
    s.connect((host, port))
    return s


def send(s: socket.socket, obj: dict) -> None:
    s.sendall((json.dumps(obj) + "\n").encode())


def reply(s: socket.socket, rid: int, timeout: float = 120.0) -> dict:
    buf = b""
    dl = time.time() + timeout
    while time.time() < dl:
        chunk = s.recv(65536)
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
    raise TimeoutError(f"no reply id={rid}")


def call(s: socket.socket, rid: int, name: str, args: dict,
         timeout: float = 120.0) -> dict:
    send(s, {"jsonrpc": "2.0", "id": rid, "method": "tools/call",
             "params": {"name": name, "arguments": args}})
    r = reply(s, rid, timeout)
    if r.get("result", {}).get("isError"):
        raise RuntimeError(f"{name}: {r['result']['content'][0]['text']}")
    return json.loads(r["result"]["content"][0]["text"])


def handshake(s: socket.socket) -> None:
    send(s, {"jsonrpc": "2.0", "id": 1, "method": "initialize",
             "params": {"protocolVersion": "2024-11-05",
                        "capabilities": {},
                        "clientInfo": {"name": "shakespeare-extract",
                                       "version": "1"}}})
    reply(s, 1)
    send(s, {"jsonrpc": "2.0", "method": "notifications/initialized",
             "params": {}})


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--ingest-log", default="/tmp/shakespeare_ingest.jsonl")
    ap.add_argument("--host", default="127.0.0.1")
    ap.add_argument("--port", type=int, default=3100)
    ap.add_argument("--constellation-min-members", type=int, default=3)
    ap.add_argument("--mine-max-pairs", type=int, default=5000)
    ap.add_argument("--mine-topk", type=int, default=10)
    ap.add_argument("--report", default="/tmp/shakespeare_extract_report.json")
    args = ap.parse_args()

    # Aggregate sessions from the ingest log
    sessions: dict[str, list[str]] = defaultdict(list)
    types_per_session: dict[str, str] = {}
    for line in Path(args.ingest_log).read_text().splitlines():
        if not line.strip():
            continue
        rec = json.loads(line)
        if "fingerprint_id" not in rec:
            continue
        sessions[rec["session"]].append(rec["fingerprint_id"])
        types_per_session[rec["session"]] = rec.get("chunk_type", "scene")
    print(f"Distinct sessions in ingest log: {len(sessions)}")

    s = connect(args.host, args.port, timeout=180.0)
    handshake(s)

    report: dict = {
        "sessions": {},
        "constellations": [],
        "contrastive": {},
        "training_corpus": {},
        "timings": {},
    }

    # ---- 1. Per-session constellation compilation ----
    rid = 100
    t0 = time.time()
    print("\n=== Compiling constellations per session ===")
    compiled = 0
    too_few = 0
    failed = 0
    for session, ids in sorted(sessions.items(), key=lambda kv: -len(kv[1])):
        if len(ids) < args.constellation_min_members:
            too_few += 1
            continue
        rid += 1
        # slug-like label. Strip 'shakespeare-' prefix for readability.
        label = session.replace("shakespeare-", "").replace("-", " ").title()
        try:
            body = call(s, rid, "compile_constellation", {
                "selector": "session",
                "sessionId": session,
                "label": f"Shakespeare — {label}",
                "rebuildIfExists": True,
            }, timeout=180.0)
            report["constellations"].append({
                "session": session,
                "label": label,
                "constellation_id": body.get("constellation_id"),
                "member_count": body.get("member_count"),
                "coherence": body.get("coherence"),
                "per_embedder_coverage": body.get("per_embedder_coverage"),
            })
            compiled += 1
            if compiled % 5 == 0:
                print(f"  compiled {compiled} constellations "
                      f"(elapsed {time.time() - t0:.1f}s)")
        except Exception as e:
            failed += 1
            print(f"  FAIL session={session}: {e}")
    report["timings"]["compile_constellations_sec"] = time.time() - t0
    report["sessions"]["total"] = len(sessions)
    report["sessions"]["too_few_members"] = too_few
    report["sessions"]["compiled"] = compiled
    report["sessions"]["failed"] = failed
    print(f"Compiled {compiled} / skipped {too_few} too-few / failed {failed}")

    # ---- 2. Contrastive pair mining across the full corpus ----
    print("\n=== Mining contrastive pairs across ALL anchors ===")
    rid += 1
    t1 = time.time()
    mine = call(s, rid, "mine_contrastive_pairs", {
        "maxPairs": args.mine_max_pairs,
        "topKCandidatesPerAnchor": args.mine_topk,
        "minDisagreement": 0.25,
    }, timeout=1200.0)
    report["contrastive"]["mine_summary"] = mine.get("summary", {})
    report["contrastive"]["config"] = mine.get("config", {})
    report["timings"]["mine_contrastive_sec"] = time.time() - t1
    pairs_stored = mine.get("summary", {}).get("pairs_stored", 0)
    print(f"  pairs_stored={pairs_stored} "
          f"elapsed={time.time() - t1:.1f}s")

    # ---- 3. Count by kind (all 6 anomaly kinds) ----
    kinds = ["semantic_but_not_causal", "keyword_but_not_paraphrase",
             "code_shape_but_different_intent",
             "entity_shared_but_different_structure",
             "hdc_robust_but_semantic_different", "other"]
    report["contrastive"]["counts_by_kind"] = {}
    for kind in kinds:
        rid += 1
        try:
            body = call(s, rid, "count_contrastive_pairs", {"kind": kind})
            report["contrastive"]["counts_by_kind"][kind] = body.get("count", 0)
        except Exception as e:
            report["contrastive"]["counts_by_kind"][kind] = f"error: {e}"
    rid += 1
    total = call(s, rid, "count_contrastive_pairs", {})
    report["contrastive"]["counts_by_kind"]["TOTAL"] = total.get("count", 0)

    # ---- 4. Export training corpus (all Shakespeare memories) ----
    # Note: filter by "all" would include non-Shakespeare memories too.
    # Instead iterate each session and aggregate. The training corpus is
    # session-agnostic though — CF_TRAINING_RECORDS holds ALL exports. So
    # clear it once, then export per-session.
    print("\n=== Exporting training corpus (scene-level + sonnet + poem) ===")
    # Clear first
    rid += 1
    body = call(s, rid, "export_training_corpus", {
        "filter": "session", "filterId": "__never_matches__",
        "clearExisting": True,
    }, timeout=60.0)
    # Now run PER SESSION with include-all toggles + Tucker
    exported_total = 0
    t2 = time.time()
    for session in sorted(sessions):
        rid += 1
        try:
            body = call(s, rid, "export_training_corpus", {
                "filter": "session", "filterId": session,
                "clearExisting": False,
                "includeEmbeddings": True,
                "includeSparseVectors": True,
                "includeTokenEmbeddings": False,  # keeps size sane
                "includeTemporalLabels": True,
                "includeCausalLabels": True,
                "includeEdges": True,
                "includeTuckerCore": True,
            }, timeout=300.0)
            exported_total += body.get("exported", 0)
        except Exception as e:
            print(f"  FAIL export session={session}: {e}")
    report["training_corpus"]["records_exported"] = exported_total
    report["timings"]["export_corpus_sec"] = time.time() - t2

    # Final training-record count
    rid += 1
    c = call(s, rid, "count_training_records", {})
    report["training_corpus"]["count_training_records_final"] = c.get("count")

    s.close()

    # Write machine-readable report
    Path(args.report).write_text(json.dumps(report, indent=2))
    print(f"\nReport: {args.report}")

    # Headline numbers for the demo
    N = sum(len(ids) for ids in sessions.values())
    print("\n" + "=" * 70)
    print("  TELEOX DEMO — DERIVED DATA ABUNDANCE ON SHAKESPEARE")
    print("=" * 70)
    print(f"  Input:            1 file, 5.4 MB (Project Gutenberg)")
    print(f"  Memories ingested: {N}")
    print(f"  Works covered:    {len(sessions)}")
    print(f"  Constellations compiled: {compiled}")
    print(f"  Contrastive pairs:       {pairs_stored}")
    print(f"  Training records: {report['training_corpus']['count_training_records_final']}")
    print()
    print(f"  Signals per memory = 13 embedders + 78 cross-correlations = 91")
    print(f"  Total labeled training signals = {N} × 91 = {N * 91:,}")
    print()
    print(f"  Plus per-memory: Tucker core (rank 4×4×128 = 2,048 elements)")
    print(f"  Plus per-memory: 6 group-alignments + 13 per-embedder stats")
    print(f"  Plus per-work  : constellation centroid + spread stats")
    print(f"  Plus corpus    : {pairs_stored} contrastive anomaly pairs")
    print(f"    = derived features on the order of ~{N * (91 + 2048 + 19) + pairs_stored:,}")
    print()
    print(f"  Ingestion time:   see /tmp/shakespeare_ingest.log")
    print(f"  Extraction time:  {sum(v for k, v in report['timings'].items()):.1f}s")
    print("=" * 70)

    return 0


if __name__ == "__main__":
    sys.exit(main())
