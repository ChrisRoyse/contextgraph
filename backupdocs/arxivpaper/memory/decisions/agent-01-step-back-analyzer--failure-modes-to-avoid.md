---
namespace: decisions
agent_id: agent-01-step-back-analyzer
timestamp: 2026-04-16T15:15:00Z
status: active
tags: failure-modes, anti-patterns, red-flags, non-negotiable
---

# Named Failure Modes — Downstream Agents MUST Avoid

This is a decisions file, not a pattern. The failure modes below are **binding constraints**
on every downstream agent (02–48). If an agent's output triggers one of these modes, the
coordinator or the adversarial-reviewer (42) will flag it for revision.

Each failure mode is named, described, given a concrete example of what it looks like, and
paired with the correct counter-behavior.

---

## FM-01 — Marketing Creep

**Description.** Prose drifts from research-paper voice into product-pitch voice. Superlatives
accumulate; "we" becomes heroic; Teleox.ai or Leapable.ai sneak into the body.

**Diagnostic example.** "Our groundbreaking TCT framework revolutionizes identity-locked
generation and positions Teleox.ai at the forefront of the data-wall era."

**Why fatal.** arXiv desk-rejects papers that read as commercial content. Even a single
sentence of this kind in the abstract is enough to lose reviewer trust.

**Counter-behavior.** Third-person neutral voice. Product names only in Acknowledgements.
Superlatives banned per G4. When tempted to write "revolutionary", substitute with "we
describe" or "we demonstrate".

**Enforcer.** Agent-42 (adversarial-reviewer) regex pass.

---

## FM-02 — DDA-as-Synthetic-Data Confusion

**Description.** A writer conflates **Derived Data Abundance** (one input projected through N
frozen embedders yields ~N + C(N,2) *signals* per input) with **synthetic data generation**
(a model producing new inputs). These are structurally different. DDA does NOT produce new
examples; it produces new *supervisory axes* over existing examples. Synthetic-data literature
(including the model-collapse literature in `_evidence_01_data_wall.md`) is adjacent to DDA
but distinct from it.

**Diagnostic example.** "DDA resembles synthetic data augmentation — we generate 91 extra
training points per input." (Wrong: we do not generate inputs; we decompose one input along 91
embedding dimensions.)

**Why fatal.** Reviewers familiar with Shumailov et al. (model collapse) will see "DDA" as
"just more synthetic data" and reject with a pointer to the collapse literature unless we
explicitly distinguish upfront.

**Counter-behavior.** Every reference to DDA in §3 and §5 must include one sentence like:
"DDA does not synthesize new inputs; it decomposes each input along N frozen, independent
embedding spaces, yielding orthogonal supervisory signals without new content generation.
This is why the model-collapse arguments of Shumailov et al. do not apply."

**Enforcer.** Agent-04 (construct-definer), agent-21 (theory-builder), agent-42 (adversarial-reviewer).

---

## FM-03 — AGI / Superintelligence Overclaim

**Description.** A paragraph climbs the abstraction ladder from "TCT gives identity-locked
generation" to "TCT is a path to general intelligence / AGI / post-data-wall capabilities".
Any such sentence is an immediate desk-reject risk.

**Diagnostic example.** "By breaking the data wall, meaning compression opens a route to
scaling beyond current frontier models and ultimately to general intelligence."

**Why fatal.** cs.LG reviewers treat AGI claims as crank signals.

**Counter-behavior.** The ceiling claim is: "DDA **reframes** the data wall from 'we lack
inputs' to 'we under-utilize the inputs we have'." Stop there. The manuscript does not make
any statement about AGI, scaling laws beyond current models, or intelligence generally.

**Enforcer.** Agent-42; also agent-39 (discussion-writer) must self-police §7.

---

## FM-04 — Conflating Measured and Projected Numbers

**Description.** A sentence states a Case 1 (Shakespeare) or Case 2 (Santa) number as if it
were measured, when in fact those runs are architecturally complete but pending. Case 3 is
the only T1-measured case at submission time.

**Diagnostic example.** "Across all three case studies, identity-locked generation achieves
SECS > 0.95." (Wrong: only Case 3 has measured SECS. Cases 1 and 2 have not produced SECS.)

**Why fatal.** Violates G1 (provenance integrity) and P01 (measured-or-flagged). Also
scientifically dishonest.

**Counter-behavior.** Results framed per-case. "Case 3 measures SECS 0.961 (T1). Case 1
(Shakespeare SFT+DPO in progress) and Case 2 (Santa LoRA architecturally complete, metric
pending) are at T2+T3; projected numbers are flagged and will be re-verified at
camera-ready."

**Enforcer.** Agent-38 (results-writer), agent-34 (confidence-quantifier), agent-43
(reproducibility-checker).

---

## FM-05 — Hidden Single-Subject Sample-Size Problem

**Description.** Case 3 is a single-speaker study on a fixed 10-sentence held-out set. The
author is also the subject. This is a legitimate but narrow setup. Failing to foreground the
sample-size and author-is-subject facts invites accusations of evaluation-bias.

**Diagnostic example.** A §6 subsection presents 0.961 SECS and 3.93 DNSMOS as if the protocol
generalized to arbitrary speakers.

**Why fatal.** An adversarial reviewer will ask "n=1 subject, n=10 held-out sentences; where
is your confidence interval?" and the paper has no answer unless we state the scope upfront.

**Counter-behavior.** §6 Case 3 opens with the protocol: "One speaker (the author),
50-clip reference centroid, 10 held-out sentences, WavLM-SECS-scored best-of-12 at T=0.3;
DNSMOS / UTMOS parity computed against the same speaker's real recordings. We do not
generalize beyond this protocol; see §8."

**Enforcer.** Agent-29 (validity-guardian), agent-31 (ethics-reviewer), agent-38
(results-writer), agent-42.

---

## FM-06 — Code-Block Smuggling in the Manuscript Body

**Description.** A writer, wanting to be concrete, inserts Python or Rust code blocks into
the manuscript body. The constraint says **no source code in the body — pseudocode or
equations only**. Code goes to the appendix.

**Diagnostic example.** §5 Methods containing a 20-line Python block showing the training loop.

**Why fatal.** Violates explicit constraint. Also: code in the body expands length (G5 gate)
and displaces prose.

**Counter-behavior.** Pseudocode using standard math notation or a compact algorithm-figure
pattern. Full code in Appendix A. If the reader needs to see the actual training loop, they
go to appendix.

**Enforcer.** Agent-37 (methodology-writer), agent-46 (file-length-manager), agent-42.

---

## FM-07 — Re-explaining the Prior Manuscript

**Description.** The arXiv version starts reiterating the prior `docs/researchpaper/`
16-section draft verbatim. The arXiv version is supposed to be **tighter**, not a copy. If a
point does not pass the "does a skeptical reviewer need this?" test, it goes.

**Diagnostic example.** §2 Related Work runs 2,500 words because the prior manuscript's §2
was 2,500 words. Every sub-topic is included regardless of relevance.

**Why fatal.** G5 length gate.

**Counter-behavior.** Every section is drafted from scratch against the mission brief's
target length. The prior manuscript is a *source*, not a template. Agent-36, agent-37,
agent-39 all draft fresh and pull quotes/refs, not prose.

**Enforcer.** Agent-46.

---

## FM-08 — Silent Failure Hiding

**Description.** A case study or system documentation omits known bugs (Shakespeare echo-bug,
Santa flow-matching sign-bug) because they "look bad". This is both dishonest and
strategically wrong — the forensics demonstrate depth.

**Diagnostic example.** §6 Case 1 Shakespeare: "Our LoRA stack achieves 15/15 FSV passes." —
without mentioning the echo-bug that was diagnosed and fixed before those passes.

**Why fatal.** P10 violation. Also: anyone who reads the author's public repo sees the bug
and the fix; omitting it looks worse than disclosing it.

**Counter-behavior.** Each case-study subsection has a "Forensics and corrections" paragraph
describing the bug, the diagnostic method, and the fix. Pointer to the specific inspect/
document.

**Enforcer.** Agent-38, agent-39, agent-42.

---

## FM-09 — Ambiguous "Centroid" / "Constellation" Vocabulary

**Description.** The terms **centroid**, **constellation**, **anchor**, and **reference set**
are used interchangeably across sections without a canonical definition. Reviewer asks "what
is a centroid in TCT?" and gets different answers from §3 and §5.

**Diagnostic example.** §3 says "TCT anchors generation to a constellation of frozen
centroids". §5 says "the constellation is the set of reference embeddings". §6 Case 3 says
"the 50-clip centroid ..." Each section uses a different sense.

**Why fatal.** G2 Definition-Before-Use violation; also invites reviewer pedantry that
cascades.

**Counter-behavior.** Agent-04 fixes: a **centroid** is a vector (one per modality); a
**constellation** is the set of centroids across modalities; a **reference set** is the raw
clips from which a centroid is computed. Writers pull exactly these definitions.

**Enforcer.** Agent-04, agent-42.

---

## FM-10 — Citation Padding

**Description.** A writer adds citations to look well-read rather than to support specific
claims. Reviewer reads the reference list, flips back to find where Citation X is used, finds
it decorating a generic sentence.

**Diagnostic example.** "Large language models have advanced rapidly (Brown et al. 2020;
Chowdhery et al. 2022; Touvron et al. 2023; ...)." — no specific claim is being supported.

**Why fatal.** Makes the reference list look like name-dropping. Dilutes the signal of
genuinely load-bearing citations.

**Counter-behavior.** Every citation in the body supports a specific claim. If a writer
wants to survey, they cite ONE review paper; for specific technical claims they cite exactly
the paper that established that claim. Target: 30–60 refs, every one load-bearing.

**Enforcer.** Agent-44, agent-45, agent-42.

---

## Severity triage (when multiple failure modes are detected)

| Failure Mode | Severity | Blocks submission? |
|---|---|---|
| FM-01 Marketing Creep | CRITICAL | Yes — desk-reject risk |
| FM-03 AGI Overclaim | CRITICAL | Yes — desk-reject risk |
| FM-04 Measured/Projected Conflation | CRITICAL | Yes — scientific dishonesty |
| FM-02 DDA/Synthetic-Data Confusion | HIGH | Yes — core-construct failure |
| FM-08 Silent Failure Hiding | HIGH | Yes — forensics are load-bearing |
| FM-05 Single-Subject Sample-Size | HIGH | Yes — validity gate |
| FM-09 Centroid Vocabulary Drift | MEDIUM | Yes — G2 fail |
| FM-06 Code in Body | MEDIUM | Fixable in final pass |
| FM-07 Re-explaining Prior Draft | MEDIUM | Fixable by cutting |
| FM-10 Citation Padding | LOW | Fixable in citation QA |

**Rule.** Any CRITICAL or HIGH severity failure mode triggers rework before submission.
MEDIUM and LOW can be addressed in the final polish agents (42–48).
