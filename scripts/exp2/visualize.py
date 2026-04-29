#!/usr/bin/env python3
"""T-022..T-023 visualization and table deliverables."""

from __future__ import annotations

import argparse
import csv
import itertools
from pathlib import Path
import sys

import matplotlib.pyplot as plt
import numpy as np
import seaborn as sns
from matplotlib.colors import LogNorm

if __package__ in {None, ""}:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from scripts.exp2.common import EMBEDDER_LABELS, Exp2Error, load_config, output_dir, read_json, setup_logging
from scripts.exp2.effective_rank import load_pair_rows, mi_matrix_from_rows


def render_heatmap(mi: np.ndarray, out_dir) -> None:
    positive = mi[mi > 0]
    if positive.size == 0:
        raise Exp2Error("Heatmap MI matrix has no positive values")
    plt.figure(figsize=(14, 12))
    sns.heatmap(
        mi,
        xticklabels=EMBEDDER_LABELS,
        yticklabels=EMBEDDER_LABELS,
        cmap="viridis",
        norm=LogNorm(vmin=max(float(positive.min()), 1e-6), vmax=float(positive.max())),
        cbar_kws={"label": "nats"},
        square=True,
    )
    plt.title("Pairwise mutual information across the 14-embedder Context Graph panel")
    plt.xticks(rotation=45, ha="right", fontsize=8)
    plt.yticks(rotation=0, fontsize=8)
    plt.tight_layout()
    plt.savefig(out_dir / "redundancy_heatmap.png", dpi=300)
    plt.savefig(out_dir / "redundancy_heatmap.pdf")
    plt.close()


def render_spectrum(eigenvalues: list[float], out_dir) -> None:
    ev = np.abs(np.asarray(eigenvalues, dtype=np.float64))
    threshold = 0.05 * float(ev.max())
    plt.figure(figsize=(8, 5))
    plt.bar(np.arange(1, len(ev) + 1), np.sort(ev)[::-1], color="#2a9d8f")
    plt.axhline(threshold, color="#d62828", linestyle="--", label="0.05 max threshold")
    plt.yscale("log")
    plt.xlabel("rank index")
    plt.ylabel("eigenvalue magnitude")
    plt.legend()
    plt.tight_layout()
    plt.savefig(out_dir / "spectrum_plot.png", dpi=300)
    plt.savefig(out_dir / "spectrum_plot.pdf")
    plt.close()


def render_entropy(entropy_report: dict, out_dir) -> None:
    entries = entropy_report["entries"]
    values = np.asarray([e["entropy_nats"] for e in entries])
    lows = np.asarray([e["ci_low"] for e in entries])
    highs = np.asarray([e["ci_high"] for e in entries])
    plt.figure(figsize=(12, 5))
    plt.bar(np.arange(14), values, color="#457b9d")
    plt.errorbar(np.arange(14), values, yerr=[values - lows, highs - values], fmt="none", color="black", capsize=2)
    plt.xticks(np.arange(14), [f"E{i}" for i in range(1, 15)])
    plt.ylabel("entropy (nats)")
    plt.tight_layout()
    plt.savefig(out_dir / "entropy_bars.png", dpi=300)
    plt.savefig(out_dir / "entropy_bars.pdf")
    plt.close()


def write_pair_table(rows: dict[str, dict], out_dir) -> None:
    csv_path = out_dir / "per_pair_table.csv"
    fields = [
        "j",
        "k",
        "mi_infonce_nats",
        "mi_infonce_ci_low",
        "mi_infonce_ci_high",
        "mi_ksg_nats",
        "mi_ksg_ci_low",
        "mi_ksg_ci_high",
        "agreement_nats",
        "agreement_pass",
        "recommendation",
        "training_diagnostic",
    ]
    with csv_path.open("w", newline="", encoding="utf-8") as fh:
        writer = csv.DictWriter(fh, fieldnames=fields)
        writer.writeheader()
        for key in sorted(rows):
            row = rows[key]
            rec = recommendation(row)
            writer.writerow(
                {
                    "j": row["embedder_j"],
                    "k": row["embedder_k"],
                    "mi_infonce_nats": row["infonce_estimate_nats"],
                    "mi_infonce_ci_low": row["infonce_ci_low"],
                    "mi_infonce_ci_high": row["infonce_ci_high"],
                    "mi_ksg_nats": row["ksg_estimate_nats"],
                    "mi_ksg_ci_low": row["ksg_ci_low"],
                    "mi_ksg_ci_high": row["ksg_ci_high"],
                    "agreement_nats": row["agreement_nats"],
                    "agreement_pass": row["agreement_pass"],
                    "recommendation": rec,
                    "training_diagnostic": "converged" if row["infonce_train_val_gap"] <= 0.1 else "overfit",
                }
            )


def recommendation(row: dict) -> str:
    mi = float(row["infonce_estimate_nats"])
    if mi >= 1.0:
        return "high redundancy"
    if mi >= 0.5:
        return "consider swap"
    return "keep both"


def write_swap_recommendation(rows: dict[str, dict], out_dir) -> None:
    top = sorted(rows.values(), key=lambda r: float(r["infonce_estimate_nats"]), reverse=True)[:5]
    lines = ["# Embedder Swap Recommendations", "", "Based on EXP-2 pairwise MI measurements:", ""]
    for i, row in enumerate(top, 1):
        lines.append(f"{i}. {row['embedder_j']} <-> {row['embedder_k']}: MI = {row['infonce_estimate_nats']:.4f} nats.")
        lines.append(f"   Recommendation: {recommendation(row)}.")
    lines.append("")
    (out_dir / "embedder_swap_recommendation.md").write_text("\n".join(lines), encoding="utf-8")


def write_publishable_summary(summary: dict, rows: dict[str, dict], out_dir) -> None:
    headline = summary["headline"]
    high = max(rows.values(), key=lambda r: float(r["infonce_estimate_nats"]))
    low = min(rows.values(), key=lambda r: float(r["infonce_estimate_nats"]))
    text = f"""# EXP-2 Publishable Summary

## Headline
The 14-embedder Context Graph panel produces N_eff = {headline['n_eff']:.3f} (CI: {headline['ci_low']:.3f}-{headline['ci_high']:.3f}) effective independent supervisory signals per input.

## Method
- 800 inputs, 8 strata.
- All 91 embedder pairs, InfoNCE and KSG estimators.
- Bootstrap resamples per exp2.yml, seed 42.

## Key Pairs
- Highest MI: {high['pair']} ({high['infonce_estimate_nats']:.4f} nats).
- Lowest MI: {low['pair']} ({low['infonce_estimate_nats']:.4f} nats).
"""
    (out_dir / "publishable_summary.md").write_text(text, encoding="utf-8")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--config", default="scripts/exp2/exp2.yml")
    args = parser.parse_args()
    cfg = load_config(args.config)
    out = output_dir(cfg)
    log = setup_logging(out)
    rows = load_pair_rows(out)
    entropy = read_json(out / "entropy_per_embedder.json")
    summary = read_json(out / "n_eff_summary.json")
    mi = mi_matrix_from_rows(rows, "infonce")
    for i, entry in enumerate(entropy["entries"]):
        mi[i, i] = entry["entropy_nats"]
    render_heatmap(mi, out)
    render_spectrum(summary["eigenvalues"], out)
    render_entropy(entropy, out)
    write_pair_table(rows, out)
    write_swap_recommendation(rows, out)
    write_publishable_summary(summary, rows, out)
    log.info("Rendered EXP-2 deliverables into %s", out)


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:
        raise SystemExit(f"EXP-2 visualization step failed: {exc}") from exc
