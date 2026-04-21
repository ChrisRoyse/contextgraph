#!/usr/bin/env python3
"""Train Shakespeare LoRA v4.0 SFT-only on the style-pure compositional corpus.

Uses shakespeare_full_pipeline.phase2_sft() but points at:
  - corpus: shakespeare_v4_sft_final.jsonl  (8,699 ACCEPTED style-pure pairs from Phase 2)
  - output: ~/.contextgraph/models/shakespeare_lora_v4/

Skips DPO entirely. v2.1 checkpoints at ~/.contextgraph/models/shakespeare_lora/
and v3 is already deleted.

Design basis: v2.1 (rank 256, alpha 512, 258 target modules) was the production
baseline (5/5 FSV). v3 regressed on modern topics because classical passages
diluted compositional signal from 59% → 46%. v4 inverts that: 100% compositional
(Gemma-knowledge in Shakespeare voice), no raw classical-only passages, filtered
through constellation quality gate (Sonnets 18ef2460 style_score ≥ 0.50).

Fail-fast: exits if corpus missing OR daemon still holds GPU VRAM.

Usage:
    python3 scripts/shakespeare_train_v4_sft.py
"""
from __future__ import annotations
import os
import subprocess
import sys
import time
from pathlib import Path

# Redirect module-level paths BEFORE importing phase2_sft so the module picks them up
sys.path.insert(0, str(Path(__file__).parent))
import shakespeare_full_pipeline as pipe

HOME = Path.home()

V4_CORPUS = HOME / ".contextgraph/training/shakespeare_v4_sft_final.jsonl"
V4_MODEL_DIR = HOME / ".contextgraph/models/shakespeare_lora_v4"

# ---------------------------------------------------------------------------
# Pre-flight
# ---------------------------------------------------------------------------
if not V4_CORPUS.is_file():
    sys.stderr.write(f"[fatal] v4 corpus missing: {V4_CORPUS}\n")
    sys.exit(1)

# Daemon must be DOWN — rank-256 training needs ~22 GB VRAM; daemon holds ~17 GB.
# Together they exceed the RTX 5090's 31.8 GB. Match against the compiled
# binary path so we don't false-positive on our own script source text.
my_pid = os.getpid()
daemon_check = subprocess.run(
    ["pgrep", "-fa", "target/release/context-graph-mcp"],
    capture_output=True, text=True,
)
daemon_procs = [
    line for line in daemon_check.stdout.splitlines()
    if line.strip() and not line.startswith(f"{my_pid} ")
    and "target/release/context-graph-mcp" in line
    and "python3" not in line  # exclude our own pgrep parent if nested
]
if daemon_procs:
    sys.stderr.write(
        "[fatal] context-graph daemon is still running. Kill it first:\n"
        "  pkill -f 'target/release/context-graph-mcp'\n"
        "Training needs ~22 GB VRAM, daemon holds ~17 GB, they collide.\n"
        "Active daemon process(es):\n"
    )
    for line in daemon_procs:
        sys.stderr.write(f"  {line}\n")
    sys.exit(1)

# ---------------------------------------------------------------------------
# Monkey-patch pipeline paths
# ---------------------------------------------------------------------------
pipe.MODEL_DIR = V4_MODEL_DIR
V4_MODEL_DIR.mkdir(parents=True, exist_ok=True)

# phase2_sft hardcodes `TRAINING_DIR / "shakespeare_sft_final_clean.jsonl"`.
# Create a directory with a symlink to our v4 corpus under that exact filename
# so phase2_sft picks it up unmodified.
override_training = HOME / ".contextgraph/training_v4"
override_training.mkdir(parents=True, exist_ok=True)
shim = override_training / "shakespeare_sft_final_clean.jsonl"
if shim.exists() or shim.is_symlink():
    shim.unlink()
shim.symlink_to(V4_CORPUS)
pipe.TRAINING_DIR = override_training


def main() -> int:
    print("=" * 68)
    print("  SHAKESPEARE LoRA v4.0 — rank-256 SFT on style-pure corpus")
    print("=" * 68)
    print(f"  Corpus:     {V4_CORPUS}  (8,699 ACCEPTED compositional pairs)")
    print(f"  Output:     {V4_MODEL_DIR}")
    print(f"  Base model: {pipe.BASE_MODEL}")
    print(f"  Config:     rank=256, alpha=512, target_modules=258, bf16, lr=2e-4")
    print(f"              effective batch=32 (1×32 accum), 3 epochs")
    print(f"  Skipping DPO phase (per v4 design decision)")
    print("=" * 68)

    t0 = time.time()
    # phase2_sft(compositions) takes a single historical arg it doesn't use;
    # loads its own base model + tokenizer + LoRA internally. Pass None.
    model, tokenizer = pipe.phase2_sft(None)

    print()
    print("=" * 68)
    print(f"  v4.0 SFT COMPLETE in {(time.time()-t0)/60:.1f} min")
    print(f"  Checkpoint: {V4_MODEL_DIR / 'sft_checkpoint'}")
    print()
    print("  Final step: save as 'final/' for inference server consumption")
    print("=" * 68)

    # phase2_sft saves to sft_checkpoint. We also want a 'final/' directory
    # for the inference server to consume.
    final_dir = V4_MODEL_DIR / "final"
    final_dir.mkdir(parents=True, exist_ok=True)
    model.save_pretrained(str(final_dir))
    tokenizer.save_pretrained(str(final_dir))
    print(f"[save] v4 LoRA written to {final_dir}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
