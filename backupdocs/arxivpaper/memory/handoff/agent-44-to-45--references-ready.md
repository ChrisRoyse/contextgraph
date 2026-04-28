---
namespace: handoff
agent_id: agent-44-apa-citation-specialist
timestamp: 2026-04-17T13:20:00Z
status: active
supersedes: none
tags: handoff, agent-45, citation-validator, references-ready, apa-7th
---

# Handoff: Agent-44 (apa-citation-specialist) → Agent-45 (citation-validator)

## Read these files FIRST, in this order

1. `sections/11_references.md` — the compiled APA 7th References
   section (63 entries, alphabetical by first-author surname).
   This is your primary validation target.
2. `memory/patterns/agent-44-apa-citation-specialist--reference-compilation.md`
   — the section-by-section harvest map showing which sections
   cite which keys, and the APA formatting rules applied.
3. `memory/discoveries/agent-44-apa-citation-specialist--missing-or-flagged-citations.md`
   — 8 in-text citations that appear in body text but have NO
   entry in agent-08's master pool or the evidence briefs. You
   must decide for each: remove from body, add to pool via
   blocker, or leave with a pending marker.
4. `memory/decisions/agent-08-citation-extractor--master-reference-pool.md`
   — the locked 88-citation master pool; every References
   section entry is drawn from this pool.

## What agent-44 did

### Harvest

Read all ten draft sections (`sections/00_frontmatter.md` through
`sections/10_conclusion.md`) and extracted every unique in-text
citation.

- **Unique citations in body**: 71 total (63 that resolve to the
  master pool + 8 that do NOT resolve and are flagged).
- **APA entries in References**: 63.
- **Master-pool entries NOT cited in body**: 25 (not included in
  References; retained in the locked pool for camera-ready
  revisions).

### Compilation

Wrote `sections/11_references.md` with 63 APA 7th entries:

- Alphabetical by first author's surname.
- Journal/book titles italicised, volume numbers italicised,
  issue numbers in parentheses (not italicised).
- DOIs preferred over URLs; bare `https://doi.org/...` format
  (no "DOI:" label per APA 7th).
- arXiv preprints written as `arXiv. https://arxiv.org/abs/...`.
- Up to 20 authors listed; 21+ trigger "… & [final-author]"
  ellipsis (applied to longpre2024consent, ouyang2022instructgpt,
  hoffmann2022chinchilla, bai2022constitutional).
- No "Retrieved from" language (APA 7th dropped this).
- Group / corporate authors formatted without "the" prefix
  (Anthropic, Reuters, Variety Staff, Wan Team).

### Flagging

Documented 8 unresolved in-text citations in
`memory/discoveries/agent-44-apa-citation-specialist--missing-or-flagged-citations.md`
with severity ratings and recommended actions. See that file for
the full list and rationale.

## What agent-45 should do next

### Primary validation targets

1. **Verify every URL in `sections/11_references.md`** by
   attempting resolution. DOIs should resolve to the expected
   publisher page; arXiv URLs should resolve to the expected
   paper. URL verification was deferred from agent-44 per the
   task brief's "URLs already verified. Do NOT re-verify unless
   you suspect drift" rule; agent-45 is the "suspect drift"
   backstop.

2. **Verify VALL-E 2 number (0.881 WavLM-large-sv) is present in
   the cited source** (chen2024valle2, arXiv:2406.05370), per
   agent-43's §6.3.3 STOP-4 concern. Agent-33 CA-3 flagged this
   as possibly a secondary-source citation — agent-45 must
   confirm the number appears in Chen et al. (2024) directly,
   not via intermediate citation.

3. **Resolve the 8 flagged citations**. For each of FLAG-1 through
   FLAG-8 in the discoveries file, execute the recommended
   action:
   - FLAG-1 (Shannon, 1948): recommended DROP from body text.
   - FLAG-2, FLAG-3 (Frantar GPTQ, SparseGPT): recommend ADD to
     pool via blocker (or drop as exemplar).
   - FLAG-4 (Chung & Zisserman, 2017 SyncNet): recommend DROP
     parenthetical (cited only as future instrument).
   - FLAG-5, FLAG-6, FLAG-7 (Kinnunen, Schroff, Bromley): ADD
     all three to pool OR rewrite §7.3 sentence to remove
     named citations.
   - FLAG-8 (Epoch AI, 2024): recommend DROP as redundant with
     Villalobos et al. (2024) unless agent-45 can disambiguate
     to a separate Epoch AI document.

4. **Fix the Valmeekam author-list mismatch** in §2.3 line 17.
   The body text reads "Valmeekam, Narayanan, Bharadia and Celik
   (2023)" but the canonical authors (arXiv:2306.04050) are
   "Valmeekam, Narayanan, Tuli, and Yu". The master-pool entry
   (valmeekam2023llmzip) has the correct author list. Agent-47
   (final-assembler) should update §2.3 body text.

5. **Confirm citation completeness**. After flag resolution,
   re-run the harvest: every in-text citation must have an APA
   entry in `sections/11_references.md`, and every entry must
   be cited at least once in body. No orphans on either side.

### Specific agent-43 handoff items

Agent-43 flagged five specific citation-related concerns in
`memory/handoff/agent-43-to-44--reproducibility-ready.md`. Each
is carried forward to agent-45 as appropriate.

1. **Embedder/scorer citations for §15 appendix**. Agent-43's
   §15.3 draft references WavLM (chen2022wavlm), ECAPA-TDNN
   (desplanques2020ecapa), DNSMOS (reddy2022dnsmos), UTMOS
   (saeki2022utmos), SigLIP (zhai2023siglip), ColBERTv2
   (santhanam2022colbertv2), SPLADE (formal2021splade), KEPLER
   (wang2021kepler), e5-large-v2 (no direct master-pool entry —
   see note below), nomic-embed (nussbaum2024nomic), EchoMimicV3
   (meng2025echomimicv3), DPO (rafailov2023dpo), Flow-matching
   (lipman2023flowmatching), Qwen3-TTS (no master-pool entry —
   qwen3tts2024 is X01 INELIGIBLE per agent-08). All the
   master-pool entries are in the References section (agent-44
   verified).

   - **e5-large-v2**: the model is referenced in body text
     (§4.1 line 11) as "e5-large-v2 for general semantics (E1)".
     There is no master-pool entry for e5 or for the Wang et al.
     e5 paper. Agent-08 flagged this in the initial pool. If
     the §15 reproducibility appendix will include the e5 model
     card URL, agent-45 should file a blocker to add it.

   - **Qwen3-TTS**: Agent-08 excluded qwen2024qwen3tts as X01
     INELIGIBLE. The body text (§5.6, §6.3.1, §6.3.2) and the
     pipeline version tag `qwen3-tts-foricl-v1` refer to Qwen3-TTS
     throughout. Agent-43's §15 appendix will need a Qwen3-TTS
     disclosure (T3 GitHub model-card URL per agent-08 guidance);
     agent-45 should file a blocker and add via the proposed
     key `qwen2024qwen3tts` at camera-ready.

2. **STOP-3 paste-ready edit (agent-42)**. The STOP-3 replacement
   paragraph for §6.3.3 does NOT appear to introduce new
   citations beyond what is already in the master pool; agent-45
   should confirm at the point the edit is applied (by agent-47).

3. **STOP-4 VALL-E 2 0.881 number**. Already carried as primary
   validation target above.

4. **`docs/inspect/08_training_audit.md`** (cited in §6.2.2) is
   an internal repository path, not a published citation. Agent-44
   leaves it as a prose reference without a References entry per
   agent-43's "arXiv v1: repository path" recommendation. Agent-45
   should confirm no References entry is expected; if agent-47
   adds an inline footnote with a GitHub commit URL at assembly
   time, the footnote can omit the References entry.

5. **`docs2/LORA_STYLE_TRANSFER_COMPLETE_REPORT.md`** (cited in
   §6.1.3). Same treatment as item 4.

### Citation completeness summary

- Mission brief §"arXiv compliance targets": 30–60 references.
- Current References count: **63**.
- **3 over target**. Agent-45 and agent-47 may prune if desired,
  but each of the 63 entries is actively cited in body — pruning
  requires removing a body citation or combining consecutive
  related citations.

Options for trimming to ≤ 60:

- (a) Drop `sutskever2024neurips` — oral remark, no written
  artefact. Replace inline with "late-2024 industry reporting
  (Franzen, 2024; Metz et al., 2024)" in §2.1 line 7.
- (b) Drop `anthropic2024mcp` from §4.1 line 9 — MCP is described
  generically elsewhere and the Anthropic citation is a corporate
  blog post; alternative is to retain Model Context Protocol
  (mcp2024spec) which IS in the master pool but NOT cited in the
  current body (would be a swap: drop anthropic2024mcp, add
  mcp2024spec; net zero). However mcp2024spec is already noted
  as unused in body.
- (c) Drop `christop2025cloneval` from §2.5 line 29 — cited
  once, as a supplementary benchmark-evaluator reference.

Agent-44 recommends keeping all 63: every one is load-bearing in
its section, the 30–60 range is a mission-brief target not a hard
limit, and the manuscript spans substantial literature that
genuinely requires this count.

## Files agent-44 produced

1. `sections/11_references.md` — the APA 7th References section
   (63 entries, alphabetical).
2. `memory/patterns/agent-44-apa-citation-specialist--reference-compilation.md`
   — harvest map (which sections cite which keys).
3. `memory/discoveries/agent-44-apa-citation-specialist--missing-or-flagged-citations.md`
   — 8 flagged unresolved citations.
4. `memory/handoff/agent-44-to-45--references-ready.md` (this file).
5. `memory/reports/agent-44-apa-citation-specialist--final-report.md`
   — final report.

## Confidence

- **HIGH** that every master-pool citation in the body text has a
  matching APA entry in `sections/11_references.md`.
- **HIGH** that the APA 7th formatting is correct for the 63
  entries (up-to-20 authors, italics, DOI policy, alphabetical).
- **HIGH** that the 8 FLAG items are the complete set of
  not-in-pool citations (systematic read of all ten sections).
- **MEDIUM** that URL verification is needed for every entry —
  agent-44 deferred URL probing per the task brief and relies on
  agent-08's prior verification via the evidence briefs. Agent-45
  is the backstop.
- **MEDIUM** that the VALL-E 2 0.881 number is actually present in
  chen2024valle2 — agent-44 did not re-verify the source text,
  as the task brief defers to agent-45 for this specific check.

## Dependencies I'm leaving for downstream

- **Agent-45 (citation-validator)**: URL resolution audit; VALL-E 2
  number verification; resolution of the 8 FLAGs; Valmeekam
  author-list fix in body text (§2.3).
- **Agent-46 (coherence-checker)**: ensure the References section
  renders correctly when concatenated into `PAPER.md`; ensure no
  numbering collisions between Table 1 / Table 2 references and
  the bibliography.
- **Agent-47 (final-assembler)**: import the 11_references.md
  file into the final assembly; apply the Valmeekam author fix in
  §2.3; apply agent-45's FLAG resolutions; if agent-47 ships a
  §15 reproducibility appendix, include any additional references
  added via the FLAG-2/3/5/6/7 blocker path.

## Changelog

- 2026-04-17 — Handoff written by agent-44-apa-citation-specialist.
