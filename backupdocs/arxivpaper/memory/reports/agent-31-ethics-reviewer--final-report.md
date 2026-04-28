---
namespace: reports
agent_id: agent-31-ethics-reviewer
timestamp: 2026-04-16T21:00:00Z
status: active
tags: smap, final-report, ethics, consent, misuse, dual-use, coi
---

# Agent-31 Ethics Reviewer — Final Report (SMAP)

## S — Situation

The arXiv submission under preparation is a methods paper on
**Teleological Constellation Training (TCT)** as a meaning-compression
approach to identity-locked generation, validated across three case
studies. Two of the three cases are live ethics territory:

- **Case 2 "Santa"** — identity-locked talking-head clone trained on a
  16-minute 15-second interview of a **third-party subject**, 2,362
  curated clips, LoRA rank 256 on EchoMimicV3, mouth-weighted flow-
  matching. Subject-specific artefacts (footage, clips, centroids,
  LoRA weights) exist.
- **Case 3 "Voice clone"** — voice cloning pipeline on the **author
  (Chris Royse)** as the sole subject, 0.961 mean WavLM SECS measured,
  DNSMOS / UTMOS parity with real recordings. 489-clip speaker corpus,
  50-clip reference centroid, two 8–9 s clean-mic references, 10
  held-out author-recorded sentences.

Case 1 (Shakespeare stylistic LoRA) is a public-domain corpus study
with no living-subject concern.

The author's affiliations are **Teleox.ai** (primary, for-profit
early-stage entity where the author is founder / principal technical
officer) and **Leapable.ai** (companion marketplace, author is founder).

Upstream agents (agent-29, agent-30) flagged three ethics-adjacent
risks for deepening:
- **R4-02** Case 2 subject consent (RPN 8) — verify documentation.
- **R4-03** Commercial-affiliation COI (RPN 6) — draft the footnote.
- **Deepfake / misuse** (unranked by agent-30) — agent-31 was asked to
  rule on artefact-release scope.

NeurIPS 2024, ICML 2025, and ICLR 2025 ethics checklists are all
active for identity-cloning work. arXiv cs.LG moderators routinely
flag novel-vocabulary identity-cloning papers for hold review.

## M — Mission

Produce a paste-ready §Ethics section (400–600 words), a paste-ready
§Competing Interests disclosure, a misuse-and-mitigation audit with
adversary threat modelling, and a reproducibility-vs-responsible-
disclosure trade-off analysis. Verify Case 2 consent scope covers the
five items demanded by 45 CFR 46 Common Rule and modern venue
checklists. Hand off to agent-32 (contradiction-analyzer) with six
contradiction-pair flags.

## A — Actions

Six actions completed:

### A.1 Read upstream context (6 files)
- Mission brief; case-study protocols (agent-25); sampling protocols
  (agent-26); validity threats (agent-29); FMEA register (agent-30);
  handoff from agent-30.
- Cross-referenced agent-14 bias audit (Bias-05 / Bias-06 / Bias-08 /
  Bias-09), agent-01 failure-modes (FM-01 / FM-04 / FM-05 / FM-09),
  and Case 2 / Case 3 protocol specifics from the prior manuscript's
  §08 and §09.

### A.2 Draft §Ethics (paste-ready, 540 words)
- Six subsections: Subject relationships; Consent; Dual-use posture;
  Artefact-release scope; Responsible disclosure; IRB / ethics review
  status.
- File: `./memory/decisions/agent-31-ethics-reviewer--ethics-statement.md`.

### A.3 Draft §Competing Interests (paste-ready, two variants)
- Page-1 footnote (~60 words) + full §Competing Interests (~180
  words).
- Embedded FM-01 regex compliance checklist for agent-42 (body text
  vendor-name scan).
- File: `./memory/decisions/agent-31-ethics-reviewer--competing-interests.md`.

### A.4 Build misuse-and-mitigation audit
- Adversary threat model (moderately-resourced actor, consumer GPU,
  public Hugging Face weights, may have target-subject data).
- Four capability categories analysed: clone our Case 2 subject
  (blocked); clone author/Case 3 (blocked from released artefacts;
  already publicly cloneable by pre-existing baselines); clone
  arbitrary third-party subject via our method (enabled with bounded
  marginal uplift; residual is the per-output verification primitive);
  use TCT defensively (actively encouraged).
- Explicit allowlist (9 items released) and blocklist (10 items
  withheld).
- Reproducibility trade-off framed: Case 1 byte-identical
  reproducible; Cases 2 & 3 protocol-level reproducible only.
- File: `./memory/discoveries/agent-31-ethics-reviewer--misuse-mitigation.md`.

### A.5 Write handoff to agent-32 (contradiction-analyzer)
- Six contradiction pairs flagged for shadow-search:
  (1) reproducibility claim vs. artefact withholding;
  (2) sample-complexity claim vs. misuse mitigation;
  (3) author-as-subject vs. author-as-scorer bias stack;
  (4) commercial affiliation vs. methodology novelty claim;
  (5) "identity-locked" framing vs. consent revocation capability;
  (6) IRB-status disclosure vs. downstream-venue expectation.
- File: `./memory/handoff/agent-31-to-32--ethics-ready.md`.

### A.6 Produce SMAP final report
- This file.

## P — Plan (and carries to agent-47 + principal author)

### Bright-line confirmations required before submission lock

1. **Case 2 subject written informed consent on file** covering five
   scope items (research use; manuscript publication; case-study
   description; commercial-use scope; revocation mechanism).
2. **No public synthetic artefact of Case 2 subject** has been or will
   be released.
3. **Teleox.ai / Leapable.ai affiliation description** in §Competing
   Interests matches author's actual roles and equity stake as of
   submission date.
4. **No third-party subject data** beyond the two declared subjects
   (Case 2 subject; Case 3 author).

### Submission-lock checklist items owned by agent-31

| Item | Owner | Status |
|---|---|---|
| §Ethics paste-ready delivered | agent-31 | **DONE** |
| §Competing Interests paste-ready delivered | agent-31 | **DONE** |
| Misuse-and-mitigation audit delivered | agent-31 | **DONE** |
| FM-01 regex compliance spec | agent-31 → agent-42 | **DONE** (handed to agent-42) |
| Four bright-line confirmations collected | principal author → agent-47 | **PENDING** (pre-submission-lock gate) |
| §6.2 opener paste-ready sentence | agent-31 → agent-37/38 | **DONE** (in ethics-statement doc §3) |
| §6.3 opener paste-ready sentence | agent-31 → agent-37/38 | **DONE** (in ethics-statement doc §3) |
| §7 Discussion dual-use paste-ready sentence | agent-31 → agent-39 | **DONE** (in ethics-statement doc §3) |
| §10 Future Work items (watermarking, provenance, detection) | agent-31 → agent-45 | **DONE** (named in ethics-statement) |
| Independent ethics advisor pre-arrangement | principal author | **PENDING** (not a v1 blocker; downstream venue carry) |

### Post-submission watch items

- EU AI Act implementation timeline (may constrain v2).
- US state-level deepfake legislation (watch CA, NY, MA).
- Consent revocation mechanism — if Case 2 subject revokes, v2
  redaction path is already clean (artefacts are already withheld;
  only the case-study description needs rework, and that reworks at
  protocol level without subject identifiers).

### Files written

- `./memory/decisions/agent-31-ethics-reviewer--ethics-statement.md`
- `./memory/decisions/agent-31-ethics-reviewer--competing-interests.md`
- `./memory/discoveries/agent-31-ethics-reviewer--misuse-mitigation.md`
- `./memory/handoff/agent-31-to-32--ethics-ready.md`
- `./memory/reports/agent-31-ethics-reviewer--final-report.md` (this file)

## Three-sentence summary (ethics statement + explicit misuse mitigations)

1. The **§Ethics statement** (540 words, paste-ready) discloses that
   Case 2 uses a consented third-party subject (written informed
   consent covering research use, publication, case-study description,
   commercial-use scope, and revocation mechanism), Case 3 uses the
   author as unambiguously-consenting subject, Case 1 uses a
   public-domain corpus, the methodology is **inherently dual-use**
   (defensive as an anti-drift guard, offensive as an identity-lock
   enabler), and the paper adopts an **asymmetric artefact-release
   posture** (protocols, pipelines, and Case 1 Shakespeare stack
   released in full; Case 2 footage / clips / centroids / LoRA weights
   and Case 3 author voice corpus / centroid / reference recordings
   withheld).
2. The **explicit misuse mitigations** are: (a) subject-specific weight
   and clip withholding, so a reader cannot clone our Case 2 subject or
   our author from our released artefacts alone; (b) per-output
   provenance traces (cosine + centroid hash) attached to every
   generated artefact in the pipeline by design; (c) recommended
   pairings with output watermarking (AudioSeal / Stable Signature),
   cryptographic provenance metadata (C2PA-style), and cross-encoder
   detection-side research, named explicitly in §Ethics Responsible
   Disclosure and in §10 Future Work; (d) the encoder-matched
   evaluation disclosure (§6.3, §8 L-01) which doubles as guidance to
   detectors on what to look for.
3. The **reproducibility-vs-responsible-disclosure trade-off** is stated
   openly: Case 1 is byte-identical reproducible (flagship); Cases 2
   and 3 are **protocol-level reproducible only**, so a third-party
   replicator must supply their own consented subject data, and this is
   accepted as the scientifically-and-ethically-correct posture for an
   arXiv cs.LG identity-cloning methods paper under current NeurIPS /
   ICML / ICLR venue norms.

## Confidence

**HIGH** overall. The six ethics axes requested by the mission brief
(consent, misuse, deepfake policy, COI, provenance, dual-use) are
exhaustively addressed. Paste-ready artefacts are ready for
downstream writers. The asymmetric release posture is
venue-aligned. Four bright-line confirmations give agent-47 a clean
submission-lock gate.

**MEDIUM** on two carry items: downstream venues may demand formal
IRB documentation (principal author task); future regulatory
environments may constrain v2 (monitoring task).

## Changelog

- 2026-04-16 — Initial issue by agent-31-ethics-reviewer.
