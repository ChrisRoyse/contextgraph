#!/usr/bin/env python3
"""CAMD-DPO core library — Constellation-Anchored Multi-View DPO.

The v4.2 training signal. Three core abstractions:

    ConstellationScore  — a 13-view reward vector + aggregate + verdict, for
                          one candidate text scored against a constellation.

    ConstellationScorer — wraps the daemon's score_against_constellation tool
                          with batching + cleanup. Lazily stores probes as
                          memories and soft-deletes them after scoring.

    CAMDPairFilter      — filters existing CF_CONTRASTIVE_PAIRS to CAMD-
                          discriminative ones (E1 high-similarity = same
                          meaning; E12/E13 low-similarity = style differs).

Design principles:

    * No judge LLM. Reward is deterministic multi-view distance to the Sonnets
      constellation, which is the canonical Shakespeare anchor in our
      database.
    * No mock data, no fallbacks — every daemon failure raises.
    * Reusable by all downstream v4.2 methods (DPO, EGTL token loss, MVAG
      runtime guard). The constellation reward is the shared primitive.

Reference:
    docs/V4_2_NOVEL_METHODS.md — Methods 1, 2, 4.
"""
from __future__ import annotations

import sys
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Iterator

# Reuse existing JSON-RPC client + probe helpers from the demo library.
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from _gemma_shakespeare_lib import Rpc  # noqa: E402


# ---------------------------------------------------------------------------
# Canonical constellation anchors
# ---------------------------------------------------------------------------
SONNETS_CONSTELLATION_ID = "18ef2460-9ab6-44fc-8d7e-8d7513174ac8"  # 313 members, coherence 0.9036

# Per-embedder index mapping (0-indexed). Matches the daemon's
# similarity_profile ordering from list_contrastive_pairs.
EMB = {
    "E1":  0,   # semantic (e5-large-v2, 1024D dense)
    "E2":  1,   # temporal/recency (sinusoidal PE, 512D)
    "E3":  2,   # temporal/periodic (512D)
    "E4":  3,   # temporal/sequential (512D)
    "E5":  4,   # causal asymmetric (768D)
    "E6":  5,   # sparse keyword (inverted, 30K)
    "E7":  6,   # code structure (UniXCoder, 1536D)
    "E8":  7,   # graph topology (1024D)
    "E9":  8,   # HDC (1024D)
    "E10": 9,   # paraphrase asymmetric (1024D)
    "E11": 10,  # entity/Kepler (768D) — often 0.0 (disabled/unlinked)
    "E12": 11,  # ColBERT per-token (128D/token)
    "E13": 12,  # SPLADE sparse (30K)
}

# STYLE embedders (mirror of shakespeare_chat_dashboard.py STYLE_EMBEDDER_WEIGHTS).
# These 5 discriminate style rather than topic. Weights sum to 1.0.
STYLE_WEIGHTS: dict[int, float] = {
    EMB["E7"]:  0.15,   # code/syntax pattern
    EMB["E9"]:  0.15,   # HDC structural robustness
    EMB["E10"]: 0.15,   # paraphrase
    EMB["E12"]: 0.30,   # ColBERT (strongest)
    EMB["E13"]: 0.25,   # SPLADE term-expansion
}
assert abs(sum(STYLE_WEIGHTS.values()) - 1.0) < 1e-9


# ---------------------------------------------------------------------------
# Data classes
# ---------------------------------------------------------------------------
@dataclass
class ConstellationScore:
    """One candidate scored against one constellation."""
    text: str
    constellation_id: str
    per_embedder: list[float]       # 13-dim cosine-to-centroid
    combined_score: float           # unweighted mean over covered embedders
    in_spread_p95: bool             # within 95th-percentile spread (Mahalanobis-like)
    fingerprint_id: str | None      # probe fingerprint (None after cleanup)

    def style_score(self) -> float:
        """Weighted aggregate over style-discriminating embedders only."""
        total = 0.0
        for idx, w in STYLE_WEIGHTS.items():
            if idx < len(self.per_embedder):
                total += w * self.per_embedder[idx]
        return total

    def semantic_score(self) -> float:
        """E1 cosine — how semantically close to the constellation centroid."""
        return self.per_embedder[EMB["E1"]] if self.per_embedder else 0.0


@dataclass
class ContrastivePair:
    """A pair from CF_CONTRASTIVE_PAIRS (existing daemon store)."""
    anchor_id: str
    anchor_text: str
    negative_id: str
    negative_text: str
    similarity_profile: list[float]     # 13-dim
    anomaly_kind: str
    high_embedders: list[int] = field(default_factory=list)
    low_embedders: list[int] = field(default_factory=list)
    disagreement_magnitude: float = 0.0

    def e1_similarity(self) -> float:
        return self.similarity_profile[EMB["E1"]]

    def style_similarity(self) -> float:
        """Weighted style-embedder similarity between anchor and negative.

        High = same style (bad for us, not discriminative).
        Low  = different style (good for us — chosen/rejected contrast).
        """
        total = 0.0
        for idx, w in STYLE_WEIGHTS.items():
            if idx < len(self.similarity_profile):
                total += w * self.similarity_profile[idx]
        return total


@dataclass
class CAMDPair:
    """A mined DPO triple with full provenance.

    Serializes to JSONL in the canonical TRL DPO format:
        {"prompt": str, "chosen": str, "rejected": str,
         "meta": {provenance fields}}
    """
    prompt: str
    chosen: str
    rejected: str
    chosen_score: ConstellationScore
    rejected_score: ConstellationScore
    source_pair: ContrastivePair

    def score_gap(self) -> float:
        """Gap in style score — chosen should substantially outscore rejected."""
        return self.chosen_score.style_score() - self.rejected_score.style_score()

    def to_dpo_jsonl(self) -> dict:
        return {
            "prompt": self.prompt,
            "chosen": self.chosen,
            "rejected": self.rejected,
            "meta": {
                "chosen_id": (self.source_pair.anchor_id
                              if self.chosen is self.source_pair.anchor_text
                              else self.source_pair.negative_id),
                "rejected_id": (self.source_pair.negative_id
                                if self.chosen is self.source_pair.anchor_text
                                else self.source_pair.anchor_id),
                "chosen_style_score": self.chosen_score.style_score(),
                "rejected_style_score": self.rejected_score.style_score(),
                "chosen_combined_score": self.chosen_score.combined_score,
                "rejected_combined_score": self.rejected_score.combined_score,
                "score_gap": self.score_gap(),
                "chosen_per_embedder": self.chosen_score.per_embedder,
                "rejected_per_embedder": self.rejected_score.per_embedder,
                "anomaly_kind": self.source_pair.anomaly_kind,
                "e1_similarity": self.source_pair.e1_similarity(),
                "style_similarity": self.source_pair.style_similarity(),
                "disagreement_magnitude": self.source_pair.disagreement_magnitude,
                "constellation_id": self.chosen_score.constellation_id,
                "mined_by": "camd_lib.CAMDPairFilter",
            },
        }


# ---------------------------------------------------------------------------
# Constellation scorer
# ---------------------------------------------------------------------------
class ConstellationScorer:
    """Wrap daemon score_against_constellation with probe lifecycle management.

    Each call stores a transient probe memory, scores it, then (lazily)
    soft-deletes it. Batch scorer reuses one Rpc connection.

    Thread-unsafe; use one scorer per worker.
    """

    def __init__(self, rpc: Rpc, constellation_id: str = SONNETS_CONSTELLATION_ID,
                 session_tag: str = "camd-scorer") -> None:
        self.rpc = rpc
        self.constellation_id = constellation_id
        self.session_tag = session_tag
        self._probes_to_cleanup: list[str] = []

    def score(self, text: str, cleanup: bool = True) -> ConstellationScore:
        """Score one text. If cleanup=False, defers forget_concept to flush()."""
        if not text or not text.strip():
            raise ValueError("ConstellationScorer.score: text is empty")
        fp = self._store_probe(text)
        try:
            r = self.rpc.call("score_against_constellation", {
                "constellationId": self.constellation_id,
                "memoryId": fp,
            }, timeout=30.0)
        except Exception:
            self._forget(fp)
            raise
        result = ConstellationScore(
            text=text,
            constellation_id=self.constellation_id,
            per_embedder=r.get("per_embedder_cosine") or [],
            combined_score=float(r.get("combined_score") or 0.0),
            in_spread_p95=bool(r.get("in_spread_p95", False)),
            fingerprint_id=fp,
        )
        if cleanup:
            self._forget(fp)
            result.fingerprint_id = None
        else:
            self._probes_to_cleanup.append(fp)
        return result

    def flush(self) -> None:
        """Soft-delete all deferred probes."""
        while self._probes_to_cleanup:
            fp = self._probes_to_cleanup.pop()
            try:
                self._forget(fp)
            except Exception:
                pass  # cleanup best-effort

    def _store_probe(self, text: str) -> str:
        r = self.rpc.call("store_memory", {
            "content": text,
            "sessionId": self.session_tag,
            "importance": 0.3,
            "rationale": f"CAMD probe: {text[:60]}",
        }, timeout=120.0)
        fp = r.get("fingerprintId")
        if not fp:
            raise RuntimeError(f"store_memory returned no fingerprintId: {r}")
        return fp

    def _forget(self, fp: str) -> None:
        try:
            self.rpc.call("forget_concept", {"memoryId": fp}, timeout=15.0)
        except Exception:
            pass  # soft-delete best-effort


# ---------------------------------------------------------------------------
# Pair loading from CF_CONTRASTIVE_PAIRS (daemon-persistent store)
# ---------------------------------------------------------------------------
def iter_contrastive_pairs(rpc: Rpc, page_size: int = 500,
                           kind_filter: str | None = None,
                           max_pairs: int | None = None) -> Iterator[ContrastivePair]:
    """Stream existing contrastive pairs from CF_CONTRASTIVE_PAIRS.

    13,465 pairs were pre-mined by `mine_contrastive_pairs` during corpus
    ingestion. We reuse them rather than re-mining.
    """
    offset = 0
    emitted = 0
    while True:
        if max_pairs is not None and emitted >= max_pairs:
            return
        want = page_size
        if max_pairs is not None:
            want = min(want, max_pairs - emitted)
        args: dict = {"limit": want, "offset": offset, "includeFull": True}
        if kind_filter:
            args["kind"] = kind_filter
        r = rpc.call("list_contrastive_pairs", args, timeout=60.0)
        pairs = r.get("pairs", [])
        if not pairs:
            return
        for p in pairs:
            yield ContrastivePair(
                anchor_id=p["anchor_id"],
                anchor_text=p.get("anchor_text", ""),
                negative_id=p["negative_id"],
                negative_text=p.get("negative_text", ""),
                similarity_profile=p.get("similarity_profile") or [],
                anomaly_kind=p.get("anomaly_kind", "UNKNOWN"),
                high_embedders=p.get("high_embedders") or [],
                low_embedders=p.get("low_embedders") or [],
                disagreement_magnitude=float(p.get("disagreement_magnitude") or 0.0),
            )
            emitted += 1
            if max_pairs is not None and emitted >= max_pairs:
                return
        offset += len(pairs)
        if len(pairs) < want:
            return


# ---------------------------------------------------------------------------
# CAMD pair filter
# ---------------------------------------------------------------------------
@dataclass
class CAMDCriteria:
    """Thresholds defining what makes a pair CAMD-discriminative.

    The philosophy: we want (chosen, rejected) where they mean the same thing
    but one is Shakespeare-style and the other is not.
    """
    min_e1_similarity: float = 0.70        # same semantic content
    max_style_similarity: float = 0.60     # different style
    min_constellation_gap: float = 0.15    # chosen substantially closer to Sonnets
    min_chosen_style_score: float = 0.50   # chosen must actually be Shakespeare
    max_rejected_style_score: float = 0.40 # rejected must actually NOT be Shakespeare
    max_text_len: int = 3000                # skip giant records for DPO
    min_text_len: int = 40                  # skip trivially-short records


class CAMDPairFilter:
    """Filter contrastive pairs into CAMD-eligible, constellation-ranked DPO pairs.

    Pipeline per pair:
        1. Shape filter (text lengths OK?)
        2. Similarity filter (E1-high + style-low?)
        3. Constellation scoring — score both anchor and negative against Sonnets
        4. Assign chosen/rejected — whichever has higher style_score
        5. Gap filter — discard if the gap is too small to teach DPO anything

    All rejections are counted and returned in `stats` for diagnostic purposes.
    """

    def __init__(self, scorer: ConstellationScorer,
                 criteria: CAMDCriteria | None = None) -> None:
        self.scorer = scorer
        self.criteria = criteria or CAMDCriteria()
        self.stats: dict[str, int] = {
            "seen": 0,
            "rejected_shape": 0,
            "rejected_e1_too_low": 0,
            "rejected_style_too_similar": 0,
            "rejected_gap_too_small": 0,
            "rejected_chosen_below_threshold": 0,
            "rejected_rejected_above_threshold": 0,
            "accepted": 0,
        }

    def __call__(self, pair: ContrastivePair, prompt_template: str) -> CAMDPair | None:
        """Apply full CAMD filter. Returns None if pair rejected at any stage."""
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

        # Stage 2 — cheap profile filter (no daemon scoring yet)
        if pair.e1_similarity() < c.min_e1_similarity:
            self.stats["rejected_e1_too_low"] += 1
            return None
        if pair.style_similarity() > c.max_style_similarity:
            self.stats["rejected_style_too_similar"] += 1
            return None

        # Stage 3 — constellation scoring (two daemon calls per pair)
        anchor_cs = self.scorer.score(pair.anchor_text)
        negative_cs = self.scorer.score(pair.negative_text)

        # Stage 4 — assign chosen/rejected
        if anchor_cs.style_score() >= negative_cs.style_score():
            chosen, chosen_cs = pair.anchor_text, anchor_cs
            rejected, rejected_cs = pair.negative_text, negative_cs
        else:
            chosen, chosen_cs = pair.negative_text, negative_cs
            rejected, rejected_cs = pair.anchor_text, anchor_cs

        # Stage 5 — quality filters
        if chosen_cs.style_score() < c.min_chosen_style_score:
            self.stats["rejected_chosen_below_threshold"] += 1
            return None
        if rejected_cs.style_score() > c.max_rejected_style_score:
            self.stats["rejected_rejected_above_threshold"] += 1
            return None
        gap = chosen_cs.style_score() - rejected_cs.style_score()
        if gap < c.min_constellation_gap:
            self.stats["rejected_gap_too_small"] += 1
            return None

        self.stats["accepted"] += 1
        prompt = prompt_template.format(
            first_line=chosen.splitlines()[0].strip()[:140],
            first_words=" ".join(chosen.split()[:12]),
        )
        return CAMDPair(
            prompt=prompt, chosen=chosen, rejected=rejected,
            chosen_score=chosen_cs, rejected_score=rejected_cs,
            source_pair=pair,
        )

    def summary(self) -> str:
        s = self.stats
        n = max(s["seen"], 1)
        return (
            f"CAMD filter summary: seen={s['seen']} accepted={s['accepted']} "
            f"({100*s['accepted']/n:.1f}%)\n"
            f"  rejected_shape={s['rejected_shape']}\n"
            f"  rejected_e1_too_low={s['rejected_e1_too_low']}\n"
            f"  rejected_style_too_similar={s['rejected_style_too_similar']}\n"
            f"  rejected_chosen_below_threshold={s['rejected_chosen_below_threshold']}\n"
            f"  rejected_rejected_above_threshold={s['rejected_rejected_above_threshold']}\n"
            f"  rejected_gap_too_small={s['rejected_gap_too_small']}\n"
            f"  criteria: min_e1={self.criteria.min_e1_similarity} "
            f"max_style_sim={self.criteria.max_style_similarity} "
            f"min_gap={self.criteria.min_constellation_gap} "
            f"chosen>={self.criteria.min_chosen_style_score} "
            f"rejected<={self.criteria.max_rejected_style_score}"
        )


# ---------------------------------------------------------------------------
# Default prompt templates
# ---------------------------------------------------------------------------
DEFAULT_PROMPT_TEMPLATES = [
    # Used when we don't have a topic-extraction step. All teach the same
    # invariant: "given a request for Shakespeare-voice content, prefer
    # Shakespeare-voice response over modern." Rotated for prompt-diversity.
    "Compose a passage in the voice of William Shakespeare.",
    "Speak as Shakespeare would. Begin: {first_words}…",
    "In the manner of the Bard, continue this thought: {first_line}",
    "Write verse or prose in early modern English on this theme.",
    "Give me Shakespeare's take on the subject below.",
    "Render the following idea in Shakespearean style: {first_words}…",
    "As William Shakespeare, express what follows: {first_line}",
    "In Shakespeare's voice, reflect on: {first_words}…",
]


def pick_prompt_template(idx: int) -> str:
    return DEFAULT_PROMPT_TEMPLATES[idx % len(DEFAULT_PROMPT_TEMPLATES)]


# ---------------------------------------------------------------------------
# Quick convenience function for one-shot scoring
# ---------------------------------------------------------------------------
def quick_score(text: str,
                constellation_id: str = SONNETS_CONSTELLATION_ID) -> ConstellationScore:
    """Convenience: open an Rpc, score one text, close. For REPL / testing."""
    rpc = Rpc()
    try:
        scorer = ConstellationScorer(rpc, constellation_id)
        return scorer.score(text)
    finally:
        rpc.close()


if __name__ == "__main__":
    # Smoke test — score a known Shakespeare passage and a known modern one.
    SHAKESPEARE_SAMPLE = (
        "Shall I compare thee to a summer's day? "
        "Thou art more lovely and more temperate: "
        "Rough winds do shake the darling buds of May, "
        "And summer's lease hath all too short a date."
    )
    MODERN_SAMPLE = (
        "The weather today is pretty nice. "
        "It's not too hot and not too cold, "
        "which makes it a good day to go for a walk. "
        "I think I'll head to the park after work."
    )
    print(f"Testing constellation scorer against {SONNETS_CONSTELLATION_ID}")
    t0 = time.time()
    s1 = quick_score(SHAKESPEARE_SAMPLE)
    print(f"Shakespeare sample: "
          f"style={s1.style_score():.3f} combined={s1.combined_score:.3f} "
          f"in_p95={s1.in_spread_p95} (took {(time.time()-t0)*1000:.0f}ms)")
    t0 = time.time()
    s2 = quick_score(MODERN_SAMPLE)
    print(f"Modern sample:      "
          f"style={s2.style_score():.3f} combined={s2.combined_score:.3f} "
          f"in_p95={s2.in_spread_p95} (took {(time.time()-t0)*1000:.0f}ms)")
    gap = s1.style_score() - s2.style_score()
    print(f"Gap (should be > 0.15 for reward function to be discriminative): "
          f"{gap:+.3f}")
    if gap < 0.15:
        sys.exit(1)
    print("PASS — constellation reward discriminates style.")
