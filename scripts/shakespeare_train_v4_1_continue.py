#!/usr/bin/env python3
"""v4.1 continue-training — fix the Go-code regression from v4.

Based on Sherlock-Holmes research synthesis (April 2026 LoRA continue-training
best practices):
  - LR 5e-5 (4× reduction from v4's 2e-4) — preserves existing adapter alignment
  - 1 epoch — avoids overfit to small augmentation
  - 30% new / 70% replay mix — Nova Forge proven ratio vs catastrophic forgetting
  - Keep rank 256 / alpha 512 unchanged — we're continuing, not resetting
  - Dual validation: code-gap retention + original-capability retention

Input corpora:
  - NEW:     ~/.contextgraph/training/shakespeare_v4_augment_code.jsonl (~80 records)
  - REPLAY:  sampled from ~/.contextgraph/training/shakespeare_v4_sft_final.jsonl
             (~200 records stratified across 4 working categories)
  - MERGE:   ~/.contextgraph/training/shakespeare_v4_1_train.jsonl

Output:
  ~/.contextgraph/models/shakespeare_lora_v4_1/

Preserves: v4 adapter weights (loaded into PeftModel, continued from).
Fail-fast: daemon check, disk check, file-lock (all Sherlock-audited).

Usage:
    python3 scripts/shakespeare_train_v4_1_continue.py
"""
from __future__ import annotations
import fcntl
import json
import os
import random
import re
import shutil
import subprocess
import sys
import time
from pathlib import Path

HOME = Path.home()

V4_CORPUS_ACCEPTED = HOME / ".contextgraph/training/shakespeare_v4_sft_final.jsonl"
AUGMENT_CORPUS = HOME / ".contextgraph/training/shakespeare_v4_augment_code.jsonl"
V4_1_CORPUS = HOME / ".contextgraph/training/shakespeare_v4_1_train.jsonl"
V4_MODEL_DIR = HOME / ".contextgraph/models/shakespeare_lora_v4"
V4_1_MODEL_DIR = HOME / ".contextgraph/models/shakespeare_lora_v4_1"
BASE_MODEL = "google/gemma-4-E4B-it"

# Continue-training config
LEARNING_RATE = 5e-5      # 4x reduction from v4's 2e-4
NUM_EPOCHS = 2            # Small corpus needs more passes for meaningful step count
UPSAMPLE_NEW = 4          # Duplicate new records 4× so the rare code pattern is seen repeatedly
TARGET_TOTAL = 800        # ~25 optimizer steps/epoch × 2 epochs = 50 steps total
SEED = 42

# ---------------------------------------------------------------------------
# Preflight
# ---------------------------------------------------------------------------
def preflight() -> None:
    errors = []

    if not V4_CORPUS_ACCEPTED.is_file():
        errors.append(f"missing v4 corpus: {V4_CORPUS_ACCEPTED}")
    if not AUGMENT_CORPUS.is_file():
        errors.append(f"missing augmentation corpus: {AUGMENT_CORPUS}")
    if not (V4_MODEL_DIR / "final" / "adapter_model.safetensors").is_file():
        errors.append(f"missing v4 adapter to continue from: {V4_MODEL_DIR}/final/")

    # M3: disk check
    disk_free_gb = shutil.disk_usage(HOME).free / (1024**3)
    if disk_free_gb < 15.0:
        errors.append(f"disk too full: only {disk_free_gb:.1f} GB free (need ≥15)")

    # Daemon check
    my_pid = os.getpid()
    daemon_check = subprocess.run(
        ["pgrep", "-fa", "target/release/context-graph-mcp"],
        capture_output=True, text=True,
    )
    daemon_procs = [
        line for line in daemon_check.stdout.splitlines()
        if line.strip() and not line.startswith(f"{my_pid} ")
        and "target/release/context-graph-mcp" in line
        and "python3" not in line
    ]
    if daemon_procs:
        errors.append("context-graph daemon still running — kill it first (needs ~22 GB VRAM)")

    if errors:
        sys.stderr.write("\n[PREFLIGHT FAILED]\n")
        for e in errors:
            sys.stderr.write(f"  ✗ {e}\n")
        sys.exit(1)


# H2: file lock
def acquire_lock() -> "io.TextIOBase":
    lock_path = V4_1_MODEL_DIR.parent / ".train_v4_1.lock"
    lock_path.parent.mkdir(parents=True, exist_ok=True)
    lock_fh = open(lock_path, "w")
    try:
        fcntl.flock(lock_fh.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError:
        sys.stderr.write(f"[fatal] another v4.1 training running; lock held on {lock_path}\n")
        sys.exit(1)
    lock_fh.write(f"{os.getpid()}\n")
    lock_fh.flush()
    return lock_fh


# ---------------------------------------------------------------------------
# Build training corpus: new + replay mixed
# ---------------------------------------------------------------------------
def build_corpus() -> tuple[int, int]:
    """Stratified replay + new augmentation (upsampled) → single training file."""
    # Load new (augmentation) records
    new_records_raw = [json.loads(l) for l in AUGMENT_CORPUS.open()]
    n_new_unique = len(new_records_raw)

    # Upsample new records: with only 31 unique code records, each must appear
    # multiple times so the model has enough exposure during the short
    # continue-training run. Sherlock-3 noted: "quality over quantity" but
    # also requires enough optimizer steps for learning signal.
    new_records = new_records_raw * UPSAMPLE_NEW
    n_new = len(new_records)

    # Replay size: fill to TARGET_TOTAL
    n_replay = max(0, TARGET_TOTAL - n_new)

    # Sample replay: stratified across the 4 working categories
    # (skip foreign_lingual which had the filter bug; skip adversarial)
    working_cats = {"tech", "science", "daily_life", "philosophy",
                    "emotions", "storytelling", "classical",
                    "instructional", "conversational", "creative_abstract"}
    by_cat: dict[str, list] = {}
    with V4_CORPUS_ACCEPTED.open() as f:
        for line in f:
            r = json.loads(line)
            # v4_sft_final.jsonl records have {"messages": [...]} only,
            # NO category field. We need to join back from source data.
            pass

    # v4_sft_final.jsonl is messages-only. To stratify by category we'd need
    # the full v4_sft.jsonl which has category field. Load from there instead.
    full_source = HOME / ".contextgraph/training/shakespeare_v4_sft.jsonl"
    if not full_source.is_file():
        # Fallback: random uniform sample from v4_sft_final
        rng = random.Random(SEED)
        replay = rng.sample(new_records, 0)  # will be filled below
        all_recs = [json.loads(l) for l in V4_CORPUS_ACCEPTED.open()]
        replay = rng.sample(all_recs, min(n_replay, len(all_recs)))
    else:
        # Stratified sample
        rng = random.Random(SEED)
        by_cat = {}
        with full_source.open() as f:
            for line in f:
                r = json.loads(line)
                if r.get("status") != "ACCEPTED":
                    continue
                cat = r.get("category")
                if cat not in working_cats:
                    continue
                by_cat.setdefault(cat, []).append({"messages": r["messages"]})
        # Proportional sample per category, preserving v4's category distribution
        total_working = sum(len(v) for v in by_cat.values())
        replay = []
        for cat, recs in by_cat.items():
            take = max(1, round(n_replay * len(recs) / total_working))
            rng.shuffle(recs)
            replay.extend(recs[:take])
        # Trim to exact target
        rng.shuffle(replay)
        replay = replay[:n_replay]

    # Normalize new records: drop the "source" key if present
    new_clean = [{"messages": r["messages"]} for r in new_records]

    # Combine & shuffle
    combined = new_clean + replay
    rng = random.Random(SEED)
    rng.shuffle(combined)

    # Write
    V4_1_CORPUS.parent.mkdir(parents=True, exist_ok=True)
    with V4_1_CORPUS.open("w") as fo:
        for r in combined:
            fo.write(json.dumps(r, ensure_ascii=False) + "\n")

    return n_new, len(replay)


# ---------------------------------------------------------------------------
# Main training
# ---------------------------------------------------------------------------
def main() -> int:
    preflight()
    lock_fh = acquire_lock()

    print("=" * 70)
    print("  SHAKESPEARE LoRA v4.1 CONTINUE-TRAINING")
    print("=" * 70)
    print(f"  New corpus:       {AUGMENT_CORPUS}")
    print(f"  Replay source:    {V4_CORPUS_ACCEPTED}")
    print(f"  Merged corpus:    {V4_1_CORPUS}")
    print(f"  Output:           {V4_1_MODEL_DIR}")
    print(f"  Continues from:   {V4_MODEL_DIR}/final/ (v4 adapter weights)")
    print(f"  LR: {LEARNING_RATE}  epochs: {NUM_EPOCHS}  upsample_new: {UPSAMPLE_NEW}x  target_total: {TARGET_TOTAL}")
    print("=" * 70)

    # Stage 1: build corpus
    print("\n[build] Merging new code augmentation (upsampled) + replay sample...")
    n_new, n_replay = build_corpus()
    total = n_new + n_replay
    print(f"[build] new={n_new} (from {n_new // UPSAMPLE_NEW} unique × {UPSAMPLE_NEW}x)  replay={n_replay}  total={total}")
    print(f"[build] ratio: {n_new/total*100:.1f}% new / {n_replay/total*100:.1f}% replay")
    print(f"[build] optimizer steps: ~{total // 32 * NUM_EPOCHS} ({NUM_EPOCHS} epochs × {total // 32} steps/epoch)")

    V4_1_MODEL_DIR.mkdir(parents=True, exist_ok=True)

    # Stage 2: load base + v4 adapter, continue-train
    print("\n[load] Loading base Gemma-4-E4B + v4 LoRA adapter...")
    import torch
    from transformers import AutoModelForCausalLM, AutoTokenizer, BitsAndBytesConfig
    from peft import PeftModel, prepare_model_for_kbit_training
    from trl import SFTTrainer, SFTConfig
    from datasets import Dataset

    torch.cuda.empty_cache()
    t0 = time.time()

    bnb = BitsAndBytesConfig(
        load_in_4bit=True, bnb_4bit_quant_type="nf4",
        bnb_4bit_compute_dtype=torch.bfloat16, bnb_4bit_use_double_quant=True,
    )
    base = AutoModelForCausalLM.from_pretrained(
        BASE_MODEL, quantization_config=bnb, device_map="auto",
        torch_dtype=torch.bfloat16, attn_implementation="sdpa")
    base = prepare_model_for_kbit_training(base)

    # Freeze non-language modules (same as v4)
    for name, param in base.named_parameters():
        if "vision_tower" in name or "audio_tower" in name or "embed_vision" in name:
            param.requires_grad = False

    # Load v4 adapter as trainable PeftModel
    model = PeftModel.from_pretrained(
        base, str(V4_MODEL_DIR / "final"),
        is_trainable=True,  # enable gradients on adapter weights
    )
    model.gradient_checkpointing_enable()

    # Verify adapter has trainable params (guard against silent freezing)
    trainable = sum(p.numel() for p in model.parameters() if p.requires_grad)
    total_params = sum(p.numel() for p in model.parameters())
    print(f"[load] trainable params: {trainable:,} / {total_params:,} "
          f"({trainable/total_params*100:.2f}%)")
    if trainable < 1_000_000:
        raise RuntimeError(
            f"Only {trainable:,} trainable params — LoRA adapter appears frozen. "
            "Check PeftModel.from_pretrained(is_trainable=True)."
        )

    tokenizer = AutoTokenizer.from_pretrained(str(V4_MODEL_DIR / "final"))
    if tokenizer.pad_token is None:
        tokenizer.pad_token = tokenizer.eos_token

    vram = torch.cuda.memory_allocated() / 1e9
    print(f"[load] loaded in {time.time()-t0:.1f}s, VRAM {vram:.1f} GB")

    # Stage 3: dataset
    sft_data = [json.loads(l) for l in V4_1_CORPUS.open()]
    print(f"[data] {len(sft_data)} records loaded")

    def format_ex(ex):
        return {"text": tokenizer.apply_chat_template(
            ex["messages"], tokenize=False, add_generation_prompt=False)}

    dataset = Dataset.from_list(sft_data)
    dataset = dataset.map(format_ex, remove_columns=dataset.column_names)

    # Stage 4: SFTConfig — continuation hyperparams
    sft_output = V4_1_MODEL_DIR / "sft_checkpoint"
    sft_output.mkdir(parents=True, exist_ok=True)

    training_args = SFTConfig(
        output_dir=str(sft_output),
        per_device_train_batch_size=1,
        gradient_accumulation_steps=32,
        num_train_epochs=NUM_EPOCHS,
        learning_rate=LEARNING_RATE,
        lr_scheduler_type="cosine",
        warmup_ratio=0.03,  # shorter warmup per Sherlock-3 research
        weight_decay=0.01,
        logging_steps=5,
        save_steps=50,
        save_total_limit=2,
        fp16=False, bf16=True,
        optim="adamw_8bit",
        seed=SEED,
        max_length=640,
        dataset_text_field="text",
        packing=False,
        report_to="none",
        dataloader_num_workers=4,
        dataloader_pin_memory=True,
    )

    trainer = SFTTrainer(
        model=model, processing_class=tokenizer,
        train_dataset=dataset, args=training_args,
    )

    print(f"\n[train] Starting v4.1 continue-train: {len(sft_data)} records × {NUM_EPOCHS} epoch(s)")
    t_train = time.time()

    # H1: resume from checkpoint if crash occurred
    ckpts = sorted(
        sft_output.glob("checkpoint-*"),
        key=lambda p: int(re.search(r"checkpoint-(\d+)", p.name).group(1)) if re.search(r"checkpoint-(\d+)", p.name) else -1,
        reverse=True,
    )
    resume = str(ckpts[0]) if ckpts else None
    if resume:
        print(f"[train] RESUMING from {resume}")

    trainer.train(resume_from_checkpoint=resume)
    train_time = time.time() - t_train
    print(f"\n[train] DONE in {train_time:.0f}s ({train_time/60:.1f} min)")

    # Save final/
    final_dir = V4_1_MODEL_DIR / "final"
    final_dir.mkdir(parents=True, exist_ok=True)
    model.save_pretrained(str(final_dir))
    tokenizer.save_pretrained(str(final_dir))
    print(f"[save] v4.1 LoRA saved to {final_dir}")

    print("\n" + "=" * 70)
    print(f"  v4.1 CONTINUE-TRAINING COMPLETE")
    print("=" * 70)
    print(f"  Output:   {final_dir}")
    print(f"  Corpus:   {total} records ({n_new} new + {n_replay} replay)")
    print(f"  Wall time: {train_time/60:.1f} min")
    print()
    print("  Next: python3 scripts/shakespeare_v4_1_fsv.py")
    return 0


if __name__ == "__main__":
    sys.exit(main())
