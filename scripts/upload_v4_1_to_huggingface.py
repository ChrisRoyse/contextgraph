#!/usr/bin/env python3
"""Upload Shakespeare LoRA v4.1 to Hugging Face Hub.

Pushes:
  - adapter_model.safetensors (1.1 GB, rank 256)
  - adapter_config.json
  - tokenizer.json, tokenizer_config.json
  - chat_template.jinja
  - README.md (v4.1 model card — already written)

Repo: cabdru/shakespeare-lora-gemma4

v4.1 history:
  - v2.1 (2026-04-16): rank 256, SFT + DPO, 5/5 FSV
  - v3 (2026-04-18): regressed on modern topics, deleted
  - v4 (2026-04-20): 8,699 style-pure pairs, 7/10 enhanced FSV
  - v4.1 (2026-04-21): code-gen augmentation + continue-train, 10/10 enhanced FSV ← THIS

Usage:
    HF_TOKEN=hf_... python3 scripts/upload_v4_1_to_huggingface.py
"""
from __future__ import annotations
import os
import sys
from pathlib import Path

from huggingface_hub import HfApi, create_repo

HF_TOKEN = os.environ.get("HF_TOKEN", "")
REPO_ID = "cabdru/shakespeare-lora-gemma4"
LORA_DIR = Path(os.path.expanduser("~/.contextgraph/models/shakespeare_lora_v4_1/final"))

REQUIRED_FILES = [
    "adapter_model.safetensors",
    "adapter_config.json",
    "tokenizer.json",
    "tokenizer_config.json",
    "chat_template.jinja",
    "README.md",
]

COMMIT_MESSAGE = (
    "feat: v4.1 — code-gen regression fixed via continue-training\n\n"
    "10/10 on enhanced 10-prompt FSV (up from v4's 7/10).\n"
    "Continue-trained from v4 adapter on 31 Shakespeare-framed code "
    "exemplars (upsampled 4x) + 676 replay records. 50 optimizer "
    "steps, LR 5e-5, 11.7 min. Retention: 5/5 on love/quantum/admin/"
    "grief/Spanish. Code-gap: 5/5 on Python/Go/Rust/JS/SQL."
)


def main() -> int:
    if not HF_TOKEN:
        sys.stderr.write("[fatal] HF_TOKEN env var not set\n")
        return 1

    if not LORA_DIR.is_dir():
        sys.stderr.write(f"[fatal] LoRA dir not found: {LORA_DIR}\n")
        return 1

    # Verify all required files exist
    missing = []
    for f in REQUIRED_FILES:
        p = LORA_DIR / f
        if not p.is_file():
            missing.append(f)
    if missing:
        sys.stderr.write(f"[fatal] missing required files in {LORA_DIR}: {missing}\n")
        return 1

    print(f"[upload] Source: {LORA_DIR}")
    print(f"[upload] Target: {REPO_ID}")
    total_size = sum((LORA_DIR / f).stat().st_size for f in REQUIRED_FILES)
    print(f"[upload] Total size: {total_size/1024/1024:.1f} MB across {len(REQUIRED_FILES)} files")
    print()

    api = HfApi(token=HF_TOKEN)

    # Ensure repo exists (idempotent)
    try:
        create_repo(REPO_ID, token=HF_TOKEN, exist_ok=True, repo_type="model")
        print(f"[upload] Repo exists or created: https://huggingface.co/{REPO_ID}")
    except Exception as e:
        sys.stderr.write(f"[fatal] create_repo failed: {e}\n")
        return 2

    # Upload folder — only the required files (avoids accidentally pushing
    # any .partial or intermediate files that shouldn't be there)
    print(f"[upload] Uploading {len(REQUIRED_FILES)} files via upload_folder...")
    try:
        api.upload_folder(
            folder_path=str(LORA_DIR),
            repo_id=REPO_ID,
            repo_type="model",
            allow_patterns=REQUIRED_FILES,
            commit_message=COMMIT_MESSAGE,
        )
    except Exception as e:
        sys.stderr.write(f"[fatal] upload failed: {e}\n")
        return 3

    print()
    print("=" * 70)
    print(f"  ✓ v4.1 uploaded to https://huggingface.co/{REPO_ID}")
    print("=" * 70)
    print(f"  Files pushed:")
    for f in REQUIRED_FILES:
        size_mb = (LORA_DIR / f).stat().st_size / 1024 / 1024
        print(f"    {f:40s} {size_mb:>8.2f} MB")
    print()
    print(f"  Load with:")
    print(f"    model = PeftModel.from_pretrained(base, '{REPO_ID}')")
    return 0


if __name__ == "__main__":
    sys.exit(main())
