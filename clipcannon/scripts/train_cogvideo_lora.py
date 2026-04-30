#!/usr/bin/env python3
"""LoRA fine-tuning for CogVideoX-2b on Santa's video data.

Locks the model's identity to the specific Santa from the training video.
After training, the model can ONLY produce this Santa's face and mannerisms.

Usage:
    python scripts/train_cogvideo_lora.py --steps 1000
    python scripts/train_cogvideo_lora.py --steps 2000 --rank 64 --lr 1e-4

VRAM budget (RTX 5090 32GB):
    Model (BF16):     ~4GB
    LoRA weights:     ~200MB
    Optimizer states: ~400MB
    Activations:      ~8GB (with gradient checkpointing)
    Total:            ~13GB (19GB headroom)
"""
from __future__ import annotations

import gc
import json
import logging
import os
import random
import sys
import time
from pathlib import Path

os.environ["PYTORCH_CUDA_ALLOC_CONF"] = (
    "expandable_segments:True,max_split_size_mb:256,garbage_collection_threshold:0.6"
)
os.environ["HF_HUB_OFFLINE"] = "1"
os.environ["TRANSFORMERS_OFFLINE"] = "1"

sys.stdout.reconfigure(line_buffering=True)
sys.stderr.reconfigure(line_buffering=True)

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(name)s: %(message)s",
    handlers=[
        logging.StreamHandler(sys.stdout),
        logging.FileHandler("/tmp/cogvideo_lora_training.log"),
    ],
)
logger = logging.getLogger("lora_train")

import cv2
import numpy as np
import torch
import torch.nn.functional as F
from torch.utils.data import Dataset, DataLoader

torch.backends.cuda.matmul.allow_tf32 = True
torch.backends.cudnn.allow_tf32 = True
torch.backends.cudnn.benchmark = True

from diffusers import CogVideoXPipeline, DDIMScheduler
from diffusers.training_utils import compute_snr
from peft import LoraConfig, get_peft_model


# ---------------------------------------------------------------------------
# Dataset
# ---------------------------------------------------------------------------
class SantaVideoDataset(Dataset):
    """Loads pre-extracted video clips for LoRA training."""

    def __init__(
        self,
        clips_dir: str,
        num_frames: int = 49,
        height: int = 480,
        width: int = 720,
    ):
        self.clips_dir = Path(clips_dir)
        self.num_frames = num_frames
        self.height = height
        self.width = width

        self.clip_dirs = sorted([
            d for d in self.clips_dir.iterdir()
            if d.is_dir() and len(list(d.glob("*.jpg"))) >= num_frames
        ])

        if not self.clip_dirs:
            raise ValueError(f"No valid clips found in {clips_dir}")

        # Single prompt for identity lock — all clips show the same person
        self.prompt = (
            "A close-up video of Santa Claus with a white beard, round glasses, "
            "and red suit talking emotionally in an interview setting with warm lighting"
        )

        logger.info("Dataset: %d clips of %d frames at %dx%d", len(self.clip_dirs), num_frames, width, height)

    def __len__(self):
        return len(self.clip_dirs)

    def __getitem__(self, idx):
        clip_dir = self.clip_dirs[idx]
        frames = sorted(clip_dir.glob("*.jpg"))[:self.num_frames]

        # Load and normalize frames to [-1, 1]
        video = []
        for frame_path in frames:
            img = cv2.imread(str(frame_path))
            img = cv2.cvtColor(img, cv2.COLOR_BGR2RGB)
            img = cv2.resize(img, (self.width, self.height))
            img = img.astype(np.float32) / 127.5 - 1.0  # [-1, 1]
            video.append(img)

        # (T, H, W, C) -> (C, T, H, W)
        video = np.stack(video, axis=0)
        video = torch.from_numpy(video).permute(3, 0, 1, 2).float()

        return {"pixel_values": video, "prompt": self.prompt}


# ---------------------------------------------------------------------------
# Training
# ---------------------------------------------------------------------------
def train_lora(
    model_path: str = "/home/cabdru/.cache/huggingface/hub/cogvideox-2b",
    clips_dir: str = "/home/cabdru/.clipcannon/models/santa/training_frames",
    output_dir: str = "/home/cabdru/.clipcannon/models/santa/lora",
    steps: int = 1000,
    lr: float = 1e-4,
    rank: int = 32,
    batch_size: int = 1,
    grad_accum: int = 4,
    save_every: int = 250,
    num_frames: int = 13,  # Fewer frames to fit in VRAM during training
):
    os.makedirs(output_dir, exist_ok=True)
    device = torch.device("cuda")

    # --- Load pipeline ---
    logger.info("Loading CogVideoX-2b...")
    pipe = CogVideoXPipeline.from_pretrained(model_path, torch_dtype=torch.bfloat16)

    # Extract components
    text_encoder = pipe.text_encoder.to(device)
    tokenizer = pipe.tokenizer
    transformer = pipe.transformer.to(device)
    vae = pipe.vae.to(device)
    scheduler = pipe.scheduler

    # Freeze everything except what LoRA will adapt
    text_encoder.requires_grad_(False)
    vae.requires_grad_(False)
    transformer.requires_grad_(False)

    # Enable gradient checkpointing on transformer
    transformer.enable_gradient_checkpointing()

    vram_after_load = torch.cuda.memory_allocated() / 1e9
    logger.info("Models on GPU: %.1fGB", vram_after_load)

    # --- Encode the prompt once (same for all clips) ---
    prompt = (
        "A close-up video of Santa Claus with a white beard, round glasses, "
        "and red suit talking emotionally in an interview setting with warm lighting"
    )
    prompt_inputs = tokenizer(prompt, return_tensors="pt", padding="max_length",
                              max_length=tokenizer.model_max_length, truncation=True)
    with torch.no_grad():
        prompt_embeds = text_encoder(prompt_inputs.input_ids.to(device))[0]

    # Free T5 after encoding — saves ~5GB
    text_encoder.to("cpu")
    del text_encoder, tokenizer
    gc.collect(); torch.cuda.empty_cache()
    logger.info("T5 freed. VRAM: %.1fGB", torch.cuda.memory_allocated() / 1e9)

    # --- Apply LoRA to transformer ---
    lora_config = LoraConfig(
        r=rank,
        lora_alpha=rank,
        target_modules=["to_q", "to_k", "to_v", "to_out.0"],
        lora_dropout=0.05,
    )
    transformer = get_peft_model(transformer, lora_config)
    trainable_params = sum(p.numel() for p in transformer.parameters() if p.requires_grad)
    total_params = sum(p.numel() for p in transformer.parameters())
    logger.info("LoRA: rank=%d, trainable=%d (%.2f%% of %d)", rank, trainable_params,
                100 * trainable_params / total_params, total_params)

    # --- Dataset ---
    dataset = SantaVideoDataset(clips_dir, num_frames=num_frames)
    dataloader = DataLoader(dataset, batch_size=batch_size, shuffle=True, num_workers=0)

    # --- Optimizer ---
    optimizer = torch.optim.AdamW(
        [p for p in transformer.parameters() if p.requires_grad],
        lr=lr,
        weight_decay=1e-2,
    )
    lr_scheduler = torch.optim.lr_scheduler.CosineAnnealingLR(optimizer, T_max=steps)

    # --- Training loop ---
    logger.info("Starting LoRA training: %d steps, lr=%.1e, rank=%d", steps, lr, rank)
    logger.info("VRAM: %.1fGB", torch.cuda.memory_allocated() / 1e9)

    transformer.train()
    global_step = 0
    best_loss = float("inf")
    loss_accum = 0.0
    t0 = time.time()

    while global_step < steps:
        for batch in dataloader:
            if global_step >= steps:
                break

            pixel_values = batch["pixel_values"].to(device, dtype=torch.bfloat16)

            # Encode video to latents
            with torch.no_grad():
                latents = vae.encode(pixel_values).latent_dist.sample()
                latents = latents * vae.config.scaling_factor

            # Sample noise
            noise = torch.randn_like(latents)
            timesteps = torch.randint(0, scheduler.config.num_train_timesteps,
                                      (latents.shape[0],), device=device).long()

            # Add noise to latents
            noisy_latents = scheduler.add_noise(latents, noise, timesteps)

            # Predict noise
            encoder_hidden_states = prompt_embeds.expand(latents.shape[0], -1, -1)
            model_pred = transformer(
                hidden_states=noisy_latents,
                encoder_hidden_states=encoder_hidden_states,
                timestep=timesteps,
                return_dict=False,
            )[0]

            # Compute loss
            loss = F.mse_loss(model_pred.float(), noise.float(), reduction="mean")
            loss = loss / grad_accum
            loss.backward()
            loss_accum += loss.item()

            if (global_step + 1) % grad_accum == 0:
                torch.nn.utils.clip_grad_norm_(
                    [p for p in transformer.parameters() if p.requires_grad], 1.0
                )
                optimizer.step()
                lr_scheduler.step()
                optimizer.zero_grad()

            global_step += 1

            if global_step % 25 == 0:
                avg_loss = loss_accum / 25 * grad_accum
                elapsed = time.time() - t0
                vram = torch.cuda.memory_allocated() / 1e9
                peak = torch.cuda.max_memory_allocated() / 1e9
                logger.info(
                    "Step %d/%d | loss=%.4f | lr=%.2e | %.1fs | VRAM=%.1f/%.1fGB",
                    global_step, steps, avg_loss, lr_scheduler.get_last_lr()[0],
                    elapsed, vram, peak,
                )
                if avg_loss < best_loss:
                    best_loss = avg_loss
                loss_accum = 0.0

            if global_step % save_every == 0:
                save_path = os.path.join(output_dir, f"checkpoint-{global_step}")
                transformer.save_pretrained(save_path)
                logger.info("Saved checkpoint: %s", save_path)

    # Final save
    final_path = os.path.join(output_dir, "final")
    transformer.save_pretrained(final_path)

    elapsed = time.time() - t0
    logger.info("Training complete: %d steps in %.1f min, best_loss=%.4f", steps, elapsed / 60, best_loss)
    logger.info("LoRA saved: %s", final_path)

    # Cleanup
    del transformer, vae, optimizer
    gc.collect(); torch.cuda.empty_cache()


if __name__ == "__main__":
    import argparse
    p = argparse.ArgumentParser()
    p.add_argument("--steps", type=int, default=1000)
    p.add_argument("--lr", type=float, default=1e-4)
    p.add_argument("--rank", type=int, default=32)
    p.add_argument("--batch-size", type=int, default=1)
    p.add_argument("--grad-accum", type=int, default=4)
    p.add_argument("--save-every", type=int, default=250)
    p.add_argument("--num-frames", type=int, default=13)
    args = p.parse_args()

    train_lora(
        steps=args.steps, lr=args.lr, rank=args.rank,
        batch_size=args.batch_size, grad_accum=args.grad_accum,
        save_every=args.save_every, num_frames=args.num_frames,
    )
