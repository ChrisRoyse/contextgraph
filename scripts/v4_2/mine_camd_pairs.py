#!/usr/bin/env python3
"""CAMD-DPO pair mining CLI — v4.2 Method 1 + 2.

Loads existing contrastive pairs from CF_CONTRASTIVE_PAIRS (13k+ available),
applies the CAMD filter (E1-high + style-low + constellation gap), scores
each side against the Sonnets constellation, and writes DPO-formatted JSONL.

No judge LLM. The ranking comes entirely from the Sonnets constellation via
the daemon's score_against_constellation tool.

Atomic write pattern: output file is written as .partial then renamed.
A sidecar state JSON allows resume after crash.

Usage:

    # Smoke test — mine ~50 pairs
    python3 scripts/v4_2/mine_camd_pairs.py --max-input 500 --target 50 \\
        --output ~/.contextgraph/training/v4_2_camd_pairs_smoke.jsonl

    # Production — mine up to 5k pairs from the full 13k pool
    python3 scripts/v4_2/mine_camd_pairs.py --max-input 13500 --target 5000 \\
        --output ~/.contextgraph/training/v4_2_camd_pairs.jsonl

Output schema (one JSON object per line):

    {
      "prompt": "Compose a passage in the voice of William Shakespeare.",
      "chosen": "<Shakespeare-style text>",
      "rejected": "<non-Shakespeare text with same meaning>",
      "meta": {
        "chosen_style_score": 0.62, "rejected_style_score": 0.38,
        "score_gap": 0.24, "e1_similarity": 0.91, ...
      }
    }

Resume: if --output already exists, the state sidecar tells us the last
`offset` we read from CF_CONTRASTIVE_PAIRS. Pass --resume to continue.
"""
from __future__ import annotations

import argparse
import json
import os
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from _gemma_shakespeare_lib import Rpc  # noqa: E402
from v4_2.camd_lib import (  # noqa: E402
    CAMDCriteria,
    CAMDPairFilter,
    ConstellationScorer,
    SONNETS_CONSTELLATION_ID,
    iter_contrastive_pairs,
    pick_prompt_template,
)


DEFAULT_OUTPUT = Path("~/.contextgraph/training/v4_2_camd_pairs.jsonl").expanduser()


def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(description=__doc__,
                                formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--output", type=Path, default=DEFAULT_OUTPUT,
                   help="JSONL output path.")
    p.add_argument("--target", type=int, default=5000,
                   help="Stop once this many accepted pairs written.")
    p.add_argument("--max-input", type=int, default=13500,
                   help="Maximum pairs to stream from CF_CONTRASTIVE_PAIRS.")
    p.add_argument("--page-size", type=int, default=200,
                   help="Pagination size for list_contrastive_pairs.")
    p.add_argument("--min-e1", type=float, default=0.70,
                   help="Minimum E1 similarity (semantic content match).")
    p.add_argument("--max-style-sim", type=float, default=0.60,
                   help="Maximum style-embedder similarity (chosen!=rejected style).")
    p.add_argument("--min-gap", type=float, default=0.15,
                   help="Minimum style_score(chosen) - style_score(rejected).")
    p.add_argument("--min-chosen", type=float, default=0.50,
                   help="Minimum chosen style_score (chosen must be Shakespeare).")
    p.add_argument("--max-rejected", type=float, default=0.40,
                   help="Maximum rejected style_score (rejected must NOT be Shakespeare).")
    p.add_argument("--min-text-len", type=int, default=40)
    p.add_argument("--max-text-len", type=int, default=3000)
    p.add_argument("--kind-filter", type=str, default=None,
                   help="Optional AnomalyKind filter passed to list_contrastive_pairs.")
    p.add_argument("--heartbeat-every", type=int, default=25,
                   help="Log progress every N pairs processed.")
    p.add_argument("--fail-budget", type=float, default=0.05,
                   help="Abort if fraction of scoring failures exceeds this.")
    p.add_argument("--resume", action="store_true",
                   help="Resume from sidecar state file if present.")
    return p.parse_args()


def state_path_for(output: Path) -> Path:
    return output.with_suffix(output.suffix + ".state.json")


def load_state(sp: Path) -> dict:
    if sp.exists():
        try:
            return json.loads(sp.read_text())
        except Exception:
            return {}
    return {}


def save_state(sp: Path, state: dict) -> None:
    sp.write_text(json.dumps(state, indent=2))


def main() -> int:
    args = parse_args()
    args.output.parent.mkdir(parents=True, exist_ok=True)

    sidecar = state_path_for(args.output)
    state = load_state(sidecar) if args.resume else {}
    start_offset = int(state.get("offset", 0))
    already_accepted = int(state.get("accepted", 0))

    mode = "a" if args.resume and args.output.exists() else "w"
    partial = args.output.with_suffix(args.output.suffix + ".partial")

    criteria = CAMDCriteria(
        min_e1_similarity=args.min_e1,
        max_style_similarity=args.max_style_sim,
        min_constellation_gap=args.min_gap,
        min_chosen_style_score=args.min_chosen,
        max_rejected_style_score=args.max_rejected,
        min_text_len=args.min_text_len,
        max_text_len=args.max_text_len,
    )

    print(f"[camd] constellation: {SONNETS_CONSTELLATION_ID}")
    print(f"[camd] target: {args.target} accepted pairs (have {already_accepted})")
    print(f"[camd] max input: {args.max_input}, start offset: {start_offset}")
    print(f"[camd] criteria: min_e1={criteria.min_e1_similarity} "
          f"max_style_sim={criteria.max_style_similarity} "
          f"min_gap={criteria.min_constellation_gap}")
    print(f"[camd] output: {args.output}")
    print()

    rpc = Rpc()
    scorer = ConstellationScorer(rpc)
    filt = CAMDPairFilter(scorer, criteria)

    accepted = already_accepted
    processed = 0
    scoring_failures = 0
    t_start = time.time()
    t_last_heartbeat = t_start

    # We skip start_offset by iterating; iter_contrastive_pairs does pagination
    # internally. For resume, we ask for max_input - start_offset more rows.
    remaining_input = max(0, args.max_input - start_offset)

    out_file = open(partial if not args.resume else args.output, mode, encoding="utf-8")
    try:
        # Simulate an offset by consuming the first start_offset pairs silently.
        pair_iter = iter_contrastive_pairs(
            rpc, page_size=args.page_size,
            kind_filter=args.kind_filter,
            max_pairs=args.max_input,
        )

        if start_offset > 0:
            for skipped, _pair in enumerate(pair_iter):
                if skipped + 1 >= start_offset:
                    break

        # Now continue with the real filter.
        for pair in pair_iter:
            if accepted >= args.target:
                break
            processed += 1

            try:
                result = filt(pair, pick_prompt_template(processed))
            except Exception as e:
                scoring_failures += 1
                if scoring_failures / max(processed, 1) > args.fail_budget:
                    raise RuntimeError(
                        f"fail_budget exceeded: {scoring_failures}/{processed} "
                        f"scoring calls failed. Last error: {e}")
                continue

            if result is None:
                continue

            out_file.write(json.dumps(result.to_dpo_jsonl(), ensure_ascii=False) + "\n")
            out_file.flush()
            accepted += 1

            # Heartbeat + checkpoint
            if accepted % args.heartbeat_every == 0 or time.time() - t_last_heartbeat > 30:
                elapsed = time.time() - t_start
                rate = accepted / max(elapsed, 1e-3)
                eta_sec = (args.target - accepted) / max(rate, 1e-6)
                print(f"[camd] accepted={accepted}/{args.target} "
                      f"processed={processed} "
                      f"rate={rate:.2f} pairs/s ETA={eta_sec/60:.1f}min "
                      f"failures={scoring_failures}")
                t_last_heartbeat = time.time()
                # checkpoint state
                save_state(sidecar, {
                    "offset": start_offset + processed,
                    "accepted": accepted,
                    "processed": processed,
                    "scoring_failures": scoring_failures,
                    "elapsed_sec": elapsed,
                })
    finally:
        out_file.close()
        try:
            scorer.flush()
        except Exception:
            pass
        rpc.close()

    # Atomic rename .partial → .output (unless we were appending).
    if not args.resume:
        if partial.exists() and not args.output.exists():
            partial.rename(args.output)
        elif partial.exists() and args.output.exists():
            # safety: shouldn't happen in fresh write mode
            partial.unlink()

    elapsed = time.time() - t_start
    print()
    print(f"[camd] done in {elapsed/60:.1f}min")
    print(f"[camd] {accepted} pairs written to {args.output}")
    print(f"[camd] {scoring_failures} scoring failures ({100*scoring_failures/max(processed,1):.2f}%)")
    print()
    print(filt.summary())

    # Final state snapshot
    save_state(sidecar, {
        "offset": start_offset + processed,
        "accepted": accepted,
        "processed": processed,
        "scoring_failures": scoring_failures,
        "elapsed_sec": elapsed,
        "complete": True,
    })
    return 0 if accepted > 0 else 2


if __name__ == "__main__":
    sys.exit(main())
