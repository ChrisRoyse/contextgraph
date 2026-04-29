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
    "environment": [sys.executable, "scripts/exp2/verify_environment.py"],
    "corpus": [sys.executable, "scripts/exp2/build_corpus.py", "--phase", "all"],
    "embeddings": [sys.executable, "scripts/exp2/extract_embeddings.py"],
    "pca_embeddings": [
        sys.executable,
        "scripts/exp2/extract_embeddings.py",
        "--manifest",
        "artefacts/exp2/pca_training_corpus_v1.json",
        "--output",
        "artefacts/exp2/pca_training_embeddings.npz",
        "--storage-name",
        "pca",
    ],
    "projections": [sys.executable, "scripts/exp2/fit_projections.py", "--fit", "--apply"],
    "estimators": [sys.executable, "scripts/exp2/run_estimators.py"],
    "n_eff": [sys.executable, "scripts/exp2/effective_rank.py"],
    "visualize": [sys.executable, "scripts/exp2/visualize.py"],
    "fsv": [sys.executable, "scripts/exp2/fsv.py"],
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

    We collect all per-phase chains under artefacts/exp2/ and every contracted
    production artefact. Missing state is a hard error; otherwise the chain
    would certify an unreplayable run.
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
        if not path.exists():
            raise Exp2Error(f"Required phase audit chain missing for {phase}: {path}")
        chain.add_artefact(path, f"phase_chain:{phase}", created_by=f"phase:{phase}")

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
        ("spectrum_plot.png", "spectrum_plot_png"),
        ("spectrum_plot.pdf", "spectrum_plot_pdf"),
        ("entropy_bars.png", "entropy_bars_png"),
        ("entropy_bars.pdf", "entropy_bars_pdf"),
        ("per_pair_table.csv", "per_pair_table"),
        ("embedder_swap_recommendation.md", "embedder_swap_recommendation"),
        ("publishable_summary.md", "publishable_summary"),
        ("cross_validation_report.json", "cross_validation_report"),
        ("ksg_sensitivity_report.json", "ksg_sensitivity_report"),
    ]
    for rel, label in headline_artefacts:
        path = out / rel
        if not path.exists():
            raise Exp2Error(f"Required artefact missing before audit finalization: {path}")
        chain.add_artefact(path, label, created_by=label)

    pair_dir = out / "pair_results"
    pair_files = sorted(pair_dir.glob("E*_E*.json")) if pair_dir.exists() else []
    if len(pair_files) != 91:
        raise Exp2Error(f"Expected 91 pair result artefacts before audit finalization, found {len(pair_files)}")
    for path in pair_files:
        chain.add_artefact(path, f"pair_result:{path.stem}", created_by="T-016")

    chain.finalize(sha256_file(config_path))
    return master_path


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--config", default="scripts/exp2/exp2.yml")
    parser.add_argument("--phase", choices=list(PHASE_COMMANDS) + ["audit", "all"], default="all")
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
        "audit",
        "fsv",
    ]
    phases = order if args.phase == "all" else [args.phase]
    cfg = load_config(args.config)
    out = output_dir(cfg)
    for phase in phases:
        if phase == "audit":
            master = finalize_master_audit_chain(args.config)
            print(f"Master audit chain finalized: {master}", file=sys.stderr)
            continue
        chain_path: Path | None = None
        if phase in AUDIT_AWARE_PHASES:
            chain_path = out / f"audit_chain_{phase}.json"
        run(PHASE_COMMANDS[phase], args.config, chain_path)


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:
        print(f"EXP-2 orchestrator failed: {exc}", file=sys.stderr)
        raise SystemExit(1) from exc
