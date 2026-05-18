---
namespace: discoveries
created: 2026-05-09T22:58:00Z
updated: 2026-05-09T22:58:00Z
status: active
severity: medium
tags: gpu, wsl2, rtx5090, mejepa, phase0, environment
---

# WSL GPU Access Blocked Is Not Phase 0 Blocking

## What Happened

During Phase 0 Docker/SWE-bench production, Linux-side `nvidia-smi` failed:

```text
Failed to initialize NVML: GPU access blocked by the operating system
Failed to properly shut down NVML: GPU access blocked by the operating system
```

The WSL GPU device and libraries exist:

```text
/dev/dxg
/usr/lib/wsl/lib/libcuda.so.1
/usr/lib/wsl/lib/nvidia-smi
```

Windows-side `nvidia-smi.exe` works and sees the GPU:

```text
NVIDIA GeForce RTX 5090, 596.21, 32607 MiB, 3890 MiB, 4 %
```

WSL is current enough for Docker Desktop's documented WSL requirements:

```text
WSL version: 2.6.3.0
Kernel version: 6.6.87.2-1
```

## Why It Matters

SWE-bench Phase 0 corpus generation is CPU/Docker-container bound and does not
need the RTX 5090, so this does not block finishing the 300 x 8 corpus.

Later ME-JEPA training, embedding, or GPU-accelerated local model work should
not assume Linux-side CUDA/NVML is healthy until a direct CUDA smoke passes from
inside WSL. NVIDIA's CUDA-on-WSL guidance says the Windows NVIDIA driver is the
driver source of truth and WSL should not install a Linux NVIDIA display driver.

## Resume Guidance

Before starting GPU-dependent ME-JEPA training:

1. Re-run `/usr/lib/wsl/lib/nvidia-smi`.
2. Run a real CUDA smoke, not just `nvidia-smi`, because NVML has WSL-specific
   limitations.
3. If still blocked, refresh the Windows NVIDIA driver/WSL GPU stack from
   Windows and restart WSL. Do not install a Linux NVIDIA display driver inside
   WSL.
