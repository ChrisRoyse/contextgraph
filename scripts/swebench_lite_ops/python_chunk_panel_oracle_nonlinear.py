#!/usr/bin/env python3
"""Nonlinear lower bound on I(panel;oracle) at chunk granularity (robustness check).

The screened+linear joint estimator gave LB=0.0, CLUB<=0.18 on diff-touched
chunks -> "insufficient under the linear model class". This confirms (or refutes)
that verdict with a NONLINEAR estimator that needs no marginal screening:
HistGradientBoosting captures interactions + nonlinearity across all 9600 panel
dims directly. LB = H(Y) - heldout_crossentropy is a valid MI lower bound for ANY
classifier (it is a function of X only).

Calibration: a PERMUTATION NULL (labels shuffled, X fixed) must yield LB ~ 0. If
the real LB > null LB by a clear margin, the panel carries that much decodable MI;
if real LB ~ null LB ~ 0, insufficiency is robust across linear AND nonlinear
classes. No mock data; real chunk panels; held-out evaluation only.

SoT: <out>/chunk_mi_nonlinear.json
"""
from __future__ import annotations

import argparse
import json
import math
from pathlib import Path

import numpy as np
from sklearn.ensemble import HistGradientBoostingClassifier
from sklearn.model_selection import train_test_split


def entropy_bits(y):
    p = float(np.mean(y))
    p = min(max(p, 1e-12), 1 - 1e-12)
    return -(p * math.log2(p) + (1 - p) * math.log2(1 - p))


def heldout_ce_bits(Xtr, ytr, Xho, yho, seed, shuffle_labels=False):
    if shuffle_labels:
        rng = np.random.default_rng(seed ^ 0xA11)
        ytr = rng.permutation(ytr)
    clf = HistGradientBoostingClassifier(
        max_iter=300, learning_rate=0.06, max_leaf_nodes=31,
        l2_regularization=1.0, early_stopping=True, validation_fraction=0.15,
        class_weight="balanced", random_state=seed,
    )
    clf.fit(Xtr, ytr)
    p = clf.predict_proba(Xho)[:, 1]
    p = np.clip(p, 1e-7, 1 - 1e-7)
    ce_nats = float(-np.mean(yho * np.log(p) + (1 - yho) * np.log(1 - p)))
    return ce_nats / math.log(2.0), clf


def lower_bound(X, y, label, seed, holdout=0.25):
    h = entropy_bits(y)
    Xtr, Xho, ytr, yho = train_test_split(
        X, y, test_size=holdout, random_state=seed, stratify=y
    )
    ce_real, clf = heldout_ce_bits(Xtr, ytr, Xho, yho, seed, shuffle_labels=False)
    ce_null, _ = heldout_ce_bits(Xtr, ytr, Xho, yho, seed, shuffle_labels=True)
    lb_real = max(0.0, h - ce_real)
    lb_null = max(0.0, h - ce_null)
    # held-out AUC as a second witness
    from sklearn.metrics import roc_auc_score
    p = np.clip(clf.predict_proba(Xho)[:, 1], 1e-7, 1 - 1e-7)
    auc = float(roc_auc_score(yho, p))
    return {
        "label": label,
        "n": int(X.shape[0]),
        "p": int(X.shape[1]),
        "target_entropy_bits": h,
        "heldout_ce_bits_real": ce_real,
        "heldout_ce_bits_null": ce_null,
        "nonlinear_lower_bound_bits_real": lb_real,
        "nonlinear_lower_bound_bits_null": lb_null,
        "lb_real_minus_null": lb_real - lb_null,
        "heldout_auc": auc,
        "estimator": "hist_gradient_boosting_heldout_ce",
    }


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--dataset-root", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--seed", type=int, default=20260608)
    ap.add_argument("--max-n", type=int, default=60000)
    args = ap.parse_args()

    root = Path(args.dataset_root)
    man = json.load(open(root / "chunk_panel_oracle_manifest.json"))
    data = np.load(root / "chunk_panel_oracle_dataset.npz")
    X, y, dt = data["X"], data["y"], data["diff_touched"]
    print(f"[nl] X={X.shape} difftouched={int(dt.sum())} pass={int(y.sum())} fail={int((y==0).sum())}", flush=True)

    results = []
    dti = np.where(dt == 1)[0]
    print(f"[nl] diff_touched n={dti.size} ...", flush=True)
    results.append(lower_bound(X[dti], y[dti], "diff_touched_nonlinear", args.seed))

    n = X.shape[0]
    if n > args.max_n:
        rng = np.random.default_rng(args.seed)
        pos = np.where(y == 1)[0]; neg = np.where(y == 0)[0]
        k_pos = int(args.max_n * pos.size / n); k_neg = args.max_n - k_pos
        sel = np.concatenate([rng.choice(pos, min(k_pos, pos.size), replace=False),
                              rng.choice(neg, min(k_neg, neg.size), replace=False)])
        print(f"[nl] all_chunks subsampled n={sel.size} ...", flush=True)
        results.append(lower_bound(X[sel], y[sel], "all_chunks_nonlinear", args.seed))

    summary = {
        "schema": "inv032_chunk_nonlinear_mi_v1",
        "dataset_root": str(root),
        "target_bits": 0.95,
        "results": results,
    }
    out = Path(args.out); out.mkdir(parents=True, exist_ok=True)
    sot = out / "chunk_mi_nonlinear.json"
    sot.write_text(json.dumps(summary, indent=2), encoding="utf-8")
    print(f"[nl] WROTE {sot}", flush=True)
    print("\nconfig                      n      | H(Y)   CE_real CE_null LB_real LB_null  AUC", flush=True)
    for r in results:
        print(
            f"{r['label']:<26} {r['n']:>6} | {r['target_entropy_bits']:.3f}  {r['heldout_ce_bits_real']:.3f}   "
            f"{r['heldout_ce_bits_null']:.3f}   {r['nonlinear_lower_bound_bits_real']:.3f}   "
            f"{r['nonlinear_lower_bound_bits_null']:.3f}    {r['heldout_auc']:.3f}",
            flush=True,
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
