#!/usr/bin/env python3
"""Powered MI measurement of CURRENT execution-trace features vs oracle (#774).

The prior INV-010 "no lift" falsification was measured on the ship-gate
calibration(240)/holdout(30) split via a ridge correlation-lift metric -- the
SAME underpowered regime proven below the detection floor in the chunk-MI work.
Here we instead measure I(trace_features; oracle) directly with the validated
power-gated screened estimator, over the FULL traced corpus (n=2391 code-states).
Trace features are low-dimensional (~10), so n=2391 is comfortably powered
(n/p ~ 240) -- a trustworthy verdict, unlike the n=30 lift.

Feature sets (both EXCLUDE the tautological outcome fields execution_outcome /
captured_exception, which are ~the oracle verdict):
  - shape: events_seen, files_touched, lines_executed, call/line/return counts,
           trace_events_recorded, truncated, line_coverage_density
  - shape_plus_exc: shape + exception event count (legitimate runtime signal)

If even shape_plus_exc reads ~0 bits while POWERED, the current traces are
signal-poor -- consistent with the boilerplate-truncation root cause (the event
budget is spent on import/setup, never reaching the divergent test execution).

SoT: <out>/trace_mi_measure.json
"""
from __future__ import annotations

import argparse
import glob
import importlib.util
import json
import math
import sys
from pathlib import Path


def load_probe(p):
    spec = importlib.util.spec_from_file_location("probe", str(p))
    m = importlib.util.module_from_spec(spec)
    sys.modules["probe"] = m
    spec.loader.exec_module(m)
    return m


def safe_log(x):
    return math.log1p(max(0.0, float(x)))


def extract_features(ts, mode):
    """ts = trace_summary dict. Returns feature list for the given mode."""
    et = ts.get("trace_event_types") or {}
    call = float(et.get("call", 0) or 0)
    line = float(et.get("line", 0) or 0)
    ret = float(et.get("return", 0) or 0)
    exc = float(et.get("exception", 0) or 0)
    events_seen = float(ts.get("events_seen", 0) or 0)
    files = float(ts.get("files_touched", 0) or 0)
    lines = float(ts.get("lines_executed", 0) or 0)
    recorded = float(ts.get("trace_events_recorded", 0) or 0)
    trunc = 1.0 if ts.get("truncated") else 0.0
    cov_density = lines / max(1.0, files)
    shape = [
        safe_log(events_seen), safe_log(files), safe_log(lines),
        safe_log(call), safe_log(line), safe_log(ret),
        safe_log(recorded), trunc, cov_density,
    ]
    if mode == "shape":
        return shape
    if mode == "shape_plus_exc":
        return shape + [safe_log(exc), 1.0 if exc > 0 else 0.0]
    raise SystemExit(f"UNKNOWN_FEATURE_MODE {mode}")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--probe", required=True)
    ap.add_argument("--trace-root", required=True)
    ap.add_argument("--oracle", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--seed", type=int, default=20260609)
    ap.add_argument("--holdout", type=float, default=0.25)
    args = ap.parse_args()

    import numpy as np

    probe = load_probe(Path(args.probe))
    oracle_labels, oracle_meta = probe.load_oracle_index(Path(args.oracle))

    # Gather one trace row per (task_id, category); prefer succeeded with summary.
    rowfiles = glob.glob(args.trace_root + "/**/*trace_batch_rows.jsonl", recursive=True)
    by_key = {}
    dup = 0
    for f in rowfiles:
        with open(f) as fh:
            for ln in fh:
                if not ln.strip():
                    continue
                try:
                    r = json.loads(ln)
                except Exception:
                    continue
                if r.get("status") != "succeeded":
                    continue
                ts = r.get("trace_summary")
                if not isinstance(ts, dict) or not ts:
                    continue
                tid = r.get("task_id")
                cat = r.get("category")
                if not tid or not cat:
                    continue
                key = (tid, cat)
                if key in by_key:
                    dup += 1
                    continue
                by_key[key] = ts
    print(f"[trace-mi] unique (task,cat) with trace_summary: {len(by_key)} (dup skipped {dup})", flush=True)

    # Join to oracle.
    keys = []
    y = []
    missing_oracle = 0
    for key, ts in by_key.items():
        if key not in oracle_labels:
            missing_oracle += 1
            continue
        keys.append((key, ts))
        y.append(1 if oracle_labels[key] else 0)
    y = np.asarray(y, dtype=np.int64)
    print(f"[trace-mi] joined rows: {len(keys)} pass={int(y.sum())} fail={int((y==0).sum())} missing_oracle={missing_oracle}", flush=True)

    results = []
    for mode in ("shape", "shape_plus_exc"):
        X = np.asarray([extract_features(ts, mode) for (_, ts) in keys], dtype=np.float32)
        slices = {"trace": slice(0, X.shape[1])}
        screened = probe.screened_classifier_lower_bound(X=X, y=y, label=f"trace:{mode}", seed=args.seed, holdout_fraction=args.holdout)
        gate = probe.power_calibration_gate(X=X, y=y, seed=args.seed, holdout_fraction=args.holdout)
        dec = probe._sufficiency_decision(
            target_bits=0.95,
            lower_bound_bits=screened.get("lower_bound_bits"),
            upper_bound_bits=screened.get("club_upper_bound_bits"),
            estimator_underpowered=bool(gate.get("estimator_underpowered")),
        )
        results.append({
            "feature_mode": mode, "n": int(X.shape[0]), "p": int(X.shape[1]),
            "lower_bound_bits": screened.get("lower_bound_bits"),
            "club_upper_bound_bits": screened.get("club_upper_bound_bits"),
            "target_entropy_bits": screened.get("target_entropy_bits"),
            "estimator_underpowered": gate.get("estimator_underpowered"),
            "power_recovered_bits": gate.get("recovered_bits"),
            "power_floor_bits": gate.get("detectable_floor_bits"),
            "classification": dec.get("classification"),
        })

    summary = {
        "schema": "inv010_trace_feature_mi_v1",
        "trace_root": args.trace_root,
        "oracle_sha256": oracle_meta["sha256"],
        "n_traced_code_states": len(keys),
        "label_counts": {"pass": int(y.sum()), "fail": int((y == 0).sum())},
        "target_bits": 0.95,
        "static_panel_reference_bits": "chunk-granularity diff-touched: ~[0.035, 0.176], AUC 0.72",
        "results": results,
    }
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    sot = out / "trace_mi_measure.json"
    sot.write_text(json.dumps(summary, indent=2), encoding="utf-8")
    print(f"[trace-mi] WROTE {sot}", flush=True)
    print("\nfeature_mode      n     p  | under?  LB     CLUB   floor  recovered  verdict", flush=True)
    for r in results:
        print(
            f"{r['feature_mode']:<16} {r['n']:>5} {r['p']:>3} | {str(r['estimator_underpowered']):>6} "
            f"{(r['lower_bound_bits'] or 0):>6.3f} {(r['club_upper_bound_bits'] or 0):>6.3f} "
            f"{(r['power_floor_bits'] or 0):>6.3f} {(r['power_recovered_bits'] or 0):>6.3f}  {r['classification']}",
            flush=True,
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
