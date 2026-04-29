#!/usr/bin/env python3
"""T-001 environment verification with strict failure reporting."""

from __future__ import annotations

import argparse
import importlib.util
import subprocess
from pathlib import Path
import sys

if __package__ in {None, ""}:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from scripts.exp2.audit_chain import AuditChain
from scripts.exp2.common import (
    EMBEDDER_KEYS,
    Exp2Error,
    canonical_json,
    load_config,
    output_dir,
    repo_path,
    setup_logging,
    sha256_bytes,
    sha256_file,
    write_json,
)


REQUIRED_PACKAGES = ["numpy", "scipy", "sklearn", "torch", "matplotlib", "seaborn", "yaml", "pandas"]
WEIGHT_FILES = [
    "models/semantic/model.safetensors",
    "models/causal/model.safetensors",
    "models/sparse/model.safetensors",
    "models/code-1536/model.safetensors.index.json",
    "models/graph/model.safetensors",
    "models/multimodal/model.safetensors",
    "models/entity/model.safetensors",
    "models/late-interaction/model.safetensors",
    "models/splade-v3/model.safetensors",
    "models/bge-m3-dense/model.safetensors",
]
LEARNED_EMBEDDER_WEIGHT_FILES = {
    "e1_semantic": "models/semantic/model.safetensors",
    "e5_causal_cause": "models/causal/model.safetensors",
    "e6_sparse_top_k": "models/sparse/model.safetensors",
    "e7_code": "models/code-1536/model.safetensors.index.json",
    "e8_graph_source": "models/graph/model.safetensors",
    "e10_paraphrase": "models/multimodal/model.safetensors",
    "e11_entity": "models/entity/model.safetensors",
    "e12_token_pooled": "models/late-interaction/model.safetensors",
    "e13_splade_top_k": "models/splade-v3/model.safetensors",
    "e14_bge_m3_dense": "models/bge-m3-dense/model.safetensors",
}
DETERMINISTIC_EMBEDDER_SPECS = {
    "e2_temporal_recent": "sinusoidal-positional-encoding:temporal-recent:v1",
    "e3_temporal_periodic": "fourier-temporal-periodic:v1",
    "e4_temporal_positional": "absolute-positional-encoding:v1",
    "e9_hdc": "hyperdimensional-computing-fixed-random-projection:v1",
}


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--config", default="scripts/exp2/exp2.yml")
    parser.add_argument("--skip-cargo", action="store_true")
    args = parser.parse_args()
    cfg = load_config(args.config)
    out = output_dir(cfg)
    log = setup_logging(out)
    report: dict[str, object] = {"checks": []}
    failures: list[str] = []

    def check(name: str, ok: bool, detail: object) -> None:
        report["checks"].append({"name": name, "ok": ok, "detail": detail})
        if not ok:
            failures.append(f"{name} failed: {detail}")

    py = subprocess.check_output([sys.executable, "--version"], text=True).strip()
    check("python_version", sys.version_info >= (3, 12), {"executable": sys.executable, "version": py})

    missing = [p for p in REQUIRED_PACKAGES if importlib.util.find_spec(p) is None]
    check("python_packages", not missing, {"missing": missing})

    gpu_rows = []
    try:
        smi = subprocess.check_output(
            ["nvidia-smi", "--query-gpu=name,memory.total,memory.free", "--format=csv,noheader"],
            text=True,
        ).strip()
        for line in smi.splitlines():
            parts = [p.strip() for p in line.split(",")]
            if len(parts) != 3:
                continue
            name, total_raw, free_raw = parts
            total = int(total_raw.replace(" MiB", ""))
            free = int(free_raw.replace(" MiB", ""))
            gpu_rows.append({"name": name, "total_mib": total, "free_mib": free})
        ok_gpu = any(
            row["name"].endswith("RTX 5090")
            and row["total_mib"] >= 30000
            and row["free_mib"] >= 30000
            for row in gpu_rows
        )
        check("cuda_gpu_rtx5090_30gb_free", ok_gpu, {"source_of_truth": "nvidia-smi", "rows": gpu_rows})
    except Exception as exc:
        check("cuda_gpu_rtx5090_30gb_free", False, {"source_of_truth": "nvidia-smi", "error": str(exc)})

    torch_probe: dict[str, object] = {"available": False, "devices": []}
    try:
        import torch

        torch_probe["available"] = bool(torch.cuda.is_available())
        devices = []
        if torch.cuda.is_available():
            for i in range(torch.cuda.device_count()):
                free_b, total_b = torch.cuda.mem_get_info(i)
                devices.append(
                    {
                        "device_index": i,
                        "name": torch.cuda.get_device_name(i),
                        "total_mib": total_b // (1024 * 1024),
                        "free_mib": free_b // (1024 * 1024),
                    }
                )
        torch_probe["devices"] = devices
    except Exception as exc:
        torch_probe["error"] = str(exc)
    report["torch_cuda_probe"] = torch_probe

    binary = repo_path(cfg["run"]["mcp_binary"])
    check("mcp_binary_exists", binary.exists(), str(binary))
    if args.skip_cargo:
        check("cargo_build_release_context_graph_mcp", False, "--skip-cargo is not valid for EXP-2 production verification")
    else:
        res = subprocess.run(
            ["cargo", "build", "--release", "-p", "context-graph-mcp"],
            cwd=repo_path("."),
            text=True,
            capture_output=True,
            check=False,
        )
        check("cargo_build_release_context_graph_mcp", res.returncode == 0, res.stderr[-4000:])

    hashes = {}
    for rel in WEIGHT_FILES:
        path = repo_path(rel)
        exists = path.exists()
        check(f"weight_present:{rel}", exists, str(path))
        if exists:
            hashes[rel] = sha256_file(path)
    report["weight_hashes"] = hashes
    embedder_hashes = {}
    for key in EMBEDDER_KEYS:
        if key in LEARNED_EMBEDDER_WEIGHT_FILES:
            rel = LEARNED_EMBEDDER_WEIGHT_FILES[key]
            if rel not in hashes:
                check(f"embedder_hash:{key}", False, {"weight_file": rel, "error": "weight hash missing"})
                continue
            embedder_hashes[key] = hashes[rel]
        elif key in DETERMINISTIC_EMBEDDER_SPECS:
            embedder_hashes[key] = sha256_bytes(
                canonical_json(
                    {
                        "embedder": key,
                        "frozen_spec": DETERMINISTIC_EMBEDDER_SPECS[key],
                        "code_commit_sha": AuditChain._git_commit(),
                    }
                )
            )
        else:
            check(f"embedder_hash:{key}", False, "no learned weight file or deterministic spec registered")
    missing_embedder_hashes = [key for key in EMBEDDER_KEYS if key not in embedder_hashes]
    check("all_14_embedder_hashes_logged", not missing_embedder_hashes, {"missing": missing_embedder_hashes})
    report["embedder_hashes"] = embedder_hashes

    report_path = out / "verification_log.json"
    write_json(report_path, report)
    chain = AuditChain(cfg["run_id"], out / "audit_chain_environment.json", cfg["run"]["random_seed"])
    chain.add_event("environment_verified", {"report": str(report_path)})
    chain.add_artefact(report_path, "verification_log", created_by="T-001")
    chain.finalize(sha256_file(args.config))
    if failures:
        log.error("Environment verification failed: %s", failures)
        raise Exp2Error("; ".join(failures))
    log.info("Environment verification passed: %s", report_path)


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:
        raise SystemExit(f"EXP-2 environment verification failed: {exc}") from exc
