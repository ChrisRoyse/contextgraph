#!/usr/bin/env python3
"""Train Shakespeare LoRA v3.0 SFT-only on the combined v3 corpus.

Uses shakespeare_full_pipeline.phase2_sft() but points at
  - corpus: shakespeare_sft_v3_combined.jsonl (10,129 real-text pairs)
  - output: ~/.contextgraph/models/shakespeare_lora_v3/

Skips DPO entirely (per user's concern about synthetic-data collapse risk).
v2.1 checkpoints under ~/.contextgraph/models/shakespeare_lora/ are preserved.

Usage:
    python3 scripts/shakespeare_train_v3_sft.py
"""
from __future__ import annotations
import os
import sys
import time
from pathlib import Path

# Redirect the paths BEFORE importing phase2_sft so the module picks them up
import shakespeare_full_pipeline as pipe
HOME = Path.home()

V3_CORPUS = HOME / ".contextgraph/training/shakespeare_sft_v3_combined.jsonl"
V3_MODEL_DIR = HOME / ".contextgraph/models/shakespeare_lora_v3"

# Preflight: fail-fast if data missing or daemon is holding GPU VRAM
if not V3_CORPUS.is_file():
    print(f"[fatal] v3 corpus missing: {V3_CORPUS}", file=sys.stderr)
    sys.exit(1)

# Override the module-level MODEL_DIR and the hardcoded final_path inside phase2_sft.
# Simplest clean approach: monkey-patch MODEL_DIR + override TRAINING_DIR pathing
# by re-reading the specific filename we want.
pipe.MODEL_DIR = V3_MODEL_DIR
V3_MODEL_DIR.mkdir(parents=True, exist_ok=True)

# phase2_sft hardcodes `TRAINING_DIR / "shakespeare_sft_final_clean.jsonl"`.
# We shim it by symlinking our v3 corpus to that filename inside an override
# training dir — zero code modifications to phase2_sft required.
override_training = HOME / ".contextgraph/training_v3"
override_training.mkdir(parents=True, exist_ok=True)
shim = override_training / "shakespeare_sft_final_clean.jsonl"
if shim.exists() or shim.is_symlink():
    shim.unlink()
shim.symlink_to(V3_CORPUS)
pipe.TRAINING_DIR = override_training


def main():
    print("=" * 60)
    print("  SHAKESPEARE LoRA v3.0 — SFT ONLY (real-text expanded corpus)")
    print("=" * 60)
    print(f"  Corpus: {V3_CORPUS} (10,129 real Shakespeare pairs)")
    print(f"  Output: {V3_MODEL_DIR}")
    print(f"  Skipping DPO (per user decision on model-collapse risk)")
    print(f"  Base model: {pipe.BASE_MODEL}")
    print("=" * 60)

    # Preflight: ensure Context Graph daemon is NOT holding GPU VRAM.
    # Training needs ~22 GB; daemon uses ~17 GB; cannot coexist.
    import subprocess
    try:
        mem_used = int(subprocess.check_output(
            ["nvidia-smi", "--query-gpu=memory.used",
             "--format=csv,noheader,nounits"],
            text=True).strip().split("\n")[0])
    except Exception as e:
        print(f"[fatal] nvidia-smi failed: {e}", file=sys.stderr)
        sys.exit(1)

    if mem_used > 5000:
        print(f"[fatal] GPU has {mem_used} MiB in use — need <5000 MiB free. "
              f"Stop the daemon first (check: pgrep -f context-graph-mcp)",
              file=sys.stderr)
        sys.exit(1)
    print(f"[preflight] GPU: {mem_used} MiB used, OK to proceed")

    t0 = time.time()
    # phase2_sft loads data from TRAINING_DIR (our override) and writes to MODEL_DIR (override)
    model, tokenizer = pipe.phase2_sft(compositions=[])
    elapsed = time.time() - t0

    # Also save the final merged LoRA adapter to a clean "final" subdirectory
    final_dir = V3_MODEL_DIR / "final"
    final_dir.mkdir(parents=True, exist_ok=True)
    print(f"[save] saving final adapter to {final_dir}...")
    model.save_pretrained(str(final_dir))
    tokenizer.save_pretrained(str(final_dir))

    print("\n" + "=" * 60)
    print(f"  v3 SFT COMPLETE — {elapsed:.0f}s ({elapsed/60:.1f} min)")
    print(f"  Final adapter: {final_dir}")
    print("=" * 60)


if __name__ == "__main__":
    main()
