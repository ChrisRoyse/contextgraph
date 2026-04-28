---
namespace: decisions
agent_id: agent-21-theory-builder
timestamp: 2026-04-17T05:00:00Z
status: active
locked: false
supersedes: none
tags: theoretical-framework, spine, axioms, propositions, theorems, constellation-as-spine, composition-not-invention
depends_on:
  - memory/context/mission-brief.md
  - memory/decisions/agent-04-construct-definer--dda-definition.md
  - memory/decisions/agent-04-construct-definer--meaning-compression-definition.md
  - memory/decisions/agent-04-construct-definer--tct-definition.md
  - memory/decisions/agent-04-construct-definer--identity-locked-definition.md
  - memory/patterns/agent-17-thematic-synthesizer--thematic-frameworks.md
  - memory/patterns/agent-17-thematic-synthesizer--narrative-arc.md
  - memory/patterns/agent-19-opportunity-identifier--research-opportunities.md
  - memory/patterns/agent-20-interdisciplinary-pollinator--isomorphisms.md
  - memory/handoff/agent-20-to-21--isomorphisms-ready.md
downstream_consumers:
  - agent-22-model-architect
  - agent-37-methodology-writer
  - agent-39-discussion-writer
  - agent-42-adversarial-reviewer
answers_questions: [Q04, Q05, Q06, Q07, Q15]
---

# Theoretical Framework — The Constellation as Spine

## 0. Purpose and scope

This file issues the **theoretical spine** of the manuscript. It does three
things and only these three things:

1. Names the **single paper-level object** around which every other
   contribution is defined: the **constellation** $C_P = \{(m, c_m) : m \in
   \mathcal{M}\}$ (from `tct-definition.md` §3.2 Phase 1).
2. States **three axioms** on which the theory rests and **five
   propositions** derived from them.
3. States **two theorems** (informal; the paper is not a pure theory paper)
   that make the spine actionable, plus one relation-to-existing-frameworks
   section.

The framework is **composition-not-invention** (MP-12, agent-17 Register R1).
No new primitives. Each axiom, proposition, and theorem is built from
objects already defined and locked by agent-04. What is new is how they
compose into a single coherent spine.

The spine is the theory §3 + §5 of the manuscript will **teach**. Every
section writer pulls the spine from here; every adversarial-review exposure
trace (agent-42) lands on one of the axioms or propositions below.

---

## 1. The central theoretical object — why $C_P$ is the spine

### 1.1 The object

The **constellation** for a subject $P$ is the set of per-modality,
L2-normalised, frozen centroids:

$$
C_P \;=\; \bigl\{(m, c_m) : m \in \mathcal{M}\bigr\}, \qquad c_m \;=\; \mathrm{normalize}\Bigl(\frac{1}{n_m}\sum_{i=1}^{n_m} \mathrm{normalize}(\phi_m(r_{m,i}))\Bigr).
$$

Cardinality $|C_P| = |\mathcal{M}|$. Each $c_m$ is a unit vector in
$\mathbb{R}^{d_m}$ (or the appropriate modality-specific output space).
$C_P$ is constructed in Phase 1 of TCT and held constant through Phases 2
and 3 (`tct-definition.md` §3.2).

### 1.2 Why $C_P$ is the spine

Every other load-bearing contribution in the paper is **defined in
relation to $C_P$**. This is not rhetorical convenience; it is the
structural property that makes the four claims of the mission brief a
single theoretical contribution rather than four adjacent ones.

| Contribution | Relation to $C_P$ |
|---|---|
| **DDA** (Claim 1) | DDA produces the **training-data vectors** from which the centroids $c_m \in C_P$ are computed. Without DDA's frozen-embedder decomposition there is no coherent notion of a per-modality centroid. |
| **Meaning compression** (Claim 2) | Meaning compression is the **ratio view** of DDA (`meaning-compression-definition.md` §2.2). The numerator $\mathrm{signals}(D, \Phi)$ includes the per-embedder projections $\phi_m(x)$ that go into constructing each $c_m$. |
| **TCT** (Claim 3) | TCT's Phase-1 **output** is $C_P$. Phase 2's loss targets it: $\mathcal{L}_{\mathrm{TCT}} = \mathcal{L}_{\mathrm{gen}} + \sum_m \lambda_m (1 - \cos(\phi_m(\hat y), c_m))$. Phase 3's guard verifies proximity to it: $\forall m : \cos(\phi_m(\hat y), c_m) \geq \tau_m$. |
| **Identity-locked generation** (Claim 4) | The per-output predicate **is** a predicate over $C_P$ (`identity-locked-definition.md` §4.2). Sample-complexity collapse reduces subject-sample requirements to "one reference set per modality + the frozen $C_P$". |

The spine organisation has three **operational consequences** that justify
naming $C_P$ the single object:

1. **DDA produces the centroids.** The per-embedder projections
   $\phi_m(x)$ counted in the $n \cdot N$ term of DDA's count are exactly
   the quantities aggregated (over a reference set $R_m$) into each
   $c_m$. DDA without $C_P$ is a measurement ritual with no generative
   target; $C_P$ without DDA is a set of centroids unsupported by a
   counting theory.
2. **Meaning compression measures the centroids' substrate.** The
   $N + \binom{N}{2}$ ratio is the supervisory density that constructs,
   in aggregate, the information available to form each $c_m$.
   Meaning compression is the rate at which the raw corpus loads
   informational weight onto each modality's centroid.
3. **TCT uses $C_P$ as loss target (Phase 2) and guard (Phase 3).** The
   same frozen object appears in the training objective and in the
   inference-time acceptance predicate. That "same object in two roles"
   property is the structural source of per-output verifiability.

A reader who retains a single mental object from the paper should retain
$C_P$. Every axiom below asserts a property of $C_P$; every proposition
derives a behaviour of the pipeline around $C_P$; every theorem lowers
the complexity of producing or validating generation against $C_P$.

---

## 2. Three axioms

The axioms are minimal. They are the assumptions without which the rest
of the theory does not close. Each axiom is paired with the hedge /
limitation / boundary condition that governs its use in the manuscript.

### Axiom 1 (Grounded measurement) — **frozen embedders produce grounded measurements of real inputs**

Let $\phi : \mathcal{X} \to \mathbb{R}^{d}$ be a frozen embedder
(`dda-definition.md` §1.2.1; glossary #10). For any real input $x \in
\mathcal{X}$, the embedder produces a deterministic, reproducible
measurement $\phi(x)$ that is a function of $x$ and the embedder's fixed
parameters only. The measurement is **grounded** in the sense that its
value is determined by the real input, not by the generative model being
trained, and does not drift as the generator's parameters change.

**Formally.** For a generator $g_\theta$ under training and a real input
$x$, $\partial \phi(x) / \partial \theta = 0$ because $\phi$ is frozen
and $x$ is generator-independent. For a generated candidate
$\hat y = g_\theta(z)$, $\phi(\hat y)$ is a function of $\hat y$ and of
the frozen $\phi$, so the measurement changes only through $\hat y$ (i.e.,
through the generator's output distribution), not through any hidden
coupling between the embedder and the generator.

**Hedge / boundary condition.** Axiom 1 holds only while the embedder
remains frozen (V18 "frozen" or "frozen-with-LoRA"). Any training regime
that updates $\phi$ violates the axiom and collapses downstream results
to self-distillation (glossary #6 / #10). The V11 independence-hedge is
**independent** of Axiom 1: Axiom 1 is about a single frozen embedder;
V11 is about whether $N$ frozen embedders are approximately independent
(see Axiom 2).

**Consequence in the paper.** Axiom 1 is what enables §3.3's
"collapse-silent" argument: DDA's signals do not participate in the
generator-in-loop feedback that Shumailov et al. (2024) require for
model-collapse theorems to apply. The measurement instrument is outside
the generator's training loop.

### Axiom 2 (Approximate orthogonality) — **orthogonal embedders produce orthogonal-ish information axes**

Let $\Phi = \{\phi_1, \ldots, \phi_N\}$ be a panel of frozen embedders
trained on largely disjoint corpora, under different objectives, with
different architectures (`dda-definition.md` §1.2.4). We assume that for
the target downstream predictions (supervisory signals, alignment
targets, per-output predicates) the per-embedder projections
$\{\phi_m(X)\}$ carry **approximately independent** information axes.

**Formally.** We assume that the pairwise mutual-information terms
$I(\phi_j(X); \phi_k(X))$ are small relative to the marginal entropies
$H(\phi_j(X))$ for $j \neq k$, or equivalently that the $\binom{N}{2}$
cross-embedder interaction features $\rho_{jk}$ are non-constant
functions of their inputs. Strict statistical independence is **not**
claimed (V11).

**Hedge / boundary condition.** Axiom 2 is an **approximation**, not a
theorem. If two embedders collapse to linear reparameterisations of each
other, their pair contributes no information and the $\binom{N}{2}$
count becomes an upper bound that overstates the signal yield (see
Proposition 1 below). Formal measurement of embedder independence is
named future work (RO-8, agent-19; Q20(iv), agent-04); the V11
"approximately independent; strict formal orthogonality is an open
question" hedge is carried at every use.

**Consequence in the paper.** Axiom 2 is what makes DDA's count
non-degenerate. Under the axiom, each added frozen embedder contributes
additional supervisory information. The sensor-fusion isomorphism
(ISO-02) is exactly this axiom's structural parallel: N independent
sensors yield better state estimation than any single sensor, provided
noise sources are approximately independent.

### Axiom 3 (Reference-set learnability) — **centroids are learnable from $O(10^2)$-scale reference sets without overfitting**

For a subject $P$ and a modality $m$, let $R_m$ be a reference set of
$n_m$ raw samples of $P$ in modality $m$ (glossary #9). The centroid
$c_m$ computed as in §1.1 is **stable** — in the sense of producing a
generalising acceptance predicate on held-out inputs — for $n_m$ on the
order of tens to hundreds of reference samples, without requiring
thousands of subject-specific examples or gradient updates to the
embedder.

**Formally.** The empirical estimator $\hat c_m(R_m) =
\mathrm{normalize}(\frac{1}{n_m}\sum_i \mathrm{normalize}(\phi_m(r_{m,i})))$
converges in expectation to the population mean direction of
L2-normalised $\phi_m$-embeddings of $P$'s true sample distribution as
$n_m$ grows, and the rate of convergence is governed by the
concentration properties of $\phi_m$ (which is a property of the
embedder, not of subject-$P$-specific data). For frozen embedders whose
output geometry is already well-conditioned by pretraining (a
pretraining transfer assumption, T4), a reference set of
$n_m \in [10, 10^2]$ is empirically sufficient for useful $c_m$
(Case 3 T1-measured at $n_m = 50$; Case 2 T3-projected at $n_m = 2{,}362$
across visual modalities).

**Hedge / boundary condition.** Axiom 3 is a **claim about good
conditioning of modern frozen embedders**, not a universal statement.
For badly-conditioned embedders, or for subjects far outside the
pretraining distribution, larger reference sets may be required.
The sample-complexity curve (RO-10, agent-19) is the protocol that
would quantify this axiom per modality. Case 3 supplies the **one
measured operating point** (50 clips $\to$ 0.961 mean SECS); everything
else is T3-projected.

**Consequence in the paper.** Axiom 3 is the empirical anchor for
Claim 4 (sample-complexity collapse). Without it, the collapse claim
reduces to "we used fewer samples on one subject" rather than "the
method reduces subject-sample complexity to a reference-set scale
governed by embedder conditioning". Axiom 3 generalises the Case-3
result into a principled statement about the method.

---

## 3. Five propositions

Each proposition is derived from the axioms and / or the locked
construct definitions. Numbering is $P1, \ldots, P5$. Each proposition
carries a proof sketch (informal), a confidence tag, and a
manuscript-section anchor.

### P1. Signal-count identity — **$|D'| = n \cdot (N + \binom{N}{2})$**

Under Axioms 1 and 2, a fixed corpus $D$ of $n$ raw inputs projected
through a panel $\Phi$ of $N$ approximately-independent frozen embedders
produces

$$
|D'| \;=\; n \cdot \Bigl(N + \binom{N}{2}\Bigr) \;=\; n \cdot \frac{N(N+1)}{2}
$$

structured supervisory signals, where $D'$ is the derived dataset of
§1.2.2 in `dda-definition.md`.

**Proof sketch.** Axiom 1 ensures each $\phi_m(x)$ is a well-defined
signal (deterministic function of a real input), giving the $n \cdot N$
per-embedder projections. Axiom 2 ensures each pairwise interaction
feature $\rho_{jk}$ carries non-redundant information, giving the
$n \cdot \binom{N}{2}$ cross-embedder features. Summing gives the
identity.

**Confidence.** 95 %. The count is a **structural upper bound**, not an
information-theoretic lower bound. Redundant embedders reduce the
effective count without contradicting the identity.

**Manuscript anchor.** §3.4 (worked at $N=13$ and $N=7$), §3.1
(meaning-compression ratio view).

### P2. Collapse-silence — **recursive self-generation violates Axiom 1 and triggers collapse; frozen-embedder decomposition does not**

Let $D_t$ be a corpus at round $t$ such that $D_{t+1}$ contains samples
drawn from a generator trained on $D_t$. Under Shumailov et al. (2024),
Dohmatob et al. (2024), Alemohammad et al. (2023), recursive training
on $D_t$ induces distributional drift ("model collapse") whose rate
depends on the fraction of generator-in-loop samples at each round.

**Claim.** DDA's operational procedure does **not** satisfy the
generator-in-loop precondition: by Axiom 1, each $\phi_m(x)$ is a
frozen measurement of a real input $x \in D$, independent of any
generator's parameters. The derived dataset $D'$ is a decomposition of
$D$, not synthesis of new inputs. Therefore the collapse theorems do
not apply to DDA.

**Proof sketch.** The collapse theorems require a recursion of the form
$D_{t+1} \ni \tilde x \sim p_{\theta_t}$, where $p_{\theta_t}$ is the
generator trained on $D_t$. DDA's signals are $\{\phi_m(x)\}_{x \in D}$,
which depend only on $x$ and frozen $\phi_m$, not on any generator
$p_{\theta_t}$. The recursion condition fails; the theorems do not
apply.

**Confidence.** 90 %. The claim is **categorical scope**, not an
immunity theorem. We claim the preconditions of collapse are
unsatisfied, not that DDA has a new independence-from-collapse result
(agent-10 ORTH-B; agent-17 Theme A hedge).

**Manuscript anchor.** §3.3 (DDA scope argument); §1 three-paths
framing; §2.2 related work on synthetic-data collapse.

### P3. Geometric-over-scalar verifiability — **the constellation guard provides per-output verification that scalar-reward methods cannot**

Let $r_\psi : \mathcal{Y} \to \mathbb{R}$ be a learned scalar reward
model (RLHF / DPO / CAI). Let $G_\tau(\hat y; C_P) = \mathbb{1}[\forall
m : \cos(\phi_m(\hat y), c_m) \geq \tau_m]$ be the TCT constellation
guard predicate (`identity-locked-definition.md` §4.2).

**Claim.** $G_\tau$ is a **per-output, multi-dimensional, verifiable**
predicate at inference. $r_\psi$ produces a per-output **scalar** that
is not a verifiable predicate against a fixed target unless an
inference-time threshold and a frozen reward model are jointly
committed — neither of which is the design of RLHF / DPO / CAI.
Further, $G_\tau$'s multi-dimensional acceptance (conjunction over $M$
modalities) is strictly stronger than any single-encoder similarity
check (MP-01) as an identity predicate: failing any one modality
rejects, matching the multi-modal biological-recognition intuition
(ISO-03, metaphor-only).

**Proof sketch.** $G_\tau$ is computed from frozen $\phi_m$ and frozen
$c_m$; it is a deterministic function of $\hat y$ alone (and the fixed
$C_P$, $\tau$). It can be evaluated at inference for every candidate
with no auxiliary state. $r_\psi$, in the RLHF / DPO / CAI sense, is a
learned function whose gradient shapes the policy during training; at
inference it is typically not queried as a hard predicate because
Bradley-Terry-style reward recovery is identified only up to monotone
transformations (no meaningful absolute threshold). Even if one
committed to inference-time reward thresholding, the predicate would
be a **single-dimension** threshold on a **learned** quantity — weaker
than $G_\tau$ on both axes.

**Confidence.** 92 %. The "strictly stronger" clause rests on Axiom 2
(each modality carries non-redundant information about identity); under
perfectly-correlated modalities, $G_\tau$ collapses to the minimum over
redundant dimensions.

**Manuscript anchor.** §5.1 (scalar-vs-geometric contrast table); §5.3
(guard definition); §5.4 (per-output predicate); §6.3 (measured
instantiation in Case 3 at 0.961 mean SECS); §7 (paradigm-import
synthesis).

### P4. Role-composition coherence — **the same frozen $\Phi$ can serve three semantic roles (retriever, labeller, runtime guard) without retraining, and this composition is the single architectural move that unifies DDA, Context Graph, and TCT**

**Claim.** Under Axiom 1, a fixed panel $\Phi$ of frozen embedders can
be used simultaneously as (i) a **retriever** (Context Graph §4.1, the
cluster-D canonical role); (ii) a **labeller** (DDA §3.4, producing the
$n \cdot (N + \binom{N}{2})$ signals); (iii) a **runtime guard**
(constellation guard §5.3, enforcing per-output predicates) — with no
parameter update to any $\phi_m$. No cited paper in the LOCKED 88-pool
composes the three roles over the same frozen embedder instances
(MP-11 role-flip; MP-16 pool-relative scope).

**Proof sketch.** Role (i) queries $\phi_m$ on stored content to rank
candidates by cosine similarity. Role (ii) evaluates $\phi_m$ on a
fixed corpus $D$ to produce supervisory signals. Role (iii) evaluates
$\phi_m$ on a candidate $\hat y$ to enforce $\cos(\phi_m(\hat y), c_m)
\geq \tau_m$. All three are invocations of the same frozen function;
none updates $\phi_m$. By Axiom 1, none of the three roles is
generator-coupled, so they compose without interference.

**Confidence.** 90 %. The composition **coherence** is structural and
follows from Axiom 1. The **novelty** of the composition is
pool-relative (MP-16); a pool-external counter-example acknowledged
under R3 would weaken but not invalidate the claim because the
specific three-role composition over identical instances is the
empirical move this paper instantiates.

**Manuscript anchor.** §3 Framework opener (role-flip as unifying
move); §4.1 Context Graph (retriever instantiated); §4.2 ClipCannon
(labeller instantiated); §5.3 TCT (guard instantiated); §7 Discussion
(composition-simplicity claim).

### P5. Sample-complexity collapse — **identity-locked generation collapses from thousands of subject-specific examples to "one reference set per modality + frozen $C_P$" under TCT on DDA-enriched data**

**Claim.** Let $\mathcal{T}_{\mathrm{conv}}(P)$ be the sample complexity
(in subject-$P$-specific examples) of identity-locked generation under
the cluster-E / F conventional pipeline (single-prompt ICL, optional
light fine-tuning, within-family single-encoder SECS evaluation). Let
$\mathcal{T}_{\mathrm{TCT}}(P)$ be the sample complexity under TCT on
DDA-enriched data, with $M$ per-modality reference sets of size
$O(n_m)$ each, $n_m \in [10, 10^2]$ by Axiom 3. Then

$$
\mathcal{T}_{\mathrm{TCT}}(P) \;=\; O\!\Bigl(\sum_{m=1}^{M} n_m\Bigr) \;=\; O(M \cdot \bar n),
$$

which is independent of the size of the pretraining corpus $D$ and of
any corpus of subject-specific fine-tuning data beyond the reference
sets themselves. Empirically $\mathcal{T}_{\mathrm{conv}}(P) \gg
\mathcal{T}_{\mathrm{TCT}}(P)$ across cluster-E / F conventions
(agent-17 Theme F).

**Proof sketch.** Phase 1 (constellation construction) requires only
the reference sets $R_m$ of size $n_m$. Phase 2 (training or
selection) operates on DDA-enriched data that is **not** subject-$P$
specific — the supervisory signals come from the frozen embedder panel
applied to a general corpus, and the geometric-proximity loss anchors
the generator to the frozen $c_m$. Phase 3 (guard) is inference-time
only. The total subject-$P$-specific data requirement is exactly
$\sum_m n_m$. Axiom 3 then governs how small $n_m$ can be; Axiom 2
governs how the frozen embedders provide enough information content
per modality for small $n_m$ to suffice.

**Confidence.** 85 %. Case 3 is T1-measured at a single operating
point ($n_m = 50$, one speaker, English). Cases 1 and 2 are
T3-projected. The sample-complexity **curve** is named future work
(RO-10); the **collapse claim** is an AUTHOR-EMPIRICAL phenomenon
claim, not a proof (agent-11 CL-13, CL-16).

**Manuscript anchor.** §3.6 (sample-complexity framing); §5.4
(identity-locked property derivation); §6.3 (Case 3 measured);
§6.2 (Case 2 projected); §7 Discussion; §8 Limitations.

---

## 4. Two theorems

The paper is not a pure theory paper (per mission brief). We promote to
"theorem" only the two informal results that are sharp enough to carry
a name and load-bearing enough to lead §1 and §5. Each is stated in
prose-plus-equation form and proved by sketch.

### Theorem 1 (Sample-Complexity Collapse under TCT)

**Statement (informal).** Let $P$ be a subject and $C_P$ be the
constellation of cardinality $M = |\mathcal{M}|$ constructed per Axiom
3 from per-modality reference sets of size at most $k = \max_m n_m$.
Let $g_\theta$ be a generator trained or operated in TCT on
DDA-enriched data over a general corpus $D$. Then the subject-specific
sample complexity for identity-locked generation satisfies

$$
\mathcal{T}_{\mathrm{TCT}}(P) \;=\; O(M \cdot k),
$$

**independent of $|D|$** (the size of the general corpus) and
**independent of any subject-$P$-specific fine-tuning data beyond the
reference sets**.

**Proof (sketch).** By construction (`tct-definition.md` §3.2): Phase
1 uses only $\bigcup_m R_m$, contributing $\sum_m n_m \leq M \cdot k$.
Phase 2's supervisory signals come from DDA over $D$, which is not
subject-specific; Phase 2's geometric-proximity term anchors the
generator to $c_m$, already fixed in Phase 1. Phase 3 is
inference-time. Therefore the only subject-specific data used in the
pipeline is $\{R_m\}$, of total size $\leq M \cdot k$. $\square$

**Confidence.** 85 %. The theorem is **constructive**: any pipeline
with exactly Phase 1, Phase 2, Phase 3 as defined satisfies the bound
by construction. The **non-trivial content** of the theorem lies in
Axiom 3's claim that $k$ can be taken $O(10^2)$ without overfitting —
the Case-3 T1 measurement is the anchor for this value of $k$ in one
modality; Cases 1 and 2 are T3-projected.

**Why this is the theorem we lead with.** Theorem 1 is the sharpest
statement of Mission-Brief Claim #4 (sample-complexity collapse). It
converts "fewer samples worked in Case 3" into "the method's sample
complexity is independent of corpus size and subject-corpus size
beyond the reference sets themselves". That structural statement is
what the three-paths-around-the-data-wall framing (Theme A) cashes out
to at the subject level: Path 3 (decomposition through frozen
embedders) plus TCT reduces subject-specific sample requirements to
$O(M \cdot k)$, regardless of what Path 1 (acquire more data) or Path
2 (synthesise more data) would have required.

**Manuscript anchor.** §1 headline; §3.6 sample-complexity framing;
§5.4 formal statement; §6.3 Case 3 instantiation; §7 Discussion; §9
Conclusion.

**Hedges carried.**
- R3 pool-relative: the **cluster-E / F conventional** baseline is the
  comparator (many subject-specific examples + single-prompt ICL).
- R4 projected-vs-measured: $k = 50$ T1-measured in Case 3; other
  modalities T3-projected.
- Axiom 3 boundary: badly-conditioned embedders may require larger
  $k$; RO-10 is the protocol to quantify per modality.

### Theorem 2 (Runtime Verifiability of Identity Lock)

**Statement (informal).** Let $g_{\theta^*}$ be a generator produced by
Phase 2 of TCT for subject $P$, $C_P$ the Phase-1 constellation, and
$\{\tau_m\}_{m \in \mathcal{M}}$ per-modality thresholds. Then for every
generated candidate $\hat y$, the identity-locked predicate

$$
G_\tau(\hat y; C_P) \;=\; \mathbb{1}\!\left[\,\forall m \in \mathcal{M} : \cos(\phi_m(\hat y), c_m) \geq \tau_m\,\right]
$$

is **computable at inference time in $O(M \cdot d_{\max})$** where
$d_{\max} = \max_m d_m$, **requires no auxiliary learned model**, and
**produces a per-output accept / reject decision with a deterministic
cosine trace** $\{s_m(\hat y)\}$. No cited scalar-reward method
(InstructGPT, DPO, Constitutional AI) offers this combination of
properties; no cited identity-locked-generation method in clusters E /
F offers the multi-modal conjunction.

**Proof (sketch).** Computation: for each $m$, compute $\phi_m(\hat y)$
(cost bounded by the forward pass of $\phi_m$), then $\cos(\phi_m(\hat
y), c_m) = \langle \phi_m(\hat y), c_m \rangle / (\|\phi_m(\hat y)\| \,
\|c_m\|)$, then compare to $\tau_m$. Total cost $O(M \cdot d_{\max})$
for the cosine comparisons (plus the embedder forward passes, which are
paid once per modality per candidate). No auxiliary learned model is
invoked: $\phi_m$ is frozen by Axiom 1; $c_m$ is frozen by Phase 1;
$\tau_m$ is a scalar constant.

Per-output trace: $\{s_m(\hat y)\}$ is a direct output of the cosine
computation and is attached to $\hat y$ as a provenance record (see
`tct-definition.md` §3.2 Phase 3).

Cluster contrast: RLHF / DPO / CAI (cluster I) produce a scalar
preference or implicit reward with no frozen threshold; they do not
make per-output accept / reject decisions at inference. Cluster E / F
identity-locked-generation papers use single-encoder similarity
post-hoc on test sets; they do not run conjunctive per-output guards
at inference (MP-09 per-output runtime verification gap). $\square$

**Confidence.** 93 %. The computational bound and the "no auxiliary
model" property follow directly from the construction. The
cluster-contrast claim is **pool-relative** (R3; MP-16) — a
pool-external counter-example would weaken but not invalidate the
runtime-verifiability claim itself.

**Manuscript anchor.** §1 opener (paradigm-import framing); §5.3 guard
definition; §5.4 per-output predicate; §6.3 Case 3 measured
instantiation (0.961 mean SECS at $K=12$, $T=0.3$, $\tau=0.95$,
architecturally-independent scorer WavLM-base-plus-sv); §7 Discussion.

**Hedges carried.**
- R3 pool-relative: "no cited method" bounded by the LOCKED 88-pool.
- The guard is a **probabilistic** acceptance predicate (a formal-
  methods RV monitor is a logical predicate over executions); §8
  Limitations must state the difference (agent-20 ISO-01 register
  flag).
- False-positive / false-negative trade-off vs. threshold $\tau$ is
  Q2 future work (agent-19); not closed in this paper.

---

## 5. Relation to existing frameworks

This section situates the spine among the three adjacent research
programmes the paper extends, the paradigm it structurally breaks with,
and the cross-field isomorphisms (ISO-01 … ISO-08) that illuminate it.

### 5.1 Extends — Chinchilla scaling (by changing the axis)

**What Chinchilla claims.** Hoffmann et al. (2022) establish
compute-optimal scaling: for fixed compute, the optimal model-size /
training-token ratio is $\sim 20$ tokens per parameter. Kaplan et al.
(2020) provide the power-law backdrop: loss decays as a power law in
(compute, model size, data). Both are axes defined along
**bit-compressed token counts**.

**How the spine extends.** Under P1 (DDA's counting identity) and the
meaning-compression ratio view, the axis **data** is replaced with
**supervisory-signal density**. For a fixed raw corpus, meaning
compression multiplies the supervisory signals per unit of raw data by
$(N + \binom{N}{2}) / 1$ (baseline single-encoder supervision). The
Chinchilla exponent on the data axis $\beta$ is re-expressed — under
DDA-enriched supervision — as a function of structured-signal count
rather than token count. This is the **axis-extension**, not a
contradiction: scaling laws remain valid along their axes; DDA adds a
new axis.

**Future work.** Re-fit Kaplan / Chinchilla exponents $\alpha, \beta$
under DDA-enriched supervision (RO-1 F1b, agent-19). This is named
future work; the current paper does not close it.

**Agent-10 novelty code.** **EXT** of cluster-A scaling-law data-axis.
Not BRK.

### 5.2 Extends — Compression-as-intelligence (by introducing a fourth category)

**What compression-as-intelligence claims.** Huang et al. (2024) show
a strong within-model correlation ($r \approx 0.95$) between a trained
LLM's capability and its compression ratio on benchmark data.
Classical compression categories are **bit-compression** (Shannon),
**weight-compression** (GPTQ, SparseGPT, distillation), and
**activation-compression** (KV-cache quantisation, TurboQuant).

**How the spine extends.** Meaning compression (P1 in ratio view; RO-2)
is a **fourth compression category** orthogonal to the three. Unit
analysis (`meaning-compression-definition.md` §2.3): bit-compression
has units bits / bits; weight-compression accuracy / parameters;
activation-compression memory / output-quality; meaning-compression
structured-signals / raw-data-unit. The four unit pairs are
dimensionally distinct; the four categories trade along different
axes. The Kolmogorov / MDL / rate-distortion tradition-family (ISO-04)
is the information-theoretic root; this is a **taxonomic extension**
(CAT-EXT, agent-10) of the compression-as-intelligence frame, not a
replacement.

**Tension 2 resolved.** Huang r≈0.95 is a **within-model** compression-
capability correlation; V11 is an **across-model** embedder-
independence hedge. They operate on orthogonal axes (agent-17
tension-resolution). MDL's ratio object carries cleanly across both
scales.

**Agent-10 novelty code.** **CAT-EXT** of cluster-C compression theory.
Not BRK.

### 5.3 Breaks with — Scalar-reward alignment (structural break)

**What scalar-reward alignment claims.** InstructGPT (Ouyang et al.,
2022) optimises a generator against a learned scalar reward model
trained on human preference pairs. DPO (Rafailov et al., 2023)
closed-forms the same objective. Constitutional AI (Bai et al., 2022)
augments with AI-feedback preferences and natural-language rules.

**How the spine breaks.** TCT (`tct-definition.md` §3.5 NOT clause)
differs on both axes:
- **Aim.** Scalar-reward aim is a learned scalar function;
  spine aim is a frozen multi-modal constellation $C_P$.
- **Instrument.** Scalar-reward instrument is a learned reward model;
  spine instruments are frozen embedders $\Phi$.
- **Metric.** Scalar-reward metric is a scalar preference; spine
  metric is a fixed cosine.
- **Runtime verification.** Scalar-reward methods do **not** run
  per-output guards at inference; spine runs the constellation guard
  at every output (P3; Theorem 2).

Per P3 and Theorem 2, the spine delivers **structural properties that
scalar-reward alignment cannot match** (per-output verifiability,
multi-modal conjunction, frozen target). This is a **structural break**
with the scalar-reward paradigm (BRK-I, agent-10) — not a competitive
claim on the same axis (we do not argue TCT yields higher preference
scores than RLHF on RLHF benchmarks; we argue TCT offers per-output
geometric verifiability that RLHF benchmarks do not evaluate).

**Agent-10 novelty code.** **BRK** of cluster-I scalar-reward alignment
+ **EXT** of cluster-D contrastive geometry.

### 5.4 Imports from — Cross-field isomorphisms (structural parallels, no citations)

Five isomorphisms (agent-20, ISO-01 … ISO-08) are named in the body as
structural framing borrows. They do **not** add citations to the
LOCKED pool (R3); they name tradition-families for paradigmatic
legibility.

| ISO | Field | What it illuminates about the spine |
|---|---|---|
| **ISO-01** | Formal-methods runtime verification | Explains "runtime" in the guard; Theorem 2 imports the per-output monitor topology. |
| **ISO-02** | Signal-processing sensor fusion | Explains the "N independent measurements" intuition behind P1; the $\binom{N}{2}$ pairwise term corresponds to cross-sensor covariance. |
| **ISO-04** | Kolmogorov / MDL / rate-distortion | Explains why meaning compression is the **fourth category** in a unit-analytic taxonomy (§5.2). |
| **ISO-06** | Astronomical constellations | Justifies the paper's central terminological choice: a **multi-reference, frozen** object used to **orient** generation, not a single target to **optimise** toward. |
| **ISO-07** | Statistical process control | Explains the per-output acceptance predicate with control limits; $\tau_m$ as a tolerance bound. |

Agent-20's register flags carry: each ISO is a **topology-of-control-
and-information-flow** parallel, not a mathematical-equivalence claim.
§7 Discussion uses only three (ISO-01, ISO-02, ISO-06) as the
synthesis-paragraph foundation to respect the "depth on ≤ 3"
directive.

---

## 6. Framework summary (single table)

| Element | Role in the spine |
|---|---|
| **Object** $C_P$ | The single paper-level object. DDA produces its centroids; meaning compression measures their substrate; TCT uses it as loss target (Phase 2) and guard (Phase 3); identity-locked generation is defined by the predicate over it. |
| **Axiom 1** (Grounded measurement) | Frozen embedders are generator-independent measurement instruments; enables the collapse-silence argument (P2) and the role-composition coherence (P4). |
| **Axiom 2** (Approximate orthogonality) | The $N$-embedder panel carries approximately independent information axes; enables the signal-count identity (P1) and the geometric-over-scalar verifiability (P3). V11-hedged. |
| **Axiom 3** (Reference-set learnability) | Centroids are stable at $n_m \in [10, 10^2]$; enables the sample-complexity collapse (P5; Theorem 1). Case 3 T1 anchor. |
| **P1** Signal count | $\lvert D' \rvert = n(N + \binom{N}{2})$. |
| **P2** Collapse silence | DDA's recursion does not satisfy generator-in-loop preconditions of Shumailov 2024. |
| **P3** Geometric verifiability | Guard gives per-output verification; scalar reward cannot. |
| **P4** Role composition | Same frozen $\Phi$ serves retriever / labeller / guard roles. |
| **P5** Sample-complexity collapse | $\mathcal{T}_{\mathrm{TCT}}(P) = O(M \cdot k)$. |
| **Theorem 1** (lead) | $\mathcal{T}_{\mathrm{TCT}}(P) = O(M \cdot k)$, independent of $\lvert D \rvert$. |
| **Theorem 2** (support) | $G_\tau$ is computable per-output in $O(M \cdot d_{\max})$ with no auxiliary learned model. |
| **EXT** of | Chinchilla scaling (axis extension); compression-as-intelligence (fourth category). |
| **BRK** of | Scalar-reward alignment (aim / instrument / metric / runtime-verification). |
| **Imports** | RV (ISO-01); sensor fusion (ISO-02); MDL (ISO-04); astronomy (ISO-06); SPC (ISO-07). |

---

## 7. What the framework does NOT claim (scope discipline)

- **Not** a new theoretical primitive. DDA, meaning compression, TCT,
  constellation guard are **compositions** (MP-12). Each axiom and
  proposition is built from already-locked objects (agent-04).
- **Not** a formal proof that the axioms hold universally. Axioms 1
  and 2 are engineering-operational assumptions (V11 hedge; future
  RO-8 empirical measurement). Axiom 3 has one T1 operating point
  (Case 3) and T3 projections elsewhere.
- **Not** a claim that TCT outperforms RLHF / DPO on RLHF benchmarks.
  The spine **structurally breaks** with scalar-reward alignment; it
  does not compete with it on the same axis (§5.3).
- **Not** a claim that the constellation guard is a logical
  runtime-verification monitor in the strict formal-methods sense. It
  is a probabilistic acceptance predicate (§8 Limitations flag per
  agent-20 ISO-01 register flag).
- **Not** a claim that the novelty is globally unprecedented. MP-16
  pool-relative scope: the three-role composition over identical
  frozen instances is novel **in the LOCKED 88-pool**; a pool-external
  counter-example would be acknowledged and would not invalidate the
  composition-level claim.
- **Not** a claim of AGI (mission-brief hard constraint) or imminent
  plateau (agent-17 Theme A hedge). DDA reframes the data wall; no
  stronger claim is made.

---

## 8. Hedges carried from upstream agents

- **Agent-01 V11** — "approximately independent; strict formal
  orthogonality is an open question" carried at every use of
  orthogonal / independence in the axioms and propositions.
- **Agent-01 V18** — "frozen" or "frozen-with-LoRA", never ambiguous
  "pretrained".
- **Agent-01 V22** — measured vs. projected clause-level flagging.
- **Agent-11 CL-08** — Huang r≈0.95 is SINGLE-SOURCE within-model
  correlation.
- **Agent-11 CL-11** — V11 hedge is MODERATE.
- **Agent-13 Flags 1, 2, 3, 4** — closed-artefact frontier papers
  cited as paradigm anchors, not reproducible evidence.
- **Agent-14 Bias-02, -03, -05, -06, -07, -10, -11** — scope-device
  citations; geographic / institutional concentration; single-subject
  / single-encoder-pair / English-only Case-3 scope; one dated
  Q4-2024 industry narrative; dual-use disclosure.
- **Agent-17 Register R1 (MP-12)** — composition-not-invention;
  agent-17 Theme A temporal-narrative hedge; Theme C scope-device
  hedge; Theme D single-encoder pair hedge.
- **Agent-17 Register R3** — pool-relative scope on every novelty
  claim.
- **Agent-17 Register R4** — projected / measured / AE tier-flagging.
- **Agent-20 ISO register** — tradition-families named without
  specific citations; no physics-borrowing prose; no Friston / no
  Waddington / no six-sigma numerics.

---

## 9. Cross-references

- **`dda-definition.md`** — DDA formal definition; Axiom 2 compatibility.
- **`meaning-compression-definition.md`** — ratio view of DDA; §5.2 of
  this file extends.
- **`tct-definition.md`** — three-phase protocol; Axioms 1 and 3
  compatibility; Theorems 1 and 2 derive from this.
- **`identity-locked-definition.md`** — per-output predicate; P3 and
  Theorem 2 derive from this.
- **agent-17 thematic-frameworks.md** — six themes; every axiom /
  proposition / theorem anchors to a theme primary section.
- **agent-19 research-opportunities.md** — RO-1 (P1); RO-2 (§5.2);
  RO-3 (P3); RO-4 (P3, Theorem 2); RO-5 (Theorem 2 hygiene); RO-6
  (Axiom 3 operating point); RO-8 (Axiom 2 future measurement); RO-10
  (Axiom 3 future curve).
- **agent-20 isomorphisms.md** — §5.4 of this file inherits five
  body-named ISOs.

---

## 10. Confidence

**HIGH** on the framework's internal consistency and external
positioning.

- Every axiom is paired with its hedge and boundary condition.
- Every proposition has a proof sketch, confidence, and section anchor.
- Both theorems are constructively proved under the axioms with
  explicit boundaries.
- Section 5 (Relation to existing frameworks) preserves agent-10
  novelty codes verbatim.
- All upstream hedges (agents 01, 11, 13, 14, 17, 20) carry to the
  spine.
- No new citations are introduced (R3 compliance).
- No new primitives are introduced (MP-12 composition-not-invention).
- The spine is the single theoretical object from which §3 + §5 of the
  manuscript can be taught without re-deriving upstream constructs.

## Changelog

- 2026-04-17 — Initial issue by agent-21-theory-builder.
