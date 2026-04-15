#!/usr/bin/env python3
"""Shakespeare Authenticity Detector — score any text against the 44
per-work constellations we built from the Complete Works.

Pipeline:
  1. Embed text via context-graph daemon (transient store_memory)
  2. Score that fingerprint against every Shakespeare constellation
  3. Render verdict + per-embedder breakdown
  4. Soft-delete the probe so the graph stays clean

Usage:
  echo "Shall I compare thee to a summer's day?" | python3 shakespeare_authenticity.py
  python3 shakespeare_authenticity.py --text "I have invented the lightbulb"
"""
from __future__ import annotations

import argparse
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from _gemma_shakespeare_lib import (
    Rpc, score_text, authenticity_verdict, soft_delete,
)


EMB_NAMES = ["E1_Sem", "E2_Recent", "E3_Period", "E4_Position",
             "E5_Cause", "E6_Sparse", "E7_Code", "E8_Graph",
             "E9_HDC", "E10_Para", "E11_Entity", "E12_ColBERT",
             "E13_SPLADE"]


def render(text: str, top_n: int) -> int:
    rpc = Rpc()
    print(f"\n┌{'─'*76}┐")
    print(f"│ INPUT: {text[:65]:<68} │")
    if len(text) > 65:
        print(f"│        {(text[65:130] + '...')[:65]:<68} │")
    print(f"└{'─'*76}┘\n")

    print("→ Embedding through 13 frozen models (semantic, causal, code, "
          "graph, sparse,\n  paraphrase, entity, ColBERT, SPLADE, HDC, "
          "temporal×3)…")
    fp, scores = score_text(rpc, text)
    if not scores:
        rpc.close()
        raise RuntimeError(
            "No Shakespeare constellations available. Run "
            "scripts/shakespeare_extract.py first.")
    print(f"  ✓ {len(scores)} constellations scored\n")

    # Top N table
    print(f"┌{'─'*76}┐")
    print(f"│ TOP {top_n} MOST-SIMILAR SHAKESPEARE WORKS"
          f"{' ' * (40)}│")
    print(f"├{'─'*76}┤")
    print(f"│ {'Rank':<4} {'Score':>6}  {'p95':<5} {'N':>4}  {'Work':<50} │")
    print(f"├{'─'*76}┤")
    for i, s in enumerate(scores[:top_n]):
        in_p95 = "✓" if s.in_spread_p95 else " "
        print(f"│ {i+1:<4} {s.combined_score:>6.4f}  [{in_p95:1}]   "
              f"{s.members:>4}  {s.work[:50]:<50} │")
    print(f"└{'─'*76}┘\n")

    # Verdict
    v_short, v_long, m = authenticity_verdict(scores)
    print(f"┌{'─'*76}┐")
    print(f"│ AUTHENTICITY VERDICT: {v_short:<53}│")
    print(f"├{'─'*76}┤")
    print(f"│ {v_long[:74]:<74} │")
    print(f"│{' '*76}│")
    print(f"│   Top score        : {m['top_score']:.4f} "
          f"({m['top_work'][:40]})"
          f"{' '*(76 - 25 - 6 - len(m['top_work'][:40]) - 2)}│")
    if m['second_work']:
        print(f"│   Second score     : {m['second_score']:.4f} "
              f"({m['second_work'][:40]})"
              f"{' '*(76 - 25 - 6 - len(m['second_work'][:40]) - 2)}│")
    print(f"│   Median (44 works): {m['median']:.4f}"
          f"{' '*(76 - 25 - 6)}│")
    print(f"│   Spread top-vs-med: {m['spread']:+.4f}"
          f"{' '*(76 - 25 - 7)}│")
    print(f"│   In 95-pct spread : {m['in_p95']}"
          f"{' '*(76 - 25 - len(str(m['in_p95'])))}│")
    print(f"└{'─'*76}┘\n")

    # Per-embedder breakdown for top match
    top = scores[0]
    if top.per_embedder:
        print(f"PER-EMBEDDER COSINES vs {top.work}:")
        for name, val in zip(EMB_NAMES, top.per_embedder):
            bar_len = int(max(0, val) * 40)
            bar = "█" * bar_len + "░" * (40 - bar_len)
            print(f"  {name:<12} {val:6.3f}  {bar}")
        print()

    # Cleanup
    soft_delete(rpc, fp)
    rpc.close()
    print(f"(probe memory {fp[:8]}… soft-deleted via node_id)")
    return 0 if v_short in {"AUTHENTIC", "SHAKESPEAREAN"} else 1


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--text", help="Text to score (else read stdin)")
    ap.add_argument("--top-n", type=int, default=5)
    args = ap.parse_args()
    text = args.text or sys.stdin.read().strip()
    if not text:
        print("usage: pass --text or pipe text via stdin", file=sys.stderr)
        return 2
    return render(text, args.top_n)


if __name__ == "__main__":
    sys.exit(main())
