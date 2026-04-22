#!/usr/bin/env python3
"""Extended CAMD pair miner — 18-view reward using daemon-13 + extended-5.

Compared to mine_camd_pairs.py (daemon-only):
  * Uses ExtendedConstellationScorer to compute BOTH daemon and extended
    cosines for each candidate.
  * Requires both anchor_id and negative_id to be present in the LMDB
    sidecar store (so we can skip the expensive ensemble inference per
    pair; extended cosines are read from LMDB).
  * Produces a stronger style-discriminative reward via the combined
    α·daemon_style + (1−α)·extended_style formula.

Output schema extends the daemon-only miner's JSONL with additional meta
fields for the extended views. Downstream `train_camd_dpo.py` can consume
either format.

Usage:

    # Smoke test — 10 pairs
    python3 scripts/v4_2/mine_camd_pairs_extended.py \
        --target 10 --max-input 500 \
        --output /tmp/camd_extended_smoke.jsonl

    # Production
    python3 scripts/v4_2/mine_camd_pairs_extended.py \
        --target 5000 --max-input 13500 \
        --output ~/.contextgraph/training/v4_2_camd_pairs_extended.jsonl

Hard rules:
  * No fallback to daemon-only if LMDB missing. Error out.
  * No mock pairs. Real contrastive pairs from CF_CONTRASTIVE_PAIRS only.
"""
from __future__ import annotations

import argparse
import json
import logging
import sys
import time
from dataclasses import dataclass, field
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from _gemma_shakespeare_lib import Rpc  # noqa: E402
from v4_2.camd_lib import (  # noqa: E402
    CAMDCriteria, ContrastivePair, iter_contrastive_pairs,
    pick_prompt_template,
)
from v4_2.extended_camd import (  # noqa: E402
    DEFAULT_ALPHA_DAEMON, DEFAULT_CENTROIDS_PATH,
    ExtendedConstellationScorer, ExtendedScoreResult,
)
from v4_2.extended_store import DEFAULT_PATH as DEFAULT_STORE_PATH  # noqa: E402


logger = logging.getLogger("v4_2.mine_camd_pairs_extended")

DEFAULT_OUTPUT = Path(
    "~/.contextgraph/training/v4_2_camd_pairs_extended.jsonl"
).expanduser()


@dataclass
class ExtendedCAMDPair:
    """Extended DPO triple with 18-view provenance."""
    prompt: str
    chosen: str
    rejected: str
    chosen_score: ExtendedScoreResult
    rejected_score: ExtendedScoreResult
    source_pair: ContrastivePair

    def gap_daemon_style(self) -> float:
        return (self.chosen_score.daemon_style_score
                - self.rejected_score.daemon_style_score)

    def gap_extended_style(self) -> float:
        return (self.chosen_score.extended_combined_score
                - self.rejected_score.extended_combined_score)

    def gap_combined(self) -> float:
        return (self.chosen_score.combined_reward
                - self.rejected_score.combined_reward)

    def gap_reranker(self) -> float:
        if (self.chosen_score.reranker_score is None
                or self.rejected_score.reranker_score is None):
            return 0.0
        return self.chosen_score.reranker_score - self.rejected_score.reranker_score

    def to_dpo_jsonl(self) -> dict:
        return {
            "prompt": self.prompt,
            "chosen": self.chosen,
            "rejected": self.rejected,
            "meta": {
                "source": "mine_camd_pairs_extended",
                "chosen_id": self.chosen_score.memory_id,
                "rejected_id": self.rejected_score.memory_id,
                "chosen_combined_reward": self.chosen_score.combined_reward,
                "rejected_combined_reward": self.rejected_score.combined_reward,
                "gap_combined_reward": self.gap_combined(),
                "gap_daemon_style": self.gap_daemon_style(),
                "gap_extended_style": self.gap_extended_style(),
                "gap_reranker": self.gap_reranker(),
                "chosen_daemon_style": self.chosen_score.daemon_style_score,
                "rejected_daemon_style": self.rejected_score.daemon_style_score,
                "chosen_extended_combined": self.chosen_score.extended_combined_score,
                "rejected_extended_combined": self.rejected_score.extended_combined_score,
                "chosen_daemon_per_embedder": self.chosen_score.daemon_view_cosines,
                "rejected_daemon_per_embedder": self.rejected_score.daemon_view_cosines,
                "chosen_extended_per_view": self.chosen_score.extended_view_cosines,
                "rejected_extended_per_view": self.rejected_score.extended_view_cosines,
                "chosen_reranker_score": self.chosen_score.reranker_score,
                "rejected_reranker_score": self.rejected_score.reranker_score,
                "e1_similarity_in_source_pair":
                    self.source_pair.similarity_profile[0]
                    if self.source_pair.similarity_profile else None,
                "anomaly_kind": self.source_pair.anomaly_kind,
                "alpha_daemon": self.chosen_score.alpha_daemon,
                "from_cache": (self.chosen_score.from_cache
                               and self.rejected_score.from_cache),
            },
        }


@dataclass
class ExtendedCAMDCriteria:
    """Thresholds for extended pair filtering."""
    min_e1_similarity: float = 0.70          # source pair must be content-similar
    max_style_similarity: float = 0.75       # source pair must differ in style
    min_gap_combined: float = 0.10           # chosen > rejected on combined reward
    min_chosen_combined: float = 0.50        # chosen must be meaningfully Shakespeare
    max_rejected_combined: float = 0.50      # rejected must NOT be deeply Shakespeare
    min_gap_reranker: float = 0.0            # chosen reranker > rejected reranker (≥0 allowed)
    min_text_len: int = 40
    max_text_len: int = 8000


class ExtendedCAMDPairFilter:
    """Filter existing contrastive pairs into high-quality extended DPO triples."""

    def __init__(self, scorer: ExtendedConstellationScorer,
                 criteria: ExtendedCAMDCriteria | None = None) -> None:
        self.scorer = scorer
        self.criteria = criteria or ExtendedCAMDCriteria()
        self.stats: dict[str, int] = {
            "seen": 0, "rejected_shape": 0, "rejected_e1_too_low": 0,
            "rejected_style_too_similar": 0, "rejected_lmdb_missing": 0,
            "rejected_gap_too_small": 0, "rejected_chosen_below_threshold": 0,
            "rejected_rejected_above_threshold": 0, "rejected_reranker_wrong_order": 0,
            "accepted": 0, "errors": 0,
        }

    def __call__(self, pair: ContrastivePair, prompt_template: str
                  ) -> ExtendedCAMDPair | None:
        c = self.criteria
        self.stats["seen"] += 1

        # Stage 1 — shape
        if (not pair.anchor_text or not pair.negative_text
                or len(pair.anchor_text) < c.min_text_len
                or len(pair.anchor_text) > c.max_text_len
                or len(pair.negative_text) < c.min_text_len
                or len(pair.negative_text) > c.max_text_len):
            self.stats["rejected_shape"] += 1
            return None

        # Stage 2 — cheap filters from source pair's 13-view profile
        if pair.e1_similarity() < c.min_e1_similarity:
            self.stats["rejected_e1_too_low"] += 1
            return None
        if pair.style_similarity() > c.max_style_similarity:
            self.stats["rejected_style_too_similar"] += 1
            return None

        # Stage 3 — extended scoring (requires both in LMDB)
        if not self.scorer.store.exists(pair.anchor_id) \
                or not self.scorer.store.exists(pair.negative_id):
            self.stats["rejected_lmdb_missing"] += 1
            return None

        try:
            anchor_s = self.scorer.score_memory_id(pair.anchor_id)
            negative_s = self.scorer.score_memory_id(pair.negative_id)
        except Exception as e:
            self.stats["errors"] += 1
            logger.exception(
                f"score error on pair ({pair.anchor_id[:8]},"
                f" {pair.negative_id[:8]}): {e.__class__.__name__}"
            )
            return None

        # Stage 4 — assign chosen/rejected by combined reward
        if anchor_s.combined_reward >= negative_s.combined_reward:
            chosen_text, chosen_s = pair.anchor_text, anchor_s
            rejected_text, rejected_s = pair.negative_text, negative_s
        else:
            chosen_text, chosen_s = pair.negative_text, negative_s
            rejected_text, rejected_s = pair.anchor_text, anchor_s

        # Stage 5 — quality filters
        if chosen_s.combined_reward < c.min_chosen_combined:
            self.stats["rejected_chosen_below_threshold"] += 1
            return None
        if rejected_s.combined_reward > c.max_rejected_combined:
            self.stats["rejected_rejected_above_threshold"] += 1
            return None
        gap = chosen_s.combined_reward - rejected_s.combined_reward
        if gap < c.min_gap_combined:
            self.stats["rejected_gap_too_small"] += 1
            return None

        # Optional: reranker-order check (chosen should rerank higher on
        # "Shakespearean verse about summer" query)
        if (chosen_s.reranker_score is not None
                and rejected_s.reranker_score is not None
                and c.min_gap_reranker >= 0
                and (chosen_s.reranker_score - rejected_s.reranker_score)
                    < c.min_gap_reranker):
            self.stats["rejected_reranker_wrong_order"] += 1
            return None

        self.stats["accepted"] += 1
        first_line = chosen_text.splitlines()[0].strip()[:140]
        first_words = " ".join(chosen_text.split()[:12])
        prompt = prompt_template.format(first_line=first_line, first_words=first_words)
        return ExtendedCAMDPair(
            prompt=prompt, chosen=chosen_text, rejected=rejected_text,
            chosen_score=chosen_s, rejected_score=rejected_s,
            source_pair=pair,
        )

    def summary(self) -> str:
        s = self.stats
        n = max(s["seen"], 1)
        return (
            f"ExtendedCAMD filter summary: seen={s['seen']} accepted={s['accepted']} "
            f"({100*s['accepted']/n:.1f}%) errors={s['errors']}\n"
            f"  rejected_shape={s['rejected_shape']}\n"
            f"  rejected_e1_too_low={s['rejected_e1_too_low']}\n"
            f"  rejected_style_too_similar={s['rejected_style_too_similar']}\n"
            f"  rejected_lmdb_missing={s['rejected_lmdb_missing']}\n"
            f"  rejected_chosen_below_threshold={s['rejected_chosen_below_threshold']}\n"
            f"  rejected_rejected_above_threshold={s['rejected_rejected_above_threshold']}\n"
            f"  rejected_gap_too_small={s['rejected_gap_too_small']}\n"
            f"  rejected_reranker_wrong_order={s['rejected_reranker_wrong_order']}"
        )


def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(description=__doc__,
                                formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    p.add_argument("--target", type=int, default=5000)
    p.add_argument("--max-input", type=int, default=13500)
    p.add_argument("--page-size", type=int, default=200)
    p.add_argument("--store-path", type=Path, default=DEFAULT_STORE_PATH)
    p.add_argument("--centroids-path", type=Path, default=DEFAULT_CENTROIDS_PATH)
    p.add_argument("--alpha-daemon", type=float, default=DEFAULT_ALPHA_DAEMON)
    p.add_argument("--min-e1", type=float, default=0.70)
    p.add_argument("--max-style-sim", type=float, default=0.75)
    p.add_argument("--min-gap", type=float, default=0.10)
    p.add_argument("--min-chosen", type=float, default=0.50)
    p.add_argument("--max-rejected", type=float, default=0.50)
    p.add_argument("--min-gap-reranker", type=float, default=0.0)
    p.add_argument("--heartbeat-every", type=int, default=25)
    p.add_argument("--kind-filter", default=None)
    return p.parse_args()


def main() -> int:
    args = parse_args()
    logging.basicConfig(level=logging.INFO,
                        format="%(asctime)s [%(name)s] %(levelname)s: %(message)s")

    args.output.parent.mkdir(parents=True, exist_ok=True)
    partial = args.output.with_suffix(args.output.suffix + ".partial")

    criteria = ExtendedCAMDCriteria(
        min_e1_similarity=args.min_e1,
        max_style_similarity=args.max_style_sim,
        min_gap_combined=args.min_gap,
        min_chosen_combined=args.min_chosen,
        max_rejected_combined=args.max_rejected,
        min_gap_reranker=args.min_gap_reranker,
    )

    logger.info(f"target={args.target} max_input={args.max_input} alpha_daemon={args.alpha_daemon}")
    logger.info(
        f"criteria: min_e1={criteria.min_e1_similarity} "
        f"max_style_sim={criteria.max_style_similarity} "
        f"min_gap={criteria.min_gap_combined} "
        f"chosen>={criteria.min_chosen_combined} "
        f"rejected<={criteria.max_rejected_combined}"
    )

    rpc = Rpc()
    scorer = ExtendedConstellationScorer(
        rpc=rpc, store_path=args.store_path,
        centroids_path=args.centroids_path, alpha_daemon=args.alpha_daemon,
    )
    filt = ExtendedCAMDPairFilter(scorer, criteria)

    accepted = 0
    processed = 0
    t0 = time.monotonic()
    t_hb = t0
    try:
        with open(partial, "w", encoding="utf-8") as out_f:
            for pair in iter_contrastive_pairs(
                rpc, page_size=args.page_size,
                kind_filter=args.kind_filter, max_pairs=args.max_input,
            ):
                if accepted >= args.target:
                    break
                processed += 1
                result = filt(pair, pick_prompt_template(processed))
                if result is None:
                    continue
                out_f.write(json.dumps(result.to_dpo_jsonl(),
                                        ensure_ascii=False) + "\n")
                out_f.flush()
                accepted += 1
                if (accepted % args.heartbeat_every == 0
                        or time.monotonic() - t_hb > 30):
                    elapsed = time.monotonic() - t0
                    rate = accepted / max(elapsed, 1e-3)
                    eta = (args.target - accepted) / max(rate, 1e-6)
                    logger.info(
                        f"progress accepted={accepted}/{args.target} "
                        f"processed={processed} rate={rate:.2f}/s ETA={eta/60:.1f}min"
                    )
                    t_hb = time.monotonic()
    finally:
        rpc.close()

    if accepted > 0:
        partial.rename(args.output)
    elapsed = time.monotonic() - t0

    print()
    print("=" * 74)
    print(f"EXTENDED CAMD MINE — written={accepted} processed={processed} "
          f"wall={elapsed:.1f}s")
    print("=" * 74)
    print(filt.summary())
    print(f"Output: {args.output}")
    return 0 if accepted > 0 else 2


if __name__ == "__main__":
    sys.exit(main())
