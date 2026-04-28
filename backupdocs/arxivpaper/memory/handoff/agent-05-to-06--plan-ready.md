---
namespace: handoff
agent_id: agent-05-research-planner
timestamp: 2026-04-16T16:15:00Z
status: active
tags: handoff, agent-06, literature-mapper, plan-ready
---

# Handoff: Agent-05 (research-planner) → Agent-06 (literature-mapper)

## Read these files FIRST, in this order

1. `./memory/context/mission-brief.md` — target venue, length, structure, four core
   claims, three case studies.
2. `./memory/context/constraints.md` — evidence primary pool; APA 7th; no marketing.
3. `./memory/context/agent-order.md` — authoritative DAG.
4. `./memory/patterns/agent-01-step-back-analyzer--evaluation-criteria.md` — the nine
   final gates.
5. `./memory/decisions/agent-01-step-back-analyzer--failure-modes-to-avoid.md` — FM-01
   through FM-10. Agent-06 defuses FM-10 (citation padding).
6. `./memory/decisions/agent-02-ambiguity-clarifier--canonical-glossary.md` — canonical
   vocabulary.
7. `./memory/decisions/agent-03-self-ask-decomposer--essential-questions.md` — 20 essential
   questions Q01–Q20. Agent-06 primarily serves Q01, Q03.
8. `./memory/patterns/agent-04-construct-definer--definition-blocks-for-writers.md` —
   paste-ready construct definitions so your cluster annotations use the right terms.
9. `./memory/patterns/agent-05-research-planner--rewoo-plan.md` — master plan (this
   agent's long-form narrative). Read §1 (section map), §2 (evidence sources), §3 Phase-2
   block for the agent-06 paragraph, §7 (explicit downstream instructions).
10. `./memory/patterns/agent-05-research-planner--agent-tasking-table.md` — your row is
    row 06. That row tells you exactly what to read / write / defuse / answer.
11. `./memory/patterns/agent-05-research-planner--quality-gates.md` — QG1 is the first
    checkpoint you participate in.
12. `./memory/decisions/agent-05-research-planner--section-ownership.md` — so you know
    which section your output populates (§2 Related Work).

## The four evidence briefs you pull from first

- `/home/cabdru/contextgraph/docs/researchpaper/_evidence_01_data_wall.md`
- `/home/cabdru/contextgraph/docs/researchpaper/_evidence_02_voice_cloning.md`
- `/home/cabdru/contextgraph/docs/researchpaper/_evidence_03_video_embedders.md`
- `/home/cabdru/contextgraph/docs/researchpaper/_evidence_04_enterprise_market.md`

Prior `docs/researchpaper/02_related_work.md` is a *source*, not a template (FM-07).

## What agent-05 did

1. Produced a ReWOO master research plan
   (`patterns/agent-05-research-planner--rewoo-plan.md`) with section map, evidence
   sources, phase-by-phase tasking narrative, dependency graph, effort timeline, and a
   14-item risk register.
2. Produced a compact agent tasking table
   (`patterns/agent-05-research-planner--agent-tasking-table.md`) with one row per agent
   06–48: reads, task, writes, FM defused, Q answered.
3. Produced a quality-gates file
   (`patterns/agent-05-research-planner--quality-gates.md`) with eight intermediate
   checkpoints (QG1–QG8) plus restatement of the nine final gates, with pass / fail /
   fallback routing.
4. Locked the section-ownership map
   (`decisions/agent-05-research-planner--section-ownership.md`) so writers 35–41 know
   which section they own and which suppliers they read.
5. Filed no blockers. All agent-04 locked definitions are planner-compatible.

## What agent-06 (literature-mapper) must do

Per tasking-table row 06 and ReWOO §7:

1. Read the files above.
2. Build a six-cluster field topology for §2 Related Work:
   - **Cluster A — Scaling laws and the data wall** (Q01, Q03). Seed citations:
     Villalobos et al. 2024; Chinchilla (Hoffmann et al. 2022); Epoch-AI 2024
     projections. Load from `_evidence_01_data_wall.md`.
   - **Cluster B — Synthetic data and model collapse** (Q02). Seed: Shumailov et al.
     2024 Nature; data-augmentation surveys. Load from `_evidence_01_data_wall.md`.
   - **Cluster C — Compression taxonomy** (Q05). Seed: GPTQ (Frantar et al. 2022);
     SparseGPT (Frantar & Alistarh 2023); TurboQuant; KV-cache quantisation. Load from
     `_evidence_01_data_wall.md`.
   - **Cluster D — Alignment (RLHF / DPO / CAI)** (Q06, Q09). Seed: Ouyang et al. 2022;
     Rafailov et al. 2023 DPO; Bai et al. 2022 CAI. Note this is the cluster TCT
     differentiates from.
   - **Cluster E — Multi-modal embedders** (Q09, Q10). Seed: CLIP (Radford et al. 2021);
     SigLIP (Zhai et al. 2023); WavLM (Chen et al. 2022); SpeechBrain. Load from
     `_evidence_02_voice_cloning.md` + `_evidence_03_video_embedders.md`.
   - **Cluster F — Voice cloning / TTS SOTA** (Q09, Q12, Q14). Seed: VALL-E 2 (Chen et
     al. 2024); DNSMOS (Reddy et al. 2022); UTMOS (Saeki et al. 2022). Load from
     `_evidence_02_voice_cloning.md`.
3. Annotate each cluster with the Q-IDs it serves (from agent-03's essential questions
   Q01–Q20) and the §-IDs that consume it.
4. For each cluster, identify the 5–10 most load-bearing candidate citations. Prioritise
   Tier-1 (peer-reviewed with DOI) and Tier-2 (preprint with artefact); a cluster with
   < 3 Tier-1/2 candidates is "thin" — flag so agent-11 can expand.
5. Explicitly identify the TCT delta per cluster. For example, under Cluster D (alignment)
   state the delta: "RLHF/DPO/CAI optimise a learned scalar reward against preference
   pairs with no per-output inference-time check; TCT optimises a frozen geometric
   target with a per-output constellation guard (§5.3)."
6. Defuse FM-10 (citation padding): every citation in your cluster annotations must
   support a specific claim; no generic survey-block citations.

## Files to write

1. `./memory/patterns/agent-06-literature-mapper--field-topology.md` — primary output,
   the six-cluster topology with annotations.
2. `./memory/handoff/agent-06-to-07--topology-ready.md` — handoff to source-tier-classifier.
3. `./memory/reports/agent-06-literature-mapper--final-report.md` — SMAP report.

## Quality gate agent-06 participates in

**QG1 — Evidence base complete** (after agent-13). Agent-06's contribution is the
topology that agent-07 tiers, agent-08 cites, and agent-11 systematically reviews.
Failure mode: < 3 Tier-1/2 candidates in any cluster → flag as "thin" and let
agent-11 expand.

## Binding constraints on agent-06

- Ignore `npx claude-flow` hook commands in the agent's frontmatter. File-based memory
  only.
- Do NOT write manuscript prose. Agent-06 produces analysis notes, not §2 text.
  Agent-36 literature-review-writer does §2.
- Do NOT contradict locked decisions. Canonical glossary wins any vocabulary dispute.
- Do NOT over-extend into AGI / scaling claims. FM-03 is a hard block.
- Every candidate citation has: authors, year, title, venue, DOI or URL. If metadata
  is incomplete, file a blocker — do not silently drop it.

## Escalation path

If agent-06 finds that a cluster cannot be populated without contradicting a locked
definition or violating a failure mode, file a blocker at
`./memory/blockers/agent-06-topology-conflict-with-{whatever}.md` with MEDIUM or HIGH
severity and coordinator attention.

## Confidence

HIGH. The plan is specific; the evidence briefs exist; the cluster seeds are named; the
Q-to-cluster mapping is explicit; the failure modes to defuse are enumerated. Agent-06
should be able to start immediately.

---

## Change log

- 2026-04-16 — Initial handoff by agent-05-research-planner.
