#!/usr/bin/env python3
"""Build a CHUNK-GRANULARITY panel->oracle dataset from the forward cache.

Motivation (INV-032 / #756): code-state pooling gives only n=2400 samples, below
the measurement power floor (n>=6000 proven needed). Chunk granularity yields
~10^2-10^3x more rows AND, restricted to diff-touched chunks, removes the
mean-pool dilution (unchanged chunks are identical across pass/fail -> pure noise).

Panel per chunk = native concat of the 9 active embedder vectors (slot identity
preserved; NO cross-slot mixing). Label = the chunk's code-state oracle verdict.

diff-touched detection: a candidate-state chunk is diff-touched iff its content
hash (key.chunk_id = sha256 of source text) is NOT in the base-state chunk_id set
for that task. Unchanged chunks hash identically in base and candidate, so this is
robust to line-number shifts.

Two passes:
  A) read e12 (smallest) across all shards -> base_ids[task] = {chunk_id @ base}
  B) read all 9 embedders per shard, group by chunk, emit candidate diff-touched
     panels with all 9 slots present and an oracle label.

SoT: <out>/chunk_panel_oracle_dataset.npz (X, y, diff_touched) + manifest + rows.
"""
from __future__ import annotations

import argparse
import importlib.util
import json
import sys
import time
from pathlib import Path


def load_probe(p):
    spec = importlib.util.spec_from_file_location("probe", str(p))
    m = importlib.util.module_from_spec(spec)
    sys.modules["probe"] = m
    spec.loader.exec_module(m)
    return m


ACTIVE = ("e1", "e10", "e12", "e13", "e14", "e6", "e7", "e8", "e9")


def shard_record_path(shard_root: Path, embedder: str, probe) -> Path:
    pm = probe.read_json(shard_root / "python_forward_cache_packed_manifest.json")
    info = pm["per_embedder"][embedder]
    return Path(str(info["record_path"]))


def candidate_state(category: str) -> str:
    return "test_patch" if category == "known_good" else f"mutation_{category}"


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--probe", required=True)
    ap.add_argument("--forward-root", required=True)
    ap.add_argument("--oracle", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--max-shards", type=int, default=None)
    ap.add_argument("--verify-sha", action="store_true")
    args = ap.parse_args()

    import numpy as np

    probe = load_probe(Path(args.probe))
    oracle_labels, oracle_meta = probe.load_oracle_index(Path(args.oracle))
    shards, fc_meta = probe.discover_forward_cache(Path(args.forward_root), args.max_shards)
    shard_roots = [Path(str(s["cache_root"])) for s in shards]
    print(f"[build] shards={len(shard_roots)} oracle_entries={oracle_meta['entry_count']}", flush=True)

    # ---- PASS A: base chunk_ids per task (read e12 only) ----
    t0 = time.time()
    base_ids: dict[str, set] = {}
    a_rows = 0
    for si, sr in enumerate(shard_roots):
        rp = shard_record_path(sr, "e12", probe)
        with rp.open() as fh:
            for line in fh:
                if not line.strip():
                    continue
                r = json.loads(line)
                ch = r.get("chunk") or {}
                if str(ch.get("workspace_state") or "") != "base":
                    continue
                task = str(ch.get("task_instance_id") or "")
                # diff-touched must compare CONTENT, not chunk_id: chunk_id embeds
                # absolute_path (.../base/... vs .../mutation_x/...) so it never
                # matches across states. source_text_sha256 is path-independent.
                stx = str((r.get("key") or {}).get("source_text_sha256") or "")
                if task and stx:
                    base_ids.setdefault(task, set()).add(stx)
                    a_rows += 1
    print(f"[build] PASS A done: tasks_with_base={len(base_ids)} base_chunk_rows={a_rows} ({time.time()-t0:.1f}s)", flush=True)

    # slot dims from packed manifest (native vector_len)
    pm0 = probe.read_json(shard_roots[0] / "python_forward_cache_packed_manifest.json")
    slot_dim = {e: int(pm0["per_embedder"][e].get("vector_len") or 0) for e in ACTIVE}
    # fall back: read one record's vector len if vector_len absent
    for e in ACTIVE:
        if slot_dim[e] <= 0:
            with shard_record_path(shard_roots[0], e, probe).open() as fh:
                r = json.loads(fh.readline())
                slot_dim[e] = len((r.get("output") or {}).get("vector") or [])
    offsets = {}
    off = 0
    for e in ACTIVE:
        offsets[e] = (off, off + slot_dim[e])
        off += slot_dim[e]
    panel_dim = off
    feature_slices = {e: {"start": offsets[e][0], "stop": offsets[e][1], "source_dim": slot_dim[e]} for e in ACTIVE}
    print(f"[build] panel_dim={panel_dim} slot_dim={slot_dim}", flush=True)

    # ---- PASS B: assemble candidate diff-touched panels ----
    t1 = time.time()
    X_rows = []
    y_rows = []
    dt_rows = []
    meta_rows = []
    emitted = 0
    skipped_incomplete = 0
    skipped_not_candidate = 0
    skipped_not_difftouched = 0
    skipped_no_oracle = 0
    for si, sr in enumerate(shard_roots):
        # chunk_key -> {embedder: np.array}, plus chunk meta
        bucket: dict[tuple, dict] = {}
        cmeta: dict[tuple, dict] = {}
        for e in ACTIVE:
            rp = shard_record_path(sr, e, probe)
            with rp.open() as fh:
                for line in fh:
                    if not line.strip():
                        continue
                    r = json.loads(line)
                    ch = r.get("chunk") or {}
                    ws = str(ch.get("workspace_state") or "")
                    cat = str(ch.get("mutation_category") or "")
                    if ws != candidate_state(cat):
                        continue  # candidate state only
                    task = str(ch.get("task_instance_id") or "")
                    key = r.get("key") or {}
                    cid = str(key.get("chunk_id") or "")  # per-(state,chunk) group key
                    stx = str(key.get("source_text_sha256") or "")  # content identity
                    rel = str(ch.get("relative_path") or "")
                    ln = ch.get("chunk_line_number")
                    ck = (task, cat, ws, rel, ln, cid)
                    vec = (r.get("output") or {}).get("vector")
                    if not isinstance(vec, list) or len(vec) != slot_dim[e]:
                        continue
                    if args.verify_sha:
                        exp = str((r.get("output") or {}).get("vector_sha256") or r.get("vector_sha256") or "")
                        if exp and exp != probe.vector_sha256(vec):
                            raise SystemExit(f"VECTOR_SHA_MISMATCH {rp} {ck}")
                    bucket.setdefault(ck, {})[e] = np.asarray(vec, dtype=np.float32)
                    if ck not in cmeta:
                        cmeta[ck] = {"task": task, "category": cat, "chunk_id": cid, "source_text_sha256": stx, "rel": rel}
        # emit completed chunks
        for ck, slots in bucket.items():
            m = cmeta[ck]
            task, cat, cid, stx = m["task"], m["category"], m["chunk_id"], m["source_text_sha256"]
            if len(slots) != len(ACTIVE):
                skipped_incomplete += 1
                continue
            okey = (task, cat)
            if okey not in oracle_labels:
                skipped_no_oracle += 1
                continue
            # diff-touched: candidate chunk content not present in base state
            is_dt = bool(stx) and stx not in base_ids.get(task, set())
            panel = np.concatenate([slots[e] for e in ACTIVE]).astype(np.float32)
            if panel.shape[0] != panel_dim:
                skipped_incomplete += 1
                continue
            X_rows.append(panel)
            y_rows.append(1 if oracle_labels[okey] else 0)
            dt_rows.append(1 if is_dt else 0)
            if not is_dt:
                skipped_not_difftouched += 1
            emitted += 1
            if len(meta_rows) < 32:
                meta_rows.append({"task": task, "category": cat, "oracle_pass": bool(oracle_labels[okey]), "diff_touched": bool(is_dt), "chunk_id": cid, "rel": m["rel"]})
        if (si + 1) % 10 == 0:
            print(f"[build] PASS B shard {si+1}/{len(shard_roots)} emitted={emitted} ({time.time()-t1:.1f}s)", flush=True)

    X = np.asarray(X_rows, dtype=np.float32)
    y = np.asarray(y_rows, dtype=np.int64)
    dt = np.asarray(dt_rows, dtype=np.int64)
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    np.savez(out / "chunk_panel_oracle_dataset.npz", X=X, y=y, diff_touched=dt)
    manifest = {
        "schema": "chunk_panel_oracle_dataset_v1",
        "forward_root": args.forward_root,
        "oracle_path": args.oracle,
        "oracle_sha256": oracle_meta["sha256"],
        "active_embedders": list(ACTIVE),
        "slot_dim": slot_dim,
        "feature_slices": feature_slices,
        "panel_dim": panel_dim,
        "row_count": int(X.shape[0]),
        "diff_touched_count": int(dt.sum()),
        "all_chunk_count": int(X.shape[0]),
        "label_counts": {"pass": int(y.sum()), "fail": int((y == 0).sum())},
        "diff_touched_label_counts": {"pass": int(y[dt == 1].sum()), "fail": int((y[dt == 1] == 0).sum())},
        "skipped": {
            "incomplete_panels": skipped_incomplete,
            "no_oracle": skipped_no_oracle,
            "not_difftouched_kept": skipped_not_difftouched,
        },
        "sample_rows": meta_rows,
    }
    (out / "chunk_panel_oracle_manifest.json").write_text(json.dumps(manifest, indent=2), encoding="utf-8")
    print(f"[build] WROTE {out}/chunk_panel_oracle_dataset.npz X={X.shape} difftouched={int(dt.sum())}/{int(X.shape[0])}", flush=True)
    print(json.dumps(manifest["label_counts"]), json.dumps(manifest["diff_touched_label_counts"]), flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
