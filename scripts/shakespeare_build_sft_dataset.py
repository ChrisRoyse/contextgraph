#!/usr/bin/env python3
"""Build SFT dataset from the Shakespeare corpus in Context Graph.

For each memory in the knowledge graph:
  1. Fetch the raw Shakespeare text
  2. Score against all 44 constellations to find the closest work
  3. Ask Gemma4 (base, no LoRA) to generate a modern-language prompt
     that the Shakespeare text would naturally answer
  4. Format as ChatML: {system, user, assistant}

Output: shakespeare_sft.jsonl — one JSON object per line, ready for unsloth/axolotl.

Usage:
    python3 scripts/shakespeare_build_sft_dataset.py \
        --out ~/.contextgraph/training/shakespeare_sft.jsonl \
        --max-memories 3043
"""
from __future__ import annotations

import argparse
import json
import os
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from _gemma_shakespeare_lib import (
    GemmaClient,
    Rpc,
    list_shakespeare_constellations,
    score_text,
    soft_delete,
)

# ── Constellation-conditioned system prompts ──
# Each work-constellation gets a tailored system prompt so the model
# learns to shift register based on the tag.
STYLE_PROMPTS = {
    "Sonnets": "Style: Sonnets — intimate, lyrical, focused on beauty, time, love, and mortality. Iambic pentameter where natural. 14-line form when writing verse.",
    "Hamlet": "Style: Hamlet — philosophical, brooding, questioning existence and duty. Soliloquy-like introspection.",
    "Macbeth": "Style: Macbeth — dark, ambitious, driven by fate and guilt. Imagery of blood, night, and prophecy.",
    "Romeo and Juliet": "Style: Romeo and Juliet — passionate, youthful, star-crossed love and family conflict.",
    "A Midsummer Night's Dream": "Style: A Midsummer Night's Dream — playful, magical, woodland enchantment and comic confusion.",
    "The Tempest": "Style: The Tempest — mystical, reflective, themes of power, forgiveness, and release.",
    "King Lear": "Style: King Lear — tragic, elemental, madness and filial betrayal.",
    "Othello": "Style: Othello — jealousy, manipulation, love destroyed by deception.",
    "Twelfth Night": "Style: Twelfth Night — witty, comic, disguise and mistaken identity.",
    "Much Ado About Nothing": "Style: Much Ado — sharp-tongued banter, wit-combat between lovers.",
    "The Merchant of Venice": "Style: Merchant of Venice — justice vs mercy, bonds and debts.",
    "Julius Caesar": "Style: Julius Caesar — political oratory, loyalty and betrayal in the republic.",
    "Richard III": "Style: Richard III — scheming, sardonic, theatrical villainy.",
    "Henry V": "Style: Henry V — heroic, patriotic, the burden of kingship.",
}

BASE_SYSTEM = (
    "You are William Shakespeare. Write in his exact voice — early modern English, "
    "thee/thou/thy/thine/hast/dost/doth vocabulary, imagery drawn from nature, "
    "the stars, love, death, honour, and kingship. {style_tag}"
)

PROMPT_GEN_SYSTEM = (
    "You are a dataset builder. Given a passage of Shakespeare's writing, "
    "generate a SHORT modern-English prompt (1-2 sentences, under 30 words) "
    "that a user might type into a chatbot to get a response like this passage. "
    "The prompt should describe the THEME or REQUEST, not quote Shakespeare. "
    "Examples: 'Write about the pain of unrequited love' or 'Give me a speech about ambition and its consequences'. "
    "Return ONLY the prompt text, nothing else."
)


def get_all_memory_ids(rpc: Rpc, limit: int = 5000) -> list[str]:
    """Get fingerprint IDs via search with broad query."""
    # Use list_training_records which returns all IDs
    r = rpc.call("list_training_records", {"limit": limit, "offset": 0, "includeShape": False}, timeout=60.0)
    records = r.get("records", [])
    return [rec["memory_id"] for rec in records]


def get_content_for_memory(rpc: Rpc, memory_id: str) -> str | None:
    """Fetch the raw text content for a memory."""
    try:
        r = rpc.call("get_training_record", {"memoryId": memory_id, "includeVectors": False}, timeout=30.0)
        if r.get("found") is False:
            return None
        return r.get("content", "")
    except Exception as e:
        print(f"  WARN: get_training_record({memory_id[:8]}…) failed: {e}", file=sys.stderr)
        return None


def find_closest_work(rpc: Rpc, text: str, cons_cache: list[dict]) -> tuple[str, float]:
    """Score text against all constellations, return (work_name, score)."""
    fp, scores = score_text(rpc, text, cons_cache=cons_cache)
    soft_delete(rpc, fp)
    if not scores:
        return ("Unknown", 0.0)
    return (scores[0].work, scores[0].combined_score)


def generate_modern_prompt(gemma: GemmaClient, shakespeare_text: str) -> str:
    """Use Gemma4 to generate a modern prompt for the Shakespeare text."""
    # Truncate very long texts for the prompt generator
    snippet = shakespeare_text[:500] if len(shakespeare_text) > 500 else shakespeare_text
    r = gemma.generate(
        prompt=f"Shakespeare passage:\n\n{snippet}\n\nModern prompt:",
        system=PROMPT_GEN_SYSTEM,
        temperature=0.3,
        max_tokens=60,
        think=False,
    )
    prompt = r.text.strip().strip('"').strip("'")
    # Sanity: if Gemma returned something too long or empty, use a fallback
    if not prompt or len(prompt) > 200:
        # Fallback: extract theme from first line
        first_line = shakespeare_text.split("\n")[0][:100]
        prompt = f"Write something in the style of: {first_line}"
    return prompt


def build_sft_example(
    shakespeare_text: str,
    modern_prompt: str,
    work_name: str,
    work_score: float,
    memory_id: str,
) -> dict:
    """Build one ChatML training example."""
    # Get style-specific system prompt
    style_tag = STYLE_PROMPTS.get(work_name, f"Style: {work_name}.")
    system = BASE_SYSTEM.format(style_tag=style_tag)

    return {
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": modern_prompt},
            {"role": "assistant", "content": shakespeare_text},
        ],
        # Metadata (not used by trainer, but useful for analysis)
        "_meta": {
            "memory_id": memory_id,
            "work": work_name,
            "work_score": work_score,
            "prompt_source": "gemma4_generated",
        },
    }


def main():
    ap = argparse.ArgumentParser(description="Build SFT dataset from Shakespeare corpus")
    ap.add_argument("--out", type=str, default=os.path.expanduser("~/.contextgraph/training/shakespeare_sft.jsonl"))
    ap.add_argument("--max-memories", type=int, default=3043)
    ap.add_argument("--skip-prompt-gen", action="store_true",
                    help="Skip Gemma prompt generation (use simple fallback prompts)")
    ap.add_argument("--skip-constellation-scoring", action="store_true",
                    help="Skip constellation scoring (use 'General' as work for all)")
    ap.add_argument("--batch-size", type=int, default=50,
                    help="Process memories in batches of N (progress reporting)")
    args = ap.parse_args()

    out_path = Path(args.out)
    out_path.parent.mkdir(parents=True, exist_ok=True)

    print(f"[sft-builder] connecting to daemon :3100 ...", file=sys.stderr)
    rpc = Rpc()

    cons_cache = None
    if not args.skip_constellation_scoring:
        cons_cache = list_shakespeare_constellations(rpc)
        print(f"[sft-builder] {len(cons_cache)} constellations loaded", file=sys.stderr)

    gemma = None
    if not args.skip_prompt_gen:
        gemma = GemmaClient()
        print(f"[sft-builder] Gemma4 client ready", file=sys.stderr)

    # Get all memory IDs
    print(f"[sft-builder] fetching memory IDs (limit {args.max_memories}) ...", file=sys.stderr)
    ids = get_all_memory_ids(rpc, limit=args.max_memories)
    print(f"[sft-builder] got {len(ids)} memory IDs", file=sys.stderr)

    if not ids:
        print("[sft-builder] ERROR: no memories found. Is the Shakespeare corpus loaded?", file=sys.stderr)
        sys.exit(1)

    # Process each memory
    t0 = time.time()
    written = 0
    skipped = 0
    errors = 0

    with open(out_path, "w") as f:
        for i, mid in enumerate(ids):
            if i > 0 and i % args.batch_size == 0:
                elapsed = time.time() - t0
                rate = written / elapsed if elapsed > 0 else 0
                print(
                    f"[sft-builder] progress: {i}/{len(ids)} "
                    f"({written} written, {skipped} skipped, {errors} errors, "
                    f"{rate:.1f} rows/sec)",
                    file=sys.stderr,
                )

            # 1. Get content
            content = get_content_for_memory(rpc, mid)
            if not content or len(content.strip()) < 20:
                skipped += 1
                continue

            # 2. Score against constellations (optional)
            work_name = "General"
            work_score = 0.0
            if cons_cache and not args.skip_constellation_scoring:
                try:
                    work_name, work_score = find_closest_work(rpc, content, cons_cache)
                except Exception as e:
                    print(f"  WARN: constellation scoring failed for {mid[:8]}…: {e}", file=sys.stderr)
                    errors += 1

            # 3. Generate modern prompt (optional)
            if gemma and not args.skip_prompt_gen:
                try:
                    modern_prompt = generate_modern_prompt(gemma, content)
                except Exception as e:
                    print(f"  WARN: prompt gen failed for {mid[:8]}…: {e}", file=sys.stderr)
                    # Fallback
                    first_line = content.split("\n")[0][:80]
                    modern_prompt = f"Write about: {first_line}"
                    errors += 1
            else:
                # Simple fallback prompt from first line
                first_line = content.split("\n")[0][:80]
                modern_prompt = f"Write about: {first_line}"

            # 4. Build and write example
            example = build_sft_example(content, modern_prompt, work_name, work_score, mid)
            f.write(json.dumps(example, ensure_ascii=False) + "\n")
            written += 1

    elapsed = time.time() - t0
    print(f"\n[sft-builder] DONE", file=sys.stderr)
    print(f"  written:  {written}", file=sys.stderr)
    print(f"  skipped:  {skipped}", file=sys.stderr)
    print(f"  errors:   {errors}", file=sys.stderr)
    print(f"  duration: {elapsed:.1f}s ({written/elapsed:.1f} rows/sec)", file=sys.stderr)
    print(f"  output:   {out_path}", file=sys.stderr)
    print(f"  size:     {out_path.stat().st_size / 1e6:.1f} MB", file=sys.stderr)

    rpc.close()


if __name__ == "__main__":
    main()
