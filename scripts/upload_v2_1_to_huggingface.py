#!/usr/bin/env python3
"""Upload Shakespeare LoRA v2.1 to Hugging Face Hub as the main default.

v2.1 is the production-stable release:
  - SFT + DPO trained (7.5h total on RTX 5090)
  - 5/5 on original FSV
  - Verified robust in browser-based end-to-end testing
  - Works with default inference config (no specific system prompt match required)

Supersedes the v4.1 upload that was pushed first but found fragile in browser FSV.

Repo: cabdru/shakespeare-lora-gemma4

Usage:
    HF_TOKEN=hf_... python3 scripts/upload_v2_1_to_huggingface.py
"""
from __future__ import annotations
import os
import sys
from pathlib import Path

from huggingface_hub import HfApi, create_repo

HF_TOKEN = os.environ.get("HF_TOKEN", "")
REPO_ID = "cabdru/shakespeare-lora-gemma4"
LORA_DIR = Path(os.path.expanduser("~/.contextgraph/models/shakespeare_lora/final"))

REQUIRED_FILES = [
    "adapter_model.safetensors",
    "adapter_config.json",
    "tokenizer.json",
    "tokenizer_config.json",
    "chat_template.jinja",
    "README.md",
]

COMMIT_MESSAGE = (
    "v2.1: production release (SFT + DPO, robust config)\n\n"
    "Supersedes v4.1 which was found fragile in browser-based end-to-end\n"
    "testing. v2.1 is SFT + DPO trained (7.5h on RTX 5090), passed 5/5 on\n"
    "original FSV, and works with a wide range of serving configs (not\n"
    "dependent on exact system-prompt or rep-penalty match).\n\n"
    "v4.1 remains available in commit history for researchers who want the\n"
    "sharper code-gen behavior under exact config match."
)


def main() -> int:
    if not HF_TOKEN:
        sys.stderr.write("[fatal] HF_TOKEN env var not set\n")
        return 1

    if not LORA_DIR.is_dir():
        sys.stderr.write(f"[fatal] LoRA dir not found: {LORA_DIR}\n")
        return 1

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

    try:
        create_repo(REPO_ID, token=HF_TOKEN, exist_ok=True, repo_type="model")
        print(f"[upload] Repo exists: https://huggingface.co/{REPO_ID}")
    except Exception as e:
        sys.stderr.write(f"[fatal] create_repo failed: {e}\n")
        return 2

    print(f"[upload] Uploading {len(REQUIRED_FILES)} files — v2.1 overwrites v4.1 as main")
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
    print(f"  v2.1 uploaded as main to https://huggingface.co/{REPO_ID}")
    print("=" * 70)
    for f in REQUIRED_FILES:
        size_mb = (LORA_DIR / f).stat().st_size / 1024 / 1024
        print(f"    {f:40s} {size_mb:>8.2f} MB")
    print()
    print(f"  Load with:")
    print(f"    model = PeftModel.from_pretrained(base, '{REPO_ID}')")
    print()
    print(f"  v4.1 is still available — pin commit hash in HF for research use.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
