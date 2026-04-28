#!/usr/bin/env python3
"""CUDA 13 / Blackwell runtime checks and PyTorch tuning helpers.

This module is deliberately fail-fast. The ContextGraph v4.2 GPU path is meant
to run on the local RTX 5090-class Blackwell card, so CUDA absence, wrong
architecture, missing sm_120 kernels, or undersized VRAM should be treated as a
configuration error instead of silently falling back to CPU.
"""
from __future__ import annotations

import gc
import logging
import os
from pathlib import Path
from typing import Any


logger = logging.getLogger("v4_2.cuda13_blackwell")

CUDA_HOME_13_2 = Path("/usr/local/cuda-13.2")
DEFAULT_ALLOC_CONF = "expandable_segments:True,garbage_collection_threshold:0.8"


def apply_preimport_cuda_env() -> None:
    """Set allocator/toolkit environment before importing torch.

    `PYTORCH_CUDA_ALLOC_CONF` must be present before CUDA initialization to be
    reliable, so training entry points call this before heavy imports.
    """
    os.environ.setdefault("CUDA_HOME", str(CUDA_HOME_13_2))
    os.environ.setdefault("CUDA_PATH", str(CUDA_HOME_13_2))
    os.environ.setdefault("TORCH_CUDA_ARCH_LIST", "12.0")
    os.environ.setdefault("CUDA_MODULE_LOADING", "LAZY")
    os.environ.setdefault("PYTORCH_CUDA_ALLOC_CONF", DEFAULT_ALLOC_CONF)
    os.environ.setdefault("TOKENIZERS_PARALLELISM", "false")


def configure_torch_cuda(
    torch: Any,
    *,
    require_cuda: bool = True,
    require_blackwell: bool = True,
    require_cuda_home: bool = True,
    min_total_vram_gib: float = 31.0,
    label: str = "contextgraph",
) -> dict[str, Any]:
    """Verify the runtime stack and enable Blackwell-friendly PyTorch flags."""
    if require_cuda_home:
        expected_cuda_home = str(CUDA_HOME_13_2)
        if not CUDA_HOME_13_2.exists():
            raise RuntimeError(f"{label}: required CUDA toolkit path is missing: {expected_cuda_home}")
        for env_name in ("CUDA_HOME", "CUDA_PATH"):
            if os.environ.get(env_name) != expected_cuda_home:
                raise RuntimeError(
                    f"{label}: {env_name} must be {expected_cuda_home}, "
                    f"got {os.environ.get(env_name)!r}"
                )

    if require_cuda and not torch.cuda.is_available():
        raise RuntimeError(f"{label}: CUDA is required but torch.cuda.is_available() is False")
    if not torch.cuda.is_available():
        return {"cuda_available": False}

    device_idx = torch.cuda.current_device()
    props = torch.cuda.get_device_properties(device_idx)
    capability = tuple(torch.cuda.get_device_capability(device_idx))
    arch_list = list(torch.cuda.get_arch_list())
    total_gib = props.total_memory / (1024 ** 3)

    if require_blackwell and capability != (12, 0):
        raise RuntimeError(
            f"{label}: expected RTX 50-series Blackwell compute capability (12, 0), "
            f"got {capability} on {torch.cuda.get_device_name(device_idx)!r}"
        )
    if "sm_120" not in arch_list:
        raise RuntimeError(
            f"{label}: PyTorch binary lacks sm_120 kernels; arch_list={arch_list}. "
            "Install a CUDA 12.8+ / CUDA 13.x PyTorch build."
        )
    if total_gib < min_total_vram_gib:
        raise RuntimeError(
            f"{label}: expected at least {min_total_vram_gib:.1f} GiB VRAM, "
            f"got {total_gib:.2f} GiB"
        )

    torch.backends.cuda.matmul.allow_tf32 = True
    torch.backends.cudnn.allow_tf32 = True
    torch.set_float32_matmul_precision("high")
    if hasattr(torch.backends.cuda, "enable_flash_sdp"):
        torch.backends.cuda.enable_flash_sdp(True)
    if hasattr(torch.backends.cuda, "enable_mem_efficient_sdp"):
        torch.backends.cuda.enable_mem_efficient_sdp(True)
    if hasattr(torch.backends.cuda, "enable_math_sdp"):
        torch.backends.cuda.enable_math_sdp(True)

    free_bytes, total_bytes = torch.cuda.mem_get_info(device_idx)
    info = {
        "label": label,
        "torch_version": torch.__version__,
        "torch_cuda_runtime": torch.version.cuda,
        "device_name": torch.cuda.get_device_name(device_idx),
        "device_index": device_idx,
        "compute_capability": list(capability),
        "arch_list": arch_list,
        "total_vram_gib": round(total_gib, 3),
        "free_vram_gib": round(free_bytes / (1024 ** 3), 3),
        "torch_tf32_matmul": bool(torch.backends.cuda.matmul.allow_tf32),
        "torch_tf32_cudnn": bool(torch.backends.cudnn.allow_tf32),
        "flash_sdp_enabled": bool(torch.backends.cuda.flash_sdp_enabled()),
        "mem_efficient_sdp_enabled": bool(torch.backends.cuda.mem_efficient_sdp_enabled()),
        "math_sdp_enabled": bool(torch.backends.cuda.math_sdp_enabled()),
        "cuda_home": os.environ.get("CUDA_HOME"),
        "pytorch_cuda_alloc_conf": os.environ.get("PYTORCH_CUDA_ALLOC_CONF"),
        "cuda_module_loading": os.environ.get("CUDA_MODULE_LOADING"),
        "total_mem_get_info_gib": round(total_bytes / (1024 ** 3), 3),
    }
    logger.info("CUDA runtime verified: %s", info)
    return info


def release_torch_cuda(torch: Any) -> None:
    """Synchronize and free Python/CUDA allocator state at process boundaries."""
    gc.collect()
    if torch.cuda.is_available():
        torch.cuda.synchronize()
        torch.cuda.empty_cache()
        try:
            torch.cuda.ipc_collect()
        except Exception:
            logger.exception("torch.cuda.ipc_collect failed during cleanup")
            raise
