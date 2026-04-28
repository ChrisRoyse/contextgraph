---
namespace: patterns
agent_id: agent-33-falsification-agent
timestamp: 2026-04-17T07:55:00Z
status: active
locked: false
supersedes: none
tags: claim-softening, paste-ready, language-edits, recommendations, adversarial-hardening
depends_on:
  - memory/discoveries/agent-33-falsification-agent--counterarguments.md
  - memory/discoveries/agent-33-falsification-agent--falsification-experiments.md
  - memory/discoveries/agent-24-causal-simulator--weakness-register.md
  - memory/discoveries/agent-32-contradiction-analyzer--contradiction-register.md
downstream_consumers:
  - agent-34-confidence-quantifier
  - agent-37-methodology-writer
  - agent-39-discussion-writer
  - agent-42-adversarial-reviewer
---

# Claim-Softening Recommendations — Which Claims Must Be Softened (and How)

## 0. Purpose

This file translates the counterargument analysis (CA-1 … CA-6) into
**actionable language edits** for section writers. Each recommendation
specifies:

- **Target claim** (hypothesis / theorem / section).
- **Current wording risk** (what an unsoftened version reads like).
- **Required softening** (paste-ready substitute).
- **Triggering counterargument / weakness / contradiction**.
- **Severity of edit** (MUST / SHOULD / MAY).

Scope: no new primitives, no new citations (R3). Only language-level
and scope-level edits; no construct modifications. All of these are
**consistent with** and **subsumed by** the paste-ready resolution
inserts already drafted by agent-32.

---

## Aggregate verdict

Of the **6 load-bearing claims** examined (5 hypotheses + 1 overall-
paper framing):

- **2 MUST be softened** substantively in the body text. These are
  claims where the counterargument's response was judged
  INSUFFICIENT without a scope change: **H1 (signal count) and H3
  (sample-complexity collapse)**.
- **3 SHOULD receive scope tightening** (counterargument response is
  sufficient but only if the scope is made explicit):
  H2 (guard verifiability), H5 (fourth category), overall paper (CA-6).
- **1 MAY receive minor language work** (counterargument response is
  sufficient and scope is already well-framed): H4 (collapse silence).

---

## Recommendation 1 (MUST) — H1 / H1a: "91 signals/input" → "up to 91"

### Target claim

- **H1**: $\lvert D' \rvert = n \cdot (N + \binom{N}{2}) = 91$
  signals/input at $N=13$.
- **H1a**: Context Graph production yield = 91.
- **Manuscript anchors**: §1 headline (if cited); §3.4 DDA counting;
  §4.1 Context Graph specifics.

### Current wording risk

> "A fixed corpus projected through 13 frozen independent embedders
> yields **91 orthogonal labeled training signals per input**."

Reader interpretation: 91 is the actual effective count. This is
indefensible given Axiom 2 is V11-hedged and RO-8 is future work.
CA-1 lands directly on this phrasing.

### Required softening (paste-ready)

**§3.4 headline sentence (replace current):**

> "A fixed corpus projected through $N$ frozen, approximately-
> independent embedders yields **up to $N + \binom{N}{2}$ structured
> supervisory signals per input** — a **structural upper bound**
> under Axioms 1 and 2 (agent-21 P1). For Context Graph's production
> panel at $N = 13$, this is **up to 91 signals per input**; the
> effective count depends on pairwise mutual information across
> embedder outputs, which is currently V11-hedged and is **named
> future work** (RO-8). Qualitative evidence from production Context
> Graph — blind-spot cross-check patterns where sparse (E6 / E13),
> typo-robust (E9), and code-specialised (E7) embedders surface hits
> that semantic (E1) misses — establishes the effective count strictly
> exceeds $N$; the tight value within the $[N,\; N + \binom{N}{2}]$
> interval is pending empirical MI measurement."

**§4.1 Context Graph headline sentence:**

> "Under $N = 13$ embedders (E1 semantic, E2–E4 temporal, E5 causal
> asymmetric, E6 sparse keyword, E7 code, E8 graph, E9 HDC, E10
> paraphrase, E11 entity, E12 per-token late-interaction, E13 SPLADE),
> the per-input structured-signal count is **up to** $13 + 78 = 91$
> (P1 instantiated at $N = 13$). Blind-spot cross-check experience
> confirms the effective count is strictly greater than 13; the exact
> value is pending RO-8."

**§1 opener (if cited):**

> "Decomposition through $N$ frozen embedders yields up to
> $N + \binom{N}{2}$ structured supervisory signals per raw input
> (Theorem-of-Framework P1, §3.4; Context Graph at $N=13$ is up to
> **91**)."

### Severity

**MUST.** Without this edit, CA-1 is a one-paragraph review kill. The
softening is cheap (three words: "up to") and preserves the headline's
salience.

### Cross-references

- CA-1 (counterargument register).
- W5 (agent-24 HIGH).
- H1, H1a, H1b (agent-23).
- EXP-2 (falsification-experiments): if run, replaces "up to 91" with
  "$N_{\text{eff}} = X$".

---

## Recommendation 2 (MUST) — H3 / Theorem 1: separate constructive from empirical collapse

### Target claim

- **H3**: Sample-complexity $\mathcal{T}_{\mathrm{TCT}}(P) = O(M
  \cdot k)$ independent of $|D|$.
- **Theorem 1**: formal statement of the above.
- **Manuscript anchors**: §1 headline; §3.6 sample-complexity framing;
  §5.4 Theorem 1 statement; §6.3 Case 3; §7 Discussion; §9
  Conclusion.

### Current wording risk

> "Identity-locked generation **collapses** from thousands of subject-
> specific examples to 'one reference set per modality + frozen
> centroids' when TCT is applied on top of DDA-enriched data."

Reader interpretation: collapse is a measured empirical fact across
modalities. In reality the theorem is constructive; the empirical
anchor is one point at $M=1$. CA-3 lands directly.

### Required softening (paste-ready)

**§1 headline (replace current claim):**

> "Theorem 1 (§5.4): under TCT on DDA-enriched data, the subject-
> specific sample footprint satisfies $\mathcal{T}_{\mathrm{TCT}}(P)
> = O(M \cdot k)$ **by construction**, independent of the general
> corpus size $|D|$. The **empirical claim** that $k = O(10^2)$
> suffices to produce a stable centroid (Axiom 3) is anchored at
> **one T1 measured operating point** — Case 3 at $k = 50$, $M = 1$,
> English, single speaker, WavLM-family scorer — and T3-projected at
> other operating points (Case 2 $M > 1$)."

**§3.6 two-part framing sentence:**

> "We distinguish two readings of the sample-complexity collapse:
> **(a) constructive collapse** — the pipeline's subject-specific data
> footprint is $\leq M \cdot k$ by construction (Theorem 1); **(b)
> empirical collapse** — that $k = O(10^2)$ suffices for stable $c_m$
> (Axiom 3) is evidenced at one T1 operating point (Case 3 at $k = 50$,
> $M = 1$) and T3-projected at other operating points. We defend (a)
> unconditionally and (b) at one operating point with explicit
> extrapolation hedges."

**§5.4 attribution sentence (in addition to W6 remediation):**

> "TCT does not **cause** the sample-complexity collapse. TCT
> **operationalises** the collapse that is implicit in well-conditioned
> frozen embedders (Axiom 3; inherited from cluster-D retrieval and
> contrastive-geometry literature). The contribution is the **runtime
> predicate** against a frozen multi-modal $C_P$, not the discovery
> that frozen embedders are well-conditioned."

**§6.3 Case 3 lead sentence:**

> "Case 3 supplies **one T1 operating point** for the sample-
> complexity empirical claim: $n_m = 50$, $M = 1$ (voice modality),
> single English speaker, WavLM-base-plus-sv-family scorer, 10 held-
> out sentences. This is the **single anchor** for Axiom 3's
> $n_m \in [10, 10^2]$ interval; no $n_m$ sweep was run (W3); no
> multi-modal operating point was measured (W7)."

**§9 Conclusion sentence (must avoid):**

> AVOID: "TCT collapses sample complexity to $M \cdot k$."
>
> USE: "Under TCT on DDA-enriched data, **the subject-specific data
> footprint is bounded by $M \cdot k$ by construction; that $k$ can
> be taken $O(10^2)$ is an empirical claim with one T1 operating
> point and explicit future-work extensions (RO-10).**"

### Severity

**MUST.** Without this edit, CA-3 ("theorem from one data point") is
a near-guaranteed reviewer rejection vector. The softening is
not-cost-free (adds nuance) but preserves the theorem's contribution.

### Cross-references

- CA-3 (counterargument register).
- W3 (agent-24 HIGH), W6 (agent-24 MEDIUM), W7 (agent-24 HIGH).
- IC-2 (agent-32 HIGH).
- H3, H3a, H3b (agent-23).
- Theorem 1 (agent-21).
- EXP-3 (falsification-experiments): if run, converts "one point" to
  "a curve".

---

## Recommendation 3 (SHOULD) — H2 / Theorem 2: scope the guard's novelty to composition, not primitive

### Target claim

- **H2**: Constellation guard accept-rate bounded by conjunction.
- **Theorem 2**: Runtime computability in $O(M \cdot d_{\max})$.
- **Manuscript anchors**: §1 opener; §5.1 scalar-vs-geometric
  contrast; §5.3 guard definition; §5.4 per-output predicate.

### Current wording risk

> "TCT's guard is **novel** because it is a per-output verifiable
> predicate computable at inference."

Reader interpretation: the cosine-threshold primitive itself is
novel. CA-2 ("this is classical speaker verification") lands directly,
especially at $M = 1$ (Case 3) where the guard reduces to literal
speaker verification.

### Required softening (paste-ready)

**§5.1 opening paragraph:**

> "Per-output thresholded similarity against a reference (1-NN
> verification) is classical (speaker verification, face verification,
> Siamese networks). The **novelty** of the TCT guard is not the
> primitive; it is the **composition** of three properties over the
> **same frozen artefacts**:
>   (i) **multi-modal conjunction** — $G_\tau = \mathbb{1}[\forall m:
>   s_m \geq \tau_m]$ over $|\mathcal{M}| \geq 2$ independent modalities;
>   (ii) **frozen multi-modal target** — the constellation $C_P$ is a
>   per-subject frozen set of reference means, not a learned classifier
>   over identities;
>   (iii) **per-output runtime predicate** — invoked on every
>   generated candidate at inference time, producing accept/reject
>   with a deterministic cosine trace as provenance.
> No cited method in the LOCKED 88-pool composes these three properties
> (R3 pool-relative; MP-11 role-flip; MP-16 pool-relative scope)."

**§5.3 guard definition footnote:**

> "**Footnote on novelty scope.** The guard's operation at $M = 1$
> reduces to classical speaker / face / Siamese verification; at
> $M \geq 2$ it is a conjunctive multi-modal predicate. Case 3's
> measured instantiation is at $M = 1$; Case 2 (T3-projected) is at
> $M \geq 2$; the multi-modal strictness claim (H2b) is derivational
> under Axiom 2, with $M > 1$ empirical measurement pending (W7)."

**§5.4 paradigm-scope sentence:**

> "**We do not claim TCT outperforms RLHF / DPO / CAI on RLHF
> benchmarks.** TCT addresses a different problem class — identity-
> locked generation with per-output verifiability — that scalar-
> reward alignment does not target (EC-9). The structural break is
> in aim / instrument / metric / runtime posture, not in performance
> on a shared benchmark."

### Severity

**SHOULD.** The claim is defensible as stated if the composition-
scope is explicit. Without explicit scoping, CA-2 remains a reviewer
bait. The softening is mostly already drafted by agent-32 (EC-9
resolution, IC-3 resolution).

### Cross-references

- CA-2 (counterargument register).
- H2, H2b (agent-23).
- P3, Theorem 2 (agent-21).
- IC-3 CRITICAL (agent-32).
- EC-9 (agent-32): RLHF / DPO / CAI different problem class.
- W7 (agent-24): $M > 1$ empirical gap.
- EXP-1 (falsification-experiments): triangulates at $M = 1$.

---

## Recommendation 4 (SHOULD) — H5: "dimensionally distinct" → "semantically distinct at a different pipeline stage"

### Target claim

- **H5**: Meaning compression is a distinct fourth compression
  category.
- **Manuscript anchors**: §3.1 meaning-compression ratio view; §5.2
  relation to existing frameworks; §7 Discussion.

### Current wording risk

> "Meaning compression is **dimensionally distinct** from bit-,
> weight-, and activation-compression."

Reader interpretation: the four unit pairs are formally non-
reducible. Dimensional analysis of bit/bit vs. signal/raw-input gives
the same count/count form; W8 owns this overclaim.

### Required softening (paste-ready)

**§3.1 unit-analysis table (add W8-fixed row headers):**

> "We distinguish four compression categories by **pipeline stage**
> and **counted object**, not by dimensional non-reducibility:
>
> | Category | Pipeline stage | Counted object | Denominator |
> |---|---|---|---|
> | Bit compression | Inference / storage | Output bits | Input bits |
> | Weight compression | Model storage / serving | Accuracy | Parameters |
> | Activation compression | Serving | Memory | Output quality |
> | **Meaning compression** | **Dataset preparation** | **Structured supervisory signals** | **Raw data unit** |
>
> The four trade along different axes. Meaning compression is
> **semantically distinct** — it counts structured supervisory signals
> at dataset-preparation time rather than Shannon bits at inference —
> though its unit shares the count/count form with bit compression.
> It is a **fourth accounting axis** in the compression-as-intelligence
> frame (EXT of cluster-C compression theory; agent-10 CAT-EXT)."

**§5.2 Delétang / Huang reconciliation:**

> "Delétang et al. (2024) and Huang et al. (2024) are **not
> contradicted** by H5. Their compression framing reads bit-
> compression at inference on trained models; our meaning-compression
> framing reads signal-density at dataset preparation via a frozen
> embedder panel. The two are complementary axes of the same
> information-theoretic tradition (MDL / rate-distortion, ISO-04).
> Huang's $r \approx 0.95$ is a **within-model** capability-compression
> correlation; V11's approximate-independence hedge is an **across-
> model** panel-diversity assumption. The two operate on orthogonal
> axes (agent-17 tension 2 resolved)."

### Severity

**SHOULD.** W8 already owns the language-tightening. EC-7 already
owns the Delétang / Huang reconciliation. This recommendation
consolidates the two.

### Cross-references

- CA-5 (counterargument register).
- W8 (agent-24 MEDIUM).
- EC-7 (agent-32 MEDIUM).
- H5 (agent-23).
- P1 ratio view, §5.2 (agent-21).
- ISO-04 (agent-20).

---

## Recommendation 5 (SHOULD) — Overall paper: commercial-framing scoping

### Target claim

- Overall paper's relation to Teleox.ai / Leapable.ai commercial
  context.
- **Manuscript anchors**: §1 opener; §4 systems; §Competing
  Interests; §Acknowledgements; §9 Conclusion.

### Current wording risk

Any §4 language that reads as a product description ("our platform",
"our enterprise solution", "industry-leading multi-modal retrieval"
etc.) triggers FM-01 (marketing-creep) and a summary-rejection.
CA-6 rides any such language.

### Required softening (paste-ready)

**§4 framing sentence (replace any product-style description):**

> "Context Graph is an open-source Rust research system (~370K lines,
> 13 embedders, 59 RocksDB column families, 75 MCP tools, 5,184
> tests) developed at Teleox.ai. Source code is publicly available;
> §4 describes the embedding panel, storage, and retrieval paths
> relevant to DDA's instantiation. ClipCannon is an open-source
> Python research pipeline (~67,585 lines, 23-stage DAG) producing
> 12,000+ labeled training samples from 16 min source video; it
> instantiates DDA's labelling role for multi-modal case studies.
> Leapable.ai is referenced as a companion marketplace; it is
> **not** a core contribution of this paper and is described at
> sidebar length only (§4.3)."

**§9 Conclusion closing paragraph (replace any commercial framing):**

> "The open problems are the empirical extensions: RO-8 (formal
> pairwise MI measurement across the $N$-embedder panel), RO-10
> (sample-complexity curve per modality), EXP-1 (cross-encoder SECS
> triangulation on Case 3), and the $M > 1$ multi-modal strictness
> evidence at Case 2. We invite replication and extension."

**§Competing Interests (agent-31 paste-ready; enforce):**

Page-1 footnote + full §CI section disclose: author affiliation
(Teleox.ai primary; Leapable.ai companion); equity / compensation;
the role of commercial entities as employers / funders (§Acknowledgements
only).

**FM-01 regex pass (agent-42 gate):** zero matches on §1–§9 body text
for marketing keywords. Regex list is documented in agent-01 FM-01
paste-ready material.

### Severity

**SHOULD** — conditional on FM-01 regex pass at agent-42 gate.
**Escalates to MUST** if the regex finds matches, in which case the
matched prose requires rewriting.

### Cross-references

- CA-6 (counterargument register).
- FM-01 (agent-01 failure mode).
- EC-11 (agent-32 LOW / CRITICAL conditional).
- §CI, §Ethics, §Acknowledgements (agent-31).
- §4 (Context Graph + ClipCannon framing).

---

## Recommendation 6 (MAY) — H4: scope tightening to training-time recursion

### Target claim

- **H4**: DDA is structurally not subject to Shumailov-2024 recursion.
- **Manuscript anchors**: §3.3 DDA scope argument; §8 L-05; §10
  Future Work.

### Current wording risk

> "DDA is **exempt from model collapse**."

Reader interpretation: DDA has a general immunity property. In fact
DDA is collapse-silent (preconditions of specific theorems unsatisfied),
not immune. CA-4 rides "best-of-$K$ is recursion" reading.

### Required softening (paste-ready)

**§3.3 scope sentence:**

> "DDA is **collapse-silent** under the Shumailov-2024 / Dohmatob-
> 2024 / Alemohammad-2023 theorems: these are theorems about
> **training recursion** (generator-trained-on-previous-generation-
> samples), and DDA's signals depend only on the real input $x$ and
> frozen $\phi_m$ — the recursion precondition is not satisfied and
> the theorems are **silent** on this configuration. This is a **scope
> argument**, not a new independence-from-collapse theorem (agent-11
> CL-06 MODERATE-categorical). Best-of-$K$ selection (TCT Phase 3)
> is an **inference-time** operation on a frozen generator and does
> not constitute training recursion (IC-5)."

**§8 L-05 (embedder-mediated curation scope):**

> "The collapse-silent claim addresses **generator-in-loop training
> recursion** only. Other forms of data-selection feedback (e.g.,
> embedder-mediated filtering of $D$ at ingestion) are **out of scope
> of the specific cited theorems** and are not defended by this scope
> argument. Whether embedder-mediated curation induces any form of
> distributional drift is an empirical question; RO-? (named in §10)
> specifies a toy falsification protocol."

**§10 Future Work item:**

> "RO-? — Empirical toy demonstration of DDA-enriched training vs.
> recursive self-distillation: train a generator under (A) DDA-
> enriched supervision over frozen $D$ vs. (B) recursive self-
> distillation; report distributional-drift dynamics across rounds;
> falsify H4 if (A) exhibits Shumailov-style drift."

### Severity

**MAY.** Agent-32 IC-5 + EC-8 + agent-24 W9 already own most of this
scope work. The recommendation here is to enforce consistency across
§3.3 / §8 / §10 and to make the scope boundary explicit.

### Cross-references

- CA-4 (counterargument register).
- IC-5, EC-8 (agent-32).
- W9 (agent-24 MEDIUM).
- P2, H4 (agent-21, -23).
- CL-06 (agent-11).

---

## Summary table

| Rec | Target | Severity | Key phrase shift | Paste-ready drafted? |
|---|---|---|---|---|
| **1** | H1 / H1a signal count | **MUST** | "91 signals" → "up to 91 signals" | **YES** (§3.4, §4.1, §1) |
| **2** | H3 / Theorem 1 sample-complexity | **MUST** | "collapses" → "constructive bound + one empirical T1 point" | **YES** (§1, §3.6, §5.4, §6.3, §9) |
| **3** | H2 / Theorem 2 guard novelty | **SHOULD** | "novel predicate" → "novel composition (multi-modal + frozen + runtime)" | **YES** (§5.1, §5.3, §5.4) |
| **4** | H5 fourth category | **SHOULD** | "dimensionally distinct" → "semantically distinct at different stage" | **YES** (§3.1, §5.2) |
| **5** | Overall commercial framing | **SHOULD** (→MUST if regex fails) | research-register body + §CI disclosure | **YES** (§4, §9, §CI) |
| **6** | H4 collapse scope | **MAY** | "exempt" → "collapse-silent; scope = training recursion only" | **YES** (§3.3, §8 L-05, §10) |

## Aggregate impact on manuscript

After applying all six recommendations:

- **Headline numbers preserved**: "up to 91"; "0.961 mean SECS (with
  encoder-matched caveat per W1)"; "$O(M \cdot k)$ constructive
  bound".
- **Headlines lost** (none that were defensible to begin with):
  - "TCT causes sample-complexity collapse" → "TCT operationalises
    the collapse implicit in well-conditioned frozen embedders".
  - "Dimensionally distinct" → "semantically distinct at different
    stage".
  - "Novel guard predicate" → "novel composition of three properties".
- **Headlines gained** (from pre-registered experiments): if EXP-1,
  EXP-2, EXP-3 run, several softened claims can be re-sharpened
  empirically (e.g., "$N_{\text{eff}} = X$" replacing "up to 91").

## Priority

1. **Recommendations 1 and 2** are required for publication-grade
   integrity. Apply unconditionally.
2. **Recommendations 3, 4, 6** enforce scope discipline already
   planned upstream; apply for consistency.
3. **Recommendation 5** is the submission-gating check; FM-01 regex
   pass at agent-42 determines whether language edits are required.

## Changelog

- 2026-04-17 — Initial issue by agent-33-falsification-agent.
