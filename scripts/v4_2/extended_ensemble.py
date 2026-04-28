#!/usr/bin/env python3
"""v4.2 Extended Embedder Ensemble - 4 new dense/multi-vec views + 1 reranker.

E14 BGE-M3 (dense + sparse + ColBERT) via FlagEmbedding.BGEM3FlagModel
E15 Jina-embeddings-v3 (task LoRA) via sentence-transformers
E16 Qwen3-Embedding-0.6B (instruction-tuned) via sentence-transformers
R   BGE-reranker-v2-m3 (cross-encoder) via AutoModelForSequenceClassification

Hard rules: NO fallbacks, NO mock data, robust logging, NO backwards compat,
error-out on unexpected dims. Refs: memory/decisions/ and memory/discoveries/
under agent_id=agent-v42-1-ensemble-builder.
"""
from __future__ import annotations

import logging
import sys
import time
import traceback
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Literal

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from v4_2.cuda13_blackwell import (  # noqa: E402
    apply_preimport_cuda_env,
    configure_torch_cuda,
    release_torch_cuda,
)

apply_preimport_cuda_env()

# Compat shim for FlagEmbedding on transformers 5.x (see discovery note).
import transformers.utils.import_utils as _iu  # noqa: E402
if not hasattr(_iu, "is_torch_fx_available"):
    _iu.is_torch_fx_available = lambda: False  # type: ignore[attr-defined]

import numpy as np  # noqa: E402
import torch  # noqa: E402

logging.basicConfig(level=logging.INFO,
                    format="%(asctime)s [%(name)s] %(levelname)s: %(message)s")
logger = logging.getLogger("v4_2.extended_ensemble")

BGE_M3_MODEL_ID       = "BAAI/bge-m3"
JINA_V3_MODEL_ID      = "jinaai/jina-embeddings-v3"
QWEN3_MODEL_ID        = "Qwen/Qwen3-Embedding-0.6B"
BGE_RERANKER_MODEL_ID = "BAAI/bge-reranker-v2-m3"

EXPECTED_DIMS: dict[str, int] = {
    "bge_m3_dense": 1024, "bge_m3_colbert": 1024,
    "jina_v3": 1024, "qwen3": 1024,
}

_VALID_JINA_TASKS = {
    "classification", "text-matching",
    "retrieval.query", "retrieval.passage", "separation",
}


# -- data classes -----------------------------------------------------------
@dataclass
class BGEM3Output:
    """Triple-view output from one BGE-M3 forward pass."""
    dense:   np.ndarray                # shape (1024,)
    sparse:  dict[int, float]          # vocab token id -> SPLADE-style weight
    colbert: np.ndarray                # shape (seq_len, 1024)

    def __repr__(self) -> str:
        return (f"BGEM3Output(dense={self.dense.shape}, "
                f"sparse[{len(self.sparse)}], colbert={self.colbert.shape})")


# -- helpers ----------------------------------------------------------------
def _cuda_free_mib() -> int:
    if not torch.cuda.is_available():
        return -1
    free, _ = torch.cuda.mem_get_info()
    return int(free / (1024 * 1024))


def _die(model_id: str, phase: str, exc: BaseException) -> RuntimeError:
    """Build a RuntimeError with full diagnostics for a failed phase."""
    msg = (f"extended_ensemble: {phase} failed for {model_id} "
           f"(cuda_free={_cuda_free_mib()} MiB): "
           f"{type(exc).__name__}: {exc}")
    logger.error("FATAL: %s", msg)
    return RuntimeError(msg)


def _assert_dim(got: int, expected: int, name: str) -> None:
    if got != expected:
        raise RuntimeError(
            f"extended_ensemble: dim mismatch for {name}: "
            f"expected {expected}, got {got}. HF may have shipped a "
            "retrained variant; refusing to proceed."
        )


def _params_million(module: Any) -> float:
    try:
        return sum(p.numel() for p in module.parameters()) / 1e6
    except Exception:
        return float("nan")


def _weights_fingerprint(module: Any, n_first: int = 1000) -> str:
    """Xor-hash of first N f32 values of the first tensor (cheap provenance)."""
    try:
        for p in module.parameters():
            arr = p.detach().float().flatten().cpu().numpy()[:n_first]
            h = int(np.bitwise_xor.reduce(arr.view(np.int32))) & 0xFFFFFFFF
            return f"{h:08x}"
    except Exception as e:
        return f"unavailable:{type(e).__name__}"
    return "empty"


def _cosine_norm(a: np.ndarray, b: np.ndarray) -> float:
    """Cosine similarity mapped to [0,1] via (cos+1)/2 (matches SRC-3)."""
    na = float(np.linalg.norm(a))
    nb = float(np.linalg.norm(b))
    if na == 0.0 or nb == 0.0:
        return 0.5
    return (float(np.dot(a, b) / (na * nb)) + 1.0) / 2.0


def _patch_jina_v3_for_transformers5() -> None:
    """Patch XLMRobertaLoRA on transformers 5.x to populate all_tied_weights_keys.
    See memory/discoveries/agent-v42-1-ensemble-builder--jina-v3-transformers5-compat.md
    """
    from transformers import AutoModel  # type: ignore
    try:
        AutoModel.from_pretrained(JINA_V3_MODEL_ID, trust_remote_code=True)
    except AttributeError as e:
        if "all_tied_weights_keys" not in str(e):
            raise
    except Exception:
        pass  # a non-compat error will re-surface on the real load path.
    target = None
    for name, mod in list(sys.modules.items()):
        if name.endswith(".modeling_lora") and "jinaai" in name:
            target = getattr(mod, "XLMRobertaLoRA", None)
            if target is not None:
                break
    if target is None:
        raise RuntimeError(
            "extended_ensemble: Jina-v3 compat patch failed -- "
            "XLMRobertaLoRA not found in sys.modules after probe.")
    if getattr(target, "_v42_patched", False):
        return
    orig_init = target.__init__

    def patched(self, *a, **kw):
        orig_init(self, *a, **kw)
        if not hasattr(self, "all_tied_weights_keys"):
            try:
                self.post_init()
            except Exception:
                self.all_tied_weights_keys = {}
    target.__init__ = patched
    target._v42_patched = True


# -- main ensemble class ----------------------------------------------------
class ExtendedEmbedderEnsemble:
    """Loads 4 embedders + 1 reranker. Fail-fast on any load error."""

    def __init__(self, device: str = "cuda",
                 dtype: torch.dtype = torch.bfloat16) -> None:
        if device == "cuda" and not torch.cuda.is_available():
            raise RuntimeError("extended_ensemble: device='cuda' requested "
                               "but torch.cuda.is_available() is False.")
        if device == "cuda":
            configure_torch_cuda(torch, label="v4.2-extended-ensemble")
        self.device = device
        self.dtype = dtype
        logger.info("Ensemble init: device=%s dtype=%s cuda_free=%d MiB",
                    device, dtype, _cuda_free_mib())
        self._load_bge_m3()
        self._load_jina_v3()
        self._load_qwen3()
        self._load_reranker()
        logger.info("Ensemble ready. cuda_free=%d MiB", _cuda_free_mib())

    def _load_bge_m3(self) -> None:
        t0 = time.time()
        logger.info("Loading BGE-M3 (%s) via FlagEmbedding.BGEM3FlagModel",
                    BGE_M3_MODEL_ID)
        try:
            from FlagEmbedding import BGEM3FlagModel  # type: ignore
            self.bge_m3 = BGEM3FlagModel(
                BGE_M3_MODEL_ID, use_fp16=True, devices=[self.device],
            )
        except BaseException as e:
            raise _die(BGE_M3_MODEL_ID, "load BGE-M3", e) from e
        logger.info("BGE-M3 loaded in %d ms. cuda_free=%d MiB",
                    int((time.time() - t0) * 1000), _cuda_free_mib())

    def _load_jina_v3(self) -> None:
        t0 = time.time()
        logger.info("Loading Jina-v3 (%s) via sentence_transformers",
                    JINA_V3_MODEL_ID)
        try:
            _patch_jina_v3_for_transformers5()
            from sentence_transformers import SentenceTransformer  # type: ignore
            self.jina_v3 = SentenceTransformer(
                JINA_V3_MODEL_ID, trust_remote_code=True, device=self.device,
                model_kwargs={"torch_dtype": self.dtype},
            )
        except BaseException as e:
            raise _die(JINA_V3_MODEL_ID, "load Jina-v3", e) from e
        dim = int(self.jina_v3.get_sentence_embedding_dimension() or -1)
        _assert_dim(dim, EXPECTED_DIMS["jina_v3"], "jina_v3")
        logger.info("Jina-v3 loaded in %d ms dim=%d cuda_free=%d MiB",
                    int((time.time() - t0) * 1000), dim, _cuda_free_mib())

    def _load_qwen3(self) -> None:
        t0 = time.time()
        logger.info("Loading Qwen3-Embedding-0.6B (%s) via sentence_transformers",
                    QWEN3_MODEL_ID)
        try:
            from sentence_transformers import SentenceTransformer  # type: ignore
            self.qwen3 = SentenceTransformer(
                QWEN3_MODEL_ID, trust_remote_code=True, device=self.device,
                model_kwargs={"torch_dtype": self.dtype},
            )
        except BaseException as e:
            raise _die(QWEN3_MODEL_ID, "load Qwen3-Embedding", e) from e
        dim = int(self.qwen3.get_sentence_embedding_dimension() or -1)
        _assert_dim(dim, EXPECTED_DIMS["qwen3"], "qwen3")
        logger.info("Qwen3 loaded in %d ms dim=%d cuda_free=%d MiB",
                    int((time.time() - t0) * 1000), dim, _cuda_free_mib())

    def _load_reranker(self) -> None:
        t0 = time.time()
        logger.info("Loading BGE-reranker-v2-m3 (%s) via AutoModelForSequenceClassification",
                    BGE_RERANKER_MODEL_ID)
        try:
            from transformers import (  # type: ignore
                AutoModelForSequenceClassification, AutoTokenizer,
            )
            self.rerank_tok = AutoTokenizer.from_pretrained(BGE_RERANKER_MODEL_ID)
            self.rerank_model = AutoModelForSequenceClassification.from_pretrained(
                BGE_RERANKER_MODEL_ID, torch_dtype=torch.float16,
            ).to(self.device)
            self.rerank_model.eval()
        except BaseException as e:
            raise _die(BGE_RERANKER_MODEL_ID, "load BGE-reranker-v2-m3", e) from e
        logger.info("BGE-reranker loaded in %d ms cuda_free=%d MiB",
                    int((time.time() - t0) * 1000), _cuda_free_mib())

    # -- encoders -----------------------------------------------------------
    def embed_bge_m3(self, text: str) -> BGEM3Output:
        if not text or not text.strip():
            raise RuntimeError("embed_bge_m3: text is empty")
        try:
            out = self.bge_m3.encode(
                [text], return_dense=True, return_sparse=True,
                return_colbert_vecs=True, max_length=8192,
            )
        except BaseException as e:
            raise _die(BGE_M3_MODEL_ID, "encode BGE-M3", e) from e
        dense = np.asarray(out["dense_vecs"][0], dtype=np.float32)
        _assert_dim(int(dense.shape[-1]), EXPECTED_DIMS["bge_m3_dense"],
                    "bge_m3_dense")
        sparse = {int(k): float(v) for k, v in out["lexical_weights"][0].items()}
        colbert = np.asarray(out["colbert_vecs"][0], dtype=np.float32)
        _assert_dim(int(colbert.shape[-1]), EXPECTED_DIMS["bge_m3_colbert"],
                    "bge_m3_colbert")
        return BGEM3Output(dense=dense, sparse=sparse, colbert=colbert)

    def embed_jina_v3(self, text: str,
                      task: Literal[
                          "classification", "text-matching",
                          "retrieval.query", "retrieval.passage", "separation",
                      ] = "classification") -> np.ndarray:
        if not text or not text.strip():
            raise RuntimeError("embed_jina_v3: text is empty")
        if task not in _VALID_JINA_TASKS:
            raise RuntimeError(f"embed_jina_v3: unknown task '{task}'; "
                               f"valid: {sorted(_VALID_JINA_TASKS)}")
        try:
            vec = self.jina_v3.encode(
                [text], task=task, prompt_name=task,
                convert_to_numpy=True, normalize_embeddings=True,
            )
        except BaseException as e:
            raise _die(JINA_V3_MODEL_ID, f"encode Jina-v3 task={task}", e) from e
        v = np.asarray(vec[0], dtype=np.float32)
        _assert_dim(int(v.shape[-1]), EXPECTED_DIMS["jina_v3"], "jina_v3")
        return v

    def embed_qwen3(self, text: str, task_prompt: str) -> np.ndarray:
        """Instruction-conditioned embedding. task_prompt MUST be non-empty."""
        if not text or not text.strip():
            raise RuntimeError("embed_qwen3: text is empty")
        if not task_prompt or not task_prompt.strip():
            raise RuntimeError("embed_qwen3: task_prompt is empty. "
                               "Qwen3-Embedding is instruction-tuned; provide "
                               "a task_prompt.")
        prompt = f"Instruct: {task_prompt.strip()}\nQuery: "
        try:
            vec = self.qwen3.encode(
                [text], prompt=prompt,
                convert_to_numpy=True, normalize_embeddings=True,
            )
        except BaseException as e:
            raise _die(QWEN3_MODEL_ID, "encode Qwen3-Embedding", e) from e
        v = np.asarray(vec[0], dtype=np.float32)
        _assert_dim(int(v.shape[-1]), EXPECTED_DIMS["qwen3"], "qwen3")
        return v

    @torch.no_grad()
    def rerank(self, query: str, passages: list[str]) -> list[float]:
        if not query or not query.strip():
            raise RuntimeError("rerank: query is empty")
        if not passages:
            raise RuntimeError("rerank: passages list is empty")
        for i, p in enumerate(passages):
            if not p or not str(p).strip():
                raise RuntimeError(f"rerank: passages[{i}] is empty")
        pairs = [(query, p) for p in passages]
        try:
            inputs = self.rerank_tok(
                pairs, padding=True, truncation=True,
                return_tensors="pt", max_length=8192,
            ).to(self.device)
            logits = self.rerank_model(**inputs, return_dict=True
                                       ).logits.view(-1).float().cpu().numpy()
        except BaseException as e:
            raise _die(BGE_RERANKER_MODEL_ID, "rerank forward", e) from e
        return [float(x) for x in logits]

    def describe(self) -> dict[str, dict[str, Any]]:
        bge_inner = getattr(self.bge_m3, "model", None) or self.bge_m3
        jina_inner = getattr(self.jina_v3, "_first_module", lambda: self.jina_v3)()
        qwen_inner = getattr(self.qwen3, "_first_module", lambda: self.qwen3)()

        def _row(name, module, dim, ctx, framework):
            return {"name": name,
                    "params_million": round(_params_million(module), 2),
                    "dim": dim, "context_length": ctx,
                    "framework": framework,
                    "weights_hash": _weights_fingerprint(module)}
        return {
            "bge_m3": _row(BGE_M3_MODEL_ID, bge_inner,
                           EXPECTED_DIMS["bge_m3_dense"], 8192,
                           "FlagEmbedding.BGEM3FlagModel"),
            "jina_v3": _row(JINA_V3_MODEL_ID, jina_inner,
                            EXPECTED_DIMS["jina_v3"], 8194,
                            "sentence-transformers"),
            "qwen3": _row(QWEN3_MODEL_ID, qwen_inner,
                          EXPECTED_DIMS["qwen3"], 32768,
                          "sentence-transformers"),
            "bge_reranker_v2_m3": _row(BGE_RERANKER_MODEL_ID, self.rerank_model,
                                       1, 8192,
                                       "transformers.AutoModelForSequenceClassification"),
        }


# -- smoke test -------------------------------------------------------------
SHAKESPEARE_SONNET18 = (
    "Shall I compare thee to a summer's day? "
    "Thou art more lovely and more temperate: "
    "Rough winds do shake the darling buds of May, "
    "And summer's lease hath all too short a date."
)
MODERN_WEATHER = (
    "The weather today is pretty nice. "
    "It's not too hot and not too cold, "
    "which makes it a good day to go for a walk. "
    "I think I'll head to the park after work."
)
# Sonnet 29 opening — a second real Shakespeare passage, used as the style
# anchor. The Sonnet 18 candidate should land closer to it than the modern
# passage does.
SHAKESPEARE_ANCHOR = (
    "When in disgrace with fortune and men's eyes, "
    "I all alone beweep my outcast state, "
    "And trouble deaf heaven with my bootless cries, "
    "And look upon myself and curse my fate."
)
MIN_GAP = 0.05


def _sparse_sim(a: dict[int, float], b: dict[int, float]) -> float:
    if not a or not b:
        return 0.5
    na = sum(v * v for v in a.values()) ** 0.5
    nb = sum(v * v for v in b.values()) ** 0.5
    if na == 0.0 or nb == 0.0:
        return 0.5
    sm, lg = (a, b) if len(a) <= len(b) else (b, a)
    raw = sum(lg.get(k, 0.0) * v for k, v in sm.items()) / (na * nb)
    return max(0.0, min(1.0, (raw + 1.0) / 2.0))


def _colbert_maxsim(q: np.ndarray, d: np.ndarray) -> float:
    if q.size == 0 or d.size == 0:
        return 0.5
    qn = q / (np.linalg.norm(q, axis=1, keepdims=True) + 1e-12)
    dn = d / (np.linalg.norm(d, axis=1, keepdims=True) + 1e-12)
    return float(((qn @ dn.T).max(axis=1).mean() + 1.0) / 2.0)


def _smoke_test() -> int:
    t0 = time.time()
    vram_start = _cuda_free_mib()
    logger.info("Smoke-test start. cuda_free=%d MiB", vram_start)

    try:
        ens = ExtendedEmbedderEnsemble()
    except BaseException as e:
        logger.error("ENSEMBLE INIT FAILED: %s", e)
        traceback.print_exc()
        return 1

    # Encode all three styles through all three embedders.
    bge_shk = ens.embed_bge_m3(SHAKESPEARE_SONNET18)
    bge_mod = ens.embed_bge_m3(MODERN_WEATHER)
    bge_anc = ens.embed_bge_m3(SHAKESPEARE_ANCHOR)
    jna_shk = ens.embed_jina_v3(SHAKESPEARE_SONNET18)
    jna_mod = ens.embed_jina_v3(MODERN_WEATHER)
    jna_anc = ens.embed_jina_v3(SHAKESPEARE_ANCHOR)
    qw_prompt = ("Classify the literary style of the text as Shakespearean "
                 "early modern English verse or as ordinary modern English prose.")
    qw_shk = ens.embed_qwen3(SHAKESPEARE_SONNET18, task_prompt=qw_prompt)
    qw_mod = ens.embed_qwen3(MODERN_WEATHER,       task_prompt=qw_prompt)
    qw_anc = ens.embed_qwen3(SHAKESPEARE_ANCHOR,   task_prompt=qw_prompt)

    # gap = sim(anchor, Shakespeare) - sim(anchor, modern); >= MIN_GAP means
    # the embedder correctly prefers the Shakespeare candidate.
    # STYLE-gating set (MUST pass): dense views of all three embedders + colbert.
    # INFO-only: bge_m3_sparse — it measures lexical overlap, not style.
    # See: memory/discoveries/agent-v42-1-ensemble-builder--bge-m3-sparse-is-lexical-not-style.md
    gate = [
        ("bge_m3_dense",           _cosine_norm(bge_anc.dense,   bge_shk.dense),
                                   _cosine_norm(bge_anc.dense,   bge_mod.dense)),
        ("bge_m3_colbert",         _colbert_maxsim(bge_anc.colbert, bge_shk.colbert),
                                   _colbert_maxsim(bge_anc.colbert, bge_mod.colbert)),
        ("jina_v3_classification", _cosine_norm(jna_anc, jna_shk),
                                   _cosine_norm(jna_anc, jna_mod)),
        ("qwen3_instruct",         _cosine_norm(qw_anc, qw_shk),
                                   _cosine_norm(qw_anc, qw_mod)),
    ]
    info = [
        ("bge_m3_sparse (INFO)",   _sparse_sim(bge_anc.sparse, bge_shk.sparse),
                                   _sparse_sim(bge_anc.sparse, bge_mod.sparse)),
    ]
    checks = [(n, s, m, s - m) for (n, s, m) in gate]
    info_rows = [(n, s, m, s - m) for (n, s, m) in info]

    # Reranker: query asks for Shakespearean verse — Shakespeare > modern.
    rr_shk, rr_mod = ens.rerank("Shakespearean verse about summer",
                                [SHAKESPEARE_SONNET18, MODERN_WEATHER])
    rr_gap = rr_shk - rr_mod

    print()
    bar = "=" * 74
    print(bar)
    print(f"{'embedder':30} {'shk_sim':>10} {'mod_sim':>10} {'gap':>10} {'PASS?':>8}")
    print("-" * 74)
    failed: list[str] = []
    for name, s, m, g in checks:
        ok = g >= MIN_GAP
        if not ok:
            failed.append(name)
        print(f"{name:30} {s:10.4f} {m:10.4f} {g:+10.4f} "
              f"{'PASS' if ok else 'FAIL':>8}")
    rr_ok = rr_gap > 0.0
    if not rr_ok:
        failed.append("bge_reranker_v2_m3")
    print(f"{'bge_reranker_v2_m3 (logits)':30} {rr_shk:10.4f} {rr_mod:10.4f} "
          f"{rr_gap:+10.4f} {'PASS' if rr_ok else 'FAIL':>8}")
    for name, s, m, g in info_rows:
        print(f"{name:30} {s:10.4f} {m:10.4f} {g:+10.4f} {'INFO':>8}")
    print(bar)

    print("\nModel provenance:")
    for k, v in ens.describe().items():
        print(f"  {k}: params={v['params_million']}M dim={v['dim']} "
              f"ctx={v['context_length']} hash={v['weights_hash']} "
              f"framework={v['framework']}")

    vram_end = _cuda_free_mib()
    vram_delta_mib = vram_start - vram_end
    print(f"\nVRAM used by ensemble: {vram_delta_mib / 1024.0:.2f} GB "
          f"(free before: {vram_start} MiB, after: {vram_end} MiB)")
    if vram_delta_mib > 15 * 1024:
        print(f"FAIL: VRAM usage exceeds 15 GB budget.")
        failed.append("vram_budget")

    print(f"Total wall time: {time.time() - t0:.1f}s")

    del ens
    release_torch_cuda(torch)

    if failed:
        print(f"\nSMOKE TEST FAIL: {len(failed)} check(s) below "
              f"MIN_GAP={MIN_GAP} or over budget: {failed}")
        return 1
    print(f"\nSMOKE TEST PASS: all {len(checks) + 1} checks OK "
          f"(MIN_GAP={MIN_GAP}).")
    return 0


if __name__ == "__main__":
    sys.exit(_smoke_test())
