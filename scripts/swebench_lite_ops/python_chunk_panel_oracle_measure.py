#!/usr/bin/env python3
"""Measure I(panel;oracle) at CHUNK granularity with the powered joint estimator.

Loads the chunk-level dataset (native 9600-d panel, slot identity preserved) and
runs screened_panel_joint_lower_bound + its pluggable power gate on:
  - all candidate chunks (max n, but diluted by unchanged chunks)
  - diff-touched candidate chunks only (signal-dense; mean-pool dilution removed)

The decisive question: does either configuration cross the power floor (so the
verdict is meaningful), and what is the lower bound / CLUB bracket?

SoT: <out>/chunk_mi_measure.json on aiwonder /zfs/archive.
"""
from __future__ import annotations

import argparse
import importlib.util
import json
import sys
from pathlib import Path


def load_probe(p):
    spec = importlib.util.spec_from_file_location("probe", str(p))
    m = importlib.util.module_from_spec(spec)
    sys.modules["probe"] = m
    spec.loader.exec_module(m)
    return m


def measure(probe, X, y, slices, label, seed, holdout, cross_fit):
    import numpy as np

    j = probe.screened_panel_joint_lower_bound(
        X=X, y=y, feature_slices=slices, label=label, seed=seed,
        holdout_fraction=holdout, cross_fit=cross_fit,
    )
    if not j.get("valid", False):
        return {"label": label, "n": int(X.shape[0]), "valid": False, "invalid_reason": j.get("invalid_reason")}
    g = probe.power_calibration_gate(
        X=X, y=y, seed=seed, holdout_fraction=holdout,
        estimator=probe.screened_panel_joint_lower_bound,
        estimator_kwargs={"feature_slices": slices, "cross_fit": cross_fit},
    )
    dec = probe._sufficiency_decision(
        target_bits=0.95,
        lower_bound_bits=j.get("lower_bound_bits"),
        upper_bound_bits=j.get("club_upper_bound_bits"),
        estimator_underpowered=bool(g.get("estimator_underpowered")),
    )
    return {
        "label": label,
        "n": int(X.shape[0]),
        "p": int(X.shape[1]),
        "valid": True,
        "cross_fit": cross_fit,
        "lower_bound_bits": j.get("lower_bound_bits"),
        "club_upper_bound_bits": j.get("club_upper_bound_bits"),
        "selected_feature_count": j.get("selected_feature_count"),
        "target_entropy_bits": j.get("target_entropy_bits"),
        "power_recovered_bits": g.get("recovered_bits"),
        "power_floor_bits": g.get("detectable_floor_bits"),
        "estimator_underpowered": g.get("estimator_underpowered"),
        "classification": dec.get("classification"),
    }


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--probe", required=True)
    ap.add_argument("--dataset-root", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--seed", type=int, default=20260608)
    ap.add_argument("--holdout", type=float, default=0.25)
    ap.add_argument("--max-n", type=int, default=60000, help="subsample cap for the all-chunks config")
    args = ap.parse_args()

    import numpy as np

    probe = load_probe(Path(args.probe))
    root = Path(args.dataset_root)
    man = json.load(open(root / "chunk_panel_oracle_manifest.json"))
    data = np.load(root / "chunk_panel_oracle_dataset.npz")
    X, y, dt = data["X"], data["y"], data["diff_touched"]
    slices = {e: slice(int(v["start"]), int(v["stop"])) for e, v in man["feature_slices"].items()}
    print(f"[measure] X={X.shape} difftouched={int(dt.sum())}/{X.shape[0]} pass={int(y.sum())} fail={int((y==0).sum())}", flush=True)

    results = []
    cf = True  # n is large -> cross-fit affordable

    # diff-touched only
    dti = np.where(dt == 1)[0]
    if dti.size >= 40:
        print(f"[measure] diff_touched n={dti.size} ...", flush=True)
        results.append(measure(probe, X[dti], y[dti], slices, "diff_touched", args.seed, args.holdout, cf))

    # all chunks (subsample if huge, stratified)
    n = X.shape[0]
    if n > args.max_n:
        rng = np.random.default_rng(args.seed)
        pos = np.where(y == 1)[0]; neg = np.where(y == 0)[0]
        k_pos = int(args.max_n * pos.size / n); k_neg = args.max_n - k_pos
        sel = np.concatenate([rng.choice(pos, size=min(k_pos, pos.size), replace=False),
                              rng.choice(neg, size=min(k_neg, neg.size), replace=False)])
        rng.shuffle(sel)
        print(f"[measure] all_chunks subsampled n={sel.size} (from {n}) ...", flush=True)
        results.append(measure(probe, X[sel], y[sel], slices, f"all_chunks_subsampled_{sel.size}", args.seed, args.holdout, cf))
    else:
        print(f"[measure] all_chunks n={n} ...", flush=True)
        results.append(measure(probe, X, y, slices, "all_chunks", args.seed, args.holdout, cf))

    summary = {
        "schema": "inv032_chunk_granularity_mi_v1",
        "dataset_root": str(root),
        "panel_dim": man["panel_dim"],
        "diff_touched_label_counts": man.get("diff_touched_label_counts"),
        "all_label_counts": man.get("label_counts"),
        "seed": args.seed,
        "target_bits": 0.95,
        "results": results,
    }
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    sot = out / "chunk_mi_measure.json"
    sot.write_text(json.dumps(summary, indent=2), encoding="utf-8")
    print(f"[measure] WROTE {sot}", flush=True)
    print("\nconfig                       n      p     | under?  LB     CLUB   sel   verdict", flush=True)
    for r in results:
        if not r.get("valid"):
            print(f"{r['label']:<28} {r.get('n'):>6}  INVALID {r.get('invalid_reason')}", flush=True)
            continue
        print(
            f"{r['label']:<28} {r['n']:>6} {r['p']:>6} | {str(r['estimator_underpowered']):>6} "
            f"{(r['lower_bound_bits'] or 0):>6.3f} {(r['club_upper_bound_bits'] or 0):>6.3f} "
            f"{str(r['selected_feature_count']):>4}  {r['classification']}",
            flush=True,
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
