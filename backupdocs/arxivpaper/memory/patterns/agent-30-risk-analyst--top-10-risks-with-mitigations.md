---
namespace: patterns
agent_id: agent-30-risk-analyst
timestamp: 2026-04-16T19:15:00Z
status: active
locked: false
supersedes: none
depends_on:
  - memory/discoveries/agent-30-risk-analyst--fmea-risk-register.md
  - memory/patterns/agent-29-validity-guardian--mitigation-plan.md
downstream_consumers:
  - agent-31-ethics-reviewer
  - agent-37-methodology-writer
  - agent-40-limitations-writer
  - agent-42-adversarial-reviewer
  - agent-43-reproducibility-checker
  - agent-47-submission-packager
  - agent-48-final-reviewer
tags: mitigation, top-10, risk, paste-ready, owners, checklist
answers_questions: [Q07, Q12, Q16]
---

# Top-10 Risks with Mitigations — Paste-ready Action Table

## 0. Purpose

Action-ordered extract of the 10 highest-RPN risks from the FMEA
register (`fmea-risk-register.md`). Each row has: RPN (before /
after), owner, concrete mitigation actions, and an
"implementation status" tag that agent-47 / agent-48 can check at
submission-lock time.

Mitigation actions are **paste-ready**: they correspond to specific
sentences / paragraphs / §-location instructions the writers can apply
without re-reading upstream memory. Actions cross-reference agent-29's
§4 paste-ready disclosure library where applicable.

---

## 1. Top-10 risk ordering (by pre-mitigation RPN)

| Rank | ID | Title | RPN | Residual | Owner |
|------|----|-------|-----|----------|-------|
| 1 | R1-03 | N=1 design attacked | 16 | 6 | agent-38 + agent-40 + agent-42 |
| 2 | R1-04 | Case 3 encoder-matched as circular | 16 | 4 (A) / 9 (B) | agent-38 + agent-40 |
| 3 | R2-02 | Case 2 T1 doesn't land | 16 | 9 | principal author + agent-38 + agent-40 |
| 4 | R2-01 | Case 1 T1 doesn't land | 12 | 6 | principal author + agent-38 + agent-40 |
| 5 | R2-05 | Replication fails due to vendor weight drift | 12 | 4 | agent-27 + agent-43 + agent-47 |
| 6 | R1-01 | arXiv moderator miscategorisation | 9 | 4 | agent-47 + agent-48 |
| 7 | R1-05 | Closed-artifact irreproducibility | 9 | 4 | agent-43 + agent-47 |
| 8 | R1-06 | Meaning-compression rebrand objection | 9 | 6 | agent-21 + agent-36 |
| 9 | R2-03 | W1 cross-encoder < 0.95 | 9 | 4 | agent-38 + agent-40 |
| 10 | R2-04 | W2 TCT-delta small | 9 | 6 | agent-38 + agent-40 + agent-21 |

---

## 2. Paste-ready action cards

### Rank 1 — R1-03 (N=1 design attacked). RPN 16 → 6.

**Owner**: agent-38 (results) + agent-40 (limitations) + agent-42
(adversarial review).

**Actions**:

- **[A1] §6 case openers**: each case (6.1, 6.2, 6.3) opens with the
  FM-05 protocol paragraph. Paste-ready (Case 3):
  > *"One speaker (the author), 50-clip reference centroid, 10
  > held-out sentences, WavLM-SECS-scored best-of-12 at T=0.3;
  > DNSMOS / UTMOS parity computed against the same speaker's real
  > recordings. We do not generalise beyond this protocol; see §8."*
- **[A2] §8 L-Population paragraph** un-cuttable. Agent-29 §4
  paste-ready:
  > *"Cases 2 and 3 are N=1 subject (the author). Case 1 is a
  > single-pipeline study on a single canonical corpus (Shakespeare).
  > No case supports multi-subject, multi-speaker, or multi-author
  > generalisation."*
- **[A3] §9 Conclusion scrub**: agent-42 must-catch items 1 and 6 run
  on §9 — no "generalises to" / "applies to arbitrary speakers" /
  "for any subject" / cross-language claim allowed.
- **[A4] Title discipline**: include words "three case studies" or
  "exploratory demonstration" so reader's expectation is set before
  they read §6.
- **[A5] §7 Discussion framing**: "the framework is general, the
  evidence is exploratory" — name this split explicitly.

**Implementation status**: [ ] drafted  [ ] reviewed  [ ] locked.

**Residual 6**: N=1 is UNCLOSEABLE per agent-29 VT-E1; residual is
the minimum achievable without cross-subject replication.

---

### Rank 2 — R1-04 (Case 3 encoder-matched rejected as circular). RPN 16 → 4 / 9.

**Owner**: agent-38 + agent-40.

**Actions**:

- **[A1] §6.3 L-SECS-A single-sentence disclosure** (mandatory,
  verbatim):
  > *"The 0.961 mean SECS is an encoder-matched measurement: the
  > guard and evaluator are both WavLM-base-plus-sv. This demonstrates
  > the guard behaves as specified; it does not constitute
  > cross-encoder identity verification."*
- **[A2] §8 L-01 paragraph** carries the remediation plan and
  contingency (Track A: W1 lands; Track B: W1 does not land).
  Agent-29 patterns file has both track versions paste-ready.
- **[A3] §5.4 attribution softening**: "TCT causes identity-lock" →
  "TCT verifies and selectively accepts" (agent-24 W6 remediation).
- **[A4] W1 run prioritisation**: ECAPA-TDNN + Resemblyzer on the
  10 argmax candidates before submission lock (agent-28 §4.6 instrument-
  ready, agent-27 A.2 schema). Agent-30 recommends running W1 even
  if result is < 0.95 — a weak cross-encoder signal is still honest
  triangulation evidence.
- **[A5] H2a reframe**: from "cross-encoder identity verification" to
  "within-WavLM-family identity agreement, 9/10 >= 0.95 on 10
  held-out sentences".
- **[A6] Pearson + Spearman + Bland-Altman**: the W1 analysis emits
  these agreement stats (agent-28 §4.6). A strong agreement stat with
  any offset level is evidence of ranking consistency; this is what
  §6.3 should headline, not the raw cross-encoder mean.

**Implementation status**: [ ] W1 scheduled  [ ] W1 completed
[ ] §6.3 drafted  [ ] §8 L-01 pasted  [ ] §5.4 softened.

**Residual 4 (Track A)** if W1 returns Pearson >= 0.5 and ECAPA mean
within ±0.10 of WavLM; **residual 9 (Track B)** if W1 does not land.

---

### Rank 3 — R2-02 (Case 2 T1 doesn't land). RPN 16 → 9.

**Owner**: principal author + agent-38 + agent-40.

**Actions**:

- **[A1] Two-track §6.2 drafting** (agent-28 §3.2 discipline):
  - **Track A** (T1 lands): paste measured per-modality table with
    conjunction accept-rate + Wilson-score 95% CI.
  - **Track B** (T1 does not land): per-modality table stamped
    "PENDING — architecturally complete"; §8 L-05 paste-ready.
- **[A2] §5.4 multi-modal strictness as derivational** (not empirical):
  > *"Multi-modal strictness is derivational under Axiom 2 pairwise
  > independence; M > 1 empirical measurement is pending Case 2."*
- **[A3] §8 L-05 paste-ready** (verbatim from agent-29 patterns):
  paragraph explicitly names Case 2 end-to-end T1 as "the critical
  pending measurement" and promises v2 upgrade.
- **[A4] Partial-landing triage**: if only some of the four
  modalities (visual / emotion / speaker / voice) land T1, paste the
  measured rows and flag the missing ones — do not hide the partial
  result.
- **[A5] Case 2 priority ordering**: concentrate remaining integration
  bandwidth on mouth-weighted flow-matching + SigLIP face-crop
  preprocessing (agent-26 §A.2.2) to maximise at-least-one T1 row
  landing.
- **[A6] Case 3 protection**: if Case 2 cannot land even partial T1,
  Case 3 is the measurement flagship — do not dilute its framing with
  Case 2 apologia in §6.3.

**Implementation status**: [ ] Track A/B decided  [ ] §6.2 drafted
[ ] §5.4 framing applied  [ ] §8 L-05 pasted.

**Residual 9** because multi-modal is the paper's distinct theoretical
payload and derivational-only evidence remains a real reception drag
even with discipline.

---

### Rank 4 — R2-01 (Case 1 T1 doesn't land). RPN 12 → 6.

**Owner**: principal author + agent-38 + agent-40.

**Actions**:

- **[A1] Two-track §6.1**: Track A (measured table); Track B
  ("architecturally complete; 15/15 FSV-passed; WavLM-style-cosine
  metric-report PENDING").
- **[A2] §1 opener reframe if Track B**: "two T1-measured cases
  (Case 3 Case 2) plus one architecturally-complete case (Case 1)"
  or "one T1-measured headline case (Case 3) plus two architecturally-
  complete demonstrations (Cases 1 & 2)" depending on Case 2 status.
- **[A3] FM-04 discipline**: every projected number in §6.1 carries
  T3 PROJECTED tag (agent-34 confidence-quantifier enforces).
- **[A4] Case 1 metric-report minimum**: even if DPO doesn't complete,
  run WavLM-style-cosine metric evaluation on the last SFT checkpoint
  outputs so §6.1 has at least one T1 row.
- **[A5] §8 L-Case1 paragraph**: "Case 1 is architecturally complete
  with 15/15 FSV battery passing; end-to-end metric-report is PENDING
  at submission time and will be reported in v2."
- **[A6] Forensic narrative fallback**: if numbers don't land, §6.1
  leans into the **forensic contribution** (training-data echo-bug
  diagnosis + fix) as the demonstrable deliverable.

**Implementation status**: [ ] Track A/B decided  [ ] §6.1 drafted
[ ] §1 opener reframed if needed  [ ] §8 L-Case1 pasted.

**Residual 6** because Case 1 is lowest-impact of the three
(single-pipeline, single-corpus); losing it is survivable.

---

### Rank 5 — R2-05 (Replication fails due to vendor weight drift). RPN 12 → 4.

**Owner**: agent-27 (instrument-developer) + agent-43 (reproducibility-
checker) + agent-47 (submission-packager).

**Actions**:

- **[A1] §H manifest mandatory**: every T1 number carries
  `embedder_version_hash` (SHA-256 of the weight file on disk at
  measurement time), `pipeline_version`, `measurement_date`,
  `centroid_hash`, `threshold_set_hash`, `reference_set_manifest`.
- **[A2] Preservation bucket**: commit the specific weight files
  (WavLM-base-plus-sv, ECAPA-TDNN, SigLIP, DNSMOS, UTMOS, and —
  subject to license — Qwen3-TTS) to Zenodo with permanent DOI at
  submission time.
- **[A3] §6 methodology paragraph** (paste-ready):
  > *"All measurements are tagged to encoder weight SHA-256 and
  > pipeline version. Archived weights are available at
  > [Zenodo DOI]; future vendor releases may produce different
  > values. The manifest schema enables byte-identical re-measurement
  > against pinned weights."*
- **[A4] Weight-pin script**: release a helper script (`scripts/
  reproducibility/pin_weights.sh`) that, given the manifest, fetches
  the exact pinned artefacts from the vendor or falls back to the
  Zenodo archive.
- **[A5] §8 L-VendorVersioning paragraph** (agent-29 VT-E5
  paste-ready, verbatim).
- **[A6] Reproducibility footer note**: link the DOI from the first
  page footer for discoverability.

**Implementation status**: [ ] manifest schema implemented
[ ] Zenodo bucket created  [ ] pin_weights.sh released
[ ] §6 methodology paragraph drafted  [ ] §8 L-VendorVersioning
pasted.

**Residual 4** because weight drift is inevitable at 12+ months; the
pin + preservation closes the risk that the **specific number** can't
be reproduced, but the paper's headline moves if vendor changes model
behaviour in non-recoverable ways.

---

### Rank 6 — R1-01 (arXiv moderator miscategorises). RPN 9 → 4.

**Owner**: agent-47 + agent-48.

**Actions**:

- **[A1] Pre-screen**: read last 10 cs.LG acceptances matching the
  domain (multi-modal embedding, speaker verification, generation)
  and check title + abstract tone against them.
- **[A2] Abstract first sentence in plain ML vocabulary**:
  > *"We study frozen-encoder-guided generation: a family of methods
  > that decompose a fixed corpus through N independent frozen
  > embedders to produce orthogonal supervisory signals, and verify
  > per-output identity via geometric proximity to a frozen
  > centroid."*
- **[A3] Coined terms introduced only in §1.1** after the plain-ML
  framing in the abstract.
- **[A4] cs.LG primary + cs.CL + cs.CV cross-list** — if moderator
  reroutes, cross-listing catches it.
- **[A5] Warm-contact endorser >= 1 week before submission**; provide
  draft in advance.
- **[A6] Paste-ready moderator response** (if held): 1-paragraph
  plain-ML restatement + list of load-bearing refs from
  `_evidence_*.md`.

**Implementation status**: [ ] tone pre-screen done  [ ] abstract
first sentence drafted  [ ] cross-listing set  [ ] endorser warmed
up  [ ] hold-response paragraph ready.

**Residual 4** because some hold probability remains from any
unusual-vocabulary submission.

---

### Rank 7 — R1-05 (Closed-artifact irreproducibility). RPN 9 → 4.

**Owner**: agent-43 + agent-47.

**Actions**:

- **[A1] Release the pipeline code** for all three cases (Context
  Graph repo, ClipCannon repo, Shakespeare LoRA stack).
- **[A2] Release manifest schema + weight-pin script** (R2-05
  already covers this).
- **[A3] §6 openers** name the reproducibility posture explicitly:
  - Case 1: **fully open** (reproducibility flagship).
  - Case 2: **protocol-level reproducible** (subject data is private
    with documented consent; third party substitutes their own subject
    and re-runs).
  - Case 3: **byte-identical against pinned weights** (Qwen3-TTS
    vendor-pinned; protocol also portable to open TTS per R4-01).
- **[A4] Open Case-1 reproducibility demonstration**: third-party-
  runnable notebook / script for the Shakespeare LoRA stack on the
  public canonical corpus.
- **[A5] §8 L-Reproducibility paragraph**:
  > *"Case 1 is fully reproducible on public data; Case 2 provides
  > protocol-level reproducibility against third-party subject data;
  > Case 3 is byte-identical-reproducible against pinned Qwen3-TTS
  > weights. Subject-data privacy for Case 2 is governed by the
  > consent protocol referenced in the ethics paragraph."*

**Implementation status**: [ ] code repos tagged
[ ] manifest schema released  [ ] Case-1 open notebook published
[ ] §6 openers drafted  [ ] §8 L-Reproducibility pasted.

**Residual 4** because closed subject-data (Case 2) is an inherent
restriction that no mitigation closes.

---

### Rank 8 — R1-06 (Meaning-compression rebrand objection). RPN 9 → 6.

**Owner**: agent-21 (theory-builder) + agent-36 (introduction-writer)
+ agent-42.

**Actions**:

- **[A1] Agent-29 VT-C7 rewrite**: everywhere the claim appears, use
  "semantically distinct; measured as structured-signal density
  (signals per raw-data unit) rather than Shannon bits".
- **[A2] §3.1 positioning**: explicit comparison against TurboQuant,
  GPTQ, AWQ (bit/weight compression); activation compression (e.g.
  Mishra 2017, Han 2015). Name each and identify its unit.
- **[A3] §5.2 worked example**: one input at 1 KB → ~0.5 KB under bit
  compression; same input under meaning-compression (N=13) → up to 91
  signals. Make the different unit concrete.
- **[A4] §7 humility paragraph** (paste-ready):
  > *"We do not claim a new compression algorithm or a new
  > information-theoretic quantity. We claim a new counting convention
  > orthogonal to Shannon-bit accounting — one that the bit /
  > weight / activation compression literature has not previously
  > named or measured."*
- **[A5] §9 Conclusion restraint**: no claim of "meaning compression
  solves the data wall"; stick to "DDA reframes the data wall as
  signal-under-utilisation".

**Implementation status**: [ ] VT-C7 rewrite applied all locations
[ ] §3.1 positioning drafted  [ ] §5.2 worked example drafted
[ ] §7 humility pasted  [ ] §9 restraint confirmed.

**Residual 6** because the taxonomic claim remains philosophically
contentious even under best framing.

---

### Rank 9 — R2-03 (W1 cross-encoder returns < 0.95). RPN 9 → 4.

**Owner**: agent-38 + agent-40.

**Actions**:

- **[A1] Pre-register the agreement metric** (Pearson + Spearman +
  Bland-Altman), not the raw cross-encoder mean.
- **[A2] §6.3 table**: report both raw ECAPA / Resemblyzer numbers
  **and** the agreement statistics. Framing: "agreement within ±0.10
  supports within-family identity-ranking consistency".
- **[A3] Honesty discipline**: do not rerun, do not cherry-pick, do
  not rescale. The number is what it is.
- **[A4] §8 L-01 Track B handles low-agreement gracefully**: reframe
  the Case 3 claim to "within-WavLM-family demonstration; cross-encoder
  triangulation inconclusive; named future work".
- **[A5] §6.3 narrative**: lead with the agreement statistics (if
  strong), not the cross-encoder raw mean. A Pearson of 0.8 with an
  offset of 0.10 is a stronger signal than Pearson 0.3 with an
  offset of 0.00.

**Implementation status**: [ ] agreement metrics pre-registered
[ ] §6.3 table structure decided  [ ] honesty discipline confirmed
with principal author.

**Residual 4** because the design absorbs the outcome honestly.

---

### Rank 10 — R2-04 (W2 TCT-delta small). RPN 9 → 6.

**Owner**: agent-38 + agent-40 + agent-21.

**Actions**:

- **[A1] Pre-announce TCT as three-part** in §5 so the narrative does
  not depend on a large best-of-K delta:
  1. Frozen-centroid design (load-bearing regardless of Δ_TCT).
  2. Per-output verifiable predicate (Theorem 2; load-bearing
     regardless of Δ_TCT).
  3. Selection operator (best-of-K under the predicate; Δ_TCT
     measures this piece only).
- **[A2] §6.3 reports both means** (pre-selection + post-selection)
  descriptively. Pre-selection mean is interesting **in its own
  right** — it characterises the base pipeline.
- **[A3] Pre-selection tail analysis**: report the fraction of
  candidates below the 0.95 guard threshold. Even a small Δ_mean can
  correspond to a high filter-rate on the lower tail, which is the
  more interesting quantity.
- **[A4] §7 Discussion pivot** (paste-ready if Δ_TCT ≈ 0):
  > *"TCT's contribution in Case 3 is **verification and acceptance**,
  > not **mean-score inflation**. The per-output predicate is the
  > load-bearing claim; mean-score uplift from best-of-K is a
  > secondary effect whose magnitude depends on the base pipeline's
  > saturation."*
- **[A5] §9 Conclusion restraint**: do not claim TCT-delta magnitude
  as an empirical contribution; claim TCT's **predicate-level
  guarantee** and characterise selection as "within that predicate".

**Implementation status**: [ ] §5 three-part framing drafted
[ ] §6.3 pre / post reporting structure decided  [ ] §7 pivot
paragraph pre-written  [ ] §9 restraint confirmed.

**Residual 6** because the "TCT adds significant value" narrative
is weakened even under optimal framing.

---

## 3. Cross-cutting mitigation checklist (agent-47 submission-lock)

Before submission-lock, agent-47 verifies each of these:

- [ ] **R1-03**: §6 case openers contain FM-05 protocol paragraph;
  §8 L-Population pasted un-cut; §9 Conclusion scrubbed of
  generalisation.
- [ ] **R1-04**: §6.3 L-SECS-A sentence pasted verbatim; §8 L-01
  paragraph pasted; §5.4 uses "operationalises" / "verifies", not
  "causes"; W1 run status logged.
- [ ] **R2-02**: §6.2 two-track decision made; §5.4 multi-modal
  framing derivational; §8 L-05 pasted.
- [ ] **R2-01**: §6.1 two-track decision made; §8 L-Case1 pasted if
  Track B; §1 opener matches Track A/B.
- [ ] **R2-05**: Manifest schema shipped; Zenodo DOI live; weight-pin
  script released; §6 methodology paragraph carries the weight-hash
  discipline; §8 L-VendorVersioning pasted.
- [ ] **R1-01**: abstract first sentence in plain-ML vocabulary;
  cs.LG primary with cs.CL + cs.CV cross-list; endorser confirmed;
  hold-response paragraph ready.
- [ ] **R1-05**: Pipeline code repos tagged + linked in data-availability
  statement; Case-1 open-reproducibility notebook published.
- [ ] **R1-06**: VT-C7 rewrite applied everywhere; §7 humility pasted.
- [ ] **R2-03**: Agreement metrics pre-registered; §6.3 table structure
  decided; honesty discipline noted.
- [ ] **R2-04**: §5 three-part TCT framing drafted; §7 pivot paragraph
  pre-written.

---

## 4. Role matrix — who owns what

| Role / Agent | Primary risk ownership | Review responsibilities |
|------|------|------|
| principal author (Chris Royse) | R2-01 R2-02 R5-02 R5-03 | All T1 landing decisions |
| agent-21 (theory-builder) | R1-06 R3-02 R3-03 R3-04 | Taxonomic claim framing |
| agent-22 (related-work) | R3-02 R3-03 | §2 distinguishing |
| agent-27 (instrument-developer) | R2-05 R4-04 | §H manifest + SHA pinning |
| agent-31 (ethics-reviewer) | R4-02 R4-03 | Consent + COI disclosure |
| agent-36 (introduction-writer) | R1-06 R3-01 R3-04 | §1 abstract + intro tone |
| agent-37 (methodology-writer) | R1-04 R2-05 | §6 methodology paragraphs |
| agent-38 (results-writer) | R1-03 R1-04 R2-01 R2-02 R2-03 R2-04 | §6 case openers + results |
| agent-39 (discussion-writer) | R1-02 R1-03 R1-06 R3-* | §7 voice + claims |
| agent-40 (limitations-writer) | R1-03 R1-04 R2-01 R2-02 R2-05 R4-* | §8 all L-* paragraphs |
| agent-42 (adversarial-reviewer) | R1-02 R1-03 R1-06 R3-01 R3-02 R3-03 | must-catch enforcement |
| agent-43 (reproducibility-checker) | R1-05 R2-05 R4-01 R4-04 | artefact release + manifest |
| agent-47 (submission-packager) | R1-01 R5-01 R5-02 R5-03 | arXiv upload + endorsement |
| agent-48 (final-reviewer) | R1-01 R1-02 | final pre-submit scrub |

---

## 5. Confidence

**HIGH** that the 10 top-RPN risks correctly prioritise action
across the submission program.

**HIGH** that the paste-ready actions cross-reference agent-29's
patterns file such that no writer needs to re-derive disclosure
language from scratch.

**HIGH** that the cross-cutting checklist is executable by agent-47
at submission-lock time.

**MEDIUM** on whether all Track-A remediations (W1, W2, Case 2 T1,
Case 1 T1) land in time; the plan degrades gracefully to Track-B for
each.

## Changelog

- 2026-04-16 — Initial issue by agent-30-risk-analyst.
