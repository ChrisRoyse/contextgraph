#!/usr/bin/env python3
"""Extract a broader SFT corpus from ALL real Shakespeare signals (no synthetic).

Reads ~/.contextgraph/training/shakespeare_sft_all_signals.jsonl (45,750 raw
records: 2,698 memories × 17 prompt angles + adversarial/curated variants)
and produces a curated expansion:

  ~/.contextgraph/training/shakespeare_sft_broader.jsonl

Target: 12,000-18,000 high-quality SFT examples from REAL Shakespeare text.
Zero synthetic content. Every assistant response is cleaned verbatim text
from his complete works.

Pipeline:
  1. Load all 45,750 raw signal records.
  2. Strip play formatting from each assistant response (reuse logic from
     shakespeare_rebuild_all_data.py — scene headers, stage directions,
     character-name prefixes).
  3. Drop records where cleaned response < MIN_RESPONSE_CHARS (default 120).
  4. Replace system prompt with UNIFIED_SYSTEM (matches v2.1 dashboard +
     inference server).
  5. Exclude records whose cleaned response is already in final_clean
     (deduplicate on SHA-1 of first 200 chars of cleaned response).
  6. Cap to MAX_ANGLES_PER_MEMORY (default 7) prompt angles per memory —
     keeps diversity without flooding DPO with near-duplicate fragments.
  7. Emit to broader.jsonl with stable _meta for provenance.

Fail-fast: any missing input file → exit 1. Any record without the expected
schema → logged to stderr + counted; does not corrupt output.

Idempotent: if output file is complete (has completion marker), exits 0
immediately.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import os
import re
import sys
from collections import Counter, defaultdict
from pathlib import Path
from typing import Iterator

HOME = Path.home()
INPUT_ALL_SIGNALS = HOME / ".contextgraph/training/shakespeare_sft_all_signals.jsonl"
INPUT_CLEAN = HOME / ".contextgraph/training/shakespeare_sft_final_clean.jsonl"
OUTPUT = HOME / ".contextgraph/training/shakespeare_sft_broader.jsonl"

MIN_RESPONSE_CHARS = 120
MAX_ANGLES_PER_MEMORY = 7
COMPLETION_MARKER = '{"_broader_corpus_complete": true}\n'

# Matches the v2.1 dashboard + inference server + cleaner scripts.
UNIFIED_SYSTEM = (
    "You are a knowledgeable assistant who writes EXCLUSIVELY in the style of "
    "William Shakespeare — early modern English using thee, thou, thy, thine, hast, "
    "dost, doth, ye, prithee, forsooth, hark, wherefore, methinks, 'tis, 'twas. "
    "You have full knowledge of ALL modern topics — science, technology, code, "
    "history, math, culture — and you explain them accurately and helpfully, but "
    "ALWAYS in Shakespearean prose or verse. Never refuse a topic. Never claim "
    "ignorance of modern things. Just answer everything in Shakespeare's voice."
)

# Play-formatting strippers (ported verbatim from
# scripts/shakespeare_rebuild_all_data.py & clean_training_data.py).
SCENE_HEADER_RE = re.compile(r'^(SCENE|Scene|ACT|Act)\s+[IVXivx]+')
STAGE_DIR_RE = re.compile(
    r'^(Enter|Exit|Exeunt|Re-enter|Alarum|Cornets|Flourish|Sennet|Trumpets|'
    r'Drums|Music|Retreat|Thunder|Lightning|Storm|Noise|Sound|Knocking|'
    r'A (room|street|hall|forest|garden|field|camp|prison|cave|tent|court))'
)
CHAR_NAME_RE = re.compile(r'^[A-Z][A-Z\s]+\.\s*$')
CHAR_NAME_PREFIX_RE = re.compile(r'^[A-Z][A-Z\s]+\.\s+')
LOCATION_RE = re.compile(r'^[A-Z][a-z].*\.\s*$')
SHAKE_MARKER_SET = frozenset([
    "thou", "thee", "thy", "thine", "doth", "hath", "hast", "art",
    "methinks", "forsooth", "prithee", "hark", "love", "heart", "wherefore",
])


def strip_play_formatting(text: str) -> str:
    """Return text with scene headers, stage directions, character prefixes removed.
    Cleaned output is pure speech/verse."""
    if not text:
        return ""
    out: list[str] = []
    for line in text.split("\n"):
        s = line.strip()
        if not s:
            continue
        if SCENE_HEADER_RE.match(s):
            continue
        if STAGE_DIR_RE.match(s):
            continue
        # Pure character headers like "HAMLET." or "KING LEAR." on their own line
        if CHAR_NAME_RE.match(s):
            continue
        # Location-only lines (short, capitalized, no Shakespeare vocabulary)
        if LOCATION_RE.match(s) and len(s) < 80:
            lower = s.lower()
            if not any(m in lower for m in SHAKE_MARKER_SET):
                continue
        # Parenthetical / bracketed stage directions spanning a whole line
        if (s.startswith('[') and s.endswith(']')) or (s.startswith('(') and s.endswith(')')):
            continue
        # Inline parentheticals/brackets
        s = re.sub(r'\[.*?\]', '', s)
        s = re.sub(r'\(.*?\)', '', s)
        # Leading character name prefix: "HAMLET. To be or not to be"
        s = CHAR_NAME_PREFIX_RE.sub('', s)
        s = s.strip()
        if s:
            out.append(s)
    return "\n".join(out).strip()


def sha1_key(text: str, n: int = 200) -> str:
    """Stable duplicate-detection key from first n chars of cleaned text."""
    return hashlib.sha1(text[:n].encode('utf-8', errors='replace')).hexdigest()


def load_existing_keys(path: Path) -> set[str]:
    """Return set of sha1-200 keys already present in final_clean. Empty if absent."""
    keys: set[str] = set()
    if not path.is_file():
        return keys
    with path.open() as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                obj = json.loads(line)
            except json.JSONDecodeError:
                continue
            msgs = obj.get("messages") or []
            if len(msgs) >= 3 and msgs[-1].get("role") == "assistant":
                keys.add(sha1_key(msgs[-1]["content"]))
    return keys


def iter_raw(path: Path) -> Iterator[tuple[int, dict]]:
    with path.open() as f:
        for i, line in enumerate(f):
            line = line.strip()
            if not line:
                continue
            try:
                yield i, json.loads(line)
            except json.JSONDecodeError:
                continue


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--input-all-signals", type=Path, default=INPUT_ALL_SIGNALS)
    ap.add_argument("--input-final-clean", type=Path, default=INPUT_CLEAN,
                    help="Existing corpus used to dedupe — broader excludes duplicates of this")
    ap.add_argument("--output", type=Path, default=OUTPUT)
    ap.add_argument("--min-response-chars", type=int, default=MIN_RESPONSE_CHARS)
    ap.add_argument("--max-angles-per-memory", type=int, default=MAX_ANGLES_PER_MEMORY)
    args = ap.parse_args()

    if not args.input_all_signals.is_file():
        print(f"[fatal] all_signals input missing: {args.input_all_signals}", file=sys.stderr)
        return 1

    # Idempotency check
    if args.output.is_file():
        last_line = ""
        with args.output.open() as f:
            for line in f:
                if line.strip():
                    last_line = line
        if last_line == COMPLETION_MARKER:
            print(f"[done] output already complete: {args.output}", file=sys.stderr)
            return 0
        else:
            print(f"[resume-overwrite] existing {args.output} has no completion marker; rewriting",
                  file=sys.stderr)

    existing_keys = load_existing_keys(args.input_final_clean)
    print(f"[load] existing final_clean assistants (for dedupe): {len(existing_keys)} unique SHA-1 keys",
          file=sys.stderr)

    partial = args.output.with_suffix(args.output.suffix + ".partial")
    args.output.parent.mkdir(parents=True, exist_ok=True)

    # Per-memory ordering: we want different PROMPT SOURCES for diversity,
    # so we gather all candidate records per (memory_id, prompt_source) and
    # then select top-N-distinct-sources per memory at emission time.
    # This gives us balanced coverage across E1/E5/E7/E8/E10/E11/E12/E13 angles.

    # priority: prompt_source types (most "structural" first — these are the
    # angles that ask about style/structure, not just semantics)
    PROMPT_SOURCE_PRIORITY = [
        "L2_E12_ColBERT",        # phrase-level / phrase style
        "L2_E13_SPLADE",         # term-expansion
        "L2_E7_Code/Structure",  # syntactic pattern
        "L2_E9_Character-pattern", # noise-robust structure
        "L2_E10_Paraphrase",     # paraphrase signal
        "L2_E8_Graph",           # structural relationships
        "L2_E11_Entity",         # entity-level
        "L2_E1_Semantic",        # topical (last — we don't want semantic to dominate)
        "L2_E5_Causal",          # causal chains
        "L2_E6_Keyword",         # keyword
        "L1_base",               # base prompt
        "L2_E4_Positional",      # positional
        "L2_E3_Periodic",        # periodic
        "L2_E2_Recency",         # recency
        "L4_constellation",      # constellation-level
        "L8_tucker_moderate",    # tucker decomposition
        "L6_edge_entity",
        "L6_edge_graph",
        "L6_edge_temporal",
        "L7_temporal_early",
        "L7_temporal_middle",
        "L7_temporal_late",
        "curated_creative",
        "adversarial_language_boost",
        "adversarial_code_boost",
        "adversarial_break_boost",
        "adversarial_mixed",
    ]
    PRIORITY_INDEX = {src: i for i, src in enumerate(PROMPT_SOURCE_PRIORITY)}

    # Aggregate candidates per memory
    # by_memory: mem_id -> list of (priority_index, cleaned_asst, prompt_source, raw_record)
    by_memory: dict[str, list[tuple[int, str, str, dict]]] = defaultdict(list)
    memoryless: list[tuple[int, str, str, dict]] = []

    total_seen = 0
    dropped_short = 0
    dropped_schema = 0
    dropped_dup_existing = 0

    for line_no, obj in iter_raw(args.input_all_signals):
        total_seen += 1
        msgs = obj.get("messages") or []
        if len(msgs) < 3 or msgs[-1].get("role") != "assistant" \
                or msgs[1].get("role") != "user":
            dropped_schema += 1
            continue
        user = (msgs[1].get("content") or "").strip()
        asst_raw = msgs[-1].get("content") or ""
        asst_clean = strip_play_formatting(asst_raw)

        if len(asst_clean) < args.min_response_chars:
            dropped_short += 1
            continue
        if not user:
            dropped_schema += 1
            continue

        key = sha1_key(asst_clean)
        if key in existing_keys:
            dropped_dup_existing += 1
            continue

        meta = obj.get("_meta") or {}
        mem_id = meta.get("memory_id")
        prompt_source = meta.get("prompt_source", "unknown")
        pri = PRIORITY_INDEX.get(prompt_source, len(PRIORITY_INDEX))

        entry = (pri, asst_clean, prompt_source, obj)
        if mem_id:
            by_memory[mem_id].append(entry)
        else:
            memoryless.append(entry)

    print(f"[filter] total_seen={total_seen} dropped_schema={dropped_schema} "
          f"dropped_short={dropped_short} dropped_dup_existing={dropped_dup_existing}",
          file=sys.stderr)
    print(f"[filter] unique memories: {len(by_memory)}, memoryless: {len(memoryless)}",
          file=sys.stderr)

    # For each memory, select TOP N by priority (lowest index first = highest priority).
    # We keep DISTINCT (prompt_source, user_prompt) combinations — same assistant
    # response across multiple user prompts is VALUABLE SFT signal (teaches the
    # model that different kinds of questions can all elicit the same Shakespeare
    # passage → better generalization).
    selected: list[dict] = []
    per_memory_kept = Counter()
    intra_memory_prompt_dups = 0

    for mem_id, entries in by_memory.items():
        entries.sort(key=lambda x: x[0])
        kept_prompt_keys: set[str] = set()  # (prompt_source, first 100 chars of user prompt)
        for pri, asst_clean, prompt_source, raw in entries:
            if len(kept_prompt_keys) >= args.max_angles_per_memory:
                break
            user_prompt = raw["messages"][1].get("content") or ""
            prompt_key = (prompt_source, user_prompt[:100])
            if prompt_key in kept_prompt_keys:
                intra_memory_prompt_dups += 1
                continue
            kept_prompt_keys.add(prompt_key)
            new_rec = {
                "messages": [
                    {"role": "system", "content": UNIFIED_SYSTEM},
                    {"role": "user",   "content": raw["messages"][1]["content"]},
                    {"role": "assistant", "content": asst_clean},
                ],
                "_meta": {
                    "memory_id": mem_id,
                    "work": (raw.get("_meta") or {}).get("work", ""),
                    "prompt_source": prompt_source,
                    "n_signals": (raw.get("_meta") or {}).get("n_signals"),
                    "importance": (raw.get("_meta") or {}).get("importance"),
                    "texture": (raw.get("_meta") or {}).get("texture"),
                    "source_file": "shakespeare_sft_all_signals.jsonl",
                    "cleaned_via": "shakespeare_extract_broader_sft",
                },
            }
            selected.append(new_rec)
            per_memory_kept[mem_id] += 1

    # Memoryless (curated_creative, adversarial) — emit all
    for pri, asst_clean, prompt_source, raw in memoryless:
        new_rec = {
            "messages": [
                {"role": "system", "content": UNIFIED_SYSTEM},
                {"role": "user",   "content": raw["messages"][1]["content"]},
                {"role": "assistant", "content": asst_clean},
            ],
            "_meta": {
                "memory_id": None,
                "work": (raw.get("_meta") or {}).get("work", ""),
                "prompt_source": prompt_source,
                "source_file": "shakespeare_sft_all_signals.jsonl",
                "cleaned_via": "shakespeare_extract_broader_sft",
            },
        }
        selected.append(new_rec)

    print(f"[select] selected={len(selected)} intra_memory_prompt_dup_skipped={intra_memory_prompt_dups}",
          file=sys.stderr)
    print(f"[select] per-memory angle distribution:", file=sys.stderr)
    angle_dist = Counter(per_memory_kept.values())
    for angles, memories in sorted(angle_dist.items()):
        print(f"  {angles} angles: {memories} memories", file=sys.stderr)

    # Atomic write
    with partial.open("w", encoding="utf-8") as f:
        for rec in selected:
            f.write(json.dumps(rec, ensure_ascii=False) + "\n")
        f.write(COMPLETION_MARKER)
    partial.replace(args.output)

    print(f"\n[done] wrote {len(selected)} records to {args.output}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
