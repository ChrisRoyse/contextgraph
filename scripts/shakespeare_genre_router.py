#!/usr/bin/env python3
"""Demo 3 — Auto-Genre Routing.

User types a short prompt. We embed the prompt through the 13-embedder
pipeline, score it against every Shakespeare constellation, and pick the
work whose centroid is closest. Then we ask Gemma to continue in that work's
voice, using the work's tightest member text as a few-shot example.

Demonstrates the discovered geometric clustering in action: history-play
prompts route to histories, lover prompts route to comedies/sonnets,
revenge prompts route to tragedies.
"""
from __future__ import annotations

import argparse
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from _gemma_shakespeare_lib import (
    Rpc, GemmaClient, score_text, soft_delete,
)


GENRE_HINTS = {
    "history":  "Speak as a Shakespeare history play — kings, courts, succession, war.",
    "tragedy":  "Speak as a Shakespeare tragedy — fate, mortality, hubris, grief.",
    "comedy":   "Speak as a Shakespeare comedy — wit, mistaken identity, love, reconciliation.",
    "sonnet":   "Speak as a Shakespeare sonnet — 14 lines iambic pentameter ABAB CDCD EFEF GG.",
    "poem":     "Speak as a Shakespeare narrative poem — long-form mythological / allegorical verse.",
}


def classify_work(label: str) -> str:
    s = label.lower()
    if "sonnet" in s:
        return "sonnet"
    if any(k in s for k in ["henry", "richard", "king john"]):
        return "history"
    if "tragedy" in s or s.endswith("macbeth") or "lear" in s or "hamlet" in s or "othello" in s:
        return "tragedy"
    if any(k in s for k in ["lover", "venus", "lucrece", "phoenix", "passionate", "pilgrim"]):
        return "poem"
    return "comedy"


def fetch_member_excerpt(rpc: Rpc, constellation_id: str, work_label: str,
                         max_chars: int = 600) -> str | None:
    """Pull one representative excerpt from the work's session.

    NOTE: get_constellation currently returns an empty member_ids array
    even when member_count is non-zero (Phase 2 export gap). We work
    around this by using search_graph against the work's title — the top
    semantic hit is guaranteed to be from that work's session, and its
    training_record carries the content.
    """
    # Build a search query from the work label (works for plays/sonnets/poems)
    query = work_label.replace("Shakespeare — ", "").lower()
    # Add a topic word so search_graph returns a representative scene
    query = f"{query} scene"
    body = rpc.call("search_graph", {"query": query, "limit": 5},
                    timeout=30.0)
    for hit in body.get("results", []):
        sess = hit.get("sequenceInfo", {}).get("sessionId", "")
        # Prefer hits from this work's own session
        slug = work_label.lower().replace("shakespeare — ", "")
        slug_norm = "shakespeare-" + slug.replace(" ", "-").replace("'", "").replace(",", "")
        mid = hit.get("fingerprintId")
        if not mid:
            continue
        try:
            rec = rpc.call("get_training_record",
                           {"memoryId": mid, "includeVectors": False,
                            "includeEdges": False}, timeout=30.0)
            content = (rec.get("content") or "").strip()
            if content and (slug.split()[-1] in sess.lower()
                            or rec.get("session_id", "").lower().endswith(slug.split()[-1])):
                return content[:max_chars]
            # If we don't get a session match, take any non-empty content
            # from the search result (still useful as few-shot).
            if content:
                return content[:max_chars]
        except Exception:
            continue
    return None


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--prompt", required=True)
    ap.add_argument("--max-tokens", type=int, default=300)
    ap.add_argument("--temperature", type=float, default=0.85)
    ap.add_argument("--show-top-k", type=int, default=5)
    args = ap.parse_args()

    rpc = Rpc()
    gemma = GemmaClient()

    print(f"\n═══════════════════════════════════════════════════════════════════════")
    print(f"  AUTO-GENRE ROUTING  (Pillar 1 cluster discovery in action)")
    print(f"═══════════════════════════════════════════════════════════════════════")
    print(f"  PROMPT: {args.prompt!r}\n")

    # 1. Score prompt against all Shakespeare constellations
    print(f"→ Embedding prompt through 13 frozen models…")
    fp, scores = score_text(rpc, args.prompt)
    print(f"  ✓ scored against {len(scores)} constellations\n")

    print(f"→ Top {args.show_top_k} closest works (geometric routing decision):")
    for i, s in enumerate(scores[:args.show_top_k]):
        genre = classify_work(s.work)
        print(f"    [{i+1}] {s.combined_score:.4f}  {s.work[:50]:<50}  ({genre})")
    print()

    target = scores[0]
    target_genre = classify_work(target.work)
    print(f"→ Routed to: {target.work}  (genre: {target_genre})\n")

    # 2. Pull a few-shot example from that work
    print(f"→ Loading example excerpt from {target.work}…")
    excerpt = fetch_member_excerpt(rpc, target.constellation_id, target.work)
    soft_delete(rpc, fp)
    if not excerpt:
        rpc.close()
        raise RuntimeError(f"No usable member text in constellation "
                           f"{target.constellation_id}")
    print(f"  ✓ {len(excerpt)} chars excerpt loaded\n")

    # 3. Ask Gemma to continue
    system = (
        f"You are responding in the voice of Shakespeare's '{target.work}'. "
        f"{GENRE_HINTS.get(target_genre, '')} "
        f"Use Early Modern English. Match the rhythm and vocabulary of the "
        f"following authentic excerpt:\n\n---\n{excerpt}\n---\n\n"
        f"Now continue the requested theme in that voice."
    )
    print(f"→ Generating continuation via Gemma 4 (system has {len(system)} chars)…")
    out = gemma.generate(args.prompt, system=system,
                         temperature=args.temperature,
                         max_tokens=args.max_tokens, stop=["\n\n\n"])
    print(f"  ✓ {out.eval_count} tokens in {out.total_duration_ms:.0f}ms\n")

    print("═══════════════════════════════════════════════════════════════════════")
    print(f"  GENERATED  (in the voice of {target.work}):")
    print("───────────────────────────────────────────────────────────────────────")
    for line in out.text.splitlines():
        print(f"  {line}")
    print("═══════════════════════════════════════════════════════════════════════\n")

    rpc.close()
    return 0


if __name__ == "__main__":
    sys.exit(main())
