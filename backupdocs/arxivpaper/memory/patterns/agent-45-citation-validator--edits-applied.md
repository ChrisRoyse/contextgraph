---
namespace: patterns
agent_id: agent-45-citation-validator
timestamp: 2026-04-17T13:45:00Z
status: active
supersedes: none
tags: edits, citations, body-text-fixes, audit-trail
depends_on:
  - memory/discoveries/agent-44-apa-citation-specialist--missing-or-flagged-citations.md
downstream_consumers:
  - agent-46-file-length-manager
  - agent-47-final-assembler
---

# Edits Applied — Verbatim Change Log

All edits made by agent-45 during citation validation. Each
row lists the file, the exact old text, and the exact new
text. Zero edits made to `sections/11_references.md` — the
reference list was internally correct; all fixes touched
body-text inconsistencies only.

---

## Edit 1 — §1 introduction (Efrati & Holmes → Woo & Efrati)

- **File**: `sections/01_introduction.md`
- **Rationale**: Master-pool entry is
  "Woo, E., & Efrati, A. (2024)"; body text used the
  evidence-brief-only shorthand "(Efrati & Holmes, 2024)".
- **Old text (line 3 partial)**:
  > Industry reporting through late 2024 (Efrati & Holmes,
  > 2024; Metz et al., 2024; Hu & Tong, 2024) converges on
  > the same observation from inside the frontier labs:
  > returns to raw pre-training scale are compressing.
- **New text (line 3 partial)**:
  > Industry reporting through late 2024 (Woo & Efrati,
  > 2024; Metz et al., 2024; Hu & Tong, 2024) converges on
  > the same observation from inside the frontier labs:
  > returns to raw pre-training scale are compressing.

---

## Edit 2 — §2.1 related work (Efrati & Holmes → Woo & Efrati)

- **File**: `sections/02_related_work.md`
- **Rationale**: Same as Edit 1 (second occurrence of
  same citation).
- **Old text (line 7 partial)**:
  > A cluster of late-2024 industry reports (Efrati & Holmes,
  > 2024; Metz et al., 2024; Hu & Tong, 2024; Franzen, 2024;
  > Sutskever, 2024) describes diminishing returns from
  > raw-token pre-training;
- **New text (line 7 partial)**:
  > A cluster of late-2024 industry reports (Woo & Efrati,
  > 2024; Metz et al., 2024; Hu & Tong, 2024; Franzen, 2024;
  > Sutskever, 2024) describes diminishing returns from
  > raw-token pre-training;

---

## Edit 3 — §2.3 related work (Valmeekam author-list fix)

- **File**: `sections/02_related_work.md`
- **Rationale**: Canonical arXiv:2306.04050 authors are
  Valmeekam, Narayanan, Tuli, and Yu; body text used
  non-canonical "Bharadia and Celik".
- **Old text (line 17 partial)**:
  > Valmeekam, Narayanan, Bharadia and Celik (2023)
  > implement arithmetic coding directly on language-model
  > logits ("LLMZip"),
- **New text (line 17 partial)**:
  > Valmeekam, Narayanan, Tuli, and Yu (2023) implement
  > arithmetic coding directly on language-model logits
  > ("LLMZip"),

---

## Edit 4 — §2.3 related work (FLAG-2 GPTQ / FLAG-3 SparseGPT drop)

- **File**: `sections/02_related_work.md`
- **Rationale**: GPTQ and SparseGPT not in master pool;
  exemplar-level cites; taxonomic argument stands without
  them (agent-44 LOW severity recommendation).
- **Old text (line 17 partial)**:
  > Adjacent to the compression-as-intelligence line, the
  > standard taxonomy distinguishes three categories: **bit
  > compression** (arithmetic coding, gzip, FLAC), **weight
  > compression** (GPTQ by Frantar, Ashkboos, Hoefler, and
  > Alistarh, 2022; SparseGPT by Frantar and Alistarh, 2023;
  > distillation), and **activation compression**
  > (TurboQuant; KV-cache quantisation).
- **New text (line 17 partial)**:
  > Adjacent to the compression-as-intelligence line, the
  > standard taxonomy distinguishes three categories: **bit
  > compression** (arithmetic coding, gzip, FLAC), **weight
  > compression** (post-training quantisation such as GPTQ,
  > one-shot pruning such as SparseGPT, and knowledge
  > distillation), and **activation compression**
  > (TurboQuant; KV-cache quantisation).

---

## Edit 5 — §3.2 framework (FLAG-1 Shannon drop + FLAG-2/3 drop)

- **File**: `sections/03_framework_dda_mc.md`
- **Rationale**: Shannon (1948) not in master pool;
  Delétang et al. (2024) is the adjacent anchor
  (FLAG-1 LOW). GPTQ, SparseGPT dropped as FLAG-2/3.
- **Old text (line 37)**:
  > **Positioning as a fourth category.** Conventional
  > compression asks "for fixed semantic content, how few
  > bits / weights / activations can I use?" Meaning
  > compression flips the question: "for fixed raw-data
  > volume, how many structured supervisory signals can I
  > extract?" The pre-existing three categories are
  > well-defined in the ML systems literature (Section 2.3).
  > Bit compression — arithmetic coding, gzip, FLAC, PNG —
  > measures bits of compressed output per bit of raw input
  > (Shannon, 1948; Delétang et al., 2024). Weight
  > compression — GPTQ (Frantar et al., 2022), SparseGPT
  > (Frantar & Alistarh, 2023), distillation — measures task
  > accuracy per trainable parameter. Activation compression
  > — KV-cache quantisation, TurboQuant — measures peak
  > runtime memory per unit of output quality.
- **New text (line 37)**:
  > **Positioning as a fourth category.** Conventional
  > compression asks "for fixed semantic content, how few
  > bits / weights / activations can I use?" Meaning
  > compression flips the question: "for fixed raw-data
  > volume, how many structured supervisory signals can I
  > extract?" The pre-existing three categories are
  > well-defined in the ML systems literature (Section 2.3).
  > Bit compression — arithmetic coding, gzip, FLAC, PNG —
  > measures bits of compressed output per bit of raw input
  > (Delétang et al., 2024). Weight compression —
  > post-training quantisation (GPTQ), one-shot pruning
  > (SparseGPT), and distillation — measures task accuracy
  > per trainable parameter. Activation compression —
  > KV-cache quantisation, TurboQuant — measures peak
  > runtime memory per unit of output quality.

---

## Edit 6 — §6.2.4 case studies (FLAG-4 SyncNet drop)

- **File**: `sections/07_case_studies.md`
- **Rationale**: Chung & Zisserman (2017) not in master
  pool; cited as conditional future instrument (agent-44
  LOW).
- **Old text (line 57 partial)**:
  > Lip-sync LSE-D / LSE-C via SyncNet (Chung & Zisserman,
  > 2017) is instrument-ready; if it does not land for this
  > draft, the single-value callout is deferred to v2.
- **New text (line 57 partial)**:
  > Lip-sync LSE-D / LSE-C via SyncNet-based scoring is
  > instrument-ready; if it does not land for this draft,
  > the single-value callout is deferred to v2.

---

## Edit 7 — §7.3 discussion (FLAG-5/6/7 verification-classic drop)

- **File**: `sections/08_discussion.md`
- **Rationale**: Kinnunen & Li (2010), Schroff et al.
  (2015), Bromley et al. (1993) not in master pool;
  rewrite of sentence preserves argument without named
  citations (agent-44 alternative path for MEDIUM).
- **Old text (line 21 partial)**:
  > Classical verification systems — speaker verification
  > (Kinnunen & Li, 2010), face verification (Schroff et
  > al., 2015), Siamese networks (Bromley et al., 1993) —
  > use thresholded distance to a reference at $M = 1$
  > post-hoc; they do not run inside the generation loop
  > and do not compose conjunctively over multiple
  > encoders.
- **New text (line 21 partial)**:
  > Classical speaker-, face-, and Siamese-style
  > verification systems use thresholded distance to a
  > reference at $M = 1$ post-hoc; they do not run inside
  > the generation loop and do not compose conjunctively
  > over multiple encoders.

---

## Edit 8 — §7.6 discussion (FLAG-8 Epoch AI drop)

- **File**: `sections/08_discussion.md`
- **Rationale**: Epoch AI 2024 is the research-org
  publisher of Villalobos et al. (2024); dropping the
  redundant parenthetical (agent-44 MEDIUM).
- **Old text (line 39 partial)**:
  > For **scaling-law policy**, the reading is that the
  > data-wall discourse (Villalobos et al., 2024; Epoch AI,
  > 2024) — framed as Path 1 (more human data) and Path 2
  > (synthetic generation) — admits a third path
- **New text (line 39 partial)**:
  > For **scaling-law policy**, the reading is that the
  > data-wall discourse (Villalobos et al., 2024) — framed
  > as Path 1 (more human data) and Path 2 (synthetic
  > generation) — admits a third path

---

## Edits to sections/11_references.md

**NONE.** The reference list was already internally
consistent. All 63 entries remained intact after body-text
fixes; every one continues to be cited at least once in the
body after the 8 FLAG-removals because the FLAGs pointed to
NOT-in-list citations that were removed from the body,
leaving the list unaffected.

---

## Verification

Post-edit grep sweep for the seven removed citation patterns
returns zero matches across all body sections
(sections/00_*.md through sections/10_*.md):

```
grep -l "Efrati & Holmes|Shannon, 1948|Frantar|Chung & Zisserman|Kinnunen|Schroff|Bromley|Epoch AI, 2024|Narayanan, Bharadia" sections/0[0-9]_*.md
→ 0 matches
```

---

## Changelog

- 2026-04-17 — Edits logged by agent-45-citation-validator.
