#!/usr/bin/env python3
"""End-to-end smoke test for v4.2 DPO training — BEFORE committing 3-4h.

Verifies:
  1. DPO trainer runs 20 steps on 50 pairs without OOM or import errors.
  2. Training loss decreases (or at least moves — not stuck at init).
  3. Adapter saves to disk with valid safetensors.
  4. Saved adapter reloads cleanly via PeftModel.from_pretrained.
  5. Reloaded adapter generates coherent Shakespeare on a probe prompt.
  6. Generated text is non-empty + non-degenerate (no immediate EOS,
     no empty tokens).

Total runtime: ~5-8 min on RTX 5090.

If ANY check fails, exits non-zero with root-cause diagnostics — do NOT
start the 3-4h training before this passes.

Usage:
    python3 scripts/v4_2/smoke_dpo_v4_2.py
"""
from __future__ import annotations

import argparse
import json
import logging
import os
import subprocess
import sys
import time
from pathlib import Path


logger = logging.getLogger("v4_2.smoke_dpo")


DEFAULT_PAIRS = Path(
    "~/.contextgraph/training/v4_2_camd_lmdb.jsonl"
).expanduser()


def run_training_smoke(pairs: Path, output_dir: Path, adapter_in: Path,
                        limit_pairs: int = 50, max_steps: int = 20) -> None:
    logger.info(f"Phase 1: DPO training smoke — {limit_pairs} pairs, {max_steps} steps")
    t0 = time.monotonic()
    cmd = [
        "python3", str(Path(__file__).resolve().parent / "train_camd_dpo.py"),
        "--pairs", str(pairs),
        "--output-dir", str(output_dir),
        "--adapter-in", str(adapter_in),
        "--limit-pairs", str(limit_pairs),
        "--max-steps", str(max_steps),
        "--batch-size", "2",   # smaller for smoke
        "--grad-accum", "2",
        "--lr", "5e-6",
        "--beta", "0.1",
        "--lambda-sft", "0.1",
        "--max-length", "512",       # shorter for smoke
        "--max-prompt-length", "64",
    ]
    logger.info(f"  cmd: {' '.join(cmd)}")
    result = subprocess.run(cmd, capture_output=True, text=True, check=False)
    if result.returncode != 0:
        print("=" * 74)
        print("SMOKE TRAINING FAILED — stdout tail:")
        print(result.stdout[-3000:])
        print("\nstderr tail:")
        print(result.stderr[-3000:])
        print("=" * 74)
        raise RuntimeError(
            f"train_camd_dpo.py exited with code {result.returncode}. "
            f"See tails above for root cause."
        )
    logger.info(f"  stdout tail: {result.stdout[-800:]}")
    logger.info(f"  training smoke done in {time.monotonic()-t0:.1f}s")


def verify_adapter_saved(final_dir: Path) -> dict:
    logger.info(f"Phase 2: verify adapter saved to {final_dir}")
    if not final_dir.exists():
        raise RuntimeError(f"adapter dir missing: {final_dir}")
    required = ["adapter_model.safetensors", "adapter_config.json"]
    missing = [f for f in required if not (final_dir / f).exists()]
    if missing:
        raise RuntimeError(f"adapter files missing: {missing}")
    sizes = {f: (final_dir / f).stat().st_size for f in required}
    logger.info(f"  adapter files present, sizes: {sizes}")
    if sizes["adapter_model.safetensors"] < 1_000_000:  # should be ~1.1 GB
        raise RuntimeError(
            f"adapter_model.safetensors unexpectedly small: "
            f"{sizes['adapter_model.safetensors']} bytes"
        )
    return sizes


def verify_reload_and_generate(adapter_dir: Path, base_model: str) -> str:
    logger.info(f"Phase 3: reload adapter from {adapter_dir} + generate")
    # Do the reload in a subprocess so GPU memory is fresh
    script = f"""
import sys, time
from pathlib import Path
import torch
from transformers import AutoTokenizer, AutoModelForCausalLM, BitsAndBytesConfig
from peft import PeftModel

t0 = time.time()
bnb = BitsAndBytesConfig(
    load_in_4bit=True, bnb_4bit_quant_type='nf4',
    bnb_4bit_compute_dtype=torch.bfloat16, bnb_4bit_use_double_quant=True)
tok = AutoTokenizer.from_pretrained('{adapter_dir}')
if tok.pad_token is None:
    tok.pad_token = tok.eos_token
base = AutoModelForCausalLM.from_pretrained(
    '{base_model}', quantization_config=bnb, device_map='auto',
    torch_dtype=torch.bfloat16, attn_implementation='sdpa')
model = PeftModel.from_pretrained(base, '{adapter_dir}', is_trainable=False)
model.eval()
print(f'[smoke] reload+load OK in {{time.time()-t0:.1f}}s', file=sys.stderr)

SYS = ('You are a knowledgeable assistant who writes EXCLUSIVELY in the style of '
       'William Shakespeare using early modern English.')
USR = 'Compose two lines of verse about the summer sun.'
messages = [{{'role':'system','content':SYS}},{{'role':'user','content':USR}}]
text = tok.apply_chat_template(messages, tokenize=False, add_generation_prompt=True)
inputs = tok(text, return_tensors='pt').to('cuda')
with torch.no_grad():
    out = model.generate(**inputs, max_new_tokens=80, do_sample=True,
                         temperature=0.8, top_p=0.92, repetition_penalty=1.2,
                         pad_token_id=tok.pad_token_id)
gen = tok.decode(out[0][inputs['input_ids'].shape[1]:], skip_special_tokens=True)
# Emit generation to stdout so smoke harness captures it
print(gen)
"""
    result = subprocess.run(
        ["python3", "-c", script],
        capture_output=True, text=True, check=False, timeout=300,
    )
    if result.returncode != 0:
        print("=" * 74)
        print("RELOAD+GENERATE FAILED — stderr:")
        print(result.stderr[-2000:])
        print("=" * 74)
        raise RuntimeError(
            f"reload/generate exited with {result.returncode}"
        )
    generated = result.stdout.strip()
    logger.info(f"  generation ({len(generated)} chars): {generated[:200]}")
    if len(generated) < 20:
        raise RuntimeError(
            f"generation suspiciously short ({len(generated)} chars): {generated!r}"
        )
    # Crude repetition check: no single 4-gram repeats > 3 times
    words = generated.split()
    if len(words) >= 8:
        four_grams = [tuple(words[i:i+4]) for i in range(len(words) - 3)]
        for ng in set(four_grams):
            if four_grams.count(ng) > 3:
                raise RuntimeError(
                    f"generation has a 4-gram repeated >3 times: {' '.join(ng)!r}. "
                    f"Full: {generated[:400]}"
                )
    return generated


def check_training_loss_trend(output_dir: Path) -> None:
    """Look at trainer_state.json for decreasing loss trend."""
    logger.info(f"Phase 4: verify loss trend in {output_dir}")
    # HF Trainer saves trainer_state.json inside the latest checkpoint-*
    checkpoints = sorted(output_dir.glob("checkpoint-*"))
    if not checkpoints:
        logger.warning("  no checkpoint-* dirs found; skipping loss-trend check")
        return
    latest = checkpoints[-1]
    state_file = latest / "trainer_state.json"
    if not state_file.exists():
        logger.warning(f"  trainer_state.json missing in {latest}; skipping")
        return
    state = json.loads(state_file.read_text())
    log_history = state.get("log_history", [])
    losses = [e["loss"] for e in log_history if "loss" in e]
    if len(losses) < 2:
        logger.warning(f"  only {len(losses)} loss entries; skipping trend check")
        return
    first, last = losses[0], losses[-1]
    logger.info(f"  losses: first={first:.4f} last={last:.4f} n={len(losses)}")
    # We don't require strict decrease (DPO loss can fluctuate), just "moved"
    if abs(last - first) < 1e-6:
        raise RuntimeError(
            f"training loss frozen: first={first}, last={last}. Optimizer not stepping?"
        )


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--pairs", type=Path, default=DEFAULT_PAIRS)
    ap.add_argument("--output-dir", type=Path,
                    default=Path("~/.contextgraph/models/shakespeare_lora_v4_2_smoke/dpo").expanduser())
    ap.add_argument("--adapter-in", type=Path,
                    default=Path("~/.contextgraph/models/shakespeare_lora/final").expanduser())
    ap.add_argument("--base", default="google/gemma-4-E4B-it")
    ap.add_argument("--limit-pairs", type=int, default=50)
    ap.add_argument("--max-steps", type=int, default=20)
    args = ap.parse_args()

    logging.basicConfig(level=logging.INFO,
                        format="%(asctime)s [%(name)s] %(levelname)s: %(message)s")

    # Fail fast on prereqs
    if not args.pairs.exists():
        raise FileNotFoundError(f"pairs not found: {args.pairs}")
    if not args.adapter_in.exists():
        raise FileNotFoundError(f"v2.1 adapter not found: {args.adapter_in}")
    os.environ.setdefault("HF_TOKEN", "")

    # Clean slate
    if args.output_dir.exists():
        import shutil
        shutil.rmtree(args.output_dir)

    start = time.monotonic()

    # Phase 1: 20-step training
    run_training_smoke(args.pairs, args.output_dir, args.adapter_in,
                        limit_pairs=args.limit_pairs, max_steps=args.max_steps)

    # Phase 2: adapter files on disk
    final_dir = args.output_dir / "final"
    sizes = verify_adapter_saved(final_dir)

    # Phase 3: reload + generate
    generated = verify_reload_and_generate(final_dir, args.base)

    # Phase 4: loss trend
    check_training_loss_trend(args.output_dir)

    wall = time.monotonic() - start
    print()
    print("=" * 74)
    print(f"V4.2 DPO SMOKE TEST — PASS in {wall/60:.1f} min")
    print("=" * 74)
    print(f"Training output: {args.output_dir}")
    print(f"Adapter saved: {final_dir}")
    print(f"Adapter safetensors size: {sizes['adapter_model.safetensors']/1e6:.1f} MB")
    print(f"Generated sample:\n  {generated[:300]}")
    print()
    print("GREEN LIGHT — safe to kick off full 3-4h training.")
    print("=" * 74)
    return 0


if __name__ == "__main__":
    sys.exit(main())
