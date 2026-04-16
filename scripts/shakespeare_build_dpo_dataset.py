#!/usr/bin/env python3
"""Build DPO dataset from the contrastive pairs in Context Graph.

For each contrastive pair (anchor=chosen, negative=rejected):
  1. Generate a modern-language prompt from the anchor text
  2. Format as DPO triple: {prompt, chosen, rejected, anomaly_kind}

Output: shakespeare_dpo.jsonl — ready for trl.DPOTrainer.

Usage:
    python3 scripts/shakespeare_build_dpo_dataset.py \
        --out ~/.contextgraph/training/shakespeare_dpo.jsonl \
        --max-pairs 13465
"""
from __future__ import annotations

import argparse
import json
import os
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from _gemma_shakespeare_lib import GemmaClient, Rpc

PROMPT_GEN_SYSTEM = (
    "You are a dataset builder. Given a passage of Shakespeare's writing, "
    "generate a SHORT modern-English prompt (1-2 sentences, under 30 words) "
    "that a user might type into a chatbot to get a response like this passage. "
    "The prompt should describe the THEME or REQUEST, not quote Shakespeare. "
    "Return ONLY the prompt text, nothing else."
)

SYSTEM_PROMPT = (
    "You are William Shakespeare. Write in his exact voice — early modern English, "
    "thee/thou/thy/thine/hast/dost/doth vocabulary, imagery drawn from nature, "
    "the stars, love, death, honour, and kingship."
)


def get_contrastive_pairs(rpc: Rpc, limit: int, offset: int = 0) -> list[dict]:
    """Fetch contrastive pairs with full content."""
    r = rpc.call("list_contrastive_pairs", {
        "limit": limit, "offset": offset, "includeFull": True,
    }, timeout=120.0)
    return r.get("pairs", [])


def generate_prompt(gemma: GemmaClient, text: str) -> str:
    """Generate a modern prompt from Shakespeare text."""
    snippet = text[:400] if len(text) > 400 else text
    r = gemma.generate(
        prompt=f"Shakespeare passage:\n\n{snippet}\n\nModern prompt:",
        system=PROMPT_GEN_SYSTEM,
        temperature=0.3,
        max_tokens=60,
        think=False,
    )
    prompt = r.text.strip().strip('"').strip("'")
    if not prompt or len(prompt) > 200:
        first_line = text.split("\n")[0][:80]
        prompt = f"Write about: {first_line}"
    return prompt


def build_dpo_example(
    prompt: str,
    chosen: str,
    rejected: str,
    anomaly_kind: str,
    anchor_id: str,
    negative_id: str,
    similarity_profile: list[float] | None = None,
) -> dict:
    return {
        "prompt": [
            {"role": "system", "content": SYSTEM_PROMPT},
            {"role": "user", "content": prompt},
        ],
        "chosen": [{"role": "assistant", "content": chosen}],
        "rejected": [{"role": "assistant", "content": rejected}],
        # Metadata
        "_meta": {
            "anomaly_kind": anomaly_kind,
            "anchor_id": anchor_id,
            "negative_id": negative_id,
            "similarity_profile": similarity_profile,
        },
    }


def main():
    ap = argparse.ArgumentParser(description="Build DPO dataset from contrastive pairs")
    ap.add_argument("--out", type=str,
                    default=os.path.expanduser("~/.contextgraph/training/shakespeare_dpo.jsonl"))
    ap.add_argument("--max-pairs", type=int, default=15000)
    ap.add_argument("--skip-prompt-gen", action="store_true",
                    help="Skip Gemma prompt gen (use first-line fallback)")
    ap.add_argument("--page-size", type=int, default=200,
                    help="Fetch pairs in pages of this size")
    args = ap.parse_args()

    out_path = Path(args.out)
    out_path.parent.mkdir(parents=True, exist_ok=True)

    print(f"[dpo-builder] connecting to daemon :3100 ...", file=sys.stderr)
    rpc = Rpc()

    gemma = None
    if not args.skip_prompt_gen:
        gemma = GemmaClient()
        print(f"[dpo-builder] Gemma4 client ready", file=sys.stderr)

    t0 = time.time()
    written = 0
    skipped = 0
    errors = 0
    offset = 0

    with open(out_path, "w") as f:
        while written < args.max_pairs:
            # Fetch a page of pairs
            print(f"[dpo-builder] fetching pairs offset={offset} limit={args.page_size} ...",
                  file=sys.stderr)
            pairs = get_contrastive_pairs(rpc, limit=args.page_size, offset=offset)
            if not pairs:
                print(f"[dpo-builder] no more pairs at offset {offset}", file=sys.stderr)
                break

            for pair in pairs:
                if written >= args.max_pairs:
                    break

                anchor_text = pair.get("anchor_text", "")
                negative_text = pair.get("negative_text", "")

                if not anchor_text or len(anchor_text.strip()) < 20:
                    skipped += 1
                    continue
                if not negative_text or len(negative_text.strip()) < 20:
                    skipped += 1
                    continue

                # Generate prompt from anchor (chosen) text
                if gemma and not args.skip_prompt_gen:
                    try:
                        prompt = generate_prompt(gemma, anchor_text)
                    except Exception as e:
                        print(f"  WARN: prompt gen failed: {e}", file=sys.stderr)
                        first_line = anchor_text.split("\n")[0][:80]
                        prompt = f"Write about: {first_line}"
                        errors += 1
                else:
                    first_line = anchor_text.split("\n")[0][:80]
                    prompt = f"Write about: {first_line}"

                example = build_dpo_example(
                    prompt=prompt,
                    chosen=anchor_text,
                    rejected=negative_text,
                    anomaly_kind=pair.get("anomaly_kind", "unknown"),
                    anchor_id=pair.get("anchor_id", ""),
                    negative_id=pair.get("negative_id", ""),
                    similarity_profile=pair.get("similarity_profile"),
                )
                f.write(json.dumps(example, ensure_ascii=False) + "\n")
                written += 1

            offset += args.page_size

            elapsed = time.time() - t0
            rate = written / elapsed if elapsed > 0 else 0
            print(
                f"[dpo-builder] progress: {written} written, {skipped} skipped, "
                f"{errors} errors, {rate:.1f} rows/sec",
                file=sys.stderr,
            )

    elapsed = time.time() - t0
    print(f"\n[dpo-builder] DONE", file=sys.stderr)
    print(f"  written:  {written}", file=sys.stderr)
    print(f"  skipped:  {skipped}", file=sys.stderr)
    print(f"  errors:   {errors}", file=sys.stderr)
    print(f"  duration: {elapsed:.1f}s", file=sys.stderr)
    print(f"  output:   {out_path}", file=sys.stderr)
    print(f"  size:     {out_path.stat().st_size / 1e6:.1f} MB", file=sys.stderr)

    rpc.close()


if __name__ == "__main__":
    main()
