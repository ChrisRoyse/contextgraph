#!/usr/bin/env python3
"""Top-level EXP-2 orchestration CLI."""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

if __package__ in {None, ""}:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from scripts.exp2.audit_chain import AuditChain
from scripts.exp2.common import Exp2Error, load_config, output_dir, repo_path, sha256_file


# Phases that accept `--audit-chain <path>` and append events/artefacts to it.
# verify_environment already writes its own chain at audit_chain_environment.json.
AUDIT_AWARE_PHASES = {"estimators", "n_eff"}

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


def run(cmd: list[str], config: str, audit_chain_path: Path | None = None) -> None:
    full = list(cmd)
    if "--config" not in full:
        full = full + ["--config", config]
    if audit_chain_path is not None:
        full = full + ["--audit-chain", str(audit_chain_path)]
    res = subprocess.run(full, cwd=repo_path("."), text=True)
    if res.returncode != 0:
        raise Exp2Error(f"Command failed ({res.returncode}): {' '.join(full)}")


def finalize_master_audit_chain(config_path: str) -> Path:
    """Per Constitution C7 + docs/exp2/08_output_deliverables.md §8: a master
    audit_chain.json carrying chain_sha256 must ship with every run.

    We collect all per-phase chains under artefacts/exp2/ (verify_environment
    writes audit_chain_environment.json; estimators and n_eff write their own
    when --audit-chain is forwarded). The master chain hashes each per-phase
    chain file as a primary artefact, plus the headline deliverables that the
    paper/blog/tweet ingest. Missing per-phase chains do not raise; instead
    they appear in the master events log so the dangling reference is visible.
    """
    cfg = load_config(config_path)
    out = output_dir(cfg)
    master_path = out / "audit_chain.json"
    chain = AuditChain(cfg["run_id"], master_path, cfg["run"]["random_seed"])

    # Per-phase chains. Order mirrors PHASE_COMMANDS for narrative continuity.
    per_phase_chain_files = [
        ("environment", "audit_chain_environment.json"),
        ("estimators", "audit_chain_estimators.json"),
        ("n_eff", "audit_chain_n_eff.json"),
    ]
    for phase, rel in per_phase_chain_files:
        path = out / rel
        if path.exists():
            chain.add_artefact(path, f"phase_chain:{phase}", created_by=f"phase:{phase}")
        else:
            chain.add_event("phase_chain_missing", {"phase": phase, "expected": str(path)})

    # Headline deliverables per docs/exp2/08_output_deliverables.md.
    headline_artefacts = [
        ("corpus_manifest_v1.json", "corpus_manifest"),
        ("verification_log.json", "verification_log"),
        ("pca_models.npz", "pca_models"),
        ("pca_validation_report.json", "pca_validation_report"),
        ("embeddings_per_input.npz", "embeddings_per_input"),
        ("projected.npz", "projected"),
        ("entropy_per_embedder.json", "entropy_per_embedder"),
        ("n_eff_summary.json", "n_eff_summary"),
        ("redundancy_heatmap.png", "redundancy_heatmap_png"),
        ("redundancy_heatmap.pdf", "redundancy_heatmap_pdf"),
        ("per_pair_table.csv", "per_pair_table"),
    ]
    for rel, label in headline_artefacts:
        path = out / rel
        if path.exists():
            chain.add_artefact(path, label, created_by=label)
        else:
            chain.add_event("artefact_missing", {"label": label, "expected": str(path)})

    pair_dir = out / "pair_results"
    if pair_dir.exists():
        for path in sorted(pair_dir.glob("E*_E*.json")):
            chain.add_artefact(path, f"pair_result:{path.stem}", created_by="T-016")

    chain.finalize(sha256_file(config_path))
    return master_path


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
    cfg = load_config(args.config)
    out = output_dir(cfg)
    for phase in phases:
        chain_path: Path | None = None
        if phase in AUDIT_AWARE_PHASES:
            chain_path = out / f"audit_chain_{phase}.json"
        run(PHASE_COMMANDS[phase], args.config, chain_path)
    # Always finalize the master chain so that even partial runs leave a hash trail.
    master = finalize_master_audit_chain(args.config)
    print(f"Master audit chain finalized: {master}", file=sys.stderr)


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:
        print(f"EXP-2 orchestrator failed: {exc}", file=sys.stderr)
        raise SystemExit(1) from exc
