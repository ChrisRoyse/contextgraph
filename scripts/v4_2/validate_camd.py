#!/usr/bin/env python3
"""Validate the CAMD-DPO foundation — constellation reward + mined pairs.

Three checks:

  1. REWARD SANITY — known Shakespeare passages score high; known modern
     passages score low. Gap ≥ 0.15 required for the reward function to be
     useful as a DPO signal.

  2. PAIR DISCRIMINATION — on a sample of 50 mined pairs, chosen.style_score
     must be higher than rejected.style_score by ≥ 0.10 in ≥ 95% of cases.

  3. PAIR STATISTICS — distribution of chosen/rejected scores, gaps, text
     lengths, anomaly kinds. Printed for human sanity-check.

Usage:
    python3 scripts/v4_2/validate_camd.py \
        --pairs ~/.contextgraph/training/v4_2_camd_pairs.jsonl
"""
from __future__ import annotations

import argparse
import json
import statistics
import sys
from collections import Counter
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from v4_2.camd_lib import quick_score  # noqa: E402


# Curated test texts — not from the training corpus.
REWARD_TESTS: list[tuple[str, str, float]] = [
    # (label, text, expected_min_style_score_for_shakespeare_or_max_for_modern)
    ("SHAKE:summer's day",
     "Shall I compare thee to a summer's day? Thou art more lovely and more temperate: "
     "Rough winds do shake the darling buds of May, And summer's lease hath all too short a date.",
     0.45),
    ("SHAKE:hamlet soliloquy",
     "To be, or not to be, that is the question: Whether 'tis nobler in the mind to suffer "
     "The slings and arrows of outrageous fortune, Or to take arms against a sea of troubles.",
     0.45),
    ("SHAKE:original verse",
     "Hark, fair soul, doth thou not hear the whispers of yonder breeze? Methinks it "
     "carrieth tidings most grievous from beyond the vale, where shadows oft do dwell.",
     0.45),
    ("MODERN:weather",
     "The weather today is pretty nice. It's not too hot and not too cold, which makes it "
     "a good day for a walk. I think I'll head to the park later this afternoon.",
     0.40),  # expected: MAX score below this
    ("MODERN:api docs",
     "The REST API accepts JSON payloads via POST requests. Authentication is handled via "
     "bearer tokens in the Authorization header. Rate limits are 100 requests per minute.",
     0.40),
    ("MODERN:casual",
     "So yeah, we should probably grab lunch tomorrow. I was thinking that new taco place "
     "downtown, they've got a deal on burritos this week. Text me if you're in.",
     0.40),
]


def test_reward_sanity() -> tuple[bool, list[dict]]:
    """Run reward-function tests. Returns (all_passed, per-test-results)."""
    results: list[dict] = []
    all_passed = True
    shake_scores: list[float] = []
    modern_scores: list[float] = []

    for label, text, thresh in REWARD_TESTS:
        s = quick_score(text)
        is_shake = label.startswith("SHAKE:")
        if is_shake:
            passed = s.style_score() >= thresh
            shake_scores.append(s.style_score())
        else:
            passed = s.style_score() <= thresh
            modern_scores.append(s.style_score())
        if not passed:
            all_passed = False
        results.append({
            "label": label, "style": s.style_score(),
            "combined": s.combined_score, "in_p95": s.in_spread_p95,
            "per_embedder": s.per_embedder,
            "threshold": thresh, "passed": passed,
        })

    # Cross-check: every shake should exceed every modern
    min_shake = min(shake_scores)
    max_modern = max(modern_scores)
    discrimination_gap = min_shake - max_modern
    if discrimination_gap < 0.10:
        all_passed = False

    return all_passed, results + [{
        "_summary": True,
        "min_shake_score": min_shake,
        "max_modern_score": max_modern,
        "discrimination_gap": discrimination_gap,
    }]


def test_pair_discrimination(jsonl_path: Path, sample: int = 50) -> tuple[bool, dict]:
    """On a sample of mined pairs, chosen should outscore rejected."""
    if not jsonl_path.exists():
        return False, {"error": f"pairs file missing: {jsonl_path}"}
    pairs = []
    with open(jsonl_path) as f:
        for i, line in enumerate(f):
            if i >= sample:
                break
            pairs.append(json.loads(line))
    if not pairs:
        return False, {"error": "empty pairs file"}

    failures = []
    gaps = []
    for p in pairs:
        gap = p["meta"]["chosen_style_score"] - p["meta"]["rejected_style_score"]
        gaps.append(gap)
        if gap < 0.10:
            failures.append({
                "prompt": p["prompt"][:80],
                "chosen_score": p["meta"]["chosen_style_score"],
                "rejected_score": p["meta"]["rejected_style_score"],
                "gap": gap,
            })
    pass_rate = 1.0 - len(failures) / len(pairs)
    passed = pass_rate >= 0.95

    return passed, {
        "n_tested": len(pairs),
        "pass_rate": pass_rate,
        "mean_gap": statistics.mean(gaps),
        "median_gap": statistics.median(gaps),
        "min_gap": min(gaps),
        "max_gap": max(gaps),
        "failures": failures[:5],  # first 5 for brevity
    }


def test_pair_statistics(jsonl_path: Path) -> dict:
    """Full dataset stats — distribution of scores, gaps, anomaly kinds."""
    if not jsonl_path.exists():
        return {"error": f"pairs file missing: {jsonl_path}"}
    chosen_scores = []
    rejected_scores = []
    gaps = []
    e1_sims = []
    chosen_lens = []
    rejected_lens = []
    anomaly_kinds = Counter()
    constellation_ids = Counter()

    with open(jsonl_path) as f:
        for line in f:
            p = json.loads(line)
            m = p["meta"]
            chosen_scores.append(m["chosen_style_score"])
            rejected_scores.append(m["rejected_style_score"])
            gaps.append(m["score_gap"])
            e1_sims.append(m["e1_similarity"])
            chosen_lens.append(len(p["chosen"]))
            rejected_lens.append(len(p["rejected"]))
            anomaly_kinds[m.get("anomaly_kind", "UNKNOWN")] += 1
            constellation_ids[m.get("constellation_id", "UNKNOWN")] += 1

    if not chosen_scores:
        return {"error": "empty pairs file"}

    def q(data: list[float], p: float) -> float:
        return statistics.quantiles(data, n=100)[int(p) - 1] if len(data) >= 2 else data[0]

    return {
        "total_pairs": len(chosen_scores),
        "chosen_style_score": {
            "mean": statistics.mean(chosen_scores),
            "median": statistics.median(chosen_scores),
            "p5": q(chosen_scores, 5), "p95": q(chosen_scores, 95),
            "min": min(chosen_scores), "max": max(chosen_scores),
        },
        "rejected_style_score": {
            "mean": statistics.mean(rejected_scores),
            "median": statistics.median(rejected_scores),
            "p5": q(rejected_scores, 5), "p95": q(rejected_scores, 95),
            "min": min(rejected_scores), "max": max(rejected_scores),
        },
        "gap": {
            "mean": statistics.mean(gaps), "median": statistics.median(gaps),
            "min": min(gaps), "max": max(gaps),
        },
        "e1_similarity": {
            "mean": statistics.mean(e1_sims), "median": statistics.median(e1_sims),
            "p5": q(e1_sims, 5), "min": min(e1_sims),
        },
        "chosen_len_chars": {
            "mean": statistics.mean(chosen_lens),
            "median": statistics.median(chosen_lens),
            "p95": q(chosen_lens, 95) if len(chosen_lens) >= 2 else chosen_lens[0],
        },
        "rejected_len_chars": {
            "mean": statistics.mean(rejected_lens),
            "median": statistics.median(rejected_lens),
            "p95": q(rejected_lens, 95) if len(rejected_lens) >= 2 else rejected_lens[0],
        },
        "anomaly_kinds": dict(anomaly_kinds),
        "constellation_ids": dict(constellation_ids),
    }


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--pairs", type=Path,
                    default=Path("~/.contextgraph/training/v4_2_camd_pairs.jsonl").expanduser(),
                    help="Path to mined CAMD pairs JSONL.")
    ap.add_argument("--sample", type=int, default=50,
                    help="Number of pairs to check for discrimination.")
    ap.add_argument("--skip-reward-test", action="store_true",
                    help="Skip the live reward-function test (slow; uses daemon).")
    args = ap.parse_args()

    exit_code = 0

    # --- Test 1: reward sanity ---
    print("=" * 70)
    print("TEST 1 — Constellation reward sanity")
    print("=" * 70)
    if args.skip_reward_test:
        print("  SKIPPED (per flag)")
    else:
        r1_passed, r1_results = test_reward_sanity()
        for r in r1_results:
            if r.get("_summary"):
                print(f"\n  Summary: min_shake={r['min_shake_score']:.3f} "
                      f"max_modern={r['max_modern_score']:.3f} "
                      f"gap={r['discrimination_gap']:+.3f}")
                continue
            marker = "✓" if r["passed"] else "✗"
            print(f"  {marker} {r['label']:30s} style={r['style']:.3f} "
                  f"combined={r['combined']:.3f} in_p95={r['in_p95']} "
                  f"(thresh={r['threshold']})")
        print(f"\n  Reward sanity: {'PASS' if r1_passed else 'FAIL'}")
        if not r1_passed:
            exit_code = 1

    # --- Test 2: pair discrimination ---
    print()
    print("=" * 70)
    print(f"TEST 2 — Pair discrimination ({args.sample} pair sample)")
    print("=" * 70)
    r2_passed, r2_data = test_pair_discrimination(args.pairs, sample=args.sample)
    if "error" in r2_data:
        print(f"  ERROR: {r2_data['error']}")
        exit_code = 1
    else:
        print(f"  n_tested       = {r2_data['n_tested']}")
        print(f"  pass_rate      = {100*r2_data['pass_rate']:.1f}% (need >=95%)")
        print(f"  gap mean/med   = {r2_data['mean_gap']:.3f} / {r2_data['median_gap']:.3f}")
        print(f"  gap min/max    = {r2_data['min_gap']:.3f} / {r2_data['max_gap']:.3f}")
        if r2_data["failures"]:
            print(f"\n  Sample failures (gap < 0.10):")
            for f in r2_data["failures"]:
                print(f"    gap={f['gap']:.3f} chosen={f['chosen_score']:.3f} "
                      f"rejected={f['rejected_score']:.3f} prompt={f['prompt']}")
        print(f"\n  Pair discrimination: {'PASS' if r2_passed else 'FAIL'}")
        if not r2_passed:
            exit_code = 1

    # --- Test 3: pair statistics ---
    print()
    print("=" * 70)
    print("TEST 3 — Pair corpus statistics")
    print("=" * 70)
    r3_data = test_pair_statistics(args.pairs)
    if "error" in r3_data:
        print(f"  ERROR: {r3_data['error']}")
        exit_code = 1
    else:
        print(f"  total_pairs       = {r3_data['total_pairs']}")
        print(f"  chosen style     mean={r3_data['chosen_style_score']['mean']:.3f} "
              f"median={r3_data['chosen_style_score']['median']:.3f} "
              f"[p5={r3_data['chosen_style_score']['p5']:.3f}, "
              f"p95={r3_data['chosen_style_score']['p95']:.3f}]")
        print(f"  rejected style   mean={r3_data['rejected_style_score']['mean']:.3f} "
              f"median={r3_data['rejected_style_score']['median']:.3f} "
              f"[p5={r3_data['rejected_style_score']['p5']:.3f}, "
              f"p95={r3_data['rejected_style_score']['p95']:.3f}]")
        print(f"  gap              mean={r3_data['gap']['mean']:.3f} "
              f"median={r3_data['gap']['median']:.3f} "
              f"range [{r3_data['gap']['min']:.3f}, {r3_data['gap']['max']:.3f}]")
        print(f"  E1 similarity   mean={r3_data['e1_similarity']['mean']:.3f} "
              f"p5={r3_data['e1_similarity']['p5']:.3f} "
              f"min={r3_data['e1_similarity']['min']:.3f}")
        print(f"  chosen len      mean={r3_data['chosen_len_chars']['mean']:.0f} "
              f"median={r3_data['chosen_len_chars']['median']:.0f} "
              f"p95={r3_data['chosen_len_chars']['p95']:.0f}")
        print(f"  anomaly kinds:  {r3_data['anomaly_kinds']}")

    print()
    print("=" * 70)
    print(f"Overall: {'PASS' if exit_code == 0 else 'FAIL'}")
    return exit_code


if __name__ == "__main__":
    sys.exit(main())
