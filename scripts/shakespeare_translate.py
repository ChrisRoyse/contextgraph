#!/usr/bin/env python3
"""Demo 4 — Style Transfer (modern↔Shakespeare).

Bidirectional translation:
    --to shakespeare    : modern English → Shakespearean voice
    --to modern         : Shakespearean text → plain modern English

Shakespeare-direction outputs are constellation-guarded — Gemma must produce
on-manifold text or it is rejected and regenerated.

Modern-direction outputs are not guarded (modern English by definition lives
outside the Shakespeare manifold).
"""
from __future__ import annotations

import argparse
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from _gemma_shakespeare_lib import (
    Rpc, GemmaClient, score_text, authenticity_verdict,
    list_shakespeare_constellations, soft_delete,
)


TO_SHAKESPEARE_SYSTEM = (
    "You are a translator that converts modern English into the voice of "
    "William Shakespeare. Use Early Modern English (thee, thou, doth, hath, "
    "wherefore, prithee). Preserve the meaning of the input but render it "
    "with Shakespearean cadence — iambic where natural, alliterative, "
    "metaphor-rich. Output ONLY the translation; no commentary, no preamble."
)

TO_MODERN_SYSTEM = (
    "You are a translator that converts Shakespearean English into clear, "
    "natural modern English a 21st-century reader would understand. "
    "Preserve the meaning and the speaker's intent, but use modern grammar "
    "and contemporary vocabulary. Output ONLY the translation."
)


def translate_to_shakespeare(rpc: Rpc, gemma: GemmaClient, text: str,
                             max_attempts: int, max_tokens: int) -> dict:
    cons = list_shakespeare_constellations(rpc)
    attempts = []
    for attempt in range(1, max_attempts + 1):
        out = gemma.generate(
            prompt=f"Translate to Shakespearean English:\n{text}",
            system=TO_SHAKESPEARE_SYSTEM,
            temperature=0.8 if attempt == 1 else 0.65,  # cool down on retries
            max_tokens=max_tokens,
            stop=["\n\n\n"],
        )
        out_text = (out.text or "").strip()
        if not out_text:
            attempts.append({"attempt": attempt, "text": "", "verdict": "EMPTY"})
            continue
        fp, scores = score_text(rpc, out_text, cons_cache=cons)
        v_short, v_long, m = authenticity_verdict(scores)
        soft_delete(rpc, fp)
        attempts.append({
            "attempt": attempt, "text": out_text,
            "verdict": v_short, "top_score": m["top_score"],
            "top_work": m["top_work"], "median": m["median"],
            "gen_ms": out.total_duration_ms,
        })
        if v_short in {"AUTHENTIC", "SHAKESPEAREAN"}:
            return {"text": out_text, "accepted": True,
                    "attempts": attempts, "final": attempts[-1]}
    return {"text": attempts[-1]["text"] if attempts else "",
            "accepted": False, "attempts": attempts,
            "final": attempts[-1] if attempts else None}


def translate_to_modern(gemma: GemmaClient, text: str, max_tokens: int) -> dict:
    out = gemma.generate(
        prompt=f"Translate to clear modern English:\n{text}",
        system=TO_MODERN_SYSTEM, temperature=0.4, max_tokens=max_tokens,
        stop=["\n\n\n"])
    return {"text": (out.text or "").strip(),
            "gen_ms": out.total_duration_ms,
            "tokens": out.eval_count}


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--text", required=True)
    ap.add_argument("--to", choices=["shakespeare", "modern"], required=True)
    ap.add_argument("--max-attempts", type=int, default=3,
                    help="Max regen attempts in shakespeare direction")
    ap.add_argument("--max-tokens", type=int, default=300)
    args = ap.parse_args()

    rpc = Rpc()
    gemma = GemmaClient()

    print(f"\n═══════════════════════════════════════════════════════════════════════")
    print(f"  STYLE TRANSFER  ({'modern → Shakespeare' if args.to == 'shakespeare' else 'Shakespeare → modern'})")
    print(f"═══════════════════════════════════════════════════════════════════════")
    print(f"  INPUT:")
    for line in args.text.splitlines():
        print(f"    {line}")

    if args.to == "shakespeare":
        result = translate_to_shakespeare(rpc, gemma, args.text,
                                          args.max_attempts, args.max_tokens)
        print(f"\n  Took {len(result['attempts'])} attempt(s); "
              f"accepted={result['accepted']}")
        if result["final"]:
            f = result["final"]
            print(f"  Verdict: {f['verdict']}  top={f.get('top_work', '?')} "
                  f"({f.get('top_score', 0):.4f})")
        print()
        print("  OUTPUT:")
        for line in (result["text"] or "(empty)").splitlines():
            print(f"    {line}")
    else:
        result = translate_to_modern(gemma, args.text, args.max_tokens)
        print(f"\n  Generated in {result['gen_ms']:.0f}ms ({result['tokens']} tokens)\n")
        print("  OUTPUT:")
        for line in (result["text"] or "(empty)").splitlines():
            print(f"    {line}")

    rpc.close()
    print(f"\n═══════════════════════════════════════════════════════════════════════\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
