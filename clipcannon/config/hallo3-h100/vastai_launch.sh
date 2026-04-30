#!/bin/bash
set -euo pipefail

# Hallo3 H100 — Vast.ai Instance Launcher
#
# Provisions an H100 80GB instance, uploads Santa data, runs Hallo3 inference.
#
# Prerequisites:
#   - vastai CLI installed and API key set
#   - Santa reference image and audio files ready
#
# Usage:
#   bash config/hallo3-h100/vastai_launch.sh

echo "============================================================"
echo "  Hallo3 H100 — Vast.ai Launcher"
echo "============================================================"

# Load API key
source ~/.config/vastai/credentials 2>/dev/null || {
    echo "ERROR: No Vast.ai credentials found at ~/.config/vastai/credentials"
    exit 1
}

# Verify vastai CLI
if ! command -v vastai &>/dev/null; then
    echo "ERROR: vastai CLI not installed. Run: pip install vastai"
    exit 1
fi

echo "Searching for best H100 instance..."

# Find cheapest H100 80GB with enough disk and reliability
OFFER=$(vastai search offers \
    'gpu_name=H100_SXM gpu_ram>=80 num_gpus=1 dph<=3.0 disk_space>=200 inet_down>=500 reliability>=0.95' \
    --order 'dph' --raw 2>/dev/null | python3 -c "
import json, sys
offers = json.load(sys.stdin)
if offers:
    o = offers[0]
    print(f'{o[\"id\"]}|{o[\"dph_total\"]:.4f}|{o[\"gpu_name\"]}|{o[\"gpu_ram\"]}|{o[\"disk_space\"]}')
else:
    print('NONE')
")

if [ "$OFFER" = "NONE" ]; then
    echo "ERROR: No suitable H100 instances available"
    exit 1
fi

IFS='|' read -r OFFER_ID PRICE GPU_NAME VRAM DISK <<< "$OFFER"
echo "Best offer: ID=$OFFER_ID, \$$PRICE/hr, $GPU_NAME ${VRAM}GB, ${DISK}GB disk"

# Create the instance
echo ""
echo "Creating instance..."
INSTANCE_ID=$(vastai create instance "$OFFER_ID" \
    --image "nvidia/cuda:12.1.1-cudnn8-devel-ubuntu22.04" \
    --disk 200 \
    --onstart-cmd "apt-get update && apt-get install -y python3.10 python3-pip git wget ffmpeg libgl1-mesa-glx libglib2.0-0 libsndfile1 && ln -sf /usr/bin/python3.10 /usr/bin/python" \
    --raw 2>/dev/null | python3 -c "
import json, sys
data = json.load(sys.stdin)
if 'new_contract' in data:
    print(data['new_contract'])
elif isinstance(data, dict) and 'id' in data:
    print(data['id'])
else:
    print(data)
")

echo "Instance created: ID=$INSTANCE_ID"
echo ""
echo "Waiting for instance to start (this may take 1-3 minutes)..."

# Wait for instance to be running
for i in $(seq 1 60); do
    STATUS=$(vastai show instance "$INSTANCE_ID" --raw 2>/dev/null | python3 -c "
import json, sys
try:
    data = json.load(sys.stdin)
    if isinstance(data, list) and data:
        print(data[0].get('actual_status', data[0].get('status_msg', 'unknown')))
    elif isinstance(data, dict):
        print(data.get('actual_status', data.get('status_msg', 'unknown')))
    else:
        print('unknown')
except:
    print('unknown')
" 2>/dev/null || echo "unknown")

    if [ "$STATUS" = "running" ]; then
        echo "Instance is RUNNING"
        break
    fi
    echo "  Status: $STATUS (attempt $i/60)"
    sleep 10
done

if [ "$STATUS" != "running" ]; then
    echo "ERROR: Instance failed to start after 10 minutes"
    echo "Check: vastai show instance $INSTANCE_ID"
    exit 1
fi

# Get SSH connection info
echo ""
echo "Getting SSH connection info..."
SSH_INFO=$(vastai ssh-url "$INSTANCE_ID" 2>/dev/null || echo "")
echo "SSH: $SSH_INFO"
echo ""
echo "============================================================"
echo "  Instance Ready!"
echo "============================================================"
echo ""
echo "Instance ID: $INSTANCE_ID"
echo "Cost: \$$PRICE/hr"
echo "SSH: $SSH_INFO"
echo ""
echo "Next steps:"
echo "  1. SSH into the instance"
echo "  2. Clone hallo3 and install deps"
echo "  3. Download 52GB of models"
echo "  4. Upload Santa reference image + audio"
echo "  5. Run inference"
echo ""
echo "Quick setup on the instance:"
echo "  git clone https://github.com/fudan-generative-vision/hallo3.git /app"
echo "  cd /app && pip install -r requirements.txt"
echo "  pip install torch==2.4.0 --index-url https://download.pytorch.org/whl/cu121"
echo "  huggingface-cli download fudan-generative-ai/hallo3 --local-dir ./pretrained_models"
echo ""
echo "To destroy when done:"
echo "  vastai destroy instance $INSTANCE_ID"
