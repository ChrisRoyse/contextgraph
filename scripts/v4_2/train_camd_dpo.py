#!/usr/bin/env python3
"""CAMD-DPO training — v4.2 Phase 4.

Starts from the v2.1 adapter (NOT v4.1, per docs/V4_2_NOVEL_METHODS.md §3.Phase3
rationale), loads the CAMD pairs JSONL mined by mine_camd_pairs.py, and runs
DPO with Reg-DPO SFT-regularization and length-normalized reward.

Design decisions baked in:

    - β (beta) = 0.1     — conservative for style transfer (from research
                           brief: style adapters tolerate 0.05-0.3; 0.1 is
                           middle-of-range, safe default).
    - LR 5e-6             — half of typical SFT LR (Cerebras DPO guidance).
    - λ_sft = 0.1         — Reg-DPO SFT-regularization weight (ArXiv 2511.01450).
    - length_normalize=True — length-bias-free reward (LD-DPO).
    - max_length 1024     — keep chosen+rejected each under ~500 tokens.
    - 1 epoch over ~4500 pairs.
    - Cosine LR schedule, 5% warmup, decay to 10%.
    - Gradient checkpointing on (RTX 5090 32GB with rank-256 LoRA).
    - bf16 compute, 4-bit nf4 quantized base.

Acceptance gate: after training, run browser FSV via Playwright (separate
script). 13/15 strict required before HF push per §5 acceptance criteria.

Usage:
    python3 scripts/v4_2/train_camd_dpo.py \
        --base google/gemma-4-E4B-it \
        --adapter-in ~/.contextgraph/models/shakespeare_lora/final \
        --pairs ~/.contextgraph/training/v4_2_camd_pairs.jsonl \
        --output-dir ~/.contextgraph/models/shakespeare_lora_v4_2/dpo \
        --epochs 1 --batch-size 4 --lr 5e-6 --beta 0.1

Prerequisites:
    pip install transformers>=5.5 peft>=0.19 trl>=1.1 bitsandbytes>=0.49
    Base model and v2.1 adapter already on disk.
"""
from __future__ import annotations

import argparse
import json
import os
import sys
from pathlib import Path


def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(description=__doc__,
                                formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--base", default="google/gemma-4-E4B-it",
                   help="Base model ID on HuggingFace Hub.")
    p.add_argument("--adapter-in", type=Path,
                   default=Path("~/.contextgraph/models/shakespeare_lora/final").expanduser(),
                   help="Starting LoRA adapter (v2.1 recommended, NOT v4.1).")
    p.add_argument("--pairs", type=Path,
                   default=Path("~/.contextgraph/training/v4_2_camd_pairs.jsonl").expanduser(),
                   help="CAMD pairs JSONL from mine_camd_pairs.py.")
    p.add_argument("--output-dir", type=Path,
                   default=Path("~/.contextgraph/models/shakespeare_lora_v4_2/dpo").expanduser())
    p.add_argument("--epochs", type=int, default=1)
    p.add_argument("--batch-size", type=int, default=4,
                   help="Per-device batch size. DPO needs 2x (chosen+rejected).")
    p.add_argument("--grad-accum", type=int, default=4,
                   help="Gradient accumulation steps — effective batch = batch_size * this.")
    p.add_argument("--lr", type=float, default=5e-6,
                   help="Peak learning rate. DPO convention: half of SFT LR.")
    p.add_argument("--beta", type=float, default=0.1,
                   help="DPO β (KL regularization strength). 0.05-0.3 typical for style.")
    p.add_argument("--lambda-sft", type=float, default=0.1,
                   help="Reg-DPO SFT loss regularization weight.")
    p.add_argument("--max-length", type=int, default=1024)
    p.add_argument("--max-prompt-length", type=int, default=128)
    p.add_argument("--warmup-ratio", type=float, default=0.05)
    p.add_argument("--gradient-checkpointing", action="store_true", default=True)
    p.add_argument("--seed", type=int, default=42)
    p.add_argument("--dry-run", action="store_true",
                   help="Print config, load adapter, verify pairs — do not train.")
    p.add_argument("--max-steps", type=int, default=-1,
                   help="Override: stop after N optimizer steps (smoke test). "
                        "-1 = full epochs (default).")
    p.add_argument("--limit-pairs", type=int, default=None,
                   help="Subsample dataset to N pairs (smoke test).")
    p.add_argument("--optim", default="paged_adamw_8bit",
                   help="HF Trainer optimizer. 'adamw_torch' is a safe fallback "
                        "when bitsandbytes has Blackwell driver issues.")
    p.add_argument("--precompute-ref", action="store_true",
                   help="Precompute reference log-probs for all pairs BEFORE "
                        "training. Avoids per-step ref forward passes "
                        "(memory win + sidesteps some Blackwell driver bugs).")
    return p.parse_args()


def validate_pairs(pairs_path: Path) -> int:
    if not pairs_path.exists():
        raise FileNotFoundError(f"pairs not found: {pairs_path}")
    n = 0
    with open(pairs_path) as f:
        for line in f:
            obj = json.loads(line)
            for k in ("prompt", "chosen", "rejected"):
                if k not in obj or not obj[k]:
                    raise ValueError(f"pair at line {n+1} missing {k}")
            n += 1
    if n == 0:
        raise ValueError(f"pairs file empty: {pairs_path}")
    return n


def main() -> int:
    args = parse_args()

    # Heavy imports here — fast --help and dry-run without CUDA
    import torch
    from datasets import load_dataset
    from transformers import AutoTokenizer, AutoModelForCausalLM, BitsAndBytesConfig
    from peft import PeftModel, LoraConfig
    from trl import DPOConfig, DPOTrainer

    os.environ.setdefault("TOKENIZERS_PARALLELISM", "false")

    print(f"[v4.2-dpo] base={args.base}")
    print(f"[v4.2-dpo] adapter-in={args.adapter_in}")
    print(f"[v4.2-dpo] pairs={args.pairs}")
    print(f"[v4.2-dpo] output={args.output_dir}")
    n_pairs = validate_pairs(args.pairs)
    print(f"[v4.2-dpo] validated {n_pairs} pairs")
    print(f"[v4.2-dpo] β={args.beta} λ_sft={args.lambda_sft} LR={args.lr} "
          f"epochs={args.epochs} batch={args.batch_size}x{args.grad_accum}")

    if args.dry_run:
        print("[v4.2-dpo] --dry-run set, exiting before training.")
        return 0

    # -------- Load base (4-bit) + v2.1 adapter --------
    bnb = BitsAndBytesConfig(
        load_in_4bit=True, bnb_4bit_quant_type="nf4",
        bnb_4bit_compute_dtype=torch.bfloat16, bnb_4bit_use_double_quant=True,
    )
    tok = AutoTokenizer.from_pretrained(str(args.adapter_in))
    if tok.pad_token is None:
        tok.pad_token = tok.eos_token

    print("[v4.2-dpo] loading base model (4-bit nf4, bf16 compute)...")
    base = AutoModelForCausalLM.from_pretrained(
        args.base, quantization_config=bnb, device_map="auto",
        torch_dtype=torch.bfloat16, attn_implementation="sdpa",
    )

    print("[v4.2-dpo] loading v2.1 adapter as starting weights...")
    model = PeftModel.from_pretrained(base, str(args.adapter_in),
                                      is_trainable=True)
    model.print_trainable_parameters()

    # -------- Dataset --------
    print(f"[v4.2-dpo] loading dataset from {args.pairs}...")
    ds = load_dataset("json", data_files=str(args.pairs), split="train")
    # TRL DPOTrainer expects columns "prompt", "chosen", "rejected"
    # Our JSONL has exactly these; meta is ignored.
    ds = ds.shuffle(seed=args.seed)
    if args.limit_pairs:
        ds = ds.select(range(min(args.limit_pairs, len(ds))))
    print(f"[v4.2-dpo] dataset: {len(ds)} pairs")

    # -------- DPO Config --------
    args.output_dir.mkdir(parents=True, exist_ok=True)
    # TRL 1.1.0: Reg-DPO expressed as multi-loss combination
    #   loss_type=['sigmoid','sft'] + loss_weights=[1.0, lambda_sft]
    # Older TRL used `rpo_alpha` + single loss_type='sigmoid'.
    cfg = DPOConfig(
        output_dir=str(args.output_dir),
        num_train_epochs=args.epochs,
        max_steps=args.max_steps,
        per_device_train_batch_size=args.batch_size,
        gradient_accumulation_steps=args.grad_accum,
        learning_rate=args.lr,
        lr_scheduler_type="cosine",
        warmup_ratio=args.warmup_ratio,
        max_length=args.max_length,
        beta=args.beta,
        loss_type=["sigmoid", "sft"],
        loss_weights=[1.0, args.lambda_sft],
        bf16=True,
        gradient_checkpointing=args.gradient_checkpointing,
        gradient_checkpointing_kwargs={"use_reentrant": False},
        logging_steps=5,
        save_strategy="epoch",
        save_total_limit=2,
        remove_unused_columns=False,
        seed=args.seed,
        optim=args.optim,
        precompute_ref_log_probs=args.precompute_ref,
        report_to="none",
    )

    # -------- Trainer --------
    # TRL 1.1.0: `tokenizer` renamed to `processing_class`.
    trainer = DPOTrainer(
        model=model,
        ref_model=None,   # PeftModel: reference uses base weights automatically
        args=cfg,
        train_dataset=ds,
        processing_class=tok,
    )

    print(f"[v4.2-dpo] starting training: {len(ds)} pairs, "
          f"effective batch {args.batch_size * args.grad_accum}, "
          f"{args.epochs} epochs")
    trainer.train()

    # -------- Save final adapter --------
    final_dir = args.output_dir / "final"
    final_dir.mkdir(parents=True, exist_ok=True)
    trainer.save_model(str(final_dir))
    tok.save_pretrained(str(final_dir))

    # Copy key metadata from v2.1 (chat_template.jinja, etc.)
    for f in ("chat_template.jinja",):
        src = args.adapter_in / f
        if src.exists():
            (final_dir / f).write_text(src.read_text())

    print(f"[v4.2-dpo] DONE. Adapter saved to {final_dir}")
    print("[v4.2-dpo] NEXT: run browser FSV via Playwright before HF push.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
