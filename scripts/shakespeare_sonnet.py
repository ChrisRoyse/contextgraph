#!/usr/bin/env python3
"""Demo 5 — Sonnet Completion.

Take one or more opening lines from the user. Have Gemma complete a 14-line
sonnet in ABAB CDCD EFEF GG rhyme scheme with iambic pentameter.

Verifies:
    - exactly 14 lines
    - each line ~10 syllables (8-12 tolerance — Gemma is not a perfect prosodist)
    - ABAB CDCD EFEF GG end-rhyme scheme (last word of each line)
    - constellation guard: output must score on-manifold against THE SONNETS

Up to N retry attempts on rejection.
"""
from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from _gemma_shakespeare_lib import (
    Rpc, GemmaClient, score_text, authenticity_verdict,
    list_shakespeare_constellations, soft_delete,
)

VOWEL_RE = re.compile(r"[aeiouy]+", re.IGNORECASE)


def estimate_syllables(word: str) -> int:
    """Rough heuristic: count vowel groups, subtract trailing silent 'e'."""
    word = word.lower().rstrip("'")
    if not word:
        return 0
    n = len(VOWEL_RE.findall(word))
    if word.endswith("e") and n > 1:
        n -= 1
    return max(1, n)


def syllable_count(line: str) -> int:
    return sum(estimate_syllables(w) for w in re.findall(r"[A-Za-z']+", line))


def end_rhyme(line: str) -> str:
    words = re.findall(r"[A-Za-z']+", line)
    if not words:
        return ""
    w = words[-1].lower().rstrip("'")
    # Take last 2 letters as rhyme proxy (e.g. "day" / "may" → "ay")
    return w[-3:] if len(w) >= 3 else w


def rhyme_match(a: str, b: str) -> bool:
    if not a or not b:
        return False
    return a[-2:] == b[-2:]


def verify_sonnet(text: str) -> dict:
    """Check 14-line sonnet structure. Returns metrics."""
    lines = [ln.strip() for ln in text.splitlines() if ln.strip()]
    n_lines = len(lines)
    syllables = [syllable_count(ln) for ln in lines]
    rhymes = [end_rhyme(ln) for ln in lines]

    issues: list[str] = []
    if n_lines != 14:
        issues.append(f"line_count={n_lines} (want 14)")

    bad_meter = sum(1 for s in syllables if s < 8 or s > 12)
    if bad_meter > 4:
        issues.append(f"{bad_meter} lines with bad meter (want 8-12 syllables each)")

    # Rhyme scheme ABAB CDCD EFEF GG = pairs (0,2), (1,3), (4,6), (5,7),
    # (8,10), (9,11), (12,13)
    pairs = [(0, 2), (1, 3), (4, 6), (5, 7), (8, 10), (9, 11), (12, 13)]
    rhyme_hits = 0
    rhyme_total = 0
    if n_lines >= 14:
        for i, j in pairs:
            rhyme_total += 1
            if rhyme_match(rhymes[i], rhymes[j]):
                rhyme_hits += 1
        if rhyme_hits < rhyme_total - 2:
            issues.append(f"rhyme scheme ABAB-CDCD-EFEF-GG broken "
                          f"({rhyme_hits}/{rhyme_total} pairs match)")

    return {
        "n_lines": n_lines,
        "syllables": syllables,
        "syllables_mean": sum(syllables) / max(1, len(syllables)),
        "rhymes": rhymes,
        "rhyme_match_count": rhyme_hits if n_lines >= 14 else 0,
        "rhyme_match_total": rhyme_total,
        "issues": issues,
        "structurally_valid": not issues,
    }


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--first-line", required=True,
                    help="The opening line(s) of the sonnet")
    ap.add_argument("--max-attempts", type=int, default=3)
    ap.add_argument("--max-tokens", type=int, default=400)
    args = ap.parse_args()

    rpc = Rpc()
    cons = list_shakespeare_constellations(rpc)
    gemma = GemmaClient()

    sys_prompt = (
        "You are a sonnet-writing assistant in the voice of William Shakespeare. "
        "The user has supplied the FIRST line of a sonnet. Your job is to write "
        "the remaining 13 lines so the FULL sonnet is 14 lines total in iambic "
        "pentameter (10 syllables per line, alternating unstressed/stressed). "
        "The rhyme scheme of the FULL sonnet is ABAB CDCD EFEF GG — three "
        "quatrains and a final rhymed couplet. The user's first line is line A; "
        "your line 1 must be line B (rhymes with the next-but-one line). Use "
        "Early Modern English. Output EXACTLY 13 lines, one per line, no "
        "numbering, no commentary, no preamble. Do NOT repeat the user's "
        "opening line."
    )

    print(f"\n═══════════════════════════════════════════════════════════════════════")
    print(f"  SONNET COMPLETION  (constellation-guarded against THE SONNETS centroid)")
    print(f"═══════════════════════════════════════════════════════════════════════")
    print(f"  OPENING LINE:")
    print(f"    {args.first_line}\n")

    accepted = None
    attempts = []
    for attempt in range(1, args.max_attempts + 1):
        print(f"→ Attempt {attempt}/{args.max_attempts}…")
        out = gemma.generate(
            prompt=(f"My sonnet's first line:\n{args.first_line}\n\n"
                    f"Write the next 13 lines (so the total reaches exactly 14)."),
            system=sys_prompt, temperature=0.85 if attempt == 1 else 0.65,
            max_tokens=args.max_tokens, stop=["\n\n\n"])
        # Strip any duplicate of the opening line that Gemma included
        gen_lines = [ln for ln in out.text.strip().splitlines() if ln.strip()]
        if gen_lines and gen_lines[0].strip().lower() == args.first_line.strip().lower():
            gen_lines = gen_lines[1:]
        full = args.first_line + "\n" + "\n".join(gen_lines)
        verify = verify_sonnet(full)

        # Embed + score against constellations
        fp, scores = score_text(rpc, full, cons_cache=cons)
        v_short, v_long, m = authenticity_verdict(scores)
        soft_delete(rpc, fp)

        # Find Sonnets-specifically score
        sonnets_score = next((s.combined_score for s in scores
                              if s.work.lower() == "sonnets"), None)

        rec = {"attempt": attempt, "verify": verify, "verdict": v_short,
               "top_work": m["top_work"], "top_score": m["top_score"],
               "sonnets_score": sonnets_score, "text": full,
               "gen_ms": out.total_duration_ms, "tokens": out.eval_count}
        attempts.append(rec)
        score_str = (f"  sonnets={sonnets_score:.4f}"
                     if sonnets_score is not None else "  sonnets=N/A")
        print(f"   structurally_valid={verify['structurally_valid']}  "
              f"verdict={v_short}  top={m['top_work']} ({m['top_score']:.4f})"
              f"{score_str}")
        if verify["issues"]:
            for issue in verify["issues"]:
                print(f"     · {issue}")

        is_sonnets_top = m["top_work"].lower() == "sonnets"
        if (verify["structurally_valid"] and v_short in {"SHAKESPEAREAN", "AUTHENTIC"}
                and is_sonnets_top):
            accepted = rec
            break

    rpc.close()

    print(f"\n═══════════════════════════════════════════════════════════════════════")
    if accepted:
        print(f"  ✅ ACCEPTED on attempt {accepted['attempt']}/{len(attempts)}")
        print(f"     verdict={accepted['verdict']} top={accepted['top_work']}")
        print(f"     sonnets_centroid_score={accepted['sonnets_score']:.4f}")
        print(f"     {accepted['verify']['n_lines']} lines · "
              f"~{accepted['verify']['syllables_mean']:.1f} syllables/line · "
              f"rhyme {accepted['verify']['rhyme_match_count']}/{accepted['verify']['rhyme_match_total']}")
    else:
        print(f"  ⚠ NO FULLY-VALID ATTEMPT in {len(attempts)} tries — best below:")
        accepted = max(attempts, key=lambda r: r["sonnets_score"] or 0)
    print(f"───────────────────────────────────────────────────────────────────────")
    for i, line in enumerate(accepted["text"].splitlines(), 1):
        n_syl = syllable_count(line)
        marker = "  " if 8 <= n_syl <= 12 else "* "
        print(f"  {marker}{i:>2}: ({n_syl}) {line}")
    print(f"═══════════════════════════════════════════════════════════════════════\n")
    # Exit 0 only if a fully-valid attempt was found (structurally valid +
    # SHAKESPEAREAN verdict + Sonnets ranks first). Best-of-attempts on
    # mixed validity returns 4.
    return 0 if any(r.get("verdict") in {"SHAKESPEAREAN", "AUTHENTIC"}
                    and r.get("verify", {}).get("structurally_valid")
                    and (r.get("top_work") or "").lower() == "sonnets"
                    for r in attempts) else 4


if __name__ == "__main__":
    sys.exit(main())
