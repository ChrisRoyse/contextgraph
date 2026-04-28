#!/usr/bin/env python3
"""Full-state verification for the WSL CUDA 13 / RTX 5090 environment."""
from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import tempfile
import time
import traceback
from pathlib import Path
from typing import Any

sys.dont_write_bytecode = True

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from v4_2.cuda13_blackwell import (  # noqa: E402
    apply_preimport_cuda_env,
    configure_torch_cuda,
    release_torch_cuda,
)

apply_preimport_cuda_env()


DEFAULT_OUTPUT = Path("~/.contextgraph/reports/cuda13_wsl_fsv_latest.json").expanduser()
DEFAULT_EXPECTED_PYTHON = Path("/home/cabdru/miniconda3/bin/python3")


class FsvError(RuntimeError):
    pass


def run_cmd(cmd: list[str], timeout: float = 30.0) -> dict[str, Any]:
    try:
        proc = subprocess.run(
            cmd,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=timeout,
            check=False,
        )
    except Exception as exc:
        return {
            "cmd": cmd,
            "ok": False,
            "error": f"{type(exc).__name__}: {exc}",
        }
    return {
        "cmd": cmd,
        "ok": proc.returncode == 0,
        "returncode": proc.returncode,
        "stdout": proc.stdout.replace("\x00", ""),
        "stderr": proc.stderr.replace("\x00", ""),
    }


def path_state(path: Path) -> dict[str, Any]:
    path = path.expanduser()
    if not path.exists():
        return {"path": str(path), "exists": False}
    return {
        "path": str(path),
        "exists": True,
        "is_file": path.is_file(),
        "is_dir": path.is_dir(),
        "size_bytes": path.stat().st_size if path.is_file() else None,
    }


def pip_versions() -> dict[str, str]:
    import importlib.metadata as md

    names = [
        "torch",
        "torchvision",
        "torchaudio",
        "triton",
        "transformers",
        "accelerate",
        "peft",
        "trl",
        "bitsandbytes",
        "datasets",
        "sentence-transformers",
        "xformers",
        "nvidia-ml-py",
    ]
    versions = {}
    for name in names:
        try:
            versions[name] = md.version(name)
        except md.PackageNotFoundError:
            versions[name] = "<not installed>"
    return versions


def apt_cuda_policy() -> dict[str, str]:
    out = run_cmd(["apt-cache", "policy", "cuda-toolkit-13-2"], timeout=10.0)
    if not out["ok"]:
        raise FsvError(f"apt-cache policy failed: {out}")
    return {"cuda-toolkit-13-2": out["stdout"]}


def python_runtime(args: argparse.Namespace) -> dict[str, Any]:
    expected = args.expected_python.expanduser()
    actual = Path(sys.executable)
    if actual != expected and actual.resolve() != expected.resolve():
        raise FsvError(
            f"wrong Python interpreter: expected {expected} "
            f"(resolved {expected.resolve()}), got {actual} "
            f"(resolved {actual.resolve()})"
        )
    return {
        "executable": str(actual),
        "resolved_executable": str(actual.resolve()),
        "version": sys.version,
        "prefix": sys.prefix,
        "base_prefix": sys.base_prefix,
        "expected_executable": str(expected),
        "expected_resolved_executable": str(expected.resolve()),
    }


def cuda_benchmarks(torch: Any, matmul_size: int) -> dict[str, Any]:
    if matmul_size <= 0:
        raise FsvError(f"matmul_size must be positive, got {matmul_size}")
    if matmul_size > 8192:
        raise FsvError(f"matmul_size too large for FSV guardrail: {matmul_size}")

    device = "cuda"
    torch.cuda.reset_peak_memory_stats()
    a = torch.randn((matmul_size, matmul_size), device=device, dtype=torch.bfloat16)
    b = torch.randn((matmul_size, matmul_size), device=device, dtype=torch.bfloat16)
    _ = a @ b
    torch.cuda.synchronize()
    t0 = time.perf_counter()
    c = a @ b
    torch.cuda.synchronize()
    matmul_ms = (time.perf_counter() - t0) * 1000.0
    matmul_mean = float(c.float().mean().cpu())
    del a, b, c
    torch.cuda.empty_cache()

    batch, heads, seq, dim = 2, 8, 512, 128
    q = torch.randn((batch, heads, seq, dim), device=device, dtype=torch.bfloat16)
    k = torch.randn((batch, heads, seq, dim), device=device, dtype=torch.bfloat16)
    v = torch.randn((batch, heads, seq, dim), device=device, dtype=torch.bfloat16)
    _ = torch.nn.functional.scaled_dot_product_attention(q, k, v)
    torch.cuda.synchronize()
    t1 = time.perf_counter()
    attn = torch.nn.functional.scaled_dot_product_attention(q, k, v)
    torch.cuda.synchronize()
    sdpa_ms = (time.perf_counter() - t1) * 1000.0
    sdpa_mean = float(attn.float().mean().cpu())
    del q, k, v, attn

    return {
        "bf16_matmul": {
            "size": matmul_size,
            "elapsed_ms_after_warmup": matmul_ms,
            "mean": matmul_mean,
        },
        "bf16_sdpa": {
            "shape": [batch, heads, seq, dim],
            "elapsed_ms_after_warmup": sdpa_ms,
            "mean": sdpa_mean,
        },
        "peak_allocated_gib": torch.cuda.max_memory_allocated() / (1024 ** 3),
    }


def build_report(args: argparse.Namespace) -> dict[str, Any]:
    python_info = python_runtime(args)

    import torch

    cuda_info = configure_torch_cuda(
        torch,
        min_total_vram_gib=args.min_vram_gib,
        label="cuda13-wsl-fsv",
    )
    if os.environ.get("CUDA_HOME") != "/usr/local/cuda-13.2":
        raise FsvError(f"CUDA_HOME must be /usr/local/cuda-13.2, got {os.environ.get('CUDA_HOME')}")

    wsl_exe = Path("/mnt/c/Windows/System32/wsl.exe")
    powershell = Path("/mnt/c/Windows/System32/WindowsPowerShell/v1.0/powershell.exe")
    nvidia_smi = run_cmd(["nvidia-smi"], timeout=10.0)
    nvcc = run_cmd(["nvcc", "--version"], timeout=10.0)
    if not nvidia_smi["ok"]:
        raise FsvError(f"nvidia-smi failed: {nvidia_smi}")
    if not nvcc["ok"] or "release 13.2" not in nvcc["stdout"]:
        raise FsvError(f"nvcc is not CUDA 13.2: {nvcc}")

    return {
        "schema_version": "1.0",
        "created_at_unix": time.time(),
        "source_of_truth": {
            "report_path": str(args.output.expanduser()),
            "gpu_runtime": "nvidia-smi + torch.cuda + persisted JSON report",
            "python_runtime": str(args.expected_python.expanduser()),
        },
        "python": python_info,
        "host": {
            "uname": run_cmd(["uname", "-a"]),
            "os_release": Path("/etc/os-release").read_text(encoding="utf-8"),
            "wsl_conf": Path("/etc/wsl.conf").read_text(encoding="utf-8"),
            "wsl_status": run_cmd([str(wsl_exe), "--status"]) if wsl_exe.exists() else {"ok": False, "error": "wsl.exe missing"},
            "wsl_version": run_cmd([str(wsl_exe), "--version"]) if wsl_exe.exists() else {"ok": False, "error": "wsl.exe missing"},
            "windows_gpu": run_cmd([
                str(powershell),
                "-NoProfile",
                "-Command",
                "nvidia-smi.exe --query-gpu=name,driver_version,memory.total --format=csv,noheader",
            ]) if powershell.exists() else {"ok": False, "error": "powershell.exe missing"},
        },
        "cuda": {
            "nvidia_smi": nvidia_smi,
            "nvcc": nvcc,
            "apt_policy": apt_cuda_policy(),
            "env": {
                key: os.environ.get(key)
                for key in [
                    "CUDA_HOME",
                    "CUDA_PATH",
                    "LD_LIBRARY_PATH",
                    "TORCH_CUDA_ARCH_LIST",
                    "PYTORCH_CUDA_ALLOC_CONF",
                    "CUDA_MODULE_LOADING",
                    "CUDA_VISIBLE_DEVICES",
                ]
            },
            "torch": cuda_info,
            "benchmarks": cuda_benchmarks(torch, args.matmul_size),
        },
        "python_packages": pip_versions(),
    }


def write_json_atomic(path: Path, data: dict[str, Any]) -> None:
    path = path.expanduser()
    path.parent.mkdir(parents=True, exist_ok=True)
    partial = path.with_suffix(path.suffix + ".partial")
    try:
        with partial.open("w", encoding="utf-8") as fh:
            json.dump(data, fh, indent=2, sort_keys=True)
            fh.write("\n")
            fh.flush()
            os.fsync(fh.fileno())
        os.replace(partial, path)
    except BaseException:
        try:
            partial.unlink(missing_ok=True)
        finally:
            raise


def readback_verify(path: Path) -> dict[str, Any]:
    path = path.expanduser()
    if not path.exists():
        raise FsvError(f"report was not written: {path}")
    with path.open("r", encoding="utf-8") as fh:
        report = json.load(fh)
    torch_info = report.get("cuda", {}).get("torch", {})
    if torch_info.get("compute_capability") != [12, 0]:
        raise FsvError(f"readback compute_capability mismatch: {torch_info}")
    if "sm_120" not in torch_info.get("arch_list", []):
        raise FsvError(f"readback arch_list missing sm_120: {torch_info}")
    if report.get("python_packages", {}).get("torch") != "2.11.0+cu130":
        raise FsvError("readback torch package version mismatch")
    python_info = report.get("python", {})
    if python_info.get("resolved_executable") != python_info.get("expected_resolved_executable"):
        raise FsvError(f"readback python runtime mismatch: {python_info}")
    return {
        "path": str(path),
        "size_bytes": path.stat().st_size,
        "python": report["python"]["executable"],
        "torch": report["python_packages"]["torch"],
        "transformers": report["python_packages"]["transformers"],
        "trl": report["python_packages"]["trl"],
        "compute_capability": torch_info["compute_capability"],
        "arch_list_contains_sm_120": "sm_120" in torch_info["arch_list"],
    }


def print_json(label: str, obj: Any) -> None:
    print(f"\n=== {label} ===")
    print(json.dumps(obj, indent=2, sort_keys=True))


def run_fsv(args: argparse.Namespace) -> int:
    before = path_state(args.output)
    print_json("SOURCE OF TRUTH", {
        "report": str(args.output.expanduser()),
        "runtime": "torch.cuda + nvidia-smi + nvcc + WSL status",
    })
    print_json("HAPPY PATH BEFORE", before)
    report = build_report(args)
    write_json_atomic(args.output, report)
    verified = readback_verify(args.output)
    print_json("HAPPY PATH AFTER", {"report": path_state(args.output), "verified": verified})

    with tempfile.TemporaryDirectory(prefix="contextgraph-cuda13-fsv.") as tmpdir_s:
        tmpdir = Path(tmpdir_s)

        def expected_failure(label: str, **overrides: Any) -> None:
            edge = argparse.Namespace(**vars(args))
            for key, value in overrides.items():
                setattr(edge, key, value)
            print_json(f"{label} BEFORE", {"output": path_state(edge.output), "canonical": path_state(args.output)})
            try:
                report = build_report(edge)
                write_json_atomic(edge.output, report)
            except Exception as exc:
                print_json(f"{label} FAILURE", {
                    "expected_failure": True,
                    "error": f"{type(exc).__name__}: {exc}",
                })
            else:
                raise FsvError(f"{label}: expected failure but command succeeded")
            print_json(f"{label} AFTER", {"output": path_state(edge.output), "canonical": path_state(args.output)})

        expected_failure(
            "EDGE 1 INVALID MATMUL SIZE",
            output=tmpdir / "bad_matmul.json",
            matmul_size=0,
        )
        expected_failure(
            "EDGE 2 IMPOSSIBLE VRAM REQUIREMENT",
            output=tmpdir / "bad_vram.json",
            min_vram_gib=128.0,
        )
        expected_failure(
            "EDGE 3 BAD OUTPUT DIRECTORY",
            output=tmpdir,
            matmul_size=64,
        )

    print_json("FSV CLEANUP", {
        "temporary_directory_removed": not Path(tmpdir_s).exists(),
        "canonical_report": path_state(args.output),
    })
    return 0


def parse_args(argv: list[str]) -> argparse.Namespace:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    ap.add_argument("--matmul-size", type=int, default=4096)
    ap.add_argument("--min-vram-gib", type=float, default=31.0)
    ap.add_argument("--expected-python", type=Path, default=DEFAULT_EXPECTED_PYTHON)
    return ap.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(sys.argv[1:] if argv is None else argv)
    try:
        return run_fsv(args)
    except Exception as exc:
        print(f"cuda13_wsl_fsv failed: {type(exc).__name__}: {exc}", file=sys.stderr)
        print(traceback.format_exc(), file=sys.stderr)
        return 1
    finally:
        torch_module = sys.modules.get("torch")
        if torch_module is not None:
            try:
                release_torch_cuda(torch_module)
            except Exception:
                print(traceback.format_exc(), file=sys.stderr)
                return 1


if __name__ == "__main__":
    raise SystemExit(main())
