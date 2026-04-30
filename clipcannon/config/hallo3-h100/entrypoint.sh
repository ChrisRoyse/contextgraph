#!/bin/bash
set -euo pipefail

echo "============================================================"
echo "  Hallo3 on H100 — Entrypoint"
echo "============================================================"
echo "Time: $(date)"
echo "GPU: $(nvidia-smi --query-gpu=name,memory.total --format=csv,noheader 2>/dev/null || echo 'unknown')"
echo ""

MODEL_DIR="${MODEL_DIR:-/data/models}"
HALLO3_DIR="$MODEL_DIR/hallo3"

# ============================================================
#  Step 1: Download models if not present (52GB total)
# ============================================================
if [ ! -f "$HALLO3_DIR/hallo3/1/mp_rank_00_model_states.pt" ]; then
    echo "Downloading Hallo3 models (52GB)..."
    echo "This takes 5-15 minutes depending on network speed."
    echo ""

    mkdir -p "$HALLO3_DIR"
    huggingface-cli download fudan-generative-ai/hallo3 \
        --local-dir "$HALLO3_DIR" \
        --local-dir-use-symlinks False

    echo ""
    echo "Download complete."
    du -sh "$HALLO3_DIR"
else
    echo "Models already present at $HALLO3_DIR"
    du -sh "$HALLO3_DIR"
fi

# Symlink models into the expected location
if [ ! -d "/app/pretrained_models" ]; then
    ln -sf "$HALLO3_DIR" /app/pretrained_models
fi

echo ""
echo "============================================================"
echo "  Ready. Models loaded. GPU available."
echo "============================================================"
echo ""
echo "Usage:"
echo "  Inference: bash /app/run_inference.sh /data/input/input.txt /data/output"
echo "  Gradio UI: python /app/hallo3/app.py"
echo "  Custom:    python /app/hallo3/sample_video.py --base ./configs/cogvideox_5b_i2v_s2.yaml ./configs/inference.yaml --input-file INPUT --output-dir OUTPUT"
echo ""

# If arguments provided, run them. Otherwise start bash for interactive use.
if [ $# -gt 0 ]; then
    exec "$@"
else
    exec /bin/bash
fi
