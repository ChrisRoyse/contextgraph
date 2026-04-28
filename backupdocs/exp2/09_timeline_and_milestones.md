# EXP-2 Timeline and Milestones

**Where EXP-2 sits in the 14-day action list.** Day 7 is kickoff, Day 14 is the patent-filing deadline, Day 21 is the EXP-2 internal deadline. This document maps action-list dates to concrete EXP-2 deliverables.

---

## 1 · Alignment with the 14-day action list

EXP-2 is **ACTION #3**. Kickoff is Day 7 (2026-04-30, Thursday — "Technical Day"). Per the action list §Failure Conditions: "Do NOT delay Perez send waiting for EXP-2" — the Perez memo (ACTION #8, Day 10) goes without EXP-2 results; EXP-2 follows as a revision.

---

## 2 · Day-by-day plan

### Day 0 · 2026-04-23 (Thursday, today) — DRAFT phase
- ✅ Draft this documentation package under `docs/exp2/` (this task)
- [ ] Chris reviews all 17 files
- [ ] Steve reviews style and compliance language
- [ ] Open GitHub issue `exp2-implementation` tracking the Rust binary + Python scripts

### Day 1 — Day 3 · 2026-04-24 — 2026-04-26 — Preparation and Q1–Q7
- ACTION #1 in progress (Q1–Q7 answers). No EXP-2-specific work; reviewer capacity held for Day 7 kickoff.
- Chris reads the 17-file package in its entirety; raises any ambiguity as dated amendment candidates.

### Day 4 · 2026-04-27 (Monday) — Legal Day
- ACTION #2 (counsel retained). Chris briefs counsel on EXP-2 as input to patent claim support. Counsel may flag publication timing concerns (pre-freeze publication restricts patent scope); amendment applied if needed.

### Day 5 · 2026-04-28 (Tuesday)
- **Rust corpus-build binary draft**: Chris implements `context-graph-exp2 build-corpus` end-to-end
- **Python scripts scaffolding**: `scripts/exp2/lib/{mine,infonce,ksg}.py` with unit tests passing on synthetic Gaussians
- **Dry run on $n = 10$**: validates pipeline end-to-end (corpus → PCA → controls → 3 synthetic pairs)

### Day 6 · 2026-04-29 (Wednesday) — FREEZE DAY
- **Morning**: finalize all 17 `docs/exp2/*.md` files. Chris + Steve sign-off.
- **Mid-day**: freeze preregistration. `git tag v0.exp2.preregistered`. Compute and record the SHA in [01_preregistration.md](01_preregistration.md) Appendix B.
- **Afternoon**: run the full corpus build at $n \in \{100, 500, 1000\}$ + the three sensitivity corpora.
- **Evening**: compute and record all artifact sha256 in `docs/exp2/.corpus_manifest`.
- **End-of-day goal**: corpus frozen, all hashes logged.

### Day 7 · 2026-04-30 (Thursday) — KICKOFF DAY (ACTION #3 per action list)
- **Morning**: run controls on $n = 1000$. Must pass all three. If any fails, halt + diagnose.
- **Afternoon**: start MINE + InfoNCE sweep on $n = 100$ (pilot). Wall-clock ~1 hr. Inspect convergence curves.
- **End-of-day**: pilot results in hand; decide if full $n = 500$ sweep is safe to run overnight.

### Day 8 · 2026-05-01 (Friday) — Structure Day
- **Overnight results** (from Day 7 into Day 8): $n = 500$ MINE + InfoNCE done (8 hr). Morning review.
- **Start**: $n = 1000$ MINE sweep. 24 hr wall-clock; dispatches to GPU overnight through Day 9.
- ACTION #5 and #10 proceed in parallel (co-founder agreement + compliance gate); EXP-2 runs in background.

### Day 9 · 2026-05-02 (Saturday)
- **EXP-2**: $n = 1000$ MINE completing. Start InfoNCE sweep once MINE finishes.
- **Chris checks convergence curves** spot-checks; does not make decisions on partial data.

### Day 10 · 2026-05-03 (Sunday) — Perez Day
- **ACTION #8 executes**: Perez memo sent (Day 10). **Without EXP-2 numbers.** The memo cites 91 as constructive ceiling; footnotes "realized floor measurement in progress; results in ~10 days."
- **EXP-2**: InfoNCE sweep running.

### Day 11 · 2026-05-04 (Monday)
- **EXP-2**: InfoNCE finishing. Start KSG on the 12 pre-registered pairs.
- KSG completes in < 30 min.
- **Bootstrap CIs** computed (~10 min).

### Day 12 · 2026-05-05 (Tuesday)
- **EXP-2**: sensitivity corpus runs start (Wikipedia-5k, Code-5k, Shakespeare-2k). Each ~8 GPU-hr in parallel where possible; serial otherwise = 24 GPU-hr.

### Day 13 · 2026-05-06 (Wednesday)
- **EXP-2**: sensitivity runs completing.
- **Aggregation**: compute $N_\text{eff}$ point estimates + CIs for production and sensitivity corpora.

### Day 14 · 2026-05-07 (Thursday) — PATENT DAY (ACTION #6)
- **ACTION #6 executes**: patent filed. EXP-2 may be referenced in the provisional as a to-be-reported measurement; the realized-floor number is not required for the filing.
- **EXP-2**: report generation begins. All figures, tables, per-pair flags in place by EOD.

### Day 15 · 2026-05-08 (Friday)
- **Chris**: full report review. Spot-checks of convergence curves, verifies all 78 pairs have clean flags or explicit disagreement notes.
- **Flag re-runs**: any pair with `DISAGREE_STRONG` or `MINE_NONCONV` gets a 4000-step re-run (extended from 2000). Each re-run ~5 min.

### Day 16–17 · 2026-05-09 — 2026-05-10 (Saturday, Sunday)
- **Steve**: compliance-gate review of reporting language. Every sentence in §2 (Headline), §8 (Discussion), §9 (What the number means) checked against [07_reporting_template.md](07_reporting_template.md) §12 rules.
- **Chris**: revisions applied.

### Day 18 · 2026-05-11 (Monday)
- **Dry run**: generate Perez-memo-revision paragraph (~200 words) and patent-disclosure paragraph (~500 words).
- **Figures finalization**: all PDFs rendered at publication quality.

### Day 19 · 2026-05-12 (Tuesday)
- **Final internal review**: Chris + Steve sign off on the full report.
- **Amendment log**: any post-freeze amendments formally recorded.

### Day 20 · 2026-05-13 (Wednesday)
- **Commit final report**: `exp2_report_n1000.md` and `.pdf` committed to `exp2_artifacts/report/`.
- **Manifest finalization**: all sha256 hashes in the report header match `.artifact_manifest`.
- **Chris drafts the Perez follow-up**: memo revision incorporating EXP-2 results.

### Day 21 · 2026-05-14 (Thursday) — INTERNAL DEADLINE (ACTION #3 per action list §14-day list)
- **EXP-2 COMPLETE** per internal deadline.
- **ACTION #9 executes** (same day by coincidence): Dohmatob follow-up with EXP-2 results + co-author offer for replicative extension.
- **If results are Strong or Moderate**: Perez memo revision sent (with PDF by reply if Perez has responded since Day 10).
- **If Weak**: pitch reframing in progress; revision delayed 1 week while narrative shifts per [13_failure_mode_playbook.md](13_failure_mode_playbook.md).

### Day 22+ — Post-EXP-2
- Perez memo revision sent (Strong / Moderate) or reframed pitch sent (Weak)
- Patent counsel gets the EXP-2 report as supporting material for the non-provisional filing window (12 months from Day 14 provisional)
- If NeurIPS accepted (Q4 2026): EXP-2 + report joins the supplementary materials
- If rejected: EXP-2 + preregistration SHA + corpus hashes posted to arXiv as reproducibility artifact

---

## 3 · Gating criteria between phases

| Transition | Gate | If gate fails |
|------------|------|---------------|
| Day 6 freeze → Day 7 kickoff | All 17 docs signed off by Chris + Steve | Extend freeze by 1 day; note amendment |
| Day 7 controls → Day 7 pilot sweep | 3 of 3 controls pass per [01_preregistration.md §5](01_preregistration.md) | Halt, diagnose, amend, re-run before sweep |
| Day 8 pilot → Day 8 primary sweep | $n=100$ results show stable convergence curves, no systemic flag explosion | Delay $n=500$ sweep by 1 day; re-examine critic architecture |
| Day 11 sweep done → Day 12 sensitivity | Estimator agreement within criteria ([04_estimator_specifications.md §6](04_estimator_specifications.md)) | Halt sensitivity runs; investigate pair disagreements |
| Day 19 report → Day 20 commit | Chris + Steve both sign off | Do not commit; revise |
| Day 20 commit → Day 21 external (Perez rev / Dohmatob) | Compliance-gate language check passes | Do not send; revise |

---

## 4 · Contingency — if EXP-2 slips

Per action list §Failure Conditions:

> "If EXP-2 fails to complete by Day 21:
> Impact: Perez Week 3–6 engagement window proceeds without EXP-2 result; reduced signal strength.
> Mitigation: Send Perez memo Day 10 WITHOUT EXP-2; follow up with results when ready. Do NOT delay Perez send waiting for EXP-2."

If EXP-2 isn't done by Day 21:
- Do not delay patent (Day 14), Dohmatob follow-up (Day 14), or any other action-list item.
- Perez memo revision sent when EXP-2 does complete. Target extension: up to Day 28. After Day 28, halt and publish what we have (even a partial result is better than nothing in the 14-day extension window).

---

## 5 · Reporting cadence during execution

| Frequency | Channel | Content |
|-----------|---------|---------|
| Daily (Days 7-21) | Steve update, 3 lines | "EXP-2 phase X done. Flags: 0. Blockers: none." |
| On any halt condition | Chris writes a failure memo | Root cause + amendment + restart plan |
| On completion | Full report (markdown + PDF) | Per [07_reporting_template.md](07_reporting_template.md) |

Daily updates use the existing SITREP pattern (see `tmp/SITREP_*`). Halts and final report use dedicated files under `docs/exp2/`.

---

## 6 · Who owns what

| Role | Chris | Steve |
|------|-------|-------|
| Protocol draft (Days 0–6) | primary | review |
| Preregistration freeze | executes | co-sign |
| Rust bin + Python scripts | primary | (n/a unless coding help needed) |
| Corpus build + controls + sweeps | primary | (n/a) |
| Per-pair reviews + flag triage | primary | (n/a) |
| Reporting language | draft | compliance gate (required for sign-off) |
| Amendment log | primary | co-sign |
| External surface (Perez / Dohmatob / patent counsel) | primary | compliance review |

---

## 7 · Day-0 punch list (this session)

- [x] Plan directory structure for `docs/exp2/`
- [x] Draft all 17 `.md` files (in-progress)
- [ ] Chris reviews all 17 files
- [ ] Steve style / compliance review of reporting-sensitive docs: 00, 01, 06, 07, 12, 13
- [ ] Open `exp2-implementation` GitHub issue (optional; the action list has it as out-of-scope for Day 0)
- [ ] Commit the documentation to git on Day 1 morning (2026-04-24) with explicit commit message: `docs(exp2): pre-preregistration draft package for 14-day action-list ACTION #3`
