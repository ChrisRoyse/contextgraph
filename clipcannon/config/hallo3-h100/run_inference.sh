#!/bin/bash
set -euo pipefail

# Hallo3 inference wrapper for Santa avatar generation
#
# Usage:
#   bash run_inference.sh INPUT_TXT OUTPUT_DIR [SEED]
#
# INPUT_TXT format (one line per generation):
#   prompt@@image_path@@audio_path
#
# Example:
#   An older man dressed as Santa Claus speaking warmly@@/data/input/santa.jpg@@/data/input/speech.wav

INPUT_FILE="${1:?Usage: run_inference.sh INPUT_FILE OUTPUT_DIR [SEED]}"
OUTPUT_DIR="${2:?Usage: run_inference.sh INPUT_FILE OUTPUT_DIR [SEED]}"
SEED="${3:-42}"

if [ ! -f "$INPUT_FILE" ]; then
    echo "ERROR: Input file not found: $INPUT_FILE"
    exit 1
fi

mkdir -p "$OUTPUT_DIR"

echo "============================================================"
echo "  Hallo3 Inference"
echo "============================================================"
echo "Input:  $INPUT_FILE"
echo "Output: $OUTPUT_DIR"
echo "Seed:   $SEED"
echo "GPU:    $(nvidia-smi --query-gpu=name,memory.used,memory.total --format=csv,noheader)"
echo ""
echo "Contents of input file:"
cat "$INPUT_FILE"
echo ""
echo "Starting generation..."
echo ""

WORLD_SIZE=1 RANK=0 LOCAL_RANK=0 LOCAL_WORLD_SIZE=1 \
python /app/hallo3/sample_video.py \
    --base /app/configs/cogvideox_5b_i2v_s2.yaml /app/configs/inference.yaml \
    --seed "$SEED" \
    --input-file "$INPUT_FILE" \
    --output-dir "$OUTPUT_DIR"

echo ""
echo "============================================================"
echo "  Generation complete"
echo "============================================================"
echo "Output files:"
ls -la "$OUTPUT_DIR"/*.mp4 2>/dev/null || echo "No MP4 files found"
echo ""
echo "GPU memory after generation:"
nvidia-smi --query-gpu=memory.used,memory.total --format=csv,noheader
