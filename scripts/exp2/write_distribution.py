#!/usr/bin/env python3
"""Generate measured-number distribution drafts after EXP-2 completes."""

from __future__ import annotations

import argparse
from pathlib import Path
import sys

if __package__ in {None, ""}:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from scripts.exp2.common import Exp2Error, load_config, output_dir, read_json, repo_path


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--config", default="scripts/exp2/exp2.yml")
    args = parser.parse_args()
    cfg = load_config(args.config)
    out = output_dir(cfg)
    summary_path = out / "n_eff_summary.json"
    if not summary_path.exists():
        raise Exp2Error("n_eff_summary.json is required before distribution drafts can be generated")
    summary = read_json(summary_path)
    h = summary["headline"]
    n_eff = f"{h['n_eff']:.3f}"
    ci = f"{h['ci_low']:.3f}-{h['ci_high']:.3f}"
    factor = f"{h['n_eff'] / 14.0:.2f}"
    paper = f"""# EXP-2 Paper Patch

## Section 3.1

The structural upper bound for the production 14-embedder panel is 105 signals per input. EXP-2 measures the redundancy-adjusted effective signal count as N_eff = {n_eff} (95% CI: {ci}), a {factor}x gain over the 14 marginal embedder signals.

## Section 6.1

Across all 91 embedder pairs, EXP-2 reports both InfoNCE and KSG estimates with bootstrap confidence intervals. The paper headline should replace unmeasured "up to 91" language with the measured 14-panel value above and cite `artefacts/exp2/per_pair_table.csv`, `artefacts/exp2/redundancy_heatmap.pdf`, and `artefacts/exp2/audit_chain.json`.
"""
    (out / "PAPER_PATCH.md").write_text(paper, encoding="utf-8")
    (repo_path("docs/exp2/youtube_script.md")).write_text(
        f"# YouTube Script\n\nOpen with the heatmap by T+0:30. Headline: N_eff = {n_eff} (95% CI {ci}), {factor}x over N=14.\n",
        encoding="utf-8",
    )
    (repo_path("docs/exp2/blog_post.md")).write_text(
        f"# We Measured How Much Information a 14-Embedder Memory System Actually Multiplies\n\nEXP-2 measured N_eff = {n_eff} (95% CI {ci}).\n",
        encoding="utf-8",
    )
    (repo_path("docs/exp2/tweet_thread.md")).write_text(
        f"1/ I measured my 14-embedder memory system. N_eff = {n_eff} (95% CI {ci}).\n",
        encoding="utf-8",
    )


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:
        raise SystemExit(f"EXP-2 distribution draft generation failed: {exc}") from exc
