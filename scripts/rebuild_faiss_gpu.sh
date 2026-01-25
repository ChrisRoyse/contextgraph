#!/bin/bash
# Rebuild FAISS with CUDA 13.1 support for RTX 5090 (Compute Capability 12.0)
#
# This script is required because the pre-installed FAISS was built with an older
# CUDA version that doesn't support Blackwell architecture (sm_120).
#
# Constitution: ARCH-GPU-05 requires GPU HDBSCAN via FAISS.
#
# Usage: ./scripts/rebuild_faiss_gpu.sh
#
# Requirements:
# - CUDA 13.1+ toolkit installed
# - cmake >= 3.23
# - git
# - gcc/g++

set -e

FAISS_VERSION="v1.9.0"  # or latest stable
BUILD_DIR="/tmp/faiss_build"
INSTALL_PREFIX="/usr/local"

echo "=== FAISS GPU Rebuild Script ==="
echo "Target: RTX 5090 (Compute Capability 12.0)"
echo "CUDA Architecture: sm_120"
echo ""

# Check prerequisites
echo "Checking prerequisites..."

if ! command -v nvcc &> /dev/null; then
    echo "ERROR: nvcc not found. Please install CUDA Toolkit 13.1+"
    exit 1
fi

CUDA_VERSION=$(nvcc --version | grep "release" | sed -E 's/.*release ([0-9]+\.[0-9]+).*/\1/')
echo "CUDA version: $CUDA_VERSION"

if ! command -v cmake &> /dev/null; then
    echo "ERROR: cmake not found"
    exit 1
fi

CMAKE_VERSION=$(cmake --version | head -1 | sed -E 's/cmake version ([0-9]+\.[0-9]+).*/\1/')
echo "CMake version: $CMAKE_VERSION"

# Clean build directory
rm -rf "$BUILD_DIR"
mkdir -p "$BUILD_DIR"
cd "$BUILD_DIR"

echo ""
echo "=== Cloning FAISS $FAISS_VERSION ==="
git clone --depth 1 --branch "$FAISS_VERSION" https://github.com/facebookresearch/faiss.git
cd faiss

echo ""
echo "=== Configuring FAISS with GPU support ==="
# Configure with:
# - GPU enabled
# - C API enabled (required for Rust FFI)
# - Target architecture: sm_120 (RTX 5090)
# - Build type: Release
#
# Note: CUDA 13.1 only supports architectures 60-120
# sm_52 (Maxwell) is not supported, so we must override defaults

# Find CUDA path
CUDA_PATH="${CUDA_PATH:-/usr/local/cuda}"
if [ -d "/usr/local/cuda-13.1" ]; then
    CUDA_PATH="/usr/local/cuda-13.1"
fi

export CUDACXX="$CUDA_PATH/bin/nvcc"
export CUDAHOSTCXX=/usr/bin/g++

cmake -B build \
    -DFAISS_ENABLE_GPU=ON \
    -DFAISS_ENABLE_C_API=ON \
    -DFAISS_ENABLE_PYTHON=OFF \
    -DBUILD_TESTING=OFF \
    -DCMAKE_CUDA_ARCHITECTURES="80;86;89;90;100;120" \
    -DCMAKE_CUDA_COMPILER="$CUDA_PATH/bin/nvcc" \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_INSTALL_PREFIX="$INSTALL_PREFIX" \
    -DBUILD_SHARED_LIBS=ON \
    -DFAISS_OPT_LEVEL=avx2

echo ""
echo "=== Building FAISS (this may take 10-20 minutes) ==="
cmake --build build --parallel $(nproc)

echo ""
echo "=== Installing FAISS ==="
sudo cmake --install build

echo ""
echo "=== Updating library cache ==="
sudo ldconfig

echo ""
echo "=== Verifying installation ==="
ldconfig -p | grep faiss

echo ""
echo "=== Testing FAISS GPU ==="
cat > /tmp/test_faiss_gpu.c << 'EOF'
#include <stdio.h>

int faiss_get_num_gpus(int* n);

int main() {
    int n = -1;
    int ret = faiss_get_num_gpus(&n);
    printf("faiss_get_num_gpus returned: %d, num_gpus: %d\n", ret, n);
    if (ret == 0 && n > 0) {
        printf("SUCCESS: FAISS GPU is working!\n");
        return 0;
    } else {
        printf("FAILED: FAISS GPU not working\n");
        return 1;
    }
}
EOF

gcc /tmp/test_faiss_gpu.c -o /tmp/faiss_gpu_test_bin -L/usr/local/lib -lfaiss_c -Wl,-rpath,/usr/local/lib

if /tmp/faiss_gpu_test_bin; then
    echo ""
    echo "=== SUCCESS ==="
    echo "FAISS with GPU support for RTX 5090 is now installed."
    echo "You can now run: cargo test -p context-graph-cuda hdbscan"
else
    echo ""
    echo "=== FAILED ==="
    echo "FAISS GPU test failed. Check the output above for errors."
    exit 1
fi
