#!/usr/bin/env python3
"""Train Shakespeare LoRA v5 strict SFT from FSV-approved real text only.

This is the safe v5 training entrypoint. It intentionally does not call
scripts/shakespeare_full_pipeline.py because that older pipeline includes
generated compositions and DPO rows that fail the v5 Shakespeare-only audit.

Source of truth for a successful run:
  ~/.contextgraph/models/shakespeare_lora_v5_tct/runs/<run_id>/training_fsv.json
  ~/.contextgraph/models/shakespeare_lora_v5_tct/runs/<run_id>/final/
  ~/.contextgraph/models/shakespeare_lora_v5_tct/runs/<run_id>/generation_smoke.jsonl
"""
from __future__ import annotations

import argparse
import fcntl
import hashlib
import json
import os
import random
import re
import shutil
import subprocess
import sys
import time
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

import torch
from torch.utils.data import Dataset


HOME = Path.home()
BASE_MODEL = "google/gemma-4-E4B-it"
DATASET_PATH = HOME / ".contextgraph/training/shakespeare_v5_sft_tct.jsonl"
PARENT_ADAPTER = HOME / ".contextgraph/models/shakespeare_lora/final"
MODEL_DIR = HOME / ".contextgraph/models/shakespeare_lora_v5_tct"

MIN_FREE_VRAM_MIB = 26_000
MAX_COMPUTE_PROCESS_MIB = 2_048
MIN_DISK_FREE_GB = 25.0

ARCHAIC_RE = re.compile(
    r"\b(thee|thou|thy|thine|hast|dost|doth|hath|prithee|forsooth|"
    r"wherefore|methinks|verily|perchance|anon|betwixt|hither|thither|"
    r"whither|ere|oft|nay|aye|wilt|shalt|art)\b|(?:'tis|'twas|'twere|o'er)",
    re.IGNORECASE,
)

CONTAMINATION_PATTERNS: list[tuple[str, re.Pattern[str]]] = [
    ("act_scene_header", re.compile(r"(?m)^\s*(?:ACT|SCENE)\s+[IVXLC]+(?:\.|\b).*?$", re.IGNORECASE)),
    ("stage_enter_exit", re.compile(r"(?m)^\s*(?:Enter|Exit|Exeunt|Re-enter)\b.*?$", re.IGNORECASE)),
    ("bracket_stage_direction", re.compile(
        r"\[(?:_|Aside|Exit|Exeunt|Enter|Reads|Within|Kneeling|Drum|Flourish|Alarum|"
        r"Thunder|Music|Dies|To\s+[A-Z]).*?\]",
        re.IGNORECASE | re.DOTALL,
    )),
    ("speaker_label", re.compile(r"(?m)^\s*(?:\d+\s+)?[A-Z][A-Z0-9 ':-]{2,}\.[’'\"]?\s*$")),
    ("internal_project_note", re.compile(
        r"IMPL-PHASE-|SITREP|/home/cabdru/contextgraph|context-graph|Edge case:",
        re.IGNORECASE,
    )),
]

SYSTEM_PROMPT = (
    "You are William Shakespeare. Answer in Shakespeare's style while obeying "
    "the user's request. Do not write act headers, scene headers, speaker names, "
    "or stage directions."
)


def utc_now() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="seconds")


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def normalize_text(text: str) -> str:
    text = (text or "").lower()
    text = text.replace("’", "'").replace("“", '"').replace("”", '"')
    text = re.sub(r"[^a-z0-9']+", " ", text)
    return re.sub(r"\s+", " ", text).strip()


def assistant_text(record: dict[str, Any]) -> str:
    for msg in record.get("messages") or []:
        if isinstance(msg, dict) and msg.get("role") == "assistant":
            content = msg.get("content")
            return content if isinstance(content, str) else ""
    return ""


def contamination_reasons(text: str) -> list[str]:
    return [name for name, pattern in CONTAMINATION_PATTERNS if pattern.search(text or "")]


def read_jsonl(path: Path) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    with path.open("r", encoding="utf-8") as f:
        for line_no, line in enumerate(f, 1):
            stripped = line.strip()
            if not stripped:
                continue
            try:
                obj = json.loads(stripped)
            except Exception as exc:
                raise ValueError(f"{path}:{line_no}: JSON parse failed: {exc}") from exc
            if not isinstance(obj, dict):
                raise ValueError(f"{path}:{line_no}: JSON root is not an object")
            rows.append(obj)
    return rows


def run_command(cmd: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(cmd, capture_output=True, text=True, check=False)


def nvidia_state() -> dict[str, Any]:
    gpu = run_command([
        "nvidia-smi",
        "--query-gpu=index,name,memory.total,memory.used,memory.free,utilization.gpu",
        "--format=csv,noheader,nounits",
    ])
    apps = run_command([
        "nvidia-smi",
        "--query-compute-apps=pid,process_name,used_memory",
        "--format=csv,noheader,nounits",
    ])
    state: dict[str, Any] = {
        "gpu_returncode": gpu.returncode,
        "gpu_stdout": gpu.stdout.strip(),
        "gpu_stderr": gpu.stderr.strip(),
        "compute_returncode": apps.returncode,
        "compute_stdout": apps.stdout.strip(),
        "compute_stderr": apps.stderr.strip(),
        "free_mib": None,
        "compute_processes": [],
    }
    if gpu.returncode == 0 and gpu.stdout.strip():
        first = [part.strip() for part in gpu.stdout.strip().splitlines()[0].split(",")]
        if len(first) >= 6:
            state.update({
                "index": first[0],
                "name": first[1],
                "total_mib": int(first[2]),
                "used_mib": int(first[3]),
                "free_mib": int(first[4]),
                "utilization_gpu": int(first[5]),
            })
    if apps.returncode == 0 and apps.stdout.strip():
        for line in apps.stdout.strip().splitlines():
            parts = [part.strip() for part in line.split(",")]
            if len(parts) >= 3:
                try:
                    used_mib: int | None = int(parts[2])
                except ValueError:
                    used_mib = None
                state["compute_processes"].append({
                    "pid": parts[0],
                    "process_name": parts[1],
                    "used_mib": used_mib,
                    "used_mib_raw": parts[2],
                })
    return state


def acquire_lock() -> Any:
    MODEL_DIR.mkdir(parents=True, exist_ok=True)
    lock_path = MODEL_DIR / ".train_v5_strict_sft.lock"
    fh = lock_path.open("w")
    try:
        fcntl.flock(fh.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError as exc:
        raise RuntimeError(f"another v5 strict SFT run holds {lock_path}") from exc
    fh.write(f"{os.getpid()}\n")
    fh.flush()
    return fh


def validate_training_rows(rows: list[dict[str, Any]]) -> dict[str, Any]:
    assistant_digests: set[str] = set()
    contamination_hits = []
    missing = []
    archaic_missing = []
    for idx, row in enumerate(rows, 1):
        messages = row.get("messages")
        if not isinstance(messages, list):
            missing.append({"line": idx, "reason": "missing_messages"})
            continue
        text = assistant_text(row)
        if not text:
            missing.append({"line": idx, "reason": "missing_assistant"})
            continue
        digest = hashlib.sha256(normalize_text(text).encode("utf-8")).hexdigest()
        assistant_digests.add(digest)
        reasons = contamination_reasons(text)
        if reasons:
            contamination_hits.append({"line": idx, "reasons": reasons, "preview": text[:160]})
        if not ARCHAIC_RE.search(text):
            archaic_missing.append({"line": idx, "preview": text[:160]})
    return {
        "line_count": len(rows),
        "unique_assistant_targets": len(assistant_digests),
        "duplicate_assistant_targets": len(rows) - len(assistant_digests),
        "missing_count": len(missing),
        "contamination_count": len(contamination_hits),
        "archaic_missing_count": len(archaic_missing),
        "missing_examples": missing[:5],
        "contamination_examples": contamination_hits[:5],
        "archaic_missing_examples": archaic_missing[:5],
    }


@dataclass
class EncodedExample:
    input_ids: list[int]
    attention_mask: list[int]
    labels: list[int]


class MaskedSFTDataset(Dataset):
    def __init__(self, examples: list[EncodedExample]):
        self.examples = examples

    def __len__(self) -> int:
        return len(self.examples)

    def __getitem__(self, idx: int) -> dict[str, list[int]]:
        ex = self.examples[idx]
        return {
            "input_ids": ex.input_ids,
            "attention_mask": ex.attention_mask,
            "labels": ex.labels,
        }


class CausalCollator:
    def __init__(self, pad_token_id: int):
        self.pad_token_id = pad_token_id

    def __call__(self, features: list[dict[str, list[int]]]) -> dict[str, torch.Tensor]:
        max_len = max(len(f["input_ids"]) for f in features)
        batch = {"input_ids": [], "attention_mask": [], "labels": []}
        for feature in features:
            pad = max_len - len(feature["input_ids"])
            batch["input_ids"].append(feature["input_ids"] + [self.pad_token_id] * pad)
            batch["attention_mask"].append(feature["attention_mask"] + [0] * pad)
            batch["labels"].append(feature["labels"] + [-100] * pad)
        return {k: torch.tensor(v, dtype=torch.long) for k, v in batch.items()}


def encode_rows(tokenizer: Any, rows: list[dict[str, Any]], max_length: int) -> tuple[list[EncodedExample], dict[str, Any]]:
    encoded: list[EncodedExample] = []
    token_lengths: list[int] = []
    label_lengths: list[int] = []
    truncated = 0
    prefix_mismatch = 0
    first_mask_probe: dict[str, Any] = {}

    for idx, row in enumerate(rows, 1):
        messages = row["messages"]
        prompt_messages = []
        for msg in messages:
            if msg.get("role") == "assistant":
                break
            prompt_messages.append(msg)
        if not prompt_messages:
            raise ValueError(f"row {idx}: no prompt messages before assistant")

        prompt_text = tokenizer.apply_chat_template(
            prompt_messages,
            tokenize=False,
            add_generation_prompt=True,
        )
        full_text = tokenizer.apply_chat_template(
            messages,
            tokenize=False,
            add_generation_prompt=False,
        )
        if not full_text.startswith(prompt_text):
            prefix_mismatch += 1

        enc = tokenizer(
            full_text,
            add_special_tokens=False,
            truncation=True,
            max_length=max_length,
            return_offsets_mapping=True,
        )
        input_ids = enc["input_ids"]
        offsets = enc["offset_mapping"]
        labels = list(input_ids)
        prompt_chars = len(prompt_text)
        for pos, (start, _end) in enumerate(offsets):
            if start < prompt_chars:
                labels[pos] = -100
        label_count = sum(1 for label in labels if label != -100)
        if label_count == 0:
            raise ValueError(f"row {idx}: assistant target was fully masked or truncated")
        if len(input_ids) >= max_length:
            truncated += 1
        token_lengths.append(len(input_ids))
        label_lengths.append(label_count)
        if not first_mask_probe:
            first_label_ids = [tok for tok, lab in zip(input_ids, labels) if lab != -100][:30]
            first_mask_probe = {
                "row": idx,
                "input_tokens": len(input_ids),
                "label_tokens": label_count,
                "masked_tokens": len(input_ids) - label_count,
                "first_labeled_text": tokenizer.decode(first_label_ids),
            }
        encoded.append(EncodedExample(
            input_ids=input_ids,
            attention_mask=[1] * len(input_ids),
            labels=labels,
        ))

    token_lengths_sorted = sorted(token_lengths)
    label_lengths_sorted = sorted(label_lengths)
    stats = {
        "max_length": max_length,
        "rows": len(encoded),
        "truncated_rows": truncated,
        "prefix_mismatch_rows": prefix_mismatch,
        "token_min": token_lengths_sorted[0],
        "token_p50": token_lengths_sorted[len(token_lengths_sorted) // 2],
        "token_p90": token_lengths_sorted[int((len(token_lengths_sorted) - 1) * 0.90)],
        "token_p95": token_lengths_sorted[int((len(token_lengths_sorted) - 1) * 0.95)],
        "token_p99": token_lengths_sorted[int((len(token_lengths_sorted) - 1) * 0.99)],
        "token_max": token_lengths_sorted[-1],
        "label_min": label_lengths_sorted[0],
        "label_p50": label_lengths_sorted[len(label_lengths_sorted) // 2],
        "label_max": label_lengths_sorted[-1],
        "first_mask_probe": first_mask_probe,
    }
    return encoded, stats


def output_file_state(path: Path) -> dict[str, Any]:
    return {
        "path": str(path),
        "exists": path.exists(),
        "size_bytes": path.stat().st_size if path.exists() else 0,
        "sha256": sha256_file(path) if path.exists() and path.is_file() else "",
    }


def generate_smoke(model: Any, tokenizer: Any, out_path: Path) -> dict[str, Any]:
    model.eval()
    if hasattr(model, "gradient_checkpointing_disable"):
        model.gradient_checkpointing_disable()
    if hasattr(model, "config"):
        model.config.use_cache = True
    prompts = [
        "Explain WiFi in two short sentences.",
        "Write a birthday note for a friend.",
        "Tell me about artificial intelligence without using stage directions.",
    ]
    rows = []
    failures = 0
    for prompt in prompts:
        messages = [
            {"role": "system", "content": SYSTEM_PROMPT},
            {"role": "user", "content": prompt},
        ]
        text = tokenizer.apply_chat_template(messages, tokenize=False, add_generation_prompt=True)
        inputs = tokenizer(text, return_tensors="pt").to(model.device)
        with torch.no_grad():
            output_ids = model.generate(
                **inputs,
                max_new_tokens=90,
                do_sample=False,
                repetition_penalty=1.08,
                pad_token_id=tokenizer.pad_token_id,
                eos_token_id=tokenizer.eos_token_id,
            )
        generated_ids = output_ids[0][inputs["input_ids"].shape[1]:]
        output = tokenizer.decode(generated_ids, skip_special_tokens=True).strip()
        reasons = []
        reasons.extend(contamination_reasons(output))
        if not ARCHAIC_RE.search(output):
            reasons.append("no_archaic_marker")
        if not output:
            reasons.append("empty_output")
        if reasons:
            failures += 1
        rows.append({
            "prompt": prompt,
            "output": output,
            "reasons": reasons,
            "passed": not reasons,
        })
    with out_path.open("w", encoding="utf-8") as f:
        for row in rows:
            f.write(json.dumps(row, ensure_ascii=False, sort_keys=True) + "\n")
    return {
        "path": str(out_path),
        "line_count": len(rows),
        "failures": failures,
        "sha256": sha256_file(out_path),
    }


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--dataset", type=Path, default=DATASET_PATH)
    ap.add_argument("--parent-adapter", type=Path, default=PARENT_ADAPTER)
    ap.add_argument("--model-dir", type=Path, default=MODEL_DIR)
    ap.add_argument("--base-model", default=BASE_MODEL)
    ap.add_argument("--run-id", default=datetime.now(timezone.utc).strftime("%Y%m%d_%H%M%S"))
    ap.add_argument("--max-length", type=int, default=832)
    ap.add_argument("--epochs", type=float, default=1.0)
    ap.add_argument("--learning-rate", type=float, default=5e-6)
    ap.add_argument("--gradient-accumulation-steps", type=int, default=32)
    ap.add_argument("--seed", type=int, default=42)
    ap.add_argument("--skip-smoke-generation", action="store_true")
    args = ap.parse_args()

    random.seed(args.seed)
    torch.manual_seed(args.seed)
    torch.backends.cuda.matmul.allow_tf32 = True
    torch.backends.cudnn.allow_tf32 = True
    torch.set_float32_matmul_precision("high")

    args.model_dir.mkdir(parents=True, exist_ok=True)
    run_dir = args.model_dir / "runs" / args.run_id
    checkpoint_dir = run_dir / "checkpoints"
    final_dir = run_dir / "final"
    smoke_path = run_dir / "generation_smoke.jsonl"
    fsv_path = run_dir / "training_fsv.json"
    run_dir.mkdir(parents=True, exist_ok=True)

    fsv: dict[str, Any] = {
        "run_id": args.run_id,
        "started_at": utc_now(),
        "source_of_truth": {
            "training_fsv_json": str(fsv_path),
            "run_final_dir": str(final_dir),
            "latest_final_symlink": str(args.model_dir / "final"),
            "generation_smoke_jsonl": str(smoke_path),
        },
        "config": {
            "base_model": args.base_model,
            "parent_adapter": str(args.parent_adapter),
            "dataset": str(args.dataset),
            "max_length": args.max_length,
            "epochs": args.epochs,
            "learning_rate": args.learning_rate,
            "gradient_accumulation_steps": args.gradient_accumulation_steps,
            "seed": args.seed,
            "loss_policy": "assistant_tokens_only",
            "dpo_phase": "skipped_no_verified_dpo_candidates",
        },
        "preflight": {},
        "training": {},
        "post_state": {},
    }

    lock_fh = acquire_lock()
    try:
        print(f"SOURCE_OF_TRUTH_TRAINING_FSV={fsv_path}")
        print(f"SOURCE_OF_TRUTH_FINAL_DIR={final_dir}")
        print(f"SOURCE_OF_TRUTH_SMOKE={smoke_path}")

        # Kill idle Ollama before inspecting VRAM so it cannot grab GPU memory mid-run.
        run_command(["pkill", "-9", "ollama"])
        time.sleep(2)

        disk_free_gb = shutil.disk_usage(HOME).free / (1024 ** 3)
        gpu_before = nvidia_state()
        fsv["preflight"]["disk_free_gb"] = round(disk_free_gb, 2)
        fsv["preflight"]["gpu_before"] = gpu_before

        if disk_free_gb < MIN_DISK_FREE_GB:
            raise RuntimeError(f"only {disk_free_gb:.1f} GB free under {HOME}; need >= {MIN_DISK_FREE_GB:.1f} GB")
        if gpu_before.get("free_mib") is None:
            raise RuntimeError(f"nvidia-smi failed: {gpu_before}")
        if int(gpu_before["free_mib"]) < MIN_FREE_VRAM_MIB:
            raise RuntimeError(f"only {gpu_before['free_mib']} MiB VRAM free; need >= {MIN_FREE_VRAM_MIB} MiB")
        big_apps = [
            proc for proc in gpu_before["compute_processes"]
            if proc["used_mib"] is not None and int(proc["used_mib"]) > MAX_COMPUTE_PROCESS_MIB
        ]
        if big_apps:
            raise RuntimeError(f"compute processes exceed {MAX_COMPUTE_PROCESS_MIB} MiB: {big_apps}")

        if not args.dataset.is_file():
            raise RuntimeError(f"dataset missing: {args.dataset}")
        if not (args.parent_adapter / "adapter_model.safetensors").is_file():
            raise RuntimeError(f"parent adapter missing: {args.parent_adapter}")

        rows = read_jsonl(args.dataset)
        data_audit = validate_training_rows(rows)
        fsv["preflight"]["dataset"] = {
            "path": str(args.dataset),
            "resolved_path": str(args.dataset.resolve()),
            "sha256": sha256_file(args.dataset),
            **data_audit,
        }
        fsv["preflight"]["parent_adapter"] = {
            "adapter_model": output_file_state(args.parent_adapter / "adapter_model.safetensors"),
            "adapter_config": output_file_state(args.parent_adapter / "adapter_config.json"),
        }
        if data_audit["line_count"] == 0:
            raise RuntimeError("dataset has zero rows")
        if data_audit["duplicate_assistant_targets"] != 0:
            raise RuntimeError(f"dataset still has duplicate assistant targets: {data_audit}")
        if data_audit["contamination_count"] != 0:
            raise RuntimeError(f"dataset contamination found: {data_audit['contamination_examples']}")
        if data_audit["missing_count"] != 0:
            raise RuntimeError(f"dataset schema problems found: {data_audit['missing_examples']}")

        from transformers import (
            AutoModelForCausalLM,
            AutoTokenizer,
            BitsAndBytesConfig,
            Trainer,
            TrainingArguments,
        )
        from peft import PeftModel, prepare_model_for_kbit_training

        tokenizer = AutoTokenizer.from_pretrained(str(args.parent_adapter))
        if tokenizer.pad_token is None:
            tokenizer.pad_token = tokenizer.eos_token
        if tokenizer.pad_token_id is None:
            raise RuntimeError("tokenizer has no pad_token_id")

        encoded, token_stats = encode_rows(tokenizer, rows, args.max_length)
        fsv["preflight"]["tokenization"] = token_stats
        if token_stats["truncated_rows"] != 0:
            raise RuntimeError(f"tokenization would truncate rows: {token_stats}")
        if token_stats["prefix_mismatch_rows"] != 0:
            raise RuntimeError(f"chat-template prefix mismatch: {token_stats}")

        print("[load] loading base model in 4-bit NF4")
        bnb = BitsAndBytesConfig(
            load_in_4bit=True,
            bnb_4bit_quant_type="nf4",
            bnb_4bit_compute_dtype=torch.bfloat16,
            bnb_4bit_use_double_quant=True,
        )
        model = AutoModelForCausalLM.from_pretrained(
            args.base_model,
            quantization_config=bnb,
            device_map="auto",
            torch_dtype=torch.bfloat16,
            attn_implementation="sdpa",
        )
        model.config.use_cache = False
        model = prepare_model_for_kbit_training(model, use_gradient_checkpointing=True)

        print(f"[load] continuing trainable adapter from {args.parent_adapter}")
        model = PeftModel.from_pretrained(model, str(args.parent_adapter), is_trainable=True)
        model.gradient_checkpointing_enable()
        trainable = sum(p.numel() for p in model.parameters() if p.requires_grad)
        total = sum(p.numel() for p in model.parameters())
        fsv["preflight"]["trainable_parameters"] = {
            "trainable": trainable,
            "total": total,
            "percent": round(trainable / total * 100, 4) if total else 0.0,
        }

        dataset = MaskedSFTDataset(encoded)
        collator = CausalCollator(tokenizer.pad_token_id)
        checkpoint_dir.mkdir(parents=True, exist_ok=True)

        from transformers.trainer_utils import get_last_checkpoint

        resume_ckpt = get_last_checkpoint(str(checkpoint_dir)) if checkpoint_dir.exists() else None
        fsv["preflight"]["resume_checkpoint"] = resume_ckpt

        training_args = TrainingArguments(
            output_dir=str(checkpoint_dir),
            per_device_train_batch_size=1,
            gradient_accumulation_steps=args.gradient_accumulation_steps,
            num_train_epochs=args.epochs,
            learning_rate=args.learning_rate,
            lr_scheduler_type="cosine",
            warmup_ratio=0.03,
            weight_decay=0.0,
            logging_steps=1,
            save_steps=10,
            save_total_limit=2,
            bf16=True,
            fp16=False,
            optim="adamw_8bit",
            seed=args.seed,
            data_seed=args.seed,
            max_grad_norm=0.3,
            report_to="none",
            remove_unused_columns=False,
            dataloader_num_workers=0,
            dataloader_pin_memory=True,
        )
        trainer = Trainer(
            model=model,
            args=training_args,
            train_dataset=dataset,
            data_collator=collator,
            processing_class=tokenizer,
        )

        gpu_after_setup = nvidia_state()
        fsv["preflight"]["gpu_after_setup"] = gpu_after_setup
        print(f"[train] examples={len(dataset)} epochs={args.epochs} lr={args.learning_rate} max_length={args.max_length}")
        print(f"[train] token stats={json.dumps(token_stats, sort_keys=True)}")
        started = time.time()
        train_result = trainer.train(resume_from_checkpoint=resume_ckpt)
        elapsed = time.time() - started
        fsv["training"] = {
            "elapsed_seconds": round(elapsed, 2),
            "metrics": train_result.metrics,
            "trainer_state_global_step": trainer.state.global_step,
        }

        print(f"[save] writing final adapter to {final_dir}")
        final_dir.mkdir(parents=True, exist_ok=True)
        model.save_pretrained(str(final_dir))
        tokenizer.save_pretrained(str(final_dir))

        if not args.skip_smoke_generation:
            print("[smoke] generating post-training behavior probes")
            fsv["post_state"]["generation_smoke"] = generate_smoke(model, tokenizer, smoke_path)

        latest = args.model_dir / "final"
        if latest.exists() or latest.is_symlink():
            if latest.is_symlink() or latest.is_file():
                latest.unlink()
            else:
                raise RuntimeError(f"refusing to replace existing non-symlink directory: {latest}")
        latest.symlink_to(final_dir)

        fsv["post_state"]["gpu_after_training"] = nvidia_state()
        fsv["post_state"]["final_files"] = {
            "adapter_model": output_file_state(final_dir / "adapter_model.safetensors"),
            "adapter_config": output_file_state(final_dir / "adapter_config.json"),
            "tokenizer_config": output_file_state(final_dir / "tokenizer_config.json"),
            "tokenizer_json": output_file_state(final_dir / "tokenizer.json"),
            "latest_symlink_resolves_to": str(latest.resolve()) if latest.exists() else "",
        }
        fsv["completed_at"] = utc_now()
        fsv["status"] = "PASS"

        required = fsv["post_state"]["final_files"]
        if not required["adapter_model"]["exists"] or required["adapter_model"]["size_bytes"] <= 0:
            raise RuntimeError("adapter_model.safetensors was not written")
        if fsv["post_state"].get("generation_smoke", {}).get("failures", 0):
            raise RuntimeError(f"generation smoke failed: {fsv['post_state']['generation_smoke']}")

        fsv_path.write_text(json.dumps(fsv, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        print(f"TRAINING_FSV_WRITTEN={fsv_path}")
        print(f"FINAL_MODEL_DIR={final_dir}")
        print(f"LATEST_FINAL={latest}")
        return 0
    except Exception as exc:
        fsv["status"] = "FAIL"
        fsv["failed_at"] = utc_now()
        fsv["error"] = f"{type(exc).__name__}: {exc}"
        fsv_path.write_text(json.dumps(fsv, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        print(f"TRAINING_FSV_WRITTEN={fsv_path}", file=sys.stderr)
        print(f"[fatal] {type(exc).__name__}: {exc}", file=sys.stderr)
        return 1
    finally:
        lock_fh.close()


if __name__ == "__main__":
    sys.exit(main())
