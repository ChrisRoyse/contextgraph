"""GPU memory guard for RTX 5090 + WSL2 stability.

Enforces VRAM budgets, serializes model loads, and prevents the concurrent
CUDA context contention that crashes WSL2. Must be imported before any torch
usage in the Phoenix video pipeline.

RTX 5090 Blackwell (CUDA 13.2) optimizations:
- expandable_segments: reduces fragmentation via cudaMallocAsync pools
- max_split_size_mb: prevents allocator from creating huge contiguous blocks
  that fragment when freed
- garbage_collection_threshold: triggers Python GC when CUDA allocator
  pressure exceeds threshold (0.6 = aggressive for WSL2)
- CUDA_MODULE_LOADING=LAZY: don't load all CUDA modules at context init
- TF32 matmul: use tensor cores for FP32 operations (2x throughput)
"""
from __future__ import annotations

import gc
import logging
import os
import threading
from contextlib import contextmanager

logger = logging.getLogger(__name__)

# --- Set CUDA env vars BEFORE any torch import ---
# These must be set before the first torch.cuda call or they have no effect.
_CUDA_ALLOC_CONF = (
    "expandable_segments:True,"
    "max_split_size_mb:512,"
    "garbage_collection_threshold:0.6"
)
os.environ.setdefault("PYTORCH_CUDA_ALLOC_CONF", _CUDA_ALLOC_CONF)
os.environ.setdefault("CUDA_MODULE_LOADING", "LAZY")
os.environ.setdefault("CUDA_DEVICE_ORDER", "PCI_BUS_ID")
# Offline mode — never download models at runtime
os.environ.setdefault("HF_HUB_OFFLINE", "1")
os.environ.setdefault("TRANSFORMERS_OFFLINE", "1")

import torch

# Enable Blackwell tensor core optimizations
torch.backends.cuda.matmul.allow_tf32 = True
torch.backends.cudnn.allow_tf32 = True
torch.backends.cudnn.benchmark = True
if hasattr(torch, "set_float32_matmul_precision"):
    torch.set_float32_matmul_precision("high")

# --- Singleton lock for serialized GPU operations ---
_gpu_lock = threading.Lock()

# VRAM budget: leave 4GB headroom for WSL2 driver + OS overhead
VRAM_TOTAL_MB = 32768  # RTX 5090
VRAM_RESERVED_MB = 4096  # driver + OS + safety margin
VRAM_BUDGET_MB = VRAM_TOTAL_MB - VRAM_RESERVED_MB  # 28672 MB usable


def gpu_cleanup() -> dict:
    """Aggressive GPU memory cleanup. Call between heavy operations.

    Returns dict with memory stats for logging.
    """
    gc.collect()
    if torch.cuda.is_available():
        torch.cuda.synchronize()
        torch.cuda.empty_cache()
        # Reset peak stats for next operation
        torch.cuda.reset_peak_memory_stats()
        allocated = torch.cuda.memory_allocated() / 1e6
        reserved = torch.cuda.memory_reserved() / 1e6
        logger.debug("GPU cleanup: %.0fMB allocated, %.0fMB reserved", allocated, reserved)
        return {"allocated_mb": allocated, "reserved_mb": reserved}
    return {"allocated_mb": 0, "reserved_mb": 0}


def check_vram(required_mb: int, operation: str) -> None:
    """Check if enough VRAM is available before an operation.

    Raises RuntimeError if insufficient VRAM instead of letting the
    operation OOM and crash WSL2.
    """
    if not torch.cuda.is_available():
        raise RuntimeError(f"CUDA not available for {operation}")

    free_mb = (torch.cuda.get_device_properties(0).total_memory
               - torch.cuda.memory_allocated()) / 1e6
    if free_mb < required_mb:
        # Try cleanup first
        gpu_cleanup()
        free_mb = (torch.cuda.get_device_properties(0).total_memory
                   - torch.cuda.memory_allocated()) / 1e6
        if free_mb < required_mb:
            raise RuntimeError(
                f"Insufficient VRAM for {operation}: need {required_mb}MB, "
                f"have {free_mb:.0f}MB free. Kill other GPU processes first."
            )
    logger.info("%s: %.0fMB free, %.0fMB required — OK", operation, free_mb, required_mb)


def vram_stats() -> dict:
    """Get current VRAM statistics."""
    if not torch.cuda.is_available():
        return {"available": False}
    props = torch.cuda.get_device_properties(0)
    allocated = torch.cuda.memory_allocated()
    reserved = torch.cuda.memory_reserved()
    return {
        "device": props.name,
        "total_mb": props.total_memory / 1e6,
        "allocated_mb": allocated / 1e6,
        "reserved_mb": reserved / 1e6,
        "free_mb": (props.total_memory - allocated) / 1e6,
        "peak_mb": torch.cuda.max_memory_allocated() / 1e6,
        "temperature": _gpu_temperature(),
    }


def _gpu_temperature() -> int:
    """Read GPU temperature via nvidia-smi. Returns -1 on failure."""
    try:
        import subprocess
        out = subprocess.run(
            ["nvidia-smi", "--query-gpu=temperature.gpu", "--format=csv,noheader"],
            capture_output=True, text=True, timeout=5,
        )
        return int(out.stdout.strip())
    except Exception:
        return -1


@contextmanager
def gpu_operation(name: str, estimated_vram_mb: int = 0):
    """Context manager for GPU operations with serialization and cleanup.

    Usage:
        with gpu_operation("load_liveportrait", estimated_vram_mb=600):
            model = load_model()
            result = model(input)
        # VRAM automatically cleaned up on exit

    This ensures:
    1. Only one GPU operation runs at a time (prevents WSL2 contention)
    2. VRAM is checked before starting
    3. Cleanup runs on exit (even on exception)
    4. Memory stats are logged
    """
    with _gpu_lock:
        if estimated_vram_mb > 0:
            check_vram(estimated_vram_mb, name)

        stats_before = vram_stats()
        logger.info(
            "GPU op '%s' starting: %.0fMB allocated, %.0fMB free, %d°C",
            name,
            stats_before.get("allocated_mb", 0),
            stats_before.get("free_mb", 0),
            stats_before.get("temperature", -1),
        )
        try:
            yield
        finally:
            gpu_cleanup()
            stats_after = vram_stats()
            delta = stats_after.get("allocated_mb", 0) - stats_before.get("allocated_mb", 0)
            logger.info(
                "GPU op '%s' done: %.0fMB allocated (delta=%.0fMB), peak=%.0fMB",
                name,
                stats_after.get("allocated_mb", 0),
                delta,
                stats_after.get("peak_mb", 0),
            )


def safe_del(*tensors_or_models) -> None:
    """Safely delete tensors/models and free their VRAM immediately."""
    for obj in tensors_or_models:
        if obj is not None:
            del obj
    gc.collect()
    if torch.cuda.is_available():
        torch.cuda.empty_cache()


def log_vram(tag: str = "") -> None:
    """One-line VRAM log for debugging."""
    if torch.cuda.is_available():
        a = torch.cuda.memory_allocated() / 1e6
        r = torch.cuda.memory_reserved() / 1e6
        f = (torch.cuda.get_device_properties(0).total_memory - torch.cuda.memory_allocated()) / 1e6
        t = _gpu_temperature()
        logger.info("VRAM [%s]: alloc=%.0fMB res=%.0fMB free=%.0fMB temp=%d°C", tag, a, r, f, t)
