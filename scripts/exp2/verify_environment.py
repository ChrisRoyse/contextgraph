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
from scripts.exp2.common import Exp2Error, load_config, output_dir, repo_path, setup_logging, sha256_file, write_json


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

    py = subprocess.check_output(["python3", "--version"], text=True).strip()
    check("python_version", py >= "Python 3.12", py)

    missing = [p for p in REQUIRED_PACKAGES if importlib.util.find_spec(p) is None]
    check("python_packages", not missing, {"missing": missing})

    try:
        smi = subprocess.check_output(
            ["nvidia-smi", "--query-gpu=name,memory.total,memory.free", "--format=csv,noheader"],
            text=True,
        ).strip()
    except Exception as exc:
        raise Exp2Error(f"nvidia-smi failed: {exc}") from exc
    gpu_rows = []
    for line in smi.splitlines():
        parts = [p.strip() for p in line.split(",")]
        if len(parts) != 3:
            continue
        name, total_raw, free_raw = parts
        total = int(total_raw.replace(" MiB", ""))
        free = int(free_raw.replace(" MiB", ""))
        gpu_rows.append({"name": name, "total_mib": total, "free_mib": free})
    ok_gpu = any(row["name"].endswith("RTX 5090") and row["total_mib"] >= 30000 and row["free_mib"] >= 30000 for row in gpu_rows)
    check("cuda_gpu_rtx5090_30gb_free", ok_gpu, gpu_rows)

    binary = repo_path(cfg["run"]["mcp_binary"])
    check("mcp_binary_exists", binary.exists(), str(binary))
    if not args.skip_cargo:
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
