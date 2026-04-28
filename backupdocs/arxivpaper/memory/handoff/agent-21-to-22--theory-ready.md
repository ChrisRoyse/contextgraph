---
namespace: handoff
agent_id: agent-21-theory-builder
timestamp: 2026-04-17T05:10:00Z
status: active
tags: handoff, agent-22, model-architect, spine-ready, axioms, propositions, theorems
---

# Handoff: Agent-21 (theory-builder) → Agent-22 (model-architect)

## Read these files FIRST, in this order

1. `./memory/decisions/agent-21-theory-builder--theoretical-framework.md`
   — the **spine**: $C_P$, three axioms, five propositions, two theorems,
   relation-to-existing-frameworks, hedges, cross-references.
2. `./memory/patterns/agent-21-theory-builder--theory-for-writers.md`
   — **paste-ready §3-scaffold** for the methodology-writer (agent-37);
   every block is structured so that (a) the spine is named first,
   (b) axioms are introduced before propositions, (c) propositions
   before theorems, (d) hedges carry verbatim.
3. `./memory/reports/agent-21-theory-builder--final-report.md`
   — SMAP for this run.
4. `./memory/patterns/agent-20-interdisciplinary-pollinator--isomorphisms.md`
   — 8-ISO ledger (5 body-named, 3 metaphor-only); §5.4 of the spine
   inherits.
5. `./memory/patterns/agent-19-opportunity-identifier--research-opportunities.md`
   — 10-RO ledger; P3 / Theorem 2 derive from RO-4; Theorem 1 derives
   from RO-6 + RO-10; §5.1 / §5.2 of the spine inherit RO-1 / RO-2 / RO-3.

## What agent-21 did

1. Read the five required agent-04 / agent-17 / agent-19 / agent-20
   context files.
2. **Named the single paper-level object**: the constellation
   $C_P = \{(m, c_m) : m \in \mathcal{M}\}$. Explained why it is the
   spine (every other contribution is defined in relation to it).
3. **Issued three axioms**:
   - **Axiom 1 (Grounded measurement)** — frozen embedders produce
     grounded measurements of real inputs.
   - **Axiom 2 (Approximate orthogonality)** — N approximately-
     independent frozen embedders produce approximately orthogonal
     information axes (V11-hedged).
   - **Axiom 3 (Reference-set learnability)** — centroids are
     learnable from $O(10^2)$-scale reference sets without
     overfitting.
4. **Derived five propositions**:
   - **P1** Signal-count identity: $|D'| = n(N + \binom{N}{2})$.
   - **P2** Collapse-silence: DDA's recursion violates the
     Shumailov-2024 generator-in-loop precondition.
   - **P3** Geometric-over-scalar verifiability: the guard gives
     per-output verification that scalar-reward cannot.
   - **P4** Role-composition coherence: same frozen $\Phi$ serves
     retriever / labeller / guard without retraining.
   - **P5** Sample-complexity collapse:
     $\mathcal{T}_{\mathrm{TCT}}(P) = O(M \cdot k)$.
5. **Stated two informal theorems**:
   - **Theorem 1 (Sample-Complexity Collapse under TCT, lead)**:
     $\mathcal{T}_{\mathrm{TCT}}(P) = O(M \cdot k)$, independent of
     $|D|$ (corpus size) and independent of any subject-specific
     fine-tuning data beyond the reference sets. Proved
     constructively from the three-phase protocol.
   - **Theorem 2 (Runtime Verifiability of Identity Lock)**:
     $G_\tau(\hat y; C_P)$ is computable per-output in $O(M \cdot
     d_{\max})$ with no auxiliary learned model. Proved from the
     definition of the guard and the frozen-object-trio ($\phi_m$,
     $c_m$, $\tau_m$).
6. **Positioned the spine vs. existing frameworks**:
   - **EXT** of Chinchilla scaling — axis extension (supervisory-
     signal density as a new axis alongside bit-compressed token
     counts). Does not contradict Kaplan / Hoffmann.
   - **EXT** of compression-as-intelligence — fourth taxonomic
     category alongside bit-, weight-, and activation-compression.
     Unit-analytic orthogonality, not mathematical equivalence.
   - **BRK** of scalar-reward alignment — four-axis structural break
     (aim / instrument / metric / runtime-verification). We do not
     compete with RLHF on RLHF benchmarks; we offer per-output
     geometric verifiability RLHF cannot.
   - **Imports** from five body-named ISOs (RV, sensor fusion, MDL,
     astronomy, SPC) as tradition-families; no citations added.
7. **Produced a paste-ready §3-scaffold** for agent-37 (methodology-
   writer) with 13 paragraph blocks, word-count targets, Figure 1
   specification, Algorithm 1 reference, and a 10-item writer
   checklist.

## Headline findings for agent-22 (model-architect)

### The spine is a single-object theory, not a multi-object theory

The theory is intentionally **minimalist**: one object ($C_P$), three
axioms, five propositions, two theorems. Every theorem in the paper
reduces to a property of $C_P$ or of the three-role composition
around it (P4). This is why the paper can stay at 20–30 pages without
losing theoretical weight.

### The theorem to lead with is Theorem 1 (Sample-Complexity Collapse)

Theorem 1 is the sharpest, most cash-out-able result:
$\mathcal{T}_{\mathrm{TCT}}(P) = O(M \cdot k)$, independent of
$|D|$. It converts "fewer samples worked in Case 3" into "the method's
subject-specific sample complexity is independent of corpus size". This
is the empirical statement of Mission-Brief Claim #4
(sample-complexity collapse), and the natural closing move in §1, §5,
and §9.

### Theorem 2 is the load-bearing theoretical-novelty theorem

Theorem 2 formalises P3: $G_\tau(\hat y; C_P)$ is computable at
inference with no auxiliary learned model, in $O(M \cdot d_{\max})$,
with a per-output cosine trace. Agent-16 names MP-09 (per-output
runtime verification gap) the single most theoretically novel move in
the manuscript. Theorem 2 is its crisp statement.

### The strongest proposition is P3 (Geometric-over-scalar verifiability)

P3 delivers the four-axis structural break with scalar-reward
alignment. It is what enables the §2.6 contrast table, the §5.1
theoretical framing, and the §7 paradigm-import discussion. Without
P3 the TCT definition is an alignment-technique variant; with P3 it
is a structural complement to the scalar-reward paradigm.

### The role-composition (P4) is the simplicity claim

P4 states that the same frozen $\Phi$ serves three semantic roles
(retriever in Context Graph; labeller in DDA; guard in TCT). This is
Theme B's architectural thesis and the reason the paper's four
contributions are one contribution at the compositional level. P4 is
what the §3 Framework opener should lead with.

### Composition-not-invention is preserved at every level

No new theoretical primitives are introduced. Every axiom, proposition,
theorem is built from objects already locked by agent-04. The novelty
is in the single-object spine, not in any new atom.

## Register rules inherited (carry forward)

- **R1 composition-not-invention (MP-12).** Every axiom / proposition
  / theorem is built from locked primitives; no new atom is claimed.
- **R2 open-scorer / disclosed-forensic.** Case 3's cross-encoder SECS
  hygiene (RO-5) is the operational move that makes Theorem 2's
  "no auxiliary learned model" claim defensible.
- **R3 pool-relative scope (MP-16).** "No cited method" clauses in P3,
  P4, Theorem 2 are bounded by the LOCKED 88-citation pool; §8
  Limitations carries the required paragraph verbatim.
- **R4 projected-vs-measured (V22).** Theorem 1's $k$ is T1-anchored
  at Case 3 only ($n_m = 50$, mean SECS 0.961); other modalities and
  subjects are T3-projected.

## Quality-gate status delivered

- **QG (axiom count).** 3 axioms (spec: 3). **PASS.**
- **QG (proposition count).** 5 propositions (spec: 5). **PASS.**
- **QG (theorem count).** 2 theorems (spec: 2). **PASS.**
- **QG (informal-not-pretentious).** Every theorem is stated in
  prose-plus-equation, with proof sketch, confidence, and hedges.
  **PASS.**
- **QG (spine single-object).** $C_P$ is the single paper-level
  object; every contribution is defined in relation to it.
  **PASS.**
- **QG (relation-to-existing-frameworks).** EXT of Chinchilla,
  EXT (CAT-EXT) of compression-as-intelligence, BRK of scalar-reward
  alignment, IMP of five body-named ISOs. **PASS.**
- **QG (composition-not-invention).** No new primitives introduced.
  **PASS.**
- **QG (hedges carried verbatim).** V11, V18, V22, MP-12, MP-16,
  R3, R4 all carry. **PASS.**
- **QG (no new citations).** LOCKED 88-pool preserved. **PASS.**
- **QG (novelty-code preservation).** Agent-10 codes inherited from
  RO-1 / RO-2 / RO-3 / RO-4; no upgrade. **PASS.**

## Critical constraints carrying forward (locked artefacts)

- Master pool LOCKED (agent-08) — 88 citations stable.
- Cluster labels A–I LOCKED (agent-06).
- Evidence grades LOCKED (agent-11).
- Quality flags LOCKED (agent-13).
- Bias mitigations SOFT-LOCKED (agent-14).
- Six themes + narrative arc SOFT-LOCKED (agent-17).
- Gap ledger SOFT-LOCKED (agent-18).
- Opportunity ledger SOFT-LOCKED (agent-19).
- ISO ledger SOFT-LOCKED (agent-20).
- **Theoretical framework SOFT-LOCKED (agent-21).** New axioms /
  propositions / theorems introduced downstream become Axiom 4+ /
  P6+ / Theorem 3+.

## Open items forwarded to agent-22 (model-architect)

1. **Figure 1 construction.** Agent-21 has specified a single unified
   diagram showing $C_P$ at the centre, with DDA feeding the
   centroids, TCT's Phase-2 loss pointing at $C_P$, TCT's Phase-3
   guard reading from $C_P$, and identity-locked generation as the
   output. Agent-22 should produce this figure to arXiv-publication
   quality (LaTeX TikZ or equivalent). The caption is paste-ready in
   `agent-21-theory-builder--theory-for-writers.md` under "Figure 1
   specification".

2. **Architecture diagram cross-walk for §4 Systems.** The spine is
   operationalised in two production systems (Context Graph,
   ClipCannon). Agent-22 should verify the architecture diagrams
   reflect the role-composition (P4): Context Graph's 13-embedder
   panel serves as retriever in §4.1 and as labeller-substrate in
   §3.4 (same frozen instances); ClipCannon's 7-modality pipeline
   feeds DDA in §4.2 and feeds Case-2's constellation construction
   in §6.2.

3. **Algorithm 1 pseudocode formatting.** Use the pseudocode block
   from `tct-definition.md` §3.2.1 verbatim. No Python / no Rust
   (FM-06). Three phases, commented headers, no language-specific
   syntax. Agent-22 should ensure it renders cleanly alongside
   Figure 1.

4. **Consistency check on notation.** The spine uses $C_P$ (subject-
   indexed constellation), $c_m$ (per-modality centroid), $\phi_m$
   (per-modality frozen embedder), $\tau_m$ (per-modality threshold),
   $G_\tau$ (guard predicate), $\mathcal{M}$ (modality set), $R_m$
   (reference set), $\lambda_m$ (Phase-2 loss weight), $M$ (modality
   cardinality), $N$ (embedder-panel cardinality — distinct from $M$
   because $N$ counts DDA panel embedders while $M$ counts TCT
   modalities; in Context Graph they coincide at the whole-panel
   level but not always at the TCT-constellation level). Agent-22
   should ensure Figure 1 respects this distinction.

5. **Axiom 3 visual callout.** Axiom 3 ($k = O(10^2)$) has one T1
   anchor (Case 3, $n_m = 50$). Agent-22's §6.3 architecture diagram
   should surface this sample-size fact next to the constellation
   block — it is the empirical grounding that makes Theorem 1
   non-trivial.

6. **Per-output provenance trace.** Theorem 2's per-output trace
   $\{s_m(\hat y)\}$ should be shown in the architecture flow (§5.3
   or §6.3) as a deterministic record attached to each accepted
   $\hat y$. This is an engineering-visual of the "no auxiliary
   learned model" property.

7. **Paradigm-import dual attribution in §5.3.** Agent-22's §5.3
   architecture diagram should label the guard block with **both**
   ISO-01 (runtime verification) and ISO-07 (statistical process
   control) as paradigm sources. Single-source attribution weakens
   the borrow; dual attribution strengthens it (agent-20 note).

## Dependencies agent-22 will pull forward

- **From agent-21 (this handoff):** spine + axioms + propositions +
  theorems + §3-scaffold.
- **From agent-20:** 8-ISO ledger + framing-move prose seeds.
- **From agent-19:** 10-RO ledger (RO-1 / RO-2 / RO-3 / RO-4 feed
  the spine theorems).
- **From agent-18:** 31-gap ledger for absence claims.
- **From agent-17:** six themes + narrative arc.
- **From agent-16:** meta-patterns (MP-09, MP-11, MP-12 are
  load-bearing for Theorem 2 and P4).
- **From agent-04:** four locked construct definitions; the spine
  quotes them directly without rederivation.
- **From agent-02:** canonical glossary.
- **From agent-01:** vocabulary rules (V11, V18, V22).

## One-paragraph summary for agent-22

> The paper has a single theoretical object: the frozen multi-modal
> centroid set $C_P$. Three axioms underpin it (grounded measurement
> of real inputs by frozen embedders; approximately orthogonal
> information axes across the $N$-embedder panel; learnability of
> centroids from reference sets of size $\sim 10^2$). Five
> propositions derive from the axioms (DDA's signal count; DDA's
> collapse-silence; the guard's geometric-over-scalar verifiability;
> the three-role composition of the same frozen $\Phi$; sample-
> complexity collapse). Two informal theorems lock the spine: Theorem
> 1 bounds subject-specific sample complexity at $O(M \cdot k)$
> independent of corpus size; Theorem 2 makes the per-output guard
> computable in $O(M \cdot d_{\max})$ with no auxiliary learned
> model. The theory EXTends Chinchilla scaling and compression-as-
> intelligence; it structurally BReaKs with scalar-reward alignment;
> it imports from RV, sensor fusion, MDL, astronomy, and SPC as
> paradigmatic frame borrows. Composition-not-invention is preserved
> throughout; no new primitives are introduced. Agent-22's job is to
> give the spine its visual form in Figure 1 and ensure §4 / §5 /
> §6 architecture diagrams respect the role-composition.

## Confidence

**HIGH.** Every axiom / proposition / theorem is derived from locked
upstream constructs with explicit hedges and boundary conditions. The
spine is testable (constructive proofs; Case-3 T1 anchor), falsifiable
(pool-external counter-examples would weaken composition-level
claims), and narrow enough to defend under adversarial review. No new
citations. No new primitives. Register rules R1 / R3 / R4 preserved
verbatim.

## Changelog

- 2026-04-17 — Initial handoff by agent-21-theory-builder.
