#!/usr/bin/env python3
"""Extended CAMD reward — combines daemon-13 views + extended-5 views.

Usage as a library:

    from v4_2.extended_camd import ExtendedConstellationScorer

    scorer = ExtendedConstellationScorer(
        rpc=Rpc(),                 # daemon JSON-RPC
        store_path=DEFAULT_PATH,   # LMDB sidecar
        centroids_path=DEFAULT_CENTROIDS_PATH,
    )

    # Score fresh text (no memory_id yet)
    result = scorer.score_text("Shall I compare thee to a summer's day?")
    print(result.daemon_view_cosines)     # 13 floats
    print(result.extended_view_cosines)   # 5 floats (bge_dense, bge_colbert, bge_sparse, jina, qwen)
    print(result.combined_reward)         # weighted aggregate
    print(result.per_view_contribution)   # detailed breakdown

    # Score a known memory_id (uses cached extended, queries daemon live)
    result = scorer.score_memory_id(memory_uuid)

Reward function:
    combined = sum(w_i * cosine_i for i in views with w_i > 0)
    weights mirror shakespeare_chat_dashboard.py's STYLE_EMBEDDER_WEIGHTS
    extended to 18 views. Sum of weights = 1.0.

Hard rules:
    * No daemon → error
    * No extended centroids file → error
    * Either memory_id missing or text empty → error
"""
from __future__ import annotations

import argparse
import logging
import sys
import time
from dataclasses import dataclass, field
from pathlib import Path

import numpy as np
import torch

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from _gemma_shakespeare_lib import Rpc  # noqa: E402
from v4_2.camd_lib import (  # noqa: E402
    EMB, SONNETS_CONSTELLATION_ID, STYLE_WEIGHTS as DAEMON_STYLE_WEIGHTS,
)
from v4_2.extended_ensemble import ExtendedEmbedderEnsemble  # noqa: E402
from v4_2.extended_store import (  # noqa: E402
    DEFAULT_PATH as DEFAULT_STORE_PATH,
    ExtendedStore, content_hash,
)


logger = logging.getLogger("v4_2.extended_camd")


DEFAULT_CENTROIDS_PATH = Path(
    "~/.contextgraph/models/extended_ensemble/sonnets_constellation_extended.npz"
).expanduser()

QWEN3_PROMPT = (
    "Classify the literary style of the text as Shakespearean "
    "early modern English verse or as ordinary modern English prose."
)


# Extended-view weights — used to aggregate the 5 new views into
# a single extended-style score. These are separate from DAEMON_STYLE_WEIGHTS.
# BGE reranker is NOT a cosine — it's a logit; we treat it as a bonus signal
# via sigmoid + blend, NOT added to cosine sum directly. See to_combined_reward.
EXTENDED_STYLE_WEIGHTS = {
    "bge_m3_dense":      0.15,
    "bge_m3_colbert":    0.15,
    "jina_v3_classif":   0.30,   # Agent 1 smoke: best discrimination (+0.1064)
    "qwen3_instruct":    0.30,   # Agent 1 smoke: +0.0934
    "bge_m3_sparse":     0.10,   # LEXICAL view, kept at lower weight
}
assert abs(sum(EXTENDED_STYLE_WEIGHTS.values()) - 1.0) < 1e-9

# Overall blend: α from daemon-13 style, (1−α) from extended-5 style
DEFAULT_ALPHA_DAEMON = 0.40  # extended is generally more style-discriminative


@dataclass
class ExtendedScoreResult:
    """Output of ExtendedConstellationScorer.score_*()."""
    text_prefix: str                    # first 60 chars (for logs)
    content_hash: str
    daemon_view_cosines: list[float]    # 13 floats, E1..E13
    daemon_combined_score: float        # mean over covered daemon views
    daemon_style_score: float           # weighted over STYLE daemon views
    extended_view_cosines: dict[str, float]   # 5 per-view cosines
    extended_combined_score: float      # weighted over EXTENDED_STYLE_WEIGHTS
    reranker_score: float | None        # cross-encoder logit, query=SHAKESPEARE_QUERY
    combined_reward: float              # α*daemon_style + (1-α)*extended_combined
    alpha_daemon: float
    memory_id: str | None               # if scored by id
    from_cache: bool                    # True if extended came from LMDB, False if fresh

    def summary(self) -> str:
        return (
            f"[{self.text_prefix[:40]}...] combined={self.combined_reward:+.3f} "
            f"daemon_style={self.daemon_style_score:+.3f} "
            f"extended={self.extended_combined_score:+.3f} "
            f"reranker={self.reranker_score:+.3f}" if self.reranker_score is not None
            else f"[{self.text_prefix[:40]}...] combined={self.combined_reward:+.3f}"
        )


def _sparse_cosine(a: dict[int, float], b: dict[int, float]) -> float:
    """Sparse dot-product cosine. Matches extended_ensemble._sparse_sim()."""
    if not a or not b:
        return 0.0
    ks = set(a) & set(b)
    if not ks:
        return 0.0
    dot = sum(a[k] * b[k] for k in ks)
    na = (sum(v * v for v in a.values())) ** 0.5
    nb = (sum(v * v for v in b.values())) ** 0.5
    if na < 1e-12 or nb < 1e-12:
        return 0.0
    return dot / (na * nb)


def _cosine(a: np.ndarray, b: np.ndarray) -> float:
    na = float(np.linalg.norm(a))
    nb = float(np.linalg.norm(b))
    if na < 1e-9 or nb < 1e-9:
        return 0.0
    return float(np.dot(a, b) / (na * nb))


def _colbert_vs_centroid(colbert: np.ndarray, centroid: np.ndarray) -> float:
    """Mean-pool colbert to 1024D then cosine vs the ColBERT-centroid."""
    pooled = colbert.mean(axis=0)
    return _cosine(pooled, centroid)


class ExtendedConstellationScorer:
    """Combined daemon + extended view scorer."""

    def __init__(self,
                 rpc: Rpc,
                 store_path: Path = DEFAULT_STORE_PATH,
                 centroids_path: Path = DEFAULT_CENTROIDS_PATH,
                 ensemble: ExtendedEmbedderEnsemble | None = None,
                 constellation_id: str = SONNETS_CONSTELLATION_ID,
                 alpha_daemon: float = DEFAULT_ALPHA_DAEMON,
                 qwen3_prompt: str = QWEN3_PROMPT,
                 device: str = "cuda",
                 dtype: torch.dtype = torch.bfloat16) -> None:
        self.rpc = rpc
        self.constellation_id = constellation_id
        self.alpha_daemon = alpha_daemon
        self.qwen3_prompt = qwen3_prompt

        # 1. Open store (read-only — we don't write here)
        self.store = ExtendedStore(store_path, readonly=True)

        # 2. Load extended centroids
        if not centroids_path.exists():
            raise FileNotFoundError(
                f"extended centroids not found: {centroids_path}. "
                f"Run `extended_constellation.py` after backfill."
            )
        npz = np.load(centroids_path, allow_pickle=True)
        self.cent_bge_dense = npz["bge_m3_dense_centroid"]
        self.cent_jina = npz["jina_v3_centroid"]
        self.cent_qwen = npz["qwen3_centroid"]
        self.cent_colbert = npz["bge_m3_colbert_centroid"]
        self.cent_sparse_ids = npz["bge_m3_sparse_centroid_ids"]
        self.cent_sparse_weights = npz["bge_m3_sparse_centroid_weights"]
        self.n_members_in_centroid = int(npz["n_members"])
        logger.info(
            f"ExtendedConstellationScorer loaded centroids from {centroids_path} "
            f"({self.n_members_in_centroid} members)"
        )

        # 3. Load ensemble lazily — heavy VRAM cost
        self.ensemble = ensemble
        self._device = device
        self._dtype = dtype

    def _ensure_ensemble(self) -> None:
        if self.ensemble is None:
            logger.info("loading ExtendedEmbedderEnsemble on demand...")
            t0 = time.monotonic()
            self.ensemble = ExtendedEmbedderEnsemble(device=self._device, dtype=self._dtype)
            logger.info(f"ensemble loaded in {(time.monotonic()-t0)*1000:.0f} ms")

    def _sparse_centroid_as_dict(self) -> dict[int, float]:
        return {int(tid): float(w)
                for tid, w in zip(self.cent_sparse_ids, self.cent_sparse_weights)}

    def _compute_extended_cosines(
        self, bge_dense: np.ndarray, bge_colbert: np.ndarray,
        bge_sparse: dict[int, float], jina: np.ndarray, qwen: np.ndarray,
    ) -> dict[str, float]:
        return {
            "bge_m3_dense":    _cosine(bge_dense, self.cent_bge_dense),
            "bge_m3_colbert":  _colbert_vs_centroid(bge_colbert, self.cent_colbert),
            "bge_m3_sparse":   _sparse_cosine(bge_sparse, self._sparse_centroid_as_dict()),
            "jina_v3_classif": _cosine(jina, self.cent_jina),
            "qwen3_instruct":  _cosine(qwen, self.cent_qwen),
        }

    def _daemon_scores(self, text: str | None = None,
                        memory_id: str | None = None) -> tuple[list[float], float]:
        """Return (per_embedder_13, combined_score) from the daemon."""
        # If no memory_id, must store a probe
        if memory_id is None:
            if not text:
                raise ValueError("_daemon_scores: need text or memory_id")
            r = self.rpc.call("store_memory", {
                "content": text,
                "sessionId": "extended-camd-probe",
                "importance": 0.3,
                "rationale": f"camd probe: {text[:60]}",
            }, timeout=60.0)
            fp = r.get("fingerprintId")
            if not fp:
                raise RuntimeError(f"store_memory returned no fingerprintId: {r}")
            try:
                r = self.rpc.call("score_against_constellation", {
                    "constellationId": self.constellation_id,
                    "memoryId": fp,
                }, timeout=30.0)
            finally:
                # Cleanup probe
                try:
                    self.rpc.call("forget_concept", {"memoryId": fp}, timeout=15.0)
                except Exception:
                    pass
        else:
            r = self.rpc.call("score_against_constellation", {
                "constellationId": self.constellation_id,
                "memoryId": memory_id,
            }, timeout=30.0)

        per_emb = r.get("per_embedder_cosine") or []
        if len(per_emb) != 13:
            raise RuntimeError(
                f"daemon returned {len(per_emb)} views, expected 13"
            )
        combined = float(r.get("combined_score") or 0.0)
        return [float(x) for x in per_emb], combined

    def score_text(self, text: str, include_reranker: bool = True
                    ) -> ExtendedScoreResult:
        """Score a fresh text (no memory_id). Computes all views live."""
        if not text or not text.strip():
            raise ValueError("score_text: empty text")

        # Extended views — compute fresh
        self._ensure_ensemble()
        bge = self.ensemble.embed_bge_m3(text)
        jina = self.ensemble.embed_jina_v3(text, task="classification")
        qwen = self.ensemble.embed_qwen3(text, task_prompt=self.qwen3_prompt)
        ext_cos = self._compute_extended_cosines(
            bge_dense=np.asarray(bge.dense, dtype=np.float32),
            bge_colbert=np.asarray(bge.colbert, dtype=np.float32),
            bge_sparse={int(k): float(v) for k, v in bge.sparse.items()},
            jina=np.asarray(jina, dtype=np.float32),
            qwen=np.asarray(qwen, dtype=np.float32),
        )

        # Reranker
        rr = None
        if include_reranker:
            rr = float(self.ensemble.rerank("Shakespearean verse about summer",
                                             [text])[0])

        # Daemon views
        daemon_cos, daemon_combined = self._daemon_scores(text=text)
        daemon_style = sum(w * daemon_cos[idx]
                            for idx, w in DAEMON_STYLE_WEIGHTS.items())

        # Extended combined
        ext_combined = sum(EXTENDED_STYLE_WEIGHTS[k] * v for k, v in ext_cos.items())

        reward = (self.alpha_daemon * daemon_style
                  + (1 - self.alpha_daemon) * ext_combined)

        return ExtendedScoreResult(
            text_prefix=text[:60],
            content_hash=content_hash(text),
            daemon_view_cosines=daemon_cos,
            daemon_combined_score=daemon_combined,
            daemon_style_score=daemon_style,
            extended_view_cosines=ext_cos,
            extended_combined_score=ext_combined,
            reranker_score=rr,
            combined_reward=reward,
            alpha_daemon=self.alpha_daemon,
            memory_id=None,
            from_cache=False,
        )

    def score_memory_id(self, memory_id: str) -> ExtendedScoreResult:
        """Score by memory_id. Uses cached extended from LMDB if present;
        computes fresh otherwise.
        """
        # 1. Get content so we can optionally recompute if cache miss
        r = self.rpc.call("get_memory_fingerprint", {
            "memoryId": memory_id, "includeContent": True,
            "includeVectorNorms": False,
        }, timeout=30.0)
        text = r.get("content", "")
        if not text:
            raise RuntimeError(f"memory {memory_id} has no content")

        # 2. Extended — cache or compute
        if self.store.exists(memory_id):
            bge_dense = self.store.get_dense(memory_id, embedder="bge_m3_dense")
            bge_colbert = self.store.get_colbert(memory_id)
            bge_sparse = self.store.get_sparse(memory_id)
            jina = self.store.get_dense(memory_id, embedder="jina_v3_classif")
            qwen = self.store.get_dense(memory_id, embedder="qwen3_instruct")
            meta = self.store.get_meta(memory_id)
            rr = meta.reranker_score_vs_sonnets
            from_cache = True
        else:
            self._ensure_ensemble()
            bge = self.ensemble.embed_bge_m3(text)
            bge_dense = np.asarray(bge.dense, dtype=np.float32)
            bge_colbert = np.asarray(bge.colbert, dtype=np.float32)
            bge_sparse = {int(k): float(v) for k, v in bge.sparse.items()}
            jina = np.asarray(self.ensemble.embed_jina_v3(text, task="classification"),
                              dtype=np.float32)
            qwen = np.asarray(self.ensemble.embed_qwen3(text, task_prompt=self.qwen3_prompt),
                              dtype=np.float32)
            rr = float(self.ensemble.rerank("Shakespearean verse about summer", [text])[0])
            from_cache = False

        ext_cos = self._compute_extended_cosines(
            bge_dense=bge_dense, bge_colbert=bge_colbert, bge_sparse=bge_sparse,
            jina=jina, qwen=qwen,
        )

        daemon_cos, daemon_combined = self._daemon_scores(memory_id=memory_id)
        daemon_style = sum(w * daemon_cos[idx]
                            for idx, w in DAEMON_STYLE_WEIGHTS.items())
        ext_combined = sum(EXTENDED_STYLE_WEIGHTS[k] * v for k, v in ext_cos.items())
        reward = (self.alpha_daemon * daemon_style
                  + (1 - self.alpha_daemon) * ext_combined)

        return ExtendedScoreResult(
            text_prefix=text[:60], content_hash=content_hash(text),
            daemon_view_cosines=daemon_cos, daemon_combined_score=daemon_combined,
            daemon_style_score=daemon_style,
            extended_view_cosines=ext_cos, extended_combined_score=ext_combined,
            reranker_score=rr, combined_reward=reward,
            alpha_daemon=self.alpha_daemon,
            memory_id=memory_id, from_cache=from_cache,
        )


def _smoke() -> int:
    """Smoke test — scores real Sonnet 18 and real modern sample via extended CAMD."""
    import argparse
    ap = argparse.ArgumentParser()
    ap.add_argument("--store-path", type=Path, default=DEFAULT_STORE_PATH)
    ap.add_argument("--centroids-path", type=Path, default=DEFAULT_CENTROIDS_PATH)
    ap.add_argument("--alpha-daemon", type=float, default=DEFAULT_ALPHA_DAEMON)
    args = ap.parse_args()

    logging.basicConfig(level=logging.INFO,
                        format="%(asctime)s [%(name)s] %(levelname)s: %(message)s")

    SHAKESPEARE = (
        "Shall I compare thee to a summer's day? "
        "Thou art more lovely and more temperate: "
        "Rough winds do shake the darling buds of May, "
        "And summer's lease hath all too short a date."
    )
    MODERN = (
        "The weather today is pretty nice. "
        "I think I'll grab a coffee and head to the park."
    )

    rpc = Rpc()
    try:
        scorer = ExtendedConstellationScorer(
            rpc=rpc, store_path=args.store_path,
            centroids_path=args.centroids_path, alpha_daemon=args.alpha_daemon,
        )
        shk = scorer.score_text(SHAKESPEARE)
        mod = scorer.score_text(MODERN)
    finally:
        rpc.close()

    print()
    print("=" * 78)
    print(f"EXTENDED CAMD REWARD — alpha_daemon={args.alpha_daemon}")
    print("=" * 78)
    for label, r in [("Shakespeare (Sonnet 18)", shk), ("Modern prose", mod)]:
        print(f"\n{label}")
        print(f"  daemon_style_score:  {r.daemon_style_score:+.4f}")
        print(f"  daemon_combined:     {r.daemon_combined_score:+.4f}")
        print(f"  extended_combined:   {r.extended_combined_score:+.4f}")
        print(f"  reranker (logit):    {r.reranker_score:+.4f}")
        print(f"  combined_reward:     {r.combined_reward:+.4f}")
        print(f"  daemon_view_cosines (E1..E13):")
        for i, c in enumerate(r.daemon_view_cosines, start=1):
            print(f"    E{i:02d}: {c:+.3f}")
        print(f"  extended_view_cosines:")
        for k, v in r.extended_view_cosines.items():
            print(f"    {k:22s}: {v:+.3f}")

    gap = shk.combined_reward - mod.combined_reward
    print(f"\nCombined reward gap (Shakespeare − Modern): {gap:+.4f}")
    print(f"Daemon-style gap:   {shk.daemon_style_score - mod.daemon_style_score:+.4f}")
    print(f"Extended-style gap: {shk.extended_combined_score - mod.extended_combined_score:+.4f}")
    print(f"Reranker gap:       {shk.reranker_score - mod.reranker_score:+.4f}")
    print("=" * 78)
    if gap < 0.05:
        print(f"FAIL: combined reward gap {gap:+.4f} < 0.05")
        return 1
    print("PASS: extended CAMD discriminates Shakespeare vs modern.")
    return 0


if __name__ == "__main__":
    sys.exit(_smoke())
