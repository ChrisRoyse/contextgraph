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
import atexit
from datetime import datetime, timezone
import hashlib
import json
import os
import subprocess
import sys
import traceback
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from v4_2.cuda13_blackwell import (  # noqa: E402
    apply_preimport_cuda_env,
    configure_torch_cuda,
    release_torch_cuda,
)


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
    p.add_argument("--learner-pairs", type=Path, default=None,
                   help="Optional learner-conditioned DPO pairs JSONL from "
                        "build_learner_dpo_pairs.py. When provided, these are "
                        "concatenated with CAMD content pairs into one DPO loss.")
    p.add_argument("--learner-repeat", type=int, default=1,
                   help="Integer repeat count for learner pairs in the unified "
                        "DPO dataset. Must be >= 1 when --learner-pairs is set.")
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


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def path_state(path: Path) -> dict[str, Any]:
    if not path.exists():
        return {"path": str(path), "exists": False}
    if path.is_file():
        return {
            "path": str(path),
            "exists": True,
            "kind": "file",
            "size_bytes": path.stat().st_size,
            "sha256": sha256_file(path),
        }
    if path.is_dir():
        total = 0
        files = 0
        for root, _, names in os.walk(path):
            for name in names:
                files += 1
                total += (Path(root) / name).stat().st_size
        return {
            "path": str(path),
            "exists": True,
            "kind": "directory",
            "file_count": files,
            "size_bytes": total,
        }
    return {"path": str(path), "exists": True, "kind": "other"}


def write_json_atomic(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_text(json.dumps(payload, indent=2, sort_keys=True), encoding="utf-8")
    tmp.replace(path)


def command_state(command: list[str], *, timeout_sec: int = 10) -> dict[str, Any]:
    try:
        proc = subprocess.run(
            command,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=timeout_sec,
        )
        return {
            "command": command,
            "returncode": proc.returncode,
            "stdout_tail": proc.stdout.splitlines()[-80:],
            "stderr_tail": proc.stderr.splitlines()[-80:],
        }
    except Exception as exc:
        return {
            "command": command,
            "error": f"{type(exc).__name__}: {exc}",
        }


def torch_cuda_state(torch: Any) -> dict[str, Any]:
    state: dict[str, Any] = {
        "cuda_available": bool(torch.cuda.is_available()),
    }
    if not torch.cuda.is_available():
        return state
    device_idx = torch.cuda.current_device()
    free_bytes, total_bytes = torch.cuda.mem_get_info(device_idx)
    state.update({
        "device_index": device_idx,
        "device_name": torch.cuda.get_device_name(device_idx),
        "compute_capability": list(torch.cuda.get_device_capability(device_idx)),
        "allocated_gib": round(torch.cuda.memory_allocated(device_idx) / (1024 ** 3), 3),
        "reserved_gib": round(torch.cuda.memory_reserved(device_idx) / (1024 ** 3), 3),
        "max_allocated_gib": round(torch.cuda.max_memory_allocated(device_idx) / (1024 ** 3), 3),
        "free_mem_get_info_gib": round(free_bytes / (1024 ** 3), 3),
        "total_mem_get_info_gib": round(total_bytes / (1024 ** 3), 3),
    })
    return state


def write_failure_diagnostics(
    *,
    output_dir: Path,
    args: argparse.Namespace,
    exc: BaseException,
    torch: Any | None = None,
    extra: dict[str, Any] | None = None,
) -> Path:
    payload: dict[str, Any] = {
        "status": "failed",
        "created_at": datetime.now(timezone.utc).isoformat(timespec="seconds"),
        "error": f"{type(exc).__name__}: {exc}",
        "traceback": traceback.format_exc().splitlines(),
        "args": {k: str(v) if isinstance(v, Path) else v for k, v in vars(args).items()},
        "paths": {
            "adapter_in": path_state(args.adapter_in),
            "camd_pairs": path_state(args.pairs),
            "learner_pairs": path_state(args.learner_pairs) if args.learner_pairs else None,
            "output_dir": path_state(output_dir),
        },
        "nvidia_smi": command_state(["nvidia-smi"]),
    }
    if torch is not None:
        try:
            payload["torch_cuda_state"] = torch_cuda_state(torch)
        except Exception as state_exc:
            payload["torch_cuda_state_error"] = f"{type(state_exc).__name__}: {state_exc}"
    if extra:
        payload["extra"] = extra
    out = output_dir / "failure_diagnostics.json"
    write_json_atomic(out, payload)
    return out


def validate_pairs(pairs_path: Path, label: str,
                   required_meta_source: str | None = None) -> int:
    if not pairs_path.exists():
        raise FileNotFoundError(f"{label} pairs not found: {pairs_path}")
    n = 0
    with open(pairs_path) as f:
        for line in f:
            n += 1
            obj = json.loads(line)
            for k in ("prompt", "chosen", "rejected"):
                if k not in obj or not obj[k]:
                    raise ValueError(f"{label} pair at line {n} missing {k}")
            if obj["chosen"] == obj["rejected"]:
                raise ValueError(f"{label} pair at line {n} has identical chosen/rejected")
            if required_meta_source is not None:
                meta = obj.get("meta") or {}
                if meta.get("source") != required_meta_source:
                    raise ValueError(
                        f"{label} pair at line {n} meta.source must be "
                        f"{required_meta_source!r}"
                    )
    if n == 0:
        raise ValueError(f"{label} pairs file empty: {pairs_path}")
    return n


def load_pair_records(pairs_path: Path, label: str) -> list[dict]:
    """Load validated DPO pairs into memory without creating HF JSON caches."""
    records: list[dict] = []
    with open(pairs_path) as f:
        for line_no, line in enumerate(f, start=1):
            if not line.strip():
                continue
            obj = json.loads(line)
            for k in ("prompt", "chosen", "rejected"):
                if k not in obj or not obj[k]:
                    raise ValueError(f"{label} pair at line {line_no} missing {k}")
            records.append({
                "prompt": obj["prompt"],
                "chosen": obj["chosen"],
                "rejected": obj["rejected"],
            })
    if not records:
        raise ValueError(f"{label} pairs file empty: {pairs_path}")
    return records


def main() -> int:
    args = parse_args()

    apply_preimport_cuda_env()

    print(f"[v4.2-dpo] base={args.base}")
    print(f"[v4.2-dpo] adapter-in={args.adapter_in}")
    print(f"[v4.2-dpo] pairs={args.pairs}")
    if args.learner_pairs:
        print(f"[v4.2-dpo] learner-pairs={args.learner_pairs}")
    print(f"[v4.2-dpo] output={args.output_dir}")
    n_pairs = validate_pairs(args.pairs, "CAMD")
    n_learner_pairs = 0
    if args.learner_pairs:
        if args.learner_repeat < 1:
            raise ValueError("--learner-repeat must be >= 1")
        n_learner_pairs = validate_pairs(
            args.learner_pairs,
            "learner",
            required_meta_source="learning_event_outcome",
        )
    print(f"[v4.2-dpo] validated {n_pairs} CAMD pairs")
    if args.learner_pairs:
        print(f"[v4.2-dpo] validated {n_learner_pairs} learner pairs "
              f"(repeat={args.learner_repeat})")
    print(f"[v4.2-dpo] β={args.beta} λ_sft={args.lambda_sft} LR={args.lr} "
          f"epochs={args.epochs} batch={args.batch_size}x{args.grad_accum}")

    if args.dry_run:
        print("[v4.2-dpo] --dry-run set, exiting before training.")
        return 0

    # Heavy imports here — fast --help and dry-run without CUDA
    import torch
    from datasets import Dataset, concatenate_datasets
    from transformers import AutoTokenizer, AutoModelForCausalLM, BitsAndBytesConfig
    from peft import PeftModel
    from trl import DPOConfig, DPOTrainer

    cuda_info = configure_torch_cuda(torch, label="v4.2-camd-dpo")
    print(f"[v4.2-dpo] cuda={json.dumps(cuda_info, sort_keys=True)}")

    def release_cuda_cache() -> None:
        release_torch_cuda(torch)

    atexit.register(release_cuda_cache)

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
    print(f"[v4.2-dpo] loading CAMD dataset from {args.pairs}...")
    ds = Dataset.from_list(load_pair_records(args.pairs, "CAMD"))
    # TRL DPOTrainer expects columns "prompt", "chosen", "rejected"
    # Our JSONL has exactly these; meta is ignored.
    if args.learner_pairs:
        print(f"[v4.2-dpo] loading learner dataset from {args.learner_pairs}...")
        learner_ds = Dataset.from_list(load_pair_records(args.learner_pairs, "learner"))
        parts = [ds]
        for _ in range(args.learner_repeat):
            parts.append(learner_ds)
        ds = concatenate_datasets(parts)
        print(f"[v4.2-dpo] unified dataset = CAMD {n_pairs} + "
              f"learner {n_learner_pairs}x{args.learner_repeat} = {len(ds)} pairs")
    ds = ds.shuffle(seed=args.seed, keep_in_memory=True)
    if args.limit_pairs:
        ds = ds.select(range(min(args.limit_pairs, len(ds))), keep_in_memory=True)
    print(f"[v4.2-dpo] dataset: {len(ds)} pairs")

    # -------- DPO Config --------
    args.output_dir.mkdir(parents=True, exist_ok=True)
    manifest = {
        "status": "prepared",
        "created_at": datetime.now(timezone.utc).isoformat(timespec="seconds"),
        "base": args.base,
        "adapter_in": path_state(args.adapter_in),
        "camd_pairs": path_state(args.pairs),
        "learner_pairs": path_state(args.learner_pairs) if args.learner_pairs else None,
        "pair_counts": {
            "camd": n_pairs,
            "learner": n_learner_pairs,
            "learner_repeat": args.learner_repeat if args.learner_pairs else 0,
            "trainer_dataset_rows": len(ds),
        },
        "trainer": {
            "epochs": args.epochs,
            "max_steps": args.max_steps,
            "limit_pairs": args.limit_pairs,
            "batch_size": args.batch_size,
            "grad_accum": args.grad_accum,
            "effective_batch": args.batch_size * args.grad_accum,
            "lr": args.lr,
            "beta": args.beta,
            "lambda_sft": args.lambda_sft,
            "optim": args.optim,
            "precompute_ref": args.precompute_ref,
            "max_length": args.max_length,
            "max_prompt_length": args.max_prompt_length,
        },
        "cuda": cuda_info,
        "nvidia_smi_before_train": command_state(["nvidia-smi"]),
    }
    write_json_atomic(args.output_dir / "training_manifest.json", manifest)
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
    try:
        trainer.train()
    except Exception as exc:
        diagnostics_path = write_failure_diagnostics(
            output_dir=args.output_dir,
            args=args,
            exc=exc,
            torch=torch,
            extra={
                "stage": "trainer.train",
                "dataset_rows": len(ds),
                "cuda_info": cuda_info,
            },
        )
        print(f"[v4.2-dpo] FAILURE_DIAGNOSTICS={diagnostics_path}", file=sys.stderr)
        raise

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
    manifest["status"] = "ok"
    manifest["completed_at"] = datetime.now(timezone.utc).isoformat(timespec="seconds")
    manifest["final_adapter"] = path_state(final_dir / "adapter_model.safetensors")
    manifest["output_dir_after"] = path_state(args.output_dir)
    manifest["nvidia_smi_after_train"] = command_state(["nvidia-smi"])
    write_json_atomic(args.output_dir / "training_manifest.json", manifest)
    del trainer, model, base, tok, ds
    if "learner_ds" in locals():
        del learner_ds
    release_cuda_cache()
    print("[v4.2-dpo] CUDA/Python caches released.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
