---
namespace: discoveries
agent_id: agent-s3-sherlock-scripts
timestamp: 2026-04-17T18:26:00Z
status: active
severity: medium
tags: phase-d, gemma, subprocess, orphan, cuda
---

# Phase D: Gemma unload only clears self._model/_tokenizer refs — no guarantee torch releases

## Evidence
`scripts/shakespeare_sft_augmenter.py:2077-2086`
```python
def unload(self) -> None:
    import torch
    log("unloading Gemma, clearing CUDA cache")
    self._model = None
    self._tokenizer = None
    gc.collect()
    if torch.cuda.is_available():
        torch.cuda.empty_cache()
        torch.cuda.synchronize()
    log("Gemma unloaded")
```

And called in main() at line 2761-2762:
```python
    finally:
        gemma.unload()
```

## Problems

### 1. torch holds references in functools/JIT caches
Setting `self._model = None` decrements one ref. But transformers
caches `_generation_config`, inference-mode context managers, and
autograd graph pieces. `gc.collect()` catches Python-level cycles,
but CUDA-side allocations live inside a caching allocator (THC/cuDNN
contexts). `torch.cuda.empty_cache()` returns free blocks BACK to the
driver, but only blocks not referenced. Some contexts persist until
process exit.

### 2. Daemon restart before VRAM actually frees
`main()` at 2779-2784:
```python
    log("PHASE 3: scoring (daemon UP)")
    if args.auto_daemon:
        start_daemon()
    else:
        if not port_is_listening(DAEMON_PORT):
            die(f"daemon is not listening on {DAEMON_PORT}; ...")
```

There's no assertion that VRAM is ACTUALLY freed. On a 32.6GB 5090:
- Gemma loaded = ~11.5GB
- If empty_cache didn't fully release, daemon start needs ~20.7GB
- Risk: 11.5 + 20.7 = 32.2GB — within 0.4GB of limit, crashes common

**Fix**: After unload, assert VRAM free:
```python
def unload(self) -> None:
    import torch
    self._model = None
    self._tokenizer = None
    gc.collect()
    if torch.cuda.is_available():
        torch.cuda.empty_cache()
        torch.cuda.synchronize()
        # Verify
        mem_reserved = torch.cuda.memory_reserved(0) / 1e9
        mem_allocated = torch.cuda.memory_allocated(0) / 1e9
        log(f"torch reserved={mem_reserved:.2f}GB allocated={mem_allocated:.2f}GB")
        if mem_allocated > 0.5:  # allow 500MB cached
            log(f"WARNING: torch still holding {mem_allocated:.2f}GB — "
                f"daemon restart may fail. Consider: exec python -c 'sys.exit(0)'",
                level="WARN")
```

For stubborn CUDA contexts, the only bulletproof release is process
exit. The SFT augmenter SHOULD exec a child subprocess for Gemma
phase, then return to parent for scoring phase. That pattern:

```python
# Parent process: Phase 1 (no GPU)
# subprocess.run([python, __file__, --internal-phase=2, ...]) — Gemma in child
# Child exits → VRAM fully released to driver
# Parent: start daemon, Phase 3
```

### 3. gemma.unload() runs in finally but not on SIGKILL/OOM
If OOM-killer sends SIGKILL during Gemma load/generate, `finally:
gemma.unload()` never runs. Process dies with VRAM still held. Host
sees this as a "zombie CUDA context" — nvidia-smi shows the PID as
"[Not Found]" but memory still attributed. Recovery requires `nvidia-smi
--gpu-reset` (not always possible on WSL) or driver reload.

This is the SAME failure mode as the crash reported in
`blockers/agent-04-coder-analyzer--oom-kills-daemon.md`.

## Fix Recommendation
**Strongly recommend Phase 2 run in a subprocess** to guarantee VRAM
release. Use:
```python
import subprocess, shlex
cmd = [sys.executable, __file__, "--internal-phase=generate-only",
       "--candidates-out", str(sft_candidates), ...]
r = subprocess.run(cmd, check=False)
if r.returncode != 0:
    die(f"generation subprocess failed with rc={r.returncode}")
# When subprocess returns, its VRAM is GUARANTEED released
```

Add `--internal-phase=generate-only` that does Phase 2 and exits.

## Related
- `./memory/blockers/agent-04-coder-analyzer--oom-kills-daemon.md`
- Tags: phase-d, gemma, subprocess, cuda, vram-leak
