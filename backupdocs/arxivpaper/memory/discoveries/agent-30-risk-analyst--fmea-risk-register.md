---
namespace: discoveries
agent_id: agent-30-risk-analyst
timestamp: 2026-04-16T19:00:00Z
status: active
locked: false
supersedes: none
depends_on:
  - memory/context/mission-brief.md
  - memory/decisions/agent-01-step-back-analyzer--failure-modes-to-avoid.md
  - memory/discoveries/agent-29-validity-guardian--validity-threats.md
  - memory/patterns/agent-29-validity-guardian--mitigation-plan.md
  - memory/handoff/agent-29-to-30--validity-ready.md
downstream_consumers:
  - agent-31-ethics-reviewer
  - agent-37-methodology-writer
  - agent-40-limitations-writer
  - agent-42-adversarial-reviewer
  - agent-47-submission-packager
tags: fmea, risk, submission-program, arxiv, reviewer-rejection, reproducibility, reception, vendor, timing
answers_questions: [Q07, Q12, Q16]
---

# FMEA Risk Register — arXiv Submission Program

## 0. Scope

This FMEA audits **external risk to reception and impact of the arXiv
submission itself**, not internal scientific validity. Internal validity
is handled by agent-29's VT-* register. Agent-30's frame is
**"what could go wrong between manuscript completion and community
acceptance"**, decomposed into five risk categories:

- **R1 Reviewer rejection**: arXiv moderator miscategorisation, cs.LG
  routing, post-arXiv peer-review rejection, overclaim perception,
  closed-artifact case-study criticism.
- **R2 Empirical / reproducibility**: Cases 1 & 2 T1 measurements don't
  land; W1 cross-encoder returns < 0.95; W2 LoRA/TCT-delta is small;
  third-party replication fails due to WavLM version drift.
- **R3 Community reception**: constellation / meaning-compression /
  DDA / TCT jargon rejected as rebrands of existing concepts.
- **R4 Vendor / legal / ethics**: closed-artifact encoder drift,
  identity-clone subject consent, commercial-affiliation objectivity.
- **R5 Timing / operational**: submission-window slip, endorsement
  delay, pre-submission leak.

## 1. FMEA scoring convention

- **Severity (S)** 1–5: 1 = cosmetic; 2 = minor reception drag;
  3 = non-trivial reception harm / partial retraction risk;
  4 = desk-reject / headline invalidation; 5 = reputational or legal
  catastrophe.
- **Likelihood (L)** 1–5: 1 = very unlikely (<5%); 2 = plausible
  (5–20%); 3 = expected in a typical paper (20–50%); 4 = likely
  (50–80%); 5 = near-certain (>80%).
- **RPN = S × L** (max 25). **Priority bands**: RPN ≥ 15 CRITICAL;
  10–14 HIGH; 6–9 MEDIUM; 1–5 LOW.
- **Residual**: S × L after the mitigation in the row "Mitigation plan"
  is applied.

---

## 2. Risk Register (23 entries)

### R1 — Reviewer rejection

---

#### R1-01 — arXiv moderator miscategorises / holds the submission

**Failure mode**: The arXiv cs.LG moderator flags the submission for
review and either (a) reroutes to cs.CL / cs.CV primary (delaying
appearance) or (b) places it on-hold pending endorser / clarification
because the TCT / DDA vocabulary is unfamiliar.

**Potential cause**: Novel vocabulary in the title ("teleological
constellation", "meaning compression") reads as non-standard; abstract
mentions commercial affiliations (Teleox.ai, Leapable.ai); paper cites
closed artifacts (Qwen3-TTS proprietary weights).

**Effect on submission**: 2–10 business day delay; possible forced
retitling.

**Severity**: **3** (delays impact; not fatal).
**Likelihood**: **3** (novel-vocabulary papers commonly held).
**RPN**: **9** (MEDIUM).

**Current controls**: Mission-brief §Hard constraints forbids vendor
prose in body; agent-01 FM-01 regex pass; acknowledgement-only vendor
mention.

**Mitigation owner**: agent-47 (submission-packager) + agent-48
(final-reviewer).

**Mitigation plan**:
1. Pre-screen abstract + title against cs.LG recent submissions for
   tone match.
2. First sentence of abstract defines DDA and TCT in standard ML
   vocabulary ("frozen-encoder decomposition", "geometric-proximity
   guard") before introducing coined terms.
3. Keep cs.LG primary, cs.CL + cs.CV cross-list (three-way coverage
   makes miscategorisation less likely to be fatal).
4. Have an arXiv endorser warm-contacted >= 1 week before submission.
5. If held, paste-ready response: 1-paragraph plain-ML restatement
   plus list of load-bearing references from `_evidence_*.md`.

**Residual RPN**: **4** (LOW-MEDIUM).

---

#### R1-02 — Post-arXiv peer reviewer desk-reject for "marketing paper"

**Failure mode**: A reviewer at a downstream venue (NeurIPS / ICML /
ICLR) reads the manuscript as a product pitch for Teleox.ai / Leapable.ai
and desk-rejects on presentation grounds.

**Potential cause**: Agent-01 FM-01 slippage (marketing creep) not
caught by adversarial-reviewer; commercial affiliations named.

**Severity**: **4** (desk-reject at target venue).
**Likelihood**: **2** (FM-01 is explicitly policed by agent-42).
**RPN**: **8** (MEDIUM).

**Mitigation owner**: agent-42 (adversarial-reviewer).

**Mitigation plan**:
1. Agent-42 regex pass on superlatives, "revolutionary", "breakthrough",
   "groundbreaking", "our solution", vendor names outside acknowledgements.
2. Third-person neutral voice enforced by agent-39 (discussion-writer).
3. Affiliations in footnote + Acknowledgements only.
4. Body contains zero product names; one-sentence data-availability
   note references Context Graph / ClipCannon as open research
   artefacts, not vendor products.

**Residual RPN**: **2** (LOW).

---

#### R1-03 — Reviewer attacks single-subject (N=1) design as insufficient

**Failure mode**: Peer reviewer rejects on "N=1 cannot support any
generalisation claim; the paper makes claims that its design does not
support."

**Potential cause**: agent-29 VT-E1 (UNCLOSEABLE). Case 3 is
single-speaker; Cases 1 & 2 are single-pipeline single-corpus /
single-subject respectively.

**Severity**: **4** (headline invalidation if not framed defensively).
**Likelihood**: **4** (expected reviewer probe).
**RPN**: **16** (CRITICAL).

**Mitigation owner**: agent-38 (results-writer) + agent-40
(limitations-writer) + agent-42.

**Mitigation plan**:
1. §6 case-study openers carry agent-01 FM-05 paste-ready protocol
   paragraph: "Case 3 is a single-speaker exploratory demonstration;
   no generalisation beyond this protocol is claimed."
2. §8 L-Population paragraph (agent-29 §4 paste-ready, Bias-05) is
   mandatory and un-cuttable.
3. §9 Conclusion scrubbed of all cross-subject / cross-language /
   cross-corpus generalisation claims (agent-42 must-catch item 6).
4. Title does **not** claim generality ("a three-case exploratory
   demonstration" is preferable to "a general framework").
5. Framing: TCT + DDA are **methodology contributions** with N=1
   illustration; the framework is general, the evidence is
   exploratory.

**Residual RPN**: **6** (MEDIUM; residual because N=1 cannot be
closed under current scope).

---

#### R1-04 — Reviewer rejects Case 3 headline as circular (WavLM-matched)

**Failure mode**: Reviewer catches VT-I3 / VT-C1 / VT-S4 (encoder-matched
evaluation) and rejects the 0.961 as "the guard scoring itself high by
construction".

**Potential cause**: agent-29 VT-I3 (CRITICAL, CONFIRMED) + VT-C1
(CRITICAL, CONFIRMED). W1 cross-encoder run PENDING.

**Severity**: **4** (invalidates Case 3 headline framing if not
disclosed).
**Likelihood**: **4** (every competent reviewer catches encoder-matched
evaluation on first read).
**RPN**: **16** (CRITICAL).

**Mitigation owner**: agent-38 + agent-40.

**Mitigation plan**:
1. §6.3 paste-ready VT-I3 single sentence verbatim ("the 0.961 is an
   encoder-matched measurement...guard behaves as specified, not
   cross-encoder identity verification").
2. §8 L-01 paste-ready paragraph.
3. Primary remediation: **W1 run** (ECAPA-TDNN + Resemblyzer on the 10
   argmax candidates) ideally lands pre-submission. Track A disclosure
   language carries if it lands; Track B disclosure if not.
4. Reframe H2a from "cross-encoder identity verification" to
   "within-WavLM-family identity agreement, 9/10 >= 0.95".
5. §5.4 attribution softened from "TCT causes identity-lock" to "TCT
   verifies and selectively accepts".

**Residual RPN (Track A — W1 lands, cross-encoder mean >= 0.95)**:
**4** (LOW, after disclosure + successful triangulation).

**Residual RPN (Track B — W1 does not land)**: **9** (MEDIUM, after
disclosure only; the number survives as "within-family" rather than
"cross-encoder").

---

#### R1-05 — Reviewer rejects closed-artifact case studies as irreproducible

**Failure mode**: Reviewer rejects because Qwen3-TTS is proprietary, the
Santa subject data is private (identity-clone consent), and the Shakespeare
LoRA weights are not released; no reader can re-run the three headline
cases.

**Potential cause**: Architectural necessity. Case 2's subject data is
single-subject identity-clone material (privacy-locked); Case 3's Qwen3-TTS
base is a vendor model.

**Severity**: **3** (weakens reproducibility claim; not fatal at
exploratory-demonstration venue).
**Likelihood**: **3** (reviewer will flag; not certain to kill).
**RPN**: **9** (MEDIUM).

**Mitigation owner**: agent-43 (reproducibility-checker) + agent-47.

**Mitigation plan**:
1. Release the **pipeline code** and **protocols** for all three cases
   (Context Graph repo, ClipCannon repo, Shakespeare LoRA stack)
   even when the data / vendor weights are closed.
2. Release agent-27 §H JSON manifest schema, weight-hash pinning,
   reference-set hashes — third party can substitute their own
   subject data and re-run.
3. §6 openers explicitly state: "Case 2 subject data is private; Case
   3 uses a proprietary base model (Qwen3-TTS, vendor-pinned hash).
   Reproducibility is protocol-level, not byte-identical for Cases
   2 & 3."
4. Provide **one fully-open demonstration** (Shakespeare LoRA Case 1
   stack on a public corpus) as the reproducibility flagship.
5. §8 L-Reproducibility paragraph.

**Residual RPN**: **4** (LOW-MEDIUM; exploratory venues accept closed
subject data with protocol release).

---

#### R1-06 — Reviewer attacks "meaning compression as fourth category" as rebranding

**Failure mode**: Reviewer claims meaning compression is just
information-density / representation-learning under a new name and
rejects the taxonomic claim as not a contribution.

**Potential cause**: agent-29 VT-C7 (dimensional-claim softened to
semantic-claim; MITIGATED by language but reception risk remains).

**Severity**: **3** (undermines one of four core claims; paper still
stands on the other three).
**Likelihood**: **3** (expected objection).
**RPN**: **9** (MEDIUM).

**Mitigation owner**: agent-21 (theory-builder) + agent-36
(introduction-writer) + agent-42.

**Mitigation plan**:
1. Agent-29 VT-C7 language rewrite: "semantically distinct; measured as
   structured-signal density (signals per raw-data-unit) rather than
   Shannon bits".
2. §3.1 positions meaning compression **against** specific prior work
   (TurboQuant, GPTQ, AWQ as bit/weight; activation compression) —
   not against "compression generally".
3. §5.2 derivation: bit-compression preserves signal content;
   meaning-compression increases signal count per input. Use the
   one-line formula to make the distinction concrete.
4. Frame as **measurement unit clarification**, not as a new
   compression algorithm (which would invite "where is your codec").
5. Acknowledge rebrand objection preemptively in §7 Discussion:
   "we do not claim a new compression algorithm; we claim a
   new counting convention orthogonal to Shannon-bit accounting".

**Residual RPN**: **6** (MEDIUM; the claim is philosophically
debatable regardless of presentation).

---

### R2 — Empirical / reproducibility

---

#### R2-01 — Case 1 (Shakespeare) T1 numbers don't land by submission

**Failure mode**: Shakespeare LoRA SFT+DPO run does not complete by
submission window; §6.1 remains T3 PROJECTED.

**Potential cause**: SFT+DPO training time, compute contention, residual
bugs found during training, data-cleaning delays.

**Severity**: **3** (one of three cases drops to projected-only; §6
drops one case; still survivable with two measured cases).
**Likelihood**: **4** (agent-28 handoff flags T1 PENDING for Case 1;
timeline-bounded).
**RPN**: **12** (HIGH).

**Mitigation owner**: principal author (Chris Royse) + agent-38 +
agent-40.

**Mitigation plan**:
1. Case 1 §6.1 drafted two-track: Track A (measured — 3-sig-fig tables);
   Track B (architecturally complete, numbers PENDING, 15/15 FSV-passed
   but no metric-report).
2. If Track B, §1 opener reframes submission as "two measured cases
   (Case 3 + Case 2) plus one architecturally-complete case (Case 1)".
3. Agent-01 FM-04 discipline: every PROJECTED number carries T3 tag.
4. Parallel effort: prioritise Case 2 T1 and Case 1 metric-report
   (WavLM-style-cosine on FSV-battery outputs) as the
   measurement-critical path.
5. Submission can proceed at v1 with Case 1 at T3; v2 upgrades to
   Case 1 at T1.

**Residual RPN**: **6** (MEDIUM; two-track drafting absorbs the
shortfall).

---

#### R2-02 — Case 2 (Santa) T1 numbers don't land by submission

**Failure mode**: Santa identity-clone pipeline does not produce end-to-end
visual / emotion / speaker / voice conjunction accept-rate numbers.

**Potential cause**: EchoMimicV3 + LoRA + forensic sign-fix integration
work; ClipCannon 23-stage DAG tuning; scene-chunking evaluation
stability.

**Severity**: **4** (Case 2 is the only M > 1 case; losing it collapses
the multi-modal strictness evidence to derivation only — VT-T5).
**Likelihood**: **4** (agent-28 handoff flags T1 PENDING for Case 2;
most integration work remains).
**RPN**: **16** (CRITICAL).

**Mitigation owner**: principal author + agent-38 + agent-40.

**Mitigation plan**:
1. Case 2 §6.2 drafted two-track per agent-28 §3.2.
2. §5.4 multi-modal strictness framed as **derivational under Axiom 2**,
   not as empirically validated at M > 1 — agent-29 VT-T5 disclosure.
3. §8 L-05 paste-ready paragraph: "multi-modal strictness is
   derivational; Case 2 end-to-end T1 is the critical pending
   measurement".
4. Protect Case 3 headline as the **measurement flagship**; Case 2 is
   framed as "architecturally complete demonstration pending end-to-end
   metric-report" regardless of T1 landing.
5. Prioritise at least one of the four modality-level numbers
   (visual/emotion/speaker/voice) landing T1 so §6.2 has a real
   per-modality table row.

**Residual RPN**: **9** (MEDIUM-HIGH; worst case is Case 2 remains T3
and §8 L-05 absorbs the cost. HIGH because multi-modal is the paper's
distinct theoretical payload.)

---

#### R2-03 — W1 cross-encoder returns < 0.95 mean

**Failure mode**: ECAPA-TDNN + Resemblyzer re-scoring of the 10 argmax
candidates returns mean SECS substantially below 0.95 (e.g. 0.85–0.90),
indicating WavLM-family identity agreement but not cross-encoder
triangulation at the 0.95 bar.

**Potential cause**: Cross-encoder calibration differences
(VoxCeleb-trained ECAPA has systematic offset from WavLM-SV); not a
design failure but a measurement outcome.

**Severity**: **3** (reframes the claim to "within-WavLM-family"
rather than "cross-encoder", which is weaker but honest and
scientifically acceptable).
**Likelihood**: **3** (plausible — ECAPA and WavLM have different
training distributions; ~0.05–0.10 offset is common in the literature).
**RPN**: **9** (MEDIUM).

**Mitigation owner**: agent-38 + agent-40.

**Mitigation plan**:
1. Pre-register the **agreement metric** (Pearson + Spearman +
   Bland-Altman), not a threshold-cross on the raw number. A strong
   agreement signal with a ~0.05 offset is a win; a low-agreement
   signal is the real failure condition.
2. §6.3 table presents both raw numbers and agreement statistics;
   agreement within ±0.10 is the "pass" threshold for the
   triangulation claim.
3. If cross-encoder mean < 0.90 and agreement is weak (Pearson < 0.5),
   §6.3 reframes to "within-WavLM-family demonstration; cross-encoder
   triangulation inconclusive; named future work".
4. §8 L-01 Track B disclosure handles this gracefully without
   headline loss.
5. Do not rerun or cherry-pick — the number is what it is; present
   honestly.

**Residual RPN**: **4** (LOW; the design absorbs this outcome).

---

#### R2-04 — W2 pre-selection histogram reveals TCT-delta is small

**Failure mode**: Pre-selection mean over 120 candidates is already
~0.95+, so TCT-delta Δ_TCT = post − pre is small (< 0.02) and the
"TCT adds significant value over base ICL pipeline" narrative weakens.

**Potential cause**: Qwen3-TTS + 50-clip centroid + Full ICL is already
a very strong identity-transfer pipeline; best-of-12 under WavLM guard
moves the mean only modestly.

**Severity**: **3** (recasts TCT's role from "identity-lock causation"
to "verification + selection", which is a weaker but honest framing).
**Likelihood**: **3** (the headline 0.961 is high enough that pre-selection
is likely also high; small Δ_TCT is plausible).
**RPN**: **9** (MEDIUM).

**Mitigation owner**: agent-38 + agent-40 + agent-21.

**Mitigation plan**:
1. Agent-29 VT-I2 + agent-24 W6: §5.4 language already softened from
   "TCT causes" to "TCT operationalises / verifies / selectively
   accepts". This pre-absorbs a small Δ_TCT.
2. §5 reframes TCT's contribution as **three-part** (not just
   best-of-K): (a) frozen-centroid design, (b) per-output verifiable
   predicate, (c) selection operator under the predicate. Even if
   (c)'s marginal Δ is small, (a) and (b) remain load-bearing.
3. §6.3 reports **both** the base-pipeline mean (pre-selection) and
   TCT-augmented mean (post-selection) as descriptive; does **not**
   claim TCT-delta is the headline.
4. Pre-selection histogram tail (fraction of candidates below 0.95)
   is the reframed interest: what fraction does the guard filter
   out? Even a small Δ_mean can correspond to a high filter-rate
   on the lower tail.
5. If Δ_TCT ≈ 0, §7 Discussion pivots: "TCT's contribution is
   **verification and acceptance**, not **mean-score inflation** —
   the per-output predicate is the load-bearing claim".

**Residual RPN**: **6** (MEDIUM; the design has pre-mitigated this
via §5.4 language).

---

#### R2-05 — Third-party replication fails due to WavLM version drift

**Failure mode**: 6–12 months after submission, a third-party tries to
reproduce the 0.961 headline using the protocol. WavLM weights on
Hugging Face have been updated (minor version bump, normalisation
change, or repo-rename); the re-measured number is systematically
different (e.g. 0.92 or 0.97).

**Potential cause**: agent-29 VT-E5 (vendor-versioned scorers). All six
vendor scorers (Qwen3-TTS, WavLM, ECAPA, SigLIP, DNSMOS, UTMOS) are
moving targets.

**Severity**: **3** (undermines specific-number reproducibility but
not protocol-level reproducibility; manuscript already flags this).
**Likelihood**: **4** (vendor model-weight drift is common over 12+
months).
**RPN**: **12** (HIGH).

**Mitigation owner**: agent-27 (instrument-developer) + agent-43
(reproducibility-checker) + agent-47.

**Mitigation plan**:
1. Agent-27 §H manifest schema is mandatory: every T1 number ships
   with `embedder_version_hash` (SHA-256 of weight file), pipeline
   version, measurement date.
2. Release a **weight-pin script**: given the manifest, fetch the exact
   pinned artefacts from the vendor / archive; if vendor has
   deprecated, fall back to archived weights on Zenodo (commit
   SHA-256'd weights to a preservation bucket).
3. §6 methodology paragraph: "all measurements anchored to measurement
   date 2026-04-16; encoder weight SHA-256 recorded in the data
   supplement; reproducibility is byte-identical against pinned
   weights".
4. Archive the specific WavLM-base-plus-sv, ECAPA-TDNN, SigLIP,
   DNSMOS, UTMOS weight files to a preservation URL (Zenodo,
   Internet Archive) with permanent DOI.
5. §8 Reproducibility limitation paragraph preempts the objection.

**Residual RPN**: **4** (LOW; weight-hash + preservation closes this).

---

#### R2-06 — Reviewer demands pairwise-MI measurement to validate DDA count

**Failure mode**: Reviewer demands empirical pairwise-MI on 13
embedders to justify the "91 signals per input" claim; absence
reduces DDA count to "theoretical upper bound".

**Potential cause**: agent-29 VT-C6 / VT-T1 (CONFIRMED; RO-8 future
work).

**Severity**: **2** (language-tightening to "up to 91 under Axiom 2"
already absorbs this; not headline-fatal).
**Likelihood**: **3** (likely reviewer probe for theoretical paper).
**RPN**: **6** (MEDIUM).

**Mitigation owner**: agent-36 + agent-40 + agent-42.

**Mitigation plan**:
1. Agent-29 Rank-1 rewrite already mandates "up to 91 signals per
   input (N + C(N,2) under pairwise-independence Axiom 2)" language.
2. §3.4 explicitly names the assumption: "orthogonal signal count is
   an upper bound contingent on pairwise embedder independence".
3. §8 L-DDA-Independence paragraph preempts.
4. §10 Future Work RO-8 named explicitly: "empirical pairwise-MI
   measurement on Context Graph's 13-embedder panel".
5. If a reviewer escalates, paste-ready response: the 91 is the
   theoretical headline; empirical redundancy is an open measurement
   question; any redundancy reduces the effective count but the
   structural argument (DDA is counting orthogonal supervisory axes,
   not synthesizing new inputs) holds regardless.

**Residual RPN**: **3** (LOW; language already softened).

---

### R3 — Community reception

---

#### R3-01 — "Constellation" / "centroid" / "anchor" vocabulary rejected as jargon

**Failure mode**: Reviewers / Twitter / r/MachineLearning ridicule
"teleological constellation training" as marketing-speak or astrology,
reducing citation potential.

**Potential cause**: Agent-01 FM-09 (centroid vocabulary drift)
partly addressed by agent-04 canonical definition; "teleological" is
philosophical jargon with no ML precedent.

**Severity**: **2** (citation drag; not rejection).
**Likelihood**: **4** (novel-vocabulary papers attract ridicule).
**RPN**: **8** (MEDIUM).

**Mitigation owner**: agent-04 (construct-definer) + agent-36
(introduction-writer) + agent-42.

**Mitigation plan**:
1. §1 Introduction defines "teleological" in standard ML terms
   immediately: "goal-directed, outcome-verified training — in
   contrast to process-based supervision like next-token loss or
   scalar-reward optimisation".
2. §4 construct definitions: **centroid** = vector per modality;
   **constellation** = set of centroids across modalities;
   **reference set** = raw clips from which centroid is computed
   (agent-01 FM-09 enforcement).
3. Abstract leads with the mechanism ("frozen-encoder geometric
   proximity guard at inference time"), not the coined term.
4. Provide a plain-ML alias for TCT in §5: "geometric-proximity-guided
   fine-tuning with inference-time acceptance".
5. Acknowledge vocabulary debt: "the term 'teleological' is borrowed
   from philosophy of action to emphasise outcome-verification over
   process-supervision; the mechanism itself is standard
   frozen-encoder cosine alignment".

**Residual RPN**: **4** (LOW-MEDIUM; vocabulary is in the title so
residual drag remains).

---

#### R3-02 — DDA dismissed as rebranded multi-modal / multi-view learning

**Failure mode**: Reviewers / community claims DDA is just multi-view
learning / multi-modal representation learning / self-supervised
multi-task; the "N + C(N,2)" count is a re-derivation of standard
combinatorics on view pairs.

**Potential cause**: Legitimate adjacency to (Tian 2020 contrastive
multi-view, Grill 2020 BYOL multi-target, many others); DDA's distinctness
depends on fine-grained framing.

**Severity**: **3** (undermines DDA's standalone novelty; core framework
contribution weakens but not eliminated).
**Likelihood**: **3** (expected objection).
**RPN**: **9** (MEDIUM).

**Mitigation owner**: agent-22 (related-work writer) + agent-21
(theory-builder) + agent-36 (introduction-writer).

**Mitigation plan**:
1. §2 Related Work has a dedicated subsection "DDA vs multi-view /
   multi-modal learning" that cites Tian 2020, Chen 2020 SimCLR,
   Grill 2020 BYOL, and distinguishes:
   - Multi-view learning: generates augmented views of the input,
     trains model to produce consistent representations across views.
   - DDA: fixed input, N frozen independent embedders, counts
     orthogonal supervisory signals **per input** (not across augmented
     views).
2. §3.4 DDA definition emphasises **frozen embedders** (not trainable
   encoders) and **fixed inputs** (not augmented views).
3. Explicit distinguishing claim: "DDA is not a training algorithm; it
   is a signal-count convention that measures how many orthogonal
   supervisory axes a fixed dataset carries under a given embedder
   panel".
4. Cite Shumailov 2024 model-collapse to situate DDA as "data-side
   reframing of the data wall, not model-side augmentation".
5. §7 Discussion acknowledges the adjacency explicitly and does not
   overclaim novelty against multi-view literature.

**Residual RPN**: **6** (MEDIUM; the adjacency is real, the
distinction holds but takes care to land).

---

#### R3-03 — TCT confused with contrastive / metric / triplet learning

**Failure mode**: Reviewers file TCT under "standard contrastive
learning with a centroid anchor" (e.g. prototype networks, Snell 2017;
centroid-loss / triplet-loss variants, Schroff 2015 FaceNet).

**Potential cause**: TCT's mechanism shares "centroid as anchor" with
prototype networks; the per-output inference-time guard is the distinct
claim but may be overlooked.

**Severity**: **3** (TCT's novelty hinges on the inference-time guard,
not the training-time loss).
**Likelihood**: **3** (expected objection).
**RPN**: **9** (MEDIUM).

**Mitigation owner**: agent-22 + agent-21.

**Mitigation plan**:
1. §2 Related Work distinguishes TCT from (a) prototype networks
   (closest neighbour at inference, not a geometric-predicate guard);
   (b) triplet / contrastive (training-time loss, no inference-time
   predicate); (c) RLHF / DPO (scalar reward, no per-output geometric
   guarantee).
2. §5.4 Theorem 2 (per-output verifiable predicate) is framed as **the
   distinct contribution** — not the centroid itself.
3. TCT's three-part framing (centroid anchor + training loss + inference
   guard) clarifies that the inference guard is the novel piece.
4. One-paragraph explicit comparison: "prototype networks classify at
   inference via nearest-centroid; TCT **conditions acceptance** at
   inference via a cosine-threshold predicate, which is a different
   operation with different guarantees (conditional acceptance, not
   classification)".
5. §7 Discussion: "the per-output predicate is what distinguishes TCT
   from centroid-based metric learning".

**Residual RPN**: **6** (MEDIUM).

---

#### R3-04 — Meaning compression dismissed as rebrand of information-density

**Failure mode**: Reviewers claim meaning compression is just a rename
of Bayesian mutual information / Kolmogorov complexity / representation
density.

**Potential cause**: agent-29 VT-C7 (softened to semantic claim).
The distinction is linguistic-conventional, not structural.

**Severity**: **3** (weakens one of four claims).
**Likelihood**: **3** (expected).
**RPN**: **9** (MEDIUM).

**Mitigation owner**: agent-21 + agent-36.

**Mitigation plan**:
1. Agent-29 VT-C7 rewrite: "semantically distinct; measured as
   structured-signal density (signals per raw-data unit) rather than
   Shannon bits".
2. §3.1 frames meaning compression **operationally**: "meaning
   compression measures signal-density on a fixed corpus; bit-compression
   measures bit-density on a fixed signal; the two are orthogonal
   dimensions of the same information-theoretic space".
3. §5.2 provides a worked example: a single input at 1 KB under bit
   compression yields ~0.5 KB; the same input under meaning compression
   (N=13) yields 91 signals — the same bytes carry 91 supervisory
   axes, which bit compression cannot count.
4. Explicit humility: "we do not claim meaning compression is a new
   information-theoretic quantity; we claim it is a **counting
   convention** that bit / weight / activation compression literature
   has not previously named".
5. §7 Discussion: "the contribution is categorical, not quantitative —
   we name a category that the compression literature has left
   unnamed".

**Residual RPN**: **6** (MEDIUM; the claim is taxonomic and will
remain contentious).

---

### R4 — Vendor / legal / ethics

---

#### R4-01 — Qwen3-TTS vendor withdraws / modifies base model

**Failure mode**: Alibaba / Qwen team withdraws Qwen3-TTS, changes
terms of service, or modifies model behaviour such that the protocol
is unreproducible.

**Potential cause**: Vendor business decisions. Qwen is a research
release with no SLA commitment.

**Severity**: **3** (Case 3 protocol-reproducibility lost; Cases 1 & 2
unaffected since they use open-weight bases).
**Likelihood**: **2** (Qwen is well-maintained; withdrawal in next
12 months unlikely but not zero).
**RPN**: **6** (MEDIUM).

**Mitigation owner**: agent-43 (reproducibility-checker) + agent-40.

**Mitigation plan**:
1. Archive the specific Qwen3-TTS checkpoint SHA-256 to preservation
   bucket (Zenodo or Internet Archive) at submission time, subject
   to license.
2. §6.3 methodology paragraph names the protocol, not just the vendor
   name — protocol (Full ICL + 50-clip centroid + best-of-12 under
   WavLM guard) is portable to any open TTS that supports ICL.
3. §8 L-VendorVersioning paste-ready paragraph.
4. §10 Future Work: replicate Case 3 on an open TTS base (XTTS,
   Bark, YourTTS) to reduce vendor-lock reproducibility risk.

**Residual RPN**: **3** (LOW; archival + protocol-portability framing).

---

#### R4-02 — Identity-clone subject consent challenge (Case 2)

**Failure mode**: The Santa identity-clone subject (or a third party)
raises an ethical objection about the public release of synthetic
voice / face material of the subject, or reviewers object to the
"identity-locked generation" framing on consent grounds.

**Potential cause**: Identity-clone of a real person (even with consent)
is an AI-ethics minefield; IRB-equivalent scrutiny at downstream venues
is increasing.

**Severity**: **4** (ethics rejection at target venues; potential
reputational harm).
**Likelihood**: **2** (single-subject studies where author is also the
subject are standard; third-party subject case (Case 2) needs explicit
consent documentation).
**RPN**: **8** (MEDIUM).

**Mitigation owner**: agent-31 (ethics-reviewer) + principal author.

**Mitigation plan**:
1. **Document consent**: Case 2 subject has written consent on file
   referenced by agent-31 ethics-review. Consent specifies: research
   use, manuscript mention, case-study publication, no commercial
   use beyond the Leapable acknowledgement scope.
2. §6.2 opener names the consent provenance: "Case 2 subject data
   collected with written informed consent for research use under
   protocol [ref]".
3. §8 Ethics paragraph discloses:
   (a) single-subject consent status;
   (b) no third-party synthetic voice/face material released;
   (c) identity-clone application framed as a controlled research
       demonstration, not a deployment artifact.
4. §9 Conclusion does **not** claim identity-clone is safe for
   deployment — positions the research as a capability
   characterisation.
5. Preempt: §7 Discussion paragraph on ethical / deepfake-adjacent
   concerns; named future work: consent revocation mechanisms,
   watermarking, detection-side research.

**Residual RPN**: **4** (LOW-MEDIUM; ethics documentation absorbs).

---

#### R4-03 — Commercial-affiliation objectivity challenge

**Failure mode**: Reviewer or reader accuses author of conflict of
interest because Teleox.ai / Leapable.ai are named in acknowledgement
and the paper presents methodology used by those entities.

**Potential cause**: Author's affiliation is a for-profit entity;
arXiv accepts non-academic authorship but downstream venues may
flag.

**Severity**: **2** (disclosure-only resolution; not fatal).
**Likelihood**: **3** (conflict-of-interest disclosure is standard
in modern review).
**RPN**: **6** (MEDIUM).

**Mitigation owner**: agent-31 (ethics-reviewer) + agent-47.

**Mitigation plan**:
1. Explicit conflict-of-interest footnote on page 1: "Author's
   affiliation is Teleox.ai (Leapable.ai companion). Manuscript
   presents research methodology applicable to but not proprietary
   to those entities; no vendor-specific claim is made."
2. §1 Introduction makes no reference to Teleox / Leapable; vendor
   acknowledgement is in the standard section.
3. Body text avoids first-person plural "our method" for methodology
   that cites the author's prior commercial work; uses
   third-person-neutral throughout.
4. Code / artefact URLs point to open research repos (Context Graph,
   ClipCannon) with independent commit hashes, not to company
   gated assets.
5. Data-availability statement: "research artefacts released under
   open-source license; no vendor-dependent data or tooling required
   to reproduce the protocol".

**Residual RPN**: **2** (LOW; disclosure is the standard resolution).

---

#### R4-04 — Closed WavLM-base-plus-sv artefact distribution risk

**Failure mode**: The specific WavLM-base-plus-sv artefact (which
powers both guard and evaluator) becomes unavailable on Hugging Face
due to license / repo deletion / maintainer abandonment; third-party
reproduction fails.

**Potential cause**: Hugging Face model-repo maintenance is voluntary;
WavLM-base-plus-sv is a fine-tuned derivative whose maintainer may
archive or delete.

**Severity**: **3** (reproducibility decay; weight-pinning mitigates).
**Likelihood**: **2** (WavLM-base-plus-sv is widely used; unlikely to
vanish but possible).
**RPN**: **6** (MEDIUM).

**Mitigation owner**: agent-43 + agent-47.

**Mitigation plan**:
1. SHA-256 pin the WavLM-base-plus-sv weight file; archive to Zenodo
   with permanent DOI at submission.
2. Agent-27 §H manifest records the hash; paper references the DOI,
   not the Hugging Face URL.
3. §6 methodology lists the Zenodo DOI as the canonical artefact
   pointer; HF URL is a secondary pointer.
4. §8 L-VendorVersioning paragraph covers this.
5. Same treatment for ECAPA-TDNN, SigLIP, DNSMOS, UTMOS weights.

**Residual RPN**: **3** (LOW; preservation closes).

---

### R5 — Timing / operational

---

#### R5-01 — Submission window slips past target date

**Failure mode**: Manuscript assembly slips, W1/W2/W3 runs slip,
ethics review slips; target arXiv submission date (agent-00 roadmap)
missed.

**Potential cause**: 48-agent pipeline has long critical path; any
agent stall propagates.

**Severity**: **2** (slip is not fatal; arXiv accepts at any time).
**Likelihood**: **3** (48-agent pipelines commonly slip).
**RPN**: **6** (MEDIUM).

**Mitigation owner**: agent-00 (coordinator) + agent-47.

**Mitigation plan**:
1. Two-track §6 drafting (agent-28 + agent-29 discipline) so pending
   runs do not gate submission.
2. Hard submission deadline drives scope cuts: if W1 + W2 + W3 + Case
   2 end-to-end all slip, submit with Case 3 T1 headline + two-track
   §6 for Cases 1 & 2.
3. v2 revision cycle already planned for landed-later measurements.
4. Agent-00 monitors critical path weekly; agent-47 locks submission
   scope 48h before target date.
5. Back-up: if one of (W1, W2, Case 2 T1) lands late, submit v1 and
   upload v2 within 4 weeks.

**Residual RPN**: **3** (LOW; two-track drafting absorbs).

---

#### R5-02 — arXiv endorsement delay

**Failure mode**: Author's arXiv account lacks cs.LG endorsement;
submission requires an endorsement that does not arrive in time.

**Potential cause**: First-time cs.LG submission; existing endorsements
may not cover cs.LG.

**Severity**: **3** (hard block on submission until resolved).
**Likelihood**: **2** (depends on author's endorsement status;
variable).
**RPN**: **6** (MEDIUM).

**Mitigation owner**: agent-47 + principal author.

**Mitigation plan**:
1. Check current endorsement status **weeks ahead** of target
   submission date.
2. Identify 3 candidate endorsers (published in cs.LG with > 2
   arXiv papers); warm-contact each with draft + endorsement ask.
3. Submit endorsement request >= 2 weeks before target date.
4. Fallback: submit to cs.CL or cs.CV primary if those endorsements
   exist, with cs.LG cross-listing.
5. If first endorser slow, pivot to second / third candidate within
   48h.

**Residual RPN**: **3** (LOW; proactive endorsement mitigation).

---

#### R5-03 — Pre-submission leak reduces novelty

**Failure mode**: Author's public social media / blog / talk leaks
the DDA / TCT / meaning-compression framing before arXiv submission;
a concurrent paper scoops the concept.

**Potential cause**: Author's active public presence; 48-agent
pipeline duration creates a leak window.

**Severity**: **3** (novelty partial-loss; concurrent-citation
workaround exists but reputation harm if clear precedence is lost).
**Likelihood**: **2** (authored concurrent work is rare; social-media
mention is common but not usually a scoop vector).
**RPN**: **6** (MEDIUM).

**Mitigation owner**: principal author + agent-47.

**Mitigation plan**:
1. Social-media embargo on DDA / TCT / meaning-compression specific
   framing until arXiv timestamp exists.
2. Any pre-submission talks / blog posts use generic framing
   ("frozen-encoder alignment", "signal decomposition") and do not
   coin the specific terms.
3. Maintain a **dated draft log** (git commits) — if a concurrent
   paper appears, git timestamps establish independent-development
   precedence.
4. arXiv submission generates the citation-anchor timestamp; delay
   of > 2 weeks past manuscript readiness is the risk window.
5. If a concurrent paper appears before submission, §2 Related Work
   promptly cites it and differentiates — concurrent work does not
   kill arXiv submission, only requires acknowledgement.

**Residual RPN**: **2** (LOW; embargo + dated drafts close).

---

## 3. Summary RPN table (sorted by RPN descending)

| ID | Category | Failure Mode | S | L | RPN | Priority | Residual RPN |
|---|---|---|---|---|---|---|---|
| R1-03 | R1 | Reviewer attacks N=1 design | 4 | 4 | **16** | CRITICAL | 6 |
| R1-04 | R1 | Case 3 encoder-matched rejected as circular | 4 | 4 | **16** | CRITICAL | 4 (A) / 9 (B) |
| R2-02 | R2 | Case 2 T1 doesn't land | 4 | 4 | **16** | CRITICAL | 9 |
| R2-01 | R2 | Case 1 T1 doesn't land | 3 | 4 | **12** | HIGH | 6 |
| R2-05 | R2 | Replication fails due to weight drift | 3 | 4 | **12** | HIGH | 4 |
| R1-01 | R1 | arXiv moderator miscategorises | 3 | 3 | **9** | MEDIUM | 4 |
| R1-05 | R1 | Closed-artifact irreproducibility | 3 | 3 | **9** | MEDIUM | 4 |
| R1-06 | R1 | Meaning-compression rebrand objection | 3 | 3 | **9** | MEDIUM | 6 |
| R2-03 | R2 | W1 cross-encoder < 0.95 | 3 | 3 | **9** | MEDIUM | 4 |
| R2-04 | R2 | W2 TCT-delta small | 3 | 3 | **9** | MEDIUM | 6 |
| R3-02 | R3 | DDA as rebranded multi-view | 3 | 3 | **9** | MEDIUM | 6 |
| R3-03 | R3 | TCT as contrastive / prototype | 3 | 3 | **9** | MEDIUM | 6 |
| R3-04 | R3 | Meaning compression as rebrand | 3 | 3 | **9** | MEDIUM | 6 |
| R1-02 | R1 | Desk-reject "marketing paper" | 4 | 2 | **8** | MEDIUM | 2 |
| R3-01 | R3 | Constellation vocabulary ridicule | 2 | 4 | **8** | MEDIUM | 4 |
| R4-02 | R4 | Identity-clone consent challenge | 4 | 2 | **8** | MEDIUM | 4 |
| R2-06 | R2 | Pairwise-MI demand | 2 | 3 | **6** | MEDIUM | 3 |
| R4-01 | R4 | Qwen3-TTS withdrawal | 3 | 2 | **6** | MEDIUM | 3 |
| R4-03 | R4 | Commercial-affiliation COI | 2 | 3 | **6** | MEDIUM | 2 |
| R4-04 | R4 | WavLM artefact distribution | 3 | 2 | **6** | MEDIUM | 3 |
| R5-01 | R5 | Submission window slip | 2 | 3 | **6** | MEDIUM | 3 |
| R5-02 | R5 | arXiv endorsement delay | 3 | 2 | **6** | MEDIUM | 3 |
| R5-03 | R5 | Pre-submission leak | 3 | 2 | **6** | MEDIUM | 2 |

**Criticals (RPN >= 15)**: 3 risks (R1-03, R1-04, R2-02).
**Highs (10–14)**: 2 risks (R2-01, R2-05).
**Mediums (6–9)**: 18 risks.
**Lows (<6)**: 0 risks.

**Post-mitigation**: all residuals land <= 9 (MEDIUM) assuming Track A for
R1-04 (W1 lands). Track B worst-case residual is 9 (R1-04) and 9 (R2-02)
— both survivable with agent-29 two-track disclosure in §6 and §8.

## 4. Concentration analysis

**Shared root causes** (risk clustering):

1. **Case-3 encoder-matched evaluation cluster** (R1-04, partially R2-03,
   partially R2-04) — all trace to Case-3 single-encoder self-consistency;
   mitigated jointly by W1 + W2 runs + §8 L-01 + L-02.
2. **T1 measurement-landing cluster** (R2-01, R2-02) — both Case 1 and
   Case 2 are T1 PENDING; same mitigation pattern (two-track drafting).
3. **Vocabulary-novelty cluster** (R3-01, R3-02, R3-03, R3-04) — all
   trace to coined terms; mitigated jointly by §1 + §2 + §4 definitional
   discipline and explicit distinguishing against prior art.
4. **Vendor-lock cluster** (R2-05, R4-01, R4-04) — all trace to closed-
   weight / closed-artefact dependencies; mitigated jointly by Zenodo
   preservation + SHA-256 pinning + agent-27 §H manifest.
5. **Exploratory-scope cluster** (R1-03, R1-05, overlaps R4-02) — all
   trace to N=1 single-subject design; mitigated jointly by §6 opener
   + §8 L-Population + §9 Conclusion scope discipline.

## 5. Confidence

**HIGH** that the 5 categories exhaustively cover submission-program
risk beyond agent-29's scientific-validity view.

**HIGH** that the three CRITICAL risks (R1-03, R1-04, R2-02) are
correctly identified as the top-priority items and that mitigation
residuals are <= 9.

**HIGH** that post-mitigation residuals are all <= 9 (MEDIUM) under
honest Track-A / Track-B contingency modelling.

**MEDIUM** on R2-02 Case 2 T1 timing — depends on integration work
outside agent-30's visibility.

**MEDIUM** on R5-02 endorsement timing — depends on principal author's
current endorsement status, which agent-30 has not directly verified.

## Changelog

- 2026-04-16 — Initial issue by agent-30-risk-analyst.
