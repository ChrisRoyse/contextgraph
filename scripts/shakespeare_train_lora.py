#!/usr/bin/env python3
"""Train a Shakespeare LoRA on Gemma4-8B using the SFT + DPO datasets.

Phase 1: SFT on ~1000 Shakespeare text examples
Phase 2: DPO on ~12,500 contrastive pairs (chosen=right Shakespeare, rejected=wrong Shakespeare)

Uses unsloth for 2× training speed on RTX 5090.

Usage:
    python3 scripts/shakespeare_train_lora.py \
        --sft-data ~/.contextgraph/training/shakespeare_sft.jsonl \
        --dpo-data ~/.contextgraph/training/shakespeare_dpo.jsonl \
        --output ~/.contextgraph/models/shakespeare_lora \
        --phases sft+dpo
"""
from __future__ import annotations

import argparse
import json
import os
import sys
import time
from pathlib import Path

os.environ["HF_HUB_OFFLINE"] = "0"  # Allow download for base model


def load_jsonl(path: str, max_rows: int | None = None) -> list[dict]:
    data = []
    with open(path) as f:
        for line in f:
            if max_rows and len(data) >= max_rows:
                break
            data.append(json.loads(line))
    return data


def run_sft(args):
    """Phase 1: Supervised Fine-Tuning on Shakespeare text."""
    print("\n" + "=" * 60)
    print("  PHASE 1: SFT — Teaching the model Shakespeare's voice")
    print("=" * 60)

    from transformers import AutoModelForCausalLM, AutoTokenizer, BitsAndBytesConfig
    from peft import LoraConfig, get_peft_model, prepare_model_for_kbit_training
    from trl import SFTTrainer, SFTConfig
    from datasets import Dataset
    import torch

    # Load model with 4-bit quantization
    print(f"[train] Loading base model: {args.base_model}")
    bnb_config = BitsAndBytesConfig(
        load_in_4bit=True,
        bnb_4bit_quant_type="nf4",
        bnb_4bit_compute_dtype=torch.bfloat16,
        bnb_4bit_use_double_quant=True,
    )
    model = AutoModelForCausalLM.from_pretrained(
        args.base_model,
        quantization_config=bnb_config,
        device_map="auto",
        torch_dtype=torch.bfloat16,
        attn_implementation="sdpa",
    )
    tokenizer = AutoTokenizer.from_pretrained(args.base_model)
    if tokenizer.pad_token is None:
        tokenizer.pad_token = tokenizer.eos_token
    model = prepare_model_for_kbit_training(model)

    # Add LoRA adapters
    print(f"[train] Adding LoRA: rank={args.lora_rank}, alpha={args.lora_alpha}")
    # Gemma 4 is multimodal — vision_tower and audio_tower use
    # Gemma4ClippableLinear which PEFT can't handle. We freeze them entirely
    # before applying LoRA so PEFT never sees them.
    for name, param in model.named_parameters():
        if "vision_tower" in name or "audio_tower" in name or "embed_vision" in name:
            param.requires_grad = False

    # Build explicit target list: only language_model layer modules
    target_names = []
    for name, mod in model.named_modules():
        if "language_model.layers." in name and name.endswith(
            ("q_proj", "k_proj", "v_proj", "o_proj", "gate_proj", "up_proj", "down_proj")
        ):
            target_names.append(name)

    if not target_names:
        raise RuntimeError("No language_model target modules found — check model architecture")
    print(f"[train] Found {len(target_names)} LoRA target modules in language_model")

    lora_config = LoraConfig(
        r=args.lora_rank,
        lora_alpha=args.lora_alpha,
        target_modules=target_names,
        lora_dropout=0.05,
        bias="none",
        task_type="CAUSAL_LM",
    )
    model = get_peft_model(model, lora_config)
    model.gradient_checkpointing_enable()

    # Load SFT data
    print(f"[train] Loading SFT data from {args.sft_data}")
    raw = load_jsonl(args.sft_data, max_rows=args.max_sft_rows)
    print(f"[train] SFT examples: {len(raw)}")

    # Format for the chat template
    def format_example(ex):
        messages = ex["messages"]
        text = tokenizer.apply_chat_template(messages, tokenize=False, add_generation_prompt=False)
        return {"text": text}

    dataset = Dataset.from_list(raw)
    dataset = dataset.map(format_example, remove_columns=dataset.column_names)
    print(f"[train] Formatted dataset: {len(dataset)} examples")
    print(f"[train] Sample (first 200 chars): {dataset[0]['text'][:200]}")

    # Training config
    sft_output = Path(args.output) / "sft_checkpoint"
    sft_output.mkdir(parents=True, exist_ok=True)

    training_args = SFTConfig(
        output_dir=str(sft_output),
        per_device_train_batch_size=args.batch_size,
        gradient_accumulation_steps=args.grad_accum,
        num_train_epochs=args.sft_epochs,
        learning_rate=args.sft_lr,
        lr_scheduler_type="cosine",
        warmup_ratio=0.1,
        weight_decay=0.01,
        logging_steps=10,
        save_steps=200,
        save_total_limit=2,
        fp16=False,
        bf16=True,
        optim="adamw_8bit",
        seed=42,
        max_length=args.max_seq_len,
        dataset_text_field="text",
        packing=False,  # Disabled — needs flash_attention for safe packing
        report_to="none",
    )

    trainer = SFTTrainer(
        model=model,
        processing_class=tokenizer,
        train_dataset=dataset,
        args=training_args,
    )

    print(f"[train] Starting SFT training ({args.sft_epochs} epochs, lr={args.sft_lr})")
    t0 = time.time()
    trainer.train()
    sft_time = time.time() - t0
    print(f"[train] SFT complete in {sft_time:.0f}s ({sft_time/60:.1f} min)")

    # Save SFT checkpoint
    model.save_pretrained(str(sft_output))
    tokenizer.save_pretrained(str(sft_output))
    print(f"[train] SFT checkpoint saved to {sft_output}")

    return model, tokenizer


def run_dpo(args, model=None, tokenizer=None):
    """Phase 2: DPO on contrastive pairs."""
    print("\n" + "=" * 60)
    print("  PHASE 2: DPO — Teaching the model which Shakespeare fits where")
    print("=" * 60)

    from trl import DPOTrainer, DPOConfig
    from datasets import Dataset

    if model is None:
        from transformers import AutoModelForCausalLM, AutoTokenizer, BitsAndBytesConfig
        from peft import PeftModel
        import torch
        sft_path = Path(args.output) / "sft_checkpoint"
        print(f"[train] Loading SFT checkpoint from {sft_path}")
        bnb_config = BitsAndBytesConfig(
            load_in_4bit=True,
            bnb_4bit_quant_type="nf4",
            bnb_4bit_compute_dtype=torch.bfloat16,
            bnb_4bit_use_double_quant=True,
        )
        base = AutoModelForCausalLM.from_pretrained(
            args.base_model,
            quantization_config=bnb_config,
            device_map="auto",
            torch_dtype=torch.bfloat16,
        )
        model = PeftModel.from_pretrained(base, str(sft_path), is_trainable=True)
        tokenizer = AutoTokenizer.from_pretrained(str(sft_path))
        if tokenizer.pad_token is None:
            tokenizer.pad_token = tokenizer.eos_token

    # Load DPO data
    print(f"[train] Loading DPO data from {args.dpo_data}")
    raw = load_jsonl(args.dpo_data, max_rows=args.max_dpo_rows)
    print(f"[train] DPO pairs: {len(raw)}")

    # Format for DPO — need prompt, chosen, rejected as plain text
    def format_dpo(ex):
        prompt_msgs = ex["prompt"]
        chosen_msgs = ex["chosen"]
        rejected_msgs = ex["rejected"]

        # Combine prompt into text
        prompt_text = tokenizer.apply_chat_template(
            prompt_msgs, tokenize=False, add_generation_prompt=True
        )
        chosen_text = chosen_msgs[0]["content"]
        rejected_text = rejected_msgs[0]["content"]

        return {
            "prompt": prompt_text,
            "chosen": chosen_text,
            "rejected": rejected_text,
        }

    dataset = Dataset.from_list(raw)
    dataset = dataset.map(format_dpo, remove_columns=dataset.column_names)
    print(f"[train] Formatted DPO dataset: {len(dataset)} pairs")

    dpo_output = Path(args.output) / "dpo_checkpoint"
    dpo_output.mkdir(parents=True, exist_ok=True)

    training_args = DPOConfig(
        output_dir=str(dpo_output),
        per_device_train_batch_size=1,  # DPO needs 2× memory — keep batch=1
        gradient_accumulation_steps=32,  # Effective batch of 32
        num_train_epochs=args.dpo_epochs,
        learning_rate=args.dpo_lr,
        lr_scheduler_type="cosine",
        warmup_ratio=0.1,
        weight_decay=0.01,
        beta=0.1,  # DPO temperature
        logging_steps=10,
        save_steps=1000,
        save_total_limit=2,
        fp16=False,
        bf16=True,
        optim="adamw_8bit",
        seed=42,
        max_length=1024,  # Shorter than SFT to fit DPO reference in VRAM
        max_prompt_length=512,  # Truncate long prompts
        report_to="none",
        remove_unused_columns=False,
    )

    trainer = DPOTrainer(
        model=model,
        ref_model=None,  # Use implicit reference (saves 50% VRAM)
        processing_class=tokenizer,
        train_dataset=dataset,
        args=training_args,
    )

    print(f"[train] Starting DPO training ({args.dpo_epochs} epochs, lr={args.dpo_lr}, beta=0.1)")
    t0 = time.time()
    trainer.train()
    dpo_time = time.time() - t0
    print(f"[train] DPO complete in {dpo_time:.0f}s ({dpo_time/60:.1f} min)")

    # Save final model
    final_path = Path(args.output) / "final"
    final_path.mkdir(parents=True, exist_ok=True)
    model.save_pretrained(str(final_path))
    tokenizer.save_pretrained(str(final_path))
    print(f"[train] Final LoRA saved to {final_path}")

    return model, tokenizer


def export_gguf(args):
    """Export the LoRA to GGUF format for Ollama."""
    print("\n" + "=" * 60)
    print("  EXPORT: Converting LoRA to GGUF for Ollama")
    print("=" * 60)

    final_path = Path(args.output) / "final"
    gguf_path = Path(args.output) / "shakespeare-lora.gguf"

    print(f"[export] GGUF export requires llama.cpp convert tools.", file=sys.stderr)
    print(f"[export] The HF-format LoRA at {final_path} can be used with transformers directly.", file=sys.stderr)
    print(f"[export] To create an Ollama-compatible model, run:", file=sys.stderr)
    print(f"[export]   ollama create shakespeare --file Modelfile", file=sys.stderr)
    print(f"[export] Where Modelfile contains: FROM gemma4:e4b\\nADAPTER {final_path}", file=sys.stderr)


def main():
    ap = argparse.ArgumentParser(description="Train Shakespeare LoRA on Gemma4-8B")
    ap.add_argument("--base-model", type=str, default="google/gemma-4-E4B-it",
                    help="Base model (HuggingFace ID or path)")
    ap.add_argument("--sft-data", type=str,
                    default=os.path.expanduser("~/.contextgraph/training/shakespeare_sft_optimal.jsonl"))
    ap.add_argument("--dpo-data", type=str,
                    default=os.path.expanduser("~/.contextgraph/training/shakespeare_dpo.jsonl"))
    ap.add_argument("--output", type=str,
                    default=os.path.expanduser("~/.contextgraph/models/shakespeare_lora"))
    ap.add_argument("--phases", type=str, default="sft+dpo",
                    choices=["sft", "dpo", "sft+dpo", "sft+dpo+gguf"],
                    help="Which training phases to run")
    ap.add_argument("--lora-rank", type=int, default=256)
    ap.add_argument("--lora-alpha", type=int, default=512)
    ap.add_argument("--max-seq-len", type=int, default=1024)
    ap.add_argument("--batch-size", type=int, default=2)
    ap.add_argument("--grad-accum", type=int, default=16)
    ap.add_argument("--sft-epochs", type=int, default=3)
    ap.add_argument("--sft-lr", type=float, default=2e-4)
    ap.add_argument("--dpo-epochs", type=int, default=1)
    ap.add_argument("--dpo-lr", type=float, default=5e-5)
    ap.add_argument("--max-sft-rows", type=int, default=None)
    ap.add_argument("--max-dpo-rows", type=int, default=None)
    args = ap.parse_args()

    Path(args.output).mkdir(parents=True, exist_ok=True)

    phases = args.phases.split("+")
    model, tokenizer = None, None

    if "sft" in phases:
        model, tokenizer = run_sft(args)

    if "dpo" in phases:
        model, tokenizer = run_dpo(args, model, tokenizer)

    if "gguf" in phases:
        export_gguf(args)

    print("\n" + "=" * 60)
    print("  TRAINING COMPLETE")
    print("=" * 60)
    print(f"  Output: {args.output}")
    print(f"  Phases: {args.phases}")
    if (Path(args.output) / "final").exists():
        size_mb = sum(f.stat().st_size for f in (Path(args.output) / "final").rglob("*")) / 1e6
        print(f"  Final LoRA size: {size_mb:.1f} MB")
    print("=" * 60)


if __name__ == "__main__":
    main()
