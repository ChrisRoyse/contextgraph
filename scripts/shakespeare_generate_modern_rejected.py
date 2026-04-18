#!/usr/bin/env python3
"""Generate base-Gemma MODERN RESPONSES for every SFT prompt.

These are the REJECTED candidates for DPO. For each SFT record:
  - PROMPT: the user message
  - CHOSEN: the existing Shakespeare assistant message (already in SFT)
  - REJECTED: a base-Gemma response (NO LoRA adapter) — genuinely modern English

The output is `~/.contextgraph/training/gemma_base_rejected.jsonl` with one
record per input SFT row:
  {"sft_index": N, "prompt": "...", "rejected_text": "..."}

Use this as input to shakespeare_build_resurrection_dpo.py, which then scores
CHOSEN + REJECTED against the Sonnets constellation and writes DPO pairs.

WHY this over rule-based paraphrase:
  Rule-based thee→you preserves meter/structure/rhetorical patterns. E12
  ColBERT still scores 0.42 on the paraphrased text (vs 0.04 on genuine
  modern English). Base Gemma produces fundamentally different sentence
  shape, rhythm, and vocabulary — exactly the "loses everything stylistically"
  signal DPO needs to learn.

VRAM budget:
  - Base Gemma 4-bit: ~5 GB
  - Daemon (13 embedders): ~17 GB
  - Total peak: ~22 GB on 31.8 GB RTX 5090 → 9 GB headroom

Reproducibility: deterministic generation with seed=42, temperature=0.7.
"""
from __future__ import annotations
import argparse
import gc
import json
import os
import sys
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Iterator

import torch

HOME = Path.home()
DEFAULT_SFT = HOME / ".contextgraph/training/shakespeare_sft_final_clean.jsonl"
DEFAULT_OUT = HOME / ".contextgraph/training/gemma_base_rejected.jsonl"

BASE_MODEL = "google/gemma-4-E4B-it"
MAX_NEW_TOKENS = 300
TEMPERATURE = 0.7


def iter_sft_prompts(path: Path, limit: int | None = None,
                     start: int = 0, end: int | None = None) -> Iterator[tuple[int, str, str]]:
    """Yield (sft_index, user_prompt, chosen_assistant_text) for well-formed records."""
    produced = 0
    with path.open() as f:
        for i, line in enumerate(f):
            if i < start:
                continue
            if end is not None and i >= end:
                return
            line = line.strip()
            if not line:
                continue
            try:
                obj = json.loads(line)
            except json.JSONDecodeError:
                continue
            msgs = obj.get("messages") or []
            if len(msgs) < 3 or msgs[1].get("role") != "user" or msgs[-1].get("role") != "assistant":
                continue
            user = (msgs[1].get("content") or "").strip()
            chosen = (msgs[-1].get("content") or "").strip()
            if not user or not chosen:
                continue
            yield i, user, chosen
            produced += 1
            if limit is not None and produced >= limit:
                return


def load_existing_rejected(out_path: Path) -> set[int]:
    """Parse existing output file to find already-processed sft_indexes (resumable)."""
    if not out_path.is_file():
        return set()
    done: set[int] = set()
    try:
        with out_path.open() as f:
            for line in f:
                line = line.strip()
                if not line:
                    continue
                try:
                    obj = json.loads(line)
                    if "sft_index" in obj and "rejected_text" in obj:
                        done.add(int(obj["sft_index"]))
                except json.JSONDecodeError:
                    continue
    except Exception:
        pass
    return done


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--sft-input", type=Path, default=DEFAULT_SFT)
    ap.add_argument("--output", type=Path, default=DEFAULT_OUT)
    ap.add_argument("--start", type=int, default=0)
    ap.add_argument("--end", type=int, default=None)
    ap.add_argument("--limit", type=int, default=None)
    ap.add_argument("--max-new-tokens", type=int, default=MAX_NEW_TOKENS)
    ap.add_argument("--temperature", type=float, default=TEMPERATURE)
    ap.add_argument("--hf-token", type=str, default=os.environ.get("HF_TOKEN", ""))
    args = ap.parse_args()

    if not args.sft_input.is_file():
        print(f"[fatal] SFT input missing: {args.sft_input}", file=sys.stderr)
        return 1

    if args.hf_token:
        os.environ["HF_TOKEN"] = args.hf_token
    if not os.environ.get("HF_TOKEN"):
        print("[warn] HF_TOKEN not set — gated model downloads will fail",
              file=sys.stderr)

    # Resume: parse any existing output
    args.output.parent.mkdir(parents=True, exist_ok=True)
    already_done = load_existing_rejected(args.output)
    print(f"[resume] {len(already_done)} records already in {args.output}",
          file=sys.stderr)

    # Lazy-load model so CLI --help works without GPU
    print(f"[model] loading base Gemma ({BASE_MODEL}) in 4-bit...", file=sys.stderr)
    t0 = time.time()
    from transformers import AutoModelForCausalLM, AutoTokenizer, BitsAndBytesConfig

    bnb = BitsAndBytesConfig(
        load_in_4bit=True,
        bnb_4bit_compute_dtype=torch.bfloat16,
        bnb_4bit_use_double_quant=True,
        bnb_4bit_quant_type="nf4",
    )
    tokenizer = AutoTokenizer.from_pretrained(BASE_MODEL, token=os.environ.get("HF_TOKEN"))
    if tokenizer.pad_token_id is None:
        tokenizer.pad_token = tokenizer.eos_token
    tokenizer.padding_side = "left"

    model = AutoModelForCausalLM.from_pretrained(
        BASE_MODEL,
        token=os.environ.get("HF_TOKEN"),
        quantization_config=bnb,
        torch_dtype=torch.bfloat16,
        device_map="auto",
    )
    model.eval()
    print(f"[model] loaded in {time.time()-t0:.1f}s — VRAM used: "
          f"{torch.cuda.memory_allocated()/1e9:.1f} GB", file=sys.stderr)

    # NEUTRAL system prompt — we want GENERIC modern responses, NOT Shakespeare.
    # Critical: when the prompt ASKS about Shakespeare or uses archaic language,
    # Gemma tends to echo theatrical/literary vocabulary. We explicitly block that.
    NEUTRAL_SYSTEM = (
        "You are a plain-spoken modern AI assistant in 2026. Answer every question in "
        "contemporary casual English — NEVER use archaic words (thee, thou, thy, hast, "
        "dost, doth, ye, prithee, forsooth, hark, methinks, 'tis, 'twas, o'er, verily, "
        "perchance, mayhap, alas, unto, whilst), poetic devices, iambic rhythm, rhyme, "
        "inverted word order, rhetorical flourishes, or literary vocabulary like "
        "'soliloquy', 'ode', 'sonnet'. Respond like ChatGPT or a modern assistant: "
        "short sentences, bullet points, contemporary word choice. Even if the user "
        "asks about Shakespeare, Elizabethan times, or classical literature, give "
        "FACTUAL plain answers — never imitate the style."
    )

    out_f = args.output.open("a", encoding="utf-8")
    skipped = 0
    generated = 0
    t_run = time.time()

    try:
        for sft_index, prompt, _chosen in iter_sft_prompts(
            args.sft_input, limit=args.limit, start=args.start, end=args.end
        ):
            if sft_index in already_done:
                skipped += 1
                continue

            messages = [
                {"role": "system", "content": NEUTRAL_SYSTEM},
                {"role": "user", "content": prompt},
            ]
            # apply_chat_template → plain string, then tokenize normally
            prompt_str = tokenizer.apply_chat_template(
                messages, tokenize=False, add_generation_prompt=True,
            )
            inputs = tokenizer(prompt_str, return_tensors="pt").to(model.device)
            input_len = inputs["input_ids"].shape[1]

            with torch.inference_mode():
                out = model.generate(
                    **inputs,
                    max_new_tokens=args.max_new_tokens,
                    temperature=args.temperature,
                    do_sample=True,
                    top_p=0.9,
                    repetition_penalty=1.15,
                    pad_token_id=tokenizer.pad_token_id,
                )
            # Decode only new tokens (after the input prompt)
            gen_tokens = out[0][input_len:]
            text = tokenizer.decode(gen_tokens, skip_special_tokens=True).strip()

            rec = {
                "sft_index": sft_index,
                "prompt": prompt,
                "rejected_text": text,
                "generated_at": datetime.now(timezone.utc).isoformat(),
            }
            out_f.write(json.dumps(rec, ensure_ascii=False) + "\n")
            out_f.flush()
            generated += 1

            if generated % 20 == 0:
                elapsed = time.time() - t_run
                rate = generated / elapsed
                # Estimate total based on observed rate
                done_total = len(already_done) + generated
                print(f"[gen] {generated} generated ({done_total} total), "
                      f"{rate*60:.1f}/min — {elapsed/60:.1f}min elapsed",
                      file=sys.stderr)

        out_f.close()
        elapsed = time.time() - t_run
        print(f"[done] generated={generated} skipped={skipped} "
              f"elapsed={elapsed/60:.1f}min  output={args.output}",
              file=sys.stderr)
        return 0
    finally:
        try:
            out_f.close()
        except Exception:
            pass
        # Aggressively free GPU memory
        del model
        del tokenizer
        gc.collect()
        torch.cuda.empty_cache()


if __name__ == "__main__":
    sys.exit(main())
