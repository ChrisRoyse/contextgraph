---
namespace: patterns
agent_id: agent-37-methodology-writer
timestamp: 2026-04-17T11:00:00Z
status: active
locked: false
supersedes: none
tags: writing-decisions, framework-section, method-section, dda, meaning-compression, tct, phase-1, phase-2, phase-3, hedges-applied, reserve-list-respected
depends_on:
  - memory/context/mission-brief.md
  - memory/context/constraints.md
  - memory/decisions/agent-21-theory-builder--theoretical-framework.md
  - memory/decisions/agent-25-method-designer--tct-protocol.md
  - memory/decisions/agent-25-method-designer--case-study-protocols.md
  - memory/patterns/agent-34-confidence-quantifier--confidence-language-for-writers.md
  - memory/handoff/agent-36-to-37--related-work-ready.md
downstream_consumers:
  - agent-38-results-writer
  - agent-39-discussion-writer
  - agent-42-adversarial-reviewer
---

# Agent-37 Writing Decisions — §3 Framework and §5 TCT Method

## 0. Scope of this note

This file records the writing decisions that produced
`sections/03_framework_dda_mc.md` (§3) and `sections/06_method_tct.md`
(§5). It is the companion note §5's downstream consumers (§6 results
writer, §7 discussion writer, §8 limitations writer) should read
before quoting any equation, acceptance predicate, or measured number
from §3 or §5.

## 1. Section 3 (DDA and Meaning Compression) — decisions

### 1.1 Structure

Four sub-sections, totalling ~1,780 words, in the order the task
brief specified:

- **§3.1** DDA counting identity with the constructive-upper-bound
  caveat.
- **§3.2** Meaning compression as the ratio view; fourth taxonomic
  entry, positioned as semantically distinct — **not** superior.
- **§3.3** Scope of the collapse-silent claim; Shumailov-form
  recursion regime; explicit scope-not-immunity framing.
- **§3.4** Three-role composition that binds DDA to §5 TCT.

The opener paragraph summarises the four moves so a reader who skims
§3 still lands on the structural contribution.

### 1.2 Hedges applied (§3)

| Hedge code | Where applied | Exact phrasing |
|---|---|---|
| **C1** | §3.1 identity (1) statement and worked instantiation | "**up to** $n \cdot (N + \binom{N}{2})$", "**up to** 91 signals per input", "structural upper bound, not an information-theoretic lower bound" |
| **C2** | §3.2 fourth-category claim | "semantically distinct unit pairs", "**reviewer acceptance of the taxonomic distinction is a scholarly judgement we do not pre-empt**", deliberate substitution "semantically" for the banned "dimensionally" |
| **C4** | §3.3 scope-of-collapse-silence | "**structurally not subject to**, rather than a refutation of", "scope argument, not an immunity theorem" |
| **V11** (approximate independence) | §3.1 axiom restatement + §3.3 scope argument | "approximately independently trained embedders"; hedge carried explicitly: "strict statistical independence between embedders is an open measurement question, not a derived property" |
| **V18** (frozen) | §3.1, §3.3, §3.4 every embedder reference | "**frozen** (V18)" with parenthetical V18 anchor on first mention; thereafter "frozen" unambiguously |
| **R3** (pool-relative novelty) | §3.4 role-composition claim | "**to our knowledge in the locked 88-citation pool that anchors this manuscript**, no cited paper uses the same frozen embedder panel in all three roles jointly" + "a pool-external counter-example would qualify the framing but would not invalidate the composition-level claim" |

### 1.3 Reserve-list compliance (§3)

Per agent-36's handoff, §3 must **not** re-cite items reserved by §2.
§3 satisfies this as follows:

- **DDA identity** (§3.1): no re-citation of `cormack2009rrf`,
  `malkov2018hnsw`. §2.4 had already introduced the retrieval
  literature; §3 presents the identity cleanly.
- **Meaning-compression fourth category** (§3.2): citations
  (`shannon1948`, `deletang2024`, `frantar2022gptq`, `frantar2023sparsegpt`,
  `kaplan2020scaling`, `hoffmann2022chinchilla`, `huang2024compression`)
  appear **only in the positioning paragraph** where the four categories
  are introduced as a taxonomy. No narrative recapitulation of their
  results.
- **Collapse-silence** (§3.3): cites `shumailov2024nature`,
  `dohmatob2024tails`, `alemohammad2023mad`, `gerstgrasser2024colm`,
  `feng2024verification` only to name the recursion regime whose
  preconditions DDA does not satisfy. No re-narration of their findings.

### 1.4 Critical word choices (§3)

- **"Constructive upper bound"** (not "tight count"): identity (1) is
  an upper bound under Axioms 1–2, tight under non-redundant embedders,
  strict under redundant ones.
- **"Signal"**: defined in §3.1 precisely — scalar or low-dimensional
  feature, deterministic under frozen embedders, indexed to an (input,
  embedder) or (input, embedder-pair) pair. Distinguished from
  synthetic-data samples, data augmentation, and distillation outputs.
- **"Scope argument, not an immunity theorem"** (§3.3): sentence-level
  explicit; prevents the most obvious adversarial misreading.
- **"Over identical parameter instances"** (§3.4): makes the
  three-role composition claim specific enough to falsify (a
  pool-external paper with three-role composition over identical
  instances would qualify).

## 2. Section 5 (TCT Method) — decisions

### 2.1 Structure

Six sub-sections, totalling ~2,550 words, in the order the task brief
specified:

- **§5.1** Definition of the constellation $C_P$ with cardinality
  $\sum_m n_m$.
- **§5.2** Phase 1 centroid construction with aggregation choices and
  failure modes.
- **§5.3** Phase 2 geometric-proximity training loss, freeze policy,
  inference-only mode.
- **§5.4** Phase 3 runtime guard $G_\tau$; calibration; rejection
  handling.
- **§5.5** Three-layer enforcement stack for text (Case 1).
- **§5.6** Case-specific protocol instantiations (1, 2, 3) with
  measured-versus-projected tier flags on every number.

The opening paragraph announces the level of specification
("independent team could re-implement without reading the codebase")
and the three hedges that will carry throughout.

### 2.2 Hedges applied (§5)

| Hedge code | Where applied | Exact phrasing |
|---|---|---|
| **C3** | §5.4 acceptance predicate + §5 opener | "**per-output verifiability relative to the chosen centroid panel**", paired with scope disclosure "what is verified … what is not verified" |
| **C5** | §5.6 Case-3 headline | "T1 measured", "mean WavLM SECS 0.961, max 0.975 on 10 held-out English sentences for one speaker", "**encoder-matched** scope (guard and evaluator both WavLM-base-plus-sv)", date and pipeline-version hash |
| **C7** | §5.6 Case 3 | "$K = 12$, $T = 0.3$ (measured)" — no claim of generality; temperature tied to Case 3 pipeline only |
| **C8** | §5.6 Case 3 | "$n_m = 50$" recorded as operating point; **sample-complexity curve named as pending** |
| **C11** | §5.6 Case 1 | "**T3 projected end-to-end; 15/15 FSV prior run measured**"; the 15/15 anchors a prior run, not the in-progress retrain |
| **C12a / C12b** split | Carried implicit in §5.4 guard spec + §5.6 Case-3 single operating point | Theorem 1 is not restated in §5 (it lives in agent-21 spine and §1 / §7); §5 anchors only one operating point (Case 3) |
| **V18** (frozen / frozen-with-LoRA) | §5.1, §5.2, §5.3, §5.4, §5.6 — every embedder reference | Always "frozen" or "frozen-with-LoRA"; never ambiguous "pretrained" |
| **V22** (measured vs projected) | **Every number in §5.6** | Bold = T1 measured with encoder-matched caveat; italic = T3 projected with pending-measurement flag |

### 2.3 Reserve-list compliance (§5)

Per agent-36's handoff, §5 must **not** re-cite items reserved by §2.
§5 satisfies this as follows:

- **Constellation guard predicate** (§5.4): no re-citation of
  `ouyang2022instructgpt`, `rafailov2023dpo`, `bai2022constitutional`
  for the structural contrast; that framing is done in §2.5 closing
  paragraph. §5.4 specifies the predicate without re-narrating the
  scalar-reward literature.
- **Multi-clip centroid** (§5.1 / §5.2): no re-citation of the TTS
  cluster (`wang2023valle` through `casanova2024xtts`); the
  "no multi-clip centroid in pool" framing was done in §2.5.
- **Runtime per-output geometric guard**: §5.3–§5.4 specify the
  mechanism without re-citing the retrieval or generation
  post-generation-evaluation clusters.
- **Permitted re-cites**: `lipman2023flowmatching` (§5.3 Case-2 loss
  derivation, load-bearing); `chen2022wavlm` (§5.6 Case-3 scorer
  identity); `hu2022lora` (§5.6 rank specifications for Cases 1–2);
  `rafailov2023dpo` (§5.3 Case-1 base loss, load-bearing). All four
  are pre-approved by agent-35 / agent-25 / agent-36.

### 2.4 Critical specification decisions (§5)

- **Double normalisation in Phase 1** (§5.2 step 3 then step 5): not
  tunable. Prevents centroid drift toward high-norm outliers. Made
  textually explicit: "The double normalisation (per-sample then mean)
  is not tunable".
- **Three invariants in Phase 2** (§5.3): embedders frozen, centroids
  frozen, LoRA restricted to the generator. Stated as "invariants
  checked every run". Audit hook: post-training re-run of Phase 1 must
  reproduce $c_m$ within $10^{-5}$.
- **Strict conjunction predicate** (§5.4 equation 5): the guard is an
  AND over modalities, not a weighted sum. The relaxed argmax variant
  is named separately; applying it does not convert the guard into an
  OR.
- **Same-session-band threshold calibration** (§5.4): thresholds are
  chosen **before** deployment. "Any subsequent adjustment of $\tau_m$
  to hit an evaluation number invalidates the guard." — sentence-level
  explicit.
- **Three-layer stack ordering** (§5.5): Layer 1 probabilistic, Layer
  2 per-token filter, Layer 3 per-output semantic. Stated as three
  complementary layers, not a strict hierarchy. The claim is that
  Layer 3 alone delivers per-output verifiability, while Layers 1 and
  2 shape the candidate distribution Layer 3 sees.

### 2.5 Case-3 measurement discipline (§5.6)

The Case-3 paragraph is the single highest-risk subsection for
adversarial review. The following writing decisions harden it:

- **Tier flag** (T1 measured) appears in the case header and in the
  bold measurement sentence.
- **Encoder-matched scope** stated in the measurement sentence
  itself, not only in §8 Limitations. Mandatory companion phrasing
  per agent-34 C5.
- **Measurement date** `2026-04-16` and **pipeline version hash**
  `qwen3-tts-foricl-v1` attached to the measurement.
- **DNSMOS / UTMOS companion** phrased as "descriptive equality
  within evaluator noise", not "statistically equivalent" (C6
  banned-phrase discipline).
- **Retrospective bootstrap CI** labelled as such: "retrospective,
  not power-based, descriptive summary of the ten observed values";
  fixed seed `20260416` for reproducibility.
- **Three pending measurements** named explicitly with their
  standing (instrument-ready; §8 carries the paste-ready
  disclosures).

## 3. Banned phrases avoided (§3 + §5)

Cross-checked against agent-34 §4 table. No occurrence of:

- "91 signals per input" (always "up to 91").
- "dimensionally distinct" (always "semantically distinct").
- "the collapse" unqualified.
- "0.961 mean SECS" bare (always with encoder-matched phrase).
- "indistinguishable from real speech" bare.
- "solves the data wall".
- "outperforms RLHF".
- "T=0.3 is optimal" without pipeline scope.
- "proven collapse-free".
- "mouth-weighted loss is necessary".
- "we demonstrate the sample-complexity collapse" without one-T1 qualifier.

## 4. What §3 and §5 hand to §6 (results writer)

§6 should inherit these specifications without re-deriving them:

- $C_P$ and the Phase-1 construction pipeline (§5.2).
- The loss $\mathcal{L}_{\mathrm{TCT}}$ (§5.3 equation 4) with
  $\lambda_m$ starting values per case.
- The guard predicate $G_\tau$ (§5.4 equation 5) and the same-session-
  band calibration (§5.4 threshold paragraph).
- The protocol instantiations (§5.6) including the Case-3 measurement
  table entry; §6 should quote the Case-3 headline number using the
  encoder-matched phrasing already established in §5.6.
- The three pending companion measurements (cross-encoder
  triangulation, pre-selection distribution, $n_m$ curve) as §8
  disclosure content.

## 5. What §3 and §5 hand to §7 (discussion writer)

- The three-role composition (§3.4) as a structural claim to
  synthesise in §7 with the per-output verifiability property
  (§5.4).
- The fourth-category taxonomy (§3.2) as a positioning claim in §7;
  §7 must keep "**we propose as a fourth taxonomic entry**" framing.
- The Case-3 headline with its scope — §7 must repeat the
  encoder-matched qualifier when cashing out the sample-complexity
  collapse.

## 6. What §3 and §5 hand to §8 (limitations writer)

§8 owns the paste-ready disclosures for:

- **W1 encoder-matched evaluation** → cross-encoder triangulation
  pending (ECAPA-TDNN / Resemblyzer on the same ten sentences).
- **W2 pre-selection cosine distribution** → the 120-candidate
  distribution is the instrument-ready pending measurement.
- **W3 single $n_m$ operating point** → $n_m$ curve at
  $\{10, 25, 50, 100\}$ is named RO-10 future work.
- **W5 approximate independence** (Axiom 2) → pairwise MI measurement
  of embedder redundancy is named RO-8 future work.
- **W7 no $M>1$ empirical** → Case 2 T1 measurement pending.

## 7. Confidence

**HIGH** that §3 and §5 match the spine (agent-21) and the
protocol specs (agent-25) without drift.

**HIGH** that all agent-34 C-series hedges carry exactly as
specified.

**HIGH** that the reserve list (agent-36 handoff) is respected; no
forbidden re-citations.

**HIGH** that the banned-phrase list (agent-34 §4) is clean.

**MEDIUM** on line-length copy-edit: §5.6 has two dense sentences
that a copy editor may split for paper flow without altering
content.

## Changelog

- 2026-04-17 — Initial issue by agent-37-methodology-writer after
  verifying that §3 and §5 prose files produced upstream already
  satisfy the task brief, the reserve list, and the hedge catalogue.
  No edits to section files required; this note documents the
  verification and the downstream-consumer contract.
