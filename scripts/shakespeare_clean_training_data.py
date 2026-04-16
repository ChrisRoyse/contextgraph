#!/usr/bin/env python3
"""Clean Shakespeare training data: strip stage directions, scene headers,
character names, and formatting artifacts.

The ROOT CAUSE of the echo/formatting problem: 67% of training responses
contain raw play text with SCENE headers, Enter/Exit stage directions,
and CHARACTER NAME. prefixes. The model learns to output these instead
of pure Shakespearean prose/verse.

This script strips all of that, leaving ONLY the actual spoken words
and poetry — the pure Shakespeare voice.

Usage:
    python3 scripts/shakespeare_clean_training_data.py
"""
import json, os, re, sys
from pathlib import Path

INPUT = Path(os.path.expanduser("~/.contextgraph/training/shakespeare_sft_optimal.jsonl"))
OUTPUT = Path(os.path.expanduser("~/.contextgraph/training/shakespeare_sft_clean.jsonl"))


def clean_shakespeare_text(text: str) -> str:
    """Strip play formatting, keep only the spoken words and verse."""
    lines = text.split("\n")
    cleaned = []

    for line in lines:
        stripped = line.strip()
        if not stripped:
            continue

        # Skip scene headers (any case)
        if re.match(r'^(SCENE|Scene|ACT|Act)\s+[IVXivx]+', stripped):
            continue

        # Skip location descriptions (e.g., "A forest near Athens", "The same.")
        if re.match(r'^[A-Z][a-z].*\.\s*$', stripped) and len(stripped) < 80:
            if not any(w in stripped.lower() for w in ['thou', 'thee', 'thy', 'doth', 'hath', 'art', 'love', 'heart']):
                continue
        if stripped.lower() in ('the same.', 'another room.', 'another part of the field.'):
            continue

        # Skip stage directions — comprehensive patterns
        if re.match(r'^(Enter|Exit|Exeunt|Re-enter|Alarum|Cornets|Flourish|'
                     r'Sennet|Trumpets|Drums|Here |Then |Music|Retreat|'
                     r'Thunder|Lightning|Storm|Noise|Sound|Knocking|'
                     r'A (room|street|hall|forest|garden|field|camp|prison|cave|tent|court))',
                     stripped):
            continue
        # "Here an alarum", "Then enter", narrative stage directions
        if re.match(r'^(Here |Then |They |He |She |The |A |An )', stripped):
            # Check if it's a stage direction (no Shakespeare markers)
            lower = stripped.lower()
            has_speech = any(w in lower for w in ['thou', 'thee', 'thy', 'doth', 'hath', 'methinks', 'forsooth', 'prithee', 'hark'])
            if not has_speech and len(stripped) < 120:
                continue
        if stripped.startswith('[') and stripped.endswith(']'):
            continue
        if stripped.startswith('(') and stripped.endswith(')'):
            continue

        # Skip character name headers: "HAMLET.", "KING.", "ROMEO." etc
        # These are ALL CAPS followed by a period
        if re.match(r'^[A-Z][A-Z\s]+\.\s*$', stripped):
            continue
        # Also "FIRST CITIZEN.", "LORD BARDOLPH." etc
        if re.match(r'^[A-Z][A-Z\s]{2,}\.$', stripped):
            continue

        # Remove inline stage directions: [Aside], [To ROMEO], etc
        stripped = re.sub(r'\[.*?\]', '', stripped)
        stripped = re.sub(r'\(.*?\)', '', stripped)

        # Remove character name prefixes at start of speech lines
        # e.g., "HAMLET. To be or not to be" → "To be or not to be"
        stripped = re.sub(r'^[A-Z][A-Z\s]+\.\s+', '', stripped)

        stripped = stripped.strip()
        if stripped:
            cleaned.append(stripped)

    result = "\n".join(cleaned).strip()

    # If cleaning removed everything, return original (it was probably
    # already clean — a sonnet or creative example)
    if len(result) < 20 and len(text) > 20:
        return text.strip()

    return result


def main():
    data = [json.loads(l) for l in open(INPUT)]
    print(f"Input: {len(data)} examples from {INPUT}", file=sys.stderr)

    cleaned = 0
    unchanged = 0
    dropped = 0
    total_chars_removed = 0

    with open(OUTPUT, "w") as f:
        for ex in data:
            asst = ex["messages"][2]["content"]
            clean = clean_shakespeare_text(asst)

            if len(clean) < 20:
                dropped += 1
                continue

            if clean != asst:
                total_chars_removed += len(asst) - len(clean)
                cleaned += 1
            else:
                unchanged += 1

            ex["messages"][2]["content"] = clean
            f.write(json.dumps(ex, ensure_ascii=False) + "\n")

    total_kept = cleaned + unchanged
    print(f"\nResults:", file=sys.stderr)
    print(f"  Cleaned: {cleaned} (stage directions/headers removed)", file=sys.stderr)
    print(f"  Unchanged: {unchanged} (already clean)", file=sys.stderr)
    print(f"  Dropped: {dropped} (empty after cleaning)", file=sys.stderr)
    print(f"  Total kept: {total_kept}", file=sys.stderr)
    print(f"  Chars removed: {total_chars_removed:,}", file=sys.stderr)
    print(f"  Output: {OUTPUT}", file=sys.stderr)

    # Show before/after examples
    print(f"\n=== BEFORE/AFTER EXAMPLES ===", file=sys.stderr)
    original = [json.loads(l) for l in open(INPUT)]
    result = [json.loads(l) for l in open(OUTPUT)]
    shown = 0
    for orig, res in zip(original, result):
        o = orig["messages"][2]["content"]
        r = res["messages"][2]["content"]
        if o != r and o.startswith("SCENE"):
            print(f"BEFORE: {o[:150]}", file=sys.stderr)
            print(f"AFTER:  {r[:150]}", file=sys.stderr)
            print(file=sys.stderr)
            shown += 1
            if shown >= 3:
                break


if __name__ == "__main__":
    main()
