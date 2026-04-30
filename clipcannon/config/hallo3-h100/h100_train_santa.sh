#!/bin/bash
set -euo pipefail

# ═══════════════════════════════════════════════════════════════════
# Hallo3 Santa Training — Run on H100 80GB
#
# This script runs INSIDE the H100 instance after SSH-ing in.
# It handles everything: install deps, download models, preprocess
# training data, run Stage 2 fine-tuning, test inference.
#
# Usage (on the H100 instance):
#   bash /workspace/h100_train_santa.sh [STEPS]
#
# Default: 1000 steps (~60 min, enough for single-subject fine-tuning)
# ═══════════════════════════════════════════════════════════════════

STEPS="${1:-1000}"
WORKSPACE="/workspace"
HALLO3_DIR="$WORKSPACE/hallo3"
TRAIN_DATA="$WORKSPACE/santa_training"
OUTPUT_DIR="$WORKSPACE/output"
CHECKPOINT_DIR="$WORKSPACE/checkpoints"

echo "============================================================"
echo "  Hallo3 Santa Training on H100"
echo "============================================================"
echo "Steps:     $STEPS"
echo "GPU:       $(nvidia-smi --query-gpu=name,memory.total --format=csv,noheader)"
echo "Time:      $(date)"
echo ""

# ─── Step 1: Install system deps ────────────────────────────────
echo "[1/7] Installing system dependencies..."
apt-get update -qq && apt-get install -y -qq \
    ffmpeg libgl1-mesa-glx libglib2.0-0 libsndfile1 git wget > /dev/null 2>&1
echo "  Done"

# ─── Step 2: Clone Hallo3 ───────────────────────────────────────
echo "[2/7] Setting up Hallo3..."
if [ ! -d "$HALLO3_DIR" ]; then
    git clone --depth 1 https://github.com/fudan-generative-vision/hallo3.git "$HALLO3_DIR"
fi
cd "$HALLO3_DIR"

echo "  Installing Python dependencies..."
pip install -q -r requirements.txt 2>&1 | tail -3
pip install -q "huggingface_hub[cli]"
echo "  Done"

# ─── Step 3: Download models (52GB, cached on disk) ─────────────
echo "[3/7] Checking Hallo3 models (52GB)..."
if [ ! -f "pretrained_models/hallo3/1/mp_rank_00_model_states.pt" ]; then
    echo "  Downloading models (this takes 10-20 minutes)..."
    huggingface-cli download fudan-generative-ai/hallo3 --local-dir pretrained_models
    echo "  Download complete"
else
    echo "  Models already cached"
fi
du -sh pretrained_models/
echo ""

# ─── Step 4: Verify training data ──────────────────────────────
echo "[4/7] Verifying training data..."
if [ ! -d "$TRAIN_DATA/videos" ]; then
    echo "ERROR: Training data not found at $TRAIN_DATA"
    echo "Upload with: scp -r data/hallo3_santa_training/ root@HOST:/workspace/santa_training/"
    exit 1
fi

VIDEO_COUNT=$(ls "$TRAIN_DATA/videos/"*.mp4 2>/dev/null | wc -l)
CAPTION_COUNT=$(ls "$TRAIN_DATA/caption/"*.txt 2>/dev/null | wc -l)
echo "  Videos:   $VIDEO_COUNT"
echo "  Captions: $CAPTION_COUNT"

if [ "$VIDEO_COUNT" -lt 10 ]; then
    echo "ERROR: Too few training videos ($VIDEO_COUNT). Need at least 10."
    exit 1
fi
if [ "$VIDEO_COUNT" != "$CAPTION_COUNT" ]; then
    echo "ERROR: Video count ($VIDEO_COUNT) != caption count ($CAPTION_COUNT)"
    exit 1
fi
echo "  OK"
echo ""

# ─── Step 5: Preprocess training data ──────────────────────────
echo "[5/7] Preprocessing training data (face masks, embeddings)..."
if [ ! -d "$TRAIN_DATA/face_mask" ]; then
    cd "$HALLO3_DIR"
    python hallo3/data_preprocess.py \
        -i "$TRAIN_DATA/videos" \
        -p 1 -r 0

    python hallo3/extract_meta_info.py \
        -r "$TRAIN_DATA" \
        -n santa_training
    echo "  Preprocessing complete"
else
    echo "  Preprocessed data already exists"
fi

# Verify preprocessing produced expected files
FACE_MASK_COUNT=$(ls "$TRAIN_DATA/face_mask/"*.png 2>/dev/null | wc -l)
FACE_EMB_COUNT=$(ls "$TRAIN_DATA/face_emb/"*.pt 2>/dev/null | wc -l)
AUDIO_EMB_COUNT=$(ls "$TRAIN_DATA/audio_emb/"*.pt 2>/dev/null | wc -l)
echo "  Face masks: $FACE_MASK_COUNT"
echo "  Face embeddings: $FACE_EMB_COUNT"
echo "  Audio embeddings: $AUDIO_EMB_COUNT"

if [ ! -f "$HALLO3_DIR/santa_training.json" ]; then
    echo "ERROR: Training manifest not generated"
    exit 1
fi
echo "  Manifest: $(wc -l < "$HALLO3_DIR/santa_training.json") entries"
echo ""

# ─── Step 6: Run Stage 2 training (audio modules only) ─────────
echo "[6/7] Starting Stage 2 training (audio attention fine-tuning)..."
echo "  Steps: $STEPS"
echo "  Training ONLY audio attention modules"
echo "  Identity (face/ref_model) is FROZEN"
echo ""

cd "$HALLO3_DIR"

# Create custom training config with our settings
cat > configs/sft_santa.yaml << YAML
args:
  checkpoint_activations: True
  model_parallel_size: 1
  experiment_name: santa-stage2
  mode: finetune
  load: pretrained_models/hallo3/1  # Fine-tune from pretrained Hallo3
  no_load_rng: True
  train_iters: $STEPS
  eval_iters: 1
  eval_interval: $STEPS
  eval_batch_size: 1
  save: $CHECKPOINT_DIR
  save_interval: 500
  log_interval: 10
  train_data: ["santa_training.json"]
  valid_data: ["santa_training.json"]
  split: 1,0,0
  num_workers: 4
  force_train: True
  only_log_video_latents: True

data:
  target: data_video.Stage2_SFTDataset
  params:
    video_size: [480, 720]
    fps: 8
    max_num_frames: 49
    skip_frms_num: 3.

deepspeed:
  train_micro_batch_size_per_gpu: 1
  gradient_accumulation_steps: 1
  steps_per_print: 10
  gradient_clipping: 0.1
  zero_optimization:
    stage: 2
    cpu_offload: false
    contiguous_gradients: false
    overlap_comm: true
    reduce_scatter: true
    reduce_bucket_size: 1000000000
    allgather_bucket_size: 1000000000
    load_from_fp32_weights: false
  zero_allow_untested_optimizer: true
  bf16:
    enabled: True
  fp16:
    enabled: False
  loss_scale: 0
  loss_scale_window: 400
  hysteresis: 2
  min_loss_scale: 1
  optimizer:
    type: sat.ops.FusedEmaAdam
    params:
      lr: 1e-5
      betas: [0.9, 0.95]
      eps: 1e-8
      weight_decay: 1e-4
  activation_checkpointing:
    partition_activations: false
    contiguous_memory_optimization: false
  wall_clock_breakdown: false
YAML

# Run training
WORLD_SIZE=1 RANK=0 LOCAL_RANK=0 LOCAL_WORLD_SIZE=1 \
python hallo3/train_video.py \
    --base configs/cogvideox_5b_i2v_s2.yaml configs/sft_santa.yaml

echo ""
echo "  Training complete!"
echo "  Checkpoints at: $CHECKPOINT_DIR"
ls -la "$CHECKPOINT_DIR/" 2>/dev/null
echo ""

# ─── Step 7: Test inference with fine-tuned model ───────────────
echo "[7/7] Running inference test with fine-tuned model..."

mkdir -p "$OUTPUT_DIR"

if [ -f "$TRAIN_DATA/test_input.txt" ]; then
    WORLD_SIZE=1 RANK=0 LOCAL_RANK=0 LOCAL_WORLD_SIZE=1 \
    python hallo3/sample_video.py \
        --base configs/cogvideox_5b_i2v_s2.yaml configs/inference.yaml \
        --seed 42 \
        --input-file "$TRAIN_DATA/test_input.txt" \
        --output-dir "$OUTPUT_DIR"

    echo ""
    echo "  Output:"
    ls -la "$OUTPUT_DIR/"*.mp4 2>/dev/null
else
    echo "  No test input file found. Skipping inference test."
fi

echo ""
echo "============================================================"
echo "  TRAINING COMPLETE"
echo "============================================================"
echo ""
echo "Time: $(date)"
echo "GPU:  $(nvidia-smi --query-gpu=memory.used,memory.total --format=csv,noheader)"
echo ""
echo "Checkpoints: $CHECKPOINT_DIR"
echo "Output:      $OUTPUT_DIR"
echo ""
echo "To download results:"
echo "  scp -P PORT root@HOST:$CHECKPOINT_DIR/*.pt ./checkpoints/"
echo "  scp -P PORT root@HOST:$OUTPUT_DIR/*.mp4 ./output/"
echo ""
echo "To stop billing: ./scripts/gpu down"
