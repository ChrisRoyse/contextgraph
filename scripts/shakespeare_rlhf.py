#!/usr/bin/env python3
"""Demo 6 — Contrastive RLHF Export.

Pulls every (anchor, negative) pair from CF_CONTRASTIVE_PAIRS and emits a
DPO/ORPO preference-tuning JSONL where:
    - 'prompt'   = anchor's text (or a synthesized question about it)
    - 'chosen'   = the anchor text itself (the "good" Shakespeare)
    - 'rejected' = the negative text (the cross-embedder anomaly partner)

Each row also carries similarity_profile + anomaly_kind metadata so the
training run can filter / weight by anomaly type.

This is the **ML-research-lab pitch**: 13,465 contrastive examples that no
human labeler would produce, ready to drop into TRL / OpenRLHF / Axolotl.
"""
from __future__ import annotations

import argparse
import json
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from _gemma_shakespeare_lib import Rpc


PROMPT_TEMPLATE = (
    "Continue the following passage in the same voice and style. "
    "Maintain Shakespearean cadence and Early Modern English vocabulary.\n\n"
    "PASSAGE:\n{anchor}\n\nCONTINUATION:"
)


def export(rpc: Rpc, out_path: Path, max_pairs: int,
           page_size: int = 200) -> dict:
    written = 0
    skipped_no_text = 0
    seen_pair_keys = set()
    by_kind: dict[str, int] = {}
    pages = 0
    t0 = time.time()

    with out_path.open("w") as f:
        offset = 0
        while written < max_pairs:
            page = rpc.call("list_contrastive_pairs", {
                "limit": min(page_size, max_pairs - written),
                "offset": offset,
                "includeFull": False,  # keep it light, fetch full per pair
            }, timeout=60.0)
            pairs = page.get("pairs") or []
            if not pairs:
                break
            pages += 1

            for p in pairs:
                offset += 1
                if written >= max_pairs:
                    break
                key = (p["anchor_id"], p["negative_id"])
                if key in seen_pair_keys:
                    continue
                seen_pair_keys.add(key)

                # Fetch the full pair to get texts + similarity profile
                try:
                    full = rpc.call("get_contrastive_pair", {
                        "anchorId": p["anchor_id"],
                        "negativeId": p["negative_id"],
                    }, timeout=30.0)
                except RuntimeError as e:
                    print(f"  warn: get_contrastive_pair failed: {e}",
                          file=sys.stderr)
                    continue
                anchor_text = (full.get("anchor_text") or "").strip()
                neg_text = (full.get("negative_text") or "").strip()
                if not anchor_text or not neg_text:
                    skipped_no_text += 1
                    continue
                if anchor_text == neg_text:
                    # Same content stored twice — skip
                    continue

                kind = full.get("anomaly_kind") or "other"
                by_kind[kind] = by_kind.get(kind, 0) + 1

                row = {
                    "prompt": PROMPT_TEMPLATE.format(anchor=anchor_text[:1500]),
                    "chosen": anchor_text[:2000],
                    "rejected": neg_text[:2000],
                    "metadata": {
                        "anchor_id": p["anchor_id"],
                        "negative_id": p["negative_id"],
                        "anomaly_kind": kind,
                        "anomaly_kind_byte": full.get("anomaly_kind_byte"),
                        "disagreement_magnitude": full.get("disagreement_magnitude"),
                        "high_embedders": full.get("high_embedders"),
                        "low_embedders": full.get("low_embedders"),
                        "similarity_profile": full.get("similarity_profile"),
                        "generator": full.get("generator"),
                        "mined_at": full.get("mined_at"),
                    },
                }
                f.write(json.dumps(row, separators=(",", ":")) + "\n")
                written += 1
                if written % 100 == 0:
                    elapsed = time.time() - t0
                    print(f"  wrote {written:5d} pairs  "
                          f"({elapsed:.1f}s, {written / elapsed:.1f}/s)")

            if len(pairs) < page_size:
                break

    elapsed = time.time() - t0
    return {
        "written": written,
        "pages_scanned": pages,
        "skipped_no_text": skipped_no_text,
        "by_kind": by_kind,
        "elapsed_sec": elapsed,
        "out_path": str(out_path),
        "out_size_bytes": out_path.stat().st_size,
    }


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default="/tmp/shakespeare_rlhf.jsonl")
    ap.add_argument("--max-pairs", type=int, default=15000)
    ap.add_argument("--page-size", type=int, default=200)
    args = ap.parse_args()

    rpc = Rpc()
    out_path = Path(args.out)
    out_path.parent.mkdir(parents=True, exist_ok=True)

    print(f"\n═══════════════════════════════════════════════════════════════════════")
    print(f"  CONTRASTIVE RLHF EXPORT  (DPO/ORPO-ready preference data)")
    print(f"═══════════════════════════════════════════════════════════════════════")
    # Pre-state: total pairs in CF
    total_body = rpc.call("count_contrastive_pairs", {})
    total = total_body["count"]
    print(f"  CF_CONTRASTIVE_PAIRS total: {total:,}\n")

    print(f"→ Exporting up to {args.max_pairs:,} pairs to {args.out}…")
    stats = export(rpc, out_path, args.max_pairs, args.page_size)
    rpc.close()

    print(f"\n═══════════════════════════════════════════════════════════════════════")
    print(f"  ✅ wrote {stats['written']:,} preference rows in "
          f"{stats['elapsed_sec']:.1f}s")
    print(f"     output: {stats['out_path']}  "
          f"({stats['out_size_bytes']:,} bytes, "
          f"{stats['out_size_bytes']/(1024*1024):.1f} MB)")
    print(f"     skipped (missing text): {stats['skipped_no_text']}")
    print(f"     by anomaly_kind:")
    for k, v in sorted(stats["by_kind"].items(), key=lambda kv: -kv[1]):
        print(f"        {k:<45} {v:,}")
    print(f"═══════════════════════════════════════════════════════════════════════\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
