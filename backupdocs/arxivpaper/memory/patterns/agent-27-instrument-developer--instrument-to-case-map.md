---
namespace: patterns
agent_id: agent-27-instrument-developer
timestamp: 2026-04-16T16:10:00Z
status: active
tags: instruments, case-map, crosswalk, writer-reference
depends_on:
  - memory/decisions/agent-27-instrument-developer--instrument-specifications.md
  - memory/decisions/agent-25-method-designer--case-study-protocols.md
  - memory/decisions/agent-26-sampling-strategist--sampling-protocols.md
downstream_consumers:
  - agent-37-methodology-writer
  - agent-38-results-writer
  - agent-42-adversarial-reviewer
---

# Instrument ↔ Case Map — Which Instruments Are Used Where

## 0. Purpose

A single table that lets a writer look up "which instruments does
Case $X$ require?" and a reviewer look up "which case uses
instrument $Y$?". Every cell is a forward pointer to
`agent-27-instrument-developer--instrument-specifications.md` §.

---

## 1. Forward map — instruments per case

### 1.1 Case 1 — Shakespeare stylistic LoRA

| Instrument | §in specs | Role | Tier |
|---|---|---|---|
| STYLE_CENTROID (9-embedder constellation) | D.1 | Primary guard (Layer 3); per-embedder cosine threshold conjunction | T1 PENDING |
| Vocabulary-density prefilter | D.2 | Layer-2 auxiliary (constrained decoding fast gate) | T1 PENDING |
| ColBERT / BGE-reranker cross-encoder (W1 remediation) | D.1 footnote | Independent cross-encoder for W1 | Instrument ready, run pending |
| Content SHA-256 | F.1 | Reference-set manifest + audit | Operational |

**Used instruments**: D.1, D.2, F.1. (Plus W1 remediation pointer.)

### 1.2 Case 2 — Santa identity-locked talking-head clone

| Instrument | §in specs | Role | Tier |
|---|---|---|---|
| SigLIP visual-centroid cosine (global visual) | C.1 | Global visual modality cosine scorer | T3 PROJECTED |
| FLAME 100-D expression coefficients | C.2 | Level-0 / Level-1 action-unit vocabulary (NOT the guard) | Documented, not a guard |
| WavLM-SECS (re-used) | A.1 | Speaker-modality cosine scorer (τ_speaker = 0.80 operating) | T3 PROJECTED |
| Emotion encoder (out-of-scope for agent-27; flagged open) | — | Emotion modality cosine (τ_emotion = 0.60 operating) | Open carry |
| Voice-identity encoder (out-of-scope for agent-27; flagged open) | — | Voice-identity modality cosine (τ_voice = 0.85 operating) | Open carry |
| LSE-D / LSE-C (SyncNet) | E.1 | Lip-sync quality, **PENDING / future work** | PENDING |
| FID on held-out visual | — | Auxiliary visual quality, out of agent-27 scope | PENDING |
| Content SHA-256 | F.1 | Manifest + provenance | Operational |
| YOLOv5-Face + WavLM-large diarisation + `prosody_segments` filter | G.2 | Screening gates | Operational |

**Used instruments (locked)**: A.1 (speaker), C.1 (visual), C.2 (action-unit vocab only), F.1, G.2.
**Open carries**: emotion encoder, voice-identity encoder (not specified by agent-22 / agent-26 at model-ID granularity; agent-37 must resolve before §6.2 writes).
**PENDING**: E.1 (LSE-D).

### 1.3 Case 3 — Voice clone beyond 0.95 WavLM SECS

| Instrument | §in specs | Role | Tier |
|---|---|---|---|
| WavLM-SECS | A.1 | Primary guard (strict τ = 0.95) + evaluator (headline SECS) | **T1 MEASURED** |
| ECAPA-TDNN SECS | A.2 | W1 remediation cross-encoder | Instrument ready, run pending |
| Qwen3-TTS 2048-D internal speaker encoder | A.3 | Vendor-internal conditioning — **documented, not a cross-encoder** | Documentation only |
| DNSMOS P.808 (and P.835 optional) | B.1 | Naturalness parity with real recording | **T1 MEASURED (P.808)** |
| UTMOS | B.2 | Second naturalness scorer parity | **T1 MEASURED** |
| Bootstrap 95 % CI on N = 10 | Non-parametric bootstrap over A.1 outputs | Retrospective CI, not power | T1 MEASURED |
| Pre-selection distribution recorder (W2) | A.1 over 12 × 10 = 120 candidates | W2 remediation, isolates sampling-diversity from threshold rejection | Instrument ready, run pending |
| $n_m$-curve driver (W3) | A.1 with top-$n$ pools, $n \in \{10, 25, 50, 100\}$ | W3 remediation, ablation over centroid stability | Instrument ready, run pending |
| Content SHA-256 | F.1 | Reference-set + real-mic manifest | Operational |
| Screening gates (SNR / length / single-speaker / lexical-overlap) | G.3 | Eligibility gates | Operational |

**Used instruments (headline)**: A.1, B.1, B.2, F.1, G.3.
**W1 / W2 / W3 remediation (instrument ready, run pending)**: A.2, pre-selection recorder, $n_m$-curve driver.
**Documentation only**: A.3.

---

## 2. Reverse map — cases per instrument

| Instrument | Case 1 | Case 2 | Case 3 |
|---|---|---|---|
| A.1 WavLM-SECS | — | ✓ (speaker modality) | ✓ (guard + evaluator — headline) |
| A.2 ECAPA-TDNN SECS | — | — | ✓ (W1 remediation) |
| A.3 Qwen3-TTS internal speaker | — | — | documented only |
| B.1 DNSMOS | — | — | ✓ (P.808 headline; P.835 optional) |
| B.2 UTMOS | — | — | ✓ (headline) |
| C.1 SigLIP visual centroid | — | ✓ (global visual) | — |
| C.2 FLAME coefficients | — | documented (action-unit vocab) | — |
| D.1 STYLE_CENTROID (9-embedder) | ✓ (primary guard) | — | — |
| D.2 Vocabulary prefilter | ✓ (Layer-2) | — | — |
| E.1 LSE-D / LSE-C | — | PENDING | — |
| F.1 Content SHA-256 | ✓ | ✓ | ✓ |

**Observation**: WavLM-SECS is the single most re-used instrument
(Cases 2 and 3, in both guard and evaluator roles). The W1 cross-
encoder disclosure applies uniformly wherever A.1 appears.

---

## 3. Canonical phrasing across the manuscript

When §6 / §8 reference these instruments, writers must use the
canonical phrasing below (per agent-09 metrics inventory V4):

- **"WavLM SECS"** — never bare "SECS".
- **"ECAPA-TDNN SECS"** — always with "ECAPA" prefix.
- **"DNSMOS P.808"** or **"DNSMOS P.835"** — never bare "DNSMOS".
- **"UTMOS"** with citation on first use.
- **"Per-modality cosine against $c_m$"** — always name the encoder ($\phi_m$) and modality.
- **"SigLIP visual centroid cosine"** — not "image cosine" or bare "visual cosine".
- **"STYLE_CENTROID (9-embedder constellation)"** — first use; subsequently "STYLE_CENTROID".
- **"Content SHA-256"** — not bare "hash".

---

## 4. Writer instruction per section

- **§5 (Methods / TCT)**: reference A.1, A.2 briefly when specifying
  the guard; note that D.1 is a 9-embedder constellation with
  per-embedder CV reporting.
- **§6.1 (Case 1)**: list D.1 and D.2 in the instrument table;
  flag W1 remediation (cross-encoder ColBERT / BGE) per case-study
  protocols §1.6.
- **§6.2 (Case 2)**: list C.1 and A.1 (speaker re-use) in the
  instrument table; name C.2 as the action-unit vocabulary (not the
  guard); flag E.1 (LSE-D) as pending.
- **§6.3 (Case 3)**: list A.1, A.2, A.3, B.1, B.2 in the instrument
  table; report headline numbers (0.961 / 0.975 / 3.93 / 3.93 /
  3.012 / 2.997) with the exact preprocessing from §A.1, §B.1, §B.2.
  Disclose A.3 is not a cross-encoder check.
- **§8 (Limitations)**: paste the limitation codes L-SECS-A/B/C,
  L-ECAPA-A/B/C, L-Qwen3-A/B, L-DNSMOS-A/B/C, L-UTMOS-A/B/C,
  L-SigLIP-A/B/C, L-FLAME-A/B/C, L-STYLE-A/B/C, L-VOCAB-A/B/C,
  L-LSE-A/B/C, L-SHA-A/B directly from the instrument-limitations
  discovery file.

## Changelog

- 2026-04-16 — Initial issue by agent-27-instrument-developer.
