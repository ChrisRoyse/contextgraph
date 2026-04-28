---
namespace: discoveries
agent_id: agent-42-adversarial-reviewer
timestamp: 2026-04-17T10:30:00Z
status: active
locked: false
supersedes: none
tags: adversarial-review, red-team, critique, pre-submission, stop-ship, arxiv
depends_on:
  - memory/context/mission-brief.md
  - memory/context/constraints.md
  - memory/discoveries/agent-32-contradiction-analyzer--contradiction-register.md
  - memory/discoveries/agent-33-falsification-agent--counterarguments.md
  - memory/patterns/agent-34-confidence-quantifier--per-claim-confidence.md
  - memory/handoff/agent-41-to-42--abstract-ready.md
  - sections/00_frontmatter.md
  - sections/01_introduction.md
  - sections/02_related_work.md
  - sections/03_framework_dda_mc.md
  - sections/04_systems.md
  - sections/06_method_tct.md
  - sections/07_case_studies.md
  - sections/08_discussion.md
  - sections/09_limitations.md
  - sections/10_conclusion.md
downstream_consumers:
  - agent-43-reproducibility-checker
  - agent-44-citation-validator
  - agent-45-length-auditor
  - agent-46-coherence-checker
  - agent-47-final-assembler
---

# Adversarial Review Report — The Red Team File

## 0. Role and disposition

I am agent-42. My job is to **fail** this paper before arXiv does. I
have read every section, every hedge, every paste-ready disclosure,
and I am reading them as a hostile NeurIPS / ICLR / arXiv moderator
who has not read the SMAP memory. I take every softening seriously —
most of them are load-bearing — but I also know that reviewers do not
read 1,800-line contradiction registers; they read the PDF. If a
hedge is defended in agent-32 but invisible in the manuscript, it does
not exist for review purposes.

Prior red-team work (agent-33 CA-1 … CA-6; agent-32 IC-1 … IC-6,
EC-7 … EC-11; agent-34 claim-level confidences; agent-29 validity
threats) is **upstream of this review**. I inherit the 11
contradictions, 6 counterarguments, 14 claim confidences, and the
validity-threat register; I do not re-litigate them. What I add is
**(a)** a section-by-section critique of the drafted manuscript as it
now reads, **(b)** 14 cross-cutting attacks (8 from the task brief
plus 6 I invented while reading), **(c)** severity tallies and a
stop-ship list, and **(d)** concrete text edits for every CRITICAL
and HIGH finding.

## 1. Manuscript-state snapshot

| Section | File | Approx. words | Reviewer first-read status |
|---|---|---|---|
| §0 Abstract | `sections/00_frontmatter.md` | 298 | strong hedges, dense but accurate |
| §1 Introduction | `sections/01_introduction.md` | ~1,470 | central reader-exposure surface |
| §2 Related Work | `sections/02_related_work.md` | ~2,600 | long, pool-relative throughout |
| §3 Framework | `sections/03_framework_dda_mc.md` | ~1,640 | formal spine |
| §4 Systems | `sections/04_systems.md` | ~940 | short; FM-01-adjacent |
| §5 TCT Method | `sections/06_method_tct.md` | ~2,700 | dense; Case 3 is the anchor |
| §6 Case studies | `sections/07_case_studies.md` | ~3,200 | where most overclaim risk lives |
| §7 Discussion | `sections/08_discussion.md` | ~2,300 | softening pass applied |
| §8 Limitations | `sections/09_limitations.md` | ~1,400 | exhaustive vs. defended claims |
| §9 Conclusion | `sections/10_conclusion.md` | ~610 | tight |

Two load-bearing assumptions for this review:

- Every MUST-softening from agent-33 (1 "up to 91"; 2 constructive /
  empirical split on Theorem 1) **appears in the drafted text** — I
  verified each in-place. If a subsequent edit removes them, this
  review becomes partially obsolete.
- The paper does **not** contain the §Ethics and §Competing Interests
  text yet. Those are named as future inserts (see §1, §7.6, §8.L8).
  The FM-01 regex pass has NOT been run by me; I give a pre-scan
  reading below in CA-11 and §5 below.

---

## 2. Section-by-section critique

### Section 1 — Introduction

**Finding 1.1 — MEDIUM — "Third path around the data wall" lands
before any evidence.**
§1 opens with a 450-word data-wall framing that reads closer to an
essay than an arXiv introduction. By paragraph 3 the reader has been
told there is a "third path" but has no evidence and no definition of
the panel $\Phi$. Hostile review: "the paper's emotional peak is in
paragraph 3 of §1 — that is a marketing cadence." Remediation:
tighten paragraph 1 to ~3 sentences (one stock, one access, one
recursion). The Efrati / Metz / Hu citations should be consolidated
into a single parenthetical, per §2.1's own more disciplined
treatment.

**Finding 1.2 — HIGH — "to our knowledge in the locked 88-citation
pool" appears four times in §1 and once in the abstract.**
This is the novelty-hedge that protects CA-1 / R3 against
pool-external counter-examples. Two problems. (a) The repetition
draws attention to the hedge: reviewers who see a paper repeating
"to our knowledge within our own pool" will suspect the pool is a
defensive perimeter. (b) The phrase "locked 88-citation pool" is
opaque — the reader does not know what "locked" means and why 88 is
special. Remediation: introduce the phrase once, in a footnote,
saying "the reference set of 88 sources this manuscript cites; see §2
for the thematic scope." Subsequent occurrences should say "within
the cited literature" or "within the reference set". The literal
count "88" should disappear from the abstract entirely.

**Finding 1.3 — HIGH — "91 signals per input at $N=13$" is stated
with "up to" but the closing paragraph re-emphasises "up to 91"
without the "constructive upper bound" phrase again.**
The final two sentences of §1 paragraph 4 read: "The first
consequence is the counting identity $|D'| = n \cdot (N +
\binom{N}{2})$ — up to 91 signals per input at $N=13$ — which
positions meaning compression as a fourth taxonomic entry..." This
sentence buries the softening ("up to") and pairs the fourth-category
proposal tightly with the 91 number. Hostile reviewer will read
"91 = fourth category" and claim the fourth-category argument is
conditional on the unmeasured tight count. Remediation: separate the
two moves into two sentences, and explicitly name "constructive upper
bound" on the 91. See §6.1 paste-ready edit below.

**Finding 1.4 — MEDIUM — "a retrain on forensically-cleaned data is
in progress at the time of writing" and "the Santa clone's
flow-matching sign-bug has been diagnosed" are live-status statements
that timestamp the paper.**
Reviewers dislike "in progress" claims; arXiv reviewers especially
dislike them, because they break the reader's model of the paper as
a completed artefact. Remediation: adopt `(projected; camera-ready
will re-verify)` throughout consistent with §6.1 and §6.2, and drop
the rhetorical "at the time of writing" phrasing in §1.

**Finding 1.5 — MEDIUM — the phrase "solve" appears zero times, but
"reframes" / "third path around the data wall" still reads as
triumphalist.**
The paragraph-6 line "These three properties do not solve the data
wall. They reframe it as three complementary paths..." is correct
and is the right softening. But "third path" in the title, abstract,
§1 opener, §3.3, §7.6, and §9 — five-plus repetitions — is the kind
of slogan that a reviewer will point at. Remediation: keep "third
path" in the title (mission brief locks it). Reduce to 2 in-body
occurrences (§3.3 establishing; §7.6 closing). Every other occurrence
should be replaced with "decomposition path" or "the configuration of
§3".

### Section 2 — Related Work

**Finding 2.1 — LOW — §2 is disciplined but long (~2,600 words).**
arXiv reviewers will accept this but some will skim. Remediation: no
structural change. Flagged for §7 length-auditor, not for adversarial
fix.

**Finding 2.2 — MEDIUM — §2.5 "no cited system aggregates a
multi-clip reference set into an L2-normalised per-modality centroid"
is a strong pool-relative claim.**
This is exactly the sentence a reviewer will stress-test. The
specific risk: prior face-recognition literature (FaceNet — Schroff
et al. 2015), prior multi-prompt retrieval (averaged query
embeddings, e.g., Santhanam et al. 2022 ColBERTv2 has some form of
query aggregation) have L2-normalised averages of reference
projections. They are not cited in the 88-pool. A hostile reviewer
will say the pool is tailored to exclude exactly the prior art that
would break the claim. Remediation: add an explicit footnote in
§2.5 stating that the 88-pool is not exhaustive and that the novelty
claim is that no cited system **composes** multi-clip aggregation
with per-output cosine guard and multi-modal conjunction — not that
multi-clip averaging itself is novel.

**Finding 2.3 — HIGH — §2.5 cites VALL-E 2 at "WavLM-large SV cosine
around 0.881" and §1 claims "0.961 mean SECS".**
This is the EC-10 exposure. The text in §2.5 does not pre-empt the
"0.961 > 0.881" apples-to-apples read. §6.3 has the paradigm-scope
disclosure but §1 does not explicitly say "this is not comparable to
VALL-E 2". Remediation: §1 must carry a one-clause disclosure —
"(a personalised protocol, distinct from zero-shot TTS paradigms such
as VALL-E 2)". See §6.1 paste-ready edit.

**Finding 2.4 — MEDIUM — §2.3 states "None is a per-input
labelled-signal-yield measurement, and none of the
compression-as-intelligence papers prescribes compression *of the
training corpus* via multiple frozen embedders."**
This is a strong taxonomy claim. A hostile reviewer will cite CLIP
(Radford et al. 2021, not in pool), SimCLR (Chen et al. 2020), or
ALBEF as methods that absolutely produce per-input labelled-signal
yields via multiple frozen (at some stage) encoders. Remediation:
same as 2.2 — footnote the pool-relative scope and narrow the claim
to "at dataset-preparation time with $N \geq 10$ independently-trained
frozen embedders". See §6.2 paste-ready.

**Finding 2.5 — MEDIUM — §2.6 Gap (v) reads as methodological
homework.**
"Formal embedder-independence measurement is missing" — stated as a
gap, but this is the gap the paper's own claim depends on. Reviewers
dislike self-nominated gaps that are also the paper's own weakness.
Remediation: move gap (v) to §8 Limitations L4 context, and name it
as "the instrument that would tighten our own claim", not as a gap in
the field.

### Section 3 — Framework

**Finding 3.1 — HIGH — Identity (1) $|D'| = n \cdot (N(N+1)/2)$ is
ambiguous about what a "signal" is.**
§3.1 defines a signal as "a scalar or low-dimensional feature that is
a deterministic function of a raw input under frozen embedders and
frozen interaction rules". That admits any function. A hostile
reviewer will observe that under this definition, the count is
trivially unbounded: apply 1,000 different cosine functions, get
1,000 signals. Remediation: tighten the definition to "a distinct
per-embedder or per-pair output that is indexable and storable as a
training target or input, such that distinct signals are not linear
reparameterisations of each other". See §6.3 paste-ready edit.

**Finding 3.2 — HIGH — §3.2 Table 1 has "Meaning compression" in
bold and proposes it as a category.**
The table will be read as "the author invented a new compression
category". §3.2 correctly hedges with "we refrain from dimensionally
distinct" and "scholarly judgement we do not pre-empt", but the table
itself does not carry that hedge. Remediation: retitle the row
"**Meaning compression** (proposed)" with an explicit marker that
this is this paper's contribution under review. See §6.4.

**Finding 3.3 — MEDIUM — §3.2 "for fixed raw-data volume, how many
structured supervisory signals can I extract?" is too close to the
augmentation framing to distinguish itself without the CA-5 triad
(no new inputs; frozen encoders; multiple independent encoders).**
The CA-5 response lives in agent-33; it is invoked implicitly in §3.1
paragraph 5 ("DDA is therefore not synthetic-data generation... not
data augmentation... not distillation") but the distinction is stated
negatively. Remediation: add a positive sentence in §3.2 after the
table: "Three properties distinguish meaning compression from data
augmentation and multi-view representation learning: (a) no new
inputs are generated (the raw $x$ is unchanged); (b) the encoders are
frozen (no parameter update from the $\phi_m(x)$ signals); (c) the
panel has $N \geq 2$ independently trained encoders (not a
paired-encoder view or a single encoder with multiple heads)."

**Finding 3.4 — MEDIUM — §3.3 collapse-silence claim reads cleanly
but "cited theorems are silent on this configuration" inherits CL-06.**
A hostile reviewer will cite any **non-cited** collapse-ish result
(e.g., Bertrand et al. 2023 on generative stability; there is a
cluster of post-Shumailov theoretical work) and claim the silence is
an artefact of pool selection. Remediation: add a footnote to §3.3
stating "the scope argument applies to the specific
Shumailov-Dohmatob-Alemohammad recursion regime; generalised
feedback-induced drift theorems outside this regime are out of scope
of this claim."

### Section 4 — Systems

**Finding 4.1 — HIGH — §4 carries FM-01 risk.**
Paragraphs in §4.1 and §4.2 enumerate Context Graph and ClipCannon
with line counts, tool counts, test counts, column-family counts.
This reads as product advertisement. A hostile reviewer will flag:
"why is the system description given in the primary body and not in
an appendix? The main claim is DDA and TCT — the system numbers are
supporting context." Remediation: (a) move the line-count / tool-
count / test-count enumeration to an appendix or a footnote, keep in
body only the "13 frozen embedders" and "7 modalities" as they are
load-bearing; (b) re-frame §4 explicitly as "constructive evidence
that a DDA labeller role can be implemented at production scale",
which §4 already says once — say it three times, in the opening, in
§4.3, and in the appendix pointer.

**Finding 4.2 — HIGH — §4.3 paragraph 3 mentions Leapable.ai with
its Stripe Connect 70/30 revenue split, 5% platform reserve, and
HMAC-signed billing ledger.**
This is the exact phrasing a hostile reviewer will grep for as
marketing. The paragraph says "described in the acknowledgements as
the commercial exemplar" — but then describes commercial terms in the
body. This is self-contradicting. Remediation: delete the Stripe
Connect / 70/30 / 5% / HMAC sentence from §4.3 body and move it in
full to §Acknowledgements. §4.3 keeps only the structural statement
"Leapable.ai is the commercial exemplar downstream of the two
systems; it is not evaluated in Section 6."

**Finding 4.3 — MEDIUM — "5,184 tests" is a point-in-time number.**
A reviewer opens the paper six months later, the test count is
7,200; the reviewer loses trust in the literal numbers. Remediation:
add "as of the measurement date" or "approximately 5,000" instead of
"5,184". Same for 75 MCP tools, 58 MCP tools, 59 column families.
Exact integers read as marketing; approximate ranges read as
engineering reality.

### Section 5 — TCT Method

**Finding 5.1 — CRITICAL — §5.3 defines the Phase-3 guard predicate
but the "per-output verifiability relative to the chosen centroid
panel" phrase lives only in §5.4 paragraph 2.**
The acceptance-predicate paragraph itself does not carry the
encoder-matched scope. A reader skimming §5.3 sees the predicate, the
$O(M \cdot d_{\max})$ complexity, "no auxiliary learned model", and
walks away believing this is cross-encoder verification. The scope
comes in the NEXT paragraph and uses hedged language ("what is
verified is that $\hat y$ satisfies the specified cosine predicate
against the frozen $C_P$; what is not verified is that $\hat y$ is
correct under any external identity criterion"). Remediation: fold
the scope directly into the predicate-paragraph — the FIRST sentence
after (5) must say: "This predicate is verifiable **relative to the
chosen $\Phi$ and $C_P$**; cross-encoder transportability is a
separate question not resolved by the predicate." See §6.5
paste-ready edit.

**Finding 5.2 — HIGH — §5.3 "Threshold calibration" paragraph uses
"the canonical procedure is the same-session-band floor".**
Two issues. (a) "Canonical" is a strong word; the same-session-band
floor is a choice, not a canon. (b) The paragraph says "Any
subsequent adjustment of $\tau_m$ to hit an evaluation number
invalidates the guard" — this is correct but reads as defensive. A
reviewer will ask: was $\tau = 0.95$ in Case 3 chosen before or after
the 0.961 measurement? The paper does not state this explicitly.
Remediation: (a) replace "canonical" with "specified"; (b) add a
sentence to §5.6 Case 3 specifying "$\tau = 0.95$ was set from the
same-session-band floor on a disjoint calibration set **before**
measurement of the 10 held-out sentences". If that ordering is not
true, the guard is post-hoc and the 0.961 is inflated.

**Finding 5.3 — HIGH — §5.4 calls $G_\tau$ "deterministic modulo GPU
floating-point reproducibility".**
A hostile reviewer will seize on this: a deterministic predicate that
depends on GPU float ordering is not deterministic in the formal
sense. Sampling-based generators with identical seeds can produce
different token outputs across CUDA versions; the cosine score
downstream will vary, and candidates near $\tau$ can flip decisions.
Remediation: soften to "the guard predicate is deterministic given
the embedder, centroid, and candidate tensors; in practice, bf16/fp32
mixed precision introduces $O(10^{-4})$-level cosine variance at the
threshold margin, which the audit record captures." This is W10's
own remediation language; invoke it.

**Finding 5.4 — HIGH — §5.5 Layer 3 regeneration loop at $K = 4$ per
sentence without a pre-declared fallback for Case 1 is
under-specified.**
"Rejection triggers a bounded regeneration loop ($K = 4$ per
sentence)" — what happens on budget exhaustion? §5.4's rejection-
handling paragraph names `regenerate`, `fallback`, `escalate` but
§5.5 does not say which policy Case 1 uses. Remediation: name the
policy explicitly in §5.5 Layer 3.

**Finding 5.5 — MEDIUM — §5.6 Case 2 lists seven modalities with
six distinct $\lambda_m$ values plus a seventh "voice identity" with
no listed weight.**
The "(architecturally distinct from WavLM...)" clause is useful but
the $\lambda_{\mathrm{voice}}$ is absent. Remediation: either state
$\lambda_{\mathrm{voice}}$ explicitly (recommended if the projection
is being presented as specified) or mark it as "to be calibrated at
runtime" and acknowledge that the six-modality loss runs without this
term in the current draft.

### Section 6 — Case Studies

**Finding 6.1 — CRITICAL — §6.3.2 headline uses **boldface** on
"0.961 mean WavLM SECS", "max 0.975", "9 of 10 sentences at SECS
$\geq 0.95$", and includes **bold** disclosure "encoder-matched"
only as an inline clause.**
Rendered, the eye will go to the bolded numbers; the bolded
"encoder-matched" appears only once as the disclosure block title.
This is exactly the asymmetry that creates the IC-3 / EC-10 risk.
Hostile reviewer: "headline is bold; scope is small-caps in the
disclosure block." Remediation: (a) demote the bold on "0.961"; (b)
use `*0.961 mean WavLM SECS (encoder-matched, same-family
guard-and-evaluator)*` as the complete bolded unit; the
evaluator-family scope must be **part of** the headline, not adjacent
to it. See §6.6 paste-ready edit.

**Finding 6.2 — CRITICAL — §6.3.3 Table 1 temperature sweep shows
T=0.3 with bold values.**
The table shows mean WavLM SECS at T=0.3 as **0.932**, which
conflicts with the "mean 0.961" headline earlier. The text explains
this: "the 0.961 headline is a separate run at the same $T = 0.3$
operating point reported in the paper-level evaluation." A hostile
reviewer will read: "why is the sweep-internal mean 0.932 but the
headline 0.961? Different seeds? Different sentences? Selective
reporting?" Remediation: **add a sentence clarifying that the two
values are from different run dates (or different seed chains or
different sentence sets) and that the sweep established T=0.3 as the
optimum before the headline run**. If the 0.961 run was a
re-evaluation at the same T=0.3 with a different seed choice that
happened to outperform the sweep, the paper must say so; otherwise
this reads as selective reporting. This is a POTENTIAL DATA-
INTEGRITY issue that a cautious reviewer WILL flag.

**Finding 6.3 — HIGH — §6.3.4 centroid-size ablation shows
"Single reference (1 clip)" with WavLM mean **0.946** and "—" in the
max column.**
The missing max cell invites suspicion. "—" could mean "not
measured" or "not applicable". A reviewer will ask: did the 1-clip
baseline not have a best-of-12? If it did, where is the max? If not,
the ablation is not paired at the same $K$ level. Remediation:
state explicitly in the table caption whether the 1-clip baseline
used best-of-12 or a single draw; if single-draw, note that the
ablation conflates centroid size with best-of-$K$.

**Finding 6.4 — HIGH — §6.3.5 three "pending" companion
measurements are described in imperative future tense.**
"Re-score the 10 argmax candidates with ECAPA-TDNN...", "report
mean and max ECAPA SECS...", "Instrument specification is locked;
run pending." These read as promissory notes to the reader. The
camera-ready version is supposed to include these, but the arXiv v1
does not. A reviewer will ask: "if the instruments are locked and
the computational cost is minutes, why is the paper being submitted
without them?" Remediation: add a one-sentence justification per
pending measurement — e.g., "ECAPA-TDNN re-scoring was deferred to
v2 because [specific reason — e.g., independent scorer weights under
pending license review, OR resource constraints, OR the re-scoring
must be done on unchanged audio and the v1 release artefact schedule
is frozen]". If there is no good reason, the three experiments
should block submission.

**Finding 6.5 — HIGH — §6.2.2 lists six forensic fixes in a single
paragraph.**
The flow-matching sign bug, the cv2 frame-rate mismatch, the mouth-
bounding-box bug, the dead q_audio LoRA target, the 22 GB RAM leak,
and the viseme curriculum bug are presented together. A reviewer
will read: "six major bugs were found in your own pipeline — how is
this ready for publication?" Remediation: reframe the forensic list
as "six pre-submission integrity audits documented in the forensic
record; each has a commit-level fix and a post-fix verification
step." Move commit hashes into the appendix. The fact that the
forensic tooling caught the bugs IS the contribution; the list of
bugs per se reads as an admission.

**Finding 6.6 — MEDIUM — §6.2.4 "Track A / Track B" drafting
language leaks into the body.**
"Two-track drafting is in place: Track A ships measured per-modality
tables for Case 2 and cleaned-retrain numbers for Case 1 if runs
complete; Track B preserves the forensic-narrative contributions..."
This belongs in the author memo, not in the published limitations.
Remediation: delete the Track A / Track B paragraph from §8.L3 and
replace with "Case 2 per-modality tables are projected; the camera-
ready version is expected to carry them or to re-state the scope
tag."

**Finding 6.7 — HIGH — §6.3.2 "temperature 0.3 with independent
seeds" — the word "independent" is load-bearing.**
If the 12 candidates are drawn with 12 independent seeds, the
argmax distribution is a well-defined statistic. If they are drawn
with a shared seed and temperature-sampling alone, the 12 are
correlated and the best-of-12 statistic has a different
interpretation. Remediation: state the seed procedure explicitly:
"twelve candidates drawn with independent random seeds at
$T = 0.3$; seed schedule is logged per-candidate for audit."

### Section 7 — Discussion

**Finding 7.1 — LOW — §7 is disciplined and applies most agent-33
softenings.**
No structural issue. Minor hedging fixes only.

**Finding 7.2 — MEDIUM — §7.3 "even in principle" on
per-output verifiability.**
"The property we claim — per-output verifiability relative to the
chosen panel — is one that scalar-reward alignment cannot furnish
even in principle." This is the kind of "even in principle" statement
that reviewers hate; scalar-reward alignment can trivially be made
per-output verifiable by adding a thresholded reward head at
inference. Remediation: soften to "is one that scalar-reward
alignment **as practiced in the cited literature** does not furnish;
a hybrid approach that adds a thresholded reward-head guard would
cross this boundary and would be a productive extension." See §6.7
paste-ready.

**Finding 7.3 — MEDIUM — §7.5 "TCT operationalises the collapse
implicit in those embedders, it does not cause it (W6)".**
This is correct attribution but the previous sentence reads: "the
sample-complexity reading is substantially a property of
well-conditioned frozen embedders inherited from the retrieval and
contrastive-geometry literature that produced WavLM". This is a
weakness disclosure being invited into the discussion. Good — but a
reviewer will ask: "if the sample-complexity collapse is already a
property of well-conditioned frozen embedders, what exactly is TCT
adding?" The answer (per CA-2/CA-3) is "the composition of
multi-modal conjunction + frozen target + per-output runtime". State
this explicitly as a bulleted "what TCT adds on top of frozen
embedders" — otherwise the attribution-discipline sentence reads as
TCT giving away its own contribution.

**Finding 7.4 — MEDIUM — §7.4 "DDA is structurally not subject to
this regime by construction" — the word "structurally" is hedge-
adjacent but could be misread.**
A hostile reviewer will ask: what does "structurally not subject to"
mean in a theorem sense? It is not a claim of immunity but it is
close enough that a careless reader could conflate. Remediation:
replace "structurally not subject to" with "the precondition of the
cited theorems is not satisfied for DDA, which places DDA outside
the regime the theorems address". This is EC-8 language, and it is
cleaner.

**Finding 7.5 — MEDIUM — §7.7 "Honest failure modes" paragraph reads
as a disarming device.**
Using "honest" in a section heading is reviewer-bait. "Honest" in
academic writing invites the response "as opposed to dishonest
elsewhere?" Remediation: rename §7.7 to "Forensic findings: two
load-bearing bugs" or "Engineering findings from the case record".

### Section 8 — Limitations

**Finding 8.1 — MEDIUM — §8.L1 "if cross-encoder mean lands within
$\pm 0.10$ of the WavLM vector with Pearson $\geq 0.5$, the claim
promotes" — pre-declared decision rule.**
This is actually good — pre-registering the promotion criterion is
strong practice. But "Pearson $\geq 0.5$" is a weak bar. Hostile
reviewer: "Pearson 0.5 is not a high correlation; you are
pre-committing to promote on a low threshold". Remediation: raise
to Pearson $\geq 0.6$ OR state why 0.5 is the chosen bar (e.g.,
based on published ECAPA-TDNN vs WavLM agreement studies).

**Finding 8.2 — HIGH — §8.L3 asserts "Case 1's three-layer
enforcement stack passed 15/15 forensic-smoke verification on a prior
run".**
The 15/15 FSV battery is the only measured anchor for Case 1. But
§6.1.1 says this was a prior run, not the current cleaned-data
retrain. If the prior run passed 15/15 on buggy training data (see
§7.7 echo-bug), then passing 15/15 on buggy data is not evidence of
correctness — it is evidence that the FSV battery does not catch the
echo bug. Remediation: state explicitly that the 15/15 FSV are
integrity checks (not identity-quality metrics, as §6.1 says once)
AND that they were insufficient to catch the echo bug. The cleaned-
retrain FSV will be a re-integrity check, not a new form of
validation. This is already implicit but needs to be explicit.

**Finding 8.3 — HIGH — §8.L8 "Commercial conflict of interest" is a
limitation section.**
This is structurally odd. The commercial COI is not a limitation on
the **claims** — it is a disclosure of the author's affiliation that
should live in §Competing Interests. Placing it in Limitations reads
as an attempt to dilute the disclosure by spreading it across
sections. Remediation: move §8.L8 content to §Competing Interests
(not yet drafted); §8.L8 proper can become an actual technical
limitation (e.g., "L8. The systems described are commercial
artefacts and may evolve in ways this snapshot does not capture").

**Finding 8.4 — MEDIUM — §8 has 8 items; §8 final paragraph names
3 experiments with confidence deltas.**
The "EXP-1 / EXP-2 / EXP-3 confidence-delta" paragraph uses agent-34
Open-Philanthropy-style calibration language ("$\sim 0.98$
encoder-matched confidence to $\sim 0.90$ cross-encoder"). This is
technical jargon that a reviewer outside the agent pipeline will not
parse. Remediation: rewrite in plain language without the numeric
confidence deltas — e.g., "EXP-1 would promote the Case 3 headline
from encoder-matched identity agreement to cross-encoder identity
ranking if mean ECAPA $\geq$ 0.85 with Pearson $r \geq 0.6$;
otherwise the within-WavLM-family scope carries." See §6.8.

### Section 9 — Conclusion

**Finding 9.1 — LOW — §9 is tight and applies most softenings.**
Good.

**Finding 9.2 — MEDIUM — closing sentence "We have named a
configuration... that warrants independent replication and a pairwise
mutual-information audit on a neutral panel."**
"Neutral panel" is a slightly political choice of phrase — it implies
the author's panel is not neutral (which is true but should not be
said in a conclusion). Remediation: replace with "on an independent
embedder panel" or "on a different panel". Neutral sounds defensive.

---

## 3. Cross-cutting attacks

The 14 attacks below are the reviewer-level challenges that cut across
sections. Attacks CA-1 … CA-8 were named in the task brief. Attacks
CA-9 … CA-14 are invented while reading. For each I state the
challenge, the current-draft defense, residual risk, and
recommendation.

### CA-1 — Novelty: is DDA just multi-view retrieval rebranded?

**Challenge.** Any dual-encoder retrieval system (CLIP, ALIGN, DPR)
already produces multiple embeddings per input. Contrastive learning
with $N$ views is a well-known pattern (SimCLR, BYOL, MoCo). Count
the views and you have "$N$ signals per input". The "+$\binom{N}{2}$"
pairwise term looks like gloss.

**Draft defense.** §3.1 paragraph 5 ("not synthetic-data
generation... not data augmentation... not distillation"); §3.2
Table 1 and the anti-augmentation framing; §3.4 three-role
composition ("to our knowledge in the locked 88-citation pool... no
cited paper uses the same frozen embedder panel in all three roles
jointly, over identical parameter instances").

**Residual risk.** MEDIUM. The defense rests on the pool-relative
scope and on the three-role composition claim. A hostile reviewer
from the CLIP/DPR/SimCLR lineage will ask (a) why those are not in
the pool; (b) why "three simultaneous roles" is a contribution and
not a tautology (any frozen encoder can trivially be used as a
retriever, labeller, and guard; the only question is whether anyone
has done it).

**Recommendation.** (i) Add a bullet list in §3.4 saying "what no
cited paper does: (a) use identical parameter instances across all
three roles without adapter fine-tuning; (b) deploy the per-output
cosine guard at inference, not as post-hoc evaluation; (c) compose
multi-modal conjunction over $M \geq 2$ independently-trained
encoders." (ii) Accept pool-external counter-examples (2.2 fix) will
soften the claim if they materialise.

### CA-2 — Meaning compression taxonomy: fourth category or repackaged augmentation?

**Challenge.** "Meaning compression" is either (a) a relabeling of
multi-view representation learning, (b) a relabeling of data
augmentation, or (c) a term-of-art that the reviewer has not seen
before and has no reason to accept as a named category.

**Draft defense.** §3.2 Table 1 + the "we refrain from dimensionally
distinct" softening; §5.2 EXT of Delétang / Huang; agent-33 CA-5
triad (no new inputs; frozen encoders; multiple independent
encoders).

**Residual risk.** HIGH. The three-property triad (no new inputs;
frozen encoders; multiple independent encoders) is invisible in the
drafted §3.2 — it is in the agent-33 register but not stated
positively in §3.2. See Finding 3.3 above. The fourth-category
claim's entire non-triviality depends on this triad.

**Recommendation.** Pull the CA-5 triad directly into §3.2 as a
positive statement, not as a negative via §3.1 paragraph 5. Make the
triad a single typeset-set-apart paragraph.

### CA-3 — Per-output verifiability: what does "relative to a panel" buy beyond rejection sampling?

**Challenge.** "Per-output verifiability" reduces to: generate $K$
candidates, score each with a fixed function, keep the ones above a
threshold. This is rejection sampling with a fixed verifier. Any
generator + any fixed scorer + any threshold does this. Calling it
"per-output verifiability" is just renaming rejection sampling.

**Draft defense.** §5.4 paragraph on "what is verified / what is not
verified"; §7.3 four-axis contrast table (aim / instrument / metric /
runtime); §5.3 "the novelty of the guard's composition (multi-modal
conjunction + frozen target + per-output runtime)".

**Residual risk.** MEDIUM. Against a fair reviewer, the defense
holds: the three-property composition does distinguish TCT from
classical rejection sampling, which is typically $M = 1$ post-hoc
with a tuned scorer. Against a hostile reviewer, the defense depends
on the multi-modal conjunction at $M \geq 2$ being **measured**, and
it is not (Case 2 is projected).

**Recommendation.** (i) State clearly in §5.4 that at $M = 1$ the
guard reduces to classical verification and this is what Case 3
measures; (ii) state that the multi-modal conjunction at $M \geq 2$
is the non-classical claim and is **not yet measured**. This
honesty-on-the-face is the strongest defense against the "rejection
sampling rebranded" attack.

### CA-4 — Commercial COI exposure.

**Challenge.** Author runs Teleox.ai and Leapable.ai. Systems
described (Context Graph, ClipCannon) are the author's products.
§Competing Interests is not yet drafted. FM-01 regex pass is not yet
run. A hostile reviewer with the agent-33 CA-6 prior will look for
marketing language and — if present — reject as product promotion.

**Draft defense.** §4.3 paragraph ("described in acknowledgements as
the commercial exemplar"); §8.L8 Commercial COI limitation.

**Residual risk.** HIGH until §Competing Interests is drafted and
the FM-01 regex pass is run on the full body. See Finding 4.2 above.

**Recommendation.** (i) Draft §Competing Interests per agent-31
paste-ready (not yet done per handoff state). (ii) Run FM-01 regex
pass: specifically grep for "revolutionary", "game-changing",
"state-of-the-art", "best-in-class", "industry-leading", "scalable",
"enterprise-ready", "production-grade", "cutting-edge" across
§1–§9. (iii) Delete the Stripe Connect / 70-30 / 5% reserve /
HMAC sentence from §4.3 (Finding 4.2). (iv) Move §8.L8 content to
§Competing Interests.

### CA-5 — N=10 anchor sufficiency.

**Challenge.** One speaker, one language, one session, one encoder,
ten sentences. This is the entire empirical base of the paper. For
a theorem (Theorem 1), a paradigm (TCT as a third path), and a
taxonomy (fourth compression category). A reviewer will say: "the
empirical base is too thin to support this much theoretical weight."

**Draft defense.** §6.3.1 scope tag "MEASURED, N = 10 sentences, one
speaker, one encoder (WavLM-base-plus-sv)"; §7.1 explicit limitation
"What the anchor does not support is cross-encoder triangulation,
multi-speaker generalisation, cross-language generalisation, or the
multi-modal conjunction form... at $M > 1$"; §8.L1-L3 all disclose
this.

**Residual risk.** MEDIUM. The softenings are present. The
reviewer-level risk is whether the theoretical weight is
proportionate to the empirical base after softening. Theorem 1 is
constructive (no empirical content), so it survives; but the
sample-complexity "collapse" framing still reads as empirically
validated.

**Recommendation.** §1 and §7.6 both describe TCT + DDA as a
"third path". At N=10, the only defensible claim about a third path
is "the configuration is runnable and produces numbers consistent
with prediction". That is what the paper does say, but the word
"path" implies generality the empirical base does not support.
Consider replacing "third path" with "third configuration"
in-body (keep it in the title). Less grand; more defensible.

### CA-6 — Embedder independence — the paper ASSUMES it.

**Challenge.** The counting identity $N + \binom{N}{2} = 91$ at $N=13$
depends on Axiom 2 (approximate independence). Axiom 2 is
acknowledged as unmeasured (agent-33 CA-1). What is the pairwise
mutual information across the 13 embedders?

**Draft defense.** §3.1 "up to 91" softening everywhere; §7.2
paragraph on the $\binom{N}{2}$ term being the vulnerable component;
§8.L4 Remediation via EXP-2; §9 Conclusion names EXP-2 as the kill-
or-tighten instrument; Context Graph's "blind-spot" qualitative
evidence (§7.2 "sparse E6/E13 keyword recall, E9 HDC typo robustness,
E7 code-snippet ranking").

**Residual risk.** LOW if "up to 91" is preserved throughout.
MEDIUM if any occurrence drops the "up to" and states "91 signals".
Spot-check: §4.1 first paragraph says "up to 91 structured signals
per input"; §7.2 says "up to 91"; §9 says "up to 91 per input"; §3.1
Identity 1 gives the formula and the "up to" is in §3.1 paragraph 4.
Abstract says "up to 91 signals per input". All present. Maintain
discipline.

**Recommendation.** Hold the line. Add a sentence in §7.2 stating
"even if the pairwise term $\binom{N}{2}$ contracts to zero from
perfect redundancy, the per-embedder $N$ term is tight under
Axiom 1, so the lower bound is $N$ and the upper bound is $N + \binom
{N}{2}$. EXP-2 measures $N_{\text{eff}}$ in this interval."

### CA-7 — Case 2 entirely projected — acceptable?

**Challenge.** Case 2 has no T1 identity-lock number. Is it
publishable to include an entirely projected case as a "case study"?

**Draft defense.** §6.2.1 PROJECTED label; §8.L3 asymmetric release
posture; §3.3 and §7.1 always qualify the multi-modal claim; §6.2.2
forensic finding is the one measured contribution from Case 2 (the
flow-matching sign-bug).

**Residual risk.** MEDIUM. A reviewer may legitimately argue that
"Case 2" should be relabeled "Case 2 (architectural readiness,
forensic findings; end-to-end measurement pending)" and that calling
it a "case study" implies evidence it does not provide.

**Recommendation.** Consider renaming §6.2 to "Case 2 — Architectural
readiness and forensic record for an identity-locked talking-head
pipeline (measurement pending)". This telegraphs the status clearly.
Alternatively, retain "Case 2" but lead the section with a 50-word
"status" paragraph that says "this case reports architectural
readiness and a forensic audit; no end-to-end identity measurement
exists at the time of writing".

### CA-8 — Sample-complexity theorem: constructive bound is tautological. Does the paper flag this?

**Challenge.** Theorem 1 is "any pipeline with Phase 1/2/3 as defined
has subject-specific sample footprint $O(M \cdot k)$". This is a
definitional restatement — if the pipeline uses at most $M \cdot k$
subject-specific samples, then the pipeline uses at most $M \cdot k$
subject-specific samples. This is not a non-trivial theorem.

**Draft defense.** §7.5 explicitly says: "Theorem 1 is constructive.
Any pipeline with exactly the stated Phase 1 ... Phase 2 ... Phase
3 ... satisfies the bound by the definition of those phases. There
is no empirical question about the constructive side of the claim."
This is correct and honest.

**Residual risk.** LOW if the reader reads §7.5. MEDIUM if the
reader stops at §1's headline "a sample-complexity theorem
$\mathcal{T}_{\mathrm{TCT}}(P) = O(M \cdot k)$". §1 does not say
"constructive"; it just says "theorem". §5.4 and §7.5 do say
"constructive"; §9 Conclusion says "constructive bound".

**Recommendation.** §1 paragraph 5 must say "a constructive
sample-complexity theorem (Theorem 1) holds by the definition of the
TCT phases; its empirical content is Axiom 3, which is anchored at
one T1 operating point". This is agent-33 rec. 2 MUST — verify
implemented. (Spot-check: §1 paragraph 4 says "a *constructive*
sample-complexity bound $\mathcal{T}_{\mathrm{TCT}}(P) = O(M \cdot k)$
holds under the DDA/TCT axioms by construction, and its *empirical*
instantiation is supported at one T1 operating point". This is
implemented. Maintain.)

### CA-9 (invented) — The role-composition claim is unfalsifiable.

**Challenge.** §3.4 says "no cited paper uses the same frozen
embedder panel in all three roles jointly, over identical parameter
instances, without any parameter update between roles". This claim
is constructed with so many conjunctive qualifiers that it is
essentially unfalsifiable: any prior art that comes close will fail
on at least one qualifier (different parameters? then "not identical
instances". Some parameter update? then "not frozen". Post-hoc and
not during generation? then "not inference-time". Single modality?
then "not multi-modal"). A hostile reviewer will call this a
gerrymandered novelty claim.

**Residual risk.** HIGH. This is the single largest risk I see the
drafted manuscript carries that is NOT in the agent-33 register.

**Recommendation.** (i) Acknowledge in a sentence in §3.4 that the
composition is defined by its three-role conjunction, which is a
strong qualifier; (ii) state the simplest possible verifier: "if a
cited paper trivially satisfies any two of the three roles, the
contribution collapses to the third; if a cited paper satisfies all
three, the contribution is the specific production instantiation";
(iii) name the CLIP / DPR / ColBERT / FaceNet literature as "partial
precedents" at $M = 1$ or at post-hoc evaluation, and state that the
joint specification is what is absent.

### CA-10 (invented) — The "frozen" invariant is a methodology discipline, not a measurable property.

**Challenge.** V18 "frozen" = "param.requires_grad == False on every
$\phi_m$". §5.3 has three invariant checks. But a hostile reviewer
will ask: how do you verify at review time that your reported runs
actually had frozen embedders? The code is not released for Cases 2
and 3; the weight hash check is internal; the reader cannot verify.

**Residual risk.** MEDIUM. The reviewer will assume good faith if
the disclosure is present, but absence of code for Case 2 and Case 3
means the "frozen" property is asserted not demonstrated.

**Recommendation.** (i) §5.3 Freeze policy should state that every
run's frozen-check log is part of the provenance record; (ii) for
Case 3 (measured), the frozen-check log should be included in the
companion artefact release; (iii) §8 should carry a limitation
stating "the frozen invariant is enforced at runtime by the pipeline
and logged; independent verification requires code release".

### CA-11 (invented) — The FM-01 regex pass has not been performed and the body contains marketing-adjacent phrasing.

**Challenge.** I performed a quick grep-by-eye across §§1–9 for
marketing language. Findings:

- §4.1: "The thirteen embedders span 11,008 dense dimensions..."
  (list of products, reads as spec sheet).
- §4.2: "a **23-stage analysis directed acyclic graph** over source
  video" (number-as-hero phrasing).
- §1 paragraph 4: "a roughly 370,000-line Rust workspace" and "a
  roughly 67,585-line Python package" — line counts are not
  contributions and read as scope-inflation.
- §4.2: "**58 MCP tools** covering ingest, analysis, editing,
  rendering, voice cloning, and avatar lip-sync" — feature list.
- §4.3: Leapable.ai Stripe Connect sentence (Finding 4.2).

None of these is "revolutionary" or "game-changing", but collectively
they carry a product-catalogue cadence that a reviewer will notice.

**Residual risk.** MEDIUM — conditional on FM-01 regex pass.

**Recommendation.** (i) Run FM-01 regex pass per Finding 4.2
recommendation. (ii) Specifically: demote line counts to
"approximately 370K" in §1; move "58 MCP tools covering..." to an
appendix; delete the Leapable.ai revenue-split sentence.

### CA-12 (invented) — The paper cites its own upstream agents implicitly (W1, W2, IC-3, etc.).

**Challenge.** Throughout §6, §7, §8, phrases like "VT-I3 / VT-C1 /
VT-S4; W1", "(IC-3)", "(CA-2 with multi-modal scope tightening)",
"C12a", "agent-34's Open-Philanthropy-style calibration" appear.
These are internal SMAP identifiers. A reader who has not read the
agent registers cannot parse them. A hostile reviewer will read this
as "the authors have a private vocabulary and are speaking to
themselves."

**Residual risk.** HIGH — reader-accessibility.

**Recommendation.** Before submission, strip ALL `W-N`, `VT-N`,
`IC-N`, `EC-N`, `CA-N`, `C-N`, `H-N`, `ISO-N`, `CL-N`, `FM-N` tags
from §1 through §9 body text. These belong in the SMAP memory, not
the manuscript. Replace with a parenthetical descriptor — e.g.,
"(within-WavLM-family scope; §8.L1)" or "(see §8 Limitations)".
Spot-checked occurrences:

- §6.3.2: "Mandatory scope disclosure (VT-I3 / VT-C1 / VT-S4; W1)."
  → "Mandatory scope disclosure (see §8.L1)."
- §6.3.5: "(W1 remediation)", "(W2 remediation; VT-I2)",
  "(W3 remediation; VT-T3)".
- §6.3.5: "(C1; CA-1)" → "(see §7.2; §8.L4)".
- §7.1: "(P3 of the framework; Theorem 2 of §5.4; C5 confidence
  tier)." → "(Theorem 2; §5.4)."
- §7.5: "(C12a, $\geq 95\%$ confidence)" → delete the confidence
  percentage; the hedged language "holds under the axioms" is
  sufficient.
- §7.5: "(C12b)" → delete.
- §7.5: "(W6)" → "(see §8.L4)".
- §7.7: "(W9 scope)" → delete.
- §8 final paragraph: the entire "$\sim 0.98$ encoder-matched
  confidence", "$\sim 0.90$", "$\sim 0.40$" numbers come from
  agent-34 and are not citeable — delete (Finding 8.4).

This is a stop-ship issue (see §4 below).

### CA-13 (invented) — The abstract promises what the paper doesn't quite deliver.

**Challenge.** The abstract says "Two production systems instantiate
the substrate: Context Graph... and ClipCannon...". This is true
but the rhetorical weight of "instantiate the substrate" is high;
systems are described in §4 but not used as experimental apparatus
against each other. A reviewer will ask: "you described two systems;
you ran three case studies; why two systems (not one or three)?
What do the two systems jointly demonstrate?" The answer is "§4.3
how the two systems instantiate the framework" — but §4.3 is short
and thin (one paragraph).

**Residual risk.** LOW. The systems are named as constructive
evidence, not as empirical apparatus against each other.

**Recommendation.** Abstract line "Two production systems instantiate
the substrate" — replace with "Two production systems provide
constructive evidence that the DDA labeller role runs at production
scale: Context Graph... and ClipCannon...". Small change, large
rhetorical effect.

### CA-14 (invented) — "The camera-ready version is expected to carry these" is a structural weakness.

**Challenge.** The manuscript promises camera-ready updates three
places: §6.3.5 three pending measurements; §8 final paragraph three
EXP; §9 conclusion EXP-1/2/3. But arXiv v1 papers do not have
"camera-ready versions" — arXiv is not a venue with a camera-ready
deadline. The phrase "camera-ready will re-verify" is venue-coded
for a conference submission (NeurIPS, ICLR, ICML) that has been
removed and reframed as arXiv. A hostile reviewer will ask:
"if you are submitting to arXiv, what is the camera-ready?"

**Residual risk.** LOW technical; MEDIUM presentational.

**Recommendation.** Replace "camera-ready will re-verify" with "a
subsequent version of this manuscript will re-verify" throughout.
This is a simple global replace (the phrase is used consistently).
Spot-check: §1 paragraph 4 "the cleaned retrain is in progress...
camera-ready will re-verify" — replace. §6.1 / §6.2 / §6.3 /
§6.4 Table 3 — replace. The title of §6 can keep the phrase as an
internal term of art; §8 L3 Remediation can use the phrase once
before redefining it.

---

## 4. Severity summary

Tallying across §2 section-by-section findings and §3 cross-cutting
attacks:

| Severity | Count | Items |
|---|---|---|
| **CRITICAL** | **3** | 5.1 (guard scope placement); 6.1 (0.961 headline bold asymmetry); 6.2 (T=0.3 sweep vs. headline mismatch) |
| **HIGH** | **14** | 1.2, 1.3, 2.3, 3.1, 3.2, 4.1, 4.2, 5.2, 5.3, 5.4, 6.3, 6.4, 6.5, 6.7; 8.2, 8.3; CA-2, CA-4, CA-9, CA-12 (CA-12 is stop-ship tag too) |
| **MEDIUM** | **19** | 1.1, 1.4, 1.5, 2.2, 2.4, 2.5, 3.3, 3.4, 4.3, 5.5, 6.6, 7.2, 7.3, 7.4, 7.5, 8.1, 8.4, 9.2; CA-1, CA-3, CA-5, CA-7, CA-10, CA-11, CA-13, CA-14 |
| **LOW** | **3** | 2.1, 7.1, 9.1 |

Recount deduplicating overlap between section-level and cross-cutting
items (CA-12 appears as both CA-12 and as a stop-ship concern; CA-11
overlaps with 4.1, 4.2; CA-4 overlaps with 4.2):

- **CRITICAL (stop-ship): 3**
- **HIGH (require fix before submission): 14**
- **MEDIUM (should fix, submittable if time-constrained): 19**
- **LOW (cosmetic): 3**

---

## 5. Stop-ship list (must fix before arXiv v1 submission)

The three CRITICAL findings and five of the HIGH findings are
submission blockers. Everything else can be accepted as a risk if
the review cycle is time-constrained, but the eight below cannot:

1. **STOP-1 — Finding 5.1 — Fold "relative to $\Phi$ and $C_P$"
   scope into the predicate paragraph itself (§5.3 / §5.4).**
   The verifiability claim's scope must be in the same paragraph as
   the predicate definition. Currently it is one paragraph below.
2. **STOP-2 — Finding 6.1 — Fix the bolded-headline asymmetry in
   §6.3.2.** The encoder-matched scope must be bolded in the same
   typographical unit as "0.961 mean WavLM SECS".
3. **STOP-3 — Finding 6.2 — Explain the T=0.3 sweep mean (0.932)
   vs headline mean (0.961) discrepancy in §6.3.3.** Without a
   one-sentence clarifier, a reviewer will read selective reporting.
4. **STOP-4 — Finding 4.2 + CA-4 — Delete the Stripe Connect / 70-30
   / 5% reserve / HMAC sentence from §4.3 body, move to
   §Acknowledgements / §Competing Interests.** Commercial-terms
   sentence in body triggers FM-01.
5. **STOP-5 — CA-12 — Strip all internal SMAP identifiers (W-N,
   VT-N, IC-N, EC-N, CA-N, C-N, H-N) from §1 through §9 body.**
   Replace with section references. Reader cannot parse these.
6. **STOP-6 — Finding 8.3 + CA-4 — Move §8.L8 commercial-COI content
   to §Competing Interests (draft required).** Placing COI in
   Limitations dilutes the disclosure.
7. **STOP-7 — Finding 6.4 — Add a justification sentence for EACH of
   the three pending companion measurements.** "Pending" without
   justification reads as deferred work on instruments that exist.
8. **STOP-8 — Finding 6.6 + CA-14 — Delete "Track A / Track B"
   language from §8.L3; replace "camera-ready will re-verify"
   globally with "a subsequent version of this manuscript will
   re-verify".** These are author-memo artefacts leaking into
   publication.

The eight stop-ship fixes together are ~2–3 hours of editing and do
not require new measurement runs. They are the minimum viable bar
for arXiv v1.

---

## 6. The worst cross-cutting attack

Of the fourteen cross-cutting attacks, the worst is **CA-12 (internal
SMAP identifiers leaking into the manuscript)**. This is worst
because:

- It is the most easily detected by a reviewer (simple text scan).
- It is the most damaging rhetorically (the paper reads as a draft
  rather than a finished artefact).
- It affects every other attack's defense (if the reader cannot
  parse the hedges because they are tagged `W1` / `VT-I3`, the
  hedges cannot do their defensive work).
- It is the cheapest to fix (~1 hour of search-and-replace).

CA-12 is the stop-ship that would ALSO fix reviewer first-impression
risk across CA-2, CA-3, CA-5, CA-7, CA-11.

The second-worst is **CA-4 (commercial COI exposure)**, because
§Competing Interests is not yet drafted and FM-01 regex is not yet
run. CA-4 is the one that would trigger an arXiv-moderator-level
rejection (as opposed to a reviewer-level request-for-revision). The
fix is also straightforward (~2 hours) but requires the agent-31
paste-ready §Competing Interests to be actually inserted into the
document — which has not happened yet.

---

## 7. Confidence

**HIGH** that the eight stop-ship items, if fixed, move the paper
from "arXiv v1 will draw targeted reviewer challenges" to "arXiv v1
will stand on its own merits with the known limitations".

**HIGH** that without the stop-ship fixes, a hostile review will
find **at least three** of the stop-ship items within the first 20
minutes of reading.

**MEDIUM** that the ~19 MEDIUM findings need to land for a strong
submission, but none individually is a blocker.

**HIGH** that the paper's underlying claims are defensible; the risk
is presentation, not substance. Every claim that landed HIGH or
CRITICAL in this review is a presentation issue (scope placement,
bold asymmetry, internal identifier leakage, FM-01-adjacent
commercial phrasing) rather than a substantive claim flaw. The
agent-32 contradiction register and agent-33 counterargument register
caught the substantive issues upstream.

**MEDIUM** on my ability to catch everything. I am one agent reading
a large manuscript once; a reviewer who has read similar papers for
years will catch things I missed. Specifically: I did not check
citation accuracy (agent-44 downstream), I did not check
reproducibility in the strict sense (agent-43 downstream), and I did
not check formatting / arXiv template compliance.

## 8. Changelog

- 2026-04-17 — Initial adversarial review by agent-42.
