#!/usr/bin/env python3
"""Enhance SFT dataset prompts using Gemma4 to generate modern-language prompts.

Takes the existing shakespeare_sft.jsonl (with "Write about: {first_line}" prompts)
and replaces each prompt with a Gemma-generated modern-language question that the
Shakespeare text naturally answers.

NO daemon needed — only Ollama.

Usage:
    python3 scripts/shakespeare_enhance_prompts.py \
        --input ~/.contextgraph/training/shakespeare_sft.jsonl \
        --output ~/.contextgraph/training/shakespeare_sft_v2.jsonl
"""
from __future__ import annotations
import argparse, json, os, sys, time, urllib.request
from pathlib import Path

OLLAMA_URL = "http://127.0.0.1:11434"
MODEL = "gemma4:e4b"

PROMPT_GEN_SYSTEM = (
    "You are a dataset builder. Given a passage of Shakespeare's writing, "
    "generate a SHORT modern-English prompt (1-2 sentences, 10-25 words) "
    "that a user would type into a chatbot to get a response like this passage. "
    "The prompt should describe the THEME, EMOTION, or REQUEST — not quote Shakespeare. "
    "Do NOT include 'Write a sonnet' unless the passage is actually a sonnet. "
    "Do NOT include 'In the style of Shakespeare'. "
    "Examples of good prompts:\n"
    "- 'Tell me about the pain of unrequited love'\n"
    "- 'Give me a speech about ambition and its dark consequences'\n"
    "- 'Describe the beauty of a summer morning'\n"
    "- 'Write about jealousy destroying trust between friends'\n"
    "- 'Talk about how power corrupts even good people'\n"
    "Return ONLY the prompt text, nothing else. No quotes around it."
)


def gemma_generate(prompt: str, system: str, max_tokens: int = 60) -> str:
    body = {"model": MODEL, "stream": False, "think": False,
            "messages": [{"role": "system", "content": system},
                         {"role": "user", "content": prompt}],
            "options": {"temperature": 0.3, "num_predict": max_tokens}}
    req = urllib.request.Request(f"{OLLAMA_URL}/api/chat",
        data=json.dumps(body).encode(),
        headers={"Content-Type": "application/json"}, method="POST")
    with urllib.request.urlopen(req, timeout=120) as resp:
        return (json.loads(resp.read()).get("message") or {}).get("content", "").strip()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--input", default=os.path.expanduser("~/.contextgraph/training/shakespeare_sft.jsonl"))
    ap.add_argument("--output", default=os.path.expanduser("~/.contextgraph/training/shakespeare_sft_v2.jsonl"))
    ap.add_argument("--max-rows", type=int, default=None)
    args = ap.parse_args()

    # Verify Ollama
    try:
        urllib.request.urlopen(f"{OLLAMA_URL}/api/tags", timeout=5)
    except Exception as e:
        print(f"ERROR: Ollama not reachable at {OLLAMA_URL}: {e}", file=sys.stderr)
        sys.exit(1)

    rows = []
    with open(args.input) as f:
        for line in f:
            rows.append(json.loads(line))
    if args.max_rows:
        rows = rows[:args.max_rows]
    print(f"[enhance] loaded {len(rows)} rows from {args.input}", file=sys.stderr)

    t0 = time.time()
    enhanced = 0
    errors = 0
    with open(args.output, "w") as out:
        for i, row in enumerate(rows):
            if i > 0 and i % 25 == 0:
                elapsed = time.time() - t0
                rate = enhanced / elapsed if elapsed > 0 else 0
                print(f"[enhance] {i}/{len(rows)}: {enhanced} enhanced, {errors} errors, {rate:.1f}/sec", file=sys.stderr)

            # Skip adversarial/creative — they already have good prompts
            src = row.get("_meta", {}).get("prompt_source", "")
            if "adversarial" in src or "creative" in src:
                out.write(json.dumps(row, ensure_ascii=False) + "\n")
                continue

            # Get the Shakespeare text (assistant content)
            shakespeare_text = row["messages"][2]["content"]
            if not shakespeare_text or len(shakespeare_text.strip()) < 30:
                out.write(json.dumps(row, ensure_ascii=False) + "\n")
                continue

            # Generate modern prompt
            snippet = shakespeare_text[:400]
            try:
                prompt = gemma_generate(
                    f"Shakespeare passage:\n\n{snippet}\n\nModern prompt for this passage:",
                    PROMPT_GEN_SYSTEM
                )
                prompt = prompt.strip().strip('"').strip("'").strip()
                if not prompt or len(prompt) < 5 or len(prompt) > 150:
                    raise ValueError(f"bad prompt: '{prompt}'")
                # Replace the user message
                row["messages"][1]["content"] = prompt
                row["_meta"]["prompt_source"] = "gemma4_generated"
                enhanced += 1
            except Exception as e:
                errors += 1
                # Keep original prompt as fallback

            out.write(json.dumps(row, ensure_ascii=False) + "\n")

    elapsed = time.time() - t0
    print(f"\n[enhance] DONE in {elapsed:.0f}s", file=sys.stderr)
    print(f"  enhanced: {enhanced}/{len(rows)}", file=sys.stderr)
    print(f"  errors:   {errors}", file=sys.stderr)
    print(f"  output:   {args.output}", file=sys.stderr)
    sz = Path(args.output).stat().st_size / 1e6
    print(f"  size:     {sz:.1f} MB", file=sys.stderr)


if __name__ == "__main__":
    main()
