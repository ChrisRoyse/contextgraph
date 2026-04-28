---
namespace: patterns
agent_id: agent-48-adaptive-orchestrator
timestamp: 2026-04-17T16:05:00Z
status: active
locked: false
supersedes: none
tags: coordinator-todo, prioritised-checklist, stop-ship, assembly-readiness, paste-ready-pointers
depends_on:
  - memory/discoveries/agent-48-adaptive-orchestrator--final-health-assessment.md
  - memory/patterns/agent-42-adversarial-reviewer--stop-ship-and-edits.md
  - memory/patterns/agent-43-reproducibility-checker--reproducibility-appendix-draft.md
  - memory/patterns/agent-46-file-length-manager--split-recommendations.md
downstream_consumers:
  - coordinator (PAPER.md assembly)
answers_questions: [what-coordinator-must-do-before-assembly, what-can-wait, what-is-optional]
---

# Prioritised Coordinator To-Do — Before `PAPER.md` Assembly

## 0. How to use this file

Three priority buckets:

- **MUST** — block arXiv v1 submission. Apply before first
  `build.sh` run.
- **SHOULD** — strongly recommended before v1; safe to defer to
  camera-ready (i.e., a v2 arXiv revision) if time-compressed.
- **NICE** — cosmetic; apply when time allows.

Each item names the paste-ready source in agent-42 / agent-43 /
agent-46 memory files so edits can be applied without re-reading
the full discovery/pattern chain.

Estimated total MUST-list time: **~4–6 hours of coordinator editing**
plus **1 × 10-minute author consultation** on the two-run question
(R2 / MUST-2 below).

---

## 1. MUST — fix before first `PAPER.md` assembly

### MUST-1 — Resolve §6.3.4 Table 2 source mismatch (R1, CRITICAL)
**File**: `sections/07_case_studies.md` §6.3.4 Table 2.
**Action**: Replace the current Table 2 (which mixes headline
0.961/0.975 with ablation-internal 0.924/0.946) with agent-43
§15.8 Option A — report ablation-internal verbatim (1-clip row
0.924 / 0.946; 50-clip row 0.939 / 0.950) as the enrollment-
strategy ablation, and present the headline (0.961/0.975) in a
separate row or sentence labelled "separate headline evaluation
run at same operating point". The Δ=+0.015 claim is preserved
(ablation-internal 0.939 − 0.924 = +0.015).
**Source**: `memory/patterns/agent-43-reproducibility-checker--
reproducibility-appendix-draft.md` §15.8 (Option A table).
**Time**: 15 minutes.

### MUST-2 — Author consultation + STOP-3 §6.3.3 fix (R2, CRITICAL)
**File**: `sections/07_case_studies.md` §6.3.3 (Table 1 +
caption + surrounding paragraph).
**Author consultation (10 min)**: does an archived second seed
chain exist for the 0.961 headline run at T=0.3 that is distinct
from the seed chain the sweep used? If YES → apply agent-42
STOP-3 paste-edit verbatim + add archive pointer to §6.3.2.
If NO → rewrite §6.3.3 without the "two separate runs" framing;
present the sweep at T=0.3 (mean 0.932) as the primary
temperature-sweep result and the 0.961 headline as the
paper-level evaluation without asserting independence of seed
chains.
**Source**: `memory/patterns/agent-42-adversarial-reviewer--
stop-ship-and-edits.md` §2.3 (STOP-3 paste-edit). Agent-43 audit
§4.2.2 (alternative framing).
**Time**: 20 minutes edit + 10 minutes author sync.

### MUST-3 — Fix §6.3.2 N=14 DNSMOS/UTMOS ambiguity (R9, MEDIUM but quick)
**File**: `sections/07_case_studies.md` §6.3.2 lines 75–76.
**Action**: Replace the DNSMOS and UTMOS parity lines with:
> DNSMOS P.808 (Reddy et al., 2022): clone mean **3.93** across
> 14 cloned generations (10 held-out + 4 calibration-session
> recordings), versus single real-microphone reference at
> **3.93** (MEASURED; descriptive parity at 3 sig-fig; no formal
> equivalence test). UTMOS (Saeki et al., 2022): clone mean
> **3.012** (N=14) versus real recording **2.997** (N=1; clone
> marginally higher by +0.015; no formal equivalence test).
**Source**: agent-43 appendix draft §15.9 (paste-ready sentence).
**Time**: 5 minutes.

### MUST-4 — STOP-8 global "camera-ready" replacement (R4, HIGH)
**Files**: 6 files contain the string — `00_frontmatter.md` (1),
`01_introduction.md` (1), `06_method_tct.md` (1),
`07_case_studies.md` (7), `08_discussion.md` (1),
`09_limitations.md` (2). Total 13 occurrences.
**Action**: Global replace `camera-ready will re-verify` →
`a subsequent version of this manuscript will re-verify`.
`for the camera-ready version` → `for a subsequent version`.
Also delete the "Track A / Track B" paragraph in §8.L3 per
agent-42 STOP-8 (b).
**Source**: `memory/patterns/agent-42-adversarial-reviewer--
stop-ship-and-edits.md` §3 STOP-8.
**Time**: 10 minutes (single sed) + 5 min to delete §8.L3 Track
paragraph + 10 min to rewrite §8 final paragraph confidence-delta
enumeration per STOP-8 (c).

### MUST-5 — STOP-4 Remove Leapable.ai commercial terms from §4 body (R7, MEDIUM)
**File**: `sections/04_systems.md` §4.3 paragraph 3.
**Action**: Apply agent-42 STOP-4 paste-edit verbatim — delete
the Stripe Connect 70/30 / 75/25 / 5% reserve / HMAC sentence
from body; replace with the structural sentence ("Leapable.ai —
an MCP marketplace operated by the author — is named in the
Acknowledgements..."). Relocate the commercial-terms sentence
to `sections/00_frontmatter.md` or a new `Acknowledgements`
section per agent-42 (see MUST-6).
**Source**: agent-42 stop-ship-and-edits §2.4.
**Time**: 15 minutes.

### MUST-6 — Create §Competing Interests section (R6, HIGH / STOP-6)
**File**: new file — `sections/12_competing_interests.md`
(placed after `10_conclusion.md` in `build.sh` order;
before `11_references.md`).
**Action**: Paste agent-42 §Competing Interests block verbatim.
Then REWRITE §8.L8 as a technical limitation (agent-42 provides
paste-ready rewrite) so the commercial-COI content lives in
Competing Interests, not Limitations.
**Source**: agent-42 stop-ship-and-edits §3 STOP-6 (both
paste-ready blocks).
**Time**: 20 minutes (include updating `build.sh` concatenation
order).

### MUST-7 — Create §Ethics section (R6, HIGH)
**File**: new file — `sections/13_ethics.md` (placed after
§Competing Interests, before References).
**Action**: Paste agent-31 locked ethics statement. Add IC-6
three-tier asymmetric release posture (byte-identical /
protocol-level / protocol-level) as a subsection. Cross-
reference from §6.1.1, §6.2.1, §6.3.1 (one sentence per case).
**Source**: `memory/decisions/agent-31-ethics-reviewer--*.md`
(locked); IC-6 content reproduced in agent-43 appendix §15.1.
**Time**: 30 minutes (import + compress + add per-case
crosslinks).

### MUST-8 — Create §15 Reproducibility appendix (R5, HIGH / C5)
**File**: new file — `sections/15_reproducibility.md` (placed
after §Ethics, before References).
**Action**: Import agent-43 appendix draft (1,700 lines).
Compress to ~400 lines / ~6–10 formatted pages. Core blocks
(do NOT drop): §15.1 release posture, §15.2 per-case
hyperparameters, §15.3 embedder/scorer version manifest, §15.5
Case-3 two-session structure, §15.8 Table 2 reconciliation,
§15.9 N=14 disambiguation, §15.10 bootstrap CI, §15.12
alldata.md crosswalk. **Author fills SHA-256 placeholders at
submission time.**
**Source**: `memory/patterns/agent-43-reproducibility-checker--
reproducibility-appendix-draft.md` §15.1–§15.15.
**Time**: 45–60 minutes (import + compress + fact-check author
placeholders).

### MUST-9 — STOP-7 pending-measurement justifications (R8, MEDIUM)
**File**: `sections/07_case_studies.md` §6.3.5.
**Action**: Apply agent-42 STOP-7 paste-edits BUT with agent-43
correction — weaken the ECAPA "licensing" reason to "time
constraints of v1 submission" per agent-43 §7.4 (SpeechBrain's
ECAPA is Apache-2.0, so "licensing" is not a defensible reason).
**Source**: agent-42 stop-ship-and-edits §3 STOP-7 (as a base);
agent-43 audit §7.4 (for the corrected rationale).
**Time**: 20 minutes.

### MUST-10 — Label IC-6 asymmetric release in §6.1.1 / §6.2.1 / §6.3.1 (C4, MEDIUM)
**File**: `sections/07_case_studies.md`.
**Action**: Add one sentence per case announcing the release
posture: "Case 1 is byte-identically reproducible on a released
LoRA stack (see §13 Ethics and §15 Reproducibility)." Case 2 and
Case 3 get parallel sentences with "protocol-level" wording.
**Source**: agent-43 audit §6.2 items 1–3 (paste-ready).
**Time**: 10 minutes.

### MUST-11 — Re-verify STOP-1 and STOP-2 (CRITICAL but possibly already fixed)
**Files**: `sections/06_method_tct.md` §5.4; `sections/07_case_studies.md`
§6.3.2.
**Action**: Read the §5.4 predicate paragraph (STOP-1) and §6.3.2
headline paragraph (STOP-2) to confirm agent-42's paste-ready
fixes are applied. If NOT applied, paste from agent-42
stop-ship-and-edits §2.1 (STOP-1) and §2.2 (STOP-2). Agent-47
marked these "UNKNOWN — requires reading"; I inherited that
status.
**Source**: agent-42 stop-ship-and-edits §2.1, §2.2.
**Time**: 15 minutes re-verify + up to 20 minutes apply if needed.

### MUST-12 — Apply length compression (R3, HIGH)
**File**: multiple (§1, §2, §5, §6, §7).
**Action**: Apply agent-46 compression targets P1–P5:
- P1 (§6 Cases): compress PROJECTED Cases 1/2; preserve
  MEASURED Case 3 — save ~600–800 words.
- P2 (§5 TCT Method): factor out §5.5/§5.6 overlap with
  §5.2–§5.4 — save ~300–400 words.
- P3 (§2 Related Work): tighten §2.6 gap restatement —
  save ~300–500 words.
- P4 (§7 Discussion): de-duplicate §7.4 scope vs §3.3 —
  save ~200–400 words.
- P5 (§1 Introduction): general 10–15% trim — save ~150–250
  words.
Additional ~1,200 words must come from coordinator editorial
judgement (suggested: trim §4 Systems further; fold §7.7
forensic section into §6.2 Case 2 with cross-reference; tighten
§9 conclusion).
**Source**: `memory/patterns/agent-46-file-length-manager--
split-recommendations.md`.
**Time**: 90–120 minutes (this is the single largest
coordinator-editorial lift).

---

## 2. SHOULD — strongly recommended but deferrable

### SHOULD-1 — Apply agent-42 HIGH findings beyond stop-ship (Findings 1.2, 1.3, 2.3, 3.1, 3.2, 5.2, 5.4, 6.3, 6.5, 6.7, 8.2, CA-2, CA-9)
**File**: various.
**Action**: 13 paste-ready edits supplied in agent-42
stop-ship-and-edits §4. None individually load-bearing; together
they harden the paper against determined review pressure.
**Time**: 90–120 minutes.

### SHOULD-2 — Apply agent-46 additional ~1,200-word trim
**File**: coordinator discretion.
**Action**: After MUST-12 completes P1–P5, a further editorial
trim is needed to hit ≤30 pages two-column (450 w/p). Target:
§4 Systems further compression + §7.7 fold + §9 tighten.
**Time**: 30–60 minutes (absorbed partly into MUST-12's time
budget).

### SHOULD-3 — Add weight SHA-256 placeholders to appendix table + commit to author-fill at submission
**File**: `sections/15_reproducibility.md` §15.3.
**Action**: Either fill SHA-256 at submission time or explicitly
label table as "author to fill at final submission". Shipping
TBD placeholders in a v1 is acceptable if clearly labelled but
weakens reproducibility posture.
**Time**: 10 minutes (labelling) or variable (fill).

### SHOULD-4 — Global replace "88 pool" / "locked 88-citation pool" per agent-42 Finding 1.2
**File**: §1, §3.4, §5.4, §7.3.
**Action**: Introduce pool once as footnote in §1 paragraph 2;
subsequent occurrences → "within the reference set" or delete.
**Time**: 15 minutes.

### SHOULD-5 — Apply agent-47 B1 role-label correction in handoff files (cosmetic)
**File**: `memory/handoff/agent-43-to-44--*.md`,
`agent-44-to-45--*.md`, `agent-45-to-46--*.md`.
**Action**: Replace "agent-47 (final-assembler)" with "agent-47
(graph-gardener)". Does not affect `PAPER.md`. This is a
memory-graph tidy.
**Time**: 10 minutes.

---

## 3. NICE — cosmetic

### NICE-1 — Apply agent-42 MEDIUM findings (19 items)
**File**: various.
**Action**: 19 MEDIUM fixes enumerated in agent-42 critique §4.
None blocking. Coherence-checker can batch.
**Time**: 60–90 minutes.

### NICE-2 — Apply agent-42 LOW findings (3 items)
**File**: various.
**Action**: Cosmetic hedging fixes in §2.1, §7.1, §9.1.
**Time**: 15 minutes.

### NICE-3 — Replace "third path" → "third configuration" in-body (preserve in title)
**File**: §1, §3.3, §7.6, §9 (5+ in-body occurrences).
**Action**: per agent-42 Finding 1.5 recommendation. Keep title
as locked ("Third Path Around the Data Wall" — mission brief).
**Time**: 10 minutes.

### NICE-4 — Replace "honest" section heading in §7.7
**File**: `sections/08_discussion.md` §7.7.
**Action**: Rename §7.7 "Honest failure modes" → "Engineering
findings from the case record" per agent-42 Finding 7.5.
**Time**: 5 minutes.

### NICE-5 — File retroactive blocker-discipline entries in `memory/blockers/`
**Action**: Create one blocker file per stop-ship item (STOP-1
through STOP-8) documenting resolution status. Improves memory-
graph forensics for future maintainers. NOT needed for arXiv
submission.
**Time**: 30 minutes.

---

## 4. Order-of-operations (recommended coordinator workflow)

1. **(10 min)** Re-read agent-42 stop-ship-and-edits.md + agent-
   43 appendix draft §§15.1–15.15.
2. **(10 min)** Author consultation on MUST-2 (archived second
   seed chain — yes/no).
3. **(MUST-11, MUST-1, MUST-3)** Re-verify / fix §5.4, §6.3.2,
   §6.3.4 — in order — ~40 min.
4. **(MUST-4)** Global "camera-ready" + §8.L3 + §8 final rewrite
   — ~25 min.
5. **(MUST-5, MUST-9, MUST-10)** §4 / §6.3.5 / §6.1.1 / §6.2.1 /
   §6.3.1 edits — ~45 min.
6. **(MUST-6, MUST-7)** Create §Competing Interests + §Ethics —
   ~50 min; update `build.sh` concatenation order.
7. **(MUST-8)** Create §15 Reproducibility appendix (import +
   compress) — ~50 min.
8. **(MUST-12)** Length-compression pass — ~90–120 min.
9. **(SHOULD-1 through SHOULD-5)** if time permits — ~2 hours.
10. **Assemble `PAPER.md`** via `build.sh`. Verify it typesets
    under 30 pages.
11. **(NICE-*)** as time allows before camera-ready revision.

**Total MUST-list**: ~4.5–6 hours of editing + 10-min author
consult.

---

## 5. What NOT to do

- **Do NOT** ship without resolving MUST-1 (Case-3 Table 2
  source mismatch). This is the single finding most likely to
  produce a reviewer-level or moderator-level challenge.
- **Do NOT** apply agent-42 STOP-3 paste-edit verbatim without
  the author consultation in MUST-2 — it asserts a claim agent-43
  flagged as unsupported by the source.
- **Do NOT** delete the 19 MEDIUM and 3 LOW agent-42 findings
  from memory if deferring; file a `decisions/` note documenting
  deferral reason.
- **Do NOT** re-verify citations — agent-45 validated all 88 with
  HIGH confidence. Re-verification is wasted effort unless a
  specific citation is touched during editing.
- **Do NOT** edit section files without reading the paste-ready
  source first. Most edits are already composed verbatim in
  agent-42 / agent-43 pattern files.

---

## 6. Assembly-time verification checklist

After running `build.sh`:

- [ ] `PAPER.md` concatenates all 11 body sections + §Competing
      Interests + §Ethics + §15 Reproducibility + §References in
      the correct order.
- [ ] `grep -c camera-ready PAPER.md` returns 0.
- [ ] `grep -c "Stripe Connect" PAPER.md` returns 0 in §4 body
      (may return 1 if moved to Acknowledgements).
- [ ] `grep -Ec '^\s*## (Ethics|Competing Interests|15\.? Reproducibility)' PAPER.md`
      returns 3.
- [ ] `wc -w PAPER.md` returns ≤ ~14,000 words (body + appendices,
      allowing ~2K words for the repro appendix).
- [ ] `pandoc --metadata=title:"..." PAPER.md -o PAPER.pdf` (or
      LaTeX-equivalent) produces a PDF under 30 pages.
- [ ] Spot-check Case-3 Table 2 reports ablation-internal values
      (0.924 / 0.946 → 0.939 / 0.950) AND the headline values
      (0.961 / 0.975) in distinct rows.
- [ ] Spot-check §6.3.2 headline sentence does NOT bold the
      scope tag separately from the number (agent-42 STOP-2).
- [ ] Spot-check §5.4 predicate paragraph folds "relative to Φ
      and C_P" into the predicate definition itself (agent-42
      STOP-1).

---

## 7. Confidence

- **HIGH** on the MUST list being sufficient to move the paper
  from CONDITIONAL GO to GO.
- **HIGH** on time estimates (based on agent-42's own 2.5–3 hour
  estimate for STOP-fixes, plus ~1.5 hours for §15 + §Ethics +
  §Competing-Interests + IC-6 labels, plus ~1.5 hours for length
  compression, plus 10 min author sync).
- **MEDIUM** on whether the final typeset PDF lands under 30
  pages after MUST-12 alone — the worst-case estimate is ~35
  pages after full P1–P5, and the additional ~5-page cut
  (SHOULD-2) is judgment-heavy. Coordinator should expect at
  least two compression rounds.
- **HIGH** that NICE items are genuinely optional for arXiv v1.

## 8. Changelog

- 2026-04-17 — Initial coordinator to-do list by agent-48-adaptive-
  orchestrator.
