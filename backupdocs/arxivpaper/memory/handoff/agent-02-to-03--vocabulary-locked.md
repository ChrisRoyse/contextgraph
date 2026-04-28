---
namespace: handoff
agent_id: agent-02-ambiguity-clarifier
timestamp: 2026-04-16T15:30:00Z
status: active
tags: handoff, agent-03, self-ask-decomposer, vocabulary, glossary
---

# Handoff: Agent-02 → Agent-03 (self-ask-decomposer)

## Read these files FIRST, in this order

1. `./memory/decisions/agent-02-ambiguity-clarifier--canonical-glossary.md` —
   **LOCKED** glossary of 30 canonical terms. All 15–20 critical questions you generate must
   use these terms; do NOT invent synonyms.
2. `./memory/patterns/agent-02-ambiguity-clarifier--vocabulary-rules.md` — the 25 rules
   (V1–V25) downstream writers apply. Read these so your questions don't accidentally trip
   a prohibition.
3. `./memory/discoveries/agent-02-ambiguity-clarifier--ambiguities-resolved.md` — 20 resolved
   ambiguities with rationale. If your question touches a resolved term, reference the R-row
   (e.g., "see R04").
4. `./memory/patterns/agent-01-step-back-analyzer--guiding-principles.md` — P01–P10
   principles (still binding).
5. `./memory/decisions/agent-01-step-back-analyzer--failure-modes-to-avoid.md` — FM-01–FM-10.
   Your decomposition must not enable any of them.

## What agent-02 did

Locked a 30-entry canonical glossary. Every load-bearing term (DDA, meaning compression, TCT,
identity-locked generation, centroid, constellation, reference set, embedder, modality,
alignment, evidence tier T1–T4, same-session band, etc.) has ONE canonical spelling, ONE
definition, ONE usage rule. Forbidden substitutes ("prototype", "anchor", "semantic
compression", bare "SECS", etc.) are regex-detectable and enforced by agent-42.

## What agent-03 (self-ask-decomposer) must do

Produce 15–20 critical "what must this paper answer?" questions that jointly cover the four
core claims (DDA, meaning compression, TCT, sample-complexity collapse) and the three case
studies. Each question should be answerable by a specific later agent or section.

**Vocabulary constraints on your output:**

- Use the canonical glossary terms verbatim. Do NOT write "identity prototype" or "reference
  vector" — use "constellation centroid" / "reference embedding".
- When a question touches an evidence-tier distinction, tag the tier (T1/T2/T3/T4) per
  glossary #21.
- Questions about SECS must name the encoder (WavLM / SpeechBrain / Qwen3).
- Questions about "alignment" must specify "geometric alignment to constellation" or "scalar-
  reward alignment"; never bare "alignment".
- Questions about DDA must NOT phrase it as "synthetic data" or "augmentation" — use the DDA
  framing (decomposition through frozen embedders, no new inputs).

**Structural recommendations:**

- Questions should be **falsifiable or answerable by specific evidence**. "Is TCT better than
  contrastive learning?" is bad (unfalsifiable in-scope). "What specific property of TCT's
  runtime guard is absent from contrastive learning, and in which §2 paragraph is the delta
  stated?" is good.
- Target 3–5 questions per core claim + 2–3 per case study + 1–2 meta questions
  (reproducibility, limitations). That yields 15–22 questions, within the ask-decomposer's
  normal range.
- Each question names the **primary evidence source** that should answer it (evidence brief,
  SITREP, forensic report, or prior-manuscript section).

## Useful pointers agent-03 should know

- **Case 3 is T1-measured** (SECS, DNSMOS, UTMOS in SITREP `tmp/SITREP_2026-04-16_135543.md`
  and `tasks/newdata/alldata.md`).
- **Cases 1 and 2 are T2+T3** (systems run, metrics pending).
- **DDA count is T4** (constructive, $n(N + \binom{N}{2})$).
- **Meaning-compression taxonomy is T4** (the four-way orthogonality table in §3.5).
- **Same-session band is an empirical claim of ours**, not a universal benchmark — glossary
  #17 and rule V12.

## What NOT to do

- Do NOT introduce a new term. If you believe a concept is missing from the glossary, file a
  blocker at `./memory/blockers/agent-03-ambiguity--{term}.md` with HIGH severity; do not
  silently coin.
- Do NOT override any glossary entry; file a decision override if necessary.
- Do NOT phrase questions as sales hooks ("Is this the first time anyone has…?"). Reviewers
  interpret that as marketing-creep. Use neutral framing.

## Success criterion for agent-03's handoff

Agent-04 (construct-definer) can read your question set and immediately know which questions
bind its definitions (DDA, meaning compression, TCT, identity-locked) vs. which bind agents
36 (literature review), 37 (methodology), 38 (results). No question should ambiguate across
downstream assignees.

## Escalation

If agent-03 encounters a question whose answer depends on a term not in my glossary and which
is load-bearing, file a blocker at `./memory/blockers/agent-03-question--{term}.md`. Do not
silently adopt a provisional term.
