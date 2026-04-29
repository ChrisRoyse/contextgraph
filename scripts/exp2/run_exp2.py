#!/usr/bin/env python3
"""Top-level EXP-2 orchestration CLI."""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

if __package__ in {None, ""}:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from scripts.exp2.common import Exp2Error, repo_path


PHASE_COMMANDS = {
    "environment": ["python3", "scripts/exp2/verify_environment.py"],
    "corpus": ["python3", "scripts/exp2/build_corpus.py", "--phase", "all"],
    "embeddings": ["python3", "scripts/exp2/extract_embeddings.py"],
    "pca_embeddings": [
        "python3",
        "scripts/exp2/extract_embeddings.py",
        "--manifest",
        "artefacts/exp2/pca_training_corpus_v1.json",
        "--output",
        "artefacts/exp2/pca_training_embeddings.npz",
        "--storage-name",
        "pca",
    ],
    "projections": ["python3", "scripts/exp2/fit_projections.py", "--fit", "--apply"],
    "estimators": ["python3", "scripts/exp2/run_estimators.py"],
    "n_eff": ["python3", "scripts/exp2/effective_rank.py"],
    "visualize": ["python3", "scripts/exp2/visualize.py"],
    "fsv": ["python3", "scripts/exp2/fsv.py"],
}


def run(cmd: list[str], config: str) -> None:
    full = cmd + ["--config", config] if "--config" not in cmd else cmd
    res = subprocess.run(full, cwd=repo_path("."), text=True)
    if res.returncode != 0:
        raise Exp2Error(f"Command failed ({res.returncode}): {' '.join(full)}")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--config", default="scripts/exp2/exp2.yml")
    parser.add_argument("--phase", choices=list(PHASE_COMMANDS) + ["all"], default="all")
    args = parser.parse_args()
    order = [
        "environment",
        "corpus",
        "embeddings",
        "pca_embeddings",
        "projections",
        "estimators",
        "n_eff",
        "visualize",
        "fsv",
    ]
    phases = order if args.phase == "all" else [args.phase]
    for phase in phases:
        run(PHASE_COMMANDS[phase], args.config)


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:
        print(f"EXP-2 orchestrator failed: {exc}", file=sys.stderr)
        raise SystemExit(1) from exc
