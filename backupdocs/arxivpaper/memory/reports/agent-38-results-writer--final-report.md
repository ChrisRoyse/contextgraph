---
namespace: reports
agent_id: agent-38-results-writer
timestamp: 2026-04-17T13:00:00Z
status: active
tags: final-report, results, case-studies, systems, measured-projected, handoff-to-agent-39
---

# Final Report — Agent-38 (results-writer)

## 0. Task recap

Agent 38 of 48 in the arXiv manuscript pipeline. Responsible for
writing:

- §4 Production Systems (brief, 700–1,000 words)
- §6 Case Studies (combined §6.1/§6.2/§6.3/§6.4, 2,500–3,200 words)

covering the two production systems (Context Graph, ClipCannon) and
the three case studies (Shakespeare stylistic LoRA PROJECTED; Santa
talking-head PROJECTED; voice cloning MEASURED).

## 1. Files delivered

### Manuscript section files

1. **`/home/cabdru/contextgraph/docs/arxivpaper/sections/04_systems.md`**
   — §4 Production Systems. Header: `# 4. Production Systems`. Word
   count: 980. Subsections 4.1 (Context Graph), 4.2 (ClipCannon),
   4.3 (how the two systems instantiate the framework).

2. **`/home/cabdru/contextgraph/docs/arxivpaper/sections/07_case_studies.md`**
   — §6 Case Studies (combined). Header: `# 6. Case Studies`. Word
   count: 3,050. Subsections 6.1 Case 1 Shakespeare (projected; prior
   15/15 FSV anchor; echo-bug forensic), 6.2 Case 2 Santa (projected;
   flow-matching sign-bug forensic; constellation thresholds), 6.3
   Case 3 Voice (measured; headline 0.961 mean SECS with encoder-
   matched scope; temperature sweep; centroid ablation; three pending
   companion measurements), 6.4 summary across three cases.

### Memory files

3. **`/home/cabdru/contextgraph/docs/arxivpaper/memory/patterns/agent-38-results-writer--writing-decisions.md`**
   — writing decisions, measured/projected split ledger, hedges
   carried, banned-phrase audit, table discipline, content-boundary
   discipline with §3/§5, APA citation list.

4. **`/home/cabdru/contextgraph/docs/arxivpaper/memory/handoff/agent-38-to-39--results-ready.md`**
   — handoff to agent-39 (discussion-writer). Names the files to read,
   what §6 owns, what §7 should own, the seven discussion topics with
   evidence-to-result mapping, the full hedge carry-forward list, the
   banned-phrase list for §7, and the quality-gate status report.

5. **`/home/cabdru/contextgraph/docs/arxivpaper/memory/reports/agent-38-results-writer--final-report.md`**
   (this file).

## 2. Three-sentence summary

§4 covers Context Graph (~370K-line Rust, 13 embedders E1–E13, 59
RocksDB column families, 75 MCP tools, 5,184 tests) and ClipCannon
(~67,585-line Python, 23-stage DAG, 7 modalities × 4,044 dim, 58 MCP
tools) at a block-diagram level, with Leapable.ai named only as the
commercial exemplar downstream. §6 reports three graduated case
studies: Case 1 Shakespeare stylistic LoRA is PROJECTED end-to-end
with a prior-run 15/15 FSV MEASURED anchor and the training-data
echo-bug forensic; Case 2 Santa talking-head clone is PROJECTED
end-to-end with forensic integrity complete (flow-matching sign-bug
fix plus five companion blockers); Case 3 voice clone is MEASURED
(mean WavLM SECS 0.961, max 0.975, min 0.914, 9 of 10 $\geq$ 0.95;
DNSMOS parity 3.93/3.93; UTMOS 3.012/2.997; bootstrap CI
[0.951, 0.971]) on N=10 English sentences for one speaker under an
encoder-matched protocol, with three pending companion measurements
(cross-encoder ECAPA-TDNN triangulation, pre-selection distribution
over 120 candidates, $n_m$ curve at {10,25,50,100}) named for §8.
The measured-versus-projected split is therefore one T1 headline
case (Case 3) and two T3 projected cases (Cases 1 and 2), with every
number in both files tier-flagged in-line.

## 3. What went well

- **Exact number fidelity.** The Case 3 headline values (0.961 /
  0.975 / 0.914 / 9 of 10 / 3.93 / 3.012 / 2.997) were verified
  against `tasks/newdata/alldata.md` lines 1450–1525 and against
  agent-28 §4.3 before insertion, and reproduced verbatim.
- **Hedge discipline.** Every T1 MEASURED number in Case 3 carries
  its mandatory companion sentence per agent-34 §2 templates (C5 for
  the SECS headline, C6 for the naturalness parity, C7 for T=0.3,
  C8 for centroid size, C11 for Case 1, C9/C10 for Case 2).
- **Content boundary.** §6 does not re-narrate §3 DDA identity or §5
  TCT phases; it instantiates them. Cross-checked against
  `sections/03_framework_dda_mc.md` and `sections/06_method_tct.md`
  before writing.
- **Forensic findings cited by doc path.** Case 1 points to
  `docs2/LORA_STYLE_TRANSFER_COMPLETE_REPORT.md`; Case 2 points to
  `docs/inspect/08_training_audit.md` in the Santa project. Both are
  real paths the reviewer can verify.
- **Banned-phrase list clean.** No "breakthrough", "revolutionary",
  "game-changing", "the best", "solves AGI", or any of the agent-34
  §4 items.

## 4. What limitations carry forward

- **§6.2 Case 2 is narrative-heavy by necessity.** No T1 numbers
  exist for it, so the subsection is forensic-integrity + projected
  thresholds + projected ablations. The reviewer may push back that
  §6.2 reads as methodology rather than results; the mitigation is
  the explicit "PROJECTED end-to-end" tag at §6.2.1 and the
  italicisation of all quantitative operating points. Agent-39 and
  agent-42 should treat §6.2 as signalling architectural readiness,
  not as a source of interpretable result values.
- **Case 3 carries critical-severity validity threats.** VT-I2, VT-I3,
  VT-C1, VT-S4 share the same root cause (encoder-matched
  self-consistency plus missing pre-selection distribution). The §6.3
  prose carries all four as single-sentence scope disclosures and
  names the three pending companion measurements as the remediation
  path. If any of the three lands before camera-ready submission, the
  headline framing should be promoted accordingly; until they do, §8
  carries the within-WavLM-family scope.
- **Temperature-sweep reconciliation.** Table 1 (§6.3.3) reports the
  sweep mean at $T=0.3$ as 0.932 from the sweep-internal run, while
  the paper-level evaluation run at the same operating point is
  0.961. Both are MEASURED on the same 10 sentences but with
  different seed chains. I elected to report both values with their
  measurement contexts rather than averaging; §6.3.3 explicitly
  states the two values come from different runs at the same
  operating point.

## 5. Confidence

**HIGH** on §4 coverage of both systems at block-diagram level under
1,000 words, with Leapable confined to the acknowledgements per the
commercial-pitch-free discipline.

**HIGH** on §6 word count (3,050 within 2,500–3,200 target) and on
the table budget (three tables: temperature sweep, centroid
ablation, case summary).

**HIGH** on MEASURED/PROJECTED tier-flagging. Every Case 3 number
carries its source (alldata.md / SITREP / agent-28) and its
encoder-matched scope; every Case 1 and Case 2 number is italicised
or carries "T3 projected".

**HIGH** on banned-phrase compliance and hedge carry-forward per
agent-34 §2 templates.

**MEDIUM** on whether the handoff to agent-39 adequately constrains
the discussion against over-interpretation: §7 Discussion is the
most likely place for the encoder-matched scope to be accidentally
relaxed, and for the C12a/C12b sample-complexity split to be
smoothed. The handoff file explicitly flags both as mandatory
continuations.

**MEDIUM** on agent-42 adversarial-review exposure: the two review
hotspots are (a) whether the Case-2 T3 framing reads as projected
(extensive italics should handle this), and (b) whether the Case-3
encoder-matched scope is prominent enough (repeated at §6.3.2 and
§6.3.5 item 1).

## 6. XP self-report (per agent role-card)

50 XP reward claimed for "Completed rigorous results section with
full statistical reporting under the measured-vs-projected
discipline".

## Changelog

- 2026-04-17 — Initial final report by agent-38-results-writer.
