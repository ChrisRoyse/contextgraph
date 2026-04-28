---
namespace: patterns
agent_id: agent-05-research-planner
timestamp: 2026-04-16T16:15:00Z
status: active
tags: rewoo, master-plan, section-map, evidence-sources, dag, timeline, risks
depends_on:
  - memory/context/mission-brief.md
  - memory/context/agent-order.md
  - memory/context/constraints.md
  - memory/patterns/agent-01-step-back-analyzer--evaluation-criteria.md
  - memory/patterns/agent-01-step-back-analyzer--guiding-principles.md
  - memory/decisions/agent-01-step-back-analyzer--failure-modes-to-avoid.md
  - memory/decisions/agent-02-ambiguity-clarifier--canonical-glossary.md
  - memory/decisions/agent-03-self-ask-decomposer--essential-questions.md
  - memory/patterns/agent-04-construct-definer--definition-blocks-for-writers.md
  - memory/decisions/agent-05-research-planner--section-ownership.md
  - memory/patterns/agent-05-research-planner--agent-tasking-table.md
  - memory/patterns/agent-05-research-planner--quality-gates.md
---

# ReWOO Master Research Plan

**Reasoning WithOut Observation.** Every task on the critical path is specified upfront:
task list, dependencies, evidence sources, quality gates, timeline, risk register. The
downstream pipeline (agents 06–48) executes this plan; it does not re-plan.

This document is the narrative companion to three load-bearing sibling files:

- `agent-tasking-table.md` — compact per-agent tasking (one row per agent 06–48).
- `section-ownership.md` — which agent writes which section (locked).
- `quality-gates.md` — eight intermediate checkpoints + nine final gates.

Together, these four files are the operating plan.

---

## 0. Scope reminders (so the rest of the document is not surprising)

- **Target venue.** arXiv cs.LG primary, cs.CL + cs.CV cross-listings.
- **Working title.** *Teleological Constellation Training: Multi-Modal Embedding
  Decomposition as Meaning Compression for Identity-Locked Generation and a Third Path
  Around the Data Wall.*
- **Author.** Chris Royse (non-anonymised, as arXiv requires).
- **Length.** 7,500–12,500 prose words; ≤ 30 typeset pages; 30–60 references.
- **Structure (locked).** Abstract; §1 Introduction; §2 Related Work; §3 Framework (DDA +
  meaning compression); §4 Systems (Context Graph + ClipCannon); §5 TCT method; §6 Case
  Studies (three); §7 Discussion; §8 Limitations; §9 Conclusion; Acknowledgements;
  References.
- **Three case studies.** Case 1 Shakespeare stylistic LoRA (T2 + T3); Case 2 Santa
  identity-locked talking head (T2 + T3); Case 3 voice cloning (T1, the measured one).
- **Four core claims.** DDA; meaning compression; TCT; sample-complexity collapse.
- **Non-negotiable anti-patterns.** FM-01 marketing; FM-02 DDA-as-synthetic-data; FM-03
  AGI overclaim; FM-04 measured/projected conflation; FM-05 hidden single-subject;
  FM-06 code-in-body; FM-07 re-explaining prior draft; FM-08 silent failure hiding;
  FM-09 centroid/constellation drift; FM-10 citation padding.

---

## 1. Section map — which agent writes which section

Locked in `section-ownership.md`. Recap:

| Section | Primary writer | Word budget | Supplier agents whose memory the writer reads |
|---|---|---|---|
| Abstract | agent-41 | 180–250 | all prior sections |
| §1 Introduction | agent-35 | 900–1,200 | 01–04, 15, 17 |
| §2 Related Work | agent-36 | 1,200–1,800 | 06, 07, 08, 10, 11, 15, 17, 18 |
| §3 Framework | agent-37 | 1,200–1,600 | 04, 21, 22 |
| §4 Systems | agent-37 | 700–900 | 04, 09, 10 |
| §5 TCT Method | agent-37 | 1,100–1,400 | 04, 25, 27, 28 |
| §6 Case Studies | agent-38 | 1,500–2,000 | 25, 26, 27, 28, 34 + SITREP + prior case files + Santa `docs/inspect/08_training_audit.md` |
| §7 Discussion | agent-39 | 800–1,100 | 15, 16, 24, 32, 33 |
| §8 Limitations | agent-29 → agent-39 | 500–700 | 29, 30, 31 |
| §9 Conclusion | agent-40 | 250–400 | 19, 39 |
| Acknowledgements | agent-35 draft → agent-40 polish | 80–150 | mission brief (Teleox.ai / Leapable.ai disclosure footer) |
| References | agent-44, agent-45 | 30–60 entries | agent-08 master citation database |

Case-study sub-ordering inside §6 is locked: **Case 3 first (T1 anchor), then Case 2,
then Case 1**, so the adversarial reviewer encounters the measured evidence before the
pending-metric subsections.

---

## 2. Evidence sources — what every downstream agent pulls from

The master evidence pools are already researched and URL-verified:

1. `/home/cabdru/contextgraph/docs/researchpaper/_evidence_01_data_wall.md`
   — scaling laws, Chinchilla, data-exhaustion (Villalobos et al. 2024), synthetic-data
   and model collapse (Shumailov et al. 2024 Nature), compression taxonomy adjacents
   (GPTQ, SparseGPT, TurboQuant, KV-cache).
2. `/home/cabdru/contextgraph/docs/researchpaper/_evidence_02_voice_cloning.md`
   — VALL-E 2 (Chen et al. 2024), WavLM / SpeechBrain speaker-verification SOTA, TTS
   benchmark protocols, DNSMOS, UTMOS.
3. `/home/cabdru/contextgraph/docs/researchpaper/_evidence_03_video_embedders.md`
   — CLIP, SigLIP, EchoMimicV3 lineage, flow-matching video, lip-sync metrics.
4. `/home/cabdru/contextgraph/docs/researchpaper/_evidence_04_enterprise_market.md`
   — licensing deals, industry signals around the data wall.

**Prior manuscript as a source, not a template.** `/home/cabdru/contextgraph/docs/researchpaper/00..15*.md`
is quote-pool + reference-pool only. FM-07 forbids re-explaining it. Writers draft fresh
against word budgets.

**Case-3 primary data.** `/home/cabdru/contextgraph/tmp/SITREP_2026-04-16_135543.md`
and `/home/cabdru/contextgraph/tasks/newdata/alldata.md` contain the measured numbers
(WavLM SECS 0.961 mean / 0.975 max on 10 unseen sentences; DNSMOS P.808 parity 3.93;
UTMOS parity 3.012 vs 2.997).

**Cases 1 & 2 artefacts.** Shakespeare: `scripts/shakespeare_full_pipeline.py` and the
Shakespeare echo-bug FSV log (15/15 passes). Santa: Santa-project
`docs/inspect/08_training_audit.md` (flow-matching sign-bug forensic file).

**Systems constructive evidence.** `CLAUDE.md` "Key Architecture Facts" gives the
authoritative embedder list for Context Graph (E1–E13). `docs2/contextgraphspecs/` (18
files) and `leapablememory/docs2/systemdescription/` (17 files) contain system detail;
writers cite file paths + counts only, not content, to keep §4 short.

**Vocabulary authority.** Canonical glossary (agent-02) and definition blocks (agent-04).
Writers pull construct language verbatim. FM-09 forbids paraphrase.

---

## 3. Agent-by-agent tasking (narrative form)

The compact table lives in `agent-tasking-table.md`. Below are one-paragraph expansions
of each agent's remit, organised by pipeline phase. Inputs, outputs, and gates are
specified in the tasking table — the text below adds the rationale.

### Phase 2 — Literature & evidence (agents 06–13)

- **agent-06 literature-mapper.** Builds the field topology in six clusters so §2 has a
  spine to hang on. The clusters correspond to the six sub-lines of §2: (i) scaling laws
  and the data wall; (ii) synthetic data and model collapse; (iii) compression taxonomy;
  (iv) alignment (RLHF / DPO / CAI); (v) multi-modal embedders (CLIP / SigLIP / WavLM /
  SpeechBrain); (vi) voice cloning (VALL-E 2). Each cluster is annotated with the
  question-IDs it serves so later writers pull the right cluster. Quality gate: every
  cluster has ≥ 3 Tier-1-or-2 candidate citations. Defuses FM-10 by forbidding generic
  survey-style citation blocks.

- **agent-07 source-tier-classifier.** Classifies every candidate citation. Tier-1 is a
  peer-reviewed venue with working DOI; Tier-2 is a preprint with an artefact; Tier-3 is
  a blog or technical report; Tier-4 is an authoritative web reference. Target ≥ 80%
  Tier-1 or Tier-2 (QG1 pass criterion). Works in tandem with agent-13 quality-assessor.

- **agent-08 citation-extractor.** Emits the master reference database (APA 7th) with
  full metadata: authors, year, venue, DOI/URL, pages. Deduplicates. Every downstream
  agent references by citation key, not by free-text URL.

- **agent-09 methodology-scanner.** Inventories the methodologies in related work:
  contrastive alignment, preference optimisation, zero-shot TTS in-context learning,
  flow-matching video, diffusion-guided generation. Surfaces the one thing none of them
  do — a per-output runtime geometric guard against a frozen centroid — so that
  agent-10 can map the gap onto the TCT delta (G3).

- **agent-10 theoretical-framework-analyst.** Maps the theoretical frameworks (IT, geometric
  ML, prototypical networks, alignment taxonomy) onto DDA / TCT. Identifies the nearest
  precedent for each Tier-A construct. Defuses FM-02: DDA is *adjacent* to synthetic-
  data-and-model-collapse literature but structurally different; the "no generator
  feedback loop" argument must be prepared here.

- **agent-11 systematic-reviewer.** PRISMA-style review on the Q01–Q03 cluster. This is
  the longest evidence-phase artefact; its result feeds §2 directly.

- **agent-12 context-tier-manager.** Infrastructure. Buckets sources into hot / warm /
  cold tiers per section so writers know what to load.

- **agent-13 quality-assessor.** Gates QG1. Spot-checks Tier-1 rigor; any failure sends
  us back to agent-06.

### Phase 3 — Analysis & synthesis (agents 14–19)

- **agent-14 bias-detector.** Flags venue bias, geographic bias, n=1 evaluation bias,
  encoder-as-training-signal bias. Case 3 is n=1 subject and author-is-subject; this
  agent surfaces it explicitly so §6.3 and §8 can address it head-on (FM-05).

- **agent-15 evidence-synthesizer.** Meta-synthesis across Phase-2 clusters: what the
  literature agrees on, disputes, and is silent on. Silence is grist for agent-18.

- **agent-16 pattern-analyst.** Cross-study patterns across Cases 1/2/3: what recurs
  (reference-set → centroid → runtime guard); what varies (modalities, loss weights,
  reference-set size).

- **agent-17 thematic-synthesizer.** Produces the five themes §2 and §7 rest on:
  (i) data-wall reframing; (ii) DDA is not synthetic data; (iii) meaning compression
  as a distinct fourth category; (iv) TCT as geometric-not-scalar alignment;
  (v) sample-complexity collapse.

- **agent-18 gap-hunter.** Enumerates ≥ 15 gaps. Minimum: formal embedder-independence
  measurement; cross-encoder SECS generalisation; non-author subjects; multi-language;
  hierarchical constellations; safety-profile modality; scope beyond the three
  protocols tested. Each gap attributes to Q16 or Q20.

- **agent-19 opportunity-identifier.** Sorts gaps into §9 future-work, §8 limitations, and
  out-of-scope.

### Phase 4 — Theory & model building (agents 20–24)

- **agent-20 interdisciplinary-pollinator.** Pulls *isomorphic* patterns from adjacent
  fields (information theory's coding theorems; psycholinguistics' prototype-exemplar
  debate; astronomy's constellation as a *metaphor only*). Stops short of over-
  extension — no claim that our constellation is "like" astronomical star catalogs in any
  technical sense.

- **agent-21 theory-builder.** Consolidates the theoretical spine. Reproduces each piece
  of math from agent-04's locked definitions; does NOT re-derive. Embeds the V11
  hedge verbatim.

- **agent-22 model-architect.** Draws the conceptual block-diagram in Markdown pseudocode
  (not rendered Mermaid unless necessary): corpus → N frozen embedders → n(N + C(N,2))
  signals → per-subject centroid per modality → constellation → training loss →
  constellation guard at inference. No Python / Rust.

- **agent-23 hypothesis-generator.** Produces ≥ 5 hypotheses each tied to a measurable
  outcome; each becomes a bullet in §6 case-study results.

- **agent-24 causal-simulator.** Runs counterfactuals: "if $\phi_m$ is replaced by
  noise, does DDA survive?"; "if $\tau_m \to 0$, does the guard become vacuous?"; the
  Shakespeare echo-bug counterfactual. Defuses FM-08 by making bugs visible to
  the theory.

### Phase 5 — Design (agents 25–28)

- **agent-25 method-designer.** Specifies the TCT three-phase protocol as pseudocode.
  Pulls verbatim from `tct-definition.md` §3.2. Specifies per-case instantiation.

- **agent-26 sampling-strategist.** Names reference-set size, held-out size, split rule,
  subject cohort for each case. Case 3: 50-clip reference set, 10 held-out sentences,
  single speaker (the author), explicit disclosure. Cases 1 & 2 projected.

- **agent-27 instrument-developer.** Pins every evaluation metric to a checkpoint /
  protocol version: WavLM SECS (checkpoint ID, pooling rule), DNSMOS P.808 (protocol
  version), UTMOS (checkpoint ID), visual cosine (CLIP/SigLIP version), lip-sync metric.
  Enforces V4 (SECS always encoder-qualified).

- **agent-28 analysis-planner.** Statistical / qualitative analysis plan. For Case 3:
  point estimate + max + parity. Because n=10 held-out and n=1 speaker, a formal
  confidence interval is either reported or explicitly flagged as non-reportable.

### Phase 6 — Quality & risk (agents 29–34)

- **agent-29 validity-guardian.** Drafts §8 Limitations. Covers every G9 item: small-n
  Case 3; pending runs Cases 1 & 2; orthogonality hedge; non-generalisation; metric
  caveats; author-bias. No handwaving allowed (FM-05, FM-08).

- **agent-30 risk-analyst.** FMEA with ≥ 6 risks, each with severity × likelihood ×
  mitigation. Covers embedder non-independence, threshold drift, cross-encoder mismatch,
  model-collapse conflation, marketing creep, length creep.

- **agent-31 ethics-reviewer.** Author-is-subject consent (explicit); impersonation-
  misuse mitigation (the constellation guard is both an *enforcer* of identity and a
  *detector* of mismatched outputs); downstream safety profiles as future work; cross-
  language / demographic scope disclosure.

- **agent-32 contradiction-analyzer.** Surfaces every contradiction between our claims
  and cited prior work. Each must be addressed in §7 or explicitly flagged as open.

- **agent-33 falsification-agent.** Produces ≥ 5 falsifiers; ≥ 1 per Tier-A construct.
  Each has a response of the form "would be falsified if X; we would retract".

- **agent-34 confidence-quantifier.** Per-claim confidence table covering 100% of
  numeric claims: {claim, value, tier T1–T4, source, page/line, confidence}. This is
  the G1 provenance artefact — if any row is missing, submission is blocked.

### Phase 7 — Writing (agents 35–41)

Writer agents do no analysis. They read locked decisions and definition blocks and
produce section files in `../sections/`. Per-section failure modes are defined in
QG7 (quality-gates.md).

### Phase 8 — Review & polish (agents 42–48)

- **agent-42 adversarial-reviewer.** Reads the *whole* manuscript as a skeptical cs.LG
  reviewer. Runs the G4 no-marketing regex. Cross-checks G2 definition consistency
  across sections. Cross-checks G3 novelty delta specificity. Produces findings with
  severity. BLOCKING on any H-gate failure. Budget ≤ 2 adversarial rounds.

- **agent-43 reproducibility-checker.** Produces the 5 × 6 matrix: {Case 1, Case 2,
  Case 3, Context Graph, ClipCannon} × {input provenance; model architecture +
  dimensions; hyperparameters; evaluation metric definition; seed/determinism;
  reproduction pointer}. Any unexplained blank blocks submission (G6).

- **agent-44 apa-citation-specialist.** Finalises APA 7th formatting, normalises in-text
  citations, emits references section.

- **agent-45 citation-validator.** Verifies every URL / DOI resolves (HTTP 200 or known-
  stable DOI). G7 allows ≤ 5% transient failures.

- **agent-46 file-length-manager.** Word count + reference count. Enforces G5. Splits
  files > 1500 lines per SMAP rule.

- **agent-47 graph-gardener.** Prunes dead notes in `./memory/`, merges near-duplicates,
  produces final memory-graph summary.

- **agent-48 adaptive-orchestrator.** Final sign-off scorecard against all nine gates.
  If any H-gate fails, selects the remediation agent by the G-table cost column and
  re-runs the affected sub-pipeline.

---

## 4. Dependency graph (narrative form; table in agent-order.md is authoritative)

We run strictly sequentially; the DAG exists for documentation and for remediation
routing.

**Load-bearing dependencies (if these slip, the pipeline stalls):**

- agent-04 → agent-05. ✅ done.
- agents 06–10 → agent-11 (PRISMA needs the topology + tiering + methodology patterns).
- agent-11 + agent-13 → QG1 (evidence base complete).
- agents 14–17 → agent-18 → agent-19 (gap-hunter needs synthesis; opportunity-identifier
  needs gaps).
- agent-04 locked constructs → agent-21 theory-builder. Agent-21 must not re-derive.
- agent-21 → agent-22 → agent-23 → agent-24 → QG3 (theory & hypotheses locked).
- agent-25 + agent-26 + agent-27 + agent-28 → QG4 (method reproducibility-ready).
- agent-29 + agent-30 + agent-31 → QG5 (validity & ethics).
- agent-32 + agent-33 + agent-34 → QG6 (adversarial resilience; G1 provenance ready).
- agents 35–40 each pass QG7 per-section. Agent-41 abstract last. Combined → QG8.
- agent-42 is the first blocking reviewer of the assembled manuscript.
- agent-43 produces the reproducibility matrix; any unexplained blank is blocking.
- agent-44 + agent-45 finalise references.
- agent-46 enforces length.
- agent-47 tidies memory.
- agent-48 final-signs.

**Parallelisable paths (documented but unused — coordinator runs sequentially):**

- Group A after agent-11: agent-14 bias-detector and agent-12 context-tier-manager
  are independent.
- Group B after agent-17: agent-18 gap-hunter, agent-19 opportunity-identifier,
  agent-20 pollinator are independent after agent-17 but sequenced 18 → 19 → 20 in
  the compact plan.
- Group C after agent-24: agent-25 method-designer and agent-26 sampling-strategist
  are independent of each other when agent-23 is locked.
- Group D inside writing phase: agent-35 and agent-36 independent; agent-37 sequential
  over §3 / §4 / §5; agent-38 independent after §5; agent-39 needs §6; agent-40 needs
  §7; agent-41 is last.

---

## 5. Timeline / effort budget

Not real-time constrained. Budget is "rough effort units" so the coordinator's final
report has something to say about distribution. One effort unit ≈ one agent-working-session.

| Phase | Agents | Effort units | Cumulative |
|---|---|---|---|
| Phase 1 scoping & planning | 01–05 | 5 | 5 |
| Phase 2 literature & evidence | 06–13 | 8 | 13 |
| Phase 3 analysis & synthesis | 14–19 | 6 | 19 |
| Phase 4 theory & model | 20–24 | 5 | 24 |
| Phase 5 design | 25–28 | 4 | 28 |
| Phase 6 quality & risk | 29–34 | 6 | 34 |
| Phase 7 writing | 35–41 | 7 + 2 rewrites contingency | 43 |
| Phase 8 review & polish | 42–48 | 7 + 2 adversarial rounds | 52 |
| **Total** | | ~52 units | |

Heavier-effort agents (double unit): 06 (literature-mapper), 08 (citation-extractor),
11 (systematic-reviewer), 34 (confidence-quantifier), 37 (methodology-writer), 38
(results-writer), 42 (adversarial-reviewer), 43 (reproducibility-checker).

---

## 6. Risk register

Every risk has: name, description, severity (CRITICAL / HIGH / MEDIUM / LOW), probability
(rough: H / M / L), mitigation, trigger, fallback.

### R1 — Marketing creep (FM-01)

- **Severity.** CRITICAL (desk-reject).
- **Probability.** M (author has commercial affiliations; easy to slip).
- **Mitigation.** G4 regex runs in agents 42, 46. Glossary V24 / V25 / V8 confines
  Teleox.ai / Leapable.ai to Acknowledgements. Abstract writer is instructed explicitly.
- **Trigger.** G4 regex hit anywhere in body.
- **Fallback.** agent-42 rewrites the offending sentence; re-run regex until clean.

### R2 — AGI overclaim (FM-03)

- **Severity.** CRITICAL.
- **Probability.** M (data-wall framing is easy to over-extend).
- **Mitigation.** Ceiling claim locked: "reframes the data wall", never "solves".
  agent-35 intro and agent-39 discussion are the two most likely offenders; QG7 per-
  section checks them.
- **Trigger.** Any occurrence of "AGI", "solve intelligence", "post-scaling", "general
  intelligence", "breakthrough", "future of" (G4 regex handles most).
- **Fallback.** agent-42 rewrites; agent-48 final sign-off blocks if still present.

### R3 — Measured-vs-projected conflation (FM-04)

- **Severity.** CRITICAL (scientific dishonesty).
- **Probability.** M (three cases with different tiers is easy to muddle).
- **Mitigation.** Every §6 subsection header tagged with tier (V20). Every numeric claim
  in agent-34's per-claim table. agent-38 is instructed to never write aggregate
  statements like "across all cases…" unless the aggregate is valid for all tiers.
- **Trigger.** agent-43 reproducibility-checker cannot find a source for any number in §6
  or §7 or abstract.
- **Fallback.** agent-38 rewrites the offending sentence with explicit tier labelling;
  if a measured number cannot be sourced, flag it as T3 projected.

### R4 — DDA-as-synthetic-data confusion (FM-02)

- **Severity.** HIGH (core-construct failure).
- **Probability.** H (the surface similarity is strong; every cs.LG reader primes to it).
- **Mitigation.** V6 mandates "not synthetic data" within two sentences of every first-
  use of DDA. agent-10 and agent-21 produce the structural argument. §3.3 opens with
  it explicitly.
- **Trigger.** adversarial-reviewer (agent-42) flags a DDA reference without the
  distinguishing clause.
- **Fallback.** Add the V6 sentence; re-run agent-42.

### R5 — Single-subject sample-size hidden (FM-05)

- **Severity.** HIGH.
- **Probability.** H (it is the biggest structural limitation of Case 3).
- **Mitigation.** §6.3 opens with the protocol statement "One speaker (the author)…";
  §8 includes it under G9; agent-31 addresses it under ethics.
- **Trigger.** §6.3 reads as if the protocol generalises.
- **Fallback.** agent-38 rewrites §6.3 opening paragraph; agent-29 strengthens §8.

### R6 — Centroid / constellation vocabulary drift (FM-09)

- **Severity.** MEDIUM.
- **Probability.** M (multiple writers, terminology density is high).
- **Mitigation.** Glossary locked; paste-ready definition blocks (agent-04). Forbidden
  terms list in glossary footer (prototype, anchor, identity-preserving, etc.).
- **Trigger.** agent-42 regex matches any forbidden term.
- **Fallback.** agent-42 find-and-replace.

### R7 — Code-in-body (FM-06)

- **Severity.** MEDIUM.
- **Probability.** M (temptation to be concrete in §5 is strong).
- **Mitigation.** Writer instructions ban Python / Rust in body; pseudocode and
  equations only. Full code in appendix (§14 / §15 of prior manuscript).
- **Trigger.** agent-42 or agent-46 finds a triple-backtick `python` or `rust` fence
  in body.
- **Fallback.** agent-37 / 38 converts to pseudocode; original code moves to appendix.

### R8 — Re-explaining prior manuscript (FM-07)

- **Severity.** MEDIUM (length overrun; blandness).
- **Probability.** M (easy path for writers).
- **Mitigation.** Word budgets per section; prior manuscript used as source, not
  template. agent-46 enforces length.
- **Trigger.** G5 length fails (> 12,500 words).
- **Fallback.** agent-46 requests cuts from the longest section; agent-36 and agent-39
  are primary targets.

### R9 — Silent failure hiding (FM-08)

- **Severity.** HIGH (forensics are load-bearing per P10).
- **Probability.** L (explicitly planned; case-study sub-ownership requires forensic
  paragraphs).
- **Mitigation.** §6.1 must include the Shakespeare echo-bug paragraph; §6.2 must
  include the flow-matching sign-bug paragraph (glossary #30). agent-42 checks.
- **Trigger.** agent-42 finds a case-study subsection without a forensic paragraph
  where one is expected.
- **Fallback.** agent-38 adds the paragraph with a pointer to the source file.

### R10 — Citation padding (FM-10)

- **Severity.** LOW.
- **Probability.** M.
- **Mitigation.** Every citation must support a specific claim. agent-36 literature-
  review-writer is the most likely offender. agent-44 / 45 audit; agent-42 spot-checks.
- **Trigger.** Multi-citation block appended to a generic sentence.
- **Fallback.** Prune citations to the one that actually supports the claim.

### R11 — Reproducibility matrix incomplete (G6 fail)

- **Severity.** CRITICAL (blocks submission).
- **Probability.** M (five rows × six columns = 30 cells, plenty of chance to miss one).
- **Mitigation.** agent-25 / 26 / 27 / 28 each contribute explicit fields. §8.5
  Reproducibility notes + Appendix. agent-43 audits pre-submission.
- **Trigger.** Any blank cell without explicit "N/A: <reason>".
- **Fallback.** agent-43 re-routes the missing cell to its owner agent for fill-in;
  re-run agent-43.

### R12 — Citation URL rot (G7 marginal)

- **Severity.** LOW (G7 tolerates ≤ 5%).
- **Probability.** L (evidence briefs are already verified).
- **Mitigation.** agent-45 URL health pass; DOI preferred over URL for peer-reviewed
  sources.
- **Trigger.** > 5% URL failures.
- **Fallback.** Replace with DOI or archived copy (web.archive.org); if neither, drop
  the citation and find a substitute.

### R13 — VALL-E 2 comparability over-claim (FM-04 adjacent)

- **Severity.** HIGH.
- **Probability.** M (we WILL compare Case 3's 0.961 WavLM SECS to VALL-E 2's reported
  human parity; comparability is limited).
- **Mitigation.** Q14 answer shape: state *what is comparable* (both report speaker-
  similarity scores) and *what is not* (different held-out sets, different encoders
  — V4 bare-SECS risk, different prompt protocols, different speaker cohorts). agent-38
  and agent-36 align on this wording.
- **Trigger.** A sentence like "Case 3 matches VALL-E 2" without the comparability
  caveat.
- **Fallback.** agent-42 rewrites the comparator paragraph.

### R14 — Adversarial-review rounds exceed budget

- **Severity.** MEDIUM (time / effort risk).
- **Probability.** L.
- **Mitigation.** ≤ 2 adversarial rounds permitted. If round 2 still fails, escalate to
  coordinator for scoped rewrite of the affected sections only.
- **Trigger.** agent-42 rejects after round 2.
- **Fallback.** Coordinator-scoped rewrite; do not loop indefinitely.

---

## 7. Explicit downstream instructions (what agent-06 should do next)

Agent-06 literature-mapper begins with these steps (reiterated in the handoff file):

1. Read `./memory/context/mission-brief.md`, `./memory/context/constraints.md`, all of
   `./memory/decisions/` and `./memory/patterns/`, and the four `_evidence_0*.md`
   files.
2. Build the six-cluster field topology per the tasking-table description.
3. Annotate each cluster with the Q-IDs it serves (from agent-03's essential questions).
4. For each cluster, identify the 5–10 most load-bearing citations (Tier-1/2 priority);
   mark the cluster as "thin" if < 3 T1/T2 candidates surface — agent-11 will need to
   expand.
5. Write `./memory/patterns/agent-06--field-topology.md`.
6. Write `./memory/handoff/agent-06-to-07--topology-ready.md`.
7. Write `./memory/reports/agent-06--final-report.md`.

Agent-06's output gates QG1 together with agents 07, 08, 13.

---

## 8. What counts as "done" for this plan

This plan is complete iff:

- [ ] Section ownership is locked for every manuscript section (✓ `section-ownership.md`).
- [ ] Every agent 06–48 has a row in the tasking table with reads / task / writes /
  FM-defused / Q-answered (✓ `agent-tasking-table.md`).
- [ ] Eight intermediate checkpoints plus nine final gates are specified, each with
  pass / fail / fallback (✓ `quality-gates.md`).
- [ ] Dependency graph is documented (✓ this file §4 + `agent-order.md`).
- [ ] Timeline / effort budget exists (✓ this file §5).
- [ ] Risk register covers every named failure mode plus infrastructure risks (✓ this
  file §6).
- [ ] Handoff file tells agent-06 where to start (✓ `handoff/agent-05-to-06--plan-ready.md`).
- [ ] Final SMAP report filed (✓ `reports/agent-05-research-planner--final-report.md`).

---

## Change log

- 2026-04-16 — Initial issue by agent-05-research-planner.
