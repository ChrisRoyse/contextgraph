#!/usr/bin/env python3
"""Santa lip sync LoRA training with mouth-weighted loss + viseme curriculum.

ALL BLOCKERS FROM SHERLOCK AUDIT FIXED:
  1. Video extraction now uses FFmpeg with explicit 25fps (was reading at native 60fps)
  2. Mouth bboxes come from precomputed YOLOv5-Face landmarks (was geometric nose)
  3. Flow matching target corrected: noise - latent (was latent - noise)
  4. q_audio removed from LoRA targets (dead upstream, overwritten)
  5. v2 LoRA merging removed — trains all needed LoRA targets from scratch
  6. Audio features streamed to disk, not accumulated in RAM
  7. Real WeightedRandomSampler based on viseme rarity

Hard-fails if any required input is missing instead of silently falling back.

Usage:
    python scripts/train_santa_lipsync.py --phase encode
    python scripts/train_santa_lipsync.py --phase train --steps 3000
"""

import argparse
import gc
import json
import logging
import math
import os
import subprocess
import sys
import tempfile
import time
from collections import Counter
from pathlib import Path

os.environ.setdefault("PYTORCH_CUDA_ALLOC_CONF", "")
os.environ.setdefault("HF_HUB_OFFLINE", "1")
os.environ.setdefault("TRANSFORMERS_OFFLINE", "1")
os.environ.setdefault("TF_ENABLE_ONEDNN_OPTS", "0")
sys.stdout.reconfigure(line_buffering=True)

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(message)s",
    handlers=[
        logging.StreamHandler(),
        logging.FileHandler("/tmp/santa_lipsync_train.log"),
    ],
)
log = logging.getLogger("lipsync")

sys.path.insert(0, "/home/cabdru/echomimic_v3")

import numpy as np
import librosa
import torch
import torch.nn.functional as F
from torch.utils.data import Dataset, DataLoader, WeightedRandomSampler
from safetensors.torch import load_file
from omegaconf import OmegaConf
from PIL import Image
from einops import rearrange

torch.backends.cuda.matmul.allow_tf32 = True
torch.backends.cudnn.allow_tf32 = True

from peft import LoraConfig, get_peft_model

# ============================================================
#  Paths
# ============================================================
MODEL_PATH = "/home/cabdru/echomimic_v3/pretrained_weights/Wan2.1-Fun-V1.1-1.3B-InP"
TRANSFORMER_PATH = "/home/cabdru/echomimic_v3/pretrained_weights/echomimicv3-flash-pro/diffusion_pytorch_model.safetensors"
WAV2VEC_DIR = "/home/cabdru/echomimic_v3/pretrained_weights/chinese-wav2vec2-base"
CONFIG_PATH = "/home/cabdru/echomimic_v3/config/config.yaml"

DATA_DIR = Path.home() / "echomimic_v3" / "datasets" / "santa_teleological"
COND_DIR = DATA_DIR / "conditioning"
LABELS_FILE = DATA_DIR / "labels.json"
SOURCE_VIDEO = Path.home() / ".clipcannon" / "projects" / "proj_2ea7221d" / "source" / "2026-04-03 04-23-11.mp4"
VOCALS_WAV = Path.home() / ".clipcannon" / "projects" / "proj_2ea7221d" / "stems" / "vocals.wav"
REFERENCE_IMAGE = Path.home() / ".clipcannon" / "models" / "santa" / "reference" / "santa_fullframe_600s.png"

OUTPUT_DIR = Path.home() / ".clipcannon" / "models" / "santa" / "echov3_lipsync"

# Training dimensions
VIDEO_LENGTH = 49           # frames at 25 fps
LATENT_T = 13               # (49-1)//4 + 1
RESOLUTION = 480            # 480x480 input
LATENT_SPATIAL = 60         # 480 / 8

# Loss weighting
MOUTH_WEIGHT = 10.0
FACE_WEIGHT = 2.0
BG_WEIGHT = 1.0


# ============================================================
#  Utility: fail-fast preflight
# ============================================================

def preflight_check():
    """Verify every required input exists before starting."""
    required = {
        "MODEL_PATH": Path(MODEL_PATH),
        "TRANSFORMER_PATH": Path(TRANSFORMER_PATH),
        "WAV2VEC_DIR": Path(WAV2VEC_DIR),
        "CONFIG_PATH": Path(CONFIG_PATH),
        "SOURCE_VIDEO": SOURCE_VIDEO,
        "VOCALS_WAV": VOCALS_WAV,
        "REFERENCE_IMAGE": REFERENCE_IMAGE,
        "LABELS_FILE": LABELS_FILE,
    }
    errors = []
    for name, path in required.items():
        if not path.exists():
            errors.append(f"{name}: {path}")
    if errors:
        log.error("Missing required files:")
        for e in errors:
            log.error("  %s", e)
        raise FileNotFoundError(
            "Preflight failed. Run precompute_face_landmarks.py and "
            "label_training_clips.py first."
        )
    log.info("Preflight OK — all required inputs present")


# ============================================================
#  Video extraction (BLOCKER 1 FIX: use FFmpeg at explicit 25fps)
# ============================================================

def extract_clip_frames_ffmpeg(source_video: Path, start_ms: int, n_frames: int,
                                resolution: int) -> torch.Tensor:
    """Extract exactly n_frames starting at start_ms, at 25fps, sized to resolution.

    FIX: Previous version used cv2.VideoCapture which reads at source FPS (60).
    This made clips 2.4x shorter than intended in time. FFmpeg with -r 25 forces
    25fps output regardless of source FPS.

    Uses a scale+crop pipeline to produce resolution x resolution square output
    with the face centered as much as possible. The scene_map face is ROUGHLY
    centered horizontally in the source, so we just do a center crop.
    """
    import cv2

    start_s = start_ms / 1000.0
    duration_s = n_frames / 25.0  # 25fps target

    with tempfile.TemporaryDirectory(prefix="clip_frames_") as tmp:
        tmp_path = Path(tmp)
        frame_pattern = tmp_path / "f_%04d.jpg"

        # FFmpeg:
        #   -ss seek
        #   -t duration
        #   -r 25 force output fps
        #   -vf scale to short-edge=resolution, then center crop
        cmd = [
            "ffmpeg", "-y", "-loglevel", "error",
            "-ss", f"{start_s:.3f}",
            "-i", str(source_video),
            "-t", f"{duration_s:.3f}",
            "-r", "25",
            "-vf", f"scale=-2:{resolution},crop={resolution}:{resolution}",
            "-q:v", "3",
            str(frame_pattern),
        ]
        result = subprocess.run(cmd, capture_output=True, text=True)
        if result.returncode != 0:
            raise RuntimeError(
                f"FFmpeg failed for clip at {start_ms}ms: {result.stderr}"
            )

        frame_files = sorted(tmp_path.glob("f_*.jpg"))
        if len(frame_files) < n_frames:
            raise RuntimeError(
                f"Expected {n_frames} frames but FFmpeg produced {len(frame_files)}"
            )

        frames = []
        for i in range(n_frames):
            frame = cv2.imread(str(frame_files[i]))
            if frame is None:
                raise RuntimeError(f"Failed to read extracted frame {i}")
            frame = cv2.cvtColor(frame, cv2.COLOR_BGR2RGB)
            frames.append(frame)

        video = np.stack(frames)  # (T, H, W, C)
        video = torch.from_numpy(video).permute(3, 0, 1, 2).float()  # (C, T, H, W)
        video = video / 127.5 - 1.0
        return video.unsqueeze(0)  # (1, C, T, H, W)


# ============================================================
#  Audio extraction (unchanged, already correct)
# ============================================================

def loudness_norm(audio, sr, target=-23.0):
    import pyloudnorm as pyln
    meter = pyln.Meter(sr)
    loudness = meter.integrated_loudness(audio)
    if loudness == float("-inf"):
        return audio
    return pyln.normalize.loudness(audio, loudness, target)


def extract_audio_features(vocals_path: Path, start_ms: int,
                           wav2vec_model, feature_extractor,
                           video_length: int, device) -> torch.Tensor:
    """Extract Wav2Vec features for a clip."""
    start_s = start_ms / 1000.0
    duration_s = video_length / 25.0  # match video duration
    audio, sr = librosa.load(str(vocals_path), sr=16000, offset=start_s,
                             duration=duration_s)
    audio = loudness_norm(audio, sr)
    audio = audio[: int(video_length / 25 * sr)]
    audio_feature = np.squeeze(
        feature_extractor(audio, sampling_rate=16000).input_values
    )
    audio_feature = torch.from_numpy(audio_feature).float().unsqueeze(0).to(device)
    with torch.no_grad():
        embeddings = wav2vec_model(audio_feature, seq_len=video_length,
                                   output_hidden_states=True)
        audio_emb = torch.stack(embeddings.hidden_states[1:], dim=1).squeeze(0)
        audio_emb = rearrange(audio_emb, "b s d -> s b d")

    indices = (torch.arange(2 * 2 + 1) - 2) * 1
    center_indices = (torch.arange(0, video_length, 1).unsqueeze(1)
                      + indices.unsqueeze(0))
    center_indices = torch.clamp(center_indices, min=0, max=audio_emb.shape[0] - 1)
    return audio_emb[center_indices].unsqueeze(0).cpu()  # (1, F, 5, 12, 768)


# ============================================================
#  Mouth mask computation (FIXED: uses real per-frame mouth bboxes)
# ============================================================

def compute_latent_mouth_mask(label: dict, latent_t: int, latent_s: int,
                              resolution: int) -> torch.Tensor:
    """Build weight mask in latent space from per-frame mouth bboxes.

    Mouth bboxes come from YOLOv5-Face landmarks (precomputed), expressed
    in source pixel coordinates (2560x1440). We map them through the same
    scale+crop transform that extract_clip_frames_ffmpeg uses:
        scale = resolution / src_h   (480/1440 = 0.333)
        new_w = src_w * scale        (853)
        new_h = resolution           (480)
        x_off = (new_w - resolution) / 2  (186.5 — horizontal center crop)
        y_off = 0

    Then map to latent space by dividing by 8 (VAE spatial compression).
    """
    mask = torch.ones(1, 1, latent_t, latent_s, latent_s, dtype=torch.float32) * BG_WEIGHT

    src_w, src_h = 2560, 1440
    scale = resolution / src_h  # 0.333
    new_w = src_w * scale        # 853
    x_off = (new_w - resolution) / 2  # 186.5
    y_off = 0                    # top-aligned (new_h == resolution)

    face_bboxes = label.get("face_bbox_per_frame", [])
    mouth_bboxes = label.get("mouth_bbox_per_frame", [])
    n_frames = len(face_bboxes)
    if n_frames == 0:
        return mask

    def src_to_latent(bb_src):
        x1 = bb_src[0] * scale - x_off
        y1 = bb_src[1] * scale - y_off
        x2 = bb_src[2] * scale - x_off
        y2 = bb_src[3] * scale - y_off
        # Clamp to resolution window
        x1 = max(0.0, min(resolution, x1))
        x2 = max(0.0, min(resolution, x2))
        y1 = max(0.0, min(resolution, y1))
        y2 = max(0.0, min(resolution, y2))
        # To latent (divide by 8)
        lx1 = int(x1 / 8)
        lx2 = max(lx1 + 1, int(math.ceil(x2 / 8)))
        ly1 = int(y1 / 8)
        ly2 = max(ly1 + 1, int(math.ceil(y2 / 8)))
        return (max(0, lx1), max(0, ly1),
                min(latent_s, lx2), min(latent_s, ly2))

    # WAN VAE is causal: latent[0] comes from pixel[0] alone, then
    # latent[k] for k>=1 comes from pixel frames [4k-3 : 4k+1]. We sample
    # the middle of each latent time step's pixel range.
    for t in range(latent_t):
        if t == 0:
            pix_idx = 0
        else:
            # pixel range [4t-3, 4t+1], take middle = 4t-1
            pix_idx = min(n_frames - 1, max(0, 4 * t - 1))

        fx1, fy1, fx2, fy2 = src_to_latent(face_bboxes[pix_idx])
        mx1, my1, mx2, my2 = src_to_latent(mouth_bboxes[pix_idx])

        if fx2 > fx1 and fy2 > fy1:
            mask[0, 0, t, fy1:fy2, fx1:fx2] = FACE_WEIGHT
        if mx2 > mx1 and my2 > my1:
            mask[0, 0, t, my1:my2, mx1:mx2] = MOUTH_WEIGHT

    return mask


# ============================================================
#  Phase 1: Pre-encoding
#  FIX: audio features stream to disk per-clip (not dict accumulation)
# ============================================================

def pre_encode_all():
    """Pre-encode all conditioning tensors per clip.

    Encoding order: T5 (once), CLIP (once), Wav2Vec+VAE per-clip.
    Each clip's conditioning is saved immediately to disk to avoid
    accumulating tens of GB in RAM.
    """
    preflight_check()
    COND_DIR.mkdir(parents=True, exist_ok=True)

    labels_data = json.loads(LABELS_FILE.read_text())
    clips = labels_data["clips"]
    log.info("Encoding %d clips", len(clips))

    config = OmegaConf.load(CONFIG_PATH)
    device = torch.device("cuda")

    base_prompt = (
        "SANTA An older man dressed as Santa Claus with a white beard, "
        "round glasses, and red suit is speaking naturally during an interview."
    )

    # --- T5 (shared prompt) ---
    from src.wan_text_encoder import WanT5EncoderModel
    from transformers import AutoTokenizer
    log.info("Loading T5...")
    tokenizer = AutoTokenizer.from_pretrained(
        os.path.join(MODEL_PATH, config["text_encoder_kwargs"].get("tokenizer_subpath", "tokenizer"))
    )
    text_encoder = WanT5EncoderModel.from_pretrained(
        os.path.join(MODEL_PATH, config["text_encoder_kwargs"].get("text_encoder_subpath", "text_encoder")),
        additional_kwargs=OmegaConf.to_container(config["text_encoder_kwargs"]),
        torch_dtype=torch.bfloat16,
    ).to(device).eval()

    text_inputs = tokenizer(base_prompt, return_tensors="pt", padding="max_length",
                            max_length=512, truncation=True)
    seq_lens = text_inputs.attention_mask.sum(dim=1).tolist()
    with torch.no_grad():
        prompt_embeds = text_encoder(
            text_inputs.input_ids.to(device),
            attention_mask=text_inputs.attention_mask.to(device),
        )[0]
    text_context_cpu = [prompt_embeds[0, : seq_lens[0]].cpu()]
    del text_encoder, tokenizer
    gc.collect()
    torch.cuda.empty_cache()
    log.info("T5 done")

    # --- CLIP (shared reference image) ---
    import torchvision.transforms.functional as TF
    from src.wan_image_encoder import CLIPModel
    log.info("Loading CLIP...")
    clip_encoder = CLIPModel.from_pretrained(
        os.path.join(MODEL_PATH, config["image_encoder_kwargs"].get("image_encoder_subpath", "image_encoder")),
    ).to(torch.bfloat16).to(device).eval()

    ref_img = Image.open(REFERENCE_IMAGE).convert("RGB").resize((RESOLUTION, RESOLUTION))
    clip_image = TF.to_tensor(ref_img).sub_(0.5).div_(0.5).to(device, dtype=torch.bfloat16)
    with torch.no_grad():
        clip_fea_cpu = clip_encoder([clip_image[:, None, :, :]]).cpu()
    del clip_encoder
    gc.collect()
    torch.cuda.empty_cache()
    log.info("CLIP done: %s", tuple(clip_fea_cpu.shape))

    # --- Wav2Vec (loaded once, used per-clip) ---
    from src.wav2vec2 import Wav2Vec2Model
    from transformers import Wav2Vec2FeatureExtractor
    log.info("Loading Wav2Vec...")
    audio_encoder = Wav2Vec2Model.from_pretrained(
        WAV2VEC_DIR, local_files_only=True
    ).to(device).eval()
    wav2vec_fe = Wav2Vec2FeatureExtractor.from_pretrained(
        WAV2VEC_DIR, local_files_only=True
    )
    log.info("Wav2Vec loaded")

    # --- VAE (FP32 for WSL2 stability) ---
    from src.wan_vae import AutoencoderKLWan
    log.info("Loading VAE (FP32)...")
    vae = AutoencoderKLWan.from_pretrained(
        os.path.join(MODEL_PATH, config["vae_kwargs"].get("vae_subpath", "vae")),
        additional_kwargs=OmegaConf.to_container(config["vae_kwargs"]),
    ).to(torch.float32).to(device)

    # Reference image latent (shared)
    ref_tensor = torch.from_numpy(np.array(ref_img)).permute(2, 0, 1).float() / 127.5 - 1.0
    ref_tensor = ref_tensor.unsqueeze(0).unsqueeze(2).to(device)
    with torch.no_grad():
        ref_latent_cpu = vae.encode(ref_tensor).latent_dist.sample().cpu()
    log.info("Reference latent: %s", tuple(ref_latent_cpu.shape))

    # Encode each clip: audio + video + mouth mask → save to disk
    log.info("Encoding per-clip conditioning (streaming to disk)...")
    failed = 0
    for idx, clip in enumerate(clips):
        clip_id = clip["clip_id"]
        out_path = COND_DIR / f"{clip_id}.pt"
        if out_path.exists():
            continue  # resume support

        try:
            # Audio features (stay on GPU briefly, move to CPU immediately)
            audio_emb = extract_audio_features(
                VOCALS_WAV, clip["start_ms"],
                audio_encoder, wav2vec_fe, VIDEO_LENGTH, device,
            )

            # Video tensor via FFmpeg (25fps guaranteed)
            video = extract_clip_frames_ffmpeg(
                SOURCE_VIDEO, clip["start_ms"], VIDEO_LENGTH, RESOLUTION
            ).to(device)
            with torch.no_grad():
                latent_cpu = vae.encode(video).latent_dist.sample().cpu()
            del video

            # Mouth mask in latent space
            mouth_mask = compute_latent_mouth_mask(
                clip, LATENT_T, LATENT_SPATIAL, RESOLUTION
            )

            torch.save({
                "text_context": text_context_cpu,
                "clip_fea": clip_fea_cpu,
                "audio_emb": audio_emb,
                "latent": latent_cpu,
                "ref_latent": ref_latent_cpu,
                "mouth_mask": mouth_mask,
                "latent_t": LATENT_T,
                "video_length": VIDEO_LENGTH,
                "clip_id": clip_id,
                "viseme_sequence": clip["viseme_sequence"],
                "clip_category": clip["clip_category"],
                "has_speech": clip["has_speech"],
                "prosody_energy_level": clip["prosody_energy_level"],
            }, out_path)

        except Exception as e:
            log.error("Failed clip %s: %s", clip_id, e)
            failed += 1
            continue

        if (idx + 1) % 100 == 0:
            log.info("  %d/%d | VRAM=%.1fGB | failed=%d",
                     idx + 1, len(clips),
                     torch.cuda.memory_allocated() / 1e9, failed)

    del vae, audio_encoder, wav2vec_fe
    gc.collect()
    torch.cuda.empty_cache()

    saved = len(list(COND_DIR.glob("*.pt")))
    log.info("Pre-encoding complete: %d saved, %d failed", saved, failed)
    if failed > 0:
        log.warning("%d clips failed encoding", failed)


# ============================================================
#  Phase 2: Training with mouth-weighted loss + viseme-weighted sampler
# ============================================================

class VisemeWeightedDataset(Dataset):
    """Dataset with per-clip viseme rarity weights for sampler."""

    def __init__(self, cond_dir: Path):
        self.cond_dir = cond_dir
        self.files = sorted(cond_dir.glob("*.pt"))
        if not self.files:
            raise ValueError(f"No conditioning files in {cond_dir}")

        # Read viseme sequences to compute rarity weights
        log.info("Computing viseme rarity weights from %d clips...", len(self.files))
        viseme_global_counts = Counter()
        clip_visemes = []
        for f in self.files:
            data = torch.load(f, weights_only=False, map_location="cpu")
            seq = data["viseme_sequence"]
            clip_visemes.append(seq)
            viseme_global_counts.update(seq)

        # Per-clip weight = sum of (1 / global_count) for each unique viseme in clip
        # Clips with rare visemes get higher weight
        total = sum(viseme_global_counts.values())
        viseme_rarity = {
            v: total / (count * len(viseme_global_counts))
            for v, count in viseme_global_counts.items()
        }

        self.sample_weights = []
        for seq in clip_visemes:
            # Weight of a clip = mean rarity of its unique visemes
            unique_v = set(seq)
            w = sum(viseme_rarity[v] for v in unique_v) / max(1, len(unique_v))
            self.sample_weights.append(w)

        self.sample_weights = torch.tensor(self.sample_weights, dtype=torch.double)
        log.info("Viseme rarity weights: min=%.3f max=%.3f mean=%.3f",
                 float(self.sample_weights.min()),
                 float(self.sample_weights.max()),
                 float(self.sample_weights.mean()))

    def __len__(self):
        return len(self.files)

    def __getitem__(self, idx):
        return torch.load(self.files[idx], weights_only=False, map_location="cpu")


def train_lipsync(steps=3000, rank=128, lr=1e-4, save_every=500):
    """Train lipsync LoRA with all fixes applied."""
    preflight_check()
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)

    cond_files = list(COND_DIR.glob("*.pt"))
    if not cond_files:
        raise RuntimeError(f"No conditioning files in {COND_DIR}. Run --phase encode first.")
    log.info("Training on %d conditioning files", len(cond_files))

    device = torch.device("cuda")
    config = OmegaConf.load(CONFIG_PATH)

    # --- Load transformer ---
    log.info("Loading transformer...")
    from src.wan_transformer3d_audio_2512 import WanTransformerAudioMask3DModel as WanTransformer
    transformer = WanTransformer.from_pretrained(
        os.path.join(MODEL_PATH, config["transformer_additional_kwargs"].get("transformer_subpath", "./")),
        transformer_additional_kwargs=OmegaConf.to_container(config["transformer_additional_kwargs"]),
        torch_dtype=torch.bfloat16,
    )
    transformer.load_state_dict(load_file(TRANSFORMER_PATH), strict=False)
    transformer = transformer.to(device)
    transformer.gradient_checkpointing = True
    log.info("Transformer: %.1fGB", torch.cuda.memory_allocated() / 1e9)

    # --- LoRA config ---
    # BLOCKER 4 FIX: q_audio is dead upstream (overwritten immediately after q_audio(q) call).
    # Target only the live modules: k_audio, v_audio (audio cross-attention K/V), plus
    # k_img, v_img (image cross-attention K/V) for identity, and q,k,v,o for self-attention.
    # Identity is trained from scratch since v2 LoRA path was deleted in cleanup.
    lipsync_lora_config = LoraConfig(
        r=rank,
        lora_alpha=rank * 2,
        target_modules=["q", "k", "v", "o", "k_audio", "v_audio", "k_img", "v_img"],
        lora_dropout=0.0,
    )
    transformer = get_peft_model(transformer, lipsync_lora_config)
    trainable = sum(p.numel() for p in transformer.parameters() if p.requires_grad)
    total = sum(p.numel() for p in transformer.parameters())
    log.info("LoRA rank=%d: %d trainable (%.2f%% of %d)",
             rank, trainable, 100 * trainable / total, total)

    # --- Dataset with viseme-weighted sampler ---
    dataset = VisemeWeightedDataset(COND_DIR)
    sampler = WeightedRandomSampler(
        weights=dataset.sample_weights,
        num_samples=steps * 2,   # enough for all training steps (with some headroom)
        replacement=True,
    )
    loader = DataLoader(
        dataset, batch_size=1, sampler=sampler, num_workers=0,
        collate_fn=lambda x: x[0],
    )

    # --- Optimizer ---
    optimizer = torch.optim.AdamW(
        [p for p in transformer.parameters() if p.requires_grad],
        lr=lr, weight_decay=1e-2,
    )
    lr_sched = torch.optim.lr_scheduler.CosineAnnealingLR(optimizer, T_max=steps)

    log.info("Training: %d steps, rank=%d, lr=%.1e, mouth_weight=%.0f",
             steps, rank, lr, MOUTH_WEIGHT)
    transformer.train()

    step = 0
    best_loss = float("inf")
    loss_accum = 0.0
    mouth_loss_accum = 0.0
    t0 = time.time()

    for sample in loader:
        if step >= steps:
            break

        latent = sample["latent"].to(device, dtype=torch.bfloat16)
        ref_latent = sample["ref_latent"].to(device, dtype=torch.bfloat16)
        clip_fea = sample["clip_fea"].to(device, dtype=torch.bfloat16)
        audio_emb = sample["audio_emb"].to(device, dtype=torch.bfloat16)
        mouth_mask = sample["mouth_mask"].to(device, dtype=torch.bfloat16)
        latent_t = sample["latent_t"]

        B, C, T, H, W = latent.shape

        # Flow matching noise
        noise = torch.randn_like(latent)
        t_idx = torch.randint(0, 1000, (1,)).item()
        sigma = t_idx / 1000.0
        noisy = (1 - sigma) * latent + sigma * noise

        # Inpainting condition: frame 0 visible as reference, rest masked
        mask_cond = torch.ones(B, 4, T, H, W, device=device, dtype=torch.bfloat16)
        mask_cond[:, :, 0, :, :] = 0
        masked_lat = torch.zeros(B, C, T, H, W, device=device, dtype=torch.bfloat16)
        masked_lat[:, :, 0:1, :, :] = ref_latent
        y = torch.cat([mask_cond, masked_lat], dim=1)

        text_ctx = [t.to(device, dtype=torch.bfloat16) for t in sample["text_context"]]
        context_tuple = (text_ctx, audio_emb, latent_t, None)
        seq_len = T * H * W // 4

        with torch.amp.autocast("cuda", dtype=torch.bfloat16):
            output = transformer(
                x=noisy,
                t=torch.tensor([t_idx], device=device, dtype=torch.int64),
                context=context_tuple,
                seq_len=seq_len,
                clip_fea=clip_fea,
                y=y,
            )
        if isinstance(output, tuple):
            output = output[0]

        # BLOCKER 3 FIX: flow matching target is (noise - latent), not (latent - noise).
        # The scheduler expects velocity dx/dt pointing from latent toward noise.
        target = noise - latent

        per_token_loss = F.mse_loss(output.float(), target.float(), reduction="none")
        # mouth_mask: (1,1,T,H,W) broadcasts over channel dim of per_token_loss (1,C,T,H,W)
        weighted = per_token_loss * mouth_mask.float()
        avg_weight = mouth_mask.float().mean().clamp_min(1.0)
        loss = weighted.mean() / avg_weight / 2

        # Track mouth-specific loss for monitoring
        mouth_region_mask = (mouth_mask.float() >= MOUTH_WEIGHT - 0.1).float()
        mouth_denom = mouth_region_mask.sum().clamp_min(1.0)
        mouth_only_loss = (per_token_loss * mouth_region_mask).sum() / mouth_denom

        loss.backward()
        loss_accum += loss.item()
        mouth_loss_accum += mouth_only_loss.item()

        if (step + 1) % 2 == 0:
            torch.nn.utils.clip_grad_norm_(
                [p for p in transformer.parameters() if p.requires_grad], 1.0
            )
            optimizer.step()
            lr_sched.step()
            optimizer.zero_grad()

        step += 1
        if step % 25 == 0:
            avg = loss_accum / 25 * 2
            mouth_avg = mouth_loss_accum / 25
            if avg < best_loss:
                best_loss = avg
            log.info(
                "Step %d/%d | loss=%.4f mouth_loss=%.4f | lr=%.2e | %.0fs | VRAM=%.1f/%.1fGB",
                step, steps, avg, mouth_avg, lr_sched.get_last_lr()[0],
                time.time() - t0,
                torch.cuda.memory_allocated() / 1e9,
                torch.cuda.max_memory_allocated() / 1e9,
            )
            loss_accum = 0.0
            mouth_loss_accum = 0.0

        if step % save_every == 0:
            ckpt_dir = OUTPUT_DIR / f"checkpoint-{step}"
            transformer.save_pretrained(str(ckpt_dir))
            log.info("Checkpoint saved: %s", ckpt_dir)

    final_dir = OUTPUT_DIR / "final"
    transformer.save_pretrained(str(final_dir))
    log.info("Done: %d steps in %.1f min | best_loss=%.4f | saved %s",
             steps, (time.time() - t0) / 60, best_loss, final_dir)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--phase", choices=["encode", "train", "all"], default="all")
    parser.add_argument("--steps", type=int, default=3000)
    # rank=256 doubles trainable LoRA parameters for higher-quality identity
    # and lip sync fidelity. Requires a bit more VRAM (~1GB extra) but still
    # fits comfortably in 32GB.
    parser.add_argument("--rank", type=int, default=256)
    parser.add_argument("--lr", type=float, default=1e-4)
    args = parser.parse_args()

    if args.phase in ("encode", "all"):
        pre_encode_all()
    if args.phase in ("train", "all"):
        train_lipsync(steps=args.steps, rank=args.rank, lr=args.lr)
