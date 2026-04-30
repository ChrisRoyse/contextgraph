# ═══════════════════════════════════════════════════════════════════
# Hallo3 H100 — All-in-one Docker image with 52GB models baked in
#
# Build time: ~30-60 min (downloads 52GB during build)
# Image size: ~65GB
# Runtime: instant — GPU ready immediately, no download needed
#
# Build:
#   docker build -t leapable/clipcannon-hallo3:latest \
#     -f config/hallo3-h100/Dockerfile .
#
# Push:
#   docker push leapable/clipcannon-hallo3:latest
#
# Run (Vast.ai pulls this image, models included):
#   docker run --gpus all -it leapable/clipcannon-hallo3:latest
# ═══════════════════════════════════════════════════════════════════

FROM nvidia/cuda:12.1.1-cudnn8-devel-ubuntu22.04

ENV DEBIAN_FRONTEND=noninteractive
ENV PYTHONUNBUFFERED=1

# System deps + Python 3.10 (required by Hallo3's pinned pyav==14.0.1)
RUN apt-get update && apt-get install -y --no-install-recommends \
    python3.10 python3.10-dev python3.10-venv python3-pip \
    git wget curl ffmpeg \
    libgl1-mesa-glx libglib2.0-0 libsm6 libxext6 libxrender-dev \
    libsndfile1 \
    && rm -rf /var/lib/apt/lists/* \
    && ln -sf /usr/bin/python3.10 /usr/bin/python \
    && python -m pip install --upgrade pip setuptools wheel

WORKDIR /workspace

# Clone Hallo3
RUN git clone --depth 1 https://github.com/fudan-generative-vision/hallo3.git /workspace/hallo3

# Install PyTorch 2.4 for CUDA 12.1 FIRST (before requirements.txt)
WORKDIR /workspace/hallo3
RUN pip install --no-cache-dir \
    torch==2.4.0 torchvision==0.19.0 torchaudio==2.4.0 \
    --index-url https://download.pytorch.org/whl/cu121

# Install Hallo3 deps (pyav 14.0.1 works with Python 3.10)
RUN pip install --no-cache-dir -r requirements.txt && \
    pip install --no-cache-dir "huggingface_hub[cli]"

# Download ALL models (52GB) — baked into the image
RUN huggingface-cli download fudan-generative-ai/hallo3 \
    --local-dir /workspace/hallo3/pretrained_models \
    --local-dir-use-symlinks False

# Verify models downloaded correctly
RUN test -f /workspace/hallo3/pretrained_models/hallo3/1/mp_rank_00_model_states.pt && \
    echo "Hallo3 main checkpoint: OK" || exit 1
RUN test -f /workspace/hallo3/pretrained_models/cogvideox-5b-i2v-sat/transformer/1/mp_rank_00_model_states.pt && \
    echo "CogVideoX transformer: OK" || exit 1
RUN test -f /workspace/hallo3/pretrained_models/t5-v1_1-xxl/model-00001-of-00002.safetensors && \
    echo "T5-XXL encoder: OK" || exit 1

# Create workspace dirs for user data
RUN mkdir -p /workspace/input /workspace/output /workspace/training_data

WORKDIR /workspace/hallo3

# Default: drop into bash (user runs commands manually)
CMD ["/bin/bash"]
